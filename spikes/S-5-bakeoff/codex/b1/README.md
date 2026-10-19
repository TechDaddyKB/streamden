# DEN-8 · Codex B-1 queue spike

Standalone, clean-room Rust/Tokio library; no production workspace or protocol changes.
Run `cargo test` and `cargo clippy -- -D warnings` in this directory.

Each `Queue` has a name and independent scheduler. Clone the handle to share that queue;
constructing another handle with the same name creates a separate queue, not a registry lookup.
Blocking mode starts one run at a time in submission order. Non-blocking mode starts up to
the configured nonzero limit. Capacity bounds accepted waiting jobs; a full queue returns
`Error::Full`. Active jobs are excluded from waiting capacity. The command mailbox holds
at most 32 commands, with async backpressure on additional callers.

Pause/resume, clear and queued cancellation are acknowledged by the scheduler. Active jobs
continue through these operations. Always-run jobs may pass paused ordinary jobs, preserving
FIFO among eligible jobs, but never bypass capacity or concurrency limits. Clear cancels all
waiting jobs, including always-run jobs. Cancellation after dispatch returns false. IDs are
local to one queue. Ticket outcomes distinguish completion, queued cancellation and failure
(panic or scheduler shutdown). Dropping a ticket does not cancel its job. Dropping the last
queue handle cancels waiting jobs and aborts active jobs; tickets do not keep the queue alive.
Actions must be cooperative async futures; blocking CPU work belongs on a bounded pool.

One scheduler owns all state, receives commands and waits for running tasks through a
`JoinSet`; there are no polling loops. A panicked action releases its concurrency slot.
Cancelling a submit future before acceptance prevents enqueueing; after acceptance the run
may execute even if the caller drops its reply. Retain the returned ticket for cancellation.

Spike defaults: reject on overflow, count only waiting runs against capacity, and treat
always-run as a pause exception only. These are local implementation assumptions for this
bake-off, not decisions about StreamDen's final public contract.

Dependencies: Tokio 1.53.3 (MIT), proptest 1.9.0 (MIT OR Apache-2.0; tests only).
The generated local Cargo.lock pins the complete dependency graph. License: GPL-3.0-or-later.
