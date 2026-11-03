//! Windows with the sandbox: CEF's bootstrap.exe (shipped as wolfweb.exe) loads wolfweb.dll and calls RunWinMain.
//! Source: cef-rs examples/cefsimple/src/win.rs.

use cef::*;

/// # Safety
/// Called once by CEF's bootstrap.exe with its process instance and sandbox information.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn RunWinMain(
    instance: sys::HINSTANCE,
    _command_line: *const u8,
    _command_show: i32,
    sandbox_info: *mut u8,
) -> i32 {
    // The API version must be set before any other CEF call (cefsimple load_cef()).
    let _ = api_hash(sys::CEF_API_VERSION_LAST, 0);
    let main_args = MainArgs { instance };
    let args = args::Args::from(main_args);
    crate::run(&args, sandbox_info)
}
