//! 会话管理模态窗口（新建会话 / 编辑会话）
//! 100% 对齐 CLIHub 设置页面卡片设计规范：
//! - 无边框流式极简输入框（与搜索框同款）
//! - Workspaces 卡片同款按键（双层立体漫反射阴影、纯净零描边、主题色强调）
//! - 精致标题栏与关闭微交互

use egui::{
    Align2, Color32, FontId, Id, Rect, RichText, Sense, Stroke, Ui, vec2,
};

/// 模态框交互动作
pub enum SessionModalAction {
    None,
    Confirm,
    Cancel,
}

/// 渲染新建或编辑会话模态窗口
pub fn show_session_modal(
    ui: &mut Ui,
    is_edit: bool,
    name: &mut String,
    command: &mut String,
    cwd: &mut String,
    accent_rgb: [u8; 3],
) -> SessionModalAction {
    let mut action = SessionModalAction::None;
    let screen_rect = ui.ctx().input(|i| i.raw.screen_rect).unwrap_or_else(|| ui.max_rect());
    let dark = ui.visuals().dark_mode;

    // 半透明柔和暗黑背景遮罩
    ui.painter().rect_filled(screen_rect, 0.0, Color32::from_black_alpha(if dark { 160 } else { 75 }));

    let area_id = if is_edit { "edit_session_modal" } else { "add_session_modal" };
    egui::Area::new(Id::new(area_id))
        .order(egui::Order::Foreground)
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ui.ctx(), |ui: &mut Ui| {
            let modal_w = 400.0;
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
                    let title_en = if is_edit { "EDIT SESSION" } else { "ADD SESSION" };
                    let title_zh = if is_edit { "编辑会话" } else { "新建会话" };

                    ui.label(RichText::new(title_en).size(11.0).color(if dark { Color32::from_gray(130) } else { Color32::from_gray(120) }).strong());
                    ui.label(RichText::new(title_zh).size(13.0).color(if dark { Color32::from_gray(200) } else { Color32::from_gray(60) }));

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui: &mut Ui| {
                        let (rect, resp) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::click());
                        let hf = ui.ctx().animate_bool(Id::new(("session_modal_close_h", is_edit)), resp.hovered());
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
                            action = SessionModalAction::Cancel;
                        }
                    });
                });

                ui.add_space(14.0);

                // ---- 2. 会话名称 ----
                ui.label(RichText::new("会话名称").size(12.0).color(if dark { Color32::from_gray(150) } else { Color32::from_gray(100) }));
                ui.add_space(4.0);
                draw_fluent_input(ui, dark, modal_w, 34.0, name, "例如: Claude / PowerShell / Bash", Id::new(("session_input_name", is_edit)));

                ui.add_space(12.0);

                // ---- 3. 启动命令 ----
                ui.label(RichText::new("启动命令").size(12.0).color(if dark { Color32::from_gray(150) } else { Color32::from_gray(100) }));
                ui.add_space(4.0);
                draw_fluent_input(ui, dark, modal_w, 34.0, command, "例如: pwsh.exe / claude / wsl", Id::new(("session_input_cmd", is_edit)));

                ui.add_space(12.0);

                // ---- 4. 工作目录 (可选) ----
                ui.label(RichText::new("工作目录 (可选)").size(12.0).color(if dark { Color32::from_gray(150) } else { Color32::from_gray(100) }));
                ui.add_space(4.0);
                ui.horizontal(|ui: &mut Ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    let browse_w = 78.0;
                    let input_w = modal_w - browse_w - 6.0;

                    draw_fluent_input(ui, dark, input_w, 34.0, cwd, "默认当前工作区目录", Id::new(("session_input_cwd", is_edit)));

                    if draw_btn(ui, dark, [147, 112, 219], browse_w, 34.0, "浏览...", false, true) {
                        if let Some(path) = rfd::FileDialog::new().pick_folder() {
                            *cwd = path.display().to_string();
                        }
                    }
                });

                ui.add_space(18.0);

                // ---- 5. 底部操作按键 ----
                let name_ok = !name.trim().is_empty();
                let cmd_ok = !command.trim().is_empty();
                let can_confirm = name_ok && cmd_ok;

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui: &mut Ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;

                    let confirm_label = if is_edit { "保存修改" } else { "添加会话" };
                    if draw_btn(ui, dark, accent_rgb, 96.0, 34.0, confirm_label, true, can_confirm) && can_confirm {
                        action = SessionModalAction::Confirm;
                    }

                    if draw_btn(ui, dark, accent_rgb, 78.0, 34.0, "取消", false, true) {
                        action = SessionModalAction::Cancel;
                    }
                });
            });
        });

    action
}

/// 绘制【阴刻】流式极简内凹输入槽体（Sunken Trench / Debossed Effect）
/// 与凸起按键（阳刻）形成鲜明视觉虚实呼应：
/// - 顶部内阴影（Top Inset Shadow）呈现向内深陷的下坠感
/// - 底部内边缘微反光（Bottom Lip Highlight）呈现槽体切口的立体光泽
/// - 深沉内敛底色，绝无凸起投影
fn draw_fluent_input(
    ui: &mut Ui,
    dark: bool,
    w: f32,
    h: f32,
    text: &mut String,
    hint: &str,
    id: Id,
) {
    let (rect, resp) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
    let hf = ui.ctx().animate_bool(id.with("_hov"), resp.hovered());

    // 1. 阴刻深陷槽体底色（比窗口底色更深沉、自然内陷）
    let sunken_bg = if dark {
        Color32::from_black_alpha(80)
    } else {
        Color32::from_black_alpha(15)
    };
    ui.painter().rect_filled(rect, 8.0, sunken_bg);

    // 2. 阴刻顶部内阴影（Top Inset Shadow - 模拟顶部槽口遮光形成的下坠阴影）
    let shadow_color_1 = if dark { Color32::from_black_alpha(110) } else { Color32::from_black_alpha(40) };
    let shadow_color_2 = if dark { Color32::from_black_alpha(60) } else { Color32::from_black_alpha(20) };
    ui.painter().line_segment(
        [rect.left_top() + vec2(4.0, 1.0), rect.right_top() - vec2(4.0, -1.0)],
        Stroke::new(1.2, shadow_color_1),
    );
    ui.painter().line_segment(
        [rect.left_top() + vec2(6.0, 2.2), rect.right_top() - vec2(6.0, -2.2)],
        Stroke::new(1.0, shadow_color_2),
    );

    // 3. 阴刻底部内沿微反光（Bottom Inset Lip Highlight - 槽口下沿朝上迎光的发丝亮边）
    let bottom_lip_color = if dark {
        lerp_color(Color32::from_white_alpha(8), Color32::from_white_alpha(16), hf)
    } else {
        lerp_color(Color32::from_white_alpha(50), Color32::from_white_alpha(90), hf)
    };
    ui.painter().line_segment(
        [rect.left_bottom() + vec2(4.0, 0.0), rect.right_bottom() - vec2(4.0, 0.0)],
        Stroke::new(0.6, bottom_lip_color),
    );

    // 4. 阴刻四周内沿微弱暗色边框
    let stroke_c = if dark { Color32::from_black_alpha(45) } else { Color32::from_black_alpha(12) };
    ui.painter().rect_stroke(rect, 8.0, Stroke::new(0.5, stroke_c), egui::StrokeKind::Inside);

    // 5. 内部无边框流式 TextEdit
    let input_margin = vec2(10.0, (h - 18.0) * 0.5);
    let inner_rect = Rect::from_min_max(rect.min + input_margin, rect.max - vec2(input_margin.x, input_margin.y));

    let mut child_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(inner_rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );

    let text_color = if dark { Color32::WHITE } else { Color32::BLACK };
    child_ui.add(
        egui::TextEdit::singleline(text)
            .id(id)
            .desired_width(inner_rect.width())
            .font(FontId::proportional(12.5))
            .text_color(text_color)
            .hint_text(hint)
            .frame(egui::Frame::NONE)
            .margin(vec2(0.0, 0.0)),
    );
}

/// 绘制 Workspaces 卡片同款操作按键
fn draw_btn(
    ui: &mut Ui,
    dark: bool,
    custom_color: [u8; 3],
    w: f32,
    h: f32,
    label: &str,
    is_primary: bool,
    enabled: bool,
) -> bool {
    let (rect, resp) = ui.allocate_exact_size(vec2(w, h), if enabled { Sense::click() } else { Sense::hover() });
    let hovered = resp.hovered() && enabled;
    let hf = ui.ctx().animate_bool(Id::new(("btn_hov", label, is_primary)), hovered);

    if enabled {
        let s1 = if dark { Color32::from_black_alpha(30) } else { Color32::from_black_alpha(8) };
        let s2 = if dark { Color32::from_black_alpha(55) } else { Color32::from_black_alpha(12) };
        ui.painter().rect_filled(rect.translate(vec2(0.0, 2.0)), 8.0, s1);
        ui.painter().rect_filled(rect.translate(vec2(0.0, 1.0)), 8.0, s2);
    }

    let bg = if is_primary {
        let base = if enabled {
            Color32::from_rgba_unmultiplied(custom_color[0], custom_color[1], custom_color[2], if dark { 55 } else { 40 })
        } else {
            Color32::from_rgba_unmultiplied(custom_color[0], custom_color[1], custom_color[2], if dark { 20 } else { 15 })
        };
        let hover = Color32::from_rgba_unmultiplied(custom_color[0], custom_color[1], custom_color[2], if dark { 80 } else { 60 });
        lerp_color(base, hover, hf)
    } else {
        let base = if dark { Color32::from_white_alpha(6) } else { Color32::from_black_alpha(8) };
        let hover = if dark { Color32::from_white_alpha(14) } else { Color32::from_black_alpha(14) };
        lerp_color(base, hover, hf)
    };

    ui.painter().rect_filled(rect, 8.0, bg);

    let text_c = if is_primary {
        if enabled {
            if dark { Color32::WHITE } else { Color32::BLACK }
        } else {
            if dark { Color32::from_white_alpha(90) } else { Color32::from_black_alpha(90) }
        }
    } else {
        let normal = if dark { Color32::from_gray(160) } else { Color32::from_gray(100) };
        let hover = if dark { Color32::WHITE } else { Color32::BLACK };
        lerp_color(normal, hover, hf)
    };

    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::new(12.5, egui::FontFamily::Proportional),
        text_c,
    );

    resp.clicked() && enabled
}

fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    Color32::from_rgba_premultiplied(
        (a.r() as f32 * (1.0 - t) + b.r() as f32 * t).clamp(0.0, 255.0) as u8,
        (a.g() as f32 * (1.0 - t) + b.g() as f32 * t).clamp(0.0, 255.0) as u8,
        (a.b() as f32 * (1.0 - t) + b.b() as f32 * t).clamp(0.0, 255.0) as u8,
        (a.a() as f32 * (1.0 - t) + b.a() as f32 * t).clamp(0.0, 255.0) as u8,
    )
}
