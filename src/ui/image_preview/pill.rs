//! 终端右下角多模态附件悬浮胶囊组件：
//! 严格照搬 CLIHub Workspaces 卡片底色、阴影与搜索框按键群规范（发光微方块、矢量线条、无任何 Unicode 字体缺失方框）。

use std::time::Instant;

use egui::{
    Align2, Color32, CornerRadius, FontId, Id, Image, Pos2, Rect, Sense, Stroke, Ui, vec2,
};

use super::loader::ensure_thumbnail_loaded;
use super::state::ImagePreviewState;
use crate::ui::terminal::TermTheme;

/// 在终端区域右下角绘制多模态图片附件悬浮暂存区胶囊
pub fn show_attachment_pill(
    ui: &mut Ui,
    state: &mut ImagePreviewState,
    term_rect: Rect,
    theme: &TermTheme,
) {
    if state.attachments.is_empty() {
        return;
    }

    let dark = theme.is_dark();
    let count = state.attachments.len();
    let now = Instant::now();

    let margin_right = 14.0;
    let margin_bottom = 14.0;

    // 按钮通用设计规范（与搜索栏按键群完全一致）
    let btn_base = if dark { Color32::from_white_alpha(5) } else { Color32::from_black_alpha(8) };
    let btn_hover = if dark { Color32::from_white_alpha(15) } else { Color32::from_black_alpha(16) };
    let btn_base_stroke = if dark { Color32::from_white_alpha(8) } else { Color32::from_black_alpha(10) };
    let btn_hover_stroke = if dark { Color32::from_white_alpha(20) } else { Color32::from_black_alpha(18) };
    let btn_shadow = if dark { Color32::from_black_alpha(50) } else { Color32::from_black_alpha(12) };

    // ---- 顶级纯黑中性微透卡片面板（底色 100% 对齐 CLIHub 主题，移除多余标头行）----
    let card_w = 268.0;
    let item_h = 48.0;
    let padding_y = 6.0;
    let list_spacing = 4.0;
    let total_h = (count as f32 * item_h) + ((count.saturating_sub(1)) as f32 * list_spacing) + padding_y * 2.0;

    let card_rect = Rect::from_min_size(
        Pos2::new(
            term_rect.max.x - margin_right - card_w,
            term_rect.max.y - margin_bottom - total_h,
        ),
        vec2(card_w, total_h),
    );

    let card_resp = ui.allocate_rect(card_rect, Sense::hover());
    if card_resp.hovered() {
        state.last_interaction = Some(now);
    }

    // 纯粹中性底色（与 Workspace 卡片底色完全同源）
    let panel_bg = if dark {
        Color32::from_rgb(18, 18, 22)
    } else {
        Color32::from_rgb(248, 249, 252)
    };
    let border_stroke = if dark {
        Color32::from_white_alpha(10)
    } else {
        Color32::from_black_alpha(12)
    };

    // 柔和漫反射环境底阴影
    let shadow_color = Color32::from_black_alpha(if dark { 75 } else { 18 });
    let painter = ui.painter();
    painter.rect_filled(card_rect.translate(vec2(0.0, 2.0)), 12.0, shadow_color);
    painter.rect_filled(card_rect, 12.0, panel_bg);
    painter.rect_stroke(card_rect, 12.0, Stroke::new(0.5, border_stroke), egui::StrokeKind::Inside);

    // 附件条目列表（直接平铺呈现，无冗余标头）
    let mut to_remove_id: Option<u64> = None;
    let mut to_preview_id: Option<u64> = None;
    let mut to_open_folder: Option<std::path::PathBuf> = None;

    let items_top = card_rect.min.y + padding_y;
    for (i, item) in state.attachments.iter_mut().enumerate() {
        let item_y = items_top + (i as f32 * (item_h + list_spacing));
        let item_rect = Rect::from_min_size(
            Pos2::new(card_rect.min.x + 8.0, item_y),
            vec2(card_w - 16.0, item_h),
        );

        let item_resp = ui.interact(item_rect, Id::new(("pill_item_row", item.id)), Sense::hover());
        let item_hf = ui.ctx().animate_bool(Id::new(("pill_item_h", item.id)), item_resp.hovered());

        // 照搬 Workspace 卡片独立渲染公式
        let card_base = if dark { Color32::from_white_alpha(5) } else { Color32::from_black_alpha(8) };
        let card_hover = if dark { Color32::from_white_alpha(12) } else { Color32::from_black_alpha(15) };
        let card_bg = lerp_color(card_base, card_hover, item_hf);

        let card_base_stroke = if dark { Color32::from_white_alpha(6) } else { Color32::from_black_alpha(10) };
        let card_hover_stroke = if dark { Color32::from_white_alpha(14) } else { Color32::from_black_alpha(18) };
        let card_stroke = lerp_color(card_base_stroke, card_hover_stroke, item_hf);

        painter.rect_filled(item_rect.translate(vec2(0.0, 1.0)), 10.0, Color32::from_black_alpha(if dark { 40 } else { 10 }));
        painter.rect_filled(item_rect, 10.0, card_bg);
        painter.rect_stroke(item_rect, 10.0, Stroke::new(0.5, card_stroke), egui::StrokeKind::Inside);

        // 确保缩略图纹理已加载
        ensure_thumbnail_loaded(ui.ctx(), item);

        // 缩略图区域（34x34，圆角 6px）
        let thumb_rect = Rect::from_min_size(
            Pos2::new(item_rect.min.x + 7.0, item_rect.min.y + 7.0),
            vec2(34.0, 34.0),
        );

        if let Some(tex) = &item.thumbnail {
            let img = Image::from_texture(tex).corner_radius(CornerRadius::same(6));
            img.paint_at(ui, thumb_rect);
        } else {
            painter.rect_filled(thumb_rect, 6.0, Color32::from_gray(40));
        }

        painter.rect_stroke(
            thumb_rect,
            6.0,
            Stroke::new(0.5, Color32::from_white_alpha(20)),
            egui::StrokeKind::Inside,
        );

        // 点击缩略图快速放大预览
        let thumb_resp = ui.interact(thumb_rect, Id::new(("thumb_click_box", item.id)), Sense::click());
        if thumb_resp.clicked() {
            to_preview_id = Some(item.id);
        }
        thumb_resp.on_hover_text("点击放大预览");

        // 文本信息：主标题文件名 + 副标题尺寸与大小
        let text_left = thumb_rect.max.x + 8.0;
        let name_display = if item.file_name.chars().count() > 13 {
            let prefix: String = item.file_name.chars().take(10).collect();
            format!("{prefix}...")
        } else {
            item.file_name.clone()
        };

        let title_c = if dark { Color32::from_rgb(205, 214, 244) } else { Color32::from_rgb(30, 41, 59) };
        painter.text(
            Pos2::new(text_left, item_rect.min.y + 15.0),
            Align2::LEFT_CENTER,
            name_display,
            FontId::new(12.5, egui::FontFamily::Proportional),
            title_c,
        );

        let sub_text = if let Some((w, h)) = item.dimensions {
            format!("{w}×{h} · {}", item.file_size_str)
        } else {
            item.file_size_str.clone()
        };
        let sub_c = if dark { Color32::from_rgb(166, 173, 200) } else { Color32::from_rgb(148, 163, 184) };
        painter.text(
            Pos2::new(text_left, item_rect.min.y + 32.0),
            Align2::LEFT_CENTER,
            sub_text,
            FontId::new(10.5, egui::FontFamily::Monospace),
            sub_c,
        );

        // ---- 右侧按键群（符合搜索栏按键群设计规范：小方框、微发光、纯矢量图标）----
        let btn_center_y = item_rect.center().y;

        // 1) [✕] 移除微方块按键（纯矢量发丝交叉线，永远不会出现 □ 方框）
        let del_rect = Rect::from_center_size(Pos2::new(item_rect.max.x - 16.0, btn_center_y), vec2(22.0, 22.0));
        let del_resp = ui.interact(del_rect, Id::new(("pill_del_btn", item.id)), Sense::click());
        let del_hf = ui.ctx().animate_bool(Id::new(("pill_del_h", item.id)), del_resp.hovered());
        let d_bg = lerp_color(btn_base, btn_hover, del_hf);
        let d_stroke = lerp_color(btn_base_stroke, btn_hover_stroke, del_hf);
        painter.rect_filled(del_rect.translate(vec2(0.0, 1.0)), 6.0, btn_shadow);
        painter.rect_filled(del_rect, 6.0, d_bg);
        painter.rect_stroke(del_rect, 6.0, Stroke::new(0.5, d_stroke), egui::StrokeKind::Inside);

        let d_fg = lerp_color(
            if dark { Color32::from_gray(160) } else { Color32::from_gray(100) },
            Color32::from_rgb(245, 90, 90),
            del_hf,
        );
        let dc = del_rect.center();
        let d_len = 3.2;
        painter.line_segment([dc + vec2(-d_len, -d_len), dc + vec2(d_len, d_len)], Stroke::new(1.35, d_fg));
        painter.line_segment([dc + vec2(-d_len, d_len), dc + vec2(d_len, -d_len)], Stroke::new(1.35, d_fg));
        if del_resp.clicked() {
            to_remove_id = Some(item.id);
        }
        del_resp.on_hover_text("移除附件");

        // 2) [📂] 资源管理器定位微方块按键（纯矢量文件夹轮廓）
        let loc_rect = Rect::from_center_size(Pos2::new(item_rect.max.x - 41.0, btn_center_y), vec2(22.0, 22.0));
        let loc_resp = ui.interact(loc_rect, Id::new(("pill_loc_btn", item.id)), Sense::click());
        let loc_hf = ui.ctx().animate_bool(Id::new(("pill_loc_h", item.id)), loc_resp.hovered());
        let l_bg = lerp_color(btn_base, btn_hover, loc_hf);
        let l_stroke = lerp_color(btn_base_stroke, btn_hover_stroke, loc_hf);
        painter.rect_filled(loc_rect.translate(vec2(0.0, 1.0)), 6.0, btn_shadow);
        painter.rect_filled(loc_rect, 6.0, l_bg);
        painter.rect_stroke(loc_rect, 6.0, Stroke::new(0.5, l_stroke), egui::StrokeKind::Inside);

        let l_fg = lerp_color(
            if dark { Color32::from_gray(160) } else { Color32::from_gray(100) },
            if dark { Color32::WHITE } else { Color32::BLACK },
            loc_hf,
        );
        let lc = loc_rect.center();
        let f_tab = Rect::from_min_size(lc + vec2(-5.0, -4.5), vec2(4.0, 2.5));
        painter.rect_filled(f_tab, 1.0, l_fg);
        let f_body = Rect::from_min_size(lc + vec2(-5.0, -2.5), vec2(10.0, 7.0));
        painter.rect_stroke(f_body, 1.5, Stroke::new(1.15, l_fg), egui::StrokeKind::Inside);
        if loc_resp.clicked() {
            to_open_folder = Some(item.path.clone());
        }
        loc_resp.on_hover_text("在资源管理器中定位文件");

        // 3) [🔍] 全屏放大预览微方块按键（纯矢量放大镜）
        let prev_rect = Rect::from_center_size(Pos2::new(item_rect.max.x - 66.0, btn_center_y), vec2(22.0, 22.0));
        let prev_resp = ui.interact(prev_rect, Id::new(("pill_prev_btn", item.id)), Sense::click());
        let prev_hf = ui.ctx().animate_bool(Id::new(("pill_prev_h", item.id)), prev_resp.hovered());
        let p_bg = lerp_color(btn_base, btn_hover, prev_hf);
        let p_stroke = lerp_color(btn_base_stroke, btn_hover_stroke, prev_hf);
        painter.rect_filled(prev_rect.translate(vec2(0.0, 1.0)), 6.0, btn_shadow);
        painter.rect_filled(prev_rect, 6.0, p_bg);
        painter.rect_stroke(prev_rect, 6.0, Stroke::new(0.5, p_stroke), egui::StrokeKind::Inside);

        let p_fg = lerp_color(
            if dark { Color32::from_gray(160) } else { Color32::from_gray(100) },
            if dark { Color32::WHITE } else { Color32::BLACK },
            prev_hf,
        );
        let pc = prev_rect.center();
        let lens_c = pc + vec2(-1.5, -1.5);
        painter.circle_stroke(lens_c, 3.5, Stroke::new(1.2, p_fg));
        painter.line_segment([lens_c + vec2(2.5, 2.5), pc + vec2(4.5, 4.5)], Stroke::new(1.35, p_fg));
        if prev_resp.clicked() {
            to_preview_id = Some(item.id);
        }
        prev_resp.on_hover_text("全屏大图预览");
    }

    // 状态更新派发
    if let Some(id) = to_remove_id {
        state.remove_attachment(id);
    }
    if let Some(id) = to_preview_id {
        state.open_preview(id);
    }
    if let Some(path) = to_open_folder {
        #[cfg(target_os = "windows")]
        {
            let _ = std::process::Command::new("explorer")
                .args(["/select,", &path.to_string_lossy()])
                .spawn();
        }
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
