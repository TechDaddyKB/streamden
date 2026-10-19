# Spec 03: Engine core (WS-03)

- **Status:** Revision 1, draft for Codex review. Card: DEN-15.
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
description, tags, origin }`; `Queue { id, name, blocking, paused, maxPending, overflow }`; `Trigger { id, type,
plugin?, enabled, unresolved, alwaysRun, filters, config }`; `SubAction { id, enabled, weight, note, node }` with
`Node` = `Op | Group | If | Switch | While | BreakLoop | ContinueLoop | Stop | Parallel | Delay | RunAction | Code |
Placeholder`; the run context roots `event, user, trigger, action, run, globals, plugins, match, args, now`
(spec 02 §6.3); `AuthContext` and the rules of spec 02 §8.8.

This spec adds the engine's own records and documents, all persisted per spec 02 §7.3 and carried by
`den-protocol` through a contract task (§23):

```rust
/// One execution of an action. Persisted in Action History (§12) unless `history.record` is false.
pub struct Run {
    pub id: RunId,                       // UUIDv7, minted at admission
    pub action: ActionId, pub action_name: String, pub action_revision: u64,
    pub trigger: Option<TriggerId>,      // None for actions.run, replay, RunAction children
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
    pub untrusted_args: Vec<String>,     // arg names carrying x-den-untrusted text at the end of the run (§21)
    pub error: Option<RunError>,         // set when state is failed, rejected or dropped
    pub stopped_by: Option<NodeId>,      // the Stop node or Code node (ok: false) that ended the run early
    pub warnings: Vec<RunWarning>,       // loop cap hit, break outside loop, dynamic reads of missing args…
}
pub enum RunState { Queued, Running, Completed, Failed, Cancelled, Rejected, Dropped }   // snake_case strings
pub enum CancelReason { User, Cleared, Shutdown, Timeout, Parent /* a Parallel sibling or parent run stopped it */ }
pub struct RunError { pub code: String /* spec 02 §8.5 data.code */, pub node: Option<NodeId>, pub message: String, pub details: Option<Value> }
pub struct RunWarning { pub node: Option<NodeId>, pub code: String, pub message: String }

/// Chat command (rows 4.01–4.10). Persisted document, kind "command".
pub struct Command {
    pub id: CommandId, pub revision: u64, pub name: String, pub group: Option<String>, pub enabled: bool,
    pub listed: bool,                    // shown by commands.list_public (row 4.11); Streamer.bot "include"
    pub aliases: Vec<String>,            // at least one; matched case-insensitively unless case_sensitive
    pub mode: CommandMode,               // basic { location: start | exact | anywhere } | regex { pattern }
    pub case_sensitive: bool,
    pub sources: Vec<CommandSource>,     // open enum: "chat.message", "chat.whisper", "chat.sub_message"; empty = chat.message
    pub platforms: Vec<PlatformId>,      // empty = all
    pub ignore_bot_account: bool, pub ignore_internal: bool,
    pub cooldown: Cooldown,              // { globalMs: u32, userMs: u32, exemptRoles: Vec<String> /* default ["broadcaster"] */ }
    pub permissions: Permissions,        // { mode: allow | deny, users: Vec<UserRef-lite>, roles: Vec<String>, groups: Vec<GroupId> }
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
  sub-millisecond, §20); every other subscriber receives through its own bounded channel.
- Delivery is at-most-once per subscriber. The WS replay ring (spec 02 §8.4) and the persisted event log (§12.5)
  are how consumers recover.

### 5.3 Backpressure

- The bus queue is bounded (`engine.bus.capacity`, default 8,192 events). Publishers `await` when it is full
  **unless** the event type is flagged `highVolume` in the catalog, in which case the event is dropped immediately
  and counted; every 5 s of drops produces one `system.events_dropped { count, since, types }` event.
- Critical events (catalog flag `critical`) are never dropped anywhere in the engine.
- A subscriber's own channel overflow follows the same rule, per subscriber (den-server adds its per-session policy,
  spec 02 §8.4).

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
   One event therefore fires one action at most once (Q-E3 asks the owner to confirm);
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
lazily; `trigger` is `{ id, type }`; `match` holds the regex captures of the firing trigger (empty otherwise);
`args` is seeded with the event's **top-level payload fields** (spec 02 §6.3). Reserved names cannot collide
because codegen refuses payloads that use them. Streamer.bot's run-scope built-ins (row 3.09) map as `%actionId%`
→ `{action.id}`, `%actionName%` → `{action.name}`, `%triggerId%` → `{trigger.id}`, `%triggerName%` →
`{trigger.type}` (triggers have no name in spec 02), `%queueName%` → `{run.queueName}` (amendment A1),
`%actionQueuedAt%` → `{run.queuedAt}`, `%isTest%` → `{run.isTest}`, `%date%`/`%time%` → `{now | date:…}`.

### 6.5 Test and simulated events (rows 2.32–2.34)

- `triggers.test { triggerId, payload? }` (tier 1) builds an event of the trigger's type from the catalog example
  (or the given payload, validated), sets `meta.test = true`, and publishes it through the bus with an in-engine
  **target** marker, so WS subscribers and the event log see it while the matcher fires **that trigger's action
  only**, bypassing filters and precedence. The run's context is derived from the caller's synthetic run (spec 02
  rules 1 and 5: `Principal::Trigger`, `surface: event`, `chain.originRun` = the synthetic run, `hops` = 1). It
  returns `{ runId }`.
- `events.simulate { type, payload, user?, platform? }` (tier 1, scope `events:simulate`) publishes a normal event
  with `meta.test = true` through the bus, so every matching trigger fires under the normal precedence rule, and
  other subscribers (overlays, chat) see it too. Runs started by a test event have `run.isTest = true`.
- `core.test` is a trigger type (Streamer.bot `Core > Test`) whose `config.variables` object is copied into args;
  firing it is `triggers.test` on that trigger.

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
   the sender is a configured bot account, or when `ignoreInternal` and the message was sent by the engine itself
   (`meta.causedBy` set, or the connector marks it internal).
2. **Basic mode:** for each alias, compare per `location`: `start` → the message starts with the alias followed by
   end-of-text or whitespace; `exact` → the whole trimmed message equals the alias; `anywhere` → the alias occurs as
   a whole word anywhere. Case-insensitive unless `caseSensitive`. There is no prefix concept: `!so` is just text.
   **Regex mode:** the pattern matches anywhere; captures go to `match`.
3. **Permissions:** `allow` mode → the user must be listed, have one of the `roles` (`UserRef.roles`) or be in a
   group; `deny` mode → the user must not be. An empty `allow` list allows everyone; an empty `deny` list denies
   nobody (Streamer.bot's "empty Deny denies everyone" is **not** copied; the importer sets an explicit list; Q-E6).
   Users with a role in `cooldown.exemptRoles` (default `broadcaster`) bypass cooldowns, not permissions.
4. **Cooldowns:** global and per-user windows are shared across all aliases of the command. A hit publishes
   `commands.cooldown { commandId, user, globalLeftMs, userLeftMs }` instead of `commands.triggered` and does not
   advance counters. Cooldown state is in memory (lost on restart; row 4.04 does not require persistence).
5. On success the engine increments `counter` and `userCounter` (persisted per `counters` policy, §13.2) and
   publishes **`commands.triggered`** as a **derived event** of the chat message (§5.1: same `causedBy` and
   context, no extra hop) with payload `{ commandId, commandName, alias, location, source, input:
   Vec<String> /* whitespace-split remainder */, rawInput, message: ChatText, counter, userCounter, match }` plus the
   envelope's `user` and `platform`; `correlationId` = the chat message id. Streamer.bot's `input0..n` and
   `rawInputEscaped` are importer aliases over `input` (spec 02 §6.3).

Several commands MAY match one message; each publishes its own event. A command's event is what actions trigger
on: trigger type `commands.triggered` with a filter `commandId eq …` (the editor's "Command" trigger is sugar for
that filter), so precedence and chains work like any other event.

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

DST rules: a scheduled local time that does not exist (spring forward) is skipped; one that occurs twice (fall
back) fires on the first occurrence only. `catchUp: true` fires at most **one** missed occurrence after a suspend or
restart, stamped with the missed `scheduledAt`; `false` (default) skips missed occurrences. Cron has no shared
state, so it needs no document; the trigger is the schedule.

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
2. runs the **authorization re-check** of spec 02 §8.8 rule 3 (tier and scopes against the run's context) and, for
   a tier-3 `Op`, the plan check of rule 4(b). A failure stops the run with `PERMISSION_DENIED` or
   `APPROVAL_REQUIRED`, regardless of the node's `onError`;
3. takes **one snapshot** of the variable state and renders every template of the node against it, recording the
   binding render for writers (spec 02 §8.8 "no time-of-check gap"). A snapshot is a **version pin**, not a copy:
   globals and user variables live in persistent (structurally shared) maps swapped atomically on write (§11.4),
   so pinning is an `Arc` clone and costs nothing per node;
4. coerces and validates rendered params against the invocation schema (spec 02 §10.1);
5. executes the node (§9.3–9.9) with the node's timeout;
6. applies the node's writes to `args` (`saveAs`, `writesArgs`, `Code.setArgs`) **atomically after the node
   returns**: a node never sees its own partial writes, and a later node sees all of them;
7. publishes `runs.node_completed` (§12.4) and continues, or applies the error policy (§9.10).

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

### 9.5 `If`, `Switch`, `While`, `BreakLoop`, `ContinueLoop`, `Stop`

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
  `RunWarning { code: "loop_cap" }` and continues after the loop (Q-E7 asks whether it should fail instead).
  Each iteration also yields to the runtime once, so a loop without awaits cannot starve other runs.
- **`BreakLoop` / `ContinueLoop`** affect the innermost enclosing `While` (or `ForEach`, amendment A2) **within the
  same action**; outside a loop they are no-ops with a warning (the editor forbids them, §18). They never cross a
  `Parallel` boundary or a `RunAction`.
- **`Stop`** ends the **current action's run** as `Completed` with `stoppedBy`. Inside an inline `RunAction`
  child it ends the child; the parent continues after the `RunAction` node (Q-E5). Inside a `Parallel` branch it
  cancels the sibling branches (`cancelReason: parent`) and then ends the run.

### 9.6 `Parallel` (D-028)

Every enabled branch runs as its own task, started together, each with a **copy** of the args map as of the
`Parallel` node's snapshot. The node joins when all branches have ended. A branch's **writes** are the names it set
or deleted (not its whole copied map); they are merged back into the parent's args **in branch order** (a later
branch's write to the same name wins), so the result is deterministic and independent of timing. `Delay` inside a branch delays only that branch. A branch that fails under
`onError: stop` cancels its siblings and fails the run; under `continue` the other branches finish and the run goes
on. `Action.concurrent` is defined as `Parallel` over the top-level nodes, one node per branch (Streamer.bot's
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
  the rendered `args` map (spec 02 §8.8), the parent's `event` envelope carried for `{event.*}` templates, and the
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
| `stop` (default) | The run ends `Failed { error }`; later nodes do not run; a blocking queue advances. Streamer.bot mostly continues on failure (outputs a `success` arg); this spec defaults to stop because a half-executed alert is worse than a visible failure. **Q-E2** asks the owner to confirm the default. |
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
| Args map size per run | 4 MiB serialized (`engine.run.maxArgsBytes`) | node fails `PAYLOAD_TOO_LARGE` |
| Single variable value | 256 KiB (`engine.variables.maxValueBytes`) | write fails `PAYLOAD_TOO_LARGE` |
| Concurrent running runs, whole engine | 256 (`engine.run.maxConcurrent`) | further dispatches wait, FIFO across queues |

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
   engine-wide concurrency has a free slot.
2. If `canStart`, the run is dispatched immediately. **The bound never applies to a run that does not have to
   wait** (B-1 review P2-1: an always-run alert on a paused, full queue must get through).
3. Otherwise the run must wait. If `waiting.len() >= maxPending` (default `engine.queue.defaultMaxPending`, 1,000,
   when the queue's `maxPending` is `None`), apply `overflow`:
   - `reject_new`: the new run is recorded as `Rejected` with `RunError { code: "INVALID_STATE", details: {
     reason: "queue_full" } }`, `queues.run_rejected` is published, and the caller (if any) gets `INVALID_STATE`.
   - `drop_oldest`: the **oldest waiting run** (regardless of `alwaysRun`: overflow is about capacity, not pause) is
     recorded as `Dropped`, `queues.run_dropped` is published, and the new run is appended.
4. The run is appended to `waiting` with `queuedAt = now`; `runs.queued` is published.

Rejected and dropped runs appear in history (unless `history.record` is false) so a storm is visible afterwards.
Admission never allocates the run's future: the executor is created at dispatch (B-1: cancelled runs never build
their future).

### 10.3 Dispatch order

When a slot frees, the queue is resumed, or `blocking` is turned off, the scheduler picks the first eligible
waiting run: if the queue is paused, the **first `alwaysRun` run in FIFO order**; otherwise the first run. Always-run
runs therefore overtake held runs only while the queue is paused, and stay FIFO among themselves. There is no
priority field; `queues.force_next { queueId, runId? }` (tier 1) moves one waiting run to the head (Streamer.bot
`ForceNextAction` / `ForceActionById`), and `queues.shuffle { queueId }` (tier 1) randomizes the waiting order.

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

On `system.stopping`: admission is closed (new runs are `Rejected` with `details.reason = "shutting_down"`),
every waiting run is `cancelled` / `shutdown`, running runs get `engine.shutdown.graceMs` (5,000) to finish, then
are cancelled cooperatively, then the runtime is torn down. Pending runs are **not** persisted and **not** resumed on
the next start (Q-E1): replaying yesterday's alerts after an update is the surprising behaviour, and durable queues
are a later feature if the owner wants them. Temp globals are discarded (spec 02 §6.3). History records are flushed
before exit.

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
importer renames a colliding temp global to `deaths_temp` and reports it.

### 11.2 Operations

`variables.get { scope: global | user, name, platform?, userId? }` (tier 0; `{ value, updatedAt, updatedBy }` or
`NOT_FOUND`), `variables.list { scope, persisted?, prefix?, platform?, userId? }` (tier 0, paged),
`variables.set { scope, name, value, persisted, mode: set | increment | decrement | default, durable?: bool,
platform?, userId? }`
(tier 1; `increment`/`decrement` require a number or missing (treated as 0) and are atomic; `default` writes only
if the variable does not exist; returns `{ value, created }`), `variables.delete` (tier 1), `variables.clear_temp {
scope }` (tier 1; Streamer.bot "Clear Non-Persisted (User) Globals"), `variables.set_arg { name, value }` (tier 1,
sub-action only, `writesArgs: Named { nameParam: "name" }`, refuses reserved roots). All writes publish
`variables.global_updated` / `variables.user_updated` with `{ name, old, new, persisted, updatedBy, platform?,
user? }`; `old: null` means created, `new: null` means deleted (rows 3.16; `variables.user_updated` is
`highVolume`).

### 11.3 Reads in templates

`{globals.x}` and `{user.vars.x}` read the node's snapshot (§9.2). A missing variable is a missing value (empty
string in text, `not_exists` true). User variables for the event's user are loaded once per run on first use; the
`user` root of a run whose event has no `user` has no `vars`.

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
`engine.history.maxSnapshotBytes` (256 KiB) with a `truncated: true` marker. The firing **event envelope is
copied into the run record** (`eventSnapshot`, redacted, counted against the same budget), because the event log
(§12.5) is pruned sooner than history and never holds `highVolume` events (a run fired by
`variables.user_updated` must still be inspectable and replayable). `untrustedArgs` (§21) is stored alongside. `history.pending: false` hides the run from `runs.list { state: queued |
running }` (the Pending view, row 2.20) but does not stop recording.

### 12.2 Retention

Records are pruned by the storage thread, triggered by history writes (a count and age check every 500 inserts,
never by a timer, so an idle engine does not wake), to `engine.history.maxRuns` (10,000) and
`engine.history.maxAgeDays` (7), whichever prunes more; `Failed` runs of the last 24 h are kept past `maxRuns`
up to 1,000 extra so a storm of successes does not erase the failures (Q-E4 asks the owner to confirm defaults).
History survives restarts. `runs.clear_history` (tier 1) deletes everything.

### 12.3 Replay (row 2.22)

`runs.replay { runId, snapshot: queued | after /* default queued */, args?: Map }` (tier 1 in the catalog;
effective tier = the action's, so it joins spec 02's dynamic-tier set, amendment A8) admits a new run of the **current** revision of the same action into its queue,
seeded with the chosen snapshot overlaid by `args`, with the run's `eventSnapshot` carried as `event` and
`meta.replay = true`, `replayOf` set, `trigger` copied for `{trigger.*}` templates, and the **caller's** `AuthContext` (never
the original run's). Events the replay emits are caused by the new run. A replay of a run whose action no longer
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

Every non-`highVolume` event is appended to the `events` table (§13.2) and pruned, write-triggered like history, to
`engine.events.maxAgeHours` (24) and `engine.events.maxCount` (50,000). This is the "replay a logged event" source (row 2.22):
`events.replay { eventId }` (tier 1, scope `events:simulate`) re-publishes the stored event with a new id,
`meta.replay = true`, under the caller's context (spec 02 rule 1), through normal precedence. Chat messages are in
this log; Q-H1's answer on chat retention bounds `maxAgeHours` for `chat.*` types (spec 08 may prune them
earlier).

## 13. Storage

### 13.1 Engine, files, pragmas

- **SQLite via `rusqlite` with the `bundled` feature** (MIT; SQLite is public domain), one pinned SQLite version on
  every OS. Not `sqlx`: its SQLite driver is async over a blocking pool anyway, is slower for this write pattern,
  and would make the engine's hot path depend on compile-time query checking. All access goes through the storage
  thread (§4.2); the API is a typed command enum, never raw SQL from other modules.
- Two files in the data directory (§19): **`config.db`** (documents, variables, counters, migrations, meta) and
  **`history.db`** (runs, snapshots, events, audit). History churns and is pruned; config is small, backed up before
  migrations and exported as JSON. Pragmas on both: `journal_mode=WAL`, `synchronous=NORMAL`, `foreign_keys=ON`,
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
  updated_at TEXT NOT NULL, updated_by TEXT NOT NULL
);                                  -- persisted globals only; temp globals never touch disk
CREATE TABLE user_vars (
  platform TEXT NOT NULL, user_id TEXT NOT NULL, name TEXT NOT NULL,
  value TEXT NOT NULL CHECK (json_valid(value)), updated_at TEXT NOT NULL, updated_by TEXT NOT NULL,
  PRIMARY KEY (platform, user_id, name)
);
CREATE INDEX user_vars_name ON user_vars (name);
CREATE TABLE counters (
  owner_kind TEXT NOT NULL,         -- command | timer
  owner_id TEXT NOT NULL, key TEXT NOT NULL,          -- key: '' for the global counter, 'u:<platform>:<userId>' per user
  value INTEGER NOT NULL DEFAULT 0, updated_at TEXT NOT NULL,
  PRIMARY KEY (owner_kind, owner_id, key)
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
  untrusted_args TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(untrusted_args))
);
CREATE INDEX runs_queued_at ON runs (queued_at);
CREATE INDEX runs_action    ON runs (action_id, queued_at);
CREATE INDEX runs_state     ON runs (state, queued_at);
CREATE INDEX runs_parent    ON runs (parent_id);
CREATE TABLE events (
  id TEXT PRIMARY KEY, type TEXT NOT NULL, ts TEXT NOT NULL, platform TEXT, user_id TEXT,
  source TEXT NOT NULL CHECK (json_valid(source)), payload TEXT NOT NULL CHECK (json_valid(payload)),
  meta TEXT NOT NULL CHECK (json_valid(meta))
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

Settings are one document (`kind = 'settings'`, fixed id) validated against the settings schema (§16).
`view_state` documents (§18) are opaque to the engine. `export_meta` records the last export/import.

### 13.3 Migrations

- Two version numbers: the **database schema version** in `meta.schema_version` (DDL steps in Rust, applied in one
  transaction each, recorded in `migrations`) and the **per-kind document `schemaVersion`** of spec 02 §7.3,
  migrated lazily on load and rewritten on the next write of that document.
- Before a DDL step runs, the storage thread copies `config.db` to `backups/config-<schema>-<ts>.db` (spec 02
  §7.3); the last 5 backups are kept. `history.db` is not backed up (it is a cache of the past; a failed migration
  deletes and recreates it with a warning).
- A database with a **newer** schema version than the app opens read-only with `system.storage_readonly { reason:
  "newer_schema" }` and the UI shows the downgrade message; nothing is rewritten.

### 13.4 Export and import

`system.export { kinds?, ids? }` (tier 0) writes a JSON bundle `{ kind: "export", schemaVersion, exportedAt, app:
{ version }, documents: [...wrapped documents], globals?: [...] }` with secrets redacted, human-readable
(pretty-printed, sorted keys; row 20.13), to a temporary file and returns `{ exportId, bytes, documents }`; the
bundle is downloaded over REST (`/api/v1/exports/<id>`, spec 06) because 1,000 actions exceed spec 02's 1 MiB
message default. The UI saves it where the user chooses (spec 07). `system.import { bundle, mode:
merge | replace_by_id }` (tier 2, draft-first under MCP) validates every document strictly, runs document
migrations, keeps ids (so re-importing updates in place), bumps revisions, and returns an import report. Spec 15
converts Streamer.bot and SAMMI formats into this bundle; the engine never parses those formats.

### 13.5 Startup

1. Open `config.db`, run DDL migrations (with backup), load all documents, migrate document versions, validate;
   an invalid document is loaded as **disabled** with a `system.document_invalid` event rather than refusing to
   start (one broken action must not take the app down).
2. Load persisted globals into memory; load the queue index, the trigger index, timers and cron schedules.
3. Open `history.db` (recreate if corrupt); write `meta.clean_shutdown = false`.
4. Publish `system.started` (and `system.crash_recovered` first if the previous `clean_shutdown` was `false`).
   Budget: §20.

### 13.6 Backups and corruption

Besides pre-migration backups, `engine.storage.dailyBackup` (default on) copies `config.db` to `backups/` on the
first write after 24 h have passed since the last backup (checked on writes, not by a timer), keeping 7. The engine never writes to a cloud location (Q-C5 is the owner's). `PRAGMA quick_check` runs
at startup; a failing `config.db` is moved aside and the newest backup is offered through the UI
(`system.storage_corrupt`).

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
- **Inline recursion** is refused at depth 32 or on a cycle (§9.8); **queued lineage** is refused at 64 (§9.8).
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
| `actions.run { action: ActionRef, args?, queue?: override }` | 1 (effective = action's) | ✓ / — (use `RunAction`) / ✓ | returns `{ runId, state }` |
| `actions.get`, `actions.list`, `actions.effective_tier` | 0 | ✓ / — / ✓ | |
| `actions.create/update/delete` | 2 | ✓ / — / ✓ | spec 02 §6.1 draft rules |
| `actions.set_state { action, enabled }`, `actions.set_node_state { action, node, enabled }`, `actions.set_node_weight`, `triggers.set_state { action, trigger, enabled }` | 2 | ✓ / ✓ / ✓ | document writes with revision bump (rows 2.30, 2.39, 2.13) |
| `actions.set_group_state { group, enabled }` | 2 | ✓ / ✓ / ✓ | every action whose `group` path starts with `group` |
| `triggers.test`, `events.simulate`, `events.replay` | 1 | ✓ / ✓ / ✓ | §6.5, §12.5 |
| `events.emit { name, data?: object, user?: UserRef }` | 1 | ✓ / ✓ / ✓ | publishes `core.custom { name, data }` (row 2.36, below); scope `events:emit` |
| `runs.list { state?, action?, since?, limit }`, `runs.get { runId, trace? }` | 0 | ✓ / — / ✓ | pending view = `state: [queued, running]` |
| `runs.cancel`, `runs.replay`, `runs.clear_history` | 1 | ✓ / ✓ / ✓ | `runs.replay` effective tier = action's |
| `queues.get/list` | 0 | ✓ / — / ✓ | with runtime counters |
| `queues.create/update/delete` | 2 | ✓ / — / ✓ | |
| `queues.pause/resume/pause_all/resume_all/clear/force_next/shuffle` | 1 | ✓ / ✓ / ✓ | |
| `variables.get/list` | 0 | ✓ / ✓ / ✓ | `variables:read[:ns]` |
| `variables.set/delete/clear_temp` | 1 | ✓ / ✓ / ✓ | `variables:write[:ns]`; `set` has `writesArgs: None` |
| `variables.set_arg` | 1 | — / ✓ / — | `writesArgs: Named { nameParam: "name" }` |
| `signals.send`, `signals.wait` | 1 | ✓ / ✓ / — | `wait` is not an MCP tool (it blocks) |
| `commands.*` (§7.2) | 2 / 1 / 0 | ✓ / set_state, reset_counter, set_cooldown / ✓ | |
| `timers.create/update/delete/get/list`, `timers.set_state`, `timers.set_interval` | 2 (reads 0) | ✓ / ✓ / ✓ | |
| `timers.reset_counter` | 1 | ✓ / ✓ / ✓ | |
| `system.export`, `system.import`, `system.audit`, `system.reset_state` | 0 / 2 / 0 / 1 | ✓ / — / export, audit | `reset_state` clears temp globals, cooldowns, signals and publishes `system.reset` |

Events: `system.started`, `system.stopping`, `system.crash_recovered`, `system.reset`, `system.events_dropped`,
`system.trigger_storm`, `system.document_invalid`, `system.storage_readonly`, `system.storage_corrupt`,
`runs.queued/started/node_started/node_completed/completed`, `queues.changed/run_rejected/run_dropped`,
`variables.global_updated/user_updated`, `commands.triggered/cooldown`, `schedule.timer_fired`, `schedule.cron`,
`core.test`, `core.custom`.

**User-defined events (row 2.36).** `events.emit` publishes **`core.custom { name /* ^[A-Za-z_][A-Za-z0-9_.-]{0,63}$
*/, data }`** through the bus like any event: `causedBy` is the emitting run (or synthetic run), so chains and the
hop limit apply, and `data`'s top-level fields seed args. Actions listen with trigger type `core.custom` and filter
`name eq …` (the editor's "Custom event" trigger; `name` has catalog specificity 4 so a named trigger beats a
catch-all). Scripts call it through spec 04's facade; Streamer.bot `TriggerCodeEvent`/`Trigger Custom Event`
import to it. There is no registration step: a trigger on an unknown name simply never fires. `runs.node_*` and `variables.user_updated` are `highVolume`; `system.stopping` and
`system.storage_*` are `critical`.

## 16. Settings (document `kind: settings`, schema-generated form)

| Key | Default | Range | Used in |
|---|---|---|---|
| `engine.bus.capacity` / `internalCapacity` | 8192 / 16384 | 256–65536 | §5.1, §5.3 |
| `engine.run.maxConcurrent` | 256 | 1–4096 | §9.13 |
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
| `engine.signals.queuedTtlMs` / `maxWaitMs` | 30000 / 30000 | ≤ 300000 | §9.12 |
| `engine.variables.maxValueBytes` | 262144 | 1024–4194304 | §11.4 |
| `engine.storage.flushMs` | 1000 | 0 (sync) – 10000 | §11.4 |
| `engine.storage.dailyBackup` | true | | §13.6 |
| `engine.history.maxRuns` / `maxAgeDays` / `maxSnapshotBytes` / `traceRuns` | 10000 / 7 / 262144 / 200 | | §12 |
| `engine.events.maxAgeHours` / `maxCount` | 24 / 50000 | | §12.5 |
| `engine.shutdown.graceMs` | 5000 | 0–60000 | §10.8 |
| `engine.action.maxNodes` / `maxDepth` | 2000 / 32 | | §9.13 |

Changing a setting is a tier-2 configuration write (`settings.update`, draft-first under MCP). Every setting is
hot-applied except `engine.bus.capacity` and `internalCapacity` (restart).

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
- **Nothing in `den-core` is OS-gated**; OS-specific sources (hotkeys, MIDI, file watching) are connectors.

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
- **Untrusted-text taint.** Each run keeps a set of tainted arg names, seeded with the payload fields the catalog
  marks `x-den-untrusted` and extended whenever a writer's template or `saveAs` source references a tainted name
  (the same syntactic reference check as spec 02's dynamic fields; `Code.setArgs` taints every name it writes).
  The set is stored with both history snapshots (`untrustedArgs`) and returned by `runs.get`, so MCP and the AI
  layer wrap those values and an approval request never shows tainted text as if it were configuration (spec 02
  §16). Globals written from tainted args are **not** tracked beyond the run (fail-open, documented: wrap chat
  text before storing it, or keep it in args).
- `x-secret` values are redacted from snapshots, trace events, exports and the event log before they leave the
  operation that produced them.
- The storage files are created `0600` on Unix; the data directory is created `0700` (Windows: per-user ACL, §19).
  Backups inherit this.
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
   two cron triggers with different expressions never fire each other; `core.custom` named vs catch-all.
2. **Filter operator table tests:** every `FilterOp` against every JSON type, missing fields, `between` bounds,
   number normalization, case rules.
3. **Scheduler proptests** (`proptest`, paused Tokio clock): random workloads of ≤ 64 runs with random durations,
   `alwaysRun`, cancellations, pause windows, clears, blocking toggles and overflow policies, asserting: blocking
   queues never run two at once; FIFO among eligible runs; no ordinary run starts inside a pause window; the bound
   never rejects a run that could start; `drop_oldest` evicts the oldest waiting run; counters equal outcomes;
   every run reaches exactly one terminal state. Plus the B-1 regressions: shutdown with 20,000 waiting runs
   (no stack growth), cancellation of a running run in a blocking queue on a 4-thread runtime (no overlap; slot
   released only after the future is dropped), a panicking run frees its slot, always-run on a paused and full
   queue starts.
4. **Executor semantics:** each node kind; `autoType` and `ignoreCase` matrices; `Switch` default; loop cap with
   warning; `BreakLoop` outside a loop; `Stop` in child, in branch, at top level; `Parallel` deterministic merge
   and sibling cancellation; inline arg flow in and back; queued child seeding and `queue_full` failure; recursion
   and lineage limits; `Code` `ok: false` vs exception; `onError` stop vs continue and `run.lastError`; missing
   value warnings; args size limit.
5. **Variables:** atomic increment under 1,000 concurrent runs; persisted/temp move; change events with
   `old`/`new`; durability: `kill -9` after a write and before `flushMs` loses at most that write, after
   `durable: true` loses nothing (test harness with a child process).
6. **History and replay:** snapshots at admission and after; redaction; truncation marker; retention pruning keeps
   recent failures; replay uses the current revision and the caller's context; `events.replay` attribution.
7. **Timers and cron:** paused-clock tests for drift-free intervals, direct dispatch to the owning trigger, random ranges, `minChatLines` gate, `repeat:
   false` disabling, missed-tick coalescing after a simulated suspend; cron DST skip and fall-back, `catchUp`,
   time zone changes, leap day.
8. **Storage:** fresh schema; every DDL migration with before/after fixtures and a backup created; newer-schema
   read-only; corrupt `history.db` recreated; corrupt `config.db` moved aside; export → import round-trip equals
   the original documents; invalid document loads disabled.
9. **Authorization (engine side of spec 02 §17.9):** every chain, approval and refusal case there, run against the
   real executor; attribution table expiry → `chain_context_lost`; hop limit at 16 under a user token; storm event;
   **laundering**: an MCP-origin `chat.send` echoed by the platform, attributed, matched by a command with
   `ignoreInternal: false` → the `commands.triggered` run inherits the MCP context and its `Code` node is refused.
10. **Performance:** criterion benches for match, admission, node overhead, `variables.set`; a soak test at 5,000
    events/s for 10 minutes checking the §20 budgets and zero idle wake-ups afterwards (`strace`/ETW count).
11. **Cross-platform:** the storage, timer and shutdown tests run on Linux and Windows CI (Windows compiles and
    runs unit tests, D-035).

## 23. Contract amendments requested from spec 02 (`den-protocol`)

Spec 02 is approved; these are additive requests for the contract task that precedes implementation (AGENTS.md
"Contracts first"), tracked as board card **DEN-33**. None blocks the rest of this spec: where an amendment has not
landed, the stated default applies.

- **A1 — `SubAction.onError: OnError { stop | continue }`** (default `stop`) and the read-only run-context fields
  `run.lastError` (§9.10) and `run.queueName` (§6.4). Needed for Streamer.bot import parity and rows 2.08, 3.09.
- **A2 — `Node::ForEach { items: Template, itemVar: String, indexVar: Option<String>, maxIterations:
  Option<u32>, children }`** (row 2.07, P1). Until it lands the importer and the editor use `While` with a
  counter. `Node` is documented as extensible with a placeholder fallback, so this is non-breaking.
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
- **Scopes:** `events:simulate`, `events:emit`.

## 24. Open questions (added to [QUESTIONS.md](../QUESTIONS.md), section E)

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

Assumptions made while writing (reviewers, please check): spec 02 r4 is the contract base and its rules are not
restated here; the alert queue is a spec 09 concern layered on events, not an action queue; `Action.concurrent`
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
