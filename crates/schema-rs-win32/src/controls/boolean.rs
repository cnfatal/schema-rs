//! Boolean field controls.

use schema_rs_core::{FieldNode, SchemaRuntime};

use crate::state::*;
use crate::util::*;

use super::primitives::*;

pub fn build_boolean_control(
    form: &mut SchemaFormWindow,
    runtime: &SchemaRuntime,
    node: &FieldNode,
    x: i32,
    w: i32,
) {
    let label = field_label(node);
    let checked = runtime
        .get_value(&node.instance_location)
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let read_only = node.schema.read_only.unwrap_or(false) || node.schema.const_.is_some();
    create_checkbox(form, &node.instance_location, x, w, &label, checked, read_only);
}
