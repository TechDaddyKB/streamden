# DEN-11 · Codex B-4 CI spike

A tiny standalone Cargo + pnpm workspace, with an **inactive** GitHub Actions
workflow in `ci.yml`. Nothing under the repository's `.github/` is changed.

To activate later, copy `ci.yml` to `.github/workflows/b4.yml` without moving the
fixture. It supports PRs, pushes to main, manual dispatch, and `workflow_call`.
A caller can use `jobs.checks.uses: ./.github/workflows/b4.yml`. When adapting to
the real monorepo, change the working directory and all cache/lockfile paths;
replace the sample generator with the approved protocol generation command.
Do not copy it into active CI as part of this spike.

## Jobs and dry run

- `fmt`: Ubuntu 22.04 checks out the repository without persisted credentials,
  installs stable Rust with rustfmt, and checks the entire Cargo workspace.
- `checks`: five independent native environments: Ubuntu 22.04, Fedora 44 and
  Arch containers on Ubuntu, Windows latest, and macOS latest. Container root
  installs Git, CA certificates, compilers and Tauri libraries **before checkout**;
  native Ubuntu uses apt afterward. Windows/macOS rely on hosted native build
  tools. Every entry restores a distro/architecture/lockfile-specific Rust cache,
  runs clippy with warnings denied, installs pinned nextest, runs tests and
  doctests, then installs pinned pnpm and Node 22, restores the pnpm store,
  performs a frozen install and runs recursive typecheck, lint and test scripts.
  Explicit Bash handles Windows Git Bash and container shell defaults. No GUI
  launches or WebView runtime validation occur. A failed entry leaves its siblings
  running (`fail-fast: false`).
- `codegen`: Ubuntu checks out the repo, installs Node 22 and regenerates
  `generated/version.json` deterministically from package metadata. The placeholder
  needs no package installation. `git diff --exit-code -- generated/` catches
  changed/deleted tracked output; an additional untracked-file guard catches new
  output. Real schema generation remains DEN-14/DEN-19 work.

The fixture intentionally has no Tauri crate. Dependency installation prepares
Linux build environments but does **not** prove a real Tauri app compiles or runs.
Its frontend uses TypeScript checking of JSDoc-typed JavaScript, Node syntax lint
and Node's test runner, avoiding an unrelated framework setup.

## Local verification

From this directory (Rust stable, Node >=22, pnpm 10.11.0, nextest 0.9.114):

```sh
actionlint ci.yml
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo nextest run --workspace --all-targets --all-features --locked
cargo test --workspace --doc --locked
pnpm install --frozen-lockfile --ignore-scripts
pnpm typecheck
pnpm lint
pnpm test
pnpm codegen
git diff --exit-code -- generated/
test -z "$(git ls-files --others --exclude-standard -- generated/)"
```

Check generated output only after the fixture is tracked. A deliberate change to
`generated/version.json` must fail the drift check; rerunning codegen restores it.

## Operational choices

Read-only `contents` permission, no secrets, no publish/deploy steps. The concurrency
key includes workflow and PR/ref so newer runs cancel superseded runs on that ref.
Action implementations and container images are pinned by SHA/digest. Rust stable,
hosted runner labels and distro package repositories deliberately track updates;
this is a compatibility check rather than a hermetic build. Refresh image digests
and action pins through a reviewed maintenance change. Arch performs a full
upgrade (`-Syu`) to avoid unsupported partial upgrades.

Rust cache keys separate Fedora/Arch/Ubuntu, include architecture and Cargo.lock,
and use a per-commit suffix with a lockfile-specific restore prefix. Rust binaries
in `~/.cargo/bin` are not cached over toolchain installation. pnpm caches its store,
not `node_modules`; every matrix entry uses the frozen lockfile.

New fixture dependency: TypeScript 5.9.3 (Apache-2.0). CI tools: pnpm 10.11.0 (MIT),
cargo-nextest 0.9.114 (MIT OR Apache-2.0), actionlint 1.7.12 (MIT, local validator).
Cargo has no third-party fixture dependencies.

Sources (checked 2026-10-19):
[Tauri prerequisites](https://v2.tauri.app/start/prerequisites/),
[GitHub reusable workflows](https://docs.github.com/en/actions/how-tos/reuse-automations/reuse-workflows),
[nextest installation](https://nexte.st/docs/installation/from-source/).
