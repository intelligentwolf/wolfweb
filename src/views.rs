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
