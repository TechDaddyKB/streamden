//! Domain documents (Spec 02 §6): Action, Queue, Trigger, the sub-action tree, variables and the
//! persisted-document envelope (§7.3). These are persisted and also travel over the API.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::auth::Principal;
use crate::ids::{
    ActionId, EntityId, LocalId, NodeId, OperationName, PluginId, QueueId, TriggerId,
};
use crate::shared::ContributionRef;
use crate::template::{Template, TemplateOr};
use crate::timestamp::Timestamp;

/// A stored automation (§6.1). `revision` and `origin` are server-assigned and read-only on the wire.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct Action {
    /// The action ID.
    pub id: ActionId,
    /// Server-assigned, +1 on every write; approvals bind to it (§8.8). Read-only.
    #[serde(default)]
    #[schemars(extend("readOnly" = true))]
    pub revision: u64,
    /// Unique per installation among non-draft actions, case-insensitively. Actions run by ID or name.
    pub name: String,
    /// Folder path such as `Alerts/Twitch`; organisation only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub group: Option<String>,
    /// Disabled actions never run (§8.8 rule 6).
    pub enabled: bool,
    /// The queue this action runs on; the nil UUID is the Default queue.
    #[serde(default)]
    pub queue: QueueId,
    /// Start all top-level nodes at once (Streamer.bot "Concurrent").
    #[serde(default)]
    pub concurrent: bool,
    /// Run one weighted-random top-level node.
    #[serde(default)]
    pub random: bool,
    /// Runs even when its queue is paused.
    #[serde(default)]
    pub always_run: bool,
    /// History recording policy.
    #[serde(default)]
    pub history: HistoryPolicy,
    /// Triggers that start this action.
    #[serde(default)]
    pub triggers: Vec<Trigger>,
    /// The sub-action tree (§6.4).
    #[serde(default)]
    pub nodes: Vec<SubAction>,
    /// Free-text description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub description: Option<String>,
    /// Tags; also used by scopes such as `actions:run:<tag>`.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Where the action came from (D-029 trust, D-030 drafts). Server-assigned, read-only; ignored on input.
    #[serde(default)]
    #[schemars(extend("readOnly" = true))]
    pub origin: Origin,
}

/// Whether runs of an action are recorded in history (§6.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct HistoryPolicy {
    /// Record finished runs.
    pub record: bool,
    /// Also show runs while they are pending in a queue.
    pub pending: bool,
}

impl Default for HistoryPolicy {
    fn default() -> Self {
        Self {
            record: true,
            pending: false,
        }
    }
}

/// Where a document came from (§6.1). Carries the D-029 trust decision and the D-030 draft state.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize, JsonSchema, TS)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum Origin {
    /// Created or applied by the user in the app.
    #[default]
    User,
    /// Imported from another tool; runs sandboxed until the user trusts it.
    Imported {
        /// The source format, e.g. `streamerbot`.
        from: String,
        /// Set by *Trust this action* after the consent screen.
        trusted: bool,
    },
    /// Proposed by an AI client over MCP; saved disabled until the user applies it.
    AiDraft {
        /// The live action this draft proposes to replace, for `actions.update` drafts.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        draft_of: Option<ActionId>,
    },
    /// Created from a template a plugin ships (spec 12); D-029 treats it like imported content.
    PluginTemplate {
        /// The plugin that ships the template.
        plugin: PluginId,
        /// The `localId` of the template contribution.
        template: LocalId,
    },
}

/// The server-assigned writer identity on everything that records one (§6.1, A7): `VariableRecord.updatedBy`,
/// the `variables.*_updated` payloads, audit rows. Derived from the writing context's origin principal
/// (§8.8 rule 5): `Token` → `user`, `Mcp` → `mcp`, `Plugin` → `plugin`, `System` → `system`, an unchained
/// `Trigger` → `user`; with a chain the `chain.originPrincipal` decides; the importer → `imported`. Distinct
/// from [`Origin`] (where a document came from) and [`Principal`] (who acts).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize, JsonSchema, TS)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum UpdatedBy {
    /// The user, through the UI, a user token or their own stored automation.
    #[default]
    User,
    /// The importer.
    Imported,
    /// An MCP session, directly or through a `causedBy` chain it started.
    Mcp,
    /// A plugin.
    Plugin {
        /// The plugin.
        plugin_id: PluginId,
    },
    /// The engine itself (startup, timers, migrations, `queues.pause` write-back).
    System,
}

/// A run queue (§6.1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct Queue {
    /// The queue ID; the nil UUID is the Default queue.
    pub id: QueueId,
    /// Display name.
    pub name: String,
    /// FIFO, one run at a time.
    pub blocking: bool,
    /// Paused queues hold runs (except `alwaysRun` actions).
    #[serde(default)]
    pub paused: bool,
    /// Maximum pending runs; absent means the engine default (spec 03).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub max_pending: Option<u32>,
    /// What happens when `maxPending` is reached.
    #[serde(default)]
    pub overflow: Overflow,
    /// A run exceeding it is cancelled (`cancelReason: timeout`, spec 03 §9.11); absent means no limit.
    /// Capped by `engine.queue.maxRunCeilingMs` (A9).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub max_run_ms: Option<u64>,
}

/// Queue overflow policy (§6.1).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, JsonSchema, TS,
)]
#[serde(rename_all = "snake_case")]
pub enum Overflow {
    /// Refuse new runs while the queue is full.
    #[default]
    RejectNew,
    /// Drop the oldest pending run to make room.
    DropOldest,
}

/// A trigger (§6.2): a trigger type **is** an event type. On the wire the contribution is flattened:
/// `{ "type": "twitch.cheer" }` or `{ "type": "acme.gift", "plugin": "io.github.alice.acmelive" }`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct Trigger {
    /// The trigger ID.
    pub id: TriggerId,
    /// The event type this trigger listens to, with its plugin when not core.
    #[serde(flatten)]
    #[ts(flatten)]
    pub contribution: ContributionRef,
    /// Disabled triggers never fire.
    pub enabled: bool,
    /// Read-only, derived: `plugin` is not installed; the trigger never fires until it is (§4).
    #[serde(default)]
    #[schemars(extend("readOnly" = true))]
    pub unresolved: bool,
    /// Fire even when a more specific trigger matched (Streamer.bot semantics).
    #[serde(default)]
    pub always_run: bool,
    /// Filters, AND-ed.
    #[serde(default)]
    pub filters: Vec<Filter>,
    /// Type-specific settings (command text, timer interval…), schema from the catalog. Always literal (§10.1).
    #[serde(default)]
    #[ts(as = "serde_json::Value")]
    pub config: serde_json::Map<String, serde_json::Value>,
}

/// A trigger filter on a payload field (§6.2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct Filter {
    /// Dotted path into the payload: `amount`, `user.roles`.
    pub field: String,
    /// The comparison.
    pub op: FilterOp,
    /// The operand, validated against the field's schema; `between` takes `[min, max]`.
    #[ts(as = "serde_json::Value")]
    pub value: serde_json::Value,
}

/// Filter operators (§6.2). A closed union: adding a value is a breaking change (§7.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum FilterOp {
    /// Equal.
    Eq,
    /// Not equal.
    Ne,
    /// Member of the list in `value`.
    In,
    /// Not a member of the list in `value`.
    NotIn,
    /// Greater than or equal.
    Gte,
    /// Less than or equal.
    Lte,
    /// Between `value[0]` and `value[1]`, inclusive.
    Between,
    /// String or array contains `value`.
    Contains,
    /// Matches the regular expression in `value`.
    Regex,
    /// The field is present (`value` is `true`) or absent (`false`).
    Exists,
}

/// A node of the sub-action tree with its common fields; the node itself is flattened by `kind` (§6.4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct SubAction {
    /// The node ID.
    pub id: NodeId,
    /// Disabled nodes are skipped and count as tier 0 (§8.8 rule 2).
    pub enabled: bool,
    /// Weight used when the parent picks one random child.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub weight: Option<f32>,
    /// Author's note.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub note: Option<String>,
    /// What a failure of this node does to the run (A1); default `stop`. Authorization and approval
    /// failures always stop the run whatever this says (§8.8 rule 3).
    #[serde(default, skip_serializing_if = "OnError::is_stop")]
    pub on_error: OnError,
    /// The node, internally tagged by `kind`.
    #[serde(flatten)]
    #[ts(flatten)]
    pub node: Node,
}

/// What a node failure does to the run (§6.4, A1). Applies to node failures only: an operation error,
/// rendered params or a result that do not validate, a timeout, an unavailable `RunAction` target, a
/// raising `Code` node.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, JsonSchema, TS,
)]
#[serde(rename_all = "snake_case")]
pub enum OnError {
    /// End the run `Failed { error }`.
    #[default]
    Stop,
    /// Record the failure as a `RunWarning`, set `run.lastError` (§6.3) and go on with the next sibling.
    Continue,
}

impl OnError {
    /// Whether this is the default, which the wire form omits.
    pub fn is_stop(&self) -> bool {
        *self == Self::Stop
    }
}

crate::tagged::string_kind! { "Node";
/// The node kinds of the sub-action tree (§6.4). Extensible with a fallback: a `kind` this build does not
/// know decodes as [`Node::Unknown`], which a client renders as a read-only placeholder and which keeps
/// the raw node so a later save does not lose it (§7.2). [`Node::Placeholder`] is different: it is the
/// explicit node an importer writes for a type it could not convert.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema, TS)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum Node {
    /// Any sub-action-exposed operation (§8.1). Wire: `{ "kind": "op", "type": "acme.send_chat",
    /// "plugin": "io.github…", "params": {…} }`.
    Op {
        /// The operation, with its plugin when not core.
        #[serde(flatten)]
        #[ts(flatten)]
        contribution: ContributionRef,
        /// Parameters, validated against the operation's authoring schema at save time and its invocation
        /// schema after rendering (§10.1).
        #[serde(default)]
        #[ts(as = "serde_json::Value")]
        params: serde_json::Map<String, serde_json::Value>,
        /// Store the operation's result under this run-argument name.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        save_as: Option<String>,
    },
    /// A named group of children.
    Group {
        /// Display name.
        name: String,
        /// Display colour, as a CSS colour string.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        color: Option<String>,
        /// Run one weighted-random child instead of all.
        #[serde(default)]
        random: bool,
        /// The children.
        #[serde(default)]
        children: Vec<SubAction>,
    },
    /// Conditional branch.
    If {
        /// The condition.
        condition: Condition,
        /// Nodes run when the condition holds.
        #[serde(default)]
        then: Vec<SubAction>,
        /// Nodes run otherwise.
        #[serde(default)]
        r#else: Vec<SubAction>,
    },
    /// Multi-way branch on a rendered value.
    Switch {
        /// The value to match.
        value: Template,
        /// Compare after coercing both sides to the same type.
        #[serde(default)]
        auto_type: bool,
        /// The cases, matched in order.
        #[serde(default)]
        cases: Vec<Case>,
        /// Nodes run when no case matched.
        #[serde(default)]
        default: Vec<SubAction>,
    },
    /// Loop while a condition holds.
    While {
        /// The condition, evaluated before every iteration.
        condition: Condition,
        /// Safety bound on iterations; absent means the engine default (spec 03).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        max_iterations: Option<u32>,
        /// The loop body.
        #[serde(default)]
        children: Vec<SubAction>,
    },
    /// Iterate over a collection (A2; semantics spec 03 §9.5, writer contract §8.8). Writes `itemVar` and
    /// `indexVar` (0-based) as ordinary run args; reserved roots are refused at save time.
    ForEach {
        /// The collection: a literal, or one `{…}` form rendered once per loop to an array (element
        /// order) or an object (`{ key, value }` pairs in sorted key order). `null` or a missing value is
        /// zero iterations; any other scalar fails the node with `INVALID_PARAMS` (`not_iterable`).
        #[ts(as = "TemplateOr<serde_json::Value>")]
        items: TemplateOr<serde_json::Value>,
        /// The run arg that receives the current element.
        item_var: String,
        /// The run arg that receives the 0-based index, when wanted.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        index_var: Option<String>,
        /// Safety bound on iterations; absent means the engine default (spec 03).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        max_iterations: Option<u32>,
        /// The loop body.
        #[serde(default)]
        children: Vec<SubAction>,
    },
    /// Leave the innermost loop.
    BreakLoop,
    /// Skip to the next iteration of the innermost loop.
    ContinueLoop,
    /// End the run (Streamer.bot "Break").
    Stop,
    /// Run branches concurrently and join when all finish (D-028).
    Parallel {
        /// The branches.
        #[serde(default)]
        branches: Vec<Vec<SubAction>>,
    },
    /// Wait.
    Delay {
        /// Milliseconds to wait: a literal or one `{…}` form.
        ms: TemplateOr<u64>,
    },
    /// Run another action.
    RunAction {
        /// The target action.
        action: ActionRef,
        /// Inline (shares args, waits) or queued.
        mode: RunMode,
        /// Arguments passed to the child; templates rendered in the parent's context.
        #[serde(default)]
        args: BTreeMap<String, Template>,
    },
    /// Execute code (tier 4, §8.8 rule 2).
    Code {
        /// The language.
        lang: CodeLang,
        /// Where the source comes from.
        source: CodeSource,
        /// Deadline in milliseconds; absent means the engine default (spec 04).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        timeout_ms: Option<u32>,
    },
    /// An unknown imported node type; never runs; kept verbatim (WS-15 rule).
    Placeholder {
        /// The importer that produced it, e.g. `streamerbot`, and the original type name.
        origin: String,
        /// The original node JSON, untouched.
        #[ts(as = "serde_json::Value")]
        original: serde_json::Value,
    },
    /// Receiving fallback for a `kind` this build does not know (§7.2): shown read-only, never run.
    /// Serialises back to the raw node it was decoded from.
    #[serde(untagged)]
    #[ts(untagged)]
    Unknown(UnknownNode),
}
}

/// A node whose `kind` this build does not know (§7.2). The node-specific members are kept verbatim so
/// the node survives a round trip through an older client.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct UnknownNode {
    /// The unknown `kind` tag. Never one of the kinds this build knows: decoding refuses a known kind
    /// here, so a malformed known node is an error and never falls through to this variant.
    #[serde(deserialize_with = "unknown_node_kind")]
    #[schemars(schema_with = "unknown_node_kind_schema")]
    pub kind: String,
    /// The node-specific members, untouched.
    #[serde(flatten)]
    #[ts(flatten)]
    pub fields: serde_json::Map<String, serde_json::Value>,
}

fn unknown_node_kind_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    crate::schema::unknown_tag_schema(Node::KNOWN_KINDS)
}

fn unknown_node_kind<'de, D: serde::Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    crate::schema::deserialize_unknown_tag(d, Node::KNOWN_KINDS)
}

/// One case of a [`Node::Switch`] (§6.4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct Case {
    /// The values to compare the switch value with: at least one; the case matches when any renders
    /// equal to the switch value (`eq`, with `autoType` coercion when enabled).
    #[schemars(length(min = 1))]
    pub values: Vec<Template>,
    /// Nodes run when this case matches.
    #[serde(default)]
    pub children: Vec<SubAction>,
}

/// A comparison between two rendered templates (§6.4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct Condition {
    /// Left operand.
    pub left: Template,
    /// The comparison.
    pub op: CompareOp,
    /// Right operand.
    pub right: Template,
    /// Compare after coercing both sides to the same type (number, boolean, string).
    #[serde(default)]
    pub auto_type: bool,
    /// Case-insensitive string comparison.
    #[serde(default)]
    pub ignore_case: bool,
}

/// Comparison operators of [`Condition`] (§6.4), the Streamer.bot set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum CompareOp {
    /// Equal.
    Eq,
    /// Not equal.
    Ne,
    /// Left contains right.
    Contains,
    /// Left matches the regular expression on the right; captures land in `match`.
    Regex,
    /// Less than.
    Lt,
    /// Greater than.
    Gt,
    /// Less than or equal.
    Lte,
    /// Greater than or equal.
    Gte,
    /// The left path does not exist in the run context.
    NotExists,
    /// The left value is null or an empty string.
    IsNullOrEmpty,
}

/// How a [`Node::RunAction`] names its target (§6.4, §8.8 rule 2). A `template` target is unresolvable at
/// check time and counts as tier 4.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum ActionRef {
    /// By ID.
    Id {
        /// The action ID.
        id: ActionId,
    },
    /// By unique name.
    Name {
        /// The action name.
        name: String,
    },
    /// Resolved at run time from a template.
    Template {
        /// Renders to an action ID or name.
        template: Template,
    },
}

/// How a child action runs (§6.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum RunMode {
    /// In the same run: shares args, waits for completion.
    Inline,
    /// Enqueued on the target's queue with its own args scope.
    Queued,
}

/// Script languages of a [`Node::Code`] (D-003).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum CodeLang {
    /// JavaScript.
    Js,
    /// Python.
    Python,
    /// C# (optional, D-003).
    Csharp,
}

/// Where a [`Node::Code`] gets its source (§6.4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum CodeSource {
    /// Source stored in the node.
    Inline {
        /// The source text.
        source: String,
    },
    /// Source stored as a script file managed by the app.
    ScriptRef {
        /// Path of the script, relative to the scripts directory.
        path: String,
    },
}

/// A stored variable value (§6.3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct VariableRecord {
    /// The value (any JSON value). Its provenance is decided per value: MCP wraps it, and the engine seeds
    /// taint from it, exactly when the sibling `untrusted` is `true` (§3, A10).
    #[ts(as = "serde_json::Value")]
    #[schemars(extend("x-den-untrusted" = "conditional"))]
    pub value: serde_json::Value,
    /// Survives a restart; an attribute of the entry, not part of its key (A11).
    pub persisted: bool,
    /// The provenance flag spec 03 §21 writes (chat-derived, plugin-, MCP- or import-written values).
    /// Never a client input: a write replaces value and flag together. Server-assigned, read-only.
    #[schemars(extend("readOnly" = true))]
    pub untrusted: bool,
    /// When it was last written.
    pub updated_at: Timestamp,
    /// Who last wrote it (A7). Server-assigned, read-only.
    #[schemars(extend("readOnly" = true))]
    pub updated_by: UpdatedBy,
}

/// A deferred configuration write created by a tier-2 operation under an MCP context (§6.1). *Apply*
/// replays `operation` with `params` under the user's context; *Discard* deletes it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct PendingChange {
    /// The pending change ID.
    pub id: EntityId,
    /// The kind of entity the change targets: `action`, `queue`, `command`, `timer`, `overlay`,
    /// `settings` (open). Named `targetKind` because the stored envelope's own `kind` is `pending_change`.
    pub target_kind: String,
    /// The live entity the change targets, when it targets an existing one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub draft_of: Option<EntityId>,
    /// The operation *Apply* will execute.
    pub operation: OperationName,
    /// The operation's params as proposed.
    #[serde(default)]
    #[ts(as = "serde_json::Value")]
    pub params: serde_json::Map<String, serde_json::Value>,
    /// Who proposed it.
    pub created_by: Principal,
}

/// The persisted-document envelope (§7.3): every stored document is wrapped as
/// `{ "kind": "action", "schemaVersion": N, "data": {…} }`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct StoredDocument {
    /// The document kind, one of [`StoredDocument::KINDS`] (§7.3).
    pub kind: String,
    /// The stored shape's version; migrations chain from N to N+1 on load.
    pub schema_version: u32,
    /// The document itself.
    #[ts(as = "serde_json::Value")]
    pub data: serde_json::Value,
}

impl StoredDocument {
    /// The `action` document kind (§6.1).
    pub const KIND_ACTION: &'static str = "action";
    /// The `queue` document kind (§6.1).
    pub const KIND_QUEUE: &'static str = "queue";
    /// The `command` document kind (§6.5).
    pub const KIND_COMMAND: &'static str = "command";
    /// The `timer` document kind (§6.5).
    pub const KIND_TIMER: &'static str = "timer";
    /// The `pending_change` document kind (§6.1).
    pub const KIND_PENDING_CHANGE: &'static str = "pending_change";
    /// The `settings` document kind (spec 03 §16).
    pub const KIND_SETTINGS: &'static str = "settings";
    /// The `export` document kind (spec 03 §13.4).
    pub const KIND_EXPORT: &'static str = "export";
    /// The `export_meta` document kind (spec 03 §13.4).
    pub const KIND_EXPORT_META: &'static str = "export_meta";
    /// The `design` document kind (spec 09).
    pub const KIND_DESIGN: &'static str = "design";
    /// The `view_state` document kind (spec 07).
    pub const KIND_VIEW_STATE: &'static str = "view_state";
    /// The `plugin_data` document kind (spec 12).
    pub const KIND_PLUGIN_DATA: &'static str = "plugin_data";

    /// Every persisted document kind of §7.3. Spec 03 §13.2 stores the engine's kinds.
    pub const KINDS: &'static [&'static str] = &[
        Self::KIND_ACTION,
        Self::KIND_QUEUE,
        Self::KIND_COMMAND,
        Self::KIND_TIMER,
        Self::KIND_PENDING_CHANGE,
        Self::KIND_SETTINGS,
        Self::KIND_EXPORT,
        Self::KIND_EXPORT_META,
        Self::KIND_DESIGN,
        Self::KIND_VIEW_STATE,
        Self::KIND_PLUGIN_DATA,
    ];

    /// `schemaVersion` of the `action` document kind written by this build.
    pub const ACTION_SCHEMA_VERSION: u32 = 1;
    /// `schemaVersion` of the `queue` document kind written by this build.
    pub const QUEUE_SCHEMA_VERSION: u32 = 1;
    /// `schemaVersion` of the `command` document kind written by this build (§6.5).
    pub const COMMAND_SCHEMA_VERSION: u32 = 1;
    /// `schemaVersion` of the `timer` document kind written by this build (§6.5).
    pub const TIMER_SCHEMA_VERSION: u32 = 1;
    /// `schemaVersion` of the `pending_change` document kind written by this build.
    pub const PENDING_CHANGE_SCHEMA_VERSION: u32 = 1;

    /// Whether `kind` is one of the document kinds this build knows (§7.3).
    pub fn is_known_kind(kind: &str) -> bool {
        Self::KINDS.contains(&kind)
    }
}

impl Node {
    /// The `kind` tags this build knows. The schema of [`Node::Unknown`] excludes them.
    pub const KNOWN_KINDS: &'static [&'static str] = &[
        "op",
        "group",
        "if",
        "switch",
        "while",
        "for_each",
        "break_loop",
        "continue_loop",
        "stop",
        "parallel",
        "delay",
        "run_action",
        "code",
        "placeholder",
    ];

    /// The `kind` tag of this node on the wire.
    pub fn kind(&self) -> &str {
        match self {
            Self::Op { .. } => "op",
            Self::Group { .. } => "group",
            Self::If { .. } => "if",
            Self::Switch { .. } => "switch",
            Self::While { .. } => "while",
            Self::ForEach { .. } => "for_each",
            Self::BreakLoop => "break_loop",
            Self::ContinueLoop => "continue_loop",
            Self::Stop => "stop",
            Self::Parallel { .. } => "parallel",
            Self::Delay { .. } => "delay",
            Self::RunAction { .. } => "run_action",
            Self::Code { .. } => "code",
            Self::Placeholder { .. } => "placeholder",
            Self::Unknown(u) => &u.kind,
        }
    }
}
