//! API actions besides import: Chromium window commands for the focused tab, and starting WolfMail.

use crate::scheme::{param, Params, Reply};
use cef::*;
use serde_json::json;
#[cfg(any(target_os = "linux", target_os = "windows"))]
use std::path::PathBuf;

// Source: cef/include/cef_command_ids.h (CEF 156.0.15) — 88 IDC_NEW_SPLIT_TAB 34057, 89 IDC_TOGGLE_VERTICAL_TABS
// 34058, 92 IDC_TOGGLE_VERTICAL_TABS_COLLAPSE 34061, 395 IDC_TAB_SEARCH 52500.
const IDC_NEW_SPLIT_TAB: i32 = 34057;
const IDC_TOGGLE_VERTICAL_TABS: i32 = 34058;
const IDC_TOGGLE_VERTICAL_TABS_COLLAPSE: i32 = 34061;
const IDC_TAB_SEARCH: i32 = 52500;

/// The Chromium commands the WolfWeb extension may run, by name. Nothing else can be sent.
fn chrome_command(name: &str) -> Option<i32> {
    Some(match name {
        "split-view" => IDC_NEW_SPLIT_TAB,
        "vertical-tabs" => IDC_TOGGLE_VERTICAL_TABS,
        "vertical-tabs-collapse" => IDC_TOGGLE_VERTICAL_TABS_COLLAPSE,
        "tab-search" => IDC_TAB_SEARCH,
        _ => return None,
    })
}

pub fn api(cmd: &str, params: &Params) -> Reply {
    match cmd {
        // UI thread (scheme.rs thread_for).
        "browser/command" => {
            let Some(id) = param(params, "name").and_then(chrome_command) else {
                return Reply::error(400, "Unknown command");
            };
            let Some(host) = crate::tabs::focused_browser().and_then(|b| b.host()) else {
                return Reply::error(409, "Open a WolfWeb window first.");
            };
            if host.can_execute_chrome_command(id) == 0 {
                return Reply::error(409, "That isn't available in this window right now.");
            }
            host.execute_chrome_command(id, WindowOpenDisposition::CURRENT_TAB);
            Reply::json(&json!({ "ok": true }))
        }
        // Blocking thread.
        "wolfmail/open" => match open_wolfmail() {
            Ok(()) => Reply::json(&json!({ "ok": true })),
            Err(e) => Reply::error(404, &e),
        },
        _ => Reply::error(404, "Unknown request"),
    }
}

/// Start WolfMail where its installers put it.
/// Linux: `wolfmail` on PATH (the .deb installs /usr/bin/wolfmail; Paul's build is ~/.local/bin/wolfmail), else the
/// Flatpak (id uk.co.intelligentwolf.wolfmail — wolfmail/src-tauri/tauri.conf.json "identifier").
/// macOS: the app bundle by identifier (`open -b`). Windows: Tauri's NSIS installer puts it in
/// %LOCALAPPDATA%\WolfMail by default (tauri-bundler installer.nsi:546-547, tauri-utils config.rs:831-832
/// NSISInstallerMode `#[default] CurrentUser`), %ProgramFiles%\WolfMail when installed for all users.
fn open_wolfmail() -> Result<(), String> {
    const NOT_FOUND: &str = "WolfMail isn't installed on this computer. Get it at github.com/intelligentwolf/wolfmail/releases.";
    #[cfg(target_os = "linux")]
    {
        if let Some(exe) = find_on_path("wolfmail") {
            return spawn(&exe, &[]);
        }
        if find_on_path("flatpak").is_some() {
            let installed = std::process::Command::new("flatpak")
                .args(["info", "uk.co.intelligentwolf.wolfmail"])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
            if installed {
                return spawn(&PathBuf::from("flatpak"), &["run", "uk.co.intelligentwolf.wolfmail"]);
            }
        }
        Err(NOT_FOUND.to_string())
    }
    #[cfg(target_os = "macos")]
    {
        let status = std::process::Command::new("open")
            .args(["-b", "uk.co.intelligentwolf.wolfmail"])
            .status()
            .map_err(|e| format!("Couldn't start WolfMail: {e}"))?;
        if status.success() { Ok(()) } else { Err(NOT_FOUND.to_string()) }
    }
    #[cfg(target_os = "windows")]
    {
        let dirs: Vec<PathBuf> = [std::env::var_os("LOCALAPPDATA"), std::env::var_os("ProgramFiles")]
            .into_iter()
            .flatten()
            .map(|d| PathBuf::from(d).join("WolfMail"))
            .collect();
        for dir in dirs {
            for name in ["wolfmail.exe", "WolfMail.exe"] {
                let exe = dir.join(name);
                if exe.is_file() {
                    return spawn(&exe, &[]);
                }
            }
        }
        Err(NOT_FOUND.to_string())
    }
}

/// PATH plus the usual install folders (a WolfWeb started from the desktop menu may not have ~/.local/bin on PATH).
#[cfg(target_os = "linux")]
fn find_on_path(name: &str) -> Option<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect()).unwrap_or_default();
    if let Some(home) = dirs::home_dir() {
        dirs.push(home.join(".local/bin"));
    }
    dirs.extend(["/usr/local/bin", "/usr/bin"].map(PathBuf::from));
    dirs.into_iter().map(|d| d.join(name)).find(|p| p.is_file())
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
fn spawn(exe: &std::path::Path, args: &[&str]) -> Result<(), String> {
    std::process::Command::new(exe)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map(|mut child| {
            // Reap it when it exits, so it doesn't linger as a zombie while WolfWeb runs.
            std::thread::spawn(move || {
                let _ = child.wait();
            });
        })
        .map_err(|e| format!("Couldn't start WolfMail: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_listed_commands_are_allowed() {
        assert_eq!(chrome_command("vertical-tabs"), Some(34058));
        assert_eq!(chrome_command("split-view"), Some(34057));
        assert_eq!(chrome_command("exit"), None);
        assert_eq!(chrome_command(""), None);
    }
}
