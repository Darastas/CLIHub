#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod backend;
mod config;
mod state;
mod ui;

use app::HubApp;

#[cfg(windows)]
fn init_windows_toast_appid() {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

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

        let reg_sub_key: Vec<u16> = OsStr::new(r"Software\Classes\AppUserModelId\CLIHub")
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

            let temp_dir = std::env::temp_dir();
            let icon_path = temp_dir.join("clihub_logo.png");
            if !icon_path.exists() {
                let _ = std::fs::write(&icon_path, include_bytes!("../assets/icon.png"));
            }

            let icon_key: Vec<u16> = OsStr::new("IconUri").encode_wide().chain(std::iter::once(0)).collect();
            let icon_val: Vec<u16> = icon_path.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
            let _ = RegSetValueExW(
                hkey,
                icon_key.as_ptr(),
                0,
                REG_SZ,
                icon_val.as_ptr() as *const u8,
                (icon_val.len() * 2) as u32,
            );

            let color_key: Vec<u16> = OsStr::new("IconBackgroundColor").encode_wide().chain(std::iter::once(0)).collect();
            let color_val: Vec<u16> = OsStr::new("0").encode_wide().chain(std::iter::once(0)).collect();
            let _ = RegSetValueExW(
                hkey,
                color_key.as_ptr(),
                0,
                REG_SZ,
                color_val.as_ptr() as *const u8,
                (color_val.len() * 2) as u32,
            );

            let _ = RegCloseKey(hkey);
        }
    }
}

fn main() -> eframe::Result {
    #[cfg(windows)]
    init_windows_toast_appid();

    // 崩溃日志落盘，便于诊断闪退
    std::panic::set_hook(Box::new(|info| {
        let msg = format!(
            "[clihub panic] {info}\n{}",
            std::backtrace::Backtrace::force_capture()
        );
        eprintln!("{msg}");
        let _ = std::fs::write("clihub-panic.log", msg);
    }));

    let icon_data = eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon.png")).ok();

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("CLIHub")
        .with_decorations(false)
        .with_inner_size([1120.0, 720.0])
        .with_min_inner_size([760.0, 480.0]);

    if let Some(icon) = icon_data {
        viewport = viewport.with_icon(icon);
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        "AI CLI Hub",
        options,
        Box::new(|cc| Ok(Box::new(HubApp::new(cc)))),
    )
}
