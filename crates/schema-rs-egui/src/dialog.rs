use schema_rs_core::{DefaultValidator, JsonValue, SchemaRuntime};

use crate::form::show_schema_form;

struct ConfirmFormApp {
    runtime: SchemaRuntime,
    confirmed: Option<bool>,
}

impl eframe::App for ConfirmFormApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .max_height(ui.available_height() - 40.0)
                .show(ui, |ui| {
                    show_schema_form(ui, &mut self.runtime);
                });

            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("确定").clicked() {
                    self.confirmed = Some(true);
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                if ui.button("取消").clicked() {
                    self.confirmed = Some(false);
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
        });
    }
}

/// Show a blocking dialog with a JSON Schema form and OK/Cancel buttons.
///
/// Returns `Some(value)` if the user confirmed, or `None` if cancelled / closed.
///
/// This mirrors the `schema_rs_win32::run_form_confirm` API.
pub fn run_form_confirm(
    schema: JsonValue,
    value: JsonValue,
    _on_change: Option<Box<dyn FnMut(&JsonValue)>>,
) -> Option<JsonValue> {
    let title = schema
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("Form")
        .to_string();

    let validator = Box::new(DefaultValidator::new());
    let runtime = SchemaRuntime::new(validator, schema, value);

    let app = ConfirmFormApp {
        runtime,
        confirmed: None,
    };

    // We need a way to get the result back after eframe::run_native returns.
    // eframe takes ownership, so we use a shared pointer.
    let result: std::sync::Arc<std::sync::Mutex<Option<JsonValue>>> =
        std::sync::Arc::new(std::sync::Mutex::new(None));
    let result_clone = result.clone();

    // Wrap in a cell so we can move the app into the closure.
    let app_cell = std::sync::Mutex::new(Some(app));

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([480.0, 640.0])
            .with_title(&title),
        ..Default::default()
    };

    let _ = eframe::run_native(
        &title,
        options,
        Box::new(move |_cc| {
            let app = app_cell.lock().unwrap().take().unwrap();
            // Wrap the app to capture the result on close.
            Ok(Box::new(ResultCapture {
                inner: app,
                result: result_clone,
            }))
        }),
    );

    std::sync::Arc::try_unwrap(result)
        .ok()
        .and_then(|m| m.into_inner().ok())
        .flatten()
}

struct ResultCapture {
    inner: ConfirmFormApp,
    result: std::sync::Arc<std::sync::Mutex<Option<JsonValue>>>,
}

impl eframe::App for ResultCapture {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.inner.update(ctx, frame);

        if let Some(true) = self.inner.confirmed {
            let value = self
                .inner
                .runtime
                .get_value("")
                .cloned()
                .unwrap_or(JsonValue::Null);
            *self.result.lock().unwrap() = Some(value);
        }
    }
}
