//! StreamDen protocol and contracts (Spec 02, `docs/spec/02-protocol.md`).
//!
//! Every public wire and storage type lives here. Changing this crate needs an ADR and a dedicated
//! contract card (AGENTS.md, single-writer files). This crate holds types, the operation catalog,
//! schema generation and validation helpers only: no engine logic, no I/O beyond what
//! `cargo xtask codegen` asks for through [`codegen`].
//!
//! Conventions (Spec 02 §3): every public type derives `Serialize`, `Deserialize`, `JsonSchema` and
//! `TS`, field names are `camelCase` on the wire, enum values are `snake_case`, domain unions are
//! internally tagged by `kind`, and absent means "not provided".

pub mod auth;
pub mod catalog;
pub mod codegen;
pub mod core;
pub mod domain;
pub mod events;
pub mod ids;
pub mod manifest;
pub mod records;
pub mod results;
pub mod rpc;
pub mod schema;
pub mod secret;
pub mod shared;
mod tagged;
pub mod template;
pub mod timestamp;
pub mod version;

pub use auth::{
    ApprovalBinding, ApprovedPlan, AuthContext, ChainOrigin, ClientInfo, PlannedCall, Principal,
    Surface, TokenKind,
};
pub use catalog::{
    ArgWrites, Catalog, CatalogError, Deprecation, EventDef, Exposure, FilterableField, L10nString,
    OperationDef, OperationExample, ResultShape,
};
pub use domain::{
    Action, ActionRef, Case, CodeLang, CodeSource, CompareOp, Condition, Filter, FilterOp,
    HistoryPolicy, Node, OnError, Origin, Overflow, PendingChange, Queue, RunMode, StoredDocument,
    SubAction, Trigger, UpdatedBy, VariableRecord,
};
pub use events::{Event, EventMeta, SourceRef};
pub use ids::{
    ActionId, ApprovalId, CommandId, ConnectionId, EntityId, EventId, EventType, GroupId, LocalId,
    NodeId, OperationName, OverlayId, PlatformId, PluginId, QueueId, RunId, Scope, SessionId,
    Sha256, Slug, TimerId, TokenId, TopicGlob, TriggerId, VariableName,
};
pub use manifest::PluginManifest;
pub use records::{
    CancelReason, Command, CommandLocation, CommandMode, Cooldown, CounterPolicy, PermissionMode,
    Permissions, Run, RunError, RunState, RunWarning, Timer, UserKey,
};
pub use results::{
    ActionRevisionRef, DraftKind, DraftRef, DraftResult, DryRunResult, PlanStep, PluginCallParams,
    PluginCallResult,
};
pub use secret::{Secret, expose_secrets, withhold_secrets};
pub use shared::{
    Badge, ChatText, ContributionRef, Fragment, ImageRef, Money, Os, RiskTier, UnknownEventPayload,
    UntrustedString, UserRef,
};
pub use template::{Template, TemplateOr};
pub use timestamp::Timestamp;

/// Version of this crate, reported by binaries in their startup log.
pub const CRATE_VERSION: &str = env!("CARGO_PKG_VERSION");
