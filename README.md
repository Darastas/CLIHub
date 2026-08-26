<div align="center">

# CLIHub

**A modern, elegant multi-session terminal aggregator built for the AI era.**

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Built%20with-Rust-orange.svg)](https://www.rust-lang.org/)

</div>

**[English](README.md)** | [简体中文](README.zh-CN.md)

---

![CLIHub Terminal View](Example.png)

<br/>

![CLIHub Overview View](Example-Overview.png)

---

CLIHub is a modern desktop terminal aggregator purpose-built for the AI era. It unifies your scattered command-line tools — especially AI-powered ones like **OpenAI Codex CLI**, **Claude Code**, **Oh My PI**, and more — into a single, beautifully crafted application.

Powered by a robust PTY engine and Alacritty-grade character grid rendering, CLIHub delivers a silky-smooth, full-featured true-color terminal experience wrapped in a premium, native-feeling UI.

### Why CLIHub?

In today's AI-driven development workflow, developers often juggle multiple AI assistants and CLI tools simultaneously. Traditional terminals fall short in multi-instance switching, visual aesthetics, and intuitive operation.

CLIHub solves this by offering:

- **Centralized Management** — Save your frequently used commands (AI agents or regular environments) as persistent Workspaces. One click to launch or switch — no more hunting for terminal windows.
- **Global Overview & Single-CLI Tab Panoramas** — Inspect all workspaces or browse all tabs of a specific CLI in a real-time responsive multi-grid dashboard, jump into any session/tab with a click, or toggle with `Ctrl+Shift+O`.
- **Premium Aesthetics** — Dark minimalist frameless window design with subtle frosted glass accents, native Windows 10/11 caption buttons, and a fully adaptive multi-theme system built on Egui.
- **Hardcore Foundation** — Perfect compatibility with any modern CLI program requiring true-color and specialized environment injection.

### Key Features

- 🖥️ **Powerful Terminal Engine** — Built on `alacritty_terminal`, supporting full ANSI escape sequences, true-color (256-color / Truecolor), bold/underline, wide characters (CJK), and cursor states.
- 📦 **Workspaces & Multi-Tab Management**
  - Sidebar for centralized workspace management with persistent background processes.
  - Each workspace supports multiple parallel Tabs for seamless edit-run-monitor workflows.
  - Drag-and-drop reordering and inline editing for workspaces and tabs.
- 🔍 **Interactive Terminal Search** — Press `Ctrl+F` to summon the interactive search bar with case sensitivity toggles, match navigation (`▲`/`▼`), and live hit counting.
- ⊞ **Global Overview & Tab Panorama** — Bird's-eye view of all workspaces or individual CLI tabs with scaled live previews and quick actions.
- 🎨 **Top-Tier Visual Experience**
  - Immersive frameless window with smooth drag and resize.
  - Pixel-perfect native Windows 10/11 title bar caption buttons (full-bleed hover, official red close hover, and 1.0px hairline vector strokes).
  - Independent color system with classic schemes: Campbell, One Half, Solarized, Tango, and more.
  - **OS-level smart theme sync**: Adapts Light/Dark mode across the entire UI, injects POSIX-standard `TERM`, `LANG`, `COLORFGBG` variables into PTY, and responds to `OSC 11` color queries.
- 📸 **AI Multimodal Attachment Staging & Magic-Byte Decoding**
  - **Smart Screenshot Pasting & File Drop Staging**: Seamlessly paste screenshots (`Ctrl+V` / right-click) or drag external image files into any terminal; images are automatically stored in an interactive floating attachment staging capsule.
  - **Deep Magic-Byte Format Decoding**: Full header magic-number sniffing supporting PNG, JPEG, WebP, BMP, GIF, ICO, and TIFF even without file extensions.
  - **One-Key Enter Direct Submission**: Automatically bundles staged image filepaths with ongoing command text and dispatches directly on a single Enter stroke (no duplicate Enter required).
  - **3 Attachment Staging Positions**: Switch between Top-Right HUD (`TopRight`), Full-Width Top Banner (`TopBanner`), and Bottom-Right Classic (`BottomRight`) in Settings.
  - **Full-Screen Modal Lightbox**: Click any thumbnail to inspect high-resolution images in a centered dark backdrop lightbox.
  - **Auto Cache Cleanup**: Silently purges temporary images older than 24 hours on application startup to ensure zero disk bloat.
- ⚙️ **Workspaces Card-Style Settings Modal**: 1:1 rebuilt settings panel matching Workspaces sidebar card aesthetics, dual-layer diffuse shadows, borderless fills, card-style dropdown menu, and aligned swatches.
- 📝 **Session Modal with Debossed Intaglio Inputs & Raised Buttons**: Sunken trench text fields with top inset shadow and bottom lip highlight paired with raised themed card buttons.
- 💎 **Pure Transparent High-Res Application Icon**: Rebuilt 32-bit RGBA icons with 1-bit AND transparency masks and scaled up geometry (93% canvas fill) for Windows Taskbar and Explorer.
- 🛡️ **Double-Press Ctrl+C Protection** — Sidebar bottom neutral micro-card with exact terminal bottom alignment, 150ms smooth fade-in animation, and 1.8s tolerance window.
- 💾 **Instant Persistence** — All workspaces, color preferences, and theme modes are saved in real-time and restored on next launch.

### Usage

#### Adding & Editing Workspaces
1. Click the **`+`** button in the sidebar to open the "Add Workspace" panel.
2. Enter a **Name**, **Command** (e.g., `codex`, `claude`, `omp`), and an optional **Working directory**.
3. Click to launch or switch; hover to edit or drag to reorder.

#### Tabs & Overview
- Click **`+`** next to the tab bar to spawn a new tab within the current workspace context.
- Click the **`⊞`** button in the sidebar (or press `Ctrl+Shift+O`) to open the Global Workspaces Overview.
- Right-click any workspace card and select **"⊞ 浏览全部窗口 (Browse All Windows)"** to open the single-CLI multi-tab panorama view.
- Press `Ctrl+F` in any terminal tab to open the interactive search bar.

#### Theme & Appearance
- Click **Settings** next to the CLIHub logo.
- Switch between **Light / Dark** global themes.
- Choose a **Terminal Color Scheme** or customize Background, Foreground, and Sidebar Card Color via the color pickers.
- Choose your preferred **Attachment Staging Position** (Top-Right HUD / Top Banner / Bottom-Right).

### Tech Stack

| Layer | Technology |
|:------|:-----------|
| GUI Framework | `eframe` / `egui` 0.35 |
| Terminal Engine | `alacritty_terminal` 0.26 + `vte` parser |
| PTY Backend | `portable-pty` 0.9 (Windows ConPTY / Unix) |
| Concurrency | `crossbeam-channel` (lock-free data forwarding) |
| Image Decoding | `image` 0.25 (PNG, JPEG, WebP, BMP, GIF, ICO, TIFF) |
| OS Integration | Win32 Job Objects, `SetThreadExecutionState`, `GetUserDefaultLocaleName` |

### Build & Run

Requires the Rust toolchain (`rustup` / `cargo`).

```bash
git clone https://github.com/Darastas/CLIHub.git
cd CLIHub
cargo run              # Development mode
cargo build --release  # Release build
```

### Releases & Changelog

#### v1.3.0
- **AI Multimodal Attachment Staging & Intelligent Dispatch**:
  - Interactive attachment staging area with thumbnail capsules, full-screen lightbox preview, and seamless one-key Enter submission for AI CLI prompts.
  - Deep file-header magic-number sniffing supporting PNG, JPEG, WebP, BMP, GIF, ICO, and TIFF.
  - 3 customizable attachment positions (`TopRight HUD`, `TopBanner`, and `BottomRight`).
- **Card-Style Settings Interface Refactoring**: 1:1 rebuilt preference modal with Workspaces card language, dual-layer diffuse shadows, borderless selections, custom card dropdowns, and pixel-aligned color pickers.
- **Session Management Modal (Debossed & Raised Contrast)**: Sunken trench input fields featuring top inset shadows and bottom lip highlights paired with raised themed card buttons.
- **Pure Transparent High-Res App Icon**: Converted black icon backgrounds to 32-bit RGBA with 1-bit AND masks and 93% canvas fill, resolving Windows Taskbar and Explorer display issues.
- **Modular Codebase (<500 Lines per File)**: Decoupled modal views into standalone modules (`ui/settings.rs`, `ui/session_modal.rs`, `ui/image_preview/`).
- **Multi-Architecture Release Packages**: Standalone native precompiled binaries and zip archives for Windows x64, x86 (32-bit), and ARM64.
- **AI Multimodal Image Input & File Drag-and-Drop**:
  - Support instant image pasting via `Ctrl+V`, `Ctrl+Shift+V`, or secondary click; clipboard bitmaps are automatically saved as temporary PNG files and injected as safe quoted file paths for multi-modal AI CLI tools (Claude Code, OpenCode, Aider, etc.).
  - Added drag-and-drop file support with smooth semi-transparent hover feedback and automatic absolute path insertion into active terminal PTYs.
  - Automatic background garbage collection for temporary images older than 24 hours on application startup.
- **Windows Process Stability & Tree Guard (Win32 Engineering)**: Integrated Win32 Job Object (`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`) to automatically terminate full process trees (preventing orphan `node.exe`, `cargo.exe`, `python.exe` processes) and Win32 `SetThreadExecutionState` Sleep Inhibitor for background AI tasks.
- **Zero-Latency PTY Performance (Benchmarked against Windows Terminal)**: Direct UI event loop wake-up upon incoming PTY bytes (`<1ms` latency), eliminating immediate-mode UI sleep lag. Initial viewport geometry matching eliminates Oh-My-Posh / PowerShell / OpenCode2 double-render resize storms.
- **Windows Terminal Protocol Injections**: Injects `WT_SESSION` GUID and UTF-8 console code page (`CP 65001`), activating full-speed Virtual Terminal (VT) prediction and true-color rendering.
- **0ms Roundtrip DSR / CPR Writeback**: Synchronous, same-frame cursor position query response eliminates communication stalls.
- **Symmetric Canvas Centering & XTWINOPS Sync**: Dynamically balances remainder pixel margins for 100% symmetric top/bottom/left/right padding, with live font metrics synced to XTWINOPS.
- **Frameless Search Input**: Replaced the cramped inner textedit capsule with a clean, borderless fluid input field with refined baseline alignment.
- **Modular Codebase Refactoring**: Decoupled monolithic terminal code into structured modules (`grid_render`, `input_handler`, `clipboard`, `mod`, and `fonts.rs`).
- **Multi-Architecture Release Packages**: Standalone native precompiled binaries and zip archives for Windows x64, x86 (32-bit), and ARM64.

#### v1.1.0
- Single-CLI Tab Panorama Overview: Added "Browse All Windows" in workspace context menu to view and manage all active tabs of a specific CLI with scaled live previews, top toolbar, and Esc cascade navigation.
- Workspaces System & Optical Alignment: Refactored SESSIONS into WORKSPACES with strict x=24px vertical alignment matching card text, refined card spacing, and clean minimalist sidebar layout.
- Bottom Ctrl+C Notification with Smooth Fade-in: Relocated exit confirmation card to sidebar bottom with exact pixel-level terminal bottom alignment, 150ms smooth fade-in animation, and neutral monochrome glass card aesthetics.
- Native Windows 10/11 Title Bar Caption Buttons: Rebuilt minimize, maximize/restore, and close buttons to 1:1 match Windows 10/11 standards with full-bleed flat hover blocks, official red close hover, and 1.0px hairline vector strokes.
- Multi-Architecture Release Packages: Official standalone executables and zip archives for Windows x64, x86, and ARM64.

#### v1.0.0
- Global Sessions Overview: Real-time multi-grid live preview dashboard for all sessions with one-click navigation and clean vector 2x2 grid UI (Ctrl+Shift+O).
- Interactive In-Terminal Search: Integrated Ctrl+F search capsule with case-sensitivity matching, occurrence badge, and navigation controls.
- Unified Design & Top-Bar Alignment: Perfect subpixel baseline alignment across sidebar title, session buttons, tab cards, and search container.
- Refined Ctrl+C Guard: Centered typography and extended 1.8s double-press grace period.
- Native Multi-Architecture Releases: Official binaries compiled for Windows x64, x86, and ARM64.

#### v0.1.2
- Native Clipboard & Selection Copying: Win32 direct clipboard API integration, QuickEdit auto-copy on mouse release, right-click actions, and standard terminal shortcuts.
- Background Focus Protection: Strictly ignores external shortcuts when the app is in the background to prevent accidental process termination.
- Process Exit Loop Fix: Fixed infinite frame spam of [process exited] on child termination.
- Multi-Architecture Binaries: Native precompiled releases for Windows x64, x86 (32-bit), and ARM64.

#### v0.1.1
- Smart Path Truncation: Compresses long directories with middle ellipses and ~ with full path tooltips on hover.
- Drag-and-Drop Reordering: Fixed upward drag slot calculation and layout cursor offset issues.
- Multi-Architecture Support: Precompiled binaries for Windows x64, x86, and ARM64.

---

## License

[MIT](LICENSE)
