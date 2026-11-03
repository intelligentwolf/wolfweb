//! Bringing your things over from other browsers.
//!
//! Chromium-family browsers (Vivaldi, Chrome, Chromium, Brave, Edge, Opera) are read here, in Rust: bookmarks,
//! history and the list of installed Web Store extensions. The WolfWeb extension's import page fetches this data
//! from wolfweb://app/api/import/… and writes it into WolfWeb through Chrome's own bookmarks and history APIs.
//! Passwords are not read here: you export them from the other browser as a CSV file and import that file in
//! WolfWeb's password manager (chrome://password-manager/settings → Import), so they never pass through WolfWeb's
//! own code. Firefox goes through Chromium's built-in importer (chrome://settings/importData).

mod chromium;
mod pending;

pub use pending::apply as apply_pending_history;

use crate::scheme::{param, Params, Reply};
use serde_json::json;


pub fn is_api_command(cmd: &str) -> bool {
    cmd.starts_with("import/")
}

/// Runs on a blocking-allowed CEF thread (TID_FILE_USER_BLOCKING) — see scheme.rs.
pub fn api(cmd: &str, params: &Params) -> Reply {
    let sources = chromium::detect();
    match cmd {
        "import/sources" => Reply::json(&json!({ "ok": true, "sources": sources })),
        "import/bookmarks" | "import/history" | "import/extensions" => {
            // Only a profile WolfWeb found itself can be read — the page can't point us at an arbitrary path.
            let Some(source) = param(params, "source").and_then(|key| sources.iter().find(|s| s.key == key)) else {
                return Reply::error(404, "That browser profile wasn't found. Refresh the list and try again.");
            };
            let result = match cmd {
                "import/bookmarks" => chromium::bookmarks(source).map(|b| json!({ "ok": true, "bookmarks": b })),
                "import/history" => {
                    // Staged for the next start, so dates and visit counts survive (pending.rs).
                    let limit = param(params, "limit").and_then(|v| v.parse().ok()).unwrap_or(5000).clamp(1, 50_000);
                    chromium::history(source, limit).and_then(|h| {
                        let root = crate::profile::root_dir().map_err(|e| e.to_string())?;
                        pending::stage(&root, &h).map(|n| json!({ "ok": true, "staged": n }))
                    })
                }
                _ => chromium::extensions(source).map(|e| json!({ "ok": true, "extensions": e })),
            };
            match result {
                Ok(v) => Reply::json(&v),
                Err(e) => Reply::error(500, &e),
            }
        }
        _ => Reply::error(404, "Unknown import request"),
    }
}
