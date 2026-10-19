# DEN-8 b1 — codex, reviewed by claude

Reviewer: Fabiana (claude / claude-opus-5-5), 2026-10-19. Target: `spikes/S-5-bakeoff/codex/b1` on
`agent/codex/DEN-8-b1` @ `363e663` (`git diff origin/main...HEAD`: 5 files, +800, all inside the card's scope).

Done-when: PASS — run in `spikes/S-5-bakeoff/codex/b1` with `--locked` and `CARGO_TARGET_DIR=/tmp/...`, so the repo stayed clean:

```text
$ cargo test --locked
running 6 tests
test tests::final_handle_drop_aborts_active_job ... ok
test tests::always_run_bypasses_pause_without_bypassing_serial_limit ... ok
test tests::pause_full_cancel_clear_resume ... ok
test tests::panic_releases_slot_and_names_are_independent ... ok
test tests::parallel_limit_and_clear_preserve_active_runs ... ok
test tests::blocking_fifo_survives_arbitrary_queued_cancellations ... ok   <- proptest
test result: ok. 6 passed; 0 failed; ... Doc-tests: 0 tests, ok        (exit 0)
$ cargo clippy --locked -- -D warnings                (exit 0)
$ cargo clippy --locked --all-targets -- -D warnings  (exit 0)
$ cargo fmt --check                                   (exit 0)
```

## Findings

### P0
None.

### P1
None.

### P2

1. **An always-run job is rejected with `Full` on a paused queue that has free run slots.** `src/lib.rs:148` (`if pending.len() == capacity`)
   - Failure scenario: I reproduced this in a scratch copy. Use `NonBlocking(4)` with capacity 2, call `set_paused(true)`, and submit two ordinary jobs. Then `submit(true, ..)` returns `Err(Full)`, even with 0 active runs and 4 free slots. The always-run job would never wait, yet it's still counted against *waiting* capacity. A paused queue whose backlog is full blocks exactly the jobs that are meant to get past the pause. The same check gives a brief false `Full` when a run has finished but `join_next` hasn't reaped it yet, because `active.len()` still counts finished tasks.
   - Suggested fix: check capacity only when the new run actually has to wait. Admit it if `active.len() < limit && (!paused || always_run)` and start it right away. Optionally, reap ready tasks (`try_join_next` loop) before the check.

2. **"Named queues" are only labels, with no registry.** `src/lib.rs:59-78`, `README.md:6-7`
   - Failure scenario: the card's goal follows the Streamer.bot Queues model, where actions refer to a queue by name. Here `Queue::new("alerts", ..)` twice gives two independent schedulers. Nothing looks a queue up by name, rejects a duplicate, or pauses/resumes all queues. Two actions configured with queue "alerts" would run concurrently despite Blocking mode if each built its own handle. The README states the assumption, but the requirement is only half met (the Claude and Grok entries both have a registry).
   - Suggested fix: add a small `Queues` registry (`create(name, mode, cap) -> Result<Queue, Duplicate>`, `get(name)`, `pause_all`/`resume_all`).

### P3

3. **Shutdown, panic and abort all look the same.** `src/lib.rs:29-31,41`: `Outcome::Failed` covers a job panic, a dropped scheduler, and an active job aborted when the last handle drops. `clear` and `cancel` both give `Cancelled`. A caller can't tell "my action crashed" (worth logging or alerting) from "the app is shutting down". Fix: split it into `Panicked` and `Aborted` (the panic can be read from the `JoinError` in `join_next`), and optionally add `Cleared`.

4. **Tickets don't keep the queue alive, which is a lifetime footgun.** `src/lib.rs:186-189`, `README.md:19`: `Queue::new(..).submit(..).await?.wait().await` gives `Cancelled` or `Failed`, because the temporary handle drops and the scheduler shuts down. It's documented, but it's easy to hit with a builder-style call. Fix: have `Ticket` hold a weak or strong sender clone, or return an error when you submit through a handle that is about to drop.

5. **`Queue::new` panics with no Tokio runtime.** `src/lib.rs:73` (`tokio::spawn`). A library constructor should return `Result` or take a `Handle`. Fix: `tokio::runtime::Handle::try_current()` and return an `Error`.

6. **Mode can't be changed after construction.** `src/lib.rs:13-17,67-72`: there's no `set_mode` or limit change. Streamer.bot lets you switch a queue between blocking and non-blocking. The card doesn't require it, but nothing in the API supports it. Fix: add a `Command::SetMode` that changes `limit` without preempting active runs.

7. **The property test is narrow.** `src/tests.rs:159-183`: the one proptest checks only Blocking FIFO after queued cancellations while paused. It doesn't check the concurrency limit (peak ≤ limit), always-run under pause, clear, or cancels that race with dispatch, which is where the scheduler is most likely to break. It does meet the "at least one proptest" bar. Fix: build a randomized workload (limit 1–4, always-run flags, a pause window, a clear) and assert peak ≤ limit, no ordinary start while paused, and outcome counts.

8. **Some negative assertions depend on timing.** `src/tests.rs:102,113` (`assert!(seen.try_recv().is_err())` right after submit, on a multi-thread runtime) would pass even if a spawned run just hadn't been polled yet. I mutation-checked `always_run_bypasses_pause_without_bypassing_serial_limit` by making always-run ignore the limit, and it does fail correctly through its later assertions, so this is a weakness, not a false pass. A paused clock or explicit gates would make these assertions meaningful.

Notes (no action needed): the design is event-driven (one scheduler task, `select!` over the mpsc mailbox plus `JoinSet`, no polling), panics release their slot, abandoned submits aren't enqueued, deps are pinned and GPL-compatible (tokio MIT, proptest MIT/Apache-2.0), and the diff is about 420 lines excluding Cargo.lock.

## Quality 1-5

**3.** It's compact and correct for the common paths, event-driven, and every Done-when passes. But the always-run admission bug (P2-1), named queues that are only labels (P2-2), and a thin property test leave it behind a production-grade queue.
