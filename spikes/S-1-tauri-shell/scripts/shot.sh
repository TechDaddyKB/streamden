#!/usr/bin/env bash
# Screenshot of ONE spike window via the foreign-toplevel capture (grim -T):
# only that window's own buffer, never other windows, overlays or the screen.
# Usage: shot.sh PID TITLE_REGEX OUT.png   -> prints "WxH colors=N stddev=S"
set -euo pipefail
unset LD_LIBRARY_PATH
here=$(cd "$(dirname "$0")" && pwd)
w=$("$here/win.sh" "$1" "$2" | head -1)
[[ -z $w ]] && { echo "no window"; exit 1; }
grim -T "$(jq -r .stableId <<<"$w")" "$3.full.png"
size=$(magick identify -format '%wx%h' "$3.full.png")
stats=$(magick "$3.full.png" -format '%k %[fx:standard_deviation]' info:)
magick "$3.full.png" -resize '50%' -strip -define png:compression-level=9 "$3"
rm -f "$3.full.png"
echo "$size colors=${stats% *} stddev=${stats#* }"
