# B-1 action queue (Grok)

Standalone spike crate for DEN-8. It is not part of the production workspace.

```bash
cargo test
cargo clippy -- -D warnings
```

Run both from this directory.

Spike assumptions (not product decisions):

- Each named queue is independent. Nothing is auto-created.
- `pending_capacity` bounds runs that have not started. A full queue returns `QueueError::Full`. In-flight runs do not consume that capacity.
- Blocking mode is FIFO with concurrency 1. Non-blocking mode admits the oldest runnable runs up to its concurrency limit.
- Pause holds non-`always_run` runs in place. `always_run` may start while paused, including ahead of older paused runs, and still uses a concurrency slot.
- Clear drops waiting runs and leaves in-flight runs alone. Completed counts include cancelled runs.
- Cancelling a waiting run removes it. Cancelling an in-flight run aborts that task. Dropping a `RunHandle` does not cancel.
- Dropping the last `ActionQueues` handle cancels waiting runs and aborts in-flight ones.
- A panicked job frees its slot and is reported as `RunOutcome::Panicked`.
