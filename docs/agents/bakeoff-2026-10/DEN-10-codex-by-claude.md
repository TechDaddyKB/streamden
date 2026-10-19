# DEN-10 b3 — codex, reviewed by claude

Reviewer: Claude (claude-opus-5-5), agent Ursula `7fe21757-da3b-4aa0-b0b1-efa300390a2d`, 2026-10-19.
Target: `spikes/S-5-bakeoff/codex/b3` on `agent/codex/DEN-10-b3` @ `38b4b95` (`git diff origin/main...HEAD`: 19 files, +2203, all in the card's scope; ~1.8k of the lines are `pnpm-lock.yaml`, the 200-item fixture is a single JSON line, and there's one PNG).

Done-when: PASS

Evidence: run by the reviewer in `spikes/S-5-bakeoff/codex/b3` with Node 26.7.0 and pnpm 12.14.0. `pnpm install --frozen-lockfile` reported the lockfile up to date.
- `pnpm typecheck` → `tsc --noEmit`, exit 0, no diagnostics.
- `pnpm test` → Vitest 3.2.4: `Test Files 1 passed (1)`, `Tests 9 passed (9)`, exit 0. The tests cover search, keyboard nav (arrows, Home/End, Left/Right, type-ahead), selection with Enter and the mouse, favourites, recents, malformed or blocked storage, and roving tabindex.
- `pnpm screenshot` (= `playwright test`, Chromium headless) → `2 passed (1.5s)`, exit 0. It compared against the committed baseline `e2e/picker.spec.ts-snapshots/picker-linux.png` (1280px, full page, dark), which I inspected: it renders correctly. A second run with a fresh `PWTEST_CACHE_DIR` also passed.
- Port 5173 was free before the run. The worktree stayed clean afterwards (`git status --short` was empty; only ignored `node_modules/`, `dist/` and `test-results/` exist).
- Oddity, not a defect: the Playwright reporter labels the second test `picker.spec.ts:44:1` even though the file has 33 LF lines and no hidden characters. The test still runs and passes.

The four extra behaviour probes below ran in a throwaway copy under `/tmp`, which has since been deleted. No repo file was edited.

## Findings

### P0
None.

### P1
None.

### P2

1. **Keyboard focus is lost, and Escape stops working, after un-favouriting in the Favourites view.** `src/Picker.tsx:30-34` and `:72`.
   - Failure scenario (reproduced with a Testing Library probe): favourite "Set variable", open Favourites, select it, then activate "★ Remove favourite". The item leaves `rows`, so `current` falls back to the category row and `candidate` becomes `undefined`. The favourite button that had focus unmounts and `document.activeElement` becomes `BODY`. The Escape handler sits on the `<section>` (`:59`), so pressing Escape now does nothing (the probe showed the tree still open). A keyboard or screen-reader user is dropped to the top of the page with no way to dismiss the picker from the keyboard. The same thing happens when un-favouriting moves the leaf out of the visible rows in any filtered view. The existing test (`src/Picker.test.tsx:48-59`) is titled "…without corrupting focus or selection" but never asserts focus.
   - Suggested fix: in `favourite()`, when the item is about to disappear from the current view, move focus to the nearest remaining row (next sibling, else parent category, else the search input). Or keep the details button mounted and disabled until focus moves. Add a `toHaveFocus` assertion to the test.

2. **The match set is very noisy because subsequence matching runs over `name + category`.** `src/model.ts:6-15`, used at `src/Picker.tsx:16-17`.
   - Failure scenario: typing `set` returns 91 of 200 actions (probe). Any action whose name and category contain `s…e…t` in order matches, for example through "Re**set**" or letters spread across the verb, subject and category. Ordering puts tight matches first, but only *within* each category, because the categories are always rendered in fixed order (`:19`). So the best overall match can sit below dozens of weak matches in earlier categories. With about 200 real items and longer names, the "fuzzy" search turns into "everything".
   - Suggested fix: score name and category separately and prefer word-start or contiguous hits (bonus for a boundary match, penalty for gaps). Cap the gap cost, or reject matches whose cost is over a threshold relative to the query length. Order the category groups by their best item score while searching, and auto-focus or preview the best match.

### P3

3. **Search state persists across close and reopen.** `src/Picker.tsx:7-8,25`. After adding an action found by `stscn`, reopening the picker still shows `stscn` and the filtered tree (probe; the e2e test at `e2e/picker.spec.ts:12-14` relies on this without noticing). Users expect a fresh picker. Fix: reset `query`, `active` and possibly `view` in the opener's `onClick` (or unmount the picker as its own component with local state).

4. **Escape in a non-empty search box closes the whole picker.** `src/Picker.tsx:59,61`. The common pattern (and the Streamer.bot-style picker) clears the query on the first Escape and closes on the second. Fix: in the input's `onKeyDown`, if `query` is non-empty, clear it and `stopPropagation()`.

5. **Assistive technology can't perceive favourite state in the tree.** `src/Picker.tsx:68`. The leaf `<li>` has `aria-label={action.name}`, which overrides the content, so the `★` span's `aria-label="Favourite"` is never part of the accessible name or description (probe: the accessible name is just "Set variable"). Fix: drop the leaf `aria-label` and let the text content name the item with a visually hidden ", favourite" suffix, or use `aria-describedby`.

6. **Favourites have no keyboard shortcut from the tree.** `src/Picker.tsx:35-53`. Toggling a favourite requires Tab to the details-pane button and back, which is reachable but slow. The card asks for "full keyboard navigation". Fix: a modifier shortcut (for example Alt+F, which `navigate` already ignores), documented in the footer hints.

7. **Type-ahead can't match labels containing spaces.** `src/Picker.tsx:45-51`. Space is consumed as activation before the type-ahead branch, so "wait f…" can't be typed. This is minor because prefixes are usually unique enough, but it's worth a README note.

8. **Maintainability.** `src/Picker.tsx` packs most of the UI into a few very long lines (line 68 is about 600 characters, and lines 16-21 hold the whole filter, sort and grouping pipeline). `tsconfig.json`, `vite.config.ts` and `playwright.config.ts` are one-liners. `score()` is recomputed inside the sort comparator, which is fine at 200 items, but memoising it would make the intent clearer. This is acceptable for a spike, but it would be hard to review or extend if it were promoted.

Not findings (checked and OK): the scope is limited to `spikes/S-5-bakeoff/codex/b3/**`. The dev server binds `127.0.0.1` with `strictPort`. The README lists dependency licenses (all MIT or Apache-2.0). The spike-local `pnpm-workspace.yaml` and lockfile keep it out of the root single-writer lockfile, and esbuild builds are explicitly declined. Storage reads validate IDs and dedupe. Roving tabindex stays at exactly one tab stop when filtering hides the active leaf. Nested `tree`/`group`/`treeitem` roles use `aria-expanded` and `aria-selected`. The fixture is deterministic (`scripts/fixture.mjs`, 200 unique IDs). No React plugin is configured for Vite, but esbuild handles the `react-jsx` transform, so the build works, just without Fast Refresh.

## Quality 1-5

**4** — Every Done-when check is green on a re-run. The UI is polished, and the tests are thoughtful about edge cases (blocked storage, roving tabindex). It falls short of a 5 because of a real keyboard and accessibility focus-loss bug in the favourites flow, a fuzzy search that over-matches badly at this data size, and very dense code.
