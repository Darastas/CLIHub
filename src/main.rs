#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod backend;
mod config;
mod state;
mod ui;

use app::HubApp;
use backend::notification::APP_USER_MODEL_ID;

#[cfg(windows)]
fn init_windows_app_id() {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    let app_id: Vec<u16> = OsStr::new(APP_USER_MODEL_ID)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        unsafe extern "system" {
            fn SetCurrentProcessExplicitAppUserModelID(app_id: *const u16) -> i32;
        }
        let _ = SetCurrentProcessExplicitAppUserModelID(app_id.as_ptr());
    }
}

fn main() -> eframe::Result {
    #[cfg(windows)]
    init_windows_app_id();

    // 崩溃日志落盘，便于诊断闪退
    std::panic::set_hook(Box::new(|info| {
        let msg = format!(
            "[clihub panic] {info}\n{}",
            std::backtrace::Backtrace::force_capture()
        );
        eprintln!("{msg}");
        let _ = std::fs::write("clihub-panic.log", msg);
    }));

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("CLIHub")
            .with_decorations(false)
            .with_inner_size([1120.0, 720.0])
            .with_min_inner_size([760.0, 480.0]),
        ..Default::default()
    };
    eframe::run_native(
        "AI CLI Hub",
        options,
        Box::new(|cc| Ok(Box::new(HubApp::new(cc)))),
    )
}
