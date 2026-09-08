use std::{fs, path::{Path, PathBuf}, sync::atomic::Ordering};
use anyhow::{Context, Result};
use crate::{backend::{pty::PtyHandle, terminal::Terminal}, state::{Session, TerminalInstance}, ui::terminal::TermTheme};
use super::{Message, Store, runner};

pub struct NativeRun {
    pub session: Session,
    pub round: String,
    pub agent: String,
    pub status: String,
    dir: PathBuf,
    pub request: String,
    cancelled: bool,
}

fn find_helper() -> Result<PathBuf> {
    let exe = std::env::current_exe()?;
    let mut helper = exe.with_file_name("clihub-agent.exe");
    if !helper.is_file() && exe.parent().is_some_and(|p| p.ends_with("deps")) {
        helper = exe.parent().unwrap().parent().unwrap().join("clihub-agent.exe");
    }
    if !helper.is_file() {
        let debug_h = PathBuf::from("target/debug/clihub-agent.exe");
        if debug_h.is_file() {
            return Ok(debug_h);
        }
        let release_h = PathBuf::from("target/release/clihub-agent.exe");
        if release_h.is_file() {
            return Ok(release_h);
        }
    }
    anyhow::ensure!(helper.is_file(), "缺少配套信箱工具：{}", helper.display());
    Ok(helper)
}

fn write_mcp_config(cwd: &Path, helper: &Path, dir: &Path, agent: &str) {
    let mcp_path = cwd.join(".mcp.json");
    let server_entry = serde_json::json!({
        "command": helper.display().to_string(),
        "args": ["mcp", "--room", dir.display().to_string(), "--agent", agent]
    });

    let mut config = if let Ok(content) = fs::read_to_string(&mcp_path) {
        serde_json::from_str::<serde_json::Value>(&content).unwrap_or_else(|_| serde_json::json!({"mcpServers": {}}))
    } else {
        serde_json::json!({"mcpServers": {}})
    };

    if let Some(servers) = config.get_mut("mcpServers").and_then(|s| s.as_object_mut()) {
        servers.insert("clihub".to_string(), server_entry);
    } else {
        config["mcpServers"] = serde_json::json!({ "clihub": server_entry });
    }

    let _ = fs::write(mcp_path, serde_json::to_string_pretty(&config).unwrap_or_default());
}

impl NativeRun {
    #[cfg(test)]
    pub fn preview(round: &str, agent: &str, name: &str) -> Self {
        let mut session = Session::new(0, name, "codex", PathBuf::new());
        let mut tab = TerminalInstance::new();
        let mut terminal = Terminal::new(80, 24, Default::default());
        terminal.feed_text("Codex\r\nWorking...\r\n$ cargo test\r\nTests passed\r\n");
        tab.terminal = Some(terminal);
        session.tabs.push(tab);
        Self { session, round: round.into(), agent: agent.into(), status: "执行中".into(), dir: PathBuf::new(), request: String::new(), cancelled: false }
    }

    pub fn start(command: &str, cwd: &Path, dir: &Path, round: &str, agent: &str, name: &str, request: &str, prompt: String, theme: &TermTheme) -> Result<Self> {
        let (program, mut args) = runner::interactive_command(command)?;
        let helper = find_helper()?;
        write_mcp_config(cwd, &helper, dir, agent);
        let tasks = dir.join("tasks");
        fs::create_dir_all(&tasks)?;
        let task = tasks.join(format!("{request}.md"));
        let body = tasks.join(format!("{request}-reply.md"));
        let quote = |p: &Path| format!("'{}'", p.display().to_string().replace('\'', "''"));
        let prompt = format!(
            "{prompt}\n\n\
            ## 协同交接协议（推荐使用原生 MCP 工具，亦支持命令行保底）\n\
            1. **MCP 原生交接（强烈推荐）**：\n\
               若当前客户端支持 MCP，可直接调用工具：\n\
               - `collab_get_task()`：查看本次待办任务、团队目标与历史交接。\n\
               - `collab_finish_step(summary, is_complete)`：结构化提交工作总结，自动交接给下一位角色。若全部目标彻底达成且无需后续角色跟进，设置 `is_complete` 为 `true`。\n\
            2. **命令行交接（保底）**：\n\
               亦可将可交接的最终回复写入 UTF-8 Markdown 文件 `{}`，并在终端中通过 PowerShell 运行以下命令完成投递：\n\
               ```powershell\n\
               & {} send --room {} --from '{}' --to user --reply-to '{}' --body-file {}\n\
               ```\n\
               > 提示：若使用命令行交接，当任务已彻底完成且不需要后续角色跟进时，请在回复文件末尾单独一行附上 `[WORKFLOW_COMPLETE]`。\n",
            body.display(), quote(&helper), quote(dir), agent, request, quote(&body)
        );
        let prompt = format!(
            "<!-- 接收确认说明 -->\n\
            开始工作前请确认接收任务（若使用 MCP，调用 collab_get_task 即可自动确认；若在命令行执行可运行）：\n\
            ```powershell\n\
            & {} read --room {} --agent '{}' --message '{}'\n\
            ```\n\n\
            {}",
            quote(&helper), quote(dir), agent, request, prompt
        );
        fs::write(&task, prompt)?;
        let kind = runner::detect(command)?;
        let task_instruction = format!(
            "【协同任务】请执行职责。支持 MCP 可直接调用 collab_get_task / collab_finish_step 自动交接，或查阅规范文件 '{}'。",
            task.display()
        );
        match kind {
            runner::Kind::Codex => {
                args.push("--no-alt-screen".into());
                args.push(task_instruction);
            }
            runner::Kind::Claude => {
                args.push(task_instruction);
            }
            runner::Kind::Antigravity => {
                args.push("-i".into());
                args.push(task_instruction);
            }
            runner::Kind::Opencode => {
                args.push("--prompt".into());
                args.push(task_instruction);
            }
            runner::Kind::OhMyPi => {
                args.push(task_instruction);
            }
            runner::Kind::Mimo => {
                args.push("--prompt".into());
                args.push(task_instruction);
            }
            runner::Kind::Aider => {
                args.push("--message".into());
                args.push(task_instruction);
            }
            runner::Kind::Gemini | runner::Kind::Generic => {
                args.push(task_instruction);
            }
        }
        let mut session = Session::new(0, name, command, cwd.to_path_buf());
        let mut tab = TerminalInstance::new();
        tab.terminal = Some(Terminal::new(100, 30, theme.to_theme_colors()));
        let (tx, rx) = crossbeam_channel::bounded(1);
        let cwd = cwd.to_path_buf();
        let dark = theme.is_dark();
        std::thread::Builder::new().name("collab-native-start".into()).spawn(move || {
            let result = PtyHandle::spawn(&program.to_string_lossy(), &args, &cwd, 30, 100, dark, None)
                .with_context(|| format!("启动程序 {}，工作目录 {}", program.display(), cwd.display()));
            let _ = tx.send(result);
        })?;
        tab.pending_pty = Some(rx);
        session.tabs.push(tab);
        Ok(Self { session, round: round.into(), agent: agent.into(), status: "正在启动".into(), dir: dir.into(), request: request.into(), cancelled: false })
    }

    pub fn cancel(&mut self) { self.cancelled = true; self.stop(); }

    pub fn stop(&mut self) {
        for tab in &mut self.session.tabs {
            tab.pending_pty.take();
            tab.pty.take();
            tab.alive.store(false, Ordering::SeqCst);
        }
    }

    /// 打开 Agent 交互终端（默认打开状态，供用户选择模型、配置 skill、交互调试）
    pub fn open_interactive(command: &str, cwd: &Path, dir: &Path, round: &str, agent: &str, name: &str, theme: &TermTheme) -> Self {
        if let Ok(helper) = find_helper() {
            write_mcp_config(cwd, &helper, dir, agent);
        }
        let mut session = Session::new(0, name, command, cwd.to_path_buf());
        let mut tab = TerminalInstance::new();
        let mut terminal = Terminal::new(70, 30, theme.to_theme_colors());

        let cmd_res = runner::interactive_command(command);
        let mut status = "正在启动...".to_string();

        match cmd_res {
            Ok((program, mut args)) => {
                if let Ok(kind) = runner::detect(command) {
                    if kind == runner::Kind::Codex {
                        args.push("--no-alt-screen".into());
                    }
                }
                let (tx, rx) = crossbeam_channel::bounded(1);
                let cwd = cwd.to_path_buf();
                let dark = theme.is_dark();
                let spawn_res = std::thread::Builder::new().name("collab-native-interactive".into()).spawn(move || {
                    let result = PtyHandle::spawn(&program.to_string_lossy(), &args, &cwd, 30, 70, dark, None)
                        .with_context(|| format!("启动交互终端 {}，工作目录 {}", program.display(), cwd.display()));
                    let _ = tx.send(result);
                });
                if let Err(e) = spawn_res {
                    terminal.feed_text(&format!("创建启动线程失败: {e}\r\n"));
                    status = "启动失败".into();
                } else {
                    tab.pending_pty = Some(rx);
                }
            }
            Err(err) => {
                let msg = format!("终端定位失败: {err:#}\r\n请检查该 Agent 命令配置或安装路径。\r\n");
                terminal.feed_text(&msg.replace('\n', "\r\n"));
                status = "启动失败".into();
            }
        }

        tab.terminal = Some(terminal);
        session.tabs.push(tab);
        Self {
            session,
            round: round.into(),
            agent: agent.into(),
            status,
            dir: dir.into(),
            request: String::new(),
            cancelled: false,
        }
    }

    /// 向已启动的交互终端直接投递任务与协同指令，保留用户已选的模型与会话上下文
    pub fn dispatch(&mut self, _command: &str, cwd: &Path, dir: &Path, request: &str, prompt: String) -> Result<()> {
        let helper = find_helper()?;
        write_mcp_config(cwd, &helper, dir, &self.agent);
        let tasks = dir.join("tasks");
        fs::create_dir_all(&tasks)?;
        let task = tasks.join(format!("{request}.md"));
        let body = tasks.join(format!("{request}-reply.md"));
        let quote = |p: &Path| format!("'{}'", p.display().to_string().replace('\'', "''"));
        let prompt_file_content = format!(
            "{prompt}\n\n\
            ## 协同交接协议（推荐使用原生 MCP 工具，亦支持命令行保底）\n\
            1. **MCP 原生交接（强烈推荐）**：\n\
               若当前客户端支持 MCP，可直接调用工具：\n\
               - `collab_get_task()`：查看本次待办任务、团队目标与历史交接。\n\
               - `collab_finish_step(summary, is_complete)`：结构化提交工作总结，自动交接给下一位角色。若全部目标彻底达成且无需后续角色跟进，设置 `is_complete` 为 `true`。\n\
            2. **命令行交接（保底）**：\n\
               亦可将可交接的最终回复写入 UTF-8 Markdown 文件 `{}`，并在终端中通过 PowerShell 运行以下命令完成投递：\n\
               ```powershell\n\
               & {} send --room {} --from '{}' --to user --reply-to '{}' --body-file {}\n\
               ```\n\
               > 提示：若使用命令行交接，当任务已彻底完成且不需要后续角色跟进时，请在回复文件末尾单独一行附上 `[WORKFLOW_COMPLETE]`。\n",
            body.display(), quote(&helper), quote(dir), self.agent, request, quote(&body)
        );
        let prompt_file_content = format!(
            "<!-- 接收确认说明 -->\n\
            开始工作前请确认接收任务（若使用 MCP，调用 collab_get_task 即可自动确认；若在命令行执行可运行）：\n\
            ```powershell\n\
            & {} read --room {} --agent '{}' --message '{}'\n\
            ```\n\n\
            {}",
            quote(&helper), quote(dir), self.agent, request, prompt_file_content
        );
        fs::write(&task, prompt_file_content)?;

        let tab = &mut self.session.tabs[0];
        if tab.pty.is_none() {
            if let Some(pending) = tab.pending_pty.take() {
                if let Ok(Ok((pty, rx))) = pending.recv_timeout(std::time::Duration::from_millis(1500)) {
                    tab.alive = pty.alive.clone();
                    tab.pty = Some(pty);
                    tab.rx = Some(rx);
                }
            }
        }
        if let Some(pty) = &mut tab.pty {
            if tab.alive.load(Ordering::Relaxed) {
                let instruction = format!(
                    "【协同任务已派发】\r\n- 支持 MCP（推荐）：直接调用 `collab_get_task()` 查看任务，完成后调用 `collab_finish_step(summary, is_complete)` 自动交接。\r\n- 规范文件已生成：{}\r\n请立即开始执行职责。\r\n",
                    task.display()
                );
                pty.write(instruction.as_bytes())?;
                self.request = request.into();
                self.status = "已指派任务 · 执行中".into();
                return Ok(());
            }
        }
        anyhow::bail!("终端未就绪或已退出")
    }

    pub fn poll(&mut self) -> Result<Option<Message>> {
        anyhow::ensure!(!self.cancelled, "任务已取消");
        let tab = &mut self.session.tabs[0];
        if let Some(pending) = &tab.pending_pty {
            match pending.try_recv() {
                Ok(result) => {
                    tab.pending_pty = None;
                    let (pty, rx) = match result.context("启动 AI 终端失败") {
                        Ok(value) => value,
                        Err(error) => {
                            let detail = format!("{error:#}");
                            let _ = fs::write(self.dir.join("last-start-error.txt"), &detail);
                            if let Some(terminal) = &mut tab.terminal {
                                terminal.feed_text(&detail.replace('\n', "\r\n"));
                            }
                            self.status = "启动失败".into();
                            return Err(error);
                        }
                    };
                    tab.alive = pty.alive.clone();
                    tab.pty = Some(pty);
                    tab.rx = Some(rx);
                    if self.request.is_empty() {
                        self.status = "就绪 · 可交互".into();
                    } else {
                        self.status = "终端已启动 · 等待结果".into();
                    }
                }
                Err(crossbeam_channel::TryRecvError::Disconnected) => {
                    tab.pending_pty = None;
                    self.status = "启动中断".into();
                    anyhow::bail!("终端启动中断");
                }
                Err(crossbeam_channel::TryRecvError::Empty) => {}
            }
        }
        if let (Some(rx), Some(term)) = (&tab.rx, &mut tab.terminal) {
            for bytes in rx.try_iter().take(256) { term.feed(&bytes); }
            if let Some(pty) = &mut tab.pty {
                for response in term.drain_pty_writes() { pty.write(response.as_bytes())?; }
            }
        }
        if self.request.is_empty() {
            if let Some(pty) = &mut tab.pty {
                if let Ok(Some(_status)) = pty.child.try_wait() {
                    tab.alive.store(false, Ordering::SeqCst);
                    self.status = "已退出".into();
                }
            }
            return Ok(None);
        }
        let store = Store::new(&self.dir)?;
        if store.is_read(&self.agent, &self.request) { self.status = "已接收任务 · 执行中".into(); }
        let response = store.all_messages()?.into_iter().find(|m| {
            m.from == self.agent && m.to == "user" && m.reply_to.as_deref() == Some(&self.request)
        });
        if response.is_some() { return Ok(response); }
        if let Some(pty) = &mut tab.pty {
            if let Some(status) = pty.child.try_wait()? {
                anyhow::bail!("AI 终端已退出（{status:?}），尚未提交交接结果");
            }
        }
        Ok(None)
    }
}
