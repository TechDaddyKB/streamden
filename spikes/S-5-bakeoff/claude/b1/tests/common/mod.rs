#![allow(dead_code)]

use std::future::Future;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use den_action_queue_b1::{RunContext, RunId};
use tokio::time::Instant;

/// Records which runs started, in dispatch order, and the peak number running at once.
#[derive(Default)]
pub struct Tracker {
    active: AtomicUsize,
    peak: AtomicUsize,
    /// (dispatch_seq, run id, start instant)
    starts: Mutex<Vec<(u64, RunId, Instant)>>,
}

impl Tracker {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub fn peak(&self) -> usize {
        self.peak.load(Ordering::SeqCst)
    }

    pub fn active(&self) -> usize {
        self.active.load(Ordering::SeqCst)
    }

    /// Started run ids in dispatch order.
    pub fn started_ids(&self) -> Vec<RunId> {
        self.starts_sorted()
            .into_iter()
            .map(|(_, id, _)| id)
            .collect()
    }

    pub fn starts_sorted(&self) -> Vec<(u64, RunId, Instant)> {
        let mut starts = self.starts.lock().unwrap().clone();
        starts.sort_by_key(|s| s.0);
        starts
    }
}

struct ActiveGuard(Arc<Tracker>);

impl Drop for ActiveGuard {
    fn drop(&mut self) {
        self.0.active.fetch_sub(1, Ordering::SeqCst);
    }
}

/// A job that sleeps for `dur` (returning its run id) while being tracked. The active
/// count is decremented on completion and when the future is dropped by cancellation.
pub fn job(
    tracker: &Arc<Tracker>,
    dur: Duration,
) -> impl FnOnce(RunContext) -> std::pin::Pin<Box<dyn Future<Output = RunId> + Send>> + Send + 'static
{
    let tracker = Arc::clone(tracker);
    move |ctx: RunContext| {
        let now = tracker.active.fetch_add(1, Ordering::SeqCst) + 1;
        tracker.peak.fetch_max(now, Ordering::SeqCst);
        tracker
            .starts
            .lock()
            .unwrap()
            .push((ctx.dispatch_seq(), ctx.id(), Instant::now()));
        let guard = ActiveGuard(tracker);
        Box::pin(async move {
            let _guard = guard;
            tokio::time::sleep(dur).await;
            ctx.id()
        })
    }
}

pub fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}
