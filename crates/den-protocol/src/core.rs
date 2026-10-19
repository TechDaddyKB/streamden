//! Core registration (Spec 02 §9.1): everything Spec 02 itself defines, registered explicitly into a
//! [`Catalog`]. Connectors and other specs register their own operations and events the same way.

use schemars::JsonSchema;
use serde_json::json;

use crate::catalog::{
    ArgWrites, Catalog, CatalogError, Exposure, OperationDef, OperationExample, ResultShape,
};
use crate::ids::{OperationName, Scope};
use crate::results::{DraftResult, DryRunResult, PluginCallParams, PluginCallResult};
use crate::shared::RiskTier;
use crate::version::PROTOCOL_VERSION;

/// Registers the core operations and the shared types every spec references (§5.3, §8.1, §8.8).
pub fn register(catalog: &mut Catalog) -> Result<(), CatalogError> {
    register_shared_types(catalog);
    let plugins_call = plugins_call(catalog);
    catalog.register_operation(plugins_call)?;
    Ok(())
}

/// Shared types published under `types` even when no core operation references them (§5.3, §9.1).
fn register_shared_types(catalog: &mut Catalog) {
    fn reg<T: JsonSchema + ?Sized>(catalog: &mut Catalog) {
        catalog.register_type::<T>();
    }
    // §4 IDs
    reg::<crate::ids::ActionId>(catalog);
    reg::<crate::ids::TriggerId>(catalog);
    reg::<crate::ids::NodeId>(catalog);
    reg::<crate::ids::QueueId>(catalog);
    reg::<crate::ids::CommandId>(catalog);
    reg::<crate::ids::TimerId>(catalog);
    reg::<crate::ids::ConnectionId>(catalog);
    reg::<crate::ids::OverlayId>(catalog);
    reg::<crate::ids::EventId>(catalog);
    reg::<crate::ids::RunId>(catalog);
    reg::<crate::ids::TokenId>(catalog);
    reg::<crate::ids::EntityId>(catalog);
    reg::<crate::ids::PluginId>(catalog);
    reg::<crate::ids::Slug>(catalog);
    reg::<crate::ids::LocalId>(catalog);
    reg::<crate::ids::EventType>(catalog);
    reg::<crate::ids::OperationName>(catalog);
    reg::<crate::ids::VariableName>(catalog);
    reg::<crate::ids::ApprovalId>(catalog);
    reg::<crate::ids::SessionId>(catalog);
    reg::<crate::ids::Sha256>(catalog);
    reg::<crate::ids::Scope>(catalog);
    reg::<crate::ids::TopicGlob>(catalog);
    reg::<crate::ids::PlatformId>(catalog);
    reg::<crate::timestamp::Timestamp>(catalog);
    // §5 events and shared core types
    reg::<crate::events::Event>(catalog);
    reg::<crate::shared::UserRef>(catalog);
    reg::<crate::shared::ChatText>(catalog);
    reg::<crate::shared::Fragment>(catalog);
    reg::<crate::shared::Money>(catalog);
    reg::<crate::shared::ImageRef>(catalog);
    reg::<crate::shared::ContributionRef>(catalog);
    reg::<crate::shared::RiskTier>(catalog);
    reg::<crate::shared::UnknownEventPayload>(catalog);
    // §6 domain documents
    reg::<crate::domain::Action>(catalog);
    reg::<crate::domain::Queue>(catalog);
    reg::<crate::domain::Trigger>(catalog);
    reg::<crate::domain::SubAction>(catalog);
    reg::<crate::domain::Node>(catalog);
    reg::<crate::domain::VariableRecord>(catalog);
    reg::<crate::domain::PendingChange>(catalog);
    reg::<crate::domain::StoredDocument>(catalog);
    reg::<crate::template::Template>(catalog);
    // §8 results, catalog shapes, authorization, envelope
    reg::<DryRunResult>(catalog);
    reg::<DraftResult>(catalog);
    reg::<crate::catalog::OperationDef>(catalog);
    reg::<crate::catalog::EventDef>(catalog);
    reg::<crate::auth::AuthContext>(catalog);
    reg::<crate::auth::Principal>(catalog);
    reg::<crate::auth::ApprovalBinding>(catalog);
    reg::<crate::rpc::RpcRequest>(catalog);
    reg::<crate::rpc::RpcResponse>(catalog);
    reg::<crate::rpc::RpcNotification>(catalog);
    reg::<crate::rpc::RpcError>(catalog);
    reg::<crate::rpc::Hello>(catalog);
    reg::<crate::rpc::SessionIdentifyParams>(catalog);
    reg::<crate::rpc::SessionIdentifyResult>(catalog);
    reg::<crate::rpc::SessionReidentifyParams>(catalog);
    reg::<crate::rpc::EventsSubscribeParams>(catalog);
    reg::<crate::rpc::EventsUnsubscribeParams>(catalog);
    reg::<crate::rpc::EventNotification>(catalog);
    reg::<crate::rpc::EventsDroppedNotification>(catalog);
    reg::<crate::rpc::BatchExecuteParams>(catalog);
    reg::<crate::rpc::BatchExecuteResult>(catalog);
    // §11 manifest
    reg::<crate::manifest::PluginManifest>(catalog);
}

/// `plugins.call` (§8.1): the generic alias for clients that only know plugin IDs. A catalog operation
/// with a dynamic effective tier, its own static result contract and no draft result.
fn plugins_call(catalog: &mut Catalog) -> OperationDef {
    let params = catalog.schema_for::<PluginCallParams>();
    let result = catalog.schema_for::<PluginCallResult>();
    OperationDef {
        name: OperationName::new("plugins.call").expect("valid"),
        title: "Call a plugin operation".into(),
        description: "Plugins. Calls an operation a plugin contributes, by plugin ID and local method name. \
             The target must pass its own exposure, scope and tier checks on the calling surface; the \
             effective tier is max(1, the target's tier)."
            .into(),
        category: vec!["Plugins".into()],
        params,
        result,
        tier: RiskTier::REVERSIBLE_WRITE,
        scopes: vec![Scope::new("plugins:call").expect("valid")],
        expose: Exposure { rpc: true, sub_action: false, mcp: true },
        idempotent: false,
        writes_args: ArgWrites::None,
        external: true,
        timeout_ms: None,
        platforms: None,
        high_volume_result: false,
        since: PROTOCOL_VERSION.into(),
        deprecated: None,
        examples: vec![
            OperationExample {
                title: "Read a plugin's gift catalog (tier-0 target)".into(),
                params: json!({
                    "plugin": "io.github.alice.acmelive",
                    "method": "get_gift_catalog",
                    "params": {}
                }),
                shape: ResultShape::Normal,
                result: json!({
                    "value": [
                        { "id": "rocket", "name": "Rocket", "amountMinor": 500, "currency": "USD" }
                    ]
                }),
            },
            OperationExample {
                title: "Dry run of a tier-0 target: one planned step, nothing invoked".into(),
                params: json!({
                    "plugin": "io.github.alice.acmelive",
                    "method": "get_gift_catalog",
                    "params": {},
                    "dryRun": true
                }),
                shape: ResultShape::DryRun,
                result: json!({
                    "dryRun": true,
                    "plan": [
                        {
                            "operation": "acme.get_gift_catalog",
                            "params": {},
                            "tier": 0,
                            "description": "Read the gift catalog of Acme Live."
                        }
                    ],
                    "warnings": [],
                    "wouldRequireApproval": false,
                    "effectiveTier": 1
                }),
            },
        ],
    }
}
