//! Number / integer field controls.

use schema_rs_core::{FieldNode, SchemaRuntime};

use crate::state::*;
use crate::util::*;

use super::primitives::*;

pub fn build_number_control(
    form: &mut SchemaFormWindow,
    runtime: &SchemaRuntime,
    node: &FieldNode,
    x: i32,
    w: i32,
) {
    let label = label_text(node);
    create_static(form, &label, x, w);

    if let Some(desc) = &node.schema.description {
        create_static(form, desc, x, w);
    }

    let current = runtime
        .get_value(&node.instance_location)
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let text = if node.type_ == schema_rs_core::SchemaType::Integer {
        format!("{}", current as i64)
    } else {
        format!("{current}")
    };
    let read_only = node.schema.read_only.unwrap_or(false) || node.schema.const_.is_some();
    create_edit(form, &node.instance_location, x, w, &text, read_only, ControlKind::NumberEdit);
}
