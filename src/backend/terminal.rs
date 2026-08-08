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
    pub line: usize,
    pub col: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectionRange {
    pub start: GridPoint,
    pub end: GridPoint,
}


/// 把 alacritty 需要写回 PTY 的事件转发给应用层。
pub struct HubListener {
    pty_tx: Sender<String>,
    /// 当前 (cols, rows)，供尺寸查询应答
    size: Arc<Mutex<(usize, usize)>>,
    /// 背景色，供 OSC 颜色查询应答
    bg_color: Arc<Mutex<Rgb>>,
}

impl EventListener for HubListener {
    fn send_event(&self, event: Event) {
        match event {
            // 终端应答（如 DSR `ESC[6n`）必须写回 PTY
            Event::PtyWrite(text) => {
                let _ = self.pty_tx.send(text);
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
            // OSC 颜色查询：用背景色应答
            Event::ColorRequest(_, formatter) => {
                let rgb = *self.bg_color.lock().unwrap();
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
    size: Arc<Mutex<(usize, usize)>>,
    pub bg_color: Arc<Mutex<Rgb>>,
    cols: u16,
    rows: u16,
    pub selection: Option<SelectionRange>,
}

impl Terminal {
    pub fn new(cols: u16, rows: u16) -> Self {
        let (pty_tx, pty_rx) = unbounded();
        let size = Arc::new(Mutex::new((cols as usize, rows as usize)));
        let bg_color = Arc::new(Mutex::new(Rgb {
            r: 255,
            g: 255,
            b: 255,
        }));
        let listener = HubListener {
            pty_tx,
            size: size.clone(),
            bg_color: bg_color.clone(),
        };
        let term = Term::new(Config::default(), &TermDims { cols: cols as usize, rows: rows as usize }, listener);
        
        Self {
            term,
            processor: Processor::new(),
            pty_rx,
            size,
            bg_color,
            cols,
            rows,
            selection: None,
        }
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
        let display_offset = grid.display_offset();
        let mut text = String::new();
        
        let mut line_texts = Vec::new();
        for l in start_line..=end_line {
            let actual_line = l as i32 + display_offset as i32;
            let row = &grid[Line(actual_line)];
            
            let sc = if l == start_line { start_col } else { 0 };
            let ec = if l == end_line { end_col } else { self.cols as usize - 1 };
            
            let mut line_str = String::new();
            let mut trailing_spaces = 0;
            for c in sc..=ec {
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
        let mut t = Terminal::new(40, 10);
        t.feed(b"hello\r\nworld");

        let grid = t.term.grid();
        assert_eq!(row_text(grid, 0).trim_end(), "hello");
        assert_eq!(row_text(grid, 1).trim_end(), "world");
    }

    /// ANSI 颜色序列应被解析，不影响字符本身。
    #[test]
    fn ansi_processed() {
        let mut t = Terminal::new(40, 10);
        t.feed(b"\x1b[31mRED\x1b[0m");
        let cell = &t.term.grid()[Line(0)][Column(0)];
        assert_eq!(cell.c, 'R');
    }

    /// resize 后网格行列随之变化。
    #[test]
    fn resize_changes_grid() {
        let mut t = Terminal::new(10, 5);
        assert_eq!(t.term.grid().screen_lines(), 5);
        assert_eq!(t.term.grid().columns(), 10);
        t.resize(20, 8);
        assert_eq!(t.term.grid().screen_lines(), 8);
        assert_eq!(t.term.grid().columns(), 20);
    }

    /// DSR 查询（`ESC[6n`）应产生写回 PTY 的应答，而不是被丢弃。
    #[test]
    fn dsr_produces_pty_write() {
        let mut t = Terminal::new(40, 10);
        // 模拟终端查询光标位置（claude 等 TUI 启动时会发这个）
        t.feed(b"\x1b[6n");
        let writes = t.drain_pty_writes();
        assert!(
            writes.iter().any(|w| w.starts_with("\x1b[") && w.ends_with('R')),
            "应产生 DSR 应答, 实际: {writes:?}"
        );
    }

}
