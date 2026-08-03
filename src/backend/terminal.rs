//! 终端状态机：封装 `alacritty_terminal` 维护字符网格。
//!
//! PTY 输出的原始字节先经 VTE 处理器（`ansi::Processor`）解析，
//! 更新网格状态；UI 层每帧读取网格进行渲染，键盘输入由 UI 层写回 PTY。

use alacritty_terminal::event::VoidListener;
use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::term::{Config, Term};
use alacritty_terminal::vte::ansi::Processor;

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

/// alacritty 终端 + VTE 解析器。
pub struct Terminal {
    pub term: Term<VoidListener>,
    processor: Processor,
    cols: u16,
    rows: u16,
}

impl Terminal {
    pub fn new(cols: u16, rows: u16) -> Self {
        let dims = TermDims {
            cols: cols as usize,
            rows: rows as usize,
        };
        let term = Term::new(Config::default(), &dims, VoidListener);
        Self {
            term,
            processor: Processor::new(),
            cols,
            rows,
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

    #[allow(dead_code)] // Phase 5 字体适配使用
    pub fn dimensions(&self) -> (u16, u16) {
        (self.cols, self.rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alacritty_terminal::index::{Column, Line};

    /// 把一行单元格拼成字符串。
    fn row_text(grid: &alacritty_terminal::Grid<alacritty_terminal::term::cell::Cell>, line: usize) -> String {
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
}
