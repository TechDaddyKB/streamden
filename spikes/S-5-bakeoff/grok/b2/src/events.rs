use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use crate::error::ParseError;

/// Event body of a `notification` frame.
///
/// Types the card names are structs. Anything else, including a known name on
/// a version this crate does not model, is [`EventBody::Raw`] so the payload
/// is kept.
#[derive(Debug, Clone, PartialEq)]
pub enum EventBody {
    ChatMessage(Box<ChatMessage>),
    Follow(Follow),
    Subscribe(Subscribe),
    SubscriptionGift(SubscriptionGift),
    SubscriptionMessage(SubscriptionMessage),
    Cheer(Cheer),
    Raid(Raid),
    RedemptionAdd(Box<RedemptionAdd>),
    HypeTrainBegin(Box<HypeTrainBegin>),
    Raw(Value),
}

impl EventBody {
    pub(crate) fn to_value(&self) -> Result<Value, ParseError> {
        Ok(match self {
            EventBody::ChatMessage(event) => serde_json::to_value(event)?,
            EventBody::Follow(event) => serde_json::to_value(event)?,
            EventBody::Subscribe(event) => serde_json::to_value(event)?,
            EventBody::SubscriptionGift(event) => serde_json::to_value(event)?,
            EventBody::SubscriptionMessage(event) => serde_json::to_value(event)?,
            EventBody::Cheer(event) => serde_json::to_value(event)?,
            EventBody::Raid(event) => serde_json::to_value(event)?,
            EventBody::RedemptionAdd(event) => serde_json::to_value(event)?,
            EventBody::HypeTrainBegin(event) => serde_json::to_value(event)?,
            EventBody::Raw(event) => event.clone(),
        })
    }
}

pub(crate) fn parse_event(
    type_name: &str,
    version: &str,
    event: Value,
) -> Result<EventBody, ParseError> {
    match (type_name, version) {
        ("channel.chat.message", _) => {
            Ok(EventBody::ChatMessage(Box::new(typed(type_name, event)?)))
        }
        // v1 and v2 publish the same event fields. v2 is the current subscription.
        ("channel.follow", "1" | "2") => Ok(EventBody::Follow(typed(type_name, event)?)),
        ("channel.subscribe", "1") => Ok(EventBody::Subscribe(typed(type_name, event)?)),
        ("channel.subscription.gift", "1") => {
            Ok(EventBody::SubscriptionGift(typed(type_name, event)?))
        }
        ("channel.subscription.message", "1") => {
            Ok(EventBody::SubscriptionMessage(typed(type_name, event)?))
        }
        ("channel.cheer", "1") => Ok(EventBody::Cheer(typed(type_name, event)?)),
        ("channel.raid", "1") => Ok(EventBody::Raid(typed(type_name, event)?)),
        ("channel.channel_points_custom_reward_redemption.add", "1") => {
            Ok(EventBody::RedemptionAdd(Box::new(typed(type_name, event)?)))
        }
        // Version 2 is the current Hype Train begin schema (v1 was removed).
        ("channel.hype_train.begin", "2") => Ok(EventBody::HypeTrainBegin(Box::new(typed(
            type_name, event,
        )?))),
        _ => Ok(EventBody::Raw(event)),
    }
}

fn typed<T: DeserializeOwned>(type_name: &str, event: Value) -> Result<T, ParseError> {
    serde_json::from_value(event).map_err(|source| ParseError::Event {
        type_name: type_name.to_owned(),
        source,
    })
}

/// A JSON field that may be omitted, explicit `null`, or a value.
///
/// `None` is omitted on the way back out. `Some(None)` is `null`. `Some(Some)`
/// is the value. Chat examples use both shapes for the shared-chat fields.
/// Pair it with [`nullable::deserialize`]: a bare `Option<Option<T>>` turns
/// JSON `null` into `None` and then drops the field.
pub type Nullable<T> = Option<Option<T>>;

/// Serde helper for [`Nullable`].
pub mod nullable {
    use super::{Deserialize, Deserializer};

    /// Present field, including JSON `null`, becomes `Some`. Absence uses
    /// `#[serde(default)]` and stays `None`.
    pub fn deserialize<'de, T, D>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
    where
        T: Deserialize<'de>,
        D: Deserializer<'de>,
    {
        Ok(Some(Option::deserialize(deserializer)?))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChatMessage {
    pub broadcaster_user_id: String,
    pub broadcaster_user_login: String,
    pub broadcaster_user_name: String,
    pub chatter_user_id: String,
    pub chatter_user_login: String,
    pub chatter_user_name: String,
    pub message_id: String,
    pub message: ChatMessageBody,
    pub color: String,
    pub badges: Vec<Badge>,
    pub message_type: String,
    pub cheer: Option<CheerFragment>,
    pub reply: Option<Reply>,
    pub channel_points_custom_reward_id: Option<String>,
    #[serde(
        default,
        deserialize_with = "nullable::deserialize",
        skip_serializing_if = "Option::is_none"
    )]
    pub source_broadcaster_user_id: Nullable<String>,
    #[serde(
        default,
        deserialize_with = "nullable::deserialize",
        skip_serializing_if = "Option::is_none"
    )]
    pub source_broadcaster_user_login: Nullable<String>,
    #[serde(
        default,
        deserialize_with = "nullable::deserialize",
        skip_serializing_if = "Option::is_none"
    )]
    pub source_broadcaster_user_name: Nullable<String>,
    #[serde(
        default,
        deserialize_with = "nullable::deserialize",
        skip_serializing_if = "Option::is_none"
    )]
    pub source_message_id: Nullable<String>,
    #[serde(
        default,
        deserialize_with = "nullable::deserialize",
        skip_serializing_if = "Option::is_none"
    )]
    pub source_badges: Nullable<Vec<Badge>>,
    #[serde(
        default,
        deserialize_with = "nullable::deserialize",
        skip_serializing_if = "Option::is_none"
    )]
    pub is_source_only: Nullable<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChatMessageBody {
    pub text: String,
    pub fragments: Vec<Fragment>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fragment {
    #[serde(rename = "type")]
    pub fragment_type: String,
    pub text: String,
    pub cheermote: Option<Cheermote>,
    pub emote: Option<Emote>,
    pub mention: Option<Mention>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cheermote {
    pub prefix: String,
    pub bits: i64,
    pub tier: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Emote {
    pub id: String,
    pub emote_set_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mention {
    pub user_id: String,
    pub user_login: String,
    pub user_name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Badge {
    pub set_id: String,
    pub id: String,
    pub info: String,
}

/// Bits attached to a chat message. Distinct from a [`Cheermote`] fragment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheerFragment {
    pub bits: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reply {
    pub parent_message_id: String,
    pub parent_message_body: String,
    pub parent_user_id: String,
    pub parent_user_name: String,
    pub parent_user_login: String,
    pub thread_message_id: String,
    pub thread_user_id: String,
    pub thread_user_name: String,
    pub thread_user_login: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Follow {
    pub user_id: String,
    pub user_login: String,
    pub user_name: String,
    pub broadcaster_user_id: String,
    pub broadcaster_user_login: String,
    pub broadcaster_user_name: String,
    pub followed_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Subscribe {
    pub user_id: String,
    pub user_login: String,
    pub user_name: String,
    pub broadcaster_user_id: String,
    pub broadcaster_user_login: String,
    pub broadcaster_user_name: String,
    pub tier: String,
    pub is_gift: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubscriptionGift {
    pub user_id: Option<String>,
    pub user_login: Option<String>,
    pub user_name: Option<String>,
    pub broadcaster_user_id: String,
    pub broadcaster_user_login: String,
    pub broadcaster_user_name: String,
    pub total: i64,
    pub tier: String,
    pub cumulative_total: Option<i64>,
    pub is_anonymous: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubscriptionMessage {
    pub user_id: String,
    pub user_login: String,
    pub user_name: String,
    pub broadcaster_user_id: String,
    pub broadcaster_user_login: String,
    pub broadcaster_user_name: String,
    pub tier: String,
    pub message: ResubMessage,
    pub cumulative_months: i64,
    pub streak_months: Option<i64>,
    pub duration_months: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResubMessage {
    pub text: String,
    pub emotes: Vec<ResubEmote>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResubEmote {
    pub begin: i64,
    pub end: i64,
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cheer {
    pub is_anonymous: bool,
    pub user_id: Option<String>,
    pub user_login: Option<String>,
    pub user_name: Option<String>,
    pub broadcaster_user_id: String,
    pub broadcaster_user_login: String,
    pub broadcaster_user_name: String,
    pub message: String,
    pub bits: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Raid {
    pub from_broadcaster_user_id: String,
    pub from_broadcaster_user_login: String,
    pub from_broadcaster_user_name: String,
    pub to_broadcaster_user_id: String,
    pub to_broadcaster_user_login: String,
    pub to_broadcaster_user_name: String,
    pub viewers: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RedemptionAdd {
    pub id: String,
    pub broadcaster_user_id: String,
    pub broadcaster_user_login: String,
    pub broadcaster_user_name: String,
    pub user_id: String,
    pub user_login: String,
    pub user_name: String,
    pub user_input: String,
    pub status: String,
    pub reward: RedeemedReward,
    pub redeemed_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RedeemedReward {
    pub id: String,
    pub title: String,
    pub cost: i64,
    pub prompt: String,
}

/// `channel.hype_train.begin` version 2, the current schema.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HypeTrainBegin {
    pub id: String,
    pub broadcaster_user_id: String,
    pub broadcaster_user_login: String,
    pub broadcaster_user_name: String,
    pub total: i64,
    pub progress: i64,
    pub goal: i64,
    pub top_contributions: Vec<Contribution>,
    /// Null in the published begin example. Participant objects are kept as
    /// JSON so an unseen shape is not rejected.
    pub shared_train_participants: Option<Vec<Value>>,
    pub level: i64,
    pub started_at: String,
    pub expires_at: String,
    pub is_shared_train: bool,
    #[serde(rename = "type")]
    pub train_type: String,
    pub all_time_high_level: i64,
    pub all_time_high_total: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Contribution {
    pub user_id: String,
    pub user_login: String,
    pub user_name: String,
    #[serde(rename = "type")]
    pub contribution_type: String,
    pub total: i64,
}
