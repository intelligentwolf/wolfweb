#!/usr/bin/env bash
# Carries WolfWeb's CEF build from one GitHub Actions job to the next (a job may run for at most 6 hours; a
# Chromium build on a free 4-core runner takes far longer). The same approach as ungoogled-chromium-portablelinux
# (.github/scripts/import-cache.sh / export-cache.sh): the build folder goes into a zstd tarball, uploaded as an
# artifact and unpacked by the next job.
#
#   ci-state.sh setup     — free disk space, choose where the build and the tarball live (written to $GITHUB_ENV)
#   ci-state.sh save      — pack the build folder into $STATE_DIR/cef-build.tar.zst
#   ci-state.sh restore   — unpack $STATE_DIR/cef-build.tar.zst into the build folder
set -euo pipefail

case "${1:-}" in
  setup)
    # Preinstalled toolchains WolfWeb's build doesn't use (the same list ungoogled-chromium removes).
    sudo rm -rf /usr/local/lib/android /usr/local/.ghcup /usr/lib/jvm /usr/lib/google-cloud-sdk /usr/lib/dotnet \
      /usr/share/swift
    df -h / /mnt 2>/dev/null || df -h /
    # Build on whichever disk has the most room; keep the tarball on the other one when there are two.
    root_free=$(df --output=avail -B1 / | tail -1)
    mnt_free=0
    if mountpoint -q /mnt; then mnt_free=$(df --output=avail -B1 /mnt | tail -1); fi
    if [ "$mnt_free" -gt "$root_free" ]; then
      build=/mnt/cef-build; state=/home/runner/cef-state
    elif [ "$mnt_free" -gt 0 ]; then
      build=/home/runner/cef-build; state=/mnt/cef-state
    else
      build=/home/runner/cef-build; state=/home/runner/cef-state
    fi
    sudo mkdir -p "$build" "$state"
    sudo chown "$(id -u):$(id -g)" "$build" "$state"
    echo "WOLFWEB_CEF_ROOT=$build" >> "$GITHUB_ENV"
    echo "STATE_DIR=$state" >> "$GITHUB_ENV"
    echo "build folder: $build, state tarball: $state"
    ;;
  save)
    tar -C "$WOLFWEB_CEF_ROOT" -cf - . | zstd -f -T0 -3 -o "$STATE_DIR/cef-build.tar.zst"
    ls -la "$STATE_DIR"
    df -h "$WOLFWEB_CEF_ROOT" "$STATE_DIR"
    ;;
  restore)
    zstd -d -c "$STATE_DIR/cef-build.tar.zst" | tar -xf - -C "$WOLFWEB_CEF_ROOT"
    rm "$STATE_DIR/cef-build.tar.zst" # unpacked; make room for this job's own save
    df -h "$WOLFWEB_CEF_ROOT"
    ;;
  *) echo "usage: ci-state.sh setup|save|restore" >&2; exit 2 ;;
esac
