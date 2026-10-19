//! Spike S-2 (DEN-3): a tiny loopback server for OBS browser sources and docks.
//!
//! Throwaway code. Its only purpose is to produce evidence for `docs/spikes/S-2.md`.
//! It serves:
//! - `http://127.0.0.1:<port>/o/alerts`   transparent overlay fed by a WebSocket
//! - `http://127.0.0.1:<port>/o/widgets`  overlay that iframes per-widget origins
//! - `http://127.0.0.1:<port>/dock/chat`  chat dock page
//! - `http://w-<id>.localhost:<port>/`    per-widget origin probe (Host-header routing)
//! - `http://w-<id>.localhost:<port>/sew/se-sample/`  StreamElements-style sample widget
//! - `/report` (GET = machine-readable log, POST = client reports), `/emit`, `/api/*`, `/probe/*`, `/ws`

use std::{
    collections::{HashMap, VecDeque},
    fs::OpenOptions,
    io::{BufRead, BufReader, Write},
    net::SocketAddr,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use axum::{
    Extension, Json, Router,
    body::Bytes,
    extract::{
        Path, Query, Request, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::{HeaderMap, HeaderValue, Method, StatusCode, Uri, header},
    middleware::{self, Next},
    response::{Html, IntoResponse, Response},
    routing::{any, get, post},
};
use serde_json::{Value, json};
use tokio::sync::broadcast;

const INDEX: &str = include_str!("../assets/index.html");
const OVERLAY: &str = include_str!("../assets/overlay.html");
const WIDGETS_HOST: &str = include_str!("../assets/widgets-host.html");
const DOCK: &str = include_str!("../assets/dock.html");
const WIDGET: &str = include_str!("../assets/widget.html");
const SE_SAMPLE: &str = include_str!("../assets/se-sample.html");

/// 1x1 transparent GIF returned by `/probe/*`.
const GIF: [u8; 42] = [
    0x47, 0x49, 0x46, 0x38, 0x39, 0x61, 0x01, 0x00, 0x01, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00,
    0xff, 0xff, 0xff, 0x21, 0xf9, 0x04, 0x01, 0x00, 0x00, 0x00, 0x00, 0x2c, 0x00, 0x00, 0x00, 0x00,
    0x01, 0x00, 0x01, 0x00, 0x00, 0x02, 0x01, 0x44, 0x00, 0x3b,
];

const EVENT_RING: usize = 200;

/// Which "site" a request's Host header maps to.
#[derive(Clone, Debug, PartialEq)]
enum Site {
    /// `127.0.0.1:<port>` or `localhost:<port>`: overlays, docks, control API.
    App,
    /// `w-<id>.localhost:<port>`: one origin per widget instance.
    Widget(String),
}

#[derive(Clone)]
struct HostName(String);

struct Inner {
    port: u16,
    /// Optional extra loopback ports, one per widget (the "distinct ports" origin scheme).
    widget_ports: Vec<u16>,
    boot_ms: u64,
    reports_path: PathBuf,
    events_path: PathBuf,
    reports: Mutex<Vec<Value>>,
    events: Mutex<(u64, VecDeque<Value>)>,
    tx: broadcast::Sender<Value>,
}

type AppState = Arc<Inner>;

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn read_jsonl(path: &PathBuf) -> Vec<Value> {
    let Ok(file) = std::fs::File::open(path) else {
        return Vec::new();
    };
    BufReader::new(file)
        .lines()
        .map_while(Result::ok)
        .filter_map(|l| serde_json::from_str(&l).ok())
        .collect()
}

fn append_jsonl(path: &PathBuf, value: &Value) {
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "{value}");
    }
}

impl Inner {
    fn load(port: u16, widget_ports: Vec<u16>, data_dir: PathBuf) -> Self {
        let reports_path = data_dir.join("reports.jsonl");
        let events_path = data_dir.join("events.jsonl");
        let reports = read_jsonl(&reports_path);
        let all_events = read_jsonl(&events_path);
        let last_id = all_events
            .iter()
            .filter_map(|e| e["id"].as_u64())
            .max()
            .unwrap_or(0);
        let ring: VecDeque<Value> = all_events
            .into_iter()
            .rev()
            .take(EVENT_RING)
            .rev()
            .collect();
        let (tx, _) = broadcast::channel(256);
        Self {
            port,
            widget_ports,
            boot_ms: now_ms(),
            reports_path,
            events_path,
            reports: Mutex::new(reports),
            events: Mutex::new((last_id, ring)),
            tx,
        }
    }

    /// Append one entry to the machine-readable log (memory + JSONL file).
    fn log(&self, host: &str, kind: &str, data: Value) {
        let mut reports = self.reports.lock().expect("reports lock");
        let seq = reports.len() as u64 + 1;
        let entry = json!({ "seq": seq, "ts_ms": now_ms(), "host": host, "kind": kind, "data": data });
        append_jsonl(&self.reports_path, &entry);
        eprintln!("[{seq}] {host} {kind}");
        reports.push(entry);
    }

    /// Assign an id, store the event in the replay ring, and fan it out.
    fn publish(&self, mut ev: Value) -> (u64, usize) {
        let id = {
            let mut events = self.events.lock().expect("events lock");
            events.0 += 1;
            let id = events.0;
            ev["id"] = json!(id);
            ev["sent_ms"] = json!(now_ms());
            append_jsonl(&self.events_path, &ev);
            events.1.push_back(ev.clone());
            if events.1.len() > EVENT_RING {
                events.1.pop_front();
            }
            id
        };
        let receivers = self.tx.send(ev).unwrap_or(0);
        (id, receivers)
    }
}

/// Only loopback names on our ports are served (blocks DNS rebinding).
fn classify_host(host: &str, port: u16, widget_ports: &[u16]) -> Option<Site> {
    let (name, p) = host.rsplit_once(':')?;
    let p: u16 = p.parse().ok()?;
    if p != port {
        // Distinct-port scheme: http://127.0.0.1:<widget-port>/ is widget "p<port>".
        return (matches!(name, "127.0.0.1" | "localhost") && widget_ports.contains(&p))
            .then(|| Site::Widget(format!("p{p}")));
    }
    match name {
        "127.0.0.1" | "localhost" => Some(Site::App),
        _ => {
            let id = name.strip_suffix(".localhost")?.strip_prefix("w-")?;
            let valid = !id.is_empty()
                && id.len() <= 40
                && id
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
            valid.then(|| Site::Widget(id.to_owned()))
        }
    }
}

fn origin_allowed(origin: &str, st: &Inner) -> bool {
    origin
        .strip_prefix("http://")
        .and_then(|h| classify_host(h, st.port, &st.widget_ports))
        .is_some()
}

async fn host_guard(State(st): State<AppState>, mut req: Request, next: Next) -> Response {
    let host = req
        .headers()
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("")
        .to_ascii_lowercase();
    match classify_host(&host, st.port, &st.widget_ports) {
        Some(site) => {
            req.extensions_mut().insert(site);
            req.extensions_mut().insert(HostName(host));
            next.run(req).await
        }
        None => {
            st.log(
                &host,
                "server.host_rejected",
                json!({ "path": req.uri().path() }),
            );
            (StatusCode::MISDIRECTED_REQUEST, "unknown host").into_response()
        }
    }
}

/// Headers that tell us how Chromium classified a request (CORS, PNA/LNA, fetch metadata).
fn interesting_headers(headers: &HeaderMap) -> Value {
    let mut map = serde_json::Map::new();
    for (name, value) in headers {
        let n = name.as_str();
        if n.starts_with("sec-fetch-")
            || n.starts_with("access-control-request-")
            || n == "origin"
            || n == "referer"
        {
            map.insert(n.to_owned(), json!(value.to_str().unwrap_or("?")));
        }
    }
    Value::Object(map)
}

fn with_cors(mut resp: Response, headers: &HeaderMap) -> Response {
    let h = resp.headers_mut();
    let origin = headers
        .get(header::ORIGIN)
        .cloned()
        .unwrap_or_else(|| HeaderValue::from_static("*"));
    h.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, origin);
    h.insert(
        header::ACCESS_CONTROL_ALLOW_METHODS,
        HeaderValue::from_static("GET, POST, OPTIONS"),
    );
    h.insert(
        header::ACCESS_CONTROL_ALLOW_HEADERS,
        HeaderValue::from_static("content-type"),
    );
    // Legacy Private Network Access preflight opt-in (pre-LNA Chromium). Logged either way.
    if headers.contains_key("access-control-request-private-network") {
        h.insert(
            "access-control-allow-private-network",
            HeaderValue::from_static("true"),
        );
    }
    resp
}

async fn pages(Extension(site): Extension<Site>, uri: Uri) -> Response {
    let body = match (&site, uri.path()) {
        (Site::App, "/") => INDEX,
        (Site::App, "/o/alerts") => OVERLAY,
        (Site::App, "/o/widgets") => WIDGETS_HOST,
        (Site::App, "/dock/chat") => DOCK,
        (Site::Widget(_), "/") => WIDGET,
        (Site::Widget(_), "/sew/se-sample/") => SE_SAMPLE,
        _ => return (StatusCode::NOT_FOUND, "not found").into_response(),
    };
    ([(header::CACHE_CONTROL, "no-store")], Html(body)).into_response()
}

async fn report_get(
    State(st): State<AppState>,
    Query(q): Query<HashMap<String, String>>,
) -> Json<Vec<Value>> {
    let since: u64 = q.get("since").and_then(|s| s.parse().ok()).unwrap_or(0);
    let reports = st.reports.lock().expect("reports lock");
    Json(
        reports
            .iter()
            .filter(|r| r["seq"].as_u64().unwrap_or(0) > since)
            .cloned()
            .collect(),
    )
}

async fn report_post(
    State(st): State<AppState>,
    Extension(HostName(host)): Extension<HostName>,
    method: Method,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if method == Method::POST {
        let mut data: Value =
            serde_json::from_slice(&body).unwrap_or_else(|_| json!({ "raw": String::from_utf8_lossy(&body) }));
        let kind = data["kind"].as_str().unwrap_or("client").to_owned();
        data["_origin_hdr"] = json!(headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()));
        st.log(&host, &kind, data);
    }
    with_cors(StatusCode::NO_CONTENT.into_response(), &headers)
}

async fn emit(
    State(st): State<AppState>,
    Extension(site): Extension<Site>,
    body: Bytes,
) -> Response {
    if site != Site::App {
        return StatusCode::FORBIDDEN.into_response();
    }
    let mut ev: Value = serde_json::from_slice(&body).unwrap_or_else(|_| json!({}));
    if !ev.is_object() {
        ev = json!({ "text": ev });
    }
    if ev["type"].is_null() {
        ev["type"] = json!("alert");
    }
    let (id, receivers) = st.publish(ev);
    Json(json!({ "id": id, "receivers": receivers })).into_response()
}

async fn chat(
    State(st): State<AppState>,
    Extension(site): Extension<Site>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let same_origin = headers
        .get(header::ORIGIN)
        .and_then(|o| o.to_str().ok())
        .is_none_or(|o| origin_allowed(o, &st));
    if site != Site::App || !same_origin {
        return StatusCode::FORBIDDEN.into_response();
    }
    let msg: Value = serde_json::from_slice(&body).unwrap_or_else(|_| json!({}));
    let ev = json!({
        "type": "chat",
        "user": msg["user"].as_str().unwrap_or("dock-user"),
        "text": msg["text"].as_str().unwrap_or(""),
    });
    let (id, receivers) = st.publish(ev);
    Json(json!({ "id": id, "receivers": receivers })).into_response()
}

/// Loopback reachability probe: logs how the browser classified the request.
async fn probe(
    State(st): State<AppState>,
    Extension(HostName(host)): Extension<HostName>,
    method: Method,
    Path(name): Path<String>,
    headers: HeaderMap,
) -> Response {
    st.log(
        &host,
        "server.probe",
        json!({ "name": name, "method": method.as_str(), "headers": interesting_headers(&headers) }),
    );
    let resp = if method == Method::OPTIONS {
        StatusCode::NO_CONTENT.into_response()
    } else if name.ends_with(".json") || name.starts_with("ping") {
        Json(json!({ "pong": true, "server_ms": now_ms() })).into_response()
    } else {
        ([(header::CONTENT_TYPE, "image/gif"), (header::CACHE_CONTROL, "no-store")], GIF.to_vec())
            .into_response()
    };
    with_cors(resp, &headers)
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(st): State<AppState>,
    Extension(HostName(host)): Extension<HostName>,
    headers: HeaderMap,
    Query(q): Query<HashMap<String, String>>,
) -> Response {
    let origin = headers
        .get(header::ORIGIN)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let client = q.get("client").cloned().unwrap_or_default();
    let last: u64 = q.get("last").and_then(|s| s.parse().ok()).unwrap_or(0);
    let allowed = origin.as_deref().is_none_or(|o| origin_allowed(o, &st));
    st.log(
        &host,
        "server.ws_upgrade",
        json!({ "client": client, "origin": origin, "allowed": allowed, "last": last }),
    );
    if !allowed {
        return (StatusCode::FORBIDDEN, "origin not allowed").into_response();
    }
    ws.on_upgrade(move |socket| ws_session(socket, st, host, client, last))
}

async fn send_json(socket: &mut WebSocket, v: &Value) -> bool {
    socket.send(Message::Text(v.to_string().into())).await.is_ok()
}

async fn ws_session(mut socket: WebSocket, st: AppState, host: String, client: String, last: u64) {
    let mut rx = st.tx.subscribe();
    let hello = json!({ "type": "hello", "boot_ms": st.boot_ms, "server_ms": now_ms() });
    if !send_json(&mut socket, &hello).await {
        return;
    }
    // Resume: replay buffered events after the client's cursor (fresh clients get none).
    if last > 0 {
        let backlog: Vec<Value> = {
            let events = st.events.lock().expect("events lock");
            events
                .1
                .iter()
                .filter(|e| e["id"].as_u64().unwrap_or(0) > last)
                .cloned()
                .collect()
        };
        for mut ev in backlog {
            ev["replayed"] = json!(true);
            if !send_json(&mut socket, &ev).await {
                return;
            }
        }
    }
    let mut hb = tokio::time::interval(Duration::from_secs(5));
    hb.tick().await;
    loop {
        tokio::select! {
            msg = socket.recv() => match msg {
                Some(Ok(Message::Text(t))) => {
                    let v: Value = serde_json::from_str(t.as_str())
                        .unwrap_or_else(|_| json!({ "raw": t.as_str() }));
                    st.log(&host, "ws.client", json!({ "client": client, "msg": v, "recv_ms": now_ms() }));
                }
                Some(Ok(Message::Close(_)) | Err(_)) | None => break,
                Some(Ok(_)) => {}
            },
            ev = rx.recv() => match ev {
                Ok(ev) => if !send_json(&mut socket, &ev).await { break },
                Err(broadcast::error::RecvError::Lagged(_)) => {}
                Err(broadcast::error::RecvError::Closed) => break,
            },
            _ = hb.tick() => {
                if !send_json(&mut socket, &json!({ "type": "hb", "server_ms": now_ms() })).await { break }
            }
        }
    }
    st.log(&host, "server.ws_closed", json!({ "client": client }));
}

#[tokio::main]
async fn main() {
    let mut port: u16 = 47800;
    let mut data_dir = PathBuf::from("/tmp/den-s2/data");
    let mut widget_ports: Vec<u16> = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--port" => port = args.next().and_then(|v| v.parse().ok()).expect("--port <u16>"),
            "--data" => data_dir = args.next().map(PathBuf::from).expect("--data <dir>"),
            "--widget-ports" => {
                widget_ports = args
                    .next()
                    .expect("--widget-ports <p1,p2>")
                    .split(',')
                    .map(|p| p.parse().expect("port"))
                    .collect()
            }
            other => panic!("unknown argument {other}"),
        }
    }
    std::fs::create_dir_all(&data_dir).expect("create data dir");
    let st: AppState = Arc::new(Inner::load(port, widget_ports.clone(), data_dir));

    let app = Router::new()
        .route("/ws", get(ws_handler))
        .route("/report", get(report_get).post(report_post).options(report_post))
        .route("/emit", post(emit))
        .route("/api/chat", post(chat))
        .route("/probe/{name}", any(probe))
        .route("/healthz", get(|| async { "ok" }))
        .fallback(get(pages))
        .layer(middleware::from_fn_with_state(st.clone(), host_guard))
        .with_state(st.clone());

    for wp in widget_ports {
        let listener = tokio::net::TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], wp)))
            .await
            .expect("bind widget port");
        let app = app.clone();
        tokio::spawn(async move { axum::serve(listener, app).await });
    }
    let listener = tokio::net::TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], port)))
        .await
        .expect("bind 127.0.0.1");
    st.log(
        "server",
        "server.listening",
        json!({ "port": port, "boot_ms": st.boot_ms, "pid": std::process::id() }),
    );
    axum::serve(listener, app).await.expect("serve");
}
