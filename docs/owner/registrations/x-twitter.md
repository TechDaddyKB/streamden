# X (Twitter): developer account and app

Last verified against official docs: 2026-10-18. Part of the [owner runbooks](../README.md).

## Purpose

StreamDen treats X as an **action target**: post a "we're live" tweet (with or without an image), post clip announcements and optionally read your own mentions. It does **not** read X Live chat (that needs the whitelisted Livestream API, see Policy notes). Per **Q-C4** each user registers their **own** X developer app and pays X directly.

The auth decision from the platform research: OAuth 2.0 Authorization Code with **PKCE** as a **Native App** (public client, no secret), with a loopback callback on `127.0.0.1`.

## Prerequisites

- [ ] An X account to own the app (the streamer's own account or a project account). Enable 2FA on it.
- [ ] A payment card for buying credits (there is no free API tier any more, apart from public-utility apps).
- [ ] Final product name (Q-B1) and, optionally, a website URL (your future domain or the GitHub repo URL).
- [ ] `<SL_PORT>` from spec 06. X requires an **exact** callback match, so the app must use a **fixed** callback port (or you register several URLs, max 10). This is a design constraint for spec 06.
- [ ] Q-C4 answered (pay-per-use acceptable for BYO users; Bluesky alongside).

## Cost and limits

X moved to **pay-per-use** on 2026-02-06. Credits are bought in advance in the Developer Console and deducted per request. Current official rates (the page itself says the console shows live prices):

| Operation | Price |
|---|---|
| Create post | **$0.015** per request |
| Create post **containing a URL** | **$0.20** per request |
| Read a post | $0.005 per post returned |
| Read a user | $0.010 per user |
| Owned reads (your own posts, mentions, followers) | $0.001 per resource |
| Media metadata | $0.005 per request (no separate price is listed for `media.write` uploads: check the console or test with a small credit) |
| Cap | 3 million post reads per billing month |

Key consequences:

- A go-live post such as "I'm live: twitch.tv/name" contains a URL, so it costs **$0.20**. A text-only or image-only post costs $0.015. StreamDen's UI must warn the user before enabling URL posts.
- **Spending limit** per billing cycle is available (requests block once reached). **Auto-recharge** needs a default payment method and runs at most once per 5-minute window.
- New payers can earn **$20 in X API credits** after saving a card and making a first purchase, and a first auto-recharge can be matched up to $50 (the research notes these expire after about 3 months; check the console).
- Following, liking and quote-posting via API were removed from self-serve tiers on 2026-04-20.

## Step-by-step

### A. Developer account and credits

1. Go to <https://console.x.com> and sign in with the X account that will own the app.
2. Review and accept the **Developer Agreement and Policy**.
3. Complete the profile: describe how you will use the API. Example text: "Personal streaming-automation app: posts a message to my own account when I go live. Posts only on my own account, with my explicit setup."
4. Open the credits/billing area of the console (menu names: verify on screen). Add a payment card as the default payment method and buy a small amount of credits, for example $5 to $10 for testing.
5. Set a **maximum spend per billing cycle** (for example $5) so a bug cannot drain the card. Leave auto-recharge **off** until you trust the setup.

### B. Create the app

6. On the console dashboard click **New App** (some pages say **Create App**; verify on screen).
7. Enter the **app name** (`<PRODUCT_NAME>`), a description and the use case.
8. Open the app's **User authentication settings** (label: verify on screen).
9. Choose OAuth 2.0 and for **Type of App** choose **Native App**. Native App and Single Page App are public clients (PKCE only, no Client Secret). Web App and Automated App / Bot are confidential clients that get a secret. The exact label "Type of App" is not in the docs, so verify on screen.
10. **App permissions**: **Read and write** (needed to post). The OAuth 2.0 scopes are requested at runtime.
11. **Callback URI / Redirect URL**: `http://127.0.0.1:<SL_PORT>/auth/x/callback`
    - Use `127.0.0.1`, **not** `localhost`. Entries must match exactly including trailing slashes, max 10 per app, `https://` in production, and `javascript:`, `data:` and `file:` schemes are rejected.
12. **Website URL**: your project site or the GitHub repo URL (the field may be required; verify on screen).
13. Save. The console shows credentials **once**. Copy the **Client ID** (for a Native App there is no Client Secret). The console may also generate an **API Key and Secret**, an **Access Token and Secret** and a **Bearer Token**: we do **not** use these. Do not store them. If you generated them, keep them out of any file.

### C. What the app will request

- Authorize URL: `https://x.com/i/oauth2/authorize` (PKCE `S256`).
- Token URL: `https://api.x.com/2/oauth2/token`.
- Scopes: `tweet.read tweet.write users.read media.write offline.access`
  - `tweet.write` posts, `media.write` uploads images, `users.read` + `tweet.read` identify the user, `offline.access` returns a refresh token (access tokens last only 2 hours).
- Endpoints we use: `POST /2/tweets`, `POST /2/media/upload` for images up to 5 MB (JPG, PNG, GIF, WEBP; animated GIF up to 15 MB), and the chunked `POST /2/media/upload/initialize` flow for video (up to 16 GB).

## What to copy and where it goes

| Value | Secret? | Where it goes |
|---|---|---|
| **Client ID** (Native App) | No (public client) | `SL_X_CLIENT_ID` in `.env.local` / Settings > Connections > X |
| API Key/Secret, Access Token/Secret, Bearer Token | Yes | **Not used. Do not copy.** Regenerate or delete if exposed. |
| User access and refresh tokens | Yes | Runtime only, OS keychain |

(Variable names are suggestions; P0 decides the real location.)

## Verification and test

Cost of this test is about $0.03 or less. **Do not paste tokens into chat or an AI agent.**

```bash
VERIFIER=$(openssl rand -base64 48 | tr -d '=+/\n' | cut -c1-64)
CHALLENGE=$(printf %s "$VERIFIER" | openssl dgst -sha256 -binary | openssl base64 -A | tr '+/' '-_' | tr -d '=')
echo "https://x.com/i/oauth2/authorize?response_type=code&client_id=<CLIENT_ID>&redirect_uri=http%3A%2F%2F127.0.0.1%3A<SL_PORT>%2Fauth%2Fx%2Fcallback&scope=tweet.read%20tweet.write%20users.read%20offline.access&state=abc123&code_challenge=$CHALLENGE&code_challenge_method=S256"
```

1. Open the URL, approve. The redirect to `127.0.0.1` fails to load; copy `code` from the address bar **within 30 seconds** (the code expires fast).
2. Exchange it (public client: send `client_id`, no secret):

```bash
curl -s -X POST https://api.x.com/2/oauth2/token \
  -d grant_type=authorization_code -d client_id=<CLIENT_ID> \
  -d redirect_uri='http://127.0.0.1:<SL_PORT>/auth/x/callback' \
  -d code=<CODE> -d code_verifier="$VERIFIER"
```

3. Check identity (an owned read): `read -rs T; curl -s -H "Authorization: Bearer $T" https://api.x.com/2/users/me`
4. Post a test without a URL ($0.015) and delete it:

```bash
curl -s -X POST https://api.x.com/2/tweets -H "Authorization: Bearer $T" \
  -H 'Content-Type: application/json' -d '{"text":"StreamDen connection test, ignore"}'
# then: curl -s -X DELETE https://api.x.com/2/tweets/<ID> -H "Authorization: Bearer $T"
```

5. Look at the console's usage view to confirm credits dropped by the expected amount.

## Common pitfalls

- **`localhost` callback rejected.** X wants `http://127.0.0.1`.
- **Authorization code expires in 30 seconds.** Exchange immediately.
- **Access token 2 hours.** Without `offline.access` there is no refresh token.
- **Duplicate content** is rejected by X; the app should vary or skip repeated identical posts.
- **Credits at zero.** If the balance goes negative, requests block until you add credits. Show a clear message in the UI.
- **URL posts cost 13 times more** ($0.20 versus $0.015). Offer "post without link" and "post with link" as separate choices.
- **Pricing drift.** Legacy Basic plans migrated to pay-per-use after 2026-06-01 (secondary source), Pro reportedly after 2026-09-01 (unverified). Always read the live console price.
- Editing app permissions after users authorized requires them to re-authorize.

## Policy notes

- X Developer Policy: do not run multiple apps for the same use case, keep credentials private, get explicit user consent before automated replies or DMs, and do not circumvent rate limits (the official policy page was not fetchable here; read it at registration).
- **X Live chat**: the Livestream API (RTMP sources, broadcasts, chat send/moderate) is "limited to whitelisted apps" via an access form. Reading live chat is a private `broadcast.chat` event delivered to a webhook you host. Not realistic for a general local app, so out of scope.
- 2026-10-02 incentive: $20 credit on first card and a first auto-recharge match (see Cost). Check the console for the current terms.

## Sources

- Pricing: <https://docs.x.com/x-api/getting-started/pricing>
- Developer apps (types, callbacks): <https://docs.x.com/fundamentals/developer-apps>
- Getting access: <https://docs.x.com/x-api/getting-started/getting-access>
- OAuth 2.0 Authorization Code with PKCE: <https://docs.x.com/fundamentals/authentication/oauth-2-0/authorization-code>
- Media upload: <https://docs.x.com/x-api/media/introduction>
- Developer Policy: <https://docs.x.com/developer-terms/policy>
- Livestream API access: <https://docs.x.com/livestream-api/getting-started.md>
- Changelog: <https://docs.x.com/changelog>
- Console: <https://console.x.com>
