# Glossary

Shared vocabulary for specs, code and agents. Use these names in types and UI unless a spec says otherwise.

| Term | Meaning |
|---|---|
| **Action** | A named automation: a sub-action tree plus settings (queue, concurrency, random, history). Run by triggers, other actions, the API, decks, or MCP. |
| **Action History** | A record of each run, with before/after variable snapshots. Runs can be replayed. |
| **Alert** | A visual/audio notification played in an overlay in response to an event (sub, follow, super chat…). |
| **Alert queue** | Ordered lanes that play alerts one at a time per lane, with priority, merge (gift bombs), pause/skip/replay. |
| **Args / run context** | The variables available during one action run: event payload, plus values set by earlier sub-actions. |
| **Connector** | Code that talks to an external platform or app (Twitch, OBS, VTube Studio…). Implemented as a plugin (built-in or external). |
| **Deck** | A grid of buttons (SAMMI / Stream Deck style) that run actions. Can be shown on a phone or tablet. |
| **Designer** | The WYSIWYG editor for overlays, alerts and widgets. |
| **Dock** | A web page shown inside OBS as a custom browser dock (e.g. the combined chat). |
| **Event** | A typed occurrence from a source: `{id, source, type, ts, platform?, user?, payload}`. |
| **Filter profile** | A named set of chat filters (bots, words/regex, commands, roles, platforms) applied to a surface (dock, overlay, TTS). |
| **Global variable** | A named value shared across actions. Persisted or temporary. |
| **Lease** | A task's exclusive write claim on the paths in its `paths:` until `lease_expires`. |
| **MCP** | Model Context Protocol. Our built-in server exposes app functions to AI agents as tools and resources. |
| **Overlay** | A web page served by the engine (`/o/<id>`) and shown in OBS as a browser source. |
| **Plugin** | An extension that adds triggers, sub-actions, settings, widgets or panels. Tiers: declarative, sidecar, embedded JS, WASM (later). Built-ins use the same model. |
| **Queue** | Where action runs wait. Default = parallel; named queues can be blocking (one at a time), paused or cleared. |
| **Relay** | An optional, self-hostable web service that gives the local app a public URL for webhooks (Kick, Ko-fi…). |
| **Sidecar** | A plugin or worker running as a separate process, speaking JSON-RPC over stdio. |
| **Source** | Anything that emits events (connector, timer, hotkey, MIDI, file watcher, HTTP/WS endpoint, plugin). |
| **Sub-action** | One step inside an action: a typed operation (`obs.set_scene`, `chat.send`, `code.run`) or control flow (if/else, switch, while, group, delay). |
| **Trigger** | A rule on an action that matches events (type plus conditions) and starts the action. |
| **User variable** | A value stored per viewer (per platform account; cross-platform identity later). |
| **VMC** | Virtual Motion Capture protocol (OSC-based), used by many VTuber apps to share tracking and avatar data. |
| **Voice alias** | A named TTS voice configuration (engine, voice, rate, pitch, device) that rules and users can refer to. |
| **WS API** | Our public WebSocket API (JSON-RPC 2.0, obs-websocket-style handshake and subscriptions). |
