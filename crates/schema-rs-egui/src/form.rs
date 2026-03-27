use egui::Ui;
use schema_rs_core::SchemaRuntime;

use crate::field::{PendingAction, render_field};
use crate::renderer::{DefaultRenderer, FieldAction, FieldRenderer};

/// Top-level form widget that renders a JSON Schema form.
pub struct SchemaForm<'a> {
    runtime: &'a mut SchemaRuntime,
    renderer: &'a dyn FieldRenderer,
}

impl<'a> SchemaForm<'a> {
    pub fn new(runtime: &'a mut SchemaRuntime, renderer: &'a dyn FieldRenderer) -> Self {
        Self { runtime, renderer }
    }

    /// Render the form and apply any changes. Returns `true` if values changed.
    pub fn show(&mut self, ui: &mut Ui) -> bool {
        // Drain pending events (egui re-renders each frame anyway).
        let _ = self.runtime.drain_events();

        // Phase 1: Immutable read — render UI and collect pending actions.
        let actions: Vec<PendingAction> = {
            let rt: &SchemaRuntime = &*self.runtime;
            let root = rt.root();
            render_field(ui, rt, root, self.renderer)
        };

        // Phase 2: Mutable write — apply collected actions.
        let mut changed = false;
        for pending in actions {
            match pending.action {
                FieldAction::SetValue(v) => {
                    changed |= self.runtime.set_value(&pending.path, v);
                }
                FieldAction::Remove => {
                    changed |= self.runtime.remove_value(&pending.path);
                }
                FieldAction::AddChild { key, value } => {
                    changed |= self.runtime.add_child(&pending.path, key.as_deref(), value);
                }
                FieldAction::None => {}
            }
        }

        if changed {
            ui.ctx().request_repaint();
        }
        changed
    }
}

/// Convenience function that uses the `DefaultRenderer`.
pub fn show_schema_form(ui: &mut Ui, runtime: &mut SchemaRuntime) -> bool {
    let renderer = DefaultRenderer;
    SchemaForm::new(runtime, &renderer).show(ui)
}
