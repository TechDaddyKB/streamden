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
- `x-secret` (Spec 02 §3) is monotonic: mark a field or type `true` and every position that reaches it, by name,
  pattern, composition or `$ref`, is published as `"***"`. `x-secret: false` never clears a mark a referenced
  type carries (the lint rejects that sibling); a non-boolean mark fails lint and, if it ever gets through,
  publication redacts the position whole. `codegen::redact_secrets` asks the `jsonschema` validator which
  `patternProperties` apply, so never add a second regex engine to decide that. Marks under
  `patternProperties` and `additionalProperties` are allowed on purpose: schemars emits `additionalProperties`
  for `HashMap<String, T>` (a map of named secrets is a common shape), and applicability there is the
  validator's own verdict, so forbidding them would cost expressiveness without adding safety.
- `x-secret: false` is rejected by the lint everywhere: §3 marks with `true` only, and a `false` can only
  hide a mark. `#[schemars(extend(...))]` on a field *replaces* top-level keys of that field's schema,
  including everything an `#[schemars(inline)]` type brought with it, before any transform of this crate
  runs: use `extend` for `x-` annotations only, never to set `x-secret: false` or a validation keyword
  (`properties`, `required`, …) beside an inline type. The `DenyNullForOptional` transform lifts an
  optional's surviving branch into the property only when the property holds nothing but annotations
  (`x-secret` joined monotonically) and keeps it whole in `allOf` otherwise; a transform must never take
  "the keys the other side lacks" and drop the rest. `tests/secret_invariant.rs` plants a sentinel at every
  marked position of the authored schemas and runs the real pipeline; it is the net for this whole class.
- Fixtures live in `tests/fixtures/<type>/*.json`: one per tagged-union variant, round-tripped and schema-validated
  by `tests/contract.rs`. Add a fixture for every new variant (Spec 02 §17.1). `schema/protocol.json` and
  `schema/plugin.schema.json` are golden snapshots: a type change fails `cargo test -p den-protocol` until you run
  `cargo xtask codegen` and commit the regenerated files.

Checks: `cargo build -p den-protocol`, `cargo clippy -p den-protocol --all-targets -- -D warnings`,
`cargo test -p den-protocol`, `cargo xtask codegen --check` (drift), `cargo xtask lint-catalog`.
`cargo nextest run -p den-protocol` replaces `cargo test` once nextest is adopted (Spec 02 §9.7).
