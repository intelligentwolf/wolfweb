//! API actions besides import: Chromium window commands for the focused tab, WolfWeb's own engine features (the
//! native menu bar), and starting WolfWeb's partner app WolfMail.

use crate::scheme::{param, Params, Reply};
use cef::*;
use serde_json::json;
#[cfg(any(target_os = "linux", target_os = "windows"))]
use std::path::PathBuf;

/// The Chromium commands WolfWeb's menus and pages may run, by name — nothing else can be sent.
/// Source: cef/include/cef_command_ids.h (CEF 156.0.15); each value is that header's #define (line in brackets).
pub fn chrome_command(name: &str) -> Option<i32> {
    Some(match name {
        "back" => 33000,                     // IDC_BACK [43]
        "forward" => 33001,                  // IDC_FORWARD [44]
        "reload" => 33002,                   // IDC_RELOAD [45]
        "new-window" => 34000,               // IDC_NEW_WINDOW [51]
        "new-private-window" => 34001,       // IDC_NEW_INCOGNITO_WINDOW [52]
        "close-window" => 34012,             // IDC_CLOSE_WINDOW [54]
        "new-tab" => 34014,                  // IDC_NEW_TAB [55]
        "close-tab" => 34015,                // IDC_CLOSE_TAB [56]
        "next-tab" => 34016,                 // IDC_SELECT_NEXT_TAB [57]
        "previous-tab" => 34017,             // IDC_SELECT_PREVIOUS_TAB [58]
        "duplicate-tab" => 34027,            // IDC_DUPLICATE_TAB [70]
        "reopen-tab" => 34028,               // IDC_RESTORE_TAB [71]
        "fullscreen" => 34030,               // IDC_FULLSCREEN [73]
        "exit" => 34031,                     // IDC_EXIT [74]
        "split-view" => 34057,               // IDC_NEW_SPLIT_TAB [88]
        "vertical-tabs" => 34058,            // IDC_TOGGLE_VERTICAL_TABS [89]
        "vertical-tabs-collapse" => 34061,   // IDC_TOGGLE_VERTICAL_TABS_COLLAPSE [92]
        "bookmark-this-tab" => 35000,        // IDC_BOOKMARK_THIS_TAB [110]
        "view-source" => 35002,              // IDC_VIEW_SOURCE [112]
        "print" => 35003,                    // IDC_PRINT [113]
        "save-page" => 35004,                // IDC_SAVE_PAGE [114]
        "passwords" => 35041,                // IDC_SHOW_PASSWORD_MANAGER [139]
        "pin-tab" => 35051,                  // IDC_PIN_TARGET_TAB [147]
        "cut" => 36000,                      // IDC_CUT [150]
        "copy" => 36001,                     // IDC_COPY [151]
        "paste" => 36003,                    // IDC_PASTE [152]
        "find" => 37000,                     // IDC_FIND [153]
        "zoom-in" => 38001,                  // IDC_ZOOM_PLUS [168]
        "zoom-reset" => 38002,               // IDC_ZOOM_NORMAL [169]
        "zoom-out" => 38003,                 // IDC_ZOOM_MINUS [170]
        "open-file" => 40000,                // IDC_OPEN_FILE [180]
        "dev-tools" => 40004,                // IDC_DEV_TOOLS [182]
        "task-manager" => 40006,             // IDC_TASK_MANAGER [184]
        "history" => 40010,                  // IDC_SHOW_HISTORY [188]
        "bookmark-manager" => 40011,         // IDC_SHOW_BOOKMARK_MANAGER [189]
        "downloads" => 40012,                // IDC_SHOW_DOWNLOADS [190]
        "clear-browsing-data" => 40013,      // IDC_CLEAR_BROWSING_DATA [191]
        "settings" => 40015,                 // IDC_OPTIONS [193]
        "about" => 40018,                    // IDC_ABOUT [196]
        "extensions" => 40022,               // IDC_MANAGE_EXTENSIONS [200]
        "tab-search" => 52500,               // IDC_TAB_SEARCH [395]
        _ => return None,
    })
}

/// Run a named command on `host`'s window. Err with a message fit to show.
pub fn run_chrome_command(host: &BrowserHost, name: &str) -> Result<(), &'static str> {
    let Some(id) = chrome_command(name) else { return Err("Unknown command") };
    if host.can_execute_chrome_command(id) == 0 {
        return Err("That isn't available in this window right now.");
    }
    host.execute_chrome_command(id, WindowOpenDisposition::CURRENT_TAB);
    Ok(())
}

pub fn api(cmd: &str, params: &Params) -> Reply {
    match cmd {
        // UI thread (scheme.rs thread_for).
        "browser/command" => {
            let name = param(params, "name").unwrap_or("");
            let Some(host) = crate::tabs::focused_browser().and_then(|b| b.host()) else {
                return Reply::error(409, "Open a WolfWeb window first.");
            };
            match run_chrome_command(&host, name) {
                Ok(()) => Reply::json(&json!({ "ok": true })),
                Err(e) => Reply::error(409, e),
            }
        }
        "engine/features" => {
            let Some(context) = profile_context() else {
                return Reply::error(409, "Open a WolfWeb window first.");
            };
            Reply::json(&json!({ "ok": true, "nativeMenuBar": has_native_menu_bar(&context) }))
        }
        "menu-bar/set" => {
            let on = param(params, "on") == Some("1");
            let Some(context) = profile_context() else {
                return Reply::error(409, "Open a WolfWeb window first.");
            };
            if !has_native_menu_bar(&context) {
                return Reply::error(409, "This WolfWeb's engine has no built-in menu bar.");
            }
            let Some(value) = crate::defaults::bool_value(on) else {
                return Reply::error(500, "Couldn't make the setting.");
            };
            if crate::defaults::set_preference(&context, MENU_BAR_PREF, value) {
                Reply::json(&json!({ "ok": true }))
            } else {
                Reply::error(500, "WolfWeb couldn't change the menu bar setting.")
            }
        }
        // Blocking thread.
        "default-browser/status" => match crate::default_browser::is_default() {
            Ok(is_default) => Reply::json(&json!({ "ok": true, "isDefault": is_default })),
            Err(e) => Reply::error(500, &e),
        },
        "default-browser/set" => match crate::default_browser::make_default() {
            Ok(message) => Reply::json(&json!({ "ok": true, "message": message })),
            Err(e) => Reply::error(500, &e),
        },
        "wolfmail/open" => match open_wolfmail() {
            Ok(()) => Reply::json(&json!({ "ok": true })),
            Err(e) => Reply::error(404, &e),
        },
        _ => Reply::error(404, "Unknown request"),
    }
}

/// The profile preference WolfWeb's own engine adds for its menu bar (build/cef/patches/wolfweb-menu-bar.patch:
/// WolfWebMenuBar::kPrefName, registered in chrome/browser/ui/browser_ui_prefs.cc RegisterBrowserUserPrefs).
pub const MENU_BAR_PREF: &str = "wolfweb.menu_bar";

/// The profile's request context, from any open browser (it's initialized by then; cef_browser.h GetRequestContext).
fn profile_context() -> Option<RequestContext> {
    crate::tabs::focused_browser().and_then(|b| b.host()).and_then(|h| h.request_context())
}

/// Does this engine have WolfWeb's built-in menu bar? Only an engine built with the patch registers its preference;
/// the standard CEF build doesn't (then the extension's bookmarks-bar menus stand in).
fn has_native_menu_bar(context: &RequestContext) -> bool {
    context.has_preference(Some(&CefString::from(MENU_BAR_PREF))) != 0
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
        if let Some(exe) = crate::util::find_program("wolfmail") {
            return spawn(&exe, &[]);
        }
        if crate::util::find_program("flatpak").is_some() {
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
        assert_eq!(chrome_command("find"), Some(37000));
        assert_eq!(chrome_command("new-private-window"), Some(34001));
        assert_eq!(chrome_command("format-disk"), None);
        assert_eq!(chrome_command(""), None);
    }
}
