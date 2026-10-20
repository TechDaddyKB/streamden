# Donation, tip and commerce services

Last verified against official docs: 2026-10-18; connection model re-verified 2026-10-19 (DEN-244, D-059). Part of the [owner runbooks](../README.md). Phase P5, except the relay-dependent ones which also wait on **Q-C1**.

## Purpose

These services deliver tips, subscriptions, merch orders and gifts to StreamDen as events. The platform research split them into two groups:

- **Outbound socket or token, fully local:** Streamlabs, StreamElements. You copy a token from the service's dashboard. No app registration, no relay.
- **Webhook only, needs a public HTTPS URL:** Ko-fi, Throne, Fourthwall (and Patreon webhooks). These need the relay (Q-C1) or a user-run tunnel. Patreon can also be polled locally with a creator token.

You only need accounts on services **you actually use**. For the project owner this means: pick the ones you want to test with, or none and rely on test fixtures.

## How users connect (D-059 applied)

- **Streamlabs and StreamElements.** **Preferred target:** a "Connect with …" button that opens the provider's consent page in the browser against a StreamDen OAuth app. Both providers need the client secret for the code exchange and the refresh, so the exchange runs in the hosted relay's token broker (D-019: later), never in the desktop. Streamlabs app registration is official; until Streamlabs approves the app only 10 whitelisted users may authorize it ([register](https://dev.streamlabs.com/docs/register-your-application), re-read 2026-10-19). StreamElements documents the whole OAuth2 flow (authorize, code exchange with `client_secret`, refresh with `client_secret`, validate, revoke; scopes `channel:read`, `tips:read`, `activities:read` and more) but not how a developer obtains `client_id` and `client_secret` ([OAuth2](https://github.com/StreamElements/api-docs/blob/main/docs/OAuth2.md), read 2026-10-19), so credential issuance is an owner request to SE. **Interim default and advanced fallback:** the pasted **socket token / JWT** below, which stays supported.
- **Ko-fi, Throne, Fourthwall:** webhook-only; the user pastes the relay URL into the vendor and a token/secret into StreamDen. No login flow exists, so the paste is the only path.
- **Patreon.** **Preferred target:** "Sign in with Patreon" browser consent on the StreamDen client, with the exchange and refresh in the hosted broker because both need the client secret ([docs](https://docs.patreon.com/), re-read 2026-10-19: the exchange is "performing the following request on the server"). **Interim default and advanced fallback:** the pasted **Creator's Access Token** (local polling). Webhooks need the relay either way (Q-C1).
- **Prerequisites before any of these browser buttons can ship** (none of them moves D-019): provider app approval or credential issuance (owner), scope selection per provider, the broker's exchange, refresh and revoke endpoints with a secure token handoff to the desktop (relay design, Q-C1), and a privacy policy. Timing is Q-C12; the follow-up cards are proposed in the DEN-244 artifact.

## Prerequisites

- [ ] An account on each service you want to test (a creator/streamer account, not a supporter account).
- [ ] For webhook services: the relay or a tunnel exists (see [relay-cloudflare.md](relay-cloudflare.md)). Until then you can create the account and copy tokens, but the webhook URL field must wait.
- [ ] Never paste any token below into the repo, a chat or an AI agent.

## Cost and limits

- Streamlabs, StreamElements, Ko-fi, Patreon (creator side), Throne, Fourthwall: free to set up as a creator (Patreon, Ko-fi and the like take platform/payment fees on money you receive, which is irrelevant to the integration).
- Patreon API: 100 requests per minute per token (research figure).
- Fourthwall webhooks: 5 delivery attempts, 5-second timeout. Throne: respond 2xx within 10 seconds.

---

## 1. Streamlabs

**Interim route (the D-059 fallback): the Socket API token.** No application registration. The browser "Connect with Streamlabs" target needs the OAuth app below plus the hosted broker.

1. Log in at <https://streamlabs.com>.
2. Go to **Dashboard > Settings > API Settings > API Tokens**.
3. Next to **Your Socket API Token**, click **Copy**.
4. Paste it into StreamDen (**Settings > Connections > Streamlabs**) or `.env.local` as `SL_STREAMLABS_SOCKET_TOKEN`.

Test (scratch folder outside the repo; Socket.IO v2 is required):

```bash
mkdir -p ~/scratch/slabs && cd ~/scratch/slabs && npm i socket.io-client@2
read -rs ST; ST="$ST" node -e "
const io=require('socket.io-client');
const s=io('https://sockets.streamlabs.com?token='+process.env.ST,{transports:['websocket']});
s.on('connect',()=>console.log('connected'));
s.on('event',e=>console.log(JSON.stringify(e)));"
```

Then send a test alert from the Streamlabs dashboard (Alert Box settings have test buttons; verify on screen). The `message` field is always an array and `for` says the platform (`twitch_account`, `youtube_account`, ...).

**OAuth app: the D-059 target (owner action once the broker is scheduled).** The registration page is <https://streamlabs.com/dashboard#/oauth-clients/register> (log in first). Until Streamlabs sets an app to **approved**, only up to **10 whitelisted users** may authorize it, and the exchange needs a client secret that must live in the broker, never in the desktop. Streamlabs documents only "provide the necessary information needed to register a new oauth application" and "click create"; it names no fields and no approval contact ([register](https://dev.streamlabs.com/docs/register-your-application), re-read 2026-10-19). **Do not register yet.** The redirect URI must be the hosted broker's callback, which does not exist until the broker design fixes it; an agent prep card (DEN-244 artifact, N3a) first records the form's fields and the approval route, and the owner task then carries exact values: name `StreamDen Dev`, website = the project domain, redirect = the broker callback, whitelist = the owner's test accounts, Client ID recorded in this runbook (public), Client Secret to the password manager until `wrangler secret put`. Until approval and the broker exist, the pasted socket token is the working route.

Pitfalls: treat the token as a password (anyone with it can read your alerts); community reports say a Streamlabs widget may need to be open for events to flow (unverified); regenerating the token on the dashboard breaks the saved one.

## 2. StreamElements

1. Log in at <https://streamelements.com>.
2. Click your **avatar (top right)**, select your **channel name**, choose the correct channel, then copy the token. Secondary guides name the buttons **Channels > Show secrets** and show the **JWT Token** (verify on screen).
3. If you have several linked accounts (Twitch, YouTube, Kick), **switch to the channel whose activities you want before copying**, or you will subscribe fine but receive nothing.
4. Two token types work with the WebSocket: the **JWT** (`token_type: jwt`) and the channel **Overlay token** (`token_type: apikey`, found in the same place). OAuth2 access tokens (`oauth2`) also work and are the D-059 target once StreamElements issues StreamDen a client ID and secret and the broker exists.
   **Credential request (not yet dispatchable).** SE's OAuth2 page only says "Once you receive the `client_id` and `client_secret`"; it names no issuance route, and the api-docs repository has no README or contact ([OAuth2](https://github.com/StreamElements/api-docs/blob/main/docs/OAuth2.md), [repo](https://github.com/StreamElements/api-docs), re-read 2026-10-19). An agent prep card (DEN-244 artifact, N4a) must first find and source the contact route (the repository's Issues tab is observable but not documented as support). The request then states: StreamDen (GPL-3.0-or-later desktop app, link), purpose (read tips and activities for alerts), scopes `channel:read`, `tips:read`, `activities:read`, redirect URI = the hosted broker's callback (from the broker design), and asks how the secret is rotated. **Non-secret facts to record here:** `client_id`, registered redirect URIs, granted scopes, SE's answer and date. **Secret handling:** the `client_secret` goes to the password manager, later to the broker with `wrangler secret put`; never into the desktop, this repo or a chat. Approval is not guaranteed; a "no" is a valid recorded answer and the JWT stays the route.
5. Store as `SL_STREAMELEMENTS_JWT` (keychain / `.env.local`). Community reports say the JWT can rotate roughly every two weeks (unverified), so the app must handle a "token rejected, please re-paste" state.

Test:

```bash
read -rs JWT; curl -s -H "Authorization: Bearer $JWT" https://api.streamelements.com/kappa/v2/channels/me
```

(endpoint path from SE's public API: verify). The JSON includes your channel `_id`, which is the `room` value for the WebSocket. The WebSocket host is `wss://astro.streamelements.com/`; subscribe with `{ "type":"subscribe", "nonce":"x", "data": {"topic":"channel.activities","room":"<_id>","token":"<JWT>","token_type":"jwt"} }`, and send PING every 30 seconds.

## 3. Ko-fi (needs relay or tunnel)

Ko-fi has no public REST API, only **webhooks** with a verification token.

1. Log in at <https://ko-fi.com> as the creator.
2. Open the webhooks page: <https://ko-fi.com/manage/webhooks>. Secondary guides describe the path as **More > API > Webhooks**, with the token under **Advanced > Verification Token** (verify on screen; Ko-fi's own help page could not be fetched).
3. Copy the **Verification Token**. It identifies Ko-fi as the sender. Store it as `SL_KOFI_VERIFICATION_TOKEN`. **The token stays on the desktop. The relay does not hold it** (the relay is a dumb pipe and the desktop verifies).
4. In **Webhook URL**, paste the relay mailbox URL, for example `https://<RELAY_HOST>/h/<mailboxId>/kofi`. This URL does not exist until the relay (P2) is deployed.
5. Use Ko-fi's **Send test** button (verify on screen) to fire a sample donation.

Payload facts: `application/x-www-form-urlencoded` with a field `data` containing JSON that includes `verification_token`, types Donation, Subscription, Commission, Shop Order. A manual test (once a receiver exists):

```bash
curl -X POST '<RECEIVER_URL>' --data-urlencode 'data={"verification_token":"<TOKEN>","type":"Donation","amount":"3.00","from_name":"Test"}'
```

Pitfalls: if your PC is off, events are lost unless the relay buffers them; Ko-fi may disable a failing webhook.

## 4. Patreon (creator token for polling, or webhooks via relay)

1. Log in to Patreon as the creator and open the client registration page: <https://www.patreon.com/portal/registration/register-clients>.
2. Create a client. The attributes are: name, description, **redirect URIs**, domain, privacy policy URL, terms of service URL, and the **API version**. Choose **v2** (v2 clients are scoped to the v2 scopes). Redirect URI for the D-059 target: the hosted broker's callback once the broker design fixes it (not invented here); a `localhost` redirect only fits a developer's own client, because the exchange needs the secret. Which fields are mandatory: verify on screen.
3. After creation the page shows **Client ID**, **Client Secret**, **Creator's Access Token** and **Creator's Refresh Token**. You can refresh the token manually on that page.
4. For local polling, only the **Creator's Access Token** is needed. Store as `SL_PATREON_CREATOR_TOKEN`. The secret and refresh token are only for a full OAuth app.
5. For instant alerts, create a webhook at <https://www.patreon.com/portal/registration/register-webhooks>: URL = the relay URL, triggers = `members:create`, `members:update`, `members:delete` and the pledge ones. Patreon signs requests with `X-Patreon-Signature`, the **HMAC-MD5 hex digest** of the body with your webhook secret. The portal does not document where the secret is displayed; the API returns it on webhook creation (scope `w:campaigns.webhook`).

Test:

```bash
read -rs PT; curl -s -H "Authorization: Bearer $PT" 'https://www.patreon.com/api/oauth2/v2/campaigns'
```

Pitfalls: the creator token is full-access to your campaign data; HMAC-MD5 is weak, so rely on the unguessable relay URL too.

## 5. Throne (webhook, relay required)

1. In your Throne creator settings, enable the **Webhook integration** and set the **subscriber URL** (HTTPS required in production). The help center article does not give the settings path (verify on screen).
2. Throne needs no shared secret: it signs with **Ed25519** and publishes its public key in the help article (headers `X-Signature-Ed25519` and `X-Signature-Timestamp`; the signed message is the timestamp, a dot, then the raw body).
3. Events: `gift_purchased`, `contribution_purchased`, `gift_crowdfunded`. Envelope fields: `contract_version`, `event_id` (use it for deduplication), `event_type`, `data`. Respond with 2xx within 10 seconds.
4. Nothing secret to store except the unguessable relay URL.

## 6. Fourthwall (webhook, relay required)

1. Log in to your shop admin. The documented developer settings URL is `https://<your-shop>.fourthwall.com/admin/dashboard/settings/for-developers` (menu labels: verify on screen).
2. Create a webhook subscription: pick the topics (for example `ORDER_PLACED`, donation, gift purchase, subscription events) and enter an **HTTPS** endpoint (the relay URL).
3. Fourthwall signs each request with `X-Fourthwall-Hmac-SHA256` (base64 HMAC-SHA256). How to obtain the signing secret is described on its "Verify Signatures" page (verify on screen). Store it as `SL_FOURTHWALL_WEBHOOK_SECRET` on the desktop.
4. Fourthwall retries 5 times with a 5-second timeout and then gives up, so the relay must answer fast.

## Not planned here

**Stripe and PayPal Business** are core tips integrations (D-046) and still need their own runbooks (Stripe account and CLI access; PayPal Business REST app): proposed agent prep work in the DEN-244 artifact, see the README "Connection classes" rows for the sourced connection model (Stripe CLI browser pairing with API-key fallback; PayPal REST app credentials, no end-user consent). Buy Me a Coffee (HMAC webhooks), Tipeeestream (API key), Tiltify, DonorDrive/Extra Life: see [platform-apis.md section 10](../../research/platform-apis.md). Shopify is class 2 (merchant authorization through the broker, admin-created webhook as the interim path; README). Write runbooks when they enter scope.

## What to copy and where it goes (summary)

| Service | Value | Secret? | Goes to |
|---|---|---|---|
| Streamlabs | Socket API token | Yes | keychain / `SL_STREAMLABS_SOCKET_TOKEN` |
| StreamElements | JWT (or Overlay token) | Yes | keychain / `SL_STREAMELEMENTS_JWT` |
| Ko-fi | Verification token | Yes | desktop keychain only (never the relay) |
| Patreon | Creator access token (poll) / webhook secret | Yes | keychain / `SL_PATREON_CREATOR_TOKEN` |
| Throne | none (public key verifies) | No | n/a |
| Fourthwall | webhook signing secret | Yes | desktop keychain |

All names are suggestions; P0 decides the real storage location.

## Policy notes

- Respect each service's API terms: do not resell their data, and avoid storing donor personal data longer than needed (Q-H1 retention).
- Webhook URLs with random path segments are bearer secrets. Rotate them if leaked.

## Sources

- Streamlabs socket API: <https://dev.streamlabs.com/docs/socket-api>
- Streamlabs app registration: <https://dev.streamlabs.com/docs/register-your-application>
- Socket token dashboard path (Streamer.bot guide): <https://docs.streamer.bot/guide/integrations/streamlabs>
- StreamElements websockets: <https://docs.streamelements.com/websockets>
- StreamElements OAuth2 (authorize, exchange, refresh, validate, revoke; secret required): <https://github.com/StreamElements/api-docs/blob/main/docs/OAuth2.md>
- JWT location (secondary): <https://content-creator-integration.readthedocs.io/en/latest/gettingstarted/socketkey/streamelements/>
- Ko-fi webhooks page: <https://ko-fi.com/manage/webhooks>
- Patreon API docs (clients, webhooks): <https://docs.patreon.com/>
- Throne webhook integration: <https://help.throne.com/en/articles/14982436-webhook-integration>
- Fourthwall webhooks: <https://docs.fourthwall.com/webhooks/>
- Platform research: [platform-apis.md section 10](../../research/platform-apis.md)
