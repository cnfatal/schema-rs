//! Win32 window procedure, event handling, scrollbar, and form rebuild.

use std::ptr;

use schema_rs_core::{JsonValue, SchemaRuntime};
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::UI::Controls::*;
use windows_sys::Win32::UI::Controls::Dialogs::*;
use windows_sys::Win32::UI::Shell::{SHBrowseForFolderW, SHGetPathFromIDListW, BROWSEINFOW, BIF_NEWDIALOGSTYLE};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use crate::controls::build_controls_recursive;
use crate::state::*;
use crate::util::*;

// ── WndProc ──

pub unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_COMMAND => {
            let notify_code = ((wparam >> 16) & 0xFFFF) as u16;
            let ctrl_id = (wparam & 0xFFFF) as u16;

            match ctrl_id {
                ID_OK_BUTTON => {
                    if notify_code == BN_CLICKED as u16 {
                        FORM_STATE.with(|cell| {
                            if let Ok(mut state) = cell.try_borrow_mut() {
                                if let Some(ref mut s) = *state {
                                    s.form.confirmed = true;
                                }
                            }
                        });
                        unsafe { DestroyWindow(hwnd) };
                    }
                }
                ID_CANCEL_BUTTON => {
                    if notify_code == BN_CLICKED as u16 {
                        unsafe { DestroyWindow(hwnd) };
                    }
                }
                _ => handle_command(ctrl_id, notify_code),
            }
            0
        }
        WM_NOTIFY => {
            let nmhdr = unsafe { &*(lparam as *const NMHDR) };
            if nmhdr.code == TCN_SELCHANGE {
                handle_tab_change(nmhdr.idFrom as u16);
            }
            0
        }
        WM_VSCROLL => {
            let action = (wparam & 0xFFFF) as i32;
            FORM_STATE.with(|cell| {
                if let Ok(mut state) = cell.try_borrow_mut() {
                    if let Some(ref mut s) = *state {
                        let delta = match action {
                            SB_LINEUP => -FIELD_HEIGHT,
                            SB_LINEDOWN => FIELD_HEIGHT,
                            SB_PAGEUP => -200,
                            SB_PAGEDOWN => 200,
                            SB_THUMBPOSITION | SB_THUMBTRACK => {
                                let pos = ((wparam >> 16) & 0xFFFF) as i32;
                                pos - s.form.scroll_y
                            }
                            _ => 0,
                        };
                        do_scroll(&mut s.form, delta);
                    }
                }
            });
            0
        }
        WM_MOUSEWHEEL => {
            let wheel_delta = ((wparam >> 16) & 0xFFFF) as i16;
            let scroll = -(wheel_delta as i32) / 3;
            FORM_STATE.with(|cell| {
                if let Ok(mut state) = cell.try_borrow_mut() {
                    if let Some(ref mut s) = *state {
                        do_scroll(&mut s.form, scroll);
                    }
                }
            });
            0
        }
        WM_SIZE => {
            FORM_STATE.with(|cell| {
                if let Ok(mut state) = cell.try_borrow_mut() {
                    if let Some(ref mut s) = *state {
                        let new_w = (lparam & 0xFFFF) as i32;
                        if new_w != s.form.client_width {
                            s.form.client_width = new_w;
                            rebuild_form(&mut s.form, &s.runtime);
                        } else {
                            update_scrollbar(&s.form);
                        }
                    }
                }
            });
            0
        }
        WM_DESTROY => {
            unsafe { PostQuitMessage(0) };
            0
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

// ── Command handler ──

/// Process a WM_COMMAND for a form control. Returns `true` if the runtime
/// value changed and the form should be rebuilt.
pub fn handle_control_command(
    form: &mut SchemaFormWindow,
    runtime: &mut SchemaRuntime,
    ctrl_id: u16,
    notify_code: u16,
) -> bool {
    let entry_idx = match form.id_map.get(&ctrl_id) {
        Some(&idx) => idx,
        None => return false,
    };

    let path = form.controls[entry_idx].path.clone();
    let kind = form.controls[entry_idx].kind;
    let hwnd = form.controls[entry_idx].hwnd;

    let mut changed = false;

    match kind {
        ControlKind::Edit | ControlKind::EditPassword | ControlKind::EditMultiline => {
            if notify_code == EN_KILLFOCUS as u16 {
                let text = get_window_text(hwnd);
                changed = runtime.set_value(&path, JsonValue::String(text));
            }
        }
        ControlKind::NumberEdit => {
            if notify_code == EN_KILLFOCUS as u16 {
                let text = get_window_text(hwnd);
                if let Ok(n) = text.parse::<i64>() {
                    changed = runtime.set_value(&path, JsonValue::from(n));
                } else if let Ok(f) = text.parse::<f64>() {
                    if let Some(num) = serde_json::Number::from_f64(f) {
                        changed = runtime.set_value(&path, JsonValue::Number(num));
                    }
                }
            }
        }
        ControlKind::ComboBox => {
            if notify_code == CBN_SELCHANGE as u16 {
                let idx = unsafe { SendMessageW(hwnd, CB_GETCURSEL, 0, 0) };
                if idx >= 0 {
                    let len = unsafe { SendMessageW(hwnd, CB_GETLBTEXTLEN, idx as usize, 0) };
                    if len > 0 {
                        let mut buf = vec![0u16; (len + 1) as usize];
                        unsafe {
                            SendMessageW(hwnd, CB_GETLBTEXT, idx as usize, buf.as_mut_ptr() as _);
                        }
                        let text = from_wide(&buf);
                        changed = runtime.set_value(&path, JsonValue::String(text));
                    }
                }
            }
        }
        ControlKind::CheckBox => {
            if notify_code == BN_CLICKED as u16 {
                let state = unsafe { SendMessageW(hwnd, BM_GETCHECK, 0, 0) };
                let checked = state == BST_CHECKED as isize;
                changed = runtime.set_value(&path, JsonValue::Bool(checked));
            }
        }
        ControlKind::AddButton => {
            if notify_code == BN_CLICKED as u16 {
                changed = runtime.add_child(&path, None, None);
            }
        }
        ControlKind::RemoveButton => {
            if notify_code == BN_CLICKED as u16 {
                changed = runtime.remove_value(&path);
            }
        }
        ControlKind::BrowseButton => {
            if notify_code == BN_CLICKED as u16 {
                // Find the paired Edit control (registered just before this button).
                let edit_hwnd = find_edit_for_browse(form, &path);
                if let Some(edit_hwnd) = edit_hwnd {
                    // Determine if this is file-path or directory-path from the runtime node.
                    let is_dir = runtime
                        .get_node(&path)
                        .and_then(|n| n.schema.format.as_deref())
                        .map(|f| f == "directory-path")
                        .unwrap_or(false);

                    if let Some(picked) = if is_dir {
                        show_folder_dialog(form.hwnd)
                    } else {
                        show_file_open_dialog(form.hwnd)
                    } {
                        let wide = to_wide(&picked);
                        unsafe { SetWindowTextW(edit_hwnd, wide.as_ptr()) };
                        changed = runtime.set_value(&path, JsonValue::String(picked));
                    }
                }
            }
        }
        ControlKind::TabControl | ControlKind::Label | ControlKind::ConfirmButton | ControlKind::CancelButton => {}
    }

    if changed {
        let _ = runtime.drain_events();
    }
    changed
}

fn handle_command(ctrl_id: u16, notify_code: u16) {
    FORM_STATE.with(|cell| {
        let Ok(mut state) = cell.try_borrow_mut() else { return };
        let Some(ref mut s) = *state else { return };

        if handle_control_command(&mut s.form, &mut s.runtime, ctrl_id, notify_code) {
            if let Some(ref mut cb) = s.on_change {
                cb(s.runtime.get_root_value());
            }
            rebuild_form(&mut s.form, &s.runtime);
        }
    });
}

fn handle_tab_change(ctrl_id: u16) {
    FORM_STATE.with(|cell| {
        let Ok(mut state) = cell.try_borrow_mut() else { return };
        let Some(ref mut s) = *state else { return };

        if let Some(&idx) = s.form.id_map.get(&ctrl_id) {
            if s.form.controls[idx].kind == ControlKind::TabControl {
                let hwnd_tab = s.form.controls[idx].hwnd;
                let path = s.form.controls[idx].path.clone();
                let sel = unsafe { SendMessageW(hwnd_tab, TCM_GETCURSEL, 0, 0) } as usize;
                s.form.tab_selection.insert(path, sel);
                rebuild_form(&mut s.form, &s.runtime);
            }
        }
    });
}

/// Handle tab change in a custom window procedure (playground).
pub fn handle_tab_change_ext(form: &mut SchemaFormWindow, ctrl_id: u16) -> bool {
    if let Some(&idx) = form.id_map.get(&ctrl_id) {
        if form.controls[idx].kind == ControlKind::TabControl {
            let hwnd_tab = form.controls[idx].hwnd;
            let path = form.controls[idx].path.clone();
            let sel = unsafe { SendMessageW(hwnd_tab, TCM_GETCURSEL, 0, 0) } as usize;
            form.tab_selection.insert(path, sel);
            return true;
        }
    }
    false
}

// ── File/Folder dialogs ──

fn find_edit_for_browse(form: &SchemaFormWindow, path: &str) -> Option<HWND> {
    // The Edit control is the one with the same path and kind Edit.
    for entry in &form.controls {
        if entry.path == path && entry.kind == ControlKind::Edit {
            return Some(entry.hwnd);
        }
    }
    None
}

fn show_file_open_dialog(owner: HWND) -> Option<String> {
    let mut buf = [0u16; 260];
    let filter = to_wide("All Files\0*.*\0\0");
    let mut ofn: OPENFILENAMEW = unsafe { std::mem::zeroed() };
    ofn.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
    ofn.hwndOwner = owner;
    ofn.lpstrFilter = filter.as_ptr();
    ofn.lpstrFile = buf.as_mut_ptr();
    ofn.nMaxFile = buf.len() as u32;
    ofn.Flags = OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR;
    let ok = unsafe { GetOpenFileNameW(&mut ofn) };
    if ok != 0 {
        Some(from_wide(&buf))
    } else {
        None
    }
}

fn show_folder_dialog(owner: HWND) -> Option<String> {
    let title = to_wide("Select Folder");
    let bi = BROWSEINFOW {
        hwndOwner: owner,
        pidlRoot: ptr::null_mut(),
        pszDisplayName: ptr::null_mut(),
        lpszTitle: title.as_ptr(),
        ulFlags: BIF_NEWDIALOGSTYLE,
        lpfn: None,
        lParam: 0,
        iImage: 0,
    };
    let pidl = unsafe { SHBrowseForFolderW(&bi) };
    if pidl.is_null() {
        return None;
    }
    let mut buf = [0u16; 260];
    let ok = unsafe { SHGetPathFromIDListW(pidl, buf.as_mut_ptr()) };
    // Free the PIDL.
    unsafe {
        windows_sys::Win32::System::Com::CoTaskMemFree(pidl as _);
    }
    if ok != 0 {
        Some(from_wide(&buf))
    } else {
        None
    }
}

// ── Rebuild / Scrollbar ──

pub fn rebuild_form(form: &mut SchemaFormWindow, runtime: &SchemaRuntime) {
    unsafe { SendMessageW(form.hwnd, WM_SETREDRAW, 0, 0) };

    for entry in form.controls.drain(..) {
        unsafe { DestroyWindow(entry.hwnd) };
    }
    form.id_map.clear();
    form.next_id = ID_CONTROL_BASE;
    form.y_cursor = SPACING;

    build_controls_recursive(form, runtime, runtime.root(), 0);
    form.content_height = form.y_cursor + SPACING;
    update_scrollbar(form);

    unsafe {
        SendMessageW(form.hwnd, WM_SETREDRAW, 1, 0);
        RedrawWindow(form.hwnd, ptr::null(), 0 as _, RDW_ERASE | RDW_FRAME | RDW_INVALIDATE | RDW_ALLCHILDREN | RDW_UPDATENOW);
    };
}

pub fn update_scrollbar(form: &SchemaFormWindow) {
    let mut rc: RECT = RECT { left: 0, top: 0, right: 0, bottom: 0 };
    unsafe { GetClientRect(form.hwnd, &mut rc) };
    let page = (rc.bottom - rc.top) as u32;
    let si = SCROLLINFO {
        cbSize: std::mem::size_of::<SCROLLINFO>() as u32,
        fMask: SIF_RANGE | SIF_PAGE | SIF_POS,
        nMin: 0,
        nMax: form.content_height,
        nPage: page,
        nPos: form.scroll_y,
        nTrackPos: 0,
    };
    unsafe { SetScrollInfo(form.hwnd, SB_VERT, &si, 1) };
}

fn do_scroll(form: &mut SchemaFormWindow, delta: i32) {
    let old = form.scroll_y;
    form.scroll_y += delta;
    form.scroll_y = form.scroll_y.max(0);
    let max = (form.content_height - 400).max(0);
    form.scroll_y = form.scroll_y.min(max);
    let diff = form.scroll_y - old;
    if diff != 0 {
        unsafe {
            ScrollWindow(form.hwnd, 0, -diff, ptr::null(), ptr::null());
        }
        let si = SCROLLINFO {
            cbSize: std::mem::size_of::<SCROLLINFO>() as u32,
            fMask: SIF_POS,
            nMin: 0,
            nMax: 0,
            nPage: 0,
            nPos: form.scroll_y,
            nTrackPos: 0,
        };
        unsafe {
            SetScrollInfo(form.hwnd, SB_VERT, &si, 1);
            UpdateWindow(form.hwnd);
        }
    }
}
