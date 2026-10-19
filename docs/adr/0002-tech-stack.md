# ADR-0002: Technology stack — Rust engine + Tauri 2 shell + TypeScript web UI

- **Status:** **Accepted for Linux-first development** (owner, 2026-10-18, D-035). Full cross-platform acceptance is gated on DEN-20 (KDE + Windows re-validation after the Linux v0.1 deliverable).
- **Date:** 2026-10-18
- **Research:** [docs/research/tech-stack.md](../research/tech-stack.md)

## Context
We need a cross-platform desktop app (Windows, Linux deb/rpm/AUR on Wayland + NVIDIA, macOS) that runs a
long-lived, low-overhead automation engine next to OBS and games. It needs many concurrent sockets, a local
HTTP/WS server for overlays and docks, audio, TTS/STT, MIDI and hotkeys, embedded JS and Python scripting, and a
web-tech WYSIWYG designer. It must be maintainable by Claude, Codex and Grok agents.

## Decision (proposed)
- **Engine:** Rust (Tokio) library `den-core`, **independent of Tauri**. It also builds as a headless daemon,
  `streamdend`.
- **Desktop shell:** Tauri 2 (thin). The webview is closed when minimised to tray, and the engine keeps running.
- **UI:** TypeScript + Vite + React (decided: D-018). The overlay runtime is light vanilla TS/Preact.
- **Scripting:** JS via `rquickjs` in-process (sandboxed). Python via pooled out-of-process workers over
  JSON-RPC/stdio. C# via an optional .NET 10 worker.
- **Speech:** `sherpa-onnx` (Piper, Kokoro, Whisper and others), OS voices, cloud providers. Audio through `cpal`/`rodio`.
- **Contracts:** Rust types (`serde` + `schemars`) are the single source of truth. Codegen produces TS, Python,
  OpenAPI/AsyncAPI and MCP definitions, with a CI drift check.

## Scored alternatives (from research)
| Option | Weighted score / 5 |
|---|---|
| Tauri 2 + Rust | 4.28 |
| Electron 44 + TS | 4.23 |
| Wails v3 (Go) | 3.83 |
| Qt 6 | 3.68 |
| Avalonia 12 (.NET) | 3.50 |
| Slint | 3.48 |
| Flutter | 3.03 |

## Consequences
- ➕ About 4 MB installers (vs about 300 MB for Electron; research figures). Idle memory is not the
  research's about 94 MB: S-1 measured about 280 MB PSS with the window open, still at the low end of
  Electron's 280–590 MB.
  Native deb/rpm/AppImage/MSI/DMG bundling. A strong engine for sockets, audio and inference.
- ➖ WebKitGTK on Wayland + NVIDIA is fragile (blank windows, "Error 71"). Mitigations: an early GL-context
  quirk, a graphics-compat setting with crash-loop fallback, never unconditional env vars, the UI also served
  to a browser, no WebGL in the main UI.
- ➖ The designer preview (WebKit on Linux) can differ from the OBS output (Chromium/CEF). Mitigation: a
  "preview in OBS/browser" button, plus Playwright Chromium tests.
- ➖ Rust has a learning curve for some agents. Mitigation: small crates, strong CI, generated types.
- **Exit hatches:** the Tauri 3 CEF runtime (Chromium), or Electron with the same TS UI. The Rust engine moves
  unchanged into a sidecar either way.

## Spike evidence (2026-10-18, E-1 = Hyprland + NVIDIA/AMD hybrid)
- **S-1 (DEN-2): Conditional GO for Tauri 2. Out of the box, WebKitGTK is not reliable on Hyprland +
  NVIDIA**, so the graphics-compat ladder ships as a hard, tested requirement with an NVIDIA-aware default.
  (The first run measured a broken driver state, NVIDIA userspace and kernel module mismatched before a reboot,
  and is void for GPU results; see S-1 §0. GPU figures below are from the re-run; the non-GPU memory checks
  are first-run figures that S-1 keeps as valid.)
  - Level 0 (defaults) exits with `Error 71` before first paint on every launch (0/11). NVIDIA explicit sync
    triggers it: level 0 + `__NV_DISABLE_EXPLICIT_SYNC=1` paints 6/6.
  - Level 1 (the early-GL quirk) is the best level on this machine: it painted on all 13 launches in about
    350 ms and renders on NVIDIA with no llvmpipe, at 4.4 % CPU animated, 12 % fullscreen at 5120×1440 and
    0.4 % idle.
  - Levels 2–3 (no DMA-BUF) paint, but WebKit paints on the CPU (28 % fullscreen). Level 4 (X11/XWayland)
    shows a blank window and never paints (0/7) while the process stays alive.
  - The crash ladder rescues a clean install by itself (level 0 crashes, the next start comes up at level 1).
    The tray, close-to-tray with the engine still running, the second window, fullscreen and the browser
    fallback work.

  GO conditions (S-1 §10):
  1. **NVIDIA-aware default.** On Wayland with an NVIDIA GPU, start at the early-GL quirk, after checking that
     NVIDIA EGL actually works. Never ship level 0 as the NVIDIA default. Keep level 0 for other GPUs (untested;
     E-2).
  2. **Ladder:** quirk → `__NV_DISABLE_EXPLICIT_SYNC=1` → no-DMA-BUF → no-DMA-BUF + X11 → browser. Plain X11 is
     not a step (blank on NVIDIA). Bump immediately on a hard crash and after two failures otherwise.
  3. **First-paint watchdog** so a live blank window also bumps the level.
  4. **Broken-driver detection:** if GTK's GL is llvmpipe or NVIDIA reports an API mismatch, skip the quirk and
     tell the user to reboot.
  5. **Shell rules:** closing a webview terminates its web process (`destroy()` alone leaks it: 3 cycles left
     4 web processes, 573 MB PSS while closed; report the leak to wry/Tauri). Two-process model: closed to
     tray the in-process app still held 184 MB PSS / 332 MB RSS, against 52 MB PSS / 92 MB RSS for engine + tray alone, so the engine + tray
     process never loads WebKit and the UI process exits on close. `always_on_top` is ignored on Hyprland, so
     don't promise it on Wayland; ship Hyprland/KWin rules (`pin`) and the OBS-dock chat.
  6. Cap Tokio worker threads in the engine. Watch for `tray-icon` moving to libayatana-appindicator-glib.
- **S-2 (DEN-3), Linux done:** transparent overlays, WebSocket reconnect (0.14–3.2 s), `w-<id>.localhost`
  per-widget origins (isolated, persistent) and CDN loads all pass. Native-Wayland OBS has no custom docks
  (native chat window confirmed), and OBS browser hardware acceleration must be **off** on NVIDIA Linux
  (blank or crash). Public pages can't reach loopback (Chromium LNA), so widgets are always served from our
  server.

## Acceptance checklist
- [x] Spike S-1 on Hyprland + NVIDIA (E-1): Conditional GO with the conditions above
- [ ] Spike S-1 on KDE (E-2) and Windows (E-3): **parked, DEN-20** (D-035)
- [x] Spike S-2 on Linux (E-1)
- [ ] Spike S-2 on Windows (E-3): **parked, DEN-20** (D-035)
- [x] Q-U1 (UI framework) answered: React + TypeScript (D-018)
- [x] Owner accepts for Linux-first development (D-035)
- [ ] Cross-platform acceptance after DEN-20
