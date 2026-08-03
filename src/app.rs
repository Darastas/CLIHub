//! 串联 UI 渲染层与后台数据层：持有全部会话，驱动 PTY 读写与终端网格。

use std::path::PathBuf;
use std::time::Duration;

use egui::{Color32, Context};

use crate::backend::io_loop;
use crate::backend::pty::PtyHandle;
use crate::backend::terminal::Terminal;
use crate::config::{AppConfig, CliEntry};
use crate::state::{Session, SessionStatus};
use crate::ui::{sidebar, terminal, titlebar};

pub struct HubApp {
    config: AppConfig,
    sessions: Vec<Session>,
    selected: usize,
    // 新增会话对话框状态
    adding_cli: bool,
    new_name: String,
    new_command: String,
    new_cwd: String,
}

fn home_dir() -> PathBuf {
    directories::BaseDirs::new()
        .map(|d| d.home_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
}

/// 加载字体：内嵌 JetBrains Mono（终端等宽，含加粗族）+ 系统字体。
/// - Monospace：JetBrains Mono → Consolas → 微软雅黑(CJK)
/// - Proportional（UI）：Segoe UI → 微软雅黑(CJK)
fn setup_fonts(ctx: &egui::Context) {
    use egui::FontFamily;

    let mut fonts = egui::FontDefinitions::default();
    let arc = |data: &'static [u8]| std::sync::Arc::new(egui::FontData::from_static(data));

    // 内嵌 JetBrains Mono（Regular + Bold）
    fonts.font_data.insert(
        "jbmono".into(),
        arc(include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf")),
    );
    fonts.font_data.insert(
        "jbmono-bold".into(),
        arc(include_bytes!("../assets/fonts/JetBrainsMono-Bold.ttf")),
    );
    // 自定义加粗族（供终端粗体字形使用）
    fonts
        .families
        .insert(FontFamily::Name("jbmono-bold".into()), vec!["jbmono-bold".into()]);
    // 终端等宽族：JetBrains Mono 打头
    if let Some(mono) = fonts.families.get_mut(&FontFamily::Monospace) {
        mono.insert(0, "jbmono".into());
    }

    let load = |path: &str| -> Option<(String, Vec<u8>)> {
        std::fs::read(path).ok().map(|data| {
            let name = path
                .split(['\\', '/'])
                .last()
                .unwrap_or("font")
                .to_owned();
            (name, data)
        })
    };

    // UI 拉丁字体：Segoe UI 放最前
    if let Some((name, data)) = load(r"C:\Windows\Fonts\segoeui.ttf") {
        fonts
            .font_data
            .insert(name.clone(), std::sync::Arc::new(egui::FontData::from_owned(data)));
        if let Some(prop) = fonts.families.get_mut(&FontFamily::Proportional) {
            prop.insert(0, name);
        }
    }
    // 终端等宽兜底：Consolas
    if let Some((name, data)) = load(r"C:\Windows\Fonts\consola.ttf") {
        fonts
            .font_data
            .insert(name.clone(), std::sync::Arc::new(egui::FontData::from_owned(data)));
        if let Some(mono) = fonts.families.get_mut(&FontFamily::Monospace) {
            mono.push(name);
        }
    }
    // CJK 兜底链：微软雅黑 → DengXian → SimHei
    for path in [
        r"C:\Windows\Fonts\msyh.ttc",
        r"C:\Windows\Fonts\Deng.ttf",
        r"C:\Windows\Fonts\simhei.ttf",
    ] {
        if let Some((name, data)) = load(path) {
            fonts
                .font_data
                .insert(name.clone(), std::sync::Arc::new(egui::FontData::from_owned(data)));
            for family in [FontFamily::Monospace, FontFamily::Proportional] {
                if let Some(list) = fonts.families.get_mut(&family) {
                    list.push(name.clone());
                }
            }
        }
    }
    ctx.set_fonts(fonts);
}

impl HubApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // 目标视觉是白/浅灰极简风格：面板浅灰、选中中性灰、无描边
        let mut visuals = egui::Visuals::light();
        visuals.panel_fill = Color32::from_rgb(247, 248, 250);
        visuals.window_fill = Color32::from_rgb(250, 251, 252);
        visuals.selection.bg_fill = Color32::from_rgb(228, 230, 234);
        visuals.selection.stroke = egui::Stroke::NONE;
        visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(237, 239, 242);
        visuals.widgets.hovered.bg_fill = Color32::from_rgb(237, 239, 242);
        cc.egui_ctx.set_visuals(visuals);
        setup_fonts(&cc.egui_ctx);

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
            adding_cli: false,
            new_name: String::new(),
            new_command: String::new(),
            new_cwd: String::new(),
        };
        // 自动启动首个可用的终端会话，验证链路
        if let Some(i) = app.find_terminal_index() {
            app.selected = i;
            app.spawn_session(i);
        }
        app
    }

    fn find_terminal_index(&self) -> Option<usize> {
        self.sessions.iter().position(|s| s.name == "Terminal")
    }

    /// 把当前会话列表同步回配置并持久化。
    fn sync_config(&mut self) {
        self.config.clis = self
            .sessions
            .iter()
            .map(|s| CliEntry {
                name: s.name.clone(),
                command: s.command.clone(),
                args: Vec::new(),
                cwd: Some(s.cwd.clone()),
                env: Default::default(),
            })
            .collect();
        if let Err(e) = self.config.save() {
            eprintln!("[config] 保存失败: {e}");
        }
    }

    fn add_cli(&mut self) {
        let name = self.new_name.trim().to_string();
        let command = self.new_command.trim().to_string();
        if name.is_empty() || command.is_empty() {
            return;
        }
        let cwd = if self.new_cwd.trim().is_empty() {
            home_dir()
        } else {
            PathBuf::from(self.new_cwd.trim())
        };
        let idx = self.sessions.len();
        self.sessions
            .push(Session::new(idx, &name, &command, cwd));
        self.selected = idx;
        self.new_name.clear();
        self.new_command.clear();
        self.new_cwd.clear();
        self.sync_config();
    }

    fn remove_session(&mut self, idx: usize) {
        if idx >= self.sessions.len() {
            return;
        }
        self.kill_session(idx);
        self.sessions.remove(idx);
        if self.selected >= self.sessions.len() {
            self.selected = self.sessions.len().saturating_sub(1);
        }
        self.sync_config();
    }

    fn spawn_session(&mut self, idx: usize) {
        let Some(s) = self.sessions.get_mut(idx) else {
            return;
        };
        s.error = None;
        s.terminal = Some(Terminal::new(24, 80));
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
        if let Some(t) = &mut s.terminal {
            t.feed_text("\r\n[session ended]\r\n");
        }
        s.pty.take(); // Drop 会 kill 子进程
        s.rx = None;
        s.alive.store(false, std::sync::atomic::Ordering::SeqCst);
    }

    fn restart_session(&mut self, idx: usize) {
        self.kill_session(idx);
        if let Some(s) = self.sessions.get_mut(idx) {
            s.terminal = Some(Terminal::new(24, 80));
        }
        self.spawn_session(idx);
    }

    /// 后台数据更新：拉取 PTY 输出喂进终端、检测进程退出。禁止在此绘制。
    fn update_backend(&mut self, ctx: &Context) {
        let mut dirty = false;
        for s in &mut self.sessions {
            // 终端需要写回 PTY 的应答（DSR 光标位置等），立即转发
            if let Some(t) = &mut s.terminal {
                for text in t.drain_pty_writes() {
                    if let Some(pty) = &mut s.pty {
                        if let Err(e) = pty.write(text.as_bytes()) {
                            s.error = Some(format!("写回 PTY 失败: {e}"));
                        }
                    }
                }
            }
            // 拉取 PTY 输出 → 喂进 alacritty 网格
            if let Some(rx) = &s.rx {
                if io_loop::drain(rx, &mut s.terminal) > 0 {
                    dirty = true;
                }
            }
            // 检测进程退出
            if let Some(pty) = &mut s.pty {
                if let Ok(Some(_)) = pty.child.try_wait() {
                    s.alive.store(false, std::sync::atomic::Ordering::SeqCst);
                    if let Some(t) = &mut s.terminal {
                        t.feed_text("\r\n[process exited]\r\n");
                    }
                }
            }
        }
        if dirty {
            ctx.request_repaint();
        }
        // 终端流式输出需要持续重绘
        ctx.request_repaint_after(Duration::from_millis(33));
    }

    fn update_ui(&mut self, ui: &mut egui::Ui) {
        // 自定义无边框标题栏（占用顶部，面板自动下移）
        titlebar::show(ui);

        let mut side = sidebar::SidebarAction::default();
        egui::Panel::left("sidebar")
            .resizable(false)
            .exact_size(232.0)
            .show(ui, |ui| {
                side = sidebar::show(ui, &self.sessions, self.selected);
            });
        if let Some(idx) = side.select {
            self.selected = idx;
            // 非 Running 状态（Idle/Failed/Exited）点击即启动；
            // 之前启动失败过的会话（如旧的 os error 193）也能重新拉起
            if self.sessions[idx].status() != SessionStatus::Running {
                self.spawn_session(idx);
            }
        }
        if let Some(idx) = side.remove {
            self.remove_session(idx);
        }
        if side.add {
            self.adding_cli = true;
        }

        // 新增会话对话框
        if self.adding_cli {
            self.add_cli_dialog(ui);
        }

        let mut action = None;
        // 新增会话对话框打开时禁止键盘转发（避免输入串进终端）
        let input_enabled = !self.adding_cli;
        egui::CentralPanel::default_margins().show(ui, |ui| {
            let session = self.sessions.get_mut(self.selected);
            match session {
                Some(session) => {
                    action = terminal::show(ui, session, input_enabled);
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

    /// 新增会话的模态小窗。
    fn add_cli_dialog(&mut self, ui: &mut egui::Ui) {
        let mut confirm = false;
        let mut cancel = false;
        egui::Window::new("Add CLI Session")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ui.ctx(), |ui| {
                ui.label("Name");
                ui.text_edit_singleline(&mut self.new_name);
                ui.label("Command (e.g. codex, claude, powershell.exe)");
                ui.text_edit_singleline(&mut self.new_command);
                ui.label("Working directory (optional)");
                ui.text_edit_singleline(&mut self.new_cwd);
                ui.add_space(8.0);
                let name_ok = !self.new_name.trim().is_empty();
                let cmd_ok = !self.new_command.trim().is_empty();
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(name_ok && cmd_ok, egui::Button::new("Add"))
                        .clicked()
                    {
                        confirm = true;
                    }
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                });
            });
        if confirm {
            self.add_cli();
        }
        if confirm || cancel {
            self.adding_cli = false;
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
