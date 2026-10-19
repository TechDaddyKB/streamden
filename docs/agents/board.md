# Work board — codex-orchestrator project `streamden`

The **codex-orchestrator board** (project slug **`streamden`**, console at `http://localhost:8488`) is the single
source of truth for work items: status, who holds what, dependencies, and handoff notes
([ADR-0005](../adr/0005-work-management-and-agent-tooling.md)). Every vendor reaches it through the orchestrator
MCP that `clx`, `cdx` and `cgx` install. Docs, specs and contracts stay in git.

(The earlier Plane project "DEN" was archived on 2026-10-18. Card numbers were kept identical: **DEN-n = card #n**.)

## Identity
- Card titles start with `DEN-<n> · `. Refer to work as **DEN-<n>** in branches, commits and docs.
- Branch: `agent/<vendor>/DEN-<n>-<slug>`. Commit subject names the card `(DEN-<n>)`, then a body and one `AI-Assisted-By:` trailer (D-037, see AGENTS.md).
- `username` you pass to the board: `<vendor>/<model>`, e.g. `claude/claude-opus-5-5`, `codex/gpt-6.1-sol`,
  `grok/grok-4.7`. `worktree_path`: the absolute path of your worktree.

## Lanes (board template "software")
| Lane | Key | Role allowed | Used for |
|---|---|---|---|
| Backlog | `backlog` | any (intake) | Not started. The owner triages and labels. |
| Planning | `planning` | `plan` | Specs, ADR drafts, research cards in progress |
| Coding | `coding` | `code` | Implementation and spikes in progress |
| Review | `review` | `review` | Cross-vendor review: the reviewer's vendor ≠ the author's |
| Verifying | `verifying` | `verify` | The owner checks the evidence and merges the PR |
| Done | `done` | — (terminal) | Merged. **Only the owner moves cards here.** |
| Blocked | `blocked` | — | Waiting on a decision, owner action or external factor; `blocked_reason` says why |

Dependencies use the card's `depends_on` (card numbers). A card with `ready: false` lists `waiting_on`. Don't
start it.

## Labels
- `agent:claude` · `agent:codex` · `agent:grok` · `agent:human`: who implements (set by the owner when dispatching)
- `reviewer:claude` · `reviewer:codex` · `reviewer:grok`: who reviews (must differ from `agent:*`)
- `phase:P0` … `phase:P6` · `kind:spike|spec|contract|impl|test|research|docs|ops|owner-action` · `WS-01` … `WS-18`
- `bakeoff`: S-5 cards that every vendor runs on its own branch
- `parked`: deliberately deferred by the owner (sits in Blocked with a `blocked_reason`). Don't touch until un-parked.

## Agent protocol (MCP calls)
1. **Orient:** `project_summary {slug:"streamden"}` (cheap; shows open cards and what you hold), then
   `project_card_get {slug, card:<n>}` for your card's full detail and history.
2. **Claim** (only a card whose `agent:<vendor>` label is yours, or whose `reviewer:<vendor>` label is yours when
   it's in Review, or an owner-dispatched shared `bakeoff` card):
   `project_card_claim {slug, card, role, worktree_path, username, client_request_id:"DEN-<n>-<vendor>-<mode>-<dispatch-id>"}`.
   `mode` is `work` or `review`, not the board role. Generate the ID once per dispatch and reuse it exactly
   for transport retries within that dispatch. A later dispatch (including another round on the same day)
   needs a fresh ID. `scripts/dispatch.sh` uses a UTC timestamp (`YYYYMMDDTHHMMSSZ`), process ID and random
   suffix for `dispatch-id`; manual sessions may use a fresh UUID instead. Keep punctuation outside the ID.
   Passing `worktree_path` + `username` ties the claim to your session, so it frees itself if you die.
   Confirm the live claim with `project_card_get {slug, card, worktree_path, username}` and require
   `claim.yours:true` before working, moving or releasing; a replayed `claimed:true` is insufficient.
   If another vendor/session holds a shared bake-off card, report the holder and stop this dispatch;
   never release or move their live claim. If unheld but not yours, report and stop; do not reuse that ID
   for a new dispatch. Only after confirming ownership, `project_card_move` a Backlog card to the lane
   for your role (`planning` for plan, `coding` for code).
3. **Stay alive:** any call naming the card renews the claim (30-min TTL). On long runs, poll
   `project_card_get` with `worktree_path` + `username`.
4. **Finish a step:** `project_card_release {slug, card, note:<handoff>}`:
   - no `resolution`, when every Done-when passes: the card **advances** to the next lane (coding → review);
   - `resolution:"handoff"`: partial work, the card stays where it is for the next session;
   - `resolution:"blocked"` + the reason in `note`: the card moves to Blocked;
   - never `resolution:"done"`, which is the owner's call.
   **Keep `note` to one short line (under ~200 chars).** Long notes currently fail with a database error. Put
   the full handoff (template: [`tasks/_TEMPLATE.md`](../../tasks/_TEMPLATE.md) §B) into the card itself:
   `project_card_update` with `detail` = existing detail + an appended `### Handoff — <date> — <vendor>/<model>`
   section.
5. **New work you discover:** `project_card_create` (lands in Backlog) with `kind:*`, `phase:*` and `WS-*`
   labels and a `detail` following §A of the template. Mention its number in your handoff. Don't claim it.
6. **Merging locally:** Git Director (`git_register` → `git_merge_request` → `git_release`), see AGENTS.md.
7. **Never** put secrets in card details, notes, or orchestrator memory.
