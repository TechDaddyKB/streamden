# Twitch: developer application (and optional bot account)

Last verified against official docs: 2026-10-18. Part of the [owner runbooks](../README.md).

## Purpose

Twitch is the P1 platform. StreamDen needs one **Twitch developer application** so that it can:

- read chat and events through **EventSub over WebSocket** (`wss://eventsub.wss.twitch.tv/ws`),
- send chat and moderate through **Helix**,
- create and manage **channel-point rewards** (see the ownership caveat below).

The decision already made in [platform-apis.md section 3.5](../../research/platform-apis.md): register one **Public** client and use the **Device Code Flow (DCF)**. A public client has no client secret, so nothing secret ships inside the app and we need no relay for Twitch.

## Prerequisites

- [ ] The product name is final (Q-B1). The name is shown to every user on their Twitch **Connections** page, and the Client ID you get is permanent (see "Channel-point reward ownership").
- [ ] A Twitch account to own the application (your main account, or a dedicated project account). It needs a verified email and **two-factor authentication (2FA) enabled**. Twitch will not let you register an app without 2FA.
- [ ] A password manager and an authenticator app ready (see [README general rules](../README.md#general-rules)).
- [ ] Decide `<SL_PORT>`: the local server port is chosen in spec 06 (`docs/spec/06-api-mcp.md`). Until then, use the placeholder and come back to edit the redirect URL.

## Cost and limits

- **Free.**
- Helix: 800 points per minute per client ID per user (most endpoints cost 1 point).
- EventSub WebSocket: 3 connections per (client ID, user), 300 subscriptions per connection, max total cost 10.
- Chat sending: 20 messages per 30 s for a normal account, 100 per 30 s for the broadcaster, a moderator or a VIP, and 1 message per second per channel for non-privileged accounts.
- Access tokens last 4 hours. Refresh tokens are **single-use** and expire after **30 days unused**.

## Step-by-step

### A. Prepare the owning account

1. Sign in at <https://www.twitch.tv> with the account that will own the application.
2. Open your email and click the verification link Twitch sent (check spam).
3. Turn on 2FA: go to <https://www.twitch.tv/settings/security> and use the **Set Up Two-Factor Authentication** button (exact label: verify on screen). Twitch asks for a phone number first, then an authenticator app. Store the backup codes in your password manager.
4. Go to <https://dev.twitch.tv/console> and log in with Twitch. If you were already on the page, refresh it after enabling 2FA. Accept the Developer Services Agreement if the console asks (verify on screen).

### B. Register the application

5. In the console, open the **Applications** tab and click **Register Your Application**.
6. **Name**: enter the final product name (`<PRODUCT_NAME>`). It must be unique among all Twitch applications. Users see it on their Connections page.
7. **OAuth Redirect URLs**: enter `http://localhost:<SL_PORT>/auth/twitch/callback` and click **Add**.
   - The Device Code Flow does not use a redirect URL, but the form asks for at least one (verify on screen whether it is mandatory for Public clients).
   - Keeping a loopback URL here also leaves room for an authorization-code fallback later.
8. **Category**: pick the closest to a streaming-automation tool. The list contains entries such as "Chat Bot", "Broadcaster Suite" and "Application Integration" (exact options: verify on screen). The category is informational.
9. **Client Type**: choose **Public**. Public clients are intended for the Device Code Flow and have no client secret. Confidential clients have a secret and are meant for Authorization Code or Implicit flows. Twitch's registration page does not document this field, but the console has it (Twitch announced it with DCF). Community reports say the client type cannot be changed after creation, so if you pick wrong, create a new app and discard the old one.
10. Tick the **I'm not a robot** CAPTCHA and click **Create**.
11. Back on the Applications tab, find your app under **Developer Applications** and click **Manage**.
12. Copy the **Client ID**. A Client ID is public by design (it may appear in a web page's source).
13. **Do not click "New Secret"**. Public clients cannot have a secret, and generating one on a confidential app invalidates the old one.

### C. Scopes (nothing to configure)

Twitch has no scope registration in the console. The app asks for scopes at runtime when the user links their account. For your information, the set StreamDen will request (all names exist in Twitch's scope list as of today):

| Area | Scopes |
|---|---|
| Chat read/send | `user:read:chat`, `user:write:chat` |
| Chat bot | `user:bot`, `channel:bot` |
| Channel points | `channel:read:redemptions`, `channel:manage:redemptions` |
| Followers, subs, bits, hype train | `moderator:read:followers`, `channel:read:subscriptions`, `bits:read`, `channel:read:hype_train` |
| Moderation | `moderator:manage:banned_users`, `moderator:manage:chat_messages`, `moderator:manage:announcements` |
| Broadcast | `channel:manage:broadcast`, `clips:edit`, `channel:manage:polls`, `channel:manage:predictions` |

More scopes will be added by the engine spec. The consent screen lists exactly what is requested.

### D. Optional: a separate bot account

Use this if you want chat messages to come from a different identity (for example `<name>_bot`) instead of your streaming account.

14. Create a second Twitch account with the project email (or a `+bot` alias). Verify the email and enable 2FA on it too.
15. In StreamDen (once the Twitch connector exists), link the bot account separately through the same device-code screen. The scopes it needs are `user:read:chat` and `user:write:chat`.
16. In your own channel chat, make the bot a moderator: `/mod <botname>`. Moderators get 100 messages per 30 s instead of 20.
17. **Chat Bot badge**: this badge needs an **App Access Token** (client-credentials grant), which needs a client secret. A Public client cannot have one, so StreamDen's local-only design **cannot show the badge** (decision recorded in the platform research). It is cosmetic. If Q-C1 ever approves a relay, a second **Confidential** application (for example "`<PRODUCT_NAME>` Relay") could mint app tokens. Twitch says every application must have its own Client ID, so never reuse the Public app's ID for that.

### E. Optional: a dev application

Create a second Public app named `<PRODUCT_NAME> Dev` only if you want to experiment without touching the production Client ID. Remember that channel-point rewards created by one Client ID cannot be edited by another (next section). For a solo owner, one app is simpler.

## Channel-point reward ownership caveat (read this before registering)

- Custom rewards created through the API can only be **updated, deleted and have their redemptions fulfilled or refunded by the same Client ID that created them**. Rewards created by hand in the Twitch dashboard can be read by the API but not managed by it. This is documented by Twitch staff and developers on the Twitch forums. The official Helix reference does not state it in the sections I could fetch, so treat it as community-confirmed (see the Sources below).
- EventSub still delivers redemption events for any reward, whoever created it.
- Consequence: the **production Client ID is forever**. Changing it later forces every streamer to recreate their rewards. Firebot hit this in v5.60.
- Channel points exist only on Affiliate or Partner channels (commonly stated, verify on your channel). To test rewards you need an Affiliate channel. Everything else can be tested with the Twitch CLI mock server (see [go-live-requirements.md](../go-live-requirements.md)).

## What to copy and where it goes

| Value | Secret? | Where it goes |
|---|---|---|
| **Client ID** | No (public) | A config constant in the app source (the one shipped Client ID), and `.env.local` as `SL_TWITCH_CLIENT_ID` for local runs. Final name and location are decided in P0. |
| Client secret | n/a for Public | Nothing to copy. Do not create one. |
| Users' access and refresh tokens | Yes | Created at runtime by the app. Stored in the OS keychain (see Q-A5), never in files you commit. |

## Verification and test

Run this from a terminal once the app exists. It uses the device flow end to end. **Do not paste the output (tokens) into chat, an issue or an AI agent.**

```bash
CID='<your client id>'
# 1. Ask Twitch for a device code
curl -s -X POST 'https://id.twitch.tv/oauth2/device' \
  -d "client_id=$CID" --data-urlencode 'scopes=user:read:chat'
# The JSON has verification_uri, user_code and device_code.
# 2. Open the verification_uri in a browser, log in and click Authorize.
# 3. Exchange the device code (returns authorization_pending until you approve)
curl -s -X POST 'https://id.twitch.tv/oauth2/token' \
  -d "client_id=$CID" --data-urlencode 'scopes=user:read:chat' \
  -d "device_code=<device_code from step 1>" \
  -d 'grant_type=urn:ietf:params:oauth:grant-type:device_code'
# 4. Validate the access token you received
read -rs TOKEN; curl -s -H "Authorization: OAuth $TOKEN" https://id.twitch.tv/oauth2/validate
```

A successful validate response shows your `client_id`, `login`, `scopes` and `expires_in`. Afterwards revoke the test grant: Twitch **Settings > Connections**, find the app, **Disconnect** (labels: verify on screen).

## Common pitfalls

- **Wrong client type.** Public is for DCF. If DCF returns an error about the client type, you registered Confidential. Make a new app.
- **Skipped 2FA.** The console shows an error and no register button. Enable 2FA and refresh.
- **Name already taken.** Names are globally unique. Pick the final name early.
- **Regenerating a secret** (confidential apps) invalidates the old one at once.
- **Refresh tokens are single-use.** Persist the new refresh token atomically after every refresh, or the user is logged out. Tokens unused for 30 days expire and the user must run the device flow again.
- **Validate at startup and hourly.** Twitch audits for this call (`/oauth2/validate`).
- **Redirect URLs must match exactly** if you ever use a flow that needs them (scheme, host, port, path).
- **Do not share one Client ID across different applications.** Twitch may suspend API access.

## Policy notes

- Twitch Developer Services Agreement: keep a public privacy policy, do not cache Twitch content longer than about 24 hours without authorization, delete user data on revocation or request, do not circumvent rate limits.
- Never expose a client secret "even in obscured form" (relevant only if a confidential app is ever created).
- The Chat Bot badge and "verified bot" status are separate. Verified-bot review is currently paused and verification does not grant the badge.

## Sources

- Register an app: <https://dev.twitch.tv/docs/authentication/register-app/>
- Getting tokens (DCF, public vs confidential): <https://dev.twitch.tv/docs/authentication/getting-tokens-oauth/>
- Refresh tokens: <https://dev.twitch.tv/docs/authentication/refresh-tokens/>
- Validate tokens: <https://dev.twitch.tv/docs/authentication/validate-tokens/>
- Scopes: <https://dev.twitch.tv/docs/authentication/scopes/>
- Chat (badge, bots, limits): <https://dev.twitch.tv/docs/chat/> and <https://dev.twitch.tv/docs/chat/authenticating/>
- EventSub over WebSocket: <https://dev.twitch.tv/docs/eventsub/handling-websocket-events/>
- Client type discussion (community): <https://discuss.dev.twitch.com/t/what-type-of-client-to-choose/63638>
- Reward ownership (community threads): <https://discuss.dev.twitch.com/t/refunding-channel-points-error-403/29929>, <https://discuss.dev.twitch.com/t/bug-update-custom-reward-redemption-gives-403/61230>
- Firebot's login change FAQ: <https://github.com/crowbartools/Firebot/wiki/Firebot-v5.60.0-Twitch-Login-Changes-FAQ>
- Developer Services Agreement: <https://legal.twitch.com/legal/developer-agreement/>
- Twitch CLI (mock EventSub): <https://dev.twitch.tv/docs/cli/> and <https://dev.twitch.tv/docs/cli/websocket-event-command/>
