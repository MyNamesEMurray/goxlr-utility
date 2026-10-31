use goxlr_ipc::DaemonStatus;
use goxlr_ui::app::Msg;
use goxlr_ui::{AppWindow, wire, with_app};
use slint::platform::software_renderer::{MinimalSoftwareWindow, RepaintBufferType};
use slint::platform::{Platform, WindowAdapter};
use slint::PlatformError;
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
    let w = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    slint::platform::set_platform(Box::new(P(w, Instant::now()))).unwrap();
    let ui = AppWindow::new().unwrap();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    wire(&ui, tx);
    let st: DaemonStatus = serde_json::from_str(include_str!("mock.json")).unwrap();
    with_app(|a| a.on_status(Some(st)));
    with_app(|a| a.event("x", 0.0, ""));
    ui.set_page(5);
    ui.set_light_tab(1);
    with_app(|a| a.refresh());
    let events: &[(&str, f32, &str)] = &[
        ("mic.gain", 55.0, ""), ("gate.on", 1.0, ""), ("gate.atk", 3.0, ""), ("fx.rev.amount", 40.0, ""), ("fx.robot.lg", -3.0, ""),
        ("mix.cough", 1.0, ""), ("mix.fch.A", 2.0, ""), ("scr.top.A", 0.0, "hi"), ("sub.vol.2", 77.0, ""), ("sub.link.2", 1.0, ""),
        ("eq.gain.3", 4.0, ""), ("eq.freq.3", 0.0, "250"), ("samp.mode.A.TopLeft", 5.0, ""), ("samp.s.A.TopLeft.0|0", 0.0, ""),
        ("samp.s.A.TopLeft.0|1", 0.0, ""), ("samp.s.A.TopLeft.0|1", 0.0, ""), ("light.fader.A:1", 0xFF8800 as f32, ""),
        ("light.simple.0:0", 0x112233 as f32, ""), ("prof.2|0", 0.0, ""), ("prof.saveas", 0.0, "NewName"), ("path.0|0", 0.0, ""),
        ("d.tray", 0.0, ""), ("d.log", 4.0, ""), ("set.disp.1", 1.0, ""), ("fx.load", 1.0, ""),
    ];
    for (k, v, t) in events {
        with_app(|a| a.event(k, *v, t));
        while let Ok(m) = rx.try_recv() {
            match m {
                Msg::Cmd(s, c) => println!("{k:28} -> Cmd({s}, {c:?})"),
                Msg::Daemon(c) => println!("{k:28} -> Daemon({c:?})"),
                Msg::Mic(m) => println!("{k:28} -> Mic({m:?})"),
            }
        }
    }
}
