//! Native Win32 playground backend with schema selector.

use std::cell::RefCell;
use std::collections::HashMap;
use std::ptr;
use std::sync::atomic::{AtomicPtr, Ordering};

use schema_rs::core::{DefaultValidator, SchemaRuntime};
use schema_rs::win32::controls::build_controls_recursive;
use schema_rs::win32::state::*;
use schema_rs::win32::util::*;
use schema_rs::win32::wndproc::{handle_control_command, handle_tab_change_ext, update_scrollbar};
use serde_json::Value;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::Console::SetConsoleCtrlHandler;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Controls::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use crate::schemas::example_schemas;

static PG_HWND: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(ptr::null_mut());

unsafe extern "system" fn console_ctrl_handler(ctrl_type: u32) -> BOOL {
    let _ = ctrl_type;
    let hwnd = PG_HWND.load(Ordering::SeqCst);
    if !hwnd.is_null() {
        unsafe { PostMessageW(hwnd, WM_CLOSE, 0, 0) };
    }
    1
}

// ── Constants ──

const ID_SCHEMA_COMBO: u16 = 1;
const TOOLBAR_HEIGHT: i32 = 36;

// ── Playground state ──

struct PlaygroundState {
    form: SchemaFormWindow,
    runtime: SchemaRuntime,
    examples: Vec<(&'static str, Value, Value)>,
    current_idx: usize,
    hwnd_combo: HWND,
}

thread_local! {
    static PG_STATE: RefCell<Option<PlaygroundState>> = const { RefCell::new(None) };
}

// ── Public entry ──

pub fn run() {
    let examples = example_schemas();
    let (name, schema, value) = &examples[0];

    let mut runtime = SchemaRuntime::new(
        Box::new(DefaultValidator::new()),
        schema.clone(),
        value.clone(),
    );
    let _ = runtime.drain_events();

    let class_name = to_wide("SchemaRsPlaygroundClass");
    let h_instance = unsafe { GetModuleHandleW(ptr::null()) };

    let icc = INITCOMMONCONTROLSEX {
        dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
        dwICC: ICC_STANDARD_CLASSES | ICC_WIN95_CLASSES,
    };
    unsafe { InitCommonControlsEx(&icc) };

    let wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        style: 0,
        lpfnWndProc: Some(wnd_proc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: h_instance,
        hIcon: unsafe { LoadIconW(0 as _, IDI_APPLICATION) },
        hCursor: unsafe { LoadCursorW(0 as _, IDC_ARROW) },
        hbrBackground: (COLOR_BTNFACE + 1) as HBRUSH,
        lpszMenuName: ptr::null(),
        lpszClassName: class_name.as_ptr(),
        hIconSm: 0 as _,
    };
    unsafe { RegisterClassExW(&wc) };

    let title = format!("schema-rs playground — {name}");
    let title_wide = to_wide(&title);
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_COMPOSITED,
            class_name.as_ptr(),
            title_wide.as_ptr(),
            WS_OVERLAPPEDWINDOW | WS_VSCROLL | WS_CLIPCHILDREN,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            720,
            640,
            0 as _,
            0 as _,
            h_instance,
            ptr::null(),
        )
    };

    let hfont = create_default_font();

    // ── Schema selector ComboBox ──
    let cls_combo = to_wide("COMBOBOX");
    let empty = to_wide("");
    let hwnd_combo = unsafe {
        CreateWindowExW(
            0,
            cls_combo.as_ptr(),
            empty.as_ptr(),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | CBS_DROPDOWNLIST as u32 | CBS_HASSTRINGS as u32,
            LEFT_MARGIN,
            4,
            400,
            TOOLBAR_HEIGHT * 10,
            hwnd,
            ID_SCHEMA_COMBO as HMENU,
            h_instance,
            ptr::null(),
        )
    };
    if !hwnd_combo.is_null() {
        unsafe { SendMessageW(hwnd_combo, WM_SETFONT, hfont as usize, 1) };
        for (i, (name, _, _)) in examples.iter().enumerate() {
            let wi = to_wide(name);
            unsafe { SendMessageW(hwnd_combo, CB_ADDSTRING, 0, wi.as_ptr() as _) };
            if i == 0 {
                unsafe { SendMessageW(hwnd_combo, CB_SETCURSEL, 0, 0) };
            }
        }
    }

    // ── Label ──
    let label_cls = to_wide("STATIC");
    let label_text_w = to_wide("Example:");
    let hwnd_label = unsafe {
        CreateWindowExW(
            0,
            label_cls.as_ptr(),
            label_text_w.as_ptr(),
            WS_CHILD | WS_VISIBLE,
            LEFT_MARGIN + 410,
            10,
            60,
            20,
            hwnd,
            0 as _,
            h_instance,
            ptr::null(),
        )
    };
    if !hwnd_label.is_null() {
        unsafe { SendMessageW(hwnd_label, WM_SETFONT, hfont as usize, 1) };
    }

    // ── Form controls below toolbar ──
    let mut form = SchemaFormWindow {
        hwnd,
        controls: Vec::new(),
        id_map: HashMap::new(),
        next_id: ID_CONTROL_BASE,
        y_cursor: TOOLBAR_HEIGHT + SPACING,
        client_width: 680,
        content_height: 0,
        scroll_y: 0,
        hfont,
        tab_selection: HashMap::new(),
    };

    build_controls_recursive(&mut form, &runtime, runtime.root(), 0);
    form.content_height = form.y_cursor + SPACING;
    update_scrollbar(&form);

    PG_STATE.with(|cell| {
        *cell.borrow_mut() = Some(PlaygroundState {
            form,
            runtime,
            examples,
            current_idx: 0,
            hwnd_combo,
        });
    });

    // Register console Ctrl+C handler.
    PG_HWND.store(hwnd, Ordering::SeqCst);
    unsafe { SetConsoleCtrlHandler(Some(console_ctrl_handler), 1) };

    unsafe {
        ShowWindow(hwnd, SW_SHOW);
        UpdateWindow(hwnd);
    }

    let mut msg: MSG = unsafe { std::mem::zeroed() };
    loop {
        let ret = unsafe { GetMessageW(&mut msg, 0 as _, 0, 0) };
        if ret <= 0 {
            break;
        }
        unsafe {
            if IsDialogMessageW(hwnd, &msg) == 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }

    // Cleanup.
    PG_HWND.store(ptr::null_mut(), Ordering::SeqCst);
    unsafe { SetConsoleCtrlHandler(Some(console_ctrl_handler), 0) };
    PG_STATE.with(|cell| {
        if let Some(pg) = cell.borrow_mut().take() {
            let result = pg.runtime.get_root_value().clone();
            unsafe { DeleteObject(pg.form.hfont as _) };
            println!(
                "Final value:\n{}",
                serde_json::to_string_pretty(&result).unwrap()
            );
        }
    });
}

// ── WndProc ──

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_COMMAND => {
            let notify_code = ((wparam >> 16) & 0xFFFF) as u16;
            let ctrl_id = (wparam & 0xFFFF) as u16;

            if ctrl_id == ID_SCHEMA_COMBO && notify_code == CBN_SELCHANGE as u16 {
                handle_schema_change();
            } else {
                // Delegate to form controls.
                PG_STATE.with(|cell| {
                    if let Ok(mut state) = cell.try_borrow_mut() {
                        if let Some(ref mut pg) = *state {
                            if handle_control_command(
                                &mut pg.form,
                                &mut pg.runtime,
                                ctrl_id,
                                notify_code,
                            ) {
                                pg_rebuild_form(pg);
                            }
                        }
                    }
                });
            }
            0
        }
        WM_NOTIFY => {
            let nmhdr = unsafe { &*(lparam as *const NMHDR) };
            if nmhdr.code == TCN_SELCHANGE {
                PG_STATE.with(|cell| {
                    if let Ok(mut state) = cell.try_borrow_mut() {
                        if let Some(ref mut pg) = *state {
                            if handle_tab_change_ext(&mut pg.form, nmhdr.idFrom as u16) {
                                pg_rebuild_form(pg);
                            }
                        }
                    }
                });
            }
            0
        }
        WM_VSCROLL => {
            let action = (wparam & 0xFFFF) as i32;
            PG_STATE.with(|cell| {
                if let Ok(mut state) = cell.try_borrow_mut() {
                    if let Some(ref mut pg) = *state {
                        let delta = match action {
                            SB_LINEUP => -FIELD_HEIGHT,
                            SB_LINEDOWN => FIELD_HEIGHT,
                            SB_PAGEUP => -200,
                            SB_PAGEDOWN => 200,
                            SB_THUMBPOSITION | SB_THUMBTRACK => {
                                let pos = ((wparam >> 16) & 0xFFFF) as i32;
                                pos - pg.form.scroll_y
                            }
                            _ => 0,
                        };
                        do_scroll(&mut pg.form, delta);
                    }
                }
            });
            0
        }
        WM_MOUSEWHEEL => {
            let wheel_delta = ((wparam >> 16) & 0xFFFF) as i16;
            let scroll = -(wheel_delta as i32) / 3;
            PG_STATE.with(|cell| {
                if let Ok(mut state) = cell.try_borrow_mut() {
                    if let Some(ref mut pg) = *state {
                        do_scroll(&mut pg.form, scroll);
                    }
                }
            });
            0
        }
        WM_SIZE => {
            PG_STATE.with(|cell| {
                if let Ok(mut state) = cell.try_borrow_mut() {
                    if let Some(ref mut pg) = *state {
                        let new_w = (lparam & 0xFFFF) as i32;
                        if new_w != pg.form.client_width {
                            pg.form.client_width = new_w;
                            pg_rebuild_form(pg);
                        } else {
                            update_scrollbar(&pg.form);
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

// ── Schema switching ──

fn handle_schema_change() {
    PG_STATE.with(|cell| {
        let Ok(mut state) = cell.try_borrow_mut() else { return };
        let Some(ref mut pg) = *state else { return };

        let idx = unsafe { SendMessageW(pg.hwnd_combo, CB_GETCURSEL, 0, 0) };
        if idx < 0 || idx as usize == pg.current_idx {
            return;
        }
        let idx = idx as usize;
        if idx >= pg.examples.len() {
            return;
        }

        pg.current_idx = idx;
        let (name, schema, value) = &pg.examples[idx];

        // Update window title.
        let title = format!("schema-rs playground — {name}");
        let title_wide = to_wide(&title);
        unsafe { SetWindowTextW(pg.form.hwnd, title_wide.as_ptr()) };

        // Create new runtime.
        let mut runtime = SchemaRuntime::new(
            Box::new(DefaultValidator::new()),
            schema.clone(),
            value.clone(),
        );
        let _ = runtime.drain_events();
        pg.runtime = runtime;
        pg.form.tab_selection.clear();

        pg_rebuild_form(pg);
    });
}

// ── Rebuild helpers ──

fn pg_rebuild_form(pg: &mut PlaygroundState) {
    unsafe { SendMessageW(pg.form.hwnd, WM_SETREDRAW, 0, 0) };

    for entry in pg.form.controls.drain(..) {
        unsafe { DestroyWindow(entry.hwnd) };
    }
    pg.form.id_map.clear();
    pg.form.next_id = ID_CONTROL_BASE;
    pg.form.y_cursor = TOOLBAR_HEIGHT + SPACING;
    pg.form.scroll_y = 0;

    build_controls_recursive(&mut pg.form, &pg.runtime, pg.runtime.root(), 0);
    pg.form.content_height = pg.form.y_cursor + SPACING;
    update_scrollbar(&pg.form);

    unsafe {
        SendMessageW(pg.form.hwnd, WM_SETREDRAW, 1, 0);
        RedrawWindow(pg.form.hwnd, ptr::null(), 0 as _, RDW_ERASE | RDW_FRAME | RDW_INVALIDATE | RDW_ALLCHILDREN | RDW_UPDATENOW);
    };
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
