use eframe::egui;
use schema_rs::core::{DefaultValidator, SchemaRuntime};
use schema_rs::egui::{DefaultRenderer, SchemaForm, apply_default_theme};
use serde_json::{Value, json};

/// Built-in example schemas for the playground.
fn example_schemas() -> Vec<(&'static str, Value, Value)> {
    vec![
        (
            "Simple Object",
            json!({
                "title": "User Profile",
                "type": "object",
                "required": ["name", "age"],
                "properties": {
                    "name": { "type": "string", "title": "Name", "description": "Your full name" },
                    "age": { "type": "integer", "title": "Age", "minimum": 0, "maximum": 150 },
                    "email": { "type": "string", "title": "Email", "format": "email" },
                    "bio": { "type": "string", "title": "Bio", "description": "Short biography" }
                }
            }),
            json!({ "name": "Alice", "age": 30 }),
        ),
        (
            "Nested Object",
            json!({
                "title": "Address Book Entry",
                "type": "object",
                "required": ["name"],
                "properties": {
                    "name": { "type": "string", "title": "Name" },
                    "address": {
                        "type": "object",
                        "title": "Address",
                        "properties": {
                            "street": { "type": "string", "title": "Street" },
                            "city": { "type": "string", "title": "City" },
                            "zip": { "type": "string", "title": "ZIP Code" }
                        },
                        "required": ["city"]
                    }
                }
            }),
            json!({ "name": "Bob", "address": { "city": "Beijing" } }),
        ),
        (
            "Array",
            json!({
                "title": "Todo List",
                "type": "object",
                "properties": {
                    "title": { "type": "string", "title": "List Title" },
                    "items": {
                        "type": "array",
                        "title": "Tasks",
                        "items": {
                            "type": "object",
                            "required": ["task"],
                            "properties": {
                                "task": { "type": "string", "title": "Task" },
                                "done": { "type": "boolean", "title": "Done", "default": false }
                            }
                        }
                    }
                }
            }),
            json!({
                "title": "My Todos",
                "items": [
                    { "task": "Buy groceries", "done": true },
                    { "task": "Write code", "done": false }
                ]
            }),
        ),
        (
            "Enum & Const & Default",
            json!({
                "title": "Settings",
                "type": "object",
                "required": ["theme"],
                "properties": {
                    "theme": {
                        "type": "string",
                        "title": "Theme",
                        "enum": ["light", "dark", "auto"]
                    },
                    "language": {
                        "type": "string",
                        "title": "Language",
                        "enum": ["en", "zh", "ja", "ko"],
                        "default": "en"
                    },
                    "version": {
                        "type": "string",
                        "title": "Version (const, read-only)",
                        "const": "1.0.0"
                    },
                    "notifications": {
                        "type": "boolean",
                        "title": "Enable Notifications",
                        "default": true
                    }
                }
            }),
            // Start empty: const "version" auto-fills, default "language"/"notifications" auto-fill
            json!({}),
        ),
        (
            "Conditional (if/then/else)",
            json!({
                "title": "Payment",
                "type": "object",
                "required": ["method"],
                "properties": {
                    "method": {
                        "type": "string",
                        "title": "Payment Method",
                        "enum": ["credit_card", "bank_transfer", "paypal"]
                    }
                },
                "if": {
                    "properties": { "method": { "const": "credit_card" } }
                },
                "then": {
                    "properties": {
                        "card_number": { "type": "string", "title": "Card Number" },
                        "expiry": { "type": "string", "title": "Expiry Date" }
                    },
                    "required": ["card_number"]
                },
                "else": {
                    "if": {
                        "properties": { "method": { "const": "bank_transfer" } }
                    },
                    "then": {
                        "properties": {
                            "iban": { "type": "string", "title": "IBAN" }
                        },
                        "required": ["iban"]
                    },
                    "else": {
                        "properties": {
                            "paypal_email": { "type": "string", "title": "PayPal Email" }
                        }
                    }
                }
            }),
            json!({ "method": "credit_card" }),
        ),
        (
            "String Formats",
            json!({
                "title": "String Format Examples",
                "type": "object",
                "properties": {
                    "email": { "type": "string", "title": "Email", "format": "email" },
                    "website": { "type": "string", "title": "Website", "format": "uri" },
                    "birthday": { "type": "string", "title": "Birthday", "format": "date" },
                    "created_at": { "type": "string", "title": "Created At", "format": "date-time" },
                    "password": { "type": "string", "title": "Password", "format": "password" },
                    "host": { "type": "string", "title": "Hostname", "format": "hostname" },
                    "ip": { "type": "string", "title": "IPv4 Address", "format": "ipv4" },
                    "notes": { "type": "string", "title": "Notes", "format": "textarea" }
                }
            }),
            json!({}),
        ),
        (
            "File Path",
            json!({
                "title": "File Selection",
                "type": "object",
                "properties": {
                    "config": {
                        "type": "string",
                        "title": "Config File",
                        "format": "file-path"
                    },
                    "image": {
                        "type": "string",
                        "title": "Image File",
                        "format": "file-path",
                        "x-accept": "Images:jpg,png,webp,gif"
                    },
                    "output_dir": {
                        "type": "string",
                        "title": "Output Directory",
                        "format": "directory-path"
                    }
                }
            }),
            json!({}),
        ),
        (
            "Tabs Layout",
            json!({
                "title": "Server Configuration",
                "type": "object",
                "x-layout": "tabs",
                "properties": {
                    "general": {
                        "type": "object",
                        "title": "General",
                        "properties": {
                            "name": { "type": "string", "title": "Server Name" },
                            "port": { "type": "integer", "title": "Port", "default": 8080 },
                            "debug": { "type": "boolean", "title": "Debug Mode", "default": false }
                        }
                    },
                    "database": {
                        "type": "object",
                        "title": "Database",
                        "properties": {
                            "host": { "type": "string", "title": "DB Host", "default": "localhost" },
                            "port": { "type": "integer", "title": "DB Port", "default": 5432 },
                            "name": { "type": "string", "title": "DB Name" },
                            "user": { "type": "string", "title": "DB User" }
                        }
                    },
                    "logging": {
                        "type": "object",
                        "title": "Logging",
                        "properties": {
                            "level": {
                                "type": "string",
                                "title": "Log Level",
                                "enum": ["debug", "info", "warn", "error"],
                                "default": "info"
                            },
                            "file": {
                                "type": "string",
                                "title": "Log File",
                                "format": "file-path"
                            }
                        }
                    }
                }
            }),
            json!({}),
        ),
        (
            "Table Layout",
            json!({
                "title": "Team Members",
                "type": "object",
                "properties": {
                    "team_name": { "type": "string", "title": "Team Name" },
                    "members": {
                        "type": "array",
                        "title": "Members",
                        "x-layout": "table",
                        "items": {
                            "type": "object",
                            "properties": {
                                "name": { "type": "string", "title": "Name" },
                                "role": {
                                    "type": "string",
                                    "title": "Role",
                                    "enum": ["Developer", "Designer", "Manager", "QA"]
                                },
                                "email": { "type": "string", "title": "Email", "format": "email" },
                                "active": { "type": "boolean", "title": "Active", "default": true }
                            }
                        }
                    }
                }
            }),
            json!({
                "team_name": "Engineering",
                "members": [
                    { "name": "Alice", "role": "Developer", "email": "alice@example.com", "active": true },
                    { "name": "Bob", "role": "Designer", "email": "bob@example.com", "active": true },
                    { "name": "Charlie", "role": "Manager", "email": "charlie@example.com", "active": false }
                ]
            }),
        ),
        (
            "Collapsible",
            json!({
                "title": "Application Config",
                "type": "object",
                "properties": {
                    "app_name": { "type": "string", "title": "Application Name" },
                    "network": {
                        "type": "object",
                        "title": "Network Settings",
                        "x-collapsible": true,
                        "properties": {
                            "host": { "type": "string", "title": "Host", "default": "0.0.0.0" },
                            "port": { "type": "integer", "title": "Port", "default": 3000 },
                            "tls": { "type": "boolean", "title": "Enable TLS", "default": false }
                        }
                    },
                    "advanced": {
                        "type": "object",
                        "title": "Advanced Settings",
                        "x-collapsible": true,
                        "properties": {
                            "max_connections": { "type": "integer", "title": "Max Connections", "default": 100 },
                            "timeout": { "type": "integer", "title": "Timeout (ms)", "default": 30000 },
                            "retry": { "type": "boolean", "title": "Auto Retry", "default": true }
                        }
                    }
                }
            }),
            json!({ "app_name": "MyApp" }),
        ),
    ]
}

struct PlaygroundApp {
    schema_text: String,
    value_text: String,
    runtime: Option<SchemaRuntime>,
    selected_example: usize,
    parse_error: Option<String>,
    renderer: DefaultRenderer,
}

impl PlaygroundApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        apply_default_theme(&cc.egui_ctx);

        let examples = example_schemas();
        let (_, schema, value) = &examples[0];
        let schema_text = serde_json::to_string_pretty(schema).unwrap();
        let value_text = serde_json::to_string_pretty(value).unwrap();
        let runtime = SchemaRuntime::new(
            Box::new(DefaultValidator::new()),
            schema.clone(),
            value.clone(),
        );

        Self {
            schema_text,
            value_text,
            runtime: Some(runtime),
            selected_example: 0,
            parse_error: None,
            renderer: DefaultRenderer,
        }
    }

    fn rebuild_runtime(&mut self) {
        let schema: Result<Value, _> = serde_json::from_str(&self.schema_text);
        let value: Result<Value, _> = serde_json::from_str(&self.value_text);

        match (schema, value) {
            (Ok(s), Ok(v)) => {
                self.runtime = Some(SchemaRuntime::new(Box::new(DefaultValidator::new()), s, v));
                self.parse_error = None;
            }
            (Err(e), _) => {
                self.parse_error = Some(format!("Schema JSON error: {e}"));
            }
            (_, Err(e)) => {
                self.parse_error = Some(format!("Value JSON error: {e}"));
            }
        }
    }
}

impl eframe::App for PlaygroundApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Top bar: example selector
        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("schema-rs playground");
                ui.separator();
                ui.label("Examples:");
                let examples = example_schemas();
                for (i, (name, _, _)) in examples.iter().enumerate() {
                    if ui
                        .selectable_label(self.selected_example == i, *name)
                        .clicked()
                        && self.selected_example != i
                    {
                        self.selected_example = i;
                        let (_, schema, value) = &examples[i];
                        self.schema_text = serde_json::to_string_pretty(schema).unwrap();
                        self.value_text = serde_json::to_string_pretty(value).unwrap();
                        self.rebuild_runtime();
                    }
                }
            });
        });

        // Left panel: Schema & Value JSON editors
        egui::SidePanel::left("editor_panel")
            .default_width(400.0)
            .resizable(true)
            .show(ctx, |ui| {
                ui.heading("Schema");
                let schema_resp = egui::ScrollArea::vertical()
                    .id_salt("schema_editor")
                    .max_height(ui.available_height() * 0.45)
                    .show(ui, |ui| {
                        ui.add(
                            egui::TextEdit::multiline(&mut self.schema_text)
                                .code_editor()
                                .desired_width(f32::INFINITY),
                        )
                    });

                ui.separator();
                ui.heading("Value");
                let value_resp = egui::ScrollArea::vertical()
                    .id_salt("value_editor")
                    .max_height(ui.available_height() * 0.7)
                    .show(ui, |ui| {
                        ui.add(
                            egui::TextEdit::multiline(&mut self.value_text)
                                .code_editor()
                                .desired_width(f32::INFINITY),
                        )
                    });

                ui.separator();

                if ui.button("Apply Changes").clicked()
                    || schema_resp.inner.lost_focus()
                    || value_resp.inner.lost_focus()
                {
                    self.rebuild_runtime();
                }

                if let Some(ref err) = self.parse_error {
                    ui.colored_label(egui::Color32::RED, err);
                }
            });

        // Center panel: rendered form
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Form Preview");
            ui.separator();

            if let Some(ref mut runtime) = self.runtime {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let changed = SchemaForm::new(runtime, &self.renderer).show(ui);
                    if changed {
                        // Sync the value text editor with the updated runtime value.
                        if let Some(val) = runtime.get_value("") {
                            self.value_text = serde_json::to_string_pretty(val).unwrap_or_default();
                        }
                    }
                });
            } else {
                ui.label("No runtime — fix JSON errors in the editor.");
            }
        });
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_title("schema-rs playground"),
        ..Default::default()
    };
    eframe::run_native(
        "schema-rs-playground",
        options,
        Box::new(|cc| Ok(Box::new(PlaygroundApp::new(cc)))),
    )
}
