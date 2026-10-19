# Streamer.live — Technology Stack Research

> **Status:** research / recommendation (not yet a decision record)
> **Date:** 2026-10-18
> **Scope:** desktop shell, core engine language, embedded scripting (JS / Python / C#), TTS/STT/audio/MIDI, Linux packaging (deb/rpm/AUR/AppImage/Flatpak), CI.
> **Method:** Version numbers come from live registry queries on 2026-10-18 (crates.io API, npm registry, NuGet flat-container API, GitHub Releases API) and from web sources cited inline. "Local evidence" means something checked on the owner's machine. Claims I could not verify are marked **(unverified)**.
> **Related:** streaming platform APIs, competitor/overlay-designer, VTuber integration and plugin SDK research are in separate docs in `docs/research/`. This doc covers them only where they affect the stack choice.

---

## 0. TL;DR

**Recommendation: Rust core + Tauri 2 shell + TypeScript web UI. Build it as a headless-capable daemon from day one.**

- **Engine.** Put it in a plain Rust library crate on Tokio: axum HTTP/WS server, Twitch/OBS/YouTube/Kick clients, action engine, audio, TTS, MIDI. It must not depend on Tauri. Two binaries use it:
  1. the Tauri desktop app (normal use, engine runs in-process);
  2. a headless `streamer-lived` daemon (systemd user service, remote or LAN UI, or the fallback when the webview misbehaves).
- **UI.** One TypeScript web app, served both inside the Tauri webview and by the engine at `http://127.0.0.1:<port>/ui`. Any browser and the OBS dock can open the same UI. Overlays, alerts and the multichat dock are web pages served by the same axum server, so the WYSIWYG designer and the OBS output use the same technology.
- **Scripting.**
  - **JavaScript:** in-process `rquickjs` (QuickJS-NG), with interrupt-handler timeouts and memory limits.
  - **Python:** always out-of-process. A pre-warmed worker pool talks JSON-RPC over stdio. It uses the user's Python or an on-demand `python-build-standalone` download.
  - **C# (optional):** a .NET 10 sidecar that compiles Streamer.bot-style `CPHInline` classes with Roslyn. It only runs if the user has .NET installed.
- **Speech.** Use the official `sherpa-onnx` Rust crate for local neural TTS (Piper/VITS, Kokoro, Kitten, Matcha) and STT (streaming Zipformer, Parakeet, Moonshine, Whisper, VAD, keyword spotting). Add the OS voices (Windows OneCore/SAPI, macOS AVSpeechSynthesizer) and the cloud providers. Every engine returns PCM, which goes to the selected device via `cpal` 0.18 / `rodio` 0.23.

**Runner-up: Electron 44 + TypeScript.** It scores best on Linux webview consistency (Chromium everywhere, native Wayland by default since Electron 38, GlobalShortcuts portal on by default) and on AI-agent familiarity. It loses on idle RAM and installer size, and the app runs next to OBS and games, which is exactly where that matters.

**Biggest risk: WebKitGTK on Wayland + NVIDIA,** which is exactly the owner's setup (high-end NVIDIA RTX + AMD iGPU, NVIDIA open driver, Hyprland, webkit2gtk-4.1 2.52.x). The mitigations, detailed in §4, are:
- an early-GL-context quirk;
- `__NV_DISABLE_EXPLICIT_SYNC` / `WEBKIT_DISABLE_DMABUF_RENDERER` fallbacks behind a setting;
- an engine that never depends on the webview;
- "open UI in browser" as a permanent escape hatch;
- **Tauri 3's new CEF (Chromium) runtime**, which is now in alpha, as a planned exit if WebKitGTK proves unacceptable.

---

## 1. Requirements recap and scoring weights

| # | Criterion | Weight | Why |
|---|---|---|---|
| A | Cross-platform / **Linux quality** (Wayland, NVIDIA, deb/rpm/AUR) | **25%** | The owner runs Hyprland on Arch with NVIDIA; Linux is first-class |
| B | **Performance** (idle CPU/RAM, binary size) | **15%** | Runs next to OBS and games |
| C | **Maintainability by AI agents** (Claude, Codex, Grok) | **20%** | Mainstream typed languages, lots of training data, fast feedback |
| D | **Scripting embed story** (JS + Python, ideally C#) | **15%** | Non-negotiable "Execute Code" sub-action |
| E | **Overlay-designer synergy** | **15%** | Overlays render in OBS browser sources (CEF/Chromium), so web tech is native |
| F | **Packaging** (deb/rpm/AppImage/AUR/Flatpak/MSI/NSIS/DMG, updater, signing, CI) | **10%** | Must ship binaries everywhere with little friction |

---

## 2. Owner's machine (local evidence, 2026-10-18)

Details are generalized for the public repo; the exact inventory was checked locally.

| Item | Value |
|---|---|
| OS | Arch-based rolling distro (Omarchy), current 7.x kernel |
| Session | Wayland, Hyprland compositor (0.5x) |
| GPUs | High-end NVIDIA RTX GPU + AMD integrated GPU (hybrid) |
| NVIDIA driver | NVIDIA open kernel driver (6xx series) with `egl-wayland` / `egl-wayland2` |
| Env | NVIDIA-specific VA/GLX variables set; `GDK_BACKEND=wayland,x11,*`; Electron Ozone hint = wayland |
| WebKitGTK | `webkit2gtk-4.1` 2.52.x at research time (2.54.x a day later) |
| Portals | `xdg-desktop-portal` with the Hyprland and GTK backends |
| Tray libs | `libayatana-appindicator` |
| Audio | PipeWire + WirePlumber |
| Toolchains | Rust stable, Node 26 (mise), Python 3.14/3.15, .NET SDK 10 + 8 |
| Existing WebKitGTK apps installed | a Tauri 2 app (`goxlr-utility-ui`) and a Wails v2 app |

The owner already runs a **Tauri 2 app and a Wails v2 app** on this exact NVIDIA + Hyprland setup. Asking whether `goxlr-utility-ui` renders correctly is the quickest real-world signal for the WebKitGTK risk (§4). GoXLR Utility is also strong prior art for the proposed architecture: a Rust daemon that serves a web UI "directly from the utility to your web browser of choice", with an optional app that "wraps the web UI into a dedicated app", a tray icon, and deb/rpm/AUR packages ([GoXLR Utility README](https://github.com/GoXLR-on-Linux/goxlr-utility)).

---

## 3. Candidate evaluations

### 3.1 Tauri 2 (Rust core + system webview) — **recommended**

| Aspect | Finding |
|---|---|
| Versions | `tauri` **2.12.4** stable (2026-10-14); `wry` 0.57.0 (2026-09-08); `tao` 0.37.1; `@tauri-apps/cli` 2.12.3 ([crates.io/tauri](https://crates.io/crates/tauri), [releases](https://github.com/tauri-apps/tauri/releases)). **Tauri 3.0.0-alpha.5** also shipped 2026-10-14 |
| Tauri 3 alpha | Adds an official **CEF (Chromium) runtime**, `tauri-runtime-cef`, as an alternative to wry on Windows/macOS/Linux. It is "new in the Tauri 3.0 alpha … APIs may still change". Bundles are "hundreds of megabytes larger than with wry". deb/rpm "don't depend on webkit2gtk", AppImage bundles CEF, and tray/menus/plugins "behave the same as on wry" ([v3 CEF docs](https://v3.tauri.app/develop/cef/)). Runtime selection is now by depending on `tauri-runtime-cef` or `tauri-runtime-wry`, and alpha.6 bumps CEF to 154 ([tauri-runtime-cef alpha.6](https://github.com/tauri-apps/tauri/releases/tag/tauri-runtime-cef-v3.0.0-alpha.6)) |
| Webviews | WebView2 (Windows), WKWebView (macOS), **WebKitGTK 4.1** (Linux) — see §4 |
| Size / RAM | Empty-app benchmark (CI): Linux build ≈ **4 MB**, Windows ≈ 3 MB; median process-tree memory in release ≈ **94 MB on Linux**, ≈ 96 MB on macOS, ≈ 307 MB on Windows. Electron is ≈ 296–385 MB on disk and ≈ 279–589 MB of RAM. The Tauri Linux startup figure (≈ 30 s) looks like a headless-CI artefact ([Elanis benchmark](https://github.com/Elanis/web-to-desktop-framework-comparison)). Tauri's own docs say an AppImage grows "from the 2-6 MB range to 70+ MB" ([AppImage docs](https://v2.tauri.app/distribute/appimage/)) |
| Packaging | Built-in bundler: **deb, rpm, AppImage**, MSI (WiX), NSIS, .app/DMG. Official guides for AUR, Flathub and Snap ([AUR](https://v2.tauri.app/distribute/aur/), [Flathub](https://v2.tauri.app/distribute/flatpak/), [RPM](https://v2.tauri.app/distribute/rpm/), [Debian](https://v2.tauri.app/distribute/debian/)). Details in §10 |
| Updater | `tauri-plugin-updater` 2.14.0. Supports **AppImage, MSI, NSIS, macOS `.app.tar.gz`**, not deb/rpm. Signatures are mandatory ("This cannot be disabled") ([updater docs](https://v2.tauri.app/plugin/updater/)) |
| Tray | Built in (`tray-icon` 0.26.1, `muda` 0.21.2). Linux uses libayatana-appindicator / StatusNotifierItem |
| Global hotkeys | `tauri-plugin-global-shortcut` 2.4.1 uses `global-hotkey` 0.9.0 (2026-10-16). Its changelog has **no Wayland/portal support**; only X11 keysym work ([changelog](https://github.com/tauri-apps/global-hotkey/blob/dev/CHANGELOG.md)). A developer reports it "only natively supports X11" ([devlog](https://stardance.hackclub.com/projects/16334/devlogs/13944)). On Wayland, use the portal directly (§5) |
| Audio / MIDI | Native Rust: `cpal` 0.18.2, `rodio` 0.23.0, `midir` 0.11.1 (§9) |
| CI | `tauri-apps/tauri-action` **v1.0.0** (2026-06-29) ([releases](https://github.com/tauri-apps/tauri-action/releases)) |
| Maturity | Apache-2.0, ≈ 112k GitHub stars, very active (local GitHub API query) |
| Verdict | Best fit on performance, packaging and web-overlay synergy. Its weak spot is the Linux webview (§4), and the CEF runtime gives it an exit strategy |

### 3.2 Electron (+ Node/TypeScript) — **runner-up**

| Aspect | Finding |
|---|---|
| Versions | Electron **44.9.0** (2026-10-17); 45 in beta; electron-builder 26.15.3; electron-updater 6.8.9; Electron Forge 8.0.1 (npm registry) |
| Wayland | Electron 38 **removed `ELECTRON_OZONE_PLATFORM_HINT`** and set `--ozone-platform` to auto, so Electron "runs as a native Wayland app in a Wayland session" ([Electron 38 release](https://electronjs.org/blog/electron-38-0), [Wayland tech talk](https://electronjs.org/blog/tech-talk-wayland)) |
| Global hotkeys | "The `GlobalShortcutsPortal` and `GlobalShortcutsPortalPreferredTrigger` features are on by default". This needs a valid reverse-DNS desktop ID, and GNOME prompts for consent the first time ([globalShortcut docs](https://www.electronjs.org/docs/latest/api/global-shortcut)) |
| Size / RAM | ≈ 296 MB build on Linux; ≈ 589 MB median process-tree RAM on Linux and ≈ 279 MB on Windows in release ([benchmark](https://github.com/Elanis/web-to-desktop-framework-comparison)) |
| Packaging | electron-builder targets deb, rpm, AppImage, pacman, snap, flatpak, NSIS, MSI and DMG, plus electron-updater (well-established; target list not re-verified this session) |
| Scripting | JS: `isolated-vm` 7.0.1 for Node 26. It is "**currently in maintenance mode**", its memory limit is "more of a guideline", and it advises running it "in a separate Node process" ([isolated-vm](https://github.com/laverdet/isolated-vm)). Python and C# would need the same sidecars as any other host |
| Prior art | **Firebot** (TypeScript + Electron ^39, GPL-3.0) ([repo](https://github.com/crowbartools/Firebot)) |
| Verdict | Lowest Linux risk and the most AI-friendly stack (TypeScript end to end). Its overlay preview engine matches OBS's CEF. It costs roughly 3–6× the RAM and about 70× the disk of Tauri. Choose it if the WebKitGTK spike fails and Tauri 3 CEF is not ready |

### 3.3 Avalonia UI (.NET 10, C#)

| Aspect | Finding |
|---|---|
| Versions | Avalonia **12.1.4** (2026-10-09); the 11.3.x line is maintained (11.3.22) ([GitHub releases](https://github.com/AvaloniaUI/Avalonia/releases)) |
| Wayland | 12.1 graduated a **native Wayland backend** from private preview. It is "still experimental and is not picked up by automatic platform detection" and needs the `Avalonia.Wayland` package plus `UseWayland()` ([12.1 blog](https://avaloniaui.net/blog/release-12-1), [NuGet Avalonia.Wayland](https://www.nuget.org/packages/avalonia.wayland/)). By default it still runs through XWayland |
| Scripting | Strongest in-process story: Jint **4.17.0**, ClearScript.V8 7.5.1.1, pythonnet 3.2.1, Roslyn Scripting 5.9.0 (NuGet). C# scripts run natively, which matters for Streamer.bot import |
| Overlay synergy | Weak. The designer must be HTML anyway, so an embedded web view is needed. On Linux that means WebKitGTK or CEF again, plus a second UI paradigm (XAML) |
| Packaging | No single bundler for deb/rpm/AppImage/AUR. You combine `dotnet publish` with third-party tools such as Velopack 1.2.161 for updates ([NuGet](https://www.nuget.org/packages/velopack)) |
| Owner fit | The owner is "not a fan of C#" (project memory) |
| Verdict | A credible engine; a poor fit for the web designer and the owner's preference |

### 3.4 Wails (Go + system webview)

| Aspect | Finding |
|---|---|
| Versions | v2 **2.17.0** (2026-10-18); **v3.0.0-beta.32** (2026-10-18) ([releases](https://github.com/wailsapp/wails/releases)) |
| Linux | The v3 beta uses **GTK4 + WebKitGTK 6.0** by default, with GTK3/WebKit2GTK 4.1 "opt-in legacy" until v3.1 ([status](https://docs-preview.wails.io/status/)). The **same WebKitGTK/NVIDIA risk** as Tauri, with **no CEF exit** |
| Packaging | I couldn't confirm native deb/rpm/AppImage output for v3 (unverified) |
| Scripting | Go JS engines (goja) cover ES5.1 plus partial ES2015+ (from memory, not re-verified). Python/C# need sidecars either way |
| Verdict | Go is very agent-friendly, but v3 is still beta, has the same webview risk, and has a weaker packaging story |

### 3.5 Flutter desktop

| Aspect | Finding |
|---|---|
| Versions | **3.44.8** stable (2026-07-23) still has **no public multi-window API**. Windowing classes are `@internal` and gated behind a main-channel flag ([startdebugging.net, Aug 2026](https://startdebugging.net/2026/08/how-to-enable-multi-window-support-in-a-flutter-desktop-app/)); Canonical's multi-window effort reached Windows first ([Canonical](https://ubuntu.com/blog/multiple-window-flutter-desktop)) |
| Linux | GTK3 embedder |
| Overlay synergy | Poor. Overlays must be HTML for OBS, so the designer would be a non-WYSIWYG translation layer |
| Scripting | Dart has no mainstream embeddable JS or Python; everything would be sidecars |
| Verdict | Not suitable |

### 3.6 Qt 6 (C++/QML) and Slint (Rust)

| Aspect | Qt 6 | Slint |
|---|---|---|
| Versions | 6.11.x current; Qt WebEngine 6.11 is based on **Chromium 140**; from 6.12 WebEngine ships as a separate product, "6.140.0" ([Qt blog](https://www.qt.io/blog/qt-webengine-will-release-separate), [Chromium versions](https://wiki.qt.io/QtWebEngine/ChromiumVersions)) | **1.18.1** (2026-09-21) ([releases](https://github.com/slint-ui/slint/releases)) |
| Wayland | Excellent, native | Native via winit; has a `SystemTrayIcon` element ([docs](https://docs.slint.dev/latest/docs/slint/reference/window/systemtrayicon/)) |
| Web / designer | QtWebEngine is large but Chromium-based | **No webview**, so the overlay designer can't be WYSIWYG HTML |
| License | LGPL-3/GPL-3/commercial | GPLv3 / royalty-free desktop / commercial ([Slint desktop post](https://slint.dev/blog/making-slint-desktop-ready)) |
| AI agents | C++ memory-safety burden; QML has less training data | Slint DSL has little training data |
| Verdict | Excellent native UI, but it breaks overlay synergy and AI maintainability | Same problem |

### 3.7 "Headless core daemon + web UI" (architecture, not a toolkit)

The engine runs as a process with no UI dependency. Any web client works: an embedded webview, a normal browser tab, an OBS custom dock, or a phone on the LAN.
- **Pros:**
  - It takes the webview out of the reliability path. A blank WebKitGTK window never stops alerts, TTS or chat.
  - It enables remote control, stream-PC/gaming-PC split setups, and an OBS dock UI for free.
  - It makes the API-first goal real, because the official UI uses the same public HTTP/WS API as plugins.
  - It matches GoXLR Utility's proven model (§2).
- **Cons:**
  - Lifecycle management (single instance, autostart, tray).
  - Local-server security (auth tokens, Origin/Host checks; see §8).
  - Slightly more complex packaging if shipped as a separate service.
- **Recommendation:** adopt it as the internal architecture inside the Tauri app (§11). The Tauri binary embeds the core by default, and a separate `streamer-lived` binary runs the same core headless.

---

## 4. Linux webview deep dive: WebKitGTK on Wayland + NVIDIA

### 4.1 Known failure modes (2025–2026)

Tauri's official **Linux Graphics Issues** page says NVIDIA GPUs are the usual cause, where "WebKitGTK and the graphics driver disagree" ([v2.tauri.app/develop/debug/linux-graphics](https://v2.tauri.app/develop/debug/linux-graphics/)). Symptoms:
- blank or white window;
- flicker on resize;
- silent crash on resize;
- `AcceleratedSurfaceDMABuf was unable to construct a complete framebuffer`;
- `Gdk-Message: Error 71 (Protocol error) dispatching to Wayland display`.

The page's root-cause statement is that the DMABUF renderer requests buffer formats the NVIDIA driver does not provide.

Official workarounds, in order. The earlier ones keep hardware acceleration ([same page](https://v2.tauri.app/develop/debug/linux-graphics/)):
1. `nvidia_drm.modeset=1` (drivers older than 545).
2. `__NV_DISABLE_EXPLICIT_SYNC=1`, which "often fixes the Wayland `Error 71` crash without a performance cost".
3. `WEBKIT_DISABLE_DMABUF_RENDERER=1`, which fixes both errors but disables the fast path.
4. `WEBKIT_DISABLE_COMPOSITING_MODE=1` as a last resort.

The page warns: "Only ship an unconditional override like this if you have verified your app is affected."

The same page also covers **silent degradation**: WebGL/canvas can fall back to a slow path with no error. WebKitGTK masks the GPU renderer string, so the app can't detect it. The page recommends a non-WebGL fallback on Linux. This matters for the designer: **build it with DOM/CSS, not WebGL.**

Recent field reports:
- **Aug 2026, Arch/CachyOS, RTX 5080, Wayland:** blank gray Tauri window. It needed both `WEBKIT_DISABLE_DMABUF_RENDERER=1` and `GDK_BACKEND=x11` ([voicebox#1029](https://github.com/jamiepine/voicebox/issues/1029)).
- **Cline desktop** sets `WEBKIT_DISABLE_DMABUF_RENDERER` in `main()` before creating the webview, to fix "blank window on Linux with the NVIDIA driver" ([cline#14687](https://github.com/cline/cline/pull/14687)).
- **Counter-signal, Oct 14 2026:** with **WebKitGTK 2.54.1 + Mutter 51**, *forcing* the DMABUF override **caused** repaint corruption in a frameless transparent window, and removing it fixed the problem ([folio#13](https://github.com/liminal-hq/folio/pull/13)). An unconditional env-var override is therefore not safe across WebKitGTK versions.
- WebKit/Tauri white screens are also reported on NVIDIA DGX Spark ([NVIDIA forum](https://forums.developer.nvidia.com/t/webkit-tauri-application-white-screen-on-dgx-spark-gpupermission-issues/348741)).

### 4.2 Best current mitigation: the early-GL-context quirk

`tauri-plugin-wayland-nvidia-quirk` explains the root cause. GTK decides per frame whether to draw with GL or into shared memory. WebKitGTK creates its GL context only during the first draw, so frame 1 is a shared-memory frame. The NVIDIA driver enables explicit sync once the EGL surface exists, and the shm buffer that GTK then attaches "has no acquire point", so the compositor drops the connection.

The plugin creates the GL context before the first frame. It keeps **both** DMA-BUF and explicit sync on, sets **no env vars**, and only activates on Wayland + vendor `0x10de` + `nvidia` driver. A fallback, `GDK_GL=always`, costs about 5% peak RSS ([plugin README](https://github.com/arsalan-anwari/tauri-plugin-wayland-nvidia-quirk)).

### 4.3 Mitigation plan for Streamer.live (ordered)

1. **The engine never depends on the webview.** If the UI window fails, alerts, TTS, chat and overlays keep working, and the tray offers "Open UI in browser" (`http://127.0.0.1:<port>/ui`).
2. **Apply the early-GL-context quirk on NVIDIA + Wayland**, either by adopting that plugin or by reimplementing its roughly 100 lines in our shell crate.
3. **A "Graphics compatibility" setting and CLI flags.** Options: `auto` (quirk), `nv-explicit-sync-off` (`__NV_DISABLE_EXPLICIT_SYNC=1`), `no-dmabuf` (`WEBKIT_DISABLE_DMABUF_RENDERER=1`), `no-compositing`, `x11` (`GDK_BACKEND=x11`). Persist the choice in a small pre-GTK config that is read in `main()` before any GTK init. Never hard-code an override (see folio#13).
4. **Crash-loop detection.** If the window process dies twice within N seconds of startup, restart with the next compatibility level and tell the user.
5. **Keep the UI GPU-light.** No WebGL in the main app; use CSS transforms for the designer. Overlays render in OBS's CEF, not in our webview.
6. **Escape hatch: the Tauri 3 CEF runtime.** If WebKitGTK is unacceptable once 3.0 is stable, ship a `-cef` Linux variant: a `streamer-live-cef` AUR package and a CEF AppImage. The cost is hundreds of MB ([v3 CEF docs](https://v3.tauri.app/develop/cef/)). Keep Tauri-specific code thin so the 2→3 migration is cheap.
7. **Day-1 spike on the owner's machine.** Build a Tauri 2.12 window with a Konva/DOM drag-and-drop test plus a 60 fps CSS animation, and measure it on Hyprland + NVIDIA with and without each mitigation. Also note whether `goxlr-utility-ui` (Tauri 2.2.3) and `aether` (Wails v2) currently render correctly there.

### 4.4 Other WebKitGTK facts that matter

- Build on the **oldest base you support that has WebKitGTK 4.1**. Tauri names **Ubuntu 22.04 / Debian 12** as suitable baselines, because glibc breaks backward compatibility ([AppImage/Debian/RPM docs](https://v2.tauri.app/distribute/appimage/)).
- Audio and video *inside the webview* on Linux go through GStreamer. Our audio goes through Rust/cpal instead, so we don't need `bundleMediaFramework`. That keeps the AppImage small and avoids GStreamer licensing questions ([AppImage docs](https://v2.tauri.app/distribute/appimage/)).
- The designer previews in WebKitGTK/WebView2/WKWebView, but overlays render in OBS's **CEF**. OBS 33 (rc2 on 2026-10-16) jumps CEF "from version 127 to 150" ([linuxcompatible.org](https://www.linuxcompatible.org/story/obs-studio-330rc1-released-major-plugin-rework-and-cef-upgrade-lead-the-way/), [OBS releases](https://github.com/obsproject/obs-studio/releases)). Mitigations: an "Open preview in browser" button, an OBS-dock preview, and Playwright/Chromium visual tests for overlay templates in CI.

---

## 5. Global hotkeys on Wayland

- **The portal.** Implemented by the **GNOME, KDE and Hyprland** portal backends, but not by the gtk, wlr, xapp or cosmic backends ([xfwl4 issue summarising support](https://gitlab.xfce.org/xfce/xfwl4/-/issues/93)). The app registers shortcut IDs with descriptions and preferred triggers; the user or compositor owns the actual binding.
- **Hyprland.** With **xdg-desktop-portal-hyprland**, users list app shortcuts with `hyprctl globalshortcuts` and bind them with the `global` dispatcher, e.g. `bind = SUPER SHIFT, A, global, streamerlive:scene1`. Newer wikis also show a Lua form. The wiki notes "this function will only work with XDPH" ([Hyprland wiki: global binds](https://wiki.hypr.land/configuring/core/binds/globals/)). The owner has XDPH 1.4.1 installed.
- **Rust.** `ashpd` **0.13.13** (2026-07-17) wraps `org.freedesktop.portal.GlobalShortcuts` ([crates.io/ashpd](https://crates.io/crates/ashpd)). `global-hotkey` / `tauri-plugin-global-shortcut` cover Windows, macOS and X11 only (§3.1).
- **Design.** A `HotkeyBackend` trait with these implementations:
  1. `global-hotkey` on Windows, macOS and X11;
  2. `ashpd` GlobalShortcuts on Wayland;
  3. always: `streamer-live trigger <action-id>` CLI, plus an HTTP `POST /api/v1/actions/{id}/trigger` with a token. Any compositor, Stream Deck, or script can bind to these;
  4. optional, off by default: an evdev reader for `/dev/input`. It needs the `input` group and is the only way to get "listen-only" hotkeys on Wayland. It also needs `--device=input` in Flatpak ([Flatpak permissions](https://docs.flatpak.org/en/latest/sandbox-permissions.html)).

Electron gets the portal for free (§3.2). With Tauri we write about 200 lines of `ashpd` glue.

---

## 6. Scored comparison

Scores are 1–5 (5 = best). Weighted total = Σ(score × weight).

| Criterion (weight) | **Tauri 2 (+headless core)** | Electron 44 | Avalonia 12 | Wails v3 | Qt 6 / QML | Slint | Flutter |
|---|---|---|---|---|---|---|---|
| A. Linux / cross-platform (25%) | 3.5 — WebKitGTK/NVIDIA risk, mitigations and CEF exit (§4) | **4.5** — bundled Chromium, native Wayland, portal hotkeys | 3 — Wayland opt-in and experimental | 3 — same WebKitGTK risk, beta | **4.5** | 4 | 3 |
| B. Performance (15%) | **5** — ≈ 4 MB, ≈ 94 MB RAM (Linux) | 2 — ≈ 300 MB, ≈ 280–590 MB RAM | 4 | 4.5 | 4.5 | **5** | 4 |
| C. AI-agent maintainability (20%) | 4 — Rust + TS; strict compiler, slightly harder for agents | **5** — TypeScript end to end | 4 — C# is mainstream; owner dislikes it | 4.5 — Go is simple | 3 — C++/QML | 3 — Slint DSL | 3.5 — Dart |
| D. Scripting embed (15%) | 4 — rquickjs in-process, Python/C# sidecars | 4 — isolated-vm in maintenance mode, sidecars | **4.5** — Jint/ClearScript, native C# | 3 — goja, sidecars | 3.5 — QJSEngine, sidecars | 4 — same as Tauri | 2.5 |
| E. Overlay-designer synergy (15%) | **5** — same web stack as overlays | **5** — and same engine as OBS CEF | 2.5 | **5** | 3 | 1.5 | 2 |
| F. Packaging (10%) | **5** — deb/rpm/AppImage/MSI/NSIS/DMG built in, AUR/Flathub guides, tauri-action | 4.5 | 3 | 3 | 3 | 3 | 3 |
| **Weighted total** | **4.28** | **4.23** | 3.50 | 3.83 | 3.68 | 3.48 | 3.03 |

**Sensitivity.**
- If performance drops to 10% and Linux rises to 30%, Electron edges ahead (≈ 4.35 vs 4.20).
- If the Day-1 spike shows the WebKitGTK problems are fully mitigated on the owner's machine (A → 4), Tauri leads clearly (≈ 4.40).

**Decision rule:** run the §4.3 spike first. If it passes, choose Tauri 2. If it fails and Tauri 3 CEF isn't usable yet, choose Electron. The architecture in §11 keeps the engine portable, so a later swap mostly affects the shell.

**AI-agent note.** Rust is not a blocker for frontier agents. A 2026 multi-language agent benchmark found "none of the failures was a type error" across 7 languages, with frontier models passing all eligible attempts. Rust used more tokens (≈ 188k vs ≈ 148k for TypeScript), and **provided tests mattered more than language** (24 vs 16 correct artifacts) ([akitaonrails, Oct 2026](https://akitaonrails.com/en/2026/10/11/are-there-better-languages-for-programming-with-llms/)). Repository-level Rust work is harder: agents resolve up to 21.2% on Rust-SWE-bench ([arXiv 2602.22764](https://arxiv.org/html/2602.22764v1)). The mitigations are strict CI (clippy `-D warnings`, `cargo nextest`, `vitest`, Playwright), small crates with clear boundaries, and generated TS types from Rust (§11.4). TypeScript 7.0 now ships a Go-native compiler that is about 8–12× faster ([InfoQ](https://infoq.com/news/2026/08/typescript-7-released/)), which speeds up agent iteration loops on the UI.

---

## 7. Embedded scripting runtimes

### 7.1 Options (verified versions)

| Runtime | Host | Version (date) | Notes |
|---|---|---|---|
| **rquickjs** (QuickJS-NG) | Rust | **0.14.0** (2026-09-18); QuickJS-NG 0.17.0 | "High level bindings of the **QuickJS-NG** JavaScript engine"; "feature complete, mostly stable" ([README](https://github.com/DelSkayn/rquickjs)). The `Runtime` has `set_interrupt_handler` ("If it returns `true`, the interpreter raises an uncatchable exception"), `set_memory_limit` (no-op with a custom allocator), `set_max_stack_size`, `memory_usage` and `set_loader` ([docs.rs 0.14.0](https://docs.rs/rquickjs/latest/rquickjs/struct.Runtime.html)). Async Rust integration and ES modules. Adds about 1–2 MB (estimate) |
| boa_engine | Rust | **0.22.0** (2026-08-28) | Pure Rust; "more than 90% of the latest ECMAScript specification" ([README](https://github.com/boa-dev/boa), [conformance](https://boajs.dev/conformance)). Slower than QuickJS; still pre-1.0 |
| deno_core / V8 | Rust | deno_core **0.412.0**, `v8` **152.2.0** | Prebuilt static V8 lib is ≈ **40 MB gzipped** per target ([rusty_v8 releases](https://github.com/denoland/rusty_v8/releases)). The Deno runtime-only binary `denort` is a ≈ 34 MB zip ([Deno v2.9.7](https://github.com/denoland/deno/releases)). Expect **+25–40 MB** to our binary and long link times. `rustyscript` 0.12.3 was last released 2025-08 |
| Wasm sandbox (wasmtime + QuickJS / Javy / Extism) | Rust | wasmtime 49.0.3, extism 1.41.0, javy 8.1.0 | Strongest isolation (fuel/epoch interruption, memory caps), but every host API is a hand-written import. Better suited to third-party plugins than user scripts |
| isolated-vm | Node | 7.0.1 (Node 26) | Maintenance mode; "run in a separate Node process"; memory limit is a guideline ([README](https://github.com/laverdet/isolated-vm)) |
| Jint / ClearScript | .NET | 4.17.0 / 7.5.1.1 | Only relevant if the core were .NET |
| **CPython via pyo3** (in-process) | Rust | pyo3 **0.30.1** (2026-10-14), CPython ≥ 3.9, free-threaded wheels ([README](https://github.com/PyO3/pyo3)) | Embedding links libpython. Distro Python ABI and version mismatch (owner has 3.14; Ubuntu 22.04 has 3.10), the GIL, and **no possible sandbox in-process**: one segfaulting C extension kills the engine. **Not recommended in-process** |
| **python-build-standalone** | any | release **20261013**; 3.14.8 `install_only_stripped`: **36.3 MB** Linux x64, **23.2 MB** Windows x64, **26.7 MB** macOS arm64 | Maintained by **Astral**; the same builds `uv` (0.13.1) uses for `uv python install` ([releases](https://github.com/astral-sh/python-build-standalone/releases), [uv](https://github.com/astral-sh/uv/releases)) |
| RustPython | Rust | `rustpython-vm` **0.6.0** (2026-10-02) | Pure-Rust interpreter, no CPython C-extension ecosystem (no numpy, etc.) ([repo](https://github.com/RustPython/RustPython)). Users expect real Python, so **no** |
| Roslyn scripting | .NET sidecar | Microsoft.CodeAnalysis.CSharp.Scripting **5.9.0**; dotnet-script 2.0.1 (NuGet) | Runs C# in a .NET process |
| .NET 10 file-based apps | .NET SDK | .NET 10+ | `dotnet run app.cs` / `dotnet app.cs`, `#:package` directives, build caching, stdin piping (`dotnet run -`). **Requires the SDK**, and concurrent runs of the same file can contend on build output ([MS Learn](https://learn.microsoft.com/en-us/dotnet/core/sdk/file-based-apps)). Good for one-shot tools, too slow and SDK-dependent for per-event execution |

**Prior art: n8n "task runners".** n8n runs user JavaScript and Python in separate runner processes that connect to a broker over WebSocket. A launcher starts runners on demand and shuts them down after 15 s idle. Package imports are allowlisted (`N8N_RUNNERS_STDLIB_ALLOW`, `N8N_RUNNERS_EXTERNAL_ALLOW`). In-process "internal mode" is "insecure by design" and deprecated ([n8n docs](https://docs.n8n.io/hosting/configuration/task-runners/)). VS Code's extension host, Jupyter kernels and LSP servers follow the same out-of-process, JSON-RPC pattern.

### 7.2 Recommended design

```
                          ┌──────────────────────────── streamer-core (Rust, Tokio) ───────────────────────────┐
 event ─► action engine ─►│ Execute Code sub-action                                                            │
                          │   ├─ lang=js     ─► JsHost: rquickjs Runtime per script-context, on a dedicated     │
                          │   │                 OS thread pool; interrupt handler = deadline; memory limit;      │
                          │   │                 only capability-scoped `api.*` + `args` injected (no fs/net)    │
                          │   ├─ lang=python ─► WorkerPool<python>  ──stdio JSON-RPC──► python worker process    │
                          │   └─ lang=csharp ─► WorkerPool<dotnet>  ──stdio JSON-RPC──► cs worker (Roslyn)       │
                          └────────────────────────────────────────────────────────────────────────────────────┘
```

**Common contract (all languages).**
- **Input:** `args`, a JSON object holding the current action's variables. This mirrors Streamer.bot `args`.
- **Output:** `{ ok: bool, setArgs: {...}, logs: [...] }`. Returning `false` stops the action, like Streamer.bot's `Execute()` returning `bool`.
- **API object:** `api.chat.send(platform, text)`, `api.obs.setScene(name)`, `api.globals.get/set(name, value, persist)`, `api.actions.run(id, args)`, `api.tts.speak(...)`, `api.http.fetch(...)` (capability-gated), `api.log.info(...)`.
  - In JS these are direct Rust host functions.
  - In Python and C# they are synchronous stubs. Each one sends a JSON-RPC **request back to the core** over the same stdio pipe (bidirectional JSON-RPC 2.0 with LSP-style `Content-Length` framing) and blocks on the reply.
- **Schema.** One JSON Schema/OpenAPI definition of `api.*` generates the TS `.d.ts` (editor IntelliSense in Monaco), the Python stub package (`streamerlive/api.pyi`) and the C# shim. This keeps the three languages in sync and gives agents a single source of truth.

**JavaScript: in-process (default).**
- Startup is effectively instant, cheap enough per event.
- One `rquickjs::Runtime` per *script context*. Contexts are cached per action and recreated on edit.
- **Timeout:** the interrupt handler checks a deadline (default 5 s, configurable per sub-action).
- **Memory:** `set_memory_limit` (e.g. 64 MB), plus `set_max_stack_size`.
- **Async:** run pending jobs for `await api.*` calls until the deadline.
- **TypeScript:** transpile on save with `oxc` or `swc` (Rust crates) (version not verified).
- **Isolation level:** QuickJS has no ambient I/O; scripts get only what we inject. A native QuickJS crash *would* take down the engine, so offer an "isolated" toggle that runs the same JsHost in a worker process (the same binary re-executed with `--worker js`) for untrusted imported scripts.

**Python: always out-of-process.**
- **Provisioning, in order:**
  1. the user-configured interpreter path;
  2. system `python3` ≥ 3.10 (deb/rpm/AUR list it as an *optional* dependency);
  3. "Install managed Python" downloads `python-build-standalone` (≈ 23–36 MB) into the app data dir, checksum-verified, the way `uv` does.
- Per-script dependencies: an optional `requirements` field, installed with `uv pip install` into a per-profile venv.
- **Worker runtime:** a small pure-Python package shipped as a Tauri resource. It reads JSON-RPC from stdin, `exec`s the user code in a fresh module namespace with `args` and `api` injected, and wraps `print` into logs.
- **Pool:** N pre-warmed workers (default 1–2, started lazily on first use and reaped after idle, like n8n). This hides the ≈ 30–80 ms interpreter startup (estimate).
- **Timeouts:** a soft timeout via a `KeyboardInterrupt` raised in the worker; a **hard timeout** (deadline + grace) **kills the process** and the pool respawns it. Crash isolation is free.
- **Resource limits:**
  - Linux: `setrlimit(RLIMIT_AS / RLIMIT_CPU)`, plus optional Landlock or bubblewrap if present.
  - Windows: a **Job Object** with memory and CPU limits and kill-on-close.
  - macOS: rlimits only.
- Be honest in the UI: this is *robustness*, not a security sandbox. User scripts are trusted the way Streamer.bot C# code is; imported scripts from others get a review/consent screen.

**C#: optional sidecar (Streamer.bot compatibility).**
- A small **framework-dependent .NET 10 worker** (`streamer-cs-worker.dll`, a few MB; size not measured), shipped as a resource and run with the user's `dotnet`. The owner has .NET 10 installed.
- It compiles Streamer.bot-style code with **Roslyn** into a collectible `AssemblyLoadContext`, so edits unload cleanly.
- Streamer.bot's model to emulate:
  - class `CPHInline` with `public bool Execute()`;
  - optional `Init()`, which runs "when the code is first compiled, including at Streamer.bot startup";
  - optional `Dispose()`, which runs when the code is destroyed or the app shuts down ([lifecycle methods](https://docs.streamer.bot/api/csharp/guide/lifecycle-methods));
  - arguments via `args` and `CPH.TryGetArg("rawInput", out string rawInput)`;
  - globals via `CPH.GetGlobalVar<T>(name, persist)` / `CPH.SetGlobalVar(name, value, persist)` ([arguments & variables](https://docs.streamer.bot/api/csharp/guide/variables)).
- Our `CPH` shim maps these to `api.*` JSON-RPC calls. Compile once and keep the worker warm; per-call latency is then IPC-bound (sub-ms to a few ms).
- If `dotnet` is missing, show "C# support requires the .NET 10 runtime" with a link. Don't bundle it: a self-contained runtime would add tens of MB for a feature the owner calls optional.
- Not recommended for C#: `dotnet run app.cs` per execution, because it needs the SDK and has build latency and contention ([MS Learn](https://learn.microsoft.com/en-us/dotnet/core/sdk/file-based-apps)).

**Why not in-process Python or a V8 core?** In-process CPython can't be sandboxed or killed safely, and it ties us to a libpython ABI per distro. V8 adds +25–40 MB and slow builds for little gain over QuickJS for short glue scripts. Revisit V8, via `deno_core` in a worker process, only if users need full npm compatibility.

---

## 8. Overlay / alert designer and the local server (stack implications)

- **Rendering model.** Overlays are HTML pages served by the core's axum server at `http://127.0.0.1:<port>/o/<overlay-id>`. They take live data over a WebSocket on the **same origin**. A tiny vanilla-TS (or Preact) overlay runtime renders a JSON scene graph, which keeps OBS browser-source CPU low. The designer edits the same JSON scene graph in the app UI.
- **Editor libraries** (npm, 2026-10-18; see the separate overlay-designer doc for the full survey):
  - DOM-based editing fits HTML overlays best: absolutely-positioned elements with transform handles. `moveable` (MIT) was last published in 2023, which is stale; `interactjs` 1.10.28 (MIT).
  - Canvas options: `konva` 10.9.0 (MIT) and `fabric` 7.4.0 (MIT).
  - `grapesjs` 0.23.8 (BSD-3-Clause) is a full HTML page builder.
  - **Avoid `tldraw`** ("SEE LICENSE IN LICENSE.md", a commercial license).
  - No WebGL in the editor (§4.1).
- **Chromium Local Network Access (LNA).** Since Chrome 142 (Oct 2025), public origins need permission to reach loopback addresses. Requests within the same address space don't trigger the prompt ([OpenReplay explainer](https://blog.openreplay.com/chrome-local-network-access-lna-permission/)). OBS's CEF integration **"temporarily"** auto-grants `CEF_PERMISSION_TYPE_LOOPBACK_NETWORK` for both browser sources and docks ([obs-browser commit 058c30f](https://github.com/obsproject/obs-browser/commit/058c30f1e6201d08db6af51faaf88f23feab4a3c)), and it recreated the old WebSocket-to-local behaviour for modern CEF ([commit a162443](https://github.com/obsproject/obs-browser/commit/a1624431ae60cd89560d3d12c8143b1b926b410a)). **Design rule:** always serve overlays and docks *from* our loopback server, never from a hosted CDN page that connects back to localhost.
- **Local API security.**
  - Bind `127.0.0.1` by default; LAN access is opt-in.
  - Per-client tokens for control endpoints; overlays get read-only scoped tokens embedded in the URL.
  - Validate `Host` (blocks DNS rebinding) and `Origin` on WebSocket upgrades.
  - Rate-limit trigger endpoints.

---

## 9. TTS, STT, audio and MIDI

### 9.1 Engine options

| Engine | Kind | Status (verified) | License / notes | Fit |
|---|---|---|---|---|
| **sherpa-onnx** (k2-fsa) | Offline TTS + STT + VAD + KWS | **v1.13.8** (2026-09-10). **Official Rust crate `sherpa-onnx` 1.13.8**, published by the k2-fsa maintainer, **static linking by default**, native libs auto-downloaded ([crate](https://crates.io/crates/sherpa-onnx), [Rust examples](https://github.com/k2-fsa/sherpa-onnx/tree/master/rust-api-examples)). Official **Tauri v2 examples** ([tauri-examples](https://github.com/k2-fsa/sherpa-onnx/tree/master/tauri-examples)) | Apache-2.0 (from memory, not re-verified this session). Rust examples cover **Piper VITS, Kokoro, Kitten, Matcha, Pocket TTS, Supertonic, ZipVoice (voice cloning)**; ASR via streaming Zipformer, **Parakeet TDT, Moonshine v2, SenseVoice, Whisper, Qwen3-ASR**, **Silero VAD**, **keyword spotter**, speech enhancement and speaker ID | **Primary local engine** |
| Piper | Offline TTS | Development moved to **OHF-Voice/piper1-gpl**, v1.8.0 (2026-09-04); **GPL-3.0**; Python `piper-tts` plus a C/C++ `libpiper`; "embeds espeak-ng" ([repo](https://github.com/OHF-Voice/piper1-gpl)). Voices: `rhasspy/piper-voices` (tagged MIT on HF, updated 2026-09-17) | Run Piper voices **through sherpa-onnx** rather than linking piper1-gpl. Phonemisation still uses **espeak-ng (GPL-3.0)** (see licensing question, §13) | Voice catalogue |
| Kokoro-82M | Offline neural TTS | **Apache-2.0** ([HF](https://huggingface.co/hexgrad/Kokoro-82M)). ONNX sizes: fp32 325 MB, fp16 163 MB, **q8f16 86 MB**, quantized 92 MB ([onnx-community](https://huggingface.co/onnx-community/Kokoro-82M-v1.0-ONNX)) | High quality; download on demand | "Premium local" voices |
| Windows OneCore / SAPI 5 | OS TTS | Via the `windows` crate. `SpeechSynthesizer.SynthesizeTextToStreamAsync` returns a WAV stream, so it can be routed to any device; SAPI can output to a memory stream ([MS docs](https://learn.microsoft.com/en-us/uwp/api/windows.media.speechsynthesis.speechsynthesizer.synthesizetexttostreamasync)) | Free, many voices. Windows 11 Narrator "natural" voices are only reachable through the unofficial **NaturalVoiceSAPIAdapter**, which "can stop working at any time" ([repo](https://github.com/gexgd0419/NaturalVoiceSAPIAdapter)) | Windows default |
| macOS AVSpeechSynthesizer | OS TTS | `write(_:toBufferCallback:)` yields PCM buffers ([Apple docs](https://developer.apple.com/documentation/avfaudio/avspeechsynthesizer/write(_:tobuffercallback:))) via `objc2` crates | — | macOS default |
| Linux speech-dispatcher | OS TTS | Speaks directly; no PCM capture, so it can't route to a selected device (design inference) | — | Skip; use sherpa-onnx |
| `tts` crate (tts-rs) | Wrapper | **0.26.3, last release 2024-07-06** ([crate](https://crates.io/crates/tts)) | Stale, and speaks directly instead of returning buffers | **Don't use** |
| eSpeak NG | Offline formant TTS | GPL-3.0 | Robotic voice, but "classic" streamer meme voices | Optional via sherpa's espeak data |
| Cloud | TTS | Azure, Polly, Google, ElevenLabs, OpenAI: all REST, returning audio bytes. Speaker.bot supports Google Cloud, Azure, Amazon Polly, Watson, SAPI5, UberDuck, TTS Monster, Acapela, CereProc and ElevenLabs ([speaker.bot](https://speaker.bot/)) | User supplies the keys | Provider plugins |
| whisper.cpp | STT | **v1.9.5** (2026-10-06) ([releases](https://github.com/ggml-org/whisper.cpp/releases)); `whisper-rs` **0.16.0** (2026-03-12) with CUDA/HIP/Metal/**Vulkan**/CoreML features; development moved to Codeberg ([listing](https://openapps.pro/packages/whisper-rs)) | MIT | Dictation-quality STT (optional) |
| ONNX Runtime | ML runtime | `ort` **2.0.0-rc.14** (2026-10-12; still RC) for ONNX Runtime 1.31 ([crate](https://crates.io/crates/ort)) | — | Only if we run custom models outside sherpa-onnx |

**Recommendation.**
- `TtsEngine` trait: `async fn synth(&self, text, voice, params) -> PcmStream`. Implementations: `SherpaTts` (Piper, Kokoro, Kitten), `OneCoreTts` and `SapiTts` (Windows), `AvSpeechTts` (macOS), `AzureTts`, `PollyTts`, `GoogleTts`, `ElevenLabsTts`, `OpenAiTts`.
- Speaker.bot-style features sit above the engine: queue, per-user voice aliases, word replacement and filters, max length, skip/clear, and events.
- **STT:** sherpa-onnx streaming Zipformer or Parakeet with Silero VAD for voice commands, plus the keyword spotter for a wake word. `whisper-rs` is an optional higher-accuracy mode.
- **Models are never bundled.** A model manager downloads them, with checksums, on first use. This keeps packages small; Kokoro alone is 86–325 MB.

### 9.2 Audio output

- **`cpal` 0.18.2** (2026-08-16). Hosts: WASAPI/ASIO (Windows), CoreAudio, ALSA, plus **native `pipewire` and `pulseaudio` backends** as optional features (PipeWire ≥ 0.3.53). PipeWire devices "with multiple ports now get distinct, localized, per-port names", and JACK `DeviceId` is "stable across application restarts" ([README](https://github.com/RustAudio/cpal), [CHANGELOG](https://github.com/RustAudio/cpal/blob/master/CHANGELOG.md)). ALSA dev files are still needed at build time (`libasound2-dev` / `alsa-lib-devel`).
- **`rodio` 0.23.0** (2026-10-11): mixing, decoding and the queue. The next release adds high-quality resampling and a 48 kHz default ([CHANGELOG](https://github.com/RustAudio/rodio/blob/master/CHANGELOG.md)).
- **Design:** one output "bus" per selected device, so TTS, sound alerts and music each pick their own device. Persist the stable device ID and fall back to the default device with a warning. On Linux, enable cpal's `pipewire` feature so device names make sense (users route TTS to a PipeWire null sink or virtual cable for OBS capture).

### 9.3 MIDI

- **`midir` 0.11.1** (2026-10-10). Backends: ALSA, WinMM, CoreMIDI, WinRT (feature), JACK (feature), Web MIDI, Android. Supports virtual ports (except on Windows) and full SysEx ([README](https://github.com/Boddlnagg/midir)).
- Flatpak gets ALSA/MIDI through `--socket=pulseaudio`, which "includes access to `/dev/snd`" ([Flatpak permissions](https://docs.flatpak.org/en/latest/sandbox-permissions.html)).

---

## 10. Packaging specifics

### 10.1 What the Tauri bundler produces

- `tauri build` on Linux produces `.deb`, `.rpm` and `.AppImage` under `src-tauri/target/release/bundle/{deb,rpm,appimage}`.
- Extra dependencies go in `bundle.linux.deb.depends` / `bundle.linux.rpm.depends`; extra files in `bundle.linux.{deb,rpm,appimage}.files`.
- RPM supports pre/post-install scripts, conflicts/provides/obsoletes and a custom desktop template. Signing uses a GPG key in `TAURI_SIGNING_RPM_KEY` (+ `_PASSPHRASE`) ([RPM docs](https://v2.tauri.app/distribute/rpm/), [Debian docs](https://v2.tauri.app/distribute/debian/)).
- **Build on Ubuntu 22.04** (or Debian 12) for a glibc baseline that still has WebKitGTK 4.1 ([AppImage docs](https://v2.tauri.app/distribute/appimage/)).
- `linuxdeploy` can't cross-compile ARM AppImages, so use GitHub's **`ubuntu-22.04-arm` / `ubuntu-24.04-arm`** runners, which are free for public repos since Aug 2025 ([AppImage docs](https://v2.tauri.app/distribute/appimage/), [GitHub changelog](https://github.blog/changelog/2025-08-07-arm64-hosted-runners-for-public-repositories-are-now-generally-available/)).
- Windows: MSI (WiX) + NSIS. macOS: `.app` + DMG. The Tauri 3 CEF variant bundles `libcef` and skips the WebView2 installer ([v3 CEF docs](https://v3.tauri.app/develop/cef/)).

Suggested `tauri.conf.json` fragment (illustrative):

```jsonc
{
  "bundle": {
    "active": true,
    "targets": ["deb", "rpm", "appimage", "nsis", "msi", "dmg", "app"],
    "createUpdaterArtifacts": true,
    "linux": {
      "deb": { "depends": ["libasound2", "libayatana-appindicator3-1"],
               "files": { "/usr/share/metainfo/live.streamer.StreamerLive.metainfo.xml": "../packaging/metainfo.xml" } },
      "rpm": { "depends": ["alsa-lib", "libayatana-appindicator-gtk3"] },
      "appimage": { "bundleMediaFramework": false }
    }
  },
  "plugins": { "updater": { "pubkey": "<minisign public key content>", "endpoints": ["https://github.com/<org>/streamer-live/releases/latest/download/latest.json"] } }
}
```

Python and .NET are **optional runtime features**. Don't make them hard package dependencies; use deb `Suggests`, rpm `Recommends` and pacman `optdepends`.

### 10.2 Updater strategy per format

| Install type | Update mechanism |
|---|---|
| Windows NSIS/MSI, macOS .app, Linux **AppImage** | `tauri-plugin-updater` (signed; static `latest.json` on GitHub Releases) ([updater docs](https://v2.tauri.app/plugin/updater/)) |
| **deb / rpm** | The package manager. Detect the install type at runtime, disable self-update, and show an "update available" notice instead. Later, host repos: an APT repo or Launchpad PPA, a Fedora **COPR** or openSUSE Build Service (suggestions, unverified) |
| **AUR** | The AUR helper. Bump `pkgver` from CI (§10.3) |
| Flatpak | Flathub |

### 10.3 AUR packages

The base dependency set comes from Tauri's AUR guide: `cairo desktop-file-utils gdk-pixbuf2 glib2 gtk3 hicolor-icon-theme libsoup pango webkit2gtk-4.1`, `options=('!strip' '!debug')`, and an `.install` hook that refreshes the icon cache and desktop database. The from-source example adds `makedepends=('git' 'openssl' 'appmenu-gtk-module' 'libappindicator-gtk3' 'librsvg' 'cargo' 'pnpm' 'nodejs')` and builds with `pnpm tauri build -b deb`, then copies `bundle/deb/*/data/*` into `$pkgdir` ([v2.tauri.app/distribute/aur](https://v2.tauri.app/distribute/aur/)). Our additions are `alsa-lib` (cpal/midir), `libayatana-appindicator` (tray, which the owner has) and `optdepends`.

**`streamer-live-bin`** (repackages the CI-built `.deb`):

```bash
# Maintainer: <you>
pkgname=streamer-live-bin
_pkgname=streamer-live
pkgver=0.1.0
pkgrel=1
pkgdesc="Streaming automation hub (actions, TTS, overlays, multichat)"
arch=('x86_64' 'aarch64')
url="https://github.com/<org>/streamer-live"
license=('GPL-3.0-or-later')           # pending licence decision (§13)
depends=('cairo' 'desktop-file-utils' 'gdk-pixbuf2' 'glib2' 'gtk3' 'hicolor-icon-theme'
         'libsoup' 'pango' 'webkit2gtk-4.1' 'alsa-lib' 'libayatana-appindicator')
optdepends=('python: Python Execute-Code support (or use the in-app managed Python)'
            'dotnet-runtime: C# Execute-Code support (Streamer.bot compatibility)'
            'xdg-desktop-portal-hyprland: global shortcuts on Hyprland'
            'pipewire: native PipeWire audio device names')
provides=("$_pkgname")
conflicts=("$_pkgname")
options=('!strip' '!debug')
install=${_pkgname}.install
source_x86_64=("$url/releases/download/v$pkgver/${_pkgname}_${pkgver}_amd64.deb")
source_aarch64=("$url/releases/download/v$pkgver/${_pkgname}_${pkgver}_arm64.deb")
sha256sums_x86_64=('<filled by CI>')
sha256sums_aarch64=('<filled by CI>')

package() {
  bsdtar -xf data.tar.gz -C "$pkgdir"
}
```

**`streamer-live`** (from a release tarball; follows the Arch Rust/Node guidelines: fetch in `prepare()`, build `--frozen`):

```bash
pkgname=streamer-live
pkgver=0.1.0
pkgrel=1
pkgdesc="Streaming automation hub (actions, TTS, overlays, multichat)"
arch=('x86_64' 'aarch64')
url="https://github.com/<org>/streamer-live"
license=('GPL-3.0-or-later')
depends=('cairo' 'desktop-file-utils' 'gdk-pixbuf2' 'glib2' 'gtk3' 'hicolor-icon-theme'
         'libsoup' 'pango' 'webkit2gtk-4.1' 'alsa-lib' 'libayatana-appindicator')
makedepends=('cargo' 'nodejs' 'pnpm' 'git' 'librsvg' 'clang' 'cmake')   # clang/cmake: sherpa-onnx / bindgen if not using prebuilt
optdepends=('python: Python Execute-Code support' 'dotnet-runtime: C# Execute-Code support')
options=('!lto' '!debug')
install=${pkgname}.install
source=("$pkgname-$pkgver.tar.gz::$url/archive/refs/tags/v$pkgver.tar.gz")
sha256sums=('<filled by CI>')

prepare() {
  cd "$pkgname-$pkgver"
  export CARGO_HOME="$srcdir/cargo-home"
  cargo fetch --locked --target "$(rustc -vV | sed -n 's/host: //p')" --manifest-path src-tauri/Cargo.toml
  pnpm install --frozen-lockfile
}

build() {
  cd "$pkgname-$pkgver"
  export CARGO_HOME="$srcdir/cargo-home" CARGO_TARGET_DIR=target
  pnpm tauri build --bundles deb -- --frozen        # deb layout is the simplest to copy from
}

package() {
  cd "$pkgname-$pkgver"
  cp -a target/release/bundle/deb/${pkgname}_${pkgver}_*/data/* "$pkgdir"/
  install -Dm644 LICENSE "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
}
```

**Notes.**
- sherpa-onnx's crate downloads prebuilt native libs at build time by default. AUR builds should pin them with `SHERPA_ONNX_LIB_DIR` or vendor them as a `source=` entry, so the build stays reproducible and offline after `prepare()` (crate supports `SHERPA_ONNX_LIB_DIR`, per its [Rust examples README](https://github.com/k2-fsa/sherpa-onnx/tree/master/rust-api-examples)).
- Generate `.SRCINFO` with `makepkg --printsrcinfo > .SRCINFO` ([Tauri AUR guide](https://v2.tauri.app/distribute/aur/)).
- Automate publishing from the release workflow, e.g. with `KSXGitHub/github-actions-deploy-aur` (widely used; current version not verified).

### 10.4 Flatpak (secondary target)

Tauri's Flathub guide uses `org.gnome.Platform` with `flatpak-node-generator` and `flatpak-cargo-generator.py` for offline sources, builds the deb bundle inside flatpak-builder, and installs the binary, desktop file, icons and metainfo into `/app` ([Tauri Flatpak guide](https://v2.tauri.app/distribute/flatpak/)). The sample manifest pins `runtime-version: '47'`; use the current GNOME runtime.

Our `finish-args`:

```yaml
finish-args:
  - --socket=wayland
  - --socket=fallback-x11
  - --share=ipc
  - --device=dri                 # GL for WebKitGTK
  - --share=network              # Twitch/OBS/YouTube/Kick + local HTTP/WS server for OBS browser sources
  - --socket=pulseaudio          # audio out/in incl. ALSA + MIDI via /dev/snd
  - --talk-name=org.kde.StatusNotifierWatcher   # tray icon
  - --filesystem=xdg-music:ro    # sound alerts library (or use the FileChooser portal)
  # optional / avoid on Flathub unless justified:
  # - --device=input             # evdev hotkey fallback (Flatpak ≥ 1.15.6)
```

Sources: [Flatpak sandbox permissions](https://docs.flatpak.org/en/latest/sandbox-permissions.html). `--share=network` exposes host abstract sockets, `--socket=pulseaudio` covers mic, output, MIDI and ALSA, and `--device=input` needs Flatpak 1.15.6 or newer.

**Flatpak friction:**
- The user's host `python3` and `dotnet` are **not visible** in the sandbox. Use the managed python-build-standalone download into `~/.var/app/<id>/`. C# would need a .NET SDK extension or `flatpak-spawn --host` (`--talk-name=org.freedesktop.Flatpak`), which Flathub reviewers dislike.
- File watching is limited to granted paths.
- **Recommendation:** ship deb, rpm, AUR and AppImage first. Flatpak follows once Python is fully managed.

### 10.5 Code signing

- **Windows.** Choose one:
  - **SignPath Foundation**: free for OSS. Needs an OSI license with no commercial dual-licensing, a fully automated build from the public repo, and a published code-signing policy. The certificate names SignPath Foundation as publisher ([terms](https://signpath.org/terms), [signpath.org](https://signpath.org/)).
  - **Azure Artifact Signing** (renamed from Trusted Signing): Basic plan US$9.99/month for 5,000 signatures ([Azure](https://azure.microsoft.com/en-us/products/artifact-signing)).
- **macOS.** An Apple Developer Program membership (US$99/yr) is required for Developer ID signing and notarization. Unsigned Apple Silicon builds should at least use ad-hoc signing to avoid "damaged app" errors ([Tauri GitHub guide](https://v2.tauri.app/distribute/pipelines/github/)).
- **Linux.** GPG-sign RPMs (`TAURI_SIGNING_RPM_KEY`), sign the release `SHA256SUMS`, and keep the updater minisign key (`TAURI_SIGNING_PRIVATE_KEY`; ".env files do not work") in GitHub secrets ([updater docs](https://v2.tauri.app/plugin/updater/)).

### 10.6 CI (GitHub Actions matrix)

Based on the official Tauri workflow, which uses `actions/checkout@v7`, `actions/setup-node@v6`, `dtolnay/rust-toolchain@stable`, `swatinem/rust-cache@v2` and **`tauri-apps/tauri-action@v1`**, on `ubuntu-22.04`, `ubuntu-22.04-arm`, `windows-latest` and `macos-latest` for both macOS architectures ([Tauri GitHub pipeline docs](https://v2.tauri.app/distribute/pipelines/github/)). We add ALSA and PipeWire headers for cpal and midir.

```yaml
name: release
on:
  push: { tags: ['v*'] }
  workflow_dispatch:
jobs:
  check:                      # fast gate for every PR too (separate ci.yml in practice)
    runs-on: ubuntu-22.04
    steps:
      - uses: actions/checkout@v7
      - run: sudo apt-get update && sudo apt-get install -y libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev patchelf libasound2-dev libpipewire-0.3-dev libdbus-1-dev
      - uses: dtolnay/rust-toolchain@stable
        with: { components: 'clippy,rustfmt' }
      - uses: swatinem/rust-cache@v2
      - uses: actions/setup-node@v6
        with: { node-version: lts/*, cache: pnpm }
      - run: corepack enable && pnpm install --frozen-lockfile
      - run: cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings
      - run: cargo test --workspace && pnpm -r test && pnpm -r typecheck

  build:
    needs: check
    permissions: { contents: write }
    strategy:
      fail-fast: false
      matrix:
        include:
          - { platform: ubuntu-22.04,     args: '' }                                 # deb, rpm, AppImage (x86_64)
          - { platform: ubuntu-22.04-arm, args: '' }                                 # deb, rpm, AppImage (aarch64)
          - { platform: windows-latest,   args: '' }                                 # NSIS + MSI
          - { platform: macos-latest,     args: '--target aarch64-apple-darwin' }
          - { platform: macos-latest,     args: '--target x86_64-apple-darwin' }
    runs-on: ${{ matrix.platform }}
    steps:
      - uses: actions/checkout@v7
      - if: startsWith(matrix.platform, 'ubuntu')
        run: |
          sudo apt-get update
          sudo apt-get install -y libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev patchelf xdg-utils \
                                  libasound2-dev libpipewire-0.3-dev libdbus-1-dev rpm
      - uses: actions/setup-node@v6
        with: { node-version: lts/*, cache: pnpm }
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.platform == 'macos-latest' && 'aarch64-apple-darwin,x86_64-apple-darwin' || '' }}
      - uses: swatinem/rust-cache@v2
        with: { workspaces: './src-tauri -> target' }
      - run: corepack enable && pnpm install --frozen-lockfile
      - uses: tauri-apps/tauri-action@v1
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
          TAURI_SIGNING_PRIVATE_KEY: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}
          TAURI_SIGNING_PRIVATE_KEY_PASSWORD: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY_PASSWORD }}
          TAURI_SIGNING_RPM_KEY: ${{ secrets.RPM_GPG_KEY }}
          # Windows: SignPath / Artifact Signing step; macOS: APPLE_CERTIFICATE, APPLE_ID, APPLE_PASSWORD, APPLE_TEAM_ID
        with:
          tagName: v__VERSION__
          releaseName: 'Streamer.live v__VERSION__'
          releaseDraft: true
          args: ${{ matrix.args }}

  aur:
    needs: build
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
      - run: ./packaging/aur/update-pkgbuild.sh "${GITHUB_REF_NAME#v}"   # fills pkgver + sha256sums from release assets
      - uses: KSXGitHub/github-actions-deploy-aur@v4   # version unverified
        with:
          pkgname: streamer-live-bin
          pkgbuild: ./packaging/aur/streamer-live-bin/PKGBUILD
          commit_username: ${{ secrets.AUR_USERNAME }}
          commit_email: ${{ secrets.AUR_EMAIL }}
          ssh_private_key: ${{ secrets.AUR_SSH_PRIVATE_KEY }}
          commit_message: "Update to ${{ github.ref_name }}"
```

**Additional CI jobs:**
- `makepkg` smoke test in an `archlinux:latest` container.
- Install tests: `.deb` in `debian:12` / `ubuntu:24.04`, `.rpm` in `fedora:latest`.
- Playwright overlay tests in Chromium, which matches OBS's CEF.
- A nightly job against `tauri` 3 alpha with the CEF runtime, to track the escape hatch.

---

## 11. Proposed architecture

### 11.1 Process model

```
┌─────────────────────────────── streamer-live (Tauri 2 app, one OS process) ───────────────────────────────┐
│  Tauri shell (thin): windows, tray, single-instance, autostart, deep links, updater, NVIDIA/Wayland quirk  │
│        │  in-proc channel (or WS to an external daemon when "attach to running streamer-lived")           │
│  ┌─────▼──────────────────────────── streamer-core (Rust lib, Tokio) ─────────────────────────────────┐   │
│  │ EventBus ─► Trigger matcher ─► Action engine (queues, concurrency groups, sub-actions, variables)   │   │
│  │ Integrations: Twitch (EventSub WS, Helix, chat) · OBS ws v5 · YouTube · Kick · X · VTube Studio …   │   │
│  │ Local server (axum): /api (REST+OpenAPI) · /ws (event+RPC) · /ui (SPA) · /o/* overlays · /dock/*    │   │
│  │ Audio buses (cpal/rodio) · TTS service (sherpa-onnx/OS/cloud) · STT (sherpa-onnx VAD+ASR, optional) │   │
│  │ MIDI (midir) · Hotkeys (global-hotkey | ashpd portal | CLI/HTTP) · File watch (notify) · Timers     │   │
│  │ Storage: SQLite (sqlx/rusqlite) · secrets in OS keyring (keyring) · JsHost (rquickjs)               │   │
│  │ WorkerPools: python (JSON-RPC/stdio) · dotnet C# (JSON-RPC/stdio) · isolated JS (same binary)       │   │
│  └─────────────────────────────────────────────────────────────────────────────────────────────────────┘   │
└──────────────┬──────────────────────────────────────────────────────────────────────────────────────────────┘
               │ WebKitWebProcess / WebView2 / WKWebView (UI only)
   child procs: python worker(s) · cs worker · optional isolated-js worker
   external clients: OBS browser sources & docks · browsers (LAN, opt-in) · Stream Deck/Companion · plugins
```

- **One engine, many clients.** The Tauri UI talks to the engine through the **same public API** (`/api` + `/ws`) as third-party plugins. In-process it uses Tauri IPC with the same typed messages, which avoids an HTTP hop. This enforces the owner's API-first requirement.
- `streamer-lived` is the same core with no GUI dependencies. It ships in the same packages, with an optional systemd user unit on Linux. The Tauri app can attach to an already-running daemon.
- Workspace layout: `crates/core`, `crates/integrations-*`, `crates/tts`, `crates/scripting`, `crates/server`, `apps/desktop` (Tauri), `apps/daemon`, `web/ui`, `web/overlay-runtime`, `web/designer`, `sdk/ts`, `sdk/python`, `sdk/csharp-shim`.

### 11.2 Key crates and libraries (versions verified 2026-10-18)

| Area | Choice | Version |
|---|---|---|
| Shell | `tauri` (+ plugins: updater 2.14.0, global-shortcut 2.4.1, single-instance, autostart, deep-link, window-state, notification) | 2.12.4 |
| Async | `tokio` | 1.53.3 |
| HTTP/WS server | `axum` | 0.8.9 |
| WS client | `tokio-tungstenite` | 0.30.0 |
| HTTP client | `reqwest` | 0.13.5 |
| Twitch | `twitch_api` (Helix + EventSub types), `twitch-irc` (fallback) | 0.8.0 / 6.1.3 |
| OBS | `obws` (OBS WebSocket v5) | 0.15.0 |
| Storage | `sqlx` 0.9.0 or `rusqlite` 0.40.2 | — |
| Secrets | `keyring` | 4.2.0 |
| File watching | `notify` (8.x stable; 9.0 in RC) | 8.2.0 |
| JS | `rquickjs` | 0.14.0 |
| Wasm plugins (later) | `wasmtime` / `extism` | 49.0.3 / 1.41.0 |
| IPC | `interprocess` (local sockets) / hand-rolled JSON-RPC (or `jsonrpsee` 0.26.1) | 2.4.6 |
| Speech | `sherpa-onnx` (official crate, static) / `whisper-rs` (optional) | 1.13.8 / 0.16.0 |
| Audio | `cpal` (+ `pipewire` feature on Linux) / `rodio` | 0.18.2 / 0.23.0 |
| MIDI | `midir` | 0.11.1 |
| Hotkeys | `global-hotkey` (Win/macOS/X11) + `ashpd` (Wayland portal) | 0.9.0 / 0.13.13 |
| Tray / menu | `tray-icon` / `muda` (via Tauri) | 0.26.1 / 0.21.2 |
| TS type generation | `ts-rs` 12.0.1 (stable) or `specta`/`tauri-specta` (still **2.0.0-rc.25**) | — |
| Frontend | TypeScript 7.0 · Vite 8.3 · **React 19.3** (most agent training data) or Svelte 5.57 / Solid 1.9 · `@tauri-apps/api` 2.12.4 | — |
| Designer | DOM editor + `interactjs`/custom handles; optional `konva` 10.9 for canvas widgets | — |
| Overlay runtime | Vanilla TS (or Preact), served from axum; no framework in OBS | — |

### 11.3 Threading and performance notes

- Tokio multi-thread runtime for I/O, with a small worker count; the engine is I/O-bound.
- Separate OS threads for the cpal callbacks, the QuickJS pool and sherpa-onnx inference (CPU-bound; bounded via `spawn_blocking` or a dedicated pool, with ONNX threads capped at 1–2 so games keep their cores).
- Idle target: under 1% CPU and under 150 MB RSS for the engine plus the webview with the UI closed. **Close the webview when minimised to tray**: the engine keeps running and the window is recreated on demand. Firebot-like Electron apps can't drop to near-zero this way, and on Linux it also takes WebKitGTK out of the picture while streaming.

### 11.4 Keeping it agent-maintainable

- **Single source of truth** for the public API: Rust types → JSON Schema/OpenAPI (e.g. via `schemars`/`utoipa`, not version-checked) → generated TS client, Python stubs and C# shim.
- Golden tests per integration, using recorded EventSub/OBS payloads.
- Playwright tests for UI and overlays; `cargo nextest` for the core.
- `AGENTS.md`/`CLAUDE.md` rules, e.g. "never set WebKit env vars unconditionally" (cf. folio#13), and a "no WebGL in main UI" lint.

---

## 12. Key risks and mitigations

| Risk | Likelihood / impact | Mitigation |
|---|---|---|
| **WebKitGTK blank window, flicker or crash on Wayland + NVIDIA** (owner's exact setup) | High / Medium. The UI breaks but the engine doesn't | §4.3: early-GL quirk; compatibility levels plus crash-loop fallback; never unconditional env vars; UI available in a browser; close the webview while streaming; Day-1 spike; Tauri 3 CEF variant as the exit |
| WebKitGTK slower than Chromium for the designer | Medium / Low–Medium | DOM/CSS editor, no WebGL; heavy previews in a browser or OBS dock |
| Designer preview ≠ OBS CEF 150 output | Medium / Medium | Chromium Playwright tests; "Preview in OBS dock/browser" button |
| Tauri 2 → 3 churn (3.0 alpha now) | Certain / Low | Thin shell crate; engine independent of Tauri; nightly CI against 3.x |
| Wayland global hotkeys | Medium / Medium | Portal via `ashpd` (Hyprland/KDE/GNOME); CLI/HTTP triggers; Hyprland `global` binds documented |
| Python availability and versions | Medium / Medium | Managed python-build-standalone; out-of-process worker; optional dependency |
| Chromium LNA tightening in OBS | Low–Medium / High for overlays | Serve overlays from our loopback origin; track obs-browser's "temporary" loopback grant |
| GPL contamination via espeak-ng (Piper/Kokoro phonemisation in sherpa-onnx) | Certain if those voices are used / licensing | Pick **GPL-3.0-or-later** for the app, or make espeak-based voices a separately-downloaded component (legal review) |
| Rust learning curve for agents | Medium / Low | Strong CI, tests, small crates, generated types (§11.4) |
| Model and binary bloat | Medium / Low | Download models on demand; no V8; no bundled Python/.NET |

---

## 13. Open questions for the owner

1. **License.** GPL-3.0-or-later (simplest with espeak-ng/Piper; Firebot is GPL-3.0) or MIT/Apache-2.0, which would push espeak-based voices into an optional download? This also decides SignPath eligibility (OSI license, no commercial dual-licensing).
2. **macOS.** Commit now (US$99/yr Apple Developer account, notarization in CI) or "best effort, unsigned" later?
3. **Day-1 spike.** Can we run a 1-day Tauri 2 prototype on your Hyprland + NVIDIA machine before locking the stack? Does `goxlr-utility-ui` currently render correctly for you, with or without any env vars?
4. **UI framework.** Any preference among React (most agent training data), Svelte 5 or Solid? The recommendation is React + TypeScript.
5. **Python provisioning.** Use system Python first, or always a managed, pinned interpreter for reproducibility? Should scripts be able to `pip install` packages (per-script requirements)?
6. **C#.** Given that you dislike C#, how much Streamer.bot compatibility do you want? Options: (a) import actions and triggers only and rewrite C# to JS/Python; (b) run `CPHInline` code via the optional .NET sidecar with a partial `CPH` shim; (c) both.
7. **Remote / LAN access.** Should the UI and API be reachable from another PC or a phone (two-PC setups), or stay localhost-only?
8. **Distro baselines.** Is Ubuntu 22.04 / Debian 12 / Fedora (latest two) / Arch rolling acceptable? Is Flatpak wanted at launch or later?
9. **Updates.** Is it acceptable that deb/rpm users update via the package manager or a hosted repo, while AppImage/Windows/macOS self-update?
10. **STT and GPU.** Is STT a v1 feature? Should we ship GPU acceleration (Vulkan/CUDA/DirectML) or stay CPU-only to protect game performance?
11. **Plugin SDK languages.** TS/JS and Python via the public WS API only, or also in-process Wasm plugins later?
12. **Daemon mode.** Should `streamer-lived` (headless systemd user service) be a supported v1 configuration, or internal-only at first?

---

## 14. Sources (primary, in addition to inline links)

- Registry data, queried 2026-10-18: crates.io API (`tauri`, `wry`, `tao`, `global-hotkey`, `ashpd`, `rquickjs`, `boa_engine`, `deno_core`, `v8`, `pyo3`, `rustpython-vm`, `sherpa-onnx`, `sherpa-rs`, `whisper-rs`, `tts`, `cpal`, `rodio`, `midir`, `ort`, `axum`, `tokio`, `tokio-tungstenite`, `reqwest`, `obws`, `twitch_api`, `twitch-irc`, `notify`, `sqlx`, `rusqlite`, `keyring`, `wasmtime`, `extism`, `javy`, `specta`, `tauri-specta`, `ts-rs`, `tray-icon`, `muda`, `interprocess`, `jsonrpsee`); npm registry (`electron`, `electron-builder`, `electron-updater`, `@electron-forge/cli`, `isolated-vm`, `@tauri-apps/api`, `@tauri-apps/cli`, `svelte`, `react`, `solid-js`, `vite`, `typescript`, `grapesjs`, `konva`, `fabric`, `moveable`, `interactjs`, `tldraw`, `@wailsio/runtime`); NuGet (`Avalonia`, `Jint`, `Microsoft.ClearScript.V8`, `pythonnet`, `Microsoft.CodeAnalysis.CSharp.Scripting`, `Velopack`, `dotnet-script`); GitHub Releases API (tauri, tauri-action, wails, electron, Avalonia, slint, sherpa-onnx, whisper.cpp, piper1-gpl, python-build-standalone, uv, quickjs-ng, boa, rusty_v8, deno, jint, pyo3, OBS Studio, n8n, Firebot).
- Tauri: [Linux graphics issues](https://v2.tauri.app/develop/debug/linux-graphics/) · [CEF runtime (v3)](https://v3.tauri.app/develop/cef/) · [AUR](https://v2.tauri.app/distribute/aur/) · [Flathub](https://v2.tauri.app/distribute/flatpak/) · [RPM](https://v2.tauri.app/distribute/rpm/) · [Debian](https://v2.tauri.app/distribute/debian/) · [AppImage](https://v2.tauri.app/distribute/appimage/) · [GitHub pipeline](https://v2.tauri.app/distribute/pipelines/github/) · [Updater](https://v2.tauri.app/plugin/updater/) · raw docs source: `tauri-apps/tauri-docs` `src/content/docs/distribute/*.mdx` (branch `v2`).
- WebKitGTK/NVIDIA field reports: [voicebox#1029](https://github.com/jamiepine/voicebox/issues/1029) · [cline#14687](https://github.com/cline/cline/pull/14687) · [folio#13](https://github.com/liminal-hq/folio/pull/13) · [tauri-plugin-wayland-nvidia-quirk](https://github.com/arsalan-anwari/tauri-plugin-wayland-nvidia-quirk) · [NVIDIA dev forum](https://forums.developer.nvidia.com/t/webkit-tauri-application-white-screen-on-dgx-spark-gpupermission-issues/348741).
- Hotkeys: [global-hotkey changelog](https://github.com/tauri-apps/global-hotkey/blob/dev/CHANGELOG.md) · [Hyprland global binds](https://wiki.hypr.land/configuring/core/binds/globals/) · [Electron globalShortcut](https://www.electronjs.org/docs/latest/api/global-shortcut) · [xfwl4 portal support note](https://gitlab.xfce.org/xfce/xfwl4/-/issues/93).
- Frameworks: [Elanis framework benchmark](https://github.com/Elanis/web-to-desktop-framework-comparison) · [Electron 38](https://electronjs.org/blog/electron-38-0) · [Electron Wayland talk](https://electronjs.org/blog/tech-talk-wayland) · [Avalonia 12.1](https://avaloniaui.net/blog/release-12-1) · [Wails status](https://docs-preview.wails.io/status/) · [Flutter multi-window (2026)](https://startdebugging.net/2026/08/how-to-enable-multi-window-support-in-a-flutter-desktop-app/) · [Qt WebEngine separate release](https://www.qt.io/blog/qt-webengine-will-release-separate) · [Slint tray](https://docs.slint.dev/latest/docs/slint/reference/window/systemtrayicon/).
- Scripting: [rquickjs](https://github.com/DelSkayn/rquickjs) · [rquickjs Runtime docs](https://docs.rs/rquickjs/latest/rquickjs/struct.Runtime.html) · [boa](https://github.com/boa-dev/boa) · [rusty_v8 releases](https://github.com/denoland/rusty_v8/releases) · [isolated-vm](https://github.com/laverdet/isolated-vm) · [pyo3](https://github.com/PyO3/pyo3) · [python-build-standalone](https://github.com/astral-sh/python-build-standalone/releases) · [RustPython](https://github.com/RustPython/RustPython) · [n8n task runners](https://docs.n8n.io/hosting/configuration/task-runners/) · [.NET file-based apps](https://learn.microsoft.com/en-us/dotnet/core/sdk/file-based-apps) · [Streamer.bot C# lifecycle](https://docs.streamer.bot/api/csharp/guide/lifecycle-methods) · [Streamer.bot C# variables](https://docs.streamer.bot/api/csharp/guide/variables).
- Speech/audio: [sherpa-onnx Rust examples](https://github.com/k2-fsa/sherpa-onnx/tree/master/rust-api-examples) · [sherpa-onnx Tauri examples](https://github.com/k2-fsa/sherpa-onnx/tree/master/tauri-examples) · [piper1-gpl](https://github.com/OHF-Voice/piper1-gpl) · [Kokoro-82M](https://huggingface.co/hexgrad/Kokoro-82M) · [Kokoro ONNX](https://huggingface.co/onnx-community/Kokoro-82M-v1.0-ONNX) · [whisper.cpp](https://github.com/ggml-org/whisper.cpp/releases) · [cpal](https://github.com/RustAudio/cpal) · [rodio changelog](https://github.com/RustAudio/rodio/blob/master/CHANGELOG.md) · [midir](https://github.com/Boddlnagg/midir) · [NaturalVoiceSAPIAdapter](https://github.com/gexgd0419/NaturalVoiceSAPIAdapter) · [Speaker.bot](https://speaker.bot/).
- Platform/OBS/security: [Twitch EventSub WebSocket limits](https://dev.twitch.tv/docs/eventsub/handling-websocket-events/) (3 connections per user token, 300 subscriptions per connection, max total cost 10) · [Twitch refresh tokens](https://dev.twitch.tv/docs/authentication/refresh-tokens/) (public-client refresh tokens "expire 30 days after they are generated"; no client secret needed, which suits an open-source desktop app) · [OBS 33 CEF 150](https://www.linuxcompatible.org/story/obs-studio-330rc1-released-major-plugin-rework-and-cef-upgrade-lead-the-way/) · [obs-browser loopback commit](https://github.com/obsproject/obs-browser/commit/058c30f1e6201d08db6af51faaf88f23feab4a3c) · [Chrome LNA](https://blog.openreplay.com/chrome-local-network-access-lna-permission/).
- Packaging/signing: [Flatpak permissions](https://docs.flatpak.org/en/latest/sandbox-permissions.html) · [SignPath Foundation terms](https://signpath.org/terms) · [Azure Artifact Signing](https://azure.microsoft.com/en-us/products/artifact-signing) · [GitHub ARM runners](https://github.blog/changelog/2025-08-07-arm64-hosted-runners-for-public-repositories-are-now-generally-available/).
- Agents and languages: [akitaonrails language benchmark (Oct 2026)](https://akitaonrails.com/en/2026/10/11/are-there-better-languages-for-programming-with-llms/) · [Rust-SWE-bench](https://arxiv.org/html/2602.22764v1) · [TypeScript 7 release](https://infoq.com/news/2026/08/typescript-7-released/).
- Prior art: [GoXLR Utility](https://github.com/GoXLR-on-Linux/goxlr-utility) · [Firebot](https://github.com/crowbartools/Firebot).
