//! 串联 UI 渲染层与后台数据层：持有全部会话，驱动 PTY 读写。

use std::time::Duration;

use egui::Context;

use crate::backend::io_loop;
use crate::backend::pty::PtyHandle;
use crate::config::AppConfig;
use crate::state::{Session, SessionStatus};
use crate::ui::{sidebar, terminal};

pub struct HubApp {
    config: AppConfig,
    sessions: Vec<Session>,
    selected: usize,
    input: String,
}

fn home_dir() -> std::path::PathBuf {
    directories::BaseDirs::new()
        .map(|d| d.home_dir().to_path_buf())
        .unwrap_or_else(|| std::path::PathBuf::from("."))
}

impl HubApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // 目标视觉是白/浅灰极简风格
        cc.egui_ctx.set_visuals(egui::Visuals::light());

        let config = AppConfig::load();
        let sessions: Vec<Session> = config
            .clis
            .iter()
            .enumerate()
            .map(|(i, cli)| {
                let cwd = cli.cwd.clone().unwrap_or_else(home_dir);
                Session::new(i, &cli.name, &cli.command, cwd)
            })
            .collect();

        let mut app = Self {
            config,
            sessions,
            selected: 0,
            input: String::new(),
        };
        // 自动启动首个可用的终端会话，验证 PTY 链路
        if let Some(i) = app.find_terminal_index() {
            app.selected = i;
            app.spawn_session(i);
        }
        app
    }

    fn find_terminal_index(&self) -> Option<usize> {
        self.sessions.iter().position(|s| s.name == "Terminal")
    }

    fn session_mut(&mut self) -> Option<&mut Session> {
        self.sessions.get_mut(self.selected)
    }

    fn spawn_session(&mut self, idx: usize) {
        let Some(s) = self.sessions.get_mut(idx) else {
            return;
        };
        s.error = None;
        let command = s.command.clone();
        let cwd = s.cwd.clone();
        match PtyHandle::spawn(&command, &[], &cwd, 24, 80) {
            Ok((pty, rx)) => {
                s.rx = Some(rx);
                s.alive = pty.alive.clone();
                s.pty = Some(pty);
            }
            Err(e) => {
                s.error = Some(format!("无法启动 `{command}`: {e:#}"));
            }
        }
    }

    fn kill_session(&mut self, idx: usize) {
        let Some(s) = self.sessions.get_mut(idx) else {
            return;
        };
        s.pty.take(); // Drop 会 kill 子进程
        s.rx = None;
        s.alive.store(false, std::sync::atomic::Ordering::SeqCst);
        s.output.lock().unwrap().push_str("\n[session ended]\n");
    }

    fn restart_session(&mut self, idx: usize) {
        self.kill_session(idx);
        self.spawn_session(idx);
    }

    /// 后台数据更新：拉取 PTY 输出、检测进程退出。禁止在此绘制。
    fn update_backend(&mut self, ctx: &Context) {
        let mut dirty = false;
        for s in &mut self.sessions {
            // 拉取 PTY 输出
            if let Some(rx) = &s.rx {
                if io_loop::drain(rx, &s.output) > 0 {
                    dirty = true;
                }
            }
            // 检测进程退出
            if let Some(pty) = &mut s.pty {
                if let Ok(Some(_)) = pty.child.try_wait() {
                    s.alive.store(false, std::sync::atomic::Ordering::SeqCst);
                    s.output.lock().unwrap().push_str("\n[process exited]\n");
                }
            }
        }
        if dirty {
            ctx.request_repaint();
        }
        // 终端流式输出需要持续重绘
        ctx.request_repaint_after(Duration::from_millis(50));
    }

    fn update_ui(&mut self, ui: &mut egui::Ui) {
        let mut clicked = None;
        egui::Panel::left("sidebar")
            .resizable(false)
            .exact_size(232.0)
            .show(ui, |ui| {
                clicked = sidebar::show(ui, &self.sessions, self.selected);
            });
        if let Some(idx) = clicked {
            self.selected = idx;
            if self.sessions[idx].status() == SessionStatus::Idle {
                self.spawn_session(idx);
            }
        }

        let mut action = None;
        egui::CentralPanel::default_margins().show(ui, |ui| {
            // 字段级分离借用：sessions 与 input 互不重叠
            let session = self.sessions.get_mut(self.selected);
            match session {
                Some(session) => {
                    action = terminal::show(ui, session, &mut self.input);
                }
                None => {
                    ui.centered_and_justified(|ui| {
                        ui.label("No session selected");
                    });
                }
            }
        });

        match action {
            Some(terminal::TerminalAction::Kill) => self.kill_session(self.selected),
            Some(terminal::TerminalAction::Restart) => self.restart_session(self.selected),
            None => {}
        }
    }
}

impl eframe::App for HubApp {
    /// 每帧回调，先于 `ui`：更新后台数据，不绘制。
    fn logic(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.update_backend(ctx);
    }

    /// 绘制 UI。传入的 `Ui` 无外边距/背景，可再用 Panel 布局。
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.update_ui(ui);
    }
}
