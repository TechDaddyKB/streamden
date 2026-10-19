use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// `metadata` object on every EventSub WebSocket text frame.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Metadata {
    pub message_id: String,
    pub message_type: String,
    pub message_timestamp: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subscription_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subscription_version: Option<String>,
}

/// How this subscription is delivered. Notification examples use webhook,
/// WebSocket, or conduit, so only `method` is required.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Transport {
    pub method: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub callback: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conduit_id: Option<String>,
}

/// Subscription object carried on `notification` and `revocation` frames.
///
/// `cost` is optional because some published examples omit it. Condition keys
/// differ by subscription type, so they stay a JSON object.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Subscription {
    pub id: String,
    pub status: String,
    #[serde(rename = "type")]
    pub type_name: String,
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost: Option<i64>,
    pub condition: Map<String, Value>,
    pub transport: Transport,
    pub created_at: String,
}

/// `session` object inside welcome and reconnect frames.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Session {
    pub id: String,
    pub status: String,
    pub connected_at: String,
    /// Present as a number on welcome and JSON `null` on reconnect.
    pub keepalive_timeout_seconds: Option<i64>,
    /// JSON `null` until Twitch sends a reconnect frame.
    pub reconnect_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionPayload {
    pub session: Session,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionWelcome {
    pub metadata: Metadata,
    pub payload: SessionPayload,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionReconnect {
    pub metadata: Metadata,
    pub payload: SessionPayload,
}

/// Keepalive payload is an empty object in the published example. A map keeps
/// any future keys instead of dropping them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionKeepalive {
    pub metadata: Metadata,
    pub payload: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RevocationPayload {
    pub subscription: Subscription,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Revocation {
    pub metadata: Metadata,
    pub payload: RevocationPayload,
}

/// Wire shape of a notification before the event body is interpreted.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NotificationWire {
    pub metadata: Metadata,
    pub payload: NotificationPayloadWire,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NotificationPayloadWire {
    pub subscription: Subscription,
    pub event: Value,
}
