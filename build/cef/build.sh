#!/usr/bin/env bash
# WolfWeb's own CEF build (branch 8078 = Chromium 156, the version the `cef` crate in Cargo.toml binds), with
# H.264/AAC. One set of settings for both places it runs:
#
# Locally, in the container from ./Dockerfile with the build folder mounted at /build:
#   build.sh download     — depot_tools, CEF, and Chromium from Google's source tarball (no build)
#   build.sh build        — WolfWeb's patches (patches/), Chromium's build dependencies, build Release, make the
#                           minimal + client distributions
#
# On GitHub Actions (.github/workflows/cef.yml), where a job may run for at most 6 hours, so the build is split:
#   build.sh ci-prepare   — fetch everything, apply WolfWeb's patches, install dependencies, generate the build files
#   build.sh ci-build     — compile for up to CI_BUILD_SECONDS (default 4 h, leaving time to unpack and save the
#                           build within the 6-hour job limit), then stop cleanly; the next job carries on
#   build.sh ci-package   — make the minimal binary distribution (what the `cef` crate downloads)
# Set WOLFWEB_CEF_ROOT to the build folder (default /build).
#
# Flags: automate-git.py --help (CEF tools/automate/automate-git.py at the branch commit below).
set -euo pipefail

ROOT="${WOLFWEB_CEF_ROOT:-/build}"
BRANCH=8078
# CEF's head of branch 8078 when this was set up; its tools/automate/automate-git.py is the one used here.
CEF_COMMIT=cf671c072925b27823a61181152f4f288e8e4306
AUTOMATE_SHA256=94d3ddd1e0d35482132f45dc101a593c9371b6eb2d8ff4af6cfe2734d2e48c6a
# Chromium comes from Google's official source tarball, not a git fetch from googlesource (which stalled on
# wolf-server, 2026-11-03). automate-git.py supports this itself: --chromium-archive extracts it, applies CEF's
# tarball_deps / tarball_gclient patches and checks the hash. The version is CEF's chromium_checkout
# (cef/CHROMIUM_BUILD_COMPATIBILITY.txt: refs/tags/156.0.8078.80); the hash is Google's published one
# (chromium-browser-official/chromium-156.0.8078.80.tar.xz.hashes).
CHROMIUM_VERSION=156.0.8078.80
CHROMIUM_SHA256=5b1a1f3bfdf4fc559db4eddfb4492b72ffcec7ae1a52b30f8b172230788f5740
TARBALL="$ROOT/chromium-$CHROMIUM_VERSION.tar.xz"
SRC="$ROOT/download/chromium/src"
# WolfWeb's changes to Chromium (build/cef/patches/*.patch, made with `diff -ruN a b` against this Chromium version),
# applied to the unpacked source before the build files are generated. Beside this script by default.
PATCH_DIR="${WOLFWEB_CEF_PATCHES:-$(cd "$(dirname "$0")" && pwd)/patches}"
# WolfWeb's logo images (resources/brand), put in place of Chromium's before the build (brand()). Beside this script
# when it's been copied into a build folder (brand/), otherwise in the repository.
here="$(cd "$(dirname "$0")" && pwd)"
if [ -d "$here/brand" ]; then BRAND_DIR="$here/brand"; else BRAND_DIR="$here/../../resources/brand"; fi
BRAND_DIR="${WOLFWEB_BRAND_DIR:-$BRAND_DIR}"
# automate-git.py get_build_directory_name(): Release + _GN_x64 for --x64-build.
OUT_DIR=out/Release_GN_x64
# automate-git.py: --build-target=cefsimple, plus chrome_sandbox on Linux.
TARGETS=(cefsimple chrome_sandbox)

# A git fetch from googlesource can stall with the connection open and nothing arriving (seen 2026-11-03: 4 KB in,
# then silence for 20+ minutes). Under 1000 bytes/s for 10 minutes now fails the fetch instead of hanging forever.
git config --global http.lowSpeedLimit 1000
git config --global http.lowSpeedTime 600

A=(python3 "$ROOT/automate/automate-git.py" --download-dir="$ROOT/download" --depot-tools-dir="$ROOT/depot_tools"
   --branch="$BRANCH" --x64-build --no-chromium-history --no-debug-build)

as_root() { if [ "$(id -u)" = 0 ]; then "$@"; else sudo "$@"; fi; }

fetch_inputs() {
  if [ ! -f "$ROOT/automate/automate-git.py" ]; then
    mkdir -p "$ROOT/automate"
    curl -fsSL -o "$ROOT/automate/automate-git.py" \
      "https://raw.githubusercontent.com/chromiumembedded/cef/$CEF_COMMIT/tools/automate/automate-git.py"
  fi
  echo "$AUTOMATE_SHA256  $ROOT/automate/automate-git.py" | sha256sum -c -
  if [ ! -f "$TARBALL" ]; then
    curl -fsSL --retry 5 -C - -o "$TARBALL" \
      "https://commondatastorage.googleapis.com/chromium-browser-official/chromium-$CHROMIUM_VERSION.tar.xz"
  fi
}

download() {
  "${A[@]}" --no-build --no-distrib --chromium-archive="$TARBALL" --chromium-archive-sha256="$CHROMIUM_SHA256"
}

apply_patches() {
  local p
  for p in "$PATCH_DIR"/*.patch; do
    [ -e "$p" ] || continue
    # Already in (a re-run on the same tree): leave it.
    if (cd "$SRC" && patch -p1 -R --dry-run -s -f < "$p" > /dev/null 2>&1); then
      echo "already applied: $(basename "$p")"
      continue
    fi
    (cd "$SRC" && patch -p1 --forward -s < "$p")
    echo "applied: $(basename "$p")"
  done
}

brand() {
  # Chromium's own window code draws its product logo from these files (ResourceBundle images, which WolfWeb's
  # run-time replacement in src/branding.rs never sees), so WolfWeb's build swaps them for the wolf: every
  # chrome/app/theme/**/chromium/**/product_logo_<N>.png for wolf-<pixels>.png, and the "Chromium" wordmark
  # (product_logo_name_22[_white].png) for WolfWeb's. Matched by actual pixel size; a size WolfWeb has no image
  # for stops the build rather than leaving Chromium's logo in.
  python3 - "$SRC/chrome/app/theme" "$BRAND_DIR" <<'PY'
import pathlib, shutil, struct, sys

theme, brand = pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2])

def size(path):
    head = path.read_bytes()[:24]
    if head[:8] != b"\x89PNG\r\n\x1a\n":
        return None
    return struct.unpack(">II", head[16:24])

wolf = {size(p): p for p in brand.glob("wolf-*.png")}
wordmark = {(kind, size(p)): p for kind in ("dark", "white") for p in brand.glob(f"wordmark-{kind}-*.png")}
done = 0
for target in sorted(theme.rglob("product_logo*.png")):
    if "chromium" not in target.relative_to(theme).parts:
        continue
    name, dims = target.name, size(target)
    if name.startswith("product_logo_name_22"):
        source = wordmark.get(("white" if name.endswith("_white.png") else "dark", dims))
    elif name.endswith("_mono.png"):
        continue  # a one-colour shape for monochrome UI, not the coloured logo
    else:
        source = wolf.get(dims)
    if source is None:
        sys.exit(f"no WolfWeb image {dims[0]}x{dims[1]} for {target.relative_to(theme)}")
    shutil.copyfile(source, target)
    done += 1
print(f"WolfWeb logo in place of Chromium's: {done} files")
PY
}

deps() {
  # Chromium's own build dependencies, from this checkout so they match the version. Flags are from
  # src/build/install-build-deps.py (Chromium 156): no prompt, no debug symbols, no ARM toolchain or ChromeOS fonts.
  as_root "$SRC/build/install-build-deps.sh" --no-prompt --no-syms --no-arm --no-chromeos-fonts --no-nacl
}

gn_defines() {
  # chromiumembedded.github.io/cef/automated_build_setup.html, Linux 64-bit example.
  # symbol_level=0: no debug symbols (WolfWeb doesn't collect crash reports), which keeps the official ThinLTO links
  # small enough for a 16 GB free runner — ungoogled-chromium-portablelinux builds the same way (flags.linux.gn).
  export GN_DEFINES="is_official_build=true use_sysroot=true symbol_level=0 is_cfi=false"
  # H.264 / AAC (Facebook, Instagram, most news video): stock CEF leaves them out. The args are Chromium's own:
  # build/config/features.gni:31 proprietary_codecs, third_party/ffmpeg/ffmpeg_options.gni:24 ffmpeg_branding
  # ("Chromium" or "Chrome"; Chrome's FFmpeg carries the extra decoders).
  export GN_DEFINES="$GN_DEFINES proprietary_codecs=true ffmpeg_branding=Chrome"
  # No concurrent_links: Chromium sets it from free memory for ThinLTO builds and refuses an explicit value
  # (build/toolchain/concurrent_links.gni:175, assert(!use_thin_lto, ...)).
  export CEF_ARCHIVE_FORMAT=tar.bz2
}

case "${1:-}" in
  download)
    fetch_inputs
    download
    ;;
  build)
    apply_patches
    brand
    deps
    gn_defines
    # --chromium-archive here too: automate-git.py only accepts a source tree that isn't a git checkout when it's told
    # the tree came from an archive (it's already unpacked, so nothing is extracted again).
    "${A[@]}" --no-update --force-build --minimal-distrib --client-distrib --build-target=cefsimple --with-pgo-profiles \
      --chromium-archive="$TARBALL" --chromium-archive-sha256="$CHROMIUM_SHA256"
    ;;
  ci-prepare)
    fetch_inputs
    download
    rm -f "$TARBALL" # extracted; the runner's disk is small
    apply_patches
    brand
    deps
    gn_defines
    export PATH="$ROOT/depot_tools:$PATH"
    # automate-git.py's build step generates the build files with cef/tools/gclient_hook.py before building.
    (cd "$SRC/cef" && python3 tools/gclient_hook.py)
    ;;
  ci-build)
    # Each job is a fresh runner: dependencies again, then compile until the time is up. timeout sends SIGINT so
    # the build stops cleanly (finished objects are kept; the next job's build carries on from them).
    deps
    export PATH="$ROOT/depot_tools:$PATH"
    set +e
    (cd "$SRC" && timeout -k 5m -s INT "${CI_BUILD_SECONDS:-14400}" autoninja -C "$OUT_DIR" "${TARGETS[@]}")
    rc=$?
    set -e
    if [ "$rc" = 124 ] && [ "${CI_FINAL:-false}" = true ]; then
      echo "Out of time in the last build job." >&2
      exit 1
    elif [ "$rc" = 124 ]; then
      echo "Time's up for this job; the next one carries on."
      echo "status=running" >> "${GITHUB_OUTPUT:-/dev/null}"
      exit 0
    fi
    [ "$rc" = 0 ] && [ -x "$SRC/$OUT_DIR/cefsimple" ] && echo "status=completed" >> "${GITHUB_OUTPUT:-/dev/null}"
    exit "$rc"
    ;;
  ci-package)
    export PATH="$ROOT/depot_tools:$PATH"
    gn_defines
    # automate-git.py's distrib command for a Release-only minimal build: make_distrib.py --allow-partial
    # --ninja-build --x64-build --minimal.
    (cd "$SRC/cef/tools" && python3 make_distrib.py --output-dir="$SRC/cef/binary_distrib" \
      --allow-partial --ninja-build --x64-build --minimal)
    ls -la "$SRC/cef/binary_distrib"
    ;;
  *) echo "usage: build.sh download|build|ci-prepare|ci-build|ci-package" >&2; exit 2 ;;
esac
