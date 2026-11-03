#!/usr/bin/env bash
# Download the CEF runtime that matches the `cef` crate version in Cargo.toml into third_party/cef.
# Uses cef-rs's own export tool so the binary distribution and the Rust bindings always match.
set -euo pipefail
cd "$(dirname "$0")/.."

version="$(sed -n 's/^cef = { version = "=\([^"]*\)".*/\1/p' Cargo.toml)"
if [[ -z "$version" ]]; then
  echo "could not read the cef version from Cargo.toml" >&2
  exit 1
fi
cargo install --locked --version "$version" export-cef-dir --root target/tools
target/tools/bin/export-cef-dir --force third_party/cef
echo "CEF $version runtime is in third_party/cef"
