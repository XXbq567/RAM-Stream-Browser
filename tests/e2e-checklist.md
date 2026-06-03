# RAM-Stream-Browser — 端到端测试计划

## Sprint 1：最小核心跑通

### 前置条件

- [ ] Windows 11 桌面环境
- [ ] PowerShell 执行策略已设 `RemoteSigned`
- [ ] 已安装 Rust (`rustc --version`)
- [ ] 已安装 Node.js 20+ (`node --version`)
- [ ] 已安装 npm 依赖 (`npm install`)
- [ ] 有 B站大会员账号（用于验证 4K/HDR 流）

### 启动测试

**环境检查：**
```powershell
cd C:\Users\WBLbo\Documents\GitHub\RAM-Stream-Browser
node --version    # 应显示 v22.x
cargo --version   # 应显示 1.96.0
npm ls @tauri-apps/cli  # 应显示 2.x
```

---

### 测试 1.1：应用冷启动

| 项目 | 内容 |
|------|------|
| **目的** | 验证 Tauri 2.0 + Webview2 能正常启动，无崩溃 |
| **步骤** | 1. 打开 PowerShell<br>2. `cd` 到项目根目录<br>3. 运行 `npx tauri dev` |
| **预期结果** | 1. 终端显示 `[RAM-Stream] v0.1.0 — Window created`<br>2. 弹出一个 1600×900 的窗口<br>3. 窗口标题为 "RAM-Stream-Browser"<br>4. 终端无 error/panic |
| **失败处理** | 若报 `http-server` 未找到：运行 `npm install`<br>若报 Rust 编译错误：运行 `cargo check` 查看详情<br>若窗口白屏：检查 `src/index.html` 是否能被 http-server 访问 |

---

### 测试 1.2：B站页面加载 + 登录

| 项目 | 内容 |
|------|------|
| **目的** | 验证 Webview2 能完整加载 B站，且能正常登录 |
| **步骤** | 1. 应用启动后，窗口自动跳转到 bilibili.com<br>2. 等待 B站首页加载完成<br>3. 点击右上角"登录"<br>4. 用手机 B站 App 扫码登录 |
| **预期结果** | 1. 页面加载完整（轮播图、推荐视频、导航栏正常显示）<br>2. 登录后右上角显示用户头像<br>3. **页面顶部出现蓝色调试工具栏**（RAM-Stream 标志 + 两个按钮）<br>4. 工具栏状态显示"就绪 — 等待视频页面"或"PlayInfo已捕获" |
| **失败处理** | 若工具栏不出现：检查终端是否有 `[RAM-Stream] Preload script injected` 日志<br>若页面加载慢/报错：检查网络，B站可能触发人机验证<br>若 CSP 拦截脚本：检查 `tauri.conf.json` 中 `csp: null` 是否生效 |

---

### 测试 1.3：Cookie 提取（非会员基础验证）

| 项目 | 内容 |
|------|------|
| **目的** | 验证 Cookie 提取命令工作正常 |
| **步骤** | 1. 确保已登录 B站<br>2. 点击调试工具栏上的 **🍪 提取Cookie** 按钮<br>3. 查看终端输出 |
| **预期结果** | 1. 终端打印 `[Cookie] Total cookies received: N`（N > 5）<br>2. 若登录成功，应至少看到 `bili_jct`, `buvid3`, `buvid4`, `dedeuserid` 等关键 cookie<br>3. **SESSDATA 大概率缺失**（HttpOnly cookie），终端会打印警告<br>4. 日志中 cookie value 被截断显示（安全措施） |
| **失败处理** | 若显示 0 个 cookie：确认已登录，刷新页面重试<br>若 SESSDATA 缺失：这是预期行为，Sprint 2 用 CookieManager API 解决 |

---

### 测试 1.4：PlayInfo 提取（__playinfo__ 轮询）

| 项目 | 内容 |
|------|------|
| **目的** | 验证能自动捕获 B站视频的流信息 |
| **步骤** | 1. 登录后，在地址栏输入任意 B站视频网址<br>2. 观察调试工具栏右侧的状态文字<br>3. 等待状态从"等待 __playinfo__..."变为"✅ 就绪 \| video tracks: N, audio: M"<br>4. 点击 **📡 提取PlayInfo** 按钮<br>5. 查看终端输出 |
| **预期结果** | 1. 打开视频页后，**几秒内**自动检测到 `__playinfo__`<br>2. 终端打印 `[PlayInfo] PlayinfoSummary { ... }`<br>3. `video_qualities` 数组包含多个画质选项<br>4. 至少应有 id=80 (1080p)<br>5. 若是大会员 + 4K视频，还应包含 id=120 (4K) 和可能的 id=125 (4K HDR)<br>6. `has_dash: true` |
| **失败处理** | 若始终显示"等待"：确认当前是视频详情页（URL 含 `/video/BV...`），刷新重试<br>若终端报 JSON 解析错误：B站 API 可能已变更，检查 `__playinfo__` 结构<br>若显示 `has_dash: false`：可能是老旧视频只有 FLV 格式，属于正常降级 |

---

### 测试 1.5：PlayInfo 提取（fetch hook 备份方案）

| 项目 | 内容 |
|------|------|
| **目的** | 验证 Method A（网络拦截）能截获 playurl API 响应 |
| **步骤** | 1. 打开一个 B站视频页<br>2. 打开终端，观察 `[PlayURL Intercepted]` 日志 |
| **预期结果** | 1. B站播放器请求 playurl API 时，自动触发拦截<br>2. 终端显示 `[PlayURL Intercepted] url=... | PlayinfoSummary { ... }`<br>3. fetch hook 截获的数据与 `__playinfo__` 数据高度一致（都是 dash 流信息） |
| **失败处理** | 若未触发：刷新页面，B站播放器初始化时会请求 playurl<br>若 JSON 解析失败：playurl 响应格式可能不同，详见日志 |

---

### 测试 1.6：大会员 4K/HDR 流验证（关键测试）

| 项目 | 内容 |
|------|------|
| **目的** | 验证大会员权限下能检测到 4K 和 HDR 流 |
| **步骤** | 1. 用大会员账号登录<br>2. 打开已知有 4K 资源的视频（搜索"4K 测试视频"）<br>3. 点击 **📡 提取PlayInfo**<br>4. 检查终端输出的 `quality_descriptions` 数组 |
| **预期结果** | 1. 列表中应包含：<br>   - `"4K HEVC/H.265 (xx.xMbps)"` — id=120<br>   - `"4K HDR HEVC/H.265 (xx.xMbps)"` — id=125（若视频支持 HDR）<br>   - `"1080p60+ AVC/H.264 (xx.xMbps)"` — id=112<br>2. 若只有 1080p 及以下 → 账号可能无大会员，或该视频无 4K 源 |
| **失败处理** | 若无 4K 选项：确认账号是大会员、确认该视频确实有 4K 源、在普通浏览器中验证 |

---

### 测试 1.7：SPA 导航检测（视频间切换）

| 项目 | 内容 |
|------|------|
| **目的** | 验证从推荐/相关视频点击跳转时，能重新捕获 __playinfo__ |
| **步骤** | 1. 打开一个视频（等待 PlayInfo 就绪）<br>2. 在右侧推荐列表点击另一个视频<br>3. 观察工具栏状态变化 |
| **预期结果** | 1. 点击后，状态短暂变为"等待 __playinfo__..."<br>2. 新视频加载完成后，状态重新变为"✅ 就绪"<br>3. 终端打印新的 PlayinfoSummary（新的视频流信息） |
| **失败处理** | 若状态不更新：SPA 路由检测依赖 URL 轮询（500ms），确认 URL 确实变了 |

---

### 测试 1.8：非视频页降级

| 项目 | 内容 |
|------|------|
| **目的** | 验证在非视频页（首页/动态/直播）不会报错 |
| **步骤** | 1. 在 B站首页停留 30 秒<br>2. 打开动态页、直播页 |
| **预期结果** | 1. 30 秒后状态显示"⚠️ __playinfo__ 超时 (可能非视频页)"<br>2. 应用不崩溃、不卡死<br>3. 工具栏按钮仍可点击（但提取不到数据） |
| **失败处理** | 若应用崩溃：检查终端 panic 日志 |

---

### Sprint 1 通过标准

所有以下条件满足，Sprint 1 视为完成：

- [x] 应用能启动，窗口正常显示
- [ ] B站页面完整加载，可登录
- [ ] Cookie 提取命令返回非空列表
- [ ] 打开视频页后，10 秒内自动捕获 `__playinfo__`
- [ ] PlayInfo 中包含 `dash.video` 和 `dash.audio` 数组
- [ ] 大会员账号下能看到 4K 流（id=120）
- [ ] fetch hook 拦截到 playurl 响应
- [ ] SPA 页面切换后能重新捕获
- [ ] 非视频页正常降级，不崩溃
- [ ] 应用关闭无残留进程

---

## Sprint 2：流拦截与 libmpv 播放（后续补充）

Sprint 2 测试将在 Sprint 1 通过后编写，主要验证：
- libmpv 成功播放 B站 DASH 流（视频+音频同步）
- 403 防盗链已解决（Cookie 透传验证）
- RAM 缓冲生效（任务管理器内存增长至 2GB）
- 进度条拖拽秒响应
- 断网缓冲保护（断网 10 秒视频不卡）
- HDR 色彩正常（HDR/SDR 显示器均测试）
