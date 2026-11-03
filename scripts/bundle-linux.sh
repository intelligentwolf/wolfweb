#!/usr/bin/env bash
# Build WolfWeb and assemble a runnable Linux folder (dist/wolfweb-linux-<arch>/). Same as:
#   cargo run -p xtask -- bundle [--release]
set -euo pipefail
cd "$(dirname "$0")/.."
exec cargo run -q -p xtask -- bundle "$@"
