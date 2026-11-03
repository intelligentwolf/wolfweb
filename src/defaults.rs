//! First-run defaults: WolfWeb starts with its features on and visible; turning any of them off sticks, because this
//! runs once per profile (a marker file records that it has).
//!
//! Preference names (Chromium 156.0.8078.80): chrome/common/pref_names.h:604 kShowHomeButton
//! "browser.show_home_button", :94 kHomePageIsNewTabPage "homepage_is_newtabpage";
//! components/bookmarks/common/bookmark_pref_names.h:41 kShowBookmarkBar "bookmark_bar.show_on_all_tabs" (the bar
//! carries WolfWeb's menus and the 📰 Daily button) and :43-46 kBookmarkBarVisibilityState "bookmark_bar.visibility_state"
//! — the one Chromium 156 actually follows (chrome/browser/ui/bookmarks/bookmark_utils.cc:203-219), values from
//! components/bookmarks/common/bookmark_bar_visibility_state.h:15-23 (kAlwaysShow = 0, kOnlyShowOnNtp = 1,
//! kAlwaysHide = 2); extensions/browser/pref_names.h:131 kPinnedExtensions
//! "extensions.pinned_extensions" (WolfWeb's quick-commands button in the toolbar).

use cef::*;

/// Bumped when a default is added, so existing profiles get it once (v2: bookmark_bar.visibility_state).
const MARKER: &str = "wolfweb-defaults-2";
/// Source: bookmark_bar_visibility_state.h:17 kAlwaysShow = 0.
const BOOKMARK_BAR_ALWAYS_SHOW: i32 = 0;

/// Set a preference, reporting why it failed.
pub fn set_preference(context: &RequestContext, name: &str, mut value: Value) -> bool {
    // Non-empty on purpose: cef-rs passes an empty CefString as a null pointer and CEF's C wrapper then rejects
    // the call (cef-rs cef/src/string.rs:538-542).
    let mut error = CefString::from(" ");
    let ok = context.set_preference(Some(&CefString::from(name)), Some(&mut value), Some(&mut error)) != 0;
    if !ok {
        eprintln!("wolfweb: couldn't set {name}:{error}");
    }
    ok
}

pub fn bool_value(b: bool) -> Option<Value> {
    let v = value_create()?;
    v.set_bool(b as i32);
    Some(v)
}

/// Apply the defaults if this profile hasn't had them yet.
pub fn apply_once(context: &RequestContext) {
    let Ok(root) = crate::profile::root_dir() else { return };
    let marker = root.join(MARKER);
    if marker.exists() {
        return;
    }
    let mut all_ok = true;
    for name in ["browser.show_home_button", "bookmark_bar.show_on_all_tabs", "homepage_is_newtabpage"] {
        all_ok &= bool_value(true).is_some_and(|v| set_preference(context, name, v));
    }
    if let Some(v) = value_create() {
        v.set_int(BOOKMARK_BAR_ALWAYS_SHOW);
        all_ok &= set_preference(context, "bookmark_bar.visibility_state", v);
    }
    if let (Some(list), Some(value)) = (list_value_create(), value_create()) {
        list.set_string(0, Some(&CefString::from(crate::extension::ID)));
        value.set_list(Some(&mut list.clone()));
        all_ok &= set_preference(context, "extensions.pinned_extensions", value);
    }
    if all_ok {
        let _ = std::fs::write(&marker, b"WolfWeb's first-run defaults were applied to this profile.\n");
    }
}
