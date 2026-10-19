//! Property test: random workloads with random cancels, a pause window and a clear,
//! checked against the queue's invariants on a paused-clock runtime (deterministic timing).

mod common;

use std::collections::HashSet;
use std::num::NonZeroUsize;
use std::time::Duration;

use common::{job, ms, Tracker};
use den_action_queue_b1::{ActionQueue, EnqueueOptions, QueueConfig, QueueMode, RunOutcome};
use proptest::prelude::*;
use tokio::time::{sleep, timeout, Instant};

#[derive(Clone, Debug)]
struct JobSpec {
    dur_ms: u64,
    always_run: bool,
    cancel_at_ms: Option<u64>,
}

#[derive(Clone, Debug)]
struct Scenario {
    limit: usize,
    jobs: Vec<JobSpec>,
    /// (start, length) in ms.
    pause: Option<(u64, u64)>,
    clear_at_ms: Option<u64>,
}

fn scenario() -> impl Strategy<Value = Scenario> {
    let job = (0u64..30, any::<bool>(), proptest::option::of(0u64..60)).prop_map(
        |(dur_ms, always_run, cancel_at_ms)| JobSpec {
            dur_ms,
            always_run,
            cancel_at_ms,
        },
    );
    (
        1usize..=4,
        proptest::collection::vec(job, 0..25),
        proptest::option::of((0u64..40, 1u64..60)),
        proptest::option::weighted(0.25, 0u64..80),
    )
        .prop_map(|(limit, jobs, pause, clear_at_ms)| Scenario {
            limit,
            jobs,
            pause,
            clear_at_ms,
        })
}

fn is_increasing(ids: &[u64]) -> bool {
    ids.windows(2).all(|w| w[0] < w[1])
}

fn run(s: &Scenario) -> Result<(), TestCaseError> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .start_paused(true)
        .build()
        .unwrap();
    rt.block_on(async {
        let mode = if s.limit == 1 {
            QueueMode::Blocking
        } else {
            QueueMode::NonBlocking {
                max_concurrency: NonZeroUsize::new(s.limit).unwrap(),
            }
        };
        let q = ActionQueue::new(QueueConfig {
            name: "prop".into(),
            mode,
            capacity: QueueConfig::DEFAULT_CAPACITY,
        });
        let t = Tracker::new();
        let begin = Instant::now();

        let handles: Vec<_> = s
            .jobs
            .iter()
            .map(|j| {
                let opts = EnqueueOptions {
                    always_run: j.always_run,
                };
                q.enqueue(opts, job(&t, ms(j.dur_ms))).unwrap()
            })
            .collect();

        let mut controls = Vec::new();
        for (h, j) in handles.iter().zip(&s.jobs) {
            if let Some(at) = j.cancel_at_ms {
                let c = h.canceller();
                controls.push(tokio::spawn(async move {
                    sleep(ms(at)).await;
                    c.cancel();
                }));
            }
        }
        if let Some((start, len)) = s.pause {
            let q = q.clone();
            controls.push(tokio::spawn(async move {
                sleep(ms(start)).await;
                q.pause();
                sleep(ms(len)).await;
                q.resume();
            }));
        }
        if let Some(at) = s.clear_at_ms {
            let q = q.clone();
            controls.push(tokio::spawn(async move {
                sleep(ms(at)).await;
                q.clear();
            }));
        }

        // A run that is never dispatched fails the case instead of hanging the suite
        // (the paused clock jumps to the deadline once every task is idle).
        let collect = async {
            let mut outcomes = Vec::new();
            for h in handles {
                outcomes.push(h.outcome().await);
            }
            outcomes
        };
        let Ok(outcomes) = timeout(Duration::from_secs(3600), collect).await else {
            return Err(TestCaseError::fail("runs never resolved (scheduling hang)"));
        };
        for c in controls {
            c.await.unwrap();
        }

        // 1. Never more runs at once than the mode allows.
        prop_assert!(t.peak() <= s.limit, "peak {} > limit {}", t.peak(), s.limit);

        // 2. Every run resolved exactly once and the counters agree with the outcomes.
        let stats = q.stats();
        let completed = outcomes
            .iter()
            .filter(|o| matches!(o, RunOutcome::Completed(_)))
            .count() as u64;
        let cancelled = outcomes
            .iter()
            .filter(|o| matches!(o, RunOutcome::Cancelled | RunOutcome::Cleared))
            .count() as u64;
        prop_assert_eq!(stats.pending, 0);
        prop_assert_eq!(stats.running, 0);
        prop_assert_eq!(stats.aborted, 0);
        prop_assert_eq!(stats.completed, completed);
        prop_assert_eq!(stats.cancelled, cancelled);
        prop_assert_eq!(completed + cancelled, s.jobs.len() as u64);

        // 3. Outcomes are consistent with whether the job started.
        let started: HashSet<u64> = t.started_ids().into_iter().collect();
        for (i, o) in outcomes.iter().enumerate() {
            let id = i as u64;
            match o {
                RunOutcome::Completed(v) => {
                    prop_assert_eq!(*v, id);
                    prop_assert!(started.contains(&id));
                }
                RunOutcome::Cleared => prop_assert!(!started.contains(&id)),
                RunOutcome::Cancelled => prop_assert!(s.jobs[i].cancel_at_ms.is_some()),
                RunOutcome::Aborted => prop_assert!(false, "unexpected abort of run {}", id),
            }
        }

        // 4. FIFO dispatch: always among normal runs and among always-run runs; overall
        //    when the queue was never paused (pausing lets always-run runs overtake).
        let order = t.starts_sorted();
        let ids: Vec<u64> = order.iter().map(|o| o.1).collect();
        let (always, normal): (Vec<u64>, Vec<u64>) =
            ids.iter().partition(|&&id| s.jobs[id as usize].always_run);
        prop_assert!(
            is_increasing(&normal),
            "normal runs out of order: {:?}",
            normal
        );
        prop_assert!(
            is_increasing(&always),
            "always-run runs out of order: {:?}",
            always
        );
        if s.pause.is_none() {
            prop_assert!(is_increasing(&ids), "runs out of order: {:?}", ids);
        }

        // 5. While paused, only always-run runs start.
        if let Some((start, len)) = s.pause {
            for (_, id, at) in &order {
                let offset = at.duration_since(begin);
                if offset > ms(start) && offset < ms(start + len) {
                    prop_assert!(
                        s.jobs[*id as usize].always_run,
                        "run {} started at {:?} inside the pause window",
                        id,
                        offset
                    );
                }
            }
        }
        Ok(())
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn queue_invariants_hold_for_random_workloads(s in scenario()) {
        run(&s)?;
    }
}
