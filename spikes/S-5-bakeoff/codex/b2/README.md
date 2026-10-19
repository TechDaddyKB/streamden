# DEN-9 · Codex B-2 EventSub parser

Standalone throwaway Rust crate; no production workspace or network client.

```sh
cd spikes/S-5-bakeoff/codex/b2
cargo test
cargo clippy -- -D warnings
```

Deserialize `Message` with `serde_json::from_str`. Match `message.payload()` on
`Welcome`, `Keepalive`, `Reconnect`, `Notification`, `Revocation`, or `Unknown`.
Notifications expose typed events for chat, follow v2, subscribe, gift,
subscription message, cheer, raid, custom reward redemption, and Hype Train Begin
v2 (the current version listed by Twitch on 2026-10-19).

Dispatch uses both subscription type and version. Unknown event types and versions
retain the raw event in `Event::Unknown`; unknown WebSocket message types retain
the raw payload in `Payload::Unknown`. Metadata and subscription discriminators
must agree. Malformed known payloads return a deserialization error; they never
silently become unknown events.

`Message` owns an immutable JSON document and typed read-only projections. Its
serializer emits that document, preserving unknown fields at every depth, explicit
nulls and omitted fields. Round-trips compare JSON values and typed payloads,
not bytes: whitespace and object ordering are not preserved. There is no mutation
API; cloning and editing a projection does not edit the message. This deliberate
spike tradeoff uses extra memory to preserve future fields without complex custom
serializers for every nested type. IDs, timestamps, URLs and open vocabularies are
strings; this parser checks structure and field types, not semantic validity or
transport authentication. The caller must bound input size.

Fourteen golden fixtures cover the four session/revocation messages, the nine
notification types, and shared chat. Additional tests cover unknown types and
versions, additions, absent/null fields, anonymous users, private streaks,
structured chat fragments (including GIFs), shared trains, malformed payloads and
conflicting metadata. Golden tests assert concrete typed fields as well as JSON
round-trip equality.

## Sources and fixture adaptations

- [WebSocket examples](https://dev.twitch.tv/docs/eventsub/handling-websocket-events/)
- [Subscription examples and versions](https://dev.twitch.tv/docs/eventsub/eventsub-subscription-types/)
- [Event field definitions](https://dev.twitch.tv/docs/eventsub/eventsub-reference/)

Exact section URLs are cited in `tests/golden.rs`. Fixtures were retrieved on
2026-10-19. Event bodies are unchanged; explanatory JSON comments were removed.
The subscription examples are wrapped in deterministic notification metadata and
their transport changed to WebSocket with a synthetic session ID. The revocation
example intentionally retains Twitch's old follow v1 subscription: revocations do
not decode event bodies. Edge cases generated in tests are explicitly synthetic.

`python tests/fixtures/refresh.py` refreshes examples from those official pages;
it is a maintainer operation requiring network access. Tests use only checked-in
fixtures and require no Twitch account, token, or network after Cargo dependencies
are cached. Review fixture changes if upstream docs change.

Dependencies: serde 1.0.228 and serde_json 1.0.145 (MIT OR Apache-2.0).
Cargo.lock pins transitive dependencies. App code: GPL-3.0-or-later.
