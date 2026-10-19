# Spike reports

One file per spike (`S-1.md` … `S-5.md`), written by the agent that ran it. Each one has the results per test
environment, evidence, numbers, and a recommendation. Spike code lives in `spikes/<id>/` and is throwaway.

| Spike | Work item | Question it answers |
|---|---|---|
| S-1 | DEN-2 | Is a Tauri 2 window reliable on Hyprland + NVIDIA (and KDE, Windows)? **E-1: GO with conditions** (branch `agent/claude/DEN-2-s1-tauri-shell`); KDE and Windows **parked until v0.1** (DEN-20) |
| S-2 | DEN-3 | Do overlays, docks and per-widget origins work in OBS on Linux and Windows? **Linux: pass** (no docks on native Wayland; HW accel off on NVIDIA); Windows **parked until v0.1** (DEN-20) |
| S-3 | DEN-4 | Do the QuickJS sandbox and Python worker design meet latency and safety goals? **Pass**; isolated JS worker required (in review) |
| S-4 | DEN-5 | Twitch Device Code Flow + EventSub WebSocket from a local app |
| S-5 | DEN-7 | Which vendor is best at what (bake-off)? |
