**WolfWeb** gets its own icon: a **W with wolf ears**. WolfWeb and WolfMail used to share the same wolf head, so the two looked alike side by side in the taskbar. WolfMail now has an envelope with wolf ears (WolfMail 0.1.15).

## What's changed

- **New icon everywhere**: the app icon, the taskbar, the About page, the new tab, settings and import pages, and their tab icons.
- **Windows: the taskbar shows WolfWeb, not Chromium's logo.** `wolfweb.exe` now carries WolfWeb's icon, and WolfWeb's windows use it.
- **Linux:** the published engine still has the old wolf head built in. WolfWeb swaps it for the new icon as it starts, so you don't need a new engine download.

## Downloads

| System | File | Status |
|---|---|---|
| Linux (x86-64) | `WolfWeb-v0.1.3-linux-x86_64.tar.gz` | Tested on Omarchy (Arch) |
| Windows (x86-64) | `WolfWeb-v0.1.3-windows-x86_64.zip` | **Not tested on a Windows PC yet.** The icon inside `wolfweb.exe` was checked after the build. Uses Chromium's standard engine (no H.264, no menu bar) |
| macOS (Apple silicon) | `WolfWeb-v0.1.3-macos-arm64.zip` | **Not tested yet.** Uses Chromium's standard engine; ad-hoc signed (right-click → Open the first time) |

**Linux:** unpack the archive anywhere (for example `~/.local/opt`) and run `wolfweb` inside it.

## Checking your download

Every file has a SHA-256 checksum in `SHA256SUMS`, a keyless cosign signature (`*.cosign.bundle`) and GitHub build provenance:

```
sha256sum -c SHA256SUMS --ignore-missing
gh attestation verify WolfWeb-v0.1.3-linux-x86_64.tar.gz --repo intelligentwolf/wolfweb
```

## Known limits

- On Linux with an X11 desktop, the window's own icon can still be the old wolf head until the next engine build. Wayland desktops (and every taskbar that uses WolfWeb's desktop entry) show the new one.
- The limits listed for v0.1.1 still apply.

Built by CodeWolf & IntelligentWolf Ltd
