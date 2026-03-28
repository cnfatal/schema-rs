//! Object field controls: default layout and tabs layout.

use std::ptr;

use schema_rs_core::{FieldNode, SchemaRuntime};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Controls::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use crate::state::*;
use crate::util::*;

use super::build_controls_recursive;
use super::primitives::*;

pub fn build_object_control(
    form: &mut SchemaFormWindow,
    runtime: &SchemaRuntime,
    node: &FieldNode,
    x: i32,
    w: i32,
    depth: usize,
    layout: &str,
) {
    if layout == "tabs" {
        build_tabs_control(form, runtime, node, x, w, depth);
    } else {
        build_header_control(form, node, x, w, "Object");
        for &child_idx in &node.children {
            if let Some(child) = runtime.get_node_by_index(child_idx) {
                build_controls_recursive(form, runtime, child, depth + 1);
            }
        }
        if node.can_add {
            create_button(form, &node.instance_location, x, w, "Add Property", ControlKind::AddButton);
        }
    }
}

pub fn build_header_control(
    form: &mut SchemaFormWindow,
    node: &FieldNode,
    x: i32,
    w: i32,
    type_hint: &str,
) {
    let label = field_label(node);
    let text = if label.is_empty() || label == "Root" {
        type_hint.to_string()
    } else {
        label
    };
    create_static(form, &format!("── {text} ──"), x, w);

    if let Some(desc) = &node.schema.description {
        create_static(form, desc, x, w);
    }

    if node.can_remove {
        create_button(form, &node.instance_location, x, w, "Remove", ControlKind::RemoveButton);
    }
}

fn build_tabs_control(
    form: &mut SchemaFormWindow,
    runtime: &SchemaRuntime,
    node: &FieldNode,
    x: i32,
    w: i32,
    depth: usize,
) {
    let label = field_label(node);
    if !label.is_empty() && label != "Root" {
        create_static(form, &format!("── {label} ──"), x, w);
    }

    let active_children: Vec<(usize, &FieldNode)> = node
        .children
        .iter()
        .filter_map(|&idx| {
            runtime
                .get_node_by_index(idx)
                .filter(|n| n.activated)
                .map(|n| (idx, n))
        })
        .collect();

    if active_children.is_empty() {
        return;
    }

    // Resolve selected tab index.
    let selected = *form
        .tab_selection
        .get(&node.instance_location)
        .unwrap_or(&0);
    let selected = selected.min(active_children.len().saturating_sub(1));

    // Create Win32 Tab Control.
    let tab_id = alloc_id(form);
    let cls = to_wide("SysTabControl32");
    let empty = to_wide("");
    let tab_h = 28;
    let hwnd_tab = unsafe {
        CreateWindowExW(
            0,
            cls.as_ptr(),
            empty.as_ptr(),
            WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS,
            x,
            form.y_cursor,
            w,
            tab_h,
            form.hwnd,
            tab_id as HMENU,
            GetModuleHandleW(ptr::null()),
            ptr::null(),
        )
    };
    if !hwnd_tab.is_null() {
        unsafe { SendMessageW(hwnd_tab, WM_SETFONT, form.hfont as usize, 1) };

        // Add tab items.
        for (i, (_, child)) in active_children.iter().enumerate() {
            let tab_label = child
                .schema
                .title
                .clone()
                .unwrap_or_else(|| path_last_segment(&child.instance_location));
            let wide_label = to_wide(&tab_label);
            let item = TCITEMW {
                mask: TCIF_TEXT,
                dwState: 0,
                dwStateMask: 0,
                pszText: wide_label.as_ptr() as *mut _,
                cchTextMax: 0,
                iImage: -1,
                lParam: 0,
            };
            unsafe { SendMessageW(hwnd_tab, TCM_INSERTITEMW, i, &item as *const _ as _) };
        }

        // Select current tab.
        unsafe { SendMessageW(hwnd_tab, TCM_SETCURSEL, selected, 0) };

        register_control(form, hwnd_tab, &node.instance_location, ControlKind::TabControl);
    }
    form.y_cursor += tab_h + SPACING;

    // Only render the selected tab's child.
    if let Some((_, child_node)) = active_children.get(selected) {
        build_controls_recursive(form, runtime, child_node, depth + 1);
    }
}
