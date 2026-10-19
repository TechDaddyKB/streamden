//! Local HTTP server (loopback only): serves the same UI to a browser, exposes
//! engine state and an SSE tick stream, and (with `--test-control`) a few
//! window actions so the spike can be driven from a script.

use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::Arc;

use axum::extract::{Path, Query, Request, State};
use axum::http::{header, StatusCode};
use axum::middleware::{self, Next};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::json;
use tauri::AppHandle;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::{Stream, StreamExt};
use tower_http::services::ServeDir;

use crate::engine::Shared;

const DIST_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../dist");

#[derive(Clone)]
struct Ctx {
    app: AppHandle,
    shared: Arc<Shared>,
    port: u16,
}

pub fn spawn(app: AppHandle, shared: Arc<Shared>, port: u16) {
    tauri::async_runtime::spawn(async move {
        let listener = match tokio::net::TcpListener::bind(("127.0.0.1", port)).await {
            Ok(l) => l,
            Err(_) => match tokio::net::TcpListener::bind(("127.0.0.1", 0)).await {
                Ok(l) => l,
                Err(e) => {
                    eprintln!("[s1] http server bind failed: {e}");
                    return;
                }
            },
        };
        let Ok(addr) = listener.local_addr() else { return };
        if let Ok(mut a) = shared.http_addr.lock() {
            *a = Some(addr);
        }
        shared
            .state
            .event(json!({"event": "http_listening", "addr": addr.to_string()}));
        let ctx = Ctx {
            app,
            shared,
            port: addr.port(),
        };
        let router = Router::new()
            .route("/api/state", get(state))
            .route("/api/ticks", get(ticks))
            .route("/api/test/{action}", post(test_action))
            .fallback_service(ServeDir::new(DIST_DIR))
            .layer(middleware::from_fn_with_state(ctx.clone(), guard))
            .with_state(ctx);
        if let Err(e) = axum::serve(listener, router).await {
            eprintln!("[s1] http server stopped: {e}");
        }
    });
}

/// DNS-rebinding / cross-site guard: only our loopback Host, and for
/// state-changing requests only a same-origin (or absent) Origin.
async fn guard(State(ctx): State<Ctx>, req: Request, next: Next) -> Response {
    let allowed_hosts = [
        format!("127.0.0.1:{}", ctx.port),
        format!("localhost:{}", ctx.port),
    ];
    let host_ok = req
        .headers()
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .is_some_and(|h| allowed_hosts.iter().any(|a| a == h));
    let origin_ok = match req.headers().get(header::ORIGIN) {
        None => true,
        Some(o) => o
            .to_str()
            .ok()
            .is_some_and(|o| allowed_hosts.iter().any(|a| o == format!("http://{a}"))),
    };
    if !host_ok || (req.method() != axum::http::Method::GET && !origin_ok) {
        return (StatusCode::FORBIDDEN, "forbidden").into_response();
    }
    next.run(req).await
}

async fn state(State(ctx): State<Ctx>) -> Json<serde_json::Value> {
    Json(crate::shell::state_json(&ctx.shared))
}

async fn ticks(State(ctx): State<Ctx>) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let stream = BroadcastStream::new(ctx.shared.tick_tx.subscribe())
        .filter_map(|r| r.ok())
        .map(|t| Ok(Event::default().data(serde_json::to_string(&t).unwrap_or_default())));
    Sse::new(stream).keep_alive(KeepAlive::default())
}

async fn test_action(
    State(ctx): State<Ctx>,
    Path(action): Path<String>,
    Query(q): Query<HashMap<String, String>>,
) -> Response {
    if !ctx.shared.test_control {
        return (StatusCode::NOT_FOUND, "test control disabled").into_response();
    }
    let result = tokio::task::spawn_blocking(move || {
        crate::shell::test_action(&ctx.app, &ctx.shared, &action, &q)
    })
    .await;
    match result {
        Ok(Ok(v)) => Json(v).into_response(),
        Ok(Err(e)) => (StatusCode::BAD_REQUEST, Json(json!({"error": e}))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}
