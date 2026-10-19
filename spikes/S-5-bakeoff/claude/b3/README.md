# S-5 bake-off B-3 (Claude): sub-action picker

Throwaway spike for DEN-10. A Streamer.bot-style "Add sub-action" picker in Vite + React 19 + TypeScript with
plain CSS modules and a dark theme. Not production code; the real UI lands under `apps/` after DEN-19.

## Run

```sh
pnpm install
pnpm gen:fixture   # regenerates src/fixture/subactions.json (199 items, committed)
pnpm dev           # http://127.0.0.1:5183
pnpm typecheck
pnpm test          # Vitest + Testing Library
pnpm exec playwright install --only-shell chromium   # once per Playwright version
pnpm test:e2e      # Playwright screenshot tests against `vite preview`
```

Screenshot baselines: `e2e/__screenshots__/picker-{default,search,favourites}-linux.png` (Chromium headless shell,
Linux). Update them with `pnpm test:e2e --update-snapshots`.

## Behaviour

- Tree: category › group › sub-action, plus **Favourites** and **Recently used** sections when not searching.
  Both persist in `localStorage` (`streamden.subActionPicker.v1`); recents keep the last 5.
- Search: whitespace-separated terms, each fuzzy-matched (subsequence, rewards word starts and runs) against
  name, keywords, group and category. Matches are expanded, ranked by score, and highlighted. A `role=status`
  line announces the count.
- Keyboard (WAI-ARIA APG tree pattern, roving `tabindex`):

  | Key | In search box | In tree |
  |---|---|---|
  | ↓ | Focus the current/best match | Next row |
  | ↑ | — | Previous row; from the first row, back to search |
  | → / ← | — | Expand / go to first child · collapse / go to parent |
  | Home / End | — | First / last row |
  | Enter | Add the best match | Add leaf · toggle branch (Space too) |
  | Alt+F | — | Toggle favourite |
  | printable | Filters | Type-ahead (500 ms buffer; repeating a letter cycles) |
  | / | — | Focus search |
  | Esc | Clear query, else close | Same |

- ARIA: `role=tree` labelled by the heading; flat `treeitem`s with `aria-level`, `aria-setsize`,
  `aria-posinset`, `aria-expanded` (branches) and `aria-selected` (leaves; selection follows focus).
  The favourite star is mouse-only and `aria-hidden`; favourite state is exposed as visually hidden text.
