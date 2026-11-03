//! Where WolfWeb keeps its data.

use std::path::PathBuf;

/// The CEF root_cache_path: Chromium's "Default" profile (extensions, history, bookmarks, passwords, cookies)
/// plus WolfWeb's own files (session). Linux ~/.local/share/WolfWeb, macOS ~/Library/Application Support/WolfWeb,
/// Windows %APPDATA%\WolfWeb — dirs::data_dir() per platform.
pub fn root_dir() -> std::io::Result<PathBuf> {
    let base = dirs::data_dir()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "no per-user data folder on this system"))?;
    let dir = base.join("WolfWeb");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// WolfWeb's session file (open windows and tabs), restored at the next start.
pub fn session_file() -> Option<PathBuf> {
    root_dir().ok().map(|d| d.join("wolfweb-session.json"))
}

/// Is a WolfWeb (Chromium) process using the profile right now? A stale lock left by a crash or a kill doesn't count.
///
/// Source: chromium chrome/browser/process_singleton_posix.cc:23 — Linux/macOS: `SingletonLock` is a symlink to
/// "<hostname>-<pid>" of the owning browser process; process_singleton_win.cc:244-247 — Windows: `lockfile` is held
/// open (share-read only, delete-on-close) while the browser runs.
pub fn in_use(root: &std::path::Path) -> bool {
    #[cfg(unix)]
    {
        let Ok(target) = std::fs::read_link(root.join("SingletonLock")) else { return false };
        let target = target.to_string_lossy().to_string();
        let Some((host, pid)) = target.rsplit_once('-') else { return true };
        let Ok(pid) = pid.parse::<u32>() else { return true };
        // A lock from another computer (shared home folder): assume that browser is running.
        if hostname().is_some_and(|h| h != host) {
            return true;
        }
        pid_alive(pid)
    }
    #[cfg(windows)]
    {
        let lock = root.join("lockfile");
        // While the browser holds it, deleting fails (sharing violation); a leftover file can simply go.
        lock.exists() && std::fs::remove_file(&lock).is_err()
    }
}

#[cfg(target_os = "linux")]
fn hostname() -> Option<String> {
    std::fs::read_to_string("/proc/sys/kernel/hostname").ok().map(|h| h.trim().to_string())
}

#[cfg(target_os = "macos")]
fn hostname() -> Option<String> {
    let out = std::process::Command::new("/bin/hostname").output().ok()?;
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

#[cfg(target_os = "linux")]
fn pid_alive(pid: u32) -> bool {
    std::path::Path::new(&format!("/proc/{pid}")).exists()
}

#[cfg(target_os = "macos")]
fn pid_alive(pid: u32) -> bool {
    // kill -0 checks that the process exists without signalling it.
    std::process::Command::new("/bin/kill")
        .args(["-0", &pid.to_string()])
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(true)
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    #[test]
    fn stale_locks_dont_count() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        assert!(!in_use(root));
        let host = hostname().unwrap();
        std::os::unix::fs::symlink(format!("{host}-{}", std::process::id()), root.join("SingletonLock")).unwrap();
        assert!(in_use(root), "our own live pid");
        std::fs::remove_file(root.join("SingletonLock")).unwrap();
        std::os::unix::fs::symlink(format!("{host}-4194304"), root.join("SingletonLock")).unwrap(); // above pid_max
        assert!(!in_use(root), "dead pid");
        std::fs::remove_file(root.join("SingletonLock")).unwrap();
        std::os::unix::fs::symlink("another-computer-1", root.join("SingletonLock")).unwrap();
        assert!(in_use(root), "other host");
    }
}
