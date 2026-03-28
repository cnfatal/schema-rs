//! Native Win32 form rendering for JSON Schema.
//!
//! This crate provides a Win32 API-based form renderer that creates native
//! Windows controls from a `SchemaRuntime` field tree.

#[cfg(target_os = "windows")]
pub mod controls;
#[cfg(target_os = "windows")]
pub mod form;
#[cfg(target_os = "windows")]
pub mod state;
#[cfg(target_os = "windows")]
pub mod util;
#[cfg(target_os = "windows")]
pub mod wndproc;

#[cfg(target_os = "windows")]
pub use form::run_form;
#[cfg(target_os = "windows")]
pub use form::run_form_confirm;
#[cfg(target_os = "windows")]
pub use state::SchemaFormWindow;
