//! 终端状态机：封装 `alacritty_terminal` 维护字符网格。
//!
//! PTY 输出的原始字节先经 VTE 处理器（`ansi::Processor`）解析，
//! 更新网格状态；UI 层每帧读取网格进行渲染。
//!
//! 关键点：alacritty 会把**需要写回 PTY 的应答**（如 DSR 光标位置
//! 查询 `ESC[6n`、XTWINOPS 尺寸查询）经 `EventListener` 发出来。
//! 若用 `VoidListener` 丢弃，claude 这类 TUI 会一直等应答而卡死。
//! 因此这里用 [`HubListener`] 经通道转发给应用层写回 PTY。

use std::sync::{Arc, Mutex};

use alacritty_terminal::event::{Event, EventListener, WindowSize};
use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::term::{Config, Term};
use alacritty_terminal::vte::ansi::{Processor, Rgb};
use alacritty_terminal::index::{Column, Line};
use crossbeam_channel::{unbounded, Receiver, Sender};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridPoint {
    /// 实际网格缓冲区行号（Line(-history_size)..Line(screen_lines - 1)）
    pub line: i32,
    pub col: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectionRange {
    pub start: GridPoint,
    pub end: GridPoint,
}

/// 终端内搜索到的单个匹配项坐标。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SearchMatch {
    /// 实际网格行号（Line(-history_size)..Line(screen_lines - 1)）
    pub line: i32,
    /// 匹配起始列（包含）
    pub col_start: usize,
    /// 匹配结束列（包含）
    pub col_end: usize,
}

use alacritty_terminal::vte::ansi::NamedColor;

#[derive(Debug, Clone, Copy)]
pub struct TermThemeColors {
    pub foreground: Rgb,
    pub background: Rgb,
    pub cursor: Rgb,
    pub ansi: [Rgb; 16],
}

impl Default for TermThemeColors {
    fn default() -> Self {
        Self {
            foreground: Rgb { r: 56, g: 58, b: 66 },
            background: Rgb { r: 250, g: 250, b: 250 },
            cursor: Rgb { r: 191, g: 193, b: 200 },
            ansi: [
                Rgb { r: 56, g: 58, b: 66 },
                Rgb { r: 228, g: 86, b: 73 },
                Rgb { r: 80, g: 161, b: 79 },
                Rgb { r: 193, g: 132, b: 1 },
                Rgb { r: 1, g: 132, b: 188 },
                Rgb { r: 166, g: 38, b: 164 },
                Rgb { r: 9, g: 151, b: 179 },
                Rgb { r: 250, g: 250, b: 250 },
                Rgb { r: 79, g: 82, b: 93 },
                Rgb { r: 228, g: 86, b: 73 },
                Rgb { r: 80, g: 161, b: 79 },
                Rgb { r: 193, g: 132, b: 1 },
                Rgb { r: 1, g: 132, b: 188 },
                Rgb { r: 166, g: 38, b: 164 },
                Rgb { r: 9, g: 151, b: 179 },
                Rgb { r: 250, g: 250, b: 250 },
            ],
        }
    }
}

/// 终端内部产生的事件（供通知系统及 UI 层使用）。
#[derive(Debug, Clone)]
pub enum TerminalEvent {
    /// 终端响铃 (Bell: \a, \x07)，通常表示 AI 任务等待确认/完成或命令结束
    Bell,
    /// 标题变更
    #[allow(dead_code)]
    Title(String),
}

/// 把 alacritty 需要写回 PTY 的事件转发给应用层。
pub struct HubListener {
    pty_tx: Sender<String>,
    event_tx: Sender<TerminalEvent>,
    /// 当前 (cols, rows)，供尺寸查询应答
    size: Arc<Mutex<(usize, usize)>>,
    /// 主题颜色配置，供 OSC 颜色查询应答
    theme_colors: Arc<Mutex<TermThemeColors>>,
}

impl EventListener for HubListener {
    fn send_event(&self, event: Event) {
        match event {
            // 终端应答（如 DSR `ESC[6n`）必须写回 PTY
            Event::PtyWrite(text) => {
                let _ = self.pty_tx.send(text);
            }
            // 终端响铃（Bell: \a 或 \x07）
            Event::Bell => {
                let _ = self.event_tx.send(TerminalEvent::Bell);
            }
            // 窗口标题变更
            Event::Title(title) => {
                let _ = self.event_tx.send(TerminalEvent::Title(title));
            }
            // 查询窗口/单元格尺寸（XTWINOPS）
            Event::TextAreaSizeRequest(formatter) => {
                let (cols, rows) = *self.size.lock().unwrap();
                let size = WindowSize {
                    num_lines: rows as u16,
                    num_cols: cols as u16,
                    cell_width: 8,
                    cell_height: 16,
                };
                let _ = self.pty_tx.send(formatter(size));
            }
            // OSC 颜色查询：按 index 分别以对应颜色（前景/背景/光标/调色板）应答
            Event::ColorRequest(index, formatter) => {
                let tc = *self.theme_colors.lock().unwrap();
                let rgb = if index == NamedColor::Background as usize {
                    tc.background
                } else if index == NamedColor::Foreground as usize {
                    tc.foreground
                } else if index == NamedColor::Cursor as usize {
                    tc.cursor
                } else if index < 16 {
                    tc.ansi[index]
                } else {
                    tc.background
                };
                let _ = self.pty_tx.send(formatter(rgb));
            }
            _ => {}
        }
    }
}

/// 行列尺寸（实现 alacritty 的 `Dimensions` trait）。
pub struct TermDims {
    pub cols: usize,
    pub rows: usize,
}

impl Dimensions for TermDims {
    fn total_lines(&self) -> usize {
        self.rows
    }

    fn screen_lines(&self) -> usize {
        self.rows
    }

    fn columns(&self) -> usize {
        self.cols
    }
}

/// alacritty 终端 + VTE 解析器 + PTY 应答转发。
pub struct Terminal {
    pub term: Term<HubListener>,
    processor: Processor,
    pty_rx: Receiver<String>,
    event_rx: Receiver<TerminalEvent>,
    size: Arc<Mutex<(usize, usize)>>,
    pub theme_colors: Arc<Mutex<TermThemeColors>>,
    pub cols: u16,
    pub rows: u16,
    pub selection: Option<SelectionRange>,
}

impl Terminal {
    pub fn new(cols: u16, rows: u16, theme_colors: TermThemeColors) -> Self {
        let (pty_tx, pty_rx) = unbounded();
        let (event_tx, event_rx) = unbounded();
        let size = Arc::new(Mutex::new((cols as usize, rows as usize)));
        let theme_colors = Arc::new(Mutex::new(theme_colors));
        let listener = HubListener {
            pty_tx,
            event_tx,
            size: size.clone(),
            theme_colors: theme_colors.clone(),
        };
        let term = Term::new(Config::default(), &TermDims { cols: cols as usize, rows: rows as usize }, listener);
        
        Self {
            term,
            processor: Processor::new(),
            pty_rx,
            event_rx,
            size,
            theme_colors,
            cols,
            rows,
            selection: None,
        }
    }

    /// 拉取终端内部产生的事件（如 Bell 响铃、标题更新）
    pub fn drain_events(&mut self) -> Vec<TerminalEvent> {
        let mut out = Vec::new();
        while let Ok(evt) = self.event_rx.try_recv() {
            out.push(evt);
        }
        out
    }

    /// 把 PTY 输出字节喂给解析器，更新网格状态。
    pub fn feed(&mut self, bytes: &[u8]) {
        self.processor.advance(&mut self.term, bytes);
    }

    /// 直接写入一段文本（用于会话结束等提示，走同一解析管线）。
    pub fn feed_text(&mut self, text: &str) {
        self.feed(text.as_bytes());
    }

    pub fn resize(&mut self, cols: u16, rows: u16) {
        if cols == 0 || rows == 0 {
            return;
        }
        if self.cols == cols && self.rows == rows {
            return;
        }
        self.cols = cols;
        self.rows = rows;
        *self.size.lock().unwrap() = (cols as usize, rows as usize);
        self.term.resize(TermDims {
            cols: cols as usize,
            rows: rows as usize,
        });
    }

    /// 滚动历史缓冲区（正值向上翻，负值向下翻）。
    pub fn scroll_display(&mut self, lines: i32) {
        if lines == 0 {
            return;
        }
        self.term.scroll_display(Scroll::Delta(lines));
    }

    /// 取出 alacritty 要求写回 PTY 的应答文本（DSR 光标位置等）。
    pub fn drain_pty_writes(&mut self) -> Vec<String> {
        let mut out = Vec::new();
        while let Ok(s) = self.pty_rx.try_recv() {
            out.push(s);
        }
        out
    }

    #[allow(dead_code)] // Phase 5 字体适配使用
    pub fn dimensions(&self) -> (u16, u16) {
        (self.cols, self.rows)
    }

    pub fn selected_text(&self) -> Option<String> {
        let sel = self.selection?;
        
        let mut start_line = sel.start.line;
        let mut end_line = sel.end.line;
        let mut start_col = sel.start.col;
        let mut end_col = sel.end.col;
        
        if start_line > end_line || (start_line == end_line && start_col > end_col) {
            std::mem::swap(&mut start_line, &mut end_line);
            std::mem::swap(&mut start_col, &mut end_col);
        }
        
        let grid = self.term.grid();
        let history = grid.history_size() as i32;
        let screen = grid.screen_lines() as i32;
        
        let mut line_texts = Vec::new();
        for actual_line in start_line..=end_line {
            if actual_line < -history || actual_line >= screen {
                continue;
            }
            let row = &grid[Line(actual_line)];
            
            // 动态计算当前行真实文本内容的末尾列
            let mut content_end = 0;
            for col in 0..self.cols as usize {
                if col >= row.len() { break; }
                let cell = &row[Column(col)];
                if cell.c != ' ' && !cell.flags.contains(alacritty_terminal::term::cell::Flags::HIDDEN) {
                    let w = if cell.flags.contains(alacritty_terminal::term::cell::Flags::WIDE_CHAR) { 2 } else { 1 };
                    content_end = (col + w).min(self.cols as usize);
                }
            }

            let (sc, ec) = if start_line == end_line {
                (start_col, (end_col + 1).min(content_end))
            } else if actual_line == start_line {
                (start_col, content_end)
            } else if actual_line == end_line {
                (0, (end_col + 1).min(content_end))
            } else {
                (0, content_end)
            };

            if sc >= ec {
                if start_line != end_line && actual_line != end_line {
                    line_texts.push(String::new());
                }
                continue;
            }
            
            let mut line_str = String::new();
            let mut trailing_spaces = 0;
            for c in sc..ec {
                if c >= row.len() { break; }
                let cell = &row[Column(c)];
                if cell.flags.contains(alacritty_terminal::term::cell::Flags::WIDE_CHAR_SPACER) { continue; }
                if cell.c == ' ' {
                    trailing_spaces += 1;
                } else {
                    for _ in 0..trailing_spaces { line_str.push(' '); }
                    trailing_spaces = 0;
                    if !cell.flags.contains(alacritty_terminal::term::cell::Flags::HIDDEN) {
                        line_str.push(cell.c);
                    }
                }
            }
            line_texts.push(line_str);
        }
        Some(line_texts.join("\n"))
    }

    /// 在终端回滚历史与当前屏幕网格中搜索关键词。
    pub fn search(&self, query: &str, case_sensitive: bool) -> Vec<SearchMatch> {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return Vec::new();
        }

        let query_chars: Vec<char> = if case_sensitive {
            trimmed.chars().collect()
        } else {
            trimmed.to_lowercase().chars().collect()
        };
        let query_len = query_chars.len();
        if query_len == 0 {
            return Vec::new();
        }

        let grid = self.term.grid();
        let history = grid.history_size() as i32;
        let screen = grid.screen_lines() as i32;
        let cols = self.cols as usize;

        let mut matches = Vec::new();

        // 遍历所有历史行与当前屏幕行 (从最顶端的历史行 -history 到屏幕底端 screen - 1)
        for actual_line in -history..screen {
            let row = &grid[Line(actual_line)];

            // 提取该行字符与列索引映射 (跳过宽字符占位符)
            let mut chars = Vec::new();
            let mut col_indices = Vec::new();

            for col in 0..cols {
                if col >= row.len() {
                    break;
                }
                let cell = &row[Column(col)];
                if cell.flags.contains(alacritty_terminal::term::cell::Flags::WIDE_CHAR_SPACER) {
                    continue;
                }
                let c = if case_sensitive {
                    cell.c
                } else {
                    cell.c.to_lowercase().next().unwrap_or(cell.c)
                };
                chars.push(c);
                col_indices.push(col);
            }

            if chars.len() < query_len {
                continue;
            }

            // 字符切片匹配
            for i in 0..=(chars.len() - query_len) {
                if chars[i..i + query_len] == query_chars[..] {
                    let col_start = col_indices[i];
                    let end_char = i + query_len - 1;
                    let col_end_base = col_indices[end_char];

                    let col_end = if end_char + 1 < col_indices.len() {
                        col_indices[end_char + 1] - 1
                    } else {
                        (col_end_base + 1).min(cols - 1)
                    };

                    matches.push(SearchMatch {
                        line: actual_line,
                        col_start,
                        col_end: col_end.max(col_start),
                    });
                }
            }
        }

        matches
    }

    /// 滚动视口使某个搜索匹配项呈现在屏幕可视区域中。
    pub fn scroll_to_match(&mut self, m: &SearchMatch) {
        let screen = self.term.grid().screen_lines() as i32;
        let history = self.term.grid().history_size() as usize;

        // 计算目标行相对屏幕的行号：我们希望目标行显示在屏幕中间偏上位置（比如 1/3 处）
        let desired_screen_row = (screen / 3).max(0);
        let target_display_offset = (desired_screen_row - m.line).clamp(0, history as i32) as usize;

        // 重置后滚动到 target_display_offset
        let current = self.term.grid().display_offset();
        if target_display_offset > current {
            self.term.scroll_display(alacritty_terminal::grid::Scroll::Delta((target_display_offset - current) as i32));
        } else if target_display_offset < current {
            self.term.scroll_display(alacritty_terminal::grid::Scroll::Delta(-((current - target_display_offset) as i32)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alacritty_terminal::index::{Column, Line};

    /// 把一行单元格拼成字符串。
    fn row_text(
        grid: &alacritty_terminal::Grid<alacritty_terminal::term::cell::Cell>,
        line: usize,
    ) -> String {
        let row = &grid[Line(line as i32)];
        let mut s = String::new();
        for i in 0..row.len() {
            s.push(row[Column(i)].c);
        }
        s
    }

    /// 喂入普通文本应出现在网格中。
    #[test]
    fn feed_updates_grid() {
        let mut t = Terminal::new(40, 10, TermThemeColors::default());
        t.feed(b"hello\r\nworld");

        let grid = t.term.grid();
        assert_eq!(row_text(grid, 0).trim_end(), "hello");
        assert_eq!(row_text(grid, 1).trim_end(), "world");
    }

    /// ANSI 颜色序列应被解析，不影响字符本身。
    #[test]
    fn ansi_processed() {
        let mut t = Terminal::new(40, 10, TermThemeColors::default());
        t.feed(b"\x1b[31mRED\x1b[0m");
        let cell = &t.term.grid()[Line(0)][Column(0)];
        assert_eq!(cell.c, 'R');
    }

    /// resize 后网格行列随之变化。
    #[test]
    fn resize_changes_grid() {
        let mut t = Terminal::new(10, 5, TermThemeColors::default());
        assert_eq!(t.term.grid().screen_lines(), 5);
        assert_eq!(t.term.grid().columns(), 10);
        t.resize(20, 8);
        assert_eq!(t.term.grid().screen_lines(), 8);
        assert_eq!(t.term.grid().columns(), 20);
    }

    /// DSR 查询（`ESC[6n`）应产生写回 PTY 的应答，而不是被丢弃。
    #[test]
    fn dsr_produces_pty_write() {
        let mut t = Terminal::new(40, 10, TermThemeColors::default());
        // 模拟终端查询光标位置（claude 等 TUI 启动时会发这个）
        t.feed(b"\x1b[6n");
        let writes = t.drain_pty_writes();
        assert!(
            writes.iter().any(|w| w.starts_with("\x1b[") && w.ends_with('R')),
            "应产生 DSR 应答, 实际: {writes:?}"
        );
    }

    #[test]
    fn selected_text_basic_and_reverse() {
        let mut t = Terminal::new(40, 10, TermThemeColors::default());
        t.feed(b"Hello World\r\nRust Terminal\r\n");

        // 正向选择 "World" (line 0, col 6..10)
        t.selection = Some(SelectionRange {
            start: GridPoint { line: 0, col: 6 },
            end: GridPoint { line: 0, col: 10 },
        });
        assert_eq!(t.selected_text().as_deref(), Some("World"));

        // 反向选择 "World"
        t.selection = Some(SelectionRange {
            start: GridPoint { line: 0, col: 10 },
            end: GridPoint { line: 0, col: 6 },
        });
        assert_eq!(t.selected_text().as_deref(), Some("World"));
    }

    #[test]
    fn selected_text_chinese_wide_char() {
        let mut t = Terminal::new(40, 10, TermThemeColors::default());
        t.feed("你好世界\r\n".as_bytes());

        // 选择整个 "你好世界" (每个汉字占 2 列宽度，4个汉字共 8 列: 0..7)
        t.selection = Some(SelectionRange {
            start: GridPoint { line: 0, col: 0 },
            end: GridPoint { line: 0, col: 7 },
        });
        assert_eq!(t.selected_text().as_deref(), Some("你好世界"));
    }

    #[test]
    fn selected_text_with_scrollback() {
        let mut t = Terminal::new(40, 3, TermThemeColors::default());
        t.feed(b"line 1\r\nline 2\r\nline 3\r\nline 4\r\nline 5\r\n");
        
        // 3 行屏幕经 5 次换行后，屏幕显示 line 4, line 5, 空行。
        // 历史缓冲区中第 -1 行为 line 3，第 -2 行为 line 2，第 -3 行为 line 1
        t.selection = Some(SelectionRange {
            start: GridPoint { line: -1, col: 0 },
            end: GridPoint { line: -1, col: 5 },
        });
        assert_eq!(t.selected_text().as_deref(), Some("line 3"));

        t.selection = Some(SelectionRange {
            start: GridPoint { line: -2, col: 0 },
            end: GridPoint { line: -2, col: 5 },
        });
        assert_eq!(t.selected_text().as_deref(), Some("line 2"));
    }

    #[test]
    fn selected_text_multiline_stream() {
        let mut t = Terminal::new(40, 5, TermThemeColors::default());
        t.feed(b"first line\r\nsecond line\r\nthird line\r\n");

        // 跨行流式选择：从第 0 行 "line" (col 6) 到第 1 行 "second" (col 5)
        t.selection = Some(SelectionRange {
            start: GridPoint { line: 0, col: 6 },
            end: GridPoint { line: 1, col: 5 },
        });
        let text = t.selected_text().unwrap();
        assert_eq!(text, "line\nsecond");
    }

    #[test]
    fn search_basic_and_case_sensitive() {
        let mut t = Terminal::new(40, 5, TermThemeColors::default());
        t.feed(b"Hello World\r\nhello Rust\r\nWORLD\r\n");

        let matches_case_ins = t.search("world", false);
        assert_eq!(matches_case_ins.len(), 2);
        assert_eq!(matches_case_ins[0].line, 0);
        assert_eq!(matches_case_ins[0].col_start, 6);
        assert_eq!(matches_case_ins[0].col_end, 10);

        let matches_case_sens = t.search("WORLD", true);
        assert_eq!(matches_case_sens.len(), 1);
        assert_eq!(matches_case_sens[0].line, 2);
    }

    #[test]
    fn search_chinese_and_scrollback() {
        let mut t = Terminal::new(40, 3, TermThemeColors::default());
        t.feed("第一行 测试\r\n第二行 搜索\r\n第三行 测试\r\n第四行 终端\r\n".as_bytes());

        let matches = t.search("测试", false);
        assert_eq!(matches.len(), 2);
    }

    #[test]
    fn alternate_screen_and_mouse_modes() {
        use alacritty_terminal::term::TermMode;
        let mut t = Terminal::new(40, 10, TermThemeColors::default());
        assert!(!t.term.mode().contains(TermMode::ALT_SCREEN));

        // 进入 Alternate Screen 备用屏幕: \x1b[?1049h
        t.feed(b"\x1b[?1049h");
        assert!(t.term.mode().contains(TermMode::ALT_SCREEN));

        // 开启 SGR 鼠标模式: \x1b[?1000h\x1b[?1006h
        t.feed(b"\x1b[?1000h\x1b[?1006h");
        assert!(t.term.mode().intersects(TermMode::MOUSE_MODE | TermMode::MOUSE_REPORT_CLICK));
        assert!(t.term.mode().contains(TermMode::SGR_MOUSE));

        // 退出备用屏幕: \x1b[?1049l
        t.feed(b"\x1b[?1049l");
        assert!(!t.term.mode().contains(TermMode::ALT_SCREEN));
    }
}

