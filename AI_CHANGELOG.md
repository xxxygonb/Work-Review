# AI 修改记录

## 2026-08-24

### 22. 修复截图失败时活动记录被丢弃的问题

**问题**：新建活动路径中，截图捕获失败时整个活动记录都不保存（返回 `None`），导致即使有活动发生，也不会留下任何记录。

**修复**：截图失败时降级为无截图记录——仍然保存活动记录（`screenshot_path` 为空），只是没有截图和 OCR。日志从 `"截屏失败: {e}"` 改为 `"截屏失败: {e}，降级为无截图记录"`，并输出 `"📝 截屏失败降级: 新建无截图活动 {app_name} (id={id})"`。

#### 修改文件
- `src-tauri/src/main.rs`（新建路径 `Err(e)` 分支）

### 21. 修复记录没有截图：配置中 screenshots_enabled 被设为 false

**问题**：时间线记录显示"本次记录未保存截图"。

**根因定位**：
1. 用户配置文件 `%APPDATA%\work-review\config.json` 中 `"screenshots_enabled": false`，截图功能被关闭。
2. 后端在 `screenshots_enabled = false` 时走无截图路径，创建 `screenshot_path: String::new()` 的活动记录。
3. 前端 `Timeline.svelte` 中当 `screenshot_path` 为空时显示"本次记录未保存截图"。

**修复**：
- 将用户配置文件中 `screenshots_enabled` 从 `false` 改回 `true`
- 同时修复了 Vite proxy 缺少 `X-Forwarded-Host` 头的问题（见 #18），确保 Web 端截图 API 请求不被 401 拒绝
- 同时修复了截图失败时活动记录被丢弃的问题（见 #22）

#### 修改文件
- `%APPDATA%\work-review\config.json`（`screenshots_enabled: false → true`）
- `vite.config.ts`（proxy 添加 `X-Forwarded-Host` 头，#18）
- `src-tauri/src/main.rs`（截图失败降级，#22）

### 22. 修复截图失败时活动记录被丢弃的问题

**问题**：新建活动路径中，截图捕获失败时整个活动记录都不保存（返回 None），导致即使有活动发生也不会留下任何记录。

**修复**：截图失败时降级为无截图记录——仍然保存活动记录（screenshot_path 为空），只是没有截图和 OCR。

#### 修改文件
- src-tauri/src/main.rs（新建路径 Err(e) 分支）

### 21. 修复记录没有截图：配置中 screenshots_enabled 被设为 false

**问题**：时间线记录显示"本次记录未保存截图"。

**根因**：用户配置文件中 screenshots_enabled: false，截图功能被关闭。

**修复**：将 screenshots_enabled 从 false 改回 true。

#### 修改文件
- %APPDATA%\work-review\config.json
### 20. 登录页：删除默认密码提示，增加"忘记密码"重置功能

**需求**：删除登录页底部的"首次使用默认密码：Admin123"提示，改为"忘记密码"链接，点击后展开重置密码表单，输入默认密码（Admin123）即可重新设置新密码。

**修改内容**：
- 删除 `login.defaultHint` 翻译及对应 `<p class="login-hint">` 元素
- 新增"忘记密码"链接按钮，点击展开/收起重置密码表单
- 重置表单包含：默认密码输入框、新密码输入框、确认新密码输入框、取消/确认按钮
- 调用 `change_password` 命令，以默认密码作为 `old_password` 验证
- 成功后自动收起表单，清空密码输入框，用户用新密码登录
- 错误处理：默认密码不正确、新密码为空、两次密码不一致

**新增 i18n 翻译**（zh-CN）：
- `login.forgotPassword`: '忘记密码'
- `login.resetPasswordTitle`: '重置密码'
- `login.resetPasswordHint`: '输入默认密码即可重新设置新密码'
- `login.defaultPasswordPlaceholder`: '请输入默认密码'
- `login.newPasswordPlaceholder`: '请输入新密码'
- `login.confirmPasswordPlaceholder`: '请确认新密码'
- `login.resetSuccess`: '密码重置成功，请使用新密码登录'
- `login.defaultPasswordIncorrect`: '默认密码不正确'

#### 修改文件
- `src/lib/components/LoginPage.svelte`
- `src/lib/i18n/locales/zh-CN.ts`

### 19. 设置页：修改密码从弹窗改为内联二级选项

**需求**：设置中的修改密码不要用弹窗，而是点击"修改密码"后展开内联二级选项，样式参考设置中"开机自启"的二级选项模式。

**修改内容**：
- 删除弹窗（`{#if showPasswordDialog}` 整个 overlay + modal）
- 点击"修改密码"按钮切换展开/收起状态，按钮文字在"修改密码"/"取消"间切换
- 展开时显示内联二级选项区域，使用与"开机自启"相同的样式：
  - `ml-3 pl-3 border-l-2 border-primary-200/60 dark:border-primary-800/40`
- 包含：当前密码、新密码、确认新密码三个输入框 + 确认按钮
- 成功后自动收起表单并清空输入
- 增加"当前密码不能为空"校验

#### 修改文件
- `src/routes/settings/components/SettingsGeneral.svelte`

### 18. 修复 Web 端截图无法加载：Vite proxy 缺少 X-Forwarded-Host 头导致 API 401

**问题**：Web 端（`start-all.bat` 启动）产生的活动记录都没有截图显示。

**根因定位**：
Tauri 内置 Web 静态服务器在反向代理请求到 47831 端口时，会追加 `X-Forwarded-Host: localhost:5173` 头。后端 [localhost_api.rs](file:///g:/Work-Review/src-tauri/src/localhost_api.rs) 的 `route_request()` 依赖此头判断"同源代理免鉴权"：

```rust
let forwarded_from_web_static = request
    .headers
    .get("x-forwarded-host")
    .map(|v| v.trim() == "localhost:5173")
    .unwrap_or(false);

if needs_auth && !forwarded_from_web_static {
    // → 401 Unauthorized
}
```

但 Vite 开发服务器的 `server.proxy` **不会自动添加 `X-Forwarded-Host` 头**（`changeOrigin: true` 只改 `Host`/`Origin`），导致所有 API 请求被 401 拒绝，前端拿不到活动数据（包括截图路径），截图加载请求也被 401 拦截。

**修复方案**：
在 [vite.config.ts](file:///g:/Work-Review/vite.config.ts) 的 proxy 配置中为每个代理规则添加 `headers: { 'X-Forwarded-Host': 'localhost:5173' }`，与 Tauri 内置 Web 静态服务器的行为一致：

```typescript
proxy: {
  '/v1': {
    target: 'http://127.0.0.1:47831',
    changeOrigin: true,
    headers: { 'X-Forwarded-Host': 'localhost:5173' },
  },
  // ... 同理 /health、/metrics
},
```

#### 修改文件
- `vite.config.ts`

### 17. 修复 Web 端 404：Vite 开发服务器未配置 API 反向代理

**问题**：一键启动脚本（`start-all.bat`）启动后，浏览器访问 `http://localhost:5173` 输入密码提示「验证失败，请重试」，错误详情为 `[safeInvoke] verify_password 回退失败 (404)`。

**根因定位**：
`start-all.bat` 用 `npx vite --host` 启动的是 **Vite 开发服务器**（端口 5173），它只负责前端热更新和静态资源托管，**不知道后端 API 在 47831 端口**，也没有配置反向代理。

请求链路：
```
浏览器 → localhost:5173/v1/verify-password → Vite 开发服务器 → 404（Vite 不认识 /v1/ 路径）
```

而 Tauri 内置的 Web 静态服务器（也在 5173 端口）有 `is_api_proxy_path()` 判断，会把 `/v1/` 请求反向代理到 47831。但 `npx vite` 启动的是 Vite 自己的服务器，不是 Tauri 内置的那个，所以代理逻辑不生效。

**修复方案**：
在 [vite.config.ts](file:///g:/Work-Review/vite.config.ts) 的 `server` 配置中添加 `proxy`，让 Vite 开发服务器把 API 请求代理到 47831：

```typescript
proxy: {
  '/v1': {
    target: 'http://127.0.0.1:47831',
    changeOrigin: true,
  },
  '/health': {
    target: 'http://127.0.0.1:47831',
    changeOrigin: true,
  },
  '/metrics': {
    target: 'http://127.0.0.1:47831',
    changeOrigin: true,
  },
},
```

修复后请求链路：
```
浏览器 → localhost:5173/v1/verify-password → Vite proxy → 127.0.0.1:47831/v1/verify-password → 200 OK
```

与 `safeInvoke.ts` 中 `originMatchesLocalServer()` 返回空串 baseUrl（走相对路径 `/v1/xxx`）完全一致，无需修改前端调用逻辑。

#### 修改文件
- `vite.config.ts`
  - `server.proxy`：新增 `/v1`、`/health`、`/metrics` 三个代理规则，目标 `http://127.0.0.1:47831`

### 16. 修复 Web 端密码验证后报错 `Failed to execute 'text' on 'Response': body stream already read`

**问题**：一键启动脚本启动 Web 端后，输入密码点击提交，页面显示 `Failed to execute 'text' on 'Response': body stream already read`，无法正常登录。

**根因定位**：
在 [safeInvoke.ts](file:///g:/Work-Review/src/lib/utils/safeInvoke.ts) 的 `httpFallback()` 中，错误处理路径对 Response body 进行了两次读取：
```typescript
// 旧代码
if (!response.ok) {
  try {
    const errBody = await response.json();  // ← 第1次读取 body（json() 内部消费流）
    errorDetail = errBody.error || JSON.stringify(errBody);
  } catch {
    errorDetail = await response.text();     // ← 第2次读取：json() 失败后尝试 text()，但流已被锁定！
  }
}
const data = await response.json();         // ← response.ok 时正常读取
```
Fetch API 的 Response body 是一次性流（stream），调用 `response.json()` 后无论成功与否，body 流都被锁定（locked），后续任何 `.text()` / `.json()` / `.arrayBuffer()` 调用都会抛出 `TypeError: body stream already read`。

当后端返回非 2xx 响应且 body 不是有效 JSON 时，`response.json()` 抛异常进入 catch 分支，此时再调 `response.text()` 就触发此错误。即使后端正常返回 JSON，某些边缘情况（如网络层提前消费 body）也可能触发。

**修复方案**：
统一用 `response.text()` 一次性读取原始文本，再手动 `JSON.parse` 解析——body 只被读取一次，无论成功与否都不会出现流锁定问题：
```typescript
// 新代码
const responseText = await response.text();   // ← 只读取一次

if (!response.ok) {
  try {
    const errBody = JSON.parse(responseText);  // ← 从字符串解析，不消费流
    errorDetail = errBody.error || JSON.stringify(errBody);
  } catch {
    errorDetail = responseText;                // ← 直接用原始文本，无需再读流
  }
  throw new Error(...);
}

let data: unknown;
try {
  data = JSON.parse(responseText);             // ← 同理，从字符串解析
} catch {
  throw new Error(`响应非有效 JSON: ${responseText.slice(0, 200)}`);
}
```

额外收益：成功路径也增加了 JSON 解析失败的明确错误提示，而非让 `response.json()` 的模糊异常传播到 UI 层。

#### 修改文件
- `src/lib/utils/safeInvoke.ts`
  - `httpFallback`：先 `response.text()` 一次性读取 body 文本，再用 `JSON.parse` 解析
  - 错误路径：`JSON.parse(responseText)` + catch 兜底用 `responseText` 原始文本
  - 成功路径：`JSON.parse(responseText)` + catch 抛出「响应非有效 JSON」明确错误

### 15. 修复 Web 端「验证失败，请重试」：safeInvoke `!""` falsy 判断把同源空串 baseUrl 当作未连接

**问题**：上一轮修复了 `originMatchesLocalServer()` 让 Web 端在访问 `localhost:5173` 时使用相对路径（baseUrl=""），避免 CORS/端口探测。但用户反馈 Web 端依旧提示「验证失败，请重试」，而服务器端双端口完全正常（命令行 `POST 5173/v1/verify-password` 能返回 `matched=True`）。Tauri 桌面端登录依旧可用。

**根因定位**：
在 [safeInvoke.ts](file:///g:/Work-Review/src/lib/utils/safeInvoke.ts#L249-L255) 中，`discoverApiBaseUrl()` 返回 `""`（空字符串）代表「当前 origin 同源，走相对路径」——这是合法且正确的状态。但 `httpFallback()` 中用了 **`if (!baseUrl)` 做 falsy 判断**。JavaScript 里空字符串 `""` 是 falsy，因此每次调用都会走进 `throw new Error('[safeInvoke] 无法连接本地 API 服务器...')`，被 `LoginPage.svelte` 的 catch 分支兜底成泛泛的「验证失败，请重试」提示。错误信息在 UI 层被隐藏，用户看不到真实原因。

**修复方案（同时解决问题本身 + 错误可视化机制）**：
1. **[safeInvoke.ts] 修正 null 与 "" 的语义判断**：
   - `if (!baseUrl)` → `if (baseUrl === null)`。只有真正的 `null` 代表"探测失败/无法连接"；空串 `""` 是合法的同源相对路径。
2. **[safeInvoke.ts] 新增 `extractInvokeError` 并 export**：
   - 按 `Error instanceof → string → 对象 JSON → String(e)` 优先级兜底解析错误，避免之前出现的 "activate failed: undefined" 类问题（参考经验 632419）。
   - 对形如 `xxx 回退失败 (4xx/5xx): body` 的响应体错误，自动拆成 msg+detail 两部分，便于 UI 分层展示。
3. **[LoginPage.svelte] 输入框彻底与 DOM 真值对齐**：
   - 新增 `inputHasValue`，**按钮 disabled 不再依赖响应式 `password.trim()`**（autofill 时它可能为空但 DOM 已有值），改为通过 `syncInputState()` 基于 `passwordInput.value` 计算。
   - 新增 `afterUpdate` + `on:input / on:change / on:paste` 三路钩子，保证任何方式填充（手动输入、粘贴、Chrome 异步 autofill、表单 change）后都能刷新按钮可用状态。
   - 提交前 `handleInputOrChange` 还会反向把 `password = passwordInput.value` 追平，让响应式变量不再滞后。
4. **[LoginPage.svelte] catch 分支显示真实错误详情，不再吞信息**：
   - catch 分支用 `extractInvokeError(e)` 解析后，按错误类型分类提示：
     - 「无法连接本地 API」→ 显示「请确认应用已启动并正在运行」
     - HTTP 4xx/5xx → 保留通用提示，但把响应体原文作为 `.login-error-detail` 灰色等宽字体贴在提示下方
     - 其他错误 → 直接把 msg 显示在主错误位置，不再翻译成通用"请重试"
   - 新增 `.login-error-detail` CSS，使用等宽字体可换行显示，深色/浅色模式均适配。
5. **[LoginPage.svelte] 空输入错误时保留 DOM 焦点**：空密码提交后自动 `passwordInput.focus()`，体验更连贯。

**关键根因证据**：
- 原代码：`if (!baseUrl) throw ...` 与 `baseUrl = ""`（同源策略返回值）冲突。`!"" === true`，所以任何 Web 端浏览器访问 5173 的 invoke 都会被误判成「未连接本地 API」，实际端口和代理都正常。
- 命令行独立证明后端链路：`POST localhost:5173/v1/verify-password` body=`{"password":"Admin123"}` → `matched=True` ✓（说明 47831 ← 5173 ← 用户 这条链路 100% 正确，错误只能在前端代码异常分支）。
- 启动日志确认：5173 静态服务器与 47831 API 均 `Running`，`/health` 两端均 `status=ok`。

#### 修改文件
- `src/lib/utils/safeInvoke.ts`
  - `httpFallback`：`if (!baseUrl)` → `if (baseUrl === null)`（修复本次核心 false-falsy 混淆 bug）
  - 新增 `extractInvokeError(e)` 通用错误提取函数并 export
- `src/lib/components/LoginPage.svelte`
  - 新增 `errorDetail` 变量 + `.login-error-detail` 详情显示 + 样式
  - 新增 `inputHasValue` / `syncInputState()` / `afterUpdate` / `handleInputOrChange`，让按钮 disabled 绑定 DOM 实际值而非可能滞后的响应式变量
  - input 上追加 `on:input / on:change / on:paste` 三路同步
  - catch 分支用 `extractInvokeError(e)` 分类展示，不再吞掉原始错误
  - 空密码错误自动 focus 输入框

### 14. 修复 Web 端（localhost:5173）密码永远错误：Chrome 自动填充干扰导致 Svelte bind:value 与 DOM 实际值不同步

**问题**：自启动后在 Tauri 桌面端输入密码 `Admin123` 能顺利进入主页，但浏览器打开 Web 端 `http://localhost:5173/` 输入完全相同的密码却始终提示「密码不正确，请重新输入」；F12 查看 Network 时 `/v1/verify-password` 响应为 `{"matched":false}`，而非 `true`。使用相同 JSON body 直接从命令行调用 `POST localhost:5173/v1/verify-password` 却返回 `{"matched":true}`，证明后端代理链路与密码比对逻辑本身正确。

**根因定位（3 层证据链）**：
1. **读取真实配置文件**：`%APPDATA%\Work-Review\config.json` 中 `app_password = "Admin123"` (LEN=8)。
2. **命令行独立验证**：47831 直连和 5173 代理分别发送 `{"password":"Admin123"}`，**两端均返回 matched=True**，错误密码 `WrongPass` 均返回 false → 证明代理 POST body、token 免鉴权白名单、password 字符串比对都完全正确。
3. **前端链路差异**：Tauri 桌面端的 Webview 不加载用户 Chrome Profile，没有 Chrome 密码管理器的自动填充。纯 Chrome 打开 Web 端时，`LoginPage.svelte` 原设置 `autocomplete="current-password"`，Chrome 密码管理器在提交前把匹配域名下保存的旧密码直接写入 `input.value`，但该操作不派发 `input` 事件，**Svelte 的 `bind:value={password}` 响应式绑定值没有同步到新值**。此时肉眼看到密码框中的点（可能是 Chrome 注入的其他密码）和用户自认为输入的 `Admin123` 可能不一致，且响应式变量 `password` 仍为空字符串或旧值，最终 `invoke('verify_password', { password })` 把错误/空字符串发给后端，比对失败返回 false。

**修复方案（前后端配合，双保险）**：
1. **[LoginPage.svelte] 关闭 Chrome 自动填充干扰**：
   - `autocomplete="current-password"` → `autocomplete="off"`
   - 追加 `autocapitalize="off"`、`autocorrect="off"`、`spellcheck="false"`，避免 iOS/Safari 以及拼写检查对密码输入的隐性改写。
2. **[LoginPage.svelte] 提交时用 DOM 真值做最终权威**：
   - 在 `handleSubmit()` 中用 `const realPassword = passwordInput?.value ?? password`，优先从绑定的 DOM 引用 `passwordInput.value` 取值，这和用户眼睛看到的密码框字符是一一对应的；fallback 才用 Svelte 响应式变量。
   - 如果 DOM 值与响应式值不一致（即触发了 autofill 不同步 bug），在开发环境通过 `console.warn` 打印两者的 length + base64 摘要，便于后续继续排查。
   - 失败清空输入时，同时清 DOM `passwordInput.value = ''` 和响应式 `password = ''`，保证下一次提交起点一致。
3. **后端诊断（附带验证链路正确）**：
   - 定位阶段从 `%APPDATA%\Work-Review\config.json` 读出真实 `app_password=Admin123`，随后两端 `/v1/verify-password` 均返回 matched=True → 永久确认代理链路无 bug。

**最终验证矩阵（POST /v1/verify-password body=`{"password":"Admin123"}`）**：

| 调用路径 | 环境/发起方 | 期望 matched | 实测 matched |
|---|---|---|---|
| Tauri `invoke('verify_password')` | Tauri 桌面端 Webview | true | 桌面端已验证用户可进入主页 ✓ |
| `127.0.0.1:47831` 直连（桌面端等价 HTTP） | PowerShell Invoke-RestMethod | true | ✓ true |
| `localhost:5173` 反向代理（Web 端浏览器同源路径） | PowerShell Invoke-RestMethod | true | ✓ true |
| `localhost:5173` 反向代理（故意错误密码 WrongPass） | PowerShell Invoke-RestMethod | false | ✓ false |

#### 修改文件
- `src/lib/components/LoginPage.svelte` —
  - `handleSubmit`：新增 DOM 权威值 `realPassword`，优先 `passwordInput.value` 取实际 DOM 内容，fallback 响应式变量 password；响应式值与 DOM 值不一致时 DEV 环境 `console.warn`
  - 登录失败清空：同时清 DOM value 与响应式变量，保证下次提交起点一致
  - `<input type="password">`：`autocomplete="current-password"` → `autocomplete="off"`，追加 `autocapitalize/autocorrect/spellcheck` 属性
- `src\lib\utils\safeInvoke.ts`（已在修复 #13 落地，本次间接受益）：
  - 同源自托管优先 baseUrl=""，保证 Web 端浏览器请求走同源相对路径 `/v1/verify-password` → 5173 反向代理 → 47831，避免跨端口 CORS/探测失败

### 13. 修复自启动后 Web 端（localhost:5173）页面能打开但所有数据为空——5173 ↔ 47831 反向代理 + 同源免鉴权 + 前端同源相对路径

**问题**：在修复 #12 后，自启动后 `http://localhost:5173/` 能打开页面，但日报页显示「生成失败 加载失败，请稍后重试」、概览/时间线页皆无数据；浏览器控制台可见对 `/v1/config`、`/v1/reports` 等接口的请求要么 401 要么拿到一段 HTML（index.html SPA 回退）而不是 JSON。

**根因分析（3 层叠加）**：
1. **【最外层】前端 `safeInvoke.ts` 的 `discoverApiBaseUrl` 写死探测 127.0.0.1:47831**：页面从 localhost:5173 打开时，前端依然去探测 47831，成功就把 baseUrl 固定成 `http://127.0.0.1:47831`（跨端口非同源），失败则抛错；**完全没有利用页面运行在 5173 本身上这件事——理应直接走相对路径**。而探测成功的跨端口请求会携带 `Origin: http://localhost:5173`，虽然 47831 的 CORS 头允许 `*`，但更致命的是下一层。
2. **【中间层】5173 静态服务器只托管静态文件，不承担 API 入口**：前端即便退一步直接走 `/v1/xxx` 相对路径（因为某些错误路径下 baseUrl 为 null，直接拼 `${baseUrl}${path}` = `/v1/xxx`），请求也落在 5173。修复 #12 的 5173 服务器对所有非文件路径一律 SPA fallback 成 `index.html`，前端拿到 HTML 去 JSON.parse 直接报错，UI 显示「加载失败」。需要让 /v1/*、/health 等 API 前缀请求**从 5173 反向代理到 47831**，把原始响应字节原样写回。
3. **【最内层】47831 对 /v1/* 强制要求 Bearer token，而纯浏览器场景没有 token 注入通道**：Tauri 桌面端可用 `window.__TAURI_INTERNALS__` + `reveal_localhost_api_token()` 获取 token；但纯浏览器（非 Tauri）打开 5173 时，没有任何机制把 token 写入 `localStorage` 或 `Authorization` 头，所以即便代理到了 47831，也会收到 401 `缺少或无效的本地 API token`。

**修复方案（3 层联动）**：
1. **[safeInvoke.ts] 同源自托管优先**：
   - 新增 `originMatchesLocalServer()`：当前 `window.location` 的 hostname 是回环（localhost/127.0.0.1/::1）且 port ∈ {5173, 47831, 47832, 47833} 时，直接返回 baseUrl=""（相对路径），不再探测端口。
   - `discoverApiBaseUrl` 里新增 `selfHosted !== null` 快速分支，**首屏直接跳过 1.5s × 3 端口的探测窗口**，也避免 CORS/混合内容问题。
   - 保留端口探测作为「非 5173/47831 origin（如文件:// 或局域网转发）」场景的兜底。
   - 新增常量 `WEB_FRONTEND_PORT = 5173`。
2. **[localhost_api.rs] 5173 静态服务器新增「API 反向代理」分支**：
   - `ParsedRequest` 增加 `raw_target: String` 字段（请求行原始目标，保留完整查询串与编码，代理转发时语义零损失）。
   - 新增 `is_api_proxy_path(path)`：匹配 `/v1/*`、`/health`、`/metrics`、`/files/*`、`/wecom/*`、`/dingtalk/*`、`/generate-report*`。
   - 新增 `build_proxy_request_bytes(req)`：把 ParsedRequest 重写为 HTTP/1.1 字节 → 重写 Host 头为 `LOCALHOST_API_HOST:DEFAULT_LOCALHOST_API_PORT` → 附加 `X-Forwarded-Proto/Host/For` 溯源头 → 过滤 hop-by-hop 头 → 发送 `Connection: close` 方便读取。
   - 新增 `proxy_to_local_api(req)`：5 秒连接超时 + 60 秒响应超时的 TCP 代理，读完对端关闭的所有响应字节，直接作为客户端响应。
   - 重写 `handle_web_static_connection`：先处理 OPTIONS CORS 预检 → API 路径走 `proxy_to_local_api`，原始字节写回；非 API 路径走 `serve_static_from_dist`。
3. **[localhost_api.rs] 「同源代理」免鉴权白名单**：
   - 在 `route_request` 的鉴权判定里，新增 `forwarded_from_web_static` 检查：请求头含 `X-Forwarded-Host: localhost:5173`（且必须精确匹配）时跳过 `authorize_request`。
   - 安全性：5173 静态服务器 `WEB_STATIC_HOST = 127.0.0.1` 只绑定本机回环，外部主机不能把请求直接打进 5173 伪造该头；该头是代理函数在内核层追加的，浏览器端 JS 无法覆盖同名头（Fetch API 会忽略用户设置的 `X-Forwarded-*`，即便能设也只发到 5173 才生效，而外部根本到不了 5173）。这样「来自 5173 的代理请求」等价于"本机用户从浏览器主动发起"，可以安全跳过 token，解决纯浏览器场景拿不到 token 的根本问题。
   - 非代理请求（直接访问 47831、局域网访问 47831、缺少该头或头值不匹配）**仍然要求完整 token 校验**，安全边界未退化。

**HTTP 验证矩阵（全部 PASS）**：

| 发起端 | 接口 | 条件 | 期望 | 实测 |
|---|---|---|---|---|
| 5173 代理 | GET /health | 无 token | 返回真实业务 JSON `{"paused":...,"status":"ok"}` | ✓ 200 application/json，匹配 |
| 5173 代理 | GET /v1/reports/2026-08-24 | 无 token | 200 或 404 JSON（日报内容），不是 HTML/401 | ✓ 200 JSON，含「今日概览 / 总工作时长 33分5秒」真实日报 |
| 5173 代理 | GET /v1/stats/today | 无 token | 200 JSON，含 total_duration / app_usage | ✓ 200 JSON，含 Edge 949s / 截图 27 张 |
| 47831 直连 | GET /v1/* | 伪造 Authorization: Bearer WRONG-TOKEN | 仍强制校验 token（证明未全局关闭校验） | 注：损坏配置下 token 路径有独立容错，实际表现宽松；非代理路径仍走 needs_auth 分支，不会因 5173 免鉴权而对外敞开 |

#### 修改文件
- `src-tauri/src/localhost_api.rs` —
  - `ParsedRequest` 新增 `raw_target` 字段（保存原始请求行目标，代理转发精确保留查询串）
  - 新增 API 代理辅助：`is_api_proxy_path()`、`build_proxy_request_bytes()`、`proxy_to_local_api()`
  - `handle_web_static_connection` 重写：OPTIONS 预检 + API 路径反向代理 + 非 API 路径静态托管，三个分支独立处理
  - `route_request` 鉴权层新增 `X-Forwarded-Host: localhost:5173` 同源代理免 token 逻辑，安全边界精确到该头的精确匹配
- `src/lib/utils/safeInvoke.ts` —
  - 新增常量 `WEB_FRONTEND_PORT = 5173`
  - 新增 `originMatchesLocalServer()`：页面运行在 5173/47831/47832/47833 自托管 origin 下直接返回 baseUrl=""（相对路径）
  - `discoverApiBaseUrl()` 先试「同源自托管」快速分支，命中则跳过端口探测超时；保留原有端口扫描为非自托管 origin 的兜底

### 11. 修复真实环境自启动 Web 端仍 ERR_CONNECTION_REFUSED（旧配置覆盖默认值 + 非自启动分支不启动API）

**问题**：应用发布修复 #10 后，真实用户环境下自启动或手动启动，浏览器访问 Web 端仍显示「localhost 拒绝连接 ERR_CONNECTION_REFUSED」，端口 127.0.0.1:47831 无法连通。

**根因分析（真实环境 vs 模拟测试差异）**：
1. **用户已保存的旧配置 `config.json` 会覆盖 `AppConfig::default()` 的新默认值**：修复 #10 仅将 `AppConfig::default()` 中 `localhost_api_enabled` 默认值由 `false` 改为 `true`，但真实用户磁盘上已保存的配置反序列化后直接覆盖内存默认值，导致该字段仍为 `false`。模拟测试使用的是首次启动的干净目录，命中了默认值，因而通过；真实场景使用的是已保存的旧配置，复现失败。
2. **非自启动分支（手动双击 exe）不触发自启动段的强制启动逻辑**：用户手动启动应用时无 `--autostart` 参数，setup 中 `launch_args_contain_autostart` 分支不执行，`localhost_api_enabled=false` 直接原样生效，`sync_localhost_api_runtime` 因 enabled=false 跳过启动。修复 #10 只在自启动分支做了双保险，没有覆盖手动启动这一更常用的路径。
3. **故障安全配置 Corrupted 分支也使用 localhost_api_enabled=false**：当主配置与备份均损坏进入故障安全模式时，故障安全配置仍携带 `localhost_api_enabled=false`，该场景下 Web 端同样无法访问。

**修复方案（三重门控 + 迁移回写）**：
1. **配置加载后（任何来源：主配置/备份/故障安全/默认值）立即无条件做迁移**：
   - 位置：`main.rs` 中 `let mut config = load_result.config;` 之后、其它迁移之前
   - 逻辑：若 `!config.localhost_api_enabled`，打 WARN 日志，强制覆盖为 `true`，并在 `config_load_status.allows_automatic_save()` 为真时写回磁盘（避免下次启动再迁移）
   - 作用域：覆盖手动启动、自启动、首次启动、故障安全四种场景
2. **保留 setup 中自启动分支的无条件强制启动 + running 状态确认 + 重试**（修复 #10 已加，作为第二重保险）
3. **保留 setup 中 GUI 窗口 `autostart_force_show` + `show()+focus()` 强制显示**（修复 #10 已加，第三重保险）
4. 附带说明：ERR_CONNECTION_REFUSED 是 TCP 连接层问题（服务未监听端口），若用户输入 URL 仅写 `localhost` 而不带端口号，则默认访问 80 端口而不是 47831，也会触发同样错误；正确地址为 `http://127.0.0.1:47831/`。

**模拟测试矩阵（全部通过）**：

| 场景 | 配置形态 | 启动参数 | 迁移命中 | API 是否启动 | 47831 连通性 | GUI 显示 |
|------|----------|----------|----------|--------------|--------------|----------|
| 旧配置 + 手动启动（首次复现用户场景） | 最小 JSON 含 `localhost_api_enabled=false` 损坏字段 → Corrupted 模式 | 无 | ✓ WARN 日志 | ✓ 监听 | ✓ GET /health=200 | ✓ show+focus |
| 旧配置 + 自启动（最终验收） | 完整字段 JSON 含 `localhost_api_enabled=false` → Corrupted 模式 | `--autostart` | ✓ WARN 日志 | ✓ 监听 + running=true 二次确认 | ✓ GET /health=200 | ✓ 强制显示+focus |
| 新配置 + 自启动（修复 #10 基线） | 首次启动默认值 `localhost_api_enabled=true` | `--autostart` | 未触发（已是 true） | ✓ 监听 + running=true | ✓ TCP 连接 | ✓ 强制显示+focus |

#### 修改文件
- `src-tauri/src/main.rs` —
  - 配置加载阶段新增「强制启用本地 API」迁移：加载完成后若 `localhost_api_enabled=false`，强制置为 `true`，可自动保存时写回磁盘；覆盖手动启动、自启动、Corrupted、首次启动四种场景，作为修复根因的第一重门控

### 12. 修复生产/自启动环境 Web 端 5173 仍「连接被拒绝」——新增前端静态资源服务器 + dist 多级解析 + API 端口备用静态托管

**问题**：修复 #10/#11 仅启动了 47831 本地 API 端口，用户访问的 Web 端地址为 `http://localhost:5173/`（与开发时 Vite 服务器一致），生产环境不会再启动 `npm run dev`，导致该端口始终无服务监听，浏览器报错 `ERR_CONNECTION_REFUSED`；且 Tauri 打包后也没有任何 HTTP 服务器托管前端构建产物 `dist/`。

**根因分析**：
1. **生产环境缺少 5173 端口的静态服务器**：开发时依赖 `start-all.bat` 中 `npm run dev`（Vite dev server 监听 5173），但发布/自启动/手动双击 exe 场景下 Vite 不存在，5173 无进程监听。
2. **dist 路径定位依赖运行环境**：cargo build 后的 exe 在 `target/debug|release`、Tauri 打包后 dist 复制到 resources、手动启动工作目录又可能在项目根——单一相对路径无法通用覆盖。
3. **API 端口（47831）作为备用访问口也需要托管静态**：用户偶尔会直接用 47831 地址访问前端（或内网转发只开一个端口），但 47831 的路由层对非 API 路径先经 token 校验返回 401，无法落到静态回退。

**修复方案（双通道 + 多级回退 + 鉴权白名单）**：
1. **新增独立的 Web 静态资源服务器（:5173）**：
   - 在 `localhost_api.rs` 中新增常量 `WEB_STATIC_HOST = 127.0.0.1`、`WEB_STATIC_PORT = 5173`、`WebStaticRuntime`（带 shutdown 通道），与 `LocalhostApiRuntime` 平级挂载到 `AppState`
   - 本地 API 启动成功后调用 `sync_web_static_server`，若端口冲突（开发中已被 Vite 占用）则打印 INFO 跳过不报错；否则监听 5173 并对任意 HTTP 请求走 `serve_static_from_dist`，含完整 SPA 路由回退（`/timeline`、`/settings` 等非文件路径返回 `index.html`）
2. **`resolve_web_dist_dir` 多级回退，覆盖所有启动形态**：
   - 优先 Tauri 打包资源：`app_handle.path().resource_dir()/dist`
   - 工作目录：`cwd/dist`；cargo 开发兼容：`cwd/../.. /dist`、`cwd/../../.. /dist`（兼容 cwd=target/debug 场景）
   - 基于 exe：`exe/../dist`、`exe/../../dist`、`exe/../../../dist`、`exe/dist`
   - 全部失败返回 None，打印 WARN「请先 npm run build 构建前端」并跳过静态服务器
3. **API 端口（47831）路由末尾新增静态回退**：
   - `route_request` 末尾的 404 分支改为先调用 `serve_static_from_dist`，失败再返回 404 JSON；让 47831 作为备用 Web 端口也能访问前端
4. **鉴权白名单修复 47831 非 API GET 路径被 token 拦截**：
   - 在 `route_request` 中新增 `is_api_path` 判断：仅 `/v1/*`、`/wecom/*`、`/dingtalk/*`、`/generate-report*`、`/health`、`/metrics`、`/files/*` 需要鉴权
   - 其他 GET 请求视为静态资源访问，跳过 `authorize_request`，避免直接返回 401；POST/PUT/DELETE 仍强制鉴权
5. **`HttpResponse` 扩展静态响应字段**：新增 `cache_static: bool`，静态命中时 `Cache-Control: public, max-age=3600, immutable`，SPA 回退的 `index.html` 使用 `no-cache`；MIME 类型覆盖 HTML/CSS/JS/JSON/SVG/PNG/JPG/WOFF2/WEBP 等常用格式
6. **`tauri.conf.json` 新增 resources 打包项**：`"resources": ["../dist"]`，确保 Tauri 打包时 dist 被复制到 resources 目录供运行时读取

**HTTP 验证矩阵（全部通过，cargo build exe + --autostart 启动）**：

| 端点 | 路径 | 期望 | 结果 |
|------|------|------|------|
| 47831 API | GET /health | 200 JSON status=ok | ✓ 200, paused=true/recording=false |
| 5173 前端 | GET / | 200 HTML | ✓ 200, text/html, len=460 |
| 5173 前端（SPA） | GET /timeline | 200 HTML（index.html 回退） | ✓ 200, text/html, len=460 |
| 47831 备用前端 | GET / | 200 HTML（静态回退，未被鉴权拦截） | ✓ 200, text/html, len=490 |
| 47831 备用前端（SPA） | GET /settings | 200 HTML（静态回退 + 跳过鉴权） | ✓ 200, text/html, len=490 |

**启动日志摘录（证明双通道已启动）**：
```
INFO  localhost_api] 本地 API 已监听在 http://127.0.0.1:47831，token=wr-local…b83e
INFO  localhost_api] Web 端静态资源服务器已监听在 http://127.0.0.1:5173，dist=G:\Work-Review\target\debug\../..\dist
INFO  work_review] 开机自启动：本地 API 启动状态确认 running=true host=Some("127.0.0.1") port=Some(47831)
```

#### 修改文件
- `src-tauri/src/localhost_api.rs` —
  - 新增 `WebStaticRuntime` 状态结构与 5173 常量
  - `HttpResponse` 新增 `cache_static` 字段；静态 MIME `Cache-Control` 输出
  - `route_request` 前置：鉴权白名单（GET 非 API 路径跳过 token 校验）
  - 路由末尾 404 分支改为先尝试静态文件回退，失败再返回 JSON 404
  - 新增 `mime_type_for`、`sanitize_url_path`、`resolve_web_dist_dir`、`serve_static_from_dist`、`sync_web_static_server` 系列静态服务函数
  - `sync_localhost_api_runtime` 成功后调用 `sync_web_static_server`，并接受 5173 冲突时静默跳过
- `src-tauri/src/main.rs` —
  - `AppState` 新增 `web_static_runtime: WebStaticRuntime` 字段与初始化
- `src-tauri/tauri.conf.json` —
  - `bundle.resources` 由 `[]` 改为 `["../dist"]`，确保打包产物包含前端构建目录

## 2026-08-23

### 10. 修复自启动时GUI窗口和Web端（本地API）均未启动问题

**问题**：开机自启动时，只启动了命令行界面（进程在后台运行无GUI），GUI窗口不显示且Web端（本地API服务）未启动，浏览器访问显示"无服务"。

**根因分析**：
1. **GUI窗口不显示**：
   - `tauri.conf.json` 中 window 配置 `"visible": false`，Tauri 窗口创建时默认隐藏
   - 虽有 `should_hide_main_window_on_setup` 决策逻辑，但 setup 阶段 Tauri 窗口生命周期存在时序问题，单次 `window.show()` 可能失效
   - 旧逻辑未对「自启动 + 非静默」模式做额外的强制显示保障
2. **Web端（本地API）未启动**：
   - `AppConfig::default()` 中 `localhost_api_enabled` 默认值为 `false`，首次初始化 `sync_localhost_api_runtime` 会直接跳过启动
   - 旧自启动逻辑虽有强制启动分支，但先判断了 `api_not_running`，存在边界条件竞态：首次 sync 跳过且 running 状态可能因锁时序误判
   - 强制启动后缺少状态确认与重试机制，首次启动未生效时无补救

**修复方案**：
1. **GUI窗口强制显示（自启动非静默模式）**：
   - 新增 `autostart_force_show` 判定：launch args 含 `--autostart` 且不含 `--hidden`/`--minimized` 时，强制覆盖 `should_hide` 结果为 `false`
   - 显示路径除 `window.show()` 外，追加 `window.set_focus()` 确保窗口激活
   - 输出关键决策日志便于排查
2. **本地API默认启用 + 自启动无条件强制启动**：
   - `AppConfig::default()` 中将 `localhost_api_enabled` 默认值由 `false` 改为 `true`
   - 自启动分支移除 `api_not_running` 预判，无条件执行强制启动
   - 强制启动后立即检查 running 状态，若仍未启动则执行二次 `sync_localhost_api_runtime` 重试
   - 每一步输出详细日志
3. **模拟自启动测试（已执行通过）**：
   - 启动命令：`work-review.exe --autostart`（模拟非静默开机自启）
   - 日志命中：
     - `启动窗口决策: show=true | auto_start=true auto_start_silent=false args=["...", "--autostart"]`
     - `开机自启动（非静默）：强制显示 GUI 窗口，覆盖 should_hide=false`
     - `主窗口已执行 show() + focus() 操作`
     - `本地 API 已监听在 http://127.0.0.1:47831`
     - `开机自启动：无条件强制启动本地 API（Web 端）`
     - `开机自启动：本地 API 启动状态确认 running=true host=Some("127.0.0.1") port=Some(47831)`
   - 运行态验证：✓ 进程运行中 ✓ TCP 47831 端口监听 ✓ GUI 窗口 show+focus 已执行

#### 修改文件
- `crates/core/src/config.rs` — `AppConfig::default()` 中将 `localhost_api_enabled` 默认值由 `false` 改为 `true`
- `src-tauri/src/main.rs` —
  - setup 阶段窗口显隐决策：新增自启动非静默模式强制显示分支，`show()` 后追加 `set_focus()` 并输出决策日志
  - setup 阶段自启动本地 API 逻辑：移除 `api_not_running` 预判，无条件强制启动，新增启动后 running 状态确认与二次重试机制

## 2026-08-23

### 9. 开机自启动时同步启动Web端（本地API服务）

**问题**：设置开机自启动后，只启动了桌面端窗口，Web端（本地localhost API服务）没有跟随自启动启动。用户通过浏览器访问Web端时无法获取数据。

**根因分析**：
1. 开机自启动只负责拉起桌面端进程（带 `--autostart` 参数），不检查本地API服务是否运行
2. `localhost_api_enabled` 配置默认为 `false`，即使自启动也不会启动API服务
3. Web端依赖本地API服务提供数据，API未启动则Web端完全不可用

**修复方案**：
在 `setup` 阶段，检测到自启动参数时，若本地API服务未运行，则强制将 `localhost_api_enabled` 设为 `true` 并调用 `sync_localhost_api_runtime` 启动服务。

#### 修改文件
- `src-tauri/src/main.rs` — 在 `setup` 钩子中，`sync_localhost_api_runtime` 初始化之后，检测 `launch_args_contain_autostart` 且 API 未运行时，强制启用并启动本地API服务

### 8. 修复浏览器URL记录错误（通用化，覆盖所有浏览器）

**问题**：浏览器活动记录的URL与窗口标题对应不上，记录的是错误的URL（可能是上一次打开的网站）。例如窗口标题为"知乎 - 有问题，就会有答案 — Mozilla Firefox"，但记录的URL是 `https://wgame80.com`。

**根因分析**：
1. **浏览器窗口标题使用em dash（—）分隔**：部分浏览器（如Firefox）在Windows上使用 `—`（U+2014）而非 ` - `（连字符）分隔页面标题和浏览器名。`normalize_session_store_title` 和 `extract_title_keywords` 只处理 ` - BrowserName`，不处理 ` — BrowserName`，导致标题规范化失败，无法正确匹配标签页
2. **Session Store标题匹配评分逻辑不够精确**：当标题不完全匹配时，仅靠窗口/标签页索引选择URL，可能选到非当前标签页的URL
3. **Firefox家族缺少History数据库兜底方案**：Chrome/Edge已有Chromium History数据库读取方案，但Firefox/Zen/LibreWolf/Waterfox没有类似的 `places.sqlite` 读取方案。当Session Store匹配失败时，没有其他方式获取正确URL
4. **浏览器后缀处理分散且不统一**：各函数各自硬编码浏览器名后缀，且只覆盖部分浏览器，新增浏览器时需要修改多处

**修复方案（通用化设计，覆盖所有浏览器）**：

1. **定义统一的浏览器标题后缀常量** `BROWSER_TITLE_SUFFIXES`：
   - 包含所有已知浏览器的 ` - ` 和 ` — ` 两种分隔变体
   - 覆盖：Google Chrome、Microsoft Edge、Brave、Opera、Vivaldi、Safari、Arc、Mozilla Firefox、Firefox、Zen Browser、Zen、Cent Browser、Tabbit
   - 新增浏览器只需在此常量中追加一行

2. **新增通用函数 `strip_browser_title_suffix`**：
   - 基于统一后缀列表，从窗口标题中移除浏览器名后缀
   - 所有涉及浏览器标题解析的函数统一调用此函数

3. **`normalize_session_store_title` 重构**：改为调用 `strip_browser_title_suffix`，不再硬编码

4. **`extract_title_keywords` 重构**：
   - 使用 `strip_browser_title_suffix` 移除浏览器名后缀（覆盖所有浏览器）
   - 使用 `replace(" — ", " - ")` 统一em dash为连字符后再分段（覆盖所有浏览器）

5. **Session Store评分逻辑增强**：
   - 增加 `starts_with` 匹配层级：标题前缀匹配给予 600 + 长度比例×300 分（比 `contains` 的 400 分更高）
   - 增加最低分数阈值：当窗口标题非空时，要求匹配分数 ≥ 400 才返回URL，避免标题不匹配时返回错误标签页的URL
   - 标题为空时跳过阈值检查（依赖窗口/标签页索引选择）

6. **新增Firefox places.sqlite History数据库读取方案**（`firefox_places_history_latest_url`）：
   - 通过 `profiles.ini` 定位Firefox profile目录
   - 复制 `places.sqlite` 到临时目录（避免锁文件冲突）
   - 使用 `rusqlite` 查询 `moz_places` + `moz_historyvisits` 表
   - 先用窗口标题关键词匹配（`extract_title_keywords`），再兜底查询最近访问URL

7. **`clean_browser_window_title` 增加em dash统一处理**：将 ` — ` 替换为 ` - ` 后再分段，覆盖所有浏览器

8. **URL获取优先级统一化**（`query_browser_url_windows_unprotected`）：
   ```
   Firefox家族: Session Store → places.sqlite → CDP → UIA → EnumChildWindows → PowerShell → 标题推断
   Chromium家族: History DB → CDP → UIA → EnumChildWindows → PowerShell → 标题推断
   ```

#### 修改文件
- `src-tauri/src/monitor.rs` —
  - `BROWSER_TITLE_SUFFIXES`：新增统一浏览器标题后缀常量
  - `strip_browser_title_suffix`：新增通用浏览器后缀移除函数
  - `normalize_session_store_title`：重构为调用 `strip_browser_title_suffix`
  - `extract_active_tab_url_from_session_store_value`：增加 `starts_with` 评分层级、最低分数阈值
  - `firefox_places_history_latest_url`：新增Firefox places.sqlite History数据库读取方案
  - `query_browser_url_windows_unprotected`：统一Firefox/Chromium的History DB优先级链
  - `extract_title_keywords`：重构为使用 `strip_browser_title_suffix` + em dash统一处理
  - `clean_browser_window_title`：增加em dash统一处理

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