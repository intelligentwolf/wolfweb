//! Linux: make the desktop show WolfWeb (name and wolf icon) for WolfWeb's windows, not Chromium's.
//!
//! Chromium names its windows after a .desktop file: chrome/common/channel_info_posix.cc:143-152 GetDesktopName()
//! returns $CHROME_DESKTOP, else "chromium-browser.desktop", and the window class / Wayland app id comes from it
//! (chrome/browser/shell_integration_linux.cc GetProgramClassName/GetProgramClassClass). So WolfWeb sets
//! CHROME_DESKTOP=wolfweb.desktop and installs that file (and its icon) for the user, following the freedesktop
//! layout ($XDG_DATA_HOME/applications, $XDG_DATA_HOME/icons/hicolor/<size>/apps).

const DESKTOP_NAME: &str = "wolfweb.desktop";

const ICONS: [(u32, &[u8]); 6] = [
    (16, include_bytes!("../resources/brand/wolf-16.png")),
    (24, include_bytes!("../resources/brand/wolf-24.png")),
    (32, include_bytes!("../resources/brand/wolf-32.png")),
    (48, include_bytes!("../resources/brand/wolf-48.png")),
    (128, include_bytes!("../resources/brand/wolf-128.png")),
    (256, include_bytes!("../resources/brand/wolf-256.png")),
];

/// Call first thing in the process, before any thread exists (environment changes aren't thread-safe).
pub fn set_desktop_name() {
    if std::env::var_os("CHROME_DESKTOP").is_none() {
        std::env::set_var("CHROME_DESKTOP", DESKTOP_NAME);
    }
}

/// Install or update ~/.local/share/applications/wolfweb.desktop and the wolf icons. Browser process only.
pub fn install_desktop_entry() {
    let Some(data) = dirs::data_dir() else { return };
    let Ok(exe) = std::env::current_exe() else { return };
    for (size, png) in ICONS {
        let dir = data.join(format!("icons/hicolor/{size}x{size}/apps"));
        let file = dir.join("wolfweb.png");
        if std::fs::read(&file).ok().as_deref() != Some(png) {
            let _ = std::fs::create_dir_all(&dir);
            let _ = std::fs::write(&file, png);
        }
    }
    let entry = desktop_entry(&exe.to_string_lossy());
    let apps = data.join("applications");
    let file = apps.join(DESKTOP_NAME);
    if std::fs::read_to_string(&file).ok().as_deref() != Some(entry.as_str()) {
        let _ = std::fs::create_dir_all(&apps);
        let _ = std::fs::write(&file, entry);
    }
}

/// The .desktop entry (freedesktop Desktop Entry Specification). Exec quoting: arguments containing spaces are
/// double-quoted, and `"`, `` ` ``, `$` and `\` inside them escaped with a backslash.
fn desktop_entry(exe: &str) -> String {
    let quoted = if exe.contains(|c: char| c.is_whitespace() || "\"`$\\".contains(c)) {
        let mut q = String::from("\"");
        for c in exe.chars() {
            if "\"`$\\".contains(c) {
                q.push('\\');
            }
            q.push(c);
        }
        q.push('"');
        q
    } else {
        exe.to_string()
    };
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=WolfWeb\n\
         GenericName=Web Browser\n\
         Comment=A fast, private web browser that runs Chrome extensions\n\
         Exec={quoted} %U\n\
         Icon=wolfweb\n\
         Terminal=false\n\
         Categories=Network;WebBrowser;\n\
         MimeType=text/html;x-scheme-handler/http;x-scheme-handler/https;\n\
         StartupWMClass=wolfweb\n\
         StartupNotify=true\n"
    )
}

#[cfg(test)]
mod tests {
    use super::desktop_entry;

    #[test]
    fn exec_is_quoted_when_needed() {
        assert!(desktop_entry("/opt/wolfweb/wolfweb").contains("Exec=/opt/wolfweb/wolfweb %U\n"));
        assert!(desktop_entry("/home/me/My Apps/wolfweb").contains("Exec=\"/home/me/My Apps/wolfweb\" %U\n"));
        assert!(desktop_entry("/a$b/wolfweb").contains("Exec=\"/a\\$b/wolfweb\" %U\n"));
        assert!(desktop_entry("/x").contains("Icon=wolfweb\n"));
    }
}
