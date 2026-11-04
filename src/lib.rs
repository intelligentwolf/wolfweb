//! WolfWeb — a Chromium (CEF) browser in WolfMail's colours that runs Chrome extensions.
//!
//! Process model: one executable is the browser process and, relaunched by Chromium with `--type=…`, every helper
//! process (renderer, GPU, utility, zygote) — except on macOS, where helpers are the separate `wolfweb_helper`
//! (src/bin/wolfweb_helper.rs). `execute_process` returns ≥ 0 in a helper and −1 in the browser process.
//! Source: cef-rs examples/cefsimple/src/shared/mod.rs run_main().

mod actions;
mod app;
mod chrome_theme;
mod branding;
mod default_browser;
mod defaults;
#[cfg(target_os = "linux")]
mod desktop;
mod extension;
mod import;
mod languages;
mod menu;
#[cfg(target_os = "macos")]
pub mod mac;
mod profile;
mod scheme;
mod session;
mod startup;
mod tabs;
mod util;
mod views;
#[cfg(all(target_os = "windows", feature = "sandbox"))]
mod win;

use cef::*;

/// The CefApp every WolfWeb process needs (helpers register the wolfweb:// scheme too —
/// CefApp::OnRegisterCustomSchemes runs in all processes, cef_app.h).
pub fn new_app() -> App {
    app::new_app()
}

/// Run WolfWeb in this process; returns the process exit code. `sandbox_info` is CEF's sandbox handle on Windows
/// (from bootstrap.exe), null elsewhere.
pub fn run(args: &cef::args::Args, sandbox_info: *mut u8) -> i32 {
    // Before Chromium (or anything else) starts a thread.
    #[cfg(target_os = "linux")]
    desktop::set_desktop_name();

    let Some(cmd_line) = args.as_cmd_line() else {
        eprintln!("wolfweb: could not read the command line");
        return 1;
    };
    let is_browser_process = cmd_line.has_switch(Some(&CefString::from("type"))) != 1;

    let mut wolf_app = new_app();
    let ret = execute_process(Some(args.as_main_args()), Some(&mut wolf_app), sandbox_info);
    if !is_browser_process {
        return ret.max(0);
    }

    let profile_dir = match profile::root_dir() {
        Ok(dir) => dir,
        Err(e) => {
            eprintln!("wolfweb: cannot create the profile folder: {e}");
            return 1;
        }
    };
    extension::repair_profile(&profile_dir);
    import::apply_pending_history(&profile_dir);
    #[cfg(target_os = "linux")]
    desktop::install_desktop_entry();
    let profile_str = profile_dir.to_string_lossy().to_string();

    // Source: cef_types.h:283-324 — root_cache_path is the install/profile root (a process-singleton lock is taken on
    // it); cache_path "must be … equal to or a child directory of" root_cache_path, empty = incognito. Equal = the
    // persistent "Default" profile, which is where extensions, history, bookmarks and passwords live.
    let settings = Settings {
        no_sandbox: (!cfg!(feature = "sandbox")) as i32,
        root_cache_path: CefString::from(profile_str.as_str()),
        cache_path: CefString::from(profile_str.as_str()),
        persist_session_cookies: 1,
        ..Default::default()
    };

    if initialize(Some(args.as_main_args()), Some(&settings), Some(&mut wolf_app), sandbox_info) != 1 {
        // A second launch with the same root_cache_path fails here after the running instance receives
        // OnAlreadyRunningAppRelaunch (cef_types.h:311-318) — that instance opens the window.
        eprintln!("wolfweb: handed over to the WolfWeb that is already running (or Chromium failed to start)");
        return 0;
    }

    // Source: cefsimple shared/mod.rs run_main() — the macOS app delegate is set up after initialize().
    #[cfg(target_os = "macos")]
    let _delegate = mac::setup_app_delegate();

    run_message_loop();
    shutdown();
    0
}
