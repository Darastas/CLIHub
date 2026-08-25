# CLIHub v1.1.0 Release Notes

## 新增功能与改进

- 单 CLI 全景多标签微缩看板：会话右键菜单新增“浏览全部窗口”功能，支持一屏全景微缩预览当前 CLI 的所有活跃标签页，并在看板顶部提供返回全景看板、统计微徽章与新建标签快捷操作，支持按 Esc 级联返回。
- 侧边栏 WORKSPACES 工作区体系重构：将原 SESSIONS 统一重命名为 WORKSPACES，标头文本与卡片文字按 x=24px 精准纵向对齐，优化卡片间距与整体排版比例。
- Ctrl+C 双击退出提示重构与底边对齐：将退出提示卡片移至侧边栏底部，与右侧终端下边框实现像素级绝对对齐，并引入 150ms 平滑渐显动效与纯净中性微卡片样式。
- Windows 10/11 原生标准标题栏三按键：完全重构最小化、最大化/还原、关闭三按键为 Windows 官方标准风格，支持满格平铺悬浮背景、官方悬停正红色与 1.0px 发丝精度矢量线框。
- 全架构原生预编译包：提供 Windows x64、x86 与 ARM64 的独立执行文件及压缩包。

## 预编译包校验码 (SHA-256)

| 文件名 | 架构 | SHA-256 校验码 |
|:---|:---|:---|
| clihub-v1.1.0-windows-x64.exe | Windows x64 | 74FB29A954EF8D17D37E581DBB8D19C5DA98A212272DC4A84C99F4AED7AEB512 |
| clihub-v1.1.0-windows-x86.exe | Windows x86 (32-bit) | 75DBF434E1C325C220C8FFCDBD18F811CB33E91EFAF66CA85C2ABF81445D60CF |
| clihub-v1.1.0-windows-arm64.exe | Windows ARM64 | F232D28A31F56BB40C0834D518412CB9A21AD4A2A6A652C92793A01E04AF70C2 |
| clihub-v1.1.0-windows-x64.zip | Windows x64 Zip | 2E5BB84026ECB3FE73B1438AB34B2E778C25D1D656FCBEBF77756C8161663730 |
| clihub-v1.1.0-windows-x86.zip | Windows x86 Zip | F4B7B22B5DE1291CA7ACF0070EEF1EF71B5A137C9F0835B1134C32D23E6D4651 |
| clihub-v1.1.0-windows-arm64.zip | Windows ARM64 Zip | DA213ED6E443671EDE456DC2A86D6FFF204097521D9BC26C49A9BDD6784E0A12 |
