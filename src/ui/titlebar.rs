//! 自定义无边框标题栏（macOS 风格圆钮 + 拖拽）。
//!
//! Round 1 使用系统标题栏，此模块为占位；Phase 2 接入
//! `ViewportBuilder::with_decorations(false)` 后启用。

use egui::Ui;

#[allow(dead_code)]
pub fn show(_ui: &mut Ui) {
    // Phase 2: 绘制三颗圆钮（红/黄/绿）与拖拽区，处理双击放大。
}
