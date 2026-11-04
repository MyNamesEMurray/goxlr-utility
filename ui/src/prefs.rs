use std::path::PathBuf;

fn path() -> Option<PathBuf> {
    let base = if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support"))
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
    }?;
    Some(base.join("goxlr-utility").join("ui.conf"))
}

pub fn load() -> bool {
    let text = path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .unwrap_or_default();
    text.lines()
        .find_map(|l| l.strip_prefix("dark="))
        .is_none_or(|v| v.trim() == "1")
}

pub fn save(dark: bool) {
    if let Some(p) = path() {
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(p, format!("dark={}\n", dark as u8));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_choice_survives_a_restart() {
        let dir = std::env::temp_dir().join(format!("goxlr-prefs-{}", std::process::id()));
        // Only this test reads the config location, so changing it for the process is safe.
        unsafe { std::env::set_var("XDG_CONFIG_HOME", &dir) };
        save(false);
        assert!(!load());
        save(true);
        assert!(load());
    }
}
