use crate::{
    collab::{
        Phase,
        workflows::{ChatState, Draft, MemberDraft},
    },
    state::Session,
};
use egui::{Color32, FontId, Id, Pos2, Rect, RichText, Sense, Stroke, Ui, vec2};

pub enum Action {
    New,
    Select(usize),
    Delete(usize),
    Send,
    Cancel,
    Handoff(String),
    Close,
}

#[allow(dead_code)]
fn phase_name(p: &Phase) -> &'static str {
    match p {
        Phase::Development => "开发",
        Phase::Review => "审查",
        Phase::Fix => "修复",
        Phase::Completed => "完成",
    }
}

fn phase_color(phase: &Phase) -> Color32 {
    match phase {
        Phase::Development => Color32::from_rgb(59, 130, 246),
        Phase::Review => Color32::from_rgb(245, 158, 11),
        Phase::Fix => Color32::from_rgb(239, 68, 68),
        Phase::Completed => Color32::from_rgb(16, 185, 129),
    }
}

fn muted(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(166, 173, 200)
    } else {
        Color32::from_rgb(148, 163, 184)
    }
}

fn text_color(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(205, 214, 244)
    } else {
        Color32::from_rgb(30, 41, 59)
    }
}

fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    Color32::from_rgba_premultiplied(
        (a.r() as f32 * (1.0 - t) + b.r() as f32 * t).clamp(0.0, 255.0) as u8,
        (a.g() as f32 * (1.0 - t) + b.g() as f32 * t).clamp(0.0, 255.0) as u8,
        (a.b() as f32 * (1.0 - t) + b.b() as f32 * t).clamp(0.0, 255.0) as u8,
        (a.a() as f32 * (1.0 - t) + b.a() as f32 * t).clamp(0.0, 255.0) as u8,
    )
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ToolIcon {
    Close,
    Plus,
    ChevronLeft,
    #[allow(dead_code)]
    ChevronDown,
    ArrowRight,
    Reply,
    Sidebar,
    Message,
}

fn paint_chevron_down(p: &egui::Painter, center: Pos2, size: f32, stroke: Stroke) {
    let half_w = size * 0.5;
    let half_h = size * 0.3;
    let tip = Pos2::new(center.x, center.y + half_h);
    p.line_segment([Pos2::new(center.x - half_w, center.y - half_h), tip], stroke);
    p.line_segment([tip, Pos2::new(center.x + half_w, center.y - half_h)], stroke);
}

fn paint_chevron_left(p: &egui::Painter, center: Pos2, size: f32, stroke: Stroke) {
    let half_h = size * 0.5;
    let half_w = size * 0.3;
    let tip = Pos2::new(center.x - half_w, center.y);
    p.line_segment([Pos2::new(center.x + half_w, center.y - half_h), tip], stroke);
    p.line_segment([tip, Pos2::new(center.x + half_w, center.y + half_h)], stroke);
}

fn paint_plus(p: &egui::Painter, center: Pos2, size: f32, stroke: Stroke) {
    let s = size * 0.5;
    p.line_segment([center + vec2(-s, 0.0), center + vec2(s, 0.0)], stroke);
    p.line_segment([center + vec2(0.0, -s), center + vec2(0.0, s)], stroke);
}

fn paint_close_x(p: &egui::Painter, center: Pos2, size: f32, stroke: Stroke) {
    let s = size * 0.5;
    p.line_segment([center + vec2(-s, -s), center + vec2(s, s)], stroke);
    p.line_segment([center + vec2(-s, s), center + vec2(s, -s)], stroke);
}

fn paint_arrow_right(p: &egui::Painter, center: Pos2, size: f32, stroke: Stroke) {
    let half_w = size * 0.5;
    let ah = size * 0.35;
    let start = Pos2::new(center.x - half_w, center.y);
    let tip = Pos2::new(center.x + half_w, center.y);
    p.line_segment([start, tip], stroke);
    p.line_segment([Pos2::new(tip.x - ah, tip.y - ah), tip], stroke);
    p.line_segment([Pos2::new(tip.x - ah, tip.y + ah), tip], stroke);
}

fn paint_reply_arrow(p: &egui::Painter, center: Pos2, size: f32, stroke: Stroke) {
    let half_w = size * 0.45;
    let half_h = size * 0.40;
    let tip = Pos2::new(center.x - half_w, center.y + half_h * 0.25);
    let corner = Pos2::new(center.x + half_w, center.y + half_h * 0.25);
    let top = Pos2::new(center.x + half_w, center.y - half_h);
    let top_hook = Pos2::new(center.x + half_w * 0.3, center.y - half_h);
    
    p.line_segment([top_hook, top], stroke);
    p.line_segment([top, corner], stroke);
    p.line_segment([corner, tip], stroke);
    
    let ah = size * 0.28;
    p.line_segment([Pos2::new(tip.x + ah, tip.y - ah), tip], stroke);
    p.line_segment([Pos2::new(tip.x + ah, tip.y + ah), tip], stroke);
}

fn paint_sidebar_icon(p: &egui::Painter, center: Pos2, stroke: Stroke, accent: Option<Color32>) {
    let w = 13.0;
    let h = 10.0;
    let r = Rect::from_center_size(center, vec2(w, h));
    p.rect_stroke(r, 2.0, stroke, egui::StrokeKind::Inside);
    let div_x = r.min.x + 4.0;
    p.line_segment([Pos2::new(div_x, r.min.y), Pos2::new(div_x, r.max.y)], stroke);
    let left_r = Rect::from_min_max(r.min, Pos2::new(div_x, r.max.y));
    let fill = accent.unwrap_or_else(|| stroke.color.gamma_multiply(0.35));
    p.rect_filled(left_r, 1.5, fill);
}

fn paint_message_icon(p: &egui::Painter, center: Pos2, stroke: Stroke) {
    let w = 12.0;
    let h = 9.0;
    let r = Rect::from_center_size(Pos2::new(center.x, center.y - 1.0), vec2(w, h));
    p.rect_stroke(r, 2.0, stroke, egui::StrokeKind::Inside);
    let tail_start = Pos2::new(r.min.x + 2.5, r.max.y);
    let tail_tip = Pos2::new(r.min.x + 1.0, r.max.y + 2.5);
    let tail_back = Pos2::new(r.min.x + 5.0, r.max.y);
    p.line_segment([tail_start, tail_tip], stroke);
    p.line_segment([tail_tip, tail_back], stroke);
}

fn paint_send_arrow(p: &egui::Painter, center: Pos2, size: f32, stroke: Stroke) {
    let half_h = size * 0.45;
    let ah = size * 0.35;
    let bottom = Pos2::new(center.x, center.y + half_h);
    let tip = Pos2::new(center.x, center.y - half_h);
    p.line_segment([bottom, tip], stroke);
    p.line_segment([Pos2::new(tip.x - ah, tip.y + ah), tip], stroke);
    p.line_segment([Pos2::new(tip.x + ah, tip.y + ah), tip], stroke);
}

fn paint_stop_square(p: &egui::Painter, center: Pos2, size: f32, fill: Color32) {
    p.rect_filled(Rect::from_center_size(center, vec2(size, size)), 1.5, fill);
}

fn vector_tool_button(ui: &mut Ui, icon: ToolIcon, tip: &str) -> bool {
    let dark = ui.visuals().dark_mode;
    let (rect, response) = ui.allocate_exact_size(vec2(24.0, 24.0), egui::Sense::click());
    let hover = ui.ctx().animate_bool(response.id.with("hover"), response.hovered() && ui.is_enabled());
    let alpha = (6.0 + 10.0 * hover) as u8;
    let fill = if dark { Color32::from_white_alpha(alpha) } else { Color32::from_black_alpha(alpha) };
    let border = if dark { Color32::from_white_alpha(8) } else { Color32::from_black_alpha(10) };
    let painter = ui.painter();
    painter.rect_filled(rect.translate(vec2(0.0, 1.2)), 5.0, Color32::from_black_alpha(if dark { 40 } else { 12 }));
    painter.rect_filled(rect, 5.0, fill);
    painter.rect_stroke(rect, 5.0, egui::Stroke::new(0.5, border), egui::StrokeKind::Inside);
    let fg = if !ui.is_enabled() {
        ui.visuals().weak_text_color()
    } else if response.hovered() {
        if dark { Color32::WHITE } else { Color32::BLACK }
    } else if dark {
        Color32::from_gray(170)
    } else {
        Color32::from_gray(90)
    };
    let stroke = Stroke::new(1.3, fg);
    match icon {
        ToolIcon::Close => paint_close_x(painter, rect.center(), 7.0, stroke),
        ToolIcon::Plus => paint_plus(painter, rect.center(), 8.0, stroke),
        ToolIcon::ChevronLeft => paint_chevron_left(painter, rect.center(), 7.0, stroke),
        ToolIcon::ChevronDown => paint_chevron_down(painter, rect.center(), 7.0, stroke),
        ToolIcon::ArrowRight => paint_arrow_right(painter, rect.center(), 9.0, stroke),
        ToolIcon::Reply => paint_reply_arrow(painter, rect.center(), 9.0, stroke),
        ToolIcon::Sidebar => paint_sidebar_icon(painter, rect.center(), stroke, None),
        ToolIcon::Message => paint_message_icon(painter, rect.center(), stroke),
    }
    response.on_hover_text(tip).clicked()
}

fn icon_pill_button(
    ui: &mut Ui,
    icon: Option<ToolIcon>,
    text: &str,
    tip: &str,
    active: bool,
    theme_accent: Option<[u8; 3]>,
) -> bool {
    let dark = ui.visuals().dark_mode;
    let font = egui::FontId::proportional(12.5);
    let fg_default = if !ui.is_enabled() {
        ui.visuals().weak_text_color()
    } else if active {
        if dark { Color32::WHITE } else { Color32::BLACK }
    } else {
        ui.visuals().text_color()
    };
    let galley = ui.painter().layout_no_wrap(text.to_string(), font.clone(), fg_default);
    
    let has_icon = icon.is_some();
    let icon_w = if has_icon { 14.0 } else { 0.0 };
    let gap = if has_icon { 6.0 } else { 0.0 };
    let pad_x = 10.0;
    let total_w = pad_x * 2.0 + icon_w + gap + galley.size().x;
    
    let (rect, response) = ui.allocate_exact_size(vec2(total_w, 28.0), egui::Sense::click());
    let hover = ui.ctx().animate_bool(response.id.with("hover"), response.hovered() && ui.is_enabled());
    let p = ui.painter();
    
    let (fill, border) = if active {
        if let Some(accent) = theme_accent {
            let bg = Color32::from_rgba_unmultiplied(accent[0], accent[1], accent[2], (45.0 + 15.0 * hover) as u8);
            let b = Color32::from_rgb(accent[0], accent[1], accent[2]).gamma_multiply(0.65);
            (bg, b)
        } else {
            let a = (22.0 + 12.0 * hover) as u8;
            let bg = if dark { Color32::from_white_alpha(a) } else { Color32::from_black_alpha(a) };
            let b = if dark { Color32::from_white_alpha(35) } else { Color32::from_black_alpha(38) };
            (bg, b)
        }
    } else {
        let a = (5.0 + 9.0 * hover) as u8;
        let bg = if dark { Color32::from_white_alpha(a) } else { Color32::from_black_alpha(a) };
        let b = if dark { Color32::from_white_alpha(8) } else { Color32::from_black_alpha(10) };
        (bg, b)
    };
    
    p.rect_filled(rect.translate(vec2(0.0, 1.2)), 6.0, Color32::from_black_alpha(if dark { 50 } else { 12 }));
    p.rect_filled(rect, 6.0, fill);
    p.rect_stroke(rect, 6.0, Stroke::new(0.6, border), egui::StrokeKind::Inside);
    
    let fg = if !ui.is_enabled() {
        ui.visuals().weak_text_color()
    } else if active {
        if dark { Color32::WHITE } else { Color32::BLACK }
    } else if hover > 0.01 {
        if dark { Color32::WHITE } else { Color32::BLACK }
    } else {
        ui.visuals().text_color()
    };
    
    let stroke = Stroke::new(1.3, fg);
    
    let mut cur_x = rect.min.x + pad_x;
    if let Some(ic) = icon {
        let icon_center = Pos2::new(cur_x + icon_w * 0.5, rect.center().y);
        let accent_c = theme_accent.map(|a| Color32::from_rgba_unmultiplied(a[0], a[1], a[2], 80));
        match ic {
            ToolIcon::Close => paint_close_x(p, icon_center, 7.0, stroke),
            ToolIcon::Plus => paint_plus(p, icon_center, 8.0, stroke),
            ToolIcon::ChevronLeft => paint_chevron_left(p, icon_center, 7.0, stroke),
            ToolIcon::ChevronDown => paint_chevron_down(p, icon_center, 7.0, stroke),
            ToolIcon::ArrowRight => paint_arrow_right(p, icon_center, 9.0, stroke),
            ToolIcon::Reply => paint_reply_arrow(p, icon_center, 9.0, stroke),
            ToolIcon::Sidebar => paint_sidebar_icon(p, icon_center, stroke, accent_c),
            ToolIcon::Message => paint_message_icon(p, icon_center, stroke),
        }
        cur_x += icon_w + gap;
    }
    
    p.text(Pos2::new(cur_x, rect.center().y), egui::Align2::LEFT_CENTER, text, font, fg);
    
    response.on_hover_text(tip).clicked()
}

fn draw_phase_badge(ui: &mut Ui, phase: &Phase) {
    let (name, bg, fg) = match phase {
        Phase::Development => ("开发", Color32::from_rgba_unmultiplied(59, 130, 246, 35), Color32::from_rgb(100, 180, 255)),
        Phase::Review => ("审查", Color32::from_rgba_unmultiplied(245, 158, 11, 35), Color32::from_rgb(255, 180, 90)),
        Phase::Fix => ("修复", Color32::from_rgba_unmultiplied(239, 68, 68, 35), Color32::from_rgb(255, 120, 120)),
        Phase::Completed => ("完成", Color32::from_rgba_unmultiplied(16, 185, 129, 35), Color32::from_rgb(100, 220, 150)),
    };
    let font = FontId::proportional(11.5);
    let galley = ui.painter().layout_no_wrap(name.to_string(), font.clone(), fg);
    let (rect, _) = ui.allocate_exact_size(vec2(galley.size().x + 12.0, 20.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, 5.0, bg);
    ui.painter().rect_stroke(rect, 5.0, Stroke::new(0.5, fg.gamma_multiply(0.4)), egui::StrokeKind::Inside);
    ui.painter().galley(rect.center() - galley.size() * 0.5, galley, fg);
}

fn chip_badge(ui: &mut Ui, text: &str) -> egui::Response {
    let dark = ui.visuals().dark_mode;
    let font = egui::FontId::proportional(11.5);
    let fg = if dark { Color32::from_gray(190) } else { Color32::from_gray(75) };
    let galley = ui.painter().layout_no_wrap(text.to_string(), font.clone(), fg);
    let (rect, resp) = ui.allocate_exact_size(vec2(galley.size().x + 12.0, 20.0), egui::Sense::hover());
    let bg = if dark { Color32::from_white_alpha(7) } else { Color32::from_black_alpha(8) };
    let border = if dark { Color32::from_white_alpha(10) } else { Color32::from_black_alpha(10) };
    ui.painter().rect_filled(rect, 4.0, bg);
    ui.painter().rect_stroke(rect, 4.0, egui::Stroke::new(0.5, border), egui::StrokeKind::Inside);
    ui.painter().galley(rect.center() - galley.size() * 0.5, galley, fg);
    resp
}

/// 侧边栏：项目列表与新建管理
fn draw_chat_sidebar(
    ui: &mut Ui,
    state: &mut ChatState,
    sidebar_rect: Rect,
    custom_color: [u8; 3],
) -> Option<Action> {
    let mut action = None;
    let dark = ui.visuals().dark_mode;
    let p = ui.painter();

    // 边栏整体柔和背景与右侧分隔线
    let sidebar_bg = if dark {
        Color32::from_black_alpha(35)
    } else {
        Color32::from_black_alpha(10)
    };
    p.rect_filled(sidebar_rect, 0.0, sidebar_bg);

    let divider_c = if dark {
        Color32::from_white_alpha(10)
    } else {
        Color32::from_black_alpha(12)
    };
    p.line_segment(
        [sidebar_rect.right_top(), sidebar_rect.right_bottom()],
        egui::Stroke::new(0.6, divider_c),
    );

    let mut side_ui = ui.new_child(
        egui::UiBuilder::new()
            .id_salt("chat_left_sidebar")
            .max_rect(sidebar_rect),
    );

    // 1. 顶部栏：PROJECTS 标题 + ＋ 新建 + ◀ 收起按钮（纯矢量绘制，严格水平垂直对齐）
    let (header_row_rect, _) = side_ui.allocate_exact_size(vec2(sidebar_rect.width(), 36.0), Sense::hover());
    let title_c = if dark { Color32::from_gray(165) } else { Color32::from_gray(95) };
    let mut header_ui = side_ui.new_child(
        egui::UiBuilder::new()
            .id_salt("chat_sidebar_header_ui")
            .max_rect(header_row_rect),
    );
    header_ui.horizontal(|ui| {
        ui.add_space(14.0);
        ui.label(RichText::new("PROJECTS").size(11.5).color(title_c).strong());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(10.0);
            if vector_tool_button(ui, ToolIcon::ChevronLeft, "收起侧边栏") {
                state.sidebar_expanded = false;
            }
            ui.add_space(2.0);
            if vector_tool_button(ui, ToolIcon::Plus, "新建工作流项目") {
                action = Some(Action::New);
            }
        });
    });

    // 2. 底部返回工作区按钮区域
    let bottom_h = 42.0;
    let bottom_rect = Rect::from_min_max(
        Pos2::new(sidebar_rect.min.x, sidebar_rect.max.y - bottom_h),
        sidebar_rect.max,
    );

    // 3. 中间卡片可滚动区域
    let cards_rect = Rect::from_min_max(
        Pos2::new(sidebar_rect.min.x, header_row_rect.max.y + 4.0),
        Pos2::new(sidebar_rect.max.x, bottom_rect.min.y - 4.0),
    );

    let mut cards_ui = side_ui.new_child(
        egui::UiBuilder::new()
            .id_salt("chat_cards_scroll_ui")
            .max_rect(cards_rect),
    );

    let active_idx = state.active.unwrap_or(0);
    let card_w = (cards_rect.width() - 16.0).max(120.0);

    egui::ScrollArea::vertical()
        .id_salt("chat_projects_list")
        .auto_shrink([false, false])
        .show(&mut cards_ui, |ui| {
            ui.add_space(2.0);
            if state.rounds.is_empty() {
                ui.centered_and_justified(|ui| {
                    ui.label(RichText::new("暂无项目\n点击 ＋ 新建").small().color(muted(dark)));
                });
            } else {
                for (i, (_, round)) in state.rounds.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.add_space(8.0);
                        let is_sel = i == active_idx;
                        let (rect, resp) = ui.allocate_exact_size(vec2(card_w, 54.0), Sense::click());
                        let hf = ui.ctx().animate_bool(resp.id.with("hov"), resp.hovered() && !is_sel);
                        let sf = ui.ctx().animate_bool(resp.id.with("sel"), is_sel);

                        let base_bg = if dark { Color32::from_white_alpha(5) } else { Color32::from_black_alpha(8) };
                        let hover_bg = if dark { Color32::from_white_alpha(13) } else { Color32::from_black_alpha(15) };
                        let sel_bg = if dark {
                            Color32::from_rgba_unmultiplied(custom_color[0], custom_color[1], custom_color[2], 42)
                        } else {
                            Color32::from_rgba_unmultiplied(custom_color[0], custom_color[1], custom_color[2], 26)
                        };
                        let bg = lerp_color(lerp_color(base_bg, hover_bg, hf), sel_bg, sf);

                        let shadow_c = if dark { Color32::from_black_alpha(45) } else { Color32::from_black_alpha(12) };
                        let cp = ui.painter();
                        cp.rect_filled(rect.translate(vec2(0.0, 1.2)), 10.0, shadow_c);
                        cp.rect_filled(rect, 10.0, bg);

                        let base_stroke = if dark { Color32::from_white_alpha(8) } else { Color32::from_black_alpha(10) };
                        let sel_stroke = Color32::from_rgb(custom_color[0], custom_color[1], custom_color[2]).gamma_multiply(0.65);
                        let stroke_c = lerp_color(base_stroke, sel_stroke, sf);
                        cp.rect_stroke(rect, 10.0, egui::Stroke::new(0.6, stroke_c), egui::StrokeKind::Inside);

                        // 状态指示圆点（依阶段着色）
                        let dot_c = phase_color(&round.phase);
                        cp.circle_filled(Pos2::new(rect.min.x + 14.0, rect.min.y + 17.0), 3.5, dot_c);

                        // 悬浮显示纯矢量 ✕ 删除按键
                        let del_center = Pos2::new(rect.max.x - 14.0, rect.min.y + 17.0);
                        let del_rect = Rect::from_center_size(del_center, vec2(18.0, 18.0));
                        let del_resp = ui.interact(del_rect, Id::new(("chat_card_del", i, &round.id)), Sense::click());
                        let del_hov = ui.ctx().animate_bool(del_resp.id.with("hov"), del_resp.hovered());

                        let is_card_hovered = resp.hovered();
                        let mut del_clicked = false;
                        if is_card_hovered || is_sel || del_resp.hovered() {
                            let d_fill = Color32::from_rgba_unmultiplied(220, 70, 70, (del_hov * 30.0) as u8);
                            let d_fg = lerp_color(
                                if dark { Color32::from_gray(140) } else { Color32::from_gray(120) },
                                Color32::from_rgb(240, 90, 90),
                                del_hov,
                            );
                            cp.rect_filled(del_rect, 4.0, d_fill);
                            paint_close_x(cp, del_rect.center(), 6.0, Stroke::new(1.2, d_fg));
                            if del_resp.on_hover_text("删除此项目").clicked() {
                                del_clicked = true;
                                action = Some(Action::Delete(i));
                            }
                        }

                        // 项目标题（预留删除键宽度）
                        let name_c = if is_sel {
                            if dark { Color32::WHITE } else { Color32::BLACK }
                        } else {
                            text_color(dark)
                        };
                        let title_max_w = card_w - 46.0;
                        let title_font = FontId::proportional(13.0);
                        let title_galley = cp.layout_no_wrap(round.title.clone(), title_font.clone(), name_c);
                        if title_galley.size().x > title_max_w {
                            let mut short = round.title.clone();
                            while !short.is_empty() && cp.layout_no_wrap(format!("{}…", short), title_font.clone(), name_c).size().x > title_max_w {
                                short.pop();
                            }
                            cp.text(Pos2::new(rect.min.x + 24.0, rect.min.y + 17.0), egui::Align2::LEFT_CENTER, format!("{}…", short), title_font, name_c);
                        } else {
                            cp.text(Pos2::new(rect.min.x + 24.0, rect.min.y + 17.0), egui::Align2::LEFT_CENTER, &round.title, title_font, name_c);
                        }

                        // 目标或参与说明
                        let goal_text = if !round.goal.is_empty() {
                            format!("目标: {}", round.goal)
                        } else {
                            let names: Vec<_> = round.participants.iter().filter(|p| p.id != "user").map(|p| p.name.as_str()).collect();
                            format!("参与: {}", names.join(", "))
                        };
                        let goal_font = FontId::proportional(11.0);
                        let goal_galley = cp.layout_no_wrap(goal_text.clone(), goal_font.clone(), muted(dark));
                        let goal_max_w = card_w - 24.0;
                        if goal_galley.size().x > goal_max_w {
                            let mut short_goal = goal_text;
                            while !short_goal.is_empty() && cp.layout_no_wrap(format!("{}…", short_goal), goal_font.clone(), muted(dark)).size().x > goal_max_w {
                                short_goal.pop();
                            }
                            cp.text(Pos2::new(rect.min.x + 14.0, rect.min.y + 36.0), egui::Align2::LEFT_CENTER, format!("{}…", short_goal), goal_font, muted(dark));
                        } else {
                            cp.text(Pos2::new(rect.min.x + 14.0, rect.min.y + 36.0), egui::Align2::LEFT_CENTER, goal_text, goal_font, muted(dark));
                        }

                        if resp.clicked() && !del_clicked {
                            action = Some(Action::Select(i));
                        }
                    });
                    ui.add_space(4.0);
                }
            }
        });

    // 4. 底部退出操作按钮
    let mut bottom_ui = side_ui.new_child(
        egui::UiBuilder::new()
            .id_salt("chat_sidebar_bottom_ui")
            .max_rect(bottom_rect),
    );
    bottom_ui.horizontal(|ui| {
        ui.add_space(8.0);
        let btn_w = sidebar_rect.width() - 16.0;
        let (btn_rect, btn_resp) = ui.allocate_exact_size(vec2(btn_w, 30.0), Sense::click());
        let bhf = ui.ctx().animate_bool(btn_resp.id.with("hov"), btn_resp.hovered());
        let fill = if dark {
            Color32::from_white_alpha((6.0 + 8.0 * bhf) as u8)
        } else {
            Color32::from_black_alpha((6.0 + 8.0 * bhf) as u8)
        };
        let bp = ui.painter();
        bp.rect_filled(btn_rect, 6.0, fill);
        let stroke_c = if dark { Color32::from_white_alpha(8) } else { Color32::from_black_alpha(10) };
        bp.rect_stroke(btn_rect, 6.0, egui::Stroke::new(0.5, stroke_c), egui::StrokeKind::Inside);
        let font = FontId::proportional(12.0);
        let label = "返回工作区";
        let label_g = bp.layout_no_wrap(label.to_string(), font.clone(), text_color(dark));
        let icon_w = 12.0;
        let gap = 6.0;
        let total_content_w = icon_w + gap + label_g.size().x;
        let start_x = btn_rect.center().x - total_content_w * 0.5;
        let fg = if bhf > 0.01 {
            if dark { Color32::WHITE } else { Color32::BLACK }
        } else {
            text_color(dark)
        };
        paint_reply_arrow(bp, Pos2::new(start_x + icon_w * 0.5, btn_rect.center().y), 9.0, Stroke::new(1.3, fg));
        bp.text(
            Pos2::new(start_x + icon_w + gap, btn_rect.center().y),
            egui::Align2::LEFT_CENTER,
            label,
            font,
            fg,
        );
        if btn_resp.on_hover_text("退出 Agent Chat，返回终端与工作区").clicked() {
            action = Some(Action::Close);
        }
    });

    action
}

/// 绘制单个 Agent 的一体化现代终端视口卡片（Terminal Window Card）
fn draw_terminal_card(
    ui: &mut Ui,
    state: &mut ChatState,
    round_id: &str,
    member: &crate::collab::Participant,
    card_rect: Rect,
    custom_color: [u8; 3],
) {
    let dark = ui.visuals().dark_mode;
    let p = ui.painter();
    let weak = ui.visuals().weak_text_color();

    // 1. 卡片外框与底色
    let card_bg = state.theme.background;
    let border_c = if dark { Color32::from_white_alpha(10) } else { Color32::from_black_alpha(12) };
    p.rect_filled(card_rect.translate(vec2(0.0, 1.2)), 8.0, Color32::from_black_alpha(if dark { 45 } else { 12 }));
    p.rect_filled(card_rect, 8.0, card_bg);
    p.rect_stroke(card_rect, 8.0, Stroke::new(0.6, border_c), egui::StrokeKind::Inside);

    // 2. 一体化标题栏（高度 32px）
    let titlebar_h = 32.0;
    let titlebar_rect = Rect::from_min_size(card_rect.min, vec2(card_rect.width(), titlebar_h));
    let titlebar_bg = if dark { Color32::from_white_alpha(4) } else { Color32::from_black_alpha(5) };
    p.rect_filled(titlebar_rect, 8.0, titlebar_bg);
    p.line_segment([titlebar_rect.left_bottom(), titlebar_rect.right_bottom()], Stroke::new(0.5, border_c));

    let run = state.native_runs.iter_mut().find(|r| r.round == round_id && r.agent == member.id);

    // 左侧：状态指示灯与 Agent 名称、角色微芯片
    let (dot_c, status_text, status_c) = if let Some(r) = &run {
        if r.status.contains("失败") {
            (Color32::from_rgb(239, 68, 68), r.status.clone(), Color32::from_rgb(239, 68, 68))
        } else if r.status.contains("就绪") || r.status.contains("交互") {
            (Color32::from_rgb(34, 197, 94), r.status.clone(), Color32::from_rgb(34, 197, 94))
        } else if r.status.contains("退出") {
            (Color32::from_rgb(156, 163, 175), r.status.clone(), Color32::from_rgb(156, 163, 175))
        } else {
            (Color32::from_rgb(59, 130, 246), r.status.clone(), Color32::from_rgb(59, 130, 246))
        }
    } else {
        (Color32::from_rgb(100, 116, 139), "● 未启动".to_string(), weak)
    };
    p.circle_filled(Pos2::new(titlebar_rect.min.x + 14.0, titlebar_rect.center().y), 3.5, dot_c);

    let name_font = FontId::proportional(12.5);
    p.text(
        Pos2::new(titlebar_rect.min.x + 24.0, titlebar_rect.center().y),
        egui::Align2::LEFT_CENTER,
        &member.name,
        name_font,
        if dark { Color32::WHITE } else { Color32::BLACK },
    );

    let role_font = FontId::proportional(11.0);
    let role_galley = p.layout_no_wrap(member.role.clone(), role_font.clone(), muted(dark));
    let name_w = p.layout_no_wrap(member.name.clone(), FontId::proportional(12.5), Color32::WHITE).size().x;
    let chip_x = titlebar_rect.min.x + 24.0 + name_w + 8.0;
    let chip_rect = Rect::from_center_size(
        Pos2::new(chip_x + role_galley.size().x * 0.5 + 4.0, titlebar_rect.center().y),
        vec2(role_galley.size().x + 8.0, 18.0),
    );
    p.rect_filled(chip_rect, 3.0, if dark { Color32::from_white_alpha(8) } else { Color32::from_black_alpha(8) });
    p.galley(chip_rect.center() - role_galley.size() * 0.5, role_galley, muted(dark));

    // 右侧：状态小标签
    p.text(
        Pos2::new(titlebar_rect.max.x - 12.0, titlebar_rect.center().y),
        egui::Align2::RIGHT_CENTER,
        status_text,
        FontId::proportional(11.0),
        status_c,
    );

    // 3. 视口主体
    let body_rect = Rect::from_min_max(
        Pos2::new(card_rect.min.x + 1.0, titlebar_rect.max.y + 1.0),
        Pos2::new(card_rect.max.x - 1.0, card_rect.max.y - 1.0),
    );
    let mut body_ui = ui.new_child(
        egui::UiBuilder::new()
            .id_salt(("agent_term_body", round_id, &member.id))
            .max_rect(body_rect),
    );
    body_ui.set_clip_rect(body_rect);

    if let Some(run) = run {
        crate::ui::terminal::show_embedded(&mut body_ui, &mut run.session, state.draft.is_none(), &state.theme);
    } else {
        body_ui.centered_and_justified(|ui| {
            ui.vertical_centered(|ui| {
                ui.add_space((body_rect.height() - 100.0).max(10.0) * 0.38);
                let box_w = (body_rect.width() - 32.0).clamp(160.0, 320.0);
                let (pb_rect, _) = ui.allocate_exact_size(vec2(box_w, 62.0), Sense::hover());
                let bp = ui.painter();
                let pb_bg = if dark { Color32::from_white_alpha(4) } else { Color32::from_black_alpha(4) };
                let pb_stroke = if dark { Color32::from_white_alpha(7) } else { Color32::from_black_alpha(8) };
                bp.rect_filled(pb_rect, 6.0, pb_bg);
                bp.rect_stroke(pb_rect, 6.0, Stroke::new(0.5, pb_stroke), egui::StrokeKind::Inside);
                let prompt_c = if dark {
                    Color32::from_rgba_unmultiplied(custom_color[0], custom_color[1], custom_color[2], 230)
                } else {
                    Color32::from_rgb(custom_color[0], custom_color[1], custom_color[2])
                };
                bp.text(
                    Pos2::new(pb_rect.min.x + 12.0, pb_rect.min.y + 18.0),
                    egui::Align2::LEFT_CENTER,
                    "$ 等待协同任务指令指派...",
                    FontId::monospace(12.0),
                    prompt_c,
                );
                bp.text(
                    Pos2::new(pb_rect.min.x + 12.0, pb_rect.min.y + 42.0),
                    egui::Align2::LEFT_CENTER,
                    format!("职责分工：{}", member.role),
                    FontId::proportional(11.5),
                    muted(dark),
                );

                ui.add_space(8.0);
                ui.label(RichText::new("终端就绪 · 在下方输入框下达任务将自动调起 ConPTY").small().color(weak));
            });
        });
    }
}

/// 阴刻内凹槽体流式输入条（与主界面搜索框风格一致）
fn draw_sunken_composer(
    ui: &mut Ui,
    state: &mut ChatState,
    footer_rect: Rect,
    round: &crate::collab::Round,
    custom_color: [u8; 3],
) -> Option<Action> {
    let mut action = None;
    let dark = ui.visuals().dark_mode;
    let p = ui.painter();

    // 底部控制栏底色与顶部发丝分隔线（坚固底栏，彻底杜绝下方透底与字符穿模）
    let footer_bg = if dark {
        Color32::from_rgb(16, 17, 21)
    } else {
        Color32::from_rgb(243, 244, 247)
    };
    p.rect_filled(footer_rect, 0.0, footer_bg);
    let border_c = if dark { Color32::from_white_alpha(10) } else { Color32::from_black_alpha(12) };
    p.line_segment([footer_rect.left_top(), footer_rect.right_top()], Stroke::new(0.6, border_c));

    // 阴刻内凹槽体外框（居中，42px 高度，12.0 圆角）
    let trench_margin_x = 12.0;
    let trench_h = 42.0;
    let trench_y = footer_rect.center().y - trench_h * 0.5;
    let trench_rect = Rect::from_min_max(
        Pos2::new(footer_rect.min.x + trench_margin_x, trench_y),
        Pos2::new(footer_rect.max.x - trench_margin_x, trench_y + trench_h),
    );

    // 1. 阴刻深陷槽体底色
    let sunken_bg = if dark {
        Color32::from_black_alpha(80)
    } else {
        Color32::from_black_alpha(15)
    };
    p.rect_filled(trench_rect, 12.0, sunken_bg);

    // 2. 阴刻顶部双层内阴影 (Top Inset Shadow)
    let shadow_1 = if dark { Color32::from_black_alpha(110) } else { Color32::from_black_alpha(40) };
    let shadow_2 = if dark { Color32::from_black_alpha(60) } else { Color32::from_black_alpha(20) };
    p.line_segment(
        [trench_rect.left_top() + vec2(6.0, 1.0), trench_rect.right_top() - vec2(6.0, -1.0)],
        Stroke::new(1.2, shadow_1),
    );
    p.line_segment(
        [trench_rect.left_top() + vec2(8.0, 2.2), trench_rect.right_top() - vec2(8.0, -2.2)],
        Stroke::new(1.0, shadow_2),
    );

    // 3. 阴刻底部内沿微反光 (Bottom Lip Highlight)
    let lip = if dark { Color32::from_white_alpha(12) } else { Color32::from_white_alpha(60) };
    p.line_segment(
        [trench_rect.left_bottom() + vec2(6.0, 0.0), trench_rect.right_bottom() - vec2(6.0, 0.0)],
        Stroke::new(0.6, lip),
    );

    // 4. 四周内沿暗色微边框
    let stroke_c = if dark { Color32::from_black_alpha(45) } else { Color32::from_black_alpha(12) };
    p.rect_stroke(trench_rect, 12.0, Stroke::new(0.5, stroke_c), egui::StrokeKind::Inside);

    // 内部流式交互区域
    let inner_rect = trench_rect.shrink2(vec2(6.0, 4.0));
    let mut trench_ui = ui.new_child(
        egui::UiBuilder::new()
            .id_salt("sunken_trench_inner")
            .max_rect(inner_rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    trench_ui.spacing_mut().item_spacing = vec2(6.0, 0.0);

    // 5. 左侧：对哪个 Agent 说话微卡片药丸（纯矢量下箭头，彻底杜绝 Unicode 字体缺失方框）
    let cur_target_name = round.participants.iter().find(|p| p.id == state.recipient).map(|p| p.name.as_str()).unwrap_or("首位执行");
    let pill_text = if state.reply.is_some() {
        format!("回复 {}", cur_target_name)
    } else {
        format!("对 {} 说话", cur_target_name)
    };

    let pill_font = FontId::proportional(12.0);
    let pill_galley = trench_ui.painter().layout_no_wrap(pill_text.clone(), pill_font.clone(), text_color(dark));
    let pill_w = (pill_galley.size().x + 26.0).max(78.0);
    let (pill_rect, pill_resp) = trench_ui.allocate_exact_size(vec2(pill_w, 28.0), Sense::click());

    let phov = trench_ui.ctx().animate_bool(pill_resp.id.with("hov"), pill_resp.hovered());
    let pill_bg = if dark {
        Color32::from_white_alpha((10.0 + 10.0 * phov) as u8)
    } else {
        Color32::from_black_alpha((10.0 + 8.0 * phov) as u8)
    };
    let pill_p = trench_ui.painter();
    pill_p.rect_filled(pill_rect.translate(vec2(0.0, 1.0)), 7.0, Color32::from_black_alpha(if dark { 40 } else { 10 }));
    pill_p.rect_filled(pill_rect, 7.0, pill_bg);
    let pill_stroke = if dark { Color32::from_white_alpha(12) } else { Color32::from_black_alpha(14) };
    pill_p.rect_stroke(pill_rect, 7.0, Stroke::new(0.5, pill_stroke), egui::StrokeKind::Inside);
    pill_p.text(
        Pos2::new(pill_rect.min.x + 9.0, pill_rect.center().y),
        egui::Align2::LEFT_CENTER,
        pill_text,
        pill_font,
        text_color(dark),
    );
    // 纯矢量下尖角符号
    let chevron_pos = Pos2::new(pill_rect.max.x - 9.0, pill_rect.center().y);
    let chevron_fg = if dark { Color32::from_gray(160) } else { Color32::from_gray(100) };
    paint_chevron_down(pill_p, chevron_pos, 6.0, Stroke::new(1.2, chevron_fg));

    let popup_id = trench_ui.make_persistent_id("agent_selector_popup");
    let mut is_popup_open = trench_ui.data(|d| d.get_temp::<bool>(popup_id).unwrap_or(false));
    if pill_resp.clicked() {
        is_popup_open = !is_popup_open;
        trench_ui.data_mut(|d| d.insert_temp(popup_id, is_popup_open));
    }

    if state.reply.is_some() && vector_tool_button(&mut trench_ui, ToolIcon::Close, "取消回复指定消息") {
        state.reply = None;
    }

    // 浮层菜单
    if is_popup_open {
        let members: Vec<_> = round.participants.iter().filter(|p| p.id != "user").collect();
        let item_h = 28.0;
        let popup_h = members.len() as f32 * item_h + 8.0;
        let popup_pos = pill_rect.left_top() - vec2(0.0, popup_h + 4.0);

        egui::Area::new(popup_id)
            .order(egui::Order::Foreground)
            .fixed_pos(popup_pos)
            .show(trench_ui.ctx(), |ui| {
                let frame_bg = if dark { Color32::from_rgb(26, 26, 32) } else { Color32::from_rgb(245, 245, 248) };
                let frame_stroke = Stroke::new(0.5, if dark { Color32::from_white_alpha(15) } else { Color32::from_black_alpha(15) });
                egui::Frame::NONE
                    .fill(frame_bg)
                    .stroke(frame_stroke)
                    .corner_radius(8)
                    .inner_margin(4.0)
                    .shadow(egui::epaint::Shadow { offset: [0, 6], blur: 14, spread: 0, color: Color32::from_black_alpha(90) })
                    .show(ui, |ui| {
                        let popup_w = 160.0;
                        ui.set_width(popup_w);
                        for p in &members {
                            let is_curr = state.recipient == p.id;
                            let (item_rect, item_resp) = ui.allocate_exact_size(vec2(popup_w, item_h), Sense::click());
                            let item_hf = ui.ctx().animate_bool(Id::new(("chat_agent_item_h", &p.id)), item_resp.hovered());
                            let item_bg = if is_curr {
                                if dark {
                                    Color32::from_rgba_unmultiplied(custom_color[0], custom_color[1], custom_color[2], 50)
                                } else {
                                    Color32::from_rgba_unmultiplied(custom_color[0], custom_color[1], custom_color[2], 35)
                                }
                            } else {
                                lerp_color(Color32::TRANSPARENT, if dark { Color32::from_white_alpha(8) } else { Color32::from_black_alpha(8) }, item_hf)
                            };
                            ui.painter().rect_filled(item_rect, 5.0, item_bg);
                            let label = format!("{} ({})", p.name, p.role);
                            let fg = if is_curr {
                                if dark { Color32::WHITE } else { Color32::BLACK }
                            } else {
                                if dark { Color32::from_gray(190) } else { Color32::from_gray(70) }
                            };
                            ui.painter().text(
                                Pos2::new(item_rect.min.x + 8.0, item_rect.center().y),
                                egui::Align2::LEFT_CENTER,
                                label,
                                FontId::proportional(12.0),
                                fg,
                            );
                            if item_resp.clicked() {
                                state.recipient = p.id.clone();
                                ui.data_mut(|d| d.insert_temp(popup_id, false));
                            }
                        }
                    });
            });

        if trench_ui.ctx().input(|i| i.pointer.any_click()) && !pill_resp.clicked() {
            trench_ui.data_mut(|d| d.insert_temp(popup_id, false));
        }
    }

    // 6. 右侧按键与状态预留宽度
    let right_reserved_w = if state.is_running() { 160.0 } else { 42.0 };
    let edit_w = (trench_ui.available_width() - right_reserved_w).max(40.0);

    // 7. 中间：无边框流式极简输入框（回车发送，Shift+Enter 换行）
    let edit_id = trench_ui.make_persistent_id("chat_composer_body_input");
    let has_focus = trench_ui.memory(|m| m.has_focus(edit_id));
    let shift_down = trench_ui.input(|i| i.modifiers.shift);
    let enter_pressed = if has_focus && !shift_down {
        trench_ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter))
    } else {
        false
    };

    let text_edit = egui::TextEdit::multiline(&mut state.body)
        .id(edit_id)
        .desired_rows(1)
        .desired_width(edit_w)
        .font(FontId::proportional(13.0))
        .hint_text("输入任务或指令… (Enter 发送，Shift+Enter 换行)")
        .frame(egui::Frame::NONE)
        .margin(vec2(4.0, 2.0));
    trench_ui.add(text_edit);

    if enter_pressed {
        while state.body.ends_with('\n') || state.body.ends_with('\r') {
            state.body.pop();
        }
        if !state.is_running() && !state.body.trim().is_empty() {
            action = Some(Action::Send);
        }
    }

    // 8. 右侧操作按键（运行中终止 / 就绪发送）
    if state.is_running() {
        trench_ui.add(
            egui::Label::new(
                RichText::new(&state.execution_status)
                    .small()
                    .color(muted(dark)),
            )
            .truncate(),
        ).on_hover_text(&state.execution_status);

        let (c_rect, c_resp) = trench_ui.allocate_exact_size(vec2(28.0, 28.0), Sense::click());
        let c_hov = trench_ui.ctx().animate_bool(c_resp.id.with("hov"), c_resp.hovered());
        let c_bg = Color32::from_rgba_unmultiplied(220, 80, 70, (40.0 + 30.0 * c_hov) as u8);
        let cp = trench_ui.painter();
        cp.rect_filled(c_rect, 14.0, c_bg);
        paint_stop_square(cp, c_rect.center(), 8.0, Color32::from_rgb(240, 100, 90));
        if c_resp.on_hover_text("取消协作任务").clicked() {
            action = Some(Action::Cancel);
        }
    } else {
        let can_send = !state.body.trim().is_empty();
        let (s_rect, s_resp) = trench_ui.allocate_exact_size(vec2(28.0, 28.0), if can_send { Sense::click() } else { Sense::hover() });
        let s_hov = trench_ui.ctx().animate_bool(s_resp.id.with("hov"), s_resp.hovered() && can_send);
        let sp = trench_ui.painter();

        let s_bg = if can_send {
            if dark {
                Color32::from_rgba_unmultiplied(custom_color[0], custom_color[1], custom_color[2], (180.0 + 60.0 * s_hov) as u8)
            } else {
                Color32::from_rgba_unmultiplied(custom_color[0], custom_color[1], custom_color[2], (200.0 + 55.0 * s_hov) as u8)
            }
        } else if dark {
            Color32::from_white_alpha(10)
        } else {
            Color32::from_black_alpha(8)
        };
        sp.rect_filled(s_rect.translate(vec2(0.0, 1.0)), 14.0, Color32::from_black_alpha(if dark { 40 } else { 10 }));
        sp.rect_filled(s_rect, 14.0, s_bg);
        let s_stroke_c = if can_send {
            Color32::TRANSPARENT
        } else if dark {
            Color32::from_white_alpha(12)
        } else {
            Color32::from_black_alpha(12)
        };
        if s_stroke_c != Color32::TRANSPARENT {
            sp.rect_stroke(s_rect, 14.0, Stroke::new(0.5, s_stroke_c), egui::StrokeKind::Inside);
        }
        let s_fg = if can_send {
            Color32::WHITE
        } else if dark {
            Color32::from_gray(140)
        } else {
            Color32::from_gray(120)
        };
        paint_send_arrow(sp, s_rect.center(), 10.0, Stroke::new(1.4, s_fg));
        if can_send && s_resp.on_hover_text("发送任务 (Enter)").clicked() {
            action = Some(Action::Send);
        }
    }

    action
}

pub fn show(ui: &mut Ui, state: &mut ChatState, theme: &crate::config::ThemeSettings) -> Option<Action> {
    let mut action = None;
    let dark = ui.visuals().dark_mode;
    let custom_color = theme.sidebar_card_color.unwrap_or([0, 111, 238]);
    ui.spacing_mut().item_spacing = vec2(8.0, 6.0);
    ui.spacing_mut().button_padding = vec2(10.0, 6.0);

    let bounds = ui.available_rect_before_wrap();
    // 计算项目侧边栏宽度与主绘图区域
    let sidebar_w = if state.sidebar_expanded {
        let max_sidebar = (bounds.width() * 0.32).min(240.0);
        let min_sidebar = 130.0_f32.min(max_sidebar);
        if max_sidebar > min_sidebar {
            (bounds.width() * 0.22).clamp(min_sidebar, max_sidebar)
        } else {
            max_sidebar
        }
    } else {
        0.0
    };

    let sidebar_rect = Rect::from_min_size(bounds.min, vec2(sidebar_w, bounds.height()));
    let main_rect = Rect::from_min_max(
        Pos2::new(bounds.min.x + sidebar_w, bounds.min.y),
        bounds.max,
    );

    // 渲染左侧项目侧边栏
    if state.sidebar_expanded {
        if let Some(act) = draw_chat_sidebar(ui, state, sidebar_rect, custom_color) {
            action = Some(act);
        }
    }

    // 主视图区域
    if state.rounds.is_empty() {
        let mut empty_ui = ui.new_child(egui::UiBuilder::new().id_salt("chat_empty_view").max_rect(main_rect));
        empty_ui.horizontal(|ui| {
            if !state.sidebar_expanded {
                if icon_pill_button(ui, Some(ToolIcon::Sidebar), "项目", "展开项目侧边栏", false, Some(custom_color)) {
                    state.sidebar_expanded = true;
                }
                if icon_pill_button(ui, Some(ToolIcon::Reply), "返回工作区", "返回普通终端", false, None) {
                    action = Some(Action::Close);
                }
            }
            if vector_tool_button(ui, ToolIcon::Plus, "新建工作流") {
                action = Some(Action::New);
            }
        });
        empty_ui.centered_and_justified(|ui| {
            ui.label("还没有工作流项目，点击侧边栏或右上角「＋」创建新项目");
        });
        ui.allocate_rect(bounds, Sense::hover());
        return action;
    }

    let active = state.active.unwrap_or(0).min(state.rounds.len().saturating_sub(1));
    let round = state.rounds[active].1.clone();

    // 确保有合法的首选对话角色
    if state.recipient.is_empty() || !round.participants.iter().any(|p| p.id == state.recipient) {
        if let Some(first_p) = round.participants.iter().find(|p| p.id != "user") {
            state.recipient = first_p.id.clone();
        }
    }

    let header_h = 42.0;
    let footer_h = 58.0;

    let header_rect = Rect::from_min_size(main_rect.min, vec2(main_rect.width(), header_h));
    let footer_rect = Rect::from_min_max(
        Pos2::new(main_rect.min.x, main_rect.max.y - footer_h),
        main_rect.max,
    );
    let content_rect = Rect::from_min_max(
        Pos2::new(main_rect.min.x, header_rect.max.y),
        Pos2::new(main_rect.max.x, footer_rect.min.y),
    );

    // 1. 顶部操作栏（包含项目标题、阶段徽章、工作目录、目标提示、角色标签及协作消息展开按键）
    let mut header_ui = ui.new_child(egui::UiBuilder::new().id_salt("chat_main_header").max_rect(header_rect));
    header_ui.spacing_mut().item_spacing = vec2(8.0, 0.0);
    header_ui.horizontal(|ui| {
        ui.add_space(8.0);
        if !state.sidebar_expanded {
            if icon_pill_button(ui, Some(ToolIcon::Sidebar), "项目", "展开项目侧边栏", false, Some(custom_color)) {
                state.sidebar_expanded = true;
            }
            if icon_pill_button(ui, Some(ToolIcon::Reply), "返回", "返回普通终端与工作区", false, None) {
                action = Some(Action::Close);
            }
        }

        // 项目名称
        ui.label(RichText::new(&round.title).size(15.5).strong());

        // 阶段胶囊徽章
        draw_phase_badge(ui, &round.phase);

        // 工作目录提示芯片
        let short_repo = crate::ui::sidebar::shorten_path(&round.repo, 20);
        chip_badge(ui, &format!("📁 {}", short_repo)).on_hover_text(format!("工作目录：\n{}", round.repo.display()));

        // 目标与约束提示芯片
        if !round.goal.is_empty() {
            let goal_chip = if round.goal.chars().count() > 14 {
                format!("目标: {}…", round.goal.chars().take(14).collect::<String>())
            } else {
                format!("目标: {}", round.goal)
            };
            chip_badge(ui, &goal_chip).on_hover_text(format!("项目目标与约束：\n{}", round.goal));
        }

        // 参与角色芯片
        for p in round.participants.iter().filter(|p| p.id != "user") {
            chip_badge(ui, &format!("{} · {}", p.name, p.role));
        }

        // 右侧操作项：展开消息抽屉与新建
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(8.0);
            if !state.sidebar_expanded && vector_tool_button(ui, ToolIcon::Plus, "新建工作流") {
                action = Some(Action::New);
            }
            let msg_text = if state.messages.is_empty() {
                "消息".to_string()
            } else {
                format!("消息 ({})", state.messages.len())
            };
            if icon_pill_button(ui, Some(ToolIcon::Message), &msg_text, if state.chat_expanded { "收起协作消息" } else { "展开协作消息" }, state.chat_expanded, Some(custom_color)) {
                state.chat_expanded = !state.chat_expanded;
            }
        });
    });

    let h_divider_c = if dark { Color32::from_white_alpha(8) } else { Color32::from_black_alpha(10) };
    ui.painter().line_segment([header_rect.left_bottom(), header_rect.right_bottom()], Stroke::new(0.5, h_divider_c));

    if let Some(error) = &state.error {
        let mut err_ui = ui.new_child(egui::UiBuilder::new().id_salt("chat_error_banner").max_rect(content_rect));
        err_ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.add(egui::Label::new(RichText::new(format!("⚠️ {}", error)).color(Color32::from_rgb(220, 100, 90))).truncate()).on_hover_text(error);
        });
    }

    // 2. 主体区域：拉长多终端卡片（Terminal Window Card），右侧可收放协作消息面板
    let (term_rect, drawer_rect) = if state.chat_expanded {
        let max_drawer = (content_rect.width() * 0.45).min(380.0);
        let min_drawer = 100.0_f32.min(max_drawer);
        let drawer_w = if max_drawer > min_drawer {
            (content_rect.width() * 0.36).clamp(min_drawer, max_drawer)
        } else {
            max_drawer
        };
        let term_w = (content_rect.width() - drawer_w - 6.0).max(60.0);
        (
            Rect::from_min_size(content_rect.min, vec2(term_w, content_rect.height())),
            Some(Rect::from_min_size(
                Pos2::new(content_rect.min.x + term_w + 6.0, content_rect.min.y),
                vec2(drawer_w, content_rect.height()),
            )),
        )
    } else {
        (content_rect, None)
    };

    let card_pad_x = 10.0;
    let card_pad_y = 10.0;
    let cards_area = term_rect.shrink2(vec2(card_pad_x, card_pad_y));

    let members: Vec<_> = round.participants.iter().filter(|p| p.id != "user").collect();
    let mut terminal_ui = ui.new_child(
        egui::UiBuilder::new()
            .id_salt(("agents", &round.id))
            .max_rect(cards_area),
    );
    terminal_ui.set_clip_rect(cards_area);

    let n = members.len().max(1);
    egui::ScrollArea::horizontal()
        .id_salt(("terminal_columns", &round.id))
        .auto_shrink([false, false])
        .show(&mut terminal_ui, |ui| {
            ui.horizontal(|ui| {
                let col_spacing = 10.0;
                let card_h = cards_area.height();
                let width = ((cards_area.width() - col_spacing * (n - 1) as f32) / n as f32).max(320.0);
                for member in &members {
                    let (card_rect, _) = ui.allocate_exact_size(vec2(width, card_h), Sense::hover());
                    draw_terminal_card(ui, state, &round.id, member, card_rect, custom_color);
                }
            });
        });

    if let Some(d_rect) = drawer_rect {
        let d_area = d_rect.shrink2(vec2(0.0, 10.0));
        let border = if dark { Color32::from_white_alpha(12) } else { Color32::from_black_alpha(14) };
        let fill = if dark { Color32::from_rgb(22, 23, 27) } else { Color32::from_rgb(246, 247, 249) };
        let mut drawer_ui = ui.new_child(egui::UiBuilder::new().id_salt("conversation_drawer").max_rect(d_area));
        drawer_ui.set_clip_rect(d_area);
        egui::Frame::NONE
            .fill(fill)
            .corner_radius(8)
            .stroke(Stroke::new(0.6, border))
            .inner_margin(egui::Margin::symmetric(10, 8))
            .show(&mut drawer_ui, |ui| {
                ui.horizontal(|ui| {
                    ui.strong(format!("协作消息 ({})", state.messages.len()));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if vector_tool_button(ui, ToolIcon::Close, "收起协作消息抽屉") {
                            state.chat_expanded = false;
                        }
                    });
                });
                ui.separator();
                let running = state.is_running();
                egui::ScrollArea::vertical()
                    .id_salt(("messages", &round.id))
                    .auto_shrink([false, false])
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        if state.messages.is_empty() {
                            ui.centered_and_justified(|ui| {
                                ui.label(RichText::new("暂无协作消息").small().color(muted(dark)));
                            });
                        }
                        for m in &state.messages {
                            let name = |id: &str| {
                                if id == "user" {
                                    "我".to_owned()
                                } else {
                                    round.participants.iter().find(|p| p.id == id).map(|p| p.name.clone()).unwrap_or_else(|| id.into())
                                }
                            };
                            ui.push_id(&m.id, |ui| {
                                ui.horizontal_wrapped(|ui| {
                                    ui.strong(format!("{} → {}", name(&m.from), name(&m.to)));
                                    if vector_tool_button(ui, ToolIcon::Reply, "回复此消息") {
                                        state.reply = Some(m.id.clone());
                                        state.recipient = if m.from == "user" { m.to.clone() } else { m.from.clone() };
                                    }
                                    if m.from != "user" && m.to == "user" {
                                        ui.add_enabled_ui(!running, |ui| {
                                            if vector_tool_button(ui, ToolIcon::ArrowRight, "交给其他 AI") { action = Some(Action::Handoff(m.id.clone())); }
                                        });
                                    }
                                });
                                let text = m.body.lines().filter(|line| line.trim() != "[WORKFLOW_COMPLETE]").collect::<Vec<_>>().join("\n");
                                ui.add(egui::Label::new(text).wrap().selectable(true));
                                ui.separator();
                            });
                        }
                    });
            });
    }

    // 3. 底部阴刻内凹槽体流式输入条
    if let Some(act) = draw_sunken_composer(ui, state, footer_rect, &round, custom_color) {
        action = Some(act);
    }

    ui.allocate_rect(bounds, Sense::hover());
    action
}

pub fn begin_draft(sessions: &[Session], selected: usize) -> Draft {
    Draft {
        title: String::new(),
        goal: String::new(),
        repo: sessions
            .get(selected)
            .map(|s| s.cwd.display().to_string())
            .unwrap_or_default(),
        members: sessions
            .iter()
            .map(|s| MemberDraft {
                id: format!("workspace-{}", s.id),
                name: s.name.clone(),
                command: s.command.clone(),
                cwd: s.cwd.clone(),
                selected: false,
                role: "开发".into(),
            })
            .collect(),
    }
}

fn paint_check(p: &egui::Painter, center: Pos2, size: f32, stroke: Stroke) {
    let s = size * 0.5;
    let p1 = Pos2::new(center.x - s * 0.7, center.y);
    let p2 = Pos2::new(center.x - s * 0.1, center.y + s * 0.6);
    let p3 = Pos2::new(center.x + s * 0.8, center.y - s * 0.6);
    p.line_segment([p1, p2], stroke);
    p.line_segment([p2, p3], stroke);
}

fn draw_modal_input(
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

    let sunken_bg = if dark {
        Color32::from_black_alpha(80)
    } else {
        Color32::from_black_alpha(15)
    };
    ui.painter().rect_filled(rect, 8.0, sunken_bg);

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

    let bottom_lip_color = if dark {
        lerp_color(Color32::from_white_alpha(8), Color32::from_white_alpha(16), hf)
    } else {
        lerp_color(Color32::from_white_alpha(50), Color32::from_white_alpha(90), hf)
    };
    ui.painter().line_segment(
        [rect.left_bottom() + vec2(4.0, 0.0), rect.right_bottom() - vec2(4.0, 0.0)],
        Stroke::new(0.6, bottom_lip_color),
    );

    let stroke_c = if dark { Color32::from_black_alpha(45) } else { Color32::from_black_alpha(12) };
    ui.painter().rect_stroke(rect, 8.0, Stroke::new(0.5, stroke_c), egui::StrokeKind::Inside);

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

fn draw_modal_button(
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
    let hf = ui.ctx().animate_bool(Id::new(("mbtn_hov", label, is_primary)), hovered);

    if enabled {
        let s1 = if dark { Color32::from_black_alpha(30) } else { Color32::from_black_alpha(8) };
        let s2 = if dark { Color32::from_black_alpha(55) } else { Color32::from_black_alpha(12) };
        ui.painter().rect_filled(rect.translate(vec2(0.0, 2.0)), 8.0, s1);
        ui.painter().rect_filled(rect.translate(vec2(0.0, 1.0)), 8.0, s2);
    }

    let bg = if is_primary {
        let base = if enabled {
            Color32::from_rgba_unmultiplied(custom_color[0], custom_color[1], custom_color[2], if dark { 65 } else { 50 })
        } else {
            Color32::from_rgba_unmultiplied(custom_color[0], custom_color[1], custom_color[2], if dark { 20 } else { 15 })
        };
        let hover = Color32::from_rgba_unmultiplied(custom_color[0], custom_color[1], custom_color[2], if dark { 95 } else { 75 });
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
        egui::Align2::CENTER_CENTER,
        label,
        FontId::new(12.5, egui::FontFamily::Proportional),
        text_c,
    );

    resp.clicked() && enabled
}

pub fn new_round_modal(ui: &mut Ui, state: &mut ChatState, theme: &crate::config::ThemeSettings) {
    let Some(draft) = &mut state.draft else {
        return;
    };
    let mut create = false;
    let mut cancel = false;
    let screen_rect = ui.ctx().input(|i| i.raw.screen_rect).unwrap_or_else(|| ui.max_rect());
    let dark = ui.visuals().dark_mode;
    let custom_color = theme.sidebar_card_color.unwrap_or([0, 111, 238]);

    ui.painter().rect_filled(screen_rect, 0.0, Color32::from_black_alpha(if dark { 160 } else { 75 }));

    egui::Area::new(Id::new("new_workflow_modal"))
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ui.ctx(), |ui| {
            let modal_w = 420.0;
            let modal_bg = if dark { Color32::from_rgb(18, 18, 22) } else { Color32::from_rgb(250, 250, 252) };
            let modal_border = if dark { Color32::from_white_alpha(8) } else { Color32::from_black_alpha(10) };
            let modal_shadow = Color32::from_black_alpha(if dark { 140 } else { 30 });

            let frame = egui::Frame::NONE
                .fill(modal_bg)
                .stroke(Stroke::new(0.5, modal_border))
                .corner_radius(12)
                .inner_margin(22.0)
                .shadow(egui::epaint::Shadow { offset: [0, 16], blur: 32, spread: 0, color: modal_shadow });

            frame.show(ui, |ui| {
                ui.set_width(modal_w);

                // ---- 1. 顶部标题栏 ----
                ui.horizontal(|ui| {
                    ui.label(RichText::new("NEW WORKFLOW").size(11.0).color(if dark { Color32::from_gray(130) } else { Color32::from_gray(120) }).strong());
                    ui.label(RichText::new("新建工作流").size(13.0).color(if dark { Color32::from_gray(200) } else { Color32::from_gray(60) }));

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let (rect, resp) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::click());
                        let hf = ui.ctx().animate_bool(Id::new("workflow_modal_close_h"), resp.hovered());
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
                            cancel = true;
                        }
                    });
                });

                ui.add_space(14.0);

                // ---- 2. 工作流名称 ----
                ui.label(RichText::new("工作流名称").size(12.0).color(if dark { Color32::from_gray(150) } else { Color32::from_gray(100) }));
                ui.add_space(4.0);
                draw_modal_input(ui, dark, modal_w, 34.0, &mut draft.title, "例如: 功能开发与代码审查 / 性能优化", Id::new("workflow_input_title"));

                ui.add_space(14.0);

                // ---- 3. 选择协同 Agent ----
                ui.horizontal(|ui| {
                    ui.label(RichText::new("选择协同 Agent").size(12.0).color(if dark { Color32::from_gray(150) } else { Color32::from_gray(100) }));
                    let selected_count = draft.members.iter().filter(|m| m.selected).count();
                    if selected_count > 0 {
                        ui.label(RichText::new(format!("(已选 {})", selected_count)).size(11.0).color(Color32::from_rgba_unmultiplied(custom_color[0], custom_color[1], custom_color[2], 220)));
                    }
                });
                ui.add_space(6.0);

                let card_bg_idle = if dark { Color32::from_white_alpha(4) } else { Color32::from_black_alpha(6) };
                let card_bg_active = if dark {
                    Color32::from_rgba_unmultiplied(custom_color[0], custom_color[1], custom_color[2], 24)
                } else {
                    Color32::from_rgba_unmultiplied(custom_color[0], custom_color[1], custom_color[2], 20)
                };
                let accent_col = Color32::from_rgb(custom_color[0], custom_color[1], custom_color[2]);

                egui::ScrollArea::vertical()
                    .id_salt("modal_agents_scroll")
                    .max_height(190.0)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing = vec2(0.0, 6.0);
                        for m in &mut draft.members {
                            let supported = crate::collab::runner::detect(&m.command).is_ok();
                            let (rect, resp) = ui.allocate_exact_size(vec2(modal_w, 42.0), if supported { Sense::click() } else { Sense::hover() });

                            if supported && resp.clicked() {
                                m.selected = !m.selected;
                            }

                            let hf = ui.ctx().animate_bool(Id::new(("agent_card_h", &m.id)), resp.hovered() && supported);
                            let bg = if !supported {
                                if dark { Color32::from_white_alpha(2) } else { Color32::from_black_alpha(3) }
                            } else if m.selected {
                                card_bg_active
                            } else {
                                lerp_color(card_bg_idle, if dark { Color32::from_white_alpha(8) } else { Color32::from_black_alpha(10) }, hf)
                            };

                            let border_color = if m.selected {
                                Color32::from_rgba_unmultiplied(custom_color[0], custom_color[1], custom_color[2], 120)
                            } else {
                                if dark { Color32::from_white_alpha(6) } else { Color32::from_black_alpha(8) }
                            };

                            ui.painter().rect_filled(rect, 8.0, bg);
                            ui.painter().rect_stroke(rect, 8.0, Stroke::new(0.6, border_color), egui::StrokeKind::Inside);

                            // 左侧选择框
                            let chk_center = Pos2::new(rect.min.x + 18.0, rect.center().y);
                            let chk_box = Rect::from_center_size(chk_center, vec2(16.0, 16.0));
                            if m.selected {
                                ui.painter().rect_filled(chk_box, 4.0, accent_col);
                                paint_check(ui.painter(), chk_center, 12.0, Stroke::new(1.8, Color32::WHITE));
                            } else {
                                let chk_border = if supported {
                                    if dark { Color32::from_white_alpha(35) } else { Color32::from_black_alpha(40) }
                                } else {
                                    if dark { Color32::from_white_alpha(15) } else { Color32::from_black_alpha(18) }
                                };
                                ui.painter().rect_stroke(chk_box, 4.0, Stroke::new(1.0, chk_border), egui::StrokeKind::Inside);
                            }

                            // Agent 名称与命令
                            let text_x = rect.min.x + 36.0;
                            let text_y = rect.center().y;
                            let name_c = if !supported {
                                if dark { Color32::from_gray(100) } else { Color32::from_gray(160) }
                            } else if m.selected {
                                if dark { Color32::WHITE } else { Color32::BLACK }
                            } else {
                                if dark { Color32::from_gray(200) } else { Color32::from_gray(60) }
                            };
                            ui.painter().text(
                                Pos2::new(text_x, text_y - 6.0),
                                egui::Align2::LEFT_CENTER,
                                &m.name,
                                FontId::proportional(12.5),
                                name_c,
                            );
                            let cmd_c = if dark { Color32::from_gray(120) } else { Color32::from_gray(140) };
                            ui.painter().text(
                                Pos2::new(text_x, text_y + 8.0),
                                egui::Align2::LEFT_CENTER,
                                &m.command,
                                FontId::proportional(10.5),
                                cmd_c,
                            );

                            // 右侧：未适配提示或角色胶囊
                            if !supported {
                                let chip_c = if dark { Color32::from_gray(100) } else { Color32::from_gray(150) };
                                ui.painter().text(
                                    Pos2::new(rect.max.x - 14.0, text_y),
                                    egui::Align2::RIGHT_CENTER,
                                    "暂未适配",
                                    FontId::proportional(11.0),
                                    chip_c,
                                );
                            } else if m.selected {
                                let role_w = 68.0;
                                let role_h = 22.0;
                                let role_rect = Rect::from_min_max(
                                    Pos2::new(rect.max.x - 12.0 - role_w, text_y - role_h * 0.5),
                                    Pos2::new(rect.max.x - 12.0, text_y + role_h * 0.5),
                                );
                                let role_resp = ui.allocate_rect(role_rect, Sense::click());
                                let role_hf = ui.ctx().animate_bool(Id::new(("role_pill_h", &m.id)), role_resp.hovered());
                                let role_bg = lerp_color(
                                    Color32::from_rgba_unmultiplied(custom_color[0], custom_color[1], custom_color[2], if dark { 35 } else { 25 }),
                                    Color32::from_rgba_unmultiplied(custom_color[0], custom_color[1], custom_color[2], if dark { 65 } else { 45 }),
                                    role_hf,
                                );
                                ui.painter().rect_filled(role_rect, 11.0, role_bg);
                                ui.painter().text(
                                    role_rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    &m.role,
                                    FontId::proportional(11.0),
                                    if dark { Color32::WHITE } else { Color32::BLACK },
                                );
                                if role_resp.clicked() {
                                    m.role = match m.role.as_str() {
                                        "主开发" | "开发" => "代码审查".into(),
                                        "代码审查" => "方案设计".into(),
                                        "方案设计" => "测试验证".into(),
                                        _ => "主开发".into(),
                                    };
                                }
                            }
                        }
                    });

                if let Some(error) = &state.error {
                    ui.add_space(4.0);
                    ui.colored_label(Color32::from_rgb(230, 95, 85), error);
                }

                ui.add_space(18.0);

                // ---- 4. 底部操作按键 ----
                let can_create = !draft.title.trim().is_empty() && draft.members.iter().any(|m| m.selected);

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;

                    if draw_modal_button(ui, dark, custom_color, 96.0, 34.0, "创建工作流", true, can_create) && can_create {
                        create = true;
                    }

                    if draw_modal_button(ui, dark, custom_color, 78.0, 34.0, "取消", false, true) {
                        cancel = true;
                    }
                });
            });
        });

    if cancel {
        state.draft = None;
        state.error = None;
    }
    if create {
        state.error = state.create().err().map(|e| e.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collab::Store;

    #[test]
    fn chat_and_creation_dialog_render_at_supported_window_sizes() {
        let root = std::env::temp_dir().join(Store::new_id("chat-layout"));
        let mut state = ChatState::new(root.clone());
        state.native_mode = false;
        let sessions = vec![
            Session::new(3, "Codex", "codex", root.clone()),
            Session::new(8, "Claude Code", "claude", root.clone()),
            Session::new(10, "Antigravity", "agy", root.clone()),
            Session::new(11, "Opencode", "opencode", root.clone()),
            Session::new(12, "Oh My Pi", "omp", root.clone()),
            Session::new(13, "Mimo", "mimo", root.clone()),
        ];
        let mut draft = begin_draft(&sessions, 0);
        draft.title = "Development and review".into();
        draft.goal = "Review changes".into();
        draft.members[0].selected = true;
        draft.members[1].selected = true;
        state.draft = Some(draft);
        state.create().unwrap();
        state.body = "测试消息：请审查当前工作流的修改。".repeat(4);
        state.send().unwrap();
        state.execution_status = "工作流 · Codex 正在执行".into();
        for p in state.rounds[0].1.participants.iter().filter(|p| p.id != "user") {
            state.native_runs.push(crate::collab::native::NativeRun::preview(&state.rounds[0].1.id, &p.id, &p.name));
        }
        for (w, h) in [(760.0, 480.0), (1120.0, 720.0), (1600.0, 900.0)] {
            let ctx = egui::Context::default();
            ctx.set_visuals(crate::fonts::app_visuals(true));
            crate::fonts::setup_fonts(&ctx);
            for pass in 0..4 {
                state.chat_expanded = pass % 2 == 0;
                let input = egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, vec2(w, h))),
                    ..Default::default()
                };
                let output = ctx.run_ui(input, |ui| {
                    egui::Panel::left("test_sidebar")
                        .exact_size(232.0)
                        .show(ui, |_| {});
                    egui::CentralPanel::default_margins().show(ui, |ui| {
                        show(ui, &mut state, &crate::config::ThemeSettings::default());
                        assert!(ui.min_rect().max.x <= w + 1.0, "horizontal overflow at {w}");
                        assert!(
                            ui.min_rect().max.y <= h + 1.0,
                            "vertical overflow at {h}: {:?}",
                            ui.min_rect()
                        );
                    });
                });
                assert!(!output.shapes.is_empty());
            }
            state.draft = Some(begin_draft(&sessions, 0));
            let output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, vec2(w, h))),
                    ..Default::default()
                },
                |ui| new_round_modal(ui, &mut state, &crate::config::ThemeSettings::default()),
            );
            assert!(!output.shapes.is_empty());
            state.draft = None;
        }
        for run in &mut state.native_runs {
            run.stop();
        }
        let _ = std::fs::remove_dir_all(root);
    }
}
