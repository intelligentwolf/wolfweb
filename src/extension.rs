//! The built-in WolfWeb extension (folder `extension/`): new-tab page, import, quick commands, notes panel, mouse
//! gestures. It is loaded into Chromium with `--load-extension` at every start.

use std::path::PathBuf;

/// Fixed by the public key in extension/manifest.json ("key"): Chromium derives an extension's id from the first
/// 128 bits of SHA-256 over that DER public key, written with the letters a–p. Checked at runtime: WolfWeb's
/// background service worker runs as chrome-extension://fhmfanjkjkjnbognbfdiegdineklmkjm/background.js.
pub const ID: &str = "fhmfanjkjkjnbognbfdiegdineklmkjm";

pub fn origin() -> String {
    format!("chrome-extension://{ID}")
}

/// Where the extension's files are: `extension/` beside the executable (Linux, Windows bundles), in the app's
/// Contents/Resources (macOS bundle — xtask puts it there), else the source tree (`cargo run`).
pub fn dir() -> Option<PathBuf> {
    let exe_dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    let candidates = [
        exe_dir.join("extension"),
        exe_dir.join("../Resources/extension"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("extension"),
    ];
    candidates.into_iter().find(|d| d.join("manifest.json").is_file()).and_then(|d| d.canonicalize().ok())
}

/// `--load-extension` value: ours plus any the user passed (comma-separated list).
pub fn load_extension_value(existing: &str) -> Option<String> {
    let ours = dir()?.to_string_lossy().to_string();
    let existing = existing.trim();
    if existing.split(',').any(|p| p == ours) {
        return None;
    }
    Some(if existing.is_empty() { ours } else { format!("{existing},{ours}") })
}

/// Source: chromium extensions/common/mojom/manifest.mojom:38 — ManifestLocation::kCommandLine = 8 (--load-extension).
const LOCATION_COMMAND_LINE: i64 = 8;

/// Before Chromium starts: if the profile has WolfWeb's extension stored as anything but a healthy command-line
/// extension, forget that entry so Chromium registers it afresh from --load-extension.
///
/// Why: pressing Reload on it in chrome://extensions re-registers it as an unpacked extension, and Chromium disables
/// unpacked extensions outside developer mode (chrome/browser/extensions/extension_management.cc:437-456,
/// DISABLE_UNSUPPORTED_DEVELOPER_EXTENSION). The stored disable reason then survives even once it loads from the
/// command line again (observed: disable_reasons [16777216] with location 8), leaving WolfWeb without its new-tab
/// page and import.
pub fn repair_profile(root: &std::path::Path) {
    // Another WolfWeb owns the profile right now (Chromium's process-singleton lock): leave its files alone.
    if ["SingletonLock", "lockfile"].iter().any(|l| root.join(l).symlink_metadata().is_ok()) {
        return;
    }
    let prefs = root.join("Default").join("Preferences");
    let Ok(text) = std::fs::read_to_string(&prefs) else { return };
    if let Some(fixed) = repaired_prefs(&text) {
        let tmp = prefs.with_extension("wolfweb-tmp");
        if std::fs::write(&tmp, fixed).is_ok() {
            let _ = std::fs::rename(&tmp, &prefs);
        }
    }
}

/// The Preferences JSON with WolfWeb's extension entry removed, or None when it is fine as it is.
fn repaired_prefs(text: &str) -> Option<String> {
    let mut v: serde_json::Value = serde_json::from_str(text).ok()?;
    let settings = v.pointer_mut("/extensions/settings")?.as_object_mut()?;
    let entry = settings.get(ID)?;
    let location = entry.get("location").and_then(|l| l.as_i64());
    let disabled = entry.get("disable_reasons").and_then(|d| d.as_array()).is_some_and(|d| !d.is_empty());
    if location == Some(LOCATION_COMMAND_LINE) && !disabled {
        return None;
    }
    settings.remove(ID);
    serde_json::to_string(&v).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    #[test]
    fn healthy_entry_is_left_alone_broken_one_removed() {
        let ok = format!(r#"{{"extensions":{{"settings":{{"{ID}":{{"location":8}},"other":{{"location":4}}}}}}}}"#);
        assert!(repaired_prefs(&ok).is_none());
        let unpacked = format!(r#"{{"extensions":{{"settings":{{"{ID}":{{"location":4}},"other":{{"location":4}}}}}}}}"#);
        let fixed: serde_json::Value = serde_json::from_str(&repaired_prefs(&unpacked).unwrap()).unwrap();
        assert!(fixed["extensions"]["settings"].get(ID).is_none());
        assert!(fixed["extensions"]["settings"].get("other").is_some());
        let disabled = format!(r#"{{"extensions":{{"settings":{{"{ID}":{{"location":8,"disable_reasons":[16777216]}}}}}}}}"#);
        assert!(repaired_prefs(&disabled).is_some());
        assert!(repaired_prefs(r#"{"extensions":{"settings":{}}}"#).is_none());
        assert!(repaired_prefs("not json").is_none());
    }

    /// The id must match the manifest key (Chromium: crx_file::id_util::GenerateId — SHA-256 of the key, first 16
    /// bytes, each nibble as 'a' + value).
    #[test]
    fn id_matches_manifest_key() {
        let manifest: serde_json::Value =
            serde_json::from_str(include_str!("../extension/manifest.json")).expect("manifest.json parses");
        let key = manifest["key"].as_str().expect("manifest has a key");
        let der = base64_decode(key);
        let hash = Sha256::digest(&der);
        let id: String = hash[..16].iter().flat_map(|b| [b >> 4, b & 0xf]).map(|n| (b'a' + n) as char).collect();
        assert_eq!(id, ID);
    }

    fn base64_decode(s: &str) -> Vec<u8> {
        const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = Vec::new();
        let mut buf = 0u32;
        let mut bits = 0;
        for c in s.bytes().filter(|c| *c != b'=') {
            let v = T.iter().position(|t| *t == c).expect("base64 character") as u32;
            buf = (buf << 6) | v;
            bits += 6;
            if bits >= 8 {
                bits -= 8;
                out.push((buf >> bits) as u8);
                buf &= (1 << bits) - 1;
            }
        }
        out
    }
}
