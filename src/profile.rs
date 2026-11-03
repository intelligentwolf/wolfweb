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
