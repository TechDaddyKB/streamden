# crates/ — agent notes

Rust library crates of the engine (architecture §6). Root `AGENTS.md` still applies.

- Crate names use the `den-` prefix and inherit `[workspace.package]` (edition, license, `publish = false`).
  Add new crates to `members` in the root `Cargo.toml` and refer to siblings with `{ workspace = true }`.
- **Engine ≠ shell:** nothing under `crates/` may depend on Tauri, wry, WebKit or any UI toolkit.
- Public wire types belong in `den-protocol` only (see its `AGENTS.md`). Other crates use them, never redefine them.
- `den-core` (Spec 03): event bus, triggers, actions, queues, variables, storage. CPU-heavy work goes on bounded
  pools, never the async runtime; cap Tokio worker threads (ADR-0002, S-1 condition 6).
- `den-server` (Spec 06): binds `127.0.0.1` by default, checks Host and Origin, LAN mode needs pairing tokens.
- OS-specific code sits behind `cfg` gates or traits; Windows must still compile (D-035).
- New crates.io dependencies: GPL-3.0-compatible license, noted in your handoff. `Cargo.lock` is single-writer.

Checks: `cargo build --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check`.
