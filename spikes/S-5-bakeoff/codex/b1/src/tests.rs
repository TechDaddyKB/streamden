use crate::{Error, Mode, Outcome, Queue, Ticket};
use proptest::prelude::*;
use std::{num::NonZeroUsize, sync::Arc, time::Duration};
use tokio::sync::{mpsc, Semaphore};

fn size(n: usize) -> NonZeroUsize {
    NonZeroUsize::new(n).unwrap()
}
async fn bounded<T>(future: impl std::future::Future<Output = T>) -> T {
    tokio::time::timeout(Duration::from_secs(3), future)
        .await
        .expect("scheduler stalled")
}
async fn outcome(ticket: Ticket, expected: Outcome) {
    assert_eq!(bounded(ticket.wait()).await, expected);
}

#[tokio::test]
async fn pause_full_cancel_clear_resume() {
    let queue = Queue::new("alerts", Mode::Blocking, size(2));
    assert_eq!(queue.name(), "alerts");
    queue.set_paused(true).await.unwrap();
    let first = queue
        .submit(false, async { panic!("cancelled run started") })
        .await
        .unwrap();
    let second = queue
        .submit(false, async { panic!("cleared run started") })
        .await
        .unwrap();
    assert!(matches!(
        queue.submit(false, async {}).await,
        Err(Error::Full)
    ));
    assert!(queue.cancel(first.id).await.unwrap());
    assert!(!queue.cancel(first.id).await.unwrap());
    outcome(first, Outcome::Cancelled).await;
    assert_eq!(queue.clear().await.unwrap(), 1);
    outcome(second, Outcome::Cancelled).await;
    let resumed = queue.submit(false, async {}).await.unwrap();
    queue.set_paused(false).await.unwrap();
    outcome(resumed, Outcome::Completed).await;
}

#[tokio::test]
async fn always_run_bypasses_pause_without_bypassing_serial_limit() {
    let queue = Queue::new("alerts", Mode::Blocking, size(4));
    let gate = Arc::new(Semaphore::new(0));
    let (started, mut seen) = mpsc::unbounded_channel();
    let permit = gate.clone();
    let first = queue
        .submit(false, async move {
            started.send(()).unwrap();
            permit.acquire().await.unwrap().forget();
        })
        .await
        .unwrap();
    bounded(seen.recv()).await.unwrap();
    queue.set_paused(true).await.unwrap();
    let ordinary = queue
        .submit(false, async { panic!("paused run started") })
        .await
        .unwrap();
    let (started, mut seen) = mpsc::unbounded_channel();
    let forced = queue
        .submit(true, async move {
            started.send(()).unwrap();
        })
        .await
        .unwrap();
    assert!(seen.try_recv().is_err());
    assert!(!queue.cancel(first.id).await.unwrap());
    gate.add_permits(1);
    outcome(first, Outcome::Completed).await;
    bounded(seen.recv()).await.unwrap();
    outcome(forced, Outcome::Completed).await;
    assert_eq!(queue.clear().await.unwrap(), 1);
    outcome(ordinary, Outcome::Cancelled).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn parallel_limit_and_clear_preserve_active_runs() {
    let queue = Queue::new("parallel", Mode::NonBlocking(size(2)), size(4));
    let gate = Arc::new(Semaphore::new(0));
    let (started, mut seen) = mpsc::unbounded_channel();
    let mut tickets = Vec::new();
    for _ in 0..3 {
        let (gate, started) = (gate.clone(), started.clone());
        tickets.push(
            queue
                .submit(false, async move {
                    started.send(()).unwrap();
                    gate.acquire().await.unwrap().forget();
                })
                .await
                .unwrap(),
        );
    }
    for _ in 0..2 {
        bounded(seen.recv()).await.unwrap();
    }
    assert!(seen.try_recv().is_err());
    assert_eq!(queue.clear().await.unwrap(), 1);
    outcome(tickets.pop().unwrap(), Outcome::Cancelled).await;
    let (gate2, started2) = (gate.clone(), started.clone());
    let third = queue
        .submit(false, async move {
            started2.send(()).unwrap();
            gate2.acquire().await.unwrap().forget();
        })
        .await
        .unwrap();
    assert!(seen.try_recv().is_err());
    gate.add_permits(1);
    bounded(seen.recv()).await.unwrap();
    gate.add_permits(2);
    for ticket in tickets {
        outcome(ticket, Outcome::Completed).await;
    }
    outcome(third, Outcome::Completed).await;
}

#[tokio::test]
async fn panic_releases_slot_and_names_are_independent() {
    let one = Queue::new("one", Mode::Blocking, size(2));
    let two = Queue::new("two", Mode::Blocking, size(2));
    one.set_paused(true).await.unwrap();
    let paused = one.submit(false, async {}).await.unwrap();
    let panicked = two
        .submit(false, async { panic!("expected action panic") })
        .await
        .unwrap();
    let next = two.submit(false, async {}).await.unwrap();
    outcome(panicked, Outcome::Failed).await;
    outcome(next, Outcome::Completed).await;
    drop(one);
    outcome(paused, Outcome::Cancelled).await;
}

#[tokio::test]
async fn final_handle_drop_aborts_active_job() {
    let queue = Queue::new("shutdown", Mode::Blocking, size(1));
    let (started, mut seen) = mpsc::unbounded_channel();
    let active = queue
        .submit(false, async move {
            started.send(()).unwrap();
            std::future::pending::<()>().await;
        })
        .await
        .unwrap();
    bounded(seen.recv()).await.unwrap();
    let clone = queue.clone();
    drop(queue);
    clone.set_paused(true).await.unwrap();
    drop(clone);
    outcome(active, Outcome::Failed).await;
}

proptest! {
    #[test]
    fn blocking_fifo_survives_arbitrary_queued_cancellations(cancel in prop::collection::vec(any::<bool>(), 1..64)) {
        let runtime = tokio::runtime::Builder::new_current_thread().enable_time().build().unwrap();
        runtime.block_on(bounded(async {
            let queue = Queue::new("property", Mode::Blocking, size(64));
            queue.set_paused(true).await.unwrap();
            let (sent, mut received) = mpsc::unbounded_channel();
            let mut tickets = Vec::new();
            for index in 0..cancel.len() {
                let sent = sent.clone();
                tickets.push(queue.submit(false, async move { sent.send(index).unwrap(); }).await.unwrap());
            }
            for (ticket, cancel) in tickets.iter().zip(&cancel) {
                if *cancel { assert!(queue.cancel(ticket.id).await.unwrap()); }
            }
            queue.set_paused(false).await.unwrap();
            for (ticket, cancel) in tickets.into_iter().zip(&cancel) {
                outcome(ticket, if *cancel { Outcome::Cancelled } else { Outcome::Completed }).await;
            }
            let expected: Vec<_> = cancel.iter().enumerate().filter_map(|(i, cancelled)| (!cancelled).then_some(i)).collect();
            let actual: Vec<_> = std::iter::from_fn(|| received.try_recv().ok()).collect();
            assert_eq!(actual, expected);
        }));
    }
}
