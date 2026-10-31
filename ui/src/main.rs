#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use goxlr_ui::{AppWindow, backend, wire};
use slint::ComponentHandle;

fn main() -> Result<(), slint::PlatformError> {
    // A second launch just exits, the existing window stays.
    let Ok(_guard) = std::net::TcpListener::bind(("127.0.0.1", 14599)) else { return Ok(()) };
    let ui = AppWindow::new()?;
    let (dark, advanced) = goxlr_ui::prefs::load();
    ui.global::<goxlr_ui::Theme>().set_dark(dark);
    ui.global::<goxlr_ui::Mode>().set_advanced(advanced);
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    wire(&ui, tx);
    backend::spawn(rx, ui.as_weak());
    ui.run()
}
