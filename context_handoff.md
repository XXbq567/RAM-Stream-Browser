# RAM-Stream-Browser — 上下文交接文档

> 最后更新：2026-06-03 | 状态：Sprint 1 调试中

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
| Sprint 1（验证） | 🔴 阻塞 — IPC bridge 在远程域名不可用 |

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

### 坑 8：B站 iframe 限制
- B站设置了 `X-Frame-Options: DENY`，无法在 iframe 中嵌入
- 这意味着不能简单用 iframe 套 B站，必须用 Webview 直接导航

## 7. 下一步

### 立即（Sprint 1 收尾）
1. 确定 Tauri 2.0 的 IPC 是否可在远程域名工作
2. 如果不能 → 切换到多 Webview 架构
3. 验证 Cookie 和 `__playinfo__` 提取

### 短期（Sprint 2）
1. 安装 libmpv dev 库
2. 实现 `BilibiliAdapter`（解析 DASH 流）
3. 实现 `MpvInstance`（创建、配置、播放）
4. Cookie 透传（CookieManager COM API）
5. 音画分离播放验证
6. RAM 缓冲配置

### 中期（Sprint 3）
1. 内存配置 UI（滑块 500MB~8GB）
2. 画质切换
3. B站 UI 控件转发
4. 缓冲状态可视化

## 8. 关键文件索引

| 文件 | 用途 |
|------|------|
| `PROJECT_REPORT.md` | 项目报告、功能清单、架构 |
| `PROGRESS.md` | 详细进度跟踪 |
| `tests/e2e-checklist.md` | 测试计划 |
| `src/inject/preload.js` | 核心注入脚本 |
| `src-tauri/src/lib.rs` | Tauri 入口 |
| `rules/rules.json` | B站适配规则 |
| `白皮书/视频浏览器想法.txt` | 原始设计 v1.0 |
| `白皮书/视频浏览器想法1.txt` | 务实版本 v2.0 |
