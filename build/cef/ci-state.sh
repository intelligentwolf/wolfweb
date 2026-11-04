#!/usr/bin/env bash
# Carries WolfWeb's CEF build from one GitHub Actions job to the next (a job may run for at most 6 hours; a
# Chromium build on a free 4-core runner takes far longer). The same approach as ungoogled-chromium-portablelinux
# (.github/scripts/import-cache.sh / export-cache.sh): the build folder goes into a zstd tarball, uploaded as an
# artifact and unpacked by the next job.
#
#   ci-state.sh setup     — free disk space, choose where the build and the tarball live (written to $GITHUB_ENV)
#   ci-state.sh save      — pack the build folder into $STATE_DIR/cef-build.tar.zst.part-NNN
#   ci-state.sh restore   — unpack those pieces (or an older single cef-build.tar.zst) into the build folder
set -euo pipefail

case "${1:-}" in
  setup)
    # Preinstalled toolchains WolfWeb's build doesn't use (the same list ungoogled-chromium removes).
    sudo rm -rf /usr/local/lib/android /usr/local/.ghcup /usr/lib/jvm /usr/lib/google-cloud-sdk /usr/lib/dotnet \
      /usr/share/swift
    # More of the same, from jlumbroso/free-disk-space (action.yml: /usr/share/dotnet, /opt/ghc, the tool cache
    # $AGENT_TOOLSDIRECTORY, docker images). Runners differ: one with a smaller disk ran out of room unpacking the
    # build (cef run 41799911675, build_02). Swap stays: the links need it on a 16 GB runner.
    sudo rm -rf /usr/share/dotnet /opt/ghc "${AGENT_TOOLSDIRECTORY:-/opt/hostedtoolcache}"
    sudo docker image prune --all --force > /dev/null 2>&1 || true
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
    # In 2 GiB pieces, so the next job can delete each piece as soon as it's unpacked (restore): it then needs room
    # for the build plus one piece, not the build plus the whole tarball.
    rm -f "$STATE_DIR"/cef-build.tar.zst*
    tar -C "$WOLFWEB_CEF_ROOT" -cf - . | zstd -T0 -3 -c | split -b 2G -d -a 3 - "$STATE_DIR/cef-build.tar.zst.part-"
    ls -la "$STATE_DIR"
    df -h "$WOLFWEB_CEF_ROOT" "$STATE_DIR"
    ;;
  restore)
    df -h "$WOLFWEB_CEF_ROOT" "$STATE_DIR"
    if [ -f "$STATE_DIR/cef-build.tar.zst" ]; then
      # One tarball (saved before the pieces).
      zstd -d -c "$STATE_DIR/cef-build.tar.zst" | tar -xf - -C "$WOLFWEB_CEF_ROOT"
      rm "$STATE_DIR/cef-build.tar.zst"
    else
      parts=("$STATE_DIR"/cef-build.tar.zst.part-*)
      [ -f "${parts[0]}" ] || { echo "No saved build in $STATE_DIR" >&2; exit 1; }
      # The glob sorts the pieces in order (-d -a 3: part-000, part-001, ...); each is deleted once it's been read.
      for part in "${parts[@]}"; do cat "$part"; rm "$part"; done | zstd -d -c | tar -xf - -C "$WOLFWEB_CEF_ROOT"
    fi
    df -h "$WOLFWEB_CEF_ROOT"
    ;;
  *) echo "usage: ci-state.sh setup|save|restore" >&2; exit 2 ;;
esac
