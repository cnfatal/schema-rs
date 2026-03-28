//! egui/eframe playground backend.

use eframe::egui;
use schema_rs::core::{DefaultValidator, SchemaRuntime};
use schema_rs::egui::{DefaultRenderer, SchemaForm, apply_default_theme};
use serde_json::Value;

use crate::schemas::example_schemas;

pub struct PlaygroundApp {
    schema_text: String,
    value_text: String,
    runtime: Option<SchemaRuntime>,
    selected_example: usize,
    parse_error: Option<String>,
    renderer: DefaultRenderer,
}

impl PlaygroundApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
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
                self.runtime =
                    Some(SchemaRuntime::new(Box::new(DefaultValidator::new()), s, v));
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

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Form Preview");
            ui.separator();

            if let Some(ref mut runtime) = self.runtime {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let changed = SchemaForm::new(runtime, &self.renderer).show(ui);
                    if changed {
                        if let Some(val) = runtime.get_value("") {
                            self.value_text =
                                serde_json::to_string_pretty(val).unwrap_or_default();
                        }
                    }
                });
            } else {
                ui.label("No runtime — fix JSON errors in the editor.");
            }
        });
    }
}

pub fn run() -> eframe::Result<()> {
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
