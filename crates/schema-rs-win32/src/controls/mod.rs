//! Win32 control creation and recursive form builder.
//!
//! Each field type is implemented in its own submodule for maintainability:
//! - [`primitives`] — Low-level Win32 control helpers (edit, combo, checkbox, button, static)
//! - [`string`] — String fields (text, password, textarea, file path, combo)
//! - [`number`] — Number and integer fields
//! - [`boolean`] — Boolean / checkbox fields
//! - [`object`] — Object containers and tabs layout
//! - [`array`] — Array containers and table layout

pub mod array;
pub mod boolean;
pub mod number;
pub mod object;
pub mod primitives;
pub mod string;

use schema_rs_core::{FieldNode, SchemaRuntime, SchemaType};

use crate::state::*;
use crate::util::*;

use self::array::build_array_control;
use self::boolean::build_boolean_control;
use self::number::build_number_control;
use self::object::build_object_control;
use self::primitives::create_static;
use self::string::build_string_control;

// ── Recursive control builder ──

pub fn build_controls_recursive(
    form: &mut SchemaFormWindow,
    runtime: &SchemaRuntime,
    node: &FieldNode,
    depth: usize,
) {
    if !node.activated {
        return;
    }

    let x = LEFT_MARGIN + depth as i32 * INDENT;
    let w = (form.client_width - x - RIGHT_MARGIN).max(60);

    let layout = node
        .schema
        .extensions
        .get("x-layout")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    match node.type_ {
        SchemaType::String => build_string_control(form, runtime, node, x, w),
        SchemaType::Number | SchemaType::Integer => build_number_control(form, runtime, node, x, w),
        SchemaType::Boolean => build_boolean_control(form, runtime, node, x, w),
        SchemaType::Object => build_object_control(form, runtime, node, x, w, depth, layout),
        SchemaType::Array => build_array_control(form, runtime, node, x, w, depth, layout),
        SchemaType::Null => {
            let label = field_label(node);
            create_static(form, &format!("{label}: (null)"), x, w);
        }
        SchemaType::Unknown => {
            let label = field_label(node);
            create_static(form, &format!("{label}: (unknown)"), x, w);
        }
    }

    // Validation error.
    if let Some(vo) = &node.error {
        if let Some(msg) = format_first_error(vo) {
            create_static(form, &format!("⚠ {msg}"), x, w);
        }
    }
}
