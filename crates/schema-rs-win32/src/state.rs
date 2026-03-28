//! Shared types, layout constants, and thread-local form state.

use std::cell::RefCell;
use std::collections::HashMap;

use schema_rs_core::SchemaRuntime;
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::Graphics::Gdi::HFONT;
use windows_sys::Win32::UI::WindowsAndMessaging::GetDlgCtrlID;

// ── Layout constants ──

pub const FIELD_HEIGHT: i32 = 24;
pub const LABEL_HEIGHT: i32 = 18;
pub const SPACING: i32 = 6;
pub const INDENT: i32 = 20;
pub const LEFT_MARGIN: i32 = 10;
pub const RIGHT_MARGIN: i32 = 10;
pub const ID_CONTROL_BASE: u16 = 1000;

// ── Control tracking ──

pub struct ControlEntry {
    pub hwnd: HWND,
    pub path: String,
    pub kind: ControlKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlKind {
    Edit,
    EditMultiline,
    EditPassword,
    ComboBox,
    CheckBox,
    NumberEdit,
    AddButton,
    RemoveButton,
    BrowseButton,
    TabControl,
    #[allow(dead_code)]
    Label,
}

// ── SchemaFormWindow ──

pub struct SchemaFormWindow {
    pub hwnd: HWND,
    pub controls: Vec<ControlEntry>,
    pub id_map: HashMap<u16, usize>,
    pub next_id: u16,
    pub y_cursor: i32,
    pub client_width: i32,
    pub content_height: i32,
    pub scroll_y: i32,
    pub hfont: HFONT,
    /// Tracks which tab is selected per node path.
    pub tab_selection: HashMap<String, usize>,
}

// ── Thread-local for WndProc ──

thread_local! {
    pub static FORM_STATE: RefCell<Option<(SchemaFormWindow, SchemaRuntime)>> = const { RefCell::new(None) };
}

// ── ID management ──

pub fn alloc_id(form: &mut SchemaFormWindow) -> u16 {
    let id = form.next_id;
    form.next_id = form.next_id.wrapping_add(1);
    id
}

pub fn register_control(form: &mut SchemaFormWindow, hwnd: HWND, path: &str, kind: ControlKind) {
    let id = unsafe { GetDlgCtrlID(hwnd) } as u16;
    let idx = form.controls.len();
    form.controls.push(ControlEntry {
        hwnd,
        path: path.to_string(),
        kind,
    });
    form.id_map.insert(id, idx);
}
