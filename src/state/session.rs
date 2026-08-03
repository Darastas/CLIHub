//! 单个 CLI 会话：绑定一个 PTY 子进程、终端状态机（字符网格）和运行状态。

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crossbeam_channel::Receiver;

use crate::backend::pty::PtyHandle;
use crate::backend::terminal::Terminal;

/// 会话当前状态，用于侧边栏展示。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionStatus {
    /// 尚未启动
    Idle,
    /// 正在后台运行
    Running,
    /// 进程已退出
    Exited,
    /// 启动失败
    Failed,
}

pub struct Session {
    #[allow(dead_code)] // Phase 4 多实例索引
    pub id: usize,
    pub name: String,
    pub command: String,
    pub cwd: PathBuf,

    /// alacritty 终端状态机；None 表示未启动
    pub terminal: Option<Terminal>,
    /// 由 PTY reader 线程发来的原始字节块
    pub rx: Option<Receiver<Vec<u8>>>,
    /// PTY 句柄；None 表示未启动
    pub pty: Option<PtyHandle>,
    /// 进程是否存活
    pub alive: Arc<AtomicBool>,
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
            terminal: None,
            rx: None,
            pty: None,
            alive: Arc::new(AtomicBool::new(false)),
            error: None,
        }
    }

    pub fn status(&self) -> SessionStatus {
        if self.error.is_some() {
            return SessionStatus::Failed;
        }
        if self.pty.is_none() {
            return SessionStatus::Idle;
        }
        if self.alive.load(Ordering::SeqCst) {
            SessionStatus::Running
        } else {
            SessionStatus::Exited
        }
    }

    /// 该会话是否处于可交互状态。
    pub fn is_interactive(&self) -> bool {
        self.pty.is_some() && self.alive.load(Ordering::SeqCst)
    }
}
