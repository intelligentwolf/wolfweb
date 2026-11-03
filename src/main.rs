//! The WolfWeb executable on Linux and macOS (and Windows when built without the sandbox feature).
//! Source: cef-rs examples/cefsimple/src/main.rs + shared/mod.rs load_cef().

#![cfg_attr(all(not(debug_assertions), target_os = "windows"), windows_subsystem = "windows")]

use std::process::ExitCode;

#[cfg(not(all(target_os = "windows", feature = "sandbox")))]
fn main() -> ExitCode {
    // macOS: the CEF framework is loaded at run time from the app bundle (cefsimple load_cef()).
    #[cfg(target_os = "macos")]
    let _loader = {
        let exe = std::env::current_exe().expect("the executable's path");
        let loader = cef::library_loader::LibraryLoader::new(&exe, false);
        assert!(loader.load(), "couldn't load the Chromium Embedded Framework from the app bundle");
        loader
    };

    // The API version must be set before any other CEF call (cefsimple load_cef()).
    let _ = cef::api_hash(cef::sys::CEF_API_VERSION_LAST, 0);

    // macOS: NSApp must be WolfWeb's CEF-aware NSApplication before anything touches it (cefsimple load_cef()).
    #[cfg(target_os = "macos")]
    wolfweb::mac::setup_application();

    let args = cef::args::Args::new();
    ExitCode::from(wolfweb::run(&args, std::ptr::null_mut()).clamp(0, 255) as u8)
}

/// With the Windows sandbox, wolfweb.exe is CEF's bootstrap.exe and WolfWeb runs from wolfweb.dll
/// (src/win.rs RunWinMain) — this executable isn't part of that bundle.
/// Source: cefsimple src/main.rs (same message and behaviour).
#[cfg(all(target_os = "windows", feature = "sandbox"))]
fn main() -> ExitCode {
    eprintln!("With the sandbox, WolfWeb starts from the bundle's wolfweb.exe (cargo run -p xtask -- bundle).");
    ExitCode::FAILURE
}
