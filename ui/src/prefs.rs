use std::path::PathBuf;

fn path() -> Option<PathBuf> {
    let base = if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support"))
    } else {
        std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
    }?;
    Some(base.join("goxlr-utility").join("ui.conf"))
}

pub fn load() -> (bool, bool) {
    let text = path().and_then(|p| std::fs::read_to_string(p).ok()).unwrap_or_default();
    let flag = |k: &str, default: bool| text.lines().find_map(|l| l.strip_prefix(k)).map_or(default, |v| v.trim() == "1");
    (flag("dark=", true), flag("advanced=", false))
}

pub fn save(dark: bool, advanced: bool) {
    if let Some(p) = path() {
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(p, format!("dark={}\nadvanced={}\n", dark as u8, advanced as u8));
    }
}
