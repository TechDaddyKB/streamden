# Delegation Guide — which AI agent does what, and how work is dispatched

> Audience: the **owner** (dispatcher), plus any agent that needs to know the process.
> Evidence and sources: [docs/research/ai-agent-fleet.md](../research/ai-agent-fleet.md) (benchmarks read 2026-10-18).
> **Routing below is provisional.** Lock it in after the S-5 bake-off (§6) and re-run the bake-off whenever a
> vendor ships a new flagship.

## 1. The fleet (as of 2026-10-18 — re-verify model names before use)

| Vendor | Tool | Models to use | Best at (evidence-weighted) | Avoid giving it |
|---|---|---|---|---|
| Anthropic | Claude Code | **Opus 5.5** (default), **Sonnet 5.5** (UI, docs, cheaper), Fable 5.1 (hardest designs, long unattended runs) | Architecture and specs; long multi-file Rust work; UI and visual bugs (#1 SWE-bench Multimodal); refactors and migrations | Security-adjacent tests that look like exploits (may be refused or rerouted); very mechanical bulk work (expensive) |
| OpenAI | Codex CLI / Codex cloud | **GPT-6.1 Sol** (workhorse, about 1/8 the cost per solved task), **GPT-6 Astra** (hardest problems), Luna (trivial bulk) | Protocol and algorithm code (parsers, rate limiters, EventSub/OBS framing); CI, packaging and ops (#1–2 SRE Bench); tests; bulk adapters; code review | Open-ended product design without a spec (Astra over-asks and over-formats) |
| xAI | Grok Build (`grok` CLI) | **Grok 4.7** | Live web and **X** research (API changes, platform news, community sentiment); quick small fixes; cheap third-opinion review | Long unsupervised implementation (Terminal-Bench 4.0 about 29% vs 55–65% for peers). Always route its PRs through Claude or Codex review. |

## 2. Routing by task type

| Task type | Primary | Reviewer | Confidence |
|---|---|---|---|
| Architecture, specs, ADRs | Claude Opus 5.5 (Fable 5.1 for the gnarliest) | GPT-6 Astra (adversarial critic) | Medium |
| Rust engine and systems code | Claude Opus 5.5 | GPT-6.1 Sol | Low–medium |
| Protocol, parsers, algorithms, rate limiting | GPT-6.1 Sol (Astra if hard) | Claude Opus 5.5 | Medium |
| TypeScript/React UI, designer | Claude Sonnet 5.5 (Opus for visual bug-fixing) | GPT-6.1 Sol | Medium |
| CI, packaging, release, build scripts | GPT-6.1 Sol / Astra | Claude Opus 5.5 | Medium |
| Tests | GPT-6.1 Sol, **for code another vendor wrote** | Claude Sonnet 5.5 | Low |
| Bulk adapters (integration #2…#N once the pattern exists) | GPT-6.1 Sol | Claude Sonnet 5.5 | High (cost) |
| Docs and user guides | Claude Sonnet 5.5 | GPT-6.1 Sol | Low–medium |
| Live API/platform research, X data | Grok 4.7 | Claude or Codex verifies against official docs | Medium–high |
| Code review | **Always a different vendor from the author** | — | Medium |

## 3. Routing by workstream (from PLAN.md §4)

| WS | Workstream | Spec author | Implementer(s) | Reviewer | Why |
|---|---|---|---|---|---|
| WS-01 | Build, CI, packaging, release | Codex Astra | Codex Sol | Claude Opus | Ops/CI is OpenAI's strongest independent result |
| WS-02 | Protocol and contracts | Claude Opus | Claude Opus | Codex Astra | Highest-leverage design; keep one author |
| WS-03 | Engine core | Claude Opus | Claude Opus (+ Codex Sol for isolated algorithms: expression parser, queue scheduler) | Codex Sol | Long-horizon Rust plus tricky concurrency |
| WS-04 | Scripting runtimes | Claude Opus | Claude Opus | Codex Sol | Sandboxing and FFI need care |
| WS-05 | Platform connectors | Claude Opus (one spec) | Codex Sol (Twitch first, then YouTube, Kick, X…) | Claude Opus | Protocol-heavy; Grok feeds API-change research |
| WS-06 | Local server, API, MCP | Claude Opus | Codex Sol | Claude Opus | JSON-RPC, auth, framing |
| WS-07 | Shell and core UI | Claude Opus | Claude Sonnet | Codex Sol | UI strength; Opus for visual bugs |
| WS-08 | Multichat | Claude Opus | Codex Sol (message model, emote/badge parsers) + Claude Sonnet (UI) | swap | Split by layer |
| WS-09 | Overlay runtime and designer | Claude Opus | Claude Sonnet (editor) + Claude Opus (runtime/queue) | Codex Sol | Visual and multimodal strength |
| WS-10 | Audio, TTS, STT | Claude Opus | Claude Opus | Codex Sol | Native libs, threading |
| WS-11 | App and VTuber integrations | Claude Opus | **Claude Opus writes the first reference integration (VTube Studio)**, then Codex Sol does the rest | Claude Sonnet | Pattern first, then bulk |
| WS-12 | Plugin SDK, CLI, host | Claude Opus | Claude Opus (host) + Codex Sol (CLI, SDK packaging) | swap | |
| WS-13 | AI layer | Claude Opus | Claude Opus | Codex Sol | |
| WS-14 | Decks and remote | Claude Sonnet | Claude Sonnet | Codex Sol | UI-heavy |
| WS-15 | Import/compat | Codex Sol | Codex Sol | Claude Opus | Format parsing |
| WS-16 | QA, test kit, e2e | Codex Astra | Codex Sol | Claude Opus | Tests by a different vendor than the implementation |
| WS-17 | Docs site | Claude Sonnet | Claude Sonnet | Codex Sol | |
| WS-18 | Ecosystem watch | — | Grok 4.7 (weekly research brief) | Claude Sonnet (fact-check) | Live X/web access |

## 4. Dependency graph (what can run in parallel)

```
P0:  WS-01 (CI/packaging) ─┐
     WS-02 (protocol v0) ──┼─► gates everything below
     Spikes S-1…S-5 ───────┘
P1:  WS-03 engine ──► WS-06 API ──► WS-07 UI (action editor)
          │             │
          ├─► WS-04 scripting (JS)
          └─► WS-05 Twitch ──► WS-11 OBS
P2:  WS-08 multichat ◄── WS-05 (YouTube, Kick) ;  WS-10 TTS (parallel) ;  relay (parallel)
P3:  WS-09 designer (needs WS-08 message model + alert events)
P4:  WS-12 SDK ─► WS-11 bulk integrations (parallel, one agent per integration) ; WS-13 AI ; MCP (WS-06)
P5:  WS-14 decks ; STT (WS-10) ; loyalty (WS-03) ; WS-15 import
Always: WS-16 QA, WS-17 docs, WS-18 watch
```

Rules of thumb: **no two agents on the same crate at once**, unless their tasks' `paths:` don't overlap.
Integrations parallelize well (one crate each). The engine core does not; keep it to one agent at a time.

## 5. Dispatch protocol (the owner runs this)

Tools: the **codex-orchestrator board** (project `streamden`; lanes and protocol in [board.md](board.md)), plus the
orchestrator's launchers, Git Director and messaging ([ADR-0005](../adr/0005-work-management-and-agent-tooling.md)).
You can do all of this in the orchestrator console, or ask the planning agent to do it over MCP.

1. **Pick a card** in Backlog with `ready: true`. Check that its *Scope (paths)* doesn't overlap any claimed card.
2. **Dispatch:** set the `agent:<vendor>` label (and a `reviewer:<other vendor>` label).
3. **Shortcut:** `scripts/dispatch.sh DEN-<n> <claude|codex|grok> [work|review]` does steps 3–5 for you: it
   creates or reuses the worktree, then launches the right wrapper with the standard prompt. The manual steps:
   **Create the worktree** (vendor-neutral):
   ```bash
   git worktree add ../den-wt/DEN-<n> -b agent/<vendor>/DEN-<n>-<slug>
   cd ../den-wt/DEN-<n>
   ```
4. **Launch through the orchestrator** so auth, config, skills and fleet instructions are identical:
   - **Claude Code:** `clx` (the managed `claude` shim)
   - **Codex:** `cdx` (for module work: `cdx --cd crates/<mod>` so the module `AGENTS.md` loads; 32 KiB cap)
   - **Grok Build:** `cgx`. Grok also reads `CLAUDE.md` and `.claude` hooks, so keep both vendor-neutral.
5. **Prompt:** "Work on DEN-<n>." plus the universal template in [PROMPTING.md](PROMPTING.md) §2. The agent
   claims the card, moves it into its lane, and releases it with a handoff note at the end.
6. **When the card reaches Review:** launch the reviewer vendor in the same worktree ("Review DEN-<n>"). It claims
   with role `review` and releases into Verifying (or back to Coding with findings). Open the PR. Reviewer and
   author can talk through orchestrator messaging; decisions get copied into the PR or the card.
7. **Verify and merge** when CI is green and findings are resolved, then move the card to Done. Remove the
   worktree: `git worktree remove ../den-wt/DEN-<n>`.
8. **Stale cards** free themselves: claims bound to a session release when it ends, or after the 30-min TTL.
   Read the last handoff note before re-dispatching, ideally to a different vendor.

**Parallelism:** with Claude Max, ChatGPT Pro and SuperGrok you're bound by subscription **quotas**, not API
bills. The orchestrator's quota view (Claude and Codex) shows headroom. A practical starting point: about 2
Claude sessions + 2 Codex sessions + 1 Grok session at once, on non-overlapping paths.

**Escalation:** if an agent fails the same *Done when* twice after corrections, stop. Clear its context and
hand the item, with its handoff comments, to a different vendor. Repeated failure usually means the spec is
unclear; fix the spec first.

## 6. Bake-off (spike S-5) — calibrate this guide with real data

Give the same four tasks to Claude (Opus 5.5), Codex (GPT-6.1 Sol) and Grok (4.7), each in its own worktree:

| # | Task | Measures |
|---|---|---|
| B-1 | Rust: a bounded action queue (blocking/non-blocking, pause/clear) with unit tests | Concurrency correctness, Rust idiom |
| B-2 | Protocol: parse 20 recorded Twitch EventSub payloads into typed structs (golden tests) | Protocol accuracy |
| B-3 | UI: a React "sub-action picker" (searchable tree, keyboard nav) against a JSON fixture | UI quality, a11y |
| B-4 | Refactor: rename and restructure a small module across ~15 files without behaviour change | Long-horizon edits |

Score each on: CI pass on first `done_when` run (Y/N), review findings (count by severity), wall-clock time,
quota used (subscription), and the owner's subjective quality (1–5). Board cards: DEN-7 (parent), DEN-8…DEN-11 (label `bakeoff`). Record results in `docs/agents/bakeoff-YYYY-MM.md` and update
§2–3.

## 7. Known tool quirks (verify after each tool update)

- **Instruction files:** `AGENTS.md` is read natively by Codex, Grok Build, Cursor and Copilot. Claude Code reads
  `CLAUDE.md`, which imports `@AGENTS.md`. Research reports a user-level Claude Code setting that also loads
  *nested* module `AGENTS.md` files when a root `CLAUDE.md` exists (details in ai-agent-fleet.md §1.6).
  **This is unverified; check it with `/memory` before relying on it.** Fallback: each module folder gets a
  one-line `CLAUDE.md` containing `@AGENTS.md`.
- **Check what each agent loaded:** `/memory` (Claude), `grok inspect` (Grok), and ask Codex to "summarize your
  current instructions".
- **Enforcement belongs in git hooks and CI** (lefthook/pre-commit), not vendor-specific hooks.
