pub mod app;
pub mod backend;
pub mod import;
pub mod instance;
pub mod model;
pub mod pages;
pub mod prefs;
pub mod table;

slint::include_modules!();

use std::cell::RefCell;

thread_local! {
    pub static APP: RefCell<Option<app::App>> = const { RefCell::new(None) };
}

pub fn with_app(f: impl FnOnce(&mut app::App)) {
    APP.with(|a| {
        if let Some(app) = a.borrow_mut().as_mut() {
            f(app);
        }
    });
}

pub fn wire(ui: &AppWindow, tx: tokio::sync::mpsc::UnboundedSender<app::Msg>) {
    APP.with(|a| *a.borrow_mut() = Some(app::App::new(ui, tx)));
    ui.on_event(|k, v, t| with_app(|a| a.event(&k, v, &t)));
    ui.on_strip_volume(|k, v| with_app(|a| a.strip_volume(&k, v)));
    ui.on_strip_mute(|k| with_app(|a| a.strip_mute(&k)));
    ui.on_route(|i, o, on| with_app(|a| a.route(i as usize, o as usize, on)));
    ui.on_tab_changed(|_, _| with_app(|a| a.refresh()));
    ui.on_page_changed(|_| with_app(|a| a.refresh()));
    ui.on_theme_changed(prefs::save);
}
