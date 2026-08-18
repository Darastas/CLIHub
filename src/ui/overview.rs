//! 全局多会话全景看板 (Grid Overview Mode)
//!
//! 一屏统览所有正在运行的 AI CLI 会话状态与终端实时画面，
//! 支持自适应多宫格布局、状态指示灯、微缩终端画面预览与一键聚焦跳转。

use alacritty_terminal::grid::Dimensions;
use egui::{vec2, Align2, Color32, CornerRadius, FontId, Pos2, Rect, Sense, Stroke, Ui};

use crate::config::ThemeSettings;
use crate::state::{Session, SessionStatus};
use crate::ui::terminal::TermTheme;

/// 看板中触发的用户动作。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverviewAction {
    /// 聚焦并切换进入某个会话
    SelectSession(usize),
    /// 打开新增会话弹窗
    NewSession,
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

    let bg_card = if dark {
        Color32::from_rgb(26, 29, 36)
    } else {
        Color32::from_rgb(255, 255, 255)
    };
    let border_normal = if dark {
        Color32::from_rgb(48, 54, 66)
    } else {
        Color32::from_rgb(222, 228, 238)
    };
    let border_hover = Color32::from_rgb(0, 111, 238);
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
    let preview_bg = if dark {
        Color32::from_rgb(16, 18, 22)
    } else {
        Color32::from_rgb(244, 246, 250)
    };

    ui.add_space(16.0);

    // ---- 顶部概览 Header ----
    ui.horizontal(|ui| {
        ui.add_space(20.0);
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("⊞ 全景多会话看板 (Overview)")
                        .font(FontId::proportional(18.0))
                        .color(text_main)
                        .strong(),
                );
            });
            ui.add_space(2.0);

            let running_count = sessions.iter().filter(|s| s.status() == SessionStatus::Running).count();
            let total_tabs: usize = sessions.iter().map(|s| s.tabs.len()).sum();
            ui.label(
                egui::RichText::new(format!(
                    "共 {} 个会话 · {} 个正在运行 · 活跃标签 {} 个 (按 Ctrl+Shift+O 或点击卡片进入工作台)",
                    sessions.len(),
                    running_count,
                    total_tabs
                ))
                .font(FontId::proportional(12.0))
                .color(text_sub),
            );
        });

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(20.0);
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new("＋ 新建会话")
                            .font(FontId::proportional(12.5))
                            .color(Color32::WHITE),
                    )
                    .fill(Color32::from_rgb(0, 111, 238))
                    .corner_radius(CornerRadius::same(6))
                    .min_size(vec2(90.0, 30.0)),
                )
                .clicked()
            {
                action = Some(OverviewAction::NewSession);
            }
        });
    });

    ui.add_space(12.0);
    ui.separator();
    ui.add_space(12.0);

    // ---- 多宫格自适应卡片区域 ----
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.add_space(4.0);
            let available_width = ui.available_width() - 40.0;
            let min_card_w = 340.0;
            let cols_count = ((available_width / min_card_w).floor() as usize).clamp(1, 4);
            let card_spacing = 16.0;
            let card_width = ((available_width - (cols_count - 1) as f32 * card_spacing) / cols_count as f32).max(280.0);
            let card_height = 230.0;

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
                                Color32::from_black_alpha(if dark { 120 } else { 35 })
                            } else {
                                Color32::from_black_alpha(if dark { 60 } else { 15 })
                            };
                            let shadow_offset = if is_hovered { 6.0 } else { 2.0 };
                            painter.rect_filled(
                                card_rect.translate(vec2(0.0, shadow_offset)),
                                CornerRadius::same(10),
                                shadow_color,
                            );

                            painter.rect_filled(card_rect, CornerRadius::same(10), bg_card);

                            let border_color = if is_hovered { border_hover } else { border_normal };
                            let border_w = if is_hovered { 1.5 } else { 1.0 };
                            painter.rect_stroke(
                                card_rect,
                                CornerRadius::same(10),
                                Stroke::new(border_w, border_color),
                                egui::StrokeKind::Inside,
                            );

                            // ---- 卡片 Header ----
                            let header_h = 42.0;
                            let header_rect = Rect::from_min_size(card_rect.min, vec2(card_width, header_h));

                            // 状态圆点
                            let status = s.status();
                            let (dot_color, dot_tooltip) = match status {
                                SessionStatus::Running => (Color32::from_rgb(46, 204, 113), "运行中 (Running)"),
                                SessionStatus::Idle => (Color32::from_rgb(149, 165, 166), "空闲 (Idle)"),
                                SessionStatus::Exited => (Color32::from_rgb(241, 196, 15), "已退出 (Exited)"),
                                SessionStatus::Failed => (Color32::from_rgb(231, 76, 60), "启动失败 (Failed)"),
                            };

                            let dot_pos = Pos2::new(header_rect.min.x + 16.0, header_rect.min.y + 21.0);
                            painter.circle_filled(dot_pos, 4.5, dot_color);

                            // 会话名称
                            painter.text(
                                Pos2::new(header_rect.min.x + 28.0, header_rect.min.y + 12.0),
                                Align2::LEFT_TOP,
                                &s.name,
                                FontId::proportional(14.5),
                                text_main,
                            );

                            // 右上角 Tab 数量标徽
                            let tab_badge = format!("{} tab{}", s.tabs.len(), if s.tabs.len() > 1 { "s" } else { "" });
                            painter.text(
                                Pos2::new(header_rect.max.x - 14.0, header_rect.min.y + 13.0),
                                Align2::RIGHT_TOP,
                                tab_badge,
                                FontId::proportional(11.0),
                                text_sub,
                            );

                            // ---- 卡片 Body (微缩终端画面) ----
                            let preview_margin = 12.0;
                            let preview_rect = Rect::from_min_max(
                                Pos2::new(card_rect.min.x + preview_margin, card_rect.min.y + header_h),
                                Pos2::new(card_rect.max.x - preview_margin, card_rect.max.y - 12.0),
                            );

                            painter.rect_filled(preview_rect, CornerRadius::same(6), preview_bg);
                            painter.rect_stroke(
                                preview_rect,
                                CornerRadius::same(6),
                                Stroke::new(0.5, border_normal),
                                egui::StrokeKind::Inside,
                            );

                            // 提取最新终端文本并渲染微缩画面
                            let active_tab = s.tabs.get(s.active_tab);
                            let preview_lines = if let Some(tab) = active_tab {
                                if let Some(t) = &tab.terminal {
                                    extract_recent_terminal_lines(t, 8)
                                } else {
                                    vec!["[终端未初始化]".to_string()]
                                }
                            } else {
                                vec!["[未启动实例 · 点击进入]".to_string()]
                            };

                            let preview_painter = ui.painter().with_clip_rect(preview_rect.shrink(6.0));
                            let font_mono = FontId::monospace(10.5);
                            let line_h = 15.0;

                            for (li, ltext) in preview_lines.iter().enumerate() {
                                let y = preview_rect.min.y + 8.0 + li as f32 * line_h;
                                if y + line_h > preview_rect.max.y - 4.0 {
                                    break;
                                }
                                preview_painter.text(
                                    Pos2::new(preview_rect.min.x + 8.0, y),
                                    Align2::LEFT_TOP,
                                    ltext,
                                    font_mono.clone(),
                                    if dark { Color32::from_rgb(185, 195, 210) } else { Color32::from_rgb(60, 70, 85) },
                                );
                            }

                            // 悬停时提示进入
                            if resp.on_hover_text(format!("{} · 点击快速切换进入工作台", dot_tooltip)).clicked() {
                                action = Some(OverviewAction::SelectSession(idx));
                            }
                        }
                        ui.add_space(card_spacing);
                    }
                });
                ui.add_space(card_spacing);
            }
            ui.add_space(20.0);
        });

    let _ = term_theme;
    action
}

/// 提取终端最近 N 行的纯文本（过滤空行与右侧空白）。
fn extract_recent_terminal_lines(terminal: &crate::backend::terminal::Terminal, max_lines: usize) -> Vec<String> {
    use alacritty_terminal::index::{Column, Line};

    let grid = terminal.term.grid();
    let screen = grid.screen_lines() as i32;
    let cols = terminal.dimensions().0 as usize;

    let mut result = Vec::new();

    // 从屏幕底部向上读取
    for row_idx in (0..screen).rev() {
        let row = &grid[Line(row_idx)];
        let mut line_str = String::new();
        let mut trailing_spaces = 0;

        for c in 0..cols.min(row.len()) {
            let cell = &row[Column(c)];
            if cell.flags.contains(alacritty_terminal::term::cell::Flags::WIDE_CHAR_SPACER) {
                continue;
            }
            if cell.c == ' ' {
                trailing_spaces += 1;
            } else {
                for _ in 0..trailing_spaces {
                    line_str.push(' ');
                }
                trailing_spaces = 0;
                line_str.push(cell.c);
            }
        }

        if !line_str.is_empty() || !result.is_empty() {
            result.push(line_str);
            if result.len() >= max_lines {
                break;
            }
        }
    }

    result.reverse();
    if result.is_empty() {
        result.push("[无输出 / 等待输入]".to_string());
    }
    result
}
