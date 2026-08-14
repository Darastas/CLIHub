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
use egui::{Align2, Color32, FontId, Id, Modifiers, Pos2, Rect, RichText, Sense, Stroke, Ui, vec2};

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
}

impl TermTheme {
    pub fn from_scheme(name: &str) -> Self {
        let mut theme = match name {
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

    // ---- 标签栏（Tab Bar，类似 Chrome/VS Code）----
    let tab_h = 32.0;
    ui.add_space(10.0);
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
                    ui.add_space(4.0);
                }
                // 新增实例
                let plus = egui::Button::new(RichText::new("＋").size(15.0)).frame(false);
                if ui
                    .add(plus)
                    .on_hover_text("Start a new instance")
                    .clicked()
                {
                    action = Some(TerminalAction::NewTab);
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
        // 点击终端区域或切换 session 后自动获取焦点，确保 IME 立即可用
        if resp.clicked() || (input_enabled && !resp.has_focus()) {
            resp.request_focus();
        }
        let focused = resp.has_focus();

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
        

        if let Some(pos) = resp.interact_pointer_pos() {
            if grid_rect.contains(pos) {
                let col = ((pos.x - grid_rect.min.x) / col_w).floor().max(0.0) as usize;
                let line = ((pos.y - grid_rect.min.y) / row_h).floor().max(0.0) as usize;
                
                let gp = crate::backend::terminal::GridPoint { line, col };
                
                if let Some(t) = &mut tab.terminal {
                    if resp.drag_started() {
                        t.selection = Some(crate::backend::terminal::SelectionRange { start: gp, end: gp });
                    } else if resp.dragged() {
                        if let Some(sel) = &mut t.selection {
                            sel.end = gp;
                        }
                    }
                }
            }
        }
        
        if resp.clicked() {
            if let Some(t) = &mut tab.terminal {
                t.selection = None;
            }
        }

        if let Some(t) = &mut tab.terminal {
            t.resize(cols, rows);
        }
        if let Some(p) = &mut tab.pty {
            p.resize(cols, rows);
        }

        // 输入转发（由 App 控制开关，与焦点无关）+ 滚轮
        if input_enabled {
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

        handle_scroll(ui, tab, resp.hovered(), grid_rect);

        // 渲染网格
        if let Some(t) = &mut tab.terminal {
            paint_grid(ui, theme, t, grid_rect, col_w, row_h, cols, rows, focused);
            
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
            let (_fg, bg) = resolve_cell(cell, colors, theme);
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
        
        // 3.5) 选区背景
        if let Some(sel) = terminal.selection {
            let mut s_line = sel.start.line;
            let mut e_line = sel.end.line;
            let mut s_col = sel.start.col;
            let mut e_col = sel.end.col;
            if s_line > e_line || (s_line == e_line && s_col > e_col) {
                std::mem::swap(&mut s_line, &mut e_line);
                std::mem::swap(&mut s_col, &mut e_col);
            }
            
            if li >= s_line && li <= e_line {
                let sc = if li == s_line { s_col } else { 0 };
                let ec = if li == e_line { e_col } else { cols as usize - 1 };
                
                let x0 = origin.x + sc as f32 * col_w;
                let x1 = origin.x + (ec + 1) as f32 * col_w;
                let sel_bg = if theme.is_dark() { Color32::from_white_alpha(50) } else { Color32::from_rgb(186, 212, 255) };
                painter.rect_filled(
                    Rect::from_min_max(Pos2::new(x0, y), Pos2::new(x1, y + row_h)),
                    0.0,
                    sel_bg,
                );
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

fn luminance(c: Color32) -> f32 {
    0.2126 * (c.r() as f32) + 0.7152 * (c.g() as f32) + 0.0722 * (c.b() as f32)
}

fn is_graphic_char(c: char) -> bool {
    // 常见的 Unicode 绘图字符、色块、方块（ANSI 图形/Logo 使用）
    matches!(
        c,
        ' ' | '█' | '▀' | '▄' | '▌' | '▐' | '■' | '▲' | '▼' | '◆' | '●' | '░' | '▒' | '▓'
            | '┌' | '┐' | '└' | '┘' | '├' | '┤' | '┬' | '┴' | '┼' | '─' | '│'
    )
}

fn resolve_cell(cell: &Cell, colors: &Colors, theme: &TermTheme) -> (Color32, Color32) {
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

fn resolve_color(color: &TermColor, colors: &Colors, theme: &TermTheme, fallback: Color32) -> Color32 {
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

fn rgb_to_color32(rgb: Rgb) -> Color32 {
    Color32::from_rgb(rgb.r, rgb.g, rgb.b)
}

// ---------------------------------------------------------------------------
// 输入
// ---------------------------------------------------------------------------

fn forward_keys(ui: &mut Ui, tab: &mut TerminalInstance) -> Option<String> {
    let events = ui.input(|i| i.events.clone());
    let mut out: Vec<u8> = Vec::new();
    
    for ev in events {
        // 临时记录所有事件到文件，方便排查输入法重发问题
        if let egui::Event::Ime(_) | egui::Event::Text(_) | egui::Event::Key { .. } = &ev {
            use std::io::Write;
            if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open("d:\\Terminal desk\\events.log") {
                let _ = writeln!(f, "EVENT: {:?}", ev);
            }
        }
        
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
                if let Some(sel) = tab.terminal.as_ref().and_then(|t| t.selected_text()) {
                    if !sel.is_empty() {
                        ui.ctx().copy_text(sel);
                        continue;
                    }
                }
                out.extend_from_slice(b"\x03"); // Ctrl+C 兜底
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
                if modifiers.ctrl && key == egui::Key::C && tab.terminal.as_ref().map_or(false, |t| t.selection.is_some()) {
                    if let Some(sel) = tab.terminal.as_ref().and_then(|t| t.selected_text()) {
                        if !sel.is_empty() {
                            ui.ctx().copy_text(sel);
                            continue;
                        }
                    }
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

fn handle_scroll(ui: &mut Ui, tab: &mut TerminalInstance, hovered: bool, rect: Rect) {
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
            t.scroll_display(lines);
        }
    }
}

/// 绘制一个标签页，返回点击/关闭对应的动作。
fn draw_tab(
    ui: &mut Ui,
    session: &Session,
    ti: usize,
    is_active: bool,
    theme: &TermTheme,
) -> Option<TerminalAction> {
    let label = format!("{} {}", session.name, ti + 1);
    let font_id = FontId::proportional(12.5);
    let text_w = ui.painter().layout_no_wrap(label.clone(), font_id.clone(), Color32::WHITE).rect.width();
    let tab_w = text_w + 40.0;
    let tab_h = 32.0;
    let (tab_rect, resp) = ui.allocate_exact_size(vec2(tab_w, tab_h), Sense::click());

    // HeroUI style Pill tabs
    let bg = if is_active {
        theme.background
    } else if resp.hovered() {
        if theme.is_dark() {
            Color32::from_gray(39) // Zinc-800
        } else {
            Color32::from_rgb(228, 228, 231) // Zinc-200
        }
    } else {
        Color32::TRANSPARENT
    };
    if bg != Color32::TRANSPARENT {
        ui.painter().rect_filled(tab_rect, 16.0, bg); // Pill shape
    }

    let label_color = if is_active {
        theme.foreground
    } else if theme.is_dark() {
        Color32::from_gray(180)
    } else {
        Color32::from_gray(90)
    };
    ui.painter().text(
        Pos2::new(tab_rect.min.x + 12.0, tab_rect.center().y),
        Align2::LEFT_CENTER,
        &label,
        font_id,
        label_color,
    );

    // 关闭按钮
    let close_rect = Rect::from_center_size(
        Pos2::new(tab_rect.right() - 16.0, tab_rect.center().y),
        vec2(16.0, 16.0),
    );
    let close_resp = ui.interact(close_rect, Id::new(("tab-close", ti)), Sense::click());
    
    if close_resp.hovered() {
        ui.painter().rect_filled(close_rect, 4.0, Color32::from_rgb(220, 60, 50));
    }
    
    let close_color = if close_resp.hovered() {
        Color32::WHITE
    } else if theme.is_dark() {
        Color32::from_gray(140)
    } else {
        Color32::from_gray(120)
    };
    
    let center = close_rect.center();
    let d = 3.5;
    ui.painter().line_segment(
        [center + vec2(-d, -d), center + vec2(d, d)],
        egui::Stroke::new(1.5, close_color),
    );
    ui.painter().line_segment(
        [center + vec2(-d, d), center + vec2(d, -d)],
        egui::Stroke::new(1.5, close_color),
    );
    
    if close_resp.clicked() {
        return Some(TerminalAction::KillTab(ti));
    }
    if resp.clicked() {
        return Some(TerminalAction::SwitchTab(ti));
    }
    None
}
