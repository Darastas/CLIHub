use super::{Message, Participant, Phase, Round, Store};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf, time::Instant};

#[derive(Clone, Serialize, Deserialize)]
pub struct MemberDraft {
    pub id: String,
    pub name: String,
    pub command: String,
    pub cwd: PathBuf,
    pub role: String,
    pub selected: bool,
}

pub struct Draft {
    pub title: String,
    pub goal: String,
    pub repo: String,
    pub members: Vec<MemberDraft>,
}

pub struct ChatState {
    pub root: PathBuf,
    pub rounds: Vec<(PathBuf, Round)>,
    pub active: Option<usize>,
    pub draft: Option<Draft>,
    pub messages: Vec<Message>,
    pub body: String,
    pub recipient: String,
    pub reply: Option<String>,
    pub error: Option<String>,
    pub last_refresh: Instant,
    pub running: Option<super::runner::Run>,
    pub run_context: Option<(PathBuf, String, String, String)>,
    pub execution_status: String,
    pub native_mode: bool,
    pub native_runs: Vec<super::native::NativeRun>,
    pub theme: crate::ui::terminal::TermTheme,
    pub chat_expanded: bool,
    automation: Option<Automation>,
}

struct Automation {
    dir: PathBuf,
    round: Round,
    members: Vec<MemberDraft>,
    task: String,
    step: usize,
}

impl ChatState {
    pub fn new(root: PathBuf) -> Self {
        let mut state = Self {
            root,
            rounds: vec![],
            active: None,
            draft: None,
            messages: vec![],
            body: String::new(),
            recipient: String::new(),
            reply: None,
            error: None,
            last_refresh: Instant::now(),
            running: None,
            run_context: None,
            execution_status: String::new(),
            native_mode: true,
            native_runs: Vec::new(),
            theme: crate::ui::terminal::TermTheme::from_scheme("One Half Dark"),
            chat_expanded: true,
            automation: None,
        };
        if let Err(e) = state.load() {
            state.error = Some(e.to_string());
        }
        state
    }

    fn load(&mut self) -> Result<()> {
        fs::create_dir_all(&self.root)?;
        let mut dirs = vec![self.root.clone()];
        for entry in fs::read_dir(&self.root)? {
            let path = entry?.path();
            if path.is_dir() {
                dirs.push(path);
            }
        }
        for dir in dirs {
            if dir.join("round.json").exists() {
                let round = Store::new(&dir)?
                    .load_round()
                    .with_context(|| format!("读取工作流 {}", dir.display()))?;
                self.rounds.push((dir, round));
            }
        }
        self.rounds
            .sort_by_key(|(_, r)| std::cmp::Reverse(r.created));
        if !self.rounds.is_empty() {
            self.select(0)?;
        }
        Ok(())
    }

    pub fn select(&mut self, index: usize) -> Result<()> {
        let (_, r) = self.rounds.get(index).context("工作流不存在")?;
        self.recipient = r
            .participants
            .iter()
            .find(|p| p.id != "user")
            .map(|p| p.id.clone())
            .unwrap_or_default();
        self.active = Some(index);
        self.reply = None;
        self.body.clear();
        self.refresh()
    }

    pub fn refresh(&mut self) -> Result<()> {
        self.poll_execution()?;
        if let Some(i) = self.active {
            self.messages = Store::new(&self.rounds[i].0)?.all_messages()?;
        }
        self.last_refresh = Instant::now();
        Ok(())
    }

    pub fn create(&mut self) -> Result<()> {
        let draft = self.draft.as_ref().context("没有工作流草稿")?;
        if draft.title.trim().is_empty() || draft.goal.trim().is_empty() {
            bail!("请填写名称和目标");
        }
        let repo = PathBuf::from(draft.repo.trim());
        if !repo.is_dir() {
            bail!("请选择有效的工作目录");
        }
        let members: Vec<_> = draft
            .members
            .iter()
            .filter(|m| m.selected)
            .cloned()
            .collect();
        if members.is_empty() {
            bail!("请至少选择一个 AI");
        }
        if members.iter().any(|m| m.role.trim().is_empty()) {
            bail!("请填写每个 AI 的职责");
        }
        let mut participants = vec![Participant {
            id: "user".into(),
            name: "我".into(),
            role: "协调者".into(),
        }];
        participants.extend(members.iter().map(|m| Participant {
            id: m.id.clone(),
            name: m.name.clone(),
            role: m.role.trim().into(),
        }));
        let id = Store::new_id("workflow");
        let dir = self.root.join(&id);
        let store = Store::new(&dir)?;
        fs::write(
            dir.join("members.json"),
            serde_json::to_vec_pretty(&members)?,
        )?;
        let round = store.create_round(Round {
            id,
            title: draft.title.trim().into(),
            goal: draft.goal.trim().into(),
            repo,
            created: Store::now(),
            phase: Phase::Development,
            participants,
        })?;
        self.rounds.insert(0, (dir, round));
        self.select(0)?;
        self.draft = None;
        Ok(())
    }

    pub fn send(&mut self) -> Result<()> {
        let i = self.active.context("请先创建工作流")?;
        if self.body.trim().is_empty() {
            bail!("消息不能为空");
        }
        let (dir, r) = &self.rounds[i];
        Store::new(dir)?.send(Message {
            id: Store::new_id("msg"),
            round: r.id.clone(),
            from: "user".into(),
            to: self.recipient.clone(),
            body: self.body.trim().into(),
            reply_to: self.reply.clone(),
            anchor: None,
            snapshot: None,
            time: Store::now(),
        })?;
        self.body.clear();
        self.reply = None;
        self.refresh()
    }

    pub fn execute(&mut self) -> Result<()> {
        if self.is_running() { bail!("已有 AI 正在执行，请等待或取消"); }
        if self.body.trim().is_empty() { bail!("消息不能为空"); }
        let i = self.active.context("请先创建工作流")?;
        let (dir, mut round) = self.rounds[i].clone();
        let mut members: Vec<MemberDraft> = serde_json::from_slice(&fs::read(dir.join("members.json")).context("旧工作流缺少 AI 启动配置，请新建工作流")?)?;
        let start = members.iter().position(|m| m.id == self.recipient).context("未找到收件人的启动配置")?;
        members.rotate_left(start);
        for m in &members { super::runner::detect(&m.command).with_context(|| format!("{} 暂不支持自动执行", m.name))?; }
        round.phase = Phase::Development;
        Store::new(&dir)?.save_round(&round)?;
        self.rounds[i].1 = round.clone();
        let task = self.body.trim().to_string();
        self.automation = Some(Automation { dir, round, members, task: task.clone(), step: 0 });
        if let Err(e) = self.launch_step("user".into(), task, self.reply.clone()) { self.automation = None; return Err(e); }
        self.error = None;
        self.body.clear(); self.reply = None;
        self.refresh()
    }

    pub fn handoff(&mut self, id: &str) -> Result<()> {
        if self.is_running() { bail!("自动协作正在执行"); }
        let message = self.messages.iter().find(|m| m.id == id).context("消息不存在")?.clone();
        let i = self.active.context("未选择工作流")?;
        let next = self.rounds[i].1.participants.iter().find(|p| p.id != "user" && p.id != message.from).context("此工作流没有其他 AI")?;
        self.recipient = next.id.clone();
        self.body = format!("请审查或继续完成以下任务结果：\n{}", message.body);
        self.reply = Some(message.id);
        self.execute()
    }

    fn launch_step(&mut self, from: String, instruction: String, reply: Option<String>) -> Result<()> {
        let auto = self.automation.as_ref().context("自动协作上下文丢失")?;
        let dir = &auto.dir;
        let round = &auto.round;
        let member = &auto.members[auto.step % auto.members.len()];
        let request_id = Store::new_id("request");
        let team = auto.members.iter().map(|m| format!("{}: {}", m.name, m.role)).collect::<Vec<_>>().join("\n");
        let mut prompt = format!("你正在全自动多 Agent 协作中，不要只确认收到任务，不要等待用户再次下令。\n你是 {}。职责：{}。\n团队：{}\n工作流目标：{}\n本次用户任务：{}\n工作目录：{}\n这是第 {} 次执行。实际执行职责，检查前一位结果，修复或验证，并给出可交接结果。只有确认整个用户任务已完成且不需要其他成员修正时，最后单独一行输出 [WORKFLOW_COMPLETE]；否则写清下一位需要继续做什么。不能把问候或接收成功当作任务完成。\n", member.name, member.role, team, round.goal, auto.task, round.repo.display(), auto.step + 1);
        let history = Store::new(dir)?.all_messages()?;
        for m in history.iter().rev().take(20).collect::<Vec<_>>().into_iter().rev() {
            prompt.push_str(&format!("\n[{} -> {}]\n{}\n", m.from, m.to, m.body));
        }
        prompt.push_str(&format!("\n[本次交接]\n{}", instruction));
        Store::new(dir)?.send(Message { id: request_id.clone(), round: round.id.clone(), from, to: member.id.clone(), body: instruction, reply_to: reply, anchor: None, snapshot: None, time: Store::now() })?;
        if self.native_mode {
            let native = super::native::NativeRun::start(&member.command, &round.repo, dir, &round.id, &member.id, &member.name, &request_id, prompt, &self.theme)?;
            self.native_runs.retain(|r| r.round != round.id || r.agent != member.id);
            self.native_runs.push(native);
        } else {
            self.running = Some(super::runner::start(&member.command, &round.repo, prompt)?);
        }
        self.run_context = Some((dir.clone(), round.id.clone(), member.id.clone(), request_id));
        self.execution_status = format!("自动协作第 {} 次 · {} · {}", auto.step + 1, member.name, member.role);
        Ok(())
    }

    pub fn cancel_execution(&mut self) {
        self.automation = None;
        if self.native_mode {
            if let Some((_, round, agent, _)) = &self.run_context {
                if let Some(run) = self.native_runs.iter_mut().find(|r| &r.round == round && &r.agent == agent) { run.cancel(); run.status = "已取消".into(); }
            }
            self.run_context = None;
            self.execution_status = "协作已取消".into();
        }
        if let Some(run) = &self.running { run.cancel(); self.execution_status = "正在取消…".into(); }
    }

    pub fn is_running(&self) -> bool { self.run_context.is_some() || self.running.is_some() }

    fn poll_execution(&mut self) -> Result<()> {
        if self.native_mode { return self.poll_native(); }
        let result = self.running.as_ref().and_then(|r| match r.rx.try_recv() {
            Ok(result) => Some(result),
            Err(crossbeam_channel::TryRecvError::Disconnected) => Some(Err("执行进程意外中断".into())),
            Err(_) => None,
        });
        if let Some(result) = result {
            self.running.take();
            let (dir, round, agent, request) = self.run_context.take().context("执行上下文丢失")?;
            match result {
                Ok(body) => {
                    let response_id = Store::new_id("response");
                    Store::new(&dir)?.send(Message { id: response_id.clone(), round: round.clone(), from: agent.clone(), to: "user".into(), body: body.clone(), reply_to: Some(request), anchor: None, snapshot: None, time: Store::now() })?;
                    if let Some(auto) = self.automation.as_mut() {
                        auto.step += 1;
                        if workflow_complete(&body, auto.step, auto.members.len()) {
                            let mut saved = Store::new(&dir)?.load_round()?;
                            saved.phase = Phase::Completed;
                            Store::new(&dir)?.save_round(&saved)?;
                            if let Some((_, r)) = self.rounds.iter_mut().find(|(_, r)| r.id == round) { *r = saved; }
                            self.automation = None;
                            self.execution_status = "自动协作完成 · 最后一位 AI 已确认".into();
                        } else {
                            let instruction = format!("上一位 {} 的结果如下。请执行你的职责，继续完成原始任务；若有问题直接修复或给出明确交接，不要等待用户确认。\n\n{}", agent, body);
                            if let Err(e) = self.launch_step(agent, instruction, Some(response_id)) {
                                self.automation = None;
                                self.execution_status = "自动交接失败".into();
                                return Err(e);
                            }
                        }
                    } else { self.execution_status = "协作已停止".into(); }
                }
                Err(error) => { self.automation = None; self.execution_status = "执行失败或已取消，自动协作已停止".into(); self.error = Some(error); }
            }
        }
        Ok(())
    }

    fn poll_native(&mut self) -> Result<()> {
        let Some((dir, round, agent, _)) = self.run_context.clone() else { return Ok(()); };
        let run = self.native_runs.iter_mut().find(|r| r.round == round && r.agent == agent).context("终端执行上下文丢失")?;
        let response = match run.poll() {
            Ok(Some(response)) => response,
            Ok(None) => return Ok(()),
            Err(error) => {
                run.stop(); run.status = "执行失败".into();
                self.automation = None; self.run_context = None;
                self.execution_status = "协作中断".into();
                return Err(error);
            }
        };
        run.stop(); run.status = "已提交结果".into();
        self.run_context = None;
        if let Some(auto) = &mut self.automation {
            auto.step += 1;
            if workflow_complete(&response.body, auto.step, auto.members.len()) {
                let mut saved = Store::new(&dir)?.load_round()?;
                saved.phase = Phase::Completed;
                Store::new(&dir)?.save_round(&saved)?;
                if let Some((_, r)) = self.rounds.iter_mut().find(|(_, r)| r.id == round) { *r = saved; }
                self.automation = None;
                self.execution_status = "任务完成 · 最后一位 AI 已确认".into();
            } else {
                if let Err(error) = self.launch_step(agent, format!("请继续原始任务，检查并接续上一位的结果：\n{}", response.body), Some(response.id)) {
                    self.automation = None;
                    self.execution_status = "自动交接失败".into();
                    return Err(error);
                }
            }
        }
        Ok(())
    }

    pub fn phase(&mut self, phase: Phase) -> Result<()> {
        let i = self.active.context("请先创建工作流")?;
        let (dir, r) = &mut self.rounds[i];
        let mut next = r.clone();
        next.phase = phase;
        Store::new(dir.clone())?.save_round(&next)?;
        *r = next;
        Ok(())
    }
}

fn workflow_complete(body: &str, completed: usize, members: usize) -> bool {
    members > 0 && completed >= members && completed % members == 0 && body.trim_end().lines().last().is_some_and(|line| line.trim() == "[WORKFLOW_COMPLETE]")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "starts native AI terminals and uses installed credentials"]
    fn real_native_handoff() {
        let root = std::env::temp_dir().join(Store::new_id("native-workflow"));
        let repo = std::env::current_dir().unwrap();
        let command = std::env::var("CLIHUB_NATIVE_TEST_CLI").unwrap_or_else(|_| "codex".into());
        let mut chat = ChatState::new(root.clone());
        chat.draft = Some(Draft { title: "Native transport verification".into(), goal: "Transport test only. Do not edit project files. Read the task and use the mailbox protocol to submit your reply.".into(), repo: repo.display().to_string(), members: ["developer", "reviewer"].iter().map(|id| MemberDraft { id: id.to_string(), name: id.to_string(), command: command.clone(), cwd: repo.clone(), role: "Verify transport then submit the reply file using the required mailbox command.".into(), selected: true }).collect() });
        chat.create().unwrap();
        chat.body = "Reply NATIVE_OK followed by [WORKFLOW_COMPLETE] on a separate line in the reply file. Follow the mailbox acknowledgment and submission protocol. Do not modify project files.".into();
        chat.execute().unwrap();
        let start = Instant::now();
        while chat.is_running() && start.elapsed().as_secs() < 600 {
            std::thread::sleep(std::time::Duration::from_millis(50));
            chat.refresh().unwrap();
        }
        for run in &chat.native_runs {
            if let Some(t) = &run.session.tabs[0].terminal {
                use alacritty_terminal::index::{Line, Column};
                let mut screen = String::new();
                for row in 0..t.rows { for col in 0..t.cols { screen.push(t.term.grid()[Line(row as i32)][Column(col as usize)].c); } screen.push('\n'); }
                eprintln!("{} {}\n{}", run.agent, run.status, screen);
            }
        }
        let completed = chat.rounds[0].1.phase == Phase::Completed;
        chat.cancel_execution();
        assert!(completed, "native workflow did not complete: {:?}", chat.error);
        assert_eq!(chat.messages.iter().filter(|m| m.to == "user" && m.body.contains("NATIVE_OK")).count(), 2);
    }

    #[test]
    #[ignore = "calls installed AI CLIs"]
    fn real_workflow_handoff() {
        let root = std::env::temp_dir().join(Store::new_id("real-workflow"));
        let mut chat = ChatState::new(root.clone());
        chat.native_mode = false;
        chat.draft = Some(Draft { title: "Transport verification".into(), goal: "Only respond to prompts. Never use tools or modify files.".into(), repo: root.display().to_string(), members: ["codex", "reviewer"].iter().map(|id| MemberDraft { id: id.to_string(), name: id.to_string(), command: "codex".into(), cwd: root.clone(), role: "Reply exactly as requested".into(), selected: true }).collect() });
        chat.create().unwrap();
        chat.recipient = "codex".into();
        chat.body = "This is a transport test. Do not use tools. Developer must reply CHAIN_FIRST_OK then [WORKFLOW_COMPLETE] on a new line. Reviewer must acknowledge the developer by replying CHAIN_SECOND_OK then [WORKFLOW_COMPLETE] on a new line. No other work is needed.".into();
        chat.execute().unwrap();
        while chat.running.is_some() { std::thread::sleep(std::time::Duration::from_millis(100)); chat.refresh().unwrap(); }
        assert!(chat.error.is_none(), "{:?}", chat.error);
        let first = chat.messages.iter().find(|m| m.from == "codex").unwrap().clone();
        assert!(first.body.contains("CHAIN_FIRST_OK"), "{}", first.body);
        let second = chat.messages.iter().find(|m| m.from == "reviewer").unwrap();
        assert!(second.body.contains("CHAIN_SECOND_OK"), "{}", second.body);
        assert_eq!(chat.rounds[0].1.phase, Phase::Completed);
        assert!(chat.messages.iter().any(|m| m.from == "codex" && m.to == "reviewer"));
        eprintln!("Automatic workflow verified: ONE execute -> developer -> reviewer -> completed");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn completion_requires_all_members_and_explicit_final_marker() {
        assert!(!workflow_complete("OK\n[WORKFLOW_COMPLETE]", 1, 2));
        assert!(workflow_complete("OK\n[WORKFLOW_COMPLETE]", 2, 2));
        assert!(!workflow_complete("收到任务", 2, 2));
        assert!(!workflow_complete("OK\n[WORKFLOW_COMPLETE]", 3, 2));
        assert!(workflow_complete("OK\n[WORKFLOW_COMPLETE]", 100, 2));
        assert!(!workflow_complete("[WORKFLOW_COMPLETE]\nbut still working", 2, 2));
    }

    #[test]
    fn selected_agents_roles_messages_and_multiple_rounds_survive_restart() {
        let root = std::env::temp_dir().join(Store::new_id("workflow-test"));
        let mut chat = ChatState::new(root.clone());
        let make = |title: &str| Draft {
            title: title.into(),
            goal: "Review changes".into(),
            repo: root.display().to_string(),
            members: vec![
                MemberDraft {
                    id: "codex".into(),
                    name: "Codex".into(),
                    command: "codex".into(),
                    cwd: root.clone(),
                    role: "Reviewer".into(),
                    selected: true,
                },
                MemberDraft {
                    id: "claude".into(),
                    name: "Claude".into(),
                    command: "claude".into(),
                    cwd: root.clone(),
                    role: "Developer".into(),
                    selected: false,
                },
            ],
        };
        chat.draft = Some(make("First"));
        chat.create().unwrap();
        assert_eq!(chat.rounds[0].1.participants.len(), 2);
        assert_eq!(chat.rounds[0].1.participants[1].role, "Reviewer");
        let first_id = chat.rounds[0].1.id.clone();
        chat.body = "Please review".into();
        chat.send().unwrap();
        chat.phase(Phase::Review).unwrap();
        chat.draft = Some(make("Second"));
        chat.create().unwrap();
        assert!(chat.messages.is_empty());
        let mut restored = ChatState::new(root.clone());
        assert!(restored.error.is_none());
        assert_eq!(restored.rounds.len(), 2);
        let first = restored
            .rounds
            .iter()
            .position(|(_, r)| r.id == first_id)
            .unwrap();
        restored.select(first).unwrap();
        assert_eq!(restored.messages.len(), 1);
        assert_eq!(restored.messages[0].to, "codex");
        assert_eq!(restored.rounds[first].1.phase, Phase::Review);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn invalid_workflow_keeps_draft_without_creating_round() {
        let root = std::env::temp_dir().join(Store::new_id("workflow-test"));
        let mut chat = ChatState::new(root.clone());
        chat.draft = Some(Draft {
            title: "Test".into(),
            goal: "Goal".into(),
            repo: root.display().to_string(),
            members: vec![],
        });
        assert!(chat.create().is_err());
        assert!(chat.draft.is_some());
        assert!(chat.rounds.is_empty());
        fs::remove_dir_all(root).unwrap();
    }
}
