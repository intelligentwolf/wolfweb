Platform resources for the macOS and Windows bundles (`cargo run -p xtask -- bundle`).

- `mac/English.lproj/MainMenu.xib` — the application menu, from cef-rs `examples/cefsimple/resources/mac`
  (Apache-2.0 OR MIT) with the name changed to WolfWeb. The bundler compiles it to a .nib with `ibtool`.
- `mac/wolfweb.icns`, `win/wolfweb.ico`, `brand/wolf-*.png` — WolfWeb's icon, a W with wolf ears, drawn from
  `brand/wolf-logo.svg` (rsvg-convert for the PNGs; `npx tauri icon` on a 1024 px render for the .ico/.icns).
  On Windows the bundler also writes it into wolfweb.exe (xtask/src/win_icon.rs), which is CEF's bootstrap.exe.
- `win/wolfweb.exe.manifest` — from cef-rs `cef/src/build_util/win/cef-app.exe.manifest` (CEF's compatibility manifest).
