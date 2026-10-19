# B-2 EventSub parser (Grok)

Standalone spike crate for DEN-9. The StreamDen workspace is created later by DEN-19, so this crate is not a workspace member.

```sh
cargo test
cargo clippy -- -D warnings
```

`tests/golden.rs` round-trips one fixture per file. Fixtures 01–18 are the published Twitch EventSub examples (WebSocket frames, or a notification body inside the documented WebSocket envelope). Fixtures 19–20 are constructed and must parse as raw variants rather than being dropped. URLs are in the test file.
