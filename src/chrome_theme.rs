//! The look of WolfWeb's new-tab page comes from Chrome's own "Customize Chrome" panel (one place to choose it): the
//! background picture chosen there, shown on WolfWeb's page. Chrome only draws that picture on its own new-tab page,
//! so WolfWeb's page asks for it here. Light or dark follows Chrome by itself: regular pages follow the browser
//! theme's colour mode (chromium chrome/browser/chrome_content_browser_client.cc:4093-4099).

use crate::scheme::{param, Params, Reply};
use cef::*;
use serde_json::json;

/// Source: chromium chrome/common/pref_names.h:1644-1650 — kNtpCustomBackgroundDict "ntp.custom_background_dict2",
/// kNtpCustomBackgroundLocalToDevice "ntp.custom_background_local_to_device" (an uploaded picture),
/// kNtpCustomBackgroundLocalToDeviceId "ntp.custom_background_local_to_device_id".
const BACKGROUND_DICT: &str = "ntp.custom_background_dict2";
const LOCAL_TO_DEVICE: &str = "ntp.custom_background_local_to_device";
const LOCAL_TO_DEVICE_ID: &str = "ntp.custom_background_local_to_device_id";

/// Source: chromium components/themes/ntp_custom_background_service_constants.h:8-14 — the dictionary's keys.
const URL: &str = "background_url";
const ATTRIBUTION_1: &str = "attribution_line_1";
const ATTRIBUTION_2: &str = "attribution_line_2";
const ATTRIBUTION_URL: &str = "attribution_action_url";

/// Source: chromium chrome/common/url_constants.h:142-143 kChromeUIUntrustedNewTabPageBackgroundFilename
/// "background.jpg"; an uploaded picture is <profile>/<id>background.jpg, id empty for "Upload an image"
/// (ntp_custom_background_service.cc:286-297 GetCustomBackground, 401-408 SetBackgroundToLocalResource; a searched
/// wallpaper's id: wallpaper_search_background_manager.cc:38-39, 226-228).
const UPLOADED_FILE: &str = "background.jpg";

/// UI thread (scheme.rs thread_for): "chrome-theme" → the background chosen in Customize Chrome, if any.
/// `{ ok, background: null | {kind: "image", url, attribution1, attribution2, attributionUrl}
///                          | {kind: "uploaded", id} }`
pub fn current(context: &RequestContext) -> Reply {
    let pref = |name: &str| context.preference(Some(&CefString::from(name)));
    if pref(LOCAL_TO_DEVICE).is_some_and(|v| v.bool() != 0) {
        let id = pref(LOCAL_TO_DEVICE_ID).map(|v| CefString::from(&v.string()).to_string()).unwrap_or_default();
        if valid_id(&id) {
            return Reply::json(&json!({ "ok": true, "background": { "kind": "uploaded", "id": id } }));
        }
        return Reply::json(&json!({ "ok": true, "background": null }));
    }
    let Some(dict) = pref(BACKGROUND_DICT).and_then(|v| v.dictionary()) else {
        return Reply::json(&json!({ "ok": true, "background": null }));
    };
    let text = |key: &str| CefString::from(&dict.string(Some(&CefString::from(key)))).to_string();
    // Only web addresses go to the page (it puts them in CSS and a link).
    let Some(url) = https(&text(URL)) else {
        return Reply::json(&json!({ "ok": true, "background": null }));
    };
    Reply::json(&json!({ "ok": true, "background": {
        "kind": "image",
        "url": url,
        "attribution1": text(ATTRIBUTION_1),
        "attribution2": text(ATTRIBUTION_2),
        "attributionUrl": https(&text(ATTRIBUTION_URL)),
    } }))
}

/// Blocking thread: "chrome-theme/image?id=…" → the uploaded picture's bytes.
pub fn uploaded_image(params: &Params) -> Reply {
    let id = param(params, "id").unwrap_or("");
    if !valid_id(id) {
        return Reply::error(400, "bad picture id");
    }
    let Ok(root) = crate::profile::root_dir() else { return Reply::error(404, "no profile folder") };
    match std::fs::read(root.join("Default").join(format!("{id}{UPLOADED_FILE}"))) {
        Ok(bytes) => Reply::bytes("image/jpeg", bytes),
        Err(_) => Reply::error(404, "Chrome's uploaded picture isn't there"),
    }
}

/// A picture id is base::Token::ToString() (hex) or empty; nothing else may reach a file name.
fn valid_id(id: &str) -> bool {
    id.len() <= 64 && id.bytes().all(|b| b.is_ascii_hexdigit())
}

fn https(address: &str) -> Option<String> {
    let url = url::Url::parse(address).ok()?;
    (url.scheme() == "https").then(|| url.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picture_ids_cannot_leave_the_profile_folder() {
        assert!(valid_id(""));
        assert!(valid_id("0123456789ABCDEF0123456789abcdef"));
        assert!(!valid_id("../Preferences"));
        assert!(!valid_id("a/b"));
        assert!(!valid_id(&"a".repeat(65)));
    }

    #[test]
    fn only_https_addresses_are_passed_on() {
        assert_eq!(https("https://lh3.googleusercontent.com/x=w3840").as_deref(), Some("https://lh3.googleusercontent.com/x=w3840"));
        assert!(https("javascript:alert(1)").is_none());
        assert!(https("http://example.com/").is_none());
        assert!(https("").is_none());
    }
}
