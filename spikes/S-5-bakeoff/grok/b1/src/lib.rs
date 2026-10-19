//! Bounded action queues for the StreamDen S-5 bake-off (Grok, DEN-8).
//!
//! Semantics follow the queue section of `docs/research/streamerbot-engine-ui.md`:
//! named queues, blocking FIFO or a non-blocking concurrency limit, pause,
//! resume, clear of waiting runs, and an always-run flag that still starts
//! while the queue is paused. This crate is a spike, not `den-core`.

use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tokio::runtime::Handle;
use tokio::sync::{oneshot, watch};
use tokio::task::{AbortHandle, JoinHandle};

/// Cooperative cancel flag passed into a running job.
///
/// [`RunHandle::cancel`] trips every clone. Cancelling is idempotent.
#[derive(Clone, Debug)]
pub struct StopToken {
    rx: watch::Receiver<bool>,
}

impl StopToken {
    pub fn is_cancelled(&self) -> bool {
        *self.rx.borrow()
    }

    pub async fn cancelled(&mut self) {
        loop {
            if *self.rx.borrow() {
                return;
            }
            if self.rx.changed().await.is_err() {
                return;
            }
        }
    }
}

#[derive(Clone, Debug)]
struct StopFlag {
    tx: Arc<watch::Sender<bool>>,
    rx: watch::Receiver<bool>,
}

impl StopFlag {
    fn new() -> Self {
        let (tx, rx) = watch::channel(false);
        Self {
            tx: Arc::new(tx),
            rx,
        }
    }

    fn token(&self) -> StopToken {
        StopToken {
            rx: self.rx.clone(),
        }
    }

    fn cancel(&self) {
        let _ = self.tx.send(true);
    }
}

/// How many runs a queue may execute at once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueMode {
    /// One run at a time, in enqueue order.
    Blocking,
    /// Up to `concurrency` runs at once. Admission stays FIFO.
    NonBlocking { concurrency: usize },
}

/// Configuration for one named queue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueConfig {
    pub name: String,
    pub mode: QueueMode,
    /// Maximum number of runs waiting to start.
    pub pending_capacity: usize,
}

/// Why a queue operation was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueueError {
    NotFound,
    AlreadyExists,
    Full,
    Closed,
    NoRuntime,
    InvalidConfig(&'static str),
}

impl fmt::Display for QueueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => write!(f, "queue not found"),
            Self::AlreadyExists => write!(f, "queue already exists"),
            Self::Full => write!(f, "queue pending capacity is full"),
            Self::Closed => write!(f, "action queues are shut down"),
            Self::NoRuntime => write!(f, "a Tokio runtime is required to create a queue"),
            Self::InvalidConfig(reason) => write!(f, "invalid queue config: {reason}"),
        }
    }
}

impl std::error::Error for QueueError {}

/// Terminal state of one enqueued run.
#[derive(Debug)]
pub enum RunOutcome<T> {
    Finished(T),
    Cancelled,
    Panicked,
}

/// Observable counters for one queue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueSnapshot {
    pub name: String,
    pub paused: bool,
    pub blocking: bool,
    pub concurrency: usize,
    pub pending_capacity: usize,
    pub pending: usize,
    pub running: usize,
    pub completed: u64,
    pub cancelled: u64,
}

/// Handle for a run accepted by [`ActionQueues::enqueue`].
///
/// Dropping the handle does not cancel the run. The run stays alive until it
/// finishes, is cancelled, or the last [`ActionQueues`] owner shuts the
/// scheduler down.
#[must_use = "dropping the handle does not cancel the run"]
pub struct RunHandle<T> {
    id: u64,
    rx: oneshot::Receiver<RunOutcome<T>>,
    cancel: Arc<dyn Fn() + Send + Sync>,
}

impl<T> RunHandle<T> {
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Cancel a waiting run, or abort it if it has already started.
    pub fn cancel(&self) {
        (self.cancel)();
    }

    /// Wait until the run finishes, is cancelled, or panics.
    pub async fn wait(self) -> RunOutcome<T> {
        self.rx.await.unwrap_or(RunOutcome::Cancelled)
    }
}

/// Scheduler for independent named action queues.
///
/// Clones share one scheduler. The last clone drops waiting runs and aborts
/// in-flight ones.
#[derive(Clone)]
pub struct ActionQueues {
    shared: Arc<Shared>,
    owner: Arc<Owner>,
}

struct Owner;

struct Shared {
    state: Mutex<State>,
    shutdown: StopFlag,
}

struct State {
    closed: bool,
    next_id: u64,
    queues: HashMap<String, Queue>,
}

struct Queue {
    paused: bool,
    blocking: bool,
    concurrency: usize,
    pending_capacity: usize,
    pending: VecDeque<PendingJob>,
    running: usize,
    completed: u64,
    cancelled: u64,
    aborts: HashMap<u64, AbortHandle>,
}

type LaunchJob = Box<dyn FnOnce(Arc<Shared>, String) -> JoinHandle<()> + Send>;
type CancelJob = Box<dyn FnOnce() + Send>;

struct PendingJob {
    id: u64,
    always_run: bool,
    done: Arc<AtomicBool>,
    on_cancel: Option<CancelJob>,
    launch: Option<LaunchJob>,
}

impl PendingJob {
    fn abandon(mut self) {
        self.launch.take();
        if let Some(cancel) = self.on_cancel.take() {
            cancel();
        }
    }

    fn start(mut self, shared: Arc<Shared>, name: String) -> JoinHandle<()> {
        self.on_cancel.take();
        let launch = self.launch.take().expect("pending job launched once");
        launch(shared, name)
    }
}

struct SlotGuard {
    shared: Arc<Shared>,
    name: String,
    id: u64,
    done: Arc<AtomicBool>,
    active: bool,
}

impl SlotGuard {
    fn finish(&mut self, cancelled: bool) {
        if !self.active {
            return;
        }
        self.active = false;
        let shared = Arc::clone(&self.shared);
        let name = self.name.clone();
        {
            let mut state = lock(&self.shared);
            self.done.store(true, Ordering::Release);
            if let Some(queue) = state.queues.get_mut(&self.name) {
                queue.running = queue.running.saturating_sub(1);
                queue.completed = queue.completed.saturating_add(1);
                if cancelled {
                    queue.cancelled = queue.cancelled.saturating_add(1);
                }
                queue.aborts.remove(&self.id);
            }
        }
        schedule(&shared, &name);
    }
}

impl Drop for SlotGuard {
    fn drop(&mut self) {
        self.finish(true);
    }
}

struct AbortOnDrop<T> {
    handle: Option<JoinHandle<T>>,
}

impl<T> Drop for AbortOnDrop<T> {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            handle.abort();
        }
    }
}

impl Default for ActionQueues {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for ActionQueues {
    fn drop(&mut self) {
        if Arc::strong_count(&self.owner) == 1 {
            self.shared.shutdown_now();
        }
    }
}

impl ActionQueues {
    pub fn new() -> Self {
        Self {
            shared: Arc::new(Shared {
                state: Mutex::new(State {
                    closed: false,
                    next_id: 1,
                    queues: HashMap::new(),
                }),
                shutdown: StopFlag::new(),
            }),
            owner: Arc::new(Owner),
        }
    }

    /// Create a named queue. Call this from a Tokio runtime; runs are spawned on it.
    pub fn create(&self, config: QueueConfig) -> Result<(), QueueError> {
        if config.name.is_empty() {
            return Err(QueueError::InvalidConfig("queue name must not be empty"));
        }
        let (blocking, concurrency) = match config.mode {
            QueueMode::Blocking => (true, 1),
            QueueMode::NonBlocking { concurrency } if concurrency >= 1 => (false, concurrency),
            QueueMode::NonBlocking { .. } => {
                return Err(QueueError::InvalidConfig(
                    "non-blocking concurrency must be at least 1",
                ));
            }
        };
        if config.pending_capacity < 1 {
            return Err(QueueError::InvalidConfig(
                "pending capacity must be at least 1",
            ));
        }
        Handle::try_current().map_err(|_| QueueError::NoRuntime)?;

        let mut state = lock(&self.shared);
        if state.closed {
            return Err(QueueError::Closed);
        }
        if state.queues.contains_key(&config.name) {
            return Err(QueueError::AlreadyExists);
        }
        state.queues.insert(
            config.name,
            Queue {
                paused: false,
                blocking,
                concurrency,
                pending_capacity: config.pending_capacity,
                pending: VecDeque::new(),
                running: 0,
                completed: 0,
                cancelled: 0,
                aborts: HashMap::new(),
            },
        );
        Ok(())
    }

    pub fn pause(&self, name: &str) -> Result<(), QueueError> {
        let mut state = lock(&self.shared);
        let queue = state.queues.get_mut(name).ok_or(QueueError::NotFound)?;
        queue.paused = true;
        Ok(())
    }

    pub fn resume(&self, name: &str) -> Result<(), QueueError> {
        {
            let mut state = lock(&self.shared);
            let queue = state.queues.get_mut(name).ok_or(QueueError::NotFound)?;
            queue.paused = false;
        }
        schedule(&self.shared, name);
        Ok(())
    }

    /// Drop waiting runs. In-flight runs continue and keep their slots.
    pub fn clear(&self, name: &str) -> Result<usize, QueueError> {
        let jobs = {
            let mut state = lock(&self.shared);
            let queue = state.queues.get_mut(name).ok_or(QueueError::NotFound)?;
            let jobs: Vec<PendingJob> = queue.pending.drain(..).collect();
            let count = jobs.len() as u64;
            queue.completed = queue.completed.saturating_add(count);
            queue.cancelled = queue.cancelled.saturating_add(count);
            jobs
        };
        let count = jobs.len();
        for job in jobs {
            job.abandon();
        }
        Ok(count)
    }

    /// Change blocking mode. In-flight runs are not preempted when the limit shrinks.
    pub fn set_mode(&self, name: &str, mode: QueueMode) -> Result<(), QueueError> {
        let (blocking, concurrency) = match mode {
            QueueMode::Blocking => (true, 1),
            QueueMode::NonBlocking { concurrency } if concurrency >= 1 => (false, concurrency),
            QueueMode::NonBlocking { .. } => {
                return Err(QueueError::InvalidConfig(
                    "non-blocking concurrency must be at least 1",
                ));
            }
        };
        {
            let mut state = lock(&self.shared);
            let queue = state.queues.get_mut(name).ok_or(QueueError::NotFound)?;
            queue.blocking = blocking;
            queue.concurrency = concurrency;
        }
        schedule(&self.shared, name);
        Ok(())
    }

    pub fn snapshot(&self, name: &str) -> Result<QueueSnapshot, QueueError> {
        let state = lock(&self.shared);
        let queue = state.queues.get(name).ok_or(QueueError::NotFound)?;
        Ok(QueueSnapshot {
            name: name.to_owned(),
            paused: queue.paused,
            blocking: queue.blocking,
            concurrency: queue.concurrency,
            pending_capacity: queue.pending_capacity,
            pending: queue.pending.len(),
            running: queue.running,
            completed: queue.completed,
            cancelled: queue.cancelled,
        })
    }

    /// Accept one run if the queue has a free waiting slot.
    ///
    /// The future is async so callers compose it with Tokio. Admission itself
    /// does not wait: a full queue returns [`QueueError::Full`].
    #[allow(clippy::unused_async)]
    pub async fn enqueue<T, F, Fut>(
        &self,
        name: &str,
        always_run: bool,
        job: F,
    ) -> Result<RunHandle<T>, QueueError>
    where
        T: Send + 'static,
        F: FnOnce(StopToken) -> Fut + Send + 'static,
        Fut: Future<Output = T> + Send + 'static,
    {
        let token = StopFlag::new();
        let (tx, rx) = oneshot::channel();
        let slot: Arc<Mutex<Option<oneshot::Sender<RunOutcome<T>>>>> =
            Arc::new(Mutex::new(Some(tx)));
        let id = {
            let mut state = lock(&self.shared);
            if state.closed {
                return Err(QueueError::Closed);
            }
            let id = state.next_id;
            state.next_id = state.next_id.saturating_add(1);
            id
        };

        let shared = Arc::clone(&self.shared);
        let queue_name = name.to_owned();
        let cancel_token = token.clone();
        let cancel: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
            cancel_token.cancel();
            shared.cancel_job(&queue_name, id);
        });

        let on_cancel_slot = Arc::clone(&slot);
        let on_cancel: CancelJob = Box::new(move || {
            if let Some(sender) = on_cancel_slot
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .take()
            {
                let _ = sender.send(RunOutcome::Cancelled);
            }
        });

        let done = Arc::new(AtomicBool::new(false));
        let launch_done = Arc::clone(&done);
        let launch_token = token;
        let launch: LaunchJob = Box::new(move |shared, queue_name| {
            // The guard is a future capture, not a local, so dropping the
            // task before it is polled still frees the concurrency slot.
            let guard = SlotGuard {
                shared,
                name: queue_name,
                id,
                done: launch_done,
                active: true,
            };
            tokio::spawn(async move {
                let mut guard = guard;
                let mut stop = launch_token.token();
                let worker_token = stop.clone();
                let mut worker = AbortOnDrop {
                    handle: Some(tokio::spawn(async move { job(worker_token).await })),
                };
                let outcome = tokio::select! {
                    biased;
                    _ = stop.cancelled() => RunOutcome::Cancelled,
                    joined = async { worker.handle.as_mut().expect("worker").await } => {
                        match joined {
                            Ok(value) => RunOutcome::Finished(value),
                            Err(err) if err.is_panic() => RunOutcome::Panicked,
                            Err(_) => RunOutcome::Cancelled,
                        }
                    }
                };
                drop(worker);
                let cancelled = matches!(outcome, RunOutcome::Cancelled);
                guard.finish(cancelled);
                if let Some(sender) = slot.lock().unwrap_or_else(|err| err.into_inner()).take() {
                    let _ = sender.send(outcome);
                }
            })
        });

        let pending = PendingJob {
            id,
            always_run,
            done,
            on_cancel: Some(on_cancel),
            launch: Some(launch),
        };

        {
            let mut state = lock(&self.shared);
            if state.closed {
                pending.abandon();
                return Err(QueueError::Closed);
            }
            let Some(queue) = state.queues.get_mut(name) else {
                pending.abandon();
                return Err(QueueError::NotFound);
            };
            if queue.pending.len() >= queue.pending_capacity {
                pending.abandon();
                return Err(QueueError::Full);
            }
            queue.pending.push_back(pending);
        }
        schedule(&self.shared, name);

        Ok(RunHandle { id, rx, cancel })
    }
}

impl Shared {
    fn cancel_job(&self, name: &str, id: u64) {
        let mut pending = None;
        let mut abort = None;
        {
            let mut state = lock(self);
            let Some(queue) = state.queues.get_mut(name) else {
                return;
            };
            if let Some(index) = queue.pending.iter().position(|job| job.id == id) {
                let job = queue.pending.remove(index).expect("index just found");
                queue.completed = queue.completed.saturating_add(1);
                queue.cancelled = queue.cancelled.saturating_add(1);
                pending = Some(job);
            } else if let Some(handle) = queue.aborts.remove(&id) {
                abort = Some(handle);
            }
        }
        if let Some(job) = pending {
            job.abandon();
        }
        if let Some(handle) = abort {
            handle.abort();
        }
    }

    fn shutdown_now(&self) {
        self.shutdown.cancel();
        let (pending, aborts) = {
            let mut state = lock(self);
            state.closed = true;
            let mut pending = Vec::new();
            let mut aborts = Vec::new();
            for queue in state.queues.values_mut() {
                let jobs: Vec<PendingJob> = queue.pending.drain(..).collect();
                let count = jobs.len() as u64;
                queue.completed = queue.completed.saturating_add(count);
                queue.cancelled = queue.cancelled.saturating_add(count);
                pending.extend(jobs);
                aborts.extend(queue.aborts.drain().map(|(_, abort)| abort));
            }
            (pending, aborts)
        };
        for job in pending {
            job.abandon();
        }
        for abort in aborts {
            abort.abort();
        }
    }
}

fn lock(shared: &Shared) -> std::sync::MutexGuard<'_, State> {
    shared.state.lock().unwrap_or_else(|err| err.into_inner())
}

fn schedule(shared: &Arc<Shared>, name: &str) {
    let batch = {
        let mut state = lock(shared);
        if state.closed {
            return;
        }
        let Some(queue) = state.queues.get_mut(name) else {
            return;
        };
        let mut batch = Vec::new();
        while queue.running + batch.len() < queue.concurrency {
            let index = if queue.paused {
                queue.pending.iter().position(|job| job.always_run)
            } else if queue.pending.is_empty() {
                None
            } else {
                Some(0)
            };
            let Some(index) = index else {
                break;
            };
            batch.push(queue.pending.remove(index).expect("runnable index"));
        }
        queue.running = queue.running.saturating_add(batch.len());
        batch
    };

    for job in batch {
        let id = job.id;
        let done = Arc::clone(&job.done);
        let handle = job.start(Arc::clone(shared), name.to_owned());
        let abort = handle.abort_handle();
        drop(handle);
        let mut state = lock(shared);
        if done.load(Ordering::Acquire) {
            continue;
        }
        if let Some(queue) = state.queues.get_mut(name) {
            queue.aborts.insert(id, abort);
        } else {
            abort.abort();
        }
    }
}

#[cfg(test)]
mod tests;
