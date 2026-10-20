# Kick: developer app

Last verified against official docs: 2026-10-18; connection model re-verified 2026-10-19 (DEN-244, D-059). Part of the [owner runbooks](../README.md).

## Purpose

Kick's official API (launched 2025) gives StreamDen chat sending, moderation, channel and livestream info, channel-point rewards and **events**. Two Kick quirks shape this runbook:

1. **Events are webhooks only.** Kick POSTs events to **one webhook URL per app**, and that URL must be publicly reachable. A desktop app cannot receive that directly. This is why Kick depends on the relay decision **Q-C1** (a self-hostable Cloudflare Worker, see [relay-cloudflare.md](relay-cloudflare.md)), or on each user running a tunnel.
2. **OAuth needs a client secret even with PKCE.** There is no public/native client type yet (request is open: KickDevDocs issue #413). So either each user registers **their own Kick app** (bring-your-own, secret stays on their PC) or a relay acts as the token broker.

What you do now: register **your own dev app** for testing. What you do not do yet: wire a production webhook URL.

## How users connect (D-059: browser login first, where possible)

- **Wanted default:** a **Sign in with Kick** button that opens Kick's PKCE consent page in the browser against StreamDen's own app. Kick requires the `client_secret` for every token exchange and refresh and has **no public/native client type** ([OAuth flow](https://docs.kick.com/getting-started/generating-tokens-oauth2-flow.md), [#413](https://github.com/KickEngineering/KickDevDocs/issues/413) open, re-read 2026-10-19; the changelog has no auth change since 2026-01-15). So a no-setup button exists **only through the official hosted relay's token broker**, which D-019 schedules "later" (Q-C13). The desktop app never ships the secret.
- **Working path at P2 (fallback, Advanced):** **bring your own Kick app**: the user registers an app as in section B, pastes its Client ID and secret into StreamDen (keychain), points its webhook at a self-hosted relay or a tunnel, and then completes the same browser consent. Events are webhook-only either way.
- **Provider gates:** secret requirement, 1,000 `chat.message.sent` subscriptions per unverified app (email verification needed for a shared app), webhooks auto-unsubscribed after a day of failures.

## Prerequisites

- [ ] A Kick account with a verified email and **two-factor authentication enabled** (required before the Developer tools unlock).
- [ ] Final product name (Q-B1).
- [ ] Read Kick's Developer Terms and Conditions (linked from the docs setup page).
- [ ] For webhooks later: a public HTTPS URL (relay, or your own tunnel such as Tailscale Funnel, ngrok or a Cloudflare Tunnel). Not needed to register the app.
- [ ] `<SL_PORT>` from spec 06.

## Cost and limits

- **Free.**
- Webhook subscriptions: 10,000 per event type per app, **but `chat.message.sent` is capped at 1,000 for unverified apps** (10,000 when verified). A single shared relay app would serve about 1,000 users' chat until Kick verifies it.
- **App verification** is a manual request: email **developers@kick.com** with the Client ID, app name, whether the bot account also needs verification, the reason and supporting evidence. Verified apps get a badge on the bot account and the higher limit.
- A webhook that keeps failing for more than a day is **auto-unsubscribed** (since 2025-12-12). Disabling webhooks for the app unsubscribes everything (since 2025-12-19). The app must re-subscribe on start.

## Step-by-step

### A. Prepare the account

1. Sign in at <https://kick.com>.
2. Open **Account Settings** and enable **two-factor authentication** (Security section; authenticator app preferred, SMS is the fallback; exact labels: verify on screen).
3. Save the 2FA recovery codes in the password manager.

### B. Create the app

4. Go to <https://kick.com/settings/developer> (the **Developer** tab of Account Settings).
5. Create a new app (button label: verify on screen). Kick's docs say creating an app generates a **Client ID**, a **Client Secret** and the **redirect URL** you specify. The docs do not name the form fields, so verify these on screen:
   - **App name**: `<PRODUCT_NAME>` (or `<PRODUCT_NAME> Dev` for testing).
   - **Redirect URL**: `http://localhost:<SL_PORT>/auth/kick/callback`. Kick's own examples use `localhost`. The redirect must match exactly what the app sends. If you will use Kick's interactive docs to try endpoints, you may temporarily set it to `https://docs.kick.com` (Kick's documented trick for testing without hosting an app).
6. Copy the **Client ID** and **Client Secret**. Where the secret is displayed (once at creation, or on the app page) is not documented: copy it immediately and store it in the password manager.

### C. Webhooks (do later, only when Q-C1 is decided)

7. In **Account Settings > Developer**, open your existing app and click **Edit**. You will find a section labeled **Enable Webhooks** with a switch and a textbox below it (documented).
8. Switch it **On** and enter a **publicly accessible** URL in the textbox. Kick sends `POST` requests with event payloads there. Localhost URLs do not work. Whether HTTPS is mandatory is not stated in Kick's docs; use HTTPS anyway (verify on screen).
9. For the relay the URL will look like `https://<RELAY_HOST>/h/<mailboxId>/kick` (exact shape comes from the relay spec, not final).
10. Webhook security: each delivery carries `Kick-Event-Signature` (RSA PKCS#1 v1.5 + SHA-256 over `message_id.timestamp.raw_body`, using `Kick-Event-Message-Id` and `Kick-Event-Message-Timestamp`). The public key comes from `GET https://api.kick.com/public/v1/public-key` and must **not** be hard-coded because it rotates. The desktop verifies, not the relay.

### D. Scopes

Scopes are requested in the authorize URL (space-separated). The 13 documented scopes and our intended use:

| Scope | Use in StreamDen |
|---|---|
| `user:read` | basic user info (needed) |
| `channel:read` | channel details, category |
| `channel:write` | update stream title and category |
| `chat:write` | send chat messages as user or bot |
| `events:subscribe` | create webhook subscriptions (needed) |
| `moderation:ban` | ban and unban |
| `moderation:chat_message:manage` | delete chat messages |
| `channel:rewards:read` / `channel:rewards:write` | channel-point rewards |
| `kicks:read` | KICKs leaderboards |
| `ads:read` / `ads:write` | only if we add ad controls |
| `streamkey:read` | **do not request.** Reading a user's stream key is sensitive and we do not need it. |

## What to copy and where it goes

| Value | Secret? | Where it goes |
|---|---|---|
| Client ID | No | `SL_KICK_CLIENT_ID` in `.env.local` / Settings > Connections > Kick |
| **Client Secret** | **Yes** | `SL_KICK_CLIENT_SECRET` in `.env.local` (gitignored) for your dev app. For users: the OS keychain via the BYO wizard. If a relay becomes the broker, the secret lives only as a Worker secret (`wrangler secret put`), never in the repo. |
| Webhook URL | Semi (an unguessable path) | Kick app settings. The relay mailbox ID inside it is a secret. |
| User tokens | Yes | Runtime only, in the OS keychain. |

(Names are suggestions; P0 decides the real location.)

## Verification and test

Manual PKCE check from a terminal. **Do not paste codes or tokens into chat or an AI agent.**

```bash
VERIFIER=$(openssl rand -base64 48 | tr -d '=+/\n' | cut -c1-64)
CHALLENGE=$(printf %s "$VERIFIER" | openssl dgst -sha256 -binary | openssl base64 -A | tr '+/' '-_' | tr -d '=')
STATE=$(openssl rand -hex 8)
echo "https://id.kick.com/oauth/authorize?client_id=<CLIENT_ID>&response_type=code&redirect_uri=http%3A%2F%2Flocalhost%3A<SL_PORT>%2Fauth%2Fkick%2Fcallback&scope=user%3Aread%20channel%3Aread&code_challenge=$CHALLENGE&code_challenge_method=S256&state=$STATE"
```

1. Open the printed URL while logged in to Kick, approve. The browser then fails to load the localhost callback. Copy `code` from the address bar (check `state` matches).
2. Exchange the code (the secret is required here, which is exactly why this step belongs on a server or in a BYO app):

```bash
read -rs SECRET
curl -s -X POST https://id.kick.com/oauth/token \
  -d grant_type=authorization_code -d client_id=<CLIENT_ID> -d client_secret="$SECRET" \
  -d redirect_uri='http://localhost:<SL_PORT>/auth/kick/callback' \
  -d code=<CODE> -d code_verifier="$VERIFIER"
```

3. Call the API: `read -rs T; curl -s -H "Authorization: Bearer $T" https://api.kick.com/public/v1/users` returns your user (endpoint path: verify in the Kick docs UI).
4. For webhooks: after enabling them, create a subscription with `POST https://api.kick.com/public/v1/events/subscriptions` (scope `events:subscribe`, `method` is always `webhook`), then send a chat message in your channel and check the relay or tunnel log for the delivery.

## Common pitfalls

- **Redirect mismatch.** The `redirect_uri` in the authorize call and in the token exchange must exactly equal the URL saved in the app.
- **`state` and `code_challenge` are mandatory** (PKCE S256).
- **Refresh also needs the client secret** and returns a new access and refresh token.
- **Secret handling.** Anyone with the secret can impersonate your app. If it leaks, regenerate it in the Developer tab (verify on screen) and update every place it is stored.
- **Webhook failures silently unsubscribe you** after a day. Monitor and re-subscribe on startup.
- **`127.0.0.1` redirects**: Kick's docs mention a Next.js quirk when using `127.0.0.1`; prefer `localhost` here.
- **Unofficial chat WebSocket** (Pusher) is unsupported by Kick staff ("we don't support any bots or apps connecting to ws.kick.com"). It would only be an opt-in read-only fallback (Q-C3).
- Raids/hosts, polls and message-deleted events are not available through the official API at the time of writing.

## Policy notes

- Developer Terms and Conditions apply, and 2FA is required for developer tools.
- A shared, multi-user app needs verification (email developers@kick.com) to exceed 1,000 chat subscriptions.
- A relay that brokers tokens must never store them, and the hosted-relay option (Q-C1 option c) needs a privacy policy.

## Sources

- App setup: <https://docs.kick.com/getting-started/kick-apps-setup.md>
- OAuth 2.1 flow: <https://docs.kick.com/getting-started/generating-tokens-oauth2-flow.md>
- Scopes: <https://docs.kick.com/getting-started/scopes.md>
- Webhooks introduction (Enable Webhooks switch and URL box): <https://docs.kick.com/events/introduction.md>
- Webhook security: <https://docs.kick.com/events/webhook-security.md>
- Subscribe to events (limits): <https://docs.kick.com/events/subscribe-to-events.md>
- FAQ (verification, docs-UI redirect trick): <https://docs.kick.com/apis/faqs.md>
- Changelog: <https://docs.kick.com/changelog.md>
- Public/native client request: <https://github.com/KickEngineering/KickDevDocs/issues/413>
- Unofficial WebSocket statement: <https://github.com/KickEngineering/KickDevDocs/issues/285>
