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

### Q-B5 — Release on Steam? 🟩 (Round 2)
GitHub Releases and itch.io are planned. Steam pros: discoverability among VTubers/streamers, auto-updates,
Linux/Steam Deck, Workshop for sharing overlays and plugins. Cons: $100 fee, the Steam Linux Runtime container,
GPL vs Steamworks SDK, two update channels. Research: [branding-and-distribution.md](research/branding-and-distribution.md).
More findings: Valve says it is "primarily accepting games". Steam needs a legal entity (no DBAs). Steam's Linux
runtime ships no WebKitGTK, so a Linux depot is hard. No competing automation tool appears to be on Steam.
**Recommendation:** at launch, GitHub Releases + itch.io + AUR + winget, with GitHub Releases as the single
update source (the in-app updater is compiled out of store and package builds). Steam after v1.0: free,
Windows first, no Steamworks SDK (like OBS).
**Answer:** Go with the recommendation (D-044)

### Q-B6 — License of the `den-protocol` crate 🟩 (Round 3)
D-016 makes SDKs and protocol schemas MIT OR Apache-2.0, and the app GPL-3.0-or-later. `crates/den-protocol` is
the Rust source that the schemas and SDK types are generated from ([spec 02 §9.5](spec/02-protocol.md)).
Options: (a) MIT OR Apache-2.0, so the public Rust SDK crate can re-export the types and permissive plugins can
depend on it; (b) GPL-3.0-or-later like the rest of the app, with the SDK crate getting separately generated
types.
**Recommendation:** (a). It contains only types, schemas and catalog metadata, no engine logic.
**Answer:** Go with the recommendation (D-044)

### Q-B7 — Host for published schema URLs 🟩 (Round 3)
Plugin manifests and `protocol.json` carry `$schema`/`$id` URLs that ship inside plugins and binaries, so they
must not point at the personal GitHub account (ADR-0004). The app always validates offline against its bundled
copy, so this only affects editors and docs ([spec 02 §11](spec/02-protocol.md)).
Options: (a) a project domain (e.g. `schemas.<domain>/streamden/plugin/v0.json`), needs the domain from
ADR-0004's open item; (b) GitHub Pages on the org, `tech-daddy-digital.github.io/streamden/schemas/…`;
(c) raw files from the org repo.
**Recommendation:** (b) now, since it needs no domain and survives the repo transfer. Add a redirect from (a)
later.
**Answer:** Go with the recommendation (D-044)

### Q-B8 — Public version and prerelease scheme 🟥 (Round 3)

Options: SemVer with alpha/beta/rc channels and immutable versioned assets; or stable-only versions until
updating ships.
**Recommendation:** SemVer `0.1.0-alpha.N` → beta/rc → `0.1.0`, with tested monotonic native
installer mappings before publishing any prerelease format. Never replace published bytes under a tag.
See [Spec 01 §3](spec/01-build-release.md#3-release-identity-and-data-model).
**Answer:**

### Q-B9 — Update delivery and consent at v0.1 🟥 (Round 3)

Options: manual verified downloads first; or opt-in signed self-update for AppImage/Windows (macOS later),
with package-manager formats remaining external.
**Recommendation:** design stable/beta/alpha now, default
to manual checks and explicit download/install; enable self-update only after backup/recovery, signing
and contract gates pass. Decide whether v0.1 must include self-update or may defer that part of parity
23.17; notification cadence and staged rollout are also unapproved. No silent restart during streaming.
See [Spec 01 §6](spec/01-build-release.md#6-update-channels-api-and-ui-proposed-q-b9).
**Answer:**

### Q-B10 — Public release signing prerequisites 🟥 (Round 3)

Options: require signed Linux checksums/RPMs and defer Windows publication until SignPath approval; or
choose a paid Windows signer if SignPath is unavailable (budget remains Q-G3).
**Recommendation:** signed
Linux release first after org transfer, SignPath OSS for Windows later, no public unsigned Windows fallback
without an explicit decision. Updater keys remain separate. Existing account steps live in the
[distribution runbook](owner/registrations/distribution-and-signing.md); this question authorizes no key
generation, application or purchase.
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

### Q-P3 — Flatpak at launch? 🟩 (Round 2)
**Recommendation:** no, P6. The sandbox complicates system Python/.NET, local ports, MIDI and input
simulation.
**Answer:** Go with the recommendation (D-044)

### Q-P4 — Minimum Linux release baseline 🟥 (Round 3)

Options: publish the current Ubuntu 24.04 x86_64 build baseline plus tested Debian/Fedora/Arch versions;
or invest in an older build baseline (research suggested Ubuntu 22.04) before release.
**Recommendation:**
keep the verified 24.04 pipeline for v0.1, explicitly document minimum glibc/WebKitGTK and exact tested
distros, add older/ARM targets only with evidence. AppImage does not mean every Linux version works.
See [Spec 01 §7](spec/01-build-release.md#7-platform-reproducibility-and-budgets).
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

### Q-S2 — Streamer.bot alpha-only features count toward parity? 🟩 (Round 2)
Points/currency, Cron triggers, game-controller input, Whisper voice control, new Elgato apps, OBSBOT.
**Recommendation:** yes. Points and Cron in P5; controller and voice in P5.
**Answer:** Go with the recommendation (D-045)

### Q-S3 — Built-in loyalty (points, currency, ranks, cross-platform identity merge)? 🟩 (Round 2)
**Recommendation:** yes, P5, including linking one viewer across Twitch, YouTube and Kick.
**Answer:** Go with the recommendation (D-045)

### Q-S4 — Speech-to-text and voice commands in v1? CPU-only or GPU? 🟩 (Round 2)
**Recommendation:** P5. CPU by default with 1–2 threads so games aren't affected; GPU optional.
**Answer:** Go with the recommendation (D-045)

### Q-S5 — SAMMI-style OBS source/filter animation (easing, curves)? 🟩 (Round 2)
**Recommendation:** yes, P5, as sub-actions using obs-websocket.
**Answer:** Go with the recommendation (D-045)

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

### Q-S6 — Viewer-facing Twitch Extension (viewer-pressable buttons, Bits)? 🟩 (Round 3)
**Recommendation:** P6. It needs a hosted backend (EBS), which conflicts with local-first.
**Answer:** Go with the recommendation (D-045)

### Q-S8 — Pull Rumble earlier than P5? 🟥 (Round 3)
Owner request (2026-10-19): the combined chat must send to every platform that has chat. D-024 puts Rumble in P5,
but the official read side is cheap: one pasted URL polled every 5–10 s, plus an optional login-free real-time SSE
chat stream ([platform-apis.md 8.1](research/platform-apis.md)). Options: (a) keep everything Rumble in P5;
(b) Rumble **read** (official URL plus optional SSE) in P2 with the other multichat sources, send in P5;
(c) read **and** opt-in send in P2 (only if Q-C8 is (b) or (c)).
**Recommendation:** (b). Read is low-risk and fills the multichat in P2. Ship send in P2 too only if you accept the
Q-C8 risk, after a spike against a private test stream.
**Answer:**

### Q-S9 — Add Streamplace (atproto live video) as a chat platform? 🟥 (Round 3)
Bluesky itself has no live chat; its "Live Now" badge links out to Twitch, YouTube or stream.place. Streamplace
chat is plain atproto records (`place.stream.chat.message`), readable and sendable with the same login as our
planned Bluesky integration, and it has official moderation calls (block, hide, pin)
([platform-apis.md 14.1](research/platform-apis.md)). Options: (a) not in scope; (b) a small P2/P4 connector next
to Bluesky; (c) leave it to a community plugin.
**Recommendation:** (c) for now. Revisit if you or your audience start streaming there.
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
**Update (owner, 2026-10-19, DEN-244):** the owner now prefers **browser login first, manual connection as the
fallback** for every service (D-059). For YouTube that flips the default order: "Sign in with Google" against the
shared StreamDen project is the default, and the bring-your-own-project wizard is the advanced fallback. The shared
project already works for limited development and test use before Google verification (test users, warning screen,
7-day tokens in Testing, lifetime 100-user cap); OAuth verification (public rollout) and the quota audit (capacity)
are owner actions in [google-youtube.md](owner/registrations/google-youtube.md), tracked as a follow-up card, not as
a new question. BYO stays the supported path while the shared production sign-in is unavailable, so D-031's wizard
is still required. The historical answer above is unchanged.

### Q-C3 — Kick unofficial chat WebSocket fallback? 🟩 (Round 2)
It's the only way to get deleted-message sync, but Kick staff call it unsupported.
**Recommendation:** read-only fallback behind an "experimental" toggle.
**Answer:** Go with the recommendation (D-046)

### Q-C4 — X/Twitter 🟩 (Round 2)
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
**Answer:** Go with the recommendation (D-046)

### Q-C6 — Venmo / Cash App / X Money tips 🟩 (Round 2)
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
**Answer:** Go with the recommendation (D-046)

### Q-C7 — Bluesky, Threads, Mastodon 🟩 (Round 2)
Bluesky is free and works locally: posting, the "Live Now" badge (Twitch/YouTube links only, not Kick), and
follow/like/repost/mention/reply triggers by polling notifications. DMs are possible.
**Recommendation:** Bluesky in P2 alongside X. Threads and Mastodon as go-live announcement plugins later.
App password or OAuth as the default Bluesky login?
**Answer:** Go with the recommendation (D-046)

### Q-C5 — Cloud backups? 🟩 (Round 3)
**Recommendation:** local backups plus "export to a folder" (users can point it at Dropbox, Syncthing or Nextcloud).
No hosted backups.
**Answer:** Go with the recommendation (D-046)

### Q-C8 — Send chat to Rumble through its unofficial website API? 🟥 (Round 3)
Rumble has no official write API. Its Live Stream API URL is read-only. Sending, deleting, pinning and muting work
only through Rumble's private web endpoints while logged in as the streamer (session cookie `u_s`), as the
Apache-2.0 library cocorum shows. Rumble's terms prohibit "any form of automated software to interact with or access
the Rumble Site" without written permission, and cocorum's session check broke "again" in October 2026 (a 403
after a Cloudflare change, fixed two days later).
The risk falls on the streamer's own account. Details and our clean-room position (behaviour learned, no code
copied): [platform-apis.md 8.1](research/platform-apis.md).
Options: (a) no Rumble sending; Rumble stays a read-only target with a "Rumble has no official send API" reason in
the composer; (b) opt-in "unofficial" toggle, **off by default**, with a risk notice, sign-in through a StreamDen
login window (cookie to the OS keychain, password never seen), automatic fallback to read-only on any sign of
breakage, no paid rants; (c) like (b) but on by default once signed in.
**Recommendation:** (b). It meets the "send everywhere" goal, keeps the risk a visible, informed choice per user,
and fails safe. Pick the phase in Q-S8.
**Answer:**

### Q-C9 — TikTok LIVE chat sending stance 🟥 (Round 3)
TikTok has no official LIVE API. Sending needs the account's `sessionid` cookie **sent to a third-party signing
service** (Euler Stream) through AGPL libraries, and TikTok's terms forbid bots (section 3.4). D-024 already makes
TikTok a community plugin. Options: (a) no TikTok sending anywhere; first-party TikTok, if any, is read-only;
(b) no first-party sending, but the plugin SDK doesn't stop a community TikTok plugin from offering it (with a
mandatory warning that the session goes to a third party); (c) first-party opt-in sending like Q-C8 (b).
**Recommendation:** (b). The composer shows TikTok as "read-only (no official API)" unless a community plugin
declares a send capability.
**Answer:**

### Q-C12 — Hosted token broker timing: keep D-019's "later", or fund it earlier? 🟥 (Round 4)
D-059 makes browser consent the preferred target for five providers whose OAuth needs a confidential client secret:
Kick (no public client; verified 2026-10-19: [OAuth flow](https://docs.kick.com/getting-started/generating-tokens-oauth2-flow.md),
[KickDevDocs #413](https://github.com/KickEngineering/KickDevDocs/issues/413) open with no Kick reply), Discord
`webhook.incoming`, Streamlabs, StreamElements and Patreon. The secret may only live in the official hosted relay's
token broker, which D-019 schedules "later" (unfunded). Until it runs, each of them ships with its manual path (BYO
Kick app plus self-hosted relay or tunnel; pasted webhook URL, socket token, JWT, creator token) and the docs say so.
This is not a request to re-confirm browser-first, and it does not block the DEN-244 docs or story plan: D-019 stands
if unanswered. Options: (a) keep D-019 as is: the broker arrives with the hosted relay whenever that is funded
(default); (b) fund the hosted broker in P2 with a budget line, abuse handling and a privacy policy, so the five
buttons ship together with their connectors. Shipping the secrets inside the app is not an option (research and
provider terms).
**Recommendation:** (a) for planning; keep the broker endpoint in the P2 self-host relay design so (b) is a
deployment, not a rewrite. Provider app approvals (Streamlabs, Kick verification, StreamElements credential request)
can start before either.
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

### Q-X2 — Other imports 🟩 (Round 2)
SAMMI decks, Speaker.bot settings and voice aliases, Touch Portal `.tpp` plugin compatibility, StreamElements
widget import.
**Recommendation:** Speaker.bot settings (P2), SAMMI decks (P5). Skip `.tpp` unless demand appears.
**Update 2026-10-18:** StreamElements widget compatibility is now a goal (D-012) and is proposed for **P2**,
so chat overlays work before our own designer ships in P3. Streamlabs custom chat-box compatibility is a
possible follow-up.
**Answer:** Go with the recommendation (D-047)

### Q-X3 — Variable and template syntax 🟩 (Round 2)
Keep Streamer.bot's `%var%` / `~global~` / `$fn()$` as-is? Use a new typed expression language (e.g.
`{user.name}`, `{amount | currency}`)? Or new syntax plus a compatibility mode?
**Recommendation:** new `{…}` syntax as canonical, with `%var%` accepted on import and in a compat mode.
**Answer:** New `{…}` syntax canonical; `%var%` accepted and converted on import (D-027).

### Q-X4 — How much template expression language in v1? 🟩 (Round 3)
D-027 fixed the `{…}` syntax. [Spec 02 §10](spec/02-protocol.md) proposes three layers: paths (`{user.name}`),
filters (`{amount | currency}`), and inline expressions (`{= count * 2}`, which replaces `$math()$` and
`$parse()$`). Options: (a) all three in P1; (b) paths and filters in P1, expressions later (imported `$math()$`
then stays unconverted text until they land); (c) paths only, with logic in sub-actions and code.
**Recommendation:** (a). Imported Streamer.bot actions use `$math()$` a lot, and a pure expression evaluator with
a step budget is small.
**Answer:** Go with the recommendation (D-047)

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

### Q-R3 — Embedded JS plugins have no Node.js APIs 🟩 (Round 2)
Plugins needing Node would be sidecars. Acceptable?
**Recommendation:** yes.
**Answer:** Go with the recommendation (D-047)

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

### Q-U3 — Visual direction 🟩 (Round 2)
"Like Streamer.bot" (dark, dense, panel-based)? Any brand colors, references or things to avoid?
**Answer:** No (D-048)

### Q-U4 — Localization at launch? 🟩 (Round 3)
**Recommendation:** i18n-ready strings from day one; English only until P6.
**Answer:** Go with the recommendation (D-048)

### Q-U5 — Native chat window on Wayland ✅ (proposed default)
OBS disables custom browser docks on native Wayland (Hyprland included), so we ship a native always-on-top
chat window *and* an OBS dock URL. **Confirmed by spike S-2:** no dock menu at all on native Wayland; docks work
under XWayland. On **NVIDIA Linux**, OBS's browser source hardware acceleration must be turned off (blank sources
on Wayland, crash under XWayland), and the setup wizard should detect this and guide the user.

### Q-U6 — Combined-chat composer: "send to all" defaults 🟥 (Round 3)
The composer sends to one or more targets (platform × channel × identity). Open defaults: (1) What does **Enter**
send to: the platform of the tab you're viewing or of the message you're replying to, or every enabled target?
(2) Which identity is the default, broadcaster or bot? (3) On a partial failure, show per-target results only, or
also offer "retry failed"? (No automatic retries; a resend could double-post.) (4) Should unofficial or
risk-flagged targets (Rumble via Q-C8) be included in "send to all"?
**Recommendation:** Streamer.bot-like keys: Enter = current target (the tab's platform, or the replied-to
message's platform), Shift+Enter = all enabled targets, Ctrl+Enter = as bot. Default identity = broadcaster.
Per-target result chips with a manual "retry failed". Unofficial targets join "send to all" only when the user
ticks them in the target picker.
**Answer:**

---

## N — Network

### Q-N1 — Local-only, or LAN mode for two-PC rigs and the phone deck? 🟩 (Round 1)
Many VTubers run the avatar app on one PC and the bot on another. The phone deck also needs LAN access.
**Recommendation:** localhost by default. **Opt-in LAN mode** with pairing tokens in P4. Every connector
accepts a remote host, not just `localhost`.
**Answer:** Accepted default: localhost by default, opt-in LAN mode in P4 (D-024).

---

## D — Designer and alerts

### Q-D1 — Alert and TTS audio routing 🟩 (Round 2)
Play alert sounds through the OBS browser source (captured by OBS), the app's own audio device, or user's choice
per alert?
**Recommendation:** user's choice, defaulting to the browser source.
**Answer:** Go with the recommendation (D-049)

### Q-D2 — Alert queue defaults 🟩 (Round 2)
How many priority lanes, should higher priority interrupt, and should gift bombs merge into one alert?
**Recommendation:** three lanes (critical, normal, low), no interruption by default, gift bombs merged.
**Answer:** Go with the recommendation (D-049)

### Q-D4 — StreamElements widget runtime defaults 🟩 (Round 2)
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
**Spike S-2 input (2026-10-18):** the per-widget origin scheme `w-<id>.localhost` works in OBS's CEF (isolated and
persistent). But iframed widgets get partitioned storage, and writes made just before a CEF crash were lost. So
the SE `localStorage` shim should be **backed by our server's storage**. Recommendation: yes.
**Answer:** Yes (D-049)

### Q-D3 — Real-time co-editing with mods? 🟩 (Round 3)
**Recommendation:** not in v1. Yjs underneath keeps it possible.
**Answer:** Go with the recommendation (D-049)

---

## T — TTS

### Q-T1 — TTS engines at launch 🟩 (Round 2)
Local: Piper, Kokoro (via sherpa-onnx), OS voices (SAPI/OneCore, AVSpeech, speech-dispatcher). Cloud: Azure,
Polly, Google, ElevenLabs, OpenAI, TTS Monster…
**Recommendation:** P2 = Piper + Kokoro + OS voices + ElevenLabs + Azure. The rest are plugins.
**Answer:** Piper + Kokoro + OS voices + ElevenLabs + Azure at launch (D-033).

---

## I — Integrations

### Q-I1 — Is a small C# companion plugin for Warudo acceptable? 🟩 (Round 2)
Warudo mods must be C#. It would be a tiny, separately built plugin.
**Recommendation:** yes.
**Answer:** Yes (D-050)

### Q-I2 — Wayland keystroke simulation 🟩 (Round 2)
Portal-only (prompts the user, partly supported on Hyprland), or also ship a `ydotool`-style fallback that needs
a udev rule?
**Recommendation:** portal first, with an optional fallback and a guided setup.
**Answer:** Go with the recommendation (D-050)

### Q-I3 — Spotify 🟩 (Round 2)
Since February 2026, apps in Spotify's development mode need the owner to have Premium, and allow 5 users.
Options: users supply their own client ID, or skip and use OS "now playing" (MPRIS/Windows media session).
**Recommendation:** OS now-playing in core; Spotify as a bring-your-own-client-ID plugin.
**Answer:** Go with the recommendation (D-050)

### Q-I4 — Unofficial/reverse-engineered APIs (Wave Link, SteelSeries Sonar…)? 🟩 (Round 2)
**Recommendation:** only behind an "experimental" toggle.
**Answer:** Go with the recommendation (D-050)

### Q-I5 — Stream Deck approach 🟩 (Round 2)
Our own Elgato plugin (also runs on OpenDeck on Linux), direct hardware control, or both?
**Recommendation:** an Elgato SDK plugin that is OpenDeck-compatible (P4). Direct HID control later, if ever.
**Answer:** Go with the recommendation (D-050)

### Q-I6 — Home Assistant as the official smart-home bridge? 🟩 (Round 2)
**Recommendation:** yes for the long tail. Native Hue, Elgato Key Light and WLED for the most common devices.
**Answer:** Go with the recommendation (D-050)

### Q-I7 — Unknowns to confirm 🟩 (Round 2)
What is "Karasubot"? Is PolyPop still used by your community? Any other tools you or your VTuber friends rely
on that aren't in [app-integrations-vtuber.md](research/app-integrations-vtuber.md)?
**Answer:** I think you mistook karasubot for kbonk (https://typeou.itch.io/karasubonk) but yes lets make sure integration works there too. Polypop is still used I believe (D-051)

---

## K — Plugins and registry

### Q-K1 — Native sidecar plugins in the public registry? 🟩 (Round 3)
**Recommendation:** only in a "Verified" channel with build provenance. Declarative and JS plugins in the open
channel.
**Answer:** Go with the recommendation (D-052)

### Q-K2 — Paid or DRM plugins ever? 🟩 (Round 3)
**Recommendation:** the registry only lists free plugins. Paid plugins can exist outside it.
**Answer:** Go with the recommendation (D-052)

### Q-K3 — Registry governance 🟩 (Round 3)
Who reviews submissions, the namespace policy, takedown process.
**Answer:** I (or an AI agent) will review and handle the registry governance (D-052)

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

### Q-A4 — Remote AI access (ChatGPT web, claude.ai, grok.com) 🟩 (Round 3)
These need a public URL (tunnel) plus OAuth.
**Recommendation:** after v1. Local clients cover most needs.
**Answer:** Go with the recommendation (D-053)

### Q-A5 — Secrets on Linux without a keychain daemon 🟩 (Round 2)
Some Hyprland setups have no Secret Service daemon. Fall back to an encrypted file (passphrase), or require
gnome-keyring/KeePassXC?
**Recommendation:** use Secret Service if present, otherwise an encrypted file with a clear warning in the UI.
**Answer:** Go with the recommendation (D-053)

### Q-A3 — AI in chat 🟩 (Round 2)
AI co-host replies, chat summaries, moderation assist? Should replies auto-post or go to an approval queue? Which
providers can power it (local models too)? Disclosure (the EU AI Act Art. 50 transparency duty has applied since
2026-08-02, per secondary sources)? Budget caps per stream?
**Recommendation:** v0.5. The responder has **no write tools**, so viewers can't talk it into running actions.
Replies pass moderation, the approval queue is the default, disclosure is on, and there's a hard budget cap.
**Answer:** Go with the recommendation (D-053)

### Q-A6 — Do MCP-caused event chains inherit the MCP restrictions? 🟩 (Round 3)
[Spec 02 §8.8](spec/02-protocol.md) makes authorization transitive: a run started over MCP can never reach tier 4
(code, end stream) through nested or queued actions. But an MCP run can emit events (a chat message, a variable
change), and the user's own triggers on those events start new runs. Options: (a) new runs start under the
trigger's authority (tier 4 allowed): a trigger is the user's configuration, so this is user intent; the audit log
links the chain through `causedBy`; (b) **runs with MCP lineage inherit the MCP restrictions** (`maxTier: 3`,
scopes, approval policy) for every hop of `causedBy`, which also blocks legitimate user automations whenever an AI
client is active; (c) configurable per MCP token, default (a).
**Recommendation:** (a) was the spec author's recommendation; the Codex re-review (2026-10-19) rated resetting
authority at each event a P1 privilege-widening path, and the coordinator chose (b).
**Answer (owner confirmed 2026-10-19, D-038):** (b). Event chains caused by an MCP-originated
run inherit the MCP restrictions for the whole `causedBy` chain (no hop limit), with audit linkage through
`causedBy` and `run.chain`. Spec 02 §8.8 rule 5 is written that way, applies the same rule to every restricted
principal (a plugin's limited permissions follow its chains too; a user token or system run is unaffected), stops a
chained tier-3 node as `APPROVAL_REQUIRED` / `unattended` because nobody can answer, and keeps the UI lint. Events
without `causedBy` (upstream events, including a platform echoing the engine's own message) are not covered.

---

## H — Data

### Q-H1 — Chat history retention 🟩 (Round 3)
Twitch and YouTube developer policies limit how long chat data is cached (about 24 h and 30 days).
**Recommendation:** keep a rolling 24 h by default, configurable, and keep only aggregate stats beyond that.
**Answer:** Go with the recommendation but implement local chat history storage with length/size definable by the user (up to 120 streams/120 days) (D-054)

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

### Q-G3 — Budget for paid items 🟩 (Round 2)
Apple Developer ($99/yr), Windows signing (SignPath free for OSS; Azure Trusted Signing about $10/mo), domain,
Cloudflare (relay free tier), test accounts.
**Answer:** I already have an Apple Developer, and cloudflare account. Lets use signpath for signing. (D-055)

---

## E — Engine core

Raised by [spec 03](spec/03-engine.md) (DEN-15). Each has a spec default so implementation is not blocked; the
owner's answer either confirms it or changes the spec.

### Q-E1 — Should queued runs survive a restart? 🟩 (Round 3)
Streamer.bot does not document what happens to pending runs at shutdown. Options: (a) **not durable**: waiting runs
are cancelled (`Cancelled { Shutdown }`, visible in history) and nothing is replayed on the next start; (b) durable
queues: waiting runs are persisted and resumed after restart, including after an update.
**Recommendation:** (a), spec 03 §10.8. Replaying yesterday's alerts after an update is the surprising behaviour;
(b) can be added later per queue.
**Answer:** Spec default accepted by the owner on 2026-10-19 (D-039).

### Q-E2 — Should a failed sub-action stop the run by default? 🟩 (Round 3)
Streamer.bot mostly continues after a failed sub-action and reports the failure in an output variable (`success`);
only C# `return false` and a few explicit flags stop the action. Options: (a) **stop by default** (`onError: stop`),
with `onError: continue` per node for "try"-style steps and for imports; (b) continue by default, stop per node.
**Recommendation:** (a), spec 03 §9.10: a half-executed alert is worse than a visible failure, and the importer
sets `continue` where Streamer.bot semantics need it.
**Answer:** Spec default accepted by the owner on 2026-10-19 (D-039).

### Q-E3 — One event, one action: fire once or once per matching trigger? 🟩 (Round 3)
When several triggers of the **same** action match one event, spec 03 §6.3 fires the action **once**, through its most
specific matching trigger. Streamer.bot's documented behaviour covers only precedence *across* actions. Options:
(a) once per action (spec default); (b) once per matching trigger (N runs).
**Recommendation:** (a); duplicate alerts from overlapping triggers are a common Streamer.bot support question.
**Answer:** Spec default accepted by the owner on 2026-10-19 (D-039).

### Q-E4 — Action History and event log retention defaults 🟩 (Round 3)
Spec 03 §12 proposes 10,000 runs or 7 days (whichever prunes more, keeping up to 1,000 recent failures extra), a
24 h / 50,000-event log, history kept across restarts, snapshots capped at 256 KiB. Streamer.bot documents no
retention. Related: Q-H1 (chat retention) bounds the event log for chat types.
**Recommendation:** accept the defaults; all are settings.
**Answer:** Spec default accepted by the owner on 2026-10-19 (D-039).

### Q-E5 — `Stop` inside an inline Run Action: end the child only, or the whole chain? 🟩 (Round 3)
Streamer.bot runs an inline action "as if nested", which suggests its Break stops the caller too. Spec 03 §9.5 ends
only the child action's run and lets the parent continue, because a reusable child should not decide the caller's
fate. Options: (a) child only (spec default); (b) whole chain; (c) a flag on the Run Action node.
**Recommendation:** (a).
**Answer:** Spec default accepted by the owner on 2026-10-19 (D-039).

### Q-E6 — Command permissions: what does an empty list mean? 🟩 (Round 3)
Streamer.bot: an empty Allow list allows everyone, an empty Deny list **denies everyone**. Spec 03 §7.1 makes an
empty list neutral in both modes (empty Deny denies nobody) and lets the importer write an explicit list.
**Recommendation:** neutral (spec default); the Streamer.bot rule reads as an accident of implementation.
**Answer:** Spec default accepted by the owner on 2026-10-19 (D-039).

### Q-E7 — When a `While` loop hits its iteration cap, warn or fail? 🟩 (Round 3)
Spec 03 §9.5 ends the loop, records a warning on the run, and continues after the loop (default cap 10,000,
ceiling 1,000,000). Streamer.bot has no cap and offers a manual "Break Running" kill switch. Options: (a) warn and
continue (spec default); (b) fail the run.
**Recommendation:** (a).
**Answer:** Spec default accepted by the owner on 2026-10-19 (D-039).

### Q-E8 — Does the `causedBy` hop limit apply to the user's own re-trigger loops? 🟩 (Round 3)
Spec 03 §14.3 stops any chain after 16 `causedBy` hops, for every principal, and audits it. That also ends a
deliberate Streamer.bot-style loop (action A → custom event → A, "repeat until a global flips") after 16 rounds.
Options: (a) keep the limit for everyone; `While`, timers and queued Run Action are the supported loop forms and the
UI notice says so (spec default); (b) exempt user-token and system origins from the hop limit and rely on the storm
guard alone; (c) make the limit a setting with a higher default (for example 64).
**Recommendation:** (a).
**Answer:** Spec default accepted by the owner on 2026-10-19 (D-039).

### Q-E9 — How long may a user keep Action History and the event log? 🟩 (Round 4)
Raised by [spec 03 r6 §16](spec/03-engine.md) (DEN-68). Q-E4 accepted the retention **defaults** (10,000 runs or
7 days of Action History, 24 h / 50,000 events in the event log) and Q-H1 answered **chat** retention (a rolling
24 h by default, plus a local chat-history store of up to 120 streams or 120 days, owned by spec 08). Neither says
how far a user may raise `engine.history.maxAgeDays` and `engine.events.maxAgeHours` for non-chat data (run
records with their argument snapshots, alert and automation events). Both are plain settings; the question is only
the ceiling the settings validator enforces. Chat entries in the event log are pruned by spec 08 on the Q-H1
setting whatever the answer here. Options: (a) **365 days** for both (a year of history fits a streaming season;
the file sizes are bounded by `maxRuns` / `maxCount` anyway); (b) 120 days for both, the same figure as the chat
store, so one number explains every retention screen; (c) no ceiling beyond the storage counters (`maxRuns`,
`maxCount`), the user decides. Until answered, the spec validates only the lower bound (1 day / 1 h) and keeps the
defaults.
**Recommendation:** (a).
**Answer:** Option (a), approved by the owner on 2026-10-19 (D-057): **365 days** for both non-chat
Action History and the event log (`engine.history.maxAgeDays` maximum `365`;
`engine.events.maxAgeHours` maximum `8760`). Existing defaults and the separate Q-H1 / D-054 chat-retention
policy remain unchanged.
