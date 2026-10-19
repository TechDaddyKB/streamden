# B-3 sub-action picker (Grok)

Spike for DEN-10. A Vite + React 19 dialog that adds a sub-action from a generated catalog of about 200 items.

## Run

```bash
pnpm install
pnpm typecheck
pnpm test
pnpm exec playwright install --only-shell chromium
pnpm test:e2e
```

`pnpm dev` opens the same picker at `http://127.0.0.1:4173`.

Regenerate the committed fixture with `pnpm gen:fixture`. Screenshot baselines are Linux Chromium baselines under `e2e/__screenshots__/`. Refresh them with `pnpm test:e2e -- --update-snapshots`.

## Behaviour

- Category tree (Core, Twitch, YouTube, Kick, OBS, Streamlabs) with ARIA `tree` / `treeitem`.
- Fuzzy search matches a subsequence of the name, the category, or the description. The best catalog match is selected. Enter adds that match. Enter in an empty box only moves onto the tree.
- Arrow keys, Home, and End move. Right expands a branch or steps into it. Left collapses it or jumps to the parent. While a search is active, matches stay expanded.
- Type-ahead (prefix, 700 ms) works when the tree itself has focus. Printable keys in the search box edit the query.
- Alt+F toggles a favourite. Favourites and the last eight selections are kept in `localStorage`. Removing the focused favourite moves focus to the next remaining row.
- Escape clears the query first, then closes and returns focus to Add sub-action.

## Assumptions

These are spike choices, not product decisions. Favourites use Alt+F because letter keys are type-ahead. The star is mouse-only and `aria-hidden`; favourite state is in the treeitem name. Selection follows focus (`aria-selected`). Enter, Space (on the tree), or a click adds a leaf. Two "Set Current Scene" entries stay distinct because every row shows its category.

## Dependencies (spike-local)

React and react-dom 19.3.1 (MIT), Vite 8.3.4 (MIT), `@vitejs/plugin-react` 6.1.2 (MIT), Vitest 5.0.5 (MIT), jsdom 30.1.3 (MIT), Testing Library react 16.3.3 / user-event 14.6.8 / jest-dom 7.0.1 (MIT), TypeScript 7.0.2 (Apache-2.0), `@playwright/test` 1.64.1 (Apache-2.0), `@types/react` and `@types/react-dom` 19.3.0 (MIT).
