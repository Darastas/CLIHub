//! 左侧边栏：SESSIONS 列表，支持点击切换 / 拖拽排序 / 悬浮删除 / 新增。

use egui::{Align2, Color32, FontId, Id, Pos2, Rect, RichText, Sense, Ui, vec2};

use crate::state::Session;

use super::{status_color, status_dot};

/// 边栏交互结果，由 App 层执行。
#[derive(Debug, Clone, Copy, Default)]
pub struct SidebarAction {
    pub select: Option<usize>,
    pub remove: Option<usize>,
    pub add: bool,
    pub settings: bool,
    /// 拖拽排序：(从哪个索引 → 放到哪个索引)
    pub move_to: Option<(usize, usize)>,
}

// 配色（浅色成品观感）
const ACCENT: Color32 = Color32::from_rgb(59, 130, 246); // blue-500
const SEL_BG: Color32 = Color32::from_rgb(239, 246, 255); // blue-50
const HOVER_BG: Color32 = Color32::from_rgb(243, 244, 246); // gray-100
const TEXT: Color32 = Color32::from_rgb(55, 65, 81); // gray-700
const MUTED: Color32 = Color32::from_rgb(148, 163, 184); // slate-400

pub fn show(ui: &mut Ui, sessions: &[Session], selected: usize) -> SidebarAction {
    let mut action = SidebarAction::default();

    // ---- SESSIONS 分区标题 + 新增按钮 ----
    ui.add_space(12.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new("SESSIONS").size(10.5).color(MUTED));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let add = egui::Button::new(RichText::new("＋").size(15.0).color(ACCENT)).frame(false);
            if ui
                .add(add)
                .on_hover_text("Add a new CLI session")
                .clicked()
            {
                action.add = true;
            }
        });
    });
    ui.add_space(2.0);
    ui.label(RichText::new("drag cards to reorder").size(9.5).color(MUTED));
    ui.add_space(6.0);

    if sessions.is_empty() {
        ui.add_space(8.0);
        ui.label(
            RichText::new("No sessions — click ＋ to add one.")
                .size(11.0)
                .color(MUTED),
        );
    }

    // ---- 会话卡片（可拖拽排序）----
    for (idx, s) in sessions.iter().enumerate() {
        let is_sel = idx == selected;
        let (_, dropped) = ui.dnd_drop_zone(egui::Frame::NONE, |ui| {
            ui.dnd_drag_source(Id::new(("session", idx)), idx, |ui| {
                draw_card(ui, s, idx, is_sel, &mut action);
            });
        });
        if let Some(from) = dropped {
            action.move_to = Some((*from, idx));
        }
        ui.allocate_space(vec2(0.0, 3.0));
    }

    // ---- 底部：设置 + 会话计数 ----
    ui.add_space(10.0);
    ui.separator();
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        if ui
            .button(RichText::new("⚙").size(13.0))
            .on_hover_text("Settings (theme, colors)")
            .clicked()
        {
            action.settings = true;
        }
        ui.label(
            RichText::new(format!("{} session(s)", sessions.len()))
                .size(10.0)
                .color(MUTED),
        );
    });

    action
}

/// 绘制一张会话卡片，返回是否需要重排（在 drop 时）。
fn draw_card(
    ui: &mut Ui,
    s: &Session,
    idx: usize,
    is_sel: bool,
    action: &mut SidebarAction,
) {
    let row_rect = Rect::from_min_size(
        Pos2::new(ui.cursor().min.x, ui.cursor().min.y),
        vec2(ui.available_width(), 48.0),
    );
    let resp = ui.allocate_rect(row_rect, Sense::click());

    // 背景：选中浅蓝，悬浮浅灰
    let bg = if is_sel {
        SEL_BG
    } else if resp.hovered() {
        HOVER_BG
    } else {
        Color32::TRANSPARENT
    };
    if bg != Color32::TRANSPARENT {
        ui.painter().rect_filled(row_rect, 8.0, bg);
    }
    // 选中左侧强调条
    if is_sel {
        ui.painter().rect_filled(
            Rect::from_min_max(
                Pos2::new(row_rect.min.x + 2.0, row_rect.min.y + 8.0),
                Pos2::new(row_rect.min.x + 4.5, row_rect.max.y - 8.0),
            ),
            2.0,
            ACCENT,
        );
    }

    // 状态圆点 + 名称 + 工作目录
    ui.painter().text(
        Pos2::new(row_rect.min.x + 15.0, row_rect.min.y + 8.0),
        Align2::LEFT_TOP,
        status_dot(s.status()),
        FontId::proportional(11.0),
        status_color(s.status()),
    );
    ui.painter().text(
        Pos2::new(row_rect.min.x + 31.0, row_rect.min.y + 7.0),
        Align2::LEFT_TOP,
        &s.name,
        FontId::proportional(13.0),
        if is_sel { TEXT } else { Color32::from_gray(80) },
    );
    ui.painter().text(
        Pos2::new(row_rect.min.x + 31.0, row_rect.min.y + 26.0),
        Align2::LEFT_TOP,
        s.cwd.display().to_string(),
        FontId::monospace(10.0),
        MUTED,
    );

    // 悬浮时右侧删除按钮
    if resp.hovered() {
        let btn_rect = Rect::from_min_size(
            Pos2::new(row_rect.right() - 26.0, row_rect.top() + 8.0),
            vec2(18.0, 18.0),
        );
        let btn = ui.interact(btn_rect, Id::new(("remove-session", idx)), Sense::click());
        ui.painter().text(
            btn_rect.center(),
            Align2::CENTER_CENTER,
            "✕",
            FontId::proportional(12.0),
            if btn.hovered() {
                Color32::from_rgb(220, 60, 50)
            } else {
                MUTED
            },
        );
        if btn.clicked() {
            action.remove = Some(idx);
        }
    }

    if resp.clicked() && action.remove.is_none() {
        action.select = Some(idx);
    }
    if resp.hovered() && !resp.clicked() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
    }
}
