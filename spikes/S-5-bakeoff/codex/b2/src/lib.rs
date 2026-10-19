//! Immutable, lossless JSON messages with typed EventSub WebSocket payload views.
use serde::{de::Error, Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

/// The original JSON is retained so new fields and absent/null distinctions survive.
/// Typed views are immutable; serialization always emits the original document.
#[derive(Debug, Clone, PartialEq)]
pub struct Message {
    raw: Value,
    metadata: Metadata,
    payload: Payload,
}
impl Message {
    pub fn metadata(&self) -> &Metadata {
        &self.metadata
    }
    pub fn payload(&self) -> &Payload {
        &self.payload
    }
    pub fn raw(&self) -> &Value {
        &self.raw
    }
}
impl Serialize for Message {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.raw.serialize(serializer)
    }
}
impl<'de> Deserialize<'de> for Message {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = Value::deserialize(deserializer)?;
        let envelope: Envelope = serde_json::from_value(raw.clone()).map_err(D::Error::custom)?;
        let metadata = envelope.metadata;
        let body = Value::Object(envelope.payload);
        let decode = || -> serde_json::Result<Payload> {
            Ok(match metadata.message_type.as_str() {
                "session_welcome" => Payload::Welcome(
                    serde_json::from_value::<SessionPayload>(body.clone())?.session,
                ),
                "session_reconnect" => Payload::Reconnect(
                    serde_json::from_value::<SessionPayload>(body.clone())?.session,
                ),
                "session_keepalive" => Payload::Keepalive,
                "notification" | "revocation" => {
                    let subscription: Subscription =
                        serde_json::from_value(body["subscription"].clone())?;
                    if metadata.subscription_type.as_deref() != Some(&subscription.kind)
                        || metadata.subscription_version.as_deref() != Some(&subscription.version)
                    {
                        return Err(serde_json::Error::custom(
                            "metadata and subscription type/version disagree",
                        ));
                    }
                    if metadata.message_type == "revocation" {
                        Payload::Revocation(subscription)
                    } else {
                        let event = body
                            .get("event")
                            .ok_or_else(|| serde_json::Error::custom("missing event"))?;
                        Payload::Notification {
                            event: Event::decode(
                                &subscription.kind,
                                &subscription.version,
                                event.clone(),
                            )?,
                            subscription,
                        }
                    }
                }
                _ => Payload::Unknown(body.clone()),
            })
        };
        let payload = decode().map_err(D::Error::custom)?;
        Ok(Self {
            raw,
            metadata,
            payload,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
struct Envelope {
    metadata: Metadata,
    payload: serde_json::Map<String, Value>,
}
#[derive(Debug, Clone, PartialEq)]
pub enum Payload {
    Welcome(Session),
    Keepalive,
    Reconnect(Session),
    Notification {
        subscription: Subscription,
        event: Event,
    },
    Revocation(Subscription),
    Unknown(Value),
}

// These are typed read-only projections. Unknown fields remain in Message::raw().
macro_rules! data {
    ($name:ident { $($field:ident : $ty:ty),* $(,)? }) => {
        #[derive(Debug, Clone, PartialEq, Deserialize)]
        pub struct $name { $(pub $field: $ty),* }
    };
}
data!(Metadata {
    message_id: String, message_type: String, message_timestamp: String,
    subscription_type: Option<String>, subscription_version: Option<String>
});
data!(Session {
    id: String, status: String, connected_at: String,
    keepalive_timeout_seconds: Option<u64>, reconnect_url: Option<String>
});
data!(SessionPayload { session: Session });
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Subscription {
    pub id: String,
    pub status: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub version: String,
    pub cost: u64,
    pub condition: serde_json::Map<String, Value>,
    pub transport: Transport,
    pub created_at: String,
}
data!(Transport {
    method: String,
    session_id: String
});
data!(Broadcaster {
    broadcaster_user_id: String,
    broadcaster_user_login: String,
    broadcaster_user_name: String
});
data!(User {
    user_id: String,
    user_login: String,
    user_name: String
});
data!(AnonymousUser { user_id: Option<String>, user_login: Option<String>, user_name: Option<String> });

macro_rules! event_data {
    ($name:ident, $user:ty, { $($field:ident : $ty:ty),* $(,)? }) => {
        #[derive(Debug, Clone, PartialEq, Deserialize)]
        pub struct $name {
            #[serde(flatten)] pub broadcaster: Broadcaster,
            #[serde(flatten)] pub user: $user,
            $(pub $field: $ty),*
        }
    };
}
event_data!(Follow, User, { followed_at: String });
event_data!(Subscribe, User, { tier: String, is_gift: bool });
event_data!(Gift, AnonymousUser, { total: u64, tier: String, cumulative_total: Option<u64>, is_anonymous: bool });
event_data!(SubscriptionMessage, User, { tier: String, message: ResubText, cumulative_months: u64, streak_months: Option<u64>, duration_months: u64 });
data!(ResubText { text: String, emotes: Vec<EmotePosition> });
data!(EmotePosition {
    begin: u64,
    end: u64,
    id: String
});
event_data!(Cheer, AnonymousUser, { is_anonymous: bool, message: String, bits: u64 });
data!(Raid {
    from_broadcaster_user_id: String,
    from_broadcaster_user_login: String,
    from_broadcaster_user_name: String,
    to_broadcaster_user_id: String,
    to_broadcaster_user_login: String,
    to_broadcaster_user_name: String,
    viewers: u64
});
event_data!(Redemption, User, { id: String, user_input: String, status: String, reward: Reward, redeemed_at: String });
data!(Reward {
    id: String,
    title: String,
    cost: u64,
    prompt: String
});
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct HypeTrain {
    #[serde(flatten)]
    pub broadcaster: Broadcaster,
    pub id: String,
    pub total: u64,
    pub progress: u64,
    pub goal: u64,
    pub level: u64,
    pub top_contributions: Vec<Contribution>,
    pub shared_train_participants: Option<Vec<Broadcaster>>,
    pub started_at: String,
    pub expires_at: String,
    pub is_shared_train: bool,
    #[serde(rename = "type")]
    pub kind: String,
    pub all_time_high_level: u64,
    pub all_time_high_total: u64,
}
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Contribution {
    #[serde(flatten)]
    pub user: User,
    #[serde(rename = "type")]
    pub kind: String,
    pub total: u64,
}
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Chat {
    #[serde(flatten)]
    pub broadcaster: Broadcaster,
    pub chatter_user_id: String,
    pub chatter_user_login: String,
    pub chatter_user_name: String,
    pub message_id: String,
    pub message: ChatText,
    pub message_type: String,
    pub color: String,
    pub badges: Vec<Badge>,
    pub cheer: Option<ChatCheer>,
    pub reply: Option<Reply>,
    pub channel_points_custom_reward_id: Option<String>,
    pub source_broadcaster_user_id: Option<String>,
    pub source_broadcaster_user_login: Option<String>,
    pub source_broadcaster_user_name: Option<String>,
    pub source_message_id: Option<String>,
    pub source_badges: Option<Vec<Badge>>,
    pub is_source_only: Option<bool>,
}
data!(ChatText { text: String, fragments: Vec<Fragment> });
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Fragment {
    #[serde(rename = "type")]
    pub kind: String,
    pub text: String,
    pub cheermote: Option<Cheermote>,
    pub emote: Option<Emote>,
    pub mention: Option<User>,
    pub gif: Option<Gif>,
}
data!(Gif {
    id: String,
    url: String
});
data!(Cheermote {
    prefix: String,
    bits: u64,
    tier: u64
});
data!(Emote { id: String, emote_set_id: String, owner_id: String, format: Vec<String> });
data!(Badge {
    set_id: String,
    id: String,
    info: String
});
data!(ChatCheer { bits: u64 });
data!(Reply {
    parent_message_id: String,
    parent_message_body: String,
    parent_user_id: String,
    parent_user_name: String,
    parent_user_login: String,
    thread_message_id: String,
    thread_user_id: String,
    thread_user_name: String,
    thread_user_login: String
});

macro_rules! events {
    ($($variant:ident($ty:ty) => ($kind:literal, $version:literal)),* $(,)?) => {
        #[derive(Debug, Clone, PartialEq)]
        pub enum Event { $($variant(Box<$ty>)),*, Unknown(Value) }
        impl Event {
            fn decode(kind: &str, version: &str, raw: Value) -> serde_json::Result<Self> {
                Ok(match (kind, version) {
                    $(($kind, $version) => Self::$variant(Box::new(serde_json::from_value(raw)?))),*,
                    _ => Self::Unknown(raw),
                })
            }
        }
    };
}
events! {
    Chat(Chat) => ("channel.chat.message", "1"),
    Follow(Follow) => ("channel.follow", "2"),
    Subscribe(Subscribe) => ("channel.subscribe", "1"),
    Gift(Gift) => ("channel.subscription.gift", "1"),
    SubscriptionMessage(SubscriptionMessage) => ("channel.subscription.message", "1"),
    Cheer(Cheer) => ("channel.cheer", "1"),
    Raid(Raid) => ("channel.raid", "1"),
    Redemption(Redemption) => ("channel.channel_points_custom_reward_redemption.add", "1"),
    HypeTrain(HypeTrain) => ("channel.hype_train.begin", "2"),
}
