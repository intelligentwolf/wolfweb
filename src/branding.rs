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
// Chromium's product logos are theme images with no stable name CEF can map (cef_id_for_pack_resource_name returns -1
// for IDR_PRODUCT_LOGO_16/_32/_NAME_22 — observed), so their ids come from scanning this exact CEF build's packs
// (CEF 156.0.15 / Chromium 156.0.8078.80, pinned by Cargo.toml `cef = "=156.3.0"`): every PNG in
// chrome_100_percent.pak, chrome_200_percent.pak and resources.pak was compared pixel for pixel with Chromium's
// logos from chrome/app/theme/{chromium,default_100_percent/chromium,default_200_percent/chromium} at tag
// 156.0.8078.80. `scanned_ids_match_this_cef_build` re-checks them against third_party/cef.
//
// The logo inside the address bar's chip on chrome:// pages is a vector icon compiled into Chromium
// (chrome/browser/ui/toolbar/chrome_location_bar_model_delegate.cc:197-201 omnibox::kChromeProductIcon) and can't
// be replaced at run time.

const WOLF_16: &[u8] = include_bytes!("../resources/brand/wolf-16.png");
const WOLF_24: &[u8] = include_bytes!("../resources/brand/wolf-24.png");
const WOLF_32: &[u8] = include_bytes!("../resources/brand/wolf-32.png");
const WOLF_64: &[u8] = include_bytes!("../resources/brand/wolf-64.png");
const WOLF_128: &[u8] = include_bytes!("../resources/brand/wolf-128.png");
/// WebUI's Chromium logo SVG (resources.pak 51305 = IDR_WEBUI_IMAGES_CHROME_LOGO_DARK_SVG, found with
/// cef_id_for_pack_resource_name; Chromium stores it gzip-compressed, so the replacement is gzipped too).
const WOLF_SVG_GZ: &[u8] = include_bytes!("../resources/brand/wolf-logo.svg.gz");
const WORDMARK_1X: &[u8] = include_bytes!("../resources/brand/wordmark-dark-1x.png");
const WORDMARK_2X: &[u8] = include_bytes!("../resources/brand/wordmark-dark-2x.png");
const WORDMARK_WHITE_1X: &[u8] = include_bytes!("../resources/brand/wordmark-white-1x.png");
const WORDMARK_WHITE_2X: &[u8] = include_bytes!("../resources/brand/wordmark-white-2x.png");

/// Scale-independent images (resources.pak): 14187 16 px, 14188 24 px, 14189 64 px, 14190 128 px.
fn unscaled_image(id: i32) -> Option<&'static [u8]> {
    Some(match id {
        14187 => WOLF_16,
        14188 => WOLF_24,
        14189 => WOLF_64,
        14190 => WOLF_128,
        51305 => WOLF_SVG_GZ,
        _ => return scaled_image(id, false),
    })
}

/// Scaled images (chrome_100_percent.pak / chrome_200_percent.pak): 14307 IDR_PRODUCT_LOGO_32 (32 / 64 px),
/// 14309 IDR_PRODUCT_LOGO_16 Linux (16 px in both), 14313 / 14314 IDR_PRODUCT_LOGO_NAME_22 (+_WHITE) (97×22 /
/// 194×44), 40264 a 16 px logo (16 / 32 px).
fn scaled_image(id: i32, double: bool) -> Option<&'static [u8]> {
    Some(match (id, double) {
        (14307, false) => WOLF_32,
        (14307, true) => WOLF_64,
        (14309, _) => WOLF_16,
        (14313, false) => WORDMARK_1X,
        (14313, true) => WORDMARK_2X,
        (14314, false) => WORDMARK_WHITE_1X,
        (14314, true) => WORDMARK_WHITE_2X,
        (40264, false) => WOLF_16,
        (40264, true) => WOLF_32,
        _ => return None,
    })
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

    /// The image ids must still be Chromium's logos in the CEF build in third_party/cef (catches a CEF upgrade that
    /// renumbers them). Skipped when the runtime hasn't been fetched.
    #[test]
    fn scanned_ids_match_this_cef_build() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("third_party/cef");
        let Ok(p100) = std::fs::read(dir.join("chrome_100_percent.pak")) else { return };
        let p200 = std::fs::read(dir.join("chrome_200_percent.pak")).unwrap();
        let res = std::fs::read(dir.join("resources.pak")).unwrap();
        let size = |pak: &[u8], id: u16| png_size(pak_entry(pak, id).unwrap_or_else(|| panic!("id {id} missing")));
        assert_eq!(size(&p100, 14307), (32, 32));
        assert_eq!(size(&p200, 14307), (64, 64));
        assert_eq!(size(&p100, 14309), (16, 16));
        assert_eq!(size(&p100, 14313), (97, 22));
        assert_eq!(size(&p200, 14313), (194, 44));
        assert_eq!(size(&p100, 14314), (97, 22));
        assert_eq!(size(&p100, 40264), (16, 16));
        assert_eq!(size(&res, 14187), (16, 16));
        assert_eq!(size(&res, 14188), (24, 24));
        assert_eq!(size(&res, 14189), (64, 64));
        assert_eq!(size(&res, 14190), (128, 128));
        // And WolfWeb's replacements have the same sizes.
        assert_eq!(png_size(super::WOLF_32), (32, 32));
        assert_eq!(png_size(super::WORDMARK_1X), (97, 22));
        assert_eq!(png_size(super::WORDMARK_WHITE_2X), (194, 44));
        assert_eq!(png_size(super::WOLF_128), (128, 128));
    }

    /// Chromium .pak version 5 (ui/base/resource/data_pack.cc): u32 version, u8 encoding, 3 pad, u16 count,
    /// u16 alias count, then (count + 1) × (u16 id, u32 offset), then aliases (u16 id, u16 entry index).
    fn pak_entry(pak: &[u8], want: u16) -> Option<&[u8]> {
        let u16_at = |o: usize| u16::from_le_bytes([pak[o], pak[o + 1]]);
        let u32_at = |o: usize| u32::from_le_bytes([pak[o], pak[o + 1], pak[o + 2], pak[o + 3]]);
        assert_eq!(u32_at(0), 5, "pak version");
        let count = u16_at(8) as usize;
        let aliases = u16_at(10) as usize;
        let entry = |i: usize| (u16_at(12 + 6 * i), u32_at(14 + 6 * i) as usize);
        for i in 0..count {
            let (id, start) = entry(i);
            if id == want {
                return Some(&pak[start..entry(i + 1).1]);
            }
        }
        let alias_base = 12 + 6 * (count + 1);
        for a in 0..aliases {
            if u16_at(alias_base + 4 * a) == want {
                let i = u16_at(alias_base + 4 * a + 2) as usize;
                return Some(&pak[entry(i).1..entry(i + 1).1]);
            }
        }
        None
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
