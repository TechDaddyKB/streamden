# DEN-10 b3 — claude, reviewed by codex

Done-when: PASS — independently run on Linux against the reviewed entry: `pnpm typecheck` exited 0; `pnpm test` exited 0 (2 files, 24 tests passed); `pnpm test:e2e` exited 0 (2 tests passed, including all three screenshot comparisons). This describes the card's supplied checks; the additional review reproductions below fail.

## Findings

### P2 — Returning from search to the same active row does not restore keyboard focus

- **File:line:** `spikes/S-5-bakeoff/claude/b3/src/SubActionPicker.tsx:148` (the `moveTo` helper, lines 148–152; focus effect at lines 134–140).
- **Failure scenario:** On a fresh page, press ArrowDown to focus Core, ArrowUp to return to search, then ArrowDown again. Focus remains in search instead of returning to Core. Likewise, search for `delay`, press ArrowDown to focus Delay, press `/` to return to search, then ArrowDown: focus remains in search. Further ArrowDown presses cannot resume tree navigation until another action changes state. This breaks the card's full keyboard navigation requirement and the documented search-box ArrowDown behavior.
- **Cause:** `moveTo` sets a ref and calls `setActiveId(id)`. When that ID is already active, React skips rendering, so the effect that consumes `focusPending` never runs. Moving focus to search does not clear the active ID.
- **Suggested fix:** Focus and scroll the existing row directly when moving to the already-active ID, or trigger focus through state that changes for every focus request. Keep deferred focus for rows that need rendering. Add regression coverage for both search/tree round trips.
- **Independent evidence:** Added two Playwright reproductions only in the temporary review copy. `pnpm exec playwright test e2e/reviewer-focus.spec.ts` exited 1: **2 failed**, both at the final `toBeFocused()` assertion (`Expected: focused; Received: inactive`). Existing tests cover the initial entry into the tree but not re-entry to the unchanged active row.

No P0, P1, or additional P3 findings.

## Quality 1-5

**4/5** — Covers the requested fixture, fuzzy search, favourites, recents, dark CSS-module UI and tree semantics with useful passing tests and screenshots; the reproducible keyboard focus regression prevents a higher score.

## Review scope and evidence

- Reviewed only `spikes/S-5-bakeoff/claude/b3/**` from `git diff origin/main...HEAD` on `agent/claude/DEN-10-b3`, HEAD `5cdef2bf1cb54ecd01c1f6aa45db5adda9c3c97c`, origin/main `adff917ef4563c955b60ac5923fe8edda7a5e800`.
- Read DEN-10 with `project_card_get` and evaluated its Goal, Done-when, and AGENTS.md code review rules. No card claim, update, movement, or release was performed.
- To preserve the repository, ran commands in `/tmp/den10-claude-review.qsU1nh`, a copy of the entry. The original source was unchanged; the two reviewer tests were added only after the supplied checks completed.
- Screenshot baselines successfully compared: `spikes/S-5-bakeoff/claude/b3/e2e/__screenshots__/picker-default-linux.png`, `picker-search-linux.png`, and `picker-favourites-linux.png`.
- Assumption: Done-when status records the supplied acceptance commands separately from additional review findings. This is a review, so no implementation fixes were attempted. No product decisions or questions were needed.
- Repository status was clean before and after verification; no repository file was edited. Required fleet reflection lookup returned `status: missing` for `fleet-reflect`; no actionable fleet finding arose.
