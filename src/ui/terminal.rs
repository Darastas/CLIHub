//! 右侧主区域：终端输出区 + 底部输入栏 + 快捷操作提示。
//!
//! Round 1 以去 ANSI 后的纯文本展示 PTY 输出，行输入与回车发送命令；
//! Phase 3 将替换为字符网格渲染与原始按键转发。

use egui::{Color32, RichText, ScrollArea, TextEdit, TextStyle, Ui};

use crate::state::{Session, SessionStatus};

/// 主区域顶部按钮可能触发的动作，由 App 层执行。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalAction {
    Restart,
    Kill,
}

pub fn show(ui: &mut Ui, session: &mut Session, input: &mut String) -> Option<TerminalAction> {
    let mut action = None;
    let status = session.status();
    let interactive = session.is_interactive();

    // ---- 顶部信息栏 ----
    ui.horizontal(|ui| {
        ui.label(RichText::new(&session.name).strong().size(14.0));
        ui.label(RichText::new(super::status_dot(status)).color(super::status_color(status)));
        ui.label(
            RichText::new(session.cwd.display().to_string())
                .size(11.0)
                .color(Color32::from_gray(140)),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add_enabled(session.pty.is_some(), egui::Button::new("✕ Kill"))
                .clicked()
            {
                action = Some(TerminalAction::Kill);
            }
            if ui
                .add_enabled(!interactive, egui::Button::new("Restart"))
                .clicked()
            {
                action = Some(TerminalAction::Restart);
            }
        });
    });
    ui.separator();

    // ---- 错误提示 ----
    if let Some(err) = &session.error {
        ui.colored_label(Color32::from_rgb(190, 60, 50), err);
        ui.add_space(4.0);
    }

    // ---- 终端输出区 ----
    let text = { session.output.lock().map(|g| g.clone()).unwrap_or_default() };
    let display_color = if status == SessionStatus::Failed {
        Color32::from_gray(120)
    } else {
        ui.visuals().text_color()
    };
    ScrollArea::vertical()
        .auto_shrink([false, false])
        .stick_to_bottom(true)
        .show(ui, |ui| {
            ui.set_min_height(ui.available_height());
            ui.add(
                egui::Label::new(RichText::new(text).monospace().size(13.0).color(display_color))
                    .selectable(true),
            );
        });

    // ---- 底部输入栏 ----
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("❯")
                .monospace()
                .color(Color32::from_rgb(0, 120, 200)),
        );
        let resp = ui.add_enabled(
            interactive,
            TextEdit::singleline(input)
                .font(TextStyle::Monospace)
                .desired_width(f32::INFINITY)
                .hint_text("type a command, Enter to send"),
        );
        let submit = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if submit {
            let line = std::mem::take(input);
            let mut bytes = line.into_bytes();
            bytes.push(b'\r');
            if interactive {
                if let Some(pty) = &mut session.pty {
                    if let Err(e) = pty.write(&bytes) {
                        session.error = Some(format!("写入 PTY 失败: {e}"));
                    }
                }
            }
            resp.request_focus();
        }
        if !interactive {
            ui.label(
                RichText::new("(session not running)")
                    .size(11.0)
                    .color(Color32::from_gray(140)),
            );
        }
    });

    // ---- 快捷提示栏 ----
    ui.add_space(2.0);
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("? for shortcuts · ← for agents")
                .size(10.5)
                .color(Color32::from_gray(150)),
        );
    });

    action
}
