# crates/den-protocol — agent notes

Contracts crate for Spec 02 (`docs/spec/02-protocol.md`). Root `AGENTS.md` still applies; this file only adds
crate rules.

- **Single-writer.** Change this crate only when your card names it. Any change to a public type needs an ADR and a
  dedicated contract card that lands before implementation cards (root `AGENTS.md`, Workflow 6).
- Every public type derives `Serialize, Deserialize, JsonSchema, TS` and has doc comments on the type and every
  field (Spec 02 §3). schemars 1.x and ts-rs 12.x, pinned in `Cargo.lock` (§9.2–9.3).
- Registration is explicit: `fn register(&mut Catalog)`, never `inventory`/link-time collection (§9.1).
- No engine logic, I/O, Tokio or Tauri here. Types, the catalog and validation helpers only.
- Generated artifacts (`schema/*`, `sdk/*/gen`) come from `cargo xtask codegen`. Never hand-edit them.
- License is provisional (`GPL-3.0-or-later` via the workspace) until Q-B6 is answered.

- Layout (DEN-34): `ids`, `timestamp`, `shared`, `events`, `domain`, `template`, `results`, `auth`, `rpc`, `catalog`,
  `manifest` hold the types; `schema` holds the generator settings and transforms; `core::register` registers what
  Spec 02 itself defines; `codegen` builds `protocol.json`, the catalog lint and the TS export that `xtask` writes.
- Fixtures live in `tests/fixtures/<type>/*.json`: one per tagged-union variant, round-tripped and schema-validated
  by `tests/contract.rs`. Add a fixture for every new variant (Spec 02 §17.1). `schema/protocol.json` and
  `schema/plugin.schema.json` are golden snapshots: a type change fails `cargo test -p den-protocol` until you run
  `cargo xtask codegen` and commit the regenerated files.

Checks: `cargo build -p den-protocol`, `cargo clippy -p den-protocol --all-targets -- -D warnings`,
`cargo test -p den-protocol`, `cargo xtask codegen --check` (drift), `cargo xtask lint-catalog`.
`cargo nextest run -p den-protocol` replaces `cargo test` once nextest is adopted (Spec 02 §9.7).
