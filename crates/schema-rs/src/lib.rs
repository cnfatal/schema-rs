//! `schema-rs` is a schema-driven native GUI framework.
//!
//! This crate serves as the main entry point, providing access to core logic
//! and optional UI backends like `egui`.

/// Core schema logic, validation, and runtime.
pub use schema_rs_core as core;

#[cfg(feature = "egui")]
/// UI components for `egui`.
pub use schema_rs_egui as egui;

#[cfg(feature = "win32")]
/// UI components for native Win32.
pub use schema_rs_win32 as win32;

// Re-export commonly used types for easier access
pub mod prelude {
    pub use schema_rs_core::{Schema, SchemaRuntime, ValidationOutput};
    #[cfg(feature = "egui")]
    pub use schema_rs_egui as gui;
}
