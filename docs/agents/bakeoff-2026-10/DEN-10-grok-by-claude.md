# DEN-10 b3 — grok, reviewed by claude

Reviewed: `spikes/S-5-bakeoff/grok/b3/**` on `agent/grok/DEN-10-b3` @ `7a02907` (`git diff origin/main...HEAD`: 25 files, +4962, all inside the card's scope; root lockfile and single-writer files untouched). Reviewer: Claude (claude-opus-5-5, agent Lea), 2026-10-19. I edited nothing in the repo. Probes ran from `/tmp` against a throwaway Vite dev server.

Done-when: PASS. I ran each command in `spikes/S-5-bakeoff/grok/b3`, using the existing `node_modules`:
- `pnpm typecheck` → `tsc --noEmit -p tsconfig.json && tsc --noEmit -p tsconfig.e2e.json`, exit 0
- `pnpm test` → `Test Files 2 passed (2)`, `Tests 17 passed (17)`, exit 0. The tests cover search, keyboard navigation and selection, as the card asks.
- `pnpm test:e2e` → `1 passed (1.1s)`, exit 0. Screenshot baselines are in `e2e/__screenshots__/picker-{default,search,recent}-linux.png`.
- Extra check: copying `scripts/gen-fixture.mjs` to `/tmp` and running it gives a byte-identical `subactions.json` with 201 items, so the fixture is deterministic.

The checks are green. But the default screenshot baseline shows a broken render, and the test accepts it (see P1-1). So a passing screenshot test here does not mean the picker renders correctly.

## Findings

### P0
None.

### P1-1 — The dialog is not held in the viewport. One search letter pushes it fully off-screen.
- **Where:** `src/SubActionPicker.module.css:1-9` (`.backdrop`: `display: grid; place-items: center`) and `:14` (`.panel`: `height: min(560px, 100%)`).
- **Failure scenario:** The grid's implicit row is sized from the panel's content, meaning every tree row. The panel's `100%` height doesn't limit that row, so the 560 px panel is centred inside a row that can be thousands of pixels tall. I measured this at 960×720, the Playwright config's own viewport:
  - Default open: the dialog spans y = 326–888, so its bottom 168 px (lower tree and the footer description) is below the fold.
  - Query `send announcement` (6 rows): y = 79–641, which is correct.
  - Query `e` (241 rows): y = 3686–4248. The whole dialog, search box included, leaves the viewport after one keystroke.

  Expanding branches by hand has the same effect. The committed `picker-default-linux.png` shows this: its bottom ~40% is blank white because the element was partly outside the viewport when captured. The baseline encodes the bug, so the test stays green.
- **Suggested fix:** Bound the grid track, for example `.backdrop { grid-template-rows: minmax(0, 1fr); }` with `.panel { height: min(560px, 100%); min-height: 0; }`. Another option is `.panel { max-height: calc(100vh - 48px) }` with the tree as the only scroller. Then regenerate the baselines and add an assertion that the dialog's bounding box lies inside the viewport (or one screenshot after a broad query like `e`).

### P2-1 — Alt+F does nothing on macOS.
- **Where:** `src/SubActionPicker.tsx:120` (`event.altKey && event.key.toLowerCase() === "f"`).
- **Failure scenario:** On macOS, Option+F produces `event.key === "ƒ"`, so the only keyboard way to favourite an item never fires. Favouriting then needs the mouse (the star is `aria-hidden` and mouse-only). This is a cross-platform behaviour gap, and macOS is tier 2 per D-035.
- **Suggested fix:** Match on `event.code === "KeyF"` together with `altKey`.

### P2-2 — Home and End are taken from the search text box.
- **Where:** `src/SubActionPicker.tsx:153-154`. `TREE_KEYS` contains `Home`/`End`, and only ArrowLeft, ArrowRight and Space are exempt while `inSearch`.
- **Failure scenario:** A user types `send anouncement`, presses Home to fix the start of the query, and focus jumps to the first tree row instead. Shift+Home and End are swallowed too. Editing the query by keyboard is broken.
- **Suggested fix:** Add `Home` and `End` to the `inSearch` early return. They should navigate only when the tree has focus.

### P2-3 — Tab focuses the tree container, not the active treeitem.
- **Where:** `src/SubActionPicker.tsx:141` (the focus-trap list pushes `treeRef`) and `:221` (the `role="tree"` div has `tabIndex={-1}` and no `aria-activedescendant`).
- **Failure scenario:** Search `delay`, then press Tab. DOM focus lands on the bare `tree` element. A screen reader announces "Sub-actions, tree" but not the selected "Delay" row, even though that row already has `tabIndex=0`. The roving tabindex pattern is half-applied, and it is mixed with `aria-activedescendant` on the search box (`:215`). The arrow-key path (`:108-112`) does focus the row, so only Tab is affected. ARIA tree semantics are an explicit card requirement.
- **Suggested fix:** In the Tab handler, focus `document.getElementById(focusId)` (or the first row) instead of `treeRef`. Alternatively, drop roving focus and keep focus in the search box with `aria-activedescendant` throughout. Pick one pattern.

### P2-4 — The fuzzy filter is too loose because it runs over the generated description.
- **Where:** `src/model.ts:76-84` (`matchItem` also scores `item.description`), together with `scripts/gen-fixture.mjs`, where every description is `"<name> in <top> / <sub>."`.
- **Failure scenario:** The match is an unbounded subsequence over a string that contains the name and the full path. Counts against the 201-item fixture: `obs` → 54 matches (including all of Core/Globals), `ae` → 118, `st` → 123, `ban` → 27 (including "Break" and "Enable Action"), `raid` → 13 (the first are Core/Commands "Remove Command" and Twitch/Moderation "Clear Chat Messages"). The tree is grouped by category, not score, so noise rows from Core appear above the real hits. Enter still picks the best score, but the list hardly narrows.
- **Suggested fix:** Match only name and path, or require description hits to be contiguous or word-prefix. Also drop leaves below a score threshold relative to the best match.

### P3-1 — ArrowDown from the search box skips the highlighted match.
- **Where:** `src/model.ts:266-268` with `src/SubActionPicker.tsx:108-112`.
- **Failure scenario:** Type `announcement`. The best match is highlighted. Pressing ArrowDown moves to the row after it, so the user has to press ArrowUp to reach the row they were shown.
- **Suggested fix:** On the first ArrowDown out of the search box, focus the current `focusId` row instead of `index + 1`.

### P3-2 — The page is missing a base reset.
- **Where:** `index.html` and `src/App.module.css:1-9`.
- **Failure scenario:** The default 8 px body margin plus `.page { min-height: 100vh; padding: 32px 24px }` gives a document 800 px tall in a 720 px viewport. The page scrolls, and a light frame shows around the dark theme.
- **Suggested fix:** Add `body { margin: 0 }` and `box-sizing: border-box` (or `min-height: 100dvh` with a padding calc).

### P3-3 — DOM ids are hard-coded and global.
- **Where:** `src/SubActionPicker.tsx:221` (`id="subaction-tree"`) and the row ids built in `model.ts`.
- **Failure scenario:** Two pickers mounted at once (for example, a future nested editor) would produce duplicate ids. `getElementById` focus calls and `aria-activedescendant` would then target the wrong instance.
- **Suggested fix:** Prefix ids with `useId()`. This is acceptable for a spike, but note it for the product version.

### P3-4 — The live footer is noisy.
- **Where:** `src/SubActionPicker.tsx:264` (footer has `aria-live="polite"`).
- **Failure scenario:** Every arrow press announces the row's accessible name and then the footer description, which repeats the name and path ("Delay in Core / Logic.").
- **Suggested fix:** Link the description with `aria-describedby` on the focused row, or remove `aria-live`.

## Strengths

- The scope is clean, and the keyboard model is pure and unit-tested (`onTreeKey`, `typeAhead`, `neighbourId`).
- Focus is kept when a favourite is removed. Escape clears the query first, then closes. Focus returns to the trigger. Enter on an empty query adds nothing.
- The duplicate "Set Current Scene" items stay distinguishable. The fixture is deterministic and clean-room.

## Quality 1-5

**3** — The code is tidy and well-tested, and every Done-when passes. But the dialog leaves the viewport on ordinary searches, and the screenshot baseline locks in that broken render. A real gate would have caught both.
