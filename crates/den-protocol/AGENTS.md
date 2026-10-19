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

- Layout (DEN-34, DEN-48): `ids`, `timestamp`, `shared`, `events`, `domain`, `records` (§6.5 engine records),
  `template`, `results`, `auth`, `rpc`, `catalog`, `manifest` hold the types; `schema` holds the generator settings and
  transforms; `core::register` registers what Spec 02 itself defines; `codegen` builds `protocol.json`, the catalog
  lint, the example validator and the TS export that `xtask` writes.
- A `Catalog` holds **core** registrations only (plugins contribute through manifests, §11), so every operation and
  event registered must live under a slug in `RESERVED_SLUGS` (§4); synthetic test catalogs use `core.*`, `util.*`,
  `batch.*`. A core event's payload-version steps need both functions registered
  (`Catalog::register_payload_upgrade` / `register_payload_downgrade`, §5.2 A13).
- Examples are validated by the shared `codegen::validate_examples` (what `xtask` runs) against the **producer** view
  (`ProtocolDocument::producer_schema_at`: the receiving-only `unknown` branches of `Node` and `Fragment` stripped,
  §7.2). Diagnostics go through `codegen::describe_validation_error`, which never formats the validator's error kind
  or the instance (the authored values carry exposed secrets; DEN-53): keyword, schema location and an instance path
  with authored map keys masked.
- **Secrets (Spec 02 §3) are typed values first, schema marks second.** A position that holds a secret is
  `Secret<T>` (`src/secret.rs`) in the Rust type: it serializes as `"***"` unless the caller is inside
  `expose_secrets` (the wire path, synchronous and thread-local), `Debug` never shows it, its schema is
  `T`'s plus `x-secret: true`, its TypeScript type is `T`'s. Examples are built from the typed values
  (`OperationExample::typed`, `EventDef::push_typed_example`): the published `params`/`result`/payload is
  serialized with secrets withheld, so it never held the secret, and the authored values the validator
  checks live in a `#[serde(skip)]` field that `redact_examples` drops. Every core example must be typed
  (`tests/codegen_rules.rs::core_examples_are_typed`), and `build_unredacted` refuses a typed example the
  schema walk would still change (a plain field with a hand-set `x-secret`, a pre-serialized exposed value,
  plan params rendered exposed): the value level must be what withholds it. Never set `x-secret` by hand
  on a Rust type when `Secret<T>` can express it; `#[schemars(extend("x-secret" = true))]` is for
  JSON-authored schemas. `EventDef`/`AuthoredExample` print the authored values only by count in `Debug`.
- The schema walk `codegen::redact_secrets` is defence in depth, and the only mechanism for JSON-authored
  examples. `x-secret` is monotonic: mark a field or type `true` and every position that reaches it, by
  name, pattern, composition or `$ref`, is published as `"***"`. `x-secret: false` is rejected by the lint
  everywhere (§3 marks with `true` only; a `false` can only hide a mark, and a field-level
  `#[schemars(extend(...))]` replaces the keys an `#[schemars(inline)]` type brought, before any transform
  here runs: use `extend` for `x-` annotations only). A non-boolean mark fails lint and, if it ever gets
  through, publication redacts the position whole. The walk asks the `jsonschema` validator which
  `patternProperties` apply, so never add a second regex engine to decide that. Marks under
  `patternProperties` and `additionalProperties` are allowed on purpose (schemars emits
  `additionalProperties` for `HashMap<String, T>`).
- **A transform never changes keyword scope.** JSON Schema keywords interact only within their own schema
  object (`additionalProperties` exempts the `properties` beside it, `items` the `prefixItems` beside it),
  so a `$ref` sibling or an optional's surviving branch must never be laid beside the other side's
  validation keywords: `merge_ref_target` keeps the target whole as `allOf: [target]` whenever the property
  carries a validation keyword (a sibling `allOf` takes the target as one more member; annotations are
  lifted, `x-secret` joined monotonically), and
  `DenyNullForOptional` lifts a branch only into a property that holds nothing but annotations. Preserving
  every key is not enough (Codex rounds 4 and 5): a mark that survives textually can stop applying.
- `tests/secret_invariant.rs` is the net for this whole class: it plants a sentinel at every marked
  position of the authored schemas (per schema object, the way the walk unions them), plus typed
  `Secret<T>` sentinels, runs the real pipeline, and fails on any sentinel in any artifact; typed examples
  must hold none before the walk runs and be left unchanged by it.
- Fixtures live in `tests/fixtures/<type>/*.json`: one per tagged-union variant, round-tripped and schema-validated
  by `tests/contract.rs`. Add a fixture for every new variant (Spec 02 §17.1). `schema/protocol.json` and
  `schema/plugin.schema.json` are golden snapshots: a type change fails `cargo test -p den-protocol` until you run
  `cargo xtask codegen` and commit the regenerated files.

Checks: `cargo build -p den-protocol`, `cargo clippy -p den-protocol --all-targets -- -D warnings`,
`cargo test -p den-protocol`, `cargo xtask codegen --check` (drift), `cargo xtask lint-catalog`.
`cargo nextest run -p den-protocol` replaces `cargo test` once nextest is adopted (Spec 02 §9.7).
