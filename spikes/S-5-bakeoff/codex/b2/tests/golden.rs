use eventsub_b2_codex::{Event, Message, Payload};
use serde_json::{json, Value};

// Official sources (retrieved 2026-10-19):
// https://dev.twitch.tv/docs/eventsub/handling-websocket-events/#welcome-message
// https://dev.twitch.tv/docs/eventsub/handling-websocket-events/#keepalive-message
// https://dev.twitch.tv/docs/eventsub/handling-websocket-events/#reconnect-message
// https://dev.twitch.tv/docs/eventsub/handling-websocket-events/#revocation-message
// https://dev.twitch.tv/docs/eventsub/eventsub-subscription-types/#channel-chat-message
// https://dev.twitch.tv/docs/eventsub/eventsub-subscription-types/#channel-follow
// https://dev.twitch.tv/docs/eventsub/eventsub-subscription-types/#channel-subscribe
// https://dev.twitch.tv/docs/eventsub/eventsub-subscription-types/#channel-subscription-gift
// https://dev.twitch.tv/docs/eventsub/eventsub-subscription-types/#channel-subscription-message
// https://dev.twitch.tv/docs/eventsub/eventsub-subscription-types/#channel-cheer
// https://dev.twitch.tv/docs/eventsub/eventsub-subscription-types/#channel-raid
// https://dev.twitch.tv/docs/eventsub/eventsub-subscription-types/#channel-points-custom-reward-redemption-add
// https://dev.twitch.tv/docs/eventsub/eventsub-subscription-types/#channel-hype-train-begin
// See fixtures/refresh.py: unchanged event bodies; transport/envelope adapted to WS.
fn round_trip(raw: Value) -> Message {
    let parsed: Message = serde_json::from_value(raw.clone()).unwrap();
    let serialized = serde_json::to_value(&parsed).unwrap();
    assert_eq!(serialized, raw);
    let again: Message = serde_json::from_value(serialized).unwrap();
    assert_eq!(parsed.payload(), again.payload());
    parsed
}
macro_rules! golden {
    ($name:ident, $pattern:pat, $check:expr) => {
        #[test]
        fn $name() {
            let message = round_trip(
                serde_json::from_str(include_str!(concat!(
                    "fixtures/",
                    stringify!($name),
                    ".json"
                )))
                .unwrap(),
            );
            let $pattern = message.payload() else {
                panic!("wrong payload variant: {:?}", message.payload())
            };
            $check
        }
    };
}
golden!(welcome, Payload::Welcome(s), {
    assert_eq!(s.keepalive_timeout_seconds, Some(10));
    assert_eq!(s.reconnect_url, None);
});
golden!(keepalive, Payload::Keepalive, {});
golden!(reconnect, Payload::Reconnect(s), {
    assert_eq!(s.keepalive_timeout_seconds, None);
    assert!(s.reconnect_url.as_ref().unwrap().starts_with("wss://"));
});
golden!(revocation, Payload::Revocation(s), {
    assert_eq!(s.status, "authorization_revoked");
});
golden!(
    chat,
    Payload::Notification {
        event: Event::Chat(e),
        ..
    },
    {
        assert_eq!(e.message.text, "Hi chat");
        assert_eq!(e.badges.len(), 3);
        assert_eq!(e.message.fragments[0].kind, "text");
    }
);
golden!(
    shared_chat,
    Payload::Notification {
        event: Event::Chat(e),
        ..
    },
    {
        assert_eq!(e.source_broadcaster_user_id.as_deref(), Some("112233"));
        assert_eq!(e.is_source_only, Some(true));
    }
);
golden!(
    follow,
    Payload::Notification {
        event: Event::Follow(e),
        ..
    },
    {
        assert_eq!(e.user.user_id, "1234");
        assert_eq!(e.followed_at, "2020-07-15T18:16:11.17106713Z");
    }
);
golden!(
    subscribe,
    Payload::Notification {
        event: Event::Subscribe(e),
        ..
    },
    {
        assert_eq!(e.tier, "1000");
        assert!(!e.is_gift);
    }
);
golden!(
    gift,
    Payload::Notification {
        event: Event::Gift(e),
        ..
    },
    {
        assert_eq!(e.total, 2);
        assert_eq!(e.cumulative_total, Some(284));
    }
);
golden!(
    subscription_message,
    Payload::Notification {
        event: Event::SubscriptionMessage(e),
        ..
    },
    {
        assert_eq!(e.cumulative_months, 15);
        assert_eq!(e.message.emotes[0].id, "302976485");
    }
);
golden!(
    cheer,
    Payload::Notification {
        event: Event::Cheer(e),
        ..
    },
    {
        assert_eq!(e.bits, 1000);
    }
);
golden!(
    raid,
    Payload::Notification {
        event: Event::Raid(e),
        ..
    },
    {
        assert_eq!(e.viewers, 9001);
    }
);
golden!(
    redemption,
    Payload::Notification {
        event: Event::Redemption(e),
        ..
    },
    {
        assert_eq!(e.reward.cost, 100);
    }
);
golden!(
    hype_train,
    Payload::Notification {
        event: Event::HypeTrain(e),
        subscription
    },
    {
        assert_eq!(subscription.version, "2");
        assert_eq!(e.all_time_high_total, 2845);
        assert_eq!(e.top_contributions[0].total, 50);
    }
);

fn notification() -> Value {
    serde_json::from_str(include_str!("fixtures/follow.json")).unwrap()
}

#[test]
fn unknown_types_and_versions_are_lossless() {
    for (kind, version) in [("channel.future", "1"), ("channel.follow", "99")] {
        let mut raw = notification();
        raw["metadata"]["subscription_type"] = json!(kind);
        raw["metadata"]["subscription_version"] = json!(version);
        raw["payload"]["subscription"]["type"] = json!(kind);
        raw["payload"]["subscription"]["version"] = json!(version);
        raw["payload"]["event"] = json!({"future": [null, 42, {"a": "b"}]});
        let message = round_trip(raw);
        let Payload::Notification {
            event: Event::Unknown(event),
            ..
        } = message.payload()
        else {
            panic!()
        };
        assert_eq!(event["future"][1], 42);
    }
    let mut raw = notification();
    raw["metadata"]["message_type"] = json!("session_future");
    raw["payload"] = json!({"unknown": true});
    assert!(matches!(round_trip(raw).payload(), Payload::Unknown(_)));
}

#[test]
fn malformed_known_events_and_conflicting_discriminators_are_errors() {
    let original = notification();
    for pointer in [
        "/payload/event/followed_at",
        "/payload/event/user_id",
        "/payload/subscription/cost",
    ] {
        let mut raw = original.clone();
        *raw.pointer_mut(pointer).unwrap() = json!(false);
        assert!(serde_json::from_value::<Message>(raw).is_err(), "{pointer}");
    }
    let mut raw = original.clone();
    raw["payload"].as_object_mut().unwrap().remove("event");
    assert!(serde_json::from_value::<Message>(raw).is_err());
    for key in ["subscription_type", "subscription_version"] {
        let mut raw = original.clone();
        raw["metadata"][key] = json!("wrong");
        assert!(serde_json::from_value::<Message>(raw).is_err());
    }
    for raw in [
        json!(null),
        json!({}),
        json!({"metadata": original["metadata"]}),
    ] {
        assert!(serde_json::from_value::<Message>(raw).is_err());
    }
}

#[test]
fn additions_and_missing_optional_fields_survive() {
    let mut raw = notification();
    raw["new_envelope"] = json!([1, 2]);
    raw["metadata"]["new_metadata"] = json!(null);
    raw["payload"]["event"]["future_field"] = json!({"nested": true});
    assert!(matches!(
        round_trip(raw).payload(),
        Payload::Notification {
            event: Event::Follow(_),
            ..
        }
    ));
    let mut raw: Value = serde_json::from_str(include_str!("fixtures/chat.json")).unwrap();
    raw["payload"]["event"]
        .as_object_mut()
        .unwrap()
        .remove("source_badges");
    round_trip(raw);
}

#[test]
fn anonymous_gifts_cheers_and_private_streaks() {
    for fixture in [
        include_str!("fixtures/gift.json"),
        include_str!("fixtures/cheer.json"),
    ] {
        let mut raw: Value = serde_json::from_str(fixture).unwrap();
        for key in ["user_id", "user_login", "user_name"] {
            raw["payload"]["event"][key] = Value::Null;
        }
        raw["payload"]["event"]["is_anonymous"] = json!(true);
        if raw["metadata"]["subscription_type"] == "channel.subscription.gift" {
            raw["payload"]["event"]["cumulative_total"] = Value::Null;
        }
        let parsed = round_trip(raw);
        match parsed.payload() {
            Payload::Notification {
                event: Event::Gift(e),
                ..
            } => {
                assert!(e.is_anonymous);
                assert!(e.user.user_id.is_none());
            }
            Payload::Notification {
                event: Event::Cheer(e),
                ..
            } => {
                assert!(e.is_anonymous);
                assert!(e.user.user_id.is_none());
            }
            _ => panic!(),
        }
    }
    let mut raw: Value =
        serde_json::from_str(include_str!("fixtures/subscription_message.json")).unwrap();
    raw["payload"]["event"]["streak_months"] = Value::Null;
    assert!(
        matches!(round_trip(raw).payload(), Payload::Notification { event: Event::SubscriptionMessage(e), .. } if e.streak_months.is_none())
    );
}

// Synthetic edge cases based on the current reference field definitions:
// https://dev.twitch.tv/docs/eventsub/eventsub-reference/#channel-chat-message-event
// https://dev.twitch.tv/docs/eventsub/eventsub-reference/#hype-train-begin-event
#[test]
fn structured_chat_and_shared_hype_train() {
    let mut raw: Value = serde_json::from_str(include_str!("fixtures/chat.json")).unwrap();
    raw["payload"]["event"]["message"]["fragments"] = json!([
        {"type":"cheermote", "text":"Cheer100", "cheermote":{"prefix":"cheer","bits":100,"tier":100}},
        {"type":"emote", "text":"Kappa", "emote":{"id":"25","emote_set_id":"0","owner_id":"0","format":["static"]}},
        {"type":"mention", "text":"@viewer", "mention":{"user_id":"123","user_login":"viewer","user_name":"Viewer"}},
        {"type":"gif", "text":"GIF", "gif":{"id":"gif-id", "url":"https://example.com/asset.gif"}}
    ]);
    raw["payload"]["event"]["cheer"] = json!({"bits":100});
    raw["payload"]["event"]["reply"] = json!({
        "parent_message_id":"p", "parent_message_body":"hello", "parent_user_id":"1",
        "parent_user_login":"parent", "parent_user_name":"Parent", "thread_message_id":"t",
        "thread_user_id":"2", "thread_user_login":"thread", "thread_user_name":"Thread"
    });
    let parsed = round_trip(raw.clone());
    let Payload::Notification {
        event: Event::Chat(e),
        ..
    } = parsed.payload()
    else {
        panic!()
    };
    assert_eq!(e.message.fragments[0].cheermote.as_ref().unwrap().bits, 100);
    assert_eq!(e.message.fragments[1].emote.as_ref().unwrap().id, "25");
    assert_eq!(
        e.message.fragments[2].mention.as_ref().unwrap().user_id,
        "123"
    );
    assert_eq!(e.message.fragments[3].gif.as_ref().unwrap().id, "gif-id");
    assert_eq!(e.reply.as_ref().unwrap().thread_message_id, "t");
    raw["payload"]["event"]["message"]["fragments"][0]["cheermote"]["bits"] = json!("100");
    assert!(serde_json::from_value::<Message>(raw).is_err());

    let mut raw: Value = serde_json::from_str(include_str!("fixtures/hype_train.json")).unwrap();
    raw["payload"]["event"]["is_shared_train"] = json!(true);
    raw["payload"]["event"]["shared_train_participants"] = json!([{
        "broadcaster_user_id":"42", "broadcaster_user_login":"guest", "broadcaster_user_name":"Guest"
    }]);
    let parsed = round_trip(raw);
    let Payload::Notification {
        event: Event::HypeTrain(e),
        ..
    } = parsed.payload()
    else {
        panic!()
    };
    assert!(e.is_shared_train);
    assert_eq!(
        e.shared_train_participants.as_ref().unwrap()[0].broadcaster_user_id,
        "42"
    );
}

#[test]
fn invalid_sessions_and_envelopes_are_rejected() {
    let original: Value = serde_json::from_str(include_str!("fixtures/welcome.json")).unwrap();
    for pointer in [
        "/payload/session/keepalive_timeout_seconds",
        "/payload/session/id",
        "/metadata/message_id",
    ] {
        let mut raw = original.clone();
        *raw.pointer_mut(pointer).unwrap() = json!(false);
        assert!(serde_json::from_value::<Message>(raw).is_err());
    }
    for payload in [json!(null), json!([]), json!("unexpected")] {
        let mut raw = original.clone();
        raw["payload"] = payload;
        assert!(serde_json::from_value::<Message>(raw).is_err());
    }
    let mut raw = original.clone();
    raw.as_object_mut().unwrap().remove("payload");
    assert!(serde_json::from_value::<Message>(raw).is_err());
    assert!(serde_json::from_str::<Message>("{broken").is_err());
}
