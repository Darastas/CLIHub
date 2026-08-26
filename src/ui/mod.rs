//! 视图渲染层：仅使用 egui 绘制界面，不直接触碰后台进程。

pub mod image_preview;
pub mod overview;
pub mod sidebar;
pub mod terminal;
pub mod titlebar;

use egui::Color32;

use crate::state::SessionStatus;

/// 会话状态对应的指示圆点颜色。
pub fn status_color(s: SessionStatus) -> Color32 {
    match s {
        SessionStatus::Running => Color32::from_rgb(46, 160, 67),
        SessionStatus::Idle => Color32::from_gray(150),
        SessionStatus::Exited => Color32::from_gray(110),
        SessionStatus::Failed => Color32::from_rgb(200, 80, 60),
    }
}

/// 会话状态对应的指示圆点。
#[allow(dead_code)]
pub fn status_dot(s: SessionStatus) -> &'static str {
    match s {
        SessionStatus::Running => "●",
        SessionStatus::Idle => "○",
        SessionStatus::Exited => "◌",
        SessionStatus::Failed => "✕",
    }
}
