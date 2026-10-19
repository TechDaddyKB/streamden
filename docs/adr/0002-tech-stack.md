# ADR-0002: Technology stack — Rust engine + Tauri 2 shell + TypeScript web UI

- **Status:** Proposed (accept after spikes S-1 and S-2: DEN-2, DEN-3)
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
- ➕ About 4 MB installers and about 94 MB idle on Linux (vs about 300 MB and 280–590 MB for Electron).
  Native deb/rpm/AppImage/MSI/DMG bundling. A strong engine for sockets, audio and inference.
- ➖ WebKitGTK on Wayland + NVIDIA is fragile (blank windows, "Error 71"). Mitigations: an early GL-context
  quirk, a graphics-compat setting with crash-loop fallback, never unconditional env vars, the UI also served
  to a browser, no WebGL in the main UI.
- ➖ The designer preview (WebKit on Linux) can differ from the OBS output (Chromium/CEF). Mitigation: a
  "preview in OBS/browser" button, plus Playwright Chromium tests.
- ➖ Rust has a learning curve for some agents. Mitigation: small crates, strong CI, generated types.
- **Exit hatches:** the Tauri 3 CEF runtime (Chromium), or Electron with the same TS UI. The Rust engine moves
  unchanged into a sidecar either way.

## Acceptance checklist
- [ ] Spike S-1 passes on Hyprland + NVIDIA (render, tray, multi-window, close and reopen webview)
- [ ] Spike S-2: an overlay from axum renders in an OBS browser source on Linux and Windows
- [x] Q-U1 (UI framework) answered: React + TypeScript (D-018)
- [ ] Owner accepts
