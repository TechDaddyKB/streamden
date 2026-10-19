//! Engine records (Spec 02 §6.5, A3): the persisted documents and history records the engine owns.
//! Spec 03 (§3, §7, §8, §9, §12) defines their behaviour; the shapes live here so clients get generated
//! types. `Command` and `Timer` are configuration documents (tier-2 writes, draft-first under MCP);
//! `Run` is read-only on the wire.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::auth::{ChainOrigin, Surface};
use crate::domain::Origin;
use crate::events::Event;
use crate::ids::{
    ActionId, CommandId, EventId, EventType, GroupId, NodeId, PlatformId, QueueId, RunId, TimerId,
    TriggerId,
};
use crate::rpc::ErrorCode;
use crate::timestamp::Timestamp;

/// A chat command (spec 03 §7). Persisted document, kind `command`; configuration writes are tier 2.
/// `revision` and `origin` are server-assigned like `Action`'s.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct Command {
    /// The command ID.
    pub id: CommandId,
    /// Server-assigned, +1 on every write. Read-only.
    #[serde(default)]
    #[schemars(extend("readOnly" = true))]
    pub revision: u64,
    /// Display name.
    pub name: String,
    /// Folder path; organisation only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub group: Option<String>,
    /// Disabled commands never trigger.
    pub enabled: bool,
    /// Shown by `commands.list_public`.
    #[serde(default)]
    pub listed: bool,
    /// The texts that invoke it; at least one.
    #[schemars(length(min = 1))]
    pub aliases: Vec<String>,
    /// How an alias matches the message.
    pub mode: CommandMode,
    /// Match aliases case-sensitively.
    #[serde(default)]
    pub case_sensitive: bool,
    /// Event types it listens to (`chat.message`, `chat.whisper`, `chat.sub_message`…, open); empty means
    /// `chat.message`.
    #[serde(default)]
    pub sources: Vec<EventType>,
    /// Platforms it listens on; empty means all.
    #[serde(default)]
    pub platforms: Vec<PlatformId>,
    /// Ignore messages the bot account sends.
    #[serde(default)]
    pub ignore_bot_account: bool,
    /// Ignore messages the engine itself sent.
    #[serde(default)]
    pub ignore_internal: bool,
    /// Cooldowns.
    #[serde(default)]
    pub cooldown: Cooldown,
    /// Who may use it.
    #[serde(default)]
    pub permissions: Permissions,
    /// Which counters persist.
    #[serde(default)]
    pub counters: CounterPolicy,
    /// Where the command came from. Server-assigned, read-only; ignored on input.
    #[serde(default)]
    #[schemars(extend("readOnly" = true))]
    pub origin: Origin,
}

/// How a command's aliases match a message (§6.5), tagged by `kind`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, TS)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum CommandMode {
    /// An alias matched as plain text at a location.
    Basic {
        /// Where in the message the alias must appear.
        location: CommandLocation,
    },
    /// The message matched against a regular expression; captures land in `commands.triggered.captures`.
    Regex {
        /// The regular expression.
        pattern: String,
    },
}

/// Where a basic command alias must appear in the message (§6.5).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, JsonSchema, TS,
)]
#[serde(rename_all = "snake_case")]
pub enum CommandLocation {
    /// The message starts with the alias.
    #[default]
    Start,
    /// The message is exactly the alias.
    Exact,
    /// The alias appears anywhere in the message.
    Anywhere,
}

/// Command cooldowns (§6.5).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct Cooldown {
    /// Milliseconds before anyone may use the command again.
    #[serde(default)]
    pub global_ms: u32,
    /// Milliseconds before the same user may use it again.
    #[serde(default)]
    pub user_ms: u32,
    /// Roles the cooldowns do not apply to; default `["broadcaster"]`.
    #[serde(default = "Cooldown::default_exempt_roles")]
    pub exempt_roles: Vec<String>,
}

impl Cooldown {
    fn default_exempt_roles() -> Vec<String> {
        vec!["broadcaster".into()]
    }
}

impl Default for Cooldown {
    fn default() -> Self {
        Self {
            global_ms: 0,
            user_ms: 0,
            exempt_roles: Self::default_exempt_roles(),
        }
    }
}

/// Who may use a command (§6.5): an allow list or a deny list of users, roles and groups.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct Permissions {
    /// Whether the lists allow or deny.
    #[serde(default)]
    pub mode: PermissionMode,
    /// Platform users.
    #[serde(default)]
    pub users: Vec<UserKey>,
    /// Platform roles (`broadcaster`, `moderator`, `vip`, `subscriber`…, open).
    #[serde(default)]
    pub roles: Vec<String>,
    /// User groups (spec 08).
    #[serde(default)]
    pub groups: Vec<GroupId>,
}

/// Whether a [`Permissions`] list allows or denies (§6.5).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, JsonSchema, TS,
)]
#[serde(rename_all = "snake_case")]
pub enum PermissionMode {
    /// Only the listed users, roles and groups may use it; empty lists allow everyone.
    #[default]
    Allow,
    /// Everyone but the listed users, roles and groups may use it.
    Deny,
}

/// A platform user without the volatile [`crate::UserRef`] fields (§6.5).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct UserKey {
    /// The platform.
    pub platform: PlatformId,
    /// The platform's stable user ID.
    pub user_id: String,
}

/// Which command counters persist (§6.5).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, JsonSchema, TS,
)]
#[serde(rename_all = "camelCase")]
pub struct CounterPolicy {
    /// The global use counter survives a restart.
    #[serde(default)]
    pub persist_global: bool,
    /// Per-user use counters survive a restart.
    #[serde(default)]
    pub persist_user: bool,
}

/// An interval timer (spec 03 §8). Persisted document, kind `timer`; configuration writes are tier 2.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct Timer {
    /// The timer ID.
    pub id: TimerId,
    /// Server-assigned, +1 on every write. Read-only.
    #[serde(default)]
    #[schemars(extend("readOnly" = true))]
    pub revision: u64,
    /// Display name.
    pub name: String,
    /// Disabled timers never fire; a non-repeating timer disables itself after firing.
    pub enabled: bool,
    /// Milliseconds between fires; at least 1000.
    #[schemars(range(min = 1000))]
    pub interval_ms: u64,
    /// When set, each delay is uniform in `[intervalMs, randomUpperMs]`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub random_upper_ms: Option<u64>,
    /// `false`: fire once, then set `enabled = false`.
    pub repeat: bool,
    /// Fire only if at least this many `chat.message` events arrived since the last fire.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub min_chat_lines: Option<u32>,
    /// Where the timer came from. Server-assigned, read-only; ignored on input.
    #[serde(default)]
    #[schemars(extend("readOnly" = true))]
    pub origin: Origin,
}

/// One execution of an action (spec 03 §9, §12). Returned by `runs.get` and `runs.list`; stored in
/// Action History. Read-only on the wire. Counters, cooldown state and the two args snapshots are spec 03
/// storage; `runs.get` returns the snapshots as plain objects next to the record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct Run {
    /// The run ID (UUIDv7).
    pub id: RunId,
    /// The action that ran.
    pub action: ActionId,
    /// The action's name at run time.
    pub action_name: String,
    /// The action's revision at run time (§8.8).
    pub action_revision: u64,
    /// The trigger that started it: absent for `actions.run` and `RunAction` children; a replay copies the
    /// replayed run's trigger (spec 03 §12.3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub trigger: Option<TriggerId>,
    /// The event that fired the trigger, or the replayed event.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub event: Option<EventId>,
    /// The queue it ran on.
    pub queue: QueueId,
    /// Where the run is in its life cycle.
    pub state: RunState,
    /// Why it was cancelled; only when `state` is `cancelled`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub cancel_reason: Option<CancelReason>,
    /// When it was enqueued.
    pub queued_at: Timestamp,
    /// When it started executing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub started_at: Option<Timestamp>,
    /// When it finished, in any final state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub completed_at: Option<Timestamp>,
    /// Milliseconds from `startedAt` to `completedAt`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub duration_ms: Option<u64>,
    /// The `kind` of the context's principal (`token`, `mcp`, `plugin`, `trigger`, `system`; §8.8 rule 7).
    pub principal_kind: String,
    /// The surface the context was created at (§8.8).
    pub surface: Surface,
    /// Set when the context crossed a `causedBy` edge (§8.8 rule 5).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub chain: Option<ChainOrigin>,
    /// The run whose `RunAction` node started this one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub parent: Option<RunId>,
    /// An inline `RunAction` child, sharing the parent's task.
    #[serde(default)]
    pub inline: bool,
    /// Started by the simulator or "Test trigger".
    #[serde(default)]
    pub test: bool,
    /// The run this one replays (`runs.replay`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub replay_of: Option<RunId>,
    /// The firing or replayed event, secrets redacted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub event_snapshot: Option<Event>,
    /// Arg names tainted in the queued snapshot (spec 03 §21).
    #[serde(default)]
    pub untrusted_args_queued: Vec<String>,
    /// Arg names tainted in the snapshot after the run (spec 03 §21).
    #[serde(default)]
    pub untrusted_args_after: Vec<String>,
    /// The failure, when `state` is `failed`, `rejected` or `dropped`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub error: Option<RunError>,
    /// The `Stop` node or `Code` node that ended the run early.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub stopped_by: Option<NodeId>,
    /// Non-fatal findings, including node failures under `onError: continue`.
    #[serde(default)]
    pub warnings: Vec<RunWarning>,
}

/// Where a run is in its life cycle (§6.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum RunState {
    /// Waiting in its queue.
    Queued,
    /// Executing.
    Running,
    /// Finished without a fatal failure.
    Completed,
    /// A node failed under `onError: stop`, or a check failed before a node ran.
    Failed,
    /// Cancelled; see `cancelReason`.
    Cancelled,
    /// Refused before it was enqueued (a disabled draft, a paused queue).
    Rejected,
    /// Dropped by the queue's overflow policy.
    Dropped,
}

/// Why a run was cancelled (§6.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum CancelReason {
    /// The user cancelled it.
    User,
    /// Its queue was cleared.
    Cleared,
    /// The engine shut down.
    Shutdown,
    /// It exceeded `Queue.maxRunMs` (spec 03 §9.11).
    Timeout,
    /// A `Parallel` sibling or the parent run stopped it.
    Parent,
}

/// A run's fatal failure (§6.5), also the shape of `run.lastError` (§6.3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct RunError {
    /// The application error code (§8.5 `data.code`).
    pub code: ErrorCode,
    /// The node that failed, when the failure happened inside the tree.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub node: Option<NodeId>,
    /// Human-readable message.
    pub message: String,
    /// Code-specific details, as in `error.data.details` (§8.5).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional, as = "Option<serde_json::Value>")]
    pub details: Option<serde_json::Value>,
}

/// A non-fatal finding of a run (§6.5).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct RunWarning {
    /// The node it concerns, when any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub node: Option<NodeId>,
    /// A stable code: `loop_cap`, `missing_value`, `break_outside_loop`, … (open).
    pub code: String,
    /// Human-readable message.
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cooldown_defaults_exempt_the_broadcaster() {
        let c: Cooldown = serde_json::from_str("{}").unwrap();
        assert_eq!(c.exempt_roles, vec!["broadcaster".to_owned()]);
        assert_eq!(c, Cooldown::default());
        let c: Cooldown = serde_json::from_str(r#"{"exemptRoles":[]}"#).unwrap();
        assert!(c.exempt_roles.is_empty(), "an explicit empty list is kept");
    }
}
