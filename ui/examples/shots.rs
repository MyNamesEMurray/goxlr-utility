use goxlr_ui::{AppWindow, Theme, wire, with_app};
use goxlr_ipc::DaemonStatus;
use slint::platform::software_renderer::{MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType};
use slint::platform::{Platform, WindowAdapter};
use slint::{ComponentHandle, PhysicalSize, PlatformError};
use std::rc::Rc;
use std::time::{Duration, Instant};

struct P {
    window: Rc<MinimalSoftwareWindow>,
    start: Instant,
}

impl Platform for P {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        Ok(self.window.clone())
    }
    fn duration_since_start(&self) -> Duration {
        self.start.elapsed()
    }
}

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| "/tmp/shots".into());
    std::fs::create_dir_all(&out).unwrap();
    let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    slint::platform::set_platform(Box::new(P { window: window.clone(), start: Instant::now() })).unwrap();
    let ui = AppWindow::new().unwrap();
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    wire(&ui, tx);
    let st: DaemonStatus = serde_json::from_str(include_str!("mock.json")).expect("mock json");
    let (w, h) = (1180u32, 800u32);
    window.set_size(PhysicalSize::new(w, h));
    ui.show().unwrap();
    with_app(|a| a.on_status(Some(st)));
    let names = ["mixer", "routing", "mic", "effects", "sampler", "lighting", "profiles", "settings"];
    let tabs: [&[i32]; 8] = [&[0], &[0], &[0, 1, 2, 3], &[0, 1, 6], &[0, 3], &[0, 1, 2, 3], &[0, 1, 2], &[0, 1, 2]];
    for dark in [true, false] {
        ui.global::<Theme>().set_dark(dark);
        for (p, name) in names.iter().enumerate() {
            for &t in tabs[p] {
                ui.set_page(p as i32);
                match p {
                    2 => ui.set_mic_tab(t),
                    3 => ui.set_fx_tab(t),
                    4 => ui.set_sampler_tab(t),
                    5 => ui.set_light_tab(t),
                    6 => ui.set_profile_tab(t),
                    7 => ui.set_settings_tab(t),
                    _ => {}
                }
                with_app(|a| a.refresh());
                slint::platform::update_timers_and_animations();
                let mut buf = vec![PremultipliedRgbaColor::default(); (w * h) as usize];
                window.draw_if_needed(|r| {
                    r.render(&mut buf, w as usize);
                });
                let mut img = Vec::with_capacity(buf.len() * 4);
                for px in &buf {
                    img.extend_from_slice(&[px.red, px.green, px.blue, 255]);
                }
                let file = std::fs::File::create(format!("{out}/{}-{name}-{t}.png", if dark { "dark" } else { "light" })).unwrap();
                let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w, h);
                enc.set_color(png::ColorType::Rgba);
                enc.set_depth(png::BitDepth::Eight);
                enc.write_header().unwrap().write_image_data(&img).unwrap();
            }
        }
    }
}
