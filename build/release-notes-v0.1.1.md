The first release of **WolfWeb**, IntelligentWolf's web browser: Chromium underneath, runs Chrome extensions, and the partner app to WolfMail.

## What's in it

- **Your home page and the WolfWeb Daily**: a new-tab page with your speed dial, and a daily broadsheet built from your interests and news sources. It brings you news for your own country (it asks where you're reading from) and prints properly on A4.
- **Looks follow Chrome**: "Customise" opens Chrome's Customize Chrome panel. Its background collections, your own picture, colours, and light or dark mode all apply to WolfWeb's home page.
- **Pinned tabs that stay**: pinned tabs come back every time WolfWeb starts. Typing an address or clicking a bookmark in a pinned tab opens a new tab instead of replacing it.
- **Your startup choice**: by default WolfWeb opens on its home page; "Continue where you left off" and "Open specific pages" work as in Chrome.
- **Websites get your country's English**: once you tell the Daily where you are, websites are asked for your language as it's used there (en-GB in Britain, for example), even if your computer is set to US English.
- **Import** from other browsers (bookmarks, passwords CSV), quick commands (Ctrl+Shift+Space), notes and gestures.
- **Linux only so far:**
  - WolfWeb's own build of the Chromium engine: H.264/AAC video (Facebook, and other sites that use it), a classic menu bar (File, Edit, View…) with the wolf logo, the sidebar showing pinned tabs at most 4 to a row, and the status bar (load time, reading time, zoom, clock) as a strip below the page instead of over it.
  - The engine reports itself as Google Chrome, so sites that block unknown browsers let you in.

The Linux engine is published as the `cef-156.0.15+gcf671c0+chromium-156.0.8078.80` pre-release. It was built on IntelligentWolf's own workstation from this repository's `build/cef` (the build script, patches and Dockerfile), with the same settings as the `cef` workflow, so you can rebuild it yourself.

## Downloads

| System | File | Status |
|---|---|---|
| Linux (x86-64) | `WolfWeb-v0.1.1-linux-x86_64.tar.gz` | Tested on Omarchy (Arch) and Nobara 44 (Fedora) |
| Windows (x86-64) | `WolfWeb-v0.1.1-windows-x86_64.zip` | **Not tested yet.** Uses Chromium's standard engine (no H.264, no menu bar) |
| macOS (Apple silicon) | `WolfWeb-v0.1.1-macos-arm64.zip` | **Not tested yet.** Uses Chromium's standard engine; ad-hoc signed (right-click → Open the first time) |

**Linux:** unpack the archive anywhere (for example `~/.local/opt`) and run `wolfweb` inside it.

## Checking your download

Every file has a SHA-256 checksum in `SHA256SUMS`, a keyless cosign signature (`*.cosign.bundle`) and GitHub build provenance:

```
sha256sum -c SHA256SUMS --ignore-missing
gh attestation verify WolfWeb-v0.1.1-linux-x86_64.tar.gz --repo intelligentwolf/wolfweb
```

## Known limits

- On some AMD graphics setups hardware video decoding drew black frames, so on Linux WolfWeb decodes video on the processor. To turn hardware decoding back on, start WolfWeb with `--enable-features=AcceleratedVideoDecodeLinuxGL`.
- The first start after an update can still run the built-in extension's previous background code; restart once more if something looks out of date.
- Daily stories from Google News show a headline only (Google's links don't lead straight to the article).

Built by CodeWolf & IntelligentWolf Ltd
