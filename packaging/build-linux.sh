#!/usr/bin/env bash
# Compile with the display product name, then override only Linux bundle metadata.
set -euo pipefail
repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_root/apps/desktop"
cargo tauri build --no-bundle -- --locked
cargo tauri bundle --bundles "${BUNDLE_TARGETS:-deb,rpm,appimage}" \
  --config "$repo_root/packaging/tauri-linux-bundle.json"
