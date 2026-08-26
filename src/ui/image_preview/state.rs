//! 图片附件与 Lightbox 预览状态管理。

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use egui::{TextureHandle, Vec2};

static NEXT_ATTACHMENT_ID: AtomicU64 = AtomicU64::new(1);

/// 单个多模态图片附件
pub struct ImageAttachment {
    /// 唯一标识 ID
    pub id: u64,
    /// 图片文件的本地绝对路径
    pub path: PathBuf,
    /// 显示用的文件名
    pub file_name: String,
    /// 格式化后的文件大小（例如 "1.2 MB"）
    pub file_size_str: String,
    /// 图片实际像素分辨率 (宽度, 高度)
    pub dimensions: Option<(u32, u32)>,
    /// 创建/注入时间
    pub created_at: Instant,
    /// 缩略图纹理缓存（最大 128px 宽/高，极速省显存）
    pub thumbnail: Option<TextureHandle>,
    /// 全屏高清原图纹理缓存（按需延迟加载）
    pub full_image: Option<TextureHandle>,
}

impl ImageAttachment {
    pub fn new(path: PathBuf) -> Self {
        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "image.png".to_string());

        let file_size_str = if let Ok(meta) = std::fs::metadata(&path) {
            format_file_size(meta.len())
        } else {
            String::new()
        };

        Self {
            id: NEXT_ATTACHMENT_ID.fetch_add(1, Ordering::Relaxed),
            path,
            file_name,
            file_size_str,
            dimensions: None,
            created_at: Instant::now(),
            thumbnail: None,
            full_image: None,
        }
    }
}

/// 格式化字节数为易读字符串
fn format_file_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

/// 终端实例持有的多模态图片暂存区（Staging Area）状态
#[derive(Default)]
pub struct ImagePreviewState {
    /// 暂存的待发送图片附件列表（最多保留 6 个最近项）
    pub attachments: Vec<ImageAttachment>,
    /// 当前正在全屏 Lightbox 模态中预览的附件 ID
    pub active_preview_id: Option<u64>,
    /// Lightbox 大图缩放系数（默认 1.0）
    pub zoom: f32,
    /// Lightbox 大图拖拽平移偏移量
    pub pan: Vec2,
    /// 附件悬浮胶囊是否处于收起折叠状态
    pub is_collapsed: bool,
    /// 最近一次用户与胶囊交互的时间（用于微动画）
    pub last_interaction: Option<Instant>,
}

impl ImagePreviewState {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {
            attachments: Vec::new(),
            active_preview_id: None,
            zoom: 1.0,
            pan: Vec2::ZERO,
            is_collapsed: false,
            last_interaction: Some(Instant::now()),
        }
    }

    /// 添加新图片到暂存区（去重并移动到末尾）
    pub fn add_attachment(&mut self, path: PathBuf) {
        if let Some(pos) = self.attachments.iter().position(|a| a.path == path) {
            let mut item = self.attachments.remove(pos);
            item.created_at = Instant::now();
            self.attachments.push(item);
        } else {
            if self.attachments.len() >= 6 {
                self.attachments.remove(0);
            }
            self.attachments.push(ImageAttachment::new(path));
        }
        self.is_collapsed = false;
        self.last_interaction = Some(Instant::now());
    }

    /// 清空待发送附件（当用户按下回车发送命令或 Ctrl+C 取消输入时触发）
    pub fn clear(&mut self) {
        self.attachments.clear();
        self.active_preview_id = None;
        self.is_collapsed = false;
    }

    /// 移除指定 ID 的附件
    pub fn remove_attachment(&mut self, id: u64) {
        if let Some(pos) = self.attachments.iter().position(|a| a.id == id) {
            self.attachments.remove(pos);
        }
        if self.active_preview_id == Some(id) {
            self.close_preview();
        }
    }

    /// 打开指定 ID 附件的 Lightbox 模态大图预览
    pub fn open_preview(&mut self, id: u64) {
        self.active_preview_id = Some(id);
        self.zoom = 1.0;
        self.pan = Vec2::ZERO;
        self.last_interaction = Some(Instant::now());
    }

    /// 关闭当前大图预览
    pub fn close_preview(&mut self) {
        self.active_preview_id = None;
        self.zoom = 1.0;
        self.pan = Vec2::ZERO;
    }

    /// 切换上一张 / 下一张预览
    pub fn navigate_preview(&mut self, forward: bool) {
        if let Some(current_id) = self.active_preview_id {
            if let Some(idx) = self.attachments.iter().position(|a| a.id == current_id) {
                let len = self.attachments.len();
                if len > 1 {
                    let next_idx = if forward {
                        (idx + 1) % len
                    } else if idx == 0 {
                        len - 1
                    } else {
                        idx - 1
                    };
                    self.open_preview(self.attachments[next_idx].id);
                }
            }
        }
    }

    /// 将所有暂存图片格式化为注入字符串（例如 "\"D:\\pic1.png\" \"D:\\pic2.png\" "`）
    pub fn format_injection_text(&self) -> String {
        if self.attachments.is_empty() {
            return String::new();
        }
        let mut text = String::new();
        for item in &self.attachments {
            let path_str = item.path.to_string_lossy();
            text.push_str(&format!("\"{path_str}\" "));
        }
        text
    }
}
