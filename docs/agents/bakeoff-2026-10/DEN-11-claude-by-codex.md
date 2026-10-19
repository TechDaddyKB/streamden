# DEN-11 b4 — claude, reviewed by codex

Done-when: PASS — independently ran `/tmp/den11-tools/actionlint -no-color spikes/S-5-bakeoff/claude/b4/.github/workflows/ci.yml` (actionlint 1.7.12): exit 0, no output. DEN-11's Claude handoff contains written dry-run explanations for all four jobs: fmt, rust, web, and codegen-drift. This passes the card's explicit Done-when; the Goal finding below remains.

## Findings

### P2 — Enable superseded-run cancellation on main

- File: `spikes/S-5-bakeoff/claude/b4/.github/workflows/ci.yml:29` (related comments at lines 24–26).
- Failure scenario: commit A is running on main when commits B and C arrive. `cancel-in-progress` evaluates to false, so obsolete A continues consuming the five-platform matrix instead of being canceled as required by the Goal. B can also be canceled while pending when C replaces it; this configuration does not deliver the documented promise that every main commit gets a result. GitHub's default concurrency queue retains only one pending run. See [GitHub concurrency documentation](https://docs.github.com/en/actions/how-tos/write-workflows/choose-when-workflows-run/control-workflow-concurrency).
- Suggested fix: use `cancel-in-progress: true` for this CI workflow and update the comments/dry-run explanation. If preserving every main run is desired instead, obtain an explicit requirement change and configure an appropriate queue rather than claiming the current setting guarantees it.

No P0, P1, or P3 findings.

## Verification and limits

Reviewed branch `agent/claude/DEN-11-b4`, HEAD `5542353018da32b4de40794aec5e40c1942fb176`, using `git diff origin/main...HEAD` after `git fetch --prune`; base `adff917ef4563c955b60ac5923fe8edda7a5e800`. All 20 changed files are inside the requested Claude b4 scope.

Additional checks ran on a temporary copy at `/tmp/den11-claude-review.Hncvoq`, preserving the original checkout:

| Check | Result |
| --- | --- |
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | PASS; no warnings |
| `/tmp/den11-tools/bin/cargo-nextest nextest run --workspace --all-features --locked --no-tests=warn` | PASS; 2/2 tests |
| `cargo test --workspace --doc --locked` | PASS; 0 doctests |
| `pnpm install --frozen-lockfile --ignore-scripts` | PASS; pnpm 12.14.0 |
| `pnpm typecheck` | PASS |
| `pnpm lint` | PASS; 5 files checked |
| `pnpm test` | PASS; 1/1 test |
| `cargo run --locked -p b4-codegen`, then `cmp` against committed generated output | PASS; identical output |

Local frontend checks used Node 26.7.0, whereas CI selects Node 22. No GitHub run or Windows/macOS execution was performed. Container execution was unavailable: `docker info` returned permission denied for `/var/run/docker.sock`; no privilege escalation attempted. Shellcheck is absent, so actionlint's optional shellcheck integration was unavailable. These limits do not change the explicit Done-when result.

Assumptions: assessed the inactive workflow as a reusable spike, with future activation governed by its README. Accepted the documented split between the five-platform Rust matrix and the three-host frontend matrix; the card does not explicitly require every job on every distro. No product question was needed. Repository files, index, branch, and card were left unchanged; final tracked and staged diffs are empty. Required fleet-reflect lookup returned `status: missing`; no actionable fleet finding beyond this review.

## Quality 1-5

4/5 — Clear, reusable workflow with pinned actions/images, minimal permissions, distro-specific dependencies, meaningful fixture checks and thorough dry runs; main-branch cancellation misses an explicit Goal requirement, and hosted platform execution remains unproven.
