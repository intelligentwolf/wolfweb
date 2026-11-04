//! `wolfweb://app/api/…` — the small JSON API the built-in WolfWeb extension uses for what only native code can do:
//! reading other browsers' profiles for import, starting WolfMail, and Chromium commands extensions can't reach
//! (vertical tabs, split view).
//!
//! Who may call it: only pages of the WolfWeb extension. The check is the URL of the frame that made the request,
//! which CEF's browser process tracks (a page can't set it; the extension declares no web_accessible_resources, so
//! no website can frame its pages). Chromium sends no `Origin` header to custom-scheme handlers (observed: only
//! Accept, Accept-Language and User-Agent arrive), so that can't be used. The reply's Access-Control-Allow-Origin
//! names the extension, so no other page can read a response either.
//!
//! Threads: CefSchemeHandlerFactory::Create and the CefResourceHandler methods run on the IO thread
//! (cef_resource_handler.h:82-85). File and process work runs on TID_FILE_USER_BLOCKING, browser commands on the UI
//! thread; either way the reply is completed through the CefCallback, so the IO thread never blocks and the page
//! learns whether its request worked.

use cef::*;
use std::sync::{Arc, Mutex};

pub const SCHEME: &str = "wolfweb";
pub const HOST: &str = "app";

/// A finished response.
#[derive(Clone)]
pub struct Reply {
    pub status: i32,
    pub mime: &'static str,
    pub body: Vec<u8>,
}

impl Reply {
    pub fn json(value: &serde_json::Value) -> Self {
        Reply { status: 200, mime: "application/json", body: value.to_string().into_bytes() }
    }
    pub fn error(status: i32, message: &str) -> Self {
        Reply {
            status,
            mime: "application/json",
            body: serde_json::json!({ "ok": false, "error": message }).to_string().into_bytes(),
        }
    }
    pub fn bytes(mime: &'static str, body: Vec<u8>) -> Self {
        Reply { status: 200, mime, body }
    }
}

/// Request parameters (query string).
pub type Params = Vec<(String, String)>;

pub fn param<'a>(params: &'a Params, name: &str) -> Option<&'a str> {
    params.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
}

pub fn register_factory() {
    let mut factory = WolfSchemeFactory::new();
    register_scheme_handler_factory(Some(&CefString::from(SCHEME)), Some(&CefString::from(HOST)), Some(&mut factory));
}

/// What to do with one request.
enum Route {
    Ready(Reply),
    /// Run `cmd` on `thread` and answer with its result.
    Deferred { cmd: String, params: Params, thread: Thread },
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Thread {
    /// May block (files, starting programs): TID_FILE_USER_BLOCKING.
    Blocking,
    /// Touches browsers: the UI thread.
    Ui,
}

fn thread_for(cmd: &str) -> Option<Thread> {
    if crate::import::is_api_command(cmd) || cmd == "wolfmail/open" || cmd == "chrome-theme/image" || cmd.starts_with("default-browser/") {
        Some(Thread::Blocking)
    } else if cmd == "browser/command" || cmd == "pdf/save" || cmd == "engine/features" || cmd == "menu-bar/set" || cmd == "chrome-theme" {
        Some(Thread::Ui)
    } else {
        None
    }
}

/// Is `frame_url` a page of the WolfWeb extension?
fn from_wolfweb_extension(frame_url: &str) -> bool {
    frame_url.strip_prefix(&crate::extension::origin()).is_some_and(|rest| rest.starts_with('/'))
}

fn route(url: &str, method: &str, frame_url: &str) -> Route {
    if !from_wolfweb_extension(frame_url) {
        return Route::Ready(Reply::error(403, "forbidden"));
    }
    if method != "GET" {
        return Route::Ready(Reply::error(405, "GET only"));
    }
    let Ok(parsed) = url::Url::parse(url) else { return Route::Ready(Reply::error(400, "bad address")) };
    if parsed.host_str() != Some(HOST) {
        return Route::Ready(Reply::error(404, "not found"));
    }
    let Some(cmd) = parsed.path().strip_prefix("/api/") else { return Route::Ready(Reply::error(404, "not found")) };
    let Some(thread) = thread_for(cmd) else { return Route::Ready(Reply::error(404, "not found")) };
    let params = parsed.query_pairs().map(|(k, v)| (k.into_owned(), v.into_owned())).collect();
    Route::Deferred { cmd: cmd.to_string(), params, thread }
}

/// The response being served, shared between the IO-thread handler calls and the blocking job that fills it.
struct Served {
    reply: Option<Reply>,
    offset: usize,
}

wrap_scheme_handler_factory! {
    struct WolfSchemeFactory;

    impl SchemeHandlerFactory {
        fn create(
            &self,
            browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            _scheme_name: Option<&CefString>,
            request: Option<&mut Request>,
        ) -> Option<ResourceHandler> {
            let request = request?;
            let url = CefString::from(&request.url()).to_string();
            let method = CefString::from(&request.method()).to_string();
            // No frame (e.g. a service worker or a request from the browser itself): refused.
            let frame_url = frame.map(|f| CefString::from(&f.url()).to_string()).unwrap_or_default();
            let mut route = route(&url, &method, &frame_url);
            // The tab that asked (pdf/save prints that tab).
            if let (Route::Deferred { params, .. }, Some(b)) = (&mut route, browser) {
                params.push(("__browser".to_string(), b.identifier().to_string()));
            }
            let served = Arc::new(Mutex::new(Served { reply: None, offset: 0 }));
            Some(WolfResource::new(served, Arc::new(Mutex::new(Some(route)))))
        }
    }
}

wrap_resource_handler! {
    struct WolfResource {
        served: Arc<Mutex<Served>>,
        pending: Arc<Mutex<Option<Route>>>,
    }

    impl ResourceHandler {
        fn open(
            &self,
            _request: Option<&mut Request>,
            handle_request: Option<&mut i32>,
            callback: Option<&mut Callback>,
        ) -> i32 {
            // cef_resource_handler.h:90-98 — handle now: handle_request=true, return true. Decide later:
            // handle_request=false, return true, then run |callback|.
            let route = self.pending.lock().ok().and_then(|mut p| p.take());
            let Some(handle_request) = handle_request else { return 0 };
            match route {
                Some(Route::Ready(reply)) => {
                    self.set_reply(reply);
                    *handle_request = 1;
                    1
                }
                Some(Route::Deferred { cmd, params, thread }) => match callback {
                    Some(callback) => {
                        let mut job = DeferredJob::new(cmd, params, self.served.clone(), callback.clone());
                        let id = match thread {
                            Thread::Blocking => ThreadId::FILE_USER_BLOCKING,
                            Thread::Ui => ThreadId::UI,
                        };
                        post_task(id, Some(&mut job));
                        *handle_request = 0;
                        1
                    }
                    None => {
                        self.set_reply(Reply::error(500, "no callback"));
                        *handle_request = 1;
                        1
                    }
                },
                None => {
                    *handle_request = 1;
                    0
                }
            }
        }

        fn response_headers(
            &self,
            response: Option<&mut Response>,
            response_length: Option<&mut i64>,
            _redirect_url: Option<&mut CefString>,
        ) {
            let Ok(served) = self.served.lock() else { return };
            let reply = served.reply.clone().unwrap_or_else(|| Reply::error(500, "no response"));
            if let Some(response) = response {
                response.set_status(reply.status);
                // CEF takes the bare MIME type and the charset separately (cef_response.h SetMimeType/SetCharset).
                response.set_mime_type(Some(&CefString::from(reply.mime)));
                if reply.mime == "application/json" {
                    response.set_charset(Some(&CefString::from("utf-8")));
                }
                let origin = crate::extension::origin();
                for (name, value) in [
                    ("Access-Control-Allow-Origin", origin.as_str()),
                    ("Vary", "Origin"),
                    ("Cache-Control", "no-store"),
                    ("X-Content-Type-Options", "nosniff"),
                ] {
                    response.set_header_by_name(Some(&CefString::from(name)), Some(&CefString::from(value)), 1);
                }
            }
            if let Some(len) = response_length {
                *len = reply.body.len() as i64;
            }
        }

        fn read(
            &self,
            data_out: *mut u8,
            bytes_to_read: i32,
            bytes_read: Option<&mut i32>,
            _callback: Option<&mut ResourceReadCallback>,
        ) -> i32 {
            // cef_resource_handler.h:159-171 — copy what's available and return true; at the end set
            // bytes_read=0 and return false.
            let Some(bytes_read) = bytes_read else { return 0 };
            let Ok(mut served) = self.served.lock() else {
                *bytes_read = -2;
                return 0;
            };
            let offset = served.offset;
            let Some(reply) = served.reply.as_ref() else {
                *bytes_read = 0;
                return 0;
            };
            let n = reply.body.len().saturating_sub(offset).min(bytes_to_read.max(0) as usize);
            if n == 0 || data_out.is_null() {
                *bytes_read = 0;
                return 0;
            }
            // SAFETY: CEF guarantees data_out points to at least bytes_to_read writable bytes
            // (cef_resource_handler.h:159-171), and n <= bytes_to_read.
            unsafe { std::ptr::copy_nonoverlapping(reply.body.as_ptr().add(offset), data_out, n) };
            served.offset += n;
            *bytes_read = n as i32;
            1
        }
    }
}

impl WolfResource {
    fn set_reply(&self, reply: Reply) {
        if let Ok(mut s) = self.served.lock() {
            s.reply = Some(reply);
            s.offset = 0;
        }
    }
}

wrap_task! {
    struct DeferredJob {
        cmd: String,
        params: Params,
        served: Arc<Mutex<Served>>,
        callback: Callback,
    }

    impl Task {
        fn execute(&self) {
            if self.cmd == "pdf/save" {
                // Finishes later, in PdfDone, when Chromium has written the file.
                start_pdf(&self.params, self.served.clone(), self.callback.clone());
                return;
            }
            let reply = if crate::import::is_api_command(&self.cmd) {
                crate::import::api(&self.cmd, &self.params)
            } else {
                crate::actions::api(&self.cmd, &self.params)
            };
            if let Ok(mut s) = self.served.lock() {
                s.reply = Some(reply);
                s.offset = 0;
            }
            // cef_callback.h Continue() — thread-safe.
            self.callback.cont();
        }
    }
}

// ── PDF of a WolfWeb page (the newspaper) ──────────────────────────────────────────────────────────────────────

/// Print the requesting tab to a PDF in the Downloads folder (CefBrowserHost::PrintToPDF, cef_browser.h:536-548).
/// UI thread. The reply is sent when Chromium reports the file written (PdfDone).
fn start_pdf(params: &Params, served: Arc<Mutex<Served>>, callback: Callback) {
    let finish = |reply: Reply| {
        if let Ok(mut s) = served.lock() {
            s.reply = Some(reply);
            s.offset = 0;
        }
        callback.cont();
    };
    let browser_id: i32 = param(params, "__browser").and_then(|v| v.parse().ok()).unwrap_or(0);
    let Some(host) = crate::tabs::browser_by_id(browser_id).and_then(|b| b.host()) else {
        return finish(Reply::error(409, "Couldn't find the page to save."));
    };
    let Some(dir) = dirs::download_dir().or_else(dirs::home_dir) else {
        return finish(Reply::error(500, "This computer has no Downloads folder."));
    };
    let title = pdf_title(param(params, "title").unwrap_or("WolfWeb page"));
    let path = unused_path(&dir, &title);
    let settings = PdfPrintSettings {
        print_background: 1,
        prefer_css_page_size: 1, // the page's @page size (the newspaper asks for A4)
        generate_tagged_pdf: 1,
        generate_document_outline: 1,
        ..Default::default()
    };
    let mut done = PdfDone::new(served.clone(), callback.clone());
    host.print_to_pdf(Some(&CefString::from(path.to_string_lossy().as_ref())), Some(&settings), Some(&mut done));
}

/// A safe file name: letters, digits, spaces and - _ ( ) only, at most 80 characters.
fn pdf_title(title: &str) -> String {
    let clean: String = title
        .chars()
        .map(|c| if c.is_alphanumeric() || " -_()".contains(c) { c } else { ' ' })
        .collect();
    let clean = clean.split_whitespace().collect::<Vec<_>>().join(" ");
    let clean: String = clean.chars().take(80).collect();
    if clean.is_empty() { "WolfWeb page".to_string() } else { clean }
}

/// `<dir>/<title>.pdf`, or `<title> (2).pdf`, … — never overwrite a file.
fn unused_path(dir: &std::path::Path, title: &str) -> std::path::PathBuf {
    let first = dir.join(format!("{title}.pdf"));
    if !first.exists() {
        return first;
    }
    (2..).map(|n| dir.join(format!("{title} ({n}).pdf"))).find(|p| !p.exists()).expect("some free name")
}

wrap_pdf_print_callback! {
    struct PdfDone {
        served: Arc<Mutex<Served>>,
        callback: Callback,
    }

    impl PdfPrintCallback {
        fn on_pdf_print_finished(&self, path: Option<&CefString>, ok: i32) {
            let path = path.map(|p| p.to_string()).unwrap_or_default();
            let reply = if ok != 0 {
                Reply::json(&serde_json::json!({ "ok": true, "path": path }))
            } else {
                Reply::error(500, "Chromium couldn't write the PDF.")
            };
            if let Ok(mut s) = self.served.lock() {
                s.reply = Some(reply);
                s.offset = 0;
            }
            self.callback.cont();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pdf_names_are_safe_and_never_overwrite() {
        assert_eq!(pdf_title("The WolfWeb Daily — 3 November 2026"), "The WolfWeb Daily 3 November 2026");
        assert_eq!(pdf_title("../../etc/passwd"), "etc passwd");
        assert_eq!(pdf_title("///"), "WolfWeb page");
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(unused_path(dir.path(), "x"), dir.path().join("x.pdf"));
        std::fs::write(dir.path().join("x.pdf"), b"").unwrap();
        assert_eq!(unused_path(dir.path(), "x"), dir.path().join("x (2).pdf"));
    }

    fn status(r: Route) -> i32 {
        match r {
            Route::Ready(r) => r.status,
            Route::Deferred { .. } => 0,
        }
    }

    #[test]
    fn only_the_wolfweb_extension_may_call() {
        let ours = format!("{}/import.html", crate::extension::origin());
        assert_eq!(status(route("wolfweb://app/api/import/sources", "GET", "https://evil.example/")), 403);
        assert_eq!(status(route("wolfweb://app/api/import/sources", "GET", "")), 403);
        assert_eq!(status(route("wolfweb://app/api/import/sources", "GET", "chrome-extension://aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/x.html")), 403);
        // A look-alike that merely starts with our id.
        assert_eq!(status(route("wolfweb://app/api/import/sources", "GET", &format!("{}x/import.html", crate::extension::origin()))), 403);
        assert_eq!(status(route("wolfweb://app/api/import/sources", "POST", &ours)), 405);
        assert_eq!(status(route("wolfweb://app/api/other", "GET", &ours)), 404);
        assert_eq!(status(route("wolfweb://elsewhere/api/import/sources", "GET", &ours)), 404);
        match route("wolfweb://app/api/import/history?source=x&limit=5", "GET", &ours) {
            Route::Deferred { cmd, params, thread } => {
                assert_eq!(cmd, "import/history");
                assert_eq!(param(&params, "limit"), Some("5"));
                assert_eq!(thread, Thread::Blocking);
            }
            Route::Ready(_) => panic!("expected a deferred route"),
        }
        match route("wolfweb://app/api/browser/command?name=vertical-tabs", "GET", &ours) {
            Route::Deferred { thread, .. } => assert_eq!(thread, Thread::Ui),
            Route::Ready(_) => panic!("expected a deferred route"),
        }
        match route("wolfweb://app/api/wolfmail/open", "GET", &ours) {
            Route::Deferred { thread, .. } => assert_eq!(thread, Thread::Blocking),
            Route::Ready(_) => panic!("expected a deferred route"),
        }
    }
}
