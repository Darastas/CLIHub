//! 后台 IO 循环：把 PTY reader 发来的字节块喂进终端状态机。
//!
//! UI 线程每帧调用 [`drain`] 拉取积压数据，避免在 UI 线程做阻塞读。

use crossbeam_channel::Receiver;

use crate::backend::terminal::Terminal;

/// 非阻塞地取出 `rx` 中所有积压字节块，合并后一次性喂给 `terminal`。
/// 返回本次处理的总字节数。
pub fn drain(rx: &Receiver<Vec<u8>>, terminal: &mut Option<Terminal>) -> usize {
    let mut total = 0usize;
    let mut batch = Vec::with_capacity(32768);

    while let Ok(chunk) = rx.try_recv() {
        total += chunk.len();
        batch.extend_from_slice(&chunk);
        if batch.len() >= 65536 {
            if let Some(t) = terminal {
                t.feed(&batch);
            }
            batch.clear();
        }
    }

    if !batch.is_empty() {
        if let Some(t) = terminal {
            t.feed(&batch);
        }
    }

    total
}
