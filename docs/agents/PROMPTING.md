# Prompting Guide — how to brief each AI agent on this project

> Audience: the owner. Copy-paste templates are below. Sources: vendor prompting guides summarized in
> [ai-agent-fleet.md §3](../research/ai-agent-fleet.md). Re-check when vendors ship new models.

## 1. Rules that work for every vendor

1. **One task, one session.** Start fresh (`/clear` in Claude, a new session in Codex/Grok) for each task.
   Long compaction chains drift.
2. **Point to files; don't paste the world.** "Read card DEN-42 and `docs/spec/05-platforms.md` §3"
   beats pasting the spec. All vendors do better with less, more relevant context.
3. **Define "done" as commands.** Every prompt ends with `done_when` commands and "paste the output". Agents
   stop when work *looks* done; runnable checks make them stop when it *is* done.
4. **Name the scope and the non-scope.** List the paths it may touch, and say "report unrelated bugs as
   follow-ups, don't fix them".
5. **Say why.** A one-line reason behind a constraint ("no WebGL — WebKitGTK on NVIDIA/Wayland breaks")
   generalizes better than ALWAYS/NEVER. Keep ALWAYS/NEVER for real invariants.
6. **Give a pattern to copy.** "Follow `crates/integrations/den-vtubestudio` for structure" is the single most
   effective line for integration work.
7. **Protect the tests.** "Don't hard-code values to make tests pass. If a test is wrong, tell me instead of
   working around it."
8. **Ask for a handoff.** Every session ends with `project_card_release` and a handoff note.

## 2. Universal task prompt (paste into any vendor, fill the <>)

```markdown
## Task DEN-<n>: <title>
Goal: <observable outcome, one or two sentences>.
Read first: board card DEN-<n> (`project_card_get`: detail + history), AGENTS.md, <module>/AGENTS.md, docs/spec/<NN-name>.md §<x>,
docs/decisions.md. Pattern to follow: <path/to/similar/code>.
Scope: only modify paths listed in the card's Scope (paths). Out of scope: <…>. Report unrelated bugs as new Backlog
items; don't fix them.
Constraints: don't change crates/den-protocol (contracts) unless this task says so; no new dependencies without
noting them and their license in the Handoff; must work on Linux (Wayland) and Windows.
Done when (run these and paste the output): `<cmd 1>`, `<cmd 2>`, `<cmd 3>`.
Autonomy: proceed without asking for reversible, in-scope steps. Stop and ask only for destructive actions,
contract changes, or product decisions (add those to docs/QUESTIONS.md).
Before ending: project_card_release DEN-<n> with the handoff note (tasks/_TEMPLATE.md §B), advancing it
only if every Done-when passes; commit with a single Conventional Commit subject line ending in (DEN-<n>): no body, no trailers.
```

## 3. Vendor-specific tips

### Anthropic — Claude Code (Opus 5.5 / Sonnet 5.5 / Fable 5.1)
- **Effort is the main dial.** Opus 5.5 defaults to `medium`, which is right for most tasks. Use `high`/`xhigh`
  only for specs, contracts and concurrency-heavy engine work. Lower effort rather than saying "think less".
- **Explore → plan → code.** Use plan mode (Shift+Tab) for anything touching contracts, the engine or the
  shell. Skip planning if the diff fits in one sentence.
- **Use action verbs.** "Implement…" gets edits; "Can you suggest…" gets suggestions only.
- **XML tags help** for long prompts: `<context>`, `<spec>`, `<constraints>`, `<done_when>`.
- **Curb scope creep** (paste for big tasks):
  `Avoid over-engineering: only make changes that are directly requested or clearly necessary. Don't add features, refactor surrounding code, or add abstractions beyond the task.`
- **Unattended runs:** add `You are operating autonomously; there is no one to answer questions. Make reasonable in-scope assumptions, note them in the Handoff, and keep going until done_when passes.`
- **Big features:** have Claude interview you first ("interview me with questions, then write the spec"), then
  implement in a *fresh* session.
- **UI work:** name what to avoid ("no gradients, no pill buttons, no cream backgrounds; match
  `web/kit` tokens"). "Avoid the generic AI look" alone just swaps one default for another.
- **Reviews:** "Flag only issues that affect correctness or the stated requirements". Otherwise it over-reports.

### OpenAI — Codex (GPT-6.1 Sol / GPT-6 Astra / Luna)
- **Outcome first, lean prompt.** Describe the destination and the completion bar, not every step. OpenAI
  measured better results with *fewer* rules. Remove anything redundant with AGENTS.md.
- **Codex shape:** behaviour, then location (files or repro), then how to verify. For example: "Reproduce the
  failing golden test, fix the parser, run `cargo nextest run -p den-twitch`, and report the commands and
  results."
- **Effort:** start with **Sol at medium** (or Astra at Light). Raise it only when a task actually fails at the
  lower level. Check first that the prompt has a success criterion.
- **Use `/plan` then `/goal`** for multi-step work. Split big refactors into milestones and review each diff.
- **Astra-specific counter-prompts:**
  - Over-asking: `Infer intent and scope from the board card; I should be approving a concrete, reviewable result.`
  - Over-testing: `Don't write tests for reversible, low-impact changes that mirror the implementation.`
  - Verbosity: `Report in short, clear paragraphs.`
- **AGENTS.md hygiene:** Codex (especially Astra) is sensitive to conflicting instructions. If it pauses
  oddly, ask it to quote the instruction that caused the pause.
- **Launch in the module dir** (`codex --cd crates/<mod>`) so the module AGENTS.md loads.
- **Reviews:** `/review` locally or `@codex review` on a GitHub PR. Ask for findings first, ordered by severity.

### xAI — Grok Build (Grok 4.7)
- **Set reasoning effort explicitly:** `high` for research briefs, `low` for quick fixes and agent loops.
- **Give bounded, verifiable tasks** with `done_when` and **proof requirements** (command output; before/after
  screenshots for UI).
- **Plan mode** only for ambiguous or high-impact work. Comment on plan lines *before* approving, because
  approval starts the build.
- **Keep rules short.** Grok loads rules files in full with no cap. Check with `grok inspect`.
- **Research prompts must demand citations:** "Every claim needs an official URL or an X post link; mark
  anything you couldn't verify as (unverified)."
- **Never merge Grok output without a Claude or Codex review.**

## 4. Role prompts (prepend to the universal template when relevant)

**Spec writer** (Claude Opus/Fable, high effort)
```
You are writing docs/spec/<NN-name>.md for StreamDen. Read PLAN.md, docs/architecture.md, docs/decisions.md,
docs/FEATURE-PARITY.md (rows for this workstream) and the research docs listed in the task.
Produce: scope and non-goals; user stories; data model (Rust-ish types); events, sub-actions and settings with
their schemas; API surface; UI flows; cross-platform notes (Linux Wayland, Windows, macOS); performance budgets;
security; test plan; open questions (add them to docs/QUESTIONS.md, don't answer them).
Ask me questions first if a product decision is missing. Don't write code.
```

**Contract author** (Claude Opus)
```
Turn spec §<x> into types in crates/den-protocol with serde + schemars, doc comments on every field, and
examples. Add stub implementations and failing contract tests. Write or amend the ADR. Don't implement behaviour.
```

**Implementer** (any vendor): use the universal template as-is.

**Test writer** (Codex Sol; never the same vendor as the implementer)
```
Write tests for <module> against docs/spec/<NN>.md §<x>, not against the implementation's current behaviour.
Use crates/den-testkit fixtures and golden payloads. If the implementation disagrees with the spec, report it;
don't change either.
```

**Reviewer** (a different vendor from the author)
```
Review the diff of branch <branch> against board card DEN-<n> and docs/spec/<NN>.md. Findings first,
ordered by severity. Flag only correctness, security, cross-platform, performance-budget or requirement gaps.
Ignore style (CI handles it). For each finding: file:line, the failure scenario, and the suggested fix.
```

**Researcher** (Grok 4.7 for live/X; Claude or Codex for docs-heavy)
```
Research <topic> as of <date>. Write docs/research/<slug>.md with inline source URLs (official docs first;
X posts allowed for announcements). Mark unverified claims. End with "Implications for StreamDen" and
"Open questions". Don't edit any other file.
```

**Docs writer** (Claude Sonnet)
```
Write user-facing docs for <feature> from docs/spec/<NN>.md and the merged code. Audience: streamers who don't
code. Task-oriented, with screenshots placeholders, and Linux/Windows/macOS differences called out.
```

## 5. Workstream-specific watch-outs (add the relevant line to the prompt)

| WS | Add to the prompt |
|---|---|
| WS-01 Build/CI | "Matrix: ubuntu-22.04, fedora (container), arch (container), windows-latest, macos-latest. No secrets in logs. AUR PKGBUILD must pass `namcap`." |
| WS-02 Protocol | "Every public type derives `Serialize, Deserialize, JsonSchema` and has doc comments. Run codegen and commit the outputs. Breaking changes bump `rpcVersion`." |
| WS-03 Engine | "No blocking calls on the Tokio runtime. Queue semantics must match spec §<x> exactly. Property tests for the scheduler." |
| WS-04 Scripting | "Deadline and memory limits are mandatory. Never expose fs/net to JS except through capability-checked `api.*`. Python worker hard-kill on timeout." |
| WS-05 Platforms | "Use recorded golden payloads in tests; no live network in CI. Twitch: Device Code Flow, public client, validate tokens hourly. Respect rate limits with a shared limiter." |
| WS-06 API/MCP | "Check Host and Origin headers. Tokens are hashed at rest. Every WS request has a schema and an example in protocol.json." |
| WS-07 UI | "Use web/kit components and tokens only. No WebGL. Keyboard-accessible. Forms come from schemas; never hand-write a form for a schema'd item." |
| WS-08 Multichat | "Unified message schema from den-protocol. Filters are pure functions with table tests. A deleted message must disappear from every surface." |
| WS-09 Designer | "Same renderer in editor and runtime. Web Animations API, no GSAP. Designs are versioned JSON validated by schema; include a migration for every schema bump." |
| WS-10 Audio/TTS | "Inference on a bounded pool with 1–2 threads by default. Output device chosen by stable ID, not index. PipeWire on Linux." |
| WS-11 Integrations | "Follow `den-vtubestudio` as the reference. Every connector accepts a remote host. Mark Windows-only apps in the manifest. Golden tests with recorded payloads." |
| WS-12 Plugins | "Built-ins must use the same manifest and trait as third-party plugins. Permissions are declared and enforced. `den dev` hot-reloads." |
| WS-13 AI | "Keys in the OS keychain. Every outbound AI chat message passes the moderation filter and the disclosure setting. Budget caps are enforced before the call." |
| WS-15 Import | "Clean-room: parse data formats only. Unknown sub-action types become disabled placeholders with the original JSON kept, never dropped." |
