//! Standalone DEN-8 spike. Each named queue owns one event-driven Tokio scheduler.
#[cfg(test)]
mod tests;

use std::{collections::VecDeque, future::Future, num::NonZeroUsize, pin::Pin, sync::Arc};
use tokio::{
    sync::{mpsc, oneshot},
    task::JoinSet,
};

type Job = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;

#[derive(Clone, Copy, Debug)]
pub enum Mode {
    Blocking,
    NonBlocking(NonZeroUsize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Full,
    Closed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Completed,
    Cancelled,
    /// The job panicked or its scheduler shut down.
    Failed,
}

/// Dropping a ticket does not cancel its job. Cancellation is explicit and queued-only.
pub struct Ticket {
    pub id: u64,
    result: oneshot::Receiver<Outcome>,
}

impl Ticket {
    pub async fn wait(self) -> Outcome {
        self.result.await.unwrap_or(Outcome::Failed)
    }
}

struct Pending {
    id: u64,
    always_run: bool,
    job: Job,
    result: oneshot::Sender<Outcome>,
}

enum Command {
    Submit(Job, bool, oneshot::Sender<Result<Ticket, Error>>),
    Pause(bool, oneshot::Sender<()>),
    Clear(oneshot::Sender<usize>),
    Cancel(u64, oneshot::Sender<bool>),
}

#[derive(Clone)]
pub struct Queue {
    name: Arc<str>,
    commands: mpsc::Sender<Command>,
}

impl Queue {
    /// Requires a Tokio runtime. `capacity` bounds waiting jobs, excluding active jobs.
    pub fn new(name: impl Into<Arc<str>>, mode: Mode, capacity: NonZeroUsize) -> Self {
        let (commands, receiver) = mpsc::channel(32);
        let limit = match mode {
            Mode::Blocking => 1,
            Mode::NonBlocking(limit) => limit.get(),
        };
        tokio::spawn(schedule(receiver, capacity.get(), limit));
        Self {
            name: name.into(),
            commands,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns Full when waiting capacity is exhausted; never waits for a free run slot.
    /// `always_run` bypasses pause, but still obeys capacity and concurrency limits.
    pub async fn submit(
        &self,
        always_run: bool,
        job: impl Future<Output = ()> + Send + 'static,
    ) -> Result<Ticket, Error> {
        let (reply, received) = oneshot::channel();
        self.commands
            .send(Command::Submit(Box::pin(job), always_run, reply))
            .await
            .map_err(|_| Error::Closed)?;
        received.await.map_err(|_| Error::Closed)?
    }

    /// Acknowledgement means the scheduler applied the pause. Active jobs continue.
    pub async fn set_paused(&self, paused: bool) -> Result<(), Error> {
        let (reply, received) = oneshot::channel();
        self.commands
            .send(Command::Pause(paused, reply))
            .await
            .map_err(|_| Error::Closed)?;
        received.await.map_err(|_| Error::Closed)
    }

    pub async fn clear(&self) -> Result<usize, Error> {
        let (reply, received) = oneshot::channel();
        self.commands
            .send(Command::Clear(reply))
            .await
            .map_err(|_| Error::Closed)?;
        received.await.map_err(|_| Error::Closed)
    }

    /// False means this queue has no pending run with this ID (it may be active).
    pub async fn cancel(&self, id: u64) -> Result<bool, Error> {
        let (reply, received) = oneshot::channel();
        self.commands
            .send(Command::Cancel(id, reply))
            .await
            .map_err(|_| Error::Closed)?;
        received.await.map_err(|_| Error::Closed)
    }
}

async fn schedule(mut commands: mpsc::Receiver<Command>, capacity: usize, limit: usize) {
    let mut pending: VecDeque<Pending> = VecDeque::new();
    let mut active = JoinSet::new();
    let mut paused = false;
    let mut next_id = 0u64;
    loop {
        while active.len() < limit {
            let Some(index) = pending.iter().position(|run| !paused || run.always_run) else {
                break;
            };
            let run = pending.remove(index).expect("selected pending run");
            active.spawn(async move {
                run.job.await;
                let _ = run.result.send(Outcome::Completed);
            });
        }
        tokio::select! {
            command = commands.recv() => match command {
                Some(Command::Submit(job, always_run, reply)) => {
                    if pending.len() == capacity {
                        let _ = reply.send(Err(Error::Full));
                        continue;
                    }
                    let Some(id) = next_id.checked_add(1) else {
                        let _ = reply.send(Err(Error::Closed));
                        continue;
                    };
                    next_id = id;
                    let (result, received) = oneshot::channel();
                    // An abandoned submit request must not leave an unobservable pending run.
                    if reply.send(Ok(Ticket { id, result: received })).is_ok() {
                        pending.push_back(Pending { id, always_run, job, result });
                    }
                }
                Some(Command::Pause(value, reply)) => {
                    paused = value;
                    let _ = reply.send(());
                }
                Some(Command::Clear(reply)) => {
                    let count = pending.len();
                    for run in pending.drain(..) { let _ = run.result.send(Outcome::Cancelled); }
                    let _ = reply.send(count);
                }
                Some(Command::Cancel(id, reply)) => {
                    let found = pending.iter().position(|run| run.id == id);
                    if let Some(index) = found {
                        let run = pending.remove(index).expect("selected pending run");
                        let _ = run.result.send(Outcome::Cancelled);
                    }
                    let _ = reply.send(found.is_some());
                }
                None => break,
            },
            // Reaping a panic still releases its slot. No polling or detached action tasks.
            _ = active.join_next(), if !active.is_empty() => {}
        }
    }
    for run in pending {
        let _ = run.result.send(Outcome::Cancelled);
    }
    // JoinSet drop aborts active jobs when the final Queue handle is dropped.
}
