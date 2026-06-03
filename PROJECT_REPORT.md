# RAM-Stream-Browser — 项目报告

> 状态：设计阶段 | 代码：0% | 最后更新：2026-06-03

## 1. 项目摘要

**名称**：RAM-Stream-Browser
**定位**：基于物理内存大缓存的 B站专用套壳浏览器
**核心痛点**：浏览器沙盒仅缓存数十秒视频 → 高码率 4K HDR 视频频繁卡顿
**解决方案**：拦截 B站视频流 → 交给 libmpv 在物理内存（RAM）中建立数 GB 缓冲区 → 进度条随意拖拽秒响应 → 彻底屏蔽网络波动

**MVP 一句话**：登录 B站大会员 → 打开 4K HDR 视频 → 视频流直接吃进内存 → 进度条随便拖 → 不卡。

## 2. 技术栈

| 层级 | 技术 | 职责 |
|------|------|------|
| 宿主框架 | Tauri 2.0 (Rust) | 系统进程管理、IPC 通信、低开销 |
| 浏览器层 | Webview2 (Chromium) | 加载 B站页面、登录鉴权、提取流信息 |
| 解码播放 | libmpv (C/Rust 绑定) | 视频解码、RAM 大缓冲、4K HDR 渲染 |
| 扩展预留 | yt-dlp | 未来 YouTube 等平台的流解析（MVP 不做） |

## 3. 核心功能清单

### MVP（当前目标）

| # | 功能 | 优先级 | 状态 |
|---|------|--------|------|
| 1 | Webview2 加载 B站，正常登录 | P0 | ⬜ 未开始 |
| 2 | 继承大会员 Cookie，透传至 mpv | P0 | ⬜ 未开始 |
| 3 | 拦截视频流（Method A: 网络层 / Method B: __playinfo__） | P0 | ⬜ 未开始 |
| 4 | libmpv 播放 B站 DASH 流（音画分离） | P0 | ⬜ 未开始 |
| 5 | RAM 缓冲，默认 2GB，用户可自定义（500MB ~ 8GB） | P0 | ⬜ 未开始 |
| 6 | 进度条随意拖拽，已缓冲区域秒级响应 | P0 | ⬜ 未开始 |
| 7 | 画质切换（4K / 1080p60 / 1080p / 720p） | P0 | ⬜ 未开始 |
| 8 | 4K HDR 色彩校准（HDR tone-mapping + ICC profile） | P1 | ⬜ 未开始 |
| 9 | 缓冲用量可视化（工具栏状态条） | P1 | ⬜ 未开始 |
| 10 | B站 UI 控件转发（暂停/播放/进度条 → mpv） | P1 | ⬜ 未开始 |
| 11 | 规则热更新（rules.json，应对 B站改版） | P1 | ⬜ 未开始 |
| 12 | 降级保护（提取失败 → 原生播放器继续工作） | P0 | ⬜ 未开始 |
| 13 | 应用自动更新（Tauri updater + GitHub Releases） | P2 | ⬜ 未开始 |

### Post-MVP

| # | 功能 |
|---|------|
| 14 | 弹幕渲染引擎（自绘 Canvas） |
| 15 | VirtualLock 物理内存锁定（防止 Pagefile） |
| 16 | YouTube 适配器（via yt-dlp） |
| 17 | 透明叠加布局（Webview2 挖洞） |
| 18 | 多标签页支持 |

## 4. MVP 不做的事（明确边界）

- ❌ 弹幕（推迟到 post-MVP）
- ❌ YouTube / 其他平台（只做 B站）
- ❌ Netflix / Prime 等 DRM 平台（永远不做）
- ❌ 花哨 UI / 皮肤系统（最基础界面）
- ❌ 多标签页（单一视频窗口）
- ❌ macOS / Linux（仅 Windows 11）

## 5. 架构概览

```
+------------------------------------------------------------------+
|                     Tauri 主进程 (Rust)                           |
|                                                                  |
|  [Webview2 ~30%]              [libmpv 渲染区 ~70%]               |
|  - 加载 bilibili.com         - 视频解码 + RAM 缓冲               |
|  - 登录/大会员鉴权           - 4K HDR 色彩输出                   |
|  - 提取 __playinfo__         - mpv 内置 OSD 控制                 |
|  - 评论区/简介/分P          - 进度条随意拖拽秒响应              |
|                                                                  |
|  Cookie 透传 ↑↓              ↑↓ 时间同步 / 控制指令              |
|                                                                  |
|  [本地 Agent]                                                    |
|  - CookieManager → Cookie → mpv --http-header-fields             |
|  - 双保险流提取：网络拦截 + DOM 注入                              |
|  - 降级保护：提取失败 → 恢复原生 <video>                         |
+------------------------------------------------------------------+
```

## 6. 版本管理

| 组件 | 版本规范 | 更新方式 |
|------|----------|----------|
| App 本体 | MAJOR.MINOR.PATCH | Tauri updater + GitHub Releases |
| rules.json | 独立递增整数 | jsDelivr CDN，启动时条件请求 |

## 7. 开发环境

- **OS**：Windows 11 (x64)
- **Rust**：stable toolchain
- **Node.js**：20 LTS
- **Tauri CLI**：2.x
- **libmpv**：dev 包（headers + DLL），捆绑到安装包
- **Webview2**：Windows 11 内置，无需额外安装

## 8. 关键风险

| 风险 | 概率 | 缓解 |
|------|------|------|
| B站改 DOM/API 导致提取失败 | 高 | Rules 热更新 + 双方法提取 + 降级到原生播放器 |
| B站防盗链 403 | 中 | 完整透传 Cookie + Referer + Origin + UA |
| mpv 音画分离不同步 | 中 | 先测试低画质，确认机制可行再上高画质 |
| Webview2 CookieManager API 受限 | 中 | 降级到 JS document.cookie |

## 9. 参考资料

- 白皮书 v1.0：`白皮书/视频浏览器想法.txt`
- 白皮书 v2.0（务实版）：`白皮书/视频浏览器想法1.txt`
- 实施计划：`.claude/plans/readme-github-github-the-skill-of-brims-merry-map.md`
- 进度跟踪：`PROGRESS.md`
