//! Typed parser for Twitch EventSub WebSocket text frames.
//!
//! Session frames (`session_welcome`, `session_keepalive`, `session_reconnect`,
//! `revocation`, `notification`) become structs. Notification types named by
//! DEN-9 become event structs. Any other message type or subscription type is
//! kept as JSON and still round-trips.

mod error;
mod events;
mod wire;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use error::ParseError;
pub use events::{
    Badge, ChatMessage, ChatMessageBody, Cheer, CheerFragment, Cheermote, Contribution, Emote,
    EventBody, Follow, Fragment, HypeTrainBegin, Mention, Raid, RedeemedReward, RedemptionAdd,
    Reply, ResubEmote, ResubMessage, Subscribe, SubscriptionGift, SubscriptionMessage,
};
pub use wire::{
    Metadata, Revocation, Session, SessionKeepalive, SessionReconnect, SessionWelcome,
    Subscription, Transport,
};

use events::parse_event;
use wire::NotificationWire;

/// One EventSub WebSocket text frame.
#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    SessionWelcome(SessionWelcome),
    SessionKeepalive(SessionKeepalive),
    SessionReconnect(SessionReconnect),
    Notification(Box<Notification>),
    Revocation(Revocation),
    /// `metadata.message_type` was not one of the five session frames.
    /// The original JSON object is unchanged.
    Unknown(Value),
}

/// A `notification` frame. `event` is typed when this crate knows the
/// subscription type and version, and raw JSON otherwise.
#[derive(Debug, Clone, PartialEq)]
pub struct Notification {
    pub metadata: Metadata,
    pub subscription: Subscription,
    pub event: EventBody,
}

impl Message {
    /// `metadata.message_type`, when the frame has one.
    pub fn message_type(&self) -> Option<&str> {
        match self {
            Message::SessionWelcome(message) => Some(message.metadata.message_type.as_str()),
            Message::SessionKeepalive(message) => Some(message.metadata.message_type.as_str()),
            Message::SessionReconnect(message) => Some(message.metadata.message_type.as_str()),
            Message::Notification(message) => Some(message.metadata.message_type.as_str()),
            Message::Revocation(message) => Some(message.metadata.message_type.as_str()),
            Message::Unknown(raw) => raw
                .get("metadata")
                .and_then(|metadata| metadata.get("message_type"))
                .and_then(Value::as_str),
        }
    }

    fn to_value(&self) -> Result<Value, ParseError> {
        match self {
            Message::SessionWelcome(message) => to_json(message),
            Message::SessionKeepalive(message) => to_json(message),
            Message::SessionReconnect(message) => to_json(message),
            Message::Revocation(message) => to_json(message),
            Message::Unknown(raw) => Ok(raw.clone()),
            Message::Notification(message) => notification_value(message),
        }
    }
}

impl Serialize for Message {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.to_value()
            .map_err(serde::ser::Error::custom)?
            .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Message {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = Value::deserialize(deserializer)?;
        parse_value(value).map_err(serde::de::Error::custom)
    }
}

/// Parse one WebSocket text frame.
pub fn parse(input: &str) -> Result<Message, ParseError> {
    parse_value(serde_json::from_str(input)?)
}

/// Parse one WebSocket text frame from bytes.
pub fn parse_slice(input: &[u8]) -> Result<Message, ParseError> {
    parse_value(serde_json::from_slice(input)?)
}

fn parse_value(value: Value) -> Result<Message, ParseError> {
    let message_type = value
        .as_object()
        .ok_or(ParseError::ExpectedObject)?
        .get("metadata")
        .and_then(|metadata| metadata.get("message_type"))
        .and_then(Value::as_str)
        .ok_or(ParseError::MissingField("metadata.message_type"))?
        .to_owned();

    match message_type.as_str() {
        "session_welcome" => Ok(Message::SessionWelcome(from_json(value)?)),
        "session_keepalive" => Ok(Message::SessionKeepalive(from_json(value)?)),
        "session_reconnect" => Ok(Message::SessionReconnect(from_json(value)?)),
        "revocation" => Ok(Message::Revocation(from_json(value)?)),
        "notification" => Ok(Message::Notification(parse_notification(value)?)),
        _ => Ok(Message::Unknown(value)),
    }
}

fn parse_notification(value: Value) -> Result<Box<Notification>, ParseError> {
    let wire: NotificationWire = from_json(value)?;
    let type_name = wire.payload.subscription.type_name.clone();
    let version = wire.payload.subscription.version.clone();
    let event = parse_event(&type_name, &version, wire.payload.event)?;
    Ok(Box::new(Notification {
        metadata: wire.metadata,
        subscription: wire.payload.subscription,
        event,
    }))
}

fn notification_value(message: &Notification) -> Result<Value, ParseError> {
    Ok(serde_json::json!({
        "metadata": message.metadata,
        "payload": {
            "subscription": message.subscription,
            "event": message.event.to_value()?,
        }
    }))
}

fn from_json<T: DeserializeOwned>(value: Value) -> Result<T, ParseError> {
    Ok(serde_json::from_value(value)?)
}

fn to_json<T: Serialize>(value: &T) -> Result<Value, ParseError> {
    Ok(serde_json::to_value(value)?)
}

/// Subscription type name for a notification or revocation, if this frame has one.
pub fn subscription_type(message: &Message) -> Option<&str> {
    match message {
        Message::Notification(notification) => Some(notification.subscription.type_name.as_str()),
        Message::Revocation(revocation) => Some(revocation.payload.subscription.type_name.as_str()),
        Message::SessionWelcome(_)
        | Message::SessionKeepalive(_)
        | Message::SessionReconnect(_)
        | Message::Unknown(_) => None,
    }
}
