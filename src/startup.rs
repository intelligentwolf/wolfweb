//! What WolfWeb opens when it starts: the user's "On startup" choice in Settings (chrome://settings/onStartup).
//! Until the user picks something there, that's WolfWeb's home page (the new-tab page); "Continue where you left
//! off" brings back the last session's tabs (session.rs) and "Open a specific page or set of pages" opens those.
//!
//! WolfWeb keeps the user's choice itself (a small file in the profile folder), written only when the user changes
//! the setting — watched with a preference observer (include/cef_preference.h:95-107, :208-216). Chromium's stored
//! value can't tell a choice from its default: its default is "continue where you left off" whenever its
//! kSetDefaultToContinueSession feature is on (chrome/browser/prefs/session_startup_pref.cc:65-76), and
//! GetAllPreferences(include_defaults = false) leaves out any value equal to the default (observed).
//!
//! WolfWeb opens the windows itself, so Chromium must not act on the same setting: it does so when the first
//! browser is created, and with "continue where you left off" it reopened every window ever left open, next to
//! WolfWeb's (observed 2026-11-03). So at each start WolfWeb sets Chromium's copy to "new tab page" (which opens
//! nothing extra), opens the windows, then puts the user's choice back so Settings shows it, and starts watching.
//!
//! Source: chromium chrome/browser/prefs/session_startup_pref.h:52-55 (kPrefValueLast = 1, kPrefValueURLs = 4,
//! kPrefValueNewTab = 5, kPrefValueLastAndURLs = 6); chrome/common/pref_names.h:180 kRestoreOnStartup
//! "session.restore_on_startup", :188 kURLsToRestoreOnStartup "session.startup_urls".

use cef::*;
use std::path::PathBuf;

const RESTORE_PREF: &str = "session.restore_on_startup";
const URLS_PREF: &str = "session.startup_urls";
const LAST: i32 = 1;
const URLS: i32 = 4;
const NEW_TAB: i32 = 5;
const LAST_AND_URLS: i32 = 6;

/// The user's startup choice, as WolfWeb acts on it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Choice {
    /// session.restore_on_startup's value (NEW_TAB when unset or unknown).
    kind: i32,
    /// session.startup_urls, for "specific pages".
    urls: Vec<String>,
}

impl Choice {
    /// Reopen the last session's windows?
    pub fn restores_session(&self) -> bool {
        self.kind == LAST || self.kind == LAST_AND_URLS
    }

    /// The pages to open (besides any restored session): the user's startup pages for "specific pages", else none
    /// (a window opens on the home page).
    pub fn pages(&self) -> Vec<String> {
        if self.kind == URLS || self.kind == LAST_AND_URLS {
            self.urls.clone()
        } else {
            Vec::new()
        }
    }
}

/// The user's choice, as WolfWeb keeps it.
fn choice_file() -> Option<PathBuf> {
    crate::profile::root_dir().ok().map(|d| d.join("wolfweb-startup-choice"))
}

fn known(kind: i32) -> i32 {
    match kind {
        LAST | URLS | NEW_TAB | LAST_AND_URLS => kind,
        _ => NEW_TAB,
    }
}

fn current(context: &RequestContext) -> i32 {
    context.preference(Some(&CefString::from(RESTORE_PREF))).map(|v| v.int()).unwrap_or(NEW_TAB)
}

fn set_current(context: &RequestContext, kind: i32) {
    if let Some(value) = value_create() {
        value.set_int(kind);
        crate::defaults::set_preference(context, RESTORE_PREF, value);
    }
}

/// Read the user's choice and keep Chromium's own startup handling out of this start. Call once, when the
/// profile's context is initialized and before any window opens.
pub fn take(context: &RequestContext) -> Choice {
    let saved = choice_file().and_then(|f| std::fs::read_to_string(f).ok()).and_then(|t| t.trim().parse::<i32>().ok());
    let kind = known(saved.unwrap_or(NEW_TAB));

    let mut urls = Vec::new();
    if let Some(list) = context.preference(Some(&CefString::from(URLS_PREF))).and_then(|v| v.list()) {
        for i in 0..list.size() {
            let url = CefString::from(&list.string(i)).to_string();
            if !url.trim().is_empty() {
                urls.push(url);
            }
        }
    }

    // Chromium acts on whatever the setting says when the first window opens; hold it at "new tab page" for now.
    if current(context) != NEW_TAB {
        set_current(context, NEW_TAB);
    }
    Choice { kind, urls }
}

thread_local! {
    /// Keeps the observer registered (include/cef_preference.h:208-216: it's removed when this is released).
    static WATCH: std::cell::RefCell<Option<Registration>> = const { std::cell::RefCell::new(None) };
}

wrap_preference_observer! {
    struct ChoiceWatcher {
        context: RequestContext,
    }

    impl PreferenceObserver {
        // The user changed "On startup" in Settings: remember it for the next start.
        fn on_preference_changed(&self, _name: Option<&CefString>) {
            let kind = known(current(&self.context));
            let written = choice_file().map(|f| std::fs::write(f, kind.to_string()));
            if !matches!(written, Some(Ok(()))) {
                eprintln!("wolfweb: couldn't save the startup choice");
            }
        }
    }
}

/// Once the first window exists: show the user's choice in Settings again, then watch for changes to it.
pub fn give_back(context: &RequestContext, choice: &Choice) {
    if choice.kind != NEW_TAB {
        set_current(context, choice.kind);
    }
    let mut observer = ChoiceWatcher::new(context.clone());
    let registration = context.add_preference_observer(Some(&CefString::from(RESTORE_PREF)), Some(&mut observer));
    if registration.is_none() {
        eprintln!("wolfweb: couldn't watch the startup setting; a change to it in Settings won't be remembered this time");
    }
    WATCH.with(|w| *w.borrow_mut() = registration);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn choice(kind: i32) -> Choice {
        Choice { kind, urls: vec!["https://example.com/".into()] }
    }

    #[test]
    fn new_tab_page_opens_the_home_page_only() {
        assert!(!choice(NEW_TAB).restores_session());
        assert!(choice(NEW_TAB).pages().is_empty());
    }

    #[test]
    fn continue_where_left_off_restores_only() {
        assert!(choice(LAST).restores_session());
        assert!(choice(LAST).pages().is_empty());
    }

    #[test]
    fn specific_pages_open_those_pages() {
        assert!(!choice(URLS).restores_session());
        assert_eq!(choice(URLS).pages(), vec!["https://example.com/".to_string()]);
        assert!(choice(LAST_AND_URLS).restores_session());
        assert_eq!(choice(LAST_AND_URLS).pages().len(), 1);
    }

    #[test]
    fn unknown_values_mean_the_new_tab_page() {
        assert_eq!(known(0), NEW_TAB);
        assert_eq!(known(42), NEW_TAB);
        assert_eq!(known(LAST), LAST);
    }
}
