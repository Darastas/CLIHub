//! 左侧边栏：SESSIONS 列表，点击切换 / 拖拽排序 / 悬浮删除 / 新增。
//!
//! 卡片用 `Sense::click_and_drag()`：单击 = 选中，按住拖动 = 排序
//! （`dnd_set_drag_payload` 设置载荷，由 `dnd_drop_zone` 接收）。

use egui::{Align2, Color32, FontId, Id, Pos2, Rect, RichText, Sense, Stroke, Ui, vec2};

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

const ACCENT: Color32 = Color32::from_rgb(59, 130, 246);

fn muted(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(166, 173, 200) // Subtext0 in Catppuccin
    } else {
        Color32::from_rgb(148, 163, 184) // Slate 400
    }
}

fn text(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(205, 214, 244) // Text in Catppuccin
    } else {
        Color32::from_rgb(30, 41, 59) // Slate 800
    }
}

fn name_secondary(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(186, 194, 222) // Subtext1 in Catppuccin
    } else {
        Color32::from_rgb(71, 85, 105) // Slate 600
    }
}

pub fn show(ui: &mut Ui, sessions: &[Session], selected: usize) -> SidebarAction {
    let mut action = SidebarAction::default();
    let dark = ui.visuals().dark_mode;

    // ---- SESSIONS 分区标题 + 新增按钮 ----
    ui.add_space(16.0);
    ui.horizontal(|ui| {
        ui.add_space(4.0);
        ui.label(RichText::new("SESSIONS").size(11.0).color(muted(dark)).strong());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let add = egui::Button::new(RichText::new("＋").size(16.0).color(ACCENT)).frame(false);
            if ui
                .add(add)
                .on_hover_text("Add a new CLI session")
                .clicked()
            {
                action.add = true;
            }
        });
    });
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.add_space(4.0);
        ui.label(
            RichText::new("click to open · drag to reorder")
                .size(9.5)
                .color(muted(dark)),
        );
    });
    ui.add_space(12.0);

    if sessions.is_empty() {
        ui.add_space(8.0);
        ui.label(
            RichText::new("No sessions — click ＋ to add one.")
                .size(11.0)
                .color(muted(dark)),
        );
    }

    // ---- 会话卡片（点击选中，拖动排序）----
    for (idx, s) in sessions.iter().enumerate() {
        let is_sel = idx == selected;
        let (_, dropped) = ui.dnd_drop_zone(egui::Frame::NONE, |ui| {
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                draw_card(ui, s, idx, is_sel, &mut action);
                ui.add_space(8.0);
            });
        });
        if let Some(from) = dropped {
            action.move_to = Some((*from, idx));
        }
        ui.allocate_space(vec2(0.0, 4.0));
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
                .color(muted(dark)),
        );
    });

    action
}

/// 绘制一张会话卡片。单击选中，按住拖动时设置排序载荷。
fn draw_card(
    ui: &mut Ui,
    s: &Session,
    idx: usize,
    is_sel: bool,
    action: &mut SidebarAction,
) {
    let dark = ui.visuals().dark_mode;
    let card_width = ui.available_width();
    let row_rect = Rect::from_min_size(
        Pos2::new(ui.cursor().min.x, ui.cursor().min.y),
        vec2(card_width, 52.0),
    );
    // click_and_drag：单击=选中，拖动=排序
    let resp = ui.allocate_rect(row_rect, Sense::click_and_drag());
    let dragged = resp.dragged();

    // 背景：基于底色的柔和 Alpha 叠加（无论深浅背景都自然融入、不突兀）
    let overlay = |a: u8| -> Color32 {
        if dark {
            Color32::from_white_alpha(a)
        } else {
            Color32::from_black_alpha(a)
        }
    };
    let bg = if dragged {
        overlay(70)
    } else if is_sel {
        overlay(34)
    } else if resp.hovered() {
        overlay(16)
    } else {
        Color32::TRANSPARENT
    };

    if bg != Color32::TRANSPARENT {
        ui.painter().rect_filled(row_rect, 12.0, bg);
    }

    // 拖拽中的描边 + 选中的左侧柔和指示条
    if dragged {
        ui.painter().rect_stroke(
            row_rect,
            12.0,
            Stroke::new(1.5, ACCENT),
            egui::StrokeKind::Inside,
        );
    } else if is_sel {
        ui.painter().rect_filled(
            Rect::from_min_max(
                Pos2::new(row_rect.min.x - 4.0, row_rect.center().y - 10.0),
                Pos2::new(row_rect.min.x, row_rect.center().y + 10.0),
            ),
            2.0,
            ACCENT,
        );
    }

    // 状态圆点 + 名称 + 工作目录
    ui.painter().text(
        Pos2::new(row_rect.min.x + 12.0, row_rect.min.y + 10.0),
        Align2::LEFT_TOP,
        status_dot(s.status()),
        FontId::proportional(11.0),
        status_color(s.status()),
    );
    ui.painter().text(
        Pos2::new(row_rect.min.x + 28.0, row_rect.min.y + 9.0),
        Align2::LEFT_TOP,
        &s.name,
        FontId::proportional(13.5),
        if is_sel { text(dark) } else { name_secondary(dark) },
    );
    ui.painter().text(
        Pos2::new(row_rect.min.x + 28.0, row_rect.min.y + 28.0),
        Align2::LEFT_TOP,
        s.cwd.display().to_string(),
        FontId::monospace(10.5),
        muted(dark),
    );

    // 悬浮时右侧删除按钮
    if resp.hovered() && !dragged {
        let btn_rect = Rect::from_center_size(
            Pos2::new(row_rect.right() - 20.0, row_rect.center().y),
            vec2(22.0, 22.0),
        );
        let btn = ui.interact(btn_rect, Id::new(("remove-session", idx)), Sense::click());
        
        let del_bg = if btn.hovered() {
            if dark { Color32::from_rgb(243, 139, 168).linear_multiply(0.2) } else { Color32::from_rgb(254, 226, 226) }
        } else {
            Color32::TRANSPARENT
        };
        
        if del_bg != Color32::TRANSPARENT {
            ui.painter().rect_filled(btn_rect, 11.0, del_bg);
        }
        
        ui.painter().text(
            btn_rect.center(),
            Align2::CENTER_CENTER,
            "✕",
            FontId::proportional(13.0),
            if btn.hovered() {
                if dark { Color32::from_rgb(243, 139, 168) } else { Color32::from_rgb(220, 38, 38) }
            } else {
                muted(dark)
            },
        );
        if btn.clicked() {
            action.remove = Some(idx);
        }
    }

    if resp.clicked() && action.remove.is_none() {
        action.select = Some(idx);
    }
    // 拖动中设置排序载荷
    if dragged {
        resp.dnd_set_drag_payload(idx);
    }
}
