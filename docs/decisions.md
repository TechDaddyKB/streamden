# Decision Log

One line per decision, **newest first**. Format: `YYYY-MM-DD · D-NNN · decision · source (Q-id / ADR / PR)`.
Only record decisions the **owner** made or approved. Proposals live in ADRs with status *Proposed*.

| Date | ID | Decision | Source |
|---|---|---|---|
| 2026-10-18 | D-034 | Commit messages follow the owner's fleet policy: one Conventional Commit subject line that names the card, e.g. `feat(core): bounded action queue (DEN-8)`. No body, no `Task:`/`Agent:` trailers, no AI co-author lines. The vendor is visible in the branch name. | Owner fleet policy |
| 2026-10-18 | D-033 | TTS at launch (P2): Piper + Kokoro (local, offline via sherpa-onnx) + OS voices + ElevenLabs + Azure. Other engines come as plugins. | Q-T1 |
| 2026-10-18 | D-032 | Monetization: free and open source; the official hosted relay is funded by donations. No feature paywalls. | Q-B3 |
| 2026-10-18 | D-031 | YouTube: users bring their own Google project through a guided wizard; a shared project is optional. No scraping in core. | Q-C2 |
| 2026-10-18 | D-030 | MCP server: off by default, small core toolset (Lean profile + search/describe/invoke), in-app approval for destructive tools, AI-created config saved as disabled drafts. Code execution and ending the stream are never exposed over MCP. | Q-A2 |
| 2026-10-18 | D-029 | Script trust: the user's own scripts get full access. Imported or community scripts run sandboxed (isolated worker process, declared capabilities) behind a consent screen. S-3 showed in-process QuickJS can't be hard-stopped, so the isolated worker is mandatory for untrusted code. | Q-R2, S-3 |
| 2026-10-18 | D-028 | Editors: the Streamer.bot-style list/tree view ships first (P1). The node graph comes in P3 as a second, synchronized view. The engine gets a Parallel block so graphs can fan out and merge. | Q-U2 |
| 2026-10-18 | D-027 | Template syntax: new `{user.name}` style is canonical; Streamer.bot's `%var%` / `~global~` / `$fn()$` are accepted on import (converted). | Q-X3 |
| 2026-10-18 | D-026 | **Switch:** the codex-orchestrator board (project `streamden`) is the single board for work items; the Plane project was archived. Cards keep `DEN-<n>` numbering. Supersedes D-025. | Owner ("switch"), ADR-0005 |
| 2026-10-18 | D-025 | ~~Plane as the board~~ (superseded by D-026 the same day). Repo task files retired in favour of board items. | ADR-0005 (earlier draft) |
| 2026-10-18 | D-024 | Round-1 defaults accepted (no objection): Twitch P1 → YouTube/Kick/X/Bluesky/Discord-webhooks P2 → Rumble/Facebook P5 → TikTok as a community plugin. macOS unsigned best-effort until v0.5. LAN mode opt-in in P4. Python: system first, managed download as fallback. | Q-S1, Q-P2, Q-N1, Q-R1 |
| 2026-10-18 | D-023 | Owner's agent fleet: Claude Max, ChatGPT Pro (Codex), SuperGrok + Grok Build, all CLIs installed and launched through codex-orchestrator. | Q-G1 |
| 2026-10-18 | D-022 | Spikes S-1…S-5 approved, to start once name, license and git were settled (now true). | Q-G2 |
| 2026-10-18 | D-021 | AI: users' own API keys + "Connect your AI" over MCP + ChatGPT-plan sign-in. No Claude subscription login inside the app. | Q-A1 |
| 2026-10-18 | D-020 | Streamer.bot compatibility: import share strings + Streamer.bot-compatible WebSocket API + `CPH`-like JS/Python helpers now. Optional C# runner later (P6). | Q-X1 |
| 2026-10-18 | D-019 | Relay: open-source self-hostable relay (Cloudflare Worker) in P2, plus an **official hosted relay later**. | Q-C1 |
| 2026-10-18 | D-018 | UI: **React + TypeScript** (Vite). The plugin UI kit uses the same stack. | Q-U1 |
| 2026-10-18 | D-017 | Git now. **Public GitHub repo from day one**, developed under the owner's personal account **TechDaddyKB** and transferred to the **tech-daddy-digital** org (owner's, legally registered) at release. Identifiers, URLs and package metadata point at the org from day one. | Q-B4, ADR-0004 |
| 2026-10-18 | D-016 | License: **GPL-3.0-or-later** for the app. **MIT OR Apache-2.0** for SDKs, protocol schemas and plugin templates. DCO sign-off for external contributors, no CLA. | Q-B2, ADR-0003 |
| 2026-10-18 | D-015 | Product name: **StreamDen** (publisher Tech Daddy Digital). Internal crate prefix `den-`. Binaries `streamden` / `streamdend`, plugin CLI `den`. | Q-B1, ADR-0004 |
| 2026-10-18 | D-014 | Add Bluesky integration (posting, live status, triggers). | Owner request |
| 2026-10-18 | D-013 | Explore direct P2P tips (Venmo, Cash App, X Money) as donation triggers, on top of Streamer.bot's donation platforms. Research: payments-x-bluesky.md. | Owner request |
| 2026-10-18 | D-012 | Goal: run StreamElements custom widgets (e.g. Etsy-bought chat and alert overlays) natively, without an SE account, fed by our combined multichat. The designer follows the StreamElements editor's mental model. | Owner request |
| 2026-10-18 | D-011 | Goal: a node-based action editor, as an alternative to the Streamer.bot-style list (frequent community request). | Owner request |
| 2026-10-18 | D-010 | Distribution: GitHub Releases and an itch.io page planned. Steam is being evaluated (Q-B5). | Owner |
| 2026-10-18 | D-009 | Publisher/studio: **Tech Daddy Digital**. | Owner |
| 2026-10-18 | D-008 | Plan for a future GitHub-PR-based plugin registry (marketplace). Design manifests and signing for it from day one. | Owner request |
| 2026-10-18 | D-007 | Built-in MCP server exposing all app functions, plus AI provider integration (OpenAI, Anthropic, xAI). | Owner request |
| 2026-10-18 | D-006 | API-first: a public plugin SDK plus HTTP/WebSocket API; plugins must feel native. | Owner request |
| 2026-10-18 | D-005 | Keep all Streamer.bot/SAMMI app integrations and add similar ones, with a strong VTuber focus (Warudo, OpenSeeFace, VSeeFace, VNyan…). | Owner request |
| 2026-10-18 | D-004 | Combined multichat usable as an OBS dock, plus one WYSIWYG designer reused for the chat overlay and alerts, with word and bot filters. | Owner request |
| 2026-10-18 | D-003 | Execute-code sub-action supports JavaScript and Python (C# optional), with access to the action's variables, running locally. | Owner request |
| 2026-10-18 | D-002 | Targets: Windows, Linux (Debian, Fedora, Arch + AUR), macOS ("maybe"). | Owner request |
| 2026-10-18 | D-001 | Goal: one app with feature parity across Streamer.bot, Speaker.bot and SAMMI. | Owner request |
