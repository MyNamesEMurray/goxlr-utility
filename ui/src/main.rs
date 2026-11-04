#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use goxlr_ui::{AppWindow, backend, instance, wire};
use slint::ComponentHandle;

fn main() -> Result<(), slint::PlatformError> {
    // A second launch just exits, the existing window stays.
    if instance::already_running("goxlr-ui") {
        return Ok(());
    }
    let _lock = instance::take_lock("goxlr-ui");

    let ui = AppWindow::new()?;
    ui.global::<goxlr_ui::Theme>()
        .set_dark(goxlr_ui::prefs::load());
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    wire(&ui, tx);
    backend::spawn(rx, ui.as_weak());
    ui.run()
}
