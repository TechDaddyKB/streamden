# Streamer.live — Branding, Name Risk and Distribution Research

> **Status:** research / recommendation (not a decision record). Feeds **Q-B1** (product name), **Q-B2** (license) and **Q-B5** (Steam) in `docs/QUESTIONS.md`.
> **Date:** 2026-10-18 (all live lookups ran 2026-10-18/19 UTC).
> **Scope:** Part 1, name availability, trademark and confusion risk, alternative names, ID/package naming for the studio "Tech Daddy Digital". Part 2, distribution channels for non-game software (Steam, itch.io, GitHub Releases, AUR, winget, Homebrew, Flathub) and one-update-source design.
> **Method:** Domain data comes from RDAP (the registries' own JSON whois) because port-43 `whois` returned "TLD is not supported" for `.live`/`.digital` and could not reach the `.app`/`.dev` server from this machine; `whois` did work for `.com`/`.net`/`.io`. `curl` on this machine is broken (libnghttp2 symbol error), so HTTP probes used Python `urllib`. Trademark data comes from the **public USPTO trademark-search backend** (the same JSON endpoint the tmsearch.uspto.gov web app calls) and Justia's USPTO index. Steam data comes from the public Steam store API and Steamworks docs. Claims I could not verify are marked **(unverified)**.
> **Not legal advice.** I am not a lawyer. Trademark conclusions below are an engineering-side risk screen, not a clearance opinion.

---

## 0. TL;DR

**Name verdict: do not ship under "Streamer.live". Keep it as the working title / repo codename only.**

| Dimension | Rating | Why (one line) |
|---|---|---|
| Legal trademark risk (US) | **Low–Medium** | No federal filing for STREAMER.BOT, STREAMERBOT, STREAMER LIVE or STREAMERLIVE found; but Streamer.bot has unregistered (common-law) rights, same goods class, same customers, same `Streamer.xxx` styling |
| Community / confusion risk vs Streamer.bot | **Medium–High** | Reads as "Streamer.bot 2 / clone / fork"; the owner's own stated reason for liking the name is its similarity to Streamer.bot, which is the wrong thing to be on record saying if anyone objects |
| Practical brand-ownership risk | **High** | `streamer.live` redirects to another company (Switchboard Live); `getstreamer.live` + GitHub repo `crimsonstrife/streamer.live` is an existing open-source product literally named "Streamer.live"; `streamerlive.com` is a BrandBucket premium name; `streamerlive.live` is registered |
| **Overall** | **Medium–High: not recommended as the shipped brand** | You cannot own the canonical domain, the name is descriptive (hard to register as a mark), and it sits next to the incumbent |

**Best alternatives** (details in §1.5): **Streamwright**, **NekoStage** (VTuber-friendly), **StreamDen**, **Livewright**, **TanukiLive**. All five have free `.live` and `.io` domains, free crates.io and npm names, and no live USPTO filing in software classes (Livewright has an unrelated Class 45 mark; StreamDen has an abandoned entertainment filing; see the table).

**IDs:** use `com.techdaddydigital.<product>` for the Tauri `identifier` and `com.techdaddydigital.<Product>` for Flatpak, but **do not bake any ID until the name is final**. The GitHub org `Tech-Daddy-Digital` already exists; `techdaddydigital.com` is registered (Cloudflare); `.digital`, `.dev`, `.app`, `.net`, `.io` variants are unregistered today.

**Steam:** viable and worth doing **after v1.0**, as a free "Software" app on Windows (and macOS) with **no Steamworks SDK** (the OBS Studio path). Defer Steam Linux (the Steam runtimes ship neither GTK3 nor WebKitGTK) and Steam Workshop (needs the proprietary Steamworks SDK, which conflicts with GPL unless you hold all copyrights and add an exception). Launch on GitHub Releases + itch.io + AUR + winget; GitHub Releases stays the single update source of truth.

---

## 1. Part 1 — Name availability and risk

### 1.1 What already exists under "Streamer.live / StreamerLive / Streamer Live"

| # | Asset | Finding | Source / how checked |
|---|---|---|---|
| 1 | **`streamer.live`** (domain) | Registered **2015-10-29**, registrar Name.com, **expires 2026-10-29** (11 days from today), last changed 2026-08-11, status `client transfer prohibited`, registrant redacted. Port 80 answers **301 → `https://switchboard.live`** (nginx on 75.126.100.7); port 443 refuses connections. Switchboard Live is a multistreaming / "government-approved live streaming management" SaaS. So the domain is almost certainly held by Switchboard Live (inferred from the redirect; registrant is not public) | RDAP `rdap.identitydigital.services/rdap/domain/streamer.live`; Python HTTP probe; <https://switchboard.live/> |
| 2 | **`getstreamer.live`** | Registered 2025-03-14 (Cloudflare). Hosts a Docusaurus site titled **"Streamer.live — Open-source web toolkit for streamers"**: "a self-hosted Laravel-based platform" (Twitch status, Discord alerts, Fourthwall merch, CMS page builder, self-updater) | <https://getstreamer.live/> |
| 3 | **GitHub `crimsonstrife/streamer.live`** | MIT, PHP/Laravel, **4 stars, 3 forks**, created 2025-01-27, last push 2026-06-09, releases up to `v1.4.7-alpha` (2026-03-26). Same name, same audience (streamers), different product (fan-site CMS, not automation) | `gh api repos/crimsonstrife/streamer.live` |
| 4 | `streamerlive.com` | Registered 2007-02-11, **BrandBucket name servers** (a premium-domain marketplace, so presumably for sale at a premium price; price not checked). Returns HTTP 403 | RDAP (Verisign); probe |
| 5 | `streamerlive.live` | Registered **2025-10-18** (WebNIC reseller), in auto-renew period; host does not answer | RDAP |
| 6 | `streamer-live.com` | Registered 2022-11-18 (GoDaddy, privacy proxy), behind Cloudflare, origin down (HTTP 525) | RDAP; probe |
| 7 | `streamerlive.app`, `streamer-live.app`, `streamerlive.dev`, `streamerlive.io` | **Unregistered** (RDAP 404 / "Domain not found") | RDAP / whois |
| 8 | Other `streamer.*` | `streamer.dev` (a sports-streams site), `streamer.tools`, `streamer.run`, `streamer.chat` are live sites; `streamer.app` and `streamer.studio` (expires 2026-11-12) are registered but have no DNS; `streamer.gg`, `streamer.tv`, `streamer.cloud` look unregistered (RDAP 404 from the Identity Digital server; `.gg`/`.tv` TLDs are served elsewhere, so treat those two as unverified) | RDAP; probe |
| 9 | GitHub accounts | User **`Streamerlive`** exists (created 2023-03-27, 1 repo). Orgs/users `streamer-live`, `streamerdotlive`, `streamer.live` do not exist. Repos named like it: `ferr-96/streamerlive` (React/Node livestream platform, 0 stars), `richliby/StreamerLive-Bot` (1 star), `chriseli2007-sketch/-streamerlivee.github.io` (0 stars) | `gh api users/...`, `search/repositories` |
| 10 | crates.io / npm | `streamerlive`, `streamer-live`, `streamer_live` are **free** on both | crates.io + npm registry APIs |
| 11 | Steam | No app named "Streamer Live" / "StreamerLive" / "Streamer.live" (a fuzzy hit, "ViRo Live Studio - Adult Streamer Tools", is unrelated) | Steam store search API |
| 12 | itch.io | No product by the exact name; unrelated games with "Streamer Live" in the title (e.g. "SLAP - Streamer Live Attack & Pelting") | itch.io search |
| 13 | App stores / companies | Search found no company or app called "StreamerLive"; nearest names are AVerMedia "Live Streamer" (mobile app) and BIGO LIVE Streamer. Web search is weak for this, so **(unverified)** beyond the above | web search |

Takeaways:

1. **You cannot own the canonical domain.** `streamer.live` is held by another company; if they let it lapse it still goes through the registry grace periods first (typically auto-renew grace up to 45 days, redemption 30, pending-delete 5; **(unverified for this registry)**), so the earliest drop would be around early 2027, and a backorder would only be a lottery ticket.
2. **There is already a product called "Streamer.live"** with a public repo, a docs site and a Discord. It is tiny (4 stars) and in a different niche, but it owns the exact GitHub repo name pattern and the `getstreamer.live` brand, and search results for the name will be split.
3. The shipped brand would live at an awkward domain (`getstreamerlive.com`, `streamerlive.app`, …), which already signals "second-best".

### 1.2 Trademark search (what was findable)

USPTO, via the public tmsearch backend, run 2026-10-19. Searches were exact-phrase word-mark queries (not phonetic, not design marks), so "no hit" means "no filing under that exact word mark", not "clear".

| Query (word mark) | Result |
|---|---|
| `STREAMER.BOT`, `STREAMERBOT`, `STREAMER BOT` | **0 filings** (also 0 in Justia's USPTO index). Streamer.bot has no US federal registration or pending application under that wording. Its rights are therefore **common-law / unregistered** (use-based). Its site footer says only "© 2026 Streamer.bot - All rights reserved" with no ™/® ([streamer.bot](https://streamer.bot)); its Terms contain no trademark clause but do forbid duplicating "the integration code or concepts" ([streamer.bot/terms](https://streamer.bot/terms)) |
| `SPEAKERBOT`, `SAMMI` (software) | `SPEAKERBOT`: 0. `SAMMI`: only dead/unrelated filings (drinks, steel, an abandoned Semantic Machines software filing). So the two other products being succeeded are unregistered as well |
| `STREAMER LIVE`, `STREAMERLIVE`, `STREAMER.LIVE` | **0 filings** under those exact words |
| `LIVE STREAMER` (word transposition) | **Live pending:** serial **99803740**, filed **2026-05-04**, owner **Live Shop Studio LLC**, Class 42 "SaaS … software for audio and video signals … video production", status "non-final action mailed". Same owner also has **`STUDIO STREAMER`** (serial 99803715; Class 9 mobile app for streaming video; Class 42). An older `LIVE STREAMER` (AVerMedia, 2021) is abandoned. A USPTO examiner treats transposed word order as similar, so if you ever file `STREAMER LIVE` for software, expect a possible citation against `LIVE STREAMER` |
| Other live `STREAMER` word-mark filings in software/media classes | `NIMBLE STREAMER` (Softvelum, Class 9 streaming software), `STREAMER PROM` (Class 9/38/25), `STREAMER BOOT CAMP`, `STREAMER ACADEMY PRO`, `THE STREAMER ACADEMY`, Kai Cenat's `STREAMER UNIVERSITY` (Class 41). Evidence that "STREAMER" is a crowded, mostly descriptive term |
| `TECH DADDY DIGITAL`, `TECH DADDY`, `TECHDADDY` | **0 filings**. The studio name is free of USPTO conflicts (but also unprotected) |
| EUIPO / UKIPO / WIPO | **Not searched.** eSearch plus / TMview / Global Brand Database are JavaScript apps with no unauthenticated API reachable from here. Do this manually, or have an attorney run it, before committing to a name for EU sales |

Notes:

- **"Streamer" + "live" are both descriptive** for streaming software. A mark that is merely descriptive is hard to register at the USPTO without proof of acquired distinctiveness (my reading of general USPTO practice; **(unverified)** for this specific mark). Even if nobody objects, you would struggle to enforce the name against copycats. A coined name (Streamwright, NekoStage) is both safer and more protectable.
- Because Streamer.bot is unregistered, a claim against you would be a common-law / Lanham Act §43(a) unfair-competition theory: it needs Streamer.bot's mark to be distinctive in the market and consumers likely to be confused. Streamer.bot's mark is itself weak-ish (descriptive words) but the community-recognized combined brand is strong in this niche.

### 1.3 Confusion risk with Streamer.bot — frank assessment

**Rating: Medium (legal/community), High (practical).**

Factors pushing risk **up**:

- **Same field, same customers, same channels.** Both are automation tools for streamers (software, Class 9/42), found via the same Discord/Reddit/YouTube tutorials. This is the single biggest factor in a likelihood-of-confusion analysis.
- **Identical naming pattern.** `Streamer.<short word>` with a dot and one short suffix. Streamer.bot established this pattern in the niche; copying it signals "family".
- **Stated intent.** `docs/QUESTIONS.md` records that the owner likes the name *because it is similar to Streamer.bot*. If a dispute ever arises, "chosen to evoke the incumbent" is the fact a complainant wants. Avoid repeating it in public copy.
- **"Successor" positioning.** The project is described internally as a Streamer.bot + Speaker.bot + SAMMI *successor*. Public copy should say "alternative to" / "imports Streamer.bot actions" (nominative, descriptive use), never "Streamer.bot 2", "new Streamer.bot" or similar.
- **Community perception.** I found no documented community reaction to a Streamer.bot lookalike (web search returned only directory listings), so the following is opinion: in a small, tightly knit community with a single founder-led incumbent (credited team: nate1280, Lyfesaver74, Whipstickgostop on [streamer.bot](https://streamer.bot)), a similarly named newcomer is likely to be read as a clone or fork whether or not it is, and support questions will leak ("is this the new Streamer.bot?"). This would be a reputational cost even with zero legal exposure.
- **Search/SEO collision.** Searching "streamer.live" lands on Switchboard Live and on the Laravel project, not on you.

Factors pushing risk **down**:

- No federal registration, and both marks are built from generic words; "bot" vs "live" differ in sound, appearance and meaning.
- Different logo/visual identity and a clear "not affiliated" statement would help a lot.
- You are clean-room (Q-B2 note), and Streamer.bot's Terms restrict copying *its integration code/concepts*, not building a competitor.

**If you keep it anyway (not recommended):** never use "Streamer.bot" in product copy except nominative comparisons with a "not affiliated" line; use a visually distinct logo/colour scheme; do not use `bot` in any package, repo or domain name; accept that your official domain will not be `streamer.live`; reach out to the Streamer.bot team as a courtesy before launch (they may well be friendly, and a heads-up costs nothing).

### 1.4 Names rejected during screening

| Name | Why rejected |
|---|---|
| Stagehand, Showrunner, Patchbay, Switchyard, Rundown, Callboard, Ghostlight, Marionette | Real words with crates.io/npm names taken and every major domain taken (Stagehand also collides with an AI browser-automation framework, **from memory, unverified**; `patchbay` has ~36k crates.io downloads) |
| StreamForge, StreamSmith, StreamNook, StreamHaven, StreamGrid | Domains and/or crates.io taken, or an existing GitHub org/project in the space |
| StageDeck, StreamerDeck, "Deck" anything | Confusable with Elgato Stream Deck |
| Kuroko | Nice metaphor (kuroko are the black-clad stagehands in kabuki who work unseen, a good fit for automation, and anime-friendly), but: a live `KUROKO` word mark in Class 9 (Shenzhen jingpei Technology, serial 86822201), npm name taken, `kuroko` GitHub org exists, `.com/.app/.dev/.io` taken, and the name is saturated by the anime *Kuroko's Basketball* and (from memory, unverified) a programming language called Kuroko |
| Livestreamer | The famous old Python CLI (superseded by Streamlink) |

### 1.5 Alternative names (10 candidates plus a note on dotted names)

Availability as of 2026-10-19. "T" = taken/registered, "free" = RDAP 404. GitHub column = user/org handle. USPTO = exact word-mark phrase search.

| # | Name | Style | .com | .app | .dev | .live | .io | GitHub handle | crates.io / npm | USPTO exact | Other signals | My take |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | **Streamwright** | Descriptive-brandable ("one who builds streams"); keeps "stream" | T (for sale on HugeDomains) | T | T | **free** | **free** | org exists, 0 repos (created 2026-02-09) | free / free | none | No Steam/itch hits. Tiny GitHub libs `streamwright-python` ("multi-LLM pipeline orchestration", 1★) and `streamwright-npm` | **Best overall.** Memorable, no baggage, clearly "stream tooling", gives you a "wright/forge" craft theme for plugins |
| 2 | **NekoStage** | VTuber/mascot-friendly (neko = cat, stage = scene/overlay) | **free** | free | free | free | free | free | free / free | none | No Steam/itch/GitHub-repo hits | **Everything is free including .com.** Cute and VTuber-native; may feel niche to non-VTuber streamers |
| 3 | **StreamDen** | Cozy/community-friendly | T (no site) | free | free | free | free | free | free / free | `STREAMDEN` (Streamden LLC, Colorado, Class 41 variety show, serial 97008816) is **abandoned** | No product found | Good feel; the abandoned entertainment-show filing suggests a prior user of the name in a different class: check before committing |
| 4 | **Livewright** | Descriptive ("builds your live show") | T (**LiveWright coaching business**, livewright.com) | free | free | free | free | free | free / free | `LIVEWRIGHT` live, Class 45 personal-development consulting (individual owner), serial 99809449 | No Steam hits | Cleanest *handles*, but the .com is an active unrelated brand; different class, so low legal risk, moderate SEO collision |
| 5 | **TanukiLive** | VTuber-friendly mascot (tanuki = shape-shifting raccoon dog) | **free** | free | free | free | free | free | free / free | none | No hits anywhere | All-free; "Live" in the name keeps the Streamer.live spirit. Less distinctive spelling than #2 |
| 6 | **StreamRig** | Descriptive ("stream rig" = a streamer's setup) | T ("coming soon" parked page) | free | free | free | free | user exists, 0 repos | free / free | none | GitHub `StreamRig` (21★) is an unrelated computer-vision repo | Strong meaning; the parked .com means someone may be planning a product (unknown) |
| 7 | **Livehearth** | Cozy/warm | T (no site) | free | free | free | free | free | free / free | none | none | Pleasant but vague; low recall |
| 8 | **StagePaw** | VTuber-friendly pun | T (no site) | free | free | free | free | free | free / free | none | none | Cute; reads as hobby project |
| 9 | **KitsuneKit** | VTuber-friendly (fox mascot + "kit") | **free** | free | free | free | free | user exists, 0 repos | free / free | none | Steam has an unrelated "Kitsune Kitchen" | Alliterative, easy to say; "Kitsune-" names are common in the anime/VTuber world |
| 10 | **Cuekit** | Descriptive (cues + toolkit) | T | T | free | T | T | user exists, 0 repos | free / **npm taken** | none | 9 small GitHub repos named cuekit | Good meaning; poor domain/npm situation |
| (note) | **Dotted `Streamer.xxx` variants** | Would keep the Streamer.live spirit | — | — | — | — | — | — | — | — | `streamer.*` dotted names are almost all taken (see §1.1 row 8) | A dotted-TLD brand like Streamer.bot/Streamer.live is not realistically obtainable now; the "stream + craft" names above are the nearest legal substitutes |

**Recommendation on style:** pick a **coined or compound name that carries no `streamer.`/`.bot` echo** (#1, #2, #3). If the owner wants to keep the "Streamer.live" *spirit* (stream + live), #1 Streamwright and #4/#5 (which contain "wright"/"live") are the closest. A tagline like "the local-first automation suite for streamers" does the descriptive work, so the name itself does not need to.

Before final pick, still do (a) a manual EUIPO/UKIPO check, (b) a Twitch/Discord/Google OAuth app-name uniqueness check (the Twitch app name must be globally unique, per `docs/owner/registrations/twitch.md`), (c) a quick Google/Bing "name + streaming" sanity search, (d) optional attorney clearance (a few hundred dollars for a knockout search **(unverified)**).

### 1.6 Package / ID naming given the studio "Tech Daddy Digital"

**Facts found**

| Item | Status | Source |
|---|---|---|
| GitHub org | **`Tech-Daddy-Digital` exists** (display name "Tech Daddy Digital", created 2025-05-15, location USA, 5+ public repos incl. `The-Miki-Komori-Project` GPL-3.0 VTuber visual novel, `DiscordNotifier` MIT, `EnchantMaster` MIT; 7 members). Handle **`techdaddydigital`** (no hyphens) returns 404 for users and orgs, i.e. unclaimed; `TechDaddyDigital` also 404; `techdaddy` is an unrelated old user | `gh api orgs/Tech-Daddy-Digital`, `users/...` |
| Domains | **`techdaddydigital.com`** registered 2022-06-15 at Cloudflare Registrar, expires 2027-06-15, registrant redacted (presumably the owner, **(unverified)**). The apex currently fails HTTPS with a TLS "unrecognized name" error and serves a ~111-byte page over HTTP, so it has no real site yet. **`.digital`, `.dev`, `.app`, `.net`, `.io` are all unregistered** | RDAP / whois / probes |
| USPTO | No `TECH DADDY DIGITAL` mark | tmsearch backend |

**Recommendations**

| Identifier | Recommendation | Reasoning / constraint |
|---|---|---|
| **Order of operations** | **Finalize the product name first**, then lock IDs | The Tauri `identifier` becomes the OS bundle ID and the **webview data directory** path ([Tauri config](https://v2.tauri.app/reference/config/)); the Flathub app-ID is permanent once published; Twitch Client ID and Google OAuth consent-screen names are re-verification triggers (see `docs/owner/registrations/google-youtube.md`) |
| **Tauri `identifier`** | `com.techdaddydigital.<product>` (lowercase, e.g. `com.techdaddydigital.streamwright`) | Allowed chars: alphanumerics, hyphens, periods; must be unique; `com.tauri.dev` is rejected. Avoid an ID whose last segment is `app` (macOS bundle-extension clash; Tauri CLI warns, **(from memory, not in the fetched docs)**) |
| **Tauri `publisher` / `copyright`** | `publisher: "Tech Daddy Digital"`; `copyright: "Copyright © 2026 Tech Daddy Digital and contributors"` | `publisher` otherwise defaults to the *second element of the identifier* (`techdaddydigital`), which looks sloppy in Windows installers; it maps to the MSI Manufacturer / Debian Maintainer fields ([Tauri config](https://v2.tauri.app/reference/config/)). Add SPDX headers (`SPDX-FileCopyrightText: 2026 Tech Daddy Digital`, `SPDX-License-Identifier: GPL-3.0-or-later`) |
| **Flatpak app-id / MetaInfo `id`** | `com.techdaddydigital.<Product>` (CamelCase last segment) | Flathub: 3–5 components, letters/digits/underscore only (dash only in the last component), must equal the MetaInfo `id`, must **not end in `.app`, `.desktop`, `.linux`**, and the domain **must be one you control**, reachable over HTTPS, verifiable via a token at `/.well-known/org.flathub.VerifiedApps.txt` ([Flathub requirements](https://docs.flathub.org/docs/for-app-authors/requirements)). So **fix HTTPS on techdaddydigital.com before submitting**. No-domain fallback: `io.github.Tech_Daddy_Digital.<Product>` (≥4 components; dashes in the org name become underscores, **(inferred from the component rule)**). `com.github.*` is reserved for GitHub's own projects |
| **Windows publisher** | The name in the Authenticode certificate, not the Tauri `publisher` field | **SignPath Foundation** (free for OSS): the certificate is issued to SignPath Foundation, so users see "SignPath Foundation" ([SignPath terms](https://signpath.org/terms)); requires an OSI license **without commercial dual-licensing for any component**. **Azure Artifact Signing** (GA 2026-01, ~$9.99/month): publisher is your validated legal name; individuals only in the US/Canada, organizations in US/Canada/EU/UK ([Microsoft Learn](https://learn.microsoft.com/azure/artifact-signing/quickstart)). Either way "Tech Daddy Digital" appears as the publisher only if it is a **legal entity** (or you accept your personal legal name). See also `docs/owner/registrations/distribution-and-signing.md` |
| **winget** | `PackageIdentifier: TechDaddyDigital.<Product>` | Manifest path is `manifests/t/TechDaddyDigital/<Product>/<version>/`; the installer URL must come **directly** from the publisher's release location over HTTPS (GitHub Releases is fine), no redirectors ([Microsoft Learn](https://learn.microsoft.com/en-us/windows/package-manager/package/repository)) |
| **AUR** | `<product>-bin` first (repackage the release), then `<product>` (build from source tarball); optional `<product>-git` | Standard AUR naming convention (**from memory; the Arch wiki page blocked automated fetching**). AUR RPC search shows none of the shortlisted names currently in use. Existing runbook assumes `streamer-live-bin` |
| **GitHub** | Keep the existing org **`Tech-Daddy-Digital`** as the parent; claim **`techdaddydigital`** (free) defensively or rename the org to it (renames leave redirects for repo URLs but can break Pages/Actions references, so test first). For the product, either keep the repo under the studio org, or create an org named after the final product (check the handle; `streamwright` is taken by an empty org) | Q-B4 already leans toward a studio org. Note the studio org already hosts GPL-3.0 and MIT repos, consistent with the proposed GPL app + MIT/Apache SDK split |
| **Domains** | Use `techdaddydigital.com` for studio/legal pages; buy the product domain once named (`.app`/`.dev`/`.live`/`.io` variants are free for most alternatives). Optionally register `techdaddydigital.dev` and `.digital` as cheap defensive buys | `.dev`/`.app` force HTTPS (HSTS-preloaded); fine for this project |
| **Steam / Apple / Azure legal name** | Needs a real legal identity: Steam says "Don't use a DBA, nickname, or friendly name"; individuals register as a sole proprietorship under their legal name ([Steamworks onboarding](https://partner.steamgames.com/doc/gettingstarted/onboarding)). Store pages can still show "Tech Daddy Digital" as developer/publisher text **(unverified)** | **Open question for the owner:** is Tech Daddy Digital an LLC / registered DBA / unregistered brand? |

---

## 2. Part 2 — Distribution channels for non-game software

### 2.1 Steam

#### 2.1.1 Policy, cost, process

| Topic | Finding | Source |
|---|---|---|
| Does Steam take non-game software? | Valve is "**primarily accepting games**". Software is accepted only in listed categories: Animation & Modeling, Audio/Video Production, Design & Illustration, Photo Editing, Educational & Tutorials, Finance & Accounting, **Player Tools**, SteamVR Tools. A streaming-automation app most plausibly fits **Audio/Video Production** or **Player Tools** (OBS Studio is tagged Utilities / Video Production). There is no pre-approval: the $100 fee is spent before Valve's review tells you the verdict (refund policy for a rejected app **(unverified)**) | [Steam Direct](https://partner.steamgames.com/steamdirect), [onboarding](https://partner.steamgames.com/doc/gettingstarted/onboarding) |
| Application type | **Software** is a separate app type from Game/DLC/Demo (set in App Admin) | [Application docs](https://partner.steamgames.com/doc/store/application) |
| Prohibited | Apps with **advertising-based business models**, blockchain apps that issue/exchange crypto or NFTs, malware/credential phishing | Steam Direct page |
| Fee | **$100 per product**, non-refundable, not payable with Steam Wallet funds; recouped from later payouts once the product has **≥ $1,000 adjusted gross revenue**. The page does not exempt free apps and free apps clearly ship (OBS, VTube Studio), so assume you pay it once per app ID; recoupment is irrelevant for a free app (a sunk $100) | Steam Direct page |
| Revenue share | Irrelevant while free. Any paid DLC/in-app purchase must use Steam Wallet and Valve's cut applies (30% standard; **(unverified, not stated on the fetched pages)**) | review-process doc |
| Paperwork | Sign NDA + Steam Distribution Agreement, **legal name (no DBA)**, business bank account matching the legal name, tax questionnaire, identity verification: reported **15–30 days**, tax details locked until verified | Steam Direct, onboarding |
| Waiting periods | After paying the fee, first titles wait **21 days** (onboarding doc) or **30 days** (Steam Direct page): the docs disagree; plan for 30. A public "coming soon" page must be up **≥ 2 weeks** before release | both pages |
| Review | Store page review **3–5 business days** (plan 7); build review **3–5 business days** (plan 7). Valve checks that the product **starts on every OS listed**, that **every listed feature is implemented**, that in-app transactions use Steam Wallet, descriptions have **no links to other websites**, screenshots show real product (not concept art). Updates after approval are not re-reviewed | [Review process](https://partner.steamgames.com/doc/store/review_process) |
| Donations / Patreon links | Store review says in-product transactions must use Steam Wallet and the product "cannot link to stores that don't offer it". A secondhand report says a Patreon link was flagged by automated review for a free app and passed after routing through an intermediate page: anecdotal, **(unverified)**. Plan **no donation link inside the Steam build**, or keep it off-app | review-process doc; community thread |
| DRM-free / runs without Steam | OBS's Steam build is "the exact same version as you would download from our website" and uses no Steamworks code, so it does not need the Steam client **(inferred)**. Valve's DRM page treats the Steam wrapper as optional but does not state it explicitly | [OBS Steam FAQ](https://obsproject.com/kb/steam-faq), [Steam DRM](https://partner.steamgames.com/doc/features/drm) |
| Updates | **SteamPipe**: depots per OS, files split into ~1 MB chunks, delta patches, `steamcmd +run_app_build`. **Default branch cannot be set live from the build script** (only beta branches via `setlive`), so promoting a release to everyone is a manual click in Steamworks. CI action exists: `game-ci/steam-deploy` (MIT, 345★, pushed 2026-10-08) | [SteamPipe docs](https://partner.steamgames.com/doc/sdk/uploading), [repo](https://github.com/game-ci/steam-deploy) |
| Overlay | The Steam overlay is **automatically disabled for "Software" app types** (a developer can opt in under Installation). Separately, overlay hooking does not work with WebView2 apps (Tauri on Windows) without a workaround plugin, so there is nothing to fix | [Overlay docs](https://partner.steamgames.com/doc/features/overlay); [WebView2 issue](https://github.com/MicrosoftEdge/WebView2Feedback/issues/3200) |
| Steam Deck | Deck Verified criteria are about controller input, 1280×800 legibility, and no launcher/compat warnings: written for games. A mouse-and-keyboard streaming tool will not get a Verified badge easily and Deck is not a target for this product **(unverified, secondary sources only)** | [GamingOnLinux summary](https://www.gamingonlinux.com/2023/05/steam-deck-hits-over-9000-games-rated-verified-and-playable/) |

#### 2.1.2 Linux: Steam Linux Runtime vs a Tauri/WebKitGTK app

Valve's own README now says: *"Newer native Linux games can run in … Steam Linux Runtime 4.0 (steamrt4) … the recommended environment for developers of new native Linux games"*; sniper (SLR 3.0) is "an older runtime"; scout (SLR 1.0) is still the publish-time default. You pick the runtime in **Steamworks → Installation → Linux Runtime** after defining a Linux launch option ([steam-runtime README](https://github.com/ValveSoftware/steam-runtime/blob/master/README.md)).

I inspected the actual package manifests of both container runtimes (public-beta channel images dated 2026-10-07; the stable channel may differ slightly, **(unverified)**):

| Runtime | Base | WebKitGTK / JavaScriptCore | GTK3 | libsoup | Python | libdbus | libayatana-appindicator |
|---|---|---|---|---|---|---|---|
| **sniper** (SLR 3.0), `3.0.20261007.263965` | Debian 11 family (python3.9, openssl 1.1) | **absent** | present (`libgtk-3-0`) | 2.4 only | python3.9 inside the container | present | **absent** |
| **steamrt4** (SLR 4.0), `4.0.20261007.263973` | Debian 13.7 (python3.13) | **absent** | **absent** (no GTK3, no GTK4) | 3.0 | python3.13 inside the container | present | **absent** |

Source: `https://repo.steampowered.com/steamrt3/images/latest-container-runtime-public-beta/` and `…/steamrt4/images/latest-container-runtime-public-beta/` (`*.manifest.dpkg`).

Implications for a **Tauri 2 (GTK3 + WebKitGTK 4.1)** app:

1. **Neither runtime provides WebKitGTK, and steamrt4 has no GTK3 at all.** A native Linux Steam depot must therefore **bundle the whole GTK3 + WebKitGTK + libsoup3 stack** (that is what a Tauri AppImage does; Tauri's own docs say an AppImage grows from 2–6 MB to 70+ MB, see `tech-stack.md`). Tauri's docs also say to build on the oldest base that has WebKitGTK 4.1, which is a poor match for Steam's two container bases.
2. **The tray icon** needs `libayatana-appindicator` / `libappindicator-gtk3` on GTK-based Tauri: absent in both runtimes, so bundle it or switch to a pure-Rust StatusNotifierItem implementation.
3. **System Python:** the container's Python is the runtime's own (3.9 or 3.13), not the user's. The host's Python, venvs and site-packages are not what the app would see by default **(unverified: depends on pressure-vessel's host-path sharing)**. The plan in `tech-stack.md` to use the user's Python *or* download `python-build-standalone` is the right one; for Steam, ship/download the standalone build.
4. **D-Bus / portals:** `libdbus` is present and the session bus is normally forwarded into the container **(unverified)**, so a pure-Rust `zbus`/`ashpd` client for the GlobalShortcuts portal should still work; confirm in a test depot.
5. **Local ports** (the engine's `127.0.0.1` HTTP/WebSocket server for overlays, OBS WebSocket client): pressure-vessel does not isolate the network namespace **(unverified, from memory)**, so loopback should work; test it.
6. **Steam Linux build review** requires the build to start on every OS listed on the store page. A broken Linux depot would block the whole app.
7. **Cheaper alternatives to bundling WebKitGTK:** (a) the planned **Tauri 3 CEF runtime** (self-contained Chromium; "hundreds of MB larger", per `tech-stack.md`), (b) the **headless daemon** (`streamer-lived`) plus "open UI in the system browser", which sidesteps the webview entirely, though launching a browser from inside the container needs testing, (c) simply not shipping Linux on Steam at first.

**Precedent:** OBS Studio shipped on Steam for Windows and macOS only and says "Currently we have no plans for providing a Linux build via Steam"; it sends Linux users to Flatpak ([OBS FAQ](https://obsproject.com/kb/steam-faq), [GamingOnLinux](https://www.gamingonlinux.com/2022/03/obs-studio-arrives-on-steam-but-no-linux-build-for-now-so-stick-to-flatpak)). The Steam store API (2026-10-19) still shows OBS Studio as Windows + macOS only. Blender (GPL) and Godot ship native Linux depots, but they are C/C++ apps that bundle their own libraries.

#### 2.1.3 Steam Workshop and GPL vs the Steamworks SDK

| Question | Finding |
|---|---|
| Can Workshop work without the SDK? | **No for uploads and subscription awareness.** Valve: "To upload content to the Workshop, you'll need to integrate the ISteamUGC API", and Ready-to-use Workshops also need it. Only `steamcmd +workshop_build_item` exists as an SDK-free upload path, and Valve limits it to testing because it needs Steam credentials ([Workshop](https://partner.steamgames.com/doc/features/workshop), [implementation](https://partner.steamgames.com/doc/features/workshop/implementation)). The Steam client downloads subscribed items to disk itself, so an app *could* read item folders without the SDK **(unverified; don't rely on it)** |
| Is the SDK GPL-compatible? | **Treat as incompatible.** The SDK is proprietary. GNU's FAQ: combining GPL code with a GPL-incompatible library needs explicit permission from the copyright holders, and "Only the copyright holders for the program can legally release their software under these terms"; the system-library exception does not cover a library you ship with the app ([GPL FAQ](https://www.gnu.org/licenses/gpl-faq.html)). Not court-tested; settled in practice as "don't link" |
| What do GPL apps on Steam do? | **OBS Studio:** "Due to OBS Studio being licensed under the GPL, we are not able to utilize the Steamworks SDK to offer such features" (Workshop, achievements); plugins "cannot be distributed through Steam Workshop"; no cloud saves ([OBS Steam FAQ](https://obsproject.com/kb/steam-faq)). **Blender (GPL):** on Steam, 92,796 reviews, no SDK features known to me **(unverified)**. **The Powder Toy (GPLv3):** maintainers concluded they could not list it unless via a separate executable for SDK calls or relicensing ([thread](https://powdertoy.co.uk/Discussions/Thread/View.html?Thread=23044&PageNum=0)). **Mindustry (GPL-3.0)** ships Steam achievements/Workshop; it is single-author, and the author reportedly said Valve never cared about the license: secondhand, **(unverified)** ([GamingOnLinux](https://www.gamingonlinux.com/2019/09/mindustry-the-factorio-like-tower-defense-game-is-now-on-steam/page=1)) |
| Workarounds | (1) **Separate process:** a closed-source (or separately licensed) helper links the SDK and talks to the GPL app over pipes/sockets; GNU says "pipes, sockets and command-line arguments are communication mechanisms normally used between two separate programs", but "arm's length" is a judgement call. `icculus/steamshim` (Zlib) implements this but only does stats/achievements today, you would extend it for `ISteamUGC` ([steamshim](https://github.com/icculus/steamshim)). (2) **Copyright-holder exception:** if Tech Daddy Digital holds *all* copyrights, add a GPLv3 §7 additional permission allowing linking with the Steamworks SDK (and ship Steam builds under it). This only works if outside contributors sign a **CLA / copyright assignment** or contributions stay out of the linked parts; a DCO alone does not give you relicensing power. (3) **Skip the SDK** and do what OBS does. Decide this **now, while contributors = 1**, because retrofitting needs everyone's consent |
| Conflict with free signing | SignPath Foundation requires an OSI license with **no commercial dual-licensing for any component** ([SignPath terms](https://signpath.org/terms)). A Steamworks §7 exception is not a commercial license, but ask SignPath before relying on both |
| Alternative to Workshop | Ship a first-party **registry** for overlays/alert packs/plugins (planned in the plugin SDK research; Streamer.bot has `extensions.streamer.bot`), hosted as static files + GitHub Releases. Add Workshop mirroring later only if there is demand and the licensing is settled |

#### 2.1.4 Streaming and VTuber tools on Steam (store API snapshot, 2026-10-19)

| App | Developer | Price | Platforms | Reviews (positive %) | Notes |
|---|---|---|---|---|---|
| OBS Studio | OBS Project | Free | Win, Mac (no Linux) | 58,600 (95%) "Overwhelmingly Positive" | Released on Steam 2022-03-21; identical to website build; GPL ⇒ no Workshop/achievements |
| VTube Studio | DenchiSoft | Free (+ paid "Remove Watermark" DLC) | Win, Mac | 7,423 (93%) "Very Positive" | Has **Steam Workshop** |
| Warudo | Hakuya Labs | Free (Early Access) | Win | 683 (92%) "Very Positive" | Steam Workshop |
| Animaze by FaceRig | Holotech Studios | Free + in-app purchases | Win | 3,797 (69%) "Mixed" | |
| 3tene | PLUSPLUS | Free | Win, Mac | 328 (87%) | |
| PNGTuber Plus | kaiakairos | Free | Win | 604 (83%) | |
| PlayClaw 7 | Eduard Kozadaev | Free | Win | 834 (85%) | Overlay / recording tool |
| Blender (GPL) | Blender Foundation | Free | Win, Mac, **Linux** | 92,796 (96%) | |
| Godot Engine (MIT) | Godot | Free | Win, Mac, Linux | 9,974 (96%) | |
| Krita (GPL) | Krita Team | $9.99 | Win, Mac, Linux | 8,419 (96%) | |
| **Not found** by store search | Streamer.bot, SAMMI, Lumia Stream, Voicemod, VSeeFace, VNyan, Touch Portal, Meld Studio, Streamlabs Desktop, XSplit, T.I.T.S. | | | | The search endpoint can miss obscure listings, so **(unverified)**; but none of the direct automation competitors appears to be on Steam, which means Steam would be **unclaimed ground** for this category |

Reviews are all-language, all-purchase-type totals from `store.steampowered.com/appreviews`.

#### 2.1.5 Advantages vs disadvantages

| Advantages | Disadvantages |
|---|---|
| **Discoverability and trust** among streamers/VTubers who already live in Steam; free apps with 90%+ reviews (OBS, VTube Studio) show appetite | **Dual update channel**: SteamPipe vs Tauri updater; must compile the updater out for Steam builds |
| **Free, automatic, delta updates** with beta branches (good for a public beta channel) | **$100 + legal-entity paperwork + 21–30 day waits + 2-week coming-soon**, even for a free app |
| Reviews and community hub as a free support/feedback surface | **Valve's software policy is "primarily games"**; a rejection after paying is possible |
| Steam Workshop for overlays/alert packs/plugins (VTube Studio and Warudo prove it works for creator tools) | **Workshop needs the proprietary SDK**, which conflicts with GPL unless you hold all copyrights and add an exception or use a helper process |
| Steam Deck / SteamOS exposure (limited relevance) | **Linux on Steam is awkward for Tauri**: no WebKitGTK/GTK3/appindicator in the runtimes, so you bundle or switch webview |
| No store fee or revenue cut for a free app | **Store obligations**: no links to other sites in the description, no external payment links, Steam Wallet only for transactions, ongoing review-and-community moderation, "software must work on every OS you list" |
| Steam users are mostly Windows: matches the largest streamer segment | Steam-only users miss the Linux/macOS builds; you must still maintain GitHub Releases etc. |

#### 2.1.6 Steam recommendation

- **Do Steam, but after v1.0**, as a free **Software** app, **Windows first (macOS if signed/notarized)**, **no Steamworks SDK**, **no Workshop**, **no Linux depot** in the first Steam release. This is exactly the OBS pattern and avoids the GPL problem entirely.
- **Start the paperwork early** (legal entity or sole-proprietor identity, bank, tax) because the clock is 15–30 days plus 21–30 days plus 2 weeks of "coming soon"; you can run it in parallel with the beta.
- **Decide the licensing posture before accepting outside contributions**: either (A) pure GPL-3.0-or-later and accept "no SDK" forever, or (B) GPL-3.0-or-later plus a CLA and a §7 Steamworks exception to keep Workshop/achievements possible. (B) costs contributor friction; (A) costs Workshop.
- Linux on Steam only when a CEF or bundled-WebKit build is proven in a test depot, or if the headless-daemon + browser path is acceptable.

### 2.2 itch.io

| Topic | Finding | Source |
|---|---|---|
| Fit | Accepts downloadable apps and tools (classification list includes "Tool" **(from memory; unverified)**). No onboarding fee, no review gate, no legal-entity requirement beyond payout identity. Audience skews indie-game/creator, which overlaps VTubers: **veadotube mini** and **PNGTuber Plus** are both distributed there, with Windows, macOS and Linux builds | [veadotube mini on itch.io](https://olmewe.itch.io/veadotube-mini/purchase), [veado.tube docs](https://veado.tube/docs/install/) |
| Upload tool | **`butler`** (MIT, <https://github.com/itchio/butler>): `butler push <dir-or-zip> <user/game>:<channel>`. Channel names set OS tags: `windows`/`win`, `linux`, `mac`/`osx`, `android`; multi-platform names like `win-linux-mac-stable` work; convention is lowercase kebab-case, e.g. `windows`, `linux`, `osx`, plus `*-beta`. `--userversion 1.2.3` records your own version | [butler pushing](https://itch.io/docs/butler/pushing.html) |
| Patching | Wharf generates a default patch immediately and an optimized patch later (can take ~30 min for large builds) | same page |
| Updates | The **itch app** (desktop client for Windows, macOS, Linux) installs and updates builds from channels. **Players who download directly from the website do not get automatic updates**; itch exposes `GET https://api.itch.io/wharf/latest?channel_name=…&game_id=…` (no auth) so your app can check for a newer `--userversion` and notify | butler pushing page; [itch app docs](https://itch.io/docs/itch/) |
| Manifest | Optional `.itch.toml` to control launch actions and prerequisites (e.g. VC++ runtime) | [manifest](https://itch.io/docs/itch/integrating/manifest.html) |
| Money | Pricing modes: free, **pay-what-you-want** (price is the minimum, can be $0), "$0 or donate", paid, no payments. Revenue share is **10% by default and creator-adjustable 0–100%**; processors PayPal/Stripe ~2.9% + $0.30; $3 one-time tax-identity fee; $5 minimum payout | [pricing](https://itch.io/docs/creators/pricing), [payments](https://itch.io/docs/creators/payments) |
| CI | `butler` with an API key in a CI secret; community actions `manleydev/butler-publish-itchio-action` (145★, pushed 2026-03-08) and `Ayowel/butler-to-itch` (13★) exist. I did not fetch butler's login page, so the exact env var (`BUTLER_API_KEY`) is **from memory** | GitHub |
| Linux | Native Linux channel supported; Linux users on itch are used to extracting a zip/AppImage and running it (veadotube's Linux guide: extract, `chmod +x`) | veado.tube docs |

**Recommendation:** list on itch.io **at launch** as the "discovery + donate" page (PWYW with $0 minimum, 0–10% revenue share), pushing the same artifacts as GitHub Releases via butler in CI. Ship the itch build with the **in-app updater disabled** (or notify-only against the wharf endpoint), because the itch app owns file integrity for itch-app installs.

### 2.3 GitHub Releases, AUR, winget, Homebrew, Flathub (brief)

| Channel | Fit for this app | Effort | Update path | Notes |
|---|---|---|---|---|
| **GitHub Releases** | Canonical artifact store; free; works for Windows (NSIS/MSI), macOS (dmg/app.tar.gz), Linux (AppImage/deb/rpm). `tauri-apps/tauri-action` v1.0.0 (2026-06-29) builds and attaches them and generates the updater `latest.json` | Low | **Tauri updater** (AppImage, MSI, NSIS, macOS `.app.tar.gz`; **not deb/rpm**; signing mandatory, key loss = no updates) | The source of truth for everything below |
| **AUR** (`<product>-bin`, later source) | Excellent for the owner's Arch/Omarchy audience; `-bin` just wraps the release asset | Low | **pacman** (updater off) | `KSXGitHub/github-actions-deploy-aur` (181★) can publish from CI |
| **winget** | Good reach for Windows power users | Low–Med | `winget upgrade` (updater can stay on; harmless) | PR to `microsoft/winget-pkgs`; validation runs the installer unattended, scans it with Defender/AV, requires HTTPS installer URL **directly from the publisher**, silent install and clean uninstall. Automation: `vedantmgoyal9/winget-releaser` (317★, pushed 2026-10-13), Komac, wingetcreate. Unsigned builds can still pass but may trip AV heuristics |
| **Homebrew** | Cask for macOS | Med | `brew upgrade` (updater off) | Homebrew announced that casks failing **Gatekeeper checks (signed + notarized) would be disabled on 2026-09-01** and `--no-quarantine` deprecated; I could not confirm what happened after the cutoff, so **assume notarization is required** ([Homebrew 5.0.0 coverage](https://workbrew.com/blog/what-homebrew-5-0-0-means-for-your-mac-fleet)). Needs the $99/yr Apple Developer Program (see `distribution-and-signing.md`) |
| **Flathub** | Best Linux reach after AUR; sandboxed; discoverable in GNOME Software / KDE Discover / SteamOS Discover | **High** | **Flatpak** (updater off) | Must **build from source with no network** (Rust + Node deps vendored via generators), mandatory MetaInfo, XDG portals preferred, "host-dependent apps relying on host components are rejected", insecure designs rejected; the app-ID/domain rules in §1.6. Concerns for this app: running arbitrary user JS/Python (sandbox has its own runtime Python, no host site-packages), loopback server (`--share=network`), tray (`--talk-name=org.kde.StatusNotifierWatcher`), global hotkeys (portal), `/dev/input` evdev (`--device=input`). Tauri has an official Flathub guide ([v2.tauri.app/distribute/flatpak](https://v2.tauri.app/distribute/flatpak/)). GPL-3.0 is fine |

### 2.4 Recommended channel strategy

| Phase | Channels | Updater behaviour |
|---|---|---|
| **Alpha/Beta (pre-1.0)** | GitHub Releases (pre-release tag) only; AUR `-git`/`-bin` for the owner's own use | Tauri updater ON, "beta" channel opt-in |
| **v1.0 launch** | GitHub Releases + **itch.io** + **AUR `-bin`** + **winget** (Windows) | Tauri updater ON for GitHub/winget builds; OFF for itch (notify-only) and AUR |
| **v1.1–1.3** | **Flathub**; **Homebrew cask** once macOS signing/notarization exists (Q-P2) | OFF (package manager owns updates) |
| **v1.x (after stable)** | **Steam** (Windows, then macOS; Linux only after a CEF/bundled test depot; no SDK/Workshop unless §7 exception is adopted) | OFF; Steam owns updates, with a beta branch for release candidates |

**One update source of truth**

1. **One tag, one CI run.** A tag `vX.Y.Z` triggers a single workflow that builds all artifacts once and publishes them to **GitHub Releases**, which is the canonical store and hosts the Tauri `latest.json`. Every other channel is a downstream job that consumes the *same* binaries: `butler push` (itch), `steamcmd` (Steam), `winget-releaser` / Komac, Homebrew cask bump, AUR bump, Flathub manifest PR. Version number lives in one place (Cargo workspace version → `tauri.conf.json`).
2. **Compile-time distribution flag.** Add a Cargo feature per channel (`dist-github` default, `dist-itch`, `dist-steam`, `dist-aur`, `dist-flatpak`, `dist-brew`). Only `dist-github` registers `tauri-plugin-updater`; the other builds omit the plugin, omit `updater:default` from capabilities, and don't generate updater artifacts (Tauri docs describe registration under `#[cfg(desktop)]` and capability gating; there is **no single "disable updater" switch**, so the feature-flag approach is the clean one).
3. **UI honesty.** The Updates screen reads `distribution_channel()` and shows either the self-updater or "Managed by Steam / itch / pacman / Flatpak / Homebrew", plus an optional **notify-only** check against the GitHub Releases API (or itch's wharf `latest` endpoint) for builds that cannot self-update. No silent network calls to update endpoints in Steam builds.
4. **Same identifier, same data directory** across channels (except Flatpak's sandboxed paths), so users can switch channels without losing profiles; ship export/import for the Flatpak case.
5. **Updater key hygiene.** The Tauri updater private key is mandatory and unrecoverable; keep an offline backup (`docs/owner/registrations/distribution-and-signing.md`).

---

## 3. Open questions for the owner

1. **Name:** given §1 (canonical domain unobtainable, existing "Streamer.live" project, same niche as Streamer.bot), will you move to one of the alternatives (Streamwright, NekoStage, StreamDen, Livewright, TanukiLive) and keep "Streamer.live" as an internal codename? Is a VTuber-flavoured name (NekoStage/TanukiLive) or a neutral one preferable?
2. **Intent:** are you comfortable dropping "similar to Streamer.bot" as a naming goal and positioning as "alternative to / imports from Streamer.bot" instead?
3. **Legal entity:** is Tech Daddy Digital an LLC, a registered DBA, or an unregistered brand? Steam (no DBA), Azure Artifact Signing and Apple all need a legal identity.
4. **Domain control:** do you control `techdaddydigital.com` (Cloudflare, since 2022)? Can it serve HTTPS on the apex? Do you want `.digital` / `.dev` defensive registrations?
5. **GitHub:** keep `Tech-Daddy-Digital` as the parent org, claim/rename to `techdaddydigital`, and/or create a product org after the name is chosen?
6. **License posture (Q-B2/Q-B5):** (A) pure GPL-3.0-or-later (Steam without SDK/Workshop, OBS-style) or (B) GPL plus CLA plus §7 Steamworks exception (keeps Workshop possible)? Does anyone besides you already own copyright in the code?
7. **Steam scope:** OK with Windows-first, no Linux/Workshop at the first Steam release, and paying the $100 + paperwork even though the app is free? Is a post-1.0 timing acceptable?
8. **Acquisition:** worth trying to buy/back-order `streamer.live` (expires 2026-10-29; held by Switchboard Live) or `streamerlive.com` (BrandBucket) if you still want the name? Budget ceiling?
9. **Legal check:** want an attorney knockout/clearance search (US + EU) on the final shortlist before baking IDs, and a trademark filing after launch?
10. **Contact:** would you like to reach out to the Streamer.bot team as a courtesy before launch?

---

## 4. Sources (primary ones used)

- Domain/registry: RDAP via `rdap.identitydigital.services`, `pubapi.registry.google`, `rdap.verisign.com` (queried 2026-10-19); <https://switchboard.live/>; <https://getstreamer.live/>; <https://github.com/crimsonstrife/streamer.live>
- Trademarks: USPTO tmsearch backend (`tmsearch.uspto.gov`); <https://trademarks.justia.com/search?q=streamer+bot>; <https://streamer.bot>; <https://streamer.bot/terms>
- Steam: <https://partner.steamgames.com/steamdirect>; <https://partner.steamgames.com/doc/gettingstarted/onboarding>; <https://partner.steamgames.com/doc/store/application>; <https://partner.steamgames.com/doc/store/review_process>; <https://partner.steamgames.com/doc/features/overlay>; <https://partner.steamgames.com/doc/features/workshop>; <https://partner.steamgames.com/doc/features/workshop/implementation>; <https://partner.steamgames.com/doc/sdk/uploading>; <https://partner.steamgames.com/doc/store/application/platforms/linux>; <https://github.com/ValveSoftware/steam-runtime/blob/master/README.md>; <https://repo.steampowered.com/steamrt3/images/latest-container-runtime-public-beta/>; <https://repo.steampowered.com/steamrt4/images/latest-container-runtime-public-beta/>; Steam store/appreviews APIs; <https://obsproject.com/kb/steam-faq>; <https://github.com/icculus/steamshim>; <https://www.gnu.org/licenses/gpl-faq.html>; <https://powdertoy.co.uk/Discussions/Thread/View.html?Thread=23044&PageNum=0>
- itch.io: <https://itch.io/docs/butler/pushing.html>; <https://itch.io/docs/creators/pricing>; <https://itch.io/docs/creators/payments>; <https://itch.io/docs/itch/integrating/manifest.html>
- Packaging: <https://v2.tauri.app/plugin/updater/>; <https://v2.tauri.app/reference/config/>; <https://docs.flathub.org/docs/for-app-authors/requirements>; <https://learn.microsoft.com/en-us/windows/package-manager/package/repository>; <https://signpath.org/terms>; <https://learn.microsoft.com/azure/artifact-signing/quickstart>; <https://workbrew.com/blog/what-homebrew-5-0-0-means-for-your-mac-fleet>
- Context in this repo: `docs/QUESTIONS.md` (Q-B1, Q-B2, Q-B4, Q-B5), `docs/research/tech-stack.md`, `docs/owner/registrations/distribution-and-signing.md`
