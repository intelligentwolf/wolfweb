//! The CEF client every WolfWeb tab uses. Handlers run on the browser-process UI thread.

use cef::*;

wrap_client! {
    pub struct WolfClient;

    impl Client {
        fn life_span_handler(&self) -> Option<LifeSpanHandler> {
            Some(WolfLifeSpan::new())
        }

        fn display_handler(&self) -> Option<DisplayHandler> {
            Some(WolfDisplay::new())
        }

        fn focus_handler(&self) -> Option<FocusHandler> {
            Some(WolfFocus::new())
        }

        fn request_handler(&self) -> Option<RequestHandler> {
            Some(WolfRequests::new())
        }
    }
}

wrap_request_handler! {
    struct WolfRequests;

    impl RequestHandler {
        fn on_before_browse(
            &self,
            browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            request: Option<&mut Request>,
            _user_gesture: i32,
            _is_redirect: i32,
        ) -> i32 {
            // WolfWeb's menu bar items (menu.rs): run them instead of navigating.
            let (Some(browser), Some(frame), Some(request)) = (browser, frame, request) else { return 0 };
            if frame.is_main() == 0 {
                return 0;
            }
            let url = CefString::from(&request.url()).to_string();
            match crate::menu::parse(&url) {
                Some(action) => {
                    crate::menu::run(browser, action);
                    1
                }
                None => 0,
            }
        }
    }
}

wrap_focus_handler! {
    struct WolfFocus;

    impl FocusHandler {
        fn on_got_focus(&self, browser: Option<&mut Browser>) {
            if let Some(b) = browser {
                crate::tabs::on_browser_focused(b);
            }
        }
    }
}

wrap_life_span_handler! {
    struct WolfLifeSpan;

    impl LifeSpanHandler {
        fn on_after_created(&self, browser: Option<&mut Browser>) {
            if let Some(b) = browser {
                crate::tabs::on_browser_created(b);
            }
        }

        fn on_before_close(&self, browser: Option<&mut Browser>) {
            if let Some(b) = browser {
                crate::tabs::on_browser_closed(b);
            }
        }
    }
}

wrap_display_handler! {
    struct WolfDisplay;

    impl DisplayHandler {
        fn on_address_change(&self, _browser: Option<&mut Browser>, frame: Option<&mut Frame>, _url: Option<&CefString>) {
            if frame.map(|f| f.is_main() != 0).unwrap_or(false) {
                crate::session::save_soon();
            }
        }
    }
}
