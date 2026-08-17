#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod backend;
mod config;
mod state;
mod ui;

use app::HubApp;

fn main() -> eframe::Result {
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
