//! Dedicated SQLite owner. Factories alone decide inspection, bootstrap and read-only policy.
use crate::clock::{Clock, SystemClock};
use den_protocol::Timestamp;
use rusqlite::Connection;
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio::{
    sync::{mpsc, oneshot},
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
struct Backlog {
    dropped: AtomicU64,
    state: Mutex<BacklogState>,
    clock: Arc<dyn Clock + Send + Sync>,
    observer: Box<dyn Fn(StorageBacklog) + Send + Sync>,
}
impl Backlog {
    fn record_drop(&self) {
        let report = {
            let mut state = self.state.lock().unwrap();
            let now = self.clock.sample();
            self.dropped.fetch_add(1, Ordering::Relaxed);
            state.pending = state.pending.saturating_add(1);
            let since = *state.since.get_or_insert(now.wall);
            if state.last_report.is_some_and(|last| {
                now.monotonic
                    .checked_duration_since(last)
                    .unwrap_or_default()
                    < Duration::from_secs(60)
            }) {
                return;
            }
            let dropped = std::mem::take(&mut state.pending);
            state.since = None;
            state.last_report = Some(now.monotonic);
            StorageBacklog { dropped, since }
        };
        // Never invoke external observer code under the accounting lock.
        (self.observer)(report);
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
}
impl StorageHandle {
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
                    self.backlog.record_drop();
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
    /// Drop-triggered reports: first drop immediately, then at most once per minute. Unreported
    /// drops accumulate until another drop passes the monotonic gate, even across idle periods.
    /// Clock/observer must be fast, nonpanicking and sender-free; observer runs on the rejecting
    /// caller, outside locks (concurrent callbacks may overlap). No task or idle timer is created.
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
                    observer: Box::new(observer),
                }),
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
