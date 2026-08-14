<div align="center">

# CLIHub

**A modern, elegant multi-session terminal aggregator built for the AI era.**

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Built%20with-Rust-orange.svg)](https://www.rust-lang.org/)

</div>

**[English](README.md)** | [简体中文](README.zh-CN.md)

---

![CLIHub Screenshot](Example.png)

---

CLIHub is a modern desktop terminal aggregator purpose-built for the AI era. It unifies your scattered command-line tools — especially AI-powered ones like **OpenAI Codex CLI**, **Claude Code**, **Oh My PI**, and more — into a single, beautifully crafted application.

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
  - **OS-level smart theme sync**: Adapts Light/Dark mode across the entire UI, injects POSIX-standard `TERM`, `LANG`, `COLORFGBG` variables into PTY, and responds to `OSC 11` color queries — so AI tools and Oh My PI automatically match your current theme.
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

### Releases & Changelog

#### v0.1.2
- **Native Clipboard & Selection Copying**: Win32 direct clipboard API integration, QuickEdit auto-copy on mouse release, right-click actions, and standard terminal shortcuts.
- **Background Focus Protection**: Strictly ignores external shortcuts when the app is in the background to prevent accidental process termination.
- **Process Exit Loop Fix**: Fixed infinite frame spam of `[process exited]` on child termination.
- **Multi-Architecture Binaries**: Native precompiled releases for Windows x64, x86 (32-bit), and ARM64.

#### v0.1.1
- **Smart Path Truncation**: Compresses long directories with middle ellipses and `~` with full path tooltips on hover.
- **Drag-and-Drop Reordering**: Fixed upward drag slot calculation and layout cursor offset issues.
- **Multi-Architecture Support**: Precompiled binaries for Windows x64, x86, and ARM64.

---

## License

[MIT](LICENSE)
