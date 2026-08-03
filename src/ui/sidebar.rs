//! 左侧边栏：列出所有 CLI 会话，支持点击切换、悬浮删除、底部新增。
//!
//! 视觉参照 prompt.md：主标题 = CLI 名称，副标题 = 工作目录路径；
//! 选中项使用浅色圆角矩形高亮，悬浮有反馈。

use egui::{Align2, Color32, FontId, Id, Pos2, Rect, RichText, Sense, Ui, vec2};

use crate::state::Session;

use super::{status_color, status_dot};

/// 边栏交互结果，由 App 层执行。
#[derive(Debug, Clone, Copy, Default)]
pub struct SidebarAction {
    /// 点击选中的会话索引
    pub select: Option<usize>,
    /// 请求删除的会话索引
    pub remove: Option<usize>,
    /// 请求新增会话
    pub add: bool,
}

/// 展示边栏，返回交互结果。
pub fn show(ui: &mut Ui, sessions: &[Session], selected: usize) -> SidebarAction {
    let mut action = SidebarAction::default();

    ui.add_space(12.0);
    ui.vertical_centered(|ui| {
        ui.label(RichText::new("AI CLI Hub").strong().size(16.0));
    });
    ui.add_space(4.0);
    ui.separator();
    ui.add_space(8.0);

    // ---- 会话区标题 + 新增按钮 ----
    ui.horizontal(|ui| {
        ui.label(RichText::new("CLI SESSIONS").size(10.5).color(Color32::from_gray(140)));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.small_button("＋").on_hover_text("Add a new CLI session").clicked() {
                action.add = true;
            }
        });
    });
    ui.add_space(4.0);

    if sessions.is_empty() {
        ui.add_space(6.0);
        ui.label(RichText::new("No sessions — click ＋ to add one.").size(11.0).color(Color32::from_gray(150)));
    }

    for (idx, s) in sessions.iter().enumerate() {
        let is_sel = idx == selected;
        let row_rect = Rect::from_min_size(
            Pos2::new(ui.cursor().min.x, ui.cursor().min.y),
            vec2(ui.available_width(), 52.0),
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
        // 选中项左侧强调条
        if is_sel {
            ui.painter().rect_filled(
                Rect::from_min_max(
                    Pos2::new(row_rect.min.x + 2.0, row_rect.min.y + 9.0),
                    Pos2::new(row_rect.min.x + 5.0, row_rect.max.y - 9.0),
                ),
                2.0,
                Color32::from_rgb(9, 105, 218),
            );
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

        // 悬浮时右侧显示删除按钮
        if resp.hovered() {
            let btn_rect = Rect::from_min_size(
                Pos2::new(row_rect.right() - 24.0, row_rect.top() + 8.0),
                vec2(18.0, 18.0),
            );
            let btn = ui.interact(btn_rect, Id::new(("remove-session", idx)), Sense::click());
            ui.painter().text(
                btn_rect.center(),
                Align2::CENTER_CENTER,
                "✕",
                FontId::proportional(12.0),
                if btn.hovered() { Color32::from_rgb(190, 60, 50) } else { Color32::from_gray(150) },
            );
            if btn.clicked() {
                action.remove = Some(idx);
            }
        }

        if resp.clicked() && action.remove.is_none() {
            action.select = Some(idx);
        }
        if resp.hovered() && !resp.clicked() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        ui.allocate_space(vec2(0.0, 2.0));
    }

    // 底部留白
    ui.add_space(8.0);
    ui.separator();
    ui.add_space(6.0);
    ui.label(RichText::new("Sessions keep running in the background").size(10.0).color(Color32::from_gray(160)));

    action
}
