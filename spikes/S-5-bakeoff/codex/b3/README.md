# DEN-10 · Codex B-3

Standalone Vite + React 19 + TypeScript spike. All 200 actions are generated demonstration
fixtures; none calls a real platform. Plain CSS modules, no component or fuzzy-search library.

```sh
cd spikes/S-5-bakeoff/codex/b3
pnpm install --frozen-lockfile
pnpm fixture                 # regenerate src/actions.json deterministically
pnpm typecheck
pnpm test                   # Vitest + Testing Library
pnpm exec playwright install --only-shell chromium
pnpm screenshot             # compare the committed Linux screenshot and test real keyboard use
pnpm build
pnpm dev                    # http://127.0.0.1:5173
```

Uses pnpm 12.14.0 and Node >=22.12. esbuild install scripts are explicitly declined;
its platform-specific optional binary package handles builds. No root workspace changes.

- Search uses case-insensitive ordered subsequences across action names and categories;
  tighter matches sort first within categories. Searching expands all matching ancestors.
- Arrow Down in search enters the tree. Up/Down traverse visible rows, Right expands or
  enters a category, Left collapses or visits the parent, Home/End jump to visible edges.
  Type-ahead finds the next visible prefix (700ms buffer, repeated letters cycle).
- Enter/Space on a category toggles it; Enter/Space on an action adds it. A category stays
  expanded while searching. Click selects a leaf for preview; double-click or Add action adds it.
- Escape or the close button dismisses the picker and returns focus to Add sub-action.
  This is an inline picker, not a modal: Tab reaches search, library views, tree, favourite
  toggle and Add action. Treeitems use roving tab stops and nested ARIA tree/group roles.
- Favourites and the last ten unique selections persist in browser localStorage under
  `b3-favourites` and `b3-recent`. Recent actions are grouped by category, newest first
  inside each group. Missing/malformed storage is ignored; blocked storage falls back to memory.
- Only Core starts expanded. Favourites and Recent use the same category expansion state.
  Empty views show a message. The layout stacks at widths <=650px.

Screenshot baseline: `e2e/picker.spec.ts-snapshots/picker-linux.png` (1280px wide, full page, dark).
Inspect intended visual changes before regenerating with `pnpm screenshot --update-snapshots`.
The baseline depends on Linux system fonts; other OSes need separately reviewed baselines.
On this host, Playwright 1.55.0 extraction stalled under Node 26.7.0; running the installer
with `mise exec node@24.21.0 -- pnpm exec playwright install --only-shell chromium`
completed successfully. All checks then passed under Node 26.7.0.
Chromium runs headless; this does not validate Tauri/WebKit or the DEN-20 platform gate.

Direct dependencies: React, React DOM, Vite, Vitest, Testing Library React/user-event/jest-dom,
React/DOM type declarations (MIT); TypeScript and Playwright (Apache-2.0); jsdom (MIT).
App code is GPL-3.0-or-later. No production workspace, public contracts or services changed.
