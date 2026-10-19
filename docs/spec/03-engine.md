# Spec 03: Engine core (WS-03)

- **Status:** Revision 6 (DEN-68, pending review): closes the gaps the DEN-65 slicing found (settings document and
  operations, engine-wide dispatch order, `actions.run` queue override, `runs.list` paging, bot accounts, data
  directory ownership, the one OS gate, UI template lint) and records the owner's Q-E1–Q-E8 answers (D-039) in
  the text. r5 (DEN-49) is on `main`. r3 was approved by the reviewer (Codex Astra, 2026-10-19; history in §25),
  provisionally approved by the coordinator on the owner's delegation (2026-10-19) and approved by the owner
  2026-10-19 (D-039). Cards: DEN-15, DEN-42, DEN-49, DEN-68.
- **Owns:** `crates/den-core`: the event bus, trigger matching, commands, timers and cron, the run executor
  (sub-action tree), queues and scheduling, variables, Action History, and the SQLite storage layer. It also owns
  the engine-side implementation of the authorization rules in [spec 02 §8.8](02-protocol.md).
- **Builds on:** [spec 02](02-protocol.md) (every type name below is spec 02's; this spec never redefines them),
  [architecture.md](../architecture.md) §1–2, §7, [streamerbot-engine-ui.md](../research/streamerbot-engine-ui.md)
  §2–4, §8, [speakerbot-sammi.md](../research/speakerbot-sammi.md) §2.3, [se-widget-compat-and-node-editor.md](../research/se-widget-compat-and-node-editor.md)
  Part B, [FEATURE-PARITY.md](../FEATURE-PARITY.md) §2–5, §20, spike [S-3](../spikes/S-3.md), the B-1 bake-off
  ([bakeoff-2026-10.md](../agents/bakeoff-2026-10.md) and the three `agent/*/DEN-8-b1` branches), decisions
  D-003, D-027, D-028, D-029, D-030, D-035.
- **Feature-parity rows:** 2.01–2.10, 2.12–2.23, 2.25, 2.28–2.30, 2.32–2.34, 2.36, 2.38, 2.39 (engine side; 2.42–2.52
  are UI, spec 07; 2.11 is spec 14; 2.24, 2.26, 2.31 and 2.35 are later rows; 2.27's numeric ids are superseded by
  spec 02's string event types), 3.01, 3.04, 3.09 (partial, §6.4), 3.10–3.12, 3.15, 3.16, 3.23, 4.01–4.11, 5.01–5.07,
  20.13, 20.14 (engine side).
- **Normative words:** MUST, SHOULD and MAY are used as in RFC 2119.

## 1. Scope and non-goals

**In scope:** how events travel from a source to a run; which triggers fire for an event and in what order; what a
run is, how its sub-action tree executes, how arguments flow, how errors and cancellation behave; how queues admit,
order, pause, clear and bound runs; how globals, user variables and counters are stored and change; what Action
History records and how a run is replayed; timers and cron; the SQLite schema, migrations, retention and backups;
the engine-side enforcement of authorization contexts and loop protection; the engine's own operations, events and
settings; the invariants the node editor needs from the tree.

**Non-goals (owned elsewhere):** wire and storage *shapes* (spec 02); the script hosts and the `Code` node's
runtime (`api.*`, deadlines, pools: spec 04; this spec only states the node's contract with the run); connectors
and their events (spec 05, 11); endpoints, auth tokens, pairing, rate limits on the API surface, the MCP runtime
(spec 06); the action editor, the pending and history views, the variable viewer (spec 07); the alert queue, which
is a presentation queue in the overlay runtime and not an action queue (spec 09); the plugin host (spec 12);
Streamer.bot import and the compatible WS shim (spec 15); user groups, points, quotes and the users database
(spec 20-data rows land in P2/P5 specs); generic primitive operations such as `http.request` or file I/O (spec 11
and 19 rows).

## 2. User stories

1. As a **streamer**, a Tier-3 sub switches my OBS scene, posts a chat message and runs a JS snippet, in that order,
   and after I restart the app the action, its queue and my `{globals.deaths}` counter are still there (P1 exit
   criterion, PLAN §5).
2. As a **streamer**, alerts run one at a time in a blocking "Alerts" queue; a follow bomb of 200 events neither
   freezes the app nor plays 200 alerts after I clear the queue.
3. As a **streamer**, a trigger on "100 bits exactly" wins over my "any bits" trigger, unless I mark the latter
   *always run* (Streamer.bot semantics, row 2.29).
4. As a **streamer debugging an action**, Action History shows me the arguments as they were when the run was
   queued and after it finished, and I can replay the run (rows 2.21, 2.22).
5. As an **overlay or deck author**, I subscribe to `runs.*` and `queues.*` events and watch runs progress node by
   node (row 2.23).
6. As an **AI client**, an action I started over MCP can never reach code execution, not even through a trigger on
   an event my run caused (spec 02 §8.8, enforced here).
7. As a **node-editor user** (P3), switching between the list and the graph never changes what the engine executes,
   and moving nodes around on the canvas never bumps the action's `revision`.

## 3. Model recap and vocabulary

The types are spec 02 §5–6: `Event` with `EventMeta { payloadVersion, test, replay, causedBy, correlationId }`;
`Action { id, revision, name, group, enabled, queue, concurrent, random, alwaysRun, history, triggers, nodes,
description, tags, origin }`; `Queue { id, name, blocking, paused, maxPending, overflow, maxRunMs }`; `Trigger { id, type,
plugin?, enabled, unresolved, alwaysRun, filters, config }`; `SubAction { id, enabled, weight, note, onError, node }` with
`Node` = `Op | Group | If | Switch | While | ForEach | BreakLoop | ContinueLoop | Stop | Parallel | Delay | RunAction | Code |
Placeholder`; the run context roots `event, user, trigger, action, run, globals, plugins, match, args, now`
(spec 02 §6.3); `AuthContext` and the rules of spec 02 §8.8.

This spec adds the engine's own records and documents, all persisted per spec 02 §7.3 and carried by
`den-protocol` through a contract task (§23; **landed in spec 02 r6**, which is normative for the shapes, including
`Queue.maxRunMs`, `SubAction.onError`, `Node::ForEach`, `Command.sources: Vec<EventType>`,
`Command.permissions.users: Vec<UserKey { platform, userId }>`, `RunError.code: ErrorCode` and `run.lastError:
RunError | null`; spec 02 §6.1, §6.4, §6.5). The settings document (`Settings { engine: EngineSettings }`, §16)
joined them in spec 02 r7 (requested by r6 of this spec, §23 A14) and is not repeated below. The block below is a
reading aid kept in step with spec 02 §6.5; where they differ, spec 02 wins:

```rust
/// One execution of an action. Persisted in Action History (§12) unless `history.record` is false.
pub struct Run {
    pub id: RunId,                       // UUIDv7, minted at admission
    pub action: ActionId, pub action_name: String, pub action_revision: u64,
    pub trigger: Option<TriggerId>,      // None for actions.run and RunAction children; a replay copies the replayed run's (§12.3)
    pub event: Option<EventId>,          // the event that fired the trigger, or the replayed event
    pub queue: QueueId,
    pub state: RunState, pub cancel_reason: Option<CancelReason>, // state is a plain string on the wire; cancelReason is set only for `cancelled`
    pub queued_at: Timestamp, pub started_at: Option<Timestamp>, pub completed_at: Option<Timestamp>,
    pub duration_ms: Option<u64>,        // started → completed
    pub principal_kind: String, pub surface: Surface, pub chain: Option<ChainOrigin>, // from the AuthContext, §8.8 rule 7
    pub parent: Option<RunId>,           // the run whose RunAction node started this one
    pub inline: bool,                    // true for an inline RunAction child (shares the parent's task)
    pub test: bool, pub replay_of: Option<RunId>,
    pub event_snapshot: Option<Event>,   // the firing or replayed event, redacted, stored with the run (§12.1)
    pub untrusted_args_queued: Vec<String>, pub untrusted_args_after: Vec<String>, // taint set (§21) of each snapshot; replay re-seeds from the chosen one
    pub error: Option<RunError>,         // set when state is failed, rejected or dropped
    pub stopped_by: Option<NodeId>,      // the Stop node or Code node (ok: false) that ended the run early
    pub warnings: Vec<RunWarning>,       // loop cap hit, break outside loop, dynamic reads of missing args…
}
pub enum RunState { Queued, Running, Completed, Failed, Cancelled, Rejected, Dropped }   // snake_case strings
pub enum CancelReason { User, Cleared, Shutdown, Timeout, Parent /* a Parallel sibling or parent run stopped it */ }
pub struct RunError { pub code: ErrorCode /* spec 02 §8.5 data.code */, pub node: Option<NodeId>, pub message: String, pub details: Option<Value> }
pub struct RunWarning { pub node: Option<NodeId>, pub code: String, pub message: String }

/// Chat command (rows 4.01–4.10). Persisted document, kind "command".
pub struct Command {
    pub id: CommandId, pub revision: u64, pub name: String, pub group: Option<String>, pub enabled: bool,
    pub listed: bool,                    // shown by commands.list_public (row 4.11); Streamer.bot "include"
    pub aliases: Vec<String>,            // at least one; matched case-insensitively unless case_sensitive
    pub mode: CommandMode,               // basic { location: start | exact | anywhere } | regex { pattern }
    pub case_sensitive: bool,
    pub sources: Vec<EventType>,         // open: "chat.message", "chat.whisper", "chat.sub_message"…; empty = chat.message
    pub platforms: Vec<PlatformId>,      // empty = all
    pub ignore_bot_account: bool, pub ignore_internal: bool,
    pub cooldown: Cooldown,              // { globalMs: u32, userMs: u32, exemptRoles: Vec<String> /* default ["broadcaster"] */ }
    pub permissions: Permissions,        // { mode: allow | deny, users: Vec<UserKey>, roles: Vec<String>, groups: Vec<GroupId> }
    pub counters: CounterPolicy,         // { persistGlobal: bool, persistUser: bool }
    pub origin: Origin,
}

/// Interval timer (rows 5.01–5.07). Persisted document, kind "timer".
pub struct Timer {
    pub id: TimerId, pub revision: u64, pub name: String, pub enabled: bool,
    pub interval_ms: u64,                // ≥ 1000
    pub random_upper_ms: Option<u64>,    // if set, each delay is uniform in [interval_ms, random_upper_ms]
    pub repeat: bool,                    // false: fire once, then set enabled = false
    pub min_chat_lines: Option<u32>,     // gate: fire only if ≥ n chat.message events since the last fire (any platform)
    pub origin: Origin,
}
```

Vocabulary used below: a **node** is one `SubAction`; a **branch** is one of a `Parallel` node's `Vec<SubAction>`;
a **writer** is a node that can set a run arg (spec 02 §8.8); **admission** is the moment a run enters a queue;
**dispatch** is the moment the scheduler starts it.

## 4. Engine architecture

### 4.1 Components (all in `den-core`, no Tauri dependency)

```
Sources ──publish──► EventBus ──► Dispatcher task ──► TriggerMatcher ──► Admission ──► Queue scheduler ──► Run task(s)
   (connectors,         │              │                   │                                       │
    timers, cron,       │              ├──► den-server fan-out (WS sessions, replay ring; spec 06)  ├──► Executor (tree walk)
    API, scripts,       │              ├──► ChatHub, TTS, alerts (other crates subscribe)           ├──► Variables
    plugins)            │              └──► Event log + audit (Storage)                              └──► History + trace events
                        └── backpressure (bounded, §5.3)                                                    │
                                                                                                    Storage thread (SQLite, §13)
```

### 4.2 Threading and clocks

- One Tokio multi-thread runtime for I/O and for runs (runs are `async` tasks; every node awaits). CPU-heavy work
  (script execution, inference, hashing large payloads) never runs on it: scripts go to spec 04's pools, other
  CPU work to `spawn_blocking` with a bounded pool (AGENTS.md "Performance").
- **Storage** runs on one dedicated OS thread owning the SQLite connections, fed by a bounded command channel; the
  engine never blocks the runtime on disk I/O (§13).
- **One dispatcher task** owns the event order (§5.2). It does no I/O and no awaiting on subscribers.
- **No polling loops.** Timers use one scheduler task sleeping until the earliest deadline (§8.2); queues are
  event-driven (§10.6); storage flushes are triggered by writes and a coalescing timer.
- **Clocks.** Intervals, delays, timeouts and cooldowns use the monotonic clock (`tokio::time::Instant`).
  Timestamps in records and events use the wall clock. Cron uses the wall clock in the configured time zone
  (§8.3). The engine detects suspend/resume (monotonic time jumping less than wall time) and applies the
  missed-tick rules of §8.2 and §8.3.

## 5. Event bus

### 5.1 Publishing

- `EventBus::publish(event: Event, ctx: Option<Arc<AuthContext>>)` is the only way an event enters the engine.
  `event.id` MUST be a fresh UUIDv7 minted by the bus (publishers cannot choose it), so ids are monotonic per process
  and usable as replay cursors (spec 02 §8.4). `meta.causedBy` MUST be stamped by the bus from the publishing run
  (the executor passes the current `RunId`; direct calls pass their synthetic run, §14.1); a value supplied by a
  connector or plugin is ignored and audited as `caused_by_ignored`. A core connector attributing an upstream echo
  (spec 02 §8.8 rule 5) passes the attributed `RunId` through a separate, trusted parameter, never inside the event.
- The `AuthContext` of the causing run travels with the event **inside the engine** (`Arc`, never serialized), so
  the matcher can derive chained contexts without a lookup. When the publisher has no context (upstream event), the
  event carries none and triggers start fresh contexts.
- **Derived events.** An event the engine builds from another event (`commands.triggered` from a `chat.message`, a
  ChatHub re-emission, any future derived type) MUST carry the source event's `causedBy` and in-engine
  `AuthContext` unchanged, with `chain.hops` **not** incremented: deriving is not a hop, so an MCP-caused chat
  echo cannot launder its restrictions through a command (spec 02 §16).
- **Engine-internal events** (`runs.*`, `queues.*`, `system.events_dropped`, `commands.*`, `variables.*`) are
  published with a non-blocking `try_send` onto an internal lane that the dispatcher merges into the total order;
  they are never awaited while a scheduler or matcher lock is held (§10.2 runs under one). The lane is bounded
  (`engine.bus.internalCapacity`, 16,384); on overflow non-`critical` internal events are dropped and counted like
  high-volume events. `critical` internal events (`system.stopping`, `system.storage_*`) are published from
  ordinary task context with `await`, never under a lock.
- Core events are typed by construction. Plugin and client events are validated against their payload schema before
  publish (spec 02 §16); invalid payloads are dropped and counted per source.

### 5.2 Ordering and delivery

- Events are delivered to every subscriber in **one total order per process**, the order the dispatcher dequeued
  them. Two events from different sources have no causal relation beyond that order.
- Subscribers get an `Arc<Event>`; they MUST NOT block the dispatcher. The matcher (§6) is the one in-line
  subscriber: it computes the fire set and hands admission work to the scheduler synchronously (both are
  sub-millisecond, §20); every other subscriber receives through its own bounded channel (§5.3).
- **The engine never depends on a subscriber channel for its own behaviour.** Everything the engine must *do* in
  response to a `critical` event (run the shutdown sequence on `system.stopping`, flip to read-only on
  `system.storage_readonly`) is invoked directly by the component that raises it, in the same task, and the event
  is published for observers afterwards. A stalled observer can therefore slow nothing but itself.
- Delivery is at-most-once per subscriber. The WS replay ring (spec 02 §8.4) and the persisted event log (§12.5)
  are how consumers recover.

### 5.3 Backpressure

- The bus queue is bounded (`engine.bus.capacity`, default 8,192 events). Publishers `await` when it is full
  **unless** the event type is flagged `highVolume` in the catalog, in which case the event is dropped immediately
  and counted; every 5 s of drops produces one `system.events_dropped { count, since, types }` event.
- Critical events (catalog flag `critical`) are never dropped by the bus or the dispatcher, and never dropped for a
  subscriber that drains (the boundary is stated below).
- **Subscriber channels** (`engine.bus.subscriberCapacity`, 4,096 per subscriber) are filled by the dispatcher with
  `try_send`; it never awaits a subscriber. The guarantee "critical events are never dropped" is a guarantee of
  the **bus and the dispatcher** (a publisher awaits space, the dispatcher never discards) and of den-server's
  per-session policy (spec 02 §8.4, with replay); for an in-engine subscriber it holds **as long as the
  subscriber drains**. On a full subscriber channel: `highVolume` and ordinary events are dropped **for that
  subscriber only**, counted per subscriber, reported in `system.events_dropped { count, since, types,
  subscriber }`, and the next event the subscriber receives carries an in-band `Gap { dropped, firstMissedId }`
  marker, so a consumer sees a gap, never a reordering (order within a channel is always the dispatch order). A
  `critical` event that finds the channel full is parked in a per-subscriber **critical lane** of at most
  `engine.bus.criticalLane` (64) entries, delivered after everything already in the channel and before the next
  ordinary event the dispatcher enqueues (order is kept); if that lane is also full, or
  the subscriber has not taken anything for `engine.bus.subscriberStallMs` (5,000) while its channel is full, the
  subscriber is **stalled**: its channel is closed, `system.subscriber_stalled { subscriber, dropped }` is
  published (`critical`), the audit log gets `subscriber_stalled`, and the owning component re-subscribes with
  a fresh channel and re-syncs from the event log or history (the storage event-log writer is itself a drop-and-
  count consumer, §13.1, and is never stalled). A stalled subscriber therefore loses events, including critical
  ones, **for itself**, visibly; it never stalls matching, admission, other subscribers or the shutdown sequence
  (§10.8). §22.12 tests a stalled subscriber under a critical-event storm.

## 6. Trigger matching and precedence

### 6.1 The index

The matcher keeps an in-memory index `EventType → Vec<TriggerRef { action, trigger, specificity }>` rebuilt
incrementally on every action write (spec 02 `revision` bump), on plugin install/uninstall (`unresolved` changes)
and at startup. A trigger is in the index only if its action is `enabled`, the trigger is `enabled`, and it is not
`unresolved`. Triggers on `<slug>.unknown` are indexed under that type. The index is immutable between rebuilds
(copy-on-write), so matching never takes a lock on the hot path.

### 6.2 Filters

All filters of a trigger are AND-ed. `field` is a dotted path into `event.payload` (plus the reserved paths
`user.*` and `platform` for the envelope's normalized user and platform). Operator semantics, evaluated on JSON
values with the field's catalog type:

| `op` | True when |
|---|---|
| `eq`, `ne` | JSON equality after number normalization (`1` equals `1.0`); strings compared case-sensitively unless the catalog field says `caseInsensitive` |
| `in`, `not_in` | `value` is an array; membership by `eq` |
| `gte`, `lte`, `between` | numeric or RFC 3339 compare; `between` takes `[min, max]`, both inclusive; a missing or non-numeric field is **false** |
| `contains` | string: substring, **case-insensitive** by default (Streamer.bot Text filters are; `eq` stays case-sensitive so ids compare exactly); array: element `eq` |
| `regex` | Rust `regex` crate syntax, compiled once at index build with a 1 MiB size limit; captures go to `match` (array plus named groups) **of the winning trigger only** |
| `exists` | `value: true` → field present and not `null`; `false` → absent or `null` |

A filter whose `field` the payload lacks is false (except `exists`). A trigger with no filters matches every event of
its type. Streamer.bot's Range (`between`; max-only = `eq`), Select (`eq`/`in`, `Any` = no filter), Toggle (`eq`
on a boolean) and Text (`eq`/`contains`) all map onto this table (row 2.28); the importer does the mapping.

### 6.3 Specificity and the fire set

Every filter contributes `fieldRank × opWeight` to a trigger's **specificity**, where `fieldRank` is the catalog's
`specificity` rank for the field (spec 02 §6.2; default 1) and `opWeight` is `eq`/`in` = 3, `between`/`gte`/`lte`
= 2, `contains`/`regex` = 2, `ne`/`not_in`/`exists` = 1. A trigger with no filters has specificity 0. The value is
computed at index build, so matching only sums booleans.

For one event the matcher:

1. collects every indexed trigger of the event's type whose filters all pass: the **match set**;
2. per action, keeps **one** trigger: the most specific matching trigger of that action (ties: lowest `TriggerId`).
   One event therefore fires one action at most once (Q-E3, confirmed by the owner, D-039);
3. applies **dominance**: trigger `T` is suppressed when some other trigger `T'` in the reduced set has higher
   specificity **and** `fields(T) ⊆ fields(T')` (the set of filtered field paths), unless `T` is `alwaysRun`. A
   trigger with no filters is dominated by every filtered trigger ("most specific wins unless Always Run", row
   2.29); two triggers filtering **unrelated** fields (`user.roles` vs `amount`) never suppress each other, so a
   moderator-alert and a 100-bits alert both fire on a moderator's 100-bit cheer. Equal specificity never
   suppresses. What survives is the **fire set**;
4. orders the fire set by `(ActionId, TriggerId)` and admits one run per entry (§10.2), in that order. Admission is
   synchronous and ordered, so if two fired actions share a blocking queue they are queued in this order.

Precedence applies **across actions**, as in Streamer.bot. Within a chain (§14.3) and for test events the same rule
applies. `events.simulate` (§6.5) also uses the normal rule; it is "simulate the event", not "fire everything".

### 6.4 Run context seeding

The run's `event` root is the full envelope; `user` is `event.user` plus the user's variables (§11.3) loaded
lazily; `trigger` is `{ id, type }`; `match` holds the regex captures of the firing trigger, or, for a
`commands.triggered` run whose trigger has no regex filter, the command's own `captures` (§7.1), and is empty
otherwise; `args` is seeded with the event's **top-level payload fields** (spec 02 §6.3). Reserved names cannot
collide because codegen refuses payloads that use them (§7.1 lists the engine's own obligation). Streamer.bot's run-scope built-ins (row 3.09) map as `%actionId%`
→ `{action.id}`, `%actionName%` → `{action.name}`, `%triggerId%` → `{trigger.id}`, `%triggerName%` →
`{trigger.type}` (triggers have no name in spec 02), `%queueName%` → `{run.queueName}` (amendment A1),
`%actionQueuedAt%` → `{run.queuedAt}`, `%isTest%` → `{run.isTest}`, `%date%`/`%time%` → `{now | date:…}`.

### 6.5 Test and simulated events (rows 2.32–2.34)

- `triggers.test { triggerId, payload? }` (tier 1, scope `events:simulate`) builds an event of the trigger's type from the catalog example
  (or the given payload, validated), sets `meta.test = true`, and publishes it through the bus with an in-engine
  **target** marker, so WS subscribers and the event log see it while the matcher fires **that trigger's action
  only**, bypassing filters and precedence. The run's context is derived from the caller's synthetic run (spec 02
  rules 1 and 5: `Principal::Trigger`, `surface: event`, `chain.originRun` = the synthetic run, `hops` = 1). It
  returns `{ runId }`.
- `events.simulate { type, payload, user?, platform? }` (tier 1, scope `events:simulate`) publishes a normal event
  with `meta.test = true` through the bus, so every matching trigger fires under the normal precedence rule, and
  other subscribers (overlays, chat) see it too. Runs started by a test event have `run.isTest = true`.
- `core.test` is a trigger type (Streamer.bot `Core > Test`) whose `config.variables` object is copied into args
  (the save-time validator refuses reserved root names as keys, like *Set argument*); firing it is `triggers.test`
  on that trigger.

### 6.6 Lifecycle and engine events as triggers (row 2.38)

The engine publishes `system.started` (after storage, plugins and connectors are up), `system.stopping { reason:
quit | restart | update }` (before queues drain, §10.8), `system.crash_recovered { previousShutdownClean: false }`
on the first start after an unclean exit (detected by the `clean_shutdown` marker, §13.5), and `system.reset` when the user
runs *Reset engine state*. Connector connected/disconnected events are `<slug>.connected` / `<slug>.disconnected`
(spec 05/11). All are ordinary trigger types.

## 7. Commands

Commands are the engine's chat-command matcher (rows 4.01–4.10). Spec 08 defines `chat.message` and its siblings;
this section defines what the engine does with them.

### 7.1 Matching

On every `chat.message` (and the other `sources` a command lists) the engine evaluates enabled commands in one pass:

1. Skip the command when `platforms` is non-empty and excludes the message's platform, when `ignoreBotAccount` and
   the sender is a **bot account** (the provider below), or when `ignoreInternal` (default `true`, Streamer.bot's default) and
   the message is **internal**: the connector marks it as the echo of the engine's own outbound message, or
   `meta.causedBy` is set while `meta.replay` and `meta.test` are both false. A replayed, simulated or test
   `chat.message` is attributed to the caller's synthetic run (spec 02 rule 1) but is not internal, so commands
   match it (§22.6).
2. **Basic mode:** for each alias, compare per `location`: `start` → the message starts with the alias followed by
   end-of-text or whitespace; `exact` → the whole trimmed message equals the alias; `anywhere` → the alias occurs as
   a whole word anywhere. Case-insensitive unless `caseSensitive`. There is no prefix concept: `!so` is just text.
   **Regex mode:** the pattern matches anywhere; captures go to `match`.
3. **Permissions:** `allow` mode → the user must be listed, have one of the `roles` (`UserRef.roles`) or be in a
   group; `deny` mode → the user must not be. An empty `allow` list allows everyone; an empty `deny` list denies
   nobody (Streamer.bot's "empty Deny denies everyone" is **not** copied; the importer sets an explicit list; Q-E6,
   confirmed by the owner, D-039).
   Users with a role in `cooldown.exemptRoles` (default `broadcaster`) bypass cooldowns, not permissions.
4. **Cooldowns:** global and per-user windows are shared across all aliases of the command. A hit publishes
   `commands.cooldown { commandId, commandName, globalLeftMs, userLeftMs }` (the envelope's `user` and
   `platform` are the chat message's) instead of `commands.triggered` and does not advance counters. Cooldown state is in memory (lost on restart; row 4.04 does not require persistence).
5. On success the engine increments `counter` and `userCounter` (persisted per `counters` policy, §13.2) and
   publishes **`commands.triggered`** as a **derived event** of the chat message (§5.1: same `causedBy` and
   context, no extra hop) with payload `{ commandId, commandName, alias, location, source, input:
   Vec<String> /* whitespace-split remainder */, rawInput, message: ChatText, counter, userCounter, captures:
   { groups: Vec<String?>, named: Map<String, String> } /* regex mode only; `{ groups: [], named: {} }` otherwise */ }`;
   the envelope's `user`, `platform` and `source` are copied from the chat message and `correlationId` = the
   chat message id. **No engine event puts a reserved run-context name (`event`, `user`, `trigger`, `action`,
   `run`, `globals`, `plugins`, `match`, `args`, `now`, spec 02 §6.3) or the reserved filter path `platform`
   (§6.2) at the top level of its payload**: the user and platform an event is *about* live in the envelope, and
   regex captures are `captures`. The catalog lint (spec 02 §9.7) rejects any such field, and §22.1 has a fixture
   per engine event type. Streamer.bot's `input0..n` and `rawInputEscaped` are importer aliases over `input`
   (spec 02 §6.3).

**Bot accounts.** The set of bot accounts is not engine configuration and no setting holds it. `den-core` defines
a `BotAccounts` provider (`fn is_bot_account(&self, platform: &PlatformId, user_id: &str) -> bool` over an
in-memory copy-on-write set) that the connector layer fills from the accounts it is signed into **as a bot**:
spec 05 states per platform which account that is (the account the connector sends chat with when it differs
from the broadcaster's account; a platform signed in with one account contributes nothing). The matcher asks the
provider at match time for the message's `(platform, user.id)`, for replayed, simulated and test messages too
(the current set, not the one at the time of the original message). Until a connector registers an account the
set is empty and `ignoreBotAccount` skips nothing; the Streamer.bot importer keeps the flag as it was.

Several commands MAY match one message; each publishes its own event. A command's event is what actions trigger
on: trigger type `commands.triggered` with a filter `commandId eq …` (the editor's "Command" trigger is sugar for
that filter), so precedence and chains work like any other event. **`match` seeding for command runs:** a run
fired by a `commands.triggered` trigger has its `match` root seeded from the payload's `captures` **unless the
winning trigger itself has a `regex` filter**, whose captures then replace them (§6.4; the trigger's own regex
is the more specific intent). A plain `commandId eq …` trigger on a regex command therefore sees the command's
captures as `{match[1]}` / `{match.named}` (§22.1 has the fixture).

### 7.2 Operations

`commands.create/update/delete/get/list` (tier 2 configuration, draft-first under MCP), `commands.set_state {
commandId | group, enabled }` (tier 2: it writes the document), `commands.reset_counter { commandId, scope: global |
user, user? }` (tier 1), `commands.set_cooldown { commandId, scope, user?, remainingMs }` (tier 1; adds, removes
or resizes, row 4.05), `commands.list_public { platform?, user? }` (tier 0; `listed` commands the user may run,
row 4.11).

## 8. Timers and cron

### 8.1 Timers (rows 5.01–5.07)

A `Timer` document fires the event **`schedule.timer_fired { timerId, timerName, counter, scheduledAt,
firedAt }`**. Actions use trigger type `schedule.timer_fired` with filter `timerId eq …` (the editor's "Timer"
trigger). `counter` counts fires since creation or the last `timers.reset_counter`, persisted in `counters` (§13.2).
`timers.set_state { timerId, enabled }` and `timers.set_interval { timerId, intervalMs, randomUpperMs? }` are tier
2 (document writes); `timers.reset_counter` is tier 1. Timers are documents, so they export and import with
actions (row 5.07) and keep Streamer.bot's timer GUIDs.

### 8.2 Timer scheduling

- One scheduler task keeps a min-heap of `(nextFire: Instant, timerId)`. It sleeps until the earliest deadline;
  any timer write re-arms it. No per-timer task, no polling.
- A timer's first fire is `enable time + delay`; each following fire is `previous scheduled fire + delay`, where
  `delay` is `intervalMs` or uniform in `[intervalMs, randomUpperMs]`. Drift does not accumulate.
- **Missed ticks** (suspend, long GC-free stall, system clock irrelevant since the heap uses the monotonic clock):
  on wake-up a timer that is overdue fires **once** and re-anchors to now; it never catches up multiple ticks.
- `minChatLines`: the engine counts `chat.message` events (every platform, excluding the engine's own messages)
  since the timer last fired; if the count is below the gate at the deadline, the fire is skipped, the count is kept,
  and the next deadline is one interval later.
- `repeat: false` → after the fire the document is updated to `enabled: false` (a server-side write, `updatedBy:
  system` per amendment A7, revision bump).

### 8.3 Cron (row 5.04)

Trigger type **`schedule.cron`**, `config: { expression: String, timeZone: String /* IANA name or "local",
default "local" */, catchUp: bool /* default false */ }`. Expressions are 5-field (minute hour day-of-month month
day-of-week) with an optional leading seconds field, Vixie-cron syntax plus `L`, `W`, `#` and `@daily`-style
nicknames; `*/n`, ranges and lists as usual. The crate is `croner` (MIT, license to be re-verified at the contract
task) or equivalent; the grammar above is the contract, not the crate. The engine computes each trigger's next fire
in its time zone and schedules it on the same heap. Payload: `{ triggerId, actionId, expression, timeZone,
scheduledAt, firedAt }`. A cron fire is **dispatched directly to its owning `(action, trigger)`** with the same
in-engine target marker as `triggers.test` (§6.5): the event is published for subscribers and the log, but the
matcher fires only that trigger, so two actions with different expressions never fire each other (the index
rule of §6.3 would otherwise match every `schedule.cron` trigger on every fire).

DST rules are part of **occurrence generation**, not of deduplication: the expression is matched against local
wall-clock times in increasing order, and each matching local time maps to **at most one** UTC instant. A local
time that does not exist (spring forward) yields no occurrence; one that exists twice (fall back) yields only its
**first** instant (the earlier UTC instant, under the pre-transition offset), so the second fold of the repeated
interval contains no occurrence at all, however many times the expression matches inside it. For an every-minute
schedule in America/Chicago, 2026-11-01 01:00–01:59 CDT are 60 occurrences, 01:00–01:59 CST none, and the next
occurrence after 01:59 CDT is 02:00 CST. The resulting sequence of UTC instants is strictly increasing and
depends only on `(expression, timeZone)`, so normal ticks and restart catch-up see the same occurrences and no
per-wall-time state is needed. If the cron crate yields second-fold instants, the engine drops every instant
whose local wall time also occurs earlier under the pre-transition offset. `catchUp: true` fires at most **one** missed occurrence after a suspend or
restart, stamped with the missed `scheduledAt`; `false` (default) skips missed occurrences. The trigger is the
schedule (no document), but catch-up needs memory of what was already accounted for, so the engine keeps a
**cron cursor** per trigger:

- **Record.** `cron_cursors (action_id, trigger_id, schedule_key, last_scheduled_at, updated_at)` in
  `config.db` (§13.2). `schedule_key` is the SHA-256 of `(expression, timeZone)`, so editing the schedule
  invalidates the cursor; `last_scheduled_at` is the UTC instant of the last **accounted** occurrence (fired or
  skipped by `catchUp: false`). Second-fold wall times are never occurrences, so they are never accounted.
- **Advancing.** Immediately before admitting a cron fire, and before skipping an occurrence, the scheduler
  writes the cursor through the storage thread with `durable: true` (one write per occurrence; cron fires are
  rare) and only then admits the run. The boundary is therefore **account first, fire second**: a crash between
  the two loses that one occurrence instead of replaying it, and no occurrence is ever fired twice across any
  number of restarts. An occurrence is a *duplicate* when its UTC instant is ≤ `last_scheduled_at`, and is
  never fired; with the strictly increasing sequence above this also covers every repeated wall time.
- **Startup and resume.** For each indexed cron trigger: no cursor, or a cursor with a different
  `schedule_key` → the cursor is set to *now* (a never-fired or edited schedule gets no catch-up); otherwise the
  occurrences in `(last_scheduled_at, now]` are computed: with `catchUp: true` the **latest** one is fired
  (stamped with its `scheduledAt`, `firedAt` = now) and becomes the cursor; with `catchUp: false` the cursor
  moves to the latest one without firing. Then the next future occurrence is scheduled as usual. Cursor rows
  of triggers that no longer exist are deleted at index rebuild. §22.7 tests a daily schedule across two
  restarts (fired before the first restart → not replayed; missed during the second downtime → replayed once),
  an edit between restarts, and the full repeated hour of a fall-back with a restart between the folds.

## 9. Runs and the sub-action tree

### 9.1 Lifecycle

```
admission ──► queued ──dispatch──► running ──► completed | failed | cancelled
     └──► rejected (queue full, reject_new)       ▲
     └──► dropped  (evicted by drop_oldest)        └── stopped early by Stop / Code ok:false → completed (stoppedBy set)
```

- **Admission** (§10.2) creates the `Run`, snapshots the args (`argsQueued`, §12.1), binds the `AuthContext` and
  publishes `runs.queued`. For a non-blocking, unpaused queue with free capacity, admission and dispatch are the
  same instant and `queuedAt == startedAt`.
- **Dispatch** publishes `runs.started` and spawns the run task. Before the first node the engine performs the
  queued-child checks of spec 02 §8.8 rule 4(c) (pinned revision, grant expiry) and refuses with `Failed` if they
  do not hold.
- **Completion** publishes `runs.completed` (shape in §12.4), writes the history
  record (`argsAfter`) and frees the queue slot (§10.4).

The context root `run` exposes `{ id, queuedAt, queue, isTest, principal, surface, maxTier, chain }` (spec 02 §6.3)
and, from this spec, `lastError: { code, node, message } | null` (§9.10) and `queueName` (both amendment A1 in
§23).

### 9.2 Executing a node

For every node, in order, the executor:

1. skips it if `enabled: false` (a disabled node is also not a writer, spec 02 §8.8);
2. charges the run's node-execution budget (§9.13) and runs the **static authorization re-check** of spec 02
   §8.8 rule 3: the node's catalog tier and scopes against the run's context. This check reads no variable, so
   it can precede the snapshot. A failure stops the run with `PERMISSION_DENIED`, regardless of the node's
   `onError`;
3. pins **one snapshot** of the variable state and renders every template of the node against it **in one
   pass**, producing both the execution render and the binding render (spec 02 §8.8 "no time-of-check gap");
   for a writer, the binding render is recorded now. A snapshot is a **version pin**, not a copy: globals and
   user variables live in persistent (structurally shared) maps swapped atomically on write (§11.4), so pinning
   is an `Arc` clone and costs nothing per node. The pin is held until step 7;
4. coerces and validates the execution render against the invocation schema (spec 02 §10.1);
5. for a **tier-3 `Op`**, performs the plan check of rule 4(b) **on exactly the binding render of step 3**:
   `bindingHash` = hash of that render with every executed writer's most recent recorded render and unexecuted
   writers rendered against this same snapshot; it must consume one matching `PlannedCall`. The check happens
   after the render and before the invocation, so the value that passed approval and the value the operation
   receives come from one pin; a global changed by another run after step 3 affects neither. A failure stops the
   run with `APPROVAL_REQUIRED` (`binding_mismatch`), regardless of `onError`;
6. executes the node (§9.3–9.9) with the node's timeout, passing the step-4 values (never re-rendering);
7. applies the node's writes to `args` (`saveAs`, `writesArgs`, `Code.setArgs`) **atomically after the node
   returns**: a node never sees its own partial writes, and a later node sees all of them; the taint set (§21)
   is extended in the same step; the snapshot pin is released;
8. publishes `runs.node_completed` (§12.4) and continues, or applies the error policy (§9.10).

These steps are the **leaf** algorithm. A container node (`Group`, `If`, `Switch`, `While`, `ForEach`, `Parallel`,
inline `RunAction`) renders its own templates (condition, `value`, `items`, `args`) at its own snapshot in
steps 3–4 and then executes its children through the same steps **as it goes**, so a child's writes are visible
to the next child and to the container's next iteration; the writes a container makes itself (`ForEach`'s
`itemVar`/`indexVar` before each iteration, `Parallel`'s merge after the join, an inline child's args copied
back) are applied at the point §9.5–9.8 define, and step 7's "after the node returns" applies to leaves.

§22.9 has the regression for the gap this order closes: a second run changes `{globals.msg}` between the
approval and the node (mismatch, as spec 02 requires), after the node's pin and before the operation is invoked
(no effect: the pinned value is both approved and sent), and after the operation returned (no effect).

A run's args map is owned by its task; there is no lock. Parallel branches get copies (§9.6).

### 9.3 `Op`

The operation is looked up in the live catalog by `(type, plugin)`. Unknown or uninstalled → the node fails with
`PLUGIN_UNAVAILABLE` (not `Placeholder`: that is reserved for import). The call carries the run's `AuthContext`,
the `RunId` (so events the operation emits get `causedBy`) and a cancellation token. Timeout = `timeoutMs` from the
catalog, default `engine.run.opTimeoutMs` (30,000). The result is validated against the result schema; `saveAs`
stores the whole result object under that arg name; `writesArgs` operations (Set argument, Split…) write what they
declare. A result validation failure is `UPSTREAM_ERROR` with `details.reason = "invalid_result"`.

### 9.4 `Group`, `random` and `weight`

A `Group` runs its children in order. With `random: true` it runs exactly **one** enabled child chosen with
probability `weight / Σ weights` over enabled children (`weight` default 1.0; `≤ 0` is never chosen; if every child
is `≤ 0` or disabled the group is a no-op with a warning). The weighted table is cached per `(action, revision,
node)`. `Action.random` applies the same rule to the top-level `nodes`. `actions.set_node_weight` and
`actions.set_node_state` (both tier 2, §15) change `weight` and `enabled` by writing the document (row 2.13, 2.39).

### 9.5 `If`, `Switch`, `While`, `ForEach`, `BreakLoop`, `ContinueLoop`, `Stop`

- **Condition evaluation.** `left` and `right` are rendered to JSON values. With `autoType: true` a string that
  parses exactly as a JSON number or boolean is compared as that type; otherwise strings compare as strings
  (`"15" < "2"` is true, Streamer.bot behaviour, documented in the editor). `ignoreCase` lowercases both sides for
  string ops. `CompareOp`: `eq`/`ne` as in §6.2; `lt`/`gt`/`lte`/`gte` numeric, or lexical when both sides are
  non-numeric strings, false when types differ; `contains` substring or array membership; `regex` matches `left`
  against `right` and writes `match`; `not_exists` is true when `left` renders to a missing value (an undefined
  arg renders as missing, not as the empty string, inside conditions); `is_null_or_empty` true for missing, `null`,
  `""`, `[]` and `{}`.
- **`If`** runs `then` or `else`. **`Switch`** renders `value`, then runs the first `Case` whose `values` contains
  it (`eq` with the switch's `autoType`), else `default`.
- **`While`** re-evaluates `condition` before each iteration. `maxIterations` defaults to `engine.run.maxLoopIterations`
  (10,000); the hard ceiling for a per-node value is 1,000,000. Reaching the cap ends the loop, records a
  `RunWarning { code: "loop_cap" }` and continues after the loop (Q-E7: the owner confirmed warn-and-continue, D-039).
  Each iteration also yields to the runtime once, so a loop without awaits cannot starve other runs.
- **`ForEach`** (amendment A2, row 2.07): `{ items: TemplateOr<Value>, itemVar: String, indexVar: Option<String>,
  maxIterations: Option<u32>, children }`.
  - *Collection.* `items` is rendered **once**, at the node's snapshot (§9.2 step 3), to a JSON value: an array
    iterates its elements in array order; an object iterates `{ key, value }` pairs in **sorted key order**
    (deterministic, independent of serialization); `null` or a missing value is zero iterations with a
    `missing_value` warning; any other scalar fails the node with `INVALID_PARAMS` (`details.reason =
    "not_iterable"`, `onError` applies). The rendered collection is pinned for the whole loop: writes to the
    source variable or arg during the loop do not change what is iterated.
  - *Variables.* `itemVar` and `indexVar` (0-based `u64`) are **ordinary run args** written by the `ForEach`
    node before each iteration (reserved root names are refused at save time, like *Set argument*). They are
    visible to the body, to inline children and to later nodes: after the loop they keep the last iteration's
    values; with zero iterations the node **deletes** both names, so a value they had before the loop never
    survives the node either way. The element is the JSON value itself, any type, so `{item.name}` and
    `{item[0]}` work (spec 02 §6.3).
  - *Order and nesting.* Iterations run sequentially; `BreakLoop`/`ContinueLoop` apply; a `Parallel` inside the
    body is joined before the next iteration; the cap is `maxIterations` (default `engine.run.maxLoopIterations`,
    ceiling 1,000,000) and behaves like `While`'s (`loop_cap` warning, continue after the loop; Q-E7 covers
    both). Each iteration yields to the runtime once.
  - *Writer contract (spec 02 §8.8, amended by A2).* The `ForEach` node is a **writer** of `itemVar` and
    `indexVar` with static names (like `ArgWrites::Named` with a literal name), so a read of `{item}` inside the
    body binds to the collection and never makes unrelated fields dynamic. Its recorded binding render for
    iteration *i* is `{ "$den": "foreach", "node": <NodeId>, "items": <binding render of the items template>,
    "index": i }` (for `indexVar`, the same record with `"field": "index"`). The render of `items` follows the
    ordinary rules: a collection read from `{globals.messages}` binds the global's value as consumed, so
    changing that global after approval moves every planned hash in the body; a collection that is a run
    result (`saveAs` of an `http.request`) is a `run_value` sentinel and makes the body's reads **dynamic**
    fields, shown to the approver with the producer chain.
  - *Planned-call multiplicity.* At plan time (dry run, approval) the engine renders `items` against the
    plan-time context: when the render is **concrete**, the loop is expanded to `min(len, cap)` iterations and
    each tier-3 node in the body gets one `PlannedCall` per iteration, with the `foreach` record for that index
    in its binding render (the plan shows "*N* × `chat.send`", `N` = `min(len, cap)`); when it is
    **dynamic**, the body is planned **once** (spec 02's finite-plan rule for `While`), so a second iteration
    reaching a tier-3 node stops with `unplanned_call`, which the dry-run warnings state in advance. Execution
    consumes the planned entries in iteration order. A read of `itemVar` **after** the loop binds to the
    node's last recorded render: at plan time the `foreach` record with index `min(len, cap) − 1` for a
    concrete collection (or a `missing` render for an empty one), dynamic otherwise, which is exactly what
    execution records.
  - *Changed collections.* Every iteration's record carries the **whole** `items` render, so any change to
    the collection after approval (an element, the order, the length, including to or from empty) changes the
    `items` binding render and with it every body hash and, when the capped iteration count `min(len, cap)`
    changes, the call multiset; it is never a mismatch only at the changed index. It is caught at two points. *At the call* (spec 02 §8.8 rule
    4(a)) the recomputed plan differs. *At loop start*, when the approved plan expanded this `ForEach`
    occurrence from a concrete render, the node compares its rendered `items` with the plan before the first
    iteration: under a one-shot approval the binding render must equal the planned one, under a session grant
    the iteration count `min(len, cap)` must equal the planned count. Under a one-shot approval any change is
    `binding_mismatch` / `args`, at the call or, when the collection changed after the call started, at loop
    start; no body node runs, an emptied collection is caught although it has no iteration, and a shorter
    collection never completes with spare planned calls. A session grant exempts the hashes but pins the
    capped count: a changed element runs, a change of `min(len, cap)` is `unplanned_call` at the call or at
    loop start, and a length change that leaves `min(len, cap)` unchanged (both lengths at or above the cap)
    runs like a changed element, because the plan holds the same number of calls. Fewer
    calls on an **unchanged** collection because of control flow (`BreakLoop`, an `If` in the body, a failure
    under `onError: continue`) is never a mismatch: the plan is a superset of what runs. §22.9 tests the changed-collection, changed-length, dynamic-collection, post-loop read,
    session-grant and iteration-cap cases.
- **`BreakLoop` / `ContinueLoop`** affect the innermost enclosing `While` or `ForEach` **within the same action**;
  outside a loop they are no-ops with a warning (the editor forbids them, §18). They never cross a `Parallel`
  boundary or a `RunAction`.
- **`Stop`** ends the **current action's run** as `Completed` with `stoppedBy`. Inside an inline `RunAction`
  child it ends the child; the parent continues after the `RunAction` node (Q-E5, confirmed by the owner, D-039). Inside a `Parallel` branch it
  cancels the sibling branches (`cancelReason: parent`) and then ends the run.

### 9.6 `Parallel` (D-028)

Every enabled branch runs as its own task, each with a **copy** of the args map as of the `Parallel` node's
snapshot. The node joins when all branches have ended. A branch's **writes** are the names it set or deleted
(not its whole copied map); they are merged back into the parent's args **in branch order** (a later branch's
write to the same name wins), so the result is deterministic and independent of timing. `Delay` inside a branch
delays only that branch (and, when the branch runs inline under the task budget below, the start of the node's
later branches). A branch that fails under `onError: stop` cancels its siblings and fails the run; under
`continue` the other branches finish and the run goes on.

**Bounded fan-out.** Branches are not free: a 20-deep chain of actions each calling the next twice through
inline `Parallel` branches would otherwise start 2^20 tasks under a single queue slot. Every **root run** (a
run started by admission; its inline children and every `Parallel` branch at any depth belong to it) owns a
**task budget** of `engine.run.maxTasksPerRun` (64) permits, and a permit bounds an **existing** branch task,
not an executing one:

- a branch task holds its permit from the moment it is spawned until it ends, **including while it is parked at
  a join** (a `Parallel` node awaiting its branches, a `ForEach`/`While` body awaiting a nested `Parallel`).
  Permits are never released at a join and never re-acquired, so the number of branch tasks that exist for one
  root run, parked or executing, is at most `maxTasksPerRun` at every instant;
- a `Parallel` node takes its enabled branches **in branch order**. For each one it tries to take a permit
  **without waiting**: with a permit the branch is spawned as its own task; without one the branch runs
  **inline in the node's own task**, to completion, before the node considers the next branch. An inline
  branch has its own copy of the args, its own writes record, its own `Stop` scope and the same `onError`
  treatment as a spawned one; only the scheduling differs. No task ever waits for a permit, so nested fan-out
  cannot deadlock however deep: a parked task waits only on spawned tasks, and the leaves of that finite tree
  are executing. Branch-level parallelism is a scheduling property, not a semantic one: the merge order
  (branch order) and the merged args are identical whether a branch was spawned or ran inline; the wall-clock
  duration and the relative order of side effects *between* branches are timing, which this spec has never
  guaranteed (a branch that `signals.wait`s for a sibling that has not started yet times out under either
  rule). Before taking each next branch the node checks the run's cancellation token and whether a sibling has
  already stopped or failed the run under `onError: stop`; it takes no further branch after that. With 100
  branches and 64 permits the first 64 are spawned, branch 65 runs inline in
  the node's task, and from branch 66 on a branch is spawned whenever a permit has come back;
- the root task itself needs no permit (it is the one task a run always has); inline `RunAction` children run
  in their caller's task and need none either;
- a branch observes the run's cancellation token like every node (§9.11), spawned or inline;
- the args map is a **persistent map** like the variable store (§11.4), so a branch's "copy" shares structure
  with its parent and costs nothing until the branch writes. The **run memory budget** `engine.run.maxRunBytes`
  (32 MiB) bounds the serialized size of the root's map plus every branch's distinct writes record **from the
  branch's first write until the join merges it**, whether the branch is spawned or inline and executing,
  parked at a join or already ended (a `Parallel` with 1,000 branches that each write 3 MiB and exit holds
  those records until its join, so they are charged); the
  per-map limit `maxArgsBytes` (4 MiB) still applies to each map. A write that would exceed either fails the
  node with `PAYLOAD_TOO_LARGE` (`details.reason ∈ { "args", "run_budget" }`), under the node's `onError`;
- the **node-execution budget** `engine.run.maxNodeExecutions` (1,000,000 per root run, counted across inline
  children, branches and loop iterations) turns an exponential fan-out into a deterministic failure:
  exceeding it fails the run with `INVALID_STATE` (`details.reason = "node_budget"`), not a slow death.

Every piece of allocated run state is therefore bounded by a setting, not by the shape of the action: branch
tasks by `maxTasksPerRun` per root run and `maxConcurrent × maxTasksPerRun` engine-wide (16,384 at the
defaults), parked or executing; the continuation frames of one task by `maxDepth × maxInlineDepth` (1,024),
which an inline branch does not increase because its subtree is part of its action's tree depth; pending
branches by the node's own child list (a `Parallel` node keeps an index into its children, never a queue of
branch descriptors); and args by `maxConcurrent × maxRunBytes` (8 GiB theoretical at the defaults; the settings
validator warns when that product exceeds half of physical memory). The r2 rule released permits at joins and
bounded only executing tasks; a breadth-first schedule of a 17-level binary chain of inline `Parallel` nodes
could then park 131,071 joins under 64 permits before any leaf ran. An action that wants more tasks than the
budget does not fail and does not grow: it degrades to sequential execution inside the tasks it has and then
either completes or hits `node_budget`. §22.4 tests the
2^20 chain under the budget (bounded existing tasks, `node_budget` failure, cancellation mid-fan-out), the
17-level breadth-first chain with `Delay` leaves at budgets 1 and 64, and a
100-branch `Parallel` with 64 permits (all branches run, merge order preserved). `Action.concurrent` is defined as `Parallel` over the top-level nodes, one node per branch (Streamer.bot's
"Concurrent"; unlike Streamer.bot, `Delay` is honoured per branch, and the importer drops top-level `Delay` nodes
of imported concurrent actions to preserve behaviour). `random` takes precedence over `concurrent` when both are
set.

### 9.7 `Delay`

`ms` is a `TemplateOr<u64>`; a random range is `{= random(500, 1500)}` (spec 02 §10), which the importer produces
for Streamer.bot's "To" field. Maximum `engine.run.maxDelayMs` (3,600,000); larger values are clamped with a
warning. A delay is cancellable (§9.11) and does not hold a thread.

### 9.8 `RunAction`

`action` is an `ActionRef` (id or name; a templated name resolves at run time and makes the effective tier 4,
spec 02 rule 2). The target must be `enabled` and not a draft, else the node fails with `INVALID_STATE`.

- **`inline`:** the child executes **in the parent's task**, as a nested `Run` record (`parent`, `inline: true`,
  same queue slot, no admission), with args = a copy of the parent's args overlaid with the rendered `args` map; the
  child's `event`, `user`, `match` roots are the parent's. On completion **all** of the child's args are copied
  back into the parent (spec 02 §6.3 "flows into inline children and back"). The child's `Stop` ends the child
  only (§9.5); the child's failure follows the `RunAction` node's `onError`. Recursion is legal stored content
  (spec 02 rule 2 "a cycle counts once") and is **guarded by depth only**: when the chain of inline ancestors
  reaches `engine.run.maxInlineDepth` (32) the node fails with `INVALID_STATE` `details.reason =
  "inline_depth"`; the editor lints self-recursion without a `Stop`/`If` guard.
- **`queued`:** a new run is admitted to the **target's** queue (§10.2) with `parent` set, args seeded **only** by
  the rendered `args` map (spec 02 §8.8), the parent's `event` envelope and `user` carried for `{event.*}`,
  `{user.*}` and `{user.vars.*}` (loaded lazily for that user, §11.3), `match` empty, and the
  parent's `AuthContext` with the parent run appended to `lineage`. The parent continues immediately; the node's
  result is `{ runId, queued: true }` or, if admission was rejected, the node fails with `INVALID_STATE`
  `details.reason = "queue_full"` (so the parent can notice). Lineage length is bounded by
  `engine.run.maxLineage` (64): a longer chain is refused at admission and audited `lineage_limit`.

### 9.9 `Code`

Spec 04 owns the hosts. The engine's contract with a `Code` node: the host receives the full run context (args,
read-only roots, the operations facade) and the node's `timeoutMs` (default `engine.run.codeTimeoutMs`, 5,000),
and returns `{ ok: bool, setArgs: Map<String, Value>, logs: Vec<LogLine> }`. `setArgs` is applied as the node's
writes (any name except reserved roots, spec 02 §6.3). `ok: false` ends the run like `Stop` (Streamer.bot's
`return false`), with `stoppedBy` = the node. A thrown error, a deadline or a worker crash is a node failure
(`UPSTREAM_ERROR` with `details.lang`, `details.kind ∈ { exception, timeout, crash, oom }`) and follows `onError`.
Under a context with `maxTier < 4` the node is refused before execution (spec 02 rule 3). `logs` go to the run's
trace and the engine log, never to history snapshots. Capturing `ok` into an arg instead of stopping (Streamer.bot
"Save Result to Variable", row 6.04) is a `Code` node option owned by spec 04.

### 9.10 Errors

A node **fails** when its operation returns an error, its rendered params do not validate, its result does not
validate, it times out, the target of a `RunAction` is unavailable, or a `Code` node raises. Authorization and
approval failures are not "node errors": they always stop the run (spec 02 §8.8).

The node's `onError` policy (amendment A1; until it lands, the default applies):

| `onError` | Effect |
|---|---|
| `stop` (default) | The run ends `Failed { error }`; later nodes do not run; a blocking queue advances. Streamer.bot mostly continues on failure (outputs a `success` arg); this spec defaults to stop because a half-executed alert is worse than a visible failure. **Q-E2**: the owner confirmed the default (D-039). |
| `continue` | The failure is recorded as a `RunWarning`, `run.lastError` is set, and execution continues with the next sibling. This is the parity path for Streamer.bot imports (the importer sets `continue` on operations that had a `success` output) and for row 2.08 "Try block" (a `Group` with `onError: continue` on its children). |

`Placeholder` nodes never run and produce a warning once per run. A missing arg or global in a `text` template
renders as the empty string (spec 02 §10) and adds a `RunWarning { code: "missing_value" }` the first time per
run per name, which is what the editor's lint and the history "undefined reads" column show.

### 9.11 Cancellation

- `runs.cancel { runId }` (tier 1): a `Queued` run leaves its queue immediately (`cancelled`, `cancelReason: user`); a `Running`
  run is cancelled **cooperatively**: the run's cancellation token is tripped, the executor stops before the next
  node, and in-flight awaits (operation calls, `Delay`, `signals.wait`, script executions) observe the token and
  return early. The queue slot is released **only when the run's future has been dropped** (B-1 review finding:
  freeing the slot first lets two runs overlap in a blocking queue). Operations that have already produced an
  external side effect are not rolled back; the run records `cancelled` / `user` and the node that was interrupted.
- Cancelling a parent does not cancel its queued children (they have their own `RunId`); it does cancel inline
  children and `Parallel` branches (`cancelReason: parent`).
- A queue MAY set `maxRunMs` (amendment A9; default none): a run exceeding it is cancelled with `cancelReason:
  timeout`, so one stuck action cannot stall an alerts queue forever. The engine setting
  `engine.queue.maxRunCeilingMs` (3,600,000) caps the field, and the save-time validator warns when a static
  `Delay` in an action exceeds its queue's `maxRunMs`. Until A9 lands there is no run-level timeout (an `Op` still
  has its own).
- **S-3 consequence:** an in-process JS `Code` node stuck in native-heavy code can overshoot its deadline by
  seconds (S-3 G-1). The slot is still released only when the future drops, so such a node can hold a blocking
  queue until spec 04's watchdog reports the execution lost and the engine fails the node; the host thread is then
  abandoned, not killed.

### 9.12 Signals (row 2.25)

Two operations, both tier 1 and sub-action-exposed. `signals.send { name, args?: Value /* object */, queue: bool
}` delivers to every run currently waiting on `name`; with `queue: true` and no waiter, one copy is held for up to
`engine.signals.queuedTtlMs` (30,000) and delivered to the next waiter. `signals.wait { name, timeoutMs /* ≤
engine.signals.maxWaitMs, default 30,000, ceiling 300,000 */, overwrite: bool }` blocks the node until a signal
arrives or the timeout elapses; result `{ received: bool, signalName, args }`; with `overwrite` the signal's args
are also written into the run's args (the operation declares `writesArgs: Any`). Signals are process-local and not
persisted. A signal name is `^[A-Za-z_][A-Za-z0-9_.-]{0,63}$`.

### 9.13 Limits

| Limit | Default (setting) | On exceed |
|---|---|---|
| Nodes per action | 2,000 (`engine.action.maxNodes`) | save rejected `INVALID_PARAMS` |
| Tree depth | 32 (`engine.action.maxDepth`) | save rejected |
| Inline `RunAction` depth | 32 (`engine.run.maxInlineDepth`) | node fails `INVALID_STATE` |
| Lineage length (queued chains) | 64 (`engine.run.maxLineage`) | admission refused, audited |
| `causedBy` hops | 16 (`engine.run.maxChainHops`) | triggers do not fire, audited (§14.3) |
| Loop iterations | 10,000 default, 1,000,000 ceiling | loop ends with warning |
| Args map size (each map: root or branch) | 4 MiB serialized (`engine.run.maxArgsBytes`) | node fails `PAYLOAD_TOO_LARGE` |
| Run memory budget (root map + live branch writes) | 32 MiB (`engine.run.maxRunBytes`) | node fails `PAYLOAD_TOO_LARGE` (`run_budget`), §9.6 |
| Branch tasks per root run (existing, parked or executing) | 64 (`engine.run.maxTasksPerRun`) | further branches run inline in the `Parallel` node's task, §9.6 |
| Node executions per root run | 1,000,000 (`engine.run.maxNodeExecutions`) | run fails `INVALID_STATE` (`node_budget`), §9.6 |
| Single variable value | 256 KiB (`engine.variables.maxValueBytes`) | write fails `PAYLOAD_TOO_LARGE` |
| Concurrent running root runs, whole engine | 256 (`engine.run.maxConcurrent`) | further dispatches wait; a freed slot goes to the oldest-admitted eligible head across queues (§10.3) |

## 10. Queues and scheduling

The B-1 bake-off produced three implementations and two reviews of this design; this section fixes the semantics
they left as assumptions. The chosen admission rule is the one Claude-as-reviewer proposed and no implementation
had; the slot-release rule is the one Codex-as-reviewer required; the single-flight pump is the architecture that
passed review without a correctness finding.

### 10.1 Queue kinds and the Default queue

- The **Default queue** (nil UUID) always exists, is created non-blocking, cannot be deleted, and MAY be toggled to
  blocking or paused like any other queue (Streamer.bot 1.0.5 behaviour).
- **Non-blocking** (`blocking: false`): every admitted run is dispatched immediately, limited only by the engine-wide
  `engine.run.maxConcurrent`. There is no per-queue concurrency limit in v1 (the B-1 spikes had one; nothing in
  the parity rows needs it; it can be added as a `Queue` field later without breaking semantics).
- **Blocking** (`blocking: true`): one running run at a time, strict FIFO by admission order among eligible runs.
- Toggling `blocking` or `paused` takes effect at the next dispatch decision; running runs are never pre-empted.
- `queues.delete` cancels the queue's waiting runs (`cancelReason: cleared`), lets running runs finish, and reassigns actions that
  referenced it to the Default queue (each a document write, `updatedBy: system`, A7). The operation is tier 2 and
  reports the reassigned action ids.

### 10.2 Admission

Admission runs synchronously in the matcher or the calling operation, under the scheduler's lock:

1. **Eligibility to start now:** `canStart = !paused || action.alwaysRun`, and `blocking → running == 0`, and the
   engine-wide concurrency has a free slot **that is not owed to an older head**: a free slot always goes to the
   oldest-admitted eligible head across queues (§10.3), so a new run takes it immediately only when no eligible
   head of any queue is waiting for one (then the new run is that head). Admission runs the same selector as the
   pump, under the same lock, so a slot released while the pump is between its unlock and relock is never taken
   by a newer run ahead of an older head; the pump sees the result when it relocks.
2. If `canStart`, the run is dispatched immediately. **The bound never applies to a run that does not have to
   wait** (B-1 review P2-1: an always-run alert on a paused, full queue must get through). A run that must wait
   only because an older head is owed the slot does have to wait, and the bound applies to it.
3. Otherwise the run must wait. If `waiting.len() >= maxPending` (default `engine.queue.defaultMaxPending`, 1,000,
   when the queue's `maxPending` is `None`), apply `overflow`:
   - `reject_new`: the new run is recorded as `Rejected` with `RunError { code: "INVALID_STATE", details: {
     reason: "queue_full" } }`, `queues.run_rejected` is published, and the caller (if any) gets `INVALID_STATE`.
   - `drop_oldest`: the **oldest waiting run** (regardless of `alwaysRun`: overflow is about capacity, not pause) is
     recorded as `Dropped`, `queues.run_dropped` is published, and the new run is appended. **Zero capacity:**
     when evicting cannot make room (the effective `maxPending` is 0, so `waiting` is empty and full at once),
     there is nothing to drop and the new run is treated as `reject_new` (`Rejected`, `queue_full`,
     `details.overflow = "drop_oldest"`), never appended past the bound. `queues.create/update` refuse the
     explicit combination `maxPending: 0` + `overflow: drop_oldest` with `INVALID_PARAMS` ("use `reject_new`");
     the runtime rule still covers `maxPending: None` under `engine.queue.defaultMaxPending = 0`.
4. The run is appended to `waiting` with `queuedAt = now`; `runs.queued` is published.

Every admitted run, started or waiting, takes the next value of a per-process **admission sequence** under the same
lock, and its `RunId` is minted there from the process's **one monotonic UUIDv7 generator** (the counter bits of
RFC 9562 §6.2 break ties inside one millisecond), so the ids of admitted runs order exactly like the sequence.
Inline `RunAction` children are never admitted (§9.8); the executor mints their ids from the same generator when
they start, so every `RunId` orders by creation time. §10.3 uses the sequence across queues and §12.6 sorts by
the id.

**Target queue.** A run is admitted to its action's `queue`, with one exception: `actions.run { queue }` admits
that root run into the named queue instead. The override takes any existing `QueueId` (the Default queue
included) and needs no scope beyond the call's own (`actions:run`); an unknown or deleted id fails the call with
`NOT_FOUND` (`details.reason = "queue"`) before anything is admitted. It applies to that root run only:
`run.queue` / `run.queueName`, the `runs.*` events and the history record show the override; inline `RunAction`
children share it (same task, same slot); queued `RunAction` children are admitted to **their** target's queue as
always, and the node has no override field; `runs.replay` of such a run admits into the action's *current* queue
(the override is a call argument, not part of the run's replayable input). `queue` is an ordinary param of the
call: it is part of the call's binding hash (`args_hash`, spec 02 §8.8), so an approval obtained for a call
without it does not bind to a call with it; the `ApprovedPlan` (reachable actions, planned calls) and the
effective tier are the same with and without it, because the queue changes when the action runs, not what it
executes. It exists so a
deck or a test can run an action outside a busy blocking queue and for the queue presets of §10.9; it is not a
way past a pause (a paused target holds the run like any other unless the action is `alwaysRun`).

Rejected and dropped runs appear in history (unless `history.record` is false) so a storm is visible afterwards.
Admission never allocates the run's future: the executor is created at dispatch (B-1: cancelled runs never build
their future).

### 10.3 Dispatch order

When a slot frees, the queue is resumed, or `blocking` is turned off, the scheduler picks the first eligible
waiting run: if the queue is paused, the **first `alwaysRun` run in FIFO order**; otherwise the first run. Always-run
runs therefore overtake held runs only while the queue is paused, and stay FIFO among themselves. There is no
priority field; `queues.force_next { queueId, runId? }` (tier 1) moves one waiting run to the head (Streamer.bot
`ForceNextAction` / `ForceActionById`), and `queues.shuffle { queueId }` (tier 1) randomizes the waiting order.

**The engine-wide slot (`engine.run.maxConcurrent`).** Every root run holds one engine-wide slot from dispatch
until its future drops (§10.4); inline children and `Parallel` branches hold none (§9.6 bounds them separately).
While every slot is taken, eligible runs wait in their queues and the pump's selection has two levels: within a
queue, the first eligible run by the rule above (the queue's **head**); across queues, the head with the lowest
admission sequence (§10.2). A freed slot therefore goes to the oldest-admitted run that could start, whatever its
queue. A blocking queue whose run is still running has no head and does not compete; a paused queue's head is its
first `alwaysRun` run or nothing; `alwaysRun` has no cross-queue effect (it passes a pause, not other queues'
runs); `queues.force_next` and `queues.shuffle` reorder **within** one queue and never change a run's admission
sequence, so under engine-wide saturation a forced run still waits behind older heads of other queues: the cap is
a safety limit (§9.13), not a priority scheme, and at its default of 256 it is rarely the binding constraint. The
shutdown fire set (§10.8) competes like any other run. The selection is O(number of queues) per dispatch, computed
under the lock inside the single-flight pump (§10.6); §22.3 has the ordering cases.

### 10.4 Slot release

A run holds its slot (blocking: the queue's single slot; always: one engine-wide slot) from dispatch until its
future is **dropped**: after `Completed`/`Failed`, after a cooperative cancellation has unwound, or after a panic
has been caught (a panicking run records `failed` with `RunError { code: "INTERNAL", details: { reason: "panic" }
}`, code requested in amendment A8, and never wedges the queue). Release triggers exactly one dispatch pass (§10.6).

### 10.5 Pause, resume, clear

- `queues.pause { queueId }` / `queues.resume` / `queues.pause_all` / `queues.resume_all` (tier 1) flip `paused`
  (a runtime field persisted with the document so a pause survives restart, as Streamer.bot does). Running runs
  continue. `queues.changed` is published.
- `queues.clear { queueId }` (tier 1) cancels **every** waiting run, including `alwaysRun` runs (unanimous B-1
  choice; "clear" means empty the queue), as `cancelled` / `cleared`, and returns `{ cleared: n }`. Running runs
  are untouched. A `queues.clear` sub-action inside a run on the same queue does not cancel the run executing it.
- `queues.get`/`queues.list` (tier 0) return the document plus runtime `{ pending, running, completed, cancelled,
  rejected, dropped }` counters since start. `completed` excludes cancelled; the UI shows the Streamer.bot figure
  as `completed + cancelled` if it wants to.

### 10.6 Scheduler architecture

One `Mutex<SchedulerState>` guards all queues (admission and release are sub-microsecond critical sections). A
**single-flight pump** drains eligible runs: the first caller that finds `pumping == false` sets it, loops
(select eligible → mark running → unlock → spawn → relock) until nothing is eligible, and clears the flag; nested
callers return immediately because the active loop re-reads state. No dispatcher task, no recursion through
`Drop`, so a runtime shutdown with 20,000 waiting runs cannot overflow the stack (B-1 P1 on the Grok entry). The
proptests of §22.3 are the acceptance bar.

### 10.7 Runtime events

`queues.changed { queueId, paused, blocking, pending, running }` on every state change; `queues.run_rejected` and
`queues.run_dropped { queueId, runId, actionId }`; the `runs.*` events of §12.4. There is no polling API for queue
state.

### 10.8 Shutdown

`system.stopping` is both an ordinary trigger type (§6.6: "run my cleanup action when the app quits") and the
signal that ends admission, so the sequence is phased to let the first happen before the second:

1. **Close and fire, atomically.** The shutdown driver hands `system.stopping { reason, graceMs }` to the
   dispatcher as one unit: the matcher computes its fire set under the normal precedence rule; then, under the
   scheduler lock, **every waiting run is cancelled** (`cancelled` / `shutdown`), the fire set is admitted
   (§10.2) with `alwaysRun` semantics (it bypasses a paused queue; on a busy blocking queue it waits behind the
   running run), and **admission is closed**. Direct calls that arrive afterwards (`actions.run`, `runs.replay`,
   a queued `RunAction`) are `Rejected` with `details.reason = "shutting_down"`; later events are still
   delivered to subscribers but fire no triggers (one audit row `shutdown_unmatched { count }` at exit). A
   second `system.stopping` during shutdown fires nothing.
2. **Grace.** Running runs, including the shutdown fire set, get `engine.shutdown.graceMs` (5,000, from step 1)
   to finish; a shutdown-fired run that is still waiting on a busy blocking queue when the grace ends is
   cancelled like any other (`shutdown`; §22.3 covers the idle and the busy blocking queue, and the paused
   queue). Then every remaining run is cancelled cooperatively and the executor waits for the futures to drop
   (§10.4).
3. **Final flush, then the marker.** The storage thread flushes the variable write-behind buffer and the
   history records, runs `wal_checkpoint(TRUNCATE)` on both files, and **only after every flush succeeded**
   writes `meta.clean_shutdown = true` as the last statement before closing (startup set it to `false`, §13.5;
   a database kept read-only by §13.5 is neither flushed, checkpointed nor marked).
   A flush failure leaves it `false`, so the next start reports `system.crash_recovered` with `reason:
   "flush_failed"` rather than pretending the exit was clean.

Pending runs are **not** persisted and **not** resumed on the next start (Q-E1, confirmed by the owner, D-039): replaying yesterday's alerts
after an update is the surprising behaviour, and durable queues are a later feature if the owner wants them.
Temp globals are discarded (spec 02 §6.3).

### 10.9 SAMMI button semantics (row 2.17, P5)

SAMMI's `{ discard | queue | overlap }` and Group ID map as: *overlap* = the action's queue is non-blocking;
*queue* = a blocking queue per button or per Group ID; *discard* = a blocking queue with `maxPending: 0` and
`overflow: reject_new` (a run that must wait is rejected: exactly "skip if busy"). *Block* = pause the queue; *Stop
all queued* = `queues.clear`. No new engine primitive is required; the deck spec (14) only needs a UI for these
presets.

## 11. Variables and persistence

### 11.1 Scopes (spec 02 §6.3)

| Scope | Store | Durability |
|---|---|---|
| Run args | the run task's map | the run |
| Global, `persisted: true` | memory + `globals` table | survives restart; durable within `engine.storage.flushMs` (1,000) of the write |
| Global, `persisted: false` | memory only | cleared at shutdown and by `variables.clear_temp` |
| User, `(platform, userId)` | memory cache + `user_vars` table | as globals, per `persisted` |
| Plugin | owned by the plugin host (spec 12) | spec 12 |

A name is either persisted or temp; setting an existing name with the other `persisted` flag **moves** it (the old
entry is deleted) rather than creating a second variable with the same name. Streamer.bot keeps two stores and
lets both hold `deaths`; one namespace is chosen because `{globals.deaths}` must mean one thing in a template. The
importer renames a colliding temp global to `deaths_temp` and reports it. Spec 02 §6.3 writes the global key as
"name + `persisted`"; this spec narrows it to **`name` unique across both stores, `persisted` an attribute of
the entry**, which is a stricter invariant over the same wire shape (every `variables.*` call still carries
`persisted`). Amendment A11 asks spec 02 to state the invariant so the contract and the engine do not drift.

Every stored entry is `{ value, persisted, untrusted, updatedAt, updatedBy }`. **`untrusted`** is the
provenance flag of §21: it is set by the write rule there, stored (§13.2), returned by `variables.get`,
`variables.list` and `variables.set`, carried in the `variables.*_updated` events (independently for `old` and
`new`) and in exports (§13.4), and consulted by every
template read (§11.3). It is never an input a client can clear while keeping a tainted value: a write replaces
both the value and the flag together.

### 11.2 Operations

`variables.get { scope: global | user, name, platform?, userId? }` (tier 0; `{ value, persisted, untrusted, updatedAt, updatedBy }` or
`NOT_FOUND`), `variables.list { scope, persisted?, prefix?, platform?, userId? }` (tier 0, paged),
`variables.set { scope, name, value, persisted, mode: set | increment | decrement | default, durable?: bool,
platform?, userId? }`
(tier 1; `increment`/`decrement` require a number or missing (treated as 0) and are atomic; `default` writes only
if the variable does not exist; returns `{ value, created, untrusted }`, where `value` and `untrusted` describe
the **stored entry after the call**: for `default` on an existing entry that is the existing value with the
existing flag, and no write and no event occur), `variables.delete` (tier 1; returns `{ deleted: bool }`, never
the value), `variables.clear_temp {
scope }` (tier 1; Streamer.bot "Clear Non-Persisted (User) Globals"; returns `{ count }`), `variables.set_arg { name, value }` (tier 1,
sub-action only, `writesArgs: Named { nameParam: "name" }`, refuses reserved roots). All writes publish
`variables.global_updated` / `variables.user_updated` with payload `{ name, old, oldUntrusted, new, untrusted,
persisted, updatedBy }`; `old: null` means created (`oldUntrusted: null`), `new: null` means deleted
(`untrusted: null`); otherwise `oldUntrusted` is the replaced entry's flag and `untrusted` the written entry's,
**independently**, because one flag cannot describe two values: a user-token literal overwriting chat text emits
`{ old: <chat text>, oldUntrusted: true, new: <literal>, untrusted: false }` (rows 3.16; `variables.user_updated` is
`highVolume`). For `variables.user_updated` the **envelope's** `platform` and `user` are the variable's user
(the event is about that user), never payload keys (§7.1 reserved-name rule).

**Readback rule.** Every field of an operation result or event payload that carries a stored variable value
carries the flag of the entry it came from next to it and is catalogued `x-den-untrusted: conditional` on that
sibling (A10): `value` in `variables.get`, `variables.list` and `variables.set` results (sibling `untrusted`),
`old` in `variables.*_updated` (sibling `oldUntrusted`) and `new` there (sibling `untrusted`). No operation
returns a stored value without its flag. The flag has the same consumers everywhere: MCP wraps the field as
`{ untrusted: true, source: "variable:<scope>:<name>", text }` (non-string values as canonical JSON text)
exactly when the sibling flag is set, so an AI client never reads chat-derived text as configuration through
`get`, `list`, `set`, a subscription or the event log, while an untainted scene name stays a plain value; a run
seeded from the event treats `{event.old}` and `{event.new}` as tainted sources exactly when their flag is set
(§21); a `saveAs` of such a result taints the arg when any flagged field of the result is set; and the event log
and history store the payload with both flags, so a replay (§12.3, §12.5) seeds the same taint.

### 11.3 Reads in templates

`{globals.x}` and `{user.vars.x}` read the node's snapshot (§9.2). A missing variable is a missing value (empty
string in text, `not_exists` true). User variables for the event's user are loaded once per run on first use; the
`user` root of a run whose event has no `user` has no `vars`. A read of an entry whose `untrusted` flag is set
is a read of a **tainted source** (§21): the arg a writer produces from it joins the run's taint set, and a
tier-3 param rendered from it is listed in `untrustedFields`.

### 11.4 Storage behaviour

The in-memory store is a **persistent map** (structural sharing, e.g. `im::HashMap`, MIT/MPL-2.0 dual, or an
equivalent): a write produces a new version and swaps an `Arc`; readers pin a version (§9.2). Writes are applied
to memory immediately (so a following node in the same run sees them) and appended to the storage thread's
write-behind buffer, which flushes in one transaction every `flushMs` or every 1,000 pending writes, whichever
comes first, and immediately before shutdown. An **app crash** can lose at most the last flush interval of
persisted writes; `durable: true` waits for the flush and the WAL commit, which with `synchronous=NORMAL` survives
an app crash but may lose the last transactions on **power loss** (the §22.5 test covers the former). The value
limit is 256 KiB; the number of variables is unbounded but the viewer pages. `updatedBy` is server-assigned from
the writing context as `UpdatedBy` (amendment A7): `user`, `imported`, `mcp`, `plugin { pluginId }` or `system`.
`untrusted` is written in the same row as the value (§13.2), so a crash cannot persist a tainted value without
its flag.

### 11.5 Counters

Command counters, user counters and timer counters live in the `counters` table (§13.2), keyed by owner, and are
not globals (they have no change events and no template root other than the event payload they appear in). Row
20.04 counters with overlay binding are a P2 feature on top of globals, not this table.

## 12. Action History, pending runs, replay and trace

### 12.1 What is recorded

Every run (including inline children, test runs, rejected and dropped runs) whose action has `history.record:
true` is stored as a `Run` record (§3) plus two snapshots: **`argsQueued`**, the args map at admission (for a
queued `RunAction` child, the seeded map; for a trigger run, the seeded payload fields), and **`argsAfter`**, the
map when the run ended. Both are stored as JSON with `x-secret` fields redacted (spec 02 §3) and truncated to
`engine.history.maxSnapshotBytes` (256 KiB) with a `truncated: true` marker. The record's other variable-length
fields are bounded at record time the same way, so no stored record is unbounded: `error.details` is truncated
to `maxSnapshotBytes` with the same marker; `error.message`, `RunWarning.message` and `actionName` are cut to
4,096 Unicode scalar values; `warnings` keeps at most 100 entries, the rest replaced by one final `RunWarning {
code: "warnings_truncated", message: "<n> more" }`. The firing **event envelope is
copied into the run record** (`eventSnapshot`, redacted, counted against the same budget), because the event log
(§12.5) is pruned sooner than history and never holds `highVolume` events (a run fired by
`variables.user_updated` must still be inspectable and replayable). Each snapshot stores its own taint set
(`untrustedArgsQueued`, `untrustedArgsAfter`, §21): a run that overwrites a tainted arg with a literal ends
clean, but its queued snapshot still holds the viewer text and must say so. `history.pending: false` hides the run from `runs.list { state: queued |
running }` (the Pending view, row 2.20) but does not stop recording.

### 12.2 Retention

Records are pruned by the storage thread, triggered by history writes (a count and age check every 500 inserts,
never by a timer, so an idle engine does not wake), to `engine.history.maxRuns` (10,000) and
`engine.history.maxAgeDays` (7), whichever prunes more; `Failed` runs of the last 24 h are kept past `maxRuns`
up to 1,000 extra so a storm of successes does not erase the failures (Q-E4: defaults confirmed by the owner, D-039).
History survives restarts. `runs.clear_history` (tier 1) deletes everything.

### 12.3 Replay (row 2.22)

`runs.replay { runId, snapshot: queued | after /* default queued */, args?: Map }` (tier 1 in the catalog;
effective tier = the action's, so it joins spec 02's dynamic-tier set, amendment A8) admits a new run of the **current** revision of the same action into its queue,
seeded with the chosen snapshot overlaid by `args`, with the run's `eventSnapshot` carried as `event` and
`meta.replay = true`, `replayOf` set, `trigger` copied for `{trigger.*}` templates, the taint set re-seeded from
the **chosen snapshot's** stored set (`untrustedArgsQueued` or `untrustedArgsAfter`, §21; names supplied in
`args` are tainted when their value came from a tainted source of the caller, clean for literals), and the
**caller's** `AuthContext` (never the original run's). Events the replay emits are caused by the new run. A replay of a run whose action no longer
exists fails `NOT_FOUND`. Spec 02's `dryRun` works on `runs.replay` like on `actions.run`.

### 12.4 Trace events (row 2.23)

`runs.queued { runId, actionId, actionName, queueId, triggerId?, eventId?, parent? }`, `runs.started { runId }`,
`runs.node_started { runId, nodeId, kind, iteration? }` and `runs.node_completed { runId, nodeId, status: ok |
failed | skipped, durationMs, error? }` (both `highVolume`), `runs.completed { runId, state, cancelReason?,
durationMs, error?, stoppedBy?, warnings }`. These are the Streamer.bot `Raw.Action` / `Raw.SubAction` /
`Raw.ActionCompleted` equivalents; the node events are what the node editor's live trace highlights (§18). Per-node
timings are also kept in memory for the last `engine.history.traceRuns` (200) runs and returned by `runs.get {
runId, trace: true }`; they are not persisted.

### 12.5 Event log

Every non-`highVolume` event is appended to the `events` table (§13.2) as its **complete envelope** (`id`,
`type`, `ts`, `source`, `platform`, the full `UserRef` with roles and badges, `payload`, `meta` including
`payloadVersion`), redacted of `x-secret` fields only, and pruned, write-triggered like history, to
`engine.events.maxAgeHours` (24) and `engine.events.maxCount` (50,000). This is the "replay a logged event"
source (row 2.22): `events.replay { eventId }` (tier 1, scope `events:simulate`) re-publishes the **stored
envelope** with a new id, `meta.replay = true`, under the caller's context (spec 02 rule 1), through normal
precedence. Filters on `user.roles`, command permissions and `{user.displayName}` templates therefore see the
user **as they were**, never a current user record, and a stored `payloadVersion` older than the catalog's is
converted through a registered **upgrade** function `(type, from, to)` (A13: spec 02 §5.2 registers only
downgrades today) or, when none exists, the call fails `INVALID_STATE` (`details.reason = "payload_version"`). §22.6 replays a chat event after a restart with a role
filter and no Action History record. Chat messages are in
this log; the owner's Q-H1 answer (2026-10-19) governs **chat** retention only: a rolling 24 h by default,
configurable, plus a separate local chat-history store with a user-defined length up to 120 streams or 120 days,
owned by spec 08, which prunes `chat.*` entries from this log on that setting. The ceiling of `maxAgeHours` for
the log as a whole is Q-E9 (§16, §24), not derived from Q-H1.

### 12.6 Listing runs: order, paging and sources

`runs.list { state?: Vec<RunState>, action?: ActionId, since?: Timestamp, limit?: u32 /* 1–1000, default 100 */,
cursor?: String }` (tier 0) returns `{ runs: Vec<Run>, nextCursor: Option<String> }`:

- **Order.** Newest first by creation: every `RunId` comes from the one monotonic UUIDv7 generator (admitted runs
  take theirs under the scheduler lock, inline children when they start, §10.2), so descending `id` is descending
  `queuedAt` with admission order as the tie-break, and `id` is the single sort key for every source.
- **Paging.** Keyset, never offsets: `cursor` is an opaque string that encodes the `id` of the last run of the
  previous page (clients never parse it; a malformed cursor is `INVALID_PARAMS`); a page holds the first `limit`
  matching runs with `id` below it; `nextCursor` is absent on the last page. A run's `id` never changes, so a run
  that moves from live to terminal between two pages appears on exactly the page its `id` falls on, once.
- **Sources.** `queued` and `running` runs come from the scheduler's in-memory view (every live run, also those of
  actions with `history.record: false`); terminal runs (`completed`, `failed`, `cancelled`, `rejected`, `dropped`)
  come from the `runs` table of `history.db` (§13.2; the `runs_queued_at`, `runs_state` and `runs_action` indexes
  serve the filters). **Visibility handoff.** History inserts are asynchronous and droppable (§13.1), so a run
  that has ended is not in `history.db` yet; it therefore stays in the live view as a **settling** entry (its
  terminal `state` already set) until the storage thread acknowledges the insert, or the insert is dropped, or
  the action has `history.record: false` (never persisted: it leaves the view at once and no terminal query ever
  shows it). A run is thus always in exactly one source, live (queued, running, settling) or history, and the
  settling set is bounded by the storage channel (§13.1) because a dropped insert releases its entry immediately
  (`system.storage_backlog` reports the drop; the run is then absent for good). The operation reads the live
  view **first**, then the history rows, each with the same `state`, `action`, `since` (`queuedAt ≥ since`) and
  cursor bounds; the two lists are merged by `id` with the history row winning when both hold the same run (its
  insert landed between the two reads), sorted, and cut at `limit`. A run that completes and lands between two
  pages appears on exactly the page its `id` falls on; one whose insert is still pending is on that page as a
  settling live entry. `runs.get` follows the same two sources.
- **Filters.** An absent or empty `state` means every state. `history.pending: false` (§12.1) hides a run only
  from a query whose `state` filter is non-empty and names no terminal state (the Pending view); every other
  query shows it. `since` applies to `queuedAt`.
- **Size.** `runs.list` returns the `Run` record with `eventSnapshot` **omitted** (absent, never `null`, spec 02
  §3 "Optional vs null"; `event: EventId` still says whether there was one) and without the two args snapshots;
  `runs.get { runId, trace?, snapshots?: bool /* default true */ }` returns the record with `eventSnapshot` and
  both snapshots (§12.1). Storage limits and transport budgets are different things: `maxSnapshotBytes` bounds
  what is **stored** per snapshot; what one response may carry is the session's message budget (spec 02 §3,
  default 1 MiB, raised per session by spec 06). Records are not fixed-size (`error.details`, `warnings`, the
  taint lists), so the server **serializes the response as it goes** (the JSON-RPC envelope included) and cuts
  the page **before** `limit` when the next record would push it over the budget, returning `nextCursor`. When
  the **first** remaining record alone would exceed the budget, it is returned as a **reduced row** so that the
  cursor advances: the reduced row keeps every required field and the fixed-size optional ids (`event`,
  `parent`, `replayOf`, `stoppedBy`, `cancelReason`), omits `chain`, `error.details` and both taint lists (they
  are optional or default-empty on the wire), cuts `error.message` and `actionName` to 256 Unicode scalar values
  each, and replaces `warnings` by the single `RunWarning { code: "row_truncated", message: "<comma-separated
  names of the reduced fields>" }`. Every field of the reduced row is therefore of fixed or capped size, so its
  serialized size has a constant upper bound `K_row` (identifiers, timestamps, enums and the capped strings;
  the implementation derives the exact constant from the schema and asserts it, below 8 KiB). A session whose
  negotiated budget is below `K_row` plus the envelope cannot be served a list at all: `runs.list` fails
  `PAYLOAD_TOO_LARGE` (`details.reason = "message_budget"`, `details.bytes`, `details.minimum = K_row`) instead
  of returning an empty page, and nothing is skipped. The reduced row is a presentation of the stored record,
  never a rewrite of it. A `runs.get` response is assembled in three **components**, the record (bounded by
  §12.1: at most `maxSnapshotBytes` of `error.details` plus the capped strings and warnings), then the three
  snapshots (`snapshots: true`, default), then the trace (`trace: true`); each component is added only if the
  response still fits, otherwise the call fails `PAYLOAD_TOO_LARGE` (`details.reason = "message_budget"`,
  `details.bytes`, `details.component ∈ { "record", "snapshots", "trace" }`). `snapshots: false` and
  `trace: false` therefore cure exactly the overflow their component caused; a record whose §12.1-bounded
  `error.details` alone exceeds the budget is readable only after the session raises its budget, which the
  error says. The `message_size` settings warning (§16) fires when three snapshots at `maxSnapshotBytes` plus a
  record with `maxSnapshotBytes` of `error.details` could exceed the default budget; it is advice, not a
  guarantee.

## 13. Storage

### 13.1 Engine, files, pragmas

- **SQLite via `rusqlite` with the `bundled` feature** (MIT; SQLite is public domain), one pinned SQLite version on
  every OS. Not `sqlx`: its SQLite driver is async over a blocking pool anyway, is slower for this write pattern,
  and would make the engine's hot path depend on compile-time query checking. All access goes through the storage
  thread (§4.2); the API is a typed command enum, never raw SQL from other modules.
- Two files in the data directory (§19): **`config.db`** (documents, variables, counters, migrations, meta) and
  **`history.db`** (runs, snapshots, events, audit). History churns and is pruned; config is small, backed up
  (§13.6) before migrations and exported as JSON. Pragmas on both: `journal_mode=WAL`, `synchronous=NORMAL`, `foreign_keys=ON`,
  `busy_timeout=5000`, `journal_size_limit=67108864`, `temp_store=MEMORY`; `wal_checkpoint(TRUNCATE)` after a
  migration and at clean shutdown.
- The storage command channel is bounded (4,096). Document, variable, counter and audit writes **await** when it is
  full (they are correctness-relevant and rare enough); history, trace and event-log inserts are dropped and
  counted, and `system.storage_backlog { dropped, since }` is published once per minute while it happens.
- Every multi-row change is one transaction (row 20.13 "atomic saves"). Document writes bump `revision` inside the
  same transaction that stores the new body.

### 13.2 Schema v1

```sql
-- config.db
CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);            -- schema_version, instance_id, clean_shutdown, created_at
CREATE TABLE documents (
  kind TEXT NOT NULL,             -- action | queue | command | timer | settings | view_state | pending_change | export_meta
  id TEXT NOT NULL,               -- UUID string
  schema_version INTEGER NOT NULL,
  revision INTEGER NOT NULL DEFAULT 1,
  name TEXT,                      -- denormalized from data for display; NULL for kinds without a name
  name_key TEXT,                  -- Unicode case-folded name computed in Rust (SQLite lower() is ASCII-only); uniqueness key
  enabled INTEGER,                -- denormalized; NULL where not applicable
  data TEXT NOT NULL CHECK (json_valid(data)),   -- spec 02 §7.3 `data`
  created_at TEXT NOT NULL, updated_at TEXT NOT NULL, updated_by TEXT NOT NULL CHECK (json_valid(updated_by)),
  PRIMARY KEY (kind, id)
);
CREATE UNIQUE INDEX documents_action_name ON documents (name_key)
  WHERE kind = 'action' AND json_extract(data, '$.origin.kind') IS NOT 'ai_draft';   -- IS NOT: a missing origin.kind must not escape the index
CREATE UNIQUE INDEX documents_queue_name ON documents (name_key) WHERE kind = 'queue';
CREATE UNIQUE INDEX documents_command_name ON documents (name_key) WHERE kind = 'command';
CREATE INDEX documents_kind_updated ON documents (kind, updated_at);
CREATE TABLE globals (
  name TEXT PRIMARY KEY, value TEXT NOT NULL CHECK (json_valid(value)),
  untrusted INTEGER NOT NULL DEFAULT 0,                 -- provenance flag, §21; written with the value
  updated_at TEXT NOT NULL, updated_by TEXT NOT NULL
);                                  -- persisted globals only; temp globals never touch disk
CREATE TABLE user_vars (
  platform TEXT NOT NULL, user_id TEXT NOT NULL, name TEXT NOT NULL,
  value TEXT NOT NULL CHECK (json_valid(value)), untrusted INTEGER NOT NULL DEFAULT 0,
  updated_at TEXT NOT NULL, updated_by TEXT NOT NULL,
  PRIMARY KEY (platform, user_id, name)
);
CREATE INDEX user_vars_name ON user_vars (name);
CREATE TABLE counters (
  owner_kind TEXT NOT NULL,         -- command | timer
  owner_id TEXT NOT NULL, key TEXT NOT NULL,          -- key: '' for the global counter, 'u:<platform>:<userId>' per user
  value INTEGER NOT NULL DEFAULT 0, updated_at TEXT NOT NULL,
  PRIMARY KEY (owner_kind, owner_id, key)
);
CREATE TABLE cron_cursors (                              -- §8.3: last accounted occurrence per cron trigger
  action_id TEXT NOT NULL, trigger_id TEXT NOT NULL, schedule_key TEXT NOT NULL,
  last_scheduled_at TEXT NOT NULL, updated_at TEXT NOT NULL,
  PRIMARY KEY (action_id, trigger_id)
);
CREATE TABLE migrations (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL, app_version TEXT NOT NULL);

-- history.db
CREATE TABLE runs (
  id TEXT PRIMARY KEY, action_id TEXT NOT NULL, action_name TEXT NOT NULL, action_revision INTEGER NOT NULL,
  trigger_id TEXT, event_id TEXT, queue_id TEXT NOT NULL,
  state TEXT NOT NULL, cancel_reason TEXT,
  queued_at TEXT NOT NULL, started_at TEXT, completed_at TEXT, duration_ms INTEGER,
  principal_kind TEXT NOT NULL, surface TEXT NOT NULL, chain TEXT CHECK (chain IS NULL OR json_valid(chain)),
  parent_id TEXT, inline INTEGER NOT NULL DEFAULT 0, test INTEGER NOT NULL DEFAULT 0, replay_of TEXT,
  error TEXT CHECK (error IS NULL OR json_valid(error)), stopped_by TEXT,
  warnings TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(warnings)),
  args_queued TEXT CHECK (args_queued IS NULL OR json_valid(args_queued)),
  args_after  TEXT CHECK (args_after  IS NULL OR json_valid(args_after)),
  event_snapshot TEXT CHECK (event_snapshot IS NULL OR json_valid(event_snapshot)),
  untrusted_args_queued TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(untrusted_args_queued)),
  untrusted_args_after  TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(untrusted_args_after))
);
CREATE INDEX runs_queued_at ON runs (queued_at);
CREATE INDEX runs_action    ON runs (action_id, queued_at);
CREATE INDEX runs_state     ON runs (state, queued_at);
CREATE INDEX runs_parent    ON runs (parent_id);
CREATE TABLE events (
  id TEXT PRIMARY KEY, type TEXT NOT NULL, ts TEXT NOT NULL, platform TEXT, user_id TEXT,   -- denormalized for indexes
  payload_version INTEGER NOT NULL,
  envelope TEXT NOT NULL CHECK (json_valid(envelope))   -- the complete redacted Event (§12.5): source, user (full UserRef), payload, meta
);
CREATE INDEX events_type_ts ON events (type, ts);
CREATE INDEX events_ts      ON events (ts);
CREATE TABLE audit (
  id TEXT PRIMARY KEY, ts TEXT NOT NULL, kind TEXT NOT NULL,       -- tier_exceeds_context | binding_mismatch | unattended | chain_context_lost | chain_hop_limit | lineage_limit | caused_by_ignored | plugin_payload_invalid …
  run_id TEXT, details TEXT NOT NULL CHECK (json_valid(details))
);
CREATE INDEX audit_ts  ON audit (ts);
CREATE INDEX audit_run ON audit (run_id);
```

Settings are one document (`kind = 'settings'`, fixed id = the nil UUID; the primary key is `(kind, id)`, so it does
not collide with the Default queue) whose `data` is the `Settings` type of spec 02 §6.5, validated against its
schema (§16).
`view_state` documents (§18) are opaque to the engine. `export_meta` records the last export/import.

### 13.3 Migrations

- Two version numbers: the **database schema version** in `meta.schema_version` (DDL steps in Rust, applied in one
  transaction each, recorded in `migrations`) and the **per-kind document `schemaVersion`** of spec 02 §7.3,
  migrated lazily on load and rewritten on the next write of that document.
- Before a DDL step runs, the storage thread takes a backup of `config.db` as `backups/config-<schema>-<ts>.db`
  with the §13.6 procedure (never a file copy; spec 02 §7.3); the last 5 pre-migration backups are kept. The
  migration starts only after the backup has been verified. `history.db` is not backed up (it is a cache of the past; a failed migration
  deletes and recreates it with a warning).
- A database with a **newer** schema version than the app opens read-only with `system.storage_readonly { reason:
  "newer_schema" }` and the UI shows the downgrade message; nothing is rewritten.

### 13.4 Export and import

`system.export { kinds?, ids? }` (tier 0) writes a JSON bundle `{ kind: "export", schemaVersion, exportedAt, app:
{ version }, documents: [...wrapped documents], globals?: [{ name, value, persisted, untrusted, updatedAt }] }`
with secrets redacted (the `untrusted` flag travels with every value, §21), human-readable
(pretty-printed, sorted keys; row 20.13), to a temporary file and returns `{ exportId, bytes, documents }`; the
bundle is downloaded over REST (`/api/v1/exports/<id>`, spec 06) because 1,000 actions exceed spec 02's 1 MiB
message default. The UI saves it where the user chooses (spec 07). `system.import { bundle, mode:
merge | replace_by_id }` (tier 2, draft-first under MCP) validates every document strictly, runs document
migrations, keeps ids (so re-importing updates in place), bumps revisions, imports a `globals[]` entry without an
`untrusted` flag as `untrusted: true` (§21), and returns an import report. Spec 15
converts Streamer.bot and SAMMI formats into this bundle; the engine never parses those formats.

### 13.5 Startup

1. **Inspect before writing.** A missing or empty `config.db` is **bootstrapped**: the app's current schema
   (the current DDL, no migration steps) is created in one transaction with `meta` rows `schema_version`,
   `instance_id`, `created_at` and `clean_shutdown = false`. Any other file is first opened over a
   **read-only** connection: `PRAGMA quick_check` must return `ok` and `meta.schema_version` must be readable,
   else the file is **corrupt** and handled by §13.6 (the inspection connection is closed without a
   checkpoint, `config.db` is moved aside together with its `-wal` and `-shm` files, untouched;
   `system.storage_corrupt`; the newest backup is offered) and startup continues with a freshly bootstrapped
   `config.db`. A `schema_version` **newer** than the app's keeps the database **read-only** (§13.3):
   `clean_shutdown` is neither read nor written, no migration runs, no connection to `config.db` ever writes
   or checkpoints, §10.8 step 3 does not touch `config.db`, and the bytes of `config.db` and its `-wal` are
   never modified.
2. **Marker, then migrate.** Only for a supported (same or older) schema, reopened read-write: read
   `meta.clean_shutdown` (a file bootstrapped in this start counts as clean: a first start reports no crash),
   then write it to `false` and commit **before any migration** (it is set back to `true` only by the last statement
   of a successful final flush, §10.8 step 3, so a crash during migration is reported too). Run DDL migrations
   (with backup, §13.3), load all documents, migrate document versions, validate; an invalid document is loaded
   as **disabled** with a `system.document_invalid` event rather than refusing to start (one broken action must
   not take the app down).
3. Load persisted globals into memory; load the queue index, the trigger index, timers and cron schedules;
   restore the cron cursors (§8.3).
4. Open `history.db` (recreate if corrupt).
5. Publish `system.storage_corrupt` or `system.storage_readonly` if step 1 raised it, then `system.started`
   (and `system.crash_recovered { previousShutdownClean: false, reason }` first if the value read in step 2
   was `false`). Budget: §20.

### 13.6 Backups and corruption

**A backup is never a file copy.** With `journal_mode=WAL` committed rows can live only in `config.db-wal` until
the next checkpoint, so copying `config.db` silently loses them (the r1 review reproduced it: source 1 row,
copy 0 rows). The storage thread produces every backup, pre-migration (§13.3) and daily, by the same procedure:

1. `VACUUM INTO '<backups>/.<name>.tmp'` on its own connection: SQLite writes a transactionally consistent
   snapshot of the committed state, WAL included, into a fresh, compacted file, without blocking readers.
   `sqlite3_backup` (rusqlite `Backup`) from the same connection to the temp file is the permitted
   equivalent; both are serialized on the storage thread, so no write interleaves.
2. **Verify** the temp file: open it read-only, `PRAGMA integrity_check` must return `ok`, `meta.schema_version`
   must equal the source's, and the row counts of `documents` and `globals` must equal the source's as of the
   snapshot.
3. `fsync` the temp file, rename it atomically to its final name, `fsync` the directory. A failed step deletes
   the temp file and publishes `system.storage_backup_failed { reason }`; a pre-migration backup failure
   aborts the migration (§13.3).

`engine.storage.dailyBackup` (default on) runs the procedure on the first write after 24 h have passed since
the last backup (checked on writes, not by a timer), keeping 7. Restoring (spec 07 UI, engine stopped) copies the
backup over `config.db` **after deleting any stale `config.db-wal` and `config.db-shm`**, so the restored file is
not paired with a WAL from the broken database. The engine never writes to a cloud location (Q-C5, answered
2026-10-19: local backups plus "export to a folder", no hosted backups; the folder is the user's). `PRAGMA
quick_check` runs at startup; a failing `config.db` is moved aside and the newest backup is
offered through the UI (`system.storage_corrupt`). §22.8 repeats the review's reproduction as a positive test:
commit with auto-checkpoint disabled, back up, open the backup, the row is there.

## 14. Authorization, attribution and loop protection

This section lists what the engine does to honour spec 02 §8.8; the rules themselves are not restated.

### 14.1 Contexts and synthetic runs

Every run owns an `Arc<AuthContext>` fixed at admission (rule 1). Every operation call that is not inside a run
(WS, REST, MCP, `batch.execute` entries, plugin host calls) is wrapped by den-server or the plugin host in a
**synthetic run**: the engine mints a `RunId`, stores `(RunId, Arc<AuthContext>)` in the **attribution table**,
executes the operation with that `RunId` as the causing run, and keeps the entry for `engine.run.attributionWindowMs`
(300,000) after completion. Synthetic runs are not written to Action History (they have no action), but their
audit entries are. Real runs stay in the attribution table for the same window after completion.

### 14.2 Stamping and deriving

The bus stamps `causedBy` (§5.1). When the matcher fires a trigger on an event that carries a context, it derives
the run's context per rule 5 (`maxTier`, `scopes`, `approvals = []`, `lineage`, `chain`). When an event's
`causedBy` names a run that is no longer in the attribution table, **no trigger fires** and the engine audits
`chain_context_lost` (rule 5), while the event is still delivered to WS subscribers and other consumers.

### 14.3 Loop protection

- **Hop limit:** a derived context whose `chain.hops` would exceed `engine.run.maxChainHops` (16) is not created:
  the event is delivered to subscribers, no trigger fires, `chain_hop_limit` is audited once per origin run, and
  the UI shows a notice naming the origin action. Every principal is subject to it, user tokens included: a
  self-triggering loop (A sets a global, a trigger on `variables.global_updated` runs A) stops after 16 hops.
- **Inline recursion** is refused by depth only, at `maxInlineDepth` (32), never by cycle detection (§9.8: a cycle
  is legal content and a guarded loop is a supported pattern); **queued lineage** is refused at 64 (§9.8); the
  **node-execution budget** (§9.6) bounds the total work of any recursion or fan-out.
- **Storm guard:** when one trigger fires more than `engine.run.stormThreshold` (200) runs within 10 s from
  caused events (`hops ≥ 1`), the engine publishes `system.trigger_storm { triggerId, actionId, count }` once per
  minute; it does not disable anything by itself (the user decides; spec 07 offers "disable this trigger").

### 14.4 Audit

The `audit` table (§13.2) receives one row for every authorization refusal, approval mismatch, unattended stop,
lost-context event, ignored `causedBy`, hop/lineage limit and invalid plugin payload, with the run, node, origin
and chain. `system.audit { since?, kinds?, limit }` (tier 0, scope `admin`) reads it; spec 06/13 build the
approval UI on it.

## 15. Operations and events owned by this spec

All are catalog operations (spec 02 §8.1) with the usual generated `dryRun`; tiers follow spec 02's rules
(document writes are tier 2 and draft-first under MCP; runtime state is tier 1; reads are tier 0).

| Operation | Tier | Expose (rpc / subAction / mcp) | Notes |
|---|---|---|---|
| `actions.run { action: ActionRef, args?, queue?: QueueId }` | 1 (effective = action's) | ✓ / — (use `RunAction`) / ✓ | returns `{ runId, state }`; `queue` overrides the action's queue for this root run only (§10.2 *Target queue*); scope `actions:run` |
| `actions.get`, `actions.list`, `actions.effective_tier` | 0 | ✓ / — / ✓ | |
| `actions.create/update/delete` | 2 | ✓ / — / ✓ | spec 02 §6.1 draft rules |
| `actions.set_state { action, enabled }`, `actions.set_node_state { action, node, enabled }`, `actions.set_node_weight`, `triggers.set_state { action, trigger, enabled }` | 2 | ✓ / ✓ / ✓ | document writes with revision bump (rows 2.30, 2.39, 2.13) |
| `actions.set_group_state { group, enabled }` | 2 | ✓ / ✓ / ✓ | every action whose `group` path starts with `group` |
| `triggers.test`, `events.simulate`, `events.replay` | 1 | ✓ / ✓ / ✓ | §6.5, §12.5 |
| `events.emit { name, data?: object, user?: UserRef }` | 1 | ✓ / ✓ / ✓ | publishes `core.custom { name, data }` (row 2.36, below); scope `events:emit` |
| `runs.list { state?, action?, since?, limit?, cursor? }` → `{ runs, nextCursor? }`, `runs.get { runId, trace?, snapshots? }` | 0 | ✓ / — / ✓ | pending view = `state: [queued, running]`; order, keyset paging, the live/history merge and the size rules in §12.6 |
| `runs.cancel`, `runs.replay`, `runs.clear_history` | 1 | ✓ / ✓ / ✓ | `runs.replay` effective tier = action's |
| `queues.get/list` | 0 | ✓ / — / ✓ | with runtime counters |
| `queues.create/update/delete` | 2 | ✓ / — / ✓ | |
| `queues.pause/resume/pause_all/resume_all/clear/force_next/shuffle` | 1 | ✓ / ✓ / ✓ | |
| `variables.get/list` | 0 | ✓ / ✓ / ✓ | `variables:read[:ns]` |
| `variables.set/delete/clear_temp` | 1 | ✓ / ✓ / ✓ | `variables:write[:ns]`; `set` has `writesArgs: None`; results `{ value, created, untrusted }` / `{ deleted }` / `{ count }` (§11.2 readback rule) |
| `variables.set_arg` | 1 | — / ✓ / — | `writesArgs: Named { nameParam: "name" }` |
| `signals.send`, `signals.wait` | 1 | ✓ / ✓ / — | `wait` is not an MCP tool (it blocks) |
| `commands.*` (§7.2) | 2 / 1 / 0 | ✓ / set_state, reset_counter, set_cooldown / ✓ | |
| `timers.create/update/delete/get/list`, `timers.set_state`, `timers.set_interval` | 2 (reads 0) | ✓ / ✓ / ✓ | |
| `timers.reset_counter` | 1 | ✓ / ✓ / ✓ | |
| `system.export`, `system.import`, `system.audit`, `system.reset_state` | 0 / 2 / 0 / 1 | ✓ / — / export, audit | `reset_state` clears temp globals, cooldowns, signals and publishes `system.reset` |
| `settings.get {}` | 0 | ✓ / — / ✓ | `{ settings, revision, updatedAt, updatedBy, source, pendingRestart }`; scope `settings:read` (§16); **new in r6 with `settings.update` and `settings.updated`, proposals until spec 02 r7 is reviewed** |
| `settings.update { settings: Settings, revision? }` | 2 | ✓ / — / ✓ | whole document, draft-first under MCP; `{ settings, revision, changed, applied, restartRequired, warnings }`; scope `settings:write` (§16) |
| `templates.parse { source, mode: text \| value }` | 0 | ✓ / — / ✓ | returns `TemplateParseResult` (spec 02 §10, A15): the editor's template lint (§17); no scope beyond an identified session; `idempotent`; **new in r6, a proposal until spec 02 r7 is reviewed** |

Events: `system.started`, `system.stopping`, `system.crash_recovered`, `system.reset`, `system.events_dropped`,
`system.subscriber_stalled`, `system.trigger_storm`, `system.document_invalid`, `system.storage_readonly`,
`system.storage_corrupt`, `system.storage_backlog`, `system.storage_backup_failed`,
`runs.queued/started/node_started/node_completed/completed`, `queues.changed/run_rejected/run_dropped`,
`variables.global_updated/user_updated`, `settings.updated`, `commands.triggered/cooldown`, `schedule.timer_fired`,
`schedule.cron`,
`core.test`, `core.custom`. None of them carries a reserved run-context name or `platform` at the top level of
its payload (§7.1).

**User-defined events (row 2.36).** `events.emit` publishes **`core.custom { name /* ^[A-Za-z_][A-Za-z0-9_.-]{0,63}$
*/, data }`** through the bus like any event: `causedBy` is the emitting run (or synthetic run), so chains and the
hop limit apply, and the payload's top-level fields `name` and `data` seed args as for every event (spec 02
§6.3), so templates read `{data.points}`; `data` keys are free-form and never collide with reserved roots. The
importer maps Streamer.bot custom-event arguments to `{data.*}`. Actions listen with trigger type `core.custom` and filter
`name eq …` (the editor's "Custom event" trigger; `name` has catalog specificity 4 so a named trigger beats a
catch-all). Scripts call it through spec 04's facade; Streamer.bot `TriggerCodeEvent`/`Trigger Custom Event`
import to it. There is no registration step: a trigger on an unknown name simply never fires. `runs.node_*` and
`variables.user_updated` are `highVolume`; `system.stopping`, `system.subscriber_stalled` and `system.storage_*`
are `critical`.

## 16. Settings (document `kind: settings`, schema-generated form)

The engine's settings are **one persisted document** (`kind = 'settings'`, id = the nil UUID, `schemaVersion` 1,
§13.2) whose `data` is the `Settings` type of spec 02 §6.5: one section per owning spec, `engine: EngineSettings`
in v1 (spec 06 and later specs add their own sections additively, spec 02 §7.2). A key in the table is the JSON
path of the field inside `data` (`engine.run.maxConcurrent` is `data.engine.run.maxConcurrent`), and that dotted
form is what `fieldErrors[].path`, the `settings.update` result lists and the `settings.updated` event use. The
schema carries every default and range below (`default`, `minimum`, `maximum`), the `x-ui` hints and
`x-den-restart: true` on the keys that take effect only after a restart; the UI form is generated from it and
`den-protocol` exports the restart-only keys as a constant list, so the form, the engine and the tests read one
source.

| Key | Default | Range | Used in |
|---|---|---|---|
| `engine.bus.capacity` / `internalCapacity` (restart) | 8192 / 16384 | 256–65536 | §5.1, §5.3 |
| `engine.bus.subscriberCapacity` (restart) / `criticalLane` / `subscriberStallMs` | 4096 / 64 / 5000 | 64–65536 / 8–1024 / 1000–60000 | §5.3 |
| `engine.run.maxConcurrent` | 256 | 1–4096 | §9.13, §10.3 |
| `engine.run.maxTasksPerRun` | 64 | 1–1024 | §9.6 |
| `engine.run.maxRunBytes` | 33554432 | 1048576–1073741824 | §9.6 |
| `engine.run.maxNodeExecutions` | 1000000 | 1000–100000000 | §9.6 |
| `engine.run.opTimeoutMs` | 30000 | 1000–600000 | §9.3 |
| `engine.run.codeTimeoutMs` | 5000 | 100–600000 | §9.9 (spec 04 may cap lower) |
| `engine.run.maxLoopIterations` | 10000 | 1–1000000 | §9.5 |
| `engine.run.maxDelayMs` | 3600000 | 1000–86400000 | §9.7 |
| `engine.run.maxInlineDepth` / `maxLineage` / `maxChainHops` | 32 / 64 / 16 | 1–128 / 1–256 / 1–64 | §9.8, §14.3 |
| `engine.run.maxArgsBytes` | 4194304 | 65536–67108864 | §9.13 |
| `engine.run.attributionWindowMs` | 300000 | 10000–3600000 | §14.1 |
| `engine.run.stormThreshold` | 200 | 10–10000 | §14.3 |
| `engine.queue.defaultMaxPending` | 1000 | 0–100000 | §10.2 |
| `engine.queue.maxRunCeilingMs` | 3600000 | 1000–86400000 | §9.11 (ceiling for `Queue.maxRunMs`, A9) |
| `engine.signals.queuedTtlMs` / `maxWaitMs` | 30000 / 30000 | 1000–300000 | §9.12 |
| `engine.variables.maxValueBytes` | 262144 | 1024–4194304 | §11.4 |
| `engine.storage.flushMs` | 1000 | 0 (sync) – 10000 | §11.4 |
| `engine.storage.dailyBackup` | true | boolean | §13.6 |
| `engine.history.maxRuns` / `maxAgeDays` / `maxSnapshotBytes` / `traceRuns` | 10000 / 7 / 262144 / 200 | 100–1000000 / 1–(Q-E9) / 4096–1048576 / 0–10000 | §12 |
| `engine.events.maxAgeHours` / `maxCount` | 24 / 50000 | 1–(Q-E9) / 1000–10000000 | §12.5 |
| `engine.shutdown.graceMs` | 5000 | 0–60000 | §10.8 |
| `engine.action.maxNodes` / `maxDepth` | 2000 / 32 | 50–20000 / 4–64 | §9.13 |

The ranges of the `engine.history.*`, `engine.events.*` and `engine.action.*` rows and the lower bound of the
`engine.signals.*` row were added in r6 (the earlier revisions gave only their defaults). They are **validator
bounds** chosen by this spec, not product limits, with one exception: how long a user may keep Action History
and the event log is a product choice (Q-E4 accepted the **defaults** only; Q-H1 answers **chat** retention
only), so the upper bounds of `history.maxAgeDays` and `events.maxAgeHours` are **open as Q-E9** (§24) and the
table shows no number for them. Until the owner answers, the validator enforces only the lower bound of those
two keys and the implementation cards (DEN-146, DEN-118, DEN-119) carry no upper bound for them; the defaults
(7 days, 24 h) are unchanged either way. `maxSnapshotBytes` and `maxNodes` bound what is **stored**; transport
is bounded separately by the session's message budget (§12.6), which is why their ceilings are plain storage
limits and the `message_size` warning below is advice. A value outside its range is **rejected, never
clamped**: `settings.update` fails with `INVALID_PARAMS` and stores nothing, and a stored document with a key
outside its range is invalid as a whole (the startup rule below). The defaults are unchanged.

**Operations.** Both are catalog operations (§15, spec 02 §8.1) in the reserved `settings` namespace, exposed on
WS/REST and MCP and never as sub-actions (a run does not reconfigure the engine that is executing it):

- `settings.get {}` (tier 0, scope `settings:read`) returns `{ settings: Settings, revision: u64, updatedAt:
  Timestamp, updatedBy: UpdatedBy, source: "document" | "defaults", pendingRestart: Vec<String> }`.
  `pendingRestart` names the restart-only keys whose stored value differs from the **boot value** (the value the
  running engine sized its channels with at start); `source` is `defaults` while the engine runs on the built-in
  defaults because the stored document is unusable (below). `settings` is always a **valid `Settings` value**:
  with `source: "document"` it is the stored document, `revision`, `updatedAt` and `updatedBy` are its metadata,
  and `pendingRestart` compares it with the boot values; with `source: "defaults"` it is the **built-in defaults
  the engine actually runs on** (never the stored bytes, which may not satisfy the schema), `reason: "invalid" |
  "newer_schema"` says why (present only in that state), `revision`, `updatedAt` and `updatedBy` are the
  metadata of the persisted record that was set aside (its body is untouched until a successful
  `settings.update` replaces it, or never under `newer_schema`), and `pendingRestart` is empty, because the
  engine booted on those same defaults. The diagnostics of the rejected body travel in
  `system.document_invalid` (§13.5), not in this result. Stored document, effective configuration and
  provenance are read together under the settings writer (below), so one response never mixes two
  revisions.
- `settings.update { settings: Settings, revision?: u64 }` (tier 2, scope `settings:write`, draft-first under MCP
  as a `PendingChange { targetKind: "settings", operation: "settings.update" }`, spec 02 §6.1) replaces **whole
  sections**: inside a section (`engine.run`, `engine.bus`, …) every field is `required`, so a partial section is
  `INVALID_PARAMS` rather than a silent reset of the fields it omits (the defaults exist for the first start and
  for the form's *Reset to defaults*, which sends them explicitly); a top-level section that is absent from
  the call keeps its stored values, so a client pinned to an older `rpcVersion` keeps working when a later spec
  adds a section (spec 02 §7.2 forbids new required request fields). The stored `PendingChange.params` omit
  `revision`, so *Apply* days later does not fail on a stale revision; the Apply screen shows the diff against
  the then-current document instead. The call validates ranges (`INVALID_PARAMS` with one `fieldErrors` entry per
  offending field, whose `path` is the ordinary JSON Pointer into the **request** params, for example
  `/settings/engine/run/maxConcurrent`, spec 02 §8.5; nothing is stored), then hands the write to the
  engine's **settings writer**: one owned task (started with the engine, stopped by §10.8 step 3) that consumes
  a bounded request queue in order and runs, for each request, the sequence *compare revision → commit → swap
  effective config → publish* to completion **independently of the requesting future**. The caller only
  submits `(request, cancellation token, completion channel)` and awaits the completion; callers never hold a
  lock across an `await`, and the writer never holds a synchronous lock across one either. The `revision`
  check (`CONFLICT`, `details.reason = "revision"`, when it is not the stored one) and the store happen in the
  same storage transaction (`revision + 1`, `updatedBy` from the context, A7); only after the commit is the
  new document swapped into the effective configuration (hot-apply of every changed key that is not
  restart-only; restart-only keys keep their boot value in the effective configuration) and `settings.updated {
  revision, changed, restartRequired, updatedBy }` published (an engine-internal event, §5.1: non-blocking, so
  it can be dropped and counted under internal-lane overflow, in which case the stored revision is still the
  truth and `settings.get` shows it). Because one task does all of it in queue order, updates with or without
  `revision` always commit, apply and publish in revision order and the effective configuration never
  regresses behind the stored one. **Cancellation boundary:** the writer checks the request's token
  immediately before it opens the storage transaction; a request cancelled before that point is discarded with
  no durable write. From that point on the request is **accepted** and runs to completion whatever happens to
  the caller: a caller whose future is dropped after submission learns the outcome through `settings.get`
  (the stored `revision`) and `settings.updated`, nothing is rolled back and nothing is pretended undone. A
  storage failure inside the transaction swaps and publishes nothing and the completion carries the error.
  Queue capacity is small and fixed (16 requests); a caller that finds it full awaits a slot (the must-land
  class of §13.1), never fails. At shutdown the writer finishes the accepted request it is running, rejects
  the rest of the queue and every later submission with `INVALID_STATE` (`details.reason = "shutting_down"`),
  and exits before the final flush. The
  result is `{ settings, revision, changed: Vec<String> /* keys whose value differs from the previous document
  */, applied: Vec<String> /* changed keys in effect now */, restartRequired: Vec<String> /* restart-only keys
  whose new stored value differs from the boot value; an undo back to the boot value removes the key and is
  reported as an empty list */, warnings: Vec<SettingsWarning { key: Option<String>, code: String, message:
  String }> }`; the event carries the same `changed` and `restartRequired`, and `pendingRestart` of a later
  `settings.get` is the same set computed over the whole document. The lists and `SettingsWarning.key` use the
  **dotted document key** (`engine.run.maxConcurrent`); only `fieldErrors[].path` uses the request pointer, and
  §22.13 has a fixture showing both forms for the same field. Warnings never block:
  `memory_budget` when `engine.run.maxConcurrent × maxRunBytes` exceeds half of physical memory (§9.6),
  `below_running_value` when a tightened limit is already exceeded by live state (for example `maxConcurrent`
  under the current number of running runs), `message_size` when three snapshots at `maxSnapshotBytes` plus a
  record could exceed the default message budget (§12.6). `dryRun` returns the usual
  `DryRunResult` with `diff` =
  `{ "<key>": { "from", "to" } }` over the changed keys and the same warnings as strings.
- **Hot-apply** means the new value is read at the next decision that consults it, never retroactively: a lower
  `maxConcurrent` dispatches nothing until enough runs have ended, a lower `maxRunBytes` or `maxNodeExecutions` is
  enforced at the next check inside a run, a new `flushMs` re-arms the next flush, new retention limits apply at
  the next write-triggered prune, a new `graceMs` applies to the next shutdown. No running run is cancelled by a
  settings change. The restart-only keys are the three bus capacities (`engine.bus.capacity`, `internalCapacity`,
  `subscriberCapacity`): channels are sized once at start, so their **boot value** is what the effective
  configuration holds until the next start, whatever the stored document says. Every other key hot-applies.
- **Startup.** A missing document is created with the defaults (`revision` 1, `updatedBy: system`). A stored
  document that fails validation (schema or range) is **not** used: the engine runs on the defaults, publishes
  `system.document_invalid { kind: "settings", … }` (§13.5), reports `source: "defaults", reason: "invalid"`
  with the defaults as `settings` from `settings.get` (above), keeps the rejected record in place, and the next
  successful `settings.update` replaces it (sections the call omits come from the defaults then, since there is
  no usable stored section). A document with a newer `schemaVersion` than the app knows is read-only (spec 02
  §7.3): the engine runs on the defaults the same way with `reason: "newer_schema"`, and `settings.update`
  fails `INVALID_STATE` (`details.reason = "read_only"`). Document migrations add new keys with their defaults
  (spec 02 §7.3).

## 17. UI flows (engine-visible parts; spec 07 owns the screens)

- **Action editor save:** `actions.update` validates the tree (schema, limits of §9.13, `BreakLoop` outside a loop
  is a lint, not a refusal), bumps `revision`, rebuilds the trigger index, and returns the stored document.
- **Test trigger (F5)** → `triggers.test`; **Simulate** → `events.simulate`; the editor then opens the run from
  `runs.queued` and follows `runs.node_*`.
- **Pending view** → `runs.list { state: [queued, running] }` + `runs.*` events; **cancel** → `runs.cancel`.
- **Action History** → `runs.list` + `runs.get` (snapshots, trace) + `runs.replay`; "Inspect variables when
  queued / after run" are the two snapshots.
- **Queues panel** → `queues.list` + `queues.changed`; pause/resume/clear/blocking toggle.
- **Variable viewer** → `variables.list` (tabs: persisted, temp, per-platform users) + `variables.*_updated`.
- **Approval UI** (spec 06/13) reads `system.audit` for stopped chained runs and re-runs through `actions.run`.
- **Settings form** (generated from the `Settings` schema) → `settings.get` + `settings.update` (whole document;
  the restart badge comes from `x-den-restart` and `pendingRestart`); other windows refresh on `settings.updated`.
- **Template lint in the editor** → `templates.parse { source, mode }` per field, debounced and batched with
  `batch.execute`; the editor never embeds a second parser (§15; spec 02 §10).

## 18. Node-editor round-trip constraints (D-028, row 2.52)

The sub-action tree is the only persisted semantics; the graph is a view (research Part B). The engine guarantees
and requires:

1. **Layout never changes `revision`.** Graph positions, collapsed state and viewport are stored in a separate
   `view_state` document keyed by the action id (§13.2), which the engine never reads. An approval bound to a
   revision therefore survives rearranging nodes.
2. **Structured control flow only.** Every graph the editor saves is a tree; the engine accepts only the `Node` kinds
   of spec 02 §6.4 (plus the §23 amendments). There are no gotos, no exec fan-out except `Parallel`, no back-edges
   except `While` (and `ForEach`, A2), no edges leaving an `If`/`Switch`/`While`/`Group` region except its
   continuation. Research B3.3 lists the editor-side validation; the engine's save-time validator enforces the
   subset that is expressible in the document: depth and node limits, `BreakLoop`/`ContinueLoop` placement
   (warning), and `Parallel` branches that are non-empty.
3. **Data edges are args.** A graph data edge from node P's output to node C's param is stored as `saveAs: "x"` on
   P and `{x}` in C's template; the engine's writer model (spec 02 §8.8) and the "may be undefined" rule (a read
   of a name no dominating node wrote renders as missing, §9.10) are what the editor's amber warning describes.
4. **Deterministic `Parallel` join** (§9.6) so the graph's Join port has one meaning.
5. **Per-node trace events** (§12.4) with stable `NodeId`s, so a live run can be highlighted in either view.
6. **Round-trip property:** for every built-in example and every imported action in the test corpus, `tree → graph
   → tree` is the identity (spec 07/16 test; the engine's contribution is that node ids are stable across saves and
   that `actions.update` never reorders or renumbers nodes).

## 19. Cross-platform notes (D-035)

- **Data directory:** Linux `$XDG_DATA_HOME/streamden` (default `~/.local/share/streamden`), Windows
  `%LOCALAPPDATA%\StreamDen`, macOS `~/Library/Application Support/StreamDen`; overridable with `--data-dir` and
  the `STREAMDEN_DATA_DIR` environment variable (portable mode). Paths are stored and compared as the OS gives them.
- **Data directory ownership:** `den-core` resolves it (`DataDir::resolve(explicit: Option<PathBuf>)`, returning
  the directory and a list of warnings): the explicit path wins, then `STREAMDEN_DATA_DIR`, then the OS
  default above. The apps (`apps/desktop`, `streamdend`) parse `--data-dir` and pass it, nothing more; `den-core`
  reads the environment variable itself so that every embedder, test harnesses included, honours the portable-mode
  contract the same way. Resolution creates a missing directory (`0700`, §21) and fails startup with a clear
  error when the path is a file or not writable; it returns, instead of refusing, the warnings `network_share`
  (a UNC path, or a mount whose file-system type a portable mount-table crate reports as a network file system;
  a string and table check, no `cfg` block) and `synced_folder` (Dropbox, OneDrive, iCloud Drive, Nextcloud
  markers) that the app shows (the SQLite bullet below). `den-core` owns `config.db` and `history.db` with their
  `-wal`/`-shm` files, `backups/` and the temporary export files (`exports/`, §13.4); other crates and the apps
  keep their files directly under the data directory under their own names and never inside those.
- **SQLite:** bundled, so the version is identical on every OS; WAL works on all three. On Windows the `-wal` and
  `-shm` files must live next to the database (no network shares; the UI warns when the data dir is on one).
  Synced folders (Dropbox, OneDrive) are detected heuristically and warned about, as Streamer.bot's docs do.
- **Clocks:** monotonic timers on every OS; suspend detection compares monotonic and wall deltas on every scheduler
  wake. Cron time zones come from the IANA database bundled with the `chrono-tz`/`jiff` crate, not the OS, so
  behaviour is identical across platforms; "local" resolves via the OS at each computation.
- **Signals and shutdown:** `SIGTERM`/`SIGINT` on Unix and `CTRL_CLOSE_EVENT`/service stop on Windows all run the
  §10.8 sequence; `streamdend` as a systemd user unit gets `TimeoutStopSec` ≥ grace + 5 s.
- **File permissions:** `0600`/`0700` on Unix; on Windows the data directory lives under the per-user
  `%LOCALAPPDATA%` ACL and nothing extra is set; macOS follows Unix.
- **OS gating in `den-core`:** `den-core` has no OS-specific behaviour or feature (OS-specific sources such as
  hotkeys, MIDI and file watching are connectors; the signal handling above belongs to the apps) and exactly one
  `cfg`-gated block: the file-mode helper in `storage/paths.rs` (`cfg(unix)`), which creates the data directory
  and `backups/` `0700` and **pre-creates** each database file `0600` before SQLite opens it (an empty file is
  bootstrapped as fresh, §13.5), applies `0600` to the backups and exports it writes, and is a no-op on other
  targets (Windows relies on the `%LOCALAPPDATA%` ACL, the bullet above). SQLite creates the `-wal` and `-shm`
  files with the database file's mode. The Windows CI leg compiles and runs the storage tests, so the no-op path
  is covered (§22.11).

## 20. Performance budgets (reference box; architecture §7)

| Metric | Budget |
|---|---|
| Event publish → run `Running` (non-blocking queue, matching trigger, no approval) | **< 10 ms p99**, < 1 ms p50; breakdown: bus hop < 50 µs, match < 200 µs with 10,000 indexed triggers, admission + dispatch < 300 µs, task start < 500 µs |
| Matcher throughput | 5,000 events/s sustained with 1,000 actions, < 20% of one core |
| Per-node executor overhead (excluding the operation itself) | < 100 µs p99: render < 20 µs and invocation-schema validation < 50 µs (spec 02 §15), snapshot pin, writes and trace event < 30 µs together |
| `variables.set` (persisted) | < 20 µs to memory; durable within `flushMs`; 10,000 writes/s sustained without back-pressure on runs |
| History write | off the hot path; a run's record and snapshots cost < 100 µs on the storage thread |
| Startup with 1,000 actions / 10,000 triggers / 50,000 globals | < 500 ms to `system.started` (including spec 02's < 200 ms migration) |
| Idle | no periodic wake-up at all when nothing is scheduled (pruning, backups and drop reports are write-triggered, §12.2, §13.6, §5.3); engine core contributes < 0.1% of one core to the < 1% idle budget; < 30 MB RSS for the engine core data structures at the sizes above |
| Memory per queued run | < 4 KiB plus its args snapshot |
| 10,000 queued runs then `queues.clear` | < 50 ms, no stack growth |
| Runtime shutdown with 20,000 waiting runs | < 1 s, no stack overflow (B-1 regression) |

## 21. Security

- Everything spec 02 §16 requires of the engine: contexts never widen; `causedBy` only from the bus; every node
  re-checked; approvals bound to the plan; synthetic runs for direct calls; `chain_context_lost` fail-closed.
- Variables and args are **data**: a template cannot execute anything, and `Code` nodes receive values, not
  templates.
- **Untrusted-text provenance.** Spec 02 §16 requires `x-den-untrusted` text to stay wrapped for AI consumers and
  never to satisfy an approval; the engine carries that provenance through everything it stores, so the boundary
  does not depend on authors remembering to wrap chat text:
  - **Tainted sources of a run:** the `event` and `user` root paths the catalog marks `x-den-untrusted` (chat
    text, display names, titles…) or marks `x-den-untrusted: conditional` with the sibling flag set in this
    envelope (a `variables.*_updated` event's `old` when `oldUntrusted` is true, its `new` when `untrusted` is
    true, §11.2), the args seeded from them, the `match` root whenever its captures come from
    a regex over an untrusted field (a chat-text trigger filter, a regex command's `captures`, a `Condition`
    regex whose `left` is tainted), every stored variable whose `untrusted` flag is set (§11.1), and every arg
    the rules below taint. Each run keeps its set of tainted arg names; history stores it per snapshot (§12.1).
  - **Propagation inside a run:** an arg is tainted when its writer's source references a tainted source (the
    same syntactic reference check as spec 02's dynamic fields, through `writesArgs`, `saveAs`, `RunAction.args`
    seeds, `ForEach` collections and `Code.setArgs`, which taints every name it writes); a `saveAs` or
    `writesArgs` of an operation result is also tainted when a conditionally marked field of that result has
    its flag set (a `variables.get`/`set` result with `untrusted: true`). A signal carries the
    taint of each of its `args` from the sending run (or taints all of them when the sender is a `Plugin`
    principal), and `signals.wait { overwrite }` applies it to the names it writes. Untainting is impossible by
    assignment: only a literal or a trusted source produces a clean arg.
  - **Propagation into variables:** `variables.set` (sub-action, `Code` facade, or any direct call) writes
    `untrusted = true` when its value references a tainted source, when the writing `Code` node's run has any
    tainted arg, when the caller's principal is `Plugin` (plugin outputs are untrusted text, spec 02 §3), or
    when the context's origin principal is `Mcp`: `variables.set` is tier 1, not draft-first, and an MCP client
    that has just read a wrapped chat value could otherwise write it back as a trusted literal, which the engine
    cannot tell from genuine AI-authored configuration. The flag is `false` only for a literal value written
    under a `Token` or `System` origin, or by an unchained `Trigger` run (an event-started run with no `causedBy`
    chain, which spec 02 §6.1 attributes to `user`; with a chain the chain's origin principal decides). That is
    the user's own content; the variable viewer shows the flag, and a user-token rewrite is how an AI-written value becomes trusted. `increment`/`decrement` keep the existing
    flag; `default` on a missing entry follows the write rule, and on an existing entry writes nothing and
    returns the existing value **with its flag** (§11.2). The flag is stored in the same row (§13.2), survives restart, travels
    in `variables.*_updated` (independent `oldUntrusted`/`untrusted`), `variables.get/list/set` results and the export bundle, and a foreign import (spec 15) that
    carries no flag imports every variable as `untrusted: true` (unknown provenance is untrusted; the user
    re-writes a value to clear it).
  - **Reads:** a template read of a tainted variable is a tainted source (§11.3); every operation result or
    event field that carries a stored value carries its entry's flag, MCP wraps it on that flag, and a run
    seeded or `saveAs`-written from it is tainted on that flag (§11.2 readback rule: `get`, `list`, `set`
    including `default` on an existing entry, and `old`/`new` of `variables.*_updated` with their own flags);
    `runs.get` returns both snapshots' taint sets and replay re-seeds
    from the chosen one (§12.3).
  - **Plans and approvals:** every dry-run `PlanStep` and `PlannedCall` lists `untrustedFields` (A10), the param
    paths whose render references a tainted source, computed by the same syntactic rule as `dynamicFields`, so
    an approval request renders those values inside the untrusted wrapper and the AI layer (D-030) never sees
    viewer text as configuration. For a reference to a `saveAs` arg whose producer is a `variables.get`/`set`
    (a `run_value` sentinel in `dynamicFields`), the planner resolves the conditional flag **through the
    producer's `name`**: with a literal `name` it reads the stored entry's flag in the plan snapshot and lists
    the field when set; with a templated `name`, or a scope/user it cannot resolve at plan time, it lists the
    field conservatively. Execution applies the run-time flag (§11.2 readback rule), so a field the plan did
    not list can still be tainted when the variable turned untrusted in between, and never the reverse. The binding hash still covers the value: untrusted text does not bypass
    binding, it is only never presented as trusted.
  - §22.9 covers chat → `variables.set` → restart → `variables.get` over MCP (wrapped), a later run's
    `chat.send { message: {globals.last} }` dry run (`untrustedFields: ["message"]`) and approval request, an
    `increment` on a tainted counter (flag kept), an MCP-origin literal write (flag set), a literal user-token
    write clearing the flag, a replay of the queued snapshot of a run that overwrote the chat text with a literal
    (`untrustedFields` still lists the field), a `{match[1]}` from a regex command in a tier-3 param, an MCP
    `variables.set { mode: default }` on an existing tainted variable (existing value returned wrapped, no
    write, no event), and a tainted → literal overwrite whose `variables.global_updated` carries
    `oldUntrusted: true, untrusted: false` and whose `{event.old}` reaches a later tier-3 plan as untrusted.
- `x-secret` values are redacted from snapshots, trace events, exports and the event log before they leave the
  operation that produced them.
- The storage files are created `0600` on Unix; the data directory is created `0700` (Windows: per-user ACL; the
  single `cfg(unix)` helper of §19 does both). Backups and exports inherit this.
- Rejected and dropped runs are visible, so an event flood from a platform or a plugin cannot silently discard
  alerts; `events.simulate`, `events.replay` and `triggers.test` need `events:simulate` and are tier 1 so an MCP
  client's simulated event is attributed and chained like any of its calls.
- Imported actions keep `origin: imported { trusted: false }`; their `Code` nodes run in the isolated worker (D-029,
  spec 04), which this spec passes through as a flag on the `Code` call.

## 22. Test plan

1. **Trigger precedence golden tests:** the bits example (any vs exactly 100, with and without `alwaysRun`); tie
   between two equally specific triggers; one action with two matching triggers fires once; disabled, unresolved and
   draft actions never fire; `<slug>.unknown` with `upstreamType` filter; regex captures land in `match` for the
   winning trigger only; `events.simulate` honours precedence while `triggers.test` bypasses it; dominance: a
   `user.roles` trigger and an `amount` trigger both fire on one cheer, while a filterless trigger is suppressed;
   two cron triggers with different expressions never fire each other; `core.custom` named vs catch-all;
   **reserved payload keys:** a fixture per engine event type asserts no reserved run-context name or `platform`
   at the payload top level, and the catalog lint rejects a synthetic event that has one; **command captures:** a
   regex command fired by a `commandId eq` trigger seeds `match` from `captures`, a trigger with its own `regex`
   filter replaces them, a basic-mode command seeds an empty `match`; **bot accounts (§7.1):** a message from an
   account the `BotAccounts` provider lists is skipped by an `ignoreBotAccount` command and matched by one without
   the flag, a replayed message from it is skipped too, and an empty provider skips nothing.
2. **Filter operator table tests:** every `FilterOp` against every JSON type, missing fields, `between` bounds,
   number normalization, case rules.
3. **Scheduler proptests** (`proptest`, paused Tokio clock): random workloads of ≤ 64 runs with random durations,
   `alwaysRun`, cancellations, pause windows, clears, blocking toggles and overflow policies, asserting: blocking
   queues never run two at once; FIFO among eligible runs; no ordinary run starts inside a pause window; the bound
   never rejects a run that could start; `drop_oldest` evicts the oldest waiting run; counters equal outcomes;
   every run reaches exactly one terminal state. Plus the B-1 regressions: shutdown with 20,000 waiting runs
   (no stack growth), cancellation of a running run in a blocking queue on a 4-thread runtime (no overlap; slot
   released only after the future is dropped), a panicking run frees its slot, always-run on a paused and full
   queue starts; **zero capacity:** `maxPending: 0` with `drop_oldest` (via `defaultMaxPending = 0`) rejects the
   incoming run and never exceeds the bound, and the explicit combination is refused at save; **shutdown
   phase:** a `system.stopping` trigger's action runs to completion on an idle queue, on a paused queue, and on a
   busy blocking queue when the running run finishes within the grace, is cancelled `shutdown` when it does not,
   waiting runs are cancelled before the fire set is admitted, a direct `actions.run` after step 1 is
   `shutting_down`, and `clean_shutdown` is `true` only after a successful flush (a flush fault leaves `false`
   and the next start reports `flush_failed`); **engine-wide slot (§10.3):** with `maxConcurrent = 1` and three
   queues, runs admitted A1, B1, C1, A2 start in admission order as slots free, `force_next` in queue C does not
   let C2 overtake A2, a blocking queue with a running run does not compete, an `alwaysRun` run on a paused queue
   takes its turn by admission order and not before older heads, a slot release costs O(queues), not a scan
   of every waiting run, and an admission interleaved with a release while the pump is active (between its
   unlock and relock) does not start ahead of an older eligible head, also under a paused queue with
   `alwaysRun` and after `force_next`; **queue override (§10.2):** `actions.run { queue }` admits into the named queue and the
   record, `runs.queued` and `run.queueName` show it, a queued `RunAction` child goes to its target's queue,
   `runs.replay` of the run uses the action's queue, an unknown queue is `NOT_FOUND` before admission, the
   dry-run plan steps are identical with and without the override, and an approval obtained without it does not
   bind to the call with it (`binding_mismatch` / `args`).
4. **Executor semantics:** each node kind; `autoType` and `ignoreCase` matrices; `Switch` default; loop cap with
   warning; `BreakLoop` outside a loop; `Stop` in child, in branch, at top level; `Parallel` deterministic merge
   and sibling cancellation; inline arg flow in and back; queued child seeding and `queue_full` failure; recursion
   and lineage limits; `Code` `ok: false` vs exception; `onError` stop vs continue and `run.lastError`; missing
   value warnings; args size limit; **`ForEach`:** array, object (sorted keys), `null`, scalar failure, cap
   warning, `Break`/`Continue`, variables after the loop, pinned collection under concurrent writes; **bounded
   fan-out:** a 20-deep chain of actions each with two inline `Parallel` branches calling the next never has
   more than `maxTasksPerRun` **existing** branch tasks (spawned, executing or parked, counted by a spawn/exit
   hook), fails with `node_budget` at the budget, and cancels cleanly
   mid-fan-out with every spawned and inline branch `cancelled` / `parent` and the slot released once;
   **breadth-first expansion:** a 17-level binary chain of inline `Parallel` nodes whose 131,072 leaves each
   `Delay` (paused clock), run at `maxTasksPerRun` 1 and 64 under a scheduler that prefers starting new
   branches to finishing old ones: existing branch tasks (parked ones included) never exceed the budget, the
   measured task and continuation memory stays under the fixed ceiling §9.6's bounds give, the run completes
   with the same merged args and the same node charge as a sequential run, and the same chain cancelled while
   expanding ends every branch `cancelled` / `parent` with zero branch tasks left; a 100-branch `Parallel`
   with 64 permits runs every branch (64 spawned, the rest inline or spawned as permits return) and merges in
   branch order; `Parallel` nested three deep with `maxTasksPerRun = 1`, started from an idle root: the outer
   node spawns its first branch with the sole permit (the root needs none), at most one spawned branch task
   exists besides the root at every instant, a node that finds the permit taken runs its branch inline and a
   later branch may be spawned once the permit has come back, and the run completes without deadlock with the
   same merged args as with 64; the same nest placed inside a spawned branch that holds the sole permit runs
   every nested branch inline in that branch's task; `run_budget` failure when branch writes exceed `maxRunBytes` while
   each map stays under `maxArgsBytes`, including writes held by parked branches.
5. **Variables:** atomic increment under 1,000 concurrent runs; persisted/temp move; change events with
   `old`/`new` and independent `oldUntrusted`/`untrusted` (created: `null`/flag; tainted → clean literal:
   `true`/`false`; clean → tainted: `false`/`true`; deleted: flag/`null`); `set` returns the stored entry's
   flag in every mode, `delete` and `clear_temp` return no value; durability: `kill -9` after a write and before `flushMs` loses at most that write, after
   `durable: true` loses nothing (test harness with a child process).
6. **History and replay:** snapshots at admission and after; redaction; truncation marker; retention pruning keeps
   recent failures; replay uses the current revision and the caller's context and re-seeds the taint set;
   `events.replay` attribution; **event-log fidelity:** a `chat.message` from a moderator is logged, the engine
   restarts with no Action History record of it, `events.replay` fires a trigger filtered on `user.roles contains
   moderator`, a command with a role permission, and a `{user.displayName}` template, all with the stored user,
   while the current user record has lost the role; a stored older `payloadVersion` converts or fails
   `payload_version`; **`runs.list` paging (§12.6):** 2,500 terminal and 20 live runs page newest-first in pages
   of 1,000 with no duplicate and no gap, also when a live run completes between two pages, and when it completes
   before the second page's live read while its history insert is **delayed** past that read (it is returned as
   a settling entry), and when that insert is dropped (absent and counted); the cursor is opaque and a malformed
   one is `INVALID_PARAMS`; `history.pending: false` runs are absent from `state: [queued, running]` and present
   in an unfiltered query; list rows **omit** `eventSnapshot` (the key is absent, validated against the schema)
   while `runs.get` returns it; **bounded rows (§12.1, §12.6):** a run failed with a 2 MiB `error.message` and
   a 2 MiB `error.details` is stored with both cut (4,096 scalars / `maxSnapshotBytes` with the marker) and 101
   warnings become 100 plus `warnings_truncated`; a first remaining record that alone exceeds a 16 KiB session
   budget is returned as a schema-valid reduced row (`row_truncated` with a non-empty `message` naming the
   reduced fields, `error.message` ≤ 256 scalars, no `chain`, no `error.details`, empty taint lists) whose
   serialized size is ≤ `K_row`, the measured response including the envelope is under the budget, and the
   cursor advances; a session budget below `K_row` fails `runs.list` with `message_budget` and
   `details.minimum`, never an empty page; `runs.get` of a run whose three snapshots exceed the budget fails
   `PAYLOAD_TOO_LARGE` / `message_budget` / `component: "snapshots"` and succeeds with `snapshots: false`; a
   trace-only overflow fails with `component: "trace"` and succeeds with `trace: false`; a record whose bounded
   `error.details` alone exceeds a small budget fails with `component: "record"` and succeeds after the budget
   is raised.
7. **Timers and cron:** paused-clock tests for drift-free intervals, direct dispatch to the owning trigger, random ranges, `minChatLines` gate, `repeat:
   false` disabling, missed-tick coalescing after a simulated suspend; cron DST skip and fall-back, `catchUp`,
   time zone changes, leap day; **cron cursor:** a daily trigger fires, the engine restarts twice within the day
   (no replay either time), is down across the next occurrence (`catchUp: true` fires it exactly once with the
   missed `scheduledAt`; `false` skips it and moves the cursor), is down across three occurrences (one fire),
   the expression is edited while down (no catch-up); **fall-back hour** (America/Chicago, 2026-11-01, local
   00:30 CDT to 02:30 CST, paused clock): an every-minute schedule fires exactly 60 times at 01:00–01:59 CDT,
   never at 01:00–01:59 CST, and next at 02:00 CST; `*/15` fires 01:00, 01:15, 01:30 and 01:45 CDT once each;
   `30 1 * * *` fires once at 01:30 CDT; each of these with a restart at 01:20 CST (between the folds), under
   `catchUp: true` and `false`, fires nothing until its next first-fold or post-transition occurrence; and a
   daily 01:30 schedule with `catchUp: true` that is down from 01:00 CDT to 01:45 CST fires once, stamped
   01:30 CDT; a crash injected between cursor write and admission loses that
   occurrence and never duplicates.
8. **Storage:** fresh schema; every DDL migration with before/after fixtures and a backup created; newer-schema
   read-only; **startup order (§13.5):** a fresh file is bootstrapped with `clean_shutdown = false` and
   reports no crash, also when it replaces a corrupt file; a supported older schema writes the marker before the pre-migration backup and the
   first DDL step (a crash injected in the migration is reported on the next start); a newer-schema file
   publishes `system.storage_readonly`, runs no migration, and the SHA-256 of `config.db` and of a
   non-empty `config.db-wal` left by the newer app are identical before startup and after a clean shutdown;
   a `config.db` failing `quick_check` is moved aside with its `-wal`/`-shm` and unchanged bytes, the engine
   starts on a fresh file and publishes `system.storage_corrupt`; corrupt `history.db` recreated; export → import round-trip equals
   the original documents and their `untrusted` flags; invalid document loads disabled; **WAL-safe backup:**
   with `wal_autocheckpoint = 0`, commit one document and one global, take a daily backup and a pre-migration
   backup, open each copy: both rows present, `integrity_check` ok; a backup whose verification is made to fail
   leaves no final file and aborts the migration; restore deletes stale `-wal`/`-shm` first; **data directory
   (§19):** the override beats `STREAMDEN_DATA_DIR`, which beats the OS default; a synced-folder marker yields the
   warning without refusing; on Unix the directory, `backups/`, both database files and their `-wal`/`-shm`
   files after the first write are `0700`/`0600` (the Windows leg sets no mode and passes).
9. **Authorization (engine side of spec 02 §17.9):** every chain, approval and refusal case there, run against the
   real executor; attribution table expiry → `chain_context_lost`; hop limit at 16 under a user token; storm event;
   **laundering**: an MCP-origin `chat.send` echoed by the platform, attributed, matched by a command with
   `ignoreInternal: false` → the `commands.triggered` run inherits the MCP context and its `Code` node is refused;
   **approval ordering (§9.2):** with an approved `chat.send { message: {globals.msg} }`, a concurrent run
   changes `msg` (i) before the node's pin → `binding_mismatch` / `args`, (ii) after the pin and before the
   operation call → the approved value is sent, (iii) after the call → no effect, and the sent text equals the
   hashed text in every case; **`ForEach` binding:** approve `ForEach {globals.messages}` → `chat.send
   {item}` with three elements `[a, b, c]` (three `PlannedCall`s); under that one-shot approval, before the
   call: change the third element, append a fourth, remove one (`[a, b]`), or empty the collection → each
   `binding_mismatch` / `args` at the call; the same four changes made after the call started and before the
   loop → `args` at loop start, with no `chat.send` sent (the emptied collection included); an approval of
   `[]` (no body calls) and a collection that became `[a]` → `args` at the call; unchanged `[a, b, c]` with a
   `BreakLoop` after the second iteration → runs, one planned call unconsumed; under a **session grant** for
   the same plan (default cap): a changed element runs, an append or a removal → `unplanned_call`, at the call before it
   started and at loop start after, with no `chat.send` sent; **`ForEach` cap:** the same body with
   `maxIterations: 2` over `[a, b, c]` plans two calls; under a one-shot approval an
   append (`[a, b, c, d]`) → `args` at the call and at loop start (the whole collection is bound); under a
   session grant, before the call or after it started: the append, or a removal to `[a, b]` (both lengths at or
   above the cap, count 2 → 2) → runs two calls, and a removal to `[a]` (crossing below the cap, count 2 → 1)
   → `unplanned_call`, at the call before it started and at loop start after, with no `chat.send` sent; a
   session grant over `[a]` (one planned call) and a collection grown to `[a, b, c]` (count 1 → 2) →
   `unplanned_call` the same way;
   a collection from `saveAs` → `dynamicFields` on the body and a one-iteration plan; **untrusted provenance:** the §21 chain (chat → global →
   restart → MCP read wrapped → dry-run `untrustedFields` → approval request shows the wrapper), `Code` writes
   tainted, `Plugin` writes tainted, foreign import tainted, literal user write clean; **readback paths
   (§11.2 rule):** an MCP `variables.set { mode: default }` on an existing chat-derived variable writes
   nothing, publishes nothing and returns the existing value wrapped (`untrusted: true`); a user-token literal
   overwrite of that variable emits `{ oldUntrusted: true, untrusted: false }`, a trigger on
   `variables.global_updated` runs `chat.send { message: {event.old} }` whose dry run lists `untrustedFields:
   ["message"]` and whose approval request shows the wrapper, while the same action over `{event.new}` lists
   none; an MCP subscriber receives that event with `old` wrapped and `new` plain; `events.replay` of it from
   the event log after a restart seeds the same taint; a `saveAs: "v"` of `variables.get` on a tainted
   variable taints `v` and a tier-3 param rendered from `{v.value}` is listed, while the same `saveAs` on a
   clean variable is not.
10. **Performance:** criterion benches for match, admission, node overhead, `variables.set`; a soak test at 5,000
    events/s for 10 minutes checking the §20 budgets and zero idle wake-ups afterwards (`strace`/ETW count).
11. **Cross-platform:** the storage, timer and shutdown tests run on Linux and Windows CI (Windows compiles and
    runs unit tests, D-035).
12. **Subscriber stall:** a subscriber that stops draining while 10,000 ordinary and 100 `critical` events are
    published: matching, admission and the other subscribers keep their throughput and order; the stalled
    subscriber receives `Gap` markers, then is detached with `system.subscriber_stalled`, and its owner
    re-subscribes and re-syncs; a `system.stopping` published during the stall still runs the §10.8 sequence to
    completion within the grace.
13. **Settings (§16):** a first start creates the document with the defaults; an invalid stored document (bad
    range, unknown key) starts the engine on the defaults with `system.document_invalid` and `source: "defaults"`;
    a partial `engine.run` section is `INVALID_PARAMS` while an omitted top-level section keeps its stored values;
    an out-of-range value yields one `fieldErrors` entry per field with the request pointer
    (`/settings/engine/run/maxConcurrent`) while the same field appears as `engine.run.maxConcurrent` in
    `changed` after a valid write (one fixture shows both), and stores nothing; a stale `revision` is `CONFLICT`;
    **concurrent writers** (forced interleaving): two unconditional updates, and two updates with the same
    explicit `revision`, never both succeed against one stored revision, commit, apply and publish in revision
    order, and the effective configuration never shows an older revision than the stored one; a storage failure
    before the commit leaves the effective configuration and the event stream untouched; **cancellation
    finality (§16 writer):** a caller cancelled before its request reaches the writer's transaction leaves no
    durable write and no event; a caller cancelled after submission once the transaction has opened, after the
    commit before the swap, and during publication, in every case ends with the stored revision, the effective
    configuration and `settings.updated` all showing the committed revision, with the same assertions as the
    two-writer case; a full writer queue makes the 17th caller wait, not fail; a submission after shutdown has
    begun is `shutting_down` while the accepted request completes; **defaults path (§16 `settings.get`):** a
    stored document with a wrong-typed field, a missing field, an out-of-range value and a newer `schemaVersion`
    each start the engine on the defaults, `settings.get` returns `source: "defaults"` with the matching
    `reason`, a `settings` value that validates against the `Settings` schema, the persisted record's
    `revision`/`updatedAt`/`updatedBy` and an empty `pendingRestart`, never the stored bytes; a valid
    whole-section repair update then returns `source: "document"`, and the newer-version case refuses the
    repair with `read_only`; a changed
    `maxConcurrent` and `flushMs` take effect at the next dispatch and flush without cancelling a running run; a
    changed `engine.bus.capacity` (8192 → 16384) is reported in `restartRequired`, in `pendingRestart` afterwards
    and in `settings.updated`, the channel size is unchanged until restart, and the **undo** (16384 → 8192, the
    boot value) reports an empty `restartRequired` and empties `pendingRestart`, while an unrelated change made
    in between keeps the pending key; the `memory_budget` warning appears at the threshold; under MCP the call
    yields a `PendingChange { targetKind: "settings" }` and the live document is untouched; `dryRun` returns the
    key diff.
14. **`templates.parse` (§15):** the template strings of every spec 02 §17.11 fixture parsed through the operation
    yield the AST, errors, spans and references the in-process parser yields (one code path); a `{"a": 1}` text
    yields the literal-brace warning; a `source` over `maxLength` is `INVALID_PARAMS` from the schema, never a
    parse diagnostic, and a source at exactly `maxLength` parses; nesting deeper than 32 or more than 10,000 AST
    nodes is a parse diagnostic with `ast` absent, `references` empty and `dynamicLookup: false`, never
    `INTERNAL`; multibyte and astral-plane text (emoji, combining marks) yields spans in Unicode scalar offsets,
    `end` exclusive, equal through the operation and the in-process call; the largest permitted AST serializes
    under the default message budget; `value` mode with two forms is an error with the second form's span.

## 23. Contract amendments requested from spec 02 (`den-protocol`)

Spec 02 is approved; these are additive requests for the contract task that precedes implementation (AGENTS.md
"Contracts first"), tracked as board card **DEN-33**. None blocks the rest of this spec: where an amendment has not
landed, the stated default applies.

**Landed in spec 02 r6 (DEN-33, 2026-10-19).** Every item below is decided text in spec 02 (§3–§11, §17); spec 02
is normative where its wording differs from this list (`PendingChange.targetKind`, `Case.values`,
`FilterableField.caseInsensitive`, `PlannedCall.untrustedFields` not part of the plan identity, `x-den-untrusted:
"conditional"` naming its sibling in `x-den-untrusted-flag`). The list stays as the request record; the
`den-protocol` code follow-ups are tracked from the DEN-33 handoff.

- **A1 — `SubAction.onError: OnError { stop | continue }`** (default `stop`) and the read-only run-context fields
  `run.lastError` (§9.10) and `run.queueName` (§6.4). Needed for Streamer.bot import parity and rows 2.08, 3.09.
- **A2 — `Node::ForEach { items: TemplateOr<Value>, itemVar: String, indexVar: Option<String>, maxIterations:
  Option<u32>, children }`** (row 2.07, P1) **with its approval data-flow contract** (§9.5): the node is a
  writer of `itemVar`/`indexVar` with static names; its recorded binding render is the `{ "$den": "foreach",
  node, items, index }` record over the binding render of `items`; plan-time expansion to `min(len, cap)`
  `PlannedCall`s per tier-3 body node when the collection render is concrete, one when it is dynamic; `items`
  is the collection as consumed. Spec 02 §8.8 (writer set, sentinel list, rule 4 multiplicity) and §17.9
  (changed-element, changed-length, dynamic-collection, session-grant cases) need the matching text. Until it
  lands the importer and the editor use `While` with a counter. `Node` is documented as extensible with a
  placeholder fallback, so the variant is non-breaking; the §8.8 text is a clarification of an existing rule.
- **A3 — Documents `Command` and `Timer`** (§3), the `Run`, `RunState`, `CancelReason`, `RunError`, `RunWarning`
  records, the missing `Case { values: Vec<Template>, children: Vec<SubAction> }` struct of `Node::Switch`, and
  the catalog registrations of §15 (operations, events including `core.custom`, `highVolume`/`critical` flags,
  `writesArgs`).
- **A4 — `Queue.paused` persisted** (already a field; the amendment is the note that it is a runtime field the
  engine writes back, `updatedBy: system` (A7), without bumping action revisions).
- **A5 — Catalog field `specificity` default** (1) and the `caseInsensitive` field hint used by §6.2.
- **A6 (deferred, spec 12)** — `Op` nodes with structured branch children (`x-ui.branches`), raised by research
  Part B §3.5; not needed for P1, flagged so spec 12 does not add it without an ADR.
- **A7 — `UpdatedBy { user | imported | mcp | plugin { pluginId } | system }`** for every `updatedBy` field (spec 02
  §6.1, §6.3 currently say `Origin`, which has no `system` or `plugin` variant). Until it lands the engine records
  `user` for its own writes.
- **A8 — Error code `INTERNAL`** (engine fault, `details.reason`), and `runs.replay` added to the dynamic-effective-tier
  set (reserved `approvalId`, tier-3 MCP annotation, dry-run plan) alongside `actions.run` and `plugins.call`.
- **A9 — `Queue.maxRunMs: Option<u64>`** (§9.11), default none.
- **A10 — Untrusted provenance on stored variables and plans** (§21): stored variables carry `untrusted: bool`
  (`variables.get/list` results, the `variables.set` result `{ value, created, untrusted }`, the export bundle's
  `globals[]` entries); `variables.*_updated` payloads carry **independent** `oldUntrusted` and `untrusted`
  flags for `old` and `new`; `variables.delete` returns `{ deleted }` and `clear_temp` `{ count }` (no values);
  `x-den-untrusted: conditional` as a catalog marking that **names the sibling boolean** gating MCP wrapping
  and run seeding (`untrusted` unless stated, `oldUntrusted` for `old`; §11.2 readback rule);
  `PlanStep.untrustedFields` / `PlannedCall.untrusted_fields` next to `dynamicFields`, shown by the
  approval request inside the untrusted wrapper. `variables.user_updated` puts the user in the envelope.
- **A11 — Global and user variable key** (§11.1): spec 02 §6.3 should state that `name` is unique across the
  persisted and temp stores and `persisted` is an attribute of the entry (the engine's invariant), not part of
  the key.
- **A12 — Event payload hygiene** (§7.1): spec 02 §6.3's reserved-name rule extended to the filter path
  `platform`; `commands.triggered.captures` replaces the r1 `match` key; the catalog lint covers both.
- **A13 — Payload upgrade functions** (§12.5): a core payload version bump also registers the upgrade
  `(type, from, to)` so logged events of the older version can be replayed through the current matcher.
- **Scopes:** `events:simulate`, `events:emit`.

**Requested by r6 of this spec (DEN-68) and written into spec 02 r7 in the same change** (`den-protocol` code:
cards DEN-69 and DEN-118):

- **A14 — Settings document** `Settings { engine: EngineSettings }` with the typed sections of §16,
  `x-den-restart` and the exported restart-only list; operations `settings.get` / `settings.update`; event
  `settings.updated`; scopes `settings:read`, `settings:write` (spec 02 §6.5, §8.7).
- **A15 — `templates.parse`** in the reserved namespace `templates` (spec 02 §4), returning
  `TemplateParseResult { ast?, errors, warnings, references, dynamicLookup }` with the result types and parser
  limits spec 02 §10 now defines next to the AST (`TemplateParseError`, `TemplateLint`, `Span`, `TemplatePath`).
  A14 and A15 are **proposals** of this revision until spec 02 r7 is reviewed by Codex Astra and the owner; the
  DEN-65 catalog inventory counts them conditionally (65 operations, 29 events).
- **A16 — `runs.list` paging** (`limit`, `cursor`, `{ runs, nextCursor }`), `runs.get.snapshots: Option<bool>`, the
  `row_truncated` warning code and `actions.run.queue: Option<QueueId>` as the catalog registrations of the existing operations (§12.6, §10.2; the shapes are stated
  here because spec 02 §5.2 lets this spec own its operations' payloads).

## 24. Open questions (added to [QUESTIONS.md](../QUESTIONS.md), section E)

Q-E1–Q-E8 were answered by the owner on 2026-10-19 with the spec defaults (D-039); the list is kept as the record
of what was asked. The eight DEN-65 gaps were engineering decisions, taken in §7.1, §10.2, §10.3, §12.6, §15, §16
and §19; r6 raised one product question, Q-E9.

- **Q-E9:** the upper bound a user may set for `engine.history.maxAgeDays` and `engine.events.maxAgeHours`
  (Action History and the event log for non-chat data; Q-E4 accepted the defaults, Q-H1 covers chat only). Spec
  default until answered: only the lower bound is validated (§16).

- **Q-E1:** should queued runs survive a restart (durable queues)? Spec default: no (§10.8).
- **Q-E2:** should a failed sub-action stop the run by default? Spec default: yes, `onError: stop` (§9.10).
- **Q-E3:** should one event fire an action once (its most specific trigger) or once per matching trigger? Spec
  default: once (§6.3).
- **Q-E4:** Action History retention defaults (10,000 runs / 7 days / 24 h event log). Spec default as stated (§12.2).
- **Q-E5:** does `Stop` inside an inline Run Action end only the child or the whole chain? Spec default: the child
  (§9.5).
- **Q-E6:** command permissions: keep Streamer.bot's "empty Deny list denies everyone", or the spec's "empty list is
  neutral" (§7.1)? Spec default: neutral.
- **Q-E7:** when a `While` hits its iteration cap, end the loop with a warning (spec default) or fail the run?
- **Q-E8:** the 16-hop `causedBy` limit also stops a user's deliberate re-trigger loop (A → custom event → A).
  Keep it for every principal (spec default, with `While`, timers and queued `RunAction` as the supported loop
  forms), or exempt user-token and system origins?

Assumptions made while writing (reviewers, please check): spec 02 r5 (origin/main) is the contract base and
its rules are not restated here; the alert queue is a spec 09 concern layered on events, not an action queue; `Action.concurrent`
honours `Delay` per branch and the importer compensates; cooldown state is not persisted; `rusqlite` over `sqlx`
is an engineering choice open to challenge; the `croner` crate is a placeholder for "a cron parser with the §8.3
grammar"; signals are implemented as operations rather than new `Node` kinds so spec 02's tree stays unchanged.

## 25. Revision history

- **r1**: initial draft (DEN-15). A fresh-context Claude reviewer pass (4 P1, 11 P2, 14 P3) was folded in before
  commit: cron fires dispatched to their owning trigger; derived events keep their `causedBy` and context;
  `UpdatedBy` (A7); `events.emit`/`core.custom` (row 2.36); event envelope stored with the run; internal events
  never awaited under a lock; dominance-based precedence; depth-only recursion guard; `INTERNAL` and
  `runs.replay` dynamic tier (A8); per-queue `maxRunMs` (A9); persistent-map snapshots and the per-node budget;
  untrusted-arg taint set; SQL `IS NOT`, `name_key`, extra indexes; write-triggered pruning and backups; export via
  download handle; `RunState` as a string plus `cancelReason`; `Case` struct; Q-E8.
- **r2**: resolves the Codex (Astra) review of r1 (5 P1, 6 P2; none rejected). Finding → sections:
  1. P1 approval before snapshot → §9.2 reordered (static check, one-pass pin and render, validate, plan check on
     that render, invoke), §22.9 ordering regression.
  2. P1 `ForEach` data flow → §9.5 `ForEach` semantics (collection, variables, order, cap, writer contract,
     `foreach` record, plan-time multiplicity), §23 A2 expanded, §22.4 and §22.9 cases.
  3. P1 untrusted provenance through variables → §11.1 entry shape, §11.2 events and MCP wrapping, §11.3 reads,
     §11.4 same-row flag, §12.3 replay re-seeding, §13.2 `untrusted` columns, §13.4 export, §21 rewritten
     (sources, propagation, variables, reads, plans), §23 A10, §22.5, §22.8, §22.9.
  4. P1 unbounded `Parallel` → §9.6 task budget with permit release at joins, persistent-map args, run memory
     budget, node-execution budget; §9.13 and §16 new limits; §14.3 cross-reference; §22.4 fan-out tests.
  5. P1 WAL-unsafe backups → §13.6 `VACUUM INTO` / backup API, verification, atomic publish, restore rule;
     §13.3 pre-migration backup via the same procedure; §13.1 cross-reference; §22.8 positive reproduction.
  6. P2 subscriber backpressure → §5.2 engine never depends on a subscriber channel; §5.3 per-subscriber
     drop-with-`Gap`, critical lane, stall detection and detachment; `system.subscriber_stalled` in §15; §16
     settings; §22.12.
  7. P2 reserved payload keys → §7.1 `captures`, envelope `user`/`platform`, the engine's reserved-name rule and
     `match` seeding precedence; §6.4, §6.5 (`core.test` keys), §11.2 (`variables.user_updated`), §15 note,
     §23 A12, §22.1 fixtures.
  8. P2 event-log replay fidelity → §12.5 complete envelope and version conversion; §13.2 `events.envelope`;
     §22.6.
  9. P2 shutdown triggers → §10.8 phased sequence (close-and-fire atomically, grace, flush then marker); §13.5
     reads then clears the marker; §22.3 cases.
  10. P2 zero-capacity `drop_oldest` → §10.2 runtime rule and save-time refusal; §22.3.
  11. P2 cron catch-up cursor → §8.3 cursor record, account-first boundary, startup rule, DST duplicate rule;
      §13.2 `cron_cursors`; §13.5; §22.7.
  Review-scope notes: §11.1 reconciled with spec 02 §6.3 (A11); §14.3 "or on a cycle" removed (depth only);
  §24 names spec 02 r5 as the base; DEN-33 synchronized with A1–A13 and the scopes. A fresh-context Claude
  reviewer pass on r2 (10 resolved, 1 partial, 5 P2, 11 P3; all folded in) added: a taint set per history
  snapshot and replay re-seeding from the chosen one; `match` and signal args as tainted sources; MCP-origin
  variable writes stay untrusted; the leaf-versus-container rule in §9.2; `ForEach` deleting its variables on
  zero iterations and the post-loop binding render; `ignoreInternal` excluding replayed, simulated and test
  messages (default `true`); `core.custom` args seeded as `name`/`data` per spec 02 §6.3; the critical-event
  guarantee boundary and lane ordering wording; the existing-task bound; A13 payload upgrades; `clean_shutdown`
  cleared before migrations; `user` carried into queued children; `untrusted` in `variables.get`; the
  `events:simulate` scope on `triggers.test`.
- **r3**: resolves the two P1s of the Codex (Astra) re-review of r2 (the three P2s are Backlog cards DEN-42,
  DEN-43 and DEN-44 and are not changed here).
  1. P1 parked `Parallel` tasks escaped the permit limit → §9.6: a permit now bounds an **existing** branch
     task and is held from spawn to exit, including at joins; a `Parallel` node never waits for a permit but
     runs a branch **inline in its own task** when none is free, which keeps the merge order and every other
     observable result and makes deadlock impossible; parked branches' writes count against `maxRunBytes`; the
     state bound is restated as `maxTasksPerRun` tasks per root run, `maxConcurrent × maxTasksPerRun`
     engine-wide, `maxDepth × maxInlineDepth` frames per task, no pending-branch queue; `Delay` note for inline
     branches; §9.13 row; §22.4 adds the 17-level breadth-first chain with `Delay` leaves at budgets 1 and 64,
     parked-task counting, cancellation mid-expansion and the `maxTasksPerRun = 1` nesting case.
  2. P1 variable readback losing provenance → §11.2: `variables.set` returns `{ value, created, untrusted }` of
     the stored entry after the call (`default` on an existing entry returns the existing value with its flag,
     no write, no event); `variables.delete`/`clear_temp` return no value; `variables.*_updated` carries
     independent `oldUntrusted`/`untrusted`; a **readback rule** makes every value-carrying field conditional
     on its sibling flag for MCP wrapping, event seeding, `saveAs` tainting, event log and replay; §21 sources,
     propagation, variables and reads bullets; §15 results; §23 A10 extended (DEN-33 synchronized); §22.5 and
     §22.9 cases (MCP `default` on a tainted entry, tainted → literal overwrite → `{event.old}` in a later
     plan, subscriber and replay fidelity, `saveAs` of `variables.get`).
  A fresh-context Claude reviewer pass on the r3 diff (both P1s resolved; 2 P2, 3 P3, all folded in) added:
  ended-but-unmerged branch writes charged to `maxRunBytes` until the join; the plan-time resolution of the
  conditional flag through the producer's `name`; the node stops taking branches after a sibling stopped or
  failed the run; "identical" narrowed to merge order and merged args; §11.1 names `set` and the two event
  flags.
- **r4** (DEN-42, with DEN-43, DEN-44, DEN-45 and the DEN-33 Spec 03 follow-ups folded in; the P2s of the Codex
  re-reviews of r2 and r3):
  1. DEN-42 cron fall-back → §8.3: DST handling moves into occurrence generation (each local wall time maps to
     at most one instant, the first fold), so every repeated wall time is suppressed on ticks and at catch-up;
     the cursor drops `last_local` (§13.2); §22.7 covers every-minute, `*/15` and daily schedules across the
     full repeated hour with a restart between the folds.
  2. DEN-43 startup order → §13.5: bootstrap a new file, inspect integrity and schema version before any
     write, newer schema read-only with no marker at startup or shutdown (§10.8 step 3), corrupt files moved
     aside untouched, marker written only for a supported schema and before migration; §22.8 cases.
  3. DEN-44 `ForEach` approvals → §9.5 *Changed collections*: whole-collection binding kept; any change fails
     at the call or at loop start (a new check, so an emptied collection is caught) under a one-shot
     approval, a session grant pins only the count; fewer calls
     through control flow is not a mismatch; §22.9 cases; spec 02 §8.8 rule 4 and §17.9 corrected (r6
     amendment line there).
  4. DEN-45 one-permit `Parallel` → §22.4: an idle root spawns its first branch; all-inline only when the
     enclosing branch already holds the sole permit.
  5. DEN-33 follow-ups → §3 recap synchronized with spec 02 r6 (`ForEach`, `onError`, `maxRunMs`,
     `RunError.code: ErrorCode`, `Command.sources`, `UserKey`) and marked non-normative; `Run.trigger` copied
     on replay as in §12.3; §21 names the unchained `Trigger` origin among the clean literal writers.
- **r5** (DEN-49, the P2 of the Codex review of r4): §9.5 *Changed collections* states the session-grant check
  as a change of the capped iteration count `min(len, cap)`, not of the length, consistent with the loop-start
  rule; one-shot approvals keep whole-collection binding. §22.9 adds `maxIterations: 2` cases: a length change
  at or above the cap runs under a session grant and fails `args` under a one-shot approval, a change crossing
  the cap is `unplanned_call`. Spec 02 §8.8 rule 4 and §17.9 synchronized (r6 amendment line there).
- **r6** (DEN-68, the gaps found while slicing Spec 03 into engine cards, DEN-65):
  1. Settings → §16 rewritten around the `Settings { engine: EngineSettings }` document (spec 02 §6.5, A14):
     dotted keys as JSON paths, the missing ranges filled as validator bounds (reject, never clamp; defaults
     unchanged; the DEN-67/DEN-146 handoff; the two retention ceilings left to Q-E9), `x-den-restart`, `settings.get` / `settings.update`
     (whole document, every field required, `CONFLICT` on a stale revision, hot-apply and restart reporting,
     warnings, dry-run diff), `settings.updated`, startup fallback to defaults; §13.2 fixed id; §15 rows; §17;
     §22.13.
  2. Engine-wide dispatch order → §10.2 admission sequence, §10.3 *The engine-wide slot* (oldest-admitted
     eligible head across queues; `force_next`/`shuffle` within a queue only); §9.13 row; §22.3.
  3. `actions.run { queue }` → §10.2 *Target queue* (any existing queue, no extra scope, root run only, not
     inherited by queued children or replay, outside the plan identity); §15 row; §22.3.
  4. `runs.list` → §12.6 (newest-first by `id`, opaque keyset cursor, `limit` 1–1000, live view then history
     merged by `id`, `pending: false` rule, snapshots only on `runs.get`); §15 row; §22.6.
  5. Bot accounts → §7.1 *Bot accounts*: a `BotAccounts` provider filled by the connector layer (spec 05 names the
     account), consulted at match time, empty until then; §22.1.
  6. Data directory → §19 ownership bullet: `den-core` resolves override → `STREAMDEN_DATA_DIR` → OS default,
     creates and checks the directory, returns the share/sync warnings; apps parse `--data-dir`; file ownership
     inside the directory; §22.8.
  7. OS gate → §19 last bullet: one `cfg(unix)` file-mode helper in `storage/paths.rs` (pre-created `0600`
     files, `0700` directories, no-op elsewhere); §21.
  8. UI template lint → `templates.parse` (§15, §17; spec 02 §10, A15): the editor lints through the engine's
     parser, a WASM build of the parser module is permitted, a TypeScript port is not; §22.14.
  A fresh-context Claude reviewer pass on the r6 diff (0 P1, 4 P2, 9 P3, all folded in) added: `queue` is part of
  the call's binding hash while the plan is unchanged; `settings.update` replaces whole sections and omitted
  sections keep their stored values (spec 02 §7.2); the `templates.parse` result types and parser limits in
  spec 02 §10; the `network_share` probe without a second `cfg` block; `explicit` instead of the keyword
  `override`; `maxNodes` and `maxSnapshotBytes` ceilings and the `message_size` warning under the 1 MiB default;
  pages cut at the message budget; one id generator for admitted and inline runs; `eventSnapshot` elided on
  list rows; `PendingChange.params` without `revision`.
  **Round 2** (the Codex Astra review of `dabe19e`, 1 P1 / 9 P2, all resolved): F1 the Q-H1 inference removed,
  the two retention ceilings opened as Q-E9 with only lower bounds validated meanwhile (§12.5, §16, §24); F2 the
  live-to-terminal visibility handoff with settling entries (§12.6, §22.6); F3 storage limits separated from the
  message budget, `row_truncated` progress rule, `runs.get { snapshots }` and `PAYLOAD_TOO_LARGE` /
  `message_budget` (§12.6, §16, §22.6); F4 `eventSnapshot` omitted, never `null` (§12.6, §22.6; spec 02 §6.5); F5
  `fieldErrors[].path` stays a request JSON Pointer, dotted keys only in the lists, warnings and the event (§16,
  §22.13; spec 02 §6.5); F6 the settings guard ordering compare → commit → swap → publish (§16, §22.13); F7
  `restartRequired` and `pendingRestart` against the boot value, undo case (§16, §22.13); F8 immediate admission
  honours the global head (§10.2, §22.3); F9 transport-independent parser limits, `source.maxLength`, failure
  shape, span units (§22.14; spec 02 §10). Cards DEN-69, 77, 82, 93, 118, 119 and 146 re-synchronized (F10).
  **Round 3** (the Codex Astra re-review of `360358d`, 3 P2, all resolved): R2-1 every variable-length record
  field bounded at record time (§12.1) and the reduced list row made fixed-size with a derived bound `K_row`, a
  serialized-size check including the envelope, an explicit `message_budget` / `details.minimum` failure
  below that bound, and `runs.get` overflow reported per component (§12.6, §22.6); R2-2 the settings guard
  replaced by an owned, bounded **settings writer** task with a defined cancellation boundary (before the
  transaction: discarded; after: completes commit → swap → publish regardless of the caller) and shutdown
  behaviour (§16, §22.13); R2-3 `settings.get` under `source: "defaults"` returns the defaults the engine runs
  on as a valid `Settings`, a `reason`, the set-aside record's provenance and an empty `pendingRestart`, never
  the invalid stored bytes (§16, §22.13).
  Owner answers applied: Q-E1–Q-E8 marked confirmed (D-039) where the text asked for confirmation (§6.3, §7.1,
  §9.5, §9.10, §10.8, §12.2, §24); the Q-H1 and Q-C5 answers of 2026-10-19 recorded where §12.5 and §13.6
  deferred to them (no behaviour change here; spec 08 owns the chat history store). Cards DEN-69, 70, 77, 82, 93,
  99, 102, 118, 119 and 146 updated where the decision adds to their assumption.
