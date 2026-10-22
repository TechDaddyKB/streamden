# ADR-0006: Public approval surface and confirmation-required operations

- **Status:** Proposed (drafted by Claude, DEN-538, 2026-10-22). Not accepted. Only the owner sets Accepted,
  Rejected or Superseded (ADR-0001); no `docs/decisions.md` row exists for it and no decision ID is assigned.
- **Date:** 2026-10-22
- **Deciders:** Owner (explicit acceptance of this text; D-058 is the precedent for text acceptance)
- **Inputs (design under review; cited by stored project file and SHA-256; not normative):**
  `context/den519-fable-design-r1.md` (`d849b20f0b7018c52e47ce604067c4244ca8cc2bb9bce7421965586f7bc9bf1c`),
  `context/den532-fable-review-r1.md` (`339be3e93eb2d4d9e607c8344f1ff15a77167a7d59e71123c1180972cb7d006a`),
  `context/den519-fable-design-r2.md` (`5e674787b4f80ac4e1dbe8f63f144a984cde6a0bfd33cd431993a3d18a68831b`),
  `context/den533-fable-review-r2.md` (`363e0dbffa06c4223e6843e377b666fa7834f0e24bf7f7d70718fdf44c923dd5`).
  This ADR restates every clause it relies on; nothing in those files is normative by reference.
- **Scope:** the shared in-app approval surface and the operation class that always requires it. Repair-specific
  operations, payloads, eligibility, write policy and tests belong to a later, separate ADR ("the repair ADR"
  below; no number or API is assigned here). Every name, value, tier, scope and bound in this document is
  **PROPOSED**; none exists in code, `protocol.json` or any spec at the accepted main this draft was written
  against (`8a7c0e43497df5f4dee38106549eed3fc2322e17`).

## Context

**Owner policy premise (answered, not re-asked).** Q-E16 was answered B on 2026-10-21T18:37:29Z (project file
`context/den423-owner-qe16-option-b-decision.md`, `d44b60b51ff2a42cd2835320401e91a3265540cc1cfa6bcfebf094cd55db3e7e`;
recorded in [QUESTIONS.md](../QUESTIONS.md)): "B. Allow explicit repair when migration is missing, fails, or
produces invalid data. After confirmation, the complete replacement may omit old unknown and migration-preserved
fields." Confirmation is a requirement of every repair operation, for every caller. That answer is policy, not
per-document consent, and it is not a decision of this ADR.

**What the accepted contract already has** ([Spec 02](../spec/02-protocol.md), [Spec 03](../spec/03-engine.md),
`crates/den-protocol`): an approval handle `ApprovalId` with a 120 s TTL (`APPROVAL_TTL_MS`, Spec 02 §4);
`ApprovalBinding { id, operation, argsHash, plan?, sessionGrant, expiresAt }` living in `AuthContext.approvals`,
which is created at the entry surface, never widened and never settable from any API input (Spec 02 §8.8 rules 4
and 7); the binding hash over canonical NFC JSON of the call's params minus `dryRun` and `approvalId`; the
protocol `APPROVAL_REQUIRED { details.approvalId }` followed by the identical call with `approvalId`, and
`details.reason = "binding_mismatch"` with `details.mismatch ∈ { args, revision, unplanned_call, expired }` or
`details.reason = "unattended"` (Spec 02 §8.5); reserved, codegen-added params `dryRun` (tier ≥ 1) and
`approvalId` (tier 3 and the constant `DYNAMIC_TIER_OPERATIONS`), both rejected in author params by `lint-catalog`
(Spec 02 §8.1, `codegen.rs`); the pipeline order validation → scope → tier policy → approval → rate limit → audit
(Spec 02 §16); tier policy keyed on the origin principal, with `Token`, `Trigger` and `System` origins exempt from
approval (Spec 02 §8.8 rule 5); `Principal::Token { tokenKind: ui | api | overlay }`, `Surface::Rpc` meaning
WebSocket or REST, `SessionId` per WebSocket connection and no session for a REST request (Spec 02 §4, §8.8);
D-030 "in-app approval for destructive tools" and Spec 03 §14.4 "spec 06/13 build the approval UI on it".

**What it does not have (verified at the accepted main named above).** No catalog operation mints, lists, grants
or denies an approval: `crates/den-protocol/src/core.rs` registers only `plugins.call`; `crates/den-core/src/auth/*`
and `crates/den-core/src/ops/*` are one-line reserved stubs (no approval store, binding engine, router or
handler); spec 06 (API/MCP) and spec 13 (approval UI) do not exist. AGENTS.md forbids private UI-only endpoints, so
the desktop shell cannot record an approval through anything but the public API. Spec 02 §8.1 prose lists
`auth.*` among protocol methods, while the lint constant `PROTOCOL_METHODS` names only `auth.request_pairing`.

**Why this ADR is needed first.** Owner B requires confirmation of a repair by every principal, including the
user's own UI token, which rule 5 exempts today; the same gap exists for MCP tier-3 approvals (D-030), whose
"in-app approval" has no contracted public surface. Both need one answer: who may record an approval, under which
identity it is redeemable, how it is consumed exactly once, what the approver sees, and how clients learn about it
without polling. The repair ADR depends on this one and adds its operation to the class defined here.

### Constraints this design must satisfy (restated, self-contained)

| # | Constraint | Source |
|---|---|---|
| C1 | Document writes are tier-2 configuration operations, draft-first under an `Mcp` origin | Spec 02 §8.1, §6.1; Spec 03 §15 |
| C2 | `approvalId` is codegen-added only (tier 3 and a `den-protocol` constant set); author params never define it | Spec 02 §8.1; `codegen.rs` lint |
| C3 | Tier-3 approval, MCP exclusion and draft-first are keyed on the origin principal; `Token`/`Trigger`/`System` need no approval | Spec 02 §8.8 rule 5 |
| C4 | An approval binds `operation + argsHash`, lives in the caller's context, expires with the handle TTL; replay is the identical call | Spec 02 §8.8 rule 4, §8.1 |
| C5 | `PendingChange` *Apply* replays under the user's context; it has no binding hash, TTL or one-shot rule | Spec 02 §6.1; Spec 03 §16 |
| C6 | `dryRun: true` never mutates and takes precedence over draft-first | Spec 02 §8.1 |
| C7 | `x-secret` positions are redacted by a schema walk that only knows current schemas | Spec 02 §3, §8.1 |
| C8 | Error codes are additive; `details.reason` strings are contract text, not new codes | Spec 02 §8.5 |
| C9 | Read errors precede revision checks; callback panics are caught per callback; worker admission precedes execution | `documents.rs`, `thread.rs` |
| C10 | No public approval surface exists; "in-app approval" is unspecified beyond D-030 and Spec 03 §14.4 | verified above |
| C11 | Approval state is auth-layer state in the context, never storage-worker state | Spec 02 §8.8 rules 4, 7 |
| C12 | Rows rejected at load are invisible to typed reads; `CONFLICT` carries no current revision | Spec 03 §13.5, §16 |

## Decision (PROPOSED; normative once accepted)

### 1. Operations

All three are catalog operations in the reserved `system` slug (Spec 02 §4), beside `system.audit`, the read on
which Spec 03 §14.4 already builds the approval UI. The `auth` slug is not used: Spec 02 §8.1 prose lists
`auth.*` as a protocol-method family (see Alternatives). None is a protocol method; `lint-catalog` rules are
unchanged.

| Operation | Tier | Scope and rule | Expose rpc / subAction / mcp | Params → normal result |
|---|---|---|---|---|
| `system.list_approvals {}` | 0 | `admin` **and** the approver rule (§2) | ✓ / — / — | `{ approvals: Vec<PendingApproval> }`, bounded (§6) |
| `system.grant_approval { approval: ApprovalId }` | 1 | `admin` and the approver rule | ✓ / — / — | `{ granted: True, expiresAt: Timestamp }` |
| `system.deny_approval { approval: ApprovalId }` | 1 | `admin` and the approver rule | ✓ / — / — | `{ denied: True }` |

- The author param is named **`approval`**, never `approvalId`: the reserved name means "the presented approval of
  this call", is rejected in author params by `lint-catalog`, and neither operation is in any approval set, so
  codegen injects no `approvalId` on them. Both tier-1 operations receive the codegen `dryRun`; a dry run returns a
  `DryRunResult` whose single `PlanStep` names the operation and the handle and changes no state.
- `idempotent: false`, `external: false`, `highVolumeResult: false`. `mcp: false` is structural, not a first-scope
  choice: an MCP principal granting an approval would be the AI approving itself (D-030). `subAction: false`: a run
  never records consent. Granting is never itself approval-gated.
- `True` is the existing literal-true result type (`results.rs`), so a normal result is schema-distinguishable
  from a `DryRunResult`.
- Session grants ("approve for 15 min", Spec 02 §8.8 rule 4) are **out of scope**: this surface mints one-shot
  approvals only. A `sessionGrant` param and its duration are a later additive change contracted with spec 13; the
  one-shot surface is complete for every operation in the class of §4, which never accepts a session grant.

### 2. Approver rule (the concrete form of D-030 "in-app approval")

A grant, a deny, and a list of other requesters' records are accepted only when **all** of the following hold on
the caller's own server-built `AuthContext`:

1. `principal` is `Token { tokenKind: ui }`: never `api`, never `overlay`, never `Mcp`, `Plugin`, `Trigger` or
   `System`.
2. `surface` is `rpc` and `chain` is `None`: a direct call by the shell, not a derived context (a `batch.execute`
   entry from the same ui context is still direct).
3. `scopes` contain `admin`.

Otherwise `PERMISSION_DENIED`: with `details.required` when the scope is missing (existing shape), else with
`details.reason = "approver_required"` (PROPOSED reason string). The error carries no record content. A requester
learns its own approval's state by retrying the call (§5), which is the contracted protocol; non-approver
principals therefore never read the list (§7 lists the alternative).

**Ui-token issuance invariant** (normative text until spec 06 states it): a `ui`-kind token is created only by the
engine for the in-process desktop shell over the loopback transport it starts; it is never issued by pairing or by
any token-management operation (those issue `api` or `overlay`), never exported, listed with its secret or shown
in the UI, and is rotated on every engine start. `tokenKind` is copied from the server-side token record at
identify time; no client can set it. With this invariant "only a ui-kind token may grant" means "only the human at
the app may grant". The shell both requests (when the user clicks a confirmation-required action) and grants (when
the user confirms in the prompt); the rule prevents any non-shell principal from closing the loop alone. The
server cannot distinguish a human click from shell automation on any design; that limit is accepted and named.

### 3. Approval store: identity key, record, lifecycle, bounds, restart

**Store.** Process-local, in-memory, owned by the auth layer (`crates/den-core/src/auth/approval.rs`, a stub
today). Never persisted and never written to `config.db`: an approval does not survive an engine restart, which
also closes consent drift across code changes (schemas and migrations are compiled code and change only across
restarts). Every check and transition below is one atomic step under the store's single lock; the lock is never
held across a storage job or any await, so Minting steps (0) and (2) run outside it.

**Requester key.** `SessionId` when the requesting context has one (a WebSocket or MCP session), else the
`TokenId` (a REST request, whose context lives for one request). A record is redeemable only by a call presenting
the same key: an approval minted on one WebSocket connection cannot be redeemed on another, and a REST approval only
by the same token. `AuthContext.approvals` of a session is the store's view filtered by its key; for REST it is
populated per request from the store. A context with neither key (`Plugin`, `Trigger`, `System`, or any
non-`rpc`/non-`mcp` surface) cannot request an approval: a call that would need one fails `APPROVAL_REQUIRED
details.reason = "unattended"` (existing reason) and nothing is minted.
`AuthContext.approvals` lists each `granted` record of the key as `ApprovalBinding { id, operation, argsHash,
plan, sessionGrant: false, expiresAt }`, where `plan` is the server-computed `ApprovedPlan` of a composite call
(`actions.run`, `plugins.call`, a `batch.execute` entry) that the store keeps beside the record, never on the wire.

**Record** (the wire type `system.list_approvals` returns):

```rust
pub struct PendingApproval {
    pub approval_id: ApprovalId,
    pub operation: OperationName,
    pub args_hash: Sha256,            // binding hash of the requesting call, computed by the server
    pub requester: ApprovalRequester, // { principalKind, tokenKind?, tokenId?, sessionId?, client?: ClientInfo }
                                      // identity only; `client` is `x-den-untrusted: true` (§8)
    pub requested_at: Timestamp,
    pub expires_at: Timestamp,        // requested_at + APPROVAL_TTL_MS (120 s)
    pub state: ApprovalState,         // pending | granted | denied | consumed | expired (open string enum)
    pub plan: Vec<PlanStep>,          // the dry-run plan of the requesting call; params after the x-secret walk
    pub disclosure: Option<Value>,    // operation-specific, server-computed (§8); never client-supplied
}
```

**Minting.** The pipeline's approval step (after validation, scope and tier policy, before rate limit and audit,
Spec 02 §16) only classifies a call as approval-bearing: a member of §4 under every origin, or a tier-3 or
effective-tier-3 call whose origin principal is `Mcp` (rule 5 unchanged: `Token`, `Trigger` and `System` origins
of ordinary tier-3 calls stay exempt and never reach this section; a `Plugin` origin follows its permissions (spec 12)
and is not classified here); for such a call it refuses a keyless context (`unattended`, nothing minted) and discards
a `sessionGrant: true` binding presented to a member; it neither mints nor consumes. Minting and §5 are performed by
the engine's **generic dispatch** for every approval-bearing call, after rate limit and audit and before the
operation's handler body runs: no handler implements them, so none can omit them; an operation contributes only its
pre-mint check (step 2) and the work it submits. A `dryRun: true` call (C6) and a draft-first deferral (§4) never
reach Minting or §5. Without an `approvalId`, in this order: (0) outside the lock, the binding render and `argsHash`
of the call (Spec 02 §8.8); (1) under the lock, lookup of a non-terminal record under
`(requester key, operation, argsHash)`; a match returns the **same** `approvalId` and does no further work (no
duplicate prompts, no storage work for a repeated identical call); (2) outside the lock, the dry-run `plan` and, for a
composite call, the `ApprovedPlan` (Spec 02 §8.8 rule 4: server-computed, never from client-supplied data; it reads
stored actions), then the operation's own pre-mint checks, if any, which also yield its `disclosure` (the repair ADR
defines a preview read here), so nobody is asked to approve an impossible call; (3) under the lock again, repeat the
lookup, because a concurrent identical first request may have minted meanwhile (then: one record, the same id for both
callers, at most one plan and one pre-mint check per caller, no id before a check passed); else mint `ApprovalId`,
store the precomputed record as `pending` and release the lock; then publish `system.approval_pending` (§9) and return
`APPROVAL_REQUIRED { details.approvalId }`.

**Transitions.** `pending → granted` and `pending → denied` by an approver (§2); `granted → denied` by an
approver before consumption (revocation of a mistaken grant inside the TTL; §7 lists the alternative);
`granted → consumed` by the one-shot compare-and-swap of §5, performed by the generic dispatch (§3 Minting);
`pending | granted → expired` when `now ≥ expiresAt`, evaluated inside the lock at every access and by a sweeper.
`denied`, `consumed` and `expired` are terminal. Every transition publishes `system.approval_resolved` (§9) and
grants and denials write an audit row (§10).

**Bounds.** At most N non-terminal (`pending` or `granted`) records per requester key and M non-terminal records
in total; when a bound is reached the oldest non-terminal record is expired first. Terminal records are retained
for one TTL so the UI can show the outcome, capped at M as well, oldest dropped first. PROPOSED values N = 8 and
M = 256 are engine bounds for the owner's acceptance (§7), not product policy.
On every transition to a terminal state a record drops `plan` and `disclosure`; the UI needs only the outcome.

**Restart.** The store starts empty; a retry presenting a pre-restart id is `binding_mismatch` / `expired`.

### 4. Confirmation-required operation class

- `den-protocol` gains the constant `CONFIRMATION_REQUIRED_OPERATIONS: &[&str]`, published in `protocol.json` as
  `confirmationRequiredOperations`, exactly like `dynamicTierOperations`; it is a constant, not an `OperationDef`
  field, so a plugin cannot declare membership. **This ADR adds no member**: the set starts empty, and each member
  is added by the ADR that defines the operation (the repair ADR adds its repair operation).
- Codegen: `reserved_params_for` also injects `approvalId` when the set contains the operation; MCP annotates a
  member like tier 3 (`destructiveHint: true`, `requiresUserInteraction`) if it is ever MCP-exposed; the member's
  tier is unchanged (a tier-2 member stays a configuration operation, C1), and `tests/codegen_rules.rs` asserts the
  constant is published, `approvalId` is on every member's invocation schema and absent from its author params.
- Engine: for a member, an approval is required for **every** origin principal (this overrides the rule-5
  exemption of `Token`, `Trigger` and `System` for members only); a `sessionGrant: true` binding never satisfies a
  member, and a call that presents one behaves as if no approval were presented (§5 step 1 applies).
- `PlanStep.argsHash` and `PlanStep.dynamicFields` (Spec 02 §8.1: "on tier ≥ 3 steps") are also present on the
  step of a member, so a dry run shows the hash the approval will bind to. For a member called directly the
  binding render is the identity (direct calls never carry templates, Spec 02 §10.1), `dynamicFields` is empty and
  the reserved params are `x-den-template: false` as codegen already emits.
- Under an `Mcp` origin the tier-2 draft-first policy still runs **before** the approval step (pipeline order), so
  a member with `expose.mcp: true` defers into a `PendingChange` and mints nothing; *Apply* replays under the
  user's context **without** an `approvalId`, so the replay reaches §3 Minting and the human's in-app grant is the
  only way to proceed. *Apply* is never a bypass of this class.

### 5. Retry, ordered checks, one-shot consumption, admission

On a call that presents `approvalId`, the whole pipeline runs again (schema; scope and `maxTier` against the
**current** token record: a revoked token is `UNAUTHENTICATED` at identify, a removed scope is `PERMISSION_DENIED`; an
approval never grants capability), then the generic dispatch (§3 Minting) checks, in this order and all before the
operation's work is submitted:

1. The pipeline result above.
2. Lookup under the caller's requester key: (a) no record → `binding_mismatch` / `expired`; (b) `operation` or
   `argsHash` differs → `binding_mismatch` / `args`; (c) `now ≥ expiresAt` → `expired`; (d) state `pending` →
   `APPROVAL_REQUIRED` with the **same** `approvalId` (wait); (e) state `granted` → one-shot compare-and-swap
   `granted → consumed` inside the lock; (f) any other state → `expired`. Unknown, foreign (another key), denied
   and consumed ids are indistinguishable from expired on the wire: no oracle.
   For a composite call the plan is recomputed outside the lock before this step and compared under it in (b): a
   difference is `binding_mismatch` / `args`, `revision` or `unplanned_call` exactly as Spec 02 §8.8 rule 4 (a)
   assigns (`args` for a changed `dynamicFields` set or planned-call hash); rule 4 (b) and (c) keep enforcing the plan
   at execution.
3. Only the CAS winner submits the operation's work (a storage job, the settings writer request, or whatever the
   operation uses). Exactly one caller can win; every later presentation of the id is a mismatch.

**Consumed stays consumed.** If the work is then cancelled before admission, rejected at admission
(`INVALID_STATE` / `shutting_down`), fails before commit, or the caller disconnects, the approval remains
`consumed`; the caller re-requests and re-confirms. There is no refund and no un-consume path across threads: one
extra confirmation in that rare case is the price of never having two writers under one consent. The storage
worker never owns approval state (C11); authorization is checked in the pipeline and at the CAS, not again inside
the job, exactly as for every ordinary write. This ADR makes no claim about containment of panics inside an
operation's job; the accepted worker contract stands (a job panic is terminal for the worker, `thread.rs`), and the
approval is consumed in that case too. How an operation classifies contained callback panics is that operation's
ADR to state without contradicting this section.

### 6. Rate limits and dedupe

Grant, deny and list run under the ordinary pipeline rate limit of their caller. A request that needs an approval
is rate-limited by its own operation's pipeline before any mint; the dedupe lookup of §3 precedes every
operation-specific pre-mint check, so a repeated identical call does no storage work and mints nothing new; the
per-key bound N caps prompts a single requester can hold open. A request repeated after a denial mints a new record
(the denied one is terminal) only within the bounds.

### 7. Decisions the owner accepts explicitly with this text (options; nothing is presumed)

| # | Decision | Recommendation | Alternatives |
|---|---|---|---|
| O1 | Approver rule: ui-kind token, `rpc`, no chain (*policy flavour, D-030*) | as §2 | any `admin` principal (rejected: self-confirmation by an `api` token); ui or api with a dedicated scope |
| O2 | Scope for grant/deny/list | `admin` (no new constant) | new `approvals:grant` in `KNOWN_SCOPES` and Spec 02 §8.7, assignable only to ui-kind tokens |
| O3 | Requester key | `SessionId` when present, else `TokenId` | always `TokenId` (looser: any connection of the token redeems) |
| O4 | TTL for grant and retry | `APPROVAL_TTL_MS` (120 s) for both | a separate longer `CONFIRMATION_TTL_MS` for members of §4 (widens the consent-drift window) |
| O5 | Bounds N per key / M total, non-terminal plus one-TTL terminal retention | 8 / 256 | other values; no bound (rejected: unbounded memory) |
| O6 | List visibility | approver rule only (§2) | non-approvers get their own-key records with `plan` and `disclosure` omitted (second visibility shape) |
| O7 | Revocation of a `granted` record by an approver before consumption | allowed (`granted → denied`) | `pending` only (a mistaken grant is irrevocable for up to 120 s) |
| O8 | Events (§9) | both normative, `critical`, minimal payloads | `system.approval_pending` only; non-`critical` (droppable) events; payloads carrying `operation` and `requesterKind` |
| O9 | Mismatch value for unknown / foreign / denied / consumed ids | uniform `expired` (no oracle) | additive mismatch value `unknown` (more precise, enum growth) |
| O10 | Disclosure provenance invariant (§8) | closed trust state only; no raw stored strings | bounded raw string (regex, ≤ 64 chars); rejected here because a bound does not make a stored string secret-safe |
| O11 | Slug for the three operations | `system` | `auth` plus an amendment of the Spec 02 §8.1 protocol-method prose; a new reserved `approvals` slug |

Owner B (Q-E16) is a premise of this ADR and is **not** one of these decisions.

### 8. What the approver sees: privacy invariants

- `plan[].params` are the requesting call's own submitted params after the `x-secret` schema walk (`"***"` at
  secret positions); the binding hash covers the real values. This is inherited from Spec 02 §8.8 for every
  approval.
- A `disclosure` is computed by the server from record metadata and closed enumerations. It MUST NOT contain
  values or key names of a stored body, free-text messages derived from stored content (migration, validator or
  SQLite text), panic payloads, hashes of any of these (a hash of a low-entropy name is a dictionary oracle), or any
  string decoded from a stored body that has not passed the current whole-document schema. Provenance of such a
  body is disclosed as a **closed trust state only**: the `Origin` kind tag and its `trusted` flag, never the raw
  `from` text. Counts, booleans, closed reason enums, timestamps and typed identifiers taken from record metadata
  (row id, token id, session id) are permitted; no identifier decoded from the body is.
- Records of other requesters are visible only to an approver context (§2); errors never carry record content;
  the events of §9 carry no params, hash or disclosure; audit rows (§10) carry identifiers and kinds only.
- `ApprovalRequester` carries identity (`principalKind`, `tokenKind`, `tokenId`, `sessionId`, `client`), never a
  token secret; `client` is the requester's self-description (Spec 02 §8.3), marked `x-den-untrusted: true`.

### 9. Events (no polling)

AGENTS.md forbids polling where events exist, and a shell must learn about approvals requested by other
principals (MCP tier 3, REST third parties) without polling `system.list_approvals`. Two engine-internal events
(Spec 03 §5.1 internal lane), registered `critical: true`, `highVolume: false`, `payloadVersion: 1` with one example
each (Spec 02 §5.2), published from task context after the store's lock is released, not dropped by the slow-consumer
policy while the session drains (Spec 02 §8.4; Spec 03 §5.3), and replayed on reconnect (Spec 02 §8.4); a session
closed with 4006 or a stalled subscriber recovers by the resync rule below:

| Event | Payload | When |
|---|---|---|
| `system.approval_pending` | `{ approvalId, expiresAt }` | a record is minted |
| `system.approval_resolved` | `{ approvalId, state }` | any transition out of `pending` or `granted` |

Payloads are minimal on purpose (O8): a subscriber with `events:read` learns an opaque handle and a time, which it
can neither redeem (requester key) nor grant (approver rule); the approver reads everything else from the list.
A shell reconciles with `system.list_approvals` on (re)identify and on a replay `gap: true` (Spec 02 §8.4); that
is event-driven resync, never polling, and the list stays the on-demand truth.

### 10. Errors (existing codes only; PROPOSED additions are reason strings)

| Situation | Code | `details` | Status |
|---|---|---|---|
| Grant/deny/list by a non-ui principal, non-`rpc` surface or derived context | `PERMISSION_DENIED` | `reason = "approver_required"` | PROPOSED reason |
| Scope missing on any of the three | `PERMISSION_DENIED` | `required` | existing |
| Grant/deny of an id that was swept or never existed | `NOT_FOUND` | — | existing |
| Grant of a retained record that is not `pending`; deny of a retained terminal record | `INVALID_STATE` | `reason = "not_pending"` | PROPOSED reason |
| Request needing approval, no `approvalId`; or id presented while `pending` | `APPROVAL_REQUIRED` | `approvalId` (new or the same) | existing |
| Id unknown to this key, foreign, denied, consumed, past `expiresAt`, or pre-restart | `APPROVAL_REQUIRED` | `reason = "binding_mismatch"`, `mismatch = "expired"` | existing values, PROPOSED uniform rule (O9) |
| Id known but `operation` or `argsHash` differs | `APPROVAL_REQUIRED` | `binding_mismatch`, `mismatch = "args"` | existing |
| Composite call whose recomputed plan differs from the kept `ApprovedPlan` | `APPROVAL_REQUIRED` | `binding_mismatch`, `mismatch = "args"`, `"revision"` or `"unplanned_call"` per Spec 02 §8.8 rule 4 (a) | existing |
| Member of §4 called by a context without a requester key | `APPROVAL_REQUIRED` | `reason = "unattended"` | existing value, PROPOSED rule |
| `sessionGrant: true` binding presented for a member of §4 | `APPROVAL_REQUIRED` | `approvalId` (one-shot) | existing shape, PROPOSED rule |
| Work rejected at admission after the CAS | `INVALID_STATE` | `reason = "shutting_down"` | existing |

No new `ErrorCode`, no new `fieldErrors` shape. Reason strings land in Spec 02 §8.5 text and the `ErrorCode`
description constants in the same contract card, never ad hoc in a handler. Audit kinds `approval_granted` and
`approval_denied` are added to the Spec 03 §14.4 list; mismatches use the existing `binding_mismatch` kind.

### 11. Exposure and generation

`protocol.json` gains the three operations, `PendingApproval`, `ApprovalRequester`, `ApprovalState`, the grant and
deny param and result types, `confirmationRequiredOperations: []`, and the §9 events (`payloadVersion: 1`,
`highVolume: false`, `critical: true`, one example each); TypeScript and Python
bindings and OpenAPI regenerate; no MCP tool is generated (`mcp: false`). Fixtures: one per `ApprovalState`, one
per operation example (normal and dry run for the tier-1 pair). `lint-catalog` keeps rejecting `approvalId` and
`dryRun` in author params. The UI consumes the generated schema; no UI-only endpoint exists anywhere in this design.

## Consequences

- **Positive.** One public, schema-generated surface serves one-shot MCP tier-3 approvals, the `unattended` flow
  and every future confirmation-required operation; self-confirmation by a non-shell principal is impossible by
  rule, not by convention; consumption is exactly-once by a single CAS; restarts and code changes invalidate
  consent; the approver never sees stored secrets or unvalidated stored strings; nothing requires a new error code.
- **Negative / accepted limits.** A pure REST third party cannot complete a confirmation-required call without the
  human at the app and may time out repeatedly at 120 s (O4). A ui-token issuance invariant is contract text until
  spec 06 exists; code does not yet enforce it anywhere. The shell both requests and grants under one token; the
  server cannot tell a human click from automation. A consumed approval is never refunded. Approvals requested by
  contexts without a requester key fail `unattended`.
- **Dependencies and follow-ups.** The store, router and every handler are stubs; implementation needs a separate
  contract card for public types (den-protocol single-writer rule), then engine units, each ≤ 400 formatted manual
  lines including all tests, each with qualified Claude review and mandatory Grok QA, canonical checks, native
  Linux and Windows, current CI and an exact tested merge. Spec 02 §8.1, §8.5, §8.7, §8.8 and Spec 03 §14.4, §15
  gain the clauses above only after acceptance. Spec 06 and spec 13 inherit §2, §3, §8 and §9. The repair ADR
  depends on this one and must restate, not cite, every clause it relies on.
- **Nothing here authorizes code.** No public type, handler, store, fixture or test exists or is approved by this
  draft; acceptance of this text is a prerequisite, not an implementation go.

## Alternatives considered

| # | Alternative | Why not |
|---|---|---|
| A1 | `confirm: true` or a "repair" flag on an ordinary update | not evidence of informed confirmation; no binding, TTL, principal or one-shot rule; forbidden by owner B's "ordinary updates keep failing" |
| A2 | Classify the operation tier 3 so codegen adds `approvalId` | rule 5 still exempts `Token` callers, so the user's own UI would never be asked; misclassifies a document write as external/irreversible (C1) |
| A3 | `PendingChange` *Apply* as the confirmation | *Apply* is unspecified (no spec 06/13), has no hash, TTL or one-shot rule, and the settings precedent omits `revision` (C5) |
| A4 | Ad-hoc confirmation or preview token param | duplicates §8.8 without its guarantees; needs a new reserved name and new error semantics |
| A5 | Tier 4 | code execution / end stream; never exposed over MCP; no confirmation semantics |
| A6 | Tier-2 operation plus a constant set (chosen), **without** a public approval surface | nobody could record the binding through the public API (C10); a private shell hook violates AGENTS.md |
| A7 | Storage worker owns approval state | violates C11; makes "transaction open" and "consumed" a cross-thread race |
| A8 | Any `admin` principal may grant | self-confirmation: an `api` token confirms its own call with a second call (A1 with extra steps) |
| A9 | Persist approvals in `config.db` | consent survives restarts and code changes, the drift case to avoid; a document write for runtime state |
| A10 | Names under `auth.*` | Spec 02 §8.1 prose lists `auth.*` as a protocol-method family (the lint constant `PROTOCOL_METHODS` names only `auth.request_pairing`, and `system.*` already mixes both kinds); `system` avoids a prose amendment (O11) |

## Acceptance tests (names PROPOSED; none exists; nothing was run)

Approval store and handler (integration tests once the router and store exist; Linux and Windows):

| # | Case | Expected |
|---|---|---|
| T1 | ui token, `rpc`, `admin` grants a `pending` id | `granted`; `expiresAt` unchanged; audit `approval_granted`; `system.approval_resolved { state: granted }` |
| T2 | `api` token with `admin`; `overlay`; `Plugin`; `Trigger`; `System`; ui token with `chain: Some` or `surface: sub_action` | grant, deny and list → `PERMISSION_DENIED approver_required`; record unchanged; no record content in the error |
| T3 | `Mcp` principal calls any of the three | `NOT_FOUND` (not MCP-exposed), never a grant |
| T4 | params carrying `tokenKind`, `principal`, `approvals` or `approvalId` | `INVALID_PARAMS`; `AuthContext` unchanged |
| T5 | grant of an unknown or swept id; grant of a retained `granted`/`denied`/`consumed`/`expired` id | `NOT_FOUND`; `INVALID_STATE not_pending` |
| T6 | deny of a `granted` id before consumption (O7); then retry by requester | `denied`; `binding_mismatch expired`; a new request mints a new record |
| T7 | identical retry with a granted id; then replay | CAS wins, work submitted, `consumed`; replay → `binding_mismatch expired`, no second submission |
| T8 | retry with changed params (hash differs), including while `pending` | `binding_mismatch args`; record state unchanged (hash check precedes the CAS and the wait) |
| T9 | retry while `pending` | `APPROVAL_REQUIRED` with the same id; no mint, no consume |
| T10 | past TTL; expiry race between grant and sweeper at the boundary | `expired`; exactly one outcome under the lock |
| T11 | id minted on session S1 retried on S2 with the same token; REST id of token A retried by token B | `binding_mismatch expired`; no capability inherited |
| T12 | scope removed or token revoked between grant and retry | `PERMISSION_DENIED` / `UNAUTHENTICATED` before the lookup; approval untouched |
| T13 | `sessionGrant: true` binding presented for a member of §4 | ignored; `APPROVAL_REQUIRED` with a one-shot id; nothing submitted |
| T14 | two concurrent identical retries with one granted id | exactly one CAS winner and exactly one `binding_mismatch expired`; never two submissions |
| T15 | shutdown or cancellation after the CAS and before admission | `INVALID_STATE shutting_down` or cancelled; approval `consumed`; nothing durable |
| T16 | dedupe: repeated identical request while `pending` or `granted`; two concurrent identical first requests | same id; the pre-mint check runs zero times for the repeat and at most once per caller in the race; one record (counter asserts) |
| T17 | bounds: N+1 non-terminal per key; M+1 total counting `granted`; terminal retention beyond M | oldest expired first; list bounded; no unbounded growth |
| T18 | engine restart with pending and granted records | store empty; retry → `binding_mismatch expired` |
| T19 | member called by `Trigger`, `System` or `Plugin` context | `APPROVAL_REQUIRED unattended`; nothing minted |
| T20 | `dryRun: true` on grant and deny | `DryRunResult`, one `PlanStep`, no transition, no audit row, no event |
| T21 | serialized `PendingApproval`, events and audit rows for a request whose params hold `x-secret` values and whose disclosure covers a body with secret-looking keys and strings | `"***"` at secret positions; no key name, stored value, message or raw `from` string anywhere (substring asserts) |
| T22 | composite call approved, then a reachable action's revision or the planned call multiset changes before the retry | `binding_mismatch revision` / `unplanned_call` / `args` for a changed planned-call hash or `dynamicFields` set; approval untouched; nothing submitted |
| T23 | subscriber reconnects with `since` after a mint and a resolution; then with a cursor older than the buffer | both events replayed with `meta.replay: true`; on `gap: true` the list shows the same state; no polling |

Protocol (`den-protocol`): fixtures per `ApprovalState` and per example round-trip; `codegen_rules`: the three
operations carry `dryRun` (tier 1) and no `approvalId`, name the handle `approval`; `confirmationRequiredOperations`
is published; every member has `approvalId` on its invocation schema and not in its author params; no MCP tool is
generated for the three; the secret sentinel test is unchanged. Native both: every named test plus the canonical
workspace, web, codegen `--check`, hooks and current CI on Linux and Windows before any merge (existing gates).
