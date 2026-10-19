# S-5 agent bake-off — 2026-10 results

| | |
|---|---|
| Work item | DEN-7 (children DEN-8…DEN-11, label `bakeoff`) |
| Runs | 2026-10-18 23:35 → 2026-10-19 00:20 (America/Chicago) |
| Write-up | claude/claude-opus-5-5, 2026-10-19 (delegated overnight by the owner to the coordinator) |
| Evidence | The 12 cross-reviews and the coordinator's timing log, copied verbatim into [bakeoff-2026-10/](bakeoff-2026-10/). The vendor handoffs are on cards DEN-8…DEN-11. The code is on branches `agent/{claude,codex,grok}/DEN-{8,9,10,11}-b{1,2,3,4}`. |
| Status | **Provisional.** The owner rating column is still `pending`. The DELEGATION.md changes are marked *from bake-off 2026-10, owner to confirm*. |

## Verdict

- **Claude (Opus 5.5)** had the fewest serious findings: 0 P1 and 2 P2 across 4 tasks, mean reviewer score 4.25/5.
  Its B-1 (Rust queue) and B-2 (EventSub parser) entries had no findings at all. It was the middle vendor on
  wall-clock time, about 33 min in total. That time includes a fresh-context self-review the other vendors didn't
  run (see Caveats).
- **Codex (GPT-6.1 Sol)** was the fastest on every task, about 18 min in total, roughly 2× faster than Claude. It
  also wrote the smallest diffs. But it had one P1: on B-2, a typed-parse failure drops the whole EventSub frame.
  Its two Rust/protocol entries scored 3/5. Mean reviewer score 3.5/5.
- **Grok (4.7)** was the slowest, about 71 min in total, 2–4× slower than the others. It had the most serious
  findings: 3 P1s (one each on B-1, B-2 and B-3) and 10 P2s. Mean reviewer score 3.25/5. Its only P1-free task was
  CI (B-4). This matches the existing guidance: don't give Grok long unsupervised implementation work.
- **Every one of the 12 entries passed its Done-when** at the end of its first dispatched session, and each
  reviewer confirmed this independently. Even so, 10 of the 12 had P2-or-worse findings. **On this sample,
  Done-when was not a quality gate**, and the cross-vendor review is what found the real defects.

## Method

- **Tasks** (cards DEN-8…DEN-11, same text for every vendor):
  - B-1: a bounded Tokio action queue (blocking or non-blocking, pause, resume, clear, always-run, cancel) with
    a proptest.
  - B-2: typed parsing of Twitch EventSub WebSocket frames and 9 notification types, with golden fixtures from
    the official docs.
  - B-3: a React 19 sub-action picker (a tree of about 200 items, fuzzy search, favourites, recents, keyboard
    and ARIA, dark theme), with Vitest and a Playwright screenshot.
  - B-4: a reusable GitHub Actions CI for Cargo + pnpm across 5 OS targets (`actionlint` plus a written dry run).
  - Note: B-4 differs from the "rename and restructure across ~15 files" task in DELEGATION.md §6. The card is
    what ran.
- **Vendors and models:** claude-opus-5-5 (`clx`), gpt-6.1-sol (`cdx`) and grok-4.7 (`cgx`), all at default
  effort.
- **Dispatch:** each vendor got the same dispatch prompt and autonomy note, in its own worktree and branch.
  All runs were headless and unattended. Up to 4 runs shared the same Linux box at a time.
- **Scope:** every entry is one commit (Grok B-4 is two) and touches only `spikes/S-5-bakeoff/<vendor>/b<n>/**`.
  I checked this with `git diff --name-only <merge-base> <branch>`: 0 files outside scope on all 12 branches.
- **Review:** each entry got one cross-vendor review (`git diff origin/main...HEAD`, read-only). Each review
  re-ran Done-when, listed findings by severity (P0–P3) and gave a quality score (1–5). The pairings:

  | Author | Reviewed by |
  |---|---|
  | Claude | Codex, all 4 |
  | Codex | Claude, all 4 |
  | Grok | Claude on B-2, B-3 and B-4; Codex on B-1 |

- **Wall-clock:** for Claude and Codex, the wrapper duration of the first run. For Grok, launch to commit,
  taken from timestamps (less precise). Source: [_timings.txt](bakeoff-2026-10/_timings.txt).

## Results

**First-run DW** means Done-when passed at the end of the first dispatched session, as confirmed by the
reviewer. This is looser than §6's "pass on first `done_when` run": under that stricter reading, Claude B-3
and Grok B-3 would be N (note 2). **Score** is the reviewer's quality score (1–5) for the entry. The owner rating stays `pending` until
the owner fills it in.

| Task | Vendor | Branch @ commit | First-run DW | P0 | P1 | P2 | P3 | Wall-clock | Diff (+lines) | Score | Owner rating |
|---|---|---|---|---|---|---|---|---|---|---|---|
| B-1 Rust queue | Claude | `agent/claude/DEN-8-b1` @ `6797512` | Y | 0 | 0 | 0 | 0 | 9m51s | 1,932 | 5 | pending |
| | Codex | `agent/codex/DEN-8-b1` @ `363e663` | Y¹ | 0 | 0 | 2 | 6 | 3m19s (+58s commit re-run¹) | 800 | 3 | pending |
| | Grok | `agent/grok/DEN-8-b1` @ `0200459` | Y | 0 | 1 | 1 | 0 | ~29m | 1,701 | 3 | pending |
| B-2 EventSub | Claude | `agent/claude/DEN-9-b2` @ `4a1d436` | Y | 0 | 0 | 0 | 0 | 7m51s | 1,932 | 4 | pending |
| | Codex | `agent/codex/DEN-9-b2` @ `a3acdc7` | Y | 0 | 1 | 2 | 4 | 4m28s | 1,475 | 3 | pending |
| | Grok | `agent/grok/DEN-9-b2` @ `8b0e290` | Y | 0 | 1 | 2 | 2 | ~16m | 1,861 | 3 | pending |
| B-3 React picker | Claude | `agent/claude/DEN-10-b3` @ `5cdef2b` | Y² | 0 | 0 | 1 | 0 | 10m02s | 4,773 | 4 | pending |
| | Codex | `agent/codex/DEN-10-b3` @ `38b4b95` | Y | 0 | 0 | 2 | 6 | 6m43s | 2,203 | 4 | pending |
| | Grok | `agent/grok/DEN-10-b3` @ `7a02907` | Y² | 0 | 1 | 4 | 4 | ~16m³ | 4,962 | 3 | pending |
| B-4 CI | Claude | `agent/claude/DEN-11-b4` @ `5542353` | Y | 0 | 0 | 1 | 0 | 5m41s | 913 | 4 | pending |
| | Codex | `agent/codex/DEN-11-b4` @ `a31801b` | Y | 0 | 0 | 2 | 6 | 3m34s | 320 | 4 | pending |
| | Grok | `agent/grok/DEN-11-b4` @ `b658c51` | Y | 0 | 0 | 3 | 5 | ~10m | 527 | 4 | pending |

**Totals per vendor**

| Vendor | First-run DW | P1 | P2 | P3 | Mean score | Total wall-clock | Quota |
|---|---|---|---|---|---|---|---|
| Claude Opus 5.5 | 4/4 | 0 | 2 | 0 | 4.25 | ~33m | not measured⁴ |
| Codex GPT-6.1 Sol | 4/4 | 1 | 8 | 22 | 3.50 | ~18m (+58s) | about 7% of the 168 h window at 00:05⁴ |
| Grok 4.7 | 4/4 | 3 | 10 | 11 | 3.25 | ~71m | not measured⁴ |

Notes:

1. Codex's first B-1 run passed every check but couldn't commit, because the coordinator had launched it with a
   `workspace-write` sandbox and a read-only `.git`. This was the coordinator's mistake, not Codex's. A 58 s
   re-run committed the same code unchanged. Every later Codex run used `danger-full-access`.
2. The Done-when commands passed when the entry was handed off, but the session went through red runs first,
   which it fixed itself:
   - Claude B-3: Playwright browser missing, then a test race.
   - Grok B-3: three typecheck errors, 7 Vitest failures, and a Playwright locator.
   - The other handoffs report no red Done-when runs.
3. Grok B-3 has two start times in the timing log: "started 00:04:19" and "started ~00:00". I used 00:04 to
   00:20. With 00:00 it would be about 20 min.
4. Quota wasn't measured per run. The wrappers have no quota telemetry for Claude, and Grok has no quota view.
   The Codex figure was read once, after its four runs.

## Per-task observations

### B-1 — Rust bounded action queue (concurrency)

- **Claude, 5/5, no findings.**
  - Design: a `QueueRegistry` plus a single-flight pump under one mutex per queue, with no dispatcher task and
    no user code run under the lock.
  - Tests: a 512-case proptest on a paused clock, which checks peak ≤ limit, FIFO and pause. Also a
    20,000-run runtime-shutdown regression test.
  - Its own fresh-context self-review had found a stack overflow on shutdown with a large backlog. It fixed this
    before committing.
- **Grok, 3/5.**
  - P1: the same class of bug that Claude's self-review caught. Shutting down the runtime recursively
    dispatches the backlog, which overflows the stack and aborts the process. Codex reproduced it with an
    external harness (exit 134).
  - P2: cancelling a running job frees its slot before the job stops, so two jobs can run at once in blocking
    mode. Reproduced.
- **Codex, 3/5.**
  - P2: an always-run job on a paused, full queue is rejected with `Full` even when run slots are free.
  - P2: "named queues" are only labels; there's no registry.
  - Six P3s, including a narrow proptest and timing-dependent negative assertions.
  - Compact, about 800 lines, and the fastest entry.
- **Takeaway:** concurrency is where the vendors differed most. Only the Claude entry got through without a
  correctness finding.

### B-2 — Twitch EventSub parsing (protocol)

- **Claude, 4/5, no findings.**
  - Design: `Event::Raw { parse_error }` for a known event that fails typed parsing, and
    `Message::Malformed` for a bad envelope. Every struct has a flattened `extra` map, and each test asserts that
    `extra` is empty. String enums have an `Other` fallback.
  - Codex checked all 16 fixtures against the official docs and found they matched.
- **Codex, 3/5.**
  - P1: a `null` or a type change in any non-optional typed field makes the whole frame `Err`, losing the
    metadata and the raw event. New fields are tolerated, because there's no `deny_unknown_fields`.
  - P2: the golden round trip just echoes the stored raw JSON, so it's nearly a tautology for typed fields
    (shown by a mutation check).
  - P2: the typed structs are `Deserialize`-only, so they can't be serialized back.
- **Grok, 3/5.**
  - P1: `deny_unknown_fields` on every struct. One added Twitch field breaks parsing. The reviewer showed that
    `recovery_url` on `session_welcome` fails parsing, and says live welcome frames carry that key. If they do,
    the first frame of every real connection fails.
  - P2: the fixtures were reconstructed from search excerpts, because the docs returned HTTP 403 to Grok's
    fetch tool. They aren't copied from the docs.
  - P2: `channel.chat.message` is typed for any version.
- **Takeaway:** the deciding factor was **tolerance of upstream schema drift**, not parsing accuracy. That bears
  on the "protocol, parsers" row in §2: it's currently routed to Codex, and this sample doesn't support that.

### B-3 — React sub-action picker (UI and a11y)

- **Claude, 4/5.**
  - P2: going back from search to the same active row doesn't restore focus. Codex reproduced this with two
    Playwright tests.
  - The self-review had fixed 5 issues before commit, including focus loss and gaps in the fuzzy matcher.
- **Codex, 4/5.**
  - P2: un-favouriting in the Favourites view drops focus to `BODY` and disables Escape.
  - P2: the fuzzy match runs over name + category, so `set` matches 91 of 200 items.
  - Six P3s, including very dense code (one line is about 600 characters).
- **Grok, 3/5.**
  - P1: the dialog isn't constrained to the viewport. One search letter pushes it fully off-screen. The
    committed screenshot baseline shows the broken render, so the test passes anyway.
  - P2: Alt+F does nothing on macOS.
  - P2: Home and End are taken away from the search box.
  - P2: Tab focuses the tree container instead of the active item.
  - P2: the fuzzy search over-matches.
- **Takeaway:**
  - Claude and Codex were close here. Both had keyboard-focus bugs.
  - A screenshot test can lock in a broken render. Done-when should add a viewport or bounding-box assertion.
  - The bake-off ran Opus, not Sonnet, so it says nothing about the Sonnet routing for UI.

### B-4 — GitHub Actions CI (ops)

- **All three scored 4/5.** No entry had a P1, and none ran on GitHub.
- **Claude:**
  - P2: `cancel-in-progress` is off on main. Codex says this fails the "superseded-run cancellation" goal, and
    that GitHub's queue keeps only one pending run, so "every main commit gets a result" isn't actually
    guaranteed.
  - Claude's entry is the only one with `workflow_call` inputs, which makes it truly reusable.
  - It is also the only one with a real linter (`biome ci`).
- **Codex:**
  - The smallest entry (320 lines) and the fastest.
  - P2: not reusable, because the paths are hard-coded.
  - P2: nextest is compiled from source on every matrix entry.
- **Grok:**
  - P2: not reusable, because the paths are hard-coded.
  - P2: lint is a syntax check only.
  - P2: a stale Arch image plus `pacman -Syu` is likely to hit keyring failures. Plausible, but not reproduced.
- **Takeaway:** on CI, Codex's speed and size advantage is real, and the quality gap is small. Two disagreements
  remain unresolved:
  - **Cancelling runs on main.** Codex flagged Claude's "no cancel on main" choice as P2. The Claude reviewers
    recommended exactly that choice to Codex and Grok, as P3.
  - **The Arch package name.** The Claude reviewer says `libappindicator-gtk3` still resolves through `Provides`.
    Grok's handoff says `pacman -Si` reports it missing on the same host.

## Review quality (the reviewers, not the authors)

- **Codex as reviewer** (5 reviews):
  - Few findings, high signal. Both Grok B-1 bugs and the Claude B-3 focus bug were proven with external
    harnesses or Playwright reproductions.
  - It checked every Claude B-2 fixture against the live docs.
  - It reported no P3s at all.
- **Claude as reviewer** (7 reviews):
  - Broad coverage: mutation checks, negative controls, action-pin verification with `git ls-remote`, and
    `pacman -Si`.
  - It produced about 4.7 P3s per review (33 across 7 reviews; 5.5 per review on the Codex entries). One P2 was explicitly "plausible, not reproduced" (Grok B-4 keyring).
- **What this means for the counts:**
  - **Compare P3 counts across authors with care.** Every Codex entry was reviewed by Claude, and Claude
    reviews list many more P3s.
  - The P1 and P2 counts are a fairer comparison.
  - Grok wasn't used as a reviewer in this bake-off, so there's no data on its review quality.

## Caveats

- **Tiny sample.** One run per vendor per task, so 12 data points. A single P1 moves a vendor's mean by a lot.
  Treat every conclusion as directional.
- **Headless and unattended runs.** Nobody answered questions or steered, so this measures autonomous behaviour.
  Interactive use may rank differently.
- **Unbalanced reviewer pairings.** Claude entries were reviewed only by Codex. Codex entries were reviewed only
  by Claude. Reviewer style adds noise, as described above.
- **Claude ran an extra self-review step.** The repository's `CLAUDE.md` tells Claude Code to run a
  fresh-context reviewer subagent before handoff. That step caught real bugs: the B-1 stack overflow, 5 B-3
  issues, and 3 B-4 issues. It also added to Claude's wall-clock. Codex and Grok had no equivalent step, so part
  of Claude's quality lead is a process difference, not only a model difference.
- **Coordinator sandbox issue on Codex B-1** (note 1 above). Codex's B-1 time doesn't include the 58 s commit
  re-run.
- **Grok has no Tier-3 model on this host.** The fleet policy rates concurrency work like B-1 as Tier 3, and
  Grok's best model, grok-4.7, is Tier 2. So Grok ran B-1 one tier under the requirement. Claude
  (claude-opus-5-5) and Codex (gpt-6.1-sol) both ran Tier 3.
- **Concurrent runs on one machine** share CPU for cargo and pnpm builds, so wall-clock times are approximate.
  Grok's times come from timestamps, not wrapper durations.
- **Quota wasn't measured per run** (note 4).
- **Codex handoffs are missing on DEN-9 and DEN-10.** Those two cards have handoffs from Claude and Grok only. The
  Codex results for those tasks come from the timing log, the commits and the reviews. I couldn't find out why.
  Shared claims on bake-off cards are a likely cause (DEN-21).
- **No owner rating yet.** The `Owner rating` column is the owner's 1–5 rating and is still to be filled in.
- **No hosted CI.** No entry ran on GitHub or on Windows or macOS. The B-4 evidence is static.

## Recommendations

1. **Rust engine and concurrency code: Claude Opus.**
   - This includes isolated algorithms like the queue scheduler, which WS-03 currently gives to Codex.
   - Codex Sol is still the reviewer. Its reviews produced the reproduced concurrency bugs.
   - Applied to DELEGATION §2–3.
2. **Protocol and wire parsers: lower confidence in Codex-primary. Have Claude Opus write the first parser
   pattern.**
   - The pattern means tolerant decoding, a raw fallback, and an `extra` map plus an assertion that it's empty.
   - Codex Sol can then do the bulk connectors, as WS-11 already does for integrations.
   - Applied to DELEGATION §2 (confidence) and §3 (WS-05).
3. **CI and ops: keep Codex Sol.** It was fastest with the smallest diff, and the quality gap was small. No routing
   change.
4. **Grok: keep it on research and small fixes.** On implementation it was the slowest vendor and had 3 P1s in 4
   tasks. This confirms the existing §1 "avoid" note, so §1 isn't changed.
5. **Make the fresh-context self-review step vendor-neutral.** Put it in AGENTS.md or PROMPTING.md §2 so Codex and
   Grok run it too. It's the cheapest quality lever this bake-off found. This is a change to single-writer files,
   so it isn't made here. It needs its own card.
6. **Harden Done-when.** It passed 12 out of 12 times by the end of the first session (10 of 12 on the very
   first run, see note 2) and didn't separate the vendors. Card authors should add
   checks that fail on the defects found here:
   - a viewport assertion for UI screenshots;
   - a schema-drift fixture for parsers;
   - a shutdown and cancellation stress test for async code.
7. **Next bake-off:**
   - balance the reviewer pairings, including Grok as a reviewer;
   - measure quota per run;
   - include Sonnet 5.5 on the UI task;
   - either align DELEGATION §6 B-4 with what ran or run the refactor task too;
   - re-run when a vendor ships a new flagship.

## Changes made to DELEGATION.md

Each change is marked *from bake-off 2026-10, owner to confirm* in the file:

- **§2, Rust engine and systems code:** confidence raised from Low–medium to Medium.
- **§2, Protocol, parsers, algorithms, rate limiting:** confidence lowered from Medium to Low. Added a note to
  have Claude Opus write the first drift-tolerant wire parser, with Codex Sol reviewing that parser.
- **§3, WS-03:** the queue scheduler moves from Codex Sol to Claude Opus.
- **§3, WS-05:** Claude Opus writes the first connector's wire-model and parsing pattern, then Codex Sol does the
  rest. Codex Sol reviews that Claude-written pattern, so the reviewer is still a different vendor from the
  author.

Rows the data doesn't speak to are unchanged: UI and Sonnet, tests, bulk adapters, docs, research, and the
WS-01 CI routing (the data agrees with it).
