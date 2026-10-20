# TikTok: nothing to register (no official LIVE events API)

Last verified against official docs: 2026-10-18; connection model re-verified 2026-10-19 (DEN-244, D-059). Part of the [owner runbooks](../README.md). Later phase (P6, optional), subject to Q-S1.

## Purpose

TikTok does **not** offer an official API for LIVE events or LIVE chat. **TikTok for Developers** (<https://developers.tiktok.com>) exists, but its products list covers none of this: Mini Games, Mini Dramas, Login Kit, Data Portability API, Share Kit, Content Posting API, Embed Videos, Research API, Display API, Green Screen Kit and Commercial Content API. None of them is described as covering LIVE streaming events or chat.

So for the core product the owner has **nothing to register**. The decision in the research is to offer TikTok only as an **opt-in, read-only, "unofficial" community plugin**, because:

- the open-source connectors (TikTok-Live-Connector, TikTokLive) are reverse-engineered, licensed **AGPL** (or "modified AGPL"), and rely on a third-party signing service;
- TikTok's Terms of Service (section 3.4) forbid reverse engineering and automated bots without written approval;
- sending chat needs the account's session cookies, which would be sent to the third-party signer.

## How users connect (D-059 applied)

Nothing to connect in core: there is no official LIVE API, no OAuth and no app. The community plugin's optional Euler Stream key is a paste. "Browser login" (session cookies) is explicitly **not** offered (Q-C9).

## Prerequisites

- None for the core app.
- For your own experiments with the unofficial route: a TikTok account (any). **You do not need to be eligible to go live yourself to read someone else's public LIVE** (the connectors read any public LIVE room by username), but see the policy risk.

## Cost and limits

- Core: **nothing**.
- Optional third-party services (not ours, not core):
  - **Euler Stream** signing API, which the connectors use: Community plan $0 (2,500 requests/day, 25 cloud WebSockets), Business $50/month (10,000/day). An API key is needed to use it.
  - **TikFinity**: a free third-party tool that pushes TikTok LIVE events into other apps (Streamer.bot uses it). "Not affiliated with TikTok."

## Step-by-step

There is no registration. If, later, the owner wants to try the unofficial route:

1. Do not use your main TikTok account for experiments. Create a throwaway account (account bans for automation are a real, undocumented risk).
2. Option 1: install TikFinity from <https://tikfinity.zerody.one>, sign in with the TikTok username you want to watch, and use its outputs (WebSocket/webhook) as the integration surface. This keeps StreamDen free of AGPL code.
3. Option 2: wait for the plugin boundary (P6) and install the community plugin. It will ask for an optional Euler Stream API key (sign up at <https://www.eulerstream.com>; paste the key into the plugin settings only).

If a future feature needs TikTok **Login Kit** or **Content Posting API** (for example, posting clips), that is a separate decision: you would create an app in the TikTok for Developers portal (menu name "Manage apps": verify on screen) and submit it for review. Nothing in the current plan needs that.

## What to copy and where it goes

| Value | Secret? | Where it goes |
|---|---|---|
| Euler Stream API key (optional) | Yes | The community plugin's settings, which store it in the OS keychain. Never the repo. |
| TikTok `sessionid` cookies (only for unofficial chat sending) | **Very** | Do not provide. Sending chat is out of scope. |

## Verification and test

Not applicable for the core. For the unofficial route, watch any public LIVE you do not own and confirm that chat and gift events arrive in the plugin. Expect breakage when TikTok changes behavior.

## Common pitfalls

- **ToS and account risk.** Automation is against TikTok's terms; do not use accounts you care about.
- **AGPL contagion.** Embedding the libraries in a non-AGPL app is a licensing problem (this is why the plan keeps them in a plugin boundary).
- **Signer dependency.** If Euler Stream or the signing method changes, connectors stop working.
- **Eligibility to stream** on TikTok is a different topic: see [go-live-requirements.md](../go-live-requirements.md).

## Policy notes

- TikTok Terms of Service section 3.4 (no reverse engineering, no automated access without written approval).
- Treat TikTok as unsupported by the project. Document it as experimental.

## Sources

- TikTok for Developers products: <https://developers.tiktok.com/products/>
- TikTok Terms of Service: <https://www.tiktok.com/legal/page/us/terms-of-service/en>
- Euler Stream pricing: <https://www.eulerstream.com/pricing>
- TikTok-Live-Connector: <https://github.com/zerodytrash/TikTok-Live-Connector>
- TikFinity: <https://tikfinity.zerody.one>
- Platform research: [platform-apis.md section 6](../../research/platform-apis.md)
