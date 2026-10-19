# Application & Device Integrations (VTuber focus)

**Date:** 2026-10-18. **Scope:** local apps, devices and generic protocols for Streamer.live. Streaming platforms, donation services and core OBS websocket are covered in other research docs.
**Method:** I checked official docs, GitHub repos and the competitor docs (Streamer.bot, SAMMI) with WebSearch/WebFetch on 2026-10-18. Ports and protocols are only as solid as the cited source. Items marked **(inferred)** are my own conclusions or recall that I could not confirm against a source today. Re-verify them before implementing.

**Legend:** OS columns are **W**indows / **L**inux / **M**acOS.
- **Y** = native support.
- **~** = partial (Wine/Proton, web build, or "experimental").
- **N** = not supported.
- **SB** = Streamer.bot native integration. **SAMMI** = SAMMI native, or **ext** for a community extension.

---

## 0. Executive summary

1. **The VTuber stack is mostly Windows-only.** VTube Studio is Windows and macOS. Warudo, VNyan, VSeeFace, Animaze, vMix, VoiceMeeter, SteelSeries Sonar, Mix It Up, Beat Saber and Overwolf are Windows-only.
   - **Linux story:** veadotube mini, PNGTuber Plus, Inochi Session (Flathub), OpenSeeFace, XR Animator, the GoXLR Utility, OpenRGB, Home Assistant, WLED, the VRChat OSC protocol, and MPRIS/PipeWire. Our own app can be the Linux glue.
2. **Two protocols cover most of the 3D VTuber world: VMC (OSC over UDP) and plain OSC.** VSeeFace, Warudo, VNyan, XR Animator, Webcam Motion Capture, Waidayo, VirtualMotionCapture and mocopi bridges all speak VMC. VRChat speaks OSC. A good **built-in VMC/OSC router** gives broad coverage cheaply and also simplifies VTuber setups. Nobody (SB or SAMMI) does this today.
3. **VTube Studio is the single most important app integration.** It is the de-facto Live2D app, has the richest public API (hotkeys, expressions, items, parameter injection, 16 event types), and SB integrates it natively. T.I.T.S., vts-heartrate and similar tools are built on that API.
4. **Warudo needs a small companion plugin.** Its external surface is a WebSocket on `localhost:19190` that plugins (C# mods) open "channels" on. To be first-class, we should publish a tiny Warudo plugin mod that exposes "On Streamer.live trigger" nodes and a "Send to Streamer.live" node. A community Streamer.bot plugin for Warudo already exists, so this approach is proven.
5. **Several "APIs" are unofficial.** Wave Link, SteelSeries Sonar, the PRISM remote protocol, the Elgato Camera Hub and parts of VNyan are reverse-engineered and change between releases. Treat them as Tier 2/3 behind feature flags, with version detection.
6. **Spotify's Web API is now hostile to apps that ship to many users.** Since Feb 2026, Development Mode requires the app owner to have Premium and allows only 5 test users. Extended quota needs a registered business with 250k MAU. Use **bring-your-own client ID**, and prefer OS media sessions (MPRIS, Windows GSMTC) for "now playing".
7. **The long tail should go through generic primitives** rather than hand-written connectors: OSC, VMC, MIDI, WS client/server, HTTP/webhooks, UDP/TCP, MQTT, Home Assistant, run-program and keystrokes. Wayland keystroke injection and global hotkeys require xdg-desktop-portal and libei, and portal support varies by compositor. This matters on the owner's Hyprland machine.

---

## 1. Competitor baseline: what Streamer.bot and SAMMI integrate today

| Area | Streamer.bot (native) | SAMMI (native / ext) |
|---|---|---|
| Broadcasters | OBS, Streamlabs Desktop, Meld Studio ("New"), PolyPop ([stream-apps](https://docs.streamer.bot/guide/stream-apps)) | OBS only ("SAMMI does not support other streaming software"), Meld Studio listed under integrations ([docs](https://sammi.solutions/docs)) |
| VTuber | VTube Studio, T.I.T.S. | VTube Studio (ext), veadotube mini (ext "VeadoSAMMI"), VTuber Plus (ext) ([extensions](https://sammi.solutions/extensions)) |
| Audio | Voicemod, Elgato Wave Link (incl. Wave Link 3 triggers) | Voicemod (V3 ext), Spotify (ext), foobar2000 (ext), OBS audio monitoring (ext) |
| Devices | Elgato Stream Deck, Elgato Camera Hub, Elgato Control Center sub-actions | Stream Deck, Elgato lights (ext), Hue (ext), LIFX (ext), QLC+ DMX (ext) |
| Heart rate | Pulsoid, HypeRate.io | Pulsoid |
| Interactive / peers | Crowd Control, Lumia Stream, Speaker.bot, StreamFog, IFTTT | Crowd Control, Mix It Up, Discord (ext), VoiceAttack (ext), LiveSplit (ext), Palworld (ext) |
| Full list | 27 integrations: Crowd Control, Discord, DonorDrive, Camera Hub, Stream Deck, Wave Link, Fourthwall, HypeRate, IFTTT, Ko-Fi, Lumia, Pally.gg, Patreon, Pulsoid, Shopify, Speaker.bot, StreamElements, SB Website, StreamFog, Streamlabs, StreamLoots, Throne, TipeeeStream, T.I.T.S., TreatStream, VoiceMod, VTube Studio ([integrations](https://docs.streamer.bot/guide/integrations)) | Crowd Control, Stream Deck, Meld Studio, Mix It Up, OBS, Pulsoid, Twitch, Voicemod, YouTube; plus community extensions |

Notable gaps in both, which are our differentiation opportunity:
- No native support for Warudo, VNyan, VSeeFace/VMC, VRChat OSC, Home Assistant, OpenRGB, GoXLR, VoiceMeeter or PipeWire.
- No Linux story at all. Streamer.bot is a .NET Windows app. SAMMI is Windows-first **(inferred)**.

---

## 2. VTuber software (highest priority)

### 2.1 Overview matrix

| App | W | L | M | Control surface | Default port / discovery | Auth / pairing | SB | SAMMI |
|---|---|---|---|---|---|---|---|---|
| **VTube Studio** | Y | ~ Proton **(inferred)** | Y | Public WebSocket JSON API + Event API | `ws://localhost:8001`; UDP broadcast on **47779** every 2 s with port/instanceID ([API](https://github.com/DenchiSoft/VTubeStudio)) | Plugin token: user approves a popup once, then the token is reused | Y | ext |
| **Warudo** | Y (Win10, Steam app 2079120) | ~ Proton; capture, Spout and NDI broken ([ProtonDB](https://www.protondb.com/app/2079120)) | N ([CrossOver](https://www.codeweavers.com/compatibility/crossover/warudo)) | C# plugin mods (Steam Workshop). Plugins open **WebSocket channels** on `ws://localhost:19190`. HTTP callbacks on `:19052`. Stream Deck plugin, MIDI, VMC in/out, DMX ([external callbacks](https://docs.warudo.app/docs/scripting/api/external-callbacks)) | 19190 (WS), 19052 (HTTP) | None documented. Channel selected by a `Channel` header | N (community Warudo plugin exists) | N |
| **VNyan** | Y | N **(inferred)** | N | WebSocket "receiver": text commands fire "WebSocket Command" graph nodes. C# plugin SDK. VMC in/out | `ws://localhost:8000/vnyan` ([community wiki](https://github-wiki-see.page/m/NeocortexVT/VNyanDocFork/wiki/How-To)); REST `127.0.0.1:8069` reported by a third party ([vnyan-mcp](https://glama.ai/mcp/servers/seth-rah/vnyan-mcp/schema)) | None | N | N |
| **VSeeFace** | Y (Win8+ x64) | ~ Wine; no virtual cam or Spout | N ([site](https://www.vseeface.icu/)) | **No API.** VMC send/receive, global hotkeys, OpenSeeFace input | VMC ports user-set (39539/39540 typical) | None | N | N |
| **OpenSeeFace** (tracker) | Y | Y (with an onnxruntime workaround) | ~ **(inferred)** | UDP tracking packets (Python/ONNX, BSD-2) ([repo](https://github.com/emilianavt/OpenSeeFace)) | UDP 11573 **(inferred; conventional default)** | None | N | N |
| **VMC Protocol** (spec) | n/a | n/a | n/a | OSC over UDP. Roles: Marionette, Performer, Assistant ([spec](https://protocol.vmc.info/english)) | 39539 (Performer to Marionette), 39540 (Assistant to Performer) | None | N | N |
| **veadotube mini / veadotube** | Y | Y **(inferred from itch listing)** | Y **(inferred)** | WebSocket (channels `instance`, `nodes`). HTTP GET on the same address. CLI `-i` ([API](https://veado.tube/docs/tech/api/)) | Discovery: JSON files in `<data>/instances/` (ignore if timestamp > 10 s old). Example `ws://127.0.0.1:2424?n=<client name>` ([instance](https://veado.tube/docs/tech/api/instance/)) | Client must pass a display name `?n=`; unnamed clients are rejected | N | ext |
| **PNGTuber Plus** | Y | Y | Y ([itch](https://kaiakairos.itch.io/PNGtuber-plus)) | Global hotkeys only. Legacy Stream Deck hook is "not working" per the dev ([thread](https://itch.io/t/3633134/stream-deck-integration-not-working)) | none | n/a | N | N |
| **PNGTubeRemix** | Y | Y **(inferred)** | Y **(inferred)** | Open source (Godot). Users report HTTP web-request triggers. Undocumented ([itch](https://mudkipworld.itch.io/pngtube-remix)) | unknown | n/a | N | N |
| **Animaze** | Y | N **(inferred)** | N **(inferred)** | WebSocket JSON API ("Settings > Animaze API > Enabled") ([API](https://holotechconfluence.atlassian.net/wiki/spaces/HD/pages/157941761/Animaze+API)) | `ws://localhost:9000` (configurable) | Enable toggle only | N | N |
| **Inochi Session** (Inochi2D) | Y **(inferred)** | **Y** (Flathub x86_64/aarch64) | Y **(inferred)** | No external control API documented. Tracking input from VMC, OpenSeeFace and VTS-phone **(inferred)**. BSD-2; "under heavy development, not ready for production" ([repo](https://github.com/Inochi2D/inochi-session)) | n/a | n/a | N | N |
| **VRChat** | Y | ~ Proton **(inferred)** | N | OSC: `/avatar/parameters/*`, `/avatar/change`, `/input/*`, `/chatbox/input`. OSCQuery over mDNS `_oscjson._tcp` ([OSC](https://docs.vrchat.com/docs/osc-overview), [params](https://docs.vrchat.com/docs/osc-avatar-parameters), [OSCQuery lib](https://www.nuget.org/packages/VRChat.OSCQuery/0.0.6)) | UDP in **9000**, out **9001**; `--osc=in:ip:out` | None | N | N |
| **XR Animator** | Y | Y (0.19.0 "Windows and Linux version") | ~ web | VMC sender (configurable port), webcam mocap with MediaPipe ([release](https://github.com/ButzYung/SystemAnimatorOnline/releases/tag/XR-Animator_v0.19.0)) | user-set | None | N | N |
| **iFacialMocap / FaceMotion3D** (iOS) | phone to PC | phone to PC | phone to PC | UDP blendshape stream to PC apps | UDP 49983 **(inferred)** | None | N | N |
| **VTS phone app** (iOS/Android), **MeowFace** | phone to PC | | | UDP tracking stream. Third-party PCs can request the stream **(inferred)** | UDP 21412 **(inferred)** | None | N | N |
| **mocopi** (Sony) | phone to PC | | | UDP "mocopi (UDP)" format. Unity/Unreal/MotionBuilder receiver plugins ([spec](https://www.sony.co.jp/en/Products/mocopi-dev/en/documents/Home/TechSpec.html)) | UDP **12351**; IPv4 only, unencrypted | None | N | N |
| **Beat Saber** (mods) | Y | ~ Proton | N | `beatsaber-http-status` (WS + HTTP) ([repo](https://github.com/denpadokei/beatsaber-http-status)); `AnotherStatusSocket` (updated Jul 2026) ([repo](https://git.gay/TheBlackParrot/BS_AnotherStatusSocket)) | 6557 **(inferred)**; `ws://localhost:13339` | None | N | N |
| **ChilloutVR** | Y | ~ | N | OSC through a community mod **(inferred)** | mod-defined | None | N | N |
| **3tene, Luppet** | Y | N | N | Hotkeys; VMC in some editions **(inferred)** | | | N | N |
| **Kalidoface** | web | web | web | Browser app (Kalidokit). No control API **(inferred)** | | | N | N |
| **T.I.T.S.** (throwing) | Y **(inferred)** | N | N | Local API (must enable in settings). Drives VTS through VTS's API ([MIU wiki](https://wiki.mixitupapp.com/en/services/tits)) | 42069 **(inferred, unverified)** | Enable toggle | **Y** | N |
| **vts-heartrate** | Y | ~ | Y | VTS plugin (MIT). Reads Pulsoid, HypeRate, ANT+ and Fitbit, and injects `VTS_Heartrate_*` params ([itch](https://skeletom-ch.itch.io/vts-heartrate)) | n/a | VTS token | (via VTS) | N |
| **Pulsoid** (cloud) | any | any | any | OAuth2 (Device Flow recommended for desktop apps). Realtime WS via `@pulsoid/socket` ([docs](https://docs.pulsoid.net/)) | `wss://dev.pulsoid.net/api/v1/data/real_time` **(inferred)** | OAuth2 / manual token | Y | Y |
| **HypeRate** (cloud) | any | any | any | Phoenix-channels WebSocket **(inferred)**. SB exposes HeartRatePulse and TwitchClipCreated ([SB](https://docs.streamer.bot/api/websocket/events/hype-rate)) | `wss://app.hyperate.io/socket/websocket` **(inferred)** | API key (by application) **(inferred)** | Y | N |

**Not verified:**
- **Karasubot:** I found no public source for this "VTS throwing" bot. Ask the owner for a link.
- **PolyPop:** SB still lists it, but I could not confirm its current status.

### 2.2 Per-app detail (triggers / sub-actions to expose)

#### VTube Studio. Tier 1. Docs: [API README](https://github.com/DenchiSoft/VTubeStudio), [Events](https://github.com/DenchiSoft/VTubeStudio/blob/master/Events/README.md)

**Libraries:**
- TS: `vtubestudio` on npm ([Hawkbat/VTubeStudioJS](https://github.com/Hawkbat/VTubeStudioJS)).
- Rust: the `vtubestudio` crate (walfie/vtubestudio-rs) **(inferred, verify)**.
- The protocol is simple JSON over WebSocket, so writing our own client is trivial.

**Sub-actions:**
- `HotkeyTriggerRequest`. Queue of 32. Populate a dropdown from `HotkeysInCurrentModelRequest`.
- `ExpressionActivationRequest`.
- `ModelLoadRequest`. 2 s global cooldown.
- `MoveModelRequest`: position, rotation, size, with a time.
- `ColorTintRequest` on ArtMeshes (rainbow/flash effects).
- Items: list, load, unload, animation control, move, sort, pin.
- Parameters: `InjectParameterDataRequest` and custom `ParameterCreationRequest`. These feed heart rate, audio level, chat sentiment and so on into the model.
- `SetCurrentModelPhysicsRequest` (physics multipliers).
- `NDIConfigRequest`. Needs the `ControlNDISettings` permission and has a 3 s cooldown.
- Post-processing presets.

**Triggers (Event API):**

| Event | Notes |
|---|---|
| `ModelLoadedEvent` | |
| `TrackingStatusChangedEvent` | Face or hand lost / found |
| `BackgroundChangedEvent` | |
| `ModelConfigChangedEvent` | |
| `ModelMovedEvent` | |
| `ModelOutlineEvent` | About 15 fps; use for "throw at model" overlays |
| `HotkeyTriggeredEvent` | |
| `ExpressionToggledEvent` | Beta |
| `ModelAnimationEvent` | Includes custom animation events. Lets the rig fire actions. |
| `ItemEvent` | added, removed, dropped, clicked, locked |
| `ModelClickedEvent` | Hit ArtMesh details |
| `PostProcessingEvent` | |
| `Live2DCubismEditorConnectedEvent` | |
| `ArtMeshTrackingEvent`, `ArtMeshOutlineEvent` | Beta |

**Notes:**
- Multiple instances are supported; the UDP broadcast includes `instanceID`, port and window title. Let the wizard auto-discover them.
- Ignore unknown fields. API version stays "1.0" while changes remain backward compatible.

#### Warudo. Tier 1, via a companion plugin.

**Native surfaces:**
- C# plugin SDK ([scripting overview](https://docs.warudo.app/docs/scripting/overview)).
- `CreateWebSocketChannel`: bidirectional channel on `ws://localhost:19190`, keyed by a `Channel` handshake header.
- `CreateExternalCallback`: `http://localhost:19052/callback/{id}`.
- Raw WS messages without a header reach plugins as `WebSocketRawMessageEvent` ([external callbacks](https://docs.warudo.app/docs/scripting/api/external-callbacks)).
- Built-in Stream Deck nodes (On Stream Deck Trigger/Toggle/Message) ([docs](https://docs.warudo.app/docs/blueprints/miscellaneous/streamdeck-integration)).
- VMC receiver (default 39539) and VMC sender ([VMC](https://docs.warudo.app/docs/mocap/vmc)).
- DMX lighting.
- Community plugins listed in the docs include a **Streamer.bot integration** and Twitch emote droppers. "VDeck" is a third-party WebSocket browser remote ([booth](https://booth.pm/ja/items/8088698)).

**Plan:** ship a small open-source "Streamer.live for Warudo" plugin mod. It is necessarily C#/Unity, but it is a one-time shim of a few hundred lines **(inferred)**. It would:
1. Open channel `streamerlive` on 19190.
2. Expose blueprint nodes: *On Streamer.live Action (name, args)*, *Send Event to Streamer.live*, *Set/Get Streamer.live variable*.
3. Report the list of blueprints and assets so our UI can offer dropdowns.

**What this gives us:**
- **Sub-actions:** trigger blueprint by name with args, toggle props/assets, throw props, change expression or animation **(inferred: Warudo has built-in prop/interaction nodes)**.
- **Triggers:** any blueprint calling *Send Event*, plus tracking-lost events.

**Without the plugin**, we can still do VMC blendshape injection and Stream Deck-style messages.

#### VNyan. Tier 1 (simple), Windows-only.

**Surfaces:**
- WebSocket receiver. Settings > Misc > WebSockets > Start Receiver, port 8000, path `/vnyan`.
- Each text message fires matching "WebSocket Command" nodes in VNyan's node graph ([how-to](https://github-wiki-see.page/m/NeocortexVT/VNyanDocFork/wiki/How-To)).
- C# plugins ([plugins wiki](https://github.com/Suvidriel/VNyanDoc/wiki/Plugins)).
- Community WebSocket plugin ([Shirodork](https://github.com/Shirodork/Vnyan-Websocket-Plugin)).

**Caveats:**
- VNyan 1.5.3 changed its WebSocket library and broke older plugins ([devlog](https://jayo-exe.itch.io/obs-plugin-for-vnyan/devlog/934933/v041-vnyan-v153-support)). Use version detection.
- Official docs are a community wiki, last updated April 2025.

**Integration:**
- **Sub-action:** "Send VNyan command (text + optional JSON)".
- **Triggers:** VNyan can call out to *our* WebSocket/HTTP server from its graph **(inferred: VNyan has WS/HTTP request nodes)**. Our API-first server makes that easy.

#### VSeeFace. Tier 2 (via VMC + hotkeys).

- No API. Development appears slow; latest release is v1.13.38c5 ([site](https://www.vseeface.icu/)).
- **Integration:**
  - Act as a VMC **Assistant**: send `/VMC/Ext/Blend/Val` + `/Blend/Apply` to its VMC receiver for expressions.
  - Or simulate its global hotkeys.
  - Receive its VMC output to get tracking status.
- **Port warning:** send and receive ports must differ or "very strange things may happen". The wizard must check for this.

#### VMC Protocol (generic). Tier 1. [Spec](https://protocol.vmc.info/english)

**Implementation:** build on `rosc` (Rust OSC) with typed messages:

| Message | Use |
|---|---|
| `/VMC/Ext/OK` | Loaded / calibration / tracking status. **Trigger:** tracking lost / found |
| `/VMC/Ext/T` | Time |
| `/VMC/Ext/Root/Pos` | Root transform |
| `/VMC/Ext/Bone/Pos` | Bone transforms |
| `/VMC/Ext/Blend/Val` + `/Blend/Apply` | Blendshapes |
| `/VMC/Ext/Set/Calib/*` | Calibration |
| `/VMC/Ext/Set/Config` | Load config |
| `/VMC/Ext/Set/Shortcut` | V3.1 |
| `/VMC/Ext/Cam`, `/Light`, `/Key`, `/Midi/*` | Camera, light, key, MIDI |
| `/VMC/Thru/*` | Pass-through |

**Killer feature: a VMC router/multiplexer.**
- Receive from one tracker (XR Animator, Webcam Motion Capture, a mocopi bridge, VSeeFace).
- Fan out to several apps (Warudo + VNyan + VRChat bridge).
- Merge or override blendshapes from action triggers ("on raid, force `Joy=1.0` for 3 s").
- Record and replay.
- This removes a whole class of "which app sends to which port" confusion **(inferred: community tools such as VMCProtocolMultiplexer exist, which shows demand)**.

**Other libraries:** Python `vmcp` ([docs](https://vivi90.codeberg.page/python-vmcp/vmcp.html)); EVMC4U/VMC4UE/VMC4B for engines.

#### veadotube mini / veadotube. Tier 1 (cross-platform PNGtuber).

- **Discovery:** read the instance files (`mini-<hex>-<hex>`). Each JSON file holds timestamp, name, id, version, server address. Ignore stale files (> 10 s).
- **Connection:** connect with `?n=Streamer.live`.
- **Channels:**
  - `instance:` returns info.
  - `nodes:` with `{"event":"list"|"listen"|"unlisten"|"payload"}`.
- **Fixed nodes in mini:**
  - boolean `mini`: microphone on/off.
  - `stateEvents` `mini`: avatar state (expression/state switching).

([nodes](https://veado.tube/docs/tech/api/nodes/), [websocket help](https://veado.tube/help/docs/websocket))

**Exposed:**
- **Sub-actions:** set state, toggle mic.
- **Triggers:** state changed, mic toggled (via `listen`).

#### Animaze. Tier 2.

- **WS JSON actions** ([API](https://holotechconfluence.atlassian.net/wiki/spaces/HD/pages/157941761/Animaze+API)):
  - Avatars and scenes: `LoadAvatar`, `LoadScene`, `GetAvatars`, `GetScenes`.
  - Emotes, special actions, poses, idles: `GetEmotes`/`TriggerEmote`, `TriggerSpecialAction`, `TriggerPose`, `TriggerIdle`.
  - Quickscenes: load/save.
  - Tracking and camera: `CalibrateTracker`, `Broadcast`, Set/Get camera transform and FOV, Set/Get avatar transform.
  - Overrides: `SetOverride`.
  - ChatPal: `SendMessageToAnimazeChatPal`.
- **Triggers:** mostly planned. ChatPal speech start/end is on the "next-version" branch only.
- **Separate surface:** a UDP "Programmable Tracker" input.
- Community lib: AnimazePy.

#### VRChat (OSC). Tier 1, via the generic OSC module plus a VRChat preset.

- **Sub-actions:**
  - Set avatar parameter (bool/int/float).
  - `/input/*` (jump, move, Voice toggle).
  - `/chatbox/input` (text plus "send immediately" and "notify" flags), for chat relay or TTS captions.
- **Triggers:** `/avatar/change` and any parameter change on 9001.
- **Discovery:** use OSCQuery (mDNS `_oscjson._tcp`) to discover VRChat's actual ports and the avatar parameter list.
- **Caveat:** OSCQuery can report stale initial values ([bug](https://feedback.vrchat.com/bug-reports/p/oscquery-broadcasts-wrong-parameter-values)).

#### Inochi2D / Inochi Session. Tier 3 now, strategic for Linux.

- The only open-source, Linux-native Live2D-class puppet app (BSD-2, Flathub). It has no control API.
- **Integration:** be a good tracking/VMC source for it. Offer to contribute an OSC or WebSocket control API upstream **(inferred opportunity)**.
- **Status risk:** "not ready for production".

#### Heart rate (Pulsoid / HypeRate). Tier 1 (cheap, very popular with VTubers).

- **Triggers:** HR value (throttled), HR crosses threshold, connected/disconnected.
- **Built-in recipe:** "HR to VTS parameter" (replaces part of vts-heartrate) and "HR to VRChat OSC parameter".

#### T.I.T.S. Tier 2.

- Throw items, activate T.I.T.S. triggers, list items.
- SB already does this ([SB TITS](https://docs.streamer.bot/guide/integrations/tits)), so it is needed for parity.
- Our own "throw at VTS model" could later be done natively with VTS `ModelOutlineEvent` plus a browser overlay **(inferred)**.

#### Tracking sources: iFacialMocap, VTS phone, MeowFace, mocopi, OpenSeeFace, XR Animator

These are not control targets. They matter for the **wizard's routing graph**: which tracker feeds which renderer on which port. They also give us optional "tracking lost" or "face found" triggers if we sit in the middle as a relay.

---

## 3. Broadcasting apps, OBS plugins, NDI

| App | W | L | M | Control surface | Port | Auth | SB | SAMMI | Notes |
|---|---|---|---|---|---|---|---|---|---|
| **OBS Studio** | Y | Y | Y | obs-websocket v5 (covered elsewhere) | 4455 | Password | Y | Y | Add a generic **CallVendorRequest** sub-action to unlock plugin control (below) |
| **Streamlabs Desktop** | Y | N | Y | JSON-RPC 2.0 over SockJS WebSocket; also a named pipe ([API ref](https://dev.streamlabs.com/reference/streamlabs-obs-api), [docs repo](https://github.com/stream-labs/streamlabs-obs-api-docs)) | 59650 | Token from Settings > Remote Control | Y | N | Python: PySLOBS |
| **Meld Studio** | Y (10/11) | N | Y (12+) ([download](https://meldstudio.co/download)) | Qt **WebChannel** over WebSocket ([WebChannelAPI.md](https://github.com/MeldStudio/streamdeck/blob/main/WebChannelAPI.md)) | `ws://127.0.0.1:13376` (fixed) | None | Y (new) | Y | Methods: `showScene`, `setStagedScene`, `toggleLayer`, `toggleEffect`, `toggleMute`, `toggleMonitor`, `setGain`, `toggleStream`, `toggleRecord`, `sendCommand("meld.screenshot"/"meld.recordClip")`, `setProperty`. Signals: `sessionChanged`, `isStreamingChanged`, `isRecordingChanged`, `gainUpdated` |
| **vMix** | Y | N | N | HTTP `/api/?Function=...` + XML state; TCP API with `SUBSCRIBE TALLY` ([HTTP](https://www.vmix.com/help24/DeveloperAPI.html), [TCP](https://sdapi01.vmix.com/help/TCPAPI.html)) | 8088 HTTP, 8099 TCP | Optional web password | N | N | Pro/church/esports market. Trivial protocol |
| **XSplit Broadcaster** | Y | N | N | XJS plugin framework; no documented public remote API ([XJS](https://cdn.jsdelivr.net/npm/@dcefram/xjs@0.2.4/README.md)) | (9999 in an XJS example) | n/a | N | N | Low priority |
| **PRISM Live Studio** | Y | N | Y | Unofficial reverse-engineered remote protocol, v4.0.0+ ([prism_connect](https://pub.dev/packages/prism_connect)) | From the in-app QR code | Pairing via QR | N | N | Moving to subscriptions ([notice](https://guide.prismlive.com/lens/announcement/general/upcoming-subscription-model-for-prism-live-studio)). Tier 3 |
| **Twitch Studio** | | | | **Discontinued 2024-05-30** ([Tubefilter](https://www.tubefilter.com/2024/05/20/twitch-shuts-down-twitch-studio/)) | | | | | Skip |
| **PolyPop** | Y | N | N | WebSocket plugin (per SB) | ? | ? | Y | N | Status unverified. Skip unless the owner wants it |

**OBS plugins that matter for automation** (all reached through obs-websocket, so no separate connector):

| Plugin | How to drive | Status |
|---|---|---|
| **Advanced Scene Switcher** | Vendor `AdvancedSceneSwitcher`, request `AdvancedSceneSwitcherMessage` (fires macros listening for that text). Emits vendor events `AdvancedSceneSwitcherEvent` back ([forum](https://obsproject.com/forum/threads/advanced-scene-switcher.48264/page-210)) | Active. Expose as "Send ASS message" sub-action + "ASS event" trigger |
| **Move transition** (Exeldro) | Enable or change "Move" filters via `SetSourceFilterEnabled`/`SetSourceFilterSettings` **(inferred)** | Active. Wrap as a "Move source to..." preset |
| **Source Record** | Filter enable/settings to start/stop per-source recording **(inferred)** | Active |
| **obs-shaderfilter** | Filter settings (uniforms) via obs-websocket **(inferred)** | Active (Exeldro fork) |
| **StreamFX** | Public builds stale or paywalled, breaks on OBS 30+; FreeFX fork abandoned ([reviews](https://obsproject.com/forum/resources/streamfx.578/review/3837)) | **Do not depend on it** |
| **Aitum Vertical / Multistream** | obs-websocket vendor requests for the vertical canvas, streams and recording **(inferred)** | Active. Verify vendor names |

**NDI.** NDI 6 SDK runs on Win/Linux/macOS. Rust bindings:
- `grafton-ndi`: NDI 6, all three OSes, but its 0.11.0 docs build failed ([docs.rs](https://docs.rs/grafton-ndi)).
- `ndi-sdk-sys`: tested against SDK 6.2/6.3 ([docs.rs](https://docs.rs/ndi-sdk-sys)).

Use cases: publish our overlay renderer as an NDI source; NDI tally/discovery triggers. VTS and Warudo already output NDI. Licensing and redistribution terms of the NDI runtime need a check **(inferred)**. Tier 3.

---

## 4. Audio and "now playing"

| App | W | L | M | Control surface | Port / discovery | Auth | SB | SAMMI | Libs / notes |
|---|---|---|---|---|---|---|---|---|---|
| **Voicemod** | Y | N | Y (cask 3.16.x, Big Sur–Tahoe) ([brew](https://formulae.brew.sh/cask/voicemod)) | Control API, WebSocket JSON (`action`/`id`/`payload`) ([docs](https://control-api.voicemod.net/getting-started/)) | Try in order: 59129, 20000, 39273, 42152, 43782, 46667, 35679, 37170, 38501, 33952, 30546 (`/v1`) | `registerClient` with **clientKey** (apply via form) | Y | Y | Unverified whether the Control API works on macOS. **Sub-actions:** `loadVoice`, `getVoices`, toggle voice changer / hear-myself, memes/soundboard, background **(last three inferred from API ref)** |
| **VoiceMeeter** (Basic/Banana/Potato) | Y | N | N | Remote API DLL `VoicemeeterRemote64.dll` (C header + PDF) ([forum](https://forum.vb-audio.com/viewtopic.php?p=494)); VBAN-TEXT over UDP for network remote **(inferred)** | n/a (DLL) | none | N | N | Python `voicemeeter-api` ([pypi](https://pypi.org/project/voicemeeter-api/2.4.8)); Go wrapper. Strip/bus gain, mute, routing (A1–A5/B1–B3), macro buttons; "parameter dirty" polling for triggers |
| **GoXLR (official app)** | Y | N | Y **(inferred)** | none public | | | N | N | |
| **GoXLR Utility** (open source, Rust, MIT) | Y | **Y** | ~ experimental | HTTP + WebSocket API, JSON-Patch status deltas; CLI ([repo](https://github.com/GoXLR-on-Linux/goxlr-utility)) | `http://localhost:14564`, `ws://…/api/websocket` ([py docs](https://goxlr.readthedocs.io/en/stable/api/socket.html)) | none (LAN access opt-in) | N | N | Faders, mutes, FX, samples, profiles, lighting. **Linux showcase**; write a native Rust client |
| **Elgato Wave Link 3.x** | Y (3.2.9) | N | Y (3.2.2) | Unofficial local WebSocket JSON-RPC ([ElgatoWavePy](https://github.com/Noctinae/ElgatoWavePy/wiki/Available-Commands)); npm SDK by darrellvs | 1824 (community-reported) | none | Y (incl. WL3 triggers such as Focused App Changed ([SB](https://docs.streamer.bot/api/triggers/elgato/wave-link-3/system/focused-app-changed))) | N | Wave Link 3 is free and works with non-Elgato mics ([release notes](https://help.elgato.com/hc/en-us/sections/4913442828941-Wave-Link-Release-Notes)). The protocol changed between v2 and v3 **(inferred)** |
| **SteelSeries Sonar** | Y | N | N | Unofficial local REST. Discover via GG `coreProps.json`, then `/subApps`, then Sonar `webServerAddress`; port changes every restart ([sonar-rev](https://github.com/wex/sonar-rev)) | dynamic | none | N | N | Rust crate `steelseries-sonar` ([docs.rs](https://docs.rs/steelseries-sonar)); Python `steelseries-sonar-py`. Volume/mute per channel (game, chat, media, aux, mic), ChatMix, streamer mode |
| **PipeWire / PulseAudio** | | Y | | Native libs (`pipewire-rs`, `libpulse-binding`), `wpctl`/`pactl` **(inferred crates)** | | | N | N | Per-app volume/mute, default device, **create virtual sinks/loopbacks**: a VoiceMeeter-like routing story for Linux |
| **Windows per-app volume** | Y | | | WASAPI `IAudioSessionManager2` / `ISimpleAudioVolume` via `windows` crate; default-device switching needs undocumented `IPolicyConfig` **(inferred)** | | | partial **(inferred)** | N | |
| **macOS per-app volume** | | | ~ | No public per-app mixer API. Core Audio process taps (14.2+) **(inferred)** | | | | | Weakest platform |
| **Spotify** | cloud | cloud | cloud | Web API (OAuth PKCE). **Feb 2026 Dev Mode: owner must have Premium, 5 test users/app; extended quota = registered business + 250k MAU** ([Spotify blog](https://developer.spotify.com/blog/2026-02-06-update-on-developer-access-and-platform-security), [TechCrunch](https://techcrunch.com/2026/02/06/spotify-changes-developer-mode-api-to-require-premium-accounts-limits-test-users/), [migration](https://developer.spotify.com/documentation/web-api/tutorials/february-2026-migration-guide)); July 2026: 25 client IDs per developer, quota pooled per account ([changelog](https://developer.spotify.com/documentation/web-api/references/changes/july-2026)) | | OAuth | N (community) | ext | Require **BYO client ID** per user; fall back to MPRIS/GSMTC for now-playing and media keys. Song requests need the Web API (queue) |
| **YouTube Music Desktop (ytmdesktop v2)** | Y | Y | Y | Companion Server REST + Socket.IO realtime ([wiki](https://github.com/ytmdesktop/ytmdesktop/wiki/v2-%E2%80%90-Companion-Server-API-v1)) | `http://localhost:9863/api/v1` (IPv4!) | Code-approval flow, then token | N | N | `state-update` events; commands: play/pause/seek/volume/next/like/changeVideo; good song-request target |
| **Pear Desktop** (ex th-ch/youtube-music) | Y | Y | Y | "API Server" plugin (REST) ([repo](https://github.com/pear-devs/pear-desktop)) | 26538 (per Songify guide) | optional | N | N | |
| **Tidal** | | | | No local API. Use OS media session; Linux `tidal-hifi` exposes MPRIS **(inferred)** | | | N | N | Tier 3 |
| **VLC** | Y | Y | Y | Lua HTTP interface (`/requests/status.json`), also MPRIS on Linux **(inferred details)** | 8080 | Password (basic auth) | N | N | |
| **foobar2000** | Y | N | ~ (v2 mac) | `beefweb` plugin REST **(inferred)** | 8880 **(inferred)** | optional | N | ext | Tier 3 |
| **MPRIS (Linux D-Bus)** | | Y | | `org.mpris.MediaPlayer2.*` via `zbus` / `mpris` crate **(inferred crates)** | session bus | none | N | N | **Generic now-playing + transport control for every Linux player** |
| **Windows GSMTC** | Y | | | `GlobalSystemMediaTransportControlsSessionManager` (WinRT) via `windows` crate **(inferred)** | | none | N | N | Generic now-playing on Windows (Spotify, browsers, etc.) |
| **macOS MediaRemote** | | | ~ | Private framework, **entitlement-gated since macOS 15.4**; workaround `mediaremote-adapter` / `media-control` (runs via `/usr/bin/perl`) ([repo](https://github.com/ungive/mediaremote-adapter), [KM forum](https://forum.keyboardmaestro.com/t/beware-upgrading-to-macos-15-4-if-you-need-now-playing-data/40285)) | | | | | Fragile; Apple may close it |

---

## 5. Hardware and control surfaces

| Device / app | W | L | M | Surface | Notes | SB | SAMMI |
|---|---|---|---|---|---|---|---|
| **Elgato Stream Deck (official app)** | Y | N | Y | Plugin SDK 2.0: Node.js 24+, Stream Deck 7.1+, `@elgato/cli` ([SDK](https://docs.elgato.com/sdk/)). Plugin talks to the SD app over a local WebSocket | Ship **our own Stream Deck plugin** that calls our WS API (both competitors do). Dynamic action lists from our API | Y | Y |
| **OpenDeck** | Y | **Y** | Y | Rust + TS, GPL-3, 2.3k stars. Runs **Elgato SDK plugins and OpenAction API plugins** ([repo](https://github.com/nekename/OpenDeck)) | **Linux path for Stream Deck users.** Make our plugin OpenDeck-compatible (test it) | N | N |
| **StreamController** | N | Y | N | GTK4, Python plugins, Flathub ([flathub](https://flathub.org/apps/com.core447.StreamController)) | Optional native plugin (Tier 3) | N | N |
| **Boatswain / streamdeck-ui** | | Y | | GNOME app / legacy | Limited plugin support | | |
| **Direct HID** | Y | Y | Y | Rust `elgato-streamdeck` crate **(inferred)** | "Built-in deck" mode with no third-party app. Conflicts if the Elgato app holds the device | | |
| **Loupedeck / Logitech MX Creative Console** | Y | N | Y | Logi Actions SDK + Plugin Tool (C#/.NET **(inferred)**) ([dev](https://futureisnow.logitech.com/en-au/software/marketplace/developer.html)) | Tier 3 plugin | N | N |
| **Touch Portal** | Y | **Y** | Y ([site](https://www.touch-portal.com/download/)) | Plugin API: TCP socket, JSON lines, `entry.tp` manifest, API "13.0" ([API](https://www.touch-portal.com/api)) | 127.0.0.1:12136 **(inferred)**. Ship a TP plugin (actions + states + connectors/sliders) | N | N |
| **MIDI controllers** | Y | Y | Y | `midir` (WinMM/WinRT, ALSA/JACK, CoreMIDI) **(inferred crate)** | Triggers: note/CC/PC; sub-actions: send MIDI (motorized faders, LED feedback). Virtual ports are native on Linux/macOS; Windows needs loopMIDI or the new Windows MIDI Services loopback **(inferred)** | Y **(inferred)** | ? |
| **Elgato Key Light / Light Strip** | (LAN) | | | HTTP REST, no auth: GET/PUT `/elgato/lights`, `/elgato/accessory-info`, `/elgato/lights/settings`; temperature in **mireds** ([write-up](https://www.ntpro.nl/blog/archives/3613-Postman-and-the-Elgato-Key-Light-Air-API.html)) | Port 9123; mDNS `_elg._tcp` **(inferred)**. Firmware quirks: use HTTP/1.0 for GET, `Connection: close` for PUT, avoid rapid reconnects ([elgato-cli](https://git.sub-net.at/tina.schellander/elgato-cli)) | Y (Control Center) | ext |
| **Elgato Camera Hub** (Facecam) | Y | N | Y | Undocumented local channel (SB has only a "Connected" trigger documented) ([SB](https://docs.streamer.bot/guide/integrations/elgato-camera-hub)) | Tier 3. On Linux use V4L2 controls (`v4l2-ctl`) for any UVC cam **(inferred)** | Y | N |
| **OpenRGB** (Razer/Corsair/Logitech/ASUS/etc.) | Y | **Y** | Y | SDK server: binary TCP, header magic "ORGB", protocol v5 = OpenRGB 1.0 ([SDK doc mirror](https://gitea.marinkevich.ru/grayhook/OpenRGB/src/branch/master/Documentation/OpenRGBSDK.md)) | Port 6742. Rust crates `openrgb` / `openrgb2` **(inferred; verify v5 support)**. Replaces Windows-only vendor SDKs (Chroma, iCUE, LGS) | N | N |
| **Game controllers** | Y | Y | Y | `gilrs` / SDL3 **(inferred)** | Button-combo triggers (e.g. "controller disconnected") | N | N |

---

## 6. Smart home and lighting

| System | Surface | Port / discovery | Pairing | Rust/TS libs | SB | SAMMI |
|---|---|---|---|---|---|---|
| **Philips Hue** | CLIP v2 HTTPS REST; SSE event stream `/eventstream/clip/v2`; Entertainment API = DTLS-PSK over UDP for high-rate sync **(inferred details; official docs are login-gated: [developers.meethue.com](https://developers.meethue.com/develop/hue-api-v2/))** | Bridge 443; mDNS `_hue._tcp`; UDP 2100 for Entertainment **(inferred)** | Press link button, then POST `/api` with `generateclientkey:true`; header `hue-application-key` **(inferred)** | openhue OpenAPI spec **(inferred)** | N | ext |
| **Nanoleaf** (Wi-Fi panels) | OpenAPI REST `/api/v1/{token}/…`; SSE events and UDP external control **(inferred)** | 16021; mDNS `_nanoleafapi._tcp` **(inferred)** | Hold power 5–7 s, then POST `/api/v1/new` within 30 s; 32-char token ([openHAB](https://www.openhab.org/addons/bindings/nanoleaf)) | | N | N |
| Nanoleaf Essentials (Thread/Matter) | No OpenAPI; go via Matter / Home Assistant ([TapHome](https://taphome.com/en/compatibility/nanoleaf-rgb-light/)) | | | | | |
| **LIFX** | LAN binary protocol over UDP; HTTP cloud API **(inferred)** | UDP 56700 **(inferred)** | none (LAN) / token (cloud) | | N | ext |
| **Govee** | LAN API: UDP multicast scan + JSON commands (`turn`, `brightness`, `colorwc`, `devStatus`) **(inferred command names)** ([guide](https://app-h5.govee.com/user-manual/wlan-guide)) | scan to 239.255.255.250:4001, reply on 4002, control to device:4003 **(inferred)** | Enable "LAN Control" per device in the Govee app; only some models | | N | N |
| **Home Assistant** | WebSocket API: `auth` then `subscribe_events`, `subscribe_trigger`, `call_service`, `get_states`, `fire_event`; plus REST + webhooks ([docs](https://contributors.home-assistant.io/core/internals/websocket-api/)) | `ws://<host>:8123/api/websocket` | Long-lived access token | | N | N |
| **WLED** | JSON API `/json`, WebSocket `/ws`, realtime UDP (WARLS/DNRGB), DDP, E1.31/Art-Net **(inferred)** | 80; UDP 21324 / DDP 4048 **(inferred)**; mDNS `_wled._tcp` **(inferred)** | none | | N | N |
| **Tuya** | Encrypted local protocol (v3.3–3.5); needs per-device `localKey` from Tuya cloud (tinytuya) **(inferred)** | 6668 **(inferred)** | Cloud dev account | | N | N |
| **DMX** (QLC+, Art-Net, sACN) | QLC+ WebSocket (SAMMI ext); Art-Net/sACN direct **(inferred)** | | | | N | ext |

**Recommendation:** make **Home Assistant** the Tier 1 "universal smart-home bridge". It covers Hue, Nanoleaf, LIFX, Govee, Tuya, Matter, Zigbee and plugs. Add direct **Elgato Key Light** (trivial) and **Hue** (popular) connectors, then WLED/Nanoleaf/Govee/LIFX in Tier 2.

---

## 7. Interactive, game and peer-automation integrations

| App | W | L | M | Surface | Port | Auth | SB | SAMMI | Use for us |
|---|---|---|---|---|---|---|---|---|---|
| **Streamer.bot** | Y | N | N | WebSocket server (`Subscribe`, `GetEvents`, `GetActions`, `DoAction`, custom events); HTTP server (`DoAction` returns 204); UDP DoAction ([requests](https://docs.streamer.bot/api/servers/websocket/requests), [HTTP](https://docs.streamer.bot/api/servers/http)) | `127.0.0.1:8080` | Optional password (enforced only for privileged requests unless "Enforce") | (self) | | **Coexistence + migration**: run SB actions, mirror SB events, import the action list. TS client `@streamerbot/client` ([guide](https://docs.streamer.bot/api/websocket/guide/client)) |
| **SAMMI** (formerly LioranBoard 2 **(inferred)**) | Y | N | N | Local HTTP API (buttons, variables, decks, alerts) + webhooks; Bridge WebSocket ([API](https://sammi.solutions/docs/api/overview)) | HTTP 9450; Bridge 9425 | Optional password | | (self) | Coexistence: trigger buttons, set variables |
| **Firebot** | Y | Y **(inferred)** | Y **(inferred)** | REST `/api/v1` + WebSocket ([API](https://docs.firebot.app/v5/dev/api), [WS](https://docs.firebot.app/v5/dev/websocket)) | 7472 | none | N | N | Run effect lists, read variables |
| **Mix It Up** | Y | N | N | Developer API REST `http://localhost:8911/api/v2/` ([wiki](https://wiki.mixitupapp.com/services/developer-api)) | 8911 | Enable in Services | N | Y | Run commands |
| **Lumia Stream** | Y | N **(inferred)** | Y **(inferred)** | REST `/api/send`; WebSocket pushes all events ([docs](https://dev.lumiastream.com/docs/intro)) | 39231 (HTTPS 39232) | Token | Y | N | Lights-centric users; send commands |
| **Crowd Control** | cloud | cloud | cloud | PubSub WebSocket (TLS 1.2+); production needs approval ([docs](https://developer.crowdcontrol.live/sockets/)) | | OAuth / approval | Y | Y | Triggers: effect request/success/failure, timed effects. Also "custom effects run our actions" ([CC guide](https://crowdcontrol.live/guides/streamerbot/)) |
| **Touch Portal** | (see §5) | | | | | | N | N | |
| **Minecraft Java** | | | | RCON over TCP (`enable-rcon`) **(inferred)** | 25575 | RCON password | N | N | Send commands (summon, title, give) on chat/redeem |
| **Minecraft Bedrock** | | | | `/connect` to an external WebSocket server with event subscriptions **(inferred)** | ours | none | N | N | Bedrock events as triggers (Tier 3) |
| **Source-engine games / Factorio / Rust / ARK** | | | | Source RCON over TCP **(inferred)** | 27015 typ. | password | N | N | Generic RCON sub-action |
| **Discord** | | | | Bot (gateway/REST) + webhooks; **local RPC over IPC** (named pipe / Unix socket `discord-ipc-N`). WebSocket RPC deprecated. `rpc.voice.read/write` scopes **need Discord approval** ([RPC docs](https://docs.discord.com/developers/topics/rpc)) | IPC | OAuth (approval-gated) | Y | ext | Voice-activity overlay ("who's talking") is approval-gated. Tier 3 behind an experimental flag |
| **Tiltify / Extra Life** | cloud | | | Tiltify v5 + webhooks; Extra Life = DonorDrive **(inferred)** | | | DonorDrive | N | Covered by the donations agent |
| **Overwolf** | Y | N | N | Game Events Provider inside the Overwolf runtime **(inferred)** | | | N | N | Tier 3 / skip |
| **LiveSplit** | Y | ~ | N | LiveSplit Server (TCP) **(inferred)** | 16834 **(inferred)** | | N | ext | Speedrunner triggers (Tier 3) |
| **VoiceAttack** | Y | N | N | Plugin API **(inferred)** | | | N | ext | Tier 3 |

---

## 8. Generic integration primitives (the long tail)

| Primitive | Trigger / sub-action | Rust building blocks **(inferred crate choices)** | OS caveats |
|---|---|---|---|
| **OSC** send/receive (+ OSCQuery) | Address-pattern triggers with typed args; send messages and bundles | `rosc`, `mdns-sd` | none |
| **VMC** (typed OSC) | Bones, blendshapes, status; router / multiplexer | `rosc` | none |
| **MIDI** in/out | Note, CC, PC, SysEx | `midir`, `wmidi` | Windows virtual ports |
| **WebSocket client** | "Message matches" trigger; send text/JSON | `tokio-tungstenite` | none |
| **WebSocket/HTTP server** (our API) | Inbound webhook trigger; custom endpoints | `axum` | Public webhooks need a tunnel or relay |
| **HTTP request** | GET/POST with templating; JSON-path extract to variables | `reqwest` | none |
| **UDP / TCP raw** | Packet trigger; send | `tokio` | none |
| **MQTT** client (+ optional embedded broker) | Topic trigger; publish | `rumqttc` (`rumqttd`) | none |
| **Named pipes / Unix sockets** | Read/write | `interprocess`, `tokio::net::windows::named_pipe` | Windows pipes vs Unix sockets |
| **Serial** | Line/byte triggers (Arduino buttons, DIY) | `serialport` | Linux needs `dialout` group |
| **File watch / text file** | "File changed" trigger; write text (legacy OBS text sources) | `notify` | none |
| **Keyboard/mouse simulation** | Send keys / clicks to apps (VSeeFace hotkeys, PNGTuber Plus) | Windows `SendInput`; macOS CGEvent (needs **Accessibility** permission); X11 XTest (`enigo`) | **Wayland: no global injection by design.** Options: (1) `xdg-desktop-portal` **RemoteDesktop** + **libei** (`ashpd` + `reis`): one consent prompt, persistent sessions since portal 1.21 ([Hutterer](https://planet.gnome.org/page4.html)); KDE (KWin 6.1+) and GNOME work, **Hyprland support partial/in progress** ([deskflow discussion](https://github.com/deskflow/deskflow/discussions/7499)); (2) wlroots virtual-keyboard/pointer protocols (Hyprland, Sway); (3) `ydotool` via `/dev/uinput` (needs a uinput permission or udev rule; works everywhere) ([wdotool](https://github.com/cushycush/wdotool) combines these) |
| **Global hotkeys** (trigger) | Hotkey pressed | Windows `RegisterHotKey`; macOS CGEventTap (Input Monitoring permission); X11 XGrabKey | **Wayland:** `org.freedesktop.portal.GlobalShortcuts` (KDE, GNOME 48+, Hyprland via xdph) **(inferred)**, otherwise compositor binds that call our CLI (`streamerlive trigger <name>`). On Hyprland a `bind = … exec streamerlive-cli` recipe is the reliable fallback |
| **Run program / shell** | Exec with args and env; capture stdout into variables | `tokio::process` | none |
| **D-Bus** (Linux) | Signal trigger; method call | `zbus` | Linux only (MPRIS, notifications, portals) |
| **AppleScript / JXA** (macOS) | Run script | `osascript` | macOS only; Automation permission prompts |
| **PowerShell** | Run script | `pwsh` / `powershell.exe` | PowerShell 7 is cross-platform |
| **Process watcher** | App started/stopped (e.g. "VTube Studio launched") | `sysinfo` | none. Also feeds the wizard |
| **mDNS / SSDP discovery** | Device appeared | `mdns-sd` | none |

---

## 9. Cross-platform reality check

**Windows-only (no realistic Linux path except Wine/Proton):**
- **VTuber apps:** Warudo, VNyan, VSeeFace, Animaze, 3tene, Luppet, T.I.T.S. **(inferred)**, Beat Saber mods.
- **Broadcasters:** vMix, XSplit.
- **Audio:** VoiceMeeter, SteelSeries Sonar.
- **Peers and platforms:** Mix It Up, Streamer.bot, SAMMI, Overwolf, Loupedeck legacy.
- **Partly broken under Proton/Wine:** VSeeFace (Wine OK minus virtual cam/Spout), Warudo (Proton ≤ 8.0-5, no capture/Spout/NDI) ([ProtonDB](https://www.protondb.com/app/2079120)), VRChat **(inferred)**.

**Windows + macOS (no Linux):**
- **VTuber / broadcasting:** VTube Studio, Streamlabs Desktop, Meld Studio, PRISM.
- **Audio:** Voicemod, Elgato Wave Link, the GoXLR official app.
- **Devices:** the official Stream Deck app, Camera Hub, Logi Options+.

**Genuinely cross-platform (works on Linux):**
- **VTuber:** veadotube mini, PNGTuber Plus, PNGTubeRemix **(inferred)**, Inochi Session, OpenSeeFace, XR Animator.
- **Broadcasting:** OBS.
- **Audio:** GoXLR Utility, PipeWire/MPRIS, VLC, ytmdesktop, Pear Desktop.
- **Devices and lighting:** OpenDeck / StreamController (Stream Deck), Touch Portal, OpenRGB, MIDI, Home Assistant, Hue/Nanoleaf/Govee/LIFX/WLED/Elgato lights (network devices are OS-agnostic).
- **Protocols and cloud:** VMC/OSC, every cloud service.

**Honest Linux pitch:** "Run Streamer.live on Linux and use the open stack: OBS + veadotube/Inochi Session or VTS-under-Proton + OpenSeeFace/XR Animator over VMC + PipeWire routing + GoXLR Utility + OpenDeck + Home Assistant." **Split-machine** setups are common among VTubers (inferred): Windows for the avatar app, Linux for the bot. Our network-transparent connectors make these first-class. Every connector should accept a **host**, not only `localhost`.

---

## 10. Default port registry (for the wizard and conflict detection)

| Port | Proto | App |
|---|---|---|
| 4455 | TCP/WS | OBS websocket v5 |
| 6557 | WS | Beat Saber HTTP Status **(inferred)** |
| 6742 | TCP | OpenRGB SDK |
| 7472 | HTTP/WS | Firebot |
| 8000 | WS | VNyan receiver (`/vnyan`) |
| 8001 | WS | VTube Studio API |
| 8069 | HTTP | VNyan REST (third-party report) |
| 8080 | WS / HTTP | Streamer.bot WS **and** VLC HTTP default. **Conflict** |
| 8088 / 8099 | HTTP / TCP | vMix |
| 8123 | WS | Home Assistant |
| 8911 | HTTP | Mix It Up |
| 9000 | WS (TCP) | Animaze API |
| 9000 / 9001 | UDP | VRChat OSC in/out. Different transport from Animaze; same number confuses users |
| 9123 | HTTP | Elgato Key Light |
| 9425 / 9450 | WS / HTTP | SAMMI Bridge / SAMMI API |
| 9863 | HTTP | ytmdesktop Companion |
| 11573 | UDP | OpenSeeFace **(inferred)** |
| 12136 | TCP | Touch Portal plugins **(inferred)** |
| 12351 | UDP | mocopi |
| 13339 | WS | Beat Saber AnotherStatusSocket |
| 13376 | WS | Meld Studio |
| 14564 | HTTP/WS | GoXLR Utility |
| 16021 | HTTP | Nanoleaf |
| 19052 / 19190 | HTTP / WS | Warudo callbacks / channels |
| 21412 | UDP | VTS phone tracking **(inferred)** |
| 25575 | TCP | Minecraft RCON **(inferred)** |
| 26538 | HTTP | Pear Desktop API Server |
| 39231 / 39232 | HTTP / HTTPS | Lumia Stream |
| 39539 / 39540 | UDP | VMC Marionette / Performer |
| 42069 | WS | T.I.T.S. **(inferred)** |
| 47779 | UDP (bcast) | VTube Studio discovery |
| 49983 | UDP | iFacialMocap **(inferred)** |
| 59129 (+10 fallbacks) | WS | Voicemod |
| 59650 | WS | Streamlabs Desktop |
| 1824 | WS | Elgato Wave Link (community) |
| dynamic | HTTP | SteelSeries Sonar (`coreProps.json`); veadotube (instance files) |

**Our own defaults must avoid all of the above.** Pick something like 7580–7589 **(inferred: verify free)**. Never take 8080.

---

## 11. Integration plugin architecture (brief; the SDK agent owns the details)

**Common contract.** Rust trait for built-ins; the same shape is serialized for out-of-process plugins.
- `manifest()`:
  - id and semver.
  - `apiVersion` range.
  - OS support flags.
  - Category tags.
  - Discovery hints: ports, mDNS types, process names, file paths. The wizard consumes these.
- `settings_schema()`: JSON Schema. Drives auto-generated settings UI, with `x-secret` for tokens.
- `connect(ctx)`, `disconnect()`, `health()`.
  - State machine: `Disconnected`, `Discovering`, `Connecting`, `AuthRequired(flow)`, `Connected`, `Degraded(reason)`.
  - Auto-reconnect with backoff.
  - `AuthFlow` enum covers the pairing styles seen above:

| `AuthFlow` | Example apps |
|---|---|
| `None` | Elgato Key Light, OSC/VMC |
| `TokenPaste` | Streamlabs, Lumia, Home Assistant |
| `AppApprovalPopup` | VTS, ytmdesktop |
| `PhysicalButton` | Hue link button, Nanoleaf power-hold |
| `OAuthDeviceCode` | Pulsoid |
| `OAuthPkce` | Spotify BYO client |
| `ApiKeyRegistration` | Voicemod clientKey |

- `discover()`: returns candidate instances (host, port, instance id, version). Supports multi-instance (VTS windows, veadotube instances).
- `triggers()`: descriptors with id, label, payload JSON Schema, example payload and rate hint. High-rate streams such as VMC bones, HR and ModelOutline are flagged `stream`, so the UI offers throttling and threshold conditions instead of raw firing.
- `actions()`: descriptors with parameter JSON Schema plus **dynamic option providers**, e.g. `vts.hotkeys`, `meld.scenes`, `voicemod.voices`. These are resolved live so dropdowns show real hotkey/scene names.
- `variables()`: exposed state such as current VTS model, Meld streaming flag, GoXLR fader values. Usable in conditions and text templates.

**Delivery tiers:**
1. **Built-in Rust connectors** for Tier 1/2.
2. **Out-of-process plugins over JSON-RPC 2.0** on stdio or local WebSocket. Language-agnostic, so JS/TS, Python or C# all work, which matches the owner's JS/Python requirement. A crash does not take down the core. Same schema as built-ins, so the UI treats them identically. This is the main community path. Model it after LSP (inferred), or reuse our public WS API so "plugin" and "API client" are the same thing.
3. **WASM components (WIT)** later, for sandboxed pure-logic or protocol plugins that need no OS access, distributed in a registry.

Avoid in-process native dylibs (ABI and stability pain).

**Versioning:**
- Semver per plugin.
- Core exposes `apiVersion` with capability negotiation at handshake.
- Unknown fields are ignored (VTS-style).
- Deprecations last at least one minor version.

---

## 12. "VTuber Setup Wizard" concept

**Goal:** a first-run (and on-demand) wizard that finds the user's VTuber tools, connects them with one click each, and generates a working starter configuration.

### 12.1 Detection pipeline (all local; nothing leaves the machine)

1. **Process scan** (`sysinfo`): match known executables, e.g. `VTube Studio.exe`, `Warudo.exe`, `VNyan.exe`, `VSeeFace.exe`, `Animaze*.exe`, `veadotube*`, `inochi-session`, `obs64`/`obs`, `Voicemod*`, `voicemeeter*`, `goxlr-daemon`, `WaveLink*`, `Meld Studio`, `Streamlabs*`, `VRChat.exe`, `XR Animator`, `TouchPortal`, `OpenRGB`, `Streamer.bot.exe`, `SAMMI*` (names partly **(inferred)**). Also read **Steam library manifests** (`appmanifest_*.acf`) to detect installed-but-not-running apps, e.g. Warudo 2079120.
2. **Passive listeners:**
   - VTube Studio UDP broadcast on 47779: instant VTS discovery with instanceID and port.
   - mDNS browse: `_oscjson._tcp` (VRChat OSCQuery), `_hue._tcp`, `_elg._tcp`, `_nanoleafapi._tcp`, `_wled._tcp`, `_home-assistant._tcp` **(type names partly inferred)**.
   - Govee multicast scan.
3. **Filesystem discovery:**
   - veadotube `instances/` files.
   - SteelSeries GG `coreProps.json`.
   - VSeeFace/Warudo config files to read their VMC ports **(inferred)**.
4. **Active port probes** on 127.0.0.1 (and an optional user-entered "avatar PC" IP) for the registry in §10. Each probe performs a **protocol fingerprint**, not just a TCP connect:
   - VTS: `APIStateRequest`.
   - Meld: WebChannel init.
   - Voicemod: try its port list.
   - ytmdesktop: `/metadata`.
   - Firebot: `/api/v1/status`.
   - Streamer.bot: `GetInfo` **(inferred request name)**.
   - OBS: `Hello` op.

   Probes are rate-limited and never send state-changing commands.
5. **"Installed but API disabled" detection:** process running but port closed. Show an app-specific card with screenshots, e.g. "In VTube Studio: Settings > Plugins > Start API", "Animaze: Settings > Animaze API > Enabled", "VNyan: Settings > Misc > WebSockets > Start Receiver", "Streamlabs: Settings > Remote Control". Then re-probe automatically.

### 12.2 Pairing walk-through

One card per app, showing only the pairing step it needs:
- Approve the VTS popup.
- Approve ytmdesktop.
- Press the Hue link button (countdown).
- Hold Nanoleaf power for 5–7 s (30 s window).
- Paste the Streamlabs/HA/Lumia token.
- Run Pulsoid device-code login.
- Install the Warudo companion plugin (Steam Workshop link).
- Install our Stream Deck / Touch Portal / OpenDeck plugin.

Secrets go to the OS keychain **(inferred: `keyring` crate; Secret Service on Linux)**.

### 12.3 Tracking and VMC routing graph (the VTuber-specific part)

- **Visual graph:** tracker, then (optional) Streamer.live VMC router, then renderer(s), then OBS.
- **Auto-detects port conflicts:** two receivers on 39539; VSeeFace send equals receive port; VRChat 9000/9001 vs Animaze 9000.
- **Recommends sources by OS and hardware:**
  - iPhone: VTS phone or iFacialMocap.
  - Android: MeowFace.
  - Webcam on Linux: OpenSeeFace or XR Animator.
  - Full body: mocopi / XR Animator.
- **Offers to route through our router**, so triggers can inject expressions into *any* VMC renderer and the user can hot-swap renderers without reconfiguring trackers.
- **Shows live "signal OK" lights** per hop: packets per second and last-seen timestamps.

### 12.4 Starter packs

Generate editable actions from templates, only for detected apps:

| Starter action | Wiring |
|---|---|
| Channel-point redeem to VTS hotkey / expression | Dropdown populated from live VTS hotkeys |
| Bits / donations to T.I.T.S. throw, or VTS item drop | |
| Raid to Warudo blueprint "Raid", or a VMC blendshape burst | |
| Heart rate to VTS custom parameter / VRChat OSC parameter, plus an HR overlay | |
| Mic mute (GoXLR / Wave Link / VoiceMeeter / PipeWire) to veadotube "muted" state, plus a VTS "muted" expression | |
| "Tracking lost" from VTS or VMC `/OK` | Switch OBS to a BRB scene after N seconds |
| Now-playing (MPRIS / GSMTC / ytmdesktop) | Chat command `!song` + overlay |

### 12.5 Health dashboard

Persistent "VTuber rig status" panel:
- Each app: connected / API-disabled / not running.
- Tracker FPS.
- "Fix it" buttons that re-run the relevant wizard card.

---

## 13. Prioritized list

**Tier 1: MVP (must-have).**
`(Win)` = Windows-only app; the connector is still built for all OSes so it works against a remote Windows PC.

| # | Integration | Justification |
|---|---|---|
| 1 | VTube Studio | Most-used VTuber app; richest API; SB parity; foundation for throw/HR plugins |
| 2 | VMC send/receive + **VMC router** | One protocol covers VSeeFace/Warudo/VNyan/XR Animator/WMC; router is a unique simplifier |
| 3 | Generic OSC (+ VRChat preset, OSCQuery) | VRChat VTubers; also covers ChilloutVR mods, TouchOSC and many tools |
| 4 | Warudo `(Win)` via companion plugin + VMC | Owner-requested; growing 3D platform; no competitor has it native |
| 5 | VNyan `(Win)` WebSocket | Popular free 3D app; trivial text-command protocol |
| 6 | veadotube mini | Most popular PNGtuber tool; cross-platform; clean documented API |
| 7 | Voicemod (W/M) | SB and SAMMI parity; very common with VTubers |
| 8 | Pulsoid + HypeRate | Cheap, beloved VTuber gimmick; SB parity |
| 9 | Our Stream Deck plugin (+ OpenDeck/OpenAction compatibility) | Primary physical trigger for streamers; OpenDeck gives the Linux story |
| 10 | MIDI in/out | Cheap universal hardware surface; cross-platform |
| 11 | Hotkeys: global hotkey trigger + key/mouse simulation (incl. Wayland portal/libei + ydotool fallback) | Needed for hotkey-only apps (VSeeFace, PNGTuber Plus); owner is on Hyprland |
| 12 | Generic WS client, HTTP request/webhook-in, UDP/TCP, run program, file watch | Long-tail coverage on day one |
| 13 | Now playing: MPRIS (Linux) + GSMTC (Windows) | Universal song info without Spotify API pain |
| 14 | Streamer.bot WS client | Lets users migrate gradually and coexist; import actions |
| 15 | Home Assistant | One connector covers most smart-home lights and plugs; Linux-friendly |
| 16 | Elgato Key Light | Trivial HTTP; extremely common streamer hardware; SB parity |
| 17 | PipeWire (Linux) / WASAPI (Windows) per-app volume + mute | Core audio automation without third-party mixers; big Linux win |

**Tier 2.**

| # | Integration | Justification |
|---|---|---|
| 1 | Meld Studio (W/M) | SB and SAMMI parity; clean WebChannel API |
| 2 | Streamlabs Desktop (W/M) | SB parity; large user base |
| 3 | T.I.T.S. `(Win)` | SB parity; popular VTS throwing tool |
| 4 | Animaze `(Win)` | Documented WS API; mainstream avatar app |
| 5 | VSeeFace `(Win)` preset (VMC Assistant + hotkeys) | Still widely used; no API, so mostly preset/config work |
| 6 | GoXLR Utility | Open-source, cross-platform API; Linux showcase |
| 7 | VoiceMeeter `(Win)` | Ubiquitous on Windows streaming rigs |
| 8 | Elgato Wave Link 3 (W/M, unofficial) | SB parity; free WL3 broadens its user base; protocol fragile |
| 9 | Philips Hue (direct, incl. Entertainment later) | Most popular lights; SAMMI ext parity |
| 10 | Nanoleaf, Govee LAN, LIFX, WLED | Popular streamer lighting; simple LAN APIs |
| 11 | OpenRGB | Cross-platform RGB for every peripheral brand |
| 12 | Touch Portal plugin | Cross-platform control surface incl. Linux |
| 13 | ytmdesktop + Pear Desktop | Song requests and now playing without Spotify restrictions |
| 14 | Spotify Web API (BYO client ID) | Expected feature; 2026 restrictions push it out of the MVP |
| 15 | VLC HTTP | Media-player control for watch-alongs; cross-platform |
| 16 | Crowd Control | SB and SAMMI parity; interactive games |
| 17 | Lumia, Firebot, Mix It Up `(Win)`, SAMMI `(Win)` peers | Coexistence for users migrating from other bots |
| 18 | OBS vendor-request helpers (Advanced Scene Switcher, Aitum Vertical, Move, Source Record) | Big power for little code on top of obs-websocket |
| 19 | MQTT | DIY / IoT long tail |
| 20 | Discord bot + webhooks | Common need (bot side may be covered elsewhere) |
| 21 | Minecraft / Source RCON | Popular interactive-stream gimmick; trivial protocol |
| 22 | Game controllers | Cheap triggers; cross-platform |
| 23 | Beat Saber `(Win)` status | Popular VR streaming game with live event sockets |

**Tier 3: long tail.**

| # | Integration | Justification |
|---|---|---|
| 1 | vMix `(Win)` | Niche for VTubers; easy protocol |
| 2 | XSplit `(Win)` | Declining, no clean API |
| 3 | PRISM (W/M) | Unofficial protocol; subscription shift |
| 4 | SteelSeries Sonar `(Win)` | Unofficial and dynamic port |
| 5 | Elgato Camera Hub (W/M) | Undocumented |
| 6 | Loupedeck / Logi MX Creative Console (W/M) | C# SDK; smaller user base |
| 7 | StreamController native plugin (Linux) | OpenDeck may suffice |
| 8 | NDI (send our overlays / tally) | Heavy SDK; licensing check |
| 9 | Inochi Session (tracking/VMC source; upstream API contribution) | Strategic for Linux, but no API yet |
| 10 | PNGTuber Plus / PNGTubeRemix (hotkey presets) | No API |
| 11 | 3tene, Luppet `(Win)` | Small user base |
| 12 | Kalidoface | Web app, no API |
| 13 | mocopi / iFacialMocap direct ingest | Normally routed via VMC apps |
| 14 | ChilloutVR `(Win)` OSC mod | Small user base; generic OSC covers it |
| 15 | Discord local RPC voice overlay | Approval-gated scopes |
| 16 | Tidal / foobar2000 | MPRIS/GSMTC covers most of it |
| 17 | Tuya local | Fragile; Home Assistant covers it |
| 18 | DMX / QLC+ | Niche |
| 19 | Minecraft Bedrock WebSocket | Niche |
| 20 | LiveSplit | Speedrunner niche |
| 21 | VoiceAttack `(Win)` | Niche |
| 22 | Overwolf `(Win)` | Walled runtime |
| 23 | macOS MediaRemote adapter | Fragile private API |
| 24 | AppleScript / JXA | macOS is "maybe" |
| 25 | PolyPop | Status unknown |
| 26 | Karasubot | Unverified |

---

## 14. Open questions for the project owner

1. **Warudo companion plugin.** It has to be C# (a Unity mod). Is a small C# shim acceptable, given the "not a fan of C#" preference? Alternatively we can rely on VMC plus a community bridge only.
2. **macOS scope.** Is it "maybe"? It decides whether we invest in Voicemod-mac verification, MediaRemote workarounds and macOS input permissions.
3. **Split-machine rigs.** Should Tier 1 connectors all support remote hosts (avatar on a Windows PC, bot on Linux)? I recommend yes.
4. **Wayland input.** OK to ship a ydotool/uinput fallback, which needs a udev rule (AUR post-install hook)? Or portal-only, with prompts?
5. **Spotify.** Accept BYO client ID with 5-user dev mode, or skip Spotify Web API in favor of MPRIS/GSMTC plus ytmdesktop?
6. **Unofficial APIs** (Wave Link, Sonar, PRISM, VNyan REST). Ship them behind an "experimental" toggle with version pinning?
7. **VMC router.** Should it be a headline feature, positioned as "the VTuber hub"? It shapes the wizard and UI design.
8. **Which specific helper apps** did the owner mean by "Karasubot"? Is PolyPop still in use?
9. **Stream Deck.** Prioritize our own Elgato SDK plugin (Node.js), OpenDeck compatibility, or direct HID control?
10. **Home Assistant as the official smart-home path.** Accept it, with direct connectors only for Elgato Key Light and Hue in early tiers?

---

## 15. Key sources (also cited inline)

- **Competitors:**
  - Streamer.bot integrations: https://docs.streamer.bot/guide/integrations
  - Streamer.bot stream apps: https://docs.streamer.bot/guide/stream-apps
  - Streamer.bot WS requests: https://docs.streamer.bot/api/servers/websocket/requests
  - SAMMI docs: https://sammi.solutions/docs
  - SAMMI API: https://sammi.solutions/docs/api/overview
  - SAMMI extensions: https://sammi.solutions/extensions
- **VTuber apps:**
  - VTube Studio API: https://github.com/DenchiSoft/VTubeStudio
  - VTube Studio events: https://github.com/DenchiSoft/VTubeStudio/blob/master/Events/README.md
  - Warudo: https://docs.warudo.app/docs/scripting/api/external-callbacks, https://docs.warudo.app/docs/scripting/overview, https://docs.warudo.app/docs/mocap/vmc
  - VNyan: https://github.com/Suvidriel/VNyanDoc/wiki/Plugins, https://github-wiki-see.page/m/NeocortexVT/VNyanDocFork/wiki/How-To
  - veadotube: https://veado.tube/docs/tech/api/
  - VMC: https://protocol.vmc.info/english
  - OpenSeeFace: https://github.com/emilianavt/OpenSeeFace
  - VSeeFace: https://www.vseeface.icu/
  - VRChat OSC: https://docs.vrchat.com/docs/osc-overview
  - Inochi Session: https://github.com/Inochi2D/inochi-session
  - Animaze: https://holotechconfluence.atlassian.net/wiki/spaces/HD/pages/157941761/Animaze+API
  - XR Animator: https://github.com/ButzYung/SystemAnimatorOnline
  - mocopi: https://www.sony.co.jp/en/Products/mocopi-dev/en/documents/Home/TechSpec.html
  - Pulsoid: https://docs.pulsoid.net/
  - vts-heartrate: https://skeletom-ch.itch.io/vts-heartrate
- **Broadcasting:**
  - Meld Studio: https://github.com/MeldStudio/streamdeck/blob/main/WebChannelAPI.md
  - Streamlabs Desktop: https://dev.streamlabs.com/reference/streamlabs-obs-api
  - vMix: https://www.vmix.com/help24/DeveloperAPI.html
  - PRISM (unofficial): https://pub.dev/packages/prism_connect
- **Audio:**
  - Voicemod: https://control-api.voicemod.net/getting-started/
  - GoXLR Utility: https://github.com/GoXLR-on-Linux/goxlr-utility
  - SteelSeries Sonar: https://github.com/wex/sonar-rev
  - Wave Link (unofficial): https://github.com/Noctinae/ElgatoWavePy/wiki/Available-Commands
  - Spotify changes: https://developer.spotify.com/blog/2026-02-06-update-on-developer-access-and-platform-security
  - ytmdesktop: https://github.com/ytmdesktop/ytmdesktop/wiki/v2-%E2%80%90-Companion-Server-API-v1
  - macOS MediaRemote: https://github.com/ungive/mediaremote-adapter
- **Devices:**
  - Elgato Stream Deck SDK: https://docs.elgato.com/sdk/
  - OpenDeck: https://github.com/nekename/OpenDeck
  - Touch Portal: https://www.touch-portal.com/api
  - OpenRGB SDK: https://gitea.marinkevich.ru/grayhook/OpenRGB/src/branch/master/Documentation/OpenRGBSDK.md
  - Home Assistant: https://contributors.home-assistant.io/core/internals/websocket-api/
  - Govee LAN: https://app-h5.govee.com/user-manual/wlan-guide
- **Interactive:**
  - Crowd Control: https://developer.crowdcontrol.live/sockets/
  - Lumia Stream: https://dev.lumiastream.com/docs/intro
  - Firebot: https://docs.firebot.app/v5/dev/api
  - Mix It Up: https://wiki.mixitupapp.com/services/developer-api
  - Discord RPC: https://docs.discord.com/developers/topics/rpc
- **Wayland input:**
  - libei / portals: https://planet.gnome.org/page4.html
  - Compositor status: https://github.com/deskflow/deskflow/discussions/7499
  - wdotool: https://github.com/cushycush/wdotool
