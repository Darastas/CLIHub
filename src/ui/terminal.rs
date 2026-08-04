//! 右侧主区域：基于 alacritty 字符网格的终端渲染 + 原始按键转发。
//!
//! - 渲染：每行先画背景色块（按连续同色合并），再按 (fg/bold/underline)
//!   分组生成 LayoutJob 画字；宽字符（CJK）由 WIDE_CHAR 标志处理。
//! - 输入：终端区获得焦点后，把 `Event::Text/Key/Paste` 转成字节流写回 PTY。
//! - 缩放：按面板尺寸 × 字体度量计算行列数，同步 PTY 与网格。

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
    pub fn light() -> Self {
        Self {
            font_size: 15.0,
            font_family: egui::FontFamily::Monospace,
            bold_family: egui::FontFamily::Monospace,
            background: Color32::from_rgb(255, 255, 255),
            foreground: Color32::from_rgb(71, 85, 105), // Slate 600
            cursor: Color32::from_rgb(99, 102, 241), // Indigo 500
            ansi: [
                Color32::from_rgb(30, 41, 59),    // black
                Color32::from_rgb(239, 68, 68),   // red
                Color32::from_rgb(34, 197, 94),   // green
                Color32::from_rgb(234, 179, 8),   // yellow
                Color32::from_rgb(59, 130, 246),  // blue
                Color32::from_rgb(168, 85, 247),  // magenta
                Color32::from_rgb(6, 182, 212),   // cyan
                Color32::from_rgb(148, 163, 184), // white
                Color32::from_rgb(100, 116, 139), // bright black
                Color32::from_rgb(248, 113, 113), // bright red
                Color32::from_rgb(74, 222, 128),  // bright green
                Color32::from_rgb(253, 224, 71),  // bright yellow
                Color32::from_rgb(96, 165, 250),  // bright blue
                Color32::from_rgb(192, 132, 252), // bright magenta
                Color32::from_rgb(34, 211, 238),  // bright cyan
                Color32::from_rgb(203, 213, 225), // bright white
            ],
        }
    }

    /// 暗色主题：VS Code 风格中性深灰，不偏色，最能承载 ANSI 彩色输出。
    pub fn dark() -> Self {
        Self {
            font_size: 15.0,
            font_family: egui::FontFamily::Monospace,
            bold_family: egui::FontFamily::Monospace,
            background: Color32::from_rgb(30, 30, 30),    // #1E1E1E
            foreground: Color32::from_rgb(212, 212, 212), // #D4D4D4
            cursor: Color32::from_rgb(174, 175, 173),
            ansi: [
                Color32::from_rgb(0, 0, 0),       // black
                Color32::from_rgb(205, 49, 49),   // red
                Color32::from_rgb(13, 188, 121),  // green
                Color32::from_rgb(229, 229, 16),  // yellow
                Color32::from_rgb(36, 114, 200),  // blue
                Color32::from_rgb(188, 63, 188),  // magenta
                Color32::from_rgb(17, 168, 205),  // cyan
                Color32::from_rgb(229, 229, 229), // white
                Color32::from_rgb(102, 102, 102), // bright black
                Color32::from_rgb(241, 76, 76),   // bright red
                Color32::from_rgb(35, 209, 139),  // bright green
                Color32::from_rgb(245, 245, 67),  // bright yellow
                Color32::from_rgb(59, 142, 234),  // bright blue
                Color32::from_rgb(214, 112, 214), // bright magenta
                Color32::from_rgb(41, 184, 219),  // bright cyan
                Color32::from_rgb(255, 255, 255), // bright white
            ],
        }
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
                ui.add_space(8.0);
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
        if resp.clicked() {
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
        handle_scroll(ui, tab);

        // 渲染网格
        if let Some(t) = &tab.terminal {
            paint_grid(ui, theme, t, grid_rect, col_w, row_h, cols, rows, focused);
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
    let term = &terminal.term;
    let grid = term.grid();
    let colors = term.colors();
    let display_offset = grid.display_offset();
    let painter = ui.painter().with_clip_rect(rect);
    let origin = rect.min;

    // 收集可见行 → 每行的单元格
    let mut lines: Vec<Vec<&Cell>> = Vec::new();
    for item in grid.display_iter() {
        let li = item.point.line.0 as usize;
        if li >= lines.len() {
            lines.resize(li + 1, Vec::new());
        }
        lines[li].push(item.cell);
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

        // 2) 文本 span（连续非空格、同色、同粗体 → 一个 painter.text）
        let mut spans: Vec<(usize, String, Color32, bool)> = Vec::new();
        let mut cur_start: Option<usize> = None;
        let mut cur_str = String::new();
        let mut cur_color = Color32::TRANSPARENT;
        let mut cur_bold = false;
        for (ci, cell) in cells.iter().enumerate() {
            let flags = cell.flags;
            if flags.contains(Flags::WIDE_CHAR_SPACER) {
                continue;
            }
            let (mut fg, bg) = resolve_cell(cell, colors, theme);
            if flags.contains(Flags::INVERSE) {
                fg = bg;
            }
            let dim = flags.contains(Flags::DIM);
            let mut color = fg;
            if dim {
                color = Color32::from_rgb(fg.r() / 2, fg.g() / 2, fg.b() / 2);
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
                continue;
            }
            if cur_start.is_none() || cur_color != color || cur_bold != is_bold {
                if let Some(sc) = cur_start.take() {
                    spans.push((sc, std::mem::take(&mut cur_str), cur_color, cur_bold));
                }
                cur_start = Some(ci);
                cur_color = color;
                cur_bold = is_bold;
            }
            cur_str.push(ch);
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

    // ---- 光标 ----
    let cursor_point = grid.cursor.point;
    let viewport_line = cursor_point.line.0 - display_offset as i32;
    if viewport_line >= 0 && (viewport_line as usize) < lines.len() {
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

fn resolve_cell(cell: &Cell, colors: &Colors, theme: &TermTheme) -> (Color32, Color32) {
    let fg = resolve_color(&cell.fg, colors, theme, theme.foreground);
    let bg = resolve_color(&cell.bg, colors, theme, theme.background);
    (fg, bg)
}

fn resolve_color(color: &TermColor, colors: &Colors, theme: &TermTheme, fallback: Color32) -> Color32 {
    match color {
        TermColor::Named(n) => {
            let idx = *n as usize;
            if idx < 16 {
                theme.ansi[idx]
            } else {
                colors[*n].map(rgb_to_color32).unwrap_or(fallback)
            }
        }
        TermColor::Spec(rgb) => rgb_to_color32(*rgb),
        TermColor::Indexed(i) => colors[*i as usize].map(rgb_to_color32).unwrap_or(fallback),
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
        match ev {
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

fn handle_scroll(ui: &mut Ui, tab: &mut TerminalInstance) {
    let scroll = ui.input(|i| i.smooth_scroll_delta.y);
    if scroll == 0.0 {
        return;
    }
    let lines = (scroll / 40.0).round() as i32;
    if lines != 0 {
        if let Some(t) = &mut tab.terminal {
            t.scroll_display(-lines);
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

    // 背景：激活 = 终端底色（与内容区连成一体），非激活 = 透明/悬浮灰
    let bg = if is_active {
        theme.background
    } else if resp.hovered() {
        if theme.is_dark() {
            Color32::from_gray(45)
        } else {
            Color32::from_rgb(241, 245, 249)
        }
    } else {
        Color32::TRANSPARENT
    };
    if bg != Color32::TRANSPARENT {
        ui.painter().rect_filled(tab_rect, 8.0, bg);
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
