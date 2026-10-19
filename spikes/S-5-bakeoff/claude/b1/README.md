# S-5 bake-off B-1 — Claude (Opus 5.5): bounded action queue

Throwaway spike crate for card DEN-8. It is not production code; DEN-15 (Spec 03, engine core) decides the real design.

```sh
cargo test                       # 15 unit/integration tests + 1 proptest (512 cases)
cargo clippy -- -D warnings      # also clean with --all-targets
```

## API in one screen

```rust
let reg = QueueRegistry::new(NonZeroUsize::new(8).unwrap()); // has a non-blocking "Default" queue
let alerts = reg.create(QueueConfig::blocking("alerts"))?;
let h = alerts.enqueue(EnqueueOptions { always_run: false }, |ctx| async move { ctx.id() })?;
h.canceller().cancel();                 // a pending run never starts; a running run is dropped
match h.outcome().await { RunOutcome::Completed(_) | RunOutcome::Cancelled
                        | RunOutcome::Cleared | RunOutcome::Aborted => {} }
alerts.pause(); alerts.resume(); alerts.clear(); alerts.set_mode(QueueMode::Blocking);
reg.pause_all(); reg.resume_all(); alerts.stats();
```

## Design
- **No dispatcher task, no polling.** One `std::sync::Mutex` per queue protects the state. Every event that can
  start a run (enqueue, resume, mode change, run finished) calls `pump`, which pops the eligible runs under the
  lock and spawns them after unlocking. No user code runs under the lock. Only one pump loop runs per queue;
  nested or concurrent calls return at once and the active loop re-reads the state before stopping, so the stack
  stays flat even when a shutting-down runtime drops spawned futures synchronously.
- **Slots can't leak.** Each running task owns a `FinishGuard` whose `Drop` frees the slot. Job panics are caught
  around every poll, so the counters update before `Aborted` is sent. A runtime shutdown also reports `Aborted`.
- **Cancellation** uses a `CancellationToken` per run. A pending run is removed from the queue, and its factory
  closure is never called. A running run's future is dropped through `select!`. The token is also exposed to
  the job through `RunContext` for cooperative checks.
- **Bounded:** each queue has a pending-run `capacity` (default 1024). `enqueue` returns `EnqueueError::Full`
  beyond it. Running runs don't count.
- `RunContext::dispatch_seq` is assigned under the lock, so tests can check FIFO dispatch independently of how
  Tokio orders task polls.

## Assumptions (my reading of the Streamer.bot semantics; see the handoff on DEN-8)
1. **Always run** bypasses *pause* only. It still respects the concurrency limit (a paused blocking queue runs
   always-run actions one at a time) and keeps FIFO order among always-run runs, overtaking held normal runs.
2. **Clear** drops every pending run, including always-run ones. Running runs continue.
3. Cancelled and cleared runs both count as `cancelled`. Streamer.bot's "Completed Count" includes cancelled
   runs. Here `completed` counts only runs that finished, and `completed + cancelled + aborted` gives the
   Streamer.bot figure.
4. Queue names are case-sensitive and exact. The `Default` queue can't be removed. Removing a queue from the
   registry doesn't stop running runs. Its pending runs resolve as `Aborted` once the last reference to the
   queue is gone.
5. Dropping a `RunHandle` does **not** cancel the run (fire-and-forget triggers).
6. Lowering the concurrency limit never interrupts running runs.

## Tests
- `tests/queue.rs`: blocking FIFO and timing, the non-blocking limit and waves, pause/resume, always-run under
  pause, clear, cancelling queued and running runs, cancel after completion, capacity, a panicking job, raising the
  limit at runtime, a multi-thread stress test (8 producers × 50 runs, limit 4), runtime shutdown with a
  20,000-run backlog (a regression test for a stack overflow found in self-review), and the registry.
- `tests/properties.rs` (proptest, paused clock, so it's deterministic): random limit 1–4, up to 24 jobs with
  random durations, always-run flags and cancel times, plus an optional pause window and an optional clear. It
  checks the peak concurrency against the limit, the counters against the outcomes, outcome-versus-started
  consistency, FIFO dispatch (overall when never paused, and per class otherwise), and that no normal run starts
  inside the pause window.
  A mutation check confirmed it catches both an off-by-one in the limit and a pause that's ignored.
