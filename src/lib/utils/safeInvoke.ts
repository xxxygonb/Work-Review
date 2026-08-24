import { invoke as tauriInvoke } from '@tauri-apps/api/core';

const DEFAULT_API_PORT = 47831;
const WEB_FRONTEND_PORT = 5173;
const API_HOST = '127.0.0.1';

let cachedBaseUrl: string | null = null;
let tauriAvailable: boolean | null = null;

function detectTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

/**
 * 当浏览器直接访问 Web 端（http://localhost:5173 或 http://127.0.0.1:47831）时，
 * 页面 origin 本身就是我们托管前端的 HTTP 服务器，同时承担反向代理 / API 入口。
 * 这种场景无需探测端口，直接使用相对路径即可：
 *   - 保证 100% 同源，不存在 CORS 预检 / 混合内容问题；
 *   - 省去端口探测超时，首屏加载更快；
 *   - 5173 会走同源代理到 47831，47831 本身就带 API。
 */
function originMatchesLocalServer(): string | null {
  if (typeof window === 'undefined' || !window.location) return null;
  const { hostname, port } = window.location;
  const isLoopback = hostname === 'localhost' || hostname === '127.0.0.1' || hostname === '[::1]';
  if (!isLoopback) return null;
  const p = Number(port);
  if (p === WEB_FRONTEND_PORT || p === DEFAULT_API_PORT || p === 47832 || p === 47833) {
    return ''; // 空字符串即 baseUrl，等价于相对路径：/v1/xxx
  }
  return null;
}

async function discoverApiBaseUrl(): Promise<string | null> {
  if (cachedBaseUrl !== null) return cachedBaseUrl;

  // 浏览器直接访问 localhost:5173 / 127.0.0.1:47831 → 同源相对路径，跳过探测
  const selfHosted = originMatchesLocalServer();
  if (selfHosted !== null) {
    cachedBaseUrl = selfHosted;
    return cachedBaseUrl;
  }

  const ports = [DEFAULT_API_PORT, 47832, 47833];
  for (const port of ports) {
    try {
      const resp = await fetch(`http://${API_HOST}:${port}/health`, {
        signal: AbortSignal.timeout(1500),
      });
      if (resp.ok) {
        cachedBaseUrl = `http://${API_HOST}:${port}`;
        return cachedBaseUrl;
      }
    } catch {
      // try next port
    }
  }
  return null;
}


const COMMAND_ENDPOINT_MAP: Record<string, (args: Record<string, unknown>) => { method: string; path: string; body?: unknown }> = {
  get_config: () => ({ method: 'GET', path: '/v1/config' }),
  save_config: (args) => ({ method: 'PUT', path: '/v1/config', body: args.config }),
  verify_password: (args) => ({ method: 'POST', path: '/v1/verify-password', body: { password: args.password } }),
  change_password: (args) => ({ method: 'POST', path: '/v1/change-password', body: { old_password: args.old_password, new_password: args.new_password } }),
  get_recording_state: () => ({ method: 'GET', path: '/v1/recording-state' }),
  get_platform: () => ({ method: 'GET', path: '/v1/platform' }),
  get_today_stats: () => ({ method: 'GET', path: '/v1/stats/today' }),
  get_overview_stats: (args) => {
    const params = new URLSearchParams();
    if (args.mode) params.set('mode', String(args.mode));
    if (args.date) params.set('date', String(args.date));
    if (args.dateFrom) params.set('date_from', String(args.dateFrom));
    if (args.dateTo) params.set('date_to', String(args.dateTo));
    if (args.date_from) params.set('date_from', String(args.date_from));
    if (args.date_to) params.set('date_to', String(args.date_to));
    return { method: 'GET', path: `/v1/stats/overview?${params}` };
  },
  get_overview_domains: (args) => {
    const params = new URLSearchParams();
    if (args.mode) params.set('mode', String(args.mode));
    if (args.date) params.set('date', String(args.date));
    if (args.dateFrom) params.set('date_from', String(args.dateFrom));
    if (args.dateTo) params.set('date_to', String(args.dateTo));
    if (args.date_from) params.set('date_from', String(args.date_from));
    if (args.date_to) params.set('date_to', String(args.date_to));
    return { method: 'GET', path: `/v1/stats/overview/domains?${params}` };
  },
  get_overview_domain_detail: (args) => {
    const params = new URLSearchParams();
    if (args.mode) params.set('mode', String(args.mode));
    if (args.date) params.set('date', String(args.date));
    if (args.dateFrom) params.set('date_from', String(args.dateFrom));
    if (args.dateTo) params.set('date_to', String(args.dateTo));
    if (args.date_from) params.set('date_from', String(args.date_from));
    if (args.date_to) params.set('date_to', String(args.date_to));
    const domain = String(args.domain || '');
    return { method: 'GET', path: `/v1/stats/overview/domains/${encodeURIComponent(domain)}?${params}` };
  },
  get_daily_stats: (args) => ({ method: 'GET', path: `/v1/stats/daily/${args.date}` }),
  get_range_daily_totals: (args) => {
    const params = new URLSearchParams();
    if (args.dateFrom) params.set('date_from', String(args.dateFrom));
    if (args.dateTo) params.set('date_to', String(args.dateTo));
    if (args.date_from) params.set('date_from', String(args.date_from));
    if (args.date_to) params.set('date_to', String(args.date_to));
    return { method: 'GET', path: `/v1/stats/range-daily-totals?${params}` };
  },
  get_hourly_app_breakdown: (args) => {
    const params = new URLSearchParams();
    if (args.date) params.set('date', String(args.date));
    if (args.mode) params.set('mode', String(args.mode));
    if (args.dateFrom) params.set('date_from', String(args.dateFrom));
    if (args.dateTo) params.set('date_to', String(args.dateTo));
    if (args.date_from) params.set('date_from', String(args.date_from));
    if (args.date_to) params.set('date_to', String(args.date_to));
    const datePart = args.date ? `/${args.date}` : '';
    return { method: 'GET', path: `/v1/hourly-app-breakdown${datePart}?${params}` };
  },
  get_timeline: (args) => {
    const params = new URLSearchParams();
    if (args.limit) params.set('limit', String(args.limit));
    if (args.offset) params.set('offset', String(args.offset));
    return { method: 'GET', path: `/v1/timeline/${args.date}?${params}` };
  },
  get_activity: (args) => ({ method: 'GET', path: `/v1/activity/${args.id}` }),
  get_saved_report: (args) => {
    const params = new URLSearchParams();
    if (args.locale) params.set('locale', String(args.locale));
    return { method: 'GET', path: `/v1/reports/${args.date}?${params}` };
  },
  generate_report: (args) => ({
    method: 'POST',
    path: '/v1/reports/generate',
    body: { date: args.date, force: args.force ?? false, locale: args.locale },
  }),
  update_report_content: (args) => ({
    method: 'PUT',
    path: `/v1/reports/${args.date}/content`,
    body: { locale: args.locale, content: args.content },
  }),
  set_report_block_preference: (args) => ({
    method: 'PUT',
    path: '/v1/reports/block-preference',
    body: { pinnedBlocks: args.pinnedBlocks, hiddenBlocks: args.hiddenBlocks },
  }),
  export_report_markdown: (args) => ({
    method: 'POST',
    path: '/v1/reports/export-markdown',
    body: { date: args.date, content: args.content, exportDir: args.exportDir, locale: args.locale },
  }),
  export_reports_range: (args) => ({
    method: 'POST',
    path: '/v1/reports/export-range',
    body: args,
  }),
  get_categories: () => ({ method: 'GET', path: '/v1/categories' }),
  get_semantic_categories: () => ({ method: 'GET', path: '/v1/categories/semantic' }),
  save_custom_category: (args) => ({ method: 'POST', path: '/v1/categories/custom', body: args }),
  delete_custom_category: (args) => ({ method: 'DELETE', path: `/v1/categories/custom/${encodeURIComponent(String(args.key))}` }),
  save_custom_semantic_category: (args) => ({ method: 'POST', path: '/v1/categories/semantic/custom', body: args }),
  delete_custom_semantic_category: (args) => ({ method: 'DELETE', path: `/v1/categories/semantic/custom/${encodeURIComponent(String(args.key))}` }),
  set_app_category_rule: (args) => ({ method: 'PUT', path: '/v1/apps/category-rule', body: args }),
  set_domain_semantic_rule: (args) => ({ method: 'PUT', path: '/v1/domains/semantic-rule', body: args }),
  get_hourly_summaries: (args) => ({ method: 'GET', path: `/v1/hourly-summaries/${args.date}` }),
  get_storage_stats: () => ({ method: 'GET', path: '/v1/storage/stats' }),
  get_screenshot_thumbnail: (args) => {
    const params = new URLSearchParams();
    params.set('path', String(args.path));
    return { method: 'GET', path: `/v1/screenshot-thumbnail?${params}` };
  },
  get_screenshot_full: (args) => {
    const params = new URLSearchParams();
    params.set('path', String(args.path));
    return { method: 'GET', path: `/v1/screenshot-full?${params}` };
  },
  get_background_image: () => ({ method: 'GET', path: '/v1/background-image' }),
  save_background_image: (args) => ({ method: 'PUT', path: '/v1/background-image', body: args }),
  clear_background_image: () => ({ method: 'DELETE', path: '/v1/background-image' }),
  get_data_dir: () => ({ method: 'GET', path: '/v1/data-dir' }),
  get_default_data_dir: () => ({ method: 'GET', path: '/v1/default-data-dir' }),
  get_runtime_platform: () => ({ method: 'GET', path: '/v1/platform' }),
  get_recent_apps: () => ({ method: 'GET', path: '/v1/apps/recent' }),
  get_running_apps: () => ({ method: 'GET', path: '/v1/apps/running' }),
  get_app_category_overview: () => ({ method: 'GET', path: '/v1/apps/category-overview' }),
  get_localhost_api_status: () => ({ method: 'GET', path: '/v1/localhost-api-status' }),
  reveal_localhost_api_token: () => ({ method: 'GET', path: '/v1/auth/token' }),
  rotate_localhost_api_token: () => ({ method: 'POST', path: '/v1/auth/token/rotate' }),
  get_node_gateway_status: () => ({ method: 'GET', path: '/v1/node-gateway/status' }),
  get_telegram_bot_status: () => ({ method: 'GET', path: '/v1/telegram/status' }),
  generate_telegram_bot_bind_code: () => ({ method: 'POST', path: '/v1/telegram/bind-code' }),
  is_autostart_enabled: () => ({ method: 'GET', path: '/v1/autostart' }),
  enable_autostart: (args) => ({ method: 'POST', path: '/v1/autostart/enable', body: args }),
  disable_autostart: () => ({ method: 'POST', path: '/v1/autostart/disable' }),
  open_data_dir: () => ({ method: 'POST', path: '/v1/data-dir/open' }),
  change_data_dir: (args) => ({ method: 'PUT', path: '/v1/data-dir', body: args }),
  cleanup_old_data_dir: (args) => ({ method: 'POST', path: '/v1/data-dir/cleanup', body: args }),
  clear_old_activities: (args) => ({ method: 'POST', path: '/v1/activities/clear-old', body: args }),
  test_remote_storage: (args) => ({ method: 'POST', path: '/v1/storage/test-remote', body: args }),
  check_permissions: () => ({ method: 'GET', path: '/v1/permissions' }),
  open_permission_settings: (args) => ({ method: 'POST', path: '/v1/permissions/open', body: args }),
  get_linux_session_support: () => ({ method: 'GET', path: '/v1/linux-session' }),
  delete_activity: (args) => ({ method: 'DELETE', path: `/v1/activities/${args.id}` }),
  delete_activities_by_date: (args) => ({ method: 'DELETE', path: '/v1/activities/by-date', body: args }),
  delete_activities_by_range: (args) => ({ method: 'DELETE', path: '/v1/activities/by-range', body: args }),
  delete_activities_by_app: (args) => ({ method: 'DELETE', path: '/v1/activities/by-app', body: args }),
  export_timeline_json: (args) => ({ method: 'POST', path: '/v1/timeline/export', body: args }),
  pause_recording: () => ({ method: 'POST', path: '/v1/recording/pause' }),
  resume_recording: () => ({ method: 'POST', path: '/v1/recording/resume' }),
  get_avatar_state: () => ({ method: 'GET', path: '/v1/avatar/state' }),
  save_avatar_position: () => ({ method: 'POST', path: '/v1/avatar/position' }),
  persist_avatar_position: () => ({ method: 'POST', path: '/v1/avatar/persist-position' }),
  set_avatar_window_expanded: () => ({ method: 'POST', path: '/v1/avatar/expanded' }),
  set_avatar_interactive_regions: () => ({ method: 'POST', path: '/v1/avatar/interactive-regions' }),
  set_dock_visibility: (args) => ({ method: 'PUT', path: '/v1/dock-visibility', body: args }),
  show_main_window: () => ({ method: 'POST', path: '/v1/window/show' }),
  get_app_icon: (args) => ({ method: 'GET', path: `/v1/app-icon/${encodeURIComponent(String(args.appName || ''))}?executablePath=${encodeURIComponent(String(args.executablePath || ''))}` }),
};

/**
 * 从各种异常形态里尽量提取人类可读的错误详情。
 * 很多时候 Promise reject 不是 Error 实例（或者 message 很模糊），
 * 需要一层兜底，避免用户只能看到"undefined"或"请重试"。
 */
function extractInvokeError(e: unknown): { msg: string; detail?: string } {
  if (e instanceof Error) {
    // 如果 stack 里含有更具体的响应体（比如 "[safeInvoke] xxx 回退失败 (400): ..."），
    // 就把响应体作为 detail 方便定位。
    const m = e.message.match(/^(.*?)\s*\((\d{3})\):\s*([\s\S]*)$/);
    if (m) {
      return { msg: `${m[1]} (${m[2]})`, detail: m[3].slice(0, 300) };
    }
    return { msg: e.message };
  }
  if (typeof e === 'string' && e) {
    return { msg: e };
  }
  if (e && typeof e === 'object') {
    try {
      return { msg: '调用异常', detail: JSON.stringify(e).slice(0, 300) };
    } catch {
      return { msg: String(e) };
    }
  }
  return { msg: '未知错误' };
}

async function httpFallback<T>(command: string, args: Record<string, unknown>): Promise<T> {
  const baseUrl = await discoverApiBaseUrl();
  // 注意：baseUrl=""（空串）是合法值 —— 代表当前 origin 同源的相对路径（/v1/xxx）。
  // 仅当 baseUrl 真的为 null 时才表示端口探测失败 / 无法连接本地 API。
  if (baseUrl === null) {
    throw new Error(`[safeInvoke] 无法连接本地 API 服务器，请确认应用已启动 (命令: ${command})`);
  }

  const mapper = COMMAND_ENDPOINT_MAP[command];
  if (!mapper) {
    throw new Error(`[safeInvoke] 命令 "${command}" 未注册 HTTP 回退映射`);
  }

  const { method, path, body } = mapper(args);
  const url = `${baseUrl}${path}`;

  const headers: Record<string, string> = { 'Content-Type': 'application/json' };
  const init: RequestInit = { method, headers };
  if (body !== undefined && method !== 'GET') {
    init.body = JSON.stringify(body);
  }

  const response = await fetch(url, init);

  const responseText = await response.text();

  if (!response.ok) {
    let errorDetail = '';
    try {
      const errBody = JSON.parse(responseText);
      errorDetail = errBody.error || JSON.stringify(errBody);
    } catch {
      errorDetail = responseText;
    }
    throw new Error(`[safeInvoke] ${command} 回退失败 (${response.status}): ${errorDetail}`);
  }

  let data: unknown;
  try {
    data = JSON.parse(responseText);
  } catch {
    throw new Error(`[safeInvoke] ${command} 响应非有效 JSON: ${responseText.slice(0, 200)}`);
  }

  const CONTENT_WRAPPED_COMMANDS = new Set([
    'get_screenshot_thumbnail',
    'get_screenshot_full',
    'generate_report',
    'get_app_icon',
  ]);
  if (CONTENT_WRAPPED_COMMANDS.has(command) && data && typeof data === 'object' && 'content' in data) {
    return data.content as T;
  }

  if (command === 'verify_password' && data && typeof data === 'object' && 'matched' in data) {
    return (data as { matched: boolean }).matched as T;
  }

  return data as T;
}

export { extractInvokeError };

export async function invoke<T = unknown>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (tauriAvailable === null) {
    tauriAvailable = detectTauri();
  }

  if (tauriAvailable) {
    return tauriInvoke<T>(command, args);
  }

  return httpFallback<T>(command, args || {});
}

export function isTauriEnvironment(): boolean {
  if (tauriAvailable === null) {
    tauriAvailable = detectTauri();
  }
  return tauriAvailable;
}