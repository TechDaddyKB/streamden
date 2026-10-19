//! Probe: time individual JS snippets with no deadline (used to explain a perf cliff).
use rquickjs::{Context, Runtime};
use std::time::Instant;

fn main() {
    let snippets: Vec<String> = std::env::args().skip(1).collect();
    for s in snippets {
        let rt = Runtime::new().unwrap();
        if let Ok(limit) = std::env::var("MEM_MB") {
            rt.set_memory_limit(limit.parse::<usize>().unwrap() * 1024 * 1024);
        }
        let ctx = Context::full(&rt).unwrap();
        let t = Instant::now();
        let r: rquickjs::Result<()> = ctx.with(|ctx| ctx.eval::<rquickjs::Value, _>(s.as_str()).map(|_| ()));
        println!("{:>10.1} ms  ok={}  {s}", t.elapsed().as_secs_f64() * 1e3, r.is_ok());
    }
}
