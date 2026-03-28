//! String field controls: text edit, combo box, password, textarea, file path.

use std::ptr;

use schema_rs_core::{FieldNode, JsonValue, SchemaRuntime};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use crate::state::*;
use crate::util::*;

use super::primitives::*;

pub fn build_string_control(
    form: &mut SchemaFormWindow,
    runtime: &SchemaRuntime,
    node: &FieldNode,
    x: i32,
    w: i32,
) {
    let label = label_text(node);
    create_static(form, &label, x, w);

    if let Some(desc) = &node.schema.description {
        create_static(form, desc, x, w);
    }

    let value = runtime
        .get_value(&node.instance_location)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_owned();
    let read_only = node.schema.read_only.unwrap_or(false) || node.schema.const_.is_some();

    if let Some(enum_values) = &node.schema.enum_ {
        let items: Vec<String> = enum_values
            .iter()
            .map(|v| match v {
                JsonValue::String(s) => s.clone(),
                other => other.to_string(),
            })
            .collect();
        create_combo(form, &node.instance_location, x, w, &items, &value);
    } else {
        let format = node.schema.format.as_deref().unwrap_or("");
        match format {
            "textarea" => create_edit(form, &node.instance_location, x, w, &value, read_only, ControlKind::EditMultiline),
            "password" => create_edit(form, &node.instance_location, x, w, &value, read_only, ControlKind::EditPassword),
            "file-path" | "directory-path" => {
                create_file_path_control(form, &node.instance_location, x, w, &value, read_only);
            }
            _ => create_edit(form, &node.instance_location, x, w, &value, read_only, ControlKind::Edit),
        }
    }
}

/// Create a file/directory path control: Edit + Browse button side by side.
pub fn create_file_path_control(
    form: &mut SchemaFormWindow,
    path: &str,
    x: i32,
    w: i32,
    text: &str,
    read_only: bool,
) {
    let btn_w = 80;
    let edit_w = (w - btn_w - 4).max(60);

    // Edit control for the path text.
    let edit_id = alloc_id(form);
    let cls = to_wide("EDIT");
    let wide = to_wide(text);
    let mut style = WS_CHILD | WS_VISIBLE | WS_BORDER | WS_TABSTOP | ES_AUTOHSCROLL as u32;
    if read_only {
        style |= ES_READONLY as u32;
    }
    let hwnd_edit = unsafe {
        CreateWindowExW(
            0,
            cls.as_ptr(),
            wide.as_ptr(),
            style,
            x,
            form.y_cursor,
            edit_w,
            FIELD_HEIGHT,
            form.hwnd,
            edit_id as HMENU,
            GetModuleHandleW(ptr::null()),
            ptr::null(),
        )
    };
    if !hwnd_edit.is_null() {
        unsafe { SendMessageW(hwnd_edit, WM_SETFONT, form.hfont as usize, 1) };
        register_control(form, hwnd_edit, path, ControlKind::Edit);
    }

    // Browse button.
    if !read_only {
        let btn_id = alloc_id(form);
        let btn_cls = to_wide("BUTTON");
        let btn_text = to_wide("Browse…");
        let hwnd_btn = unsafe {
            CreateWindowExW(
                0,
                btn_cls.as_ptr(),
                btn_text.as_ptr(),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_PUSHBUTTON as u32,
                x + edit_w + 4,
                form.y_cursor,
                btn_w,
                FIELD_HEIGHT,
                form.hwnd,
                btn_id as HMENU,
                GetModuleHandleW(ptr::null()),
                ptr::null(),
            )
        };
        if !hwnd_btn.is_null() {
            unsafe { SendMessageW(hwnd_btn, WM_SETFONT, form.hfont as usize, 1) };
            register_control(form, hwnd_btn, path, ControlKind::BrowseButton);
        }
    }
    form.y_cursor += FIELD_HEIGHT + SPACING;
}
