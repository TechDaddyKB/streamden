//! Internal effective settings from accepted Spec 03 r6 §16 (not a wire document).
//! Limits are read at decision time. Restart reconciliation and persistence belong to DEN-119.

use std::sync::Arc;

use arc_swap::ArcSwap;
use tokio::sync::watch;

/// One rejected setting; keys are dotted document keys, not request JSON Pointers.
/// The settings operation must map these into its request paths (Spec 02 §8.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigRangeError {
    pub key: &'static str,
    pub value: u64,
    pub minimum: u64,
    pub maximum: u64,
}

// Keep defaults, field documentation and validation together; no serialized contract is generated.
macro_rules! engine_config {
    ($($section:ident: $name:ident => $path:literal {
        $($field:ident: $ty:ty = $default:literal $(, $min:literal..=$max:literal)? => $key:literal;)*
    })*) => {
        /// Typed engine settings. Numeric values use bytes, milliseconds or counts as named in §16.
        #[derive(Debug, Clone, Default, PartialEq, Eq)]
        pub struct EngineConfig { $(pub $section: $name,)* }
        $(
            #[doc = concat!("Internal `", $path, "` settings (Spec 03 §16).")]
            #[derive(Debug, Clone, PartialEq, Eq)]
            pub struct $name {
                $(#[doc = concat!($path, ".", $key, ": default ", stringify!($default),
                    $("; inclusive range ", stringify!($min), "..=", stringify!($max),)? ".")]
                pub $field: $ty,)*
            }
            impl Default for $name {
                fn default() -> Self { Self { $($field: $default,)* } }
            }
        )*
        impl EngineConfig {
            /// Reject every out-of-range field without modifying this value; never clamp.
            pub fn validate(&self) -> Result<(), Vec<ConfigRangeError>> {
                let mut errors = Vec::new();
                $($($(
                    if !($min..=$max).contains(&self.$section.$field) {
                        errors.push(ConfigRangeError { key: concat!($path, ".", $key),
                            value: self.$section.$field, minimum: $min, maximum: $max });
                    }
                )?)*)*
                if errors.is_empty() { Ok(()) } else { Err(errors) }
            }
        }
    };
}

engine_config! {
    bus: BusConfig => "engine.bus" {
        capacity: u64 = 8192, 256..=65536 => "capacity";
        internal_capacity: u64 = 16384, 256..=65536 => "internalCapacity";
        subscriber_capacity: u64 = 4096, 64..=65536 => "subscriberCapacity";
        critical_lane: u64 = 64, 8..=1024 => "criticalLane";
        subscriber_stall_ms: u64 = 5000, 1000..=60000 => "subscriberStallMs";
    }
    run: RunConfig => "engine.run" {
        max_concurrent: u64 = 256, 1..=4096 => "maxConcurrent";
        max_tasks_per_run: u64 = 64, 1..=1024 => "maxTasksPerRun";
        max_run_bytes: u64 = 33554432, 1048576..=1073741824 => "maxRunBytes";
        max_node_executions: u64 = 1000000, 1000..=100000000 => "maxNodeExecutions";
        op_timeout_ms: u64 = 30000, 1000..=600000 => "opTimeoutMs";
        code_timeout_ms: u64 = 5000, 100..=600000 => "codeTimeoutMs";
        max_loop_iterations: u64 = 10000, 1..=1000000 => "maxLoopIterations";
        max_delay_ms: u64 = 3600000, 1000..=86400000 => "maxDelayMs";
        max_inline_depth: u64 = 32, 1..=128 => "maxInlineDepth";
        max_lineage: u64 = 64, 1..=256 => "maxLineage";
        max_chain_hops: u64 = 16, 1..=64 => "maxChainHops";
        max_args_bytes: u64 = 4194304, 65536..=67108864 => "maxArgsBytes";
        attribution_window_ms: u64 = 300000, 10000..=3600000 => "attributionWindowMs";
        storm_threshold: u64 = 200, 10..=10000 => "stormThreshold";
    }
    queue: QueueConfig => "engine.queue" {
        default_max_pending: u64 = 1000, 0..=100000 => "defaultMaxPending";
        max_run_ceiling_ms: u64 = 3600000, 1000..=86400000 => "maxRunCeilingMs";
    }
    signals: SignalsConfig => "engine.signals" {
        queued_ttl_ms: u64 = 30000, 1000..=300000 => "queuedTtlMs";
        max_wait_ms: u64 = 30000, 1000..=300000 => "maxWaitMs";
    }
    variables: VariablesConfig => "engine.variables" {
        max_value_bytes: u64 = 262144, 1024..=4194304 => "maxValueBytes";
    }
    storage: StorageConfig => "engine.storage" {
        flush_ms: u64 = 1000, 0..=10000 => "flushMs";
        daily_backup: bool = true => "dailyBackup";
    }
    history: HistoryConfig => "engine.history" {
        max_runs: u64 = 10000, 100..=1000000 => "maxRuns";
        max_age_days: u64 = 7, 1..=365 => "maxAgeDays";
        max_snapshot_bytes: u64 = 262144, 4096..=1048576 => "maxSnapshotBytes";
        trace_runs: u64 = 200, 0..=10000 => "traceRuns";
    }
    events: EventsConfig => "engine.events" {
        max_age_hours: u64 = 24, 1..=8760 => "maxAgeHours";
        max_count: u64 = 50000, 1000..=10000000 => "maxCount";
    }
    shutdown: ShutdownConfig => "engine.shutdown" {
        grace_ms: u64 = 5000, 0..=60000 => "graceMs";
    }
    action: ActionConfig => "engine.action" {
        max_nodes: u64 = 2000, 50..=20000 => "maxNodes";
        max_depth: u64 = 32, 4..=64 => "maxDepth";
    }
}

impl EngineConfig {
    /// Only these channel capacities retain boot values until restart (§16).
    pub fn is_restart_only(key: &str) -> bool {
        matches!(
            key,
            "engine.bus.capacity" | "engine.bus.internalCapacity" | "engine.bus.subscriberCapacity"
        )
    }

    /// Advisory only: strict exceedance of half the injected physical RAM (§9.6).
    /// Widen before multiplication, even for unvalidated u64 values; no host probing.
    pub fn memory_warning(&self, physical_ram_bytes: u64) -> bool {
        u128::from(self.run.max_concurrent) * u128::from(self.run.max_run_bytes)
            > u128::from(physical_ram_bytes) / 2
    }
}

/// Clones share effective settings. Load at each decision; held snapshots remain immutable.
/// Watch coalesces updates: its value is the last published snapshot, not an event queue.
#[derive(Clone)]
pub struct ConfigHandle {
    current: Arc<ArcSwap<EngineConfig>>,
    changes: watch::Sender<Arc<EngineConfig>>,
}

impl ConfigHandle {
    pub fn new(initial: EngineConfig) -> Result<Self, Vec<ConfigRangeError>> {
        initial.validate()?;
        let initial = Arc::new(initial);
        let (changes, _) = watch::channel(Arc::clone(&initial));
        Ok(Self {
            current: Arc::new(ArcSwap::from(initial)),
            changes,
        })
    }

    pub fn load(&self) -> Arc<EngineConfig> {
        self.current.load_full()
    }

    /// Subscribe before loading/awaiting to avoid missing an intervening swap.
    /// Clone a borrowed watch value and drop its guard before awaiting or calling swap.
    pub fn subscribe(&self) -> watch::Receiver<Arc<EngineConfig>> {
        self.changes.subscribe()
    }

    /// Swap a validated effective config, then notify, even with no current receivers.
    /// The watch write lock serializes writers across the store and watch-version update.
    /// A watch borrow pins that publication; a later load without it may see a newer value.
    /// Callers reconcile restart-only boot values before supplying the effective config.
    pub fn swap(&self, next: EngineConfig) -> Result<(), Vec<ConfigRangeError>> {
        next.validate()?;
        let next = Arc::new(next);
        self.changes.send_modify(|published| {
            self.current.store(Arc::clone(&next));
            *published = next;
        });
        Ok(())
    }
}
