//! Dedicated SQLite owner. Factories alone decide inspection, bootstrap and read-only policy.
use crate::clock::{Clock, SystemClock};
use den_protocol::Timestamp;
use rusqlite::Connection;
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio::{
    sync::{Notify, mpsc, oneshot},
    time::Instant,
};

pub const CAPACITY: usize = 4096;
pub struct StorageConns {
    pub config: Connection,
    pub history: Connection,
}
impl StorageConns {
    /// Attempt both closes even when config fails. Never checkpoint a read-only database here.
    pub fn close(self) -> Result<(), StorageError> {
        let config = self
            .config
            .close()
            .map_err(|(_, error)| StorageError::Sqlite(error));
        let history = self
            .history
            .close()
            .map_err(|(_, error)| StorageError::Sqlite(error));
        config.and(history)
    }
}
#[derive(Debug)]
pub enum StorageError {
    Spawn(std::io::Error),
    Sqlite(rusqlite::Error),
    Panicked,
    Disconnected,
}
#[derive(Clone, Copy)]
pub enum JobClass {
    MustLand,
    Droppable,
}
pub enum Admission<T> {
    Accepted(Reply<T>),
    Dropped,
}
pub struct Reply<T>(oneshot::Receiver<Result<T, StorageError>>);
impl<T> Reply<T> {
    pub async fn wait(self) -> Result<T, StorageError> {
        self.0.await.map_err(|_| StorageError::Disconnected)?
    }
}
/// Hooks own sender-free buffer state, never a StorageHandle or command sender. Only the worker
/// retains the registry; hook implementations must not introduce an indirect sender cycle either.
/// Return raw Sqlite errors for retryable failures. Spawn, Panicked and Disconnected are reserved
/// for transport; a hook must not manufacture them. A hook panic terminates the worker.
pub trait FlushHook: Send + 'static {
    fn flush(&self, conns: &mut StorageConns) -> Result<(), StorageError>;
}
type Hooks = Vec<Box<dyn FlushHook>>;

/// Internal report for DEN-116's eventual publisher; no wire event is emitted here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StorageBacklog {
    pub dropped: u64,
    pub since: Timestamp,
}
#[derive(Default)]
struct BacklogState {
    pending: u64,
    since: Option<Timestamp>,
    last_report: Option<Instant>,
}
impl BacklogState {
    fn deadline(&self) -> Option<Instant> {
        self.since
            .and(self.last_report)
            .map(|last| last + Duration::from_secs(60))
    }
    fn take_due(&mut self, now: Instant) -> Option<StorageBacklog> {
        if self.pending == 0 || self.deadline().is_some_and(|deadline| now < deadline) {
            return None;
        }
        let report = StorageBacklog {
            dropped: std::mem::take(&mut self.pending),
            since: self.since.take().unwrap(),
        };
        self.last_report = Some(now);
        Some(report)
    }
}
struct Backlog {
    dropped: AtomicU64,
    state: Mutex<BacklogState>,
    clock: Arc<dyn Clock + Send + Sync>,
    changed: Notify,
}
impl Backlog {
    fn state(&self) -> MutexGuard<'_, BacklogState> {
        // A clock panic occurs before any mutation; the accounting remains consistent.
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
    fn record_drop(&self) -> Option<StorageBacklog> {
        let mut state = self.state();
        let now = self.clock.sample();
        self.dropped.fetch_add(1, Ordering::Relaxed);
        let was_empty = state.pending == 0;
        state.pending = state.pending.saturating_add(1);
        state.since.get_or_insert(now.wall);
        let report = state.take_due(now.monotonic);
        if was_empty && report.is_none() {
            self.changed.notify_one();
        }
        report
    }
}
/// Sender-free internal port for DEN-116's single scheduler; owns neither observer nor worker.
/// Keeping clones alive cannot keep command admission open or delay last-sender finalization.
#[derive(Clone)]
pub struct BacklogReporter(Arc<Backlog>);
impl BacklogReporter {
    /// Earliest gated report time, only while drops are pending. Recheck after every wake/take.
    pub fn deadline(&self) -> Option<Instant> {
        self.0.state().deadline()
    }
    /// Atomically retire a nonempty due window, sharing the drop-triggered report gate.
    /// Returns data to the caller; it does not call the observer or emit a wire event.
    pub fn take_due(&self) -> Option<StorageBacklog> {
        let mut state = self.0.state();
        let now = self.0.clock.sample().monotonic;
        state.take_due(now)
    }
    /// Single-consumer wake when suppressed drops first become pending. A retained permit covers
    /// the deadline-check/await race. Wakes may be stale after a competing take/drop; recheck state.
    /// Await this alongside the scheduler's existing deadline, never in a polling loop.
    pub async fn changed(&self) {
        self.0.changed.notified().await;
    }
}
type Job = Box<dyn FnOnce(&mut StorageConns, &mut Hooks) -> bool + Send>;
enum Command {
    Run(Job),
    Stop,
}
#[derive(Clone)]
pub struct StorageHandle {
    tx: mpsc::Sender<Command>,
    backlog: Arc<Backlog>,
    observer: Arc<dyn Fn(StorageBacklog) + Send + Sync>,
}
impl StorageHandle {
    pub fn backlog_reporter(&self) -> BacklogReporter {
        BacklogReporter(self.backlog.clone())
    }
    /// Cumulative full-queue rejections, independent of report retirement.
    pub fn dropped(&self) -> u64 {
        self.backlog.dropped.load(Ordering::Relaxed)
    }
    /// Cancellation before admission removes this attempt. Once accepted, a job runs even if
    /// its reply is dropped. T may itself be a typed leaf-adapter Result; SQL errors stay intact.
    /// A job panic is terminal: its reply gets Panicked, queued replies get Disconnected, and
    /// finalization is attempted. A finalizer failure never replaces the primary job panic.
    pub async fn submit<T: Send + 'static>(
        &self,
        class: JobClass,
        job: impl FnOnce(&mut StorageConns) -> T + Send + 'static,
    ) -> Result<Admission<T>, StorageError> {
        self.dispatch(class, move |conns, _| job(conns)).await
    }
    /// Registration order is worker queue order. Cancellation after admission retains the hook.
    pub async fn register_flush_hook(&self, hook: impl FlushHook) -> Result<(), StorageError> {
        self.must_land(move |_, hooks| hooks.push(Box::new(hook)))
            .await
    }
    /// Explicit reusable flush for DEN-73. Stops at the first raw hook error; hooks remain registered
    /// for retry. No automatic flush, checkpoint, marker, or close policy is added to transport.
    pub async fn flush_hooks(&self) -> Result<(), StorageError> {
        self.must_land(|conns, hooks| {
            for hook in hooks {
                hook.flush(conns)?;
            }
            Ok(())
        })
        .await?
    }
    async fn must_land<T: Send + 'static>(
        &self,
        job: impl FnOnce(&mut StorageConns, &mut Hooks) -> T + Send + 'static,
    ) -> Result<T, StorageError> {
        match self.dispatch(JobClass::MustLand, job).await? {
            Admission::Accepted(reply) => reply.wait().await,
            Admission::Dropped => unreachable!("MustLand never drops"),
        }
    }
    async fn dispatch<T: Send + 'static>(
        &self,
        class: JobClass,
        job: impl FnOnce(&mut StorageConns, &mut Hooks) -> T + Send + 'static,
    ) -> Result<Admission<T>, StorageError> {
        let (reply, result) = oneshot::channel();
        let command = Command::Run(Box::new(move |conns, hooks| {
            let value = catch_unwind(AssertUnwindSafe(|| job(conns, hooks)));
            let healthy = value.is_ok();
            let _ = reply.send(value.map_err(|_| StorageError::Panicked));
            healthy
        }));
        match class {
            JobClass::MustLand => self
                .tx
                .send(command)
                .await
                .map_err(|_| StorageError::Disconnected)?,
            JobClass::Droppable => match self.tx.try_send(command) {
                Ok(()) => (),
                Err(mpsc::error::TrySendError::Full(_)) => {
                    if let Some(report) = self.backlog.record_drop() {
                        // Never invoke external observer code under the accounting lock.
                        (self.observer)(report);
                    }
                    return Ok(Admission::Dropped);
                }
                Err(mpsc::error::TrySendError::Closed(_)) => {
                    return Err(StorageError::Disconnected);
                }
            },
        }
        Ok(Admission::Accepted(Reply(result)))
    }
}
pub struct StorageWorker {
    pub handle: StorageHandle,
    done: oneshot::Receiver<Result<(), StorageError>>,
}
impl StorageWorker {
    /// Both factories execute on the dedicated thread. Close is an injectable finalizer for
    /// later startup/shutdown adapters; transport adds no pragmas, schema writes or markers.
    pub async fn start(
        open: impl FnOnce() -> Result<StorageConns, StorageError> + Send + 'static,
        close: impl FnOnce(StorageConns) -> Result<(), StorageError> + Send + 'static,
    ) -> Result<Self, StorageError> {
        Self::start_with_backlog(open, close, Arc::new(SystemClock), |_| {}).await
    }
    /// First drop reports immediately; later reports share a monotonic one-minute gate with
    /// BacklogReporter::take_due. Pending tails wake the future DEN-116 scheduler via changed().
    /// No task, timer, publisher or forced close report is created here.
    /// Clock/observer are trusted, fast, nonpanicking and sender-free. A clock panic propagates
    /// to that caller before counting its rejection; subsequent accounting recovers unchanged.
    /// Observer runs outside locks on the rejecting caller (callbacks may overlap); an observer
    /// panic propagates after report retirement, with no delivery retry.
    pub async fn start_with_backlog(
        open: impl FnOnce() -> Result<StorageConns, StorageError> + Send + 'static,
        close: impl FnOnce(StorageConns) -> Result<(), StorageError> + Send + 'static,
        clock: Arc<dyn Clock + Send + Sync>,
        observer: impl Fn(StorageBacklog) + Send + Sync + 'static,
    ) -> Result<Self, StorageError> {
        let (tx, mut rx) = mpsc::channel(CAPACITY);
        let (ready, initialized) = oneshot::channel();
        let (finished, done) = oneshot::channel();
        std::thread::Builder::new()
            .name("streamden-storage".into())
            .spawn(move || {
                let opened =
                    catch_unwind(AssertUnwindSafe(open)).unwrap_or(Err(StorageError::Panicked));
                let mut conns = match opened {
                    Ok(conns) => conns,
                    Err(error) => {
                        let _ = ready.send(Err(error));
                        return;
                    }
                };
                let _ = ready.send(Ok(()));
                let mut status = Ok(());
                let mut hooks = Hooks::new();
                while let Some(command) = rx.blocking_recv() {
                    match command {
                        Command::Stop => rx.close(),
                        Command::Run(job) => {
                            if !job(&mut conns, &mut hooks) {
                                status = Err(StorageError::Panicked);
                                break;
                            }
                        }
                    }
                }
                // Wake queued replies and pending admissions before potentially slow finalization.
                drop(rx);
                let closed = catch_unwind(AssertUnwindSafe(|| close(conns)))
                    .unwrap_or(Err(StorageError::Panicked));
                let _ = finished.send(status.and(closed));
            })
            .map_err(StorageError::Spawn)?;
        initialized
            .await
            .map_err(|_| StorageError::Disconnected)??;
        Ok(Self {
            handle: StorageHandle {
                tx,
                backlog: Arc::new(Backlog {
                    dropped: AtomicU64::new(0),
                    state: Mutex::new(BacklogState::default()),
                    clock,
                    changed: Notify::new(),
                }),
                observer: Arc::new(observer),
            },
            done,
        })
    }
    /// Stop admission, drain accepted jobs, then finalize on the owner thread. Cloned handles
    /// cannot keep shutdown alive. Dropping the last sender also drains/finalizes, without waiting.
    /// Cancellation before Stop admission cancels that attempt; after admission shutdown continues.
    pub async fn close(self) -> Result<(), StorageError> {
        let _ = self.handle.tx.send(Command::Stop).await;
        self.done.await.map_err(|_| StorageError::Disconnected)?
    }
}
