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
    /// 上次 resize 的行列，防重复触发 ConPTY 重绘
    cols: u16,
    rows: u16,
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
                cols,
                rows,
            },
            rx,
        ))
    }

    /// 将键盘输入写回子进程的 stdin。
    pub fn write(&mut self, data: &[u8]) -> std::io::Result<()> {
        self.writer.write_all(data)?;
        self.writer.flush()
    }

    /// 调整窗口行列数；尺寸未变化时跳过，避免每帧重复 resize
    /// 触发 ConPTY 反复重绘（表现为海量 `ESC[K` 清屏指令）。
    pub fn resize(&mut self, cols: u16, rows: u16) {
        if cols == 0 || rows == 0 {
            return;
        }
        if self.cols == cols && self.rows == rows {
            return;
        }
        self.cols = cols;
        self.rows = rows;
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

    /// cmd.exe 经 PTY 是否产出原始字节流（诊断：上层网格空白时定位）。
    #[cfg(windows)]
    #[test]
    #[ignore]
    fn cmd_raw_output() {
        let cwd = std::env::current_dir().unwrap();
        let (mut handle, rx) = PtyHandle::spawn("cmd", &[], &cwd, 24, 80).expect("spawn cmd");
        std::thread::sleep(std::time::Duration::from_secs(2));
        let mut all = Vec::new();
        while let Ok(chunk) = rx.try_recv() {
            all.extend_from_slice(&chunk);
        }
        eprintln!(
            "[cmd_raw_output] {} bytes: {:?}",
            all.len(),
            String::from_utf8_lossy(&all)
        );
        drop(handle);
    }

    /// 完整闭环：spawn cmd → 应答 DSR → 网格应出现 banner/prompt。
    /// 复现 app 的 update_backend 逻辑。
    #[cfg(windows)]
    #[test]
    #[ignore]
    fn cmd_through_terminal() {
        use crate::backend::terminal::Terminal;

        let cwd = std::env::current_dir().unwrap();
        let (mut handle, rx) = PtyHandle::spawn("cmd", &[], &cwd, 24, 80).unwrap();
        let mut term = Terminal::new(24, 80);

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(4);
        let mut writes_roundtripped = 0usize;
        while std::time::Instant::now() < deadline {
            for text in term.drain_pty_writes() {
                writes_roundtripped += 1;
                let _ = handle.write(text.as_bytes());
            }
            while let Ok(chunk) = rx.try_recv() {
                term.feed(&chunk);
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        eprintln!("[cmd_through_terminal] 应答了 {writes_roundtripped} 次 pty_write");

        let grid = term.term.grid();
        let mut lines: Vec<String> = Vec::new();
        for item in grid.display_iter() {
            let li = item.point.line.0 as usize;
            if li >= lines.len() {
                lines.resize(li + 1, String::new());
            }
            lines[li].push(item.cell.c);
        }
        for (i, l) in lines.iter().take(6).enumerate() {
            eprintln!("[cmd_through_terminal] 行 {i}: {:?}", l);
        }
        assert!(
            lines.iter().any(|l| !l.trim().is_empty()),
            "闭环 4 秒后网格仍全空"
        );
        drop(handle);
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
