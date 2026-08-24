//! 右侧主区域：基于 alacritty 字符网格的终端渲染 + 原始按键转发。
//!
//! - 渲染：每行先画背景色块（按连续同色合并），再按 (fg/bold/underline)
//!   分组生成 LayoutJob 画字；宽字符（CJK）由 WIDE_CHAR 标志处理。
//! - 输入：终端区获得焦点后，把 `Event::Text/Key/Paste` 转成字节流写回 PTY。
//! - 缩放：按面板尺寸 × 字体度量计算行列数，同步 PTY 与网格。

use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::term::cell::{Cell, Flags};
use alacritty_terminal::term::color::Colors;
use alacritty_terminal::vte::ansi::{Color as TermColor, Rgb};
use egui::{Align2, Color32, FontId, Id, Modifiers, Pos2, Rect, Sense, Ui, vec2};

use crate::state::{Session, TerminalInstance};

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

/// 终端配色主题（浅色）。
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
                    Color32::from_rgb(131, 148, 150),
                    Color32::from_rgb(108, 113, 196),
                    Color32::from_rgb(147, 161, 161),
                    Color32::from_rgb(253, 246, 227),
                ],
                sidebar_card_color: None,
            },
            "Tango Dark" => Self {
                font_size: 15.0,
                font_family: egui::FontFamily::Monospace,
                bold_family: egui::FontFamily::Monospace,
                background: Color32::from_rgb(0, 0, 0),
                foreground: Color32::from_rgb(211, 215, 207),
                cursor: Color32::from_rgb(255, 255, 255),
                ansi: [
                    Color32::from_rgb(0, 0, 0),
                    Color32::from_rgb(204, 0, 0),
                    Color32::from_rgb(78, 154, 6),
                    Color32::from_rgb(196, 160, 0),
                    Color32::from_rgb(52, 101, 164),
                    Color32::from_rgb(117, 80, 123),
                    Color32::from_rgb(6, 152, 154),
                    Color32::from_rgb(211, 215, 207),
                    Color32::from_rgb(85, 87, 83),
                    Color32::from_rgb(239, 41, 41),
                    Color32::from_rgb(138, 226, 52),
                    Color32::from_rgb(252, 233, 79),
                    Color32::from_rgb(114, 159, 207),
                    Color32::from_rgb(173, 127, 168),
                    Color32::from_rgb(52, 226, 226),
                    Color32::from_rgb(238, 238, 236),
                ],
                sidebar_card_color: None,
            },
            "Readable Solar Light" => Self {
                font_size: 15.0,
                font_family: egui::FontFamily::Monospace,
                bold_family: egui::FontFamily::Monospace,
                background: Color32::from_rgb(244, 245, 244), // F4F5F4 background
                foreground: Color32::from_rgb(101, 123, 131), // 657B83 foreground
                cursor: Color32::from_rgb(88, 110, 117), // 586E75 cursor
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
                    Color32::from_rgb(131, 148, 150),
                    Color32::from_rgb(108, 113, 196),
                    Color32::from_rgb(147, 161, 161),
                    Color32::from_rgb(253, 246, 227),
                ],
                sidebar_card_color: None,
            },
            "Campbell" | _ => Self {
                font_size: 15.0,
                font_family: egui::FontFamily::Monospace,
                bold_family: egui::FontFamily::Monospace,
                background: Color32::from_rgb(12, 12, 12),
                foreground: Color32::from_rgb(204, 204, 204),
                cursor: Color32::from_rgb(255, 255, 255),
                ansi: [
                    Color32::from_rgb(12, 12, 12),
                    Color32::from_rgb(197, 15, 31),
                    Color32::from_rgb(19, 161, 14),
                    Color32::from_rgb(193, 156, 0),
                    Color32::from_rgb(0, 55, 218),
                    Color32::from_rgb(136, 23, 152),
                    Color32::from_rgb(58, 150, 221),
                    Color32::from_rgb(204, 204, 204),
                    Color32::from_rgb(118, 118, 118),
                    Color32::from_rgb(231, 72, 86),
                    Color32::from_rgb(22, 198, 12),
                    Color32::from_rgb(249, 241, 165),
                    Color32::from_rgb(59, 120, 255),
                    Color32::from_rgb(180, 0, 158),
                    Color32::from_rgb(97, 214, 214),
                    Color32::from_rgb(242, 242, 242),
                ],
                sidebar_card_color: None,
            },
        };
        theme
    }

    /// 应用自定义颜色覆盖。
    pub fn apply(&mut self, settings: &crate::config::ThemeSettings) {
        if let Some([r, g, b]) = settings.background {
            self.background = Color32::from_rgb(r, g, b);
        }
        if let Some([r, g, b]) = settings.foreground {
            self.foreground = Color32::from_rgb(r, g, b);
        }
        self.sidebar_card_color = settings.sidebar_card_color;
    }

    /// 终端是否为暗底（用于派生提示条等颜色）。
    pub fn is_dark(&self) -> bool {
        self.background.r() < 128 && self.background.g() < 128 && self.background.b() < 128
    }

    /// 转换为 PTY OSC 响应使用的结构体。
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

/// `input_enabled`：是否把键盘事件转发给 PTY（由 App 决定，如新增会话
/// 对话框打开时禁止）。不依赖 egui 焦点，保证点击会话后即可打字。
/// `theme`：终端配色（由 App 按配置构建）。
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
                ui.add_space(12.0); // 必须与下方终端区域的左外边距完全一致 (12.0)
                for ti in 0..session.tabs.len() {
                    let is_active = ti == session.active_tab;
                    if let Some(a) = draw_tab(ui, session, ti, is_active, theme) {
                        action = Some(a);
                    }
                    ui.add_space(8.0);
                }

                // 新增实例按钮（自然阴影与超细微边框）
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
                // 柔和投影
                let plus_shadow = if dark { Color32::from_black_alpha(60) } else { Color32::from_black_alpha(15) };
                p.rect_filled(plus_rect.translate(vec2(0.0, 1.5)), 12.0, plus_shadow);
                // 卡片底色
                p.rect_filled(plus_rect, 12.0, plus_bg);
                // 极细微边框
                p.rect_stroke(plus_rect, 12.0, egui::Stroke::new(0.5, plus_stroke), egui::StrokeKind::Inside);
                
                let plus_fg = if is_plus_hovered {
                    if dark { Color32::WHITE } else { Color32::BLACK }
                } else {
                    if dark { Color32::from_gray(160) } else { Color32::from_gray(100) }
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

                // ---- Keynote / Apple 风格横向流体展开/收缩搜索栏 ----
                let is_search_open = session.tabs.get(session.active_tab).map_or(false, |t| t.search_state.is_open);
                // 动画时长 0.32 秒，配合苹果 signature fluid ease-out 阻尼曲线
                let raw_expand_t = ui.ctx().animate_bool_with_time(Id::new(("search_tab_expand", session.id)), is_search_open, 0.32);
                // 苹果经典减速缓动曲线: cubic-bezier(0.22, 1.0, 0.36, 1.0)
                let expand_factor = 1.0 - (1.0 - raw_expand_t.clamp(0.0, 1.0)).powf(3.0);

                let collapsed_w = 34.0;
                let expanded_w = 340.0;
                let current_w = egui::lerp(collapsed_w..=expanded_w, expand_factor);
                let (search_rect, search_resp) = ui.allocate_exact_size(vec2(current_w, 34.0), Sense::hover());

                let custom_color = theme.sidebar_card_color.unwrap_or([0, 111, 238]);
                let is_hovered = search_resp.hovered();
                let hover_factor = ui.ctx().animate_bool(Id::new(("search_tab_hover", session.id)), is_hovered && !is_search_open);

                // 外层磨砂玻璃背景与高级质感微透底色
                let outer_bg = if is_search_open {
                    if dark {
                        Color32::from_rgba_premultiplied(22, 25, 34, 245)
                    } else {
                        Color32::from_rgba_premultiplied(255, 255, 255, 245)
                    }
                } else if dark {
                    Color32::from_white_alpha(5)
                } else {
                    Color32::from_black_alpha(8)
                };
                let outer_hover = if is_search_open {
                    outer_bg
                } else if dark {
                    Color32::from_white_alpha(14)
                } else {
                    Color32::from_black_alpha(16)
                };
                let search_bg = Color32::from_rgba_premultiplied(
                    (outer_bg.r() as f32 * (1.0 - hover_factor) + outer_hover.r() as f32 * hover_factor).clamp(0.0, 255.0) as u8,
                    (outer_bg.g() as f32 * (1.0 - hover_factor) + outer_hover.g() as f32 * hover_factor).clamp(0.0, 255.0) as u8,
                    (outer_bg.b() as f32 * (1.0 - hover_factor) + outer_hover.b() as f32 * hover_factor).clamp(0.0, 255.0) as u8,
                    (outer_bg.a() as f32 * (1.0 - hover_factor) + outer_hover.a() as f32 * hover_factor).clamp(0.0, 255.0) as u8,
                );
                
                // 外层发丝级微边框（融入主题色与微高光）
                let search_stroke = if is_search_open {
                    Color32::from_rgba_unmultiplied(custom_color[0], custom_color[1], custom_color[2], if dark { 80 } else { 60 })
                } else if dark {
                    Color32::from_white_alpha(8)
                } else {
                    Color32::from_black_alpha(10)
                };

                let p = ui.painter();
                // 柔和深邃投影
                let search_shadow = if dark { Color32::from_black_alpha(70) } else { Color32::from_black_alpha(20) };
                p.rect_filled(search_rect.translate(vec2(0.0, 1.5)), 12.0, search_shadow);
                p.rect_filled(search_rect, 12.0, search_bg);
                p.rect_stroke(search_rect, 12.0, egui::Stroke::new(0.65, search_stroke), egui::StrokeKind::Inside);

                if expand_factor < 0.15 {
                    // ---- 收起状态：精致居中图标按钮 ----
                    let compact_click_resp = ui.interact(search_rect, Id::new(("search_compact_btn", session.id)), Sense::click());
                    let search_fg = if is_hovered {
                        if dark { Color32::WHITE } else { Color32::BLACK }
                    } else {
                        if dark { Color32::from_gray(160) } else { Color32::from_gray(100) }
                    };
                    p.text(
                        search_rect.center(),
                        Align2::CENTER_CENTER,
                        "🔍",
                        FontId::new(13.0, egui::FontFamily::Proportional),
                        search_fg,
                    );

                    if compact_click_resp.on_hover_text("Search in Terminal (Ctrl+F)").clicked() {
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
                } else {
                    // ---- 展开状态：精致微结构与操作群组 ----
                    let mut child_ui = ui.new_child(
                        egui::UiBuilder::new()
                            .max_rect(search_rect.shrink2(vec2(6.0, 3.5)))
                            .layout(egui::Layout::left_to_right(egui::Align::Center)),
                    );

                    let text_main = if dark { Color32::from_rgb(235, 240, 250) } else { Color32::from_rgb(30, 35, 45) };
                    let text_sub = if dark { Color32::from_rgb(140, 150, 168) } else { Color32::from_rgb(120, 130, 145) };

                    if let Some(tab) = session.tabs.get_mut(session.active_tab) {
                        let mut execute_search = false;
                        let mut go_prev = false;
                        let mut go_next = false;
                        let mut close_bar = false;

                        // 1) 内嵌文本胶囊框 (Inset Search Capsule)
                        let capsule_w = egui::lerp(40.0..=135.0, ((expand_factor - 0.15) / 0.85).clamp(0.0, 1.0));
                        let (capsule_rect, _) = child_ui.allocate_exact_size(vec2(capsule_w, 26.0), Sense::hover());
                        
                        let capsule_bg = if dark { Color32::from_black_alpha(65) } else { Color32::from_black_alpha(12) };
                        let capsule_stroke = if dark { Color32::from_white_alpha(10) } else { Color32::from_black_alpha(14) };
                        child_ui.painter().rect_filled(capsule_rect, 7.0, capsule_bg);
                        child_ui.painter().rect_stroke(capsule_rect, 7.0, egui::Stroke::new(0.5, capsule_stroke), egui::StrokeKind::Inside);

                        let mut capsule_ui = child_ui.new_child(
                            egui::UiBuilder::new()
                                .max_rect(capsule_rect.shrink2(vec2(6.0, 2.0)))
                                .layout(egui::Layout::left_to_right(egui::Align::Center)),
                        );
                        capsule_ui.label(egui::RichText::new("🔍").size(10.5).color(text_sub));
                        capsule_ui.add_space(2.0);

                        let edit_id = capsule_ui.id().with("find_input");
                        let edit_resp = capsule_ui.add(
                            egui::TextEdit::singleline(&mut tab.search_state.query)
                                .id(edit_id)
                                .desired_width(capsule_w - 28.0)
                                .font(FontId::proportional(12.5))
                                .hint_text("Find...")
                                .text_color(text_main)
                                .frame(egui::Frame::NONE)
                                .margin(egui::Margin::ZERO),
                        );

                        if tab.search_state.request_focus {
                            edit_resp.request_focus();
                            tab.search_state.request_focus = false;
                        }

                        if edit_resp.changed() {
                            execute_search = true;
                        }

                        if edit_resp.has_focus() {
                            if child_ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                                close_bar = true;
                            } else if child_ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                if child_ui.input(|i| i.modifiers.shift) {
                                    go_prev = true;
                                } else {
                                    go_next = true;
                                }
                            }
                        }

                        child_ui.add_space(4.0);

                        // 2) 匹配计数微徽章
                        let total_matches = tab.search_state.matches.len();
                        let current_idx = if total_matches == 0 { 0 } else { tab.search_state.active_match + 1 };
                        let has_query = !tab.search_state.query.trim().is_empty();
                        let count_text = format!("{current_idx}/{total_matches}");
                        let (badge_bg, badge_fg) = if total_matches == 0 && has_query {
                            (Color32::from_rgba_unmultiplied(225, 55, 45, 40), Color32::from_rgb(240, 90, 80))
                        } else {
                            (if dark { Color32::from_white_alpha(10) } else { Color32::from_black_alpha(10) }, text_sub)
                        };

                        let (badge_rect, _) = child_ui.allocate_exact_size(vec2(36.0, 20.0), Sense::hover());
                        let p_b = child_ui.painter();
                        p_b.rect_filled(badge_rect, 6.0, badge_bg);
                        p_b.text(badge_rect.center(), Align2::CENTER_CENTER, count_text, FontId::proportional(10.5), badge_fg);

                        child_ui.add_space(3.0);

                        // 3) 发丝级垂直分割线
                        let (sep_rect, _) = child_ui.allocate_exact_size(vec2(1.0, 16.0), Sense::hover());
                        child_ui.painter().rect_filled(sep_rect, 0.0, if dark { Color32::from_white_alpha(12) } else { Color32::from_black_alpha(14) });

                        child_ui.add_space(3.0);

                        // 4) ▲ 上一个按钮
                        let (prev_rect, prev_resp) = child_ui.allocate_exact_size(vec2(22.0, 22.0), Sense::click());
                        let prev_hover = prev_resp.hovered();
                        if prev_hover {
                            child_ui.painter().rect_filled(prev_rect, 5.0, if dark { Color32::from_white_alpha(18) } else { Color32::from_black_alpha(15) });
                        }
                        child_ui.painter().text(prev_rect.center(), Align2::CENTER_CENTER, "▲", FontId::proportional(9.5), if prev_hover { text_main } else { text_sub });
                        if prev_resp.on_hover_text("Previous match (Shift+Enter)").clicked() {
                            go_prev = true;
                        }

                        // 5) ▼ 下一个按钮
                        let (next_rect, next_resp) = child_ui.allocate_exact_size(vec2(22.0, 22.0), Sense::click());
                        let next_hover = next_resp.hovered();
                        if next_hover {
                            child_ui.painter().rect_filled(next_rect, 5.0, if dark { Color32::from_white_alpha(18) } else { Color32::from_black_alpha(15) });
                        }
                        child_ui.painter().text(next_rect.center(), Align2::CENTER_CENTER, "▼", FontId::proportional(9.5), if next_hover { text_main } else { text_sub });
                        if next_resp.on_hover_text("Next match (Enter)").clicked() {
                            go_next = true;
                        }

                        // 6) Aa 大小写切换按钮
                        let is_case = tab.search_state.case_sensitive;
                        let (case_rect, case_resp) = child_ui.allocate_exact_size(vec2(24.0, 22.0), Sense::click());
                        let case_hover = case_resp.hovered();
                        let case_bg = if is_case {
                            Color32::from_rgba_unmultiplied(custom_color[0], custom_color[1], custom_color[2], if dark { 80 } else { 55 })
                        } else if case_hover {
                            if dark { Color32::from_white_alpha(18) } else { Color32::from_black_alpha(15) }
                        } else {
                            Color32::TRANSPARENT
                        };
                        let case_fg = if is_case {
                            if dark { Color32::WHITE } else { Color32::from_rgb(custom_color[0], custom_color[1], custom_color[2]) }
                        } else if case_hover {
                            text_main
                        } else {
                            text_sub
                        };
                        if case_bg != Color32::TRANSPARENT {
                            child_ui.painter().rect_filled(case_rect, 5.0, case_bg);
                        }
                        if is_case {
                            child_ui.painter().rect_stroke(case_rect, 5.0, egui::Stroke::new(0.5, Color32::from_rgba_unmultiplied(custom_color[0], custom_color[1], custom_color[2], 130)), egui::StrokeKind::Inside);
                        }
                        child_ui.painter().text(case_rect.center(), Align2::CENTER_CENTER, "Aa", FontId::new(11.0, egui::FontFamily::Proportional), case_fg);
                        if case_resp.on_hover_text("Match Case").clicked() {
                            tab.search_state.case_sensitive = !tab.search_state.case_sensitive;
                            execute_search = true;
                        }

                        // 7) ✕ 关闭按钮
                        let (close_rect, close_resp) = child_ui.allocate_exact_size(vec2(22.0, 22.0), Sense::click());
                        let close_hover = close_resp.hovered();
                        if close_hover {
                            child_ui.painter().rect_filled(close_rect, 5.0, Color32::from_rgba_premultiplied(225, 55, 45, 75));
                        }
                        let close_color = if close_hover { Color32::WHITE } else { text_sub };
                        let c_center = close_rect.center();
                        let cd = 3.5;
                        child_ui.painter().line_segment([c_center + vec2(-cd, -cd), c_center + vec2(cd, cd)], egui::Stroke::new(1.2, close_color));
                        child_ui.painter().line_segment([c_center + vec2(-cd, cd), c_center + vec2(cd, -cd)], egui::Stroke::new(1.2, close_color));
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
    const PAD_X: f32 = 12.0;
    const HINT_H: f32 = 28.0;
    let term_size = vec2(
        ui.available_width() - 24.0, // 外边距
        (ui.available_height() - HINT_H - 12.0).max(60.0),
    );
    
    ui.horizontal(|ui| {
        ui.add_space(12.0); // 左外边距
        let (term_rect, resp) = ui.allocate_exact_size(term_size, Sense::click_and_drag());
        
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

        // 网格可用区域：内边距
        let grid_rect = Rect::from_min_max(
            term_rect.min + vec2(PAD_X, 8.0),
            term_rect.max - vec2(PAD_X, 8.0),
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

        // 按网格尺寸换算行列数并同步到当前标签
        let cols = ((grid_rect.width() / col_w).floor().max(1.0)) as u16;
        let rows = ((grid_rect.height() / row_h).floor().max(1.0)) as u16;
        

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
                        // 拖拽超出终端视口顶部/底部时触发自动滚屏 (Drag-to-select Auto-Scroll)
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

        // 选区拖拽结束：自动同步写入系统剪贴板（选中文本即复制，无需多按快捷键）
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
        
        // 单击空白处取消选区（只有在没有发生拖拽且确实是单纯点击时才清除）
        if resp.clicked_by(egui::PointerButton::Primary) && !resp.dragged_by(egui::PointerButton::Primary) {
            let is_inside_find_bar = find_bar_rect.and_then(|r| resp.interact_pointer_pos().map(|p| r.contains(p))).unwrap_or(false);
            if !is_inside_find_bar {
                if let Some(t) = &mut tab.terminal {
                    t.selection = None;
                }
            }
        }
        
        // 经典终端右键交互：有选区则复制并取消选中，无选区则从剪贴板粘贴
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
                } else if let Some(clip) = get_clipboard_text() {
                    if let Some(pty) = &mut tab.pty {
                        let _ = pty.write(clip.as_bytes());
                    }
                }
            }
        }

        if let Some(t) = &mut tab.terminal {
            t.resize(cols, rows);
        }
        if let Some(p) = &mut tab.pty {
            p.resize(cols, rows);
        }

        // 仅在窗口具有 OS 焦点且终端处于激活输入状态时转发键盘事件
        if input_enabled && focused {
            if let Some(err) = forward_keys(ui, tab) {
                session.error = Some(err);
            }
        }
        
        // 启用 IME — 位置跟随光标而非整个网格
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
                    Pos2::new(term_rect.max.x - 2.0, term_rect.max.y - 2.0)
                );
                
                // 独立分配一个拖拽响应区
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
                
                // 处理拖拽 (无极滑动)
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
                
                // 获取最新的 offset
                let display_offset = t.term.grid().display_offset() as f32;
                let y = track_rect.max.y - thumb_h - (display_offset / history as f32) * (track_rect.height() - thumb_h);
                
                let thumb_rect = Rect::from_min_max(
                    Pos2::new(track_rect.min.x + 1.0, y),
                    Pos2::new(track_rect.max.x - 1.0, y + thumb_h)
                );
                
                let thumb_color = if is_hovered { Color32::from_white_alpha(150) } else { Color32::from_white_alpha(80) };
                painter.rect_filled(thumb_rect, 3.0, thumb_color);
            }
        }

        // 渲染双击 Ctrl+C 防误触浮动提示条
        if let Some(last) = tab.last_ctrl_c {
            let elapsed_ms = last.elapsed().as_millis();
            if elapsed_ms <= 1000 {
                let remaining_ratio = 1.0 - (elapsed_ms as f32 / 1000.0).clamp(0.0, 1.0);
                let alpha = (remaining_ratio * 255.0) as u8;

                let hud_w = 260.0;
                let hud_h = 32.0;
                let hud_rect = Rect::from_center_size(
                    Pos2::new(term_rect.center().x, term_rect.max.y - 28.0),
                    vec2(hud_w, hud_h),
                );

                let p = ui.painter().with_clip_rect(term_rect);
                // 投影
                p.rect_filled(hud_rect.translate(vec2(0.0, 2.0)), 16.0, Color32::from_black_alpha((alpha as f32 * 0.4) as u8));
                // 底色
                let hud_bg = if dark {
                    Color32::from_rgba_premultiplied(35, 38, 48, alpha)
                } else {
                    Color32::from_rgba_premultiplied(240, 243, 250, alpha)
                };
                p.rect_filled(hud_rect, 16.0, hud_bg);
                // 边框
                let hud_stroke = if dark {
                    Color32::from_rgba_premultiplied(220, 160, 40, (alpha as f32 * 0.8) as u8)
                } else {
                    Color32::from_rgba_premultiplied(200, 140, 30, (alpha as f32 * 0.8) as u8)
                };
                p.rect_stroke(hud_rect, 16.0, egui::Stroke::new(1.0, hud_stroke), egui::StrokeKind::Inside);

                // 提示文字
                let text_color = if dark {
                    Color32::from_rgba_premultiplied(250, 220, 120, alpha)
                } else {
                    Color32::from_rgba_premultiplied(160, 90, 0, alpha)
                };
                p.text(
                    hud_rect.center(),
                    Align2::CENTER_CENTER,
                    "⚠️ 再按一次 Ctrl+C 终止/退出",
                    FontId::proportional(12.5),
                    text_color,
                );

                ui.ctx().request_repaint(); // 保证淡出动画平滑过渡
            }
        }

        // 在光标位置渲染 IME 预编辑文字（拼音）
        if !tab.ime_preedit.is_empty() {
            if let Some(t) = &tab.terminal {
                let cursor_pt = t.term.grid().cursor.point;
                let cx = grid_rect.min.x + cursor_pt.column.0 as f32 * col_w;
                let cy = grid_rect.min.y + cursor_pt.line.0 as f32 * row_h;
                let painter = ui.painter().with_clip_rect(grid_rect);
                let preedit_font = FontId::new(theme.font_size, theme.font_family.clone());
                // 背景条
                let text_width = painter.layout_no_wrap(
                    tab.ime_preedit.clone(), preedit_font.clone(), Color32::WHITE
                ).rect.width();
                let bg_rect = Rect::from_min_size(
                    Pos2::new(cx, cy),
                    vec2(text_width + 4.0, row_h),
                );
                painter.rect_filled(bg_rect, 2.0, Color32::from_rgb(60, 60, 80));
                // 文字
                painter.text(
                    Pos2::new(cx + 2.0, cy),
                    Align2::LEFT_TOP,
                    &tab.ime_preedit,
                    preedit_font,
                    Color32::from_rgb(120, 180, 255),
                );
                // 下划线
                painter.line_segment(
                    [Pos2::new(cx, cy + row_h - 1.0), Pos2::new(cx + text_width + 4.0, cy + row_h - 1.0)],
                    egui::Stroke::new(1.5, Color32::from_rgb(120, 180, 255)),
                );
            }
        }
    });



    action
}

// ---------------------------------------------------------------------------
// 渲染
// ---------------------------------------------------------------------------

fn paint_grid(
    ui: &mut Ui,
    theme: &TermTheme,
    terminal: &crate::backend::terminal::Terminal,
    rect: Rect,
    col_w: f32,
    row_h: f32,
    cols: u16,
    rows: u16,
    focused: bool,
    search_state: &crate::state::session::SearchState,
) {
    // Update the terminal's idea of the theme colors so OSC queries respond correctly
    {
        let mut tc = terminal.theme_colors.lock().unwrap();
        *tc = theme.to_theme_colors();
    }

    let term = &terminal.term;
    let grid = term.grid();
    let colors = term.colors();
    let display_offset = grid.display_offset();
    let painter = ui.painter().with_clip_rect(rect);
    let origin = rect.min;

    // 收集可见行 -> 每行的单元格
    let mut lines: Vec<Vec<&Cell>> = Vec::new();
    let mut current_line = None;
    for item in grid.display_iter() {
        if Some(item.point.line) != current_line {
            lines.push(Vec::new());
            current_line = Some(item.point.line);
        }
        if let Some(last_row) = lines.last_mut() {
            last_row.push(item.cell);
        }
    }

    for (li, cells) in lines.iter().enumerate() {
        let y = origin.y + li as f32 * row_h;

        // 1) 背景 run（连续同色合并）
        let mut runs: Vec<(usize, usize, Color32)> = Vec::new();
        for (ci, cell) in cells.iter().enumerate() {
            let flags = cell.flags;
            if flags.contains(Flags::WIDE_CHAR_SPACER) {
                continue;
            }
            let width = if flags.contains(Flags::WIDE_CHAR) {
                2
            } else {
                1
            };
            let (fg, bg) = resolve_cell(cell, colors, theme);
            let _ = fg;
            if bg != theme.background {
                let mut merged = false;
                if let Some((_, e, c)) = runs.last_mut() {
                    if *c == bg && *e == ci {
                        *e = ci + width;
                        merged = true;
                    }
                }
                if !merged {
                    runs.push((ci, ci + width, bg));
                }
            }
        }

        // 浅色主题下全宽深色容器（如输入框，span >= 20）智能调和；短色块（如 Logo，span < 20）保持 100% 原色
        let mut container_runs: Vec<(usize, usize)> = Vec::new();
        if !theme.is_dark() {
            for run in &mut runs {
                let span_len = run.1 - run.0;
                if span_len >= 20 && luminance(run.2) < 80.0 {
                    run.2 = Color32::from_rgb(234, 236, 240);
                    container_runs.push((run.0, run.1));
                }
            }
        }

        // 2) 文本 span（连续非空格、同色、同粗体 → 一个 painter.text）
        let mut spans: Vec<(usize, String, Color32, bool)> = Vec::new();
        let mut cur_start: Option<usize> = None;
        let mut cur_str = String::new();
        let mut cur_color = Color32::TRANSPARENT;
        let mut cur_bold = false;
        let mut last_ci: Option<usize> = None;
        
        for (ci, cell) in cells.iter().enumerate() {
            let flags = cell.flags;
            if flags.contains(Flags::WIDE_CHAR_SPACER) {
                continue;
            }
            let (mut color, _) = resolve_cell(cell, colors, theme);
            
            // 若字符落在调和为浅色容器的区间内且文字偏亮，则加深为深色文本
            if container_runs.iter().any(|(s, e)| ci >= *s && ci < *e) {
                if luminance(color) > 100.0 {
                    color = Color32::from_rgb(28, 30, 36);
                }
            }

            let is_bold = flags.contains(Flags::BOLD);
            let ch = if flags.contains(Flags::HIDDEN) {
                ' '
            } else {
                cell.c
            };
            if ch == ' ' {
                if let Some(sc) = cur_start.take() {
                    spans.push((sc, std::mem::take(&mut cur_str), cur_color, cur_bold));
                }
                cur_color = Color32::TRANSPARENT;
                last_ci = Some(ci);
                continue;
            }
            
            // 如果颜色、粗体改变，或者列索引不连续（中间有空格或被跳过的 wide char spacer），则断开 span
            let is_contiguous = last_ci.map_or(true, |l| ci == l + 1);
            if cur_start.is_none() || cur_color != color || cur_bold != is_bold || !is_contiguous {
                if let Some(sc) = cur_start.take() {
                    spans.push((sc, std::mem::take(&mut cur_str), cur_color, cur_bold));
                }
                cur_start = Some(ci);
                cur_color = color;
                cur_bold = is_bold;
            }
            cur_str.push(ch);
            last_ci = Some(ci);
        }
        if let Some(sc) = cur_start.take() {
            spans.push((sc, cur_str, cur_color, cur_bold));
        }

        // 3) 画背景
        for (s, e, c) in &runs {
            let x0 = origin.x + *s as f32 * col_w;
            let x1 = origin.x + *e as f32 * col_w;
            painter.rect_filled(
                Rect::from_min_max(Pos2::new(x0, y), Pos2::new(x1, y + row_h)),
                0.0,
                *c,
            );
        }
        
        // 3.5) 选区背景（基于缓冲区绝对行号，紧贴真实文字末尾，消除右侧大片空白高亮）
        if let Some(sel) = terminal.selection {
            let mut s_line = sel.start.line;
            let mut e_line = sel.end.line;
            let mut s_col = sel.start.col;
            let mut e_col = sel.end.col;
            if s_line > e_line || (s_line == e_line && s_col > e_col) {
                std::mem::swap(&mut s_line, &mut e_line);
                std::mem::swap(&mut s_col, &mut e_col);
            }
            
            let actual_line = li as i32 - display_offset as i32;
            if actual_line >= s_line && actual_line <= e_line {
                // 计算当前行真实文字内容的起始列与末尾列（跳过左侧前导空白和右侧填充空白）
                let mut content_start = 0;
                let mut found_start = false;
                let mut content_end = 0;
                for (ci, cell) in cells.iter().enumerate() {
                    if cell.c != ' ' && !cell.flags.contains(Flags::HIDDEN) {
                        if !found_start {
                            content_start = ci;
                            found_start = true;
                        }
                        let w = if cell.flags.contains(Flags::WIDE_CHAR) { 2 } else { 1 };
                        content_end = (ci + w).min(cols as usize);
                    }
                }

                let sel_span = if s_line == e_line {
                    if content_end > 0 && s_col < content_end {
                        Some((s_col.max(content_start), (e_col + 1).min(content_end)))
                    } else {
                        None
                    }
                } else if actual_line == s_line {
                    if content_end > 0 && s_col < content_end {
                        Some((s_col.max(content_start), content_end))
                    } else {
                        None
                    }
                } else if actual_line == e_line {
                    if content_end > 0 && e_col >= content_start {
                        Some((content_start, (e_col + 1).min(content_end)))
                    } else {
                        None
                    }
                } else {
                    if content_end > 0 && content_start < content_end {
                        Some((content_start, content_end))
                    } else {
                        None
                    }
                };

                if let Some((sc, ec)) = sel_span {
                    if sc < ec {
                        let x0 = origin.x + sc as f32 * col_w;
                        let x1 = origin.x + ec as f32 * col_w;
                        let sel_bg = if theme.is_dark() {
                            Color32::from_rgb(0, 105, 230).gamma_multiply(0.42)
                        } else {
                            Color32::from_rgb(186, 212, 255)
                        };
                        painter.rect_filled(
                            Rect::from_min_max(Pos2::new(x0, y), Pos2::new(x1, y + row_h)),
                            0.0,
                            sel_bg,
                        );
                    }
                }
            }
        }

        // 3.6) 搜索匹配项高亮背景
        if search_state.is_open && !search_state.matches.is_empty() {
            let actual_line = li as i32 - display_offset as i32;
            for (match_idx, m) in search_state.matches.iter().enumerate() {
                if m.line == actual_line {
                    let is_active = match_idx == search_state.active_match;
                    let hl_bg = if is_active {
                        Color32::from_rgb(227, 179, 65)
                    } else if theme.is_dark() {
                        Color32::from_rgba_premultiplied(160, 120, 20, 160)
                    } else {
                        Color32::from_rgba_premultiplied(255, 230, 140, 200)
                    };
                    let x0 = origin.x + m.col_start as f32 * col_w;
                    let x1 = origin.x + (m.col_end + 1) as f32 * col_w;
                    painter.rect_filled(
                        Rect::from_min_max(Pos2::new(x0, y), Pos2::new(x1, y + row_h)),
                        2.0,
                        hl_bg,
                    );
                }
            }
        }

        // 4) 画文字（painter.text 经实测可渲染；粗体用独立字体族）
        let font_id = FontId::new(theme.font_size, theme.font_family.clone());
        let bold_id = FontId::new(theme.font_size, theme.bold_family.clone());
        for (start, text, color, bold) in &spans {
            let pos = Pos2::new(origin.x + *start as f32 * col_w, y);
            painter.text(
                pos,
                Align2::LEFT_TOP,
                text.as_str(),
                if *bold { bold_id.clone() } else { font_id.clone() },
                *color,
            );
        }
    }

    // ---- 光标（仅当终端未隐藏光标时绘制）----
    use alacritty_terminal::term::TermMode;
    let show_cursor = term.mode().contains(TermMode::SHOW_CURSOR);
    let cursor_point = grid.cursor.point;
    let viewport_line = cursor_point.line.0 + display_offset as i32;
    if show_cursor && viewport_line >= 0 && (viewport_line as usize) < lines.len() {
        let mut col = cursor_point.column.0;
        let cell = &grid[cursor_point];
        if cell.flags.contains(Flags::WIDE_CHAR_SPACER) && col > 0 {
            col -= 1;
        }
        if col < cols as usize {
            let width = if cell.flags.contains(Flags::WIDE_CHAR) {
                2.0 * col_w
            } else {
                col_w
            };
            let x = origin.x + col as f32 * col_w;
            let y = origin.y + viewport_line as f32 * row_h;
            let visible = if focused {
                (ui.input(|i| i.time * 2.0) as i64) % 2 == 0
            } else {
                true
            };
            if visible {
                painter.rect_filled(
                    Rect::from_min_size(Pos2::new(x, y), vec2(width, row_h)),
                    0.0,
                    theme.cursor,
                );
                // 在光标块上以背景色重画该格字形
                let glyph = if cell.c == ' ' { " " } else { &cell.c.to_string() };
                painter.text(
                    Pos2::new(x, y),
                    Align2::LEFT_TOP,
                    glyph,
                    FontId::new(theme.font_size, theme.font_family.clone()),
                    theme.background,
                );
            }
        }
    }
    let _ = rows;
}

pub(crate) fn luminance(c: Color32) -> f32 {
    0.2126 * (c.r() as f32) + 0.7152 * (c.g() as f32) + 0.0722 * (c.b() as f32)
}

pub(crate) fn is_graphic_char(c: char) -> bool {
    // 常见的 Unicode 绘图字符、色块、方块（ANSI 图形/Logo 使用）
    matches!(
        c,
        ' ' | '█' | '▀' | '▄' | '▌' | '▐' | '■' | '▲' | '▼' | '◆' | '●' | '░' | '▒' | '▓'
            | '┌' | '┐' | '└' | '┘' | '├' | '┤' | '┬' | '┴' | '┼' | '─' | '│'
    )
}

pub(crate) fn resolve_cell(cell: &Cell, colors: &Colors, theme: &TermTheme) -> (Color32, Color32) {
    let mut fg = resolve_color(&cell.fg, colors, theme, theme.foreground);
    let mut bg = resolve_color(&cell.bg, colors, theme, theme.background);

    // 1) 处理 SGR 7 (INVERSE 反显)
    if cell.flags.contains(Flags::INVERSE) {
        std::mem::swap(&mut fg, &mut bg);
    }

    // 2) 处理 SGR 2 (DIM 弱化)
    if cell.flags.contains(Flags::DIM) {
        let base = theme.background;
        fg = Color32::from_rgb(
            ((fg.r() as u16 + base.r() as u16) / 2) as u8,
            ((fg.g() as u16 + base.g() as u16) / 2) as u8,
            ((fg.b() as u16 + base.b() as u16) / 2) as u8,
        );
    }

    // 3) 仅对常规文字字符执行对比度保护，绝对不干扰图形色块与 Logo
    if !is_graphic_char(cell.c) {
        let lum_fg = luminance(fg);
        let lum_bg = luminance(bg);
        let contrast_diff = (lum_fg - lum_bg).abs();

        if contrast_diff < 35.0 {
            if lum_bg < 128.0 {
                // 背景偏深，文字太暗看不清 -> 提升文字为浅亮色
                fg = Color32::from_rgb(228, 231, 236);
            } else {
                // 背景偏浅，文字太浅看不清 -> 加深文字为深色
                fg = Color32::from_rgb(32, 35, 42);
            }
        }
    }

    (fg, bg)
}

pub(crate) fn resolve_color(color: &TermColor, colors: &Colors, theme: &TermTheme, fallback: Color32) -> Color32 {
    match color {
        TermColor::Named(n) => {
            let idx = *n as usize;
            if idx < 16 {
                theme.ansi[idx]
            } else if *n == alacritty_terminal::vte::ansi::NamedColor::Foreground {
                theme.foreground
            } else if *n == alacritty_terminal::vte::ansi::NamedColor::Background {
                theme.background
            } else if *n == alacritty_terminal::vte::ansi::NamedColor::Cursor {
                theme.cursor
            } else {
                colors[*n].map(rgb_to_color32).unwrap_or(fallback)
            }
        }
        TermColor::Spec(rgb) => rgb_to_color32(*rgb),
        TermColor::Indexed(i) => {
            let idx = *i as usize;
            if idx < 16 {
                theme.ansi[idx]
            } else {
                colors[*i as usize].map(rgb_to_color32).unwrap_or(fallback)
            }
        }
    }
}

pub(crate) fn rgb_to_color32(rgb: Rgb) -> Color32 {
    Color32::from_rgb(rgb.r, rgb.g, rgb.b)
}

// ---------------------------------------------------------------------------
// 输入
// ---------------------------------------------------------------------------

fn forward_keys(ui: &mut Ui, tab: &mut TerminalInstance) -> Option<String> {
    if !ui.input(|i| i.focused) {
        return None;
    }
    let find_input_id = ui.id().with("find_input");
    if ui.memory(|m| m.has_focus(find_input_id)) {
        return None; // 搜索框处于输入状态，绝对不向 PTY 终端转发按键
    }
    let events = ui.input(|i| i.events.clone());
    let mut out: Vec<u8> = Vec::new();
    
    for ev in events {
        match ev {
            // ---- IME 事件 ----
            egui::Event::Ime(egui::ImeEvent::Preedit { text, .. }) => {
                // 非空 preedit = 正在输入拼音；空 preedit = IME 取消
                tab.ime_composing = !text.is_empty();
                tab.ime_preedit = text;
            }
            egui::Event::Ime(egui::ImeEvent::Commit(text)) => {
                // 用户确认了输入法选字，把最终文字写入 PTY
                tab.ime_composing = false;
                tab.ime_preedit.clear();
                tab.ime_just_committed_text = Some(text.clone());
                out.extend_from_slice(text.as_bytes());
            }
            #[allow(deprecated)]
            egui::Event::Ime(_) => {}

            // ---- 以下事件在 IME 组合期间全部跳过 ----
            _ if tab.ime_composing => {
                // IME 正在编辑拼音时，Enter / Backspace / Text 等
                // 都属于输入法内部操作，不得转发给 PTY
                continue;
            }

            // ---- 普通事件（非 IME 组合态）----
            egui::Event::Copy => {
                // 1) 若存在选区：优先执行智能复制，绝不杀死当前运行任务
                if let Some(t) = &mut tab.terminal {
                    if t.selection.is_some() {
                        if let Some(sel) = t.selected_text() {
                            if !sel.is_empty() {
                                set_clipboard_text(&sel);
                                ui.ctx().copy_text(sel);
                            }
                        }
                        t.selection = None;
                        continue;
                    }
                }

                // 2) 无选区：执行双击 Ctrl+C 防误触中断保护 (1秒内连按两次才发送 SIGINT)
                let now = std::time::Instant::now();
                let is_double_press = if let Some(last) = tab.last_ctrl_c {
                    now.duration_since(last).as_millis() <= 1000
                } else {
                    false
                };

                if is_double_press {
                    tab.last_ctrl_c = None;
                    out.extend_from_slice(b"\x03"); // 发送 SIGINT (^C)
                } else {
                    tab.last_ctrl_c = Some(now);
                    ui.ctx().request_repaint_after(std::time::Duration::from_millis(1000));
                }
            }
            egui::Event::Paste(text) => out.extend_from_slice(text.as_bytes()),
            egui::Event::Text(text) => {
                // IME Commit 后紧跟的 Text 事件可能是被拆分的单个字符或整个字符串。
                // 只要它能跟 ime_just_committed_text 匹配上，我们就吞掉它。
                if let Some(mut committed) = tab.ime_just_committed_text.take() {
                    if committed.starts_with(text.as_str()) {
                        committed.drain(..text.len());
                        if !committed.is_empty() {
                            tab.ime_just_committed_text = Some(committed);
                        }
                        continue;
                    }
                }
                
                for ch in text.chars() {
                    let c = ch as u32;
                    if c < 0x20 || c == 0x7f {
                        continue; // 控制字符交给 Key 路径
                    }
                    let mut buf = [0u8; 4];
                    out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                }
            }
            egui::Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } => {
                let is_ctrl_or_cmd = modifiers.ctrl || modifiers.command;

                // 智能 Ctrl+C 与 双击 Ctrl+C 防误触保护
                let is_ctrl_c = is_ctrl_or_cmd && !modifiers.shift && !modifiers.alt && key == egui::Key::C;
                if is_ctrl_c {
                    // 1) 若存在选区：优先执行复制并取消选中
                    if let Some(t) = &mut tab.terminal {
                        if t.selection.is_some() {
                            if let Some(sel) = t.selected_text() {
                                if !sel.is_empty() {
                                    set_clipboard_text(&sel);
                                    ui.ctx().copy_text(sel);
                                }
                            }
                            t.selection = None;
                            continue;
                        }
                    }

                    // 2) 无选区：执行双击 Ctrl+C 判定 (1秒内连续按两次才真正中断)
                    let now = std::time::Instant::now();
                    let is_double_press = if let Some(last) = tab.last_ctrl_c {
                        now.duration_since(last).as_millis() <= 1000
                    } else {
                        false
                    };

                    if is_double_press {
                        tab.last_ctrl_c = None;
                        out.extend_from_slice(b"\x03"); // 发送 SIGINT (^C)
                    } else {
                        tab.last_ctrl_c = Some(now);
                        ui.ctx().request_repaint_after(std::time::Duration::from_millis(1000));
                    }
                    continue;
                }

                // 终端专用复制快捷键（Ctrl+Shift+C / Cmd+Shift+C / Ctrl+Insert）
                let is_copy_key = (is_ctrl_or_cmd && modifiers.shift && key == egui::Key::C)
                    || (key == egui::Key::Insert && modifiers.ctrl);

                if is_copy_key {
                    if let Some(t) = &mut tab.terminal {
                        if let Some(sel) = t.selected_text() {
                            if !sel.is_empty() {
                                set_clipboard_text(&sel);
                                ui.ctx().copy_text(sel);
                            }
                        }
                        t.selection = None;
                    }
                    continue;
                }

                // 搜索快捷键：Ctrl+F / Cmd+F 唤起终端内搜索条
                if is_ctrl_or_cmd && !modifiers.shift && !modifiers.alt && key == egui::Key::F {
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
                    continue;
                }

                // 粘贴快捷键：
                // 1) 终端标准 Ctrl+Shift+V / Cmd+Shift+V
                // 2) 终端标准 Shift+Insert
                let is_paste_key = (is_ctrl_or_cmd && modifiers.shift && key == egui::Key::V)
                    || (modifiers.shift && key == egui::Key::Insert);

                if is_paste_key {
                    if let Some(clip) = get_clipboard_text() {
                        out.extend_from_slice(clip.as_bytes());
                    }
                    continue;
                }
                
                if let Some(bytes) = map_key(key, &modifiers) {
                    out.extend_from_slice(&bytes);
                }
            }
            _ => {}
        }
    }
    if !out.is_empty() {
        if let Some(pty) = &mut tab.pty {
            if let Err(e) = pty.write(&out) {
                return Some(format!("写入 PTY 失败: {e}"));
            }
        }
    }
    None
}

fn map_key(key: egui::Key, mods: &Modifiers) -> Option<Vec<u8>> {
    use egui::Key::*;
    match key {
        Enter => Some(b"\r".to_vec()),
        Backspace => Some(vec![0x7f]),
        Escape => Some(vec![0x1b]),
        Tab => Some(if mods.shift { b"\x1b[Z".to_vec() } else { b"\t".to_vec() }),
        ArrowUp => Some(b"\x1b[A".to_vec()),
        ArrowDown => Some(b"\x1b[B".to_vec()),
        ArrowRight => Some(b"\x1b[C".to_vec()),
        ArrowLeft => Some(b"\x1b[D".to_vec()),
        Home => Some(b"\x1b[H".to_vec()),
        End => Some(b"\x1b[F".to_vec()),
        PageUp => Some(b"\x1b[5~".to_vec()),
        PageDown => Some(b"\x1b[6~".to_vec()),
        Delete => Some(b"\x1b[3~".to_vec()),
        Insert => Some(b"\x1b[2~".to_vec()),
        _ => {
            if mods.ctrl && !mods.alt {
                if let Some(c) = key_to_char(key) {
                    return Some(vec![(c as u8) & 0x1f]);
                }
            }
            None
        }
    }
}

fn key_to_char(key: egui::Key) -> Option<char> {
    use egui::Key::*;
    let k = key as usize;
    // egui 的 Key 枚举中 A..Z、Num0..Num9 各自连续
    if (A as usize..=Z as usize).contains(&k) {
        return Some(char::from(b'a' + (k - A as usize) as u8));
    }
    if (Num0 as usize..=Num9 as usize).contains(&k) {
        return Some(char::from(b'0' + (k - Num0 as usize) as u8));
    }
    match key {
        Space => Some(' '),
        _ => None,
    }
}

fn handle_scroll(
    ui: &mut Ui,
    tab: &mut TerminalInstance,
    hovered: bool,
    rect: Rect,
    col_w: f32,
    row_h: f32,
) {
    let pointer_in = ui.input(|i| {
        i.pointer.latest_pos().map_or(false, |p| rect.contains(p))
    });
    if !hovered && !pointer_in {
        return;
    }
    
    // 使用 smooth_scroll_delta 支持触控板无极滚动和普通鼠标滚轮。
    let scroll_y = ui.input(|i| i.smooth_scroll_delta.y);
    if scroll_y != 0.0 {
        tab.scroll_accum += scroll_y;
    }
    
    // 每 15 像素累积触发一行滚动（1格标准滚轮约50像素，可滚动3行）
    let pixels_per_line = 15.0;
    let lines = (tab.scroll_accum / pixels_per_line).trunc() as i32;
    
    if lines != 0 {
        tab.scroll_accum -= (lines as f32) * pixels_per_line;
        if let Some(t) = &mut tab.terminal {
            use alacritty_terminal::term::TermMode;
            let mode = t.term.mode();
            let has_mouse_report = mode.intersects(
                TermMode::MOUSE_REPORT_CLICK
                    | TermMode::MOUSE_DRAG
                    | TermMode::MOUSE_MOTION
                    | TermMode::MOUSE_MODE,
            );
            let in_alt_screen = mode.contains(TermMode::ALT_SCREEN);

            if has_mouse_report {
                // TUI 启用了鼠标协议（如 vim, htop, 现代 TUI 界面）
                let pointer_pos = ui.input(|i| i.pointer.latest_pos()).unwrap_or(rect.min);
                let rel_x = (pointer_pos.x - rect.min.x).max(0.0);
                let rel_y = (pointer_pos.y - rect.min.y).max(0.0);
                let col = ((rel_x / col_w) as usize + 1).min(t.cols as usize);
                let row = ((rel_y / row_h) as usize + 1).min(t.rows as usize);

                let count = lines.abs();
                let is_up = lines > 0;
                let mut bytes = Vec::new();

                for _ in 0..count {
                    if mode.contains(TermMode::SGR_MOUSE) {
                        // SGR 模式: 向上 64, 向下 65
                        let btn = if is_up { 64 } else { 65 };
                        bytes.extend_from_slice(format!("\x1b[<{btn};{col};{row}M").as_bytes());
                    } else if mode.contains(TermMode::UTF8_MOUSE) {
                        let btn = if is_up { 64 } else { 65 };
                        let mut buf = Vec::new();
                        buf.extend_from_slice(b"\x1b[M");
                        let b = 32 + btn;
                        let c = 32 + col.min(2015);
                        let r = 32 + row.min(2015);
                        let mut char_buf = [0u8; 4];
                        buf.extend_from_slice(char::from_u32(b as u32).unwrap_or(' ').encode_utf8(&mut char_buf).as_bytes());
                        buf.extend_from_slice(char::from_u32(c as u32).unwrap_or(' ').encode_utf8(&mut char_buf).as_bytes());
                        buf.extend_from_slice(char::from_u32(r as u32).unwrap_or(' ').encode_utf8(&mut char_buf).as_bytes());
                        bytes.extend_from_slice(&buf);
                    } else {
                        // 标准 X10 鼠标协议
                        let btn = if is_up { 64 } else { 65 };
                        let b = (32 + btn).min(255) as u8;
                        let c = (32 + col.min(223)) as u8;
                        let r = (32 + row.min(223)) as u8;
                        bytes.extend_from_slice(&[0x1b, b'[', b'M', b, c, r]);
                    }
                }

                if !bytes.is_empty() {
                    if let Some(pty) = &mut tab.pty {
                        let _ = pty.write(&bytes);
                    }
                }
            } else if in_alt_screen {
                // TUI 处于备用屏幕 (如 opencode, mimocode, less, nano 等) 但未开启鼠标协议时：
                // 工业级标准行为是将滚轮转换为光标上下箭头键（Alternate Screen Scroll），直接驱动 TUI 内容滚动！
                let is_app_cursor = mode.contains(TermMode::APP_CURSOR);
                let count = lines.abs();
                let is_up = lines > 0;
                let mut bytes = Vec::new();

                for _ in 0..count {
                    if is_up {
                        if is_app_cursor {
                            bytes.extend_from_slice(b"\x1bOA");
                        } else {
                            bytes.extend_from_slice(b"\x1b[A");
                        }
                    } else {
                        if is_app_cursor {
                            bytes.extend_from_slice(b"\x1bOB");
                        } else {
                            bytes.extend_from_slice(b"\x1b[B");
                        }
                    }
                }

                if !bytes.is_empty() {
                    if let Some(pty) = &mut tab.pty {
                        let _ = pty.write(&bytes);
                    }
                }
            } else {
                // 普通 Shell 终端屏幕：滚动 Scrollback 历史缓冲区
                t.scroll_display(lines);
            }
        }
    }
}

/// 绘制一个标签页卡片，返回点击/关闭对应的动作（100% 继承 Session 卡片美学、阴影与边缘高光）。
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

    // 100% 复刻 Session 侧边栏卡片的色彩计算公式
    let base_color = if dark { Color32::from_white_alpha(5) } else { Color32::from_black_alpha(8) };
    let hover_color = if dark { Color32::from_white_alpha(14) } else { Color32::from_black_alpha(16) };
    
    let custom_color = theme.sidebar_card_color.unwrap_or([0, 111, 238]);
    let sel_color = if dark { 
        Color32::from_rgba_unmultiplied(custom_color[0], custom_color[1], custom_color[2], 42)
    } else { 
        Color32::from_rgba_unmultiplied(custom_color[0], custom_color[1], custom_color[2], 26)
    };
    
    let bg = lerp_color(lerp_color(base_color, hover_color, hover_factor), sel_color, sel_factor);

    // 极细微边缘描边（纤细精致，绝不突兀粗厚）
    let base_stroke = if dark { Color32::from_white_alpha(6) } else { Color32::from_black_alpha(10) };
    let hover_stroke = if dark { Color32::from_white_alpha(14) } else { Color32::from_black_alpha(18) };
    let sel_stroke = Color32::from_rgba_unmultiplied(custom_color[0], custom_color[1], custom_color[2], if dark { 45 } else { 35 });
    let stroke_c = lerp_color(lerp_color(base_stroke, hover_stroke, hover_factor), sel_stroke, sel_factor);

    // 文字颜色插值（完全与 Session 卡片一致）
    let name_normal = if dark { Color32::from_gray(160) } else { Color32::from_gray(100) };
    let name_hover = if dark { Color32::from_gray(230) } else { Color32::from_gray(30) };
    let name_sel = if dark { Color32::WHITE } else { Color32::BLACK };
    let fg_name = lerp_color(lerp_color(name_normal, name_hover, hover_factor), name_sel, sel_factor);

    let painter = ui.painter();

    // 1) 绘制柔和下沉投影 (Drop Shadow，与 Session 卡片机制完全一致)
    if bg != Color32::TRANSPARENT {
        let shadow_alpha = (if dark { 60.0 } else { 15.0 } * (1.0 + sel_factor * 0.3 + hover_factor * 0.15)) as u8;
        let shadow_color = Color32::from_black_alpha(shadow_alpha);
        painter.rect_filled(tab_rect.translate(vec2(0.0, 1.5)), 12.0, shadow_color);
    }

    // 2) 绘制卡片本体背景 (12.0px 圆角矩形)
    if bg != Color32::TRANSPARENT {
        painter.rect_filled(tab_rect, 12.0, bg);
    }

    // 3) 绘制细腻微边框 (0.5px 发丝级微描边)
    painter.rect_stroke(tab_rect, 12.0, egui::Stroke::new(0.5, stroke_c), egui::StrokeKind::Inside);

    // 标题文本（已删除指示灯，左侧留出舒适的 14px 内边距）
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

/// 读取系统剪贴板文本（Windows 走 Win32 API 零依赖无额外分配，跨平台回退 None）
#[cfg(windows)]
pub fn get_clipboard_text() -> Option<String> {
    use std::ptr::null_mut;
    unsafe extern "system" {
        fn OpenClipboard(hWndNewOwner: *mut std::ffi::c_void) -> i32;
        fn CloseClipboard() -> i32;
        fn GetClipboardData(uFormat: u32) -> *mut std::ffi::c_void;
        fn GlobalLock(hMem: *mut std::ffi::c_void) -> *mut u16;
        fn GlobalUnlock(hMem: *mut std::ffi::c_void) -> i32;
    }
    const CF_UNICODETEXT: u32 = 13;
    unsafe {
        if OpenClipboard(null_mut()) == 0 {
            return None;
        }
        let handle = GetClipboardData(CF_UNICODETEXT);
        if handle.is_null() {
            CloseClipboard();
            return None;
        }
        let ptr = GlobalLock(handle);
        if ptr.is_null() {
            CloseClipboard();
            return None;
        }
        let mut len = 0;
        while *ptr.add(len) != 0 {
            len += 1;
        }
        let slice = std::slice::from_raw_parts(ptr, len);
        let s = String::from_utf16_lossy(slice);
        GlobalUnlock(handle);
        CloseClipboard();
        Some(s)
    }
}

/// 将文本写入系统剪贴板（Windows 走 Win32 原生 API，零延迟、零依赖、立即生效）
#[cfg(windows)]
pub fn set_clipboard_text(text: &str) -> bool {
    use std::ptr::null_mut;
    unsafe extern "system" {
        fn OpenClipboard(hWndNewOwner: *mut std::ffi::c_void) -> i32;
        fn CloseClipboard() -> i32;
        fn EmptyClipboard() -> i32;
        fn SetClipboardData(uFormat: u32, hMem: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
        fn GlobalAlloc(uFlags: u32, dwBytes: usize) -> *mut std::ffi::c_void;
        fn GlobalLock(hMem: *mut std::ffi::c_void) -> *mut u16;
        fn GlobalUnlock(hMem: *mut std::ffi::c_void) -> i32;
        fn GlobalFree(hMem: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    }
    const CF_UNICODETEXT: u32 = 13;
    const GMEM_MOVEABLE: u32 = 0x0002;

    let utf16: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let bytes_len = utf16.len() * std::mem::size_of::<u16>();

    unsafe {
        if OpenClipboard(null_mut()) == 0 {
            return false;
        }
        EmptyClipboard();
        let h_mem = GlobalAlloc(GMEM_MOVEABLE, bytes_len);
        if h_mem.is_null() {
            CloseClipboard();
            return false;
        }
        let ptr = GlobalLock(h_mem);
        if ptr.is_null() {
            GlobalFree(h_mem);
            CloseClipboard();
            return false;
        }
        std::ptr::copy_nonoverlapping(utf16.as_ptr(), ptr, utf16.len());
        GlobalUnlock(h_mem);
        if SetClipboardData(CF_UNICODETEXT, h_mem).is_null() {
            GlobalFree(h_mem);
            CloseClipboard();
            return false;
        }
        CloseClipboard();
        true
    }
}

#[cfg(not(windows))]
pub fn get_clipboard_text() -> Option<String> {
    None
}

#[cfg(not(windows))]
pub fn set_clipboard_text(_text: &str) -> bool {
    false
}





