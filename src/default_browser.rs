//! Is WolfWeb the default browser, and making it so. Chromium's own "isn't your default browser" prompt doesn't run
//! under CEF, so WolfWeb asks itself (new-tab page and WolfWeb settings) and does the work here, per platform:
//!
//! - Linux: the freedesktop default (`xdg-settings`), with WolfWeb's wolfweb.desktop (desktop.rs). On Omarchy the
//!   desktop-specific list routes links through omarchy-browser.desktop and the real browser is kept in
//!   mimeapps.list; Omarchy reads it with `omarchy-cmd-default-browser` and sets it with
//!   `env -u BROWSER XDG_CURRENT_DESKTOP= xdg-settings set default-web-browser <id>` (omarchy-default-browser) —
//!   WolfWeb does the same so Omarchy's launcher keeps working.
//! - macOS: Launch Services (LSCopyDefaultHandlerForURLScheme / LSSetDefaultHandlerForURLScheme; macOS asks the
//!   user to confirm the change). The app declares http/https in its Info.plist (xtask adds CFBundleURLTypes).
//! - Windows: register WolfWeb as a browser for this user the way Chromium does
//!   (chrome/installer/util/shell_util.cc GetShellIntegrationEntries / GetProgIdEntries; key names :1494-1515),
//!   then open Settings → Default apps on WolfWeb, where the user confirms (Windows lets only the user change it).

/// Whether WolfWeb is the default browser now.
pub fn is_default() -> Result<bool, String> {
    platform::is_default()
}

/// Make WolfWeb the default (or start the system's own confirmation). Returns what to tell the user.
pub fn make_default() -> Result<String, String> {
    platform::make_default()
}

#[cfg(target_os = "linux")]
mod platform {
    use std::process::Command;

    const DESKTOP_ID: &str = "wolfweb.desktop";

    fn on_omarchy() -> bool {
        crate::util::find_program("omarchy-cmd-default-browser").is_some()
    }

    pub fn is_default() -> Result<bool, String> {
        let out = if on_omarchy() {
            Command::new("omarchy-cmd-default-browser").output()
        } else {
            Command::new("xdg-settings").args(["get", "default-web-browser"]).output()
        }
        .map_err(|e| format!("Couldn't ask the desktop for its default browser: {e}"))?;
        Ok(String::from_utf8_lossy(&out.stdout).trim() == DESKTOP_ID)
    }

    pub fn make_default() -> Result<String, String> {
        let mut cmd = Command::new("xdg-settings");
        cmd.args(["set", "default-web-browser", DESKTOP_ID]);
        if on_omarchy() {
            // As omarchy-default-browser does: set the browser itself in mimeapps.list, leaving Omarchy's router.
            cmd.env_remove("BROWSER").env("XDG_CURRENT_DESKTOP", "");
        }
        let out = cmd.output().map_err(|e| format!("Couldn't run xdg-settings: {e}"))?;
        if !out.status.success() {
            return Err(format!(
                "The desktop didn't accept WolfWeb as the default browser: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
        if is_default()? {
            Ok("WolfWeb is now your default browser.".into())
        } else {
            Err("The desktop still reports another default browser.".into())
        }
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use std::ffi::{c_char, c_void, CStr, CString};

    const BUNDLE_ID: &str = "uk.co.intelligentwolf.wolfweb";
    /// Source: CoreFoundation CFString.h — kCFStringEncodingUTF8 = 0x08000100.
    const K_CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;

    type CFStringRef = *const c_void;

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFStringCreateWithCString(alloc: *const c_void, c_str: *const c_char, encoding: u32) -> CFStringRef;
        fn CFStringGetCString(s: CFStringRef, buffer: *mut c_char, size: isize, encoding: u32) -> u8;
        fn CFRelease(cf: *const c_void);
    }

    #[link(name = "CoreServices", kind = "framework")]
    extern "C" {
        fn LSCopyDefaultHandlerForURLScheme(scheme: CFStringRef) -> CFStringRef;
        fn LSSetDefaultHandlerForURLScheme(scheme: CFStringRef, handler: CFStringRef) -> i32;
    }

    /// An owned CFString, released on drop.
    struct CfString(CFStringRef);
    impl CfString {
        fn new(s: &str) -> Option<Self> {
            let c = CString::new(s).ok()?;
            // SAFETY: c is NUL-terminated; a null allocator means the default one.
            let r = unsafe { CFStringCreateWithCString(std::ptr::null(), c.as_ptr(), K_CF_STRING_ENCODING_UTF8) };
            (!r.is_null()).then_some(CfString(r))
        }
        fn to_string(&self) -> String {
            let mut buf = vec![0 as c_char; 512];
            // SAFETY: buf has buf.len() bytes; CFStringGetCString writes at most that, NUL-terminated.
            let ok = unsafe { CFStringGetCString(self.0, buf.as_mut_ptr(), buf.len() as isize, K_CF_STRING_ENCODING_UTF8) };
            if ok == 0 {
                return String::new();
            }
            // SAFETY: on success the buffer holds a NUL-terminated string.
            unsafe { CStr::from_ptr(buf.as_ptr()) }.to_string_lossy().into_owned()
        }
    }
    impl Drop for CfString {
        fn drop(&mut self) {
            // SAFETY: we own this reference (Create/Copy rule).
            unsafe { CFRelease(self.0) };
        }
    }

    pub fn is_default() -> Result<bool, String> {
        let scheme = CfString::new("https").ok_or("Couldn't make a CFString")?;
        // SAFETY: valid CFString; the result follows the Copy rule (we release it).
        let handler = unsafe { LSCopyDefaultHandlerForURLScheme(scheme.0) };
        if handler.is_null() {
            return Ok(false);
        }
        let handler = CfString(handler);
        Ok(handler.to_string().eq_ignore_ascii_case(BUNDLE_ID))
    }

    pub fn make_default() -> Result<String, String> {
        let bundle = CfString::new(BUNDLE_ID).ok_or("Couldn't make a CFString")?;
        for scheme in ["http", "https"] {
            let s = CfString::new(scheme).ok_or("Couldn't make a CFString")?;
            // SAFETY: both are valid CFStrings for the duration of the call.
            let status = unsafe { LSSetDefaultHandlerForURLScheme(s.0, bundle.0) };
            if status != 0 {
                return Err(format!("macOS didn't accept WolfWeb as the browser for {scheme} (error {status})."));
            }
        }
        Ok("macOS will ask you to confirm — choose “Use WolfWeb”.".into())
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use std::process::Command;

    /// WolfWeb's ProgId and registered-application name (Chromium uses "ChromeHTML" / its app name the same way).
    const PROG_ID: &str = "WolfWebHTML";
    const APP: &str = "WolfWeb";
    /// Source: shell_util.h:336-338 kPotentialFileAssociations.
    const FILE_TYPES: [&str; 8] = [".htm", ".html", ".mhtml", ".shtml", ".svg", ".xht", ".xhtml", ".webp"];

    fn reg(args: &[&str]) -> Result<String, String> {
        let out = Command::new("reg").args(args).output().map_err(|e| format!("Couldn't run reg: {e}"))?;
        if out.status.success() {
            Ok(String::from_utf8_lossy(&out.stdout).into_owned())
        } else {
            Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
        }
    }

    /// reg add <key> [/v name | /ve] /d data /f
    fn set(key: &str, name: Option<&str>, data: &str) -> Result<(), String> {
        let mut args = vec!["add", key];
        match name {
            Some(n) => args.extend(["/v", n]),
            None => args.push("/ve"),
        }
        args.extend(["/t", "REG_SZ", "/d", data, "/f"]);
        reg(&args).map(|_| ())
    }

    /// Register WolfWeb as a browser for this user — the entries Chromium writes (GetShellIntegrationEntries,
    /// GetProgIdEntries), under HKCU so no administrator rights are needed.
    fn register() -> Result<(), String> {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let exe = exe.to_string_lossy();
        let open = format!("\"{exe}\" \"%1\"");
        let icon = format!("{exe},0");
        let classes = format!(r"HKCU\Software\Classes\{PROG_ID}");
        set(&classes, None, "WolfWeb HTML Document")?;
        set(&format!(r"{classes}\DefaultIcon"), None, &icon)?;
        set(&format!(r"{classes}\shell\open\command"), None, &open)?;
        let client = format!(r"HKCU\Software\Clients\StartMenuInternet\{APP}");
        set(&client, None, APP)?;
        set(&format!(r"{client}\DefaultIcon"), None, &icon)?;
        set(&format!(r"{client}\shell\open\command"), None, &format!("\"{exe}\""))?;
        let caps = format!(r"{client}\Capabilities");
        set(&caps, Some("ApplicationName"), APP)?;
        set(&caps, Some("ApplicationDescription"), "A fast, private web browser that runs Chrome extensions")?;
        set(&caps, Some("ApplicationIcon"), &icon)?;
        set(&format!(r"{caps}\Startmenu"), Some("StartMenuInternet"), APP)?;
        for t in FILE_TYPES {
            set(&format!(r"{caps}\FileAssociations"), Some(t), PROG_ID)?;
        }
        for p in ["http", "https"] {
            set(&format!(r"{caps}\URLAssociations"), Some(p), PROG_ID)?;
        }
        set(r"HKCU\Software\RegisteredApplications", Some(APP), &format!(r"Software\Clients\StartMenuInternet\{APP}\Capabilities"))
    }

    pub fn is_default() -> Result<bool, String> {
        // Source: shell_util.cc:1279-1285 GetProtocolUserChoiceEntry — ...\UrlAssociations\<protocol>\UserChoice ProgId.
        match reg(&["query", r"HKCU\Software\Microsoft\Windows\Shell\Associations\UrlAssociations\https\UserChoice", "/v", "ProgId"]) {
            Ok(out) => Ok(out.contains(PROG_ID)),
            Err(_) => Ok(false), // no choice recorded yet
        }
    }

    pub fn make_default() -> Result<String, String> {
        register()?;
        // Windows only lets the user change defaults: open Default apps on WolfWeb's page.
        Command::new("cmd")
            .args(["/C", "start", "", "ms-settings:defaultapps?registeredAppUser=WolfWeb"])
            .spawn()
            .map_err(|e| format!("Couldn't open Settings: {e}"))?;
        Ok("Windows Settings is open on WolfWeb — choose “Set default”.".into())
    }
}
