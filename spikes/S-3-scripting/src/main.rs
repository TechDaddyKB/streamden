//! Spike S-3 (DEN-4): measure the StreamDen scripting design.
//! JS in-process via QuickJS-NG (rquickjs) and Python in pooled worker processes over
//! bidirectional JSON-RPC 2.0 with LSP-style framing. Throwaway code: numbers, not product.

mod js;
mod py;

use std::{
    collections::HashMap,
    fmt::Write as _,
    sync::{Arc, Mutex},
    time::Duration,
};

pub type Globals = Arc<Mutex<HashMap<String, serde_json::Value>>>;

#[derive(Clone, Copy, PartialEq)]
pub enum Status {
    Pass,
    Fail,
    /// A measured limitation of the design (not a broken check).
    Warn,
    Info,
}

pub struct Row {
    pub area: &'static str,
    pub check: String,
    pub status: Status,
    pub detail: String,
}

#[derive(Default)]
pub struct Report {
    pub rows: Vec<Row>,
}

impl Report {
    pub fn add(&mut self, area: &'static str, check: &str, status: Status, detail: String) {
        let tag = match status {
            Status::Pass => "PASS",
            Status::Fail => "FAIL",
            Status::Warn => "WARN",
            Status::Info => "INFO",
        };
        eprintln!("  [{tag}] {area}: {check} -- {detail}");
        self.rows.push(Row { area, check: check.to_string(), status, detail });
    }
    pub fn check(&mut self, area: &'static str, check: &str, ok: bool, detail: String) {
        self.add(area, check, if ok { Status::Pass } else { Status::Fail }, detail);
    }
}

pub struct Stats {
    pub n: usize,
    pub p50: Duration,
    pub p99: Duration,
    pub min: Duration,
    pub max: Duration,
    pub mean: Duration,
}

pub fn stats(mut v: Vec<Duration>) -> Stats {
    assert!(!v.is_empty());
    v.sort();
    let pct = |p: f64| v[(((v.len() - 1) as f64) * p).round() as usize];
    let total: Duration = v.iter().sum();
    Stats {
        n: v.len(),
        p50: pct(0.50),
        p99: pct(0.99),
        min: v[0],
        max: v[v.len() - 1],
        mean: total / v.len() as u32,
    }
}

impl std::fmt::Display for Stats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "p50 {} | p99 {} | min {} | max {} | mean {} (n={})",
            fd(self.p50),
            fd(self.p99),
            fd(self.min),
            fd(self.max),
            fd(self.mean),
            self.n
        )
    }
}

/// First line of an error message (QuickJS appends a stack trace).
pub fn first_line(s: &Option<String>) -> String {
    s.as_deref().map_or("-".into(), |e| e.lines().next().unwrap_or("").to_string())
}

/// Human duration: µs below 1 ms, ms otherwise.
pub fn fd(d: Duration) -> String {
    let us = d.as_secs_f64() * 1e6;
    if us < 1000.0 { format!("{us:.1}µs") } else { format!("{:.2}ms", us / 1000.0) }
}

/// Resident set size of this process in MiB (Linux only; 0 elsewhere).
pub fn rss_mib() -> f64 {
    rss_mib_of("self")
}

/// Resident set size of `/proc/<pid>` in MiB (Linux only; 0 elsewhere).
pub fn rss_mib_of(pid: &str) -> f64 {
    std::fs::read_to_string(format!("/proc/{pid}/status"))
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("VmRSS:"))
                .and_then(|l| l.split_whitespace().nth(1)?.parse::<f64>().ok())
        })
        .map_or(0.0, |kb| kb / 1024.0)
}

fn print_table(report: &Report) {
    let w_area = report.rows.iter().map(|r| r.area.len()).max().unwrap_or(4).max(4);
    let w_check = report.rows.iter().map(|r| r.check.chars().count()).max().unwrap_or(5).max(5);
    let mut out = String::new();
    let line = format!("+{}+{}+------+{}\n", "-".repeat(w_area + 2), "-".repeat(w_check + 2), "-".repeat(60));
    out.push_str(&line);
    let _ = writeln!(out, "| {:w_area$} | {:w_check$} | Res  | Numbers / detail", "Area", "Check");
    out.push_str(&line);
    for r in &report.rows {
        let tag = match r.status {
            Status::Pass => "PASS",
            Status::Fail => "FAIL",
            Status::Warn => "WARN",
            Status::Info => "info",
        };
        let pad = w_check - r.check.chars().count();
        let _ = writeln!(out, "| {:w_area$} | {}{} | {tag} | {}", r.area, r.check, " ".repeat(pad), r.detail);
    }
    out.push_str(&line);
    let fails = report.rows.iter().filter(|r| r.status == Status::Fail).count();
    let passes = report.rows.iter().filter(|r| r.status == Status::Pass).count();
    let warns = report.rows.iter().filter(|r| r.status == Status::Warn).count();
    let _ = writeln!(out, "Summary: {passes} PASS, {fails} FAIL, {warns} WARN (measured design limitations)");
    print!("{out}");
}

fn main() {
    let globals: Globals = Arc::new(Mutex::new(HashMap::new()));
    let mut report = Report::default();

    eprintln!("== JS (rquickjs / QuickJS-NG) on a dedicated thread ==");
    let g = globals.clone();
    let js_rows = std::thread::Builder::new()
        .name("js-host".into())
        // QuickJS checks its own stack limit (set_max_stack_size) against the thread's real
        // stack. The thread stack must be comfortably larger than that limit.
        .stack_size(16 * 1024 * 1024)
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
            let mut r = Report::default();
            rt.block_on(js::run_checks(g, &mut r));
            r.rows
        })
        .unwrap()
        .join()
        .expect("js thread panicked");
    report.rows.extend(js_rows);

    eprintln!("== Python worker pool ==");
    let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(4).enable_all().build().unwrap();
    rt.block_on(py::run_checks(globals, &mut report));

    println!();
    print_table(&report);
    if report.rows.iter().any(|r| r.status == Status::Fail) {
        std::process::exit(1);
    }
}
