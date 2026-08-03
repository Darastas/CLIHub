//! 封装 `portable-pty`：派生并接管 CLI 子进程，后台线程持续读输出。

use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::{Context, Result};
use crossbeam_channel::{unbounded, Receiver};
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};

/// 一个被 PTY 接管的子进程及其读写端。
pub struct PtyHandle {
    pub master: Box<dyn MasterPty + Send>,
    pub writer: Box<dyn Write + Send>,
    pub child: Box<dyn Child + Send + Sync>,
    /// 进程存活标志；reader 线程在 EOF/出错时置为 false
    pub alive: Arc<AtomicBool>,
}

impl PtyHandle {
    /// 以指定命令在 `cwd` 下派生一个 PTY 子进程，并启动后台读线程。
    ///
    /// 返回 `(handle, rx)`：`rx` 是后台 reader 线程推送输出的通道。
    pub fn spawn(
        command: &str,
        args: &[String],
        cwd: &Path,
        rows: u16,
        cols: u16,
    ) -> Result<(Self, Receiver<Vec<u8>>)> {
        let pty_system = native_pty_system();
        let size = PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        };
        let pair = pty_system.openpty(size).context("打开 PTY 失败")?;

        // Windows 上解析命令（npm 垫片 .cmd/.bat 需用 cmd.exe /C 包装）
        let (program, args) = resolve_command(command, args);
        let mut cmd = CommandBuilder::new(&program);
        cmd.args(&args);
        cmd.cwd(cwd);
        for (k, v) in std::env::vars() {
            cmd.env(k, v);
        }
        let child = pair
            .slave
            .spawn_command(cmd)
            .context("派生子进程失败")?;
        // 释放 slave 端，只有 master 保留在父进程
        drop(pair.slave);

        let master = pair.master;
        let mut reader = master.try_clone_reader().context("克隆 reader 失败")?;
        let writer = master.take_writer().context("获取 writer 失败")?;

        let (tx, rx) = unbounded();
        let alive = Arc::new(AtomicBool::new(true));
        let reader_alive = alive.clone();

        // 后台死循环：阻塞读 PTY，经 channel 推送，避免卡死 UI 线程。
        std::thread::Builder::new()
            .name("pty-reader".to_string())
            .spawn(move || {
                let mut buf = [0u8; 16384];
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            if tx.send(buf[..n].to_vec()).is_err() {
                                break;
                            }
                        }
                    }
                }
                reader_alive.store(false, Ordering::SeqCst);
            })?;

        Ok((
            Self {
                master,
                writer,
                child,
                alive,
            },
            rx,
        ))
    }

    /// 将键盘输入写回子进程的 stdin。
    pub fn write(&mut self, data: &[u8]) -> std::io::Result<()> {
        self.writer.write_all(data)?;
        self.writer.flush()
    }

    /// 调整窗口行列数。
    pub fn resize(&mut self, rows: u16, cols: u16) {
        let _ = self.master.resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        });
    }
}

impl Drop for PtyHandle {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

// ---------------------------------------------------------------------------
// 命令解析
// ---------------------------------------------------------------------------

/// 解析要派生的程序与参数。
///
/// 非 Windows 平台原样返回。Windows 上处理 npm 垫片：
/// npm 装的 CLI（如 `claude`、`codex`）只有无扩展名的 bash shim 与
/// `*.cmd` 批处理，没有 `*.exe`。`CreateProcessW` 无法直接执行它们，
/// 必须用 `cmd.exe /C <垫片.cmd> ...` 包装。
#[cfg(windows)]
fn resolve_command(command: &str, args: &[String]) -> (String, Vec<String>) {
    let has_ext = Path::new(command)
        .extension()
        .map(|e| !e.is_empty())
        .unwrap_or(false);
    let has_sep = command.contains(['\\', '/']);

    let resolved: String = if has_ext || has_sep {
        command.to_string()
    } else {
        // 无扩展名：按 exe → cmd → bat → com 顺序在 PATH 中查找
        ["exe", "cmd", "bat", "com"]
            .iter()
            .find_map(|ext| find_in_path(command, ext))
            .unwrap_or_else(|| command.to_string())
    };

    let lower = resolved.to_lowercase();
    if lower.ends_with(".cmd") || lower.ends_with(".bat") {
        // 批处理垫片：经 cmd.exe /C 执行
        let mut wrapped = vec!["/C".to_string(), resolved];
        wrapped.extend(args.iter().cloned());
        ("cmd.exe".to_string(), wrapped)
    } else {
        (resolved, args.to_vec())
    }
}

#[cfg(not(windows))]
fn resolve_command(command: &str, args: &[String]) -> (String, Vec<String>) {
    (command.to_string(), args.to_vec())
}

/// 在 PATH 的每个目录里找 `name.<ext>` 是否存在，返回完整路径。
#[cfg(windows)]
fn find_in_path(name: &str, ext: &str) -> Option<String> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(format!("{name}.{ext}"));
        if candidate.is_file() {
            return Some(candidate.to_string_lossy().into_owned());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// npm 垫片（claude.cmd）应被 cmd.exe /C 包装。
    #[cfg(windows)]
    #[test]
    fn npm_shim_wrapped_with_cmd() {
        if find_in_path("claude", "cmd").is_none() {
            eprintln!("skip: 本机未安装 claude.cmd");
            return;
        }
        let (program, args) = resolve_command("claude", &[]);
        assert_eq!(program.to_lowercase(), "cmd.exe");
        assert_eq!(args[0], "/C");
        assert!(args[1].to_lowercase().ends_with("claude.cmd"));
    }

    /// 真正的 exe（cmd.exe）应直接派发，不包装。
    #[cfg(windows)]
    #[test]
    fn exe_command_direct() {
        let (program, args) = resolve_command("cmd", &[]);
        assert!(program.to_lowercase().ends_with("cmd.exe"));
        assert!(args.is_empty());
    }

    /// 真实 spawn 一次 `claude`：不应再报 os error 193，且能保持运行。
    /// 需本机安装 claude，故默认忽略，用 `cargo test -- --ignored` 手动跑。
    #[cfg(windows)]
    #[test]
    #[ignore]
    fn spawn_claude_real() {
        let cwd = std::env::current_dir().unwrap();
        let (mut handle, rx) =
            PtyHandle::spawn("claude", &[], &cwd, 24, 80).expect("spawn claude 失败");
        std::thread::sleep(std::time::Duration::from_secs(3));
        assert!(
            handle.alive.load(Ordering::SeqCst),
            "claude 应在 3 秒后仍保持运行"
        );
        let mut saw = String::new();
        while let Ok(chunk) = rx.try_recv() {
            saw.push_str(&String::from_utf8_lossy(&chunk));
        }
        eprintln!("[spawn_claude_real] 首屏输出预览: {}", &saw[..saw.len().min(300)]);
        drop(handle);
    }
}
