//! 终端字符网格渲染（基于 alacritty 字符网格与 egui 高性能绘制）。

use alacritty_terminal::term::cell::{Cell, Flags};
use alacritty_terminal::term::color::Colors;
use alacritty_terminal::vte::ansi::{Color as TermColor, Rgb};
use egui::{Align2, Color32, FontId, Pos2, Rect, Ui, vec2};

use super::TermTheme;

pub fn paint_grid(
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

pub fn luminance(c: Color32) -> f32 {
    0.2126 * (c.r() as f32) + 0.7152 * (c.g() as f32) + 0.0722 * (c.b() as f32)
}

pub fn is_graphic_char(c: char) -> bool {
    // 常见的 Unicode 绘图字符、色块、方块（ANSI 图形/Logo 使用）
    matches!(
        c,
        ' ' | '█' | '▀' | '▄' | '▌' | '▐' | '■' | '▲' | '▼' | '◆' | '●' | '░' | '▒' | '▓'
            | '┌' | '┐' | '└' | '┘' | '├' | '┤' | '┬' | '┴' | '┼' | '─' | '│'
    )
}

pub fn resolve_cell(cell: &Cell, colors: &Colors, theme: &TermTheme) -> (Color32, Color32) {
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

pub fn resolve_color(color: &TermColor, colors: &Colors, theme: &TermTheme, fallback: Color32) -> Color32 {
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

pub fn rgb_to_color32(rgb: Rgb) -> Color32 {
    Color32::from_rgb(rgb.r, rgb.g, rgb.b)
}
