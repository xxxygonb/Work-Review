# AI 修改记录

## 2026-08-18

### 1. 修复网页端无数据 Bug

**问题**：桌面端（Tauri）有数据，但网页端（纯浏览器访问 Vite dev server）所有数据为空。

**原因**：
1. 前端通过 `@tauri-apps/api/core` 的 `invoke` 与后端通信，纯浏览器环境下 Tauri IPC 桥不存在
2. 本地 API 服务器未设置 CORS 响应头，浏览器跨域请求被拦截

**修复方案**：
1. 创建跨环境安全调用工具函数，在 Tauri 环境使用原生 invoke，在浏览器环境回退到 localhost REST API
2. 为本地 API 服务器添加 CORS 响应头（Access-Control-Allow-Origin 等）及 OPTIONS 预检请求处理

#### 新增文件
- `src/lib/utils/safeInvoke.ts` — 跨环境 invoke 实现，包含：
  - Tauri 环境检测（`__TAURI_INTERNALS__`）
  - 本地 API 服务器自动发现（端口 47831/47832/47833）
  - 完整的命令→REST 端点映射表（COMMAND_ENDPOINT_MAP）
  - HTTP 回退请求逻辑

#### 修改文件（前端 invoke 导入替换）
- `src/App.svelte` — `@tauri-apps/api/core` → `$lib/utils/safeInvoke.ts`
- `src/routes/Overview.svelte` — 同上
- `src/routes/timeline/Timeline.svelte` — 同上
- `src/routes/report/Report.svelte` — 同上
- `src/routes/settings/Settings.svelte` — 同上
- `src/routes/settings/components/SettingsAI.svelte` — 同上
- `src/routes/settings/components/nodeGateway/LocalApiPanel.svelte` — 同上
- `src/routes/settings/components/nodeGateway/TelegramBotPanel.svelte` — 同上
- `src/routes/settings/components/nodeGateway/ApiExamplesPanel.svelte` — 同上
- `src/routes/settings/components/AssistantMemoryManager.svelte` — 同上
- `src/lib/components/AppUsageChart.svelte` — 同上
- `src/lib/components/Sidebar.svelte` — 同上
- `src/lib/stores/categories.ts` — 同上
- `src/routes/timeline/timelineGateway.ts` — 同上
- `src/routes/avatar/AvatarWindow.svelte` — 同上
- `src/routes/ask/Ask.svelte` — 同上（保留 Channel 从原 @tauri-apps/api/core 导入）

#### 修改文件（后端新增 REST 端点）
- `src-tauri/src/localhost_api.rs` — 新增以下端点：
  - `GET /v1/config` — 获取配置
  - `PUT /v1/config` — 保存配置
  - `GET /v1/recording-state` — 获取录制状态
  - `GET /v1/platform` — 获取运行平台
  - `GET /v1/stats/range-daily-totals` — 获取日期范围每日统计
  - `GET /v1/stats/overview/domains` — 获取概览域名集合
  - `GET /v1/stats/overview/domains/{domain}` — 获取单域名详情
  - `GET /v1/activity/{id}` — 获取单个活动
  - `GET /v1/screenshot-thumbnail` — 获取截图缩略图
  - `GET /v1/screenshot-full` — 获取完整截图
  - `GET /v1/background-image` — 获取背景图
  - `GET /v1/data-dir` — 获取数据目录
  - `GET /v1/default-data-dir` — 获取默认数据目录
  - `GET /v1/apps/running` — 获取运行中的应用
  - `GET /v1/localhost-api-status` — 获取本地 API 状态
  - `GET /v1/auth/token` — 获取 API Token
  - `POST /v1/auth/token/rotate` — 轮换 API Token
  - `GET /v1/node-gateway/status` — 获取节点网关状态
  - `GET /v1/telegram/status` — 获取 Telegram Bot 状态
  - `POST /v1/telegram/bind-code` — 生成 Telegram 绑定码
  - `GET /v1/autostart` — 获取自启动状态
  - `POST /v1/autostart/enable` — 启用自启动
  - `POST /v1/autostart/disable` — 禁用自启动
  - `GET /v1/linux-session` — 获取 Linux 会话信息
  - 更新 `request_auth_mode` 白名单，允许新端点免 Token 访问
  - **添加 CORS 支持**：`HttpResponse::to_bytes()` 增加 `Access-Control-Allow-Origin: *`、`Access-Control-Allow-Methods`、`Access-Control-Allow-Headers`、`Access-Control-Max-Age` 响应头
  - **添加 OPTIONS 预检请求处理**：`route_request()` 函数开头拦截 OPTIONS 方法，返回 204 No Content
  - **添加 `/v1/app-icon/{appName}` 端点**：支持网页端获取应用图标 base64 数据
  - **修复 REST API 返回值格式**：`safeInvoke.ts` 中添加 `CONTENT_WRAPPED_COMMANDS` 集合，自动解包 `{ content: ... }` 包装格式，确保截图、图标等命令返回值与 Tauri invoke 一致

- `src-tauri/src/commands/stats.rs` — 将内部函数改为 `pub(crate)`：
  - `resolve_overview_date_span`
  - `load_full_overview_stats`
  - `build_overview_domain_collection`
  - `build_overview_domain_detail`
  - 新增 `get_running_apps_inner` 公开包装函数

- `src-tauri/src/commands/system.rs` — 新增 `get_app_icon_inner` 公开包装函数，供 localhost_api 调用

- `src-tauri/src/commands/mod.rs` —
  - `stats` 模块改为 `pub(crate)`
  - 重新导出 `validate_relative_path`

### 2. 删除其它语言支持（仅保留中文）

#### 删除文件
- `src/lib/i18n/locales/en.ts` — 英文语言包
- `src/lib/i18n/locales/zh-TW.ts` — 繁体中文语言包
- `src/lib/i18n/locales/ar.ts` — 阿拉伯文语言包

#### 修改文件
- `src/lib/i18n/index.ts` —
  - `SUPPORTED_LOCALES` 仅保留 `['zh-CN']`
  - `LOCALE_CYCLE` 仅保留 `['zh-CN']`
  - `LOCALE_META` 仅保留简体中文
  - `CATEGORY_LABELS` / `SEMANTIC_LABELS` 仅保留 zh-CN 条目
  - `MESSAGES` 仅加载 zh-CN
  - `normalizeLocale` / `initializeLocale` / `setLocale` / `cycleLocale` 简化为固定返回 zh-CN
  - `formatDurationLocalized` 移除 en/ar/zh-TW 分支
  - `applyLocaleToDocument` 固定 ltr 方向

- `src/lib/components/Sidebar.svelte` — 语言选项仅保留简体中文
- `src/routes/settings/components/SettingsAI.svelte` — providerLabels 仅保留 zh-CN
- `src/routes/avatar/AvatarWindow.svelte` — RUNTIME_BUBBLE_MESSAGES 仅保留 zh-CN
- `src/routes/ask/modelPresentation.ts` — ProviderLabelLocale 仅保留 zh-CN
- `src/routes/report/reportSections.ts` — sectionNumberPrefix 固定使用中文序号
- `src/lib/components/Avatar/avatarStateMeta.ts` — AvatarBubbleLocale 仅保留 zh-CN，所有多语言对象仅保留 zh-CN 条目