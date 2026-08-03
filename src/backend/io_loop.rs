//! 后台 IO 循环：把 PTY reader 发来的字节块落进会话输出缓冲区。
//!
//! UI 线程每帧调用 [`drain`] 拉取积压数据，避免在 UI 线程做阻塞读。

use std::sync::Mutex;

use crossbeam_channel::Receiver;

use crate::backend::terminal::append_bytes;

/// 非阻塞地取出 `rx` 中所有积压字节块并追加到 `output`。
/// 返回本次处理的总字节数。
pub fn drain(rx: &Receiver<Vec<u8>>, output: &Mutex<String>) -> usize {
    let mut total = 0usize;
    while let Ok(chunk) = rx.try_recv() {
        total += chunk.len();
        if let Ok(mut guard) = output.lock() {
            append_bytes(&mut guard, &chunk);
        }
    }
    total
}
