//! The thin Tauri shell: windows, tray, IPC commands.

use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use serde_json::{json, Value};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{
    AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};

use crate::compat::{self, level_name};
use crate::engine::{Shared, START};

const SESSION_VARS: &[&str] = &[
    "XDG_SESSION_TYPE",
    "WAYLAND_DISPLAY",
    "DISPLAY",
    "GDK_BACKEND",
    "GDK_SCALE",
    "GDK_GL",
    "WEBKIT_DISABLE_DMABUF_RENDERER",
    "WEBKIT_DISABLE_COMPOSITING_MODE",
    "__NV_DISABLE_EXPLICIT_SYNC",
];

fn process_ms() -> f64 {
    START
        .get()
        .map(|s| s.elapsed().as_secs_f64() * 1000.0)
        .unwrap_or(0.0)
}

pub fn quirk_status(shared: &Shared) -> String {
    if shared.decision.quirk {
        format!("{:?}", tauri_plugin_wayland_nvidia_quirk::status())
    } else {
        "not registered".to_string()
    }
}

pub fn state_json(shared: &Shared) -> Value {
    let session: HashMap<&str, Option<String>> = SESSION_VARS
        .iter()
        .map(|k| (*k, std::env::var(k).ok()))
        .collect();
    json!({
        "ticks": shared.ticks.load(Ordering::Relaxed),
        "uptime_ms": crate::engine::uptime_ms(),
        "level": shared.decision.level,
        "level_name": level_name(shared.decision.level),
        "level_source": shared.decision.source,
        "browser_fallback": shared.decision.browser_fallback(),
        "env_applied": shared.decision.env_applied,
        "quirk_registered": shared.decision.quirk,
        "quirk_status": quirk_status(shared),
        "session": session,
        "first_paint_ms": shared.first_paint_ms.lock().ok().and_then(|g| *g),
        "http_url": shared.http_url(),
        "pid": std::process::id(),
    })
}

fn shared_of(app: &AppHandle) -> Arc<Shared> {
    app.state::<Arc<Shared>>().inner().clone()
}

pub fn create_main(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    let shared = shared_of(app);
    shared.mark_opening("main");
    shared.state.event(json!({"event": "window_create", "label": "main"}));
    WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
        .title(format!(
            "StreamDen S-1 · level {} ({})",
            shared.decision.level,
            level_name(shared.decision.level)
        ))
        .inner_size(1000.0, 700.0)
        .min_inner_size(480.0, 360.0)
        .resizable(true)
        .build()
}

pub fn show_main(app: &AppHandle) -> tauri::Result<()> {
    if let Some(w) = app.get_webview_window("main") {
        w.show()?;
        w.set_focus()?;
        return Ok(());
    }
    create_main(app).map(|_| ())
}

/// Close to tray: destroy the webview (WebKitWebProcess goes away), keep the
/// engine running. Recreated on demand by `show_main`.
pub fn hide_to_tray(app: &AppHandle) -> tauri::Result<()> {
    if let Some(w) = app.get_webview_window("main") {
        shared_of(app)
            .state
            .event(json!({"event": "hide_to_tray", "label": "main"}));
        close_webview_window(app, &w)?;
    }
    Ok(())
}

/// Destroy a webview window. On Linux, `destroy()` alone leaves the window's
/// WebKitWebProcess running until the app exits (measured in S-1), so by
/// default we terminate it first. `--no-terminate-webprocess` shows the leak.
pub fn close_webview_window(app: &AppHandle, w: &WebviewWindow) -> tauri::Result<()> {
    let shared = shared_of(app);
    #[cfg(target_os = "linux")]
    if shared.terminate_on_close {
        w.with_webview(|wv| {
            use webkit2gtk::WebViewExt;
            wv.inner().terminate_web_process();
        })?;
    }
    shared.state.event(json!({
        "event": "close_webview",
        "label": w.label(),
        "terminate_web_process": shared.terminate_on_close,
    }));
    w.destroy()
}

pub fn open_chat(app: &AppHandle) -> tauri::Result<()> {
    if let Some(w) = app.get_webview_window("chat") {
        w.show()?;
        return Ok(());
    }
    let shared = shared_of(app);
    shared.mark_opening("chat");
    shared.state.event(json!({"event": "window_create", "label": "chat"}));
    WebviewWindowBuilder::new(app, "chat", WebviewUrl::App("chat.html".into()))
        .title("StreamDen S-1 · chat")
        .inner_size(360.0, 560.0)
        .always_on_top(true)
        .resizable(true)
        .build()?;
    Ok(())
}

pub fn open_in_browser(app: &AppHandle) {
    if let Some(url) = shared_of(app).http_url() {
        let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    }
}

pub fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Show", true, None::<&str>)?;
    let hide = MenuItem::with_id(app, "hide", "Hide to tray (destroy webview)", true, None::<&str>)?;
    let chat = MenuItem::with_id(app, "chat", "Second window (chat)", true, None::<&str>)?;
    let browser = MenuItem::with_id(app, "browser", "Open UI in browser", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&show, &hide, &chat, &sep, &browser, &quit])?;

    let mut tray = TrayIconBuilder::with_id("s1-tray")
        .tooltip("StreamDen S-1")
        .title("StreamDen S-1")
        .menu(&menu)
        .on_menu_event(|app, event| {
            let id = event.id().as_ref().to_string();
            shared_of(app)
                .state
                .event(json!({"event": "tray_menu", "id": id}));
            let res = match id.as_str() {
                "show" => show_main(app),
                "hide" => hide_to_tray(app),
                "chat" => open_chat(app),
                "browser" => {
                    open_in_browser(app);
                    Ok(())
                }
                "quit" => {
                    app.exit(0);
                    Ok(())
                }
                _ => Ok(()),
            };
            if let Err(e) = res {
                eprintln!("[s1] tray action {id} failed: {e}");
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}

#[tauri::command]
pub fn first_paint(window: WebviewWindow, shared: tauri::State<'_, Arc<Shared>>, webview_ms: f64) {
    let label = window.label().to_string();
    let since_open_ms = shared
        .take_opening(&label)
        .map(|t| t.elapsed().as_secs_f64() * 1000.0);
    let now = process_ms();
    let mut first_in_process = false;
    if label == "main" {
        if let Ok(mut fp) = shared.first_paint_ms.lock() {
            if fp.is_none() {
                *fp = Some(now);
                first_in_process = true;
            }
        }
        if first_in_process {
            if shared.no_first_paint_ack {
                shared
                    .state
                    .event(json!({"event": "first_paint_ack_suppressed"}));
            } else {
                compat::clear_marker(&shared.state);
            }
        }
    }
    shared.state.event(json!({
        "event": "first_paint",
        "label": label,
        "level": shared.decision.level,
        "first_in_process": first_in_process,
        "process_ms": now,
        "since_open_ms": since_open_ms,
        "webview_ms": webview_ms,
    }));
}

#[tauri::command]
pub fn get_state(shared: tauri::State<'_, Arc<Shared>>) -> Value {
    state_json(&shared)
}

#[tauri::command]
pub async fn open_chat_cmd(app: AppHandle) -> Result<(), String> {
    open_chat(&app).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn hide_to_tray_cmd(app: AppHandle) -> Result<(), String> {
    hide_to_tray(&app).map_err(|e| e.to_string())
}

fn window_info(w: &WebviewWindow) -> Value {
    json!({
        "label": w.label(),
        "inner_size": w.inner_size().ok().map(|s| [s.width, s.height]),
        "outer_position": w.outer_position().ok().map(|p| [p.x, p.y]),
        "scale_factor": w.scale_factor().ok(),
        "fullscreen": w.is_fullscreen().ok(),
        "visible": w.is_visible().ok(),
        "always_on_top": w.is_always_on_top().ok(),
    })
}

fn num(q: &HashMap<String, String>, k: &str) -> Result<f64, String> {
    q.get(k)
        .ok_or(format!("missing {k}"))?
        .parse()
        .map_err(|e| format!("{k}: {e}"))
}

fn main_window(app: &AppHandle) -> Result<WebviewWindow, String> {
    app.get_webview_window("main")
        .ok_or("no main window".to_string())
}

/// Script-driven actions (only with `--test-control`).
pub fn test_action(
    app: &AppHandle,
    shared: &Shared,
    action: &str,
    q: &HashMap<String, String>,
) -> Result<Value, String> {
    let e = |r: tauri::Result<()>| r.map_err(|e| e.to_string());
    shared
        .state
        .event(json!({"event": "test_action", "action": action, "query": q}));
    match action {
        "info" => {}
        "show" => e(show_main(app))?,
        "hide" => e(hide_to_tray(app))?,
        "chat" => e(open_chat(app))?,
        "chat-close" => {
            if let Some(w) = app.get_webview_window("chat") {
                e(close_webview_window(app, &w))?;
            }
        }
        "fullscreen" => {
            let on = q.get("on").map(|v| v == "1").unwrap_or(true);
            e(main_window(app)?.set_fullscreen(on))?;
        }
        "resize" => {
            let size = LogicalSize::new(num(q, "w")?, num(q, "h")?);
            e(main_window(app)?.set_size(size))?;
        }
        "move" => {
            let pos = LogicalPosition::new(num(q, "x")?, num(q, "y")?);
            e(main_window(app)?.set_position(pos))?;
        }
        "animation" => {
            let on = q.get("on").map(|v| v == "1").unwrap_or(true);
            e(app.emit_to("main", "set-animation", on))?;
        }
        "quit" => app.exit(0),
        other => return Err(format!("unknown action {other}")),
    }
    let windows: Vec<Value> = app.webview_windows().values().map(window_info).collect();
    Ok(json!({"ok": true, "action": action, "windows": windows}))
}
