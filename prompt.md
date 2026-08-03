# AI CLI 聚合工具桌面端规划方案

## 1. 项目概述
本项目旨在开发一款跨平台的桌面应用程序，将多个 AI 命令行工具（如 Claude CLI、OpenAI Codex CLI、Cursor 等）聚合在一个统一的图形界面中。左侧提供便捷的侧边栏进行 CLI 实例的管理和切换，右侧提供一个现代化的终端（TUI）窗口与各个 AI CLI 进行交互。

## 2. 核心功能
*   **多实例管理 (Sidebar)**：左侧边栏支持列出所有已配置的 AI CLI 工具及其当前工作目录 (CWD)。支持快速点击切换，保持后台运行。
*   **终端模拟器 (Terminal Emulator)**：右侧提供一个高性能的终端界面，能够捕获并展示对应 CLI 的标准输入和输出 (stdin/stdout/stderr)。
*   **配置管理**：允许用户自定义添加不同的 CLI 启动命令、环境变量、工作目录等。
*   **现代 UI/UX**：提供类似 macOS 原生应用的高级视觉体验（如示例图所示），支持无边框窗口拖拽与快捷键操作。

## 3. UI/UX 设计 (参考 Example.png)
*   **整体布局**：
    *   **左侧边栏 (Sidebar)**：
        *   列表项：主标题为 CLI 名称（如 Codex、Claude），副标题为工作目录路径（如 `~/anneal`）。
        *   交互状态：当前选中项有明显的背景高亮（如浅灰色圆角矩形），悬浮时有交互反馈。
    *   **右侧主区域 (TUI Pane)**：
        *   采用纯白或极简背景，文字排版清晰。
        *   核心终端区：用于执行和显示 CLI 的交互内容，光标支持闪烁等原生终端特性。
        *   底部提示栏：提供类似 `? for shortcuts · ← for agents` 的快捷操作引导信息。
*   **配色/字体**：采用极简的白/浅灰底色，清晰的黑/灰文字。建议使用现代无衬线字体 (如 Inter) 结合等宽字体 (如 JetBrains Mono 或 Fira Code) 来显示终端内容。

---

## 4. 技术栈选择方案一：Web 前端架构 (快速迭代/高保真UI)
如果你希望使用成熟的 Web 技术快速实现复杂的 UI 设计和动画，这是首选方案：
*   **桌面框架**：
    *   **Tauri + Rust** (推荐)：体积较小，性能高。Rust 侧处理底层 PTY 进程管理，前端负责展示。
    *   **Electron + Node.js**：最成熟的终端应用框架（如 Tabby, Hyper 都用此技术），可直接使用 `node-pty` 库。
*   **前端组件**：**React** 或 **Vue3** + **TailwindCSS**。
*   **终端引擎**：**`xterm.js`**。这是业界标准的终端模拟器库，能够完美解决终端字符渲染、ANSI 颜色解析、交互等所有脏活累活。

---

## 5. 技术栈选择方案二：纯 Rust 架构 (极致性能/极低内存)
如果你希望彻底脱离浏览器内核（Chromium 或系统 WebView），追求极低内存占用、单可执行文件，并享受 Rust 带来的极致性能，可以选择此方案。

### 5.1 推荐依赖库 (Cargo.toml)
*   **GUI / 渲染层**：
    *   `egui` = "0.27" & `eframe` = "0.27" —— 核心即时渲染 GUI 框架。底层硬件加速，轻量且极快。
*   **底层终端与 PTY 管理**：
    *   `portable-pty` = "0.8" —— 跨平台伪终端库（Wezterm 使用），负责在后台派生并接管 CLI 子进程。
    *   `alacritty_terminal` = "0.22" —— 负责解析 PTY 输出流中的 ANSI 颜色和光标控制符，维护内存中的字符网格状态。
*   **并发控制与状态通信**：
    *   `tokio` —— 异步运行时，专门用来放在后台死循环读取 PTY 流，防止卡死 UI。
    *   `crossbeam-channel` —— 用于 UI 主线程与后台 PTY 线程安全传递消息。
*   **工具库**：
    *   `serde` & `directories` —— 用于处理本地配置的序列化和跨平台路径读取。

### 5.2 纯 Rust 工程结构设计 (Project Layout)
为了实现纯 Rust 方案下的“数据与表现分离”，推荐如下工程目录：
```text
ai-cli-hub/
├── Cargo.toml
├── src/
│   ├── main.rs            # 程序入口，初始化 tokio 和 eframe 窗口。
│   ├── app.rs             # 实现 App trait，串联起 UI 渲染层与后台数据层。
│   │
│   ├── ui/                # 视图渲染层 (仅使用 egui 绘制界面)
│   │   ├── mod.rs
│   │   ├── sidebar.rs     # 左侧列表栏 (处理多开实例的切换和点击样式)
│   │   ├── terminal.rs    # ★核心：将 backend 提供的字符网格转化为 egui 富文本并绘制
│   │   └── titlebar.rs    # 自定义无边框窗口，处理 macOS 风格按钮和拖拽
│   │
│   ├── backend/           # 核心业务层 (处理所有后台进程流)
│   │   ├── mod.rs
│   │   ├── pty.rs         # 封装 portable-pty 处理子进程生命周期
│   │   ├── terminal.rs    # 封装 alacritty_terminal 维护终端状态
│   │   └── io_loop.rs     # 后台循环线程，读写 PTY 并通过 channel 通知 UI
│   │
│   ├── state/             # 全局状态管理
│   │   ├── mod.rs
│   │   └── session.rs     # 记录多个 CLI 的运行状态，供左侧边栏切换
│   │
│   └── config/            # 配置管理，序列化本地 CLI 列表
```

---

## 6. 开发步骤规划 (以纯 Rust 为例)
*   **Phase 1: 验证核心链路** - 在 `backend/` 下跑通 `portable-pty` + `alacritty_terminal`，确保后台能正确抓取 CLI 输出的文本。
*   **Phase 2: 界面骨架搭建** - 利用 egui 快速划分左中右布局，画出自定义的无边框标题栏。
*   **Phase 3: 终端渲染器实现 (难点)** - 把内存里的字符矩阵绘制到右侧的 UI 面板上；并将键盘敲击转为 ANSI 序列写回 PTY 进程。
*   **Phase 4: 多实例切换支持** - 完善 `state` 状态层，使得左侧点击不同 CLI 时，右侧 UI 能瞬间切换绑定的数据流，实现多开。
*   **Phase 5: 视觉打磨** - 加载 JetBrains Mono 等宽字体，调整背景色、圆角大小，直至完美贴合 Example.png 的高级感。
