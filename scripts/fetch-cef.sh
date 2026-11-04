#!/usr/bin/env bash
# Put the CEF runtime that matches the `cef` crate version in Cargo.toml into third_party/cef.
# Uses cef-rs's own export tool so the binary distribution and the Rust bindings always match.
#
# By default it's CEF's standard build (cef-builds.spotifycdn.com). To use WolfWeb's own build instead (H.264/AAC;
# build/cef/build.sh), set WOLFWEB_CEF_ARCHIVE to its minimal distribution, cef_binary_<version>_linux64_minimal.tar.bz2:
# a local file, or an https URL whose <URL>.sha256 holds its SHA-256 (as the cef-<version> releases do).
# export-cef-dir --archive lays it out exactly as it does a downloaded one.
set -euo pipefail
cd "$(dirname "$0")/.."

version="$(sed -n 's/^cef = { version = "=\([^"]*\)".*/\1/p' Cargo.toml)"
if [[ -z "$version" ]]; then
  echo "could not read the cef version from Cargo.toml" >&2
  exit 1
fi
cargo install --locked --version "$version" export-cef-dir --root target/tools

archive="${WOLFWEB_CEF_ARCHIVE:-}"
if [[ -z "$archive" ]]; then
  target/tools/bin/export-cef-dir --force third_party/cef
  echo "CEF $version runtime (standard build) is in third_party/cef"
  exit 0
fi

# The archive must be the CEF this crate binds: its name carries the CEF version (e.g. 156.0.15+gcf671c0+...), and
# the locked crate version's build metadata is that version's first part (Cargo.lock: cef 156.3.0+156.0.15).
cef_version="$(awk '/^name = "cef"$/ { getline; gsub(/"/, "", $3); print $3 }' Cargo.lock)"
cef_version="${cef_version#*+}"
if [[ -z "$cef_version" ]]; then
  echo "could not read the cef crate's CEF version from Cargo.lock" >&2
  exit 1
fi
case "$(basename "$archive")" in
  "cef_binary_${cef_version}+"*_minimal.tar.bz2) ;;
  *) echo "$archive isn't a minimal CEF $cef_version distribution" >&2; exit 1 ;;
esac

if [[ "$archive" == https://* ]]; then
  dir=target/cef-archive
  mkdir -p "$dir"
  local_file="$dir/$(basename "$archive")"
  curl -fsSL --retry 3 -o "$local_file" "$archive"
  expected="$(curl -fsSL --retry 3 "$archive.sha256" | awk '{print $1}')"
  echo "$expected  $local_file" | sha256sum -c -
  archive="$local_file"
elif [[ ! -f "$archive" ]]; then
  echo "no such file: $archive" >&2
  exit 1
fi

# export-cef-dir unpacks next to the output folder and moves the archive's contents into it; give it a copy so the
# original stays put.
mkdir -p third_party # not in a fresh checkout
copy="third_party/$(basename "$archive")"
cp "$archive" "$copy"
target/tools/bin/export-cef-dir --force --archive "$copy" third_party/cef
rm -f "$copy"
echo "CEF $version runtime (WolfWeb's own build, $(basename "$archive")) is in third_party/cef"
