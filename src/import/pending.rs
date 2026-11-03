//! Imported history keeps its dates and visit counts: Chrome's extension API can only add an address "visited now",
//! so WolfWeb stages the rows and writes them into its own History database at the next start, before Chromium
//! opens the profile (it keeps the database locked while running).
//!
//! Schema: WolfWeb's Default/History (Chromium 156, meta version 70 — read from the profile 2026-11-03):
//!   urls(id, url, title, visit_count, typed_count, last_visit_time NOT NULL, hidden)
//!   visits(id, url NOT NULL, visit_time NOT NULL, from_visit, external_referrer_url, transition NOT NULL DEFAULT 0,
//!          segment_id, visit_duration … the rest have defaults)
//! Times are µs since 1601 in both browsers, so they copy across unchanged.

use super::chromium::RawHistory;
use std::path::{Path, PathBuf};

// Source: chromium ui/base/page_transition_types.h:30-40,142-145 — PAGE_TRANSITION_LINK = PAGE_TRANSITION_FIRST = 0,
// PAGE_TRANSITION_TYPED = 1, PAGE_TRANSITION_CHAIN_START = 0x10000000, PAGE_TRANSITION_CHAIN_END = 0x20000000.
const PAGE_TRANSITION_LINK: i64 = 0;
const PAGE_TRANSITION_TYPED: i64 = 1;
const PAGE_TRANSITION_CHAIN_START_END: i64 = 0x1000_0000 | 0x2000_0000;

const PENDING_FILE: &str = "wolfweb-pending-history.json";

fn pending_path(root: &Path) -> PathBuf {
    root.join(PENDING_FILE)
}

/// Stage rows for the next start. Adds to anything already staged.
pub fn stage(root: &Path, rows: &[RawHistory]) -> Result<usize, String> {
    let path = pending_path(root);
    let mut all: Vec<RawHistory> = std::fs::read_to_string(&path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    all.extend_from_slice(rows);
    let json = serde_json::to_string(&all).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json).map_err(|e| format!("Couldn't save the history to import: {e}"))?;
    std::fs::rename(&tmp, &path).map_err(|e| format!("Couldn't save the history to import: {e}"))?;
    Ok(rows.len())
}

/// At start-up, before Chromium: merge staged history into Default/History, then forget it. Skipped while another
/// WolfWeb holds the profile (Chromium's singleton lock) or before the profile's History exists (first run).
pub fn apply(root: &Path) {
    let path = pending_path(root);
    if !path.is_file() || ["SingletonLock", "lockfile"].iter().any(|l| root.join(l).symlink_metadata().is_ok()) {
        return;
    }
    let db = root.join("Default").join("History");
    if !db.is_file() {
        return;
    }
    let Some(rows) = std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str::<Vec<RawHistory>>(&t).ok())
    else {
        let _ = std::fs::remove_file(&path); // unreadable: don't retry forever
        return;
    };
    match merge(&db, &rows) {
        Ok(_) => {
            let _ = std::fs::remove_file(&path);
        }
        Err(e) => eprintln!("wolfweb: couldn't add the imported history yet (will try at the next start): {e}"),
    }
}

/// Insert each page WolfWeb doesn't know yet (with one visit at its last visit time). Pages already in WolfWeb's
/// history keep their own rows. Returns how many were added.
fn merge(db: &Path, rows: &[RawHistory]) -> rusqlite::Result<usize> {
    let mut conn = rusqlite::Connection::open(db)?;
    let tx = conn.transaction()?;
    let mut added = 0;
    for r in rows {
        let exists: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM urls WHERE url = ?1)", [&r.url], |row| row.get(0))?;
        if exists {
            continue;
        }
        tx.execute(
            "INSERT INTO urls (url, title, visit_count, typed_count, last_visit_time, hidden) VALUES (?1, ?2, ?3, ?4, ?5, 0)",
            rusqlite::params![r.url, r.title, r.visit_count.max(1), r.typed_count.max(0), r.last_visit_time],
        )?;
        let url_id = tx.last_insert_rowid();
        let core = if r.typed_count > 0 { PAGE_TRANSITION_TYPED } else { PAGE_TRANSITION_LINK };
        tx.execute(
            "INSERT INTO visits (url, visit_time, from_visit, transition, segment_id) VALUES (?1, ?2, 0, ?3, 0)",
            rusqlite::params![url_id, r.last_visit_time, core | PAGE_TRANSITION_CHAIN_START_END],
        )?;
        added += 1;
    }
    tx.commit()?;
    Ok(added)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn schema(db: &Path) {
        let c = rusqlite::Connection::open(db).unwrap();
        // The tables exactly as in WolfWeb's profile (see module doc).
        c.execute_batch(
            "CREATE TABLE urls(id INTEGER PRIMARY KEY AUTOINCREMENT,url LONGVARCHAR,title LONGVARCHAR,visit_count INTEGER DEFAULT 0 NOT NULL,typed_count INTEGER DEFAULT 0 NOT NULL,last_visit_time INTEGER NOT NULL,hidden INTEGER DEFAULT 0 NOT NULL);
             CREATE TABLE visits(id INTEGER PRIMARY KEY AUTOINCREMENT,url INTEGER NOT NULL,visit_time INTEGER NOT NULL,from_visit INTEGER,external_referrer_url TEXT,transition INTEGER DEFAULT 0 NOT NULL,segment_id INTEGER,visit_duration INTEGER DEFAULT 0 NOT NULL,incremented_omnibox_typed_score BOOLEAN DEFAULT FALSE NOT NULL,opener_visit INTEGER,originator_cache_guid TEXT,originator_visit_id INTEGER,originator_from_visit INTEGER,originator_opener_visit INTEGER,is_known_to_sync BOOLEAN DEFAULT FALSE NOT NULL,consider_for_ntp_most_visited BOOLEAN DEFAULT FALSE NOT NULL,visited_link_id INTEGER DEFAULT 0 NOT NULL,app_id TEXT);
             INSERT INTO urls(url,title,visit_count,typed_count,last_visit_time) VALUES('https://known.example/','Known',9,0,5);",
        )
        .unwrap();
    }

    #[test]
    fn staged_history_keeps_dates_and_skips_known_pages() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("Default")).unwrap();
        schema(&root.join("Default/History"));
        let rows = [
            RawHistory { url: "https://a.example/".into(), title: "A".into(), visit_count: 4, typed_count: 2, last_visit_time: 13_400_000_000_000_000 },
            RawHistory { url: "https://known.example/".into(), title: "Other".into(), visit_count: 1, typed_count: 0, last_visit_time: 7 },
        ];
        assert_eq!(stage(root, &rows[..1]).unwrap(), 1);
        assert_eq!(stage(root, &rows[1..]).unwrap(), 1);
        apply(root);
        assert!(!root.join(PENDING_FILE).exists());
        let c = rusqlite::Connection::open(root.join("Default/History")).unwrap();
        let (count, typed, last): (i64, i64, i64) = c
            .query_row("SELECT visit_count, typed_count, last_visit_time FROM urls WHERE url='https://a.example/'", [], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .unwrap();
        assert_eq!((count, typed, last), (4, 2, 13_400_000_000_000_000));
        let (vt, tr): (i64, i64) = c.query_row("SELECT visit_time, transition FROM visits", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
        assert_eq!(vt, 13_400_000_000_000_000);
        assert_eq!(tr, 1 | 0x3000_0000);
        let known: String = c.query_row("SELECT title FROM urls WHERE url='https://known.example/'", [], |r| r.get(0)).unwrap();
        assert_eq!(known, "Known");
    }

    #[test]
    fn waits_while_wolfweb_runs() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("Default")).unwrap();
        schema(&root.join("Default/History"));
        stage(root, &[RawHistory { url: "https://b.example/".into(), title: String::new(), visit_count: 1, typed_count: 0, last_visit_time: 1 }]).unwrap();
        std::fs::write(root.join("lockfile"), b"").unwrap();
        apply(root);
        assert!(root.join(PENDING_FILE).exists());
    }
}
