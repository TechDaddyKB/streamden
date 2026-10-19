//! Typed notification events.
//!
//! Field lists follow <https://dev.twitch.tv/docs/eventsub/eventsub-reference/>. Timestamps
//! stay strings so they round-trip exactly (Twitch sends nanosecond RFC 3339). Nullable
//! fields are `Option` and serialize as `null`; a field that is sometimes omitted entirely
//! is skipped when `None`.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize, Serializer};
use serde_json::{Map, Value};

/// A notification event, typed when this crate knows its subscription type and version.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Event {
    /// `channel.chat.message` v1
    ChatMessage(Box<ChatMessageV1>),
    /// `channel.follow` v2
    Follow(FollowV2),
    /// `channel.subscribe` v1
    Subscribe(SubscribeV1),
    /// `channel.subscription.gift` v1
    SubscriptionGift(SubscriptionGiftV1),
    /// `channel.subscription.message` v1
    SubscriptionMessage(SubscriptionMessageV1),
    /// `channel.cheer` v1
    Cheer(CheerV1),
    /// `channel.raid` v1
    Raid(RaidV1),
    /// `channel.channel_points_custom_reward_redemption.add` v1
    RewardRedemptionAdd(RewardRedemptionAddV1),
    /// `channel.hype_train.begin` v2
    HypeTrainBegin(Box<HypeTrainBeginV2>),
    /// Any other type or version, or a known one that failed typed parsing.
    Raw(RawEvent),
}

/// An event kept as JSON.
#[derive(Debug, Clone, PartialEq)]
pub struct RawEvent {
    pub subscription_type: String,
    pub version: String,
    pub event: Value,
    /// Set when the type and version are known but the event did not match the typed
    /// struct (a Twitch schema change, or a malformed payload). Not serialized.
    pub parse_error: Option<String>,
}

impl Event {
    /// Parses `event` for the given subscription type and version. Never fails: anything
    /// that is not a known, well-formed event becomes [`Event::Raw`].
    pub fn parse(subscription_type: &str, version: &str, event: Value) -> Self {
        fn typed<T: DeserializeOwned>(
            event: &Value,
            wrap: impl FnOnce(T) -> Event,
        ) -> Result<Event, serde_json::Error> {
            T::deserialize(event).map(wrap)
        }

        let result = match (subscription_type, version) {
            ("channel.chat.message", "1") => typed(&event, |e| Self::ChatMessage(Box::new(e))),
            ("channel.follow", "2") => typed(&event, Self::Follow),
            ("channel.subscribe", "1") => typed(&event, Self::Subscribe),
            ("channel.subscription.gift", "1") => typed(&event, Self::SubscriptionGift),
            ("channel.subscription.message", "1") => typed(&event, Self::SubscriptionMessage),
            ("channel.cheer", "1") => typed(&event, Self::Cheer),
            ("channel.raid", "1") => typed(&event, Self::Raid),
            ("channel.channel_points_custom_reward_redemption.add", "1") => {
                typed(&event, Self::RewardRedemptionAdd)
            }
            ("channel.hype_train.begin", "2") => {
                typed(&event, |e| Self::HypeTrainBegin(Box::new(e)))
            }
            _ => return Self::raw(subscription_type, version, event, None),
        };
        result.unwrap_or_else(|error| {
            Self::raw(subscription_type, version, event, Some(error.to_string()))
        })
    }

    fn raw(subscription_type: &str, version: &str, event: Value, error: Option<String>) -> Self {
        Self::Raw(RawEvent {
            subscription_type: subscription_type.to_owned(),
            version: version.to_owned(),
            event,
            parse_error: error,
        })
    }

    /// Top-level event fields the typed struct did not recognise (`None` for [`Event::Raw`]).
    pub fn extra(&self) -> Option<&Map<String, Value>> {
        Some(match self {
            Self::ChatMessage(e) => &e.extra,
            Self::Follow(e) => &e.extra,
            Self::Subscribe(e) => &e.extra,
            Self::SubscriptionGift(e) => &e.extra,
            Self::SubscriptionMessage(e) => &e.extra,
            Self::Cheer(e) => &e.extra,
            Self::Raid(e) => &e.extra,
            Self::RewardRedemptionAdd(e) => &e.extra,
            Self::HypeTrainBegin(e) => &e.extra,
            Self::Raw(_) => return None,
        })
    }
}

impl Serialize for Event {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::ChatMessage(e) => e.serialize(serializer),
            Self::Follow(e) => e.serialize(serializer),
            Self::Subscribe(e) => e.serialize(serializer),
            Self::SubscriptionGift(e) => e.serialize(serializer),
            Self::SubscriptionMessage(e) => e.serialize(serializer),
            Self::Cheer(e) => e.serialize(serializer),
            Self::Raid(e) => e.serialize(serializer),
            Self::RewardRedemptionAdd(e) => e.serialize(serializer),
            Self::HypeTrainBegin(e) => e.serialize(serializer),
            Self::Raw(e) => e.event.serialize(serializer),
        }
    }
}

string_enum! {
    /// Subscription tier.
    SubTier { Tier1 => "1000", Tier2 => "2000", Tier3 => "3000" }
}

string_enum! {
    ChatMessageType {
        Text => "text",
        ChannelPointsHighlighted => "channel_points_highlighted",
        ChannelPointsSubOnly => "channel_points_sub_only",
        UserIntro => "user_intro",
        PowerUpsMessageEffect => "power_ups_message_effect",
        PowerUpsGigantifiedEmote => "power_ups_gigantified_emote",
    }
}

string_enum! {
    FragmentType {
        Text => "text",
        Cheermote => "cheermote",
        Emote => "emote",
        Mention => "mention",
        Gif => "gif",
    }
}

string_enum! {
    RedemptionStatus {
        Unknown => "unknown",
        Unfulfilled => "unfulfilled",
        Fulfilled => "fulfilled",
        Canceled => "canceled",
    }
}

string_enum! {
    HypeTrainType { Regular => "regular", GoldenKappa => "golden_kappa", Treasure => "treasure" }
}

string_enum! {
    ContributionType { Bits => "bits", Subscription => "subscription", OtherMethod => "other" }
}

// --- channel.chat.message v1 -------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatMessageV1 {
    pub broadcaster_user_id: String,
    pub broadcaster_user_login: String,
    pub broadcaster_user_name: String,
    pub chatter_user_id: String,
    pub chatter_user_login: String,
    pub chatter_user_name: String,
    pub message_id: String,
    pub message: ChatMessageBody,
    pub message_type: ChatMessageType,
    pub badges: Vec<Badge>,
    pub cheer: Option<ChatCheer>,
    /// `#RRGGBB`, or empty if the user never set one.
    pub color: String,
    pub reply: Option<ChatReply>,
    pub channel_points_custom_reward_id: Option<String>,
    /// Shared chat: `null` when the message was sent in this broadcaster's channel.
    pub source_broadcaster_user_id: Option<String>,
    pub source_broadcaster_user_login: Option<String>,
    pub source_broadcaster_user_name: Option<String>,
    pub source_message_id: Option<String>,
    pub source_badges: Option<Vec<Badge>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_source_only: Option<bool>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatMessageBody {
    pub text: String,
    pub fragments: Vec<ChatFragment>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatFragment {
    #[serde(rename = "type")]
    pub fragment_type: FragmentType,
    pub text: String,
    pub cheermote: Option<Cheermote>,
    pub emote: Option<FragmentEmote>,
    pub mention: Option<Mention>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gif: Option<Gif>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Cheermote {
    pub prefix: String,
    pub bits: u64,
    pub tier: u64,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FragmentEmote {
    pub id: String,
    pub emote_set_id: String,
    pub owner_id: String,
    /// `static` and/or `animated`.
    pub format: Vec<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Mention {
    pub user_id: String,
    pub user_name: String,
    pub user_login: String,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Gif {
    pub id: String,
    /// Use as is; Twitch forbids modifying it.
    pub url: String,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Badge {
    pub set_id: String,
    pub id: String,
    pub info: String,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatCheer {
    pub bits: u64,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatReply {
    pub parent_message_id: String,
    pub parent_message_body: String,
    pub parent_user_id: String,
    pub parent_user_name: String,
    pub parent_user_login: String,
    pub thread_message_id: String,
    pub thread_user_id: String,
    pub thread_user_name: String,
    pub thread_user_login: String,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

// --- channel.follow v2 -------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FollowV2 {
    pub user_id: String,
    pub user_login: String,
    pub user_name: String,
    pub broadcaster_user_id: String,
    pub broadcaster_user_login: String,
    pub broadcaster_user_name: String,
    pub followed_at: String,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

// --- channel.subscribe v1 ----------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SubscribeV1 {
    pub user_id: String,
    pub user_login: String,
    pub user_name: String,
    pub broadcaster_user_id: String,
    pub broadcaster_user_login: String,
    pub broadcaster_user_name: String,
    pub tier: SubTier,
    pub is_gift: bool,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

// --- channel.subscription.gift v1 --------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SubscriptionGiftV1 {
    /// `null` when `is_anonymous`.
    pub user_id: Option<String>,
    pub user_login: Option<String>,
    pub user_name: Option<String>,
    pub broadcaster_user_id: String,
    pub broadcaster_user_login: String,
    pub broadcaster_user_name: String,
    pub total: u64,
    pub tier: SubTier,
    /// `null` if anonymous or not shared by the user.
    pub cumulative_total: Option<u64>,
    pub is_anonymous: bool,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

// --- channel.subscription.message v1 -----------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SubscriptionMessageV1 {
    pub user_id: String,
    pub user_login: String,
    pub user_name: String,
    pub broadcaster_user_id: String,
    pub broadcaster_user_login: String,
    pub broadcaster_user_name: String,
    pub tier: SubTier,
    pub message: ResubMessage,
    pub cumulative_months: u64,
    /// `null` if the user does not share it.
    pub streak_months: Option<u64>,
    pub duration_months: u64,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResubMessage {
    pub text: String,
    pub emotes: Option<Vec<ResubEmote>>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResubEmote {
    pub begin: u64,
    pub end: u64,
    pub id: String,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

// --- channel.cheer v1 --------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CheerV1 {
    pub is_anonymous: bool,
    /// `null` when `is_anonymous`.
    pub user_id: Option<String>,
    pub user_login: Option<String>,
    pub user_name: Option<String>,
    pub broadcaster_user_id: String,
    pub broadcaster_user_login: String,
    pub broadcaster_user_name: String,
    pub message: String,
    pub bits: u64,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

// --- channel.raid v1 ---------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RaidV1 {
    pub from_broadcaster_user_id: String,
    pub from_broadcaster_user_login: String,
    pub from_broadcaster_user_name: String,
    pub to_broadcaster_user_id: String,
    pub to_broadcaster_user_login: String,
    pub to_broadcaster_user_name: String,
    pub viewers: u64,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

// --- channel.channel_points_custom_reward_redemption.add v1 ------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RewardRedemptionAddV1 {
    pub id: String,
    pub broadcaster_user_id: String,
    pub broadcaster_user_login: String,
    pub broadcaster_user_name: String,
    pub user_id: String,
    pub user_login: String,
    pub user_name: String,
    /// Empty if the reward takes no input.
    pub user_input: String,
    pub status: RedemptionStatus,
    pub reward: Reward,
    pub redeemed_at: String,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Reward {
    pub id: String,
    pub title: String,
    pub cost: u64,
    pub prompt: String,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

// --- channel.hype_train.begin v2 ---------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HypeTrainBeginV2 {
    pub id: String,
    pub broadcaster_user_id: String,
    pub broadcaster_user_login: String,
    pub broadcaster_user_name: String,
    pub total: u64,
    pub progress: u64,
    pub goal: u64,
    pub top_contributions: Vec<Contribution>,
    /// Non-null only for a shared Hype Train.
    pub shared_train_participants: Option<Vec<SharedTrainParticipant>>,
    pub level: u64,
    pub started_at: String,
    pub expires_at: String,
    pub is_shared_train: bool,
    #[serde(rename = "type")]
    pub train_type: HypeTrainType,
    pub all_time_high_level: u64,
    pub all_time_high_total: u64,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Contribution {
    pub user_id: String,
    pub user_login: String,
    pub user_name: String,
    #[serde(rename = "type")]
    pub contribution_type: ContributionType,
    /// Bits for `bits`; 500, 1000 or 2500 for tier 1, 2 or 3 subscriptions.
    pub total: u64,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SharedTrainParticipant {
    pub broadcaster_user_id: String,
    pub broadcaster_user_login: String,
    pub broadcaster_user_name: String,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}
