//! 设置页面组件：100% 对齐 CLIHub Workspaces 侧边栏卡片代码规范。
//! 极简克制，饱满微立体阴影卡片，自定义配色下拉器，精准内嵌色块方框。

use egui::{
    Align2, Color32, FontId, Id, Pos2, Rect, RichText, Sense, Stroke, Ui, vec2,
};

use crate::config::{AttachmentPillPosition, NotificationSettings, ThemeSettings};

/// 渲染全局偏好设置模态窗口
pub fn show_settings_modal(
    ui: &mut Ui,
    theme_draft: &mut ThemeSettings,
    notification_draft: &mut NotificationSettings,
    open: &mut bool,
) -> bool {
    let mut changed = false;
    let mut close = false;

    let screen_rect = ui.ctx().input(|i| i.raw.screen_rect).unwrap_or_else(|| ui.max_rect());
    let dark = ui.visuals().dark_mode;
    let accent_rgb = theme_draft.sidebar_card_color.unwrap_or([147, 112, 219]);

    // 半透明柔和暗黑背景遮罩
    ui.painter().rect_filled(screen_rect, 0.0, Color32::from_black_alpha(if dark { 160 } else { 75 }));

    egui::Area::new(Id::new("app_settings_modal"))
        .order(egui::Order::Foreground)
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ui.ctx(), |ui: &mut Ui| {
            let modal_w = 460.0;
            let modal_bg = if dark { Color32::from_rgb(18, 18, 22) } else { Color32::from_rgb(250, 250, 252) };
            let modal_border = if dark { Color32::from_white_alpha(8) } else { Color32::from_black_alpha(10) };
            let modal_shadow = Color32::from_black_alpha(if dark { 140 } else { 30 });

            let frame = egui::Frame::NONE
                .fill(modal_bg)
                .stroke(Stroke::new(0.5, modal_border))
                .corner_radius(12)
                .inner_margin(22.0)
                .shadow(egui::epaint::Shadow { offset: [0, 16], blur: 32, spread: 0, color: modal_shadow });

            frame.show(ui, |ui: &mut Ui| {
                ui.set_width(modal_w);

                // ---- 1. 顶部标题栏 ----
                ui.horizontal(|ui: &mut Ui| {
                    ui.label(RichText::new("SETTINGS").size(11.0).color(if dark { Color32::from_gray(130) } else { Color32::from_gray(120) }).strong());
                    ui.label(RichText::new("偏好设置").size(13.0).color(if dark { Color32::from_gray(200) } else { Color32::from_gray(60) }));

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui: &mut Ui| {
                        let (rect, resp) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::click());
                        let hf = ui.ctx().animate_bool(Id::new("settings_close_btn_h"), resp.hovered());
                        let btn_bg = lerp_color(Color32::TRANSPARENT, if dark { Color32::from_white_alpha(12) } else { Color32::from_black_alpha(12) }, hf);

                        ui.painter().rect_filled(rect, 5.0, btn_bg);

                        let c = rect.center();
                        let fg = lerp_color(
                            if dark { Color32::from_gray(130) } else { Color32::from_gray(120) },
                            if dark { Color32::WHITE } else { Color32::BLACK },
                            hf,
                        );
                        let s = 3.5;
                        ui.painter().line_segment([c - vec2(s, s), c + vec2(s, s)], Stroke::new(1.2, fg));
                        ui.painter().line_segment([c - vec2(s, -s), c + vec2(s, -s)], Stroke::new(1.2, fg));

                        if resp.clicked() {
                            close = true;
                        }
                    });
                });

                ui.add_space(14.0);

                // ---- 2. 界面主题 ----
                ui.label(RichText::new("界面主题").size(12.0).color(if dark { Color32::from_gray(150) } else { Color32::from_gray(100) }));
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    let card_w = (modal_w - 6.0) / 2.0;
                    if draw_ws_card(ui, dark, accent_rgb, card_w, 42.0, "深色模式", theme_draft.dark) {
                        theme_draft.dark = true;
                        theme_draft.color_scheme = "Campbell".to_string();
                        theme_draft.background = None;
                        theme_draft.foreground = None;
                        changed = true;
                    }
                    if draw_ws_card(ui, dark, accent_rgb, card_w, 42.0, "浅色模式", !theme_draft.dark) {
                        theme_draft.dark = false;
                        theme_draft.color_scheme = "One Half Light".to_string();
                        theme_draft.background = None;
                        theme_draft.foreground = None;
                        changed = true;
                    }
                });

                ui.add_space(12.0);

                // ---- 3. 终端配色方案与主题色 ----
                ui.label(RichText::new("色彩与主题").size(12.0).color(if dark { Color32::from_gray(150) } else { Color32::from_gray(100) }));
                ui.add_space(4.0);

                // 自定义卡片式配色方案选择器（深黑卡片 + 矢量下箭头 + 弹出浮层）
                ui.horizontal(|ui| {
                    ui.label(RichText::new("配色方案").size(12.0).color(if dark { Color32::from_gray(180) } else { Color32::from_gray(80) }));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if draw_custom_scheme_combo(ui, dark, &mut theme_draft.color_scheme) {
                            theme_draft.background = None;
                            theme_draft.foreground = None;
                            changed = true;
                        }
                    });
                });

                ui.add_space(6.0);

                let default_theme = crate::ui::terminal::TermTheme::from_scheme(&theme_draft.color_scheme);
                let preset_bg = [default_theme.background.r(), default_theme.background.g(), default_theme.background.b()];
                let preset_fg = [default_theme.foreground.r(), default_theme.foreground.g(), default_theme.foreground.b()];
                let mut bg = theme_draft.background.unwrap_or(preset_bg);
                let mut fg = theme_draft.foreground.unwrap_or(preset_fg);
                let mut accent = theme_draft.sidebar_card_color.unwrap_or([147, 112, 219]);

                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    let swatch_w = (modal_w - 12.0) / 3.0;
                    if draw_color_card(ui, dark, swatch_w, 44.0, "主题色", &mut accent) {
                        theme_draft.sidebar_card_color = Some(accent);
                        changed = true;
                    }
                    if draw_color_card(ui, dark, swatch_w, 44.0, "终端背景", &mut bg) {
                        theme_draft.background = Some(bg);
                        changed = true;
                    }
                    if draw_color_card(ui, dark, swatch_w, 44.0, "终端文字", &mut fg) {
                        theme_draft.foreground = Some(fg);
                        changed = true;
                    }
                });

                ui.add_space(12.0);

                // ---- 4. 图片附件暂存区位置 ----
                ui.label(RichText::new("图片附件暂存区位置").size(12.0).color(if dark { Color32::from_gray(150) } else { Color32::from_gray(100) }));
                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    use AttachmentPillPosition::*;
                    let curr_pos = theme_draft.attachment_position;
                    let card_w = (modal_w - 12.0) / 3.0;

                    if draw_ws_card(ui, dark, accent_rgb, card_w, 44.0, "右上角 HUD", curr_pos == TopRight) {
                        theme_draft.attachment_position = TopRight;
                        changed = true;
                    }
                    if draw_ws_card(ui, dark, accent_rgb, card_w, 44.0, "顶部横向槽", curr_pos == TopBanner) {
                        theme_draft.attachment_position = TopBanner;
                        changed = true;
                    }
                    if draw_ws_card(ui, dark, accent_rgb, card_w, 44.0, "右下角悬浮", curr_pos == BottomRight) {
                        theme_draft.attachment_position = BottomRight;
                        changed = true;
                    }
                });

                ui.add_space(12.0);

                // ---- 5. 系统通知 ----
                ui.label(RichText::new("系统通知与提醒").size(12.0).color(if dark { Color32::from_gray(150) } else { Color32::from_gray(100) }));
                ui.add_space(4.0);

                if draw_ws_card(ui, dark, accent_rgb, modal_w, 44.0, "启用 Windows 系统通知", notification_draft.enabled) {
                    notification_draft.enabled = !notification_draft.enabled;
                    changed = true;
                }

                if notification_draft.enabled {
                    ui.add_space(6.0);
                    let sub_card_w = (modal_w - 6.0) / 2.0;
                    ui.horizontal(|ui| {
                        if draw_ws_card(ui, dark, accent_rgb, sub_card_w, 38.0, "任务等待确认提醒", notification_draft.on_attention_needed) {
                            notification_draft.on_attention_needed = !notification_draft.on_attention_needed;
                            changed = true;
                        }
                        if draw_ws_card(ui, dark, accent_rgb, sub_card_w, 38.0, "进程退出时提醒", notification_draft.on_process_exit) {
                            notification_draft.on_process_exit = !notification_draft.on_process_exit;
                            changed = true;
                        }
                    });
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        if draw_ws_card(ui, dark, accent_rgb, sub_card_w, 38.0, "仅后台免打扰", notification_draft.only_when_unfocused) {
                            notification_draft.only_when_unfocused = !notification_draft.only_when_unfocused;
                            changed = true;
                        }
                        if draw_ws_card(ui, dark, accent_rgb, sub_card_w, 38.0, "播放提示音", notification_draft.play_sound) {
                            notification_draft.play_sound = !notification_draft.play_sound;
                            changed = true;
                        }
                    });
                }

                ui.add_space(14.0);

                // ---- 6. 底部操作栏 ----
                ui.horizontal(|ui: &mut Ui| {
                    ui.label(RichText::new("Auto-saved").size(11.5).color(if dark { Color32::from_gray(110) } else { Color32::from_gray(140) }));

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui: &mut Ui| {
                        let (rect, resp) = ui.allocate_exact_size(vec2(96.0, 26.0), Sense::click());
                        let hf = ui.ctx().animate_bool(Id::new("settings_reset_btn_h"), resp.hovered());

                        let btn_base = if dark { Color32::from_white_alpha(5) } else { Color32::from_black_alpha(6) };
                        let btn_hover = if dark { Color32::from_white_alpha(12) } else { Color32::from_black_alpha(12) };
                        let bg = lerp_color(btn_base, btn_hover, hf);

                        ui.painter().rect_filled(rect, 6.0, bg);

                        let text_c = lerp_color(
                            if dark { Color32::from_gray(150) } else { Color32::from_gray(100) },
                            if dark { Color32::WHITE } else { Color32::BLACK },
                            hf,
                        );
                        ui.painter().text(rect.center(), Align2::CENTER_CENTER, "恢复默认值", FontId::new(12.0, egui::FontFamily::Proportional), text_c);

                        if resp.clicked() {
                            *theme_draft = ThemeSettings::default();
                            *notification_draft = NotificationSettings::default();
                            changed = true;
                        }
                    });
                });
            });
        });

    if close {
        *open = false;
    }

    changed
}

/// 照搬 sidebar.rs 纯正 Workspaces 卡片代码规范（纯净填充，绝无描边，自然双层阴影）
fn draw_ws_card(
    ui: &mut Ui,
    dark: bool,
    custom_color: [u8; 3],
    w: f32,
    h: f32,
    label: &str,
    is_sel: bool,
) -> bool {
    let (rect, resp) = ui.allocate_exact_size(vec2(w, h), Sense::click());
    let hovered = resp.hovered();

    let sel_factor = ui.ctx().animate_bool(Id::new(("ws_card_sel", label)), is_sel);
    let hover_factor = ui.ctx().animate_bool(Id::new(("ws_card_hov", label)), hovered && !is_sel);

    let base_color = if dark { Color32::from_white_alpha(5) } else { Color32::from_black_alpha(8) };
    let hover_color = if dark { Color32::from_white_alpha(12) } else { Color32::from_black_alpha(15) };

    let sel_color = if dark {
        Color32::from_rgba_unmultiplied(custom_color[0], custom_color[1], custom_color[2], 50)
    } else {
        Color32::from_rgba_unmultiplied(custom_color[0], custom_color[1], custom_color[2], 30)
    };

    let bg = lerp_color(lerp_color(base_color, hover_color, hover_factor), sel_color, sel_factor);

    // 1. 双层漫反射立体微阴影
    let s1 = if dark { Color32::from_black_alpha(35) } else { Color32::from_black_alpha(10) };
    let s2 = if dark { Color32::from_black_alpha(65) } else { Color32::from_black_alpha(15) };
    ui.painter().rect_filled(rect.translate(vec2(0.0, 2.5)), 12.0, s1);
    ui.painter().rect_filled(rect.translate(vec2(0.0, 1.2)), 12.0, s2);

    // 2. 纯净卡片本体（100% 零描边，与 Workspaces 侧边栏完全一致）
    ui.painter().rect_filled(rect, 12.0, bg);

    // 3. 文字颜色插值
    let name_normal = if dark { Color32::from_gray(160) } else { Color32::from_gray(100) };
    let name_hover = if dark { Color32::from_gray(210) } else { Color32::from_gray(50) };
    let name_sel = if dark { Color32::WHITE } else { Color32::BLACK };
    let fg = lerp_color(lerp_color(name_normal, name_hover, hover_factor), name_sel, sel_factor);

    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::new(12.5, egui::FontFamily::Proportional),
        fg,
    );

    resp.clicked()
}

/// 绘制卡片式配色方案下拉选择器（深黑半透明卡片 + 矢量下箭头 + 弹出浮层选单）
fn draw_custom_scheme_combo(ui: &mut Ui, dark: bool, current: &mut String) -> bool {
    let mut changed = false;
    let schemes = ["Campbell", "One Half Dark", "One Half Light", "Solarized Dark", "Solarized Light", "Nord"];
    
    let btn_size = vec2(130.0, 28.0);
    let (rect, resp) = ui.allocate_exact_size(btn_size, Sense::click());
    let hf = ui.ctx().animate_bool(Id::new("scheme_combo_h"), resp.hovered());

    let base_bg = if dark { Color32::from_white_alpha(6) } else { Color32::from_black_alpha(8) };
    let hover_bg = if dark { Color32::from_white_alpha(14) } else { Color32::from_black_alpha(14) };
    let bg = lerp_color(base_bg, hover_bg, hf);

    let shadow_color = if dark { Color32::from_black_alpha(40) } else { Color32::from_black_alpha(10) };
    ui.painter().rect_filled(rect.translate(vec2(0.0, 1.0)), 6.0, shadow_color);
    ui.painter().rect_filled(rect, 6.0, bg);

    // 方案名称
    let text_c = if dark { Color32::from_gray(200) } else { Color32::from_gray(60) };
    ui.painter().text(
        Pos2::new(rect.min.x + 10.0, rect.center().y),
        Align2::LEFT_CENTER,
        current.as_str(),
        FontId::new(12.0, egui::FontFamily::Proportional),
        text_c,
    );

    // 纯矢量向下小三角箭头
    let arrow_c = rect.right_center() - vec2(12.0, 0.0);
    let arrow_fg = if dark { Color32::from_gray(140) } else { Color32::from_gray(120) };
    let s = 2.5;
    ui.painter().line_segment([arrow_c + vec2(-s, -s * 0.6), arrow_c + vec2(0.0, s * 0.6)], Stroke::new(1.2, arrow_fg));
    ui.painter().line_segment([arrow_c + vec2(0.0, s * 0.6), arrow_c + vec2(s, -s * 0.6)], Stroke::new(1.2, arrow_fg));

    // 响应点击打开浮层
    let popup_id = Id::new("scheme_dropdown_popup_menu");
    let mut is_open = ui.data(|d| d.get_temp::<bool>(popup_id).unwrap_or(false));
    if resp.clicked() {
        is_open = !is_open;
        ui.data_mut(|d| d.insert_temp(popup_id, is_open));
    }

    if is_open {
        let popup_pos = rect.left_bottom() + vec2(0.0, 4.0);
        egui::Area::new(popup_id)
            .order(egui::Order::Foreground)
            .fixed_pos(popup_pos)
            .show(ui.ctx(), |ui| {
                let frame_bg = if dark { Color32::from_rgb(26, 26, 32) } else { Color32::from_rgb(245, 245, 248) };
                let frame_stroke = Stroke::new(0.5, if dark { Color32::from_white_alpha(15) } else { Color32::from_black_alpha(15) });
                egui::Frame::NONE
                    .fill(frame_bg)
                    .stroke(frame_stroke)
                    .corner_radius(8)
                    .inner_margin(4.0)
                    .shadow(egui::epaint::Shadow { offset: [0, 8], blur: 16, spread: 0, color: Color32::from_black_alpha(100) })
                    .show(ui, |ui| {
                        ui.set_width(130.0);
                        for scheme in &schemes {
                            let is_curr = current.as_str() == *scheme;
                            let (item_rect, item_resp) = ui.allocate_exact_size(vec2(130.0, 26.0), Sense::click());
                            let item_hf = ui.ctx().animate_bool(Id::new(("scheme_item_h", *scheme)), item_resp.hovered());
                            let item_bg = if is_curr {
                                if dark { Color32::from_white_alpha(16) } else { Color32::from_black_alpha(16) }
                            } else {
                                lerp_color(Color32::TRANSPARENT, if dark { Color32::from_white_alpha(8) } else { Color32::from_black_alpha(8) }, item_hf)
                            };
                            ui.painter().rect_filled(item_rect, 4.0, item_bg);
                            let item_fg = if is_curr {
                                if dark { Color32::WHITE } else { Color32::BLACK }
                            } else {
                                if dark { Color32::from_gray(190) } else { Color32::from_gray(70) }
                            };
                            ui.painter().text(
                                Pos2::new(item_rect.min.x + 8.0, item_rect.center().y),
                                Align2::LEFT_CENTER,
                                *scheme,
                                FontId::new(12.0, egui::FontFamily::Proportional),
                                item_fg,
                            );
                            if item_resp.clicked() {
                                *current = scheme.to_string();
                                changed = true;
                                ui.data_mut(|d| d.insert_temp(popup_id, false));
                            }
                        }
                    });
            });

        if ui.ctx().input(|i| i.pointer.any_click()) && !resp.clicked() {
            ui.data_mut(|d| d.insert_temp(popup_id, false));
        }
    }

    changed
}

/// 绘制带精致外围方框的色块卡片
fn draw_color_card(ui: &mut Ui, dark: bool, w: f32, h: f32, label: &str, color: &mut [u8; 3]) -> bool {
    let mut changed = false;
    let (rect, resp) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
    let hf = ui.ctx().animate_bool(Id::new(("color_card_h", label)), resp.hovered());

    let base_color = if dark { Color32::from_white_alpha(5) } else { Color32::from_black_alpha(8) };
    let hover_color = if dark { Color32::from_white_alpha(12) } else { Color32::from_black_alpha(15) };
    let bg = lerp_color(base_color, hover_color, hf);

    let s1 = if dark { Color32::from_black_alpha(35) } else { Color32::from_black_alpha(10) };
    let s2 = if dark { Color32::from_black_alpha(65) } else { Color32::from_black_alpha(15) };
    ui.painter().rect_filled(rect.translate(vec2(0.0, 2.5)), 12.0, s1);
    ui.painter().rect_filled(rect.translate(vec2(0.0, 1.2)), 12.0, s2);
    ui.painter().rect_filled(rect, 12.0, bg);

    // 标签文本
    let text_x = rect.min.x + 12.0;
    ui.painter().text(
        Pos2::new(text_x, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::new(12.0, egui::FontFamily::Proportional),
        if dark { Color32::from_gray(180) } else { Color32::from_gray(60) },
    );

    // 右侧色块与外围清晰方框
    let swatch_size = vec2(18.0, 18.0);
    let swatch_rect = Rect::from_center_size(Pos2::new(rect.max.x - 22.0, rect.center().y), swatch_size);

    // 绘制清晰的外围小方框边框（距色块 2px）
    let outer_border_rect = swatch_rect.expand(2.5);
    let border_color = if dark { Color32::from_gray(80) } else { Color32::from_gray(190) };
    ui.painter().rect_stroke(outer_border_rect, 4.0, Stroke::new(1.0, border_color), egui::StrokeKind::Inside);

    let mut child_ui = ui.new_child(egui::UiBuilder::new().max_rect(swatch_rect));
    child_ui.spacing_mut().interact_size = swatch_size;
    let prev_stroke = child_ui.visuals().widgets.noninteractive.bg_stroke;
    child_ui.visuals_mut().widgets.noninteractive.bg_stroke = Stroke::NONE;
    let r = child_ui.color_edit_button_srgb(color);
    if r.changed() {
        changed = true;
    }
    child_ui.visuals_mut().widgets.noninteractive.bg_stroke = prev_stroke;

    changed
}

fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    Color32::from_rgba_premultiplied(
        (a.r() as f32 * (1.0 - t) + b.r() as f32 * t).clamp(0.0, 255.0) as u8,
        (a.g() as f32 * (1.0 - t) + b.g() as f32 * t).clamp(0.0, 255.0) as u8,
        (a.b() as f32 * (1.0 - t) + b.b() as f32 * t).clamp(0.0, 255.0) as u8,
        (a.a() as f32 * (1.0 - t) + b.a() as f32 * t).clamp(0.0, 255.0) as u8,
    )
}

