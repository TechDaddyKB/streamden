//! The event envelope (Spec 02 §5.1).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::ids::{ConnectionId, EventId, EventType, PlatformId, RunId};
use crate::shared::UserRef;
use crate::timestamp::Timestamp;

/// One occurrence from a source. Immutable once emitted. `payload` is an opaque object on the wire;
/// the catalog maps each `type` to its payload schema and the generated helpers parse on demand (§5.2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    /// UUIDv7, monotonic per process; the replay cursor (§8.4).
    pub id: EventId,
    /// The event type, `<slug>.<name>` (`twitch.subscription`).
    #[serde(rename = "type")]
    pub r#type: EventType,
    /// When it happened upstream, or the emission time if unknown.
    pub ts: Timestamp,
    /// Where it came from.
    pub source: SourceRef,
    /// The platform (open enum): `twitch`, `youtube`, `kick`, `x`, `bluesky`, or a plugin slug.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub platform: Option<PlatformId>,
    /// The actor, normalised.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub user: Option<UserRef>,
    /// The typed payload, an object whose schema the catalog publishes under `events.<type>.payload`.
    #[ts(as = "serde_json::Value")]
    pub payload: serde_json::Value,
    /// Delivery metadata.
    pub meta: EventMeta,
}

/// Where an event came from (§5.1).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct SourceRef {
    /// `core.<connector>` for built-in sources (`core.twitch`) or the plugin ID for plugin sources.
    pub plugin: String,
    /// The connection the event arrived on, when the source has connections.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub connection: Option<ConnectionId>,
    /// The account on that connection (a broadcaster or bot account), when relevant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub account: Option<String>,
}

/// Delivery metadata of an event (§5.1).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct EventMeta {
    /// Schema version of this event type's payload, starting at 1 (§5.2). States the shape actually delivered.
    pub payload_version: u32,
    /// Emitted by the simulator or "Test trigger".
    #[serde(default)]
    pub test: bool,
    /// Re-delivered from the replay buffer or from history (§8.4).
    #[serde(default)]
    pub replay: bool,
    /// The run that caused this event, including the synthetic run of a direct operation call. Server-assigned,
    /// never settable by clients, connectors or plugins (§8.8 rule 5, §16).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub caused_by: Option<RunId>,
    /// Upstream message ID, for de-duplication and moderation links.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub correlation_id: Option<String>,
}

impl Default for EventMeta {
    fn default() -> Self {
        Self {
            payload_version: 1,
            test: false,
            replay: false,
            caused_by: None,
            correlation_id: None,
        }
    }
}

impl Event {
    /// Parses `payload` as `T`. Does not check that `T` matches `type`; use the catalog for that.
    pub fn payload_as<T: serde::de::DeserializeOwned>(&self) -> Result<T, serde_json::Error> {
        serde_json::from_value(self.payload.clone())
    }
}
