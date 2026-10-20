# Owner runbooks: registrations, accounts and go-live setup

These are the **human-only steps** that agents cannot do for you: creating developer apps, buying credits, enabling 2FA, proving your identity, copying secrets. Each runbook is click-by-click, with the exact values to type, what to copy, where it goes, and how to test it.

Written 2026-10-18 against current official developer docs. Developer consoles change often. Where I could not confirm a button or field label in an official source, the text says **(verify on screen)**. If your screen differs from a runbook, trust the screen and ask an agent to fix the runbook.

> **Tracked on the orchestrator board** (project `streamden`, cards labelled `kind:owner-action`): DEN-1 GitHub repo (**done**, https://github.com/TechDaddyKB/streamden; please verify) · DEN-6 Twitch dev app ·
> DEN-16 Round-2 answers · DEN-17 baseline accounts. Name decided: **StreamDen** (D-015). For dev apps during
> development, use "StreamDen Dev"; register production apps separately later.

## Contents

| File | What it covers |
|---|---|
| [registrations/twitch.md](registrations/twitch.md) | Twitch developer app (Public client, Device Code Flow), optional bot account, reward-ownership caveat |
| [registrations/google-youtube.md](registrations/google-youtube.md) | Google Cloud project, YouTube Data API v3, consent screen, Desktop OAuth client, 7-day token trap, verification, quota extension, end-user (BYO fallback) version |
| [registrations/kick.md](registrations/kick.md) | Kick developer app, scopes, PKCE, webhooks (pending relay decision) |
| [registrations/x-twitter.md](registrations/x-twitter.md) | X developer app, pay-per-use credits, OAuth 2.0 Native App, posting |
| [registrations/discord.md](registrations/discord.md) | Webhook path, application, bot token, intents, invite permissions |
| [registrations/rumble.md](registrations/rumble.md) | Rumble Live Stream API URL (no registration) |
| [registrations/tiktok.md](registrations/tiktok.md) | Why there is nothing official to register |
| [registrations/facebook.md](registrations/facebook.md) | Meta developer app, Login for Devices, App Review (later phase) |
| [registrations/streaming-services-donations.md](registrations/streaming-services-donations.md) | Streamlabs, StreamElements, Ko-fi, Patreon, Throne, Fourthwall |
| [registrations/openai.md](registrations/openai.md) | OpenAI API key, spend limits, "Sign in with ChatGPT" |
| [registrations/anthropic.md](registrations/anthropic.md) | Claude API key, spend limits, subscription policy |
| [registrations/xai.md](registrations/xai.md) | xAI API key and credits |
| [registrations/distribution-and-signing.md](registrations/distribution-and-signing.md) | GitHub rulesets, updater key, AUR, Windows signing, Apple, domain |
| [registrations/relay-cloudflare.md](registrations/relay-cloudflare.md) | Cloudflare account, Workers, Durable Objects, `wrangler`, the optional relay |
| [go-live-requirements.md](go-live-requirements.md) | What you need to be able to **stream** on each platform, stream keys, safe testing |

Related planning docs: [decisions.md](../decisions.md) (D-059: browser login first), [QUESTIONS.md](../QUESTIONS.md) (open decisions), [platform-apis.md](../research/platform-apis.md), [mcp-and-ai-providers.md](../research/mcp-and-ai-providers.md), [tech-stack.md](../research/tech-stack.md), [PLAN.md](../../PLAN.md) (phases P0 to P6).

## Conventions in these files

- **Bold** = a button or field you click or fill in. `code` = a value to type exactly.
- `<PRODUCT_NAME>` = the final product name (open question **Q-B1**).
- `<SL_PORT>` = the local server port. It is chosen in spec 06 (`docs/spec/06-api-mcp.md`, not yet written). Use the placeholder in forms you must fill before then, and come back to edit the URL. Note that X, Kick and Patreon match redirect URLs **exactly**, so the app needs a **fixed callback port** (or you register several URLs). Spec 06 must account for that. Twitch Device Code Flow, Google Desktop clients and OpenAI "Sign in with ChatGPT" need no fixed port.
- `<DOMAIN>`, `<PROJECT_EMAIL>`, `<RELAY_HOST>`, `<CLIENT_ID>`: obvious from context.
- **(verify on screen)** = I could not confirm this label or behavior in an official source.
- "Secondary source" or "community" = a third-party guide or forum, not the vendor.
- Shell examples use `read -rs VAR` so secrets do not land in your shell history. Run them only on your own machine.

## General rules

1. **Use a dedicated project identity.** Create one project email address (a mailbox you control, ideally on your own domain once Q-B1 is settled) and use it for the developer-side accounts: GitHub, Google Cloud, Cloudflare, AUR, Apple, and the AI vendors. Keep your personal and streaming accounts separate from these where the service allows, because some registrations are tied to the account that created them (a Twitch Client ID, a Google Cloud project, an Apple Account Holder). Your own streaming channels (Twitch, YouTube, Kick, ...) are needed as *test targets*; they do not have to own the developer apps.
2. **Turn on 2FA everywhere, and keep recovery codes in the password manager.** Twitch will not let you register an app without 2FA, and Kick will not unlock its developer tools without it. GitHub, Google, Cloudflare, Apple (mandatory), Discord and X should all have it. Prefer a passkey or hardware key for GitHub, Google, Cloudflare and Apple.
3. **Secrets live in a password manager, never in the repo, chat, or an AI agent.**
   - Never paste a secret, token, key, stream key or webhook URL into the repository, a commit message, an issue, a chat window, or a prompt to an agent. Agents need only the **names** of variables, not values. Public identifiers (a Twitch Client ID, a Discord Application ID) are fine to share.
   - If you pasted one by mistake, treat it as leaked: rotate it immediately (each runbook says how) and delete the message if you can.
   - Check that `.env.local` and `.env*.local` are listed in `.gitignore` before you create the file.
4. **Where each copied value eventually goes.** The real storage design is decided in **P0** (and Q-A5 for Linux keychains). Until then, assume:

   | Kind of value | Development (your machine) | Released app (end users) |
   |---|---|---|
   | Public identifiers (Twitch Client ID, Discord App ID, X Client ID for a public client) | `.env.local`, or a config constant in the source for the one shipped ID | Same shipped constant |
   | Secrets you own (Kick client secret, AI keys, bot tokens, webhook secrets, stream keys) | `.env.local` (gitignored) or the app's **Settings > Connections** screen, which writes to the OS keychain | OS keychain through the Settings screen (Secret Service on Linux, or an encrypted file with a warning if there is no daemon: Q-A5) |
   | CI/release secrets (signing keys, notarization credentials, SSH key for the AUR, Cloudflare token) | Password manager only | GitHub Actions secrets (ideally in a protected `release` environment) |
   | Relay/Worker secrets | n/a | `wrangler secret put`, never in the repo |

   Variable names used in these files (`SL_TWITCH_CLIENT_ID`, `SL_KICK_CLIENT_SECRET`, ...) are suggestions. P0 decides the real names and location.
5. **Do not register OAuth apps until the product name (Q-B1) is final.**
   - The app name appears on every consent screen (Twitch Connections, Google, Kick, X, Discord), and users will see it.
   - A **Twitch Client ID is permanent**, and custom channel-point rewards can only be managed by the Client ID that created them. Changing it later forces every streamer to recreate their rewards.
   - Renaming a Google OAuth app after verification means re-verification.
   - Package names (AUR, crates) and the domain are baked in at P0.
   - Q-B1's own recommendation is to keep "StreamDen" as the working title and check the domain/trademark before P0 ends. Answer Q-B1 first, then register. Things that do **not** depend on the name are safe to do now (see the order below).
6. **One registration per purpose.** Do not share one Client ID across different applications (Twitch can suspend it). Record every app in a "registrations" secure note in your password manager: service, owning account, creation date, Client ID, redirect URLs, console URL.
7. **Set spending limits when you create paid accounts.** OpenAI, Anthropic, xAI and X all let you cap spend. Do it at creation, with small credit amounts (about $5 to $10) while testing.
8. **Check scopes and permissions before clicking Authorize** on your own apps. Keep your own channels' sensitive scopes (like `streamkey:read` on Kick) out of anything you do not need.
9. **Test on throwaway targets first**: a test Discord server, a second Twitch/Kick account, unlisted YouTube streams. See [go-live-requirements.md](go-live-requirements.md).

## Connection model (D-059, owner 2026-10-19)

The owner's standing preference for **every** service: **browser login first, manual connection as the fallback.**
"Browser login" means the app sends the user to the provider's own sign-in and consent page in the **system
browser** (device code, loopback + PKCE, or the provider's "Sign in with …" program) against a developer app that
**StreamDen** registered once; the user copies nothing. "Manual" means the user pastes a token, key or URL, or
registers their own developer app ("bring your own", BYO) and types its IDs into **Settings > Connections >
Advanced**. Three different things are easy to mix up in these runbooks:

1. **One-time developer registration** (this folder): the owner creates the StreamDen app in a provider console.
   Done once per provider, never by end users, unless the provider forces BYO.
2. **End-user consent**: what a streamer clicks in StreamDen. With browser login this is one button and a
   provider web page; with manual connection it is a console walk-through plus a paste.
3. **Provider gates**: app verification, review, quota audits and secret requirements that decide whether 2 can be
   browser-only. D-059 does not skip them, and it does not invent flows a provider lacks.

Shared OAuth **confidential** secrets (Kick client secret, future Twitch app token) never ship inside the desktop
app; they live only on a server (the relay's token broker, `wrangler secret put`). Desktop-native public clients
(Twitch Public, Google Desktop, X Native App, OpenAI SIWC, Bluesky loopback) carry no secret by design.
"Browser login" never means reusing the user's **browser cookies or session**: Rumble's opt-in sign-in window is
the one exception the owner may accept (Q-C8), TikTok and X cookie login stay out (Q-C9, D-046).

| Service | Default (browser login) | Fallback (manual / BYO, under **Advanced**) | Provider gate that decides it (verified 2026-10-19) |
|---|---|---|---|
| Twitch | **Device Code Flow**: StreamDen shows a code, the user approves at `twitch.tv/activate` in the browser. Public client, no secret. | None needed; a BYO Client ID field only for developers. | Public clients get DCF only (no PKCE, no app tokens, no Chat Bot badge). Client ID permanent (rewards). |
| YouTube | **"Sign in with Google"** (loopback + PKCE, system browser) against the **shared StreamDen project**. Before Google verifies it the shared project serves limited development and test use only (test users, warning screen, 7-day tokens in Testing, lifetime 100-user cap); public rollout waits for verification (owner action, row 9). | **BYO Google project** wizard (Part B of the runbook): the supported path while the shared production sign-in is unavailable, and an option afterwards for users who want their own quota; still browser consent at the end. | Sensitive-scope verification gates public rollout; the separate quota audit gates capacity (one 10,000-unit pool for all users until approved); Testing-mode tokens die after 7 days. |
| Kick | **Preferred target:** "Sign in with Kick" (PKCE in the browser) **through the official hosted relay's token broker** (D-019: later; timing and budget are Q-C12). | **Interim default and advanced fallback:** **BYO Kick app** + self-hosted relay or tunnel, until the broker exists. | `client_secret` required for every exchange and refresh; no public client ([#413](https://github.com/KickEngineering/KickDevDocs/issues/413) open); events webhook-only. |
| X | Browser PKCE consent (Native App, `127.0.0.1` callback) **on the user's own X developer app** (D-046: BYO app, BYO credits). | The BYO app *is* the manual step; there is no shared-app default because X bills per app. | Pay-per-use credits per app; exact callback match; no cookie login. |
| Bluesky | **atproto OAuth** loopback (public client: PKCE, PAR, DPoP) in the browser. | **App password** (paste) for unattended rigs: public-client OAuth sessions are capped at 2 weeks. | Spec: 2-week session and refresh lifetime for public clients; app passwords are "for bots and CLI tools". |
| Discord go-live posts | **Preferred target:** Discord's `webhook.incoming` browser consent (the user picks the channel on Discord's page; the token response carries the webhook `id` and `token`), through the hosted relay's token broker because every Discord token call needs the client secret (D-019: later). | **Interim default and advanced fallback:** paste a **channel webhook URL** (runbook step 0). Both paths store the same webhook credential. | Confidential client only (no PKCE public client documented); broker availability decides it. |
| Discord bot (P4, Q-S7) | **Bot invite link** opens in the browser (consent, no token exchange); the bot token is still pasted once from the user's own app (BYO bot). | Same. | Token exchange needs a secret; privileged intents gate at 10,000 users. |
| Facebook Live (P5) | **Login for Devices** (code shown, approve at `facebook.com/device`) with the StreamDen app. | None. | Standard vs Advanced Access (App Review + Business Verification) decides who can consent; device tokens last up to 60 days, no refresh. |
| Rumble | No OAuth exists. Read: paste the **Live Stream API URL**. Send (Q-C8, opt-in): StreamDen **sign-in window** on rumble.com, cookie to the keychain. | Paste the session cookie (advanced, warned). | No developer program; automated access is against Rumble's terms (hence opt-in). |
| TikTok | Nothing official. Community plugin only (D-024, Q-C9). | Plugin settings. | No LIVE API; session cookies would go to a third-party signer. |
| Streamlabs | **Preferred target:** "Connect with Streamlabs" browser consent on a StreamDen OAuth app, through the hosted broker (the exchange needs the client secret). Until Streamlabs approves the app only 10 whitelisted users may authorize it. | **Interim default and advanced fallback:** paste the **Socket API token** (official, no app). | App approval (owner request) plus broker availability (D-019: later). |
| StreamElements | **Preferred target:** "Connect with StreamElements" browser consent: SE documents authorize, code exchange, refresh, validate and revoke; exchange and refresh need the client secret, so they run in the hosted broker. | **Interim default and advanced fallback:** paste the **JWT** (or overlay token). | How SE issues `client_id`/`client_secret` is undocumented (owner request to SE); broker availability (D-019: later). |
| Ko-fi, Throne | Paste a verification token or secret **and** a public webhook URL (relay or tunnel). No login flow exists. | Same. | Webhook-only; relay dependency (Q-C1). |
| Fourthwall | **Preferred target:** Fourthwall's OAuth consent in the browser on a StreamDen platform app, through the hosted broker: the code exchange and the refresh at `/open-api/v1.0/platform/token` both send `client_id` **and** `client_secret`, and access tokens expire "within a few minutes" ([OAuth guide](https://docs.fourthwall.com/guides/oauth), read 2026-10-19). | **Interim default and advanced fallback:** paste the **webhook signing secret** and the relay URL (runbook §6). | App publishing is "still developing" and reviewed by Fourthwall over Discord ([publishing](https://docs.fourthwall.com/apps/publishing)); the scope list and whether an OAuth token may create webhooks (the [webhook API](https://docs.fourthwall.com/webhooks/api-management) examples use Basic auth) are undocumented, so a prep card verifies them before any owner task; broker availability (D-019: later, Q-C12). |
| Patreon | **Preferred target:** "Sign in with Patreon" browser consent on the StreamDen client, through the hosted broker (exchange and refresh need the client secret server-side). | **Interim default and advanced fallback:** paste the **Creator's Access Token** (local polling); webhooks need the relay either way (Q-C1). | Broker availability (D-019: later); scopes fixed at broker design time. |
| OpenAI | **"Continue with ChatGPT"** (SIWC: dynamic registration, PKCE, `127.0.0.1` loopback, no secret) for Plus/Pro plan usage, with its preview request constraints. | Paste an **API key** (pay-as-you-go; a key is not a subscription and a subscription is not a key). | Open-source/local apps need no **manual** developer registration (the app registers itself dynamically on first sign-in); paid or hosted apps need OpenAI's interest form. |
| Anthropic | No browser login: Anthropic prohibits Claude.ai login or Pro/Max credentials in third-party apps (D-021). | Paste an **API key** (the only path). "Connect your AI": the user's Claude Desktop/Code talks to our MCP server. | Policy, not technology. |
| xAI | No published program for SuperGrok/X Premium inside third-party apps (gray, Q-A1 c). | Paste an **API key** (prepaid credits). Grok Build connects to our MCP server. | Policy gap. |
| Cloudflare relay (owner only) | `wrangler login` opens the browser (`--browser=false` prints the link). | API token in CI. | n/a (owner tooling). |

Paid AI APIs are **not** consumer subscriptions: an API key bills the key owner per token, and a Plus/Pro/Max/SuperGrok
plan never funds it. Only OpenAI publishes a plan-usage sign-in for local apps.

### Connection classes, and the integrations without a runbook yet

Every service falls into one of five classes. The class, not the preference, decides what the UI may offer:

1. **Browser authorization, desktop-native** (no shared developer secret; any per-installation credential stays in the keychain): Twitch DCF, Google loopback + PKCE, X Native App
   PKCE, Bluesky atproto OAuth, Facebook Login for Devices, OpenAI SIWC, Pulsoid device flow, Spotify PKCE (on the
   user's own client ID, D-050), Crowd Control code login, Home Assistant instance OAuth (PKCE against the user's own
   instance), Stripe CLI browser pairing (`stripe login`), Mastodon (instance-issued per-installation credential).
2. **Browser authorization through the hosted token broker** (the provider needs a confidential secret): the
   **preferred target**, with the manual paste as the **interim default** until the provider's app approval and the
   broker exist (D-019 unchanged, timing Q-C12): Kick, Discord `webhook.incoming`, Streamlabs, StreamElements, Patreon,
   Fourthwall, and, when those later plugins are sliced, Threads (D-046) and Shopify merchant authorization.
3. **Key or token paste only** (no end-user sign-in for third parties exists): Anthropic, xAI, ElevenLabs, Azure
   Speech, HypeRate, Pally.gg, Lumia Stream, Streamlabs Desktop remote control, PayPal Business REST
   app credentials, Ko-fi/Throne/Buy Me a Coffee webhook secrets, the Rumble Live Stream API URL. Tipeeestream and
   Tiltify are **unclassified** until sliced (observed OAuth, table below); they are not in this class.
4. **Local pairing** (no account at all): VTube Studio plugin popup, ytmdesktop code approval, Hue link button, WLED
   (plain HTTP/JSON on the LAN, D-050),
   Nanoleaf power-hold, Voicemod client key, OBS WebSocket password, Elgato Key Light, VMC/OSC, DonorDrive public API.
5. **Not offered**: TikTok (no API, Q-C9), cookie reuse anywhere except the Rumble opt-in (Q-C8), Discord RPC voice
   scopes (Discord approval), Claude.ai or SuperGrok login.

| Integration (research only, no runbook yet) | Class | Default (browser login) | Fallback (manual) | Provider gate |
|---|---|---|---|---|
| Pulsoid (heart rate) | 1 | **Device authorization flow**: StreamDen shows a code, the user approves on pulsoid.net; both the device-code and the token request send only `client_id` ([device flow](https://docs.pulsoid.net/getting-started/access-token-management/oauth-2-device-authorization-flow.md), read 2026-10-19). | Paste a **manual token**: "suitable for personal use, client credentials are not needed" ([docs](https://docs.pulsoid.net/), read 2026-10-19). | Owner creates client credentials at `pulsoid.net/ui/api-clients` (runbook still to write when the connector is sliced). |
| Spotify (song requests, plugin) | 1 | Browser **PKCE** consent with no client secret and an exactly registered loopback redirect ([PKCE flow](https://developer.spotify.com/documentation/web-api/tutorials/code-pkce-flow), read 2026-10-19). | The BYO client ID *is* the manual step (D-050 / Q-I3): Dev Mode needs the owner on Premium and allows 5 users, so no shared StreamDen app. | Spotify's February 2026 Dev Mode limits; OS now-playing (MPRIS/GSMTC) stays the core path. |
| Crowd Control | 1, approval-gated | Browser **code login**: the app requests a code and URL, the user signs in at Crowd Control, public apps exchange with a PKCE verifier ([sockets](https://developer.crowdcontrol.live/sockets/), read 2026-10-19). | None documented by Crowd Control (Streamer.bot's pasted Overlay URL is an undocumented route, research). | "Applications need to be created manually by our developers" (request via their Discord); tokens last 24 h unless `token:extend`. |
| HypeRate (heart rate) | 3 | none | **API key** issued per application (research, inferred). | Key issuance by HypeRate (owner action when sliced). |
| ElevenLabs (TTS, D-033) | 3 | none: no OAuth or end-user sign-in is documented. | **API key** in the `xi-api-key` header, billed to the key owner ([authentication](https://elevenlabs.io/docs/api-reference/authentication), read 2026-10-19). | A consumer subscription is not an API key. |
| Azure Speech (TTS, D-033) | 3 | none for consumers (Microsoft Entra bearer tokens serve organisations, not a streamer's personal account). | **Resource key** (`Ocp-Apim-Subscription-Key`) plus region or endpoint ([REST reference](https://learn.microsoft.com/en-us/azure/ai-services/speech-service/rest-text-to-speech), read 2026-10-19). | Azure account and a Speech resource (free F0 tier). |
| Home Assistant (smart-home bridge, D-050) | 1 | **Instance OAuth in the browser**: StreamDen opens `/auth/authorize` on the user's own instance; code exchange and refresh at `/auth/token`, PKCE `S256` ("always in a native app"), revoke at `/auth/revoke`; refresh tokens expire after 90 days unused ([auth API](https://contributors.home-assistant.io/core/internals/auth-api/), read 2026-10-19). Local hosting does not make it paste-only. | **Long-lived access token** paste (user profile > Security, 10-year token), under Advanced. | The `client_id` must be a public website URL that StreamDen serves, and a loopback redirect on another host must be declared there with `<link rel="redirect_uri">` (or a JSON client-metadata document): a static page on the project domain (row 3), no secret. Client page and callback setup are pending design (proposed prep card, DEN-244 artifact). |
| Lumia Stream, Streamlabs Desktop | 3 | none (local apps without a cloud identity or OAuth) | API token, Remote Control token ([research](../research/app-integrations-vtuber.md) §6, §7). | n/a |
| Pally.gg | 3 | none | **API key** (research). | Not in scope yet. |
| Tipeeestream, Tiltify | Unclassified | Verify when sliced. Research: Tipeeestream also has an OAuth with a localhost callback (observed in Streamer.bot); Tiltify undocumented OAuth observed in Streamer.bot. | Verify when sliced. Research: Tiltify app token (client credentials with a secret). | Auth capability and registration requirements need primary-source verification before implementation; do not assume paste-only or promise OAuth. |
| Shopify (merch orders) | 2 | **Preferred target:** merchant authorization in the browser (`https://{shop}/admin/oauth/authorize`, code grant; the exchange at `/admin/oauth/access_token` needs `client_id` **and** `client_secret`, so it runs in the hosted broker) with app-managed webhook subscriptions ([standalone apps](https://shopify.dev/docs/apps/build/authentication-authorization/authenticate-standalone-apps), [subscribe](https://shopify.dev/docs/apps/build/webhooks/subscribe), read 2026-10-19). | **Interim default and advanced fallback:** the merchant creates the webhook by hand in the Shopify admin (**Settings > Notifications > Webhooks > Create webhook**: event, JSON, HTTPS relay URL, API version; [help](https://help.shopify.com/en/manual/fulfillment/setup/notifications/webhooks)) and StreamDen verifies the signature. | A StreamDen app for arbitrary merchants must use **public distribution** (Shopify App Store review); custom distribution installs on one store or one Plus organisation only ([distribution](https://shopify.dev/docs/apps/launch/distribution)). Scopes, review and the broker (D-019, Q-C12) decide it; relay for webhooks (Q-C1). |
| Buy Me a Coffee, DonorDrive | 3 / 4 | none | Webhook secret; public REST without auth (research). | Relay dependency for Buy Me a Coffee (Q-C1). |
| Stripe (tips, core per D-046) | 1 | **Browser pairing through the Stripe CLI sidecar** (`stripe login`): the CLI shows a pairing code and opens the Dashboard, the user confirms, credentials land in the OS credential store and refresh automatically ([CLI login](https://docs.stripe.com/cli/login), read 2026-10-19). `stripe listen --live` then delivers events locally without a public URL (research §1.2). | **API key** paste: `stripe login --interactive`, or a **restricted read-only key** for the `GET /v1/events` polling path; a Dashboard webhook to the relay as a third option. | Stripe account (tips must be for a good or service; no P2P). For CLI versions newer than v1.50.0 an account **Administrator must enable CLI access** (Settings > Team and security > MCP and CLI access). **Stripe Connect OAuth is not used**: it is a platform feature whose token exchange uses the platform's secret key ([Connect OAuth](https://docs.stripe.com/connect/oauth-reference)), a different product from the user's own account. |
| PayPal Business (tips, core per D-046) | 3 | none: PayPal's REST authentication is a `client_credentials` grant with the app's client ID and secret ([REST auth](https://developer.paypal.com/api/rest/authentication/), read 2026-10-19); it authenticates the user's **own REST app**, not an end user, so there is no consent page to prefer. No partner or "Log in with PayPal" onboarding is assumed. | The user creates a REST app on their own Business account and pastes its client ID and secret (keychain); webhooks need the relay (Q-C1); Payment Links carry Venmo for US merchants (research §1.2). | Business account; relay for webhooks; the Transaction Search API lags up to 3 h (reconciliation only). IPN is retired: never built on. |
| Other payment rails (research §1) | 5 / plugin | none | Venmo-at-checkout is covered by the PayPal row. **Not offered:** Venmo direct, Cash App direct, Zelle, X Money (no API), PayPal personal as core, IPN, any login to or scraping of a payment app (D-046). **Deferred, not in D-046 core:** Cash App Pay via Square. | The experimental email-alerts plugin (off by default, D-046) reads a **dedicated tips mailbox**: Gmail **app password** is the interim default because the restricted `mail.google.com` scope needs its own Google verification before a shared project may offer XOAUTH2 in the browser (that OAuth is the D-059 target, same gate class as YouTube's); Outlook.com is OAuth-only on a BYO Entra app (research §1.3). |
| Threads (go-live announcement plugin, later per D-046) | 2 | **Preferred target:** Threads authorization window in the browser on a StreamDen Meta app with the Threads use case; the code exchange at `POST graph.threads.com/oauth/access_token` requires `client_secret` and the 60-day long-lived exchange "include[s] your app secret" (server-side), so both run in the hosted broker; the long-lived refresh needs only the token ([get started](https://developers.facebook.com/docs/threads/get-started), [tokens](https://developers.facebook.com/docs/threads/get-started/get-access-tokens-and-permissions), [long-lived](https://developers.facebook.com/docs/threads/get-started/long-lived-tokens), read 2026-10-19). | **BYO Meta app** (the user is their own Threads Tester, research §3.8, unverified) with the same browser window; no token paste is documented. | Until each permission passes App Review and the app is published, only Threads Testers can grant it (owner action when sliced); broker availability (D-019, Q-C12); no PKCE or public client documented. |
| Mastodon (go-live announcement plugin, later per D-046) | 1 | **Browser authorization on the user's own instance:** StreamDen registers itself on that instance with `POST /api/v1/apps` (proprietary, not Dynamic Client Registration) and gets a per-installation `client_id`/`client_secret`, opens `GET /oauth/authorize` in the browser and exchanges the code at `POST /oauth/token`; the secret is instance-issued to this install and stays in the keychain, never shared ([authorized flow](https://docs.joinmastodon.org/client/authorized/), [OAuth spec](https://docs.joinmastodon.org/spec/oauth/), read 2026-10-19). | Paste a token generated under **Settings > Development** (advanced; research §3.8). | Mastodon "only supports confidential clients"; PKCE exists from 4.3.0 and server metadata at `/.well-known/oauth-authorization-server` from 4.3.0, so older or forked instances differ; token lifetime and refresh are not stated on the fetched pages (verify when sliced). |
| VTube Studio, ytmdesktop, Voicemod, Hue, Nanoleaf, WLED, Elgato, OBS, VMC/OSC, VRChat | 4 | n/a: local pairing (plugin popup, code approval, link button, power-hold, none, client key, password, none) | same | [app-integrations-vtuber.md](../research/app-integrations-vtuber.md) §2, §5, §6 and §11 (`AuthFlow`). |
| Discord local RPC (voice overlay) | 5 | `rpc.voice.*` scopes need Discord approval; experimental flag only (research §7). | n/a | Discord approval. |

Rows marked "research" cite the existing research files rather than a fresh 2026-10-19 source read; re-verify when the
connector is sliced.

## Master checklist

Phases are from [PLAN.md](../../PLAN.md): P0 Foundations, P1 Core (Twitch, v0.1), P2 Chat + TTS (YouTube, Kick, X posting, Discord, relay), P4 Integrations/SDK/AI, P5 Decks + donation services, P6 Ecosystem. "Blocked by decision" refers to [QUESTIONS.md](../QUESTIONS.md). All are still open at the time of writing.

| # | Service | Why we need it | Needed by phase | Cost | Blocked by decision | Status |
|---|---|---|---|---|---|---|
| 0 | Project identity: email, password manager, authenticator/passkeys, 2FA | Foundation for every account below | P0 (now) | Free (KeePassXC/Bitwarden free tier) | none | ☐ |
| 1 | GitHub: account, repo, branch ruleset, Actions secrets ([distribution](registrations/distribution-and-signing.md)) | Code hosting, CI, multi-agent PRs, releases, updater endpoint | P0 | Free if public. Private repos need Pro/Team (about $4/mo, verify) for branch protection | Q-B4 (visibility), Q-B1 (repo name) | ☐ |
| 2 | Tauri updater key + GPG key ([distribution](registrations/distribution-and-signing.md)) | Signed auto-updates; signed RPM and checksums | P0 | Free | none (but back them up) | ☐ |
| 3 | Domain ([distribution](registrations/distribution-and-signing.md)) | Homepage and privacy policy (Google, Twitch, Meta, SignPath need them), docs, relay hostname | P0 to P2 | about $10-20/year (verify at registrar) | Q-B1 | ☐ |
| 4 | AUR account + `-bin` package ([distribution](registrations/distribution-and-signing.md)) | Arch distribution | P0 (account), first push at v0.1 | Free | Q-B1 (name), Q-B2 (license) | ☐ |
| 5 | Windows code signing: **SignPath Foundation (chosen, D-055)**; Azure Artifact Signing not pursued ([distribution](registrations/distribution-and-signing.md)) | Avoid SmartScreen warnings | Before the first public release (v0.1) | SignPath free for OSS; Azure Basic $9.99/mo | Q-B2 (OSI license), Q-B4 (public repo), Q-G3 (budget) | ☐ |
| 6 | Apple Developer Program ([distribution](registrations/distribution-and-signing.md)) | Developer ID signing + notarization | P0 build, signing at v0.5 (or earlier if Q-P2 says so) | $99/year | Q-P2 | ✅ Account exists (owner, 2026-10-19, D-055); signing is still gated by Q-P2 |
| 7 | Twitch developer app ([twitch](registrations/twitch.md)) | All Twitch features: EventSub, chat, moderation, rewards | P1 | Free | Q-B1 | ☐ |
| 8 | Stream-ready test accounts and eligibility ([go-live](go-live-requirements.md)) | Test against real platforms; some have waiting periods | P1 onward (start the clocks now) | Free; X Premium is paid | Q-S1 (which platforms) | ☐ |
| 9 | Google Cloud + YouTube Data API ([google-youtube](registrations/google-youtube.md)) | YouTube chat, events, moderation; the **shared project is the D-059 default** sign-in. Owner follow-ups, not questions: OAuth verification (public rollout) and the quota audit (capacity), see the runbook's "Production verification" | P2 for the project and test use; verification before the YouTube connector's public release | Free; quota 10,000 units/day | Q-C2 answered; Q-S1; domain for verification (Q-B1) | ☐ |
| 10 | Kick developer app ([kick](registrations/kick.md)) | Kick chat send, moderation, events (webhooks); "Sign in with Kick" only via the hosted broker (D-019 later, Q-C12) | P2 | Free | Q-C1 (webhooks/secret), Q-S1, Q-C3, Q-C12 | ☐ |
| 11 | Cloudflare account + `wrangler` ([relay](registrations/relay-cloudflare.md)) | Optional self-hostable relay for Kick events and webhook-only donation services | P2 | Free tier (Workers Free + Durable Objects SQLite) | Q-C1 | ✅ Account exists (owner, 2026-10-19, D-055); deployment still P2 |
| 12 | X developer app + credits ([x-twitter](registrations/x-twitter.md)) | Post go-live and clip tweets | P2 | Pay-per-use: $0.015 per post, $0.20 per post with a link; buy about $5-10 to test | Q-B1, Q-C4 | ☐ |
| 13 | Discord: webhook and (optional) bot application ([discord](registrations/discord.md)) | Go-live posts (webhook, no app) and optional chat bridge (bot) | P2 | Free | Q-B1 (bot name), Q-S1 | ☐ |
| 14 | OpenAI API key + "Sign in with ChatGPT" ([openai](registrations/openai.md)) | In-app assistant and AI chat responder; plan-usage sign-in | P4 | Pay-as-you-go (start with about $10). SIWC plan usage: no manual developer registration for open-source/local apps | Q-A1; Q-B2 and Q-B3 for SIWC eligibility | ☐ |
| 15 | Anthropic API key ([anthropic](registrations/anthropic.md)) | Claude as a provider (API key only; no subscription sign-in) | P4 | Pay-as-you-go | Q-A1 | ☐ |
| 16 | xAI API key ([xai](registrations/xai.md)) | Grok as a provider | P4 | Prepaid credits (auto top-up minimum $5) | Q-A1 | ☐ |
| 17 | Streamlabs socket token, later the OAuth app ([donations](registrations/streaming-services-donations.md)) | Tips and alerts; the OAuth app (needs Streamlabs approval) is the D-059 target behind the broker | P5 | Free | none for the token; Q-C12 for the app | ☐ |
| 18 | StreamElements JWT, later OAuth client credentials ([donations](registrations/streaming-services-donations.md)) | Tips and alerts; ask SE for `client_id`/`client_secret` for the D-059 target | P5 | Free | none for the JWT; Q-C12 for OAuth | ☐ |
| 19 | Ko-fi webhook ([donations](registrations/streaming-services-donations.md)) | Ko-fi events | P5 | Free | Q-C1 (needs relay) | ☐ |
| 20 | Patreon client ([donations](registrations/streaming-services-donations.md)) | Member/pledge events (creator token now; "Sign in with Patreon" via the broker later) | P5 | Free | Q-C1 for webhooks; polling needs none; Q-C12 for OAuth | ☐ |
| 21 | Throne, Fourthwall webhooks; later the Fourthwall OAuth app ([donations](registrations/streaming-services-donations.md)) | Gifts, shop orders; the platform app is the D-059 target behind the broker | P5 | Free | Q-C1; Q-C12 and the Fourthwall prep card for the app | ☐ |
| 22 | Rumble Live Stream API URL ([rumble](registrations/rumble.md)) | Rumble followers, subs, rants, chat | P5 | Free | Q-S1 | ☐ |
| 23 | Meta developer app ([facebook](registrations/facebook.md)) | Facebook Live comments | P5 or later | Free; Business Verification effort for public use | Q-S1 | ☐ |
| 24 | TikTok ([tiktok](registrations/tiktok.md)) | Nothing official exists to register | P6, optional plugin | Free (third-party signer has paid tiers) | Q-S1 | ☐ |

Not covered yet (write a runbook when the feature enters scope): **Stripe account and CLI access, PayPal Business REST app** (core tips per D-046, P2; runbooks are proposed agent prep work in the DEN-244 artifact), ElevenLabs/Azure/other TTS keys (Q-T1), Spotify client ID (Q-I3), Pulsoid client credentials, the Crowd Control application request, the Home Assistant client page on the project domain, the Fourthwall platform app (scopes, webhook permission and publishing eligibility first), Threads Meta app and Mastodon plugin notes (later per D-046), Tiltify/Buy Me a Coffee/Tipeeestream/DonorDrive/Shopify, Elgato Marketplace (Stream Deck plugin), a Twitch Extension (Q-S6), Flathub (P6), winget, the plugin registry GitHub org (D-008). Their connection classes are in the table above.

## Recommended order

| When | Do | Why now |
|---|---|---|
| **Today (no decisions needed)** | Row 0: project email, password manager, authenticator/passkeys. | Everything else depends on it. |
| **Today** | Start the slow clocks from [go-live-requirements.md](go-live-requirements.md): enable YouTube live streaming (up to 24 h), enable Twitch 2FA, get Kick/Rumble accounts ready, check your Twitch Affiliate progress. | Calendar time, not effort. |
| **Today** | Row 2: generate the Tauri updater key and a GPG key and back them up. Row 1: GitHub account + 2FA. Cloudflare account (row 11, account only). AUR account (row 4, account and SSH key only). | None of these need the name. |
| **Any time (optional, cheap)** | Rows 14-16: AI API keys with small credits and spend limits. | They do not carry the product name. Not needed until P4. |
| **Answer the questions** | **Q-B1 first** (name), then Q-B2 (license), Q-B4 (GitHub visibility), Q-P2 (macOS), Q-C1 (relay), Q-C2 (YouTube project), Q-C4 (X), Q-S1 (platform scope), Q-G3 (budget). See "Decisions that block registrations" below. | Unlocks the rest. |
| **After Q-B1** | Row 3: domain. Row 1: create the repo under the final name. Row 4: reserve the AUR package name (first push later). | Names get baked in at P0. |
| **Start of P1** | Row 7: the Twitch app. Then smoke-test it with the device-flow `curl` commands. | P1 needs it. Twitch is certain. |
| **Before the first public release** | Row 5: Windows signing (SignPath needs a public repo and an OSI license; apply once there is a release). Row 6 only if Q-P2 says (a). | Needs a release to apply. |
| **P2** | Row 9 (Google/YouTube), 13 (Discord), 10 (Kick, dev app only), 12 (X), 11 (relay deploy), in that order. | Matches the P2 deliverables. |
| **P4** | Rows 14-16 finishing touches: SIWC testing with a Plus/Pro account. | AI layer. |
| **P5 and later** | Rows 17-24 only for services you actually use. | Donation services, Rumble, Facebook. |

## Decisions that block registrations

| Decision | Blocks |
|---|---|
| **Q-B1 product name** | Every OAuth app (Twitch, Google, Kick, X, Discord, Patreon, Meta), the repo name, AUR package name, domain, privacy-policy text, consent-screen branding. **Do not register OAuth apps before this.** |
| Q-B2 license | SignPath Foundation (needs an OSI license without commercial dual-licensing), the AUR `license=` field, OpenAI SIWC eligibility ("open-source") |
| Q-B3 monetization | Whether SIWC needs OpenAI's paid/hosted waitlist approval |
| Q-B4 git hosting/visibility | GitHub plan (branch protection on private repos is paid), SignPath (public repo), free Actions minutes |
| Q-C1 relay | Cloudflare deployment, Kick webhooks, Ko-fi, Throne, Fourthwall, Patreon webhooks, Kick token brokering |
| Q-C2 YouTube project (answered: D-031, order amended by D-059) | Nothing: the shared project is the default and usable for test use now; its OAuth verification and quota audit are owner actions in the runbook, not open questions; the BYO wizard is the fallback |
| **Q-C12** hosted token broker timing | Whether "Sign in with Kick / Streamlabs / StreamElements / Patreon" and the Discord webhook consent arrive before or with the official hosted relay (D-019: later). The manual paths ship regardless |
| Q-C4 X | Whether you bother registering an X app at all |
| Q-P2 macOS tier | Apple Developer Program purchase |
| Q-S1 platform scope | Which of Kick, Rumble, Facebook, TikTok, Discord runbooks you actually run |
| Q-A1 AI subscriptions | Which AI accounts you create and whether to contact Anthropic/xAI |
| Q-G3 budget (answered, D-055) | Apple and Cloudflare accounts already exist and SignPath is the Windows signer; only the domain and test credits remain |

## Handing results back to the agents

After each registration, tell the planning agent only the public facts: "Twitch app `<PRODUCT_NAME>` created, Public client, Client ID is `abc...` (public), redirect `http://localhost:<SL_PORT>/auth/twitch/callback`". Never the secrets. Agents record the public IDs in the specs and code; you keep the rest in your password manager and local `.env.local`.
