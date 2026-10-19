# Google Cloud and YouTube Data API

Last verified against official docs: 2026-10-18. Part of the [owner runbooks](../README.md).

## Purpose

StreamDen reads YouTube live chat (`liveChatMessages.streamList`), sends chat and moderates, lists broadcasts and, optionally, polls recent subscribers. All of that needs:

1. a **Google Cloud project** with the **YouTube Data API v3** enabled,
2. an **OAuth consent screen** (now called the **Google Auth Platform** in the console),
3. a **Desktop app OAuth client** (loopback redirect + PKCE, system browser).

Which project the end users sign in through is **Q-C2** (shared project, or each user brings their own). This file has two parts:

- **Part A**: the project's own Google project (needed for the shared-project option and for your own testing).
- **Part B**: the short "end-user version" that the in-app wizard will walk users through.

## Prerequisites

- [ ] A Google account used only for the project (the project email). Turn on 2-Step Verification on it.
- [ ] Final product name (Q-B1). The name appears on the Google consent screen. Renaming after verification forces a re-verification.
- [ ] For **production verification only** (not for testing): a domain you own, a public homepage on it, a public privacy policy on the same domain (needs Q-B1 and the domain from [distribution-and-signing.md](distribution-and-signing.md)).
- [ ] Your own YouTube channel enabled for live streaming, to test (see [go-live-requirements.md](../go-live-requirements.md); the activation wait is up to 24 hours).

## Cost and limits

- **Free.** Verification is free.
- **Quota**: the default is **10,000 units per day per project** (resets at midnight Pacific Time), plus 100 `search.list` calls and 100 `videos.insert` calls per day in their own buckets. Costs we care about: `liveChatMessages.list` 5, `streamList` 5 per call (re-billing of an open stream is unverified, so measure it), `liveChatMessages.insert` 20, `liveChatBans.insert` 200, most `list` reads 1.
- More quota requires the **YouTube API Services Audit and Quota Extension Form** (below).
- **Testing** status: max 100 test users, authorizations and refresh tokens **expire after 7 days**.
- **Unverified app in production**: the "Google hasn't verified this app" screen, capped at **100 new users in total, forever**.
- Refresh tokens also expire after 6 months unused, and each Google account can hold at most 100 per OAuth client (the oldest is silently dropped).

## Part A: step-by-step (the project's project)

### 1. Create the Cloud project

1. Sign in at <https://console.cloud.google.com> with the project Google account. Accept the terms if asked.
2. Click the project picker in the top bar, then **New Project** (labels: verify on screen).
3. **Project name**: `<PRODUCT_NAME>`. The auto-generated **Project ID** underneath is permanent, so read it before you click **Create**.
4. **Organization / Location**: leave **No organization** for a personal account.
5. Click **Create** and make sure the new project is selected in the picker.

### 2. Enable the YouTube Data API v3

6. Open the menu, then **APIs & Services > Library**, search for **YouTube Data API v3**, open it and click **Enable**. Direct link: <https://console.cloud.google.com/apis/library/youtube.googleapis.com>.

### 3. Configure the Google Auth Platform (consent screen)

7. Menu > **Google Auth platform** > **Branding**. If it says "Google Auth platform not configured yet", click **Get Started**.
8. **App Information**: **App name** = `<PRODUCT_NAME>`; **User support email** = the project email. Click **Next**.
9. **Audience**: choose **External**. (Internal needs a Google Workspace organization.) Click **Next**.
10. **Contact Information**: **Email address** = the project email. Click **Next**.
11. **Finish**: tick "I agree to the Google API Services: User Data Policy", click **Continue**, then **Create**. The consent screen cannot be removed once created.
12. Open **Audience**. The **Publishing status** is **Testing**. Under **Test users** click **Add users**, enter your own Google account (the one that owns your YouTube channel) and any other tester, then **Save**.
13. Open **Data Access** > **Add or Remove Scopes** and select the scopes in the table below. Click **Save**. The page shows "Your sensitive scopes" and "Your restricted scopes" sections.
14. Later, for verification, fill in **Branding** fully: logo, **Application home page**, **Application privacy policy link**, **Authorized domains** (field names: verify on screen). Not needed while in Testing.

**Scopes to select**

| Scope | Why | Notes |
|---|---|---|
| `https://www.googleapis.com/auth/youtube.readonly` | Read chat, read broadcasts, poll recent subscribers, `superChatEvents.list` | Ask for this first ("read-only mode"). |
| `https://www.googleapis.com/auth/youtube` | Send chat messages, delete messages, bans, moderators | Ask for it only when the user enables sending or moderation. Also the one scope accepted by the device flow (together with readonly). |
| `https://www.googleapis.com/auth/youtube.force-ssl` | Not needed now | Skip it. Each extra sensitive scope needs its own justification during verification. Add only if a method we adopt later accepts only this scope. Not usable with the device flow. |
| `youtube.channel-memberships.creator` | Not needed | Requires approval by a YouTube representative. Chat-stream membership events cover our need. Do not add. |

All YouTube user-data scopes are **sensitive**, so an unverified app shows the warning screen (secondary sources say so; the Data Access page labels them, verify on screen).

### 4. Create the Desktop app OAuth client

15. Menu > **Google Auth platform** > **Clients** > **Create client**.
16. **Application type**: **Desktop app**. (Not "Web application": that type requires exact redirect URIs. Desktop clients accept any loopback `http://127.0.0.1:<port>` redirect, which is why they suit our random-port loopback.)
17. **Name**: `<PRODUCT_NAME> Desktop` (only visible in the console).
18. The form may show a checkbox **Use this client for an AI-powered agent**. It cannot be changed after creation. Official docs do not explain it beyond "if your app takes actions on behalf of users". My recommendation is to leave it **unchecked** because this client authenticates the StreamDen app, not an autonomous agent. Confirm the wording on screen before you click **Create**.
19. Click **Create**. Copy the **Client ID** and **Client secret** from the dialog (or the client's detail page). Downloading the JSON is optional (verify on screen).
20. Optional fallback client: **Create client** again with **Application type** = **TVs and Limited Input devices** for the device flow (only `youtube` and `youtube.readonly` allowed).
21. Optional API key: **APIs & Services > Credentials > Create credentials > API key**, then restrict it to the YouTube Data API v3. A community report says an API key alone is enough to stream public chats, but this is unverified. Skip until measured.

### 5. Check the quota page

22. Menu > **APIs & Services > Enabled APIs & services > YouTube Data API v3 > Quotas & System Limits** (verify on screen) shows the daily allowance and usage. Use it while testing to measure what `streamList` really costs.

## What to copy and where it goes

| Value | Secret? | Where it goes |
|---|---|---|
| Client ID | No | `SL_GOOGLE_CLIENT_ID` in `.env.local`, or the Settings > Connections > YouTube screen. |
| Client secret (Desktop app) | Google calls it "not treated as a secret" for desktop apps, but keep it out of the repo anyway | `SL_GOOGLE_CLIENT_SECRET` in `.env.local` (gitignored) or the app's secret store. If Q-C2 chooses a shared project, it ships in the binary by design. |
| Project ID, project number | No | Notes file in your password manager. |
| User tokens | Yes | Created at runtime, stored in the OS keychain. |

Names above are suggestions. The real storage location is decided in P0.

## Verification and test (no app needed)

This tests the whole loopback + PKCE path by hand. Pick any free port for the example (here 8765). **Do not paste tokens into chat or an AI agent.**

```bash
VERIFIER=$(openssl rand -base64 48 | tr -d '=+/\n' | cut -c1-64)
CHALLENGE=$(printf %s "$VERIFIER" | openssl dgst -sha256 -binary | openssl base64 -A | tr '+/' '-_' | tr -d '=')
echo "Open this URL in a browser (one line):"
echo "https://accounts.google.com/o/oauth2/v2/auth?client_id=<CLIENT_ID>&redirect_uri=http%3A%2F%2F127.0.0.1%3A8765&response_type=code&scope=https%3A%2F%2Fwww.googleapis.com%2Fauth%2Fyoutube.readonly&code_challenge=$CHALLENGE&code_challenge_method=S256"
```

1. Open the URL while signed in as a **test user**. Click **Continue** past "Google hasn't verified this app" (expected in Testing), approve the scope.
2. The browser then fails to load `http://127.0.0.1:8765/?code=...`. That is fine: copy the `code` value from the address bar.
3. Exchange it:

```bash
read -rs SECRET
curl -s https://oauth2.googleapis.com/token \
  -d code=<CODE> -d client_id=<CLIENT_ID> -d client_secret="$SECRET" \
  -d redirect_uri=http://127.0.0.1:8765 -d grant_type=authorization_code \
  -d code_verifier="$VERIFIER"
```

4. Call the API with the returned `access_token` (cost: 1 quota unit):

```bash
read -rs T; curl -s -H "Authorization: Bearer $T" 'https://www.googleapis.com/youtube/v3/channels?part=snippet&mine=true'
```

You should see your channel. A `refresh_token` is present in the token response; remember it expires in 7 days while the project is in Testing.

## The 7-day refresh-token trap (Testing mode)

- While **Publishing status** is **Testing**, every authorization and refresh token expires 7 days after consent, so you re-login weekly (`invalid_grant`).
- Fix for your own use or a user's own project: **Google Auth platform > Audience > Publish app** (button label: verify on screen) moves the project to **In production**. The 7-day rule applies to Testing only. The app stays *unverified*: you see the warning screen at consent and the 100-new-user lifetime cap applies, which is irrelevant for a personal project.
- Fix for the shared project: complete verification (below).

## Production verification (for the shared-project option, Q-C2)

Sensitive scopes need Google's OAuth app verification before more than 100 people can authorize. Typical review time is **3 to 5 business days**, longer if Trust & Safety emails you questions.

1. Own a domain and verify it in **Google Search Console** (Owner or Editor of the Cloud project). Add it under Branding > Authorized domains.
2. Publish a public **homepage** (not just for logged-in users; Play Store or Facebook pages do not count) and a **privacy policy** on the same domain that says how Google user data is accessed, used, stored and shared. Link both from the consent screen. The YouTube API Services Terms also expect your privacy policy to be consistent with Google's policy and your ToS to bind users to the YouTube Terms (check the current wording in the Developer Policies).
3. Complete **brand verification** on the Branding page and publish the branding. The result is valid for 7 days, so submit the data access request right after.
4. Under **Data Access**, declare every scope. In the **Verification Center** submit the request with a written justification per scope, including why a narrower scope will not work (readonly versus full `youtube`).
5. Record a **demo video** in English, upload it to YouTube as **Unlisted**, and link it. It must show: the complete OAuth grant flow, the consent screen showing your app name, the browser address bar containing your OAuth **client ID**, and each sensitive scope's feature in use.
6. Branding changes are blocked while verification is in progress. Keep contact emails monitored.

### Quota extension (separate from OAuth verification)

Verification does not raise quota. For more than 10,000 units per day, first complete a compliance audit through the **YouTube API Services - Audit and Quota Extension Form**: <https://support.google.com/youtube/contact/yt_api_form>. YouTube's team replies "as soon as possible" (no stated timeline). The page does not list required attachments; prepare the quota maths from [platform-apis.md section 4.1](../../research/platform-apis.md), your privacy policy and a screencast, and expect follow-up questions. If you already passed an audit in the last 12 months you reuse the same form.

## Part B: the end-user version (bring your own project)

This is the condensed version the in-app wizard (Q-C2) will present. Each user gets their own 10,000 units per day.

1. Go to <https://console.cloud.google.com>, sign in with the Google account that owns the YouTube channel, and create a project named `StreamDen`.
2. **APIs & Services > Library** > **YouTube Data API v3** > **Enable**.
3. **Google Auth platform > Branding > Get Started**: app name `StreamDen`, your email, audience **External**, your email, agree, **Create**.
4. **Google Auth platform > Data Access > Add or Remove Scopes**: tick `.../auth/youtube.readonly` (and `.../auth/youtube` if you want to send chat); **Save**.
5. **Google Auth platform > Audience**: click **Publish app** and confirm (avoids the 7-day expiry). If you stay in Testing, add your own email under **Test users** and expect to sign in again every 7 days.
6. **Google Auth platform > Clients > Create client > Desktop app** > **Create**.
7. Copy the **Client ID** and **Client secret** into StreamDen (Settings > Connections > YouTube). Click **Sign in with Google**. You will see "Google hasn't verified this app": click **Advanced**, then **Go to StreamDen (unsafe)** (wording: verify on screen). This is your own project, so it is safe.
8. Policy note for the wizard: YouTube's Developer Policies require one API project per API client and forbid sharing API credentials. Each user using their own project for themselves is fine; never ship someone else's credentials in a way that circumvents quota.

## Common pitfalls

- `access_denied` / "app not completed the Google verification process": your account is not in **Test users** (Testing) or you are past the 100-user unverified cap.
- `invalid_grant` after about a week: Testing-mode token expiry. Publish the app or re-login.
- `redirect_uri_mismatch`: you created a **Web application** client instead of **Desktop app**.
- `quotaExceeded`: the project hit 10,000 units. The counter resets at midnight Pacific Time. Polling `liveChatMessages.list` every 5 s for a 4 h stream costs about 14,400 units, so use `streamList`.
- `liveChatMessages.*` need an **active broadcast** whose `liveChatId` you get from `liveBroadcasts.list`. A channel not yet enabled for live streaming returns nothing useful.
- Embedded webviews and custom URI schemes are blocked by Google. Use the system browser and the loopback redirect.
- Changing your Google password revokes YouTube-scope refresh tokens (reported by secondary sources, verify).
- Chat deletion/retraction events are not delivered by the API, so deleted-message sync cannot be mirrored from YouTube.
- Deleting the Cloud project, or letting it be flagged inactive, kills every user's tokens at once.

## Policy notes

- YouTube API Services Terms and Developer Policies apply: no scraping or undocumented APIs (so InnerTube libraries are out of core), data refreshed or deleted within 30 days, and a privacy policy is mandatory.
- Verification re-checks apply annually for **restricted** scopes (not ours), and Google may re-review on material changes.
- The shared-project model means one failure (quota, suspension) hits every user at once.

## Sources

- Configure OAuth consent: <https://developers.google.com/workspace/guides/configure-oauth-consent>
- Create credentials (Desktop app client): <https://developers.google.com/workspace/guides/create-credentials>
- OAuth for installed apps (loopback, PKCE): <https://developers.google.com/identity/protocols/oauth2/native-app>
- OAuth 2.0 overview (7-day / 6-month expiry, 100 token limit): <https://developers.google.com/identity/protocols/oauth2>
- App audience (Testing vs In production, 100 users): <https://support.google.com/cloud/answer/15549945>
- Sensitive scope verification: <https://developers.google.com/identity/protocols/oauth2/production-readiness/sensitive-scope-verification>
- OAuth scopes list: <https://developers.google.com/identity/protocols/oauth2/scopes#youtube>
- YouTube Data API getting started and quota: <https://developers.google.com/youtube/v3/getting-started>
- Quota costs: <https://developers.google.com/youtube/v3/determine_quota_cost>
- Audit and quota extension: <https://developers.google.com/youtube/v3/guides/quota_and_compliance_audits> and <https://support.google.com/youtube/contact/yt_api_form>
- Developer Policies: <https://developers.google.com/youtube/terms/developer-policies>
- Streaming live chat (streamList): <https://developers.google.com/youtube/v3/live/streaming-live-chat>
