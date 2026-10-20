# OpenAI: API key, spend limits and "Sign in with ChatGPT"

Last verified against official docs: 2026-10-18; connection model re-verified 2026-10-19 (DEN-244, D-059). Part of the [owner runbooks](../README.md). Phase P4 (AI layer), decision **Q-A1**.

## Purpose

OpenAI is used in two different ways, and they need different things:

1. **API key (bring your own).** The user's own key pays per token. Works for everything (Responses API, also the wire format xAI and the ChatGPT-plan route share). The owner needs one for development.
2. **"Sign in with ChatGPT" (SIWC) with "ChatGPT plan usage".** A user signs in with their ChatGPT Plus or Pro account and the *plan* pays for eligible Responses API requests. This is an official OpenAI program for **open-source projects and personal projects that run locally**. Q-A1 option (b) recommends offering it in v1.

## How users connect (D-059: browser login first)

- **Default:** **Continue with ChatGPT** (SIWC): the app opens `auth.openai.com` in the browser, registers itself dynamically on first sign-in, uses PKCE and a `127.0.0.1` loopback, and holds no client secret ([token-sharing-open-source](https://developers.openai.com/siwc/token-sharing-open-source), re-read 2026-10-19). The user's Plus/Pro plan then pays for eligible Responses API calls, within the preview constraints listed under Cost and limits.
- **Fallback (Advanced):** paste an **API key**. This is a different product: pay-as-you-go API billing, not a consumer subscription. The UI must say so, and must never read the Codex CLI's stored login.
- **Provider gate:** open-source/local apps need no **manual** developer registration (the dynamic registration above still happens); a paid or remotely hosted StreamDen would need OpenAI's interest-form approval first.

## Prerequisites

- [ ] A project email and a password manager. Billing details (card). 2FA on the OpenAI account.
- [ ] For SIWC: nothing to register for an open-source/local app (see below), but to **test** you need an eligible ChatGPT account: **Plus or Pro** ("Eligible ChatGPT Plus and Pro users"; Go is unverified).
- [ ] For SIWC the app must qualify as *open-source or locally run*. That ties to **Q-B2** (license) and **Q-B3** (monetization). A paid or remotely hosted app needs OpenAI's approval first.
- [ ] You do not need the product name before creating an API key. You need it only if you ever apply for SIWC as a paid/hosted app.

## Cost and limits

- API: pay as you go. After you add billing details OpenAI assigns your organization an approved **usage limit** that rises with usage tiers (see <https://platform.openai.com/settings/organization/limits>).
- **Spend alerts** notify you; **hard spend limits** stop requests with HTTP 429 once reached (enforcement is not instantaneous, so spend can slightly exceed the limit).
- SIWC plan usage costs the *user* nothing beyond their ChatGPT plan, but their plan has its own usage limits. The "Manage usage" link is `https://chatgpt.com/settings/usage`.
- Constraints on the SIWC route (preview): `store: false` and `stream: true` are required; many parameters (temperature, top_p, max_output_tokens, metadata, previous_response_id) must be omitted; hosted MCP/connectors, Responses `tool_search`, file search, code interpreter, image generation, audio input and transcription are not supported.

## Step-by-step: API key

1. Go to <https://platform.openai.com> and sign in or create an account with the project email. Enable 2FA in your account security settings (verify on screen).
2. **Billing.** Open <https://platform.openai.com/settings/organization/billing> (the docs name this "Billing settings"). Add a payment method and buy a small amount of credits, for example $10 (labels and whether credits are prepaid: verify on screen; the production guide describes pay-as-you-go, and a `credit_balance_exhausted` error means prepaid credits ran out).
3. **Spend limit.** Open <https://platform.openai.com/settings/organization/limits>. In **Spend**, choose **Edit spend limit**, enter a **Monthly spend limit** (for example $20), turn on **Enforce a hard limit** and **Save**. Also create a **spend alert** at a lower amount. For tighter control use **Project settings > Limits > Spend** on the project that owns the key.
4. **Create the key.** Open <https://platform.openai.com/settings/organization/api-keys> and create a new secret key (button label: verify on screen).
   - Give it a name such as `streamden-dev`, pick the project, set permissions to **Restricted** if offered and enable only what you need (Responses/model requests), and **set an expiration date** (OpenAI "strongly recommends" it).
   - Copy the key immediately; it is shown once.
5. Store it in the password manager and in `.env.local` as `OPENAI_API_KEY` (the SDK default name; StreamDen may use `SL_OPENAI_API_KEY`). Never put it in the repo.

## Step-by-step: Sign in with ChatGPT (what the owner actually does)

For an **open-source or locally run** app there is **no registration step**. OpenAI registers the client dynamically during the first sign-in:

1. Nothing to do in any dashboard. The app picks a stable `ext_agent_host_id` (a JWK-thumbprint `urn:` or a UUID `urn:uuid:`), opens `https://auth.openai.com/api/accounts/authorize` in the browser with PKCE and a loopback callback `http://127.0.0.1:<port>/auth/callback` (**not** `localhost`), and OpenAI returns an issued `client_id` (`oaiapp_...`) that the app persists. No client secret and no API key are used.
2. Your jobs: (a) make sure the repo is open source with an OSI license (Q-B2), (b) have a Plus or Pro ChatGPT account to test with, (c) read the UI rules (button "Continue with ChatGPT", first-use modal, "Using ChatGPT plan" indicator, "Manage usage" link, and the one MUST: clearly show which of the app's plans support ChatGPT plan usage).
3. **Paid or hosted app?** Complete OpenAI's interest form to join the waitlist *before* offering it to users: <https://openai.com/form/sign-in-with-chatgpt-interest/>. The "Request a client ID" page is for a select group of commercial partners and is not what an open-source app uses.
4. Do **not** read the Codex CLI's stored login (`~/.codex/auth.json`). No official page allows it; SIWC exists to issue our own credential.
5. Open question (not addressed by OpenAI): whether an always-on, viewer-facing chatbot is acceptable on plan usage. Treat as gray: warn the user and let them set their own cap.

## What to copy and where it goes

| Value | Secret? | Where it goes |
|---|---|---|
| API key `sk-...` | **Yes** | `.env.local` (`OPENAI_API_KEY` / `SL_OPENAI_API_KEY`) for development; for users, the OS keychain via Settings > AI. Never the repo, chat or an AI agent. |
| SIWC `client_id` (`oaiapp_...`), `ext_agent_host_id` | Low | Generated and stored by the app on each user's machine. |
| SIWC access/refresh tokens | Yes | OS keychain (owner-only, refreshed atomically). |

(Names are suggestions; P0 decides the real location.)

## Verification and test

```bash
read -rs OPENAI_API_KEY; export OPENAI_API_KEY
curl -s https://api.openai.com/v1/models -H "Authorization: Bearer $OPENAI_API_KEY" | head -c 400
```

HTTP 200 with a model list proves the key and billing work. Check <https://platform.openai.com/settings/organization/usage> afterwards. For SIWC, test with your own Plus/Pro account once the P4 sign-in screen exists.

## Common pitfalls

- **ChatGPT subscription is not API credit.** A Plus/Pro plan does not fund the API key, and vice versa.
- **Hard limit hit** returns 429; raise the limit or alert earlier.
- **Key in a shell history or a repo.** Use `read -rs` as shown. If a key leaks, delete it on the API keys page and create a new one.
- **SIWC loopback**: `127.0.0.1`, not `localhost`.
- **SIWC request shape**: `store:false` + `stream:true`, and no unsupported parameters or tools.
- Only owners can change billing; invite teammates via the Team page if needed.

## Policy notes

- OpenAI usage policies apply to anything the AI chat responder says in viewers' chats. The responder has no write tools by design.
- SIWC: the plan-usage feature is "optional", gives *no access to the user's conversations*, and is available "to open-source projects, personal projects that run locally, and selected private apps".
- EU AI Act Article 50 disclosure for AI chat replies has applied since 2026-08-02 (secondary reading; see Q-A3).

## Sources

- SIWC (open-source overview): <https://developers.openai.com/siwc/token-sharing-open-source>
- SIWC cookbook (eligibility, waitlist): <https://developers.openai.com/cookbook/articles/sign-in-with-chatgpt>
- SIWC request a client ID: <https://developers.openai.com/siwc/request-client-id>
- SIWC interest form: <https://openai.com/form/sign-in-with-chatgpt-interest/>
- SIWC sign-in flow: <https://developers.openai.com/siwc/token-sharing-open-source/sign-in>
- SIWC preview limitations: <https://developers.openai.com/siwc/token-sharing-open-source/preview-limitations>
- SIWC UI/UX guidelines: <https://developers.openai.com/siwc/ui-ux-guidelines>
- Production best practices (keys, billing, limits): <https://developers.openai.com/api/docs/guides/production-best-practices>
- Spend limits: <https://developers.openai.com/api/docs/guides/spend-limits>
- Quickstart: <https://developers.openai.com/api/docs/quickstart>
- Research notes: [mcp-and-ai-providers.md section 2.1](../../research/mcp-and-ai-providers.md)
