//! Injectable clocks, wake-time suspend detection and explicit runtime bounds (Spec 03 §4.2).

use std::{num::NonZeroUsize, time::Duration};

use den_protocol::Timestamp;
use tokio::{runtime::Runtime, time::Instant};

/// Time sources used by the engine. Tokio's monotonic clock supports paused tests.
pub trait Clock {
    fn monotonic(&self) -> Instant;
    fn wall(&self) -> Timestamp;

    fn sample(&self) -> ClockSample {
        ClockSample {
            monotonic: self.monotonic(),
            wall: self.wall(),
        }
    }
}

/// Production time sources; constructing this value starts no tasks or runtime.
#[derive(Debug, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn monotonic(&self) -> Instant {
        Instant::now()
    }

    fn wall(&self) -> Timestamp {
        Timestamp::now()
    }
}

/// A pair of readings, also directly injectable for deterministic suspend tests.
#[derive(Debug, Clone, Copy)]
pub struct ClockSample {
    pub monotonic: Instant,
    pub wall: Timestamp,
}

/// Called by the scheduler on wake; owns no timer, task or polling loop.
#[derive(Debug)]
pub struct SuspendDetector {
    previous: ClockSample,
    tolerance: Duration,
}

impl SuspendDetector {
    /// Caller policy supplies tolerance, including wall-clock millisecond quantization and sample skew.
    pub fn new(initial: ClockSample, tolerance: Duration) -> Self {
        Self {
            previous: initial,
            tolerance,
        }
    }

    /// Flags wall time advancing farther than monotonic time by more than tolerance.
    /// A forward clock correction is indistinguishable from suspend here. Backward wall readings
    /// do not flag suspend. Every reading rebases the detector, so jumps are reported once.
    pub fn on_wake(&mut self, sample: ClockSample) -> bool {
        let previous = std::mem::replace(&mut self.previous, sample);
        let Some(monotonic_delta) = sample.monotonic.checked_duration_since(previous.monotonic)
        else {
            return false;
        };
        let wall_nanos = (i128::from(sample.wall.unix_millis())
            - i128::from(previous.wall.unix_millis()))
            * 1_000_000;
        wall_nanos > 0
            && (wall_nanos as u128) > monotonic_delta.as_nanos() + self.tolerance.as_nanos()
    }
}

/// Builds only when called. Both thread limits are mandatory, nonzero caller policy.
/// The blocking cap bounds running threads, not Tokio's pending blocking-job queue;
/// producers must bound admission separately. No product defaults are selected here.
pub fn build_runtime(
    worker_threads: NonZeroUsize,
    max_blocking_threads: NonZeroUsize,
) -> std::io::Result<Runtime> {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(worker_threads.get())
        .max_blocking_threads(max_blocking_threads.get())
        .enable_all()
        .build()
}
