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

pub fn show(ui: &mut Ui, sessions: &[Session], selected: usize, theme: &crate::config::ThemeSettings) -> SidebarAction {
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
    }

    // ---- SESSIONS 分区标题 + 新增按钮 ----
    ui.add_space(16.0);
    ui.horizontal(|ui| {
        ui.add_space(12.0); // SidePanel inner margin is 8.0, 8.0 + 12.0 = 20.0
        ui.label(RichText::new("SESSIONS").size(12.0).color(muted(dark)).strong());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(12.0); // 从右侧往左加 12px 的边距，与卡片的右边缘对齐 (224.0 - 12.0 = 212.0)
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

    if sessions.is_empty() {
        ui.add_space(8.0);
        ui.label(
            RichText::new("No sessions — click ＋ to add one.")
                .size(11.0)
                .color(muted(dark)),
        );
    }
    ui.add_space(16.0); // Added generous spacing before cards

    // ---- 会话卡片（点击选中，拖动排序）----
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for (idx, s) in sessions.iter().enumerate() {
                let is_sel = idx == selected;
                ui.push_id(s.id, |ui| {
                    draw_card(ui, s, idx, is_sel, &mut action, theme);
                });
                ui.add_space(2.0);
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

/// 绘制一张会话卡片。单击选中，按住拖动时设置排序载荷。
fn draw_card(
    ui: &mut Ui,
    s: &Session,
    idx: usize,
    is_sel: bool,
    action: &mut SidebarAction,
    theme: &crate::config::ThemeSettings,
) {
    let dark = ui.visuals().dark_mode;
    let row_height = 56.0;
    
    // Allocate full width for interaction
    let (row_rect, resp) = ui.allocate_exact_size(
        vec2(ui.available_width(), row_height),
        Sense::click_and_drag(),
    );
    let dragged = resp.dragged();
    let hovered = resp.hovered();

    // The visual background rect is slightly inset for a beautiful rounded floating look
    let margin_x = 12.0;
    let margin_y = 4.0;
    
    if dragged {
        ui.memory_mut(|mem| mem.data.insert_temp(Id::new("dragged_idx"), idx));
    }
    
    let is_dragging = ui.memory(|mem| mem.data.get_temp::<usize>(Id::new("dragged_idx")).is_some());
    if is_dragging || dragged {
        if let Some(pos) = ui.ctx().pointer_interact_pos() {
            if row_rect.contains(pos) {
                ui.memory_mut(|mem| mem.data.insert_temp(Id::new("target_idx"), idx));
            }
        }
    }

    let mut visual_rect = row_rect;
    if is_dragging || dragged {
        if let Some(dragged_id) = ui.memory(|mem| mem.data.get_temp::<usize>(Id::new("dragged_idx"))) {
            let target_id = ui.memory(|mem| mem.data.get_temp::<usize>(Id::new("target_idx"))).unwrap_or(dragged_id);
            
            let mut target_visual_idx = idx;
            if idx == dragged_id {
                target_visual_idx = target_id;
            } else if dragged_id < target_id {
                if idx > dragged_id && idx <= target_id {
                    target_visual_idx = idx.saturating_sub(1);
                }
            } else if dragged_id > target_id {
                if idx >= target_id && idx < dragged_id {
                    target_visual_idx = idx + 1;
                }
            }
            
            let offset_slots = target_visual_idx as f32 - idx as f32;
            let spacing = ui.spacing().item_spacing.y;
            visual_rect = visual_rect.translate(vec2(0.0, offset_slots * (row_height + 2.0 + spacing)));
        }
    }

    let anim_y = ui.ctx().animate_value_with_time(Id::new(("anim_y", s.id)), visual_rect.min.y, 0.15);
    visual_rect = Rect::from_min_size(Pos2::new(visual_rect.min.x, anim_y), visual_rect.size());

    let bg_rect = visual_rect.shrink2(vec2(margin_x, margin_y));

    // Smooth animations for hover and selection
    let sel_factor = ui.ctx().animate_bool(Id::new(("sel", s.id)), is_sel);
    let hover_factor = ui.ctx().animate_bool(Id::new(("hover", s.id)), hovered && !is_sel);

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
    
    // Interpolate background color
    let bg = lerp_color(lerp_color(base_color, hover_color, hover_factor), sel_color, sel_factor);

    // If dragging, we make the original card slightly faint
    let opacity = if dragged { 0.3 } else { 1.0 };

    // Interpolate text colors
    let name_normal = name_secondary(dark);
    let name_hover = text(dark);
    // Selected text becomes perfectly crisp against the subtle background
    let name_sel = if dark { Color32::WHITE } else { Color32::BLACK };
    let fg_name = lerp_color(lerp_color(name_normal, name_hover, hover_factor), name_sel, sel_factor);

    let cwd_normal = muted(dark);
    let cwd_sel = if dark { Color32::from_white_alpha(160) } else { Color32::from_black_alpha(160) };
    let fg_cwd = lerp_color(cwd_normal, cwd_sel, sel_factor);

    let dot_normal = status_color(s.status());
    let dot_sel = Color32::WHITE; // Pure white when selected on the blue background
    let fg_dot = lerp_color(dot_normal, dot_sel, sel_factor);
    
    let render_card = |painter: &egui::Painter, rect: Rect, alpha: f32, stroke: bool, is_hovered_for_delete: bool| {
        let mut bg = bg;
        bg[3] = (bg[3] as f32 * alpha) as u8;
        if bg != Color32::TRANSPARENT {
            painter.rect_filled(rect, 12.0, bg);
        }
        
        // Drag outline
        if stroke {
            painter.rect_stroke(
                rect,
                12.0,
                Stroke::new(2.0, Color32::from_rgb(custom_color[0], custom_color[1], custom_color[2]).gamma_multiply(0.5)),
                egui::StrokeKind::Inside,
            );
        }
        
        let mut fg_dot = fg_dot; fg_dot[3] = (fg_dot[3] as f32 * alpha) as u8;
        let mut fg_name = fg_name; fg_name[3] = (fg_name[3] as f32 * alpha) as u8;
        let mut fg_cwd = fg_cwd; fg_cwd[3] = (fg_cwd[3] as f32 * alpha) as u8;
        
        let text_start_x = rect.min.x + 12.0;
        painter.text(
            Pos2::new(text_start_x, rect.min.y + 8.0),
            Align2::LEFT_TOP,
            &s.name,
            FontId::new(14.0, egui::FontFamily::Monospace),
            fg_name,
        );
        let short_cwd = shorten_path(&s.cwd, 22);
        painter.text(
            Pos2::new(text_start_x, rect.min.y + 27.0),
            Align2::LEFT_TOP,
            short_cwd,
            FontId::new(11.5, egui::FontFamily::Monospace),
            fg_cwd,
        );
        
        // Draw elegant vector status indicator on the right side if delete button is not shown
        if !is_hovered_for_delete {
            let dot_center = Pos2::new(rect.right() - 20.0, rect.center().y);
            match s.status() {
                crate::state::SessionStatus::Running => { painter.circle_filled(dot_center, 4.0, fg_dot); },
                crate::state::SessionStatus::Idle => { painter.circle_stroke(dot_center, 3.5, Stroke::new(1.5, fg_dot)); },
                crate::state::SessionStatus::Failed => {
                    let d = 3.0;
                    painter.line_segment([dot_center - vec2(d, d), dot_center + vec2(d, d)], Stroke::new(1.5, fg_dot));
                    painter.line_segment([dot_center - vec2(d, -d), dot_center + vec2(d, -d)], Stroke::new(1.5, fg_dot));
                },
                crate::state::SessionStatus::Exited => { painter.circle_stroke(dot_center, 3.5, Stroke::new(1.0, fg_dot)); },
            }
        }
    };

    let delete_hovered = hovered && !dragged;
    if bg != Color32::TRANSPARENT && !dragged {
        let shadow_color = if dark { Color32::from_black_alpha(120) } else { Color32::from_black_alpha(30) };
        ui.put(bg_rect, |ui: &mut Ui| {
            egui::Frame::NONE
                .corner_radius(12)
                .shadow(egui::epaint::Shadow { offset: [0, 4].into(), blur: 12, spread: 0, color: shadow_color })
                .show(ui, |ui| {
                    ui.allocate_exact_size(bg_rect.size(), Sense::hover());
                }).response
        });
    }
    
    render_card(ui.painter(), bg_rect, opacity, false, delete_hovered);

    if dragged {
        // Draw the payload following the pointer
        if let Some(pointer_pos) = ui.ctx().pointer_interact_pos() {
            // drag_rect uses the original non-animated bg_rect size, centered at pointer
            let drag_rect = Rect::from_center_size(pointer_pos, bg_rect.size());
            let painter = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Tooltip, resp.id));
            
            // Opaque background for readability
            let opaque_bg = if dark { Color32::from_rgb(45, 45, 45) } else { Color32::from_rgb(240, 240, 240) };
            painter.rect_filled(drag_rect, 12.0, opaque_bg);
            
            render_card(&painter, drag_rect, 1.0, true, false);
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

    if resp.clicked() && action.remove.is_none() {
        action.select = Some(idx);
    }
}
