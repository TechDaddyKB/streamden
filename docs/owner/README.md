# Owner runbooks: registrations, accounts and go-live setup

These are the **human-only steps** that agents cannot do for you: creating developer apps, buying credits, enabling 2FA, proving your identity, copying secrets. Each runbook is click-by-click, with the exact values to type, what to copy, where it goes, and how to test it.

Written 2026-10-18 against current official developer docs. Developer consoles change often. Where I could not confirm a button or field label in an official source, the text says **(verify on screen)**. If your screen differs from a runbook, trust the screen and ask an agent to fix the runbook.

> **Tracked on the orchestrator board** (project `streamden`, cards labelled `kind:owner-action`): DEN-1 GitHub repo (**done**, https://github.com/TechDaddyKB/streamden; please verify) · DEN-6 Twitch dev app ·
> DEN-16 Round-2 answers · DEN-17 baseline accounts. Name decided: **StreamDen** (D-015). For dev apps during
> development, use "StreamDen Dev"; register production apps separately later.

## Contents

| File | What it covers |
|---|---|
| [registrations/twitch.md](registrations/twitch.md) | Twitch developer app (Public client, Device Code Flow), optional bot account, reward-ownership caveat |
| [registrations/google-youtube.md](registrations/google-youtube.md) | Google Cloud project, YouTube Data API v3, consent screen, Desktop OAuth client, 7-day token trap, verification, quota extension, end-user version |
| [registrations/kick.md](registrations/kick.md) | Kick developer app, scopes, PKCE, webhooks (pending relay decision) |
| [registrations/x-twitter.md](registrations/x-twitter.md) | X developer app, pay-per-use credits, OAuth 2.0 Native App, posting |
| [registrations/discord.md](registrations/discord.md) | Webhook path, application, bot token, intents, invite permissions |
| [registrations/rumble.md](registrations/rumble.md) | Rumble Live Stream API URL (no registration) |
| [registrations/tiktok.md](registrations/tiktok.md) | Why there is nothing official to register |
| [registrations/facebook.md](registrations/facebook.md) | Meta developer app, Login for Devices, App Review (later phase) |
| [registrations/streaming-services-donations.md](registrations/streaming-services-donations.md) | Streamlabs, StreamElements, Ko-fi, Patreon, Throne, Fourthwall |
| [registrations/openai.md](registrations/openai.md) | OpenAI API key, spend limits, "Sign in with ChatGPT" |
| [registrations/anthropic.md](registrations/anthropic.md) | Claude API key, spend limits, subscription policy |
| [registrations/xai.md](registrations/xai.md) | xAI API key and credits |
| [registrations/distribution-and-signing.md](registrations/distribution-and-signing.md) | GitHub rulesets, updater key, AUR, Windows signing, Apple, domain |
| [registrations/relay-cloudflare.md](registrations/relay-cloudflare.md) | Cloudflare account, Workers, Durable Objects, `wrangler`, the optional relay |
| [go-live-requirements.md](go-live-requirements.md) | What you need to be able to **stream** on each platform, stream keys, safe testing |

Related planning docs: [QUESTIONS.md](../QUESTIONS.md) (open decisions), [platform-apis.md](../research/platform-apis.md), [mcp-and-ai-providers.md](../research/mcp-and-ai-providers.md), [tech-stack.md](../research/tech-stack.md), [PLAN.md](../../PLAN.md) (phases P0 to P6).

## Conventions in these files

- **Bold** = a button or field you click or fill in. `code` = a value to type exactly.
- `<PRODUCT_NAME>` = the final product name (open question **Q-B1**).
- `<SL_PORT>` = the local server port. It is chosen in spec 06 (`docs/spec/06-api-mcp.md`, not yet written). Use the placeholder in forms you must fill before then, and come back to edit the URL. Note that X, Kick and Patreon match redirect URLs **exactly**, so the app needs a **fixed callback port** (or you register several URLs). Spec 06 must account for that. Twitch Device Code Flow, Google Desktop clients and OpenAI "Sign in with ChatGPT" need no fixed port.
- `<DOMAIN>`, `<PROJECT_EMAIL>`, `<RELAY_HOST>`, `<CLIENT_ID>`: obvious from context.
- **(verify on screen)** = I could not confirm this label or behavior in an official source.
- "Secondary source" or "community" = a third-party guide or forum, not the vendor.
- Shell examples use `read -rs VAR` so secrets do not land in your shell history. Run them only on your own machine.

## General rules

1. **Use a dedicated project identity.** Create one project email address (a mailbox you control, ideally on your own domain once Q-B1 is settled) and use it for the developer-side accounts: GitHub, Google Cloud, Cloudflare, AUR, Apple, and the AI vendors. Keep your personal and streaming accounts separate from these where the service allows, because some registrations are tied to the account that created them (a Twitch Client ID, a Google Cloud project, an Apple Account Holder). Your own streaming channels (Twitch, YouTube, Kick, ...) are needed as *test targets*; they do not have to own the developer apps.
2. **Turn on 2FA everywhere, and keep recovery codes in the password manager.** Twitch will not let you register an app without 2FA, and Kick will not unlock its developer tools without it. GitHub, Google, Cloudflare, Apple (mandatory), Discord and X should all have it. Prefer a passkey or hardware key for GitHub, Google, Cloudflare and Apple.
3. **Secrets live in a password manager, never in the repo, chat, or an AI agent.**
   - Never paste a secret, token, key, stream key or webhook URL into the repository, a commit message, an issue, a chat window, or a prompt to an agent. Agents need only the **names** of variables, not values. Public identifiers (a Twitch Client ID, a Discord Application ID) are fine to share.
   - If you pasted one by mistake, treat it as leaked: rotate it immediately (each runbook says how) and delete the message if you can.
   - Check that `.env.local` and `.env*.local` are listed in `.gitignore` before you create the file.
4. **Where each copied value eventually goes.** The real storage design is decided in **P0** (and Q-A5 for Linux keychains). Until then, assume:

   | Kind of value | Development (your machine) | Released app (end users) |
   |---|---|---|
   | Public identifiers (Twitch Client ID, Discord App ID, X Client ID for a public client) | `.env.local`, or a config constant in the source for the one shipped ID | Same shipped constant |
   | Secrets you own (Kick client secret, AI keys, bot tokens, webhook secrets, stream keys) | `.env.local` (gitignored) or the app's **Settings > Connections** screen, which writes to the OS keychain | OS keychain through the Settings screen (Secret Service on Linux, or an encrypted file with a warning if there is no daemon: Q-A5) |
   | CI/release secrets (signing keys, notarization credentials, SSH key for the AUR, Cloudflare token) | Password manager only | GitHub Actions secrets (ideally in a protected `release` environment) |
   | Relay/Worker secrets | n/a | `wrangler secret put`, never in the repo |

   Variable names used in these files (`SL_TWITCH_CLIENT_ID`, `SL_KICK_CLIENT_SECRET`, ...) are suggestions. P0 decides the real names and location.
5. **Do not register OAuth apps until the product name (Q-B1) is final.**
   - The app name appears on every consent screen (Twitch Connections, Google, Kick, X, Discord), and users will see it.
   - A **Twitch Client ID is permanent**, and custom channel-point rewards can only be managed by the Client ID that created them. Changing it later forces every streamer to recreate their rewards.
   - Renaming a Google OAuth app after verification means re-verification.
   - Package names (AUR, crates) and the domain are baked in at P0.
   - Q-B1's own recommendation is to keep "StreamDen" as the working title and check the domain/trademark before P0 ends. Answer Q-B1 first, then register. Things that do **not** depend on the name are safe to do now (see the order below).
6. **One registration per purpose.** Do not share one Client ID across different applications (Twitch can suspend it). Record every app in a "registrations" secure note in your password manager: service, owning account, creation date, Client ID, redirect URLs, console URL.
7. **Set spending limits when you create paid accounts.** OpenAI, Anthropic, xAI and X all let you cap spend. Do it at creation, with small credit amounts (about $5 to $10) while testing.
8. **Check scopes and permissions before clicking Authorize** on your own apps. Keep your own channels' sensitive scopes (like `streamkey:read` on Kick) out of anything you do not need.
9. **Test on throwaway targets first**: a test Discord server, a second Twitch/Kick account, unlisted YouTube streams. See [go-live-requirements.md](go-live-requirements.md).

## Master checklist

Phases are from [PLAN.md](../../PLAN.md): P0 Foundations, P1 Core (Twitch, v0.1), P2 Chat + TTS (YouTube, Kick, X posting, Discord, relay), P4 Integrations/SDK/AI, P5 Decks + donation services, P6 Ecosystem. "Blocked by decision" refers to [QUESTIONS.md](../QUESTIONS.md). All are still open at the time of writing.

| # | Service | Why we need it | Needed by phase | Cost | Blocked by decision | Status |
|---|---|---|---|---|---|---|
| 0 | Project identity: email, password manager, authenticator/passkeys, 2FA | Foundation for every account below | P0 (now) | Free (KeePassXC/Bitwarden free tier) | none | ☐ |
| 1 | GitHub: account, repo, branch ruleset, Actions secrets ([distribution](registrations/distribution-and-signing.md)) | Code hosting, CI, multi-agent PRs, releases, updater endpoint | P0 | Free if public. Private repos need Pro/Team (about $4/mo, verify) for branch protection | Q-B4 (visibility), Q-B1 (repo name) | ☐ |
| 2 | Tauri updater key + GPG key ([distribution](registrations/distribution-and-signing.md)) | Signed auto-updates; signed RPM and checksums | P0 | Free | none (but back them up) | ☐ |
| 3 | Domain ([distribution](registrations/distribution-and-signing.md)) | Homepage and privacy policy (Google, Twitch, Meta, SignPath need them), docs, relay hostname | P0 to P2 | about $10-20/year (verify at registrar) | Q-B1 | ☐ |
| 4 | AUR account + `-bin` package ([distribution](registrations/distribution-and-signing.md)) | Arch distribution | P0 (account), first push at v0.1 | Free | Q-B1 (name), Q-B2 (license) | ☐ |
| 5 | Windows code signing: SignPath Foundation or Azure Artifact Signing ([distribution](registrations/distribution-and-signing.md)) | Avoid SmartScreen warnings | Before the first public release (v0.1) | SignPath free for OSS; Azure Basic $9.99/mo | Q-B2 (OSI license), Q-B4 (public repo), Q-G3 (budget) | ☐ |
| 6 | Apple Developer Program ([distribution](registrations/distribution-and-signing.md)) | Developer ID signing + notarization | P0 build, signing at v0.5 (or earlier if Q-P2 says so) | $99/year | Q-P2, Q-G3 | ☐ |
| 7 | Twitch developer app ([twitch](registrations/twitch.md)) | All Twitch features: EventSub, chat, moderation, rewards | P1 | Free | Q-B1 | ☐ |
| 8 | Stream-ready test accounts and eligibility ([go-live](go-live-requirements.md)) | Test against real platforms; some have waiting periods | P1 onward (start the clocks now) | Free; X Premium is paid | Q-S1 (which platforms) | ☐ |
| 9 | Google Cloud + YouTube Data API ([google-youtube](registrations/google-youtube.md)) | YouTube chat, events, moderation | P2 | Free; quota 10,000 units/day | Q-B1, Q-C2, Q-S1; domain for verification | ☐ |
| 10 | Kick developer app ([kick](registrations/kick.md)) | Kick chat send, moderation, events (webhooks) | P2 | Free | Q-B1, Q-C1 (webhooks/secret), Q-S1, Q-C3 | ☐ |
| 11 | Cloudflare account + `wrangler` ([relay](registrations/relay-cloudflare.md)) | Optional self-hostable relay for Kick events and webhook-only donation services | P2 | Free tier (Workers Free + Durable Objects SQLite) | Q-C1 | ☐ |
| 12 | X developer app + credits ([x-twitter](registrations/x-twitter.md)) | Post go-live and clip tweets | P2 | Pay-per-use: $0.015 per post, $0.20 per post with a link; buy about $5-10 to test | Q-B1, Q-C4 | ☐ |
| 13 | Discord: webhook and (optional) bot application ([discord](registrations/discord.md)) | Go-live posts (webhook, no app) and optional chat bridge (bot) | P2 | Free | Q-B1 (bot name), Q-S1 | ☐ |
| 14 | OpenAI API key + "Sign in with ChatGPT" ([openai](registrations/openai.md)) | In-app assistant and AI chat responder; plan-usage sign-in | P4 | Pay-as-you-go (start with about $10). SIWC plan usage: no registration for open-source/local apps | Q-A1; Q-B2 and Q-B3 for SIWC eligibility | ☐ |
| 15 | Anthropic API key ([anthropic](registrations/anthropic.md)) | Claude as a provider (API key only; no subscription sign-in) | P4 | Pay-as-you-go | Q-A1 | ☐ |
| 16 | xAI API key ([xai](registrations/xai.md)) | Grok as a provider | P4 | Prepaid credits (auto top-up minimum $5) | Q-A1 | ☐ |
| 17 | Streamlabs socket token ([donations](registrations/streaming-services-donations.md)) | Tips and alerts | P5 | Free | none | ☐ |
| 18 | StreamElements JWT ([donations](registrations/streaming-services-donations.md)) | Tips and alerts | P5 | Free | none | ☐ |
| 19 | Ko-fi webhook ([donations](registrations/streaming-services-donations.md)) | Ko-fi events | P5 | Free | Q-C1 (needs relay) | ☐ |
| 20 | Patreon client ([donations](registrations/streaming-services-donations.md)) | Member/pledge events | P5 | Free | Q-C1 for webhooks; polling needs none | ☐ |
| 21 | Throne, Fourthwall webhooks ([donations](registrations/streaming-services-donations.md)) | Gifts, shop orders | P5 | Free | Q-C1 | ☐ |
| 22 | Rumble Live Stream API URL ([rumble](registrations/rumble.md)) | Rumble followers, subs, rants, chat | P5 | Free | Q-S1 | ☐ |
| 23 | Meta developer app ([facebook](registrations/facebook.md)) | Facebook Live comments | P5 or later | Free; Business Verification effort for public use | Q-S1 | ☐ |
| 24 | TikTok ([tiktok](registrations/tiktok.md)) | Nothing official exists to register | P6, optional plugin | Free (third-party signer has paid tiers) | Q-S1 | ☐ |

Not covered yet (write a runbook when the feature enters scope): ElevenLabs/Azure/other TTS keys (Q-T1), Spotify client ID (Q-I3), Tiltify/Buy Me a Coffee/Tipeeestream/DonorDrive, Elgato Marketplace (Stream Deck plugin), a Twitch Extension (Q-S6), Flathub (P6), winget, the plugin registry GitHub org (D-008).

## Recommended order

| When | Do | Why now |
|---|---|---|
| **Today (no decisions needed)** | Row 0: project email, password manager, authenticator/passkeys. | Everything else depends on it. |
| **Today** | Start the slow clocks from [go-live-requirements.md](go-live-requirements.md): enable YouTube live streaming (up to 24 h), enable Twitch 2FA, get Kick/Rumble accounts ready, check your Twitch Affiliate progress. | Calendar time, not effort. |
| **Today** | Row 2: generate the Tauri updater key and a GPG key and back them up. Row 1: GitHub account + 2FA. Cloudflare account (row 11, account only). AUR account (row 4, account and SSH key only). | None of these need the name. |
| **Any time (optional, cheap)** | Rows 14-16: AI API keys with small credits and spend limits. | They do not carry the product name. Not needed until P4. |
| **Answer the questions** | **Q-B1 first** (name), then Q-B2 (license), Q-B4 (GitHub visibility), Q-P2 (macOS), Q-C1 (relay), Q-C2 (YouTube project), Q-C4 (X), Q-S1 (platform scope), Q-G3 (budget). See "Decisions that block registrations" below. | Unlocks the rest. |
| **After Q-B1** | Row 3: domain. Row 1: create the repo under the final name. Row 4: reserve the AUR package name (first push later). | Names get baked in at P0. |
| **Start of P1** | Row 7: the Twitch app. Then smoke-test it with the device-flow `curl` commands. | P1 needs it. Twitch is certain. |
| **Before the first public release** | Row 5: Windows signing (SignPath needs a public repo and an OSI license; apply once there is a release). Row 6 only if Q-P2 says (a). | Needs a release to apply. |
| **P2** | Row 9 (Google/YouTube), 13 (Discord), 10 (Kick, dev app only), 12 (X), 11 (relay deploy), in that order. | Matches the P2 deliverables. |
| **P4** | Rows 14-16 finishing touches: SIWC testing with a Plus/Pro account. | AI layer. |
| **P5 and later** | Rows 17-24 only for services you actually use. | Donation services, Rumble, Facebook. |

## Decisions that block registrations

| Decision | Blocks |
|---|---|
| **Q-B1 product name** | Every OAuth app (Twitch, Google, Kick, X, Discord, Patreon, Meta), the repo name, AUR package name, domain, privacy-policy text, consent-screen branding. **Do not register OAuth apps before this.** |
| Q-B2 license | SignPath Foundation (needs an OSI license without commercial dual-licensing), the AUR `license=` field, OpenAI SIWC eligibility ("open-source") |
| Q-B3 monetization | Whether SIWC needs OpenAI's paid/hosted waitlist approval |
| Q-B4 git hosting/visibility | GitHub plan (branch protection on private repos is paid), SignPath (public repo), free Actions minutes |
| Q-C1 relay | Cloudflare deployment, Kick webhooks, Ko-fi, Throne, Fourthwall, Patreon webhooks, Kick token brokering |
| Q-C2 YouTube project | Shared project (needs Google verification and a quota audit) or only the end-user wizard |
| Q-C4 X | Whether you bother registering an X app at all |
| Q-P2 macOS tier | Apple Developer Program purchase |
| Q-S1 platform scope | Which of Kick, Rumble, Facebook, TikTok, Discord runbooks you actually run |
| Q-A1 AI subscriptions | Which AI accounts you create and whether to contact Anthropic/xAI |
| Q-G3 budget | Apple, Azure signing, domain, credits |

## Handing results back to the agents

After each registration, tell the planning agent only the public facts: "Twitch app `<PRODUCT_NAME>` created, Public client, Client ID is `abc...` (public), redirect `http://localhost:<SL_PORT>/auth/twitch/callback`". Never the secrets. Agents record the public IDs in the specs and code; you keep the rest in your password manager and local `.env.local`.
