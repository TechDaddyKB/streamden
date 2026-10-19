//! A single named action queue.

use std::collections::VecDeque;
use std::fmt;
use std::future::Future;
use std::num::NonZeroUsize;
use std::panic::{self, AssertUnwindSafe};
use std::pin::Pin;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, Weak};
use std::task::{Context, Poll};

use tokio::runtime::Handle;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

/// Identifier of one run, unique within its queue and increasing in enqueue order.
pub type RunId = u64;

/// How many runs of a queue may execute at the same time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueueMode {
    /// One run at a time, strictly FIFO (the canonical alert queue).
    Blocking,
    /// Runs start in FIFO order and execute in parallel, up to `max_concurrency` at once.
    NonBlocking { max_concurrency: NonZeroUsize },
}

impl QueueMode {
    /// Maximum number of runs executing at once in this mode.
    pub fn concurrency_limit(self) -> usize {
        match self {
            QueueMode::Blocking => 1,
            QueueMode::NonBlocking { max_concurrency } => max_concurrency.get(),
        }
    }
}

/// Static configuration of a queue.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueueConfig {
    pub name: String,
    pub mode: QueueMode,
    /// Maximum number of *pending* (not yet started) runs. Enqueueing beyond it fails with
    /// [`EnqueueError::Full`], which keeps memory bounded when a trigger floods the queue.
    pub capacity: NonZeroUsize,
}

impl QueueConfig {
    /// Pending-run capacity used by the constructors below.
    pub const DEFAULT_CAPACITY: NonZeroUsize = match NonZeroUsize::new(1024) {
        Some(n) => n,
        None => unreachable!(),
    };

    pub fn blocking(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            mode: QueueMode::Blocking,
            capacity: Self::DEFAULT_CAPACITY,
        }
    }

    pub fn non_blocking(name: impl Into<String>, max_concurrency: NonZeroUsize) -> Self {
        Self {
            name: name.into(),
            mode: QueueMode::NonBlocking { max_concurrency },
            capacity: Self::DEFAULT_CAPACITY,
        }
    }

    pub fn with_capacity(mut self, capacity: NonZeroUsize) -> Self {
        self.capacity = capacity;
        self
    }
}

/// Per-run options.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EnqueueOptions {
    /// Start this run even while the queue is paused. It still waits for a free
    /// concurrency slot and keeps FIFO order relative to other always-run runs.
    pub always_run: bool,
}

/// How a run ended.
#[derive(Debug, PartialEq, Eq)]
pub enum RunOutcome<T> {
    /// The job ran to completion and returned this value.
    Completed(T),
    /// The run was cancelled through its [`CancelHandle`], before or while running.
    Cancelled,
    /// The run was still pending when the queue was cleared.
    Cleared,
    /// The job panicked, or the run was dropped unfinished: its runtime shut down, or the
    /// last reference to its queue went away while it was pending.
    Aborted,
}

/// Why [`ActionQueue::enqueue`] refused a run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EnqueueError {
    Full { queue: String, capacity: usize },
}

impl fmt::Display for EnqueueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EnqueueError::Full { queue, capacity } => {
                write!(f, "queue {queue:?} is full ({capacity} pending runs)")
            }
        }
    }
}

impl std::error::Error for EnqueueError {}

/// Point-in-time counters of a queue (the Streamer.bot queue list columns, plus running).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QueueStats {
    pub mode: QueueMode,
    pub paused: bool,
    pub pending: usize,
    pub running: usize,
    pub completed: u64,
    /// Cancelled or cleared runs.
    pub cancelled: u64,
    pub aborted: u64,
}

/// Handed to a job when it starts.
#[derive(Clone, Debug)]
pub struct RunContext {
    id: RunId,
    dispatch_seq: u64,
    token: CancellationToken,
}

impl RunContext {
    pub fn id(&self) -> RunId {
        self.id
    }

    /// Position of this run in the queue's dispatch order (0, 1, 2, …). Assigned under the
    /// queue lock, so it reflects scheduling order even when task polling is reordered.
    pub fn dispatch_seq(&self) -> u64 {
        self.dispatch_seq
    }

    /// Cancelled when the run is cancelled. The job future is dropped at its next `.await`
    /// anyway; long synchronous sections can poll this to stop early.
    pub fn cancellation_token(&self) -> &CancellationToken {
        &self.token
    }
}

type BoxFuture = Pin<Box<dyn Future<Output = ()> + Send>>;

/// Type-erased pending job.
trait Job: Send {
    fn start(self: Box<Self>, ctx: RunContext, guard: FinishGuard) -> BoxFuture;
    fn reject(self: Box<Self>, outcome: Rejection);
}

enum Rejection {
    Cancelled,
    Cleared,
}

struct TypedJob<F, T> {
    factory: F,
    tx: oneshot::Sender<RunOutcome<T>>,
}

impl<F, Fut, T> Job for TypedJob<F, T>
where
    F: FnOnce(RunContext) -> Fut + Send + 'static,
    Fut: Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    fn start(self: Box<Self>, ctx: RunContext, mut guard: FinishGuard) -> BoxFuture {
        let TypedJob { factory, tx } = *self;
        Box::pin(async move {
            let token = ctx.token.clone();
            // Cancelled between dispatch and first poll: never call the factory.
            let outcome = if token.is_cancelled() {
                RunOutcome::Cancelled
            } else {
                let run = CatchUnwind(Box::pin(async move { factory(ctx).await }));
                tokio::select! {
                    biased;
                    _ = token.cancelled() => RunOutcome::Cancelled,
                    result = run => match result {
                        Ok(value) => RunOutcome::Completed(value),
                        Err(()) => RunOutcome::Aborted,
                    },
                }
            };
            guard.finish(match outcome {
                RunOutcome::Completed(_) => Finish::Completed,
                RunOutcome::Aborted => Finish::Aborted,
                _ => Finish::Cancelled,
            });
            // Counters are updated before the outcome is observable.
            let _ = tx.send(outcome);
        })
    }

    fn reject(self: Box<Self>, outcome: Rejection) {
        let _ = self.tx.send(match outcome {
            Rejection::Cancelled => RunOutcome::Cancelled,
            Rejection::Cleared => RunOutcome::Cleared,
        });
    }
}

/// Turns a panic in the job (factory call or any poll) into `Err(())`, so the run is
/// accounted as aborted *before* its outcome is sent. Tokio would catch the panic too, but
/// then the order of dropping the guard and the sender during unwinding is unspecified.
struct CatchUnwind<T>(Pin<Box<dyn Future<Output = T> + Send>>);

impl<T> Future for CatchUnwind<T> {
    type Output = Result<T, ()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let fut = self.0.as_mut();
        match panic::catch_unwind(AssertUnwindSafe(|| fut.poll(cx))) {
            Ok(Poll::Pending) => Poll::Pending,
            Ok(Poll::Ready(value)) => Poll::Ready(Ok(value)),
            Err(_) => Poll::Ready(Err(())),
        }
    }
}

#[derive(Clone, Copy)]
enum Finish {
    Completed,
    Cancelled,
    Aborted,
}

/// Owned by a running task. Frees the concurrency slot exactly once, also on panic or when
/// the task is dropped, so a misbehaving job can never wedge the queue.
struct FinishGuard {
    inner: Arc<Inner>,
    done: bool,
}

impl FinishGuard {
    fn finish(&mut self, how: Finish) {
        if !self.done {
            self.done = true;
            self.inner.on_finish(how);
        }
    }
}

impl Drop for FinishGuard {
    fn drop(&mut self) {
        self.finish(Finish::Aborted);
    }
}

struct Pending {
    id: RunId,
    always_run: bool,
    token: CancellationToken,
    job: Box<dyn Job>,
}

struct State {
    mode: QueueMode,
    capacity: NonZeroUsize,
    paused: bool,
    pending: VecDeque<Pending>,
    running: usize,
    next_id: RunId,
    next_dispatch: u64,
    completed: u64,
    cancelled: u64,
    aborted: u64,
    /// A pump loop is active (see [`Inner::pump`]).
    pumping: bool,
}

struct Inner {
    name: String,
    runtime: Handle,
    state: Mutex<State>,
}

impl Inner {
    fn lock(&self) -> MutexGuard<'_, State> {
        // No user code runs under this lock, so poisoning can only come from a bug here;
        // the state stays consistent at every unlock point, so keep going.
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Starts every eligible pending run. Called after each state change that can free a
    /// slot or make a run eligible; there is no dispatcher task and no polling.
    ///
    /// Only one pump runs per queue at a time. A nested or concurrent call returns at once:
    /// the active pump re-reads the state under the lock before it stops, so the change is
    /// never lost. This keeps the stack flat when `spawn` drops futures synchronously
    /// (runtime shutdown), which would otherwise recurse once per pending run.
    fn pump(self: &Arc<Self>) {
        {
            let mut st = self.lock();
            if st.pumping {
                return;
            }
            st.pumping = true;
        }
        loop {
            let ready = {
                let mut st = self.lock();
                let ready = Self::take_ready(&mut st);
                if ready.is_empty() {
                    st.pumping = false;
                    return;
                }
                ready
            };
            for (run, dispatch_seq) in ready {
                let guard = FinishGuard {
                    inner: Arc::clone(self),
                    done: false,
                };
                let ctx = RunContext {
                    id: run.id,
                    dispatch_seq,
                    token: run.token,
                };
                drop(self.runtime.spawn(run.job.start(ctx, guard)));
            }
        }
    }

    /// Pops the runs that may start now and reserves their slots.
    fn take_ready(st: &mut State) -> Vec<(Pending, u64)> {
        let mut ready = Vec::new();
        while st.running < st.mode.concurrency_limit() {
            let next = if st.paused {
                st.pending.iter().position(|p| p.always_run)
            } else if st.pending.is_empty() {
                None
            } else {
                Some(0)
            };
            let Some(index) = next else { break };
            let Some(run) = st.pending.remove(index) else {
                break;
            };
            st.running += 1;
            let seq = st.next_dispatch;
            st.next_dispatch += 1;
            ready.push((run, seq));
        }
        ready
    }

    fn on_finish(self: &Arc<Self>, how: Finish) {
        {
            let mut st = self.lock();
            st.running -= 1;
            match how {
                Finish::Completed => st.completed += 1,
                Finish::Cancelled => st.cancelled += 1,
                Finish::Aborted => st.aborted += 1,
            }
        }
        self.pump();
    }

    fn cancel_pending(&self, id: RunId) {
        let removed = {
            let mut st = self.lock();
            let index = st.pending.iter().position(|p| p.id == id);
            let removed = index.and_then(|i| st.pending.remove(i));
            if removed.is_some() {
                st.cancelled += 1;
            }
            removed
        };
        // Reject (and drop the user's closure) outside the lock.
        if let Some(run) = removed {
            run.job.reject(Rejection::Cancelled);
        }
    }
}

/// A named action queue. Cheap to clone; clones share the same queue.
///
/// Dispatch is event-driven: enqueue, resume, mode changes and run completion each start
/// whatever has become eligible. Runs always start in FIFO order; while paused, only
/// always-run runs start (FIFO among themselves).
#[derive(Clone)]
pub struct ActionQueue {
    inner: Arc<Inner>,
}

impl fmt::Debug for ActionQueue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ActionQueue")
            .field("name", &self.inner.name)
            .field("stats", &self.stats())
            .finish()
    }
}

impl ActionQueue {
    /// Creates a queue whose runs are spawned on the current Tokio runtime.
    ///
    /// # Panics
    /// When called outside a Tokio runtime; use [`ActionQueue::with_runtime`] there.
    pub fn new(config: QueueConfig) -> Self {
        Self::with_runtime(config, Handle::current())
    }

    pub fn with_runtime(config: QueueConfig, runtime: Handle) -> Self {
        Self {
            inner: Arc::new(Inner {
                name: config.name,
                runtime,
                state: Mutex::new(State {
                    mode: config.mode,
                    capacity: config.capacity,
                    paused: false,
                    pending: VecDeque::new(),
                    running: 0,
                    next_id: 0,
                    next_dispatch: 0,
                    completed: 0,
                    cancelled: 0,
                    aborted: 0,
                    pumping: false,
                }),
            }),
        }
    }

    pub fn name(&self) -> &str {
        &self.inner.name
    }

    /// Queues a run. `job` is called when the run starts (not now) and receives its
    /// [`RunContext`]. Dropping the returned handle does not cancel the run.
    ///
    /// Pending runs live as long as the queue: when the last [`ActionQueue`] clone is
    /// dropped and nothing is running, they resolve as [`RunOutcome::Aborted`]. A job that
    /// captures a clone of its own queue keeps the queue alive until the run finishes or is
    /// removed.
    pub fn enqueue<F, Fut, T>(
        &self,
        options: EnqueueOptions,
        job: F,
    ) -> Result<RunHandle<T>, EnqueueError>
    where
        F: FnOnce(RunContext) -> Fut + Send + 'static,
        Fut: Future<Output = T> + Send + 'static,
        T: Send + 'static,
    {
        let (tx, rx) = oneshot::channel();
        let token = CancellationToken::new();
        let id = {
            let mut st = self.inner.lock();
            if st.pending.len() >= st.capacity.get() {
                return Err(EnqueueError::Full {
                    queue: self.inner.name.clone(),
                    capacity: st.capacity.get(),
                });
            }
            let id = st.next_id;
            st.next_id += 1;
            st.pending.push_back(Pending {
                id,
                always_run: options.always_run,
                token: token.clone(),
                job: Box::new(TypedJob { factory: job, tx }),
            });
            id
        };
        self.inner.pump();
        Ok(RunHandle {
            cancel: CancelHandle {
                id,
                token,
                queue: Arc::downgrade(&self.inner),
            },
            rx,
        })
    }

    /// Stops starting new runs (always-run runs excepted). Running runs continue.
    pub fn pause(&self) {
        self.inner.lock().paused = true;
    }

    pub fn resume(&self) {
        self.inner.lock().paused = false;
        self.inner.pump();
    }

    pub fn is_paused(&self) -> bool {
        self.inner.lock().paused
    }

    /// Drops every pending run (they resolve as [`RunOutcome::Cleared`]) and returns how
    /// many were dropped. Running runs are not affected.
    pub fn clear(&self) -> usize {
        let drained: Vec<Pending> = {
            let mut st = self.inner.lock();
            let drained: Vec<Pending> = st.pending.drain(..).collect();
            st.cancelled += drained.len() as u64;
            drained
        };
        let count = drained.len();
        for run in drained {
            run.job.reject(Rejection::Cleared);
        }
        count
    }

    pub fn mode(&self) -> QueueMode {
        self.inner.lock().mode
    }

    /// Switches between blocking and non-blocking at runtime. Lowering the limit never
    /// interrupts running runs; it only delays new starts until the count drops below it.
    pub fn set_mode(&self, mode: QueueMode) {
        self.inner.lock().mode = mode;
        self.inner.pump();
    }

    pub fn stats(&self) -> QueueStats {
        let st = self.inner.lock();
        QueueStats {
            mode: st.mode,
            paused: st.paused,
            pending: st.pending.len(),
            running: st.running,
            completed: st.completed,
            cancelled: st.cancelled,
            aborted: st.aborted,
        }
    }
}

/// Cancels one run. Cloneable, so it can be handed to other tasks.
#[derive(Clone, Debug)]
pub struct CancelHandle {
    id: RunId,
    token: CancellationToken,
    queue: Weak<Inner>,
}

impl CancelHandle {
    pub fn id(&self) -> RunId {
        self.id
    }

    /// A pending run is removed and never starts; a running run's future is dropped.
    /// Does nothing if the run already ended. The run's outcome says which case happened.
    pub fn cancel(&self) {
        self.token.cancel();
        if let Some(inner) = self.queue.upgrade() {
            inner.cancel_pending(self.id);
        }
    }
}

/// Returned by [`ActionQueue::enqueue`].
#[derive(Debug)]
pub struct RunHandle<T> {
    cancel: CancelHandle,
    rx: oneshot::Receiver<RunOutcome<T>>,
}

impl<T> RunHandle<T> {
    pub fn id(&self) -> RunId {
        self.cancel.id
    }

    pub fn cancel(&self) {
        self.cancel.cancel();
    }

    pub fn canceller(&self) -> CancelHandle {
        self.cancel.clone()
    }

    /// Waits until the run ends.
    pub async fn outcome(self) -> RunOutcome<T> {
        self.rx.await.unwrap_or(RunOutcome::Aborted)
    }
}
