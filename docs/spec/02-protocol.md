# Spec 02: Protocol and contracts (WS-02)

- **Status:** Draft, revision 2 (resolves the Codex Astra review findings 1–6, see §19). Reviewer: Codex
  (Astra). Approval: owner. Card: DEN-14.
- **Owns:** `crates/den-protocol` (single-writer), the codegen pipeline (`cargo xtask codegen`), and every generated
  artifact (`protocol.json`, OpenAPI, AsyncAPI, TS, Python, plugin manifest schema, MCP tool catalog).
- **Builds on:** [architecture.md](../architecture.md) §2–4, [plugin-sdk-and-api.md](../research/plugin-sdk-and-api.md)
  B1–B3, [mcp-and-ai-providers.md](../research/mcp-and-ai-providers.md) §1.4–1.9,
  [streamerbot-engine-ui.md](../research/streamerbot-engine-ui.md) §2, §4, §7, §11, spike S-3 (stdio framing),
  decisions D-006, D-016, D-020, D-027, D-028, D-029, D-030.
- **Feature-parity rows:** 6.03, 7.02, 7.14, 22.02, 22.17 (schema side), 22.18 (schema side).
- **Normative words:** MUST, SHOULD and MAY are used as in RFC 2119.

## 1. Scope and non-goals

**In scope:** the wire and storage shapes every other subsystem builds against: the event envelope and naming;
the domain types (Action, Trigger, Sub-action, Variable, Queue); the ID scheme; versioning and migrations; the
operation catalog; the authorization context every call and run carries; the WebSocket JSON-RPC envelope,
handshake, subscriptions and errors; stdio framing for workers and sidecars; plugin manifest schema v0; template
syntax; codegen and its CI checks.

**Non-goals (owned elsewhere):** engine semantics such as trigger precedence, queue scheduling, and run and
history behaviour (spec 03). Endpoints, ports, the pairing UI, REST resource design and the MCP server runtime
(spec 06). Per-platform payload catalogs (spec 05, 11). Plugin host behaviour and the registry (spec 12).
Designer document schema (spec 09; it reuses §7 versioning). The Streamer.bot-compatible WS shim (spec 15; it
translates to and from this protocol and never extends it).

## 2. User stories

1. As a **UI developer**, I import generated TS types and a typed client, so the UI can't call something a third
   party can't (D-006, "the UI is just another API client").
2. As a **plugin author**, I write `plugin.json` against a published JSON Schema with editor completion, and my
   events and operations appear in the same pickers, API explorer and MCP catalog as built-ins.
3. As an **overlay or deck author**, I connect over WS, identify with a token, subscribe to `twitch.*`, and after a
   reconnect I replay what I missed.
4. As an **AI client** (Claude Code, Codex…), I get well-named tools with valid input schemas and honest risk
   annotations.
5. As a **streamer migrating from Streamer.bot**, my imported actions keep their GUIDs, their queues and their
   `%var%` templates (converted to `{…}`, D-027).
6. As a **maintainer**, CI tells me when a change breaks clients, before release.

## 3. Conventions (apply to every type)

| Rule | Value |
|---|---|
| Rust derives | Every public type derives `Serialize, Deserialize, JsonSchema, TS` (ts-rs, §9.2) and has a doc comment on the type and on every field. Doc comments become schema `description`s. |
| Field names | `camelCase` on the wire (`#[serde(rename_all = "camelCase")]`). |
| Enum values | `snake_case` strings. **Open enums** (platforms, tiers of other services) are strings with documented known values, never closed Rust enums, so new values don't break old clients. |
| Tagged unions | Internally tagged: `kind` for domain unions, `type` only for the event type. No `untagged` unions on the wire (they make schemas ambiguous), with the single exception in the Unions row. `#[serde(flatten)]` inside a struct variant (`Node::Op`, §6.4) is covered by the round-trip and ts-rs tests in §17.1. |
| Identifiers in namespaces | Dotted, `lower_snake_case` segments, `^[a-z][a-z0-9_]*(\.[a-z][a-z0-9_]*)+$`: event types `twitch.channel_point_redemption`, operations `obs.set_scene`, methods `actions.run`. |
| Time | RFC 3339 UTC with milliseconds and `Z` (`2026-10-18T20:15:03.120Z`). Durations are integer milliseconds in fields ending `Ms`. |
| Money | `Money { amountMinor: i64, currency: String /* ISO 4217 or a platform unit like "bits" */, exponent: u8 }`. Floats are never used for money. The run context also exposes a derived `amount` number for templates (§6.3). |
| Unknown fields | Wire types **ignore** unknown fields (forward compatibility). Manifests and persisted documents validate strictly, except keys starting with `x-`. Strictness is enforced by the JSON Schema validator, not serde's `deny_unknown_fields`, which doesn't work with `flatten`. Types using `#[serde(flatten)]` (e.g. `SubAction`, §6.4) get `unevaluatedProperties: false` (2020-12) instead of `additionalProperties: false` per branch, via a schemars transform; a test validates every fixture against it. |
| Unions | No `untagged` unions, with one exception: `TemplateOr<T>` (§6.4, §10.1) is the Rust form of a value-mode template field. Its schema is `{ "anyOf": [T, { "type": "string", "x-den-template": "value" }] }`, never at a root, decoded by a hand-written deserializer (a string that is exactly one `{…}` form → template, anything else → `T`). The same derivation produces every operation's authoring schema (§10.1). |
| Optional vs null | Absent means "not provided". `null` is only used where a value can be explicitly cleared; the schema says so. |
| Sizes | One WS message ≤ 1 MiB by default (configurable, spec 06). Max JSON nesting depth 32 for anything coming from plugins or clients. |
| Untrusted text | Fields that carry viewer- or third-party-controlled text (chat text, usernames, titles, plugin outputs) are marked `x-den-untrusted: true`. MCP and the AI layer wrap these values (§9.6). |
| Secrets | Fields marked `x-secret: true` are never emitted in events, history snapshots, logs, exports or `protocol.json` examples. They are redacted to `"***"`. |

## 4. IDs

| Thing | Format | Notes |
|---|---|---|
| Entity IDs (action, trigger, sub-action node, queue, command, timer, connection, overlay) | UUID, lowercase hyphenated string. New IDs are **UUIDv7** (time-ordered, good SQLite index locality). | Any UUID version is accepted, so imported Streamer.bot GUIDs (v4) keep their identity (user story 5). Rust newtypes per kind: `ActionId(Uuid)`, `QueueId(Uuid)`… |
| Default queue | The nil UUID `00000000-0000-0000-0000-000000000000` | Matches Streamer.bot's export convention, so import is a straight mapping. |
| Event IDs, run IDs | UUIDv7 | Monotonic per process, used for replay cursors (§8.4) and history. |
| Plugin ID | Reverse-DNS, `^[a-z0-9]+(\.[a-z0-9-]+)+$`, ≤ 128 chars (`io.github.alice.acmelive`) | Namespace verification is the registry's job (spec 12, Q-K3). |
| Slug | `^[a-z][a-z0-9_]{1,23}$`, unique per installation | Prefix for every event type, operation and variable namespace the plugin contributes. Reserved for core: `core`, `system`, `session`, `auth`, `actions`, `triggers`, `queues`, `variables`, `events`, `chat`, `tts`, `audio`, `plugins`, `obs`, `twitch`, `youtube`, `kick`, `x`, `bluesky`, `discord`, `vts`, `vmc`, `mcp`, `ai`, `code`, `ui`, `overlay`, `deck`, `util`, `batch`, `global`, `globals`, `user`, `event`, `run`, `trigger`, `action`. |
| Contribution IDs | `<slug>.<localId>` (`acme.gift`, `acme.send_chat`) | `localId` is `lower_snake_case`. Persisted references to a contribution are the shared type `ContributionRef { type, plugin? }` (§5.3), flattened into `Trigger` (§6.2) and `Node::Op` (§6.4): `plugin` (the plugin ID) is **required** when the slug is not reserved for core and **forbidden** when it is. The derive emits `plugin` as optional; codegen adds a generated `if/then/else` clause from the reserved-slug list to every schema that embeds a `ContributionRef` (`if type matches ^(core|system|…)\. then plugin is not allowed, else plugin is required`), and the save-time and import validators enforce the same rule in code, so the pair is part of the generated types and survives strict validation and round-trips. Installing a plugin whose slug is already used by a different plugin ID is refused locally (the registry also keeps slugs unique, spec 12). Import re-binds by `plugin`: if that plugin is installed but its current manifest uses a different slug (the author renamed it in a later version), `type` is rewritten to the installed slug; if it is not installed, an `Op` becomes `placeholder` and a `Trigger` is kept verbatim but `unresolved` (§6.2) until the plugin is installed; a non-reserved `type` without `plugin` cannot be re-bound safely and becomes `placeholder` with an import-report entry. |
| API tokens | `sdt_` + 43 chars base64url (256 random bits) | The prefix lets secret scanners find leaked tokens. Stored as SHA-256 hashes and compared in constant time (spec 06). |
| Approval handles | `apr_` + 22 chars base64url | MCP approval flow, 120 s TTL. |
| Session IDs | `ses_` + 22 chars base64url | Per WS connection. |

## 5. Events

### 5.1 Envelope

```rust
/// One occurrence from a source. Immutable once emitted.
pub struct Event {
    pub id: EventId,                 // UUIDv7
    pub r#type: EventType,           // "twitch.subscription"
    pub ts: Timestamp,               // when it happened upstream, or emission time if unknown
    pub source: SourceRef,           // { plugin: "core.twitch" | "<pluginId>", connection?: ConnectionId, account?: String }
    pub platform: Option<PlatformId>,// open enum: "twitch", "youtube", "kick", "x", "bluesky", or a plugin slug
    pub user: Option<UserRef>,       // the actor, normalized (shared type, §5.3)
    pub payload: serde_json::Value,  // typed per event type, see §5.2
    pub meta: EventMeta,
}
pub struct EventMeta {
    pub payload_version: u32,        // per-event-type schema version, starts at 1
    pub test: bool,                  // simulator or "Test trigger"
    pub replay: bool,                // re-delivered from the replay buffer or from history
    pub caused_by: Option<RunId>,    // set when emitted by an action run (loop protection, spec 03)
    pub correlation_id: Option<String>, // upstream message id, for dedupe and moderation links
}
```

The architecture's `{ id, source, type, ts, platform?, user?, payload }` is kept as is; `meta` is the only addition.

### 5.2 Naming and typed payloads

- **Type = `<slug>.<name>`**, past-tense or noun form for the thing that happened: `twitch.follow`,
  `twitch.subscription`, `twitch.gift_subscription`, `youtube.super_chat`, `obs.scene_changed`,
  `vts.hotkey_triggered`, `chat.message`, `chat.message_deleted`, `system.started`, `variables.global_updated`.
- **Cross-platform events live under neutral slugs** when the payload is unified: `chat.message` (spec 08) carries
  `platform` and is what multichat, overlays and TTS consume. Platform-specific events (`twitch.cheer`) carry the
  full detail. A connector MAY emit both for one upstream message, linked by `correlationId`.
- Each known event type has a Rust payload struct registered in the catalog with: title, description, category
  path, `payloadVersion`, payload schema, trigger filters (§6.2), `highVolume` and `critical` flags (§8.4), and at least one example
  (the example is also the "Test trigger" fixture). Example:

```rust
/// A new or renewed Twitch subscription (not a gift; see twitch.gift_subscription).
#[event(type = "twitch.subscription", category = "Twitch/Subscriptions", version = 1)]
pub struct TwitchSubscription {
    /// "1000" | "2000" | "3000" | "prime" (open enum).
    pub tier: String,
    /// Cumulative months, when Twitch shares it.
    pub cumulative_months: Option<u32>,
    pub streak_months: Option<u32>,
    /// Resub message with emote fragments. Untrusted.
    pub message: Option<ChatText>,
}
```

- **Unknown-event passthrough.** When a connector receives an upstream event it has no type for (for example, a
  new EventSub subscription type), it MUST emit `<slug>.unknown` with payload
  `{ upstreamType: String, upstreamVersion: Option<String>, data: Value }` instead of dropping it. Triggers can
  match `<slug>.unknown` with a filter on `upstreamType`, so users can act on new platform features before we
  ship types. When a type is added later, the connector stops emitting `unknown` for it (noted in the changelog).
- **Typing model (one model everywhere).** On the wire and in `Event`, `payload` is an opaque object; the
  envelope is not a discriminated union. Typing is a second step: the catalog maps each `type` to its payload
  schema, and the generated helpers parse on demand. In Rust that's `event.payload_as::<TwitchSubscription>()`
  and a `KnownEvent` enum built by a hand-written lookup on `type` (serde's derive can't put an unknown tag
  into a data variant), with fallback `KnownEvent::Other(Event)`. TS gets an `EventPayloadMap` keyed by
  type, plus type guards. Python gets a `parse_payload(event)` function that returns `None` for unknown types.
  `protocol.json` lists the envelope schema once and each payload schema under `events.<type>.payload`.
- **Unknown types at the receiver.** Every client MUST ignore event types and fields it doesn't know. The
  generated parsers return "unknown", never an error, for an unrecognised `type`.
- **Payload evolution.** Additive changes keep `payloadVersion`. A breaking change to a core event's payload bumps
  its `payloadVersion` **and** `rpcVersion` together: each `rpcVersion` fixes exactly one payload version per core
  event type, published in `protocol.json` as `events.<type>.payloadVersions: { "1": 1, "2": 2 }` with one
  payload schema per version. The session's negotiated `rpcVersion` (§8.3) is therefore the only negotiation knob:
  the engine emits, and replays (§8.4), the payload version that the session's `rpcVersion` maps to, through a
  registered downgrade function keyed `(type, from, to)`, for the deprecation window (§7.4), and
  `meta.payloadVersion` always states the shape actually delivered. Two sessions on `rpcVersion` 1 and 2 receiving
  the same upstream event get different shapes (§17.10). Per-event negotiation in `identify`/`subscribe` was
  rejected: it adds a second knob that generated SDKs, which are pinned to one `rpcVersion`, cannot use. Breaking
  payload changes SHOULD be batched into one `rpcVersion` bump. **Plugin events** are outside this table: a plugin
  declares `payloadVersion` per event in its manifest (§11), a breaking change there is a plugin major version
  bump, the host never downgrades plugin payloads, and clients consuming plugin events check
  `meta.payloadVersion`. Stored events (history, exports) are immutable and keep the version they were emitted
  with; readers use `meta.payloadVersion`, not §7.3 migrations.

### 5.3 Shared core types (referenced by every connector and plugin)

`UserRef { platform, id, login, displayName, avatarUrl?, color?, roles: Vec<String> /* broadcaster, moderator,
vip, subscriber, founder, member… open */, badges: Vec<Badge>, pronouns? }` ·
`ChatText { text, fragments: Vec<Fragment> }` where `Fragment` is `kind`-tagged `text | emote | mention | link |
cheermote` · `Money` · `ImageRef { url, width?, height?, scale? }` · `Duration` (ms) · `PlatformId` (open string) ·
`ContributionRef { type: String /* "twitch.cheer", "acme.gift" */, plugin: Option<PluginId> /* required for a
non-core slug, forbidden for a core slug (§4) */ }` · `RiskTier` (integer 0–4, §8.1) · `DryRunResult`, `PlanStep`
(§8.1) · `AuthContext`, `Principal`, `ApprovalBinding` (§8.8).
Plugin manifests reference them as `den://schemas/v0/<Name>.json` (resolved offline from the bundled catalog), so
`{user.displayName}` means the same thing everywhere.

## 6. Domain types

These are persisted documents (§7.3) and also travel over the API (`actions.get`, `actions.create`…). Engine
behaviour is in spec 03.

### 6.1 Action and Queue

```rust
pub struct Action {
    pub id: ActionId,
    pub revision: u64,                 // server-assigned, +1 on every write; approvals bind to it (§8.8)
    pub name: String,                  // unique per installation among non-draft actions, case-insensitively; actions run by id or name
    pub group: Option<String>,         // folder path "Alerts/Twitch"; organisation only
    pub enabled: bool,
    pub queue: QueueId,                // nil UUID = Default queue
    pub concurrent: bool,              // start all top-level nodes at once (Streamer.bot "Concurrent")
    pub random: bool,                  // run one weighted-random top-level node
    pub always_run: bool,              // runs even when its queue is paused
    pub history: HistoryPolicy,        // { record: bool, pending: bool }
    pub triggers: Vec<Trigger>,
    pub nodes: Vec<SubAction>,         // the sub-action tree, §6.4
    pub description: Option<String>,
    pub tags: Vec<String>,             // also used by scopes like actions:run:<tag>
    pub origin: Origin,                // user | imported { from: "streamerbot", trusted: false } | ai_draft { draftOf?: ActionId } | plugin_template
}
pub struct Queue {
    pub id: QueueId, pub name: String,
    pub blocking: bool,                // FIFO, one run at a time
    pub paused: bool,
    pub max_pending: Option<u32>,      // bound; None = engine default (spec 03)
    pub overflow: Overflow,            // reject_new | drop_oldest
}
```

`origin` carries the D-029 trust decision (imported or community code runs sandboxed) and the D-030 rule that
AI-created config is saved as a disabled draft (`ai_draft` actions MUST have `enabled: false` until the user
applies them). **`origin` is server-assigned and read-only on the wire**: it is output-only in the schema
(`readOnly: true`), ignored on `actions.create`/`actions.update` input, and set from the caller's identity (MCP
session → `ai_draft`, importer → `imported`, UI or user token → `user`). It changes only through in-app user
actions: *Apply* on an AI draft sets `user`; *Trust this action* on an imported one sets `imported { trusted:
true }` after the consent screen. Editing an imported action does not change its trust. The same rule applies to
`updatedBy` on variables (§6.3) and to `revision`, which the server bumps on every write.

**Draft-first under MCP (tier-2 policy, §8.1).** A tier-2 operation called under an MCP context never mutates a
live document. For **actions**: `actions.create` creates an `ai_draft` with `enabled: false`; `actions.update`
creates a new action `ai_draft { draftOf: <target> }` holding the proposed content (it keeps the target's name,
which is why uniqueness is checked among non-draft actions only) and leaves the target untouched. *Apply* in the
"Pending AI changes" panel replaces the target's content (bumping its `revision`), sets `origin: user`, and deletes
the draft; *Discard* deletes it. For **every other document kind** (queues, commands, overlays, settings) the
draft is a `PendingChange { id, kind, draftOf?: EntityId, operation, params, createdBy: Principal }` document
(§7.3, its own `schemaVersion`), which *Apply* replays under the user's context. Codegen adds the optional result
field `draft?: DraftRef { id, kind }` to every tier-2 operation, set when the call was executed draft-first; `draft`
is a reserved result key like `dryRun` (§8.1), so WS, REST and MCP results share one schema. Drafts never run (§8.8
rule 6).

### 6.2 Trigger

A trigger type **is** an event type. The catalog entry for the event declares which payload fields can be
filtered and with which operators, so the UI, MCP and plugins all get the same filter forms.

```rust
pub struct Trigger {
    pub id: TriggerId,
    #[serde(flatten)] pub contribution: ContributionRef, // on the wire: { "type": "twitch.cheer" } or { "type": "acme.gift", "plugin": "io.github.alice.acmelive" }
    pub enabled: bool,
    pub unresolved: bool,              // read-only, derived: `plugin` isn't installed; the trigger never fires until it is
    pub always_run: bool,              // fire even when a more specific trigger matched (Streamer.bot semantics)
    pub filters: Vec<Filter>,          // AND-ed
    pub config: serde_json::Value,     // type-specific settings (e.g. command text, timer interval), schema from the catalog
}
pub struct Filter {
    pub field: String,                 // dotted path into the payload: "amount", "user.roles"
    pub op: FilterOp,                  // eq | ne | in | not_in | gte | lte | between | contains | regex | exists
    pub value: serde_json::Value,      // validated against the field's schema; "between" takes [min, max]
}
```

The catalog entry for each filterable field lists its allowed `ops`, an `optionsFrom` source for pickers, and a
`specificity` rank, which spec 03 uses for "specific beats any" precedence.

### 6.3 Variables and the run context

| Scope | Key | Lifetime | Template path |
|---|---|---|---|
| **Run args** | name | one run (flows into inline `run_action` children and back) | `{name}` |
| **Global** | name + `persisted: bool` | app-wide; persisted survives restart, temp is cleared at shutdown | `{globals.name}` |
| **User** | `(platform, userId)` + name + `persisted` | per platform user (cross-platform identity in P5) | `{user.vars.name}` for the event's user |
| **Plugin** | `<slug>.<id>` | maintained by the plugin; read-only to others unless granted | `{plugins.<slug>.<id>}` |

- **Values are JSON values** (`null`, bool, number, string, array, object). Unlike Streamer.bot there is no
  flattening into `input0..n`; templates index arrays directly (`{words[0]}`). The importer generates flat aliases
  only where imported templates need them.
- **Names:** `^[A-Za-z_][A-Za-z0-9_]{0,63}$`, case-sensitive.
- **Run context root** (what `{…}` resolves against). Reserved root names, which always resolve to the
  engine's value: `event` (full envelope), `user` (the event's `UserRef` plus `vars`), `trigger` (`id`,
  `type`), `action` (`id`, `name`), `run` (`id`, `queuedAt`, `queue`, `isTest`, `principal`, `surface`, `maxTier`; §8.8),
  `globals`, `plugins`, `match`
  (last regex captures), `args` (the whole args map), and `now`. Other names resolve against the args map,
  which is seeded with the event's top-level payload fields at run start; args set by later nodes overwrite
  earlier values, including seeded payload fields (`event.payload` keeps the original). Codegen and manifest
  validation fail when an event payload declares a top-level field with a reserved name, and `Set argument`
  refuses reserved names.
- Stored variables carry `{ value, updatedAt, updatedBy: Origin }`. Change events: `variables.global_updated`,
  `variables.user_updated` (with `old` and `new`).

### 6.4 Sub-action tree

```rust
pub struct SubAction {
    pub id: NodeId,
    pub enabled: bool,
    pub weight: Option<f32>,           // used when the parent is random
    pub note: Option<String>,
    #[serde(flatten)] pub node: Node,  // internally tagged by "kind"
}
pub enum Node {                        // #[serde(tag = "kind", rename_all = "snake_case")]
    Op { #[serde(flatten)] contribution: ContributionRef, params: Value, save_as: Option<String> }, // any sub-action-exposed operation (§8.1); wire: { "kind": "op", "type": "acme.send_chat", "plugin": "io.github…", "params": {…} }
    Group { name: String, color: Option<String>, random: bool, children: Vec<SubAction> },
    If { condition: Condition, then: Vec<SubAction>, r#else: Vec<SubAction> },
    Switch { value: Template, auto_type: bool, cases: Vec<Case>, default: Vec<SubAction> },
    While { condition: Condition, max_iterations: Option<u32>, children: Vec<SubAction> },
    BreakLoop, ContinueLoop, Stop,     // Stop = Streamer.bot "Break": end the run
    Parallel { branches: Vec<Vec<SubAction>> },  // D-028; joins when all branches finish
    Delay { ms: TemplateOr<u64> },
    RunAction { action: ActionRef, mode: RunMode /* inline | queued */, args: Map<String, Template> },
    Code { lang: CodeLang /* js | python | csharp */, source: CodeSource /* inline | script_ref */, timeout_ms: Option<u32> },
    Placeholder { origin: String, original: Value }, // unknown imported type; never runs; kept verbatim (WS-15 rule)
}
pub struct Condition { pub left: Template, pub op: CompareOp, pub right: Template, pub auto_type: bool, pub ignore_case: bool }
```

`CompareOp` covers the Streamer.bot set (`eq`, `ne`, `contains`, `regex`, `lt`, `gt`, `lte`, `gte`, `not_exists`,
`is_null_or_empty`). Regex captures land in `match` (array plus named groups). Every `Op` node's `params` are
validated twice, against two generated schemas (§10.1): at save time against the operation's **authoring** schema
(which admits templates only at template-eligible fields), and at run time, after rendering, against the
**invocation** schema the operation actually executes with (plugins can change, and rendered values can violate
ranges and enums). `Template` fields are strings in the §10 syntax; `TemplateOr<T>` is a value-mode field (§10.1)
that accepts a literal `T` or one whole `{…}` form. Every node executes under the run's authorization context
(§8.8): a node whose tier or scopes exceed that context fails before it runs.

## 7. Versioning

### 7.1 Version numbers

| Version | Where | Bumps when |
|---|---|---|
| App version | SemVer, `hello.version` | Every release |
| **`protocolVersion`** | SemVer, top of `protocol.json` | Minor on any additive change; major together with `rpcVersion` |
| **`rpcVersion`** | Integer, negotiated in the handshake (§8.3), also the `/api/v<rpcVersion>` URL major | Only on a breaking change to any WS/REST/stdio method, event envelope, shared type or core event payload (§5.2) |
| `payloadVersion` | Per event type, in `meta` and in `protocol.json` | Breaking change to that one payload. For core events always together with an `rpcVersion` bump: each `rpcVersion` fixes one payload version per core event type (§5.2). Plugin events: a plugin major version bump |
| `pluginApi` | SemVer major.minor in manifests (`engines.pluginApi`) | Major on breaking host↔plugin changes; minor on additions |
| `manifestVersion` | Integer in `plugin.json` | Breaking manifest changes. v0 = pre-stable (§11) |
| `schemaVersion` | Integer per persisted document kind | Any change to the stored shape (§7.3) |

`rpcVersion` 1 is the first public version. Before v1.0 the protocol is unstable, and breaking changes still bump
`rpcVersion` so clients get a clear refusal instead of silent breakage.

**Where `rpcVersion` lives.** The `session.identify` value (WS) and the `/api/v<N>` path segment (REST) are
authoritative. The WS subprotocol name (`streamden.v1.json`) is the **encoding** version and does not follow
`rpcVersion`; it only changes if the framing or encoding does. On WS, a missing or unsupported `rpcVersion` closes
with 4002. On REST, an unsupported `/api/v<N>` returns 404 with `data.code = UNSUPPORTED_VERSION` and the
supported range. The server supports `[current - 1, current]` during a deprecation window (§7.4).

### 7.2 What counts as breaking

Checked mechanically by `cargo xtask schema-compat` against the `protocol.json` of the last release tag:
removing or renaming a method, event type, field or enum value; adding a required field to a request or
manifest; narrowing a type, range or pattern; changing a default; changing error codes for an existing
condition. Adding optional request fields, response fields, methods, event types or open-enum values is not
breaking. Adding a variant to a **closed** union that clients receive (`Node`, `Fragment`, `FilterOp`) **is**
breaking, unless the union is documented as extensible with a fallback (clients render an unknown `Node` as a
read-only placeholder and an unknown `Fragment` as its text). `Node` and `Fragment` are documented that way;
other closed enums are not. A breaking diff without an `rpcVersion` bump fails CI, and so does a core
`payloadVersion` bump without an `rpcVersion` bump (§5.2).

### 7.3 Persisted documents and migrations

- Every stored document (action, queue, settings, export bundle, designer document, plugin data envelope) is
  wrapped as `{ "kind": "action", "schemaVersion": N, "data": {...} }`.
- Migrations are pure Rust functions `fn(Value) -> Result<Value>` from N to N+1, registered per kind and chained
  on load. Each step has before and after golden fixtures. Migrations are never lossy: fields a step can't map go
  to `data.x-migrated` and a warning is logged.
- Loading a document with a **newer** `schemaVersion` than the app knows opens it read-only with a clear message
  (downgrade protection). The app never rewrites it.
- The database keeps a backup copy before running a migration batch (spec 03 storage).

### 7.4 Deprecation

Deprecated items carry `deprecated: { since, removeIn, replacement }` in `protocol.json`. They stay for **at
least two minor releases and 6 months**. Using one produces a `system.deprecation_used` event for that session
(once per method per session) and a log line. The engine serves the previous `rpcVersion` for the same window.

## 8. Operations and the wire protocol

### 8.1 Operation catalog

Every callable thing is an **operation** registered once in the catalog, and exposed on up to three surfaces
from that one definition:

```rust
pub struct OperationDef {
    pub name: OperationName,          // "obs.set_scene", "actions.run", "acme.send_chat"
    pub title: L10nString, pub description: L10nString, pub category: Vec<String>,
    pub params: Schema, pub result: Schema,     // JSON Schema 2020-12; params root MUST be `type: object` (§9.6); result root MUST be `type: object` for tier ≥ 1 (dry-run discrimination, below)
    pub tier: RiskTier,               // 0 read · 1 reversible write · 2 config change · 3 external/irreversible · 4 dangerous
    pub scopes: Vec<Scope>,           // e.g. ["obs:control"]
    pub expose: Exposure,             // { rpc: bool, sub_action: bool, mcp: bool }
    pub idempotent: bool,
    pub external: bool,               // talks to an external platform/app (MCP openWorldHint)
    pub timeout_ms: Option<u32>,
    pub platforms: Option<Vec<Os>>,   // OS gate: ["windows"] for Windows-only apps; hidden elsewhere with a reason
    pub high_volume_result: bool,
    pub since: String, pub deprecated: Option<Deprecation>,
    pub examples: Vec<OperationExample>,  // at least one; CI validates every example against the schemas
}
```

- `chat.send` is one operation: a WS method, a sub-action type and an MCP tool. Two kinds of callable are
  distinguished: **protocol methods** (`hello`, `session.*`, `auth.*`, `system.ping`, `events.subscribe`,
  `events.unsubscribe`, `batch.execute`; §12) are **not** operations: they live outside the catalog, have no tier,
  and are never sub-actions or MCP tools. **Configuration operations**, which write persisted documents (§7.3:
  `actions.create/update/delete`, `queues.create/update/delete`, commands, overlays, settings), are ordinary catalog
  operations, tier 2, `expose: { rpc: true, subAction: <per op>, mcp: true }`; over MCP they follow the tier-2
  draft-first policy (§6.1), which is how the D-030 draft workflow gets its tools (`actions_create`,
  `actions_update` in the Lean profile, §9.6). Writes to runtime state that are not documents (`variables.set`,
  `queues.pause`) are tier 1. Control-flow nodes are not operations.
- **Reserved, codegen-added params.** `dryRun: bool` on every tier ≥ 1 operation (below), and `approvalId?:
  ApprovalId` on every operation that can require approval: tier 3, plus the operations whose effective tier is
  dynamic (`actions.run`, `plugins.call`, §8.8). A client that received `APPROVAL_REQUIRED` repeats the identical
  call with `approvalId` set; both names are rejected in author-defined params and in results by `lint-catalog`.
- **Dry run.** Tier ≥ 1 operations get an optional `dryRun: bool` param added by codegen, and their result root
  MUST be `type: object` (so `actions.delete` returns `{ deleted: true }`, never `null`), which `lint-catalog`
  enforces. The result contract has two shapes, both in `protocol.json`: `operations.<name>.result` (the normal
  result) and `operations.<name>.dryRunResult`, which is always the shared type
  `DryRunResult { dryRun: true, plan: Vec<PlanStep>, diff?: Value, warnings: Vec<String>, wouldRequireApproval:
  bool, effectiveTier: RiskTier }` with `PlanStep { operation, params /* rendered, secrets redacted */, tier,
  target?: String, description }`. The required literal `dryRun: true` discriminates the two shapes on the wire,
  so `dryRun` is also a reserved **result** key and `lint-catalog` fails any result schema that defines it. The
  contract is the same on WS, REST and MCP; generated clients expose it as overloads keyed on the `dryRun` literal
  (§9.1), OpenAPI responses are `oneOf: [result, DryRunResult]`, and MCP applies its own wrapping afterwards
  (§9.6). Every tier ≥ 1 operation ships at least one normal and one dry-run example, both validated in CI (§17).
- **MCP exposure rules come from the tier, not the author:** tier 4 is **never** exposed over MCP (D-030: code
  execution, ending the stream), whatever the manifest says. Tier 3 requires in-app approval. Tier 2 creates
  disabled drafts. The tier that counts is the **effective tier** of the call (§8.8): for `actions.run` that is
  the maximum over everything the action would execute, re-checked at every node, so a `Code` node or an
  end-stream operation reached through a stored action, a nested or queued `RunAction`, `plugins.call` or a batch
  is refused under an MCP context exactly like a direct call.
- Plugin operations are first-class methods (`acme.send_chat`). `plugins.call { plugin, method, params }`
  remains as a generic alias for clients that only know plugin IDs.

### 8.2 Envelope (WebSocket and REST `POST /api/v1/rpc`)

JSON-RPC 2.0, unchanged: requests `{ jsonrpc, id, method, params }`, responses `{ jsonrpc, id, result | error }`,
notifications without `id`. `params` is always an object (never positional). WS subprotocol
`streamden.v1.json`; `streamden.v1.msgpack` is reserved for later and not required in v1. Endpoint paths and port
are in spec 06.

**Batches:** standard JSON-RPC batch arrays run independently and in parallel (≤ 64 entries). Ordered execution
uses `batch.execute { mode: "serial", haltOnFailure, requests: [...] }`, which returns one result per entry.

### 8.3 Handshake

1. On connect the server sends a notification:
   `hello { server: "StreamDen", version, protocolVersion, rpcVersions: { min, max }, auth: ["token", "pairing"],
   features: ["batch", "replay", "sse"], heartbeatMs: 30000 }`.
2. The client MUST call `session.identify` within 10 s:
   `{ rpcVersion, token?, client: { name, version }, subscribe?: [TopicGlob], highVolume?: [EventType], since?: EventId }`.
   Result: `{ sessionId, negotiatedRpcVersion, scopes, serverTime, replayed?: u32, gap?: bool }`.
3. Before identify, only `session.identify`, `auth.request_pairing` and `system.ping` are accepted; anything else
   gets `UNAUTHENTICATED`. Pairing (spec 06) returns the token once over the pending connection.
4. `session.reidentify { subscribe?, highVolume? }` changes options without reconnecting.
5. Heartbeat: either side MAY send `system.ping`; the server closes a session silent for 3 × `heartbeatMs`.

**WS close codes:** 4000 unknown error · 4001 identify timeout · 4002 unsupported `rpcVersion` (with the
supported range in the reason) · 4003 authentication failed · 4004 token revoked · 4005 message too large ·
4006 rate limited · 4007 server shutting down (clients SHOULD reconnect with backoff) · 4008 session replaced.

### 8.4 Subscriptions and replay

- `events.subscribe { topics: [TopicGlob], highVolume?: [EventType], since?: EventId }` and
  `events.unsubscribe { topics?, highVolume? }`. These are the same shapes as `subscribe`/`highVolume` in
  `session.identify`. Identify **sets** the session's initial set, `events.subscribe`/`unsubscribe` **add to and
  remove from** it, and `session.reidentify` **replaces** it. Topics are event types with `*` matching one segment
  and `**` matching the rest (`twitch.**`, `*.follow`). Each event is delivered at most once per session.
  High-volume types are delivered only when listed exactly in `highVolume`; globs in `topics` never match them.
- Delivery: notification `event { event: Event }`.
- Only events in the session's scopes are delivered (`events:read[:<glob>]`). **High-volume** event types (audio
  levels, raw transforms, VMC frames) are flagged in the catalog and are only delivered when requested explicitly
  (obs-websocket lesson).
- **Replay:** the server keeps a ring buffer of recent non-high-volume events (default 1,000 events or 5 minutes,
  spec 06 tunes it). `since` re-delivers newer events marked `meta.replay: true`; if the cursor is older than the
  buffer, the result has `gap: true` and the client should resync state. Overlays use this to survive reconnects.
- Slow consumers: each session has a bounded send queue. On overflow the server drops the oldest events of types
  not marked `critical` in the catalog (critical: moderation deletes/bans, `catalog.changed`, alert control), sends `events.dropped { count, since }`, and closes with 4006 if it keeps overflowing.

### 8.5 Errors

Protocol faults use the JSON-RPC codes (-32700 parse, -32600 invalid request, -32601 method not found, -32602
invalid params, -32603 internal). Application errors use **-32000** with a stable string in `error.data.code`:

`error.data = { code, details?: Value, fieldErrors?: [{ path, message }], retryAfterMs?, docsUrl? }`

| `data.code` | Meaning |
|---|---|
| `UNAUTHENTICATED` | No or invalid identify |
| `PERMISSION_DENIED` | Scope missing (`details.required` lists it), or the call's effective tier exceeds the context's `maxTier` (`details.reason = "tier_exceeds_context"`, plus `details.node` when it happened inside a run, §8.8) |
| `INVALID_PARAMS` | Schema validation failed. Always sent with JSON-RPC code **-32602** (the only `data.code` that doesn't use -32000); `fieldErrors` set |
| `NOT_FOUND` · `CONFLICT` · `INVALID_STATE` | Entity missing · name or version clash · wrong state (e.g. queue paused, running a disabled draft) |
| `RATE_LIMITED` | `retryAfterMs` set |
| `TIMEOUT` · `CANCELLED` | Operation deadline · cancelled by the client or by shutdown |
| `PLUGIN_UNAVAILABLE` · `CONNECTION_UNAVAILABLE` · `UPSTREAM_ERROR` | Plugin not running · connector not connected · platform/app returned an error (`details.upstream`) |
| `APPROVAL_REQUIRED` · `PAUSED` | In-app approval pending (`details.approvalId`); `details.reason = "binding_mismatch"` when a supplied approval does not bind to this call (§8.8) · "Pause AI control" or a global pause is on |
| `UNSUPPORTED_ON_PLATFORM` | OS-gated operation (`details.platforms`) |
| `PAYLOAD_TOO_LARGE` | Size limit exceeded |
| `UNSUPPORTED_VERSION` | Unsupported `rpcVersion` on REST (WS closes with 4002); `details.supported` gives the range |

New codes are additive. Clients MUST treat an unknown `data.code` as a generic failure.

### 8.6 Stdio channels (Python/C# workers and sidecar plugins)

JSON-RPC 2.0 with LSP-style `Content-Length` framing, in both directions. Spike S-3 validated this: stray
`print` output couldn't corrupt the stream, and 2 MiB payloads passed without deadlock. This supersedes the
research note's newline-delimited suggestion. The MCP stdio shim follows the MCP spec's own framing. Methods are
listed in spec 04 (workers) and spec 12 (plugins), and they reuse this envelope, error table and the shared types.
Binary payloads (TTS audio) never go through JSON; they use a side channel defined in spec 10/12.
**Windows:** `Content-Length` counts bytes, so both ends MUST use binary-mode stdio (Python's text-mode stdout
turns `\n` into `\r\n`). The worker and sidecar SDKs set binary mode (in Python, `sys.stdout.buffer` and
`msvcrt.setmode`), and the conformance suite runs a framing test on Windows CI.

### 8.7 Scopes

Format `<area>:<verb>[:<qualifier>]`: `actions:read`, `actions:run[:<actionId|tag>]`, `actions:write`,
`variables:read`, `variables:write[:<namespace>]`, `events:read[:<glob>]`, `chat:send`, `chat:moderate`,
`obs:control`, `tts:speak`, `plugins:call[:<pluginId>]`, `plugins:manage`, `admin`. Overlay tokens are limited to
`events:read:<globs>` plus `variables:read:<names>`. Each operation lists its scopes in the catalog; the server
checks them on every surface (WS, REST, MCP, plugin host calls).

### 8.8 Authorization context and composite execution

Checking the static tier of the outer operation is not enough: `actions.run` executes a stored tree that can hold a
`Code` node, an end-stream operation, or a nested or queued `RunAction`. So every call and every run carries one
**authorization context**, created at the entry surface and never widened afterwards:

```rust
pub struct AuthContext {
    pub principal: Principal,          // kind-tagged, below
    pub surface: Surface,              // rpc | mcp | sub_action | plugin_host | internal
    pub scopes: Vec<Scope>,
    pub max_tier: RiskTier,            // the highest tier this context may ever execute
    pub approvals: Vec<ApprovalBinding>,
    pub lineage: Vec<RunId>,           // runs this context has passed through (nested RunAction), newest last
}
pub enum Principal {                   // #[serde(tag = "kind")]
    Token { token_id: TokenId, name: String, kind: TokenKind /* ui | api | overlay */ }, // max_tier: the token's limit, default 4
    Mcp { token_id: TokenId, session_id: SessionId, client: ClientInfo },               // max_tier: 3, always (D-030)
    Plugin { plugin_id: PluginId },                                                       // max_tier: from its permissions (spec 12)
    Trigger { trigger_id: TriggerId, action_id: ActionId },                               // event-started runs; max_tier: 4
    System,                                                                               // startup, timers, migrations; max_tier: 4
}
pub struct ApprovalBinding {
    pub id: ApprovalId, pub operation: OperationName,
    pub args_hash: Sha256,             // canonical JSON of the rendered params, minus `dryRun` and `approvalId`
    pub action: Option<ActionRevisionRef>, // { id: ActionId, revision: u64 } (§3: no tuples on the wire)
    pub tier3_descendants: Vec<OperationName>, // for actions.run: what the dry-run plan listed
    pub session_grant: bool, pub expires_at: Timestamp,
}
```

The rules are normative for every surface. Spec 03 (engine) and spec 06 (API/MCP) implement them; §17.9 tests
them.

1. **Propagation.** A run started by `actions.run` inherits the caller's context. Inline and queued `RunAction`
   children, `Code` nodes, `Parallel` branches, `batch.execute` entries and `plugins.call` targets inherit the
   context of the node or request that started them, with the run appended to `lineage`. A queued run keeps the
   context it was enqueued with, however long it waits. Nothing executed on behalf of a context may exceed its
   `max_tier` or use a scope it lacks, at any depth.
2. **Effective tier.** `effectiveTier(action)` is the maximum tier over every enabled node reachable from the
   action: `Op` → the operation's catalog tier (host classification included, §11); `Code` → 4; `RunAction` with a
   static target → `effectiveTier(target)` (a cycle counts once); `RunAction` whose target is a template or is
   otherwise unresolvable at check time → 4 ("unknown is dangerous"); `Placeholder`, control flow, disabled nodes
   and disabled targets → 0. `actions.run` is tier 1 in the catalog, but a call's effective tier is
   `max(1, effectiveTier(target))`, and tier policy (MCP exclusion, approval, draft-first) applies to the
   **effective** tier. The same computation is exposed as `actions.effective_tier` (tier 0) and in every dry-run
   `plan` (§8.1). Under an MCP context, `actions.run` on an action with effective tier 4 is refused with
   `PERMISSION_DENIED` (`tier_exceeds_context`) before anything runs; effective tier 3 requires approval. The two
   refusal codes are deliberate: a tier-4 *operation* is hidden from MCP entirely and so is `NOT_FOUND` (§9.6),
   while an *action* the client can list is visible, so running it is a permission failure.
3. **Enforcement at every node, not only at the entry.** Static checks go stale: plugins are reinstalled, actions
   are edited between enqueue and execution, dynamic `RunAction` targets resolve at run time. The engine therefore
   re-checks each node's tier and scopes against the run's context immediately before executing it. A node that
   exceeds the context fails with `PERMISSION_DENIED` (`tier_exceeds_context`, `details.node`), the run **stops**
   (it does not skip the node and continue), and the audit log records it. Under an MCP context a `Code` node
   therefore never executes, whatever path led to it.
4. **Approvals bind to what actually executes.** An approval covers one `operation` + `args_hash`, and for
   `actions.run` also the target `ActionId` + `revision` (§6.1) and the tier-3 descendants the dry-run plan listed.
   A call with a different hash, a bumped revision or a new tier-3 descendant gets `APPROVAL_REQUIRED` again with
   `details.reason = "binding_mismatch"`. Session grants ("approve for 15 min") bind to `operation` + action +
   revision, not to arguments. One approval covers one execution unless it is a session grant; the handle TTL (§4)
   counts from issue. Untrusted text never satisfies an approval (§16).
5. **Events start a new context.** Events emitted by a run carry `meta.causedBy`. Triggers that fire on them start
   new runs under `Principal::Trigger` with an empty `lineage`, because a trigger is the user's own configuration,
   not the caller's intent. The audit record links both runs through `causedBy`; loop protection stays in spec 03.
   Whether MCP-caused chains should instead inherit MCP restrictions is Q-A6 (the spec's default is no).
6. **Drafts never run.** `actions.run` on an `ai_draft`, or on any `enabled: false` action, fails with
   `INVALID_STATE`; an MCP context cannot enable it (tier 2 is draft-first).
7. **Visible, not settable.** The run context (§6.3) exposes `run.principal` (kind only), `run.surface` and
   `run.maxTier` read-only for templates and scripts. No API input can set any part of an `AuthContext`.

## 9. Codegen

### 9.1 Pipeline

```
crates/den-protocol (types + catalog registration: fn register(&mut Catalog))
  └─ cargo xtask codegen
       ├─ schema/protocol.json        catalog: operations (params, paramsAuthoring, result, dryRunResult), events
       │                              (payload schema per payloadVersion, payloadVersions per rpcVersion), shared
       │                              types, errors, scopes, deprecations
       ├─ schema/plugin.schema.json   manifest schema (§11)
       ├─ schema/openapi.json         OpenAPI 3.1 via aide (REST resources + POST /api/v1/rpc)
       ├─ schema/asyncapi.yaml        AsyncAPI 3.0, emitted by xtask from protocol.json (WS events)
       ├─ sdk/ts/src/gen/             ts-rs types + client methods generated from protocol.json
       ├─ sdk/python/streamden/_gen/  pydantic v2 models (datamodel-code-generator) + async client + api.pyi
       ├─ schema/mcp-tools.json       MCP tool definitions per profile (§9.6)
       └─ docs/reference/             generated reference pages
```

- **Registration is explicit** (`fn register(&mut Catalog)` called from each crate's entry point), not
  link-time magic like `inventory`, because that breaks on some targets and hides missing registrations.
- `protocol.json` is the canonical artifact. Every downstream generator reads it, except ts-rs (which reads the
  Rust types). A CI test validates every example and fixture against both the JSON Schema and the TS types.
- `cargo xtask codegen --check` regenerates into a temp dir and diffs. CI runs it on every PR, plus `git diff
  --exit-code`. Generated files are committed, marked `// @generated`, and never hand-edited (AGENTS.md).
- Generator versions are pinned (Cargo.lock, `pnpm-lock.yaml`, `uv.lock`), so output only changes when sources do.
- **Generated clients carry the dry-run contract** (§8.1). TS: `client.chat.send(params): Promise<ChatSendResult>`
  and an overload `client.chat.send(params & { dryRun: true }): Promise<DryRunResult>`; the generic
  `client.call(method, params)` returns the union. Python: `@overload` on `dry_run: Literal[True]`, with pydantic
  models `ChatSendResult | DryRunResult` discriminated by `dryRun`. Clients validate incoming results against the
  shape the request selected; a mismatch is a client-side protocol error, not a silently widened type.
- **Authoring schemas are generated, never hand-written**: `paramsAuthoring` (§10.1) is derived from `params` by
  xtask and is what the UI forms, `den validate` and the save-time validator use for `Op.params`.

### 9.2 TS: ts-rs, not specta

**Pick `ts-rs` (12.x, stable).** Reasons:
1. `specta` 2.0 is still a release candidate after a long RC period. A contract crate shouldn't depend on an RC.
2. We don't need `tauri-specta`'s typed Tauri commands. The UI talks to the engine with the same JSON-RPC messages
   as `/ws` (architecture §1; process split from S-1), so the client is generated from `protocol.json` like any
   other SDK, and there are no per-command Tauri bindings to type.
3. ts-rs reads the same serde attributes, emits one file per type, and is simple to review.

**Rejected:** generating TS purely from JSON Schema (`json-schema-to-typescript`). It loses doc comments on
unions and gives weaker discriminated unions. It stays the fallback if ts-rs falls behind serde features.
**Cost:** two generators (ts-rs and schemars) read one source, so the conformance test in §9.1 is mandatory.

### 9.3 OpenAPI: aide, not utoipa

`aide` builds OpenAPI 3.1 directly from `schemars` schemas. `utoipa` needs its own `ToSchema` derive on every
type, which would be a second schema source that can drift. If aide stalls, the fallback is emitting OpenAPI
from `protocol.json` in xtask, the same way AsyncAPI is emitted. Pin an `aide` release built on the same
`schemars` major as `den-protocol` (schemars 1.x); a mismatch means two incompatible `JsonSchema` traits.

### 9.4 Python

pydantic v2 models from `protocol.json` with `datamodel-code-generator` (pinned), plus a generated async client.
`api.pyi` stubs for the script host facade (FEATURE-PARITY 6.03) come from the operations with
`expose.sub_action = true`, so scripts and actions share one vocabulary.

### 9.5 Licensing

Generated SDKs and `schema/*` are **MIT OR Apache-2.0** (D-016). Whether `crates/den-protocol` itself uses that
license or GPL is asked in Q-B6.

### 9.6 MCP tool generation

From each operation that passes the **exposure predicate** `expose.mcp = true && tier ≤ 3`. The same predicate,
evaluated at runtime against the live catalog, filters every profile list and the meta-tools `search_operations`,
`describe_operation` and `invoke_operation`, so no path reaches an operation a native tool could not. Protocol
methods (§8.1, §12) are not operations: `invoke_operation` returns `NOT_FOUND` for them, as it does for tier 4.
Per tool:
- **name** = method with `.` → `_` (`obs.set_scene` → `obs_set_scene`). Codegen fails unless it matches
  `^[a-z0-9_]{1,64}$`. Renames keep an entry in `schema/mcp-aliases.json`.
- **inputSchema** = params schema. Codegen **fails** if the root isn't `type: object` or has root
  `oneOf`/`anyOf`/`allOf` (rejected by the Claude API, flattened by Claude Code). Enums go inside a property.
  Small `$ref`s are inlined. Property names must match `^[A-Za-z0-9_.-]{1,64}$`.
- **outputSchema** = the result schema **after** MCP transforms: `x-den-untrusted` string fields are replaced
  by the wrapper schema `{ untrusted: true, source, text }`, and tier ≥ 1 tools get the `oneOf`-free envelope
  `{ result?: <result>, dryRun?: DryRunResult }` (the same `DryRunResult` as §8.1, wrapped so that
  `structuredContent` always validates without a root union). The server also returns a JSON text block.
- **annotations**, all four always set explicitly (MCP defaults `destructiveHint` to true when not read-only):
  tier 0 → `readOnlyHint: true`; tiers 1–2 → `readOnlyHint: false, destructiveHint: false`; tier 3 →
  `destructiveHint: true` plus `_meta["anthropic/requiresUserInteraction"]`. Operations whose effective tier is
  dynamic (`actions.run`, `plugins.call`, the `invoke_operation` meta-tool, §8.8) are annotated like tier 3,
  because a given call may need approval; the dry-run plan tells the client in advance. `idempotentHint` comes
  from the catalog. `openWorldHint: true` when the operation talks to an external platform or app (catalog flag
  `external`), otherwise false.
- **Runtime as well as build time:** plugin operations appear at runtime, so the same name, length, schema and
  tier checks run when a manifest is validated (`den validate`), on install, and when building `tools/list`. A
  name collision after the `.` → `_` mapping (`a_b.c` vs `a.b_c`) rejects the later plugin's operation from
  MCP with a UI notice. A plugin operation that declares `expose.mcp: true` but is raised to tier 4 by the host's
  classification (§11) installs with a warning and is dropped from MCP (the host's classification wins; the author
  may not know it); for core operations the same condition is a `lint-catalog` build failure. Tier 4 operations are excluded from `tools/list`, `search_operations`,
  `describe_operation` and `invoke_operation` alike.
- **description:** the first line holds category and search keywords. Codegen warns above 1,024 chars and fails
  above 4,096.
- Fields marked `x-den-untrusted` are returned wrapped as `{ untrusted: true, source, text }`.
- The profiles (Lean, Full, Custom, Read-only) are lists of operation names in `mcp-tools.json`, each filtered by
  the exposure predicate at build and at run time. Lean contains the research §1.5 core list, including the
  configuration operations `actions.create` and `actions.update` (tier 2, draft-first, §6.1) so an AI client can
  propose disabled drafts; Read-only contains tier 0 only. Runtime behaviour is in spec 06/13.

### 9.7 CI checks (WS-02 owns them; WS-01 wires them into CI)

| Check | Fails when |
|---|---|
| `cargo xtask codegen --check` | Any generated artifact drifts |
| `cargo xtask schema-compat --base <last-tag>` | Breaking change without a version bump (§7.2) |
| `cargo xtask lint-catalog` | Missing doc comment/description/example; bad identifier; reserved run-context name; MCP name/schema rule violated; a result schema defining `dryRun` or `draft`, or an author param named `dryRun` or `approvalId`; a tier ≥ 1 operation whose result root isn't `type: object` or that lacks a dry-run example; a leaf param without a resolved template mode, or a value-mode enum member starting with `{` (§10.1); a protocol method registered as an operation; a core tier-4 operation marked `mcp` |
| `cargo nextest run -p den-protocol` | Round-trip, golden fixture or migration test fails |
| `pnpm -C sdk/ts typecheck && pnpm -C sdk/ts test` | Generated TS doesn't compile or rejects fixtures |
| `uv run pytest sdk/python` (plus pyright) | Generated models reject fixtures |

## 10. Template syntax (proposal; D-027 fixes `{…}` as canonical, Q-X4 asks about expression scope)

| Form | Meaning |
|---|---|
| `{path}` | Value at `path` in the run context: `{user.displayName}`, `{amount}`, `{words[0]}`, `{globals.deaths}` |
| `{path \| filter[:arg,…] \| …}` | Filters, left to right: `default:x`, `upper`, `lower`, `trim`, `truncate:n`, `length`, `round:n`, `number:"0.00"`, `currency`, `date:"yyyy-MM-dd"`, `json`, `url`, `escape_html`, `plural:"sub","subs"` |
| `{= expr}` | A small, pure expression: arithmetic, comparisons, `and/or/not`, ternary `a ? b : c`, and built-ins `min`, `max`, `abs`, `floor`, `ceil`, `random(a,b)`, `lookup(name)` (dynamic name, Streamer.bot `$parse$`). No I/O, no loops; evaluation has a step budget. |
| `{{` and `}}` | Literal braces |

- **Only template fields are parsed.** A field is a template only when its resolved template mode (§10.1) says so
  (`x-den-template: "text"` or `"value"`; `x-ui.variables: true` implies `"text"`). Other strings, including JSON
  and HTTP bodies, are literal. In a `text` field, a `{` that isn't followed by an identifier start, `=` or `{` is
  literal text (so `{"a": 1}` stays as it is), and the editor warns about it. In declarative-plugin JSON bodies,
  templates only apply to whole string values (`"{params.bri}"`, value mode), never to the JSON structure.

- A missing value renders as an empty string (Streamer.bot behaviour), and the editor shows a lint warning.
  `| default:"…"` overrides it.
- In `text` mode mixed text always renders to a string. In `value` mode the field is exactly one form: `{path}`
  keeps the value's JSON type (so `{amount}` in a numeric param stays a number) and `{= expr}` yields a number,
  boolean or string (§10.1).
- Templates are parsed when saved, and parse errors are shown with their position. The parser lives in
  `den-core`; its grammar and AST types live in `den-protocol` so the UI can lint them with the same rules.
- **Import (D-027):** `%x%` → `{x}`; `%x:fmt%` → `{x | number:"fmt"}` or `{x | date:"fmt"}` (by type);
  `~g~` → `{globals.g}`; `$math(e)$` → `{= e}` (variables inside are converted the same way); `$length(t)$` →
  `{t | length}`; `$parse(n)$` → `{= lookup("n")}`; flattened names (`input0`, `rawInputEscaped`) → generated
  aliases or filters. Anything that can't be converted is kept literally, and the import report lists it. No
  runtime `%var%` mode is planned, because D-027 only asks for conversion on import.

### 10.1 Template modes: authoring schema vs invocation schema

A numeric schema rejects the string `"{amount}"`, and a schema widened to accept it would let unresolved templates
through the public API. So every leaf field of an operation's `params`, and of a declarative plugin's request
bodies (§11), has a **template mode**, which codegen resolves and writes into `protocol.json` as `x-den-template`,
so no downstream tool guesses. Trigger `config` and plugin `settings` are always literal (mode `false`): they are
evaluated at match time or activation time, when there is no run context.

| Mode | Applies to | Meaning | Default for |
|---|---|---|---|
| `false` | any type | Literal. Never parsed; braces are plain characters (JSON and HTTP bodies, regexes, URLs). | `type: string` without `x-ui.variables` |
| `"text"` | `type: string` | Mixed text and `{…}` forms; always renders to a string. | strings with `x-ui.variables: true` |
| `"value"` | number, integer, boolean, enum string, array, object | The whole field is exactly one `{…}` form (`"{amount}"`, `"{= count * 2}"`); the rendered JSON value replaces the field. | number, integer, boolean, enum string |

Arrays and objects are `false` unless the author marks them `"value"`. An author may set any mode explicitly. In a
`value`-mode field a string that matches the `TemplateString` pattern **is** a template, never a literal, so
`lint-catalog` rejects enum members that start with `{`; a literal brace in such a field is impossible by
construction, and `text` mode is the place for literal braces (`{{`).

- **Two generated schemas per operation.** `params` is the **invocation schema**: concrete types, what the engine
  executes with and what WS, REST and MCP callers must send. `paramsAuthoring` is derived from it by xtask: every
  `value`-mode leaf becomes `anyOf: [<concrete>, TemplateString]` where `TemplateString` is `{ "type": "string",
  "pattern": "^\\{(=|[A-Za-z_]).*\\}$", "x-den-template": "value" }`; `text` leaves stay strings; `false`
  leaves stay concrete. Required-ness, nesting and `additionalProperties` are unchanged, and the `anyOf` is never
  at the root (§9.6 is unaffected because MCP tools use the invocation schema). The same derivation gives
  `TemplateOr<T>` for Rust-typed domain fields such as `Delay.ms` (§6.4).
- **Save time:** `Op.params` validate against `paramsAuthoring`, and every template string must parse (§10). A
  stored document therefore never holds a value the engine cannot type.
- **Run time:** render every template field in the run context → coerce → validate the rendered params against the
  **invocation** schema → execute. Coercion applies only to `value`-mode leaves whose concrete type is number,
  integer or boolean and whose rendered value is a string: it is accepted when it parses exactly (`"5"` → 5,
  `"true"` → true; `"5.5"` for an integer fails), otherwise `INVALID_PARAMS` with `fieldErrors` naming the node
  and field. Range, enum and pattern constraints apply to the rendered value (`{= volume * 2}` against `0..100`
  passes at 40 and fails at 60, at run time, not at save time). A failed node follows spec 03 error handling; the
  operation is never called with unvalidated params.
- **Direct calls never carry templates.** WS, REST and MCP callers have no run context: their `params` validate
  against the invocation schema, so `"{amount}"` in a numeric field is `INVALID_PARAMS`, and braces in string
  fields are literal characters. `actions.run.args` are values, not templates; they become `{args.*}` in the run.
- **Fixtures** (`den-protocol`, §17.11): numeric `{amount}`; `{= volume * 2}` against `0..100` at 40 and at 60;
  enum `{globals.scene}` resolving to a non-member; nested `{ "source": { "name": "{args.src}" } }`; a
  `false`-mode HTTP body `{"a":1}` left untouched; a `text` field mixing text and paths; the literal string
  `"{scene}"` saved into a value-mode enum field is parsed as a template, not matched as a member; a direct RPC
  call with `"{amount}"` rejected; coercion `"5"` → 5 accepted and `"5.5"` → integer rejected.

## 11. Plugin manifest schema v0

`plugin.json`, validated against `schema/plugin.schema.json` (generated from Rust types). It follows
[plugin-sdk-and-api.md B2](../research/plugin-sdk-and-api.md) with these changes for v0:

| Key | v0 rule |
|---|---|
| `$schema`, `manifestVersion: 0` | v0 is pre-stable. Breaking changes are allowed until `manifestVersion: 1` ships with v1.0 of the SDK. |
| `id`, `slug`, `name`, `version` (SemVer), `description`, `author`, `license` (SPDX), `repository`, `icon`, `categories`, `keywords` | Required: `id`, `slug`, `name`, `version`, `license`. Rules in §4. |
| `engines` | `{ streamden: SemverRange, pluginApi: "^0.1" }` |
| `runtime.kind` | `declarative` · `js` · `sidecar`. `wasm` is reserved and rejected in v0. |
| `activation`, `permissions`, `connection`, `settings` | As in B2. `settings` is JSON Schema with `x-ui` hints (B2.4) and `x-secret`. |
| `contributes.events` | Replaces B2 `triggers` (a trigger type **is** an event type, §6.2). Each: `id`, title, description, category, `payloadVersion` (starts at 1; a breaking change bumps it and the plugin's major version, §5.2), `payload` schema, `filters`, `highVolume`, `testPayload`. Becomes event type `<slug>.<id>`. |
| `contributes.operations` | Replaces B2 `subActions` **and** `requests` (one catalog, §8.1). Each: `id`, title, `sentence`, category, `params`, `result`, `tier`, `scopes`, `expose: { rpc, subAction, mcp }`, `timeoutMs`, `platforms`. Becomes method `<slug>.<id>`. A declared tier lower than the host's classification of the permissions it uses is raised (e.g. any `process` permission makes it tier 4). Leaf fields of `params` may carry `x-den-template` (§10.1); the host resolves defaults and generates the authoring schema. |
| `contributes.conditions`, `variables`, `chatSources`, `ttsVoices`, `overlayWidgets`, `panels`, `commands`, `templates` | As in B2; schemas fixed in spec 12. |
| `definitions` | Local JSON Schema definitions, referenced as `#/definitions/<Name>`. Shared types are referenced as `den://schemas/v0/<Name>.json`. |
| `l10n` | `%key%` placeholders in **manifest strings only** (VS Code style). These are not templates and are never evaluated at run time. |

Validation is strict (`additionalProperties: false` outside `x-*`), so typos fail loudly at `den validate`. The
package is a zip named `.denplugin` (B2.3 layout). `$schema` URLs use the host asked about in Q-B7, and the app
always validates against its bundled copy, never the network.

## 12. API surface owned by this spec

Only the **protocol methods** are defined here: they are not catalog operations, have no tier and are never
sub-actions or MCP tools (§8.1). Resource and configuration methods (`actions.*`, `variables.*`, `chat.*`…) are
defined by their owning spec, but always as catalog operations following §8.1 and §8.8.

| Method / notification | Direction | Purpose |
|---|---|---|
| `hello` | S→C notification | Handshake (§8.3) |
| `session.identify`, `session.reidentify` | C→S | Identify, negotiate, change subscriptions |
| `auth.request_pairing` | C→S | Start pairing (flow in spec 06) |
| `system.ping` | both | Heartbeat |
| `system.describe` | C→S | Returns `protocol.json` for this installation, **including installed plugins** (the live catalog) |
| `events.subscribe`, `events.unsubscribe` | C→S | §8.4 |
| `event`, `events.dropped`, `system.deprecation_used`, `catalog.changed` | S→C notification | Delivery, back-pressure, deprecations, plugin install/uninstall (clients re-fetch `system.describe`) |
| `batch.execute` | C→S | Serial batch |
| `plugins.call` | C→S | Generic plugin method alias |

## 13. UI flows (protocol-visible parts only)

- **Schema-driven forms:** every sub-action, trigger filter, setting and manifest form is rendered from the
  catalog schema plus `x-ui` hints (AGENTS.md: "never hand-write a form for something that has a schema").
- **API explorer** (spec 07): browses `system.describe`, runs operations with `dryRun` and renders the typed
  `DryRunResult` plan (§8.1), shows the normal and dry-run examples.
- **Deprecation and version notices:** a client that negotiated an old `rpcVersion` is flagged in
  *Settings → API Clients* (spec 06).
- **Import report:** lists unconverted templates and `placeholder` nodes (spec 15).

## 14. Cross-platform notes

- The protocol has no OS-specific types. Paths in payloads are UTF-8 strings in the OS's native form; the protocol
  never normalises them.
- OS-specific operations and events (Windows-only apps such as Meld Studio, Voicemeeter) carry `platforms` in the
  catalog. On other OSes they still appear in `system.describe` but are marked unavailable with the reason, and the
  call returns `UNSUPPORTED_ON_PLATFORM` (AGENTS.md: gate it and say so in the UI).
- Line endings in templates and text are preserved as given. `\n` is canonical for generated text.
- Generated artifacts are byte-identical on Linux, Windows and macOS (sorted keys, `\n` endings), so the drift
  check passes on every CI OS.

## 15. Performance budgets

| Metric | Budget |
|---|---|
| Serialize one typical event (≤ 2 KiB) | < 10 µs p99 |
| Fan out one event to 50 WS sessions | < 2 ms p99 (contributes to the < 10 ms event→action budget, architecture §7) |
| Schema validation of a plugin-originated payload | < 50 µs p99 for ≤ 4 KiB (compiled validators, cached per schema) |
| Template render, typical alert text | < 20 µs p99; `{= expr}` capped at 10,000 evaluation steps |
| Migration of 1,000 actions on load | < 200 ms |
| `protocol.json` size (core only) | < 2 MiB; served gzip-compressed |
| `cargo xtask codegen` | < 30 s on the reference box, so it can run in pre-commit |

Core-originated events are typed by construction and are not re-validated at run time; only plugin-, client- and
import-originated data is.

## 16. Security

- Every inbound message (WS, REST, stdio, webhook, import) is parsed with size and depth limits before any work.
- Transport hardening is in spec 06 and is mandatory: bind to `127.0.0.1` by default, check the `Host` and
  `Origin` headers on every HTTP request and WS upgrade, and use pairing tokens for LAN mode (AGENTS.md).
- Server-assigned fields (`origin`, `updatedBy`, IDs on create, `meta.caused_by`) are ignored on input (§6.1).
- Every operation is checked against the caller's scopes on every surface. The MCP meta-tools and `plugins.call`
  go through the same pipeline (validation → scope → tier policy → approval → rate limit → audit), so neither is a
  bypass.
- Tier comes from the catalog and the host's classification, never only from the client or the plugin (§8.1, §11).
- Authorization is transitive (§8.8): every node of every run is checked against the originating context's
  `maxTier` and scopes right before it executes, approvals bind to the rendered arguments and the action revision,
  and an MCP context can never reach tier 4 through `actions.run`, a nested or queued `RunAction`, `plugins.call` or
  `batch.execute`.
- Templates are rendered and the result re-validated against the concrete invocation schema before any operation
  runs; direct API callers cannot submit templates at all (§10.1).
- `x-secret` values never leave the keychain path; `x-den-untrusted` text is wrapped for AI consumers and never
  satisfies an approval.
- Plugin events are validated against their manifest schema; invalid payloads are dropped and logged, and repeated
  failures disable the plugin's event source with a UI notice.
- The template expression language has no I/O and a step budget, so a hostile imported template can't hang or
  exfiltrate.
- `system.describe` only lists operations the session's scopes allow.

## 17. Test plan

1. **Round-trip and golden tests** (`den-protocol`): every type serializes to its committed fixture and back
   unchanged (property tests with `proptest` for the tree types), including `#[serde(flatten)]` inside the
   `Node::Op` variant through serde, schemars (`unevaluatedProperties`) and ts-rs; if ts-rs can't express it,
   `ContributionRef` is written out as two fields and this test is what catches the drift.
2. **Examples are tests:** every catalog example and `testPayload` validates against its JSON Schema, its TS type
   (tsc) and its pydantic model.
3. **Catalog lint, codegen drift and schema-compat** as in §9.7.
4. **Migrations:** before and after fixtures for every step; loading a future `schemaVersion` gives a read-only
   result.
5. **WS conformance suite** in `den-testkit`, run against the real server: hello/identify, identify timeout (4001),
   unsupported `rpcVersion` (4002), scope denial, batch limits, replay with and without a gap, slow-consumer
   drop, unknown event types ignored by the generated TS and Python clients.
6. **Fuzzing** (`cargo fuzz`) of the JSON-RPC envelope, stdio framing, manifest and template parsers.
7. **MCP lint** runs on the generated `mcp-tools.json`, plus a smoke test that the Claude and OpenAI tool-schema
   validators accept every Lean-profile tool.
8. **Importer fixtures:** a corpus of Streamer.bot templates mapped to the expected `{…}` output (spec 15 extends
   it).
9. **Authorization context** (§8.8), run over MCP and over an API token limited to `maxTier: 3`: `actions.run` on
   an action with a direct `Code` node → `PERMISSION_DENIED` before execution; an inline `RunAction` child that
   contains `Code` → refused; a queued `RunAction` child edited to add `Code` after enqueue → the node fails at
   execution and the run stops; a template-targeted `RunAction` → tier 4; `actions.run` on an `ai_draft` →
   `INVALID_STATE`; approval replay with changed args, a bumped revision or a new tier-3 descendant →
   `APPROVAL_REQUIRED` with `binding_mismatch`; the same action succeeds under a `maxTier: 4` user token; a
   follow-up run triggered by an event the MCP run emitted starts a new context (asserts the Q-A6 default).
10. **Payload versions** (§5.2): two sessions on `rpcVersion` N and N−1 receive one upstream event in their
    respective shapes, replay honours each session's version, and a core `payloadVersion` bump without an
    `rpcVersion` bump fails `schema-compat`.
11. **Template modes** (§10.1): the fixture list there, at save time against `paramsAuthoring` and at run time
    against `params`, plus the direct-call rejection.
12. **Contribution references** (§4): export from installation A, import into B where the plugin is (a) installed
    under the same slug, (b) installed in a later version whose manifest renamed the slug (`type` rewritten), (c) absent (`placeholder` node,
    `unresolved` trigger, both resolve on install), (d) a different plugin owns the slug (re-bound by plugin ID); a
    core type with `plugin` set and a plugin type without `plugin` are rejected by the generated `if/then/else`
    schema clause and by the save-time validator (§4).
13. **MCP exposure** (§9.6): the Lean profile creates a disabled `ai_draft` through `actions_create` and through
    `invoke_operation("actions.create")` with identical results; `invoke_operation` on a protocol method or a tier-4
    operation → `NOT_FOUND`; `actions_update` yields an `ai_draft { draftOf }` with the target's name and leaves the
    target untouched; `queues_update` yields a `PendingChange`; every tier-2 result carries `draft` on MCP and
    validates against the same result schema on WS.
14. **Dry-run results** (§8.1): every tier ≥ 1 operation's normal and dry-run examples validate against `result`
    and `DryRunResult` respectively in JSON Schema, TS and pydantic, and the TS overload and Python `@overload`
    resolve to the right type.

## 18. Open questions (added to [QUESTIONS.md](../QUESTIONS.md))

- **Q-X4:** how much of the template expression language (§10) ships in v1.
- **Q-B6:** license of `crates/den-protocol` itself.
- **Q-B7:** host for the published schema `$id` URLs (needs a domain, ADR-0004 open item).
- **Q-A6:** whether runs triggered by events that an MCP-initiated run caused inherit the MCP restrictions (§8.8
  rule 5; the spec's default is no, with audit linkage).

Assumptions made while writing this spec (reviewers, please check): D-027 already settles Q-X3, so §10 proposes
only the details; LSP framing supersedes the research's newline-delimited sidecar framing (§8.6); MCP never
exposes tier 4, per D-030, even with a per-token opt-in; binding core payload versions to `rpcVersion` (§5.2) is a
protocol design choice made here, not a product decision, and is open to reviewer challenge.

## 19. Revision history

- **r1** (78db819): initial draft.
- **r2**: resolves the Codex (Astra) review of r1. Finding 1 (P1): §8.8 authorization context, effective tier,
  per-node enforcement, approval bindings, `Action.revision`, §17.9. Finding 2: core payload versions bound to
  `rpcVersion` with a published mapping, plugin events separated (§5.2, §7.1, §7.2, §11, §17.10). Finding 3:
  template modes, generated `paramsAuthoring` vs invocation `params`, coercion and direct-call rule (§10.1, §3,
  §6.4, §9.1, §17.11). Finding 4: `ContributionRef` flattened into `Trigger` and `Node::Op`, `Trigger.unresolved`,
  import re-binding rules (§4, §5.3, §6.2, §6.4, §17.12). Finding 5: protocol methods vs configuration operations,
  one MCP exposure predicate for tools, profiles and meta-tools, draft-first `actions.create`/`actions.update`
  with `Origin::ai_draft { draftOf }` (§6.1, §8.1, §9.6, §12, §17.13). Finding 6: `DryRunResult` as a
  discriminated result contract on every transport, client overloads, examples validated (§8.1, §9.1, §9.6, §9.7,
  §13, §17.14). New question Q-A6. A fresh-context Claude reviewer pass on r2 added: result root `type: object`
  for tier ≥ 1, the reserved `approvalId` param, draft-first narrowed to document writes with `PendingChange` and
  a reserved `draft` result field, the generated `if/then/else` clause for `ContributionRef.plugin`, tier-3
  annotations for dynamic-tier MCP tools, trigger `config` and `settings` excluded from template modes, and
  `ActionRevisionRef` instead of a tuple.
