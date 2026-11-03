//! `cargo run -p xtask -- bundle [--release]` — build WolfWeb and assemble a runnable bundle in dist/:
//! - Linux:   dist/wolfweb-linux-<arch>/   (wolfweb + the CEF runtime beside it + extension/)
//! - Windows: dist/wolfweb-windows-<arch>/ (wolfweb.exe = CEF's bootstrap.exe, wolfweb.dll, CEF runtime, extension\)
//! - macOS:   dist/WolfWeb.app             (helper apps, framework, Info.plists — via cef-rs's bundler)
//!
//! The CEF runtime comes from $CEF_PATH (scripts/fetch-cef.sh puts it in third_party/cef; .cargo/config.toml sets
//! CEF_PATH to that).

use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("bundle") => {
            let release = args.iter().any(|a| a == "--release");
            let out = bundle(release)?;
            println!("Run WolfWeb: {}", out.display());
            Ok(())
        }
        _ => bail!("usage: cargo run -p xtask -- bundle [--release]"),
    }
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("xtask lives inside the WolfWeb repo").to_path_buf()
}

fn cef_dir(root: &Path) -> Result<PathBuf> {
    let dir = std::env::var_os("CEF_PATH").map(PathBuf::from).unwrap_or_else(|| root.join("third_party/cef"));
    let dir = if dir.is_absolute() { dir } else { root.join(dir) };
    if !dir.join("archive.json").is_file() {
        bail!("no CEF runtime in {} — run scripts/fetch-cef.sh first", dir.display());
    }
    Ok(dir)
}

fn cargo_build(root: &Path, release: bool, what: &[&str]) -> Result<()> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut cmd = Command::new(cargo);
    cmd.current_dir(root).args(["build", "-p", "wolfweb"]).args(what);
    if release {
        cmd.arg("--release");
    }
    let status = cmd.status().context("couldn't run cargo")?;
    if !status.success() {
        bail!("cargo build failed");
    }
    Ok(())
}

fn target_dir(root: &Path, release: bool) -> PathBuf {
    let base = std::env::var_os("CARGO_TARGET_DIR").map(PathBuf::from).unwrap_or_else(|| root.join("target"));
    base.join(if release { "release" } else { "debug" })
}

/// Copy a directory tree (files and folders).
fn copy_tree(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let to = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree(&entry.path(), &to)?;
        } else {
            std::fs::copy(entry.path(), &to).with_context(|| format!("copying {}", entry.path().display()))?;
        }
    }
    Ok(())
}

/// The CEF runtime files (not its headers, CMake files or wrapper sources).
fn copy_cef_runtime(cef: &Path, out: &Path, skip_exe: bool) -> Result<()> {
    for entry in std::fs::read_dir(cef)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if matches!(name.as_str(), "include" | "cmake" | "libcef_dll" | "CMakeLists.txt" | "archive.json" | "README.txt")
            || name.ends_with(".md")
            || (skip_exe && name.ends_with(".exe"))
        {
            continue;
        }
        let to = out.join(&name);
        if entry.file_type()?.is_dir() {
            copy_tree(&entry.path(), &to)?;
        } else {
            std::fs::copy(entry.path(), &to)?;
        }
    }
    Ok(())
}

fn fresh(dir: &Path) -> Result<()> {
    if dir.exists() {
        std::fs::remove_dir_all(dir)?;
    }
    std::fs::create_dir_all(dir)?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn bundle(release: bool) -> Result<PathBuf> {
    let root = root();
    let cef = cef_dir(&root)?;
    cargo_build(&root, release, &["--bin", "wolfweb"])?;
    let out = root.join(format!("dist/wolfweb-linux-{}", std::env::consts::ARCH));
    fresh(&out)?;
    copy_cef_runtime(&cef, &out, false)?;
    std::fs::copy(target_dir(&root, release).join("wolfweb"), out.join("wolfweb"))?;
    copy_tree(&root.join("extension"), &out.join("extension"))?;
    std::fs::copy(root.join("resources/brand/wolf-256.png"), out.join("wolfweb.png"))?;
    Ok(out.join("wolfweb"))
}

/// Windows with the sandbox (cef-rs cef/src/build_util/win/mod.rs copy_app, sandbox branch): CEF's bootstrap.exe
/// is shipped as wolfweb.exe and loads wolfweb.dll (src/win.rs RunWinMain); the manifest sits beside it.
#[cfg(target_os = "windows")]
fn bundle(release: bool) -> Result<PathBuf> {
    let root = root();
    let cef = cef_dir(&root)?;
    cargo_build(&root, release, &["--lib"])?;
    let out = root.join(format!("dist/wolfweb-windows-{}", std::env::consts::ARCH));
    fresh(&out)?;
    copy_cef_runtime(&cef, &out, true)?;
    std::fs::copy(cef.join("bootstrap.exe"), out.join("wolfweb.exe")).context("CEF's bootstrap.exe")?;
    std::fs::copy(root.join("resources/win/wolfweb.exe.manifest"), out.join("wolfweb.exe.manifest"))?;
    std::fs::copy(target_dir(&root, release).join("wolfweb.dll"), out.join("wolfweb.dll"))?;
    copy_tree(&root.join("extension"), &out.join("extension"))?;
    Ok(out.join("wolfweb.exe"))
}

/// macOS: cef-rs's bundler (cef/src/build_util/mac.rs bundle) makes wolfweb.app with the framework, the five
/// "wolfweb Helper (…)" apps and the Info.plists, and compiles resources/mac (MainMenu.xib → .nib, the icon).
/// The built-in extension goes in Contents/Resources (src/extension.rs dir() looks there).
#[cfg(target_os = "macos")]
fn bundle(release: bool) -> Result<PathBuf> {
    let root = root();
    cef_dir(&root)?;
    cargo_build(&root, release, &["--bin", "wolfweb", "--bin", "wolfweb_helper"])?;
    let out = root.join("dist");
    std::fs::create_dir_all(&out)?;
    let app = out.join("wolfweb.app");
    if app.exists() {
        std::fs::remove_dir_all(&app)?;
    }
    let version = semver::Version::parse(env!("CARGO_PKG_VERSION"))?;
    let info = cef::build_util::mac::BundleInfo {
        name: "WolfWeb".into(),
        identifier: "uk.co.intelligentwolf.wolfweb".into(),
        display_name: "WolfWeb".into(),
        development_region: "English".into(),
        version,
    };
    let app = cef::build_util::mac::bundle(
        &out,
        &target_dir(&root, release),
        "wolfweb",
        "wolfweb_helper",
        Some(root.join("resources")),
        info,
    )
    .map_err(|e| anyhow::anyhow!("{e:?}"))?;
    copy_tree(&root.join("extension"), &app.join("Contents/Resources/extension"))?;
    Ok(app)
}
