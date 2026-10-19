# DEN-11 · Grok B-4 CI spike

Inactive GitHub Actions workflow plus a tiny Cargo and pnpm workspace.
Nothing under the repository `.github/` directory is changed. GitHub ignores
`ci.yml` until it is copied to `.github/workflows/`.

After that copy, the file runs on pull requests, pushes to `main`, manual
dispatch, and `workflow_call`. A caller uses
`jobs.<id>.uses: ./.github/workflows/<copied-name>.yml`.
When adapting it to the real monorepo, change `working-directory`, the
rust-cache workspace, and the pnpm manifest and lockfile paths. Replace
`scripts/codegen.mjs` with the approved protocol generator. Do not activate
this file as part of the spike.

## What each job does

- `fmt` on `ubuntu-22.04` checks `cargo fmt --all -- --check`.
- `verify` runs five times with `fail-fast: false`: native Ubuntu 22.04,
  Fedora 44 and Arch containers on Ubuntu 22.04, `windows-latest`, and
  `macos-latest`. Fedora and Arch install git and the distro WebKitGTK 4.1
  Tauri packages before checkout. Ubuntu installs the apt equivalents after
  checkout. Windows and macOS use the hosted toolchain image and do not
  install a WebView. Each cell then runs locked clippy (`-D warnings`),
  pinned nextest 0.9.148, doctests, a frozen pnpm install, and
  typecheck, lint, and test.
- `codegen` on `ubuntu-22.04` regenerates `generated/stamp.json` with Node
  and rejects both tracked drift and untracked files under `generated/`.

The fixture has no Tauri crate. Linux package steps only provision headers.
They do not prove a WebView builds or runs.

## Local checks

From this directory, with Rust stable, Node 22 or newer, and pnpm 12.14.0:

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

## Pins and permissions

Workflow permissions are `contents: read`. There are no secrets and no
publish or deploy steps. Concurrency groups by workflow and pull request or
ref, and cancels a superseded run. Action SHAs and the Fedora and Arch image
digests are pinned. Rust stable, the hosted runner labels, Node 22, and the
distro repositories float so the matrix keeps covering current toolchains.
Refresh pins in a reviewed change.

Rust caches are split by OS name and runner architecture and do not save
`~/.cargo/bin`. pnpm caches its store from `pnpm-lock.yaml`.
