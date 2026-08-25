//! 串联 UI 渲染层与后台数据层：持有全部会话，驱动 PTY 读写与终端网格。

use std::path::PathBuf;
use std::time::Duration;

use egui::{Color32, Context, RichText};

use crate::backend::io_loop;
use crate::backend::notification::{NotificationAction, NotificationService};
use crate::backend::pty::PtyHandle;
use crate::backend::sleep_inhibitor::SleepInhibitor;
use crate::backend::terminal::{Terminal, TerminalEvent};
use crate::config::{AppConfig, CliEntry, NotificationSettings, ThemeSettings};
use crate::fonts::{app_visuals, setup_fonts};
use crate::state::{Session, SessionStatus, TerminalInstance};
use crate::ui::{sidebar, terminal, titlebar};

pub struct HubApp {
    config: AppConfig,
    sessions: Vec<Session>,
    next_id: usize,
    selected: usize,
    // 新增会话对话框状态
    adding_cli: bool,
    new_name: String,
    new_command: String,
    new_cwd: String,
    // 编辑会话对话框状态
    editing_cli: Option<usize>,
    edit_name: String,
    edit_command: String,
    edit_cwd: String,
    // 设置窗口状态
    show_settings: bool,
    settings_draft: ThemeSettings,
    notification_draft: NotificationSettings,
    notification_service: NotificationService,
    in_overview: bool,
    overview_session: Option<usize>,
    sleep_inhibitor: SleepInhibitor,
    egui_ctx: Option<egui::Context>,
    last_grid_cols: u16,
    last_grid_rows: u16,
}

fn home_dir() -> PathBuf {
    directories::BaseDirs::new()
        .map(|d| d.home_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
}

impl HubApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // 整体主题（侧边栏/标题栏/面板/对话框一并切换）
        let config = AppConfig::load();
        cc.egui_ctx.set_visuals(app_visuals(config.theme.dark));
        setup_fonts(&cc.egui_ctx);

        let initial_theme = config.theme.clone();
        let initial_notification = config.notification.clone();
        let sessions: Vec<Session> = config
            .clis
            .iter()
            .enumerate()
            .map(|(i, cli)| {
                let cwd = cli.cwd.clone().unwrap_or_else(home_dir);
                Session::new(i, &cli.name, &cli.command, cwd)
            })
            .collect();

        let next_id = sessions.iter().map(|s| s.id).max().unwrap_or(0) + 1;

        let mut app = Self {
            config,
            sessions,
            next_id,
            selected: 0,
            adding_cli: false,
            new_name: String::new(),
            new_command: String::new(),
            new_cwd: String::new(),
            editing_cli: None,
            edit_name: String::new(),
            edit_command: String::new(),
            edit_cwd: String::new(),
            show_settings: false,
            settings_draft: initial_theme,
            notification_draft: initial_notification,
            notification_service: NotificationService::new(),
            in_overview: false,
            overview_session: None,
            sleep_inhibitor: SleepInhibitor::new(),
            egui_ctx: Some(cc.egui_ctx.clone()),
            last_grid_cols: 120,
            last_grid_rows: 35,
        };
        // 自动为默认终端会话开第一个标签页，验证链路
        if let Some(i) = app.find_terminal_index() {
            app.selected = i;
            app.spawn_tab(i);
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
        let id = self.next_id;
        self.next_id += 1;
        let new_idx = self.sessions.len();
        self.sessions
            .push(Session::new(id, &name, &command, cwd));
        self.selected = new_idx;
        self.new_name.clear();
        self.new_command.clear();
        self.new_cwd.clear();
        self.sync_config();
    }

    fn edit_cli(&mut self) {
        if let Some(idx) = self.editing_cli {
            let name = self.edit_name.trim().to_string();
            let command = self.edit_command.trim().to_string();
            if name.is_empty() || command.is_empty() {
                return;
            }
            let cwd = if self.edit_cwd.trim().is_empty() {
                home_dir()
            } else {
                PathBuf::from(self.edit_cwd.trim())
            };
            if let Some(s) = self.sessions.get_mut(idx) {
                s.name = name;
                s.command = command;
                s.cwd = cwd;
            }
            self.editing_cli = None;
            self.sync_config();
        }
    }

    fn remove_session(&mut self, idx: usize) {
        if idx >= self.sessions.len() {
            return;
        }
        // 杀掉该会话的全部标签页
        for tab in self.sessions[idx].tabs.drain(..) {
            drop(tab); // PtyHandle::drop 会 kill 子进程
        }
        self.sessions.remove(idx);
        if self.selected >= self.sessions.len() {
            self.selected = self.sessions.len().saturating_sub(1);
        }
        self.sync_config();
    }

    /// 为某个会话新建一个标签页并激活（后台异步非阻塞派生）。
    fn spawn_tab(&mut self, session_idx: usize) {
        let term_theme = self.build_theme();
        let dark_mode = self.config.theme.dark;
        let Some(s) = self.sessions.get_mut(session_idx) else {
            return;
        };
        s.error = None;

        let command = s.command.clone();
        let cwd = s.cwd.clone();
        let cols = self.last_grid_cols;
        let rows = self.last_grid_rows;
        let mut inst = TerminalInstance::new();
        // 立即初始化终端字符网格（使用当前精准网格尺寸，防止 ConPTY 启动瞬间再次触发 resize 导致 OMP 重绘两次）
        inst.terminal = Some(Terminal::new(rows, cols, term_theme.to_theme_colors()));
        inst.alive.store(true, std::sync::atomic::Ordering::SeqCst);

        // 异步派生 PTY 进程，彻底杜绝主 UI 线程卡顿
        let (spawn_tx, spawn_rx) = crossbeam_channel::bounded(1);
        let cmd_clone = command.clone();
        let cwd_clone = cwd.clone();
        let ctx_clone = self.egui_ctx.clone();
        std::thread::Builder::new()
            .name(format!("spawn-{}", command))
            .spawn(move || {
                let res = PtyHandle::spawn(&cmd_clone, &[], &cwd_clone, rows, cols, dark_mode, ctx_clone);
                let _ = spawn_tx.send(res);
            })
            .ok();

        inst.pending_pty = Some(spawn_rx);
        s.tabs.push(inst);
        s.active_tab = s.tabs.len() - 1;
    }

    /// 关闭并移除某个标签页（杀进程）。
    fn kill_tab(&mut self, session_idx: usize, tab_idx: usize) {
        let Some(s) = self.sessions.get_mut(session_idx) else {
            return;
        };
        if tab_idx >= s.tabs.len() {
            return;
        }
        s.tabs[tab_idx].pty.take(); // Drop 会 kill 子进程
        s.tabs[tab_idx].rx = None;
        s.tabs[tab_idx].pending_pty = None;
        s.tabs[tab_idx].alive.store(false, std::sync::atomic::Ordering::SeqCst);
        s.tabs.remove(tab_idx);
        if s.tabs.is_empty() {
            s.active_tab = 0;
            s.error = None;
        } else if s.active_tab >= s.tabs.len() {
            s.active_tab = s.tabs.len() - 1;
        }
    }

    /// 后台数据更新：遍历 会话 → 标签页，拉取 PTY 输出喂进终端、检测退出与通知。禁止绘制。
    fn update_backend(&mut self, ctx: &Context) {
        // 1) 响应用户点击系统 Toast 后的激活唤醒请求
        while let Ok(action) = self.notification_service.receiver().try_recv() {
            match action {
                NotificationAction::SwitchTo { session_idx, tab_idx } => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                    if session_idx < self.sessions.len() {
                        self.selected = session_idx;
                        if let Some(s) = self.sessions.get_mut(session_idx) {
                            if tab_idx < s.tabs.len() {
                                s.active_tab = tab_idx;
                            }
                        }
                    }
                    ctx.request_repaint();
                }
            }
        }

        let window_focused = ctx.input(|i| i.focused);
        let mut active_dirty = false;
        let mut background_dirty = false;

        for (si, s) in self.sessions.iter_mut().enumerate() {
            for (ti, tab) in s.tabs.iter_mut().enumerate() {
                let is_active_tab = self.selected == si && s.active_tab == ti;

                // 异步 PTY 轮询接入
                if let Some(pending) = &tab.pending_pty {
                    match pending.try_recv() {
                        Ok(Ok((pty, rx))) => {
                            tab.alive = pty.alive.clone();
                            tab.pty = Some(pty);
                            tab.rx = Some(rx);
                            tab.pending_pty = None;
                            if is_active_tab || self.in_overview {
                                active_dirty = true;
                            }
                        }
                        Ok(Err(e)) => {
                            s.error = Some(format!("无法启动 `{}`: {e:#}", s.command));
                            tab.alive.store(false, std::sync::atomic::Ordering::SeqCst);
                            tab.pending_pty = None;
                            if is_active_tab || self.in_overview {
                                active_dirty = true;
                            }
                        }
                        Err(crossbeam_channel::TryRecvError::Empty) => {
                            // 正在后台异步创建中
                        }
                        Err(crossbeam_channel::TryRecvError::Disconnected) => {
                            tab.pending_pty = None;
                        }
                    }
                }

                // 终端需要写回 PTY 的应答（DSR 光标位置等），立即转发
                if let Some(t) = &mut tab.terminal {
                    for text in t.drain_pty_writes() {
                        if let Some(pty) = &mut tab.pty {
                            if let Err(e) = pty.write(text.as_bytes()) {
                                s.error = Some(format!("写回 PTY 失败: {e}"));
                            }
                        }
                    }

                    // 终端内部事件（Bell 响铃 / AI 任务等待确认）
                    for evt in t.drain_events() {
                        match evt {
                            TerminalEvent::Bell => {
                                if self.config.notification.enabled && self.config.notification.on_attention_needed {
                                    let is_focused_tab = window_focused && is_active_tab;
                                    if !self.config.notification.only_when_unfocused || !is_focused_tab {
                                        self.notification_service.send(
                                            &format!("🔔 {}", s.name),
                                            "AI 任务需要确认或已就绪",
                                            si,
                                            ti,
                                            self.config.notification.play_sound,
                                        );
                                    }
                                }
                            }
                            TerminalEvent::Title(_) => {}
                        }
                    }
                }

                // 拉取 PTY 输出 → 喂进 alacritty 网格
                if let Some(rx) = &tab.rx {
                    if io_loop::drain(rx, &mut tab.terminal) > 0 {
                        if is_active_tab || self.in_overview {
                            active_dirty = true;
                        } else {
                            background_dirty = true;
                        }
                    }
                }

                // 终端刚刚收到输入后产生的新应答（DSR 光标位置/XTWINOPS 等），在同帧立刻写回 PTY（0ms 往返延迟）
                if let Some(t) = &mut tab.terminal {
                    for text in t.drain_pty_writes() {
                        if let Some(pty) = &mut tab.pty {
                            if let Err(e) = pty.write(text.as_bytes()) {
                                s.error = Some(format!("写回 PTY 失败: {e}"));
                            }
                        }
                    }
                }

                // 检测进程退出
                if let Some(pty) = &mut tab.pty {
                    if let Ok(Some(status)) = pty.child.try_wait() {
                        tab.alive.store(false, std::sync::atomic::Ordering::SeqCst);
                        if let Some(t) = &mut tab.terminal {
                            t.feed_text("\r\n[process exited]\r\n");
                        }
                        tab.pty = None; // 释放 PTY 句柄，避免每帧重复打印 [process exited]

                        // 触发任务完成 / 退出通知
                        if self.config.notification.enabled && self.config.notification.on_process_exit {
                            let is_focused_tab = window_focused && is_active_tab;
                            if !self.config.notification.only_when_unfocused || !is_focused_tab {
                                let (title, body) = if status.success() {
                                    (format!("🎉 {}", s.name), "任务已完成".to_string())
                                } else {
                                    (
                                        format!("⚠️ {}", s.name),
                                        format!("任务已结束 ({status:?})"),
                                    )
                                };
                                self.notification_service.send(
                                    &title,
                                    &body,
                                    si,
                                    ti,
                                    self.config.notification.play_sound,
                                );
                            }
                        }
                    }
                }
            }
        }

        // 当有任何正在运行的 AI 任务时阻止系统自动休眠，全部空闲后恢复省电策略
        let any_running = self.sessions.iter().any(|s| s.status() == SessionStatus::Running);
        if any_running {
            self.sleep_inhibitor.prevent_sleep();
        } else {
            self.sleep_inhibitor.allow_sleep();
        }

        if active_dirty || (background_dirty && self.in_overview) {
            ctx.request_repaint(); // 仅在前台活跃或全景看板时触发瞬时重绘
        } else if background_dirty {
            // 后台会话有数据时，限频 10Hz 轻量通知，避免 GPU 100% 空转
            ctx.request_repaint_after(Duration::from_millis(100));
        } else {
            // 空闲时用低频重绘（仅维持光标闪烁 ~2Hz），避免 30fps 空转烧 CPU
            ctx.request_repaint_after(Duration::from_millis(500));
        }
    }

    fn update_ui(&mut self, ui: &mut egui::Ui) {
        // 全局快捷键：Esc 退出看板（单会话全景返回全局全景，全局全景退出看板）
        if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            if self.in_overview {
                if self.overview_session.is_some() {
                    self.overview_session = None;
                } else {
                    self.in_overview = false;
                }
            }
        }

        // 全局快捷键：Ctrl+Shift+O 切换全景多会话看板
        if ui.input_mut(|i| i.consume_key(egui::Modifiers::CTRL | egui::Modifiers::SHIFT, egui::Key::O)) {
            self.in_overview = !self.in_overview;
            if !self.in_overview {
                self.overview_session = None;
            }
        }

        // 自定义无边框标题栏（占用顶部，面板自动下移）
        if titlebar::show(ui) {
            self.settings_draft = self.config.theme.clone();
            self.show_settings = true;
        }

        let mut side = sidebar::SidebarAction::default();
        egui::Panel::left("sidebar")
            .resizable(false)
            .exact_size(232.0)
            .show(ui, |ui| {
                side = sidebar::show(ui, &self.sessions, self.selected, self.in_overview, &self.config.theme);
            });
        if side.toggle_overview {
            self.in_overview = !self.in_overview;
            if !self.in_overview {
                self.overview_session = None;
            }
        }
        if let Some(idx) = side.select {
            self.selected = idx;
            self.in_overview = false;
            self.overview_session = None;
            // 首次选中且无标签页时，自动开第一个标签；后续用右侧 + 开新标签
            let should_start = {
                let s = &self.sessions[idx];
                s.tabs.is_empty() && s.status() != SessionStatus::Failed
            };
            if should_start {
                self.spawn_tab(idx);
            }
        }
        if let Some(idx) = side.remove {
            self.remove_session(idx);
        }
        if side.add {
            self.adding_cli = true;
        }
        if let Some((from, to)) = side.move_to {
            self.reorder_session(from, to);
        }
        if side.settings {
            self.settings_draft = self.config.theme.clone();
            self.show_settings = true;
        }
        if let Some(idx) = side.edit {
            if let Some(s) = self.sessions.get(idx) {
                self.editing_cli = Some(idx);
                self.edit_name = s.name.clone();
                self.edit_command = s.command.clone();
                self.edit_cwd = s.cwd.display().to_string();
            }
        }

        let mut action = None;
        // 弹窗打开时禁止键盘转发（避免输入串进终端）
        let input_enabled = !self.adding_cli && self.editing_cli.is_none() && !self.show_settings;
        // 终端主题（按配置构建一次）
        let theme = self.build_theme();
        egui::CentralPanel::default_margins().show(ui, |ui| {
            if self.in_overview {
                if let Some(ov_act) = crate::ui::overview::show(ui, &self.sessions, self.overview_session, &self.config.theme, &theme) {
                    match ov_act {
                        crate::ui::overview::OverviewAction::SelectSessionTab { session_idx, tab_idx } => {
                            self.selected = session_idx;
                            self.in_overview = false;
                            self.overview_session = None;
                            let should_start = {
                                let s = &self.sessions[session_idx];
                                s.tabs.is_empty() && s.status() != SessionStatus::Failed
                            };
                            if should_start {
                                self.spawn_tab(session_idx);
                            } else if let Some(s) = self.sessions.get_mut(session_idx) {
                                if tab_idx < s.tabs.len() {
                                    s.active_tab = tab_idx;
                                }
                            }
                        }
                        crate::ui::overview::OverviewAction::NewTab(session_idx) => {
                            self.selected = session_idx;
                            self.in_overview = false;
                            self.overview_session = None;
                            self.spawn_tab(session_idx);
                        }
                        crate::ui::overview::OverviewAction::BrowseSessionTabs(session_idx) => {
                            self.overview_session = Some(session_idx);
                        }
                        crate::ui::overview::OverviewAction::BackToGlobalOverview => {
                            self.overview_session = None;
                        }
                        crate::ui::overview::OverviewAction::CloseTab { session_idx, tab_idx } => {
                            self.kill_tab(session_idx, tab_idx);
                        }
                    }
                }
            } else {
                let session = self.sessions.get_mut(self.selected);
                match session {
                    Some(session) => {
                        action = terminal::show(ui, session, input_enabled, &theme);
                        if let Some(tab) = session.tabs.get(session.active_tab) {
                            if let Some(t) = &tab.terminal {
                                if t.cols > 0 && t.rows > 0 {
                                    self.last_grid_cols = t.cols;
                                    self.last_grid_rows = t.rows;
                                }
                            }
                        }
                    }
                    None => {
                        ui.centered_and_justified(|ui| {
                            ui.label("No session selected");
                        });
                    }
                }
            }
        });

        // 对话框必须在所有 Panel 渲染完毕后绘制，这样背景遮罩才能覆盖全屏（包括终端区域）
        // 新增会话对话框
        if self.adding_cli {
            self.add_cli_dialog(ui);
        }
        // 编辑会话对话框
        if self.editing_cli.is_some() {
            self.edit_cli_dialog(ui);
        }
        // 设置窗口
        if self.show_settings {
            self.settings_window(ui);
        }

        match action {
            Some(terminal::TerminalAction::NewTab) => self.spawn_tab(self.selected),
            Some(terminal::TerminalAction::SwitchTab(t)) => {
                if let Some(s) = self.sessions.get_mut(self.selected) {
                    if t < s.tabs.len() {
                        s.active_tab = t;
                    }
                }
            }
            Some(terminal::TerminalAction::KillTab(t)) => self.kill_tab(self.selected, t),
            None => {}
        }
    }

    /// 拖拽排序会话：把 from 移到 to。
    fn reorder_session(&mut self, from: usize, to: usize) {
        if from == to || from >= self.sessions.len() || to >= self.sessions.len() {
            return;
        }
        let s = self.sessions.remove(from);
        self.sessions.insert(to, s);
        // 修正选中索引
        if self.selected == from {
            self.selected = to;
        } else if from < to {
            if self.selected > from && self.selected <= to {
                self.selected -= 1;
            }
        } else {
            if self.selected >= to && self.selected < from {
                self.selected += 1;
            }
        }
        self.sync_config();
    }

    /// 依据配置构建终端主题。
    fn build_theme(&self) -> terminal::TermTheme {
        let settings = &self.config.theme;
        let mut theme = terminal::TermTheme::from_scheme(&settings.color_scheme);
        theme.apply(settings);
        theme
    }

    /// 设置窗口：主题预设 + 自定义背景/前景色。
    fn settings_window(&mut self, ui: &mut egui::Ui) {
        let mut changed = false;
        let mut close = false;

        let screen_rect = ui.ctx().input(|i| i.raw.screen_rect).unwrap_or_else(|| ui.max_rect());
        ui.painter().rect_filled(screen_rect, 0.0, Color32::from_black_alpha(150));

        egui::Area::new(egui::Id::new("settings_modal"))
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ui.ctx(), |ui: &mut egui::Ui| {
                let dark = ui.visuals().dark_mode;
                let modal_bg = if dark { Color32::from_rgb(35, 35, 35) } else { Color32::from_rgb(250, 250, 250) };
                let stroke = if dark { egui::Stroke::new(1.0, Color32::from_rgb(60, 60, 60)) } else { egui::Stroke::new(1.0, Color32::from_rgb(220, 220, 220)) };

                let frame = egui::Frame::NONE
                    .fill(modal_bg)
                    .stroke(stroke)
                    .corner_radius(16)
                    .inner_margin(32.0)
                    .shadow(egui::epaint::Shadow { offset: [0, 16], blur: 32, spread: 0, color: Color32::from_black_alpha(if dark { 120 } else { 30 }) });
                
                frame.show(ui, |ui: &mut egui::Ui| {
                    let visuals = &mut ui.style_mut().visuals;
                    visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(6);
                    visuals.widgets.hovered.corner_radius = egui::CornerRadius::same(6);
                    visuals.widgets.active.corner_radius = egui::CornerRadius::same(6);
                    visuals.widgets.noninteractive.corner_radius = egui::CornerRadius::same(6);
                    visuals.widgets.inactive.bg_fill = if dark { Color32::from_rgb(24, 24, 27) } else { Color32::WHITE };
                    visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, if dark { Color32::from_gray(60) } else { Color32::from_gray(210) });
                    visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, if dark { Color32::from_gray(100) } else { Color32::from_gray(170) });
                    visuals.selection.stroke = egui::Stroke::new(1.5, Color32::from_rgb(0, 111, 238));

                    ui.set_width(320.0);
                        
                    // Header with close button
                    ui.horizontal(|ui: &mut egui::Ui| {
                        ui.label(RichText::new("Settings").size(18.0).strong());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui: &mut egui::Ui| {
                            let (rect, resp) = ui.allocate_exact_size(egui::vec2(20.0, 20.0), egui::Sense::click());
                            let color = if resp.hovered() { Color32::from_rgb(220, 60, 50) } else { if dark { Color32::from_gray(140) } else { Color32::from_gray(120) } };
                            
                            let c = rect.center();
                            let s = 4.5;
                            ui.painter().line_segment([c - egui::vec2(s, s), c + egui::vec2(s, s)], egui::Stroke::new(1.5, color));
                            ui.painter().line_segment([c - egui::vec2(s, -s), c + egui::vec2(s, -s)], egui::Stroke::new(1.5, color));
                            
                            if resp.clicked() {
                                close = true;
                            }
                        });
                    });
                    ui.add_space(16.0);

                    ui.label(RichText::new("App Theme").size(12.0));
                    ui.horizontal(|ui: &mut egui::Ui| {
                        // Restore bg_stroke for radio buttons
                        let prev_stroke = ui.visuals().widgets.inactive.bg_stroke;
                        ui.visuals_mut().widgets.inactive.bg_stroke = egui::Stroke::new(1.5, if dark { Color32::from_gray(100) } else { Color32::from_gray(160) });
                        
                        if ui.radio(!self.settings_draft.dark, "Light").clicked() {
                            self.settings_draft.dark = false;
                            self.settings_draft.color_scheme = "One Half Light".to_string();
                            self.settings_draft.background = None;
                            self.settings_draft.foreground = None;
                            changed = true;
                        }
                        if ui.radio(self.settings_draft.dark, "Dark").clicked() {
                            self.settings_draft.dark = true;
                            self.settings_draft.color_scheme = "Campbell".to_string();
                            self.settings_draft.background = None;
                            self.settings_draft.foreground = None;
                            changed = true;
                        }
                        
                        ui.visuals_mut().widgets.inactive.bg_stroke = prev_stroke;
                    });
                    ui.add_space(8.0);
                    
                    ui.label(RichText::new("Terminal Color Scheme").size(12.0));
                    egui::ComboBox::from_id_salt("color_scheme_combo")
                        .selected_text(&self.settings_draft.color_scheme)
                        .show_ui(ui, |ui: &mut egui::Ui| {
                            ui.visuals_mut().selection.bg_fill = if dark {
                                Color32::from_white_alpha(30)
                            } else {
                                Color32::from_black_alpha(20)
                            };
                            for scheme in &["Campbell", "One Half Light", "One Half Dark", "Solarized Dark", "Tango Dark", "Readable Solar Light"] {
                                if ui.selectable_value(&mut self.settings_draft.color_scheme, scheme.to_string(), *scheme).changed() {
                                    self.settings_draft.background = None;
                                    self.settings_draft.foreground = None;
                                    changed = true;
                                }
                            }
                        });
                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(8.0);

                    let default_theme = terminal::TermTheme::from_scheme(&self.settings_draft.color_scheme);
                    let preset_bg = [default_theme.background.r(), default_theme.background.g(), default_theme.background.b()];
                    let preset_fg = [default_theme.foreground.r(), default_theme.foreground.g(), default_theme.foreground.b()];
                    let mut bg = self.settings_draft.background.unwrap_or(preset_bg);
                    let mut fg = self.settings_draft.foreground.unwrap_or(preset_fg);

                    let border_color = if dark { Color32::from_gray(80) } else { Color32::from_gray(200) };
                    
                    let draw_color_picker = |ui: &mut egui::Ui, label: &str, color: &mut [u8; 3]| -> bool {
                        ui.label(label);
                        let mut res = false;
                        egui::Frame::NONE
                            .stroke(egui::Stroke::new(1.0, border_color))
                            .inner_margin(2.0)
                            .corner_radius(4)
                            .show(ui, |ui: &mut egui::Ui| {
                                // Temporarily add a very subtle inner stroke to the color edit button 
                                // so it doesn't blend into the gap if the color matches the background
                                let prev_stroke = ui.visuals().widgets.noninteractive.bg_stroke;
                                ui.visuals_mut().widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, if dark { Color32::from_gray(60) } else { Color32::from_gray(220) });
                                res = ui.color_edit_button_srgb(color).changed();
                                ui.visuals_mut().widgets.noninteractive.bg_stroke = prev_stroke;
                            });
                        res
                    };
                    
                    if draw_color_picker(ui, "Background", &mut bg) {
                        self.settings_draft.background = Some(bg);
                        changed = true;
                    }
                        
                    ui.add_space(4.0);
                    
                    if draw_color_picker(ui, "Foreground", &mut fg) {
                        self.settings_draft.foreground = Some(fg);
                        changed = true;
                    }
                    ui.add_space(4.0);
                    
                    let mut sidebar_bg = self.settings_draft.sidebar_card_color.unwrap_or([0, 111, 238]);
                    if draw_color_picker(ui, "Sidebar Card Color", &mut sidebar_bg) {
                        self.settings_draft.sidebar_card_color = Some(sidebar_bg);
                        changed = true;
                    }

                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(8.0);

                    ui.label(RichText::new("Notifications").size(14.0).strong());
                    ui.add_space(4.0);

                    if ui.checkbox(&mut self.notification_draft.enabled, "启用 Windows 系统通知").changed() {
                        changed = true;
                    }

                    if self.notification_draft.enabled {
                        ui.indent("notification_options", |ui| {
                            if ui.checkbox(&mut self.notification_draft.on_attention_needed, "AI 任务等待确认 / Bell 响铃时提醒").changed() {
                                changed = true;
                            }
                            if ui.checkbox(&mut self.notification_draft.on_process_exit, "任务完成 / 进程退出时提醒").changed() {
                                changed = true;
                            }
                            if ui.checkbox(&mut self.notification_draft.only_when_unfocused, "仅在后台或非活动会话时提醒（智能免打扰）").changed() {
                                changed = true;
                            }
                            if ui.checkbox(&mut self.notification_draft.play_sound, "播放提示音").changed() {
                                changed = true;
                            }
                        });
                    }

                    ui.add_space(16.0);
                    
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui: &mut egui::Ui| {
                        if ui.button("Reset defaults").clicked() {
                            self.settings_draft = ThemeSettings::default();
                            self.notification_draft = NotificationSettings::default();
                            changed = true;
                        }
                    });
                });
            });

        if changed {
            self.config.theme = self.settings_draft.clone();
            self.config.notification = self.notification_draft.clone();
            ui.ctx().set_visuals(app_visuals(self.settings_draft.dark));
            let _ = self.config.save();
        }
        if close {
            self.show_settings = false;
        }
    }

    /// 新增会话的模态小窗。
    fn add_cli_dialog(&mut self, ui: &mut egui::Ui) {
        let mut confirm = false;
        let mut cancel = false;

        let screen_rect = ui.ctx().input(|i| i.raw.screen_rect).unwrap_or_else(|| ui.max_rect());
        ui.painter().rect_filled(screen_rect, 0.0, Color32::from_black_alpha(150));

        egui::Area::new(egui::Id::new("add_session_modal"))
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ui.ctx(), |ui: &mut egui::Ui| {
                let dark = ui.visuals().dark_mode;
                let modal_bg = if dark { Color32::from_rgb(35, 35, 35) } else { Color32::from_rgb(250, 250, 250) };
                let stroke = if dark { egui::Stroke::new(1.0, Color32::from_rgb(60, 60, 60)) } else { egui::Stroke::new(1.0, Color32::from_rgb(220, 220, 220)) };

                let frame = egui::Frame::NONE
                    .fill(modal_bg)
                    .stroke(stroke)
                    .corner_radius(16)
                    .inner_margin(32.0)
                    .shadow(egui::epaint::Shadow { offset: [0, 16], blur: 32, spread: 0, color: Color32::from_black_alpha(if dark { 120 } else { 30 }) });
                
                frame.show(ui, |ui: &mut egui::Ui| {
                    let visuals = &mut ui.style_mut().visuals;
                    visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(6);
                    visuals.widgets.hovered.corner_radius = egui::CornerRadius::same(6);
                    visuals.widgets.active.corner_radius = egui::CornerRadius::same(6);
                    visuals.widgets.noninteractive.corner_radius = egui::CornerRadius::same(6);
                    visuals.widgets.inactive.bg_fill = if dark { Color32::from_rgb(24, 24, 27) } else { Color32::WHITE };
                    visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, if dark { Color32::from_gray(60) } else { Color32::from_gray(210) });
                    visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, if dark { Color32::from_gray(100) } else { Color32::from_gray(170) });
                    visuals.selection.stroke = egui::Stroke::new(1.5, Color32::from_rgb(0, 111, 238));

                    ui.set_width(340.0);
                        
                    // Header with close button
                    ui.horizontal(|ui: &mut egui::Ui| {
                        ui.label(RichText::new("Add CLI Session").size(18.0).strong());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui: &mut egui::Ui| {
                            let (rect, resp) = ui.allocate_exact_size(egui::vec2(20.0, 20.0), egui::Sense::click());
                            let color = if resp.hovered() { Color32::from_rgb(220, 60, 50) } else { if dark { Color32::from_gray(140) } else { Color32::from_gray(120) } };
                            
                            let c = rect.center();
                            let s = 4.5;
                            ui.painter().line_segment([c - egui::vec2(s, s), c + egui::vec2(s, s)], egui::Stroke::new(1.5, color));
                            ui.painter().line_segment([c - egui::vec2(s, -s), c + egui::vec2(s, -s)], egui::Stroke::new(1.5, color));
                            
                            if resp.clicked() {
                                cancel = true;
                            }
                        });
                    });
                    ui.add_space(16.0);

                    ui.label(RichText::new("Name").size(12.0));
                    ui.add_sized(egui::vec2(ui.available_width(), 28.0), egui::TextEdit::singleline(&mut self.new_name).margin(egui::vec2(8.0, 5.0)));
                    ui.add_space(8.0);

                    ui.label(RichText::new("Command").size(12.0));
                    ui.add_sized(egui::vec2(ui.available_width(), 28.0), egui::TextEdit::singleline(&mut self.new_command).margin(egui::vec2(8.0, 5.0)));
                    ui.add_space(8.0);

                    ui.label(RichText::new("Working directory (optional)").size(12.0));
                    ui.horizontal(|ui: &mut egui::Ui| {
                        let browse_w = 75.0;
                        let text_w = (ui.available_width() - browse_w - ui.spacing().item_spacing.x).max(100.0);
                        ui.add_sized(egui::vec2(text_w, 28.0), egui::TextEdit::singleline(&mut self.new_cwd).margin(egui::vec2(8.0, 5.0)));
                        if ui.add_sized(egui::vec2(browse_w, 28.0), egui::Button::new("Browse...")).clicked() {
                            if let Some(path) = rfd::FileDialog::new().pick_folder() {
                                self.new_cwd = path.display().to_string();
                            }
                        }
                    });
                    ui.add_space(16.0);

                    let name_ok = !self.new_name.trim().is_empty();
                    let cmd_ok = !self.new_command.trim().is_empty();
                    
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui: &mut egui::Ui| {
                        let add_btn = egui::Button::new(RichText::new("Add Session").color(Color32::WHITE).strong())
                            .fill(Color32::from_rgb(0, 111, 238))
                            .min_size(egui::vec2(95.0, 28.0));
                        if ui.add_enabled(name_ok && cmd_ok, add_btn).clicked() {
                            confirm = true;
                        }
                        ui.add_space(4.0);
                        if ui.add_sized(egui::vec2(75.0, 28.0), egui::Button::new("Cancel")).clicked() {
                            cancel = true;
                        }
                    });
                });
            });

        if confirm {
            self.add_cli();
        }
        if confirm || cancel {
            self.adding_cli = false;
        }
    }

    /// 编辑会话的模态小窗。
    fn edit_cli_dialog(&mut self, ui: &mut egui::Ui) {
        let mut confirm = false;
        let mut cancel = false;

        let screen_rect = ui.ctx().input(|i| i.raw.screen_rect).unwrap_or_else(|| ui.max_rect());
        ui.painter().rect_filled(screen_rect, 0.0, Color32::from_black_alpha(150));

        egui::Area::new(egui::Id::new("edit_session_modal"))
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ui.ctx(), |ui: &mut egui::Ui| {
                let dark = ui.visuals().dark_mode;
                let modal_bg = if dark { Color32::from_rgb(35, 35, 35) } else { Color32::from_rgb(250, 250, 250) };
                let stroke = if dark { egui::Stroke::new(1.0, Color32::from_rgb(60, 60, 60)) } else { egui::Stroke::new(1.0, Color32::from_rgb(220, 220, 220)) };

                let frame = egui::Frame::NONE
                    .fill(modal_bg)
                    .stroke(stroke)
                    .corner_radius(16)
                    .inner_margin(32.0)
                    .shadow(egui::epaint::Shadow { offset: [0, 16], blur: 32, spread: 0, color: Color32::from_black_alpha(if dark { 120 } else { 30 }) });
                
                frame.show(ui, |ui: &mut egui::Ui| {
                    let visuals = &mut ui.style_mut().visuals;
                    visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(6);
                    visuals.widgets.hovered.corner_radius = egui::CornerRadius::same(6);
                    visuals.widgets.active.corner_radius = egui::CornerRadius::same(6);
                    visuals.widgets.noninteractive.corner_radius = egui::CornerRadius::same(6);
                    visuals.widgets.inactive.bg_fill = if dark { Color32::from_rgb(24, 24, 27) } else { Color32::WHITE };
                    visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, if dark { Color32::from_gray(60) } else { Color32::from_gray(210) });
                    visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, if dark { Color32::from_gray(100) } else { Color32::from_gray(170) });
                    visuals.selection.stroke = egui::Stroke::new(1.5, Color32::from_rgb(0, 111, 238));

                    ui.set_width(340.0);
                        
                    // Header with close button
                    ui.horizontal(|ui: &mut egui::Ui| {
                        ui.label(RichText::new("Edit CLI Session").size(18.0).strong());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui: &mut egui::Ui| {
                            let (rect, resp) = ui.allocate_exact_size(egui::vec2(20.0, 20.0), egui::Sense::click());
                            let color = if resp.hovered() { Color32::from_rgb(220, 60, 50) } else { if dark { Color32::from_gray(140) } else { Color32::from_gray(120) } };
                            
                            let c = rect.center();
                            let s = 4.5;
                            ui.painter().line_segment([c - egui::vec2(s, s), c + egui::vec2(s, s)], egui::Stroke::new(1.5, color));
                            ui.painter().line_segment([c - egui::vec2(s, -s), c + egui::vec2(s, -s)], egui::Stroke::new(1.5, color));
                            
                            if resp.clicked() {
                                cancel = true;
                            }
                        });
                    });
                    ui.add_space(16.0);

                    ui.label(RichText::new("Name").size(12.0));
                    ui.add_sized(egui::vec2(ui.available_width(), 28.0), egui::TextEdit::singleline(&mut self.edit_name).margin(egui::vec2(8.0, 5.0)));
                    ui.add_space(8.0);

                    ui.label(RichText::new("Command").size(12.0));
                    ui.add_sized(egui::vec2(ui.available_width(), 28.0), egui::TextEdit::singleline(&mut self.edit_command).margin(egui::vec2(8.0, 5.0)));
                    ui.add_space(8.0);

                    ui.label(RichText::new("Working directory (optional)").size(12.0));
                    ui.horizontal(|ui: &mut egui::Ui| {
                        let browse_w = 75.0;
                        let text_w = (ui.available_width() - browse_w - ui.spacing().item_spacing.x).max(100.0);
                        ui.add_sized(egui::vec2(text_w, 28.0), egui::TextEdit::singleline(&mut self.edit_cwd).margin(egui::vec2(8.0, 5.0)));
                        if ui.add_sized(egui::vec2(browse_w, 28.0), egui::Button::new("Browse...")).clicked() {
                            if let Some(path) = rfd::FileDialog::new().pick_folder() {
                                self.edit_cwd = path.display().to_string();
                            }
                        }
                    });
                    ui.add_space(16.0);

                    let name_ok = !self.edit_name.trim().is_empty();
                    let cmd_ok = !self.edit_command.trim().is_empty();
                    
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui: &mut egui::Ui| {
                        let save_btn = egui::Button::new(RichText::new("Save").color(Color32::WHITE).strong())
                            .fill(Color32::from_rgb(0, 111, 238))
                            .min_size(egui::vec2(85.0, 28.0));
                        if ui.add_enabled(name_ok && cmd_ok, save_btn).clicked() {
                            confirm = true;
                        }
                        ui.add_space(4.0);
                        if ui.add_sized(egui::vec2(75.0, 28.0), egui::Button::new("Cancel")).clicked() {
                            cancel = true;
                        }
                    });
                });
            });

        if confirm {
            self.edit_cli();
        }
        if confirm || cancel {
            self.editing_cli = None;
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
