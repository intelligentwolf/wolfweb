//! Small shared helpers.

/// Turn what the user typed into a URL: a URL as-is, a bare host gets https://, anything else is a Google search.
pub fn url_from_input(input: &str) -> String {
    let text = input.trim();
    if text.is_empty() {
        return "chrome://newtab/".to_string();
    }
    let has_scheme = ["http://", "https://", "file://", "chrome://", "chrome-extension://", "wolfweb://", "about:", "data:", "view-source:"]
        .iter()
        .any(|p| text.to_ascii_lowercase().starts_with(p));
    if has_scheme {
        return text.to_string();
    }
    let looks_like_host = !text.contains(char::is_whitespace)
        && (text.starts_with("localhost") || (text.contains('.') && !text.ends_with('.') && !text.starts_with('.')));
    if looks_like_host {
        if let Ok(u) = url::Url::parse(&format!("https://{text}")) {
            if u.host_str().is_some() {
                return u.to_string();
            }
        }
    }
    google_search_url(text)
}

pub fn google_search_url(query: &str) -> String {
    let mut u = url::Url::parse("https://www.google.com/search").expect("static URL parses");
    u.query_pairs_mut().append_pair("q", query);
    u.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_becomes_url_or_search() {
        assert_eq!(url_from_input("https://example.com/a"), "https://example.com/a");
        assert_eq!(url_from_input("example.com"), "https://example.com/");
        assert_eq!(url_from_input("localhost:8080"), "https://localhost:8080/");
        assert_eq!(url_from_input("wolf browser"), "https://www.google.com/search?q=wolf+browser");
        assert_eq!(url_from_input("rust"), "https://www.google.com/search?q=rust");
        assert_eq!(url_from_input("chrome://extensions"), "chrome://extensions");
    }
}
