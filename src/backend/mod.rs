//! 核心业务层：PTY 进程管理、终端状态解析、后台 IO 循环。

pub mod io_loop;
pub mod pty;
pub mod terminal;
