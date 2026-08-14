<div align="center">

# CLIHub

**一款专为 AI 时代打造的现代化多屏终端聚合神器，拥有极简、优雅的图形化操作界面。**

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Built%20with-Rust-orange.svg)](https://www.rust-lang.org/)

</div>

[English](README.md) | **[简体中文](README.zh-CN.md)**

---

![CLIHub 截图](Example.png)

---

CLIHub 是一款专为 AI 时代打造的现代化多屏终端聚合神器，拥有极简、优雅的图形化操作界面。它能够将你散落在各处的命令行工具（特别是像 **OpenAI Codex CLI**、**Oh My PI**、**Claude Code** 等 AI 命令行工具）统一聚合到一个美观的桌面应用中。

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
  - **OS 级智能主题跟随**：完美处理 Light / Dark 深浅色模式，底层智能注入 `TERM`、`LANG`、`COLORFGBG` 等 POSIX 环境变量，并响应 `OSC 11` 颜色查询，让 AI 工具和 Oh My PI 自适应你的当前主题。
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

### 🚀 版本更新日志 (v0.1.1)

- **边栏长路径智能精简**：自动将用户家目录替换为 `~`，对过长项目路径执行平滑的中间省略（如 `C:\...\project`），鼠标悬停实时展示完整路径。
- **重构边栏卡片拖拽排序系统**：
  - 彻底修复从下往上拖动卡片时的槽位误判与事件劫持问题。
  - 彻底拔除布局污染（`ui.put`），修复因布局光标篡改导致的卡片异常空白和断层错位。
  - 修复途经卡片触发 egui 引擎红框警告的问题，恢复原生点击拖拽交互。
- **多平台架构原生支持**：提供 Windows x64、x86 (32位)、ARM64 三架构原生 Release 可执行文件。

---

## License

[MIT](LICENSE)
