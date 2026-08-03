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

        let mut cmd = CommandBuilder::new(command);
        cmd.args(args);
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
