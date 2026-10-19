# Streamer.bot — Core Automation Engine & UI (Research for Streamer.live parity)

> **Snapshot date:** 2026-10-18. **Scope:** core engine (actions, sub-actions, variables, commands, C#, servers/clients), UI/UX, data/settings, licensing and tech stack. Triggers and platform/3rd-party integrations are covered by a separate document. I only touch them here where the engine needs context.
>
> **Primary sources:** the official docs site <https://docs.streamer.bot> and its source repo <https://github.com/Streamerbot/docs> (cloned at commit `56253da`, 2026-10-14), the official JS client repo <https://github.com/streamerbot/client>, the release API `https://streamer.bot/api/releases/streamer.bot`, and the shipped 1.0.7 zip. For the zip I only read the **file listing** and a few version strings. Nothing was executed. Statements marked **(inferred)** are my reading of indirect evidence, not documented fact.

## 0. Current version status

| Channel | Version | Released | Notes | Source |
|---|---|---|---|---|
| **Stable** | **1.0.7** (hotfix that rolls up 1.0.5 + 1.0.6) | 2026-08-06 | 28.3 MB zip, `Streamer.bot-x64-1.0.7`, SHA-256 `47a4e56f…c5` | `https://streamer.bot/api/releases/streamer.bot` (`stable` object); [changelog v1.0.5](https://docs.streamer.bot/changelogs/v1.0.5) |
| Beta | 1.0.5 | 2026-08-05 | Beta channel. Needs the Discord "Beta Testers" role plus a website login | [changelogs](https://docs.streamer.bot/changelogs), [beta FAQ](https://docs.streamer.bot/faq) |
| **Alpha** | **1.1.0-alpha.1** | 2026-10-09 | Patreon Bronze tier or higher. Adds an integrated **Points service**, a **Cron** trigger, controller input (SDL2), Whisper.net voice control, Elgato Wave Link 3 / Studio / Control Center, a new queue handler, and per-run isolated C# execution context | [changelog v1.1.0](https://docs.streamer.bot/changelogs/v1.1.0) |
| Previous stables | 1.0.4 (2026-01-16), 1.0.3 (2026-01-12), 1.0.1 (2025-08-28), 1.0.0 (2025-08-20), 0.2.8 (2025-04-09) | | 1.0.0 introduced the WPF rewrite, dark mode, Kick, nestable groups, Switch/While, Monaco and supporter perks | [changelog v1.0.0](https://docs.streamer.bot/changelogs/v1.0.0) |

The developer is nate1280 (solo, full-time). Docs, client and web tooling come mainly from Whipstickgostop.

---

## 1. Key architecture concepts (TL;DR)

1. **Action = ordered tree of sub-actions + N triggers + a queue.** Events become triggers, triggers enqueue actions, and an action runs its sub-actions top-down over a shared **argument stack**, a `Dictionary<string, object>` ([Actions guide](https://docs.streamer.bot/guide/core/actions)).
2. **Many-to-many wiring.** A trigger lives inside an action. Several actions can own the same trigger type, and an action can have many triggers ([Triggers guide](https://docs.streamer.bot/guide/core/triggers)).
3. **Queues** control concurrency. `Default` is non-blocking. User-created queues can be **blocking** (serial) and can be paused, resumed or cleared. Every run is recorded in **Pending** and **Action History** with variable snapshots taken before and after the run.
4. **Variables:** `%arg%` text substitution with .NET format specifiers (`%x:c2%`). `~persistedGlobal~` reads a persisted global. `$math()$`, `$length()$` and `$parse()$` are inline functions. Globals come in persisted and temp flavours. **User globals** are per platform and per user.
5. **Control flow is itself a sub-action:** If/Else with True/False containers, Switch/Case, While, Loop Break/Continue, Break, nestable **groups** (random and weighted), Run Action (inline or queued), and local/remote **signals** (wait/notify).
6. **The escape hatch is C#.** Roslyn compiles a `CPHInline` class in-process. `CPH` is a facade over the whole app, with about 627 documented methods. Methods can be shared across actions, and C# can register **custom triggers**.
7. **API surface:** a WebSocket server with request/response plus pub/sub events (password challenge auth) and an official JS client. There are also an HTTP server (actions, credits, static files), a UDP server, user-defined custom WebSocket servers and clients, a Stream Deck server, and cloud-relayed remote instances and webhooks.
8. **Distribution:** a portable, closed-source, free Windows app on .NET Framework 4.8.x / WPF. Linux and macOS run only under Wine, and that is experimental.

---
## 2. Actions model

### 2.1 Action entity
Source: [Actions guide](https://docs.streamer.bot/guide/core/actions) and decoded export JSON (see §10).

| Property (UI) | Export key | Default | Semantics |
|---|---|---|---|
| Name | `name` | required | Friendly name. Actions can be run **by name or by GUID** `id`. |
| Group | `group` | None | Free-text folder used only to organise the actions list. Groups can be enabled or disabled in bulk through sub-actions/C#. |
| Queue | `queue` (GUID; `0000…` = Default) | Default | The action queue the run is placed in. |
| Enabled | `enabled` | true | Disabled actions do not run. |
| Random Action (RA) | `randomAction` | false | Runs **one** randomly chosen top-level sub-action. A group counts as a single entity. Turning it on enables per-sub-action `weight`. |
| Concurrent (CC) | `concurrent` | false | Starts **all** sub-actions simultaneously. **Ignores `Delay`.** |
| Always Run | `alwaysRun` | false | Runs even when its queue is **paused**. |
| Exclude from Pending/History | `excludeFromHistory`, `excludeFromPending` | false | Performance option for high-frequency actions. |
| (UI state) | `collapsedGroups` | [] | Persisted collapse state of sub-action groups. |
| Triggers | `triggers[]` | | Each trigger has `{id, type:int, enabled, exclusions[], …criteria}`. |
| Sub-actions | `subActions[]` (≥1.0), `actions[]` (legacy) | | A tree. See §2.2. |

Context menu on an action: Add, Edit, Delete (with a confirmation you can suppress and re-enable in Settings), Duplicate, Copy Action ID, Collapse/Expand All, quick toggles for the options above, and Add to Export. Keyboard: `Del`, `Ctrl+D` duplicate, `Ctrl+C` copy, `Ctrl+O` toggle enabled ([Shortcuts](https://docs.streamer.bot/guide/advanced/shortcuts)). Multi-select works for deletion.

### 2.2 Sub-action tree and control flow
- Every sub-action shares `{id, type:int, parentId, enabled, index, weight}` plus type-specific fields. Since 1.0 the list is a **tree**. Groups (`type 99900`, with `name`, `color` and `random`) **nest without limit** ([v1.0.0 changelog](https://docs.streamer.bot/changelogs/v1.0.0)).
- **If/Else** (`type 120`) contains two container children: `99901` (True) and `99902` (False). Sub-actions sit directly in each branch. Operators: Equals, Not Equals, Contains, Regex Match, Less Than, Greater Than, Does Not Exist, Equals/Not Equals (Ignore Case), Is Null or Empty. It has an **Auto Type** toggle. Regex matches populate `match.count`, `match[i]` and named groups as arguments ([If/Else](https://docs.streamer.bot/api/sub-actions/core/logic/if-else)). Legacy If/Else instead pointed at an `actionId`/`elseActionId` with `runImmediately`.
- **Switch** holds Case groups (each with one or more values) plus **Default**. Auto Type is available ([Switch](https://docs.streamer.bot/api/sub-actions/core/logic/switch), [FAQ](https://docs.streamer.bot/faq)).
- **While** (`type 128`) uses Equals/Not Equals/Less Than/Greater Than with Ignore Case and Auto Type. **Loop Break / Loop Continue** arrived in 1.0.5. The context menu has a **"Break Running"** item to kill an infinite loop ([While](https://docs.streamer.bot/api/sub-actions/core/logic/while)).
- **Break** (`124`) stops the action. In C#, `Execute()` returning `false` also stops the remaining sub-actions, unless "Save Result to Variable" is on.
- **Random/weights:** this works at action level (RA) or group level (`random`), with per-item **Weight**. The weighted list is cached and invalidated when weights or members change ([v0.2.x changelogs](https://docs.streamer.bot/changelogs)). `Set Sub-Action Weight` changes weights at runtime.
- Sub-action context menu: Favorites, Edit (double-click), Copy, Paste, Duplicate, Delete, Add/Delete/Rename Group, Move to Group, **Enabled** (disabled items show in red text), Random (groups), Weight, Delete all, Move Up/Down/Top/Bottom (`Ctrl+↑/↓/Home/End`), plus drag-and-drop into and out of groups. Copy/paste uses the clipboard as a Base64 string, so it can be pasted into another instance.

### 2.3 Execution semantics
- **Sequential by default.** Each sub-action can read and write the argument stack. Variables set by a sub-action are only available to sub-actions **after** it ([Variables guide](https://docs.streamer.bot/guide/core/variables)).
- **Run Action** (`type 4`), `CPH.RunAction(name, runImmediately)` and `RunActionById`:
  - `Run Immediately = true` runs the target **inline**, like nested sub-actions. Its variables **flow back** to the caller.
  - `false` enqueues the target into its own queue and the caller continues ([Run Action](https://docs.streamer.bot/api/sub-actions/core/actions/run-action), [C# RunAction](https://docs.streamer.bot/api/csharp/methods/core/actions/run-action)).
- **Signals** (since 0.2.4) are cross-action synchronisation:
  - `Send Local Signal` takes a name, can include the action's args or listed args, and can **queue** the signal for a waiter that has not started yet.
  - `Wait for Local Signal` takes a name, an optional variable overwrite and a timeout (max and default 30 s). It outputs `signalTimeout`, `hasResult` and `signalName`.
  - Remote variants work across instances via the website relay ([Signals](https://docs.streamer.bot/api/sub-actions/core/signals/wait-for-local-signal)).
- **Enable/disable at every level:** action, action group, sub-action, trigger, command, command group, timer and queue (pause). Each has get/set sub-actions and C# methods.
- **Execution tracing events:** the WebSocket `Raw.Action`, `Raw.SubAction` and `Raw.ActionCompleted` events stream every action and sub-action execution ([client event constants](https://github.com/streamerbot/client/blob/main/packages/client/src/ws/types/events/events.const.ts)). That makes external debuggers and visualisers possible.

### 2.4 Queues
Source: [Actions guide, Action Queues section](https://docs.streamer.bot/guide/core/actions#action-queues).

| Feature | Detail |
|---|---|
| Default queue | Auto-created and **non-blocking**: actions run immediately and concurrently. Since 1.0.5 its blocking flag applies without a restart. |
| Custom queue | Name plus **Blocking** toggle. Blocking means FIFO, one action at a time. The canonical use is alerts. |
| Queue list columns | Pending Count, Completed Count (includes cancelled), Paused, Blocking |
| Queue context menu | Add, Pause/Resume, Blocking toggle, Clear (drop pending), Pause All, Resume All. Double-click to edit and hotkeys arrived in 1.0.5. |
| Sub-actions | Clear Action Queue, Set Action Queue Blocking State, Set Action Queue Pause State |
| C# | Pause/Resume(All)ActionQueue. The docs site also lists `ForceActionById`, `ForceNextAction`, `ForceRandomAction`, `GetPausedActions`, `GetQueuePendingCount`, `IsQueuePaused` and `ShuffleActionsInQueue` (newer "Actionqueues" group) ([C# core methods](https://docs.streamer.bot/api/csharp/methods/core)) |
| Pending Actions view | Shows queued and blocked runs |
| Action History view | Columns: Name, State (Completed/Cancelled/Pending), Variables count, Duration (ms), Queue, Queued At, Started At, Completed At. Context menu: **Inspect Variables When Queued**, **Inspect Variables After Run**, **Requeue**, Clear History. 1.1.0 adds sortable columns, performance work and an **APS (actions/sec) display**. |
| Run-scope variables | `%actionId%`, `%actionName%`, `%runningActionId%`, `%queueName%` (1.0.5), `%actionQueuedAt%`, `%triggerId%`, `%triggerName%`, `%triggerCategory%` ([General variables](https://github.com/Streamerbot/docs/blob/main/streamerbot/3.api/.variables/General.md)) |

### 2.5 Triggers (engine-side behaviour only)
Source: [Triggers guide](https://docs.streamer.bot/guide/core/triggers).

- **Criteria types:**
  - Basic: no configuration.
  - Toggle: checkbox filters.
  - Select: dropdown with an `Any` option.
  - **Range:** min/max inclusive. Max only means an exact match. Both empty means all values.
  - Text: empty means all.
- **Precedence ("Always Run" on range triggers).** Suppose one trigger is set to *any* bits and another to exactly 100 bits. Without Always Run, the `Any` trigger is skipped when a more specific range matches. With Always Run on, both fire. Always Run triggers are shown in **blue**.
- Trigger context menu: **Test Trigger** (`F5`), Edit, Copy/Paste (Base64), Delete, Enabled, Refresh Triggers, Clear All, Enable All, Disable All, Always Run. The `?` icon opens the **Trigger Viewer**, a global map of triggers to actions.
- **Testing:** the `Core > Test` trigger supplies `isTest=True`. Its export form (`type 702`) carries a `variables{}` dictionary of custom test args, which acts as a test fixture. There are also simulated events.
- `exclusions[]` exists on every trigger in exports **(inferred: per-trigger exclusion filters, e.g. users. Details are undocumented)**.
- **Custom triggers:** C# `CPH.RegisterCustomTrigger(label, eventName, string[] categoryPath)` adds an entry under `Custom` in the trigger menu. Re-register it on every boot from `Init()` with precompile on. Fire it with `CPH.TriggerCodeEvent(eventName, useArgs)`, the `Trigger Custom Event` sub-action, or the WebSocket `ExecuteCodeTrigger`. There is also the UI-defined `Custom Event` trigger, fired by `CPH.TriggerEvent(name, useArgs)`.
- `Set Trigger State` sub-action (0.2.5) enables or disables individual triggers on an action.
- Scale: the docs list about 455 trigger pages. The C# `EventType` enum has **489 values** over 52 sections ([EventType](https://docs.streamer.bot/api/csharp/enums/event-type)).

---
## 3. Sub-action catalog

The docs claim "more than 300 sub-actions" ([Introduction](https://docs.streamer.bot/get-started/introduction)). Counting reference pages in the docs repo (`streamerbot/3.api/1.sub-actions`) gives **421 documented sub-actions**. The app ships some that the docs lag on (see §3.3).

### 3.1 Counts by category

| Category | Documented sub-actions | Sub-groups |
|---|---|---|
| Core | 82 | (root), action-queues, actions, arguments, commands, csharp, file-io, file-tail, globals, logic, midi, misc, network, points/admin, points/bulk, points/groups, points/leaderboards, points/user, quotes, signals, sounds, system, timers, triggers, users, voice-control |
| Twitch | 95 | ads, channel, charity, chat, followers, guest-star, moderation, moderation/chat-modes, moderation/shield, polls, predictions, raid, rewards, sub-counter, subscribers, tags, user, user-groups |
| YouTube | 9 | (root), moderation, user-groups |
| Kick | 14 | channel, chat, moderation, user, user-groups |
| OBS Studio | 29 | (root), filters, groups, replay, scenes, sources |
| Streamlabs Desktop | 16 | (root), filters, groups, scenes, sources |
| Meld Studio | 10 | (root), effects, layers, tracks |
| PolyPop | 1 | (root) |
| Elgato | 90 | camera-hub/epoccam, camera-hub/prompter, camera-hub/webcam, control-center/brightness, control-center/power, control-center/temperature, stream-deck, studio, studio/preview, studio/recording, wave-link, wave-link-3/channels, wave-link-3/inputs, wave-link-3/mixes, wave-link-3/outputs |
| Integrations | 45 | discord, ifttt, lumia-stream, mtion, pronouns, streamfog/backgrounds, streamfog/lenses, streamfog/outfits, tits, voicemod, vtube-studio |
| Speaker.bot | 7 | (root) |
| Streamer.bot (Decks / Remote) | 13 | decks, remote |
| StreamFog (standalone category) | 6 | lenses, outfits |
| Settings | 4 | (root) |
| **Total** | **421** | |


**Platform/integration share:** Twitch 95, Elgato 90, Integrations 45, OBS 29, Streamlabs Desktop 16, Kick 14, Meld 10, YouTube 9. Core is 82, of which **18 are the new Points sub-actions (1.1.0)**.

### 3.2 Internal sub-action and trigger type IDs (from decoded exports)
Every sub-action and trigger is persisted with a numeric `type`. These IDs come from decoding the import strings in the docs repo (§10). They matter if we ever want to import Streamer.bot exports.

| type | Sub-action | type | Sub-action |
|---|---|---|---|
| 4 | Run Action (`actionId`, `runImmedately` [sic]) | 1000 | Get Quote (`quoteType`, `quoteId`) |
| 6 | Run a Program (`command`, `arguments`, `workingDir`, `envVars{}`, `waitForExit`) | 1007 | Fetch URL (`url`, `variableName`, `headers{}`, `parseAsJson`) |
| 10 | Twitch Send Message (`text`, `useBot`/`preferredAccount`, `fallback`) | 1009 | Comment (`value`, `color`) |
| 46 | OBS Raw (`name`, `prefix`, `raw` JSON, `resultsToArgs`, `connectionId`) | 1013 | Get Commands (`groupName`, `variableName`, `includeAll`, `ignoreAliases`, `hasPermission`) |
| 120 | Logic If/Else (`input`, `operation`, `value`, `autoType`) | 1020 | Log (`logLevel`, `message`) |
| 121 | Get Global Variable (`source`, `variableName`, `persisted`, `destinationVariable`, `defaultValue`) | 1036 / 1037 | Delete Quote / Add Quote |
| 122 | Set Global Variable (`destination`, `variableName`, `persisted`, `source`=Value/Inc/Dec, `value`) | 11001 / 11002 / 11008 | Stream Deck Set Title / Set Background / Set Value |
| 123 | Set Argument (`variableName`, `value`, `autoType`) | 99900 | **Sub-action group** (`name`, `color`, `random`) |
| 124 | Break | 99901 / 99902 | If/Else **True** / **False** container |
| 128 | Logic While (`input`, `operation`, `value`, `autoType`, `ignoreCase`) | 99999 | **Execute C# Code** (`name`, `description`, `references[]`, `byteCode` = Base64 source, `precompile`, `delayStart`, `saveResultToVariable`, `saveToVariable`) |
| 550 | Twitch Add Present User (`userIndex`) | 21, 24, 29, 50, 51, 506 | Seen in exports but not identified (Twitch user-info family **(inferred)**) |

Trigger types seen: `401` Command Triggered (`commandId`), `701` Timed Action (`timerId`), `702` Test (`variables{}`).

### 3.3 Sub-actions in changelogs but missing from the reference pages
- Tray Notification. Pick Color, Set Voice Control Input, Reset Credits and Reset First Words are under `Settings`.
- Streamlabs Spin Wheel. YouTube Create Poll / End Poll. Twitch Get Clip Download Urls (1.0.3). Kick Channel Reward management (1.0.3).
- Twitch add/remove **suspicious user status** (1.0.5). Stream Deck MCP "trigger button" (1.0.5). Meld Set Browser Layer Url and Take Vertical Screenshot (1.0).
- Many 1.1.0 additions (Cron, controller input, Wave Link 3, Elgato Studio/Control Center) appear only partially ([v1.0.0](https://docs.streamer.bot/changelogs/v1.0.0), [v1.0.3](https://docs.streamer.bot/changelogs/v1.0.3), [v1.0.5](https://docs.streamer.bot/changelogs/v1.0.5), [v1.1.0](https://docs.streamer.bot/changelogs/v1.1.0)).
- Trovo was **removed** as a platform in 1.0.5. Its C# methods remain temporarily.
- **There is no "WebSocket send" sub-action** in the reference. Sending to custom WS servers or clients is C#-only (`WebsocketSend`, `WebsocketCustomServerBroadcast`) **(inferred from the docs)**.

### 3.4 Notable core sub-action semantics
- **Delay** takes ms with optional random `To` range. It is ignored when the action is Concurrent.
- **Set Argument:** Variable Name is parsed (1.0.5). Value supports `%vars%` and inline functions, with Auto Type.
- **Get/Set Global Variable:** Source/Destination is one of `Global`, `User (redeemer)` or `User (target)` (the `targetUser` arg); Persisted or Non-Persisted. Set modes are `Value`, `Increment` and `Decrement` (default 1). Get has a Default Value that creates the variable if missing ([shared params](https://github.com/Streamerbot/docs/tree/main/streamerbot/3.api/.parameters/core)).
- **Run a Program:**
  - Accepts exe, `node`, `python`, `.bat`, `.ps1` or any URI (`streamdeck://…`).
  - Optional working dir, args, env vars.
  - **Wait for Exit (s)** captures stdout into `output#` / `outputCount`. This is the official way to bridge to Python/Node ([Run a Program](https://docs.streamer.bot/api/sub-actions/core/system/run-a-program), [Execute scripts example](https://docs.streamer.bot/examples/execute-scripts)).
- **Fetch URL:** GET only, with custom headers. "Parse as JSON" flattens the response to `<var>.<path>` arguments. POST needs C# ([Fetch URL](https://docs.streamer.bot/api/sub-actions/core/network/fetch-url)).
- **Play Sound / Play Sound From Folder:** audio device selection, `mp3/wav/m4a/ogg`, "finish before continuing", volume, and a named sound so `Stop Sound Playback` can stop it. Outputs `soundLength`, `soundLengthMs`, `randomSoundFile`.
- **File I/O:** Read Lines gives `line#` and `lineCount`. There are also Read Random Line(s) and Read Specific Line. All have optional variable parsing and auto-typing. Write To File (append). File/Folder Exists (1.0.1). Set File Tail State.
- **Get Input:** a modal input dialog that writes `success` and `inputResult`.
- **Toast Notification:** a Windows toast. Activation fires the `Toast Activation` trigger.
- **Keyboard Press:** simulates key plus modifiers.
- **MIDI out:** Note On, Control Change, Generic.
- **UDP Broadcast:** port plus a payload string.
- **Comment:** coloured, a no-op.
- **Log:** level plus message.

### 3.5 Full per-category tables
Generated from the docs-repo frontmatter. Each link goes to the reference page. "Since" is the documented minimum version, and blank means pre-0.2. Parameter and variable lists are truncated to the first few.


#### Core (82)

| Group | Sub-action | Since | Key parameters | Output variables |
|---|---|---|---|---|
| flow | [Delay](https://docs.streamer.bot/api/sub-actions/core/delay) |  | Delay, Random, To |  |
| action-queues | [Clear Action Queue](https://docs.streamer.bot/api/sub-actions/core/action-queues/clear-action-queue) |  |  |  |
| action-queues | [Set Action Queue Blocking State](https://docs.streamer.bot/api/sub-actions/core/action-queues/set-action-queue-blocking-state) |  |  |  |
| action-queues | [Set Action Queue Pause State](https://docs.streamer.bot/api/sub-actions/core/action-queues/set-action-queue-pause-state) |  |  |  |
| actions | [Get Action Group State](https://docs.streamer.bot/api/sub-actions/core/actions/get-action-group-state) |  | Group | actionGroupState, actionsEnabled, actionsDisabled |
| actions | [Get Action State](https://docs.streamer.bot/api/sub-actions/core/actions/get-action-state) |  | Action | actionState |
| actions | [Run Action](https://docs.streamer.bot/api/sub-actions/core/actions/run-action) |  | Action, Run Action Immediately |  |
| actions | [Set Action Group State](https://docs.streamer.bot/api/sub-actions/core/actions/set-action-group-state) |  | Group, State |  |
| actions | [Set Action State](https://docs.streamer.bot/api/sub-actions/core/actions/set-action-state) |  | Action, State |  |
| actions | [Set Sub-Action State](https://docs.streamer.bot/api/sub-actions/core/actions/set-sub-action-state) | 1.0.0 | Action, Sub-Action, State |  |
| actions | [Set Sub-Action Weight](https://docs.streamer.bot/api/sub-actions/core/actions/set-sub-action-weight) | 1.0.0 | Action, Sub-Action, Weight |  |
| arguments | [Set Argument](https://docs.streamer.bot/api/sub-actions/core/arguments/set-argument) |  | Variable Name, Value, Auto Type |  |
| commands | [Get Command Group State](https://docs.streamer.bot/api/sub-actions/core/commands/get-command-group-state) |  |  | commandGroupState, commandsEnabled, commandsDisabled |
| commands | [Get Command State](https://docs.streamer.bot/api/sub-actions/core/commands/get-command-state) |  |  | commandState |
| commands | [Get Commands](https://docs.streamer.bot/api/sub-actions/core/commands/get-commands) |  | Group, Variable Name, Include All, Only Has Permission, Ignore Aliases | <variableName>, <variableName>List |
| commands | [Set Command Group State](https://docs.streamer.bot/api/sub-actions/core/commands/set-command-group-state) |  |  |  |
| commands | [Set Command State](https://docs.streamer.bot/api/sub-actions/core/commands/set-command-state) |  |  |  |
| csharp | [Execute C# Code](https://docs.streamer.bot/api/sub-actions/core/csharp/execute-csharp-code) |  | Name, Description, Precompile on Application Start, Delayed Start, Save Result to Variable, Variable Name |  |
| csharp | [Execute C# Method](https://docs.streamer.bot/api/sub-actions/core/csharp/execute-csharp-method) |  | Execute C# Code, Method, Run on UI Thread, Save Result to Variable, Variable Name |  |
| file-io | [File Exists](https://docs.streamer.bot/api/sub-actions/core/file-io/file-exists) | 1.0.1 | File to Check if Exists, Variable Name | fileExists |
| file-io | [Folder Exists](https://docs.streamer.bot/api/sub-actions/core/file-io/folder-exists) | 1.0.1 | Folder to Check if Exists, Variable Name | folderExists |
| file-io | [Read Lines From File](https://docs.streamer.bot/api/sub-actions/core/file-io/read-lines-from-file) |  | File to read from, Variable Name, Parse Variables, Attempt auto-typing | lineCount, line#, fileFound |
| file-io | [Read Random Lines From File](https://docs.streamer.bot/api/sub-actions/core/file-io/read-random-line-from-file) |  | File to read from, Variable Name, Parse Variables, Attempt auto-typing, Count | randomLine#, randomLineNumber#, fileFound |
| file-io | [Read Specific Line From File](https://docs.streamer.bot/api/sub-actions/core/file-io/read-specific-line-from-file) |  | File to read from, Variable Name, Parse Variables, Attempt auto-typing, Line Number | line, fileFound |
| file-io | [Write To File](https://docs.streamer.bot/api/sub-actions/core/file-io/write-to-file) |  | File to write to, Append to file, Text to write |  |
| file-tail | [Set File Tail State](https://docs.streamer.bot/api/sub-actions/core/file-tail/set-file-tail-state) |  | File Tail, State |  |
| globals | [Clear Non-Persisted Globals](https://docs.streamer.bot/api/sub-actions/core/globals/clear-non-persisted-globals) | 0.2.3 |  |  |
| globals | [Clear Non-Persisted User Globals](https://docs.streamer.bot/api/sub-actions/core/globals/clear-non-persisted-user-globals) | 0.2.3 |  |  |
| globals | [Get Global Variable](https://docs.streamer.bot/api/sub-actions/core/globals/global-get) |  | Source, Persisted, Variable Name, Destination Variable, Default Value | <variableName> |
| globals | [Set Global Variable](https://docs.streamer.bot/api/sub-actions/core/globals/global-set) |  | Destination, Persisted, Variable Name, Value |  |
| logic | [Break](https://docs.streamer.bot/api/sub-actions/core/logic/break) |  |  |  |
| logic | [Get Input](https://docs.streamer.bot/api/sub-actions/core/logic/get-input) |  | Title, Prompt, Original Text, Output Variable, Auto Type Output | success, <inputResult> |
| logic | [Get Random Number](https://docs.streamer.bot/api/sub-actions/core/logic/get-random-number) |  | Type, Between | randomNumber, randomFloat, randomPercent |
| logic | [If / Else](https://docs.streamer.bot/api/sub-actions/core/logic/if-else) |  | Input, Operator, Auto Type, Value | match.count, match[<index>], <groupName> |
| logic | [Loop Break](https://docs.streamer.bot/api/sub-actions/core/logic/loop-break) | 1.0.5 |  |  |
| logic | [Loop Continue](https://docs.streamer.bot/api/sub-actions/core/logic/loop-continue) | 1.0.5 |  |  |
| logic | [Switch](https://docs.streamer.bot/api/sub-actions/core/logic/switch) | 1.0.0 | Input, Auto Type |  |
| logic | [While](https://docs.streamer.bot/api/sub-actions/core/logic/while) | 1.0.0 | Input, Operator, Value, Auto Type, Ignore Case |  |
| midi | [Control Change](https://docs.streamer.bot/api/sub-actions/core/midi/control-change) |  |  |  |
| midi | [Generic Event](https://docs.streamer.bot/api/sub-actions/core/midi/generic-event) |  |  |  |
| midi | [Note On](https://docs.streamer.bot/api/sub-actions/core/midi/note-on) |  |  |  |
| misc | [Comment](https://docs.streamer.bot/api/sub-actions/core/misc/comment) |  | Comment, Color |  |
| misc | [Log](https://docs.streamer.bot/api/sub-actions/core/misc/log) |  | Log Level, Message |  |
| network | [Fetch URL](https://docs.streamer.bot/api/sub-actions/core/network/fetch-url) |  | URL, Variable Name, Parse result as JSON, Headers | <variableName>, <variableName>.<jsonKey> |
| network | [UDP Broadcast](https://docs.streamer.bot/api/sub-actions/core/network/udp-broadcast) |  | UDP Port, Payload Data |  |
| points/admin | [Get Points Definition](https://docs.streamer.bot/api/sub-actions/core/points/admin/get-points-definition) | 1.1.0 | Point Definition, Destination Variable |  |
| points/admin | [Reset Points Definition](https://docs.streamer.bot/api/sub-actions/core/points/admin/reset-points-definition) | 1.1.0 | Point Definition |  |
| points/bulk | [Award Points to All](https://docs.streamer.bot/api/sub-actions/core/points/bulk/award-points-to-all) | 1.1.0 | Point Definition, Amount, Platform, Only Award to Active Users, Apply User/Group Multipliers |  |
| points/groups | [Award Points to Group](https://docs.streamer.bot/api/sub-actions/core/points/groups/award-points-to-group) | 1.1.0 | Point Definition, Group, Amount, Apply User/Group Multipliers |  |
| points/groups | [Get Group Multiplier](https://docs.streamer.bot/api/sub-actions/core/points/groups/get-group-multiplier) | 1.1.0 | Point Definition, Group, Destination Variable |  |
| points/groups | [Remove Group Multiplier](https://docs.streamer.bot/api/sub-actions/core/points/groups/remove-group-multiplier) | 1.1.0 | Point Definition, Group |  |
| points/groups | [Set Group Multiplier](https://docs.streamer.bot/api/sub-actions/core/points/groups/set-group-multiplier) | 1.1.0 | Point Definition, Group, Multiplier |  |
| points/leaderboards | [Get Points Leaderboard](https://docs.streamer.bot/api/sub-actions/core/points/leaderboards/get-points-leaderboard) | 1.1.0 | Point Definition, Count, Offset |  |
| points/user | [Award Points](https://docs.streamer.bot/api/sub-actions/core/points/user/award-points) | 1.1.0 | Point Definition, User, platform, Amount, Apply User/Group Multipliers, Destination Variable |  |
| points/user | [Check Points](https://docs.streamer.bot/api/sub-actions/core/points/user/check-points) | 1.1.0 | Point Definition, User, platform, Required Amount, Break Action of Failure, Destination Variable |  |
| points/user | [Delete Points](https://docs.streamer.bot/api/sub-actions/core/points/user/delete-points) | 1.1.0 | Point Definition, User, platform |  |
| points/user | [Get Points](https://docs.streamer.bot/api/sub-actions/core/points/user/get-points) | 1.1.0 | Point Definition, User, platform, Destination Variable |  |
| points/user | [Get User Multiplier](https://docs.streamer.bot/api/sub-actions/core/points/user/get-user-multiplier) | 1.1.0 | Point Definition, User, platform, Destination Variable |  |
| points/user | [Reset User Points](https://docs.streamer.bot/api/sub-actions/core/points/user/reset-user-points) | 1.1.0 | Point Definition, User, platform, Destination Variable |  |
| points/user | [Set Points](https://docs.streamer.bot/api/sub-actions/core/points/user/set-points) | 1.1.0 | Point Definition, User, platform, Amount, Destination Variable |  |
| points/user | [Spend Points](https://docs.streamer.bot/api/sub-actions/core/points/user/spend-points) | 1.1.0 | Point Definition, User, platform, Amount, Break action sequence if user has insufficient points, Destination Variable |  |
| points/user | [Transfer Points](https://docs.streamer.bot/api/sub-actions/core/points/user/transfer-points) | 1.1.0 | Point Definition, Source User, Source Platform, Target User, Target Platform, Amount, Base Variable Name |  |
| points/user | [Update Points](https://docs.streamer.bot/api/sub-actions/core/points/user/update-points) | 1.1.0 | Point Definition, User, platform, Amount, Apply User/Group Multipliers, Destination Variable |  |
| quotes | [Add Quote](https://docs.streamer.bot/api/sub-actions/core/quotes/add-quote) |  | Variable | success, quoteTime, quoteId, quoteUserId, quoteUser, quotePlatform |
| quotes | [Delete Quote](https://docs.streamer.bot/api/sub-actions/core/quotes/delete-quote) |  | Quote Id | success, quoteId, error |
| quotes | [Get Quote](https://docs.streamer.bot/api/sub-actions/core/quotes/get-quote) |  | Type, Quote Id | quoteTimestamp, quoteTime, quoteId, quoteUserId, quoteUser, quotePlatform |
| quotes | [Get Quote Count](https://docs.streamer.bot/api/sub-actions/core/quotes/get-quote-count) |  |  | quoteCount |
| signals | [Send Local Signal](https://docs.streamer.bot/api/sub-actions/core/signals/send-local-signal) | 0.2.4 | Signal Name, Include Action's Arguments, Arguments, Queue Signal |  |
| signals | [Wait for Local Signal](https://docs.streamer.bot/api/sub-actions/core/signals/wait-for-local-signal) | 0.2.4 | Signal Name, Overwrite Variables, Timeout | signalTimeout, hasResult, signalName |
| sounds | [Play Sound](https://docs.streamer.bot/api/sub-actions/core/sounds/play-sound) |  | Audio Device, Sound file to play, Finish playing before continuing, Volume, Sound Name, Use Filename As Name | soundLength, soundLengthMs |
| sounds | [Play Sound From Folder](https://docs.streamer.bot/api/sub-actions/core/sounds/play-sound-from-folder) |  | Audio Device, Folder containing..., Finish playing before continuing, Recursive, Volume, Sound Name, Use Filename As Name | randomSoundFile, randomSoundFileName, soundLength, soundLengthMs |
| sounds | [Stop All Sound Playback](https://docs.streamer.bot/api/sub-actions/core/sounds/stop-all-sound-playback) |  |  |  |
| sounds | [Stop Sound Playback](https://docs.streamer.bot/api/sub-actions/core/sounds/stop-sound-playback) |  | Sound Name |  |
| system | [Keyboard Press](https://docs.streamer.bot/api/sub-actions/core/system/keyboard-press) |  | Key, Modifiers |  |
| system | [Run a Program](https://docs.streamer.bot/api/sub-actions/core/system/run-a-program) |  | Command, Working Directory, Arguments, Wait for Exit, Environment Variables | outputCount, output# |
| system | [Toast Notification](https://docs.streamer.bot/api/sub-actions/core/system/toast-notification) |  | Toast Id, Title, Text, Attribution, Icon Path |  |
| timers | [Set Timer State](https://docs.streamer.bot/api/sub-actions/core/timers/set-timer-state) |  |  |  |
| triggers | [Set Trigger State](https://docs.streamer.bot/api/sub-actions/core/triggers/set-trigger-state) | 0.2.5 | Action, Triggers, State |  |
| triggers | [Trigger Custom Event](https://docs.streamer.bot/api/sub-actions/core/triggers/custom-event-trigger) | 0.2.5 | Event Name, Use Args |  |
| users | [Clear Users From Group](https://docs.streamer.bot/api/sub-actions/core/users/clear-users-from-group) |  |  |  |
| voice-control | [Set Voice Control Command](https://docs.streamer.bot/api/sub-actions/core/voice-control/set-voice-control-command) |  |  |  |
| voice-control | [Set Voice Control Command State](https://docs.streamer.bot/api/sub-actions/core/voice-control/set-voice-control-command-state) |  |  |  |

#### Twitch (95)

| Group | Sub-action | Since | Key parameters / outputs |
|---|---|---|---|
| ads | [Get Ad Schedule](https://docs.streamer.bot/api/sub-actions/twitch/ads/get-ad-schedule) | 0.2.3 |  → `prerollFree, prerollFreeSeconds, snoozeCount` |
| ads | [Get Pre-roll Free Time](https://docs.streamer.bot/api/sub-actions/twitch/ads/get-pre-roll-free-time) | 0.2.3 |  → `prerollFree, prerollFreeSeconds` |
| ads | [Run Commercial](https://docs.streamer.bot/api/sub-actions/twitch/ads/run-commercial) |  | Duration |
| ads | [Snooze Ad](https://docs.streamer.bot/api/sub-actions/twitch/ads/snooze-ad) | 0.2.3 |  → `snooze.success` |
| channel | [Add Viewer Count](https://docs.streamer.bot/api/sub-actions/twitch/channel/add-viewer-count) |  |  → `viewerCount` |
| channel | [Create Clip](https://docs.streamer.bot/api/sub-actions/twitch/channel/create-clip) |  | Clip Title, Duration → `createClipSuccess, createClipId, createClipCreatedAt, createClipUrl` |
| channel | [Create Stream Marker](https://docs.streamer.bot/api/sub-actions/twitch/channel/create-stream-marker) |  | Description → `streamMarkerSuccess, streamMarkerId, streamMarkerCreatedAt, streamMarkerDescription` |
| channel | [Set Channel Branded Content](https://docs.streamer.bot/api/sub-actions/twitch/channel/set-channel-branded-content) | 0.2.4 | State → `success` |
| channel | [Set Channel Game](https://docs.streamer.bot/api/sub-actions/twitch/channel/set-channel-game) |  | Source, Title → `gameSuccess, gameName, gameId, gameBoxArt` |
| channel | [Set Channel Title](https://docs.streamer.bot/api/sub-actions/twitch/channel/set-channel-title) |  | Title → `titleSuccess` |
| charity | [Get Latest Charity Campaign](https://docs.streamer.bot/api/sub-actions/twitch/charity/get-latest-charity-campaign) |  |  |
| chat | [Clear Chat Messages](https://docs.streamer.bot/api/sub-actions/twitch/chat/clear-chat-messages) | 0.2.3 |  → `clearSuccess` |
| chat | [Get Pinned Chat Message](https://docs.streamer.bot/api/sub-actions/twitch/chat/get-pinned-chat-message) | 1.0.5 |  |
| chat | [Pin Chat Message](https://docs.streamer.bot/api/sub-actions/twitch/chat/pin-chat-message) | 1.0.5 | Message Id, Duration |
| chat | [Reply to Message](https://docs.streamer.bot/api/sub-actions/twitch/chat/reply-to-message) |  | Reply Id, Message, Send using bot account, Fallback to Broadcaster |
| chat | [Send Announcement to Channel](https://docs.streamer.bot/api/sub-actions/twitch/chat/send-announcement-to-channel) |  | Preferred Account, Color, Message |
| chat | [Send Message](https://docs.streamer.bot/api/sub-actions/twitch/chat/send-message-to-channel) |  | Message, Send using bot account, Fallback to Broadcaster |
| chat | [Unpin Chat Message](https://docs.streamer.bot/api/sub-actions/twitch/chat/unpin-chat-message) | 1.0.5 |  |
| chat | [twitch/chat/update-pinned-chat-message-duration.md](https://docs.streamer.bot/api/sub-actions/twitch/chat/update-pinned-chat-message-duration) |  |  |
| followers | [Get Follow Age Info for Target](https://docs.streamer.bot/api/sub-actions/twitch/followers/get-follow-age-info-for-target) |  | Source Type, Variable → `isFollowing, followDate, followAgeLong, followAgeShort` |
| followers | [Get Follower Count](https://docs.streamer.bot/api/sub-actions/twitch/followers/get-follower-count) |  |  |
| followers | [Get Latest Follower](https://docs.streamer.bot/api/sub-actions/twitch/followers/get-latest-follower) |  |  |
| guest-star | [Assign Guest Star Slot](https://docs.streamer.bot/api/sub-actions/twitch/guest-star/assign-guest-star-slot) |  | User Login, Slot |
| guest-star | [Create Guest Star Session](https://docs.streamer.bot/api/sub-actions/twitch/guest-star/create-guest-star-session) |  |  |
| guest-star | [Delete Guest Star Invite](https://docs.streamer.bot/api/sub-actions/twitch/guest-star/delete-guest-star-invite) |  | User Login |
| guest-star | [Delete Guest Star Slot](https://docs.streamer.bot/api/sub-actions/twitch/guest-star/delete-guest-star-slot) |  | User Login, Slot |
| guest-star | [End Guest Star Session](https://docs.streamer.bot/api/sub-actions/twitch/guest-star/end-guest-star-session) |  |  |
| guest-star | [Get Channel Guest Star Settings](https://docs.streamer.bot/api/sub-actions/twitch/guest-star/get-channel-guest-star-settings) |  |  |
| guest-star | [Get Guest Star Invites](https://docs.streamer.bot/api/sub-actions/twitch/guest-star/get-guest-star-invites) |  |  |
| guest-star | [Get Guest Star Session](https://docs.streamer.bot/api/sub-actions/twitch/guest-star/get-guest-star-session) |  |  |
| guest-star | [Send Guest Star Invite](https://docs.streamer.bot/api/sub-actions/twitch/guest-star/send-guest-star-invite) |  | User Login |
| guest-star | [Update Channel Guest Star Settings](https://docs.streamer.bot/api/sub-actions/twitch/guest-star/update-channel-guest-star-settings) |  | Moderator Send Live Enabled, Slot Count, Browser Source Audio Enabled, Group Layout, Regenerate Browser Sources |
| guest-star | [Update Guest Star Slot](https://docs.streamer.bot/api/sub-actions/twitch/guest-star/update-guest-star-slot) |  | Source Slot, Destination Slot |
| guest-star | [Update Guest Star Slot Settings](https://docs.streamer.bot/api/sub-actions/twitch/guest-star/update-guest-star-slot-settings) |  | Slot, Audio Enabled, Video Enabled, Live, Volume |
| moderation | [Add Moderator](https://docs.streamer.bot/api/sub-actions/twitch/moderation/add-moderator) |  | Source Type, Variable |
| moderation | [Add VIP](https://docs.streamer.bot/api/sub-actions/twitch/moderation/add-vip) |  | Source Type, Variable |
| moderation | [Ban User](https://docs.streamer.bot/api/sub-actions/twitch/moderation/ban-user) |  | User Login, Reason → `bannedUser, bannedUserName, bannedUserId, bannedUserType` |
| moderation | [Remove Moderator](https://docs.streamer.bot/api/sub-actions/twitch/moderation/remove-moderator) |  | Source Type, Variable |
| moderation | [Remove VIP](https://docs.streamer.bot/api/sub-actions/twitch/moderation/remove-vip) |  | Source Type, Variable |
| moderation | [Send Shoutout](https://docs.streamer.bot/api/sub-actions/twitch/moderation/send-shoutout) |  | User Login → `shoutoutSuccess` |
| moderation | [Timeout User](https://docs.streamer.bot/api/sub-actions/twitch/moderation/timeout-user) | 0.2.4 | User Login, Duration, Reason → `timedOutUser, timedOutUserName, timedOutUserId, timedOutUserType` |
| moderation | [UnTimeout User](https://docs.streamer.bot/api/sub-actions/twitch/moderation/untimeout-user) | 0.2.3 | User Login → `unTimedOutUser, unTimedOutUserName, unTimedOutUserId, unTimedOutUserType` |
| moderation | [Unban User](https://docs.streamer.bot/api/sub-actions/twitch/moderation/unban-user) |  | User Login → `unbannedUser, unbannedUserName, unbannedUserId, unbannedUserType` |
| moderation | [Warn User](https://docs.streamer.bot/api/sub-actions/twitch/moderation/warn-user) | 0.2.4 | User Login, Reason → `warnedUser, warnedUserName, warnedUserId, warnedUserType` |
| moderation/chat-modes | [Emote Only](https://docs.streamer.bot/api/sub-actions/twitch/moderation/chat-modes/emote-only) |  | State |
| moderation/chat-modes | [Follow Mode](https://docs.streamer.bot/api/sub-actions/twitch/moderation/chat-modes/follow-mode) |  | State, Duration |
| moderation/chat-modes | [Slow Mode](https://docs.streamer.bot/api/sub-actions/twitch/moderation/chat-modes/slow-mode) |  | State, Duration |
| moderation/chat-modes | [Subscriber Only](https://docs.streamer.bot/api/sub-actions/twitch/moderation/chat-modes/subscriber-only) |  | State |
| moderation/shield | [Get Shield Mode Status](https://docs.streamer.bot/api/sub-actions/twitch/moderation/shield/get-shield-mode-status) |  |  |
| moderation/shield | [Update Shield Mode Status](https://docs.streamer.bot/api/sub-actions/twitch/moderation/shield/update-shield-mode-status) |  |  |
| polls | [Archive Active Poll](https://docs.streamer.bot/api/sub-actions/twitch/polls/archive-active-poll) | 0.2.4 |  |
| polls | [Create Poll](https://docs.streamer.bot/api/sub-actions/twitch/polls/create-poll) |  | Title, Choices, Channel Points Per Vote, Duration → `pollId, success` |
| polls | [Get Active Poll](https://docs.streamer.bot/api/sub-actions/twitch/polls/get-active-poll) | 0.2.5 |  → `active, poll.Title, poll.Id, poll.StartedAt` |
| polls | [Terminate Active Poll](https://docs.streamer.bot/api/sub-actions/twitch/polls/terminate-active-poll) |  |  |
| predictions | [Cancel Active Prediction](https://docs.streamer.bot/api/sub-actions/twitch/predictions/cancel-active-prediction) | 0.2.5 |  → `active, prediction.Id, prediction.CreatedAt, prediction.Title` |
| predictions | [Create Prediction](https://docs.streamer.bot/api/sub-actions/twitch/predictions/create-prediction) | 0.2.5 |  → `success, prediction.Id` |
| predictions | [Get Active Prediction](https://docs.streamer.bot/api/sub-actions/twitch/predictions/get-active-prediction) | 0.2.5 |  → `active, prediction.Id, prediction.CreatedAt, prediction.Title` |
| predictions | [Lock Active Prediction](https://docs.streamer.bot/api/sub-actions/twitch/predictions/lock-active-prediction) | 0.2.5 |  → `active, prediction.Id, prediction.CreatedAt, prediction.Title` |
| predictions | [Resolve Last Prediction](https://docs.streamer.bot/api/sub-actions/twitch/predictions/resolve-last-prediction) | 0.2.5 | Winning Index → `active, prediction.Id, prediction.CreatedAt, prediction.Title` |
| predictions | [Resolve Last Prediction by Title](https://docs.streamer.bot/api/sub-actions/twitch/predictions/resolve-last-prediction-by-title) | 0.2.5 | Outcome Title → `active, prediction.Id, prediction.CreatedAt, prediction.Title` |
| raid | [Cancel Raid](https://docs.streamer.bot/api/sub-actions/twitch/raid/cancel-raid) |  |  |
| raid | [Start Raid](https://docs.streamer.bot/api/sub-actions/twitch/raid/start-raid) |  | User Login → `startRaidSuccess` |
| rewards | [Configure Rewards](https://docs.streamer.bot/api/sub-actions/twitch/rewards/configure-rewards) |  | Disable not configured rewards, Move To, Move All To |
| rewards | [Get Reward Info](https://docs.streamer.bot/api/sub-actions/twitch/rewards/get-reward-info) |  | Reward → `rewardId, rewardCost, rewardTitle, rewardBackgroundColor` |
| rewards | [Reset Reward Counter](https://docs.streamer.bot/api/sub-actions/twitch/rewards/reset-reward-counter) |  | Reward |
| rewards | [Reset Reward User Counters](https://docs.streamer.bot/api/sub-actions/twitch/rewards/reset-reward-user-counters) |  | Reward |
| rewards | [Set Background Color](https://docs.streamer.bot/api/sub-actions/twitch/rewards/set-background-color) |  | Reward, Random Color, Color |
| rewards | [Set Cost](https://docs.streamer.bot/api/sub-actions/twitch/rewards/set-cost) |  | Reward, Cost |
| rewards | [Set Enabled State](https://docs.streamer.bot/api/sub-actions/twitch/rewards/set-enabled-state) |  | Reward, State |
| rewards | [Set Global Cooldown](https://docs.streamer.bot/api/sub-actions/twitch/rewards/set-global-cooldown) |  |  |
| rewards | [Set Group Enabled State](https://docs.streamer.bot/api/sub-actions/twitch/rewards/set-group-enabled-state) |  | Group, State |
| rewards | [Set Group Paused State](https://docs.streamer.bot/api/sub-actions/twitch/rewards/set-group-paused-state) |  | Group, State |
| rewards | [Set Max Per Stream](https://docs.streamer.bot/api/sub-actions/twitch/rewards/set-max-per-stream) | 0.2.4 | Reward, Redeems, Additive |
| rewards | [Set Max per User Per Stream](https://docs.streamer.bot/api/sub-actions/twitch/rewards/set-max-per-user-per-stream) | 0.2.4 | Reward, Redeems, Additive |
| rewards | [Set Paused State](https://docs.streamer.bot/api/sub-actions/twitch/rewards/set-paused-state) |  | Reward, State |
| rewards | [Set Prompt](https://docs.streamer.bot/api/sub-actions/twitch/rewards/set-prompt) |  |  |
| rewards | [Set Title](https://docs.streamer.bot/api/sub-actions/twitch/rewards/set-title) |  |  |
| rewards | [Update](https://docs.streamer.bot/api/sub-actions/twitch/rewards/update) |  |  |
| rewards | [Update Redemption Status](https://docs.streamer.bot/api/sub-actions/twitch/rewards/update-redemption-status) |  | Status |
| sub-counter | [Get Twitch Sub Counter Count](https://docs.streamer.bot/api/sub-actions/twitch/sub-counter/get-twitch-sub-counter-count) |  |  → `subCounter.count, subCounter.rollover, subCounter.rolloverCount` |
| sub-counter | [Set Twitch Sub Counter Count](https://docs.streamer.bot/api/sub-actions/twitch/sub-counter/set-twitch-sub-counter-count) |  | Count → `setResult` |
| subscribers | [Get Latest Subscriber](https://docs.streamer.bot/api/sub-actions/twitch/subscribers/get-latest-subscriber) |  |  |
| subscribers | [Get Subscriber Count](https://docs.streamer.bot/api/sub-actions/twitch/subscribers/get-subscriber-count) |  |  |
| tags | [Add Channel Tag](https://docs.streamer.bot/api/sub-actions/twitch/tags/add-channel-tag) |  | Tag |
| tags | [Clear Channel Tags](https://docs.streamer.bot/api/sub-actions/twitch/tags/clear-channel-tags) |  |  |
| tags | [Remove Channel Tag](https://docs.streamer.bot/api/sub-actions/twitch/tags/remove-channel-tag) |  | Tag |
| tags | [Set Channel Tags](https://docs.streamer.bot/api/sub-actions/twitch/tags/set-channel-tags) |  | Tags |
| user | [Add Broadcaster Information](https://docs.streamer.bot/api/sub-actions/twitch/user/add-broadcaster-information) |  |  → `broadcastUser, broadcastUserName, broadcastUserId, broadcastUserType` |
| user | [Add Present User](https://docs.streamer.bot/api/sub-actions/twitch/user/add-present-user) |  | Index → `presentUsersCount, presentUser, presentUserName, presentUserId` |
| user | [Add Random Users](https://docs.streamer.bot/api/sub-actions/twitch/user/add-random-users) |  | Count, Present Only → `randomUser#, randomUserName#, randomUserId#, randomUserType#` |
| user | [Get Team Info For Target](https://docs.streamer.bot/api/sub-actions/twitch/user/get-team-info-for-target) |  |  |
| user | [Get User Info for Target](https://docs.streamer.bot/api/sub-actions/twitch/user/get-user-info-for-target) |  |  → `addTargetResult, targetUser, targetUserName, targetUserId` |
| user-groups | [Add User To Group](https://docs.streamer.bot/api/sub-actions/twitch/user-groups/add-user-to-group) |  | User Login, Group Name → `addResult` |
| user-groups | [Remove User From Group](https://docs.streamer.bot/api/sub-actions/twitch/user-groups/remove-user-from-group) |  | User Login, Group Name → `removeResult` |
| user-groups | [User in Group](https://docs.streamer.bot/api/sub-actions/twitch/user-groups/user-in-group) |  | User Login, Group Name → `inGroup, foundUser, foundUserName, foundUserId` |

#### YouTube (9)

| Group | Sub-action | Since | Key parameters / outputs |
|---|---|---|---|
| — | [Send Message to Channel](https://docs.streamer.bot/api/sub-actions/youtube/send-message-to-channel) |  | Message |
| — | [Set Description](https://docs.streamer.bot/api/sub-actions/youtube/set-description) | 0.2.4 |  |
| — | [Set Title](https://docs.streamer.bot/api/sub-actions/youtube/set-title) | 0.2.4 |  |
| — | [Set Title and Description](https://docs.streamer.bot/api/sub-actions/youtube/set-title-and-description) |  | Title, Description |
| moderation | [Ban User](https://docs.streamer.bot/api/sub-actions/youtube/moderation/ban-user) |  | Broadcast, User Login → `bannedUser, bannedUserName, bannedUserId, bannedUserType` |
| moderation | [Timeout User](https://docs.streamer.bot/api/sub-actions/youtube/moderation/timeout-user) | 0.2.4 | Broadcast, User Login, duration → `bannedUser, bannedUserName, bannedUserId, bannedUserType` |
| user-groups | [Add User to Group](https://docs.streamer.bot/api/sub-actions/youtube/user-groups/add-user-to-group) | 0.2.3 |  |
| user-groups | [Remove User from Group](https://docs.streamer.bot/api/sub-actions/youtube/user-groups/remove-user-from-group) | 0.2.3 |  |
| user-groups | [User in Group](https://docs.streamer.bot/api/sub-actions/youtube/user-groups/user-in-group) | 0.2.3 |  |

#### Kick (14)

| Group | Sub-action | Since | Key parameters / outputs |
|---|---|---|---|
| channel | [Set Channel Category](https://docs.streamer.bot/api/sub-actions/kick/channel/set-channel-category) |  | Source, Title → `category.success, category.id, category.name, category.thumbnail` |
| channel | [Set Channel Title](https://docs.streamer.bot/api/sub-actions/kick/channel/set-channel-title) |  | Title → `titleSuccess` |
| chat | [Reply to Message](https://docs.streamer.bot/api/sub-actions/kick/chat/reply-to-message) |  | Reply Id, Message, Send using bot account, Fallback to Broadcaster |
| chat | [Send Message](https://docs.streamer.bot/api/sub-actions/kick/chat/send-message-to-channel) |  | Message, Send using bot account, Fallback to Broadcaster |
| moderation | [Ban User](https://docs.streamer.bot/api/sub-actions/kick/moderation/ban-user) |  | User Login, Reason → `bannedUser, bannedUserName, bannedUserId, bannedUserType` |
| moderation | [Timeout User](https://docs.streamer.bot/api/sub-actions/kick/moderation/timeout-user) |  | User Login, Duration → `timedOutUser, timedOutUserName, timedOutUserId, timedOutUserType` |
| moderation | [UnTimeout User](https://docs.streamer.bot/api/sub-actions/kick/moderation/untimeout-user) |  | User Login → `unTimedOutUser, unTimedOutUserName, unTimedOutUserId, unTimedOutUserType` |
| moderation | [Unban User](https://docs.streamer.bot/api/sub-actions/kick/moderation/unban-user) |  | User Login → `unbannedUser, unbannedUserName, unbannedUserId, unbannedUserType` |
| user | [Add Broadcaster Information](https://docs.streamer.bot/api/sub-actions/kick/user/add-broadcaster-information) |  |  → `broadcastUser, broadcastUserName, broadcastUserId, broadcastUserType` |
| user | [Add Random Users](https://docs.streamer.bot/api/sub-actions/kick/user/add-random-users) |  | Count, Present Only, Include Groups → `randomUser#, randomUserName#, randomUserId#, randomUserType#` |
| user | [Get User Info for Target Info](https://docs.streamer.bot/api/sub-actions/kick/user/get-user-info-for-target) |  |  → `targetUser, targetUserName, targetUserId, targetDescription` |
| user-groups | [Add User To Group](https://docs.streamer.bot/api/sub-actions/kick/user-groups/add-user-to-group) |  | User Login, Group Name → `addResult` |
| user-groups | [Remove User From Group](https://docs.streamer.bot/api/sub-actions/kick/user-groups/remove-user-from-group) |  | User Login, Group Name → `removeResult` |
| user-groups | [User in Group](https://docs.streamer.bot/api/sub-actions/kick/user-groups/user-in-group) |  | User Login, Group Name → `inGroup, foundUser, foundUserName, foundUserId` |

#### OBS Studio (29)

| Group | Sub-action | Since | Key parameters / outputs |
|---|---|---|---|
| — | [Create Record Chapter](https://docs.streamer.bot/api/sub-actions/obs-studio/create-record-chapter) | 0.2.5 | Connection, Chapter Name → `success` |
| — | [Get Current Scene](https://docs.streamer.bot/api/sub-actions/obs-studio/get-current-scene) |  | Connection → `currentScene` |
| — | [Get Status](https://docs.streamer.bot/api/sub-actions/obs-studio/get-status) |  | Connection → `obs.isConnected, obs.isStreaming, obs.isRecording` |
| — | [OBS Raw](https://docs.streamer.bot/api/sub-actions/obs-studio/raw) |  | ObsConnection |
| — | [Recording](https://docs.streamer.bot/api/sub-actions/obs-studio/recording) |  | Connection, State |
| — | [Set Active Scene](https://docs.streamer.bot/api/sub-actions/obs-studio/set-active-scene) |  | Connection, Scene |
| — | [Streaming](https://docs.streamer.bot/api/sub-actions/obs-studio/streaming) |  | Connection, State |
| — | [Take Screenshot](https://docs.streamer.bot/api/sub-actions/obs-studio/take-screenshot) |  | Connection, Scene, Source, File Path, Quality → `screenshotFile, filedatetime` |
| filters | [Set State of a Random Filter](https://docs.streamer.bot/api/sub-actions/obs-studio/filters/set-state-of-a-random-filter) |  | Connection, Scene, Source, State |
| groups | [Set Random Group Source Visible](https://docs.streamer.bot/api/sub-actions/obs-studio/groups/set-random-group-source-visible) |  | ObsConnection, Scene, Group → `randomSource` |
| replay | [Set Replay Buffer State](https://docs.streamer.bot/api/sub-actions/obs-studio/replay/set-replay-buffer-state) |  | Connection, State |
| scenes | [Get Scene Item Properties](https://docs.streamer.bot/api/sub-actions/obs-studio/scenes/get-scene-item-properties) |  | Connection, Scene, Source → `props.bounds.alignment, props.bounds.type, props.bounds.x, props.bounds.y` |
| scenes | [Hide Scene's Sources](https://docs.streamer.bot/api/sub-actions/obs-studio/scenes/hide-scenes-sources) |  | Connection, Scene |
| scenes | [Set Random Scene Source Visible](https://docs.streamer.bot/api/sub-actions/obs-studio/scenes/set-random-scene-source-visible) |  | ObsConnection, Scene → `randomSource` |
| scenes | [Set Scene Filter State](https://docs.streamer.bot/api/sub-actions/obs-studio/scenes/set-scene-filter-state) |  | Connection, Scene, Filter, State |
| sources | [Flip Source](https://docs.streamer.bot/api/sub-actions/obs-studio/sources/flip-source) |  | Connection, Scene, Source, Mode |
| sources | [Hide Group's Sources](https://docs.streamer.bot/api/sub-actions/obs-studio/sources/hide-groups-sources) |  | Connection, Scene, Group |
| sources | [Hide Source's Filters](https://docs.streamer.bot/api/sub-actions/obs-studio/sources/hide-sources-filters) |  | Connection, Scene, Source |
| sources | [Rotate Source](https://docs.streamer.bot/api/sub-actions/obs-studio/sources/rotate-source) |  | Connection, Scene, Source, Alignment, Rotation |
| sources | [Set Browser Source Url](https://docs.streamer.bot/api/sub-actions/obs-studio/sources/set-browser-source-url) |  | Connection, Scene, Source, Url |
| sources | [Set Color Source Color](https://docs.streamer.bot/api/sub-actions/obs-studio/sources/set-color-source-color) |  | Connection, Scene, Source, Random Color, Color |
| sources | [Set GDI Text](https://docs.streamer.bot/api/sub-actions/obs-studio/sources/set-gdi-text) |  | Connection, Scene, Source, Text |
| sources | [Set Image Source File](https://docs.streamer.bot/api/sub-actions/obs-studio/sources/set-image-source-file) |  | Connection, Scene, Source, Filename |
| sources | [Set Media Source File](https://docs.streamer.bot/api/sub-actions/obs-studio/sources/set-media-source-file) |  | Connection, Scene, Source, Filename |
| sources | [Set Media State](https://docs.streamer.bot/api/sub-actions/obs-studio/sources/set-media-state) |  | Connection, Scene, Source, State |
| sources | [Set Source Audio Track State](https://docs.streamer.bot/api/sub-actions/obs-studio/sources/set-source-audio-track-state) |  | Connection, Scene, Source, Track, State |
| sources | [Set Source Filter State](https://docs.streamer.bot/api/sub-actions/obs-studio/sources/set-source-filter-state) |  | Connection, Scene, Source, Filter, State |
| sources | [Set Source Mute State](https://docs.streamer.bot/api/sub-actions/obs-studio/sources/set-source-mute-state) |  | Connection, Scene, Source, State |
| sources | [Set Source Visibility State](https://docs.streamer.bot/api/sub-actions/obs-studio/sources/set-source-visibility-state) |  | Connection, Scene, Source, State |

#### Streamlabs Desktop (16)

| Group | Sub-action | Since | Key parameters / outputs |
|---|---|---|---|
| — | [Recording](https://docs.streamer.bot/api/sub-actions/streamlabs-desktop/recording) |  | Connection, State |
| — | [Streaming](https://docs.streamer.bot/api/sub-actions/streamlabs-desktop/streaming) |  | Connection, State |
| filters | [Set State of a Random Filter](https://docs.streamer.bot/api/sub-actions/streamlabs-desktop/filters/set-state-of-a-random-filter) |  | Connection, Scene, Source, State |
| groups | [Hide Group's Sources](https://docs.streamer.bot/api/sub-actions/streamlabs-desktop/groups/hide-groups-sources) |  | Connection, Scene, Group |
| groups | [Set Random Group Source Visible](https://docs.streamer.bot/api/sub-actions/streamlabs-desktop/groups/set-random-group-source-visible) |  | Connection, Scene, Group |
| scenes | [Get Current Scene](https://docs.streamer.bot/api/sub-actions/streamlabs-desktop/scenes/get-current-scene) |  | Connection → `currentScene` |
| scenes | [Set Active Scene](https://docs.streamer.bot/api/sub-actions/streamlabs-desktop/scenes/set-active-scene) |  | Connection, Scene |
| scenes | [Set Scene Filter State](https://docs.streamer.bot/api/sub-actions/streamlabs-desktop/scenes/set-scene-filter-state) |  | Connection, Scene, Filter, State |
| sources | [Flip Source](https://docs.streamer.bot/api/sub-actions/streamlabs-desktop/sources/flip-source) |  | Connection, Scene, Source, Mode |
| sources | [Hide Source's Filters](https://docs.streamer.bot/api/sub-actions/streamlabs-desktop/sources/hide-sources-filters) |  | Connection, Scene, Source |
| sources | [Hide Source's Filters](https://docs.streamer.bot/api/sub-actions/streamlabs-desktop/sources/rotate-source) |  | Connection, Scene, Source, Alignment, Rotation |
| sources | [Set Browser Source URL](https://docs.streamer.bot/api/sub-actions/streamlabs-desktop/sources/set-browser-source-url) |  | Connection, Scene, Source, URL |
| sources | [Set GDI Text](https://docs.streamer.bot/api/sub-actions/streamlabs-desktop/sources/set-gdi-text) |  | Connection, Scene, Source, Text |
| sources | [Set Source Filter State](https://docs.streamer.bot/api/sub-actions/streamlabs-desktop/sources/set-source-filter-state) |  | Connection, Scene, Source, State |
| sources | [Set Source Mute State](https://docs.streamer.bot/api/sub-actions/streamlabs-desktop/sources/set-source-mute-state) |  | Connection, Scene, Source, State |
| sources | [Set Source Visibility State](https://docs.streamer.bot/api/sub-actions/streamlabs-desktop/sources/set-source-visibility-state) |  | Connection, Scene, Source, State |

#### Meld Studio (10)

| Group | Sub-action | Since | Key parameters / outputs |
|---|---|---|---|
| — | [Get Current Scene](https://docs.streamer.bot/api/sub-actions/meld-studio/get-current-scene) | 0.2.5 | Connection → `currentScene` |
| — | [Get Status](https://docs.streamer.bot/api/sub-actions/meld-studio/get-status) | 0.2.5 | Connection → `meldStudio.isConnected, meldSutdio.isStreaming, meldStudio.isRecording` |
| — | [Recording](https://docs.streamer.bot/api/sub-actions/meld-studio/recording) | 0.2.5 | Connection, State |
| — | [Show Scene](https://docs.streamer.bot/api/sub-actions/meld-studio/show-scene) | 0.2.5 | Connection, Scene |
| — | [Streaming](https://docs.streamer.bot/api/sub-actions/meld-studio/streaming) | 0.2.5 | Connection, State |
| — | [Take Screenshot](https://docs.streamer.bot/api/sub-actions/meld-studio/take-screenshot) | 0.2.5 | Connection → `screenshotSuccess` |
| effects | [Set Effect Enabled State](https://docs.streamer.bot/api/sub-actions/meld-studio/effects/set-effect-enabled-state) | 0.2.5 | Connection, Scene, Layer, Effect, State |
| layers | [Set Layer Visibility State](https://docs.streamer.bot/api/sub-actions/meld-studio/layers/set-layer-visibility-state) | 0.2.5 | Connection, Scene, Layer, State |
| tracks | [Set Layer Track Muted State](https://docs.streamer.bot/api/sub-actions/meld-studio/tracks/set-layer-track-muted-state) | 0.2.5 | Connection, Scene, Layer, Track, State |
| tracks | [Set Track Muted State](https://docs.streamer.bot/api/sub-actions/meld-studio/tracks/set-track-muted-state) | 0.2.5 | Connection, Layer, State |

#### PolyPop (1)

| Group | Sub-action | Since | Key parameters / outputs |
|---|---|---|---|
| — | [Trigger Alert](https://docs.streamer.bot/api/sub-actions/polypop/Trigger-Alert) |  | Alert Name, Variables |

#### Elgato (90)

| Group | Sub-action | Since | Key parameters / outputs |
|---|---|---|---|
| camera-hub/epoccam | [EpocCam Set AR Lens](https://docs.streamer.bot/api/sub-actions/elgato/camera-hub/epoccam/set-ar-lens) | 0.2.3 | AR Lens |
| camera-hub/epoccam | [EpocCam Set Bitrate](https://docs.streamer.bot/api/sub-actions/elgato/camera-hub/epoccam/set-bitrate) | 0.2.3 | Bitrate |
| camera-hub/epoccam | [EpocCam Set Camera](https://docs.streamer.bot/api/sub-actions/elgato/camera-hub/epoccam/set-camera) | 0.2.3 | Camera |
| camera-hub/epoccam | [EpocCam Set Flash Mode](https://docs.streamer.bot/api/sub-actions/elgato/camera-hub/epoccam/set-flash-mode) | 0.2.3 | Mode |
| camera-hub/prompter | [Prompter Fast Forward Chapter](https://docs.streamer.bot/api/sub-actions/elgato/camera-hub/prompter/fast-forward-chapter) | 0.2.3 |  |
| camera-hub/prompter | [Prompter Get Selected Script](https://docs.streamer.bot/api/sub-actions/elgato/camera-hub/prompter/get-selected-script) | 0.2.3 |  |
| camera-hub/prompter | [Prompter Rewind Chapter](https://docs.streamer.bot/api/sub-actions/elgato/camera-hub/prompter/rewind-chapter) | 0.2.3 |  |
| camera-hub/prompter | [Prompter Set Auto Scroll](https://docs.streamer.bot/api/sub-actions/elgato/camera-hub/prompter/set-auto-scroll-state) | 0.2.3 | Mode |
| camera-hub/prompter | [Prompter Set Brightness](https://docs.streamer.bot/api/sub-actions/elgato/camera-hub/prompter/set-brightness) | 0.2.3 | Brightness, Adjustment |
| camera-hub/prompter | [Prompter Set Crosshair](https://docs.streamer.bot/api/sub-actions/elgato/camera-hub/prompter/set-crosshair) | 0.2.3 | Mode |
| camera-hub/prompter | [Prompter Set Crosshair Image](https://docs.streamer.bot/api/sub-actions/elgato/camera-hub/prompter/set-crosshair-image) | 0.2.3 |  |
| camera-hub/prompter | [Prompter Set Crosshair Image](https://docs.streamer.bot/api/sub-actions/elgato/camera-hub/prompter/set-crosshair-images) | 0.2.3 | Images |
| camera-hub/prompter | [Prompter Set Font](https://docs.streamer.bot/api/sub-actions/elgato/camera-hub/prompter/set-font) | 0.2.3 | Fonts |
| camera-hub/prompter | [Prompter Set Font Size](https://docs.streamer.bot/api/sub-actions/elgato/camera-hub/prompter/set-font-size) | 0.2.3 | Font Size, Adjustment |
| camera-hub/prompter | [Prompter Set Horizontal Margin](https://docs.streamer.bot/api/sub-actions/elgato/camera-hub/prompter/set-horizontal-margin) | 0.2.3 | Margin, Adjustment |
| camera-hub/prompter | [Prompter Set Line Spacing](https://docs.streamer.bot/api/sub-actions/elgato/camera-hub/prompter/set-line-spacing) | 0.2.3 | Line Spacing, Adjustment |
| camera-hub/prompter | [Prompter Set Mode](https://docs.streamer.bot/api/sub-actions/elgato/camera-hub/prompter/set-mode) | 0.2.3 | Mode |
| camera-hub/prompter | [Prompter Set Opacity](https://docs.streamer.bot/api/sub-actions/elgato/camera-hub/prompter/set-opacity) | 0.2.3 | Opacity, Adjustment |
| camera-hub/prompter | [Prompter Set Scripts](https://docs.streamer.bot/api/sub-actions/elgato/camera-hub/prompter/set-scripts) | 0.2.3 |  |
| camera-hub/prompter | [Prompter Set Vertical Margin](https://docs.streamer.bot/api/sub-actions/elgato/camera-hub/prompter/set-vertical-margin) | 0.2.3 | Margin, Adjustment |
| camera-hub/webcam | [Webcam Set Anti Flicker](https://docs.streamer.bot/api/sub-actions/elgato/camera-hub/webcam/set-anti-flicker) | 0.2.3 | Anti-flicker |
| camera-hub/webcam | [Webcam Set Auto White Balance](https://docs.streamer.bot/api/sub-actions/elgato/camera-hub/webcam/set-auto-white-balance) | 0.2.3 | Mode |
| camera-hub/webcam | [Webcam Set Brightness](https://docs.streamer.bot/api/sub-actions/elgato/camera-hub/webcam/set-brightness) | 0.2.3 | Brightness, Adjustment |
| camera-hub/webcam | [Webcam Set Contrast](https://docs.streamer.bot/api/sub-actions/elgato/camera-hub/webcam/set-contrast) | 0.2.3 | Contrast, Adjustment |
| camera-hub/webcam | [Webcam Set Dynamic Range](https://docs.streamer.bot/api/sub-actions/elgato/camera-hub/webcam/set-dynamic-range) | 0.2.4 | Zoom |
| camera-hub/webcam | [Webcam Set Focus](https://docs.streamer.bot/api/sub-actions/elgato/camera-hub/webcam/set-focus) | 0.2.3 | Focus, Adjustment |
| camera-hub/webcam | [Webcam Set Noise Reduction](https://docs.streamer.bot/api/sub-actions/elgato/camera-hub/webcam/set-noise-reduction) | 0.2.3 | Mode |
| camera-hub/webcam | [Webcam Set Pan](https://docs.streamer.bot/api/sub-actions/elgato/camera-hub/webcam/set-pan) | 0.2.3 | Pan, Adjustment |
| camera-hub/webcam | [Webcam Set Saturation](https://docs.streamer.bot/api/sub-actions/elgato/camera-hub/webcam/set-saturation) | 0.2.3 | Saturation, Adjustment |
| camera-hub/webcam | [Webcam Set Sharpness](https://docs.streamer.bot/api/sub-actions/elgato/camera-hub/webcam/set-sharpness) | 0.2.3 | Sharpness, Adjustment |
| camera-hub/webcam | [Webcam Set Tilt](https://docs.streamer.bot/api/sub-actions/elgato/camera-hub/webcam/set-tilt) | 0.2.3 | Tilt, Adjustment |
| camera-hub/webcam | [Webcam Set White Balance](https://docs.streamer.bot/api/sub-actions/elgato/camera-hub/webcam/set-white-balance) | 0.2.3 | White Balance, Adjustment |
| camera-hub/webcam | [Webcam Set White Balance Tint](https://docs.streamer.bot/api/sub-actions/elgato/camera-hub/webcam/set-white-balance-tint) | 0.2.3 | White Balance Tint, Adjustment |
| camera-hub/webcam | [Webcam Set Zoom](https://docs.streamer.bot/api/sub-actions/elgato/camera-hub/webcam/set-zoom) | 0.2.3 | Zoom, Adjustment |
| control-center/brightness | [Set Power State](https://docs.streamer.bot/api/sub-actions/elgato/control-center/brightness/set-brightness) | 1.1.0 | Device, Brightness |
| control-center/power | [Set Power State](https://docs.streamer.bot/api/sub-actions/elgato/control-center/power/set-power-state) | 1.1.0 | Device, State |
| control-center/temperature | [Set Temperature](https://docs.streamer.bot/api/sub-actions/elgato/control-center/temperature/set-temperature) | 1.1.0 | Device, Temperature |
| stream-deck | [Set Background](https://docs.streamer.bot/api/sub-actions/elgato/stream-deck/set-background) |  | Button ID, Image, Color, State |
| stream-deck | [Set State](https://docs.streamer.bot/api/sub-actions/elgato/stream-deck/set-state) |  | Button ID, ElgatoStreamDeckButtonState |
| stream-deck | [Set Title](https://docs.streamer.bot/api/sub-actions/elgato/stream-deck/set-title) |  | Button ID, Title, State |
| stream-deck | [Set Value](https://docs.streamer.bot/api/sub-actions/elgato/stream-deck/set-value) |  | Button ID, Value |
| stream-deck | [Show Alert](https://docs.streamer.bot/api/sub-actions/elgato/stream-deck/show-alert) |  | Button ID |
| stream-deck | [Show OK](https://docs.streamer.bot/api/sub-actions/elgato/stream-deck/show-ok) |  | Button ID |
| stream-deck | [Toggle State](https://docs.streamer.bot/api/sub-actions/elgato/stream-deck/toggle-state) |  | Button ID |
| studio | [Open Storage Location](https://docs.streamer.bot/api/sub-actions/elgato/studio/open-storage-location) | 1.1.0 |  |
| studio | [Take Snapshot](https://docs.streamer.bot/api/sub-actions/elgato/studio/take-snapshot) | 1.1.0 |  |
| studio/preview | [Get Preview Info](https://docs.streamer.bot/api/sub-actions/elgato/studio/preview/get-preview-info) | 1.1.0 |  |
| studio/preview | [Set Preview Fullscreen State](https://docs.streamer.bot/api/sub-actions/elgato/studio/preview/set-preview-fullscreen-state) | 1.1.0 | State |
| studio/preview | [Set Preview Mute State](https://docs.streamer.bot/api/sub-actions/elgato/studio/preview/set-preview-mute-state) | 1.1.0 | State |
| studio/preview | [Set Preview Volume](https://docs.streamer.bot/api/sub-actions/elgato/studio/preview/set-preview-volume) | 1.1.0 | Volume |
| studio/recording | [Get Last Recording Info](https://docs.streamer.bot/api/sub-actions/elgato/studio/recording/get-last-recording-info) | 1.1.0 |  |
| studio/recording | [Get Recording Info](https://docs.streamer.bot/api/sub-actions/elgato/studio/recording/get-recording-info) | 1.1.0 |  |
| studio/recording | [Set Recording State](https://docs.streamer.bot/api/sub-actions/elgato/studio/recording/set-recording-state) | 1.1.0 | State |
| wave-link | [Get Filter State](https://docs.streamer.bot/api/sub-actions/elgato/wave-link/get-filter-state) | 0.2.0 | Input, Filter → `input.identifier, input.name, filter.id, filter.name` |
| wave-link | [Get Input Information](https://docs.streamer.bot/api/sub-actions/elgato/wave-link/get-input-information) | 0.2.0 | Input → `input.identifier, input.name, input.local.muted, input.local.volume` |
| wave-link | [Get Microphone Information](https://docs.streamer.bot/api/sub-actions/elgato/wave-link/get-microphone-information) | 0.2.0 | Microphone |
| wave-link | [Get Output Volumes](https://docs.streamer.bot/api/sub-actions/elgato/wave-link/get-output-volumes) | 0.2.0 |  → `localVolume, streamVolume` |
| wave-link | [Get Selected Output](https://docs.streamer.bot/api/sub-actions/elgato/wave-link/get-selected-output) | 0.2.0 |  → `output.identifier, output.name` |
| wave-link | [Mute Input](https://docs.streamer.bot/api/sub-actions/elgato/wave-link/mute-input) | 0.2.0 | Input, Mixer, State |
| wave-link | [Mute Microphone](https://docs.streamer.bot/api/sub-actions/elgato/wave-link/mute-microphone) | 0.2.0 | Microphone, State |
| wave-link | [Mute Output](https://docs.streamer.bot/api/sub-actions/elgato/wave-link/mute-output) | 0.2.0 | Output, State |
| wave-link | [Set Filter Bypass State](https://docs.streamer.bot/api/sub-actions/elgato/wave-link/set-filter-bypass-state) | 0.2.0 | Input, Mixer, State |
| wave-link | [Set Filter State](https://docs.streamer.bot/api/sub-actions/elgato/wave-link/set-filter-state) | 0.2.0 | Input, Filter, State |
| wave-link | [Set Input Volume](https://docs.streamer.bot/api/sub-actions/elgato/wave-link/set-input-volume) | 0.2.0 | Input, Mixer, Volume, Adjustement |
| wave-link | [Set Microphone Balance](https://docs.streamer.bot/api/sub-actions/elgato/wave-link/set-microphone-balance) | 0.2.0 | Microphone, Balance |
| wave-link | [Set Microphone Gain](https://docs.streamer.bot/api/sub-actions/elgato/wave-link/set-microphone-gain) | 0.2.0 | Microphone, Gain |
| wave-link | [Set Microphone Output Volume](https://docs.streamer.bot/api/sub-actions/elgato/wave-link/set-microphone-output-volume) | 0.2.0 | Microphone, Output Volume, Adjustement |
| wave-link | [Set Output Monitor Device](https://docs.streamer.bot/api/sub-actions/elgato/wave-link/set-output-monitor-device) | 0.2.0 | Device |
| wave-link | [Set Output Volume](https://docs.streamer.bot/api/sub-actions/elgato/wave-link/set-output-volume) | 0.2.0 | Output, Volume, Adjustement |
| wave-link-3/channels | [Add Application to Channel](https://docs.streamer.bot/api/sub-actions/elgato/wave-link-3/channels/add-application-to-channel) | 1.1.0 | Mix |
| wave-link-3/channels | [Adjust Channel Volume](https://docs.streamer.bot/api/sub-actions/elgato/wave-link-3/channels/adjust-channel-volume) | 1.1.0 | Channel, Mix, Volume Delta |
| wave-link-3/channels | [Get Channel Information](https://docs.streamer.bot/api/sub-actions/elgato/wave-link-3/channels/get-channel-information) | 1.1.0 | Channel → `waveLinkChannelId, waveLinkChannelName, waveLinkChannelVolume, waveLinkChannelMuted` |
| wave-link-3/channels | [Mute Channel](https://docs.streamer.bot/api/sub-actions/elgato/wave-link-3/channels/mute-channel) | 1.1.0 | Channel, State |
| wave-link-3/channels | [Set Channel Effect State](https://docs.streamer.bot/api/sub-actions/elgato/wave-link-3/channels/set-channel-effect-state) | 1.1.0 | Channel, Effect, State |
| wave-link-3/channels | [Set Channel Mix Mute](https://docs.streamer.bot/api/sub-actions/elgato/wave-link-3/channels/set-channel-mix-mute) | 1.1.0 | Channel, Mix, State |
| wave-link-3/channels | [Set Channel Volume](https://docs.streamer.bot/api/sub-actions/elgato/wave-link-3/channels/set-channel-volume) | 1.1.0 | Channel, Mix, Volume |
| wave-link-3/inputs | [Get Input Device Information](https://docs.streamer.bot/api/sub-actions/elgato/wave-link-3/inputs/get-input-device-information) | 1.1.0 | Input Device → `waveLinkInputDeviceId, waveLinkInputDeviceName, waveLinkInputDeviceType, waveLinkInputDeviceGain` |
| wave-link-3/inputs | [Mute Input Device](https://docs.streamer.bot/api/sub-actions/elgato/wave-link-3/inputs/mute-input-device) | 1.1.0 | Input Device, State |
| wave-link-3/inputs | [Set Input Clipguard State](https://docs.streamer.bot/api/sub-actions/elgato/wave-link-3/inputs/set-input-clipguard-state) | 1.1.0 | Input Device, State |
| wave-link-3/inputs | [Set Input Gain](https://docs.streamer.bot/api/sub-actions/elgato/wave-link-3/inputs/set-input-gain) | 1.1.0 | Input Device, Gain |
| wave-link-3/inputs | [Set Input Gain Lock State](https://docs.streamer.bot/api/sub-actions/elgato/wave-link-3/inputs/set-input-gain-lock-state) | 1.1.0 | Input Device, State |
| wave-link-3/inputs | [Set Input Low Cut Filter](https://docs.streamer.bot/api/sub-actions/elgato/wave-link-3/inputs/set-input-low-cut-filter) | 1.1.0 | Input Device, State |
| wave-link-3/inputs | [Set Mic/PC Balance](https://docs.streamer.bot/api/sub-actions/elgato/wave-link-3/inputs/set-mic-pc-balance) | 1.1.0 | Input Device, Mic/PC Mix |
| wave-link-3/mixes | [Adjust Mix Volume](https://docs.streamer.bot/api/sub-actions/elgato/wave-link-3/mixes/adjust-mix-volume) | 1.1.0 | Mix, Volume Delta |
| wave-link-3/mixes | [Get Mix Information](https://docs.streamer.bot/api/sub-actions/elgato/wave-link-3/mixes/get-mix-information) | 1.1.0 | Mix → `waveLinkMixId, waveLinkMixName, waveLinkMixVolume, waveLinkMixMuted` |
| wave-link-3/mixes | [Set Mix Volume](https://docs.streamer.bot/api/sub-actions/elgato/wave-link-3/mixes/set-mix-volume) | 1.1.0 | Mix, Volume |
| wave-link-3/outputs | [Add Output Device to Mix](https://docs.streamer.bot/api/sub-actions/elgato/wave-link-3/outputs/add-output-to-device-mix) | 1.1.0 | Output Device, Mix |
| wave-link-3/outputs | [Get Selected Output Device](https://docs.streamer.bot/api/sub-actions/elgato/wave-link-3/outputs/get-selected-output-device) | 1.1.0 |  → `waveLinkOutputDeviceId, waveLinkOutputDeviceName, waveLinkOutputId` |
| wave-link-3/outputs | [Set Monitored Mix](https://docs.streamer.bot/api/sub-actions/elgato/wave-link-3/outputs/set-monitored-mix) | 1.1.0 | Monitored Mix |
| wave-link-3/outputs | [Set Output Volume](https://docs.streamer.bot/api/sub-actions/elgato/wave-link-3/outputs/set-output-volume) | 1.1.0 | Output Device, Volume |

#### Integrations (45)

| Group | Sub-action | Since | Key parameters / outputs |
|---|---|---|---|
| discord | [Basic Webhook](https://docs.streamer.bot/api/sub-actions/integrations/discord/basic-webhook) | 0.1.14 | Webhook Name, Username, Webhook URL, Content, Avatar URL → `messageId` |
| ifttt | [IFTTT Notification (Advanced)](https://docs.streamer.bot/api/sub-actions/integrations/ifttt/notification-advanced) |  |  |
| ifttt | [IFTTT Notification (Basic)](https://docs.streamer.bot/api/sub-actions/integrations/ifttt/notification-basic) |  |  |
| lumia-stream | [Send Command](https://docs.streamer.bot/api/sub-actions/integrations/lumia-stream/send-command) | 0.1.11 | Command |
| lumia-stream | [Set Color](https://docs.streamer.bot/api/sub-actions/integrations/lumia-stream/set-color) | 0.1.11 | Colour, Brightness, Duration, Transition, Skip Queue |
| lumia-stream | [Set to Default](https://docs.streamer.bot/api/sub-actions/integrations/lumia-stream/set-to-default) | 0.1.11 |  |
| mtion | [Get Active Clubhouse](https://docs.streamer.bot/api/sub-actions/integrations/mtion/get-active-clubhouse) | 0.2.5 |  → `mtion.running, mtion.clubhouse.found, mtion.clubhouse.id, mtion.clubhouse.name` |
| mtion | [Sends Trigger](https://docs.streamer.bot/api/sub-actions/integrations/mtion/send-trigger) | 0.2.5 |  → `mtion.running, mtion.trigger.found, mtion.trigger.sent` |
| pronouns | [Add Pronouns for User](https://docs.streamer.bot/api/sub-actions/integrations/pronouns/add-pronouns-for-user) | 0.2.3 |  → `pronounPronoun, pronounPronounLower, pronounReflexive, pronounReflexiveLower` |
| streamfog/backgrounds | [Play Video Background](https://docs.streamer.bot/api/sub-actions/integrations/streamfog/backgrounds/play-video-background) | 1.0.7 |  |
| streamfog/backgrounds | [Play Video Background by Name](https://docs.streamer.bot/api/sub-actions/integrations/streamfog/backgrounds/play-video-background-by-name) | 1.0.7 |  |
| streamfog/lenses | [Activate Lens](https://docs.streamer.bot/api/sub-actions/integrations/streamfog/lenses/activate-lens) | 1.0.7 |  |
| streamfog/lenses | [Activate Lens by Name](https://docs.streamer.bot/api/sub-actions/integrations/streamfog/lenses/activate-lens-by-name) | 1.0.7 |  |
| streamfog/lenses | [Deactivate Lens](https://docs.streamer.bot/api/sub-actions/integrations/streamfog/lenses/deactivate-lens) | 1.0.7 |  |
| streamfog/outfits | [Activate Outfit](https://docs.streamer.bot/api/sub-actions/integrations/streamfog/outfits/activate-outfit) | 1.0.7 |  |
| streamfog/outfits | [Activate Outfit by Name](https://docs.streamer.bot/api/sub-actions/integrations/streamfog/outfits/activate-outfit-by-name) | 1.0.7 |  |
| streamfog/outfits | [Deactivate Outfit](https://docs.streamer.bot/api/sub-actions/integrations/streamfog/outfits/deactivate-outfit) | 1.0.7 |  |
| tits | [Activate Trigger](https://docs.streamer.bot/api/sub-actions/integrations/tits/activate-trigger) | 0.2.3 |  |
| tits | [Activate Trigger by Name](https://docs.streamer.bot/api/sub-actions/integrations/tits/activate-trigger-by-name) | 0.2.3 | Trigger |
| tits | [Throw Item](https://docs.streamer.bot/api/sub-actions/integrations/tits/throw-item) | 0.2.3 | Item, Delay, Amount |
| tits | [Throw Item by Name](https://docs.streamer.bot/api/sub-actions/integrations/tits/throw-item-by-name) | 0.2.3 | Item, Delay, Amount |
| tits | [Throw Items](https://docs.streamer.bot/api/sub-actions/integrations/tits/throw-items) | 0.2.4 | Items, Delay, Amount |
| tits | [Throw Items by Name](https://docs.streamer.bot/api/sub-actions/integrations/tits/throw-items-by-name) | 0.2.4 | Items, Delay, Amount |
| voicemod | [Get Current Voice](https://docs.streamer.bot/api/sub-actions/integrations/voicemod/get-current-voice) |  |  → `currentVoiceId` |
| voicemod | [Play Sound](https://docs.streamer.bot/api/sub-actions/integrations/voicemod/play-sound) | 0.1.11 | Soundboard, Sound |
| voicemod | [Select Random Voice](https://docs.streamer.bot/api/sub-actions/integrations/voicemod/select-random-voice) | 0.1.8 | Mode |
| voicemod | [Select Voice](https://docs.streamer.bot/api/sub-actions/integrations/voicemod/select-voice) | 0.1.8 | Voice |
| voicemod | [Select Voice By ID](https://docs.streamer.bot/api/sub-actions/integrations/voicemod/select-voice-by-id) | 0.1.8 | Voice ID |
| voicemod | [Select Voice By Name](https://docs.streamer.bot/api/sub-actions/integrations/voicemod/select-voice-by-name) | 0.1.8 | Voice Name |
| voicemod | [Set Background Effect State](https://docs.streamer.bot/api/sub-actions/integrations/voicemod/set-background-effect-state) | 0.1.8 | State |
| voicemod | [Set Censor State](https://docs.streamer.bot/api/sub-actions/integrations/voicemod/set-censor-state) | 0.1.8 | State |
| voicemod | [Set Hear My Voice State](https://docs.streamer.bot/api/sub-actions/integrations/voicemod/set-hear-my-voice-state) | 0.1.8 | State |
| voicemod | [Set Mute State](https://docs.streamer.bot/api/sub-actions/integrations/voicemod/set-mute-state) | 0.1.8 | State |
| voicemod | [Set Voice Changer State](https://docs.streamer.bot/api/sub-actions/integrations/voicemod/set-voice-changer-state) | 0.1.8 | State |
| voicemod | [Stop All Sounds](https://docs.streamer.bot/api/sub-actions/integrations/voicemod/stop-all-sounds) | 0.1.11 |  |
| vtube-studio | [Color Tint](https://docs.streamer.bot/api/sub-actions/integrations/vtube-studio/color-tint) |  | Color, Mix With Scene Lighting Color, Random Color, Tint All, Filter Type |
| vtube-studio | [Get Current Model](https://docs.streamer.bot/api/sub-actions/integrations/vtube-studio/get-current-model) |  |  → `model.id, model.name, model.vtsModelName, model.vtsModelIconName` |
| vtube-studio | [Load Model](https://docs.streamer.bot/api/sub-actions/integrations/vtube-studio/load-model) |  | Model → `success` |
| vtube-studio | [Load Model by Name](https://docs.streamer.bot/api/sub-actions/integrations/vtube-studio/load-model-by-name) |  | Model → `success` |
| vtube-studio | [Move Model](https://docs.streamer.bot/api/sub-actions/integrations/vtube-studio/move-model) |  | Time in Seconds, Relative to Model, Position X, Position Y, Rotation → `success` |
| vtube-studio | [Reset All Color Tints](https://docs.streamer.bot/api/sub-actions/integrations/vtube-studio/reset-all-color-tints) |  |  |
| vtube-studio | [Send Raw Request](https://docs.streamer.bot/api/sub-actions/integrations/vtube-studio/send-raw-request) |  | Name, Variable Prefix, Request, JSON → `warnedUser` |
| vtube-studio | [Set Expression State](https://docs.streamer.bot/api/sub-actions/integrations/vtube-studio/set-expression-state) |  | Expression, State, Fade Time → `success` |
| vtube-studio | [Trigger Hotkey](https://docs.streamer.bot/api/sub-actions/integrations/vtube-studio/trigger-hotkey) |  | Model, Hotkey → `success` |
| vtube-studio | [Trigger Hotkey by Name](https://docs.streamer.bot/api/sub-actions/integrations/vtube-studio/trigger-hotkey-by-name) |  | Name → `success` |

#### Speaker.bot (7)

| Group | Sub-action | Since | Key parameters / outputs |
|---|---|---|---|
| — | [Activate VoiceGate Profile](https://docs.streamer.bot/api/sub-actions/speakerbot/activate-voicegate-profile) | 0.2.4 | Profile Name → `success` |
| — | [Clear Pending](https://docs.streamer.bot/api/sub-actions/speakerbot/clear-pending) |  |  |
| — | [Set Enabled State](https://docs.streamer.bot/api/sub-actions/speakerbot/set-enabled-state) | 0.2.3 |  |
| — | [Set Paused State](https://docs.streamer.bot/api/sub-actions/speakerbot/set-paused-state) | 0.2.3 |  |
| — | [Speak](https://docs.streamer.bot/api/sub-actions/speakerbot/speak) |  |  → `success, speechFile, duration` |
| — | [Speak (UDP)](https://docs.streamer.bot/api/sub-actions/speakerbot/speak-udp) |  |  |
| — | [Stop Speaking](https://docs.streamer.bot/api/sub-actions/speakerbot/stop-speaking) |  |  |

#### Streamer.bot (Decks / Remote) (13)

| Group | Sub-action | Since | Key parameters / outputs |
|---|---|---|---|
| decks | [Deck Notify](https://docs.streamer.bot/api/sub-actions/streamerbot/decks/deck-notify) | 1.0.0 | Deck, Title, Description |
| decks | [Item Set Background](https://docs.streamer.bot/api/sub-actions/streamerbot/decks/item-set-background) | 1.0.0 | Deck, Item, Color, URL, File ID |
| decks | [Item Set Icon](https://docs.streamer.bot/api/sub-actions/streamerbot/decks/item-set-icon) | 1.0.0 | Deck, Item, Color, Name, URL |
| decks | [Item Set State](https://docs.streamer.bot/api/sub-actions/streamerbot/decks/item-set-state) | 1.0.0 | Deck, Item, State |
| decks | [Item Set State](https://docs.streamer.bot/api/sub-actions/streamerbot/decks/item-set-title) | 1.0.0 | Deck, Item, Title, Color, State |
| decks | [Item Set Value](https://docs.streamer.bot/api/sub-actions/streamerbot/decks/item-set-value) | 1.0.0 | Deck, Item, Value, State |
| decks | [Item Toggle State](https://docs.streamer.bot/api/sub-actions/streamerbot/decks/item-toggle-state) | 1.0.0 | Deck, Item |
| decks | [Page Next](https://docs.streamer.bot/api/sub-actions/streamerbot/decks/page-next) | 1.0.0 | Deck |
| decks | [Page Previous](https://docs.streamer.bot/api/sub-actions/streamerbot/decks/page-previous) | 1.0.0 | Deck |
| decks | [Page Set](https://docs.streamer.bot/api/sub-actions/streamerbot/decks/page-set) | 1.0.0 | Deck, Page |
| remote | [Send Remote Signal](https://docs.streamer.bot/api/sub-actions/streamerbot/remote/send-remote-signal) | 0.2.4 | Instance, Signal Name, Include Action's Arguments, Arguments |
| remote | [Send Remote Trigger](https://docs.streamer.bot/api/sub-actions/streamerbot/remote/send-remote-trigger) | 0.2.4 | Instance, Event Name, Include Action's Arguments, Arguments |
| remote | [Wait For Remote Signal](https://docs.streamer.bot/api/sub-actions/streamerbot/remote/wait-for-remote-signal) | 0.2.4 | Instance, Signal Name, Overwrite Variables, Timeout → `signalTimeout, hasResult, signalName, instanceId` |

#### StreamFog (standalone category) (6)

| Group | Sub-action | Since | Key parameters / outputs |
|---|---|---|---|
| lenses | [Activate Lens](https://docs.streamer.bot/api/sub-actions/streamfog/lenses/activate-lens) | 1.0.5 | Category, Lens, Duration |
| lenses | [Activate Lens by Name](https://docs.streamer.bot/api/sub-actions/streamfog/lenses/activate-lens-by-name) | 1.0.5 | Lens Name, Duration |
| lenses | [Deactivate Lens](https://docs.streamer.bot/api/sub-actions/streamfog/lenses/deactivate-lens) | 1.0.5 |  |
| outfits | [Activate Outfit](https://docs.streamer.bot/api/sub-actions/streamfog/outfits/activate-outfit) | 1.0.5 |  |
| outfits | [Activate Outfit by Name](https://docs.streamer.bot/api/sub-actions/streamfog/outfits/activate-outfit-by-name) | 1.0.5 | Outfit Name, Duration |
| outfits | [Reset Look](https://docs.streamer.bot/api/sub-actions/streamfog/outfits/reset-look) | 1.0.5 |  |

#### Settings (4)

| Group | Sub-action | Since | Key parameters / outputs |
|---|---|---|---|
| — | [Pick Color](https://docs.streamer.bot/api/sub-actions/settings/pick-color) |  |  |
| — | [Reset Credits](https://docs.streamer.bot/api/sub-actions/settings/reset-credits) |  |  |
| — | [Reset First Words](https://docs.streamer.bot/api/sub-actions/settings/reset-first-words) |  |  |
| — | [Set Voice Control Input](https://docs.streamer.bot/api/sub-actions/settings/set-voice-control-input) |  |  |

---
## 4. Variables system

Source: [Variables guide](https://docs.streamer.bot/guide/core/variables), [C# variables guide](https://docs.streamer.bot/api/csharp/guide/variables).

### 4.1 Kinds of variables
| Kind | Scope / lifetime | Read syntax | Write |
|---|---|---|---|
| **Arguments** (argument stack) | One action run (flows into inline Run Action children and back) | `%name%` in nearly every text field. **Case-sensitive.** Since 1.0.5 names are stricter and **no spaces** are allowed. | Triggers, sub-actions (`Set Argument` and most "get"-style sub-actions), C# `CPH.SetArgument` |
| **Global, persisted** | App-wide, survives restart | `~name~` inline (persisted non-user globals only) or `Get Global Variable` into an arg | `Set Global Variable`, `CPH.SetGlobalVar(name, value, persisted:true)` |
| **Global, non-persisted (temp)** | App-wide, wiped at shutdown | `Get Global Variable` only | Same, with `persisted:false`. `Clear Non-Persisted Globals` sub-action. |
| **User globals** (persisted / temp) | **Per platform user** (Twitch/YouTube/Kick, keyed by user id), for example points or counters | `Get Global Variable` with Source = User (redeemer/target) | Sub-actions, or C# `Set{Twitch,YouTube,Kick}UserVar[ById]`, `Increment…UsersVar…`, `UnsetAllUsersVar`, etc. |
| Generic built-ins | Always present | `%date%` (`yyyy/MM/dd`, formattable `%date:dddd%`), `%time%`, `%longtime%`, `%unixtime%` (0.2.5), `%filedatetime%`, `%lf%` (CRLF), `%actionId%`, `%actionName%`, `%runningActionId%`, `%actionQueuedAt%`, `%queueName%`, `%triggerId/Name/Category%`; `%__source%` (EventType) and `%eventSource%` (EventSource) need C# | read-only |

### 4.2 Formatting and inline functions
- **Format specifiers:** `%var:fmt%` uses .NET standard and custom numeric or date format strings, e.g. `%tipAmount:c2%`, `%time:t%`. The 1.1.0 changelog notes that the `:` formatting tag was fixed and the parser now has a full unit-test suite.
- **`$math(expr)$`:** evaluated with **mXparser**, which gives the full mXparser function set (`ceil`, `round`, trig, etc.). Example: `$math(%x% * 1.25)$`.
- **`$length(text)$`:** string length.
- **`$parse(name)$`** (1.0): double dereference. For example, `$parse(line%i%)$` returns the value of `%line0%`, `%line1%`, and so on. Inside `parse()`, only `%args%` and `~globals~` are expanded.
- **Auto Type:** a toggle on many sub-actions that coerces `"0"` to int/long and `"true"` to bool. Without it, values are strings, so `"15" < "2"` compares as text.
- **Indexed and flattened conventions** (no arrays in the template language):
  - Words and lines become `input0..n`, `line#`, `randomUser#`, `output#`.
  - JSON becomes dotted paths: `json.key`, `props.bounds.x`, `obsRaw.outputBytes`.
  - Regex matches become `match[i]` and named groups.
  - Most strings also get `…Escaped` and `…UrlEncoded` variants, e.g. `rawInputEscaped` and `inputUrlEncoded0`.
- **`CPH.Parse(text, args)`** (1.0.7) exposes the same parser to C#.

### 4.3 Tooling
- **Global Variable Viewer** (toolbar `Global Variables`, `Ctrl+Alt+V`). A live-updating window with tabs for persisted, non-persisted and per-platform user globals (persisted and temp). Context menu: Add, Edit (Name, Value, **Auto Type**), Delete, Copy Name, Delete all. User tabs also have "delete this var for **all** users".
- **Action History, Inspect Variables** (before and after a run) is the primary way to discover which args a trigger supplies.
- **Triggers:** Global Variable Created, Updated (0.2.1) and Deleted (1.0); User Global Variable Updated (0.2.3).
- **WebSocket:** `GetGlobals`, `GetGlobal`, `{Twitch|YouTube|Kick|Trovo}GetUserGlobals` and `…GetUserGlobal`. The Vue package ships a `useGlobalVariable` composable ([client repo](https://github.com/streamerbot/client)).

### 4.4 How variables reach C#
- `args` is a `Dictionary<string, object>` holding the live argument stack. The recommended accessor is `CPH.TryGetArg<T>(name, out T value)` (0.2.3). Direct `args[...]` access is discouraged as unsafe.
- `CPH.SetArgument(name, value)` writes back to the stack, so later sub-actions see it.
- `CPH.GetGlobalVar<T>(name, persisted=true)` returns `null` or default when the variable is missing. `SetGlobalVar` creates it on demand.
- Complex values (List, Dictionary, POCOs) are JSON-serialised by the built-in serialiser. Newtonsoft.Json is bundled for custom serialisation.
- `GetGlobalVarValues(persisted)` returns `List<GlobalVariableValue>` with `{VariableName, Value, LastWrite}`.
- In 1.1.0, C# execution context is **isolated per run**: action args are separate for each run ([v1.1.0](https://docs.streamer.bot/changelogs/v1.1.0)).

---

## 5. Commands system

Source: [Commands guide](https://docs.streamer.bot/guide/core/commands), [Command Triggered](https://docs.streamer.bot/api/triggers/core/commands/command-triggered), export JSON.

| Setting | Export key | Options / notes |
|---|---|---|
| Name, Group, Enabled | `name`, `group`, `enabled` | Group is a free-text folder. Group-level get/set state sub-actions exist. |
| Include | `include` | Whether `Get Commands` lists it (used for `!commands` listings) |
| **Mode** | `mode` (0 = Basic, 1 = Regex) | Regex fills `match[i]` and named-group args. `regexExplicitCapture` flag. |
| **Location** (Basic) | `location` | `Start` (prefix), `Exact`, `Anywhere` (substring). Arg `commandLocation` (1.0.5). |
| **Commands** (aliases) | `command` (newline-separated) | One trigger string per line. **Cooldowns are shared across all aliases** of one command. **No fixed prefix concept:** `!` is just part of the string. The chat UI's `!` autocomplete only lists Start-located commands that begin with `!`. |
| Case Sensitive | `caseSensitive` | default off |
| **Sources** | `sources` (bitmask → array in 1.0) | Twitch Message, Twitch Whisper, Twitch (Re)Subscription Message, YouTube Message, **Kick Message** (1.0). Platform-agnostic: one command serves all platforms. |
| Ignore Bot Account / Ignore Internal Messages | `ignoreBotAccount`, `ignoreInternal` | Turn Internal off to fire commands from the Streamer.bot Chat window loopback |
| Counters | `persistCounter`, `persistUserCounter` | `%counter%` and `%userCounter%` (per user). Optionally persisted. Reset via context menu or C# `CommandReset…`. |
| **Cooldowns** | `globalCooldown`, `userCooldown` (seconds) | **Broadcaster is always exempt.** A `Command Cooldown` trigger fires when a user hits a cooldown. C# can add to, remove, reset or resize cooldowns at runtime (`CommandAddToUserCooldown`, `CommandSetGlobalCooldownDuration`, …). |
| **Permissions** | `grantType` (Allow/Deny), `permittedUsers[]`, `permittedGroups[]` | Allow means only the listed users/groups. Deny means everyone except them. An empty Allow list allows everyone, and an empty Deny list **denies everyone**. **Broadcaster is always exempt.** Groups come from user groups (Settings → Groups). Exports reference role-like group names such as `"Moderators"` **(inferred: built-in role groups alongside custom groups)**. |

- **Trigger variables:** `command`, `commandId`, `commandName`, `commandSource` (twitch/youtube/kick), `commandType` (message/whisper), `msgId`, `rawInput` (message minus the command when Start), `rawInputEscaped`, `rawInputUrlEncoded`, `input#`, `inputEscaped#`, `inputUrlEncoded#`, `counter`, `userCounter`, regex `match.*`, plus platform user and broadcaster common variables.
- **Context menu:** Add, Edit, Delete, Rename/Set Group, Enabled, Reset Counters, Reset User Counters, Add to Export, Copy Command Id. Shortcuts: `Del`, `Ctrl+R`, `Ctrl+C`, `Ctrl+*`, `Ctrl+-`, `Ctrl+O`.
- **Defaults:** 1.0 added default settings for new Actions and Commands.
- **Voice commands** are a separate entity (Voice Control → Commands): phrase, location Exact/Start/Anywhere, per-command confidence override. They run on Windows Speech Recognition, and 1.1.0 adds a Whisper.net engine ([Voice Control](https://docs.streamer.bot/guide/core/voice-control)).
- **Hotkeys** became **triggers** in 1.0 (Key Press with any combination, Mouse Click). MIDI In events are configured per device and used as triggers ([Hotkeys](https://docs.streamer.bot/guide/core/hotkeys), [MIDI](https://docs.streamer.bot/guide/core/midi)).

---
## 6. Execute C# Code and the CPH API

### 6.1 Compilation and runtime
| Aspect | Detail | Source |
|---|---|---|
| Compiler | **Roslyn** in-process. The 1.0.7 zip ships `Microsoft.CodeAnalysis.CSharp.dll` / `Microsoft.CodeAnalysis.dll`, whose version resource reads **5.6.0** (a modern compiler, so recent C# syntax compiles, but the runtime is .NET Framework). The VS Code template uses `LangVersion 13.0`. | zip listing; [VS Code recipe](https://docs.streamer.bot/api/csharp/recipes/visual-studio-code) |
| Target runtime | .NET Framework **4.8.1** (changelog and csproj `net481`). The shipped `Streamer.bot.exe.config` still declares `supportedRuntime sku=".NETFramework,Version=v4.7.2"`. | [v1.0.0](https://docs.streamer.bot/changelogs/v1.0.0) |
| Sandbox | "An isolated **AppDomain** sandbox catches and logs runtime errors originating from C# CPH scripts". Unhandled exceptions on raw threads or `async void` can still crash the host. Static source inspection of user code is mentioned, and Mono.Cecil ships, **(inferred: used to analyse compiled scripts)**. AppDomain unload crash fixes landed in 1.0.5. | [Threading best practices](https://docs.streamer.bot/api/csharp/guide/threading-best-practices) |
| Contract | `public class CPHInline { public bool Execute() { … return true; } }`. In-app it implicitly derives from **`CPHInlineBase`**, which provides `CPH` and `args`. Returning `false` stops the remaining sub-actions. "CPH" stands for "ChannelPointsHandler", the original project name. | [C# intro](https://docs.streamer.bot/api/csharp/guide) |
| Lifecycle | Optional `public void Init()` runs at compile time, including startup precompile. Optional `public void Dispose()` runs on unload or shutdown. | [Lifecycle](https://docs.streamer.bot/api/csharp/guide/lifecycle-methods) |
| Per-sub-action options | Name, Description, **Precompile on Application Start**, **Delayed Start** (later in boot; for UI code), **Save Result to Variable** with Variable Name (captures the bool and continues regardless) | [Execute C# Code](https://docs.streamer.bot/api/sub-actions/core/csharp/execute-csharp-code) |
| Persistence | Source is stored as Base64 (`byteCode`) inside the action, together with `references[]`, i.e. DLL paths such as `C:\Windows\Microsoft.NET\Framework64\v4.0.30319\System.dll` or `.\Newtonsoft.Json.dll` | decoded export (§10) |
| References | A per-sub-action References pane (right-click to add DLLs). **Find Refs** guesses references from `using`s and auto-adds `System.Core.dll` for `System.Linq`. Global compiler settings add common references to every C# sub-action (0.2.x). Custom DLLs are supported (recipe is WIP). | [Debugging](https://docs.streamer.bot/api/csharp/guide/debugging) |
| Editor | **Monaco** inside WebView2 (1.0; light/dark theme; font family and size settings in 1.0.5). Falls back to **AvalonEdit** if WebView2 fails, if `/nowebview2` is set, or if the `Use Monaco Editor` setting is off. Buttons: Format Document, Find Refs, Compile, Save and Compile, OK, Cancel. Tabs: **Compiling Log** and **References**. Basic `CPH.` autocompletion. | [C# intro](https://docs.streamer.bot/api/csharp/guide), [v1.0.5](https://docs.streamer.bot/changelogs/v1.0.5) |
| Shared code | Any `public bool Name()` method with no parameters in a **named** C# sub-action can be called from other actions: the **Execute C# Method** sub-action (picker of named code blocks and methods, with "Run on UI Thread" and save-result) or `CPH.ExecuteMethod("CodeName", "Method")`. State lives in class fields across calls **(inferred: the compiled instance persists until recompile or unload)**. | [Custom methods](https://docs.streamer.bot/api/csharp/guide/custom-methods), [Execute C# Method](https://docs.streamer.bot/api/sub-actions/core/csharp/execute-csharp-method) |
| Right-click extras | Compile, and **Unload** (dispose) a C# sub-action | v0.2.x changelog |
| UI from C# | Allowed: WPF/WinForms dialogs via Run on UI Thread or Delayed Start. Extensions used custom WinForms settings dialogs. | [v1.0.0 details](https://docs.streamer.bot/changelogs/v1.0.0) |
| External IDE | Reference `Streamer.bot.Plugin.Interface.dll`, `Streamer.bot.Common.dll` and `Twitch.Common.dll`. Namespaces: `Streamer.bot.Plugin.Interface`, `.Model`, `.Enums`, `Streamer.bot.Common.Events`. A community VS Code extension (FajitaOfTreason) gives IntelliSense and hover docs. | [VS Code recipe](https://docs.streamer.bot/api/csharp/recipes/visual-studio-code) |
| Import safety | Since 1.0 the Import dialog **warns when an import contains Execute C# Code**. The docs warn against running as admin because "the included C# compiler can also run custom code". | [v1.0.0](https://docs.streamer.bot/changelogs/v1.0.0), [Installation](https://docs.streamer.bot/get-started/installation) |

**Other scripting languages:** none. The only embedded language is C#. Python and Node are reached through `Run a Program` with Wait for Exit capturing `output#`, or through the WebSocket/HTTP/UDP APIs ([Execute scripts example](https://docs.streamer.bot/examples/execute-scripts)). There is no JS or Python runtime in the shipped DLL list.

### 6.2 CPH API shape
The docs site's [C# Methods index](https://docs.streamer.bot/api/csharp/methods) is generated from the interface assembly. It lists **627 methods in 13 categories**: Core 175, Twitch 151, OBS Studio 69, YouTube 36, Integrations 43, Streamlabs Desktop 39, Elgato 35, Kick 28, Trovo 25 (legacy), Streamerbot 11, Meld Studio 9, Settings 5, Speakerbot 1. Signatures are plain synchronous .NET, mostly `bool`/`void`/`T` returns with optional args. Examples:
- `public T GetGlobalVar<T>(string varName, bool persisted = true)`
- `public bool AddPoints(string userName, Platform platform, Guid pointId, long amount, bool useMultipliers = false)`
- `bool RunAction(string actionName, bool runImmediately = true)`

**Core (175), by group** ([source](https://docs.streamer.bot/api/csharp/methods/core)):

| Group | Methods |
|---|---|
| Action Queues | PauseActionQueue, PauseAllActionQueues, ResumeActionQueue, ResumeAllActionQueues; ForceActionById, ForceNextAction, ForceRandomAction, GetPausedActions, GetQueuePendingCount, IsQueuePaused, ShuffleActionsInQueue |
| Actions | ActionExists, DisableAction(ById), EnableAction(ById), GetActions, RunAction, RunActionById |
| Arguments / Parsing | SetArgument, TryGetArg; Parse (1.0.7) |
| Commands | CommandAddToAllUserCooldowns, CommandAddToGlobalCooldown, CommandAddToUserCooldown, CommandGetCounter, CommandGetUserCounter(ById), CommandRemoveAllUserCooldowns, CommandRemoveGlobalCooldown, CommandRemoveUserCooldown, CommandResetAllUserCooldowns, CommandResetAllUserCounters, CommandResetCounter, CommandResetGlobalCooldown, CommandResetUserCooldown, CommandResetUserCounter, CommandResetUsersCounters, CommandSetGlobalCooldownDuration, CommandSetUserCooldownDuration, DisableCommand, EnableCommand, GetCommands |
| C# | ExecuteMethod |
| Events | GetEventType, GetSource |
| Globals | ClearNonPersistedGlobals, ClearNonPersistedUserGlobals, GetGlobalVar, GetGlobalVarValues, GetUserVar*, SetGlobalVar, SetUserVar*, UnsetAllUsersVar, UnsetGlobalVar, UnsetUser, UnsetUserVar (*deprecated in favour of per-platform methods) |
| Logging | LogDebug, LogError, LogInfo, LogVerbose, LogWarn |
| MIDI | MidiSendControlChange(ByName), MidiSendNoteOn(ByName), MidiSendRaw(ByName) |
| Misc | Between, EscapeString, GetVersion, NextDouble, UrlEncode, Wait |
| **Points** (1.1.0) | Definitions: GetPointDefinition(s), ResetPointDefinition. Global: AwardPointsToAll, GetLeaderboard, GetPointsByCutoff. Groups: AwardPointsToGroup, Get/Set/RemoveGroupMultiplier. Ticks: GetPayoutRules, SetPayoutRuleState. Users: AddPoints, DeletePoints, GetPoints, GetPointsAndRank, GetPointsForUsers, GetUserMultiplier, HasPoints, RemovePoints, ResetUserPoints, SetPoints, SpendPoints, TransferPoints, UpdateBulkPoints, UpdatePoints |
| Quotes | AddQuoteFor{Twitch,YouTube,Kick,Trovo}, DeleteQuote, GetQuote, GetQuoteCount |
| Sounds | PlaySound, PlaySoundFromFolder, StopAllSoundPlayback, StopSoundPlayback |
| System | KeyboardPress, ShowToastNotification |
| Timers | Disable/EnableTimer(ById), GetTimerState, SetTimerInterval |
| Triggers | RegisterCustomTrigger, TriggerCodeEvent, TriggerEvent |
| UDP | BroadcastUdp |
| Users / groups | AddGroup, DeleteGroup, GetGroups, GroupExists, AddUserToGroup, AddUserIdToGroup, RemoveUserFromGroup, RemoveUserIdFromGroup, UserInGroup, UserIdInGroup, UsersInGroup, ClearUsersFromGroup (all take a `Platform` since 0.2.3) |
| WebSocket | WebsocketBroadcastJson, WebsocketBroadcastString (to the main WS server's subscribers as `General.Custom`), WebsocketConnect/Disconnect/IsConnected/Send (outgoing clients by index), WebsocketCustomServerBroadcast/Start/Stop/IsListening/CloseSession/CloseAllSessions/GetConnectionByName |

**Other categories (names only, for shape):**
- **Twitch (151):** Ads, Channel Reward (30; enable/pause/update/cost/cooldown/redemption fulfil/cancel/counters, group ops), Channel (title/game/tags/cheermotes), Chat (SendMessage, SendAction, SendWhisper, Announce, Reply, Delete, Pin/Unpin), Cheers, Clips/markers, Globals (per-user vars), Guest Star (12), Moderation (25), Polls, Predictions, Raid, Teams, User (GetUserInfo/Extended, Mods, VIPs, IsUserSubscribed).
- **OBS (69):** Filters, Groups, Media, **Raw** (ObsSendRaw, ObsSendBatchRaw), Recording, Replay, Scenes, Sources, Streaming, connection helpers.
- **Integrations (43):** Discord webhook, Lumia, pronouns, T.I.T.S., VoiceMod, VTube Studio.
- **Streamerbot (11):** Decks (DeckNotify, DeckItemSet*, DeckPage*).
- **Settings (5):** AddToCredits, ResetCredits, ResetFirstWords, …
- **Speakerbot (1):** TtsSpeak.
- Kick (28), YouTube (36), Meld (9), Streamlabs Desktop (39) and Elgato (35) follow the same pattern.
- Supporting **classes:** GlobalVariableValue, UserVariableValue<T>, TwitchUserInfo(Ex), KickUserInfo, YouTubeUserInfo, QuoteData, CommandData, ClipData, TwitchReward, etc. **Enums:** EventType (489), EventSource (46), Platform {Twitch, YouTube, Kick} ([classes/enums](https://docs.streamer.bot/api/csharp)).

---
## 7. Servers and clients

### 7.1 Streamer.bot WebSocket Server (primary API)
Source: [Configuration](https://docs.streamer.bot/api/websocket/guide/configuration), [Requests](https://docs.streamer.bot/api/websocket/requests), [Authentication](https://docs.streamer.bot/api/websocket/guide/authentication), [Events](https://docs.streamer.bot/api/websocket/guide/events).

| Setting | Default | Notes |
|---|---|---|
| Auto Start | true | Servers/Clients → WebSocket Server |
| Address | `127.0.0.1` | Use `0.0.0.0` for LAN. Remote access is recommended via Tailscale Serve (wss) ([Remote access recipe](https://docs.streamer.bot/api/websocket/recipes/remote-access)). |
| Port / Endpoint | `8080` / `/` | |
| Authentication | off | When on, only privileged requests (`SendMessage`) need auth, unless **Enforce** is on (then all requests do). |
| Password | — | |
| Implementation | Kestrel (ASP.NET Core) since 1.0.5, previously websocket-sharp | [v1.0.5](https://docs.streamer.bot/changelogs/v1.0.5) |

- **Handshake.** On connect the server sends `Hello`: `{timestamp, session, request:"Hello", info:{instanceId, name, version, os, osVersion, mode:"ui", darkMode, source:"websocketServer"}, authentication?:{salt, challenge}}`.
- **Auth.** `secret = b64(sha256(password + salt))`, then `auth = b64(sha256(secret + challenge))`, sent as `{"request":"Authenticate","id","authentication"}`. This is the same scheme as obs-websocket v5.
- **Request envelope:** `{"request":"<Name>","id":"<any>", …}`. **Response:** `{"status":"ok"|"error","id", …}`.
- **Event envelope:** `{"timeStamp", "event":{"source","type"}, "data":{…}}`. **No events are sent until you `Subscribe`** with `{"events":{"Twitch":["ChatMessage",…]}}`.

**Requests** (union of the docs and the client's `StreamerbotRequestName` type, [client source](https://github.com/streamerbot/client/blob/main/packages/client/src/ws/types/streamerbot-request.types.ts)):

| Request | Purpose |
|---|---|
| Authenticate | Challenge response |
| Subscribe / UnSubscribe | Event subscriptions by `{source:[types]}` |
| GetEvents | List subscribable events |
| GetInfo | Instance info |
| GetActions | `[{id, name, group, enabled, subaction_count}]` (per client `StreamerbotAction` type) |
| **DoAction** | `{action:{id|name}, args:{}}`. Since 1.0 it returns the queued run ID and initial args. |
| GetBroadcaster | Connected platform accounts |
| GetActiveViewers | Present viewers |
| GetCodeTriggers / ExecuteCodeTrigger | List or fire C#-registered custom triggers with args |
| GetCommands | Command list, including settings (1.0) |
| GetCredits / TestCredits / ClearCredits | Credits system |
| GetGlobals / GetGlobal | Globals (persisted flag) |
| {Twitch, YouTube, Kick, Trovo}GetUserGlobals / …GetUserGlobal | Per-platform user globals |
| TwitchGetEmotes / YouTubeGetEmotes | Emote lists |
| GetMonitoredYouTubeBroadcasts | YouTube broadcasts |
| SendMessage | `{platform:"twitch|kick|youtube", bot, internal, message}` (**auth required**) |
| GetUserPronouns | alejo.io pronouns |

**Events.** The client's constants, compiled from `GetEvents`, list **44 sources / 464 event types** ([events.const.ts](https://github.com/streamerbot/client/blob/main/packages/client/src/ws/types/events/events.const.ts)). The engine-relevant ones:
- `General.Custom`: anything sent by `CPH.WebsocketBroadcastJson/String`. This is the overlay data channel.
- `Raw.Action`, `Raw.SubAction`, `Raw.ActionCompleted`: execution tracing.
- `Application.ActionAdded/Updated/Deleted` and `Application.CommandAdded/Updated/Deleted`: config change notifications.
- `Misc.*`: TimedAction, Test, ProcessStarted/Stopped, ChatWindowAction, StreamerbotStarted/Exiting, ToastActivation, GlobalVariableCreated/Updated/Deleted, UserGlobalVariableUpdated, ApplicationImport.
- `Command.Triggered/Cooldown`, `Custom.Event/CodeEvent`, `Group.*`, `Inputs.*`, `Midi.Message`, `FileWatcher.*`, `FileTail.Changed`, `WebsocketClient.*`, `WebsocketCustomServer.*`, `StreamerBotRemote.*`, `StreamerBot.CustomWebook`, `System.ClipboardChanged`.
- Platform sources: Twitch 136, YouTube 29, Kick 19, Elgato 90, …

### 7.2 Official JS client — `@streamerbot/client`
- Latest **v2.0.1** (2026-07-20, MIT, maintainer whipstickgostop). Works in browser and Node (uses `ws` in Node). CDN builds are on jsDelivr. There is a companion **`@streamerbot/vue` 2.0.2** with `useStreamerbot` and `useGlobalVariable` composables ([npm](https://npm.im/@streamerbot/client), [docs](https://streamerbot.github.io/client/)).
- **Options:** `scheme` (ws/wss), `host`, `port`, `endpoint`, `password`, `immediate`, `autoReconnect`, `retries` (-1 means infinite), `subscribe` (`{…}` or `'*'`), `onConnect(info)`, `onDisconnect`, `onError`, `onData`, `logger`, `logLevel`.
- **Methods:** `connect`, `disconnect`, `request`, `on('Source.Type' | 'Source.*' | '*')` (auto-subscribes), `subscribe`, `unsubscribe`, `getEvents`, `getActions`, `doAction(idOrName, args, {customEventResponse})`, `getBroadcaster`, `getMonitoredYouTubeBroadcasts`, `getCredits/testCredits/clearCredits`, `getInfo`, `getActiveViewers`, `executeCodeTrigger`, `getCodeTriggers`, `getCommands`, `getEmotes`, `getGlobals/getGlobal`, `getUserGlobals/getUserGlobal`, `sendMessage`, `getUserPronouns`.
- **RPC-style replies:** `doAction(..., {customEventResponse:true})` injects an `sbClientResponse` arg. The client then waits for a matching `General.Custom` event broadcast back by the action. This builds request/response on top of fire-and-forget actions ([StreamerbotClient.ts](https://github.com/streamerbot/client/blob/main/packages/client/src/ws/StreamerbotClient.ts)).

### 7.3 HTTP Server
Source: [HTTP config](https://docs.streamer.bot/api/http/guide/configuration), [HTTP requests](https://docs.streamer.bot/api/http/requests/do-action).
- Configured under Servers/Clients → HTTP Server.
- Defaults: Auto Start **off**, Host `127.0.0.1`, Port **7474**. Kestrel since 1.0.5, so `netsh` URL ACLs are no longer needed.
- Endpoints: `POST /DoAction` (`{action:{id|name}, args}`), `GET /GetActions`, `GET /GetCredits`, `GET /TestCredits`, `GET /ClearCredits`, `GET /ClearFirstWordsCache` (204 No Content).
- 1.0 moved these to an **`/api/` prefix**. The un-prefixed paths are deprecated.
- **Static file hosting (1.0):** a "Mappings" list of `Path → Folder` serves local files or folders, e.g. overlays.

### 7.4 UDP Server
- Defaults: Auto Start off, Port **4242**.
- Accepts `{"request":"DoAction","action":{"id"|"name"},"args":{}}`.
- Used by the official VoiceAttack plugin.
- Outbound sends use the `UDP Broadcast` sub-action or `CPH.BroadcastUdp`.
- Sources: [UDP config](https://docs.streamer.bot/api/udp/guide/configuration), [UDP DoAction](https://docs.streamer.bot/api/udp/requests/do-action).

### 7.5 Custom WebSocket Servers and Clients (user-defined)
- **Custom servers:** multiple user-defined WS servers. Triggers: Connection Opened, Closed, Message. Variables: `wssIdx`, `wssId`, `wssName`, `ip`, `sessionId` plus the message data. Setting `IgnoreExtensions` (1.0.5). C#: `WebsocketCustomServer*`. Kestrel since 1.0.5.
- **Clients:** multiple outgoing WS connections. Triggers: Open, Close, Message. Variables: `wsIdx`, `wsId`, `wsName`, `wsUrl`, `wsScheme`, `wsHost`, `wsPort`, `wsPath`, `wsQuery`. Setting `EnableCompression` (1.0.5). C#: `WebsocketConnect`, `WebsocketDisconnect`, `WebsocketIsConnected`, `WebsocketSend`.
- Both are included in import/export (`websocketServers[]`, `websocketClients[]`).
- Sources: [trigger refs](https://docs.streamer.bot/api/triggers/core/websocket/client/websocket-client-message), [variables](https://github.com/Streamerbot/docs/tree/main/streamerbot/3.api/.variables).

### 7.6 Other channels
- **Elgato Stream Deck server** (Integrations → Elgato → Stream Deck). Defaults `127.0.0.1:8059`, auto start; Kestrel since 1.0.5.
  - The official plugin `bot.streamer.streamdeck` provides an Action key (Key-Down, Long-Press ~800 ms, Key-Up), Action Switch, and dials. Every press sends `%sdButtonId%` plus custom args.
  - Sub-actions can set a key's title, background, state or value, or show OK/Alert.
  - Stream Deck **MCP** support arrived in 1.0.5.
  - Sources: [Stream Deck plugin docs](https://github.com/Streamerbot/docs/tree/main/streamdeck-plugin).
- **Streamer.bot Remote (multi-instance).** Instances linked through the same Streamer.bot website account can exchange data, even across networks.
  - Sub-actions: Send Remote Trigger, Send Remote Signal, Wait For Remote Signal (returns `instanceId/Name/Version/Os`).
  - Triggers: Remote Instance Trigger, Connected, Disconnected.
  - **Cloud relay, not peer-to-peer (inferred).** Source: [Remote triggers example](https://docs.streamer.bot/examples/remote-triggers).
- **Custom Webhooks** (1.0, supporter perk). The website receives arbitrary 3rd-party webhooks and forwards them to the instance, which fires the `StreamerBot.CustomWebook` event/trigger ([v1.0.0](https://docs.streamer.bot/changelogs/v1.0.0)).
- **Headless mode.** `/headless` launches without UI and `/shutdown` stops it. 1.0.3 switched headless IPC to **Named Pipes**. Clipboard handling works in headless mode ([Startup switches](https://docs.streamer.bot/guide/advanced/switches), [v1.0.3](https://docs.streamer.bot/changelogs/v1.0.3)).

---
## 8. UI/UX

### 8.1 Main window
The left-hand navigation reconstructed from documented "navigate to" paths. Exact labels may vary slightly; this is **(partly inferred)**.

| Nav section | Pages / content |
|---|---|
| **Actions & Queues** | Actions (3-pane editor), Queues, Pending Actions, Action History |
| **Commands** | Grouped command list (tree with collapsible groups) |
| **Platforms** | Twitch (Accounts broadcaster+bot, Settings, Channel Point Rewards, Polls, Predictions, Pyramids, …), YouTube (Accounts, Settings, Stream Info/broadcasts), Kick |
| **Stream Apps** | OBS (multiple connections, v5), Streamlabs Desktop, Meld Studio, PolyPop |
| **Integrations** | Streamer.bot Website, Streamer.bot Remote, Elgato (Stream Deck, Wave Link, Camera Hub, …), Speaker.bot, VTube Studio, VoiceMod, StreamElements, Streamlabs, Crowd Control, Pulsoid, HypeRate, Lumia, Pally.gg, DonorDrive, StreamLoots, StreamFog, … (sort order and hidden items are persisted, 1.1.0) |
| **Servers/Clients** | WebSocket Server, WebSocket Custom Servers, WebSocket Clients, HTTP Server, UDP Server |
| **Services** | Timers (Timed Actions), Quotes, Credits, File Watchers, File Tails, Process Watcher, plus Points in 1.1.0 **(inferred placement)** |
| **Voice Control** | Commands, Settings (engine, locale, confidence, input device), Log (live dictation) |
| **MIDI** | MIDI In (devices pane, then device events pane with "learn" auto-detect), MIDI Out |
| **Viewers** | Users database with per-user stats (watch time, raid stats, channel points spent, Kicks gifted, super stickers, …) ([v1.0.5](https://docs.streamer.bot/changelogs/v1.0.5)) |
| **Settings** | General, Backups (local + cloud), Groups (user groups), sounds/output device, confirmation and colour preferences, Use Monaco Editor, Minimize to Tray on Close, defaults for new Actions/Commands, … |

**Chrome around the nav:**
- **Hamburger menu:** Check for Updates (release channel picker), Open Log Folder, …
- **Toolbar:** Import, Export, Global Variables, Chat, Events (feed).
- **Title bar:** User Profile dropdown for the Streamer.bot account (1.0).
- **Bottom-left:** website Login.
- **Tray icon:** handled internally in 1.1.0.
- **Toasts:** save complete, export add/remove, backup success/failure.
- **Auto-save:** every 5 minutes, plus `Ctrl+S` ([v1.0.0](https://docs.streamer.bot/changelogs/v1.0.0), [v1.0.1](https://github.com/Streamerbot/docs/blob/main/streamerbot/5.changelogs/.archive/v1.0.1.md)).

### 8.2 Action editor pattern (worth copying)
- **Three panes:** (1) Actions list grouped by folder, (2) **Triggers** pane (top right), (3) **Sub-Actions** pane (bottom right, as a tree). Selecting an action fills both right-hand panes ([Actions guide](https://docs.streamer.bot/guide/core/actions)).
- **Adding things:** everything is added by **right-click** (or `+`). The hierarchical sub-action and trigger menus have **Favorites**, a searchable hierarchy (1.0.5), and an optional legacy menu style.
- **Editing:** each sub-action opens a modal config dialog with a `?` button that opens its docs page (`F1` is global docs). Dialogs that support it have a **Test** button (OBS Set Color, Speaker.bot Speak, OBS Screenshot, …). Inline variable hints appear in text fields.
- **Visual language:**
  - Disabled items are red. The colour is configurable.
  - Comments are coloured.
  - Groups have their own colour.
  - Always-Run triggers are blue.
  - The Trigger Viewer gives a global map of triggers.
- **Productivity:**
  - Copy/paste of actions, triggers and sub-actions between instances through Base64 clipboard strings.
  - Drag and drop.
  - Multi-select delete.
  - Delete confirmations with "don't ask again" (re-enable in Settings).
  - Keyboard map: `Del`, `Ctrl+D/C/V/O/R`, `Ctrl+↑↓/Home/End`, `F5` test trigger.
  - Global shortcuts: `Ctrl+Alt+I/E/C/V/L` for Import, Export, Chat, Globals, Logs ([Shortcuts](https://docs.streamer.bot/guide/advanced/shortcuts)).
- **Debug loop:** Test Trigger, then Action History, then Inspect Variables, plus the Log sub-action and the log folder. While loops have "Break Running".

### 8.3 Theming
WPF rewrite in 1.0 using the **WPF-UI** (Fluent) library, with **Dark Mode** (the most requested feature) and an optional Light Mode. Monaco follows the theme. 1.0.5 condensed margins and list density ([v1.0.0](https://docs.streamer.bot/changelogs/v1.0.0)).

### 8.4 Chat window, docks, overlays (Streamer.bot Chat)
Source: [Chat guide](https://docs.streamer.bot/guide/extra-features/chat).
- **Hosting:** a web app (`chat.streamer.bot`) embedded through WebView2 as the in-app **Chat** and **Events** windows. Being web-based, it can also be used as an **OBS custom browser dock**: `https://chat.streamer.bot/feed/chat` and `/feed/events`. In that mode it talks to the local WS server, and sending needs WS auth.
- **Multi-platform:** Twitch, YouTube and Kick in combined or tabbed view, with per-platform send toggles. `Enter` sends to selected chats, `Shift+Enter` to all, and `Ctrl+Enter` sends as the bot account.
- **Autocomplete:**
  - `/` opens slash commands: `/action` runs any action, `/title` sets the title on all platforms, `/clear`, `/clear/overlay`.
  - `:` opens emotes from Twitch, YouTube, 7TV, FFZ and BTTV.
  - `!` opens Streamer.bot commands.
- **Quick Actions:** buttons that run any action with custom args, in four scopes: Global, User (in the user popup), Message (on hover) and Event.
- **Highlights:** configurable message and event highlights.
- **Settings sync:** one-way from the app to docks.
- **Overlay mode (experimental):** settings are persisted **in the URL**. Custom CSS uses data attributes (`data-platform`, `data-user-id`, `data-event-type`, `data-slot=…`).
- **Internal messages** loop back into commands when "Ignore Internal" is off. The `ChatWindowAction` event exists for this.

### 8.5 Import/Export and community sharing
Source: [Import & Export](https://docs.streamer.bot/guide/core/import-export).
- **Exportable:** Actions (with sub-actions, triggers and C#), Queues, Commands, Timed Actions, WebSocket Clients, WebSocket Servers.
- **Export dialog:** you build the export incrementally with right-click → **Add to Export** from anywhere. The dialog can float or close while building. Fields: Name, Author, Version (SemVer), Description. Output via Export to Clipboard or Export to File.
- **Import dialog:** paste a string or drag-drop a file. It shows a tree of items with per-item **Include in Import** and **Overwrite** toggles. The default is to overwrite items with the same name. It warns if the import contains C#. OBS references are re-matched by name.
- **String format:** see §10.
- **Community:** the **Streamer.bot Extensions** site (<https://extensions.streamer.bot>) hosts community imports such as TerrierDart's leaderboard. Feature requests go to <https://ideas.streamer.bot>. The Discord is the hub.

### 8.6 Other user-facing features and companion tools
| Feature | Summary | Source |
|---|---|---|
| **Users database / groups** | Viewers tab with per-platform user records and stats. Custom **user groups** (Settings → Groups) with platform-aware membership. Group triggers (Added, Removed, Cleared, Users Added/Removed). Used by command permissions and random-user picks. First Words tracking. Present viewers. Pronouns (alejo.io). | [Commands](https://docs.streamer.bot/guide/core/commands), [Groups triggers](https://docs.streamer.bot/api/triggers/core/groups/added) |
| **Points / currency (1.1.0 alpha)** | Integrated Points Service: multiple **point definitions** (Guid), per-platform user balances, award/spend/transfer/set/reset, **user and group multipliers**, **payout rules ("ticks")** with an active-users-only option, leaderboards with rank and cutoff. 18 sub-actions and about 26 C# methods. Before 1.1 this was done with user globals or the community "Leaderboard Points System" extension. | [v1.1.0](https://docs.streamer.bot/changelogs/v1.1.0), [points sub-actions](https://docs.streamer.bot/api/sub-actions/core/points/user/award-points) |
| **Counters** | Per-command `counter` and `userCounter` (optionally persisted). Twitch reward counters and per-user reward counters. Timer counters. Everything else uses global or user variables. | [Commands](https://docs.streamer.bot/guide/core/commands) |
| **Quotes** | Services → Quotes list. Since 1.0, sub-actions replace the built-in `!quote`: Add (captures user, platform, game, time), Get (random or by id), Delete, Count. Triggers: Quote Added/Show. | [Quotes example](https://docs.streamer.bot/examples/quotes-commands) |
| **Credits** | Services → Credits. Choose events to record (follows, cheers, subs, gifts, raids, redemptions, goals, game changes, pyramids, hype trains), users present by role or group, top cheer and channel-points users. Auto-reset after N hours. Exposed via WS/HTTP `GetCredits` for an end-credits overlay. | [Credits](https://docs.streamer.bot/guide/core/credits) |
| **Timers (Timed Actions)** | Name, Enabled, Repeat, Interval (s), Random interval (upper bound), **Lines** (minimum chat lines; single-platform only). Used as a trigger. 1.1.0 adds **Cron** triggers. | [Timers](https://docs.streamer.bot/guide/core/timers) |
| **Watchers** | File/Folder Watcher triggers, File Tail (new lines), Process Started/Stopped | [core triggers](https://docs.streamer.bot/api/triggers/core) |
| **Decks 2.0** | Web-based Stream Deck/Loupedeck alternative served through the website. Items and pages are controllable from sub-actions/C#. Free with limits; supporters get higher limits. | [v1.0.0](https://docs.streamer.bot/changelogs/v1.0.0) |
| **Streamer.bot Toolkit** | Open-source Vue/Vuetify demo app in the client repo: connect form, actions viewer and run, action history drawer, active viewers, chat panel, broadcast, variables, logs. A reference "remote control panel" built on the WS API. | [apps/toolkit](https://github.com/streamerbot/client/tree/main/apps/toolkit) |
| **HTML Overlay** | Separate desktop app (by Ruirize) that renders a transparent browser overlay **on the streamer's own screen**, driven by `window.client`. Modules: Splat, Show Image, Emote Rain. | [HTML Overlay](https://docs.streamer.bot/guide/extra-features/html-overlay) |
| **Stream Deck plugin** | `bot.streamer.streamdeck`: Action, Action Switch and dials, multi-instance, per-key args | [plugin docs](https://github.com/Streamerbot/docs/tree/main/streamdeck-plugin) |
| **Speaker.bot** | Sister TTS app by the same developer, with separate docs. Integrates via WS/UDP sub-actions (Speak, Speak (UDP), VoiceGate profile, pause/enable, clear). | [Speaker.bot docs](https://github.com/Streamerbot/docs/tree/main/speakerbot) |

---
## 9. Settings, data, operations, licensing, tech stack

### 9.1 Storage on disk
- **Install layout.** Streamer.bot is **portable**: you unzip it anywhere and run `Streamer.bot.exe`. There is no installer and no registry dependency. Synced folders such as OneDrive are discouraged ([Installation](https://docs.streamer.bot/get-started/installation)). Configuration lives in a `data/` folder next to the exe. Logs go in a `logs` folder and backups in a `backups` folder, whose location is configurable since 1.0.
- **Files.** The community wiki lists: `actions.json`, `commands.json`, `settings.json`, `midi.json`, `twitch_rewards.json`, `users.dat`, `quotes.dat`, `auth.db` (platform tokens), `globals.db`, `firstwords.cache`, `credits.cache` ([wiki backup page](https://wiki.streamer.bot/en/Backup); possibly dated).
  - The 1.0.7 zip ships **LiteDB 5.0.21**. **(Inferred:** the `.db` files are LiteDB, an embedded document database; JSON is used for config.)
  - Twitch, YouTube and Kick each have a separate "data repo/file" (1.0.5 mentions Power-Up tables and a YouTube data repo).
  - 1.0.5 introduced "new save routines" to prevent corruption from file locks ([v1.0.5](https://docs.streamer.bot/changelogs/v1.0.5)).
- **Secrets.** `auth.db` holds tokens and is optional in cloud backups. Cloud backups are **encrypted** ([Backup guide](https://docs.streamer.bot/guide/core/backup), [v1.0.0](https://docs.streamer.bot/changelogs/v1.0.0)).

### 9.2 Backup, update, logging
- **Local backups.** A zip of `data/` is made automatically on every start and before every built-in update. The filename is `[date]-[time]-[version]` (version added in 1.0.1). Manual backups are available. Restore by copying the zip contents into `data/`. Rolling back the app version requires a data backup from that same version.
- **Cloud backups** (supporter perk) live under Settings → Backups. They are manual, with a description and optional `auth.db`, and restore through the right-click menu. Backups from other instances are visible.
- **Auto-update.** The built-in updater has **Stable / Beta / Alpha** channels. Non-stable channels require the website account integration. The release API is `https://streamer.bot/api/releases/streamer.bot[/{ver}/download]`, which returns JSON with SHA-256, filesize, a markdown description and a `force` flag.
- **Logging.** Serilog, with file and async sinks. Files roll over at a size limit (1.0.5). Log levels are Verbose, Debug, Info, Warn and Error (Log sub-action and `CPH.Log*`). Voice dictation logging is optional. **Sentry** (with a Serilog sink) is bundled, **(inferred:** crash and error telemetry).
- **Startup switches:** `/headless`, `/shutdown`, `/nologin`, `/nowebview2`, `/resetwindow` ([Switches](https://docs.streamer.bot/guide/advanced/switches)).
- **Multi-instance.** Several instances can run, each a separate portable folder **(inferred)**. They interconnect via Remote triggers and signals and via cloud backups' "Other Instances" tab.

### 9.3 Licensing and business model
- **Closed source and free.** The stated goal is that "at its core… it will always be free to use" ([v1.0.0](https://docs.streamer.bot/changelogs/v1.0.0)). Only the docs, the JS/Vue client, the Linux installer and the Stream Deck/VoiceAttack plugins are public on GitHub.
- **Supporter perks** need Patreon (`nate1280`) at the lowest **"Thank You"** tier. It is $3/month after 1.0, previously $2, and an annual plan was offered at the old price. Perks:
  - **Cloud Backups**
  - **Custom Webhooks**
  - **higher Decks limits**
- **Early access:** Alpha needs **Bronze tier or higher**. Beta is free with the Discord beta role.
- **Kick** was originally planned as a perk but made free.

### 9.4 Tech stack (from the shipped 1.0.7 file list)
| Area | Library / tech |
|---|---|
| Runtime / UI | C#, **.NET Framework 4.8.x**, **WPF** (WinForms before 1.0), **WPF-UI 4.3.0** (1.1.0 alpha downgrades to 4.2.1 for UIAutomation crashes), CommunityToolkit.Mvvm, GongSolutions.WPF.DragDrop, Microsoft.Xaml.Behaviors, Markdig(.Wpf) for release notes |
| Embedded web | **Microsoft WebView2** for the Chat and Events windows, the Monaco editor and release notes. Plus **AvalonEdit** (JSON editors for OBS Raw and VTS Raw, and the C# fallback). |
| Scripting | **Roslyn** (Microsoft.CodeAnalysis.CSharp 5.6.0), Mono.Cecil, Newtonsoft.Json + System.Text.Json (migration started in 1.0.5) |
| Expressions | **mXparser** (`$math()$`) |
| Servers | **Kestrel / ASP.NET Core 2.3** on netfx (HTTP, WebSocket and Stream Deck servers, 1.0.5). **websocket-sharp** for clients and legacy servers. |
| Storage | **LiteDB 5** plus JSON files |
| Audio / MIDI / input | **NAudio** (WASAPI, ASIO, Vorbis, MIDI), **SharpHook/uiohook** for global keyboard and mouse hooks. **SDL2** controller input and **Whisper.net** STT arrive in 1.1.0. Microsoft Speech (`Speech.dll`) for voice control. |
| Platform SDKs | `Twitch.EventSub`/`Twitch.ClientApi` (custom), `ObsWebsocket.dll`, **gRPC + Protobuf** (`Streamer.bot.YouTube.Grpc`, likely the YouTube streaming chat API), per-integration assemblies (`Streamer.bot.Kick.dll`, `.Elgato.dll`, `.VTubeStudio.dll`, …). This is a **modular, assembly-per-integration** layout. |
| OS integration | Microsoft.Toolkit.Uwp.Notifications (toasts), WindowsAPICodePack, Named Pipes (headless IPC) |
| Observability | Serilog, Sentry |

### 9.5 Platforms supported
- **Windows:** the only official target (x64 build).
- **Linux:** "experimental, not officially supported". It runs under Wine ≥ 11.9 with winetricks `dotnet48`, `d3dcompiler_47` (required), `dxvk` and `corefonts` (recommended), and `sapi` (for Speaker.bot). Install options are the official `sb-linux-installer` script, Bottles or Lutris. 1.0.5 and later may need a `UIAutomationCore.dll` workaround, and `/nowebview2` is suggested ([Linux install](https://docs.streamer.bot/get-started/installation/linux)).
- **macOS:** experimental via Whisky, now deprecated because Whisky is in maintenance ([macOS](https://docs.streamer.bot/get-started/installation/macos)).
- **Takeaway for us:** cross-platform support is a real gap and a differentiator.

---

## 10. Data model reference — export/share string

Decoded from import strings in the docs repo. These are format observations, not official specs.

```
share string = Base64( "SBAE" + GZip( UTF-8 JSON ) )        // first 4 bytes are the ASCII magic "SBAE"
{
  "meta": { "name", "author", "version", "description", "autoRunAction": null|<actionId?>, "minimumVersion": null|"x.y.z" },
  "version": 23,                       // export schema version
  "exportedFrom": "1.0.4",             // app version
  "minimumVersion": "1.0.0-alpha.1",
  "data": {
    "actions": [ { "id","name","group","enabled","queue","alwaysRun","randomAction","concurrent",
                   "excludeFromHistory","excludeFromPending","collapsedGroups":[],
                   "triggers": [ { "id","type":401,"enabled":true,"exclusions":[], "commandId": "…" } ],
                   "subActions": [ { "id","type":120,"parentId":null,"enabled":true,"index":0,"weight":0.0,
                                     "input":"%rawInput%","operation":9,"value":null,"autoType":true,
                                     "subActions":[ {"type":99901,"random":false,"subActions":[…]}, {"type":99902,…} ] } ] } ],
    "queues": [], "commands": [ { "id","name","enabled","include","mode","command","regexExplicitCapture","location",
                                   "ignoreBotAccount","ignoreInternal","sources":[0],"persistCounter","persistUserCounter",
                                   "caseSensitive","globalCooldown","userCooldown","group","grantType",
                                   "permittedUsers":[],"permittedGroups":["Moderators"] } ],
    "timers": [ { "id","name","enabled","repeat","interval","randomInterval","upperInterval","lines","counter" } ],
    "websocketServers": [], "websocketClients": []
  }
}
```

- Pre-1.0 exports use `actions[].actions[]` (a flat list with a `group` string per sub-action) rather than `subActions` trees. 1.0 upgrades legacy configs on import. For example, Global Set's removed `Argument` source becomes `Value` with `%…%`.
- `autoRunAction` in meta suggests an action that can run right after import, e.g. an installer or setup action **(inferred)**.
- Triggers and sub-actions copied to the clipboard are standalone Base64 blobs of the same family.

---
## 11. What Streamer.live should replicate, and what to improve

### 11.1 Must-replicate engine semantics
1. **The action, trigger, sub-action and queue model**, with the same knobs:
   - Actions: enabled, random with weights, concurrent, always-run (bypasses pause), exclude-from-history.
   - Queues: default non-blocking plus named blocking/pausable queues.
   - History with argument snapshots taken before and after each run, and **requeue**.
2. **Tree-structured sub-actions:**
   - Nestable groups that can be coloured, randomised and weighted.
   - If/Else containers, Switch, While, Loop Break/Continue, Break.
   - Run Action inline (variables merge back) or queued.
   - Local and remote signals.
3. **Argument-stack templating:**
   - `%var%` with format specifiers, `~persistedGlobal~`, and `$math/length/parse$`.
   - Auto-typing.
   - Flattening of arrays and JSON to indexed or dotted args.
   - Escaped and URL-encoded variants.
   - Keep these as an *import-compatibility* parser even if we add a nicer expression language.
4. **Globals:**
   - Persisted and temp, global and per-platform-user, with change triggers.
   - A live viewer/editor.
5. **Commands:**
   - Platform-agnostic, multi-alias, Start/Exact/Anywhere/Regex, case option.
   - Source filters.
   - Global and user cooldowns, with the broadcaster exempt.
   - Allow/Deny permissions over users and groups.
   - Persisted counters and a cooldown trigger.
6. **Trigger precedence:**
   - Range-criteria specificity with the "Always Run" override.
   - Per-trigger enable.
   - Test triggers that carry fixture args.
   - Code-registered custom triggers.
7. **API parity:**
   - A WS server speaking the same request and event names: Hello, Authenticate with the obs-websocket-style challenge, Subscribe, DoAction, GetActions, GetGlobals, ExecuteCodeTrigger, `General.Custom`, `Raw.*`.
   - This would let the existing `@streamerbot/client`, Streamer.bot Chat docks, the Toolkit and overlays work against our app unchanged. It is a big ecosystem win; see Q1.
   - Also HTTP `/api/DoAction` and UDP DoAction.
8. **Import compatibility.** The `SBAE`+gzip+JSON share string and numeric type IDs mean we could import community extensions. This needs a type-ID mapping table and a C# story.

### 11.2 UI patterns worth copying
- The 3-pane action editor (actions | triggers | sub-actions), right-click-everything, favourites, and hierarchical add menus with search.
- Per-dialog `?` docs links and Test buttons in dialogs.
- Colour coding: disabled red, always-run blue, coloured comments and groups.
- Copy/paste of any node as a portable string. Drag-drop into groups.
- The incremental "Add to Export" basket with a floating export dialog.
- Import preview with per-item include/overwrite.
- Live Global Variable viewer. Action History with "inspect variables".
- Chat window: multi-platform, send-target toggles, `/action` and `!command` autocomplete, quick actions per message/user/event, and dockable in OBS as a web page.

### 11.3 Where Streamer.bot is weak (opportunities)
- Windows-only. Linux and macOS run through Wine with fragile WebView2 and UIAutomation.
- C#-only scripting, with no JS or Python. Scripts can crash the host on background threads.
- The template language is string-typed and has no arrays.
- The Fetch URL sub-action is GET-only. There is no WebSocket-send sub-action.
- Docs lag the app. Cloud relay is required for multi-instance.
- A stringly-typed export format with magic numeric IDs.
- Points/currency only arrived in 1.1.0 alpha.

---

## 12. Surprises and notable findings
- **Roslyn 5.6** ships in a **.NET Framework** app, so modern C# syntax runs on a legacy runtime. Scripts run in an AppDomain "sandbox" that is really an error barrier, not a security boundary.
- The HTTP and WebSocket servers moved to **Kestrel** on .NET Framework in 1.0.5.
- **Remote instances and custom webhooks are relayed through the Streamer.bot cloud account.** Some flows need you to be logged in. Kick needs the website link.
- The Chat and Events windows are the **same web app** that runs as an OBS dock (chat.streamer.bot), hosted in WebView2.
- `Raw.Action`/`Raw.SubAction` WS events give external tools a full execution trace.
- The 1.1.0 alpha adds a native **Points economy**, **Cron**, **game controller** input and **Whisper** STT.
- The C# API has 627 methods. Nearly every UI capability is mirrored as a `CPH.*` method, and many sub-actions list their backing `csharpMethods` in the docs frontmatter.

---

## 13. Open questions for the project owner
1. **API compatibility:** should Streamer.live's WebSocket API be wire-compatible with Streamer.bot's (Hello/Auth/Subscribe/DoAction/event names)? That would let existing overlays, docks and the `@streamerbot/client` ecosystem work out of the box.
2. **Import compatibility:** should we import Streamer.bot share strings (`SBAE…`)? If so, what happens to `Execute C# Code` sub-actions? The options are to skip them, port them to JS/Python, or embed .NET (Mono/.NET 8 + Roslyn) as an optional runtime.
3. **Scripting runtimes:** JS and Python are non-negotiable. Should they run in-process (e.g. embedded V8/QuickJS and CPython) or as sandboxed sidecar processes? And should a C#-compatible `CPH` facade (same method names) be offered to ease porting?
4. **Template language:** keep `%var%` / `~global~` / `$fn()$` syntax 1:1 for familiarity and imports, or introduce a typed expression language with compatibility mode?
5. **Cloud features:** do we want an equivalent of the website relay (remote instances, custom webhooks, cloud backups, Decks), or do we stay strictly local/P2P? The owner said "no mystery cloud routing."
6. **Monetisation:** should we mirror the free-core-plus-supporter-perks model, or be fully open source?
7. **Points/currency:** design it natively from day 1 (Streamer.bot only added it in 1.1.0), including per-platform identity merging?
8. **Storage:** use a single embedded DB (SQLite) instead of a JSON + LiteDB mix, with human-readable export?
9. **Chat/overlay UI:** build our multichat as a web app usable both in-app and as an OBS dock, like Streamer.bot Chat? This fits the WYSIWYG designer requirement.

---

### Appendix — source index
- Docs site: <https://docs.streamer.bot> (guide/core/{actions, triggers, variables, commands, hotkeys, voice-control, midi, import-export, backup, credits, quotes, timers}, guide/extra-features/{chat, html-overlay}, guide/advanced/{shortcuts, switches}, api/{sub-actions, triggers, csharp, websocket, http, udp}, changelogs/{v1.0.0, v1.0.3, v1.0.4, v1.0.5, v1.1.0}, faq)
- Docs source: <https://github.com/Streamerbot/docs> (commit 56253da, 2026-10-14)
- JS client: <https://github.com/streamerbot/client>, <https://streamerbot.github.io/client/>, npm `@streamerbot/client@2.0.1`, `@streamerbot/vue@2.0.2`
- Release API: `https://streamer.bot/api/releases/streamer.bot` (stable 1.0.7, published 2026-08-06)
- Shipped artifact: `Streamer.bot-x64-1.0.7.zip` (SHA-256 `47a4e56f7a8a3e5f09ec16a5ba14c50e034dd98d67d2dc1ec89a671994c1f5c5`). Only the file listing and DLL version resources were read.
- Wiki (older): <https://wiki.streamer.bot/en/Backup>
