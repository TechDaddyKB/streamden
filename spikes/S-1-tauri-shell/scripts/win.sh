#!/usr/bin/env bash
# Print hyprctl client JSON for the spike's windows. Usage: win.sh PID [TITLE_REGEX]
unset LD_LIBRARY_PATH
hyprctl clients -j | jq -c --argjson pid "$1" --arg re "${2:-.}" \
  '.[] | select(.pid==$pid and (.title|test($re))) | {address, title, class, at, size, floating, pinned, fullscreen, xwayland, monitor, workspace: .workspace.name, mapped, hidden, stableId}'
