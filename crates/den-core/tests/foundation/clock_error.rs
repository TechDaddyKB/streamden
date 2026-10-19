//! DEN-147 clock, runtime and protocol error acceptance tests.

use std::{num::NonZeroUsize, sync::mpsc, time::Duration};

use den_core::{
    clock::{Clock, ClockSample, SuspendDetector, SystemClock, build_runtime},
    error::EngineError,
};
use den_protocol::{
    Timestamp,
    rpc::{ErrorCode, ErrorData},
};
use serde_json::json;
use tokio::time::Instant;

fn sample(monotonic: Instant, millis: i64) -> ClockSample {
    ClockSample {
        monotonic,
        wall: Timestamp::from_unix_millis(millis).unwrap(),
    }
}

#[test]
fn suspend_detector_flags_wall_jump() {
    let now = Instant::now();
    let mut detector = SuspendDetector::new(sample(now, 0), Duration::from_millis(100));
    assert!(detector.on_wake(sample(now + Duration::from_secs(1), 61_000)));
    assert!(!detector.on_wake(sample(now + Duration::from_secs(2), 62_000)));
}

#[test]
fn suspend_detector_handles_normal_tolerance_and_backward_wall() {
    let now = Instant::now();
    let mut detector = SuspendDetector::new(sample(now, 0), Duration::from_millis(100));
    assert!(!detector.on_wake(sample(now + Duration::from_secs(1), 1_000)));
    assert!(!detector.on_wake(sample(now + Duration::from_secs(2), 2_100)));
    assert!(detector.on_wake(sample(now + Duration::from_secs(3), 3_201)));
    assert!(!detector.on_wake(sample(now + Duration::from_secs(4), -1_000)));
    assert!(!detector.on_wake(sample(now + Duration::from_secs(5), 0)));
}

#[test]
fn suspend_detector_rebases_backward_monotonic_samples() {
    let now = Instant::now();
    let mut detector = SuspendDetector::new(sample(now, 0), Duration::ZERO);
    assert!(!detector.on_wake(sample(now - Duration::from_secs(1), 50_000)));
    assert!(!detector.on_wake(sample(now, 51_000)));
}

#[tokio::test(start_paused = true)]
async fn clock_uses_pausable_tokio_time_and_protocol_wall_time() {
    let clock = SystemClock;
    let before = clock.sample();
    assert_eq!(clock.monotonic(), before.monotonic);
    tokio::time::advance(Duration::from_secs(60)).await;
    assert_eq!(
        clock.monotonic() - before.monotonic,
        Duration::from_secs(60)
    );
    let wall = clock.wall();
    assert_eq!(Timestamp::parse(&wall.to_string()).unwrap(), wall);
    // Tokio advancement must not advance the production wall source.
    assert!(wall.unix_millis() - before.wall.unix_millis() < 60_000);
}

#[test]
fn runtime_honors_explicit_worker_and_blocking_caps() {
    let runtime =
        build_runtime(NonZeroUsize::new(2).unwrap(), NonZeroUsize::new(1).unwrap()).unwrap();
    assert_eq!(runtime.metrics().num_workers(), 2);
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let first = runtime.spawn_blocking(move || {
        started_tx.send(()).unwrap();
        release_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    });
    started_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    let (second_tx, second_rx) = mpsc::channel();
    let second = runtime.spawn_blocking(move || second_tx.send(()).unwrap());
    let queued = second_rx.recv_timeout(Duration::from_millis(100));
    // Release before assertions so even a broken cap cannot leave a blocked runtime at drop.
    release_tx.send(()).unwrap();
    runtime.block_on(async {
        first.await.unwrap();
        second.await.unwrap();
        tokio::time::sleep(Duration::from_millis(1)).await;
    });
    assert_eq!(queued, Err(mpsc::RecvTimeoutError::Timeout));
    second_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(NonZeroUsize::new(0).is_none());
}

#[test]
fn error_codes_map_to_protocol() {
    for (error, code, reason) in [
        (EngineError::queue_full(), "INVALID_STATE", "queue_full"),
        (
            EngineError::shutting_down(),
            "INVALID_STATE",
            "shutting_down",
        ),
        (EngineError::node_budget(), "INVALID_STATE", "node_budget"),
        (EngineError::run_budget(), "PAYLOAD_TOO_LARGE", "run_budget"),
        (EngineError::inline_depth(), "INVALID_STATE", "inline_depth"),
        (EngineError::panic(), "INTERNAL", "panic"),
        (
            EngineError::payload_version(),
            "INVALID_STATE",
            "payload_version",
        ),
        (
            EngineError::not_iterable(),
            "INVALID_PARAMS",
            "not_iterable",
        ),
        (
            EngineError::invalid_result(),
            "UPSTREAM_ERROR",
            "invalid_result",
        ),
        (EngineError::read_only(), "INVALID_STATE", "read_only"),
    ] {
        assert_eq!(error.code().0, code);
        assert_eq!(error.reason(), Some(reason));
        assert_eq!(error.details(), Some(json!({ "reason": reason })));
        let data: ErrorData = error.into();
        assert_eq!(
            serde_json::to_value(data).unwrap(),
            json!({
                "code": code, "details": { "reason": reason }
            })
        );
    }
    let error = EngineError::new(ErrorCode(ErrorCode::TIMEOUT.to_owned()));
    assert_eq!(error.reason(), None);
    assert_eq!(error.details(), None);
    assert_eq!(
        serde_json::to_value(ErrorData::from(error)).unwrap(),
        json!({ "code": "TIMEOUT" })
    );
}

#[test]
fn panic_and_result_errors_only_expose_fixed_metadata() {
    for (error, code, reason) in [
        (EngineError::panic(), "INTERNAL", "panic"),
        (
            EngineError::invalid_result(),
            "UPSTREAM_ERROR",
            "invalid_result",
        ),
    ] {
        assert_eq!(error.to_string(), format!("{code}: {reason}"));
        assert_eq!(
            format!("{error:?}"),
            format!("EngineError {{ code: ErrorCode({code:?}), reason: Some({reason:?}) }}")
        );
        assert!(std::error::Error::source(&error).is_none());
    }
}
