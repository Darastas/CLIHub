//! Windows 系统通知与点击唤醒服务。
//!
//! 在任务完成、报错或 AI 等待确认时，通过 Windows Toast 发送原生系统通知。
//! 点击通知时通过 Channel 通知主应用置顶并切换至对应会话/标签页。

use crossbeam_channel::{unbounded, Receiver, Sender};
use notify_rust::Notification;

pub const APP_USER_MODEL_ID: &str = "CLIHub.Desktop.App";

/// 用户点击通知后触发的动作。
#[derive(Debug, Clone, Copy)]
pub enum NotificationAction {
    /// 唤醒并切换到指定的会话与标签页
    SwitchTo {
        session_idx: usize,
        tab_idx: usize,
    },
}

/// 系统通知管理器。
pub struct NotificationService {
    action_tx: Sender<NotificationAction>,
    action_rx: Receiver<NotificationAction>,
}

impl NotificationService {
    pub fn new() -> Self {
        let (action_tx, action_rx) = unbounded();
        Self {
            action_tx,
            action_rx,
        }
    }

    pub fn receiver(&self) -> Receiver<NotificationAction> {
        self.action_rx.clone()
    }

    /// 发送一条系统通知。
    pub fn send(
        &self,
        title: &str,
        body: &str,
        session_idx: usize,
        tab_idx: usize,
        play_sound: bool,
    ) {
        let action_tx = self.action_tx.clone();
        let title = title.to_string();
        let body = body.to_string();

        // 异步派生线程发送通知，避免阻塞 UI 线程或等待用户点击
        std::thread::spawn(move || {
            // 播放 Windows 原生系统提示音
            if play_sound {
                #[cfg(windows)]
                {
                    unsafe {
                        unsafe extern "system" {
                            fn MessageBeep(u_type: u32) -> i32;
                        }
                        // 0x00000040 = MB_ICONASTERISK (Windows 标准提示音)
                        let _ = MessageBeep(0x00000040);
                    }
                }
            }

            let mut notification = Notification::new();
            notification
                .appname("CLIHub")
                .summary(&title)
                .body(&body)
                .timeout(notify_rust::Timeout::Milliseconds(6000));

            #[cfg(windows)]
            {
                notification.app_id(APP_USER_MODEL_ID);
                if play_sound {
                    notification.sound_name("ms-winsoundevent:Notification.Default");
                } else {
                    notification.sound_name("Silent");
                }
            }

            // 在 Windows 上通过 show() 显示 Toast 通知
            match notification.show() {
                Ok(handle) => {
                    // 监听通知点击回调 (在后台线程等待点击)
                    handle.wait_for_action(move |action| match action {
                        "default" | "clicked" => {
                            let _ = action_tx.send(NotificationAction::SwitchTo {
                                session_idx,
                                tab_idx,
                            });
                        }
                        _ => {}
                    });
                }
                Err(e) => {
                    eprintln!("[Notification] 发送系统通知失败: {e}");
                }
            }
        });
    }
}

impl Default for NotificationService {
    fn default() -> Self {
        Self::new()
    }
}
