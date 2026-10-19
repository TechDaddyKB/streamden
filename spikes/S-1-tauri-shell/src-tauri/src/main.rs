//! StreamDen spike S-1 (DEN-2). Throwaway code; see docs/spikes/S-1.md.
//!
//! Flags:
//!   --compat-level N      force graphics-compat level 0..=5 for this run
//!   --reset-compat        forget the persisted level and the crash marker
//!   --state-dir PATH      where level file, marker and logs live
//!   --no-first-paint-ack  never clear the marker (simulates a blank window)
//!   --test-control        enable POST /api/test/{action} for scripted tests
//!   --no-open-browser     in browser-fallback mode, don't launch a browser
//!   --port N              local HTTP port (default 47811, falls back to any)
//!   --no-terminate-webprocess  plain destroy() on close (reproduces the leak)

mod compat;
mod engine;
mod server;
mod shell;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::json;
use tauri::{Manager, RunEvent, WindowEvent};

use crate::engine::{Shared, START};

struct Args {
    compat_level: Option<u8>,
    reset: bool,
    state_dir: PathBuf,
    no_ack: bool,
    test_control: bool,
    no_open_browser: bool,
    terminate_on_close: bool,
    port: u16,
}

fn parse_args() -> Args {
    let mut a = Args {
        compat_level: None,
        reset: false,
        state_dir: PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../.state")),
        no_ack: false,
        test_control: false,
        no_open_browser: false,
        terminate_on_close: true,
        port: 47811,
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--compat-level" => a.compat_level = it.next().and_then(|v| v.parse().ok()),
            "--reset-compat" => a.reset = true,
            "--state-dir" => {
                if let Some(v) = it.next() {
                    a.state_dir = PathBuf::from(v);
                }
            }
            "--no-first-paint-ack" => a.no_ack = true,
            "--test-control" => a.test_control = true,
            "--no-open-browser" => a.no_open_browser = true,
            "--no-terminate-webprocess" => a.terminate_on_close = false,
            "--port" => a.port = it.next().and_then(|v| v.parse().ok()).unwrap_or(a.port),
            other => eprintln!("[s1] ignoring unknown argument {other}"),
        }
    }
    a
}

fn main() {
    START.get_or_init(Instant::now);
    let args = parse_args();
    let state = compat::StateDir::new(args.state_dir.clone()).expect("cannot create state dir");

    // 1. Decide the level and set its env vars while we are still single-threaded
    //    and before GTK/WebKit read them.
    let decision = compat::decide(&state, args.compat_level, args.reset);
    compat::apply_env(&decision);
    eprintln!(
        "[s1] pid={} level={} ({}) source={:?} env={:?}",
        std::process::id(),
        decision.level,
        compat::level_name(decision.level),
        decision.source,
        decision.env_applied
    );

    let shared = Arc::new(Shared::new(
        decision.clone(),
        state.clone(),
        args.no_ack,
        args.test_control,
        args.terminate_on_close,
    ));

    // 2. Build the shell. The NVIDIA early-GL-context quirk must be the first plugin.
    let mut builder = tauri::Builder::default();
    if decision.quirk {
        builder = builder.plugin(tauri_plugin_wayland_nvidia_quirk::init());
    }
    let setup_shared = shared.clone();
    let port = args.port;
    let no_open_browser = args.no_open_browser;
    let app = builder
        .manage(shared.clone())
        .invoke_handler(tauri::generate_handler![
            shell::first_paint,
            shell::get_state,
            shell::open_chat_cmd,
            shell::hide_to_tray_cmd
        ])
        .setup(move |app| {
            let handle = app.handle().clone();
            shell::build_tray(&handle)?;
            engine::spawn(handle.clone(), setup_shared.clone());
            server::spawn(handle.clone(), setup_shared.clone(), port);
            if setup_shared.decision.browser_fallback() {
                setup_shared
                    .state
                    .event(json!({"event": "browser_fallback", "open_browser": !no_open_browser}));
                let h = handle.clone();
                let s = setup_shared.clone();
                tauri::async_runtime::spawn(async move {
                    for _ in 0..50 {
                        if s.http_url().is_some() {
                            break;
                        }
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                    eprintln!("[s1] browser fallback: UI at {:?}", s.http_url());
                    if !no_open_browser {
                        shell::open_in_browser(&h);
                    }
                });
            } else {
                shell::create_main(&handle)?;
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(move |app, event| match event {
        // Last window closed (code None): keep running in the tray.
        // Explicit app.exit(n) (tray Quit) has Some(code) and is allowed.
        RunEvent::ExitRequested { api, code, .. } => {
            if code.is_none() {
                api.prevent_exit();
            }
        }
        RunEvent::WindowEvent { label, event, .. } => match event {
            // Close button / compositor close: close to tray via our own path
            // so the web process is handled the same way as tray "Hide".
            WindowEvent::CloseRequested { api, .. } => {
                shared_event(app, json!({"event": "close_requested", "label": label}));
                if let Some(w) = app.get_webview_window(&label) {
                    api.prevent_close();
                    if let Err(e) = shell::close_webview_window(app, &w) {
                        eprintln!("[s1] close {label} failed: {e}");
                    }
                }
            }
            WindowEvent::Destroyed => {
                shared_event(app, json!({"event": "window_destroyed", "label": label}));
            }
            _ => {}
        },
        RunEvent::Exit => {
            shared_event(app, json!({"event": "exit"}));
        }
        _ => {}
    });
}

fn shared_event(app: &tauri::AppHandle, v: serde_json::Value) {
    app.state::<Arc<Shared>>().state.event(v);
}
