# StreamDen — Master Plan

> **Product:** **StreamDen** by Tech Daddy Digital · GPL-3.0-or-later (SDKs MIT/Apache) · repo `TechDaddyKB/streamden` → `tech-daddy-digital/streamden` at release.
> **Status:** P0 Foundations, **Linux-first** (D-035). Rounds 1–2 decided. Spikes S-1/S-2/S-3 done on Linux and merged to main (DEN-2/3/4, S-1 evidence corrected in DEN-23); KDE/Windows validation parked until v0.1 (DEN-20). No production code yet.
> **How work runs:** cards on the codex-orchestrator board; agents launched with `scripts/dispatch.sh` (D-036).
> **Work items:** codex-orchestrator board, project **`streamden`** (`http://localhost:8488`), cards `DEN-<n>`. Agents launch via `clx`/`cdx`/`cgx` ([ADR-0005](docs/adr/0005-work-management-and-agent-tooling.md)).
> **Owner:** Kris (human; final say on every decision).
> **This file is the roadmap.** The detailed spec is written per subsystem under [`docs/spec/`](docs/spec/) once the open questions in [`docs/QUESTIONS.md`](docs/QUESTIONS.md) are answered.

---

## 1. Vision

**One modern, fast, cross-platform app that replaces Streamer.bot + Speaker.bot + SAMMI.** It adds a combined multichat, a WYSIWYG overlay/alert designer, first-class VTuber tooling, JavaScript and Python scripting, an API-first plugin SDK, and a built-in MCP server and AI layer.

It must feel as good as Streamer.bot's UI, run natively on **Windows, Linux (Debian/Ubuntu, Fedora, Arch + AUR) and macOS**, and do everything locally unless the user explicitly opts into a cloud feature.

### 1.1 Principles (non-negotiable unless the owner changes them)

| # | Principle | What it means in practice |
|---|---|---|
| P-1 | **Local-first, no mystery cloud** | Everything runs on the user's machine. Any feature needing a public URL (Kick events, Ko-fi webhooks…) goes through a relay the user **explicitly enables**, and the relay is open-source and self-hostable. |
| P-2 | **Linux is a first-class citizen** | Every feature is designed and tested on Linux (Wayland included) at the same time as Windows. "Windows-only" is allowed only when the *third-party app* is Windows-only, and the UI says so honestly. |
| P-3 | **API-first and dogfooded** | The UI, the overlays, the MCP server and every built-in integration use the **same public API and plugin SDK** that third-party developers get. There are no private endpoints. |
| P-4 | **Schema-driven everything** | Triggers, sub-actions, settings, plugin manifests and API messages are typed Rust structs that generate JSON Schema. Schemas generate the UI forms, the TS/Python/C# SDKs, the OpenAPI/AsyncAPI docs and the MCP tools. A plugin's sub-action looks exactly like a built-in one. |
| P-5 | **Light on resources** | It runs next to OBS and games. Idle targets are under 1% CPU and under 150 MB RSS with the window closed. The engine keeps running when the UI is closed. |
| P-6 | **Simple for streamers, deep for power users** | Wizards and starter packs (e.g. the VTuber Setup Wizard) sit on top, with full scripting and the API underneath. |
| P-7 | **Clean-room** | We study competitors' public docs and behaviour, never their code or decompiled binaries. Importers for their *data formats* are fine. |

### 1.2 What we're combining (research summary)

| Source | Size of the surface (Oct 2026) | Research doc |
|---|---|---|
| Streamer.bot 1.0.7 / 1.1.0-alpha | ~467 triggers, ~462 sub-actions, ~627 C# `CPH` methods, WS/HTTP/UDP servers | [triggers & integrations](docs/research/streamerbot-triggers-integrations.md), [engine & UI](docs/research/streamerbot-engine-ui.md) |
| Speaker.bot 0.1.6 | 10 TTS engines, voice aliases, filters, queue, VoiceGate, WS API | [speakerbot-sammi.md](docs/research/speakerbot-sammi.md) |
| SAMMI 2026.6 | ~440 commands, button decks, Bridge JS extensions, remote decks, OBS animation | [speakerbot-sammi.md](docs/research/speakerbot-sammi.md) |
| Competitors (Firebot, Lumia, Mix It Up, Social Stream Ninja, StreamElements, Streamlabs…) | Designer, alert queue, multichat and loyalty ideas | [competitors-and-designer.md](docs/research/competitors-and-designer.md) |
| Platform APIs (Twitch, YouTube, Kick, TikTok, X, donation services, OBS) | Feasibility and auth for a local-only app | [platform-apis.md](docs/research/platform-apis.md) |
| App and VTuber integrations (~70 apps and devices) | Control surfaces, OS support, tiers | [app-integrations-vtuber.md](docs/research/app-integrations-vtuber.md) |
| Plugin SDK and public API | Plugin tiers, manifest, codegen, registry | [plugin-sdk-and-api.md](docs/research/plugin-sdk-and-api.md) |
| MCP and AI providers | MCP server design; subscription-vs-API compliance | [mcp-and-ai-providers.md](docs/research/mcp-and-ai-providers.md) |
| Discord companion | Bot hosting, intents, voice (DAVE), StreamKit, feasibility per feature | [discord-companion.md](docs/research/discord-companion.md) |
| Branding and distribution | Name availability, trademarks, IDs, Steam/itch/GitHub | [branding-and-distribution.md](docs/research/branding-and-distribution.md) |
| StreamElements widgets + node editor | SE custom-widget API compatibility runtime; node-graph editor design | [se-widget-compat-and-node-editor.md](docs/research/se-widget-compat-and-node-editor.md) |
| P2P payments, X, Bluesky | Venmo/Cash App/X Money/PayPal/Stripe feasibility; X via own account; Bluesky | [payments-x-bluesky.md](docs/research/payments-x-bluesky.md) |
| Tech stack and packaging | Tauri 2 vs Electron vs others; scripting; TTS/STT; deb/rpm/AUR | [tech-stack.md](docs/research/tech-stack.md) |
| AI agent fleet | Instruction files, vendor strengths, coordination | [ai-agent-fleet.md](docs/research/ai-agent-fleet.md) |

The full feature checklist is **[docs/FEATURE-PARITY.md](docs/FEATURE-PARITY.md)**, one row per feature with the proposed priority and phase.

### 1.3 Where we beat all three

- **Real Linux and macOS support.** Streamer.bot runs only experimentally under Wine; SAMMI and Speaker.bot are Windows-only.
- **JS and Python scripting**, not just C#, with a typed `api` object and editor autocomplete.
- **One designer** for alerts, the chat overlay and widgets. Streamer.bot and SAMMI have none.
- **Integrated TTS.** Speaker.bot's features move inside, with local neural voices (Piper/Kokoro) that work offline on every OS.
- **A VTuber hub:** VTube Studio, VMC/OSC router, Warudo, VNyan, veadotube, heart rate, plus a setup wizard.
- **API-first plugins and a GitHub-PR-based registry.**
- **A built-in MCP server and AI assistant.**

---

## 2. Proposed architecture (summary)

See **[docs/architecture.md](docs/architecture.md)** for the full version and **[ADR-0002](docs/adr/0002-tech-stack.md)** for the stack decision (status: *Proposed*).

- **Rust engine** (`den-core`, Tokio) is a library with **no Tauri dependency**. It owns the event bus, the action engine, integrations, audio/TTS/STT, storage (SQLite) and the local server (axum).
- **Two binaries:** `streamden` (Tauri 2 desktop shell with the engine inside) and `streamdend` (headless daemon for two-PC rigs and servers).
- **One TypeScript web UI (React proposed)** served to the Tauri window, a normal browser, and OBS docks.
- **Overlays** are pages served by the engine's local server and rendered in OBS browser sources. The designer and the runtime share one renderer.
- **Scripting:** JS in-process (QuickJS, sandboxed). Python and optional C# run in out-of-process worker pools over JSON-RPC.
- **Plugins:** v1 tiers are declarative (manifest only), sidecar (any language over JSON-RPC) and embedded JS. WASM components come later.
- **Public API:** WebSocket (JSON-RPC 2.0, obs-websocket-style handshake and subscriptions) plus REST (OpenAPI) plus **MCP**, all generated from Rust types.
- **Runner-up stack: Electron.** We switch only if the Day-1 spike on the owner's Hyprland + NVIDIA machine fails badly ([tech-stack §4](docs/research/tech-stack.md)).

---

## 3. How we get from here to code

```
 Q&A rounds ──► ADRs accepted ──► Subsystem specs ──► Contracts (den-protocol) ──► Task breakdown ──► Build
 (QUESTIONS.md)  (docs/adr/)      (docs/spec/*.md)    (+ stubs + failing tests)    (board DEN-n)      (phases P0…P6)
        ▲                                   │
        └─────────── spikes S-1…S-5 feed back into decisions ─────────────┘
```

1. **Q&A rounds.** The owner answers [`docs/QUESTIONS.md`](docs/QUESTIONS.md). Answers become decision-log lines and ADRs.
2. **Spikes** (time-boxed, throwaway code under `spikes/`) de-risk the stack before we commit:
   - **S-1** Tauri 2 window on Hyprland + NVIDIA: render, tray, close-webview-on-minimise, multi-window.
   - **S-2** axum serves an overlay into an OBS browser source and a dock. Check the Wayland dock limitation and the native chat-window fallback.
   - **S-3** QuickJS sandbox (timeouts, memory limit) plus a Python worker JSON-RPC round-trip, with latency numbers.
   - **S-4** Twitch Device Code Flow plus EventSub WebSocket hello-world on Linux and Windows.
   - **S-5** Agent bake-off: the same 3–4 tasks for Claude, Codex and Grok (see [docs/agents/DELEGATION.md](docs/agents/DELEGATION.md)).
3. **Subsystem specs** in `docs/spec/`, one file per workstream below. Each is written by the "spec" agent for that workstream, cross-reviewed by another vendor, and approved by the owner.
4. **Contracts first.** `crates/den-protocol` (types, plus generated JSON Schema, TS and Python) lands *before* parallel implementation, together with stubs and failing contract tests.
5. **Work items.** Each spec is broken into board cards (`DEN-<n>`, template in `tasks/_TEMPLATE.md`) that an agent can finish in one session (≤ ~400 changed lines).

---

## 4. Workstreams

Each workstream has a spec file, a code area, and a default vendor routing. Routing is **provisional until the S-5 bake-off**; see [docs/agents/DELEGATION.md](docs/agents/DELEGATION.md).

| ID | Workstream | Code area (proposed) | Spec | Default implementer → reviewer |
|---|---|---|---|---|
| WS-01 | Repo, CI, packaging and release (deb/rpm/AppImage/AUR/MSI/DMG, updater, signing) | `.github/`, `packaging/`, `scripts/` | `spec/01-build-release.md` | Codex (GPT-6.1 Sol) → Claude |
| WS-02 | Protocol and contracts (types, schema, codegen, versioning) | `crates/den-protocol`, `sdk/*` (generated) | `spec/02-protocol.md` | Claude Opus → Codex |
| WS-03 | Engine core (event bus, actions, triggers, sub-actions, queues, variables, expressions, storage) | `crates/den-core` | `spec/03-engine.md` | Claude Opus → Codex |
| WS-04 | Scripting (QuickJS host, Python and C# worker pools, `api` bindings, editor typings) | `crates/den-scripting`, `workers/*` | `spec/04-scripting.md` | Claude Opus → Codex |
| WS-05 | Platform connectors (Twitch, YouTube, Kick, X, Discord, donation services, relay) | `crates/integrations/den-{twitch,youtube,…}`, `relay/` | `spec/05-platforms.md` | Codex (Sol) → Claude |
| WS-06 | Local server and public API (REST, WS JSON-RPC, auth and pairing, LAN mode, MCP) | `crates/den-server` | `spec/06-api-mcp.md` | Codex (Sol) → Claude |
| WS-07 | App shell and core UI (Tauri shell, action editor, settings, logs, design system) | `apps/desktop`, `web/ui`, `web/kit` | `spec/07-ui.md` | Claude Sonnet → Codex |
| WS-08 | Multichat (unified message model, emotes, badges, filters, native chat window, OBS dock) | `crates/den-chat`, `web/ui/chat` | `spec/08-multichat.md` | Claude Sonnet (UI) + Codex (parsers) |
| WS-09 | Overlay runtime and WYSIWYG designer (alerts, variations, queue, widgets) | `web/overlay-runtime`, `web/designer` | `spec/09-designer.md` | Claude Opus/Sonnet → Codex |
| WS-10 | Audio, TTS and STT (devices, buses, sherpa-onnx, OS and cloud voices, Speaker.bot features) | `crates/den-audio`, `crates/den-tts`, `crates/den-stt` | `spec/10-audio-tts.md` | Claude Opus → Codex |
| WS-11 | App and VTuber integrations (OBS, VTS, VMC/OSC, veadotube, Warudo, Stream Deck, MIDI, hotkeys, HA…) | `crates/integrations/*`, `plugins/*` | `spec/11-integrations.md` | Codex (Sol) bulk; Claude does the first reference integration |
| WS-12 | Plugin SDK, `den` CLI, plugin host, registry | `crates/den-plugin-host`, `sdk/`, `tools/den-cli` | `spec/12-plugins.md` | Claude Opus → Codex |
| WS-13 | AI layer (providers, assistant, AI chat responder, Connect-your-AI) | `crates/den-ai`, `web/ui/assistant` | `spec/13-ai.md` | Claude Opus → Codex |
| WS-14 | Decks and remote control (SAMMI-style grid, phone deck, Stream Deck/Touch Portal bridges) | `web/deck`, `crates/den-deck` | `spec/14-decks.md` | Claude Sonnet → Codex |
| WS-15 | Import/compat (Streamer.bot share strings, SAMMI decks, Speaker.bot settings, SB WS-API compat) | `crates/den-import` | `spec/15-import.md` | Codex (Sol) → Claude |
| WS-16 | QA (event simulator, golden payloads, e2e, perf budgets) | `crates/den-testkit`, `e2e/` | `spec/16-qa.md` | Codex (Sol) writes tests for code Claude wrote, and vice versa |
| WS-17 | Docs site and user guides | `docs-site/` | — | Claude Sonnet |
| WS-18 | Ecosystem watch (API deprecations, X/Twitter, platform news, community sentiment) | `docs/research/` | — | Grok 4.7 (live X/web) → verified by Claude/Codex |

---

## 5. Phases and milestones

Phases overlap where dependencies allow. **Exit criteria are what "done" means**; dates are set after the bake-off.

| Phase | Release | Scope | Exit criteria |
|---|---|---|---|
| **P0 Foundations** | — | Repo skeleton, CI (fmt/clippy/test/typecheck on Win/Linux/macOS), packaging skeleton (deb, rpm, AppImage, AUR `-bin`, MSI/NSIS), `den-protocol` + codegen + drift check, task tooling (`scripts/claim`, index generator), design tokens, ADRs 0001–0005 accepted, spikes S-1…S-5 done | A "hello" build installs from CI artifacts on Debian, Fedora and Arch (Windows **compiles** in CI; runtime check at DEN-20). Codegen drift check is green. |
| **P1 Core** | v0.1 (MVP) | Engine (actions/triggers/sub-actions/queues/variables), SQLite storage, local REST+WS API with tokens, **Twitch** (EventSub, chat, core events, channel points), **OBS**, **JS execute-code**, action editor UI, logs, action history, test triggers, JSON import/export | On **Linux** a streamer can build "Tier-3 sub → switch OBS scene + chat message + JS snippet" and it survives a restart. This is the first working deliverable. Then the **DEN-20 cross-platform gate** (KDE + Windows) runs before P2 work is called done on Windows. |
| **P2 Chat + TTS** | v0.2 | Unified chat model, **YouTube**, **Kick**, native chat window + OBS dock, **StreamElements widget compatibility runtime** (Etsy SE chat/alert widgets run natively, fed by multichat), chat overlay (basic template), filters and bot list, emotes (Twitch/7TV/BTTV/FFZ/YouTube), deleted-message sync, **TTS** (Piper/Kokoro via sherpa-onnx, OS voices, cloud voices), voice aliases, TTS filters and queue, **X** (your own developer app: posting plus follower/mention triggers), **Bluesky** (posting, Live badge, triggers), Discord webhooks, Stripe/PayPal tips, **self-hostable relay** | Combined chat from three platforms in a dock and an overlay, filtered; TTS reads chat with per-user voices on Linux and Windows. |
| **P3 Designer** | v0.3 | WYSIWYG designer (StreamElements-editor mental model), **node-based action editor** (second view, in sync with the list), alert queue (priority lanes, gift-bomb merge), variations (tier, amount, gifted…), widgets (goals, counters, event list, credits), chat overlay re-done in the designer, asset packs | A user designs Twitch sub (per tier), follow, and YouTube membership/super chat alerts with no code, and they play in OBS. |
| **P4 Integrations + SDK + AI** | v0.5 | **Discord companion bot** (first-party plugin; Q-S7), public plugin SDK (declarative, sidecar, embedded JS), `den` CLI, **Python scripting**, VTuber Tier-1 (VTS, VMC/OSC router, veadotube, Warudo, VNyan, heart rate) + **VTuber Setup Wizard**, Stream Deck plugin (and OpenDeck), MIDI, global hotkeys (incl. Wayland portal), Home Assistant, Voicemod, **MCP server**, **AI layer** (API keys + Connect-your-AI) | Two first-party integrations ship as *external* plugins built with the public SDK. Claude Desktop can create an action through MCP. |
| **P5 Decks + Voice + Community** | v0.8 | SAMMI-style decks and phone deck (LAN mode + pairing), STT/voice commands, points/currency, quotes, counters, giveaways, cross-platform identity merge, donation services (Streamlabs, StreamElements, Ko-fi via relay, Patreon…), Streamer.bot share-string import | Feature parity checklist: all P1-priority rows done. |
| **P6 Ecosystem** | v1.0+ | Public plugin registry (GitHub-PR model), WASM plugin tier, optional hosted relay, C# `CPH` compatibility sidecar, more platforms (TikTok?), i18n, Flatpak | Registry live with third-party plugins; v1.0 stabilization. |

---

## 6. Distribution targets

| Target | Format | Phase | Notes |
|---|---|---|---|
| Debian/Ubuntu | `.deb` (+ apt repo later) | P0 | Tauri bundler |
| Fedora | `.rpm` (+ COPR later) | P0 | Tauri bundler |
| Arch | AUR `streamden-bin` (+ `streamden` from source, `-git`) | P0 | PKGBUILDs drafted in [tech-stack §10.3](docs/research/tech-stack.md) |
| Any Linux | AppImage | P0 | Has its own updater path |
| Windows | MSI + NSIS, winget later | P0 | Signing via SignPath (OSS) or Azure Trusted Signing |
| macOS | DMG (universal) | P0 build, signing pending Q | Needs the $99/yr Apple account (open question) |
| Flatpak / Flathub | — | P6 | Sandbox makes system Python/.NET and local ports awkward |
| Headless | `streamdend` + systemd user unit | P1 | Two-PC rigs |

### 6.1 Test environments available to the project

| Env | What | How agents use it | Covers |
|---|---|---|---|
| **E-1 Primary dev box** | Omarchy (Arch), **Hyprland** (Wayland), high-end NVIDIA RTX + AMD iGPU, NVIDIA open driver | Local | Worst-case WebKitGTK/Wayland/NVIDIA; Arch/AUR packaging; Hyprland portals |
| **E-2 CachyOS + KDE Plasma** (Arch-based, Wayland) | Separate machine owned by the owner | **Parked until v0.1 (D-035, DEN-20).** SSH, granted by the owner when needed. Never store its credentials in the repo. | KDE portals (GlobalShortcuts, RemoteDesktop), the second most popular Wayland desktop |
| **E-3 Windows VM on E-1** | `omarchy windows vm` (dockur/windows container on KVM, needs Docker; launching asks for polkit auth) | Start/stop: `omarchy windows vm launch / stop / status`. Viewer at `http://127.0.0.1:8006` (browser) or RDP at `127.0.0.1:3389`. Shared folder: host `~/Windows`. **Parked until v0.1 (D-035, DEN-20).** Ask the owner before launching (it prompts for auth and uses lots of RAM). | WebView2, MSI/NSIS installers, Windows-only integrations (VoiceMeeter, Warudo, VNyan…) |
| **CI** | GitHub Actions: Ubuntu, Fedora and Arch containers, Windows, macOS runners | Automatic | Build/package matrix; headless tests |
| _Gap_ | Debian/Ubuntu and Fedora desktops, macOS hardware | CI containers for builds; VMs later if needed | |

---

## 7. Top risks

| Risk | Mitigation |
|---|---|
| WebKitGTK blank windows or crashes on Wayland + NVIDIA (the owner's exact setup) | Spike S-1. Graphics-compat setting with crash-loop fallback, never unconditional env vars, the UI also works in a browser, the engine survives UI crashes. Exit hatch: Tauri 3 CEF runtime or Electron. |
| OBS disables custom browser docks on native Wayland (confirmed S-2) | Ship a native chat window. Document running OBS under XWayland. |
| OBS browser sources blank or crashing on NVIDIA Linux with browser HW acceleration on (S-2) | The setup wizard detects Linux + NVIDIA and walks the user through turning it off; troubleshooting docs. |
| WebKit memory with the UI closed (S-1: 184 MB PSS / 332 MB RSS closed to tray) | Engine + tray process never loads WebKit; the UI is a separate on-demand process (spec 07). |
| Platform API churn (Twitch removals, Kick's webhook-only events, YouTube quota, X pricing) | WS-18 ecosystem watch (Grok), golden payload tests, connectors behind the plugin interface |
| YouTube quota (10k units/day per Google project) | Users bring their own Google project (wizard), with a shared project as an option, and use `streamList` streaming |
| Kick requires a public webhook URL and a client secret | Self-hostable relay (Cloudflare Worker) in P2; users can bring their own Kick app |
| Scope explosion (~900 triggers and sub-actions across competitors) | FEATURE-PARITY priorities, schema-driven generation, integrations as plugins |
| Multi-vendor agents stepping on each other | Contracts-first, path leases, single-writer files, cross-vendor review, CI as arbiter ([AGENTS.md](AGENTS.md)) |
| GPL interplay (espeak-ng via Piper/Kokoro) | License decision (Q-L1) |

---

## 8. Document map

| File | Purpose | Who edits |
|---|---|---|
| [AGENTS.md](AGENTS.md) | Rules every AI agent follows (all vendors) | Owner, or a dedicated task |
| [CLAUDE.md](CLAUDE.md) | Imports AGENTS.md, plus Claude-only notes | Owner |
| [PLAN.md](PLAN.md) | This roadmap | Planning agent + owner |
| [docs/QUESTIONS.md](docs/QUESTIONS.md) | Open questions for the owner, with recommendations | Any agent may *add*; owner answers |
| [docs/decisions.md](docs/decisions.md) | One line per decision, newest first | Whoever records an owner decision |
| [docs/adr/](docs/adr/) | Architecture Decision Records | Agents propose; owner accepts |
| [docs/architecture.md](docs/architecture.md) | System map | WS-02/WS-03 spec agents |
| [docs/FEATURE-PARITY.md](docs/FEATURE-PARITY.md) | Feature checklist with priorities and phases | Planning agent; owner sets priorities |
| [docs/spec/](docs/spec/) | Subsystem specs (after Q&A) | Workstream spec agents |
| [docs/agents/DELEGATION.md](docs/agents/DELEGATION.md) | Which vendor does what, bake-off, dispatch | Owner + planning agent |
| [docs/agents/PROMPTING.md](docs/agents/PROMPTING.md) | How to prompt each vendor, with templates | Owner + planning agent |
| [docs/agents/board.md](docs/agents/board.md) | Orchestrator board lanes, labels, claim/release protocol | Planning agent |
| [scripts/dispatch.sh](scripts/dispatch.sh) | Launch `clx`/`cdx`/`cgx` on a card in its own worktree (work or review) | Owner |
| [docs/spikes/](docs/spikes/) | Spike reports (S-1…S-5) | Spike agents |
| [docs/GLOSSARY.md](docs/GLOSSARY.md) | Shared vocabulary | Any agent |
| [docs/owner/](docs/owner/) | **Human runbooks**: step-by-step registrations (Twitch, Google, Kick, X, Discord…), go-live requirements, signing, master checklist | Any agent whose task needs the owner to act |
| [tasks/](tasks/) | Card and handoff templates (the cards themselves live on the orchestrator board) | Owner |
| [docs/research/](docs/research/) | Raw research with sources | Research agents |
