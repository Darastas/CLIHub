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

    let app_id_str = APP_USER_MODEL_ID;
    let app_id: Vec<u16> = OsStr::new(app_id_str)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    // 1) 注册当前进程的 AppUserModelID
    unsafe {
        unsafe extern "system" {
            fn SetCurrentProcessExplicitAppUserModelID(app_id: *const u16) -> i32;
        }
        let _ = SetCurrentProcessExplicitAppUserModelID(app_id.as_ptr());
    }

    // 2) 自动在 HKCU\Software\Classes\AppUserModelId\<AppID> 注册 DisplayName 与 IconUri
    // 使得 Windows 10/11 Toast 通知横幅显示 CLIHub
    unsafe {
        unsafe extern "system" {
            fn RegCreateKeyW(h_key: isize, sub_key: *const u16, result: *mut isize) -> i32;
            fn RegSetValueExW(
                h_key: isize,
                value_name: *const u16,
                reserved: u32,
                dw_type: u32,
                data: *const u8,
                cb_data: u32,
            ) -> i32;
            fn RegCloseKey(h_key: isize) -> i32;
        }

        const HKEY_CURRENT_USER: isize = 0x80000001u32 as isize;
        const REG_SZ: u32 = 1;

        let reg_sub_key: Vec<u16> = OsStr::new(&format!(r"Software\Classes\AppUserModelId\{app_id_str}"))
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        let mut hkey: isize = 0;
        if RegCreateKeyW(HKEY_CURRENT_USER, reg_sub_key.as_ptr(), &mut hkey) == 0 && hkey != 0 {
            let display_name_key: Vec<u16> = OsStr::new("DisplayName").encode_wide().chain(std::iter::once(0)).collect();
            let display_name_val: Vec<u16> = OsStr::new("CLIHub").encode_wide().chain(std::iter::once(0)).collect();
            let _ = RegSetValueExW(
                hkey,
                display_name_key.as_ptr(),
                0,
                REG_SZ,
                display_name_val.as_ptr() as *const u8,
                (display_name_val.len() * 2) as u32,
            );

            if let Ok(exe_path) = std::env::current_exe() {
                let icon_key: Vec<u16> = OsStr::new("IconUri").encode_wide().chain(std::iter::once(0)).collect();
                let icon_val: Vec<u16> = exe_path.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
                let _ = RegSetValueExW(
                    hkey,
                    icon_key.as_ptr(),
                    0,
                    REG_SZ,
                    icon_val.as_ptr() as *const u8,
                    (icon_val.len() * 2) as u32,
                );
            }

            let _ = RegCloseKey(hkey);
        }
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
