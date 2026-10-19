# DEN-9 b2 — grok, reviewed by claude

Reviewer: Claude (claude-opus-5-5, agent Delia), 2026-10-19. Scope: `spikes/S-5-bakeoff/grok/b2/**` on
`agent/grok/DEN-9-b2` @ `8b0e290` (`git diff origin/main...HEAD`: 28 files, +1861, all inside scope). Read-only review.

Done-when: PASS. Run in `spikes/S-5-bakeoff/grok/b2`:
- `cargo test`: `every_fixture_round_trips_and_keeps_unknowns ... ok`, 1 passed, 0 failed. That one test loops over all 20 fixtures. Each must give exact JSON equality after parse → serialize → parse, and each has a per-fixture variant assertion.
- `cargo clippy -- -D warnings`: clean (`Finished dev profile`, no warnings).
- Extra: `cargo clippy --all-targets -- -D warnings` clean. `cargo fmt --check` clean (exit 0). Worktree clean apart from the ignored `target/`.

The Goal coverage is complete. All 5 message types are typed. All 9 notification types are typed: chat.message, follow v2 (also v1), subscribe, gift, subscription.message, cheer, raid, redemption.add, and hype_train.begin v2. An unknown message type goes to `Message::Unknown`. An unknown subscription type or version goes to `EventBody::Raw`. Fixtures 19 and 20 test that both are kept, not dropped.

## Findings

### P1 — `deny_unknown_fields` on every wire and event struct turns ordinary Twitch schema growth into a hard parse error, and the frame is lost
- **Where:** `src/wire.rs:6,19,34,51,63`, every struct in `src/events.rs` (for example `:105`, `:186`), and the error path at `src/lib.rs:122-125` and `src/events.rs:74-79`.
- **Failure scenario:** Twitch adds response fields without bumping the subscription version, and its docs say clients must tolerate new fields. I checked these cases against this crate in a scratch crate outside the repo:
  - `session_welcome` with `"recovery_url": null` added to `payload.session` → `Err(invalid JSON: unknown field 'recovery_url')`. Live welcome frames carry this key (the Twitch CLI mock server sends it too). If so, the very first frame of every real connection fails.
  - `channel.chat.message` with `"channel_points_animation_id": null` (a field in the current schema) → `Err(ParseError::Event …)`. The notification is gone, not even returned raw.
  - `transport` with `connected_at`, or an emote fragment with one extra key → `Err`.

  The result is that a production multichat would drop every chat message after a non-breaking Twitch change. Typed strictness was meant to catch a "drifted fixture". Here it costs liveness. The card's "never dropped" promise only holds for unknown *types*. A known type with one new field is dropped.
- **Suggested fix:** Remove `deny_unknown_fields`. Add `#[serde(flatten)] pub extra: Map<String, Value>` to each struct so unknown keys still round-trip exactly. In the golden test, assert `extra.is_empty()` so fixtures still prove full coverage. Separately, when a known type fails typed parsing, return the event as `EventBody::Raw` with the error attached, for example `Raw { value, parse_error: Option<String> }`, not `Err`. Then a bad body never discards the metadata or subscription.

### P2 — `channel.chat.message` is typed for any version, unlike every other type
- **Where:** `src/events.rs:49` (`("channel.chat.message", _)`).
- **Failure scenario:** The crate-level rule (`src/events.rs:9-11`, `src/lib.rs:42-43`) is that a known name on an unmodelled version is Raw. Chat breaks that rule. A future `channel.chat.message` v2 whose body differs gets forced into the v1 struct. Verified: v2 with the v1 body is typed silently, and v2 with any new field → `Err`. That is the exact case the Raw fallback exists for, and it hits the highest-volume event.
- **Suggested fix:** Match `("channel.chat.message", "1")` and let other versions fall through to `Raw`.

### P2 — Fixture provenance is weaker than the card asks for
- **Where:** `tests/fixtures/01-18*.json`, `tests/golden.rs:3-21`, and the handoff ("docs returned HTTP 403 … transcribed from search excerpts").
- **Failure scenario:** The card asks for the official doc example payloads. The fixtures are a reconstruction from search snippets. Their envelope `metadata` was synthesized: `message_id` and `message_timestamp` reuse the subscription id and `created_at`. At least one value has known type uncertainty (fixture 18 emote id `"81274"` string vs number, from the handoff). The golden test only proves the parser is self-consistent with these files, not with what Twitch sends. That is why the P1 drift (for example `recovery_url`) went unnoticed. Citing one URL per group of fixtures, with no per-file mapping, makes them hard to re-verify.
- **Suggested fix:** Re-fetch the pages, using a browser if the fetch tool gets a 403. Diff each event body against its doc example and add the source URL plus section anchor to each fixture name or a `SOURCES.md`. Mark synthesized envelope fields as such. Add at least one frame captured from `twitch event websocket start-server` (Twitch CLI) as a real-shape fixture.

### P3 — All 20 golden checks sit in one `#[test]`
- **Where:** `tests/golden.rs:29-51`.
- **Failure scenario:** The first failing fixture panics and hides the state of the other 19, and `cargo test` reports "1 passed" for a "golden test per fixture" requirement. That makes regressions and reviewer evidence harder to read.
- **Suggested fix:** Generate one test per fixture (a small `macro_rules!` list, or a `datatest`-style harness), or collect failures and assert once at the end.

### P3 — Numeric and enum fields are untyped strings or `i64` with no validation
- **Where:** `src/events.rs` (`tier: String`, `status: String`, `message_type: String`, `train_type: String`, `fragment_type: String`). Timestamps are `String`.
- **Failure scenario:** This is not a bug, since it keeps round-trips exact. But consumers must string-match `"1000"`/`"2000"`/`"3000"` and `"text"`/`"emote"` themselves, which gives up most of the "typed structs" value for these fields.
- **Suggested fix:** Use string enums with an `Other(String)` fallback (serialize back verbatim). That keeps round-trip exactness and the "never dropped" rule.

Non-findings, checked:
- The `Nullable<T>` with the custom deserializer correctly keeps "absent" separate from explicit `null` for the shared-chat fields.
- `Message`'s manual `Serialize`/`Deserialize` is sound.
- Dependencies are serde and serde_json only (MIT OR Apache-2.0, GPL-compatible).
- Crate license is GPL-3.0-or-later and `publish = false`. There are no out-of-scope paths.

## Quality 1-5

**3**: The code is clean, small and idiomatic, with complete type coverage, exact golden round-trips and correct Unknown/Raw handling for new types. The strict `deny_unknown_fields` plus `Err`-on-mismatch design would drop real Twitch frames as soon as a field is added (likely already on `session_welcome`), and the fixtures are reconstructions rather than verified doc copies.
