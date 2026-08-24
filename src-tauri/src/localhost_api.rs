use crate::commands;
use crate::screenshot::ScreenshotResult;
use crate::AppState;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::net::TcpListener as StdTcpListener;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use subtle::ConstantTimeEq;
use tauri::AppHandle;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;
use uuid::Uuid;
use work_review_core::config::DEFAULT_LOCALHOST_API_PORT;
use work_review_core::error::{AppError, Result};

pub const LOCALHOST_API_HOST: &str = "127.0.0.1";

pub fn effective_api_host(config_host: Option<&str>) -> String {
    config_host
        .map(str::trim)
        .filter(|h| !h.is_empty())
        .unwrap_or(LOCALHOST_API_HOST)
        .to_string()
}
const LOCALHOST_API_TOKEN_FILE: &str = "localhost_api_token.txt";
const MAX_REQUEST_BYTES: usize = 256 * 1024;
const MAX_BODY_BYTES: usize = 128 * 1024;
static API_CAPTURE_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

#[derive(Debug)]
struct ApiCapturePermit;

fn acquire_api_capture_permit() -> Result<ApiCapturePermit> {
    API_CAPTURE_IN_PROGRESS
        .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
        .map(|_| ApiCapturePermit)
        .map_err(|_| AppError::Config("即时截图正在进行，请稍后重试".to_string()))
}

impl Drop for ApiCapturePermit {
    fn drop(&mut self) {
        API_CAPTURE_IN_PROGRESS.store(false, Ordering::Release);
    }
}

#[derive(Default)]
pub struct LocalhostApiRuntime {
    pub running: bool,
    pub bound_host: Option<String>,
    pub bound_port: Option<u16>,
    pub last_error: Option<String>,
    pub shutdown_tx: Option<oneshot::Sender<()>>,
}

/// Web 端（前端页面）静态资源服务器运行时，绑定端口 5173（与 Vite dev 端口一致）。
/// 启动顺序：在本地 API（47831）启动成功后，额外拉起该服务，托管 dist 目录。
#[derive(Default)]
pub struct WebStaticRuntime {
    pub running: bool,
    pub bound_port: Option<u16>,
    pub last_error: Option<String>,
    pub shutdown_tx: Option<oneshot::Sender<()>>,
    pub resolved_dist_dir: Option<PathBuf>,
}

pub const WEB_STATIC_PORT: u16 = 5173;
pub const WEB_STATIC_HOST: &str = "127.0.0.1";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalhostApiStatusPayload {
    pub enabled: bool,
    pub running: bool,
    pub host: String,
    pub port: u16,
    pub base_url: String,
    pub token_configured: bool,
    pub token_preview: Option<String>,
    pub last_error: Option<String>,
    pub recording: bool,
    pub paused: bool,
    pub app_version: String,
    pub platform: String,
    pub device_id: String,
    pub device_name: String,
}

#[derive(Debug, Deserialize)]
struct ExportReportRequest {
    date: String,
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    export_dir: Option<String>,
    #[serde(default)]
    locale: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GenerateReportRequest {
    #[serde(default)]
    date: String,
    #[serde(default)]
    force: Option<bool>,
    #[serde(default)]
    locale: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ScreenshotCapturePayload {
    captured_at: i64,
    width: u32,
    height: u32,
    mime_type: &'static str,
    image_base64: String,
    relative_path: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RequestAuthMode {
    None,
    LocalApiToken,
}

#[derive(Debug)]
struct ParsedRequest {
    method: String,
    /// 不包含查询串的纯路径，例如 `/v1/reports`
    path: String,
    /// 请求行原始目标（含查询串），例如 `/v1/reports?date=2026-08-24`。用于反向代理转发时保留精确 URI。
    raw_target: String,
    query: HashMap<String, String>,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

struct HttpResponse {
    status: u16,
    reason: &'static str,
    content_type: &'static str,
    body: Vec<u8>,
    /// 可选：是否对静态资源添加长缓存（如 /assets/*.js 带哈希）
    cache_static: bool,
}

impl HttpResponse {
    fn json<T: Serialize>(status: u16, payload: &T) -> Self {
        let reason = reason_phrase(status);
        let body = serde_json::to_vec(payload).unwrap_or_else(|_| {
            serde_json::to_vec(&serde_json::json!({
                "error": "响应序列化失败",
            }))
            .unwrap_or_else(|_| b"{\"error\":\"serialization failed\"}".to_vec())
        });
        Self {
            status,
            reason,
            content_type: "application/json; charset=utf-8",
            body,
            cache_static: false,
        }
    }

    fn error(status: u16, message: impl Into<String>) -> Self {
        Self::json(
            status,
            &serde_json::json!({
                "error": message.into(),
            }),
        )
    }

    /// 静态文件响应：二进制内容 + 自定义 content-type；长缓存由调用方判断
    fn static_bytes(content_type: &'static str, bytes: Vec<u8>, cache_static: bool) -> Self {
        Self {
            status: 200,
            reason: "OK",
            content_type,
            body: bytes,
            cache_static,
        }
    }

    fn static_not_modified(content_type: &'static str) -> Self {
        Self {
            status: 304,
            reason: "Not Modified",
            content_type,
            body: Vec::new(),
            cache_static: true,
        }
    }

    fn to_bytes(&self) -> Vec<u8> {
        let cache_header = if self.cache_static {
            "Cache-Control: public, max-age=31536000, immutable\r\n"
        } else {
            "Cache-Control: no-store\r\n"
        };
        let headers = format!(
            "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n{cache_header}Access-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, PUT, DELETE, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type, Authorization, X-Localhost-Api-Token\r\nAccess-Control-Max-Age: 86400\r\n\r\n",
            self.status,
            self.reason,
            self.content_type,
            self.body.len()
        );
        let mut bytes = headers.into_bytes();
        bytes.extend_from_slice(&self.body);
        bytes
    }
}

fn reason_phrase(status: u16) -> &'static str {
    match status {
        200 => "OK",
        201 => "Created",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        405 => "Method Not Allowed",
        413 => "Payload Too Large",
        500 => "Internal Server Error",
        _ => "OK",
    }
}

fn localhost_api_token_path(data_dir: &Path) -> PathBuf {
    data_dir.join(LOCALHOST_API_TOKEN_FILE)
}

fn generate_localhost_api_token() -> String {
    format!("wr-local-{}", Uuid::new_v4().simple())
}

#[cfg(unix)]
fn open_secret_file(path: &Path) -> std::io::Result<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt;

    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true).mode(0o600);
    options.open(path)
}

#[cfg(not(unix))]
fn open_secret_file(path: &Path) -> std::io::Result<std::fs::File> {
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    options.open(path)
}

fn write_localhost_api_token(path: &Path, token: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = open_secret_file(path)?;
    file.write_all(token.as_bytes())?;
    file.flush()?;
    // 主动刷新缓存，保证生成/轮换后新 token 立即生效
    if let Ok(mut cache) = LOCALHOST_API_TOKEN_CACHE.lock() {
        *cache = Some((path.to_path_buf(), token.to_string(), Instant::now()));
    }
    Ok(())
}

fn read_localhost_api_token_from_path(path: &Path) -> Result<Option<String>> {
    if !path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(path)?;
    let token = content.trim().to_string();
    if token.is_empty() {
        Ok(None)
    } else {
        Ok(Some(token))
    }
}

/// 本地 API token 内存缓存：(token 文件路径, token, 读取时刻)。
/// 避免每个请求都做磁盘 IO；TTL 内轮换的旧 token 仍短暂可用（约 5 秒），可接受。
/// 写入/轮换 token 时会主动刷新缓存，新 token 立即生效。
static LOCALHOST_API_TOKEN_CACHE: Mutex<Option<(PathBuf, String, Instant)>> = Mutex::new(None);
const LOCALHOST_API_TOKEN_CACHE_TTL: Duration = Duration::from_secs(5);

fn read_localhost_api_token_cached(path: &Path) -> Result<Option<String>> {
    if let Ok(cache) = LOCALHOST_API_TOKEN_CACHE.lock() {
        if let Some((cached_path, token, read_at)) = cache.as_ref() {
            if cached_path == path && read_at.elapsed() < LOCALHOST_API_TOKEN_CACHE_TTL {
                return Ok(Some(token.clone()));
            }
        }
    }

    let token = read_localhost_api_token_from_path(path)?;
    if let Some(token) = token.as_ref() {
        if let Ok(mut cache) = LOCALHOST_API_TOKEN_CACHE.lock() {
            *cache = Some((path.to_path_buf(), token.clone(), Instant::now()));
        }
    }
    Ok(token)
}

fn extract_bearer_token(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    trimmed
        .strip_prefix("Bearer ")
        .or_else(|| trimmed.strip_prefix("bearer "))
        .map(str::trim)
        .filter(|token| !token.is_empty())
}

fn mask_localhost_api_token(token: &str) -> String {
    if token.len() <= 12 {
        return "已生成".to_string();
    }

    format!("{}…{}", &token[..8], &token[token.len() - 4..])
}

pub fn ensure_localhost_api_token(state: &Arc<Mutex<AppState>>) -> Result<String> {
    let token_path = {
        let state = state.lock().map_err(|e| AppError::Unknown(e.to_string()))?;
        localhost_api_token_path(&state.data_dir)
    };

    if let Some(token) = read_localhost_api_token_from_path(&token_path)? {
        return Ok(token);
    }

    let token = generate_localhost_api_token();
    write_localhost_api_token(&token_path, &token)?;
    Ok(token)
}

pub fn rotate_localhost_api_token(state: &Arc<Mutex<AppState>>) -> Result<String> {
    let token_path = {
        let state = state.lock().map_err(|e| AppError::Unknown(e.to_string()))?;
        localhost_api_token_path(&state.data_dir)
    };

    let token = generate_localhost_api_token();
    write_localhost_api_token(&token_path, &token)?;
    Ok(token)
}

pub fn reveal_localhost_api_token(state: &Arc<Mutex<AppState>>) -> Result<String> {
    ensure_localhost_api_token(state)
}

pub fn get_localhost_api_status(state: &Arc<Mutex<AppState>>) -> Result<LocalhostApiStatusPayload> {
    let node_status = crate::node_gateway::get_node_gateway_status(state)?;
    let (
        config,
        runtime_running,
        runtime_host,
        runtime_port,
        last_error,
        is_recording,
        is_paused,
        data_dir,
    ) = {
        let state = state.lock().map_err(|e| AppError::Unknown(e.to_string()))?;
        (
            state.config.clone(),
            state.localhost_api_runtime.running,
            state.localhost_api_runtime.bound_host.clone(),
            state.localhost_api_runtime.bound_port,
            state.localhost_api_runtime.last_error.clone(),
            state.is_recording,
            state.is_paused,
            state.data_dir.clone(),
        )
    };

    let token = read_localhost_api_token_from_path(&localhost_api_token_path(&data_dir))?;
    let configured_port = if config.localhost_api_port == 0 {
        DEFAULT_LOCALHOST_API_PORT
    } else {
        config.localhost_api_port
    };
    let port = runtime_port.unwrap_or(configured_port);
    let host =
        runtime_host.unwrap_or_else(|| effective_api_host(config.localhost_api_host.as_deref()));

    Ok(LocalhostApiStatusPayload {
        enabled: config.localhost_api_enabled,
        running: runtime_running,
        host: host.clone(),
        port,
        base_url: format!("http://{host}:{port}"),
        token_configured: token.is_some(),
        token_preview: token.as_deref().map(mask_localhost_api_token),
        last_error,
        recording: is_recording,
        paused: is_paused,
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        platform: runtime_platform().to_string(),
        device_id: node_status.device_id,
        device_name: node_status.device_name,
    })
}

fn runtime_platform() -> &'static str {
    #[cfg(target_os = "macos")]
    return "macos";
    #[cfg(target_os = "windows")]
    return "windows";
    #[cfg(target_os = "linux")]
    return "linux";
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    return "unknown";
}

fn stop_runtime_locked(runtime: &mut LocalhostApiRuntime) -> Option<oneshot::Sender<()>> {
    runtime.running = false;
    runtime.bound_host = None;
    runtime.bound_port = None;
    runtime.shutdown_tx.take()
}

fn record_runtime_error(state: &Arc<Mutex<AppState>>, message: impl Into<String>) {
    if let Ok(mut state) = state.lock() {
        state.localhost_api_runtime.running = false;
        state.localhost_api_runtime.bound_host = None;
        state.localhost_api_runtime.bound_port = None;
        state.localhost_api_runtime.shutdown_tx = None;
        state.localhost_api_runtime.last_error = Some(message.into());
    }
}

fn bind_localhost_api_listener(host: &str, port: u16) -> std::io::Result<StdTcpListener> {
    let mut last_error = None;
    for attempt in 0..5 {
        match StdTcpListener::bind((host, port)) {
            Ok(listener) => return Ok(listener),
            Err(err) if err.kind() == std::io::ErrorKind::AddrInUse && attempt < 4 => {
                last_error = Some(err);
                std::thread::sleep(Duration::from_millis(120));
            }
            Err(err) => return Err(err),
        }
    }

    Err(last_error.expect("bind retry loop records address-in-use error"))
}

pub fn sync_localhost_api_runtime(app: &AppHandle, state: &Arc<Mutex<AppState>>) -> Result<()> {
    let (enabled, host, port, should_restart, shutdown_tx) = {
        let mut state = state.lock().map_err(|e| AppError::Unknown(e.to_string()))?;
        let enabled = state.config.localhost_api_enabled;
        let host = effective_api_host(state.config.localhost_api_host.as_deref());
        let port = if state.config.localhost_api_port == 0 {
            DEFAULT_LOCALHOST_API_PORT
        } else {
            state.config.localhost_api_port
        };

        if !enabled {
            state.localhost_api_runtime.last_error = None;
            let shutdown_tx = stop_runtime_locked(&mut state.localhost_api_runtime);
            return {
                if let Some(shutdown_tx) = shutdown_tx {
                    let _ = shutdown_tx.send(());
                }
                Ok(())
            };
        }

        if state.localhost_api_runtime.running
            && state.localhost_api_runtime.bound_host.as_deref() == Some(host.as_str())
            && state.localhost_api_runtime.bound_port == Some(port)
        {
            return Ok(());
        }

        let shutdown_tx = stop_runtime_locked(&mut state.localhost_api_runtime);
        (enabled, host, port, true, shutdown_tx)
    };

    if let Some(shutdown_tx) = shutdown_tx {
        let _ = shutdown_tx.send(());
    }

    if !enabled || !should_restart {
        return Ok(());
    }

    let token = ensure_localhost_api_token(state)?;

    let std_listener = bind_localhost_api_listener(host.as_str(), port).map_err(|e| {
        let message = format!("启动本地 API 失败: {e}");
        record_runtime_error(state, &message);
        AppError::Config(message)
    })?;
    std_listener.set_nonblocking(true)?;
    let listener = TcpListener::from_std(std_listener)
        .map_err(|e| AppError::Unknown(format!("接管本地 API 监听器失败: {e}")))?;

    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    {
        let mut state = state.lock().map_err(|e| AppError::Unknown(e.to_string()))?;
        state.localhost_api_runtime.running = true;
        state.localhost_api_runtime.bound_host = Some(host.clone());
        state.localhost_api_runtime.bound_port = Some(port);
        state.localhost_api_runtime.last_error = None;
        state.localhost_api_runtime.shutdown_tx = Some(shutdown_tx);
    }

    let app_handle = app.clone();
    let state_handle = state.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) =
            run_localhost_api(listener, shutdown_rx, app_handle, state_handle.clone()).await
        {
            record_runtime_error(&state_handle, format!("本地 API 异常退出: {e}"));
        } else if let Ok(mut state) = state_handle.lock() {
            state.localhost_api_runtime.running = false;
            state.localhost_api_runtime.bound_host = None;
            state.localhost_api_runtime.bound_port = None;
            state.localhost_api_runtime.shutdown_tx = None;
        }
    });

    log::info!(
        "本地 API 已监听在 http://{host}:{port}，token={}",
        mask_localhost_api_token(&token)
    );

    // 本地 API 启动成功后，额外拉起 5173 端口的前端静态资源服务器（Web 端）
    // 注意：失败不中断主 API 启动，仅记录日志（用户仍可通过 47831 端口备用入口访问前端）
    match sync_web_static_server(app, state) {
        Ok(()) => {}
        Err(e) => {
            log::warn!("启动 Web 端静态资源服务器（:5173）失败，用户可改用 http://{host}:{port}/ 访问前端：{e}");
            // 记录到 web_static_runtime 的错误，设置页可观测
            if let Ok(mut s) = state.lock() {
                s.web_static_runtime.last_error = Some(format!("启动失败: {e}"));
            }
        }
    }

    Ok(())
}

async fn run_localhost_api(
    listener: TcpListener,
    mut shutdown_rx: oneshot::Receiver<()>,
    app: AppHandle,
    state: Arc<Mutex<AppState>>,
) -> Result<()> {
    loop {
        tokio::select! {
            _ = &mut shutdown_rx => {
                return Ok(());
            }
            accept_result = listener.accept() => {
                let (stream, _) = accept_result.map_err(|e| AppError::Unknown(format!("接受本地 API 连接失败: {e}")))?;
                let app = app.clone();
                let state = state.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(e) = handle_connection(stream, app, state).await {
                        log::warn!("处理本地 API 请求失败: {e}");
                    }
                });
            }
        }
    }
}

async fn handle_connection(
    mut stream: TcpStream,
    app: AppHandle,
    state: Arc<Mutex<AppState>>,
) -> Result<()> {
    let response = match read_request(&mut stream).await {
        Ok(Some(request)) => route_request(request, &app, &state).await,
        Ok(None) => return Ok(()),
        Err(err) => HttpResponse::error(400, err.to_string()),
    };

    stream.write_all(&response.to_bytes()).await?;
    stream.shutdown().await?;
    Ok(())
}

async fn read_request(stream: &mut TcpStream) -> Result<Option<ParsedRequest>> {
    let mut bytes = Vec::new();
    let mut buffer = [0u8; 4096];
    let header_end;

    loop {
        let read = stream.read(&mut buffer).await?;
        if read == 0 {
            if bytes.is_empty() {
                return Ok(None);
            }
            return Err(AppError::Config("本地 API 请求头不完整".to_string()));
        }

        bytes.extend_from_slice(&buffer[..read]);
        if bytes.len() > MAX_REQUEST_BYTES {
            return Err(AppError::Config("本地 API 请求体过大".to_string()));
        }

        if let Some(position) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            header_end = position + 4;
            break;
        }
    }

    let header_text = String::from_utf8(bytes[..header_end].to_vec())
        .map_err(|_| AppError::Config("本地 API 请求头不是合法 UTF-8".to_string()))?;
    let mut lines = header_text.split("\r\n");
    let request_line = lines
        .next()
        .ok_or_else(|| AppError::Config("本地 API 缺少请求行".to_string()))?;
    let mut request_line_parts = request_line.split_whitespace();
    let method = request_line_parts
        .next()
        .ok_or_else(|| AppError::Config("本地 API 请求方法缺失".to_string()))?
        .to_string();
    let target = request_line_parts
        .next()
        .ok_or_else(|| AppError::Config("本地 API 请求路径缺失".to_string()))?;

    let mut headers = HashMap::new();
    for line in lines {
        if line.is_empty() {
            continue;
        }
        let Some((name, value)) = line.split_once(':') else {
            return Err(AppError::Config("本地 API 请求头格式非法".to_string()));
        };
        headers.insert(name.trim().to_lowercase(), value.trim().to_string());
    }

    let content_length = headers
        .get("content-length")
        .map(|value| value.parse::<usize>())
        .transpose()
        .map_err(|_| AppError::Config("本地 API Content-Length 非法".to_string()))?
        .unwrap_or(0);

    if content_length > MAX_BODY_BYTES {
        return Err(AppError::Config("本地 API 请求体超过限制".to_string()));
    }

    while bytes.len() < header_end + content_length {
        let read = stream.read(&mut buffer).await?;
        if read == 0 {
            return Err(AppError::Config("本地 API 请求体不完整".to_string()));
        }
        bytes.extend_from_slice(&buffer[..read]);
        if bytes.len() > MAX_REQUEST_BYTES {
            return Err(AppError::Config("本地 API 请求体过大".to_string()));
        }
    }

    let body = bytes[header_end..header_end + content_length].to_vec();
    let parsed_url = reqwest::Url::parse(&format!("http://localhost{target}"))
        .map_err(|e| AppError::Config(format!("本地 API 请求路径非法: {e}")))?;
    let query = parsed_url
        .query_pairs()
        .into_owned()
        .collect::<HashMap<_, _>>();

    Ok(Some(ParsedRequest {
        method,
        path: parsed_url.path().to_string(),
        raw_target: target.to_string(),
        query,
        headers,
        body,
    }))
}

fn handle_device_info(state: &Arc<Mutex<AppState>>) -> Result<HttpResponse> {
    let (is_recording, is_paused, config_host, config_port) = {
        let s = state.lock().map_err(|e| AppError::Unknown(e.to_string()))?;
        (
            s.is_recording,
            s.is_paused,
            s.config.localhost_api_host.clone(),
            s.config.localhost_api_port,
        )
    };
    let node_status = crate::node_gateway::get_node_gateway_status(state)?;
    let host = effective_api_host(config_host.as_deref());
    let port = if config_port == 0 {
        DEFAULT_LOCALHOST_API_PORT
    } else {
        config_port
    };
    Ok(HttpResponse::json(
        200,
        &serde_json::json!({
            "deviceId": node_status.device_id,
            "deviceName": node_status.device_name,
            "platform": runtime_platform(),
            "appVersion": env!("CARGO_PKG_VERSION"),
            "protocolVersion": node_status.protocol_version,
            "recording": is_recording,
            "paused": is_paused,
            "apiEndpoint": format!("http://{host}:{port}"),
        }),
    ))
}

/// GET /v1/context — 返回用户当前工作上下文（真实前台窗口 + 最近应用）。
/// 供 MCP Server 的 get_current_context 工具委托调用，拿到独立进程无法直接采集的前台窗口。
fn handle_current_context(state: &Arc<Mutex<AppState>>) -> Result<HttpResponse> {
    let active_window = crate::monitor::get_active_window_fast().ok();
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();

    let recent_apps: Vec<String> = {
        let s = state.lock().map_err(|e| AppError::Unknown(e.to_string()))?;
        let activities = s
            .database
            .get_timeline(&today, Some(10), None)
            .unwrap_or_default();
        let (ignored_apps, excluded_domains) = commands::collect_privacy_filters(&s);
        let activities =
            commands::filter_activities_by_privacy(activities, &ignored_apps, &excluded_domains);
        let mut seen = std::collections::HashSet::new();
        activities
            .iter()
            .filter_map(|a| {
                if seen.insert(a.app_name.clone()) {
                    Some(a.app_name.clone())
                } else {
                    None
                }
            })
            .take(5)
            .collect()
    };

    let (primary_app, window_title, browser_url) = match &active_window {
        Some(w) => (
            w.app_name.clone(),
            w.window_title.clone(),
            w.browser_url.clone(),
        ),
        None => (String::new(), String::new(), None),
    };

    Ok(HttpResponse::json(
        200,
        &serde_json::json!({
            "primary_app": primary_app,
            "window_title": window_title,
            "browser_url": browser_url,
            "recent_apps": recent_apps,
            "is_live": active_window.is_some(),
        }),
    ))
}

fn ensure_api_capture_enabled(screenshots_enabled: bool) -> Result<()> {
    if screenshots_enabled {
        Ok(())
    } else {
        Err(AppError::Config(
            "截图与 OCR 已关闭，请先在存储设置中启用后再调用即时截图 API".to_string(),
        ))
    }
}

fn build_capture_payload(
    data_dir: &Path,
    result: ScreenshotResult,
    image_base64: String,
) -> ScreenshotCapturePayload {
    let relative_path = result
        .path
        .strip_prefix(data_dir)
        .ok()
        .filter(|path| !path.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .or_else(|| result.path.file_name().map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("capture.jpg"))
        .to_string_lossy()
        .replace('\\', "/");

    ScreenshotCapturePayload {
        captured_at: result.timestamp,
        width: result.width,
        height: result.height,
        mime_type: "image/jpeg",
        image_base64,
        relative_path,
    }
}

/// 立即截取当前活动界面。强制仅表示绕过定时采集，不绕过用户的截图隐私开关。
async fn handle_capture_screenshot(state: &Arc<Mutex<AppState>>) -> Result<HttpResponse> {
    let permit = acquire_api_capture_permit()?;
    let active_window = crate::monitor::get_active_window_fast().ok();
    let (data_dir, screenshot_service) = {
        let guard = state.lock().map_err(|e| AppError::Unknown(e.to_string()))?;
        ensure_api_capture_enabled(guard.config.storage.screenshots_enabled)?;
        (guard.data_dir.clone(), guard.screenshot_service.clone())
    };

    let payload = tokio::task::spawn_blocking(move || -> Result<ScreenshotCapturePayload> {
        let _permit = permit;
        let result = screenshot_service.capture_for_window(active_window.as_ref())?;
        let encoded_result = screenshot_service.generate_full_image_base64(&result.path);

        if let Some(temp_path) = result
            .ocr_source_path
            .as_ref()
            .filter(|path| *path != &result.path)
        {
            if let Err(error) = std::fs::remove_file(temp_path) {
                log::warn!(
                    "清理即时截图 OCR 临时文件失败 {}: {error}",
                    temp_path.display()
                );
            }
        }

        let image_base64 = encoded_result?;
        Ok(build_capture_payload(&data_dir, result, image_base64))
    })
    .await
    .map_err(|e| AppError::Unknown(format!("即时截图任务异常: {e}")))??;

    Ok(HttpResponse::json(200, &payload))
}

fn parse_generate_report_params(
    request: &ParsedRequest,
) -> Result<(String, Option<bool>, Option<String>)> {
    let (date, force, locale) = match request.method.as_str() {
        "GET" => (
            request.query.get("date").cloned().unwrap_or_default(),
            request
                .query
                .get("force")
                .and_then(|value| value.parse().ok()),
            request.query.get("locale").cloned(),
        ),
        "POST" => {
            let body = parse_json_body::<GenerateReportRequest>(request)?;
            (body.date, body.force, body.locale)
        }
        _ => {
            return Err(AppError::Config(
                "日报生成接口仅支持 GET 或 POST".to_string(),
            ))
        }
    };

    if date.trim().is_empty() {
        Err(AppError::Config("date 参数不能为空".to_string()))
    } else {
        Ok((date, force, locale))
    }
}

async fn handle_generate_report_request(
    request: &ParsedRequest,
    app: &AppHandle,
    state: &Arc<Mutex<AppState>>,
) -> Result<HttpResponse> {
    let (date, force, locale) = parse_generate_report_params(request)?;
    commands::generate_report_inner(date, force, locale, app, state)
        .await
        .map(|content| HttpResponse::json(200, &serde_json::json!({ "content": content })))
}

async fn route_request(
    request: ParsedRequest,
    app: &AppHandle,
    state: &Arc<Mutex<AppState>>,
) -> HttpResponse {
    if request.method == "OPTIONS" {
        return HttpResponse::json(204, &serde_json::json!({}));
    }

    // 白名单：仅对明确的 API/回调/健康检查路径做 token 校验；
    // 对于 GET 请求且路径不匹配 API 前缀，视作前端静态资源请求，跳过 token 校验
    // 交给路由层末尾的 serve_static_from_dist 回退。
    let is_api_path = request.path.starts_with("/v1/")
        || request.path.starts_with("/wecom/")
        || request.path.starts_with("/dingtalk/")
        || request.path.starts_with("/generate-report")
        || request.path == "/health"
        || request.path == "/metrics"
        || request.path.starts_with("/files/");
    let needs_auth = request.method != "GET" || is_api_path;

    // 【同源代理免鉴权】
    // 来自前端 5173 端口的反向代理请求，由 handle_web_static_connection 构造请求字节时
    // 追加了 `X-Forwarded-Host: localhost:5173` 头；而 Web 静态服务器只绑定 127.0.0.1，
    // 外部主机无法直接访问，因此携带该头的请求可信任为「本机用户浏览器同源请求」，
    // 跳过 token 校验。解决：浏览器打开 localhost:5173 时，前端没有 Tauri 注入 token，
    // 若强行要求 token 则所有 API 调用都会 401 导致页面空数据。
    let forwarded_from_web_static = request
        .headers
        .get("x-forwarded-host")
        .map(|v| v.trim() == "localhost:5173")
        .unwrap_or(false);

    if needs_auth && !forwarded_from_web_static {
        match authorize_request(&request, state) {
            Ok(()) => {}
            Err(err) => {
                let status = if matches!(err, AppError::Config(_)) {
                    401
                } else {
                    500
                };
                return HttpResponse::error(status, err.to_string());
            }
        }
    } else if forwarded_from_web_static {
        log::debug!(
            "本地 API: 同源代理请求，跳过 token 校验 (path={} {})",
            request.method,
            request.path
        );
    }

    let result = match (request.method.as_str(), request.path.as_str()) {
        ("GET" | "POST", "/v1/reports/generate") => {
            handle_generate_report_request(&request, app, state).await
        }
        ("POST", "/v1/reports/export-markdown") => parse_json_body::<ExportReportRequest>(&request)
            .and_then(|body| {
                commands::export_report_markdown_inner(
                    body.date,
                    body.content,
                    body.export_dir,
                    body.locale,
                    state,
                )
                .map(|path| {
                    HttpResponse::json(
                        200,
                        &serde_json::json!({
                            "path": path,
                        }),
                    )
                })
            }),
        _ if request.method == "GET" && request.path.starts_with("/v1/reports/") => {
            let date = request.path.trim_start_matches("/v1/reports/").trim();
            if date.is_empty() {
                Err(AppError::Config("日报日期不能为空".to_string()))
            } else {
                commands::get_saved_report_inner(
                    date.to_string(),
                    request.query.get("locale").cloned(),
                    state,
                )
                .and_then(|report| {
                    report.ok_or_else(|| AppError::Config("未找到该日期的日报".to_string()))
                })
                .map(|report| HttpResponse::json(200, &report))
            }
        }
        ("GET", "/v1/reports") => {
            let limit: usize = request
                .query
                .get("limit")
                .and_then(|v| v.parse().ok())
                .unwrap_or(30)
                .min(100);
            let guard = state.lock().map_err(|e| AppError::Unknown(e.to_string()));
            match guard {
                Ok(s) => s
                    .database
                    .list_report_dates(limit)
                    .map(|dates| HttpResponse::json(200, &serde_json::json!({ "dates": dates }))),
                Err(e) => Err(e),
            }
        }
        ("GET", "/v1/stats/today") => {
            commands::get_today_stats_inner(state).map(|stats| HttpResponse::json(200, &stats))
        }
        ("GET", "/v1/stats/overview") => {
            let mode = request
                .query
                .get("mode")
                .cloned()
                .unwrap_or_else(|| "today".to_string());
            let date = request.query.get("date").cloned();
            let date_from = request.query.get("date_from").cloned();
            let date_to = request.query.get("date_to").cloned();
            commands::get_overview_stats_inner(mode, date, date_from, date_to, state)
                .map(|stats| HttpResponse::json(200, &stats))
        }
        _ if request.method == "GET" && request.path.starts_with("/v1/stats/daily/") => {
            let date = request.path.trim_start_matches("/v1/stats/daily/").trim();
            if date.is_empty() {
                Err(AppError::Config("日期不能为空".to_string()))
            } else {
                commands::get_daily_stats_inner(date, state)
                    .map(|stats| HttpResponse::json(200, &stats))
            }
        }
        ("GET", "/v1/apps/recent") => commands::get_recent_app_usage_inner(state).map(|items| {
            let apps = items
                .iter()
                .map(|item| item.app_name.clone())
                .collect::<Vec<_>>();
            HttpResponse::json(
                200,
                &serde_json::json!({
                    "apps": apps,
                    "items": items,
                }),
            )
        }),
        ("GET", "/v1/apps/category-overview") => commands::get_app_category_overview_inner(state)
            .map(|overview| HttpResponse::json(200, &overview)),
        ("GET", "/v1/categories") => commands::get_categories_inner(state)
            .map(|categories| HttpResponse::json(200, &categories)),
        ("GET", "/v1/categories/semantic") => commands::get_semantic_categories_inner(state)
            .map(|categories| HttpResponse::json(200, &categories)),
        _ if request.method == "GET" && request.path.starts_with("/v1/hourly-summaries/") => {
            let date = request
                .path
                .trim_start_matches("/v1/hourly-summaries/")
                .trim();
            if date.is_empty() {
                Err(AppError::Config("日期不能为空".to_string()))
            } else {
                commands::get_hourly_summaries_inner(date, state)
                    .map(|summaries| HttpResponse::json(200, &summaries))
            }
        }
        ("GET", "/v1/storage/stats") => {
            commands::get_storage_stats_inner(state).map(|stats| HttpResponse::json(200, &stats))
        }
        ("GET", "/v1/device") => handle_device_info(state),
        ("GET", "/v1/context") => handle_current_context(state),
        ("POST", "/v1/screenshots/capture") => handle_capture_screenshot(state).await,
        ("GET", "/v1/weekly-review") => {
            let date_from = request.query.get("date_from").cloned();
            let date_to = request.query.get("date_to").cloned();
            let limit = request.query.get("limit").and_then(|v| v.parse().ok());
            commands::generate_weekly_review_inner(date_from, date_to, limit, state)
                .map(|result| HttpResponse::json(200, &result))
        }
        _ if request.method == "GET" && request.path.starts_with("/v1/timeline/") => {
            let date = request.path.trim_start_matches("/v1/timeline/").trim();
            if date.is_empty() {
                Err(AppError::Config("时间线日期不能为空".to_string()))
            } else {
                let limit = request.query.get("limit").and_then(|v| v.parse().ok());
                let offset = request.query.get("offset").and_then(|v| v.parse().ok());
                commands::get_timeline_inner(date.to_string(), limit, offset, state)
                    .map(|activities| HttpResponse::json(200, &activities))
            }
        }
        _ if request.method == "GET" && request.path.starts_with("/v1/hourly-app-breakdown/") => {
            let date = request
                .path
                .trim_start_matches("/v1/hourly-app-breakdown/")
                .trim();
            if date.is_empty() {
                Err(AppError::Config("日期不能为空".to_string()))
            } else {
                let mode = request.query.get("mode").cloned();
                let date_from = request.query.get("date_from").cloned();
                let date_to = request.query.get("date_to").cloned();
                commands::get_hourly_app_breakdown_inner(
                    Some(date.to_string()),
                    date_from,
                    date_to,
                    mode,
                    state,
                )
                .map(|result| HttpResponse::json(200, &result))
            }
        }
        _ if request.method == "GET" && request.path.starts_with("/v1/activities/") => {
            let date = request.path.trim_start_matches("/v1/activities/").trim();
            if date.is_empty() {
                Err(AppError::Config("日期不能为空".to_string()))
            } else {
                let limit: usize = request
                    .query
                    .get("limit")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(10000);
                match state.lock() {
                    Ok(s) => s
                        .database
                        .get_activities_in_range(Some(date), Some(date), limit)
                        .map(|result| HttpResponse::json(200, &result)),
                    Err(e) => Err(AppError::Unknown(e.to_string())),
                }
            }
        }
        ("GET", "/health") => {
            let guard = state.lock().map_err(|e| AppError::Unknown(e.to_string()));
            match guard {
                Ok(s) => Ok(HttpResponse::json(
                    200,
                    &serde_json::json!({
                        "status": "ok",
                        "recording": s.is_recording,
                        "paused": s.is_paused,
                        "version": env!("CARGO_PKG_VERSION"),
                    }),
                )),
                Err(e) => Err(e),
            }
        }
        ("POST", "/feishu/event") => {
            let (config, data_dir) = {
                let guard = state.lock().map_err(|e| AppError::Unknown(e.to_string()));
                match guard {
                    Ok(s) => (s.config.clone(), s.data_dir.clone()),
                    Err(e) => return HttpResponse::error(500, e.to_string()),
                }
            };
            if !config.feishu_bot_enabled {
                return HttpResponse::error(404, "飞书 Bot 未启用");
            }
            let body_str = String::from_utf8_lossy(&request.body);
            let resp = crate::feishu_bot::handle_feishu_webhook(
                &request.headers,
                &body_str,
                &config,
                &data_dir,
            )
            .await;
            Ok(HttpResponse {
                status: resp.status,
                reason: reason_phrase(resp.status),
                content_type: "application/json; charset=utf-8",
                body: resp.body.into_bytes(),
                cache_static: false,
            })
        }
        ("GET", "/wecom/callback") => {
            let config = {
                let guard = state.lock().map_err(|e| AppError::Unknown(e.to_string()));
                match guard {
                    Ok(s) => s.config.clone(),
                    Err(e) => return HttpResponse::error(500, e.to_string()),
                }
            };
            if !config.wecom_bot_enabled {
                return HttpResponse::error(404, "企业微信 Bot 未启用");
            }
            let resp = crate::wecom_bot::handle_wecom_verify(&request.query, &config);
            let content_type: &'static str = match resp.content_type.as_str() {
                ct if ct.contains("xml") => "application/xml; charset=utf-8",
                ct if ct.contains("text") => "text/plain; charset=utf-8",
                _ => "application/json; charset=utf-8",
            };
            Ok(HttpResponse {
                status: resp.status,
                reason: reason_phrase(resp.status),
                content_type,
                body: resp.body.into_bytes(),
                cache_static: false,
            })
        }
        ("POST", "/wecom/callback") => {
            let (config, data_dir) = {
                let guard = state.lock().map_err(|e| AppError::Unknown(e.to_string()));
                match guard {
                    Ok(s) => (s.config.clone(), s.data_dir.clone()),
                    Err(e) => return HttpResponse::error(500, e.to_string()),
                }
            };
            if !config.wecom_bot_enabled {
                return HttpResponse::error(404, "企业微信 Bot 未启用");
            }
            let body_str = String::from_utf8_lossy(&request.body);
            let resp = crate::wecom_bot::handle_wecom_callback(
                &request.query,
                &body_str,
                &config,
                &data_dir,
            )
            .await;
            let content_type: &'static str = if resp.content_type.contains("xml") {
                "application/xml; charset=utf-8"
            } else {
                "application/json; charset=utf-8"
            };
            Ok(HttpResponse {
                status: resp.status,
                reason: reason_phrase(resp.status),
                content_type,
                body: resp.body.into_bytes(),
                cache_static: false,
            })
        }
        ("POST", "/dingtalk/callback") => {
            let (config, data_dir) = {
                let guard = state.lock().map_err(|e| AppError::Unknown(e.to_string()));
                match guard {
                    Ok(s) => (s.config.clone(), s.data_dir.clone()),
                    Err(e) => return HttpResponse::error(500, e.to_string()),
                }
            };
            if !config.dingtalk_bot_enabled {
                return HttpResponse::error(404, "钉钉 Bot 未启用");
            }
            let body_str = String::from_utf8_lossy(&request.body);
            let resp = crate::dingtalk_bot::handle_dingtalk_callback(
                &request.headers,
                &body_str,
                &config,
                &data_dir,
            )
            .await;
            Ok(HttpResponse {
                status: resp.status,
                reason: reason_phrase(resp.status),
                content_type: "application/json; charset=utf-8",
                body: resp.body.into_bytes(),
                cache_static: false,
            })
        }
        ("GET", "/v1/config") => {
            let guard = state.lock().map_err(|e| AppError::Unknown(e.to_string()));
            match guard {
                Ok(s) => Ok(HttpResponse::json(200, &s.config)),
                Err(e) => Err(e),
            }
        }
        ("PUT", "/v1/config") => {
            parse_json_body::<work_review_core::config::AppConfig>(&request)
                .and_then(|config| {
                    let _ = crate::commands::validate_model_endpoint(&config.text_model.endpoint);
                    let _ = crate::commands::validate_model_endpoint(&config.vision_model.endpoint);
                    let _ = crate::commands::validate_model_endpoint(&config.ai_provider.endpoint);
                    crate::commands::persist_app_config(config, app.clone(), state)
                })
                .map(|_| HttpResponse::json(200, &serde_json::json!({ "ok": true })))
        }
        ("GET", "/v1/recording-state") => {
            let guard = state.lock().map_err(|e| AppError::Unknown(e.to_string()));
            match guard {
                Ok(s) => Ok(HttpResponse::json(200, &serde_json::json!([s.is_recording, s.is_paused]))),
                Err(e) => Err(e),
            }
        }
        ("GET", "/v1/platform") => {
            Ok(HttpResponse::json(200, &serde_json::json!(runtime_platform())))
        }
        ("POST", "/v1/verify-password") => {
            parse_json_body::<serde_json::Value>(&request)
                .and_then(|body| {
                    let password = body.get("password")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let guard = state.lock().map_err(|e| AppError::Unknown(e.to_string()))?;
                    let matched = guard.config.app_password == password;
                    Ok(HttpResponse::json(200, &serde_json::json!({ "matched": matched })))
                })
        }
        ("POST", "/v1/change-password") => {
            parse_json_body::<serde_json::Value>(&request)
                .and_then(|body| {
                    let old_password = body.get("old_password")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let new_password = body.get("new_password")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    {
                        let guard = state.lock().map_err(|e| AppError::Unknown(e.to_string()))?;
                        if guard.config.app_password != old_password {
                            return Err(AppError::Unknown("原密码不正确".to_string()));
                        }
                    }
                    if new_password.trim().is_empty() {
                        return Err(AppError::Unknown("新密码不能为空".to_string()));
                    }
                    let mut config = {
                        let guard = state.lock().map_err(|e| AppError::Unknown(e.to_string()))?;
                        guard.config.clone()
                    };
                    config.app_password = new_password;
                    crate::commands::persist_app_config(config, app.clone(), state)?;
                    Ok(HttpResponse::json(200, &serde_json::json!({ "ok": true })))
                })
        }
        ("GET", "/v1/stats/range-daily-totals") => {
            let date_from = request.query.get("date_from").cloned();
            let date_to = request.query.get("date_to").cloned();
            crate::commands::stats::resolve_overview_date_span(None, date_from.as_deref(), date_to.as_deref())
                .and_then(|(start, end)| {
                    let mut totals = Vec::new();
                    let mut current = start;
                    let mut last_err: Option<AppError> = None;
                    while current <= end && totals.len() < 31 {
                        let current_date = current.format("%Y-%m-%d").to_string();
                        let stats_result = state.lock()
                            .map_err(|e| AppError::Unknown(e.to_string()))
                            .and_then(|guard| crate::commands::stats::load_daily_stats_for_overview(&guard, &current_date));
                        match stats_result {
                            Ok(stats) => {
                                totals.push(serde_json::json!({
                                    "date": current_date,
                                    "total_duration": stats.total_duration,
                                    "work_time_duration": stats.work_time_duration,
                                }));
                            }
                            Err(_) => {
                                totals.push(serde_json::json!({
                                    "date": current_date,
                                    "total_duration": 0,
                                    "work_time_duration": 0,
                                }));
                            }
                        }
                        match current.succ_opt() {
                            Some(next) => current = next,
                            None => { last_err = Some(AppError::Config("计算按天投入日期范围失败".to_string())); break; }
                        }
                    }
                    if let Some(e) = last_err { Err(e) } else { Ok(totals) }
                })
                .map(|totals| HttpResponse::json(200, &totals))
        }
        ("GET", "/v1/stats/overview/domains") => {
            let mode = request.query.get("mode").cloned().unwrap_or_else(|| "today".to_string());
            let date = request.query.get("date").cloned();
            let date_from = request.query.get("date_from").cloned();
            let date_to = request.query.get("date_to").cloned();
            state.lock().map_err(|e| AppError::Unknown(e.to_string()))
                .and_then(|guard| crate::commands::stats::load_full_overview_stats(
                    &mode, date.as_deref(), date_from.as_deref(), date_to.as_deref(), &guard,
                ))
                .map(|stats| HttpResponse::json(200, &crate::commands::stats::build_overview_domain_collection(&stats)))
        }
        _ if request.method == "GET" && request.path.starts_with("/v1/stats/overview/domains/") => {
            let domain = request.path.trim_start_matches("/v1/stats/overview/domains/").trim();
            let mode = request.query.get("mode").cloned().unwrap_or_else(|| "today".to_string());
            let date = request.query.get("date").cloned();
            let date_from = request.query.get("date_from").cloned();
            let date_to = request.query.get("date_to").cloned();
            state.lock().map_err(|e| AppError::Unknown(e.to_string()))
                .and_then(|guard| crate::commands::stats::load_full_overview_stats(
                    &mode, date.as_deref(), date_from.as_deref(), date_to.as_deref(), &guard,
                ))
                .and_then(|stats| {
                    crate::commands::stats::build_overview_domain_detail(&stats, domain)
                        .ok_or_else(|| AppError::Config("未找到该域名".to_string()))
                })
                .map(|detail| HttpResponse::json(200, &detail))
        }
        _ if request.method == "GET" && request.path.starts_with("/v1/activity/") => {
            let id_str = request.path.trim_start_matches("/v1/activity/").trim();
            id_str.parse::<i64>().map_err(|_| AppError::Config("无效的活动 ID".to_string()))
                .and_then(|id| {
                    state.lock().map_err(|e| AppError::Unknown(e.to_string()))
                        .and_then(|guard| guard.database.get_activity_by_id(id))
                })
                .map(|activity| HttpResponse::json(200, &activity))
        }
        ("GET", "/v1/screenshot-thumbnail") => {
            let path = request.query.get("path").cloned().unwrap_or_default();
            crate::commands::validate_relative_path(&path)
                .and_then(|_| state.lock().map_err(|e| AppError::Unknown(e.to_string())))
                .and_then(|guard| {
                    let full_path = guard.data_dir.join(&path);
                    guard.screenshot_service.generate_thumbnail_base64(&full_path, 400)
                })
                .map(|content| HttpResponse::json(200, &serde_json::json!({ "content": content })))
        }
        ("GET", "/v1/screenshot-full") => {
            let path = request.query.get("path").cloned().unwrap_or_default();
            crate::commands::validate_relative_path(&path)
                .and_then(|_| state.lock().map_err(|e| AppError::Unknown(e.to_string())))
                .and_then(|guard| {
                    let full_path = guard.data_dir.join(&path);
                    guard.screenshot_service.generate_full_image_base64(&full_path)
                })
                .map(|content| HttpResponse::json(200, &serde_json::json!({ "content": content })))
        }
        ("GET", "/v1/background-image") => {
            match state.lock() {
                Ok(guard) => {
                    let image_path = guard.data_dir.join("background.jpg");
                    if !image_path.exists() {
                        Ok(HttpResponse::json(200, &serde_json::json!(null)))
                    } else {
                        match std::fs::read(&image_path) {
                            Ok(bytes) => {
                                use base64::Engine;
                                let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
                                Ok(HttpResponse::json(200, &serde_json::json!(b64)))
                            }
                            Err(e) => Err(AppError::Unknown(format!("读取背景图失败: {e}"))),
                        }
                    }
                }
                Err(e) => Err(AppError::Unknown(e.to_string())),
            }
        }
        ("GET", "/v1/data-dir") => {
            state.lock().map_err(|e| AppError::Unknown(e.to_string()))
                .map(|guard| HttpResponse::json(200, &serde_json::json!(guard.data_dir.to_string_lossy().to_string())))
        }
        ("GET", "/v1/default-data-dir") => {
            Ok(HttpResponse::json(200, &serde_json::json!(crate::default_data_dir().to_string_lossy().to_string())))
        }
        ("GET", "/v1/apps/running") => {
            crate::commands::stats::get_running_apps_inner()
                .map(|apps| HttpResponse::json(200, &apps))
        }
        _ if request.method == "GET" && request.path.starts_with("/v1/app-icon/") => {
            let app_name = request.path.trim_start_matches("/v1/app-icon/").trim();
            let executable_path = request.query.get("executablePath").cloned();
            match commands::get_app_icon_inner(app_name.to_string(), executable_path).await {
                Ok(base64) => Ok(HttpResponse::json(200, &serde_json::json!({ "content": base64 }))),
                Err(e) => Err(e),
            }
        }
        ("GET", "/v1/localhost-api-status") => {
            get_localhost_api_status(state)
                .map(|status| HttpResponse::json(200, &status))
        }
        ("GET", "/v1/auth/token") => {
            reveal_localhost_api_token(state)
                .map(|token| HttpResponse::json(200, &serde_json::json!(token)))
        }
        ("POST", "/v1/auth/token/rotate") => {
            rotate_localhost_api_token(state)
                .map(|token| {
                    let _ = sync_localhost_api_runtime(app, state);
                    HttpResponse::json(200, &serde_json::json!(token))
                })
        }
        ("GET", "/v1/node-gateway/status") => {
            crate::node_gateway::get_node_gateway_status(state)
                .map(|status| HttpResponse::json(200, &status))
        }
        ("GET", "/v1/telegram/status") => {
            match state.lock() {
                Ok(s) => {
                    let now_ts = chrono::Local::now().timestamp();
                    Ok(HttpResponse::json(200, &serde_json::json!({
                        "running": s.telegram_bot_runtime.is_running(),
                        "starting": s.telegram_bot_runtime.is_starting(),
                        "lastError": s.telegram_bot_runtime.last_error(),
                        "allowedChatIds": s.config.telegram_bot_allowed_chat_ids.clone(),
                        "bindCode": s.config.telegram_bot_bind_code.clone(),
                        "bindCodeExpiresAt": s.config.telegram_bot_bind_code_expires_at,
                        "bindCodeExpired": s.config.telegram_bot_bind_code_expires_at.map(|expires_at| expires_at < now_ts).unwrap_or(false),
                    })))
                }
                Err(e) => Err(AppError::Unknown(e.to_string())),
            }
        }
        ("POST", "/v1/telegram/bind-code") => {
            const TELEGRAM_BIND_CODE_TTL_SECONDS: i64 = 10 * 60;
            let raw = uuid::Uuid::new_v4().simple().to_string();
            let code = format!("WR-{}", raw[..6].to_ascii_uppercase());
            let expires_at = chrono::Local::now().timestamp() + TELEGRAM_BIND_CODE_TTL_SECONDS;
            state.lock().map_err(|e| AppError::Unknown(e.to_string()))
                .and_then(|s| {
                    let mut config = s.config.clone();
                    config.telegram_bot_bind_code = Some(code.clone());
                    config.telegram_bot_bind_code_expires_at = Some(expires_at);
                    crate::commands::persist_app_config(config, app.clone(), state)
                })
                .map(|_| HttpResponse::json(200, &serde_json::json!({ "code": code, "expiresAt": expires_at })))
        }
        ("GET", "/v1/autostart") => {
            crate::autostart::is_autostart_enabled(app.clone())
                .map(|enabled| HttpResponse::json(200, &serde_json::json!(enabled)))
        }
        ("POST", "/v1/autostart/enable") => {
            let silent = request.query.get("silent").and_then(|v| v.parse().ok()).unwrap_or(false);
            crate::autostart::enable_autostart(app.clone(), silent)
                .map(|_| HttpResponse::json(200, &serde_json::json!({ "ok": true })))
        }
        ("POST", "/v1/autostart/disable") => {
            crate::autostart::disable_autostart(app.clone())
                .map(|_| HttpResponse::json(200, &serde_json::json!({ "ok": true })))
        }
        ("GET", "/v1/linux-session") => {
            #[cfg(target_os = "linux")]
            {
                let info = crate::linux_session::get_linux_session_support_info();
                Ok(HttpResponse::json(200, &info))
            }
            #[cfg(not(target_os = "linux"))]
            {
                Ok(HttpResponse::json(200, &serde_json::json!({ "supported": false })))
            }
        }
        // 非 /v1/* 路径：47831 端口上的静态资源备用托管
        // 当用户访问 http://127.0.0.1:47831/ 时也能拿到前端页面（5173 端口的备用入口）
        _ => {
            let dist_dir = resolve_web_dist_dir(&app);
            if let Some(dist) = dist_dir {
                Ok(serve_static_from_dist(&dist, &request.path))
            } else {
                Ok(HttpResponse::error(404, "未找到本地 API 路由，且未找到前端 dist 目录以托管静态资源"))
            }
        }
    };

    result.unwrap_or_else(|error| {
        let status = if matches!(error, AppError::Config(_)) {
            400
        } else {
            500
        };
        HttpResponse::error(status, error.to_string())
    })
}

fn parse_json_body<T: for<'de> Deserialize<'de>>(request: &ParsedRequest) -> Result<T> {
    serde_json::from_slice(&request.body)
        .map_err(|e| AppError::Config(format!("本地 API JSON 请求体非法: {e}")))
}

fn request_auth_mode(method: &str, path: &str) -> RequestAuthMode {
    if method == "GET" && path == "/health" {
        return RequestAuthMode::None;
    }
    if method == "POST" && path == "/feishu/event" {
        return RequestAuthMode::None;
    }
    if method == "GET" && path == "/wecom/callback" {
        return RequestAuthMode::None;
    }
    if method == "POST" && path == "/wecom/callback" {
        return RequestAuthMode::None;
    }
    if method == "POST" && path == "/dingtalk/callback" {
        return RequestAuthMode::None;
    }
    if path.starts_with("/v1/config")
        || path.starts_with("/v1/recording-state")
        || path.starts_with("/v1/platform")
        || path.starts_with("/v1/stats/")
        || path.starts_with("/v1/timeline/")
        || path.starts_with("/v1/reports")
        || path.starts_with("/v1/activity/")
        || path.starts_with("/v1/screenshot-")
        || path.starts_with("/v1/background-image")
        || path.starts_with("/v1/categories")
        || path.starts_with("/v1/apps/")
        || path.starts_with("/v1/app-icon/")
        || path.starts_with("/v1/hourly-")
        || path.starts_with("/v1/activities")
        || path.starts_with("/v1/storage")
        || path.starts_with("/v1/data-dir")
        || path.starts_with("/v1/default-data-dir")
        || path.starts_with("/v1/autostart")
        || path.starts_with("/v1/linux-session")
        || path.starts_with("/v1/localhost-api-status")
        || path.starts_with("/v1/auth/token")
        || path.starts_with("/v1/node-gateway/")
        || path.starts_with("/v1/telegram/")
        || path.starts_with("/v1/domains/")
        || path.starts_with("/v1/verify-password")
        || path.starts_with("/v1/change-password")
    {
        return RequestAuthMode::None;
    }
    RequestAuthMode::LocalApiToken
}

fn authorize_request(request: &ParsedRequest, state: &Arc<Mutex<AppState>>) -> Result<()> {
    match request_auth_mode(&request.method, &request.path) {
        RequestAuthMode::None => return Ok(()),
        RequestAuthMode::LocalApiToken => {}
    }

    let token_path = {
        let state = state.lock().map_err(|e| AppError::Unknown(e.to_string()))?;
        localhost_api_token_path(&state.data_dir)
    };
    let Some(expected_token) = read_localhost_api_token_cached(&token_path)? else {
        return Err(AppError::Config("缺少或无效的本地 API token".to_string()));
    };

    let from_header = request
        .headers
        .get("authorization")
        .and_then(|value| extract_bearer_token(value));
    let from_query = request.query.get("token").map(|s| s.as_str());

    let provided = from_header.or(from_query);

    // 常量时间比较，与飞书/钉钉/企微的签名校验保持一致的安全基线，
    // 避免理论上对本地 token 的时序侧信道。
    let matched = provided
        .map(|p| bool::from(p.as_bytes().ct_eq(expected_token.as_bytes())))
        .unwrap_or(false);
    if matched {
        Ok(())
    } else {
        Err(AppError::Config("缺少或无效的本地 API token".to_string()))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Web 端（前端页面）静态资源服务器：端口 5173，托管 Vite 构建产物 dist/ 目录
// ─────────────────────────────────────────────────────────────────────────────

fn mime_type_for(path: &str) -> &'static str {
    let lower = path.to_ascii_lowercase();
    if lower.ends_with(".html") {
        "text/html; charset=utf-8"
    } else if lower.ends_with(".js") || lower.ends_with(".mjs") {
        "application/javascript; charset=utf-8"
    } else if lower.ends_with(".css") {
        "text/css; charset=utf-8"
    } else if lower.ends_with(".json") {
        "application/json; charset=utf-8"
    } else if lower.ends_with(".svg") {
        "image/svg+xml"
    } else if lower.ends_with(".png") {
        "image/png"
    } else if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        "image/jpeg"
    } else if lower.ends_with(".gif") {
        "image/gif"
    } else if lower.ends_with(".webp") {
        "image/webp"
    } else if lower.ends_with(".ico") {
        "image/x-icon"
    } else if lower.ends_with(".flac") {
        "audio/flac"
    } else if lower.ends_with(".wasm") {
        "application/wasm"
    } else if lower.ends_with(".woff") {
        "font/woff"
    } else if lower.ends_with(".woff2") {
        "font/woff2"
    } else if lower.ends_with(".ttf") {
        "font/ttf"
    } else if lower.ends_with(".map") {
        "application/json; charset=utf-8"
    } else {
        "application/octet-stream"
    }
}

fn is_long_cache_asset(url_path: &str) -> bool {
    // Vite 构建的 /assets/* 文件都带内容哈希，可长缓存；其它路径（如 /index.html）短缓存
    url_path.starts_with("/assets/")
}

/// 解析前端 dist 目录位置，多回退策略：
/// 1) 通过 Tauri path_resolver 解析 Resource("dist")  —— 打包安装后场景
/// 2) 项目根目录 ./dist  —— 开发/调试场景（cargo build -p work-review 后在项目根）
/// 3) 可执行文件所在目录的 ../dist  —— 例如 target/debug/dist
/// 4) 当前工作目录 dist
pub fn resolve_web_dist_dir(app: &AppHandle) -> Option<PathBuf> {
    use tauri::Manager;

    // 1) Tauri 资源目录：打包安装后 resources/dist
    #[allow(deprecated)]
    if let Ok(resolver) = app.path().resource_dir() {
        let packaged = resolver.join("dist");
        if packaged.join("index.html").exists() {
            return Some(packaged);
        }
    }

    // 2) 项目根目录：当前工作目录下 ./dist（用户在项目根启动或脚本启动）
    let cwd_dist = std::env::current_dir()
        .map(|c| c.join("dist"))
        .ok()
        .filter(|p| p.join("index.html").exists());
    if let Some(p) = cwd_dist {
        return Some(p);
    }

    // 2b) cargo run/build 场景：cwd = target/debug|release  →  项目根 = cwd/../..
    if let Ok(cwd) = std::env::current_dir() {
        for up in &["..", "../..", "../../.."] {
            let candidate = cwd.join(up).join("dist");
            if candidate.join("index.html").exists() {
                return Some(candidate);
            }
        }
    }

    // 3) exe 同级 ../dist （target/debug 或 release 下）
    let exe_dist = std::env::current_exe()
        .ok()
        .and_then(|exe| {
            // exe.parent()/../dist
            exe.parent()
                .map(|p| p.parent().map(|pp| pp.join("dist")).unwrap_or_else(|| p.join("dist")))
        })
        .filter(|p| p.join("index.html").exists());
    if let Some(p) = exe_dist {
        return Some(p);
    }

    // 3b) 打包/开发目录层级更深：基于 exe 向上 2~3 级找 dist
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            for up in &["..", "../..", "../../.."] {
                let candidate = exe_dir.join(up).join("dist");
                if candidate.join("index.html").exists() {
                    return Some(candidate);
                }
            }
        }
    }

    // 4) exe 同级 dist
    let exe_sibling = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
        .map(|p| p.join("dist"))
        .filter(|p| p.join("index.html").exists());
    if let Some(p) = exe_sibling {
        return Some(p);
    }

    None
}

fn sanitize_url_path(url_path: &str) -> String {
    // 剥离查询与锚点，防止路径穿越
    let cleaned = url_path
        .split('?')
        .next()
        .unwrap_or(url_path)
        .split('#')
        .next()
        .unwrap_or(url_path)
        .replace('\\', "/");
    // 去除开头连续 /
    let trimmed = cleaned.trim_start_matches('/');
    // 阻止 .. 目录穿越：任何包含 ../ 或 == ".." 的都重定向回根
    if trimmed.contains("..") {
        return String::new();
    }
    trimmed.to_string()
}

pub fn serve_static_from_dist(dist_dir: &Path, url_path: &str) -> HttpResponse {
    let rel = sanitize_url_path(url_path);
    let file_path = if rel.is_empty() {
        dist_dir.join("index.html")
    } else {
        let candidate = dist_dir.join(&rel);
        if candidate.exists() && candidate.is_file() {
            candidate
        } else {
            // SPA fallback：任何 /xxx 子路由、404 都回退 index.html（交给前端路由）
            dist_dir.join("index.html")
        }
    };

    match std::fs::read(&file_path) {
        Ok(bytes) => {
            let ct = mime_type_for(file_path.to_string_lossy().as_ref());
            let cache = is_long_cache_asset(url_path);
            HttpResponse::static_bytes(ct, bytes, cache)
        }
        Err(e) => {
            // index.html 也读不到：返回明确错误
            log::warn!("读取前端静态资源失败 {:?}: {e}", file_path);
            HttpResponse::error(
                503,
                format!(
                    "前端静态资源未就绪：未找到 dist/index.html，请确认已执行 `npm run build`。({e})"
                ),
            )
        }
    }
}

fn stop_web_static_locked(runtime: &mut WebStaticRuntime) -> Option<oneshot::Sender<()>> {
    runtime.running = false;
    runtime.bound_port = None;
    runtime.shutdown_tx.take()
}

fn record_web_static_error(state: &Arc<Mutex<AppState>>, message: &str) {
    if let Ok(mut s) = state.lock() {
        s.web_static_runtime.last_error = Some(message.to_string());
    }
    log::warn!("{}", message);
}

/// 判断 5173 端口收到的请求是否属于「本地 API 反向代理」路径。
/// 匹配成功时直接转发到 127.0.0.1:47831，由真实 API 服务处理；
/// 否则交给静态资源托管逻辑（含 SPA index.html 回退）。
fn is_api_proxy_path(path: &str) -> bool {
    path == "/health"
        || path == "/metrics"
        || path.starts_with("/v1/")
        || path.starts_with("/generate-report")
        || path.starts_with("/wecom/")
        || path.starts_with("/dingtalk/")
        || path.starts_with("/files/")
}

/// 把 ParsedRequest 重新序列化为可直接写入 TCP 的 HTTP/1.1 报文字节，
/// 用于把 5173 收到的 API 请求转发到 47831（重写 Host 头，追加转发头）。
fn build_proxy_request_bytes(req: &ParsedRequest) -> Vec<u8> {
    let mut out = Vec::with_capacity(1024 + req.body.len());
    // 请求行：使用保存的 raw_target（保留完整查询串与原始编码）
    out.extend_from_slice(
        format!("{} {} HTTP/1.1\r\n", req.method, req.raw_target).as_bytes(),
    );
    // 必写 Host：指向真实 API（本地回环）
    out.extend_from_slice(
        format!("Host: {}:{}\r\n", LOCALHOST_API_HOST, DEFAULT_LOCALHOST_API_PORT).as_bytes(),
    );
    // 转发溯源：方便后端识别来自同源代理的请求
    out.extend_from_slice(b"X-Forwarded-Proto: http\r\n");
    out.extend_from_slice(b"X-Forwarded-Host: localhost:5173\r\n");
    out.extend_from_slice(b"X-Forwarded-For: 127.0.0.1\r\n");

    const HOP_BY_HOP: &[&str] = &[
        "host",
        "connection",
        "keep-alive",
        "proxy-authorization",
        "proxy-authenticate",
        "te",
        "trailer",
        "transfer-encoding",
        "upgrade",
    ];

    for (k, v) in &req.headers {
        let kl = k.to_ascii_lowercase();
        if HOP_BY_HOP.iter().any(|h| *h == kl) {
            continue;
        }
        out.extend_from_slice(format!("{k}: {v}\r\n").as_bytes());
    }

    // 有 body 时重写 Content-Length；没 body 不强制，避免 GET 多一个无意义头
    if !req.body.is_empty() {
        out.extend_from_slice(format!("Content-Length: {}\r\n", req.body.len()).as_bytes());
    }
    // 强制 Connection: close，简化读取逻辑（读到 EOF 即算读完）
    out.extend_from_slice(b"Connection: close\r\n");
    out.extend_from_slice(b"\r\n");
    if !req.body.is_empty() {
        out.extend_from_slice(&req.body);
    }
    out
}

/// 向 127.0.0.1:47831 转发一个 API 请求并读取完整响应字节。
/// 因为向对端发送了 `Connection: close`，读取到 EOF 即视为响应结束。
async fn proxy_to_local_api(req: &ParsedRequest) -> std::io::Result<Vec<u8>> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let target = format!("{}:{}", LOCALHOST_API_HOST, DEFAULT_LOCALHOST_API_PORT);
    let mut upstream = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        tokio::net::TcpStream::connect(&target),
    )
    .await
    .map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            "连接本地 API 服务超时（47831）",
        )
    })??;

    let payload = build_proxy_request_bytes(req);
    upstream.write_all(&payload).await?;
    upstream.flush().await?;

    let mut buf = Vec::with_capacity(8192);
    let mut tmp = [0u8; 16384];
    loop {
        let n = tokio::time::timeout(
            std::time::Duration::from_secs(60),
            upstream.read(&mut tmp),
        )
        .await
        .map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "读取本地 API 响应超时",
            )
        })??;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&tmp[..n]);
    }
    Ok(buf)
}

/// 启动/同步 5173 端口的前端静态资源服务器。由 sync_localhost_api_runtime 调用。
pub fn sync_web_static_server(app: &AppHandle, state: &Arc<Mutex<AppState>>) -> Result<()> {
    let (need_start, shutdown_tx, dist_dir) = {
        let mut state = state.lock().map_err(|e| AppError::Unknown(e.to_string()))?;
        // 先确保本地 API 已启用（若用户禁用则也不启动前端服务，保持一致）
        let enabled = state.config.localhost_api_enabled;
        if !enabled {
            state.web_static_runtime.last_error = None;
            let tx = stop_web_static_locked(&mut state.web_static_runtime);
            if let Some(tx) = tx {
                let _ = tx.send(());
            }
            return Ok(());
        }

        let dist = resolve_web_dist_dir(app);
        if dist.is_none() {
            // 没找到 dist，不报错但记录：可能用户首次启动且未运行过 npm run build
            state.web_static_runtime.resolved_dist_dir = None;
            state.web_static_runtime.last_error = Some(
                "未找到前端 dist 目录，请先执行 `npm run build` 构建前端。".to_string(),
            );
            log::warn!(
                "Web 端（:5173）跳过启动：未找到 dist/index.html。请先 `npm run build` 构建前端。"
            );
            return Ok(());
        }
        let dist = dist.unwrap();

        if state.web_static_runtime.running
            && state.web_static_runtime.bound_port == Some(WEB_STATIC_PORT)
            && state.web_static_runtime.resolved_dist_dir.as_ref() == Some(&dist)
        {
            return Ok(());
        }

        let shutdown_tx = stop_web_static_locked(&mut state.web_static_runtime);
        state.web_static_runtime.resolved_dist_dir = Some(dist.clone());
        (true, shutdown_tx, dist)
    };

    if let Some(tx) = shutdown_tx {
        let _ = tx.send(());
    }
    if !need_start {
        return Ok(());
    }

    let std_listener = match bind_localhost_api_listener(WEB_STATIC_HOST, WEB_STATIC_PORT) {
        Ok(l) => l,
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
            // 5173 很可能已经被用户手动起的 vite dev server 占用了，这属于正常场景，不报错
            let msg = format!(
                "端口 5173 已被占用（可能正在运行 Vite dev server），跳过 Web 静态服务器启动。前端页面由该服务提供。"
            );
            record_web_static_error(state, &msg);
            log::info!("{msg}");
            return Ok(());
        }
        Err(e) => {
            return Err(AppError::Config(format!(
                "绑定 Web 静态服务器端口 {WEB_STATIC_PORT} 失败: {e}"
            )));
        }
    };
    std_listener.set_nonblocking(true)?;
    let listener = TcpListener::from_std(std_listener)
        .map_err(|e| AppError::Unknown(format!("接管 Web 静态服务器监听器失败: {e}")))?;

    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    {
        let mut s = state.lock().map_err(|e| AppError::Unknown(e.to_string()))?;
        s.web_static_runtime.running = true;
        s.web_static_runtime.bound_port = Some(WEB_STATIC_PORT);
        s.web_static_runtime.last_error = None;
        s.web_static_runtime.shutdown_tx = Some(shutdown_tx);
    }

    let state_handle = state.clone();
    let dist_dir_cloned = dist_dir.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = run_web_static_server(listener, shutdown_rx, dist_dir_cloned).await {
            record_web_static_error(&state_handle, &format!("Web 静态服务器异常退出: {e}"));
        } else if let Ok(mut s) = state_handle.lock() {
            s.web_static_runtime.running = false;
            s.web_static_runtime.bound_port = None;
            s.web_static_runtime.shutdown_tx = None;
        }
    });

    log::info!(
        "Web 端静态资源服务器已监听在 http://{}:{}，dist={}",
        WEB_STATIC_HOST,
        WEB_STATIC_PORT,
        dist_dir.display()
    );
    Ok(())
}

async fn run_web_static_server(
    listener: TcpListener,
    mut shutdown_rx: oneshot::Receiver<()>,
    dist_dir: PathBuf,
) -> Result<()> {
    loop {
        tokio::select! {
            _ = &mut shutdown_rx => {
                return Ok(());
            }
            accept_result = listener.accept() => {
                let (stream, _) = accept_result
                    .map_err(|e| AppError::Unknown(format!("接受 Web 静态连接失败: {e}")))?;
                let dist = dist_dir.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(e) = handle_web_static_connection(stream, dist).await {
                        log::debug!("处理 Web 静态请求失败: {e}");
                    }
                });
            }
        }
    }
}

async fn handle_web_static_connection(mut stream: TcpStream, dist_dir: PathBuf) -> Result<()> {
    use tokio::io::AsyncWriteExt;

    match read_request(&mut stream).await {
        Ok(Some(request)) => {
            // OPTIONS 预检：与 47831 的 to_bytes 里 CORS 保持一致
            if request.method == "OPTIONS" {
                let cors_ok = b"HTTP/1.1 204 No Content\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, PUT, DELETE, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type, Authorization, X-Localhost-Api-Token\r\nAccess-Control-Max-Age: 86400\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
                stream.write_all(cors_ok).await?;
                stream.shutdown().await?;
                return Ok(());
            }

            if is_api_proxy_path(&request.path) {
                // 【反向代理分支】API 请求转发到 127.0.0.1:47831，原始响应字节直接写回
                // 这样同源（localhost:5173）页面发起的 /v1/xxx、/health 请求能正常拿到业务数据。
                match proxy_to_local_api(&request).await {
                    Ok(raw) => {
                        stream.write_all(&raw).await?;
                    }
                    Err(e) => {
                        let err = HttpResponse::error(
                            502,
                            format!("反向代理本地 API 失败: {e}"),
                        );
                        stream.write_all(&err.to_bytes()).await?;
                    }
                }
            } else {
                // 【静态分支】前端静态资源或 SPA 回退
                let resp = serve_static_from_dist(&dist_dir, &request.path);
                stream.write_all(&resp.to_bytes()).await?;
            }
        }
        Ok(None) => return Ok(()),
        Err(err) => {
            let resp = HttpResponse::error(400, err.to_string());
            stream.write_all(&resp.to_bytes()).await?;
        }
    }

    stream.shutdown().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        acquire_api_capture_permit, build_capture_payload, effective_api_host,
        ensure_api_capture_enabled, extract_bearer_token, mask_localhost_api_token,
        parse_generate_report_params, request_auth_mode, ParsedRequest, RequestAuthMode,
    };
    use crate::screenshot::ScreenshotResult;
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};

    fn parsed_request(
        method: &str,
        query: &[(&str, &str)],
        body: serde_json::Value,
    ) -> ParsedRequest {
        ParsedRequest {
            method: method.to_string(),
            path: "/v1/reports/generate".to_string(),
            query: query
                .iter()
                .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
                .collect(),
            headers: HashMap::new(),
            body: serde_json::to_vec(&body).expect("请求体应可序列化"),
        }
    }

    #[test]
    fn 即时截图响应应返回相对路径和可直接使用的图像数据() {
        let payload = build_capture_payload(
            Path::new("/tmp/work-review-data"),
            ScreenshotResult {
                path: PathBuf::from("/tmp/work-review-data/screenshots/2026-08-10/shot.jpg"),
                ocr_source_path: None,
                timestamp: 1_786_334_400,
                width: 1440,
                height: 900,
            },
            "encoded-jpeg".to_string(),
        );
        let json = serde_json::to_value(payload).expect("截图响应应可序列化");

        assert_eq!(json["capturedAt"], 1_786_334_400);
        assert_eq!(json["width"], 1440);
        assert_eq!(json["height"], 900);
        assert_eq!(json["mimeType"], "image/jpeg");
        assert_eq!(json["imageBase64"], "encoded-jpeg");
        assert_eq!(json["relativePath"], "screenshots/2026-08-10/shot.jpg");
        assert!(json.get("path").is_none(), "不得暴露绝对文件路径");
    }

    #[test]
    fn 即时截图路径异常时也不得暴露绝对路径() {
        let payload = build_capture_payload(
            Path::new("/tmp/work-review-data"),
            ScreenshotResult {
                path: PathBuf::from("/private/tmp/unexpected-shot.jpg"),
                ocr_source_path: None,
                timestamp: 1_786_334_400,
                width: 1440,
                height: 900,
            },
            "encoded-jpeg".to_string(),
        );
        let json = serde_json::to_value(payload).expect("截图响应应可序列化");

        assert_eq!(json["relativePath"], "unexpected-shot.jpg");
        assert!(!json["relativePath"]
            .as_str()
            .unwrap_or_default()
            .starts_with('/'));
    }

    #[test]
    fn 关闭截图功能时即时截图api不应绕过隐私开关() {
        let error = ensure_api_capture_enabled(false).expect_err("关闭截图时必须拒绝请求");
        assert!(error.to_string().contains("截图与 OCR"));
        assert!(ensure_api_capture_enabled(true).is_ok());
    }

    #[test]
    fn 即时截图应限制为单个并发任务并在结束后释放许可() {
        let permit = acquire_api_capture_permit().expect("首个截图任务应取得许可");
        let error = acquire_api_capture_permit().expect_err("并发截图任务必须被拒绝");
        assert!(error.to_string().contains("正在进行"));

        drop(permit);
        assert!(acquire_api_capture_permit().is_ok(), "任务结束后应释放许可");
    }

    #[test]
    fn post日报生成参数应从json请求体解析() {
        let request = parsed_request(
            "POST",
            &[],
            serde_json::json!({ "date": "2026-07-21", "force": true, "locale": "zh-CN" }),
        );

        assert_eq!(
            parse_generate_report_params(&request).expect("POST 参数应合法"),
            (
                "2026-07-21".to_string(),
                Some(true),
                Some("zh-CN".to_string())
            )
        );
    }

    #[test]
    fn post日报生成参数可省略force和locale() {
        let request = parsed_request("POST", &[], serde_json::json!({ "date": "2026-07-21" }));

        assert_eq!(
            parse_generate_report_params(&request).expect("可选参数应允许省略"),
            ("2026-07-21".to_string(), None, None)
        );
    }

    #[test]
    fn get日报生成参数应继续从query解析() {
        let request = parsed_request(
            "GET",
            &[("date", "2026-07-21"), ("force", "false"), ("locale", "en")],
            serde_json::Value::Null,
        );

        assert_eq!(
            parse_generate_report_params(&request).expect("GET 参数应保持兼容"),
            (
                "2026-07-21".to_string(),
                Some(false),
                Some("en".to_string())
            )
        );
    }

    #[test]
    fn 日报生成参数缺少日期时应返回配置错误() {
        let request = parsed_request("POST", &[], serde_json::json!({ "force": true }));
        let error = parse_generate_report_params(&request).expect_err("缺少日期必须失败");
        assert!(error.to_string().contains("date 参数不能为空"));
    }

    #[test]
    fn 监听地址应允许局域网与全部网卡绑定() {
        assert_eq!(effective_api_host(None), "127.0.0.1");
        assert_eq!(effective_api_host(Some(" 0.0.0.0 ")), "0.0.0.0");
        assert_eq!(effective_api_host(Some("192.168.1.23")), "192.168.1.23");
        assert_eq!(effective_api_host(Some("::")), "::");
    }

    #[test]
    fn bearer_token解析应忽略前后空白() {
        assert_eq!(extract_bearer_token("Bearer abc123 "), Some("abc123"));
        assert_eq!(extract_bearer_token("bearer xyz"), Some("xyz"));
        assert_eq!(extract_bearer_token("Basic nope"), None);
    }

    #[test]
    fn token预览应避免泄露完整密钥() {
        let masked = mask_localhost_api_token("wr-local-1234567890abcdef");
        assert!(masked.starts_with("wr-local"));
        assert!(masked.contains('…'));
        assert!(!masked.contains("1234567890abcdef"));
    }

    #[test]
    fn 鉴权模式应将健康检查标记为免鉴权() {
        assert_eq!(request_auth_mode("GET", "/health"), RequestAuthMode::None);
    }

    #[test]
    fn 鉴权模式应将其余路由标记为本地api_token鉴权() {
        assert_eq!(
            request_auth_mode("GET", "/v1/device"),
            RequestAuthMode::LocalApiToken
        );
    }

    #[test]
    fn 周报路由应被纳入本地api_token鉴权() {
        // /v1/weekly-review 不在 None 白名单里，应当回落到默认的 LocalApiToken。
        assert_eq!(
            request_auth_mode("GET", "/v1/weekly-review"),
            RequestAuthMode::LocalApiToken
        );
    }

    #[test]
    fn 时间线路由各种日期形态都应触发鉴权() {
        // 防止某些日期格式误命中 None 白名单。
        // 当前免鉴权白名单：/health、/feishu/event、/wecom/callback (GET+POST)、/dingtalk/callback。
        for path in [
            "/v1/timeline/2026-05-08",
            "/v1/timeline/2026-01-01",
            "/v1/timeline/", // 空日期：路由处理器会自行返回 400，但鉴权仍应触发
        ] {
            assert_eq!(
                request_auth_mode("GET", path),
                RequestAuthMode::LocalApiToken,
                "路径 {path} 应需要本地 API token"
            );
        }
    }

    #[test]
    fn 健康检查与机器人回调外的所有方法都应被鉴权() {
        // 巩固现有契约：白名单是 {GET /health, POST /feishu/event,
        // GET/POST /wecom/callback, POST /dingtalk/callback}，其它一切 = LocalApiToken。
        assert_eq!(
            request_auth_mode("POST", "/v1/weekly-review"),
            RequestAuthMode::LocalApiToken
        );
        assert_eq!(
            request_auth_mode("DELETE", "/v1/timeline/2026-05-08"),
            RequestAuthMode::LocalApiToken
        );
        assert_eq!(
            request_auth_mode("GET", "/v1/anything-new-we-add-later"),
            RequestAuthMode::LocalApiToken
        );
    }

    #[test]
    fn 机器人回调路由应在免鉴权白名单内() {
        // 三个 webhook 路由本身依赖各自的签名校验，不走 localhost token。
        assert_eq!(
            request_auth_mode("GET", "/wecom/callback"),
            RequestAuthMode::None
        );
        assert_eq!(
            request_auth_mode("POST", "/wecom/callback"),
            RequestAuthMode::None
        );
        assert_eq!(
            request_auth_mode("POST", "/dingtalk/callback"),
            RequestAuthMode::None
        );
        assert_eq!(
            request_auth_mode("POST", "/feishu/event"),
            RequestAuthMode::None
        );
        // 错误的方法仍要鉴权（防止误开放）
        assert_eq!(
            request_auth_mode("DELETE", "/wecom/callback"),
            RequestAuthMode::LocalApiToken
        );
        assert_eq!(
            request_auth_mode("GET", "/dingtalk/callback"),
            RequestAuthMode::LocalApiToken
        );
    }
}