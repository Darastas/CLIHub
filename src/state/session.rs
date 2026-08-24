//! 会话模型：一个 CLI 配置（Session）可持有多个后台进程（Tab）。
//!
//! 侧边栏管理的是 `Session`（即 CLI 配置）；右侧终端区是一组 `TerminalInstance`
//! 标签页，每个 Tab 独立绑定一个 PTY 子进程、终端状态机（字符网格）和运行状态。

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crossbeam_channel::Receiver;

use crate::backend::pty::PtyHandle;
use crate::backend::terminal::{SearchMatch, Terminal};

/// 终端搜索栏状态。
#[derive(Debug, Clone, Default)]
pub struct SearchState {
    pub is_open: bool,
    pub query: String,
    pub case_sensitive: bool,
    pub matches: Vec<SearchMatch>,
    pub active_match: usize,
    /// 标记是否需要将焦点聚焦到搜索输入框
    pub request_focus: bool,
}

impl SearchState {
    pub fn next_match(&mut self) {
        if !self.matches.is_empty() {
            self.active_match = (self.active_match + 1) % self.matches.len();
        }
    }

    pub fn prev_match(&mut self) {
        if !self.matches.is_empty() {
            if self.active_match == 0 {
                self.active_match = self.matches.len() - 1;
            } else {
                self.active_match -= 1;
            }
        }
    }

    pub fn current_match(&self) -> Option<&SearchMatch> {
        if self.matches.is_empty() {
            None
        } else {
            self.matches.get(self.active_match)
        }
    }
}

/// 会话当前状态，用于侧边栏展示。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionStatus {
    /// 尚未启动任何实例
    Idle,
    /// 至少一个实例正在运行
    Running,
    /// 实例均已退出
    Exited,
    /// 启动失败
    Failed,
}

/// 单个终端实例（标签页）：一个 PTY 子进程 + 一个字符网格。
pub struct TerminalInstance {
    /// alacritty 终端状态机；None 表示该实例未创建终端
    pub terminal: Option<Terminal>,
    /// 由 PTY reader 线程发来的原始字节块
    pub rx: Option<Receiver<Vec<u8>>>,
    /// PTY 句柄；None 表示未启动
    pub pty: Option<PtyHandle>,
    /// 进程是否存活
    pub alive: Arc<AtomicBool>,
    /// IME 是否正处于预编辑（拼音输入中）状态
    pub ime_composing: bool,
    /// IME 预编辑文字（拼音字母），用于在光标位置内联渲染
    pub ime_preedit: String,
    /// 刚刚通过 IME Commit 提交的文字，用于吞掉随后由 OS 发送的重复 Text 事件
    pub ime_just_committed_text: Option<String>,
    /// 滚动累积量
    pub scroll_accum: f32,
    /// 终端内关键词搜索状态
    pub search_state: SearchState,
    /// 最近一次按下 Ctrl+C 的时间戳（用于短时间双击防误触退出）
    pub last_ctrl_c: Option<std::time::Instant>,
}

impl TerminalInstance {
    pub fn new() -> Self {
        Self {
            terminal: None,
            rx: None,
            pty: None,
            alive: Arc::new(AtomicBool::new(false)),
            ime_composing: false,
            ime_preedit: String::new(),
            ime_just_committed_text: None,
            scroll_accum: 0.0,
            search_state: SearchState::default(),
            last_ctrl_c: None,
        }
    }

    pub fn is_running(&self) -> bool {
        self.pty.is_some() && self.alive.load(Ordering::SeqCst)
    }
}

impl Default for TerminalInstance {
    fn default() -> Self {
        Self::new()
    }
}

/// 一个 CLI 配置及其全部实例（标签页）。
pub struct Session {
    #[allow(dead_code)]
    pub id: usize,
    pub name: String,
    pub command: String,
    pub cwd: PathBuf,

    /// 全部实例（标签页）
    pub tabs: Vec<TerminalInstance>,
    /// 当前激活的标签页
    pub active_tab: usize,
    /// 启动或运行期间的错误
    pub error: Option<String>,
}

impl Session {
    pub fn new(id: usize, name: &str, command: &str, cwd: PathBuf) -> Self {
        Self {
            id,
            name: name.to_string(),
            command: command.to_string(),
            cwd,
            tabs: Vec::new(),
            active_tab: 0,
            error: None,
        }
    }

    pub fn status(&self) -> SessionStatus {
        if self.error.is_some() {
            return SessionStatus::Failed;
        }
        if self.tabs.is_empty() {
            return SessionStatus::Idle;
        }
        if self.tabs.iter().any(|t| t.is_running()) {
            SessionStatus::Running
        } else {
            SessionStatus::Exited
        }
    }

}
