# Facebook Live (Meta developer app): later phase

Last verified against official docs: 2026-10-18. Part of the [owner runbooks](../README.md). **P5 or later, low priority** (Q-S1 recommendation: Rumble and Facebook in P5). Do not start this until Q-S1 confirms Facebook is in scope.

## Purpose

To read live comments and reactions (Graph API: Server-Sent Events from `streaming-graph.facebook.com/{live-video-id}/live_comments`, or polling `/comments`) and to comment as a Page, StreamDen needs a **Meta developer app**. Users authorize with **Login for Devices** (a device-code style flow using `APP_ID|CLIENT_TOKEN`) or the manual login flow. Long-lived token exchange needs the **app secret**, so device tokens (up to 60 days, no refresh) are the secret-free option.

## Prerequisites

- [ ] A Facebook account tied to the project (2FA on). Meta requires a real personal profile to own a developer account.
- [ ] Final product name (Q-B1).
- [ ] For Advanced Access (letting people other than your app's admins, developers and testers authorize): **Business Verification** (legal business documents, verify requirements on screen) plus **App Review** per permission, with screencasts and a privacy policy URL (needs the domain from [distribution-and-signing.md](distribution-and-signing.md)).
- [ ] To test your own Page live at all: the Page or professional-mode profile must meet the go-live rules in [go-live-requirements.md](../go-live-requirements.md).

## Cost and limits

- **Free** to register. Business Verification and App Review are free but take time and documents.
- **Standard Access** (automatic for Business, Consumer and Gaming apps): permissions work only for people with a **role on the app** (admins, developers, testers). That is enough for the owner's own testing, no review needed.
- **Advanced Access**: required for the general public, needs Business Verification plus App Review per permission.
- Graph API version at the time of writing: v26.0 (2026-07-29). Groups API was removed (v19, 2024).

## Step-by-step (minimal, Standard Access only)

1. Go to <https://developers.facebook.com> and click **Get Started** / log in. Complete developer registration (accept the platform terms; verify email and phone; labels: verify on screen).
2. **My Apps > Create App**. Meta organizes apps by "use cases" and app types now; choose the option that exposes **Facebook Login** (exact wording: verify on screen). Name it `<PRODUCT_NAME>` and use the project email.
3. In the app dashboard add the **Facebook Login** product, then open **Facebook Login > Settings** and set **Login from Devices** to **Yes** (documented path: Product > Facebook login > Settings > Login from Devices). If the device endpoints return an "Invalid API method" error, this setting is the cause.
4. Open **App Settings > Basic**. Copy the **App ID**. The **App Secret** is also shown there: do not use or store it unless a future relay needs it for long-lived token exchange.
5. Open **App Settings > Advanced** and copy the **Client Token**. The device-flow access token is the string `APP_ID|CLIENT_TOKEN`.
6. Add testers if needed: **App Roles > Roles** (labels: verify on screen).

## What to copy and where it goes

| Value | Secret? | Where it goes |
|---|---|---|
| App ID | No | `SL_FB_APP_ID` in `.env.local` / settings |
| Client Token | Semi-public (ships inside device-flow clients by design) | `SL_FB_CLIENT_TOKEN` in `.env.local` / settings |
| App Secret | **Yes** | Do not copy now. Only a future broker (relay) may hold it, as a Worker secret. |
| User tokens | Yes | Runtime only, OS keychain |

## Verification and test

Device-login smoke test (replace `<API_VERSION>` with the current Graph version, for example `v26.0`). **Do not paste tokens into chat or an AI agent.**

```bash
curl -s -X POST "https://graph.facebook.com/<API_VERSION>/device/login" \
  -d "access_token=<APP_ID>|<CLIENT_TOKEN>" -d "scope=public_profile"
```

The reply contains `code`, `user_code`, `verification_uri` (facebook.com/device), `expires_in` and `interval`. Enter the `user_code` at the URI, then poll `POST .../device/login_status` with `access_token` and `code` until it returns a user token. Finally `GET /me?fields=name`. Page-live scopes (for example `publish_video`, Page permissions) can only be requested by app role holders until Advanced Access is granted.

## Common pitfalls

- **"Invalid API method"**: **Login from Devices** is not enabled.
- **Permissions work for you but not for users**: you only have Standard Access.
- **Tokens do not refresh.** Device tokens last up to 60 days. Plan a re-login flow.
- **Long-lived token exchange needs the app secret** (never ship it).
- **Live eligibility** (account at least 60 days old, at least 100 followers for Pages) blocks your own tests more often than the API does.
- Meta changes app dashboards and review forms often. Expect labels to differ.

## Policy notes

- Meta Platform Terms and Developer Policies; a privacy policy URL and data deletion instructions are required for App Review.
- App Review needs a screencast per permission. Business Verification may reject personal projects.
- Facebook Live is "YELLOW/RED (bureaucracy)" in the feasibility matrix. Keep it behind Q-S1.

## Sources

- Access levels (Standard vs Advanced, App Review, Business Verification): <https://developers.facebook.com/docs/graph-api/overview/access-levels>
- Login for Devices: <https://developers.facebook.com/docs/facebook-login/for-devices>
- Manual login flow: <https://developers.facebook.com/docs/facebook-login/guides/advanced/manual-flow>
- Long-lived tokens: <https://developers.facebook.com/docs/facebook-login/guides/access-tokens/get-long-lived>
- Live Video API getting started: <https://developers.facebook.com/documentation/live-video-api/getting-started>
- Interact with viewers (comments SSE): <https://developers.facebook.com/documentation/live-video-api/interact-with-viewers>
- Graph API changelog: <https://developers.facebook.com/docs/graph-api/changelog>
