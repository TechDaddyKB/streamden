# Stripe and PayPal Business: owner setup preparation

Official public documentation checked **2026-10-20 UTC** (DEN-254). Part of the
[owner runbooks](../README.md), checklist rows 25–26; core tips are P2 (D-046).
This is preparation for a later owner task, not an instruction to register anything now.
No account, console, API or authorization operation was performed for this document.
Any owner report of existing registrations remains **unverified**: account identity,
environment, permissions and payment readiness have not been observed.

## Readiness before owner dispatch

DEN-254 needs coordinator review, cross-vendor review and publication/integration before
the coordinator may mark prep Done. Only then may the reserved N9 human follow-up be
created, with `kind:owner-action`, `agent:human` and `depends_on: [254]`.
Do not ask the owner to fill missing technical fields. The following remain agent gates:

- **PayPal relay:** no deployed hostname, mailbox URL or production domain is established
  by this prep. D-019 already chooses self-hostable P2 relay, official hosted service later;
  [Q-C1](../../QUESTIONS.md#q-c1--relay-stance--round-1) is not a new preference question.
  The relay agent must supply the actual deployment's HTTPS port-443 listener URL,
  mailbox provisioning instructions and separate sandbox/live routing. Record these
  privately if the mailbox URL is sensitive; publish only the non-secret hostname and
  evidence reference. Never paste the conceptual URL in the [relay guide](relay-cloudflare.md)
  into PayPal. Q-C12 does not accelerate the official hosted milestone.
- **Connector packet:** the implementation agent must supply the implemented local Stripe
  listener address, exact event subscription list, secure credential import instructions,
  and a sandbox test harness that redacts credentials and donor data. No port or Settings
  screen is promised here. The destinations below are storage requirements, not shipped UI.
- **Permissions and coverage:** validate Stripe Events-read access in a sandbox against
  the actual connector's GET requests; record the Dashboard permission label and errors
  without keys. For PayPal, prove a checkout created by the selected REST app generates
  the subscribed capture event. Account-wide receipt coverage is not established.

These gates block end-to-end owner dispatch and live use, not writing/reviewing this prep.
Use synthetic fixtures while the relay/connector is absent. Do not create a human task
or request a new owner decision to work around these gates.

## Stripe account and CLI browser pairing

Later owner procedure, once dispatched:

1. Open [Stripe registration](https://dashboard.stripe.com/register), or sign in to the
   existing merchant account. Use the real account holder's details and a unique password;
   enable 2FA in [user settings](https://dashboard.stripe.com/settings/user) and retain recovery
   codes in the password manager. In the Dashboard account picker choose **Sandboxes**,
   then open a separate development sandbox ([sandbox guide](https://docs.stripe.com/sandboxes)).
   Record its name/ID; do not enter live mode. Live activation needs
   truthful business/product/representative information in Stripe's onboarding checklist;
   never substitute the software name for a legal entity. The live business country cannot
   be changed after activation. [Account setup](https://docs.stripe.com/get-started/account/set-up)
   explains activation and customer-visible business/support details. Country, legal name,
   website and payout details are owner-specific, not values an agent can invent.
2. Install the official CLI using [Stripe's installation guide](https://docs.stripe.com/cli/install)
   and its linked OS instructions; record `stripe version` before choosing version-specific
   steps. This prep installs no payment tooling and authorizes no agent access.
3. For CLI versions **newer than v1.50.0**, an Administrator or IAM Admin selects the
   intended account/environment, then **Settings → Team and security → MCP and CLI access**
   ([direct route](https://dashboard.stripe.com/settings/mcp-and-cli-access)) and enables CLI
   access. Earlier versions are unaffected. Enable only the intended CLI/environment access.
4. In the owner's private terminal run `stripe login`. Compare its pairing code with the
   Stripe Dashboard opened in the system browser; approve only the intended sandbox/account
   and minimum offered permissions. Inspect `stripe login list` privately, and use
   `stripe switch` to select the sandbox. The official [login reference](https://docs.stripe.com/cli/login)
   documents credential-store use when available, automatic refresh, `stripe reauth` to change
   access and `stripe logout` to revoke this user's session. If browser launch fails, open
   the CLI-provided Stripe URL manually; `stripe login --interactive` is the advanced API-key
   fallback. Do not capture that session or its output in agent logs.

Browser pairing satisfies D-059 for the CLI. It is not a public native OAuth client for
StreamDen. No Stripe Connect registration, platform secret or Connect OAuth is required.

## Stripe restricted read-only key and Payment Link

The key is the advanced events-polling alternative, not proof that it can authenticate
`stripe listen`. Create a separate key in each environment; never distribute a merchant key.

1. In the selected sandbox open [API keys](https://dashboard.stripe.com/apikeys), then
   **Create restricted key**. Start from zero permissions; use **Key name**
   `StreamDen Dev Events Read`. Set only the Events resource to **Read**, everything else
   **None**, including connected-account access; no Write permission. The exact Events
   label/availability needs the agent's sandbox confirmation above. If absent, stop this
   alternative and record the non-secret error; never select an all-resources preset.
2. Select **Create key**, complete 2FA, and save the displayed value directly in the password
   manager. **Add a note**: `Password manager: StreamDen / Stripe / Sandbox / Events Read`.
   These steps and permission levels follow [restricted keys](https://docs.stripe.com/keys/restricted-api-keys).
   Later, after sandbox acceptance and a separate live gate, create `StreamDen Live Events Read`
   with the same tested read permissions. Sandbox/live keys and objects are separate;
   see [API keys](https://docs.stripe.com/keys). Restricted key values cannot be retrieved after
   creation in either environment; save each immediately.
3. For the test Payment Link, stay in the sandbox and open
   [Payment Links](https://dashboard.stripe.com/payment-links/create/customer-chooses-pricing),
   click **New**, select **Customers choose what to pay**. Suggested test content:
   title `StreamDen Dev test tip`, description `Sandbox test for streaming content; no real payment`,
   no image, suggested amount **USD 1.00** where USD is available, optional min/max unset.
   If the account uses another currency, the agent records that supported test currency
   and its valid minimum before dispatch. Click **Create link** and retain the sandbox URL.
   These are test fixture values, not production branding. The
   [creation guide](https://docs.stripe.com/payment-links/create) documents these fields;
   this pricing model does not support recurring payments.
4. Before any live link, confirm [tip eligibility](https://support.stripe.com/questions/requirements-for-accepting-tips-or-donations):
   tips must relate to goods/services provided; do not label personal transfers or an
   unqualified charitable fundraiser as tips. Agent prep must resolve the actual public
   description, currency, website and account activation before owner live dispatch.

The [CLI listener](https://docs.stripe.com/cli/listen) needs no Dashboard webhook endpoint.
The implementation packet supplies the real loopback destination for `stripe listen --forward-to`;
its signing secret belongs to that listener, not the restricted key. Default requests are
sandbox; `--live` is a separate live action. Never add it during sandbox testing.
The polling alternative reads [GET /v1/events](https://docs.stripe.com/api/events/list), whose
lookback is 30 days; it must not require permission to create payments or refunds.
After the connector packet exists, open only the sandbox Payment Link and pay USD 1.00
(or the recorded supported test amount) with Stripe's [test card](https://docs.stripe.com/testing)
`4242 4242 4242 4242`, expiry `12/34`, CVC `123`, and synthetic form details. Confirm the
sandbox payment and corresponding verified connector event refer to the same payment;
record event ID/pass/fail without payloads. A checkout success page alone does not prove
ingestion. Never enter real card details or run this fixture against a live link.

## PayPal Business REST app: sandbox, then live

D-059 applies only where a provider offers the needed flow. PayPal REST uses the merchant's
own client ID/secret and `client_credentials`, not end-user browser consent. No partner
onboarding, Platform app, Log in with PayPal, personal-account scraping or IPN is included.

1. Later, open [PayPal Business signup](https://www.paypal.com/us/business/open-business-account)
   or use the existing Business account. Complete PayPal's real country-specific business,
   identity and contact checks privately: the US signup guide lists full legal name,
   email, password, tax ID/SSN, business description and business bank details. Confirm
   email and bank/card using PayPal's prompts; enable account 2FA and save recovery privately.
   Do not invent legal/tax/bank values. Existing
   registration does not prove verified Business status or live API eligibility.
2. Sign in to the [Developer Dashboard](https://developer.paypal.com/dashboard/).
   **Apps & Credentials → Sandbox → Create App**. Set **App Name** `StreamDen Dev`,
   **Type** `Merchant`, and **Sandbox business account** to the test seller, not the test buyer.
   Click **Create App**. The app details show **Client ID** and **Secret**; transfer directly
   to the password manager. Do not reuse a Default Application belonging to other tests.
   [PayPal's credential procedure](https://www.paypal.com/us/cshelp/article/how-do-i-create-paypal-rest-api-credentials-ts1949)
   confirms these fields and the Sandbox/Live selector.
3. Under **Testing Tools → Sandbox Accounts**, inspect the business seller and personal
   buyer separately (**⋮ → View/Edit Account**); keep their generated logins private.
   The [REST getting-started guide](https://developer.paypal.com/api/rest/) documents these
   simulated accounts. They are not live money or evidence of live merchant approval.
4. Use only the PayPal payments features required by the implemented capture-event test.
   Do not request Payouts, Vault, subscriptions, partner access or Log in with PayPal.
   Exact console feature labels/defaults and granted scopes are unobserved; the agent must
   map the actual connector calls to current official API permissions before live readiness.
   REST has no Stripe-style read-only-key promise. Authentication is **POST /v1/oauth2/token**,
   HTTP Basic client ID/secret, form `grant_type=client_credentials`, on
   `https://api-m.sandbox.paypal.com`. The connector, not the owner copying a response,
   handles token expiry. [REST authentication](https://developer.paypal.com/api/rest/authentication/)
   documents the grant. Never request credentials in chat or run verbose credential commands.
5. **Live is a separate later gate:** after sandbox validation and verified Business eligibility,
   select **Apps & Credentials → Live → Create App**, name `StreamDen`, type `Merchant` where
   offered, owned by the real Business account. Save the distinct live client ID/secret;
   use `https://api-m.paypal.com`. Re-register the webhook under the live app. Never switch
   only the hostname while retaining sandbox credentials. The
   [production guide](https://developer.paypal.com/api/rest/production/) confirms separate
   environments; its legacy signature/User ID examples are not REST client credentials.

## PayPal webhook: wait for the real relay URL

After the relay gate, select the correct Sandbox app in **Apps & Credentials**, locate its
webhook settings and add a webhook (**Add Webhook** label: verify on screen).
**Webhook URL** must be the exact provisioned relay HTTPS listener on port 443, never localhost,
a success/return URL, a conceptual mailbox template or an invented project domain.
Select only the connector's tested [event names](https://developer.paypal.com/api/rest/webhooks/event-names/):
the capture-state test set is `PAYMENT.CAPTURE.COMPLETED`, `PAYMENT.CAPTURE.PENDING`,
`PAYMENT.CAPTURE.DECLINED`, `PAYMENT.CAPTURE.REFUNDED`, `PAYMENT.CAPTURE.REVERSED`;
include `CUSTOMER.DISPUTE.CREATED` only when dispute handling is implemented/tested.
Save the generated **Webhook ID** in that environment's PayPal password-manager item.
Import it only through the connector packet's procedure once that exists.
Do not select all events to conceal missing connector support.

[Webhook overview](https://developer.paypal.com/api/rest/webhooks/) establishes that subscriptions
are **app-specific**, not account-wide. A PayPal.me payment or dashboard-created Payment Link
is not proven to belong to this REST app. Before promising tip coverage, the agent must
document the supported same-app checkout path and demonstrate its event end to end.
If that path is absent, retain fixtures and mark payment coverage unverified.

[Webhook integration](https://developer.paypal.com/api/rest/webhooks/rest/) requires HTTPS/443,
a 2xx acknowledgment and message verification using the saved Webhook ID. The relay packet
must preserve original bytes/headers for the desktop verifier. A simulator event is a mock,
belongs to no app, uses `WEBHOOK_ID`, and cannot use the postback verification endpoint.
It proves neither app subscription nor payment. Test a real sandbox app capture separately;
reject tampered messages and duplicates before a verified tip claim. Live endpoint/ID and
signature verification must be validated separately; never disable verification to pass a test.

## Credential destinations and completion evidence

These password-manager item paths are exact organizational labels for this procedure;
they are not environment variables or existing app/keychain service identifiers.

| Value | Private destination now | Later runtime destination |
|---|---|---|
| Stripe/PayPal passwords, 2FA recovery, sandbox buyer/seller logins | Password manager only | Never in StreamDen |
| Stripe CLI session | CLI-managed OS secure store when available; verify backend privately | Owner's CLI session, never bundled with the app |
| Stripe restricted key | `StreamDen / Stripe / Sandbox / Events Read`; distinct `Live` item later | Per-user OS keychain via implemented import; D-053 warned encrypted fallback if unavailable |
| Stripe listener signing secret | `StreamDen / Stripe / Sandbox / Listener`; separate live listener item | Desktop verifier keychain; no raw logs |
| Stripe Payment Link | Sandbox test inventory; production public link only after live gate | Connection config/approved public tip page |
| PayPal client ID + secret | `StreamDen / PayPal / Sandbox / StreamDen Dev`; separate `Live / StreamDen` item | Merchant's desktop keychain; never a shared shipped secret |
| PayPal Webhook ID + environment | Same PayPal item, separate sandbox/live records | Desktop verifier config; ID is not a client secret |
| Relay mailbox URL/access credential | Private relay provisioning record | Desktop relay config/keychain per relay contract; never public evidence |

No credential goes to repo, board, chat, CI, or an agent; no `.env` file is needed for this
procedure. Do not send PayPal client secrets to a shared broker for this own-app path.
After future sandbox testing, record only environment, app/account public identifiers,
CLI version, permissions (names only), test event IDs, pass/fail and date in the N9 project
evidence. Redact donor data, pairing links, tokens and mailbox URLs. CLI pairing, simulator
delivery, sandbox capture and live acceptance are four separate evidence claims.

## Costs, failure handling and remaining gates

Sandbox tests move no real money. No paid plan or live charge is needed for this prep.
Stripe's [US standard pricing](https://stripe.com/us/pricing) lists no setup/monthly fee,
**2.9% + USD 0.30** per successful domestic-card payment; Payment Links is included with
Payments. Optional custom domain/invoice add-ons are unnecessary. PayPal's
[US Business signup](https://www.paypal.com/us/business/open-business-account) lists no opening
or monthly maintenance fee; its [US merchant fee schedule](https://www.paypal.com/us/business/paypal-business-fees)
lists **3.49% + USD 0.49** for PayPal Checkout in USD. Other methods/countries, international
payments, conversion, disputes and negotiated plans differ. Before live dispatch, the agent
records the merchant's actual country, product/rate schedule and date; these US examples
are not an observed account quote. Relay/domain costs are separate per the relay guide.

If pairing fails, check CLI version, user role, account/environment and CLI enablement;
revoke an unintended session with `stripe logout`. On Stripe permission errors, retain
read-only scope and have the agent identify the failing GET before adding any Read access.
If PayPal events are absent, check Sandbox/Live selection, REST app association, webhook
ID, relay reachability and signature validation; do not grant partner access as a workaround.
Revoke/rotate only a task-created credential in its provider console if exposed, update its
private runtime entry and retest. Delete only the task-created test webhook/link/app after
checking other integrations do not use it; preserve merchant accounts and financial records.

Prep does not establish account ownership, activation, provider approval, real endpoint,
implemented import/connector, least-privilege acceptance or live coverage. Coordinator
review/publication and those readiness gates remain; no owner follow-up has been created.
