//! Golden round-trip tests: every fixture parses into the expected typed variant and
//! serializes back to JSON equal to the fixture.
//!
//! Fixture sources (official Twitch docs, fetched 2026-10-18; see README.md for edits):
//! - `ws_*.json`: <https://dev.twitch.tv/docs/eventsub/handling-websocket-events/>
//! - `event_*.json`, `unknown_*.json`: notification payload examples in
//!   <https://dev.twitch.tv/docs/eventsub/eventsub-subscription-types/>

use den_eventsub_b2::*;
use serde_json::{json, Value};

fn fixture(name: &str) -> Value {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// Parses a WebSocket message fixture from text and checks the golden round trip.
fn ws_round_trip(original: &Value) -> Message {
    let message = parse_message(&original.to_string()).expect("parse");
    let back = serde_json::to_value(&message).expect("serialize");
    assert_eq!(&back, original, "round trip changed the message");
    message
}

/// Parses a notification payload fixture, checks its round trip both bare (webhook shape)
/// and wrapped in a WebSocket `notification` envelope, and returns the typed event.
fn event_round_trip(name: &str) -> Event {
    let payload = fixture(name);

    let notification: Notification = serde_json::from_value(payload.clone()).expect("parse");
    assert!(
        notification.extra.is_empty(),
        "{name}: unexpected payload keys"
    );
    assert_eq!(
        serde_json::to_value(&notification).unwrap(),
        payload,
        "{name}: payload round trip"
    );

    let subscription = &payload["subscription"];
    let envelope = json!({
        "metadata": {
            "message_id": "befa7b53-d79d-478f-86b9-120f112b044e",
            "message_type": "notification",
            "message_timestamp": "2022-11-16T10:11:12.464757833Z",
            "subscription_type": subscription["type"],
            "subscription_version": subscription["version"],
        },
        "payload": payload,
    });
    match ws_round_trip(&envelope) {
        Message::Notification { payload, .. } => assert_eq!(*payload, notification),
        other => panic!("{name}: expected Notification, got {other:?}"),
    }

    let event = notification.event;
    if let Some(extra) = event.extra() {
        assert!(
            extra.is_empty(),
            "{name}: fields not covered by the typed struct: {extra:?}"
        );
    }
    event
}

// --- WebSocket messages ------------------------------------------------------------------

#[test]
fn session_welcome() {
    let Message::SessionWelcome { metadata, payload } =
        ws_round_trip(&fixture("ws_session_welcome.json"))
    else {
        panic!("expected SessionWelcome");
    };
    assert_eq!(metadata.message_type, "session_welcome");
    assert_eq!(payload.session.id, "AQoQILE98gtqShGmLD7AM6yJThAB");
    assert_eq!(payload.session.keepalive_timeout_seconds, Some(10));
    assert_eq!(payload.session.reconnect_url, None);
}

#[test]
fn session_keepalive() {
    let Message::SessionKeepalive { payload, .. } =
        ws_round_trip(&fixture("ws_session_keepalive.json"))
    else {
        panic!("expected SessionKeepalive");
    };
    assert!(payload.is_empty());
}

#[test]
fn session_reconnect() {
    let Message::SessionReconnect { payload, .. } =
        ws_round_trip(&fixture("ws_session_reconnect.json"))
    else {
        panic!("expected SessionReconnect");
    };
    assert_eq!(payload.session.status, "reconnecting");
    assert_eq!(payload.session.keepalive_timeout_seconds, None);
    assert_eq!(
        payload.session.reconnect_url.as_deref(),
        Some("wss://eventsub.wss.twitch.tv?...")
    );
}

#[test]
fn revocation() {
    let Message::Revocation { metadata, payload } = ws_round_trip(&fixture("ws_revocation.json"))
    else {
        panic!("expected Revocation");
    };
    assert_eq!(
        metadata.subscription_type.as_deref(),
        Some("channel.follow")
    );
    assert_eq!(payload.subscription.status, "authorization_revoked");
    assert_eq!(payload.subscription.transport.method, "websocket");
}

/// The docs' WebSocket notification example is `channel.follow` v1, which this crate does
/// not type (v2 is current). It must survive as a raw event.
#[test]
fn notification_with_unsupported_version_stays_raw() {
    let Message::Notification { payload, .. } = ws_round_trip(&fixture("ws_notification.json"))
    else {
        panic!("expected Notification");
    };
    let Event::Raw(raw) = payload.event else {
        panic!("expected Raw");
    };
    assert_eq!(
        (raw.subscription_type.as_str(), raw.version.as_str()),
        ("channel.follow", "1")
    );
    assert_eq!(raw.parse_error, None);
    assert_eq!(raw.event["followed_at"], "2023-07-15T18:16:11.17106713Z");
}

#[test]
fn unknown_message_type_is_kept() {
    let original = json!({
        "metadata": {
            "message_id": "1",
            "message_type": "session_teleport",
            "message_timestamp": "2026-10-18T00:00:00Z"
        },
        "payload": { "anything": [1, 2, 3] }
    });
    let message = ws_round_trip(&original);
    assert!(matches!(message, Message::Unknown(_)));
    assert!(message.metadata().is_none());
}

/// A known message type whose envelope does not match is kept, not turned into an error.
#[test]
fn malformed_known_message_is_kept() {
    let mut no_session = fixture("ws_session_welcome.json");
    no_session["payload"] = json!({});
    let mut no_cost = fixture("ws_notification.json");
    no_cost["payload"]["subscription"]
        .as_object_mut()
        .unwrap()
        .remove("cost");
    for original in [no_session, no_cost] {
        let Message::Malformed { error, .. } = ws_round_trip(&original) else {
            panic!("expected Malformed for {original}");
        };
        assert!(error.contains("missing field"), "{error}");
    }
}

#[test]
fn non_eventsub_frames_are_errors() {
    assert!(parse_message(r#"{"payload":{}}"#).is_err());
    assert!(parse_message("[]").is_err());
    assert!(parse_message("not json").is_err());
}

// --- Typed notification events -----------------------------------------------------------

#[test]
fn chat_message() {
    let Event::ChatMessage(e) = event_round_trip("event_channel_chat_message_v1.json") else {
        panic!("expected ChatMessage");
    };
    assert_eq!(e.chatter_user_login, "viewer32");
    assert_eq!(e.message.text, "Hi chat");
    assert_eq!(e.message.fragments[0].fragment_type, FragmentType::Text);
    assert_eq!(e.message_type, ChatMessageType::Text);
    assert_eq!(e.badges.len(), 3);
    assert_eq!(e.badges[1].info, "16");
    assert_eq!(e.source_broadcaster_user_id, None);
    assert_eq!(e.is_source_only, None);
}

#[test]
fn chat_message_shared_chat() {
    let Event::ChatMessage(e) = event_round_trip("event_channel_chat_message_v1_shared_chat.json")
    else {
        panic!("expected ChatMessage");
    };
    assert_eq!(
        e.source_broadcaster_user_login.as_deref(),
        Some("streamer33")
    );
    assert_eq!(e.source_badges.as_ref().map(Vec::len), Some(1));
    assert_eq!(e.is_source_only, Some(true));
}

#[test]
fn follow_v2() {
    let Event::Follow(e) = event_round_trip("event_channel_follow_v2.json") else {
        panic!("expected Follow");
    };
    assert_eq!(e.user_name, "Cool_User");
    assert_eq!(e.followed_at, "2020-07-15T18:16:11.17106713Z");
}

#[test]
fn subscribe() {
    let Event::Subscribe(e) = event_round_trip("event_channel_subscribe_v1.json") else {
        panic!("expected Subscribe");
    };
    assert_eq!(e.tier, SubTier::Tier1);
    assert!(!e.is_gift);
}

#[test]
fn subscription_gift() {
    let Event::SubscriptionGift(e) = event_round_trip("event_channel_subscription_gift_v1.json")
    else {
        panic!("expected SubscriptionGift");
    };
    assert_eq!(e.total, 2);
    assert_eq!(e.cumulative_total, Some(284));
    assert_eq!(e.user_id.as_deref(), Some("1234"));
}

#[test]
fn subscription_message() {
    let Event::SubscriptionMessage(e) =
        event_round_trip("event_channel_subscription_message_v1.json")
    else {
        panic!("expected SubscriptionMessage");
    };
    assert_eq!(e.message.text, "Love the stream! FevziGG");
    let emotes = e.message.emotes.expect("emotes");
    assert_eq!(
        (emotes[0].begin, emotes[0].end, emotes[0].id.as_str()),
        (23, 30, "302976485")
    );
    assert_eq!(
        (e.cumulative_months, e.streak_months, e.duration_months),
        (15, Some(1), 6)
    );
}

#[test]
fn cheer() {
    let Event::Cheer(e) = event_round_trip("event_channel_cheer_v1.json") else {
        panic!("expected Cheer");
    };
    assert_eq!(e.bits, 1000);
    assert!(!e.is_anonymous);
    assert_eq!(e.message, "pogchamp");
}

#[test]
fn raid() {
    let Event::Raid(e) = event_round_trip("event_channel_raid_v1.json") else {
        panic!("expected Raid");
    };
    assert_eq!(e.viewers, 9001);
    assert_eq!(e.from_broadcaster_user_login, "cool_user");
}

#[test]
fn reward_redemption_add() {
    let Event::RewardRedemptionAdd(e) =
        event_round_trip("event_channel_channel_points_custom_reward_redemption_add_v1.json")
    else {
        panic!("expected RewardRedemptionAdd");
    };
    assert_eq!(e.status, RedemptionStatus::Unfulfilled);
    assert_eq!(e.reward.cost, 100);
    assert_eq!(e.user_input, "pogchamp");
}

#[test]
fn hype_train_begin_v2() {
    let Event::HypeTrainBegin(e) = event_round_trip("event_channel_hype_train_begin_v2.json")
    else {
        panic!("expected HypeTrainBegin");
    };
    assert_eq!(e.train_type, HypeTrainType::Regular);
    assert_eq!((e.total, e.progress, e.goal, e.level), (137, 137, 500, 1));
    assert_eq!(
        e.top_contributions[0].contribution_type,
        ContributionType::Bits
    );
    assert_eq!(e.shared_train_participants, None);
    assert_eq!(e.all_time_high_total, 2845);
}

// --- Never drop data ---------------------------------------------------------------------

#[test]
fn unknown_subscription_type_stays_raw() {
    let Event::Raw(raw) = event_round_trip("unknown_channel_update_v2.json") else {
        panic!("expected Raw");
    };
    assert_eq!(raw.subscription_type, "channel.update");
    assert_eq!(raw.parse_error, None);
    assert_eq!(raw.event["title"], "Best Stream Ever");
}

/// A known type whose event no longer matches the typed struct is kept raw, with the error.
#[test]
fn malformed_known_event_falls_back_to_raw() {
    let mut payload = fixture("event_channel_raid_v1.json");
    payload["event"]["viewers"] = json!("lots");
    let notification: Notification = serde_json::from_value(payload.clone()).unwrap();
    let Event::Raw(raw) = &notification.event else {
        panic!("expected Raw");
    };
    assert_eq!(raw.subscription_type, "channel.raid");
    assert!(raw
        .parse_error
        .as_deref()
        .is_some_and(|e| e.contains("viewers") || e.contains("invalid type")));
    assert_eq!(serde_json::to_value(&notification).unwrap(), payload);
}

/// Fields Twitch adds later are kept in `extra` at every level and serialized back.
#[test]
fn new_fields_are_preserved() {
    let mut payload = fixture("event_channel_chat_message_v1.json");
    payload["event"]["channel_points_animation_id"] = json!("rainbow-eclipse");
    payload["event"]["message"]["fragments"][0]["future"] = json!({"x": 1});
    payload["event"]["badges"][0]["title"] = json!("Moderator");
    payload["subscription"]["transport"]["conduit_id"] = json!("abc");

    let notification: Notification = serde_json::from_value(payload.clone()).unwrap();
    let Event::ChatMessage(e) = &notification.event else {
        panic!("expected ChatMessage");
    };
    assert_eq!(e.extra["channel_points_animation_id"], "rainbow-eclipse");
    assert_eq!(e.message.fragments[0].extra["future"], json!({"x": 1}));
    assert_eq!(e.badges[0].extra["title"], "Moderator");
    assert_eq!(
        notification.subscription.transport.extra["conduit_id"],
        "abc"
    );
    assert_eq!(serde_json::to_value(&notification).unwrap(), payload);
}

/// String enums keep values this crate does not know yet.
#[test]
fn unknown_enum_values_are_preserved() {
    let mut payload = fixture("event_channel_hype_train_begin_v2.json");
    payload["event"]["type"] = json!("rainbow");
    payload["event"]["top_contributions"][0]["type"] = json!("other");
    let notification: Notification = serde_json::from_value(payload.clone()).unwrap();
    let Event::HypeTrainBegin(e) = &notification.event else {
        panic!("expected HypeTrainBegin");
    };
    assert_eq!(e.train_type, HypeTrainType::Other("rainbow".into()));
    assert_eq!(
        e.top_contributions[0].contribution_type,
        ContributionType::OtherMethod
    );
    assert_eq!(serde_json::to_value(&notification).unwrap(), payload);
}
