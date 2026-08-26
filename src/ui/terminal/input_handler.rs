//! 终端按键映射、输入法转发与鼠标滚轮事件处理。

use egui::{Modifiers, Rect, Ui};

use crate::state::TerminalInstance;
use super::clipboard::{get_clipboard_text, set_clipboard_text};

/// 处理按键事件并写回 PTY
pub fn forward_keys(ui: &mut Ui, tab: &mut TerminalInstance) -> Option<String> {
    let find_input_id = ui.id().with("find_input");
    if ui.memory(|m| m.has_focus(find_input_id)) {
        return None; // 搜索框处于输入状态，绝对不向 PTY 终端转发按键
    }

    let mut out: Vec<u8> = Vec::new();
    let events = ui.input(|i| i.events.clone());

    for ev in events {
        match ev {
            // ---- IME 事件 ----
            egui::Event::Ime(egui::ImeEvent::Preedit { text, .. }) => {
                tab.ime_composing = !text.is_empty();
                tab.ime_preedit = text;
            }
            egui::Event::Ime(egui::ImeEvent::Commit(text)) => {
                tab.ime_composing = false;
                tab.ime_preedit.clear();
                tab.ime_just_committed_text = Some(text.clone());
                out.extend_from_slice(text.as_bytes());
            }
            #[allow(deprecated)]
            egui::Event::Ime(_) => {}

            // ---- 以下事件在 IME 组合期间全部跳过 ----
            _ if tab.ime_composing => {
                continue;
            }

            // ---- 普通事件（非 IME 组合态）----
            egui::Event::Copy => {
                // 1) 若存在选区：优先执行智能复制，绝不杀死当前运行任务
                if let Some(t) = &mut tab.terminal {
                    if t.selection.is_some() {
                        if let Some(sel) = t.selected_text() {
                            if !sel.is_empty() {
                                set_clipboard_text(&sel);
                                ui.ctx().copy_text(sel);
                            }
                        }
                        t.selection = None;
                        continue;
                    }
                }

                // 2) 无选区：执行双击 Ctrl+C 防误触中断保护 (1.8秒内连按两次才发送 SIGINT)
                let now = std::time::Instant::now();
                let is_double_press = if let Some(last) = tab.last_ctrl_c {
                    now.duration_since(last).as_millis() <= 1800
                } else {
                    false
                };

                if is_double_press {
                    tab.last_ctrl_c = None;
                    out.extend_from_slice(b"\x03"); // 发送 SIGINT (^C)
                } else {
                    tab.last_ctrl_c = Some(now);
                    ui.ctx().request_repaint_after(std::time::Duration::from_millis(1800));
                }
            }
            egui::Event::Paste(text) => out.extend_from_slice(text.as_bytes()),
            egui::Event::Text(text) => {
                if let Some(mut committed) = tab.ime_just_committed_text.take() {
                    if committed.starts_with(text.as_str()) {
                        committed.drain(..text.len());
                        if !committed.is_empty() {
                            tab.ime_just_committed_text = Some(committed);
                        }
                        continue;
                    }
                }

                for ch in text.chars() {
                    let c = ch as u32;
                    if c < 0x20 || c == 0x7f {
                        continue; // 控制字符交给 Key 路径
                    }
                    let mut buf = [0u8; 4];
                    out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                }
            }
            egui::Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } => {
                let is_ctrl_or_cmd = modifiers.ctrl || modifiers.command;

                // 智能 Ctrl+C 与 双击 Ctrl+C 防误触保护
                let is_ctrl_c = is_ctrl_or_cmd && !modifiers.shift && !modifiers.alt && key == egui::Key::C;
                if is_ctrl_c {
                    if let Some(t) = &mut tab.terminal {
                        if t.selection.is_some() {
                            if let Some(sel) = t.selected_text() {
                                if !sel.is_empty() {
                                    set_clipboard_text(&sel);
                                    ui.ctx().copy_text(sel);
                                }
                            }
                            t.selection = None;
                            continue;
                        }
                    }

                    let now = std::time::Instant::now();
                    let is_double_press = if let Some(last) = tab.last_ctrl_c {
                        now.duration_since(last).as_millis() <= 1800
                    } else {
                        false
                    };

                    if is_double_press {
                        tab.last_ctrl_c = None;
                        out.extend_from_slice(b"\x03"); // 发送 SIGINT (^C)
                    } else {
                        tab.last_ctrl_c = Some(now);
                        ui.ctx().request_repaint_after(std::time::Duration::from_millis(1800));
                    }
                    continue;
                }

                // 终端专用复制快捷键（Ctrl+Shift+C / Cmd+Shift+C / Ctrl+Insert）
                let is_copy_key = (is_ctrl_or_cmd && modifiers.shift && key == egui::Key::C)
                    || (key == egui::Key::Insert && modifiers.ctrl);

                if is_copy_key {
                    if let Some(t) = &mut tab.terminal {
                        if let Some(sel) = t.selected_text() {
                            if !sel.is_empty() {
                                set_clipboard_text(&sel);
                                ui.ctx().copy_text(sel);
                            }
                        }
                        t.selection = None;
                    }
                    continue;
                }

                // 搜索快捷键：Ctrl+F / Cmd+F 唤起终端内搜索条
                if is_ctrl_or_cmd && !modifiers.shift && !modifiers.alt && key == egui::Key::F {
                    tab.search_state.is_open = true;
                    tab.search_state.request_focus = true;
                    if let Some(sel) = tab.terminal.as_ref().and_then(|t| t.selected_text()) {
                        let trimmed = sel.trim();
                        if !trimmed.is_empty() {
                            tab.search_state.query = trimmed.to_string();
                            if let Some(t) = &tab.terminal {
                                tab.search_state.matches = t.search(&tab.search_state.query, tab.search_state.case_sensitive);
                                tab.search_state.active_match = 0;
                            }
                        }
                    }
                    continue;
                }

                // 粘贴快捷键：
                // 1) 终端标准 Ctrl+Shift+V / Cmd+Shift+V
                // 2) 终端标准 Shift+Insert
                let is_paste_key = (is_ctrl_or_cmd && modifiers.shift && key == egui::Key::V)
                    || (modifiers.shift && key == egui::Key::Insert);

                if is_paste_key {
                    if let Some(clip) = get_clipboard_text() {
                        out.extend_from_slice(clip.as_bytes());
                    }
                    continue;
                }

                // 暂存区生命周期与提交控制：
                // 1) 常规 Enter 提交：若暂存区有图片，先注入路径并在独立时钟周期内提交回车，实现单次回车即发即走
                let is_regular_enter = key == egui::Key::Enter && !modifiers.shift && !modifiers.alt;
                if is_regular_enter && !tab.image_preview.attachments.is_empty() {
                    let inject_text = tab.image_preview.format_injection_text();
                    if let Some(pty) = &mut tab.pty {
                        if !out.is_empty() {
                            let _ = pty.write(&out);
                            out.clear();
                        }
                        pty.write_then_submit_delayed(inject_text.as_bytes(), 100);
                    }
                    tab.image_preview.clear();
                    continue;
                }

                // 2) 取消当前命令行：Ctrl+C / Ctrl+U 清空暂存区
                if is_ctrl_or_cmd && (key == egui::Key::U || key == egui::Key::C) {
                    tab.image_preview.clear();
                }

                if let Some(bytes) = map_key(key, &modifiers) {
                    out.extend_from_slice(&bytes);
                }
            }
            _ => {}
        }
    }
    if !out.is_empty() {
        if let Some(pty) = &mut tab.pty {
            if let Err(e) = pty.write(&out) {
                return Some(format!("写入 PTY 失败: {e}"));
            }
        }
    }
    None
}

fn map_key(key: egui::Key, mods: &Modifiers) -> Option<Vec<u8>> {
    use egui::Key::*;
    match key {
        Enter => Some(b"\r".to_vec()),
        Backspace => Some(vec![0x7f]),
        Escape => Some(vec![0x1b]),
        Tab => Some(if mods.shift { b"\x1b[Z".to_vec() } else { b"\t".to_vec() }),
        ArrowUp => Some(b"\x1b[A".to_vec()),
        ArrowDown => Some(b"\x1b[B".to_vec()),
        ArrowRight => Some(b"\x1b[C".to_vec()),
        ArrowLeft => Some(b"\x1b[D".to_vec()),
        Home => Some(b"\x1b[H".to_vec()),
        End => Some(b"\x1b[F".to_vec()),
        PageUp => Some(b"\x1b[5~".to_vec()),
        PageDown => Some(b"\x1b[6~".to_vec()),
        Delete => Some(b"\x1b[3~".to_vec()),
        Insert => Some(b"\x1b[2~".to_vec()),
        _ => {
            if mods.ctrl && !mods.alt {
                if let Some(c) = key_to_char(key) {
                    return Some(vec![(c as u8) & 0x1f]);
                }
            }
            None
        }
    }
}

fn key_to_char(key: egui::Key) -> Option<char> {
    use egui::Key::*;
    let k = key as usize;
    if (A as usize..=Z as usize).contains(&k) {
        return Some(char::from(b'a' + (k - A as usize) as u8));
    }
    if (Num0 as usize..=Num9 as usize).contains(&k) {
        return Some(char::from(b'0' + (k - Num0 as usize) as u8));
    }
    match key {
        Space => Some(' '),
        _ => None,
    }
}

/// 处理鼠标滚轮与平滑触控板滚动
pub fn handle_scroll(
    ui: &mut Ui,
    tab: &mut TerminalInstance,
    hovered: bool,
    rect: Rect,
    col_w: f32,
    row_h: f32,
) {
    let pointer_in = ui.input(|i| {
        i.pointer.latest_pos().map_or(false, |p| rect.contains(p))
    });
    if !hovered && !pointer_in {
        return;
    }

    let scroll_y = ui.input(|i| i.smooth_scroll_delta.y);
    if scroll_y != 0.0 {
        tab.scroll_accum += scroll_y;
    }

    let pixels_per_line = 15.0;
    let lines = (tab.scroll_accum / pixels_per_line).trunc() as i32;

    if lines != 0 {
        tab.scroll_accum -= (lines as f32) * pixels_per_line;
        if let Some(t) = &mut tab.terminal {
            use alacritty_terminal::term::TermMode;
            let mode = t.term.mode();
            let has_mouse_report = mode.intersects(
                TermMode::MOUSE_REPORT_CLICK
                    | TermMode::MOUSE_DRAG
                    | TermMode::MOUSE_MOTION
                    | TermMode::MOUSE_MODE,
            );
            let in_alt_screen = mode.contains(TermMode::ALT_SCREEN);

            if has_mouse_report {
                let pointer_pos = ui.input(|i| i.pointer.latest_pos()).unwrap_or(rect.min);
                let rel_x = (pointer_pos.x - rect.min.x).max(0.0);
                let rel_y = (pointer_pos.y - rect.min.y).max(0.0);
                let col = ((rel_x / col_w) as usize + 1).min(t.cols as usize);
                let row = ((rel_y / row_h) as usize + 1).min(t.rows as usize);

                let count = lines.abs();
                let is_up = lines > 0;
                let mut bytes = Vec::new();

                for _ in 0..count {
                    if mode.contains(TermMode::SGR_MOUSE) {
                        let btn = if is_up { 64 } else { 65 };
                        bytes.extend_from_slice(format!("\x1b[<{btn};{col};{row}M").as_bytes());
                    } else if mode.contains(TermMode::UTF8_MOUSE) {
                        let btn = if is_up { 64 } else { 65 };
                        let mut buf = Vec::new();
                        buf.extend_from_slice(b"\x1b[M");
                        let b = 32 + btn;
                        let c = 32 + col.min(2015);
                        let r = 32 + row.min(2015);
                        let mut char_buf = [0u8; 4];
                        buf.extend_from_slice(char::from_u32(b as u32).unwrap_or(' ').encode_utf8(&mut char_buf).as_bytes());
                        buf.extend_from_slice(char::from_u32(c as u32).unwrap_or(' ').encode_utf8(&mut char_buf).as_bytes());
                        buf.extend_from_slice(char::from_u32(r as u32).unwrap_or(' ').encode_utf8(&mut char_buf).as_bytes());
                        bytes.extend_from_slice(&buf);
                    } else {
                        let btn = if is_up { 64 } else { 65 };
                        let b = (32 + btn).min(255) as u8;
                        let c = (32 + col.min(223)) as u8;
                        let r = (32 + row.min(223)) as u8;
                        bytes.extend_from_slice(&[0x1b, b'[', b'M', b, c, r]);
                    }
                }

                if !bytes.is_empty() {
                    if let Some(pty) = &mut tab.pty {
                        let _ = pty.write(&bytes);
                    }
                }
            } else if in_alt_screen {
                let is_app_cursor = mode.contains(TermMode::APP_CURSOR);
                let count = lines.abs();
                let is_up = lines > 0;
                let mut bytes = Vec::new();

                for _ in 0..count {
                    if is_up {
                        if is_app_cursor {
                            bytes.extend_from_slice(b"\x1bOA");
                        } else {
                            bytes.extend_from_slice(b"\x1b[A");
                        }
                    } else {
                        if is_app_cursor {
                            bytes.extend_from_slice(b"\x1bOB");
                        } else {
                            bytes.extend_from_slice(b"\x1b[B");
                        }
                    }
                }

                if !bytes.is_empty() {
                    if let Some(pty) = &mut tab.pty {
                        let _ = pty.write(&bytes);
                    }
                }
            } else {
                t.scroll_display(lines);
            }
        }
    }
}
