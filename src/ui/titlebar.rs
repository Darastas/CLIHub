//! 自定义无边框标题栏：左侧 macOS 风格圆钮（红/黄/绿）+ 拖拽 + 双击最大化。
//!
//! 窗口以 `ViewportBuilder::with_decorations(false)` 打开；Windows 下
//! 保持 `resizable` 时仍保留隐形缩放进边，无需手动实现 resize。

use egui::{Align2, Color32, FontId, Id, Pos2, Rect, Sense, Ui, vec2};
use egui::ViewportCommand;

pub fn show(ui: &mut Ui) {
    let (rect, resp) = ui.allocate_exact_size(
        vec2(ui.available_width(), 38.0),
        Sense::click_and_drag(),
    );

    // 背景（与面板同色）
    ui.painter().rect_filled(rect, 0.0, ui.visuals().panel_fill);

    // 拖拽窗口
    if resp.drag_started() {
        ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
    }
    // 双击标题栏 -> 最大化/还原
    if resp.double_clicked() {
        let maxed = ui.input(|i| i.viewport().maximized == Some(true));
        ui.ctx().send_viewport_cmd(ViewportCommand::Maximized(!maxed));
    }

    // 应用名（按钮右侧，垂直居中）
    ui.painter().text(
        Pos2::new(rect.min.x + 76.0, rect.center().y),
        Align2::LEFT_CENTER,
        "AI CLI Hub",
        FontId::proportional(12.5),
        Color32::from_gray(120),
    );

    // 左侧 macOS 风格圆钮：红(关闭) / 黄(最小化) / 绿(最大化)
    let radius = 6.0;
    let y = rect.center().y;
    let spacing = 20.0;
    let start_x = rect.min.x + 16.0;
    let colors = [
        Color32::from_rgb(255, 95, 86),   // red
        Color32::from_rgb(255, 189, 46),  // yellow
        Color32::from_rgb(39, 201, 63),   // green
    ];

    for (i, color) in colors.iter().enumerate() {
        let center = Pos2::new(start_x + i as f32 * spacing, y);
        let hit = Rect::from_center_size(center, vec2(22.0, 22.0));
        let btn = ui.interact(hit, Id::new(("titlebar-btn", i)), Sense::click());
        ui.painter().circle_filled(center, radius, *color);
        if btn.hovered() {
            ui.painter().circle_stroke(
                center,
                radius + 1.5,
                egui::Stroke::new(1.0, Color32::from_gray(160)),
            );
        }
        if btn.clicked() {
            match i {
                0 => ui.ctx().send_viewport_cmd(ViewportCommand::Close),
                1 => ui.ctx().send_viewport_cmd(ViewportCommand::Minimized(true)),
                _ => {
                    let maxed = ui.input(|i| i.viewport().maximized == Some(true));
                    ui.ctx().send_viewport_cmd(ViewportCommand::Maximized(!maxed));
                }
            }
        }
    }
}
