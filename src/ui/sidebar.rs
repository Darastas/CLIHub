//! 左侧边栏：列出所有 CLI 会话，支持点击切换。
//!
//! 视觉参照 prompt.md：主标题 = CLI 名称，副标题 = 工作目录路径；
//! 选中项使用浅色圆角矩形高亮，悬浮有反馈。

use egui::{Align2, Color32, FontId, Pos2, Rect, RichText, Sense, Ui};

use crate::state::Session;

use super::{status_color, status_dot};

/// 展示边栏，返回被点击的会话索引（无点击则为 `None`）。
pub fn show(ui: &mut Ui, sessions: &[Session], selected: usize) -> Option<usize> {
    let mut clicked = None;

    ui.add_space(12.0);
    ui.vertical_centered(|ui| {
        ui.label(RichText::new("AI CLI Hub").strong().size(16.0));
    });
    ui.add_space(4.0);
    ui.separator();
    ui.add_space(8.0);

    ui.label(RichText::new("CLI SESSIONS").size(10.5).color(Color32::from_gray(140)));
    ui.add_space(4.0);

    for (idx, s) in sessions.iter().enumerate() {
        let is_sel = idx == selected;
        let row_rect = Rect::from_min_size(
            Pos2::new(ui.cursor().min.x, ui.cursor().min.y),
            egui::vec2(ui.available_width(), 52.0),
        );
        let resp = ui.allocate_rect(row_rect, Sense::click());

        // 背景：选中 -> 浅灰圆角矩形；悬浮 -> 更浅的灰色
        let bg = if is_sel {
            ui.visuals().selection.bg_fill
        } else if resp.hovered() {
            ui.visuals().widgets.hovered.weak_bg_fill
        } else {
            Color32::TRANSPARENT
        };
        if bg != Color32::TRANSPARENT {
            ui.painter().rect_filled(row_rect, 8.0, bg);
        }

        // 状态圆点 + 名称 + 工作目录
        let left = Pos2::new(row_rect.min.x + 12.0, row_rect.min.y + 8.0);
        ui.painter().text(
            left,
            Align2::LEFT_TOP,
            status_dot(s.status()),
            FontId::proportional(12.0),
            status_color(s.status()),
        );
        let name_pos = Pos2::new(left.x + 18.0, row_rect.min.y + 7.0);
        ui.painter().text(
            name_pos,
            Align2::LEFT_TOP,
            &s.name,
            FontId::proportional(13.5),
            ui.visuals().text_color(),
        );
        ui.painter().text(
            Pos2::new(name_pos.x, row_rect.min.y + 28.0),
            Align2::LEFT_TOP,
            s.cwd.display().to_string(),
            FontId::monospace(10.5),
            Color32::from_gray(140),
        );

        if resp.clicked() {
            clicked = Some(idx);
        }
        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        ui.allocate_space(egui::vec2(0.0, 2.0));
    }

    clicked
}
