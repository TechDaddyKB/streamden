# B-4 bake-off entry (Claude): CI for a Cargo + pnpm monorepo

Throwaway spike for DEN-11 (S-5 bake-off). Not production code; nothing here is wired into the real repo CI.

- `.github/workflows/ci.yml`: the workflow. GitHub ignores it at this path, so it never runs from here.
- `crates/b4-core`: a library with one public constant, standing in for `den-protocol`.
- `crates/b4-codegen`: writes `packages/b4-web/src/generated/event-kind.ts` from `b4-core`, standing in for
  ts-rs / schemars output.
- `packages/b4-web`: a TypeScript package that imports the generated file. It has a typecheck (tsc), a lint
  (Biome) and a test (`node --test` with type stripping).

## Jobs

| Job | Runs on | What it checks |
|---|---|---|
| `fmt` | ubuntu-24.04 | `cargo fmt --check` |
| `rust` | ubuntu-22.04, Fedora 44 container, Arch container, windows-latest, macos-latest | Tauri Linux deps, `clippy -D warnings`, `cargo nextest`, doctests |
| `web` | ubuntu-22.04, windows-latest, macos-latest | `pnpm install --frozen-lockfile`, typecheck, lint, test |
| `codegen-drift` | ubuntu-24.04 | Reruns the generator, then fails on `git diff --exit-code` (new untracked files count too) |

The Fedora and Arch containers run on ubuntu-24.04 hosts. The `web` job skips the containers because Node
behaves the same on every glibc distro. The distro-specific risk is native libraries, and the `rust` job
already covers that.

## Activating it

1. Copy `ci.yml` to the repo's `.github/workflows/`. That's a single-writer path, so it needs its own card.
2. Change the default working directory from `spikes/S-5-bakeoff/claude/b4` to `.` in each place it
   appears (the `workflow_call` input default and the `inputs.working-directory || ...` fallbacks).
3. Replace `b4-codegen` with the real generator command.
4. Refresh the pinned SHAs and image digests (Renovate or Dependabot can keep them current).

To reuse the workflow from another one, call it with `uses: ./.github/workflows/ci.yml` and pass
`with: { working-directory: ... }`.

## Local equivalents

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo nextest run --workspace --all-features --locked --no-tests=warn
cargo test --workspace --doc --locked
pnpm install --frozen-lockfile --ignore-scripts && pnpm typecheck && pnpm lint && pnpm test
cargo run --locked -p b4-codegen && git add -N . && git diff --exit-code .
```
