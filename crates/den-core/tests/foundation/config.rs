//! Independent transcription of accepted Spec 03 r6 §16; no production metadata drives assertions.
use std::{
    sync::{Arc, Barrier},
    time::Duration,
};

use den_core::config::{ConfigHandle, ConfigRangeError, EngineConfig};

struct Row {
    key: &'static str,
    field: fn(&mut EngineConfig) -> &mut u64,
    default: u64,
    min: u64,
    max: u64,
}
macro_rules! spec_rows {
    ($($key:literal => $section:ident.$field:ident: $default:literal, $min:literal, $max:literal;)*) => {
        fn rows() -> [Row; 34] { [$(Row { key: $key, field: |c| &mut c.$section.$field,
            default: $default, min: $min, max: $max },)*] }
    };
}
spec_rows! {
    "engine.bus.capacity" => bus.capacity: 8192, 256, 65536;
    "engine.bus.internalCapacity" => bus.internal_capacity: 16384, 256, 65536;
    "engine.bus.subscriberCapacity" => bus.subscriber_capacity: 4096, 64, 65536;
    "engine.bus.criticalLane" => bus.critical_lane: 64, 8, 1024;
    "engine.bus.subscriberStallMs" => bus.subscriber_stall_ms: 5000, 1000, 60000;
    "engine.run.maxConcurrent" => run.max_concurrent: 256, 1, 4096;
    "engine.run.maxTasksPerRun" => run.max_tasks_per_run: 64, 1, 1024;
    "engine.run.maxRunBytes" => run.max_run_bytes: 33554432, 1048576, 1073741824;
    "engine.run.maxNodeExecutions" => run.max_node_executions: 1000000, 1000, 100000000;
    "engine.run.opTimeoutMs" => run.op_timeout_ms: 30000, 1000, 600000;
    "engine.run.codeTimeoutMs" => run.code_timeout_ms: 5000, 100, 600000;
    "engine.run.maxLoopIterations" => run.max_loop_iterations: 10000, 1, 1000000;
    "engine.run.maxDelayMs" => run.max_delay_ms: 3600000, 1000, 86400000;
    "engine.run.maxInlineDepth" => run.max_inline_depth: 32, 1, 128;
    "engine.run.maxLineage" => run.max_lineage: 64, 1, 256;
    "engine.run.maxChainHops" => run.max_chain_hops: 16, 1, 64;
    "engine.run.maxArgsBytes" => run.max_args_bytes: 4194304, 65536, 67108864;
    "engine.run.attributionWindowMs" => run.attribution_window_ms: 300000, 10000, 3600000;
    "engine.run.stormThreshold" => run.storm_threshold: 200, 10, 10000;
    "engine.queue.defaultMaxPending" => queue.default_max_pending: 1000, 0, 100000;
    "engine.queue.maxRunCeilingMs" => queue.max_run_ceiling_ms: 3600000, 1000, 86400000;
    "engine.signals.queuedTtlMs" => signals.queued_ttl_ms: 30000, 1000, 300000;
    "engine.signals.maxWaitMs" => signals.max_wait_ms: 30000, 1000, 300000;
    "engine.variables.maxValueBytes" => variables.max_value_bytes: 262144, 1024, 4194304;
    "engine.storage.flushMs" => storage.flush_ms: 1000, 0, 10000;
    "engine.history.maxRuns" => history.max_runs: 10000, 100, 1000000;
    "engine.history.maxAgeDays" => history.max_age_days: 7, 1, 365;
    "engine.history.maxSnapshotBytes" => history.max_snapshot_bytes: 262144, 4096, 1048576;
    "engine.history.traceRuns" => history.trace_runs: 200, 0, 10000;
    "engine.events.maxAgeHours" => events.max_age_hours: 24, 1, 8760;
    "engine.events.maxCount" => events.max_count: 50000, 1000, 10000000;
    "engine.shutdown.graceMs" => shutdown.grace_ms: 5000, 0, 60000;
    "engine.action.maxNodes" => action.max_nodes: 2000, 50, 20000;
    "engine.action.maxDepth" => action.max_depth: 32, 4, 64;
}

#[test]
fn config_defaults_match_spec_16() {
    let mut config = EngineConfig::default();
    for row in rows() {
        assert_eq!(*(row.field)(&mut config), row.default, "{}", row.key);
    }
    assert!(config.storage.daily_backup);
    assert_eq!(config.validate(), Ok(()));
    config.storage.daily_backup = false;
    assert_eq!(config.validate(), Ok(()));
}

#[test]
fn config_ranges_rejected_at_boundaries() {
    let mut all_invalid = EngineConfig::default();
    for row in rows() {
        let mut values = vec![row.min, row.max, row.max + 1, u64::MAX];
        if row.min > 0 {
            values.push(row.min - 1);
        }
        for value in values {
            let mut config = EngineConfig::default();
            *(row.field)(&mut config) = value;
            let before = config.clone();
            let expected = if value >= row.min && value <= row.max {
                Ok(())
            } else {
                Err(vec![ConfigRangeError {
                    key: row.key,
                    value,
                    minimum: row.min,
                    maximum: row.max,
                }])
            };
            assert_eq!(config.validate(), expected, "{} = {value}", row.key);
            assert_eq!(config, before, "validation must not clamp");
        }
        *(row.field)(&mut all_invalid) = row.max + 1;
    }
    assert_eq!(all_invalid.validate().unwrap_err().len(), 34);
}

#[test]
fn restart_only_keys() {
    let restart = [
        "engine.bus.capacity",
        "engine.bus.internalCapacity",
        "engine.bus.subscriberCapacity",
    ];
    for key in restart {
        assert!(EngineConfig::is_restart_only(key));
    }
    for row in rows() {
        assert_eq!(
            EngineConfig::is_restart_only(row.key),
            restart.contains(&row.key)
        );
    }
    for key in [
        "engine.storage.dailyBackup",
        "capacity",
        "internalCapacity",
        "engine.bus",
        "engine.bus.capacity.extra",
        "Engine.bus.capacity",
        "",
    ] {
        assert!(!EngineConfig::is_restart_only(key), "{key}");
    }
}

#[test]
fn memory_warning_threshold() {
    let mut config = EngineConfig::default();
    // Default reservation is exactly 8 GiB; equality at half of 16 GiB is not a warning.
    assert!(!config.memory_warning(17_179_869_184));
    assert!(config.memory_warning(17_179_869_183));
    assert!(!config.memory_warning(17_179_869_185));
    assert!(config.memory_warning(0));
    config.run.max_concurrent = u64::MAX;
    config.run.max_run_bytes = u64::MAX;
    assert!(config.memory_warning(u64::MAX));
    config.run.max_run_bytes = 0;
    assert!(!config.memory_warning(0));
}

#[tokio::test]
async fn config_handle_swap_visible_to_readers() {
    let handle = ConfigHandle::new(EngineConfig::default()).unwrap();
    let reader = handle.clone();
    let old = reader.load();
    let mut changes = reader.subscribe();
    let mut next = (*old).clone();
    next.run.max_concurrent = 1;
    handle.swap(next.clone()).unwrap();
    tokio::time::timeout(Duration::from_secs(5), changes.changed())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(*reader.load(), next);
    assert!(Arc::ptr_eq(&reader.load(), &changes.borrow_and_update()));
    assert_eq!(old.run.max_concurrent, 256);
    next.history.max_age_days = 366;
    assert!(ConfigHandle::new(next.clone()).is_err());
    assert!(handle.swap(next).is_err());
    assert!(!changes.has_changed().unwrap());
    assert_eq!(reader.load().history.max_age_days, 7);
    drop(changes);
    handle.swap(EngineConfig::default()).unwrap();
    assert!(Arc::ptr_eq(&reader.load(), &reader.subscribe().borrow()));
}

#[tokio::test]
async fn config_handle_concurrent_writers_are_coherent() {
    let mut initial = EngineConfig::default();
    initial.run.max_concurrent = 1;
    initial.run.max_tasks_per_run = 1;
    let handle = ConfigHandle::new(initial).unwrap();
    let mut changes = handle.subscribe();
    let writer = handle.clone();
    let mut writers = tokio::task::spawn_blocking(move || {
        let barrier = Barrier::new(4);
        std::thread::scope(|scope| {
            for thread in 0..4 {
                let (writer, barrier) = (&writer, &barrier);
                scope.spawn(move || {
                    barrier.wait();
                    for i in 1..=256 {
                        let mut next = EngineConfig::default();
                        next.run.max_concurrent = thread * 256 + i;
                        next.run.max_tasks_per_run = thread * 256 + i;
                        writer.swap(next).unwrap();
                        std::thread::yield_now();
                    }
                });
            }
        });
    });
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            tokio::select! {
                result = &mut writers => { result.unwrap(); break; }
                result = changes.changed() => {
                    result.unwrap();
                    let published = changes.borrow_and_update();
                    assert!(Arc::ptr_eq(&published, &handle.load()));
                    assert_eq!(published.run.max_concurrent, published.run.max_tasks_per_run);
                }
            }
        }
    })
    .await
    .unwrap();
    // The final notification must describe the final store, even when intermediate updates coalesced.
    assert!(Arc::ptr_eq(&changes.borrow_and_update(), &handle.load()));
}
