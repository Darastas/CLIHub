//! 右侧主区域：基于 alacritty 字符网格的终端渲染 + 原始按键转发。
//!
//! - 渲染：每行先画背景色块（按连续同色合并），再按 (fg/bold/underline)
//!   分组生成 LayoutJob 画字；宽字符（CJK）由 WIDE_CHAR 标志处理。
//! - 输入：终端区获得焦点后，把 `Event::Text/Key/Paste` 转成字节流写回 PTY。
//! - 缩放：按面板尺寸 × 字体度量计算行列数，同步 PTY 与网格。

use alacritty_terminal::event::VoidListener;
use alacritty_terminal::term::cell::{Cell, Flags};
use alacritty_terminal::term::color::Colors;
use alacritty_terminal::term::Term;
use alacritty_terminal::vte::ansi::{Color as TermColor, Rgb};
use egui::text::{LayoutJob, TextFormat};
use egui::{Align2, Color32, FontId, Modifiers, Pos2, Rect, RichText, Sense, Stroke, Ui, vec2};

use crate::state::Session;

/// 主区域顶部按钮可能触发的动作，由 App 层执行。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalAction {
    Restart,
    Kill,
}

/// 终端配色主题（浅色，贴近 Example.png 的白/灰极简风格）。
pub struct TermTheme {
    pub font_size: f32,
    pub background: Color32,
    pub foreground: Color32,
    pub cursor: Color32,
    pub ansi: [Color32; 16],
}

impl TermTheme {
    pub fn light() -> Self {
        Self {
            font_size: 13.0,
            background: Color32::from_rgb(255, 255, 255),
            foreground: Color32::from_rgb(31, 35, 40),
            cursor: Color32::from_rgb(9, 105, 218),
            ansi: [
                Color32::from_rgb(31, 35, 40),    // black
                Color32::from_rgb(194, 59, 34),   // red
                Color32::from_rgb(26, 127, 55),   // green
                Color32::from_rgb(154, 103, 0),   // yellow
                Color32::from_rgb(9, 105, 218),   // blue
                Color32::from_rgb(130, 80, 223),  // magenta
                Color32::from_rgb(27, 124, 131),  // cyan
                Color32::from_rgb(110, 119, 129), // white
                Color32::from_rgb(87, 96, 106),   // bright black
                Color32::from_rgb(229, 83, 75),   // bright red
                Color32::from_rgb(45, 164, 78),   // bright green
                Color32::from_rgb(191, 135, 0),   // bright yellow
                Color32::from_rgb(9, 105, 218),   // bright blue
                Color32::from_rgb(191, 57, 137),  // bright magenta
                Color32::from_rgb(49, 146, 170),  // bright cyan
                Color32::from_rgb(175, 184, 193), // bright white
            ],
        }
    }
}

pub fn show(ui: &mut Ui, session: &mut Session) -> Option<TerminalAction> {
    let mut action = None;
    let theme = TermTheme::light();
    let status = session.status();
    let interactive = session.is_interactive();

    // ---- 顶部信息栏 ----
    ui.horizontal(|ui| {
        ui.label(RichText::new(&session.name).strong().size(14.0));
        ui.label(RichText::new(super::status_dot(status)).color(super::status_color(status)));
        ui.label(
            RichText::new(session.cwd.display().to_string())
                .size(11.0)
                .color(Color32::from_gray(140)),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add_enabled(session.pty.is_some(), egui::Button::new("✕ Kill"))
                .clicked()
            {
                action = Some(TerminalAction::Kill);
            }
            if ui
                .add_enabled(!interactive, egui::Button::new("Restart"))
                .clicked()
            {
                action = Some(TerminalAction::Restart);
            }
        });
    });
    ui.separator();

    // ---- 错误提示 ----
    if let Some(err) = &session.error {
        ui.colored_label(Color32::from_rgb(190, 60, 50), err);
        ui.add_space(4.0);
    }

    // ---- 字体度量（决定每格宽高）----
    let font_id = FontId::monospace(theme.font_size);
    let (col_w, row_h) = ui.fonts_mut(|f| (f.glyph_width(&font_id, ' '), f.row_height(&font_id)));

    // ---- 终端区域 ----
    let hint_h = row_h + 6.0;
    let term_size = vec2(
        ui.available_width(),
        (ui.available_height() - hint_h).max(60.0),
    );
    let (term_rect, resp) = ui.allocate_exact_size(term_size, Sense::click());
    if resp.clicked() {
        resp.request_focus();
    }
    let focused = resp.has_focus();

    // 按实际面板尺寸换算行列数并同步
    let cols = ((term_rect.width() / col_w).floor().max(1.0)) as u16;
    let rows = ((term_rect.height() / row_h).floor().max(1.0)) as u16;
    if let Some(t) = &mut session.terminal {
        t.resize(cols, rows);
    }
    if let Some(p) = &mut session.pty {
        p.resize(cols, rows);
    }

    // 背景
    let painter = ui.painter().with_clip_rect(term_rect);
    painter.rect_filled(term_rect, 0.0, theme.background);

    // 输入转发 + 滚轮
    if focused {
        forward_keys(ui, session);
    }
    handle_scroll(ui, session);

    // 渲染网格
    match &session.terminal {
        Some(t) => {
            paint_grid(ui, &theme, &t.term, term_rect, col_w, row_h, cols, rows, focused);
        }
        None if session.error.is_none() => {
            painter.text(
                term_rect.min + vec2(10.0, 8.0),
                Align2::LEFT_TOP,
                "Session not started — click a session in the sidebar, or press Restart.",
                FontId::proportional(13.0),
                Color32::from_gray(150),
            );
        }
        None => {}
    }

    // ---- 快捷提示栏 ----
    ui.allocate_space(vec2(0.0, hint_h));
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("click terminal to type · Ctrl+C interrupt · wheel scrolls history · ? for shortcuts")
                .size(10.5)
                .color(Color32::from_gray(150)),
        );
    });

    action
}

// ---------------------------------------------------------------------------
// 渲染
// ---------------------------------------------------------------------------

fn paint_grid(
    ui: &mut Ui,
    theme: &TermTheme,
    term: &Term<VoidListener>,
    rect: Rect,
    col_w: f32,
    row_h: f32,
    cols: u16,
    rows: u16,
    focused: bool,
) {
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

        let mut runs: Vec<(usize, usize, Color32)> = Vec::new();
        let mut job = LayoutJob::default();
        job.wrap.max_width = f32::INFINITY;
        let mut has_bold = false;

        for (ci, cell) in cells.iter().enumerate() {
            let flags = cell.flags;
            // 宽字符第二列占位格，不单独渲染
            if flags.contains(Flags::WIDE_CHAR_SPACER) {
                continue;
            }
            let wide = flags.contains(Flags::WIDE_CHAR);
            let width = if wide { 2 } else { 1 };

            let (mut fg, mut bg) = resolve_cell(cell, colors, theme);
            if flags.contains(Flags::INVERSE) {
                std::mem::swap(&mut fg, &mut bg);
            }

            // 背景色块：与前一格同色则延伸
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

            // 文本
            let is_bold = flags.contains(Flags::BOLD);
            if is_bold {
                has_bold = true;
            }
            let dim = flags.contains(Flags::DIM);
            let ch = if flags.contains(Flags::HIDDEN) {
                ' '
            } else {
                cell.c
            };
            let mut color = fg;
            if dim {
                color = Color32::from_rgb(fg.r() / 2, fg.g() / 2, fg.b() / 2);
            }
            job.append(
                &ch.to_string(),
                0.0,
                TextFormat {
                    font_id: FontId::monospace(theme.font_size),
                    color,
                    underline: if flags.contains(Flags::UNDERLINE) {
                        Stroke::new(1.0, color)
                    } else {
                        Stroke::NONE
                    },
                    strikethrough: if flags.contains(Flags::STRIKEOUT) {
                        Stroke::new(1.0, color)
                    } else {
                        Stroke::NONE
                    },
                    line_height: Some(row_h),
                    ..Default::default()
                },
            );
        }

        // 画背景
        for (s, e, c) in &runs {
            let x0 = origin.x + *s as f32 * col_w;
            let x1 = origin.x + *e as f32 * col_w;
            painter.rect_filled(
                Rect::from_min_max(Pos2::new(x0, y), Pos2::new(x1, y + row_h)),
                0.0,
                *c,
            );
        }

        // 画文字
        if !job.sections.is_empty() {
            let galley = ui.fonts_mut(|f| f.layout_job(job));
            painter.galley(Pos2::new(origin.x, y), galley.clone(), theme.foreground);
            if has_bold {
                // 简易伪粗体：同 galley 偏移 1px 再画一遍
                painter.galley(Pos2::new(origin.x + 1.0, y), galley, theme.foreground);
            }
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
                    FontId::monospace(theme.font_size),
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

fn forward_keys(ui: &mut Ui, session: &mut Session) {
    let events = ui.input(|i| i.events.clone());
    let mut out: Vec<u8> = Vec::new();
    for ev in events {
        match ev {
            egui::Event::Copy => out.extend_from_slice(b"\x03"), // Ctrl+C 兜底（部分平台）
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
                if let Some(bytes) = map_key(key, &modifiers) {
                    out.extend_from_slice(&bytes);
                }
            }
            _ => {}
        }
    }
    if !out.is_empty() {
        if let Some(pty) = &mut session.pty {
            if let Err(e) = pty.write(&out) {
                session.error = Some(format!("写入 PTY 失败: {e}"));
            }
        }
    }
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

fn handle_scroll(ui: &mut Ui, session: &mut Session) {
    let scroll = ui.input(|i| i.smooth_scroll_delta.y);
    if scroll == 0.0 {
        return;
    }
    let lines = (scroll / 40.0).round() as i32;
    if lines != 0 {
        if let Some(t) = &mut session.terminal {
            t.scroll_display(-lines);
        }
    }
}
