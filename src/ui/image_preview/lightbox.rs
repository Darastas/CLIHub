//! 全屏暗黑沉浸式 Lightbox 大图查看器组件。

use egui::{
    Align2, Area, Color32, CornerRadius, FontId, Id, Image, Key, Order, Pos2, Rect, Sense, Stroke,
    Ui, Vec2, vec2,
};

use super::loader::ensure_full_image_loaded;
use super::state::ImagePreviewState;

/// 渲染全屏 Lightbox 模态大图查看器
pub fn show_lightbox_modal(ui: &mut Ui, state: &mut ImagePreviewState) {
    let Some(active_id) = state.active_preview_id else {
        return;
    };

    let ctx = ui.ctx().clone();
    let screen_rect = ctx.input(|i| i.raw.screen_rect).unwrap_or_else(|| ui.max_rect());

    // 键盘快捷键监听：Esc 退出，左右方向键切换
    let key_esc = ctx.input(|i| i.key_pressed(Key::Escape));
    let key_left = ctx.input(|i| i.key_pressed(Key::ArrowLeft));
    let key_right = ctx.input(|i| i.key_pressed(Key::ArrowRight));

    if key_esc {
        state.close_preview();
        return;
    }
    if key_left {
        state.navigate_preview(false);
    }
    if key_right {
        state.navigate_preview(true);
    }

    let Some(att) = state.attachments.iter_mut().find(|a| a.id == active_id) else {
        state.close_preview();
        return;
    };

    ensure_full_image_loaded(&ctx, att);

    let att_name = att.file_name.clone();
    let att_path = att.path.clone();
    let att_dim = att.dimensions;
    let att_size_str = att.file_size_str.clone();
    let full_tex = att.full_image.clone();

    let mut request_close = false;
    let mut request_copy = false;
    let mut request_external_open = false;
    let mut request_prev = false;
    let mut request_next = false;

    Area::new(Id::new("lightbox_modal_layer"))
        .order(Order::Tooltip)
        .fixed_pos(screen_rect.min)
        .show(&ctx, |ui| {
            // 深邃暗黑毛玻璃整体背景遮罩
            ui.painter().rect_filled(screen_rect, 0.0, Color32::from_black_alpha(215));

            // 背景点击检测（点击空白处关闭）
            let bg_resp = ui.allocate_rect(screen_rect, Sense::click_and_drag());
            if bg_resp.clicked() {
                request_close = true;
            }

            // 鼠标滚轮缩放
            let scroll_delta = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll_delta != 0.0 {
                let zoom_factor = if scroll_delta > 0.0 { 1.15 } else { 0.85 };
                state.zoom = (state.zoom * zoom_factor).clamp(0.2, 8.0);
            }

            // 鼠标按住拖动画布平移
            if bg_resp.dragged() {
                state.pan += bg_resp.drag_delta();
            }
            if bg_resp.double_clicked() {
                state.zoom = 1.0;
                state.pan = Vec2::ZERO;
            }

            // 居中大图渲染
            if let Some(tex) = &full_tex {
                let tex_size = tex.size_vec2();
                // 计算窗口自适应缩放基础尺寸（预留上下工具栏空间）
                let max_w = screen_rect.width() * 0.85;
                let max_h = screen_rect.height() * 0.80;
                let scale = (max_w / tex_size.x).min(max_h / tex_size.y).min(1.0);

                let base_size = tex_size * scale;
                let draw_size = base_size * state.zoom;
                let center_pos = screen_rect.center() + state.pan;
                let img_rect = Rect::from_center_size(center_pos, draw_size);

                // 绘制大图本体与细腻阴影
                ui.painter().rect_filled(img_rect, 6.0, Color32::BLACK);
                let img_widget = Image::from_texture(tex).corner_radius(CornerRadius::same(6));
                img_widget.paint_at(ui, img_rect);
                ui.painter().rect_stroke(
                    img_rect,
                    6.0,
                    Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 40)),
                    egui::StrokeKind::Inside,
                );
            }

            // 顶部悬浮磨砂控制栏
            let bar_w = 420.0;
            let bar_h = 44.0;
            let bar_rect = Rect::from_center_size(
                Pos2::new(screen_rect.center().x, screen_rect.min.y + 42.0),
                vec2(bar_w, bar_h),
            );

            ui.painter().rect_filled(bar_rect, 10.0, Color32::from_rgba_unmultiplied(24, 28, 38, 240));
            ui.painter().rect_stroke(
                bar_rect,
                10.0,
                Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 35)),
                egui::StrokeKind::Inside,
            );

            // 控制栏左侧文件名与分辨率标签
            let text_left = bar_rect.min.x + 14.0;
            ui.painter().text(
                Pos2::new(text_left, bar_rect.center().y - 7.0),
                Align2::LEFT_CENTER,
                &att_name,
                FontId::new(12.5, egui::FontFamily::Proportional),
                Color32::from_rgb(230, 235, 245),
            );

            let meta_text = if let Some((w, h)) = att_dim {
                format!("{w}×{h} · {att_size_str} · {:.0}%", state.zoom * 100.0)
            } else {
                format!("{att_size_str} · {:.0}%", state.zoom * 100.0)
            };
            ui.painter().text(
                Pos2::new(text_left, bar_rect.center().y + 8.0),
                Align2::LEFT_CENTER,
                meta_text,
                FontId::new(10.5, egui::FontFamily::Proportional),
                Color32::from_rgb(140, 155, 175),
            );

            // 控制栏右侧按钮群（发光微方块 + 矢量线条）
            let btn_base = Color32::from_white_alpha(5);
            let btn_hover = Color32::from_white_alpha(15);
            let btn_base_stroke = Color32::from_white_alpha(8);
            let btn_hover_stroke = Color32::from_white_alpha(20);
            let btn_shadow = Color32::from_black_alpha(50);

            let btn_y = bar_rect.center().y;

            // [✕] 关闭微方块
            let close_rect = Rect::from_center_size(Pos2::new(bar_rect.max.x - 20.0, btn_y), vec2(22.0, 22.0));
            let close_resp = ui.interact(close_rect, Id::new("lb_close_btn"), Sense::click());
            let close_hf = ui.ctx().animate_bool(Id::new("lb_close_h"), close_resp.hovered());
            ui.painter().rect_filled(close_rect.translate(vec2(0.0, 1.0)), 6.0, btn_shadow);
            ui.painter().rect_filled(close_rect, 6.0, lerp_color(btn_base, btn_hover, close_hf));
            ui.painter().rect_stroke(close_rect, 6.0, Stroke::new(0.5, lerp_color(btn_base_stroke, btn_hover_stroke, close_hf)), egui::StrokeKind::Inside);
            let close_fg = lerp_color(Color32::from_gray(160), Color32::from_rgb(245, 90, 90), close_hf);
            let cc = close_rect.center();
            let cd = 3.2;
            ui.painter().line_segment([cc + vec2(-cd, -cd), cc + vec2(cd, cd)], Stroke::new(1.35, close_fg));
            ui.painter().line_segment([cc + vec2(-cd, cd), cc + vec2(cd, -cd)], Stroke::new(1.35, close_fg));
            if close_resp.clicked() { request_close = true; }
            close_resp.on_hover_text("关闭预览 (Esc)");

            // [↗] 外部打开微方块
            let open_rect = Rect::from_center_size(Pos2::new(bar_rect.max.x - 48.0, btn_y), vec2(22.0, 22.0));
            let open_resp = ui.interact(open_rect, Id::new("lb_open_btn"), Sense::click());
            let open_hf = ui.ctx().animate_bool(Id::new("lb_open_h"), open_resp.hovered());
            ui.painter().rect_filled(open_rect.translate(vec2(0.0, 1.0)), 6.0, btn_shadow);
            ui.painter().rect_filled(open_rect, 6.0, lerp_color(btn_base, btn_hover, open_hf));
            ui.painter().rect_stroke(open_rect, 6.0, Stroke::new(0.5, lerp_color(btn_base_stroke, btn_hover_stroke, open_hf)), egui::StrokeKind::Inside);
            let open_fg = lerp_color(Color32::from_gray(160), Color32::WHITE, open_hf);
            let oc = open_rect.center();
            ui.painter().line_segment([oc + vec2(-3.5, 3.5), oc + vec2(3.5, -3.5)], Stroke::new(1.3, open_fg));
            ui.painter().line_segment([oc + vec2(0.0, -3.5), oc + vec2(3.5, -3.5)], Stroke::new(1.3, open_fg));
            ui.painter().line_segment([oc + vec2(3.5, -3.5), oc + vec2(3.5, 0.0)], Stroke::new(1.3, open_fg));
            if open_resp.clicked() { request_external_open = true; }
            open_resp.on_hover_text("在系统默认应用中打开");

            // [📋] 复制路径微方块
            let copy_rect = Rect::from_center_size(Pos2::new(bar_rect.max.x - 76.0, btn_y), vec2(22.0, 22.0));
            let copy_resp = ui.interact(copy_rect, Id::new("lb_copy_btn"), Sense::click());
            let copy_hf = ui.ctx().animate_bool(Id::new("lb_copy_h"), copy_resp.hovered());
            ui.painter().rect_filled(copy_rect.translate(vec2(0.0, 1.0)), 6.0, btn_shadow);
            ui.painter().rect_filled(copy_rect, 6.0, lerp_color(btn_base, btn_hover, copy_hf));
            ui.painter().rect_stroke(copy_rect, 6.0, Stroke::new(0.5, lerp_color(btn_base_stroke, btn_hover_stroke, copy_hf)), egui::StrokeKind::Inside);
            let copy_fg = lerp_color(Color32::from_gray(160), Color32::WHITE, copy_hf);
            let cpc = copy_rect.center();
            let doc_back = Rect::from_min_size(cpc + vec2(-4.0, -5.0), vec2(6.5, 8.0));
            let doc_front = Rect::from_min_size(cpc + vec2(-2.0, -3.0), vec2(6.5, 8.0));
            ui.painter().rect_stroke(doc_back, 1.0, Stroke::new(1.0, copy_fg.gamma_multiply(0.6)), egui::StrokeKind::Inside);
            ui.painter().rect_filled(doc_front, 1.0, Color32::from_rgb(24, 28, 38));
            ui.painter().rect_stroke(doc_front, 1.0, Stroke::new(1.2, copy_fg), egui::StrokeKind::Inside);
            if copy_resp.clicked() { request_copy = true; }
            copy_resp.on_hover_text("复制图片绝对路径");

            // [⟲] 重置缩放微方块
            let reset_rect = Rect::from_center_size(Pos2::new(bar_rect.max.x - 104.0, btn_y), vec2(22.0, 22.0));
            let reset_resp = ui.interact(reset_rect, Id::new("lb_reset_btn"), Sense::click());
            let reset_hf = ui.ctx().animate_bool(Id::new("lb_reset_h"), reset_resp.hovered());
            ui.painter().rect_filled(reset_rect.translate(vec2(0.0, 1.0)), 6.0, btn_shadow);
            ui.painter().rect_filled(reset_rect, 6.0, lerp_color(btn_base, btn_hover, reset_hf));
            ui.painter().rect_stroke(reset_rect, 6.0, Stroke::new(0.5, lerp_color(btn_base_stroke, btn_hover_stroke, reset_hf)), egui::StrokeKind::Inside);
            let reset_fg = lerp_color(Color32::from_gray(160), Color32::WHITE, reset_hf);
            let rc = reset_rect.center();
            ui.painter().circle_stroke(rc, 3.8, Stroke::new(1.2, reset_fg));
            ui.painter().line_segment([rc + vec2(-3.8, -1.0), rc + vec2(-3.8, 2.0)], Stroke::new(1.3, reset_fg));
            ui.painter().line_segment([rc + vec2(-3.8, 2.0), rc + vec2(-1.0, 2.0)], Stroke::new(1.3, reset_fg));
            if reset_resp.clicked() {
                state.zoom = 1.0;
                state.pan = Vec2::ZERO;
            }
            reset_resp.on_hover_text("重置缩放为 100%");

            // 左右导航切换按钮（多图时展示）
            if state.attachments.len() > 1 {
                let nav_y = screen_rect.center().y;
                let prev_rect = Rect::from_center_size(Pos2::new(screen_rect.min.x + 36.0, nav_y), vec2(36.0, 52.0));
                let prev_resp = ui.interact(prev_rect, Id::new("lb_nav_prev"), Sense::click());
                let prev_bg = if prev_resp.hovered() { Color32::from_rgba_unmultiplied(40, 48, 65, 230) } else { Color32::from_rgba_unmultiplied(20, 24, 32, 180) };
                ui.painter().rect_filled(prev_rect, 8.0, prev_bg);
                ui.painter().text(prev_rect.center(), Align2::CENTER_CENTER, "〈", FontId::new(18.0, egui::FontFamily::Proportional), Color32::WHITE);
                if prev_resp.clicked() { request_prev = true; }

                let next_rect = Rect::from_center_size(Pos2::new(screen_rect.max.x - 36.0, nav_y), vec2(40.0, 56.0));
                let next_resp = ui.interact(next_rect, Id::new("lb_nav_next"), Sense::click());
                let next_bg = if next_resp.hovered() { Color32::from_rgba_unmultiplied(40, 48, 65, 230) } else { Color32::from_rgba_unmultiplied(20, 24, 32, 180) };
                ui.painter().rect_filled(next_rect, 8.0, next_bg);
                ui.painter().text(next_rect.center(), Align2::CENTER_CENTER, "〉", FontId::new(18.0, egui::FontFamily::Proportional), Color32::WHITE);
                if next_resp.clicked() { request_next = true; }
            }
        });

    if request_close {
        state.close_preview();
    }
    if request_copy {
        ctx.copy_text(att_path.to_string_lossy().to_string());
    }
    if request_external_open {
        #[cfg(target_os = "windows")]
        {
            let _ = std::process::Command::new("explorer")
                .arg(&att_path)
                .spawn();
        }
    }
    if request_prev {
        state.navigate_preview(false);
    }
    if request_next {
        state.navigate_preview(true);
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
