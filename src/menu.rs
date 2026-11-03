//! WolfWeb's menu bar: folders on Chromium's bookmarks bar (File, Edit, View, …) whose items are addresses like
//! `chrome-extension://<WolfWeb>/do.html#cmd=new-window`. Clicking one would navigate the current tab, so WolfWeb
//! catches the navigation first (CefRequestHandler::OnBeforeBrowse — return true to cancel, cef_request_handler.h),
//! runs the Chromium command or opens the page, and the page you were on stays as it was.
//! The extension makes the folders (extension/menubar.js); actions::chrome_command is the list of commands.

use cef::*;

#[derive(Debug, PartialEq, Clone)]
pub enum Action {
    /// A Chromium command by name (actions::chrome_command).
    Command(String),
    /// Open this address in a new tab beside the current one.
    Open(String),
}

/// Read a menu address. Only WolfWeb's own do.html; `open=` only to chrome://, https:// or WolfWeb's own pages.
pub fn parse(url: &str) -> Option<Action> {
    let prefix = format!("{}/do.html#", crate::extension::origin());
    let fragment = url.strip_prefix(&prefix)?;
    let (key, value) = fragment.split_once('=')?;
    let value = url::form_urlencoded::parse(format!("v={value}").as_bytes()).next()?.1.into_owned();
    match key {
        "cmd" if crate::actions::chrome_command(&value).is_some() => Some(Action::Command(value)),
        "open" if value.starts_with("chrome://") || value.starts_with("https://") => Some(Action::Open(value)),
        "page" if !value.is_empty() && value.chars().all(|c| c.is_ascii_alphanumeric() || "._-#".contains(c)) && !value.contains("..") => {
            Some(Action::Open(format!("{}/{value}", crate::extension::origin())))
        }
        _ => None,
    }
}

/// Run a menu action from `browser`'s tab — posted, not inline: OnBeforeBrowse is mid-navigation, and commands
/// like "new window" create browsers.
pub fn run(browser: &Browser, action: Action) {
    let mut task = MenuTask::new(browser.identifier(), action);
    post_task(ThreadId::UI, Some(&mut task));
}

wrap_task! {
    struct MenuTask {
        browser_id: i32,
        action: Action,
    }

    impl Task {
        fn execute(&self) {
            if let Some(browser) = crate::tabs::browser_by_id(self.browser_id) {
                perform(&browser, self.action.clone());
            }
        }
    }
}

/// A tab that was opened only for the menu address (middle-click) closes afterwards.
fn perform(browser: &Browser, action: Action) {
    let Some(host) = browser.host() else { return };
    match action {
        Action::Command(name) => {
            if let Err(e) = crate::actions::run_chrome_command(&host, &name) {
                eprintln!("wolfweb: menu command {name}: {e}");
            }
        }
        Action::Open(url) => crate::tabs::open_tab_beside(browser.identifier(), &url),
    }
    let current = browser.main_frame().map(|f| CefString::from(&f.url()).to_string()).unwrap_or_default();
    if current.is_empty() || current == "about:blank" {
        host.close_browser(0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn menu(fragment: &str) -> String {
        format!("{}/do.html#{fragment}", crate::extension::origin())
    }

    #[test]
    fn only_wolfweb_menu_addresses_do_anything() {
        assert_eq!(parse(&menu("cmd=new-window")), Some(Action::Command("new-window".into())));
        assert_eq!(parse(&menu("cmd=rm-rf")), None);
        assert_eq!(parse(&menu("open=chrome%3A%2F%2Fhistory")), Some(Action::Open("chrome://history".into())));
        assert_eq!(parse(&menu("open=file%3A%2F%2F%2Fetc%2Fpasswd")), None);
        assert_eq!(parse(&menu("page=daily.html")), Some(Action::Open(format!("{}/daily.html", crate::extension::origin()))));
        assert_eq!(parse(&menu("page=..%2F..%2Fsecret")), None);
        assert_eq!(parse("https://example.com/do.html#cmd=new-window"), None);
        assert_eq!(parse("chrome-extension://aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/do.html#cmd=new-window"), None);
    }
}
