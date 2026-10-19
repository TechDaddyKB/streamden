//! Graphics-compatibility ladder and crash-loop detection.
//!
//! Everything here runs at the very top of `main()`, before Tauri/GTK/WebKit
//! initialise and before any thread exists, because the env vars below are
//! only read once at GTK/WebKit start-up. They are set for *this process*
//! (and inherited by its WebKit child processes) only, never globally.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::json;

/// Highest level that still opens a webview window.
pub const MAX_WEBVIEW_LEVEL: u8 = 4;
/// Pseudo-level: no webview at all, UI only via the local HTTP server.
pub const BROWSER_LEVEL: u8 = 5;

pub fn level_name(level: u8) -> &'static str {
    match level {
        0 => "defaults",
        1 => "early-gl-quirk",
        2 => "no-dmabuf",
        3 => "no-dmabuf+no-compositing",
        4 => "x11",
        _ => "browser-fallback",
    }
}

/// Env vars a level sets. Level 1 sets none: it registers the early-GL-context
/// plugin instead (tauri-plugin-wayland-nvidia-quirk).
pub fn level_env(level: u8) -> &'static [(&'static str, &'static str)] {
    match level {
        2 => &[("WEBKIT_DISABLE_DMABUF_RENDERER", "1")],
        3 => &[
            ("WEBKIT_DISABLE_DMABUF_RENDERER", "1"),
            ("WEBKIT_DISABLE_COMPOSITING_MODE", "1"),
        ],
        4 => &[("GDK_BACKEND", "x11")],
        _ => &[],
    }
}

#[derive(Debug, Clone)]
pub struct Decision {
    pub level: u8,
    pub source: String,
    pub env_applied: Vec<(String, String)>,
    pub quirk: bool,
}

impl Decision {
    pub fn browser_fallback(&self) -> bool {
        self.level >= BROWSER_LEVEL
    }
}

#[derive(Debug, Clone)]
pub struct StateDir(pub PathBuf);

impl StateDir {
    pub fn new(path: PathBuf) -> std::io::Result<Self> {
        fs::create_dir_all(&path)?;
        Ok(Self(path))
    }
    pub fn level_file(&self) -> PathBuf {
        self.0.join("compat-level")
    }
    pub fn marker(&self) -> PathBuf {
        self.0.join("starting.json")
    }
    pub fn events(&self) -> PathBuf {
        self.0.join("events.jsonl")
    }
    pub fn engine_log(&self) -> PathBuf {
        self.0.join("engine.log")
    }

    /// Append one JSON line to `events.jsonl` (decisions, first paint, ...).
    pub fn event(&self, mut value: serde_json::Value) {
        use std::io::Write;
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        if let Some(obj) = value.as_object_mut() {
            obj.insert("t_unix_ms".into(), json!(now_ms()));
            obj.insert("pid".into(), json!(std::process::id()));
        }
        // One write_all per line under a lock: concurrent writeln! calls from
        // different threads interleave otherwise.
        let line = format!("{value}\n");
        let _guard = LOCK.lock();
        if let Ok(mut f) = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.events())
        {
            let _ = f.write_all(line.as_bytes());
        }
    }
}

pub fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or_default()
}

fn read_level(path: &Path) -> Option<u8> {
    fs::read_to_string(path).ok()?.trim().parse().ok()
}

fn pid_alive(pid: u32) -> bool {
    pid != std::process::id() && Path::new(&format!("/proc/{pid}")).exists()
}

/// Decide the level for this run.
///
/// Order: `--compat-level` CLI flag > crash-loop bump > persisted level file > 0.
/// A leftover `starting.json` means the previous run started a webview and never
/// reported first paint (killed, crashed, or hung blank), so we move one level up
/// and persist it. Past the last webview level we fall back to the browser UI.
pub fn decide(state: &StateDir, cli_level: Option<u8>, reset: bool) -> Decision {
    if reset {
        let _ = fs::remove_file(state.level_file());
        let _ = fs::remove_file(state.marker());
        state.event(json!({"event": "compat_reset"}));
    }
    let persisted = read_level(&state.level_file()).unwrap_or(0);
    let marker: Option<serde_json::Value> = fs::read_to_string(state.marker())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok());

    let (level, source) = if let Some(l) = cli_level {
        (l.min(BROWSER_LEVEL), format!("cli --compat-level {l}"))
    } else if let Some(m) = &marker {
        let prev_pid = m["pid"].as_u64().unwrap_or(0) as u32;
        let prev_level = m["level"].as_u64().map(|l| l as u8).unwrap_or(persisted);
        if pid_alive(prev_pid) {
            // Another instance is still starting; not a crash.
            (persisted, format!("level file (pid {prev_pid} still running)"))
        } else {
            let next = (prev_level + 1).min(BROWSER_LEVEL);
            let _ = fs::write(state.level_file(), next.to_string());
            state.event(json!({
                "event": "crash_loop_bump",
                "from": prev_level,
                "to": next,
                "previous_pid": prev_pid,
            }));
            (
                next,
                format!("crash-loop: previous run (pid {prev_pid}, level {prev_level}) never painted"),
            )
        }
    } else if persisted > 0 {
        (persisted, "level file".to_string())
    } else {
        (0, "default".to_string())
    };

    let env_applied = level_env(level)
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    let decision = Decision {
        level,
        source,
        env_applied,
        quirk: (1..=MAX_WEBVIEW_LEVEL).contains(&level),
    };

    if decision.browser_fallback() {
        // No webview will be created, so no first paint is expected.
        let _ = fs::remove_file(state.marker());
    } else {
        let _ = fs::write(
            state.marker(),
            json!({"pid": std::process::id(), "level": level, "started_unix_ms": now_ms()})
                .to_string(),
        );
    }
    state.event(json!({
        "event": "startup",
        "level": level,
        "level_name": level_name(level),
        "source": decision.source,
        "env": decision.env_applied,
    }));
    decision
}

/// Apply the level's env vars to this process only.
///
/// Must be called before any other thread is spawned and before GTK init:
/// `set_var` is not thread-safe (it is `unsafe` in edition 2024).
pub fn apply_env(decision: &Decision) {
    for (k, v) in &decision.env_applied {
        std::env::set_var(k, v);
    }
}

/// Clear the "starting" marker once the main webview has painted.
pub fn clear_marker(state: &StateDir) {
    let _ = fs::remove_file(state.marker());
}
