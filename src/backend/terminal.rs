//! 终端字节流处理。
//!
//! Phase 3 将替换为 `alacritty_terminal` 维护字符网格；当前先提供 ANSI
//! 清理与文本解码，保证 Round 1 能直接展示 PTY 输出。

/// 从输出字节流中剥离 ANSI 转义序列并解码为文本。
///
/// 支持 CSI（`ESC [ … 最终字节`）与两字节转义（`ESC X`）；OSC
/// （`ESC ] … BEL`）这类长序列会被当作普通文本保留，属于已知限制，
/// 后续由真正的终端解析器接管。
pub fn strip_ansi_lossy(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        if c != '\u{1b}' {
            out.push(c);
            continue;
        }
        // ESC 之后是 CSI (ESC [)
        match chars.peek() {
            Some('[') => {
                chars.next();
                // 消费参数直到遇到最终字节（@–~ 范围）
                for n in chars.by_ref() {
                    if ('\u{40}'..='\u{7e}').contains(&n) {
                        break;
                    }
                }
            }
            Some(_) => {
                // 两字节转义序列：ESC X
                chars.next();
            }
            None => {}
        }
    }
    out
}

/// 把原始字节块追加到输出缓冲区（带大小上限）。
pub fn append_bytes(output: &mut String, chunk: &[u8]) {
    let text = strip_ansi_lossy(&String::from_utf8_lossy(chunk));
    output.push_str(&text);
    // 上限保护：超过 1MB 时丢弃前半段
    const CAP: usize = 1_000_000;
    if output.len() > CAP {
        output.drain(..output.len() - CAP / 2);
    }
}
