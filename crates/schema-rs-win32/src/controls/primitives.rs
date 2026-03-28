//! Low-level Win32 control creation helpers.

use std::ptr;

use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Controls::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use crate::state::*;
use crate::util::*;

pub fn create_static(form: &mut SchemaFormWindow, text: &str, x: i32, w: i32) {
    let wide = to_wide(text);
    let cls = to_wide("STATIC");
    let h = LABEL_HEIGHT;
    let hwnd = unsafe {
        CreateWindowExW(
            0,
            cls.as_ptr(),
            wide.as_ptr(),
            WS_CHILD | WS_VISIBLE,
            x,
            form.y_cursor,
            w,
            h,
            form.hwnd,
            0 as _,
            GetModuleHandleW(ptr::null()),
            ptr::null(),
        )
    };
    if !hwnd.is_null() {
        unsafe { SendMessageW(hwnd, WM_SETFONT, form.hfont as usize, 1) };
        form.controls.push(ControlEntry {
            hwnd,
            path: String::new(),
            kind: ControlKind::Label,
        });
    }
    form.y_cursor += h + 2;
}

pub fn create_edit(
    form: &mut SchemaFormWindow,
    path: &str,
    x: i32,
    w: i32,
    text: &str,
    read_only: bool,
    kind: ControlKind,
) {
    let id = alloc_id(form);
    let cls = to_wide("EDIT");
    let wide = to_wide(text);

    let (extra_style, h) = match kind {
        ControlKind::EditMultiline => (
            ES_MULTILINE as u32 | ES_AUTOVSCROLL as u32 | ES_WANTRETURN as u32 | WS_VSCROLL,
            FIELD_HEIGHT * 4,
        ),
        ControlKind::EditPassword => (ES_PASSWORD as u32 | ES_AUTOHSCROLL as u32, FIELD_HEIGHT),
        _ => (ES_AUTOHSCROLL as u32, FIELD_HEIGHT),
    };

    let mut style = WS_CHILD | WS_VISIBLE | WS_BORDER | WS_TABSTOP | extra_style;
    if read_only {
        style |= ES_READONLY as u32;
    }

    let hwnd = unsafe {
        CreateWindowExW(
            0,
            cls.as_ptr(),
            wide.as_ptr(),
            style,
            x,
            form.y_cursor,
            w,
            h,
            form.hwnd,
            id as HMENU,
            GetModuleHandleW(ptr::null()),
            ptr::null(),
        )
    };
    if !hwnd.is_null() {
        unsafe { SendMessageW(hwnd, WM_SETFONT, form.hfont as usize, 1) };
        register_control(form, hwnd, path, kind);
    }
    form.y_cursor += h + SPACING;
}

pub fn create_combo(
    form: &mut SchemaFormWindow,
    path: &str,
    x: i32,
    w: i32,
    items: &[String],
    selected: &str,
) {
    let id = alloc_id(form);
    let cls = to_wide("COMBOBOX");
    let empty = to_wide("");
    let drop_h = FIELD_HEIGHT * (items.len() as i32 + 1).min(10);

    let hwnd = unsafe {
        CreateWindowExW(
            0,
            cls.as_ptr(),
            empty.as_ptr(),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | CBS_DROPDOWNLIST as u32 | CBS_HASSTRINGS as u32,
            x,
            form.y_cursor,
            w,
            drop_h,
            form.hwnd,
            id as HMENU,
            GetModuleHandleW(ptr::null()),
            ptr::null(),
        )
    };
    if !hwnd.is_null() {
        unsafe { SendMessageW(hwnd, WM_SETFONT, form.hfont as usize, 1) };
        let mut sel_idx: isize = -1;
        for (i, item) in items.iter().enumerate() {
            let wi = to_wide(item);
            unsafe { SendMessageW(hwnd, CB_ADDSTRING, 0, wi.as_ptr() as _) };
            if item == selected {
                sel_idx = i as isize;
            }
        }
        if sel_idx >= 0 {
            unsafe { SendMessageW(hwnd, CB_SETCURSEL, sel_idx as usize, 0) };
        }
        register_control(form, hwnd, path, ControlKind::ComboBox);
    }
    form.y_cursor += FIELD_HEIGHT + SPACING;
}

pub fn create_checkbox(
    form: &mut SchemaFormWindow,
    path: &str,
    x: i32,
    w: i32,
    label: &str,
    checked: bool,
    read_only: bool,
) {
    let id = alloc_id(form);
    let cls = to_wide("BUTTON");
    let wide = to_wide(label);
    let mut style = WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_AUTOCHECKBOX as u32;
    if read_only {
        style |= WS_DISABLED;
    }
    let hwnd = unsafe {
        CreateWindowExW(
            0,
            cls.as_ptr(),
            wide.as_ptr(),
            style,
            x,
            form.y_cursor,
            w,
            FIELD_HEIGHT,
            form.hwnd,
            id as HMENU,
            GetModuleHandleW(ptr::null()),
            ptr::null(),
        )
    };
    if !hwnd.is_null() {
        unsafe {
            SendMessageW(hwnd, WM_SETFONT, form.hfont as usize, 1);
            if checked {
                SendMessageW(hwnd, BM_SETCHECK, BST_CHECKED as usize, 0);
            }
        }
        register_control(form, hwnd, path, ControlKind::CheckBox);
    }
    form.y_cursor += FIELD_HEIGHT + SPACING;
}

pub fn create_button(
    form: &mut SchemaFormWindow,
    path: &str,
    x: i32,
    w: i32,
    text: &str,
    kind: ControlKind,
) {
    let id = alloc_id(form);
    let cls = to_wide("BUTTON");
    let wide = to_wide(text);
    let btn_w = w.min(120);
    let hwnd = unsafe {
        CreateWindowExW(
            0,
            cls.as_ptr(),
            wide.as_ptr(),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_PUSHBUTTON as u32,
            x,
            form.y_cursor,
            btn_w,
            FIELD_HEIGHT,
            form.hwnd,
            id as HMENU,
            GetModuleHandleW(ptr::null()),
            ptr::null(),
        )
    };
    if !hwnd.is_null() {
        unsafe { SendMessageW(hwnd, WM_SETFONT, form.hfont as usize, 1) };
        register_control(form, hwnd, path, kind);
    }
    form.y_cursor += FIELD_HEIGHT + SPACING;
}
