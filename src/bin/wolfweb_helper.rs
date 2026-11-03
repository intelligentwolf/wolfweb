//! macOS helper processes (WolfWeb Helper, Helper (Renderer), (GPU), (Plugin), (Alerts) — one executable in five
//! app bundles). On Linux and Windows the main executable is its own helper, so this binary is only bundled on
//! macOS. Source: cef-rs examples/cefsimple/src/bin/cefsimple_helper.rs, with WolfWeb's App so helpers register the
//! wolfweb:// scheme (CefApp::OnRegisterCustomSchemes runs in every process).

fn main() {
    let args = cef::args::Args::new();

    #[cfg(all(target_os = "macos", feature = "sandbox"))]
    let _sandbox = {
        let mut sandbox = cef::sandbox::Sandbox::new();
        sandbox.initialize(args.as_main_args());
        sandbox
    };

    #[cfg(target_os = "macos")]
    let _loader = {
        let exe = std::env::current_exe().expect("the executable's path");
        let loader = cef::library_loader::LibraryLoader::new(&exe, true);
        assert!(loader.load(), "couldn't load the Chromium Embedded Framework from the app bundle");
        loader
    };

    let _ = cef::api_hash(cef::sys::CEF_API_VERSION_LAST, 0);

    let mut app = wolfweb::new_app();
    let code = cef::execute_process(Some(args.as_main_args()), Some(&mut app), std::ptr::null_mut());
    std::process::exit(code.max(0));
}
