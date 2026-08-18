//! 全局多会话全景看板 (Grid Overview Mode)
//!
//! 一屏统览所有正在运行的 AI CLI 会话状态与全彩完整微缩终端画面，
//! 支持自适应等比网格、全彩 ANSI 渲染、自绘 TUI 完美还原与右键直达特定 Tab。

use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::term::cell::{Cell, Flags};
use egui::{vec2, Align2, Color32, CornerRadius, FontId, Pos2, Rect, Sense, Stroke, Ui};

use crate::config::ThemeSettings;
use crate::state::{Session, SessionStatus};
use crate::ui::terminal::{luminance, resolve_cell, TermTheme};

/// 看板中触发的用户动作。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverviewAction {
    /// 聚焦并切换进入某个会话的特定标签页
    SelectSessionTab {
        session_idx: usize,
        tab_idx: usize,
    },
    /// 为某个会话新建标签页并进入
    NewTab(usize),
}

/// 渲染全局多会话全景看板。
pub fn show(
    ui: &mut Ui,
    sessions: &[Session],
    theme: &ThemeSettings,
    term_theme: &TermTheme,
) -> Option<OverviewAction> {
    let mut action = None;
    let dark = theme.dark;

    let custom_color = theme.sidebar_card_color.unwrap_or([0, 111, 238]);

    let bg_card = if dark {
        Color32::from_rgb(24, 27, 33)
    } else {
        Color32::from_rgb(255, 255, 255)
    };
    let border_normal = if dark {
        Color32::from_rgb(46, 52, 64)
    } else {
        Color32::from_rgb(222, 228, 238)
    };
    let border_hover = Color32::from_rgb(custom_color[0], custom_color[1], custom_color[2]);

    let text_main = if dark {
        Color32::from_rgb(235, 240, 250)
    } else {
        Color32::from_rgb(25, 30, 40)
    };
    let text_sub = if dark {
        Color32::from_rgb(140, 150, 165)
    } else {
        Color32::from_rgb(110, 120, 135)
    };

    ui.add_space(14.0);

    // ---- 顶部概览 Header ----
    ui.horizontal(|ui| {
        ui.add_space(20.0);
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("⊞ 全景多会话看板 (Overview)")
                        .font(FontId::proportional(17.5))
                        .color(text_main)
                        .strong(),
                );
            });
            ui.add_space(2.0);

            let running_count = sessions.iter().filter(|s| s.status() == SessionStatus::Running).count();
            let total_tabs: usize = sessions.iter().map(|s| s.tabs.len()).sum();
            ui.label(
                egui::RichText::new(format!(
                    "共 {} 个会话 · {} 个正在运行 · 活跃标签 {} 个 (点击卡片进入，右键卡片直接选择 Tab)",
                    sessions.len(),
                    running_count,
                    total_tabs
                ))
                .font(FontId::proportional(12.0))
                .color(text_sub),
            );
        });
    });

    ui.add_space(10.0);
    ui.separator();
    ui.add_space(10.0);

    // ---- 多宫格自适应卡片区域（宽屏 2 列或 3 列，等比展示完整终端） ----
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let available_width = ui.available_width() - 40.0;
            let min_card_w = 440.0;
            let cols_count = ((available_width / min_card_w).floor() as usize).clamp(1, 3);
            let card_spacing = 16.0;
            let card_width = ((available_width - (cols_count - 1) as f32 * card_spacing) / cols_count as f32).max(360.0);
            // 终端按 ~16:10 比例舒展呈现
            let card_height = (card_width * 0.58).clamp(260.0, 340.0);

            let total_sessions = sessions.len();
            let rows_count = (total_sessions + cols_count - 1) / cols_count;

            for r in 0..rows_count {
                ui.horizontal(|ui| {
                    ui.add_space(20.0);
                    for c in 0..cols_count {
                        let idx = r * cols_count + c;
                        if idx < total_sessions {
                            let s = &sessions[idx];

                            let (card_rect, resp) = ui.allocate_exact_size(
                                vec2(card_width, card_height),
                                Sense::click(),
                            );

                            let is_hovered = resp.hovered();

                            // 绘制卡片背景与阴影
                            let painter = ui.painter().with_clip_rect(card_rect);
                            let shadow_color = if is_hovered {
                                Color32::from_black_alpha(if dark { 140 } else { 40 })
                            } else {
                                Color32::from_black_alpha(if dark { 70 } else { 18 })
                            };
                            let shadow_offset = if is_hovered { 6.0 } else { 2.5 };
                            painter.rect_filled(
                                card_rect.translate(vec2(0.0, shadow_offset)),
                                CornerRadius::same(12),
                                shadow_color,
                            );

                            painter.rect_filled(card_rect, CornerRadius::same(12), bg_card);

                            let border_color = if is_hovered { border_hover } else { border_normal };
                            let border_w = if is_hovered { 1.5 } else { 1.0 };
                            painter.rect_stroke(
                                card_rect,
                                CornerRadius::same(12),
                                Stroke::new(border_w, border_color),
                                egui::StrokeKind::Inside,
                            );

                            // ---- 卡片 Header ----
                            let header_h = 38.0;
                            let header_rect = Rect::from_min_size(card_rect.min, vec2(card_width, header_h));

                            // 状态圆点
                            let status = s.status();
                            let (dot_color, dot_tooltip) = match status {
                                SessionStatus::Running => (Color32::from_rgb(46, 204, 113), "运行中 (Running)"),
                                SessionStatus::Idle => (Color32::from_rgb(149, 165, 166), "空闲 (Idle)"),
                                SessionStatus::Exited => (Color32::from_rgb(241, 196, 15), "已退出 (Exited)"),
                                SessionStatus::Failed => (Color32::from_rgb(231, 76, 60), "启动失败 (Failed)"),
                            };

                            let dot_pos = Pos2::new(header_rect.min.x + 16.0, header_rect.min.y + 19.0);
                            painter.circle_filled(dot_pos, 4.5, dot_color);

                            // 会话名称
                            painter.text(
                                Pos2::new(header_rect.min.x + 28.0, header_rect.min.y + 10.5),
                                Align2::LEFT_TOP,
                                &s.name,
                                FontId::proportional(14.0),
                                text_main,
                            );

                            // 右上角 Tab 数量标徽
                            let tab_badge = format!("{} tab{}", s.tabs.len(), if s.tabs.len() > 1 { "s" } else { "" });
                            painter.text(
                                Pos2::new(header_rect.max.x - 14.0, header_rect.min.y + 11.5),
                                Align2::RIGHT_TOP,
                                tab_badge,
                                FontId::proportional(11.0),
                                text_sub,
                            );

                            // ---- 卡片 Body (全彩等比微缩终端画面) ----
                            let preview_margin = 10.0;
                            let preview_rect = Rect::from_min_max(
                                Pos2::new(card_rect.min.x + preview_margin, card_rect.min.y + header_h),
                                Pos2::new(card_rect.max.x - preview_margin, card_rect.max.y - 10.0),
                            );

                            painter.rect_filled(preview_rect, CornerRadius::same(6), term_theme.background);
                            painter.rect_stroke(
                                preview_rect,
                                CornerRadius::same(6),
                                Stroke::new(0.5, border_normal),
                                egui::StrokeKind::Inside,
                            );

                            // 渲染真彩微缩终端网格
                            let active_tab = s.tabs.get(s.active_tab);
                            if let Some(tab) = active_tab {
                                if let Some(t) = &tab.terminal {
                                    render_mini_terminal(ui, t, term_theme, preview_rect);
                                } else {
                                    painter.text(
                                        preview_rect.center(),
                                        Align2::CENTER_CENTER,
                                        "[终端未初始化 · 点击启动]",
                                        FontId::proportional(12.0),
                                        text_sub,
                                    );
                                }
                            } else {
                                painter.text(
                                    preview_rect.center(),
                                    Align2::CENTER_CENTER,
                                    "[未启动实例 · 点击进入]",
                                    FontId::proportional(12.0),
                                    text_sub,
                                );
                            }

                            let resp = resp.on_hover_text(format!("{} · 左键进入，右键选择 Tab", dot_tooltip));

                            // 左键点击直接进入当前活跃 Tab
                            if resp.clicked() {
                                action = Some(OverviewAction::SelectSessionTab {
                                    session_idx: idx,
                                    tab_idx: s.active_tab,
                                });
                            }

                            // 右键菜单：列出所有标签页，支持直接点击直达对应 Tab
                            resp.context_menu(|ui| {
                                ui.set_min_width(160.0);
                                ui.label(
                                    egui::RichText::new(format!("{} · 全部标签页", s.name))
                                        .font(FontId::proportional(12.5))
                                        .strong(),
                                );
                                ui.separator();

                                if s.tabs.is_empty() {
                                    if ui.button("▶ 启动新会话").clicked() {
                                        action = Some(OverviewAction::SelectSessionTab {
                                            session_idx: idx,
                                            tab_idx: 0,
                                        });
                                        ui.close();
                                    }
                                } else {
                                    for (ti, tab) in s.tabs.iter().enumerate() {
                                        let is_current = ti == s.active_tab;
                                        let is_alive = tab.alive.load(std::sync::atomic::Ordering::SeqCst);
                                        let dot = if is_alive { "🟢" } else { "⚪" };
                                        let btn_text = format!(
                                            "{} Tab {} {}",
                                            dot,
                                            ti + 1,
                                            if is_current { "(当前)" } else { "" }
                                        );

                                        if ui.button(btn_text).clicked() {
                                            action = Some(OverviewAction::SelectSessionTab {
                                                session_idx: idx,
                                                tab_idx: ti,
                                            });
                                            ui.close();
                                        }
                                    }

                                    ui.separator();
                                    if ui.button("＋ 新建标签页 (New Tab)").clicked() {
                                        action = Some(OverviewAction::NewTab(idx));
                                        ui.close();
                                    }
                                }
                            });
                        }
                        ui.add_space(card_spacing);
                    }
                });
                ui.add_space(card_spacing);
            }
            ui.add_space(20.0);
        });

    action
}

/// 真彩微缩终端网格渲染器：
/// 等比缩小并完整渲染终端视口内的全部行、列、字符与真实 ANSI 色彩。
fn render_mini_terminal(
    ui: &Ui,
    terminal: &crate::backend::terminal::Terminal,
    term_theme: &TermTheme,
    rect: Rect,
) {
    let term = &terminal.term;
    let grid = term.grid();
    let colors = term.colors();

    let screen_lines = grid.screen_lines();
    let cols = terminal.dimensions().0 as usize;

    if screen_lines == 0 || cols == 0 {
        return;
    }

    let mini_col_w = rect.width() / cols as f32;
    let mini_row_h = rect.height() / screen_lines as f32;
    let font_size = (mini_row_h * 0.95).clamp(4.0, 11.0);
    let font_mono = FontId::new(font_size, term_theme.font_family.clone());

    let painter = ui.painter().with_clip_rect(rect);
    let origin = rect.min;

    // 收集屏幕可视区每行的单元格
    let mut lines: Vec<Vec<&Cell>> = Vec::with_capacity(screen_lines);
    let mut current_line = None;
    for item in grid.display_iter() {
        if Some(item.point.line) != current_line {
            lines.push(Vec::with_capacity(cols));
            current_line = Some(item.point.line);
        }
        if let Some(last_row) = lines.last_mut() {
            last_row.push(item.cell);
        }
    }

    for (li, cells) in lines.iter().enumerate() {
        if li >= screen_lines {
            break;
        }
        let y = origin.y + li as f32 * mini_row_h;

        // 1) 背景 run (合并同色背景块)
        let mut runs: Vec<(usize, usize, Color32)> = Vec::new();
        for (ci, cell) in cells.iter().enumerate() {
            if ci >= cols {
                break;
            }
            let flags = cell.flags;
            if flags.contains(Flags::WIDE_CHAR_SPACER) {
                continue;
            }
            let width = if flags.contains(Flags::WIDE_CHAR) { 2 } else { 1 };
            let (_fg, bg) = resolve_cell(cell, colors, term_theme);
            if bg != term_theme.background {
                let mut merged = false;
                if let Some((_, e, c)) = runs.last_mut() {
                    if *c == bg && *e == ci {
                        *e = ci + width;
                        merged = true;
                    }
                }
                if !merged {
                    runs.push((ci, ci + width, bg));
                }
            }
        }

        // 浅色主题容器调和
        let mut container_runs: Vec<(usize, usize)> = Vec::new();
        if !term_theme.is_dark() {
            for run in &mut runs {
                let span_len = run.1 - run.0;
                if span_len >= 20 && luminance(run.2) < 80.0 {
                    run.2 = Color32::from_rgb(234, 236, 240);
                    container_runs.push((run.0, run.1));
                }
            }
        }

        // 绘制背景色块
        for (s, e, c) in &runs {
            let x0 = origin.x + *s as f32 * mini_col_w;
            let x1 = origin.x + *e as f32 * mini_col_w;
            painter.rect_filled(
                Rect::from_min_max(Pos2::new(x0, y), Pos2::new(x1, y + mini_row_h)),
                0.0,
                *c,
            );
        }

        // 2) 文本 span 收集与全彩绘制
        let mut spans: Vec<(usize, String, Color32)> = Vec::new();
        let mut cur_start: Option<usize> = None;
        let mut cur_str = String::new();
        let mut cur_color = Color32::TRANSPARENT;
        let mut last_ci: Option<usize> = None;

        for (ci, cell) in cells.iter().enumerate() {
            if ci >= cols {
                break;
            }
            let flags = cell.flags;
            if flags.contains(Flags::WIDE_CHAR_SPACER) {
                continue;
            }
            let (mut color, _) = resolve_cell(cell, colors, term_theme);

            // 浅色主题文字对比度保护
            if container_runs.iter().any(|(s, e)| ci >= *s && ci < *e) {
                if luminance(color) > 100.0 {
                    color = Color32::from_rgb(28, 30, 36);
                }
            }

            let ch = if flags.contains(Flags::HIDDEN) { ' ' } else { cell.c };
            if ch == ' ' {
                if let Some(sc) = cur_start.take() {
                    spans.push((sc, std::mem::take(&mut cur_str), cur_color));
                }
                cur_color = Color32::TRANSPARENT;
                last_ci = Some(ci);
                continue;
            }

            let is_contiguous = last_ci.map_or(true, |l| ci == l + 1);
            if cur_start.is_none() || cur_color != color || !is_contiguous {
                if let Some(sc) = cur_start.take() {
                    spans.push((sc, std::mem::take(&mut cur_str), cur_color));
                }
                cur_start = Some(ci);
                cur_color = color;
            }
            cur_str.push(ch);
            last_ci = Some(ci);
        }
        if let Some(sc) = cur_start.take() {
            spans.push((sc, cur_str, cur_color));
        }

        // 绘制全彩文字
        for (start, text, color) in &spans {
            let pos = Pos2::new(origin.x + *start as f32 * mini_col_w, y);
            painter.text(pos, Align2::LEFT_TOP, text, font_mono.clone(), *color);
        }
    }
}
