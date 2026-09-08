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
    request: String,
    cancelled: bool,
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
        let exe = std::env::current_exe()?;
        let mut helper = exe.with_file_name("clihub-agent.exe");
        if !helper.is_file() && exe.parent().is_some_and(|p| p.ends_with("deps")) {
            helper = exe.parent().unwrap().parent().unwrap().join("clihub-agent.exe");
        }
        anyhow::ensure!(helper.is_file(), "缺少配套信箱工具：{}", helper.display());
        let tasks = dir.join("tasks");
        fs::create_dir_all(&tasks)?;
        let task = tasks.join(format!("{request}.txt"));
        let body = tasks.join(format!("{request}-reply.txt"));
        let quote = |p: &Path| format!("'{}'", p.display().to_string().replace('\'', "''"));
        let prompt = format!("{prompt}\n\n交接协议（必须执行）：完成本次职责后，把可交接的最终回复写入 UTF-8 文件 {}，再通过 PowerShell 执行：\n& {} send --room {} --from '{}' --to user --reply-to '{}' --body-file {}\n这是本次执行的最后一个工具操作；提交之后不要再修改项目。仅在终端打印回复不能完成交接。最终任务完成时，回复文件末尾单独一行写 [WORKFLOW_COMPLETE]。不需要等待用户再次下令。", quote(&body), quote(&helper), quote(dir), agent, request, quote(&body));
        let prompt = format!("开始工作前先确认接收任务，通过 PowerShell 执行：\n& {} read --room {} --agent '{}' --message '{}'\n\n{prompt}", quote(&helper), quote(dir), agent, request);
        fs::write(&task, prompt)?;
        if runner::detect(command)? == runner::Kind::Codex { args.push("--no-alt-screen".into()); }
        args.push(format!("请读取任务文件 {} 并立即执行其中的任务和交接协议。", task.display()));
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

    pub fn poll(&mut self) -> Result<Option<Message>> {
        anyhow::ensure!(!self.cancelled, "任务已取消");
        let tab = &mut self.session.tabs[0];
        if let Some(pending) = &tab.pending_pty {
            match pending.try_recv() {
                Ok(result) => {
                    let (pty, rx) = match result.context("启动 AI 终端失败") {
                        Ok(value) => value,
                        Err(error) => {
                            let detail = format!("{error:#}");
                            let _ = fs::write(self.dir.join("last-start-error.txt"), &detail);
                            if let Some(terminal) = &mut tab.terminal {
                                terminal.feed_text(&detail.replace('\n', "\r\n"));
                            }
                            return Err(error);
                        }
                    };
                    tab.alive = pty.alive.clone();
                    tab.pty = Some(pty);
                    tab.rx = Some(rx);
                    tab.pending_pty = None;
                    self.status = "终端已启动 · 等待结果".into();
                }
                Err(crossbeam_channel::TryRecvError::Disconnected) => anyhow::bail!("终端启动中断"),
                Err(_) => {}
            }
        }
        if let (Some(rx), Some(term)) = (&tab.rx, &mut tab.terminal) {
            for bytes in rx.try_iter().take(256) { term.feed(&bytes); }
            if let Some(pty) = &mut tab.pty {
                for response in term.drain_pty_writes() { pty.write(response.as_bytes())?; }
            }
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
