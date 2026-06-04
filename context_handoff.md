# RAM-Stream-Browser — 上下文交接文档

> 最后更新：2026-06-04 | 状态：Sprint 2 — 核心播放验证代码就绪，待 GUI 测试

---

## 1. 任务目标

构建基于 **Tauri 2.0 + Webview2 + libmpv** 的 B站专用套壳浏览器。核心能力：

1. **大会员 Cookie 透传** → libmpv 继承登录态，播放 4K HDR 流
2. **RAM 大缓冲** → 视频流下载到物理内存（默认 2GB），进度条随意拖拽秒响应
3. **抗网络波动** → 断网 10 秒视频不卡
4. **版本管理 + 热更新** → GitHub Releases 分发，rules.json 热更新应对 B站改版
5. **极简 UI** → 不做弹幕、不做花哨 UI，MVP 只做核心播放

## 2. 当前进度

| 阶段 | 状态 |
|------|------|
| Sprint 0（设计） | ✅ 完成 |
| Sprint 1（代码） | ✅ 编译通过 |
| Sprint 2（代码） | 🟡 编译通过 — 待 GUI 验证 | libmpv 播放链路代码就绪 |
| Sprint 2（调试） | 🟡 2026-06-04 | 修复 preload.js bridge 干扰 + 为 try_launch_player/find_mpv 添加诊断日志 |
| Sprint 2（验证） | 🔴 未执行 | 需在 GUI 环境跑 `npx tauri dev` 测试完整链路 |
| mpv.exe 环境 | ✅ 已就绪 | `C:\Program Files\MPV Player\mpv.exe` v0.41.0 |

详见 `PROGRESS.md`

## 3. 关键决策

| # | 决策 | 原因 |
|---|------|------|
| 1 | **分屏布局**（Webview2 30% + libmpv 70%） | 放弃白皮书的"透明叠加"方案 — Webview2 DirectComposition 无法被 HWND 穿透 |
| 2 | **MVP 不做弹幕** | 先做核心：4K HDR + RAM 缓冲 |
| 3 | **libmpv 捆绑到安装包** | 用户零配置，+30MB 可接受 |
| 4 | **默认 2GB 单一大缓冲池** | 不区分前向后向，用户可调 500MB~8GB |
| 5 | **Method A 为主**（网络拦截 playurl 响应） | 绕过 B站 WBI 签名问题 |
| 6 | **Method B 备份**（`__playinfo__` 注入） | 双保险 |
| 7 | **B站适配优先** | YouTube 等后期扩展 |

## 4. 设计风格

- **极简**：工具栏仅缓冲条 + 画质下拉 + 音量。mpv 内置 OSD 即可
- **单窗口分屏**：Webview2 侧边栏可折叠
- **无皮肤系统**：默认 mpv 原生外观
- **大会员状态指示**：工具栏显示当前画质是否来自大会员权限

## 5. 技术架构

```
Tauri 2.0 (Rust)
├── Webview2 → 加载 bilibili.com，提取 Cookie + __playinfo__
├── libmpv → 播放 DASH 流，RAM 缓冲 2GB，HDR 色彩校准
├── Rules 热更新 → GitHub jsDelivr CDN
└── Tauri Updater → GitHub Releases
```

## 6. 遇到的坑

### 坑 1：Windows 11 PowerShell 执行策略
- **现象**：`npx : 无法加载文件 ...\npx.ps1，因为在此系统上禁止运行脚本`
- **解决**：`Set-ExecutionPolicy -ExecutionPolicy RemoteSigned -Scope CurrentUser`

### 坑 2：Rust 未安装
- **解决**：通过 rustup 安装 Rust 1.96.0

### 坑 3：`cargo` 不在新 Shell 的 PATH 中
- **现象**：`failed to run 'cargo metadata' command: program not found`
- **原因**：Rust 安装后打开的 PowerShell 窗口未加载新 PATH
- **解决**：关闭所有 PowerShell 窗口，重新打开

### 坑 4：`beforeDevCommand` 路径错误
- **现象**：`http://localhost:1420/` 无法访问
- **原因**：`beforeDevCommand` 在 `src-tauri/` 目录执行，`npx http-server` 找不到根目录 `node_modules`
- **解决**：改为 `cd .. && npm run serve`

### 坑 5：`dangerousRemoteDomainIpcAccess` 在 Tauri 2.0 不存在
- **现象**：`unknown field 'dangerousRemoteDomainIpcAccess'` 编译错误
- **原因**：这是 Tauri 1.x 的配置项，2.0 已移除
- **当前状态**：🔴 待验证 Tauri 2.0 是否默认在远程域名注入 IPC bridge

### 坑 6（当前阻塞）: Tauri 2.0 IPC 在远程域名不可用
- **现象**：B站页面（bilibili.com）上 `window.__TAURI_INTERNALS__` 不可用，注入脚本无法调用 Rust 命令
- **调查中**：Tauri 2.0 的 IPC bridge 是否在外部域名注入
- **备用方案**：多 Webview 架构 — 主 Webview 本地页面（有 IPC），B站在第二 Webview

### 坑 7：B站 HttpOnly Cookie
- **现象**：`document.cookie` 无法获取 `SESSDATA`（大会员关键 cookie）
- **计划**：Sprint 2 用 `webview2-com` COM API 直接访问 CookieManager

### 坑 9：Tauri 2.0 外域 IPC 被 ACL 拦截
- **现象**：`submit_cookies not allowed. Plugin not found`
- **原因**：自定义 `generate_handler!()` 命令在外部域名（bilibili.com）被 Tauri 2.0 安全 ACL 拦截，且**无法通过 capabilities 配置绕过**（编译时权限系统会验证）
- **解决**：放弃 JS `invoke()` 通道，改用 Rust 侧原生方法

### 坑 10：WebView2 混合内容拦截自定义协议
- **现象**：`fetch('ram-stream://...')` 和 `XMLHttpRequest` 从 HTTPS 页面被完全拦截，Rust 终端无任何日志
- **原因**：WebView2 的混合内容策略将 HTTPS→自定义协议的 fetch/XHR 视为"主动混合内容"并拦截
- **解决**：改用 `<img>` 标签 beacon（被动内容，不受混合内容策略限制）+ `webview2-com` 原生 COM API

### 坑 11：`__TAURI_INTERNALS__` 存在但不等于 IPC 可用
- **现象**：工具栏显示 🟢 IPC，但 `invoke()` 报错 "not allowed"
- **原因**：Tauri 2.0 在外部域名会注入 `__TAURI_INTERNALS__` 对象，但 `invoke()` 调用受 ACL 权限控制。对象存在 ≠ 可调用自定义命令
- **教训**：不要相信 IPC 状态灯，要实际发起一次调用并看错误信息

### 坑 12：preload.js 在 bridge.html 上干扰 AppState
- **现象**：bridge.html 通过 IPC 发送真实数据后 mpv 仍不启动
- **原因**：preload.js 对每个页面注入且无来源检查：
  - `autoSendCookies()` 读取 bridge.html 的 `document.cookie`（空/无关）→ 通过 img beacon 送到 `handle_submit_cookies` → 用空条目覆盖 AppState
  - 2 秒延迟诊断发送 `submit_cookies({cookies:'test=1'})` 通过 Tauri IPC → 再次覆盖真实 cookie
  - `startPlayinfoPoll()` 在 bridge 页面无意义轮询
- **解决**：在 `init()` 中检查 `location.href.indexOf('bridge.html')`，跳过 auto-send、diagnostic、playinfo poll

### 坑 13：`try_launch_player()` 静默失败
- **现象**：IPC 返回值为成功，但 mpv 窗口从不出现，Rust 日志无任何线索
- **原因**：每个提前返回路径（cookies None / playinfo None / player_running true）均无日志
- **解决**：为每个提前返回路径添加 `log::info!`，明确标注哪个条件失败及附加上下文（如 playinfo_json 是否存在、cookie 数量）

## 7. 下一步

### 立即（Sprint 2 GUI 验证）🧪
1. **运行 `npx tauri dev`** — 启动 app，确认编译通过、窗口出现、加载 B站
2. **登录 B站** — 确认 cookie 自动发送（Rust 日志 `[Cookie] Stored N cookies in AppState`）
3. **打开任意 B站视频** — 确认 playinfo 自动捕获（日志 `[PlayInfo] Stored in AppState`）
4. **验证自动播放** — 日志 `[Launch] 🚀 Starting mpv player` → mpv 窗口弹出 → 视频播放
5. **检查 RAM 缓冲** — 任务管理器确认内存增长到 2GB
6. **测试画质切换** — 工具栏下拉选择不同画质 → mpv 重启新流
7. **测试停止按钮** — 点击 Stop → mpv 窗口关闭

### 短期（Sprint 2 修复）
1. 如果 403 → 检查 cookie 是否完整透传（`--http-header-fields`）
2. 如果无 HttpOnly SESSDATA → Sprite 3 用 webview2-com CookieManager
3. 如果 DASH URL 过期 → 改 `try_launch_player` 抓取 playinfo 的时效性
4. 画质切换重启逻辑 → 优化为 mpv 命令行热切换（当前是 kill + 重新 spawn）

### 架构变更（2026-06-04，Sprint 2 实施后）
```
数据流: preload.js → img beacon → ram-stream:// protocol → AppState → try_launch_player() → mpv.exe 子进程
播放器: mpv.exe 独立原生窗口（非嵌入 Tauri），2GB forward buffer + 512MB backward buffer
控制: 浏览器窗口工具栏提供画质选择 + 停止按钮，通过 img beacon 发指令
```
```
新增文件:
  src-tauri/src/parsers/mod.rs  — 共享解析模块（消除 protocol/commands 重复代码）
  src-tauri/src/state.rs        — 全局 AppState（OnceLock + Arc<Mutex<>>）
  src-tauri/src/mpv/mod.rs      — mpv 子进程启动器（非 libmpv crate，纯 Command::spawn）

删除: lib.rs 中手动的 eval() cookie 提取（preload.js 现在自动处理）
```

## 8. 关键文件索引

| 文件 | 用途 |
|------|------|
| `PROJECT_REPORT.md` | 项目报告、功能清单、架构 |
| `PROGRESS.md` | 详细进度跟踪 |
| `tests/e2e-checklist.md` | 测试计划 |
| `src/inject/preload.js` | 核心注入脚本（含画质选择 + 停止按钮） |
| `src-tauri/src/lib.rs` | Tauri 入口（State 初始化、mpv 检测） |
| `src-tauri/src/protocol.rs` | 自定义协议处理器（AppState 存储 + 播放触发） |
| `src-tauri/src/parsers/mod.rs` | 🆕 共享解析模块（消除重复代码） |
| `src-tauri/src/state.rs` | 🆕 全局 AppState（OnceLock + Arc<Mutex<>>） |
| `src-tauri/src/mpv/mod.rs` | 🆕 mpv 子进程启动器（非 libmpv crate） |
| `rules/rules.json` | B站适配规则 |
| `白皮书/视频浏览器想法.txt` | 原始设计 v1.0 |
| `白皮书/视频浏览器想法1.txt` | 务实版本 v2.0 |
