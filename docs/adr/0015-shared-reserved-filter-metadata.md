# ADR-0015: Shared reserved filter metadata for `user.*` and `platform`

- **Status:** Accepted. The owner accepted the exact table and registration contract below as the basis for
  this ADR on 2026-10-22 and this exact text (DEN-578 r3) on 2026-10-22; the contract task and its implementation are not accepted.
- **Date:** 2026-10-22
- **Deciders:** Owner
- **Number:** 0015, assigned at placement.

## Context

Spec 02 §6.2 and Spec 03 §6.2 define the reserved filter paths `user.*` and `platform`: they resolve against
the event envelope's normalized `user` (`UserRef`, Spec 02 §5.3) and `platform`, never against `event.payload`,
and a bare `user` path is `INVALID_PARAMS` (Q-E13). A payload must not declare a top-level `platform` or a
reserved run-context root such as `user` (Spec 02 §6.3, A12). At pinned `main` `9751e40a` the matcher honours
the paths (`crates/den-core/src/matcher/filter.rs`, `lookup`), but a filter only compiles against an exact
`FilterableField` of its event (`CompiledFilter::compile`), and the catalog gives a reserved path no sanctioned
`FilterableField`: the catalog lint (`crates/den-protocol/src/codegen.rs`, `lint`) fails, for a core payload that
declares `properties`, both any `EventDef.filters` entry whose head is not a payload property and any top-level
reserved run-context root or `platform`. Reserved-path filters have no sanctioned metadata home,
so no catalogued event offers them to a trigger editor or a plugin author, and the Spec 02 §6.2 rule
"validated against the field's schema" has nothing to validate against for them (Q-E14).

The catalog registers zero event types at the pin (`protocol.json` `events: {}`); every per-event filter table
waits for the Spec 03 §15 and connector registrations. `PluginManifest::validate` checks only the slug, so the
Spec 02 §6.3 payload rule and any filter rule have no enforcement point for plugins yet.

On 2026-10-22 the owner decided the direction (record 909): D1 A, operator-specific operand checks derived from
existing schemas; D2 i, one shared definition included in every event's published `filters` (no new top-level
list, no per-event hand declaration); D3 a, range operands accept RFC 3339 offsets and fractional precision and
JSON numbers on numeric fields, matching the accepted comparator. The owner then accepted the exact table and
registration contract of the DEN-577 review (file 915 §4.1–4.2, packet 916) as the basis for this ADR and a
dedicated `den-protocol` contract task (record 918). AGENTS.md rule 6 and ADR-0001 require an ADR for any
`crates/den-protocol` change.

## Decision

### 1. One shared definition

`den-protocol` owns one function, `reserved_filter_fields() -> Vec<FilterableField>`, returning the nine
entries below in this fixed order. Every entry has `specificity: 1` and `caseInsensitive: false` (the A5
defaults of Spec 02 §6.2; ids compare exactly per Spec 03 §6.2) and no `optionsFrom` (no picker operation
exists). `FilterableField`, `Filter` and `FilterOp` gain no member; the wire types are unchanged.

| # | `field` | Resolved schema (transformed, receiver view) | Presence at match time | `ops` |
|---|---|---|---|---|
| 1 | `platform` | `#/types/PlatformId` (`string`, pattern `^[a-z][a-z0-9_]{0,63}$`) | optional in the envelope | `eq ne in not_in contains regex exists` |
| 2 | `user.platform` | `#/types/UserRef/properties/platform` → PlatformId | iff `user` present | same |
| 3 | `user.id` | `string` | iff `user` present | same |
| 4 | `user.login` | `#/types/UntrustedString` (untrusted) | iff `user` present | same |
| 5 | `user.displayName` | `#/types/UntrustedString` (untrusted) | iff `user` present | same |
| 6 | `user.roles` | `array`, `items: { type: string }` | iff `user` present (serde default `[]`) | `contains eq ne in not_in exists` |
| 7 | `user.avatarUrl` | `string` (lifted optional) | iff `user` present and known | `eq ne in not_in contains regex exists` |
| 8 | `user.color` | `string` | iff `user` present and known | same |
| 9 | `user.pronouns` | `string` | iff `user` present and known | same |

`user.badges` is deliberately **not** an entry: `values::equal` compares objects by key count, so
`contains { "id": "subscriber" }` would never match a badge that also carries `version`. Adding it later is
additive and needs its own design. Range operators are absent because none of these domains is an instant or a
number. Every registered event publishes all nine entries, including engine events that carry neither `user` nor
`platform` (Spec 03 §15): on such an event `exists: false` is true for every reserved path and the other
operators never match.

### 2. Raw declarations are validated; registered definitions receive the appended list

Two operations with one insertion boundary, both in `den-protocol`:

- `validate_declared_filters(&[FilterableField]) -> Result<(), CatalogError>` runs on **raw author input** (a
  core `EventDef.filters` or a manifest `contributes.events[].filters`) and rejects any entry whose head is in
  `RESERVED_RUN_CONTEXT_ROOTS` or equals `RESERVED_FILTER_PATH_PLATFORM` (`"platform"`), any `field` with an empty
  segment, and any duplicate `field`. New variants: `CatalogError::ReservedFilterField(String)` and
  `CatalogError::DuplicateFilterField(String)`.
- `Catalog::register_event` calls it, then appends `reserved_filter_fields()` once, in order, before inserting
  the definition. `Catalog.events` is private, so this is the only insertion point.

A registered `EventDef` is never re-validated as raw input, and no idempotence is claimed: an author-declared
entry equal to a reserved entry is rejected, not merged, because its provenance cannot be told apart later.

### 3. Lint checks the post-condition

For every registered event the lint requires that the trailing `reserved_filter_fields().len()` entries equal
the definition exactly (`FilterableField: PartialEq`) and that no earlier entry has a reserved head. The
existing "head must be a payload property" check keeps applying to the earlier entries, and the existing
rejections of payload top-level reserved roots and `platform` are unchanged. `cargo xtask codegen --check`
catches drift between the function and the published artifact (Spec 02 §9.7).

### 4. Plugin manifests

`PluginManifest::validate` runs `validate_declared_filters` on every `contributes.events[].filters` and fails a
payload that declares a top-level reserved run-context root or `platform` (Spec 02 §6.3). Plugin ingestion
builds `EventDef`s and registers them through `Catalog::register_event`, so plugin events receive the same
appended entries; that ingestion code does not exist at the pin and is future work.

### 5. Publication and generated artifacts

The publication mechanism is unchanged: `protocol.json` flattens `EventDef.filters`, so the entries appear under
`events.<type>.filters` as events register. The doc comments of `FilterableField.field` ("dotted path into the
payload") and `case_insensitive` (pre-Q-E12 wording) are corrected in the same unit; they are published
descriptions and TypeScript exports, so the generated artifacts are regenerated, never hand-edited. No current
codegen output proves this ADR; the check runs with the contract task.

### 6. Trust, taint and error vocabulary

Values of `platform` and `user.*` come from the trusted envelope, never from a payload fallback (Spec 02 §6.2).
Entries 4 and 5 are `x-den-untrusted` text: a `regex` filter over them taints `match` (Spec 03 §21), which the
future resolver and the DEN-100 admission work must honour by carrying the untrusted mark; this ADR does not
design that hook. Filter rejection reasons are an engine vocabulary (`EngineError` constructors), not the Q-E15
variables mapping; the den-core unit and the Spec 03 §6.2 clarification fix them.

### 7. Compatibility

Adding entries later is additive. Removing an entry or narrowing its `ops` is a `schema-compat` question and is
not approved by this ADR.

## Consequences

- One function serves core and plugins; the catalog, the matcher's index and `protocol.json` carry the same
  list, and the lint guards drift.
- The contract task is complete only with its tests: validation rejects reserved heads, duplicate fields and empty
  segments; registration appends in order; lint accepts the canonical suffix and rejects a foreign reserved
  head, a modified suffix or a missing suffix; `Catalog::core()` lints clean; manifest validation rejects
  reserved filter heads and payload roots; regenerated artifacts pass `cargo xtask codegen --check`.
- Spec 02 §6.2, §9.7, §11 and Spec 03 §6.2 need matching clarification text, and `docs/QUESTIONS.md` Q-E14
  records D1 A / D2 i / D3 a, in a separate documentation unit.
- A client discovers the entries in `events.<type>.filters` (`protocol.json` for core; the live catalog for
  plugins, wiring unverified) and learns that an entry is reserved only from its path head (D2 i).
- Editors may offer the `user.*` entries on events that never carry a user; such filters compile, and apart from
  `exists: false` they never match; a `platform` entry evaluates normally whenever the event carries `platform`.

## Deferred boundaries (not decided here)

The general schema resolver and operand validator (D1 A, D3 a), the transformed-schema entry point, the
untrusted mark on a resolved field, engine integration and the owner of the live catalog, the plugin ingestion
code path and `den://` / `#/definitions/` reference resolution, the `user.badges` entry, the filter error-reason
vocabulary and `error.rs` scope, and the UI filter editor are separate designs that build on this contract.

## Alternatives considered

- **A new published top-level list of reserved fields** (D2 ii): not chosen by the owner; it adds a wire member
  and a second source of truth.
- **Per-event hand declaration, including plugins** (D2 iii): not chosen by the owner; it invites drift and the
  pinned lint forbids it.
- **A normalizer that rejects reserved heads and is idempotent on its own output**: self-contradictory, since
  its output contains reserved heads; replaced by the boundary of Decision 2.
- **Silently replacing an author-declared reserved entry**: guesses provenance; rejected.
- **Including `user.badges` now**: a trap under whole-object equality; deferred.
- **Publishing an operand-expectation member on `FilterableField`**: not chosen under D1 A.

## Unresolved engineering gaps (not owner questions)

- Exact `CatalogError` and `ManifestError` diagnostic texts, and whether manifest validation reuses `CatalogError`.
- Where plugin ingestion lives and which component owns the live catalog that the planned `TriggerIndex::build`
  (DEN-81) consumes.
- The transformed-schema entry point for the future resolver (untransformed definitions admit `null` on
  optional fields, against Spec 02 §3).

## Sources

Owner records 918 (acceptance) and 909 (D1 A / D2 i / D3 a) and packet 916 in the codex-orchestrator project
`streamden`; pinned sources at `9751e40a`: `crates/den-protocol/src/` `catalog.rs`, `codegen.rs`, `manifest.rs`,
`version.rs`, `shared.rs`, `ids.rs`, `events.rs`; `crates/den-core/src/matcher/filter.rs` and `error.rs`; Spec 02
§3, §5.1, §5.3, §6.2, §6.3, §9.7, §11; Spec 03 §6.2, §6.3, §15, §21; ADR-0001; `docs/decisions.md` D-039, D-058.
