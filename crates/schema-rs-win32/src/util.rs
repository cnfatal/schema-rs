//! Utility functions: encoding helpers, text extraction, font creation,
//! and field label formatting.

use schema_rs_core::FieldNode;
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

// ── Encoding helpers ──

/// Convert a Rust `&str` to a null-terminated wide (UTF-16) string.
pub fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Convert a null-terminated wide string buffer to a Rust `String`.
pub fn from_wide(buf: &[u16]) -> String {
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..len])
}

// ── Win32 text extraction ──

pub fn get_window_text(hwnd: HWND) -> String {
    let len = unsafe { GetWindowTextLengthW(hwnd) };
    if len <= 0 {
        return String::new();
    }
    let mut buf = vec![0u16; (len + 1) as usize];
    unsafe { GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32) };
    from_wide(&buf)
}

// ── Font ──

pub fn create_default_font() -> HFONT {
    let face = to_wide("Segoe UI");
    unsafe {
        CreateFontW(
            -16,
            0,
            0,
            0,
            FW_NORMAL as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET as u32,
            OUT_DEFAULT_PRECIS as u32,
            CLIP_DEFAULT_PRECIS as u32,
            CLEARTYPE_QUALITY as u32,
            DEFAULT_PITCH as u32,
            face.as_ptr(),
        )
    }
}

// ── Field label helpers ──

pub fn field_label(node: &FieldNode) -> String {
    node.schema
        .title
        .clone()
        .unwrap_or_else(|| path_last_segment(&node.instance_location))
}

pub fn label_text(node: &FieldNode) -> String {
    let label = field_label(node);
    if node.required {
        format!("{label} *")
    } else {
        label
    }
}

pub fn path_last_segment(path: &str) -> String {
    if path.is_empty() {
        return "Root".to_string();
    }
    path.rsplit('/').next().unwrap_or(path).to_string()
}

// ── Validation ──

pub fn format_first_error(vo: &schema_rs_core::ValidationOutput) -> Option<String> {
    if let Some(err) = &vo.error {
        return Some(err.key.clone());
    }
    for child in &vo.errors {
        if let Some(msg) = format_first_error(child) {
            return Some(msg);
        }
    }
    None
}
