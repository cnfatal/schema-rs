//! Array field controls: default layout and table layout.

use schema_rs_core::{FieldNode, JsonValue, SchemaRuntime, SchemaType};

use crate::state::*;
#[allow(unused_imports)]
use crate::util::*;

use super::build_controls_recursive;
use super::object::build_header_control;
use super::primitives::*;

pub fn build_array_control(
    form: &mut SchemaFormWindow,
    runtime: &SchemaRuntime,
    node: &FieldNode,
    x: i32,
    w: i32,
    depth: usize,
    layout: &str,
) {
    if layout == "table" {
        build_table_control(form, runtime, node, x, w, depth);
    } else {
        build_header_control(form, node, x, w, "Array");
        for &child_idx in &node.children {
            if let Some(child) = runtime.get_node_by_index(child_idx) {
                build_controls_recursive(form, runtime, child, depth + 1);
            }
        }
        if node.can_add {
            create_button(form, &node.instance_location, x, w, "Add Item", ControlKind::AddButton);
        }
    }
}

fn build_table_control(
    form: &mut SchemaFormWindow,
    runtime: &SchemaRuntime,
    node: &FieldNode,
    x: i32,
    w: i32,
    _depth: usize,
) {
    build_header_control(form, node, x, w, "Array");

    // Collect active object rows.
    let rows: Vec<&FieldNode> = node
        .children
        .iter()
        .filter_map(|&idx| {
            runtime
                .get_node_by_index(idx)
                .filter(|n| n.activated && n.type_ == SchemaType::Object)
        })
        .collect();

    if rows.is_empty() {
        if node.can_add {
            create_button(form, &node.instance_location, x, w, "Add Item", ControlKind::AddButton);
        }
        return;
    }

    // Gather column headers from first row.
    let columns: Vec<(usize, String)> = rows[0]
        .children
        .iter()
        .filter_map(|&idx| {
            runtime.get_node_by_index(idx).filter(|n| n.activated).map(|n| {
                let label = n
                    .schema
                    .title
                    .clone()
                    .unwrap_or_else(|| path_last_segment(&n.instance_location));
                (idx, label)
            })
        })
        .collect();

    let col_count = columns.len().max(1) as i32;
    let action_col_w = 50;
    let col_w = (w - action_col_w) / col_count;

    // Column headers (all on the same row).
    let inner_x = x;
    let header_y = form.y_cursor;
    for (i, (_, col_label)) in columns.iter().enumerate() {
        form.y_cursor = header_y;
        let cx = inner_x + i as i32 * col_w;
        create_static(form, col_label, cx, col_w - 4);
    }
    // Advance past the header row.
    form.y_cursor = header_y + LABEL_HEIGHT + SPACING;

    // Rows.
    for row_node in &rows {
        let row_y_start = form.y_cursor;

        for (col_idx, (_, _)) in columns.iter().enumerate() {
            let cx = inner_x + col_idx as i32 * col_w;
            let cw = col_w - 4;

            if let Some(&cell_idx) = row_node.children.get(col_idx) {
                if let Some(cell_node) = runtime.get_node_by_index(cell_idx) {
                    if cell_node.activated {
                        // Render inline cell (value only, no label).
                        build_table_cell(form, runtime, cell_node, cx, cw);
                    }
                }
            }

            // Reset y for next column on same row.
            form.y_cursor = row_y_start;
        }

        // Action column: Remove button.
        if row_node.can_remove {
            form.y_cursor = row_y_start;
            let btn_x = inner_x + col_count * col_w;
            create_button(form, &row_node.instance_location, btn_x, action_col_w, "✕", ControlKind::RemoveButton);
        } else {
            form.y_cursor = row_y_start + FIELD_HEIGHT + SPACING;
        }
    }

    if node.can_add {
        create_button(form, &node.instance_location, x, w, "Add Item", ControlKind::AddButton);
    }
}

/// Render a single table cell (value only, no label).
fn build_table_cell(
    form: &mut SchemaFormWindow,
    runtime: &SchemaRuntime,
    node: &FieldNode,
    x: i32,
    w: i32,
) {
    let value_str;
    let read_only = node.schema.read_only.unwrap_or(false) || node.schema.const_.is_some();

    match node.type_ {
        SchemaType::String => {
            let value = runtime
                .get_value(&node.instance_location)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_owned();

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
                create_edit(form, &node.instance_location, x, w, &value, read_only, ControlKind::Edit);
            }
        }
        SchemaType::Number | SchemaType::Integer => {
            let current = runtime
                .get_value(&node.instance_location)
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            value_str = if node.type_ == SchemaType::Integer {
                format!("{}", current as i64)
            } else {
                format!("{current}")
            };
            create_edit(form, &node.instance_location, x, w, &value_str, read_only, ControlKind::NumberEdit);
        }
        SchemaType::Boolean => {
            let checked = runtime
                .get_value(&node.instance_location)
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            create_checkbox(form, &node.instance_location, x, w, "", checked, read_only);
        }
        _ => {
            let text = runtime
                .get_value(&node.instance_location)
                .map(|v| v.to_string())
                .unwrap_or_else(|| "-".to_string());
            create_static(form, &text, x, w);
        }
    }
}
