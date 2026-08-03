# UI 深度优化与多标签页 (Multi-Tab) 支持计划

针对您的反馈，我计划进行以下深度的架构调整与视觉优化，特别是引入类似浏览器的“多标签页”功能，并解决悬浮变色、圆角生硬以及默认配色不够协调的问题。

## User Review Required

> [!IMPORTANT]
> 引入多标签页机制属于核心架构变更。侧边栏依然是管理不同的 CLI 配置，而右侧的内容区域将变为一个“标签页组”。您可以点击 `+` 来为当前的 CLI 启动多个实例。请确认这个交互逻辑是否完全符合您的预期？另外，深色主题将回归更经典的“极客灰” (类似 VS Code / GitHub Dark)，以保证最佳的文本对比度和适应性。

## 优化方向与具体细节

### 1. 核心架构重构：多标签页 (Multi-Tab) 支持
当前 `Session` 和终端进程是 1对1 的关系。为了支持多开窗口：
*   **重构 `Session` 数据结构**：将 `Session` 中的 `terminal`, `pty`, `rx`, `alive` 等状态提取成独立的 `TerminalInstance` (或称为 `Tab`) 结构体。一个 `Session` 将持有一个 `Vec<TerminalInstance>` 以及当前激活的 `active_tab_index`。
*   **应用生命周期调整 (`app.rs`)**：`spawn_session` 将变为 `spawn_tab`，每次启动都会在对应会话中 push 一个新的进程。`update_backend` 轮询 IO 时，将嵌套遍历所有的 `Session` 以及它们底下的所有 `Tab`。

### 2. 右侧终端 UI：类似浏览器的 Tab 栏
*   **移除顶部的 Restart 按钮**（正如您提到的，没太大作用且点击逻辑有歧义）。
*   **重绘头部区域**：将顶部的“名字 + 状态”标签栏替换为一个横向滑动的 **Tab Bar**（类似 Chrome 或 VS Code）。
    *   每个 Tab 会显示序号或短名称，并带有一个独立的关闭小按钮 (`x`)。
    *   在 Tab 栏的末尾增加一个显著的 `+` 按钮，点击即可为当前 CLI 新开一个进程。
*   **切换逻辑**：点击 Tab 会切换底层终端界面的渲染目标和键盘输入目标。

### 3. 视觉与配色的深度调优
*   **侧边栏卡片优化 (Sidebar)**：
    *   **圆角与比例**：加大卡片背景的圆角，使形状更加圆润自然（如 `rounding` 提升至 10~12，调整卡片的垂直 Padding）。
    *   **Hover 色彩对齐**：修复指针悬浮时的颜色加深逻辑，改用基于当前底色的柔和 Alpha 叠加（如 `Color32::from_black_alpha(20)`），确保无论背景如何变化，悬浮色都能完美融于底色，不再突兀。
    *   **左侧选中指示条**：微调选中状态的强调线，使其更柔和。
*   **深色主题 (Dark Theme) 重新配色**：
    *   弃用之前偏蓝紫的 Catppuccin 色系。由于 CLI 文本颜色多变，偏色的背景容易造成冲突（正如截图里高对比度文字显得刺眼）。
    *   改用类似 VS Code 默认的**深灰系 (Neutral Dark)**：背景色设为 `#1E1E1E`，面板/侧边栏设为 `#252526`，边框设为 `#3C3C3C`。这种配色最中正，能够完美承载任何 ANSI 彩色输出，显得极简且专业。

## 变更文件概览

### 数据模型
*   **[MODIFY]** `src/state/session.rs`
    *   定义 `TerminalInstance` 结构。
    *   修改 `Session`，将原来的单一进程字段替换为 `pub tabs: Vec<TerminalInstance>` 和 `pub active_tab: usize`。修改 `status()` 函数聚合多 Tab 状态。

### 业务逻辑
*   **[MODIFY]** `src/app.rs`
    *   修改 `spawn_session` 逻辑，支持向 `Session` push 新的 Tab。
    *   重构 `update_backend` 以支持二维遍历（Sessions -> Tabs）处理 PTY 数据。
    *   调整 `TerminalAction` 处理逻辑，支持 `NewTab` 和 `KillTab(tab_index)`。
    *   修改 `app_visuals` 提供更协调的深灰系全局 UI 色调。

### 视图层
*   **[MODIFY]** `src/ui/terminal.rs`
    *   彻底移除顶部的原信息栏与 Restart 按钮。
    *   引入 `egui::ScrollArea::horizontal` 绘制横向的 Tab 选项卡（每个选项卡带单独的关闭按钮）。
    *   添加 `+` 按钮。
    *   重写 `TermTheme::dark()` 颜色定义为经典的中性深灰色。
*   **[MODIFY]** `src/ui/sidebar.rs`
    *   使用混合函数（如按比例混合透明黑/白）重绘 Hover 和 Selected 背景，彻底解决色偏和生硬问题，并进一步圆滑卡片圆角。

## Verification Plan

### Manual Verification
1.  **运行程序**：启动程序并选择一个深色模式。
2.  **测试侧边栏视觉**：鼠标在侧边栏各个项目上移动，验证悬浮颜色加深是否柔和自然，圆角是否舒适不生硬。
3.  **测试多标签页**：
    *   选中一个 CLI，在右侧点击顶部的 `+` 按钮，验证是否成功启动了一个新实例并出现了一个新 Tab。
    *   在两个 Tab 之间来回点击，验证终端输出和输入是否独立且正确切换。
    *   在旧 Tab 输入一段命令，然后点击 Tab 右侧的 `x`，验证进程是否被杀掉且 Tab 消失。
4.  **测试终端色彩**：确认新的极简灰底色是否让彩色文本更加清晰易读。
