# CLIHub v1.3.0 Release Notes

## 新增功能与改进

- 📸 **多模态附件暂存区 (Multimodal Staging Area)**：
  - 支持截图智能粘贴与任意外部图片文件拖拽注入，自动暂存为缩略图胶囊卡片；
  - 接入基于文件头特征魔数（Magic Number）的深度嗅探与全格式解码（PNG / JPEG / WebP / BMP / GIF / ICO / TIFF），即便无扩展名也能 100% 准确识别与渲染；
  - **一键回车智能联动**：在暂存区有待发图片且终端正在输入文本时，按下回车键自动将图片临时转存路径与终端正在输入的文本一同注入并触发一次发送，无需按两次回车；
  - 支持单张或批量图片全屏模态 Lightbox 放大预览与一键删除。
- ⚙️ **附件暂存区 3 种布局自由切换**：
  - **右上角 HUD (`TopRight`)**：终端右上角半透明微缩挂件，视线自然不遮挡正文（默认推荐）；
  - **顶部横向槽 (`TopBanner`)**：终端顶部全宽横向卡槽，支持多图横向平铺展示；
  - **右下角悬浮 (`BottomRight`)**：右下角经典浮动胶囊。
- 🎨 **偏好设置页面纯正卡片重构**：
  - 100% 照搬 Workspaces 侧边栏卡片代码规范，纯净半透明圆角填充，彻底去除冗余发光描边与指示灯；
  - 采用大尺寸卡片同比例双层立体漫反射投影（`2.5px` + `1.2px`），告别扁平感；
  - 配色方案重构为定制卡片式选择器（`Campbell ▼`）与暗色圆角浮层选单；
  - 独立精致外框拾色卡片，上下两排网格像素级垂直无缝对齐。
- 📝 **新建与编辑会话模态窗【阴阳刻】虚实光影升级**：
  - **阴刻凹槽输入框 (Debossed Sunken Trench)**：顶部背光内阴影（Top Inset Shadow）呈现向内深陷的下坠深度，底部槽口迎光发丝微亮边（Bottom Lip Highlight），内嵌无边框流式排版；
  - **阳刻卡片按键 (Raised Card Buttons)**：双层立体落差投影 + 用户主题色半透明强调，与深凹输入框形成鲜明虚实对比。
- 💎 **全新纯透明底与大幅饱满应用图标**：
  - 彻底去除原有黑色方框底色，转换为平滑 Alpha 透明通道并消除边缘黑边；
  - 主体图形等比大幅放大（从 154px 放大至 238px，占 256x256 画布 93%），饱满醒目；
  - 重构生成包含 32 位 BGRA DIB 矩阵与 1-bit AND 透明掩码的标准 Windows 多分辨率 ICO（256/128/64/48/32/24/16），彻底修复 Windows 任务栏与资源管理器黑底问题。
- 🧩 **高内聚低耦合模块化架构**：
  - 严格遵守单个文件 `< 500` 行规范，模块化抽离 `src/ui/settings.rs`（374 行）、`src/ui/session_modal.rs`（286 行）与 `src/ui/image_preview/`。

## 预编译包校验码 (SHA-256)

| 文件名 | 架构 | SHA-256 校验码 |
|:---|:---|:---|
| clihub-v1.3.0-windows-x64.exe | Windows x64 | 030A0FECACE6E2035A23DECFD06FB9ACC537BC6C598DD59512F8AB4EC066381D |
| clihub-v1.3.0-windows-x64.zip | Windows x64 Zip | EAC736A30678FC6BBDDB7C1F2C9C1088442C2312DBE133785CD2FFCE51CEA641 |
| clihub-v1.3.0-windows-x86.exe | Windows x86 (32-bit) | 5CAFC6A1582628DA33B84A5DE025E71107BFB3011FD10B496317A1715F517771 |
| clihub-v1.3.0-windows-x86.zip | Windows x86 (32-bit) Zip | CF1ECBF5920EFB0339ED40E4DCFFDE0EBC3CFE022D62CDE5BE15C3F8F210D65B |
| clihub-v1.3.0-windows-arm64.exe | Windows ARM64 | F809673820FF1ABAF8B7BF1E9F125182907F47A0D799CFB6E59188E6E8774F76 |
| clihub-v1.3.0-windows-arm64.zip | Windows ARM64 Zip | 38AA299892E1DA5F36CB36F3CA0BFB7BECA5405CF0CF92501438DE3A409747AD |
