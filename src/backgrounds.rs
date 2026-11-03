//! New-tab backgrounds from the user's Pictures folder. The WolfWeb extension asks for the list, then for one
//! picture by its number in that list — it never names a path, so nothing outside the Pictures folder can be read.

use crate::scheme::{param, Params, Reply};
use serde_json::json;
use std::path::{Path, PathBuf};

/// How deep to look below the Pictures folder (Pictures/Holidays/2024/…).
const MAX_DEPTH: usize = 2;
/// Enough for any real collection while keeping the walk quick.
const MAX_FILES: usize = 20_000;
/// Larger files are skipped (raw camera files renamed .jpg, panoramas…): 40 MB.
const MAX_BYTES: u64 = 40 * 1024 * 1024;

pub fn api(cmd: &str, params: &Params) -> Reply {
    let Some(folder) = dirs::picture_dir() else {
        return Reply::error(404, "This computer has no Pictures folder.");
    };
    let pictures = list(&folder);
    match cmd {
        "backgrounds/pictures" => Reply::json(&json!({
            "ok": true,
            "folder": folder.to_string_lossy(),
            "count": pictures.len(),
        })),
        "backgrounds/picture" => {
            let Some(index) = param(params, "index").and_then(|v| v.parse::<usize>().ok()) else {
                return Reply::error(400, "Which picture?");
            };
            let Some(path) = pictures.get(index) else {
                return Reply::error(404, "That picture isn't in your Pictures folder any more.");
            };
            match std::fs::read(path) {
                Ok(bytes) => Reply::bytes(mime_for(path), bytes),
                Err(e) => Reply::error(500, &format!("Couldn't read the picture: {e}")),
            }
        }
        _ => Reply::error(404, "Unknown request"),
    }
}

/// Image files under `folder`, sorted by path so a number means the same picture each time.
fn list(folder: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    walk(folder, 0, &mut out);
    out.sort();
    out
}

fn walk(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        if out.len() >= MAX_FILES {
            return;
        }
        let name = entry.file_name();
        if name.to_string_lossy().starts_with('.') {
            continue; // hidden files and folders (thumbnail caches)
        }
        // file_type() doesn't follow symlinks: a link out of the Pictures folder is never followed.
        let Ok(kind) = entry.file_type() else { continue };
        let path = entry.path();
        if kind.is_dir() {
            if depth < MAX_DEPTH {
                walk(&path, depth + 1, out);
            }
        } else if kind.is_file() && is_picture(&path) && entry.metadata().map(|m| m.len() <= MAX_BYTES).unwrap_or(false) {
            out.push(path);
        }
    }
}

fn is_picture(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).as_deref(),
        Some("jpg" | "jpeg" | "png" | "webp" | "avif" | "gif")
    )
}

fn mime_for(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).as_deref() {
        Some("png") => "image/png",
        Some("webp") => "image/webp",
        Some("avif") => "image/avif",
        Some("gif") => "image/gif",
        _ => "image/jpeg",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_pictures_only_within_depth_and_sorted() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("a/b/c")).unwrap();
        std::fs::create_dir_all(root.join(".thumbnails")).unwrap();
        for f in ["z.jpg", "a.PNG", "notes.txt", "a/b/deep.webp", "a/b/c/too-deep.jpg", ".thumbnails/t.jpg"] {
            std::fs::write(root.join(f), b"x").unwrap();
        }
        #[cfg(unix)]
        std::os::unix::fs::symlink("/etc", root.join("escape")).unwrap();
        let names: Vec<String> =
            list(root).iter().map(|p| p.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/")).collect();
        // Path order compares component by component: the folder "a" sorts before the file "a.PNG".
        assert_eq!(names, vec!["a/b/deep.webp", "a.PNG", "z.jpg"]);
        assert_eq!(mime_for(Path::new("x.PNG")), "image/png");
        assert_eq!(mime_for(Path::new("x.jpeg")), "image/jpeg");
    }
}
