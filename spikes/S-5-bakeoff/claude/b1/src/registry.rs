//! Named queues, including the always-present non-blocking `Default` queue.

use std::collections::HashMap;
use std::fmt;
use std::num::NonZeroUsize;
use std::sync::{Mutex, MutexGuard, PoisonError};

use tokio::runtime::Handle;

use crate::queue::{ActionQueue, QueueConfig};

/// Name of the queue every action uses unless it names another one.
pub const DEFAULT_QUEUE: &str = "Default";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RegistryError {
    AlreadyExists(String),
    /// The `Default` queue cannot be removed.
    DefaultQueue,
}

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RegistryError::AlreadyExists(name) => write!(f, "queue {name:?} already exists"),
            RegistryError::DefaultQueue => {
                write!(f, "the {DEFAULT_QUEUE:?} queue cannot be removed")
            }
        }
    }
}

impl std::error::Error for RegistryError {}

/// Queues by exact (case-sensitive) name.
pub struct QueueRegistry {
    runtime: Handle,
    queues: Mutex<HashMap<String, ActionQueue>>,
}

impl fmt::Debug for QueueRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("QueueRegistry")
            .field("queues", &self.names())
            .finish()
    }
}

impl QueueRegistry {
    /// Creates a registry holding a non-blocking `Default` queue.
    ///
    /// # Panics
    /// When called outside a Tokio runtime; use [`QueueRegistry::with_runtime`] there.
    pub fn new(default_concurrency: NonZeroUsize) -> Self {
        Self::with_runtime(default_concurrency, Handle::current())
    }

    pub fn with_runtime(default_concurrency: NonZeroUsize, runtime: Handle) -> Self {
        let default = ActionQueue::with_runtime(
            QueueConfig::non_blocking(DEFAULT_QUEUE, default_concurrency),
            runtime.clone(),
        );
        Self {
            runtime,
            queues: Mutex::new(HashMap::from([(DEFAULT_QUEUE.to_owned(), default)])),
        }
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<String, ActionQueue>> {
        self.queues.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn default_queue(&self) -> ActionQueue {
        self.get(DEFAULT_QUEUE)
            .expect("the Default queue is never removed")
    }

    pub fn create(&self, config: QueueConfig) -> Result<ActionQueue, RegistryError> {
        let mut queues = self.lock();
        if queues.contains_key(&config.name) {
            return Err(RegistryError::AlreadyExists(config.name));
        }
        let name = config.name.clone();
        let queue = ActionQueue::with_runtime(config, self.runtime.clone());
        queues.insert(name, queue.clone());
        Ok(queue)
    }

    pub fn get(&self, name: &str) -> Option<ActionQueue> {
        self.lock().get(name).cloned()
    }

    /// Unregisters a queue. Running runs carry on. Pending runs carry on while some clone of
    /// the queue (or a running run) keeps it alive; otherwise they resolve as
    /// [`RunOutcome::Aborted`](crate::RunOutcome::Aborted). Clear it first to drop them
    /// explicitly as `Cleared`.
    pub fn remove(&self, name: &str) -> Result<Option<ActionQueue>, RegistryError> {
        if name == DEFAULT_QUEUE {
            return Err(RegistryError::DefaultQueue);
        }
        Ok(self.lock().remove(name))
    }

    /// Sorted queue names.
    pub fn names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.lock().keys().cloned().collect();
        names.sort();
        names
    }

    pub fn pause_all(&self) {
        for queue in self.snapshot() {
            queue.pause();
        }
    }

    pub fn resume_all(&self) {
        for queue in self.snapshot() {
            queue.resume();
        }
    }

    /// Copies the queue list so per-queue calls (which may spawn runs) happen without the
    /// registry lock held.
    fn snapshot(&self) -> Vec<ActionQueue> {
        self.lock().values().cloned().collect()
    }
}
