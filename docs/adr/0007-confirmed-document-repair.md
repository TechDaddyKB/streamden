# ADR-0007: Explicit confirmed repair of older rejected documents

- **Status:** Proposed (drafted by Claude, DEN-550, 2026-10-22; corrected by the same author unit, DEN-552 and
  DEN-554, 2026-10-22, after the independent reviews DEN-551 and DEN-553; cache-only draft, not applied to the
  repository).
  Not accepted. Only the owner sets Accepted, Rejected or Superseded (ADR-0001); no `docs/decisions.md` row exists
  for it and no decision ID is assigned. The number 0007 is the next free ADR number at the main this draft was
  written against (`docs/adr/` holds 0001–0006 there); the author of the tracked edit re-checks it, never assumes it.
- **Date:** 2026-10-22
- **Deciders:** Owner (explicit acceptance of this text; D-058 is the precedent for text acceptance)
- **Depends on:** [ADR-0006](./0006-approval-surface.md) (public approval surface and confirmation-required
  operations). The owner accepted its exact candidate text and recommendations O1–O11 on 2026-10-22 (project file
  `context/owner-approval-c-and-docs-budget-20261022.md`,
  `f1bec2d4dc3c4fb2016f3df10eafd4334abcf1403e80e545ef66f751cb7fde7e`); the committed file at main
  `99aba2065fc5e169a5e8d9419fc8b50911e2a6f7` (blob SHA-256
  `9f132b0a40002b920cffea11b0167638da3f90438153003b5d300c98fff42500`) still carries its `Proposed` header, and the
  status and ledger follow-up is accounted separately. This ADR **restates** every ADR-0006 clause it relies on
  (Context, "Relied clauses"); nothing is normative here by reference.
- **Inputs (design under review; cited by stored project file and SHA-256; not normative):**
  `context/den519-fable-design-r1.md` (`d849b20f0b7018c52e47ce604067c4244ca8cc2bb9bce7421965586f7bc9bf1c`),
  `context/den532-fable-review-r1.md` (`339be3e93eb2d4d9e607c8344f1ff15a77167a7d59e71123c1180972cb7d006a`),
  `context/den519-fable-design-r2.md` (`5e674787b4f80ac4e1dbe8f63f144a984cde6a0bfd33cd431993a3d18a68831b`),
  `context/den533-fable-review-r2.md` (`363e0dbffa06c4223e6843e377b666fa7834f0e24bf7f7d70718fdf44c923dd5`),
  `context/den551-fable-repair-adr-b-review-r1.md`
  (`3e065b7348ba965edfade3aba5ce07b1b9419f092e9f870ee1e66ce41ce77e98`),
  `context/den553-fable-repair-adr-b-review-r2.md`
  (`5b1c863b9623d84fe06cb2d9a4684059257b4fa5a25bc19592b2845f374ab1af`).
  Review 702 (G1–G10) is applied here as text: G3 (panic staging), G4 (provenance privacy) and G5 (test placement)
  are repair-specific and appear in §9, §11, §12 and the Implementation seams; G1, G2 and G6–G9 are ADR-0006 text.
  Review 765 (M1 validator-before-callbacks, M2 resolve-or-drop origin ids, L1 disclosure fidelity, L2 enum naming,
  L3 pending-record bytes, L4 panic-hook scope) is applied as text in §4, §5, §7, §8, §9, §10, §11, §12, §13, §18,
  Consequences and the acceptance tests. Review 774 (N1–N8 text precisions: guard order, test realizability, the
  disclosure example and wording) is applied as text in §4, §5, §7.1, §8, §11, §18 and the acceptance tests.
- **Scope:** one explicit repair operation for older rejected documents, its discovery read, eligibility, write
  policy, consent disclosure, error mapping, acceptance tests and implementation seams. Every name, value, tier,
  scope, bound and reason string in this document is **PROPOSED**; none exists in code, `protocol.json` or any spec
  at main `99aba2065fc5e169a5e8d9419fc8b50911e2a6f7` (tree `58596815e82f462045e8cfbca4ef12cc1521804d`).

## Context

**Owner policy premise (answered 2026-10-21T18:37:29Z; never re-asked).** Q-E16 in [QUESTIONS.md](../QUESTIONS.md)
is ANSWERED B from project file `context/den423-owner-qe16-option-b-decision.md`
(`d44b60b51ff2a42cd2835320401e91a3265540cc1cfa6bcfebf094cd55db3e7e`). Exact owner text:

> B. Allow explicit repair when migration is missing, fails, or produces invalid data. After confirmation, the
> complete replacement may omit old unknown and migration-preserved fields.

As policy (restated from the recorded answer, which is this ADR's premise and not one of its decisions): an older
document whose migration is missing, fails, or yields a body the current validator rejects may be replaced by an
explicit, confirmed, complete, valid current replacement, which may omit old unknown fields and migration-preserved
fields including `data.x-migrated`. Ordinary updates of such a row keep failing until an explicit repair and never
fall back to a destructive repair. Unsupported newer versions stay read-only. Unreadable raw JSON, malformed raw
metadata, and the revision, conflict, overflow, name, draft and final-current-body guards remain. Current-version
invalid ordinary replacement and guarded deletion keep their existing semantics. Confirmation is a requirement of
every repair operation; the answer is not consent to repair any actual document. A contract and ADR must define
routing, payload, confirmation boundary and concurrency binding before any implementation or public type.

**What the storage seam does at the main named above** (`crates/den-core/src/storage/documents.rs`, an internal,
unfrozen seam with no wire contract; `thread.rs`; `schema.rs`):

- `Documents::get`/`list` decode persisted rows (`read`, `decode_row`) without migrating, validating or
  installing them; a malformed stored id, a revision that is not a positive `i64`, an unparsable timestamp, an
  undecodable `updated_by` or body fail with local typed errors (`InvalidStored`, `Json`).
- `WritePolicy::prepare` is pure: the input `schemaVersion` must equal the registered version; `expected_revision`,
  when given, must equal the stored revision (`Conflict`); **every existing row whose stored version differs from
  the registered one is refused** (`check_existing` → `UnsupportedVersion`, older and newer alike; its comment says
  older data must pass future DEN-302 migrations first); the next revision is `old + 1` with an overflow guard at
  `i64::MAX`; the body must be an object; a body `id` must equal the row id; `revision` is injected into the body
  for `action`, `command` and `timer`; an `action` whose `/origin/kind` is `ai_draft` is forced `enabled: false`;
  `name` must be a string for `action`, `queue` and `command`; `enabled` must be a boolean. `write` re-decodes the
  body, refuses a live-name clash for non-draft `action`, `queue` and `command` rows (`Conflict`) and upserts.
  `batch_put` and `delete` run inside one `IMMEDIATE` transaction on the storage worker; `delete` refuses
  non-current rows the same way.
- `Registry::migrate` chains per-kind steps `N → N+1` under `catch_unwind`: a missing step is
  `InvalidStored("missing document migration")`, a step returning `Err` is `Schema`, a step panic is
  `CallbackPanicked("migration")`, a validator `Err` is `Schema`, a validator panic is
  `CallbackPanicked("validator")`, a stored version above the registered one is `UnsupportedVersion`.
  `Registry::validate_input` runs the same function on a caller body and maps only `Schema` to `InputSchema`.
  `Documents::load` yields `Loaded::{Valid, Invalid, Newer}` in memory only. `Registry` holds `fn` pointers built
  once by `Registry::new` with no mutation API.
- `DocumentError::code()` maps `InvalidInput` and `InputSchema` to `INVALID_PARAMS`, `Conflict` to `CONFLICT`,
  `UnsupportedVersion` to `INVALID_STATE`, everything else to `INTERNAL`; raw diagnostics never reach the wire.
- The storage worker (`thread.rs`): a `MustLand` job awaits capacity and runs once admitted even if its reply is
  dropped; cancellation before admission removes the attempt; **a job panic is terminal** (its reply gets
  `Panicked`, queued replies get `Disconnected`, finalization is attempted). The `audit` table lives in
  `history.db`, not in `config.db` with `documents`.
- Not implemented: `storage/migrate.rs`, `auth/approval.rs`, `ops/settings.rs`, `ops/router.rs` and every other
  `ops/*` and `auth/*` module are one-line reserved stubs; `crates/den-protocol/src/core.rs` registers only
  `plugins.call`; the `Settings` type of Spec 02 §6.5 (A14, cards DEN-69 / DEN-118) is not yet in `den-protocol`.

**What the accepted specs say** ([Spec 02](../spec/02-protocol.md), [Spec 03](../spec/03-engine.md)): migrations
are "never lossy: fields a step can't map go to `data.x-migrated`" and newer documents open read-only (Spec 02 §7.3);
document versions are "migrated lazily on load and rewritten on the next write of that document" (Spec 03 §13.3); an
invalid document loads disabled with `system.document_invalid` (Spec 03 §13.5), whose payload is unspecified beyond
`kind`; the settings writer keeps a rejected current-version record in place until the next successful
`settings.update` replaces it, filling omitted sections from the defaults, and refuses a newer-version record with
`INVALID_STATE read_only` (Spec 03 §16, §22.13). None of these sections distinguishes the owner's older-rejected
route from current-invalid replacement; this ADR does.

### Relied clauses of ADR-0006 (restated; the dependency this ADR binds to)

1. **Approval surface.** `system.list_approvals` (tier 0), `system.grant_approval { approval }` and
   `system.deny_approval { approval }` (tier 1), all `expose: { rpc: true, subAction: false, mcp: false }`, scope
   `admin` **and** the approver rule: only a `Token { tokenKind: ui }` principal on `surface: rpc` with `chain:
   None` may grant, deny or list other requesters' records; anything else is `PERMISSION_DENIED approver_required`.
   A `ui`-kind token is issued only by the engine to the in-process desktop shell, never by pairing or token
   management, and rotates on every engine start (invariant stated as text until spec 06).
2. **Store and key.** Process-local, in-memory, auth-layer store; never persisted; empty after a restart. Records
   are keyed by `(requester key, ApprovalId)` with the requester key `SessionId` when the context has one, else
   `TokenId`; a context with neither (`Plugin`, `Trigger`, `System`, non-`rpc`/non-`mcp` surfaces) cannot request
   an approval and fails `APPROVAL_REQUIRED unattended`. TTL for grant and retry = `APPROVAL_TTL_MS` (120 s; O4).
   Bounds N = 8 non-terminal records per key, M = 256 in total, terminal records retained one TTL and stripped of
   `plan` and `disclosure` on every transition to a terminal state (O5). States
   `pending | granted | denied | consumed | expired`; `granted → denied` revocation allowed (O7).
3. **Generic dispatch owns minting and consumption.** For every approval-bearing call (a member of the
   confirmation-required class under **every** origin principal; MCP-origin tier 3), after the pipeline's rate limit
   and audit and before the operation's handler body: (0) binding render and `argsHash`; (1) dedupe lookup under
   `(requester key, operation, argsHash)`, a match returns the same `approvalId`; (2) the dry-run `plan` and **the
   operation's own pre-mint check, which yields its `disclosure`** ("the repair ADR defines a preview read here");
   (3) repeat the lookup, else mint `pending` and return `APPROVAL_REQUIRED { details.approvalId }`. On a retry:
   pipeline again (a revoked token is `UNAUTHENTICATED`, a removed scope `PERMISSION_DENIED`; an approval never
   grants capability), then (a) no record → `binding_mismatch`/`expired`; (b) `operation` or `argsHash` differs →
   `binding_mismatch`/`args`; (c) past `expiresAt` → `expired`; (d) `pending` → `APPROVAL_REQUIRED` with the same id;
   (e) `granted` → one-shot compare-and-swap `granted → consumed` under the store lock; (f) any other state →
   `expired`. Unknown, foreign, denied and consumed ids are indistinguishable from expired (O9, no oracle). Only the
   CAS winner submits the operation's work. **Consumed stays consumed**: cancellation before admission, admission
   failure (`INVALID_STATE shutting_down`), failure before commit or a disconnect never refund it. The storage
   worker never owns approval state; authorization is checked in the pipeline and at the CAS, not inside the job.
   ADR-0006 makes **no claim about containment of panics inside an operation's job**; the worker contract stands
   (a job panic is terminal) and the approval is consumed in that case too; "how an operation classifies contained
   callback panics is that operation's ADR to state without contradicting this section" (§9 below does).
4. **Class.** `CONFIRMATION_REQUIRED_OPERATIONS` is a `den-protocol` constant published as
   `confirmationRequiredOperations`, **empty in ADR-0006**; each member is added by the ADR that defines the
   operation. Codegen injects `approvalId` on every member; a member's tier is unchanged; `PlanStep.argsHash` and
   `dynamicFields` are present on a member's step; a directly called member has the identity binding render (direct
   calls never carry templates), empty `dynamicFields` and `x-den-template: false` reserved params; a
   `sessionGrant: true` binding never satisfies a member. Under an `Mcp` origin draft-first runs before the approval
   step, and *Apply* replays without an `approvalId`, so it is never a bypass.
5. **Privacy (O10, §8).** A `disclosure` is server-computed from record metadata and closed enumerations. It MUST
   NOT contain values or key names of a stored body, free text derived from stored content (migration, validator or
   SQLite text), panic payloads, hashes of any of these, or **any string decoded from a stored body that has not
   passed the current whole-document schema**; provenance of such a body is disclosed as a **closed trust state
   only**: the `Origin` kind tag and its `trusted` flag, never the raw `from` text. Counts, booleans, closed reason
   enums, timestamps and typed identifiers from record metadata are permitted; no identifier decoded from the body
   is. `plan[].params` are the caller's own params after the `x-secret` schema walk. Events carry no params, hash
   or disclosure; audit rows carry identifiers and kinds only.
6. **Errors and events of the surface.** Reason strings `approver_required`, `not_pending`; events
   `system.approval_pending { approvalId, expiresAt }` and `system.approval_resolved { approvalId, state }`
   (`critical`, minimal payloads, O8); audit kinds `approval_granted`, `approval_denied`.

### Constraints this design must satisfy (restated, self-contained)

| # | Constraint | Source |
|---|---|---|
| C1 | Document writes are tier-2 configuration operations, draft-first under an `Mcp` origin; a whole-document replacement is a document write | Spec 02 §8.1, §6.1; Spec 03 §15 |
| C2 | `approvalId` is codegen-added only (tier 3 and a `den-protocol` constant set); author params never define it; `lint-catalog` rejects it | Spec 02 §8.1; `codegen.rs` |
| C3 | Tier policy is keyed on the origin principal; `Token`, `Trigger` and `System` origins need no approval for ordinary calls, so confirmation for every caller needs the ADR-0006 class | Spec 02 §8.8 rule 5; ADR-0006 §4 |
| C4 | An approval binds `operation + argsHash` over canonical NFC JSON of the params minus `dryRun`/`approvalId`; replay is the identical call | Spec 02 §8.8 rule 4, §8.1 |
| C5 | `PendingChange` *Apply* replays under the user's context; `settings.update` drafts omit `revision` | Spec 02 §6.1; Spec 03 §16 |
| C6 | `dryRun: true` never mutates and takes precedence over draft-first; tier ≥ 1 result roots are objects; every tier-2 operation needs a draft example | Spec 02 §8.1; `codegen.rs` lint |
| C7 | `x-secret` positions are redacted by a schema walk that only knows **current** schemas; an older stored body has no current schema, so its values can never be safely echoed | Spec 02 §3, §8.1 |
| C8 | Error codes are additive; `details.reason` strings are contract text, not new codes | Spec 02 §8.5 |
| C9 | Read errors precede revision checks; callback panics are caught per callback; a job panic is terminal for the worker | `documents.rs`, `thread.rs` |
| C10 | The approval surface and class are ADR-0006's; this ADR adds one member and its pre-mint check | ADR-0006 §3, §4 |
| C11 | Approval state is auth-layer state; the storage worker never owns it | Spec 02 §8.8 rules 4, 7; ADR-0006 §5 |
| C12 | Rows rejected at load are invisible to typed reads (`actions.get`/`list` cannot carry an undecodable `Action`); only `settings.get` exposes a set-aside record's metadata; `CONFLICT` carries no current revision | Spec 03 §13.5, §16 |
| C13 | At the pinned main the writer refuses every non-current row (`UnsupportedVersion` → `INVALID_STATE`); lazy migration on write is unimplemented | `documents.rs` `check_existing` |
| C14 | Server-assigned fields (`revision`, `origin`, `updatedBy`) are ignored on input and set by the server; `origin` changes only through in-app user actions | Spec 02 §6.1, §16 |
| C15 | Persisted documents validate strictly except keys starting with `x-`; a stored body may therefore carry arbitrary `x-*` keys | Spec 02 §3 |
| C16 | Settings are written through the settings writer task: compare revision → commit → swap → publish in queue order; cancellation boundary immediately before the transaction; `shutting_down` after shutdown began | Spec 03 §16 |
| C17 | Engine-internal events may be dropped under internal-lane overflow unless `critical`; the stored revision stays the truth | Spec 03 §5.1, §16 |
| C18 | `x-den-untrusted` marks wrap text for AI consumers but do not redact it for humans, logs or audit | Spec 02 §3, §9.6 |

## Decision (PROPOSED; normative once accepted)

### 1. Operations

Both live in the reserved `system` slug (Spec 02 §4), beside `system.import` (Spec 03 §13.4) and `system.audit`
(§14.4). Neither name exists at the pinned main (`core.rs` registers only `plugins.call`; Spec 03 §15 names
`system.export`, `system.import`, `system.audit`, `system.reset_state`); the author of the registration re-checks the
live catalog for collisions and never assigns a colliding name. Per-kind operations (`settings.repair`,
`actions.repair`, …) are rejected (Alternatives, A11).

| Operation | Tier | Scope | Expose rpc / subAction / mcp | Params → normal result |
|---|---|---|---|---|
| `system.repair_document` | 2 (configuration write, C1) | `admin` (R3) | ✓ / — / — (R2) | `RepairDocumentParams` → `RepairDocumentResult` (§2, §3); `dryRunResult` = `DryRunResult` whose `diff` is the disclosure (§4); `draftResult` = `DraftResult` (§16) |
| `system.list_rejected_documents` | 0 | `admin` (R3) | ✓ / — / — (R2) | `ListRejectedDocumentsParams` → `ListRejectedDocumentsResult` (§6) |

- `system.repair_document` is the first member of `CONFIRMATION_REQUIRED_OPERATIONS`
  (`confirmationRequiredOperations: ["system.repair_document"]`): codegen injects the reserved `approvalId` on its
  invocation schema, approval is required for **every** origin principal, a session grant never satisfies it, and
  its tier stays 2 (relied clause 4). `idempotent: false`, `external: false`, `highVolumeResult: false`.
- `subAction: false` for the same reason as `settings.update`: a run must never rewrite the documents that
  configure the engine executing it, and a run must never record consent. `mcp: false` in the first contract (R2):
  the UI and third parties use the same public RPC; enabling MCP later is additive and §16 already defines what it
  means.
- Examples: one normal, one dry-run (required for tier ≥ 1) and one draft example (required by `lint-catalog` for
  every tier-2 operation even though `mcp: false` makes draft-first unreachable; R12 offers the exemption). All
  examples are typed (`OperationExample::typed`) so secrets are withheld at the value level.
- `system.list_rejected_documents` is tier 0 (a read of record metadata), carries no `dryRun`, is never a
  sub-action and is not MCP-exposed in the first contract (same decision R2; a later `mcp: true` is low-risk and
  additive).

### 2. Params

```rust
pub struct RepairDocumentParams {
    pub id: EntityId,                   // canonical lowercase hyphenated UUID; the nil UUID for `settings`
    pub expected_revision: u64,         // REQUIRED, ≥ 1: the stored revision the caller previewed; no unconditional repair
    pub schema_version: u32,            // REQUIRED: must equal the registered current version of the kind, else INVALID_PARAMS
    pub replacement: RepairReplacement, // the WHOLE current body, kind-tagged
}
#[serde(tag = "kind")]                  // no variant has a field named `kind` (Spec 02 §8.8 note; PendingChange.targetKind exists for this reason)
pub enum RepairReplacement {
    Action(Action), Queue(Queue), Command(Command), Timer(Timer),
    Settings(SettingsDocument),         // every top-level section REQUIRED (below); distinct from `settings.update`'s `Settings`
}
```

- **Typed union inside a property** (Spec 02 §9.6 forbids a root `oneOf`): the replacement is schema-validated with
  ordinary `fieldErrors` JSON Pointers (`/replacement/engine/run/maxConcurrent`), the existing `x-secret` schema
  walk redacts `PlanStep.params` and examples, and each kind's save-time validator runs exactly as `*.update` does.
  A generic `data: object` would defeat C7 (A12).
- **Repairable kinds in the first contract:** `action`, `queue`, `command`, `timer`, `settings` (the kinds with
  contracted schema versions and user-authored content). `pending_change`, `view_state` and `export_meta` are
  excluded: a rejected draft or opaque row is discarded with the existing guarded delete, not repaired. Widening the
  enum is additive.
- **Settings sections.** `SettingsDocument` is the stored-document form of the Spec 02 §6.5 `Settings` type in
  which every top-level section (`engine` in v1; later sections as specs add them) is **required**; an absent
  section is `INVALID_PARAMS` with `fieldErrors[].path = "/replacement/engine"`. The repair route never fills a
  section from the defaults, because consent binds to the exact body the user confirmed (Spec 03 §16's "sections the
  call omits come from the defaults" stays the ordinary `settings.update` rule over a rejected **current-version**
  record and is unchanged). Realization is a distinct struct or a schemars transform of `Settings` that marks the
  sections required (R5); the `Settings` type itself lands with its own contract card (A14), on which this one
  depends.
- **Server-assigned fields.** `revision`, `origin` and `updatedBy` are ignored on input as for every write (C14);
  the params schema marks them as `actions.update` does (`readOnly`, not required). The server assigns `revision`
  (§8), `origin` (§11) and `updatedBy` from the context's origin principal (Spec 02 §6.1 A7). Every server-side
  change to the submitted body is listed in the disclosure's `replacement.normalized` (§4), including the draft
  normalization that **does** arise when the preserved origin of an `action` is `ai_draft` (§11).
- **Size and depth** follow Spec 02 §3 (one message ≤ 1 MiB, depth ≤ 32); a replacement above them is
  `PAYLOAD_TOO_LARGE` or `INVALID_PARAMS` from the envelope and schema layers before any storage work.

### 3. Result (normal shape)

```rust
pub struct RepairDocumentResult {
    pub repaired: True,                 // the existing literal-true type (`results.rs`; schema `const: true`), like DryRunResult.dryRun
    pub kind: String, pub id: EntityId,
    pub revision: u64,                  // expectedRevision + 1
    pub schema_version: u32,            // the registered version the row now carries
    pub updated_at: Timestamp, pub updated_by: UpdatedBy,
    pub origin: Option<Origin>,         // the assigned origin for kinds whose body carries one (`action`, `command`, `timer`; §11); None otherwise
    pub warnings: Vec<String>,          // kind warnings (for `settings`: the §16 SettingsWarning codes as strings)
}
```

The normal result means the effect happened (Spec 02 §8.1). Kind-specific post-commit effects are observable through
the kind's own operations and events (§10). `origin` here is the **stored, validated** value of the repaired
document, not a disclosure of the rejected body (§11 states what may flow into it).

### 4. Preview, disclosure and the approval request

Preview is the contracted `dryRun: true` (C6). It never opens a write transaction, never mints or consumes an
approval, publishes no event and changes no index. It returns `DryRunResult` with `wouldRequireApproval: true` for
every principal, `effectiveTier: 2`, one `PlanStep { operation: "system.repair_document", params /* after the
x-secret walk */, tier: 2, argsHash, dynamicFields: [], description }`, and `diff` = the **disclosure**: a free-form
`Value` (Spec 02 §8.1) identical to what the pending approval record stores (relied clause 3, step (2)):

```json
{
  "target":      { "kind": "action", "id": "018f7a8c-3e6b-7c1d-9a2b-5f4e3d2c1b0a", "storedRevision": 7,
                   "storedSchemaVersion": 1, "registeredSchemaVersion": 2, "updatedAt": "2026-10-22T05:00:00.000Z",
                   "updatedBy": { "kind": "user" } },
  "rejection":   { "reason": "missing_migration", "atSchemaVersion": 1 },
  "discard":     { "unknownTopLevelKeyCount": 3, "xMigratedPresent": true, "source": "raw",
                   "notice": "all_fields_not_in_replacement_are_discarded" },
  "replacement": { "schemaVersion": 2, "revisionAfter": 8,
                   "origin": { "kind": "imported", "trusted": false },
                   "normalized": ["revision: 8", "origin: trust reset", "origin: source identifier not preserved"] }
}
```

Rules (relied clause 5 applied to this operation):

- **No stored values, no key names, no messages, no raw strings.** The rejected body is never echoed in any form.
  `discard.unknownTopLevelKeyCount` is the number of top-level keys of the source body that are not top-level keys
  of the submitted replacement after typed re-serialization; `xMigratedPresent` reports the Spec 02 §7.3 constant
  key `x-migrated` only; `source` says whether the count was taken over the raw stored body (`missing_migration`,
  `migration_failed`, `migration_panicked`) or over the migrated body (`migrated_invalid`). Nested keys are never
  counted or named; no hash of the key list is included (a hash of a low-entropy name is a dictionary oracle). Key
  names in a user-editable database or an import bundle can themselves be secrets or personal data, and
  `x-den-untrusted` would wrap them for AI consumers without redacting them for humans, logs or audit (C18).
  Informed consent under owner B requires knowing *that* every field absent from the replacement is discarded, not
  *which*; the UI shows the typed replacement (redactable) and the target metadata.
- `rejection.reason` is a closed enum derived from the internal classification seam (§7), never from `Schema`
  messages, panic payloads or SQLite text: `missing_migration | migration_failed | migration_panicked |
  migrated_invalid`. `migration_panicked` is listed separately so the UI can word it ("a migration step crashed")
  whatever reading R7 records.
- `target` carries the stored record's metadata only: `storedRevision`, `storedSchemaVersion`, `updatedAt`,
  `updatedBy` (the `UpdatedBy` enum, typed ids only) and the registered version. For `settings` this is what
  `settings.get` already exposes; for the other kinds it is what `system.list_rejected_documents` (§6) exposes, so
  the dry run discloses no more than the tier-0 read.
- `replacement.origin` is the **closed trust state** the server will assign (§11): the `Origin` kind tag and, for
  `imported`, the `trusted` flag; never the raw `from` text and never an identifier (relied clause 5; G4).
  `normalized` lists every server-side change to the submitted body as fixed strings or dotted document keys, in a
  fixed order: the injected `revision` (`action`, `command`, `timer`), `enabled: false` under a preserved `ai_draft`
  origin, trust reset, whether the importer source identifier was preserved (§11), whether a typed origin
  identifier was resolved or dropped (§11), and any serde `default` fill of the typed union. It never quotes a
  stored value. Each origin-resolution `normalized` entry (whether `from` was preserved, and whether a template or
  `draftOf` resolved) reveals at most that closed outcome, never the matched or unmatched value. The injected
  `revision` and the forced `enabled: false` are server-assigned values already carried by `replacement`, not echoes
  of the rejected body.
- `argsHash` in the plan is what the approval binds to (relied clause 4), so a client can show which call is being
  approved.

### 5. Confirmation lifecycle (what this operation contributes to ADR-0006's generic dispatch)

1. **Request without `approvalId`.** The pipeline (Spec 02 §16: validation → scope → tier policy → approval → rate
   limit → audit) classifies the call as approval-bearing (member of the class, every origin). The generic dispatch
   then runs steps (0)–(3) of relied clause 3; **this operation's pre-mint check (step (2)) is the preview read of
   §7.1**, so nobody is asked to approve an impossible repair. In the preview's own order: missing row →
   `NOT_FOUND`; unreadable row → `INTERNAL`; stored revision ≠ `expectedRevision` → `CONFLICT revision`; stored
   version newer than registered → `INVALID_STATE read_only`; replacement rejected by the kind validator →
   `INVALID_PARAMS` (nothing minted; this precedes every old-record callback, §7.1); row not rejected (current
   version, or older but migrates and validates), or rejected by an old-record validator panic, or by a
   migration-step panic under reading (ii) (§7) → `INVALID_STATE not_repairable` (R4). Only when the check passes
   is a `pending` record minted with the server-computed `argsHash`, `plan` and disclosure, and
   `APPROVAL_REQUIRED { details.approvalId }` returned. The dedupe lookup precedes the preview read, so a repeated
   identical call does no storage work (relied clause 3, step (1)).
2. **Human grants in the app** through `system.grant_approval` under the approver rule (relied clause 1). A pure
   REST third party cannot complete a repair without the human at the desktop app.
3. **Retry with `approvalId` (identical call).** Pipeline again, then checks (a)–(f) of relied clause 3; only the CAS
   winner submits **this operation's work**. The CAS step hands the winner, as immutable values copied out of the
   record under the store lock before that lock is released, the consumed record's `approvalId` and `disclosure`
   (the record drops `plan` and `disclosure` on consumption, relied clause 2, so the disclosure is copied out
   before the transition; the `approvalId` is copied beside it for the §10 audit row); the handler passes both by
   value into the work it submits, so the storage worker and the settings writer never read the approval store,
   never await its lock and never own approval state (C11). ADR-0006 leaves what the generic dispatch passes to a
   member's handler unspecified; this ADR requires, for its member, that the CAS step return the consumed
   record's `approvalId` and `disclosure` by value; this is a requirement on the dispatch compatible with ADR-0006
   §3 and §5, not an amendment of them, and U3 must implement it. The work is: for `action`, `queue`, `command`
   and `timer` one `MustLand` storage job running §8; for `settings` one settings-writer request carrying the same
   §8 sequence (C16). The cancellation token is checked immediately before submission (for `settings`, by the
   writer immediately before it opens the transaction).
4. **Consumed stays consumed** (relied clause 3): cancellation before admission, `shutting_down` at admission, any
   failure inside the transaction, or a disconnected caller leaves the approval `consumed`; the caller re-previews
   and re-confirms. One extra confirmation in that rare case is the price of never having two writers under one
   consent; there is no refund.

**Binding coverage.** `argsHash` covers `id`, `expectedRevision`, `schemaVersion` and the entire replacement body
(all are params; canonical JSON, sorted keys, no whitespace, NFC strings, minus `dryRun`/`approvalId`, C4). The
binding render is the identity (direct calls never carry templates, Spec 02 §10.1), `dynamicFields` is empty, and a
stored action can never contain this operation (`subAction: false`), so no node render exists. Any change to target,
revision, schema version or body between preview and retry is `binding_mismatch args`; a concurrent writer that
advanced the stored revision is `CONFLICT` inside the transaction with the approval consumed. A `sessionGrant:
true` binding is ignored for this operation and the call behaves as if no approval were presented (relied clause 4).

### 6. Discovery of rejected rows: `system.list_rejected_documents`

```rust
pub struct ListRejectedDocumentsParams { pub kind: Option<String> }   // filter; None = every repairable kind
pub struct ListRejectedDocumentsResult {
    pub documents: Vec<RejectedDocument>,  // ordered by (kind, id); at most LIMIT entries (R9: 1,000); `truncated` says if more exist
    pub unreadable: Vec<UnreadableCount>,  // { kind, count }: rows whose id, revision, timestamps, updatedBy or body cannot be decoded; never identified
    pub truncated: bool,
}
pub struct RejectedDocument {
    pub kind: String, pub id: EntityId,
    pub stored_revision: u64, pub stored_schema_version: u32, pub registered_schema_version: u32,
    pub updated_at: Timestamp, pub updated_by: UpdatedBy,
    pub rejection: Rejection,              // { reason: RejectionReason, atSchemaVersion: u32 }
    pub repairable: bool,                  // true only for the owner's three cases as read by R7; advisory (§7 re-classifies)
}
pub enum RejectionReason { MissingMigration, MigrationFailed, MigrationPanicked, MigratedInvalid, ValidatorPanicked, InvalidCurrent, NewerSchema }
```

- The result is computed from the engine's live **rejected index**, populated at startup from `Documents::load`'s
  `Loaded::Invalid`/`Loaded::Newer` results (Spec 03 §13.5) and updated by every successful write, delete or repair
  of a row; never from a raw body. It carries exactly the `target` and `rejection` parts of the §4 disclosure plus
  `repairable`, so a client can construct `system.repair_document { id, expectedRevision: storedRevision,
  schemaVersion: registeredSchemaVersion, replacement }` with no prior knowledge of the row.
- `repairable` is `true` only for `missing_migration`, `migration_failed`, `migrated_invalid` and, under reading (i)
  of R7, `migration_panicked`; `false` for `validator_panicked` (§7 rule 2), `invalid_current` (the ordinary update is
  the contracted route) and `newer_schema` (read-only).
- **Unreadable rows** (malformed stored id, revision ≤ 0 or above `i64::MAX`, bad timestamps, undecodable
  `updated_by` or body; `decode_row`) are counted per kind and never identified, because their id may be arbitrary
  bytes. The `kind` string comes from the query, never from a raw row.
- `kind` naming an unknown kind, or a non-repairable kind (`pending_change`, `view_state`, `export_meta`), is
  `INVALID_PARAMS` (the storage seam's `check_kind` precedent for unknown kinds; the non-repairable case is stated
  here so there is no silent empty list).
- Bounded by a hard cap with `truncated` rather than paging (R9), because rejected rows are expected to be rare;
  keyset paging is additive if ever needed.
- The `system.document_invalid` event (Spec 03 §13.5, §16) SHOULD carry the same `RejectedDocument` shape so the
  live notice and the list agree (R10, additive; the event is not re-emittable on demand and the list is the
  on-demand read).

### 7. Eligibility and the classification seam

A row is **repairable** iff it decodes (`read` succeeds), its `schema_version` is **less than** the registered
version, and classification of the decoded record yields one of the owner's three cases. Classification is an
**internal** seam (no wire type) that runs `Registry::migrate` step by step over the **stored old record** and
reports a typed result instead of a bare `DocumentError`:

```rust
pub(crate) enum Classified {
    Healthy(Record),      // older row that migrates and validates (ordinary route, Spec 03 §13.3)
    Current,              // schema_version == registered; validity irrelevant here (ordinary route)
    Newer,                // read-only
    Rejected(RejectedRecord), // { cause, at_schema_version, source: raw | migrated }; not the wire `Rejection`
}
pub(crate) enum RejectedCause {   // internal; distinct from the wire `RejectionReason` of §6 (mapping below)
    MissingMigration,     // case (a): `InvalidStored("missing document migration")`
    MigrationFailed,      // case (b): a step returned `Err` (`Schema`)
    MigrationPanicked,    // a step panicked (`CallbackPanicked("migration")`)
    MigratedInvalid,      // case (c): every step succeeded, the current validator returned `Err` (`Schema`)
    ValidatorPanicked,    // every step succeeded, the current validator panicked (`CallbackPanicked("validator")`)
}
```

The seam records **which record** and **which stage** panicked by construction: classification only ever runs over
the stored old record; the prepared replacement is validated by a separate `validate_input` call, which runs before
any old-record callback (§8 step 4) and whose panic is rule 1 below. The `&'static str` inside `CallbackPanicked` is
not relied on across the two call sites.

**Mapping to the wire.** `Rejection { reason: RejectionReason, atSchemaVersion }` is one wire type shared by
`RejectedDocument` (§6) and the disclosure (§4). The rejected index maps `Rejected(cause)` to the same-named wire
value; a current-version row that `Documents::load` reports as `Loaded::Invalid` to `invalid_current` (a valid
current row is never listed; the seam's `Current` arm does not run the validator); and `Loaded::Newer` to
`newer_schema`. A disclosure can carry only the repairable values under the R7 reading: `validator_panicked`,
`invalid_current` and `newer_schema` rows, and `migration_panicked` under reading (ii), are refused pre-mint
(§5 step 1), so the four-value list of §4 is the repairable subset of the seven-value wire enum.

**Three panic rules**, applied identically in §8, §9, §13 and the test rows:

1. **Replacement-stage panic** (the current validator panics on the prepared replacement): `INTERNAL`
   `details.reason = "panic"` (`error.rs::panic()`), transaction rolled back, nothing written, approval consumed;
   never repairable by that call. It is an engine fault in current code, not a property of the old row, and it
   occurs before any old-record callback has run (§8 step 4).
2. **Old-record validator panic** (all steps succeeded, the current validator panicked on the migrated body): **not**
   one of the owner's three cases (a crash is not a verdict that the data is invalid): `repairable: false` in
   discovery, `INVALID_STATE not_repairable` on preview and repair, `INTERNAL panic` on the ordinary route.
   Conservative recommendation; R7 lists the alternatives.
3. **Old-record migration-step panic**: recommended reading (i) = case (b) "fails" (repairable): from the user's side
   a step that panics and a step that returns `Err` are the same dead end, and the disclosure names it
   `migration_panicked` so the wording stays honest. Under reading (ii) the row is treated like
   `validator_panicked`: `repairable: false`, `not_repairable` on preview and repair, `INTERNAL panic` on the ordinary
   route. R7 records the choice; nothing here presumes it.

Everything else is refused unchanged: newer → `INVALID_STATE read_only`; current-version row (valid or invalid) →
`not_repairable` (the ordinary update is the contracted route, Spec 03 §16); older row that migrates and validates
→ `not_repairable` (the ordinary route migrates lazily and rewrites, Spec 03 §13.3; at the pinned main it is
refused by `check_existing` until that route lands, C13); unreadable row → `INTERNAL` from `read` exactly as today;
missing row → `NOT_FOUND`.

#### 7.1 Preview read

A read-only `MustLand` storage job (correctness-relevant reads the pipeline awaits must not be `Droppable`, Spec 03
§13.1): `read` → the §8 step 3 guards in the same order (`check_revision` → `CONFLICT revision`; newer guard →
`INVALID_STATE read_only`) → prepare and `validate_input` of the submitted replacement exactly as §8 step 4,
including the origin resolution reads of §11 (`INVALID_PARAMS` ends the preview before any old-record callback) →
classify the stored old record (migration attempted in memory under the existing `catch_unwind`, result discarded)
→ compute the disclosure (§4). No transaction write, no approval minting or consumption, no event, no
rejected-index change.
For `settings` the preview reads the stored document, the effective configuration and the provenance together
under the settings writer (Spec 03 §16) so one preview never mixes two revisions.

### 8. Confirmed commit (one `IMMEDIATE` transaction, one job, after the CAS)

In order; any `Err` rolls back by dropping the transaction:

1. Handler preflight (already done before the CAS and before any stored-row callback): params schema;
   `schemaVersion` equals the registered version; body `id` (when present) equals `id`; `enabled` boolean; `name`
   string where required; every settings section present. All `INVALID_PARAMS`. The 425 ordering (caller errors before
   stored callbacks) is preserved and tested (S10).
2. Approval CAS won (relied clause 3); the consumed record's `approvalId` and `disclosure` are handed to the
   handler as immutable values (§5 step 3) → cancellation token checked immediately before submission → the job,
   carrying both by value, opens `TransactionBehavior::Immediate`. The worker never reads the approval store (C11).
3. `read` (unreadable → `INTERNAL`; missing → `NOT_FOUND`) → `check_revision` (`CONFLICT revision`) → newer guard
   (`INVALID_STATE read_only`).
4. **Prepare and validate the replacement before any old-record callback.** `prepare` receives as its `old`
   argument an in-memory copy of the decoded old record whose `schema_version` is set to the registered version, so
   that the existing `check_existing` guard passes; the copy exists only to supply `revision`, `created_at` and the
   identity checks, is never written and is never classified, and the stored old record stays unmodified for
   step 5. Then origin assignment (§11: closed-part decode plus the resolution reads, which are reads of engine
   state that invoke no callback: a storage `read` of the `action` row for `draftOf`, a registry lookup for
   `(plugin, template)`), draft, name and `enabled` normalization, server revision `old.revision + 1` with the
   existing overflow guard, then `validate_input` on the prepared record (`InputSchema` → `INVALID_PARAMS`; a
   panic here is rule 1) and the re-decode. Promotion here is a value in memory; nothing is written until step 6.
5. **Re-classify the stored old record** (§7) under the lock: `Healthy` or `Current` → `INVALID_STATE
   not_repairable` (consent is invalidated by changed state; reachable at a pinned revision only through an impure
   callback, since every write bumps the revision checked in step 3, and therefore logged and counted as an engine
   fault); `Rejected(ValidatorPanicked)`, and
   `Rejected(MigrationPanicked)` under reading (ii) → `not_repairable`; a repairable `Rejected` continues **only
   if** the disclosure recomputed now, from this transaction's own reads and the prepared replacement of step 4,
   equals as canonical JSON (Spec 02 §8.8 canonical form) the consumed approval's `disclosure` of step 2. The
   comparison covers every disclosed field: `target`, `rejection` (`reason`, `atSchemaVersion`), `discard`
   (`unknownTopLevelKeyCount`, `xMigratedPresent`, `source`) and `replacement` (`schemaVersion`, `revisionAfter`,
   `origin`, `normalized`). With the revision pinned by step 3 the stored bytes and metadata are those of the
   preview, so a difference can come only from a callback (contracted pure, Spec 02 §7.3: logged and counted as
   an engine fault) or from the resolvable state of §11 (a template contribution or a `draftOf` row that appeared
   or disappeared: an ordinary state change). Any difference is `INVALID_STATE not_repairable`, nothing written,
   approval consumed; the caller re-previews and re-confirms what is now disclosed. A difference confined to the
   §11 resolution outcome (`replacement.origin` and the `normalized` entries of §11) is an ordinary state change; a
   difference in any other field is an engine fault and is logged and counted.
6. Name clash (`CONFLICT`) → upsert → commit. (A candidate implementation promoted and wrote unconditionally; the
   contract never writes before step 5 has returned a repairable `Rejected` whose disclosure matches.)

### 9. Atomicity, panic staging, neighbours

Refusal at any point leaves row bytes, `schema_version`, `revision`, `created_at`, `updated_at`, `updated_by`,
indexes, neighbour rows, the rejected index and the effective configuration unchanged and publishes no event. A
SQLite failure after the upsert and before commit rolls back the whole row. The operation writes exactly one row, so
batch rollback semantics are not involved (§15).

Every owner callback runs under `catch_unwind` (`Registry::migrate`), so **no callback panic escapes a job**. The
three stages are distinct and must never be merged:

- A caught panic in the **replacement** validator (rule 1), which runs before any old-record callback (§8 step 4),
  is an `Err`: the `IMMEDIATE` transaction is rolled back, nothing is written, the caller gets `INTERNAL panic`, the
  approval stays consumed.
- A caught panic in an **old-record** callback during re-classification is not an error by itself: it yields
  `Rejected(MigrationPanicked)` or `Rejected(ValidatorPanicked)`, and the §7 rules decide the outcome.
  `ValidatorPanicked`, and `MigrationPanicked` under reading (ii), refuse with `INVALID_STATE not_repairable`
  (rollback, nothing written). `MigrationPanicked` under reading (i) is repairable and the transaction **proceeds to
  write the confirmed replacement**; a contained old-record panic can therefore end in a write under the recommended
  reading, and this ADR says so plainly.
- In every caught case the connection and the storage worker stay usable (the existing storage tests
  `callback_panics_are_row_local_and_caller_worker_and_neighbors_recover` and the candidate test
  `validated_callback_failures_roll_back_batch_and_worker_recovers` record that behaviour at the seam).
- A panic that **escapes a job** is not a callback panic but an engine fault. The accepted worker contract makes it
  **terminal** (C9, relied clause 3): the reply gets `Panicked`, queued replies get `Disconnected`, finalization is
  attempted. This ADR claims **no containment, no transaction-rollback promise and no usable worker** for it: the
  caller gets `INTERNAL` (`Transport(Panicked)` maps to `INTERNAL`), the approval stays consumed, and whatever the
  dropped transaction did not commit is not durable.

### 10. Post-commit effects, ordering, audit, cancellation finality, shutdown

After the durable commit the engine does exactly what the kind's ordinary successful update does afterwards, in this
order, and nothing more:

1. **Kind-specific swap.** `settings`: the §16 writer sequence *swap effective configuration → publish
   `settings.updated { revision, changed, restartRequired, updatedBy }`*; `changed` is computed against the previous
   **effective** configuration (the defaults while the record was rejected) and `source` flips `defaults →
   document`. `action`, `command`, `timer`, `queue`: index refresh, enablement per the body's `enabled`, and whatever
   listing or notification the kind's update already emits. The rejected index drops the row.
2. **Event publish**: engine-internal, non-blocking; it may be dropped and counted under internal-lane overflow (C17),
   in which case the stored revision is still the truth and the kind's `get` shows it.
3. **Audit row** in `history.db`, kind `document_repaired` (R11; Spec 03 §14.4 lists refusal kinds today, so the
   success kind is added there on acceptance): `details` = principal kind, requester key kind, `approvalId`,
   document kind and id, from/to revision, from/to schema version, `rejection.reason`, `unknownTopLevelKeyCount`,
   `xMigratedPresent` (the values recomputed at commit, which equal the disclosed ones by the §8 step 5 rule).
   **Never** key names, values, messages or panic payloads. The audit write is a separate
   `history.db` transaction after the `config.db` commit and is **best-effort**: a failure is logged and counted,
   never fails the call and never undoes the committed repair. A refused repair produces no `document_repaired` row;
   approval mismatches are audited by the existing `binding_mismatch` kind; grants and denials by ADR-0006's kinds.

**Cancellation finality** follows Spec 03 §16 and the worker contract exactly: cancelled before admission (for
`settings`, before the writer opens the transaction) → discarded, nothing durable, no event, **approval already
consumed**; after acceptance the request runs to completion whatever happens to the caller's future; a disconnected
caller learns the outcome through the kind's `get` (stored `revision`), the kind's event and the audit row. Nothing is
rolled back because a client disconnected.

**Shutdown.** A submission after shutdown began is `INVALID_STATE shutting_down` for every kind (`settings`: the
writer's own rule; other kinds: the storage thread's admission failure mapped by the handler through
`error.rs::shutting_down()`, so the matrix is uniform); an accepted request completes before the final flush; the
approval is consumed either way.

### 11. Origin and trust of the repaired document (policy flavour, D-029)

Applies to every kind whose body carries `origin` (`action`, `command`, `timer` at the pinned main). The old origin is
read from the **raw stored body** at JSON pointer `/origin` by decoding **only the closed parts** of that subtree:
the `kind` tag against the current `Origin` tag set and, for `imported`, the `trusted` boolean. Open strings of the
subtree are never copied verbatim (the analogue of relied clause 5, which governs the disclosure; the rule for the
repaired document is this ADR's own, next).

**Principle (this ADR's rule for the repaired document; PROPOSED, R6 and R6b).** Nothing is copied from the
rejected `/origin` subtree because it *looks* well-formed: a value is preserved only when the engine **resolves**
it against its own state at commit time (and at preview, §7.1); syntax alone never suffices. At the pinned main
`LocalId` carries no length bound (`ids.rs`: `max_length_for` bounds only `PluginId`, at 128 characters), and no
pattern verifies that a plugin is installed, that a template contribution exists or that an action row exists, so
a pattern check would let an unverified string of any length into a validated document and every later `get`.
ADR-0006 O10 governs the **disclosure** only and is cited as the analogue of this rule, not as its source;
accepting ADR-0006 did not accept this document rule, which the owner accepts only through R6 and R6b. The
resolution reads are reads of engine state that invoke no callback: a storage `read` of the `action` row for
`draftOf`, a registry lookup for `(plugin, template)`; they run inside the write transaction (preview: the same
read job) before any old-record callback (§8 step 4), and their outcome is part of the disclosure the
commit re-checks (§8 step 5). Mapping (R6), invariant **a repair never raises trust**:

| Old `/origin` decodes as | Assigned on repair | `normalized` entry |
|---|---|---|
| `user` | `user` | — |
| `imported { trusted: _ }` | `imported { from: <source id, below>, trusted: false }` | "origin: trust reset" when it was `true`; "origin: source identifier not preserved" when `from` is replaced |
| `plugin_template { .. }` | `plugin_template` with the same ids **only if** `(plugin, template)` resolves to a template contribution of a plugin installed in this installation at commit time (spec 12 registry; at the pinned main no registry exists, so every such row maps to the row below until spec 12 lands); otherwise the row below | "origin: template provenance not resolvable" in place of the row below's entry when it falls through |
| `ai_draft { .. }` (`action` only) | `ai_draft { draftOf }` with `draftOf` kept **only if** it parses as an `ActionId` **and** names an `action` row that `read` decodes under the same transaction (preview: the same read job); otherwise (missing, or not decodable) `ai_draft {}` with `draftOf` absent; still a disabled draft, *Apply* remains the only path to `user` | "enabled: false (ai_draft)" (forced by `prepare`); "origin: draftOf dropped (unresolved)" when dropped |
| missing, or does not decode | `imported { from: "unknown", trusted: false }` | "origin: unknown provenance, sandboxed" |

**Importer source identifier (`from`).** `Origin::Imported.from` is an open `String` in the accepted type. It is
**not copied from the rejected body**. It is preserved only when the raw value **equals one of the importer source
identifiers the engine itself writes** (the closed list spec 15's importer uses; at the pinned main no importer
exists, so that list is empty and every imported row gets `"unknown"` until spec 15 lands); anything else becomes
`"unknown"` and `normalized` says so. This keeps the stored, validated document free of any string of unverified
provenance while retaining the known-source information users rely on (R6b lists the alternatives, including the
bounded-regex copy that ADR-0006 O10 rejected for disclosure).

Why: D-029 ties sandboxing to origin and an `Action` body holds `Code` nodes; a UI repair flow will naturally
pre-fill the replacement from the old raw body, so the content's provenance is the old document's even though the
confirming human submits it. Landing such content under `origin: user` would grant full access to code the user did
not write. The disclosure shows the assigned trust state (§4) so the user sees what they consent to; *Trust this
action* and *Apply* remain the only in-app actions that raise trust (Spec 02 §6.1), after the repair.

### 12. Redaction invariants

No raw stored value, key name, migration message, validator message, panic payload, SQLite text or unverified string
from the rejected body goes on the wire, into the pending approval record, into an event, into the audit row or into
the repaired document: the disclosure is counts, closed enums, timestamps and record metadata; errors carry codes and
reason strings only; `InputSchema` keeps the local validator message about the **caller's own** replacement as today,
subject to the existing `fieldErrors` rules (it never describes the stored body); logs follow the existing
`Debug`-never-shows-secrets and raw-lexeme diagnostic rules (`den-protocol` AGENTS, DEN-53; the `storage_documents`
loader tests). `PlanStep.params` in the dry run and in the pending record are the caller's own submitted params after
the `x-secret` schema walk. The only data that flows from the rejected body into anything durable or visible is:
two counters and one boolean (`discard`), one closed reason enum, the `Origin` kind tag and `trusted` flag,
identifiers from `/origin` that the engine resolved against its own state at commit time (§11; they enter the
repaired document and the §3 result, never the disclosure), and a `from` string that equals a known closed
importer identifier (§11).

**Panic payloads and the process hook.** A caught callback panic's payload is never retained or serialized by the
engine (`catch_unwind` discards it; `error.rs::panic()` carries no payload), but the process-wide panic hook runs
before unwinding reaches `catch_unwind` and may print the panic message to stderr; nothing at the pinned main
installs a redacting hook and this ADR claims none. The engine's panic hook policy is Spec 03's (§10.4, the run
panic rule); migration and validator authors MUST NOT format stored values into panic messages (a fixture rule for
the golden-step tests, Spec 02 §7.3, asserted by S15).

### 13. Errors (existing codes only; PROPOSED additions are reason strings)

| Situation | Code | `details` / `fieldErrors` | Status |
|---|---|---|---|
| Params fail schema (bad id form or alias, `expectedRevision` < 1, wrong `kind` tag, replacement field errors, absent settings section, size, depth) | `INVALID_PARAMS` (-32602) | `fieldErrors[].path` JSON Pointer into params, e.g. `/replacement/engine/run/maxConcurrent`, `/replacement/engine` | existing |
| `schemaVersion` ≠ registered; body `id` ≠ `id`; `enabled` not boolean; `name` not string | `INVALID_PARAMS` | existing storage `InvalidInput` messages | existing (425 ordering) |
| Prepared replacement fails the kind validator | `INVALID_PARAMS` | `InputSchema` local message (caller's own body) | existing |
| No identify / token revoked before the retry | `UNAUTHENTICATED` | — | existing |
| Scope missing / `maxTier` exceeded (re-checked on every call incl. the retry) | `PERMISSION_DENIED` | `details.required` / `reason = "tier_exceeds_context"` | existing |
| Discovery `kind` unknown or non-repairable | `INVALID_PARAMS` | `fieldErrors[].path = "/kind"` | existing code, PROPOSED rule |
| Row missing | `NOT_FOUND` | — | existing |
| Stored revision ≠ `expectedRevision` (preview, pre-mint check, or inside the transaction) | `CONFLICT` | `reason = "revision"` | existing (Spec 03 §16) |
| Replacement name clashes with another live document | `CONFLICT` | — | existing |
| Stored `schemaVersion` newer than registered | `INVALID_STATE` | `reason = "read_only"` | existing (`error.rs`, Spec 03 §16) |
| Row not rejected (current version; older but healthy), old-record validator panic (rule 2), migration-step panic under reading (ii), commit-time disclosure differs from the consented one (§8 step 5) | `INVALID_STATE` | `reason = "not_repairable"` | **PROPOSED reason** (R4) |
| Writer or storage thread shutting down (any kind) | `INVALID_STATE` | `reason = "shutting_down"` | existing |
| No `approvalId` after the pre-mint check passes; or `approvalId` presented while the record is `pending` | `APPROVAL_REQUIRED` | `details.approvalId` (new or the same) | existing (ADR-0006) |
| `approvalId` unknown to this key, foreign, denied, consumed (replay, CAS loser), past `expiresAt`, pre-restart | `APPROVAL_REQUIRED` | `reason = "binding_mismatch"`, `mismatch = "expired"` | existing values; ADR-0006 O9 rule |
| `approvalId` known but `argsHash` differs (body, id, revision or schemaVersion changed) | `APPROVAL_REQUIRED` | `binding_mismatch`, `mismatch = "args"` | existing |
| `sessionGrant: true` binding presented | `APPROVAL_REQUIRED` | `details.approvalId` (a one-shot) | ADR-0006 rule |
| Member called by a context without a requester key | `APPROVAL_REQUIRED` | `reason = "unattended"` | ADR-0006 rule |
| Unreadable raw row, SQLite or transport fault, contained replacement-stage panic (rule 1), escaped job panic (lost worker) | `INTERNAL` | `reason` existing (`panic` for contained panics) | existing |
| Revision at `i64::MAX` cannot be bumped | `INTERNAL` | `InvalidStored("revision overflow")` | existing |
| Ordinary `settings.update` / `actions.update` / … over an older **rejected** row (once the lazy-migration route exists, §14) | `INTERNAL` | `reason = "document_rejected"` so a UI can offer *Repair…* | **PROPOSED reason** (R4); the code is the one owner B's option text names |

No new `ErrorCode`, no new `fieldErrors` shape. Reason strings land in Spec 02 §8.5 text and the `ErrorCode`
description constants in the same contract card, never ad hoc in a handler.

### 14. Ordinary route: what this ADR leaves exactly as it is

- **At the pinned main** every non-current row is refused on `put`, `batch_put` and `delete` by `check_existing`
  (`UnsupportedVersion` → `INVALID_STATE`, no reason string), and nothing migrates on write (C13). This ADR does not
  change that; the lazy-migration route is its own reviewed storage unit (Implementation seams, U2a).
- **Once that route exists**, as owner B's option text describes: an ordinary configuration update over an older
  row whose migration succeeds and validates migrates lazily under the `IMMEDIATE` lock and rewrites (Spec 03 §13.3);
  an ordinary update over an older row with a missing, failing or invalid migration **fails** with `INTERNAL
  document_rejected` and rolls back; only the reason string is proposed here (§13). No silent fallback from update to
  repair anywhere; no boolean on an ordinary update.
- **Precision for the spec text.** With a validated writer the rewrite stores the **caller's** whole body, so an
  ordinary whole-body update over a *healthy* older row already drops `x-migrated` and unknown fields; Spec 02 §7.3
  "never lossy" covers migration steps only. The clause this ADR adds to §7.3 / Spec 03 §13.3 (§17) must therefore not
  promise preservation of unknown or `x-migrated` fields across the next whole-body write.
- Current-version invalid row: the ordinary whole-section replacement succeeds (Spec 03 §16, §22.13); not repairable
  here. Newer rows: read-only on put, repair and delete. Raw reads, `load`, `inspect_rows`, listing alias rejection:
  unchanged. Guarded deletion keeps its existing semantics and is independent of repair.

### 15. Scope: one document per call

First contract: exactly one document per call, one approval per call (one consent ↔ one disclosure ↔ one
`argsHash`; eligibility is per row; the `actions.*` configuration operations are single-document too). Batch repair,
if wanted later, is its own additive operation (for example `system.repair_documents`): all-or-nothing in one
`IMMEDIATE` transaction, one approval binding the whole set, per-row disclosure, whole-batch refusal when any row is
not repairable, and never mixing ordinary writes with repairs. This is a scoping decision recorded for explicit
acceptance (R8), not a presumed default.

### 16. Exposure, generation, draft-first

`protocol.json` gains `system.repair_document` with its three result shapes (`result`, `dryRunResult`,
`draftResult`), the param and result types and the `RepairReplacement` union (`SettingsDocument` included),
`system.list_rejected_documents` with its types, and `confirmationRequiredOperations: ["system.repair_document"]`.
TypeScript and Python bindings and OpenAPI (`oneOf: [result, DryRunResult, DraftResult]`) regenerate; no MCP tool is
generated for either operation while `mcp: false`. Fixtures: one per `RepairReplacement` variant, one
`RejectedDocument` per `RejectionReason`, plus every example (`den-protocol` AGENTS: a fixture for every new variant).
`lint-catalog` keeps rejecting `approvalId` and `dryRun` in author params. The UI consumes the generated schema; no
UI-only endpoint exists anywhere in this design.

**Draft-first (only if MCP exposure is later enabled).** Under an `Mcp` origin the tier-2 draft-first policy runs
before the approval step, so the call never mutates and never mints an approval: it becomes `PendingChange {
targetKind: <kind>, draftOf: id, operation: "system.repair_document", params }` and returns `DraftResult`.
Invariants for the future Apply spec (R13): (i) `PendingChange.params` for this operation **retain**
`expectedRevision` (unlike the `settings.update` precedent, C5), so an Apply after a concurrent write is `CONFLICT`,
never a silent retarget; (ii) Apply replays under the user's context **without** an `approvalId`, so the replay
reaches the pre-mint check and the human's in-app grant is the only way to proceed. With `mcp: false` and `subAction:
false` no `Mcp`-origin context can reach the operation (the Spec 02 §9.6 predicate also filters `invoke_operation`),
so this paragraph documents a future state and the mandatory draft example.

### 17. Spec, ledger and question clauses that change only on acceptance

Each is a separately accounted documentation edit after the owner accepts this text; none is applied by this draft:

- Spec 02 §7.3: "never lossy" is scoped to migration steps; a sentence that an explicit confirmed repair under this
  ADR may replace a rejected older document and omit unknown and `x-migrated` fields; no promise of preservation
  across an ordinary whole-body write (§14).
- Spec 02 §8.1: `system.repair_document` named as the first `confirmationRequiredOperations` member; §8.5: the
  reason strings `not_repairable` and `document_rejected`.
- Spec 03 §13.3: the older-rejected route (ordinary update fails `document_rejected`; explicit repair); §13.5: the
  `system.document_invalid` payload aligned with `RejectedDocument` (R10); §14.4: audit kind `document_repaired`;
  §15: the two operations in the table; §16: the older-rejected exception beside the current-invalid rule and the
  `SettingsDocument` required-sections rule; §22.13: the test rows of this ADR's acceptance tests.
- `docs/decisions.md`: one row recording owner B and this ADR's acceptance; `docs/QUESTIONS.md` Q-E16 status 🟩 with
  the decision ID. No `docs/decisions.md` row and no 🟩 before acceptance.

### 18. Decisions the owner accepts explicitly with this text (options; nothing is presumed)

| # | Decision | Recommendation | Alternatives |
|---|---|---|---|
| R1 | Operation names and slug | `system.repair_document`, `system.list_rejected_documents` in `system` | per-kind operations (A11); a new reserved `documents` slug (needs a `RESERVED_SLUGS` change) |
| R2 | Exposure in the first contract | `{ rpc: true, subAction: false, mcp: false }` for both | `mcp: true` from the start (draft-first applies; the §16 Apply invariants become mandatory immediately) |
| R3 | Scope | `admin` (no new constant) | new `documents:repair` in `KNOWN_SCOPES` and Spec 02 §8.7; `settings:write` for the settings kind only |
| R4 | Reason strings | `INVALID_STATE not_repairable`; `INTERNAL document_rejected`; discovery bad kind `INVALID_PARAMS` | fold `not_repairable` into `read_only` (rejected: a healthy row is not read-only); `INVALID_STATE` for the ordinary rejected update (contradicts owner B's option text) |
| R5 | Settings replacement type | `SettingsDocument` with every section required (distinct struct or required-sections transform of `Settings`) | reuse `Settings` with optional sections and server default fill (rejected: consent would not bind the exact body) |
| R6 | Origin and trust mapping (*policy flavour, D-029*) | §11 table: never raise trust; `imported` → `trusted: false`; `ai_draft` stays a disabled draft; typed origin ids kept only when resolved against engine state; missing or undecodable → `imported { from: "unknown", trusted: false }` | (ii) always `user` because the confirming human authored the replacement; (iii) preserve verbatim including `trusted: true`; (iv) pattern-only preservation of typed ids (rejected for the `LocalId` reason, §11) |
| R6b | Importer source identifier `from` on repair | preserve only when equal to a known importer identifier the engine writes, else `"unknown"` (§11) | bounded raw copy (non-empty, ≤ 64 chars, `^[a-z][a-z0-9_.-]*$`; rejected: a bound does not make a stored string verified, the same reason the typed ids of §11 are resolved, not merely parsed; `LocalId` carries no length bound at the pinned main); always `"unknown"` (loses known-source information) |
| R7 | Panic reading (*policy flavour: reading of owner B's "fails"*) | (i) migration-step panic repairable, old-record validator panic not, replacement-stage panic never | (ii) no panic repairable; (iii) every old-record panic repairable |
| R8 | Scope of a call | one document, one approval; batch is a later additive operation (§15) | batch repair in the first contract |
| R9 | Discovery bound | hard cap 1,000 with `truncated` | keyset paging now; a lower cap |
| R10 | `system.document_invalid` payload | aligned with `RejectedDocument` (additive) | leave the payload unspecified beyond `kind` |
| R11 | Audit kind for a successful repair | `document_repaired` in `history.db`, best-effort after commit | no success row (refusals only, as Spec 03 §14.4 reads today); also audit a commit-time disclosure mismatch under a new kind (for example `repair_disclosure_mismatch`) |
| R12 | Draft example for an unreachable draft-first path | keep the documentary draft example (`lint-catalog` rule unchanged) | a lint exemption for members with `mcp: false` |
| R13 | Apply invariants recorded now for a future MCP exposure | yes, §16 (i) and (ii) | defer to spec 06/13 |
| R14 | ADR number and file | `docs/adr/0007-confirmed-document-repair.md` if still free at authoring time | the next free number then |

Owner B (Q-E16), E11 B, E12 A, E13 A, E15 A and ADR-0006's O1–O11 are premises of this ADR and are **not** among these
decisions. Q-E14 remains an open owner question unrelated to this ADR and is not answered by accepting it.

## Consequences

- **Positive.** Owner B becomes a complete, schema-generated contract: one explicit operation, consent bound to the
  exact body, revision, schema version and principal through ADR-0006's one-shot approval; eligibility limited to
  exactly the three owner cases with an explicit reading of panics; a discovery read that makes rejected rows visible
  without exposing their content; a total trust mapping that never raises trust; a disclosure that reveals counts
  and closed states only; no new error code; the ordinary route unchanged.
- **Negative / accepted limits.** Users consent to a destructive repair seeing counts and metadata, not the old
  content (C7); the UI should show the typed replacement and the assigned trust state. The known-source identifier
  of imported rows is lost until spec 15's importer list exists (R6b), and the template provenance of
  `plugin_template` rows is lost until spec 12's registry exists (§11). A consumed approval is never refunded. A pure
  REST third party cannot complete a repair without the human at the app. Owner validators and migrations run on the
  single storage worker under the `IMMEDIATE` lock during the confirmed path; the preview classifies without the
  write lock. `system.repair_document` is the first member whose params are a whole document (≤ 1 MiB, Spec 02
  §3): ADR-0006 O5 bounds record counts, not bytes, so the non-terminal records of this operation can hold up to
  M × 1 MiB in memory (terminal records drop `plan` and `disclosure`, relied clause 2). This is an accepted limit of
  this ADR, not an owner question here; a byte bound is a future additive ADR-0006 amendment, not decided here.
- **Dependencies and follow-ups.** The approval store, router, settings writer, lazy-migration route, rejected index
  and every handler are stubs or absent at the pinned main; the `Settings` type (A14) is not in `den-protocol`.
  Implementation needs a dedicated contract card for the public types (`den-protocol` single-writer rule), then
  engine units, each ≤ 400 formatted manual lines including all its tests, each with qualified Claude review and
  mandatory Grok QA, canonical checks, native Linux and Windows, current CI and an exact tested merge. The spec
  clauses of §17 land only after acceptance.
- **Nothing here authorizes code.** No public type, handler, store, fixture or test exists or is approved by this
  draft; acceptance of this text is a prerequisite, not an implementation go. Candidates 061c86d (DEN-412) and
  81c91f3a (DEN-425), DEN-365/302/303 and original DEN-71 are not approved by it.

## Alternatives considered

| # | Alternative | Why not |
|---|---|---|
| A1 | `confirm: true` or a "repair" flag on an ordinary update | not evidence of informed confirmation; no binding, TTL, principal or one-shot rule; owner B's "ordinary updates keep failing" forbids it |
| A2 | Classify repair tier 3 so codegen adds `approvalId` | rule 5 exempts `Token` callers, so the user's own UI would never be asked; misclassifies a document write as external/irreversible (C1) |
| A3 | `PendingChange` *Apply* as the confirmation | *Apply* is unspecified (no spec 06/13), has no hash, TTL or one-shot rule; the settings precedent omits `revision` (C5); kept only as the MCP draft-first interaction (§16) |
| A4 | Ad-hoc confirmation or preview token param | duplicates Spec 02 §8.8 without its guarantees; needs a new reserved name and new error semantics |
| A5 | Tier 4 | code execution / end stream; never exposed over MCP; no confirmation semantics |
| A6 | Tier-2 operation plus the constant set **without** ADR-0006 | nobody could record the binding through the public API; a private shell hook violates AGENTS.md |
| A7 | Storage worker owns approval state | violates C11; makes "transaction open" and "consumed" a cross-thread race |
| A8 | Any `admin` principal may grant | self-confirmation by an `api` token (A1 with extra steps); settled by ADR-0006 |
| A9 | Persist approvals in `config.db` | consent survives restarts and code changes; settled by ADR-0006 |
| A10 | Unconditional promotion of every older decoded row (candidate `repair_batch`) | repairs healthy older rows and current-invalid rows the owner routed elsewhere; batch without per-row consent; optional revision |
| A11 | Per-kind repair operations (`settings.repair`, `actions.repair`, …) | multiply catalog entries, examples, fixtures and tests while the semantics are identical per kind; kind-specific post-commit effects are an engine dispatch, not a wire difference |
| A12 | `replacement: object` instead of a typed union | defeats the `x-secret` schema walk (C7), loses `fieldErrors` pointers and the kind validators |
| A13 | Disclose top-level key names (bounded, `x-den-untrusted`) or a hash of the key list | key names in a user-editable database or import bundle can be secrets or personal data; `x-den-untrusted` does not redact for humans, logs or audit (C18); a hash of a low-entropy name is a dictionary oracle |
| A14 | Copy `origin.from` verbatim, or bounded by a regex, from the rejected body | an unverified stored string would enter a validated document and every later `get`; a bound does not make it verified (ADR-0006 O10); R6b offers it as an alternative, not a default |
| A15 | Fill omitted settings sections from the defaults on repair | adds content the user did not submit to a body they consented to by hash |
| A16 | Treat every contained panic as "nothing is written" | contradicts the recommended reading of a migration-step panic (§9); a panic that escapes a job is terminal for the worker and must not be described as recovered |

## Acceptance tests (names PROPOSED; none exists; nothing was run)

Storage seam (`crates/den-core/tests/storage_documents.rs`; Linux and Windows):

| # | Case | Ordinary update | `preview()` / `repair()` with wrong revision | Confirmed `repair()` with exact revision and valid whole body |
|---|---|---|---|---|
| S1 | older, migration missing | `document_rejected` (once U2a lands; `INVALID_STATE` at the pinned main), row bytes and metadata unchanged, no event | `Rejected(MissingMigration)` / `Conflict`, unchanged | replaces; `schema_version` = registered; `revision` +1; unknown keys and `x-migrated` absent; disclosure `missing_migration`, `source: raw`, count equals the raw body's extra top-level keys |
| S2 | older, migration step returns `Err` | same | `Rejected(MigrationFailed)` | replaces; `migration_failed`, count from the raw body |
| S3 | older, migrates then validator returns `Err` | same | `Rejected(MigratedInvalid)` | replaces; `migrated_invalid`, `source: migrated`, `xMigratedPresent` true when the migrated body has it |
| S4a | older, migration step **panics** | `INTERNAL panic`, rollback, worker and neighbours usable | `Rejected(MigrationPanicked)` | reading (i): replaces, `migration_panicked`; reading (ii): refused `not_repairable`; worker usable afterwards in both |
| S4b | older, migrates then validator **panics** | `INTERNAL panic`, rollback, worker usable | `Rejected(ValidatorPanicked)`, `repairable: false` | refused `not_repairable`, unchanged, worker usable |
| S4c | replacement validator **panics** (any rejected row) | n/a | n/a | `INTERNAL panic`, rollback, row unchanged, worker and neighbours usable, no event; the old record's callbacks never ran (counter asserts) |
| S5 | older, migrates and validates | succeeds once U2a lands (lazy rewrite; stored body is the caller's) | `Healthy` | refused `not_repairable`, unchanged |
| S6 | current version, invalid body | succeeds (Spec 03 §16) | `Current` | refused `not_repairable`, unchanged |
| S7 | newer version | `INVALID_STATE read_only` | `CONFLICT revision`, unchanged | `read_only`, unchanged |
| S8 | raw bad metadata (`updated_by`, timestamps, revision ≤ 0 or > `i64::MAX`), undecodable body | `INTERNAL`, no write | `INTERNAL` from `read` | `INTERNAL`, no write, neighbours intact |
| S9 | missing row with `expectedRevision` | `CONFLICT` | `NotFound` (handler `NOT_FOUND`) / `Conflict` (seam, existing) | same |
| S10 | caller errors over a rejected row: wrong `schemaVersion`, body id mismatch, unregistered kind, `enabled` string, `name` non-string, absent settings section, final body invalid | `INVALID_PARAMS` **before** any stored callback | same | same; no write; the old record's callbacks are never invoked (counter asserts); the replacement validator runs before classification (§8 step 4) |
| S11 | name clash with a live row; preserved `ai_draft` origin | `CONFLICT` / draft forced disabled | same | `CONFLICT` / stored `enabled: false`, disclosure lists it |
| S12 | Unicode: replacement with NFD strings | stored bytes are the submitted bytes (serde does not normalize); `name_key` case-folded | — | binding hash equals the NFC preview's; stored body is the typed value re-serialized from the **confirmed** call (unknown fields dropped, field order per the Rust type), not the request bytes and not the preview's |
| S13 | id guards: braced, urn, uppercase, simple forms | `INVALID_PARAMS` | same | same; listing tests unchanged |
| S14 | origin rows: old `user` / `imported { trusted: true }` / `plugin_template` with resolvable ids (through a fixture resolver until spec 12), with well-formed but unresolvable ids, with malformed ids and with an oversized `template` string / `ai_draft` with a `draftOf` naming an existing row, a well-formed `draftOf` naming no row, and a malformed `draftOf` / missing / undecodable | n/a | disclosure shows the closed trust state only, never an id | stored origin per §11; `trusted: true` → `false`; unresolvable, oversized or malformed template ids → the undecodable row; unresolved or malformed `draftOf` → dropped; the stored origin and the result never contain the raw strings; `ai_draft` → `enabled: false`; missing → `imported { from: "unknown", trusted: false }` |
| S15 | disclosure, audit `details` and the repaired document never contain a key name, a stored value, a `Schema` message, a panic payload, the raw `from` text or an unresolved origin id (fixture rows with secret-looking key names, values, `from` strings and oversized origin ids; fixture migrations and validators never format stored values into panic messages, §12) | n/a | assert absence by substring over the serialized disclosure | same over the audit `details` and the stored body |
| S16 | `from` equal to a known importer identifier vs an unknown or oversized string | n/a | `replacement.origin` identical in both cases (closed trust state: kind + `trusted`); `normalized` carries "origin: source identifier not preserved" only in the second case; the raw string appears in neither disclosure (substring assert) | preserved only in the first case; `"unknown"` plus the `normalized` entry otherwise |

Approval store and handler (integration tests once the router and store exist; Linux and Windows):

| # | Case | Expected |
|---|---|---|
| H1 | identical retry with a granted id; then replay | CAS wins, commit, approval `consumed`; replay → `APPROVAL_REQUIRED binding_mismatch expired`, no second write |
| H2 | body changed (one field) | `binding_mismatch args`, unchanged, approval still `granted` (hash check precedes the CAS) |
| H3 | `id`, `kind` tag, `schemaVersion` or `expectedRevision` changed | `binding_mismatch args`, unchanged |
| H4 | a concurrent repair by another session, or any write that bumped the stored revision, committed first | CAS wins, `CONFLICT revision` inside the transaction, unchanged, approval `consumed` |
| H5 | the row was deleted by a concurrent guarded delete (once U2a₂ lands) | CAS wins, `NOT_FOUND` inside the transaction (§8 step 3), nothing written, approval `consumed` |
| H6 | approval past TTL | `binding_mismatch expired`, no CAS |
| H7 | id minted on WS session S1 retried on S2 with the same token; REST id of token A retried by token B | `binding_mismatch expired`; no capability inherited |
| H8 | scope removed, or token revoked, between grant and retry | `PERMISSION_DENIED` / `UNAUTHENTICATED` precede the lookup; approval untouched; nothing written |
| H9 | `sessionGrant: true` binding presented | ignored; `APPROVAL_REQUIRED` with a one-shot id; nothing written |
| H10 | `dryRun: true` with and without `approvalId` | `DryRunResult`, no write, no mint, no consume, `wouldRequireApproval: true`, `dynamicFields: []`, `diff` equals the pending record's disclosure |
| H11 | two concurrent identical retries with one granted id | exactly one CAS winner (commit or a storage-side error) and exactly one `binding_mismatch expired`; never two writes |
| H12 | cancellation before admission / after acceptance / after commit before swap / during publish | Spec 03 §16 finality: stored revision, effective configuration and event all show the committed revision or nothing durable; approval `consumed` in every case |
| H13 | shutdown after the CAS, before admission (each kind) | `INVALID_STATE shutting_down`; approval `consumed`; nothing written |
| H14 | settings repair | `settings.get` flips `source: defaults → document`, `pendingRestart` recomputed, `settings.updated` carries `changed`/`restartRequired`; no running run cancelled; routed through the writer task in queue order with a concurrent ordinary `settings.update` |
| H15 | audit | one `document_repaired` row per success when `history.db` is writable; none on refusal; with `history.db` unwritable the repair still succeeds and the failure is logged |
| H16 | retry while `pending` | `APPROVAL_REQUIRED` with the same id, no mint, no consume |
| H17 | pre-mint check refusals (missing, unreadable, stale revision, newer, kind-validator-rejected replacement, healthy, current-invalid, validator-panicked rows) | `NOT_FOUND` / `INTERNAL` / `CONFLICT` / `read_only` / `INVALID_PARAMS` / `not_repairable`; nothing minted; dedupe lookup ran first; the old record's callbacks never ran for the `INVALID_PARAMS` case (counter asserts) |
| H18 | MCP origin (only when exposure is enabled) | `DraftResult` with a `pending_change` retaining `expectedRevision`; live row untouched; Apply replay requires a fresh in-app grant |
| H19 | member called by a `Trigger`, `System` or `Plugin` context | `APPROVAL_REQUIRED unattended`; nothing minted; nothing written |
| H20 | disclosure fidelity: a fixture validator whose verdict depends on mutable state flips between preview and retry; separately, the `draftOf` row of a preserved `ai_draft` origin is deleted between preview and retry; separately, a fixture whose classification returns `Healthy` or `Current` at the pinned revision without any write | CAS wins, `INVALID_STATE not_repairable`, nothing written, approval `consumed`; the engine-fault log and counter fire in the first and third cases only (the second is the §11 state change of §8 step 5); a fresh preview discloses the new state |
| D1 | discovery: one row per S1–S7 kind of rejection plus an unreadable row | the list carries S1–S4b/S6/S7 with the right `reason`/`repairable`, counts the unreadable row per kind without an id, never includes a healthy row; validates against the schema; no key names or values |
| D2 | discovery after repair / delete / ordinary healthy write | the row leaves the list; `truncated` at the cap |
| D3 | `system.document_invalid` payload | equals the `RejectedDocument` the list returns for the same row (R10) |
| D4 | `kind` filter unknown or non-repairable | `INVALID_PARAMS` with `fieldErrors[].path = "/kind"`; no storage work |

Protocol (`den-protocol` tests): fixtures per `RepairReplacement` variant, per `RejectionReason` round-trip; examples
validate against all three shapes; `codegen_rules`: `confirmationRequiredOperations` equals
`["system.repair_document"]`, `approvalId` is on its invocation schema and absent from its author params, `dryRun` is
present, no MCP tool is generated for either operation while `mcp: false`, `system.list_rejected_documents` has no
`dryRun`, the secret sentinel test is unchanged; `SettingsDocument` rejects an absent section while `Settings` accepts
it.

Native both: every named test plus the canonical workspace, web, codegen `--check`, hooks and current CI on Linux and
Windows before any merge (existing gates; nothing waived). None of these tests exists or was run.

## Implementation seams and budget (no fixed line counts asserted, no fit claimed)

Hard rule: ≤ 400 formatted manual ADD+DEL per author unit **including all tests, helpers and revisions**; a fresh
complete canonical baseline and preflight **before** the first tracked edit; a conservative upper estimate plus
revision margin; no test trimming, minification, unsafe half-API or test-only split; a unit's tests are every test of
the behaviour that unit ships, inside that unit. The candidate figures (DEN-412 = 399, DEN-425 = 193 formatted manual
ADD+DEL, aggregate 592) are not one unit and are not imported, cherry-picked or reset; the frozen DEN-518 TLS unit is
unrelated and untouched. Each author forecasts before editing; nothing below asserts fit.

| Unit | Complete behaviour it ships (API + every test of it) | Depends on |
|---|---|---|
| U1a-B | this ADR as `docs/adr/0007-…md` (docs only; the §17 spec, ledger and QUESTIONS clauses are separate accounted units after acceptance) | ADR-0006 accepted (done, owner record 745); fresh preflight |
| U1b-B | `den-protocol` types: `RepairDocumentParams`, `RepairReplacement`, `SettingsDocument` (or the transform), `RepairDocumentResult`, `RejectedDocument` and the list types, `RejectionReason`, the `CONFIRMATION_REQUIRED_OPERATIONS` member, reason description text, fixtures, `contract.rs` / `codegen_rules.rs` tests; no registration | this ADR accepted; ADR-0006's U1b-A (constant set, codegen branch); A14 `Settings` |
| U1c | catalog registration with typed examples (DEN-69 / DEN-118 registration module), regenerated `schema/*` and `sdk/*/gen` | U1b-B |
| U2a₁ | validated writer on the **current-version** path (`validate_input` inside the transaction, 425 caller-preflight ordering, stale doc-comment fix) with **every** test of that behaviour, **including the current-version validator-panic rollback and worker-recovery test** (G5: it ships with the behaviour it tests) | none (unfrozen seam) |
| U2a₂ | older rows on the ordinary path: lazy migrate-on-write for healthy older rows, `document_rejected` for rejected older rows, **migration-step panic rollback (older rows) and worker recovery**, with every test of that behaviour | U2a₁ |
| U2b | repair primitive: `classify()` seam (§7), `preview()`, single-row `repair()` with mandatory revision, the origin resolver seam (§11: action-row existence from the transaction; template contributions from spec 12's registry once it exists, a fixture resolver until then), the commit-time disclosure comparison (§8 step 5) and the disclosure, rejected-index hooks; S1–S16 repair columns. If the forecast exceeds the cap, split by **complete primitive** (`classify` + `preview` with their tests / `repair` with its tests), never by case family | U2a₂; this ADR accepted for the disclosure shape |
| U3 | approval store (ADR-0006): mint, list, grant, deny, CAS, expire, bounds; its own tests | ADR-0006 units |
| U4 | settings writer task (Spec 03 §16) including the repair request variant | independent card |
| U5 | handler: `system.repair_document` and `system.list_rejected_documents` in `ops/`: pipeline hooks, preview, pre-mint check, consent metadata handoff (§5 step 3), kind dispatch, audit; H1–H20, D1–D4; likely several complete units (preview and discovery / confirmed commit per kind) | U1c, U2b, U3, U4, router |

Explicitly rejected: shipping an API with half its tests and the other half later. Every split above is by complete
behaviour with its complete tests.
