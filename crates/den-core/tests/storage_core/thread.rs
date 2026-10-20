use den_core::storage::thread::{
    Admission, JobClass, Reply, StorageConns, StorageError, StorageWorker,
};
use rusqlite::Connection;
use std::{future::Future, sync::mpsc, task::Poll};
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
