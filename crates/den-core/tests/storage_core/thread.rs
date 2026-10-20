use den_core::storage::thread::{
    Admission, CAPACITY, JobClass, Reply, StorageConns, StorageError, StorageHandle, StorageWorker,
};
use rusqlite::Connection;
use std::{future::Future, pin::pin, sync::mpsc, task::Poll};
use tokio::sync::oneshot;

fn open() -> Result<StorageConns, StorageError> {
    Ok(StorageConns {
        config: Connection::open_in_memory().map_err(StorageError::Sqlite)?,
        history: Connection::open_in_memory().map_err(StorageError::Sqlite)?,
    })
}
fn accepted<T>(result: Result<Admission<T>, StorageError>) -> Reply<T> {
    match result.unwrap() {
        Admission::Accepted(reply) => reply,
        Admission::Dropped => panic!("unexpected rejection"),
    }
}
async fn pending(mut future: std::pin::Pin<&mut impl Future>) {
    std::future::poll_fn(|cx| {
        assert!(future.as_mut().poll(cx).is_pending());
        Poll::Ready(())
    })
    .await;
}
#[tokio::test]
async fn owner_identity_typed_results_and_abandoned_replies() {
    let caller = std::thread::current().id();
    let (opened, owner) = oneshot::channel();
    let (closed, closer) = oneshot::channel();
    let worker = StorageWorker::start(
        move || {
            opened.send(std::thread::current().id()).unwrap();
            open()
        },
        move |conns| {
            closed.send(std::thread::current().id()).unwrap();
            conns.close()
        },
    )
    .await
    .unwrap();
    let owner = owner.await.unwrap();
    assert_ne!(caller, owner);
    let job = accepted(
        worker
            .handle
            .submit(JobClass::MustLand, move |conns| {
                assert_eq!(std::thread::current().id(), owner);
                conns
                    .config
                    .execute_batch("CREATE TABLE saved (n); INSERT INTO saved VALUES (7)")
            })
            .await,
    );
    drop(job); // Accepted work is independent of the caller waiting for its reply.
    let value = accepted(
        worker
            .handle
            .submit(JobClass::MustLand, |conns| {
                conns
                    .config
                    .query_row("SELECT n FROM saved", [], |row| row.get::<_, i64>(0))
            })
            .await,
    )
    .wait()
    .await
    .unwrap()
    .unwrap();
    assert_eq!(value, 7);
    let failure: rusqlite::Result<()> = accepted(
        worker
            .handle
            .submit(JobClass::MustLand, |conns| {
                conns.history.execute_batch("not SQL")
            })
            .await,
    )
    .wait()
    .await
    .unwrap();
    assert!(failure.is_err());
    let after_error = accepted(worker.handle.submit(JobClass::MustLand, |_| 11).await)
        .wait()
        .await
        .unwrap();
    assert_eq!(after_error, 11);
    worker.close().await.unwrap();
    assert_eq!(closer.await.unwrap(), owner);
}
#[tokio::test]
async fn factory_failures_and_canceled_open_finalize() {
    for panic in [false, true] {
        let result = StorageWorker::start(
            move || {
                assert!(!panic, "injected factory panic");
                Err(StorageError::Sqlite(rusqlite::Error::InvalidQuery))
            },
            StorageConns::close,
        )
        .await;
        assert!(if panic {
            matches!(result, Err(StorageError::Panicked))
        } else {
            matches!(result, Err(StorageError::Sqlite(_)))
        });
    }
    let (started, opening) = oneshot::channel();
    let (release, held) = mpsc::channel();
    let (closed, finalized) = oneshot::channel();
    let mut starting = Box::pin(StorageWorker::start(
        move || {
            started.send(()).unwrap();
            held.recv().unwrap();
            open()
        },
        move |conns| {
            let result = conns.close();
            closed.send(()).unwrap();
            result
        },
    ));
    pending(starting.as_mut()).await;
    opening.await.unwrap();
    drop(starting);
    release.send(()).unwrap();
    finalized.await.unwrap();
}
#[tokio::test]
async fn close_failure_and_panic_are_reported() {
    for panic in [false, true] {
        let worker = StorageWorker::start(open, move |conns| {
            conns.close()?;
            assert!(!panic, "injected close panic");
            Err(StorageError::Sqlite(rusqlite::Error::InvalidQuery))
        })
        .await
        .unwrap();
        let handle = worker.handle.clone();
        let result = worker.close().await;
        assert!(if panic {
            matches!(result, Err(StorageError::Panicked))
        } else {
            matches!(result, Err(StorageError::Sqlite(_)))
        });
        assert!(matches!(
            handle.submit(JobClass::MustLand, |_| ()).await,
            Err(StorageError::Disconnected)
        ));
    }
}

// The sender is the release guard: dropping it on assertion unwind also unblocks the owner.
async fn held_job(handle: &StorageHandle, panic: bool) -> (mpsc::Sender<()>, Reply<()>) {
    let (started, running) = oneshot::channel();
    let (release, held) = mpsc::channel();
    let active = accepted(
        handle
            .submit(JobClass::MustLand, move |_| {
                let _ = started.send(());
                let _ = held.recv(); // Only the dedicated storage thread blocks.
                assert!(!panic, "injected active job panic");
            })
            .await,
    );
    running.await.unwrap();
    (release, active)
}
async fn fill(handle: &StorageHandle) -> Vec<Reply<usize>> {
    assert_eq!(CAPACITY, 4096);
    let mut replies = Vec::new();
    for i in 0..4096 {
        replies.push(accepted(
            handle.submit(JobClass::Droppable, move |_| i).await,
        ));
    }
    replies
}
async fn drain(active: Reply<()>, replies: Vec<Reply<usize>>) {
    active.wait().await.unwrap();
    assert_eq!(replies.len(), 4096);
    for (i, reply) in replies.into_iter().enumerate() {
        assert_eq!(reply.wait().await.unwrap(), i);
    }
}
#[tokio::test]
async fn droppable_jobs_counted_when_full() {
    let worker = StorageWorker::start(open, StorageConns::close)
        .await
        .unwrap();
    let (release, active) = held_job(&worker.handle, false).await;
    let replies = fill(&worker.handle).await;
    let handle = worker.handle.clone();
    assert_eq!(handle.dropped(), 0);
    assert!(matches!(
        handle
            .submit(JobClass::Droppable, |_| panic!("rejected job ran"))
            .await
            .unwrap(),
        Admission::Dropped
    ));
    assert_eq!(worker.handle.dropped(), 1);
    release.send(()).unwrap();
    drain(active, replies).await;
    worker.close().await.unwrap();
    assert!(matches!(
        handle.submit(JobClass::Droppable, |_| ()).await,
        Err(StorageError::Disconnected)
    ));
    assert_eq!(handle.dropped(), 1); // Closed is not counted as Full.
}
#[tokio::test]
async fn must_land_jobs_wait_when_full() {
    let worker = StorageWorker::start(open, StorageConns::close)
        .await
        .unwrap();
    let (release, active) = held_job(&worker.handle, false).await;
    let replies = fill(&worker.handle).await;
    let mut canceled = Box::pin(
        worker
            .handle
            .submit(JobClass::MustLand, |_| panic!("canceled admission ran")),
    );
    pending(canceled.as_mut()).await;
    drop(canceled);
    let handle = worker.handle.clone();
    let mut waiting = pin!(handle.submit(JobClass::MustLand, |_| 42));
    pending(waiting.as_mut()).await;
    release.send(()).unwrap();
    let reply = accepted(waiting.await);
    drain(active, replies).await;
    assert_eq!(reply.wait().await.unwrap(), 42);
    assert_eq!(handle.dropped(), 0);
    worker.close().await.unwrap();
}
#[tokio::test]
async fn panic_disconnects_queued_replies_and_waiting_admissions() {
    let (closed, finalized) = oneshot::channel();
    let worker = StorageWorker::start(open, move |conns| {
        conns.close()?;
        let _ = closed.send(());
        Err(StorageError::Sqlite(rusqlite::Error::InvalidQuery))
    })
    .await
    .unwrap();
    let (release, active) = held_job(&worker.handle, true).await;
    let replies = fill(&worker.handle).await;
    let handle = worker.handle.clone();
    let mut waiting = pin!(handle.submit(JobClass::MustLand, |_| 42));
    pending(waiting.as_mut()).await;
    release.send(()).unwrap();
    assert!(matches!(active.wait().await, Err(StorageError::Panicked)));
    // Closing the receiver can race a waiting sender's acquisition of a freed slot.
    match waiting.await {
        Ok(Admission::Accepted(reply)) => assert!(matches!(
            reply.wait().await,
            Err(StorageError::Disconnected)
        )),
        Err(StorageError::Disconnected) => (),
        _ => panic!("unexpected admission"),
    }
    for reply in replies {
        assert!(matches!(
            reply.wait().await,
            Err(StorageError::Disconnected)
        ));
    }
    assert!(matches!(worker.close().await, Err(StorageError::Panicked)));
    finalized.await.unwrap(); // Finalizer ran, but its error did not replace the job panic.
    assert!(matches!(
        handle.submit(JobClass::Droppable, |_| ()).await,
        Err(StorageError::Disconnected)
    ));
    assert_eq!(handle.dropped(), 0);
}
#[tokio::test]
async fn canceled_close_drains_accepted_jobs_despite_live_handles() {
    let (closed, finalized) = oneshot::channel();
    let worker = StorageWorker::start(open, move |conns| {
        let result = conns.close();
        let _ = closed.send(result);
        Ok(())
    })
    .await
    .unwrap();
    let handle = worker.handle.clone();
    let (release, active) = held_job(&handle, false).await;
    let queued = accepted(handle.submit(JobClass::MustLand, |_| 9).await);
    let mut closing = Box::pin(worker.close());
    // Queue has space: this poll admits Stop, then waits for the held worker's finalization.
    pending(closing.as_mut()).await;
    drop(closing);
    release.send(()).unwrap();
    active.wait().await.unwrap();
    assert_eq!(queued.wait().await.unwrap(), 9);
    finalized.await.unwrap().unwrap();
    assert!(matches!(
        handle.submit(JobClass::MustLand, |_| ()).await,
        Err(StorageError::Disconnected)
    ));
}
#[tokio::test]
async fn dropping_last_sender_drains_accepted_work_and_finalizes() {
    let (closed, finalized) = oneshot::channel();
    let worker = StorageWorker::start(open, move |conns| {
        let result = conns.close();
        let _ = closed.send(result);
        Ok(())
    })
    .await
    .unwrap();
    let (release, active) = held_job(&worker.handle, false).await;
    let queued = accepted(worker.handle.submit(JobClass::MustLand, |_| 17).await);
    drop(worker);
    release.send(()).unwrap();
    active.wait().await.unwrap();
    assert_eq!(queued.wait().await.unwrap(), 17);
    finalized.await.unwrap().unwrap();
}
#[tokio::test]
async fn factory_controls_existing_read_only_files_without_bootstrap_or_wal() {
    let dir = tempfile::tempdir().unwrap();
    let paths = [dir.path().join("config.db"), dir.path().join("history.db")];
    let fixture =
        "CREATE TABLE sentinel (n); INSERT INTO sentinel VALUES (8); PRAGMA user_version=9001";
    for path in &paths {
        Connection::open(path)
            .unwrap()
            .execute_batch(fixture)
            .unwrap();
    }
    let before: Vec<_> = paths
        .iter()
        .map(|path| std::fs::read(path).unwrap())
        .collect();
    let opening = paths.clone();
    let worker = StorageWorker::start(
        move || {
            let read_only = |path| {
                Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
                    .map_err(StorageError::Sqlite)
            };
            Ok(StorageConns {
                config: read_only(&opening[0])?,
                history: read_only(&opening[1])?,
            })
        },
        StorageConns::close,
    )
    .await
    .unwrap();
    accepted(
        worker
            .handle
            .submit(JobClass::MustLand, |conns| {
                for conn in [&conns.config, &conns.history] {
                    assert!(conn.is_readonly("main").unwrap());
                    assert_eq!(
                        conn.query_row("SELECT n FROM sentinel", [], |r| r.get::<_, i64>(0))
                            .unwrap(),
                        8
                    );
                    let error = conn
                        .execute_batch("INSERT INTO sentinel VALUES (9)")
                        .unwrap_err();
                    assert_eq!(
                        error.sqlite_error_code(),
                        Some(rusqlite::ErrorCode::ReadOnly)
                    );
                    assert_eq!(
                        conn.query_row("SELECT count(*) FROM sentinel", [], |r| r.get::<_, i64>(0))
                            .unwrap(),
                        1
                    );
                    assert_eq!(
                        conn.pragma_query_value(None, "journal_mode", |r| r.get::<_, String>(0))
                            .unwrap(),
                        "delete"
                    );
                    assert_eq!(
                        conn.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
                            .unwrap(),
                        9001
                    );
                }
            })
            .await,
    )
    .wait()
    .await
    .unwrap();
    worker.close().await.unwrap();
    for (path, original) in paths.iter().zip(before) {
        assert_eq!(&original[..16], b"SQLite format 3\0");
        assert_eq!(std::fs::read(path).unwrap(), original); // Includes every header byte.
        assert!(!path.with_extension("db-wal").exists());
        assert!(!path.with_extension("db-shm").exists());
    }
}

struct Hook<F>(F);
impl<F> den_core::storage::thread::FlushHook for Hook<F>
where
    F: Fn(&mut StorageConns) -> Result<(), StorageError> + Send + 'static,
{
    fn flush(&self, conns: &mut StorageConns) -> Result<(), StorageError> {
        (self.0)(conns)
    }
}
#[tokio::test]
async fn flush_hooks_run_in_registration_order() {
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    };
    let worker = StorageWorker::start(open, StorageConns::close)
        .await
        .unwrap();
    let owner = accepted(
        worker
            .handle
            .submit(JobClass::MustLand, |conns| {
                conns
                    .config
                    .execute_batch("CREATE TABLE flushed (n)")
                    .unwrap();
                std::thread::current().id()
            })
            .await,
    )
    .wait()
    .await
    .unwrap();
    let calls = Arc::new(Mutex::new(Vec::new()));
    let fail = Arc::new(AtomicBool::new(true));
    for id in 1..=3 {
        let (calls, fail) = (calls.clone(), fail.clone());
        worker
            .handle
            .register_flush_hook(Hook(move |conns: &mut StorageConns| {
                assert_eq!(std::thread::current().id(), owner);
                calls.lock().unwrap().push(id);
                if id == 2 && fail.load(Ordering::Relaxed) {
                    return Err(StorageError::Sqlite(rusqlite::Error::InvalidParameterName(
                        "hook 2".into(),
                    )));
                }
                conns
                    .config
                    .execute("INSERT INTO flushed VALUES (?)", [id])
                    .map_err(StorageError::Sqlite)?;
                Ok(())
            }))
            .await
            .unwrap();
    }
    assert!(calls.lock().unwrap().is_empty()); // Registration never flushes.
    let error = worker.handle.flush_hooks().await.unwrap_err();
    assert!(
        matches!(error, StorageError::Sqlite(rusqlite::Error::InvalidParameterName(name)) if name == "hook 2")
    );
    assert_eq!(*calls.lock().unwrap(), [1, 2]);
    fail.store(false, Ordering::Relaxed);
    worker.handle.flush_hooks().await.unwrap();
    assert_eq!(*calls.lock().unwrap(), [1, 2, 1, 2, 3]);
    let rows = accepted(
        worker
            .handle
            .submit(JobClass::MustLand, |conns| {
                conns
                    .config
                    .query_row("SELECT count(*) FROM flushed", [], |r| r.get::<_, i64>(0))
                    .unwrap()
            })
            .await,
    )
    .wait()
    .await
    .unwrap();
    assert_eq!(rows, 4);
    worker.close().await.unwrap();
    assert_eq!(*calls.lock().unwrap(), [1, 2, 1, 2, 3]); // Close adds no policy.
}
#[tokio::test]
async fn hook_panic_is_terminal_and_registered_hooks_do_not_retain_sender() {
    for panic in [false, true] {
        let (closed, finalized) = oneshot::channel();
        let worker = StorageWorker::start(open, move |conns| {
            let result = conns.close();
            let _ = closed.send(());
            result
        })
        .await
        .unwrap();
        worker
            .handle
            .register_flush_hook(Hook(move |_: &mut StorageConns| {
                assert!(!panic, "injected hook panic");
                Ok(())
            }))
            .await
            .unwrap();
        if panic {
            assert!(matches!(
                worker.handle.flush_hooks().await,
                Err(StorageError::Panicked)
            ));
            assert!(matches!(worker.close().await, Err(StorageError::Panicked)));
        } else {
            drop(worker); // Registered hooks must not create a last-sender retention cycle.
        }
        finalized.await.unwrap();
    }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn backlog_fake_time_gate_aggregates_without_idle_reports() {
    use den_core::clock::{Clock, ClockSample};
    use den_protocol::Timestamp;
    use std::{
        sync::{Arc, Mutex},
        time::Duration,
    };
    use tokio::time::Instant;
    struct FakeClock(Mutex<ClockSample>);
    impl Clock for FakeClock {
        fn monotonic(&self) -> Instant {
            self.0.lock().unwrap().monotonic
        }
        fn wall(&self) -> Timestamp {
            self.0.lock().unwrap().wall
        }
        fn sample(&self) -> ClockSample {
            *self.0.lock().unwrap()
        }
    }
    let origin = Instant::now();
    let clock = Arc::new(FakeClock(Mutex::new(ClockSample {
        monotonic: origin,
        wall: Timestamp::from_unix_millis(1000).unwrap(),
    })));
    let (reports, observed) = mpsc::channel();
    let worker = StorageWorker::start_with_backlog(
        open,
        StorageConns::close,
        clock.clone(),
        move |report| {
            reports.send(report).unwrap();
        },
    )
    .await
    .unwrap();
    let (release, active) = held_job(&worker.handle, false).await;
    let replies = fill(&worker.handle).await;
    for (seconds, wall, count, since) in [
        (0, 1000, Some(1), 1000),
        (1, 2000, None, 0),
        (59, 999_000, None, 0), // Wall jumps cannot bypass the monotonic gate.
        (60, -1000, Some(19), 2000),
        (61, 3000, None, 0),
        (180, 4000, Some(2), 3000),
    ] {
        *clock.0.lock().unwrap() = ClockSample {
            monotonic: origin + Duration::from_secs(seconds),
            wall: Timestamp::from_unix_millis(wall).unwrap(),
        };
        assert!(matches!(
            observed.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        )); // No idle report.
        assert!(matches!(
            worker
                .handle
                .submit(JobClass::Droppable, |_| ())
                .await
                .unwrap(),
            Admission::Dropped
        ));
        if seconds == 1 {
            let mut rejections = Vec::new();
            for _ in 0..16 {
                let handle = worker.handle.clone();
                rejections.push(tokio::spawn(async move {
                    assert!(matches!(
                        handle.submit(JobClass::Droppable, |_| ()).await.unwrap(),
                        Admission::Dropped
                    ));
                }));
            }
            for rejection in rejections {
                rejection.await.unwrap();
            }
        }
        if let Some(count) = count {
            let report = observed.try_recv().unwrap();
            assert_eq!(report.dropped, count);
            assert_eq!(report.since.unix_millis(), since);
        }
        assert!(matches!(
            observed.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
    }
    assert_eq!(worker.handle.dropped(), 22);
    release.send(()).unwrap();
    drain(active, replies).await;
    let handle = worker.handle.clone();
    worker.close().await.unwrap();
    assert!(matches!(
        handle.submit(JobClass::Droppable, |_| ()).await,
        Err(StorageError::Disconnected)
    ));
    assert_eq!(handle.dropped(), 22);
    assert!(matches!(
        observed.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
}

struct ReportClock {
    sample: std::sync::Mutex<den_core::clock::ClockSample>,
    panic_once: std::sync::atomic::AtomicBool,
}
impl ReportClock {
    fn new() -> Self {
        Self {
            sample: std::sync::Mutex::new(den_core::clock::ClockSample {
                monotonic: tokio::time::Instant::now(),
                wall: den_protocol::Timestamp::from_unix_millis(1000).unwrap(),
            }),
            panic_once: std::sync::atomic::AtomicBool::new(false),
        }
    }
    fn set(&self, origin: tokio::time::Instant, seconds: u64, wall: i64) {
        *self.sample.lock().unwrap() = den_core::clock::ClockSample {
            monotonic: origin + std::time::Duration::from_secs(seconds),
            wall: den_protocol::Timestamp::from_unix_millis(wall).unwrap(),
        };
    }
}
impl den_core::clock::Clock for ReportClock {
    fn monotonic(&self) -> tokio::time::Instant {
        self.sample.lock().unwrap().monotonic
    }
    fn wall(&self) -> den_protocol::Timestamp {
        self.sample.lock().unwrap().wall
    }
    fn sample(&self) -> den_core::clock::ClockSample {
        assert!(
            !self
                .panic_once
                .swap(false, std::sync::atomic::Ordering::Relaxed),
            "injected one-shot clock panic"
        );
        *self.sample.lock().unwrap()
    }
}
async fn reject(handle: &StorageHandle) {
    assert!(matches!(
        handle.submit(JobClass::Droppable, |_| ()).await.unwrap(),
        Admission::Dropped
    ));
}
#[tokio::test]
async fn backlog_tail_wakes_and_retires_without_new_drop_or_sender() {
    use den_core::clock::Clock;
    use std::{sync::Arc, time::Duration};
    let clock = Arc::new(ReportClock::new());
    let origin = clock.monotonic();
    let (reports, observed) = mpsc::channel();
    let (closed, finalized) = oneshot::channel();
    let worker = StorageWorker::start_with_backlog(
        open,
        move |conns| {
            let result = conns.close();
            let _ = closed.send(result);
            Ok(())
        },
        clock.clone(),
        move |report| reports.send(report).unwrap(),
    )
    .await
    .unwrap();
    let reporter = worker.handle.backlog_reporter();
    let monitor = reporter.clone();
    assert_eq!(reporter.deadline(), None);
    assert_eq!(reporter.take_due(), None);
    let (release, active) = held_job(&worker.handle, false).await;
    let replies = fill(&worker.handle).await;
    let mut wake = pin!(reporter.changed());
    pending(wake.as_mut()).await;
    reject(&worker.handle).await;
    assert_eq!(observed.try_recv().unwrap().dropped, 1);
    assert_eq!(reporter.deadline(), None);
    pending(wake.as_mut()).await; // An immediate report creates no pending deadline.
    clock.set(origin, 1, 2000);
    reject(&worker.handle).await;
    wake.await; // First suppressed drop wakes an already waiting scheduler.
    assert_eq!(monitor.deadline(), Some(origin + Duration::from_secs(60)));
    let mut next_wake = pin!(reporter.changed());
    pending(next_wake.as_mut()).await;
    reject(&worker.handle).await;
    pending(next_wake.as_mut()).await; // Same pending window needs no second wake.
    clock.set(origin, 59, 999_000);
    assert_eq!(monitor.take_due(), None);
    clock.set(origin, 60, -1000);
    let report = monitor.take_due().unwrap();
    assert_eq!((report.dropped, report.since.unix_millis()), (2, 2000));
    assert_eq!(reporter.take_due(), None);
    assert_eq!(reporter.deadline(), None);
    assert!(matches!(
        observed.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    reject(&worker.handle).await; // The take reserved the same gate as direct reports.
    next_wake.await;
    assert_eq!(monitor.deadline(), Some(origin + Duration::from_secs(120)));
    assert_eq!(monitor.take_due(), None);
    clock.set(origin, 120, 3000);
    reject(&worker.handle).await; // Direct report wins; a subsequent take cannot duplicate it.
    let report = observed.try_recv().unwrap();
    assert_eq!((report.dropped, report.since.unix_millis()), (2, -1000));
    assert_eq!(monitor.take_due(), None);
    clock.set(origin, 121, 4000);
    reject(&worker.handle).await;
    reporter.changed().await; // Retained permit also covers a late subscriber.
    clock.set(origin, 180, 5000);
    let report = reporter.take_due().unwrap();
    assert_eq!((report.dropped, report.since.unix_millis()), (1, 4000));
    reject(&worker.handle).await;
    assert_eq!(monitor.take_due(), None);
    assert_eq!(worker.handle.dropped(), 7);
    release.send(()).unwrap();
    drain(active, replies).await;
    drop(worker);
    finalized.await.unwrap().unwrap(); // Both reporter clones remain alive.
    clock.set(origin, 240, 6000);
    let report = monitor.take_due().unwrap();
    assert_eq!((report.dropped, report.since.unix_millis()), (1, 5000));
    assert_eq!(reporter.take_due(), None);
    assert_eq!(monitor.deadline(), None);
    assert!(matches!(
        observed.try_recv(),
        Err(mpsc::TryRecvError::Disconnected)
    ));
    // Idle has no report or new notification after the retained pending-transition permit.
    reporter.changed().await;
    let mut idle = pin!(reporter.changed());
    pending(idle.as_mut()).await;
}
#[tokio::test]
async fn one_clock_panic_does_not_poison_future_admission_or_pending_take() {
    use den_core::clock::Clock;
    use std::{
        panic::AssertUnwindSafe,
        sync::{Arc, atomic::Ordering},
    };
    let clock = Arc::new(ReportClock::new());
    let origin = clock.monotonic();
    let (reports, observed) = mpsc::channel();
    let worker = StorageWorker::start_with_backlog(
        open,
        StorageConns::close,
        clock.clone(),
        move |report| reports.send(report).unwrap(),
    )
    .await
    .unwrap();
    let reporter = worker.handle.backlog_reporter();
    let (release, active) = held_job(&worker.handle, false).await;
    let replies = fill(&worker.handle).await;
    reject(&worker.handle).await;
    assert_eq!(observed.try_recv().unwrap().dropped, 1);
    clock.set(origin, 1, 2000);
    reject(&worker.handle).await;
    clock.panic_once.store(true, Ordering::Relaxed);
    let handle = worker.handle.clone();
    let panicked = tokio::spawn(async move { reject(&handle).await })
        .await
        .unwrap_err();
    assert!(panicked.is_panic());
    reject(&worker.handle).await; // The poisoned accounting lock recovers, including across clones.
    assert_eq!(worker.handle.dropped(), 3); // Clock-failed attempt was not counted.
    clock.set(origin, 60, 3000);
    clock.panic_once.store(true, Ordering::Relaxed);
    assert!(std::panic::catch_unwind(AssertUnwindSafe(|| reporter.take_due())).is_err());
    let report = reporter.take_due().unwrap();
    assert_eq!((report.dropped, report.since.unix_millis()), (2, 2000));
    assert_eq!(reporter.take_due(), None);
    assert!(matches!(
        observed.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    release.send(()).unwrap();
    drain(active, replies).await;
    worker.close().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn competing_reporter_clones_and_drop_share_one_reservation() {
    use den_core::clock::Clock;
    use std::sync::{Arc, Barrier};
    let clock = Arc::new(ReportClock::new());
    let origin = clock.monotonic();
    let (reports, observed) = mpsc::channel();
    let worker = StorageWorker::start_with_backlog(
        open,
        StorageConns::close,
        clock.clone(),
        move |report| reports.send(report).unwrap(),
    )
    .await
    .unwrap();
    let reporter = worker.handle.backlog_reporter();
    let (release, active) = held_job(&worker.handle, false).await;
    let replies = fill(&worker.handle).await;
    reject(&worker.handle).await;
    assert_eq!(observed.try_recv().unwrap().dropped, 1);
    clock.set(origin, 1, 2000);
    reject(&worker.handle).await;
    clock.set(origin, 60, 3000);
    let barrier = Arc::new(Barrier::new(3));
    let mut takers = Vec::new();
    for _ in 0..2 {
        let (reporter, barrier) = (reporter.clone(), barrier.clone());
        takers.push(std::thread::spawn(move || {
            barrier.wait();
            reporter.take_due()
        }));
    }
    barrier.wait();
    reject(&worker.handle).await;
    let mut reports: Vec<_> = takers
        .into_iter()
        .filter_map(|taker| taker.join().unwrap())
        .collect();
    reports.extend(observed.try_iter());
    assert_eq!(reports.len(), 1); // Exactly one winner, regardless of lock acquisition order.
    assert_eq!(reports[0].since.unix_millis(), 2000);
    assert_eq!(reporter.take_due(), None);
    clock.set(origin, 120, 4000);
    let tail = reporter.take_due();
    if let Some(tail) = tail {
        assert_eq!(tail.since.unix_millis(), 3000);
    }
    assert_eq!(
        reports[0].dropped + tail.map_or(0, |report| report.dropped),
        2
    );
    assert_eq!(reporter.take_due(), None);
    assert_eq!(reporter.deadline(), None);
    assert_eq!(worker.handle.dropped(), 3);
    release.send(()).unwrap();
    drain(active, replies).await;
    worker.close().await.unwrap();
}
