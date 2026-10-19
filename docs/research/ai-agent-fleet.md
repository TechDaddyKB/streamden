# Running a Mixed AI Coding-Agent Fleet in One Repo (Claude + Codex + Grok)

*Research date: 2026-10-18. Scope: Streamer.live (Rust core + TypeScript web UI, likely Tauri 2), solo developer, agents from Anthropic, OpenAI, and xAI working in the same repository.*

> **How to read this.** Every factual claim has an inline source. Figures marked **[vendor]** come from the company selling the model and are marketing until someone else reproduces them. Figures marked **[indep]** come from third-party evaluators (mostly Vals AI, sometimes Artificial Analysis). Where sources disagreed or I couldn't verify something, I say so. This field changes monthly, so re-check model names and defaults before relying on them.
>
> **Research gaps.** The session's web-search budget and a rate limit cut research short. Three things are **unverified**: (a) Codex's repo-level skills directory path, (b) a current official LMArena WebDev snapshot, and (c) an official GPT-6-specific Codex prompting guide. The only official Codex prompting guide I found targets `gpt-5.3-codex`. All three are flagged where they come up.

---

## 0. TL;DR

1. **One instruction source of truth: `AGENTS.md`** (root plus one per module). All three vendors read it: Codex natively, Grok Build natively, and Claude Code natively since v2.1.277, or through an `@AGENTS.md` import.
2. **Thin root `CLAUDE.md`** containing `@AGENTS.md` plus a short "Claude Code only" section. In `~/.claude/settings.json`, set Claude Code's *Project instructions* to `claude-md-and-agents-md` so nested module `AGENTS.md` files also load. Without that setting, a root `CLAUDE.md` makes Claude skip them.
3. **Current flagships (Oct 2026):**
   - Anthropic: **Claude Opus 5.5** (`claude-opus-5-5`), **Fable 5.1** (`claude-fable-5-1`), **Sonnet 5.5**, **Haiku 5.5**.
   - OpenAI: **GPT-6 Astra** (`gpt-6-astra`), **GPT-6.1 Sol** (`gpt-6.1-sol`, recommended for complex coding in Codex), **GPT-6 Luna** (`gpt-6-luna`).
   - xAI: **Grok 4.7** (`grok-4.7`) in the **Grok Build** CLI (`grok`).
   - The "GPT-5.x-codex" and "grok-code" names in the brief are now legacy.
4. **Independent evals put Claude Opus/Sonnet 5.5 at or near the top for agentic, multi-file coding.** GPT-6 Astra and 6.1 Sol lead on algorithmic (IOI) and SRE/ops tasks, and 6.1 Sol is by far the best price-performance. Grok 4.7 is well behind on independent agentic-coding evals. Use it mainly for live web/X research and cheap second opinions, not unsupervised long-horizon implementation.
5. **Coordination model:**
   - one worktree and branch per agent per task;
   - one Markdown file per task, with a claim/lease and a `paths:` scope;
   - contracts first (shared Rust types generating TS bindings);
   - ADRs plus a decision log;
   - conventional commits with an `Agent:` trailer;
   - CI is the only arbiter of "done";
   - every PR is reviewed by a **different vendor** than the one that wrote it.

---

## 1. Instruction files: what each tool reads

### 1.1 AGENTS.md (the cross-vendor standard)

- AGENTS.md is "a README for agents": plain Markdown with no required fields. It is "stewarded by the Agentic AI Foundation under the Linux Foundation" and used by 60k+ open-source projects. ([agents.md](https://agents.md/))
- Listed supporters include:
  - Codex (OpenAI), Jules and Gemini CLI (Google), Cursor, Factory, Aider, goose, opencode, Zed, Warp, VS Code;
  - Devin and Windsurf (Cognition), Junie (JetBrains), Amp, RooCode, Kilo Code, Augment Code;
  - GitHub Copilot's coding agent, UiPath, Semgrep, Phoenix, Ona.

  ([agents.md](https://agents.md/))
- Nested precedence is defined as: "The closest AGENTS.md to the edited file wins; explicit user chat prompts override everything." ([agents.md](https://agents.md/))
- Suggested sections are project overview, build/test commands, code style, testing instructions, security considerations, and commit/PR guidelines. ([agents.md](https://agents.md/))

### 1.2 Claude Code (Anthropic)

Source for this whole section: [code.claude.com/docs/en/memory](https://code.claude.com/docs/en/memory).

**Native files**
- Claude Code reads `CLAUDE.md` / `.claude/CLAUDE.md` (project), `CLAUDE.local.md` (personal, gitignored), `~/.claude/CLAUDE.md` (user), and `.claude/rules/*.md`.
- Files above the working directory load at launch. Subdirectory `CLAUDE.md` files load on demand, when Claude reads or edits a file in that directory.

**AGENTS.md support (v2.1.277+)**
- The setting is called *Project instructions*. Change it in `/config`, or in `~/.claude/settings.json` under `pluginConfigs["cc-plugin-agents-md@builtin"].options.instructionFiles`. It is ignored in project and local settings files. Values:

| Value | Behavior |
|---|---|
| `claude-md-or-agents-md` (default) | Reads AGENTS.md **only if no `CLAUDE.md`, `.claude/CLAUDE.md`, or `CLAUDE.local.md` exists in the cwd or above** |
| `claude-md-and-agents-md` | Reads both: each directory's CLAUDE.md first, then its AGENTS.md. An AGENTS.md already loaded via import or symlink is not read twice |
| `claude-md` | CLAUDE.md only |
| `managed-only` | Managed instructions only |

- **Gotcha:** adding a `CLAUDE.local.md` silently stops Claude from reading AGENTS.md under the default setting.
- **Gotcha:** some sessions can't load AGENTS.md directly (built-in plugin disabled, or the first session after upgrading from ≤2.1.276). Anthropic recommends a `CLAUDE.md` containing `@AGENTS.md` for those cases, and says that keeping the import "never makes Claude read AGENTS.md twice."

**Imports**
- Syntax is `@path`. Paths are relative to the importing file, recursion is capped at 4 hops, and imports inside code spans or blocks are ignored.
- Imports organize content but don't reduce context, because imported files also load at launch.

**Size**
- "Target under 200 lines per CLAUDE.md file. Longer files consume more context and reduce adherence." Files over 4 MiB are skipped.

**Path-scoped rules**
- `.claude/rules/*.md` with `paths:` YAML frontmatter (globs) load only when Claude touches matching files.

**Symlink caveat**
- `ln -s AGENTS.md CLAUDE.md` works, but on Windows it needs admin rights or Developer Mode. Anthropic recommends the `@AGENTS.md` import if anyone uses Windows.
- This matters for Streamer.live, since Windows is the main platform for streamers.

**Diagnostics**
- `/memory`, `/context`, `/doctor prompt-audit` (finds stale or contradictory instructions), and the `InstructionsLoaded` hook.

Anthropic's documented pattern ([memory docs](https://code.claude.com/docs/en/memory)):

```markdown
@AGENTS.md

## Claude Code
Use plan mode for changes under `src/billing/`.
```

### 1.3 Codex (OpenAI)

Source for this section: [Codex AGENTS.md guide](https://learn.chatgpt.com/docs/agent-configuration/agents-md); developers.openai.com now redirects there.

**Global**
- `~/.codex/AGENTS.override.md`, then `~/.codex/AGENTS.md`. The first non-empty file wins. `CODEX_HOME` relocates this directory.

**Project**
- Codex walks from the project root (git root) **down to the current working directory**.
- In each directory it takes at most **one** file: `AGENTS.override.md`, then `AGENTS.md`, then any names in `project_doc_fallback_filenames`.
- Files are concatenated root-first, so deeper files come later and win.
- **Implication:** module `AGENTS.md` files *below* the launch directory are not auto-loaded. Launch Codex in the module directory (`--cd crates/engine`) when working there.

**Size cap**
- `project_doc_max_bytes` defaults to **32 KiB for the combined chain**. Codex "stops adding files once the combined size reaches the limit."

**Reload**
- The instruction chain is built once per run or TUI session, so restart Codex after editing AGENTS.md.

**Review rules**
- OpenAI suggests a `## Code Review Rules` section in the AGENTS.md closest to the governed code, and to "reserve formatting and lint checks for CI."

**Auditing**
- Run `codex -c log_dir=./.codex-log` and inspect `codex-tui.log`.

**Sensitivity**
- GPT-6 Astra is "more sensitive to guidance in skills and files such as `AGENTS.md`". Unclear or conflicting guidance can make it pause early, and OpenAI "strongly recommend[s] auditing" every instruction file. ([OpenAI GPT-6 Astra guide](https://developers.openai.com/api/docs/guides/latest-model/gpt-6-astra.md), [model guidance](https://developers.openai.com/api/docs/guides/model-guidance))

### 1.4 Grok Build (xAI)

Sources: [AGENTS.md in Grok Build](https://docs.x.ai/build/features/project-rules), [settings reference](https://docs.x.ai/build/settings/reference), [skills page](https://docs.x.ai/build/features/skills-plugins-marketplaces).

**Tool**
- Grok Build is the `grok` CLI/TUI, with headless `grok -p` and ACP integration. Install with `curl -fsSL https://x.ai/cli/install.sh | bash`. ([overview](https://docs.x.ai/build/overview))

**Files read in each directory**
- `AGENTS.md`, `Agents.md`, `AGENT.md`, `CLAUDE.md`, `Claude.md`, `CLAUDE.local.md`, and every `*.md` in `.grok/rules/`.
- There is **no `GROK.md`**.
- It also reads `.claude/rules/` and `.cursor/rules/` "for compatibility". The docs don't say whether `paths:` frontmatter is honored, so assume those rules may load unconditionally.

**Discovery order**
- `~/.grok/` first, then each directory from repo root down to the cwd. "Deeper files tak[e] precedence on conflicts."
- Gitignored files are skipped.

**Size and imports**
- "Files are loaded in full, with no size cap."
- No import syntax is documented, so `@AGENTS.md` inside CLAUDE.md is presumably seen as literal text.

**Claude Code compatibility**
- Grok auto-discovers Claude Code skills, plugins, hooks, MCP servers, and subagents.
- Toggles: `GROK_CLAUDE_AGENTS_ENABLED` (CLAUDE.md scanning), `GROK_CLAUDE_RULES_ENABLED`, `GROK_CLAUDE_HOOKS_ENABLED`, and `[compat.claude] rules/hooks` in TOML. All default to **true**.
- **Gotcha:** your `.claude/settings.json` hooks may also execute under Grok, so write hooks that are vendor-neutral or disable the compatibility toggle.

**Inspection**
- `grok inspect` lists every rules file with its path and approximate token count. Use it to confirm nothing is loaded twice.

### 1.5 Cursor and Gemini CLI (for completeness)

- **Cursor:** project rules are `.cursor/rules/*.mdc` with frontmatter (`alwaysApply`, `globs`, `description`). Plain `.md` files there are ignored. AGENTS.md is supported in the root and subdirectories, nested files combine with parents, and "more specific instructions take precedence." ([Cursor rules docs](https://cursor.com/docs/context/rules))
- **Gemini CLI:** reads `GEMINI.md` hierarchically (global `~/.gemini/GEMINI.md`, workspace and ancestors, plus just-in-time loading when tools touch a directory). It supports `@file.md` imports. To use AGENTS.md, set `"context": {"fileName": ["AGENTS.md", "GEMINI.md"]}` in `settings.json`. ([Gemini CLI GEMINI.md docs](https://github.com/google-gemini/gemini-cli/blob/main/docs/cli/gemini-md.md))
- **GitHub Copilot coding agent:** listed as an AGENTS.md supporter. ([agents.md](https://agents.md/))

### 1.6 Recommended layout for Streamer.live (no duplicated rules)

```
Streamer.live/
├── AGENTS.md                  # SOURCE OF TRUTH. ≤150 lines. Universal rules, commands, conventions, coordination protocol
├── CLAUDE.md                  # "@AGENTS.md" + ≤15 lines under "## Claude Code only"
├── .gitignore                 # includes .claude/worktrees/, CLAUDE.local.md, .codex-log/
├── .worktreeinclude           # .env etc. copied into Claude worktrees
├── .claude/
│   ├── settings.json          # permissions + VENDOR-NEUTRAL hooks (Grok runs them too)
│   ├── skills/<name>/SKILL.md # reusable procedures (Grok also auto-discovers these)
│   └── agents/                # Claude subagents (e.g. reviewer)
├── .grok/rules/               # keep EMPTY unless a rule is truly Grok-only
├── docs/
│   ├── architecture.md        # system map (linked from AGENTS.md, not imported)
│   ├── adr/0001-*.md          # Architecture Decision Records (MADR style)
│   ├── decisions.md           # one-line-per-decision log, newest first
│   ├── contracts/             # IPC/event/plugin API specs (JSON Schema/OpenAPI/WIT…)
│   └── research/
├── tasks/
│   ├── README.md              # protocol (short) + generated index
│   ├── T-0042-obs-scene-switch.md
│   └── …
├── crates/
│   ├── contracts/  AGENTS.md  # shared types; generates TS bindings
│   ├── engine/     AGENTS.md  # ≤60 lines each: purpose, invariants, commands, owners
│   └── …
├── ui/             AGENTS.md  # TS/Svelte-or-React rules, test commands
├── src-tauri/      AGENTS.md
└── .github/CODEOWNERS         # human review routing (you); agent routing lives in tasks
```

Rules for keeping it duplication-free:

1. **All shared rules go in `AGENTS.md` files.** Nothing vendor-neutral goes in `CLAUDE.md`, `.claude/rules/`, or `.grok/rules/`.
2. **Configure Claude Code once.** In `~/.claude/settings.json` (user scope, because project scope is ignored):
   ```json
   { "pluginConfigs": { "cc-plugin-agents-md@builtin": {
       "options": { "instructionFiles": "claude-md-and-agents-md" } } } }
   ```
   This makes Claude load module `AGENTS.md` files lazily even though a root `CLAUDE.md` exists, without double-loading the imported root file. ([memory docs](https://code.claude.com/docs/en/memory))
3. **Grok will read both root `AGENTS.md` and root `CLAUDE.md`.** That is fine because CLAUDE.md is a ~15-line stub, but label its section "Claude Code only (other agents ignore)". If it ever causes confusion, set `GROK_CLAUDE_AGENTS_ENABLED=0`. ([settings reference](https://docs.x.ai/build/settings/reference))
4. **Avoid `paths:`-scoped `.claude/rules/`.** Grok may load them unscoped. Put directory-specific guidance in nested `AGENTS.md` instead, which all three tools scope correctly.
5. **Budget:** the root plus the deepest module chain must stay under **32 KiB** (Codex cap). Aim for well under 10 KiB, because shorter is followed better by every vendor.
6. **Codex in a module:** launch with `codex --cd crates/engine` so the module AGENTS.md is in the chain.
7. **Verify after every change to instruction files:** `/memory` (Claude), `grok inspect` (Grok), and "summarize your current instructions" (Codex).
8. **Optional extras:** if you add Gemini CLI, set `context.fileName` to include `AGENTS.md`. Cursor needs nothing.

**What goes in root AGENTS.md** (for each line, ask "would removing this cause a mistake?"; [Claude Code best practices](https://code.claude.com/docs/en/best-practices)):
- one-paragraph product summary plus a link to `docs/architecture.md`;
- exact build, test, lint, and format commands (`cargo …`, `pnpm …`, `cargo tauri dev`);
- non-default conventions only;
- the task/branch/commit protocol (section 4);
- security rules (secrets, network, plugin sandboxing);
- "Code Review Rules";
- a definition of done.

Exclude anything the agent can infer from the code.

---

## 2. Current models and relative strengths

### 2.1 Lineups (verified on vendor docs)

| Vendor | Model (API ID) | List price in/out per MTok | Notes |
|---|---|---|---|
| Anthropic | **Claude Fable 5.1** (`claude-fable-5-1`) | $10 / $50 | "For demanding reasoning and long-horizon agentic work"; 1M ctx; default effort `high` ([models overview](https://platform.claude.com/docs/en/about-claude/models/overview)) |
| Anthropic | **Claude Opus 5.5** (`claude-opus-5-5`) | $4 / $20 | "For long-running agentic coding and knowledge work"; Anthropic's recommended starting model; default effort `medium` ([overview](https://platform.claude.com/docs/en/about-claude/models/overview)) |
| Anthropic | **Claude Sonnet 5.5** (`claude-sonnet-5-5`) | $2 / $10 | "Best combination of speed and intelligence" |
| Anthropic | **Claude Haiku 5.5** (`claude-haiku-5-5`) | from $0.10 / $0.50 | High-volume, latency-sensitive tasks |
| OpenAI | **GPT-6 Astra** (`gpt-6-astra`) | $10 / $50 ([Vals](https://www.vals.ai/models/openai_gpt-6-astra)) | "Most capable", for "the hardest end-to-end work" ([Codex models](https://learn.chatgpt.com/docs/models)) |
| OpenAI | **GPT-6.1 Sol** (`gpt-6.1-sol`) | $2 / $10 ([Vals](https://www.vals.ai/models/openai_gpt-6.1-sol)) | "For complex coding and agentic workflows, use GPT-6.1 Sol"; "near-Astra performance… at a lower cost" ([Codex models](https://learn.chatgpt.com/docs/models)) |
| OpenAI | **GPT-6 Luna** (`gpt-6-luna`) | n/a | "Focused, high-volume tasks such as summarization, extraction, and focused coding" |
| xAI | **Grok 4.7** (`grok-4.7`) | $2 / $6 (2× for ≥200k ctx) | 500k ctx; "For everything else, including code, use Grok 4.7" ([xAI models](https://docs.x.ai/developers/models)); released 2026-09-21 ([x.ai](https://x.ai/news/grok-4-7)) |
| xAI | `grok-build-0.1` | $1 / $2 | 256k ctx; older coding model ([xAI models](https://docs.x.ai/developers/models)) |

Lifecycle notes:
- GPT-5.5 was retired from Codex for ChatGPT sign-ins on 2026-10-14, and the replacement is `gpt-6.1-sol`. API-key sessions are unaffected. ([Codex changelog](https://learn.chatgpt.com/docs/changelog))
- Codex effort levels are Light/Low, Medium, High, Extra High, plus Max (more time on one task) and Ultra (parallel subagents). OpenAI's suggested starting points are "High for Luna or Light for Astra." ([Codex models](https://learn.chatgpt.com/docs/models))
- Grok 4.5 was announced as "the default model in Grok Build" in July 2026 ([x.ai](https://x.ai/news/grok-4-5)). The Grok 4.7 post says only that it is "available… in Cursor and Grok Build", and the Build docs show the default as configurable (`[models] default`). **Check `/model` in your install.**

### 2.2 Benchmark landscape: which numbers still mean anything

- **SWE-bench Verified, LiveCodeBench, Terminal-Bench 2.x** are now **archived** on Vals AI ([Vals benchmarks](https://www.vals.ai/benchmarks)). Verified is saturated: Vals reports Grok 4.7 at ~96% per its home page, and Fable 5 at 95% per [Vals](https://www.vals.ai/models/anthropic_claude-fable-5). It no longer separates the leaders.
- **Aider polyglot** hasn't been updated since late 2025, and none of the 2026 models are on it ([aider.chat](https://aider.chat/docs/leaderboards/)). **Scale's public SWE-Bench Pro leaderboard** also has no 2026 flagships ([Scale](https://labs.scale.com/leaderboard/swe_bench_pro_public)). Ignore both for model choice.
- The useful current independent signals are **Terminal-Bench 4.0, ProgramBench, Code Migration, SWE-bench Multimodal, Vibe Code Bench, IOI, and SRE Bench** (all Vals AI), plus the Artificial Analysis indices.

### 2.3 Independent results (Vals AI, pages read 2026-10-18) [indep]

| Benchmark (what it tests) | Opus 5.5 | Sonnet 5.5 | Fable 5.1 | GPT-6 Astra | GPT-6.1 Sol | Grok 4.7 |
|---|---|---|---|---|---|---|
| Terminal-Bench 4.0 (agentic terminal tasks) | **65.2%** (#1) | 64.1% | 58.1% | 59.6% | 55.1% | 28.8% (#19) |
| ProgramBench (rebuild CLI programs from spec, fully resolved) | **18.5%** (#1) | 6.5% | 7.0% | 5.5% | 3.0% | 0.5% |
| Code Migration | 66.7% | **69.8%** (#1) | 54.6% | 67.7% | 65.1% | 44.8% |
| SWE-bench Multimodal (visual/UI issues) | **70.2%** (#1) | 58.3% | 61.3% | 43.3% | 43.5% | 42.9% |
| Vibe Code Bench v1.1 (build apps) | 90.3% | **92.4%** (#1) | 90.3% | 89.6% | 88.9% | 86.2% |
| IOI (competitive algorithms) | 95.1% | 83.1% | 90.8% | **100%** | 96.9% | 57.7% |
| SRE Bench (ops/incident) | 33.6% | 30.2% | 22.9% | **56.9%** (#1) | 50.8% | 3.8% |
| Vals Index (overall) | 67.0% | **67.0%** | 65.8% | 63.1% | 61.2% | 55.0% |
| Cost per test (Vals Index) | $32.14 | $21.34 | $28.71 | $18.46 | **$3.24** | $12.11 |
| Latency (Vals Index) | 79 min | 78 min | 77 min | **28 min** | 43 min | 36 min |

Sources: [Terminal-Bench 4.0](https://www.vals.ai/benchmarks/terminal-bench-4), [ProgramBench](https://www.vals.ai/benchmarks/programbench), [IOI](https://www.vals.ai/benchmarks/ioi), [Opus 5.5](https://www.vals.ai/models/anthropic_claude-opus-5-5), [Sonnet 5.5](https://www.vals.ai/models/anthropic_claude-sonnet-5-5), [Fable 5.1](https://www.vals.ai/models/anthropic_claude-fable-5-1), [GPT-6 Astra](https://www.vals.ai/models/openai_gpt-6-astra), [GPT-6.1 Sol](https://www.vals.ai/models/openai_gpt-6.1-sol), [Grok 4.7](https://www.vals.ai/models/grok_grok-4.7).

Terminal-Bench 4.0 cost per test: Sol **$1.72**, Astra $9.58, Opus $13.20, Sonnet $16.51, Fable $17.18, Grok 4.7 $18.01. GPT-6 Luna scores 13.6% at $0.35, and Haiku 5.5 scores 36.4% at $3.12. ([Vals TB4](https://www.vals.ai/benchmarks/terminal-bench-4))

**Caveats on these numbers**
- Vals notes that Opus 5.5 used server-side **fallback** to Opus 5 / 4.8 on safety refusals. Counting those as failures drops Opus 5.5 to 53.5% on TB4, 83.3% on Vibe Code, and 5.3% on SRE Bench. Several Vals pages also show different figures in their "Updates" sections than in their main tables. ([Vals Opus 5.5](https://www.vals.ai/models/anthropic_claude-opus-5-5), [Vals Grok 4.7](https://www.vals.ai/models/grok_grok-4.7))
- Practical consequence: in Claude Code, security-adjacent tasks such as exploit-like tests and binary scanning may get refused or rerouted.
- Fable 5.1 safeguards "redirect pen testing, exploit generation, and binary-based vulnerability scanning to Opus models." ([Anthropic](https://www.anthropic.com/claude-fable-and-mythos-5-1))

**Artificial Analysis**
- Intelligence Index v4.3.2: Grok 4.7 = 46, Fable 5.1 = 53, GPT-6 = 53. ([The Decoder](https://the-decoder.com/xai-launches-grok-4-7-at-bargain-prices-but-benchmarks-reveal-a-wide-gap-to-claude-and-gpt-6/))
- On Terminal-Bench 4.0 it measured Grok 4.7 at **26%**, against xAI's own claim of 37.6%. ([same source](https://the-decoder.com/xai-launches-grok-4-7-at-bargain-prices-but-benchmarks-reveal-a-wide-gap-to-claude-and-gpt-6/))
- GPT-6.1 Sol: Intelligence Index 48 (medium) to 52 (max), ~50–60 tok/s. ([AA](https://artificialanalysis.ai/models/releases/gpt-6-1-sol))

### 2.4 Vendor claims (treat as marketing) [vendor]

**Anthropic**
- *Fable 5.1:* "the world's most advanced models for coding and knowledge work."
  - Terminal-Bench 4.0: 55.8% vs GPT-5.6 Sol 37.3%. CursorBench 3.2.0: 73.4%.
  - Customer anecdote of a 38-hour unattended run.

  ([Anthropic](https://www.anthropic.com/claude-fable-and-mythos-5-1))
- *Opus 5.5:* "strongest on multistep work in a real repository". At default `medium` effort it "matched or beat Claude Opus 5 at `high`". Also "stronger code review, with more bugs caught… and fewer false alarms." ([Opus 5.5 prompting guide](https://platform.claude.com/docs/en/build-with-claude/prompt-engineering/prompting-claude-opus-5-5))
- *Opus 5.5 launch figures* (secondary coverage only; I couldn't open Anthropic's primary post): TB4 66.4% at xhigh vs GPT-6 Astra 57.9%; SWE-bench Pro 89.9%. ([llm-stats](https://llm-stats.com/blog/research/claude-opus-5-5-launch), [Vellum](https://www.vellum.ai/blog/claude-opus-5-5-benchmarks-explained))

**OpenAI**
- *GPT-6 Astra:* DeepSWE v1.1 74.1% vs GPT-5.6 Sol 70.8%, with overlapping error bars (secondary coverage). ([DataCamp](https://www.datacamp.com/blog/gpt-6-astra), [The New Stack](https://thenewstack.io/openai-gpt6-astra-benchmarks/))
- *GPT-6.1 Sol:* "nearly matches GPT-6 Astra on agentic coding at a fifth of the price" (per [Morph](https://www.morphllm.com/best-ai-model-for-coding)). This is consistent with the Vals cost data above.

**xAI**
- *Grok 4.7:* "our most capable model for coding and knowledge work"; "Twice as fast, at half the price of comparable models."
  - xAI's own table: CursorBench 4.0 46.3% vs Fable 5.1 51.8%; Terminal-Bench 4.0 37.6% vs Fable 5.1 57.9%; DeepSWE v1.1 71.0%; EEBench 64.0% (best in their table).

  ([x.ai](https://x.ai/news/grok-4-7))
- *Grok 4.5:* "strong at challenging Rust and C/C++ tasks"; ~4.2× fewer output tokens than Opus 4.8 on SWE-Bench Pro; ~80 TPS. ([x.ai](https://x.ai/news/grok-4-5))
  - The Rust claim is **not** corroborated by any independent benchmark I found. ProgramBench, the closest compiled-language proxy, has Grok 4.5 at 0% and 4.7 at 0.5%.

**WebDev Arena:** sources conflict. One aggregator has GPT-6 Astra first, and a forum comment has Opus 5.5 first. The official page was not retrievable (HTTP 403). **Unverified.** ([llmrun aggregator](https://llmrun.dev/benchmark/lmarena-webdev?show=all))

### 2.5 Task-type assignment (with confidence)

| Task type | Primary | Secondary / reviewer | Confidence | Evidence and reasoning |
|---|---|---|---|---|
| Architecture, specs, ADRs | **Claude Opus 5.5** (Fable 5.1 for the gnarliest designs) | GPT-6 Astra as adversarial reviewer | Medium | Top Vals Index, strong long-horizon claims. Astra's verbose, thorough style makes a good critic. Design quality has no good benchmark. |
| Long-horizon multi-file refactors and migrations | **Claude Opus 5.5** (Sonnet 5.5 for cheaper migrations) | GPT-6.1 Sol | Medium-high | TB4 #1, ProgramBench #1 by 2.6×, Code Migration top-4 (Sonnet #1) [indep]. Fable 5.1 for unattended multi-hour runs [vendor]. |
| Rust systems code (core engine, async, IPC, plugin host) | **Claude Opus 5.5** | GPT-6.1 Sol | Low-medium | No Rust-specific independent benchmark. ProgramBench (compiled CLI rebuilds) strongly favors Opus. Grok's Rust claim is vendor-only and contradicted by its ProgramBench score. Let `cargo clippy` and tests arbitrate. |
| TypeScript / React / Svelte UI | **Claude Sonnet 5.5** | Opus 5.5 for visual bug-fixing; GPT-6.1 Sol | Medium | Vibe Code Bench #1 (Sonnet), SWE-bench Multimodal #1 (Opus, 70% vs ~43% for OpenAI) [indep]. All vendors' frontend defaults look generic unless you name what to avoid. |
| Algorithmic and protocol code (parsers, rate limiters, schedulers, OBS-WebSocket/Twitch EventSub framing) | **GPT-6.1 Sol** (Astra for the hardest) | Claude Opus 5.5 | Medium | IOI: Astra 100%, Sol 96.9%, Opus 95.1% [indep]. Protocol work is also integration-heavy, where Claude leads, so pair them. |
| Test writing | **GPT-6.1 Sol** (cheap, thorough) | Claude Sonnet 5.5 | Low | No direct benchmark. OpenAI says Astra "tends to test thoroughly" ([guide](https://developers.openai.com/api/docs/guides/latest-model/gpt-6-astra.md)). Writing tests with one vendor and the implementation with another reduces "teaching to the test." |
| Code review | **Cross-vendor**: Codex `/review` on Claude PRs; Claude Opus 5.5 on Codex/Grok PRs | Grok 4.7 as cheap third opinion | Medium | Opus 5.5 review claims [vendor]. Codex has `/review` and `@codex review` in GitHub ([The Decoder](https://the-decoder.com/openais-new-prompting-guide-tells-users-to-stop-overthinking-and-start-with-the-result/)). A different model family has different blind spots, and a fresh context avoids self-bias ([Claude best practices](https://code.claude.com/docs/en/best-practices)). |
| Docs (READMEs, user guides, API docs) | **Claude Sonnet 5.5** | GPT-6.1 Sol | Low-medium | No benchmark. OpenAI documents Astra's tendency toward "detailed, heavily formatted responses" that need style prompts. |
| Ops, CI, build/release scripting, debugging infra | **GPT-6 Astra / 6.1 Sol** | Claude Opus 5.5 | Medium | SRE Bench: Astra #1, Sol #2, far ahead of Claude [indep]. |
| Bulk / mechanical work (renames, lint fixes, boilerplate, i18n strings, simple adapters) | **GPT-6.1 Sol** (Luna for trivially scoped work) | Claude Haiku 5.5 / Sonnet 5.5 | High (for Sol cost-efficiency) | TB4 55% at $1.72/test vs $13–18 for others [indep]. Grok 4.7's low list price doesn't translate into low cost per *solved* task ($18/test at 29% on TB4). |
| Live web / X research (API changes, streamer-community sentiment, competitor launches, breaking platform news) | **Grok 4.7** (X search + web search tools) | Claude/Codex with web search for verification | Medium-high (for X data specifically) | xAI's API exposes "web search, X search, and code execution" tools ([Grok 4.7 guide](https://docs.x.ai/developers/grok-4-7)). Native X access is unique to Grok. Always confirm platform-API facts against official Twitch/YouTube/Kick docs. |
| Unsupervised long-horizon implementation | **Not Grok** today | n/a | Medium | TB4 29% and SRE 3.8% [indep] vs 55–65% for peers. Re-evaluate on each Grok release. |

**My honest summary.** For a Rust + TS desktop app in October 2026, Claude Opus/Sonnet 5.5 should do most implementation. GPT-6.1 Sol is the cost-efficient workhorse, the algorithms/ops specialist, and the primary cross-reviewer. Grok is the researcher with live X/web access and a cheap third reviewer. Independent results only partly agree with vendor charts, so run a small in-repo bake-off (section 4.10) before locking assignments.

---

## 3. Official prompting guidance per vendor

### 3.1 Anthropic (Claude models + Claude Code)

Sources: [Prompting best practices](https://platform.claude.com/docs/en/build-with-claude/prompt-engineering/claude-prompting-best-practices), [Prompting Opus 5.5](https://platform.claude.com/docs/en/build-with-claude/prompt-engineering/prompting-claude-opus-5-5), [Prompting Fable 5.1](https://platform.claude.com/docs/en/build-with-claude/prompt-engineering/prompting-claude-fable-5-1), [Claude Code best practices](https://code.claude.com/docs/en/best-practices).

**Prompt content**
1. **Be explicit, and give the why.** Treat Claude as "a brilliant but new employee who lacks context." Explaining motivation helps it generalize; for example, give the reason behind a formatting rule rather than just "NEVER do X."
2. **Use action verbs.** "Change this function…" gets edits, while "Can you suggest…" gets suggestions only. A `<default_to_action>` block flips the default.
3. **Use XML tags** to separate instructions, context, examples, and inputs. Use 3–5 diverse examples wrapped in `<example>` tags.

**Effort and thinking**
4. **Effort is the main dial**, and thinking is always on for Opus 5.5 and Fable 5.1.
   - Opus 5.5 defaults to `medium`, which in Anthropic's testing matches Opus 5 at `high` on coding. Reserve `xhigh`/`max` for measured gains, and lower effort rather than prompting "think less."
   - Fable 5.1 starts at `high`. At `low` it is "often competitive with Claude Opus and Claude Sonnet models on cost per task."

**Workflow**
5. **Give a verification check:** tests, a build, or a screenshot diff. "Claude stops when the work looks done." Ask for evidence (command plus output), not assertions. Use `/goal` or a Stop hook as a hard gate.
6. **Explore, then plan, then code, then commit**, using plan mode (Shift+Tab). Skip planning if "you could describe the diff in one sentence."
7. **Scope prompts:** name files, scenarios, and existing patterns to imitate, and describe symptom plus location plus what "fixed" looks like.
8. **Interview first for large features**: have Claude interview you with AskUserQuestion, write `SPEC.md`, then implement in a fresh session.
9. **Manage context aggressively.** Use `/clear` between tasks, and after two failed corrections, clear and re-prompt. Use subagents for investigation and `/compact <focus>` for long sessions.

**Instruction files and enforcement**
10. **Keep CLAUDE.md short.** "Bloated CLAUDE.md files cause Claude to ignore your actual instructions." Use "IMPORTANT" on one line at most, and use hooks for must-happen actions.

**Behavior control**
11. **Curb over-engineering and scope creep.**
    - Opus: use the "Avoid over-engineering… Scope / Documentation / Defensive coding / Abstractions" block.
    - Fable 5.1: "If… you find a pre-existing bug… don't fix… report it as a follow-up," and "Commit tests only where the task asks for them."
12. **Prevent test gaming:** "Do not hard-code values… Tests are there to verify correctness, not to define the solution… If… any of the tests are incorrect, please inform me rather than working around them."
13. **Unattended runs:** Opus 5.5 and Fable 5.1 may end a turn with a status report or "Shall I…?". Use the documented "You are operating autonomously…" block, and treat a text-only end of turn as a report, not completion. Keep a checklist file and auto-continue at most 2–3 times.
14. **Track multi-session state** in `progress.txt` plus a structured `tests.json` plus git. On a fresh context, Claude is "extremely effective at discovering state from the local filesystem."

**Specific situations**
15. **Frontend:** name the specific patterns to avoid (fonts, cream backgrounds, pill buttons, and so on). "Avoid generic AI look" alone just swaps one default for another.
16. **Edits:** for Fable 5.1, ask for surgical edits over whole-file rewrites.
17. **Review prompts:** tell reviewers to "flag only gaps that affect correctness or the stated requirements," or they will over-report.

### 3.2 OpenAI (Codex with GPT-6 Astra / 6.1 Sol / Luna)

Sources: [Codex prompting](https://learn.chatgpt.com/docs/prompting), [Codex models](https://learn.chatgpt.com/docs/models), [GPT-6 Astra guide](https://developers.openai.com/api/docs/guides/latest-model/gpt-6-astra.md), [model guidance](https://developers.openai.com/api/docs/guides/model-guidance), [GPT-5.6 prompt guidance](https://developers.openai.com/docs/guides/prompt-guidance-gpt-5p6), [Codex Prompting Guide (targets gpt-5.3-codex, older)](https://developers.openai.com/cookbook/examples/gpt-5/codex_prompting_guide), [The Decoder on OpenAI's July 2026 guide](https://the-decoder.com/openais-new-prompting-guide-tells-users-to-stop-overthinking-and-start-with-the-result/).

**Prompt content**
1. **Outcome first, lean prompts.** "Describe the destination rather than prescribing every step." State the outcome, constraints, evidence, and **completion bar**.
   - Removing redundant rules gave "~10–15% better scores with 41–66% fewer tokens" in OpenAI's internal coding-agent runs (directional only).
2. **Reserve ALWAYS/NEVER for true invariants.** "Conflicting rules can create more instability than missing detail."
3. **Codex prompt shape:** name the behavior, point to code or repro steps, and say how to verify. Examples: "Start by reproducing the bug locally, then propose a patch and run checks," and "run lint + the smallest relevant test suite. Report the commands and results."

**Workflow**
4. **Plan for multi-step work.** Use `/plan` before edits and `/goal` for a persistent objective. Split large refactors into milestones, implement one at a time, and review each diff.
5. **Steer vs queue:** in the CLI, Enter steers the current turn and Tab queues the next. Commit or stash before starting, "so you can compare changes cleanly."

**Effort**
6. **Reasoning effort:** Sol supports `low`…`max`, with `medium` as the API default. `none`/`minimal` are not supported on Astra/Sol.
   - In Codex, OpenAI suggests starting at "High for Luna or Light for Astra."
   - "Use high or xhigh only when evals show a meaningful gain." Before raising effort, check for a missing success criterion or verification loop.

**Astra-specific behavior control**
7. **Initiative:** Astra may stop to ask clarifying questions. Add "You should infer the user's intent and task scope from the instructions and prior conversation context" and "The user should be approving a concrete, reviewable result."
8. **Instruction-file hygiene:**
   - Astra is sensitive to AGENTS.md and skills, so audit them for conflicts.
   - Add "The user's instructions take precedence over guidelines provided in a skill."
   - Ask it to quote which instruction caused a pause.
9. **Subagents:** Astra may under-delegate, so say when to parallelize.
10. **Tests:** Astra over-tests. Use "Do not write tests for reversible, low-impact changes that mirror the implementation."
11. **Style:** Astra is verbose and heavily formatted. Ask for "clear, concise paragraphs, each developing one main idea."
12. **Autonomy boundaries:** analysis requests report findings, while fix requests make in-scope changes. Require confirmation only for external writes, destructive actions, or scope expansion, and don't sprinkle "ask first" everywhere.

**Codex harness conventions** (older guide, still sensible)
13. Prefer `rg`. Use `apply_patch` for edits. "NEVER revert existing changes you did not make." No broad catches or silent defaults. Avoid `as any`. For reviews, findings come first, ordered by severity.

### 3.3 xAI (Grok 4.7 / Grok Build)

Sources: [xAI best practices](https://docs.x.ai/developers/model-capabilities/text/best-practices), [reasoning](https://docs.x.ai/developers/model-capabilities/text/reasoning), [Grok 4.7 guide](https://docs.x.ai/developers/grok-4-7), [plan mode](https://docs.x.ai/build/features/plan-mode), [worktrees](https://docs.x.ai/build/features/worktrees), [Grok Bot for engineering](https://x.ai/bot/guides/grok-bot-for-engineering).

xAI's official prompting guidance is thin. It covers API mechanics far more than prompt wording.

**Effort and API mechanics**
1. **Set reasoning effort explicitly on every request.**
   - `grok-4.7` defaults to `high`; levels are `low`/`medium`/`high`/`xhigh`, and reasoning can't be disabled.
   - `low` is recommended for "agent loops and simple tool calling where latency matters"; `xhigh` for "the hardest problems."
2. **Caching** (for your own API harness): set a `prompt_cache_key` per conversation, keep the history append-only, and put static content first. "Without it you often pay full input price on a cache-cold server."
3. **Use the Responses API** and pass encrypted reasoning items back unchanged. Chat Completions is deprecated and drops reasoning between turns.
4. **Sampling:** xAI recommends `temperature` 0.7 / `top_p` 0.95 and says quality usually drops with other settings.

**Grok Build workflow**
5. **Plan mode:** `/plan` or Shift+Tab. Use it for "ambiguous architecture, unclear requirements, or high-impact restructures"; skip it for obvious fixes. Comment on plan lines before approving, because approval starts the build.
6. **Write thorough task prompts with proof requirements** (Grok Bot engineering guide): "thorough prompt detailing what needs to be done and what proof is expected." Give agents "a complete feedback loop" so they can launch and drive the app end-to-end, and require before/after screenshots for UI.
7. **Legacy grok-code-fast-1 guide** (official page now 404s; summary via [PromptLayer](https://blog.promptlayer.com/xais-prompt-engineering-guide-for-grok-code-fast-1/)): iterate quickly, give multi-step agentic tasks rather than Q&A, structure context with XML or Markdown headers, give only the relevant files, and be specific about endpoints, validation, and error formats.
8. **Keep rules short.** Rules files load "in full, with no size cap," and "short, specific instructions are followed more reliably." Run `grok inspect` to see token counts.

**Fleet-specific advice for Grok** (my synthesis, not xAI guidance)
- Give Grok **bounded, verifiable** tasks with explicit `done_when` commands.
- Prefer it for research briefs (with X citations), small fixes, and third-opinion reviews.
- Always route its PRs through Claude or Codex review.

### 3.4 Cross-vendor prompt template (works for all three)

```markdown
## Task T-0042: <title>
Goal: <observable outcome in one or two sentences>
Context: read tasks/T-0042-*.md, docs/contracts/<x>, crates/<mod>/AGENTS.md. Pattern to follow: <file>.
Scope: only touch paths listed in the task file. Out of scope: <…>. Report unrelated bugs as follow-ups; don't fix them.
Constraints: do not change public contracts in crates/contracts without an ADR; no new dependencies without noting them in the task file.
Done when (run these and paste results): `cargo test -p <mod>`, `cargo clippy -p <mod> -- -D warnings`, `pnpm -C ui test`, <e2e/screenshot if UI>.
Autonomy: proceed without asking for reversible in-scope steps; stop only for destructive actions or contract changes.
Before ending: update the task file's Handoff section; commit with Conventional Commits + `Agent:` trailer.
```

---

## 4. Multi-agent coordination protocol for one repo

General principles come from the vendor docs cited above. The concrete protocol is my recommendation for Streamer.live.

### 4.1 Isolation: one worktree and branch per agent per task

- **Never run two agents in the same checkout.** All three tools support worktrees natively:
  - **Claude Code:** `claude --worktree <name>` creates `.claude/worktrees/<name>/` on branch `worktree-<name>`. Subagents can use `isolation: worktree`. `.worktreeinclude` copies gitignored files such as `.env`. Add `.claude/worktrees/` to `.gitignore`. ([Claude worktrees](https://code.claude.com/docs/en/worktrees))
  - **Grok Build:** `grok -w` / `grok --worktree=<name>` creates `~/.grok/worktrees/<repo>/<name>`, **detached at base commit** (no named branch). Create your branch explicitly. Clean up with `grok worktree gc --max-age 7d`. ([Grok worktrees](https://docs.x.ai/build/features/worktrees))
  - **Codex CLI:** 0.162.0 "added tools to create and list managed Git worktrees". Codex cloud tasks run in isolated environments. ([Codex changelog](https://learn.chatgpt.com/docs/changelog), [Codex prompting](https://learn.chatgpt.com/docs/prompting))
- **Normalize branch names yourself** so all vendors look the same: `agent/<vendor>/<task-id>-<slug>`, for example `agent/codex/T-0042-obs-scene-switch`. A small `scripts/claim` wrapper can run `git worktree add ../wt/T-0042 -b agent/codex/T-0042-…` and then launch the right CLI inside it.

### 4.2 Task board: one file per task, with claim and lease

A single `TASKS.md` causes constant merge conflicts. Use `tasks/T-NNNN-slug.md`, one per task, with frontmatter:

```yaml
---
id: T-0042
title: OBS scene-switch action
status: todo | claimed | in_progress | review | blocked | done
owner: claude | codex | grok | human
model: claude-opus-5-5 | gpt-6.1-sol | grok-4.7
branch: agent/claude/T-0042-obs-scene-switch
claimed_at: 2026-10-18T19:05Z
lease_expires: 2026-10-19T19:05Z   # stale claims can be reassigned
paths: [crates/actions/src/obs/**, ui/src/lib/actions/obs/**]   # write-scope lock
depends_on: [T-0040]
contract: docs/contracts/actions.md#obs-scene-switch
done_when: ["cargo test -p actions", "pnpm -C ui test -- obs", "cargo clippy -- -D warnings"]
reviewer: codex          # always a different vendor than owner
adr: docs/adr/0007-action-plugin-model.md
---
## Spec
## Acceptance criteria
## Handoff   # appended by the agent at end of each session
```

How claims work:
- **The lock is the branch.** A task is claimed if and only if a branch `agent/*/T-0042-*` exists.
- **Claim on main.** You (the dispatcher) flip `status` and `owner` there, in a tiny commit `chore(tasks): claim T-0042 [claude]`. The agent edits **only its own task file** on its branch (Handoff, status → review).
- **Path leases stop conflicts before they happen.** `scripts/claim` refuses a task whose `paths:` globs overlap any other `claimed`/`in_progress` task. This prevents most merge conflicts.
- **The index is generated.** `tasks/README.md` is produced by a script from the frontmatter, never hand-edited by agents.

### 4.3 Module ownership (CODEOWNERS-like)

- **`.github/CODEOWNERS`** routes human review to you. Agents don't own code permanently; they own tasks.
- **Default routing** goes in each module's AGENTS.md as a one-line "Default implementer / reviewer" field:
  - `crates/core`, `src-tauri`: Claude implements, Codex reviews;
  - `ui/`: Claude Sonnet implements, Codex reviews;
  - protocol and algorithm crates: Codex implements, Claude reviews;
  - `docs/research`: Grok.
- **Shared, conflict-prone files are single-writer.** `Cargo.lock`, `pnpm-lock.yaml`, `crates/contracts/**`, root `AGENTS.md`, CI config, and `docs/decisions.md` change only through dedicated tasks, one at a time. Dependency bumps are always their own task.

### 4.4 Interface-first / contract-first

- **Define contracts before parallel implementation starts.** These are Rust types for IPC commands and events, the plugin/action API, and the WebSocket/HTTP API (API-first is a project requirement).
- **Keep them in one place:** `crates/contracts` plus `docs/contracts/`.
- **Generate TS types from Rust**, for example with `tauri-specta`, `specta`, or `ts-rs` (verify current versions before adopting). The UI and backend can then be built by different agents against the same compile-checked boundary.
- **CI fails on drift:** regenerate bindings and `git diff --exit-code`.
- **Contract changes require an ADR** and a separate PR that lands first. Implementation tasks reference the contract section in their `contract:` field.
- **Stubs first.** The contract PR includes stub implementations plus failing contract tests, so parallel agents have a target.

### 4.5 ADRs and a shared decision log

- **ADRs:** `docs/adr/NNNN-title.md` with Context / Decision / Consequences / Alternatives (MADR style). Agents may propose ADRs in a PR; only you accept them.
- **Decision log:** `docs/decisions.md` holds one line per decision (date, ID, summary, link to ADR/PR), newest first. Root AGENTS.md links to it and says "read before changing architecture." It is not imported, which keeps context small.
- **Research outputs** go in `docs/research/` with sources. Grok's live findings must cite URLs or X posts.

### 4.6 Commits, PRs, and cross-vendor review

- **Conventional Commits** (`feat(actions): …`, `fix(ui): …`, `chore(tasks): …`), enforced by commitlint in CI. Add trailers: `Agent: codex/gpt-6.1-sol`, `Task: T-0042`. Each tool's own co-author trailer is fine too.
- **Small PRs:** under ~400 changed lines, one task per PR, rebased on `main` before review.
- **Cross-review rule:** the reviewer's vendor must differ from the author's vendor.
  - Codex on Claude PRs: `codex /review` locally, or `@codex review` on GitHub ([The Decoder](https://the-decoder.com/openais-new-prompting-guide-tells-users-to-stop-overthinking-and-start-with-the-result/)).
  - Claude on Codex/Grok PRs: `/code-review` or a reviewer subagent with a fresh context ([Claude best practices](https://code.claude.com/docs/en/best-practices)).
  - Tell reviewers to "flag only gaps that affect correctness or the stated requirements" to avoid over-engineering churn.
- **You merge.** Optionally adopt xAI's pattern: auto-merge only when review confidence is high **and** blast radius is low ([Grok Bot guide](https://x.ai/bot/guides/grok-bot-for-engineering)).

### 4.7 CI as the arbiter

- **Required checks on `main`:**
  - `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --workspace`;
  - `pnpm typecheck`, `pnpm lint`, `pnpm test`, and Playwright/e2e smoke against `tauri dev` or a web build;
  - binding-drift check, commitlint, and a license/dependency audit.
- **"Done" means green CI plus a cross-vendor review**, never an agent's claim. Every vendor's guidance converges here: give the agent a runnable check, and require evidence of the commands and outputs.
- **Put deterministic enforcement in git hooks or CI, not agent-specific hooks.** Grok executes Claude hooks by default, Codex doesn't, and vendor-neutral `lefthook`/`pre-commit` runs for everyone.

### 4.8 Avoiding merge conflicts

1. Path leases (4.2), plus single-writer files (4.3).
2. Contracts land first (4.4). Parallel tasks then touch disjoint modules.
3. Rebase often. If a lease overlap is unavoidable, the task waits for a `depends_on`.
4. Never let agents reformat files they didn't change. Formatters run in CI and pre-commit, not as opportunistic whole-repo passes.
5. Generated files (bindings, lockfiles) are regenerated after merge, never hand-merged.

### 4.9 Handoffs and keeping context small

- **Handoff section** in each task file, written at the end of every session:
  - state: done / partial / blocked;
  - files touched;
  - commands run and results;
  - next steps;
  - gotchas;
  - open questions.

  This follows Anthropic's progress-notes plus structured-state plus git pattern ([best practices](https://platform.claude.com/docs/en/build-with-claude/prompt-engineering/claude-prompting-best-practices)).
- **Start every session the same way:** "Read tasks/T-NNNN, `git log -10`, and the module AGENTS.md; run `done_when` commands to see the current state."
- **Per-module `AGENTS.md`** (≤60 lines) states purpose, invariants, local commands, contract links, and default implementer/reviewer. **Per-module `README.md`** is for humans and longer explanations. Link it, don't import it.
- **Root AGENTS.md ≤150 lines.** Stay under Codex's 32 KiB chain cap and Claude's 200-line guidance. Push procedures into skills (`.claude/skills/*/SKILL.md`, which Grok also discovers), loaded on demand.
- **One task per session.** Use `/clear` (Claude) or a new session (Codex/Grok) between tasks, and prefer a fresh session plus the handoff note over long compaction chains.

### 4.10 Suggested rollout

1. Create the repo skeleton, root and module `AGENTS.md`, thin `CLAUDE.md`, `tasks/`, `docs/adr/0001-record-architecture-decisions.md`, CI, and the user-level Claude setting from 1.6.
2. Verify loading: `/memory` (Claude), `grok inspect`, and Codex "summarize your instructions" from root and from `--cd crates/x`.
3. **Bake-off:** give the same 3–4 real tasks to Opus 5.5, GPT-6.1 Sol, and Grok 4.7 in separate worktrees: one Rust module, one UI component, one protocol parser, one refactor. Compare CI pass rate, review findings, wall-clock time, and cost. Adjust the table in 2.5 from your own results, not from leaderboards.
4. Re-run the bake-off whenever a vendor ships a new flagship. All three shipped at least one in the last ~60 days.

---

## Appendix A: Minimal file stubs

**`CLAUDE.md`**
```markdown
@AGENTS.md

## Claude Code only (other agents: ignore this section)
- Use plan mode for changes touching crates/contracts or src-tauri.
- Use a reviewer subagent on your diff before setting a task to `review`.
```

**Root `AGENTS.md` skeleton**
```markdown
# Streamer.live — agent instructions
Product: <2 lines>. Architecture: docs/architecture.md. Decisions: docs/decisions.md (read before architectural changes).
## Commands
- Build: … | Test: … | Lint: … | Dev: …
## Conventions (non-default only)
## Workflow (all agents)
1. Work only on a claimed task in tasks/; stay inside its `paths:`.
2. Branch `agent/<vendor>/<task-id>-<slug>`; Conventional Commits + `Agent:` and `Task:` trailers.
3. Contracts (crates/contracts) change only via ADR + dedicated task.
4. Done = all `done_when` commands pass (paste output) + Handoff section updated.
5. Don't touch lockfiles/CI/root AGENTS.md unless the task says so. Report unrelated bugs as follow-ups.
## Code Review Rules
- Findings first, by severity; correctness and requirements only; formatting is CI's job.
## Security
- No secrets in code/logs; scripting/plugin execution stays sandboxed; …
```

## Appendix B: Version checklist (re-verify periodically)

| Item | Value as of 2026-10-18 | Where to check |
|---|---|---|
| Claude Code AGENTS.md native read | v2.1.277+ (setting ID `cc-plugin-agents-md@builtin` from v2.1.285) | [memory docs](https://code.claude.com/docs/en/memory) |
| Codex AGENTS.md cap | 32 KiB (`project_doc_max_bytes`) | [Codex AGENTS.md](https://learn.chatgpt.com/docs/agent-configuration/agents-md) |
| Codex recommended model | `gpt-6.1-sol` | [Codex models](https://learn.chatgpt.com/docs/models) |
| Grok Build rules files | AGENTS.md/AGENT.md/CLAUDE.md family + `.grok/rules/`; no cap | [Grok project rules](https://docs.x.ai/build/features/project-rules) |
| Grok coding model | `grok-4.7` | [xAI models](https://docs.x.ai/developers/models) |
| Codex repo skills path | **Unverified this session** | Codex docs |
