use std::{
    ffi::OsString,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

use anyhow::{Context, Result, bail};
use crossbeam_channel::{Receiver, bounded};
use serde_json::Value;

use crate::backend::process_guard::ProcessJobGuard;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Codex,
    Claude,
}

pub struct Run {
    pub rx: Receiver<Result<String, String>>,
    cancelled: Arc<AtomicBool>,
}

impl Run {
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}

impl Drop for Run {
    fn drop(&mut self) {
        self.cancel();
    }
}

fn command_path(command: &str) -> Result<PathBuf> {
    let command = command.trim();
    let command = command
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .unwrap_or(command);
    if command == "codex" || command == "claude" {
        return Ok(PathBuf::from(command));
    }
    let path = PathBuf::from(command);
    if !path.is_absolute() || !path.is_file() {
        bail!("仅支持 codex、claude 或 CLI 可执行文件的完整路径，不支持附加参数和 Shell 命令");
    }
    Ok(path)
}

pub fn detect(command: &str) -> Result<Kind> {
    let path = command_path(command)?;
    match path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "codex" | "codex.exe" | "codex.cmd" | "codex.ps1" => Ok(Kind::Codex),
        "claude" | "claude.exe" | "claude.cmd" | "claude.ps1" => Ok(Kind::Claude),
        _ => bail!("当前协作仅支持 Codex 和 Claude CLI"),
    }
}

fn find_on_path(name: &str) -> Option<PathBuf> {
    let dirs: Vec<_> = std::env::split_paths(&std::env::var_os("PATH")?).collect();
    // Prefer a native executable over a shell shim, including in later PATH entries.
    let suffixes: &[&str] = if cfg!(windows) {
        &[".exe", ".cmd", ".ps1"]
    } else {
        &[""]
    };
    for suffix in suffixes {
        for dir in &dirs {
            let path = dir.join(format!("{name}{suffix}"));
            if path.is_file() {
                return Some(path);
            }
        }
    }
    None
}

fn executable(path: PathBuf, kind: Kind) -> Result<(PathBuf, Vec<OsString>)> {
    let extension = path
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if extension != "cmd" && extension != "ps1" {
        return Ok((path, Vec::new()));
    }
    let dir = path.parent().context("CLI 路径没有父目录")?;
    let name = match kind {
        Kind::Codex => "codex",
        Kind::Claude => "claude",
    };
    let adjacent = dir.join(format!("{name}.exe"));
    if adjacent.is_file() {
        return Ok((adjacent, Vec::new()));
    }
    match kind {
        Kind::Codex => {
            let package = dir.join("node_modules/@openai/codex");
            let target = if cfg!(target_arch = "aarch64") {
                "aarch64-pc-windows-msvc"
            } else {
                "x86_64-pc-windows-msvc"
            };
            let platform = if cfg!(target_arch = "aarch64") {
                "codex-win32-arm64"
            } else {
                "codex-win32-x64"
            };
            for native in [
                package.join(format!("vendor/{target}/codex/codex.exe")),
                dir.join(format!(
                    "node_modules/@openai/{platform}/vendor/{target}/codex/codex.exe"
                )),
                package.join(format!(
                    "node_modules/@openai/{platform}/vendor/{target}/codex/codex.exe"
                )),
            ] {
                if native.is_file() {
                    return Ok((native, Vec::new()));
                }
            }
            let script = package.join("bin/codex.js");
            if script.is_file() {
                let local_node = dir.join("node.exe");
                let node = if local_node.is_file() {
                    local_node
                } else {
                    find_on_path("node")
                        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("exe")))
                        .context("找不到 node.exe，无法启动 Codex npm 安装")?
                };
                return Ok((node, vec![script.into_os_string()]));
            }
        }
        Kind::Claude => {
            let native = dir.join("node_modules/@anthropic-ai/claude-code/bin/claude.exe");
            if native.is_file() {
                return Ok((native, Vec::new()));
            }
        }
    }
    bail!(
        "无法从 {} 定位受支持的 CLI 入口，请配置原生可执行文件路径",
        path.display()
    )
}

pub fn interactive_command(command: &str) -> Result<(PathBuf, Vec<String>)> {
    let kind = detect(command)?;
    let path = command_path(command)?;
    let path = if path.is_absolute() { path } else {
        find_on_path(path.to_str().unwrap_or_default()).context("PATH 中未找到 CLI")?
    };
    let (program, args) = executable(path, kind)?;
    Ok((program, args.into_iter().map(|s| s.to_string_lossy().into_owned()).collect()))
}

fn hidden(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    #[cfg(not(windows))]
    let _ = command;
}

pub fn start(command: &str, cwd: &Path, prompt: String) -> Result<Run> {
    let kind = detect(command)?;
    let path = command_path(command)?;
    let path = if path.is_absolute() {
        path
    } else {
        find_on_path(path.to_str().unwrap_or_default())
            .context("PATH 中未找到对应 CLI，请先安装或配置完整路径")?
    };
    let (program, args) = executable(path, kind)?;
    if !cwd.is_dir() {
        bail!("工作目录不存在：{}", cwd.display());
    }
    let cwd = cwd.to_path_buf();
    let cancelled = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&cancelled);
    let (tx, rx) = bounded(1);
    thread::Builder::new()
        .name("collab-cli".into())
        .spawn(move || {
            let result = execute(program, args, kind, &cwd, prompt, flag);
            let _ = tx.send(result.map_err(|error| format!("{error:#}")));
        })
        .context("启动 CLI 后台任务失败")?;
    Ok(Run { rx, cancelled })
}

fn read_pipe(mut pipe: impl Read + Send + 'static) -> Receiver<std::io::Result<String>> {
    let (tx, rx) = bounded(1);
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = pipe
            .read_to_end(&mut bytes)
            .map(|_| String::from_utf8_lossy(&bytes).into_owned());
        let _ = tx.send(result);
    });
    rx
}

fn terminate(child: &mut Child, guard: &mut Option<ProcessJobGuard>) {
    if guard.take().is_none() {
        #[cfg(windows)]
        {
            let mut command = Command::new("taskkill.exe");
            command
                .args(["/PID", &child.id().to_string(), "/T", "/F"])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            hidden(&mut command);
            let _ = command.status();
        }
    }
    let _ = child.kill();
    let _ = child.wait();
}

fn execute(
    program: PathBuf,
    args: Vec<OsString>,
    kind: Kind,
    cwd: &Path,
    prompt: String,
    flag: Arc<AtomicBool>,
) -> Result<String> {
    if flag.load(Ordering::Relaxed) {
        bail!("任务已取消");
    }
    let mut command = Command::new(program);
    command
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    match kind {
        Kind::Codex => {
            command.args([
                "exec",
                "--json",
                "--color",
                "never",
                "--skip-git-repo-check",
                "-",
            ]);
        }
        Kind::Claude => {
            command.args(["-p", "--output-format", "json"]);
        }
    }
    hidden(&mut command);
    let mut child = command.spawn().context("启动 CLI 失败")?;
    let mut guard = ProcessJobGuard::new().filter(|guard| guard.assign_process_by_id(child.id()));
    let stdout = read_pipe(child.stdout.take().context("读取 CLI stdout 失败")?);
    let stderr = read_pipe(child.stderr.take().context("读取 CLI stderr 失败")?);
    let mut stdin = child.stdin.take().context("打开 CLI stdin 失败")?;
    let (input_tx, input_rx) = bounded(1);
    thread::spawn(move || {
        let _ = input_tx.send(stdin.write_all(prompt.as_bytes()));
    });
    let status = loop {
        if flag.load(Ordering::Relaxed) {
            terminate(&mut child, &mut guard);
            bail!("任务已取消");
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => thread::sleep(Duration::from_millis(50)),
            Err(error) => {
                terminate(&mut child, &mut guard);
                return Err(error).context("等待 CLI 失败");
            }
        }
    };
    drop(guard);
    let output = stdout
        .recv_timeout(Duration::from_secs(2))
        .context("CLI 输出未正常关闭")??;
    let errors = stderr
        .recv_timeout(Duration::from_secs(2))
        .context("CLI 错误输出未正常关闭")??;
    if !status.success() {
        bail!(
            "CLI 退出失败（{}）：{}",
            status,
            diagnostic(if errors.trim().is_empty() {
                &output
            } else {
                &errors
            })
        );
    }
    input_rx
        .recv_timeout(Duration::from_secs(1))
        .context("CLI 输入未完成")?
        .context("写入 CLI 提示词失败")?;
    parse_output(kind, &output)
}

fn diagnostic(text: &str) -> String {
    text.trim().chars().take(2000).collect()
}

fn parse_output(kind: Kind, output: &str) -> Result<String> {
    let events = serde_json::Deserializer::from_str(output.trim_start_matches('\u{feff}'))
        .into_iter::<Value>();
    let mut answer = String::new();
    for event in events {
        let event = event.context("CLI 返回了无效的 JSON")?;
        match kind {
            Kind::Codex => {
                let event_type = event["type"].as_str().unwrap_or_default();
                if matches!(event_type, "error" | "turn.failed") {
                    bail!("Codex 执行失败：{}", diagnostic(&event.to_string()));
                }
                if event_type == "item.completed" && event["item"]["type"] == "agent_message" {
                    if let Some(text) = event["item"]["text"].as_str() {
                        if !answer.is_empty() {
                            answer.push('\n');
                        }
                        answer.push_str(text);
                    }
                }
            }
            Kind::Claude => {
                if event["permission_denials"].as_array().is_some_and(|items| !items.is_empty()) {
                    bail!("Claude 请求的工具权限被拒绝，任务未完整执行：{}", diagnostic(&event["permission_denials"].to_string()));
                }
                if event["is_error"].as_bool() == Some(true)
                    || event["type"] == "error"
                    || event["subtype"]
                        .as_str()
                        .is_some_and(|value| value.starts_with("error"))
                {
                    bail!("Claude 执行失败：{}", diagnostic(&event.to_string()));
                }
                if let Some(result) = event["result"].as_str() {
                    answer = result.to_owned();
                }
            }
        }
    }
    if answer.trim().is_empty() {
        bail!("CLI 没有返回最终文本回复");
    }
    Ok(answer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "starts installed CLI in a real ConPTY"]
    fn installed_codex_pty_startup() {
        let (program, mut args) = interactive_command("codex").unwrap();
        eprintln!("Resolved executable: {}", program.display());
        args.push("--version".into());
        let cwd = std::env::var_os("CLIHUB_TEST_CWD").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
        let (pty, rx) = crate::backend::pty::PtyHandle::spawn(&program.to_string_lossy(), &args, &cwd, 30, 100, true, None).unwrap();
        let mut output = Vec::new();
        while let Ok(bytes) = rx.recv_timeout(Duration::from_secs(10)) { output.extend(bytes); }
        drop(pty);
        let output = String::from_utf8_lossy(&output);
        eprintln!("{output}");
        assert!(output.contains("codex-cli"));
    }

    #[test]
    #[ignore = "starts installed CLI"]
    fn real_cancel() {
        let job = start("codex", &std::env::temp_dir(), "Do not use tools. Reply OK.".into()).unwrap();
        job.cancel();
        let result = job.rx.recv_timeout(Duration::from_secs(10)).unwrap();
        assert!(result.unwrap_err().contains("取消"));
    }

    #[test]
    fn codex_reads_multiline_json_and_ignores_tools() {
        let output = r#"{"type":"item.completed","item":{"type":"command_execution","text":"ignore"}}
        {
          "type":"item.completed",
          "item":{"type":"agent_message","text":"first\nsecond"}
        }
        {"type":"turn.completed"}"#;
        assert_eq!(parse_output(Kind::Codex, output).unwrap(), "first\nsecond");
    }

    #[test]
    fn codex_failure_after_text_is_failure() {
        let output = r#"{"type":"item.completed","item":{"type":"agent_message","text":"partial"}}
        {"type":"turn.failed","error":{"message":"failed"}}"#;
        assert!(parse_output(Kind::Codex, output).is_err());
        assert!(parse_output(Kind::Codex, r#"{"type":"error","message":"failed"}"#).is_err());
    }

    #[test]
    fn claude_reports_error_even_with_result() {
        assert!(
            parse_output(
                Kind::Claude,
                r#"{"type":"result","is_error":true,"result":"bad"}"#
            )
            .is_err()
        );
        assert!(
            parse_output(
                Kind::Claude,
                r#"{"type":"result","subtype":"error_max_turns","result":"partial"}"#
            )
            .is_err()
        );
        assert_eq!(
            parse_output(
                Kind::Claude,
                "{\n\"type\":\"result\",\"is_error\":false,\"result\":\"done\"\n}"
            )
            .unwrap(),
            "done"
        );
    }

    #[test]
    fn rejects_missing_answers_and_shell_commands() {
        assert!(parse_output(Kind::Codex, "").is_err());
        assert!(parse_output(Kind::Claude, "{}").is_err());
        assert!(detect("codex --yolo").is_err());
        assert!(detect("claude && echo secret").is_err());
        assert_eq!(detect("codex").unwrap(), Kind::Codex);
    }

    #[test]
    #[ignore = "uses installed CLI credentials and makes real model requests"]
    fn real_smoke() {
        for (command, token) in [
            ("codex", "CLIHUB_REAL_SMOKE_CODEX_74931"),
            ("claude", "CLIHUB_REAL_SMOKE_CLAUDE_86247"),
        ] {
            let prompt = format!(
                "Do not use any tools or inspect files. Respond with exactly this text and nothing else: {token}"
            );
            let run = start(command, &std::env::temp_dir(), prompt).unwrap();
            let result = run
                .rx
                .recv_timeout(Duration::from_secs(190))
                .unwrap()
                .unwrap();
            println!("{command}: {result}");
            assert_eq!(
                result.trim(),
                token,
                "{command} did not receive the prompt correctly"
            );
        }
    }
}
