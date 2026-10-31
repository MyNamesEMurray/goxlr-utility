use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq)]
pub struct Found {
    pub path: PathBuf,
    pub mic: bool,
    pub selected: bool,
}

impl Found {
    pub fn name(&self) -> String {
        self.path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
    }

    pub fn file_name(&self) -> Option<std::ffi::OsString> {
        self.path.file_name().map(|s| s.to_os_string())
    }
}

#[derive(Default)]
pub struct ImportState {
    pub path: String,
    pub scanned: bool,
    pub files: Vec<Found>,
    pub overwrite: bool,
    pub msg: String,
}

pub fn candidates() -> Vec<PathBuf> {
    let mut out = vec![];
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from);
    if let Some(h) = home {
        out.push(h.join("Documents").join("GoXLR"));
        out.push(h.join("OneDrive").join("Documents").join("GoXLR"));
        out.push(h.join("My Documents").join("GoXLR"));
        let users = h.join(".wine/drive_c/users");
        if let Ok(rd) = std::fs::read_dir(users) {
            for u in rd.flatten() {
                out.push(u.path().join("Documents").join("GoXLR"));
                out.push(u.path().join("My Documents").join("GoXLR"));
            }
        }
    }
    out
}

pub fn first_existing() -> Option<PathBuf> {
    candidates().into_iter().find(|p| p.is_dir())
}

fn kind(p: &Path) -> Option<bool> {
    match p.extension()?.to_str()?.to_ascii_lowercase().as_str() {
        "goxlr" => Some(false),
        "goxlrmicprofile" => Some(true),
        _ => None,
    }
}

fn walk(dir: &Path, depth: u8, out: &mut Vec<Found>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        let Ok(t) = e.file_type() else { continue };
        if t.is_dir() && depth > 0 {
            walk(&p, depth - 1, out);
        } else if t.is_file() {
            if let Some(mic) = kind(&p) {
                out.push(Found { path: p, mic, selected: true });
            }
        }
    }
}

pub fn scan(dir: &Path) -> Vec<Found> {
    let mut out = vec![];
    walk(dir, 2, &mut out);
    out.sort_by(|a, b| (a.mic, a.name().to_lowercase()).cmp(&(b.mic, b.name().to_lowercase())));
    out
}

pub fn dest_exists(f: &Found, profiles: &Path, mic_profiles: &Path) -> bool {
    let dir = if f.mic { mic_profiles } else { profiles };
    f.file_name().is_some_and(|n| dir.join(n).exists())
}

pub fn run(files: &[Found], overwrite: bool, profiles: &Path, mic_profiles: &Path) -> String {
    let (mut done, mut skipped, mut failed) = (0, 0, 0);
    for f in files.iter().filter(|f| f.selected) {
        let Some(name) = f.file_name() else { continue };
        let dir = if f.mic { mic_profiles } else { profiles };
        let dest = dir.join(name);
        if dest.exists() && !overwrite {
            skipped += 1;
            continue;
        }
        if std::fs::create_dir_all(dir).is_err() || std::fs::copy(&f.path, &dest).is_err() {
            failed += 1;
        } else {
            done += 1;
        }
    }
    let mut msg = format!("Imported {done}");
    if skipped > 0 {
        msg += &format!(", skipped {skipped} that already exist");
    }
    if failed > 0 {
        msg += &format!(", {failed} failed");
    }
    msg
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("goxlr-import-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn scan_and_import() {
        let src = tmp("src");
        std::fs::create_dir_all(src.join("Profiles")).unwrap();
        std::fs::create_dir_all(src.join("MicProfiles")).unwrap();
        std::fs::write(src.join("Profiles/Stream.goxlr"), b"p").unwrap();
        std::fs::write(src.join("Profiles/Game.GOXLR"), b"p2").unwrap();
        std::fs::write(src.join("MicProfiles/Podcast.goxlrMicProfile"), b"m").unwrap();
        std::fs::write(src.join("notes.txt"), b"x").unwrap();
        let found = scan(&src);
        assert_eq!(found.len(), 3);
        assert_eq!(found.iter().filter(|f| f.mic).count(), 1);

        let (p, m) = (tmp("p"), tmp("m"));
        std::fs::write(p.join("Stream.goxlr"), b"old").unwrap();
        assert!(dest_exists(&found[1], &p, &m) || dest_exists(&found[0], &p, &m));
        let msg = run(&found, false, &p, &m);
        assert!(msg.contains("Imported 2") && msg.contains("skipped 1"), "{msg}");
        assert_eq!(std::fs::read(p.join("Stream.goxlr")).unwrap(), b"old");
        let msg = run(&found, true, &p, &m);
        assert!(msg.contains("Imported 3"), "{msg}");
        assert_eq!(std::fs::read(p.join("Stream.goxlr")).unwrap(), b"p");
        assert!(m.join("Podcast.goxlrMicProfile").exists());
    }
}
