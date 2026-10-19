//! Python worker pool: N pre-warmed `python3` processes speaking bidirectional JSON-RPC 2.0
//! with LSP-style `Content-Length` framing over stdin/stdout. Soft timeout = `$/cancelRequest`
//! (worker raises KeyboardInterrupt in its main thread); hard timeout = kill + respawn.

use std::{
    collections::{HashMap, VecDeque},
    path::PathBuf,
    process::Stdio,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use serde_json::{Value as Json, json};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, ChildStdout, Command},
    sync::{Mutex as AMutex, Notify, Semaphore, oneshot, watch},
};

use crate::{Globals, Report, Status, fd, rss_mib_of, stats};

#[derive(Debug)]
pub enum ExecError {
    Script(String),
    SoftTimeout,
    HardKilled,
    Crashed(String),
}

type RpcResult = Result<Json, (i64, String)>;
type Pending = Arc<Mutex<HashMap<u64, oneshot::Sender<RpcResult>>>>;
type Stdin = Arc<AMutex<Option<ChildStdin>>>;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CancelMode {
    /// `$/cancelRequest` notification; the worker's reader thread interrupts the main thread.
    /// Portable (Windows: `_thread.interrupt_main()`), but needs the GIL in the worker.
    Protocol,
    /// Host sends SIGINT to the worker pid (Unix only). Needs no worker-side Python code to run.
    Signal,
}

pub struct Config {
    pub cancel: CancelMode,
    pub python: String,
    pub worker: PathBuf,
    pub max_mem_mb: u64,
    pub grace: Duration,
}

struct Worker {
    pid: u32,
    version: String,
    child: Child,
    stdin: Stdin,
    pending: Pending,
    /// Logs of the execution currently in flight (filled by `log` requests/notifications).
    logs: Arc<Mutex<Vec<String>>>,
    alive: Arc<AtomicBool>,
    stderr_tail: Arc<Mutex<VecDeque<String>>>,
    stderr_eof: watch::Receiver<bool>,
    next_id: u64,
    cancel: CancelMode,
}

pub struct Outcome {
    pub pid: u32,
    pub elapsed: Duration,
}

async fn write_frame(stdin: &Stdin, v: &Json) -> std::io::Result<()> {
    let body = serde_json::to_vec(v)?;
    let mut buf = format!("Content-Length: {}\r\n\r\n", body.len()).into_bytes();
    buf.extend_from_slice(&body);
    let mut guard = stdin.lock().await;
    let w = guard.as_mut().ok_or(std::io::ErrorKind::BrokenPipe)?;
    w.write_all(&buf).await?;
    w.flush().await
}

async fn read_frame(r: &mut BufReader<ChildStdout>) -> std::io::Result<Option<Vec<u8>>> {
    let mut len = None;
    let mut line = String::new();
    loop {
        line.clear();
        if r.read_line(&mut line).await? == 0 {
            return Ok(None);
        }
        let t = line.trim_end();
        if t.is_empty() {
            break;
        }
        if let Some((k, v)) = t.split_once(':')
            && k.trim().eq_ignore_ascii_case("content-length")
        {
            len = v.trim().parse::<usize>().ok();
        }
    }
    let len = len.ok_or(std::io::ErrorKind::InvalidData)?;
    let mut buf = vec![0; len];
    r.read_exact(&mut buf).await?;
    Ok(Some(buf))
}

/// Host-side handler for requests the *worker* sends while a script runs.
fn handle_worker_request(method: &str, params: &Json, globals: &Globals, logs: &Mutex<Vec<String>>) -> RpcResult {
    let name = params.get("name").and_then(Json::as_str).unwrap_or_default();
    match method {
        "globals.get" => Ok(globals.lock().unwrap().get(name).cloned().unwrap_or(Json::Null)),
        "globals.set" => {
            let v = params.get("value").cloned().unwrap_or(Json::Null);
            globals.lock().unwrap().insert(name.to_string(), v);
            Ok(Json::Null)
        }
        "log" => {
            let m = params.get("message").and_then(Json::as_str).unwrap_or_default();
            logs.lock().unwrap().push(m.to_string());
            Ok(Json::Null)
        }
        _ => Err((-32601, format!("method not found: {method}"))),
    }
}

impl Worker {
    async fn spawn(cfg: &Config, globals: Globals) -> Result<(Worker, Duration), String> {
        let t0 = Instant::now();
        let mut cmd = Command::new(&cfg.python);
        cmd.arg("-I") // isolated: ignore PYTHON* env and user site-packages
            .arg("-B") // don't write .pyc next to the (possibly read-only) worker file
            .arg(&cfg.worker)
            .env("DEN_WORKER_MAX_MEM_MB", cfg.max_mem_mb.to_string())
            .env("DEN_WORKER_NO_COREDUMP", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        // Own process group: a terminal Ctrl+C aimed at the host must not hit the workers.
        #[cfg(unix)]
        cmd.process_group(0);
        let mut child = cmd.spawn().map_err(|e| format!("spawn {}: {e}", cfg.python))?;
        let stdin: Stdin = Arc::new(AMutex::new(child.stdin.take()));
        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();
        let pending: Pending = Arc::default();
        let logs: Arc<Mutex<Vec<String>>> = Arc::default();
        let alive = Arc::new(AtomicBool::new(true));
        let stderr_tail: Arc<Mutex<VecDeque<String>>> = Arc::default();

        // stderr MUST be drained continuously: a full 64 KiB pipe blocks the worker forever.
        let tail = stderr_tail.clone();
        let (eof_tx, stderr_eof) = watch::channel(false);
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(l)) = lines.next_line().await {
                let mut t = tail.lock().unwrap();
                if t.len() == 64 {
                    t.pop_front();
                }
                t.push_back(l.chars().take(200).collect());
            }
            let _ = eof_tx.send(true);
        });

        let (ready_tx, ready_rx) = oneshot::channel::<Json>();
        let (p, l, a, si) = (pending.clone(), logs.clone(), alive.clone(), stdin.clone());
        tokio::spawn(async move {
            let mut ready_tx = Some(ready_tx);
            let mut r = BufReader::with_capacity(64 * 1024, stdout);
            while let Ok(Some(frame)) = read_frame(&mut r).await {
                let Ok(msg) = serde_json::from_slice::<Json>(&frame) else { break };
                match (msg.get("method").and_then(Json::as_str), msg.get("id")) {
                    (Some(method), Some(id)) => {
                        // Request from the worker. Answer from a separate task so this loop
                        // keeps draining stdout (never block a pipe reader on a pipe write).
                        let reply = match handle_worker_request(method, &msg["params"], &globals, &l) {
                            Ok(v) => json!({"jsonrpc": "2.0", "id": id, "result": v}),
                            Err((code, m)) => json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": m}}),
                        };
                        let si = si.clone();
                        tokio::spawn(async move {
                            let _ = write_frame(&si, &reply).await;
                        });
                    }
                    (Some("ready"), None) => {
                        if let Some(tx) = ready_tx.take() {
                            let _ = tx.send(msg["params"].clone());
                        }
                    }
                    (Some("log"), None) => {
                        let m = msg["params"]["message"].as_str().unwrap_or_default().to_string();
                        l.lock().unwrap().push(m);
                    }
                    (Some(_), None) => {}
                    (None, Some(id)) => {
                        let Some(id) = id.as_u64() else { continue };
                        if let Some(tx) = p.lock().unwrap().remove(&id) {
                            let res = match msg.get("error") {
                                Some(e) => Err((e["code"].as_i64().unwrap_or(0), e["message"].as_str().unwrap_or("").to_string())),
                                None => Ok(msg.get("result").cloned().unwrap_or(Json::Null)),
                            };
                            let _ = tx.send(res);
                        }
                    }
                    (None, None) => {}
                }
            }
            // EOF: the worker exited or crashed. Dropping the senders fails every waiter.
            a.store(false, Ordering::SeqCst);
            p.lock().unwrap().clear();
        });

        let params = tokio::time::timeout(Duration::from_secs(10), ready_rx)
            .await
            .map_err(|_| "worker did not become ready in 10s".to_string())?
            .map_err(|_| "worker exited before ready".to_string())?;
        let w = Worker {
            pid: params["pid"].as_u64().unwrap_or(0) as u32,
            version: params["version"].as_str().unwrap_or("?").to_string(),
            child,
            stdin,
            pending,
            logs,
            alive,
            stderr_tail,
            stderr_eof,
            next_id: 1,
            cancel: cfg.cancel,
        };
        Ok((w, t0.elapsed()))
    }

    fn alive(&self) -> bool {
        self.alive.load(Ordering::SeqCst)
    }

    async fn exit_reason(&mut self) -> String {
        let status = tokio::time::timeout(Duration::from_secs(2), self.child.wait()).await;
        let mut s = match status {
            Ok(Ok(st)) => {
                #[cfg(unix)]
                {
                    use std::os::unix::process::ExitStatusExt;
                    match st.signal() {
                        Some(sig) => format!("killed by signal {sig}"),
                        None => format!("exit code {}", st.code().unwrap_or(-1)),
                    }
                }
                #[cfg(not(unix))]
                format!("{st}")
            }
            _ => "unknown exit".into(),
        };
        // Let the stderr drain reach EOF so the faulthandler output is in the tail.
        let mut eof = self.stderr_eof.clone();
        let _ = tokio::time::timeout(Duration::from_millis(200), eof.wait_for(|v| *v)).await;
        if let Some(first) = self.stderr_tail.lock().unwrap().iter().find(|l| l.contains("Fatal Python error")) {
            s.push_str(&format!("; stderr: \"{first}\""));
        }
        s
    }

    async fn execute(&mut self, code: &str, args: &Json, timeout: Duration, grace: Duration) -> Result<Json, ExecError> {
        self.logs.lock().unwrap().clear();
        let id = self.next_id;
        self.next_id += 1;
        let (tx, mut rx) = oneshot::channel();
        self.pending.lock().unwrap().insert(id, tx);
        let req = json!({"jsonrpc": "2.0", "id": id, "method": "execute", "params": {"code": code, "args": args}});
        let deadline = tokio::time::Instant::now() + timeout;
        let sent = tokio::time::timeout_at(deadline, write_frame(&self.stdin, &req)).await;
        if !matches!(sent, Ok(Ok(()))) {
            // Mark dead now: the reader task may not have seen EOF yet, and the pool must not
            // put this worker back into the idle queue.
            self.alive.store(false, Ordering::SeqCst);
            let _ = self.child.start_kill();
            return Err(ExecError::Crashed(format!("write failed: {}", self.exit_reason().await)));
        }
        let res = match tokio::time::timeout_at(deadline, &mut rx).await {
            Ok(r) => r,
            Err(_) => {
                // Soft timeout: ask the worker to interrupt the script, then wait `grace`.
                match self.cancel {
                    CancelMode::Protocol => {
                        let cancel = json!({"jsonrpc": "2.0", "method": "$/cancelRequest", "params": {"id": id}});
                        let _ = tokio::time::timeout(grace, write_frame(&self.stdin, &cancel)).await;
                    }
                    CancelMode::Signal => {
                        #[cfg(unix)]
                        // SAFETY: plain kill(2) on a pid we own and have not reaped yet.
                        unsafe {
                            libc::kill(self.pid as libc::pid_t, libc::SIGINT);
                        }
                    }
                }
                match tokio::time::timeout(grace, &mut rx).await {
                    Ok(Ok(Err((-32800, _)))) => return Err(ExecError::SoftTimeout),
                    Ok(r) => r,
                    Err(_) => {
                        // Hard timeout: the script ignored/swallowed the interrupt or is stuck
                        // in native code holding the GIL. Kill; the pool respawns.
                        let _ = self.child.start_kill();
                        let _ = self.child.wait().await;
                        self.alive.store(false, Ordering::SeqCst);
                        return Err(ExecError::HardKilled);
                    }
                }
            }
        };
        match res {
            Ok(Ok(mut result)) => {
                let logs = std::mem::take(&mut *self.logs.lock().unwrap());
                result["logs"] = json!(logs);
                Ok(result)
            }
            Ok(Err((_, msg))) => Err(ExecError::Script(msg)),
            // Sender dropped by the reader task on EOF: the worker died mid-execution.
            Err(_) => {
                self.alive.store(false, Ordering::SeqCst);
                Err(ExecError::Crashed(self.exit_reason().await))
            }
        }
    }
}

struct PoolInner {
    cfg: Config,
    globals: Globals,
    idle: AMutex<VecDeque<Worker>>,
    available: Semaphore,
    respawned: Notify,
    respawn_times: Mutex<Vec<Duration>>,
    respawn_count: AtomicU64,
}

#[derive(Clone)]
pub struct Pool(Arc<PoolInner>);

impl Pool {
    pub async fn new(cfg: Config, globals: Globals, n: usize) -> Result<Pool, String> {
        let mut spawns = Vec::new();
        for _ in 0..n {
            spawns.push(Worker::spawn(&cfg, globals.clone()));
        }
        let mut idle = VecDeque::new();
        for s in spawns {
            idle.push_back(s.await?.0);
        }
        Ok(Pool(Arc::new(PoolInner {
            cfg,
            globals,
            idle: AMutex::new(idle),
            available: Semaphore::new(n),
            respawned: Notify::new(),
            respawn_times: Mutex::default(),
            respawn_count: AtomicU64::new(0),
        })))
    }

    pub async fn pids(&self) -> Vec<u32> {
        let mut v: Vec<u32> = self.0.idle.lock().await.iter().map(|w| w.pid).collect();
        v.sort();
        v
    }

    pub async fn execute(&self, code: &str, args: &Json, timeout: Duration) -> (Result<Json, ExecError>, Outcome) {
        let t0 = Instant::now();
        self.0.available.acquire().await.unwrap().forget();
        let mut w = self.0.idle.lock().await.pop_front().expect("permit implies an idle worker");
        let res = w.execute(code, args, timeout, self.0.cfg.grace).await;
        let out = Outcome { pid: w.pid, elapsed: t0.elapsed() };
        if w.alive() {
            self.0.idle.lock().await.push_back(w);
            self.0.available.add_permits(1);
        } else {
            drop(w); // kill_on_drop reaps whatever is left
            let pool = self.clone();
            tokio::spawn(async move {
                let t = Instant::now();
                match Worker::spawn(&pool.0.cfg, pool.0.globals.clone()).await {
                    Ok((nw, _)) => {
                        pool.0.idle.lock().await.push_back(nw);
                        pool.0.available.add_permits(1);
                        pool.0.respawn_times.lock().unwrap().push(t.elapsed());
                        pool.0.respawn_count.fetch_add(1, Ordering::SeqCst);
                        pool.0.respawned.notify_one();
                    }
                    Err(e) => eprintln!("respawn failed: {e}"),
                }
            });
        }
        (res, out)
    }

    /// Wait until a background respawn has finished (test helper).
    async fn wait_respawn(&self) -> bool {
        tokio::time::timeout(Duration::from_secs(10), self.0.respawned.notified()).await.is_ok()
    }

    fn last_respawn(&self) -> Duration {
        *self.0.respawn_times.lock().unwrap().last().unwrap_or(&Duration::ZERO)
    }
}

fn describe(r: &Result<Json, ExecError>) -> String {
    match r {
        Ok(v) => v.to_string(),
        Err(ExecError::Script(m)) => format!("Script(\"{m}\")"),
        Err(ExecError::Crashed(m)) => format!("Crashed({m})"),
        Err(e) => format!("{e:?}"),
    }
}

pub async fn run_checks(globals: Globals, r: &mut Report) {
    const A: &str = "Python";
    let python = std::env::var("S3_PYTHON").unwrap_or_else(|_| "python3".into());
    let worker = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("worker/den_worker.py");
    let cfg_with = |cancel| Config { cancel, python: python.clone(), worker: worker.clone(), max_mem_mb: 1024, grace: Duration::from_millis(300) };
    let cfg = || cfg_with(CancelMode::Protocol);
    let five_s = Duration::from_secs(5);

    // --- Cold start ------------------------------------------------------------------------------
    let mut bare = Vec::new();
    for _ in 0..20 {
        let t = Instant::now();
        let st = Command::new(&python).args(["-I", "-B", "-c", "pass"]).status().await;
        assert!(st.map(|s| s.success()).unwrap_or(false), "cannot run {python}");
        bare.push(t.elapsed());
    }
    r.add(A, "bare `python3 -I -B -c pass`", Status::Info, stats(bare).to_string());
    let mut cold = Vec::new();
    let mut version = String::new();
    for _ in 0..20 {
        let (w, d) = Worker::spawn(&cfg(), globals.clone()).await.expect("spawn");
        version = w.version.clone();
        cold.push(d);
    }
    r.check(A, "worker cold start (spawn -> ready)", true, format!("{} [python {version} via `{python}`]", stats(cold)));

    // stdin EOF -> worker exits (host crash leaves no orphans)
    let (mut w, _) = Worker::spawn(&cfg(), globals.clone()).await.unwrap();
    let t = Instant::now();
    *w.stdin.lock().await = None;
    let exited = tokio::time::timeout(Duration::from_secs(2), w.child.wait()).await;
    r.check(A, "worker exits when host closes stdin (no orphans)", matches!(exited, Ok(Ok(_))), format!("exited in {}", fd(t.elapsed())));

    let pool = Pool::new(cfg(), globals.clone(), 2).await.expect("pool");
    let rss: Vec<String> = pool.pids().await.iter().map(|p| format!("{:.1} MiB", rss_mib_of(&p.to_string()))).collect();
    r.add(A, "idle worker RSS", Status::Info, rss.join(", "));

    // --- Contract + bidirectional RPC ------------------------------------------------------------
    let (res, o) = pool
        .execute(
            "api.log('hello', args['user'])\n\
             n = (api.globals.get('py_counter') or 0) + 1\n\
             api.globals.set('py_counter', n)\n\
             print('printed line')\n\
             api.set_arg('counter', n)\n\
             api.set_arg('js_counter_seen', api.globals.get('js_counter'))\n",
            &json!({"user": "kris"}),
            five_s,
        )
        .await;
    let ok = matches!(&res, Ok(v) if v["ok"] == json!(true)
        && v["logs"] == json!(["hello kris", "printed line"])
        && v["setArgs"]["counter"] == json!(1))
        && globals.lock().unwrap().get("py_counter") == Some(&json!(1));
    r.check(A, "contract + bidirectional RPC (log/globals during execute)", ok, format!("{} in {}", describe(&res), fd(o.elapsed)));
    let (res, _) = pool.execute("def execute(args, api):\n    return False\n", &json!({}), five_s).await;
    r.check(A, "`execute()` returning False -> ok:false", matches!(&res, Ok(v) if v["ok"] == json!(false)), describe(&res));
    let (res, _) = pool.execute("raise ValueError('boom')", &json!({}), five_s).await;
    r.check(A, "exception -> error with message", matches!(&res, Err(ExecError::Script(m)) if m.contains("boom")), describe(&res));

    // --- Reuse after error/cancel: a partial print() line must not leak into the next run -------------
    // One worker, so the follow-up run is guaranteed to land on the same process.
    let solo = Pool::new(cfg(), globals.clone(), 1).await.expect("pool");
    let next = "print('new output')\n";
    let (err, _) = solo.execute("print('old output', end='')\nraise ValueError('boom')\n", &json!({}), five_s).await;
    let (after_err, _) = solo.execute(next, &json!({}), five_s).await;
    let (cancel, _) = solo.execute("print('stale', end='')\nwhile True: pass\n", &json!({}), Duration::from_millis(100)).await;
    let (after_cancel, _) = solo.execute(next, &json!({}), five_s).await;
    let clean = |v: &Result<Json, ExecError>| matches!(v, Ok(v) if v["logs"] == json!(["new output"]));
    r.check(
        A,
        "reuse after error/soft cancel: no partial print() leaks",
        matches!(&err, Err(ExecError::Script(_)))
            && matches!(&cancel, Err(ExecError::SoftTimeout))
            && clean(&after_err)
            && clean(&after_cancel),
        format!("after error: {} ; after cancel: {}", describe(&after_err), describe(&after_cancel)),
    );
    drop(solo); // keep later RSS/latency measurements free of an extra worker

    // --- Protocol hygiene: user code writing to fd 1 / reading stdin --------------------------------
    let (res, _) = pool
        .execute(
            "import os, sys\nprint('via print')\nos.write(1, b'Content-Length: 3\\r\\n\\r\\nbad')\n\
             sys.__stdout__.write('raw\\n'); sys.__stdout__.flush()\n\
             try:\n    input()\nexcept EOFError:\n    api.set_arg('stdin', 'eof')\n",
            &json!({}),
            five_s,
        )
        .await;
    let (res2, _) = pool.execute("api.set_arg('after', 1)", &json!({}), five_s).await;
    r.check(
        A,
        "stdout/stdin pollution cannot corrupt framing",
        matches!(&res, Ok(v) if v["setArgs"]["stdin"] == json!("eof")) && res2.is_ok(),
        format!("{} ; next: {}", describe(&res), describe(&res2)),
    );

    // --- Large payloads + stderr flood (pipe-deadlock check) -----------------------------------------
    let blob = "x".repeat(2 * 1024 * 1024);
    let (res, o) = pool
        .execute(
            "import sys\nsys.stderr.write('e' * 1_000_000); sys.stderr.flush()\napi.set_arg('echo', args['blob'])\n",
            &json!({"blob": blob}),
            five_s,
        )
        .await;
    let ok = matches!(&res, Ok(v) if v["setArgs"]["echo"].as_str().map(str::len) == Some(blob.len()));
    r.check(A, "2 MiB args in + 2 MiB setArgs out + 1 MB stderr", ok, format!("round trip {}", fd(o.elapsed)));

    // --- Warm round trips ------------------------------------------------------------------------
    for (name, code) in [
        ("warm round trip: trivial script", "api.set_arg('n', args['n'] + 1)\n"),
        (
            "warm round trip: 10 api calls (globals x8, log x2)",
            "for i in range(4):\n    api.globals.set('py_k%d' % i, i)\n\
             s = sum(api.globals.get('py_k%d' % i) for i in range(4))\n\
             api.log('sum', s)\napi.log('done')\napi.set_arg('sum', s)\n",
        ),
    ] {
        for _ in 0..100 {
            let _ = pool.execute(code, &json!({"n": 1}), five_s).await;
        }
        let mut v = Vec::new();
        let mut ok = true;
        for _ in 0..1000 {
            let (res, o) = pool.execute(code, &json!({"n": 1}), five_s).await;
            ok &= matches!(&res, Ok(v) if v["ok"] == json!(true));
            v.push(o.elapsed);
        }
        r.check(A, name, ok, stats(v).to_string());
    }

    // --- Concurrency: N=2 workers run in parallel --------------------------------------------------
    let empty = json!({});
    let t = Instant::now();
    let (a, b) = tokio::join!(
        pool.execute("import time\ntime.sleep(0.2)\n", &empty, five_s),
        pool.execute("import time\ntime.sleep(0.2)\n", &empty, five_s)
    );
    let wall = t.elapsed();
    r.check(
        A,
        "pool N=2: two 200ms scripts run concurrently",
        a.0.is_ok() && b.0.is_ok() && a.1.pid != b.1.pid && wall < Duration::from_millis(350),
        format!("wall {}", fd(wall)),
    );

    timeouts(&pool, "cancel=protocol", r).await;
    #[cfg(unix)]
    {
        let sig_pool = Pool::new(cfg_with(CancelMode::Signal), globals.clone(), 2).await.expect("pool");
        timeouts(&sig_pool, "cancel=SIGINT", r).await;
    }

    // --- Crash isolation ------------------------------------------------------------------------
    for (name, code) in [
        ("crash isolation: os._exit(1)", "import os\nos._exit(1)\n"),
        ("crash isolation: segfault via ctypes", "import ctypes\nctypes.string_at(0)\n"),
    ] {
        let (res, o) = pool.execute(code, &json!({}), five_s).await;
        let respawned = pool.wait_respawn().await;
        let (next, _) = pool.execute("api.set_arg('ok', True)", &json!({}), five_s).await;
        let n_live = pool.pids().await.len();
        r.check(
            A,
            name,
            matches!(res, Err(ExecError::Crashed(_))) && respawned && next.is_ok() && n_live == 2,
            format!("{} (detected in {}); respawn {}; pool size {n_live}", describe(&res), fd(o.elapsed), fd(pool.last_respawn())),
        );
    }

    // --- Memory bomb under RLIMIT_AS ---------------------------------------------------------------
    let pids = pool.pids().await;
    let (res, o) = pool.execute("l = []\nwhile True:\n    l.append(bytearray(50 * 1024 * 1024))\n", &json!({}), five_s).await;
    let (res2, _) = pool.execute("x = bytearray(8 * 1024 ** 3)\n", &json!({}), five_s).await;
    let kept = pool.pids().await == pids;
    r.check(
        A,
        "memory bomb (RLIMIT_AS 1 GiB) -> MemoryError, worker survives",
        matches!(&res, Err(ExecError::Script(m)) if m.starts_with("MemoryError"))
            && matches!(&res2, Err(ExecError::Script(m)) if m.starts_with("MemoryError"))
            && kept,
        format!("growing: {} in {}; 8 GiB alloc: {}; worker kept={kept}", describe(&res), fd(o.elapsed), describe(&res2)),
    );
    let respawns = pool.0.respawn_count.load(Ordering::SeqCst);
    r.add(A, "respawns during run", Status::Info, format!("{respawns} respawns in the cancel=protocol pool (hard kills + crashes)"));
}

async fn timeouts(pool: &Pool, mode: &str, r: &mut Report) {
    const A: &str = "Python";
    let five_s = Duration::from_secs(5);
    let timeout = Duration::from_millis(200);
    let pids0 = pool.pids().await;
    // --- Soft timeout -----------------------------------------------------------------------------
    for (name, code) in [
        ("soft timeout: `while True: pass`", "while True:\n    pass\n"),
        ("soft timeout: `time.sleep(30)` (blocking syscall)", "import time\ntime.sleep(30)\n"),
        ("soft timeout: blocked in api call loop", "while True:\n    api.globals.get('x')\n"),
    ] {
        let mut lat = Vec::new();
        let mut ok = true;
        for _ in 0..5 {
            let (res, o) = pool.execute(code, &json!({}), timeout).await;
            ok &= matches!(res, Err(ExecError::SoftTimeout));
            lat.push(o.elapsed.saturating_sub(timeout));
        }
        let survived = pool.pids().await == pids0;
        r.check(A, &format!("{name} [{mode}]"), ok && survived, format!("latency after 200ms deadline: {}; worker kept (no respawn)={survived}", stats(lat)));
    }

    // --- Hard timeout -----------------------------------------------------------------------------
    for (name, code) in [
        ("hard kill: script swallows KeyboardInterrupt", "while True:\n    try:\n        while True:\n            pass\n    except BaseException:\n        pass\n"),
        ("GIL-holding bigint pow (signal-aware C loop)", "x = 3 ** 600_000_000\n"),
        ("signal-deaf native call (pbkdf2, ~4s)", "import hashlib\nhashlib.pbkdf2_hmac('sha256', b'x', b'y', 40_000_000)\n"),
    ] {
        let (res, o) = pool.execute(code, &json!({}), timeout).await;
        let soft = matches!(res, Err(ExecError::SoftTimeout));
        let respawned = soft || pool.wait_respawn().await;
        let (next, _) = pool.execute("api.set_arg('ok', True)", &json!({}), five_s).await;
        let n_live = pool.pids().await.len();
        r.check(
            A,
            &format!("{name} [{mode}]"),
            matches!(res, Err(ExecError::HardKilled | ExecError::SoftTimeout)) && respawned && next.is_ok() && n_live == 2,
            format!(
                "{} after {} (deadline 200ms + grace 300ms); respawn {}; pool size {n_live}",
                describe(&res),
                fd(o.elapsed),
                if soft { "-".to_string() } else { fd(pool.last_respawn()) }
            ),
        );
    }
}
