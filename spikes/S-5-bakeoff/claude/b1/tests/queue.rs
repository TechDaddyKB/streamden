mod common;

use std::num::NonZeroUsize;
use std::sync::Arc;

use common::{job, ms, Tracker};
use den_action_queue_b1::{
    ActionQueue, EnqueueError, EnqueueOptions, QueueConfig, QueueMode, QueueRegistry,
    RegistryError, RunOutcome, DEFAULT_QUEUE,
};
use tokio::time::Instant;

const NORMAL: EnqueueOptions = EnqueueOptions { always_run: false };
const ALWAYS: EnqueueOptions = EnqueueOptions { always_run: true };

fn nz(n: usize) -> NonZeroUsize {
    NonZeroUsize::new(n).unwrap()
}

/// Lets spawned tasks run without advancing the paused clock.
async fn settle() {
    for _ in 0..10 {
        tokio::task::yield_now().await;
    }
}

#[tokio::test(start_paused = true)]
async fn blocking_runs_one_at_a_time_in_fifo_order() {
    let q = ActionQueue::new(QueueConfig::blocking("alerts"));
    let t = Tracker::new();
    let begin = Instant::now();
    let handles: Vec<_> = (0..5)
        .map(|_| q.enqueue(NORMAL, job(&t, ms(10))).unwrap())
        .collect();
    for (i, h) in handles.into_iter().enumerate() {
        assert_eq!(h.outcome().await, RunOutcome::Completed(i as u64));
    }
    assert_eq!(t.peak(), 1);
    assert_eq!(t.started_ids(), vec![0, 1, 2, 3, 4]);
    assert_eq!(begin.elapsed(), ms(50));
    let s = q.stats();
    assert_eq!((s.pending, s.running, s.completed), (0, 0, 5));
}

#[tokio::test(start_paused = true)]
async fn non_blocking_runs_in_parallel_up_to_the_limit() {
    let q = ActionQueue::new(QueueConfig::non_blocking("fx", nz(3)));
    let t = Tracker::new();
    let begin = Instant::now();
    let handles: Vec<_> = (0..10)
        .map(|_| q.enqueue(NORMAL, job(&t, ms(10))).unwrap())
        .collect();
    for h in handles {
        assert!(matches!(h.outcome().await, RunOutcome::Completed(_)));
    }
    assert_eq!(t.peak(), 3);
    assert_eq!(t.started_ids(), (0..10).collect::<Vec<_>>());
    // ceil(10 / 3) waves of 10 ms.
    assert_eq!(begin.elapsed(), ms(40));
}

#[tokio::test(start_paused = true)]
async fn pause_holds_new_runs_and_resume_releases_them() {
    let q = ActionQueue::new(QueueConfig::blocking("alerts"));
    let t = Tracker::new();
    let first = q.enqueue(NORMAL, job(&t, ms(10))).unwrap();
    settle().await;
    q.pause();
    assert!(q.is_paused());
    let second = q.enqueue(NORMAL, job(&t, ms(10))).unwrap();

    // The running run finishes; the paused queue starts nothing else.
    assert_eq!(first.outcome().await, RunOutcome::Completed(0));
    tokio::time::sleep(ms(100)).await;
    assert_eq!(t.started_ids(), vec![0]);
    assert_eq!(q.stats().pending, 1);

    q.resume();
    assert_eq!(second.outcome().await, RunOutcome::Completed(1));
}

#[tokio::test(start_paused = true)]
async fn always_run_bypasses_pause_but_respects_the_limit() {
    let q = ActionQueue::new(QueueConfig::blocking("alerts"));
    let t = Tracker::new();
    q.pause();
    let normal = q.enqueue(NORMAL, job(&t, ms(10))).unwrap();
    let a1 = q.enqueue(ALWAYS, job(&t, ms(10))).unwrap();
    let a2 = q.enqueue(ALWAYS, job(&t, ms(10))).unwrap();

    assert_eq!(a1.outcome().await, RunOutcome::Completed(1));
    assert_eq!(a2.outcome().await, RunOutcome::Completed(2));
    assert_eq!(t.peak(), 1, "always-run still obeys blocking mode");
    assert_eq!(t.started_ids(), vec![1, 2], "the normal run stays held");

    q.resume();
    assert_eq!(normal.outcome().await, RunOutcome::Completed(0));
}

#[tokio::test(start_paused = true)]
async fn clear_drops_pending_runs_but_not_running_ones() {
    let q = ActionQueue::new(QueueConfig::blocking("alerts"));
    let t = Tracker::new();
    let running = q.enqueue(NORMAL, job(&t, ms(10))).unwrap();
    let p1 = q.enqueue(NORMAL, job(&t, ms(10))).unwrap();
    let p2 = q.enqueue(ALWAYS, job(&t, ms(10))).unwrap();
    settle().await;

    assert_eq!(q.clear(), 2);
    assert_eq!(p1.outcome().await, RunOutcome::Cleared);
    assert_eq!(p2.outcome().await, RunOutcome::Cleared);
    assert_eq!(running.outcome().await, RunOutcome::Completed(0));
    assert_eq!(t.started_ids(), vec![0]);
    let s = q.stats();
    assert_eq!((s.completed, s.cancelled, s.pending), (1, 2, 0));
}

#[tokio::test(start_paused = true)]
async fn cancelled_queued_run_never_starts() {
    let q = ActionQueue::new(QueueConfig::blocking("alerts"));
    let t = Tracker::new();
    let a = q.enqueue(NORMAL, job(&t, ms(10))).unwrap();
    let b = q.enqueue(NORMAL, job(&t, ms(10))).unwrap();
    let c = q.enqueue(NORMAL, job(&t, ms(10))).unwrap();
    b.canceller().cancel();
    assert_eq!(b.outcome().await, RunOutcome::Cancelled);
    assert_eq!(a.outcome().await, RunOutcome::Completed(0));
    assert_eq!(c.outcome().await, RunOutcome::Completed(2));
    assert_eq!(t.started_ids(), vec![0, 2]);
    assert_eq!(q.stats().cancelled, 1);
}

#[tokio::test(start_paused = true)]
async fn cancelling_a_running_run_drops_it_and_frees_the_slot() {
    let q = ActionQueue::new(QueueConfig::blocking("alerts"));
    let t = Tracker::new();
    let stuck = q.enqueue(NORMAL, job(&t, ms(1_000_000))).unwrap();
    let next = q.enqueue(NORMAL, job(&t, ms(10))).unwrap();
    settle().await;
    assert_eq!(t.active(), 1);

    stuck.cancel();
    assert_eq!(stuck.outcome().await, RunOutcome::Cancelled);
    assert_eq!(next.outcome().await, RunOutcome::Completed(1));
    assert_eq!(t.active(), 0, "the cancelled future was dropped");
    let s = q.stats();
    assert_eq!((s.running, s.completed, s.cancelled), (0, 1, 1));
}

#[tokio::test(start_paused = true)]
async fn cancel_after_completion_is_a_no_op() {
    let q = ActionQueue::new(QueueConfig::blocking("alerts"));
    let t = Tracker::new();
    let h = q.enqueue(NORMAL, job(&t, ms(1))).unwrap();
    let c = h.canceller();
    assert_eq!(h.outcome().await, RunOutcome::Completed(0));
    c.cancel();
    let s = q.stats();
    assert_eq!((s.completed, s.cancelled), (1, 0));
}

#[tokio::test(start_paused = true)]
async fn enqueue_fails_when_pending_capacity_is_reached() {
    let q = ActionQueue::new(QueueConfig::blocking("alerts").with_capacity(nz(2)));
    let t = Tracker::new();
    q.pause();
    q.enqueue(NORMAL, job(&t, ms(1))).unwrap();
    q.enqueue(NORMAL, job(&t, ms(1))).unwrap();
    let err = q.enqueue(NORMAL, job(&t, ms(1))).unwrap_err();
    assert_eq!(
        err,
        EnqueueError::Full {
            queue: "alerts".into(),
            capacity: 2
        }
    );
    // Running runs do not count against capacity.
    q.resume();
    settle().await;
    assert!(q.enqueue(NORMAL, job(&t, ms(1))).is_ok());
}

#[tokio::test(start_paused = true)]
async fn panicking_job_reports_aborted_and_does_not_wedge_the_queue() {
    let q = ActionQueue::new(QueueConfig::blocking("alerts"));
    let t = Tracker::new();
    let bad = q
        .enqueue(NORMAL, |_ctx| async { panic!("job failed on purpose") })
        .unwrap();
    let good = q.enqueue(NORMAL, job(&t, ms(1))).unwrap();
    assert_eq!(bad.outcome().await, RunOutcome::<()>::Aborted);
    // Counters are already updated when the outcome arrives.
    assert_eq!(q.stats().aborted, 1);
    assert_eq!(good.outcome().await, RunOutcome::Completed(1));
    let s = q.stats();
    assert_eq!((s.aborted, s.completed, s.running), (1, 1, 0));
}

#[test]
fn runtime_shutdown_with_a_large_backlog_aborts_runs_without_recursing() {
    // Regression: spawn on a closed runtime drops the future at once, whose guard used to
    // re-enter pump, one stack frame set per pending run (overflowed at ~1000 runs).
    const BACKLOG: usize = 20_000;
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap();
    let t = Tracker::new();
    let (q, handles) = rt.block_on(async {
        let q = ActionQueue::new(QueueConfig::blocking("alerts").with_capacity(nz(BACKLOG)));
        let handles: Vec<_> = (0..=BACKLOG)
            .map(|_| q.enqueue(NORMAL, job(&t, ms(1_000_000))).unwrap())
            .collect();
        settle().await;
        (q, handles)
    });
    assert_eq!(t.active(), 1);
    drop(rt);

    let s = q.stats();
    assert_eq!((s.pending, s.running), (0, 0));
    assert_eq!(s.aborted, BACKLOG as u64 + 1);
    let check = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    check.block_on(async {
        for h in handles {
            assert_eq!(h.outcome().await, RunOutcome::Aborted);
        }
    });
}

#[tokio::test(start_paused = true)]
async fn raising_the_limit_starts_waiting_runs_immediately() {
    let q = ActionQueue::new(QueueConfig::blocking("alerts"));
    let t = Tracker::new();
    let handles: Vec<_> = (0..4)
        .map(|_| q.enqueue(NORMAL, job(&t, ms(10))).unwrap())
        .collect();
    settle().await;
    assert_eq!(t.active(), 1);
    q.set_mode(QueueMode::NonBlocking {
        max_concurrency: nz(4),
    });
    settle().await;
    assert_eq!(t.active(), 4);
    for h in handles {
        assert!(matches!(h.outcome().await, RunOutcome::Completed(_)));
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_enqueue_on_a_multi_thread_runtime_respects_the_limit() {
    let q = ActionQueue::new(QueueConfig::non_blocking("stress", nz(4)));
    let t = Tracker::new();
    let mut producers = Vec::new();
    for _ in 0..8 {
        let q = q.clone();
        let t = Arc::clone(&t);
        producers.push(tokio::spawn(async move {
            let mut hs = Vec::new();
            for _ in 0..50 {
                hs.push(q.enqueue(NORMAL, job(&t, ms(0))).unwrap());
                tokio::task::yield_now().await;
            }
            for h in hs {
                assert!(matches!(h.outcome().await, RunOutcome::Completed(_)));
            }
        }));
    }
    for p in producers {
        p.await.unwrap();
    }
    assert!(t.peak() <= 4, "peak {}", t.peak());
    let s = q.stats();
    assert_eq!((s.completed, s.pending, s.running), (400, 0, 0));
}

#[tokio::test(start_paused = true)]
async fn registry_has_a_non_blocking_default_and_unique_names() {
    let reg = QueueRegistry::new(nz(8));
    assert_eq!(reg.names(), vec![DEFAULT_QUEUE.to_string()]);
    assert_eq!(
        reg.default_queue().mode(),
        QueueMode::NonBlocking {
            max_concurrency: nz(8)
        }
    );
    reg.create(QueueConfig::blocking("alerts")).unwrap();
    assert_eq!(
        reg.create(QueueConfig::blocking("alerts")).unwrap_err(),
        RegistryError::AlreadyExists("alerts".into())
    );
    assert!(reg.get("Alerts").is_none(), "names are case-sensitive");
    assert_eq!(
        reg.remove(DEFAULT_QUEUE).unwrap_err(),
        RegistryError::DefaultQueue
    );
    assert!(reg.remove("alerts").unwrap().is_some());
    assert_eq!(reg.names(), vec![DEFAULT_QUEUE.to_string()]);
}

#[tokio::test(start_paused = true)]
async fn pause_all_and_resume_all_cover_every_queue() {
    let reg = QueueRegistry::new(nz(2));
    let alerts = reg.create(QueueConfig::blocking("alerts")).unwrap();
    reg.pause_all();
    assert!(alerts.is_paused() && reg.default_queue().is_paused());

    let t = Tracker::new();
    let h = alerts.enqueue(NORMAL, job(&t, ms(1))).unwrap();
    tokio::time::sleep(ms(50)).await;
    assert!(t.started_ids().is_empty());

    reg.resume_all();
    assert!(!alerts.is_paused() && !reg.default_queue().is_paused());
    assert_eq!(h.outcome().await, RunOutcome::Completed(0));
}
