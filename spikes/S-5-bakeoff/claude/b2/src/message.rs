//! The WebSocket envelope: `{ "metadata": {...}, "payload": {...} }`.
//!
//! <https://dev.twitch.tv/docs/eventsub/handling-websocket-events/>

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};

use crate::events::Event;

pub const SESSION_WELCOME: &str = "session_welcome";
pub const SESSION_KEEPALIVE: &str = "session_keepalive";
pub const SESSION_RECONNECT: &str = "session_reconnect";
pub const NOTIFICATION: &str = "notification";
pub const REVOCATION: &str = "revocation";

/// One EventSub WebSocket message.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Message {
    SessionWelcome {
        metadata: Metadata,
        payload: SessionPayload,
    },
    /// The payload is `{}` today; anything Twitch puts there later is kept.
    SessionKeepalive {
        metadata: Metadata,
        payload: Map<String, Value>,
    },
    SessionReconnect {
        metadata: Metadata,
        payload: SessionPayload,
    },
    Notification {
        metadata: Metadata,
        payload: Box<Notification>,
    },
    Revocation {
        metadata: Metadata,
        payload: RevocationPayload,
    },
    /// A `message_type` this crate does not know, kept verbatim.
    Unknown(Value),
    /// A known `message_type` whose envelope did not match the typed structs (for example a
    /// `notification` whose subscription lacks `cost`). Kept verbatim with the error.
    Malformed { error: String, raw: Value },
}

impl Message {
    pub fn metadata(&self) -> Option<&Metadata> {
        match self {
            Self::SessionWelcome { metadata, .. }
            | Self::SessionKeepalive { metadata, .. }
            | Self::SessionReconnect { metadata, .. }
            | Self::Notification { metadata, .. }
            | Self::Revocation { metadata, .. } => Some(metadata),
            Self::Unknown(_) | Self::Malformed { .. } => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Metadata {
    pub message_id: String,
    pub message_type: String,
    /// RFC 3339 with nanoseconds; kept as a string so it round-trips byte for byte.
    pub message_timestamp: String,
    /// Present on `notification` and `revocation` only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subscription_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subscription_version: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionPayload {
    pub session: Session,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    /// `connected` or `reconnecting` in the docs; kept open-ended.
    pub status: String,
    pub connected_at: String,
    /// `null` on `session_reconnect`.
    pub keepalive_timeout_seconds: Option<u64>,
    /// `null` on `session_welcome`. Use it as is; never modify it.
    pub reconnect_url: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RevocationPayload {
    pub subscription: Subscription,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Subscription {
    pub id: String,
    /// `enabled`, or the revocation reason (`user_removed`, `authorization_revoked`, ...).
    pub status: String,
    #[serde(rename = "type")]
    pub subscription_type: String,
    pub version: String,
    pub cost: u64,
    /// Condition keys differ per subscription type.
    pub condition: Map<String, Value>,
    pub transport: Transport,
    pub created_at: String,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Transport {
    /// `websocket`, `webhook` or `conduit`.
    pub method: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub callback: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A notification payload: the subscription plus its typed event.
///
/// The event is dispatched on `subscription.type` and `subscription.version` inside the
/// payload, which is also the shape webhook deliveries use.
#[derive(Debug, Clone, PartialEq)]
pub struct Notification {
    pub subscription: Subscription,
    pub event: Event,
    pub extra: Map<String, Value>,
}

#[derive(Deserialize)]
struct NotificationWire {
    subscription: Subscription,
    event: Value,
    #[serde(flatten)]
    extra: Map<String, Value>,
}

#[derive(Serialize)]
struct NotificationWireRef<'a> {
    subscription: &'a Subscription,
    event: &'a Event,
    #[serde(flatten)]
    extra: &'a Map<String, Value>,
}

impl<'de> Deserialize<'de> for Notification {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let wire = NotificationWire::deserialize(deserializer)?;
        let event = Event::parse(
            &wire.subscription.subscription_type,
            &wire.subscription.version,
            wire.event,
        );
        Ok(Self {
            subscription: wire.subscription,
            event,
            extra: wire.extra,
        })
    }
}

impl Serialize for Notification {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        NotificationWireRef {
            subscription: &self.subscription,
            event: &self.event,
            extra: &self.extra,
        }
        .serialize(serializer)
    }
}

#[derive(Deserialize)]
struct Envelope<P> {
    metadata: Metadata,
    payload: P,
}

#[derive(Serialize)]
struct EnvelopeRef<'a, P> {
    metadata: &'a Metadata,
    payload: &'a P,
}

impl<'de> Deserialize<'de> for Message {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = Value::deserialize(deserializer)?;
        let message_type = raw
            .get("metadata")
            .and_then(|metadata| metadata.get("message_type"))
            .and_then(Value::as_str);
        let Some(message_type) = message_type else {
            return Err(D::Error::custom("missing metadata.message_type"));
        };

        fn typed<P: for<'a> Deserialize<'a>>(
            raw: &Value,
            wrap: impl FnOnce(Metadata, P) -> Message,
        ) -> Result<Message, serde_json::Error> {
            Envelope::<P>::deserialize(raw)
                .map(|envelope| wrap(envelope.metadata, envelope.payload))
        }

        let result = match message_type {
            SESSION_WELCOME => typed(&raw, |metadata, payload| Self::SessionWelcome {
                metadata,
                payload,
            }),
            SESSION_KEEPALIVE => typed(&raw, |metadata, payload| Self::SessionKeepalive {
                metadata,
                payload,
            }),
            SESSION_RECONNECT => typed(&raw, |metadata, payload| Self::SessionReconnect {
                metadata,
                payload,
            }),
            NOTIFICATION => typed(&raw, |metadata, payload| Self::Notification {
                metadata,
                payload,
            }),
            REVOCATION => typed(&raw, |metadata, payload| Self::Revocation {
                metadata,
                payload,
            }),
            _ => return Ok(Self::Unknown(raw)),
        };
        Ok(result.unwrap_or_else(|error| Self::Malformed {
            error: error.to_string(),
            raw,
        }))
    }
}

impl Serialize for Message {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        fn envelope<S: Serializer, P: Serialize>(
            serializer: S,
            metadata: &Metadata,
            payload: &P,
        ) -> Result<S::Ok, S::Error> {
            EnvelopeRef { metadata, payload }.serialize(serializer)
        }

        match self {
            Self::SessionWelcome { metadata, payload }
            | Self::SessionReconnect { metadata, payload } => {
                envelope(serializer, metadata, payload)
            }
            Self::SessionKeepalive { metadata, payload } => envelope(serializer, metadata, payload),
            Self::Notification { metadata, payload } => envelope(serializer, metadata, payload),
            Self::Revocation { metadata, payload } => envelope(serializer, metadata, payload),
            Self::Unknown(raw) | Self::Malformed { raw, .. } => raw.serialize(serializer),
        }
    }
}
