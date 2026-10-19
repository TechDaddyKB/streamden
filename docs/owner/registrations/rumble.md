# Rumble: Live Stream API URL (no registration)

Last verified against official docs: 2026-10-19 (chat-send section added for DEN-54). Part of the [owner runbooks](../README.md). Phase P5 (later), subject to Q-S1; an earlier phase and unofficial chat sending are open owner questions (Q-S8, Q-C8).

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
- **Unofficial chat** (SSE stream and cookie-based sending) exists in community libraries but is unsupported. Sending needs a session cookie. Not part of core unless you accept it (Q-C8). See the optional section below.
- **Reset the URL** if you ever show it on stream or in a screenshot.

## Optional: sending chat to Rumble (unofficial, only if Q-C8 is accepted)

Rumble has **no official way to send chat**. The Live Stream API URL above is read-only. StreamDen can only send, delete, pin or mute through Rumble's private website API while logged in as you. That is against the letter of Rumble's terms ("Restrictions on Automated Software") and may break without notice. The risk falls on **your** Rumble account. Details: [platform-apis.md section 8.1](../../research/platform-apis.md).

There is **nothing to register**. If the feature is built and you choose to turn it on:

1. Decide which account speaks. Using your main account is simplest. A second Rumble account made a moderator of your channel limits the damage if Rumble ever acts against automated use. It posts under its own name, and as a moderator it should be able to delete, pin and mute (unverified). Check on screen how to add a channel moderator.
2. In **StreamDen > Settings > Connections > Rumble > Chat sending (unofficial)**, read the warning and switch it on.
3. Click **Sign in to Rumble**. A StreamDen window opens rumble.com. Log in normally, including your 2FA code. StreamDen keeps only the session cookie, stores it in your OS keychain, and closes the window. **StreamDen never sees or stores your password.**
4. Fallback only if that window fails: StreamDen's advanced option lets you paste the `u_s` cookie value from your browser's developer tools. Treat it like a password. **Never paste it into chat, a board card, an AI agent, or the repo.**
5. Test on a private or unlisted stream (see [go-live-requirements.md](../go-live-requirements.md)): send a short message from the combined chat and check that it appears on rumble.com.

**What to copy and where it goes (send only)**

| Value | Secret? | Where it goes |
|---|---|---|
| Rumble session cookie (`u_s`) | **Yes.** Anyone holding it is logged in as you until it expires or you log out. | Captured by StreamDen's sign-in window into the OS keychain. Never into `.env.local`, screenshots, logs or chat. |

**To revoke:** click **Sign out** in StreamDen, which logs the session out at Rumble and deletes the keychain entry. Changing your Rumble password also ends the session.

**If it stops working:** StreamDen marks Rumble "read-only" with a reason ("sign in again", "blocked by Rumble" or "Rumble changed its site"). Reading chat keeps working through the API URL. Don't work around a block. Wait for a StreamDen update.

## Policy notes

- No developer terms to accept for this URL, but Rumble's general terms apply. The URL is for you or "trusted third parties".
- Rumble's [terms](https://rumble.com/s/terms) (last modified 2026-09-01) prohibit "any form of automated software to interact with or access the Rumble Site" without prior written permission. That is why chat sending is unofficial and opt-in.

## Sources

- Rumble Live Stream API help: <https://rumble.support/en/help/how-to-use-rumble-s-live-stream-api>
- Manage URLs: <https://rumble.com/account/livestream-api>
- Rumble terms of service: <https://rumble.com/s/terms>
- Platform research: [platform-apis.md sections 8 and 8.1](../../research/platform-apis.md)
