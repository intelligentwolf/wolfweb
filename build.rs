fn main() {
    let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    // Linux: find libcef.so next to the executable (the bundle puts every CEF file beside it). macOS loads the
    // framework at run time (LibraryLoader) and Windows uses the DLL search path, so this is Linux-only.
    if os == "linux" {
        println!("cargo:rustc-link-arg-bins=-Wl,-rpath,$ORIGIN");
        println!("cargo:rustc-link-arg-cdylib=-Wl,-rpath,$ORIGIN");
    }
    // Windows: the application icon (as cef-rs examples/cefsimple/build.rs does with winres).
    #[cfg(target_os = "windows")]
    if os == "windows" {
        let mut res = winres::WindowsResource::new();
        res.set_icon("resources/win/wolfweb.ico");
        res.set("ProductName", "WolfWeb");
        res.set("FileDescription", "WolfWeb");
        res.set("CompanyName", "IntelligentWolf Ltd");
        if let Err(e) = res.compile() {
            panic!("couldn't embed the Windows icon: {e}");
        }
    }
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=resources/win/wolfweb.ico");
}
