Platform resources for the macOS and Windows bundles (`cargo run -p xtask -- bundle`).

- `mac/English.lproj/MainMenu.xib` — the application menu, from cef-rs `examples/cefsimple/resources/mac`
  (Apache-2.0 OR MIT) with the name changed to WolfWeb. The bundler compiles it to a .nib with `ibtool`.
- `mac/wolfweb.icns`, `win/wolfweb.ico` — WolfWeb's icon (IntelligentWolf artwork, from WolfMail).
- `win/wolfweb.exe.manifest` — from cef-rs `cef/src/build_util/win/cef-app.exe.manifest` (CEF's compatibility manifest).
