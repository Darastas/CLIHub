//! 全局多会话全景看板 (Grid Overview Mode) 与 单个 CLI 标签全景看板 (Single-CLI Tab Panorama)
//!
//! 一屏统览所有正在运行的 AI CLI 会话状态与全彩完整微缩终端画面，
//! 一行两列等比精确缩放，完美还原主界面终端比例与自绘 TUI 界面。
//! 支持在右键菜单中一键进入「浏览全部窗口」，全景掌控指定 CLI 的所有标签页。

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
    /// 进入某个会话的全部标签窗口浏览视图
    BrowseSessionTabs(usize),
    /// 从单个会话的标签全景视图返回全局看板
    BackToGlobalOverview,
    /// 关闭某个会话的指定标签页
    CloseTab {
        session_idx: usize,
        tab_idx: usize,
    },
}

fn lerp_c(a: Color32, b: Color32, t: f32) -> Color32 {
    Color32::from_rgba_premultiplied(
        (a.r() as f32 * (1.0 - t) + b.r() as f32 * t).clamp(0.0, 255.0) as u8,
        (a.g() as f32 * (1.0 - t) + b.g() as f32 * t).clamp(0.0, 255.0) as u8,
        (a.b() as f32 * (1.0 - t) + b.b() as f32 * t).clamp(0.0, 255.0) as u8,
        (a.a() as f32 * (1.0 - t) + b.a() as f32 * t).clamp(0.0, 255.0) as u8,
    )
}

/// 渲染全景看板：根据 overview_session 决定渲染全局看板还是单个会话的所有标签看板。
pub fn show(
    ui: &mut Ui,
    sessions: &[Session],
    overview_session: Option<usize>,
    theme: &ThemeSettings,
    term_theme: &TermTheme,
) -> Option<OverviewAction> {
    if let Some(session_idx) = overview_session {
        if session_idx < sessions.len() {
            return show_session_tabs(ui, sessions, session_idx, theme, term_theme);
        }
    }
    show_global_overview(ui, sessions, theme, term_theme)
}

/// 渲染全局多会话全景看板。
fn show_global_overview(
    ui: &mut Ui,
    sessions: &[Session],
    theme: &ThemeSettings,
    term_theme: &TermTheme,
) -> Option<OverviewAction> {
    let mut action = None;
    let dark = theme.dark;

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
    // 鼠标悬停在卡片上时使用高亮白色，与聚焦窗口对应
    let border_hover = if dark {
        Color32::WHITE
    } else {
        Color32::from_rgb(30, 35, 45)
    };

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

    // ---- 顶部概览 Header（与侧边栏、标签栏 100% 绝对水平基准线对齐）----
    ui.add_space(13.0);

    let (header_rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 34.0), Sense::hover());
    let row_center_y = header_rect.center().y;
    let p = ui.painter();

    // 1. 左侧 2x2 视窗矢量网格图标 + 标题
    let icon_c = Pos2::new(header_rect.min.x + 20.0 + 8.0, row_center_y);
    let cell_size = 4.4;
    let gap = 1.6;
    let offset = (cell_size + gap) / 2.0;
    let cell_radius = 1.0;
    let cell_stroke = egui::Stroke::new(1.15, text_main);

    for dy in [-offset, offset] {
        for dx in [-offset, offset] {
            let cell_rect = Rect::from_center_size(icon_c + vec2(dx, dy), vec2(cell_size, cell_size));
            p.rect_stroke(cell_rect, cell_radius, cell_stroke, egui::StrokeKind::Inside);
        }
    }

    // 标题文本
    p.text(
        Pos2::new(icon_c.x + 14.0, row_center_y),
        Align2::LEFT_CENTER,
        "全景多会话看板",
        FontId::new(15.0, egui::FontFamily::Proportional),
        text_main,
    );

    // 统计微徽章胶囊 (Sleek Monospace / Frosted Stats Pill)
    let running_count = sessions.iter().filter(|s| s.status() == SessionStatus::Running).count();
    let total_tabs: usize = sessions.iter().map(|s| s.tabs.len()).sum();
    let stats_text = format!("{} 会话 · {} 运行中 · {} 标签", sessions.len(), running_count, total_tabs);
    let stats_font = FontId::new(11.5, egui::FontFamily::Proportional);
    let stats_w = p.layout_no_wrap(stats_text.clone(), stats_font.clone(), Color32::WHITE).rect.width();

    let stats_badge_w = stats_w + 16.0;
    let stats_badge_rect = Rect::from_min_size(
        Pos2::new(icon_c.x + 14.0 + 118.0, row_center_y - 11.0),
        vec2(stats_badge_w, 22.0),
    );
    let badge_bg = if dark { Color32::from_white_alpha(7) } else { Color32::from_black_alpha(10) };
    let badge_stroke = if dark { Color32::from_white_alpha(12) } else { Color32::from_black_alpha(14) };
    p.rect_filled(stats_badge_rect, 5.0, badge_bg);
    p.rect_stroke(stats_badge_rect, 5.0, egui::Stroke::new(0.5, badge_stroke), egui::StrokeKind::Inside);
    p.text(stats_badge_rect.center(), Align2::CENTER_CENTER, stats_text, stats_font, text_sub);

    // 2. 右侧 ✕ 退出看板按钮
    let close_rect = Rect::from_center_size(Pos2::new(header_rect.max.x - 20.0 - 14.0, row_center_y), vec2(28.0, 28.0));
    let close_resp = ui.interact(close_rect, egui::Id::new("overview_close_btn"), Sense::click());
    let close_hf = ui.ctx().animate_bool(egui::Id::new("overview_close_h"), close_resp.hovered());

    let btn_base = if dark { Color32::from_white_alpha(5) } else { Color32::from_black_alpha(8) };
    let btn_hover = if dark { Color32::from_white_alpha(14) } else { Color32::from_black_alpha(16) };
    let btn_base_stroke = if dark { Color32::from_white_alpha(8) } else { Color32::from_black_alpha(10) };
    let btn_hover_stroke = if dark { Color32::from_white_alpha(18) } else { Color32::from_black_alpha(18) };
    let btn_bg = lerp_c(btn_base, btn_hover, close_hf);
    let btn_stroke = lerp_c(btn_base_stroke, btn_hover_stroke, close_hf);

    let p_c = ui.painter();
    p_c.rect_filled(close_rect.translate(vec2(0.0, 1.0)), 7.0, if dark { Color32::from_black_alpha(50) } else { Color32::from_black_alpha(12) });
    p_c.rect_filled(close_rect, 7.0, btn_bg);
    p_c.rect_stroke(close_rect, 7.0, egui::Stroke::new(0.5, btn_stroke), egui::StrokeKind::Inside);

    let close_fg = lerp_c(if dark { Color32::from_gray(160) } else { Color32::from_gray(100) }, if dark { Color32::WHITE } else { Color32::BLACK }, close_hf);
    let c = close_rect.center();
    let cd = 3.6;
    p_c.line_segment([c + vec2(-cd, -cd), c + vec2(cd, cd)], egui::Stroke::new(1.35, close_fg));
    p_c.line_segment([c + vec2(-cd, cd), c + vec2(cd, -cd)], egui::Stroke::new(1.35, close_fg));

    if close_resp.on_hover_text("退出看板 (Esc)").clicked() {
        if let Some(active_session) = sessions.iter().position(|s| s.status() == SessionStatus::Running).or(Some(0)) {
            let active_tab = sessions.get(active_session).map_or(0, |s| s.active_tab);
            action = Some(OverviewAction::SelectSessionTab {
                session_idx: active_session,
                tab_idx: active_tab,
            });
        }
    }

    ui.add_space(14.0);

    // 计算主界面终端的标准长宽比（通常约为 1.45 ~ 1.65）
    let orig_font_id = FontId::new(term_theme.font_size, term_theme.font_family.clone());
    let (orig_char_w, orig_row_h) = ui.fonts_mut(|f| (f.glyph_width(&orig_font_id, ' '), f.row_height(&orig_font_id)));

    // ---- 多宫格自适应卡片区域（一行固定 2 列，等比展示完整终端） ----
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let available_width = ui.available_width() - 40.0;
            let cols_count = if available_width < 600.0 { 1 } else { 2 };
            let card_spacing = 16.0;
            let card_width = if cols_count == 1 {
                available_width
            } else {
                (available_width - card_spacing) / 2.0
            };

            // 预览区域的宽度与等比高度
            let preview_margin_x = 10.0;
            let preview_margin_y = 10.0;
            let header_h = 38.0;
            let preview_w = card_width - preview_margin_x * 2.0;

            // 默认按 80 列 × 24 行的主终端标准等比计算卡片高度
            let default_aspect = (80.0 * orig_char_w) / (24.0 * orig_row_h);
            let preview_h = (preview_w / default_aspect).clamp(200.0, 380.0);
            let card_height = header_h + preview_h + preview_margin_y * 2.0;

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
                            let hover_factor = ui.ctx().animate_bool_with_time(
                                egui::Id::new(("overview_card_h", idx)),
                                is_hovered,
                                0.15,
                            );

                            // 绘制卡片背景与阴影
                            let painter = ui.painter().with_clip_rect(card_rect);
                            let shadow_alpha = ((if dark { 70.0 } else { 18.0 }) + (if dark { 70.0 } else { 22.0 }) * hover_factor) as u8;
                            let shadow_color = Color32::from_black_alpha(shadow_alpha);
                            let shadow_offset = 2.5 + 3.5 * hover_factor;
                            painter.rect_filled(
                                card_rect.translate(vec2(0.0, shadow_offset)),
                                CornerRadius::same(12),
                                shadow_color,
                            );

                            painter.rect_filled(card_rect, CornerRadius::same(12), bg_card);

                            let border_color = lerp_c(border_normal, border_hover, hover_factor);
                            let border_w = 1.0 + 0.5 * hover_factor;
                            painter.rect_stroke(
                                card_rect,
                                CornerRadius::same(12),
                                Stroke::new(border_w, border_color),
                                egui::StrokeKind::Inside,
                            );

                            // ---- 卡片 Header（垂直精准绝对居中） ----
                            let header_rect = Rect::from_min_size(card_rect.min, vec2(card_width, header_h));
                            let card_center_y = header_rect.center().y;

                            // 状态圆点
                            let status = s.status();
                            let (dot_color, dot_tooltip) = match status {
                                SessionStatus::Running => (Color32::from_rgb(46, 204, 113), "运行中 (Running)"),
                                SessionStatus::Idle => (Color32::from_rgb(149, 165, 166), "空闲 (Idle)"),
                                SessionStatus::Exited => (Color32::from_rgb(241, 196, 15), "已退出 (Exited)"),
                                SessionStatus::Failed => (Color32::from_rgb(231, 76, 60), "启动失败 (Failed)"),
                            };

                            let dot_pos = Pos2::new(header_rect.min.x + 16.0, card_center_y);
                            painter.circle_filled(dot_pos, 4.5, dot_color);

                            // 会话名称
                            let name_fg = lerp_c(text_main, if dark { Color32::WHITE } else { Color32::BLACK }, hover_factor);
                            painter.text(
                                Pos2::new(header_rect.min.x + 28.0, card_center_y),
                                Align2::LEFT_CENTER,
                                &s.name,
                                FontId::proportional(14.0),
                                name_fg,
                            );

                            // 右上角 Tab 数量标徽
                            let tab_badge = format!("{} tab{}", s.tabs.len(), if s.tabs.len() > 1 { "s" } else { "" });
                            painter.text(
                                Pos2::new(header_rect.max.x - 14.0, card_center_y),
                                Align2::RIGHT_CENTER,
                                tab_badge,
                                FontId::proportional(11.0),
                                text_sub,
                            );

                            // ---- 卡片 Body (全彩等比微缩终端画面) ----
                            let preview_rect = Rect::from_min_max(
                                Pos2::new(card_rect.min.x + preview_margin_x, card_rect.min.y + header_h),
                                Pos2::new(card_rect.max.x - preview_margin_x, card_rect.max.y - preview_margin_y),
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
                                    render_scaled_terminal(ui, t, term_theme, preview_rect, orig_char_w, orig_row_h);
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

                            let resp = resp.on_hover_text(format!("{} · 左键进入，右键选择 Tab 或浏览全部窗口", dot_tooltip));

                            // 左键点击直接进入当前活跃 Tab
                            if resp.clicked() {
                                action = Some(OverviewAction::SelectSessionTab {
                                    session_idx: idx,
                                    tab_idx: s.active_tab,
                                });
                            }

                            // 右键菜单：列出全部窗口浏览入口与所有标签页
                            resp.context_menu(|ui| {
                                ui.set_min_width(175.0);
                                ui.add_space(2.0);

                                // 标题区
                                ui.horizontal(|ui| {
                                    ui.add_space(4.0);
                                    ui.label(
                                        egui::RichText::new(&s.name)
                                            .font(FontId::proportional(13.0))
                                            .color(text_main)
                                            .strong(),
                                    );
                                    ui.label(
                                        egui::RichText::new(format!("({} tabs)", s.tabs.len()))
                                            .font(FontId::proportional(11.0))
                                            .color(text_sub),
                                    );
                                });

                                ui.add_space(4.0);
                                ui.separator();
                                ui.add_space(2.0);

                                // 新增「浏览全部窗口」功能入口按钮
                                let (browse_rect, browse_resp) = ui.allocate_exact_size(
                                    vec2(ui.available_width().max(165.0), 28.0),
                                    Sense::click(),
                                );
                                let is_browse_hover = browse_resp.hovered();
                                let browse_bg = if is_browse_hover {
                                    if dark { Color32::from_white_alpha(14) } else { Color32::from_black_alpha(12) }
                                } else {
                                    Color32::TRANSPARENT
                                };
                                let p = ui.painter();
                                p.rect_filled(browse_rect, CornerRadius::same(6), browse_bg);

                                // 绘制微缩 2x2 视窗矢量图标
                                let ic = Pos2::new(browse_rect.min.x + 12.0, browse_rect.center().y);
                                let isz = 2.8;
                                let igap = 1.2;
                                let ioff = (isz + igap) / 2.0;
                                let ic_fg = if is_browse_hover {
                                    if dark { Color32::WHITE } else { Color32::BLACK }
                                } else {
                                    text_main
                                };
                                for dy in [-ioff, ioff] {
                                    for dx in [-ioff, ioff] {
                                        let cr = Rect::from_center_size(ic + vec2(dx, dy), vec2(isz, isz));
                                        p.rect_stroke(cr, 0.6, egui::Stroke::new(0.9, ic_fg), egui::StrokeKind::Inside);
                                    }
                                }

                                p.text(
                                    Pos2::new(browse_rect.min.x + 24.0, browse_rect.center().y),
                                    Align2::LEFT_CENTER,
                                    "浏览全部窗口",
                                    FontId::proportional(12.5),
                                    ic_fg,
                                );

                                if browse_resp.clicked() {
                                    action = Some(OverviewAction::BrowseSessionTabs(idx));
                                    ui.close();
                                }

                                ui.add_space(2.0);
                                ui.separator();
                                ui.add_space(2.0);

                                if s.tabs.is_empty() {
                                    let btn = egui::Button::new(
                                        egui::RichText::new("▶ 启动新会话")
                                            .font(FontId::proportional(12.5))
                                            .color(text_main),
                                    )
                                    .fill(Color32::TRANSPARENT)
                                    .corner_radius(CornerRadius::same(6))
                                    .min_size(vec2(160.0, 26.0));

                                    if ui.add(btn).clicked() {
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
                                        let dot_color = if is_alive {
                                            Color32::from_rgb(46, 204, 113)
                                        } else {
                                            Color32::from_gray(140)
                                        };

                                        let (btn_rect, btn_resp) = ui.allocate_exact_size(
                                            vec2(ui.available_width().max(160.0), 28.0),
                                            Sense::click(),
                                        );

                                        let is_btn_hovered = btn_resp.hovered();
                                        let item_bg = if is_current {
                                            if dark { Color32::from_white_alpha(18) } else { Color32::from_black_alpha(16) }
                                        } else if is_btn_hovered {
                                            if dark { Color32::from_white_alpha(10) } else { Color32::from_black_alpha(8) }
                                        } else {
                                            Color32::TRANSPARENT
                                        };

                                        let p = ui.painter();
                                        p.rect_filled(btn_rect, CornerRadius::same(6), item_bg);
                                        if is_current {
                                            p.rect_stroke(
                                                btn_rect,
                                                CornerRadius::same(6),
                                                Stroke::new(0.5, if dark { Color32::from_white_alpha(40) } else { Color32::from_black_alpha(30) }),
                                                egui::StrokeKind::Inside,
                                            );
                                        }

                                        // 状态指示圆点
                                        p.circle_filled(
                                            Pos2::new(btn_rect.min.x + 12.0, btn_rect.center().y),
                                            3.5,
                                            dot_color,
                                        );

                                        // 标签文字
                                        let label_fg = if is_btn_hovered || is_current {
                                            if dark { Color32::WHITE } else { Color32::BLACK }
                                        } else {
                                            text_main
                                        };
                                        p.text(
                                            Pos2::new(btn_rect.min.x + 24.0, btn_rect.center().y),
                                            Align2::LEFT_CENTER,
                                            format!("Tab {}", ti + 1),
                                            FontId::proportional(12.5),
                                            label_fg,
                                        );

                                        // 当前状态标记
                                        if is_current {
                                            p.text(
                                                Pos2::new(btn_rect.max.x - 8.0, btn_rect.center().y),
                                                Align2::RIGHT_CENTER,
                                                "当前",
                                                FontId::proportional(10.5),
                                                if dark { Color32::from_white_alpha(160) } else { Color32::from_black_alpha(160) },
                                            );
                                        }

                                        if btn_resp.clicked() {
                                            action = Some(OverviewAction::SelectSessionTab {
                                                session_idx: idx,
                                                tab_idx: ti,
                                            });
                                            ui.close();
                                        }
                                    }

                                    ui.add_space(2.0);
                                    ui.separator();
                                    ui.add_space(2.0);

                                    let (add_rect, add_resp) = ui.allocate_exact_size(
                                        vec2(ui.available_width().max(160.0), 28.0),
                                        Sense::click(),
                                    );
                                    let is_add_hover = add_resp.hovered();
                                    let add_bg = if is_add_hover {
                                        if dark { Color32::from_white_alpha(12) } else { Color32::from_black_alpha(10) }
                                    } else {
                                        Color32::TRANSPARENT
                                    };
                                    let p = ui.painter();
                                    p.rect_filled(add_rect, CornerRadius::same(6), add_bg);
                                    p.text(
                                        Pos2::new(add_rect.min.x + 10.0, add_rect.center().y),
                                        Align2::LEFT_CENTER,
                                        "＋ 新建标签页 (New Tab)",
                                        FontId::proportional(12.0),
                                        if is_add_hover {
                                            if dark { Color32::WHITE } else { Color32::BLACK }
                                        } else {
                                            text_sub
                                        },
                                    );

                                    if add_resp.clicked() {
                                        action = Some(OverviewAction::NewTab(idx));
                                        ui.close();
                                    }
                                }
                                ui.add_space(2.0);
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

/// 渲染指定 CLI 会话的全部标签窗口全景看板 (Single-CLI Tab Panorama View)。
fn show_session_tabs(
    ui: &mut Ui,
    sessions: &[Session],
    session_idx: usize,
    theme: &ThemeSettings,
    term_theme: &TermTheme,
) -> Option<OverviewAction> {
    let mut action = None;
    let dark = theme.dark;
    let s = &sessions[session_idx];

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
    let border_hover = if dark {
        Color32::WHITE
    } else {
        Color32::from_rgb(30, 35, 45)
    };

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

    // ---- 顶部概览 Header ----
    ui.add_space(13.0);

    let (header_rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 34.0), Sense::hover());
    let row_center_y = header_rect.center().y;
    let p = ui.painter();

    // 1. 左侧返回按钮 "← 返回看板"
    let back_rect = Rect::from_min_size(
        Pos2::new(header_rect.min.x + 20.0, row_center_y - 14.0),
        vec2(96.0, 28.0),
    );
    let back_resp = ui.interact(back_rect, egui::Id::new("overview_back_btn"), Sense::click());
    let back_hf = ui.ctx().animate_bool(egui::Id::new("overview_back_h"), back_resp.hovered());

    let btn_base = if dark { Color32::from_white_alpha(6) } else { Color32::from_black_alpha(8) };
    let btn_hover = if dark { Color32::from_white_alpha(15) } else { Color32::from_black_alpha(16) };
    let btn_base_stroke = if dark { Color32::from_white_alpha(8) } else { Color32::from_black_alpha(10) };
    let btn_hover_stroke = if dark { Color32::from_white_alpha(18) } else { Color32::from_black_alpha(18) };
    let back_bg = lerp_c(btn_base, btn_hover, back_hf);
    let back_stroke = lerp_c(btn_base_stroke, btn_hover_stroke, back_hf);

    p.rect_filled(back_rect.translate(vec2(0.0, 1.0)), 7.0, if dark { Color32::from_black_alpha(50) } else { Color32::from_black_alpha(12) });
    p.rect_filled(back_rect, 7.0, back_bg);
    p.rect_stroke(back_rect, 7.0, egui::Stroke::new(0.5, back_stroke), egui::StrokeKind::Inside);

    let back_fg = lerp_c(text_sub, text_main, back_hf);
    p.text(
        back_rect.center(),
        Align2::CENTER_CENTER,
        "← 返回看板",
        FontId::new(12.0, egui::FontFamily::Proportional),
        back_fg,
    );

    if back_resp.on_hover_text("返回全局多会话看板 (Esc)").clicked() {
        action = Some(OverviewAction::BackToGlobalOverview);
    }

    // 分隔竖线
    let div_x = back_rect.max.x + 12.0;
    p.line_segment(
        [Pos2::new(div_x, row_center_y - 8.0), Pos2::new(div_x, row_center_y + 8.0)],
        egui::Stroke::new(1.0, border_normal),
    );

    // 会话状态圆点与名称标题
    let status = s.status();
    let dot_color = match status {
        SessionStatus::Running => Color32::from_rgb(46, 204, 113),
        SessionStatus::Idle => Color32::from_rgb(149, 165, 166),
        SessionStatus::Exited => Color32::from_rgb(241, 196, 15),
        SessionStatus::Failed => Color32::from_rgb(231, 76, 60),
    };
    let dot_pos = Pos2::new(div_x + 16.0, row_center_y);
    p.circle_filled(dot_pos, 4.5, dot_color);

    let title_x = dot_pos.x + 12.0;
    let title_text = format!("{} · 全部标签窗口", s.name);
    p.text(
        Pos2::new(title_x, row_center_y),
        Align2::LEFT_CENTER,
        &title_text,
        FontId::new(14.5, egui::FontFamily::Proportional),
        text_main,
    );

    let title_w = p.layout_no_wrap(title_text, FontId::new(14.5, egui::FontFamily::Proportional), Color32::WHITE).rect.width();

    // 统计微徽章胶囊
    let alive_tabs = s.tabs.iter().filter(|t| t.alive.load(std::sync::atomic::Ordering::SeqCst)).count();
    let stats_text = format!("{} 标签 · {} 运行中", s.tabs.len(), alive_tabs);
    let stats_font = FontId::new(11.5, egui::FontFamily::Proportional);
    let stats_w = p.layout_no_wrap(stats_text.clone(), stats_font.clone(), Color32::WHITE).rect.width();

    let stats_badge_w = stats_w + 16.0;
    let stats_badge_rect = Rect::from_min_size(
        Pos2::new(title_x + title_w + 12.0, row_center_y - 11.0),
        vec2(stats_badge_w, 22.0),
    );
    let badge_bg = if dark { Color32::from_white_alpha(7) } else { Color32::from_black_alpha(10) };
    let badge_stroke = if dark { Color32::from_white_alpha(12) } else { Color32::from_black_alpha(14) };
    p.rect_filled(stats_badge_rect, 5.0, badge_bg);
    p.rect_stroke(stats_badge_rect, 5.0, egui::Stroke::new(0.5, badge_stroke), egui::StrokeKind::Inside);
    p.text(stats_badge_rect.center(), Align2::CENTER_CENTER, stats_text, stats_font, text_sub);

    // 2. 右侧操作区：新建标签页按钮 + ✕ 退出看板按钮
    let close_rect = Rect::from_center_size(Pos2::new(header_rect.max.x - 20.0 - 14.0, row_center_y), vec2(28.0, 28.0));
    let close_resp = ui.interact(close_rect, egui::Id::new("overview_session_close_btn"), Sense::click());
    let close_hf = ui.ctx().animate_bool(egui::Id::new("overview_session_close_h"), close_resp.hovered());

    let close_bg = lerp_c(btn_base, btn_hover, close_hf);
    let close_stroke = lerp_c(btn_base_stroke, btn_hover_stroke, close_hf);

    p.rect_filled(close_rect.translate(vec2(0.0, 1.0)), 7.0, if dark { Color32::from_black_alpha(50) } else { Color32::from_black_alpha(12) });
    p.rect_filled(close_rect, 7.0, close_bg);
    p.rect_stroke(close_rect, 7.0, egui::Stroke::new(0.5, close_stroke), egui::StrokeKind::Inside);

    let close_fg = lerp_c(if dark { Color32::from_gray(160) } else { Color32::from_gray(100) }, if dark { Color32::WHITE } else { Color32::BLACK }, close_hf);
    let c = close_rect.center();
    let cd = 3.6;
    p.line_segment([c + vec2(-cd, -cd), c + vec2(cd, cd)], egui::Stroke::new(1.35, close_fg));
    p.line_segment([c + vec2(-cd, cd), c + vec2(cd, -cd)], egui::Stroke::new(1.35, close_fg));

    if close_resp.on_hover_text("退出看板并进入终端").clicked() {
        action = Some(OverviewAction::SelectSessionTab {
            session_idx,
            tab_idx: s.active_tab,
        });
    }

    // 顶部「＋ 新建标签页」快速按键
    let new_tab_btn_rect = Rect::from_min_size(
        Pos2::new(close_rect.min.x - 8.0 - 106.0, row_center_y - 14.0),
        vec2(106.0, 28.0),
    );
    let new_tab_resp = ui.interact(new_tab_btn_rect, egui::Id::new("overview_new_tab_btn"), Sense::click());
    let new_tab_hf = ui.ctx().animate_bool(egui::Id::new("overview_new_tab_h"), new_tab_resp.hovered());

    let new_tab_bg = lerp_c(btn_base, btn_hover, new_tab_hf);
    let new_tab_stroke = lerp_c(btn_base_stroke, btn_hover_stroke, new_tab_hf);

    p.rect_filled(new_tab_btn_rect.translate(vec2(0.0, 1.0)), 7.0, if dark { Color32::from_black_alpha(50) } else { Color32::from_black_alpha(12) });
    p.rect_filled(new_tab_btn_rect, 7.0, new_tab_bg);
    p.rect_stroke(new_tab_btn_rect, 7.0, egui::Stroke::new(0.5, new_tab_stroke), egui::StrokeKind::Inside);

    let new_tab_fg = lerp_c(text_sub, text_main, new_tab_hf);
    p.text(
        new_tab_btn_rect.center(),
        Align2::CENTER_CENTER,
        "＋ 新建标签页",
        FontId::new(12.0, egui::FontFamily::Proportional),
        new_tab_fg,
    );

    if new_tab_resp.clicked() {
        action = Some(OverviewAction::NewTab(session_idx));
    }

    ui.add_space(14.0);

    // 计算主界面终端的标准长宽比
    let orig_font_id = FontId::new(term_theme.font_size, term_theme.font_family.clone());
    let (orig_char_w, orig_row_h) = ui.fonts_mut(|f| (f.glyph_width(&orig_font_id, ' '), f.row_height(&orig_font_id)));

    // ---- 标签卡片多宫格区域 ----
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let available_width = ui.available_width() - 40.0;
            let cols_count = if available_width < 600.0 { 1 } else { 2 };
            let card_spacing = 16.0;
            let card_width = if cols_count == 1 {
                available_width
            } else {
                (available_width - card_spacing) / 2.0
            };

            let preview_margin_x = 10.0;
            let preview_margin_y = 10.0;
            let header_h = 38.0;
            let preview_w = card_width - preview_margin_x * 2.0;

            let default_aspect = (80.0 * orig_char_w) / (24.0 * orig_row_h);
            let preview_h = (preview_w / default_aspect).clamp(200.0, 380.0);
            let card_height = header_h + preview_h + preview_margin_y * 2.0;

            let total_tabs = s.tabs.len();
            // 总项目数 = 已有标签页 + 1 个「新建标签页」卡片
            let total_items = total_tabs + 1;
            let rows_count = (total_items + cols_count - 1) / cols_count;

            for r in 0..rows_count {
                ui.horizontal(|ui| {
                    ui.add_space(20.0);
                    for c in 0..cols_count {
                        let item_idx = r * cols_count + c;
                        if item_idx < total_tabs {
                            // 渲染实际的 Tab 窗口卡片
                            let tab_idx = item_idx;
                            let tab = &s.tabs[tab_idx];
                            let is_current_tab = tab_idx == s.active_tab;

                            let (card_rect, resp) = ui.allocate_exact_size(
                                vec2(card_width, card_height),
                                Sense::click(),
                            );

                            let is_hovered = resp.hovered();
                            // 悬停动画插值：当前聚焦卡片常驻白色高亮，其他卡片平滑渐显
                            let hover_factor = ui.ctx().animate_bool_with_time(
                                egui::Id::new(("overview_tab_card_h", session_idx, tab_idx)),
                                is_hovered && !is_current_tab,
                                0.15,
                            );

                            // 卡片阴影与背景
                            let painter = ui.painter().with_clip_rect(card_rect);
                            let shadow_alpha = if is_current_tab {
                                if dark { 150 } else { 40 }
                            } else {
                                ((if dark { 70.0 } else { 18.0 }) + (if dark { 70.0 } else { 22.0 }) * hover_factor) as u8
                            };
                            let shadow_color = Color32::from_black_alpha(shadow_alpha);
                            let shadow_offset = if is_current_tab { 6.0 } else { 2.5 + 3.5 * hover_factor };
                            painter.rect_filled(
                                card_rect.translate(vec2(0.0, shadow_offset)),
                                CornerRadius::same(12),
                                shadow_color,
                            );

                            painter.rect_filled(card_rect, CornerRadius::same(12), bg_card);

                            // 当前聚焦卡片保持白色高亮边框；其他卡片渐变过渡
                            let border_color = if is_current_tab {
                                border_hover
                            } else {
                                lerp_c(border_normal, border_hover, hover_factor)
                            };
                            let border_w = if is_current_tab {
                                1.5
                            } else {
                                1.0 + 0.5 * hover_factor
                            };
                            painter.rect_stroke(
                                card_rect,
                                CornerRadius::same(12),
                                Stroke::new(border_w, border_color),
                                egui::StrokeKind::Inside,
                            );

                            // ---- 卡片 Header（垂直绝对对称居中） ----
                            let tab_header_rect = Rect::from_min_size(card_rect.min, vec2(card_width, header_h));
                            let header_center_y = tab_header_rect.center().y;

                            // 状态圆点
                            let is_alive = tab.alive.load(std::sync::atomic::Ordering::SeqCst);
                            let dot_color = if is_alive {
                                Color32::from_rgb(46, 204, 113)
                            } else {
                                Color32::from_gray(140)
                            };
                            let dot_pos = Pos2::new(tab_header_rect.min.x + 16.0, header_center_y);
                            painter.circle_filled(dot_pos, 4.5, dot_color);

                            // 标签名称 (Tab 1, Tab 2...)
                            let tab_title = format!("Tab {}", tab_idx + 1);
                            let title_fg = if is_current_tab {
                                if dark { Color32::WHITE } else { Color32::BLACK }
                            } else {
                                lerp_c(text_main, if dark { Color32::WHITE } else { Color32::BLACK }, hover_factor)
                            };
                            painter.text(
                                Pos2::new(tab_header_rect.min.x + 28.0, header_center_y),
                                Align2::LEFT_CENTER,
                                &tab_title,
                                FontId::proportional(14.0),
                                title_fg,
                            );

                            let title_w = painter.layout_no_wrap(tab_title.clone(), FontId::proportional(14.0), text_main).rect.width();

                            // 当前活跃指示微徽章 (高级磨砂胶囊微徽章，与软件整体质感 100% 对齐)
                            if is_current_tab {
                                let badge_w = 40.0;
                                let badge_h = 19.0;
                                let badge_center_x = tab_header_rect.min.x + 28.0 + title_w + 8.0 + badge_w / 2.0;
                                let badge_rect = Rect::from_center_size(
                                    Pos2::new(badge_center_x, header_center_y),
                                    vec2(badge_w, badge_h),
                                );
                                let active_badge_bg = if dark {
                                    Color32::from_white_alpha(15)
                                } else {
                                    Color32::from_black_alpha(12)
                                };
                                let active_badge_stroke = if dark {
                                    Color32::from_white_alpha(28)
                                } else {
                                    Color32::from_black_alpha(22)
                                };
                                let active_text_fg = if dark {
                                    Color32::from_gray(240)
                                } else {
                                    Color32::from_gray(30)
                                };
                                painter.rect_filled(badge_rect, CornerRadius::same(5), active_badge_bg);
                                painter.rect_stroke(badge_rect, CornerRadius::same(5), Stroke::new(0.5, active_badge_stroke), egui::StrokeKind::Inside);
                                painter.text(
                                    badge_rect.center(),
                                    Align2::CENTER_CENTER,
                                    "当前",
                                    FontId::proportional(10.5),
                                    active_text_fg,
                                );
                            }

                            // 卡片右上角关闭按钮 ✕（精美微结构按键）
                            let tab_close_rect = Rect::from_center_size(
                                Pos2::new(tab_header_rect.max.x - 18.0, header_center_y),
                                vec2(22.0, 22.0),
                            );
                            let tab_close_resp = ui.interact(
                                tab_close_rect,
                                egui::Id::new(("overview_tab_close", session_idx, tab_idx)),
                                Sense::click(),
                            ).on_hover_text("关闭此标签页");
                            let close_hf = ui.ctx().animate_bool(
                                egui::Id::new(("overview_tab_close_h", session_idx, tab_idx)),
                                tab_close_resp.hovered(),
                            );
                            let is_tab_closed = tab_close_resp.clicked();
                            
                            let tab_close_bg = lerp_c(
                                Color32::TRANSPARENT,
                                Color32::from_rgba_unmultiplied(235, 75, 65, if dark { 45 } else { 30 }),
                                close_hf,
                            );
                            let tab_close_stroke = lerp_c(
                                Color32::TRANSPARENT,
                                Color32::from_rgba_unmultiplied(235, 75, 65, if dark { 80 } else { 55 }),
                                close_hf,
                            );
                            let tab_close_fg = lerp_c(
                                if dark { Color32::from_gray(140) } else { Color32::from_gray(120) },
                                Color32::WHITE,
                                close_hf,
                            );

                            painter.rect_filled(tab_close_rect, CornerRadius::same(5), tab_close_bg);
                            painter.rect_stroke(tab_close_rect, CornerRadius::same(5), Stroke::new(0.5, tab_close_stroke), egui::StrokeKind::Inside);
                            let cc = tab_close_rect.center();
                            let ccd = 3.2;
                            painter.line_segment([cc + vec2(-ccd, -ccd), cc + vec2(ccd, ccd)], egui::Stroke::new(1.2, tab_close_fg));
                            painter.line_segment([cc + vec2(-ccd, ccd), cc + vec2(ccd, -ccd)], egui::Stroke::new(1.2, tab_close_fg));

                            if is_tab_closed {
                                action = Some(OverviewAction::CloseTab {
                                    session_idx,
                                    tab_idx,
                                });
                            }

                            // ---- 卡片 Body (全彩等比微缩终端画面) ----
                            let preview_rect = Rect::from_min_max(
                                Pos2::new(card_rect.min.x + preview_margin_x, card_rect.min.y + header_h),
                                Pos2::new(card_rect.max.x - preview_margin_x, card_rect.max.y - preview_margin_y),
                            );

                            painter.rect_filled(preview_rect, CornerRadius::same(6), term_theme.background);
                            painter.rect_stroke(
                                preview_rect,
                                CornerRadius::same(6),
                                Stroke::new(0.5, border_normal),
                                egui::StrokeKind::Inside,
                            );

                            if let Some(t) = &tab.terminal {
                                render_scaled_terminal(ui, t, term_theme, preview_rect, orig_char_w, orig_row_h);
                            } else {
                                painter.text(
                                    preview_rect.center(),
                                    Align2::CENTER_CENTER,
                                    "[终端未初始化 · 点击启动]",
                                    FontId::proportional(12.0),
                                    text_sub,
                                );
                            }

                            // 点击卡片聚焦进入该标签页
                            if resp.clicked() && !is_tab_closed {
                                action = Some(OverviewAction::SelectSessionTab {
                                    session_idx,
                                    tab_idx,
                                });
                            }

                            // 卡片右键菜单
                            resp.context_menu(|ui| {
                                ui.set_min_width(150.0);
                                ui.add_space(2.0);
                                ui.label(
                                    egui::RichText::new(format!("{} - {}", s.name, tab_title))
                                        .font(FontId::proportional(13.0))
                                        .color(text_main)
                                        .strong(),
                                );
                                ui.add_space(2.0);
                                ui.separator();
                                ui.add_space(2.0);

                                if ui.button("▶ 进入此标签页").clicked() {
                                    action = Some(OverviewAction::SelectSessionTab {
                                        session_idx,
                                        tab_idx,
                                    });
                                    ui.close();
                                }

                                if ui.button("✕ 关闭此标签页").clicked() {
                                    action = Some(OverviewAction::CloseTab {
                                        session_idx,
                                        tab_idx,
                                    });
                                    ui.close();
                                }

                                if ui.button("＋ 在此会话新建标签页").clicked() {
                                    action = Some(OverviewAction::NewTab(session_idx));
                                    ui.close();
                                }
                            });
                        } else if item_idx == total_tabs {
                            // 渲染末尾的「＋ 新建标签页」微卡片
                            let (card_rect, resp) = ui.allocate_exact_size(
                                vec2(card_width, card_height),
                                Sense::click(),
                            );
                            let is_hovered = resp.hovered();
                            let hover_factor = ui.ctx().animate_bool_with_time(
                                egui::Id::new(("overview_tab_add_card_h", session_idx)),
                                is_hovered,
                                0.15,
                            );

                            let painter = ui.painter().with_clip_rect(card_rect);
                            let ghost_base_bg = if dark { Color32::from_white_alpha(4) } else { Color32::from_black_alpha(4) };
                            let ghost_hover_bg = if dark { Color32::from_white_alpha(10) } else { Color32::from_black_alpha(8) };
                            let ghost_bg = lerp_c(ghost_base_bg, ghost_hover_bg, hover_factor);

                            let ghost_base_stroke = if dark { Color32::from_white_alpha(15) } else { Color32::from_black_alpha(15) };
                            let ghost_hover_stroke = if dark { Color32::from_white_alpha(40) } else { Color32::from_black_alpha(30) };
                            let ghost_stroke = lerp_c(ghost_base_stroke, ghost_hover_stroke, hover_factor);

                            painter.rect_filled(card_rect, CornerRadius::same(12), ghost_bg);
                            painter.rect_stroke(
                                card_rect,
                                CornerRadius::same(12),
                                Stroke::new(1.0, ghost_stroke),
                                egui::StrokeKind::Inside,
                            );

                            let center = card_rect.center();
                            let icon_fg = lerp_c(text_sub, if dark { Color32::WHITE } else { Color32::BLACK }, hover_factor);

                            painter.text(
                                Pos2::new(center.x, center.y - 12.0),
                                Align2::CENTER_CENTER,
                                "＋",
                                FontId::proportional(26.0),
                                icon_fg,
                            );
                            painter.text(
                                Pos2::new(center.x, center.y + 16.0),
                                Align2::CENTER_CENTER,
                                "新建标签页 (New Tab)",
                                FontId::proportional(13.0),
                                icon_fg,
                            );

                            if resp.on_hover_text("为此会话开启新的终端标签页").clicked() {
                                action = Some(OverviewAction::NewTab(session_idx));
                            }
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

/// 像素级等比真彩微缩终端渲染器：
/// 严格依据等宽字体的真实长宽比等比缩小，字符与框线 100% 严密贴合，绝不错位。
fn render_scaled_terminal(
    ui: &mut Ui,
    terminal: &crate::backend::terminal::Terminal,
    term_theme: &TermTheme,
    rect: Rect,
    orig_char_w: f32,
    orig_row_h: f32,
) {
    let term = &terminal.term;
    let grid = term.grid();
    let colors = term.colors();

    let screen_lines = grid.screen_lines();
    let cols = terminal.dimensions().0 as usize;

    if screen_lines == 0 || cols == 0 || orig_char_w <= 0.0 || orig_row_h <= 0.0 {
        return;
    }

    // 终端内容的完整原始物理尺寸
    let full_content_w = cols as f32 * orig_char_w;
    let full_content_h = screen_lines as f32 * orig_row_h;

    // 计算等比缩放因子，使整个终端画面严密适配预览区域
    let scale = (rect.width() / full_content_w).min(rect.height() / full_content_h);

    let scaled_font_size = (term_theme.font_size * scale).max(3.5);
    let font_mono = FontId::new(scaled_font_size, term_theme.font_family.clone());

    // 精确获取该微缩字号下的实际单字符宽度与行高
    let (char_w, row_h) = ui.fonts_mut(|f| (f.glyph_width(&font_mono, ' '), f.row_height(&font_mono)));

    let actual_render_w = cols as f32 * char_w;
    let actual_render_h = screen_lines as f32 * row_h;

    // 居中呈现
    let origin_x = rect.min.x + (rect.width() - actual_render_w).max(0.0) / 2.0;
    let origin_y = rect.min.y + (rect.height() - actual_render_h).max(0.0) / 2.0;

    let painter = ui.painter().with_clip_rect(rect);

    // 收集当前视口的所有行
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
        let y = origin_y + li as f32 * row_h;

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
            let x0 = origin_x + *s as f32 * char_w;
            let x1 = origin_x + *e as f32 * char_w;
            painter.rect_filled(
                Rect::from_min_max(Pos2::new(x0, y), Pos2::new(x1, y + row_h)),
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
            let pos = Pos2::new(origin_x + *start as f32 * char_w, y);
            painter.text(pos, Align2::LEFT_TOP, text, font_mono.clone(), *color);
        }
    }
}
