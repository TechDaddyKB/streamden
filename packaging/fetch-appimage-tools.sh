#!/usr/bin/env bash
# Seed Tauri 2.12.3's Linux x86_64 tool cache with checksum-pinned binaries.
# Upstream's AppImage plugin URL uses a mutable continuous tag: fail on drift.
set -euo pipefail
cache_dir="${XDG_CACHE_HOME:-$HOME/.cache}/tauri"
mkdir -p "$cache_dir"
while read -r digest name url; do
  if [[ ! -f "$cache_dir/$name" ]] || ! echo "$digest  $cache_dir/$name" | sha256sum --check --status; then
    download=$(mktemp "$cache_dir/.den35-download.XXXXXX")
    trap 'rm -f "$download"' EXIT
    curl --fail --location --retry 3 "$url" --output "$download"
    echo "$digest  $download" | sha256sum --check
    chmod 755 "$download"
    mv "$download" "$cache_dir/$name"
    trap - EXIT
  fi
done <<'TOOLS'
f30140a43a0a59e46db21bdefdf749b9e9f2c6946e92afabbacf98b8ae73fb4f AppRun-x86_64 https://github.com/tauri-apps/binary-releases/releases/download/apprun-old/AppRun-x86_64
36a2d7e274d12e1050d0e9ecfe11d339ed54720b2bec464c286d53f8b07f5c62 linuxdeploy-07333c6-x86_64.AppImage https://github.com/tauri-apps/binary-releases/releases/download/linuxdeploy-07333c6/linuxdeploy-x86_64.AppImage
49d6a17160675a6bd1781699aae6bdf7692d98552e02a3671d2183d10547842e linuxdeploy-plugin-appimage.AppImage https://github.com/linuxdeploy/linuxdeploy-plugin-appimage/releases/download/continuous/linuxdeploy-plugin-appimage-x86_64.AppImage
TOOLS
