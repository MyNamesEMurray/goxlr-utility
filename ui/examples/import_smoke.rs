use goxlr_ipc::DaemonStatus;
use goxlr_ui::{AppWindow, wire, with_app};
use slint::PlatformError;
use slint::platform::software_renderer::{MinimalSoftwareWindow, RepaintBufferType};
use slint::platform::{Platform, WindowAdapter};
use std::rc::Rc;
use std::time::{Duration, Instant};

struct P(Rc<MinimalSoftwareWindow>, Instant);
impl Platform for P {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        Ok(self.0.clone())
    }
    fn duration_since_start(&self) -> Duration {
        self.1.elapsed()
    }
}

fn main() {
    slint::platform::set_platform(Box::new(P(
        MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer),
        Instant::now(),
    )))
    .unwrap();
    let ui = AppWindow::new().unwrap();
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    wire(&ui, tx);
    let st: DaemonStatus = serde_json::from_str(include_str!("mock.json")).unwrap();
    with_app(|a| a.on_status(Some(st)));
    ui.set_page(6);
    ui.set_profile_tab(3);
    with_app(|a| a.refresh());
    with_app(|a| a.event("imp.sel.0", 0.0, ""));
    with_app(|a| a.event("imp.go", 0.0, ""));
    for f in [
        "p/Stream.goxlr",
        "p/Gaming.goxlr",
        "p/Chill.goxlr",
        "m/Podcast.goxlrMicProfile",
    ] {
        println!(
            "{f}: {:?}",
            std::fs::read_to_string(format!("/tmp/fx/{f}")).map(|s| s.trim().to_string())
        );
    }
}
