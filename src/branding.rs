//! "WolfWeb", not "Chromium", in window titles, menus, settings and dialogs — and the wolf, not Chromium's logo, in
//! the images Chromium takes from its resource packs.
//!
//! CefResourceBundleHandler::GetLocalizedString is asked for every UI string (cef_resource_bundle_handler.h:50-62).
//! WolfWeb reads Chromium's own text through CefResourceBundle::GetLocalizedString (cef_resource_bundle.h:62-69) —
//! with a per-thread guard, because that lookup asks this handler again — and swaps the product name. Credits and
//! licence text that name "The Chromium Authors" / "the Chromium project" keep their wording.

use cef::*;
use std::cell::Cell;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

const PRODUCT: &str = "WolfWeb";

thread_local! {
    static IN_LOOKUP: Cell<bool> = const { Cell::new(false) };
}

/// Rebranded strings by id (None = Chromium's string doesn't mention Chromium, use it unchanged).
fn cache() -> &'static Mutex<HashMap<i32, Option<String>>> {
    static CACHE: OnceLock<Mutex<HashMap<i32, Option<String>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Replace the product name, leaving attributions to the Chromium project intact, and add IntelligentWolf Ltd to
/// the copyright line.
///
/// Source strings (Chromium 156.0.8078.80): chrome/app/chromium_strings.grd:520-522 IDS_ABOUT_VERSION_COPYRIGHT
/// "Copyright {0,date,y} The Chromium Authors. All rights reserved."; components/components_chromium_strings.grd:
/// 278-280 IDS_VERSION_UI_LICENSE "Chromium is made possible by the <a …>Chromium</a> open source project and other
/// <a …>open source software</a>." — the linked "Chromium" names the project and stays.
pub fn rebrand(text: &str) -> Option<String> {
    if !text.contains("Chromium") {
        return None;
    }
    const KEEP: [&str; 5] = [
        "The Chromium Authors",
        "the Chromium Authors",
        "Chromium</a>",
        "Chromium project",
        "Chromium open source project",
    ];
    let mut out = String::with_capacity(text.len() + 24);
    let mut rest = text;
    while let Some(pos) = rest.find("Chromium") {
        let (before, from) = rest.split_at(pos);
        out.push_str(before);
        let kept = KEEP.iter().find(|k| {
            let start = k.find("Chromium").unwrap_or(0);
            out.ends_with(&k[..start]) && from.starts_with(&k[start..])
        });
        match kept {
            Some(k) => {
                let start = k.find("Chromium").unwrap_or(0);
                out.push_str(&k[start..]);
                rest = &from[k.len() - start..];
            }
            None => {
                out.push_str(PRODUCT);
                rest = &from["Chromium".len()..];
            }
        }
    }
    out.push_str(rest);
    if out.starts_with("Copyright ") && out.contains("The Chromium Authors") && !out.contains("IntelligentWolf") {
        out = out.replacen("The Chromium Authors", "IntelligentWolf Ltd and The Chromium Authors", 1);
    }
    Some(out)
}

wrap_resource_bundle_handler! {
    pub struct WolfBranding;

    impl ResourceBundleHandler {
        fn localized_string(&self, string_id: i32, string: Option<&mut CefString>) -> i32 {
            if IN_LOOKUP.with(|g| g.get()) {
                return 0; // the inner lookup: let Chromium answer with its own string
            }
            let Some(string) = string else { return 0 };
            let cached = cache().lock().ok().and_then(|c| c.get(&string_id).cloned());
            let branded = match cached {
                Some(v) => v,
                None => {
                    IN_LOOKUP.with(|g| g.set(true));
                    let original = resource_bundle_get_global()
                        .map(|rb| CefString::from(&rb.localized_string(string_id)).to_string())
                        .unwrap_or_default();
                    IN_LOOKUP.with(|g| g.set(false));
                    let v = rebrand(&original);
                    if let Ok(mut c) = cache().lock() {
                        c.insert(string_id, v.clone());
                    }
                    v
                }
            };
            match branded {
                // try_set writes into the cef_string_t CEF passed in; assigning a new CefString would only replace
                // the Rust wrapper and CEF would see nothing (cef-rs cef/src/string.rs:409 try_set).
                Some(text) => string.try_set(&text) as i32,
                None => 0,
            }
        }
    }
}

// ── Logos ──────────────────────────────────────────────────────────────────────────────────────────────────────
//
// Chromium's product logos are theme images with no name CEF can map (cef_id_for_pack_resource_name returns -1 for
// IDR_PRODUCT_LOGO_16/_32/_NAME_22 — observed) and their numeric ids differ between platforms (the Windows build has
// no 14307, which is IDR_PRODUCT_LOGO_32 on Linux — CI, 2026-11-03). So WolfWeb finds them by content: at the first
// image request it reads CEF's packs and matches each entry's SHA-256 against Chromium's own logo files. Pack
// entries are byte-for-byte the source PNGs (checked: chrome/app/theme/{chromium,default_100_percent/chromium,
// default_200_percent/chromium}/*product_logo* at tag 156.0.8078.80 hash the same as the entries in
// chrome_100_percent.pak / chrome_200_percent.pak / resources.pak), so the same table works on every platform.
//
// The logo inside the address bar's chip on chrome:// pages is a vector icon compiled into Chromium
// (chrome/browser/ui/toolbar/chrome_location_bar_model_delegate.cc:197-201 omnibox::kChromeProductIcon) and can't
// be replaced at run time.

const WOLF_16: &[u8] = include_bytes!("../resources/brand/wolf-16.png");
const WOLF_24: &[u8] = include_bytes!("../resources/brand/wolf-24.png");
const WOLF_32: &[u8] = include_bytes!("../resources/brand/wolf-32.png");
const WOLF_64: &[u8] = include_bytes!("../resources/brand/wolf-64.png");
const WOLF_128: &[u8] = include_bytes!("../resources/brand/wolf-128.png");
const WOLF_256: &[u8] = include_bytes!("../resources/brand/wolf-256.png");
const WORDMARK_1X: &[u8] = include_bytes!("../resources/brand/wordmark-dark-1x.png");
const WORDMARK_2X: &[u8] = include_bytes!("../resources/brand/wordmark-dark-2x.png");
const WORDMARK_WHITE_1X: &[u8] = include_bytes!("../resources/brand/wordmark-white-1x.png");
const WORDMARK_WHITE_2X: &[u8] = include_bytes!("../resources/brand/wordmark-white-2x.png");
/// WebUI's Chromium logo SVG, stored gzip-compressed by Chromium, so the replacement is gzipped too. Found by name:
/// IDR_WEBUI_IMAGES_CHROME_LOGO_DARK_SVG (cef_pack_resources.h).
const WOLF_SVG_GZ: &[u8] = include_bytes!("../resources/brand/wolf-logo.svg.gz");

/// SHA-256 of Chromium 156's logo files → WolfWeb's image of the same size.
const LOGOS: &[(&str, &[u8])] = &[
    // 16 px: chromium/product_logo_16.png, default_100_percent/chromium/{,linux/}product_logo_16.png, and the
    // 200%-pack copy of the Linux one.
    ("a393eac540413ebbc51ba033b96d6f87918d1039f1ec16cd8c0e6a57818f2ea1", WOLF_16),
    ("32ea0cf8a4f67b8fde3225391201f72bb4ef117295b104961c68d0502a7142d8", WOLF_16),
    ("fdbe4e892e67409252283e2eb1faa05a9be8131e4d2dbc1dcc0f5d14390be4e2", WOLF_16),
    ("2c1d17921ced4b60a2cc6e1fd96aeb398e13547dab18537a61d3c5ad57895ccf", WOLF_16),
    ("8a9918695448c5592ca69c103acd2a5c213240d82acef163afc9b0d92b6d280c", WOLF_16),
    // 24 px (resources.pak).
    ("7a431699c78af352352932f6dd4e04848b57990c0a5675a66695e1dd454cb592", WOLF_24),
    // 32 px: default_100_percent/chromium/product_logo_32.png, default_200_percent/chromium/product_logo_16.png.
    ("c29f2c754d619246062db49e4ba1c3b8e96fcce18ec4524f5ef371172af385ad", WOLF_32),
    ("9fef85f58e6e4a2cdcbe8b0d3d8fe833547362ab1501ab700d745876fd830f4e", WOLF_32),
    // 64 px: default_200_percent/chromium/product_logo_32.png, chromium/product_logo_64.png.
    ("926700351f770f2ae5d1298c75aa269da1416d1511e90a1eed07a5ac5e27e85e", WOLF_64),
    ("aa7f129dcfe02276ebd365781c6a0c3c8ab1ca41abf3408ca47e604e7fe12018", WOLF_64),
    // 128 px, 256 px: chromium/product_logo_128.png, _256.png.
    ("ea2d5a7c15ae1e03138d2bb227d3ed4684808c7de10a1443d776cde3ca3dcd9d", WOLF_128),
    ("e14120fdefb8eb455f44eac572f34bda75c32c9404e5c3745d44793dae217331", WOLF_256),
    // The "Chromium" wordmark (product_logo_name_22, _white) at 1× and 2×.
    ("fd2ba439f3aad0b56b8228bc966f4f3c425a56f4c7c3e77b8a850fc7bfc93b24", WORDMARK_1X),
    ("655d7d7e6d0b2997a948f8ca414a71e84e7c9615df928be01c1c4ca744085b9c", WORDMARK_WHITE_1X),
    ("91f827498cde94ab869a24a861eaf37a6550733ec58f452124feeedccb4596f6", WORDMARK_2X),
    ("23195e26c92c275e99b9c55433e5add1741af93171cdee0ef6c4013f8a6cf205", WORDMARK_WHITE_2X),
];

/// SHA-256 of the earlier wolf-head logo (resources/brand/wolf-<N>.png before the W-with-ears icon) → today's image
/// of the same size. WolfWeb's own engine (build/cef/build.sh brand()) bakes the logo into its packs at build time,
/// so an engine built before the change carries the old head; this swaps it at run time like Chromium's own logos.
/// Checked against the cef-156.0.15 engine's packs: chrome_100_percent.pak 14307 (32 px), 14309 (16 px);
/// chrome_200_percent.pak 14307 (64 px), 40264 (32 px); resources.pak 14150/14151/14152 (64/128/256 px),
/// 14187/14188 (16/24 px). The 48 px file is in none of them.
const OLD_WOLF_LOGOS: &[(&str, &[u8])] = &[
    ("40455b05e71f183a0f8b7001407e51a1b92f7bec65bedb3e9014ef74a4622e94", WOLF_16),
    ("48fad94af379953a956b191b697c279ff3f446b05a75d7dc85ee5c0a9fd326f2", WOLF_24),
    ("30589d80f45b984c4aa6bc0465df0fbbb32d1e69f7564d9dd8f82122593a08ac", WOLF_32),
    ("e9ea6d442a10a40b44aac36603bc69a686ef8556ba4dbd1ecbbac022eebe59f9", WOLF_64),
    ("e4915017920d92dd9824172190731cd50a01e53d0a6f2fdf4ab456c02791448d", WOLF_128),
    ("d1894c99b64eb8394b052b6e2c2bc0072d6ac37240781b262d5b5bf448a9ebd6", WOLF_256),
];

/// Which resource ids to replace, per pack.
#[derive(Default)]
struct LogoIds {
    scale_100: std::collections::HashMap<i32, &'static [u8]>,
    scale_200: std::collections::HashMap<i32, &'static [u8]>,
    unscaled: std::collections::HashMap<i32, &'static [u8]>,
}

fn logo_ids() -> &'static LogoIds {
    static IDS: OnceLock<LogoIds> = OnceLock::new();
    IDS.get_or_init(|| {
        // Only the browser process draws Chromium's interface; helpers (renderers…) skip the work.
        let is_browser = command_line_get_global().map(|c| c.has_switch(Some(&CefString::from("type"))) == 0).unwrap_or(false);
        if !is_browser {
            return LogoIds::default();
        }
        let Some(dir) = pak_dir() else { return LogoIds::default() };
        let mut ids = LogoIds {
            scale_100: scan(&dir.join("chrome_100_percent.pak")),
            scale_200: scan(&dir.join("chrome_200_percent.pak")),
            unscaled: scan(&dir.join("resources.pak")),
        };
        let name = std::ffi::CString::new("IDR_WEBUI_IMAGES_CHROME_LOGO_DARK_SVG").expect("no NUL");
        // SAFETY: a valid NUL-terminated string; the function only reads it.
        let svg = unsafe { sys::cef_id_for_pack_resource_name(name.as_ptr()) };
        if svg >= 0 {
            ids.unscaled.insert(svg, WOLF_SVG_GZ);
        }
        ids
    })
}

/// Where CEF keeps its .pak files: beside the executable (Linux, Windows), in the framework's Resources (macOS).
fn pak_dir() -> Option<std::path::PathBuf> {
    let exe_dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    [exe_dir.clone(), exe_dir.join("../Frameworks/Chromium Embedded Framework.framework/Resources")]
        .into_iter()
        .find(|d| d.join("chrome_100_percent.pak").is_file())
}

/// Resource ids in one pack whose bytes are a known Chromium logo or the old wolf head.
fn scan(pak: &std::path::Path) -> std::collections::HashMap<i32, &'static [u8]> {
    scan_for(pak, &[LOGOS, OLD_WOLF_LOGOS])
}

type LogoTable = &'static [(&'static str, &'static [u8])];

fn scan_for(pak: &std::path::Path, tables: &[LogoTable]) -> std::collections::HashMap<i32, &'static [u8]> {
    use sha2::{Digest, Sha256};
    let mut found = std::collections::HashMap::new();
    let Ok(data) = std::fs::read(pak) else { return found };
    for (id, bytes) in pak_entries(&data) {
        if !bytes.starts_with(b"\x89PNG") {
            continue;
        }
        let hash: String = Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect();
        if let Some((_, image)) = tables.iter().flat_map(|t| t.iter()).find(|(h, _)| *h == hash) {
            found.insert(i32::from(id), *image);
        }
    }
    found
}

/// Entries of a Chromium .pak, version 5 (ui/base/resource/data_pack.cc): u32 version, u8 encoding, 3 pad,
/// u16 resource count, u16 alias count, then (count + 1) × (u16 id, u32 offset), then aliases (u16 id, u16 index).
fn pak_entries(pak: &[u8]) -> Vec<(u16, &[u8])> {
    let u16_at = |o: usize| pak.get(o..o + 2).map(|b| u16::from_le_bytes([b[0], b[1]]));
    let u32_at = |o: usize| pak.get(o..o + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
    let mut out = Vec::new();
    if u32_at(0) != Some(5) {
        return out;
    }
    let (Some(count), Some(aliases)) = (u16_at(8), u16_at(10)) else { return out };
    let (count, aliases) = (count as usize, aliases as usize);
    let entry = |i: usize| Some((u16_at(12 + 6 * i)?, u32_at(14 + 6 * i)? as usize));
    let mut by_index = Vec::with_capacity(count);
    for i in 0..count {
        let (Some((id, start)), Some((_, end))) = (entry(i), entry(i + 1)) else { return out };
        let Some(bytes) = pak.get(start..end) else { return out };
        by_index.push(bytes);
        out.push((id, bytes));
    }
    let alias_base = 12 + 6 * (count + 1);
    for a in 0..aliases {
        if let (Some(id), Some(index)) = (u16_at(alias_base + 4 * a), u16_at(alias_base + 4 * a + 2)) {
            if let Some(bytes) = by_index.get(index as usize) {
                out.push((id, bytes));
            }
        }
    }
    out
}

fn unscaled_image(id: i32) -> Option<&'static [u8]> {
    let ids = logo_ids();
    ids.unscaled.get(&id).or_else(|| ids.scale_100.get(&id)).copied()
}

fn scaled_image(id: i32, double: bool) -> Option<&'static [u8]> {
    let ids = logo_ids();
    let (first, second) = if double { (&ids.scale_200, &ids.scale_100) } else { (&ids.scale_100, &ids.scale_200) };
    first.get(&id).or_else(|| second.get(&id)).copied()
}

/// Hand CEF a pointer to a static image (CefResourceBundleHandler::GetDataResource: "The resource data will not be
/// copied and must remain resident in memory" — cef_resource_bundle_handler.h:64-72).
///
/// SAFETY: `data`/`data_size` are CEF's out-parameters for this call; `image` is 'static.
unsafe fn give(image: &'static [u8], data: *mut *mut std::ffi::c_void, data_size: *mut usize) -> i32 {
    if data.is_null() || data_size.is_null() {
        return 0;
    }
    *data = image.as_ptr() as *mut std::ffi::c_void;
    *data_size = image.len();
    1
}

extern "C" fn get_data_resource(
    _self: *mut sys::_cef_resource_bundle_handler_t,
    resource_id: std::ffi::c_int,
    data: *mut *mut std::ffi::c_void,
    data_size: *mut usize,
) -> std::ffi::c_int {
    match unscaled_image(resource_id) {
        // SAFETY: CEF's out-parameters for this call.
        Some(image) => unsafe { give(image, data, data_size) },
        None => 0,
    }
}

extern "C" fn get_data_resource_for_scale(
    _self: *mut sys::_cef_resource_bundle_handler_t,
    resource_id: std::ffi::c_int,
    scale_factor: sys::cef_scale_factor_t,
    data: *mut *mut std::ffi::c_void,
    data_size: *mut usize,
) -> std::ffi::c_int {
    let double = scale_factor as u32 >= sys::cef_scale_factor_t::SCALE_FACTOR_180P as u32;
    match scaled_image(resource_id, double) {
        // SAFETY: CEF's out-parameters for this call.
        Some(image) => unsafe { give(image, data, data_size) },
        None => 0,
    }
}

/// The resource-bundle handler: strings via `localized_string` above; images through these two C callbacks.
/// cef-rs's generated get_data_resource* treat CEF's `void*& data` out-parameter as an input buffer (they copy into
/// `*data` when it's non-null and otherwise drop what the Rust method returns — cef-rs bindings
/// impl_cef_resource_bundle_handler_t::get_data_resource), so the C function pointers are set directly.
pub fn handler() -> ResourceBundleHandler {
    let handler = WolfBranding::new();
    let raw = handler.get_raw();
    // SAFETY: `raw` is the live cef_resource_bundle_handler_t this wrapper owns; only its two function-pointer
    // fields change, before CEF has seen the object.
    unsafe {
        (*raw).get_data_resource = Some(get_data_resource);
        (*raw).get_data_resource_for_scale = Some(get_data_resource_for_scale);
    }
    handler
}

#[cfg(test)]
mod tests {
    use super::rebrand;

    /// Every logo in this CEF build's packs is found by fingerprint, and replaced by an image of the same size.
    /// Skipped when the runtime hasn't been fetched. WolfWeb's own engine (build/cef/build.sh brand()) already
    /// carries the wolf in its packs (today's, or the old head from an engine built before the icon changed): then no
    /// Chromium logo may be left, and every old head is swapped for today's image of the same size.
    #[test]
    fn logos_are_found_by_fingerprint() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("third_party/cef");
        if !dir.join("chrome_100_percent.pak").is_file() {
            return;
        }
        let pak100 = std::fs::read(dir.join("chrome_100_percent.pak")).unwrap();
        let has_wolf = pak100.windows(super::WOLF_16.len()).any(|w| w == super::WOLF_16);
        let old_wolf = super::scan_for(&dir.join("chrome_100_percent.pak"), &[super::OLD_WOLF_LOGOS]);
        if has_wolf || !old_wolf.is_empty() {
            for pak in ["chrome_100_percent.pak", "chrome_200_percent.pak", "resources.pak"] {
                let left = super::scan_for(&dir.join(pak), &[super::LOGOS]);
                assert!(left.is_empty(), "Chromium logos left in WolfWeb's own engine ({pak}): {}", left.len());
                let data = std::fs::read(dir.join(pak)).unwrap();
                let old = super::scan_for(&dir.join(pak), &[super::OLD_WOLF_LOGOS]);
                for (id, bytes) in super::pak_entries(&data) {
                    if let Some(replacement) = old.get(&i32::from(id)) {
                        assert_eq!(png_size(bytes), png_size(replacement), "{pak} id {id}");
                    }
                }
            }
            return;
        }
        let p100 = super::scan(&dir.join("chrome_100_percent.pak"));
        let p200 = super::scan(&dir.join("chrome_200_percent.pak"));
        let res = super::scan(&dir.join("resources.pak"));
        assert!(p100.len() >= 4, "100% pack logos: {}", p100.len());
        assert!(p200.len() >= 4, "200% pack logos: {}", p200.len());
        assert!(res.len() >= 3, "resources.pak logos: {}", res.len());
        let pak = std::fs::read(dir.join("chrome_200_percent.pak")).unwrap();
        for (id, bytes) in super::pak_entries(&pak) {
            if let Some(replacement) = p200.get(&i32::from(id)) {
                assert_eq!(png_size(bytes), png_size(replacement), "id {id}");
            }
        }
    }

    #[test]
    fn replacements_have_chromiums_sizes() {
        assert_eq!(png_size(super::WOLF_32), (32, 32));
        assert_eq!(png_size(super::WOLF_256), (256, 256));
        assert_eq!(png_size(super::WORDMARK_1X), (97, 22));
        assert_eq!(png_size(super::WORDMARK_WHITE_2X), (194, 44));
        assert!(super::pak_entries(b"not a pak").is_empty());
    }

    /// Width and height from a PNG's IHDR.
    fn png_size(png: &[u8]) -> (u32, u32) {
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n", "not a PNG");
        let be = |o: usize| u32::from_be_bytes([png[o], png[o + 1], png[o + 2], png[o + 3]]);
        (be(16), be(20))
    }

    #[test]
    fn product_name_is_replaced() {
        assert_eq!(rebrand("About Chromium").as_deref(), Some("About WolfWeb"));
        assert_eq!(rebrand("$1 - Chromium").as_deref(), Some("$1 - WolfWeb"));
        assert_eq!(rebrand("Chromium is up to date. Chromium!").as_deref(), Some("WolfWeb is up to date. WolfWeb!"));
        assert_eq!(rebrand("Settings"), None);
    }

    #[test]
    fn about_page_credits_chromium_and_copyright_names_intelligentwolf() {
        assert_eq!(
            rebrand("Copyright {0,date,y} The Chromium Authors. All rights reserved.").as_deref(),
            Some("Copyright {0,date,y} IntelligentWolf Ltd and The Chromium Authors. All rights reserved.")
        );
        assert_eq!(
            rebrand(r#"Chromium is made possible by the <a target="_blank" href="$1">Chromium</a> open source project."#).as_deref(),
            Some(r#"WolfWeb is made possible by the <a target="_blank" href="$1">Chromium</a> open source project."#)
        );
    }

    #[test]
    fn attributions_are_kept() {
        assert_eq!(
            rebrand("Copyright The Chromium Authors. Chromium is made possible by the Chromium open source project.").as_deref(),
            Some("Copyright IntelligentWolf Ltd and The Chromium Authors. WolfWeb is made possible by the Chromium open source project.")
        );
    }
}
