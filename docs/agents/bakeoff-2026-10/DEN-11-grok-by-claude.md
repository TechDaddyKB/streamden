# DEN-11 b4 — grok, reviewed by claude

Reviewer: claude / claude-opus-5-5 (Friedegard, agent:c97a2012-d5a1-4279-806b-433dd14f405f), 2026-10-19.
Target: `agent/grok/DEN-11-b4` @ `b658c518ccf003ec035697c0fb6c58b21a3c7dca`, `git diff origin/main...HEAD`. The diff is 16 files, 527 lines, all under `spikes/S-5-bakeoff/grok/b4/**`, so it stays inside the card's scope. Read-only review: I edited no repo files and did not touch the card. I ran the local checks on a `git archive` copy in `/tmp/den11-review-grok`.

Done-when: PASS

- `actionlint` 1.7.12 (`/tmp/den11-tools/actionlint`, sha256 `c872d6db…4cd4`, the same binary the Claude handoff verified against upstream checksums) run on `spikes/S-5-bakeoff/grok/b4/ci.yml`: no output, exit 0. Shellcheck is not installed, so actionlint's shellcheck pass did not run.
- The handoff has a written dry run for every job (`fmt`, `verify` ×5, `codegen`) plus the workflow-wide settings. It is on card DEN-11, in the "Handoff — 2026-10-19 — grok/grok-4.7" section.
- Extra local evidence (Linux only; the hosted matrix never ran):

  | Check | Result |
  | --- | --- |
  | `cargo fmt --check` | 0 |
  | `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | 0 |
  | nextest 0.9.114 | 2/2 passed |
  | `cargo test --doc` | 1 passed |
  | `pnpm install --frozen-lockfile --ignore-scripts` (pnpm 12.14.0) | 0 |
  | `pnpm typecheck` | 0 |
  | `pnpm lint` | 0 |
  | `pnpm test` | 2 pass, 0 fail |
  | codegen + `git diff --exit-code` | 0 |
  | untracked guard | 0 |

- Negative controls:

  | Control | Result |
  | --- | --- |
  | Version bump, then regenerate | rejected (1) |
  | Untracked `generated/x.json` | rejected (1) |
  | JSDoc type error in `src/` | typecheck failed (1) |
  | `var y = 1 == "1"` plus a new `test/extra.test.js` | **lint passed (0)**, see P2-2 |

- Commit hygiene: both commits use a Conventional subject naming DEN-11, have a body that lists the checks run, and end with a single `AI-Assisted-By: Grok` trailer. That matches AGENTS.md and D-037.

## Findings

### P0
None.

### P1
None. I found nothing that would fail the workflow on first activation in a way I could confirm statically.

### P2

**P2-1: "Reusable" only in name. `workflow_call` has no inputs and every path is hard-coded.** `spikes/S-5-bakeoff/grok/b4/ci.yml:10`, `:22`, `:107`, `:120`, `:125`.
- Failure scenario: a caller in another repo, or this repo after DEN-19 moves the workspace to the root, runs `uses: ./.github/workflows/ci.yml`. `defaults.run.working-directory: spikes/S-5-bakeoff/grok/b4` does not exist there. Every `run` step fails with "No such file or directory", and `package_json_file` and `cache-dependency-path` point at missing files. The card's Goal asks for a *reusable* workflow. As written, reusing it means editing five places (the README admits this).
- Fix: add `workflow_call.inputs.working-directory` (default `.`). Drive `defaults.run.working-directory`, the rust-cache `workspaces`, `package_json_file` and `cache-dependency-path` from `inputs.working-directory || 'spikes/S-5-bakeoff/grok/b4'`, so push and PR triggers keep the spike path.

**P2-2: `pnpm lint` is syntax-only and covers only a hard-coded file list.** `spikes/S-5-bakeoff/grok/b4/web/probe/package.json:7`.
- Failure scenario: `node --check src/index.js && node --check test/index.test.js` passes `var y = 1 == "1"` (verified, exit 0). It also never looks at a newly added `test/extra.test.js` or `src/other.js`. The "lint" leg of the matrix gives a green signal that holds no lint policy, and new files are silently out of scope.
- Fix: use a real linter with a directory glob, for example `biome ci .` or `eslint .`, so a style or bug-pattern violation fails. At minimum, `node --check` every file found by a glob.

**P2-3: The pinned Arch image plus a bare `pacman -Syu` will rot into keyring and signature failures.** `spikes/S-5-bakeoff/grok/b4/ci.yml:59`, `:83`.
- Failure scenario: `archlinux:base@sha256:3c64…` is frozen at 2026-10-19 while the mirrors keep moving. After a packager key rotation, `pacman -Syu` in the stale image fails with "signature … is unknown trust" or "invalid or corrupted package (PGP signature)" before checkout. The Arch cell turns red for reasons unrelated to the code. Fedora has a milder version of the same drift, but dnf handles it.
- Fix: refresh the keyring first: `pacman -Sy --noconfirm --needed archlinux-keyring && pacman -Su --noconfirm --needed …`. Alternatively, document a scheduled digest-bump task. (Plausible; not reproduced, because Docker was unavailable to both author and reviewer.)

### P3

**P3-1: `cancel-in-progress: true` also cancels runs on pushes to `main`.** `ci.yml:17`.
- Failure scenario: two quick merges to main. The first run is cancelled, so that commit never gets a CI result, and bisecting a later red main gets harder.
- Fix: `cancel-in-progress: ${{ github.ref != 'refs/heads/main' }}`.

**P3-2: rust-cache saves on every PR run.** `ci.yml:105-110`.
- Failure scenario: 5 matrix cells × N PR branches each save a target cache. That pressures the 10 GB repo cache limit and evicts main's caches, so main builds go cold.
- Fix: `save-if: ${{ github.ref == 'refs/heads/main' }}`.

**P3-3: The typecheck skips tests.** `web/probe/tsconfig.json:12`.
- Failure scenario: `include: ["src"]` means `test/**` is never type-checked, so a mistyped call in a test passes `pnpm typecheck`.
- Fix: include `test`, adding `types: ["node"]` and `@types/node`, or use a separate test tsconfig.

**P3-4: The codegen placeholder doesn't model the real Rust→TS flow, and its untracked guard fails silently.** `scripts/codegen.mjs:1-16`, `ci.yml:141-145`.
- Failure scenario: the stamp comes from `package.json`, not from a Rust crate. The pattern DEN-19 needs (cargo-driven generator, Rust toolchain in the drift job) is therefore not exercised. When `test -z "$(git ls-files --others …)"` fails, it exits 1 with no output, so the author doesn't know which file is untracked.
- Fix: optionally drive the stamp from `crates/probe` (for example `cargo metadata`). Print the offending files and an `::error::` annotation with the fix command before exiting 1.

**P3-5: Matrix `os` doubles as a display label and a branch key (`'ubuntu-22.04'`).** `ci.yml:51`, `:93`.
- Failure scenario: someone bumps the runner to 24.04 and renames `os`. The `if: matrix.os == 'ubuntu-22.04'` guard silently stops installing apt deps on Ubuntu.
- Fix: use a stable key (`os: ubuntu`) and keep the version only in `runner`.

## Notes (no action)
- Good: the container prerequisites override `working-directory: /`, so they don't `cd` into a not-yet-checked-out path. Checkout sets `persist-credentials: false`. Every action is pinned by SHA, the images are pinned by digest, `cache-bin: "false"` keeps the cache from clobbering the toolchain, pnpm runs on all five cells (as the card asks), and the dry run is concise and accurate.
- Not verified by anyone: the hosted 5-OS execution and the container package installs. The fixture does not link WebKitGTK, so a wrong Tauri package name would only show up at install time.

## Quality 1-5

**4.** The workflow is tight, correctly pinned and least-privilege, meets every Done-when item, and its handoff is honest. It loses a point because the "reusable" goal is unmet (no `workflow_call` inputs, hard-coded paths) and the lint leg is a placeholder that passes obvious violations.
