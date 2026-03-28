//! Interactive playground for schema-rs.
//!
//! Supports multiple UI backends via Cargo features:
//! - `egui` (default): egui/eframe GUI with live schema editor
//! - `win32`: Native Win32 window (Windows only)
//!
//! Usage:
//!   cargo run -p schema-rs-playground                       # egui (default)
//!   cargo run -p schema-rs-playground --no-default-features --features win32  # win32

mod schemas;

#[cfg(feature = "egui")]
mod egui_app;

#[cfg(feature = "win32")]
mod win32_app;

fn main() {
    #[cfg(feature = "egui")]
    {
        egui_app::run().expect("eframe error");
    }

    #[cfg(all(feature = "win32", not(feature = "egui")))]
    {
        win32_app::run();
    }

    #[cfg(not(any(feature = "egui", feature = "win32")))]
    {
        eprintln!("No UI backend enabled. Use --features egui or --features win32");
        std::process::exit(1);
    }
}
