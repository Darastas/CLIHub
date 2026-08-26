//! 右侧主区域：基于 alacritty 字符网格的终端渲染 + 原始按键转发。

pub mod clipboard;
pub mod grid_render;
pub mod input_handler;

#[allow(unused_imports)]
pub use clipboard::{cleanup_old_temp_images, get_clipboard_text, set_clipboard_text, smart_get_clipboard_content};
#[allow(unused_imports)]
pub use grid_render::{is_graphic_char, luminance, paint_grid, resolve_cell, resolve_color, rgb_to_color32};
pub use input_handler::{forward_keys, handle_scroll};

use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::vte::ansi::Rgb;
use egui::{Align2, Color32, FontId, Id, Pos2, Rect, Sense, Ui, vec2};

use crate::state::Session;
use crate::ui::image_preview::{is_image_path, show_attachment_pill, show_lightbox_modal};

/// 标签栏/终端区触发的动作，由 App 层执行。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalAction {
    /// 为当前会话新开一个实例（标签页）
    NewTab,
    /// 切换到某个标签页
    SwitchTab(usize),
    /// 关闭并移除某个标签页
    KillTab(usize),
}

/// 终端配色主题。
pub struct TermTheme {
    pub font_size: f32,
    /// 终端等宽字体族（JetBrains Mono）
    pub font_family: egui::FontFamily,
    /// 加粗字体族
    pub bold_family: egui::FontFamily,
    pub background: Color32,
    pub foreground: Color32,
    pub cursor: Color32,
    pub ansi: [Color32; 16],
    pub sidebar_card_color: Option<[u8; 3]>,
    pub attachment_position: crate::config::AttachmentPillPosition,
}

impl TermTheme {
    pub fn from_scheme(name: &str) -> Self {
        let theme = match name {
            "One Half Light" => Self {
                font_size: 15.0,
                font_family: egui::FontFamily::Monospace,
                bold_family: egui::FontFamily::Monospace,
                background: Color32::from_rgb(250, 250, 250),
                foreground: Color32::from_rgb(56, 58, 66),
                cursor: Color32::from_rgb(191, 193, 200),
                ansi: [
                    Color32::from_rgb(56, 58, 66),
                    Color32::from_rgb(228, 86, 73),
                    Color32::from_rgb(80, 161, 79),
                    Color32::from_rgb(193, 132, 1),
                    Color32::from_rgb(1, 132, 188),
                    Color32::from_rgb(166, 38, 164),
                    Color32::from_rgb(9, 151, 179),
                    Color32::from_rgb(250, 250, 250),
                    Color32::from_rgb(79, 82, 93),
                    Color32::from_rgb(228, 86, 73),
                    Color32::from_rgb(80, 161, 79),
                    Color32::from_rgb(193, 132, 1),
                    Color32::from_rgb(1, 132, 188),
                    Color32::from_rgb(166, 38, 164),
                    Color32::from_rgb(9, 151, 179),
                    Color32::from_rgb(250, 250, 250),
                ],
                sidebar_card_color: None,
                attachment_position: crate::config::AttachmentPillPosition::default(),
            },
            "One Half Dark" => Self {
                font_size: 15.0,
                font_family: egui::FontFamily::Monospace,
                bold_family: egui::FontFamily::Monospace,
                background: Color32::from_rgb(40, 44, 52),
                foreground: Color32::from_rgb(220, 223, 228),
                cursor: Color32::from_rgb(163, 179, 204),
                ansi: [
                    Color32::from_rgb(40, 44, 52),
                    Color32::from_rgb(224, 108, 117),
                    Color32::from_rgb(152, 195, 121),
                    Color32::from_rgb(229, 192, 123),
                    Color32::from_rgb(97, 175, 239),
                    Color32::from_rgb(198, 120, 221),
                    Color32::from_rgb(86, 182, 194),
                    Color32::from_rgb(220, 223, 228),
                    Color32::from_rgb(90, 99, 116),
                    Color32::from_rgb(224, 108, 117),
                    Color32::from_rgb(152, 195, 121),
                    Color32::from_rgb(229, 192, 123),
                    Color32::from_rgb(97, 175, 239),
                    Color32::from_rgb(198, 120, 221),
                    Color32::from_rgb(86, 182, 194),
                    Color32::from_rgb(220, 223, 228),
                ],
                sidebar_card_color: None,
                attachment_position: crate::config::AttachmentPillPosition::default(),
            },
            "Solarized Dark" => Self {
                font_size: 15.0,
                font_family: egui::FontFamily::Monospace,
                bold_family: egui::FontFamily::Monospace,
                background: Color32::from_rgb(0, 43, 54),
                foreground: Color32::from_rgb(131, 148, 150),
                cursor: Color32::from_rgb(147, 161, 161),
                ansi: [
                    Color32::from_rgb(7, 54, 66),
                    Color32::from_rgb(220, 50, 47),
                    Color32::from_rgb(133, 153, 0),
                    Color32::from_rgb(181, 137, 0),
                    Color32::from_rgb(38, 139, 210),
                    Color32::from_rgb(211, 54, 130),
                    Color32::from_rgb(42, 161, 152),
                    Color32::from_rgb(238, 232, 213),
                    Color32::from_rgb(0, 43, 54),
                    Color32::from_rgb(203, 75, 22),
                    Color32::from_rgb(88, 110, 117),
                    Color32::from_rgb(101, 123, 131),
                    Color32::from_rgb(147, 161, 161),
                    Color32::from_rgb(108, 113, 196),
                    Color32::from_rgb(147, 161, 161),
                    Color32::from_rgb(253, 246, 227),
                ],
                sidebar_card_color: None,
                attachment_position: crate::config::AttachmentPillPosition::default(),
            },
            "Solarized Light" => Self {
                font_size: 15.0,
                font_family: egui::FontFamily::Monospace,
                bold_family: egui::FontFamily::Monospace,
                background: Color32::from_rgb(253, 246, 227),
                foreground: Color32::from_rgb(101, 123, 131),
                cursor: Color32::from_rgb(101, 123, 131),
                ansi: [
                    Color32::from_rgb(7, 54, 66),
                    Color32::from_rgb(220, 50, 47),
                    Color32::from_rgb(133, 153, 0),
                    Color32::from_rgb(181, 137, 0),
                    Color32::from_rgb(38, 139, 210),
                    Color32::from_rgb(211, 54, 130),
                    Color32::from_rgb(42, 161, 152),
                    Color32::from_rgb(238, 232, 213),
                    Color32::from_rgb(0, 43, 54),
                    Color32::from_rgb(203, 75, 22),
                    Color32::from_rgb(88, 110, 117),
                    Color32::from_rgb(101, 123, 131),
                    Color32::from_rgb(147, 161, 161),
                    Color32::from_rgb(108, 113, 196),
                    Color32::from_rgb(147, 161, 161),
                    Color32::from_rgb(253, 246, 227),
                ],
                sidebar_card_color: None,
                attachment_position: crate::config::AttachmentPillPosition::default(),
            },
            "Nord" => Self {
                font_size: 15.0,
                font_family: egui::FontFamily::Monospace,
                bold_family: egui::FontFamily::Monospace,
                background: Color32::from_rgb(46, 52, 64),
                foreground: Color32::from_rgb(216, 222, 233),
                cursor: Color32::from_rgb(216, 222, 233),
                ansi: [
                    Color32::from_rgb(59, 66, 82),
                    Color32::from_rgb(191, 97, 106),
                    Color32::from_rgb(163, 190, 140),
                    Color32::from_rgb(235, 203, 139),
                    Color32::from_rgb(129, 161, 193),
                    Color32::from_rgb(180, 142, 173),
                    Color32::from_rgb(136, 192, 208),
                    Color32::from_rgb(229, 233, 240),
                    Color32::from_rgb(76, 86, 106),
                    Color32::from_rgb(191, 97, 106),
                    Color32::from_rgb(163, 190, 140),
                    Color32::from_rgb(235, 203, 139),
                    Color32::from_rgb(129, 161, 193),
                    Color32::from_rgb(180, 142, 173),
                    Color32::from_rgb(143, 188, 187),
                    Color32::from_rgb(236, 239, 244),
                ],
                sidebar_card_color: None,
                attachment_position: crate::config::AttachmentPillPosition::default(),
            },
            _ => Self {
                font_size: 15.0,
                font_family: egui::FontFamily::Monospace,
                bold_family: egui::FontFamily::Monospace,
                background: Color32::from_rgb(30, 30, 30),
                foreground: Color32::from_rgb(204, 204, 204),
                cursor: Color32::from_rgb(255, 255, 255),
                ansi: [
                    Color32::from_rgb(0, 0, 0),
                    Color32::from_rgb(205, 49, 49),
                    Color32::from_rgb(13, 188, 121),
                    Color32::from_rgb(229, 229, 16),
                    Color32::from_rgb(36, 114, 200),
                    Color32::from_rgb(188, 63, 188),
                    Color32::from_rgb(17, 168, 205),
                    Color32::from_rgb(229, 229, 229),
                    Color32::from_rgb(102, 102, 102),
                    Color32::from_rgb(241, 76, 76),
                    Color32::from_rgb(35, 209, 139),
                    Color32::from_rgb(245, 245, 67),
                    Color32::from_rgb(59, 142, 234),
                    Color32::from_rgb(214, 112, 214),
                    Color32::from_rgb(41, 184, 219),
                    Color32::from_rgb(255, 255, 255),
                ],
                sidebar_card_color: None,
                attachment_position: crate::config::AttachmentPillPosition::default(),
            },
        };
        theme
    }

    pub fn apply(&mut self, settings: &crate::config::ThemeSettings) {
        if let Some([r, g, b]) = settings.background {
            self.background = Color32::from_rgb(r, g, b);
        }
        if let Some([r, g, b]) = settings.foreground {
            self.foreground = Color32::from_rgb(r, g, b);
        }
        self.sidebar_card_color = settings.sidebar_card_color;
        self.attachment_position = settings.attachment_position;
    }

    pub fn is_dark(&self) -> bool {
        self.background.r() < 128 && self.background.g() < 128 && self.background.b() < 128
    }

    pub fn to_theme_colors(&self) -> crate::backend::terminal::TermThemeColors {
        let mut ansi = [Rgb { r: 0, g: 0, b: 0 }; 16];
        for i in 0..16 {
            ansi[i] = Rgb {
                r: self.ansi[i].r(),
                g: self.ansi[i].g(),
                b: self.ansi[i].b(),
            };
        }
        crate::backend::terminal::TermThemeColors {
            foreground: Rgb {
                r: self.foreground.r(),
                g: self.foreground.g(),
                b: self.foreground.b(),
            },
            background: Rgb {
                r: self.background.r(),
                g: self.background.g(),
                b: self.background.b(),
            },
            cursor: Rgb {
                r: self.cursor.r(),
                g: self.cursor.g(),
                b: self.cursor.b(),
            },
            ansi,
        }
    }
}

pub fn show(
    ui: &mut Ui,
    session: &mut Session,
    input_enabled: bool,
    theme: &TermTheme,
) -> Option<TerminalAction> {
    let mut action = None;

    // ---- 标签栏（Tab Bar，与 Session 卡片美学 100% 统一）----
    let tab_h = 34.0;
    let dark = theme.is_dark();
    ui.add_space(8.0);
    egui::ScrollArea::horizontal()
        .max_height(tab_h + 6.0)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.add_space(12.0);
                for ti in 0..session.tabs.len() {
                    let is_active = ti == session.active_tab;
                    if let Some(a) = draw_tab(ui, session, ti, is_active, theme) {
                        action = Some(a);
                    }
                    ui.add_space(8.0);
                }

                // 新增实例按钮
                let (plus_rect, plus_resp) = ui.allocate_exact_size(vec2(34.0, 34.0), Sense::click());
                let is_plus_hovered = plus_resp.hovered();
                let plus_hover_factor = ui.ctx().animate_bool(Id::new(("plus_tab_hover", session.id)), is_plus_hovered);

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
                p.rect_filled(plus_rect.translate(vec2(0.0, 1.5)), 12.0, plus_shadow);
                p.rect_filled(plus_rect, 12.0, plus_bg);
                p.rect_stroke(plus_rect, 12.0, egui::Stroke::new(0.5, plus_stroke), egui::StrokeKind::Inside);

                let plus_fg = if is_plus_hovered {
                    if dark { Color32::WHITE } else { Color32::BLACK }
                } else if dark {
                    Color32::from_gray(160)
                } else {
                    Color32::from_gray(100)
                };
                p.text(
                    plus_rect.center(),
                    Align2::CENTER_CENTER,
                    "＋",
                    FontId::new(14.0, egui::FontFamily::Proportional),
                    plus_fg,
                );

                if plus_resp.on_hover_text("Start a new instance").clicked() {
                    action = Some(TerminalAction::NewTab);
                }

                ui.add_space(6.0);

                // ---- 搜索栏 ----
                let is_search_open = session.tabs.get(session.active_tab).map_or(false, |t| t.search_state.is_open);
                let raw_expand_t = ui.ctx().animate_bool_with_time(Id::new(("search_tab_expand", session.id)), is_search_open, 0.28);
                let expand_factor = raw_expand_t * raw_expand_t * (3.0 - 2.0 * raw_expand_t);

                let collapsed_w = 34.0;
                let expanded_w = 340.0;
                let current_w = egui::lerp(collapsed_w..=expanded_w, expand_factor);
                let (search_rect, search_resp) = ui.allocate_exact_size(vec2(current_w, 34.0), Sense::click());

                let is_hovered = search_resp.hovered() && !is_search_open;
                let bar_hover_factor = ui.ctx().animate_bool(Id::new(("search_tab_hover", session.id)), is_hovered);

                fn lerp_c(a: Color32, b: Color32, t: f32) -> Color32 {
                    Color32::from_rgba_premultiplied(
                        (a.r() as f32 * (1.0 - t) + b.r() as f32 * t).clamp(0.0, 255.0) as u8,
                        (a.g() as f32 * (1.0 - t) + b.g() as f32 * t).clamp(0.0, 255.0) as u8,
                        (a.b() as f32 * (1.0 - t) + b.b() as f32 * t).clamp(0.0, 255.0) as u8,
                        (a.a() as f32 * (1.0 - t) + b.a() as f32 * t).clamp(0.0, 255.0) as u8,
                    )
                }

                let base_color = if dark { Color32::from_white_alpha(5) } else { Color32::from_black_alpha(8) };
                let hover_color = if dark { Color32::from_white_alpha(14) } else { Color32::from_black_alpha(16) };
                let search_bg = lerp_c(base_color, hover_color, bar_hover_factor);

                let base_stroke = if dark { Color32::from_white_alpha(6) } else { Color32::from_black_alpha(10) };
                let hover_stroke = if dark { Color32::from_white_alpha(14) } else { Color32::from_black_alpha(18) };
                let search_stroke = lerp_c(base_stroke, hover_stroke, bar_hover_factor);

                let p = ui.painter().with_clip_rect(search_rect);
                let shadow_alpha = (if dark { 50.0 } else { 12.0 } * (1.0 + bar_hover_factor * 0.2 + expand_factor * 0.25)) as u8;
                p.rect_filled(search_rect.translate(vec2(0.0, 1.5)), 12.0, Color32::from_black_alpha(shadow_alpha));
                p.rect_filled(search_rect, 12.0, search_bg);
                p.rect_stroke(search_rect, 12.0, egui::Stroke::new(0.5, search_stroke), egui::StrokeKind::Inside);

                if search_resp.on_hover_text("Search in Terminal (Ctrl+F)").clicked() && !is_search_open {
                    if let Some(tab) = session.tabs.get_mut(session.active_tab) {
                        tab.search_state.is_open = true;
                        tab.search_state.request_focus = true;
                        if let Some(sel) = tab.terminal.as_ref().and_then(|t| t.selected_text()) {
                            let trimmed = sel.trim();
                            if !trimmed.is_empty() {
                                tab.search_state.query = trimmed.to_string();
                                if let Some(t) = &tab.terminal {
                                    tab.search_state.matches = t.search(&tab.search_state.query, tab.search_state.case_sensitive);
                                    tab.search_state.active_match = 0;
                                }
                            }
                        }
                    }
                }

                let icon_pos = Pos2::new(search_rect.min.x + 17.0, search_rect.center().y);
                let icon_color = if is_search_open {
                    if dark { Color32::WHITE } else { Color32::BLACK }
                } else {
                    lerp_c(
                        if dark { Color32::from_gray(160) } else { Color32::from_gray(100) },
                        if dark { Color32::WHITE } else { Color32::BLACK },
                        bar_hover_factor,
                    )
                };
                p.text(
                    icon_pos,
                    Align2::CENTER_CENTER,
                    "🔍",
                    FontId::new(13.0, egui::FontFamily::Proportional),
                    icon_color,
                );

                let inner_alpha = ((expand_factor - 0.15) / 0.85).clamp(0.0, 1.0);
                if inner_alpha > 0.01 {
                    let inner_rect = Rect::from_min_max(
                        Pos2::new(search_rect.min.x + 34.0, search_rect.min.y),
                        Pos2::new(search_rect.max.x - 6.0, search_rect.max.y),
                    );
                    let mut child_ui = ui.new_child(
                        egui::UiBuilder::new()
                            .max_rect(inner_rect)
                            .layout(egui::Layout::left_to_right(egui::Align::Center)),
                    );
                    child_ui.spacing_mut().item_spacing = vec2(0.0, 0.0);

                    let text_sub = if dark { Color32::from_gray(210) } else { Color32::from_gray(70) };

                    if let Some(tab) = session.tabs.get_mut(session.active_tab) {
                        let mut execute_search = false;
                        let mut go_prev = false;
                        let mut go_next = false;
                        let mut close_bar = false;

                        let input_w = egui::lerp(20.0..=135.0, inner_alpha);
                        let (input_rect, _) = child_ui.allocate_exact_size(vec2(input_w, 24.0), Sense::hover());

                        let edit_id = child_ui.id().with("find_input");
                        let mut input_ui = child_ui.new_child(
                            egui::UiBuilder::new()
                                .max_rect(input_rect)
                                .layout(egui::Layout::left_to_right(egui::Align::Center)),
                        );

                        let text_color = if dark { Color32::WHITE } else { Color32::BLACK };
                        let edit_resp = input_ui.add(
                            egui::TextEdit::singleline(&mut tab.search_state.query)
                                .id(edit_id)
                                .desired_width(input_w)
                                .font(FontId::proportional(13.0))
                                .text_color(text_color)
                                .hint_text("Find...")
                                .frame(egui::Frame::NONE)
                                .margin(vec2(2.0, 2.0)),
                        );

                        if tab.search_state.request_focus {
                            edit_resp.request_focus();
                            tab.search_state.request_focus = false;
                        }

                        if edit_resp.changed() {
                            execute_search = true;
                        }

                        if edit_resp.has_focus() {
                            if child_ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
                                close_bar = true;
                            }
                            if child_ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter)) {
                                go_next = true;
                            }
                            if child_ui.input_mut(|i| i.consume_key(egui::Modifiers::SHIFT, egui::Key::Enter)) {
                                go_prev = true;
                            }
                        }

                        child_ui.add_space(6.0);

                        let total_matches = tab.search_state.matches.len();
                        let count_text = if tab.search_state.query.trim().is_empty() {
                            "-".to_string()
                        } else if total_matches == 0 {
                            "0/0".to_string()
                        } else {
                            format!("{}/{}", tab.search_state.active_match + 1, total_matches)
                        };

                        let (badge_bg, badge_stroke, badge_fg) = if total_matches == 0 && !tab.search_state.query.trim().is_empty() {
                            (
                                Color32::from_rgba_unmultiplied(225, 55, 45, 28),
                                Color32::from_rgba_unmultiplied(235, 75, 65, 75),
                                Color32::from_rgb(255, 120, 110),
                            )
                        } else if total_matches > 0 {
                            (
                                if dark { Color32::from_white_alpha(16) } else { Color32::from_black_alpha(14) },
                                if dark { Color32::from_white_alpha(26) } else { Color32::from_black_alpha(20) },
                                Color32::WHITE,
                            )
                        } else {
                            (
                                if dark { Color32::from_white_alpha(6) } else { Color32::from_black_alpha(8) },
                                if dark { Color32::from_white_alpha(8) } else { Color32::from_black_alpha(10) },
                                text_sub,
                            )
                        };

                        let (badge_rect, _) = child_ui.allocate_exact_size(vec2(38.0, 20.0), Sense::hover());
                        let p_b = child_ui.painter();
                        p_b.rect_filled(badge_rect, 5.0, badge_bg);
                        p_b.rect_stroke(badge_rect, 5.0, egui::Stroke::new(0.5, badge_stroke), egui::StrokeKind::Inside);
                        p_b.text(badge_rect.center(), Align2::CENTER_CENTER, count_text, FontId::new(11.0, egui::FontFamily::Monospace), badge_fg);

                        child_ui.add_space(6.0);

                        let (sep_rect, _) = child_ui.allocate_exact_size(vec2(1.0, 14.0), Sense::hover());
                        child_ui.painter().rect_filled(sep_rect, 0.0, if dark { Color32::from_white_alpha(12) } else { Color32::from_black_alpha(14) });

                        child_ui.add_space(6.0);

                        let btn_base = if dark { Color32::from_white_alpha(5) } else { Color32::from_black_alpha(8) };
                        let btn_hover = if dark { Color32::from_white_alpha(14) } else { Color32::from_black_alpha(16) };
                        let btn_base_stroke = if dark { Color32::from_white_alpha(8) } else { Color32::from_black_alpha(10) };
                        let btn_hover_stroke = if dark { Color32::from_white_alpha(18) } else { Color32::from_black_alpha(18) };
                        let btn_shadow = if dark { Color32::from_black_alpha(50) } else { Color32::from_black_alpha(12) };

                        // Prev
                        let (prev_rect, prev_resp) = child_ui.allocate_exact_size(vec2(24.0, 24.0), Sense::click());
                        let prev_hf = child_ui.ctx().animate_bool(Id::new(("find_prev_h", session.id)), prev_resp.hovered());
                        let prev_bg = lerp_c(btn_base, btn_hover, prev_hf);
                        let prev_stroke = lerp_c(btn_base_stroke, btn_hover_stroke, prev_hf);
                        let prev_p = child_ui.painter();
                        prev_p.rect_filled(prev_rect.translate(vec2(0.0, 1.0)), 7.0, btn_shadow);
                        prev_p.rect_filled(prev_rect, 7.0, prev_bg);
                        prev_p.rect_stroke(prev_rect, 7.0, egui::Stroke::new(0.5, prev_stroke), egui::StrokeKind::Inside);

                        let prev_c = lerp_c(if dark { Color32::from_gray(160) } else { Color32::from_gray(100) }, if dark { Color32::WHITE } else { Color32::BLACK }, prev_hf);
                        let c = prev_rect.center();
                        prev_p.line_segment([c + vec2(-3.5, 1.5), c + vec2(0.0, -2.5)], egui::Stroke::new(1.35, prev_c));
                        prev_p.line_segment([c + vec2(0.0, -2.5), c + vec2(3.5, 1.5)], egui::Stroke::new(1.35, prev_c));
                        if prev_resp.on_hover_text("Previous match (Shift+Enter)").clicked() {
                            go_prev = true;
                        }

                        child_ui.add_space(4.0);

                        // Next
                        let (next_rect, next_resp) = child_ui.allocate_exact_size(vec2(24.0, 24.0), Sense::click());
                        let next_hf = child_ui.ctx().animate_bool(Id::new(("find_next_h", session.id)), next_resp.hovered());
                        let next_bg = lerp_c(btn_base, btn_hover, next_hf);
                        let next_stroke = lerp_c(btn_base_stroke, btn_hover_stroke, next_hf);
                        let next_p = child_ui.painter();
                        next_p.rect_filled(next_rect.translate(vec2(0.0, 1.0)), 7.0, btn_shadow);
                        next_p.rect_filled(next_rect, 7.0, next_bg);
                        next_p.rect_stroke(next_rect, 7.0, egui::Stroke::new(0.5, next_stroke), egui::StrokeKind::Inside);

                        let next_c = lerp_c(if dark { Color32::from_gray(160) } else { Color32::from_gray(100) }, if dark { Color32::WHITE } else { Color32::BLACK }, next_hf);
                        let c = next_rect.center();
                        next_p.line_segment([c + vec2(-3.5, -2.0), c + vec2(0.0, 2.0)], egui::Stroke::new(1.35, next_c));
                        next_p.line_segment([c + vec2(0.0, 2.0), c + vec2(3.5, -2.0)], egui::Stroke::new(1.35, next_c));
                        if next_resp.on_hover_text("Next match (Enter)").clicked() {
                            go_next = true;
                        }

                        child_ui.add_space(4.0);

                        // Case
                        let is_case = tab.search_state.case_sensitive;
                        let (case_rect, case_resp) = child_ui.allocate_exact_size(vec2(26.0, 24.0), Sense::click());
                        let case_hf = child_ui.ctx().animate_bool(Id::new(("find_case_h", session.id)), case_resp.hovered());
                        let case_bg = if is_case {
                            if dark { Color32::from_white_alpha(26) } else { Color32::from_black_alpha(22) }
                        } else {
                            lerp_c(btn_base, btn_hover, case_hf)
                        };
                        let case_stroke = if is_case {
                            if dark { Color32::from_white_alpha(48) } else { Color32::from_black_alpha(35) }
                        } else {
                            lerp_c(btn_base_stroke, btn_hover_stroke, case_hf)
                        };
                        let case_p = child_ui.painter();
                        case_p.rect_filled(case_rect.translate(vec2(0.0, 1.0)), 7.0, btn_shadow);
                        case_p.rect_filled(case_rect, 7.0, case_bg);
                        case_p.rect_stroke(case_rect, 7.0, egui::Stroke::new(0.5, case_stroke), egui::StrokeKind::Inside);

                        let case_fg = if is_case {
                            Color32::WHITE
                        } else {
                            lerp_c(if dark { Color32::from_gray(160) } else { Color32::from_gray(100) }, if dark { Color32::WHITE } else { Color32::BLACK }, case_hf)
                        };
                        case_p.text(case_rect.center(), Align2::CENTER_CENTER, "Aa", FontId::new(11.5, egui::FontFamily::Proportional), case_fg);
                        if case_resp.on_hover_text("Match Case").clicked() {
                            tab.search_state.case_sensitive = !tab.search_state.case_sensitive;
                            execute_search = true;
                        }

                        child_ui.add_space(4.0);

                        // Close
                        let (close_rect, close_resp) = child_ui.allocate_exact_size(vec2(24.0, 24.0), Sense::click());
                        let close_hf = child_ui.ctx().animate_bool(Id::new(("find_close_h", session.id)), close_resp.hovered());
                        let close_bg = lerp_c(btn_base, btn_hover, close_hf);
                        let close_stroke = lerp_c(btn_base_stroke, btn_hover_stroke, close_hf);
                        let close_p = child_ui.painter();
                        close_p.rect_filled(close_rect.translate(vec2(0.0, 1.0)), 7.0, btn_shadow);
                        close_p.rect_filled(close_rect, 7.0, close_bg);
                        close_p.rect_stroke(close_rect, 7.0, egui::Stroke::new(0.5, close_stroke), egui::StrokeKind::Inside);

                        let close_color = lerp_c(if dark { Color32::from_gray(160) } else { Color32::from_gray(100) }, if dark { Color32::WHITE } else { Color32::BLACK }, close_hf);
                        let c_center = close_rect.center();
                        let cd = 3.2;
                        close_p.line_segment([c_center + vec2(-cd, -cd), c_center + vec2(cd, cd)], egui::Stroke::new(1.35, close_color));
                        close_p.line_segment([c_center + vec2(-cd, cd), c_center + vec2(cd, -cd)], egui::Stroke::new(1.35, close_color));
                        if close_resp.on_hover_text("Close (Esc)").clicked() {
                            close_bar = true;
                        }

                        if execute_search {
                            if let Some(t) = &tab.terminal {
                                tab.search_state.matches = t.search(&tab.search_state.query, tab.search_state.case_sensitive);
                                tab.search_state.active_match = 0;
                                if let Some(m) = tab.search_state.current_match().cloned() {
                                    if let Some(t_mut) = &mut tab.terminal {
                                        t_mut.scroll_to_match(&m);
                                    }
                                }
                            }
                        }

                        if go_prev {
                            tab.search_state.prev_match();
                            if let Some(m) = tab.search_state.current_match().cloned() {
                                if let Some(t_mut) = &mut tab.terminal {
                                    t_mut.scroll_to_match(&m);
                                }
                            }
                        }

                        if go_next {
                            tab.search_state.next_match();
                            if let Some(m) = tab.search_state.current_match().cloned() {
                                if let Some(t_mut) = &mut tab.terminal {
                                    t_mut.scroll_to_match(&m);
                                }
                            }
                        }

                        if close_bar {
                            tab.search_state.is_open = false;
                        }
                    }
                }
            });
        });
    ui.add_space(6.0);
    ui.separator();

    // ---- 错误提示 ----
    if let Some(err) = &session.error {
        ui.colored_label(Color32::from_rgb(190, 60, 50), err);
        ui.add_space(4.0);
    }

    // ---- 字体度量（决定每格宽高）----
    let font_id = FontId::new(theme.font_size, theme.font_family.clone());
    let (col_w, row_h) = ui.fonts_mut(|f| (f.glyph_width(&font_id, ' '), f.row_height(&font_id)));

    // ---- 终端区域 ----
    let term_size = vec2(
        ui.available_width() - 24.0, // 外边距
        (ui.available_height() - 12.0).max(60.0),
    );

    ui.horizontal(|ui| {
        ui.add_space(12.0); // 左外边距
        let (term_rect, resp) = ui.allocate_exact_size(term_size, Sense::click_and_drag());
        ui.memory_mut(|mem| mem.data.insert_temp(egui::Id::new("term_bottom_y"), term_rect.max.y));

        let is_search_open = session.tabs.get(session.active_tab).map_or(false, |t| t.search_state.is_open);
        let find_input_id = ui.id().with("find_input");
        let find_input_focused = ui.memory(|m| m.has_focus(find_input_id));

        // 点击终端区域时获取焦点；搜索栏未打开且未聚焦搜索框时才自动维持终端焦点
        if resp.clicked() {
            resp.request_focus();
        } else if input_enabled && !resp.has_focus() && !is_search_open && !find_input_focused {
            resp.request_focus();
        }
        let window_focused = ui.input(|i| i.focused);
        let focused = resp.has_focus() && window_focused && !find_input_focused;

        // 动态计算行列数并平分像素余数，保证上下左右 100% 绝对居中对称
        let min_margin_x = 12.0f32;
        let min_margin_y = 8.0f32;
        let avail_w = (term_rect.width() - min_margin_x * 2.0).max(col_w);
        let avail_h = (term_rect.height() - min_margin_y * 2.0).max(row_h);
        let cols = ((avail_w / col_w).floor().max(1.0)) as u16;
        let rows = ((avail_h / row_h).floor().max(1.0)) as u16;

        let grid_w = cols as f32 * col_w;
        let grid_h = rows as f32 * row_h;
        let pad_x = ((term_rect.width() - grid_w) / 2.0).max(min_margin_x);
        let pad_y = ((term_rect.height() - grid_h) / 2.0).max(min_margin_y);

        let grid_rect = Rect::from_min_size(
            term_rect.min + vec2(pad_x, pad_y),
            vec2(grid_w, grid_h),
        );

        // 背景 + 圆角
        let painter = ui.painter().with_clip_rect(term_rect);
        painter.rect_filled(term_rect, 10.0, theme.background);

        let border = if theme.is_dark() {
            Color32::from_rgb(60, 60, 60)
        } else {
            Color32::from_rgb(226, 232, 240)
        };
        let stroke = if focused {
            egui::Stroke::new(1.5, theme.cursor)
        } else {
            egui::Stroke::new(1.0, border)
        };
        painter.rect_stroke(term_rect, 10.0, stroke, egui::StrokeKind::Inside);

        // 激活标签页
        let active_tab = session.active_tab;
        let Some(tab) = session.tabs.get_mut(active_tab) else {
            painter.text(
                grid_rect.min + vec2(4.0, 4.0),
                Align2::LEFT_TOP,
                "No instances yet — click ＋ to start one.",
                FontId::proportional(13.0),
                theme.foreground.gamma_multiply(0.5),
            );
            return;
        };

        let find_bar_rect = if tab.search_state.is_open {
            let bar_w = 340.0;
            let bar_h = 36.0;
            let margin = 12.0;
            Some(Rect::from_min_size(
                Pos2::new(term_rect.max.x - bar_w - margin, term_rect.min.y + margin),
                vec2(bar_w, bar_h),
            ))
        } else {
            None
        };

        if let Some(pos) = resp.interact_pointer_pos() {
            let is_inside_find_bar = find_bar_rect.map_or(false, |r| r.contains(pos));
            if !is_inside_find_bar {
                let col = ((pos.x - grid_rect.min.x) / col_w).floor().clamp(0.0, (cols.saturating_sub(1)) as f32) as usize;
                let viewport_line = ((pos.y - grid_rect.min.y) / row_h).floor().clamp(0.0, (rows.saturating_sub(1)) as f32) as i32;

                if let Some(t) = &mut tab.terminal {
                    if resp.drag_started_by(egui::PointerButton::Primary) {
                        let display_offset = t.term.grid().display_offset() as i32;
                        let actual_line = viewport_line - display_offset;
                        let gp = crate::backend::terminal::GridPoint { line: actual_line, col };
                        t.selection = Some(crate::backend::terminal::SelectionRange { start: gp, end: gp });
                    } else if resp.dragged_by(egui::PointerButton::Primary) {
                        // 拖拽超出终端视口顶部/底部时触发自动滚屏
                        if pos.y < grid_rect.min.y {
                            let overflow = grid_rect.min.y - pos.y;
                            let scroll_lines = ((overflow / row_h) * 0.5).ceil().clamp(1.0, 5.0) as i32;
                            t.scroll_display(scroll_lines);
                            ui.ctx().request_repaint();
                        } else if pos.y > grid_rect.max.y {
                            let overflow = pos.y - grid_rect.max.y;
                            let scroll_lines = ((overflow / row_h) * 0.5).ceil().clamp(1.0, 5.0) as i32;
                            t.scroll_display(-scroll_lines);
                            ui.ctx().request_repaint();
                        }

                        let display_offset = t.term.grid().display_offset() as i32;
                        let actual_line = viewport_line - display_offset;
                        let gp = crate::backend::terminal::GridPoint { line: actual_line, col };
                        if let Some(sel) = &mut t.selection {
                            sel.end = gp;
                        }
                    }
                }
            }
        }

        // 选区拖拽结束：自动同步写入系统剪贴板
        if resp.drag_stopped_by(egui::PointerButton::Primary) {
            if let Some(t) = &tab.terminal {
                if let Some(sel_text) = t.selected_text() {
                    if !sel_text.is_empty() {
                        set_clipboard_text(&sel_text);
                        ui.ctx().copy_text(sel_text);
                    }
                }
            }
        }

        // 单击空白处取消选区
        if resp.clicked_by(egui::PointerButton::Primary) && !resp.dragged_by(egui::PointerButton::Primary) {
            let is_inside_find_bar = find_bar_rect.and_then(|r| resp.interact_pointer_pos().map(|p| r.contains(p))).unwrap_or(false);
            if !is_inside_find_bar {
                if let Some(t) = &mut tab.terminal {
                    t.selection = None;
                }
            }
        }

        // 经典终端右键交互（支持智能图文粘贴）
        if resp.secondary_clicked() {
            if let Some(t) = &mut tab.terminal {
                if t.selection.is_some() {
                    if let Some(sel_text) = t.selected_text() {
                        if !sel_text.is_empty() {
                            set_clipboard_text(&sel_text);
                            ui.ctx().copy_text(sel_text);
                        }
                    }
                    t.selection = None;
                } else if let Some(clip) = smart_get_clipboard_content() {
                    let trimmed_path = std::path::PathBuf::from(clip.trim().trim_matches('"'));
                    if is_image_path(&trimmed_path) && trimmed_path.exists() {
                        // 图片内容加入待发送暂存区，不污染终端输入行
                        tab.image_preview.add_attachment(trimmed_path);
                    } else if let Some(pty) = &mut tab.pty {
                        let _ = pty.write(clip.as_bytes());
                    }
                }
            }
        }

        // 文件拖拽注入（图片文件进入暂存区预览，非图片文件直接注入路径）
        let dropped_files = ui.input(|i| i.raw.dropped_files.clone());
        if !dropped_files.is_empty() {
            let mut paths_text = String::new();
            for file in dropped_files {
                if let Some(path) = file.path {
                    if is_image_path(&path) {
                        // 图片文件加入暂存区，不污染终端输入行
                        tab.image_preview.add_attachment(path);
                    } else {
                        let path_str = path.to_string_lossy();
                        paths_text.push_str(&format!("\"{path_str}\" "));
                    }
                }
            }
            if !paths_text.is_empty() {
                if let Some(pty) = &mut tab.pty {
                    let _ = pty.write(paths_text.as_bytes());
                }
            }
            resp.request_focus();
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Focus);
        }

        if let Some(t) = &mut tab.terminal {
            t.resize(cols, rows, col_w.round() as u16, row_h.round() as u16);
        }
        if let Some(p) = &mut tab.pty {
            p.resize(cols, rows);
        }

        // 仅在窗口具有 OS 焦点且没有在搜索框输入时转发键盘事件
        let can_receive_input = input_enabled && window_focused && !find_input_focused;
        if can_receive_input {
            if let Some(err) = forward_keys(ui, tab) {
                session.error = Some(err);
            }
        }

        // 启用 IME
        if focused {
            let ime_rect = if let Some(t) = &mut tab.terminal {
                let cursor_pt = t.term.grid().cursor.point;
                let display_offset = t.term.grid().display_offset();
                let cx = grid_rect.min.x + cursor_pt.column.0 as f32 * col_w;
                let cy = grid_rect.min.y + (cursor_pt.line.0 as i32 + display_offset as i32) as f32 * row_h;
                Rect::from_min_size(Pos2::new(cx, cy), vec2(col_w, row_h))
            } else {
                grid_rect
            };
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::IMERect(ime_rect));
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::IMEAllowed(true));
        }

        handle_scroll(ui, tab, resp.hovered(), grid_rect, col_w, row_h);

        // 渲染网格
        if let Some(t) = &mut tab.terminal {
            paint_grid(ui, theme, t, grid_rect, col_w, row_h, cols, rows, focused, &tab.search_state);

            // 绘制交互式滚动条
            let history = t.term.grid().history_size();
            let screen = t.term.grid().screen_lines();
            if history > 0 {
                let track_w = 8.0;
                let track_rect = Rect::from_min_max(
                    Pos2::new(term_rect.max.x - track_w - 2.0, term_rect.min.y + 2.0),
                    Pos2::new(term_rect.max.x - 2.0, term_rect.max.y - 2.0),
                );

                let scroll_resp = ui.interact(track_rect, ui.id().with("scrollbar"), Sense::click_and_drag());
                let painter = ui.painter();

                let is_hovered = scroll_resp.hovered() || scroll_resp.dragged();
                let bg_color = if is_hovered { Color32::from_black_alpha(60) } else { Color32::from_black_alpha(20) };
                painter.rect_filled(track_rect, 4.0, bg_color);

                let total = (history + screen) as f32;
                let thumb_h = (screen as f32 / total * track_rect.height()).max(20.0);

                let drag_id = ui.id().with("scrollbar_drag");
                if scroll_resp.drag_started() {
                    if let Some(pos) = scroll_resp.interact_pointer_pos() {
                        let display_offset = t.term.grid().display_offset() as f32;
                        let current_thumb_y = track_rect.max.y - thumb_h - (display_offset / history as f32) * (track_rect.height() - thumb_h);
                        let grab_offset = pos.y - current_thumb_y;
                        ui.data_mut(|d| d.insert_temp(drag_id, grab_offset));
                    }
                }

                if scroll_resp.dragged() {
                    if let Some(pos) = scroll_resp.interact_pointer_pos() {
                        let grab_offset: f32 = ui.data_mut(|d| d.get_temp(drag_id).unwrap_or(thumb_h / 2.0));
                        let new_thumb_y = pos.y - grab_offset;
                        let track_scrollable = track_rect.height() - thumb_h;
                        if track_scrollable > 0.0 {
                            let ratio = 1.0 - ((new_thumb_y - track_rect.min.y) / track_scrollable).clamp(0.0, 1.0);
                            let new_offset = (ratio * history as f32).round() as usize;
                            let current_offset = t.term.grid().display_offset();
                            let lines_delta = new_offset as i32 - current_offset as i32;
                            if lines_delta != 0 {
                                t.scroll_display(lines_delta);
                            }
                        }
                    }
                }

                let display_offset = t.term.grid().display_offset() as f32;
                let y = track_rect.max.y - thumb_h - (display_offset / history as f32) * (track_rect.height() - thumb_h);

                let thumb_rect = Rect::from_min_max(
                    Pos2::new(track_rect.min.x + 1.0, y),
                    Pos2::new(track_rect.max.x - 1.0, y + thumb_h),
                );

                let thumb_color = if is_hovered { Color32::from_white_alpha(150) } else { Color32::from_white_alpha(80) };
                painter.rect_filled(thumb_rect, 3.0, thumb_color);
            }
        }

        // 光标位置渲染 IME 预编辑文字
        if !tab.ime_preedit.is_empty() {
            if let Some(t) = &tab.terminal {
                let cursor_pt = t.term.grid().cursor.point;
                let cx = grid_rect.min.x + cursor_pt.column.0 as f32 * col_w;
                let cy = grid_rect.min.y + cursor_pt.line.0 as f32 * row_h;
                let painter = ui.painter().with_clip_rect(grid_rect);
                let preedit_font = FontId::new(theme.font_size, theme.font_family.clone());
                let text_width = painter.layout_no_wrap(
                    tab.ime_preedit.clone(), preedit_font.clone(), Color32::WHITE,
                ).rect.width();
                let bg_rect = Rect::from_min_size(
                    Pos2::new(cx, cy),
                    vec2(text_width + 4.0, row_h),
                );
                painter.rect_filled(bg_rect, 2.0, Color32::from_rgb(60, 60, 80));
                painter.text(
                    Pos2::new(cx + 2.0, cy),
                    Align2::LEFT_TOP,
                    &tab.ime_preedit,
                    preedit_font,
                    Color32::from_rgb(120, 180, 255),
                );
                painter.line_segment(
                    [Pos2::new(cx, cy + row_h - 1.0), Pos2::new(cx + text_width + 4.0, cy + row_h - 1.0)],
                    egui::Stroke::new(1.5, Color32::from_rgb(120, 180, 255)),
                );
            }
        }

        // 拖拽文件悬停视觉提示（优雅暗色微透渐变与居中磨砂胶囊）
        let is_hovering_file = ui.input(|i| !i.raw.hovered_files.is_empty());
        if is_hovering_file {
            let painter = ui.painter();
            // 柔和暗色微透整体遮罩
            painter.rect_filled(term_rect, 10.0, Color32::from_rgba_unmultiplied(8, 12, 18, 135));
            // 外围极细发丝微光发丝描边
            painter.rect_stroke(
                term_rect.shrink(1.0),
                10.0,
                egui::Stroke::new(1.0, Color32::from_rgba_unmultiplied(100, 160, 240, 50)),
                egui::StrokeKind::Inside,
            );

            // 居中高级悬浮磨砂玻璃胶囊
            let pill_w = 220.0;
            let pill_h = 42.0;
            let center = term_rect.center();
            let pill_rect = Rect::from_center_size(center, vec2(pill_w, pill_h));

            // 胶囊暗黑微透底色与细致高光边框
            painter.rect_filled(pill_rect, 10.0, Color32::from_rgba_unmultiplied(22, 26, 35, 235));
            painter.rect_stroke(
                pill_rect,
                10.0,
                egui::Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 32)),
                egui::StrokeKind::Inside,
            );

            let text_font = FontId::new(13.5, egui::FontFamily::Proportional);
            painter.text(
                center,
                Align2::CENTER_CENTER,
                "📥  释放以插入文件路径",
                text_font,
                Color32::from_rgb(220, 228, 240),
            );
        }

        // 绘制右下角多模态图片附件悬浮暂存区胶囊
        show_attachment_pill(ui, &mut tab.image_preview, term_rect, theme);

        // 绘制全屏 Lightbox 模态大图查看器
        show_lightbox_modal(ui, &mut tab.image_preview);
    });

    action
}

fn draw_tab(
    ui: &mut Ui,
    session: &Session,
    ti: usize,
    is_active: bool,
    theme: &TermTheme,
) -> Option<TerminalAction> {
    let label = format!("{} {}", session.name, ti + 1);
    let font_id = FontId::new(13.5, egui::FontFamily::Monospace);
    let text_w = ui.painter().layout_no_wrap(label.clone(), font_id.clone(), Color32::WHITE).rect.width();
    let tab_w = text_w + 46.0;
    let tab_h = 34.0;
    let (tab_rect, resp) = ui.allocate_exact_size(vec2(tab_w, tab_h), Sense::click());

    let dark = theme.is_dark();
    let sel_factor = ui.ctx().animate_bool(Id::new(("tab_sel", session.id, ti)), is_active);
    let hover_factor = ui.ctx().animate_bool(Id::new(("tab_hover", session.id, ti)), resp.hovered() && !is_active);

    fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
        Color32::from_rgba_premultiplied(
            (a.r() as f32 * (1.0 - t) + b.r() as f32 * t).clamp(0.0, 255.0) as u8,
            (a.g() as f32 * (1.0 - t) + b.g() as f32 * t).clamp(0.0, 255.0) as u8,
            (a.b() as f32 * (1.0 - t) + b.b() as f32 * t).clamp(0.0, 255.0) as u8,
            (a.a() as f32 * (1.0 - t) + b.a() as f32 * t).clamp(0.0, 255.0) as u8,
        )
    }

    let base_color = if dark { Color32::from_white_alpha(5) } else { Color32::from_black_alpha(8) };
    let hover_color = if dark { Color32::from_white_alpha(14) } else { Color32::from_black_alpha(16) };

    let custom_color = theme.sidebar_card_color.unwrap_or([0, 111, 238]);
    let sel_color = if dark {
        Color32::from_rgba_unmultiplied(custom_color[0], custom_color[1], custom_color[2], 42)
    } else {
        Color32::from_rgba_unmultiplied(custom_color[0], custom_color[1], custom_color[2], 26)
    };

    let bg = lerp_color(lerp_color(base_color, hover_color, hover_factor), sel_color, sel_factor);

    let base_stroke = if dark { Color32::from_white_alpha(6) } else { Color32::from_black_alpha(10) };
    let hover_stroke = if dark { Color32::from_white_alpha(14) } else { Color32::from_black_alpha(18) };
    let sel_stroke = Color32::from_rgba_unmultiplied(custom_color[0], custom_color[1], custom_color[2], if dark { 45 } else { 35 });
    let stroke_c = lerp_color(lerp_color(base_stroke, hover_stroke, hover_factor), sel_stroke, sel_factor);

    let name_normal = if dark { Color32::from_gray(160) } else { Color32::from_gray(100) };
    let name_hover = if dark { Color32::from_gray(230) } else { Color32::from_gray(30) };
    let name_sel = if dark { Color32::WHITE } else { Color32::BLACK };
    let fg_name = lerp_color(lerp_color(name_normal, name_hover, hover_factor), name_sel, sel_factor);

    let painter = ui.painter();

    if bg != Color32::TRANSPARENT {
        let shadow_alpha = (if dark { 60.0 } else { 15.0 } * (1.0 + sel_factor * 0.3 + hover_factor * 0.15)) as u8;
        let shadow_color = Color32::from_black_alpha(shadow_alpha);
        painter.rect_filled(tab_rect.translate(vec2(0.0, 1.5)), 12.0, shadow_color);
    }

    if bg != Color32::TRANSPARENT {
        painter.rect_filled(tab_rect, 12.0, bg);
    }

    painter.rect_stroke(tab_rect, 12.0, egui::Stroke::new(0.5, stroke_c), egui::StrokeKind::Inside);

    painter.text(
        Pos2::new(tab_rect.min.x + 14.0, tab_rect.center().y),
        Align2::LEFT_CENTER,
        &label,
        font_id,
        fg_name,
    );

    // 关闭按钮
    let close_rect = Rect::from_center_size(
        Pos2::new(tab_rect.right() - 16.0, tab_rect.center().y),
        vec2(18.0, 18.0),
    );
    let close_resp = ui.interact(close_rect, Id::new(("tab-close", session.id, ti)), Sense::click());

    if close_resp.hovered() {
        painter.rect_filled(close_rect, 6.0, Color32::from_rgb(220, 60, 50).gamma_multiply(0.85));
    }

    let close_color = if close_resp.hovered() {
        Color32::WHITE
    } else if dark {
        Color32::from_gray(140)
    } else {
        Color32::from_gray(120)
    };

    let center = close_rect.center();
    let d = 3.5;
    painter.line_segment(
        [center + vec2(-d, -d), center + vec2(d, d)],
        egui::Stroke::new(1.3, close_color),
    );
    painter.line_segment(
        [center + vec2(-d, d), center + vec2(d, -d)],
        egui::Stroke::new(1.3, close_color),
    );

    if close_resp.clicked() {
        return Some(TerminalAction::KillTab(ti));
    }
    if resp.clicked() {
        return Some(TerminalAction::SwitchTab(ti));
    }
    None
}
