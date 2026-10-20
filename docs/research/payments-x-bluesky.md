# Payments (P2P apps), X "own account", and Bluesky: research for Streamer.live

Research date: **2026-10-18**. Scope: (1) Venmo / Cash App / X Money / PayPal / Stripe / Zelle and tip platforms as alert triggers, (2) X (Twitter) using the streamer's own account, (3) Bluesky (AT Protocol) plus Threads and Mastodon for go-live announcements.

Evidence tags: **[official]** = vendor docs, terms or source code fetched today. **[secondary]** = reputable third party (news, vendor blogs). **[measured]** = I measured it today. **(unverified)** = could not confirm. Anything in `platform-apis.md` is not repeated here unless it changes (see section 5, "Deltas").

Things I could not fetch (so claims rely on secondary sources): X Terms of Service (HTTP 402), X automation rules help page (403), Venmo's developer pages (venmo.com/docs/* now serves the business-profile marketing page), the full Cash App ToS (only ~200k of 327k characters read), Buy Me a Coffee's developer docs, the Bluesky docs search index.

---

## 0. TL;DR

| Topic | Verdict |
|---|---|
| **Venmo / Cash App (personal)** | **No official receive API or webhook for personal accounts.** Only unofficial local paths (email IMAP, Android notification forwarding) or a human "confirm tip" button. Ship as an **experimental plugin**, not core. |
| **Venmo / Cash App (official route)** | Venmo-at-checkout via **PayPal** (Payment Links / Checkout, business account, 3.49% + $0.10) and **Cash App Pay via Square** (seller account + Web Payments SDK). Both need merchant accounts. PayPal and Square webhooks need a public URL (our relay). |
| **X Money** | **Not integrable.** US-only, "select users", no developer API, webhooks, or documented creator/merchant receive feature. Watch only. |
| **PayPal (Business)** | **Core, official.** REST webhooks (relay). Polling the Transaction Search API is too slow (up to 3 h). IPN is being retired. |
| **Stripe** | **Core, official, and the best no-relay path:** `stripe listen --live` (direct connection, no public URL) or polling `GET /v1/events`. |
| **Zelle** | **No API at all.** Bank-only channels. Not supported (email plugin may catch bank emails). |
| **X "own account"** | Means: streamer's **own X developer app + OAuth 2.0 PKCE** (BYO app, BYO credits). Then reads of own followers/mentions are "owned reads" at **$0.001/resource**. Feasible triggers: follower, mention, reply, repost/like of go-live post, DM. Cost is cents per stream. Cookie/headless automation violates X's ToS and is **not recommended**. |
| **Bluesky** | Everything needed is free and local: post (rich), **Live Now status** (`app.bsky.actor.status`), notifications polling, DMs. **Jetstream filtered by the user's DID only shows the user's own actions, not inbound follows/likes/mentions**, so use `listNotifications` polling for inbound events. |

---

## 1. Part 1: Payment apps as alert triggers

### 1.1 Per-app matrix

| App | Official receive path? | Business/merchant acct needed | Fees (receiving) | KYC | Public webhook URL (relay)? | Local / unofficial path | ToS risk for streamer | Verdict |
|---|---|---|---|---|---|---|---|---|
| **Venmo personal** | **None.** Developer + Payouts APIs "retired", closed to new businesses ([TechBloat 2026-09-30, quoting Venmo](https://www.techbloat.com/?p=1965319) **[secondary]**). Legacy account webhooks documented only for pre-2016 grandfathered access | n/a | Free P2P | Venmo account | n/a | Email parse, Android notifications, manual confirm | Personal accounts "may not be used to conduct business, commercial or merchant transactions with other personal accounts"; payments for goods/services "must either be identified as for goods and services or sent to business profiles" ([Venmo US User Agreement](https://venmo.com/legal/us-user-agreement/) **[official]**). Tips are not mentioned (gray area) | **Plugin (unofficial)** |
| **Venmo business profile** | No API. Venmo is a **payment method inside PayPal/Braintree** instead | Business profile (available to sellers with an existing Venmo account; Venmo says it is "not currently accepting applications for new business accounts", a different product) ([Venmo UA](https://venmo.com/legal/us-user-agreement/), [Venmo business page](https://venmo.com/docs/payments)) | **1.9% + $0.10** direct/QR; Tap to Pay 2.9% + $0.09 | Yes | n/a | Same as above | Compliant for commercial use | Use via PayPal route below |
| **Venmo via PayPal** (Payment Links/Buttons/Checkout) | **Yes.** PayPal Payment Links accept "PayPal, Venmo, Pay Later, Apple Pay, cards", buyers can set amounts, "accept a donation or tip" ([PayPal links](https://pep.paypal.com/us/business/accept-payments/payment-links), [overview](https://developer.paypal.com/payment-links-buttons/overview) **[official]**). Venmo is US-only | PayPal **business** account | **3.49% + $0.10** (PayPal & Venmo), 2.99% + $0.10 cards | Yes | **Yes** (PayPal webhooks, up to 10 URLs per app, 25 retries over 3 days) ([PayPal webhooks](https://developer.paypal.com/api/rest/webhooks/)) | n/a | Low | **Core** (via PayPal integration). Whether Venmo-funded payments produce `PAYMENT.CAPTURE.COMPLETED` identical to PayPal ones: (unverified) |
| **Cash App ($cashtag, personal)** | **None.** Square staff: "we do not have any APIs around the Square Cash App, so webhooks of course are not possible" ([Square forum, Dec 2020](https://developer.squareup.com/forums/t/cashapp-receive-payments-webhook-support/1368) **[official, old]**; no later change found) | n/a | Free P2P | Cash App account | n/a | Email parse (user must enable email notifications in app: [Cash App help](https://cash.app/help/gb/en-us/1015-account-settings)), Android notifications, manual | P2P is "for peer-to-peer personal, non-commercial purposes"; "Your Cash App Account remains a personal consumer account and is not a business operating account"; registered businesses "must use a Cash App Business Account" ([Cash App ToS VII.1, VII.7](https://cash.app/legal/us/en-us/tos) **[official]**) | **Plugin (unofficial)** |
| **Cash App Pay (Square)** | **Yes**: Web Payments SDK token, then your backend calls `CreatePayment`. **US only**, cannot store buyer's Cash App account ([Square docs](https://developer.squareup.com/docs/payments-api/take-payments/cash-app-payments) **[official]**) | Square **seller** account + a web checkout you build and host | Not stated on Square docs; **2.6% + $0.10 widely quoted (unverified)** | Square onboarding | Square webhooks (`payment.updated`) need a public URL (the docs I read do not describe webhooks for Cash App Pay: (unverified)). Polling Payments API is possible (unverified for latency) | n/a | Low | **Optional plugin**; heavy setup for streamers |
| **Cash App "Earnings" pilot** | New (**2026-09-10**): eligible **sole proprietors** can enroll from a personal account, no fees on eligible goods-and-services payments, tag payers as customers; "currently in pilot for select eligible customers" ([Cash App press](https://cash.app/press/introducing-simpler-way-sell-goods-services-cash-app) **[official]**) | No separate business acct | $0 | Cash App | No API mentioned | Same as personal | Resolves the ToS gray area for enrolled users | Watch |
| **X Money** | **None found.** No developer API, webhook, or creator/merchant receive feature on [money.x.com](https://money.x.com/en) | n/a | P2P "free with no limits" | Yes (name, address, SSN4 during beta: [ALM Corp](https://almcorp.com/blog/x-money-beta-payments-platform/) **[secondary]**) | n/a | Unknown email/notification format (unverified) | 18+, US only, "select users" | **Not supported** (see 1.2) |
| **PayPal (personal)** | REST webhooks need a REST app. Developer login works with any account but **live receiving of commercial payments belongs on a business account** | Strongly implied | Personal P2P: free to receive "friends and family" | Yes | Yes | Email parse | Domestic friends-and-family receipt is typically free (unverified here). UA: "If the activity associated with your personal account primarily involves business or commercial activity, PayPal may close your account unless you agree to cease ... or convert" ([PayPal UA](https://www.paypal.com/us/webapps/mpp/ua/useragreement-full) **[official]**) | Plugin only; steer to Business |
| **PayPal (Business)** | **Yes.** Webhooks (below), Payment Links, PayPal.me for business. "With a business account, you can ... Sell goods or services or receive donations" (PayPal UA) | Business account (+ REST app, live credentials) | See Payment Links row; standard checkout rates (unverified) | Yes | **Yes** | Transaction Search API: **up to 3 h delay**, 3-year history, 31-day range ([PayPal docs via search](https://docs.paypal.ai/growth/reports/api) **[secondary]**) so reconciliation only | Low | **Core** |
| **PayPal IPN** | Legacy. PayPal "stopped letting merchants generate new IPN/WPS credentials at the end of 2025"; Website Payments Standard deprecated Jan 2026, EOL Jan 2027 ([DEV article](https://dev.to/flarecanary/paypal-ipn-is-deprecated-the-ipn-webhooks-migration-drops-payments-in-four-silent-ways-4jb9) **[secondary]**) | n/a | n/a | n/a | Yes | n/a | n/a | **Do not build on IPN** |
| **Stripe** | **Yes.** Payment Links ("Customers choose what to pay": best for "donations, tipping, or pay-what-you-want", default max 10,000 USD, no recurring) ([Stripe docs](https://docs.stripe.com/payment-links/api) **[official]**), events `checkout.session.completed`, `payment_intent.succeeded` | Stripe account (can be individual/sole prop) | ~2.9% + 30c US standard **(unverified from Stripe's own pricing page)** | Full Stripe verification | **No, if you use `stripe listen`** or poll events. Registered webhook endpoints "must be publicly accessible HTTPS URLs" ([webhooks](https://docs.stripe.com/webhooks)) | `stripe listen --live --forward-to localhost:PORT` ("direct connection to Stripe's API", no dashboard endpoint needed) ([CLI](https://docs.stripe.com/cli/listen)); or poll `GET /v1/events` ("going back up to 30 days", filter `types`, `created`, limit 100) ([API](https://docs.stripe.com/api/events/list)) | Tips must be "given for a good or service that has been provided" (content counts); donations need a specific charitable purpose; "Personal or peer-to-peer money transfers ... are not supported" ([Stripe support](https://support.stripe.com/questions/requirements-for-accepting-tips-or-donations)) | **Core** (best local-first official option) |
| **Zelle** | **None.** Access only through banks and processors (FIS, Fiserv "Turnkey Zelle small business") ([FIS](https://www.fisglobal.com/en/products/fis-zelle), [IBS](https://ibsintelligence.com/ibsi-news/bank-of-the-west-taps-fiserv-to-bring-zelle-to-small-businesses) **[secondary]**) | Bank | Bank-dependent | Bank | n/a | Bank alert emails/SMS (unverified formats) | Zelle is for people you trust; bank-specific | **Not supported**; generic email plugin may match bank emails |
| **Ko-fi, Buy Me a Coffee, Throne, Fourthwall** | Webhooks only (details in `platform-apis.md` section 9). Throne webhooks re-confirmed 2026-07-27: `gift_purchased`, `contribution_purchased`, `gift_crowdfunded`, Ed25519 headers, HTTPS required, 2xx within 10 s ([Throne help](https://help.throne.com/en/articles/14982436-webhook-integration) **[official]**) | Creator account | Platform-specific | Platform-specific | **Yes (relay)** | none | Low | **Core via relay** (already planned) |
| **Tipeeestream, Streamlabs, StreamElements** | Outbound sockets (see `platform-apis.md`) | Creator account | Platform-specific | Platform-specific | **No** | n/a | Low | **Core, local.** Also the easiest way to take **PayPal** tips with local alerts |

### 1.2 Notes per app

**Venmo.**
- The only official programmatic receive path for a creator is **PayPal** (Venmo shows up in PayPal Checkout/Payment Links for US merchants). Braintree's Venmo "excludes in-person sales, peer-to-peer transactions, and payments for services through the Venmo app" ([TechBloat](https://www.techbloat.com/?p=1965319) **[secondary]**), so it is not a tip rail.
- Reversals: Venmo may reverse "money or payments you previously received" and charges chargeback fees on debit/credit-funded payments to business/charity profiles ([Venmo UA](https://venmo.com/legal/us-user-agreement/)).
- Automation clause: Restricted Activities bar "robot, spider, other automatic device ... to monitor or copy our websites" and "unauthorized access to any system, data, information or Venmo services". Logging into Venmo to scrape or poll is a clear violation; reading your *own mailbox* is not touching Venmo.
- Minors: Venmo has **Teen Accounts (13-17)**; personal accounts need 18+.
- Venmo's help page lists emails that cannot be disabled ("Payment sent", statements, crypto) and does not document a "payment received" email ([Venmo help](https://help.venmo.com/hc/en-us/articles/217532247-Notification-Settings)). Whether the user receives an email on **receipt** must be tested by the streamer: (unverified).

**Cash App.**
- Age: 18+ for standard accounts; teen sponsored accounts from 13, child accounts 6-12 (ToS III.1, III.6).
- Reversals: P2P payments can be "canceled or otherwise returned" (VII.2(b)); Earn in P2P sellers can be charged back (VII.7(n)).
- Automated access/bots/scraping: no clause found in the ~200k characters read (the Acceptable Use Policy is referenced but not read): (unverified). Treat login automation as prohibited anyway.

**X Money** (live in the US only, rolling out in waves).
- Rollout to US Premium/Premium+ began 2026-07-27/28 ([TechCrunch](https://techcrunch.com/2026/07/28/elon-musks-x-money-app-is-rolling-out-in-the-u-s/)). The product page today says "Rolling out to select users in the United States", 18+, no interest in New York; coverage reported as 41 states + DC, not NY or MA ([money.x.com](https://money.x.com/en), [Crowdfund Insider](https://www.crowdfundinsider.com/2026/10/318143-x-money-offers-3-cash-back-up-to-6-00-apy/) **[secondary]**). Another review says an X subscription stopped being required for basic terms on 2026-09-23 ([Card Savvy](https://card-savvy.com/learn/x-money-review-2026) **[secondary]**).
- P2P: "pay anyone on X for free with no limits". **No request-money, handle-pay, merchant, tip-jar, API, or webhook documentation** found.
- US creator payouts (Original Content Rewards, Subscriptions) now go through X Money only; non-US stays on Stripe ([Social Media Today, 2026-09-02](https://www.socialmediatoday.com/news/x-money-will-be-the-only-payout-option-for-us-creators/829514/)).
- International expansion: none confirmed. Treat as not integrable; revisit if X publishes a developer surface.

**PayPal webhooks (for core support).**
- Useful events: `PAYMENT.CAPTURE.COMPLETED`, `...PENDING`, `...DECLINED`/`DENIED`, `...REFUNDED`, `...REVERSED`, `CHECKOUT.ORDER.APPROVED`, `INVOICING.INVOICE.PAID`, `CUSTOMER.DISPUTE.CREATED` ([event names](https://developer.paypal.com/api/rest/webhooks/event-names) **[official]**).
- Gotchas: the capture resource does **not** include the payer email (needs an order lookup); status casing is `COMPLETED`; skip-verification fallbacks let anyone forge a payment ([DEV](https://dev.to/flarecanary/paypal-ipn-is-deprecated-the-ipn-webhooks-migration-drops-payments-in-four-silent-ways-4jb9) **[secondary]**). Always verify via PayPal's verify-signature endpoint.
- PayPal.me payments to a business account: no documentation found on which webhook (if any) fires: (unverified). Recommend Payment Links (documented, API-created) instead.

**Stripe, local options (no relay).**
1. `stripe listen --live --forward-to http://127.0.0.1:PORT/hook` ships as a sidecar binary. The signing secret printed is stable across restarts. It needs `stripe login` (a dashboard-authorised CLI key). Restricted keys are possible: (unverified).
2. Poll `GET /v1/events?types[]=checkout.session.completed&created[gt]=...` with a **restricted read-only key**. Official, no sidecar, latency = poll interval (5-10 s is fine), 30-day lookback gives free catch-up after the PC was off.
3. Dashboard webhook to the relay (works, extra moving part).
- Idempotency: key on event ID; delivery order is not guaranteed; live-mode retries run up to 3 days.

> **D-059 (2026-10-19, DEN-244): connection model for the two core rails.** **Stripe:** `stripe login` is already a browser flow: the CLI shows a pairing code and opens the Dashboard, the user confirms, and credentials sit in the OS credential store with automatic refresh; `stripe login --interactive` (paste an API key) is the manual fallback, and a restricted read-only key serves option 2. Newer CLIs (after v1.50.0) need an account Administrator to enable CLI access first ([CLI login](https://docs.stripe.com/cli/login), read 2026-10-19). Stripe **Connect** OAuth is a platform product whose exchange uses the platform's secret key ([reference](https://docs.stripe.com/connect/oauth-reference)); it is **not** this integration and is not planned. **PayPal Business:** REST authentication is a `client_credentials` grant with the app's own client ID and secret ([REST auth](https://developer.paypal.com/api/rest/authentication/), read 2026-10-19), so there is no end-user consent to prefer: the user pastes their own REST app credentials (keychain) and webhooks reach the desktop through the relay. No partner or "Log in with PayPal" onboarding is assumed. The event-listener design above is unchanged; the owner runbooks for both are still to be written (proposed in the DEN-244 artifact). Payment-app sessions or scraping stay prohibited (D-046).
### 1.3 Unofficial local paths

#### A. Payment-notification emails via IMAP IDLE

How it works: the payment app emails the streamer; a Streamer.live plugin holds an IMAP `IDLE` connection (RFC 2177), fetches new mail from a sender allowlist, verifies authenticity, parses amount/payer/note, emits a `tip` event.

| Provider | Auth for IMAP | Notes |
|---|---|---|
| **Gmail** | OAuth 2.0 **XOAUTH2** with scope `https://mail.google.com/` (restricted), or **app password** (requires 2-Step Verification; revoked on password change; Workspace admins can disable) | A public app using the restricted scope "must complete a verification process" ([Gmail XOAUTH2](https://developers.google.com/workspace/gmail/imap/xoauth2-protocol)). For a user-owned Google Cloud project in **Testing**, refresh tokens expire in **7 days**; tokens also die after 6 months idle, on password change (Gmail scopes), and beyond 100 tokens per client per account ([Google OAuth](https://developers.google.com/identity/protocols/oauth2) **[official]**). Practical default: **app password**, with OAuth as an advanced option. |
| **Outlook.com / Hotmail** | **OAuth only.** Basic auth removed from personal Outlook.com (reported 2024-09-16, [Microsoft notice](https://support.microsoft.com/en-us/office/f4202ebf-89c6-4a8a-bec3-3d60cf7deaef), [Thurrott](https://thurrott.com/?p=303862) **[secondary]**) | Needs an Entra app registration (BYO): scopes `https://outlook.office.com/IMAP.AccessAsUser.All` + `offline_access`; "OAuth2 support ... is available for both Microsoft 365 ... and Outlook.com users" ([Microsoft Learn](https://learn.microsoft.com/en-us/exchange/client-developer/legacy-protocols/how-to-authenticate-an-imap-pop-smtp-application-by-using-oauth) **[official]**). Heavy for non-technical users. |
| **Fastmail** | **App password** required ("You cannot use your regular Fastmail password"); `imap.fastmail.com:993` TLS ([Fastmail](https://www.fastmail.help/hc/en-us/articles/1500000278342-Using-Fastmail-with-IMAP-or-POP-apps) **[official]**) | Easiest provider. IDLE support is standard IMAP; page does not mention it. |
| iCloud, Yahoo, Proton (Bridge) | app-specific passwords / local bridge | (unverified) |

Rust building blocks (crates.io, checked today): `async-imap` 0.12.0 (2026-09-30), `mail-parser` 0.11.9 (2026-09-09), `mailparse` 0.18.0, `mail-auth` 0.13.3 (DKIM/SPF/DMARC/ARC verification, Stalwart, 2026-09-18), `oauth2` 5.0.0. For TS plugins: `imapflow` 2.5.5, `mailparser` 3.9.46 (both published 2026-10-18).

Reliability and latency:
- IDLE delivers within seconds of the provider receiving the message; the dominant delay is the **payment app's own email send time** (unverified; likely seconds to minutes).
- IDLE sessions drop; re-issue before ~29 min (RFC 2177), reconnect with backoff, and on reconnect `UID SEARCH` for UIDs above the last seen. Dedupe on `Message-ID` plus the payment's transaction ID.
- Email templates change without notice: ship **versioned template packs** (per sender, per locale), with a self-test ("send yourself $1") and a "could not parse" inbox that never silently drops mail.
- A payer's note may be truncated or absent in emails (unverified per app).

Authenticity (must-do):
- Allowlist sender domains (`venmo.com`, `paypal.com`, `cash.app`, ...), and require **DKIM pass with `d=` aligned to the From domain**.
- Over IMAP you cannot re-run SPF (no connecting IP). Trust the mailbox provider's own `Authentication-Results` (match your provider's authserv-id, ignore any others), and use `mail-auth` only for DKIM re-verification.
- **DKIM replay**: a genuine signed email can be re-sent to a victim, and DKIM "verifies message integrity, but it does not confirm who actually delivered the email" ([Kaseya](https://www.kaseya.com/blog/dkim-replay-attacks-apple-paypal-invoice-abuse/) **[secondary]**). Mitigate by requiring the `To:` header to equal the streamer's address and a template match for *received payment*, not requests.
- **Legit-origin abuse**: attackers use PayPal's own invoice and money-request features with scam text in the note field, and the mail passes DKIM/DMARC ([Xeams](https://www.xeams.com/blocking-fake-invoices-from-paypal.htm), [Tessian](https://www.tessian.com/?p=11206) **[secondary]**). Parse only "sent you / paid you" templates, reject "requested / invoice", and treat the note as untrusted text (escape, length-cap, profanity filter).
- Even a perfect parse cannot prove funds settled (reversals, pending holds). Label these events **"reported"**, not "verified" (see 1.4).

Privacy: IMAP with `https://mail.google.com/` or a password gives the app **the whole mailbox**. Recommend a **dedicated tips mailbox/alias** (e.g. `tips@`), store only extracted fields, never persist raw bodies, log redacted. Show the permission scope in the consent screen.

ToS: reading your own mailbox does not touch Venmo/Cash App/PayPal systems, so the payment app's automation clauses are not engaged. The residual exposure is the one in 1.4 (personal accounts used commercially). Mailbox providers allow IMAP clients.

#### B. Android notification forwarding

- Mechanism: an Android app with Notification Access reads the payment app's push (package filter) and forwards title/text to Streamer.live: **KDE Connect** (notification sync; desktop 26.04.0 on 2026-04-10, Android 1.35.x with "Fixed notification sync" on F-Droid 2026-06; Windows since 2021, macOS only unstable nightlies) ([Wikipedia](https://en.wikipedia.org/wiki/KDE_Connect), [F-Droid](https://f-droid.org/packages/org.kde.kdeconnect_tp/) **[secondary]**), or **ntfy / Tasker / MacroDroid** posting to a local HTTP endpoint (specific apps: (unverified)).
- Reliability: Android battery optimisation can kill forwarders; payment apps may hide amounts on the lock screen; text is truncated; the iPhone has no equivalent. Latency is typically 1-3 s (unverified).
- Privacy: the forwarder sees **every** notification. Filter on package IDs on the phone, and forward only matched fields.
- ToS: does not touch the payment service (low); the burden is on the phone-side app's Play policy.
- Integration: expose an authenticated `POST /api/ingest/notification` on the local server (LAN bind is opt-in), or subscribe to a self-hosted/ntfy.sh topic.

#### C. Do not build

Logging into Venmo/Cash App/PayPal web with stored credentials or cookies (headless browser or reverse-engineered mobile API). It breaks the robot clauses quoted above, risks account closure and fund freezes (the account that holds the streamer's money), and breaks constantly. The Venmo "personal access token" trick in old forum posts used the retired developer tab and is not a supported path.

#### D. Human-in-the-loop fallback (recommended for P2P)

A dock button / hotkey "Fire tip alert" (amount, display name, note prefilled from a notification or email preview) triggers the same alert pipeline. The streamer or a moderator confirms after seeing the app notification. No ToS risk, no parsing risk, and it works for Zelle, X Money, Cash App, Venmo and anything else.

### 1.4 Safety and compliance

| Concern | What the sources say | Design response |
|---|---|---|
| **Reversals / chargebacks** | PayPal emits `PAYMENT.CAPTURE.REVERSED/REFUNDED/DECLINED` and `CUSTOMER.DISPUTE.CREATED` (official list). Stripe sends refund/dispute events (standard) and retries up to 3 days. Venmo and Cash App reserve the right to reverse received payments (UAs above). P2P emails have no machine-readable reversal signal. | Normalized `tip.state`: `pending, completed, reversed, disputed`. Reversal updates the **ledger** (alert already played). Optional **alert hold** for high amounts (e.g. above $X wait for `completed`). Gate expensive actions (long TTS, giveaways) on trust tier. |
| **Fake emails / spoofing** | DKIM replay and legit-origin invoice abuse (section 1.3A). | Trust tiers below; strict sender/DKIM/To checks; template match for *received*; never auto-fire high-impact actions from Tier 2/3. |
| **Payer privacy** | Venmo/Cash App/PayPal expose real names, handles, and free-text notes. | "Stream-safe" default: show handle or first name only; notes shown only after profanity/PII filter or moderator approve; per-source toggle for full names; do not retain raw email. |
| **Minors** | Venmo Teen 13-17; Cash App teen 13+, child 6-12; X Money 18+. | Cannot verify payer age. Keep the displayed identity minimal, do not collect extra payer data, and give a docs note on local rules (COPPA/GDPR). (Not legal advice.) |
| **Commercial use of personal accounts** | Venmo, Cash App, PayPal personal terms (section 1.1). Stripe wants tips tied to goods/services provided, forbids P2P. | Disclaimers in the plugin consent screen; link to each vendor's business option. Recommend Stripe or PayPal Business for regular income. |
| **Tax** | Cash App pilot mentions Form 1099-K only above $20,000 and 200 transactions ([Cash App press](https://cash.app/press/introducing-simpler-way-sell-goods-services-cash-app)). | Out of scope for the app; do not give tax guidance. |

### 1.5 Recommendation

**Normalize everything to one `tip` event** with: `source`, `amount`, `currency`, `payer_display`, `note`, `txn_id`, `state`, `received_at`, and **`trust`**:
- **Tier 1 "Verified"**: official webhook/API with signature check (Stripe, PayPal, Throne, Ko-fi, Fourthwall, Streamlabs/StreamElements/Tipeeestream sockets).
- **Tier 2 "Reported"**: DKIM-aligned email parsed from an allowlisted sender (Venmo, Cash App, PayPal personal, bank/Zelle).
- **Tier 3 "Unverified"**: phone notification forwarding or manual button.

| Support level | Items |
|---|---|
| **Core (official)** | Stripe (Payment Links + `stripe listen` sidecar or events polling), PayPal Business (webhooks via relay, Payment Links incl. Venmo), existing aggregator/tip platforms |
| **Plugin (experimental, off by default)** | "Payment email alerts" (IMAP IDLE) with Venmo, Cash App, PayPal, generic bank/Zelle template packs; "Phone notification alerts"; "Manual tip" dock |
| **Not supported** | X Money (no API), Zelle API (does not exist), any login or cookie scraping of payment apps |

UX/disclaimers (consent screen text, shown once and in plugin settings):
1. "This reads emails/notifications; **it cannot confirm money arrived or stayed**. Payments can be reversed."
2. "Venmo, Cash App and PayPal personal accounts are intended for personal payments; using them to collect viewer payments may violate their terms. A business option is linked."
3. "Use a dedicated mailbox. The app will request access to the whole mailbox (Gmail/Outlook limitation)."
4. "Payer names and notes are untrusted and shown only after filtering."
5. Self-test wizard (send yourself $1) and a "unparsed messages" list.

---

## 2. Part 2: X (Twitter) with the streamer's own account

### 2.1 What "uses the streamer's own account" can mean

| Option | What it is | Data reachable | Cost | Policy | Verdict |
|---|---|---|---|---|---|
| **(a1) BYO developer app + OAuth 2.0 PKCE** | Streamer creates their own app in the X Developer Console, buys credits, our app signs them in with PKCE (loopback `http://127.0.0.1`, Native App public client) | Own posts, mentions, followers/following, likes, DMs (scope `dm.read`), broadcasts (scope `broadcast.read`, whitelisted), Activity events | Pay-per-use; reads of **own** data at **$0.001/resource** because the user owns the app | Official | **Recommended** |
| (a2) Shared Streamer.live app + users OAuth in | One app owned by us | Same endpoints | **We would pay** per resource for every user, and reads are not "owned reads" ($0.005-$0.010) | Official; but the Developer Policy reportedly forbids "multiple apps for the same use case" (as recorded in `platform-apis.md` section 7.2) and app-level limits are shared | Not viable |
| **(b) Session cookies / headless browser** | Reuse the logged-in web session to call internal GraphQL or drive a browser | Everything the web UI shows, including X Live/Live Studio analytics | $0 | X ToS (paraphrased) prohibits accessing the Services by automated means other than X's published interfaces, and states that "crawling or scraping the Services in any form, for any purpose without our prior written consent is expressly prohibited" ([ConductAtlas excerpt of X ToS, captured 2026-10-10](https://conductatlas.com/platform/x/x-terms-of-service/acceptable-use-and-scraping-prohibition/) **[secondary]**; the ToS page itself returned HTTP 402). The automation rules say X "may take action on your account, including filtering your posts from search results or suspending your account" (excerpt via search; page returned 403). Also the liquidated-damages clause (US$15,000 per 1,000,000 posts accessed in 24 h; [Knight Institute](https://knightcolumbia.org/content/knight-institute-says-xs-new-terms-of-service-will-stifle-independent-research) **[secondary]**) shows how X treats scraping | **Do not ship** |
| (c) Third-party "X data APIs" | twitterapi.io (~$0.15/1k tweets), SocialData (~$0.20/1k), others ([comparison](https://www.getxapi.com/blogs/cheapest-twitter-api-2026-8-providers-ranked-by-real-per-1000-tweet-cost) **[secondary]**) | Public data only, mostly read-only; **no private data such as DMs**; some accept login cookies (unverified) | Cheap per tweet | They are themselves scraping X (ToS-gray) and cookies would go to a third party | Not recommended |
| (d) Zero-API, free | **Web Intent link** `https://x.com/intent/tweet?text=...&url=...` opens the composer; the streamer taps Post ([X docs](https://docs.x.com/x-for-websites/post-button/guides/web-intent)). Also parse X notification emails with the section 1.3A pipeline (unverified formats) | Post only (intent); notifications via email | $0 | Official / mailbox only | Good fallback and the cheapest go-live post |

### 2.2 Pay-per-use prices that matter (official, fetched today)

Source: [X API pricing](https://docs.x.com/x-api/getting-started/pricing), [changelog](https://docs.x.com/changelog). Only pay-per-use (+ Enterprise) appears on the pricing page. Spend limits can be set per billing cycle. Cap: 3M post reads/month. Billable resources are **de-duplicated within a 24-hour UTC window** ("soft guarantee").

| Item | Price |
|---|---|
| Post: create | **$0.015** |
| Post: create **with URL** | **$0.200** |
| Post: create (summoned reply) | $0.010 |
| Post: read | $0.005 |
| User: read | $0.010 |
| DM Event: read | $0.010 |
| Following/Followers: read | $0.010 |
| Like: read | $0.001 |
| Space: read | $0.005 |
| DM interaction: create | $0.015 |
| **Owned reads** (`/users/{id}/` `tweets`, `mentions`, `liked_tweets`, `bookmarks`, `followers`, `following`, `blocking`, `muting`, lists) when `{id}` is the authenticated user **and that user owns the developer app** | **$0.001 per resource** |
| Activity events: `post.create` $0.005, `follow.follow`/`unfollow` $0.010, `dm.received` $0.010, `chat.received` $0.010, `spaces.start/end` $0.005, mute/block $0.001, profile updates $0.005; `post.delete`, `dm.sent/read`, `chat.sent` not billed | per event |
| Incentive (2026-10-02) | $20 for saving a first card, up to $50 matching first auto-recharge (expire after 3 months) |

Not on the pricing page: price of `like.create`, `post.mention.create`, `post.repost.create` events (docs say post-type events are billed "as a Post", i.e. $0.005, from [Activity intro](https://docs.x.com/x-api/activity/introduction)), and broadcast events (unverified).

Related changes (changelog): 2026-04-16/20 Owned Reads cut to $0.001, URL posts $0.20; "Following, Likes, and Quote-Posts via the API" removed from self-serve tiers (write side; whether it also affects reading *who liked* a post is ambiguous: (unverified)); 2026-02-23 programmatic replies allowed only if the author "summoned" the replier by mention/quote.

### 2.3 Which streamer triggers are feasible

OAuth scopes available to an own-account app: `tweet.read users.read follows.read like.read dm.read dm.write offline.access broadcast.read broadcast.write space.read mute.read block.read tweet.write media.write` ([scopes](https://docs.x.com/fundamentals/authentication/oauth-2-0/authorization-code)). Access tokens last 2 h; `offline.access` for refresh.

| Trigger | Polling route (no public URL) | Push route (Activity API) | Notes |
|---|---|---|---|
| **New follower** | `GET /2/users/:id/followers?max_results=1..1000` (owned read $0.001/resource; per-user limit 300/15 min). The spec does not state ordering: assume newest-first but **verify** (unverified) | `follow.follow` $0.010/event | With `max_results=1..10` and 24h de-dup, idle polling costs ~0 (table below) |
| **Mention** | `GET /2/users/:id/mentions?since_id=` (owned read; 300/15 min per user). Empty polls return nothing so cost nothing | `post.mention.create` (private, billed as Post, $0.005) | Replies to your posts also @mention you, so mentions usually include replies (likely; unverified) |
| **Reply to go-live post** | via mentions, or search by `conversation_id` ($0.005/post read) | `post.reply.create` (needs `post_id` of your post) | Auto-replying back is only allowed to "summoning" authors since 2026-02-23 |
| **Repost of go-live post** | `GET /2/tweets/:id/retweeted_by` (75/15 min) | `post.repost.create` | Price class of `retweeted_by` ($0.010 User read vs $0.001) unverified |
| **Like of go-live post** | `GET /2/tweets/:id/liking_users` (75/15 min) | `like.create` (direction `inbound`) | Same price/availability ambiguity after the 2026-04-20 change: test on a live account |
| **Quote** | Search or timeline | `post.quote.create` | |
| **DM received** | `GET /2/dm_events` (limit **15/15 min**, $0.010 per event read), scope `dm.read` | `dm.received` $0.010; encrypted chats appear as `chat.*` events (content readability unverified) | Developer Policy: DM content storage needs consent of the people involved; get explicit consent before automated DMs/replies |
| **Go-live post** | `POST /2/tweets` ($0.015, or $0.20 with a URL). Use the **Web Intent link** to avoid the $0.20 | n/a | Streamer pays |
| **X Space started/ended** | `GET /2/spaces` ($0.005 per Space) | `spaces.start`, `spaces.end` ($0.005) | |
| **X Live started/ended** | n/a | `broadcast.start`/`broadcast.end` are documented as **public events** usable with app-only or user-context auth ([Activity intro](https://docs.x.com/x-api/activity/introduction)). Whether they need Livestream-API whitelisting: (unverified) | OBS already tells us when we start streaming |

**Activity API transport.** Two delivery paths: webhook (public URL, our relay) and a persistent HTTP stream `GET /2/activity/stream` (2 connections per app, 450 connects/15 min, `backfill_minutes` 0-5) ([stream ref](https://docs.x.com/x-api/stream/activity-stream), [rate limits](https://docs.x.com/x-api/fundamentals/rate-limits)). The quickstart shows the stream with an **app-only bearer token** for *public* events; for *private* events (likes, mentions, reposts, DMs, broadcast chat) the docs require user-context OAuth 2.0 and a subscription for a user who authorized your app, **but never say whether private events flow over the stream with an app token or a user token** ([quickstart](https://docs.x.com/x-api/activity/quickstart.md)). Spike this before promising "no public URL" push for private events. Self-serve subscription cap: 1,500.

### 2.4 Polling cost per hour (own account, owned reads)

Assumptions: one shared poll loop, `max_results` small, de-dup works (soft guarantee). "Worst case" = no de-dup, every poll returns the full page.

| Poll | Interval | Polls/h | Worst-case $/h | Typical $/h (de-dup + idle) |
|---|---|---|---|---|
| Followers, `max_results=1` (detect change, then page deeper if the newest ID changed) | 30 s | 120 | $0.12 | ~$0.00 + $0.001 per new follower |
| Followers, `max_results=1` | 60 s | 60 | $0.06 | same |
| Followers, `max_results=10` | 60 s | 60 | $0.60 | ~$0.00 + $0.001 per new follower |
| Followers, `max_results=10` | 5 min | 12 | $0.12 | same |
| Mentions with `since_id` | 15-60 s | 60-240 | n/a | **$0.001 x new mentions** |
| DMs (`dm_events`) | 60 s (limit 15 per 15 min) | 60 | $0.01 per event returned | $0.01 x new DMs |
| Likers/reposters of the go-live post | 30-60 s | 60-120 | $0.01 per user returned | **$0.01 x distinct users that day** (about $0.40 for 40) |
| Go-live post with link | per stream | 1 | $0.20 | **$0.20** (or $0 with Web Intent) |

Rough monthly budget for a daily streamer: go-live post $0.20 x 30 = **$6** (or $0 with intent link), plus pennies for polling. Compare Activity push: 5 new followers (5 x $0.01) + 20 mentions (20 x $0.005) = $0.15 per stream. Cost is not the differentiator; latency and the public-URL requirement are.

Make **polling opt-in per trigger** with a visible cost estimator and a default spend-limit reminder ("set a monthly limit in the X Developer Console").

### 2.5 Going live on X, and X Live chat/viewer events

- **X Live Studio** (July 2026) replaced Media Studio Producer: inside Creator Studio, requires X **Premium** (from $3/month), supports any RTMP encoder (OBS, Streamlabs, vMix, Wirecast), reusable RTMP sources, scheduling up to a year, private test streams, 24 h cap per stream, and real-time analytics (concurrent viewers, chat participation, countries) in the **UI only**. It is a **beta limited to selected US states and some international cities**; protected accounts cannot go live ([AlternativeTo](https://alternativeto.net/news/2026/7/x-launches-live-studio-a-new-live-streaming-tool-to-rival-youtube-and-twitch/), [streamer.guide](https://streamer.guide/blog/x-live-studio-update-july-2026) **[secondary]**; no official X help page retrieved).
- **API**: the **Livestream API** (create broadcast, RTMP source, chat send/moderate) is "limited to whitelisted apps" via an access form ([official](https://docs.x.com/livestream-api/getting-started.md)). Reading live chat = `broadcast.chat` private Activity event "delivered over a webhook you host" ([official](https://docs.x.com/livestream-api/chat/accessing-live-chat.md)); `broadcast.chat.remove` added 2026-09-29. Payload fields include `message`, `reply_to`, `is_subscriber`, `is_moderator`, `author` ([event payloads](https://docs.x.com/x-api/activity/event-payloads.md)).
- **No viewer-count or viewer-join API** appears anywhere in the docs I read; the only documented viewer data is in Live Studio's UI.
- Conclusion (unchanged from `platform-apis.md`): X Live chat/viewer events are **not realistically available** to a general local app. Offer only: (1) go-live post (intent link or API), (2) `broadcast.start/end` if it proves not to need whitelisting, (3) optional chat via a webhook behind the relay if the owner obtains whitelisting.

### 2.6 Recommendation for X

1. Implement X as **action target (go-live post) + optional own-account triggers** using **BYO developer app, PKCE loopback, BYO credits**; document costs plainly.
2. Default go-live post = **Web Intent** (free); API post optional ($0.015 / $0.20).
3. Triggers: mentions (since_id poll), followers (small page), DMs only if the owner opts in. Activity API stream as a later spike.
4. Rust: no official crate. `twapi-v2` 0.27.0 (2026-06-15, 3.0M downloads, 346k recent) is far more used than `xdk-rs`; evaluate it, or use a thin `reqwest` client (`platform-apis.md` lists only `xdk-rs`, `twitter-v2`, `egg-mode`).
5. Never ship cookie/headless automation; do not ship credentials of a shared app.

---

## 3. Part 3: Bluesky (AT Protocol)

### 3.1 Auth: app passwords vs OAuth

| | App password | OAuth (atproto profile) |
|---|---|---|
| Official stance | "Single-purpose applications such as bots or command line tools may use password authentication"; "Applications with their own end-user login flow should implement OAuth" ([Bluesky OAuth page](https://bsky.network/docs/oauth-client), [atproto auth guide](https://atproto.com/guides/auth)). Earlier official guidance: no plans to deprecate app passwords (via search snippet, undated) | Default for apps with user login |
| Desktop fit | Long-lived until revoked; DM access is a **checkbox on the app password** ("Bad token scope" if missing: [atproto docs](https://atp.readthedocs.io/en/latest/dm.html)) | Native app = **public client**: `client_id` `http://localhost` (no port, path `/`), redirect `http://127.0.0.1/...`; **PKCE (S256), PAR, DPoP all mandatory**; access tokens < 30 min; **public clients: session and refresh token lifetime each limited to 2 weeks**, i.e. re-login about every 14 days ([spec](https://atproto.com/specs/oauth) **[official]**) |
| Scopes | All-or-nothing (+DM flag) | `atproto` + `transition:generic` (broad), `transition:chat.bsky` (DMs, depends on generic); granular `repo:`, `rpc:`, `blob:` ([permissions spec](https://atproto.com/specs/permission)). Granular scopes for our needs would be `repo:app.bsky.feed.post?action=create`, `repo:app.bsky.actor.status`, `blob:image/*`, plus `rpc:` entries with `aud`: exact strings (unverified) and host support varies |
| Rate cost | `createSession` limited to **30/5 min and 300/day** per account: always reuse/refresh the session | Refresh handled by DPoP flow |

**Recommendation:** implement **OAuth loopback (scoped, `transition:generic` initially)** as the standard option and **app password** as an "unattended / long-running" option, since the 2-week public-client cap is awkward for a streaming PC that is never opened for weeks. A confidential client (180-day refresh tokens, unlimited session) would need an HTTPS-hosted client metadata and a private key on a server, which conflicts with local-first (the self-hosted relay could do it; open question for the owner).

> **Owner decision 2026-10-19 (D-059, DEN-244):** confirmed in this order: atproto OAuth in the browser is the default, the app password is the advanced "unattended" fallback. Re-read 2026-10-19: the [atproto OAuth spec](https://atproto.com/specs/oauth) still caps public-client sessions and refresh tokens at 2 weeks and makes PKCE, PAR and DPoP mandatory, so the UI must expect a fortnightly re-login on OAuth.

### 3.2 Posting and rich posts

All **[official]** ([Creating a post](https://bsky.network/docs/bluesky-api/creating-a-post), [Posts in depth](https://bsky.network/docs/about-bluesky-content/posts), lexicons in `bluesky-social/atproto`):
- `app.bsky.feed.post`: `text` (max **300 graphemes**, 3000 bytes), `createdAt`, `langs` (up to 3), `facets`, `reply`, `embed`, tags.
- **Facets** use **UTF-8 byte offsets**: `#mention` (needs DID, resolve handle), `#link` (uri), `#tag`. The TS SDK `RichText.detectFacets` does it; in Rust you must compute byte ranges yourself.
- **Link card** = `app.bsky.embed.external` (`uri`, `title`, `description`, optional `thumb` blob, **thumb max 1,000,000 bytes**); the app must fetch OpenGraph tags itself and `uploadBlob` the thumbnail.
- **Images** = `app.bsky.embed.images`: up to **4**, each blob max **2,000,000 bytes**, alt text and `aspectRatio`.
- **Video** = `app.bsky.embed.video`: mp4, lexicon max 300,000,000 bytes; recommended path is uploading to `video.bsky.app` with a service-auth token (30 min expiry), then polling job status; Bluesky-hosted accounts need a **verified email** and have daily video-post limits ([video docs](https://bsky.network/docs/about-bluesky-content/video)).
- Reply/quote use strong refs (uri + cid).
- PDS blob upload limit 50 MB.

### 3.3 Live status ("LIVE" badge on the avatar)

Mechanism **[official: lexicon + official client source]**:
- Record: collection `app.bsky.actor.status`, **rkey `self`**, fields `status` = `app.bsky.actor.status#live`, `embed` (`app.bsky.embed.external` with the live URL), `durationMinutes` (>= 1; "Applications can choose to impose minimum and maximum limits"), `createdAt` ([lexicon](https://github.com/bluesky-social/atproto/blob/main/packages/api/lexicons/app/bsky/actor/status.json)). `statusView` exposes `expiresAt`, `isActive`, and **`isDisabled`** ("the user's go-live access has been disabled by a moderator").
- The official client upserts with `com.atproto.repo.putRecord` (`swapRecord` = existing cid or null, retry on `InvalidSwap`), and removes it with a delete ([source: social-app `liveNow/index.tsx`](https://github.com/bluesky-social/social-app/blob/main/src/features/liveNow/index.tsx)). Its dialog offers durations of **5-minute steps up to 4 hours**, and uploads a thumbnail it extracts from the page's OpenGraph data.
- The client shows the badge only if the embed's host is allowed. **Default allowlist in the app (2026-10-18)**: `twitch.tv`, `stream.place`, `bluecast.app`, **`youtube.com`**, `substack.com`, `beehiiv.com`, **plus** server-side additions and per-account exceptions served from an app-config endpoint. The public `app.bsky.unspecced.getConfig` currently lists about 20 exception DIDs (stream.place/skylight.social, youtube.com, twitch.tv, and NBA/ESPN domains) **[measured]**.
- **Kick, Trovo, Rumble, X and Facebook are not allowlisted** (Kick has no entry anywhere I could find). TechCrunch in Jan 2026 said the feature "only supports Twitch" ([TechCrunch](https://techcrunch.com/2026/01/16/bluesky-rolls-out-cashtags-and-live-badges-amid-a-boost-in-app-installs/)); the source today shows it has expanded. The UI still labels it "beta" ("not supported while the Live feature is in beta").
- Whether third-party Bluesky clients and the appview enforce the same allowlist: (unverified).

Implementation sketch: on stream-start, `putRecord` with `durationMinutes` <= 240, `embed.external` = `https://twitch.tv/<user>` (title/description from the stream), optional thumb blob; **heartbeat** re-`putRecord` every ~3 h while still live (UPDATE costs 2 write points); delete on stream end. Scope needed: `transition:generic` or `repo:app.bsky.actor.status` (+ `blob:` for the thumb).

### 3.4 Real-time triggers: what Jetstream can and cannot do

**Key correction to the brief:** Jetstream's `dids` filter returns only records **authored by** those DIDs. A filter on the streamer's DID therefore yields the streamer's **own** posts, likes, follows, reposts, not the follows/likes/replies/mentions **other people** direct at them. Inbound events are stored in the *other* user's repo.

| Route | Gives | Cost / limits | Verdict |
|---|---|---|---|
| **`app.bsky.notification.listNotifications`** (authenticated, via the user's PDS to the appview) | Inbound `like`, `repost`, `follow`, `mention`, `reply`, `quote`, `starterpack-joined`, `verified`, `like-via-repost`, `repost-via-repost`, `subscribed-post`, `contact-match`; `reasons[]` filter; `limit` <= 100; `cursor`; `record`, `author`, `reasonSubject`, `indexedAt` ([lexicon](https://github.com/bluesky-social/atproto/blob/main/packages/api/lexicons/app/bsky/notification/listNotifications.json)) | Free; no public URL; per-account appview limits for authenticated calls are not published (the rate-limits page calls the direct Bluesky API limits "generous", but that paragraph is about the unauthenticated hosts: (unverified) for this path; [rate limits](https://bsky.network/docs/rate-limits)). Poll every 5-15 s; track a **local high-water mark** instead of calling `updateSeen` (which would clear the user's unread badge in the app) | **Primary route for inbound events** |
| **Jetstream, collection-wide** (`collections=app.bsky.feed.like&...follow...repost`) + client-side filter on `subject`/`reply`/facets | Same events with push latency | **[measured 2026-10-18, 10-15 s samples]**: likes 252 ev/s (~134 KB/s), follows 43 ev/s (~17 KB/s), posts+likes+reposts+follows 362 ev/s (~199 KB/s, about 17 GB/day uncompressed). Legacy README: zstd cuts message size ~56% ([jetstream-legacy](https://github.com/bluesky-social/jetstream-legacy)) | Overkill for one streamer; make optional |
| **Jetstream, `dids=[streamer]`** | Own posts, own likes/follows: useful to detect that the streamer posted/changed something outside our app (e.g. manual go-live post) | A few events/min | Optional nicety |
| `chat.bsky.convo.getLog` (poll) | DMs and conversation events | see 3.5 | DMs |

Jetstream facts **[official]** ([docs](https://bsky.network/docs/jetstream), [webhooks guide](https://bsky.network/docs/jetstream-webhooks)): the project has been rewritten ("v2", archive/replay) with endpoints `wss://jetstream.us-west.bsky.network` and `wss://jetstream.us-east.bsky.network` at path `/xrpc/network.bsky.jetstream.subscribeEvents` and WebSocket subprotocol `xrpc.v1.json`; messages are `{ "$type": "message", "payload": {... "seq", "did", "operation", "collection", "rkey", "record" ...}}`. Legacy v1 hosts (`jetstream1.us-west...`, `jetstream2.us-east...`, path `/subscribe`, params `wantedCollections`/`wantedDids`) still run. Filters: up to **100 collections** and **10,000 DIDs**; `kinds` = commit, identity, account, sync; `cursor` is inclusive with **at-least-once** delivery, so dedupe on the `at://` URI. No authentication needed.

### 3.5 DMs (`chat.bsky.*`)

- Methods in the lexicon today include `listConvos`, `getConvo`, `getMessages`, `getLog` (poll for `logCreateMessage` and other events), `sendMessage`, `sendMessageBatch`, `acceptConvo`, `updateRead`, `muteConvo`, reactions, plus new group-chat events (add/remove member, join links) ([lexicons](https://github.com/bluesky-social/atproto/tree/main/packages/api/lexicons/chat/bsky/convo)).
- Routing: calls go to the user's PDS with header `atproto-proxy: did:web:api.bsky.chat#bsky_chat` ("Bluesky DMs ... always require proxying") ([request proxying](https://bsky.network/docs/bluesky-api/request-proxying), [API directory](https://bsky.network/docs/api-directory)).
- Auth: app password **with DM access enabled**, or OAuth `transition:chat.bsky`; granular `rpc:` syntax for chat requires a specific method and `aud=did:web:api.bsky.chat#bsky_chat`: exact working strings (unverified).
- No push/webhook; poll `getLog` (cursor-based) every 5-15 s.
- Anti-spam: [Developer Guidelines](https://bsky.network/docs/developer-guidelines) define spam as "automated or bulk interactions, including any that would cause a notification ... like a message, follow, like or reply". **Do not auto-DM or auto-reply to every new follower**; keep auto-replies to explicit user-initiated mentions/commands, rate-limited and opt-in.

### 3.6 Rate limits ([official](https://bsky.network/docs/rate-limits))

| Limit | Value |
|---|---|
| Record writes per account | **5,000 points/h and 35,000/day**; create = 3, update = 2, delete = 1 (so 1,666 creates/h) |
| Per-IP API requests (PDS) | 3,000 per 5 min |
| `createSession` | 30 per 5 min and 300 per day per account |
| `updateHandle` | 10 per 5 min, 50/day |
| Blob upload | 50 MB (PDS) |
| `public.api.bsky.app` / `api.bsky.app` | unauthenticated, cached, "generous"; contact Bluesky if limited |
| Chat limits | not published on that page |

A streamer's workload (a few posts, status heartbeats, polling reads) is far below these.

### 3.7 Libraries (checked 2026-10-18)

| Lang | Package | Version / date | Notes |
|---|---|---|---|
| Rust | `atrium-api` | 0.25.8 (2026-03-26) | Generated; contains `app::bsky::actor::status` and `chat::bsky::convo` modules (docs.rs checked). Repo last pushed 2026-08-14 (codegen fixes for permission sets); 417 stars |
| Rust | `atrium-xrpc` 0.12.4 (2025-12-27), `atrium-oauth` 0.1.7 (2026-03-26), `bsky-sdk` 0.1.24 (2026-03-26) | | `atrium-oauth` README shows `AtprotoLocalhostClientMetadata` with `http://127.0.0.1/callback` and scopes `atproto` + `transition:generic` |
| Rust | **`jacquard`** 0.12.2 (2026-09-05; with `jacquard-oauth`, `-api`, `-identity`) | | "Simple and powerful AT Protocol client library for Rust"; 121k downloads; actively released. Loopback OAuth support: (unverified) |
| Rust | `atproto-client/-oauth/-jetstream` 0.14.5 (2026-04-02) | | Independent suite |
| Rust | `rsky-lexicon` 0.12.1 (2026-10-14) | | Blacksky; lexicon types only |
| Rust | `jetstream-oxide` 0.1.2 (**2025-03-24**) | | **Stale, v1 only**. Write a thin client with `tokio-tungstenite` 0.30 + `serde_json` (the v2 protocol is small) |
| TS | `@atproto/api` 0.28.0 (2026-10-17, **not deprecated**) | | Docs now show **`@bsky/sdk` 1.3.0** + **`@atproto/lex` 0.4.0** (+ `@atproto/oauth-client-node` 0.5.10, `@bsky/jetstream` 1.0.0 published 2026-08) |

Recommendation: keep AT Protocol in the Rust core (`atrium-api` + `atrium-oauth` is the safest bet today, `jacquard` as a strong alternative), expose actions/triggers to JS/Python scripts through the app's API rather than shipping an SDK to user scripts.

### 3.8 Threads and Mastodon (go-live announcements only)

| | Threads | Mastodon |
|---|---|---|
| Auth | Meta developer app; scopes `threads_basic` (all endpoints), `threads_content_publish`; permissions "must go through App Review before app users without a role on your app can grant them"; short-lived token 1 h, long-lived **60 days** (refreshable) ([Meta docs](https://developers.facebook.com/docs/threads/get-started)) | Per-instance app registration + OAuth (PKCE), or the user pastes a token generated in Settings > Development (simplest): scope `write:statuses` ([statuses](https://docs.joinmastodon.org/methods/statuses/)) |
| BYO-app viability | A streamer-owned Meta app with themselves as tester avoids App Review (inferred from the sentence above; (unverified)) | Yes |
| Limits | Publishing quota sample: **250 posts/24 h**, 1,000 replies/24 h, 100 deletes/24 h ([troubleshooting](https://developers.facebook.com/docs/threads/troubleshooting), example response, actual values per account) | **300 requests/5 min**, media upload 30 per 30 min, delete 30 per 30 min ([rate limits](https://docs.joinmastodon.org/api/rate-limits/)); `Idempotency-Key` header stored 1 h |
| Triggers | Webhooks exist (unverified) | Streaming API (WebSocket) for notifications: local, no public URL (unverified in this research) |
| Rust | none surveyed; use `reqwest` | `megalodon` 1.3.0 (2026-04-18; Mastodon, Pleroma, Misskey...) |
| Verdict | **Plugin**, post-only | **Plugin or core-lite** (small effort, covers many instances) |

> **D-059 (2026-10-19, DEN-244): connection order for these later plugins (timing unchanged, D-046).** **Threads:** the browser authorization window is the default; the code exchange at `POST graph.threads.com/oauth/access_token` requires `client_secret` and the 60-day long-lived exchange "include[s] your app secret" (server-side only), so both run in the hosted broker on a StreamDen Meta app (D-019 later, Q-C12); the long-lived refresh needs only the token (at least 24 h old, unexpired). Until each permission passes App Review and the app is published, only Threads Testers can grant it; a BYO Meta app stays the advanced fallback ([tokens](https://developers.facebook.com/docs/threads/get-started/get-access-tokens-and-permissions), [long-lived](https://developers.facebook.com/docs/threads/get-started/long-lived-tokens), read 2026-10-19). **Mastodon:** the browser flow on the user's own instance is the default, not the pasted token: register per installation with `POST /api/v1/apps` (proprietary; no Dynamic Client Registration), `GET /oauth/authorize`, exchange at `POST /oauth/token` with the instance-issued `client_id`/`client_secret` kept in the keychain; Mastodon "only supports confidential clients", PKCE and `/.well-known/oauth-authorization-server` exist from 4.3.0, so older instances and forks differ ([authorized flow](https://docs.joinmastodon.org/client/authorized/), [OAuth spec](https://docs.joinmastodon.org/spec/oauth/), read 2026-10-19). The Settings > Development token is the advanced fallback. Token lifetime and refresh are not stated on the fetched pages.

### 3.9 Bluesky feature list and effort

Effort: S = 1-3 days, M = 1 week, L = 2+ weeks (one Rust developer, after auth plumbing).

| Feature | Notes | Effort | Tier |
|---|---|---|---|
| Auth: app password + OAuth loopback (DPoP/PAR/PKCE via `atrium-oauth`) with encrypted token store | Needed by everything | M | Core |
| Post text + auto facets (mentions/links/tags) | byte-offset facets | S | Core |
| Link card with OG fetch + thumb upload (<= 1 MB) | | S | Core |
| Images (<= 4, <= 2 MB, alt text) | | S | Core |
| Video | `video.bsky.app` flow, email-verified accounts, daily limits | M | Later |
| **Live Now status** (put/heartbeat/delete, allowlist check, `isDisabled` handling) | Warn that Kick etc. will not show a badge | S | Core |
| Trigger: new follower / like / repost / mention / reply / quote via `listNotifications` polling with local high-water mark | No public URL | S-M | Core |
| Trigger: own-account Jetstream (`dids=[user]`) | Detect external posts | S | Optional |
| Trigger: collection-wide Jetstream for inbound | ~200 KB/s | M | Skip by default |
| DMs read/send (`chat.bsky.convo.*`, polling `getLog`) | Needs DM scope/flag | M | Plugin |
| Reply to mentions / chat commands | Spam-policy caution | M | Plugin |
| Threads post / Mastodon post | | S each | Plugin |

---

## 4. Open questions for the owner

1. **P2P unofficial ingestion:** ship the email/phone-notification plugin as "experimental, off by default" with the disclaimers in 1.5, or only official routes (Stripe, PayPal, aggregators) plus the manual "Fire tip alert" button?
2. **Mailbox access:** is a **dedicated tips mailbox** an acceptable requirement? Gmail default = app password (OAuth in Testing expires every 7 days).
3. **Geography:** Venmo, Cash App Pay, Zelle and X Money are US-only. Which other regions' payment rails matter (Wise/Revolut, Pix, UPI, iDEAL, Swish)?
4. **X:** accept **BYO developer app + credits** (go-live post $0.20 with a link, or free via Web Intent) and optional own-account polling (cents per stream)? Which triggers matter most (followers, mentions, DMs)? Is a spike on the Activity API stream with user-context tokens (public-URL-free push) worth doing first?
5. **X Live:** is the owner on Live Studio (US beta, Premium), and would they pursue Livestream API whitelisting for chat?
6. **Bluesky auth:** OAuth loopback (re-login every <= 2 weeks) as default, or app password by default? Should the optional relay host a confidential OAuth client?
7. **Bluesky Live badge:** which sites does the owner stream to? Twitch and YouTube get a badge; Kick does not (unless Bluesky adds an exception).
8. **Auto-engagement:** is automatic thank-you replies/DMs wanted? Both X (explicit consent) and Bluesky (spam definition) restrict it; I would keep it opt-in and per-event.
9. **Threads/Mastodon:** are they in scope for v1 or later plugins?

---

## 5. Deltas versus `platform-apis.md`

- **Activity API** now documents follow, like, mention, reply, quote, repost, DM, chat, spaces, mute/block and `broadcast.start/end` events (not just `broadcast.chat`); `post.create/delete` public events added 2026-06-04; mute/block 2026-07-21. Whether the **HTTP stream** carries private events remains the open question.
- **Pricing page** now lists DM event reads ($0.010), Activity event prices, and shows no Basic/Pro tiers.
- **Live Studio** (July 2026) replaced Media Studio Producer.
- **Rust X crate:** `twapi-v2` 0.27.0 should be evaluated alongside `xdk-rs`.
- **Bluesky/Threads/Mastodon** were not covered before.
- **Payments:** Cash App "Earnings" pilot (2026-09-10) and Venmo retirement of the developer API are new.

## 6. Verification log (what I ran or measured)

- Jetstream event rates: Node 26 WebSocket client to `wss://jetstream2.us-east.bsky.network/subscribe` with `wantedCollections` filters for 10-15 s on 2026-10-18 (single samples; real averages vary by time of day).
- Bluesky lexicon/source files read directly from `raw.githubusercontent.com` (`atproto` and `social-app` default branches on 2026-10-18); `app.bsky.unspecced.getConfig` queried live.
- Crate and npm versions from the crates.io API and npm registry on 2026-10-18; docs.rs used to confirm `atrium-api` module coverage.
- Everything marked **[secondary]** or (unverified) should be re-checked before the corresponding feature is committed to the plan.
