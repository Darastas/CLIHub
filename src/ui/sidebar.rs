//! 左侧边栏：SESSIONS 列表，点击切换 / 拖拽排序 / 悬浮删除 / 新增。
//!
//! 卡片用 `Sense::click_and_drag()`：单击 = 选中，按住拖动 = 排序
//! （`dnd_set_drag_payload` 设置载荷，由 `dnd_drop_zone` 接收）。

use std::path::Path;

use egui::{Align2, Color32, FontId, Id, Pos2, Rect, RichText, Sense, Stroke, Ui, vec2};

use crate::state::Session;

use super::status_color;

/// 将较长的工作目录路径智能缩短，避免超出边栏卡片
pub fn shorten_path(path: &Path, max_len: usize) -> String {
    let raw = path.to_string_lossy();
    if raw.is_empty() {
        return String::new();
    }

    // 尝试将用户家目录替换为 ~
    let home_replaced = if let Some(base_dirs) = directories::BaseDirs::new() {
        let home = base_dirs.home_dir();
        if let Ok(rel) = path.strip_prefix(home) {
            if rel.as_os_str().is_empty() {
                "~".to_string()
            } else {
                format!("~{}{}", std::path::MAIN_SEPARATOR, rel.display())
            }
        } else {
            raw.to_string()
        }
    } else {
        raw.to_string()
    };

    if home_replaced.chars().count() <= max_len {
        return home_replaced;
    }

    // 分割路径各级目录（兼容 Windows '\' 与 Unix '/'）
    let sep = std::path::MAIN_SEPARATOR.to_string();
    let parts: Vec<&str> = home_replaced.split(['/', '\\']).filter(|s| !s.is_empty()).collect();
    if parts.len() >= 2 {
        let prefix = parts[0];
        let last = parts[parts.len() - 1];
        let candidate = if home_replaced.starts_with('/') {
            format!("/{prefix}{sep}...{sep}{last}")
        } else if prefix.ends_with(':') {
            format!("{prefix}{sep}...{sep}{last}")
        } else {
            format!("{prefix}{sep}...{sep}{last}")
        };

        if candidate.chars().count() <= max_len {
            return candidate;
        }

        let last_only = format!("...{sep}{last}");
        if last_only.chars().count() <= max_len {
            return last_only;
        }
    }

    // 最后一级目录本身过长时，进行截断并加前缀省略号
    let chars: Vec<char> = home_replaced.chars().collect();
    if chars.len() > max_len {
        let keep = max_len.saturating_sub(3);
        let tail: String = chars[chars.len().saturating_sub(keep)..].iter().collect();
        format!("...{tail}")
    } else {
        home_replaced
    }
}

/// 边栏交互结果，由 App 层执行。
#[derive(Debug, Clone, Copy, Default)]
pub struct SidebarAction {
    pub select: Option<usize>,
    pub remove: Option<usize>,
    pub add: bool,
    pub settings: bool,
    pub edit: Option<usize>,
    /// 拖拽排序：(从哪个索引 → 放到哪个索引)
    pub move_to: Option<(usize, usize)>,
    /// 切换全景多会话看板
    pub toggle_overview: bool,
}

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

const ROW_HEIGHT: f32 = 56.0;
const ROW_SPACING: f32 = 4.0;
const SLOT_STRIDE: f32 = ROW_HEIGHT + ROW_SPACING;

pub fn show(
    ui: &mut Ui,
    sessions: &[Session],
    selected: usize,
    in_overview: bool,
    theme: &crate::config::ThemeSettings,
) -> SidebarAction {
    let mut action = SidebarAction::default();

    let dark = ui.visuals().dark_mode;
    
    // Clear drag state and handle drop if mouse is released
    if !ui.ctx().input(|i| i.pointer.any_down()) {
        let dragged = ui.memory(|mem| mem.data.get_temp::<usize>(Id::new("dragged_idx")));
        let target = ui.memory(|mem| mem.data.get_temp::<usize>(Id::new("target_idx")));
        
        if let (Some(dragged_idx), Some(target_idx)) = (dragged, target) {
            if dragged_idx != target_idx {
                action.move_to = Some((dragged_idx, target_idx));
            }
        }
        ui.memory_mut(|mem| {
            mem.data.remove::<usize>(Id::new("dragged_idx"));
            mem.data.remove::<usize>(Id::new("target_idx"));
        });
        ui.ctx().request_repaint();
    }

    // ---- SESSIONS 分区标题 + 视图切换 + 新增按钮 ----
    ui.add_space(16.0);
    ui.horizontal(|ui| {
        ui.add_space(12.0); // SidePanel inner margin is 8.0, 8.0 + 12.0 = 20.0
        ui.label(RichText::new("SESSIONS").size(12.0).color(muted(dark)).strong());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(12.0); // 从右侧往左加 12px 的边距
            
            // ＋ 新增按钮
            let (plus_rect, plus_resp) = ui.allocate_exact_size(vec2(24.0, 24.0), Sense::click());
            ui.add_space(4.0);
            // ⊞ 全景看板按钮
            let (ov_rect, ov_resp) = ui.allocate_exact_size(vec2(24.0, 24.0), Sense::click());

            let is_plus_hovered = plus_resp.hovered();
            let plus_hover_factor = ui.ctx().animate_bool(Id::new("sidebar_add_hover"), is_plus_hovered);
            
            let plus_base = if dark { Color32::from_white_alpha(5) } else { Color32::from_black_alpha(8) };
            let plus_hover = if dark { Color32::from_white_alpha(14) } else { Color32::from_black_alpha(16) };
            let plus_bg = Color32::from_rgba_premultiplied(
                (plus_base.r() as f32 * (1.0 - plus_hover_factor) + plus_hover.r() as f32 * plus_hover_factor).clamp(0.0, 255.0) as u8,
                (plus_base.g() as f32 * (1.0 - plus_hover_factor) + plus_hover.g() as f32 * plus_hover_factor).clamp(0.0, 255.0) as u8,
                (plus_base.b() as f32 * (1.0 - plus_hover_factor) + plus_hover.b() as f32 * plus_hover_factor).clamp(0.0, 255.0) as u8,
                (plus_base.a() as f32 * (1.0 - plus_hover_factor) + plus_hover.a() as f32 * plus_hover_factor).clamp(0.0, 255.0) as u8,
            );
            let plus_stroke = if dark { Color32::from_white_alpha(8) } else { Color32::from_black_alpha(10) };
            
            let p = ui.painter();
            let plus_shadow = if dark { Color32::from_black_alpha(60) } else { Color32::from_black_alpha(15) };
            p.rect_filled(plus_rect.translate(vec2(0.0, 1.5)), 6.0, plus_shadow);
            p.rect_filled(plus_rect, 6.0, plus_bg);
            p.rect_stroke(plus_rect, 6.0, egui::Stroke::new(0.5, plus_stroke), egui::StrokeKind::Inside);
            
            let plus_fg = if is_plus_hovered {
                if dark { Color32::WHITE } else { Color32::BLACK }
            } else {
                if dark { Color32::from_gray(160) } else { Color32::from_gray(100) }
            };
            p.text(
                plus_rect.center(),
                Align2::CENTER_CENTER,
                "＋",
                FontId::new(12.0, egui::FontFamily::Proportional),
                plus_fg,
            );

            if plus_resp.on_hover_text("Add a new CLI session").clicked() {
                action.add = true;
            }

            // 绘制 ⊞ 全景看板按钮
            let is_ov_hovered = ov_resp.hovered();
            let ov_bg = if in_overview {
                Color32::from_rgb(0, 111, 238)
            } else if is_ov_hovered {
                if dark { Color32::from_white_alpha(18) } else { Color32::from_black_alpha(16) }
            } else {
                if dark { Color32::from_white_alpha(5) } else { Color32::from_black_alpha(8) }
            };
            let ov_stroke = if in_overview {
                Color32::from_rgb(0, 111, 238)
            } else {
                if dark { Color32::from_white_alpha(8) } else { Color32::from_black_alpha(10) }
            };

            p.rect_filled(ov_rect, 6.0, ov_bg);
            p.rect_stroke(ov_rect, 6.0, egui::Stroke::new(0.5, ov_stroke), egui::StrokeKind::Inside);
            let ov_fg = if in_overview {
                Color32::WHITE
            } else if is_ov_hovered {
                if dark { Color32::WHITE } else { Color32::BLACK }
            } else {
                if dark { Color32::from_gray(160) } else { Color32::from_gray(100) }
            };
            p.text(
                ov_rect.center(),
                Align2::CENTER_CENTER,
                "⊞",
                FontId::new(13.0, egui::FontFamily::Proportional),
                ov_fg,
            );

            if ov_resp.on_hover_text("Global Sessions Overview (Ctrl+Shift+O)").clicked() {
                action.toggle_overview = true;
            }
        });
    });

    if sessions.is_empty() {
        ui.add_space(8.0);
        ui.label(
            RichText::new("No sessions — click ＋ to add one.")
                .size(11.0)
                .color(muted(dark)),
        );
    }
    ui.add_space(16.0); // Spacing before cards

    // ---- 会话卡片（点击选中，拖动排序）----
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = ROW_SPACING;
            let list_top = ui.cursor().min.y;

            // 若正在拖拽，实时根据鼠标坐标精确计算目标槽位索引
            let dragged_idx = ui.memory(|mem| mem.data.get_temp::<usize>(Id::new("dragged_idx")));
            let target_idx = if let (Some(_), Some(pos)) = (dragged_idx, ui.ctx().pointer_interact_pos()) {
                ui.ctx().request_repaint(); // 拖动期间维持高帧率平滑过渡
                let rel_y = pos.y - list_top;
                let raw_target = (rel_y / SLOT_STRIDE).round() as isize;
                let clamped = raw_target.clamp(0, (sessions.len().saturating_sub(1)) as isize) as usize;
                ui.memory_mut(|mem| mem.data.insert_temp(Id::new("target_idx"), clamped));
                Some(clamped)
            } else {
                ui.memory(|mem| mem.data.get_temp::<usize>(Id::new("target_idx")))
            };

            for (idx, s) in sessions.iter().enumerate() {
                let is_sel = idx == selected;
                ui.push_id(s.id, |ui| {
                    draw_card(ui, s, idx, is_sel, &mut action, theme, dragged_idx, target_idx);
                });
            }
        });

    ui.add_space(10.0);
    ui.separator();
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(format!("{} session(s)", sessions.len()))
                .size(10.0)
                .color(muted(dark)),
        );
    });

    action
}

/// 绘制一张会话卡片。单击选中，按住拖动时平滑位移排序。
fn draw_card(
    ui: &mut Ui,
    s: &Session,
    idx: usize,
    is_sel: bool,
    action: &mut SidebarAction,
    theme: &crate::config::ThemeSettings,
    dragged_idx: Option<usize>,
    target_idx: Option<usize>,
) {
    let dark = ui.visuals().dark_mode;
    
    let is_this_dragged = dragged_idx == Some(idx);
    let is_any_dragged = dragged_idx.is_some();

    // Allocate full width for interaction
    let (row_rect, resp) = ui.allocate_exact_size(
        vec2(ui.available_width(), ROW_HEIGHT),
        Sense::click_and_drag(),
    );
    let dragged = resp.dragged();
    let hovered = resp.hovered();

    let margin_x = 12.0;
    let margin_y = 4.0;
    
    if dragged && (dragged_idx.is_none() || dragged_idx == Some(idx)) {
        ui.memory_mut(|mem| mem.data.insert_temp(Id::new("dragged_idx"), idx));
    }

    // 计算当前卡片的目标槽位偏移量
    let mut offset_slots = 0.0;
    if let (Some(d_idx), Some(t_idx)) = (dragged_idx, target_idx) {
        if idx == d_idx {
            // 被拖动的卡片在列表中作为占位框平移到目标位置
            offset_slots = t_idx as f32 - d_idx as f32;
        } else if d_idx < t_idx {
            // 向下拖动：原处于 (d_idx, t_idx] 区间的卡片向上移 1 格让位
            if idx > d_idx && idx <= t_idx {
                offset_slots = -1.0;
            }
        } else if d_idx > t_idx {
            // 向上拖动：原处于 [t_idx, d_idx) 区间的卡片向下移 1 格让位
            if idx >= t_idx && idx < d_idx {
                offset_slots = 1.0;
            }
        }
    }

    let target_y = row_rect.min.y + offset_slots * SLOT_STRIDE;
    let anim_y = ui.ctx().animate_value_with_time(Id::new(("anim_y", s.id)), target_y, 0.12);
    let visual_rect = Rect::from_min_size(Pos2::new(row_rect.min.x, anim_y), row_rect.size());
    let bg_rect = visual_rect.shrink2(vec2(margin_x, margin_y));

    // Smooth animations for hover and selection
    let sel_factor = ui.ctx().animate_bool(Id::new(("sel", s.id)), is_sel);
    let hover_factor = ui.ctx().animate_bool(Id::new(("hover", s.id)), hovered && !is_sel && !is_any_dragged);

    fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
        Color32::from_rgba_premultiplied(
            (a.r() as f32 * (1.0 - t) + b.r() as f32 * t).clamp(0.0, 255.0) as u8,
            (a.g() as f32 * (1.0 - t) + b.g() as f32 * t).clamp(0.0, 255.0) as u8,
            (a.b() as f32 * (1.0 - t) + b.b() as f32 * t).clamp(0.0, 255.0) as u8,
            (a.a() as f32 * (1.0 - t) + b.a() as f32 * t).clamp(0.0, 255.0) as u8,
        )
    }

    let base_color = if dark { Color32::from_white_alpha(5) } else { Color32::from_black_alpha(8) };
    let hover_color = if dark { Color32::from_white_alpha(12) } else { Color32::from_black_alpha(15) };
    
    let custom_color = theme.sidebar_card_color.unwrap_or([0, 111, 238]);
    let sel_color = if dark { 
        Color32::from_rgba_unmultiplied(custom_color[0], custom_color[1], custom_color[2], 40)
    } else { 
        Color32::from_rgba_unmultiplied(custom_color[0], custom_color[1], custom_color[2], 24)
    };
    
    let bg = lerp_color(lerp_color(base_color, hover_color, hover_factor), sel_color, sel_factor);

    // Interpolate text colors
    let name_normal = name_secondary(dark);
    let name_hover = text(dark);
    let name_sel = if dark { Color32::WHITE } else { Color32::BLACK };
    let fg_name = lerp_color(lerp_color(name_normal, name_hover, hover_factor), name_sel, sel_factor);

    let cwd_normal = muted(dark);
    let cwd_sel = if dark { Color32::from_white_alpha(160) } else { Color32::from_black_alpha(160) };
    let fg_cwd = lerp_color(cwd_normal, cwd_sel, sel_factor);

    let dot_normal = status_color(s.status());
    let dot_sel = Color32::WHITE;
    let fg_dot = lerp_color(dot_normal, dot_sel, sel_factor);
    
    let render_card_content = |painter: &egui::Painter, rect: Rect, alpha: f32, is_placeholder: bool| {
        let mut card_bg = bg;
        card_bg[3] = (card_bg[3] as f32 * alpha) as u8;

        if is_placeholder {
            // 列表中的占位插槽：强调色虚线框 + 极微弱半透明底色
            let stroke_color = Color32::from_rgb(custom_color[0], custom_color[1], custom_color[2]).gamma_multiply(0.45);
            painter.rect_stroke(
                rect,
                12.0,
                Stroke::new(1.5, stroke_color),
                egui::StrokeKind::Inside,
            );
            let placeholder_bg = if dark { Color32::from_white_alpha(6) } else { Color32::from_black_alpha(6) };
            painter.rect_filled(rect, 12.0, placeholder_bg);
        } else if card_bg != Color32::TRANSPARENT {
            painter.rect_filled(rect, 12.0, card_bg);
        }
        
        let mut dot_c = fg_dot; dot_c[3] = (dot_c[3] as f32 * alpha) as u8;
        let mut name_c = fg_name; name_c[3] = (name_c[3] as f32 * alpha) as u8;
        let mut cwd_c = fg_cwd; cwd_c[3] = (cwd_c[3] as f32 * alpha) as u8;
        
        let text_start_x = rect.min.x + 12.0;
        painter.text(
            Pos2::new(text_start_x, rect.min.y + 8.0),
            Align2::LEFT_TOP,
            &s.name,
            FontId::new(14.0, egui::FontFamily::Monospace),
            name_c,
        );
        let short_cwd = shorten_path(&s.cwd, 22);
        painter.text(
            Pos2::new(text_start_x, rect.min.y + 27.0),
            Align2::LEFT_TOP,
            short_cwd,
            FontId::new(11.5, egui::FontFamily::Monospace),
            cwd_c,
        );
        
        // 状态圆点
        let dot_center = Pos2::new(rect.right() - 20.0, rect.center().y);
        match s.status() {
            crate::state::SessionStatus::Running => { painter.circle_filled(dot_center, 4.0, dot_c); },
            crate::state::SessionStatus::Idle => { painter.circle_stroke(dot_center, 3.5, Stroke::new(1.5, dot_c)); },
            crate::state::SessionStatus::Failed => {
                let d = 3.0;
                painter.line_segment([dot_center - vec2(d, d), dot_center + vec2(d, d)], Stroke::new(1.5, dot_c));
                painter.line_segment([dot_center - vec2(d, -d), dot_center + vec2(d, -d)], Stroke::new(1.5, dot_c));
            },
            crate::state::SessionStatus::Exited => { painter.circle_stroke(dot_center, 3.5, Stroke::new(1.0, dot_c)); },
        }
    };

    // 绘制列表中的卡片或占位插槽
    if is_this_dragged {
        // 当前卡片正在被拖动：原列表处画占位插槽
        render_card_content(ui.painter(), bg_rect, 0.4, true);
    } else {
        if bg != Color32::TRANSPARENT {
            let shadow_color = if dark { Color32::from_black_alpha(100) } else { Color32::from_black_alpha(20) };
            ui.painter().rect_filled(bg_rect.translate(vec2(0.0, 2.0)), 12.0, shadow_color);
        }
        render_card_content(ui.painter(), bg_rect, 1.0, false);
    }

    // 若当前卡片被拖动，在顶层 Tooltip 图层绘制跟随鼠标的悬浮卡片
    if is_this_dragged {
        if let Some(pointer_pos) = ui.ctx().pointer_interact_pos() {
            let drag_rect = Rect::from_center_size(pointer_pos, bg_rect.size());
            let painter = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Tooltip, resp.id));
            
            let opaque_bg = if dark { Color32::from_rgb(36, 39, 48) } else { Color32::from_rgb(255, 255, 255) };
            painter.rect_filled(drag_rect, 12.0, opaque_bg);
            painter.rect_stroke(
                drag_rect,
                12.0,
                Stroke::new(1.5, Color32::from_rgb(custom_color[0], custom_color[1], custom_color[2])),
                egui::StrokeKind::Inside,
            );
            
            render_card_content(&painter, drag_rect, 1.0, false);
        }
    }

    let resp = resp.on_hover_ui(|ui| {
        ui.label(RichText::new(&s.name).strong());
        ui.label(RichText::new(format!("Command: {}", s.command)).size(11.5));
        ui.label(RichText::new(format!("Path: {}", s.cwd.display())).size(11.5));
    });

    resp.context_menu(|ui| {
        if ui.button("Edit").clicked() {
            action.edit = Some(idx);
            ui.close();
        }
        if ui.button("Delete").clicked() {
            action.remove = Some(idx);
            ui.close();
        }
    });

    if resp.clicked() && action.remove.is_none() && !is_any_dragged {
        action.select = Some(idx);
    }
}
