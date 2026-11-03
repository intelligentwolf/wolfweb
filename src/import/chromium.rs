//! Reading Chromium-family browser profiles (Vivaldi, Chrome, Chromium, Brave, Edge, Opera).
//!
//! Formats, checked against a real Vivaldi profile (2026-11-03) and Chromium's code:
//! - `Local State` (JSON): profile.info_cache.{dir}.name — the profile's display name.
//! - `<profile>/Bookmarks` (JSON): roots.{bookmark_bar, other, synced, trash(Vivaldi)}; nodes {type: "url"|"folder",
//!   name, url, children}.
//! - `<profile>/History` (SQLite): table urls(id, url, title, visit_count, typed_count, last_visit_time, hidden);
//!   last_visit_time in microseconds since 1601-01-01 (base/time/time.h:545-546
//!   kMicrosecondsFromWindowsToUnixEpoch = 11644473600000000).
//! - `<profile>/Preferences` (JSON): extensions.settings.{id}.{from_webstore, was_installed_by_default, manifest.name,
//!   path}.

use serde::Serialize;
use serde_json::Value;
use std::path::{Path, PathBuf};

/// Source: chromium base/time/time.h:545-546.
#[cfg(test)]
const MICROS_FROM_WINDOWS_TO_UNIX_EPOCH: i64 = 11_644_473_600_000_000;

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Source {
    /// Opaque id for the API (the profile folder).
    pub key: String,
    pub browser: String,
    pub profile: String,
    pub has_bookmarks: bool,
    pub has_history: bool,
    pub has_extensions: bool,
}

#[derive(Serialize, Debug, PartialEq)]
pub struct BookmarkNode {
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<BookmarkNode>>,
}

/// A history row exactly as Chromium stores it (times in µs since 1601), for staging into WolfWeb's own History.
#[derive(Serialize, serde::Deserialize, Debug, PartialEq, Clone)]
pub struct RawHistory {
    pub url: String,
    pub title: String,
    pub visit_count: i64,
    pub typed_count: i64,
    pub last_visit_time: i64,
}

#[derive(Serialize, Debug, PartialEq)]
pub struct ExtensionEntry {
    pub id: String,
    pub name: String,
    pub store_url: String,
}

/// (browser name, user-data folder, whether the folder is itself the only profile — Opera).
fn candidates() -> Vec<(&'static str, PathBuf, bool)> {
    let mut out = Vec::new();
    #[cfg(target_os = "linux")]
    if let Some(c) = dirs::config_dir() {
        out.extend([
            ("Vivaldi", c.join("vivaldi"), false),
            ("Vivaldi Snapshot", c.join("vivaldi-snapshot"), false),
            ("Google Chrome", c.join("google-chrome"), false),
            ("Google Chrome Beta", c.join("google-chrome-beta"), false),
            ("Chromium", c.join("chromium"), false),
            ("Brave", c.join("BraveSoftware/Brave-Browser"), false),
            ("Microsoft Edge", c.join("microsoft-edge"), false),
            ("Opera", c.join("opera"), true),
        ]);
    }
    #[cfg(target_os = "macos")]
    if let Some(a) = dirs::data_dir() {
        out.extend([
            ("Vivaldi", a.join("Vivaldi"), false),
            ("Google Chrome", a.join("Google/Chrome"), false),
            ("Chromium", a.join("Chromium"), false),
            ("Brave", a.join("BraveSoftware/Brave-Browser"), false),
            ("Microsoft Edge", a.join("Microsoft Edge"), false),
            ("Opera", a.join("com.operasoftware.Opera"), true),
        ]);
    }
    #[cfg(target_os = "windows")]
    {
        if let Some(l) = dirs::data_local_dir() {
            out.extend([
                ("Vivaldi", l.join("Vivaldi/User Data"), false),
                ("Google Chrome", l.join("Google/Chrome/User Data"), false),
                ("Chromium", l.join("Chromium/User Data"), false),
                ("Brave", l.join("BraveSoftware/Brave-Browser/User Data"), false),
                ("Microsoft Edge", l.join("Microsoft/Edge/User Data"), false),
            ]);
        }
        if let Some(r) = dirs::config_dir() {
            out.push(("Opera", r.join("Opera Software/Opera Stable"), true));
        }
    }
    out
}

/// Every importable profile on this computer.
pub fn detect() -> Vec<Source> {
    let mut found = Vec::new();
    for (browser, root, single) in candidates() {
        found.extend(profiles_in(browser, &root, single));
    }
    found
}

fn profiles_in(browser: &str, root: &Path, single: bool) -> Vec<Source> {
    if !root.is_dir() {
        return Vec::new();
    }
    if single {
        return make_source(browser, root, browser.to_string()).into_iter().collect();
    }
    let names = profile_names(root);
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(root)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.is_dir())
                .filter(|p| {
                    let n = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    n == "Default" || n.starts_with("Profile ")
                })
                .collect()
        })
        .unwrap_or_default();
    dirs.sort();
    dirs.into_iter()
        .filter_map(|dir| {
            let folder = dir.file_name()?.to_str()?.to_string();
            let display = names.iter().find(|(d, _)| *d == folder).map(|(_, n)| n.clone()).unwrap_or(folder);
            make_source(browser, &dir, display)
        })
        .collect()
}

/// (folder, display name) from `Local State` profile.info_cache.
fn profile_names(root: &Path) -> Vec<(String, String)> {
    let Ok(text) = std::fs::read_to_string(root.join("Local State")) else { return Vec::new() };
    let Ok(v) = serde_json::from_str::<Value>(&text) else { return Vec::new() };
    v.pointer("/profile/info_cache")
        .and_then(|c| c.as_object())
        .map(|c| {
            c.iter()
                .filter_map(|(dir, info)| Some((dir.clone(), info.get("name")?.as_str()?.to_string())))
                .collect()
        })
        .unwrap_or_default()
}

fn make_source(browser: &str, dir: &Path, profile: String) -> Option<Source> {
    let has_bookmarks = dir.join("Bookmarks").is_file();
    let has_history = dir.join("History").is_file();
    let has_extensions = dir.join("Preferences").is_file();
    if !has_bookmarks && !has_history {
        return None;
    }
    Some(Source {
        key: dir.to_string_lossy().to_string(),
        browser: browser.to_string(),
        profile,
        has_bookmarks,
        has_history,
        has_extensions,
    })
}

// ── Bookmarks ──────────────────────────────────────────────────────────────────────────────────────────────────

/// The bookmark folders worth importing, each as a top-level folder named like the browser's own.
pub fn bookmarks(source: &Source) -> Result<Vec<BookmarkNode>, String> {
    let path = Path::new(&source.key).join("Bookmarks");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("Couldn't read the bookmarks file: {e}"))?;
    parse_bookmarks(&text)
}

fn parse_bookmarks(text: &str) -> Result<Vec<BookmarkNode>, String> {
    let v: Value = serde_json::from_str(text).map_err(|_| "The bookmarks file isn't in the format WolfWeb expected.".to_string())?;
    let roots = v.get("roots").and_then(|r| r.as_object()).ok_or("The bookmarks file has no bookmarks in it.")?;
    let mut out = Vec::new();
    // Fixed order, the trash (Vivaldi) left out.
    for (key, title) in [("bookmark_bar", "Bookmarks bar"), ("other", "Other bookmarks"), ("synced", "Mobile bookmarks")] {
        let Some(node) = roots.get(key) else { continue };
        let children = convert_children(node);
        if !children.is_empty() {
            out.push(BookmarkNode { title: title.to_string(), url: None, children: Some(children) });
        }
    }
    Ok(out)
}

fn convert_children(folder: &Value) -> Vec<BookmarkNode> {
    folder
        .get("children")
        .and_then(|c| c.as_array())
        .map(|kids| kids.iter().filter_map(convert_node).collect())
        .unwrap_or_default()
}

fn convert_node(node: &Value) -> Option<BookmarkNode> {
    let title = node.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string();
    match node.get("type").and_then(|t| t.as_str())? {
        "url" => {
            let url = node.get("url").and_then(|u| u.as_str())?.to_string();
            importable_url(&url).then_some(BookmarkNode { title, url: Some(url), children: None })
        }
        "folder" => {
            // Vivaldi keeps a "Trash" folder inside the tree in older profiles too.
            if node.pointer("/meta_info/Trash").is_some() {
                return None;
            }
            Some(BookmarkNode { title, url: None, children: Some(convert_children(node)) })
        }
        _ => None,
    }
}

/// Only addresses Chrome's bookmarks/history APIs accept and that are safe to click.
fn importable_url(url: &str) -> bool {
    let u = url.to_ascii_lowercase();
    ["http://", "https://", "ftp://", "file://"].iter().any(|p| u.starts_with(p))
}

// ── History ────────────────────────────────────────────────────────────────────────────────────────────────────

pub fn history(source: &Source, limit: usize) -> Result<Vec<RawHistory>, String> {
    // The other browser may be running and holding its database locked: read a copy.
    let original = Path::new(&source.key).join("History");
    let mut suffix = [0u8; 8];
    getrandom::fill(&mut suffix).map_err(|e| e.to_string())?;
    let tmp = std::env::temp_dir().join(format!(
        "wolfweb-import-{}.sqlite",
        suffix.iter().map(|b| format!("{b:02x}")).collect::<String>()
    ));
    std::fs::copy(&original, &tmp).map_err(|e| format!("Couldn't copy the history file: {e}"))?;
    let result = read_history(&tmp, limit);
    let _ = std::fs::remove_file(&tmp);
    result
}

fn read_history(db: &Path, limit: usize) -> Result<Vec<RawHistory>, String> {
    let conn = rusqlite::Connection::open_with_flags(db, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| format!("Couldn't open the history file: {e}"))?;
    let mut stmt = conn
        .prepare(
            "SELECT url, title, visit_count, typed_count, last_visit_time FROM urls \
             WHERE hidden = 0 ORDER BY last_visit_time DESC LIMIT ?1",
        )
        .map_err(|e| format!("The history file isn't in the format WolfWeb expected: {e}"))?;
    let rows = stmt
        .query_map([limit as i64], |r| {
            Ok(RawHistory {
                url: r.get(0)?,
                title: r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                visit_count: r.get(2)?,
                typed_count: r.get(3)?,
                last_visit_time: r.get(4)?,
            })
        })
        .map_err(|e| format!("Couldn't read the history: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        let row = row.map_err(|e| format!("Couldn't read the history: {e}"))?;
        if importable_url(&row.url) && !row.url.starts_with("file:") {
            out.push(row);
        }
    }
    Ok(out)
}

/// Milliseconds since the Unix epoch for a Chromium time (µs since 1601).
#[cfg(test)]
fn unix_ms(chromium_time: i64) -> i64 {
    (chromium_time - MICROS_FROM_WINDOWS_TO_UNIX_EPOCH) / 1000
}

// ── Extensions ─────────────────────────────────────────────────────────────────────────────────────────────────

/// Web Store extensions installed in the profile, so they can be reinstalled in WolfWeb with one click each.
pub fn extensions(source: &Source) -> Result<Vec<ExtensionEntry>, String> {
    let dir = Path::new(&source.key);
    let text = std::fs::read_to_string(dir.join("Preferences")).map_err(|e| format!("Couldn't read the browser's settings: {e}"))?;
    parse_extensions(&text, &dir.join("Extensions"))
}

fn parse_extensions(prefs: &str, extensions_dir: &Path) -> Result<Vec<ExtensionEntry>, String> {
    let v: Value = serde_json::from_str(prefs).map_err(|_| "The browser's settings file isn't in the format WolfWeb expected.".to_string())?;
    let Some(settings) = v.pointer("/extensions/settings").and_then(|s| s.as_object()) else {
        return Ok(Vec::new());
    };
    let mut out: Vec<ExtensionEntry> = settings
        .iter()
        .filter(|(id, _)| is_extension_id(id))
        .filter(|(_, s)| s.get("from_webstore").and_then(|f| f.as_bool()) == Some(true))
        .filter(|(_, s)| s.get("was_installed_by_default").and_then(|f| f.as_bool()) != Some(true))
        .map(|(id, s)| {
            let raw = s.pointer("/manifest/name").and_then(|n| n.as_str()).map(str::to_string)
                .or_else(|| installed_manifest(extensions_dir, id).and_then(|m| m.get("name").and_then(|n| n.as_str()).map(str::to_string)))
                .unwrap_or_else(|| id.clone());
            let name = localize(&raw, s, extensions_dir, id).unwrap_or(raw);
            ExtensionEntry { id: id.clone(), name, store_url: format!("https://chromewebstore.google.com/detail/{id}") }
        })
        .collect();
    out.sort_by_key(|e| e.name.to_lowercase());
    Ok(out)
}

/// Extension ids are 32 letters a–p.
fn is_extension_id(id: &str) -> bool {
    id.len() == 32 && id.bytes().all(|b| (b'a'..=b'p').contains(&b))
}

/// The newest installed version's manifest.json.
fn installed_manifest(extensions_dir: &Path, id: &str) -> Option<Value> {
    let mut versions: Vec<PathBuf> = std::fs::read_dir(extensions_dir.join(id)).ok()?
        .filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.is_dir()).collect();
    versions.sort();
    let text = std::fs::read_to_string(versions.last()?.join("manifest.json")).ok()?;
    serde_json::from_str(&text).ok()
}

/// "__MSG_appName__" → the extension's default-locale message (Chrome i18n: _locales/<default_locale>/messages.json,
/// keys case-insensitive).
fn localize(raw: &str, setting: &Value, extensions_dir: &Path, id: &str) -> Option<String> {
    let key = raw.strip_prefix("__MSG_")?.strip_suffix("__")?.to_lowercase();
    let manifest = setting.get("manifest").cloned().or_else(|| installed_manifest(extensions_dir, id))?;
    let locale = manifest.get("default_locale")?.as_str()?.to_string();
    let rel = setting.get("path").and_then(|p| p.as_str()).map(PathBuf::from).or_else(|| {
        let mut versions: Vec<PathBuf> = std::fs::read_dir(extensions_dir.join(id)).ok()?
            .filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.is_dir()).collect();
        versions.sort();
        versions.last().cloned()
    })?;
    let base = if rel.is_absolute() { rel } else { extensions_dir.join(rel) };
    let text = std::fs::read_to_string(base.join("_locales").join(&locale).join("messages.json")).ok()?;
    let messages: Value = serde_json::from_str(&text).ok()?;
    messages.as_object()?.iter().find(|(k, _)| k.to_lowercase() == key)?.1.get("message")?.as_str().map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bookmarks_keep_folders_and_drop_trash_and_scripts() {
        let json = r#"{"roots":{
            "bookmark_bar":{"type":"folder","name":"Bookmarks bar","children":[
                {"type":"url","name":"Rust","url":"https://www.rust-lang.org/"},
                {"type":"url","name":"Bad","url":"javascript:alert(1)"},
                {"type":"folder","name":"Speed Dial","meta_info":{"Speeddial":"true"},"children":[
                    {"type":"url","name":"Wolf","url":"https://wolf.uk.com/"}]},
                {"type":"folder","name":"Trash","meta_info":{"Trash":"true"},"children":[
                    {"type":"url","name":"Old","url":"https://old.example/"}]}]},
            "other":{"type":"folder","name":"Other","children":[]},
            "synced":{"type":"folder","name":"Mobile","children":[]},
            "trash":{"type":"folder","name":"Trash","children":[{"type":"url","name":"Gone","url":"https://gone.example/"}]}
        },"version":1}"#;
        let b = parse_bookmarks(json).unwrap();
        assert_eq!(b.len(), 1);
        assert_eq!(b[0].title, "Bookmarks bar");
        let kids = b[0].children.as_ref().unwrap();
        assert_eq!(kids.len(), 2);
        assert_eq!(kids[0].url.as_deref(), Some("https://www.rust-lang.org/"));
        assert_eq!(kids[1].title, "Speed Dial");
        assert_eq!(kids[1].children.as_ref().unwrap()[0].title, "Wolf");
    }

    #[test]
    fn history_reads_urls_table_and_converts_time() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("History");
        let c = rusqlite::Connection::open(&db).unwrap();
        c.execute_batch(
            "CREATE TABLE urls(id INTEGER PRIMARY KEY, url LONGVARCHAR, title LONGVARCHAR, visit_count INTEGER, \
             typed_count INTEGER, last_visit_time INTEGER, hidden INTEGER);
             INSERT INTO urls VALUES(1,'https://a.example/','A',3,0,13438170540919934,0);
             INSERT INTO urls VALUES(2,'https://hidden.example/','H',1,0,13438170540919935,1);
             INSERT INTO urls VALUES(3,'chrome://settings/','S',1,0,13438170540919936,0);",
        )
        .unwrap();
        drop(c);
        let h = read_history(&db, 10).unwrap();
        assert_eq!(h.len(), 1);
        assert_eq!(h[0].url, "https://a.example/");
        assert_eq!(h[0].visit_count, 3);
        assert_eq!(h[0].last_visit_time, 13438170540919934);
        // 13438170540919934 µs since 1601 = 1793696940919 ms since 1970 (2026-11-03).
        assert_eq!(unix_ms(h[0].last_visit_time), 1_793_696_940_919);
    }

    #[test]
    fn extensions_only_from_store_and_localized() {
        let dir = tempfile::tempdir().unwrap();
        let ext = dir.path().join("Extensions");
        let id = "ddkjiahejlhfcafbddmgiahcphecmpfh";
        let v = ext.join(id).join("1.0_0");
        std::fs::create_dir_all(v.join("_locales/en")).unwrap();
        std::fs::write(v.join("_locales/en/messages.json"), r#"{"extName":{"message":"uBlock Origin Lite"}}"#).unwrap();
        let prefs = format!(
            r#"{{"extensions":{{"settings":{{
                "{id}":{{"from_webstore":true,"path":"{id}/1.0_0","manifest":{{"name":"__MSG_extName__","default_locale":"en"}}}},
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa":{{"from_webstore":false,"manifest":{{"name":"Unpacked"}}}},
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb":{{"from_webstore":true,"was_installed_by_default":true,"manifest":{{"name":"Bundled"}}}},
                "not-an-id":{{"from_webstore":true,"manifest":{{"name":"Weird"}}}}
            }}}}}}"#
        );
        let e = parse_extensions(&prefs, &ext).unwrap();
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].name, "uBlock Origin Lite");
        assert_eq!(e[0].store_url, format!("https://chromewebstore.google.com/detail/{id}"));
    }

    #[test]
    fn profiles_are_found_with_display_names() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("vivaldi");
        std::fs::create_dir_all(root.join("Default")).unwrap();
        std::fs::create_dir_all(root.join("Profile 1")).unwrap();
        std::fs::create_dir_all(root.join("System Profile")).unwrap();
        std::fs::write(root.join("Default/Bookmarks"), "{}").unwrap();
        std::fs::write(root.join("Profile 1/History"), "").unwrap();
        std::fs::write(root.join("System Profile/History"), "").unwrap();
        std::fs::write(root.join("Local State"), r#"{"profile":{"info_cache":{"Default":{"name":"Paul"},"Profile 1":{"name":"Work"}}}}"#).unwrap();
        let s = profiles_in("Vivaldi", &root, false);
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].profile, "Paul");
        assert!(s[0].has_bookmarks && !s[0].has_history);
        assert_eq!(s[1].profile, "Work");
        assert!(profiles_in("Vivaldi", &dir.path().join("missing"), false).is_empty());
    }
}
