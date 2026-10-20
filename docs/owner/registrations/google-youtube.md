# Google Cloud and YouTube Data API

Last verified against official docs: 2026-10-18; connection model re-verified 2026-10-19 (DEN-244, D-059). Part of the [owner runbooks](../README.md).

## Purpose

StreamDen reads YouTube live chat (`liveChatMessages.streamList`), sends chat and moderates, lists broadcasts and, optionally, polls recent subscribers. All of that needs:

1. a **Google Cloud project** with the **YouTube Data API v3** enabled,
2. an **OAuth consent screen** (now called the **Google Auth Platform** in the console),
3. a **Desktop app OAuth client** (loopback redirect + PKCE, system browser).

Which project the end users sign in through was **Q-C2**: answered by D-031 and re-ordered by D-059 (shared project first, bring-your-own as the advanced fallback). This file has two parts:

- **Part A**: the project's own Google project (needed for the shared-project option and for your own testing).
- **Part B**: the short "end-user version" that the in-app wizard will walk users through.

## How users connect (D-059: browser login first)

- **Default:** one **Sign in with Google** button. StreamDen opens the Google consent page in the **system browser** with a loopback `http://127.0.0.1:<port>` redirect and PKCE, using the **shared StreamDen project** from Part A. Google blocks embedded webviews and custom URI schemes for this ([native-app](https://developers.google.com/identity/protocols/oauth2/native-app), re-read 2026-10-19). The device flow (step 20) is the secondary browser path for the `youtube` and `youtube.readonly` scopes only.
- **What the shared project can do before verification:** limited development and test use. Google states that "apps in development/testing/staging mode are not subject to verification" and that users "will be allowed to click through 'unverified app' warning screens", with the 100-user cap in effect until verification ([verification requirements](https://support.google.com/cloud/answer/13464323?hl=en), read 2026-10-19). Concretely: in **Testing** status each user must be on the test-user list (max 100) and tokens expire after 7 days; **In production** without verification shows the warning screen and every new user counts towards the lifetime 100-user cap ([app audience](https://support.google.com/cloud/answer/15549945), re-read 2026-10-19). Good enough for the owner's and testers' streams, not for a public release of the YouTube connector.
- **Gates for public rollout and capacity (owner actions in "Production verification" below, not open questions):** sensitive-scope **OAuth verification** (domain, privacy policy, demo video) removes the warning screen and the 100-user cap; the separate **quota audit** raises the one 10,000-unit pool that all shared-project users share. Submitting either is not the same as being granted it, and only the granted quota decides how many concurrent streams the shared project can serve (unmeasured until `streamList` re-billing is measured).
- **Fallback (Advanced):** the **bring-your-own project** wizard in Part B (the historical D-031 default). The user creates a Cloud project and a Desktop client, pastes its Client ID and secret, then finishes with the same browser consent. It is the supported path while the shared production sign-in is unavailable, and it stays available afterwards for users who want their own 10,000-unit quota.

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

Procedure/source check: **2026-10-20 UTC (DEN-257)**, public official documentation only; no provider console or OAuth flow inspected. Console labels below are documented unless marked **verify on screen**. This prepares approved N1; it does not dispatch owner work. DEN-257 must first pass independent Claude + Grok review and publication/integration, then be Done before the coordinator creates N1 with `depends_on: DEN-257`. The checklist must be available with this runbook before N1 is ready.

**Current evidence:** the owner reports Google developer registration already created. That does not prove production identity, domain ownership, published pages, connector/demo readiness, OAuth approval or quota. None of those gates is marked complete here. D-059 still prefers shared-project browser login; limited shared-project dev/test use is possible now. Until verification is **granted for the requested scopes**, BYO (Part B) is the public connector path.

### Readiness packet and non-secret record

Maintain a dated **Google N1 evidence** project note on the `streamden` board. Link it from checklist rows 9a/9b and the future N1 handoff. Keep sanitized screenshots/reports in a `context/den257-google-n1-<date>` project artifact and link that artifact from the note. Record only public identifiers, URLs, aggregate measurements, statuses and dates. Keep private contact details, DNS verification material and provider correspondence originals in the owner's password manager; publish only redacted proof. Never include secrets, tokens, authorization codes, credential JSON or customer/chat data.

| Gate / field | Value or exact procedure to establish it | Evidence/status now |
|---|---|---|
| Product / publisher | `StreamDen` / `Tech Daddy Digital` (D-015/D-009). Development app convention: `StreamDen Dev`; production branding: `StreamDen`. Do not rename an existing app speculatively. | Product known; actual console names unverified. |
| Shared production project and clients | In the project picker, select the existing registration; record Project name, **Project ID**, numeric **Project number** (dashboard), and each intended **Clients** entry's Name, Client ID and Application type. Record dev versus production purpose explicitly. | IDs/client inventory absent from the tracked runbook; not guessed. |
| Environment | Follow Google's [separate dev/test and production project guidance](https://support.google.com/cloud/answer/13464323?hl=en). Reconcile the reported registration to that inventory before changing anything; preserve the development project/testers. | Production project selection pending; no new project authorized by this prep. |
| Domain and URLs | Owner supplies an owned domain through the existing [domain runbook](distribution-and-signing.md). Agent prepares and publishes reviewed homepage/privacy/terms content through separately authorized work; record the exact resulting HTTPS URLs, never placeholders in a submission. | Domain ownership and public pages not evidenced; prerequisites pending. |
| Contacts / branding | **User support email** = monitored project support address offered by the console; **Developer contact information** = monitored project contacts. Record the selected public support address and approved logo asset, not invented mailboxes. | Addresses/logo readiness unverified. |
| Connector / demo | Connector implementation owner supplies tested release/build, feature-to-scope inventory and reproducible demo; agent checks the video against the requirements below. Roadmap intent or hand-written OAuth commands alone cannot demonstrate the app. | Working connector, build and demo not evidenced; prerequisite pending. |

These are readiness prerequisites with an agent/owner handoff, not instructions to ask for credentials or perform console operations now. Missing values stay **pending** until their source is established. Brand name approval does not prove data-access approval.

### Development limits and scope packet

The earlier Testing shorthand in this runbook applies to YouTube consent. Precisely, Google's [audience guidance](https://support.google.com/cloud/answer/15549945) allows up to 100 listed test users in **Testing**; authorizations expire seven days after consent, including an offline refresh token if issued. Requests limited to name/email/profile (`openid`, `userinfo.email`, `userinfo.profile` or equivalents) are exempt from the tester/warning/seven-day behavior; adding a YouTube scope removes that exception. This does not mean an access token lasts seven days. In **In production**, unapproved sensitive/restricted scopes trigger the unverified warning and normally a lifetime cap of 100 new users; Google can adjust quotas for particular apps. The cap cannot be reset. It ceases to apply to requests containing only approved scopes, even if still displayed. Publishing alone grants neither verification nor unlimited token lifetime: [refresh tokens can expire](https://developers.google.com/identity/protocols/oauth2#expiration) through revocation, inactivity, token limits or time-based access.

Build the submission from the implemented client's actual authorization requests, not every optional scope in Part A. Use **Google Auth Platform > Data Access > Add or remove scopes**, then **Update**; record each displayed non-sensitive/sensitive/restricted category and capture dated sanitized evidence ([Data Access guidance](https://support.google.com/cloud/answer/15549135)). The intended YouTube pair is treated as sensitive in this plan; the console is the primary classification check before submission. No restricted scope is authorized here.

| Intended scope (full URI) | Justification draft to reconcile to the working build |
|---|---|
| `https://www.googleapis.com/auth/youtube.readonly` | Read the connected broadcaster's broadcast/chat data for combined chat. Document each implemented read and data retention/deletion behavior; [liveBroadcasts.list](https://developers.google.com/youtube/v3/live/docs/liveBroadcasts/list) accepts readonly. Do not request write access for read-only mode. |
| `https://www.googleapis.com/auth/youtube` | Only when sending/moderation is implemented and enabled: [liveChatMessages.insert](https://developers.google.com/youtube/v3/live/docs/liveChatMessages/insert) and [liveChatBans.insert](https://developers.google.com/youtube/v3/live/docs/liveChatBans/insert) accept this or `youtube.force-ssl`, not readonly. Explain each write, the user's initiating control, and why readonly cannot do it. Retaining `youtube` rather than force-ssl follows the existing device-flow compatibility plan; verify minimum access against the actual methods before submission. |

Do not add `youtube.force-ssl`, channel memberships, Gmail or speculative future scopes. If implementation or console classification differs, return the packet to the connector/prep agent for reconciliation; do not expand the submission silently. For each requested scope attach feature documentation, method/scope evidence, user benefit, data use/storage/sharing/deletion explanation and why narrower access fails. Do not submit a draft justification claiming unimplemented features.

### Prepare domain, pages and demo (future N1 prerequisites)

1. A Cloud project **Owner or Editor** must prove each authorized domain in [Search Console](https://search.google.com/search-console). For a Domain property: add the owned domain without a URL scheme, choose DNS verification, copy the generated DNS record to that domain's DNS provider, then **Verify** (property controls: **verify on screen**). Use only Google's generated record; leave it in place to retain verification. Record property/domain, verified status and date, without DNS proof values. See [site ownership instructions](https://support.google.com/webmasters/answer/9008080) and [OAuth domain requirements](https://support.google.com/cloud/answer/13464321).
2. Agent checks the public homepage describes StreamDen's actual functionality and links the same privacy-policy URL used on consent. Privacy policy lives on the homepage's domain and explains Google data access/use/storage/sharing, retention, deletion, revocation and contact. In-product disclosures must match. Publish terms linking/binding users to YouTube's terms; privacy must identify YouTube API Services and link Google's privacy policy and account permission/revocation page. Reconcile actual behavior to [YouTube policies III.A/III.E](https://developers.google.com/youtube/terms/developer-policies) before recording page readiness.
3. In **Branding**, fill **App name**, **User support email**, **App logo**, **Application home page**, **Application privacy policy link**, **Application terms of service link**, **Authorized domains**, **Developer contact information** (URL field capitalization: **verify on screen**). Use only the packet's real values. Add authorized domains before URLs. Google's [branding requirements](https://support.google.com/cloud/answer/15549049) describe these fields and required production links. Open the resulting public pages signed out and record reachability and matching URLs.
4. Agent/connector author prepares an English end-to-end demo for **every OAuth client/flow in the production project**: app initiation, full consent, app name, browser address bar showing the public Client ID, then actual use of every requested scope. Use test channel data; exclude secrets/codes/tokens from the recording. Upload to YouTube Studio with **Visibility = Unlisted** and record its URL and build. [Demo requirements](https://support.google.com/cloud/answer/15549135) require matching requested consent scopes. Optional clients not ready for production must be reconciled before submission; this prep does not delete any client.

### Submit and track OAuth verification (future owner N1)

After the packet is complete and the coordinator dispatches N1:

1. Select the recorded **production** project. Under **Audience**, confirm **External** and use **Publish app** to reach **In production** when ready to submit. Keep unfinished development/test clients in their own project. [Submission guidance](https://support.google.com/cloud/answer/13461325) recommends this separation; publishing status is not approval.
2. Open **Branding**, save the complete fields as draft, click **Verify Branding**. On success, status becomes **Ready to publish**; click **Publish branding** within **seven days of that result** or it becomes **Need to re-verify**. The seven days are the window to publish branding, not a deadline to submit sensitive scopes. Branding edits are blocked during verification; do not cancel an active review without recording why.
3. With branding published, open **Google Auth Platform > Verification Center** ([direct route](https://console.developers.google.com/auth/verification)). Review **Data access** status, scope list, justifications and feature documentation (up to three links); provide the demo in **YouTube link**. Review the final summary and submit (**Submit for verification** / final confirmation wording: **verify on screen**). Record submission date, project/client IDs, exact scope set, branding status, data-access status and redacted confirmation/case reference. These controls and the branding sequence are documented in [sensitive-scope verification](https://developers.google.com/identity/protocols/oauth2/production-readiness/sensitive-scope-verification).
4. Monitor the support/developer contact mailboxes and **Verification Center** for requests or paused review. Record each status/date, requested correction, response date and outcome in the N1 note; original correspondence stays private. Treat submitted, in review, paused and rejected as distinct from **granted**. On approval record Google's exact status/decision wording and approved scopes, matched to the selected project. New unapproved scopes can restore the warning/cap ([scope changes](https://support.google.com/cloud/answer/15549135)).

Approval and review time are not guaranteed. Official estimates differ (sensitive-scope guide: 3–5 business days; [FAQ](https://support.google.com/cloud/answer/13463817?hl=en): 10 business days); track the actual case. Standard OAuth verification has no budgeted provider fee here; domain/hosting and preparation time are separate. Do not interpret this as a guarantee that every verification-related service is free: [restricted-scope review](https://developers.google.com/identity/protocols/oauth2/production-readiness/restricted-scope-verification) can require an annual independent security assessment, particularly for server access/storage/transmission. The FAQ says assessor pricing is negotiated with the assessor, not charged by Google. No assessment purchase is authorized or needed on the intended sensitive-only path; unexpected restricted classification returns to the agent for review first.

### Quota extension (separate from OAuth verification)

OAuth approval does not change API quota. Google's [audit guide](https://developers.google.com/youtube/v3/guides/quota_and_compliance_audits) requires a compliance audit for additional allocation, using the [YouTube API Services — Audit and Quota Extension Form](https://support.google.com/youtube/contact/yt_api_form). Use that same form for another extension if audited within the preceding 12 months. Submission is not a passed audit or a quota grant; no approval or response date is promised. No paid quota purchase route or submission fee is stated there; domain/hosting, evidence preparation and any separately required assessment remain separate costs.

#### Measure before requesting capacity

This is a procedure for later authorized connector tests, not a claim tests ran in DEN-257. Current capacity: **unmeasured**; current granted quota: **not observed**. The documented default is 10,000 units/day across other methods, with separate daily `search.list` and `videos.insert` buckets of 100 calls each. Read the actual project limits; do not turn that default into an approval claim.

1. Select the recorded project; open **APIs & Services > YouTube Data API v3 > Quotas & System Limits** (navigation/metric labels: **verify on screen**; [quota usage route](https://console.cloud.google.com/apis/api/youtube.googleapis.com/quotas)). Capture the daily limit, unit, usage, timestamp/time zone and reset boundary (midnight Pacific). Record all relevant buckets, including per-minute limits; no quota edit is performed by this prep.
2. Connector/test agent isolates other activity in that project or accounts for it explicitly. Record baseline counter, build/config, duration, number of chats, connection/reconnect counts, API calls by method, sends, moderation, discovery, fallback polling, retries/errors and closing counter. Wait for usage reporting to settle; record observation lag. Avoid reset boundaries or split the run into daily windows. Store only aggregate counts and sanitized screenshots in the evidence artifact.
3. Test idle/busy `streamList`, long connections and reconnects, then representative send/moderation and fallback workloads. Repeat for variability and a representative daily schedule within available quota. Reconcile measured counter deltas with the [quota table](https://developers.google.com/youtube/v3/determine_quota_cost) (streamList 5, send 20, bans 200). Do not assume an open stream is charged only once or per message; unresolved rebilling remains a measurement gate. Separate search/upload bucket consumption from the shared-unit calculation.
4. Report observed units per stream-hour and measured units per representative daily workload, including overhead. Let `Q` be the **observed granted** daily units, `R` a documented reserve for background calls/retries, `U` measured daily units per workload: `floor((Q - R) / U)` is a bounded planning calculation only when `U > 0` and `Q > R`. Record the workload duration, rates, variability and reserve rationale. Validate the proposed simultaneous workload within quota and per-minute limits before calling it measured concurrent capacity. Otherwise label it an extrapolation and leave concurrency **unmeasured**. Do not convert this into an invented number of streams.

#### Prepare and submit the audit (future owner N1)

Agent prepares a truthful packet describing only the implemented client: supported features, data flow and policies, independent user value, measured consumption, requested total daily quota and justified growth/reserve. Owner provides private legal/contact facts directly to Google. Public form fields were readable on 2026-10-20; conditional fields, final upload behavior and authenticated screens require **verification on screen**. The fetched form explicitly labels uploads simulated: do not count an attempted attachment as accepted evidence.

| Form section / fields | Packet mapping |
|---|---|
| 1 **Request Type** | Choose **Complete a compliance audit to request for additional quota** for an extension. |
| 2 **Organization and Contact Information** | Owner supplies legal identity/address, category, size and primary/technical/business contacts privately; publisher name alone is not legal proof. |
| 3 **Business Model and Google Contacts** | Describe actual creator tooling, audience, free service and any real Google contact; no invented relationship. |
| 4 **API Client Overview and Access Information** | **API Client Name** = `StreamDen`; **Primary Access URL**, **Privacy Policy URL**, terms URL = published packet URLs; accessibility/access instructions must match the build. |
| 5 **Use Cases and Quota Extension Details** | Numeric **Google Cloud Project Number**, actual methods, **Above Default quota**, **Total Per Day Quota**, **Detailed Justification** = measured request, not assumed grant. |
| 6 **Additional Evidence and Documentation Requirements** | Supply requested evidence through the working on-screen route; record accepted artifact names. |
| 7 **Attestations and Submission** | Owner reviews accurate policy/other attestations, then **Submit**. |

Do not provide developer-account passwords or OAuth tokens as demo credentials. If review access is requested, the owner arranges a dedicated test-only route directly with Google; private access material never enters the project note or runbook. If the screen differs or upload is unavailable, record the exact obstacle and use Google's case follow-up for the packet; do not assert submission succeeded.

Record confirmation/case reference, date, requested project/quota, audit state and pending questions. Track audit outcome and quota decision separately. After Google's decision, record the **granted** daily limit from both the decision and console (project number, metric, unit, effective/observed date, any expiry/conditions); reconcile discrepancies before using it for capacity. If refused or unchanged, record that explicitly. Never fill the granted field with the requested total. Recompute and validate capacity with that actual limit and link the measurement artifact.

**N1 completion evidence:** OAuth verification granted for the actual scope set; quota audit submitted with proof; actual granted daily quota recorded; capacity replaced by measured results with workload/limits stated. An audit submission alone does not establish higher quota. DEN-257 prep completion establishes the procedure only. Checklist readiness and provider approval remain separate statuses.

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
