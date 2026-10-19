//! Stand-in for `den-core`: a Tokio task that ticks once per second, logs to a
//! file and fans the tick out to webviews (Tauri events) and browsers (SSE).
//! It never depends on a window existing.

use std::io::Write;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::broadcast;

use crate::compat::{Decision, StateDir};

pub static START: OnceLock<Instant> = OnceLock::new();

pub fn uptime_ms() -> u64 {
    START.get().map(|s| s.elapsed().as_millis() as u64).unwrap_or(0)
}

#[derive(Debug, Clone, Serialize)]
pub struct Tick {
    pub n: u64,
    pub uptime_ms: u64,
    pub webviews: usize,
}

pub struct Shared {
    pub ticks: AtomicU64,
    pub decision: Decision,
    pub state: StateDir,
    pub no_first_paint_ack: bool,
    pub test_control: bool,
    /// Kill the WebKitWebProcess before destroying a window (see S-1 report).
    pub terminate_on_close: bool,
    pub first_paint_ms: Mutex<Option<f64>>,
    /// Set when a window is (re)created, consumed by its first-paint ack.
    pub pending_open: Mutex<Vec<(String, Instant)>>,
    pub http_addr: Mutex<Option<SocketAddr>>,
    pub tick_tx: broadcast::Sender<Tick>,
}

impl Shared {
    pub fn new(
        decision: Decision,
        state: StateDir,
        no_ack: bool,
        test_control: bool,
        terminate_on_close: bool,
    ) -> Self {
        let (tick_tx, _) = broadcast::channel(16);
        Self {
            ticks: AtomicU64::new(0),
            decision,
            state,
            no_first_paint_ack: no_ack,
            test_control,
            terminate_on_close,
            first_paint_ms: Mutex::new(None),
            pending_open: Mutex::new(Vec::new()),
            http_addr: Mutex::new(None),
            tick_tx,
        }
    }

    pub fn http_url(&self) -> Option<String> {
        self.http_addr
            .lock()
            .ok()
            .and_then(|a| a.map(|a| format!("http://{a}/")))
    }

    pub fn mark_opening(&self, label: &str) {
        if let Ok(mut p) = self.pending_open.lock() {
            p.retain(|(l, _)| l != label);
            p.push((label.to_string(), Instant::now()));
        }
    }

    pub fn take_opening(&self, label: &str) -> Option<Instant> {
        let mut p = self.pending_open.lock().ok()?;
        let idx = p.iter().position(|(l, _)| l == label)?;
        Some(p.remove(idx).1)
    }
}

pub fn spawn(app: AppHandle, shared: Arc<Shared>) {
    tauri::async_runtime::spawn(async move {
        let mut log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(shared.state.engine_log())
            .ok();
        let mut iv = tokio::time::interval(Duration::from_secs(1));
        iv.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            iv.tick().await;
            let n = shared.ticks.fetch_add(1, Ordering::Relaxed) + 1;
            let tick = Tick {
                n,
                uptime_ms: uptime_ms(),
                webviews: app.webview_windows().len(),
            };
            let _ = app.emit("engine-tick", &tick);
            let _ = shared.tick_tx.send(tick.clone());
            if let Some(f) = log.as_mut() {
                let _ = writeln!(
                    f,
                    "{} pid={} tick={} webviews={}",
                    crate::compat::now_ms(),
                    std::process::id(),
                    tick.n,
                    tick.webviews
                );
            }
        }
    });
}
