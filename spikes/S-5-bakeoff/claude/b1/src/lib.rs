//! S-5 bake-off B-1 (Claude): StreamDen action-queue semantics as a small Tokio library.
//!
//! Model (from `docs/research/streamerbot-engine-ui.md` §2.4, Queues):
//! - Named queues; a non-blocking `Default` queue always exists ([`QueueRegistry`]).
//! - **Blocking** queues run one action at a time, FIFO. **Non-blocking** queues start runs
//!   in FIFO order and run them in parallel up to a concurrency limit.
//! - Pause, resume and clear per queue, plus pause-all and resume-all.
//! - An *always run* flag lets a run start while its queue is paused.
//! - Queued runs can be cancelled (they never start); running runs can be cancelled too.
//! - Pending runs are bounded by a per-queue capacity.

mod queue;
mod registry;

pub use queue::{
    ActionQueue, CancelHandle, EnqueueError, EnqueueOptions, QueueConfig, QueueMode, QueueStats,
    RunContext, RunHandle, RunId, RunOutcome,
};
pub use registry::{QueueRegistry, RegistryError, DEFAULT_QUEUE};
