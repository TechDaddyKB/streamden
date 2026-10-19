# Architecture (DRAFT — proposed, pending ADR-0002)

> This draft explains the system shape so that specs and agents share one picture. Details are decided in
> `docs/spec/*` and ADRs. Source research: [tech-stack.md](research/tech-stack.md),
> [plugin-sdk-and-api.md](research/plugin-sdk-and-api.md), [competitors-and-designer.md](research/competitors-and-designer.md).

## 1. Process model

```
┌──────────────────────── streamden (Tauri 2 desktop app — one OS process) ─────────────────────────┐
│ Tauri shell (thin): windows, tray, single-instance, autostart, deep links, updater, graphics-compat    │
│      │ in-proc typed channel  (or WS to an external `streamdend` when "attach to daemon")         │
│ ┌────▼───────────────────────────── den-core (Rust lib, Tokio) ───────────────────────────────────────┐ │
│ │ EventBus ─► TriggerMatcher ─► ActionEngine (queues, concurrency, sub-action tree, variables)        │ │
│ │ Connectors: Twitch · YouTube · Kick · X · OBS · VTube Studio · VMC/OSC · … (all via plugin trait)    │ │
│ │ ChatHub (unified messages, emotes, filters)   AlertQueue   Timers/Cron   FileWatch   MIDI   Hotkeys │ │
│ │ Audio buses (cpal/rodio) · TTS (sherpa-onnx / OS / cloud) · STT (optional)                         │ │
│ │ Scripting: JsHost (rquickjs, in-proc) · WorkerPool<python> · WorkerPool<dotnet> (JSON-RPC/stdio)    │ │
│ │ PluginHost: declarative · sidecar (JSON-RPC) · embedded JS · (WASM later)                           │ │
│ │ Storage: SQLite · secrets in OS keychain                                                            │ │
│ │ den-server (axum): /api REST+OpenAPI · /ws JSON-RPC events · /mcp · /ui SPA · /o/* overlays · /dock/* │ │
│ └─────────────────────────────────────────────────────────────────────────────────────────────────────┘ │
└──────┬─────────────────────────────────────────────────────────────────────────────────────────────────┘
       │ webview (WebKitGTK / WebView2 / WKWebView) shows the UI only; closed when minimised to tray
 child processes: python workers · C# worker (optional) · sidecar plugins · isolated-JS worker
 external clients: OBS browser sources + docks · browsers (LAN mode) · Stream Deck/OpenDeck · phone deck ·
                   MCP clients (Claude, Codex, ChatGPT, Cursor…) · third-party apps via WS/REST
```

- `streamdend` is the **same engine without a GUI** (headless daemon, optional systemd user unit). The
  desktop app can attach to a running daemon, e.g. a daemon on the stream PC controlled from the gaming PC.
- **The UI is just another API client.** In-process it uses Tauri IPC carrying the *same* typed messages as `/ws`.
- **Process split (from spike S-1, to be specified in spec 07):** the always-running process is the engine plus
  tray and **never loads WebKit** (52 MB PSS / 92 MB RSS measured). The UI windows run in a separate, on-demand
  Tauri process that talks to the engine over the local API, and is fully exited when the last window closes.
  Measured: the in-process app closed to tray still held 184 MB PSS / 332 MB RSS, and destroying a webview
  without terminating its web process leaked it (3 close/reopen cycles left 4 web processes, 573 MB PSS).

## 2. Core domain model

```
Event (typed, from a Source) ──► Trigger (filter + conditions) ──► Action ──► Queue ──► Run
                                                                    │
                                                                    └─ Sub-action tree:
                                                                       groups · if/else · switch · while
                                                                       · run-action · delay · code (JS/Py/C#)
                                                                       · connector ops (obs.setScene …)
```

| Concept | Definition |
|---|---|
| **Source** | Anything that emits events: a platform connector, integration, timer, hotkey, MIDI, file watcher, HTTP/WS endpoint, plugin. |
| **Event** | `{ id, source, type, ts, platform?, user?, payload }`. `type` is namespaced (`twitch.subscription`, `obs.scene_changed`, `vts.hotkey_triggered`). The payload is typed in `den-protocol`. |
| **Trigger** | Attached to an action. Matches an event type plus optional conditions (tier, amount range, reward id…). Precedence: specific beats "any" unless the trigger is marked *always run*. |
| **Action** | Named, grouped, enable/disable, with a queue assignment, concurrency flag, random/weighted option and history flag. |
| **Queue** | Default (parallel) or named (blocking, pausable, clearable). |
| **Sub-action** | A typed operation from a schema (built-in or plugin), or a control-flow node. |
| **Run context** | The variable scope for a run: event payload, plus arguments set by earlier sub-actions, plus globals. It is snapshotted into **Action History** before and after the run, for replay. |
| **Variables** | Run args (ephemeral), globals (persisted or temp), per-user vars (per platform user; P5 adds cross-platform identity). |

## 3. Schema-driven contracts (single source of truth)

```
Rust types in crates/den-protocol (serde + schemars)
      │  build step: `cargo xtask codegen`
      ├─► protocol.json (JSON Schema 2020-12 catalog)
      │      ├─► OpenAPI 3.1 (REST)          ├─► AsyncAPI 3 (WS events)
      │      ├─► TS types + client (web/ui, overlay runtime, sdk/ts)
      │      ├─► Python stubs (sdk/python, script `api.pyi`)  ├─► C# shim (optional)
      │      ├─► Plugin manifest schema       └─► MCP tool definitions
      └─► docs site reference pages
CI fails if any generated artifact drifts from the Rust types.
```

## 4. Public API surfaces

| Surface | Transport | Used by | Notes |
|---|---|---|---|
| WebSocket API | JSON-RPC 2.0 over WS on `127.0.0.1:<port>` | UI, overlays, plugins, third-party tools | obs-websocket-style hello/identify, `rpcVersion`, topic subscriptions, high-volume events opt-in, batches, `plugins.call` vendor requests |
| REST API | HTTP + OpenAPI 3.1, SSE for streams | Scripts, Stream Deck, curl, webhooks in | Serves `/openapi.json` and `/protocol.json` |
| MCP | Streamable HTTP on localhost, plus a stdio shim binary | Claude Desktop/Code, Codex, ChatGPT, Cursor, Grok… | Tools generated from sub-action and request schemas. Large toolsets are handled with search/categories. Destructive tools need approval. |
| Plugin protocol | JSON-RPC 2.0 over stdio (sidecar), host functions (embedded JS), WIT (WASM, later) | Plugins | Mirrors the WS API one-to-one |
| Compatibility (optional) | Streamer.bot-compatible WS API | Existing SB overlays and docks | Pending Q-X1 |

Auth: scoped, hashed tokens created through a pairing prompt; read-only overlay tokens; Host and Origin
checks; LAN mode opt-in (Q-N1).

Default port: must **not** collide with tools users run alongside us. Avoid OBS 4455, Streamer.bot 8080, 7474
and 4242, Speaker.bot 7580 (docs also say 7680), SAMMI 9450 and 9470, VTube Studio 8001, Streamlabs Desktop 59650,
VMC 39539/39540. Pick it in spec 06, with automatic fallback if the port is taken.

## 5. Key pipelines

**Multichat.** Platform connectors produce `chat.message` events in one unified schema (platform, user, roles,
badges, fragments: text/emote/mention/link, reply, color, pronouns, first-time flag…). The ChatHub runs
named **filter profiles** (dock, overlay, TTS), each with a bot list, `!command` hiding, words/regex, roles,
platforms and links. Results go out to the native chat window, the OBS dock (`/dock/chat`) and overlays
(`/o/<id>`). Moderation events (delete, timeout, ban) are propagated, so a deleted message disappears from
the overlays and the TTS queue.

**Alerts.** Events go to the alert rules (variation conditions: tier, amount ranges, gifted, months…) and then
into the AlertQueue (lanes, priority, merge gift bombs, pause/skip/replay, persisted). The overlay runtime
preloads the assets and plays the alert, then acks completion back so the queue advances.

**Designer.** One DOM/CSS renderer is shared by the editor, the dock preview and the OBS runtime. Designs are
versioned JSON (layers, bindings, variations, enter/idle/exit animations using the Web Animations API) with
assets stored by content hash. Live edits patch running overlays without a reload.

**TTS.** Speak requests come from chat, alerts or sub-actions. Filters run first (bad words, regex replacements,
links, emotes, length caps), then voice selection (alias, per-user voice, random), then the engine
(sherpa-onnx, OS voice or cloud). The audio goes to a bus and out to the selected device. VoiceGate ducks or
pauses TTS while the mic is active. A deleted message or a timed-out user removes the queued speech.

**Scripting.** The *Execute Code* sub-action gets `args` (the run context) and a typed `api` object, and
returns `{ ok, setArgs, logs }`. JS runs in-process in QuickJS with a deadline and memory limit. Python and C#
run in pooled worker processes (JSON-RPC over stdio with Content-Length framing); hard timeouts kill the
worker.

## 6. Proposed repository layout (created in P0)

```
apps/desktop/            Tauri 2 shell (thin)
apps/daemon/             streamdend (headless)
crates/den-protocol/      contracts → codegen (SINGLE-WRITER)
crates/den-core/          event bus, action engine, variables, expressions, storage
crates/den-server/        axum REST / WS / MCP / static (ui, overlays, dock)
crates/den-scripting/     JsHost + worker pools
crates/den-audio/ den-tts/ den-stt/
crates/den-chat/          unified chat model, emotes, filters
crates/den-plugin-host/   manifests, sidecar host, declarative plugins
crates/den-ai/            provider abstraction, assistant tools
crates/den-import/        Streamer.bot / SAMMI / Speaker.bot importers
crates/den-testkit/       event simulator, golden payloads
crates/integrations/den-<name>/   twitch, youtube, kick, x, obs, vtubestudio, vmc, …
web/ui/                  React app (main UI, chat window, assistant)
web/kit/                 design tokens + components (also exported as web components for plugins)
web/overlay-runtime/     light TS runtime for OBS browser sources
web/designer/            WYSIWYG editor
web/deck/                SAMMI-style decks + phone deck
sdk/ts/ sdk/python/ sdk/csharp/   generated + hand-written ergonomics
workers/python/ workers/dotnet/
plugins/                 first-party plugins shipped as *external* plugins (dogfooding)
relay/                   optional self-hostable Cloudflare Worker relay
tools/den-cli/            plugin dev CLI (scaffold, dev, validate, pack, sign, publish)
packaging/               aur/, flatpak/, windows/, macos/
spikes/                  throwaway experiments (S-1…)
e2e/                     Playwright + OBS harness
docs/ tasks/ scripts/
```

## 7. Performance budgets (to be confirmed in specs)

| Metric | Budget |
|---|---|
| Idle CPU (engine, UI closed) | < 1% of one core |
| Idle RSS (engine, UI closed) | < 150 MB |
| Event → action start latency | < 10 ms p99 (excluding network) |
| JS execute-code cold start | < 5 ms |
| Python execute-code (warm worker) | < 20 ms round trip |
| Chat message → overlay render | < 100 ms |
| TTS time to first audio (local Piper) | < 300 ms for a short line |
