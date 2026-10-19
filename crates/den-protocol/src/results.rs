//! The three result shapes of an operation call (Spec 02 §8.1, §6.1) and the `plugins.call` alias
//! shapes. They are disjoint on the wire: a normal result never defines `dryRun` or `draft`,
//! `DryRunResult` requires the literal `dryRun: true`, and `DraftResult` requires the object `draft`.

use std::borrow::Cow;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use ts_rs::TS;

use crate::ids::{ActionId, EntityId, NodeId, OperationName, PluginId, Sha256};
use crate::shared::RiskTier;

/// The JSON literal `true`. Used for discriminator fields that must be exactly `true`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, TS)]
#[ts(type = "true")]
pub struct True;

impl Serialize for True {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_bool(true)
    }
}

impl<'de> Deserialize<'de> for True {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match bool::deserialize(deserializer)? {
            true => Ok(True),
            false => Err(serde::de::Error::custom("expected the literal `true`")),
        }
    }
}

impl JsonSchema for True {
    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("True")
    }

    fn schema_id() -> Cow<'static, str> {
        Cow::Borrowed("den_protocol::results::True")
    }

    fn inline_schema() -> bool {
        true
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({ "type": "boolean", "const": true })
    }
}

/// Result of a call with `dryRun: true` (§8.1): what the call would do, with nothing executed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct DryRunResult {
    /// Always `true`; discriminates this shape from a normal result.
    pub dry_run: True,
    /// Every operation the call would execute, repeats included.
    pub plan: Vec<PlanStep>,
    /// For a composite call: every action the run can reach, pinned to its revision (§8.8 rule 4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub actions: Option<Vec<ActionRevisionRef>>,
    /// For configuration operations: the proposed change as a diff, when the operation can compute one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional, as = "Option<serde_json::Value>")]
    pub diff: Option<serde_json::Value>,
    /// Human-readable warnings.
    #[serde(default)]
    pub warnings: Vec<String>,
    /// Whether the real call would need in-app approval under the calling context.
    pub would_require_approval: bool,
    /// The call's effective tier (§8.8 rule 2).
    pub effective_tier: RiskTier,
}

/// One step of a dry-run plan (§8.1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct PlanStep {
    /// The operation that would run.
    pub operation: OperationName,
    /// Its params, rendered, with secrets redacted to `"***"`.
    #[ts(as = "serde_json::Value")]
    pub params: serde_json::Value,
    /// The operation's tier.
    pub tier: RiskTier,
    /// The stored action this step belongs to, for composite calls.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub action: Option<ActionId>,
    /// The node this step belongs to, for composite calls.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub node: Option<NodeId>,
    /// The binding hash (§8.8), on tier ≥ 3 steps.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub args_hash: Option<Sha256>,
    /// Params paths that take run-produced values (§8.8 rule 4), on tier ≥ 3 steps.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub dynamic_fields: Option<Vec<String>>,
    /// A short description of the target (a scene name, a channel), when the operation has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub target: Option<String>,
    /// Human-readable description of the step.
    pub description: String,
}

/// An action pinned to a revision (§8.1, §8.8). An object, never a tuple (§3).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct ActionRevisionRef {
    /// The action.
    pub id: ActionId,
    /// Its revision at plan time.
    pub revision: u64,
}

/// Result of a tier-2 configuration call that was deferred into a draft under an MCP context (§6.1):
/// nothing live changed; `plan` says what *Apply* will execute.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct DraftResult {
    /// The draft that was created.
    pub draft: DraftRef,
    /// What *Apply* will execute.
    pub plan: Vec<PlanStep>,
    /// Human-readable warnings.
    #[serde(default)]
    pub warnings: Vec<String>,
}

/// Reference to a draft (§6.1).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct DraftRef {
    /// The draft's ID (an `ai_draft` action ID or a pending-change ID).
    pub id: EntityId,
    /// Which draft representation was created.
    pub kind: DraftKind,
    /// The live entity the draft proposes to change, when it targets an existing one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub draft_of: Option<EntityId>,
    /// The operation that was deferred.
    pub operation: OperationName,
}

/// The two draft representations (§6.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum DraftKind {
    /// An `ai_draft` action (`actions.create`, `actions.update`).
    Action,
    /// A [`crate::PendingChange`] document (every other configuration write).
    PendingChange,
}

/// Params of the `plugins.call` alias (§8.1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct PluginCallParams {
    /// The plugin ID (§4).
    pub plugin: PluginId,
    /// The operation's `localId`; the target is `<installed slug>.<method>`.
    pub method: crate::ids::LocalId,
    /// Params for the target, validated against the target's author params schema. The reserved `dryRun`
    /// and `approvalId` are only accepted on the outer call.
    #[serde(default)]
    #[ts(as = "serde_json::Value")]
    pub params: serde_json::Map<String, serde_json::Value>,
}

/// Normal result of the `plugins.call` alias (§8.1): the target's result, wrapped so the alias meets the
/// tier ≥ 1 object-root rule whatever the target returns.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct PluginCallResult {
    /// The target's normal result (`null`, scalar, array or object), unchanged apart from the MCP transforms.
    #[ts(as = "serde_json::Value")]
    pub value: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dry_run_discriminator_must_be_true() {
        let json = r#"{"dryRun":false,"plan":[],"warnings":[],"wouldRequireApproval":false,"effectiveTier":1}"#;
        assert!(serde_json::from_str::<DryRunResult>(json).is_err());
    }
}
