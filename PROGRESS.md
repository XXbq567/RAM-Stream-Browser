# RAM-Stream-Browser — 开发进度

> 最后更新：2026-06-03 | 当前阶段：Sprint 1（代码就绪，待 GUI 验证）

## Sprint 0：设计与文档 ✅ 完成

| 任务 | 状态 | 备注 |
|------|------|------|
| 白皮书 v1.0（激进版） | ✅ 完成 | 系统架构全设计 |
| 白皮书 v2.0（务实版） | ✅ 完成 | MVP 范围收敛 |
| 实施计划 | ✅ 完成 | 含版本管理/CI/CD |
| 项目报告（PROJECT_REPORT.md） | ✅ 完成 | |
| 进度跟踪（PROGRESS.md） | ✅ 完成 | |

## Sprint 1：最小核心跑通 ✅ 代码完成，⏳ 待 GUI 验证

**目标**：Tauri app 打开 B站、登录、打印 Cookie 和 `__playinfo__`
**状态**：Rust 编译通过 (debug binary: 18MB)，代码就绪，需在 GUI 环境运行验证

| # | 任务 | 状态 | 备注 |
|---|------|------|------|
| 1.1 | Tauri 2.0 脚手架 | ✅ | 手动创建，非 `create-tauri-app`，更精确 |
| 1.2 | tauri.conf.json 配置 | ✅ | 窗口 1600x900，CSP 开放，devUrl + http-server |
| 1.3 | 前端入口 + preload 注入 | ✅ | index.html 跳转 B站，preload.js 通过 initialization_script 注入 |
| 1.4 | submit_cookies 命令 | ✅ | 接收 document.cookie，解析 key cookies（HttpOnly 受限） |
| 1.5 | submit_playinfo 命令 | ✅ | 接收 __playinfo__ JSON，解析 dash video/audio 流列表 |
| 1.6 | submit_playurl_response 命令 | ✅ | 接收 fetch hook 截获的 playurl 响应 |
| 1.7 | 注入调试工具栏 | ✅ | preload.js 在 B站页面顶部注入调试按钮 |
| 1.8 | 验证：4K/HDR 流可用 | ⏳ | 需在 GUI 环境实际登录大会员账号测试 |

### Sprint 1 架构决策

- **注入方式**：在 Rust 侧通过 `WebviewWindowBuilder::initialization_script()` 注入 `preload.js`，每页加载时运行
- **Cookie 提取**：Sprint 1 用 `document.cookie`（简单但无法获取 HttpOnly cookie），Sprint 2 改用 webview2-com CookieManager
- **PlayInfo 提取**：双管齐下 — 轮询 `window.__playinfo__`（30s 超时）+ hook `window.fetch` 拦截 playurl API 响应
- **前端**：极简 — index.html 立即跳转到 bilibili.com，所有 UI 通过注入脚本渲染

### 已知限制
- **HttpOnly cookies**（SESSDATA 等）无法通过 document.cookie 获取 → Sprint 2 用 CookieManager COM API
- **无 GUI 环境无法测试**：当前环境 headless，需在 Windows 11 桌面运行 `npx tauri dev`

## Sprint 2：流拦截与 libmpv 播放 ⬜ 未开始

**依赖**：Sprint 1 GUI 验证通过 + libmpv dev 库安装

## Sprint 3：最简交互 + 内存自定义 ⬜ 未开始

**依赖**：Sprint 2 完成

## 项目文件清单

| 文件 | 用途 |
|------|------|
| `src/index.html` | 入口页面，跳转到 B站 |
| `src/inject/preload.js` | 核心注入脚本：工具栏、playinfo 轮询、fetch hook |
| `src-tauri/src/lib.rs` | Tauri 入口：窗口创建、preload 注入、命令注册 |
| `src-tauri/src/main.rs` | Rust main |
| `src-tauri/src/commands/cookie.rs` | Cookie 提取命令 |
| `src-tauri/src/commands/stream.rs` | PlayInfo 解析命令 |
| `src-tauri/src/adapters/mod.rs` | PlatformAdapter trait 定义（Sprint 2 实现） |
| `src-tauri/tauri.conf.json` | Tauri 2.0 配置 |
| `rules/rules.json` | B站提取规则（热更新目标） |
| `PROJECT_REPORT.md` | 项目报告 |
| `PROGRESS.md` | 本文件 |

## 变更日志

| 日期 | 变更 |
|------|------|
| 2026-06-03 | Sprint 0 完成：白皮书阅读、需求讨论、方案设计 |
| 2026-06-03 | Sprint 1 代码完成：Tauri 脚手架、Cookie/PlayInfo 命令、preload 注入、编译通过 |
