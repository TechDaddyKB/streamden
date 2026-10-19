# Design tokens (web/kit)

Small token set shared by the UI shell, the designer chrome and the plugin UI kit. Dark theme first (streamers run
dark UIs next to OBS); the light theme is a second colour map over the same scale. Prefixed `--sd-`.

## Single source

`web/kit/src/tokens.json` is the only hand-edited file. Everything else derives from it:

| Output | How |
|---|---|
| `web/kit/src/tokens.css` (CSS custom properties) | `pnpm --filter @streamden/kit tokens:css`. `pnpm -r build` fails if it is stale. |
| `tokens`, `vars` in `@streamden/kit` (typed TS) | `tokens` is the JSON itself (typed through `resolveJsonModule`); `vars.color.bg` is `"var(--sd-color-bg)"`. |
| Contrast report | `pnpm --filter @streamden/kit contrast`; also run by `pnpm -r build`, so a failing colour fails the build. |

Use it: `import "@streamden/kit/tokens.css"` once, then `var(--sd-…)` in CSS. Set `data-theme="light"` on a
root element for the light theme (default and `data-theme="dark"` are dark).

## Tokens

- **Colour roles** (`--sd-color-*`): `bg`, `surface`, `surface-raised`, `border`, `border-strong`, `control` (3:1 edge for buttons and inputs), `text`, `muted`,
  `accent`, `on-accent`, `success`, `warning`, `danger`, `focus`. One accent (amber). The raw neutral ramp is
  `--sd-ramp-n0 … n950`; prefer roles in components.
- **Type**: `--sd-font-ui` (system UI stack), `--sd-font-mono`; sizes `--sd-text-xs … xxl` (12–32 px at the default root size).
- **Spacing**: `--sd-space-1 … 7` (4, 8, 12, 16, 24, 32, 48 px).
- **Radius**: `--sd-radius-sm` 3px, `--sd-radius-md` 6px. No pills.
- **Elevation**: borders first; `--sd-shadow-1` and `-2` for popovers and dialogs (theme-aware).
- **Motion**: `--sd-motion-fast/base/slow` (100/180/300 ms), `--sd-motion-easing`. All durations become `0ms` under
  `prefers-reduced-motion: reduce`, so use the tokens for every transition.
- **Focus**: `--sd-focus-ring` (`2px solid` focus colour) with `--sd-focus-offset`; apply on `:focus-visible`.

## Contrast (WCAG 2.x, `pnpm --filter @streamden/kit contrast`)

Every text role must reach 4.5:1 on the surfaces it is used on; the focus ring 3:1. Borders are decorative and
not checked. Selected ratios:

| Pair | Dark | Light |
|---|---|---|
| `text` on `bg` | 15.69 | 14.64 |
| `muted` on `surface-raised` | 5.97 | 7.20 |
| `accent` on `bg` | 8.40 | 4.70 |
| `success` on `bg` | 8.18 | 4.70 |
| `warning` on `surface` | 10.08 | 5.21 |
| `danger` on `surface` | 6.09 | 5.39 |
| `on-accent` on `accent` | 8.60 | 5.43 |
| `focus` on `surface` | 7.86 | 5.48 |

Full output has every pair. Both themes also set `color-scheme`. Motion tokens live on `:root` only, so the reduced-motion override always wins.
