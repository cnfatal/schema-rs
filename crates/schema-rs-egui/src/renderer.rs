use egui::Ui;
use schema_rs_core::{FieldNode, JsonValue, Schema, SchemaType};

/// Context passed to each field renderer.
pub struct FieldContext<'a> {
    pub node: &'a FieldNode,
    pub path: &'a str,
    pub value: Option<&'a JsonValue>,
    pub label: String,
    pub description: Option<&'a str>,
    pub error_message: Option<String>,
    pub read_only: bool,
    pub required: bool,
    pub can_add: bool,
    pub can_remove: bool,
}

/// Actions that a renderer can request.
pub enum FieldAction {
    None,
    SetValue(JsonValue),
    Remove,
    AddChild {
        key: Option<String>,
        value: Option<JsonValue>,
    },
}

/// Trait for customizable field rendering.
pub trait FieldRenderer {
    fn render(&self, ui: &mut Ui, ctx: &FieldContext) -> FieldAction;
}

/// Default egui renderer.
pub struct DefaultRenderer;

/// Renders a file/directory path picker: text input + Browse button.
fn render_file_path(
    ui: &mut Ui,
    text: &mut String,
    read_only: bool,
    is_directory: bool,
    schema: &Schema,
) -> egui::Response {
    let r = ui.horizontal(|ui| {
        let hint = if is_directory {
            "/path/to/directory"
        } else {
            "/path/to/file"
        };
        let te = ui.add_enabled(!read_only, egui::TextEdit::singleline(text).hint_text(hint));
        if !read_only && ui.button("Browse…").clicked() {
            let mut dialog = rfd::FileDialog::new();
            if let Some(JsonValue::String(accept)) = schema.extensions.get("x-accept") {
                for group in accept.split(';') {
                    let parts: Vec<&str> = group.splitn(2, ':').collect();
                    if parts.len() == 2 {
                        let name = parts[0].trim();
                        let exts: Vec<&str> = parts[1]
                            .split(',')
                            .map(|e| e.trim().trim_start_matches("*."))
                            .collect();
                        dialog = dialog.add_filter(name, &exts);
                    } else {
                        let exts: Vec<&str> = parts[0]
                            .split(',')
                            .map(|e| e.trim().trim_start_matches("*."))
                            .collect();
                        dialog = dialog.add_filter("Files", &exts);
                    }
                }
            }
            let picked = if is_directory {
                dialog.pick_folder().map(|p| p.display().to_string())
            } else {
                dialog.pick_file().map(|p| p.display().to_string())
            };
            if let Some(path) = picked {
                *text = path;
            }
        }
        te
    });
    r.inner
}

/// Returns a hint text for a given format.
fn format_hint(format: &str) -> &'static str {
    match format {
        "date" => "YYYY-MM-DD",
        "date-time" => "YYYY-MM-DDThh:mm:ssZ",
        "time" => "hh:mm:ss",
        "email" => "user@example.com",
        "uri" => "https://example.com",
        "uuid" => "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx",
        "ipv4" => "0.0.0.0",
        "ipv6" => "::1",
        "duration" => "P1DT12H",
        "hostname" => "example.com",
        _ => "",
    }
}

/// Renders a field label with an optional required indicator.
fn render_label(ui: &mut Ui, label: &str, required: bool) {
    ui.horizontal(|ui| {
        ui.label(label);
        if required {
            ui.colored_label(egui::Color32::RED, "*");
        }
    });
}

impl DefaultRenderer {
    fn render_string(&self, ui: &mut Ui, ctx: &FieldContext) -> FieldAction {
        let mut action = FieldAction::None;

        if let Some(desc) = ctx.description {
            ui.small(desc);
        }

        // Enum → ComboBox
        if let Some(enum_values) = &ctx.node.schema.enum_ {
            let current = ctx.value.and_then(|v| v.as_str()).unwrap_or("").to_owned();
            let mut selected = current.clone();
            let id = ui.make_persistent_id(ctx.path);
            render_label(ui, &ctx.label, ctx.required);
            egui::ComboBox::from_id_salt(id)
                .selected_text(&selected)
                .show_ui(ui, |ui| {
                    for opt in enum_values {
                        let label = match opt {
                            JsonValue::String(s) => s.clone(),
                            other => other.to_string(),
                        };
                        ui.selectable_value(&mut selected, label.clone(), &label);
                    }
                });
            if selected != current {
                action = FieldAction::SetValue(JsonValue::String(selected));
            }
        } else {
            let format = ctx.node.schema.format.as_deref().unwrap_or("");
            let mut text = ctx.value.and_then(|v| v.as_str()).unwrap_or("").to_owned();
            let original = text.clone();

            // textarea and file-path are multi-line/complex — keep vertical layout
            match format {
                "textarea" => {
                    render_label(ui, &ctx.label, ctx.required);
                    let response = ui.add_enabled(
                        !ctx.read_only,
                        egui::TextEdit::multiline(&mut text).desired_rows(4),
                    );
                    if response.changed() || text != original {
                        action = FieldAction::SetValue(JsonValue::String(text));
                    }
                }
                "file-path" | "directory-path" => {
                    render_label(ui, &ctx.label, ctx.required);
                    let response = render_file_path(
                        ui,
                        &mut text,
                        ctx.read_only,
                        format == "directory-path",
                        &ctx.node.schema,
                    );
                    if response.changed() || text != original {
                        action = FieldAction::SetValue(JsonValue::String(text));
                    }
                }
                _ => {
                    render_label(ui, &ctx.label, ctx.required);
                    let response = match format {
                        "password" => ui.add_enabled(
                            !ctx.read_only,
                            egui::TextEdit::singleline(&mut text).password(true),
                        ),
                        _ => {
                            let hint = format_hint(format);
                            ui.add_enabled(
                                !ctx.read_only,
                                egui::TextEdit::singleline(&mut text).hint_text(hint),
                            )
                        }
                    };
                    if response.changed() || text != original {
                        action = FieldAction::SetValue(JsonValue::String(text));
                    }
                }
            }
        }

        if let Some(err) = &ctx.error_message {
            ui.colored_label(egui::Color32::RED, err);
        }
        action
    }

    fn render_number(&self, ui: &mut Ui, ctx: &FieldContext) -> FieldAction {
        let mut action = FieldAction::None;

        if let Some(desc) = ctx.description {
            ui.small(desc);
        }

        let is_integer = ctx.node.type_ == SchemaType::Integer;
        let current = ctx.value.and_then(|v| v.as_f64()).unwrap_or(0.0);

        render_label(ui, &ctx.label, ctx.required);
        let mut val = current;
        let mut dv = egui::DragValue::new(&mut val);
        if is_integer {
            dv = dv.range(
                ctx.node.schema.minimum.unwrap_or(i32::MIN as f64) as i64
                    ..=ctx.node.schema.maximum.unwrap_or(i32::MAX as f64) as i64,
            );
        } else {
            if let Some(min) = ctx.node.schema.minimum {
                dv = dv.range(min..=f64::MAX);
            }
            if let Some(max) = ctx.node.schema.maximum {
                dv = dv.range(f64::MIN..=max);
            }
            dv = dv.speed(0.1);
        }
        let response = ui.add_enabled(!ctx.read_only, dv);
        if response.changed() {
            let json_val = if is_integer {
                JsonValue::from(val as i64)
            } else {
                serde_json::Number::from_f64(val)
                    .map(JsonValue::Number)
                    .unwrap_or(JsonValue::Null)
            };
            action = FieldAction::SetValue(json_val);
        }

        if let Some(err) = &ctx.error_message {
            ui.colored_label(egui::Color32::RED, err);
        }
        action
    }

    fn render_boolean(&self, ui: &mut Ui, ctx: &FieldContext) -> FieldAction {
        let mut checked = ctx.value.and_then(|v| v.as_bool()).unwrap_or(false);
        let response = ui.add_enabled(
            !ctx.read_only,
            egui::Checkbox::new(&mut checked, &ctx.label),
        );

        if let Some(desc) = ctx.description {
            ui.small(desc);
        }

        if let Some(err) = &ctx.error_message {
            ui.colored_label(egui::Color32::RED, err);
        }

        if response.changed() {
            FieldAction::SetValue(JsonValue::Bool(checked))
        } else {
            FieldAction::None
        }
    }

    fn render_object(&self, ui: &mut Ui, ctx: &FieldContext) -> FieldAction {
        let mut action = FieldAction::None;

        ui.horizontal(|ui| {
            ui.strong(&ctx.label);
            if ctx.can_remove {
                if ui.small_button("✕").clicked() {
                    action = FieldAction::Remove;
                }
            }
        });

        if let Some(desc) = ctx.description {
            ui.small(desc);
        }

        if let Some(err) = &ctx.error_message {
            ui.colored_label(egui::Color32::RED, err);
        }

        if ctx.can_add {
            let id = ui.make_persistent_id(format!("{}_new_key", ctx.path));
            let mut new_key = ui
                .data_mut(|d| d.get_temp::<String>(id))
                .unwrap_or_default();
            ui.horizontal(|ui| {
                ui.text_edit_singleline(&mut new_key);
                if ui.button("Add Property").clicked() && !new_key.is_empty() {
                    action = FieldAction::AddChild {
                        key: Some(new_key.clone()),
                        value: None,
                    };
                    new_key.clear();
                }
            });
            ui.data_mut(|d| d.insert_temp(id, new_key));
        }

        action
    }

    fn render_array(&self, ui: &mut Ui, ctx: &FieldContext) -> FieldAction {
        let mut action = FieldAction::None;

        ui.horizontal(|ui| {
            ui.strong(&ctx.label);
            if ctx.can_remove {
                if ui.small_button("✕").clicked() {
                    action = FieldAction::Remove;
                }
            }
        });

        if let Some(desc) = ctx.description {
            ui.small(desc);
        }

        if let Some(err) = &ctx.error_message {
            ui.colored_label(egui::Color32::RED, err);
        }

        if ctx.can_add {
            if ui.button("Add Item").clicked() {
                action = FieldAction::AddChild {
                    key: None,
                    value: None,
                };
            }
        }

        action
    }

    fn render_null(&self, ui: &mut Ui, ctx: &FieldContext) -> FieldAction {
        ui.horizontal(|ui| {
            ui.label(&ctx.label);
            ui.weak("(null)");
        });
        FieldAction::None
    }

    fn render_unknown(&self, ui: &mut Ui, ctx: &FieldContext) -> FieldAction {
        ui.horizontal(|ui| {
            ui.label(&ctx.label);
            let text = ctx
                .value
                .map(|v| v.to_string())
                .unwrap_or_else(|| "(unknown)".to_string());
            ui.weak(text);
        });
        FieldAction::None
    }
}

impl FieldRenderer for DefaultRenderer {
    fn render(&self, ui: &mut Ui, ctx: &FieldContext) -> FieldAction {
        match ctx.node.type_ {
            SchemaType::String => self.render_string(ui, ctx),
            SchemaType::Number | SchemaType::Integer => self.render_number(ui, ctx),
            SchemaType::Boolean => self.render_boolean(ui, ctx),
            SchemaType::Object => self.render_object(ui, ctx),
            SchemaType::Array => self.render_array(ui, ctx),
            SchemaType::Null => self.render_null(ui, ctx),
            SchemaType::Unknown => self.render_unknown(ui, ctx),
        }
    }
}
