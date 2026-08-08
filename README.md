<div align="center">

# CLIHub

**A modern, elegant multi-session terminal aggregator built for the AI era.**

*将多个 AI 命令行工具统一聚合到一个美观的桌面应用中。*

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Built%20with-Rust-orange.svg)](https://www.rust-lang.org/)

</div>

---

![CLIHub Screenshot](Example.png)

---

## English

CLIHub is a modern desktop terminal aggregator purpose-built for the AI era. It unifies your scattered command-line tools — especially AI-powered ones like **OpenAI Codex CLI**, **Claude Code**, **Oh My Posh**, and more — into a single, beautifully crafted application.

Powered by a robust PTY engine and Alacritty-grade character grid rendering, CLIHub delivers a silky-smooth, full-featured true-color terminal experience wrapped in a premium, native-feeling UI.

### Why CLIHub?

In today's AI-driven development workflow, developers often juggle multiple AI assistants and CLI tools simultaneously. Traditional terminals fall short in multi-instance switching, visual aesthetics, and intuitive operation.

CLIHub solves this by offering:

- **Centralized Management** — Save your frequently used commands (AI agents or regular environments) as persistent Sessions. One click to launch or switch — no more hunting for terminal windows.
- **Premium Aesthetics** — macOS-inspired frameless window design with frosted glass textures, hover animations, and a fully adaptive multi-theme system built on Egui.
- **Hardcore Foundation** — Perfect compatibility with any modern CLI program requiring true-color and specialized environment injection.

### Key Features

- 🖥️ **Powerful Terminal Engine** — Built on `alacritty_terminal`, supporting full ANSI escape sequences, true-color (256-color / Truecolor), bold/underline, wide characters (CJK), and cursor states.
- 📦 **Multi-Session & Multi-Tab Management**
  - Sidebar for centralized session management with persistent background processes.
  - Each session supports multiple parallel Tabs for seamless edit-run-monitor workflows.
  - Drag-and-drop reordering and inline editing for sessions and tabs.
- 🎨 **Top-Tier Visual Experience**
  - Immersive frameless window with smooth drag and resize.
  - Independent color system with classic schemes: Campbell, One Half, Solarized, Tango, and more.
  - **OS-level smart theme sync**: Adapts Light/Dark mode across the entire UI, injects POSIX-standard `TERM`, `LANG`, `COLORFGBG` variables into PTY, and responds to `OSC 11` color queries — so AI tools and Oh My Posh automatically match your current theme.
- 💾 **Instant Persistence** — All sessions, color preferences, and theme modes are saved in real-time and restored on next launch.

### Usage

#### Adding & Editing Sessions
1. Click the **`+`** button in the sidebar to open the "Add Session" panel.
2. Enter a **Name**, **Command** (e.g., `codex`, `claude`, `omp`), and an optional **Working directory**.
3. Click to launch or switch; hover to edit or drag to reorder.

#### Tabs
- Click **`+`** next to the tab bar to spawn a new tab within the current session context.
- Hover over a tab to close it with `×`.

#### Theme & Appearance
- Click **Settings** next to the CLIHub logo.
- Switch between **Light / Dark** global themes.
- Choose a **Terminal Color Scheme** or customize Background, Foreground, and Sidebar Card Color via the color pickers.

### Tech Stack

| Layer | Technology |
|:------|:-----------|
| GUI Framework | `eframe` / `egui` 0.35 |
| Terminal Engine | `alacritty_terminal` 0.26 + `vte` parser |
| PTY Backend | `portable-pty` 0.9 (Windows ConPTY / Unix) |
| Concurrency | `crossbeam-channel` (lock-free data forwarding) |
| OS Integration | Windows `GetUserDefaultLocaleName` API for locale injection |

### Build & Run

Requires the Rust toolchain (`rustup` / `cargo`).

```bash
git clone https://github.com/Darastas/CLIHub.git
cd CLIHub
cargo run              # Development mode
cargo build --release  # Release build
```

---

## 中文

CLIHub 是一款专为 AI 时代打造的现代化多屏终端聚合神器，拥有极简、优雅的图形化操作界面。它能够将你散落在各处的命令行工具（特别是像 **OpenAI Codex CLI**、**Oh My Posh**、**Claude Code** 等 AI 命令行工具）统一聚合到一个美观的桌面应用中。

它不仅拥有媲美原生系统的现代 UI 设计和动效，还通过底层的强力 PTY 引擎和 Alacritty 级别的硬件加速字符网格渲染，带给你极其丝滑、全功能的真彩色终端体验。

### 为什么使用 CLIHub？

在 AI 爆发的今天，开发者往往需要同时开启多个 AI 助手或命令行工具。传统的终端虽然强大，但在多实例切换、界面美观度和操作直觉上往往有所欠缺。

CLIHub 就是为了解决这一痛点而生：
- **集中管理** — 把常用命令（无论是 AI 代理还是常规环境）作为 Session 保存，一键呼出，不再需要满屏幕找终端窗口。
- **高级质感** — 基于 Egui 打造的 macOS 级优雅无边框窗口设计，自带精雕细琢的毛玻璃质感、悬浮动效和多主题自适应系统。
- **硬核底座** — 完美兼容任何需要真彩色和特殊环境注入的现代 CLI 程序。

### 核心特性

- 🖥️ **强大的终端内核** — 基于 `alacritty_terminal` 解析引擎，支持完整的 ANSI 转义、真彩色（256-color / Truecolor）、粗体/下划线、宽字符（CJK）以及光标状态。
- 📦 **多会话与多标签页管理**
  - 侧边栏集中管理所有会话，后台持久运行，切换如丝般顺滑。
  - 单个会话支持平行拓展多个 Tab，方便实现"编辑-运行-监控"的一体化流转。
  - 支持会话和标签页的拖拽排序与编辑。
- 🎨 **顶级视觉体验**
  - 沉浸式无边框窗口设计，流畅的阻尼拖拽与缩放。
  - 深度定制的独立色彩系统（Campbell、One Half、Solarized、Tango 等多种经典终端配色方案）。
  - **OS 级智能主题跟随**：完美处理 Light / Dark 深浅色模式，底层智能注入 `TERM`、`LANG`、`COLORFGBG` 等 POSIX 环境变量，并响应 `OSC 11` 颜色查询，让 AI 工具和 Oh My Posh 自适应你的当前主题。
- 💾 **即开即用的持久化** — 所有 Session、配色偏好、明暗模式都会实时写入配置文件，下一次打开一切依旧。

### 如何使用

#### 添加与编辑会话
1. 在左侧边栏点击 **`+`** 按钮，打开"添加会话"面板。
2. 输入 **Name（别名）**、**Command（命令）**，以及可选的 **Working directory（工作目录）**。
3. 点击即可启动或切换；悬浮可编辑；按住拖拽可排序。

#### 标签页操作
- 选中任意会话后，点击终端区域顶部标签栏旁的 **`+`** 即可新建标签页。
- 鼠标悬停标签卡可关闭（`×`）。

#### 主题与外观自定义
- 点击左上角 CLIHub Logo 旁的 **Settings**。
- 切换 **Light / Dark** 全局主题，选择 **Terminal Color Scheme**，或通过拾色器自定义背景、前景和卡片高亮色。

### 构建与运行

需要安装 Rust 工具链 (`rustup` / `cargo`)。

```bash
git clone https://github.com/Darastas/CLIHub.git
cd CLIHub
cargo run              # 开发模式
cargo build --release  # 编译发布版本
```

---

## License

[MIT](LICENSE)
