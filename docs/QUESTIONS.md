# Open Questions for the Owner

How this file works:
- Each question has an ID, the options, and the planning agent's **recommendation**. The owner writes the
  **Answer**, and the answer is copied as one line into [`decisions.md`](decisions.md) (plus an ADR if it's
  architectural).
- Agents: **never answer these yourselves.** Add new questions at the end of the right section with the next
  free ID, options and a recommendation, and keep working on unblocked parts.
- **Round** = the Q&A round where it was or will be asked. Round 1 = 2026-10-18.

Status legend: 🟥 Open · 🟨 Answered (pending ADR) · 🟩 Decided (in decisions.md)

---

## B — Product and business

### Q-B1 — Product name 🟩 (Round 1)
The working title was "Streamer.live". Research found it was already used by an open-source project
(`getstreamer.live`), that `streamer.live` redirects to Switchboard Live, and a medium–high confusion risk with
Streamer.bot. Candidates offered: Streamwright, NekoStage, StreamDen, TanukiLive.
Research: [branding-and-distribution.md](research/branding-and-distribution.md).
**Answer:** **StreamDen** (D-015, ADR-0004). Tech Daddy Digital is a legally registered entity and owns the
`tech-daddy-digital` GitHub org. Still open: a domain (`techdaddydigital.com` HTTPS is broken), not blocking
until P6.

### Q-B2 — License 🟩 (Round 1)
Options: (a) **GPL-3.0-or-later** for the app, with **MIT/Apache-2.0** for SDKs, protocol schemas and plugin
templates; (b) MIT/Apache-2.0 for everything; (c) source-available or closed.
Considerations: Piper/Kokoro voices pull in GPL-3.0 espeak-ng. Under (b) those voices would need a separate
optional download. Free code signing (SignPath) needs an OSI license. Firebot and Social Stream Ninja are GPL,
but we're clean-room either way. A permissive SDK lets plugin authors pick any license.
**Recommendation:** (a).
**Answer:** GPL-3.0-or-later app + MIT/Apache-2.0 SDKs (D-016, ADR-0003).

### Q-B3 — Monetization 🟩 (Round 2)
Fully free and OSS? Or supporter perks like Streamer.bot (cloud backups, hosted relay) and SAMMI Pro? Or paid
plugins in the registry?
**Recommendation:** free and OSS app. Optionally a donation-funded hosted relay later. No feature paywalls in
the core.
**Answer:** Free and open source; hosted relay funded by donations (D-032).

### Q-B4 — Git hosting and visibility 🟩 (Round 1)
Initialize git now? Host on GitHub? Public from day one, or private until v0.1? The multi-agent workflow
(worktrees, branches, PR reviews, `@codex review`) needs git right away, and a GitHub remote for PR-based review.
**Recommendation:** `git init` now. Then either (a) **public from day one** (free branch protection and
unlimited Actions minutes; ideas are visible early), or (b) private until v0.1 on **GitHub Pro (about $4/mo)**.
A free account's private repos have no branch protection (which the multi-agent PR flow relies on) and only
2,000 Actions minutes a month, which a five-OS build matrix uses up fast. Leaning (a) if the license is open
source.
Repo under a **Tech Daddy Digital** GitHub organization (free) rather than a personal account.
**Answer:** git now; **public** GitHub repo from day one under personal **TechDaddyKB**, transferred to the `tech-daddy-digital` org at release (D-017, ADR-0004).

### Q-B5 — Release on Steam? 🟥 (Round 2)
GitHub Releases and itch.io are planned. Steam pros: discoverability among VTubers/streamers, auto-updates,
Linux/Steam Deck, Workshop for sharing overlays and plugins. Cons: $100 fee, the Steam Linux Runtime container,
GPL vs Steamworks SDK, two update channels. Research: [branding-and-distribution.md](research/branding-and-distribution.md).
More findings: Valve says it is "primarily accepting games". Steam needs a legal entity (no DBAs). Steam's Linux
runtime ships no WebKitGTK, so a Linux depot is hard. No competing automation tool appears to be on Steam.
**Recommendation:** at launch, GitHub Releases + itch.io + AUR + winget, with GitHub Releases as the single
update source (the in-app updater is compiled out of store and package builds). Steam after v1.0: free,
Windows first, no Steamworks SDK (like OBS).
**Answer:**

---

## P — Platforms and OS support

### Q-P1 — Linux targets ✅ (given)
Debian/Ubuntu `.deb`, Fedora `.rpm`, Arch (AUR `-bin` + source), plus AppImage. Wayland and NVIDIA must work.

### Q-P2 — macOS tier 🟩 (Round 1)
(a) Tier 1 at launch: $99/yr Apple Developer account, notarized builds in CI. (b) Tier 2: builds in CI,
unsigned, best-effort. (c) Later.
**Recommendation:** (b) until v0.5, then (a) if there's demand. The architecture stays macOS-clean either way.
**Answer:** Accepted default: unsigned best-effort until v0.5 (D-024).

### Q-P3 — Flatpak at launch? 🟥 (Round 2)
**Recommendation:** no, P6. The sandbox complicates system Python/.NET, local ports, MIDI and input
simulation.
**Answer:**

---

## S — Scope

### Q-S1 — Streaming platforms for v0.x 🟩 (Round 1)
Twitch (P1) is certain. Which others are in scope, and in which phase? YouTube, Kick, X (posting), TikTok
(no official API; the unofficial libraries are AGPL and against ToS), Facebook (needs App Review), Rumble, Discord.
Trovo shut down live streaming on 2026-06-30.
**Recommendation:** Twitch (P1); YouTube, Kick, X posting, Discord (P2); Rumble and Facebook (P5); TikTok as
an optional community plugin, not core.
**Answer:** Accepted default, no objection (D-024).

### Q-S2 — Streamer.bot alpha-only features count toward parity? 🟥 (Round 2)
Points/currency, Cron triggers, game-controller input, Whisper voice control, new Elgato apps, OBSBOT.
**Recommendation:** yes. Points and Cron in P5; controller and voice in P5.
**Answer:**

### Q-S3 — Built-in loyalty (points, currency, ranks, cross-platform identity merge)? 🟥 (Round 2)
**Recommendation:** yes, P5, including linking one viewer across Twitch, YouTube and Kick.
**Answer:**

### Q-S4 — Speech-to-text and voice commands in v1? CPU-only or GPU? 🟥 (Round 2)
**Recommendation:** P5. CPU by default with 1–2 threads so games aren't affected; GPU optional.
**Answer:**

### Q-S5 — SAMMI-style OBS source/filter animation (easing, curves)? 🟥 (Round 2)
**Recommendation:** yes, P5, as sub-actions using obs-websocket.
**Answer:**

### Q-S7 — Discord companion scope 🟥 (Round 2)
Research: [discord-companion.md](research/discord-companion.md).
- 🟢 Easy, no relay needed: go-live embed that updates with the VOD link at stream end, slash commands and
  buttons that run actions, polls, schedule → Discord Events, clip posting, mod-log mirroring, reaction and voice
  triggers, replies from the multichat, bot "Streaming" status, and webhook-only mode.
- 🟡 Doable: Discord channel inside the multichat, plus join/boost triggers (need "privileged intents", a simple
  toggle under 10k members), cross-platform role sync and loyalty (need account linking), TTS into a voice
  channel (Discord voice encryption, required since 2026-03, is still a bit buggy in libraries), voice-activity
  overlay through a bot.
- 🔴 Hard or later: Linked Roles (need the relay), recording voice.

Hosting: **bring your own bot** (you create a bot in Discord's developer portal; runbook provided). It runs while
the app is open, or 24/7 in the headless daemon.
Questions: (1) Bring-your-own only for v1? (2) Build it as a first-party *plugin* (dogfooding the SDK) in
TypeScript/discord.js (needs Node or a compiled single binary) or Rust (twilight)? (3) Is voice TTS worth it in
v1? (4) Is your Discord over 10,000 members?
**Recommendation:** webhook announcements in core (P2). The full bot as a first-party plugin in P4,
bring-your-own only, no voice TTS until the Discord voice libraries stabilize.
**Answer:**

### Q-S6 — Viewer-facing Twitch Extension (viewer-pressable buttons, Bits)? 🟥 (Round 3)
**Recommendation:** P6. It needs a hosted backend (EBS), which conflicts with local-first.
**Answer:**

---

## C — Cloud, relay and third-party accounts

### Q-C1 — Relay stance 🟩 (Round 1)
Kick events and Kick OAuth, Ko-fi, Throne, Fourthwall and Shopify webhooks all need a **public URL** or a
**server-held secret**. Options: (a) fully local, users bring their own tunnel (Tailscale Funnel/ngrok);
(b) **we ship an open-source, self-hostable relay** (Cloudflare Worker, free tier) plus a one-click deploy guide;
(c) (b) plus **we run an official hosted relay** (costs money, ops, privacy policy).
**Recommendation:** (b) in P2. Decide on (c) later. The relay is a "dumb pipe": signatures are verified on the
desktop, and it never stores tokens.
**Answer:** Self-hostable relay **plus** an official hosted relay later (D-019).

### Q-C2 — YouTube Google project 🟩 (Round 2)
Shared project (one 10k-unit/day quota for *all* users, and Google app verification needed) or each user brings
their own (a wizard walks them through it)? Is a scraping "unofficial mode" acceptable (it breaks YouTube's
policies)?
**Recommendation:** bring-your-own with a guided wizard as the default, an optional shared project, and no scraping
in core (could be a community plugin).
**Answer:** Bring-your-own Google project via wizard; shared project optional; no scraping in core (D-031).

### Q-C3 — Kick unofficial chat WebSocket fallback? 🟥 (Round 2)
It's the only way to get deleted-message sync, but Kick staff call it unsupported.
**Recommendation:** read-only fallback behind an "experimental" toggle.
**Answer:**

### Q-C4 — X/Twitter 🟥 (Round 2)
X moved to pay-per-use in February 2026 (a tweet with a link costs about $0.20). Users would register their own X
developer app (OAuth 2.0 PKCE). Is that acceptable? Should we add **Bluesky** (free, open API) and **Threads**
too? Note: Streamer.bot has never had a native X integration; the "online path" you remembered is most likely
Streamer.bot's cloud relay (used for Kick, Ko-fi, Patreon, IFTTT…).
**Recommendation:** bring-your-own X app in P2, with Bluesky alongside (cheap to add).
**Update (owner, 2026-10-18):** "X should use the streamer's own account." The official form of that is a
**bring-your-own X developer app with OAuth** on your own account. Reading your own followers and mentions costs
about $0.001 each; feasible triggers are new follower, mention, reply, like or repost of the go-live post, and DMs.
Polling costs cents per stream. To avoid the $0.20 link-tweet fee, use X's free "web intent" (it opens a
prefilled tweet). Logging in with your browser session or cookies breaks X's terms (risk of suspension), so we
won't. X Live chat has no usable API.
**Answer:**

### Q-C6 — Venmo / Cash App / X Money tips 🟥 (Round 2)
Research: [payments-x-bluesky.md](research/payments-x-bluesky.md).
- **There's no official way to detect money received** in personal Venmo or Cash App accounts, and X Money has
  no API.
- The official routes are PayPal Business, Stripe (works locally, no relay) and Square Cash App Pay (needs the
  relay).
- The unofficial *local* option watches payment-notification **emails** (IMAP) or phone notifications. It's
  fragile, can be spoofed without DKIM checks, and can't detect reversals. Venmo and Cash App terms frown on
  using personal accounts for commercial purposes.

**Recommendation:**
- Core: Stripe + PayPal Business.
- An **experimental, off-by-default** plugin for email and notification tips, with a "verified / reported /
  unverified" label on every tip, and a manual "fire tip alert" button.
- Never log in to or scrape payment apps.

Also: a dedicated tips mailbox? Regions beyond the US?
**Answer:**

### Q-C7 — Bluesky, Threads, Mastodon 🟥 (Round 2)
Bluesky is free and works locally: posting, the "Live Now" badge (Twitch/YouTube links only, not Kick), and
follow/like/repost/mention/reply triggers by polling notifications. DMs are possible.
**Recommendation:** Bluesky in P2 alongside X. Threads and Mastodon as go-live announcement plugins later.
App password or OAuth as the default Bluesky login?
**Answer:**

### Q-C5 — Cloud backups? 🟥 (Round 3)
**Recommendation:** local backups plus "export to a folder" (users can point it at Dropbox, Syncthing or Nextcloud).
No hosted backups.
**Answer:**

---

## X — Compatibility and import

### Q-X1 — How deep is Streamer.bot compatibility? 🟩 (Round 1)
Pick any: (a) import Streamer.bot share strings (actions, triggers, sub-actions; the format is
`Base64("SBAE"+gzip(JSON))`); (b) a **wire-compatible Streamer.bot WebSocket API**, so existing overlays,
docks and `@streamerbot/client` work unchanged; (c) a C# sidecar with a `CPH`-compatible facade so C# blocks
still run (needs the .NET 10 runtime); (d) a `CPH`-like facade in JS/Python to ease porting; (e) none.
**Recommendation:** (a) and (b) in P5, (d) cheap and early, (c) in P6 as an optional download.
**Answer:** Share-string import + compatible WebSocket API + JS/Python helpers now; C# runner later (D-020).

### Q-X2 — Other imports 🟥 (Round 2)
SAMMI decks, Speaker.bot settings and voice aliases, Touch Portal `.tpp` plugin compatibility, StreamElements
widget import.
**Recommendation:** Speaker.bot settings (P2), SAMMI decks (P5). Skip `.tpp` unless demand appears.
**Update 2026-10-18:** StreamElements widget compatibility is now a goal (D-012) and is proposed for **P2**,
so chat overlays work before our own designer ships in P3. Streamlabs custom chat-box compatibility is a
possible follow-up.
**Answer:**

### Q-X3 — Variable and template syntax 🟩 (Round 2)
Keep Streamer.bot's `%var%` / `~global~` / `$fn()$` as-is? Use a new typed expression language (e.g.
`{user.name}`, `{amount | currency}`)? Or new syntax plus a compatibility mode?
**Recommendation:** new `{…}` syntax as canonical, with `%var%` accepted on import and in a compat mode.
**Answer:** New `{…}` syntax canonical; `%var%` accepted and converted on import (D-027).

---

## R — Scripting

### Q-R1 — Python runtime provisioning 🟩 (Round 1)
(a) Use system `python3` first, offer a managed download (about 25–35 MB) if missing; (b) always managed;
(c) system only.
**Recommendation:** (a).
**Answer:** Accepted default: system Python first, managed download as fallback (D-024).

### Q-R2 — Trust model for user scripts 🟩 (Round 2)
Your own scripts get full access (like Streamer.bot C#), and imported or community scripts get a
capability-limited sandbox with a consent screen. OK?
**Recommendation:** yes. Python is "robust" (time and memory limits, crash isolation), not a security sandbox.
The UI says so.
**Answer:** Own scripts full access; imported/community scripts sandboxed with consent (D-029).

### Q-R3 — Embedded JS plugins have no Node.js APIs 🟥 (Round 2)
Plugins needing Node would be sidecars. Acceptable?
**Recommendation:** yes.
**Answer:**

---

## U — UI and UX

### Q-U1 — UI framework 🟩 (Round 1)
React 19 (most AI-agent training data, largest ecosystem), Svelte 5 (smaller and fast, but agents often write
outdated Svelte 4 syntax), or Solid. The plugin UI kit must use the same framework, exposed as web components.
**Recommendation:** **React + TypeScript + Vite**, with the overlay runtime in vanilla TS/Preact (light inside
OBS).
**Answer:** React + TypeScript (D-018).

### Q-U2 — Automation editor style 🟩 (owner input 2026-10-18)
The owner wants a **node-based editor** (frequently requested by Streamer.bot users) as well as the list style.
**Proposal:** one structured action model with **two editable views that stay in sync**: the Streamer.bot-style
list/tree (P1) and a node graph (P3). Decks are a third view (P5). The graph uses React Flow (`@xyflow/react`,
MIT) plus elkjs auto-layout. Its control flow is structured (If/Switch/While/Group own their blocks), and
plugin sub-actions become nodes automatically from their schema. Both views share one document and one undo
stack.
Remaining questions:
1. Should the node editor move up to P1/P2? It's a lot of UI work early on.
2. Should the engine add a **Parallel** block, so the graph can fan out and merge?
3. Which view is the default for new users? (Recommendation: list by default, remembering the user's last choice.)
Research: [se-widget-compat-and-node-editor.md](research/se-widget-compat-and-node-editor.md).
**Answer:** List view first (P1); node graph in P3 as a synchronized second view; Parallel block added to the engine (D-028).

### Q-U3 — Visual direction 🟥 (Round 2)
"Like Streamer.bot" (dark, dense, panel-based)? Any brand colors, references or things to avoid?
**Answer:**

### Q-U4 — Localization at launch? 🟥 (Round 3)
**Recommendation:** i18n-ready strings from day one; English only until P6.
**Answer:**

### Q-U5 — Native chat window on Wayland ✅ (proposed default)
OBS disables custom browser docks on native Wayland (Hyprland included), so we ship a native always-on-top
chat window *and* an OBS dock URL.

---

## N — Network

### Q-N1 — Local-only, or LAN mode for two-PC rigs and the phone deck? 🟩 (Round 1)
Many VTubers run the avatar app on one PC and the bot on another. The phone deck also needs LAN access.
**Recommendation:** localhost by default. **Opt-in LAN mode** with pairing tokens in P4. Every connector
accepts a remote host, not just `localhost`.
**Answer:** Accepted default: localhost by default, opt-in LAN mode in P4 (D-024).

---

## D — Designer and alerts

### Q-D1 — Alert and TTS audio routing 🟥 (Round 2)
Play alert sounds through the OBS browser source (captured by OBS), the app's own audio device, or user's choice
per alert?
**Recommendation:** user's choice, defaulting to the browser source.
**Answer:**

### Q-D2 — Alert queue defaults 🟥 (Round 2)
How many priority lanes, should higher priority interrupt, and should gift bombs merge into one alert?
**Recommendation:** three lanes (critical, normal, low), no interruption by default, gift bombs merged.
**Answer:**

### Q-D4 — StreamElements widget runtime defaults 🟥 (Round 2)
Research: [se-widget-compat-and-node-editor.md](research/se-widget-compat-and-node-editor.md). Estimate: about
85–90% of Etsy SE chat widgets run unmodified for Twitch, and 70–80% show YouTube/Kick messages acceptably.
Context: SE announced a shutdown in May 2026, reversed it, and Razer bought its assets on 2026-07-31, so
streamers are looking for an exit.
Proposed defaults:
- each widget runs on its **own local origin** (it needs local storage; to be verified in OBS during spike S-2);
- **network access on** (most widgets load fonts and CDN libraries), with a per-widget off switch;
- messages report their **real platform**, with a per-widget "present as Twitch" toggle;
- session totals **combined across platforms**;
- widget TTS calls go to **our local TTS**;
- an optional **one-time import from your SE account** to rescue share-link-only purchases;
- Streamlabs custom-widget compatibility in **P3**;
- a native widget package format for sellers in **P6**.

Imported widget code is **never** uploaded, cloud-synced or sent to AI without consent (seller licenses are
personal-use).
**Answer:**

### Q-D3 — Real-time co-editing with mods? 🟥 (Round 3)
**Recommendation:** not in v1. Yjs underneath keeps it possible.
**Answer:**

---

## T — TTS

### Q-T1 — TTS engines at launch 🟩 (Round 2)
Local: Piper, Kokoro (via sherpa-onnx), OS voices (SAPI/OneCore, AVSpeech, speech-dispatcher). Cloud: Azure,
Polly, Google, ElevenLabs, OpenAI, TTS Monster…
**Recommendation:** P2 = Piper + Kokoro + OS voices + ElevenLabs + Azure. The rest are plugins.
**Answer:** Piper + Kokoro + OS voices + ElevenLabs + Azure at launch (D-033).

---

## I — Integrations

### Q-I1 — Is a small C# companion plugin for Warudo acceptable? 🟥 (Round 2)
Warudo mods must be C#. It would be a tiny, separately built plugin.
**Recommendation:** yes.
**Answer:**

### Q-I2 — Wayland keystroke simulation 🟥 (Round 2)
Portal-only (prompts the user, partly supported on Hyprland), or also ship a `ydotool`-style fallback that needs
a udev rule?
**Recommendation:** portal first, with an optional fallback and a guided setup.
**Answer:**

### Q-I3 — Spotify 🟥 (Round 2)
Since February 2026, apps in Spotify's development mode need the owner to have Premium, and allow 5 users.
Options: users supply their own client ID, or skip and use OS "now playing" (MPRIS/Windows media session).
**Recommendation:** OS now-playing in core; Spotify as a bring-your-own-client-ID plugin.
**Answer:**

### Q-I4 — Unofficial/reverse-engineered APIs (Wave Link, SteelSeries Sonar…)? 🟥 (Round 2)
**Recommendation:** only behind an "experimental" toggle.
**Answer:**

### Q-I5 — Stream Deck approach 🟥 (Round 2)
Our own Elgato plugin (also runs on OpenDeck on Linux), direct hardware control, or both?
**Recommendation:** an Elgato SDK plugin that is OpenDeck-compatible (P4). Direct HID control later, if ever.
**Answer:**

### Q-I6 — Home Assistant as the official smart-home bridge? 🟥 (Round 2)
**Recommendation:** yes for the long tail. Native Hue, Elgato Key Light and WLED for the most common devices.
**Answer:**

### Q-I7 — Unknowns to confirm 🟥 (Round 2)
What is "Karasubot"? Is PolyPop still used by your community? Any other tools you or your VTuber friends rely
on that aren't in [app-integrations-vtuber.md](research/app-integrations-vtuber.md)?
**Answer:**

---

## K — Plugins and registry

### Q-K1 — Native sidecar plugins in the public registry? 🟥 (Round 3)
**Recommendation:** only in a "Verified" channel with build provenance. Declarative and JS plugins in the open
channel.
**Answer:**

### Q-K2 — Paid or DRM plugins ever? 🟥 (Round 3)
**Recommendation:** the registry only lists free plugins. Paid plugins can exist outside it.
**Answer:**

### Q-K3 — Registry governance 🟥 (Round 3)
Who reviews submissions, the namespace policy, takedown process.
**Answer:**

---

## A — AI and MCP

### Q-A1 — Using Plus/Pro/Max subscriptions 🟩 (Round 1)
Findings ([mcp-and-ai-providers.md](research/mcp-and-ai-providers.md)):

| Vendor | Own API key in our app | Subscription sign-in *inside* our app | Their app connects to *our* MCP server |
|---|---|---|---|
| OpenAI | ✅ | ✅ "Sign in with ChatGPT" plan usage (open-source/local apps; hosted/paid apps need waitlist approval) | ✅ Codex CLI, ChatGPT desktop |
| Anthropic | ✅ | ❌ prohibited by Anthropic's terms unless Anthropic approves | ✅ Claude Code, Claude Desktop |
| xAI | ✅ | ⚠️ gray area; no published program | ✅ Grok Build |

Options: (a) API keys plus "Connect your AI" over MCP only; (b) (a) plus ChatGPT-plan sign-in for OpenAI
users in v1; (c) (b) plus asking Anthropic and xAI for an approved subscription path.
**Recommendation:** (b). Contact Anthropic and xAI later (c). Never read another tool's stored credentials.
**Answer:** API keys + MCP "Connect your AI" + ChatGPT-plan sign-in (D-021).

### Q-A2 — MCP server defaults 🟩 (Round 2)
Off by default? Default tool profile: *Lean* (about 20 core tools plus search/describe/invoke meta-tools) or
*Full*? Which tools need in-app approval (ban, delete action, post to X)? Should the most dangerous tier
(run code, end stream) ever be reachable over MCP?
**Recommendation:** off by default and enabled in one click from "Connect your AI". Lean profile by default.
AI-created config is saved as a *disabled draft* with a diff. Destructive tools need approval inside
StreamDen. Code execution and ending the stream are never exposed over MCP.
**Answer:** Off by default, Lean toolset, approvals for destructive tools, never expose code execution or ending the stream (D-030).

### Q-A4 — Remote AI access (ChatGPT web, claude.ai, grok.com) 🟥 (Round 3)
These need a public URL (tunnel) plus OAuth.
**Recommendation:** after v1. Local clients cover most needs.
**Answer:**

### Q-A5 — Secrets on Linux without a keychain daemon 🟥 (Round 2)
Some Hyprland setups have no Secret Service daemon. Fall back to an encrypted file (passphrase), or require
gnome-keyring/KeePassXC?
**Recommendation:** use Secret Service if present, otherwise an encrypted file with a clear warning in the UI.
**Answer:**

### Q-A3 — AI in chat 🟥 (Round 2)
AI co-host replies, chat summaries, moderation assist? Should replies auto-post or go to an approval queue? Which
providers can power it (local models too)? Disclosure (the EU AI Act Art. 50 transparency duty has applied since
2026-08-02, per secondary sources)? Budget caps per stream?
**Recommendation:** v0.5. The responder has **no write tools**, so viewers can't talk it into running actions.
Replies pass moderation, the approval queue is the default, disclosure is on, and there's a hard budget cap.
**Answer:**

---

## H — Data

### Q-H1 — Chat history retention 🟥 (Round 3)
Twitch and YouTube developer policies limit how long chat data is cached (about 24 h and 30 days).
**Recommendation:** keep a rolling 24 h by default, configurable, and keep only aggregate stats beyond that.
**Answer:**

---

## G — Process and resources

### Q-G1 — Which AI agent tools and subscriptions do you have? 🟩 (Round 1)
Claude (Pro/Max, or API)? ChatGPT Plus/Pro with Codex? SuperGrok or API with Grok Build? This decides how
work gets routed in [DELEGATION.md](agents/DELEGATION.md) and how much can run in parallel.
**Answer:** Claude Max, ChatGPT Pro (Codex), SuperGrok + Grok Build, launched via codex-orchestrator, whose board holds the work items (D-023, D-026).

### Q-G2 — Approve the Day-1 spikes (S-1…S-5)? 🟩 (Round 1)
Throwaway code in `spikes/`, mainly to test Tauri on your Hyprland + NVIDIA machine before locking the stack.
**Recommendation:** yes. It's the cheapest way to de-risk the biggest unknown.
**Answer:** Approved; start once name/license/git are settled (D-022). They are now settled.

### Q-G3 — Budget for paid items 🟥 (Round 2)
Apple Developer ($99/yr), Windows signing (SignPath free for OSS; Azure Trusted Signing about $10/mo), domain,
Cloudflare (relay free tier), test accounts.
**Answer:**
