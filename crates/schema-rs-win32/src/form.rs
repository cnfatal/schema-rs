//! Public API: create and run a native Win32 form from a `SchemaRuntime`.

use std::collections::HashMap;
use std::ptr;
use std::sync::atomic::{AtomicPtr, Ordering};

use schema_rs_core::{JsonValue, SchemaRuntime};
use windows_sys::Win32::Foundation::BOOL;
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

// ── Public API ──

/// Create a native Win32 form window for the given runtime and run the message
/// loop.  Returns the final JSON value when the window is closed.
pub fn run_schema_form(title: &str, runtime: SchemaRuntime) -> JsonValue {
    run_form_impl(title, runtime)
}

fn run_form_impl(title: &str, mut runtime: SchemaRuntime) -> JsonValue {
    let class_name = to_wide("SchemaRsFormClass");
    let h_instance = unsafe { GetModuleHandleW(ptr::null()) };

    // Init common controls.
    let icc = INITCOMMONCONTROLSEX {
        dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
        dwICC: ICC_STANDARD_CLASSES | ICC_WIN95_CLASSES,
    };
    unsafe { InitCommonControlsEx(&icc) };

    // Register window class.
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

    let mut form = crate::state::SchemaFormWindow {
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
    };

    let _ = runtime.drain_events();
    build_controls_recursive(&mut form, &runtime, runtime.root(), 0);
    form.content_height = form.y_cursor + SPACING;
    update_scrollbar(&form);

    FORM_STATE.with(|cell| {
        *cell.borrow_mut() = Some((form, runtime));
    });

    // Register console Ctrl+C handler.
    FORM_HWND.store(hwnd, Ordering::SeqCst);
    unsafe { SetConsoleCtrlHandler(Some(console_ctrl_handler), 1) };

    unsafe {
        ShowWindow(hwnd, SW_SHOW);
        UpdateWindow(hwnd);
    }

    // Message loop.
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

    // Extract final value.
    FORM_HWND.store(ptr::null_mut(), Ordering::SeqCst);
    unsafe { SetConsoleCtrlHandler(Some(console_ctrl_handler), 0) };
    FORM_STATE.with(|cell| {
        let state = cell.borrow_mut().take();
        match state {
            Some((form, rt)) => {
                unsafe { DeleteObject(form.hfont as _) };
                rt.get_root_value().clone()
            }
            None => JsonValue::Null,
        }
    })
}
