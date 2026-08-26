//! 本地图片解码与 egui GPU 纹理缓存加载器。

#![allow(dead_code)]

use std::path::Path;
use egui::{Color32, ColorImage, Context, TextureHandle, TextureOptions};

use super::state::ImageAttachment;

/// 判断给定文件路径是否为支持的图片格式
pub fn is_image_path(path: &Path) -> bool {
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        let ext_lower = ext.to_ascii_lowercase();
        matches!(
            ext_lower.as_str(),
            "png" | "jpg" | "jpeg" | "webp" | "bmp" | "gif" | "ico" | "tiff" | "tif"
        )
    } else {
        false
    }
}

/// 稳健加载本地图片文件（支持魔数格式自动猜测，免疫改扩展名问题）
fn load_dynamic_image(path: &Path) -> Option<image::DynamicImage> {
    if let Ok(reader) = image::ImageReader::open(path) {
        if let Ok(reader_with_format) = reader.with_guessed_format() {
            if let Ok(img) = reader_with_format.decode() {
                return Some(img);
            }
        }
    }
    image::open(path).ok()
}

/// 确保缩略图纹理已加载（限制长宽最大 128px，极度节省显存与 CPU）
pub fn ensure_thumbnail_loaded(ctx: &Context, attachment: &mut ImageAttachment) {
    if attachment.thumbnail.is_some() {
        return;
    }

    if let Some(img) = load_dynamic_image(&attachment.path) {
        let (w, h) = (img.width(), img.height());
        attachment.dimensions = Some((w, h));

        // 等比缩放生成微型缩略图
        let max_thumb_size = 128;
        let thumb = if w > max_thumb_size || h > max_thumb_size {
            img.thumbnail(max_thumb_size, max_thumb_size)
        } else {
            img
        };

        let rgba = thumb.to_rgba8();
        let (tw, th) = (rgba.width() as usize, rgba.height() as usize);
        let color_image = ColorImage::from_rgba_unmultiplied([tw, th], rgba.as_raw());

        let name = format!("thumb_{}", attachment.id);
        let texture = ctx.load_texture(name, color_image, TextureOptions::LINEAR);
        attachment.thumbnail = Some(texture);
    }
}

/// 确保全尺寸大图纹理已加载（用于 Lightbox 模态大图查看器）
pub fn ensure_full_image_loaded(ctx: &Context, attachment: &mut ImageAttachment) {
    if attachment.full_image.is_some() {
        return;
    }

    if let Some(img) = load_dynamic_image(&attachment.path) {
        let (w, h) = (img.width(), img.height());
        attachment.dimensions = Some((w, h));

        // 针对极端超大图片（如超过 3840 像素）适度降采样，防止显存超限
        let max_dim = 3840;
        let final_img = if w > max_dim || h > max_dim {
            img.thumbnail(max_dim, max_dim)
        } else {
            img
        };

        let rgba = final_img.to_rgba8();
        let (fw, fh) = (rgba.width() as usize, rgba.height() as usize);
        let color_image = ColorImage::from_rgba_unmultiplied([fw, fh], rgba.as_raw());

        let name = format!("full_{}", attachment.id);
        let texture = ctx.load_texture(name, color_image, TextureOptions::LINEAR);
        attachment.full_image = Some(texture);
    }
}

/// 创建占位纯色或错误纹理
pub fn placeholder_texture(ctx: &Context, id: u64) -> TextureHandle {
    let size = [32, 32];
    let pixels = vec![Color32::from_rgb(50, 55, 65); 32 * 32];
    let color_image = ColorImage::new(size, pixels);
    ctx.load_texture(format!("placeholder_{id}"), color_image, TextureOptions::NEAREST)
}
