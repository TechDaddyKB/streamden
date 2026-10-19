#!/usr/bin/env bash
# Drive the spike's tray menu over D-Bus exactly like a StatusNotifier host
# (Omarchy's quickshell bar) does: find our SNI item via the watcher, read its
# com.canonical.dbusmenu layout and send a "clicked" event.
# Usage: tray.sh PID            -> print item info + menu labels
#        tray.sh PID LABEL_RE   -> click the first item whose label matches
set -euo pipefail
unset LD_LIBRARY_PATH
pid=$1 want=${2:-}
items=$(busctl --user --json=short get-property org.kde.StatusNotifierWatcher /StatusNotifierWatcher \
  org.kde.StatusNotifierWatcher RegisteredStatusNotifierItems | jq -r '.data[]')
for it in $items; do
  bus=${it%%/*}; path=/${it#*/}
  [[ $path == "/$bus" || $path == "/" ]] && path=/StatusNotifierItem
  owner=$(busctl --user --json=short call org.freedesktop.DBus /org/freedesktop/DBus org.freedesktop.DBus \
    GetConnectionUnixProcessID s "$bus" 2>/dev/null | jq -r '.data[0]') || continue
  [[ $owner == "$pid" ]] || continue
  menu=$(busctl --user --json=short get-property "$bus" "$path" org.kde.StatusNotifierItem Menu | jq -r '.data')
  layout=$(busctl --user --json=short call -- "$bus" "$menu" com.canonical.dbusmenu GetLayout iias 0 -1 0)
  labels=$(jq -c '[.data[1][2][] | .data | {id: .[0], label: (.[1].label.data // "")}]' <<<"$layout")
  if [[ -z $want ]]; then
    id=$(busctl --user --json=short get-property "$bus" "$path" org.kde.StatusNotifierItem Id | jq -r .data)
    status=$(busctl --user --json=short get-property "$bus" "$path" org.kde.StatusNotifierItem Status | jq -r .data)
    echo "{\"item\":\"$it\",\"sni_id\":\"$id\",\"status\":\"$status\",\"menu\":\"$menu\",\"labels\":$labels}"
    exit 0
  fi
  mid=$(jq -r --arg re "$want" '[.[] | select(.label|test($re))][0].id' <<<"$labels")
  [[ $mid == null ]] && { echo "no menu item matching $want: $labels" >&2; exit 1; }
  busctl --user call "$bus" "$menu" com.canonical.dbusmenu AboutToShow i 0 >/dev/null || true
  busctl --user call "$bus" "$menu" com.canonical.dbusmenu Event isvu "$mid" clicked s "" 0
  echo "clicked $mid ($want)"
  exit 0
done
echo "no tray item registered for pid $pid" >&2
exit 1
