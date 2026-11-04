//! CefApp + CefBrowserProcessHandler: scheme registration, startup, relaunch.

use cef::*;

/// Brand colour handed to Chrome's colour pipeline so its toolbar, menus and dialogs pick up WolfWeb's red.
/// Source: wolfmail/src/styles/index.css:24 `--iw-red: #c41e1e`. cef_color_t is ARGB (cef_types.h cef_color_t).
const BRAND_ARGB: u32 = 0xFF_C4_1E_1E;

pub fn new_app() -> App {
    WolfApp::new()
}

wrap_app! {
    struct WolfApp;

    impl App {
        fn on_before_command_line_processing(
            &self,
            process_type: Option<&CefString>,
            command_line: Option<&mut CommandLine>,
        ) {
            // Browser process only (empty process type — cef_app.h OnBeforeCommandLineProcessing).
            let is_browser = process_type.map(|p| p.to_string().is_empty()).unwrap_or(true);
            let Some(command_line) = command_line else { return };
            if !is_browser {
                return;
            }
            // The built-in WolfWeb extension (new-tab page, import, quick commands, notes, gestures).
            let load = CefString::from("load-extension");
            let existing = CefString::from(&command_line.switch_value(Some(&load))).to_string();
            match crate::extension::load_extension_value(&existing) {
                Some(value) => command_line.append_switch_with_value(Some(&load), Some(&CefString::from(value.as_str()))),
                None if crate::extension::dir().is_none() => {
                    eprintln!("wolfweb: the built-in extension folder is missing; WolfWeb's own pages won't be available")
                }
                None => {}
            }
            // Use native Wayland when the session has it, X11 otherwise (Chromium's --ozone-platform-hint=auto).
            // Respect an explicit --ozone-platform the user passed.
            #[cfg(target_os = "linux")]
            if command_line.has_switch(Some(&CefString::from("ozone-platform"))) == 0
                && command_line.has_switch(Some(&CefString::from("ozone-platform-hint"))) == 0
            {
                command_line.append_switch_with_value(
                    Some(&CefString::from("ozone-platform-hint")),
                    Some(&CefString::from("auto")),
                );
            }
            #[cfg(target_os = "linux")]
            disable_hardware_video_to_gl(command_line);
        }

        fn on_register_custom_schemes(&self, registrar: Option<&mut SchemeRegistrar>) {
            let Some(registrar) = registrar else { return };
            // Source: cef_types.h:3331-3382 cef_scheme_options_t. STANDARD (origin + relative URLs), SECURE (treated
            // like https), CORS_ENABLED + FETCH_ENABLED (the interface pages fetch() their own API).
            let options = SchemeOptions::STANDARD.get_raw()
                | SchemeOptions::SECURE.get_raw()
                | SchemeOptions::CORS_ENABLED.get_raw()
                | SchemeOptions::FETCH_ENABLED.get_raw();
            // get_raw() is u32 on Linux/macOS but i32 on Windows (MSVC enums are int) — the cast is a no-op there.
            #[allow(clippy::unnecessary_cast)]
            let options = options as i32;
            registrar.add_custom_scheme(Some(&CefString::from(crate::scheme::SCHEME)), options);
        }

        fn resource_bundle_handler(&self) -> Option<ResourceBundleHandler> {
            Some(crate::branding::handler())
        }

        fn browser_process_handler(&self) -> Option<BrowserProcessHandler> {
            Some(WolfBrowserProcessHandler::new())
        }
    }
}

/// Chromium's Linux feature for handing hardware-decoded video frames to OpenGL.
/// Source: chromium media/base/media_switches.cc:1506 BASE_FEATURE(kAcceleratedVideoDecodeLinuxGL, ENABLED_BY_DEFAULT).
#[cfg(target_os = "linux")]
const VIDEO_DECODE_TO_GL: &str = "AcceleratedVideoDecodeLinuxGL";

/// Videos decode on the processor instead of handing GPU-decoded frames to OpenGL. On a Radeon 890M (Mesa 26.2,
/// Wayland) every hardware-decoded frame drew black: importing it into ANGLE failed (eglCreateImage 0x3009
/// EGL_BAD_MATCH, then OzoneImageBacking::ProduceSkiaGanesh, ozone_image_backing.cc:320), with the stock CEF too.
/// With the feature off the picture shows (observed 2026-11-04). Someone who wants it back passes
/// --enable-features=AcceleratedVideoDecodeLinuxGL.
///
/// CEF has already written its own --disable-features list by the time this runs
/// (Source: cef/libcef/common/chrome/chrome_main_delegate_cef.cc:405-424), so this adds to it rather than replacing
/// it; CefCommandLine::AppendSwitchWithValue sets the switch's value, so the last one given is the one used.
#[cfg(target_os = "linux")]
fn disable_hardware_video_to_gl(command_line: &mut CommandLine) {
    let value_of = |name: &str| CefString::from(&command_line.switch_value(Some(&CefString::from(name)))).to_string();
    let listed = |list: &str| list.split(',').any(|f| f.trim() == VIDEO_DECODE_TO_GL);
    let disabled = value_of("disable-features");
    if listed(&value_of("enable-features")) || listed(&disabled) {
        return;
    }
    let value = if disabled.trim().is_empty() { VIDEO_DECODE_TO_GL.to_string() } else { format!("{disabled},{VIDEO_DECODE_TO_GL}") };
    command_line.append_switch_with_value(Some(&CefString::from("disable-features")), Some(&CefString::from(value.as_str())));
}

wrap_browser_process_handler! {
    struct WolfBrowserProcessHandler;

    impl BrowserProcessHandler {
        fn on_context_initialized(&self) {
            debug_assert_ne!(currently_on(ThreadId::UI), 0);

            crate::scheme::register_factory();

            // A context sharing the global profile (cef_request_context.h:126-132 CreateContext(other, handler));
            // its OnRequestContextInitialized runs once the profile can take preference changes.
            let shared = request_context_get_global_context().and_then(|mut global| {
                let mut handler = GlobalContextHandler::new();
                request_context_cef_create_context_shared(Some(&mut global), Some(&mut handler))
            });
            match shared {
                Some(ctx) => STARTUP.with(|s| *s.shared_context.borrow_mut() = Some(ctx)),
                None => {
                    // Without it the preferences stay as they are, but WolfWeb must still open.
                    eprintln!("wolfweb: couldn't reach the profile's settings; Chromium may also reopen old windows");
                    STARTUP.with(|s| s.context_ready.set(true));
                }
            }
            STARTUP.with(|s| s.app_ready.set(true));
            maybe_open_first_windows();
        }

        fn on_already_running_app_relaunch(
            &self,
            command_line: Option<&mut CommandLine>,
            _current_directory: Option<&CefString>,
        ) -> i32 {
            // Someone launched WolfWeb again: open what they asked for here (cef_browser_process_handler.h —
            // return true when handled so CEF doesn't open its default window).
            let urls = command_line.map(|cl| urls_from_command_line(cl)).unwrap_or_default();
            if urls.is_empty() {
                crate::tabs::open_window_with_tabs(Vec::new());
            } else {
                crate::tabs::open_in_new_tabs(urls);
            }
            1
        }

        fn default_client(&self) -> Option<Client> {
            // Tabs and windows Chromium creates from its own UI get WolfWeb's client too
            // (cef_browser_process_handler.h:141-150).
            Some(crate::views::WolfClient::new())
        }
    }
}

/// The profile's preferences can only be changed once its request context is initialized
/// (libcef/browser/request_context_impl.cc:454-463 SetPreference → VerifyBrowserContext; at OnContextInitialized it
/// still failed — observed), and the first window may only open after that, because Chromium's startup restore
/// keys off the first browser. Whichever of the two callbacks comes second opens the windows.
struct Startup {
    /// Kept alive until its initialization callback has run.
    shared_context: std::cell::RefCell<Option<RequestContext>>,
    /// The user's startup choice (startup.rs), until it's been put back after the first window.
    choice: std::cell::RefCell<Option<crate::startup::Choice>>,
    app_ready: std::cell::Cell<bool>,
    context_ready: std::cell::Cell<bool>,
    done: std::cell::Cell<bool>,
}

thread_local! {
    static STARTUP: Startup = const {
        Startup {
            shared_context: std::cell::RefCell::new(None),
            choice: std::cell::RefCell::new(None),
            app_ready: std::cell::Cell::new(false),
            context_ready: std::cell::Cell::new(false),
            done: std::cell::Cell::new(false),
        }
    };
}

fn maybe_open_first_windows() {
    let go = STARTUP.with(|s| s.app_ready.get() && s.context_ready.get() && !s.done.replace(true));
    if !go {
        return;
    }
    let urls = command_line_get_global().map(|cl| urls_from_command_line(&cl)).unwrap_or_default();
    let choice = STARTUP.with(|s| s.choice.borrow().clone()).unwrap_or_default();
    crate::session::restore_or_open(urls, &choice);
}

/// The first window exists (tabs.rs on_browser_created): Chromium has passed its own startup handling, so the
/// user's startup choice goes back into Settings (startup.rs). Does nothing after the first time.
pub fn after_first_browser() {
    let pending = STARTUP.with(|s| s.choice.borrow_mut().take().map(|c| (c, s.shared_context.borrow().clone())));
    if let Some((choice, Some(context))) = pending {
        crate::startup::give_back(&context, &choice);
    }
}

wrap_request_context_handler! {
    struct GlobalContextHandler;

    impl RequestContextHandler {
        fn on_request_context_initialized(&self, request_context: Option<&mut RequestContext>) {
            if let Some(context) = request_context {
                // cef_request_context.h:337-345 — a non-mode variant applies |user_color| in the current
                // (system) light/dark mode.
                context.set_chrome_color_scheme(ColorVariant::TONAL_SPOT, BRAND_ARGB);
                // WolfWeb opens the startup windows itself, following the user's "On startup" choice; Chromium's
                // own handling of it is held off for this start (startup.rs).
                let choice = crate::startup::take(context);
                STARTUP.with(|s| *s.choice.borrow_mut() = Some(choice));
                crate::defaults::apply_once(context);
            }
            STARTUP.with(|s| s.context_ready.set(true));
            maybe_open_first_windows();
        }
    }
}

/// Non-switch arguments are URLs or search text to open (e.g. `wolfweb https://example.com`).
fn urls_from_command_line(command_line: &CommandLine) -> Vec<String> {
    let mut list = CefStringList::new();
    command_line.arguments(Some(&mut list));
    list.into_iter()
        .map(|s| s.to_string())
        .filter(|s| !s.trim().is_empty())
        .map(|s| crate::util::url_from_input(&s))
        .collect()
}
