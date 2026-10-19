# Platform API Research: Streamer.live

Research date: **2026-10-18**. Updated **2026-10-19** (DEN-54): Rumble chat (8.1) and the chat send capability matrix (14). Scope: what a local-first, cross-platform desktop streaming automation app (Streamer.bot / SAMMI class, with a combined multichat shown as an OBS dock and an overlay, plus alerts and automations) can do against each platform's **current** APIs. It covers what works without a vendor cloud backend and what needs a relay.

**Method and confidence tags**
- Facts were checked against official docs, changelogs, product-lifecycle pages, GitHub repos/issues, and package registries (npm / crates.io / PyPI) on or around 2026-10-18. URLs are cited inline.
- **[official]** means the claim comes from the vendor's own docs or changelog. **[community]** means a forum, issue tracker, third-party library, or blog. **[unverified]** means I couldn't confirm it from a primary source, so check it before building on it.
- Some official pages couldn't be fetched (HTTP 402/403): the Ko-fi help center, X automation rules, the X devcommunity forum, and Streamlabs' rendered API reference. Where that happened the claim uses secondary sources and is tagged.

---

## Contents

1. [Executive summary and feasibility matrix](#1-executive-summary-and-feasibility-matrix)
2. [Cross-cutting architecture implications](#2-cross-cutting-architecture-implications)
3. [Twitch](#3-twitch)
4. [YouTube Live](#4-youtube-live)
5. [Kick](#5-kick)
6. [TikTok LIVE](#6-tiktok-live)
7. [X / Twitter](#7-x--twitter)
8. [Trovo, Rumble, Facebook Live, Discord](#8-trovo-rumble-facebook-live-discord)
9. [OBS Studio (obs-websocket v5), Streamlabs Desktop, Meld Studio](#9-obs-studio-obs-websocket-v5-streamlabs-desktop-meld-studio)
10. [Donation / tip / commerce platforms](#10-donation--tip--commerce-platforms)
11. [Cloud relay: when do we need one?](#11-cloud-relay-when-do-we-need-one)
12. [OBS custom browser docks and browser-source overlay best practices](#12-obs-custom-browser-docks-and-browser-source-overlay-best-practices)
13. [Open questions for the project owner](#13-open-questions-for-the-project-owner)
14. [Chat send capability matrix](#14-chat-send-capability-matrix)

---

## 1. Executive summary and feasibility matrix

| Platform | Chat read | Chat send | Events (alerts) | Desktop auth without a server | Relay needed? | Feasibility |
|---|---|---|---|---|---|---|
| **Twitch** | EventSub WebSocket (official) | Helix `POST /chat/messages` (official) | Complete set over EventSub WS | **Yes.** Device Code Flow with a *public* client, no secret | No (only for optional Chat Bot badge / conduits) | **GREEN** |
| **YouTube** | `liveChatMessages.streamList` gRPC server-streaming (official, 5 units per call) | `liveChatMessages.insert` (20 units) | Super Chat, Super Sticker, memberships, gifted memberships, Jewels gifts, polls, bans in the chat stream. New subscribers only by polling (public subs only) | **Yes.** Google "Desktop app" client: loopback + PKCE (embedded secret is "not treated as a secret") or device flow | No for API path. **Quota** is the real constraint | **YELLOW** (quota, OAuth verification) |
| **Kick** | Official: **webhooks only** (`chat.message.sent`). Unofficial: Pusher WS (Kick says unsupported) | Official `POST /public/v1/chat` | follows, subs, gifts, KICKs, rewards, bans, live status (webhooks) | **No.** PKCE is required **and** `client_secret` is required for token exchange and refresh | **Yes.** Public HTTPS webhook receiver + token broker (or BYO app + tunnel) | **YELLOW/RED** |
| **TikTok LIVE** | Unofficial WebCast WS only (TikTok-Live-Connector / TikTokLive + Euler Stream signer) | Unofficial only (needs session cookie, sent to third-party signer) | gifts, likes, follows, shares, joins, subs, battles (unofficial) | n/a (no official API) | Third-party signer, not ours | **RED** (ToS, breakage), opt-in only |
| **X** | No chat for X Live without whitelisted Livestream API + webhooks | `POST /2/broadcasts/{id}/chat` (whitelisted) | mentions via pay-per-use reads / Activity API | **Yes.** OAuth 2.0 PKCE "Native App" public client, `http://127.0.0.1` callback | Livestream chat needs webhooks. Posting doesn't | **YELLOW** for posting (user pays). **RED** for live chat |
| **Trovo** | n/a | n/a | n/a | n/a | n/a | **DEAD**: live streaming shut down 2026-06-30 |
| **Rumble** | Official key-URL JSON polling (50 recent msgs). Unofficial SSE chat (`web7.rumble.com`, no login needed) | **No official write API.** Unofficial: internal chat API with the account's `u_s` session cookie (send, delete, pin, mute). See 8.1 | followers, subs, gifted subs, rants (polling, or live in the SSE stream) | Read: user pastes API URL (no OAuth). Send: Rumble web login (session cookie) | No | **YELLOW** (read) / **RED** (send: ToS, breakage) |
| **Facebook Live** | Graph SSE `live_comments` / polling | Comment as Page (App Review) | limited | Device login (client token) or manual flow. Long-lived exchange needs app secret | Partially (App Review, Business Verification) | **YELLOW/RED** (bureaucracy) |
| **Discord** | Bot gateway (BYO bot token) | Webhook URL paste (no auth) or bot | n/a | Webhook paste needs nothing. OAuth needs secret | No (for paste-webhook / BYO bot) | **GREEN** |
| **OBS (obs-websocket 5.x)** | n/a | n/a | full OBS control/events | Local password (SHA-256 challenge) | No | **GREEN** |
| **Streamlabs Desktop** | n/a | n/a | scenes/sources/streaming | Local token, port 59650 | No | **GREEN** |
| **Meld Studio** | n/a | n/a | scene/layer control | "Allow remote connections". Protocol undocumented, Windows/macOS only | No | **YELLOW** |
| **Streamlabs / StreamElements / Tipeeestream** | n/a | n/a | tips, merch, cross-platform activities | Copy-paste socket token / JWT / API key | No (outbound sockets) | **GREEN** |
| **Ko-fi / Throne / Fourthwall / BMC / Shopify** | n/a | n/a | tips, gifts, orders | Webhook secret | **Yes, public HTTPS URL** | **YELLOW** (relay) |
| **Patreon** | n/a | n/a | members/pledges | Creator access token (polling) or webhooks | Webhooks yes. Polling no | **GREEN/YELLOW** |

**Bottom line:** Twitch, OBS, Discord (webhook), and the socket-based tip services are fully local. YouTube is local but quota-bound. **Kick and every webhook-only service (Ko-fi, Throne, Fourthwall, BMC, Shopify, Patreon webhooks) need a publicly reachable HTTPS endpoint.** Kick also needs a confidential-client token exchange. Those needs justify an **optional, open-source, self-hostable relay** (section 11).

---

## 2. Cross-cutting architecture implications

1. **One upstream connection per platform, fanned out locally.** Twitch limits EventSub WebSockets to **3 connections per (client ID, user)** ([official](https://dev.twitch.tv/docs/eventsub/manage-subscriptions/)). YouTube bills per call against a shared daily quota ([official](https://developers.google.com/youtube/v3/determine_quota_cost)). So the core process must own each upstream connection and re-broadcast to docks/overlays over its own local WebSocket. Overlays must never talk to platforms directly.
2. **Event normalization + idempotency.** Kick (`Kick-Event-Message-Id` ULID), Throne (`event_id`), and Twitch WebSocket reconnects all need dedupe keys. Twitch EventSub WS does **not replay** events lost during a reconnect ([official](https://dev.twitch.tv/docs/eventsub/handling-websocket-events/)).
3. **Desktop OAuth pattern.** RFC 8252 loopback redirect (`http://127.0.0.1:<random port>`) + PKCE in the **system browser** is the norm. Google explicitly disallows embedded webviews and custom URI schemes ([official](https://developers.google.com/identity/protocols/oauth2/native-app)). Twitch has **no PKCE** and uses Device Code Flow instead (section 3.5).
4. **Token storage.** Use the OS keychain. Rust: `keyring` crate (Secret Service / Keychain / Windows Credential Manager. The v3 to v4 split was underway in 2026, so check crates.io) ([community](https://lib.rs/crates/keyring)). Rust OAuth: `oauth2` 5.0.0 (Jan 2025) supports PKCE and RFC 8628 device flow ([community](https://docs.rs/oauth2)). For Tauri, `tauri-plugin-oauth` runs the loopback listener ([community](https://docs.rs/tauri-plugin-oauth)).
5. **Client IDs are long-lived identities.** Twitch custom channel-point rewards can only be edited by the client ID that created them. Firebot had to make users recreate rewards when it changed login flow and app in v5.60 ([community](https://github.com/crowbartools/Firebot/wiki/Firebot-v5.60.0-Twitch-Login-Changes-FAQ)). Pick the production client IDs once.
6. **Data-retention rules affect "chat history" features.** Twitch content may only be cached about 24 h without authorization, and data must be deleted on revocation ([Twitch Developer Agreement](https://legal.twitch.com/legal/developer-agreement/)). YouTube API data must be refreshed or deleted after 30 days ([official](https://developers.google.com/youtube/terms/developer-policies)).

---

## 3. Twitch

### 3.1 Real-time transport

| Item | Detail | Source |
|---|---|---|
| EventSub WebSocket URL | `wss://eventsub.wss.twitch.tv/ws`, optional `?keepalive_timeout_seconds=10..600` | [official](https://dev.twitch.tv/docs/eventsub/handling-websocket-events/) |
| Session flow | `session_welcome` (session ID), then **10 s to create a subscription** or the socket closes. `session_keepalive` when idle. `session_reconnect` with `reconnect_url` 30 s before close (keep old socket until new Welcome). Close code 4004 if you mishandle reconnect | [official](https://dev.twitch.tv/docs/eventsub/handling-websocket-events/) |
| Client-to-server messages | None except Pong. Sending anything else closes the socket | same |
| Replay | **No replay** of events lost while reconnecting after a dropped connection (the reconnect-message flow is lossless) | same |
| Limits (per user token = client ID + user ID) | **3** WebSocket connections with enabled subs. **300** enabled subs per connection. `max_total_cost` **10** (subs the user authorized cost 0) | [official](https://dev.twitch.tv/docs/eventsub/manage-subscriptions/) |
| Token type | WebSocket subscriptions use **user access tokens** | [official](https://dev.twitch.tv/docs/authentication/) |
| Conduits | Wrapper load-balancing subs across shards (webhook/WS). **"for backend server applications and requires app access tokens"**, so a client secret is needed and conduits are **not usable by a public desktop client** | [official](https://dev.twitch.tv/docs/eventsub/handling-conduit-events/) |
| PubSub | **Decommissioned 2025-04-14** | [official](https://dev.twitch.tv/docs/product-lifecycle/) |

### 3.2 Chat: EventSub + Helix vs IRC

- **Read:** `channel.chat.message` (scope `user:read:chat`), plus `channel.chat.notification` (subs, resubs, gift subs, raids, announcements, watch streaks, modiversary, gifted-drops summaries), `channel.chat.message_delete`, `channel.chat.clear`, `channel.chat.clear_user_messages`, `channel.chat_settings.update`, `channel.chat.user_message_hold/update` ([official types](https://dev.twitch.tv/docs/eventsub/eventsub-subscription-types/)).
- **Send:** `POST /helix/chat/messages` (scope `user:write:chat`). It uses a **separate rate-limit bucket** from regular Helix. It supports replies, `for_source_only` in shared chat, and since 2026-05 an optional `pin` parameter ([official chat](https://dev.twitch.tv/docs/chat/), [changelog](https://dev.twitch.tv/docs/change-log/)).
- **IRC status:** still "Active", but **non-secure (ws://) IRC WebSocket was decommissioned 2025-08-15**. Twitch says "the preferred method of viewing and sending chats on Twitch is through EventSub and Twitch API" ([product lifecycle](https://dev.twitch.tv/docs/product-lifecycle/), [chat](https://dev.twitch.tv/docs/chat/), [IRC migration guide](https://dev.twitch.tv/docs/chat/irc-migration/)). Streamer.bot removed its IRC connection entirely in v1.0.5/1.0.7 (Aug 2026) ([community](https://docs.streamer.bot/changelogs/v1.0.7)). **Recommendation: EventSub + Helix only. No IRC.**
- **Chat Bot badge** (since 2025-08-28) requires sending with an **App Access Token** + `user:bot` + `channel:bot` (or mod status). Appearing under "Chat Bots" in the user list likewise requires reading via an app token ([official](https://dev.twitch.tv/docs/chat/)). App tokens need a client secret (client-credentials grant), so **a local public client cannot get the badge.** A relay could mint app tokens, but the badge is cosmetic.

**Chat rate limits** ([official](https://dev.twitch.tv/docs/chat/))

| Account | Limit |
|---|---|
| Regular (not broadcaster/mod/VIP) | 20 msgs / 30 s, and 1 msg/s per channel |
| Broadcaster / mod / VIP | 100 msgs / 30 s |
| Verified bot | 7,500 msgs / 30 s (verified-bot reviews currently **paused**) |
| Exceeding | Messages ignored for 1 h (except where the sender is broadcaster/mod/VIP) |
| Concurrent joins | 100 channels per account (since 2024-05-15). Broadcaster/mod joins don't count |
| Join rate | 20 joins / 10 s (normal), 2,000 / 10 s (verified) |

### 3.3 Event catalog (EventSub, as of 2026-10)

| Category | Subscription types | Notes |
|---|---|---|
| Follows | `channel.follow` v2 | needs `moderator:read:followers` |
| Subs | `channel.subscribe`, `channel.subscription.message`, `channel.subscription.gift`, `channel.subscription.end` | `tier` = `1000/2000/3000`. Prime appears as `is_prime` on chat notifications |
| Bits | `channel.cheer`, **`channel.bits.use`** (all Bits usage incl. Power-ups. Combos removed and custom Power-ups added 2026-04) | `bits:read` |
| Power-ups | `channel.bits.use` (gigantify, message effects, celebrations), **`channel.custom_power_up_redemption.add`** (v1 GA 2026-05-19) | custom Power-ups were rolling out from an invited alpha ([announcement](https://discuss.dev.twitch.com/t/introducing-api-and-eventsub-support-for-custom-power-ups/64708)) |
| Raids | `channel.raid` | |
| Hype Train | `channel.hype_train.begin/progress/end` **v2** | v1 + "Get Hype Train Events" decommissioned **2025-12-04**. Use Get Hype Train Status ([announcement](https://discuss.dev.twitch.com/t/legacy-get-hype-train-events-api-and-eventsub-hype-train-v1-subscription-types-deprecation-and-withdrawal-timeline/64299)) |
| Channel points | `channel.channel_points_custom_reward.add/update/remove`, `..._redemption.add/update`, `channel.channel_points_automatic_reward_redemption.add` | rewards editable only by creating client ID |
| Polls / Predictions | `channel.poll.begin/progress/end`, `channel.prediction.begin/progress/lock/end` | |
| Shared chat | `channel.shared_chat.begin/update/end`. `source_broadcaster_*` / `is_source_only` fields on chat events | |
| Moderation | `channel.moderate` v2, `channel.ban/unban`, `channel.moderator.add/remove`, `channel.vip.add/remove`, `channel.warning.send/acknowledge`, `channel.unban_request.create/resolve`, `channel.suspicious_user.message/update`, `automod.message.hold/update` v2 (`blocked_link` reason added 2026-09), `automod.settings.update`, `automod.terms.update`, `channel.shield_mode.begin/end` | |
| Other | `stream.online/offline`, `channel.update`, `channel.ad_break.begin`, `channel.charity_campaign.*`, `channel.goal.*`, `channel.shoutout.create/receive`, `user.whisper.message`, `channel.guest_star_*` (beta) | |

Source: the full list scraped from the [subscription types page](https://dev.twitch.tv/docs/eventsub/eventsub-subscription-types/) (79 types) and the [changelog](https://dev.twitch.tv/docs/change-log/).

### 3.4 Moderation via Helix

Ban/timeout and unban, delete chat messages, chat settings (slow/sub/follower/emote modes), Shield Mode, AutoMod held-message approve/deny and settings, blocked terms, warnings, unban-request resolution, add/remove moderators and VIPs, announcements (app-token support added 2026-04), shoutouts, pinned chat messages (GA 2026-05-29), and suspicious-user status add/remove (GA 2026-02-05) ([API reference](https://dev.twitch.tv/docs/api/reference/), [changelog](https://dev.twitch.tv/docs/change-log/)). Note the Lead Moderator role (2025-12-18): check for badge `lead_moderator` **or** `moderator` ([announcement](https://discuss.dev.twitch.com/t/lead-moderator-role-and-chat-badge-introduced/64488)).

### 3.5 Auth flows for a server-less desktop app

| Flow | Secret needed? | Refresh? | Desktop suitability | Source |
|---|---|---|---|---|
| **Device Code Grant (DCF), Public client** | **No** | Yes. Refresh tokens are **one-time use** and **expire after 30 days of inactivity** | **Recommended.** Twitch's auth overview lists "Electron applications" as a DCF use case. `verification_uri` comes pre-filled (`https://www.twitch.tv/activate?public=true&device-code=...`) | [official](https://dev.twitch.tv/docs/authentication/getting-tokens-oauth/) |
| Implicit grant (confidential client) | No at runtime | **No refresh.** Tokens reportedly last ~60 days ([community](https://discuss.dev.twitch.com/t/public-client-id-stopped-working/61505)) | Works with `http://localhost` redirect + local listener, but forces periodic re-login | [official](https://dev.twitch.tv/docs/authentication/getting-tokens-oauth/) |
| Authorization code | **Yes** | Yes | Needs server or shipped secret (Twitch: "never expose it to users, even in an obscured form") | [official](https://dev.twitch.tv/docs/authentication/register-app) |
| Client credentials (app token) | **Yes** | n/a | Server only (conduits, Chat Bot badge) | same |
| PKCE | **Not supported**. Forum requests from 2019, 2021, 2024, and Jan 2026 got no official PKCE | [community](https://discuss.dev.twitch.com/t/auth-flow-for-native-application-without-a-backend-server/64539) |

Key constraints:
- Public clients "**are only limited to the usage of device authorization grant flow**". They can't use implicit or client-credentials ([official](https://dev.twitch.tv/docs/authentication/getting-tokens-oauth/)). Client type appears fixed at creation, so you would make a new app to switch ([community](https://discuss.dev.twitch.com/t/what-type-of-client-to-choose/63638)).
- Refresh **without** `client_secret` is allowed only for public clients ([official](https://dev.twitch.tv/docs/authentication/refresh-tokens/)). **Persist the newest refresh token atomically** after every refresh (single-use).
- **Must call `/oauth2/validate` at startup and hourly.** Twitch audits for this ([official](https://dev.twitch.tv/docs/authentication/validate-tokens/)).
- Don't share one client ID across different applications ([official](https://dev.twitch.tv/docs/authentication/register-app)).
- Shipping a secret is a real risk: it's extractable from any binary, and abuse can get the client ID suspended for all users.

### 3.6 Rate limits and cost

- Helix: token bucket, **800 points/min** (`Ratelimit-Limit: 800`) per client ID per user for user tokens. Most endpoints cost 1 point ([official](https://dev.twitch.tv/docs/api/guide/)).
- Chat limits are in 3.2. Cost: free.

### 3.7 ToS highlights ([Developer Services Agreement](https://legal.twitch.com/legal/developer-agreement/))

- No caching of Twitch Content beyond **24 h** without authorization. Honor deletions.
- Delete user data on revocation or request. Retain chat logs only "as long as necessary for the operation of Your Services".
- A public privacy policy is required.
- Don't exceed or circumvent rate limits.
- Don't sell or distribute metadata.

### 3.8 Libraries

| Lang | Library | Latest | Notes |
|---|---|---|---|
| Rust | `twitch_api` (twitch-rs) | 0.8.0 (2026-06) | Helix + EventSub types + WS example. Active |
| Rust | `twitch_oauth2` | 0.17.1 (2026-03) | **DCF + secret-less refresh for public clients** supported |
| Rust | `twitch-irc` | 6.1.3 (2026-10) | IRC only. Not needed |
| TS | Twurple (`@twurple/*`) | 8.2.0 (2026-09) | **DCF added** (`startDeviceCodeFlow`, `exchangeDeviceCode`), custom Power-ups, `@twurple/eventsub-ws`. Most complete TS option |
| TS | `tmi.js` | 1.8.5 (**2021-08**) | IRC-only and effectively stale. Avoid |
| TS | `@tmi.js/chat` | 0.8.0 (2026-05) | IRC successor, still pre-1.0 |

Versions from registry/GitHub queries on 2026-10-18 ([twitch_api](https://github.com/twitch-rs/twitch_api), [twitch_oauth2 changelog](https://github.com/twitch-rs/twitch_oauth2), [twurple](https://github.com/twurple/twurple)).

### 3.9 Notable 2025–2026 changes

PubSub shutdown (2025-04-14). `for_source_only` / shared-chat behavior (2025-04/05). Hype Train v2 (2025-06) and v1 removal (2025-12-04). Non-secure IRC WS removal (2025-08-15). Chat Bot badge (2025-08-28). Get Authorization By User (GA 2025-10-30). Lead Moderator (2025-12-18). Suspicious-user APIs (GA 2026-02). App-token support on several endpoints (GA 2026-05-29). Custom Power-ups (2026-05). Pinned chat (2026-05). GIF fragments in `channel.chat.message` (2026-07-16). Gifted-drops summary notices (2026-10-01). Sources: the [changelog](https://dev.twitch.tv/docs/change-log/) and [product lifecycle](https://dev.twitch.tv/docs/product-lifecycle/) pages.

**Recommendation:** register one **Public** client. Use DCF for both the broadcaster and the optional bot account, EventSub WebSocket for everything, and Helix for sending and moderation. No relay.

---

## 4. YouTube Live

### 4.1 Chat transport and quota

| Method | Quota (units) | Notes | Source |
|---|---|---|---|
| `liveChatMessages.list` (polling) | **5 per call** | Response has `pollingIntervalMillis`. Polling faster gives `rateLimitExceeded`. The doc now says "To poll for live chat messages, use the liveChatMessages.streamList method" | [official](https://developers.google.com/youtube/v3/live/docs/liveChatMessages/list) |
| **`liveChatMessages.streamList`** (server-streaming) | **5 per call** | gRPC service `youtube.api.v3.V3DataLiveChatMessageService/StreamList` at `youtube.googleapis.com:443`. The proto is published in the guide. Auth via `x-goog-api-key` **or** `authorization: Bearer`. Resume with last `nextPageToken`. The first response holds only recent messages. `offlineAt` appears when the stream ended | [method](https://developers.google.com/youtube/v3/live/docs/liveChatMessages/streamList), [guide](https://developers.google.com/youtube/v3/live/streaming-live-chat) |
| `liveChatMessages.insert` | 20 | send message or poll | [official](https://developers.google.com/youtube/v3/determine_quota_cost) |
| `liveChatMessages.delete` / `transition` | 20 | owner or moderator | same |
| `liveChatBans.insert/delete` | **200** | `type` = `permanent` or `temporary` | same |
| `liveChatModerators.list` / `insert` / `delete` | 50 / 200 / 200 | | same |
| `liveBroadcasts.list`, `videos.list`, `channels.list`, `subscriptions.list`, `superChatEvents.list` | 1 each | `members.list` is 1 on the cost table but **2** on its own page (conflict) | same, [members](https://developers.google.com/youtube/v3/docs/members/list) |
| Default allocation | **10,000 units/day per project** (resets 00:00 PT). Since 2026-06-01, `search.list` and `videos.insert` have their own buckets (100 calls/day each) | [official](https://developers.google.com/youtube/v3/getting-started), [revision history](https://developers.google.com/youtube/v3/revision_history) |

**streamList details and caveats**
- streamList appears in the live revision history only as a 2025-07-14 doc update ("guidance on the streamList API usages"). No formal launch date was found ([official](https://developers.google.com/youtube/v3/live/revision_history)).
- Google documents "a quota cost of 5 units" **per call**, but doesn't say whether a long-lived stream is re-billed while open. Community clients assume **5 units per connection** ([muxable/rtchat#1482](https://github.com/muxable/rtchat/issues/1482)), and one OSS project was still measuring this in Sep 2026 ([Social-Feed PR #2](https://github.com/Voidscape-Development/Social-Feed/pull/2)). **[unverified]** Measure in Cloud Console before relying on it.
- A community report (2026-10-14) says **an API key is enough to stream public chats** ([rtchat#1482](https://github.com/muxable/rtchat/issues/1482)) **[community]**.
- A REST/HTTP-streaming form (`GET https://youtube.googleapis.com/youtube/v3/liveChat/messages/stream`) is used by one OSS prototype, which was **untested against real YouTube** ([Social-Feed PR #2](https://github.com/Voidscape-Development/Social-Feed/pull/2)) **[unverified]**. The official error table does list both gRPC and "web" error codes ([official](https://developers.google.com/youtube/v3/live/docs/liveChatMessages/streamList)).
- The guide recommends channel pooling, since HTTP/2 caps concurrent streams per connection ([official](https://developers.google.com/youtube/v3/live/streaming-live-chat)).

**Quota math for one 4-hour stream**

| Approach | Units |
|---|---|
| Polling `list` every 5 s | 2,880 calls × 5 = **14,400** (over the whole 10k default for one streamer) |
| Polling at 8 s | ~2,250 units/h, so 10k covers about 4.4 h ([Social-Feed PR](https://github.com/Voidscape-Development/Social-Feed/pull/2)) |
| `streamList`, ~10 reconnects | ~50 (if billed per connection) |
| New-subscriber poll every 60 s (`subscriptions.list`) | 240 |
| 50 bot chat messages | 1,000 |
| 5 bans | 1,000 |

With **one shared Google project**, the default 10k/day supports only a handful of active streamers even with streamList. Streamer.bot hits exactly this: its FAQ says its YouTube quota is a **shared pool across all users** that runs out and stops triggers until 00:00 PST ([community](https://docs.streamer.bot/faq/youtube-quota)). More quota requires a **YouTube API Services audit** via the Audit and Quota Extension Form ([official](https://developers.google.com/youtube/v3/guides/quota_and_compliance_audits)). The Elgato Stream Deck YouTube plugin and vMix Social report the same shared-key exhaustion pattern (search-result snippets only; their pages returned 403).

### 4.2 Message/event types in the chat stream ([official](https://developers.google.com/youtube/v3/live/docs/liveChatMessages))

| `snippet.type` | Meaning |
|---|---|
| `textMessageEvent` | chat message |
| `superChatEvent` | Super Chat (`amountMicros`, `currency`, `amountDisplayString`, `tier`, `userComment`) |
| `superStickerEvent` | Super Sticker (`stickerId`, `altText`, amount, tier) |
| `newSponsorEvent` | **new member** (`memberLevelName`, `isUpgrade`) |
| `memberMilestoneChatEvent` | member milestone (`memberMonth`) |
| `membershipGiftingEvent` | **gifted memberships purchased** (`giftMembershipsCount`, level) |
| `giftMembershipReceivedEvent` | recipient of a gift membership (`gifterChannelId`) |
| `giftEvent` | **Jewels gift** (added **2026-03-26**: `giftName`, `jewelsAmount`, `comboCount`. Field renamed to `snippet.giftDetails` on 2026-10-12) |
| `pollEvent` / `pollDetails` | live poll (tally only for owner-authorized requests) |
| `userBannedEvent` | ban/timeout (`banType`, `banDurationSeconds`) |
| `sponsorOnlyModeStartedEvent` / `...EndedEvent` | members-only mode |
| `chatEndedEvent`, `tombstone` | end / deleted placeholder |

`messageDeletedEvent` and `messageRetractedEvent` references were **removed 2026-06-23** because the API doesn't return them ([revision history](https://developers.google.com/youtube/v3/revision_history)). So you can't reliably mirror deletions into the overlay. `authorDetails` carries `isChatOwner`, `isChatModerator`, `isChatSponsor`, and `isVerified`.

### 4.3 "New subscriber" detection

- No real-time subscriber event exists in the Live API. The only official path is polling `subscriptions.list` with `myRecentSubscribers=true` (newest first, 1 unit, **"the maximum number of subscribers returned through this API might be limited"**) ([official](https://developers.google.com/youtube/v3/docs/subscriptions/list)).
- Only **public** subscriptions are visible. Streamer.bot v1.0.7 (2026-08-06) dropped its StreamElements dependency and **polls every 10 s**. Its docs say no event fires for users with **private subscriptions** or **no YouTube channel** ([Streamer.bot changelog](https://docs.streamer.bot/changelogs/v1.0.7), [trigger doc](https://docs.streamer.bot/api/triggers/youtube/general/new-subscriber)). Note that 10 s polling costs 360 units/h.
- Social Stream Ninja: "subscriber data may lag behind on YouTube's side and may only include public subscriptions" ([community](https://socialstream.ninja/docs/youtube-setup-guide.html)).
- Alternative: Streamlabs' socket emits `follow` for `youtube_account` (YouTube subscription) ([Streamlabs](https://dev.streamlabs.com/docs/socket-api)). StreamElements' `channel.activities` also covers multi-provider activities ([SE](https://docs.streamelements.com/websockets/topics/channel-activities)). Both route through those vendors' clouds via the user's existing account.

### 4.4 Super Chat and memberships via other endpoints

- `superChatEvents.list`: Super Chats from the last 30 days (scopes `youtube`, `youtube.force-ssl`, or `youtube.readonly`). Not deprecated as of 2026-10-15 ([official](https://developers.google.com/youtube/v3/live/docs/superChatEvents/list)). This is useful for backfill and reconciliation.
- `members.list` / `membershipsLevels.list` require the `youtube.channel-memberships.creator` scope **and approval from a Google/YouTube representative** ([official](https://developers.google.com/youtube/v3/docs/members/list)). **Don't plan on them.** Use chat-stream membership events instead.

### 4.5 Auth for desktop

| Option | Detail | Source |
|---|---|---|
| **"Desktop app" OAuth client + loopback + PKCE** | `http://127.0.0.1:<port>` or `[::1]`. PKCE S256 "always". **Must use the system browser**, not an embedded webview. OOB copy-paste and custom URI schemes no longer supported | [official](https://developers.google.com/identity/protocols/oauth2/native-app) |
| Client secret | Desktop apps "are public clients and cannot keep secrets". Embedding is expected: "the client secret is obviously not treated as a secret" | [official](https://developers.google.com/identity/protocols/oauth2) |
| **Device flow** ("TVs and Limited Input devices" client) | Allowed YouTube scopes: **`youtube` and `youtube.readonly` only** (not `force-ssl`). `liveChatMessages.insert` and bans accept `youtube`, so device flow can cover read + send + moderate | [official](https://developers.google.com/identity/protocols/oauth2/limited-input-device), [discovery doc scopes](https://youtube.googleapis.com/$discovery/rest?version=v3) |
| Verification | YouTube scopes are **sensitive**. Unverified apps show a warning screen and are capped at **100 new users** until verified | [official](https://support.google.com/cloud/answer/7454865) |
| Refresh tokens | Expire if unused 6 months. **Expire after 7 days for projects in "Testing" status.** Max 100 refresh tokens per Google account per client | [official](https://developers.google.com/identity/protocols/oauth2) |

The "Testing" 7-day expiry is a trap for **BYO-project** setups. Social Stream Ninja's own-project guide tells users to stay in testing mode and add themselves as a test user ([SSN](https://socialstream.ninja/docs/youtube-project-setup.html)), which means re-login weekly unless they publish the project.

### 4.6 Shared project vs BYO credentials vs scraping

| Model | Pros | Cons |
|---|---|---|
| Ship one Google project (Streamer.bot model) | Zero setup | Shared 10k/day until an audit is approved. Needs OAuth verification. All users fail together when exhausted |
| BYO project or API key (SSN option) | Each user gets their own 10k/day | Painful setup (SSN asks users for a "Web application" client ID, secret, and API key with `localhost:8181` redirect). Testing-mode tokens expire in 7 days. Policy gray zone: the Developer Policies require "exactly one (1) API Project" per API client and forbid sharing API credentials ([official](https://developers.google.com/youtube/terms/developer-policies)) |
| InnerTube scraping (SSN default via browser extension / pop-out chat. Libs: `youtubei.js`, `masterchat`) | No quota. Real-time | **Violates** YouTube Developer Policies ("must not use undocumented APIs", no scraping) and the YouTube ToS (no automated access) ([policies](https://developers.google.com/youtube/terms/developer-policies), [ToS](https://www.youtube.com/t/terms)). Breaks without notice |

How competitors do it: **Streamer.bot** uses OAuth "Sign in with Google" on its shared project and has hit shared-quota problems ([docs](https://docs.streamer.bot/guide/platforms/youtube), [FAQ](https://docs.streamer.bot/faq/youtube-quota)). **Social Stream Ninja** defaults to capturing the pop-out chat page via its extension or desktop app (no API). It offers an optional "WebSocket/API source mode" with OAuth and a recommended custom project for your own quota ([SSN guide](https://socialstream.ninja/docs/youtube-setup-guide.html), [repo](https://github.com/steveseguin/social_stream)).

### 4.7 Libraries

| Lang | Library | Latest | Notes |
|---|---|---|---|
| Rust | `google-youtube3` (google-apis-rs) | 7.0.0+20251222 | generated REST client. No streamList (gRPC) |
| Rust | `tonic` 0.14.6 + `prost` 0.14.4 | 2026 | generate the streamList client from the published proto. Straightforward |
| Rust | `yup-oauth2` 12.1.2 / `oauth2` 5.0.0 | 2026 / 2025 | installed-app and device flows |
| Rust | `youtube_chat` 0.2.2 | 2024-12 | InnerTube scraper. Small, stale |
| TS | `googleapis` 186 / `@googleapis/youtube` 40.0.1 | 2026-10 | official REST |
| TS | `@grpc/grpc-js` 1.14.6 | 2026-10 | for streamList |
| TS | `youtubei.js` (LuanRT) | 18.2.0 (2026-10-18) | very active InnerTube client with LiveChat parsers (unofficial) |
| TS | `@stu43005/masterchat` | 2.1.0 (2026-08) | fork of masterchat. Original `masterchat` stale (2022) |
| TS | `youtube-chat` | 2.2.0 (2022) | stale |

Sources: registry queries (npm/crates.io/GitHub API) on 2026-10-18.

### 4.8 Notable 2025–2026 changes

streamList guidance (2025-07-14). `list` docs now push streamList. Upload cost cut to ~100 units (2025-12-04). Granular quota buckets (2026-06-01). `giftEvent` / Jewels (2026-03-26). Deletion/retraction events removed from docs (2026-06-23). `liveChatBans.insert` type clarification and poll-insert fixes (2026-07-09). Midroll ad controls on `liveBroadcasts` (2026-01-12). Geo `availabilityConfig` (2026-06-01). `categoryId` on broadcasts (2026-08-17). Sources: [live revision history](https://developers.google.com/youtube/v3/live/revision_history), [Data API revision history](https://developers.google.com/youtube/v3/revision_history).

**Recommendation:** Desktop OAuth client (loopback + PKCE, with device flow as fallback). Use streamList over gRPC for chat. Poll `subscriptions.list?myRecentSubscribers` only while live, at 30–60 s. Use `superChatEvents.list` for reconciliation. Apply for a quota extension and OAuth verification early, and offer **BYO API project** as an advanced setting. Treat InnerTube scraping as an explicit opt-in "unofficial mode" (or skip it).

---

## 5. Kick

### 5.1 Official public API (launched early 2025; docs at [docs.kick.com](https://docs.kick.com), repo [KickEngineering/KickDevDocs](https://github.com/KickEngineering/KickDevDocs))

| Area | Endpoints / events | Source |
|---|---|---|
| Chat send | `POST /public/v1/chat` (`chat:write`). `type: "user"` (needs `broadcaster_user_id`) or `"bot"` (always posts to the token's own channel). 500 graphemes / 2048 bytes. Replies supported | [chat](https://docs.kick.com/apis/chat.md) |
| Chat delete | `DELETE /public/v1/chat/{message_id}` (`moderation:chat_message:manage`, added 2025-12-02) | same |
| Moderation | bans/unbans (`moderation:ban`, since 2025-05-21) | [changelog](https://raw.githubusercontent.com/KickEngineering/KickDevDocs/main/changelog.md) |
| Channel points | Channel Rewards APIs (2025-12-03). Redemptions fetch/accept/reject (2026-01-15) | same |
| Other | users, channels (incl. `active_gifted_subscribers_count` since 2026-08-11), livestreams **v2** (v1 deprecated 2026-06-23), categories **v2** (v1 deprecated 2026-01-15), KICKs leaderboard, ads, public key, drops | same |
| **Events (webhooks only)** | `chat.message.sent`, `channel.followed`, `channel.subscription.new`, `channel.subscription.renewal`, `channel.subscription.gifts`, `channel.reward.redemption.updated`, `livestream.status.updated`, `livestream.metadata.updated`, `moderation.banned`, `kicks.gifted` (all v1) | [event types](https://docs.kick.com/events/event-types.md) |
| Not available | raids/hosts (requested in [#102](https://github.com/KickEngineering/KickDevDocs/issues/102)), polls/predictions, message-deleted events | issues |

### 5.2 Webhooks need a public URL

- Subscriptions are created with `POST /public/v1/events/subscriptions`. The only `method` is **`webhook`** ([official](https://docs.kick.com/events/subscribe-to-events.md)).
- The webhook URL is configured **per app** in developer settings, so a multi-user desktop app needs one central receiver ([issue #20](https://github.com/KickEngineering/KickDevDocs/issues/20)). WebSocket delivery has been "on our roadmap" since Feb 2025, reaffirmed by Kick staff in Nov 2025, with no ETA.
- **Limits:** 10,000 subscriptions per event type per app, but **`chat.message.sent` is capped at 1,000 for unverified apps** (10,000 when verified) ([official](https://docs.kick.com/events/subscribe-to-events.md), [FAQ](https://docs.kick.com/apis/faqs.md)). A shared relay app can only serve **about 1,000 users' chat** until verified.
- **Security:** `Kick-Event-Signature` is RSA PKCS#1 v1.5 + SHA-256 over `message_id.timestamp.raw_body`. Fetch the key from `GET https://api.kick.com/public/v1/public-key` (it rotates). Use `Kick-Event-Message-Id` (ULID) for idempotency ([official](https://docs.kick.com/events/webhook-security.md)).
- **Failure handling:** apps whose webhook keeps failing for more than a day are **auto-unsubscribed** from that event (2025-12-12). Disabling webhooks unsubscribes everything (2025-12-19) ([changelog](https://docs.kick.com/changelog.md)). The relay must be highly available, and the desktop must re-subscribe on start.
- Streamer.bot users report delays ("Kick API Delay" indicator on its status page) ([Streamer.bot](https://docs.streamer.bot/guide/platforms/kick)).

### 5.3 OAuth 2.1 + PKCE (still needs a secret)

- `https://id.kick.com/oauth/authorize` requires `code_challenge` (S256) and `state`. **Token exchange requires `client_id` + `client_secret` + `code_verifier`. Refresh also requires `client_secret`.** App tokens come via client credentials ([official](https://docs.kick.com/getting-started/generating-tokens-oauth2-flow.md)).
- "Made refresh tokens reusable/flexible" (2025-11-25). The token introspect endpoint moved under `/oauth` (2026-01-15) ([changelog](https://docs.kick.com/changelog.md)).
- Public/native client support is an **open** request ([#413](https://github.com/KickEngineering/KickDevDocs/issues/413), 2026-08-30). It lists the same three bad options we face: embed the secret, run a confidential-client relay, or make every user create their own Kick app. Earlier: [#23](https://github.com/KickEngineering/KickDevDocs/issues/23).
- Developer apps require 2FA on the Kick account ([official](https://docs.kick.com/getting-started/kick-apps-setup.md)).

### 5.4 Unofficial chat (Pusher)

- The Kick web client reads chat over Pusher: `wss://ws-us2.pusher.com/app/32cbd69e4b950bf97679`. Subscribe to `chatrooms.{chatroomId}.v2`. Events include `App\Events\ChatMessageEvent` ([kick-wss source](https://github.com/nglmercer/kick-wss), 2026). Kick also uses `wss://ws.kick.com/channels/{id}/v2`.
- Kick staff (2025-11-19): "**We don't support any bots or apps connecting to the ws.kick.com domain** … use the Events API" ([#285](https://github.com/KickEngineering/KickDevDocs/issues/285)). Cloud IPs were reportedly blocked.
- Unofficial chat **sending** needs session cookies ("selfbot") ([retconned/kick-js](https://github.com/retconned/kick-js)).
- Social Stream Ninja captures Kick via the pop-out chat page ([repo](https://github.com/steveseguin/social_stream)).
- Use Pusher only as an opt-in, read-only fallback.

### 5.5 Libraries

| Lang | Library | Latest | Notes |
|---|---|---|---|
| TS | `@nekiro/kick-api` | 3.0.1 (2026-08) | official API, PKCE helpers, RSA webhook verification. Small project (9 stars) |
| TS | `@retconned/kick-js` | 0.8.0 (2026-08) | unofficial selfbot (Pusher + cookies) |
| TS | `kick-wss` | 1.0.5 (2026-02) | Pusher chat reader |
| Rust | `kicksdk` | 0.2.0 (2026-10) | sans-IO: requests, parsers, realtime chat protocol, webhook verification. Very new (35 downloads) |
| Rust | `kick-api` | 0.1.9 (2026-04) | small |

Maturity is low across the board. Expect to own the integration.

**Recommendation:** Kick needs the relay (section 11): a webhook receiver plus token broker, with per-user mailboxes demultiplexed by `broadcaster_user_id`. The advanced alternative is "bring your own Kick app" (user enters client ID/secret, plus a user-run tunnel URL as the webhook). Pusher read-only can serve as an explicit unofficial fallback.

---

## 6. TikTok LIVE

- **No official LIVE events/chat API.** TikTok's developer products are Login Kit, Share Kit, Content Posting, Embed, Minis, Data Portability, Research, Display, Green Screen, and Commercial Content. Nothing covers LIVE ([official](https://developers.tiktok.com/products/)).
- **ToS:** section 3.4 forbids reverse engineering and scraping/automated "bots" without written approval ([TikTok ToS](https://www.tiktok.com/legal/page/us/terms-of-service/en)).

| Library | Lang | Latest | Signing | License / notes |
|---|---|---|---|---|
| [TikTok-Live-Connector](https://github.com/zerodytrash/TikTok-Live-Connector) | TS/Node | 2.5.0 (2026-09) | Euler Stream sign server (`signApiKey` optional, raises limits) | **AGPL-3.0**. "not a production-ready API. It is a reverse engineering project". ~60 events: chat, gift, member/join, like, social/follow/share, question, battle, room user, super fan, streamEnd |
| [TikTokLive](https://github.com/isaackogan/TikTokLive) | Python | 7.0.1 (2026-09) | Euler Stream default (swappable) | "modified AGPL". 50+ events |
| [piratetok-live-rs](https://github.com/PirateTok/live-rs) | Rust | 0.5.0 (2026-10) | claims direct WSS with anonymous `ttwid`, no signer | tiny (7 stars). 18+ rooms need cookies. **[unverified]** longevity |
| `tiktoklive` (TikTokLiveRust) | Rust | 0.0.19 (2024-08) | Euler key | stale |

- **Euler Stream** pricing: Community **$0** (2,500 requests/day, 25 cloud WebSockets). Business **$50/mo** (10,000/day). Enterprise custom ([pricing](https://www.eulerstream.com/pricing)). An API key is required to use their API ([docs](https://www.eulerstream.com/docs)). `tik.tools` advertises itself as a drop-in signer alternative **[community]**.
- **Sending chat** needs an Euler key **and** the account's `sessionid` + `tt-target-idc` cookies with `authenticateWs`. That **sends session credentials to the third-party signer** ([README 2.5.0](https://unpkg.com/tiktok-live-connector@2.5.0/README.md)). The account-ban risk is real but undocumented.
- **How others do it:** Streamer.bot has no direct TikTok support and relies on **TikFinity** (free, "not affiliated with TikTok") pushing into Streamer.bot's WebSocket ([TikFinity](https://tikfinity.zerody.one)). Social Stream Ninja offers WS+Auto, Standard (in-app page), Local Signer, Polling, Euler, and a TikFinity-dock fallback, and warns that TikTok "changes behavior often" ([SSN TikTok guide](https://socialstream.ninja/docs/tiktok-guide.html)).

**Recommendation:** ship TikTok as an **opt-in, read-only, "unofficial"** connector behind a plugin boundary, because of AGPL contagion and ToS. Treat chat sending as out of scope. Consider integrating with TikFinity's outputs instead of embedding a connector.

---

## 7. X / Twitter

### 7.1 Pricing (2026)

- **Pay-per-use launched 2026-02-06.** Credits are bought in the Developer Console. "Basic and Pro plans remain available" (at launch). Recently active legacy Free users got a one-time $10 voucher. **Only "for-good Public Utility apps" keep free scaled access** ([X changelog](https://docs.x.com/changelog), [GIGAZINE](https://gigazine.net/gsc_news/en/20260209-x-api-pay-per-use)).
- Legacy **Basic** subscribers were auto-migrated to pay-per-use after 2026-06-01 ([secondary, citing devcommunity post 266305](https://www.simpmusic.org/blogs/x-legacy-basic-api-plans-moving-to-pay-per-use)) **[community]**. One source claims Pro migrates after 2026-09-01 **[unverified]**.
- Current rates ([official pricing](https://docs.x.com/x-api/getting-started/pricing)):

| Operation | Price |
|---|---|
| Create post | **$0.015** |
| Create post **containing a URL** | **$0.20** (a "we're live: twitch.tv/…" tweet costs $0.20) |
| Read post | $0.005 per post |
| Read user | $0.010 |
| **Owned reads** (your own mentions, posts, followers) | **$0.001** per resource |
| Activity API webhook events | e.g. `post.create` $0.005, `follow.follow` $0.010 |
| Cap | 3M post reads / month |
| Incentives (2026-10-02) | $20 credit on first card + match of first auto-recharge up to $50 (expire 3 months) |

- 2026-04-20: following, likes, and quote-posts via API were removed from all self-serve tiers ([changelog](https://docs.x.com/changelog)).

### 7.2 Auth for desktop

- OAuth 2.0 Authorization Code + **PKCE**. **Native App** and **Single page App** are public clients with no secret. Access tokens last 2 h. `offline.access` gives a refresh token. The auth code expires in 30 s ([official](https://docs.x.com/fundamentals/authentication/oauth-2-0/authorization-code)).
- Callback: use `http://127.0.0.1` ("not `localhost`"), exact match, max 10 callback URLs per app ([official](https://docs.x.com/fundamentals/developer-apps)).
- Developer Policy: no multiple apps for the same use case. Keep credentials private. Get explicit consent before automated replies/DMs. Don't circumvent rate limits ([official](https://docs.x.com/developer-terms/policy)).

### 7.3 X Live chat

- The **Livestream API** (RTMP sources, broadcasts, chat send/moderate) is "**limited to whitelisted apps**" via an access form ([official](https://docs.x.com/livestream-api/getting-started.md)).
- Reading live chat is the `broadcast.chat` **private** event on the X Activity API (scope `broadcast.read`), "delivered over a webhook you host" with CRC validation ([official](https://docs.x.com/livestream-api/chat/accessing-live-chat.md)). `broadcast.chat.remove` was added 2026-09-29.
- Sending: `POST /2/broadcasts/{id}/chat` (1–140 chars, `broadcast.write`) ([official](https://docs.x.com/x-api/broadcasts/send-a-chat-message-to-a-live-broadcast.md)).
- An HTTP stream (`GET /2/activity/stream`) exists with app bearer tokens for public events. Whether private events like `broadcast.chat` flow over it is undocumented ([official](https://docs.x.com/x-api/activity/introduction.md)) **[unverified]**.
- Net: **X Live chat is not realistically available** to a general-purpose local app.

### 7.4 Libraries

| Lang | Library | Latest | Notes |
|---|---|---|---|
| TS | XDK TypeScript (`xdevplatform/xdk-typescript`, official, generated) | npm 0.6.6 (2026-07) | young (65 stars), PKCE helpers |
| TS | `twitter-api-v2` (PLhery) | 1.29.1 (2026-08) | mature community lib |
| Rust | none official. `xdk-rs` 0.2.0 (independent, 2026-10, tiny). `twitter-v2` 0.1.8 (2022, stale). `egg-mode` 0.16.1 (2023, v1.1) | | thin REST client recommended |

**Recommendation:** treat X as an **action target** (post go-live / clip tweets, optionally read own mentions via owned reads) with **BYO X developer app and credits**, since the user pays per call. Use PKCE with a 127.0.0.1 loopback. No relay needed. X Live chat is out of scope.

---

## 8. Trovo, Rumble, Facebook Live, Discord

| Platform | Status / chat | Events | Auth / secret | Notes |
|---|---|---|---|---|
| **Trovo** | **Live streaming shut down 2026-06-30 00:00 GMT+8.** Onboarding stopped 2026-04-01 ([Streamlabs notice](https://streamlabs.com/content-hub/post/prepare-for-the-end-of-trovo-streaming)) | (was: WS `wss://open-chat.trovo.live/chat` with spells, subs, follows, raids ([docs](https://developer.trovo.live/docs/Chat%20Service.html))) | had implicit flow | **Drop.** Mix It Up and others still list it |
| **Rumble** | Official "Live Stream API": a per-user secret URL (managed at `rumble.com/account/livestream-api`) returning JSON with followers (50 recent), subscribers, gifted subs, **rants** (amount), chat (max 50 recent). Read-only. "v1.1" mentioned in future tense ([Rumble support](https://rumble.support/en/help/how-to-use-rumble-s-live-stream-api), updated 2025-11-20). Community wrappers use a 5 s minimum refresh **[community]** | polling | Read: user pastes URL, no OAuth. Send: session cookie from a Rumble web login | Unofficial real-time chat over SSE and unofficial **sending, deleting, pinning and muting** with a session cookie. Details, risks and our clean-room position in **8.1** |
| **Facebook Live** | Comments: SSE `GET https://streaming-graph.facebook.com/{live-video-id}/live_comments` or poll `/comments` ([official](https://developers.facebook.com/documentation/live-video-api/interact-with-viewers)) | reactions SSE. No Stars API found | Desktop: manual flow with `response_type=token` + redirect to `facebook.com/connect/login_success.html` (embedded webview) ([manual flow](https://developers.facebook.com/docs/facebook-login/guides/advanced/manual-flow)), or **Login for Devices** using `APP_ID\|CLIENT_TOKEN` (tokens up to 60 days, no refresh) ([devices](https://developers.facebook.com/docs/facebook-login/for-devices)). Long-lived token exchange **requires app secret** ([official](https://developers.facebook.com/docs/facebook-login/guides/access-tokens/get-long-lived)) | Page features need **App Review + Business Verification** for Advanced Access ([access levels](https://developers.facebook.com/docs/graph-api/overview/access-levels)). Groups API removed (v19, 2024). Go-live requires account ≥60 days old and ≥100 followers (2024-06-10). Graph v26.0 (2026-07-29) ([changelog](https://developers.facebook.com/docs/graph-api/changelog)). **Low priority** |
| **Discord** | Outbound: **channel webhook URL** (user pastes it; no auth). Bot: Gateway WS v10 with **BYO bot token** (a secret per application, so it can't be shipped shared) | n/a | OAuth: all token calls require `client_secret`. PKCE not documented. `webhook.incoming` scope returns a webhook but needs code exchange with a secret ([official](https://docs.discord.com/developers/topics/oauth2)) | Rate limits: 50 req/s global. Gateway 120 events/60 s per connection. 1,000 IDENTIFY/24 h ([rate limits](https://docs.discord.com/developers/topics/rate-limits), [gateway](https://docs.discord.com/developers/events/gateway)). `MESSAGE_CONTENT` is privileged, with review required once an app reaches 10k users (2026-06-10 change). Libs: Rust `serenity` 0.12.5, `twilight` 0.17.1 (2025-12). TS `discord.js` 14.27.0 (2026-07) |

**Recommendation:** Discord go-live posts via pasted webhook URL (green, local). Optional BYO bot token for Discord-chat ingestion. Rumble via pasted API URL (polling) with optional unofficial SSE, and unofficial session-cookie sending only as an explicit opt-in (8.1, owner question Q-C8). Defer Facebook. Remove Trovo.

### 8.1 Rumble chat in detail: official read, unofficial real-time read and send (updated 2026-10-19, DEN-54)

**Official surface.** The only official interface is the Live Stream API URL. Rumble's help article lists followers, subscribers, gifted subs, chat messages (with badges) and rants (with amount), title, likes, categories and "Watching Now". It describes **no write or send action**. It says "Authentication is not required for this version of the API", and the URL contains the user ID and the live stream key. About the future it says only "Our v1.1 live streaming API will allow you to integrate real time notifications and overlays" and that Rumble will partner with some apps for "a deeper and robust integration" ([Rumble support](https://rumble.support/en/help/how-to-use-rumble-s-live-stream-api), last updated 2025-11-20) **[official]**. We found no published v1.1 and **no official chat-send endpoint** as of 2026-10-19 (searches of Rumble's help center and the web) **[unverified absence]**.

**Reference studied: cocorum.** [`thelabcat/rumble-api-wrapper-py`](https://github.com/thelabcat/rumble-api-wrapper-py) (published on PyPI as `cocorum`, v2.19.0-beta released 2026-10-16, Python, 3 stars, actively maintained). It says it is unofficial and "not officially endorsed by Rumble Corp." **Licence: Apache-2.0** (`LICENSE.txt`, GitHub licence API: `apache-2.0`). Apache-2.0 is one-way compatible with GPL-3.0, so including Apache-2.0 code in a GPL-3.0-or-later work would be legally possible. **Clean-room position for StreamDen anyway:** we used cocorum only to learn endpoints, request and response shapes and observed behaviour, listed below. **No code was copied** into this repo, and a Rust implementation must be written from these notes and from our own traffic observation, not translated from cocorum. That is the AGENTS.md clean-room rule applied to an unofficial reverse-engineered client. It also avoids Apache-2.0 attribution and NOTICE obligations in the app. Everything in the table is **[community]** (one library's reading of Rumble's private web API), checked against cocorum's source at master on 2026-10-19. None of it is documented by Rumble.

| Item | What cocorum does (behaviour only) | Notes |
|---|---|---|
| Stream (chat) ID | Numeric video/stream ID, used in base 10 for the chat API and base 36 elsewhere. Obtainable from the official Live Stream API response, the pop-out chat URL `rumble.com/chat/popup/<id>`, or the embed URL `rumble.com/embed/v<id>` | The official API gives us the ID, so no scraping is needed to find it |
| Real-time read | SSE `GET https://web7.rumble.com/chat/api/chat/{stream_id_b10}/stream`, **no login needed**. First event `type: "init"` carries config (`message_length_max`, rants enabled, badge definitions), users, channels and recent messages. Later events: `messages`, `delete_messages`, `delete_non_rant_messages`, `pin_message` (and unpin). Messages carry rant price/duration, `raid_notification` and `gift_purchase_notification` | No SSE event IDs, so no resume after reconnect (same as Twitch: no replay). Silence is normal. Reconnect after ~90 s of no bytes. **HTTP 204 on reconnect means the chat has ended.** 204 also for a nonexistent stream |
| Send | `POST https://web7.rumble.com/chat/api/chat/{stream_id_b10}/message` with the session cookie and JSON body `{"data": {"request_id": <random id>, "message": {"text": "..."}, "rant": null, "channel_id": <int or null>}}`. Success is HTTP 200 with `data.id` (new message ID) and `data.user` | `channel_id` selects which of the user's channels speaks (Rumble accounts can own several channels). cocorum checks a CORS `OPTIONS` preflight with `Origin: https://rumble.com` first |
| Limits used by the client | Max **200** characters (client constant; the `init` event's `message_length_max` is authoritative). Cooldown **3 s** between sends (client constant) | **[unverified]** server-side limits. Rumble publishes no chat rate limits. Read `message_length_max` at runtime and never hard-code |
| Delete | `DELETE …/chat/api/chat/{stream_id_b10}/message/{message_id}` with the cookie | Needs moderator or owner rights |
| Pin / unpin | `POST https://rumble.com/service.php?name=chat.message.pin` (or `chat.message.unpin`), form fields `video_id`, `message_id` | |
| Mute / unmute (Rumble's ban/timeout) | `service.php?name=moderation.mute`, form fields `user_to_mute`, `entity_type` (`user`/`channel`), `video`, `duration` (seconds), `type` (`video` = this stream, `total` = all of the channel's streams). Unmute: `moderation.unmute` with a `record_id` that cocorum **scrapes from the HTML page** `rumble.com/account/moderation/muting` | Unmute depends on HTML scraping, so it's the most fragile action |
| Native commands | `POST https://rumble.com/chat/command`, form fields `video_id`, `message` (text starting with `/`) | Which commands exist is undocumented |
| Rants (paid messages) | The send body has a `rant` field, but cocorum always sends `null` | **Out of scope**: sending a rant spends money |
| Session | Cookie **`u_s`** (session token). Validity test: `GET https://rumble.com/-htmx/web-services/user-login` with the cookie returns **409 when logged in** and 200 when not | Logout: `service.php?name=user.logout` |
| Password login (not recommended for us) | `service.php` calls `user.get_salts` → `user.2fa.first_step` → `user.login`, with a proprietary multi-round MD5 password hash. With 2FA on: `user.2fa.request_email_code` / `request_sms_code` / authenticator, then `user.2fa.verify_totp`, whose `Set-Cookie` carries `u_s` | Reimplementing Rumble's password hashing is brittle and means handling the raw password. Use the web login below |

**Breakage evidence (fragility is real, not hypothetical).** cocorum [issue #35](https://github.com/thelabcat/rumble-api-wrapper-py/issues/35) (2026-10-14, "Token testing is broken again": HTTP 403) and [PR #36](https://github.com/thelabcat/rumble-api-wrapper-py/pull/36) (2026-10-16). Probes in #36 found that Cloudflare challenged **browser-like User-Agents** on `service.php` with 403 `cf-mitigated: challenge`, while an honest library UA got normal JSON. HTML pages were challenged from a datacenter IP whatever the UA. So `service.php` (login, pin, mute) can change bot-management behaviour from week to week, while the `web7` chat SSE and message endpoints were unaffected in those probes **[community]**.

**Terms of service.** Rumble's Terms (last modified 2026-09-01), section "Restrictions on Automated Software": "You are prohibited from employing any form of automated software to interact with or access the Rumble Site". The section covers scripts and bots and applies unless Rumble gives prior written permission. "Proprietary Rights and Restrictions" also bars "systematic retrieval of data or Content" ([terms](https://rumble.com/s/terms)) **[official]**. The terms say nothing specific about unofficial APIs or credential sharing. **Assessment [inference]:** sending chat as the logged-in streamer through the private API is automated access without written permission. The risk is account action, mainly against the **streamer's** account, which is worse than losing a feature. Rumble gives no published enforcement signal either way. The official Live Stream API is plainly meant for third-party tools, which is why the **read** side is safe and the **send** side isn't.

**Credential handling (if the owner accepts the risk, Q-C8).**
- **Sign-in:** a dedicated StreamDen login window (Tauri webview) opens `https://rumble.com` and the user logs in normally, including 2FA and any Cloudflare check, which a real browser engine passes more readily. On success, the app reads the `u_s` cookie for `https://rumble.com` (Tauri 2 `Webview::cookies_for_url`, merged 2025-03-15 in [tauri#12665](https://github.com/tauri-apps/tauri/pull/12665); returns HttpOnly cookies, behaviour per platform **[unverified]**, so spike it). Then it closes the window and clears the webview's cookie store. Rumble (unlike Google) has no stated rule against embedded-webview login **[inference]**. **Fallback:** an "advanced" field to paste the `u_s` value from the browser's dev tools, with a warning. Never ask for the password, and never implement the MD5 password flow.
- **Storage:** the `u_s` value is a `Secret` (Spec 02 secret typing) stored only in the OS keychain (`keyring`), never in settings files, logs, crash reports, telemetry, MCP tool output or exported configs. Redact `Cookie:` and `Set-Cookie:` headers in HTTP debug logs.
- **Liveness:** validate with the `user-login` 409 check at connect and after any 401/403 from a send. Don't poll it. Expired or invalid cookie → the target goes to **"Rumble: sign in again"**, and reading keeps working.
- **Revocation:** a "Sign out" button calls `user.logout` (invalidates the session server-side) and deletes the keychain entry. The runbook tells the user that changing the Rumble password or "log out of all sessions" (if Rumble offers it, check on screen) also kills the cookie.

**Breakage detection and graceful degradation.**
- The connector has two independent halves: **read** (official URL polling, plus optional SSE) and **send** (cookie). Either can fail without affecting the other.
- Send states shown in the composer: `ready` · `signed-out` · `session-expired` · `rate-limited (retry in N s)` · `blocked-by-rumble` (403 with a Cloudflare challenge, or an HTML body where JSON was expected) · `api-changed` (200 without `data.id`, or a schema mismatch) · `not-live` (no current stream ID).
- On `api-changed` or `blocked-by-rumble`, stop retrying, mark Rumble **read-only** with the reason and a "check for update" link, and never fall back to a riskier method automatically.
- Golden payloads (recorded from a test stream and scrubbed) for `init`, message, delete and pin events, plus the send response, go in WS-16 fixtures. A weekly ecosystem-brief check (DEN-18) watches cocorum's releases and issues as an early warning.

**Rumble send verdict:** technically feasible today (send, delete, pin, mute) with one session cookie and a dozen endpoints. It's **unofficial, against the letter of Rumble's ToS, and demonstrably fragile**. Recommended form: an opt-in "unofficial" toggle, off by default, behind a clear risk notice. Phase and acceptance are owner decisions (**Q-C8**, **Q-S8**).

---

## 9. OBS Studio (obs-websocket v5), Streamlabs Desktop, Meld Studio

### 9.1 obs-websocket 5.x

| Item | Detail | Source |
|---|---|---|
| Availability | Bundled with OBS ≥ 28. Default port **4455** (Tools menu). CLI overrides `--websocket_port/password/debug/ipv4_only` | [README](https://github.com/obsproject/obs-websocket) |
| Versions | OBS 31.0 bundled 5.5.4. OBS 32.0 → 5.6.3. OBS 32.1 → 5.7.2 (**Canvases**). OBS 32.2.2 → 5.7.4. **OBS 33.0 (rc2, 2026-10-16) → 5.7.6**. RPC version 1 | GitHub tags/submodules |
| Encoding | subprotocol `obswebsocket.json` (text) or `obswebsocket.msgpack` (binary) | [protocol](https://github.com/obsproject/obs-websocket/blob/master/docs/generated/protocol.md) |
| Handshake | Server `Hello` (op 0, may contain `authentication {challenge, salt}`). Client `Identify` (op 1, `rpcVersion`, `authentication`, `eventSubscriptions` bitmask). `Identified` (op 2). `Reidentify` (op 3) to change subscriptions | same |
| Auth string | `secret = base64(sha256(password + salt))`, `auth = base64(sha256(secret + challenge))` | same |
| Ops | `Event` (5), `Request` (6), `RequestResponse` (7), `RequestBatch` (8, execution types None/SerialRealtime/SerialFrame/Parallel), `RequestBatchResponse` (9) | same |
| Event subscriptions | categories General, Config, Scenes, Inputs, Transitions, Filters, Outputs, SceneItems, MediaInputs, Vendors, Ui, **Canvases**. **High-volume opt-in:** `InputVolumeMeters` (1<<16), `InputActiveStateChanged` (1<<17), `InputShowStateChanged` (1<<18), `SceneItemTransformChanged` (1<<19) | same |
| Request groups | General (incl. `BroadcastCustomEvent`, `CallVendorRequest`, hotkeys), Config, Sources (screenshots), **Canvases** (`GetCanvasList`), Scenes, Inputs, Transitions, Filters, Scene Items, Outputs, Stream, Record, Media Inputs, Ui | same |
| Gotchas | **Safe Mode doesn't start the WebSocket server** (OBS 33). Always surface "OBS not reachable" clearly | [OBS 33 rc notes](https://github.com/obsproject/obs-studio/releases) |
| Libraries | Rust **`obws` 0.15.0** (2026-03. GitHub repo archived, moved to forge.dnaka91.rocks, a maintenance-risk signal). TS **`obs-websocket-js` 5.0.8** (2026-03, types for 5.7.2) | [docs.rs](https://docs.rs/crate/obws/latest), npm |

`BroadcastCustomEvent` plus obs-browser's dispatch of custom vendor events into browser sources lets the app push events into OBS-hosted pages, but our own local WebSocket is simpler and works outside OBS too.

### 9.2 Streamlabs Desktop

Local remote-control API: enable it under **Settings → Remote Control** (off by default) and copy the token from "Show details". Default **127.0.0.1:59650**. It speaks JSON-RPC over a SockJS WebSocket, or the named pipe `slobs` ([Streamer.bot setup](https://docs.streamer.bot/guide/broadcasters/streamlabs-desktop), [API docs repo](https://github.com/stream-labs/streamlabs-obs-api-docs)). The exact SockJS path (`/api`) and service list are **[unverified]** because the rendered docs 404'd. Python wrapper: PySLOBS. Streamer.bot still ships fixes for it (crash on out-of-spec errors, v1.1.0 Oct 2026) ([changelog](https://docs.streamer.bot/changelogs/v1.1.0)).

### 9.3 Meld Studio

- **Windows 10+ and macOS 12+ only. No Linux** ([Meld docs](https://meldstudio.co/docs/)).
- Remote control requires enabling "Allow remote connections" under Advanced. Streamer.bot connects by host (default 127.0.0.1) and offers scene/layer show/hide plus status ([Streamer.bot](https://docs.streamer.bot/guide/stream-apps/meld-studio)).
- **No public protocol documentation was found.** Community lore says it's QWebChannel over WebSocket **[unverified]**. Integrating means reverse-engineering or contacting Meld.

---

## 10. Donation / tip / commerce platforms

| Platform | Delivery | Public URL needed? | Auth | Events | Local-only strategy | Source |
|---|---|---|---|---|---|---|
| **Streamlabs** | Socket.IO 2.x: `https://sockets.streamlabs.com?token=<socketToken>`, `transports: ['websocket']`, event `event` (payload `message` is an array) | **No** | Socket API token (copy from Dashboard → Settings → API Settings) or `/socket/token` via OAuth | donation, follow/sub/bits/raids (Twitch), YouTube sub (`follow`), sponsor, superchat, merch, charity | **Outbound socket, fully local.** OAuth apps need approval (unapproved = max 10 whitelisted users) and a secret. Avoid OAuth and use pasted socket token. A community report says one Streamlabs widget may need to be open for events to flow **[community]** | [socket API](https://dev.streamlabs.com/docs/socket-api), [register](https://dev.streamlabs.com/docs/register-your-application) |
| **StreamElements** | **Astro** WebSocket `wss://astro.streamelements.com/`: subscribe `{topic, room, token, token_type: jwt\|apikey\|oauth2}`. PING every 30 s (70 s timeout). Reconnect tokens. 1 command/100 ms | **No** | JWT (dashboard "Show secrets"), Overlay token (`apikey`), or OAuth2 (application access unclear) | `channel.activities` (tip, follow, subscriber, cheer, raid, merch, superchat, sponsor, hypetrain*, charity…, multi-provider incl. Kick), `channel.tips`, `channel.session.update`, `channel.chat.message`, `channel.overlay.broadcast`, etc. | **Outbound socket, fully local.** Legacy socket.io `realtime.streamelements.com` still in the [GitHub docs](https://github.com/StreamElements/api-docs/blob/main/docs/Websockets.md) with no deprecation notice found. Prefer Astro. JWT reportedly rotates ~every 2 weeks **[community]** | [websockets](https://docs.streamelements.com/websockets), [topics](https://docs.streamelements.com/websockets/topics), [activities](https://docs.streamelements.com/websockets/topics/channel-activities) |
| **Tipeeestream** | Socket.IO: discover host via `GET https://api.tipeeestream.com/v2.0/site/socket` (e.g. `sso-cf.tipeeestream.com:443`), `?access_token=API_KEY`, emit `join-room {room: API_KEY, username}`, listen `new-event`. Also REST `GET /v1.0/events.json?apiKey=` for polling | **No** | API key | donation, follow, subscription, hosting (cheer historically) | **Local** (socket or polling) | [socket](https://api.tipeeestream.com/api-doc/socketio), [events](https://api.tipeeestream.com/api-doc/events) |
| **Ko-fi** | **Webhooks only.** Form-encoded `data=` JSON with `verification_token`. Types Donation / Subscription / Commission / Shop Order. No public REST API | **YES** | verification token | as listed | **Relay or tunnel required.** No polling alternative. Streamer.bot uses its website-hosted webhook URL. SAMMI's Ko-fi extension uses the author's DigitalOcean relay (`…ondigitalocean.app/kofihook/<key>`) | [Streamer.bot](https://docs.streamer.bot/guide/integrations/ko-fi), [SAMMI ext](https://docs.christinak.ca/docs/extensions/ko-fi), field names [community](https://apis.io/asyncapis/ko-fi/ko-fi-asyncapi/) (official help page returned 403) |
| **Patreon** | Webhooks v2 (`members:create/update/delete`, `members:pledge:*`, `posts:*`). `X-Patreon-Signature` = HMAC-**MD5** hex. Retries up to 1 week. Or **poll** API v2 with creator access token | Webhooks yes. **Polling no** | Creator access token (user registers a client in the Patreon portal and copies it). OAuth needs client secret | members/pledges | **Local polling** with a BYO creator token (100 req/min per token). Webhooks via relay for instant alerts | [docs.patreon.com](https://docs.patreon.com/) |
| **Fourthwall** | Webhooks: `X-Fourthwall-Hmac-SHA256` (base64 HMAC-SHA256). 5 delivery attempts with 5 s timeout. Events: order placed/updated, **donation**, gift purchase, subscription purchased/changed/expired, thank-you sent, gift draw started/ended, newsletter, product/promo/collection, cart abandoned | **YES** (webhooks) | shop API key (Basic auth) or OAuth apps | as listed | Relay/tunnel. Polling via the Open API with shop key may be possible **[unverified]** | [webhooks](https://docs.fourthwall.com/webhooks/), [signatures](https://docs.fourthwall.com/webhooks/signature-verification), [retries](https://docs.fourthwall.com/webhooks/retry-policies), [index](https://docs.fourthwall.com/llms.txt) |
| **Throne** | Webhooks (added 2026). HTTPS POST, respond 2xx within 10 s. Envelope `{contract_version, event_id, event_type, data}`. **Ed25519** signature (`X-Signature-Ed25519`, `X-Signature-Timestamp`, public key published). Events `gift_purchased`, `contribution_purchased`, `gift_crowdfunded` | **YES** | none (verify signature) | gifts, contributions | Relay/tunnel. No public polling API | [Throne help](https://help.throne.com/en/articles/14982436-webhook-integration) |
| **Buy Me a Coffee** | Webhooks (HMAC-SHA256 in `x-signature-sha256` per secondary source) | **YES** | webhook secret | donations, memberships, extras, commissions **[community]** | Relay/tunnel | [community](https://apis.io/apis/buymeacoffee/buymeacoffee-webhooks-api/) |
| **Tiltify** | v5 webhooks (app token with `webhooks:write`) or REST polling | Webhooks yes. Polling no | app token (client credentials, secret) | donations, campaign facts | Poll public campaign endpoints **[unverified details]** | search results only |
| **DonorDrive / Extra Life** | Public REST API (`/api/participants/{id}/...`) with ETag/`If-None-Match` → 304 per DonorDrive's PublicAPI overview (seen via search snippet) **[community]** | **No** | none | participant donations, milestones | **Local polling**. Streamer.bot supports it with a "custom API endpoint" option | [Streamer.bot](https://docs.streamer.bot/guide/integrations/donordrive) |
| **Shopify** | webhooks | **YES** | verify token | orders | Relay | [Streamer.bot](https://docs.streamer.bot/guide/integrations/shopify) |

Streamer.bot routes **Ko-fi, Patreon, Fourthwall, Throne, Shopify** (and Kick) through its **Website Integration**. The user creates a webhook on streamer.bot, pastes that URL into the vendor, and the instance must stay logged in to receive events ([Ko-fi](https://docs.streamer.bot/guide/integrations/ko-fi), [Patreon](https://docs.streamer.bot/guide/integrations/patreon), [Fourthwall](https://docs.streamer.bot/guide/integrations/fourthwall), [Throne](https://docs.streamer.bot/guide/integrations/throne), [website integration](https://docs.streamer.bot/guide/integrations/streamerbot)). StreamElements and Streamlabs connect directly (OAuth / socket token) ([SE](https://docs.streamer.bot/guide/integrations/streamelements), [SL](https://docs.streamer.bot/guide/integrations/streamlabs)).

---

## 11. Cloud relay: when do we need one?

### 11.1 Features that cannot work purely locally

| Feature | Why | Workaround without our relay |
|---|---|---|
| **Kick events** (chat read, follows, subs, gifts, KICKs, rewards, bans, live status) | Webhooks are the only official delivery. One URL per app. 1,000 chat subs per unverified app | BYO Kick app + user tunnel (section 11.2a). Or unofficial Pusher (read-only, unsupported) |
| **Kick OAuth** (token exchange + refresh) | `client_secret` required even with PKCE | BYO Kick app (user supplies secret locally). Or ship the secret (not recommended) |
| **Ko-fi** | webhook-only, no API | user tunnel |
| **Throne, Fourthwall, Buy Me a Coffee, Shopify** | webhook-only (Fourthwall polling unverified) | user tunnel |
| **Patreon instant alerts** | webhooks | **Polling with a creator token works locally** (seconds-to-minutes latency) |
| **X Live chat** | webhook you host + whitelisted access | none realistic |
| **Facebook long-lived tokens** | exchange needs app secret | device login (60-day tokens, re-login) |
| **Discord `webhook.incoming` OAuth** | code exchange needs secret | **user pastes a webhook URL** (better UX anyway) |
| **Streamlabs / StreamElements OAuth** | secret and app approval | **paste socket token / JWT** (local) |
| **Twitch Chat Bot badge / conduits / app tokens** | client credentials need secret | skip (cosmetic). User-token chat works fine |
| **TikTok signing** | third-party signer (Euler) | not our relay. It's a vendor dependency |
| **YouTube shared quota** | not a relay issue but a shared-credential issue | BYO project, quota audit |

Everything else (Twitch, YouTube chat, OBS, Streamlabs Desktop, Discord webhook posting, Streamlabs/StreamElements/Tipeeestream sockets, Patreon/DonorDrive/Rumble polling, X posting) is **purely local**.

### 11.2 Options

**(a) User-run tunnel: zero infrastructure for us**

The app runs a local HTTPS-terminating webhook listener (only `/hooks/<provider>/<random-path>`, with signatures verified locally) and helps the user expose it.

| Tunnel | Stable URL? | Cost / limits | Fit | Source |
|---|---|---|---|---|
| **Tailscale Funnel** | **Yes** (`<node>.<tailnet>.ts.net`) | free on all plans. Ports 443/8443/10000. Beta. Unspecified bandwidth caps | **Good for power users** | [docs](https://tailscale.com/kb/1223/funnel) |
| **ngrok** free | **Yes**: 1 assigned dev domain (`*.ngrok-free.app`) | 20k HTTP req/month, 1 GB, 3 endpoints. Interstitial only for HTML **browser** traffic (webhook POSTs from servers shouldn't hit it per docs, but forum reports conflict). Hobbyist $10/mo | Good | [pricing](https://ngrok.com/pricing), [free limits](https://ngrok.com/docs/pricing-limits/free-plan-limits) |
| **Cloudflare named Tunnel** | **Yes** | free, but needs **a domain on Cloudflare** | Good if user owns a domain | [get started](https://developers.cloudflare.com/tunnel/get-started/) |
| Cloudflare **Quick Tunnel** (`trycloudflare.com`) | **No**: hostname changes every run | no account. 200 in-flight requests. **No SSE. No uptime guarantee. "testing and development"** | **Poor**: user must re-paste the URL into Ko-fi etc. on every restart | [quick tunnels](https://developers.cloudflare.com/tunnel/get-started/quick-tunnels/) |
| **zrok** | Yes (reserved shares) | OSS (Apache-2.0), free tier 5 GB/day, **self-hostable** | Good. Embeddable | [pricing](https://zrok.io/pricing/) |
| Pinggy | No on free (60-min timeout) | SSH-based | Poor | [pinggy](https://pinggy.io/) |
| bore | n/a | TCP only, no TLS | Unsuitable | [repo](https://github.com/ekzhang/bore) |
| smee.io / Hookdeck CLI | smee: channel URL stable | dev tools (SSE relay / guest accounts) | Dev only | [smee](https://smee.io/), [hookdeck](https://hookdeck.com/docs/cli) |

Pros: no operator burden, max privacy. Cons: setup friction, the PC must be on to receive (lost or delayed events when off, and Kick auto-unsubscribes after a day of failures), and it doesn't solve **Kick's shared-app** model (fine only with BYO Kick app).

**(b) Minimal open-source, self-hostable relay (recommended to build)**

Cloudflare Worker + one **Durable Object per user "mailbox"**:

```
Vendor webhook ──POST──▶ https://relay.example/h/<mailboxId>/<provider>
                              │ (Worker: size limits, rate limit, 200 OK fast)
                              ▼
                     Durable Object(mailboxId)  ── SQLite queue (TTL, e.g. 7 days)
                              ▲  ▼ WebSocket (Hibernation API)
Desktop app ── wss://relay.example/ws/<mailboxId> (bearer = mailbox secret, outbound only)
```

- **Dumb pipe by default.** Forward raw body + headers. The **desktop verifies signatures** (Kick RSA, Throne Ed25519, Fourthwall HMAC, Patreon HMAC-MD5, Ko-fi token), so vendor secrets never live on the relay. Optional E2E: encrypt at ingress with the desktop's public key, so stored backlog is unreadable to the operator.
- **Offline buffering** solves "PC was off" and keeps vendors from disabling the webhook (Kick unsubscribes after a day of failures. Fourthwall gives up after 5 attempts).
- **Kick demux:** one app-level webhook URL, so the relay routes by `broadcaster_user_id` to the mailbox registered when the user linked Kick.
- **Token broker** endpoints for secret-requiring OAuth (Kick first; optionally Discord, Streamlabs, Patreon, Facebook long-lived exchange, Twitch app tokens). The broker takes `code + code_verifier + redirect_uri`, adds `client_secret`, and returns tokens. It **never stores tokens**. Refresh goes through the same path. Rate-limit per mailbox.
- **Cost (Cloudflare):** Workers Free = 100k requests/day and 10 ms CPU/request. **Durable Objects are on the Free plan** (SQLite backend: 100k requests/day, 5 GB). WebSocket Hibernation means idle sockets accrue no duration charges. Incoming WS messages bill at 20:1 and outgoing messages are free ([DO pricing](https://developers.cloudflare.com/durable-objects/platform/pricing/), [Workers limits](https://developers.cloudflare.com/workers/platform/limits/), [WS hibernation](https://developers.cloudflare.com/durable-objects/best-practices/websockets/)). A self-hoster's personal relay fits comfortably in the free tier.
- Also provide a plain Docker/Rust implementation (same protocol) for people who don't want Cloudflare.

**(c) Optional hosted relay (operated by the project)**

Run option (b) as a default "Streamer.live Relay" so Kick and Ko-fi "just work". Requirements: privacy policy, abuse handling, Kick app verification (to lift the 1,000 chat-subscription cap), uptime monitoring, and a budget (the Workers Paid plan is needed beyond free-tier volume; verify current pricing). Make it **opt-in per integration**, keep it open source, and let users point the app at a self-hosted relay URL.

### 11.3 Why Streamer.bot routes some things through its servers

- **Kick**: official events are webhook-only, use one URL per app, and need a confidential-client token exchange. Streamer.bot's "Website Integration" receives and forwards them, and shows a "Kick API Delay" status ([docs](https://docs.streamer.bot/guide/platforms/kick)).
- **Webhook-only vendors** (Ko-fi, Patreon, Fourthwall, Throne, Shopify): the user creates the webhook URL and verify-secret on streamer.bot, and events are forwarded to the running instance ([docs](https://docs.streamer.bot/guide/integrations/streamerbot)).
- **Product features and monetization**: Custom Webhooks (supporter perk, "monitored for abuse"), encrypted Cloud Backups, and Decks are gated behind Patreon tiers ([v1.0.0](https://docs.streamer.bot/changelogs/v1.0.0)).
- **Shared credentials**: one Google project, with the shared YouTube quota problems described above ([FAQ](https://docs.streamer.bot/faq/youtube-quota)).
- Likely also: keeping vendor client secrets off user machines, which is the same reason Kick's own issue [#413](https://github.com/KickEngineering/KickDevDocs/issues/413) cites **[inference]**.
- SAMMI's Ko-fi extension does the same with a creator-hosted relay ([docs](https://docs.christinak.ca/docs/extensions/ko-fi)). Even "local" tools centralize webhook ingress.

---

## 12. OBS custom browser docks and browser-source overlay best practices

### 12.1 Custom browser docks

- **Where:** `Docks → Custom Browser Docks…` (the Docks menu exists since OBS 30, which also added Full-Height Docks). Give it a name and URL, e.g. `http://127.0.0.1:<port>/dock/chat` ([OBS 30 notes](https://github.com/obsproject/obs-studio/releases/tag/30.0.0), locale key `Basic.MainMenu.Docks.CustomBrowserDocks` in [en-US.ini](https://github.com/obsproject/obs-studio/blob/master/frontend/data/locale/en-US.ini)). OBS persists the docks. Docks are interactive (mouse and keyboard), unlike browser sources.
- **Engine:** same CEF as browser sources. CEF **127** since OBS 31. **OBS 33 (rc2, 2026-10-16) jumps to CEF 150** ("massive update"). Its RC notes include fixes for "local network requests failing in browser sources" and a security fix so the local-file handler can't be invoked from remote insecure pages ([33.0 rc notes](https://github.com/obsproject/obs-studio/releases)). OBS 32.1 "improved security of browser sources using local files". **Serve docks and overlays from `http://127.0.0.1:<port>` rather than `file://`**, and keep the page and its WebSocket same-origin. That sidesteps Chromium private/local-network-access restrictions.
- **Linux/Wayland caveat (important for this project's Omarchy/Hyprland user base):** obs-browser disables browser panels on native Wayland. `get_browser_lib()` returns null with the comment "Disable panels on Wayland for now", and the README says docks work "on all supported platforms, except for Wayland (Linux)" ([source](https://github.com/obsproject/obs-browser/blob/master/panel/browser-panel.hpp), [README](https://github.com/obsproject/obs-browser)). Workarounds: run OBS under XWayland (`QT_QPA_PLATFORM=xcb obs`), or offer our own **standalone always-on-top chat window** as the dock alternative on Wayland.
- OBS's CEF user agent includes `OBS/<version>`, so a page can detect it's inside OBS ([obs-browser-plugin.cpp](https://github.com/obsproject/obs-browser/blob/master/obs-browser-plugin.cpp)).
- Whether `window.obsstudio` control functions work inside **docks** is **[unverified]**. Don't rely on it. Docks should talk to our local WebSocket (and through us, obs-websocket).
- OBS 32.2 fixed "OAuth and dock state save corruption", and 32.1 rearranged default docks and disabled dock animations ([release notes](https://github.com/obsproject/obs-studio/releases)).

### 12.2 Browser-source overlays

| Topic | Practice | Source |
|---|---|---|
| Transparency | OBS injects default Custom CSS `body { background-color: rgba(0,0,0,0); margin: 0px auto; overflow: hidden; }`. Keep `html, body` transparent, never set opaque page backgrounds, and don't rely on users editing Custom CSS | [obs-browser-plugin.cpp](https://github.com/obsproject/obs-browser/blob/master/obs-browser-plugin.cpp), [KB](https://obsproject.com/kb/browser-source) |
| `window.obsstudio` | `pluginVersion`, `getControlLevel`, `getCurrentScene`, `getStatus`, `getScenes`, `setCurrentScene`, `getTransitions`, `get/setCurrentTransition`, start/stop streaming/recording/replay buffer/virtualcam, pause/unpause recording, `saveReplayBuffer`. **Page permission levels:** 0 None, 1 ReadObs (**default**), 2 ReadUser, 3 Basic, 4 Advanced, 5 All. Types: `@types/obs-studio` | [README](https://github.com/obsproject/obs-browser), [browser-app.cpp](https://github.com/obsproject/obs-browser/blob/master/browser-app.cpp) |
| OBS events in page | `obsSceneChanged`, `obsSceneListChanged`, `obsTransitionChanged`, `obsSourceVisibleChanged`, `obsSourceActiveChanged`, `obsStreamingStarting/Started/Stopping/Stopped`, recording/replay/virtualcam events, `obsExit`, plus custom vendor events from obs-websocket | [README](https://github.com/obsproject/obs-browser) |
| Lifecycle | "Shutdown source when not visible" and "Refresh browser when scene becomes active" (both off by default) mean overlays get reloaded. Make them **stateless**: on load, fetch a snapshot (goals, queue) from the local server, then subscribe. Pause animations on `obsSourceVisibleChanged=false` | [KB](https://obsproject.com/kb/browser-source) |
| WebSocket reconnect | Exponential backoff with jitter (e.g. 250 ms × 2ⁿ, capped ~10–30 s), app-level heartbeat (ping every 15–30 s, reconnect on 2 missed), resume with a `lastEventId` cursor so the server can replay buffered alerts, dedupe by event ID, and show a subtle "reconnecting" state only in docks, never on stream | design guidance |
| Audio | Enable "Control audio via OBS" (`reroute_audio`) for TTS and alert sounds so they hit the OBS mixer. OBS sets Chromium `autoplay-policy=no-user-gesture-required`, so audio can autoplay without a click | [obs-browser-plugin.cpp](https://github.com/obsproject/obs-browser/blob/master/obs-browser-plugin.cpp), [browser-app.cpp](https://github.com/obsproject/obs-browser/blob/master/browser-app.cpp) |
| Performance | Default 30 FPS (custom up to 60). Animate with CSS transforms and opacity or `requestAnimationFrame`. Avoid large blur/backdrop-filter. Linux browser-source hardware acceleration landed in obs-browser (PR #453, 2025-04) | [KB](https://obsproject.com/kb/browser-source), [obs-browser PRs](https://github.com/obsproject/obs-browser/pulls?q=is%3Apr+hardware+accel) |
| Sizing | Size the source to the canvas (e.g. 1920×1080) and lay out with CSS, rather than scaling the source in OBS (avoids blurry text) | general practice |

---

## 13. Open questions for the project owner

1. **Hosted relay appetite:** will the project operate an optional hosted relay (cost, privacy policy, abuse handling, Kick app verification), or only ship a self-hostable relay plus tunnel guidance?
2. **Kick strategy:** accept the relay dependency for official events, offer BYO-Kick-app + tunnel for purists, and decide whether to include the **unsupported** Pusher read-only fallback that Kick staff explicitly discourage.
3. **YouTube credentials:** one shared Google project (needs OAuth verification + quota audit, all users share 10k/day until approved) vs BYO project/key (policy gray zone, 7-day testing-token trap) vs both. Is an InnerTube/scraping "unofficial mode" acceptable given it violates YouTube's Developer Policies?
4. **TikTok:** include an unofficial connector at all? **AGPL-3.0** (TikTok-Live-Connector) and "modified AGPL" (TikTokLive) licensing conflicts with most app licenses unless it's isolated as a separate process/plugin. There's also the dependency on Euler Stream (free tier 2,500 req/day) and the ban risk for chat sending.
5. **X:** fine to require users to bring their own X developer app and pay-per-use credits ($0.20 per go-live tweet with a link)?
6. **Twitch client registration:** confirm a **Public** client + DCF (no secret, refresh tokens single-use with 30-day inactivity expiry). This rules out the Chat Bot badge and conduits unless we add the relay. Client IDs are effectively permanent because rewards are bound to them.
7. **Linux/Wayland:** OBS custom browser docks don't work on native Wayland. Should the app ship its own native multichat window for Wayland users, or document the XWayland workaround?
8. **Platform priority:** Trovo is dead (drop it). Meld Studio is Windows/macOS-only with an undocumented protocol. Facebook Live needs App Review + Business Verification. Which of these are in scope for v1?
9. **Core language:** Rust has solid Twitch (`twitch_api` / `twitch_oauth2`), gRPC (`tonic`), and OBS (`obws`, though its repo moved off GitHub) support. TypeScript has more mature platform libraries (Twurple, `googleapis`, `youtubei.js`, TikTok-Live-Connector, `obs-websocket-js`). Rust core with TS plugins, or all TS?
10. **Data retention / chat history:** how long to keep chat logs and viewer data given Twitch's 24 h caching rule and YouTube's 30-day refresh/delete rule? Is a privacy policy + revocation handling planned (required by both)?
11. **Moderation cost on YouTube:** bans cost 200 units each. Should moderation actions on YouTube be rate-limited or confirmation-gated to protect quota?

---

## 14. Chat send capability matrix

Added 2026-10-19 (DEN-54). Owner goal: the combined chat sends to **every platform that has chat**, as Streamer.bot does for its platforms. Streamer.bot's own platform list is only Twitch, YouTube and Kick ([Streamer.bot platforms](https://docs.streamer.bot/guide/platforms)). Its other integrations (Discord, Ko-fi and so on) aren't live-chat platforms. "Phase" follows D-024 and PLAN.md. Sources were checked on 2026-10-19 unless a row says otherwise.

### 14.1 Matrix

| Platform | Official send API | Auth and scope | Broadcaster / bot identity | Reply | Max length | Rate limits | Delete / mod / pin | ToS risk | Phase |
|---|---|---|---|---|---|---|---|---|---|
| **Twitch** | **Yes.** `POST /helix/chat/messages` ([reference](https://dev.twitch.tv/docs/api/reference/#send-chat-message)) **[official]** | DCF user token, `user:write:chat`. The Chat Bot badge needs an app token + `user:bot` + `channel:bot`, which a public client can't mint (3.2) | Both. The bot is a second DCF login (user token, no badge) | `reply_parent_message_id` | 500 chars | 20 / 30 s regular (1 msg/s per channel), 100 / 30 s broadcaster/mod/VIP ([chat](https://dev.twitch.tv/docs/chat/)) | Delete `DELETE /helix/moderation/chat` (under 6 h old), ban/timeout `POST /helix/moderation/bans`, announcements (500 chars), pin via `pin: true` on send (20 min, not with a reply) or `/helix/chat/pins` | Low | P1 |
| **YouTube** | **Yes.** `liveChatMessages.insert` ([official](https://developers.google.com/youtube/v3/live/docs/liveChatMessages/insert)) | OAuth `youtube` or `youtube.force-ssl` (device flow: `youtube` only, 4.5). **20 quota units per message** | Any channel that signs in posts as itself (brand-account choice at login). No bot badge. Make the bot a moderator | **None** in the API | 200 chars ([YouTube Help](https://support.google.com/youtube/answer/15270973), not stated in the API docs) | 11 msgs / 30 s per Help Center. The API only returns `rateLimitExceeded`. Owner exemption **[unverified]** | `liveChatMessages.delete`, `liveChatBans.insert` (200 units), moderators. No pin API found | Low. Quota is the constraint | P2 |
| **Kick** | **Yes.** `POST /public/v1/chat` ([official](https://docs.kick.com/apis/chat.md)) | OAuth 2.1 + PKCE, `chat:write`. Exchange and refresh need the client secret, so the relay broker or BYO app (5.3) | `type: "user"` (needs `broadcaster_user_id`) or `type: "bot"` ("always sent to the channel attached to your token"). How `bot` messages are labelled is **[unverified]** | `reply_to_message_id` (UUID) | 500 graphemes and 2048 UTF-8 bytes | Not published. Handle 429 | Delete `DELETE /public/v1/chat/{id}`, ban/timeout `POST /public/v1/moderation/bans` (duration in **minutes**, 1–10080) ([moderation](https://docs.kick.com/apis/moderation)). No pin API | Low | P2 |
| **X Live** | **Whitelisted only.** `POST /2/broadcasts/{id}/chat` ([official](https://docs.x.com/x-api/broadcasts/send-a-chat-message-to-a-live-broadcast)). Access by form ([getting started](https://docs.x.com/livestream-api/getting-started)) | OAuth 2.0 PKCE user context, `broadcast.write` + `broadcast.read`. BYO X app (Q-C4) | Posts as the signed-in account | `reply_to` | 140 chars | Not published | Delete `DELETE /2/broadcasts/{id}/chat/{message_id}`, mutes, chat moderators (in X's [OpenAPI](https://docs.x.com/openapi.json)). Reading chat needs a hosted webhook (7.3) | Medium: access may be refused | ✗ until an app is whitelisted |
| **Facebook Live** | **Yes, as a comment.** `POST /{live-video-id}/comments` ([comments edge](https://developers.facebook.com/docs/graph-api/reference/object/comments/)) | Page token. `pages_manage_engagement` (+ `pages_read_user_content`, `pages_show_list`) or the MODERATE task. **App Review** + Business Verification for other people's Pages ([Live Video API](https://developers.facebook.com/docs/live-video-api/)) | Posts as the Page. No bot concept | Replies to a comment through `/{comment-id}/comments` **[unverified]** on live video | Not published | Not published | Delete `DELETE /{comment-id}`, hide `is_hidden=true` ([comment](https://developers.facebook.com/docs/graph-api/reference/comment/)). Live-chat ban **[unverified]** | Medium: review bureaucracy | P5 |
| **Rumble** | **No.** Official API is read-only. **Unofficial:** private `web7` chat API with the `u_s` session cookie (8.1) **[community]** | Session cookie from a Rumble web login (StreamDen login window) | Posts as the logged-in account. `channel_id` picks which of its channels speaks. Bot = a second account made a moderator | None found | 200 (client constant). Read `message_length_max` from the SSE `init` event | Not published. The client uses a 3 s cooldown **[unverified]** | Delete, pin/unpin, mute/unmute (unmute scrapes HTML), `/` commands | **High:** ToS bans automated access; breakage seen in Oct 2026 | P5 per D-024. Earlier and opt-in are open (Q-S8, Q-C8) |
| **TikTok LIVE** | **No.** Unofficial only (TikTok-Live-Connector `sendMessage`) ([repo](https://github.com/zerodytrash/TikTok-Live-Connector)) **[community]** | `sessionid` + `tt-target-idc` cookies **sent to the Euler Stream signer** (paid key) (6) | Posts as the logged-in user | No | Unknown | Unknown | None | **High:** ToS 3.4, AGPL libraries, credential sent to a third party | ✗ in core. Community plugin stance is Q-C9 |
| **Discord** (companion, not a stream platform) | **Yes.** Webhook `POST /webhooks/{id}/{token}` or bot `POST /channels/{id}/messages` ([webhook](https://docs.discord.com/developers/resources/webhook), [message](https://docs.discord.com/developers/resources/message)) | Webhook URL is the credential. BYO bot token with Send Messages (Q-S7) | Webhook can override `username`/`avatar_url` per message. Bot shows as the app | Bot `message_reference`. Webhook: `thread_id` only | 2000 chars | 50 req/s global per bot, per-route `X-RateLimit-*` headers ([rate limits](https://docs.discord.com/developers/topics/rate-limits)) | Bot: delete (Manage Messages), pin. Webhook: edit/delete its own messages. Set `allowed_mentions: {parse: []}` on bridged text | Low | Webhook P2, bot replies P4 |
| **Bluesky** | **No live chat.** "Live Now" only links to an external stream ([Engadget](https://www.engadget.com/social-media/blueskys-live-now-badge-is-available-to-everyone-223335221.html)) **[community]** | n/a (posting and DMs are covered in payments-x-bluesky.md) | n/a | n/a | n/a | n/a | n/a | n/a | n/a |
| **Streamplace** (atproto live video, not in scope) | **Yes, as records.** `com.atproto.repo.createRecord`, collection `place.stream.chat.message` ([lexicon](https://raw.githubusercontent.com/streamplace/streamplace/next/lexicons/place/stream/chat/message.json)) **[official, repo]** | atproto OAuth or app password (same login as Bluesky) | Any account posts as itself | `reply` {root, parent} | 300 graphemes / 3000 bytes | Not published (PDS limits) | Streamplace XRPC `moderation.createBlock` / `createGate` / `createPin` with a delegated permission | Low | Not in scope (Q-S9) |
| **Trovo** | **Dead.** Live streaming shut down 2026-06-30 ([Streamlabs](https://streamlabs.com/content-hub/post/prepare-for-the-end-of-trovo-streaming)) | — | — | — | — | — | — | — | Dropped |

**Takeaways**
- **Official send exists** for Twitch, YouTube, Kick and Facebook (as Page comments), plus Discord channels and, outside current scope, Streamplace. X Live send is official but gated behind whitelisting.
- **No official send:** Rumble and TikTok. Both need a session cookie and an unofficial API, which is a ToS risk. TikTok also sends that cookie to a third party, which is why we treat it more strictly than Rumble.
- **Hard limits differ a lot:** 140 (X) / 200 (YouTube, Rumble) / 300 graphemes (Streamplace) / 500 (Twitch, Kick graphemes) / 2000 (Discord). Rate limits are published only for Twitch and (Help Center) YouTube. YouTube also costs **20 quota units per message**, so "send to all" spends YouTube quota on every message.
- **"Accepted" ≠ "delivered":** Twitch returns `is_sent` + `drop_reason` (AutoMod, rate limit) and Kick returns `is_sent`. Treat HTTP 200 as accepted and show a dropped reason when the platform supplies one.

### 14.2 Combined-chat UX implications (input for Spec 08 / WS-08)

1. **Send target = platform × channel × identity.** One connected account can have several targets (Twitch broadcaster + bot, a Rumble account's channels, a YouTube brand account). The composer shows **target chips** with a per-target identity picker (broadcaster/bot where the platform has both).
2. **Capability-driven, never hard-coded.** Each connector advertises a send capability: `send` (with `max_len`, a length unit of chars/graphemes/bytes, `reply` support, identities), plus `delete`/`ban`/`timeout`/`pin`. It also reports a **state** (`ready`, `signed-out`, `session-expired`, `rate-limited{retry_after}`, `quota-low`, `read-only{reason}`, `not-live`, `unofficial-disabled`). The composer renders whatever the connector reports, so plugins (for example a community TikTok plugin) get the same UI. That shape belongs in den-protocol through the normal contract process (ADR + contract card). This research doesn't define it.
3. **Disabled targets explain themselves.** A read-only platform stays visible as a greyed chip with its reason: "Rumble: no official send API, turn on unofficial sending in Settings", "X Live: needs a whitelisted X app", "TikTok: read-only (no official API)", "YouTube: daily quota nearly used".
4. **Send to all = fan-out with per-target results.** One composer message becomes N independent sends. Each target gets its own result chip (sent / dropped with reason / failed with reason / skipped too long). Partial failure is normal and is shown, never hidden. Recommended: **no automatic retries**, because a resend can double-post, and a manual "retry failed" (Q-U6).
5. **Pre-flight per target.** Count length in each target's unit and warn before sending: "too long for YouTube (200) and X (140)". Choose per message: skip those targets, or let the user shorten the text. Never silently truncate. Apply a shared outbound limiter per platform (Twitch buckets, YouTube quota, Rumble cooldown) and show "rate-limited, retry in N s" rather than queueing silently.
6. **Replies.** Replying to a message targets only that message's platform and uses native reply where it exists (Twitch, Kick, X, Discord, Streamplace). Elsewhere it falls back to an `@name` prefix (YouTube, Rumble, Facebook). "Send to all" never threads.
7. **Unofficial targets (recommended, pending Q-C8 and Q-U6) are opt-in twice.** First in Settings (risk notice, off by default), then not auto-included in "send to all" unless the user ticks them in the target picker.
8. **Loop and echo safety.** Messages we send come back on the read path (EventSub, SSE, webhooks). Tag outbound sends with the platform message ID we get back (Twitch `message_id`, Kick `message_id`, Rumble `data.id`) and dedupe the echo so overlays and TTS don't double-handle our own messages. Chat-relay rules (8.36) must not re-send them.
9. **Moderation actions follow the same capability model.** The dock's quick actions show only what the message's platform supports. Units are normalised (Kick timeouts in minutes, Twitch in seconds), and destructive or expensive actions (YouTube bans at 200 units) ask for confirmation.

