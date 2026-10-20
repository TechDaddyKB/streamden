# Cloudflare: account, Workers, Durable Objects and `wrangler` (the optional relay)

Last verified against official docs: 2026-10-18; connection model re-verified 2026-10-19 (DEN-244, D-059). Part of the [owner runbooks](../README.md). **Phase P2, and only if Q-C1 chooses option (b) or (c).** Until Q-C1 is answered you can create the account, but do not deploy anything.

## Purpose

Some services cannot reach a desktop app: **Kick events** (webhook-only, one URL per app), **Ko-fi, Throne, Fourthwall, Buy Me a Coffee, Shopify** (webhook-only) and Kick's **secret-requiring OAuth token exchange**. Q-C1 recommends an **open-source, self-hostable relay**: a Cloudflare Worker plus one Durable Object per user "mailbox".

Design summary (from [platform-apis.md section 11.2b](../../research/platform-apis.md)):

```
Vendor webhook --POST--> https://<relay>/h/<mailboxId>/<provider>
                              Worker (size limit, rate limit, fast 200)
                                   -> Durable Object(mailboxId) with SQLite queue (TTL)
Desktop app --wss://<relay>/ws/<mailboxId> (outbound only, bearer = mailbox secret)
```

It is a dumb pipe: the **desktop verifies signatures**, so vendor secrets (Kick RSA key, Ko-fi token, Throne public key) never live on the relay. A token-broker endpoint adds a `client_secret` for Kick's code exchange and never stores tokens.

## How the owner signs in (D-059 applied)

`wrangler login` is already browser-first (it opens the OAuth page; `--browser=false` prints the link for Hyprland/Wayland quirks) and an API token is the CI fallback ([wrangler commands](https://developers.cloudflare.com/workers/wrangler/commands/general/), re-read 2026-10-19). For end users the relay matters because it is the only place a **shared** confidential secret may live (the Kick token broker). D-019 keeps the official hosted relay "later"; until it runs, Kick stays bring-your-own, and the Discord webhook, Streamlabs, StreamElements and Patreon browser consents wait with it (Q-C12).

## Prerequisites

- [ ] **Q-C1** answered (self-host guide only, or an official hosted relay).
- [x] A Cloudflare account using the project email, with 2FA. The owner already has one (Q-G3, 2026-10-19, D-055); confirm 2FA and which email it uses.
- [ ] Node.js and `pnpm` or `npm` on your machine (for `wrangler`).
- [ ] Optional: a domain on Cloudflare (Q-B1, [distribution-and-signing.md](distribution-and-signing.md) section 7) for a custom hostname. Without it you get a free `*.workers.dev` address.
- [ ] For Kick through the relay: the Kick dev app from [kick.md](kick.md).

## Cost and limits

**Workers Free plan**

| Limit | Value |
|---|---|
| Requests | 100,000 per day (resets 00:00 UTC; over the limit returns error 1027) |
| CPU time per HTTP request | 10 ms |
| Subrequests per request | 50 |
| Workers per account | 100 |
| Memory | 128 MB per isolate |

**Durable Objects on the Free plan** (only the **SQLite** storage backend exists on Free)

| Limit | Value |
|---|---|
| Requests | 100,000 per day |
| Duration | 13,000 GB-s per day |
| Rows read | 5 million per day |
| Rows written | 100,000 per day |
| Stored data | 5 GB total |

- Incoming **WebSocket messages count at a 20:1 ratio** for request billing (100 messages = 5 requests); outgoing messages are not charged. With WebSocket **Hibernation**, idle sockets do not accrue duration.
- **Workers Paid** has no daily request limit and is needed beyond Free volume. The Durable Objects page's worked examples include a "minimum $5/mo" charge; check current pricing before an official hosted relay (Q-C1 option c). A personal self-hosted relay fits comfortably in the free tier.

## Step-by-step

### A. Account

1. Go to <https://dash.cloudflare.com/sign-up>, enter the project email and a password, click **Create Account**.
2. Open the verification email and confirm the address (Cloudflare sends it automatically).
3. Enable 2FA (profile security settings; the exact menu name: verify on screen) and save recovery codes.

### B. Install and log in with `wrangler`

4. In a scratch folder (not your home directory root), install locally, which Cloudflare recommends over global installs: `pnpm add -D wrangler` (or run it with `npx wrangler ...`).
5. `npx wrangler login`. This starts an OAuth flow and opens your browser. On a headless box or when the browser does not open (Hyprland/Wayland quirks), use `npx wrangler login --browser=false` and open the printed link yourself.
6. `npx wrangler whoami` prints your account and email. Use `--account <name-or-id>` if you have several.

### C. Hello-world deploy (proves the toolchain)

7. `npm create cloudflare@latest -- my-first-worker` and choose **Hello World**, **Worker only**, **JavaScript**, and decline the immediate deploy.
8. `cd my-first-worker && npx wrangler dev` and open <http://localhost:8787>.
9. `npx wrangler deploy`. On the first deploy Wrangler prompts you to register a **workers.dev subdomain**. Your Worker then runs at `https://<WORKER>.<SUBDOMAIN>.workers.dev`. A 523 error right after the first deploy usually clears within a minute.
10. Delete the sample Worker in the dashboard (Workers & Pages) when done, so no stray endpoint stays public.

### D. Deploying the real relay (P2, when the code exists)

11. An agent writes the Worker and its `wrangler` config (Durable Object bindings and migrations). You only run: `npx wrangler deploy`.
12. Add any secret **through Wrangler, never the repo**: `npx wrangler secret put KICK_CLIENT_SECRET` (and other broker secrets). The relay should hold no vendor *webhook* secrets.
13. The webhook URLs you paste into vendor dashboards look like `https://<RELAY_HOST>/h/<mailboxId>/<provider>` (exact shape set by the relay spec). Where to paste: Kick app **Enable Webhooks** ([kick.md](kick.md)), Ko-fi webhook URL, Throne subscriber URL, Fourthwall endpoint ([streaming-services-donations.md](streaming-services-donations.md)).
14. **Custom domain (optional):** add your domain to Cloudflare (change nameservers at the registrar), then in the Worker's settings add a **Custom Domain** (menu name: verify on screen).
15. **CI deploy (optional):** create an API token (profile > **API Tokens** > **Create Token**, the "Edit Cloudflare Workers" template; verify on screen) and store it as `CLOUDFLARE_API_TOKEN` plus your `CLOUDFLARE_ACCOUNT_ID` in GitHub Actions secrets. When `CLOUDFLARE_API_TOKEN` is set it takes priority over the stored OAuth login.

## What to copy and where it goes

| Value | Secret? | Where it goes |
|---|---|---|
| Account email/password, recovery codes | Yes | Password manager only |
| `workers.dev` subdomain, relay hostname | No | Settings > Relay URL in the app, docs |
| `CLOUDFLARE_API_TOKEN`, account ID (CI only) | Yes | GitHub Actions secrets |
| Worker secrets (Kick client secret etc.) | Yes | `wrangler secret put`, never `.env.local` committed to git |
| Mailbox ID / mailbox secret | Yes | Generated per user; the desktop keeps it in the OS keychain |

(Names are suggestions; P0 and the relay spec decide.)

## Verification and test

- `npx wrangler whoami` shows the right account.
- `curl -i https://<WORKER>.<SUBDOMAIN>.workers.dev/` returns the hello-world response (later, `/health` from the relay).
- Webhook round trip (once the relay exists): enable a test webhook (Ko-fi **Send test**), watch `npx wrangler tail`, and confirm the desktop receives the event.
- Check the daily numbers in the dashboard (Workers & Pages > Metrics; verify on screen) to confirm you are far from the 100,000 requests per day limit.

## Common pitfalls

- **Error 1027**: the Free daily request limit was exceeded. It resets at 00:00 UTC.
- **10 ms CPU limit on Free**: keep the Worker thin (verify nothing heavy runs in the request path; verify signatures on the desktop).
- **Kick auto-unsubscribes** your app if the webhook fails for more than a day, so relay downtime has consequences. Buffering in the Durable Object helps for offline desktops, not for a dead relay.
- **Durable Objects on Free are SQLite only.** Do not design around the older key-value backend.
- **`wrangler login` and `CLOUDFLARE_API_TOKEN`**: the token wins. Unset it if you want the OAuth session.
- **Quick Tunnels (`trycloudflare.com`)** change hostname every run, have no SSE and no uptime guarantee: not suitable for vendor webhooks.
- A hosted relay run by the project adds privacy-policy, abuse-handling and uptime duties (Q-C1 option c), and Kick app **verification** (email developers@kick.com) to lift the 1,000 chat-subscription cap.

## Policy notes

- Cloudflare's terms and acceptable use apply to whatever the relay forwards. Do not log request bodies longer than needed, and keep a clear retention window (the design's TTL is about 7 days).
- Webhook payloads include donor names and amounts. Treat them as personal data (Q-H1).

## Sources

- Create an account: <https://developers.cloudflare.com/fundamentals/account/create-account/>
- Workers limits: <https://developers.cloudflare.com/workers/platform/limits/>
- Durable Objects pricing and free limits: <https://developers.cloudflare.com/durable-objects/platform/pricing/>
- WebSocket hibernation: <https://developers.cloudflare.com/durable-objects/best-practices/websockets/>
- Wrangler login/logout/whoami: <https://developers.cloudflare.com/workers/wrangler/commands/general/>
- Get started with the CLI: <https://developers.cloudflare.com/workers/get-started/guide/>
- Tunnels (quick tunnels caveats): <https://developers.cloudflare.com/tunnel/get-started/quick-tunnels/>
- Research: [platform-apis.md section 11](../../research/platform-apis.md)
