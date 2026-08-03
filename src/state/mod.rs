//! 状态层：记录多个 CLI 的运行状态，供左侧边栏切换。

pub mod session;

pub use session::{Session, SessionStatus};
