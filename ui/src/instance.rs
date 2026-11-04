use interprocess::local_socket::{
    GenericFilePath, GenericNamespaced, Listener, ListenerOptions, Name, Stream, ToFsName,
    ToNsName, prelude::*,
};

fn socket_file(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("{tag}.socket"))
}

fn name(tag: &str) -> std::io::Result<Name<'static>> {
    if cfg!(windows) {
        tag.to_string().to_ns_name::<GenericNamespaced>()
    } else {
        socket_file(tag).to_fs_name::<GenericFilePath>()
    }
}

pub fn already_running(tag: &str) -> bool {
    name(tag).is_ok_and(|n| Stream::connect(n).is_ok())
}

// Hold the returned lock for the life of the process. None means the lock could not be created, which is not fatal.
pub fn take_lock(tag: &str) -> Option<Listener> {
    // Callers check already_running first, so a socket file still on disk is left over from a crash.
    #[cfg(unix)]
    let _ = std::fs::remove_file(socket_file(tag));
    ListenerOptions::new()
        .name(name(tag).ok()?)
        .create_sync()
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lock_is_seen_then_released() {
        let tag = format!("goxlr-ui-test-{}", std::process::id());
        assert!(!already_running(&tag));
        let lock = take_lock(&tag);
        assert!(lock.is_some());
        assert!(already_running(&tag));
        drop(lock);
        assert!(!already_running(&tag));
    }

    #[test]
    fn stale_socket_file_is_replaced() {
        let tag = format!("goxlr-ui-stale-{}", std::process::id());
        #[cfg(unix)]
        std::fs::write(socket_file(&tag), b"").unwrap();
        assert!(!already_running(&tag));
        assert!(take_lock(&tag).is_some());
    }
}
