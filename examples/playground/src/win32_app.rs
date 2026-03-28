//! Native Win32 playground backend with schema selector.
//!
//! Uses `run_form` from `schema-rs-win32` for form rendering.

use std::ptr;
use std::sync::atomic::{AtomicPtr, Ordering};

use schema_rs::win32::run_form;
use schema_rs::win32::state::{FIELD_HEIGHT, LEFT_MARGIN};
use schema_rs::win32::util::*;
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
const ID_OPEN_BUTTON: u16 = 2;
const TOOLBAR_HEIGHT: i32 = 36;

// ── Public entry ──

pub fn run() {
    let examples = example_schemas();

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

    let title_wide = to_wide("schema-rs playground");
    let hwnd = unsafe {
        CreateWindowExW(
            0,
            class_name.as_ptr(),
            title_wide.as_ptr(),
            WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            520,
            120,
            0 as _,
            0 as _,
            h_instance,
            ptr::null(),
        )
    };

    let hfont = create_default_font();

    // ── Label ──
    let label_cls = to_wide("STATIC");
    let label_text = to_wide("Example:");
    let hwnd_label = unsafe {
        CreateWindowExW(
            0,
            label_cls.as_ptr(),
            label_text.as_ptr(),
            WS_CHILD | WS_VISIBLE,
            LEFT_MARGIN,
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

    // ── Schema selector ComboBox ──
    let cls_combo = to_wide("COMBOBOX");
    let empty = to_wide("");
    let hwnd_combo = unsafe {
        CreateWindowExW(
            0,
            cls_combo.as_ptr(),
            empty.as_ptr(),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | CBS_DROPDOWNLIST as u32 | CBS_HASSTRINGS as u32,
            LEFT_MARGIN + 65,
            6,
            320,
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

    // ── Open button ──
    let btn_cls = to_wide("BUTTON");
    let btn_text = to_wide("打开");
    let hwnd_btn = unsafe {
        CreateWindowExW(
            0,
            btn_cls.as_ptr(),
            btn_text.as_ptr(),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_DEFPUSHBUTTON as u32,
            LEFT_MARGIN + 395,
            5,
            80,
            FIELD_HEIGHT + 4,
            hwnd,
            ID_OPEN_BUTTON as HMENU,
            h_instance,
            ptr::null(),
        )
    };
    if !hwnd_btn.is_null() {
        unsafe { SendMessageW(hwnd_btn, WM_SETFONT, hfont as usize, 1) };
    }

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
    unsafe { DeleteObject(hfont as _) };
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

            if ctrl_id == ID_OPEN_BUTTON && notify_code == BN_CLICKED as u16 {
                open_selected_schema(hwnd);
            }
            0
        }
        WM_DESTROY => {
            unsafe { PostQuitMessage(0) };
            0
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

// ── Open schema form ──

fn open_selected_schema(hwnd: HWND) {
    let examples = example_schemas();

    let hwnd_combo = unsafe { GetDlgItem(hwnd, ID_SCHEMA_COMBO as i32) };
    let idx = unsafe { SendMessageW(hwnd_combo, CB_GETCURSEL, 0, 0) };
    if idx < 0 || idx as usize >= examples.len() {
        return;
    }

    let (_name, schema, value) = &examples[idx as usize];

    let result = run_form(
        schema.clone(),
        value.clone(),
        Some(Box::new(|v: &Value| {
            println!("Changed: {}", serde_json::to_string(v).unwrap_or_default());
        })),
    );

    println!(
        "Result:\n{}",
        serde_json::to_string_pretty(&result).unwrap()
    );
}
