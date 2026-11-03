//! Remember open windows and tabs between runs. Private (incognito) tabs are never saved.

use cef::*;
use serde::{Deserialize, Serialize};
use std::cell::Cell;

#[derive(Serialize, Deserialize, Default, Debug, PartialEq)]
pub struct Session {
    pub windows: Vec<SavedWindow>,
}

#[derive(Serialize, Deserialize, Default, Debug, PartialEq)]
pub struct SavedWindow {
    pub tabs: Vec<String>,
}

thread_local! {
    static SAVE_PENDING: Cell<bool> = const { Cell::new(false) };
}

/// At startup: open what was asked for on the command line, else the last session, else a new-tab window.
pub fn restore_or_open(urls: Vec<String>) {
    if !urls.is_empty() {
        crate::tabs::open_window_with_tabs(urls);
        return;
    }
    let saved = load().unwrap_or_default();
    if saved.windows.is_empty() {
        crate::tabs::open_window_with_tabs(Vec::new());
        return;
    }
    // One window at a time: the tabs of a window are requested from that window's first browser.
    for w in saved.windows {
        crate::tabs::open_window_with_tabs(w.tabs);
    }
}

fn load() -> Option<Session> {
    let path = crate::profile::session_file()?;
    let text = std::fs::read_to_string(path).ok()?;
    parse(&text)
}

fn parse(text: &str) -> Option<Session> {
    let mut s: Session = serde_json::from_str(text).ok()?;
    for w in &mut s.windows {
        w.tabs.retain(|u| restorable(u));
        // Chromium's own new-tab page address (what a new tab showed before WolfWeb's page replaced it) reopens
        // as WolfWeb's new-tab page.
        for u in &mut w.tabs {
            if u.starts_with("chrome://new-tab-page") {
                *u = "chrome://newtab/".to_string();
            }
        }
    }
    s.windows.retain(|w| !w.tabs.is_empty());
    Some(s)
}

fn restorable(url: &str) -> bool {
    let u = url.to_ascii_lowercase();
    ["http://", "https://", "chrome://", "chrome-extension://", "wolfweb://app/", "file://"].iter().any(|p| u.starts_with(p))
}

/// Group live, non-private tabs by their top-level window.
fn snapshot() -> Session {
    let mut windows: Vec<(u64, SavedWindow)> = Vec::new();
    for b in crate::tabs::browsers() {
        let Some(host) = b.host() else { continue };
        if is_private(&host) {
            continue;
        }
        let url = b.main_frame().map(|f| CefString::from(&f.url()).to_string()).unwrap_or_default();
        if url.is_empty() {
            continue;
        }
        // cef_window_handle_t is an integer on Linux but a pointer (HWND) on Windows.
        #[allow(clippy::unnecessary_cast)]
        let key = host.window_handle() as u64;
        match windows.iter_mut().find(|(k, _)| *k == key) {
            Some((_, w)) => w.tabs.push(url),
            None => windows.push((key, SavedWindow { tabs: vec![url] })),
        }
    }
    Session { windows: windows.into_iter().map(|(_, w)| w).collect() }
}

/// Incognito tabs use an off-the-record context that isn't the global one.
fn is_private(host: &BrowserHost) -> bool {
    let (Some(ctx), Some(mut global)) = (host.request_context(), request_context_get_global_context()) else {
        return false;
    };
    ctx.is_sharing_with(Some(&mut global)) == 0 && ctx.is_same(Some(&mut global)) == 0
}

/// Save right now (before quitting closes the windows one by one).
#[cfg(target_os = "macos")]
pub fn save_now() {
    let s = snapshot();
    if !s.windows.is_empty() {
        if let Ok(json) = serde_json::to_string(&s) {
            write_json(&json);
        }
    }
}

/// Save within a second, coalescing bursts. While windows close, the debounce lets all of a window's tabs go
/// first; an empty snapshot (WolfWeb quitting) is never written, so the last real session survives.
pub fn save_soon() {
    if SAVE_PENDING.with(|p| p.replace(true)) {
        return;
    }
    let mut task = SaveTask::new();
    post_delayed_task(ThreadId::UI, Some(&mut task), 1000);
}

wrap_task! {
    struct SaveTask;

    impl Task {
        fn execute(&self) {
            SAVE_PENDING.with(|p| p.set(false));
            let s = snapshot();
            if s.windows.is_empty() {
                return;
            }
            if let Ok(json) = serde_json::to_string(&s) {
                // File I/O off the UI thread.
                let mut task = WriteTask::new(json);
                post_task(ThreadId::FILE_BACKGROUND, Some(&mut task));
            }
        }
    }
}

wrap_task! {
    struct WriteTask {
        json: String,
    }

    impl Task {
        fn execute(&self) {
            write_json(&self.json);
        }
    }
}

/// Temp file + rename, so a crash mid-write never leaves a truncated session.
fn write_json(json: &str) {
    let Some(path) = crate::profile::session_file() else { return };
    let tmp = path.with_extension("json.tmp");
    if std::fs::write(&tmp, json).is_ok() {
        let _ = std::fs::rename(&tmp, &path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_drops_unrestorable() {
        let s = parse(r#"{"windows":[{"tabs":["https://a.com/","javascript:x","data:text/html,hi"]},{"tabs":["about:blank"]}]}"#).unwrap();
        assert_eq!(s.windows.len(), 1);
        assert_eq!(s.windows[0].tabs, vec!["https://a.com/".to_string()]);
    }

    #[test]
    fn old_new_tab_page_reopens_as_wolfweb_new_tab() {
        let s = parse(r#"{"windows":[{"tabs":["chrome://new-tab-page/","https://b.com/"]}]}"#).unwrap();
        assert_eq!(s.windows[0].tabs, vec!["chrome://newtab/".to_string(), "https://b.com/".to_string()]);
    }

    #[test]
    fn parse_rejects_garbage() {
        assert!(parse("not json").is_none());
    }
}
