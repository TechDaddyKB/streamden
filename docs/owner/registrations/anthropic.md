# Anthropic: Claude API key and billing

Last verified against official docs: 2026-10-18; connection model re-verified 2026-10-19 (DEN-244, D-059). Part of the [owner runbooks](../README.md). Phase P4 (AI layer), decision **Q-A1**.

## Purpose

StreamDen lets users paste **their own Anthropic API key** (bring your own key, "BYOK") to power the in-app assistant and the optional AI chat responder. The owner needs a key for development.

**Important policy:** Anthropic **prohibits** third-party apps from offering Claude.ai login or routing requests through Free/Pro/Max subscription credentials. So StreamDen must not use a Claude subscription for its own requests. The compliant paths are (1) the API key in our app and (2) the user's own Claude Desktop or Claude Code connecting **to our MCP server**.

## How users connect (D-059 applied: no browser login exists)

Anthropic **prohibits** third-party apps from offering Claude.ai login or routing requests through Free/Pro/Max credentials, and from collecting or intermediating Claude.ai session tokens ([legal and compliance](https://code.claude.com/docs/en/legal-and-compliance), re-read 2026-10-19). So the pasted **API key** is the default and only in-app path (D-021), billed to the key owner per token. The browser-free "connect your AI" alternative is the user's own Claude Desktop or Claude Code talking to StreamDen's MCP server. D-059 changes nothing here.

## Prerequisites

- [ ] A project email, password manager, and 2FA wherever offered.
- [ ] A payment card for API credits. A **Claude Pro/Max subscription is billed separately and does not fund the API**.
- [ ] No dependency on the product name, so you can do this now.

## Cost and limits

- Pay as you go with credits/payment in the Claude Console.
- **Usage tiers** are assigned automatically. New organizations may start in an **Evaluation** tier with lower limits. Standard monthly spend caps: **Start $500**, **Build $1,000**, **Scale $200,000**; **Custom** has none. When the cap is reached, the API returns HTTP 429 `enforced_spend_limit_reached` until 00:00 UTC on the 1st of the next month.
- Rate limits are per model class in requests/min, input tokens/min and output tokens/min (for example Start tier: 1,000 RPM, 2,000,000 ITPM, 400,000 OTPM for Sonnet 5.5). Cached-read input tokens do not count toward ITPM for most models.
- Your own spend limit can be set lower than the tier cap and returns HTTP 400 `invalid_request_error` when reached.
- To raise limits use **Request tier increase** on the Rate limits page.

## Step-by-step

1. Go to the Claude Console at <https://platform.claude.com> (formerly console.anthropic.com; the old address redirects, verify) and sign in or create an account with the project email.
2. **Billing and credits.** Open <https://platform.claude.com/settings/billing>. Add a payment method and buy credits (labels and any minimum purchase: verify on screen).
3. **Set your own spend limit.** On the same Billing page, in **Spend limits**, click **Adjust limit** (or **Set limit** if none is set), enter a value such as $20 and confirm. It cannot exceed your tier cap.
4. **Create a workspace** (recommended): <https://platform.claude.com/settings/workspaces> > create `streamden-dev`. Workspaces separate environments and let you set per-workspace spend and rate limits. Limits cannot be set on the Default Workspace.
5. **Create the key.** Open <https://platform.claude.com/settings/keys> and click **Create key**. Name it, choose an **expiration**, set **Linked account** to yourself (a *personal key*; use a *service account* key for anything shared), and choose the workspace to scope it to. The Console shows the full key (starts with `sk-ant-`) **only once**: copy it now. If **Create key** is disabled, your role does not allow it (ask an org admin).
6. Save it in the password manager and set `ANTHROPIC_API_KEY` in `.env.local` (the SDK default name; StreamDen may use `SL_ANTHROPIC_API_KEY`).

## What to copy and where it goes

| Value | Secret? | Where it goes |
|---|---|---|
| API key `sk-ant-...` | **Yes** | `.env.local` (gitignored) for development. For users, the OS keychain via Settings > AI. Never the repo, chat or an AI agent. |
| Workspace ID `wrkspc_...` | No | Only needed if a multi-workspace key is used (`anthropic-workspace-id` header). |

## Verification and test

This lists models and costs nothing:

```bash
read -rs ANTHROPIC_API_KEY; export ANTHROPIC_API_KEY
curl -s https://api.anthropic.com/v1/models \
  -H "x-api-key: $ANTHROPIC_API_KEY" -H "anthropic-version: 2023-06-01" | head -c 400
```

HTTP 200 with a model list proves the key works. Then check <https://platform.claude.com/usage> and the **Rate limits** page <https://platform.claude.com/usage/limits> to see your tier.

## Common pitfalls

- **Subscription is not API access.** Pro/Max does not give an API key or credit.
- **Key shown once.** Lose it and you must create a new one.
- **Spend cap 429 has no `retry-after`**; SDK retries keep failing until the month resets or you request an increase.
- **Multi-workspace key** needs the `anthropic-workspace-id` header.
- **Personal keys stop working** if you leave the organization.

## Policy notes (Anthropic's wording, from the research)

- Anthropic does not permit third-party developers to offer Claude.ai login into their apps, or to route requests through Free, Pro or Max plan credentials on behalf of users, and developers may not collect, store or intermediate Claude.ai credentials or session tokens. Use API key authentication.
- The Agent SDK: "Unless previously approved", third parties may not offer claude.ai login or rate limits. There is an approval route through Anthropic sales. Asking for it is Q-A1 option (c), a later decision for the owner.
- **Never** read `~/.claude/.credentials.json`, the macOS Keychain entry or `CLAUDE_CODE_OAUTH_TOKEN` from our app.
- Driving the user's installed `claude` CLI headlessly for viewer-facing features is gray/high risk. The defensible pattern is the user launching their own Claude Code and pointing it at our MCP server.

## Sources

- Get your Claude API key: <https://platform.claude.com/docs/en/get-api-key>
- API overview: <https://platform.claude.com/docs/en/api/overview>
- Rate limits and spend limits: <https://platform.claude.com/docs/en/api/rate-limits>
- Legal and compliance: <https://code.claude.com/docs/en/legal-and-compliance>
- Consumer Terms: <https://www.anthropic.com/legal/consumer-terms>
- Research notes: [mcp-and-ai-providers.md section 2.2](../../research/mcp-and-ai-providers.md)
