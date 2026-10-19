# Discord companion bot / integration for Streamer.live

Research date: **2026-10-18**. Scope: a local-first Discord integration for a Rust/Tokio/Tauri 2 app that also runs headless as `streamer-lived`. Everything below was checked against current Discord docs, support-dev articles, the changelog, crates.io, npm and GitHub on this date unless marked **(unverified)**. Ratings: **GREEN** = works locally, no approval; **YELLOW** = works with caveats (privileged intent, relay, fragile library, policy care); **RED** = not realistic or not allowed.

---

## 0. Summary

| Question | Answer |
|---|---|
| Hosting model | **Bring-your-own (BYO) bot.** The streamer creates a Developer Portal app, pastes the token. The Gateway is an outbound WebSocket, so there is no public URL. A shared official bot needs 24/7 infra, verification, per-customer state and a relay back to every local install, which breaks local-first. |
| Interactions without an Interactions Endpoint URL | **Confirmed.** `INTERACTION_CREATE` arrives over the Gateway. Replies go out over HTTPS. The two delivery modes are mutually exclusive. |
| Privileged intents | **Rules changed 2026-06-10.** Under **10,000 users** you just toggle them in the Developer Portal (the old "under 100 servers" test is gone). At 10,000 users you must apply, then reapply yearly. This matters for BYO bots in big communities. |
| Voice | TTS into a voice channel is **possible but YELLOW**. DAVE (E2EE) has been mandatory since 2026-03-01. `songbird` 0.6.x (DAVE) and `@discordjs/voice` 0.19.x (DAVE) both support it, but songbird has an open reliability issue. Receiving voice is technically supported but **RED for MVP**. |
| Voice-activity overlay | Discord's own StreamKit URL in an OBS browser source works today and costs nothing (GREEN). Local RPC `rpc.voice.read` is approval-gated for general apps (BYO-owner/tester route is plausible, **unverified**). A bot sitting in the voice channel does receive "speaking" signals (YELLOW). |
| Libraries | Rust: **twilight 0.17.1 + songbird 0.6.x** for an embedded/dynamic design; serenity 0.12.5 is the last 0.12 release; poise 0.7.1 is macro/compile-time command oriented. TS: **discord.js 14.27.0**. |
| Build as | **First-party external sidecar plugin** (dogfoods the SDK, isolates the voice/MLS stack), with the trivial **webhook-only** poster also available without a bot. |

---

## 1. Already covered elsewhere (not repeated)

- `platform-apis.md` section 8 and 11: Discord via pasted **webhook URL** (GREEN, local) and optional **BYO bot token**; `webhook.incoming` OAuth needs a client secret so "paste a webhook" wins; 50 req/s global, Gateway 120 events/60 s, 1,000 IDENTIFY/24 h; library versions as of 2025-12; the relay/token-broker design.
- `app-integrations-vtuber.md` (table row "Discord"): local RPC over IPC (`discord-ipc-N`), WebSocket RPC deprecated, `rpc.voice.read/write` approval-gated, voice overlay Tier 3.
- `plugin-sdk-and-api.md`: sidecar tier (JSON-RPC over stdio), `chatSources` contribution, `secrets:own`, `host/secrets.get`, "at least two first-party integrations ship as external plugins in v1".

New here: the 2026-06-10 intent-rule change in detail, DAVE and library status, voice/overlay options, feature matrix, policy reading, wizard and phasing.

---

## 2. Hosting model

### 2.1 BYO bot vs shared official bot

| Aspect | **BYO bot (recommended)** | Shared official "Streamer.live" bot |
|---|---|---|
| Who creates the app | Streamer, in the Developer Portal; pastes the token | We do |
| Infra | None. Runs inside the desktop app or `streamer-lived` | 24/7 hosted gateway workers, DB, secrets, on-call |
| Local state access | Direct (variables, OBS, loyalty, clips) | Needs a persistent authenticated tunnel/relay to each install, plus an offline queue |
| Verification | Not needed. Unverified apps are capped at **100 servers** ([verification article](https://support-dev.discord.com/hc/en-us/articles/23926564536471-How-Do-I-Get-My-App-Verified), updated 2024-08-30; Stripe ID check of the team owner). A streamer uses 1-3 guilds | **Required** past 100 servers. Also the 10,000-user intent review applies to the whole product, not per streamer |
| Privileged intents | Per-streamer toggle (<10k users) | One app for all customers; review at 10k users, then annual reapply |
| Data obligations | The streamer is the "developer"; Streamer.live servers never see API Data | We become a developer holding other people's API Data: privacy policy, deletion, encryption at rest, breach notice ([Developer Terms](https://support-dev.discord.com/hc/en-us/articles/8562894815383-Discord-Developer-Terms-of-Service) section 5) |
| Branding | Bot has the streamer's own name/avatar | One shared identity |
| Onboarding friction | ~8 steps (the wizard in section 8.4 automates validation) | One click invite |
| Token risk | Token = full bot control; keep in OS keychain / daemon secret store | Our central secret store is a high-value target |

A hosted bot could be added later as an optional paid convenience. The decision for v1 is BYO only.

### 2.2 Gateway, interactions, offline behaviour

- **Gateway is outbound only.** The bot opens a WSS connection to Discord and sends `IDENTIFY` ([Gateway](https://docs.discord.com/developers/events/gateway)). No inbound port, no public URL.
- **Interactions over the Gateway: confirmed.** From [Receiving and Responding](https://docs.discord.com/developers/interactions/receiving-and-responding): an app receives an interaction "via `INTERACTION_CREATE` gateway event" or "via outgoing webhook", and "these two methods are **mutually exclusive**". The Gateway method "may be handled by connected clients"; the webhook method "does not require a connected client". For Gateway delivery "you have to respond via HTTP" (`POST /interactions/{id}/{token}/callback`; responses are not sent over the Gateway).
  - Deadline: initial response within **3 seconds** or the token is invalidated. Tokens last **15 minutes** for follow-ups/edits. So: ack immediately (`DEFERRED_CHANNEL_MESSAGE_WITH_SOURCE`), run the action, then edit.
  - **Wizard trap:** if an Interactions Endpoint URL is set in the portal (copy-pasted tutorials do this), Gateway delivery silently stops. `GET /applications/@me` returns `interactions_endpoint_url`; the wizard should detect and clear it ([Application resource](https://docs.discord.com/developers/resources/application)).
- **While the app is offline** (documented behaviour is thin; the following is partly inference):
  - The bot has no Gateway session, so it shows **offline**. Registered slash commands stay in the command picker because they live on Discord's side.
  - An interaction nobody answers within 3 s fails. The docs only state the token is invalidated; the client-side wording ("The application did not respond") is **(unverified)** here.
  - Old messages keep their buttons. Clicking one while offline fails the same way, so encode state in `custom_id` (max 100 chars) and make handlers stateless. They work again after reconnect.
  - Events that happened while offline are not replayed after a fresh IDENTIFY (RESUME only covers a short gap). Therefore **reconcile on startup**: re-read the live/offline state of each platform and edit any stale "LIVE" announcement; re-sync roles; page recent channel history if needed.
- **How `streamer-lived` solves it:** run the Discord module in the always-on daemon (home server, mini PC, VPS). The bot stays online, can answer `/live`, `/schedule`, `/points`, and keeps announcement messages correct even when the gaming PC and Tauri UI are off. Actions that need OBS go over LAN to the OBS WebSocket on the streaming PC. The desktop app then attaches to the daemon rather than opening its own Gateway session.
- **Single-owner lock:** two processes with the same token both receive every event and both try to answer, so the second callback fails (already acknowledged). Use a lease/lock in the core so only one process hosts the Discord connection (daemon wins when present).
- **Optional "never miss an interaction" mode (later):** point the Interactions Endpoint URL at the optional relay. The relay verifies Discord's Ed25519 signature, forwards to the local app over a WebSocket when online, and answers with an ephemeral "streamer's Streamer.live is offline" message otherwise. Trade-off: this switches off Gateway interaction delivery (mutually exclusive), needs the relay, and adds a trust point. Not for MVP.

### 2.3 Sharding and connection limits

- Sharding is mandatory only at **2,500+ guilds** ("each shard can only support a maximum of 2500 guilds") ([Gateway](https://docs.discord.com/developers/events/gateway)). A single-guild bot never needs it: use shard 0 of 1. The Sep 2026 changelog item about large-bot sharding (Get Current User Guilds needs `shard`) does not apply.
- **IDENTIFY is limited to 1,000 per 24 h**, and 120 Gateway events/60 s. A crash-looping local process can burn the IDENTIFY budget: implement RESUME, exponential backoff, and respect `session_start_limit` from `GET /gateway/bot`.
- Rate limits are per-bot (token), so a household sharing an IP does not share the quota. The **invalid-request limit (10,000 401/403/429 per 10 min) is per IP**, so back off on 401/403, and never retry a webhook that returned 404 ([Rate limits](https://docs.discord.com/developers/topics/rate-limits)).

---

## 3. Intents, permissions, verification

### 3.1 Intent rules (current)

Source: [What are Privileged Intents?](https://support-dev.discord.com/hc/en-us/articles/6207308062871-What-are-Privileged-Intents) (updated 2026-06-10), [Changes to Privileged Intent Access](https://support-dev.discord.com/hc/en-us/articles/40281523410967-Changes-to-Privileged-Intent-Access-for-Discord-Apps) (2026-06-11), [Gateway: privileged intents](https://docs.discord.com/developers/events/gateway).

- Privileged intents: **GUILD_PRESENCES**, **GUILD_MEMBERS**, **MESSAGE_CONTENT**. Must be toggled in the portal before IDENTIFY. If not enabled, the Gateway closes with **4014**. Invalid intents give **4013**.
- **Before 2026-06-10:** <100 servers = just toggle; 100+ = apply, and the app was blocked from new installs until approved.
- **Since 2026-06-10:** the test is **unique users who can access the app across all its servers**. <10,000 = toggle in the portal, no application. At 10,000 you get a DM/email and portal banner, then **90 days** to apply, otherwise the intents are removed. Granted apps **reapply once a year**. Apps can keep joining servers during review. **App Verification (the 100-server cap, Stripe ID) is now a separate process** from intent review.
- Consequence for BYO: a streamer whose Discord has **10k+ members** and who wants MESSAGE_CONTENT or GUILD_MEMBERS must go through Discord's review (questionnaire, use case, data practices) for their own app. The exact counting semantics ("can access" vs members) are **(unverified)**. Design so that nothing breaks without privileged intents: degrade to slash-command-only and mention/DM content.
- The portal keeps legacy `*_LIMITED` application flags: `GATEWAY_PRESENCE_LIMITED 1<<13`, `GATEWAY_GUILD_MEMBERS_LIMITED 1<<15`, `GATEWAY_MESSAGE_CONTENT_LIMITED 1<<19` (toggled), versus `1<<12`, `1<<14`, `1<<18` (approved/unlimited). The wizard can read `flags` from `GET /applications/@me` to detect whether each toggle is on ([Application flags](https://docs.discord.com/developers/resources/application)).
- **Without MESSAGE_CONTENT** the bot still gets message content for: messages it sends, DMs with it, messages that **mention** it, and the target message of a message context-menu command. All other content fields are empty.
- **Channel obfuscation, mandatory 2026-11-16** ([changelog](https://docs.discord.com/developers/change-log)): channels the bot lacks `VIEW_CHANNEL` on appear as `"___hidden___"` over the Gateway and are omitted from `GET /guilds/{id}/channels`. The wizard's channel picker must be fed from channels the bot can actually view, and must tell the user to grant the bot View Channel on the target channels/categories.
- Age assurance (rolling out Sep 2026): Private Profiles can hide custom status from presence. Another reason not to depend on GUILD_PRESENCES.

### 3.2 Non-privileged intents we want

| Intent (bit) | Events | Used for |
|---|---|---|
| `GUILDS` (0) | guild/channel/role create/update/delete, thread events | channel/role pickers, caches |
| `GUILD_MESSAGES` (9) | `MESSAGE_CREATE/UPDATE/DELETE` (content blank without the privileged intent) | message triggers, boost system messages, mention/DM content |
| `GUILD_MESSAGE_REACTIONS` (10) | reaction add/remove | reaction triggers, reaction-role opt-in |
| `GUILD_VOICE_STATES` (7) | `VOICE_STATE_UPDATE`, `VOICE_CHANNEL_EFFECT_SEND` | join/leave voice triggers, voice roster, songbird |
| `GUILD_SCHEDULED_EVENTS` (16) | event create/update/delete, user add/remove | schedule sync, RSVP triggers |
| `GUILD_MESSAGE_POLLS` (24) | `MESSAGE_POLL_VOTE_ADD/REMOVE` | poll results |
| `GUILD_MODERATION` (2) | audit log entry, ban add/remove | mod-log mirroring (also needs `VIEW_AUDIT_LOG`) |
| `AUTO_MODERATION_EXECUTION` (21) | AutoMod action executed | mod-log mirroring (AutoMod rule data may need `MANAGE_GUILD`, **unverified**) |
| `DIRECT_MESSAGES` (12) | DMs | only if we want DM commands |

Source: [Gateway intents table](https://docs.discord.com/developers/events/gateway). Slash commands, buttons, select menus and modals need **no intent at all**: interactions are delivered regardless.

### 3.3 Permissions and the computed invite URL

Bit positions are from the [Permissions](https://docs.discord.com/developers/topics/permissions) page. Compute the integer from the features the user ticks (least privilege).

| Permission | Bit | Feature |
|---|---|---|
| View Channel | 10 | everything (also required by the 2026-11-16 obfuscation change) |
| Send Messages | 11 | announcements, replies |
| Embed Links | 14 | embeds |
| Attach Files | 15 | clips, images |
| Read Message History | 16 | edit/reconcile, interaction follow-ups in channel |
| Add Reactions | 6 | opt-in reactions |
| Use External Emojis | 18 | custom emotes (optional) |
| Send Messages in Threads | 38 | thread posts |
| Send Polls | 49 | polls |
| Manage Roles | 28 | role sync (bot's top role must sit **above** the roles it grants) |
| Create Events / Manage Events | 44 / 33 | scheduled events |
| Connect / Speak / Use VAD | 20 / 21 / 25 | voice TTS |
| Send TTS Messages | 12 | optional native `/tts` |
| Send Voice Messages | 46 | voice-message clips |
| View Audit Log | 7 | mod-log mirroring |
| Manage Messages | 13 | only if deleting others' messages (avoid by default) |

Presets (precomputed):

| Preset | Features | `permissions=` |
|---|---|---|
| A: Announcer + chat | channel posts, embeds, files, reactions, threads, polls | `563224831708224` |
| B: A + Roles | + role sync | `563225100143680` |
| C: B + Scheduled events | + schedule sync | `580825876122688` |
| D: C + Voice | + TTS in voice | `580825912822848` |

Invite URL template ([OAuth2](https://docs.discord.com/developers/topics/oauth2)):
`https://discord.com/oauth2/authorize?client_id=<APP_ID>&scope=bot%20applications.commands&permissions=<N>&integration_type=0`

Role hierarchy: a bot can only grant roles below its own highest role, so the wizard must tell the owner to drag the bot's role above managed sync roles ([Permissions: hierarchy](https://docs.discord.com/developers/topics/permissions)).

---

## 4. Feature feasibility

### 4.1 Matrix

| # | Feature | Rating | API used | Intents / perms | Notes |
|---|---|---|---|---|---|
| 1 | Go-live embed, auto-edit on end with VOD link | **GREEN** | Webhook: `POST /webhooks/{id}/{token}?wait=true` then `PATCH /webhooks/{id}/{token}/messages/{mid}`; or bot `POST/PATCH /channels/{id}/messages` | none (webhook) / Preset A | Works with no bot. Persist message ID in SQLite. Embed: title 256, description 4096, 6000 total ([Message](https://docs.discord.com/developers/resources/message)). Reconcile on startup. See 4.2a |
| 2a | Twitch sub / YouTube member role sync via Discord's own integrations | **GREEN** (zero code) | Server Settings > Integrations | Twitch Partner/Affiliate; YouTube Partner with memberships | Auto-creates `Twitch Subscriber` and `Tier 1/2/3` roles, grace period, optional kick ([Twitch FAQ](https://support.discord.com/hc/en-us/articles/212112068-Twitch-Integration-FAQ), [YouTube](https://support.discord.com/hc/en-us/articles/215162978-How-to-Connect-Your-YouTube-Channel-to-Discord)). Not for Kick, followers, VIP, mods, non-partners |
| 2b | Custom cross-platform role sync (Kick subs, followers, VIPs, loyalty tiers) | **YELLOW** | `PUT /guilds/{g}/members/{u}/roles/{r}` | Manage Roles, hierarchy; no privileged intent if acting on known IDs | Needs **account linking** (4.2b). Listing all members needs `GUILD_MEMBERS` (privileged), so avoid by acting on linked IDs only |
| 2c | Linked Roles (member self-serve via "Connections") | **RED for MVP / YELLOW with relay** | `PUT /applications/{id}/role-connections/metadata`, `PUT /users/@me/applications/{id}/role-connection` (Bearer, `role_connections.write`) | OAuth2 + portal "Linked Roles Verification URL" | Needs a **publicly reachable HTTPS URL** the member's browser opens, plus a `client_secret` code exchange and stored member tokens. See 4.2c |
| 3 | Discord channel as multichat source | **YELLOW** (GREEN <10k users) | Gateway `MESSAGE_CREATE` | `MESSAGE_CONTENT` (privileged) | Normalized as a `chatSources` contribution; attachments' CDN URLs are signed and expire (**unverified**; Discord signed-URL change, late 2023), so do not cache them in overlays |
| 3b | Send replies from multichat to Discord | **GREEN** | `POST /channels/{id}/messages` with `message_reference`, `allowed_mentions: {parse: []}`; or webhook with `username`/`avatar_url` override per message | Send Messages | Set `allowed_mentions` empty on bridged text so a viewer typing `@everyone` does not ping. Ignore own and webhook messages to prevent loops |
| 4 | Discord events as triggers | **GREEN** / **YELLOW** | See 4.2d | non-privileged: messages, reactions, voice state, scheduled events, polls, AutoMod, bans. Privileged: member join/leave/update | |
| 5 | Slash commands, buttons, selects, modals running actions | **GREEN** | `INTERACTION_CREATE` over Gateway; `PUT /applications/{id}/guilds/{g}/commands` (bulk overwrite) | none | See 4.2e |
| 6 | Post clips/highlights | **GREEN** | Twitch Create Clip, then post link (Discord unfurls it, **unverified**); or upload file with `multipart` | Attach Files | Default upload limit **20 MiB** (raised from 10 MiB on 2026-09-03, [changelog](https://docs.discord.com/developers/change-log)). Local replay-buffer files are usually larger: post links, or compress |
| 7 | Twitch schedule to Discord Scheduled Events | **GREEN** | `POST /guilds/{g}/scheduled-events` with `entity_type: EXTERNAL`, `location`, required `scheduled_end_time`, optional `recurrence_rule`, cover `image` | Create Events (44) / Manage Events (33) | Max **100** `SCHEDULED`/`ACTIVE` events per guild. EXTERNAL events auto-start and auto-end at their times ([Scheduled Events](https://docs.discord.com/developers/resources/guild-scheduled-event)). Twitch side is Helix `GET /schedule` polling (no EventSub for schedule changes known, **unverified**) |
| 8 | Mod-log mirroring | **GREEN** (Twitch to Discord) / **YELLOW** (Discord to Twitch) | Twitch EventSub moderation events to webhook; Discord `GUILD_AUDIT_LOG_ENTRY_CREATE`, `GUILD_BAN_ADD`, AutoMod | `GUILD_MODERATION`, View Audit Log | Mapping Discord bans to Twitch bans needs identity linking and an explicit user decision |
| 9 | Cross-platform loyalty in Discord | **YELLOW** | `/points`, `/leaderboard`, `/link`; role rewards via role API | Preset B | Commands are GREEN. The hard part is identity linking and privacy (profiling rule, section 7) |
| 10 | Polls | **GREEN** | `poll` object on create message: up to 10 answers, duration up to 32 days (default 24 h); votes via `MESSAGE_POLL_VOTE_ADD/REMOVE`; `Get Answer Voters` | Send Polls (49), `GUILD_MESSAGE_POLLS` | A poll message is posted as the bot, so interactive vote bridging to Twitch polls is YELLOW ([Poll](https://docs.discord.com/developers/resources/poll)) |
| 11 | TTS into a voice channel | **YELLOW** | Voice gateway v8 + UDP + Opus + DAVE via songbird / `@discordjs/voice` | `GUILD_VOICE_STATES`, Connect, Speak | See section 5.1. Cheaper options: Discord's native `tts:true` message, or a voice-message attachment |
| 12 | Receiving voice (STT, recording) | **RED** (MVP) | songbird `receive` feature (`VoiceTick`, `RtpPacket`) | Connect | See section 5.2 |
| 13 | Voice-activity overlay | **GREEN** (StreamKit link-out) / **YELLOW** (bot or BYO-RPC) | See section 5.3 | | |
| 14 | "Now streaming" presence | **GREEN** (bot) / **YELLOW** (profile) | Gateway presence activity `type: 1` with a `twitch.tv`/`youtube.com` URL ([docs](https://docs.discord.com/developers/events/gateway-events)) | none | Bot status only. Profile-level Rich Presence needs the Social SDK or local RPC (section 5.4) |
| 15 | Webhook-only mode (no bot) | **GREEN** | Execute / Edit / Delete Webhook Message | none | Section 4.2f |
| 16 | Reaction-role / "Notify me" opt-in | **GREEN** | Button or select interaction to toggle a mentionable role | Manage Roles | Best way to satisfy the opt-in rule for pings (section 7) |

### 4.2 Notes on specific features

**a) Go-live announcement.** Flow: EventSub `stream.online` -> build embed (title, game, viewers, thumbnail) -> post with `?wait=true` to get the message ID -> store `{stream_id, message_id, channel_id}`. On `channel.update` edit title/game. On `stream.offline` edit: grey colour, "was live" wording, duration, and VOD link. Details:
- Twitch VOD link: Helix `GET /videos?user_id=...&type=archive` matched by `stream_id`. The VOD may not be listed the instant `stream.offline` fires, so retry for a short while (timing **unverified**). YouTube: the live video URL is the VOD URL.
- Thumbnail: use the live preview CDN URL with a cache-busting query because Discord proxies and caches embed images (**unverified**). After the stream ends, swap the image for the VOD's thumbnail.
- Role ping: put `<@&ROLE_ID>` in `content` with `allowed_mentions: {roles: [ROLE_ID]}`. A pinged role must be mentionable or the poster must have Mention Everyone/All Roles (bot) (**unverified for webhooks**, test).
- Do not DM members the notification (section 7, rule 5). Use the channel plus an opt-in role.
- Webhooks can also delete their own messages, so a "stream cancelled" cleanup is possible.

**b) Account linking (needed by 2b, 9).** Options, all local-first:
1. **Chat-code link (recommended):** user runs `/link twitch`; the bot replies ephemerally with a one-time code; the user types `!link CODE` in the streamer's Twitch/YouTube/Kick chat. The core sees the code with an authenticated platform user ID. Proves control of both accounts, no OAuth, no public URL.
2. **OAuth with copy-paste:** the user authorizes the streamer's own Discord app with the `connections` scope; `GET /users/@me/connections` returns linked accounts (`twitch`, `youtube`, ...) with IDs ([User resource](https://docs.discord.com/developers/resources/user)). The redirect lands on any static page that just shows the `code`; the user pastes it into `/link code:...` and the local app exchanges it using its own `client_id`/`client_secret`. Verified-ness of the platform account is as reported by Discord (**unverified** for Kick; Discord's Connections list does not clearly include Kick).
3. **Relay-hosted OAuth** (Linked Roles path): see 4.2c.

**c) Linked Roles.** The feature lets a member prove an attribute (for example "Twitch sub for 3+ months") and unlock an admin-configured role. Requirements per [Application Role Connection Metadata](https://docs.discord.com/developers/resources/application-role-connection-metadata) and the [tutorial](https://docs.discord.com/developers/tutorials/configuring-app-metadata-for-linked-roles): up to **5 metadata records** per app (types: integer comparisons, datetime, boolean); a "Linked Roles Verification URL" in the portal that Discord opens in the member's browser; OAuth2 with `role_connections.write`; the app then writes values with the member's access token. The docs do not state "must be public HTTPS", but the member's browser must reach it, so `localhost` cannot work for other people (inferred). This needs the optional relay/tunnel (Cloudflare Tunnel, Tailscale Funnel) from `platform-apis.md` section 11, and it makes the relay a holder of member OAuth tokens (API Data obligations). Value: it covers criteria Discord's native Twitch/YouTube sync does not (watch time, loyalty points, Kick). Defer to a late phase; do not block earlier phases on it. Also note the cheap native alternative: server admins can require "a connected Twitch account" as a Linked Role without any app ([Connections and Linked Roles](https://support.discord.com/hc/en-us/articles/8063233404823-Connections-Linked-Roles-Community-Members)).

**d) Discord events as triggers.**

| Trigger | Gateway event | Intent | Privileged? |
|---|---|---|---|
| Member joined / left / updated | `GUILD_MEMBER_ADD/REMOVE/UPDATE` | `GUILD_MEMBERS` | **Yes** (toggle <10k) |
| Member joined (fallback) | `MESSAGE_CREATE` type `USER_JOIN` in the system channel | `GUILD_MESSAGES` | No (only if system join messages are on) |
| Message posted | `MESSAGE_CREATE` | `GUILD_MESSAGES` (+ `MESSAGE_CONTENT` for text) | Content only |
| Reaction added/removed | `MESSAGE_REACTION_ADD/REMOVE` | `GUILD_MESSAGE_REACTIONS` | No |
| Voice join / leave / move / mute | `VOICE_STATE_UPDATE` | `GUILD_VOICE_STATES` | No |
| Boost | `MESSAGE_CREATE` types `GUILD_BOOST`, `GUILD_BOOST_TIER_1/2/3`; or `GUILD_MEMBER_UPDATE.premium_since` | `GUILD_MESSAGES` / `GUILD_MEMBERS` | Message route: No. System boost messages can be suppressed in server settings (**unverified**) |
| Scheduled event created / RSVP | `GUILD_SCHEDULED_EVENT_*`, `_USER_ADD/REMOVE` | `GUILD_SCHEDULED_EVENTS` | No |
| Poll vote | `MESSAGE_POLL_VOTE_ADD/REMOVE` | `GUILD_MESSAGE_POLLS` | No |
| Ban / audit entry / AutoMod hit | `GUILD_BAN_ADD`, `GUILD_AUDIT_LOG_ENTRY_CREATE`, `AUTO_MODERATION_ACTION_EXECUTION` | `GUILD_MODERATION`, `AUTO_MODERATION_EXECUTION` | No |
| Slash command / button / modal | `INTERACTION_CREATE` | none | No |

Presence updates (`GUILD_PRESENCES`) are privileged and unreliable (Private Profiles); do not use them.

**e) Interactions that run Streamer.live actions.**
- Commands should be **user-defined at runtime** (the user maps `/clip`, `/shoutout`, `/scene`, `/tts` to action groups in the UI). Register with **bulk overwrite per guild**; guild commands update immediately, global commands are meant for public apps. Limits: **100 global** commands, **200 command creates per day per guild**, up to 25 options per command, 25 choices per option ([Application Commands](https://docs.discord.com/developers/interactions/application-commands)). Diff against the last registration and skip no-op overwrites.
- Authorization: use `default_member_permissions` plus per-command permissions for up to 100 roles/users/channels, and a Streamer.live-side check that maps Discord roles/user IDs to permission groups (mods, VIPs, owner). Destructive actions go through an ephemeral confirm button.
- Responses: ephemeral for private output; `DEFERRED_CHANNEL_MESSAGE_WITH_SOURCE` if the action may take over 3 s. Select menus feed pickers (scene, sound). Modals collect text (title change, TTS text).
- Interaction callbacks are **not counted** toward the 50 req/s global limit ([Rate limits](https://docs.discord.com/developers/topics/rate-limits)).
- Autocomplete can serve live data (current scenes, last 25 clips).

**f) Webhook-only mode (no bot, no token).** The user pastes a channel webhook URL. `GET <url>` (no auth) returns the webhook object, so the app can validate it and show the guild/channel. Capabilities per [Webhook resource](https://docs.discord.com/developers/resources/webhook):

| Can do | Cannot do |
|---|---|
| Post text (2000 chars) and up to 10 embeds (6000 chars total); override `username`/`avatar_url`; post into a thread (`thread_id`); `?wait=true` to get the message; edit and delete **its own** messages; files | Read anything; receive events; slash commands; add roles; scheduled events; polls (unverified); reactions; interactive components (non-application-owned webhooks may only send non-interactive components such as link buttons, via `with_components`) |

The URL is a bearer secret: store it in the keychain/secret store, never in exported configs. A 404 means the webhook was deleted: stop using it (repeated 404s risk a temporary IP restriction).

---

## 5. Voice

### 5.1 TTS into a voice channel (YELLOW)

Protocol per [Voice Connections](https://docs.discord.com/developers/topics/voice-connections):
1. Gateway op 4 (Voice State Update) to join; wait for both `VOICE_STATE_UPDATE` and `VOICE_SERVER_UPDATE`.
2. Voice WebSocket (use **voice gateway v8**; versions below 4 are rejected), Identify, Ready (SSRC, UDP address, modes), heartbeat with `seq_ack`.
3. UDP IP discovery, Select Protocol, Session Description (mode + secret key). Modes: `aead_aes256_gcm_rtpsize` (preferred) and `aead_xchacha20_poly1305_rtpsize` (required); the older `xsalsa20_*` modes were discontinued on 2024-11-18.
4. Opus, **stereo, 48 kHz**, RTP over UDP, with at least one Speaking (op 5) before sending audio (bots set `delay` 0).
5. **DAVE** (MLS-based E2EE) over voice opcodes 21-31. Discord: "we will only support E2EE calls starting on **March 1st, 2026**"; clients send `max_dave_protocol_version` in Identify; a client with no DAVE gets close code **4017** ("E2EE/DAVE protocol required"). Discord's blog (2026-05-18) confirms every voice and video call is E2EE since early March 2026, with **stage channels** the one exception, DAVE extended to bots/apps in 2025, and fallback code being removed ([blog](https://discord.com/blog/every-voice-and-video-call-on-discord-is-now-end-to-end-encrypted), [libdave](https://github.com/discord/libdave) MIT, C++/JS, [protocol whitepaper](https://github.com/discord/dave-protocol)).

Library status (see section 6): **songbird 0.6.0 (2026-04-05) added DAVE** via the optional Rust `davey` crate (OpenMLS-based, [Snazzah/davey](https://github.com/Snazzah/davey), 0.1.5 on 2026-10-12) and switched to `opus2`. `@discordjs/voice` 0.19.2 depends on `@snazzah/davey`.

Risks:
- songbird [issue #310](https://github.com/serenity-rs/songbird/issues/310) (open, 2026-07-16, with songbird 0.6.0 and davey 0.1.4): silent per-join failures; a receive stall after the bot's own MLS transition, a send stall before the session is ready (other clients discard its audio), and per-member `Crypto(Error)` after an Append commit. Reporter workarounds: auto-rejoin when no audio decodes, or announce `max_dave_protocol_version: None` (which falls back to transport-only encryption; given the 4017 rule this likely works only where DAVE is not enforced, **unverified**). No maintainer response at the time of the check. Treat voice as "needs a watchdog and rejoin loop".
- Build: songbird needs libopus (`opus2` uses system libopus via pkg-config, otherwise builds from source and needs cmake on Windows and a C toolchain/autotools on Unix) ([songbird README](https://github.com/serenity-rs/songbird)). This is a packaging cost for all three OSes.
- Only one voice connection per guild per bot; the bot is a visible participant in the channel.

TTS audio path: the core already has a TTS crate; hand PCM/WAV to songbird (`Input` from raw PCM) in a Rust implementation, or pass a file path/local socket URL to a Node sidecar (never push audio frames through JSON-RPC stdio).

Cheaper alternatives, all without the voice stack:

| Option | How | Limits |
|---|---|---|
| Native `tts: true` message | `POST /channels/{id}/messages` with `tts: true`; needs **Send TTS Messages (bit 12)** | Played by Discord clients that enabled TTS, with Discord's voice and queue; not audible to someone without the setting |
| Voice message | Single `audio/ogg` Opus attachment with `duration_secs`, `waveform`, flag `IS_VOICE_MESSAGE`; needs **Send Voice Messages (bit 46)** | Async, cannot be edited, encoding "may change without warning" ([Message docs](https://docs.discord.com/developers/resources/message)) |
| Virtual audio device | Route TTS to a virtual cable that the streamer's own Discord client uses as its mic | Platform-specific, no bot needed, audio comes from the streamer's identity |

### 5.2 Receiving voice (RED for MVP)

- Technically supported: songbird's `receive` feature exposes `VoiceTick` (decoded 20 ms audio per user), `RtpPacket`, `RtcpPacket` ([CoreEvent](https://docs.rs/songbird/0.6.1/songbird/events/enum.CoreEvent.html)); `@discordjs/voice` has a receiver. With DAVE the bot, as an MLS group member, can decrypt other members' frames. Songbird PR/issue [#314](https://github.com/serenity-rs/songbird/issues/314) (closed 2026-10-03) fixed a DAVE RTP extension offset bug in receive, which shows this path is still being repaired.
- Allowed? I found **no explicit Discord policy** text for or against bots recording or transcribing voice (**unverified**; the [Developer Policy](https://support-dev.discord.com/hc/en-us/articles/8563934450327-Discord-Developer-Policy) has no voice section). The relevant rules are general: use API Data only for the stated, user-permitted functionality (rule 15), do not profile users (16), do not enable circumvention of privacy features (3), plus recording-consent laws that vary by jurisdiction. Voice content is not "message content", so the AI-training ban in rule 21 does not literally cover it, but feeding it to a cloud model is still sharing API Data with a third party (section 7).
- Recommendation: no recording/STT in MVP. If ever added: opt-in per session, a visible announcement in the channel, no persistence by default, local STT only.

### 5.3 Voice-activity overlay

| Option | Approval | Works headless | Notes | Rating |
|---|---|---|---|---|
| **A. Discord StreamKit overlay** as an OBS browser source | none (Discord's own app) | Needs the user's own Discord client + one-time authorization in it | Help article ([Using Discord's OBS Streamkit Overlay](https://support.discord.com/hc/en-us/articles/223415707-Using-Discord-s-OBS-Streamkit-Overlay)) last updated 2024-06-26 and still lists Status/Chat/Voice widgets. `https://streamkit.discord.com/overlay` still returns 200, but the page's `Last-Modified` is 2023-10-10, so it looks **maintained-by-neglect**. Our designer can host/preview it as a browser-source widget and link out; no code of ours touches Discord | **GREEN** (guide + widget preset) |
| **B. BYO app + local RPC over IPC** (`discord-ipc-N`) | The RPC docs say unapproved apps work for users on the app's **tester list (up to 50)**, limited to 10 guilds/channels ([RPC](https://docs.discord.com/developers/topics/rpc)). But the [OAuth2 scope list](https://docs.discord.com/developers/topics/oauth2) labels `rpc`, `rpc.voice.read`, `rpc.voice.write`, `rpc.activities.write`, `voice` as "only available to approved partners" | No: the Discord desktop client must run on the same machine; blue "app connected" banner; Flatpak/Snap sockets need path handling | Events: `SPEAKING_START/STOP`, `VOICE_STATE_CREATE/UPDATE/DELETE` with `channel_id` subscriptions. Flow: `AUTHORIZE` -> code -> token endpoint (needs the app's own secret, fine for BYO) -> `AUTHENTICATE`. Whether `rpc.voice.read` really works for an owner/tester without approval is **unverified**; needs a spike. IPC docs were published 2026-04-03 (changelog); WebSocket transport is deprecated | **YELLOW** |
| **C. Bot in the voice channel** (voice gateway) | none | Yes (daemon on a server) | The voice server sends **Speaking (op 5)** to participants: opcode table lists it as "client and server"; songbird's `CoreEvent::SpeakingStateUpdate` fires for other users, "needed to match an SSRC to a user ID", and does **not** require the `receive` feature. `VOICE_STATE_UPDATE` carries the member (name/avatar) so no `GUILD_MEMBERS` intent. No audio is touched. Costs: bot visibly sits in the channel, DAVE handshake, one channel per guild | **YELLOW** |
| **D. Reuse Discord's StreamKit client ID and token endpoint** (what some third-party tools do) | none, but borrows Discord's first-party identity (`streamkit.discord.com/overlay/token`) | No | Seen in a Noctalia plugin (v0.3.2, 2026-08): works today, "depends on Discord's public StreamKit identity and token broker, which Discord may change", no refresh token. Likely conflicts with the "do not impersonate Discord services" rule 12 (my reading) | **RED** (do not ship) |

Which third-party app can get these scopes? I found **no public application route**: the scopes are described as "approved partners" only. Plan: ship A now; try B as an experimental flag with the BYO owner as tester; offer C as part of the voice module.

### 5.4 Social SDK and Rich Presence (brief)

- The [Discord Social SDK](https://docs.discord.com/developers/discord-social-sdk/overview) is framed for **games** (account linking, friends, invites, voice, "Rich Presence & User Status"); text/voice features are rate-limited until the integration passes Discord's review. Docs cover game engines (C++/Unity/Unreal; a C# marshalling fix appears in the changelog); no Rust bindings found (**unverified**). Rich Presence shows on the user's profile ([Rich Presence](https://docs.discord.com/developers/platform/rich-presence)).
- Sep 2026 added read-only RPC access to desktop voice settings in the Social SDK (mute/deafen/volumes/PTT key) with no scope or approval details in the entry.
- Verdict: **not worth it for "now streaming".** Use (1) bot presence activity `type 1` (Streaming) with a Twitch/YouTube URL, which "currently only supports Twitch and YouTube"; (2) Discord's own Twitch/YouTube connections for profile display (behaviour **unverified**); (3) optional experiment later: IPC `SET_ACTIVITY` (the `rpc.activities.write` scope is approved-partner-only; unauthenticated legacy use is **unverified**).
- Activities (Embedded App SDK) run in an iframe inside Discord and expose `SPEAKING_START/STOP` (needs `rpc.voice.read`) but are not usable as an OBS overlay source (no evidence). Out of scope.

---

## 6. Libraries

Versions fetched from crates.io / npm / GitHub on 2026-10-18.

| Library | Latest | Released | Activity | DAVE / voice | Notes |
|---|---|---|---|---|---|
| **serenity** (Rust) | 0.12.5 | 2025-12-20 | Repo pushed 2026-10-13; release notes call 0.12.5 "the last release for the 0.12.x series" unless another is warranted; breaking next version lives on the `next` branch (not on crates.io) | none itself; pair with songbird | Batteries-included: cache, builders, `EventHandler`. Maintainers' note in 0.12.5 honours a late contributor |
| **poise** (Rust framework) | 0.7.1 | 2026-10-12 (0.7.0 on 2026-09-06, 0.6.2 on 2026-04-13) | Active | n/a | Depends on `serenity ^0.12.5`. Derive-macro, **compile-time** command definitions plus a framework that owns the run loop: poor fit for user-defined runtime commands. 0.7.0 had notable API breaks |
| **twilight** (Rust, modular) | 0.17.1 (`-gateway`, `-http`, `-model`) | 2025-12-13 | Repo pushed 2026-10-18; MSRV 1.89 | No built-in voice. Types for voice WS in `twilight-model`; `songbird` has a `twilight` feature (supports twilight `>=0.16, <=0.17`) | Opt-in `twilight-cache-inmemory`; small, embeddable, no framework. Good for dynamic command tables |
| **songbird** (Rust voice) | **0.6.1** | 2026-10-10 (docs.rs fix); **0.6.0 on 2026-04-05** | Active | **DAVE since 0.6.0** (optional `davey ^0.1.2`), receive via `receive` feature, `opus2` replaces unmaintained `audiopus`, MSRV 1.83, works with serenity or twilight | Open reliability issue #310; libopus build requirements |
| **davey** (Rust DAVE) | 0.1.5 | 2026-10-12 | Frequent 0.1.x releases (Dec 2025 onward) | OpenMLS-based; Rust crate plus Node (`@snazzah/davey` 0.1.12) and Python bindings; MIT | Pre-1.0 |
| **discord.js** (TS) | 14.27.0 | 2026-07-15 | Repo pushed 2026-10-16 | n/a | Node >= 18; the most complete and fastest to adopt new API features |
| **@discordjs/voice** | 0.19.2 | 2026-03-17 | npm modified 2026-10-14 | DAVE via `@snazzah/davey ^0.1.9`; requires **Node >= 22.12** | Receive supported via its receiver API |
| **@discordjs/core** | 2.6.0 | 2026-07-15 | Active | n/a | Lower-level, no cache; alternative if we want a thin TS layer |

Choice:
- **Embedded in Rust core:** `twilight-gateway` + `twilight-http` + `twilight-model` (+ `songbird` with `twilight` feature behind a `discord-voice` cargo feature). Reasons: runtime-defined commands, no framework owning the loop, smaller compile surface than serenity+poise.
- **As a sidecar plugin (recommended):** **TypeScript + discord.js 14.x**, with `@discordjs/voice` behind an optional voice module. Reasons: fastest to follow Discord's frequent API changes (the changelog shows breaking changes every month), dogfoods the official TS SDK, process isolation for the MLS/Opus stack, and the host already provisions `node` for TS sidecars. Cost: Node runtime (>= 22.12 if voice) must exist on the daemon host, and audio hand-off is by file path.
- Fallback if the owner insists on one static binary: a **Rust sidecar** on twilight + songbird, speaking the same JSON-RPC (types from the shared protocol crate). Do not use poise for this.

---

## 7. Policy, ToS and limits

Primary texts: [Developer Policy](https://support-dev.discord.com/hc/en-us/articles/8563934450327-Discord-Developer-Policy) (effective 2024-07-08, current on 2026-10-18), [Developer Terms of Service](https://support-dev.discord.com/hc/en-us/articles/8562894815383-Discord-Developer-Terms-of-Service) (same date), [Community Guidelines](https://discord.com/guidelines) (updated 2025-08-29).

| Topic | Rule | Consequence for us |
|---|---|---|
| **Self-bots** | "Automating normal user accounts ('self-bots') outside of the OAuth2/bot API is forbidden" and can get the account terminated ([support article](https://support.discord.com/hc/en-us/articles/115002192352-Automated-User-Accounts-Self-Bots)); Community Guidelines rule 14: no self-bots or user-bots; Policy rule 4: never collect user passwords or login tokens | Only bot tokens. Never ask for a user token, never drive the Discord client UI, never log in as the streamer's account. The RPC route (B in 5.3) talks to the official client's documented local API, which is not a self-bot, but stay within documented commands |
| **Data use** | Rule 15: use API Data only as necessary for the stated functionality; aggregated/de-identified use for improving the app is allowed. Rule 16: do not use API Data to **profile** users or relationships. Rule 20: no mining/scraping | Store only IDs and what the feature needs. Disclose loyalty tracking in the BYO privacy text and allow `/unlink` and `/forget` |
| **Retention / deletion** | Developer Terms 5(b): delete API Data when it is no longer needed for the stated functionality, when you stop operating the app, when Discord requests it, **when the user requests it**, or as law requires; give users an easy way to ask for modification/deletion. 5(c): encrypt API Data at rest and keep safeguards. Section "Termination": delete cached/stored API Data on termination | Keep message bodies in a short ring buffer or not at all; encrypt the SQLite store (or only store IDs); a "forget me" command; wipe on bot removal/token reset |
| **Privacy policy** | You "will provide and adhere to a privacy policy" and keep up-to-date links in the Developer Portal | BYO makes the streamer the developer. Ship a **template privacy policy** the wizard can fill and host/copy so streamers can paste a URL in the portal |
| **Sharing with third parties** | Terms 5(b): API Data may not be shared with third parties except Service Providers (bound in writing to use it only for you), as required by law, or when a user expressly directs it. Rule 17: no data brokers/ad networks | Sending Discord message content to a **cloud LLM or STT** counts as sharing API Data. Use local models by default, or treat the vendor as a Service Provider with a written no-training agreement. Disclose it (my reading; not legal advice) |
| **AI-generated messages** | No Discord rule found that bans or requires labelling AI bot messages. Bot accounts carry the app badge. **Rule 21:** do not use message content obtained through the APIs to train ML/AI models (including LLMs) unless Discord permits. Rule 13: do not automate messages to fake server activity. App Directory content rules require 13+ appropriateness | Allowed: AI-written announcements and replies as the clearly-marked bot. Not allowed: fine-tuning or building datasets from Discord messages; "ambient chatter" to keep a server looking active. Add a "Discord content never trains" invariant in the plugin docs |
| **Contacting users** | Rule 5: no contacting users on Discord without explicit permission (frequent unsolicited DMs); rule 6: no advertising; rule 7: no contacting outside Discord using API Data; rule 2: no initiating actions on behalf of a server/user without permission | Announcements go to a channel with an **opt-in role** (button), not DMs. Role changes follow explicit linking |
| **Modifying accounts** | Rule 1: no modifying a user's account (joining servers etc.) without explicit permission | OAuth `guilds.join` is not used |
| **Respect opt-outs** | Rule 3: do not circumvent privacy/safety features; respect blocks and removal | Bot leaves and wipes on removal; honour user blocks |
| **Monetization** | Paid features offered by an app must be purchasable via Discord Premium Apps (where supported) at no higher price | Not relevant to a free local BYO bot; relevant if a hosted paid bot is ever offered |
| **Rate limits** | Do not hard-code. Parse `X-RateLimit-*`. 50 req/s global per bot; per-route buckets keyed on channel/guild/webhook IDs; 429 `Retry-After`; interaction callbacks excluded from the global limit; invalid-request limit 10,000/10 min per IP; Gateway 120 events/60 s; IDENTIFY 1,000/24 h; Developer Policy says exceeding limits needs Discord's written consent | One rate-limit-aware HTTP client per token with a per-bucket queue; a priority lane for interaction follow-ups; coalesce announcement edits |

---

## 8. Recommendation

### 8.1 Hosting model

**BYO bot, Gateway from the user's own machine or `streamer-lived`.** Webhook-only mode needs no bot at all. No hosted infrastructure for v1. The only conceivable hosted pieces (all optional, in the existing relay design) are: Linked Roles verification page, Interactions-endpoint offline responder, and a static page that displays an OAuth `code` for copy-paste linking. The daemon is the preferred host for the bot because it is the one component that is always on.

### 8.2 Phases

| Phase | Scope | Rating mix | Needs |
|---|---|---|---|
| **P0: Webhook mode** (core, tiny) | Paste webhook URL; templated embeds for go-live, offline (edit with VOD), clip posted, mod-log lines, schedule posts; per-event channel mapping | GREEN | Nothing from Discord |
| **P1: Bot MVP** (plugin) | Token + wizard; Gateway; presence "Streaming"; go-live via bot with opt-in "Notify me" role button; slash commands/buttons/selects/modals mapped to action groups; triggers from non-privileged events (messages without content, reactions, voice state, scheduled events, polls, interactions); polls; clip command; Scheduled Events sync; mod-log mirror | GREEN | Preset A or C; no privileged intents |
| **P2: Chat and roles** | Discord channel as a multichat source (MESSAGE_CONTENT); replies from multichat; member join/leave/boost triggers (GUILD_MEMBERS); `/link` flow (chat-code); custom role sync and loyalty commands; reaction roles | GREEN/YELLOW | Privileged intents toggled (<10k users); Preset B |
| **P3: Voice (experimental flag)** | TTS in a voice channel; speaking/roster widget from the bot-in-channel path; StreamKit guide and widget preset (this part can ship in P1) | YELLOW | Preset D; `songbird`/`@discordjs/voice` + DAVE; watchdog and auto-rejoin |
| **P4: Relay-dependent extras** | Linked Roles; Interactions endpoint offline responder; BYO-RPC voice overlay spike | YELLOW/RED | Optional relay or tunnel; owner decision |
| **Not planned** | Receiving/recording voice, Social SDK integration, StreamKit-identity piggyback, any user-account automation | RED | |

### 8.3 External plugin vs core

**Build the bot as a first-party external sidecar plugin** (`kind: "sidecar"`, TypeScript). It matches the "at least two first-party integrations ship as external plugins" rule in `plugin-sdk-and-api.md` and has good fit:
- Independent lifecycle and credentials (`secrets:own`, token via `host/secrets.get`), crash isolation from the DAVE/MLS and Opus stack, restart without touching stream-critical code.
- Heavy dependency tree (libopus, MLS) and fast-moving API stay out of the core binary and its three-OS CI.
- Everything it needs is a normal contribution: triggers (`discord.message`, `discord.reaction`, `discord.voice.join`, `discord.interaction`, ...), actions (`discord.post`, `discord.editMessage`, `discord.addRole`, `discord.createEvent`, `discord.poll`, `discord.tts`), a `chatSources` entry, settings, and a status panel. Permissions: `net:discord.com`, `net:gateway.discord.gg`, `net:*.discord.media` (voice, **unverified exact hosts**), `secrets:own`, `chat:ingest`, `actions:run`.
- It runs in `streamer-lived` the same way as in the desktop app.

Keep in **core**: the plain webhook poster (an HTTPS POST that every user may want, and it must work if no plugin runtime is installed) and the generic "Discord link" data model used by loyalty. If the sidecar proves too heavy for the daemon hosts, the twilight+songbird Rust sidecar is the swap with the same manifest.

### 8.4 Setup wizard

1. **Pick mode:** "Just post announcements (webhook)" or "Full bot".
2. **Webhook path:** show 3 screenshots (Edit Channel > Integrations > Webhooks > New Webhook > Copy URL); paste; app calls `GET <url>`; display guild/channel names; send a test embed; store URL in the secret store.
3. **Bot path, create app:** deep-link to `https://discord.com/developers/applications`; user clicks New Application and names it.
4. **Bot tab:** Reset Token and copy; paste into Streamer.live (masked). Validate with `GET /users/@me` and `GET /applications/@me` (bot token auth). Show bot name and avatar.
5. **Check intents:** read `flags` from `/applications/@me`. For each requested feature, show what is needed (for example "Discord chat in multichat -> Message Content Intent") with a checkbox deep-link to the Bot tab, then re-verify. Warn if `interactions_endpoint_url` is set; offer to clear it. Warn about the 10k-user rule only when the target guild is large.
6. **Choose features** (checkboxes) and **compute permissions** from section 3.3. Build the invite URL with `scope=bot%20applications.commands`, the computed `permissions`, `integration_type=0`; open it in the browser.
7. **Pick the guild:** `GET /users/@me/guilds` lists where the bot is installed (poll until it appears). Connect the Gateway.
8. **Map channels and roles:** channel pickers fed only by channels the bot can view (required by the 2026-11-16 change); a button "Create a mentionable `Live notifications` role" and a "Notify me" button message; a role picker for the sync targets with a **hierarchy check** (the bot's top role must be above them), and a "drag the bot role up" hint if not.
9. **Register commands** (guild scope), send a test announcement, run `/ping`. Show "bot online/offline" and the last reconciliation result in the status panel.
10. **Privacy step:** generate the privacy policy text (what is stored, deletion command) and a link to paste into the portal; show a short note on the AI/training and third-party sharing rules.

---

## 9. Open questions for the owner

1. **Hosting:** confirm BYO-only for v1 (no hosted/official bot). Is a future paid hosted bot even on the table?
2. **Runtime:** is requiring Node (>= 22.12 for voice) on the daemon host acceptable, or must the Discord plugin ship as a single Rust binary (twilight + songbird)?
3. **Voice:** is TTS-in-voice worth the DAVE and libopus cost for v1, or is a native `tts:true` / voice-message fallback enough until songbird issue #310 settles?
4. **Community size:** is the owner's own Discord (or the typical target) above 10,000 members? That decides whether privileged-intent review is a real constraint.
5. **Account linking:** is the chat-code `/link` flow acceptable as the default, with Linked Roles only if a relay exists?
6. **Voice overlay:** accept "link to StreamKit + optional bot-in-channel roster" now, with the BYO-RPC tester route as an experiment?
7. **AI in Discord:** local models only for anything reading Discord content, or allow cloud models with a Service-Provider note and explicit user opt-in?
8. **Daemon ownership:** should the daemon always be the exclusive Discord host when present (lock), with the desktop app only attaching?
9. **Compliance text:** should Streamer.live ship and maintain the template privacy policy for BYO streamers?

---

## 10. Sources

- Gateway and intents: https://docs.discord.com/developers/events/gateway , https://docs.discord.com/developers/events/gateway-events
- Interactions: https://docs.discord.com/developers/interactions/receiving-and-responding , https://docs.discord.com/developers/interactions/application-commands
- Privileged intents (2026-06): https://support-dev.discord.com/hc/en-us/articles/6207308062871-What-are-Privileged-Intents , https://support-dev.discord.com/hc/en-us/articles/40281523410967-Changes-to-Privileged-Intent-Access-for-Discord-Apps , https://docs.discord.com/developers/gateway/getting-started-with-privileged-intent-review
- App verification: https://support-dev.discord.com/hc/en-us/articles/23926564536471-How-Do-I-Get-My-App-Verified
- Application, flags: https://docs.discord.com/developers/resources/application
- Permissions, OAuth2: https://docs.discord.com/developers/topics/permissions , https://docs.discord.com/developers/topics/oauth2
- Rate limits: https://docs.discord.com/developers/topics/rate-limits
- Resources: https://docs.discord.com/developers/resources/message , https://docs.discord.com/developers/resources/webhook , https://docs.discord.com/developers/resources/guild-scheduled-event , https://docs.discord.com/developers/resources/poll , https://docs.discord.com/developers/resources/user , https://docs.discord.com/developers/resources/application-role-connection-metadata
- Linked Roles tutorial: https://docs.discord.com/developers/tutorials/configuring-app-metadata-for-linked-roles ; community FAQ: https://support.discord.com/hc/en-us/articles/8063233404823-Connections-Linked-Roles-Community-Members
- Native integrations: https://support.discord.com/hc/en-us/articles/212112068-Twitch-Integration-FAQ , https://support.discord.com/hc/en-us/articles/215162978-How-to-Connect-Your-YouTube-Channel-to-Discord
- Voice and DAVE: https://docs.discord.com/developers/topics/voice-connections , https://docs.discord.com/developers/topics/opcodes-and-status-codes , https://discord.com/blog/every-voice-and-video-call-on-discord-is-now-end-to-end-encrypted , https://github.com/discord/libdave , https://github.com/discord/dave-protocol
- RPC / StreamKit / Social SDK: https://docs.discord.com/developers/topics/rpc , https://support.discord.com/hc/en-us/articles/223415707-Using-Discord-s-OBS-Streamkit-Overlay , https://streamkit.discord.com/overlay , https://noctalia.dev/plugins/community/discord-voice , https://docs.discord.com/developers/discord-social-sdk/overview , https://docs.discord.com/developers/platform/rich-presence , https://docs.discord.com/developers/developer-tools/embedded-app-sdk
- Change log (2026): https://docs.discord.com/developers/change-log
- Policy: https://support-dev.discord.com/hc/en-us/articles/8563934450327-Discord-Developer-Policy , https://support-dev.discord.com/hc/en-us/articles/8562894815383-Discord-Developer-Terms-of-Service , https://support.discord.com/hc/en-us/articles/115002192352-Automated-User-Accounts-Self-Bots , https://discord.com/guidelines
- Libraries: https://crates.io/crates/serenity , https://crates.io/crates/poise , https://crates.io/crates/twilight-gateway , https://crates.io/crates/songbird , https://crates.io/crates/davey , https://github.com/serenity-rs/songbird (releases, issues #310 and #314) , https://github.com/serenity-rs/serenity , https://github.com/serenity-rs/poise , https://github.com/twilight-rs/twilight , https://github.com/Snazzah/davey , https://www.npmjs.com/package/discord.js , https://www.npmjs.com/package/@discordjs/voice
