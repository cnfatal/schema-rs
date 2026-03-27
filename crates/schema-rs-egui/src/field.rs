use egui::Ui;
use schema_rs_core::{FieldNode, JsonValue, SchemaRuntime, SchemaType};

use crate::renderer::{FieldAction, FieldContext, FieldRenderer};

/// A pending mutation collected during the render phase.
pub struct PendingAction {
    pub path: String,
    pub action: FieldAction,
}

/// Render a field and its children recursively, returning pending mutations.
pub fn render_field(
    ui: &mut Ui,
    runtime: &SchemaRuntime,
    node: &FieldNode,
    renderer: &dyn FieldRenderer,
) -> Vec<PendingAction> {
    render_field_inner(ui, runtime, node, renderer, 0)
}

fn render_field_inner(
    ui: &mut Ui,
    runtime: &SchemaRuntime,
    node: &FieldNode,
    renderer: &dyn FieldRenderer,
    depth: usize,
) -> Vec<PendingAction> {
    let mut actions = Vec::new();
    let ctx = build_context(runtime, node);

    let collapsible = node
        .schema
        .extensions
        .get("x-collapsible")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let layout = node
        .schema
        .extensions
        .get("x-layout")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    match node.type_ {
        SchemaType::Object | SchemaType::Array => {
            if collapsible {
                let id = ui.make_persistent_id(format!("{}_collapse", node.instance_location));
                egui::collapsing_header::CollapsingState::load_with_default_open(
                    ui.ctx(),
                    id,
                    true,
                )
                .show_header(ui, |ui| {
                    let action = renderer.render(ui, &ctx);
                    if !matches!(action, FieldAction::None) {
                        actions.push(PendingAction {
                            path: node.instance_location.clone(),
                            action,
                        });
                    }
                })
                .body(|ui| {
                    render_children(ui, runtime, node, renderer, layout, depth, &mut actions);
                });
            } else {
                let action = renderer.render(ui, &ctx);
                if !matches!(action, FieldAction::None) {
                    actions.push(PendingAction {
                        path: node.instance_location.clone(),
                        action,
                    });
                }
                // depth 1 = root's direct children: wrap in a card frame
                if depth == 1 {
                    egui::Frame::group(ui.style())
                        .inner_margin(egui::Margin::same(6))
                        .show(ui, |ui| {
                            render_children(
                                ui,
                                runtime,
                                node,
                                renderer,
                                layout,
                                depth,
                                &mut actions,
                            );
                        });
                } else if depth > 1 {
                    ui.indent(node.instance_location.as_str(), |ui| {
                        render_children(ui, runtime, node, renderer, layout, depth, &mut actions);
                    });
                } else {
                    // depth 0: root node, render children directly
                    render_children(ui, runtime, node, renderer, layout, depth, &mut actions);
                }
            }
        }
        _ => {
            if collapsible {
                let id = ui.make_persistent_id(format!("{}_collapse", node.instance_location));
                egui::collapsing_header::CollapsingState::load_with_default_open(
                    ui.ctx(),
                    id,
                    true,
                )
                .show_header(ui, |ui| {
                    let action = renderer.render(ui, &ctx);
                    if !matches!(action, FieldAction::None) {
                        actions.push(PendingAction {
                            path: node.instance_location.clone(),
                            action,
                        });
                    }
                })
                .body(|_ui| {});
            } else {
                let action = renderer.render(ui, &ctx);
                if !matches!(action, FieldAction::None) {
                    actions.push(PendingAction {
                        path: node.instance_location.clone(),
                        action,
                    });
                }
            }
        }
    }

    actions
}

/// Render children of a container node using the specified layout.
fn render_children(
    ui: &mut Ui,
    runtime: &SchemaRuntime,
    node: &FieldNode,
    renderer: &dyn FieldRenderer,
    layout: &str,
    depth: usize,
    actions: &mut Vec<PendingAction>,
) {
    match layout {
        "tabs" if node.type_ == SchemaType::Object => {
            render_tabs(ui, runtime, node, renderer, actions);
        }
        "table" if node.type_ == SchemaType::Array => {
            render_table(ui, runtime, node, actions);
        }
        _ => {
            // Default: render children sequentially
            for (i, &child_idx) in node.children.iter().enumerate() {
                if let Some(child_node) = runtime.get_node_by_index(child_idx) {
                    if child_node.activated {
                        if i > 0 {
                            ui.add_space(2.0);
                        }
                        actions.extend(render_field_inner(
                            ui,
                            runtime,
                            child_node,
                            renderer,
                            depth + 1,
                        ));
                    }
                }
            }
        }
    }
}

/// Render object children as tabs.
fn render_tabs(
    ui: &mut Ui,
    runtime: &SchemaRuntime,
    node: &FieldNode,
    renderer: &dyn FieldRenderer,
    actions: &mut Vec<PendingAction>,
) {
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

    let tab_id = ui.make_persistent_id(format!("{}_tab", node.instance_location));
    let mut selected: usize = ui.data_mut(|d| d.get_temp(tab_id)).unwrap_or(0);
    if selected >= active_children.len() {
        selected = 0;
    }

    // Tab bar
    ui.horizontal(|ui| {
        for (i, (_, child)) in active_children.iter().enumerate() {
            let label = child
                .schema
                .title
                .clone()
                .unwrap_or_else(|| path_last_segment(&child.instance_location));
            if ui.selectable_label(i == selected, &label).clicked() {
                selected = i;
            }
        }
    });
    ui.data_mut(|d| d.insert_temp(tab_id, selected));

    ui.separator();

    // Render selected tab content
    if let Some((_, child_node)) = active_children.get(selected) {
        actions.extend(render_field_inner(ui, runtime, child_node, renderer, 2));
    }
}

/// Render array of objects as a table.
fn render_table(
    ui: &mut Ui,
    runtime: &SchemaRuntime,
    node: &FieldNode,
    actions: &mut Vec<PendingAction>,
) {
    // Collect active row nodes
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
        return;
    }

    // Gather column names from the first row's children
    let columns: Vec<(usize, String)> = rows[0]
        .children
        .iter()
        .filter_map(|&idx| {
            runtime
                .get_node_by_index(idx)
                .filter(|n| n.activated)
                .map(|n| {
                    let label = n
                        .schema
                        .title
                        .clone()
                        .unwrap_or_else(|| path_last_segment(&n.instance_location));
                    (idx, label)
                })
        })
        .collect();

    let column_count = columns.len() + 1; // +1 for actions column

    let table_id = format!("{}_table", node.instance_location);
    egui_extras::TableBuilder::new(ui)
        .id_salt(&table_id)
        .striped(true)
        .resizable(true)
        .columns(
            egui_extras::Column::auto().at_least(60.0).clip(true),
            column_count,
        )
        .header(20.0, |mut header| {
            for (_, col_label) in &columns {
                header.col(|ui| {
                    ui.strong(col_label);
                });
            }
            header.col(|ui| {
                ui.strong("");
            });
        })
        .body(|body| {
            body.rows(24.0, rows.len(), |mut row| {
                let row_idx = row.index();
                let row_node = rows[row_idx];

                for (col_idx, (_, _)) in columns.iter().enumerate() {
                    row.col(|ui| {
                        // Find the matching cell node in this row
                        if let Some(&cell_idx) = row_node.children.get(col_idx) {
                            if let Some(cell_node) = runtime.get_node_by_index(cell_idx) {
                                if cell_node.activated {
                                    let cell_path = &cell_node.instance_location;
                                    let cell_value = runtime.get_value(cell_path);
                                    let action =
                                        render_table_cell(ui, cell_node, cell_value, cell_path);
                                    if !matches!(action, FieldAction::None) {
                                        actions.push(PendingAction {
                                            path: cell_path.clone(),
                                            action,
                                        });
                                    }
                                }
                            }
                        }
                    });
                }

                // Actions column
                row.col(|ui| {
                    if row_node.can_remove && ui.small_button("✕").clicked() {
                        actions.push(PendingAction {
                            path: row_node.instance_location.clone(),
                            action: FieldAction::Remove,
                        });
                    }
                });
            });
        });
}

/// Render a single table cell inline (compact, no label).
fn render_table_cell(
    ui: &mut Ui,
    node: &FieldNode,
    value: Option<&JsonValue>,
    _path: &str,
) -> FieldAction {
    let read_only = node.schema.read_only.unwrap_or(false) || node.schema.const_.is_some();

    match node.type_ {
        SchemaType::String => {
            let mut text = value.and_then(|v| v.as_str()).unwrap_or("").to_owned();
            let response = ui.add_enabled(
                !read_only,
                egui::TextEdit::singleline(&mut text).desired_width(ui.available_width()),
            );
            if response.changed() {
                FieldAction::SetValue(JsonValue::String(text))
            } else {
                FieldAction::None
            }
        }
        SchemaType::Number | SchemaType::Integer => {
            let mut val = value.and_then(|v| v.as_f64()).unwrap_or(0.0);
            let response = ui.add_enabled(!read_only, egui::DragValue::new(&mut val));
            if response.changed() {
                if node.type_ == SchemaType::Integer {
                    FieldAction::SetValue(JsonValue::from(val as i64))
                } else {
                    FieldAction::SetValue(
                        serde_json::Number::from_f64(val)
                            .map(JsonValue::Number)
                            .unwrap_or(JsonValue::Null),
                    )
                }
            } else {
                FieldAction::None
            }
        }
        SchemaType::Boolean => {
            let mut checked = value.and_then(|v| v.as_bool()).unwrap_or(false);
            if ui
                .add_enabled(!read_only, egui::Checkbox::without_text(&mut checked))
                .changed()
            {
                FieldAction::SetValue(JsonValue::Bool(checked))
            } else {
                FieldAction::None
            }
        }
        _ => {
            let text = value
                .map(|v| v.to_string())
                .unwrap_or_else(|| "-".to_string());
            ui.label(text);
            FieldAction::None
        }
    }
}

fn build_context<'a>(runtime: &'a SchemaRuntime, node: &'a FieldNode) -> FieldContext<'a> {
    let path = &node.instance_location;
    let value = runtime.get_value(path);

    let label = node
        .schema
        .title
        .clone()
        .unwrap_or_else(|| path_last_segment(path));

    let description = node.schema.description.as_deref();

    let error_message = node.error.as_ref().and_then(|vo| format_first_error(vo));

    let read_only = node.schema.read_only.unwrap_or(false) || node.schema.const_.is_some();

    FieldContext {
        node,
        path,
        value,
        label,
        description,
        error_message,
        read_only,
        required: node.required,
        can_add: node.can_add,
        can_remove: node.can_remove,
    }
}

fn path_last_segment(path: &str) -> String {
    if path.is_empty() {
        return "Root".to_string();
    }
    path.rsplit('/').next().unwrap_or(path).to_string()
}

fn format_first_error(vo: &schema_rs_core::ValidationOutput) -> Option<String> {
    if let Some(err) = &vo.error {
        return Some(err.key.clone());
    }
    for child in &vo.errors {
        if let Some(msg) = format_first_error(child) {
            return Some(msg);
        }
    }
    None
}
