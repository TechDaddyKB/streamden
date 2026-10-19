# xAI: Grok API key and billing

Last verified against official docs: 2026-10-18. Part of the [owner runbooks](../README.md). Phase P4 (AI layer), decision **Q-A1**.

## Purpose

StreamDen supports an **xAI API key** (bring your own) as one of three AI wire formats (the OpenAI Responses format also covers xAI). The owner needs a key for development. xAI has **no published third-party program** for using a SuperGrok or X Premium subscription inside other apps. The compliant path is Grok Build (xAI's CLI agent) connecting **to our MCP server**.

## Prerequisites

- [ ] A project email, password manager, 2FA where offered.
- [ ] A card for prepaid credits.
- [ ] No dependency on the product name; you can do this now.

## Cost and limits

- **Prepaid credits** are the main model: you buy credits in advance and API usage is deducted. Currently purchases go through **Guest Checkout**, and bank transfers take 2-3 business days to credit.
- **Auto top-up**: automatically buys credits when the balance falls below a threshold. Minimum top-up amount **$5**, you can set a monthly maximum, and a warning appears at **80%** of it.
- **Monthly invoiced billing** is disabled by default (email sales@x.ai). The invoiced-billing limit defaults to **$0**, so requests are rejected when prepaid credits run out.
- Cross-link: spending on the X API earns xAI API credits if you link your xAI team (0% under $200 per billing cycle, 10% for $200-$499, 15% for $500-$999, 20% for $1,000+).

## Step-by-step

1. Go to <https://console.x.ai> and sign up or log in with the project identity.
2. **Check the team first.** Billing pages live under `/team/default/`; "changes made to a team will affect all users in that team".
3. **Add a payment method**: **Billing > Billing details > Add Payment Information** (documented path). Enter billing address and tax info carefully because they appear on invoices and invoices cannot be regenerated.
4. **Buy credits**: **Billing > API spend management** (documented path). Start with a small amount, for example $10. Track usage on the **Usage explorer** page.
5. **Auto top-up** (optional): set the amount (minimum $5) and a monthly maximum, or leave it off while testing.
6. **Create the key**: open the console's **API Keys** page and create a key (the docs say "open the API Keys page and create an API key there"; exact menu path and any per-key permissions or endpoint restrictions are not documented: verify on screen). Copy it immediately.
7. Set `XAI_API_KEY` in your shell or `.env.local` (StreamDen may use `SL_XAI_API_KEY`). Never commit it.

## What to copy and where it goes

| Value | Secret? | Where it goes |
|---|---|---|
| xAI API key | **Yes** | `.env.local` (gitignored) for development. For users, the OS keychain via Settings > AI. Never the repo, chat or an AI agent. |

## Verification and test

```bash
read -rs XAI_API_KEY; export XAI_API_KEY
curl -s https://api.x.ai/v1/models -H "Authorization: Bearer $XAI_API_KEY" | head -c 400
```

(endpoint path: verify in xAI's API reference). HTTP 200 with a model list proves the key and credits. Check the **Usage explorer**.

## Common pitfalls

- **No credits, no calls.** With invoiced billing off, a $0 balance rejects requests.
- **Wrong team.** Keys and credits belong to a team. Make sure you are in the one you expect.
- **Subscription is not API credit.** SuperGrok or X Premium does not fund the API.
- **Guest Checkout only** for purchases at the time of writing, and bank transfers are slow.

## Policy notes

- **Gray area**: xAI promotes using SuperGrok/X Premium inside specific open-source agents (OpenCode, Kilo Code) but has published no developer program, client registration or terms for general third-party apps. StreamDen therefore does not offer subscription sign-in for xAI. Q-A1 option (c) would contact xAI later.
- Never read Grok Build's stored credentials from our app.
- Grok Build has a headless mode (`grok -p`) and an ACP agent mode; using those against the user's own login from our app is unreviewed. Prefer "Grok Build connects to our MCP server".

## Sources

- Billing and teams: <https://docs.x.ai/docs/key-information/billing>
- Quickstart (key creation): <https://docs.x.ai/docs/tutorial>
- Grok Build and enterprise auth: <https://docs.x.ai/build/enterprise>
- Grok Build launch: <https://x.ai/news/grok-build-cli>
- OpenCode and Kilo Code announcements: <https://x.ai/news/grok-opencode>, <https://x.ai/news/grok-kilocode>
- X API pricing page (xAI credit earning): <https://docs.x.com/x-api/getting-started/pricing>
- Research notes: [mcp-and-ai-providers.md section 2.3](../../research/mcp-and-ai-providers.md)
