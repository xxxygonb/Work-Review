# AI 修改记录

## 2026-08-20

### 7. 深度优化浏览器URL记录（第三轮）

**问题**：第二轮优化后Edge浏览器URL仍然无法记录。窗口标题如 "哔哩哔哩 (゜-゜)つロ 干杯~-bilibili 和另外 1 个页面 - 个人 - Microsoft​ Edge" 无法匹配到History DB中的URL。

**根因分析**：
1. **窗口标题包含"和另外 N 个页面"后缀**：Edge在多标签页时会在标题中添加此后缀，但History DB中的页面标题不包含，导致LIKE匹配失败
2. **标题关键词提取策略单一**：只尝试一个关键词，如果匹配失败就直接放弃
3. **没有"最近URL"兜底**：标题匹配全部失败时，没有查询最近访问URL的兜底方案
4. **主循环中浏览器URL只查一次**：如果第一次查询失败（UIA超时/缓存未命中），不会重试

**优化内容**：

1. **增强标题关键词提取**（`extract_title_keywords`）：
   - 移除"和另外 N 个页面" / "and N more tabs"后缀
   - 移除零宽空格等不可见字符
   - 生成多个候选关键词：完整标题、前20字符、标题+第二段组合
   - 支持中英文Edge标题格式

2. **多关键词匹配**：对每个History文件尝试所有候选关键词，提高匹配率

3. **最近URL兜底**（`read_latest_url_from_chromium_history`）：当所有关键词匹配失败时，查询最近访问的URL作为兜底

4. **主循环二次探测**：在预探测和Sticky恢复之后，如果浏览器URL仍然为None，强制调用`resolve_browser_url_for_window`进行二次探测

5. **窗口标题清理增强**（`clean_browser_window_title`）：移除"和另外 N 个页面" / "and N more tabs"后缀，使标题更干净

6. **增加详细日志**：History DB查询增加命中/复制失败等日志

#### 修改文件
- `src-tauri/src/monitor.rs` — 增强标题关键词提取、多关键词匹配、最近URL兜底、窗口标题清理
- `src-tauri/src/main.rs` — 增加浏览器URL二次探测

## 2026-08-20

### 6. 深度优化浏览器URL记录（第二轮）

**问题**：第一轮优化后浏览器URL仍然无法记录。时间线显示窗口标题正确（如 "Try NVIDIA NIM APIs - 个人 - Microsoft Edge"），但browser_url字段始终为空。

**根因分析**：
1. **Firefox Session Store方案在Windows上完全禁用**：`firefox_family_session_store_base_dir` 在非macOS/Linux平台直接返回None，导致Firefox的URL永远无法通过session store获取
2. **Chrome/Edge没有History数据库读取方案**：UIA查询在Chrome/Edge上经常失败（地址栏不是标准Edit控件），且没有备选方案
3. **CDP方案依赖外部条件**：需要浏览器以`--remote-debugging-port`启动，普通用户不会这样启动
4. **EnumChildWindows + WM_GETTEXT方案对Chrome无效**：Chrome使用自定义渲染引擎，地址栏不是标准Windows控件

**优化内容**：

1. **Firefox Session Store方案支持Windows**：
   - 修改`firefox_family_session_store_base_dir`，添加Windows路径（`%APPDATA%/Mozilla/Firefox`）
   - 将`firefox_family_session_store_url`及相关辅助函数的cfg属性扩展到Windows
   - 将`serde_json::Value`、`Path/PathBuf`等类型导入扩展到Windows

2. **新增Chromium History数据库读取方案**：
   - 实现`chromium_history_latest_url`：通过读取Chrome/Edge的History SQLite数据库获取最近访问的URL
   - 实现`find_all_chromium_history_paths`：自动发现Default和Profile*目录下的History文件
   - 实现`read_url_from_chromium_history`：复制History到临时目录（避免锁文件冲突），用rusqlite读取
   - 实现`extract_title_keyword`：从窗口标题提取关键词用于SQL LIKE查询
   - 支持Chrome、Edge、Brave、Vivaldi、Chromium的History路径

3. **新增EnumChildWindows + WM_GETTEXT兜底方案**：
   - 实现`get_url_via_enum_child_windows`：枚举浏览器窗口的子控件，通过WM_GETTEXT获取文本
   - 评分逻辑：omnibox > address > toolbar > edit

4. **新增CDP调试端口自动扫描**：
   - 实现`scan_common_debug_ports`：自动扫描9222-9235、9300-9305、9515等常见调试端口
   - 通过`/json/version`端点验证端口是否为浏览器调试服务

5. **增加详细调试日志**：
   - UIA查询中增加控件类型、AutomationId、ClassName、候选值来源等详细日志
   - URL查询流程中增加CDP/UIA/EnumChildWindows/PowerShell每个步骤的命中/未命中日志

6. **调整URL获取优先级**：
   ```
   Firefox浏览器: Session Store → UIA → EnumChildWindows → PowerShell → 标题推断
   Chrome/Edge:   History DB → CDP → UIA → EnumChildWindows → PowerShell → 标题推断
   ```

#### 修改文件
- `src-tauri/src/monitor.rs` — 核心优化：Firefox Session Store Windows支持、Chromium History DB读取、EnumChildWindows兜底、CDP端口扫描、调试日志
- `crates/core/src/categorize.rs` — 增强窗口标题URL推断（上一轮已修改）

## 2026-08-20

### 5. 优化浏览器网站URL记录准确性

1. **增加CDP（Chrome DevTools Protocol）URL获取方式**：当浏览器以 `--remote-debugging-port` 启动时，通过CDP接口直接获取当前标签页URL，这是最准确的URL获取方式。通过窗口标题匹配确定当前活动标签页。

2. **优化UIA地址栏识别**：
   - 增加 AutomationId 检查（`omnibox`、`address`、`urlbar`、`location`），Chrome地址栏的AutomationId通常为"omnibox"
   - 增加中文地址栏名称识别（"搜索或输入网址"、"地址"）
   - 增加 `toolbar` class name 匹配
   - 增加优先扫描策略：先快速扫描带地址栏AutomationId的Edit控件，命中高置信度结果立即返回
   - Edit控件基础分从35提升到40，有AutomationId且为地址栏的额外+10分
   - 提前返回阈值从85分降低到80分

3. **优化UIA查询超时**：
   - UIA查询等待超时从350ms增加到800ms，给复杂页面更多扫描时间
   - PowerShell回退预算从200ms增加到500ms
   - 控件扫描超时从300ms增加到500ms

4. **优化缓存策略**：
   - 成功缓存TTL从3秒增加到5秒，提高标签页切换时的缓存命中率
   - 失败缓存TTL从2秒增加到3秒
   - 慢查询阈值从1秒增加到2秒
   - 熔断冷却期从20秒减少到15秒

5. **增强PowerShell UIA兜底脚本**：
   - 增加评分逻辑，不再返回第一个匹配的URL，而是选择评分最高的
   - 增加 AutomationId、ClassName 检查和地址栏识别
   - 增加中文地址栏名称识别

6. **增强窗口标题URL推断**：
   - 增加 `—`（em dash）分隔符支持
   - 增加 `extract_domain_like_from_title` 函数，从标题中提取常见域名模式（.com、.cn、.net、.org、.io、.dev、.app等）
   - 对中文域名后缀（.com.cn、.org.cn、.net.cn、.gov.cn、.edu.cn）提供专门支持

#### 修改文件
- `src-tauri/src/monitor.rs` — 核心优化：增加CDP获取方式、优化UIA评分和识别逻辑、调整超时和缓存参数、增强PowerShell脚本
- `crates/core/src/categorize.rs` — 增强 `extract_url_from_title` 和 `infer_browser_page_hint`，新增 `extract_domain_like_from_title` 函数

## 2026-08-20

### 4. 添加登录页与密码保护

**功能**：
1. 每次打开桌面端窗口或 web 端网页都需要输入密码才能进入主页面
2. 在设置 > 常规 > 系统行为区域添加"修改密码"按钮，点击弹出对话框可修改密码
3. 默认密码为 `Admin123`，首次启动可使用该密码进入

#### 修改文件
- `crates/core/src/config.rs` — `AppConfig` 结构体新增 `app_password: String` 字段（默认 `"Admin123"`）
- `src-tauri/src/commands/config.rs` — 新增 `verify_password` 和 `change_password` 两个 Tauri 命令
- `src-tauri/src/main.rs` — 注册 `verify_password` 和 `change_password` 命令
- `src-tauri/src/localhost_api.rs` — 新增 `/v1/verify-password` 和 `/v1/change-password` API 端点，加入鉴权白名单
- `src/lib/utils/safeInvoke.ts` — 添加 `verify_password` 和 `change_password` 的命令映射与返回值解包
- `src/lib/components/LoginPage.svelte` — 新建登录页组件，含密码输入、验证、错误提示
- `src/App.svelte` — 添加 `isAuthenticated` 状态，未认证时显示登录页，认证后显示主界面
- `src/routes/settings/components/SettingsGeneral.svelte` — 系统行为区域添加"修改密码"按钮与弹窗
- `src/lib/i18n/locales/zh-CN.ts` — 新增 `login.*` 和 `settingsGeneral.changePassword*` 中文文本

## 2026-08-18

### 3. 设置>常规 添加"隐藏托盘图标"开关

**功能**：在设置 > 常规 > 系统行为区域添加"隐藏托盘图标"开关，开启后隐藏系统托盘图标。

#### 修改文件
- `crates/core/src/config.rs` — `AppConfig` 结构体新增 `hide_tray_icon: bool` 字段（默认 `false`）
- `src-tauri/src/main.rs` —
  - `TrayMenuState` 新增 `tray: TrayIcon` 字段，存储 tray 引用以便后续调用 `set_visible`
  - 启动时根据 `config.hide_tray_icon` 初始值设置托盘可见性
  - 调整 `app.manage(TrayMenuState)` 位置，在 tray build 之后执行
- `src-tauri/src/commands/shared.rs` — `persist_app_config` 中添加托盘图标显隐同步：配置保存后根据 `hide_tray_icon` 调用 `tray.set_visible`
- `src/routes/settings/components/SettingsGeneral.svelte` —
  - `GeneralConfig` 接口新增 `hide_tray_icon: boolean`
  - 系统行为区域新增"隐藏托盘图标"开关（样式与其它开关一致）
- `src/lib/i18n/locales/zh-CN.ts` — 新增 `hideTrayIcon` / `hideTrayIconDescription` 中文文本

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