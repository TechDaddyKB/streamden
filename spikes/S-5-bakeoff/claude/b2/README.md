# S-5 bake-off B-2 (Claude): typed Twitch EventSub parsing

Throwaway spike crate for card DEN-9. It is not part of any workspace and is not production code.

## What it does
`parse_message(&str) -> Result<Message, serde_json::Error>` parses one EventSub WebSocket text frame.

| `message_type` | Variant |
|---|---|
| `session_welcome`, `session_reconnect` | `SessionWelcome` / `SessionReconnect` with a typed `Session` |
| `session_keepalive` | `SessionKeepalive` (payload map, `{}` today) |
| `notification` | `Notification` with a typed `Subscription` and an `Event` |
| `revocation` | `Revocation` with the typed `Subscription` (status = reason) |
| anything else | `Unknown(Value)`, kept verbatim |
| known type, envelope does not match | `Malformed { error, raw }`, kept verbatim |

Typed events, dispatched on `payload.subscription.type` and `.version`:

| Subscription | Version | Variant |
|---|---|---|
| `channel.chat.message` | 1 | `ChatMessage` |
| `channel.follow` | 2 | `Follow` |
| `channel.subscribe` | 1 | `Subscribe` |
| `channel.subscription.gift` | 1 | `SubscriptionGift` |
| `channel.subscription.message` | 1 | `SubscriptionMessage` |
| `channel.cheer` | 1 | `Cheer` |
| `channel.raid` | 1 | `Raid` |
| `channel.channel_points_custom_reward_redemption.add` | 1 | `RewardRedemptionAdd` |
| `channel.hype_train.begin` | 2 (current) | `HypeTrainBegin` |

`Notification` also deserializes on its own, which is the webhook body shape.

## Never dropping data
- Unknown subscription type or version: `Event::Raw` with the JSON.
- Known type whose body no longer matches the struct (Twitch schema change, bad payload): `Event::Raw`
  with `parse_error` set. The event is still delivered; the caller decides whether to log it.
- New fields: every struct has a `#[serde(flatten)] extra` map, so they are kept and serialized back.
- New enum values (tier, message type, fragment type, redemption status, hype train and contribution type):
  each string enum has an `Other(String)` fallback.
- Timestamps stay `String` so nanosecond RFC 3339 values round-trip exactly.

- Known message type whose envelope does not match (for example a `notification` whose subscription lacks
  `cost`, or a `session_welcome` without `payload.session`): `Message::Malformed` with the error and the JSON.

`parse_message` returns `Err` only for frames that are not EventSub messages at all: not JSON, not an object,
or no `metadata.message_type`.

## Known limitations
- Nullable fields that are present as `null` round-trip; if Twitch *omits* such a field, it comes back as
  `null`. Fields the docs or examples show as sometimes absent (`is_source_only`, fragment `gif`,
  `transport.session_id`/`callback`, metadata `subscription_type`/`version`) are skipped when `None`.
- Top-level envelope keys other than `metadata` and `payload` are not kept (Twitch sends none).
- `subscription.condition` stays an untyped map; it differs per type.

## Fixtures and sources
All fixtures in `tests/fixtures/` are the example payloads from the official docs, fetched 2026-10-18:
- `ws_*.json`: <https://dev.twitch.tv/docs/eventsub/handling-websocket-events/>
- `event_*.json`, `unknown_channel_update_v2.json`: the notification payload examples on
  <https://dev.twitch.tv/docs/eventsub/eventsub-subscription-types/>
- Field types: <https://dev.twitch.tv/docs/eventsub/eventsub-reference/>

Edits to the docs' text: re-indented, and `// ...` comments removed from four examples that the docs annotate
inline (gift, subscription message, cheer, redemption add), because JSON has no comments. Values are unchanged.
The docs' WebSocket `notification` example is `channel.follow` v1 (superseded by v2), so it doubles as the
"unsupported version stays raw" fixture. `unknown_channel_update_v2.json` covers an untyped subscription type.

## Tests
`tests/golden.rs`: each fixture is parsed into the expected variant, checked for a few typed values, and
serialized back to JSON that must equal the fixture. Event fixtures are checked both bare (webhook shape) and
wrapped in a WebSocket `notification` envelope, and the typed struct's top-level `extra` must be empty (so a
missing struct field fails the test instead of hiding in `extra`). Extra tests cover unknown message types,
malformed known messages, non-EventSub frames, a malformed known event, new fields at several levels, and unknown enum values.

```sh
cargo test
cargo clippy -- -D warnings
```

Dependencies: `serde` and `serde_json` (both MIT OR Apache-2.0, GPL-3.0-compatible).
