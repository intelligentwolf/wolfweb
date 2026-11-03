//! Browser windows and tabs.
//!
//! WolfWeb windows are Chrome-style *native* CEF browser windows: Chromium supplies the tab strip, toolbar, omnibox,
//! extension buttons/popups, side panel and menus, so Chrome extensions see real windows and tabs (chrome.tabs,
//! chrome.windows) and work exactly as in Chrome. Why not a WolfWeb-drawn tab strip in a CEF Views window: CEF allows
//! one Chrome-style browser per Views window — libcef/browser/views/browser_view_impl.cc:317-327 AddedToWidget()
//! refuses a second ("Cannot add multiple Chrome style BrowserViews"), and detaching a Chrome-style BrowserView to get
//! around that made Chromium re-host it in a window of its own (tried and observed 2026-11-03).
//!
//! Every browser — the first one WolfWeb creates and each tab Chromium creates from its UI (Ctrl+T, links,
//! extensions) — uses `views::WolfClient` (CefBrowserProcessHandler::GetDefaultClient,
//! cef_browser_process_handler.h:141-150), so WolfWeb sees them all. UI thread only.

use cef::*;
use std::cell::RefCell;
use std::collections::VecDeque;

#[derive(Default)]
struct Model {
    /// Every live browser (one per tab), in creation order.
    browsers: Vec<Browser>,
    /// URLs to load into tabs WolfWeb asked Chromium to open (IDC_NEW_TAB), in order.
    awaiting: Vec<String>,
    /// Windows still to open (each: its tabs' URLs). Opened one at a time so every new browser can be matched to
    /// the window it belongs to.
    queue: VecDeque<Vec<String>>,
    /// The window being opened right now: the URLs of its other tabs, requested once its first browser exists.
    opening: Option<Vec<String>>,
    /// The tab that last had keyboard focus (CefFocusHandler::OnGotFocus) — where window commands go.
    focused: Option<i32>,
}

thread_local! {
    static MODEL: RefCell<Model> = RefCell::new(Model::default());
}

/// Source: cef/include/cef_command_ids.h:55 — IDC_NEW_TAB 34014.
const IDC_NEW_TAB: i32 = 34014;

/// Open a new browser window showing `url`.
pub fn open_window(url: &str) {
    // Chrome style (cef_types.h cef_window_info_t.runtime_style) gives the full Chrome window.
    let window_info = WindowInfo { runtime_style: RuntimeStyle::CHROME, ..Default::default() };
    let mut client = crate::views::WolfClient::new();
    browser_host_create_browser(
        Some(&window_info),
        Some(&mut client),
        Some(&CefString::from(url)),
        Some(&BrowserSettings::default()),
        None,
        None,
    );
}

/// Open a window with `urls[0]` and the rest as tabs beside it. Empty: Chromium's new-tab page.
pub fn open_window_with_tabs(urls: Vec<String>) {
    MODEL.with(|m| m.borrow_mut().queue.push_back(urls));
    pump();
}

/// Open the next queued window when nothing else is in flight.
fn pump() {
    let next = MODEL.with(|m| {
        let mut m = m.borrow_mut();
        if m.opening.is_some() || !m.awaiting.is_empty() {
            return None;
        }
        let mut urls = m.queue.pop_front()?.into_iter();
        let first = urls.next().unwrap_or_else(|| "chrome://newtab/".to_string());
        m.opening = Some(urls.collect());
        Some(first)
    });
    if let Some(first) = next {
        open_window(&first);
    }
}

/// Open `urls` as new tabs in the most recently used window (or a new window when none is open).
pub fn open_in_new_tabs(urls: Vec<String>) {
    if urls.is_empty() {
        return;
    }
    match last_browser() {
        Some(browser) => request_tabs(&browser, urls),
        None => open_window_with_tabs(urls),
    }
}

/// Ask Chromium for one new tab per URL in `browser`'s window; each new tab loads its URL in on_browser_created.
/// Posted, not inline: straight from OnAfterCreated the window can't take the command yet (observed — the extra
/// tabs of a restored window never opened), so the task waits until CanExecuteChromeCommand says yes.
fn request_tabs(browser: &Browser, urls: Vec<String>) {
    let n = urls.len() as i32;
    MODEL.with(|m| m.borrow_mut().awaiting.extend(urls));
    let mut task = NewTabsTask::new(browser.identifier(), n, 0);
    post_task(ThreadId::UI, Some(&mut task));
}

/// Give up after this many 100 ms retries (5 s).
const NEW_TAB_RETRIES: i32 = 50;

wrap_task! {
    struct NewTabsTask {
        browser_id: i32,
        count: i32,
        attempt: i32,
    }

    impl Task {
        fn execute(&self) {
            let host = MODEL.with(|m| {
                m.borrow().browsers.iter().find(|b| b.identifier() == self.browser_id).and_then(|b| b.host())
            });
            let Some(host) = host else {
                // The window went away: don't let a later Ctrl+T tab pick up these URLs.
                MODEL.with(|m| m.borrow_mut().awaiting.clear());
                return;
            };
            if host.can_execute_chrome_command(IDC_NEW_TAB) == 0 {
                if self.attempt < NEW_TAB_RETRIES {
                    let mut again = NewTabsTask::new(self.browser_id, self.count, self.attempt + 1);
                    post_delayed_task(ThreadId::UI, Some(&mut again), 100);
                } else {
                    eprintln!("wolfweb: the window never accepted new tabs; {} tab(s) not reopened", self.count);
                    MODEL.with(|m| m.borrow_mut().awaiting.clear());
                }
                return;
            }
            for _ in 0..self.count {
                host.execute_chrome_command(IDC_NEW_TAB, WindowOpenDisposition::CURRENT_TAB);
            }
        }
    }
}

fn last_browser() -> Option<Browser> {
    MODEL.with(|m| m.borrow().browsers.last().cloned())
}

pub fn browsers() -> Vec<Browser> {
    MODEL.with(|m| m.borrow().browsers.clone())
}

/// CefLifeSpanHandler::OnAfterCreated for every tab.
pub fn on_browser_created(browser: &Browser) {
    // No borrow may be held across the CEF calls below: they can create browsers synchronously and re-enter here.
    let (load_now, window_rest) = MODEL.with(|m| {
        let mut m = m.borrow_mut();
        m.browsers.push(browser.clone());
        if !m.awaiting.is_empty() {
            (Some(m.awaiting.remove(0)), None)
        } else {
            (None, m.opening.take())
        }
    });
    if let Some(url) = load_now {
        if let Some(frame) = browser.main_frame() {
            frame.load_url(Some(&CefString::from(url.as_str())));
        }
    }
    if let Some(rest) = window_rest {
        if !rest.is_empty() {
            request_tabs(browser, rest);
        }
    }
    crate::app::after_first_browser();
    pump();
}

/// CefLifeSpanHandler::OnBeforeClose for every tab.
pub fn on_browser_closed(browser: &Browser) {
    let id = browser.identifier();
    let remaining = MODEL.with(|m| {
        let mut m = m.borrow_mut();
        m.browsers.retain(|b| b.identifier() != id);
        m.browsers.len()
    });
    if remaining == 0 {
        // Source: cefsimple simple_handler on_before_close — quit once no browsers remain.
        quit_message_loop();
    } else {
        crate::session::save_soon();
    }
}


/// CefFocusHandler::OnGotFocus. WolfWeb's quick-commands popup doesn't count: commands chosen there act on the tab
/// that was focused before it opened.
pub fn on_browser_focused(browser: &Browser) {
    let url = browser.main_frame().map(|f| CefString::from(&f.url()).to_string()).unwrap_or_default();
    if url.starts_with(&format!("{}/palette.html", crate::extension::origin())) {
        return;
    }
    let id = browser.identifier();
    MODEL.with(|m| m.borrow_mut().focused = Some(id));
}

/// The focused tab, else the most recently opened one.
pub fn focused_browser() -> Option<Browser> {
    MODEL.with(|m| {
        let m = m.borrow();
        m.focused
            .and_then(|id| m.browsers.iter().find(|b| b.identifier() == id).cloned())
            .or_else(|| m.browsers.last().cloned())
    })
}

/// Close every tab (beforeunload handlers run); the last one closing quits WolfWeb (on_browser_closed).
/// Used by macOS's Quit, which must leave the run loop instead of exiting (mac.rs terminate:).
#[cfg(target_os = "macos")]
pub fn close_all_browsers() {
    crate::session::save_now();
    for b in browsers() {
        if let Some(host) = b.host() {
            host.close_browser(0);
        }
    }
}

/// The live browser with this identifier.
pub fn browser_by_id(id: i32) -> Option<Browser> {
    MODEL.with(|m| m.borrow().browsers.iter().find(|b| b.identifier() == id).cloned())
}

/// Open `url` in a new tab in the same window as browser `browser_id` (or the last window if it's gone).
pub fn open_tab_beside(browser_id: i32, url: &str) {
    match browser_by_id(browser_id) {
        Some(b) => request_tabs(&b, vec![url.to_string()]),
        None => open_in_new_tabs(vec![url.to_string()]),
    }
}
