//! Windows 系统通知与点击唤醒服务。
//!
//! 在任务完成、报错或 AI 等待确认时，通过 Windows Toast 发送原生系统通知。
//! 点击通知时通过 Channel 通知主应用置顶并切换至对应会话/标签页。

use crossbeam_channel::{unbounded, Receiver, Sender};
use notify_rust::Notification;

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

fn ensure_icon_file() -> Option<String> {
    let icon_bytes = include_bytes!("../../assets/icon.png");
    let temp_dir = std::env::temp_dir();
    let icon_path = temp_dir.join("clihub_logo.png");
    if !icon_path.exists() {
        let _ = std::fs::write(&icon_path, icon_bytes);
    }
    Some(icon_path.to_string_lossy().to_string())
}

#[cfg(windows)]
fn play_system_sound() {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    let sound_paths = [
        r"C:\Windows\Media\Windows Notify System Generic.wav",
        r"C:\Windows\Media\Windows Background.wav",
        r"C:\Windows\Media\Windows Ding.wav",
    ];

    for path in sound_paths {
        if std::path::Path::new(path).exists() {
            let wide: Vec<u16> = OsStr::new(path).encode_wide().chain(std::iter::once(0)).collect();
            unsafe {
                unsafe extern "system" {
                    fn PlaySoundW(psz_sound: *const u16, hmod: usize, fdw_sound: u32) -> i32;
                }
                const SND_ASYNC: u32 = 0x0001;
                const SND_FILENAME: u32 = 0x00020000;
                let _ = PlaySoundW(wide.as_ptr(), 0, SND_ASYNC | SND_FILENAME);
                return;
            }
        }
    }

    unsafe {
        unsafe extern "system" {
            fn MessageBeep(u_type: u32) -> i32;
        }
        let _ = MessageBeep(0x00000040);
    }
}

#[cfg(not(windows))]
fn play_system_sound() {}

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
        let icon_path = ensure_icon_file();

        // 异步派生线程发送通知，避免阻塞 UI 线程或等待用户点击
        std::thread::spawn(move || {
            // 播放 Windows 原生系统提示音
            if play_sound {
                play_system_sound();
            }

            let mut notification = Notification::new();
            notification
                .appname("CLIHub")
                .summary(&title)
                .body(&body)
                .timeout(notify_rust::Timeout::Milliseconds(6000));

            #[cfg(windows)]
            {
                notification.app_id("CLIHub");
            }

            if let Some(ref icon) = icon_path {
                notification.icon(icon);
            }

            let result = notification.show();
            let handle_opt = match result {
                Ok(h) => Some(h),
                Err(e) => {
                    eprintln!("[Notification] 显示通知失败: {e}");
                    None
                }
            };

            if let Some(handle) = handle_opt {
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
        });
    }
}

impl Default for NotificationService {
    fn default() -> Self {
        Self::new()
    }
}
