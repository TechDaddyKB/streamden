# Rumble: Live Stream API URL (no registration)

Last verified against official docs: 2026-10-18. Part of the [owner runbooks](../README.md). Phase P5 (later), subject to Q-S1.

## Purpose

Rumble has **no developer app registration and no OAuth**. It offers a per-streamer **Live Stream API URL**: a secret URL that returns JSON (followers, subscribers, gifted subs, "rants", recent chat, title, likes, categories and "watching now"). StreamDen polls that URL. Each streamer pastes their own URL into the settings. There is nothing for the project owner to register.

## Prerequisites

- [ ] A Rumble account that can go live (see [go-live-requirements.md](../go-live-requirements.md)).
- [ ] Nothing about the product name or domain is needed.

## Cost and limits

- **Free.**
- Live data is populated **only while a stream is live**. Chat and rants are limited to the **50 most recent** items, so polling must be frequent enough not to miss messages. A community wrapper uses a 5-second minimum refresh (secondary source; Rumble gives no official polling guidance).
- "v1.1" of the API is mentioned by Rumble in the future tense. Treat the JSON shape as subject to change.

## Step-by-step

1. Log in at <https://rumble.com>.
2. Open <https://rumble.com/account/livestream-api>. Rumble says you "create and manage your stream API URLs and keys here".
3. Create or reveal your **Live Stream API URL** and copy it. Page labels: verify on screen.
4. Paste it into **StreamDen > Settings > Connections > Rumble** (once built), or into `.env.local` as `SL_RUMBLE_API_URL` for local development.
5. To rotate it: on the same page **reset the URL**. Rumble says you can "reset the URL and revoke access at any time".

## What to copy and where it goes

| Value | Secret? | Where it goes |
|---|---|---|
| Live Stream API URL | **Yes.** Rumble states the URL includes your stream key and needs no authentication, so anyone holding it can read your data. Treat it like a password. | OS keychain via the settings screen, or `.env.local` (gitignored). Never share it with anyone you do not trust, and never paste it into chat or an AI agent. |

## Verification and test

Go live (privately if Rumble offers it, verify on screen) or start a short test stream, then:

```bash
read -rs RURL; curl -s "$RURL" | head -c 2000
```

You should get JSON. While you are offline, `livestreams` is empty. Do not paste the output if it contains the key.

## Common pitfalls

- **Empty `livestreams`**: you are not live yet, so chat and rants are missing.
- **Only 50 chat items**: a busy chat can outrun slow polling.
- **Unofficial chat** (SSE stream and cookie-based sending) exists in community libraries but is unsupported and needs a session cookie. Not part of core.
- **Reset the URL** if you ever show it on stream or in a screenshot.

## Policy notes

- No developer terms to accept for this URL, but Rumble's general terms apply. The URL is for you or "trusted third parties".

## Sources

- Rumble Live Stream API help: <https://rumble.support/en/help/how-to-use-rumble-s-live-stream-api>
- Manage URLs: <https://rumble.com/account/livestream-api>
- Platform research: [platform-apis.md section 8](../../research/platform-apis.md)
