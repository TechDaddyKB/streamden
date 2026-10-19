# StreamDen Feature-Parity Matrix

Generated 2026-10-18 from the seven research files in `docs/research/` (SB triggers/integrations, SB engine/UI, Speaker.bot + SAMMI, competitors + designer, app integrations + VTuber, platform APIs, plugin SDK + API). Its purpose is to be the implementation checklist for **StreamDen**, the cross-platform (Windows/Linux/macOS) app that unifies Streamer.bot, Speaker.bot and SAMMI and adds a combined multichat (OBS dock + overlay), a WYSIWYG designer for overlays/alerts, VTuber/app integrations, JS + Python code execution and an API-first plugin SDK. **All priorities and phases below are proposed, pending owner review**; open decisions are tracked in [QUESTIONS.md](QUESTIONS.md). Nothing here is invented beyond the research: anything inferred is marked "(inferred)".

## Legend

| Column | Meaning |
|---|---|
| **ID** | Stable row id (`section.row`) so agents and issues can reference a row. |
| **SB / SpB / SAM** | Presence in Streamer.bot (v1.0.7 stable + 1.1.0 alpha) / Speaker.bot (v0.1.6) / SAMMI (2026.6.0): `✓` native, `◐` partial (incl. SAMMI community extension "ext", or reachable only via C#/another app), `—` absent. |
| **Others** | Competitors/prior art that have it: **FB** Firebot, **LS** Lumia Stream, **MIU** Mix It Up, **SSN** Social Stream Ninja, **SE** StreamElements, **SL** Streamlabs, **TP** Touch Portal, **AIT** Aitum, **KC** Kruiz Control, **CT** Chatterino, **RS** Restream Chat, **1UP** 1UP Vision, **OWN** OWN3D, **OE** Overlay Expert, **NB/FOS** Nightbot/Fossabot, **HA** Home Assistant, **OpenSpk** OpenSpeaker/Speekaboo, **SD** Stream Deck, **VSC** VS Code model. |
| **Pri** | `P0` = MVP v0.1; `P1` = needed for v1.0 parity; `P2` = post-1.0; `P3` = maybe/long-tail; `✗` = won't do (reason in Notes). `★` after the priority marks an **owner must-have** (never below P1). |
| **Phase** | Proposed delivery slot (below). For P2/P3 rows it is the phase where the row would naturally land if pulled in. `—` for `✗`. |
| **Notes** | Terse: cross-platform caveats, API constraints, relay needed, source conflicts. |

| Phase | Scope |
|---|---|
| `P0-Foundations` | Repo/CI for Win+Linux+macOS, storage, secrets, event bus, normalized model, protocol/codegen pipeline, contribution registry skeleton, API server skeleton. |
| `P1-Core` | Action engine, Twitch, OBS, local API, JS scripting, action editor UI. |
| `P2-Chat+TTS` | Unified chat, YouTube, Kick, chat window/dock/overlay, filters, TTS, **self-hostable relay** (Kick events/OAuth, webhook-only vendors). |
| `P3-Designer` | WYSIWYG designer, alerts, widgets. |
| `P4-Integrations+SDK` | VTuber/app integrations, public plugin SDK, Python scripting, Stream Deck, MIDI, hotkeys, **MCP server + AI features (§24)**. |
| `P5-Decks+Voice` | SAMMI-style decks, phone deck, STT/voice commands, points/loyalty. |
| `P6-Ecosystem` | Plugin registry, Streamer.bot import, optional **hosted** relay, WASM plugins. |

Phase changes from owner input after the initial brief: MCP moved from P6 to P4; the self-hostable relay moved from P6 to P2; only an optional hosted relay stays in P6.

Conventions: v1.0 = all P0 and P1 rows complete (phases run through P6). Where a row's `Pri` is P0 its Phase is `P0-Foundations` or `P1-Core`. Defaults assumed by the research (not yet decided, see QUESTIONS.md): Rust core + Tauri 2 + TS UI (plugin-SDK doc), Twitch via EventSub WS + Device Code Flow, local-first with an optional self-hostable relay. Template-syntax, scripting-runtime and relay-hosting choices are open.

---

## 1. Platforms

| ID | Feature | SB | SpB | SAM | Others | Pri | Phase | Notes |
|---|---|---|---|---|---|---|---|---|
| 1.01 | Normalized cross-platform event/user/chat-message model (platform ids as dedupe keys, `amount.usd`, badges, emotes, meta) | ◐ | — | — | SSN, SE | P0 | P0-Foundations | SB duplicates `user*` per platform; SSN schema is prior art. Dedupe: Twitch ids, Kick ULID, Throne `event_id`. EventSub WS has no replay after reconnect. |
| 1.02 | Per-platform accounts: broadcaster (required) + optional bot, multiple channels | ✓ | ◐ | ✓ | FB | P0 | P1-Core | Twitch bot = chat/whispers only; YouTube brand-account selection after owner login; Kick bot posts to own channel. |
| 1.03 | Twitch: OAuth via Public client + Device Code Flow (no secret) for broadcaster and bot | ✓ | ✓ | ✓ | Twurple | P0 | P1-Core | Refresh tokens single-use, 30 d idle expiry: persist atomically; call `/validate` at start + hourly; PKCE unsupported by Twitch; one permanent client ID (custom rewards bound to it). SAM must re-link every 60 d. |
| 1.04 | Twitch: EventSub WebSocket transport (no IRC, no PubSub), reconnect/keepalive handling | ✓ | ✓ | ◐ | Twurple | P0 | P1-Core | 3 WS/user, 300 subs/WS, 10 s to subscribe after Welcome, honor `session_reconnect`. SAM still uses IRC for chat. |
| 1.05 | Twitch: chat read (fragments, emotes, cheermotes, replies, first-message flag, GIF fragments) | ✓ | ✓ | ✓ | CT, SSN | P0 | P1-Core | `user:read:chat`; EventSub lacks first-message flag in SB (tracked locally). |
| 1.06 | Twitch: chat send (broadcaster/bot with fallback, reply, announcement, send-and-pin) | ✓ | ◐ | ✓ | NB, FOS | P0 | P1-Core | Helix `POST /chat/messages`; 500 chars; 20 msg/30 s regular, 100 mod/VIP; separate rate bucket; HTTP 200 can still carry `is_sent: false` + `drop_reason`. SpB only replies to `!tts` commands. Send matrix: platform-apis §14. |
| 1.07 | Twitch: whispers read/send | ✓ | ◐ | ✓ | — | P2 | P2-Chat+TTS | SpB lets mods whisper `!tts` commands. |
| 1.08 | Twitch: follows | ✓ | ✓ | ✓ | all | P0 | P1-Core | `channel.follow` v2 needs `moderator:read:followers`. |
| 1.09 | Twitch: subs by tier (T1/T2/T3, Prime), resub, sub message, months/streak | ✓ | ✓ | ✓ | SL, SE, LS | P0 ★ | P1-Core | Tier `1000/2000/3000`; Prime via `is_prime` on chat notice. Drives tier alert variants (§9). |
| 1.10 | Twitch: gift subs (single; recipient, anonymous) | ✓ | ✓ | ✓ | SE, SL, LS | P1 | P1-Core | `channel.subscription.gift` / chat notice `sub_gift`. |
| 1.11 | Twitch: gift bomb / community gift (total, bonus, recipient list, `fromGiftBomb`) | ✓ | ✓ | ✓ | SE flags, LS | P1 ★ | P1-Core | Children share community-gift id; required for merge logic (§9). |
| 1.12 | Twitch: gift paid upgrade, prime paid upgrade, pay-it-forward, sub-counter rollover | ✓ | — | — | — | P2 | P1-Core |  |
| 1.13 | Twitch: cheers/bits (cheermote strip, bits badge tier, `channel.bits.use`) | ✓ | ✓ | ✓ | all | P0 | P1-Core | `bits:read`; Combos removed 2026-04. |
| 1.14 | Twitch: incoming raid | ✓ | ✓ | ✓ | all | P0 | P1-Core |  |
| 1.15 | Twitch: outgoing raid (start/cancel, Raid Start/Send/Cancelled triggers) | ✓ | — | ✓ | — | P1 | P1-Core |  |
| 1.16 | Twitch: Hype Train v2 (begin/progress/end, level, golden-kappa/treasure/shared, top contributors) | ✓ | ✓ | ✓ | — | P1 ★ | P1-Core | v1 EventSub + Get Hype Train Events removed 2025-12-04; use Get Hype Train Status. Meter widget in §9. |
| 1.17 | Twitch: channel-point redemption + status update (fulfil/cancel) | ✓ | ✓ | ✓ | FB, MIU | P0 | P1-Core | SpB: per-reward weighted TTS (§10). |
| 1.18 | Twitch: channel-point reward CRUD (create/edit/cost/cooldown/max/pause/enable, groups, counters) | ✓ | — | ✓ | AIT groups | P1 | P1-Core | Only the creating client ID may edit custom rewards (Firebot v5.60 forced re-creation). |
| 1.19 | Twitch: Power-Ups (`bits.use` gigantify/message effects/celebrations, automatic reward redemption, custom Power-Ups) | ✓ | — | ✓ | FB | P1 | P1-Core | `custom_power_up_redemption` GA 2026-05-19; SB split Power Up Redemption trigger in 1.0.5. |
| 1.20 | Twitch: polls (create/end/archive + begin/progress/end events) | ✓ | — | ✓ | FB widget | P1 | P1-Core |  |
| 1.21 | Twitch: predictions (create/lock/resolve/cancel + events) | ✓ | — | ✓ | FB widget | P1 | P1-Core |  |
| 1.22 | Twitch: core moderation actions (ban/timeout/unban/untimeout/delete/clear, chat modes, mod/VIP add/remove, shoutout) | ✓ | — | ✓ | all | P1 | P2-Chat+TTS | Needed for dock quick-actions (§8). Lead Moderator: check badge `lead_moderator` or `moderator`. |
| 1.23 | Twitch: advanced moderation (Shield Mode, AutoMod hold/approve/deny, blocked/permitted terms, warnings, unban requests, suspicious users) | ✓ | — | ✓ | — | P2 | P2-Chat+TTS | `automod.message.hold` also feeds deleted-message sync. |
| 1.24 | Twitch: moderation events as triggers (banned/timed-out/unbanned/message-deleted/chat-cleared) | ✓ | ◐ | ✓ | — | P1 | P1-Core | SpB uses them only to drop queued TTS (§10). |
| 1.25 | Twitch: shared chat (session events, source fields, event flags, shared notices) | ✓ | ◐ | ✓ | — | P1 | P1-Core | Prevents duplicate alerts; SpB option to ignore shared chat; SAM "own chat only". |
| 1.26 | Twitch: channel management (title, game, tags, branded content, stream markers, clips + download URLs) | ✓ | — | ✓ | all | P1 | P1-Core |  |
| 1.27 | Twitch: ads (run commercial, ad schedule, snooze, Ad Run / Upcoming Ad triggers) | ✓ | — | ✓ | — | P2 | P1-Core | Ad Mid-Roll died with PubSub. |
| 1.28 | Twitch: pinned chat messages (pin/unpin/update/get) | ✓ | — | ✓ | FB 5.67 | P2 | P2-Chat+TTS | API GA 2026-05-29. |
| 1.29 | Twitch: stream online/offline/update, viewer count | ✓ | — | ✓ | all | P0 | P1-Core |  |
| 1.30 | Twitch: present-viewers list | ✓ | ◐ | ✓ | — | P2 | P2-Chat+TTS | Live chatter list; SpB tracks presence for its Users tab. |
| 1.31 | Twitch: goals, charity campaigns, community goals | ✓ | ◐ | ✓ | — | P2 | P1-Core | Community Goal events dead since PubSub removal. |
| 1.32 | Twitch: watch streak, modiversary, first words, announcements, pyramids | ✓ | — | ◐ | FB | P2 | P2-Chat+TTS | Pyramid detector is SB-internal. |
| 1.33 | Twitch: info queries (latest follower/sub, follower/sub counts, follow age, random viewer, mods/VIPs/banned lists) | ✓ | — | ✓ | — | P1 | P1-Core | SAM exposes ~25 info commands. |
| 1.34 | Twitch: generic EventSub subscription passthrough + raw Helix call | ◐ | — | ✓ | — | P2 | P1-Core | SAM Custom EventSub trigger + Raw API Call; SB via C#. Also expose in script host API. |
| 1.35 | Twitch: Guest Star | ✓ | — | ✓ | — | P3 | P4-Integrations+SDK | Beta API; SB 4 triggers + 13 sub-actions. |
| 1.36 | Twitch: IRC chat | — | — | ◐ | tmi.js | ✗ | — | Twitch prefers EventSub; non-secure IRC WS removed 2025-08-15; SB removed IRC in 1.0.5. |
| 1.37 | Twitch: PubSub, Hosts | — | — | — | — | ✗ | — | PubSub decommissioned 2025-04-14; Hosts discontinued. |
| 1.38 | Twitch: Chat Bot badge, conduits, app-access tokens | — | — | — | — | ✗ | — | Need `client_secret`/app token (public client cannot); badge is cosmetic. Revisit only if relay mints app tokens. |
| 1.39 | YouTube: OAuth (Desktop client, loopback + PKCE, device-flow fallback), broadcaster + optional bot | ✓ | — | ✓ | SSN | P1 | P2-Chat+TTS | System browser only; device flow limited to `youtube`/`readonly` scopes; unverified apps capped at 100 users; "Testing" tokens expire in 7 d. |
| 1.40 | YouTube: quota strategy (shared project + audit vs BYO API project/key) | ✓ | — | ✓ | SSN | P1 | P2-Chat+TTS | Default 10k units/day/project; SB pool runs dry; SAM has 8M/day shared. Apply for OAuth verification + quota audit early; BYO is advanced setting. Owner decision. |
| 1.41 | YouTube: chat read via `liveChatMessages.streamList` (gRPC, resume token) | ✓ | — | ✓ | SSN | P1 | P2-Chat+TTS | 5 units/call; per-connection billing unverified; `tonic`/`@grpc/grpc-js`; polling `list` burns 14,400 units/4 h. |
| 1.42 | YouTube: chat send | ✓ | — | ✓ | — | P1 | P2-Chat+TTS | 20 units each; 200 chars and ~11 msgs/30 s (Help Center, not API docs); no reply/thread field. |
| 1.43 | YouTube: moderation (ban/timeout, delete, mods) | ✓ | — | ✓ | — | P2 | P2-Chat+TTS | Bans cost 200 units: rate-limit or confirm. |
| 1.44 | YouTube: Super Chat (amount, currency, tier, comment) | ✓ | — | ✓ | SL, SE | P1 ★ | P2-Chat+TTS | `superChatEvent`; `superChatEvents.list` (30 d) for reconciliation. |
| 1.45 | YouTube: Super Sticker | ✓ | — | ✓ | SSN | P1 ★ | P2-Chat+TTS | `superStickerEvent` (`stickerId`, `altText`). |
| 1.46 | YouTube: memberships (new member, upgrade, milestone) | ✓ | — | ✓ | SL, SE | P1 ★ | P2-Chat+TTS | `newSponsorEvent`, `memberMilestoneChatEvent`; `members.list` needs Google approval so use chat events. |
| 1.47 | YouTube: gifted memberships (gifting + received) | ✓ | — | ✓ | SL, SE | P1 ★ | P2-Chat+TTS | `membershipGiftingEvent` + N `giftMembershipReceivedEvent`; needs merge (§9). |
| 1.48 | YouTube: Jewels gifts | ✓ | — | ✓ | SSN | P1 | P2-Chat+TTS | `giftEvent` since 2026-03-26; renamed `snippet.giftDetails` 2026-10-12. |
| 1.49 | YouTube: new-subscriber detection (poll `subscriptions.list`, public subs only) | ✓ | — | ✓ | SL/SE sockets | P1 ★ | P2-Chat+TTS | No real-time event; SB polls 10 s (360 units/h); recommend 30-60 s while live. SL socket `follow` + SE `channel.activities` are alternatives via vendor cloud. |
| 1.50 | YouTube: broadcast management (monitor multiple, title/description/privacy, stats, started/ended triggers) | ✓ | — | ✓ | — | P1 | P2-Chat+TTS | SB auto-unmonitors idle broadcasts after 10 min to save quota. |
| 1.51 | YouTube: polls (started/updated/closed; create/end) | ✓ | — | ✓ | — | P2 | P2-Chat+TTS | `pollEvent`; tally only for owner-authorized requests. |
| 1.52 | YouTube: members-only mode, user banned/timed out events, present viewers | ✓ | — | ✓ | — | P2 | P2-Chat+TTS | Present viewers is activity-based (no API list). |
| 1.53 | YouTube: deleted-message sync | — | — | — | SSN | P2 | P2-Chat+TTS | Conflict: `messageDeleted/Retracted` removed from API docs 2026-06-23, yet designer relies on `tombstone` + `userBannedEvent`. Verify before promising. |
| 1.54 | YouTube: InnerTube/scraping "unofficial mode" | — | — | — | SSN | P3 | P2-Chat+TTS | Violates YouTube Developer Policies/ToS; plugin-only opt-in at most. Owner decision. |
| 1.55 | YouTube: `members.list`, `membershipsLevels.list` | — | — | — | — | ✗ | — | Requires Google/YouTube representative approval. |
| 1.56 | Kick: OAuth 2.1 + PKCE (needs `client_secret` for exchange and refresh) | ✓ | — | ◐ | — | P1 | P2-Chat+TTS | Default: self-hostable relay token broker (P2, see §23); BYO Kick app + user tunnel as alternative. Public-client support is an open Kick request (#413). Dev apps need 2FA. |
| 1.57 | Kick: events via webhooks (chat, follow, sub new/renew/gifts, KICKs, rewards, live status, metadata, ban) | ✓ | — | ◐ | SSN (DOM) | P1 | P2-Chat+TTS | Public HTTPS endpoint required: self-hostable relay mailbox (P2, §23) or user tunnel; desktop verifies RSA-SHA256 sig (rotating key); ULID idempotency; `chat.message.sent` capped 1,000 subs unverified; auto-unsubscribed after ~1 day of failures. SAM = "Kick Streaming" ext. |
| 1.58 | Kick: chat send (user/bot, reply) | ✓ | — | ◐ | Botrix | P1 | P2-Chat+TTS | 500 graphemes and 2048 bytes; `reply_to_message_id`; bot posts to token's own channel; rate limit unpublished (handle 429). |
| 1.59 | Kick: moderation (ban/timeout/unban/untimeout, delete message) | ✓ | — | ◐ | — | P2 | P2-Chat+TTS | Delete API since 2025-12-02; no message-deleted event. |
| 1.60 | Kick: channel rewards CRUD + redemptions (max 15 in SB) | ✓ | — | — | — | P2 | P2-Chat+TTS | APIs 2025-12/2026-01; no realtime reward updates in SB. |
| 1.61 | Kick: title/category set | ✓ | — | — | — | P2 | P2-Chat+TTS |  |
| 1.62 | Kick: unofficial Pusher chat (read-only fallback, deleted-message sync) | — | — | — | SSN, kick-wss | P3 | P2-Chat+TTS | Kick staff discourage bots on `ws.kick.com`; opt-in plugin; only route to deleted-message events. |
| 1.63 | Kick: raids/hosts, polls, predictions | — | — | — | — | ✗ | — | No Kick API (issue #102). |
| 1.64 | TikTok LIVE: read-only unofficial connector (WebCast; chat, gifts, follows, likes) | — | — | — | LS, SSN | P3 | P4-Integrations+SDK | ToS 3.4 forbids; TikTok-Live-Connector/TikTokLive are AGPL (isolate as separate process); Euler signer free tier 2,500 req/day. Plugin-only. |
| 1.65 | TikTok: TikFinity bridge (their output pushed into our WS API) | ◐ | — | — | SSN | P2 | P6-Ecosystem | SB's documented route; works best with SB-compatible WS (§21). |
| 1.66 | TikTok: chat send | — | — | — | — | ✗ | — | Needs session cookies sent to a third-party signer; ban risk. Community-plugin stance: Q-C9. Composer shows TikTok as read-only with a reason (8.40). |
| 1.67 | X/Twitter: post (BYO developer app, OAuth 2.0 PKCE loopback 127.0.0.1) | ◐ | — | — | SL Desktop | P1 ★ | P2-Chat+TTS | SB has no native X (IFTTT only). Pay-per-use: $0.015/post, **$0.20 with URL**, user pays; `offline.access` for refresh. Plain HTTP so cheap to pull earlier. Phase aligned with D-024 and PLAN.md P2 (was P4). |
| 1.68 | X/Twitter: read own mentions/posts (owned reads) as polling trigger | — | — | — | — | P2 | P2-Chat+TTS | $0.001/resource; ~(inferred) polling only. Phase aligned with D-024 and PLAN.md P2 (was P4). |
| 1.69 | X Live chat / Livestream API | — | — | — | — | ✗ | — | Whitelisted apps only + hosted webhook. Send `POST /2/broadcasts/{id}/chat` (140 chars, `broadcast.write`) exists but is whitelisted too; revisit only if the owner's X app is whitelisted. |
| 1.70 | Trovo | — | — | — | MIU lists | ✗ | — | Live streaming shut down 2026-06-30; SB removed in 1.0.5. |
| 1.71 | Facebook Live (Graph SSE comments, comment as Page) | — | — | — | LS | P3 | P5-Decks+Voice | App Review + Business Verification; device login 60-day tokens; low priority. Phase aligned with D-024 (was P4). Send split out as 1.81. |
| 1.72 | Rumble Live Stream API (pasted API URL, polling: followers/subs/gifted/rants/chat) | — | — | — | — | P3 | P5-Decks+Voice | 50 recent items, ~5 s min refresh; read-only (no write API). Phase aligned with D-024 (was P4); Q-S8 asks about P2. SSE in 1.78, send in 1.79. |
| 1.73 | Discord: outbound webhook post (paste URL; text/file) | ✓ | — | ✓ | FB, MIU | P1 | P2-Chat+TTS | Go-live posts; no auth; SAM file send is Pro. Phase aligned with D-024 and PLAN.md P2 (was P4). |
| 1.74 | Discord: bot gateway (BYO bot token) for receiving events/messages | — | — | ◐ | MIU | P2 | P4-Integrations+SDK | SAM via "Discord Triggers" ext; `MESSAGE_CONTENT` privileged; SB has no receive path. |
| 1.75 | Discord: local RPC voice-activity overlay | ◐ | — | ◐ | — | P3 | P4-Integrations+SDK | `rpc.voice.*` scopes approval-gated. |
| 1.76 | Long-tail platform ingestion via DOM capture (120+ sites) | — | — | — | SSN | P3 | P4-Integrations+SDK | Fragile, ToS exposure; optional plugin; SSN is GPL-3.0 (reimplement). |
| 1.77 | Per-platform rate-limit/quota awareness (Twitch buckets, YouTube units, Kick caps) | ◐ | — | ◐ | — | P1 | P1-Core | YouTube quota meter in UI; Helix 800 pts/min. |
| 1.78 | Rumble: real-time chat read via unofficial SSE (`web7.rumble.com`, no login; init/messages/delete/pin events, rants, raids, gift notices; HTTP 204 = chat ended) | — | — | — | — | P3 | P5-Decks+Voice | Unofficial but login-free; no event IDs, so no resume. Stream ID comes from the official API. platform-apis §8.1. Q-S8 (phase). |
| 1.79 | Rumble: chat send via session cookie (unofficial; pending Q-C8; recommended form: opt-in, StreamDen sign-in window, cookie in keychain, read-only fallback on breakage) | — | — | — | — | P2 | P5-Decks+Voice | Owner request 2026-10-19. No official write API; ToS bans automated access; a community client's session check broke "again" in Oct 2026 (cocorum #35). Max length from SSE `init`. No paid rants. Clean-room: behaviour learned from Apache-2.0 cocorum, no code copied. Q-C8 (accept risk), Q-S8 (phase). |
| 1.80 | Rumble: moderation (delete message, pin/unpin, mute/unmute per stream or channel-wide; unofficial, pending Q-C8) | — | — | — | — | P2 | P5-Decks+Voice | Same session cookie and risk as 1.79; unmute needs an HTML-scraped record ID (most fragile). Q-C8. |
| 1.81 | Facebook Live: comment as Page (send), delete/hide comments | — | — | — | — | P3 | P5-Decks+Voice | `POST /{live-video-id}/comments` with a Page token (`pages_manage_engagement`); App Review; length and rate limits unpublished. |
| 1.82 | Discord: send from the multichat composer to a Discord channel (bot reply with `message_reference`, or webhook) | — | — | — | — | P2 | P2-Chat+TTS | Webhook target in P2 (D-024); bot replies with the companion bot in P4 (Q-S7). 2000 chars; `allowed_mentions: {parse: []}` on bridged text; ignore own/webhook messages (loop guard). |
| 1.83 | Streamplace (atproto live video): chat read/send as `place.stream.chat.message` records, moderation (block/hide/pin) | — | — | — | — | P3 | P4-Integrations+SDK | Not in current scope; priority/phase are placeholders pending Q-S9. Same login as Bluesky; 300 graphemes; native replies. |
| 1.84 | Bluesky: live chat | — | — | — | — | ✗ | — | Bluesky has no live chat; "Live Now" links out to an external stream (Twitch/YouTube/stream.place). |

## 2. Core automation engine

| ID | Feature | SB | SpB | SAM | Others | Pri | Phase | Notes |
|---|---|---|---|---|---|---|---|---|
| 2.01 | Actions (name, GUID, group folder, enabled; run by name or id) | ✓ | — | ◐ | FB, MIU | P0 | P1-Core | SAM's unit is the button (§12); one model should back both views. |
| 2.02 | Many-to-many wiring (one action N triggers; one event N actions) | ✓ | — | ✓ | FB | P0 | P1-Core |  |
| 2.03 | Sub-action tree with nestable groups (color, random, weight) | ✓ | — | ◐ | — | P0 | P1-Core | SB groups since 1.0, unlimited nesting; SAM has Comment nesting only. |
| 2.04 | If/Else with True/False containers; operators (=, !=, contains, regex, <, >, exists, ignore-case, null/empty); regex captures to `match[i]`/named groups | ✓ | — | ✓ | — | P0 | P1-Core | Auto Type toggle. |
| 2.05 | Switch/Case with Default | ✓ | — | ✓ | — | P1 | P1-Core |  |
| 2.06 | While loop, Loop Break/Continue, Break Running | ✓ | — | ◐ | — | P1 | P1-Core | SAM: Repeat/Break/Exit. |
| 2.07 | Repeat N / for-each over list | — | — | ✓ | — | P1 | P1-Core | SB lacks arrays so uses While (inferred need). |
| 2.08 | Try block (inner failure does not abort action) | ◐ | — | ✓ | — | P1 | P1-Core | SB: "Save Result to Variable" on C# only. SAM 2026.4.0. |
| 2.09 | Break/Exit action | ✓ | — | ✓ | — | P0 | P1-Core |  |
| 2.10 | Delay sub-action with random range | ✓ | — | ◐ | — | P0 | P1-Core | Ignored when action is Concurrent. |
| 2.11 | Absolute per-command delay timeline (ms offset from start) | — | — | ✓ | designer timeline | P2 | P5-Decks+Voice | SAM semantics (delay outranks order); optional deck-view feature. |
| 2.12 | Random-action mode (run one random top-level sub-action) | ✓ | — | ◐ | — | P1 | P1-Core |  |
| 2.13 | Weighted random groups; Set Sub-Action Weight at runtime | ✓ | ✓ | — | SE | P1 | P1-Core | SpB uses weighted variants for events/rewards (§10). |
| 2.14 | Concurrent mode (all sub-actions at once) | ✓ | — | ◐ | — | P1 | P1-Core |  |
| 2.15 | Run Action inline (variables flow back) vs queued | ✓ | — | ◐ | — | P0 | P1-Core | SAM: Trigger Button. |
| 2.16 | Action queues: Default non-blocking; named blocking FIFO; pause/resume/clear | ✓ | — | ◐ | KC | P0 | P1-Core | Canonical use: alerts. SB 1.0.5 applies blocking flag without restart. |
| 2.17 | Button-style concurrency: Group-ID mutual exclusion, Allow-Queue vs discard, Allow-Overlap, Block/Stop, Reinit vars | ◐ | — | ✓ | — | P1 | P5-Decks+Voice | Map onto queue model + "discard if busy" flag. |
| 2.18 | Always Run (runs while queue paused) | ✓ | — | — | — | P2 | P1-Core |  |
| 2.19 | Exclude from history/pending (high-frequency actions) | ✓ | — | — | — | P2 | P1-Core |  |
| 2.20 | Pending-actions view | ✓ | — | ✓ | — | P1 | P1-Core | SAM Active Buttons window. |
| 2.21 | Action history with before/after variable snapshots, duration, state, queue | ✓ | — | ◐ | — | P0 | P1-Core | Primary way to discover trigger args. |
| 2.22 | Requeue/replay of a history run or logged event | ✓ | — | ✓ | — | P1 | P1-Core | SB "Requeue"; SAM event-log replay is Pro. |
| 2.23 | Execution trace events over API (`Raw.Action`, `Raw.SubAction`, `Raw.ActionCompleted`) | ✓ | — | — | — | P1 | P1-Core | Enables external debuggers. |
| 2.24 | APS (actions/sec) display, sortable history | ✓ | — | — | — | P3 | P1-Core | SB 1.1.0. |
| 2.25 | Local signals (send/wait with timeout, queued signal) | ✓ | — | ◐ | — | P1 | P1-Core | SAM: Wait Until Variable Exists/Is. |
| 2.26 | Wait primitives (wait until variable exists/is, wait for keyboard) | — | — | ✓ | — | P2 | P1-Core | Signals cover most. |
| 2.27 | Trigger model: stable numeric event ids grouped by source ranges, hierarchical categories | ✓ | — | ◐ | — | P1 | P1-Core | SB: Twitch 1xx/4xx, YouTube 40xx, Kick 350xx, OBS 140xx. Copy the range idea. |
| 2.28 | Trigger filter types: Basic/Toggle/Select(Any)/Range(min-max, max-only exact)/Text | ✓ | — | ◐ | — | P0 | P1-Core |  |
| 2.29 | Trigger precedence: most-specific range wins unless Always Run | ✓ | — | ◐ | — | P1 | P1-Core | SAM instead stops at first matching trigger in insertion order (wildcards). |
| 2.30 | Per-trigger enable/disable; Set Trigger State sub-action | ✓ | — | ◐ | AIT groups | P1 | P1-Core |  |
| 2.31 | Per-trigger exclusions | ◐ | — | — | — | P3 | P1-Core | `exclusions[]` exists in exports, undocumented (inferred). |
| 2.32 | Test Trigger (sample data, F5) | ✓ | — | ✓ | SE, SL | P0 | P1-Core | SAM Bridge panels fake every Twitch/YouTube payload. |
| 2.33 | Simulated events app-wide (all matching triggers fire) | ✓ | — | ✓ | — | P1 | P1-Core |  |
| 2.34 | Core Test trigger with fixture args (`isTest`) | ✓ | — | — | — | P1 | P1-Core |  |
| 2.35 | Trigger Viewer (global trigger-to-action map) | ✓ | — | — | — | P2 | P1-Core |  |
| 2.36 | Script-registered custom triggers + fire from script/API/sub-action | ✓ | — | ✓ | — | P1 | P1-Core | SB `RegisterCustomTrigger`/`TriggerCodeEvent`; SAM Extension Trigger. Plugins declare triggers in manifest (§22). |
| 2.37 | Wildcard triggers with capture (`*`, Pull Wildcard) | — | — | ✓ | — | P2 | P1-Core | Chat, whispers, channel-point input. |
| 2.38 | Lifecycle triggers: app started/shutdown/crash/reset; integration Connected/Disconnected | ✓ | — | ✓ | — | P0 | P1-Core |  |
| 2.39 | Enable/disable at every level (action, group, sub-action, trigger, command, timer, queue) with get/set sub-actions | ✓ | — | ◐ | — | P1 | P1-Core |  |
| 2.40 | Desktop prompts in automations (Get Input, Wait for Input/Choice/Dialog, Popup) | ◐ | — | ✓ | — | P2 | P1-Core | SB Get Input only; SAM prompts non-blocking for other buttons. |
| 2.41 | OS toast/notification sub-action + activation trigger | ✓ | — | ✓ | — | P2 | P4-Integrations+SDK | SB Windows toast only; need cross-platform notifier. |
| 2.42 | Action editor: 3-pane (actions / triggers / sub-action tree) | ✓ | — | ◐ | — | P0 | P1-Core |  |
| 2.43 | Add menus: hierarchical, searchable, favorites | ✓ | — | ✓ | — | P1 | P1-Core | Plugin items appear in same menus (§22). |
| 2.44 | Copy/paste/duplicate/drag-drop nodes; portable clipboard string | ✓ | — | ✓ | — | P1 | P1-Core | SB Base64; works across instances. |
| 2.45 | Multi-select, undo/redo | ◐ | — | ✓ | — | P1 | P1-Core |  |
| 2.46 | Comment sub-action (coloured no-op) and colour coding (disabled red, always-run blue) | ✓ | — | ✓ | — | P2 | P1-Core |  |
| 2.47 | Per-dialog "?" docs link and Test button | ✓ | ✓ | ✓ | — | P2 | P1-Core | SAM F1 opens command docs. |
| 2.48 | Inline variable hints/autocomplete in text fields | ◐ | — | ◐ | — | P1 | P1-Core |  |
| 2.49 | Global search (actions, triggers, commands, text inside commands) | — | — | ✓ | — | P2 | P1-Core | SAM Pro deck search (6 modes). |
| 2.50 | Command templates/Quickbar (saved snippets, context-sensitive inserts) | — | — | ✓ | MIU library | P2 | P1-Core | SAM Pro. |
| 2.51 | Run a single node from the editor (Ctrl+R) | — | — | ✓ | — | P2 | P1-Core |  |
| 2.52 | ★ Node-based action editor (bidirectional with list/tree view) | — | — | — | SSN Event Flow | P1 | P3-Designer | Owner: frequently requested by Streamer.bot users. A second editable view of the same action model. |
| 2.53 | Tween/animation scheduler independent of UI focus (engine-level) | — | — | ✓ | — | P2 | P4-Integrations+SDK | SAM's FPS-throttle pitfall; see §13 OBS Motion. |

## 3. Variables & expressions

| ID | Feature | SB | SpB | SAM | Others | Pri | Phase | Notes |
|---|---|---|---|---|---|---|---|---|
| 3.01 | Per-run argument stack with `%var%`-style interpolation in all text fields | ✓ | ◐ | ✓ | all | P0 | P1-Core | SpB `%name%` event templates; SAM `/$var$/`. |
| 3.02 | Native template/expression language (typed, no `eval`) | — | — | ◐ | designer `{{path}}` | P0 | P1-Core | Syntax conflict in research: SB `%var%`, designer `{{path}}`, plugin SDK `{slug.field}`. Pick one, support compat parsers. Owner decision. |
| 3.03 | SB-compatible parser (`%var%`, `~global~`, `$math()$`, `$length()$`, `$parse()$`) for imports | ✓ | — | — | — | P1 | P6-Ecosystem | Needed for share-string import (§21). |
| 3.04 | Typed values: number, string, bool, array, object (nested JSON) | — | — | ✓ | — | P0 | P1-Core | SB is string-typed with Auto Type; scripts need real types. |
| 3.05 | Auto Type coercion toggle | ✓ | — | ◐ | — | P1 | P1-Core | Compat for imports; native types reduce need. |
| 3.06 | Format specifiers (`%var:fmt%`) | ✓ | — | — | — | P2 | P6-Ecosystem | .NET format strings; compat only. |
| 3.07 | Inline math/length/parse functions | ✓ | — | ✓ | — | P1 | P1-Core | SB mXparser; SAM `/$(expr)$/`. |
| 3.08 | Flattening conventions: JSON to dotted paths, `#`-indexed lists, `.count`, `rawInput` + Escaped + UrlEncoded triplets | ✓ | — | — | — | P1 | P6-Ecosystem | Needed for import and WS-API compat; native objects preferred. |
| 3.09 | Built-in variables (date, time, unixtime, filedatetime, actionId/Name, queueName, triggerId/Name/Category, `isTest`, `__source`, `eventSource`) | ✓ | — | ◐ | — | P0 | P1-Core |  |
| 3.10 | Common per-platform variable groups (user, broadcaster, chat, shared-chat source) + normalized `user.*`/`event.*`/`amount.*`/`sub.*` | ✓ | — | ✓ | SE | P0 | P1-Core | Normalize across platforms (cf. 1.01). |
| 3.11 | Set Argument / Get-Set Global sub-actions (Value/Inc/Dec, default value) | ✓ | — | ✓ | — | P0 | P1-Core |  |
| 3.12 | Global variables persisted + temp (clear non-persisted) | ✓ | — | ◐ | — | P0 | P1-Core | SAM has no built-in persistence (INI/CSV/ext). |
| 3.13 | User variables per platform user, persisted/temp | ✓ | — | — | — | P1 | P2-Chat+TTS | Superseded by merged identity (§20). |
| 3.14 | Variable scopes: local, button/action-persistent, instance, init variables, Local Variable Persist | ◐ | — | ✓ | — | P1 | P5-Decks+Voice | Needed for deck-style buttons. |
| 3.15 | Trigger payload exposed as object (`payload`) | — | — | ✓ | — | P1 | P1-Core | SAM Expose Payload 2026.5.0. |
| 3.16 | Variable change triggers (created/updated/deleted, user variable updated) | ✓ | — | — | — | P1 | P1-Core | User-updated is high volume. |
| 3.17 | Live Variable Viewer/editor (typed, searchable, copy path, delete; per-user tabs) | ✓ | — | ✓ | — | P1 | P1-Core |  |
| 3.18 | Arrays/objects: create/get/set/insert/remove/size/parse/stringify, JSON-path get | — | — | ✓ | — | P1 | P1-Core | SAM ~28 commands; scripts also cover. |
| 3.19 | Array higher-order ops (filter, map, sort, shuffle, random, concat) | — | — | ✓ | — | P2 | P1-Core |  |
| 3.20 | String utilities (format, replace, split, regex match/captures, trim, case, URL-encode, hash) | ◐ | — | ✓ | — | P1 | P1-Core |  |
| 3.21 | Number/date-time utilities (random, clamp, lerp, remap, date math/diff/duration) | ◐ | — | ✓ | — | P2 | P1-Core | SB Get Random Number is P0 (inferred). |
| 3.22 | Variable Transition (tween a numeric variable over time) | — | — | ✓ | — | P2 | P4-Integrations+SDK | Shares tween scheduler (2.53). |
| 3.23 | Variables over API (get/set/watch) | ✓ | — | ✓ | — | P0 | P1-Core | See §7. |

## 4. Commands

| ID | Feature | SB | SpB | SAM | Others | Pri | Phase | Notes |
|---|---|---|---|---|---|---|---|---|
| 4.01 | Command entity (name, group, enabled, include-in-listing) | ✓ | ◐ | ◐ | NB, FOS, MIU | P0 | P1-Core | SpB custom commands = alias + roles; SAM uses Chat triggers instead. |
| 4.02 | Match modes Start/Exact/Anywhere/Regex, case option, multi-alias, no fixed prefix | ✓ | — | ◐ | — | P0 | P1-Core | Regex fills `match[i]`/named groups; SAM uses `*` wildcards. |
| 4.03 | Platform-agnostic sources (Twitch message/whisper/sub-resub message, YouTube, Kick) | ✓ | — | ◐ | — | P1 | P2-Chat+TTS | One command serves all platforms. |
| 4.04 | Global + per-user cooldowns (broadcaster exempt; shared across aliases) | ✓ | — | — | NB, FOS | P0 | P1-Core |  |
| 4.05 | Command Cooldown trigger; runtime cooldown control (add/remove/reset/resize) | ✓ | — | — | — | P1 | P1-Core |  |
| 4.06 | Permissions Allow/Deny by users, groups, roles (mod, sub, VIP, etc.) | ✓ | ✓ | ◐ | all | P0 | P1-Core | SB: empty Allow = everyone, empty Deny = nobody; broadcaster exempt. SAM filters: Mod, Lead Mod, Sub, VIP, Founder, username. |
| 4.07 | Counters (`counter`, `userCounter`, persisted, reset) | ✓ | — | — | NB, MIU | P1 | P1-Core |  |
| 4.08 | Command arguments (`input#`, `rawInput`, escaped/URL-encoded variants) | ✓ | ◐ | ✓ | — | P0 | P1-Core | SpB uses `%input%` in reward text. Prefer native arrays (cf. 3.08). |
| 4.09 | Ignore bot account / ignore internal (loopback) messages | ✓ | — | — | — | P2 | P2-Chat+TTS | Internal off lets chat-window input fire commands. |
| 4.10 | Command groups with bulk enable/disable; Get/Set state sub-actions | ✓ | — | — | — | P1 | P1-Core |  |
| 4.11 | Get Commands sub-action (for `!commands`, permission-aware) | ✓ | — | — | — | P2 | P1-Core |  |
| 4.12 | "Not allowed" chat reply when user lacks permission | — | ✓ | — | — | P2 | P2-Chat+TTS | SpB Speaking Options. |
| 4.13 | Defaults for newly created actions/commands | ✓ | — | — | — | P3 | P1-Core | SB 1.0. |
| 4.14 | Built-in spam-filter rules (caps, symbols, links, emote flood, repetition, blocked terms) + moderator audit log | — | — | — | NB, FOS | P3 | P2-Chat+TTS | Borrow from Fossabot; not in SB/SAM. |
| 4.15 | One-click starter command library | ◐ | — | ◐ | MIU | P2 | P6-Ecosystem | Via registry templates (§22). SB extensions site is manual import. |

## 5. Timers & scheduling

| ID | Feature | SB | SpB | SAM | Others | Pri | Phase | Notes |
|---|---|---|---|---|---|---|---|---|
| 5.01 | Interval timers (repeat, interval seconds, enabled) | ✓ | — | ✓ | all | P0 | P1-Core | SAM Repeat Interval starts at launch and cannot be toggled (gated with Block). |
| 5.02 | Random interval range | ✓ | — | — | — | P1 | P1-Core |  |
| 5.03 | Minimum chat-lines gate | ✓ | — | — | NB | P1 | P2-Chat+TTS | SB limits it to a single platform; make cross-platform. |
| 5.04 | Cron trigger (expression, `triggeredAt`) | ✓ | — | — | — | P1 | P1-Core | SB 1.1.0 alpha. Time-zone/DST behaviour to define (inferred). |
| 5.05 | Enable/disable timer and set interval from sub-action/API/script | ✓ | — | ◐ | — | P1 | P1-Core |  |
| 5.06 | Timer `counter` variable | ✓ | — | — | — | P2 | P1-Core |  |
| 5.07 | Timers included in export/import packages | ✓ | — | ◐ | — | P1 | P1-Core |  |

## 6. Code execution

| ID | Feature | SB | SpB | SAM | Others | Pri | Phase | Notes |
|---|---|---|---|---|---|---|---|---|
| 6.01 | JavaScript execute-code sub-action with read/write access to action variables and return value | — | — | ◐ | FB, LS, KC | P0 ★ | P1-Core | SAM only via Bridge JS / "Javascript Eval" ext. Runtime choice open: deno_core/V8, rquickjs or sidecar; run in separate exthost process (plugin-SDK doc). |
| 6.02 | Python execute-code sub-action with read/write access to action variables | — | — | — | — | P1 ★ | P4-Integrations+SDK | SB reaches Python only via Run a Program + stdout. Provision managed Python via `uv` (+30-50 MB) or system Python (open). Owner must-have; candidate to pull earlier. |
| 6.03 | Script host API (CPH-like facade: variables, run action, chat send, OBS call, HTTP, log, wait, TTS) generated from protocol schema | ✓ | — | ◐ | FB | P0 ★ | P1-Core | SB `CPH` ~627 methods; expose a versioned, narrow facade, never internals. |
| 6.04 | Script lifecycle (Init on compile/startup, Execute, Dispose), precompile, delayed start, save result to variable | ✓ | — | — | — | P1 | P1-Core |  |
| 6.05 | Shared named code blocks callable from other actions | ✓ | — | — | — | P1 | P1-Core | SB Execute C# Method / `ExecuteMethod`. |
| 6.06 | Script isolation: separate process, timeouts, memory caps | ◐ | — | — | VSC | P0 | P1-Core | SB AppDomain is only an error barrier; bad threads can crash host. |
| 6.07 | Script editor (Monaco: autocomplete of host API, format, error log with line numbers) | ✓ | — | ◐ | — | P1 | P1-Core | SB falls back to AvalonEdit; need type definitions for JS and Python. |
| 6.08 | Run a Program (exe/node/python/bat/ps1/URI; args, cwd, env, wait for exit, stdout to `output#`) | ✓ | — | ✓ | — | P1 | P1-Core | SAM: Execute Program, Command Line. |
| 6.09 | Import review dialog for code ("this import contains code", show source) | ✓ | — | — | — | P1 | P6-Ecosystem | Also for plugin/action bundles (§22). |
| 6.10 | TypeScript for user scripts (transpile at load) | — | — | — | — | P2 | P1-Core | (inferred) plugins transpile TS with swc; reuse. |
| 6.11 | Node-compatible scripts with npm packages (via sidecar) | — | — | — | FB | P2 | P4-Integrations+SDK | Embedded runtime has no Node built-ins. |
| 6.12 | C# execute-code (Roslyn) | ✓ | — | — | — | P3 | P6-Ecosystem | Owner prefers JS/Python; at most optional .NET sidecar with CPH facade (QUESTIONS.md). |
| 6.13 | CPH-compatible facade for porting SB scripts (JS/Python/C# sidecar) | ✓ | — | — | — | P2 | P6-Ecosystem | Open: plugin-SDK Q11. |

## 7. Servers & public API

| ID | Feature | SB | SpB | SAM | Others | Pri | Phase | Notes |
|---|---|---|---|---|---|---|---|---|
| 7.01 | WebSocket server (JSON request/response + pub/sub events, Hello/auth handshake) | ✓ | ✓ | ◐ | obs-websocket | P0 ★ | P0-Foundations | SB 8080, SpB 7580, SAM Bridge 9425 / Deck 9470. |
| 7.02 | Native protocol: JSON-RPC 2.0 envelope with obs-websocket v5 semantics (Hello/Identify, `rpcVersion`, opt-in high-volume events, batches, msgpack option) | ◐ | ◐ | — | OBS | P0 ★ | P0-Foundations | Proposed in plugin-SDK doc. SB-wire-compat is a separate shim (§21). |
| 7.03 | Auth: scoped hashed tokens (`slt_` prefix), revocable, listed in Settings | ◐ | — | ◐ | — | P0 ★ | P0-Foundations | SB/obs-style password only, no scopes. |
| 7.04 | Pairing flow (device-code style approval with 6-digit code) and legacy password challenge | ◐ | — | ◐ | SD Mobile | P1 | P1-Core |  |
| 7.05 | Web-attack hardening: Host/Origin checks, no localhost bypass, bind 127.0.0.1 by default | — | — | — | — | P0 ★ | P0-Foundations | Defeats DNS rebinding/cross-origin WS from web pages; SB auth is off by default. |
| 7.06 | LAN mode (opt-in): pairing required, TLS with pinned self-signed cert, allowlist | ◐ | ◐ | ◐ | — | P1 | P1-Core | SB recommends Tailscale; SAM suggests ngrok. |
| 7.07 | HTTP REST API (OpenAPI 3.1) incl. `POST /rpc` parity endpoint | ✓ | — | ✓ | — | P0 ★ | P1-Core | SB 7474 `/api/DoAction` etc.; SAM Local API 9450. |
| 7.08 | SSE event stream (`GET /events?topics=`) | — | — | — | — | P1 | P1-Core | curl-friendly subscriptions. |
| 7.09 | Inbound webhooks `POST /hooks/<plugin>/<hook>` with per-hook secret/HMAC mapped to triggers | ◐ | — | ✓ | — | P1 | P1-Core | SAM `/webhook` local only; SB Custom Webhook goes via its cloud. Public exposure needs tunnel/relay (§23). |
| 7.10 | Static file hosting / path mappings (overlays, assets) | ✓ | — | — | MIU, FB | P1 | P1-Core | SB 1.0 Mappings. |
| 7.11 | UDP server (DoAction) | ✓ | ✓ | — | — | P2 | P4-Integrations+SDK | SB 4242, SpB 6669; VoiceAttack plugin uses it. |
| 7.12 | Custom WebSocket servers (user-defined; open/close/message triggers; broadcast) | ✓ | — | — | — | P2 | P4-Integrations+SDK |  |
| 7.13 | Outgoing WebSocket clients (user-defined; open/close/message triggers; send) | ✓ | — | ◐ | — | P1 | P4-Integrations+SDK | SB sends only via C#; add sub-action. SAM "Websocket Connector" ext. |
| 7.14 | Self-describing API: `/openapi.json`, `/asyncapi.json`, `/protocol.json`, `/docs`, in-app API explorer incl. plugin requests | — | — | — | OBS docs | P1 | P1-Core | Generated from one schema source. |
| 7.15 | Generated client SDKs: TypeScript and Python (v1.0) | ◐ | — | ◐ | `@streamerbot/client` | P1 ★ | P4-Integrations+SDK | SB JS client v2.0.1, SAMMI-Websocket lib. |
| 7.16 | Generated client SDKs: C#, Rust, WIT world | — | — | — | — | P2 | P6-Ecosystem | v1.x per plugin-SDK doc. |
| 7.17 | Event delivery: ids for dedupe, replay buffer (`since` cursor), glob topics | ◐ | — | — | — | P1 | P1-Core | Lets overlays survive reconnect. |
| 7.18 | Script-to-overlay broadcast channel (`General.Custom` equivalent) | ✓ | — | — | — | P1 | P1-Core |  |
| 7.19 | Stable error codes, per-client rate limits, versioning policy (`rpcVersion`, 2-minor/6-month deprecation) | — | — | — | OBS | P1 | P1-Core |  |
| 7.20 | Plugin-extended requests via `plugins.call` / typed vendor requests | — | — | — | OBS vendor | P1 | P4-Integrations+SDK | Typed via manifest JSON Schema. |
| 7.21 | MCP endpoint: Streamable HTTP `/mcp` + `streamden-mcp` stdio shim, token scopes, in-app approval for destructive tools | ◐ | — | — | — | P1 | P4-Integrations+SDK | Tool surface and AI features in §24. SB has only Stream Deck MCP. |
| 7.22 | Default ports chosen to avoid known app ports (port registry, conflict detection) | — | — | — | — | P0 | P0-Foundations | Research suggests 7580-7589 but 7580 is Speaker.bot's WS port; never 8080. |

## 8. Multichat

| ID | Feature | SB | SpB | SAM | Others | Pri | Phase | Notes |
|---|---|---|---|---|---|---|---|---|
| 8.01 | Normalized chat message + deletion schema (type, user, badges, emotes, donation fields, meta) | ◐ | — | — | SSN | P0 | P0-Foundations | Shared by dock, overlay, TTS, designer. |
| 8.02 | Unified chat window in-app (Twitch + YouTube + Kick + plugin sources) | ✓ | — | — | CT, RS, SSN | P1 ★ | P2-Chat+TTS | SB window is WebView2 page `chat.streamer.bot`; ours must be served locally. |
| 8.03 | OBS custom browser dock URL (`http://127.0.0.1:<port>/dock/chat`) | ✓ | — | ◐ | SSN, RS | P1 ★ | P2-Chat+TTS | Serve over 127.0.0.1 not `file://`; OBS 33 CEF 150 tightens local-network access. |
| 8.04 | Native always-on-top chat window as dock fallback | — | — | — | CT | P1 ★ | P2-Chat+TTS | obs-browser disables docks on native Wayland (Hyprland); alt is XWayland. |
| 8.05 | Chat overlay (browser source) with default theme + CSS variables | ◐ | — | — | FB, SSN, MIU | P1 ★ | P2-Chat+TTS | SB overlay is experimental (URL-persisted settings, `data-*` CSS hooks). |
| 8.06 | Designer-driven chat overlay (row template, variations, same renderer as alerts) | — | — | — | LS, FB | P1 ★ | P3-Designer | See §9. |
| 8.07 | Named filter profiles per output (dock, overlay, TTS) | — | ◐ | — | SSN, CT | P1 ★ | P2-Chat+TTS | SpB filters apply to TTS only. |
| 8.08 | Word/phrase/regex exclusion (case-insensitive, whole-word, diacritic/leet/homoglyph normalization, regex timeout guard) | — | ✓ | — | SSN, CT | P1 ★ | P2-Chat+TTS | SpB bad-word filter + regex replace for speech. SSN has no word-filter param. |
| 8.09 | Known-bot exclusion (built-in updatable per-platform list, streamer's bots, user additions; hide / hide name / dim) | — | — | — | SSN | P1 ★ | P2-Chat+TTS | Seed list in designer doc has many "(verify)" logins; sign remote updates. Twitch chat-bot badge detection to verify. |
| 8.10 | Command exclusion (hide messages with `!` and other prefixes; optionally bot replies) | — | ✓ | — | SSN | P1 ★ | P2-Chat+TTS | SpB "ignored prefixes". |
| 8.11 | Per-platform and per-channel show/hide | — | — | — | SSN | P1 | P2-Chat+TTS |  |
| 8.12 | Role filters (broadcaster, mod, VIP, sub tier/member level, founder, verified, new chatter) | — | ✓ | ◐ | SSN, CT | P1 | P2-Chat+TTS | Follower-age availability to verify. |
| 8.13 | Content filters (links with role allow, emote-only, numbers-only, short, all-caps, repeats, max length) | — | ✓ | — | SSN, FOS | P2 | P2-Chat+TTS |  |
| 8.14 | Expression filter language (Chatterino-style) compiling to the same Rule AST as the GUI builder | — | — | — | CT | P2 | P3-Designer |  |
| 8.15 | Highlight rules (mention, first-time, donation/bits, member, question, mod, VIP, keyword) with style/sound/pin/feature actions | ✓ | ◐ | — | SSN, CT | P1 | P2-Chat+TTS | SpB highlight voice alias. |
| 8.16 | Message lifetime, max messages, fade, direction, pause-on-hover | ◐ | — | — | SSN | P1 ★ | P2-Chat+TTS |  |
| 8.17 | Overlay display delay (mods can delete first; dock never delayed) | — | — | — | SSN, SL | P1 | P2-Chat+TTS |  |
| 8.18 | Deleted-message and purge sync (Twitch delete/clear/clear-user + AutoMod hold; YouTube tombstone/ban; Kick ban-purge only) with exit animation or placeholder | ◐ | ◐ | — | SSN, SE | P1 ★ | P2-Chat+TTS | See 1.53 and 1.62 for YouTube/Kick limits. |
| 8.19 | Emotes: Twitch native, 7TV, BTTV, FFZ (global + channel), zero-width stacking, cheermotes | ✓ | ✓ | — | CT, SSN | P1 ★ | P2-Chat+TTS | 7TV REST + `events.7tv.io` live updates; per-provider toggles. |
| 8.20 | Live emote-set changes (7TV/BTTV emote added/removed events) | ✓ | — | — | CT | P2 | P2-Chat+TTS |  |
| 8.21 | Kick and YouTube emotes; Unicode emoji via Twemoji | ✓ | ✓ | — | SSN | P1 | P2-Chat+TTS | Kick `[emote:id:name]` and YouTube member-emoji image lookup to verify. |
| 8.22 | Freeze animated emotes, emote scale, larger emote-only messages | — | — | — | SSN | P2 | P2-Chat+TTS |  |
| 8.23 | Pronouns (alejo.io, lazy + cached) | ✓ | — | — | FB | P2 | P2-Chat+TTS |  |
| 8.24 | Badges (Twitch global/channel, YouTube owner/mod/member/verified, Kick); limit/hide | ◐ | — | — | SSN | P1 | P2-Chat+TTS |  |
| 8.25 | First-time chatter (platform flag + local seen-users DB; first this stream; returning after N days) | ✓ | ◐ | — | SSN, CT | P1 ★ | P2-Chat+TTS | SB "First Words" resets after N hours. |
| 8.26 | Mentions: highlight streamer mentions and `/mentions` panel | — | ◐ | — | CT | P2 | P2-Chat+TTS |  |
| 8.27 | Twitch shared chat: source-channel badge, "only my channel", dedupe | ✓ | ◐ | ✓ | — | P1 | P2-Chat+TTS |  |
| 8.28 | Name colours with readability correction | — | — | — | CT | P2 | P2-Chat+TTS | 7TV paints are P3. |
| 8.29 | Featured/pinned message to overlay (queue, auto-feature rules, timeout) | — | — | — | SSN, FB | P2 | P3-Designer | Twitch pinned-message sync when API allows. |
| 8.30 | Dock composer: combined/tabbed view, reply targeting (Enter / Shift+Enter all / Ctrl+Enter as bot), quick mod actions, search, pause-scroll | ✓ | — | — | CT, RS | P1 | P2-Chat+TTS | Send targets, fan-out and per-target state: 8.39-8.41; key defaults Q-U6. |
| 8.31 | Composer autocomplete (`/action`, `/title`, `/clear`, `:` emotes, `!` commands) | ✓ | — | — | — | P2 | P2-Chat+TTS |  |
| 8.32 | Quick Actions (Global/User/Message/Event scopes run actions with args) | ✓ | — | — | — | P1 | P2-Chat+TTS |  |
| 8.33 | Events feed window/dock | ✓ | — | — | SE | P1 | P2-Chat+TTS |  |
| 8.34 | User card (history, notes, watch time) | ◐ | — | — | CT | P2 | P2-Chat+TTS | (inferred from designer doc) |
| 8.35 | Streamer Mode (hide user content while live) | — | — | — | CT | P3 | P2-Chat+TTS |  |
| 8.36 | Chat relay between platforms | — | — | — | SSN | P3 | P2-Chat+TTS |  |
| 8.37 | Settings sync from app to docks; per-split filters | ✓ | — | — | CT | P2 | P2-Chat+TTS | SB sync is one-way. |
| 8.38 | Local chat history with retention limits | ◐ | — | — | — | P2 | P2-Chat+TTS | Twitch 24 h caching rule, YouTube 30 d (see 20.14). |
| 8.39 | Composer send targets: chips per platform × channel × identity; "send to all" fan-out with a per-target result (sent / dropped with reason / failed / skipped); manual "retry failed" (recommended: no auto-retry, pending Q-U6) | ◐ | — | — | RS | P1 ★ | P2-Chat+TTS | Owner request 2026-10-19: send to every platform with chat. SB ◐ = send-to-all exists (Shift+Enter, 8.30); per-target result reporting in SB/RS (inferred), unverified. Whether unofficial targets (Rumble 1.79) join "send to all" is Q-U6 (recommended: only when ticked). platform-apis §14.2. |
| 8.40 | Read-only and unavailable targets stay visible, disabled with a reason (no official send API, unofficial sending off, signed out, session expired, rate-limited, quota low, not live), driven by connector-advertised send capabilities | — | — | — | — | P1 ★ | P2-Chat+TTS | Capability shape goes into den-protocol via ADR + contract card, not ad hoc. Plugins (e.g. a community TikTok plugin) use the same states. |
| 8.41 | Per-target send pre-flight: length in the platform's unit (chars/graphemes/bytes; 140 X, 200 YouTube/Rumble, 500 Twitch/Kick, 2000 Discord), shared outbound rate limiter, YouTube quota warning, native reply vs `@name` fallback, dedupe of our own sends echoed back on the read path | — | — | — | — | P1 | P2-Chat+TTS | Never truncate silently. Echo dedupe uses the platform message ID returned by the send. |

## 9. Overlays, alerts & designer

| ID | Feature | SB | SpB | SAM | Others | Pri | Phase | Notes |
|---|---|---|---|---|---|---|---|---|
| 9.01 | WYSIWYG designer: absolute canvas at design resolution, constraints/anchors, Stack auto-layout, rulers/guides/snap/safe-area/grid, layer tree (lock/hide/group/align/z-order) | — | — | ◐ | SE, SL, LS, 1UP, OWN | P1 ★ | P3-Designer | SAM has deck canvas only; SB has no designer. Moveable/Selecto unmaintained since Dec 2023: pin and vendor. |
| 9.02 | One renderer shared by editor, dock preview and OBS runtime (DOM/CSS, small vanilla runtime) | — | — | — | 1UP | P1 ★ | P3-Designer | Shell choice (Tauri WebKitGTK vs Electron Chromium) affects pixel parity with OBS CEF. Open. |
| 9.03 | Canvas presets and scale modes (fit/fill/stretch/none; 1080p, 1440p, 4K; read from OBS `GetVideoSettings`) | — | — | — | SE | P1 | P3-Designer |  |
| 9.04 | Vertical 9:16 canvas variants (aspect-ratio overrides) | — | — | — | AIT | P2 | P3-Designer | Open Q13. |
| 9.05 | Element: Text (rich segments, tokens, stroke/shadow/gradient, auto-fit, per-letter animation, inline emotes) | — | — | — | LS, SE | P1 ★ | P3-Designer |  |
| 9.06 | Elements: Image / Video / Audio (WebM VP9 alpha preferred) | — | — | — | all | P1 ★ | P3-Designer | Verify H.264/HEVC in OBS CEF. |
| 9.07 | Elements: Shape, Mask/Clip group, Browser embed | — | — | — | LS | P1 | P3-Designer |  |
| 9.08 | Element: Lottie / dotLottie | — | — | — | 1UP | P2 | P3-Designer | `@lottiefiles/dotlottie-web` (MIT). |
| 9.09 | Element: Rive (state machines) | — | — | — | — | P3 | P3-Designer | Optional. |
| 9.10 | Element: Chat List (filter profile, row template, lifetime, direction) | ◐ | — | — | LS, FB, SSN | P1 ★ | P3-Designer |  |
| 9.11 | Element: Alert Slot (lane binding, region, z-order) | — | — | — | SE AlertBox, SL | P1 ★ | P3-Designer |  |
| 9.12 | Elements: Event List, Label/Counter ("latest follower", "top donor", session counts) | — | — | — | MIU, SE | P1 | P3-Designer |  |
| 9.13 | Elements: Progress/Goal (linear/radial/segmented) incl. Hype Train meter | ◐ | — | — | FB, MIU, LS | P1 ★ | P3-Designer | SB exposes goal data only. |
| 9.14 | Elements: Timer/Countdown/Clock (subathon, BRB) | — | — | — | FB | P2 | P3-Designer |  |
| 9.15 | Elements: Rotator/Slideshow/Rotating text (peek/hide panels) | — | — | — | OE, FB, LS | P2 | P3-Designer |  |
| 9.16 | Elements: Poll / Prediction widgets | — | — | — | FB 5.67 | P2 | P3-Designer |  |
| 9.17 | Elements: Avatar/Badges/Platform icon/Pronouns building blocks | — | — | — | — | P1 | P3-Designer | Used in row templates. |
| 9.18 | Elements: Emote wall/rain/particles (PixiJS canvas element) | ◐ | — | — | SB HTML Overlay | P2 | P3-Designer | SB's Emote Rain lives in a separate desktop app. |
| 9.19 | Elements: Wheel, Leaderboard, Stream Boss, Game Queue | — | — | — | MIU | P3 | P3-Designer |  |
| 9.20 | Element: End Credits (from credits data) | ◐ | — | — | MIU | P2 | P3-Designer | See §20 credits. |
| 9.21 | Animation: enter/idle/exit tracks, keyframe timeline, preset JSON library, WAAPI + Motion engine, text effects | — | — | — | SL, OE, 1UP | P1 ★ | P3-Designer | Avoid GSAP (licence clause bars no-code animation builders). Preload before play. |
| 9.22 | Timeline actions (play sound, trigger app action, VTuber hotkey at +N ms) | — | — | — | designer proposal | P2 | P3-Designer | Cross-ref §14. |
| 9.23 | Template variables in designer (`{{path}}` + filters, namespaces `event/user/amount/sub/stream/global/fields/time`, no `eval`, HTML-escaped) | — | — | — | LS, 1UP | P1 ★ | P3-Designer | Syntax conflict with SB `%var%` / SDK `{slug.field}` (cf. 3.02). |
| 9.24 | Alert rules per event kind with default composition and variations (`when` Rule AST, priority, weight, enabled) | — | — | — | SL, SE, LS | P1 ★ | P3-Designer | Covers Twitch subs by tier, follows, YouTube subs/members/Super Chat/Super Sticker, hype trains. |
| 9.25 | Variation condition presets (tier, months, gift, gifted-at-once ≥N, Prime, bits ≥/=, raid size ≥, amount ≥/=, level name, largest this stream, recurring, random) | — | — | — | SL | P1 ★ | P3-Designer |  |
| 9.26 | Variation inheritance (override patch over defaults) and random pick among matches | — | — | — | SL, SE | P1 | P3-Designer |  |
| 9.27 | Alert queue lanes with priority, optional preemption, `maxConcurrent`, global delay | ◐ | — | ◐ | SL parries, LS | P1 ★ | P3-Designer | SB/SAM only have action queues/button queues. |
| 9.28 | Alert controls via API/dock: pause/resume lane, skip, replay last N, clear, test-fire, requeue | — | — | — | SE `event:skip`, LS | P1 | P3-Designer |  |
| 9.29 | Queue persistence across restart; overflow policy (drop lowest or summarise) | — | — | — | — | P2 | P3-Designer |  |
| 9.30 | Alert ack protocol (`alert.ready/started/finished`, timeout fallback, hold/auto-skip when no runtime) | — | — | — | — | P1 | P3-Designer | Avoid OBS "shutdown source when not visible" stalls. |
| 9.31 | Gift-bomb / combo merge (`mergeWindowMs` + `mergeKey`; suppress, list or mini-ticker children) | ◐ | — | ◐ | SE, LS | P1 ★ | P3-Designer | Twitch community-gift id; YouTube gifting + received; Kick pre-aggregated. |
| 9.32 | Follow/raid train coalescing ("N new followers") | — | — | — | — | P2 | P3-Designer |  |
| 9.33 | Alert moderation hold (approve/deny in dock with timeout) | — | — | — | SL | P2 | P3-Designer |  |
| 9.34 | Profanity/filter profile applied to alert text and TTS (mask or drop) | — | ✓ | — | SL | P2 | P3-Designer |  |
| 9.35 | Test-fire/emulate per variation; mock event generator; "Fire to OBS" vs preview-only | ◐ | — | ◐ | SE, SL | P1 ★ | P3-Designer | Include RTL, long names, emoji-only, 7TV zero-width. |
| 9.36 | Timeline scrub preview, dock preview before going live | — | — | — | SSN | P2 | P3-Designer |  |
| 9.37 | Action sub-action "Show alert / trigger overlay element" with custom args | ◐ | — | — | FB Overlay Alert | P1 | P3-Designer | SB has PolyPop Trigger Alert only. |
| 9.38 | Custom widget SDK: sandboxed iframe (no `allow-same-origin`), `postMessage` bridge, fields schema with `visibleIf`, scoped storage | — | — | — | LS, 1UP, SE | P1 ★ | P3-Designer | Public SDK docs land in P4. |
| 9.39 | ★ StreamElements custom-widget compat runtime (`onWidgetLoad`, `onEventReceived`, `SE_API.*`), widget import, fed by combined multichat | — | — | — | 1UP, SSN | P1 | P2-Chat+TTS | Owner: big ask, so Etsy-bought SE chat/alert widgets run natively. See research/se-widget-compat-and-node-editor.md. |
| 9.40 | Importers translate Streamlabs `{name}` and SB `%var%` templates | — | — | — | — | P2 | P6-Ecosystem |  |
| 9.41 | Overlay document JSON (Zod/JSON Schema, `schemaVersion` migrations) and `.slpack` zip with content-addressed assets | — | — | — | — | P1 | P3-Designer |  |
| 9.42 | Runtime protocol over local WS (hello, load, snapshot, patch, alert prepare/play, chat delete/purge/clear, state patch) | — | — | — | — | P1 | P3-Designer |  |
| 9.43 | Overlay URLs with per-overlay tokens, single-widget URLs, multiple named endpoints | — | — | — | MIU, FB | P1 | P3-Designer |  |
| 9.44 | OBS browser-source hygiene (transparent body, stateless reload, pause on `obsSourceVisibleChanged`, "Control audio via OBS") | — | — | — | — | P1 | P3-Designer |  |
| 9.45 | Audio routing for alerts/TTS: browser source vs app-native output, per lane | — | ◐ | ◐ | — | P1 | P3-Designer | Open Q7. |
| 9.46 | Overlay security: escape all user/platform text, DOMPurify for rich text, no `allowHtml` for chat data | — | — | — | FB | P0 | P0-Foundations | Firebot fixed an XSS in Overlay Alert. |
| 9.47 | HUD mode (overlay on own monitor) | ◐ | — | — | LS | P3 | P3-Designer | SB: separate "HTML Overlay" app. |
| 9.48 | TTS element emitting viseme/mouth events for lip-sync | — | — | — | — | P3 | P4-Integrations+SDK |  |
| 9.49 | Theme/alert-pack gallery with one-click install | ◐ | — | ◐ | OWN, SE, FB | P2 | P6-Ecosystem | Via registry. |
| 9.50 | Collaborative mod co-editing with audit log (Yjs) | — | — | — | 1UP | P3 | P3-Designer | Open Q12. |
| 9.51 | Plugin-contributed element types (designer extension SDK) | — | — | — | 1UP, FB | P1 | P4-Integrations+SDK | Web components, iframe-isolated unless verified. |

## 10. TTS

| ID | Feature | SB | SpB | SAM | Others | Pri | Phase | Notes |
|---|---|---|---|---|---|---|---|---|
| 10.01 | Multi-engine TTS abstraction (unified voice list, per-engine credential setup) | ◐ | ✓ | ◐ | SSN, OpenSpk | P1 | P2-Chat+TTS | SB delegates to Speaker.bot; SAM only via community ext (TTS by K, Uberduck). |
| 10.02 | Local neural engine: Piper | — | — | — | OpenSpk (Speekaboo) | P1 | P2-Chat+TTS | Free offline; Linux/macOS parity; cloud-only prices out hobbyists. |
| 10.03 | Local neural engine: Kokoro (and Kitten) | — | — | — | SSN | P1 | P2-Chat+TTS |  |
| 10.04 | OS-native voices (SAPI5 on Windows, AVSpeechSynthesizer on macOS, speech-dispatcher/eSpeak on Linux) | ◐ | ✓ | — | — | P1 | P2-Chat+TTS | SpB is SAPI5 only; OneCore removed in SpB 0.0.34. |
| 10.05 | Cloud engine: Azure | — | ✓ | — | SSN | P1 | P2-Chat+TTS | Key + region; rate -10..10, pitch -9..11. |
| 10.06 | Cloud engine: Google Cloud TTS | — | ✓ | — | SSN | P1 | P2-Chat+TTS | Service-account JSON; SSML wrapped; SSML counts toward quota. |
| 10.07 | Cloud engine: Amazon Polly | — | ✓ | — | — | P1 | P2-Chat+TTS | No pitch on Neural voices. |
| 10.08 | Cloud engine: ElevenLabs | — | ✓ | — | SSN | P1 | P2-Chat+TTS | Rate/pitch sliders map to Stability/Similarity. |
| 10.09 | Cloud engine: OpenAI-compatible TTS endpoint | — | — | — | SSN | P2 | P2-Chat+TTS | Covers self-hosted servers. |
| 10.10 | Cloud engines: TTS Monster (custom voices), IBM Watson | — | ✓ | — | — | P2 | P2-Chat+TTS | TTS Monster parses overlay URL. |
| 10.11 | Cloud engines: UberDuck, Acapela, CereProc | — | ✓ | ◐ | — | P3 | P2-Chat+TTS | UberDuck voices cut to ~27, some HTTP 500. |
| 10.12 | Engines in clones: Fish Audio, Cartesia, Inworld, Resemble, LMNT, TikTok voices | — | — | — | OpenSpk | P3 | P2-Chat+TTS | TikTok voices via proxy have ToS risk; deliver as plugins. |
| 10.13 | TTS provider contribution point for plugins (`ttsVoices`: list voices, formats, streaming, SSML) | — | — | — | LS | P1 | P4-Integrations+SDK | Audio over side channel, not base64 JSON. |
| 10.14 | Voice Aliases (named multi-voice groups across engines) with per-voice pitch/rate/volume/emphasis normalized to a common scale | — | ✓ | — | — | P1 | P2-Chat+TTS | Map to SSML `prosody` or engine params. |
| 10.15 | Per-alias output device (or application default) | ✓ | ✓ | ✓ | — | P1 | P2-Chat+TTS |  |
| 10.16 | Default alias, highlight/mention alias, event alias override, command alias, per-user alias | — | ✓ | — | — | P1 | P2-Chat+TTS |  |
| 10.17 | Random voices with ignore profiles (voices/locales), sticky random, assign-last-voice-to-user, Past Voices history | — | ✓ | — | — | P1 | P2-Chat+TTS |  |
| 10.18 | Per-user TTS profile (alias, nickname, Ignored, Forced, Regular, Subscribed flag) | — | ✓ | — | — | P1 | P2-Chat+TTS | Tie into merged identity (§20). |
| 10.19 | Speaking modes: say-everything vs command-only (`!say`), custom TTS commands with alias + role permissions | — | ✓ | ◐ | SSN | P1 | P2-Chat+TTS |  |
| 10.20 | Channel-point TTS (per-reward weighted messages with `%input%`) | — | ✓ | — | — | P2 | P2-Chat+TTS |  |
| 10.21 | Event-to-speech templates (weighted variants, per-event enable, minimums for raid/cheer/donation, sub vs sub-with-message) | — | ✓ | — | SSN | P1 | P2-Chat+TTS | Include YouTube/Kick events (SpB is Twitch-only natively). |
| 10.22 | TTS for all platforms (YouTube Super Chats, Kick events) | ◐ | — | — | SSN | P1 | P2-Chat+TTS | SB forwards non-Twitch events via Speak sub-action. |
| 10.23 | Text pipeline: permissions/ignore, prefix filter, emote strip, URL filter, bad-word filter, max words/chars, username/nickname, regex, SSML wrap | — | ✓ | — | SSN | P1 | P2-Chat+TTS | Order inferred; reuse chat filter profiles (§8). Never pass raw SSML from chat. |
| 10.24 | Bad-word filter (Strip or drop whole message; API bypass flag) | — | ✓ | — | SSN | P1 | P2-Chat+TTS |  |
| 10.25 | Ordered regex replacement list | — | ✓ | — | — | P2 | P2-Chat+TTS | SpB dev warns of abuse; add audit log (Q8). |
| 10.26 | URL filter (Strip / drop), max words/chars, ignored prefixes | — | ✓ | — | SSN | P1 | P2-Chat+TTS |  |
| 10.27 | Per-provider emote stripping, allow-first-emote, allowed-emote list, 7TV zero-width handling | — | ✓ | — | — | P2 | P2-Chat+TTS |  |
| 10.28 | Username reading ("said" prefix, only-if-different, grace period, nickname substitution) | — | ✓ | — | — | P2 | P2-Chat+TTS |  |
| 10.29 | Ignore Twitch Shared Chat messages option | — | ✓ | — | — | P2 | P2-Chat+TTS |  |
| 10.30 | Queue controls: pause, resume, stop current, clear pending, enable/disable, gag | ✓ | ✓ | — | — | P1 | P2-Chat+TTS | SB drives via Speaker.bot sub-actions. |
| 10.31 | Processing view (queue + history with engine, voice, locale, params, user, text) and Generic Speaker test pad | — | ✓ | — | — | P2 | P2-Chat+TTS |  |
| 10.32 | Moderation-coupled skipping (drop queued/current TTS on delete, timeout, ban) | — | ✓ | — | — | P1 | P2-Chat+TTS | Needs deletion events (§8, 1.24). |
| 10.33 | VoiceGate: mic-level threshold auto-pause/unpause, timeout, profiles, switch from action/API | ✓ | ✓ | — | — | P1 | P5-Decks+Voice | Shares mic-level capture with STT. Emits `VoiceGate.ProfileActivated`. |
| 10.34 | `!tts` moderator command suite (status, voices, pause/resume/clear/stop, mode, on/off, events, ignore, reg, random reset, sticky, nickname, assign last, profile, permissions) with configurable prefix and whisper control | — | ✓ | — | — | P1 | P2-Chat+TTS |  |
| 10.35 | Speak sub-action: alias, bad-word bypass, Silent (returns WAV path), speak-and-wait (returns `duration`) | ✓ | ✓ | — | — | P1 | P2-Chat+TTS | Needed to sync alerts/overlays; SB hard 30 s timeout. |
| 10.36 | Save all TTS to WAV, per-engine character usage tracking, render cache | — | ✓ | — | — | P2 | P2-Chat+TTS |  |
| 10.37 | TTS events on API (`TextQueued`, `EngineProcessed`, `Playing`, `Finished`, `Deleted`, `Error`, `StartedSpeaking`, `StoppedSpeaking`) for captions/overlays | ◐ | ✓ | — | — | P1 | P2-Chat+TTS |  |
| 10.38 | Donation and merch TTS (Streamlabs/StreamElements sources) | ◐ | ✓ | — | — | P2 | P4-Integrations+SDK | Depends on §17 connectors. |
| 10.39 | Lip-sync mouth/viseme output to VTuber apps | — | — | — | — | P3 | P4-Integrations+SDK | Inferred from designer doc. |
| 10.40 | Audio effects (reverb, echo, robot) | — | — | — | — | P3 | P2-Chat+TTS | None in Speaker.bot; Voicemod covers externally. |

## 11. STT & voice commands

| ID | Feature | SB | SpB | SAM | Others | Pri | Phase | Notes |
|---|---|---|---|---|---|---|---|---|
| 11.01 | Voice commands (phrase, Exact/Start/Anywhere, per-command confidence) | ✓ | — | ✓ | VoiceAttack | P1 | P5-Decks+Voice | SB Windows Speech Recognition; SAM "SAMMI Voice" phrase list. |
| 11.02 | Cross-platform local STT engine (Whisper-class) | ◐ | — | — | — | P1 | P5-Decks+Voice | SB 1.1.0 alpha Whisper.net. Vosk also mentioned as option (inferred). |
| 11.03 | Dictation trigger (free text, confidence %, alternates) | ✓ | — | — | — | P2 | P5-Decks+Voice |  |
| 11.04 | Voice-control log and input-device selection | ✓ | — | ✓ | — | P2 | P5-Decks+Voice | SAM recognition logfile + confidence threshold. |
| 11.05 | Toggle listening (sub-action, listen on startup) | ◐ | — | ✓ | — | P2 | P5-Decks+Voice | SAM "Voice: Toggle Status". |
| 11.06 | Recognition from a remote machine via API | — | — | ✓ | — | P3 | P5-Decks+Voice | SAM Voice calls Local API 9450. |
| 11.07 | Windows Speech Recognition engine | ✓ | — | ✓ | — | P3 | P5-Decks+Voice | Windows-only; Whisper replaces. |
| 11.08 | VoiceAttack integration | ◐ | — | ◐ | — | P3 | P5-Decks+Voice | SB official plugin uses UDP DoAction; SAM ext. Windows. |

## 12. Decks & remote control

| ID | Feature | SB | SpB | SAM | Others | Pri | Phase | Notes |
|---|---|---|---|---|---|---|---|---|
| 12.01 | SAMMI-style deck canvas (grid, snap off/free placement, adaptive resize, background, folders) over the same action model | ◐ | — | ✓ | FB, TP | P1 | P5-Decks+Voice | SB Decks 2.0 are web-hosted via cloud, docs WIP. Offer deck view + list view of one model (open Q4). |
| 12.02 | Button = appearance + settings + triggers + commands + release commands + init variables; all enabled decks live | — | — | ✓ | TP | P1 | P5-Decks+Voice | SAM JSON uses BGR ints and 0-1 positions (importer must handle). |
| 12.03 | Runtime button restyle pushed to all clients (text, colour, image, border, state) | ◐ | — | ✓ | TP | P1 | P5-Decks+Voice | SB deck item set title/icon/background/state. |
| 12.04 | Enable/disable decks at runtime (modes); switch deck per client | — | — | ✓ | TP | P1 | P5-Decks+Voice |  |
| 12.05 | Release commands (press-and-hold) and Drag Press (DIY faders) | — | — | ✓ | TP | P2 | P5-Decks+Voice |  |
| 12.06 | Deck export/import (JSON, with images); deck folders | ◐ | — | ✓ | — | P1 | P5-Decks+Voice |  |
| 12.07 | Phone/LAN deck over QR with state-driven visuals (served locally, installable web app) | ◐ | — | ✓ | FB Control Decks, TP | P1 | P5-Decks+Voice | Local only, pairing + pinned TLS (§7). SAM Panel/Deck Hopper are prior art. |
| 12.08 | Interactive prompts on the phone (Wait for Input/Choice/Multi-choice) | — | — | ✓ | — | P2 | P5-Decks+Voice |  |
| 12.09 | Deck App WS protocol compatibility (port 9470) so Deck Hopper / SAMMI Panel / DeckMate work | — | — | ✓ | — | P3 | P6-Ecosystem | Open Q2; clean-room from public docs. |
| 12.10 | Own Elgato Stream Deck plugin (SDK 3, Node): key down/long-press/up, Action Switch, dials, title/bg/state/value, dynamic action lists from our API | ✓ | — | ✓ | SD | P1 | P4-Integrations+SDK | SB server 8059; requires Node 24 / SD 7.1+ for SDK 3. |
| 12.11 | OpenDeck / OpenAction compatibility on Linux | — | — | — | OpenDeck | P1 | P4-Integrations+SDK | Linux path for Stream Deck users; test our plugin there. |
| 12.12 | Stream Deck MCP "Execute MCP Action" | ✓ | — | — | — | P3 | P4-Integrations+SDK | Superseded by our own MCP server (§24). |
| 12.13 | Direct HID Stream Deck mode | — | — | — | — | P3 | P4-Integrations+SDK | Conflicts if Elgato app holds the device. |
| 12.14 | StreamController (GTK4) native plugin | — | — | — | — | P3 | P4-Integrations+SDK | OpenDeck may suffice. |
| 12.15 | Touch Portal plugin (TCP JSON, actions/states/connectors) | ◐ | — | — | TP | P2 | P4-Integrations+SDK | SB reached only by community plugins. |
| 12.16 | Touch Portal `.tpp` plugin compat adapter | — | — | — | TP | P3 | P6-Ecosystem | Bootstraps ecosystem; owner decision. |
| 12.17 | Loupedeck / Logitech MX Creative Console plugin | ◐ | — | — | — | P3 | P4-Integrations+SDK | Logi Actions SDK (C#). |
| 12.18 | Twitch Extension for viewer-clickable buttons (Bits-funded "Power") | — | — | ✓ | — | P3 | P6-Ecosystem | Needs hosted backend + Twitch extension review (Q6). |
| 12.19 | Web decks hosted for moderators (shared privately) | ✓ | — | ◐ | — | P3 | P6-Ecosystem | SB via website; would need hosted relay. |

## 13. Broadcast app integrations

| ID | Feature | SB | SpB | SAM | Others | Pri | Phase | Notes |
|---|---|---|---|---|---|---|---|---|
| 13.01 | OBS Studio via obs-websocket v5 (4455, SHA-256 auth), multiple connections, LAN host, auto-reconnect | ✓ | — | ✓ | all | P0 | P1-Core | OBS 33 Safe Mode has no WS; `obws` repo archived/moved; `obs-websocket-js` alt. |
| 13.02 | OBS v4.9 legacy protocol | ✓ | — | ✓ | — | P3 | P1-Core | v4 is legacy. |
| 13.03 | OBS core sub-actions (switch scene, source visibility, text/browser URL, mute, start/stop stream/record) | ✓ | — | ✓ | all | P0 | P1-Core |  |
| 13.04 | OBS full control set (filters, transforms, crop, blend, audio tracks, monitor type, create/delete/duplicate source, media, replay buffer, vcam, studio mode, hotkeys, chapters, screenshots) | ✓ | — | ✓ | — | P1 | P1-Core | SAM ~100 OBS commands, SB 29. |
| 13.05 | OBS raw request / batch / vendor request | ✓ | — | ✓ | — | P1 | P1-Core |  |
| 13.06 | OBS events as triggers (any event, vendor events, scene changed, stream/record, connected/disconnected) | ✓ | — | ✓ | — | P0 | P1-Core | High-volume events (meters, transforms) opt-in only. |
| 13.07 | OBS source animation / Motion tweening (position, curve, scale, rotation, crop, volume, alpha, filter settings, easing) | — | — | ✓ | Move plugin | P2 | P4-Integrations+SDK | Q5; run at stream FPS independent of UI focus. |
| 13.08 | OBS plugin helpers (Advanced Scene Switcher message/event, Move transition, Source Record, shaderfilter, Aitum Vertical/Multistream) | — | — | ◐ | AIT | P2 | P4-Integrations+SDK | Via vendor requests; StreamFX unreliable, skip. |
| 13.09 | OBS Canvases (5.7.x) and vertical canvas awareness | — | — | — | AIT | P3 | P4-Integrations+SDK |  |
| 13.10 | Streamlabs Desktop (SockJS WS 59650 token, named pipe `slobs`) | ✓ | — | — | SL | P1 | P4-Integrations+SDK | Windows/macOS only; exact SockJS path unverified. |
| 13.11 | Meld Studio (Qt WebChannel WS 13376, "Allow remote connections") | ✓ | — | ✓ | — | P1 | P4-Integrations+SDK | Win/macOS only. Conflict: platform-apis says protocol undocumented; app-integrations cites `WebChannelAPI.md` in MeldStudio/streamdeck repo. |
| 13.12 | vMix (HTTP 8088 / TCP 8099) | — | — | — | — | P3 | P4-Integrations+SDK |  |
| 13.13 | PRISM Live Studio (unofficial remote protocol) | — | — | — | — | P3 | P4-Integrations+SDK | Subscription shift. |
| 13.14 | XSplit | — | — | — | — | P3 | P4-Integrations+SDK | No documented public remote API. |
| 13.15 | PolyPop (WS plugin 9652; Trigger Alert) | ✓ | — | — | — | P3 | P4-Integrations+SDK | Status unverified. |
| 13.16 | NDI publish (overlay renderer) / tally triggers | — | — | — | — | P3 | P4-Integrations+SDK | NDI runtime licensing check. |
| 13.17 | Twitch Studio | — | — | — | — | ✗ | — | Discontinued 2024-05-30. |

## 14. VTuber integrations

| ID | Feature | SB | SpB | SAM | Others | Pri | Phase | Notes |
|---|---|---|---|---|---|---|---|---|
| 14.01 | VTube Studio connect: WS 8001, UDP discovery 47779, plugin token approval, multi-instance | ✓ | — | ◐ | MIU, T.I.T.S. | P1 ★ | P4-Integrations+SDK | Win/macOS (Linux via Proton, inferred). SAM via ext. |
| 14.02 | VTS actions: trigger hotkey, activate expression, load/move model, colour tint, send raw request | ✓ | — | ◐ | — | P1 ★ | P4-Integrations+SDK | Hotkey queue 32; model-load 2 s cooldown; dropdowns from live lists. |
| 14.03 | VTS items (load/unload/animate/move/pin) and physics/post-processing/NDI config | — | — | ◐ | — | P2 | P4-Integrations+SDK | NDI needs `ControlNDISettings`. |
| 14.04 | VTS parameter injection and custom parameter creation (heart rate, audio level, sentiment) | — | — | — | vts-heartrate | P1 | P4-Integrations+SDK |  |
| 14.05 | VTS Event API triggers (model loaded, tracking status, hotkey triggered, item events, model clicked, animation event, etc.) | ✓ | — | ◐ | — | P1 | P4-Integrations+SDK | SB has 11 triggers; `ModelOutlineEvent` high-rate, throttle. |
| 14.06 | Warudo: companion plugin mod (channel `streamden` on ws://localhost:19190; nodes On Action / Send Event / Variables) | ◐ | — | — | community SB plugin | P1 ★ | P4-Integrations+SDK | Plugin is C#/Unity (Q1 vs owner C# dislike). Without it: VMC only. HTTP callbacks 19052. |
| 14.07 | VNyan: WS receiver (ws://localhost:8000/vnyan), send command text/JSON; VNyan calls our API | — | — | — | — | P1 ★ | P4-Integrations+SDK | Windows; VNyan 1.5.3 broke older plugin WS; version-detect. REST 8069 is third-party report. |
| 14.08 | VSeeFace: VMC Assistant expression injection + global hotkeys; receive VMC status | — | — | — | — | P1 ★ | P4-Integrations+SDK | No API; send/receive ports must differ (wizard check). |
| 14.09 | OpenSeeFace tracker source in routing graph (UDP 11573 inferred) and tracking-lost trigger | — | — | — | — | P1 ★ | P4-Integrations+SDK | Tracker, not a control target; works on Linux. |
| 14.10 | VMC protocol send/receive (OSC UDP 39539/39540; blendshapes, bones, status, calibration) | — | — | — | — | P1 ★ | P4-Integrations+SDK | One protocol covers VSeeFace/Warudo/VNyan/XR Animator. |
| 14.11 | VMC router/multiplexer (fan-out, merge/override blendshapes from triggers, record/replay) | — | — | — | VMCProtocolMultiplexer | P1 | P4-Integrations+SDK | Unique differentiator (Q7: headline?). |
| 14.12 | VRChat OSC preset (avatar params, `/input/*`, `/chatbox/input`, `/avatar/change` trigger, OSCQuery discovery) | — | — | — | — | P1 | P4-Integrations+SDK | In 9000 / out 9001; OSCQuery can report stale values. |
| 14.13 | veadotube mini / veadotube (WS nodes; instance-file discovery; set state, mic toggle; state-changed trigger) | — | — | ◐ | — | P1 | P4-Integrations+SDK | Address randomized per launch; client must send `?n=` name. Cross-platform. |
| 14.14 | T.I.T.S. (throw items, activate trigger by name; item-hit trigger) | ✓ | — | — | MIU | P1 | P4-Integrations+SDK | Port 42069 inferred. 8 triggers + 6 sub-actions in SB. |
| 14.15 | Animaze (WS 9000 JSON actions) | — | — | — | — | P2 | P4-Integrations+SDK | Triggers mostly planned upstream. |
| 14.16 | StreamFog (Remote Key; lenses, outfits, video backgrounds) | ✓ | — | — | — | P2 | P4-Integrations+SDK | SB 1.0.5. |
| 14.17 | mtion studio (process detect; Get Active Clubhouse, Send Trigger) | ✓ | — | — | — | P3 | P4-Integrations+SDK |  |
| 14.18 | Beat Saber status sockets (6557 / 13339) | — | — | — | — | P2 | P4-Integrations+SDK | Ports partly inferred. |
| 14.19 | Inochi Session (tracking/VMC source only) | — | — | — | — | P3 | P4-Integrations+SDK | No control API; "not production ready". |
| 14.20 | PNGTuber Plus / PNGTubeRemix (hotkey presets via keystroke sim) | — | — | — | — | P3 | P4-Integrations+SDK | No API. |
| 14.21 | Tracking sources via VMC apps (XR Animator, iFacialMocap, VTS phone, MeowFace, mocopi) | — | — | — | — | P3 | P4-Integrations+SDK | Routing-graph nodes only. |
| 14.22 | 3tene, Luppet, Kalidoface, ChilloutVR (OSC mod) | — | — | — | — | P3 | P4-Integrations+SDK | Small users / no API; generic OSC covers ChilloutVR. |
| 14.23 | VTuber Setup Wizard: process scan, Steam manifests, UDP/mDNS discovery, protocol-fingerprint port probes, "API disabled" help cards | — | — | — | — | P2 | P4-Integrations+SDK | Probes never send state-changing commands. |
| 14.24 | Pairing walk-through cards (approve popup, press Hue button, paste token, install Warudo plugin / Stream Deck plugin) | — | — | — | — | P2 | P4-Integrations+SDK | Secrets to OS keychain. |
| 14.25 | Tracking/VMC routing graph with port-conflict detection and per-hop "signal OK" meters | — | — | — | — | P2 | P4-Integrations+SDK |  |
| 14.26 | Starter packs generated for detected apps (redeem to VTS hotkey, bits to T.I.T.S., raid to Warudo, HR to param, mic mute to veadotube, tracking-lost to BRB scene, now-playing) | — | — | — | — | P2 | P4-Integrations+SDK |  |
| 14.27 | VTuber rig health dashboard | — | — | — | — | P2 | P4-Integrations+SDK |  |
| 14.28 | Every connector accepts a remote host (split Windows avatar PC / Linux bot PC) | ✓ | — | ✓ | — | P1 | P4-Integrations+SDK | OBS and Speaker.bot already LAN-capable in SB. |
| 14.29 | Unofficial APIs behind an "experimental" flag with version detection | — | — | — | — | P1 | P4-Integrations+SDK | Wave Link, Sonar, PRISM, VNyan REST. Q6. |

## 15. Audio & music

| ID | Feature | SB | SpB | SAM | Others | Pri | Phase | Notes |
|---|---|---|---|---|---|---|---|---|
| 15.01 | Play sound (mp3/wav/m4a/ogg; device; volume; wait to finish; named; stop by name; random from folder) | ✓ | ◐ | ✓ | all | P1 | P1-Core | SAM adds mp4-audio, speed, volume fades. |
| 15.02 | Per-command output device / follow OS default | ✓ | ✓ | ✓ | — | P1 | P1-Core |  |
| 15.03 | Sound fades, speed change | — | — | ✓ | — | P2 | P1-Core |  |
| 15.04 | System master volume/mute, list audio devices | — | — | ✓ | — | P2 | P4-Integrations+SDK | SAM Pro. |
| 15.05 | Per-application volume/mute (WASAPI, PipeWire/Pulse, macOS best-effort) | — | — | ✓ | — | P1 | P4-Integrations+SDK | macOS has no public per-app mixer API (Core Audio taps 14.2+, inferred). SAM Pro. |
| 15.06 | PipeWire virtual sinks/loopbacks (VoiceMeeter-like routing on Linux) | — | — | — | — | P2 | P4-Integrations+SDK |  |
| 15.07 | Voicemod (Control API WS, port list, clientKey; voices, changer, hear-myself, mute, soundboard) | ✓ | — | ◐ | — | P1 | P4-Integrations+SDK | Win/macOS. SAM Voicemod is legacy/"no longer exists" in SAM docs vs "V3 ext" in app doc. |
| 15.08 | VoiceMeeter Remote API | — | — | — | — | P2 | P4-Integrations+SDK | Windows only (DLL). |
| 15.09 | GoXLR Utility (HTTP/WS 14564, JSON-Patch) | — | — | — | — | P2 | P4-Integrations+SDK | Linux showcase; official GoXLR app has no API. |
| 15.10 | Elgato Wave Link 1.x/2.x | ✓ | — | — | — | P2 | P4-Integrations+SDK | SB 16 sub-actions, 20 triggers. |
| 15.11 | Elgato Wave Link 3.x (unofficial WS JSON-RPC) | ◐ | — | — | — | P2 | P4-Integrations+SDK | SB alpha only; protocol fragile, experimental flag. |
| 15.12 | SteelSeries Sonar (unofficial REST, dynamic port) | — | — | — | — | P3 | P4-Integrations+SDK |  |
| 15.13 | Now playing + transport via MPRIS (Linux) and GSMTC (Windows) | — | — | ◐ | — | P1 | P4-Integrations+SDK | Avoids Spotify API limits. |
| 15.14 | macOS MediaRemote adapter | — | — | — | — | P3 | P4-Integrations+SDK | Private API, entitlement-gated since macOS 15.4. |
| 15.15 | Spotify Web API (BYO client ID, PKCE) | — | — | ◐ | LS | P2 | P4-Integrations+SDK | Feb 2026: Dev Mode needs Premium + 5 test users; extended quota needs 250k MAU business. |
| 15.16 | YouTube Music Desktop (ytmdesktop v2, 9863) and Pear Desktop (26538) | — | — | — | — | P2 | P4-Integrations+SDK | Good song-request targets; IPv4 only. |
| 15.17 | VLC HTTP interface | — | — | — | — | P2 | P4-Integrations+SDK | Default 8080 collides with SB WS. |
| 15.18 | foobar2000 (beefweb), Tidal | — | — | ◐ | — | P3 | P4-Integrations+SDK | MPRIS/GSMTC cover most. |
| 15.19 | Song request flow + `!song` command + now-playing overlay | — | — | ◐ | NB, MIU | P2 | P4-Integrations+SDK | Starter pack. |
| 15.20 | Viewer soundboard via channel points | — | — | — | Sound Alerts | P3 | P4-Integrations+SDK |  |

## 16. Hardware, lighting & smart home

| ID | Feature | SB | SpB | SAM | Others | Pri | Phase | Notes |
|---|---|---|---|---|---|---|---|---|
| 16.01 | Elgato Key Light / Light Strip (HTTP 9123; mireds) | ◐ | — | ◐ | — | P1 | P4-Integrations+SDK | SB via Control Center (alpha). Firmware quirks: HTTP/1.0 GET, `Connection: close` PUT. |
| 16.02 | Home Assistant (WS API: auth, call_service, subscribe_events/trigger, get_states; long-lived token) | — | — | — | HA | P1 | P4-Integrations+SDK | Universal smart-home bridge (Hue, Nanoleaf, LIFX, Govee, Tuya, Matter). |
| 16.03 | Philips Hue (CLIP v2 REST/SSE; Entertainment later) | — | — | ◐ | LS | P2 | P4-Integrations+SDK | Link-button pairing. |
| 16.04 | Nanoleaf (OpenAPI 16021) | — | — | — | LS | P2 | P4-Integrations+SDK | Essentials (Thread/Matter) via HA. |
| 16.05 | LIFX (UDP 56700), Govee LAN (UDP multicast), WLED (JSON/WS/UDP) | — | — | ◐ | LS | P2 | P4-Integrations+SDK | Govee needs per-device LAN toggle. |
| 16.06 | Tuya local protocol, DMX (QLC+, Art-Net, sACN) | — | — | ◐ | — | P3 | P4-Integrations+SDK | HA covers Tuya. |
| 16.07 | Lumia Stream (Send Command, Set Color, Set Default; REST 39231 / WS) | ✓ | — | — | LS | P2 | P4-Integrations+SDK | Coexistence; Lumia has own Fuze sync. |
| 16.08 | OpenRGB SDK server (6742) | — | — | — | — | P2 | P4-Integrations+SDK | Cross-platform RGB. |
| 16.09 | Elgato Camera Hub (Facecam, EpocCam, Prompter; 68 triggers, 34 sub-actions) | ✓ | — | — | — | P3 | P4-Integrations+SDK | Undocumented local channel; Linux: V4L2 for UVC cams (inferred). |
| 16.10 | Elgato Studio (alpha) | ✓ | — | — | — | P3 | P4-Integrations+SDK |  |
| 16.11 | OBSBOT (gimbal, zoom, sleep, snapshot) | ✓ | — | — | — | P3 | P4-Integrations+SDK | SB manifest only, undocumented. |
| 16.12 | Thermal printer / QR / OCR style novelty hardware | — | — | ◐ | — | P3 | P4-Integrations+SDK | SAM community extensions only. |

## 17. Donation & commerce services

| ID | Feature | SB | SpB | SAM | Others | Pri | Phase | Notes |
|---|---|---|---|---|---|---|---|---|
| 17.01 | StreamElements tips/merch/activities (outbound socket; JWT or overlay token) | ✓ | ✓ | ◐ | SE | P1 | P4-Integrations+SDK | Prefer Astro WS `astro.streamelements.com`; SB still uses legacy socket.io; JWT reportedly rotates ~2 weeks (community). Paste token, avoid OAuth. |
| 17.02 | Streamlabs donations/merch/charity/YouTube-sub follow (Socket API token) | ✓ | ✓ | ◐ | SL | P1 | P4-Integrations+SDK | SB/SpB moved to OAuth (0.2.5/0.1.5); platform research says paste socket token (OAuth needs approval and secret). |
| 17.03 | TipeeeStream (Socket.IO, API key) | ✓ | — | — | — | P2 | P4-Integrations+SDK | SB guide WIP. |
| 17.04 | TreatStream | ✓ | — | — | — | P3 | P4-Integrations+SDK | SB guide WIP. |
| 17.05 | Tiltify (v5) | ◐ | — | — | — | P2 | P4-Integrations+SDK | SB binary has OAuth service but no docs/triggers. |
| 17.06 | DonorDrive / Extra Life (public REST polling, ETag) | ✓ | — | — | — | P2 | P4-Integrations+SDK | No auth; custom endpoint option. |
| 17.07 | Pally.gg (WS firehose, API key) | ✓ | — | — | — | P3 | P4-Integrations+SDK |  |
| 17.08 | StreamLoots (SSE via alerts-widget URL) | ✓ | — | — | — | P3 | P4-Integrations+SDK |  |
| 17.09 | Ko-fi (webhook only; verification token) | ✓ | — | ◐ | — | P1 | P4-Integrations+SDK | Needs public HTTPS: self-hostable relay (P2, §23) or tunnel. SAM ext uses author's relay. |
| 17.10 | Patreon (webhooks HMAC-MD5 or local polling with creator token) | ✓ | — | ◐ | — | P2 | P4-Integrations+SDK | Polling works locally (100 req/min). |
| 17.11 | Fourthwall (13 event types; HMAC-SHA256 webhooks) | ✓ | — | — | — | P2 | P4-Integrations+SDK | Relay or tunnel; 5 delivery attempts, 5 s timeout. |
| 17.12 | Throne (Ed25519 webhooks; gifts, contributions, crowdfunded) | ✓ | — | — | — | P2 | P4-Integrations+SDK | Relay or tunnel. |
| 17.13 | Shopify orders (webhook) | ✓ | — | — | — | P3 | P4-Integrations+SDK |  |
| 17.14 | Buy Me a Coffee (HMAC webhooks) | — | — | — | — | P3 | P4-Integrations+SDK | Community-sourced details. |
| 17.15 | Amount normalization to USD (`amount.usd`) for conditions, labels, "largest this stream" | — | — | — | SL, SE | P1 | P3-Designer | FX-rate source undefined (inferred). |

## 18. Game, interactive & peer apps

| ID | Feature | SB | SpB | SAM | Others | Pri | Phase | Notes |
|---|---|---|---|---|---|---|---|---|
| 18.01 | Crowd Control (PubSub WS via overlay URL; session, effect request/success/failure, timed effects, coin exchange) | ✓ | — | ◐ | MIU | P2 | P4-Integrations+SDK | Production access needs CC approval; SAM ext marked legacy. |
| 18.02 | Pulsoid heart rate (OAuth device flow, realtime WS) | ✓ | — | ◐ | — | P1 | P4-Integrations+SDK | SAM trigger marked legacy. Throttle/threshold triggers. |
| 18.03 | HypeRate.io heart rate (+ clip-created trigger) | ✓ | — | — | — | P1 | P4-Integrations+SDK | `internal-testing` ID gives fake data. |
| 18.04 | Heart-rate recipes (HR to VTS parameter, HR to VRChat OSC parameter, HR overlay) | — | — | — | vts-heartrate | P2 | P4-Integrations+SDK |  |
| 18.05 | Minecraft Java RCON; Source-engine RCON (generic RCON sub-action) | — | — | — | — | P2 | P4-Integrations+SDK | TCP 25575 / 27015 inferred. |
| 18.06 | Minecraft Bedrock `/connect` WebSocket events | — | — | — | — | P3 | P4-Integrations+SDK |  |
| 18.07 | LiveSplit server | — | — | ◐ | — | P3 | P4-Integrations+SDK |  |
| 18.08 | Streamer.bot coexistence client (run SB actions, mirror SB events, import action list) | — | — | — | — | P1 | P4-Integrations+SDK | Tier-1 migration aid. |
| 18.09 | SAMMI / Firebot / Mix It Up / Lumia peer clients | — | — | ◐ | — | P2 | P4-Integrations+SDK | SAM already calls Mix It Up (REST 8911) and Aitum. |
| 18.10 | Aitum request templates (31) | — | — | ✓ | AIT | P3 | P4-Integrations+SDK |  |
| 18.11 | Streamlabs Spin Wheel trigger | ✓ | — | — | — | P3 | P4-Integrations+SDK |  |
| 18.12 | Overwolf | — | — | — | — | ✗ | — | Walled runtime. |

## 19. Generic primitives

| ID | Feature | SB | SpB | SAM | Others | Pri | Phase | Notes |
|---|---|---|---|---|---|---|---|---|
| 19.01 | HTTP request sub-action (GET/POST/PUT/DELETE, headers, body templating, JSON-path to variables, download/upload file) | ◐ | — | ✓ | — | P0 | P1-Core | SB Fetch URL is GET only. Needed for X posting and most REST plugins. |
| 19.02 | OSC send/receive (typed args, address patterns, bundles, OSCQuery/mDNS) | — | — | — | — | P1 | P4-Integrations+SDK | Basis for VRChat and VMC. |
| 19.03 | MIDI in (note/CC/PC/SysEx, learn mode) and MIDI out (notes, CC, LED feedback), virtual ports | ✓ | — | ◐ | TP | P1 | P4-Integrations+SDK | Windows needs loopMIDI / MIDI Services. SAM via "MIDI to SAMMI" ext. |
| 19.04 | MQTT client (+ optional embedded broker) | — | — | — | HA | P2 | P4-Integrations+SDK |  |
| 19.05 | UDP/TCP raw send and receive | ◐ | — | ✓ | — | P2 | P4-Integrations+SDK | SB UDP Broadcast only; SAM Send UDP Packet. |
| 19.06 | Named pipes / Unix sockets | — | — | — | — | P3 | P4-Integrations+SDK |  |
| 19.07 | Serial port | — | — | — | — | P3 | P4-Integrations+SDK | Linux `dialout` group. |
| 19.08 | File I/O (read lines/random/specific, write/append, exists, copy, delete, folder list, zip/unzip) | ✓ | — | ✓ | — | P1 | P1-Core |  |
| 19.09 | INI / CSV / binary-buffer file helpers, leaderboard from INI | — | — | ✓ | — | P2 | P1-Core | JSON/SQLite storage covers most. |
| 19.10 | File/folder watcher and file tail triggers | ✓ | — | — | — | P1 | P4-Integrations+SDK |  |
| 19.11 | Global hotkey trigger (any combo) | ✓ | — | ✓ | — | P1 | P4-Integrations+SDK | Win `RegisterHotKey`, macOS Input Monitoring, X11 XGrabKey, Wayland GlobalShortcuts portal. |
| 19.12 | CLI `streamden trigger <name>` for compositor key binds | — | — | — | — | P1 | P4-Integrations+SDK | Reliable Hyprland fallback (`bind = ... exec`). |
| 19.13 | Keystroke simulation | ✓ | — | ✓ | — | P1 | P4-Integrations+SDK | Wayland: no global injection; portal RemoteDesktop + libei, wlroots virtual keyboard, or ydotool/uinput (udev rule). macOS needs Accessibility. |
| 19.14 | Mouse simulation (move/click) and mouse-click trigger | ◐ | — | ✓ | — | P2 | P4-Integrations+SDK | SB has trigger only. |
| 19.15 | Game controller / gamepad triggers (buttons, sticks, analog, hat) | ✓ | — | ✓ | — | P2 | P4-Integrations+SDK | SB SDL2 in 1.1.0. |
| 19.16 | Window control (find, focus, move, resize, always-on-top, close, get active) | — | — | ✓ | — | P2 | P4-Integrations+SDK | SAM Pro, Windows. Wayland limits (inferred). |
| 19.17 | Process control (started/stopped triggers; is-running, list, kill) | ◐ | — | ✓ | — | P1 | P4-Integrations+SDK | SB triggers only. |
| 19.18 | Clipboard changed trigger; get/set | ✓ | — | ✓ | — | P2 | P4-Integrations+SDK |  |
| 19.19 | D-Bus (Linux) signal trigger / method call | — | — | — | — | P2 | P4-Integrations+SDK | MPRIS, notifications, portals. |
| 19.20 | AppleScript/JXA (macOS), PowerShell | — | — | ◐ | — | P3 | P4-Integrations+SDK | Via Run a Program. |
| 19.21 | mDNS/SSDP device discovery trigger | — | — | — | HA | P2 | P4-Integrations+SDK | Feeds wizard and plugin discovery. |
| 19.22 | IFTTT bridge (execute action in; notification out) | ✓ | — | — | — | P3 | P6-Ecosystem | SB's only route to post on X; needs cloud relay. |

## 20. Data

| ID | Feature | SB | SpB | SAM | Others | Pri | Phase | Notes |
|---|---|---|---|---|---|---|---|---|
| 20.01 | Users database (per-platform records, stats: watch time, raids, points spent, Kicks gifted, super stickers; last active; present) | ✓ | ◐ | — | MIU, FB | P1 | P2-Chat+TTS | SpB `users.dat` holds voices/flags only. |
| 20.02 | Cross-platform identity merge (link Twitch/YouTube/Kick accounts into one person) | — | — | — | — | P1 | P2-Chat+TTS | SB is per-platform only; design natively (engine Q7). Linking method not researched. |
| 20.03 | User groups (custom, platform-aware; add/remove/in-group sub-actions; group triggers) | ✓ | ◐ | — | — | P1 | P2-Chat+TTS | Used by command permissions and random picks. |
| 20.04 | Counters (global/user/per-reward; inc/dec/reset; overlay binding) | ✓ | — | ◐ | NB, SE | P1 | P2-Chat+TTS |  |
| 20.05 | Points/currency definitions with per-platform balances; award/spend/transfer/set/reset | ✓ | — | ◐ | MIU, FB | P1 | P5-Decks+Voice | SB 1.1.0 alpha (18 sub-actions); SAM via Loyalty ext. |
| 20.06 | Points multipliers (user/group) and payout "ticks" (active-users-only) | ✓ | — | — | — | P1 | P5-Decks+Voice |  |
| 20.07 | Leaderboards (points, rank, cutoff) | ✓ | — | ◐ | MIU | P1 | P5-Decks+Voice |  |
| 20.08 | Loyalty commands (`!points`, gambling, shop) | ◐ | — | ◐ | MIU, NB | P2 | P5-Decks+Voice | Starter pack; SB via community extension. |
| 20.09 | Quotes (add/get/delete/count; triggers; user, game, time, platform) | ✓ | — | ◐ | NB | P2 | P5-Decks+Voice | SB replaced built-in `!quote` with sub-actions in 1.0. |
| 20.10 | Giveaways (entries, roles, draw) | — | — | ◐ | MIU, SSN | P2 | P5-Decks+Voice | SAM "Giveaway" ext. |
| 20.11 | Credits service (record events, present users, top cheer/points users; auto-reset; API) | ✓ | — | — | MIU | P2 | P3-Designer | Feeds End Credits element. |
| 20.12 | Session stats store (latest follower, top donor, largest tip, counters) | ◐ | — | — | SE | P1 | P3-Designer | Labels/goals bind here. |
| 20.13 | Single embedded database with human-readable export | ◐ | ◐ | ◐ | — | P0 | P0-Foundations | SB mixes JSON + LiteDB; SQLite recommended (engine Q8). Atomic saves. |
| 20.14 | Data retention + revocation handling (Twitch 24 h caching, YouTube 30 d refresh/delete), privacy policy | — | — | — | — | P1 | P2-Chat+TTS | Required by platform terms. |

## 21. Import & compatibility

| ID | Feature | SB | SpB | SAM | Others | Pri | Phase | Notes |
|---|---|---|---|---|---|---|---|---|
| 21.01 | Streamer.bot share-string import (`SBAE` + gzip JSON, schema v23; legacy flat actions; numeric type-id map) | ✓ | — | — | — | P1 | P6-Ecosystem | Per-item include/overwrite preview; OBS refs re-matched by name; imported commands disabled by default. |
| 21.02 | Import of SB `Execute C# Code` sub-actions | ✓ | — | — | — | P2 | P6-Ecosystem | Options: skip, port to JS/Python, optional .NET sidecar (Q2). |
| 21.03 | Streamer.bot WebSocket API compatibility shim (Hello/Authenticate/Subscribe/DoAction/GetActions/GetGlobals, event names, `General.Custom`, `Raw.*`) | ✓ | — | — | — | P2 | P6-Ecosystem | Lets `@streamerbot/client`, SB docks, Toolkit, TikFinity work unchanged (Q1). |
| 21.04 | SB HTTP `/api/DoAction` and UDP DoAction compat | ✓ | — | — | — | P2 | P6-Ecosystem |  |
| 21.05 | CPH-compatible script facade | ✓ | — | — | — | P2 | P6-Ecosystem | See 6.13. |
| 21.06 | Speaker.bot data import (`settings.json`, `voicealiases.dat`, `users.dat` LiteDB) | — | ✓ | — | OpenSpk | P2 | P6-Ecosystem | Q3. |
| 21.07 | Speaker.bot WS 7580 / UDP 6669 API shim (Speak, Stop, Pause, Mode, GetInfo, Subscribe) | — | ✓ | — | OpenSpk | P2 | P6-Ecosystem | `7680` appears in SpB docs (typo, inferred). |
| 21.08 | SAMMI deck/button JSON import (BGR colours, 0-1 positions, command mapping) | — | — | ✓ | — | P2 | P6-Ecosystem | Clean-room only: SAMMI licence signals conflict. |
| 21.09 | SAMMI Local API (9450) + `/webhook` compat shim | — | — | ✓ | — | P3 | P6-Ecosystem | Q2. |
| 21.10 | SAMMI `.sef` extension runtime (sandboxed browser) | — | — | ✓ | — | P3 | P6-Ecosystem | Or not at all (Q3). |
| 21.11 | Firebot / Mix It Up import | — | — | — | — | P3 | P6-Ecosystem | Not researched. |
| 21.12 | SB-format export from StreamDen | — | — | — | — | P3 | P6-Ecosystem | (inferred) low demand. |

## 22. Plugin SDK & ecosystem

| ID | Feature | SB | SpB | SAM | Others | Pri | Phase | Notes |
|---|---|---|---|---|---|---|---|---|
| 22.01 | Contribution registry + `PluginBackend` trait; built-ins use the same manifest format; ≥2 first-party plugins ship externally | — | — | — | VSC | P0 ★ | P0-Foundations | SB exposes internals as C# API instead; dogfooding keeps SDK honest. (SB column: no equivalent.) |
| 22.02 | Codegen pipeline: Rust types to `protocol.json` to OpenAPI/AsyncAPI/TS/Python SDKs; CI drift check | — | — | — | OBS | P0 ★ | P0-Foundations | Source-of-truth language depends on core stack (open). |
| 22.03 | Manifest `plugin.json` v1 (id, slug, engines app + pluginApi, permissions, activation, l10n, contributes, `x-ui`) | — | — | ◐ | VSC, LS, TP | P1 ★ | P4-Integrations+SDK | SAM `.sef` is the cautionary text format. |
| 22.04 | Tier D declarative plugins (HTTP/WS/OSC/MIDI/UDP mappings, JSONPath, discovery) | — | — | — | TP, LS | P1 ★ | P4-Integrations+SDK | Safest tier; covers WLED, Ko-fi style. |
| 22.05 | Tier S sidecar plugins (JSON-RPC 2.0 stdio + WS attach; TS and Python SDKs) | — | — | — | LS, SD | P1 ★ | P4-Integrations+SDK | "Full system access" label; Verified-channel only in registry. |
| 22.06 | Tier J embedded JS/TS plugins (isolate per plugin in exthost process; brokered ops) | — | — | ◐ | — | P1 ★ | P4-Integrations+SDK | No Node built-ins. SAM Bridge JS is shared-page precedent. |
| 22.07 | Tier W WASM Component plugins (wasmtime, WASI 0.3, WIT world) | — | — | — | Extism | P2 | P6-Ecosystem | Preview in v1.x; recommended marketplace tier in v2. |
| 22.08 | Contribution types: triggers (payload schema, filters, test payload), sub-actions (params, outputs, sentence, timeout), conditions, variables, typed requests | ◐ | — | ◐ | TP, LS, VSC | P1 ★ | P4-Integrations+SDK | SB custom triggers registered only from C# at runtime. |
| 22.09 | Contribution types: chatSources, ttsVoices, overlayWidgets, panels, commands, chatCommands, templates | — | — | ◐ | LS | P1 | P4-Integrations+SDK | SAM `insert_over` ships decks with extension. |
| 22.10 | Schema-generated forms using the app's own picker components (OBS scene/source, audio device, variable, reward, user, voice, account) | — | — | ◐ | SD sdpi, LS | P1 ★ | P4-Integrations+SDK | SAM `extCommand` box types are the model; SB extensions have no settings UI. |
| 22.11 | App UI kit as web components + CSS tokens and theme sync for plugin panels (sandboxed iframes, no Tauri IPC) | — | — | — | VSC | P1 | P4-Integrations+SDK | Keep kit identical to the app's (VS Code toolkit archived). |
| 22.12 | Connection model (config-flow steps, OAuth2 loopback, multiple accounts, mDNS/process discovery, health + reconnect policy) | — | — | — | HA | P1 | P4-Integrations+SDK |  |
| 22.13 | Permission model (network allow/deny, FS, audio, input, process, platform scopes proxied; tokens never shared; per-plugin keychain secrets; consent on escalation) | — | — | — | Tauri, Extism | P1 ★ | P4-Integrations+SDK | SB/SAM/Firebot have none. |
| 22.14 | Plugin supervisor (spawn, health, restart, limits, logs, crash-loop disable, CPU/mem meters) | — | — | — | VSC | P1 | P4-Integrations+SDK |  |
| 22.15 | Package `.slplugin` (zip, `SHA256SUMS`, Sigstore/minisign signature, per-target builds) | ◐ | — | ◐ | — | P1 | P4-Integrations+SDK | SB `.sb` strings / SAM `.sef` are unversioned. |
| 22.16 | `den` CLI (create, dev with hot reload, validate, pack, sign, publish, logs) and project templates | — | — | — | SD CLI, LS | P1 | P4-Integrations+SDK |  |
| 22.17 | Developer panel (event simulator, protocol inspector, debugger attach), test harness, conformance suite | — | — | ◐ | — | P1 | P4-Integrations+SDK | SAM Bridge test panels. |
| 22.18 | Docs site generated from `protocol.json`, example-plugin monorepo, AI-assistant skills/`AGENTS.md` in templates | — | — | — | LS | P2 | P4-Integrations+SDK |  |
| 22.19 | Plugin API versioning (`pluginApi` caret range, support current and previous major) | — | — | — | VSC, OBS | P1 | P4-Integrations+SDK |  |
| 22.20 | Sideload / Developer mode with unsigned warning | — | — | ◐ | — | P1 | P4-Integrations+SDK |  |
| 22.21 | GitHub-PR pointer registry (`plugins/<id>.json`, manually curated first, same file format as later) | ◐ | — | ◐ | Obsidian, HACS | P1 ★ | P6-Ecosystem | SB extensions site / SAM Bridge PR index are manual precedents. |
| 22.22 | Registry CI automation (manifest validation, release asset + hash, signature/attestation, licence, permission-summary PR comment, static scan), signed `index.json` | — | — | — | HACS | P2 | P6-Ecosystem | Owner calls registry "future". |
| 22.23 | Update hold on permission escalation/tier change; blocklist kill switch | — | — | — | VSC, Obsidian | P2 | P6-Ecosystem |  |
| 22.24 | In-app plugin browser (search, categories, README, permission summary, compat filter, install counts) | ◐ | — | ◐ | FB 5.67 | P1 | P6-Ecosystem |  |
| 22.25 | Registry channels (Community vs Verified), governance, takedown | — | — | ◐ | — | P2 | P6-Ecosystem | SAM policy: restricted marking. |
| 22.26 | Plugin quality badges (tests, localized, sandboxed, verified build) | — | — | — | HA | P3 | P6-Ecosystem |  |
| 22.27 | OS hardening for sidecars (Landlock/seccomp, AppContainer, App Sandbox) | — | — | — | — | P3 | P6-Ecosystem |  |
| 22.28 | Paid/DRM plugins | — | — | ◐ | Stream Deck | P3 | P6-Ecosystem | Some SAM extensions sold on itch/Gumroad; Q5. |

## 23. App UX & ops

| ID | Feature | SB | SpB | SAM | Others | Pri | Phase | Notes |
|---|---|---|---|---|---|---|---|---|
| 23.01 | Cross-platform desktop shell (Win/Linux/macOS) | — | — | — | FB, LS | P0 ★ | P0-Foundations | SB/SpB/SAM are Windows-only. Shell undecided: Tauri 2 (plugin-SDK assumption) vs Electron (Chromium parity with OBS CEF). |
| 23.02 | Linux first-class: CI builds, runs on Hyprland/Wayland, portals for hotkeys/input | — | — | — | FB | P0 ★ | P0-Foundations | SB works only under Wine; SAM unsupported. |
| 23.03 | AUR package | — | — | — | — | P0 ★ | P0-Foundations | Owner on Arch/Omarchy. |
| 23.04 | deb and rpm packages | — | — | — | — | P1 ★ | P6-Ecosystem |  |
| 23.05 | macOS support (permissions: Accessibility, Input Monitoring, Automation) | — | — | — | FB, LS | P1 | P6-Ecosystem | Research calls macOS "maybe" (Q2 in app-integrations). Builds in CI from P0. |
| 23.06 | Windows installer + portable mode (data folder next to exe) | ✓ | ✓ | ✓ | — | P1 | P6-Ecosystem | SB/SpB/SAM are portable zips; avoid OneDrive-synced folders. |
| 23.07 | Secrets in OS keychain (Secret Service / Keychain / Credential Manager); never exported in plaintext | ◐ | ◐ | ◐ | — | P0 | P0-Foundations | SB keeps tokens in `auth.db`. |
| 23.08 | Atomic saves / corruption-safe writes; autosave | ✓ | ✓ | ✓ | — | P0 | P0-Foundations | SB autosaves every 5 min + Ctrl+S, "new save routines" in 1.0.5. |
| 23.09 | Logging (levels, rolling files, log viewer filter by source/plugin, redaction), Event Log with payload copy | ✓ | ✓ | ✓ | — | P0 | P1-Core | Serilog in SB; SAM Ctrl+E log. |
| 23.10 | Connections/health page (status pills, last error, reconnect, account avatar) | ◐ | ◐ | ◐ | — | P1 | P1-Core | SAM bottom connection lamps. |
| 23.11 | Themes: dark/light, token-based | ✓ | — | ◐ | — | P1 | P1-Core |  |
| 23.12 | Theme editor (per-window colours, import/export) | — | — | ✓ | — | P2 | P1-Core | SAM Pro UI Color Menu. |
| 23.13 | i18n framework (externalized strings from day one) | — | — | ✓ | — | P1 | P1-Core | SB English only. |
| 23.14 | Community translations (SAM has 9 languages) | — | — | ✓ | — | P2 | P6-Ecosystem |  |
| 23.15 | Backups: auto on start and before update, manual, restore, configurable folder | ✓ | ✓ | ◐ | — | P1 | P1-Core | SB `[date]-[time]-[version]` zips. |
| 23.16 | Cloud backups (encrypted, optional token file) | ✓ | — | — | — | P3 | P6-Ecosystem | SB supporter perk; needs hosted service. |
| 23.17 | Auto-update with channels (stable/beta/alpha), SHA-256 verify, rollback guidance | ✓ | ◐ | ✓ | — | P1 | P1-Core | Package-manager installs should defer to apt/pacman (inferred). SAM has mandatory version check. |
| 23.18 | Headless mode (no UI) with control CLI | ✓ | ✓ | — | — | P1 | P1-Core | SB `/headless`, named pipes IPC; useful for split rigs. |
| 23.19 | Tray icon, minimize to tray, close confirmation, remembered window position | ✓ | ✓ | — | — | P1 | P1-Core |  |
| 23.20 | Startup CLI switches (headless, shutdown, no-login equivalents, reset window) | ✓ | — | — | — | P2 | P1-Core |  |
| 23.21 | Keyboard shortcuts map | ✓ | — | ✓ | — | P2 | P1-Core |  |
| 23.22 | First-run onboarding (connect platform + OBS, starter templates) | ◐ | ◐ | — | — | P1 | P1-Core | (inferred) generic wizard; VTuber wizard in §14. |
| 23.23 | Profiles/settings export-import | ◐ | — | ◐ | — | P1 | P1-Core |  |
| 23.24 | Multi-instance operation (several instances; remote triggers/signals between them) | ✓ | — | — | AIT | P2 | P6-Ecosystem | SB routes via cloud; ours LAN-pairing first, relay optional. |
| 23.25 | All UIs (chat dock, deck, overlay editor) served locally; no dependency on vendor-hosted pages | — | — | ◐ | — | P0 | P0-Foundations | SB `chat.streamer.bot` is cloud-hosted; owner wants no mystery cloud routing. |
| 23.26 | Self-hostable open-source relay: webhook mailbox per user, Kick demux by `broadcaster_user_id`, token broker for secret-requiring OAuth (Kick first), offline buffering, desktop verifies signatures | ◐ | — | ◐ | SSN, SAM Ko-fi ext | P1 | P2-Chat+TTS | Moved to P2 by owner input. Reference design: Cloudflare Worker + Durable Object; also ship plain Docker/Rust implementation. Tokens never stored on relay. |
| 23.27 | Optional hosted relay operated by the project (opt-in per integration, privacy policy, abuse handling, Kick app verification) | ◐ | — | — | — | P2 | P6-Ecosystem | Open owner decision on hosting/cost. |
| 23.28 | Bring-your-own tunnel helper (Tailscale Funnel, ngrok, Cloudflare named tunnel, zrok) for local HTTPS webhook listener | — | — | — | — | P1 | P2-Chat+TTS | Cloudflare Quick Tunnels change URL every run (poor). |
| 23.29 | Optional E2E encryption of relayed payloads to the desktop's public key | — | — | — | — | P2 | P6-Ecosystem |  |
| 23.30 | Crash/error telemetry, opt-in only | ◐ | — | ◐ | — | P2 | P6-Ecosystem | SB bundles Sentry; SAM analytics default on. Ours off by default. |
| 23.31 | Clean-room policy and app licence decision | — | — | — | — | P0 | P0-Foundations | SB/SpB closed; SAM licence signals conflict; SSN/Firebot GPL-3.0; Kruiz no reuse. |
| 23.32 | Supporter tier / account system | ✓ | ✓ | ✓ | — | P3 | P6-Ecosystem | Patreon-gated perks in SB and SAM Pro; monetization undecided. |
| 23.33 | Live/focus mode (suppress pairing prompts while streaming) | — | — | — | — | P3 | P4-Integrations+SDK |  |
| 23.34 | Resource monitor (CPU, memory, event rate per plugin) | — | — | — | VSC | P2 | P4-Integrations+SDK |  |

## 24. AI & MCP

Added from owner input after the initial brief. MCP and AI features land in `P4-Integrations+SDK`.

| ID | Feature | SB | SpB | SAM | Others | Pri | Phase | Notes |
|---|---|---|---|---|---|---|---|---|
| 24.01 | Built-in MCP server exposing all app functions (curated core tools, every sub-action as a JSON-Schema tool with per-plugin opt-in, resources for action catalog/variables/events/logs, destructive tools need in-app approval) | ◐ | — | — | — | P1 | P4-Integrations+SDK | SB has only Stream Deck MCP. Spec 2026-07-28 stateless; Rust `rmcp` / TS SDK. Transport in 7.21. |
| 24.02 | In-app AI setup assistant that creates/edits actions through the app API | — | — | — | — | P1 | P4-Integrations+SDK | Uses same scoped API as any client; show diff and confirmation before applying (inferred). |
| 24.03 | AI chat responder / co-host sub-action | — | — | ◐ | SSN (Ollama), LS | P2 | P4-Integrations+SDK | SAM via "ChatGPT" ext. Respect filter profiles and rate limits. |
| 24.04 | Chat summarization and moderation assist | — | — | — | SSN | P2 | P5-Decks+Voice | SSN has sentiment filter (`badkarma`) as precedent. |
| 24.05 | AI provider support: OpenAI, Anthropic, xAI Grok via API keys; OpenAI-compatible local endpoints (Ollama, LM Studio) | — | — | ◐ | SSN, LS | P1 | P4-Integrations+SDK | Keys in OS keychain. Direct use of a Plus/Pro/Max subscription inside the app: pending compliance question, see QUESTIONS.md. |
| 24.06 | "Connect your AI" screen: set up Claude Desktop / Claude Code, Codex, ChatGPT, Grok or Cursor to our MCP server (generated config, scoped token) so subscription holders get value | — | — | — | — | P1 | P4-Integrations+SDK | Goes through MCP, not the subscription itself. Subscription use inside our app: see QUESTIONS.md. |
| 24.07 | MCP client sub-action ("Call MCP tool") turning any MCP server into a plugin | — | — | — | — | P3 | P4-Integrations+SDK | v2 idea in plugin-SDK doc. |
| 24.08 | MCP Apps (render plugin panels in AI clients) | — | — | — | — | P3 | P4-Integrations+SDK | Official MCP extension; v2. |

## Research conflicts & unclassified items

Conflicts between the research files (resolve before implementing the affected rows):

- **Speaker.bot WS port:** 7580 (SB docs, old wiki) vs 7680 (Speaker.bot docs, probably a typo). The app-integrations doc also proposes our own default ports in 7580-7589, which collides with Speaker.bot's 7580 (row 7.22).
- **Meld Studio protocol:** platform-apis says undocumented/unverified; app-integrations and speakerbot-sammi cite a documented Qt WebChannel API on fixed port 13376 (row 13.11).
- **SAMMI native integrations:** speakerbot-sammi marks Voicemod, Crowd Control and Pulsoid as legacy/"no longer exists"; app-integrations lists them as SAMMI-supported (rows 15.07, 18.01, 18.02).
- **YouTube deleted messages:** platform-apis says delete/retract events were removed from the API (2026-06-23); the designer doc's sync design relies on `tombstone` and `userBannedEvent` (rows 1.53, 8.18).
- **Template syntax:** Streamer.bot `%var%`, designer `{{path}}`, plugin SDK `{slug.trigger.field}` (rows 3.02, 9.23).
- **Core stack:** plugin-SDK doc assumes Rust + Tauri 2; designer doc leaves Electron vs Tauri open (WebKitGTK preview is not pixel-identical to OBS CEF); platform-apis leaves Rust vs TS open (rows 9.02, 23.01).
- **Streamlabs auth:** SB/SpB switched to OAuth; platform-apis recommends the pasted socket token because OAuth needs app approval and a secret (row 17.02).
- **Twitch chat transport:** SAMMI still reads chat over IRC; SB (1.0.5) and Speaker.bot (0.1.6) are EventSub-only (row 1.04).
- **Counts:** SB sub-actions 421 (engine doc, docs pages) vs 462 (triggers doc, manifest); triggers 454 vs 467; WS events 513 (manifest) vs 464 (client constants). Docs lag the app; use the manifest.
- **Phase definitions vs owner needs:** chat overlay is P2 but the designer is P3, so overlay is split into a default-theme overlay (8.05) and a designer-driven one (8.06). Owner input moved MCP to P4 and the self-hostable relay to P2.

Items I could not classify confidently (priority is a judgement call):

- 1.54 YouTube InnerTube scraping (P3 plugin-only vs ✗: policy violation), 1.64 TikTok connector (P3 vs ✗: ToS), 1.62 Kick Pusher fallback (staff discourage).
- 14.11 VMC router (P1 as research Tier 1 vs P2 differentiator), 14.06 Warudo C# shim (owner dislikes C#).
- 9.39 StreamElements widget shim (P2 vs P1 ecosystem wedge), 6.12 C# execute-code (P3 vs ✗), 12.18 Twitch Extension viewer buttons (P3).
- 21.03 SB WS-API compat shim (P2 pending owner Q1), 23.05 macOS scope ("maybe" in research).
- Not in matrix: "Karasubot" (no public source found), Velora/VPZone platforms (MIU lists, unresearched), Firebot/MIU import (not researched).
- Rows marked "(inferred)" or "to verify" need confirmation before work starts.

## Coverage summary

Row counts by priority per section (`★` = owner must-have rows, already included in their P0/P1 counts). Regenerate after edits.

| Section | P0 | P1 | P2 | P3 | ✗ | Total | ★ |
|---|---|---|---|---|---|---|---|
| 1. Platforms | 12 | 30 | 22 | 11 | 9 | 84 | 9 |
| 2. Core automation engine | 13 | 24 | 14 | 2 | 0 | 53 | 0 |
| 3. Variables & expressions | 8 | 11 | 4 | 0 | 0 | 23 | 0 |
| 4. Commands | 5 | 4 | 4 | 2 | 0 | 15 | 0 |
| 5. Timers & scheduling | 1 | 5 | 1 | 0 | 0 | 7 | 0 |
| 6. Code execution | 3 | 6 | 3 | 1 | 0 | 13 | 3 |
| 7. Servers & public API | 6 | 13 | 3 | 0 | 0 | 22 | 6 |
| 8. Multichat | 1 | 26 | 12 | 2 | 0 | 41 | 15 |
| 9. Overlays, alerts & designer | 1 | 30 | 15 | 5 | 0 | 51 | 15 |
| 10. TTS | 0 | 26 | 10 | 4 | 0 | 40 | 0 |
| 11. STT & voice commands | 0 | 2 | 3 | 3 | 0 | 8 | 0 |
| 12. Decks & remote control | 0 | 8 | 3 | 8 | 0 | 19 | 0 |
| 13. Broadcast app integrations | 3 | 4 | 2 | 7 | 1 | 17 | 0 |
| 14. VTuber integrations | 0 | 15 | 9 | 5 | 0 | 29 | 7 |
| 15. Audio & music | 0 | 5 | 11 | 4 | 0 | 20 | 0 |
| 16. Hardware, lighting & smart home | 0 | 2 | 5 | 5 | 0 | 12 | 0 |
| 17. Donation & commerce services | 0 | 4 | 6 | 5 | 0 | 15 | 0 |
| 18. Game, interactive & peer apps | 0 | 3 | 4 | 4 | 1 | 12 | 0 |
| 19. Generic primitives | 1 | 8 | 9 | 4 | 0 | 22 | 0 |
| 20. Data | 1 | 9 | 4 | 0 | 0 | 14 | 0 |
| 21. Import & compatibility | 0 | 1 | 7 | 4 | 0 | 12 | 0 |
| 22. Plugin SDK & ecosystem | 2 | 18 | 5 | 3 | 0 | 28 | 10 |
| 23. App UX & ops | 8 | 14 | 9 | 3 | 0 | 34 | 4 |
| 24. AI & MCP | 0 | 4 | 2 | 2 | 0 | 8 | 0 |
| **Total** | **65** | **272** | **167** | **84** | **11** | **599** | **69** |
