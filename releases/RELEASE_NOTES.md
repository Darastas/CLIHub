# CLIHub v1.4.0 Release Notes

## 新增功能与改进

- 🤖 **Agent Chat 多智能体协作工作流 (Multi-Agent Collaboration Engine)**：
  - **原生嵌入式多终端卡片**：在工作流中为每个参与角色独立分配基于 Windows ConPTY 的真实交互进程，Alacritty 状态机 + egui 渲染，卡片化并排展示，支持独立输入聚焦、滚轮查看历史与选区复制。
  - **下沉式流式指令输入框 (Sunken Composer)**：延续阴阳刻极简美学，流式排版、动态自适应高度与圆角聚焦发光，支持快捷选择首位执行者、一键派发或直接回复。
  - **全自动信箱交接协议 (Mailbox Protocol & CLI Tool)**：随附 `clihub-agent.exe` 独立信箱 CLI 工具与文件协议（`inbox`、`send`、`list`），AI Agent 能够自主拉取任务、接收上下文、提交任务产物并触发自动下一棒交接，无需人工手动搬运。
  - **MCP Server 兼容网桥 (Model Context Protocol)**：内置标准 Model Context Protocol (MCP) 服务器实现，为兼容的 AI 客户端提供直接的信箱工具调用接口。
- 🎨 **细节美学与交互体验精修**：
  - **输入法与打字焦点冲突彻底修复**：重构键盘事件捕获逻辑，文本输入框打字时严格独占输入焦点，彻底解决终端或选区抢占焦点的跳变问题。
  - **侧边栏极简高质感瘦身**：移除左上角多余的新建按钮，去除侧边栏底部的空态占位提示，保留最高密度的沉浸式极简质感。
  - **搜索栏动态平滑过渡动画**：按下 `Ctrl+F` 呼出/关闭搜索栏时拥有平滑过渡与高质感无边框流式输入框，全面对齐终端原生视觉风格。
- 📦 **全架构原生预编译包 (Windows x64 / x86 / ARM64)**：
  - 针对 Windows x64、x86 (32位) 以及 ARM64 平台提供独立可执行文件及附带 `clihub-agent.exe` 协同工具的发布压缩包。

## 预编译包校验码 (SHA-256)

| 文件名 | 架构 | SHA-256 校验码 |
|:---|:---|:---|
| clihub-v1.4.0-windows-x64.exe | Windows x64 | 126465358CCB6EBF2B0228BB3D3E27C0841DCE67392128B684471DB88D62D129 |
| clihub-v1.4.0-windows-x64.zip | Windows x64 Zip | 96FB772B522303B325421EBF1F1E5786F597FA028C3DDC8615AF956EB0ECDBB2 |
| clihub-v1.4.0-windows-x86.exe | Windows x86 (32-bit) | 057FB7617011D987250787871E791EC8670B19B97708E443DE17F67EA99740E6 |
| clihub-v1.4.0-windows-x86.zip | Windows x86 (32-bit) Zip | 81BD3450140AEDF81FC0730FDC5F26B45102923C084AA9247F15CBBFCAA100EE |
| clihub-v1.4.0-windows-arm64.exe | Windows ARM64 | D74FCFE0FF6F303A2471E6FF46E32C5E62478600F46FD0B51EAFE872B03B736B |
| clihub-v1.4.0-windows-arm64.zip | Windows ARM64 Zip | 2EBAB2E8F9A75A68533DEFF6F472883891988B6EF1DDE6DE3AA28700781BE9C5 |
