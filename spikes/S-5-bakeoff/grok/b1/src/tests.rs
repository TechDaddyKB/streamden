use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::watch;

use super::{ActionQueues, QueueConfig, QueueError, QueueMode, RunOutcome};

fn queue_config(name: &str, mode: QueueMode, pending_capacity: usize) -> QueueConfig {
    QueueConfig {
        name: name.to_owned(),
        mode,
        pending_capacity,
    }
}

fn blocking(name: &str, pending_capacity: usize) -> QueueConfig {
    queue_config(name, QueueMode::Blocking, pending_capacity)
}

async fn until(label: &str, mut pred: impl FnMut() -> bool) {
    let start = tokio::time::Instant::now();
    while !pred() {
        if start.elapsed() > Duration::from_secs(2) {
            panic!("condition was not met within 2s: {label}");
        }
        tokio::task::yield_now().await;
    }
}

async fn wait_go(mut rx: watch::Receiver<bool>) {
    loop {
        if *rx.borrow() {
            return;
        }
        if rx.changed().await.is_err() {
            return;
        }
    }
}

async fn finished_in_order(queues: &ActionQueues, name: &str, count: usize) -> Vec<usize> {
    let mut handles = Vec::with_capacity(count);
    for id in 0..count {
        handles.push(
            queues
                .enqueue(name, false, move |_| async move { id })
                .await
                .unwrap(),
        );
    }
    let mut got = Vec::with_capacity(count);
    for handle in handles {
        match tokio::time::timeout(Duration::from_secs(2), handle.wait()).await {
            Ok(RunOutcome::Finished(id)) => got.push(id),
            other => panic!("unexpected outcome {other:?}"),
        }
    }
    got
}

#[tokio::test(flavor = "current_thread")]
async fn blocking_queue_is_fifo_and_serial() {
    let queues = ActionQueues::new();
    queues.create(blocking("alerts", 8)).unwrap();
    let inflight = Arc::new(AtomicUsize::new(0));
    let max = Arc::new(AtomicUsize::new(0));
    let mut handles = Vec::new();
    for id in 0..6 {
        let inflight = Arc::clone(&inflight);
        let max = Arc::clone(&max);
        handles.push(
            queues
                .enqueue("alerts", false, move |_| async move {
                    let now = inflight.fetch_add(1, Ordering::SeqCst) + 1;
                    max.fetch_max(now, Ordering::SeqCst);
                    tokio::task::yield_now().await;
                    inflight.fetch_sub(1, Ordering::SeqCst);
                    id
                })
                .await
                .unwrap(),
        );
    }
    let mut order = Vec::new();
    for handle in handles {
        match handle.wait().await {
            RunOutcome::Finished(id) => order.push(id),
            other => panic!("unexpected outcome {other:?}"),
        }
    }
    assert_eq!(order, (0..6).collect::<Vec<_>>());
    assert_eq!(max.load(Ordering::SeqCst), 1);
    let snap = queues.snapshot("alerts").unwrap();
    assert_eq!(snap.completed, 6);
    assert_eq!(snap.cancelled, 0);
    assert_eq!(snap.running, 0);
}

#[tokio::test(flavor = "current_thread")]
async fn nonblocking_limit_and_clear_keep_the_active_run() {
    let queues = ActionQueues::new();
    queues
        .create(queue_config(
            "parallel",
            QueueMode::NonBlocking { concurrency: 2 },
            8,
        ))
        .unwrap();
    let (release_tx, release_rx) = watch::channel(false);
    let started = Arc::new(AtomicUsize::new(0));
    let mut handles = Vec::new();
    for id in 0..5 {
        let started = Arc::clone(&started);
        let release_rx = release_rx.clone();
        handles.push(
            queues
                .enqueue("parallel", false, move |_| async move {
                    started.fetch_add(1, Ordering::SeqCst);
                    wait_go(release_rx).await;
                    id
                })
                .await
                .unwrap(),
        );
    }
    until("parallel started", || started.load(Ordering::SeqCst) == 2).await;
    for _ in 0..30 {
        tokio::task::yield_now().await;
    }
    assert_eq!(started.load(Ordering::SeqCst), 2);
    assert_eq!(queues.snapshot("parallel").unwrap().pending, 3);

    let cleared = queues.clear("parallel").unwrap();
    assert_eq!(cleared, 3);
    assert_eq!(queues.snapshot("parallel").unwrap().pending, 0);
    assert_eq!(queues.snapshot("parallel").unwrap().running, 2);

    release_tx.send(true).unwrap();
    assert!(matches!(
        handles.remove(0).wait().await,
        RunOutcome::Finished(0)
    ));
    assert!(matches!(
        handles.remove(0).wait().await,
        RunOutcome::Finished(1)
    ));
    for handle in handles {
        assert!(matches!(handle.wait().await, RunOutcome::Cancelled));
    }
}

#[tokio::test(flavor = "current_thread")]
async fn always_run_bypasses_pause_and_still_waits_for_a_slot() {
    let queues = ActionQueues::new();
    queues.create(blocking("alerts", 8)).unwrap();
    let (release_tx, release_rx) = watch::channel(false);
    let order = Arc::new(std::sync::Mutex::new(Vec::new()));

    let active = {
        let order = Arc::clone(&order);
        let release_rx = release_rx.clone();
        queues
            .enqueue("alerts", false, move |_| async move {
                order.lock().unwrap().push("active");
                wait_go(release_rx).await;
            })
            .await
            .unwrap()
    };
    until("alerts running", || {
        queues.snapshot("alerts").unwrap().running == 1
    })
    .await;
    queues.pause("alerts").unwrap();

    let held = {
        let order = Arc::clone(&order);
        queues
            .enqueue("alerts", false, move |_| async move {
                order.lock().unwrap().push("held");
            })
            .await
            .unwrap()
    };
    let bypass = {
        let order = Arc::clone(&order);
        queues
            .enqueue("alerts", true, move |_| async move {
                order.lock().unwrap().push("bypass");
            })
            .await
            .unwrap()
    };
    until("active started", || {
        order.lock().unwrap().as_slice() == ["active"]
    })
    .await;
    assert_eq!(order.lock().unwrap().as_slice(), ["active"]);
    assert_eq!(queues.snapshot("alerts").unwrap().pending, 2);

    release_tx.send(true).unwrap();
    assert!(matches!(active.wait().await, RunOutcome::Finished(())));
    until("bypass started", || {
        order.lock().unwrap().as_slice() == ["active", "bypass"]
    })
    .await;
    assert!(matches!(bypass.wait().await, RunOutcome::Finished(())));
    assert_eq!(queues.snapshot("alerts").unwrap().pending, 1);
    assert!(queues.snapshot("alerts").unwrap().paused);

    queues.resume("alerts").unwrap();
    assert!(matches!(held.wait().await, RunOutcome::Finished(())));
    assert_eq!(
        order.lock().unwrap().as_slice(),
        ["active", "bypass", "held"]
    );
}

#[tokio::test(flavor = "current_thread")]
async fn pause_is_per_queue_and_full_rejects_without_starting() {
    let queues = ActionQueues::new();
    queues.create(blocking("held", 1)).unwrap();
    queues
        .create(queue_config(
            "open",
            QueueMode::NonBlocking { concurrency: 2 },
            4,
        ))
        .unwrap();
    queues.pause("held").unwrap();

    let waiting = queues
        .enqueue("held", false, |_| async { 1 })
        .await
        .unwrap();
    let rejected = queues.enqueue("held", false, |_| async { 2 }).await;
    assert_eq!(rejected.err(), Some(QueueError::Full));
    assert_eq!(queues.snapshot("held").unwrap().pending, 1);

    let open = queues
        .enqueue("open", false, |_| async { 7 })
        .await
        .unwrap();
    assert!(matches!(open.wait().await, RunOutcome::Finished(7)));
    assert_eq!(queues.snapshot("held").unwrap().running, 0);

    queues.resume("held").unwrap();
    assert!(matches!(waiting.wait().await, RunOutcome::Finished(1)));
}

#[tokio::test(flavor = "current_thread")]
async fn cancel_queued_run_and_abort_running_run() {
    let queues = ActionQueues::new();
    queues.create(blocking("work", 4)).unwrap();
    let (release_tx, release_rx) = watch::channel(false);
    let running = queues
        .enqueue("work", false, move |_| async move {
            wait_go(release_rx).await;
            1
        })
        .await
        .unwrap();
    until("work running", || {
        queues.snapshot("work").unwrap().running == 1
    })
    .await;
    queues.pause("work").unwrap();
    let queued = queues
        .enqueue("work", false, |_| async { 2 })
        .await
        .unwrap();
    queued.cancel();
    assert!(matches!(queued.wait().await, RunOutcome::Cancelled));
    assert_eq!(queues.snapshot("work").unwrap().pending, 0);

    running.cancel();
    assert!(matches!(running.wait().await, RunOutcome::Cancelled));
    let snap = queues.snapshot("work").unwrap();
    assert_eq!(snap.running, 0, "snap={snap:?}");
    assert_eq!(snap.cancelled, 2, "snap={snap:?}");

    queues.resume("work").unwrap();
    let next = queues
        .enqueue("work", false, |_| async { 3 })
        .await
        .unwrap();
    assert!(matches!(next.wait().await, RunOutcome::Finished(3)));
    let _ = release_tx;
}

#[tokio::test(flavor = "current_thread")]
async fn panic_releases_the_slot() {
    let queues = ActionQueues::new();
    queues.create(blocking("work", 2)).unwrap();
    let panicked = queues
        .enqueue("work", false, |_| async { panic!("job failed") })
        .await
        .unwrap();
    assert!(matches!(panicked.wait().await, RunOutcome::Panicked));
    until("after panic idle", || {
        queues.snapshot("work").unwrap().running == 0
    })
    .await;
    let next = queues
        .enqueue("work", false, |_| async { 4 })
        .await
        .unwrap();
    assert!(matches!(next.wait().await, RunOutcome::Finished(4)));
    let snap = queues.snapshot("work").unwrap();
    assert_eq!(snap.cancelled, 0);
    assert_eq!(snap.completed, 2);
}

#[tokio::test(flavor = "current_thread")]
async fn set_mode_widens_admission_without_preempting() {
    let queues = ActionQueues::new();
    queues.create(blocking("work", 8)).unwrap();
    let (release_tx, release_rx) = watch::channel(false);
    let started = Arc::new(AtomicUsize::new(0));
    let mut handles = Vec::new();
    for id in 0..3 {
        let started = Arc::clone(&started);
        let release_rx = release_rx.clone();
        handles.push(
            queues
                .enqueue("work", false, move |_| async move {
                    started.fetch_add(1, Ordering::SeqCst);
                    wait_go(release_rx).await;
                    id
                })
                .await
                .unwrap(),
        );
    }
    until("one started", || started.load(Ordering::SeqCst) == 1).await;
    queues
        .set_mode("work", QueueMode::NonBlocking { concurrency: 3 })
        .unwrap();
    until("three started", || started.load(Ordering::SeqCst) == 3).await;
    assert_eq!(queues.snapshot("work").unwrap().running, 3);
    release_tx.send(true).unwrap();
    let mut finished = Vec::new();
    for handle in handles {
        match handle.wait().await {
            RunOutcome::Finished(id) => finished.push(id),
            other => panic!("unexpected outcome {other:?}"),
        }
    }
    finished.sort_unstable();
    assert_eq!(finished, vec![0, 1, 2]);
}

#[tokio::test(flavor = "current_thread")]
async fn last_owner_drop_cancels_active_work() {
    let queues = ActionQueues::new();
    queues.create(blocking("work", 2)).unwrap();
    let (release_tx, release_rx) = watch::channel(false);
    let handle = queues
        .enqueue("work", false, move |_| async move {
            wait_go(release_rx).await;
        })
        .await
        .unwrap();
    until("drop running", || {
        queues.snapshot("work").unwrap().running == 1
    })
    .await;
    let clone = queues.clone();
    drop(clone);
    assert_eq!(queues.snapshot("work").unwrap().running, 1);
    drop(queues);
    let outcome = tokio::time::timeout(Duration::from_secs(2), handle.wait()).await;
    assert!(matches!(outcome, Ok(RunOutcome::Cancelled)));
    let _ = release_tx;
}

#[tokio::test]
async fn rejects_unknown_duplicate_and_invalid_queues() {
    let queues = ActionQueues::new();
    let err = queues.enqueue("missing", false, |_| async {}).await;
    assert_eq!(err.err(), Some(QueueError::NotFound));
    assert_eq!(queues.pause("missing"), Err(QueueError::NotFound));
    assert_eq!(
        queues.create(queue_config(
            "bad",
            QueueMode::NonBlocking { concurrency: 0 },
            1
        )),
        Err(QueueError::InvalidConfig(
            "non-blocking concurrency must be at least 1"
        ))
    );
    assert_eq!(
        queues.create(blocking("", 1)),
        Err(QueueError::InvalidConfig("queue name must not be empty"))
    );
    queues.create(blocking("only", 1)).unwrap();
    assert_eq!(
        queues.create(blocking("only", 1)),
        Err(QueueError::AlreadyExists)
    );
}

#[test]
fn blocking_fifo_survives_arbitrary_queued_cancels() {
    use proptest::prelude::*;

    let mut config = ProptestConfig::with_cases(24);
    config.max_shrink_iters = 32;
    proptest!(config, |(mask in proptest::collection::vec(any::<bool>(), 1..8))| {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let queues = ActionQueues::new();
            queues.create(blocking("b", 8)).unwrap();
            queues.pause("b").unwrap();
            let mut handles = Vec::new();
            for id in 0..mask.len() {
                handles.push(
                    queues
                        .enqueue("b", false, move |_| async move { id })
                        .await
                        .unwrap(),
                );
            }
            let mut expected = Vec::new();
            let mut waiting = Vec::new();
            for (index, (handle, cancel)) in handles.into_iter().zip(mask).enumerate() {
                if cancel {
                    handle.cancel();
                    assert!(matches!(handle.wait().await, RunOutcome::Cancelled));
                } else {
                    expected.push(index);
                    waiting.push(handle);
                }
            }
            assert_eq!(queues.snapshot("b").unwrap().pending, expected.len());
            queues.resume("b").unwrap();
            let mut got = Vec::new();
            for handle in waiting {
                match handle.wait().await {
                    RunOutcome::Finished(id) => got.push(id),
                    other => panic!("unexpected outcome {other:?}"),
                }
            }
            assert_eq!(got, expected);
            assert_eq!(queues.snapshot("b").unwrap().running, 0);
        });
    });
}

#[test]
fn nonblocking_never_exceeds_the_concurrency_limit() {
    use proptest::prelude::*;

    let mut config = ProptestConfig::with_cases(16);
    config.max_shrink_iters = 16;
    proptest!(config, |(limit in 1usize..5, extra in 0usize..6)| {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let count = limit + extra;
            let queues = ActionQueues::new();
            queues
                .create(queue_config(
                    "p",
                    QueueMode::NonBlocking { concurrency: limit },
                    count,
                ))
                .unwrap();
            let inflight = Arc::new(AtomicUsize::new(0));
            let max = Arc::new(AtomicUsize::new(0));
            let (release_tx, release_rx) = watch::channel(false);
            let mut handles = Vec::new();
            for id in 0..count {
                let inflight = Arc::clone(&inflight);
                let max = Arc::clone(&max);
                let release_rx = release_rx.clone();
                handles.push(
                    queues
                        .enqueue("p", false, move |_| async move {
                            let now = inflight.fetch_add(1, Ordering::SeqCst) + 1;
                            max.fetch_max(now, Ordering::SeqCst);
                            wait_go(release_rx).await;
                            inflight.fetch_sub(1, Ordering::SeqCst);
                            id
                        })
                        .await
                        .unwrap(),
                );
            }
            until("property running", || {
                queues.snapshot("p").unwrap().running == limit
            })
            .await;
            for _ in 0..20 {
                tokio::task::yield_now().await;
            }
            assert!(max.load(Ordering::SeqCst) <= limit);
            assert_eq!(queues.snapshot("p").unwrap().running, limit);
            release_tx.send(true).unwrap();
            let mut finished = Vec::new();
            for handle in handles {
                match handle.wait().await {
                    RunOutcome::Finished(id) => finished.push(id),
                    other => panic!("unexpected outcome {other:?}"),
                }
            }
            finished.sort_unstable();
            assert_eq!(finished, (0..count).collect::<Vec<_>>());
            assert!(max.load(Ordering::SeqCst) <= limit);
            assert_eq!(queues.snapshot("p").unwrap().running, 0);
        });
    });
}

#[test]
fn create_without_a_runtime_fails() {
    let queues = ActionQueues::new();
    assert_eq!(queues.create(blocking("q", 1)), Err(QueueError::NoRuntime));
}

#[tokio::test(flavor = "current_thread")]
async fn resumed_blocking_order_matches_submission() {
    let queues = ActionQueues::new();
    queues.create(blocking("b", 8)).unwrap();
    queues.pause("b").unwrap();
    let order = finished_in_order(&queues, "b", 0).await;
    assert!(order.is_empty());
    let mut handles = Vec::new();
    for id in 0..4 {
        handles.push(
            queues
                .enqueue("b", false, move |_| async move { id })
                .await
                .unwrap(),
        );
    }
    assert_eq!(queues.snapshot("b").unwrap().running, 0);
    queues.resume("b").unwrap();
    let mut got = Vec::new();
    for handle in handles {
        match handle.wait().await {
            RunOutcome::Finished(id) => got.push(id),
            other => panic!("unexpected outcome {other:?}"),
        }
    }
    assert_eq!(got, vec![0, 1, 2, 3]);
}
