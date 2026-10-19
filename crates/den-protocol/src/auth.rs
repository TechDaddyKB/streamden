//! The authorization context every call and run carries (Spec 02 §8.8). Created at the entry
//! surface and never widened afterwards; no API input can set any part of it (rule 7).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::ids::{
    ActionId, ApprovalId, NodeId, OperationName, PluginId, RunId, Scope, SessionId, Sha256,
    TokenId, TriggerId,
};
use crate::results::ActionRevisionRef;
use crate::shared::RiskTier;
use crate::timestamp::Timestamp;

/// The authorization context of a call or run (§8.8).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct AuthContext {
    /// Who started the entry call or run.
    pub principal: Principal,
    /// The surface the context was created at.
    pub surface: Surface,
    /// Scopes the context may use (§8.7).
    #[serde(default)]
    pub scopes: Vec<Scope>,
    /// The highest tier this context may ever execute.
    pub max_tier: RiskTier,
    /// Approvals bound to concrete calls (rule 4).
    #[serde(default)]
    pub approvals: Vec<ApprovalBinding>,
    /// Runs this context has passed through (nested `RunAction`, `causedBy` chains), newest last.
    #[serde(default)]
    pub lineage: Vec<RunId>,
    /// Set once the context has crossed a `causedBy` edge (rule 5).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub chain: Option<ChainOrigin>,
}

/// Who a context acts for (§8.8), tagged by `kind`. No variant may have a field named `kind`, which is
/// why the token variant carries `tokenKind` (§17.1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum Principal {
    /// An API, UI or overlay token. `maxTier` is the token's limit, default 4.
    Token {
        /// The token record.
        token_id: TokenId,
        /// The token's display name.
        name: String,
        /// What kind of client the token was issued to.
        token_kind: TokenKind,
    },
    /// An MCP session. `maxTier` is always 3 (D-030).
    Mcp {
        /// The token the MCP client authenticated with.
        token_id: TokenId,
        /// The MCP session.
        session_id: SessionId,
        /// The MCP client.
        client: ClientInfo,
    },
    /// A plugin making a host call. `maxTier` comes from its permissions (spec 12).
    Plugin {
        /// The plugin.
        plugin_id: PluginId,
    },
    /// An event-started run. `maxTier` is 4 unless derived from a causing run (rule 5).
    Trigger {
        /// The trigger that fired.
        trigger_id: TriggerId,
        /// The action it started.
        action_id: ActionId,
    },
    /// Startup, timers, migrations. `maxTier` 4.
    System,
}

impl Principal {
    /// The `kind` tag of this principal on the wire; what `run.principal` exposes to templates (rule 7).
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Token { .. } => "token",
            Self::Mcp { .. } => "mcp",
            Self::Plugin { .. } => "plugin",
            Self::Trigger { .. } => "trigger",
            Self::System => "system",
        }
    }
}

/// What kind of client a token was issued to (§8.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum TokenKind {
    /// The app's own UI.
    Ui,
    /// A third-party API client.
    Api,
    /// An overlay or deck, limited to `events:read` and `variables:read` scopes (§8.7).
    Overlay,
}

/// A client's self-description (§8.3).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct ClientInfo {
    /// The client's name.
    pub name: String,
    /// The client's version.
    pub version: String,
}

/// The surface a context was created at (§8.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum Surface {
    /// WebSocket or REST.
    Rpc,
    /// The MCP server.
    Mcp,
    /// A sub-action node inside a run.
    SubAction,
    /// A plugin host call.
    PluginHost,
    /// A trigger firing on an event.
    Event,
    /// The engine itself.
    Internal,
}

/// Where a derived context's restrictions come from (§8.8 rule 5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct ChainOrigin {
    /// The run started at the entry surface; its restrictions are the ones inherited.
    pub origin_run: RunId,
    /// That run's principal; tier policy (MCP exclusion, draft-first) is keyed on it.
    pub origin_principal: Principal,
    /// `causedBy` edges crossed so far.
    pub hops: u32,
}

/// An approval bound to one call (§8.8 rule 4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalBinding {
    /// The approval handle.
    pub id: ApprovalId,
    /// The approved operation.
    pub operation: OperationName,
    /// Binding hash of the call's own params, minus `dryRun` and `approvalId`.
    pub args_hash: Sha256,
    /// For composite calls: what was approved.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub plan: Option<ApprovedPlan>,
    /// A session grant ("approve for 15 min") exempts argument hashes but still pins revisions and the call multiset.
    pub session_grant: bool,
    /// When the approval stops permitting a call to **start** (one-shot) or descendants to execute (session grant).
    pub expires_at: Timestamp,
}

/// What an approval of a composite call covers (§8.8 rule 4). Computed by the server, never from
/// client-supplied data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct ApprovedPlan {
    /// Every action the plan reaches (root, inline and queued `RunAction` targets), pinned.
    #[serde(default)]
    pub actions: Vec<ActionRevisionRef>,
    /// Every tier-3 node occurrence the plan would execute; a repeated node or operation is a separate entry.
    #[serde(default)]
    pub calls: Vec<PlannedCall>,
}

/// One planned tier-3 call (§8.8 rule 4). Exactly one identity form is set: a node in a stored action
/// (`action` and `node`), or a host call a plugin's dry run declared (`plugin`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct PlannedCall {
    /// The stored action, for a node call.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub action: Option<ActionId>,
    /// The node, for a node call.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub node: Option<NodeId>,
    /// The plugin, for a declared host call.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub plugin: Option<PluginId>,
    /// The operation.
    pub operation: OperationName,
    /// The binding hash of the call's params after the binding render.
    pub args_hash: Sha256,
    /// Params paths (as in `fieldErrors.path`) whose binding render references any sentinel; part of the plan
    /// identity and shown to the approver.
    #[serde(default)]
    pub dynamic_fields: Vec<String>,
}
