# Speaker.bot and SAMMI: Feature Research for a Unified Cross-Platform App

> Research date: **2026-10-18**. Sources are cited inline. **(inferred)** marks conclusions drawn from indirect evidence, not stated by a vendor.
>
> **Method.** I read the docs source of both products. Speaker.bot's docs live in [github.com/Streamerbot/docs](https://github.com/Streamerbot/docs) under `speakerbot/` and render at https://speaker.bot. SAMMI's docs live in [github.com/SAMMISolutions/docs](https://github.com/SAMMISolutions/docs), last pushed 2026-09-02, and render at https://sammi.solutions/docs. Other sources: the official release repos, changelogs, archived pages from the retired TwitchSpeaker wiki (via the Wayback Machine), the Streamer.bot docs for its Speaker.bot integration, and two open-source Speaker.bot clones that reproduce its undocumented API.

---

## Table of Contents

1. [Speaker.bot](#1-speakerbot)
2. [SAMMI](#2-sammi)
3. [Unique to Each (for the unified app)](#3-unique-to-each-for-the-unified-app)
4. [Parity checklist / implementation notes](#4-parity-checklist--implementation-notes)
5. [Open questions for the project owner](#5-open-questions-for-the-project-owner)
6. [Source index](#6-source-index)

---

## 1. Speaker.bot

### 1.1 Snapshot

| Attribute | Value | Source |
|---|---|---|
| What it is | "Supercharged Text to Speech (TTS) for your live stream". A standalone TTS companion to Streamer.bot from the same team. | [speaker.bot](https://speaker.bot), [index.yml](https://github.com/Streamerbot/docs/blob/main/speakerbot/index.yml) |
| Current version | **v0.1.6**, released **2026-01-14** ("first release of 2026"), author nate1280. No newer release is listed as of 2026-10-18. | [changelog v0.1.6](https://docs.streamer.bot/changelogs/speaker.bot/v0.1.6) |
| Release cadence | Slow. v0.1.1 2023-04-14, v0.1.2 2023-10-31, v0.1.3 2023-11-29, v0.1.4 2024-07-05, v0.1.5 2025-04-10, v0.1.6 2026-01-14. | [changelogs](https://docs.streamer.bot/changelogs/speaker.bot/v0.1.4) |
| History | Started as **TwitchSpeaker** (v0.0.7 on 2020-07-07 through v0.0.48 on 2022-01-13). The download URL still uses the name: `streamer.bot/api/releases/twitchspeaker/latest/download`. | Archived TwitchSpeaker wiki changelog (wiki.streamer.bot/en/TwitchSpeaker/Changelogs, Wayback 2022-12-08); [installation.md](https://github.com/Streamerbot/docs/blob/main/speakerbot/0.get-started/0.installation.md) |
| Distribution | Portable `.zip`. Extract it and run `Speaker.bot.exe`; there is no installer. | [installation](https://github.com/Streamerbot/docs/blob/main/speakerbot/0.get-started/0.installation.md) |
| Platform | Windows. The v0.0.34 changelog says it "may potentially work on Linux with Wine" after OneCore was removed. Wine users report that SAPI5 is a stub and that Azure and Google misbehave. | TwitchSpeaker changelog v0.0.34; [Speekaboo README](https://github.com/easyaspi314/Speekaboo) |
| Tech stack | **.NET Framework 4.8.1** (since v0.1.5), LiteDB (v0.1.2), websocket-sharp (v0.1.4), WASAPI audio output (v0.0.20; WaveOut before that), Socket.io v4 for the Streamlabs client. WinForms UI **(inferred)** from .NET Framework plus the classic Win32 look. Since v0.1.6 it is EV code-signed. | [v0.1.5](https://docs.streamer.bot/changelogs/speaker.bot/v0.1.5), [v0.1.4](https://docs.streamer.bot/changelogs/speaker.bot/v0.1.4), [v0.1.2](https://docs.streamer.bot/changelogs/speaker.bot/v0.1.2), [v0.1.6](https://docs.streamer.bot/changelogs/speaker.bot/v0.1.6) |
| Data files | `settings.json`, `voicealiases.dat`, `users.dat`, `usage.dat` in `data/`. Backups are auto-zipped into `backup/` on startup. | Archived wiki "Backup" page; TwitchSpeaker changelog v0.0.32 |
| Licensing and price | Free. Closed source **(inferred)**: no source repo exists and no license is published. Alpha and beta builds go to supporters through a Streamer.bot website login and nate1280's Patreon. | [accounts.md](https://github.com/Streamerbot/docs/blob/main/speakerbot/0.get-started/1.accounts.md), [changelogs page](https://speaker.bot/changelogs) |
| Other modes | **Headless Mode** (v0.1.2), minimize to tray and confirm on close (v0.1.1), remembered window position (v0.1.6). | changelogs |

### 1.2 Speech engines (voice providers)

Configure engines under **Settings > Speech Engines**: click **Add** and pick an engine type. The tab also lists every voice from every configured engine. Engines are loaded dynamically since the v0.1.2 rewrite. [speech-engines.md](https://github.com/Streamerbot/docs/blob/main/speakerbot/0.get-started/2.speech-engines.md)

| Engine | Cost | How it is configured | Voice-parameter notes |
|---|---|---|---|
| **SAPI5** (Windows native) | Free | No account needed. | Rate, pitch, and volume per voice. Windows only. |
| **Google Cloud TTS** | Paid, with free trial | Service-account **JSON key file**. Enable the Cloud TTS API, create a service account, add a JSON key, and browse to the file. | Rate -20 to 20 and pitch -19 to 21, in 5% steps. Emphasis on Standard and WaveNet voices. Speaker.bot **wraps text in SSML**, and the SSML tags count toward Google's character quota. Voices that do not support SSML are handled (fixed in v0.1.5). |
| **Microsoft Azure** (Cognitive Services) | Paid, free F0 tier | **Subscription key** (KEY 1) plus **service location/region**. | Rate -10 to 10 and pitch -9 to 11. Emphasis on Standard and Neural voices. Azure replaced OneCore in v0.0.34. |
| **Amazon Polly** | Paid, free tier | IAM user with `AmazonPollyReadOnlyAccess`; enter the **Access Key ID** and **Secret**. | Rate -20 to 20 and pitch -9 to 11. Pitch is **not** supported on Neural voices. Emphasis on Standard voices. The "Brian" voice that Streamlabs TTS made famous is a Polly voice, so it is reachable this way **(inferred)**. |
| **IBM Watson** | Paid, free tier of 10k characters per month | `.env` credentials file downloaded from IBM Cloud. | Rate -20 to 20 and pitch -19 to 21. No emphasis. |
| **UberDuck** | Paid (Creator subscription) | Public key goes in the Client field, secret key in the Secret field. | v0.1.4 removed "external service" voices. About 27 voices remain, and some return HTTP 500. |
| **TTS Monster** | Paid, free tier | Paste the **overlay URL**; Speaker.bot parses the credentials from it (v0.1.3). | Custom Voices supported since v0.1.6. |
| **Acapela Cloud** | Paid | API credentials. | Added in v0.0.46. |
| **CereProc** (CereVoice Cloud) | Paid | API credentials. | Added in v0.0.46. |
| **ElevenLabs** | Paid, free tier of 10k characters per month | **API key** from the profile page. | Added in v0.1.2. "Pitch" and "Rate" sliders map to **Stability** and **Similarity** (v0.1.4). Legacy voices supported (v0.1.5). Voice and model loading improved in v0.1.6. |

Sources: [speech-engines.md](https://github.com/Streamerbot/docs/blob/main/speakerbot/0.get-started/2.speech-engines.md); archived wiki engine pages (Azure, Polly, Google, UberDuck, Watson, Wayback 2023-01-25); TwitchSpeaker changelog v0.0.34 for the pitch, rate, and emphasis ranges.

**Not supported (as of v0.1.6):** Windows **OneCore** (removed v0.0.34), TikTok TTS, Streamlabs' hosted TTS, Coqui, VoiceVox, **Piper**, Kokoro, Fish Audio, Cartesia, OpenAI TTS, and every other local neural engine. The community filled the gap:
- **OpenSpeaker** ([EvaisaDev/OpenSpeaker](https://github.com/EvaisaDev/OpenSpeaker), pushed 2026-10-02) is an open-source clone claiming "(mostly) full feature parity". It adds TikTok (via proxy), Fish Audio, Inworld, Resemble, Cartesia, and LMNT, supports Lua extensions for engines, and can migrate Speaker.bot data.
- **Speekaboo** ([easyaspi314/Speekaboo](https://github.com/easyaspi314/Speekaboo)) is a Python/Piper drop-in that speaks Speaker.bot's WS and UDP API so Linux users can drive it from Streamer.bot.

**Effects:** Speaker.bot has no audio effects (reverb, echo, robot, and so on). Per-voice settings are pitch, rate, volume, and emphasis only; the Speaking History shows lines like "Pitch:1, Rate:0, Volume 162%". Archived wiki "Speaking History".

**SSML:** Speaker.bot generates SSML internally for cloud engines (Google docs note). The docs do not say whether users can type raw SSML into chat or the API **(unknown; likely escaped or stripped, inferred)**.

### 1.3 Voice aliases, voice selection, and per-user voices

| Feature | Behavior | Source |
|---|---|---|
| **Voice Alias** | A named group of one or more voices, possibly from different engines. To create one, enter a Name and click Add, then add voices from the Voice dropdown. Every TTS path refers to an alias (default alias, event alias, command alias, user alias, API `voice`). | Archived wiki "Voice Aliases"; [quick-start](https://github.com/Streamerbot/docs/blob/main/speakerbot/0.get-started/3.quick-start.md) |
| Per-alias **output device** | Each alias picks an audio output device, or `Application Default` to use the global device. | [quick-start](https://github.com/Streamerbot/docs/blob/main/speakerbot/0.get-started/3.quick-start.md), [general settings](https://github.com/Streamerbot/docs/blob/main/speakerbot/1.guide/2.config/0.index.md) |
| Per-voice pitch, rate, volume, emphasis | Set per voice inside an alias. Ranges vary by engine (see 1.2). | TwitchSpeaker changelog v0.0.34; Speaking History format |
| **Test Speak** | A button at the bottom of the Voice Aliases view. | quick-start |
| Alias list sorting | Sorted alphabetically since v0.1.6. | v0.1.6 |
| **Default Voice Alias** | Global default under Settings > General. | general settings |
| **Highlight Voice Override / Alias** | A separate alias for highlighted messages and mentions. | general settings |
| **Event voice override** | A global alias for all event messages (Events tab). | [events.md](https://github.com/Streamerbot/docs/blob/main/speakerbot/1.guide/1.events.md) |
| **Random voices** | Voices can be chosen at random (the "random" voice mode). | [commands.md](https://github.com/Streamerbot/docs/blob/main/speakerbot/1.guide/0.commands.md) |
| **Ignore Profiles** | Named profiles that exclude voices and/or locales from random selection. Double-click to toggle. Switch profiles from the UI, with `!tts profile ...` commands (v0.0.22), or with the UDP `profile` command. | [ignore.md](https://github.com/Streamerbot/docs/blob/main/speakerbot/1.guide/2.config/1.ignore.md), UDP docs |
| **Sticky Random Voice** | A user keeps the first random voice they get until it is reset (`!tts random reset <user>`). | speak-options, commands |
| **Assign last voice** | `!tts assign last <user>` (or right-click a user > Assign voice) turns the last random voice into a permanent per-user alias. | commands, UDP docs, TwitchSpeaker v0.0.41 |
| **Per-user voice** | Users tab > Voice Alias dropdown. | [users.md](https://github.com/Streamerbot/docs/blob/main/speakerbot/1.guide/2.config/4.users.md) |
| **Past Voices** | Per-user history of the voices they have used, with timestamps. Can be cleared. | Archived wiki "Users"; v0.1.4 |
| **Speech preview window** | Edit and preview a voice; you can assign a voice from the preview (v0.0.22). | TwitchSpeaker changelog |
| **Generic Speaker** | A standalone "just type and speak" form (v0.0.47). | TwitchSpeaker changelog |

### 1.4 Text-processing pipeline (filters and replacements)

Processing order is partly documented. Regex replacement "is the last thing processed" and applies to **all** spoken text, including events (TwitchSpeaker v0.0.22). The rest of the order is **(inferred)**: permission and ignore checks, then command or prefix parsing, then emote removal, then URL filter, then bad-word filter, then the max words/characters limit, then nickname and username prefix, then regex, then SSML wrapping, then the engine.

| Feature | Options | Source |
|---|---|---|
| **Bad Word Filter** | Word/phrase list, case-insensitive (v0.1.2). **Filter Action**: `Disabled`, `Strip` (remove the word), or `Yeet All` (drop the whole message). API callers can bypass it with `badWordFilter: false`. | [replace.md](https://github.com/Streamerbot/docs/blob/main/speakerbot/1.guide/2.config/3.replace.md); archived wiki "Replacement" |
| **Regex Replacement** | Ordered list of regex or plain-text find/replace rules, reordered with Ctrl+Up/Down. Invalid regex shows a warning (v0.0.23). The developer warns against using it to "put words in people's mouths". | replace.md; TwitchSpeaker v0.0.22, v0.0.28 |
| **URL Filter** | `Disabled`, `Strip`, or `Yeet All`. There is also a "Global URL Filter" setting (fixed in v0.1.4). | archived wiki "Speaking Options"; v0.1.4 |
| **Max Words / Max Characters** | Message length cap (v0.1.3). | v0.1.3 |
| **Emote stripping** | Separate toggles to ignore Twitch, BTTV, FFZ, 7TV, Cheermotes, and **Twemoji** (Unicode emoji, v0.1.4). **Allow first emote** speaks a single instance of each emote. An **allowed emotes** list bypasses the filters. 7TV zero-width emotes handled (v0.1.6). Each emote service can be turned off (v0.1.5). | [speak-options.md](https://github.com/Streamerbot/docs/blob/main/speakerbot/1.guide/2.config/2.speak-options.md); archived wiki |
| **Ignored Prefixes** | Messages that start with any configured prefix are not spoken. | speak-options |
| **Username reading** | `Say Username` with a configurable **"said"** prefix ("User said ..."). `Only say username if previous was different`. **Last spoke grace period** in seconds: don't repeat the username within N seconds (v0.0.22). `Replace %name% with nickname`. | speak-options; archived wiki |
| **Nicknames** | Per-user nickname, settable in the UI or by mods (`!tts set nickname`). | users.md, commands |
| **Not Allowed Text** | Chat reply sent when a user lacks permission. | archived wiki "Speaking Options" |
| **Shared Chat** | Option to ignore Twitch Shared Chat messages (v0.1.5). | v0.1.5 |
| Reply handling | In say-everything mode, replies are treated as normal messages (v0.0.35). | changelog |

### 1.5 Speaking options, permissions, users

| Area | Settings | Source |
|---|---|---|
| Mode | `Say Everything` reads all chat; otherwise TTS needs a speaking command such as `!say` or `!"`. Toggle with `!tts mode all|command` or the API `Mode` request. | speak-options |
| Forced | `Allowed Forced` (global) plus a per-user **Forced** flag that always speaks that user. | speak-options, users |
| Permissions | Moderators, Subscribers, VIPs, Regulars, Everyone. `!tts permissions` toggles them (v0.0.46). | speak-options |
| Users tab | Left: user list with a "Hide Not Present" filter. Middle: ID, Username, Role, Voice Alias, Nickname, Last Active, and **Ignored / Forced / Regular / Subscribed** (read-only) flags. Right: **Past Voices**. A context menu offers Delete and Assign voice. | users.md; archived wiki "Users" |
| Moderation coupling | **Stop/Skip** current and queued TTS for a user on `Message Deleted`, `User Timed Out`, or `User Banned`. | speak-options |
| Mod whispers | `Allow Moderator Whispers` lets mods whisper `!tts` commands to the broadcaster account. | general settings |
| Rate limiting | The username grace period and max words/chars; follow-spam guard (one follow per user per session, v0.0.21). No per-user cooldown or token bucket is documented **(inferred: none)**. | changelogs |

### 1.6 Queue and playback control

| Control | Where | Notes |
|---|---|---|
| **Queue** | Processing tab. Speaking Queue and Speaking History were merged into it in v0.1.2. | Rows show engine, voice, locale, pitch, rate, volume, user, and message. |
| **Pause / Resume** queue | UI, `!tts pause/resume`, WS, UDP, Streamer.bot "Set Paused State" | |
| **Stop** | UI, `!tts stop`, WS, UDP, Streamer.bot "Stop Speaking" | Stops **only the current** utterance. |
| **Clear** pending | UI, `!tts clear`, WS, UDP, Streamer.bot "Clear Pending" | |
| **Enable / Disable** engine globally | UI checkbox, `!tts on/off`, WS, UDP, Streamer.bot "Set Enabled State" (Enabled/Disabled/Toggle) | |
| **Gag** button | UI | Silences TTS outright (v0.0.11). |
| Global volume | Settings > General > Application Volume | Volume is also set per voice. |
| **Save TTS** | Settings > General | Writes all TTS to `.wav` files in a folder. Streamer.bot's "Speak" with **Silent** returns `speechFile`, which you can play in OBS or with Play Sound. |
| **Speak-and-wait** | Streamer.bot "Speak" sub-action with **Delay** | Blocks the action until playback ends (hard 30 s timeout) and returns `duration` in ms. |
| Character usage | Tracked to `usage.dat` (v0.0.11). | Useful for paid-engine quotas. |

### 1.7 VoiceGate (the only speech-input feature)

Speaker.bot has **no speech-to-text**. Its only microphone feature is **VoiceGate** (v0.1.4):
- Monitors a microphone or input device and **auto-pauses TTS** when the input level crosses a threshold, so TTS doesn't talk over the streamer.
- A threshold and timeout auto-unpause after a VoiceGate pause. Since v0.1.5 it also unpauses when there is no input level, for example when the mic is muted.
- The three settings can be changed in real time. Nothing is recorded.
- **VoiceGate Profiles** store settings for quick switching. Streamer.bot 0.2.4+ has the **Activate VoiceGate Profile** sub-action. The WS API has `GetVoiceGateProfiles` and `ActivateVoiceGateProfile` (see 1.10) and emits a `VoiceGate.ProfileActivated` event.

Sources: [v0.1.4](https://docs.streamer.bot/changelogs/speaker.bot/v0.1.4), [Activate VoiceGate Profile](https://github.com/Streamerbot/docs/blob/main/streamerbot/3.api/1.sub-actions/speakerbot/activate-voicegate-profile.md)

### 1.8 Event and chat sources

| Source | Auth | What it feeds TTS | Notes |
|---|---|---|---|
| **Twitch** (required) | OAuth broadcaster login | Chat, events (follow, sub, resub, gift sub, gift bomb, cheer, raid, hype train, whispers), channel point rewards | 100% **EventSub** since v0.1.6: the IRC chat client was removed and PubSub removed in v0.1.5. Community Goal events died with PubSub. "Hosts" were discontinued. Hype Train moved to V2 in v0.1.6. |
| **Streamlabs** | OAuth since v0.1.5 (was an API token) | Donations | Breaking change in v0.1.5: re-login required. |
| **StreamElements** | OAuth | Tips, merch | |
| **Streamer.bot website** | OAuth | Not a TTS source; unlocks alpha and beta auto-updates | |
| **Streamer.bot app** | WS 7580 / UDP 6669 | Anything sent by Speak sub-actions or C# `TtsSpeak` | The main route for other platforms: Streamer.bot receives YouTube or Kick events and forwards them to Speaker.bot. |
| **YouTube / Kick** | Not natively supported | | The old wiki says "YouTube is not currently supported". speaker.bot hosts YouTube ToS and privacy pages ([terms.md](https://github.com/Streamerbot/docs/blob/main/speakerbot/terms.md), [privacy.md](https://github.com/Streamerbot/docs/blob/main/speakerbot/privacy.md)) that describe a YouTube integration, but no account setup or changelog mentions one. **(inferred: planned or boilerplate, not shipped)** |

Sources: [accounts.md](https://github.com/Streamerbot/docs/blob/main/speakerbot/0.get-started/1.accounts.md), changelogs v0.1.4 to v0.1.6.

**Events tab** ([events.md](https://github.com/Streamerbot/docs/blob/main/speakerbot/1.guide/1.events.md)):
- Global settings: Voice Alias override and Enabled toggle.
- Per event: **multiple messages**, each with **Weight** (weighted random pick) and an **Enabled** flag.
- **Event States** toggle each event and set **minimums** for Raid viewers, Cheers, and Donations.
- Message variables use `%var%`: `name, level, gift, subtier, title, cost, bits, amount, percent, currency, tier, recipient, cumulative, message, input`. `%daysLeft%` came with community goals. Since v0.1.6 there are separate "Sub" and "Sub with Message" strings.

### 1.9 Chat commands

**Built-in `!tts` commands** (mods/broadcaster; [commands.md](https://github.com/Streamerbot/docs/blob/main/speakerbot/1.guide/0.commands.md)). Since v0.1.6 the built-in command prefix is configurable.

| Command | Effect |
|---|---|
| `!tts status` / `!tts voices` / `!tts commands` / `!tts about` | Report status, voice counts by type, custom commands, and instance info |
| `!tts pause` / `resume` / `clear` / `stop` | Queue control |
| `!tts mode all|command` | Say everything vs. command-only |
| `!tts on|enable` / `off|disable` | Global TTS toggle |
| `!tts events on|off` | Toggle event speech |
| `!tts ignore add|del <user>` / `!tts ignored` | Per-user ignore list |
| `!tts reg add|del <user>` | Regulars |
| `!tts random reset <user>` / `!tts set sticky on|off` | Sticky random voice |
| `!tts set nickname <user> <nick>` | Nickname |
| `!tts assign last <user>` | Make the last random voice permanent |
| `!tts profile`, `profile list`, `profile activate` | Ignore-voice profiles (TwitchSpeaker v0.0.22) |
| `!tts permissions` | Toggle permission groups (v0.0.46) |

**TTS speaking commands:** `!say` and `!"` by default. Add more under Speaking Options. `Silence Command Output` mutes command chatter.

**Custom Commands tab:** each has Command text, Voice Alias, and Permissions (role checkboxes). Example: a mod-only `!shoutout` with an energetic voice.

**Channel Rewards tab:** a TTS message per channel-point reward, with weighted random messages and `%input%` (archived wiki index; TwitchSpeaker v0.0.22, v0.0.45).

**Per-message voice switching** such as `alias: text alias2: text` is **not** built in. A community Streamer.bot C# action does it: [Philipp-Codes/StreamerBotTTS](https://github.com/Philipp-Codes/StreamerBotTTS).

### 1.10 APIs

#### WebSocket server (Settings > WebSocket Server)

| Setting | Default | Notes |
|---|---|---|
| Auto Start | `false` | Without it you must click Start Server. |
| Address | `127.0.0.1` | Change it to expose on the LAN or internet. |
| Port | **7580** | The Speaker.bot docs say **7680**. Streamer.bot's integration docs and the old wiki say 7580. The docs page is probably a typo **(inferred)**. |
| Endpoint | `/` | |

The request format matches Streamer.bot's: `{ "id": "<id>", "request": "<Name>", ... }`. Source: [websocket index](https://github.com/Streamerbot/docs/blob/main/speakerbot/2.api/0.websocket/0.index.md).

| Request | Payload | Documented? |
|---|---|---|
| `Speak` | `voice` (alias), `message`, `badWordFilter` (optional bool) | Yes |
| `Stop` / `Enable` / `On` / `Disable` / `Off` | none | Yes (On/Off on the old wiki) |
| `Events` | `state: on|off` | Yes |
| `Mode` | `mode: all|command`. Fails if no commands are configured. | Yes |
| `Pause` / `Resume` / `Clear` | none | Yes |
| `GetInfo` | Returns `instanceId, name, version, os, apiVersion: 2` | **Undocumented.** Reconstructed by Speekaboo from v0.1.4 ([server.py](https://github.com/easyaspi314/Speekaboo/blob/main/speekaboo/server.py)) |
| `GetEvents`, `Subscribe`, `UnSubscribe` | `events: { group: [names] }`, with `*` for all | Undocumented; required by Streamer.bot |
| `GetAliases` | Returns `aliases[{id,name,voiceCount}]` | Undocumented |
| `GetState`, `Commands`, `GetVoiceGateProfiles`, `ActivateVoiceGateProfile` | | Undocumented |

**Subscribable events** (reported by v0.1.4, per Speekaboo):

| Group | Events |
|---|---|
| `Application` | `StartedSpeaking`, `StoppedSpeaking` |
| `TextToSpeech` | `TextQueued`, `EngineProcessed`, `Playing`, `Finished`, `Deleted`, `Error` |
| `VoiceGate` | `ProfileActivated` |

#### UDP interface

- Fixed **port 6669**, always on, no configuration. Streamer.bot used it before the WS integration.
- JSON uses `command` instead of `request`. Commands: `speak` (`voice`, `message`, `id`), `stop`, `enable`/`on`, `disable`/`off`, `profile` (`none` deselects), `pause`, `resume`, `clear`, `events` (`value: <event name>`, `state`), `reg` (add/del `user`, `id`), `set` (`method: nickname|sticky`), `assign` (`method: last`).
- Sources: [udp index](https://github.com/Streamerbot/docs/blob/main/speakerbot/2.api/1.udp/0.index.md), [udp requests](https://github.com/Streamerbot/docs/blob/main/speakerbot/2.api/1.udp/1.requests.md)

#### Streamer.bot integration (Integrations > Speaker.bot)

Settings: Auto Connect, Auto Reconnect, Address (default 127.0.0.1; LAN IPs work for multi-PC setups), Port 7580. Source: [streamerbot guide](https://github.com/Streamerbot/docs/blob/main/streamerbot/2.guide/06.integrations/speakerbot.md)

| Sub-action / trigger | Parameters and outputs |
|---|---|
| **Speak** (WS) | Voice Alias, Pass Through Bad Word Filter, **Silent** (process only, output `speechFile`), **Delay** (wait for playback, 30 s cap, outputs `duration` ms), Message. Outputs `success`. |
| **Speak (UDP)** | Voice Alias, bad-word toggle, Message. C# method `TtsSpeak`. |
| **Stop Speaking**, **Clear Pending** | none |
| **Set Enabled State** | Enabled / Disabled / Toggle |
| **Set Paused State** | Paused / Enabled / Toggle |
| **Activate VoiceGate Profile** | Profile Name, outputs `success` |
| Triggers | **Speaker.bot Connected** / **Disconnected** (SB 0.2.5) |

Sources: [sub-actions/speakerbot](https://github.com/Streamerbot/docs/tree/main/streamerbot/3.api/1.sub-actions/speakerbot), [TTS delay FAQ](https://github.com/Streamerbot/docs/blob/main/streamerbot/faq/02.tts-delay.md)

### 1.11 UI layout (older WinForms/Win32 style)

The developer called it "fugly" in TwitchSpeaker v0.0.20. Layout comes from the archived wiki pages and the docs:

| Tab | Contents |
|---|---|
| **Event Log** | Timestamped app log, mirrored to `logs/*.txt`. |
| **Processing** (was Speaking Queue + Speaking History) | Grid of queued and completed utterances: engine, voice and locale, pitch/rate/volume, user, message. History is per session only. |
| **Users** | Three panes: user list, user details form, Past Voices list. |
| **Events** | Global alias and enable at the top, then Event States checkboxes with minimum thresholds, then a per-event message list (message, weight, enabled). Events show in a nested hierarchy since v0.1.6. |
| **Custom Commands** | Command, alias dropdown, permission checkboxes, with Add, Update, and Delete buttons. |
| **Channel Rewards** | Reward to weighted TTS messages. |
| **Settings** | Sub-tabs: General (instance ID and name, Enabled, mod whispers, log level, output device, app volume, Save TTS folder, default and highlight aliases), Accounts, Speech Engines, Voice Aliases, Ignored Voices/Locales, Speaking Options, Replacement, WebSocket Server, VoiceGate. |

Other windows: Generic Speaker, speech preview/voice editor, tray icon with a tooltip.

---

## 2. SAMMI

### 2.1 Snapshot

| Attribute | Value | Source |
|---|---|---|
| Name | The docs and README say only "SAMMI", a "Streaming Assistant". The itch.io storefront expands it as "**Stream Automation, Management, Monitoring and Innovation**" (seen in a search snippet). The brief's "Multi-Machine Interface" expansion appears in **no** source I found. | [README](https://github.com/SAMMISolutions/SAMMI-Official), [itch.io](https://sammisolutions.itch.io/) |
| Current version | **SAMMI 2026.6.0**, GitHub release **2026-09-02**. Earlier 2026 releases: 2026.5.1 (2026-07-01), 2026.5.0 (2026-07-01), 2026.4.2 (2026-05-22), 2026.3.0 (2026-03-22), 2026.2.x (Feb), 2026.1.x (Jan). | [SAMMI-Official releases](https://github.com/SAMMISolutions/SAMMI-Official/releases), [patch notes](https://sammi.solutions/docs/patch-notes) |
| Lineage | Fork of **LioranBoard 2** by Lioran Waters. The founders' fork started June 2022 and was rebranded SAMMI in **August 2022**. The team today is largely **Chrizzz**, plus Landie, Digi_Bunny, and others in the patch notes. | [about/overview](https://github.com/SAMMISolutions/docs/blob/main/doc_posts/_about/overview.md) |
| Tech stack | **GameMaker Studio 2**: `data.win` runtime and an `options.ini` with an `[LLVM-WINDOWS]` YYC section. Native helper DLLs: `SAMMI.dll`, `SAMMI_V2.dll` (new dialogs, 2026.4.0), `file_dropper_x64.dll`, `gamepad_force_focus_x64.dll`. The app is a **game loop with an FPS setting**: motion commands depend on it, and FPS drops when unfocused unless `fullFPS="1"`. Bridge is HTML/JS. SAMMI Voice is a separate Windows app on Microsoft Speech Recognition. | overview; [SAMMI-Official x64/](https://github.com/SAMMISolutions/SAMMI-Official/tree/main/x64); [fps FAQ](https://github.com/SAMMISolutions/docs/blob/main/doc_posts/_faq-general/fps.md) |
| OS support | **Windows 10+ x64 only.** Wine is possible but unsupported. "Mac and Linux are not currently supported." SAMMI Panel is web-based and cross-platform. | [download](https://github.com/SAMMISolutions/docs/blob/main/doc_posts/_getting-started/download.md), README |
| Stream software | **OBS Studio** via obs-websocket 5.x (also 4.9 compat), multiple simultaneous OBS connections, **Meld Studio** (official since 2026.5.0). Streamlabs Desktop is **not** supported. | download, integrations |
| Price | **Free**. Itch.io is name-your-own-price ($0 allowed). **SAMMI Pro** (since 2026.4.0) is unlocked by an active **Patreon pledge of at least €1/month** linked via api.sammi.solutions (Twitch or YouTube account). Patreon also gives beta access. Some community extensions cost money. | [patch notes 2026.4.0](https://github.com/SAMMISolutions/docs/blob/main/doc_posts/_about/patch_notes.md), [sammi.solutions](https://sammi.solutions/) |
| License | **Contradictory.** The docs say "proprietary application" and also "SAMMI and all its components are licensed under the Apache 2.0 license". The GitHub release repo is tagged **MIT**. Itch.io says **GNU GPL v3.0**. `Licenses/SAMMI Core/` contains Apache-2.0, MIT (DLL), and MIT (LioranBoard 2) files. No Core source is published. | overview, [Licenses/](https://github.com/SAMMISolutions/SAMMI-Official/tree/main/Licenses), [itch.io/sammi](https://sammisolutions.itch.io/sammi) |
| Localization | English, English-UK, German, Spanish, French, Dutch, Italian, Russian, Portuguese (all community-translated). | overview |
| Telemetry | Anonymous analytics **on by default** (opt-out). Update check with a 7-day offline grace (2026.4.0). A **mandatory version check** was added in 2026.3.0 after Twitch contacted the team about deprecated EventSub use. | [core.md settings](https://github.com/SAMMISolutions/docs/blob/main/doc_posts/_components/core.md), patch notes |

### 2.2 Components and network ports

| Component | Role | Default port / transport |
|---|---|---|
| **SAMMI Core** | Main GameMaker app: decks, buttons, triggers, commands, connections. | Hosts the servers below |
| **SAMMI Bridge** | `bridge.html`, kept open as an **OBS custom browser dock** (recommended), a browser source, or a browser tab. Hosts **JS extensions**, the **YouTube integration**, the **Twitch Extension backend socket**, and **test triggers**. Only one Bridge may connect at a time. | WS **9425**, password optional |
| **Deck App protocol** | Remote decks: SAMMI Panel, Deck Hopper, DeckMate Control, custom apps. | WS **9470** |
| **Local API / Webhooks / SAMMI Voice** | HTTP `localhost:9450/api` and `/webhook`, with an `Authorization` header when a password is set. | HTTP **9450** |
| **SAMMI Voice** | Separate app using **Windows Speech Recognition** phrase lists, which fires "SAMMI Voice" triggers. | Calls 9450 |
| **SAMMI Panel** | Free, open-source web deck at https://panel.sammi.solutions (development resumed 2026). | WS 9470 |
| **Deck Hopper** | Officially endorsed **third-party** native deck app by founder Christina K: Windows, macOS, Linux, Android, iOS. Free trial or Pro. Its setup ships with SAMMI. | WS 9470 |
| **SAMMI Updater** | Updates Core and Bridge and migrates extensions. | |
| **Official SAMMI Twitch Extension** (2026.4.0) | Viewers press SAMMI buttons from a Twitch overlay. Bits buy "Power"; Pro users get unlimited buttons with no Power requirement. | WS backend via Bridge |
| **Elgato Stream Deck plugin** | `SAMMI.streamDeckPlugin` plus a Bridge extension. Stream Deck key press and release trigger buttons, and an "Update Action" command changes title, icon, and properties. | via Bridge |
| OBS | obs-websocket | 4455 (v5) / 4444 (v4) |
| Meld Studio | WebChannel API | 13376 |

Sources: [core.md](https://github.com/SAMMISolutions/docs/blob/main/doc_posts/_components/core.md), [bridge.md](https://github.com/SAMMISolutions/docs/blob/main/doc_posts/_components/bridge.md), [deck-app.md](https://github.com/SAMMISolutions/docs/blob/main/doc_posts/_components/deck-app.md), [voice.md](https://github.com/SAMMISolutions/docs/blob/main/doc_posts/_components/voice.md), [api docs](https://github.com/SAMMISolutions/docs/tree/main/doc_posts/_api-reference), [elgato](https://github.com/SAMMISolutions/docs/blob/main/doc_posts/_integrations/elgato_stream_deck.md), [meld](https://github.com/SAMMISolutions/docs/blob/main/doc_posts/_integrations/meld_studio.md)

### 2.3 Core concepts

#### Decks and folders
- A **deck** is a canvas grid of buttons. **All enabled decks are live at all times**, whatever a Deck App is showing. Disabled decks don't trigger, and their variables are inaccessible.
- Deck settings: Name, **Grid Snapping** (off allows free placement and size), grid **X and Y** size, **Adaptive Resizing**, background color or image, undo/redo.
- Deck operations: Add (Ctrl+N), Delete, Move Left/Right, **Copy Deck** (JSON to clipboard, optionally with images), Paste Deck, Duplicate.
- **Deck folders** (2026.5.0): drag one deck onto another. Folders can have images and are stored separately from deck JSON.
- Decks can be enabled and disabled at runtime (command, API, or Deck App) to make per-scene "modes".
- **Pro deck search** has six modes: decks/folders, button labels, button IDs, trigger names, commands, and text inside commands.

#### Buttons

A button is the unit of automation: **appearance + settings + triggers + commands + release commands + init variables**. Source: [core.md](https://github.com/SAMMISolutions/docs/blob/main/doc_posts/_components/core.md)

| Setting | Meaning |
|---|---|
| **Button ID** | Unique ID, used by Trigger Button, the API, and as the button-variable namespace. |
| **Group ID** | Buttons that share a group ID form a **mutual-exclusion queue**: they never run at the same time. |
| **Allow Queue** ("Add to Request Queue") | Triggers that arrive while the button is running are queued. Without it they are **discarded**. |
| **Allow Button Overlap** | Several instances may run concurrently. Queueing does not apply. |
| **Local Variable Persist** | On: variables survive after the run and overlapping instances share them. Off: variables are per-run and private. |
| **Expose Payload** (2026.5.0) | Exposes the trigger payload as a local `payload` object (`/$payload.user_name$/`). |
| **Press Type** | Normal, or **Drag Press** (fires when a finger or mouse drags over it; single or multiple). Used for DIY volume faders on touch decks. |
| **Release commands** | Run on release. Only for Deck App or hotkey presses. |
| **Init variables** | JSON defaults so persistent buttons don't crash on undefined variables. |
| Appearance | Text, color, image (auto-resized and copied to `SAMMI/image`), transparency, border size and color, font color, font shadow. Changeable at runtime with Modify Button/Group, which pushes to Deck Apps. |
| Indicator icons | Small badges for: has commands, sound, keyboard, triggers, overlap, persistent, queue, press type, release commands, init vars, protected, transparent. |
| Protection | Extension authors can **protect** buttons so commands can't be copied or nested commands viewed. Developer Mode can **encrypt/lock decks**. |
| Sharing | Right-click **Export JSON** / **Import from JSON** for single buttons. |
| Runtime control | **Block Button/Group** (ignore triggers), **Stop Button/Group** (2026.2 added a Stop for all queued instances), **Reinitialize Button Variables**, an **Active Buttons** window to clear ongoing or queued runs, and **Reset SAMMI**. |

#### Command execution model
- **Absolute per-command delays.** Each command has a ms offset from button start, so the list acts as a timeline and delay outranks order. Delays cannot take variables; use Wait for Timeout for that. Source: [delays.md](https://github.com/SAMMISolutions/docs/blob/main/doc_posts/_commands-introduction/delays.md)
- **Async ("purple") commands.** These return later; the classic pattern is Delete Variable, then the async command, then **Wait Until Variable Exists**. Since 2025.2.0 every async command can be switched to **sync (gray)**, and an "Async is Sync" default exists. Source: [purple-commands.md](https://github.com/SAMMISolutions/docs/blob/main/doc_posts/_commands-introduction/purple-commands.md)
- **Input box semantics.** **White** boxes take literal text plus `/$var$/` interpolation and `/$(expr)$/` math. **Yellow** boxes take expressions: bare variable names, quoted strings, `+` concatenation. Source: [input-boxes.md](https://github.com/SAMMISolutions/docs/blob/main/doc_posts/_commands-introduction/input-boxes.md)
- Commands can be disabled per row ("Off"), copied across buttons, drag-reordered, nested in **Comment** blocks, and run directly from the editor (**Ctrl+R**). F1 opens docs for the hovered command.
- **Try Block** (2026.4.0) lets inner commands fail without aborting the button. **Prevent Full Crash** is a debug setting.

#### Variables ([variable-introduction.md](https://github.com/SAMMISolutions/docs/blob/main/doc_posts/_commands-variables/variable-introduction.md))

| Scope | Notes |
|---|---|
| **Global** | Stored in the `global` object. System-populated variables live there too, for example `global.Main.current_scene`, `previous_scene`, and `connected` for each OBS connection, plus `sammi_pro`, `twitch_default_channel_id`, and Meld state. |
| **Button** | Stored on the button object. Other buttons can read them with Get/Set Button Variable if the owner is persistent. |
| **Local** | The button's own variables; used 99% of the time. |
| **Instance** | Per running instance (Set Button Instance Variable; `instanceID` in the extension API). |
| **Hidden / Filepath / Boolean** | Specialized setters. |

- Types: number, string, boolean (only `1` is true), **array**, **object** (nested JSON), undefined. Each is color-coded in the **Variable Viewer** (Ctrl+Tab), which can search, sort, copy a value or path, and delete.
- **Variable Transition** tweens a numeric variable over time.
- **No built-in persistence to disk** for button variables across restarts **(inferred)**. Users rely on INI, CSV, or JSON files or the "Persistent Variables" extension.

### 2.4 Command catalog (about 440 documented commands)

Counts come from the docs repo `doc_posts/_commands-*`, excluding intro pages. A few motion and filter commands appear in two categories. **Pro** marks commands that need SAMMI Pro (per the 2026.4.0 and 2026.5.0 notes).

| Category | Count | Notable commands |
|---|---|---|
| Arrays | 18 | Create, Insert, Remove, Pull, Find, **Filter**, **Map**, **Sort**, Shuffle, Random, Concat, Parse, Stringify, Format to String |
| Objects / JSON | 10 | Create, Get/Set Object Variable, First/Next Key iteration, Size, Parse, Stringify, Object to Array, **JSON: Get Value** (`user.name`, `items[0]`) |
| Strings | 22 | Format, Substitute Variables, Split to Array, Replace/Replace All, Position, Clamp, Hash, URL Encode, Trim, case, digits/letters filters, **Regex Is Match / Replace / Get Captures** (Pro) |
| Numbers | 9 | Random, Clamp, **Lerp**, **Remap**, Hex/Number conversion, Swap Red & Blue (BGR/RGB) |
| Date/Time | 5 | Get, Math, Diff, Duration, Convert Time (Pro) |
| Buffers (binary) | 16 | Create, Load, Save, Read, Write, Peek, Poke, Base64 encode/decode, SHA1 |
| Files: general | 16 | Read All/Line, Random Line, Append, Copy, Delete, Exists, Create Folder, Folder Get Files (Pro), Get Information (Pro), JSON to Object (Pro), SHA1, **Zip/Unzip**, Open Folder |
| Files: CSV | 19 | Spreadsheet-style CSV: Load, Get/Set Box, Add/Delete/Move Row and Column, Find, **Sort**, **Math**, Save |
| Files: INI | 11 | Load/Save Text and Number, Delete Key/Section, INI to and from Object, **Create Leaderboard** (Pro) |
| Statements and loops | 8 | If, Switch/Case (case-insensitive option), **Repeat**, Break, Exit, Re-enable, **Try Block** |
| Wait | 7 | Wait for Timeout, **Wait Until Variable Exists / Is**, **Wait for User Input / Choice / Dialog** (desktop modal prompts, now non-blocking for other buttons), Wait for Keyboard |
| Variables | 12 | Set Local/String/Boolean/Button/Instance/Hidden/Filepath, Get Global/Button, Delete, Get Type, **Variable Transition** |
| Buttons | 10 | Trigger Button, Block, Stop, **Modify Button/Group** (+ From Object), Get Color/Text/ID/Instance ID, Reinit Vars |
| Deck App | 5 | **Switch Deck** (specific/next/previous, per client), **Wait for Input / Choice / Multi Choice on the phone**, Send JSON |
| Trigger | 3 | **Trigger Pull Data**, **Pull Wildcard**, Trigger Extension Triggers |
| Misc | 19 | **HTTP Request**, HTTP Download/Upload File, **Send UDP Packet**, Command Line, Execute Program, Open URL, Clipboard get/set, Alert/Popup/Notification messages, Deck Change/Get Status, Send JSON to Extension, **Overtime**, Comment, Get Random Color (Pro) |
| Mouse and keyboard | 5 | **Keyboard Simulate Press**, **Mouse Change Position**, **Mouse Simulate Click**, Check/Wait for key status |
| Windows / system (all Pro) | 22 | Window Find/Focus/Min/Max/Restore/Close/Hide/Show/Move/Resize/Always-on-top/Get Info/Get Active; Process Is Running/Kill/Get List; master volume and mute get/set; **per-app audio level**; list audio devices |
| Sound effects | 4 | **Play Sound Effect** (mp3, wav, m4a, ogg vorbis/opus, mp4-audio; **per-command output device**; follows the Windows default device), change speed, change volume (fades), stop |
| OBS general | 9 | **Send OBS Request** (raw), **Event Subscription**, Event Pull, Trigger OBS Hotkey (by name or **key sequence**), Get Stats, Studio Mode, Virtual Camera control and status |
| OBS scenes | 11 | Switch Scene, Create, transitions (set, duration, settings, change), Studio Mode Preview set/get/activate, Get Scene List (Pro) |
| OBS sources | 23 | Visibility, Position, Scale, Rotation, Crop, Center Point, Blend Mode, Volume, Mute, Audio Monitor Type, Create/Delete/Duplicate Source, change Text (GDI+/FT2), Settings, Pull Source Value, Get Settings/Transform (Pro), **Save Screenshot** (Pro) |
| **OBS Motion** | 8 | **Animated tweening** of source Position, **Position Curve**, Scale, Rotation, Cropping, Volume, **Alpha Filter**, and arbitrary **Filter Settings**, with easing types (`box_smooth_type`), driven by SAMMI's frame loop |
| OBS filters | 6 | Visibility/toggle, Change Settings, Get Settings (Pro), Motion filter variants |
| OBS media | 6 | Play/Pause, Stop, Restart, Next, Previous, Set Time |
| OBS recording | 10 | Start/Stop/Toggle/Pause/Resume/Status, Replay Buffer control/save/status/get last |
| OBS streaming | 5 | Start/Stop/Toggle/Status, **Set Stream Caption** (Pro) |
| **Meld Studio** | ~40 | Connect, Disconnect, Show/Stage Scene, Track Mute/Monitor/Volume/Position, Layer visibility/position/size/rotation/text/URL/source, Toggle Effect, Call Function, Control Widget, Replay, Record Clip, Screenshot, Virtual Camera, Event Pull, raw Send Command |
| Twitch: chat | 13 | Send Chat (IRC), **Send API Chat Message** (with pin), Reply, Whisper, Announcement, Shoutout, Delete, **Pin/Unpin/Update/Get Pinned**, Join/Leave Channel |
| Twitch: channel points | 6 | Create, Edit (combined, 2026.5.0), Edit All, Delete Reward, Update Redemption Status |
| Twitch: info | 25 | Channel, stream, user, game, team info; followers and latest followers; subscriber count and info; chatters; **random viewer**; mods; VIPs; banned; blocked terms; emotes; chat settings; Hype Train status; **Custom Power-Ups**; **Shared Chat session** |
| Twitch: moderation | 28 | Ban, Timeout, Unban, **Warn**, Add/Remove Mod/VIP, **Suspicious User** flags, AutoMod get/set, Blocked Terms, Shield/Slow/Sub/Follower/Emote/Unique modes, Start/Cancel Raid, **Run Ad**, Modify Channel Info, Set User Color, Block/Unblock |
| Twitch: polls/predictions | 7 | Create Poll, Create Prediction (2 or n outcomes), End, Get Latest ID, Get Results |
| Twitch: misc | 11 | **Raw API Call** + Create API Header, **Create/Delete/Get EventSub subscriptions** (custom topics), Create Clip (title, duration), Create Marker, Get Videos, Ad Schedule, Snooze Next Ad, Get User Status |
| YouTube | 7 (+1) | Send Chat, Ban/Unban, Change Status (title etc.), Get Stats, User Subscription Status, Ad Break, Get Member List (2026.6.0, pending YouTube permission) |
| Integrations | 6 | **Discord: Send Message / Send File** (webhooks; Send File is Pro), **Mix It Up: Get All / Run Command**, **Aitum: Send Request** (31 templates), **Voice: Toggle Status** |

**Pro-only features** beyond commands: UI Color Menu (22 window categories, `json/ui.json`, import/export), Command Editor (hide or favorite commands and categories), **Command Templates** (save multi-command snippets to the Quickbar), Pro deck search, Event Log **replay**, and unlimited Twitch Extension viewer buttons. Source: [patch notes 2026.4.0 / 2026.5.0](https://github.com/SAMMISolutions/docs/blob/main/doc_posts/_about/patch_notes.md)

### 2.5 Triggers

Add a trigger with right-click > Edit Triggers (Ctrl+double-click). A button may have many triggers. Read data with Trigger Pull Data or the `payload` object. Source: [triggers/introduction](https://github.com/SAMMISolutions/docs/blob/main/doc_posts/_triggers/introduction.md)

| Family | Trigger types |
|---|---|
| **General** | **Gamepad** (digital press/release, analog start/change/end, stick x/y, trigger value, pad index), **Hotkey** (key + Ctrl/Alt/Shift), **Repeat Interval** (ms timer; starts at launch and can't be toggled, so it is gated with Block), **SAMMI Trigger** (Startup, Shutdown, Deck Reload, Reset, **Crash**, OBS/Bridge/Deck App/Meld/Third-party Connected and Disconnected, Deck Enabled/Disabled), **Extension Trigger** |
| **Twitch** (EventSub, with chat over IRC) | Chat (filters: Mod, Lead Mod, Sub, Broadcaster, VIP, Founder, username, case-sensitive, **own chat only** for Shared Chat; payload includes reply-thread fields, badges, emotes, `message_no_emotes`, **GIF list** in 2026.6.0), Whispers, Follow, Subscription (tier, gift, anon, community, month range), Community Gift Subs (min/max), Bits (min/max), Raid (min/max), Channel Points (name, user, input wildcard), Prediction, Poll, Hype Train (V2), Moderation (incl. Mod Anniversary), Shoutout Created/Received, Automatic Reward Redemption, Announcement, Stream online/offline, Channel Info Updated, Ad Break, Charity, Guest Star, Watch Streak, Default Power-Ups, Custom Power-Ups, (Combo, removed 2026.4.0), EventSub Status Changed, **Custom EventSub Subscription** (any topic not natively supported), Twitch Extension |
| **YouTube** (via Bridge) | Chat, Subscriber, Member, Super Chat, Super Sticker. 2026.6.0 adds Jewels, Membership Gift Sent/Received, Members-only Chat, User Banned, Poll |
| **OBS** | Any obs-websocket v4/v5 event, chosen per OBS connection |
| **Meld** | Connected, Disconnected, Session/Scene changed, Layer/Scene created/removed, Streaming/Recording changed/started/stopped |
| **Webhook** | `POST /webhook` with `trigger` name (`*` = any) plus arbitrary JSON |
| **SAMMI Voice** | Phrase string (must match the SAMMI Voice trigger list) |
| **Deck App** | `CustomMessage` with `triggerName` and data |
| **Integrations** | Elgato Stream Deck (Action ID, pressed/released), Voicemod Voice Changed, Crowd Control (Effect Request/Success/Failure/Timed Update), Pulsoid heart rate (min/max). Voicemod, Crowd Control, and Pulsoid are marked **legacy / no longer exists**. |

**Wildcards:** `*` matches zero or more characters, position-sensitive. Supported in Twitch Chat, Whispers, Channel Point input, Extension triggers, and the Gamepad target. Matching stops at the first matching trigger in insertion order. Pull values with **Pull Wildcard**. Patterns like `keyword * *` plus `keyword *` capture one word.

**Native MIDI: none.** MIDI comes through the community **"MIDI to SAMMI"** extension ([extensions list](https://sammi.solutions/extensions)). **Timers:** Repeat Interval plus the "Timers" extension. **Discord triggers:** extension only ("Discord Triggers"). **Kick:** extension only ("Kick Streaming").

### 2.6 Integrations summary

| Integration | Native? | Notes |
|---|---|---|
| OBS Studio | Native | Multiple named connections ("Main" plus others), ws v4 and v5, auto-connect, non-blocking connect, global per-OBS state variables. Dual-PC works by IP. |
| Twitch | Native | **Streamer** account plus **Bot** accounts (bot scopes: `channel:moderate`, chat and whisper read/edit). "Join chat under this name" picks the sender. Events are limited to **one** EventSub account. **Re-link every 60 days**, with reminders. Dev Mode exposes advanced scopes. "Onboarded Streamer Support" toggles restricted EventSub topics. Channel-point editor built in. |
| YouTube Live | Native, but implemented in **Bridge** | The refresh token comes from an auth page; the OAuth exchange goes through the team's **AWS API** (credentials stay local). A **shared global quota of 8,000,000 units/day** across all SAMMI users and **500 units/min per user**. Each chat send costs 20 units and each ban 200. |
| Meld Studio | Native (2026.5.0) | WebChannel on port 13376. |
| Elgato Stream Deck | Plugin + extension | |
| Discord | Native commands (webhook send message/file) | Bot-level triggers via extensions. |
| Mix It Up | Native commands | Requires the Mix It Up Developer API. |
| Aitum (OBS plugin) | Native command | 31 request templates. |
| Voicemod, Crowd Control, Pulsoid | Legacy extensions | Marked "no longer exists". |
| Twitch Extension (viewer buttons) | Official, 2026.4.0 | Bits-based "Power" system. |

### 2.7 Bridge and the extension system

- **Format:** `.sef` (or legacy `.lb2`) text file with INI-like sections: `[extension_name]`, `[extension_info]`, `[extension_version]`, `[insert_external]` (HTML/CSS tab UI inside Bridge), `[insert_command]` (declares commands via `SAMMI.extCommand`), `[insert_hook]` (deprecated; use listeners now), `[insert_script]` (JS), `[insert_over]` (a **deck JSON installed with the extension**, plus requirement keys: `sammi_version`, `bridge`, `obswebsocket`, `required_extension`, `extension_triggers`, `donot_include_deck`). Installing **splices the code into `bridge.html`**. Sources: [How to make an extension.txt](https://github.com/SAMMISolutions/SAMMI-Official/blob/main/bridge/How%20to%20make%20an%20extension.txt), [extensions/build](https://sammi.solutions/extensions/build)
- **Helper API** (`SAMMI.*`, promise-based over the SAMMI WebSocket library): get, set, and delete variables; insertArray/deleteArray; **extCommand**; triggerExt; trigger/release/modify/edit button; INI load/save; openURL; httpRequest; popup, alert, notification; getDeckList/getDeck/getDeckStatus/changeDeckStatus; getImage; getActiveButtons/getModifiedButtons; getTwitchList; testTrigger; and more. Source: [SAMMI-Bridge README](https://github.com/SAMMISolutions/SAMMI-Bridge)
- **Extension-defined commands look native.** `SAMMI.extCommand(name, color, height, boxes, triggerButton, hidden, rowUnits, rowHeights)` declares typed parameter boxes. There are about 46 box types: text, multiline, checkbox, OBS scene/source/filter pickers, key picker, compare and math operators, sound picker, slider, color, dropdowns, Twitch reward picker, YouTube account/category, deck picker, password, Twitch account, Meld scene/track/layer/effect pickers, and **Save Variable** (makes the command sync with a timeout). Commands can be multi-row.
- **Marketplace:** https://sammi.solutions/extensions lists about 120 extensions in 9 categories: Alerts, Games, Twitch, YouTube, Social, Music & Audio, Productivity, Misc, Utilities. Examples: Spotify, Ko-fi, Patreon, Kick, Discord, Philips Hue, LIFX, Elgato Lights, VTube Studio, VeadoTube, ChatGPT, Speech to Text, **TTS by K**, **Uberduck TTS**, Translate, Loyalty, Giveaway, Quotes, MIDI, Gamepad, Thermal Printer, QR code, OCR, "Javascript Eval", Websocket Connector. Authors **self-host** (GitHub suggested). The site does not host files; some are sold on itch.io, Ko-fi, or Gumroad. The full list lives in Discord `#releases`.
- **Policy:** originality, no copying without consent, no hazardous extensions, testing on a clean install. Closed-source extensions must give the team a way to verify their source. Violators are marked "restricted". There is no formal sandbox: extensions run with full Bridge/browser privileges **(inferred)**. Source: [extensions/policy](https://sammi.solutions/extensions/policy)
- **Bridge tabs:** Settings (connection, appearance, message logging), Extensions (installed and update status), **Twitch Triggers** and **YouTube Triggers** test panels that send fake payloads for every event.

### 2.8 Deck App protocol and remote control

SAMMI documents a public WS protocol on **9470** ([deck-app.md](https://github.com/SAMMISolutions/docs/blob/main/doc_posts/_components/deck-app.md)), with a JS client library at [SAMMI-Websocket](https://github.com/SAMMISolutions/SAMMI-Websocket) and a spec repo at [Deck-App-Websocket-Protocol](https://github.com/SAMMISolutions/Deck-App-Websocket-Protocol).

| op | Name | Direction |
|---|---|---|
| 1 | Heartbeat | both |
| 2 | Identify (`clientName`, `authentication` hash) | client to server |
| 3 | Identified | server to client |
| 4 / 5 | Request / Response (`requestName`, `requestData`, `requestSuccess`, `responseData`) | |
| 6 | Event (`eventType`, `eventData`) | server to client |
| 7 | Close | both |

- **Requests:** GetDeckList, GetDeck (full layout; positions as **0–1 fractions**; colors as **BGR ints**; command lists stripped), GetImage (base64, CRC-cached), GetModifications, GetOngoingButtons, TriggerButton, ReleaseButton, StopButton, BlockUnblockButton, ReinitButtonVars, Get/ChangeDeckStatus, **InputRequestReply**, **CustomMessage**, **EditButton** (opens the editor on the PC).
- **Events:** ButtonTriggered, ButtonEnded, ReleaseEnded, ButtonModified, DeckStatusChanged, **DeckUpdated** (live layout push), SwitchDeck/Next/Previous, SAMMIReset, **WaitForInput** (phone-side prompts), SendJSON.
- **Remote:** the docs suggest **ngrok TCP 9470** for internet access ([remote-connection.md](https://github.com/SAMMISolutions/docs/blob/main/doc_posts/_faq-general/remote-connection.md)). There is no built-in relay or auth beyond the password hash.

**"Multi-machine" in practice:** OBS on another PC by IP; Deck Apps over LAN or ngrok; SAMMI Voice on another PC through the Local API; Bridge in a browser on another machine **(inferred possible)**. There is **no SAMMI-to-SAMMI instance federation** (unlike Streamer.bot Remote).

### 2.9 Local API and webhooks
- `GET`/`POST` `http://localhost:9450/api` (POST takes a JSON body). Requests: `getVariable`, `getDeckStatus`, `setVariable`, `deleteVariable`, `insertArray`, `deleteArray`, `changeDeckStatus` (0/1/2 toggle), `triggerButton`, `releaseButton`, `modifyButton` (text, color, image, border, border_color, font_color, font_shadow), `popupMessage`, `alertMessage`, `notificationMessage`.
- Errors follow the usual HTTP codes (400, 401, 404, 405) with `{Error, Description}`. Docs warn that C# clients must disable `Expect: 100-Continue`.
- `POST /webhook` with `{trigger, ...anyData}` fires matching Webhook triggers. It is **local only** unless exposed by the user.
- Sources: [api-reference](https://github.com/SAMMISolutions/docs/tree/main/doc_posts/_api-reference), [api-webhooks](https://github.com/SAMMISolutions/docs/tree/main/doc_posts/_api-webhooks)

### 2.10 SAMMI Voice (speech recognition)
- Uses **Windows Speech Recognition** (needs Windows SR set up and ideally trained). Spanish must be "Spanish (Spain)".
- Settings: IP and port (the Local API on **9450**, which must be enabled), password, Listen on Startup, Minimize to tray, Closing confirmation, **Recognition logfile** (results with confidence), **Confidence Level** threshold.
- **Triggers tab:** one phrase per line, editable only while not listening. A button with a "SAMMI Voice" trigger and the same phrase fires on recognition.
- The **Voice: Toggle Status** command starts and stops listening.
- No dictation or free-text STT is exposed **(inferred)**. "Speech to Text" exists only as a community extension.
- Source: [voice.md](https://github.com/SAMMISolutions/docs/blob/main/doc_posts/_components/voice.md)

### 2.11 TTS in SAMMI
SAMMI Core has **no native TTS**. TTS comes from community extensions: "TTS by K", "Uberduck.AI Speech Synthesis", or Mix It Up/Streamer.bot interop. Source: [extensions](https://sammi.solutions/extensions).

### 2.12 UI/UX description

**Main screen** ([core.md](https://github.com/SAMMISolutions/docs/blob/main/doc_posts/_components/core.md)):
- A dark, game-engine-rendered UI. It is GameMaker-drawn, not native widgets, with a custom color picker and dialogs.
- **Left sidebar:** Edit Deck, Add/Delete Deck, Move Left/Right, Copy/Paste/Duplicate Deck, **Deck App** menu (Open Panel, Open Panel (Web), Get LAN IP, Open Deck Hopper), **SAMMI Voice**, **Bridge** menu (select, reload, open in browser, reveal, copy path, install/uninstall/reinstall extensions), **Integrations**, **About**, Pro tools. Bottom: **connection lamps** (Main OBS, Meld, Twitch EventSub, Twitch Chat, Bridge, SAMMI Pro), Discord and Patreon links. The sidebar collapses below a column threshold.
- **Center:** a grid of **deck tiles** (enabled or disabled, folders), with an "Add new deck" tile. Top-right shows an **FPS counter**.
- **Bottom bar:** Settings, Connections (shared OBS/Twitch/YouTube/Meld button since 2026.5.0), **Variable Viewer**, **Active Buttons**, **Log** (Event Log, Ctrl+E), version.

**Deck editor:**
- A free-form or snapped **canvas grid**. Double-click empty space to create a button.
- Right-click a button for Appearance (Shift+double-click), Settings/Triggers (Ctrl+double-click), Commands (double-click), Release Commands, Init Variables, Copy, Delete, Export JSON, and copyable Group ID and Button ID.
- **Multi-select** (Ctrl+click), group drag, copy/paste preserving layout, undo/redo, Ctrl+S save and exit.

**Button command editor:**
- A vertical list of **command rows**. Each row has a drag handle ("ball"), the command name (colored: purple for async, red when disabled, extension-defined colors for extension commands), typed input boxes (white or yellow; dropdowns; pickers), a **delay (ms)** field, and an **Off** toggle.
- **+** (Ctrl+N or Ctrl+Enter) opens a categorized command picker with search, Pro subcategories, and favorites.
- The **Quickbar** (2025.2.0) offers common commands plus context-sensitive inserts (Case inside a Switch, Wait Until Variable Exists after async).
- Also: scissors for copy and paste, Undo/Redo, **Run (Ctrl+R)**, Save (Ctrl+S), multi-line command rows (2026.5.0), and an F1 docs link.

**Other windows:**
- **Trigger editor:** a list of triggers, each with type-specific fields.
- **Variable Viewer:** global variables (red) and per-button variables, with search, sort, copy value or path, and delete.
- **Active Buttons:** clear all, ongoing, or queued.
- **Event Log:** full payloads with copy; replay with Pro.

**Shortcuts:** Ctrl+Tab (variables), Ctrl+E (log), Ctrl+U (Pro colors), Ctrl+H (shortcut help), Esc. Source: [keyboard-shortcuts](https://github.com/SAMMISolutions/docs/blob/main/doc_posts/_faq-general/keyboard-shortcuts.md)

---

## 3. Unique to Each (for the unified app)

The unified app's baseline is Streamer.bot. Its feature set is taken from the [Streamer.bot docs tree](https://github.com/Streamerbot/docs/tree/main/streamerbot): actions, sub-actions, triggers, C# code, commands, timers, hotkeys, **Voice Control** (Windows SR with Exact/Start/Anywhere and confidence), **MIDI**, WebSocket and HTTP servers, UDP, file watcher, quotes, user groups, globals, Twitch/YouTube/Kick, OBS/SLOBS/Meld/PolyPop, the built-in **multi-chat client**, the **HTML Overlay**, **Streamer.bot Decks** (web-based and website-linked; docs still WIP), **Streamer.bot Remote** (instance-to-instance signals and triggers), and many integrations (Voicemod, VTube Studio, Pulsoid, Crowd Control, Ko-fi, Patreon, Streamlabs, StreamElements, Elgato Wave Link/Camera Hub/Stream Deck, and others).

### 3.1 SAMMI has; Streamer.bot lacks (or has only weakly)

| # | SAMMI feature | Status in Streamer.bot |
|---|---|---|
| 1 | **Frame-based animation/tween engine**: OBS Motion commands for position, **bezier/curve paths**, scale, rotation, crop, volume, alpha, any filter property, with easing types; also **Variable Transition** | No native tween sub-actions; users rely on OBS Move plugin **(inferred)** |
| 2 | **Spatial deck/button-grid authoring**: the automation **is** the button (appearance + triggers + commands + release commands), on a WYSIWYG canvas with images, borders, and free placement | SB is action-list/tree based. SB Decks exist but are web/website-linked and documented as WIP |
| 3 | **Button concurrency model** per button: Group ID mutual exclusion, Allow Queue vs discard, Overlap, Block, Stop-all-queued, Reinit | SB has action queues (blocking or concurrent); per-button discard and overlap semantics are different **(inferred)** |
| 4 | **Drag-press buttons** (DIY faders) and **release-command** lists (press and hold) | Not present **(inferred)** |
| 5 | **Live runtime button restyling** (Modify Button: text, color, image, border, font) pushed to every deck client | SB Decks has item set title/icon/background/state, so partial overlap |
| 6 | **Open, documented Deck App WS protocol** with **native third-party mobile apps** (Deck Hopper iOS/Android/desktop, DeckMate) and **interactive prompts on the phone** (Wait for Input/Choice/Multi-choice) | Not equivalent **(inferred)** |
| 7 | **Desktop modal prompts inside automations** (Wait for User Input, Choice, Dialog with dropdown; Popup) | Not documented in SB **(inferred)** |
| 8 | **JS extension runtime (Bridge)** where extensions add **native-looking commands with typed parameter widgets** (OBS pickers, reward pickers, sliders, color, file pickers), **ship their own decks**, declare requirements, and get version checks; plus a **community marketplace** of about 120 extensions | SB extends via C# code and import strings; there is no in-app typed-widget command SDK or marketplace **(inferred)** |
| 9 | **Absolute per-command delay timeline** inside a button | SB uses sequential Delay sub-actions **(inferred)** |
| 10 | **First-class data structures without code**: arrays and objects with map, filter, sort; JSON path get; **binary buffers**; **CSV "spreadsheet" DB** with sort and math; **INI DB with leaderboard generation**; zip/unzip | SB covers these mostly via C# **(inferred)** |
| 11 | **Typed live Variable Viewer** (color-coded, copy path), **Active Buttons** monitor, **Event Log with full payload and replay** (Pro) | SB has variable views and action history; replay of logged events is unclear **(inferred partial)** |
| 12 | **Native gamepad trigger** (buttons, sticks, analog triggers) | Not native **(inferred)** |
| 13 | **Window/process/system-audio control** (Pro): focus, move, resize, always-on-top, kill process, **per-application volume**, list devices | SB has some system sub-actions; per-app audio and window management are absent **(inferred)** |
| 14 | **Mouse movement and click simulation** | SB has keyboard press; mouse simulation not documented **(inferred)** |
| 15 | **Official Twitch Extension** for **viewer-clickable buttons** with a Bits-funded "Power" economy | Not present |
| 16 | **Bridge test-trigger panels** that fake every Twitch and YouTube event payload | SB has a Test trigger plus per-trigger test, so mostly overlap |
| 17 | **Twitch Custom EventSub Subscription** trigger and command (any topic) plus a raw Twitch API Call command | SB has custom code paths; generic EventSub passthrough unclear **(inferred)** |
| 18 | **Aitum** request templates, **Mix It Up** command interop | Not present **(inferred)** |
| 19 | **Per-sound-command output device** (Play Sound Effect) | SB Play Sound has device selection, so likely overlap **(inferred)** |
| 20 | **UI localization** (9 languages) and a **user-themable UI** (Pro color editor) | SB UI is English-only **(inferred)** |
| 21 | **Protected or encrypted decks** for paid extension authors (Developer Mode) | Not present **(inferred)** |
| 22 | Free **Local HTTP API** that sets, gets, and modifies **variables and button visuals** directly | SB HTTP and WS servers trigger actions and globals, so overlap except for button visuals |

### 3.2 Speaker.bot has; must be merged (Streamer.bot has no native TTS engine layer)

Streamer.bot's own docs route all TTS through Speaker.bot (Speak / Speak (UDP) sub-actions). Everything below is therefore net-new for a Streamer.bot-based core.

| # | Speaker.bot capability |
|---|---|
| 1 | **Multi-engine TTS abstraction**: SAPI5, Azure, Polly, Google, Watson, ElevenLabs, TTS Monster, UberDuck, Acapela, CereProc, with unified voice listing and per-engine credential setup (JSON key, key and region, IAM keys, .env, overlay-URL parsing) |
| 2 | **Voice Aliases**: multi-voice groups with per-voice **pitch, rate, volume, emphasis** (engine-specific ranges) and **per-alias output device** |
| 3 | **Random voice assignment** with **ignore profiles** (voices and locales), **sticky random**, and **assign-last-voice-to-user** |
| 4 | **Per-user TTS profile**: voice, nickname, Ignored, **Forced**, Regular, Past Voices history |
| 5 | **Speaking modes**: say-everything vs command-only; custom TTS commands with alias and role permissions; channel-point TTS; highlight/mention voice |
| 6 | **Text pipeline**: bad-word filter (Strip or Yeet All), **ordered regex replacements**, URL filter (Strip or Yeet All), max words/chars, ignored prefixes, per-provider emote stripping (Twitch, BTTV, FFZ, 7TV, Cheermotes, Twemoji) with allow-first-emote and an allow-list, username "said" prefix, username grace period and only-if-different, nickname substitution, Shared Chat ignore |
| 7 | **Moderation-coupled skipping**: stop current and drop queued TTS on message delete, timeout, or ban |
| 8 | **TTS queue**: pause, resume, stop current, clear, enable/disable, Gag, events on/off |
| 9 | **Event-to-speech templates** with **weighted random variants**, per-event enable, and minimum thresholds (bits, raid size, donation), plus Streamlabs and StreamElements donations, tips, and merch |
| 10 | **`!tts` moderator chat command suite**, including whisper-based mod control |
| 11 | **VoiceGate**: mic-level-based auto-pause and resume of TTS, with profiles |
| 12 | **Render-to-file** (`Silent` mode returns a WAV path) and **speak-and-wait** (`Delay` returns duration) for synchronizing alerts and overlays |
| 13 | **Character-usage tracking** for paid engines; save-all-TTS-to-WAV archive |
| 14 | **Speaker.bot WS (7580) and UDP (6669) protocols**, including Subscribe events `TextQueued`, `EngineProcessed`, `Playing`, `Finished`, `Error` (useful for captions and overlays) |
| 15 | **Generic Speaker** test pad and **Processing view** (queue and history with engine, voice, and params per line) |

### 3.3 Already overlapping (deduplicate in the unified app)
- SAMMI Voice vs SB Voice Control: both use Windows Speech Recognition, and SB's version is richer (location modes, per-command confidence).
- Hotkeys, timers, webhooks/HTTP, UDP send, OBS multi-connection, Meld, Twitch/YouTube coverage, Discord webhooks, Stream Deck plugin, test triggers, file I/O, globals.
- SAMMI lacks **Kick**, **Streamlabs Desktop**, **native MIDI**, **C#/real scripting** (only extensions plus a "Javascript Eval" extension), quotes, user groups, a multi-chat client, and instance-to-instance remote. SB is ahead on all of these.

---

## 4. Parity checklist / implementation notes

| Area | Note for the unified cross-platform app |
|---|---|
| TTS engines | SAPI5 is Windows-only. For parity on macOS and Linux, add OS-native engines (AVSpeechSynthesizer, speech-dispatcher/eSpeak) and local neural engines (**Piper**, Kokoro), since cloud engines alone price out hobbyists. Speekaboo and OpenSpeaker show demand. |
| Voice params | Normalize pitch, rate, volume, and emphasis to a common scale, then map to SSML, `prosody`, or engine-native params. Keep the ElevenLabs stability and similarity mapping. |
| Audio | Per-alias and per-command output device selection; follow-OS-default behavior (SAMMI 2026.4); WAV render cache. |
| API compatibility | Optional shim listeners for **Speaker.bot WS 7580 / UDP 6669**, **SAMMI Local API 9450 / webhooks**, and the **SAMMI Deck App protocol 9470**. This lets existing Streamer.bot setups, Deck Hopper, and SAMMI Panel work unchanged. Mind the licensing (section 5). |
| Engine loop | SAMMI's motion features rely on a fixed-FPS loop. A tween scheduler at stream FPS (independent of UI focus) avoids SAMMI's "unfocused FPS throttling" pitfall. |
| Extension model | SAMMI's typed-widget command declaration (`extCommand` box types) is a good model for plugin-defined steps. Run plugins sandboxed, which SAMMI does not. |
| Data model | Button = {appearance, settings(queue/overlap/group/persist/pressType), triggers[], commands[], releaseCommands[], initVars}. Colors are BGR ints and positions are 0–1 fractions in SAMMI's JSON, which an importer must handle. |
| YouTube | SAMMI and Speaker.bot both rely on a vendor-owned Google project and quota. The unified app needs its own verified Google OAuth app and quota plan. |

---

## 5. Open questions for the project owner

1. **TTS engine scope for v1:** which engines are required on day one? Should we add local/offline engines (Piper, Kokoro, XTTS) and TikTok-style voices that Speaker.bot lacks? Is cross-platform TTS (macOS and Linux) a must?
2. **Compatibility shims:** should the app emulate the Speaker.bot WS/UDP API and/or the SAMMI Local API and Deck App protocol so users can keep Streamer.bot sub-actions, Deck Hopper, and SAMMI Panel? (A clean-room implementation from public docs seems feasible.)
3. **Importers:** import SAMMI decks/buttons JSON and Speaker.bot `voicealiases.dat`/`users.dat` (LiteDB)? Run SAMMI `.sef` JS extensions in a sandboxed browser context, or not at all?
4. **Authoring paradigm:** SAMMI's spatial button-grid editor, Streamer.bot's action/trigger lists, or both views over one model?
5. **Animation engine:** should native OBS/Meld tweening (SAMMI Motion parity) be in scope, or should we rely on OBS plugins?
6. **Viewer-facing Twitch Extension** (SAMMI-style viewer buttons with Bits): in scope? It needs a hosted backend and Twitch extension review.
7. **Monetization:** free core plus a supporter tier like SAMMI Pro (theming, templates, search, replay)? Which features, if any, may be gated?
8. **Moderation ethics:** keep regex replacement (Speaker.bot's dev warns about abuse)? Should the defaults be safer, for example audit logging of replacements?
9. **Speech input:** go beyond Windows Speech Recognition (SAMMI Voice, SB Voice Control) with cross-platform STT such as Whisper or Vosk, and VoiceGate-style ducking or auto-pause driven by mic activity?
10. **Kick/YouTube TTS:** Speaker.bot is Twitch-only natively. Confirm TTS must cover all platforms, including Kick and YouTube Super Chats.
11. **Licensing caution:** SAMMI's license signals conflict (proprietary vs Apache-2.0 vs MIT vs GPLv3), and Speaker.bot is closed. Confirm a strict clean-room policy, with no code or asset reuse.

---

## 6. Source index

- Speaker.bot site and docs: https://speaker.bot ; docs source https://github.com/Streamerbot/docs/tree/main/speakerbot
- Speaker.bot changelogs: https://docs.streamer.bot/changelogs/speaker.bot/v0.1.1 … /v0.1.6
- Streamer.bot Speaker.bot integration: https://github.com/Streamerbot/docs/blob/main/streamerbot/2.guide/06.integrations/speakerbot.md and `streamerbot/3.api/1.sub-actions/speakerbot/*`
- Retired TwitchSpeaker wiki via Wayback: `wiki.streamer.bot/en/TwitchSpeaker/*` (snapshots 2022-12-08, 2023-01-25, 2024-05-26), covering Voice Aliases, Speaking Options, Replacement, Users, Events, Speaking Queue/History, WS Requests, engine pages, and Changelogs v0.0.7–v0.0.48
- Speaker.bot-compatible OSS: https://github.com/EvaisaDev/OpenSpeaker , https://github.com/easyaspi314/Speekaboo (`speekaboo/server.py`)
- SAMMI site: https://sammi.solutions/ ; docs https://sammi.solutions/docs ; docs source https://github.com/SAMMISolutions/docs
- SAMMI releases and patch notes: https://github.com/SAMMISolutions/SAMMI-Official (releases, `patch notes.txt`, `announcement.md`, `Licenses/`, `x64/`, `bridge/`) and https://sammi.solutions/docs/patch-notes
- SAMMI Bridge: https://github.com/SAMMISolutions/SAMMI-Bridge ; extensions https://sammi.solutions/extensions , /extensions/build , /extensions/policy
- SAMMI itch.io: https://sammisolutions.itch.io/sammi
- Deck Hopper: https://christinak.ca/deckhopper/
- Streamer.bot docs tree (for comparison): https://github.com/Streamerbot/docs/tree/main/streamerbot

> **Security note:** a GitHub repo named `DaimyoMelody/speakerbot-voice-hub` (pushed 2026-10-15) markets a "Speaker.bot Voice Hub" with inflated badges ("50K+ downloads") and download links to unrelated `*.github.io` pages. It matches common malware-lure patterns. Do **not** reference it or download from it.
