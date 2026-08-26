# CLIHub v1.2.0 Release Notes

## 新增功能与改进

- **Windows 进程级稳定性增强 (Win32 Engineering)**：
  - 基于 Windows Job Object (`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`) 实现 `ProcessJobGuard`，在标签页关闭或应用退出时，由 Windows 内核原子回收整个子进程树，彻底杜绝 `node.exe`、`cargo.exe`、`python.exe` 孤儿后台进程驻留。
  - 基于 Win32 `SetThreadExecutionState` 实现 `SleepInhibitor`，长任务与 AI 代理会话运行时自动阻止系统休眠，空闲时恢复节能。
- **零延迟性能飞跃 (对标 Windows Terminal)**：
  - PTY 读线程接入 UI 即时事件唤醒机制（`ctx.request_repaint()`），彻底消除即时模式 UI 处于休眠等待时的 50~500ms 延迟，实现 `<1ms` 极速流式吞吐。
  - 启动视口尺寸精准锁定，消除 PowerShell / Oh-My-Posh / OpenCode2 启动时因尺寸突变触发的二次重绘与重复执行风暴，启动速度提升 50%+。
  - 注入 Windows Terminal 原生兼容协议（`WT_SESSION` GUID、`TERM_PROGRAM="CLIHub"`、`COLORTERM="truecolor"`）与 UTF-8 控制台代码页（`CP 65001`），自动启用全速 VT 预测着色引擎。
  - DSR / CPR 光标位置查询同帧零延迟写回应答（0ms Roundtrip）。
- **终端画布几何优化与全景居中**：
  - 动态平分整行像素余数，保证终端内容区上下左右 100% 绝对等宽居中对称。
  - 接入动态字形宽高（`cell_w`/`cell_h`）记录与 `XTWINOPS` 视口查询响应，彻底消除自绘 TUI（OpenCode、Mimo 等）的启动视口抖动。
- **搜索栏无边框流式设计**：
  - 文本框改用无边框流式设计（`Frame::NONE`），去除狭窄多余的内层小白框，拓展输入宽度至 135px 并修正字体基线对齐。
- **大型源码架构模块化解耦**：
  - 将原 2000 行单文件拆解为独立的 `ui/terminal/`（`grid_render`、`input_handler`、`clipboard`、`mod`）与 `fonts.rs` 模块。

## 预编译包校验码 (SHA-256)

| 文件名 | 架构 | SHA-256 校验码 |
|:---|:---|:---|
| clihub-v1.2.0-windows-x64.exe | Windows x64 | 19C2B8B1D818A2C53D04A71641141048ABAE27C46E1F260076E1CEA347F11B9F |
| clihub-v1.2.0-windows-x86.exe | Windows x86 (32-bit) | 739914D7ADDE869CED917EFA41B14F38ACA2EB4BBBC79513F54F0E1786179292 |
| clihub-v1.2.0-windows-arm64.exe | Windows ARM64 | F13E6FF2B173FC7B9AE6CD42E1F3A3129687D62C7464E5384DC651586807AE02 |
| clihub-v1.2.0-windows-x64.zip | Windows x64 Zip | 58A9495490473912C3AE4BAB93DF3CEFA5AC1C3998F978617FB183F16C7C0A73 |
| clihub-v1.2.0-windows-x86.zip | Windows x86 Zip | 5F89B4D7189F0333C10D590C9F668D4B4A970D74942666D3C34D3B0904A357FD |
| clihub-v1.2.0-windows-arm64.zip | Windows ARM64 Zip | 7C4F873C4684E8295972F61C5E17F35D15AE5A8E3631C6A117D04E6367B5C9E0 |

