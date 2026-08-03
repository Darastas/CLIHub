# AI CLI Hub

将多个 AI CLI（Claude CLI / OpenAI Codex CLI / Cursor 等）聚合在一个桌面应用里。
左侧边栏管理并切换多实例（后台保持运行），右侧是真正可交互的终端（基于
alacritty 字符网格渲染）。

## 功能

- **多实例管理**：左侧边栏列出所有 CLI 及工作目录，点击切换；悬浮可删除，`＋` 新增；会话在后台保持运行。
- **终端模拟器**：`alacritty_terminal` 解析 PTY 字节流，egui 逐格渲染（ANSI 颜色 / 粗体 / 下划线 / 宽字符 / 闪烁光标）。
- **配置持久化**：会话列表写入平台配置目录 `ai-cli-hub/config.json`。
- **现代 UI**：无边框窗口（macOS 风格圆钮 + 拖拽）、白/浅灰极简视觉、等宽 + CJK 字体。

## 技术栈

- **GUI**：`eframe` / `egui` 0.35
- **终端状态机**：`alacritty_terminal` 0.26 + `vte` 解析器
- **PTY**：`portable-pty` 0.9
- **并发**：`crossbeam-channel`

## 构建与运行

```bash
cargo run            # 开发模式
cargo build --release
cargo test           # 后端单元测试（终端网格 / ANSI / resize）
```

> Windows 下首次运行会自动拉起 `Terminal`（cmd）会话以验证链路；
> 侧边栏里的 Claude CLI / Codex CLI 需本机已安装对应命令。

## 工程结构（对应 prompt.md 5.2）

```text
src/
├── main.rs            # 入口：无边框窗口 + eframe
├── app.rs             # App 状态：会话管理、后台拉流、标题栏与面板编排
├── ui/                # 视图层（仅 egui 绘制）
│   ├── sidebar.rs     # 左侧列表：切换 / 增删会话
│   ├── terminal.rs    # 网格渲染 + 原始按键转发 + 缩放 + 滚动
│   └── titlebar.rs    # 无边框标题栏（圆钮 / 拖拽 / 双击最大化）
├── backend/           # 核心业务层
│   ├── pty.rs         # portable-pty 子进程 + 后台 reader 线程
│   ├── terminal.rs    # alacritty 终端 + VTE 解析封装
│   └── io_loop.rs     # 每帧把 PTY 字节块喂进终端
├── state/session.rs   # 会话状态模型
└── config/mod.rs      # CLI 列表的 JSON 序列化
```

## 交互

| 操作 | 说明 |
|:---|:---|
| 点击终端 | 聚焦，开始键入 |
| 输入命令 + Enter | 发送到当前 CLI |
| `Ctrl+C` | 中断（SIGINT） |
| 滚轮 | 滚动历史缓冲 |
| 标题栏绿钮 / 双击 | 最大化 / 还原 |
