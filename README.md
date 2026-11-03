# WolfWeb

[![Sponsor on GitHub](https://img.shields.io/badge/Sponsor-%E2%9D%A4-red?style=for-the-badge&logo=github)](https://github.com/sponsors/intelligentwolf)

**A fast, private web browser that runs Chrome extensions — WolfMail's partner.**

WolfWeb is built in Rust on Chromium (through the [Chromium Embedded Framework](https://bitbucket.org/chromiumembedded/cef)
and the [`cef`](https://github.com/tauri-apps/cef-rs) crate), so websites and Chrome Web Store extensions work as
they do in Chrome. It wears WolfMail's colours and type, and keeps everything on your computer.

## What you get

- **Chrome extensions** — install anything from the [Chrome Web Store](https://chromewebstore.google.com/) with
  “Add to Chrome”; extension buttons and popups sit in the toolbar.
- **Google search** from the address bar and from WolfWeb's new-tab page, with your most-visited sites below it.
- **Backgrounds** (new-tab page → Customise): Wikimedia Commons' picture of the day (credited), a different photo from
  your Pictures folder each day, or a picture you choose.
- **Your own newspaper** — *The WolfWeb Daily*: pick your interests and news feeds and WolfWeb lays out a broadsheet
  front page; print it or download it as a PDF.
- **Bring your stuff** — bookmarks (including Vivaldi Speed Dials), history and your list of extensions from Vivaldi,
  Chrome, Chromium, Brave, Edge and Opera; passwords from the CSV file those browsers export; Firefox through the
  built-in importer. Open it from the new-tab page or quick commands.
- **Quick commands** (`Ctrl+Shift+Space`) — one box for open tabs, bookmarks, history and commands, like Vivaldi's;
  **command chains** run several commands one after another.
- **Side panel** — notes, web panels (any site kept beside the page you're on) and WolfMail.
- **Tabs your way** — vertical tabs, split view (two tabs side by side), tab groups, pinned and muted tabs, tab search,
  memory saver, private windows, and your windows and tabs reopened next time.
- **Mouse gestures** — hold `Alt` and drag: ← back, → forward, ↓ new tab, ↓→ close tab, ↑↓ reload, ↑ top of page.

## WolfWeb's partner: WolfMail

**[WolfMail](https://github.com/intelligentwolf/wolfmail)** is a fast, private desktop email client from the same
team — every inbox you have (Gmail, Outlook, any IMAP mailbox) in one window, with calendar and contacts, your mail
kept on your own computer. Free and open source, for Windows, macOS and Linux.

WolfWeb and WolfMail share their look, and WolfWeb's side panel has a **WolfMail** tab that opens it.
Download it from [WolfMail's releases](https://github.com/intelligentwolf/wolfmail/releases).

## How it's put together

| Part | What it does |
|---|---|
| `src/` (Rust) | Starts Chromium with WolfWeb's profile and brand, opens and restores windows and tabs, renames “Chromium” to “WolfWeb” in Chromium's interface, and reads other browsers' profiles for import (`src/import/`). |
| `extension/` | WolfWeb's built-in extension, loaded at every start: new-tab page, import page, quick commands, command chains, notes, gestures. |
| `wolfweb://app/api/…` | The small API the built-in extension uses for what only native code can do — import, Pictures-folder backgrounds, saving PDFs, starting WolfMail, vertical tabs and split view. Only WolfWeb's own extension pages may call it (`src/scheme.rs`). |
| `xtask/` | `cargo run -p xtask -- bundle [--release]` builds the bundle for the platform you're on. |

The tab strip, toolbar and menus are Chromium's own (in WolfWeb's colours): CEF allows one Chrome-style browser per
custom window, so a hand-drawn tab strip would break extensions (`src/tabs.rs` has the details).

## Build from source

You need Rust (the version is pinned in `rust-toolchain.toml`; `rustup` installs it). On Linux you also need the
usual Chromium runtime libraries (GTK 3, NSS, ALSA, libdrm, libgbm, libxkbcommon); on Windows the Visual Studio C++
build tools; on macOS Xcode's command-line tools.

```sh
scripts/fetch-cef.sh                        # the Chromium (CEF) runtime matching the cef crate, into third_party/cef
cargo run -p xtask -- bundle --release      # build and bundle for this platform, into dist/
cargo test --workspace                      # tests
```

| Platform | Bundle | Run |
|---|---|---|
| Linux | `dist/wolfweb-linux-<arch>/` | `dist/wolfweb-linux-*/wolfweb` |
| Windows | `dist/wolfweb-windows-<arch>\` (`wolfweb.exe` is CEF's sandbox bootstrap, WolfWeb itself is `wolfweb.dll`) | `wolfweb.exe` |
| macOS | `dist/wolfweb.app` (with its helper apps) | `open dist/wolfweb.app` |

Every push is built and tested on all three by `.github/workflows/build.yml`; tagging `v*` publishes a release with
checksums, cosign signatures and build provenance (`release.yml`). WolfWeb keeps its profile in
`~/.local/share/WolfWeb` (Linux), `~/Library/Application Support/WolfWeb` (macOS) or `%APPDATA%\WolfWeb` (Windows).

## Support WolfWeb

WolfWeb is free. If it's useful to you, sponsoring funds its development (and WolfMail's):
[github.com/sponsors/intelligentwolf](https://github.com/sponsors/intelligentwolf).

## Contributing

WolfWeb is open source, but **we don't accept outside contributions** — pull requests from outside IntelligentWolf
Ltd are closed automatically. Bug reports and ideas are welcome as [issues](../../issues). Security problems: please
report privately (see [SECURITY.md](SECURITY.md)).

## Licence

WolfWeb is free software under the [GNU General Public License v3.0 or later](LICENSE). Chromium and CEF are under
their own (BSD-style and other open-source) licences, listed in the `CREDITS.html` that ships with every build.

"WolfWeb", "WolfMail", "IntelligentWolf" and the wolf logos are trademarks of IntelligentWolf Ltd and are not covered
by the licence — please use your own name and branding for a modified version.

© IntelligentWolf Ltd
