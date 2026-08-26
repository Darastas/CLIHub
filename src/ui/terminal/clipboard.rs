//! 终端系统剪贴板原生交互与多模态图片转存管理。
//! 支持纯文本复制粘贴、截图/位图自动转存为本地 PNG 临时文件，以及过期缓存自动回收。

use std::path::PathBuf;

/// 读取系统剪贴板文本（Windows 走 Win32 API 零依赖无额外分配，跨平台回退 None）
#[cfg(windows)]
pub fn get_clipboard_text() -> Option<String> {
    use std::ptr::null_mut;
    unsafe extern "system" {
        fn OpenClipboard(h_wnd_new_owner: *mut std::ffi::c_void) -> i32;
        fn CloseClipboard() -> i32;
        fn GetClipboardData(u_format: u32) -> *mut std::ffi::c_void;
        fn GlobalLock(h_mem: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
        fn GlobalUnlock(h_mem: *mut std::ffi::c_void) -> i32;
    }
    const CF_UNICODETEXT: u32 = 13;
    unsafe {
        if OpenClipboard(null_mut()) == 0 {
            return None;
        }
        let handle = GetClipboardData(CF_UNICODETEXT);
        if handle.is_null() {
            CloseClipboard();
            return None;
        }
        let ptr = GlobalLock(handle) as *mut u16;
        if ptr.is_null() {
            CloseClipboard();
            return None;
        }
        let mut len = 0;
        while *ptr.add(len) != 0 {
            len += 1;
        }
        let slice = std::slice::from_raw_parts(ptr, len);
        let s = String::from_utf16_lossy(slice);
        GlobalUnlock(handle);
        CloseClipboard();
        Some(s)
    }
}

/// 将文本写入系统剪贴板（Windows 走 Win32 原生 API，零延迟、零依赖、立即生效）
#[cfg(windows)]
pub fn set_clipboard_text(text: &str) -> bool {
    use std::ptr::null_mut;
    unsafe extern "system" {
        fn OpenClipboard(h_wnd_new_owner: *mut std::ffi::c_void) -> i32;
        fn CloseClipboard() -> i32;
        fn EmptyClipboard() -> i32;
        fn SetClipboardData(u_format: u32, h_mem: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
        fn GlobalAlloc(u_flags: u32, dw_bytes: usize) -> *mut std::ffi::c_void;
        fn GlobalLock(h_mem: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
        fn GlobalUnlock(h_mem: *mut std::ffi::c_void) -> i32;
        fn GlobalFree(h_mem: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    }
    const CF_UNICODETEXT: u32 = 13;
    const GMEM_MOVEABLE: u32 = 0x0002;

    let utf16: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let bytes_len = utf16.len() * std::mem::size_of::<u16>();

    unsafe {
        if OpenClipboard(null_mut()) == 0 {
            return false;
        }
        EmptyClipboard();
        let h_mem = GlobalAlloc(GMEM_MOVEABLE, bytes_len);
        if h_mem.is_null() {
            CloseClipboard();
            return false;
        }
        let ptr = GlobalLock(h_mem) as *mut u16;
        if ptr.is_null() {
            GlobalFree(h_mem);
            CloseClipboard();
            return false;
        }
        std::ptr::copy_nonoverlapping(utf16.as_ptr(), ptr, utf16.len());
        GlobalUnlock(h_mem);
        if SetClipboardData(CF_UNICODETEXT, h_mem).is_null() {
            GlobalFree(h_mem);
            CloseClipboard();
            return false;
        }
        CloseClipboard();
        true
    }
}

#[cfg(not(windows))]
pub fn get_clipboard_text() -> Option<String> {
    let mut cb = arboard::Clipboard::new().ok()?;
    cb.get_text().ok()
}

#[cfg(not(windows))]
pub fn set_clipboard_text(text: &str) -> bool {
    if let Ok(mut cb) = arboard::Clipboard::new() {
        cb.set_text(text).is_ok()
    } else {
        false
    }
}

/// 获取剪贴板图片存放的临时缓存目录 (`%TEMP%\clihub_images`)
pub fn get_temp_image_dir() -> PathBuf {
    let dir = std::env::temp_dir().join("clihub_images");
    if !dir.exists() {
        let _ = std::fs::create_dir_all(&dir);
    }
    dir
}

/// 尝试从剪贴板读取位图图像/复制的文件，并将其保存为临时 PNG 文件或格式化路径。
/// 成功时返回带安全引号及空格的路径字符串（如 `"C:\...\clip_xxx.png" `）。
#[cfg(windows)]
pub fn get_clipboard_image_as_temp_file() -> Option<String> {
    use std::ptr::null_mut;
    unsafe extern "system" {
        fn OpenClipboard(h_wnd_new_owner: *mut std::ffi::c_void) -> i32;
        fn CloseClipboard() -> i32;
        fn GetClipboardData(u_format: u32) -> *mut std::ffi::c_void;
        fn GlobalLock(h_mem: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
        fn GlobalUnlock(h_mem: *mut std::ffi::c_void) -> i32;
        fn GlobalSize(h_mem: *mut std::ffi::c_void) -> usize;
        fn RegisterClipboardFormatW(lpsz_format: *const u16) -> u32;
    }

    // 动态加载 shell32.dll 中的 DragQueryFileW（避免静态链接缺失）
    type DragQueryFileWFn = unsafe extern "system" fn(
        h_drop: *mut std::ffi::c_void,
        i_file: u32,
        lpsz_file: *mut u16,
        cch: u32,
    ) -> u32;

    const CF_DIB: u32 = 8;
    const CF_DIBV5: u32 = 17;
    const CF_HDROP: u32 = 15;

    unsafe {
        if OpenClipboard(null_mut()) == 0 {
            return get_clipboard_image_fallback();
        }

        // 1. 优先尝试 CF_HDROP (用户从资源管理器中复制了文件/图片)
        let h_drop = GetClipboardData(CF_HDROP);
        if !h_drop.is_null() {
            if let Ok(lib) = std::ffi::CString::new("shell32.dll") {
                unsafe extern "system" {
                    fn LoadLibraryA(lpLibFileName: *const std::ffi::c_char) -> *mut std::ffi::c_void;
                    fn GetProcAddress(
                        hModule: *mut std::ffi::c_void,
                        lpProcName: *const std::ffi::c_char,
                    ) -> *mut std::ffi::c_void;
                }
                let h_mod = LoadLibraryA(lib.as_ptr());
                if !h_mod.is_null() {
                    let proc_name = std::ffi::CString::new("DragQueryFileW").unwrap();
                    let proc_ptr = GetProcAddress(h_mod, proc_name.as_ptr());
                    if !proc_ptr.is_null() {
                        let drag_query_file: DragQueryFileWFn = std::mem::transmute(proc_ptr);
                        let count = drag_query_file(h_drop, 0xFFFFFFFF, null_mut(), 0);
                        if count > 0 {
                            let mut out_paths = String::new();
                            for i in 0..count {
                                let len = drag_query_file(h_drop, i, null_mut(), 0);
                                if len > 0 {
                                    let mut buf = vec![0u16; (len + 1) as usize];
                                    drag_query_file(h_drop, i, buf.as_mut_ptr(), len + 1);
                                    let s = String::from_utf16_lossy(&buf[..len as usize]);
                                    out_paths.push_str(&format!("\"{s}\" "));
                                }
                            }
                            CloseClipboard();
                            if !out_paths.is_empty() {
                                return Some(out_paths);
                            }
                            if OpenClipboard(null_mut()) == 0 {
                                return get_clipboard_image_fallback();
                            }
                        }
                    }
                }
            }
        }

        // 2. 尝试 Registered "PNG" 剪贴板格式 (浏览器复制图片)
        let png_name: Vec<u16> = "PNG".encode_utf16().chain(std::iter::once(0)).collect();
        let png_format = RegisterClipboardFormatW(png_name.as_ptr());
        if png_format != 0 {
            let handle = GetClipboardData(png_format);
            if !handle.is_null() {
                let size = GlobalSize(handle);
                let ptr = GlobalLock(handle) as *mut u8;
                if !ptr.is_null() && size > 8 {
                    let slice = std::slice::from_raw_parts(ptr, size);
                    if slice.starts_with(b"\x89PNG\r\n\x1a\n") {
                        let dir = get_temp_image_dir();
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default();
                        let filename = format!("clip_{}_{}.png", now.as_secs(), now.subsec_millis());
                        let path = dir.join(filename);
                        if std::fs::write(&path, slice).is_ok() {
                            GlobalUnlock(handle);
                            CloseClipboard();
                            return Some(format!("\"{}\" ", path.to_string_lossy()));
                        }
                    }
                    GlobalUnlock(handle);
                }
            }
        }

        // 3. 尝试 CF_DIB / CF_DIBV5 (微信截图/Snipaste/Win+Shift+S 系统截图)
        for &format_id in &[CF_DIB, CF_DIBV5] {
            let handle = GetClipboardData(format_id);
            if !handle.is_null() {
                let size = GlobalSize(handle);
                let ptr = GlobalLock(handle) as *mut u8;
                if !ptr.is_null() && size >= 40 {
                    let slice = std::slice::from_raw_parts(ptr, size);
                    if let Some(path_str) = parse_dib_and_save_png(slice) {
                        GlobalUnlock(handle);
                        CloseClipboard();
                        return Some(path_str);
                    }
                    GlobalUnlock(handle);
                }
            }
        }

        CloseClipboard();
    }

    get_clipboard_image_fallback()
}

#[cfg(not(windows))]
pub fn get_clipboard_image_as_temp_file() -> Option<String> {
    get_clipboard_image_fallback()
}

/// 解析 Win32 DIB 内存并编码为 PNG 保存到临时目录
#[cfg(windows)]
fn parse_dib_and_save_png(slice: &[u8]) -> Option<String> {
    if slice.len() < 40 {
        return None;
    }
    let header_size = u32::from_le_bytes(slice[0..4].try_into().ok()?) as usize;
    let width = i32::from_le_bytes(slice[4..8].try_into().ok()?);
    let height = i32::from_le_bytes(slice[8..12].try_into().ok()?);
    let planes = u16::from_le_bytes(slice[12..14].try_into().ok()?);
    let bit_count = u16::from_le_bytes(slice[14..16].try_into().ok()?);
    let compression = u32::from_le_bytes(slice[16..20].try_into().ok()?);

    if planes != 1 || width <= 0 || height == 0 {
        return None;
    }

    let abs_height = height.unsigned_abs() as usize;
    let u_width = width as usize;
    let is_bottom_up = height > 0;

    let mut data_offset = header_size;
    if compression == 3 {
        // BI_BITFIELDS
        data_offset += 12;
    }

    let mut rgba = vec![0u8; u_width * abs_height * 4];

    if bit_count == 32 {
        let stride = u_width * 4;
        if slice.len() < data_offset + stride * abs_height {
            return None;
        }

        let mut has_non_zero_alpha = false;
        for y in 0..abs_height {
            let src_y = if is_bottom_up { abs_height - 1 - y } else { y };
            let src_row = &slice[data_offset + src_y * stride..data_offset + (src_y + 1) * stride];
            let dst_row = &mut rgba[y * stride..(y + 1) * stride];
            for x in 0..u_width {
                let b = src_row[x * 4];
                let g = src_row[x * 4 + 1];
                let r = src_row[x * 4 + 2];
                let a = src_row[x * 4 + 3];
                if a > 0 {
                    has_non_zero_alpha = true;
                }
                dst_row[x * 4] = r;
                dst_row[x * 4 + 1] = g;
                dst_row[x * 4 + 2] = b;
                dst_row[x * 4 + 3] = a;
            }
        }
        // 若所有 alpha 均为 0（Windows 传统 32-bit BGRX），统一设为 255
        if !has_non_zero_alpha {
            for i in (3..rgba.len()).step_by(4) {
                rgba[i] = 255;
            }
        }
    } else if bit_count == 24 {
        let stride = ((u_width * 3 + 3) / 4) * 4;
        if slice.len() < data_offset + stride * abs_height {
            return None;
        }

        for y in 0..abs_height {
            let src_y = if is_bottom_up { abs_height - 1 - y } else { y };
            let src_row = &slice[data_offset + src_y * stride..data_offset + (src_y + 1) * stride];
            let dst_row = &mut rgba[y * u_width * 4..(y + 1) * u_width * 4];
            for x in 0..u_width {
                let b = src_row[x * 3];
                let g = src_row[x * 3 + 1];
                let r = src_row[x * 3 + 2];
                dst_row[x * 4] = r;
                dst_row[x * 4 + 1] = g;
                dst_row[x * 4 + 2] = b;
                dst_row[x * 4 + 3] = 255;
            }
        }
    } else {
        return None;
    }

    let dir = get_temp_image_dir();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let filename = format!("clip_{}_{}.png", now.as_secs(), now.subsec_millis());
    let path = dir.join(filename);

    if image::save_buffer(
        &path,
        &rgba,
        u_width as u32,
        abs_height as u32,
        image::ExtendedColorType::Rgba8,
    )
    .is_ok()
    {
        Some(format!("\"{}\" ", path.to_string_lossy()))
    } else {
        None
    }
}

/// 通用回退读取方案（使用 arboard）
fn get_clipboard_image_fallback() -> Option<String> {
    let mut clipboard = arboard::Clipboard::new().ok()?;
    let image_data = clipboard.get_image().ok()?;
    if image_data.width == 0 || image_data.height == 0 || image_data.bytes.is_empty() {
        return None;
    }

    let dir = get_temp_image_dir();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let filename = format!("clip_{}_{}.png", now.as_secs(), now.subsec_millis());
    let path = dir.join(filename);

    if image::save_buffer(
        &path,
        &image_data.bytes,
        image_data.width as u32,
        image_data.height as u32,
        image::ExtendedColorType::Rgba8,
    )
    .is_ok()
    {
        let path_str = path.to_string_lossy().to_string();
        Some(format!("\"{path_str}\" "))
    } else {
        None
    }
}

/// 智能剪贴板读取：
/// 1. 优先读取系统剪贴板纯文本（若包含有效非空文本则返回）；
/// 2. 若无纯文本，尝试提取剪贴板位图图像并自动转存为临时 PNG 文件路径；
/// 3. 若均不存在则返回 None。
pub fn smart_get_clipboard_content() -> Option<String> {
    if let Some(text) = get_clipboard_text() {
        if !text.is_empty() {
            return Some(text);
        }
    }
    get_clipboard_image_as_temp_file()
}

/// 清理超过 24 小时的历史临时图片文件，避免磁盘垃圾堆积（后台异步运行，不阻塞 UI）
pub fn cleanup_old_temp_images() {
    std::thread::spawn(|| {
        let dir = std::env::temp_dir().join("clihub_images");
        if !dir.exists() {
            return;
        }
        let now = std::time::SystemTime::now();
        let max_age = std::time::Duration::from_secs(24 * 3600); // 24 小时

        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if let Ok(meta) = path.metadata() {
                        if let Ok(modified) = meta.modified() {
                            if let Ok(age) = now.duration_since(modified) {
                                if age > max_age {
                                    let _ = std::fs::remove_file(path);
                                }
                            }
                        }
                    }
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_temp_image_dir_creation() {
        let dir = get_temp_image_dir();
        assert!(dir.exists(), "临时图片目录必须成功创建");
    }

    #[test]
    fn test_save_test_image() {
        let dir = get_temp_image_dir();
        let test_path = dir.join("test_verify.png");
        let dummy_pixels = vec![255u8; 10 * 10 * 4]; // 10x10 白色 RGBA 图片
        let res = image::save_buffer(
            &test_path,
            &dummy_pixels,
            10,
            10,
            image::ExtendedColorType::Rgba8,
        );
        assert!(res.is_ok(), "应当成功保存 PNG 图片");
        assert!(test_path.exists(), "测试图片文件必须存在于临时目录");
        let _ = std::fs::remove_file(test_path);
    }
}
