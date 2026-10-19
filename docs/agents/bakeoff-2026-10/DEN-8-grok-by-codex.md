# DEN-8 b1 — grok, reviewed by codex

Done-when: PASS

Reviewed `git diff origin/main...HEAD` on `agent/grok/DEN-8-b1`, HEAD `020045942641d06ceeb7f46c6bf190dbc7f92be6`. Only `spikes/S-5-bakeoff/grok/b1/**` was reviewed, against DEN-8's Goal and Done-when and AGENTS.md's code review rules. Passing these checks does not resolve the findings below.

## Findings

### P1 — Runtime shutdown recursively dispatches the backlog and aborts the process

**File:line:** `spikes/S-5-bakeoff/grok/b1/src/lib.rs:252` (also `:625`).

**Failure scenario:** Keep an `ActionQueues` owner alive outside a current-thread Tokio runtime. Create a blocking queue with pending capacity 20,000, enqueue 20,000 futures that await `std::future::pending()`, then drop the runtime. Dropping the active supervisor drops its `SlotGuard`, whose `finish()` calls `schedule()`. Spawning on the shutting-down runtime immediately drops the next supervisor and its guard, recursively repeating this for the backlog. The process exhausts its stack instead of shutting down cleanly.

**Evidence:** An external harness using the unchanged crate as a path dependency printed:

```text
$ timeout 15s cargo run --quiet --manifest-path /tmp/den8-grok-review/Cargo.toml --bin shutdown
dropping runtime with 19999 pending
thread 'main' (...) has overflowed its stack
fatal runtime error: stack overflow, aborting
```

Exit 134. The harness builds a current-thread runtime, enqueues the jobs inside `block_on`, returns from `block_on` with the owner still alive, then calls `drop(runtime)`.

**Suggested fix:** Make dispatch non-recursive, with a single dispatcher or a single-flight iterative pump that handles immediate task destruction without nested scheduling. Ensure runtime shutdown terminates queued runs without spawning recursively. Add a large-backlog runtime-shutdown regression test.

### P2 — Cancellation frees a concurrency slot before the child job stops

**File:line:** `spikes/S-5-bakeoff/grok/b1/src/lib.rs:500` (also `:258` and `:563`).

**Failure scenario:** On a multithreaded runtime, a blocking-queue job is executing its current poll when it is cancelled. The supervisor drops `AbortOnDrop`, which requests cancellation of the separate worker but does not await its termination. `guard.finish()` immediately releases the slot and dispatches the next job; aborting the supervisor directly also releases its guard immediately. The worker may still be executing, so two jobs run simultaneously despite blocking mode. This also affects non-blocking concurrency limits. Even a bounded synchronous segment or slow future destructor can expose this interval.

**Evidence:** An external four-thread harness used atomic flags to hold the first worker inside its current poll, queued a second worker, and cancelled the first handle. Before releasing the first worker, the second finished and reported that the first had not exited:

```text
$ cargo run --quiet --manifest-path /tmp/den8-grok-review/Cargo.toml --bin den8-grok-review
second job ran while first still executing: Ok(Finished(true))
```

Exit 0; this output confirms the concurrency violation. The harness releases the first worker afterward so it can terminate.

**Suggested fix:** Retain the slot until the actual job future has been destroyed. For example, put slot ownership on the child worker and make the supervisor abort and await that worker before reporting cancellation; ensure aborting the supervisor cannot release the slot early. Add a multithreaded cancellation regression test that checks actual worker overlap.

## Done-when evidence

Commands ran in `/home/kris/Documents/den-wt/DEN-8-grok/spikes/S-5-bakeoff/grok/b1` on 2026-10-19:

```text
$ cargo test
   Compiling den-bakeoff-grok-b1 v0.1.0
    Finished `test` profile [unoptimized + debuginfo] target(s) in 2.40s
running 13 tests
[all 13 tests passed]
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   Doc-tests den_bakeoff_grok_b1
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
(exit 0)

$ cargo clippy -- -D warnings
    Checking den-bakeoff-grok-b1 v0.1.0
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.12s
(exit 0)
```

The 13 tests include two executed proptest property tests: `blocking_fifo_survives_arbitrary_queued_cancels` and `nonblocking_never_exceeds_the_concurrency_limit`. Also, `git diff --check origin/main...HEAD` passed. Repository status remained clean after verification.

## Quality 1-5

**3/5** — Clear API, documented spike assumptions and broad passing tests cover the requested features, but runtime shutdown can abort the process and running cancellation violates queue concurrency guarantees.

## Handoff

Review completed; fixes remain with the author. No repository source files or card state were changed. Reproduction code and build artifacts are outside the repository under `/tmp/den8-grok-review`. Assumptions: the documented spike policies for waiting capacity, clear and always-run precedence are acceptable for this review; the sync control API and shared scheduler can be used with a multithreaded Tokio runtime. No product questions arose, so no QUESTIONS.md change was needed. The required fleet-reflect lookup returned `status:missing`; there was no new actionable fleet finding to report.
