//! Golden round-trip for every fixture under `tests/fixtures`.
//!
//! Official examples (01–18) are quoted from Twitch's EventSub docs. Where a
//! page shows only the notification body, the fixture adds the WebSocket
//! `metadata` envelope from
//! <https://dev.twitch.tv/eventsub/handling-websocket-events/>.
//! Inline `//` comments in those examples are not JSON, so they are omitted
//! and the documented value is kept. 19 and 20 are constructed so an unknown
//! message type and an unknown subscription type are retained.
//!
//! Sources:
//! - session frames, revocation, follow v1 notification metadata:
//!   <https://dev.twitch.tv/eventsub/handling-websocket-events/>
//! - follow v1 event body (same ids): <https://dev.twitch.tv/eventsub/handling-webhook-events/>
//! - follow v2, subscribe, gift, resub message, cheer, raid, redemption,
//!   hype train begin v2, chat message, chat notification:
//!   <https://dev.twitch.tv/eventsub/eventsub-subscription-types/>
//! - hype train begin version is 2 (current):
//!   <https://dev.twitch.tv/eventsub/eventsub-subscription-types/>
//! - shorter chat message: <https://dev.twitch.tv/chat/send-receive-messages/>
//! - moderate raid: <https://dev.twitch.tv/chat/moderation/>

use std::fs;
use std::path::PathBuf;

use eventsub_b2::{parse, EventBody, Message};
use serde_json::Value;

#[test]
fn every_fixture_round_trips_and_keeps_unknowns() {
    let mut paths = fixture_paths();
    paths.sort();
    assert_eq!(paths.len(), 20, "DEN-9 asks for a golden test per fixture");

    for path in paths {
        let name = path.file_name().unwrap().to_str().unwrap().to_owned();
        let text = fs::read_to_string(&path).unwrap_or_else(|err| panic!("{name}: {err}"));
        let original: Value =
            serde_json::from_str(&text).unwrap_or_else(|err| panic!("{name}: {err}"));
        let message = parse(&text).unwrap_or_else(|err| panic!("{name}: {err}"));
        let encoded = serde_json::to_value(&message).unwrap_or_else(|err| panic!("{name}: {err}"));
        assert_eq!(encoded, original, "{name} did not round-trip");
        let again = parse(&serde_json::to_string(&encoded).unwrap()).unwrap();
        assert_eq!(
            serde_json::to_value(&again).unwrap(),
            original,
            "{name} was not stable"
        );
        assert_kind(&name, &message, &original);
    }
}

fn fixture_paths() -> Vec<PathBuf> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    fs::read_dir(&dir)
        .unwrap_or_else(|err| panic!("read {}: {err}", dir.display()))
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect()
}

fn assert_kind(name: &str, message: &Message, original: &Value) {
    match name {
        "01-session-welcome.json" => {
            let Message::SessionWelcome(welcome) = message else {
                panic!("{name}: {message:?}");
            };
            assert_eq!(welcome.payload.session.id, "AQoQILE98gtqShGmLD7AM6yJThAB");
            assert_eq!(welcome.payload.session.keepalive_timeout_seconds, Some(10));
        }
        "02-session-keepalive.json" => {
            let Message::SessionKeepalive(keepalive) = message else {
                panic!("{name}");
            };
            assert!(keepalive.payload.is_empty());
        }
        "03-session-reconnect.json" => {
            let Message::SessionReconnect(reconnect) = message else {
                panic!("{name}");
            };
            assert_eq!(
                reconnect.payload.session.reconnect_url.as_deref(),
                Some("wss://eventsub.wss.twitch.tv?...")
            );
            assert_eq!(reconnect.payload.session.keepalive_timeout_seconds, None);
        }
        "04-revocation.json" => {
            let Message::Revocation(revocation) = message else {
                panic!("{name}");
            };
            assert_eq!(
                revocation.payload.subscription.status,
                "authorization_revoked"
            );
        }
        "05-notification-follow-v1.json" | "06-notification-follow-v2.json" => {
            let Message::Notification(notification) = message else {
                panic!("{name}");
            };
            let EventBody::Follow(follow) = &notification.event else {
                panic!("{name}: {:?}", notification.event);
            };
            assert!(!follow.followed_at.is_empty());
            if name.starts_with("06") {
                assert_eq!(notification.subscription.version, "2");
                assert_eq!(
                    notification.subscription.condition["moderator_user_id"],
                    "1337"
                );
            }
        }
        "07-notification-subscribe.json" => match_subscribe(name, message),
        "08-notification-subscription-gift.json" => {
            let EventBody::SubscriptionGift(gift) = event(name, message) else {
                panic!("{name}");
            };
            assert_eq!(gift.total, 2);
            assert_eq!(gift.cumulative_total, Some(284));
        }
        "09-notification-subscription-message.json" => {
            let EventBody::SubscriptionMessage(resub) = event(name, message) else {
                panic!("{name}");
            };
            assert_eq!(resub.message.text, "Love the stream! FevziGG");
            assert_eq!(resub.message.emotes[0].id, "302976485");
        }
        "10-notification-cheer.json" => {
            let EventBody::Cheer(cheer) = event(name, message) else {
                panic!("{name}");
            };
            assert_eq!(cheer.bits, 1000);
        }
        "11-notification-raid.json" => {
            let EventBody::Raid(raid) = event(name, message) else {
                panic!("{name}");
            };
            assert_eq!(raid.viewers, 9001);
        }
        "12-notification-redemption-add.json" => {
            let EventBody::RedemptionAdd(redemption) = event(name, message) else {
                panic!("{name}");
            };
            assert_eq!(redemption.user_input, "pogchamp");
            assert_eq!(redemption.reward.cost, 100);
        }
        "13-notification-hype-train-begin-v2.json" => {
            let EventBody::HypeTrainBegin(train) = event(name, message) else {
                panic!("{name}");
            };
            assert_eq!(train.train_type, "regular");
            assert_eq!(train.level, 1);
            assert_eq!(train.top_contributions[0].contribution_type, "bits");
            let Message::Notification(notification) = message else {
                panic!("{name}");
            };
            assert_eq!(notification.subscription.version, "2");
        }
        "14-notification-chat-message.json" | "15-notification-chat-message-send-receive.json" => {
            let EventBody::ChatMessage(chat) = event(name, message) else {
                panic!("{name}");
            };
            assert_eq!(chat.message.text, "Hi chat");
            assert_eq!(chat.badges.len(), 3);
            assert!(chat.reply.is_none());
        }
        "16-notification-chat-notification.json"
        | "17-notification-moderate-raid.json"
        | "18-notification-automatic-reward.json"
        | "20-unknown-subscription-type.json" => {
            let EventBody::Raw(raw) = event(name, message) else {
                panic!("{name} was typed instead of kept raw: {message:?}");
            };
            let original_event = &original["payload"]["event"];
            assert_eq!(raw, original_event, "{name} dropped event fields");
        }
        "19-unknown-message-type.json" => {
            let Message::Unknown(raw) = message else {
                panic!("{name} was dropped or reclassified: {message:?}");
            };
            assert_eq!(raw, original);
            assert_eq!(message.message_type(), Some("session_something_new"));
        }
        other => panic!("no assertion for {other}"),
    }
}

fn event<'a>(name: &str, message: &'a Message) -> &'a EventBody {
    let Message::Notification(notification) = message else {
        panic!("{name} was not a notification: {message:?}");
    };
    &notification.event
}

fn match_subscribe(name: &str, message: &Message) {
    let EventBody::Subscribe(subscribe) = event(name, message) else {
        panic!("{name}");
    };
    assert!(!subscribe.is_gift);
    assert_eq!(subscribe.tier, "1000");
}
