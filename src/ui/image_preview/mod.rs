//! AI 多模态图片预览子模块：包含缩略图悬浮胶囊与全屏 Lightbox 大图查看器。

pub mod lightbox;
pub mod loader;
pub mod pill;
pub mod state;

#[allow(unused_imports)]
pub use lightbox::show_lightbox_modal;
pub use loader::is_image_path;
pub use pill::show_attachment_pill;
#[allow(unused_imports)]
pub use state::{ImageAttachment, ImagePreviewState};

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_is_image_path() {
        assert!(is_image_path(&PathBuf::from("test.png")));
        assert!(is_image_path(&PathBuf::from("path/to/photo.JPG")));
        assert!(is_image_path(&PathBuf::from("C:\\images\\banner.webp")));
        assert!(is_image_path(&PathBuf::from("icon.bmp")));
        assert!(!is_image_path(&PathBuf::from("script.rs")));
        assert!(!is_image_path(&PathBuf::from("document.pdf")));
        assert!(!is_image_path(&PathBuf::from("binary")));
    }

    #[test]
    fn test_attachment_lifecycle() {
        let mut state = ImagePreviewState::new();
        assert!(state.attachments.is_empty());

        let p1 = PathBuf::from("C:\\test\\pic1.png");
        let p2 = PathBuf::from("C:\\test\\pic2.jpg");

        state.add_attachment(p1.clone());
        assert_eq!(state.attachments.len(), 1);
        assert_eq!(state.attachments[0].file_name, "pic1.png");

        state.add_attachment(p2.clone());
        assert_eq!(state.attachments.len(), 2);

        // 重复添加 p1 应当置顶且总数不增加
        state.add_attachment(p1.clone());
        assert_eq!(state.attachments.len(), 2);
        assert_eq!(state.attachments[1].file_name, "pic1.png");

        // 测试格式化注入文本
        let inject_str = state.format_injection_text();
        assert!(inject_str.contains("\"C:\\test\\pic2.jpg\""));
        assert!(inject_str.contains("\"C:\\test\\pic1.png\""));

        // 打开大图预览与切换
        let id1 = state.attachments[0].id;
        state.open_preview(id1);
        assert_eq!(state.active_preview_id, Some(id1));

        state.navigate_preview(true);
        assert_ne!(state.active_preview_id, Some(id1));

        state.close_preview();
        assert_eq!(state.active_preview_id, None);

        // 移除附件
        state.remove_attachment(id1);
        assert_eq!(state.attachments.len(), 1);
    }
}
