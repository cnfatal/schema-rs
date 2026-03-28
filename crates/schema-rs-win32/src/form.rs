//! Public API: create and run a native Win32 form from a `SchemaRuntime`.

use std::collections::HashMap;
use std::ptr;
use std::sync::atomic::{AtomicPtr, Ordering};

use schema_rs_core::{DefaultValidator, JsonValue, SchemaRuntime};
use windows_sys::Win32::Foundation::{BOOL, HWND};
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::Console::SetConsoleCtrlHandler;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Controls::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use crate::controls::build_controls_recursive;
use crate::state::*;
use crate::util::*;
use crate::wndproc::{update_scrollbar, wnd_proc};

static FORM_HWND: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(ptr::null_mut());

unsafe extern "system" fn console_ctrl_handler(ctrl_type: u32) -> BOOL {
    let _ = ctrl_type;
    let hwnd = FORM_HWND.load(Ordering::SeqCst);
    if !hwnd.is_null() {
        unsafe { PostMessageW(hwnd, WM_CLOSE, 0, 0) };
    }
    1 // TRUE — handled
}

// ── Helpers ──

fn create_form_window(title: &str, runtime: &mut SchemaRuntime) -> (HWND, SchemaFormWindow) {
    let class_name = to_wide("SchemaRsFormClass");
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

    let title_wide = to_wide(title);
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_COMPOSITED,
            class_name.as_ptr(),
            title_wide.as_ptr(),
            WS_OVERLAPPEDWINDOW | WS_VSCROLL | WS_CLIPCHILDREN,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            640,
            600,
            0 as _,
            0 as _,
            h_instance,
            ptr::null(),
        )
    };

    let hfont = create_default_font();

    let mut form = SchemaFormWindow {
        hwnd,
        controls: Vec::new(),
        id_map: HashMap::new(),
        next_id: ID_CONTROL_BASE,
        y_cursor: SPACING,
        client_width: 600,
        content_height: 0,
        scroll_y: 0,
        hfont,
        tab_selection: HashMap::new(),
        confirmed: false,
    };

    let _ = runtime.drain_events();
    build_controls_recursive(&mut form, runtime, runtime.root(), 0);
    form.content_height = form.y_cursor + SPACING;
    update_scrollbar(&form);

    (hwnd, form)
}

fn run_message_loop(hwnd: HWND) {
    FORM_HWND.store(hwnd, Ordering::SeqCst);
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

    FORM_HWND.store(ptr::null_mut(), Ordering::SeqCst);
    unsafe { SetConsoleCtrlHandler(Some(console_ctrl_handler), 0) };
}

// ── Public API ──

/// Run a form window. Calls `on_change` on every value modification.
/// Returns the final value when the window is closed.
pub fn run_form(
    schema: JsonValue,
    value: JsonValue,
    on_change: Option<Box<dyn FnMut(&JsonValue)>>,
) -> JsonValue {
    let (hwnd, form, runtime) = init_form(schema, value);

    FORM_STATE.with(|cell| {
        *cell.borrow_mut() = Some(FormState { form, runtime, on_change });
    });
    run_message_loop(hwnd);
    take_result(true).unwrap_or(JsonValue::Null)
}

/// Run a form window with OK/Cancel buttons.
/// Returns `Some(value)` on confirm, `None` on cancel.
pub fn run_form_confirm(
    schema: JsonValue,
    value: JsonValue,
    on_change: Option<Box<dyn FnMut(&JsonValue)>>,
) -> Option<JsonValue> {
    let (hwnd, mut form, runtime) = init_form(schema, value);
    append_confirm_buttons(hwnd, &mut form);

    FORM_STATE.with(|cell| {
        *cell.borrow_mut() = Some(FormState { form, runtime, on_change });
    });
    run_message_loop(hwnd);
    take_result(false)
}

// ── Internals ──

fn init_form(schema: JsonValue, value: JsonValue) -> (HWND, SchemaFormWindow, SchemaRuntime) {
    let title = schema
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("Form")
        .to_string();
    let mut runtime = SchemaRuntime::new(Box::new(DefaultValidator::new()), schema, value);
    let (hwnd, form) = create_form_window(&title, &mut runtime);
    (hwnd, form, runtime)
}

fn append_confirm_buttons(hwnd: HWND, form: &mut SchemaFormWindow) {
    let h_instance = unsafe { GetModuleHandleW(ptr::null()) };
    let btn_w = 80;
    let btn_h = FIELD_HEIGHT + 4;
    let btn_y = form.y_cursor;
    let cancel_x = form.client_width - RIGHT_MARGIN - btn_w;
    let ok_x = cancel_x - SPACING - btn_w;

    let btn_class = to_wide("BUTTON");
    let ok_text = to_wide("确定");
    let cancel_text = to_wide("取消");

    unsafe {
        let ok_hwnd = CreateWindowExW(
            0,
            btn_class.as_ptr(),
            ok_text.as_ptr(),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_DEFPUSHBUTTON as u32,
            ok_x,
            btn_y - form.scroll_y,
            btn_w,
            btn_h,
            hwnd,
            ID_OK_BUTTON as _,
            h_instance,
            ptr::null(),
        );
        SendMessageW(ok_hwnd, WM_SETFONT, form.hfont as usize, 1);

        let cancel_hwnd = CreateWindowExW(
            0,
            btn_class.as_ptr(),
            cancel_text.as_ptr(),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_PUSHBUTTON as u32,
            cancel_x,
            btn_y - form.scroll_y,
            btn_w,
            btn_h,
            hwnd,
            ID_CANCEL_BUTTON as _,
            h_instance,
            ptr::null(),
        );
        SendMessageW(cancel_hwnd, WM_SETFONT, form.hfont as usize, 1);
    }

    form.y_cursor = btn_y + btn_h;
    form.content_height = form.y_cursor + SPACING;
    update_scrollbar(form);
}

/// Extract the final value. If `always` is true, ignores the confirmed flag.
fn take_result(always: bool) -> Option<JsonValue> {
    FORM_STATE.with(|cell| {
        let state = cell.borrow_mut().take();
        match state {
            Some(s) => {
                unsafe { DeleteObject(s.form.hfont as _) };
                if always || s.form.confirmed {
                    Some(s.runtime.get_root_value().clone())
                } else {
                    None
                }
            }
            None => None,
        }
    })
}
