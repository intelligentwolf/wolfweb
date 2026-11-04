//! The languages websites are told the reader wants (the Accept-Language header, navigator.languages), made to
//! name the reader's country. Chromium starts a profile's list from the computer's language (a computer set to
//! en_US gives "en-US,en"), so a reader in Britain on such a computer is asked for American English. When the reader
//! tells WolfWeb their country (the Daily's "Where are you reading from?", extension/region.js), their first language
//! in that country's form goes first: "en-US,en" with GB → "en-GB,en-US,en", the list Chromium itself makes for a
//! computer set to en_GB.

use crate::scheme::{param, Params, Reply};
use cef::*;
use serde_json::json;

// Source: chromium components/language/core/browser/pref_names.h:16,19 — kAcceptLanguages "intl.accept_languages"
// (what websites are sent), kSelectedLanguages "intl.selected_languages" (the reader's list in Chrome's settings).
// language_prefs.cc:124-145 UpdateAcceptLanguagesPref copies the selected list (after any policy-forced
// languages) into the accept list; both are set so the change holds whether or not that copier is running.
const ACCEPT_LANGUAGES: &str = "intl.accept_languages";
const SELECTED_LANGUAGES: &str = "intl.selected_languages";

/// `region` (an ISO 3166 code, two capital letters) → the list with the first language's form for that country
/// first; None when it already comes first, or the list is empty.
fn with_region(list: &str, region: &str) -> Option<String> {
    let entries: Vec<&str> = list.split(',').map(str::trim).filter(|e| !e.is_empty()).collect();
    let base = entries.first()?.split(['-', '_']).next()?.to_ascii_lowercase();
    let wanted = format!("{base}-{region}");
    if entries[0].eq_ignore_ascii_case(&wanted) {
        return None;
    }
    let mut out = vec![wanted.clone()];
    out.extend(entries.iter().filter(|e| !e.eq_ignore_ascii_case(&wanted)).map(|e| e.to_string()));
    // The plain language stays in the list, as Chromium's own lists have it ("en-GB,en-US,en").
    if !out.iter().any(|e| e.eq_ignore_ascii_case(&base)) {
        out.push(base);
    }
    Some(out.join(","))
}

fn is_region(code: &str) -> bool {
    code.len() == 2 && code.bytes().all(|b| b.is_ascii_uppercase())
}

/// "languages/region": put `region`'s form of the reader's first language first. UI thread.
pub fn set_region(context: &RequestContext, params: &Params) -> Reply {
    let Some(region) = param(params, "region").filter(|r| is_region(r)) else {
        return Reply::error(400, "That isn't a country code.");
    };
    let read = |name: &str| {
        context.preference(Some(&CefString::from(name))).map(|v| CefString::from(&v.string()).to_string()).unwrap_or_default()
    };
    let mut current = read(SELECTED_LANGUAGES);
    if current.trim().is_empty() {
        current = read(ACCEPT_LANGUAGES);
    }
    let Some(list) = with_region(&current, region) else {
        return Reply::json(&json!({ "ok": true, "languages": current }));
    };
    for name in [SELECTED_LANGUAGES, ACCEPT_LANGUAGES] {
        let Some(value) = value_create() else {
            return Reply::error(500, "Couldn't make the setting.");
        };
        value.set_string(Some(&CefString::from(list.as_str())));
        if !crate::defaults::set_preference(context, name, value) {
            return Reply::error(500, "WolfWeb couldn't change the languages websites are sent.");
        }
    }
    Reply::json(&json!({ "ok": true, "languages": list }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn puts_the_country_first() {
        // Chromium's own list for an en_GB computer is "en-GB,en-US,en" (measured: Accept-Language en-GB,en-US;q=0.9,en;q=0.8).
        assert_eq!(with_region("en-US,en", "GB").as_deref(), Some("en-GB,en-US,en"));
        assert_eq!(with_region("en-GB,en-US,en", "US").as_deref(), Some("en-US,en-GB,en"));
        assert_eq!(with_region("de", "AT").as_deref(), Some("de-AT,de"));
        assert_eq!(with_region("fr-FR, fr ,en", "CA").as_deref(), Some("fr-CA,fr-FR,fr,en"));
    }

    #[test]
    fn leaves_it_alone_when_already_first_or_empty() {
        assert_eq!(with_region("en-GB,en-US,en", "GB"), None);
        assert_eq!(with_region("en-gb,en", "GB"), None);
        assert_eq!(with_region("", "GB"), None);
    }

    #[test]
    fn region_codes() {
        assert!(is_region("GB"));
        assert!(!is_region("gb"));
        assert!(!is_region("GBR"));
        assert!(!is_region(""));
        assert!(!is_region("G1"));
    }
}
