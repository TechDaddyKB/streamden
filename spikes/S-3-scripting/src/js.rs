//! In-process JS host on rquickjs (QuickJS-NG) + its checks.
//!
//! One `AsyncRuntime` + one `AsyncContext` per host ("script context"). Built without the
//! `parallel` feature, so the runtime is `!Send` and lives on a dedicated OS thread that runs a
//! current-thread Tokio runtime (see main.rs). That keeps CPU-bound JS off the shared runtime.

use std::{
    cell::RefCell,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use rquickjs::{
    AsyncContext, AsyncRuntime, CatchResultExt, Ctx, Function, Object, Persistent, Promise, Value,
    prelude::{Async, Coerced, Func},
};
use serde_json::{Map, Value as Json, json};

use crate::{Globals, Report, Status, fd, first_line, rss_mib, stats};

const NO_DEADLINE: u64 = u64::MAX;
const MIB: usize = 1024 * 1024;

#[derive(Default)]
struct ExecState {
    logs: Vec<String>,
    set_args: Map<String, Json>,
}

/// The common contract: `{ ok, setArgs, logs }` (+ error for the host).
pub struct Outcome {
    pub ok: bool,
    pub set_args: Map<String, Json>,
    pub logs: Vec<String>,
    pub error: Option<String>,
    pub elapsed: Duration,
}

impl Outcome {
    fn contract(&self) -> Json {
        json!({ "ok": self.ok, "setArgs": self.set_args, "logs": self.logs })
    }
}

pub struct JsHost {
    // Field order is drop order and it matters: a `Persistent` that outlives its runtime makes
    // JS_FreeRuntime hit `assert(list_empty(&rt->gc_obj_list))` and abort the whole process.
    func: Option<Persistent<Function<'static>>>,
    state: Rc<RefCell<ExecState>>,
    ctx: AsyncContext,
    rt: AsyncRuntime,
    epoch: Instant,
    /// Nanoseconds since `epoch` after which the interrupt handler aborts JS.
    deadline: Arc<AtomicU64>,
}

impl JsHost {
    pub async fn new(globals: Globals, mem_limit: usize) -> rquickjs::Result<Self> {
        let rt = AsyncRuntime::new()?;
        rt.set_memory_limit(mem_limit).await;
        rt.set_max_stack_size(512 * 1024).await;
        let epoch = Instant::now();
        let deadline = Arc::new(AtomicU64::new(NO_DEADLINE));
        let d = deadline.clone();
        // Called by QuickJS every ~10k bytecode ops (JS_INTERRUPT_COUNTER_INIT) and from the
        // regex engine. Returning true raises an *uncatchable* exception.
        rt.set_interrupt_handler(Some(Box::new(move || {
            epoch.elapsed().as_nanos() as u64 >= d.load(Ordering::Relaxed)
        })))
        .await;
        let ctx = AsyncContext::full(&rt).await?;
        let state = Rc::new(RefCell::new(ExecState::default()));
        let st = state.clone();
        ctx.with(|ctx| install_api(&ctx, st, globals)).await?;
        Ok(Self { func: None, state, ctx, rt, epoch, deadline })
    }

    /// Wrap user code as the body of `async function (args)` and cache it.
    pub async fn compile(&mut self, code: &str) -> Result<(), String> {
        let src = format!("(async function (args) {{\n{code}\n}})");
        let f = self
            .ctx
            .with(|ctx| {
                let f: Function = ctx.eval(src).catch(&ctx).map_err(|e| e.to_string())?;
                Ok::<_, String>(Persistent::save(&ctx, f))
            })
            .await?;
        self.func = Some(f);
        Ok(())
    }

    pub async fn call(&self, args: &Json, timeout: Duration) -> Outcome {
        let start = Instant::now();
        *self.state.borrow_mut() = ExecState::default();
        let func = self.func.clone().expect("compile() first");
        let args_s = args.to_string();
        let dl = (start + timeout - self.epoch).as_nanos() as u64;
        self.deadline.store(dl, Ordering::Relaxed);
        let fut = self.ctx.async_with(async move |ctx| {
            let f = func.restore(&ctx).map_err(|e| e.to_string())?;
            let a = ctx.json_parse(args_s).catch(&ctx).map_err(|e| e.to_string())?;
            let p: Promise = f.call((a,)).catch(&ctx).map_err(|e| e.to_string())?;
            let v: Value = p.into_future::<Value>().await.catch(&ctx).map_err(|e| e.to_string())?;
            Ok::<bool, String>(v.as_bool() != Some(false))
        });
        // The interrupt handler only runs while bytecode executes. Time spent awaiting a host
        // future (api.http.fetch) is invisible to it, so the host must also bound the await.
        let res = tokio::time::timeout(timeout, fut).await;
        self.deadline.store(NO_DEADLINE, Ordering::Relaxed);
        let elapsed = start.elapsed();
        let st = std::mem::take(&mut *self.state.borrow_mut());
        let (ok, error) = match res {
            Ok(Ok(ok)) => (ok, None),
            Ok(Err(e)) => (false, Some(e)),
            Err(_) => (false, Some("deadline exceeded while awaiting a host future".into())),
        };
        Outcome { ok, set_args: st.set_args, logs: st.logs, error, elapsed }
    }

    pub async fn malloc_size(&self) -> i64 {
        self.rt.run_gc().await;
        self.rt.memory_usage().await.malloc_size
    }
}

fn to_json<'js>(ctx: &Ctx<'js>, v: Value<'js>) -> rquickjs::Result<Json> {
    Ok(match ctx.json_stringify(v)? {
        Some(s) => serde_json::from_str(&s.to_string()?).unwrap_or(Json::Null),
        None => Json::Null, // undefined / function
    })
}

fn install_api<'js>(ctx: &Ctx<'js>, st: Rc<RefCell<ExecState>>, globals: Globals) -> rquickjs::Result<()> {
    let api = Object::new(ctx.clone())?;

    let s = st.clone();
    api.set("log", Func::from(move |msg: Coerced<String>| s.borrow_mut().logs.push(msg.0)))?;

    api.set(
        "setArg",
        Func::from(move |ctx: Ctx<'js>, name: String, v: Value<'js>| -> rquickjs::Result<()> {
            let j = to_json(&ctx, v)?;
            st.borrow_mut().set_args.insert(name, j);
            Ok(())
        }),
    )?;

    let g = Object::new(ctx.clone())?;
    let gl = globals.clone();
    g.set(
        "get",
        Func::from(move |ctx: Ctx<'js>, name: String| -> rquickjs::Result<Value<'js>> {
            let v = gl.lock().unwrap().get(&name).cloned();
            match v {
                Some(v) => ctx.json_parse(v.to_string()),
                None => Ok(Value::new_undefined(ctx)),
            }
        }),
    )?;
    g.set(
        "set",
        Func::from(move |ctx: Ctx<'js>, name: String, v: Value<'js>| -> rquickjs::Result<()> {
            let j = to_json(&ctx, v)?;
            globals.lock().unwrap().insert(name, j);
            Ok(())
        }),
    )?;
    api.set("globals", g)?;

    // Async host function: returns a Promise; the Rust future is driven by the rquickjs
    // scheduler while `async_with` polls. `slow://<ms>` lets tests control the delay.
    let http = Object::new(ctx.clone())?;
    let fetch = Function::new(
        ctx.clone(),
        Async(move |ctx: Ctx<'js>, url: String| async move {
            let ms = url.strip_prefix("slow://").and_then(|s| s.parse().ok()).unwrap_or(5);
            tokio::time::sleep(Duration::from_millis(ms)).await;
            ctx.json_parse(json!({ "status": 200, "url": url, "body": { "hello": "world" } }).to_string())
        }),
    )?;
    http.set("fetch", fetch)?;
    api.set("http", http)?;

    ctx.globals().set("api", api)?;
    Ok(())
}

const TRIVIAL: &str = "api.setArg('n', args.n + 1); return true;";
const TEN_CALLS: &str = "for (let i = 0; i < 4; i++) api.globals.set('js_k' + i, i);
let s = 0; for (let i = 0; i < 4; i++) s += api.globals.get('js_k' + i);
api.log('sum ' + s); api.log('done'); api.setArg('sum', s); return true;";

async fn host(globals: &Globals, mem: usize, code: &str) -> JsHost {
    let mut h = JsHost::new(globals.clone(), mem).await.expect("host");
    h.compile(code).await.expect("compile");
    h
}

pub async fn run_checks(globals: Globals, r: &mut Report) {
    const A: &str = "JS";
    let five_s = Duration::from_secs(5);
    let version = unsafe { std::ffi::CStr::from_ptr(rquickjs::qjs::JS_GetVersion()) }
        .to_string_lossy()
        .into_owned();
    r.add(A, "engine", Status::Info, format!("rquickjs 0.14.0, QuickJS-NG {version}"));

    // --- Contract + host functions --------------------------------------------------------
    let mut h = host(
        &globals,
        64 * MIB,
        "api.log('hello ' + args.user);
         const n = (api.globals.get('js_counter') ?? 0) + 1;
         api.globals.set('js_counter', n);
         const r = await api.http.fetch('https://example.invalid/x');
         api.setArg('counter', n); api.setArg('fetched', r.body.hello);
         return true;",
    )
    .await;
    let o = h.call(&json!({"user": "kris"}), five_s).await;
    let ok = o.ok
        && o.error.is_none()
        && o.logs == ["hello kris"]
        && o.set_args.get("counter") == Some(&json!(1))
        && o.set_args.get("fetched") == Some(&json!("world"))
        && globals.lock().unwrap().get("js_counter") == Some(&json!(1));
    r.check(A, "contract args->{ok,setArgs,logs} + log/globals/await fetch", ok, format!("{} in {}", o.contract(), fd(o.elapsed)));

    h.compile("return false;").await.unwrap();
    let o = h.call(&json!({}), five_s).await;
    r.check(A, "`return false` -> ok:false (stop action)", !o.ok && o.error.is_none(), o.contract().to_string());
    h.compile("throw new TypeError('boom')").await.unwrap();
    let o = h.call(&json!({}), five_s).await;
    r.check(A, "thrown error -> ok:false + message", !o.ok && o.error.as_deref().unwrap_or("").contains("boom"), format!("error=\"{}\"", first_line(&o.error)));

    // --- Deadline interrupt -----------------------------------------------------------------
    let deadline = Duration::from_millis(100);
    let cases = [
        ("deadline kills while(true){}", "while (true) {}"),
        ("deadline kills catch-swallowing loop", "for (;;) { try { while (true) {} } catch (e) {} } api.log('escaped');"),
        ("deadline kills ReDoS regex", "/(a+)+$/.test('a'.repeat(40) + '!'); api.log('escaped');"),
    ];
    for (name, code) in cases {
        h.compile(code).await.unwrap();
        let mut lat = Vec::new();
        let mut all_killed = true;
        let mut err = String::new();
        for _ in 0..20 {
            let o = h.call(&json!({}), deadline).await;
            all_killed &= o.error.is_some() && o.logs.is_empty();
            err = first_line(&o.error);
            lat.push(o.elapsed.saturating_sub(deadline));
        }
        let s = stats(lat);
        let ok = all_killed && s.max < Duration::from_millis(20);
        r.check(A, name, ok, format!("kill latency after 100ms deadline: {s}; error=\"{err}\""));
    }
    h.compile(TRIVIAL).await.unwrap();
    let o = h.call(&json!({"n": 1}), five_s).await;
    r.check(A, "context reusable after interrupts", o.ok && o.set_args.get("n") == Some(&json!(2)), o.contract().to_string());

    // QuickJS polls the interrupt handler every 10,000 *ticks* (calls + backward jumps), not by
    // time. A loop whose body is one slow native call is only checked every few thousand
    // iterations, so the deadline overshoots by (ticks/iteration)^-1 * 10,000 * cost(call).
    for (name, code) in [
        (
            "native-heavy loop: indexOf on 256 KiB string",
            "const s = 'x'.repeat(1 << 18); for (;;) s.indexOf('y');",
        ),
        (
            "native-heavy loop: sort+reverse 2e4 numbers",
            "const a = Array.from({ length: 2e4 }, (_, i) => (i * 7919) % 2e4); for (;;) { a.sort(); a.reverse(); }",
        ),
    ] {
        let big = host(&globals, 256 * MIB, code).await;
        let o = big.call(&json!({}), deadline).await;
        let killed = o.error.as_deref().unwrap_or("").contains("interrupted");
        r.add(
            A,
            name,
            if killed { Status::Warn } else { Status::Fail },
            format!("100ms deadline overshot by {} (interrupted={killed}). LIMITATION: deadline is not a hard bound in-process", fd(o.elapsed.saturating_sub(deadline))),
        );
    }

    // --- Deadline while awaiting a host future ------------------------------------------------
    let mut h2 = host(
        &globals,
        64 * MIB,
        "await api.http.fetch('slow://300'); api.log('zombie resumed'); api.setArg('zombie', true); return true;",
    )
    .await;
    let o = h2.call(&json!({}), deadline).await;
    r.check(
        A,
        "deadline enforced during `await` (host timeout)",
        !o.ok && o.elapsed < deadline + Duration::from_millis(20),
        format!("returned after {}; error=\"{}\"", fd(o.elapsed), first_line(&o.error)),
    );
    tokio::time::sleep(Duration::from_millis(400)).await; // the abandoned fetch completes meanwhile
    h2.compile("await api.http.fetch('https://x'); api.log('second script'); return true;").await.unwrap();
    let o = h2.call(&json!({}), five_s).await;
    let leaked = o.logs.iter().any(|l| l.contains("zombie")) || o.set_args.contains_key("zombie");
    r.add(
        A,
        "abandoned await resumes inside the NEXT call",
        if leaked { Status::Warn } else { Status::Pass },
        format!("leaked={leaked}; next call result {} -> recreate the context after an await timeout", o.contract()),
    );
    drop(h2);

    // --- Memory limit ---------------------------------------------------------------------------
    let mut m = host(&globals, 64 * MIB, TRIVIAL).await;
    let base = m.malloc_size().await;
    m.compile("const chunks = []; while (true) chunks.push(new Array(1e6).fill(1.5));").await.unwrap();
    let o = m.call(&json!({}), five_s).await;
    let oom1 = first_line(&o.error);
    let t1 = o.elapsed;
    m.compile("const b = new ArrayBuffer(1024 * 1024 * 1024); return b.byteLength > 0;").await.unwrap();
    let o2 = m.call(&json!({}), five_s).await;
    m.compile(TRIVIAL).await.unwrap();
    let after = m.call(&json!({"n": 41}), five_s).await;
    let post = m.malloc_size().await;
    let ok = oom1.contains("out of memory") && o2.error.is_some() && after.ok && after.set_args.get("n") == Some(&json!(42));
    r.check(
        A,
        "64 MiB memory limit -> clean error, engine survives",
        ok,
        format!(
            "growing array: \"{oom1}\" after {}; 1 GiB ArrayBuffer: \"{}\"; next call ok={}; malloc before {:.1} MiB / after GC {:.1} MiB",
            fd(t1),
            first_line(&o2.error),
            after.ok,
            base as f64 / MIB as f64,
            post as f64 / MIB as f64
        ),
    );
    m.compile("function f(n) { return f(n + 1) + 1; } f(0);").await.unwrap();
    let o = m.call(&json!({}), five_s).await;
    r.check(A, "stack limit (512 KiB) -> clean error", o.error.is_some(), format!("error=\"{}\" after {}", first_line(&o.error), fd(o.elapsed)));
    drop(m);

    // --- Cold start ---------------------------------------------------------------------------
    let n = 1000;
    let rss0 = rss_mib();
    let (mut create, mut full) = (Vec::with_capacity(n), Vec::with_capacity(n));
    let mut all_ok = true;
    let mut ctx_bytes = 0;
    for i in 0..n {
        let t = Instant::now();
        let mut h = JsHost::new(globals.clone(), 64 * MIB).await.unwrap();
        create.push(t.elapsed());
        h.compile(TRIVIAL).await.unwrap();
        let o = h.call(&json!({"n": 1}), five_s).await;
        full.push(t.elapsed());
        all_ok &= o.ok;
        if i == 0 {
            ctx_bytes = h.rt.memory_usage().await.malloc_size;
        }
    }
    let rss1 = rss_mib();
    r.add(A, "cold: new Runtime+Context+api", Status::Info, stats(create).to_string());
    r.check(A, "cold start: Runtime+Context+api+eval+call", all_ok, stats(full).to_string());
    r.add(
        A,
        "memory per fresh runtime+context",
        Status::Info,
        format!("QuickJS malloc_size {:.0} KiB; process RSS {:.1} -> {:.1} MiB over {n} cold starts", ctx_bytes as f64 / 1024.0, rss0, rss1),
    );

    // --- Warm calls ---------------------------------------------------------------------------
    for (name, code, iters) in [
        ("warm call: trivial script (cached ctx)", TRIVIAL, 2000),
        ("warm call: 10 api calls (globals x8, log x2)", TEN_CALLS, 2000),
        ("warm call: one `await api.http.fetch` (5ms stub)", "const r = await api.http.fetch('https://x'); api.setArg('s', r.status); return true;", 300),
    ] {
        let h = host(&globals, 64 * MIB, code).await;
        for _ in 0..100 {
            h.call(&json!({"n": 1}), five_s).await;
        }
        let mut v = Vec::with_capacity(iters);
        let mut ok = true;
        for _ in 0..iters {
            let o = h.call(&json!({"n": 1}), five_s).await;
            ok &= o.ok && o.error.is_none();
            v.push(o.elapsed);
        }
        r.check(A, name, ok, stats(v).to_string());
    }
}
