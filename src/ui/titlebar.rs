//! 自定义无边框标题栏（Windows 风格）：左上角应用名，右上角三个控件
//! —— 最小化 / 最大化(还原) / 关闭，样式贴近 Windows 10/11。
//!
//! 窗口以 `ViewportBuilder::with_decorations(false)` 打开；Windows 下
//! 保持 `resizable` 时仍保留隐形缩放进边，无需手动实现 resize。

use egui::{Align2, Color32, FontId, Id, Pos2, Rect, Sense, Stroke, Ui, vec2};
use egui::ViewportCommand;

const TITLEBAR_H: f32 = 40.0;
const BTN_W: f32 = 46.0;

#[derive(Clone, Copy)]
enum CaptionIcon {
    Minimize,
    Maximize,
    Restore,
    Close,
}

pub fn show(ui: &mut Ui) -> bool {
    let mut settings_clicked = false;
    let (rect, resp) = ui.allocate_exact_size(
        vec2(ui.available_width(), TITLEBAR_H),
        Sense::click_and_drag(),
    );

    // 背景
    ui.painter().rect_filled(rect, 0.0, ui.visuals().panel_fill);

    // 拖拽窗口
    if resp.drag_started() {
        ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
    }
    // 双击标题栏 -> 最大化/还原
    if resp.double_clicked() {
        let maxed = ui.input(|i| i.viewport().maximized == Some(true));
        ui.ctx().send_viewport_cmd(ViewportCommand::Maximized(!maxed));
    }

    // 优雅艺术标题 (Playfair Display Italic)
    let dark = ui.visuals().dark_mode;
    let title_color = if dark {
        Color32::from_rgb(205, 214, 244) // Text in Catppuccin
    } else {
        Color32::from_rgb(30, 41, 59) // Slate 800
    };
    
    // 使用优雅但不张扬的 Playfair Display 字体
    let title_font = FontId::new(18.0, egui::FontFamily::Name("title".into()));
    let title_text = "CLIHub";
    let title_pos = Pos2::new(rect.min.x + 20.0, rect.center().y);
    
    // 主文本 (无发光，追求简约大气)
    ui.painter().text(
        title_pos,
        Align2::LEFT_CENTER,
        title_text,
        title_font.clone(),
        title_color,
    );
    
    // 动态计算标题宽度以完美对齐后面的控件
    let title_w = ui.painter().layout_no_wrap(title_text.to_owned(), title_font, Color32::WHITE).rect.width();
    
    // Settings button next to title
    let settings_font = FontId::proportional(12.0);
    let settings_text = "Settings";
    let text_w = ui.painter().layout_no_wrap(settings_text.to_owned(), settings_font.clone(), Color32::WHITE).rect.width();
    let settings_h = 24.0;
    // 严格垂直居中对齐
    let settings_rect = Rect::from_min_size(
        Pos2::new(rect.min.x + 20.0 + title_w + 16.0, rect.center().y - (settings_h / 2.0)),
        vec2(text_w + 16.0, settings_h)
    );
    let settings_resp = ui.interact(settings_rect, Id::new("titlebar_settings"), Sense::click());
    let txt_color = if settings_resp.hovered() { title_color } else { title_color.gamma_multiply(0.6) };
    ui.painter().text(
        settings_rect.center(),
        Align2::CENTER_CENTER,
        settings_text,
        settings_font,
        txt_color,
    );
    if settings_resp.clicked() {
        settings_clicked = true;
    }
    // (Removed bottom border for seamless HeroUI look)

    // 右上角三个控件（从右到左：关闭 / 最大化 / 最小化）
    let maxed = ui.input(|i| i.viewport().maximized == Some(true));
    let close_x = rect.right() - BTN_W;
    let max_x = close_x - BTN_W;
    let min_x = max_x - BTN_W;

    let min_cmd = draw_caption_button(ui, rect, min_x, CaptionIcon::Minimize);
    let max_cmd = draw_caption_button(
        ui,
        rect,
        max_x,
        if maxed {
            CaptionIcon::Restore
        } else {
            CaptionIcon::Maximize
        },
    );
    let close_cmd = draw_caption_button(ui, rect, close_x, CaptionIcon::Close);

    for cmd in [min_cmd, max_cmd, close_cmd].into_iter().flatten() {
        ui.ctx().send_viewport_cmd(cmd);
    }
    
    settings_clicked
}

/// 绘制一个标题栏控件，返回点击后要发送的命令。
fn draw_caption_button(ui: &mut Ui, titlebar: Rect, x: f32, icon: CaptionIcon) -> Option<ViewportCommand> {
    let btn_rect = Rect::from_min_size(Pos2::new(x, titlebar.top()), vec2(BTN_W, TITLEBAR_H));
    let btn = ui.interact(btn_rect, Id::new(("caption", x as u32)), Sense::click());
    let hovered = btn.hovered();
    let is_close = matches!(icon, CaptionIcon::Close);
    let dark = ui.visuals().dark_mode;

    // 悬浮背景：关闭键红色，其余灰
    let hover_bg = if dark {
        Color32::from_rgb(39, 39, 42) // Zinc-800
    } else {
        Color32::from_rgb(228, 228, 231) // Zinc-200
    };
    let bg = if hovered && is_close {
        Color32::from_rgb(228, 62, 70) // Soft but bright red
    } else if hovered {
        hover_bg
    } else {
        Color32::TRANSPARENT
    };
    if bg != Color32::TRANSPARENT {
        // Pill-shaped hover background, slightly shrunken
        ui.painter().rect_filled(btn_rect.shrink2(vec2(6.0, 4.0)), 6.0, bg);
    }

    // 图标颜色：关闭键悬浮时为白，其余灰
    let fg = if hovered && is_close {
        Color32::WHITE
    } else if dark {
        Color32::from_gray(200)
    } else {
        Color32::from_gray(70)
    };
    let c = btn_rect.center();
    match icon {
        CaptionIcon::Minimize => {
            ui.painter()
                .rect_filled(Rect::from_center_size(c, vec2(12.0, 1.5)), 0.0, fg);
        }
        CaptionIcon::Maximize => {
            ui.painter().rect_stroke(
                Rect::from_center_size(c, vec2(11.0, 11.0)),
                1.5,
                Stroke::new(1.0, fg),
                egui::StrokeKind::Inside,
            );
        }
        CaptionIcon::Restore => {
            // 两个交叠方块
            ui.painter().rect_stroke(
                Rect::from_center_size(c + vec2(2.0, -2.0), vec2(11.0, 11.0)),
                1.5,
                Stroke::new(1.0, fg),
                egui::StrokeKind::Inside,
            );
            ui.painter().rect_stroke(
                Rect::from_center_size(c + vec2(-2.0, 2.0), vec2(11.0, 11.0)),
                1.5,
                Stroke::new(1.0, fg),
                egui::StrokeKind::Inside,
            );
        }
        CaptionIcon::Close => {
            let s = 5.5;
            ui.painter()
                .line_segment([c - vec2(s, s), c + vec2(s, s)], Stroke::new(1.4, fg));
            ui.painter()
                .line_segment([c - vec2(s, -s), c + vec2(s, -s)], Stroke::new(1.4, fg));
        }
    }

    if btn.clicked() {
        Some(match icon {
            CaptionIcon::Minimize => ViewportCommand::Minimized(true),
            CaptionIcon::Close => ViewportCommand::Close,
            CaptionIcon::Maximize | CaptionIcon::Restore => {
                let maxed = ui.input(|i| i.viewport().maximized == Some(true));
                ViewportCommand::Maximized(!maxed)
            }
        })
    } else {
        None
    }
}
