# DEN-11 b4 — codex, reviewed by claude

Reviewer: Claude (claude-opus-5-5, agent Gabriele), 2026-10-19. Target: `spikes/S-5-bakeoff/codex/b4/**` on
`agent/codex/DEN-11-b4` @ `a31801b` (`git diff origin/main...HEAD`: 15 files, +320, all inside scope).
I edited nothing in the repo. I ran the pnpm and codegen checks on a `git archive` copy in `/tmp/den11-review-b4`,
and Cargo used `CARGO_TARGET_DIR=/tmp/den11-review-target`.

Done-when: PASS. Evidence:

```text
actionlint 1.7.12 (/tmp/den11-tools/actionlint, sha256 c872d6db…5f4cd4, which matches the checksum-verified binary)
actionlint ci.yml                                          -> no output, exit 0
cargo fmt --all -- --check                                 -> exit 0
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings -> Finished, exit 0
cargo nextest run --workspace --all-targets --all-features --locked -> 1 passed, exit 0
cargo test --workspace --doc --locked                      -> ok, exit 0
pnpm install --frozen-lockfile --ignore-scripts (pnpm 10.11.0) -> Done, exit 0
pnpm typecheck / pnpm lint / pnpm test                     -> exit 0 / 0 / 0 (tests 1, pass 1)
codegen + git diff --exit-code + untracked guard           -> exit 0
negative: type error in src/index.js -> typecheck exit 2; version bump -> drift detected (exit 1)
action pins (git ls-remote): checkout 11d5960=v4.4.0, setup-node 49933ea=v4.4.0, cache 0057852=v4.3.0,
  pnpm/action-setup b906aff=v4.3.0, dtolnay/rust-toolchain b725eda=refs/heads/stable. All real.
```

The written per-job dry run is in the card handoff (fmt, checks, codegen, workflow-wide). Limits: shellcheck isn't
installed, so actionlint's shellcheck pass didn't run. Docker is present but I have no permission to use it, so I
couldn't dry-run the Fedora and Arch package installs. The workflow has never run on GitHub.

## Findings

### P0
None.

### P1
None. I suspected the Arch package `libappindicator-gtk3` was missing, but it resolves (see P3-1).

### P2

**P2-1: "Reusable" workflow is hard-wired to the spike path.** `ci.yml:7`, `ci.yml:18`, `ci.yml:98-101`, `ci.yml:111`, `ci.yml:117`
- Failure scenario: `workflow_call` has no `inputs`. `defaults.run.working-directory`, the cache paths, the
  `hashFiles` globs, `package_json_file` and `cache-dependency-path` all hardcode `spikes/S-5-bakeoff/codex/b4`.
  Any caller whose code isn't at that path, including the real monorepo root after DEN-19, gets `cd` failures or
  empty cache keys, and has to edit the file. The README (line 9) admits this. The card's Goal asks for a reusable
  workflow.
- Suggested fix: add `on.workflow_call.inputs.working-directory` (string, default `.`), and use
  `${{ inputs.working-directory || 'spikes/S-5-bakeoff/codex/b4' }}` everywhere the path appears. `defaults.run`
  can't take `inputs`, so set it per job, or use a job-level `env`.

**P2-2: nextest is compiled from source in every entry on every run, and never cached.** `ci.yml:103-104`
- Failure scenario: `cargo install cargo-nextest --version 0.9.114 --locked` builds about 300 crates on all 5
  matrix entries for every push. That is several minutes per entry, slowest on Windows. `~/.cargo/bin` isn't in the
  cache paths (`ci.yml:95-98`), which README:72-73 confirms. On a busy PR this burns most of the CI minutes and eats
  into the 40-minute timeout. A crates.io hiccup also fails every entry.
- Suggested fix: use a SHA-pinned `taiki-e/install-action` with `tool: cargo-nextest@0.9.114`, which fetches a
  prebuilt, checksum-verified binary. Alternatively, cache `~/.cargo/bin/cargo-nextest*` keyed on the version.

### P3

**P3-1: Arch package name is deprecated.** `ci.yml:76`
- Failure scenario: Arch renamed `libappindicator-gtk3` to `libappindicator`. Today `pacman -S` still resolves the
  old name through `Provides/Replaces` (verified with `pacman -Si` / `pacman -Sp` on this Arch host). If that
  provides entry is ever dropped, the Arch entry fails at install time.
- Suggested fix: use `libappindicator`.

**P3-2: Cache key saves a new entry on every commit.** `ci.yml:99`
- Failure scenario: `-${{ github.sha }}` writes a fresh entry, including `target/`, for each of 4 Linux/Win/mac
  keys on every push. Under the repo's 10 GB cache cap this evicts the useful main-branch caches quickly once real
  crates exist.
- Suggested fix: drop the sha suffix and save only on `main`, using `actions/cache/restore` + `save` with
  `if: github.ref == 'refs/heads/main'`, or use `Swatinem/rust-cache` with `save-if`.

**P3-3: Superseded-run cancellation also cancels pushes to main.** `ci.yml:13`
- Failure scenario: two quick merges to main. The first run is cancelled, so that commit never gets a green or red
  result, which makes bisecting and later required-status checks harder.
- Suggested fix: `cancel-in-progress: ${{ github.ref != 'refs/heads/main' }}`.

**P3-4: Lint and typecheck are thin placeholders.** `web/sample/package.json:8-9`
- Failure scenario: `node --check` is a syntax check, not a linter, so style and correctness rules never fire.
  `tsc` checks only `src/index.js`, so a type error in `test/` passes.
- Suggested fix: acceptable for a spike. Before DEN-19, swap in a real linter (biome or eslint) and a `tsconfig`
  that includes the tests.

**P3-5: `container: null` on non-container entries hasn't been checked on a real run.** `ci.yml:49`, `ci.yml:58`, `ci.yml:61`
- Failure scenario: actionlint accepts it, but the Windows and macOS entries rely on the runner treating a null
  expression as "no container". The pattern documented in the community is an empty string.
- Suggested fix: use `container: ''`, or confirm with one `workflow_dispatch` run once the workflow is activated.

**P3-6: Version comments and the Node runtime are dated.** `ci.yml:29,93,109,113`
- Failure scenario: the comments say `# v4`, but the pins are v4.4.0 / v4.3.0. These majors run on node20, which
  GitHub has deprecated, so they will need a bump.
- Suggested fix: put exact tags in the comments (`# v4.4.0`), and plan a pin refresh to the current majors.

Assumptions:
- I reviewed behaviour statically for the hosted runners and containers I couldn't run.
- I didn't rate the README's activation guidance as a defect, because the card keeps CI inactive.

## Quality 1-5

**4**: small, correct and well pinned. Every check passes, and the negative controls work (drift, type error). It is
held back by the hardcoded paths, which undercut the "reusable" goal, and by uncached from-source nextest builds on
all five platforms.
