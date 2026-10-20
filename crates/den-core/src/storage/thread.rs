//! Dedicated SQLite owner. Factories alone decide inspection, bootstrap and read-only policy.
use rusqlite::Connection;
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};
use tokio::sync::{mpsc, oneshot};

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
type Job = Box<dyn FnOnce(&mut StorageConns) -> bool + Send>;
enum Command {
    Run(Job),
    Stop,
}
#[derive(Clone)]
pub struct StorageHandle {
    tx: mpsc::Sender<Command>,
    dropped: Arc<AtomicU64>,
}
impl StorageHandle {
    /// Cumulative full-queue rejections; DEN-263 adds the since/report aggregation observer.
    pub fn dropped(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
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
        let (reply, result) = oneshot::channel();
        let command = Command::Run(Box::new(move |conns| {
            let value = catch_unwind(AssertUnwindSafe(|| job(conns)));
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
                    self.dropped.fetch_add(1, Ordering::Relaxed);
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
                while let Some(command) = rx.blocking_recv() {
                    match command {
                        Command::Stop => rx.close(),
                        Command::Run(job) => {
                            if !job(&mut conns) {
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
                dropped: Arc::new(AtomicU64::new(0)),
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
