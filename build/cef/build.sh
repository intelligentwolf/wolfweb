#!/usr/bin/env bash
# WolfWeb's own CEF build (branch 8078 = Chromium 156, the version the `cef` crate in Cargo.toml binds).
# Runs inside the container from ./Dockerfile with the build volume mounted at /build:
#   build.sh download   — depot_tools, Chromium and CEF sources (no build)
#   build.sh build      — apply WolfWeb's patches (patches/), build Release, make the minimal binary distribution
# Flags: automate-git.py --help (CEF master tools/automate/automate-git.py, as the CEF docs say to use).
set -euo pipefail
BRANCH=8078
# A git fetch from googlesource can stall with the connection open and nothing arriving (seen 2026-11-03: 4 KB in,
# then silence for 20+ minutes). Under 1000 bytes/s for 10 minutes now fails the fetch instead of hanging forever.
git config --global http.lowSpeedLimit 1000
git config --global http.lowSpeedTime 600
STEP="${1:-}"
A=(python3 /build/automate/automate-git.py --download-dir=/build/download --depot-tools-dir=/build/depot_tools
   --branch="$BRANCH" --x64-build --no-chromium-history --no-debug-build)
case "$STEP" in
  download)
    # Chromium comes from Google's official source tarball, not a git fetch from googlesource (which stalled on
    # wolf-server, 2026-11-03). automate-git.py supports this itself: --chromium-archive extracts it, applies CEF's
    # tarball_deps / tarball_gclient patches and checks the hash. The version is CEF's chromium_checkout
    # (cef/CHROMIUM_BUILD_COMPATIBILITY.txt: refs/tags/156.0.8078.80); the hash is Google's published one
    # (chromium-browser-official/chromium-156.0.8078.80.tar.xz.hashes).
    "${A[@]}" --no-build --no-distrib \
      --chromium-archive=/build/chromium-156.0.8078.80.tar.xz \
      --chromium-archive-sha256=5b1a1f3bfdf4fc559db4eddfb4492b72ffcec7ae1a52b30f8b172230788f5740
    ;;
  build)
    # Chromium's own build dependencies, from this checkout so they match the version. Flags are from
    # src/build/install-build-deps.py (Chromium 156): no prompt, no debug symbols, no ARM toolchain or ChromeOS fonts.
    /build/download/chromium/src/build/install-build-deps.sh --no-prompt --no-syms --no-arm --no-chromeos-fonts --no-nacl
    # chromiumembedded.github.io/cef/automated_build_setup.html, Linux 64-bit example.
    export GN_DEFINES="is_official_build=true use_sysroot=true symbol_level=1 is_cfi=false"
    # H.264 / AAC (Facebook, Instagram, most news video): stock CEF leaves them out. The args are Chromium's own:
    # build/config/features.gni:31 proprietary_codecs, third_party/ffmpeg/ffmpeg_options.gni:24 ffmpeg_branding
    # ("Chromium" or "Chrome"; Chrome's FFmpeg carries the extra decoders).
    export GN_DEFINES="$GN_DEFINES proprietary_codecs=true ffmpeg_branding=Chrome"
    export CEF_ARCHIVE_FORMAT=tar.bz2
    # One link at a time keeps the official (ThinLTO) links within memory on a 30 GB desktop that is also in use.
    export GN_DEFINES="$GN_DEFINES concurrent_links=1"
    "${A[@]}" --no-update --force-build --minimal-distrib --client-distrib --build-target=cefsimple --with-pgo-profiles
    ;;
  *) echo "usage: build.sh download|build" >&2; exit 2 ;;
esac
