//! The JSON-RPC 2.0 envelope, handshake, subscriptions and error table (Spec 02 §8.2–§8.5, §12).

use std::borrow::Cow;
use std::fmt;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use ts_rs::TS;

use crate::auth::ClientInfo;
use crate::events::Event;
use crate::ids::{EventId, EventType, Scope, SessionId, TopicGlob};
use crate::timestamp::Timestamp;

/// The JSON-RPC version literal `"2.0"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, TS)]
#[ts(type = "\"2.0\"")]
pub struct JsonRpcVersion;

impl Serialize for JsonRpcVersion {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str("2.0")
    }
}

impl<'de> Deserialize<'de> for JsonRpcVersion {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = Cow::<str>::deserialize(deserializer)?;
        if s == "2.0" {
            Ok(Self)
        } else {
            Err(serde::de::Error::custom("jsonrpc must be \"2.0\""))
        }
    }
}

impl JsonSchema for JsonRpcVersion {
    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("JsonRpcVersion")
    }

    fn schema_id() -> Cow<'static, str> {
        Cow::Borrowed("den_protocol::rpc::JsonRpcVersion")
    }

    fn inline_schema() -> bool {
        true
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({ "type": "string", "const": "2.0" })
    }
}

/// A JSON-RPC request ID: a string or an integer, as JSON-RPC 2.0 allows. Not an untagged union in the
/// §3 sense: the schema is the multi-type `{ "type": ["string", "integer"] }`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, TS)]
#[ts(type = "string | number")]
pub enum RequestId {
    /// An integer ID.
    Number(i64),
    /// A string ID.
    String(String),
}

impl Serialize for RequestId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Number(n) => serializer.serialize_i64(*n),
            Self::String(s) => serializer.serialize_str(s),
        }
    }
}

impl<'de> Deserialize<'de> for RequestId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match serde_json::Value::deserialize(deserializer)? {
            serde_json::Value::String(s) => Ok(Self::String(s)),
            serde_json::Value::Number(n) => n.as_i64().map(Self::Number).ok_or_else(|| {
                serde::de::Error::custom("request id must be an integer or a string")
            }),
            _ => Err(serde::de::Error::custom(
                "request id must be an integer or a string",
            )),
        }
    }
}

impl JsonSchema for RequestId {
    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("RequestId")
    }

    fn schema_id() -> Cow<'static, str> {
        Cow::Borrowed("den_protocol::rpc::RequestId")
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": ["string", "integer"],
            "description": "JSON-RPC request ID: a string or an integer."
        })
    }
}

impl fmt::Display for RequestId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Number(n) => write!(f, "{n}"),
            Self::String(s) => f.write_str(s),
        }
    }
}

/// A JSON-RPC request (§8.2). `params` is always an object, never positional.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct RpcRequest {
    /// Always `"2.0"`.
    pub jsonrpc: JsonRpcVersion,
    /// The request ID, echoed in the response.
    pub id: RequestId,
    /// The method: a protocol method (§12) or an operation name (§8.1).
    pub method: String,
    /// Named parameters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional, as = "Option<serde_json::Value>")]
    pub params: Option<serde_json::Map<String, serde_json::Value>>,
}

/// A JSON-RPC response (§8.2). Exactly one of `result` and `error` is present: a success carries
/// `result` (which may be the JSON `null`, a valid tier-0 result) and never `error`; a failure carries
/// `error` and never `result`. The hand-written serde impl enforces this on both directions, and the
/// schema expresses it with a `oneOf` over `required`, so generated clients see the same rule.
#[derive(Debug, Clone, PartialEq, TS)]
pub struct RpcResponse {
    /// Always `"2.0"`.
    pub jsonrpc: JsonRpcVersion,
    /// The request ID, or `null` when the request could not be parsed. One of the few places where `null`
    /// is a valid value (§3).
    pub id: Option<RequestId>,
    /// The outcome: `result` or `error`, flattened into the envelope.
    #[ts(flatten)]
    pub outcome: RpcOutcome,
}

/// The two outcomes of an [`RpcResponse`] (§8.2). On the wire this is the `result` **or** `error` member
/// of the envelope, never both and never neither.
#[derive(Debug, Clone, PartialEq, TS)]
#[ts(untagged)]
pub enum RpcOutcome {
    /// The call succeeded.
    Success {
        /// The result. `null` is a valid result (a tier-0 operation may return nothing).
        #[ts(as = "serde_json::Value")]
        result: serde_json::Value,
    },
    /// The call failed.
    Failure {
        /// The error.
        error: RpcError,
    },
}

impl RpcResponse {
    /// A success envelope.
    pub fn success(id: Option<RequestId>, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: JsonRpcVersion,
            id,
            outcome: RpcOutcome::Success { result },
        }
    }

    /// A failure envelope.
    pub fn failure(id: Option<RequestId>, error: RpcError) -> Self {
        Self {
            jsonrpc: JsonRpcVersion,
            id,
            outcome: RpcOutcome::Failure { error },
        }
    }

    /// The result, when this is a success.
    pub fn result(&self) -> Option<&serde_json::Value> {
        match &self.outcome {
            RpcOutcome::Success { result } => Some(result),
            RpcOutcome::Failure { .. } => None,
        }
    }

    /// The error, when this is a failure.
    pub fn error(&self) -> Option<&RpcError> {
        match &self.outcome {
            RpcOutcome::Success { .. } => None,
            RpcOutcome::Failure { error } => Some(error),
        }
    }
}

impl Serialize for RpcResponse {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut s = serializer.serialize_struct("RpcResponse", 3)?;
        s.serialize_field("jsonrpc", &self.jsonrpc)?;
        s.serialize_field("id", &self.id)?;
        match &self.outcome {
            RpcOutcome::Success { result } => s.serialize_field("result", result)?,
            RpcOutcome::Failure { error } => s.serialize_field("error", error)?,
        }
        s.end()
    }
}

impl<'de> Deserialize<'de> for RpcResponse {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        /// Distinguishes an absent `result` from a present JSON `null`: the field is only `Some` when
        /// the key exists.
        fn present<'de, D: Deserializer<'de>>(d: D) -> Result<Option<serde_json::Value>, D::Error> {
            serde_json::Value::deserialize(d).map(Some)
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Raw {
            jsonrpc: JsonRpcVersion,
            #[serde(deserialize_with = "Option::deserialize")]
            id: Option<RequestId>,
            #[serde(default, deserialize_with = "present")]
            result: Option<serde_json::Value>,
            /// Presence, not value: `error: null` is a present (and invalid) error, not an absent one.
            #[serde(default, deserialize_with = "present")]
            error: Option<serde_json::Value>,
        }
        let raw = Raw::deserialize(deserializer)?;
        let outcome = match (raw.result, raw.error) {
            (Some(result), None) => RpcOutcome::Success { result },
            (None, Some(error)) => RpcOutcome::Failure {
                error: RpcError::deserialize(error).map_err(serde::de::Error::custom)?,
            },
            (Some(_), Some(_)) => {
                return Err(serde::de::Error::custom(
                    "a response carries either `result` or `error`, not both",
                ));
            }
            (None, None) => {
                return Err(serde::de::Error::custom(
                    "a response carries `result` or `error`; a success with no value sends `result: null`",
                ));
            }
        };
        Ok(Self {
            jsonrpc: raw.jsonrpc,
            id: raw.id,
            outcome,
        })
    }
}

impl JsonSchema for RpcResponse {
    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("RpcResponse")
    }

    fn schema_id() -> Cow<'static, str> {
        Cow::Borrowed("den_protocol::rpc::RpcResponse")
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        let jsonrpc = generator.subschema_for::<JsonRpcVersion>();
        let id = nullable_request_id(generator);
        let error = generator.subschema_for::<RpcError>();
        json_schema!({
            "description": "A JSON-RPC response (§8.2). Exactly one of `result` and `error` is present; a success with no value sends `result: null`.",
            "type": "object",
            "properties": {
                "jsonrpc": merge_description(jsonrpc, "Always `\"2.0\"`."),
                "id": merge_description(id, "The request ID, or `null` when the request could not be parsed. One of the few places where `null` is a valid value (§3)."),
                "result": {
                    "description": "The result, on success. `null` is a valid result (a tier-0 operation may return nothing).",
                    "x-den-nullable": true
                },
                "error": merge_description(error, "The error, on failure.")
            },
            "required": ["jsonrpc", "id"],
            "oneOf": [
                { "required": ["result"], "not": { "required": ["error"] } },
                { "required": ["error"], "not": { "required": ["result"] } }
            ]
        })
    }
}

/// Adds a `description` to a subschema (a `$ref` or an inline schema) the way the derive does.
fn merge_description(schema: Schema, description: &str) -> Schema {
    let mut value = schema.to_value();
    if let Some(obj) = value.as_object_mut() {
        obj.insert("description".into(), description.into());
    }
    Schema::try_from(value).expect("object")
}

/// Schema of a required, nullable request ID (`RpcResponse.id`).
fn nullable_request_id(generator: &mut SchemaGenerator) -> Schema {
    let id = generator.subschema_for::<RequestId>();
    json_schema!({ "anyOf": [id, { "type": "null" }], "x-den-nullable": true })
}

/// A JSON-RPC notification (§8.2): a request without `id`, in either direction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct RpcNotification {
    /// Always `"2.0"`.
    pub jsonrpc: JsonRpcVersion,
    /// The notification name (`hello`, `event`, `events.dropped`, …).
    pub method: String,
    /// Named parameters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional, as = "Option<serde_json::Value>")]
    pub params: Option<serde_json::Map<String, serde_json::Value>>,
}

/// A JSON-RPC error object (§8.5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct RpcError {
    /// The JSON-RPC code: a protocol fault code, or `-32000` for application errors (`-32602` for
    /// `INVALID_PARAMS`).
    pub code: i64,
    /// Human-readable message.
    pub message: String,
    /// Structured detail, always present for application errors.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub data: Option<ErrorData>,
}

/// Structured error detail (§8.5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct ErrorData {
    /// The stable application error code. Clients treat unknown codes as a generic failure.
    pub code: ErrorCode,
    /// Code-specific details (`required`, `reason`, `approvalId`, `supported`, …).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional, as = "Option<serde_json::Value>")]
    pub details: Option<serde_json::Value>,
    /// Per-field validation errors, set for `INVALID_PARAMS`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub field_errors: Option<Vec<FieldError>>,
    /// When to retry, for `RATE_LIMITED`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub retry_after_ms: Option<u64>,
    /// Link to documentation about this error.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub docs_url: Option<String>,
}

/// One validation error (§8.5).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct FieldError {
    /// JSON Pointer-style path of the offending field (`/message`, `/source/name`).
    pub path: String,
    /// What is wrong with it.
    pub message: String,
}

/// Application error code (§8.5): an open string enum. New codes are additive.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, TS)]
#[serde(transparent)]
pub struct ErrorCode(pub String);

/// One row of the §8.5 error table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ErrorCodeInfo {
    /// The `data.code` value.
    pub code: &'static str,
    /// The JSON-RPC `code` it is sent with.
    pub jsonrpc_code: i64,
    /// Meaning.
    pub description: &'static str,
}

/// JSON-RPC code for every application error except `INVALID_PARAMS` (§8.5).
pub const JSONRPC_APPLICATION_ERROR: i64 = -32000;
/// JSON-RPC parse error.
pub const JSONRPC_PARSE_ERROR: i64 = -32700;
/// JSON-RPC invalid request.
pub const JSONRPC_INVALID_REQUEST: i64 = -32600;
/// JSON-RPC method not found.
pub const JSONRPC_METHOD_NOT_FOUND: i64 = -32601;
/// JSON-RPC invalid params; also the code of `INVALID_PARAMS`.
pub const JSONRPC_INVALID_PARAMS: i64 = -32602;
/// JSON-RPC internal error.
pub const JSONRPC_INTERNAL_ERROR: i64 = -32603;

macro_rules! error_codes {
    ($($(#[$meta:meta])* $konst:ident = $code:literal, $jsonrpc:expr, $desc:literal;)*) => {
        impl ErrorCode {
            $(
                $(#[$meta])*
                pub const $konst: &'static str = $code;
            )*

            /// The error table of §8.5, in spec order.
            pub const TABLE: &'static [ErrorCodeInfo] = &[
                $(ErrorCodeInfo { code: $code, jsonrpc_code: $jsonrpc, description: $desc },)*
            ];
        }
    };
}

error_codes! {
    /// No or invalid identify.
    UNAUTHENTICATED = "UNAUTHENTICATED", JSONRPC_APPLICATION_ERROR, "No or invalid identify.";
    /// Scope missing (`details.required`), or the call's effective tier exceeds the context's `maxTier`
    /// (`details.reason = "tier_exceeds_context"`, plus `details.node` and `details.chain`, §8.8).
    PERMISSION_DENIED = "PERMISSION_DENIED", JSONRPC_APPLICATION_ERROR, "Scope missing (details.required lists it), or the call's effective tier exceeds the context's maxTier (details.reason = \"tier_exceeds_context\", plus details.node inside a run and details.chain when inherited across a causedBy chain).";
    /// Schema validation failed; sent with JSON-RPC code -32602 and `fieldErrors` set.
    INVALID_PARAMS = "INVALID_PARAMS", JSONRPC_INVALID_PARAMS, "Schema validation failed. Always sent with JSON-RPC code -32602; fieldErrors set.";
    /// Entity missing.
    NOT_FOUND = "NOT_FOUND", JSONRPC_APPLICATION_ERROR, "Entity missing.";
    /// Name or version clash.
    CONFLICT = "CONFLICT", JSONRPC_APPLICATION_ERROR, "Name or version clash.";
    /// Wrong state (queue paused, running a disabled draft).
    INVALID_STATE = "INVALID_STATE", JSONRPC_APPLICATION_ERROR, "Wrong state (e.g. queue paused, running a disabled draft).";
    /// Rate limited; `retryAfterMs` set.
    RATE_LIMITED = "RATE_LIMITED", JSONRPC_APPLICATION_ERROR, "Rate limited; retryAfterMs set.";
    /// Operation deadline passed.
    TIMEOUT = "TIMEOUT", JSONRPC_APPLICATION_ERROR, "Operation deadline passed.";
    /// Cancelled by the client or by shutdown.
    CANCELLED = "CANCELLED", JSONRPC_APPLICATION_ERROR, "Cancelled by the client or by shutdown.";
    /// Plugin not running.
    PLUGIN_UNAVAILABLE = "PLUGIN_UNAVAILABLE", JSONRPC_APPLICATION_ERROR, "Plugin not running.";
    /// Connector not connected.
    CONNECTION_UNAVAILABLE = "CONNECTION_UNAVAILABLE", JSONRPC_APPLICATION_ERROR, "Connector not connected.";
    /// Platform or app returned an error (`details.upstream`).
    UPSTREAM_ERROR = "UPSTREAM_ERROR", JSONRPC_APPLICATION_ERROR, "Platform or app returned an error (details.upstream).";
    /// In-app approval pending (`details.approvalId`), or `details.reason` ∈ { `binding_mismatch`, `unattended` } (§8.8).
    APPROVAL_REQUIRED = "APPROVAL_REQUIRED", JSONRPC_APPLICATION_ERROR, "In-app approval pending (details.approvalId); details.reason = \"binding_mismatch\" with details.mismatch in { \"args\", \"revision\", \"unplanned_call\", \"expired\" } and details.action/details.node, or details.reason = \"unattended\" for a tier-3 node reached through a causedBy chain with nobody to answer.";
    /// "Pause AI control" or a global pause is on.
    PAUSED = "PAUSED", JSONRPC_APPLICATION_ERROR, "\"Pause AI control\" or a global pause is on.";
    /// OS-gated operation (`details.platforms`).
    UNSUPPORTED_ON_PLATFORM = "UNSUPPORTED_ON_PLATFORM", JSONRPC_APPLICATION_ERROR, "OS-gated operation (details.platforms).";
    /// Size limit exceeded.
    PAYLOAD_TOO_LARGE = "PAYLOAD_TOO_LARGE", JSONRPC_APPLICATION_ERROR, "Size limit exceeded.";
    /// Unsupported `rpcVersion` on REST (WS closes with 4002); `details.supported` gives the range.
    UNSUPPORTED_VERSION = "UNSUPPORTED_VERSION", JSONRPC_APPLICATION_ERROR, "Unsupported rpcVersion on REST (WS closes with 4002); details.supported gives the range.";
}

impl ErrorCode {
    /// Wraps a code string.
    pub fn new(code: impl Into<String>) -> Self {
        Self(code.into())
    }

    /// The string form.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The table row for this code, when it is a known code.
    pub fn info(&self) -> Option<&'static ErrorCodeInfo> {
        Self::TABLE.iter().find(|row| row.code == self.0)
    }

    /// The JSON-RPC `code` this application error is sent with: `-32602` for `INVALID_PARAMS`, else `-32000`.
    pub fn jsonrpc_code(&self) -> i64 {
        self.info()
            .map_or(JSONRPC_APPLICATION_ERROR, |row| row.jsonrpc_code)
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// One WebSocket close code (§8.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CloseCodeInfo {
    /// The close code.
    pub code: u16,
    /// A stable name.
    pub name: &'static str,
    /// Meaning.
    pub description: &'static str,
}

/// WebSocket close codes (§8.3).
pub mod close_code {
    use super::CloseCodeInfo;

    /// Unknown error.
    pub const UNKNOWN: u16 = 4000;
    /// The client did not identify within the timeout.
    pub const IDENTIFY_TIMEOUT: u16 = 4001;
    /// Unsupported `rpcVersion`; the reason carries the supported range.
    pub const UNSUPPORTED_RPC_VERSION: u16 = 4002;
    /// Authentication failed.
    pub const AUTHENTICATION_FAILED: u16 = 4003;
    /// The token was revoked.
    pub const TOKEN_REVOKED: u16 = 4004;
    /// A message exceeded the size limit.
    pub const MESSAGE_TOO_LARGE: u16 = 4005;
    /// The session was rate limited or kept overflowing its send queue.
    pub const RATE_LIMITED: u16 = 4006;
    /// The server is shutting down; reconnect with backoff.
    pub const SERVER_SHUTTING_DOWN: u16 = 4007;
    /// Another connection replaced this session.
    pub const SESSION_REPLACED: u16 = 4008;

    /// The close-code table of §8.3.
    pub const TABLE: &[CloseCodeInfo] = &[
        CloseCodeInfo {
            code: UNKNOWN,
            name: "unknown",
            description: "Unknown error.",
        },
        CloseCodeInfo {
            code: IDENTIFY_TIMEOUT,
            name: "identify_timeout",
            description: "The client did not call session.identify within 10 s.",
        },
        CloseCodeInfo {
            code: UNSUPPORTED_RPC_VERSION,
            name: "unsupported_rpc_version",
            description: "Unsupported rpcVersion; the reason carries the supported range.",
        },
        CloseCodeInfo {
            code: AUTHENTICATION_FAILED,
            name: "authentication_failed",
            description: "Authentication failed.",
        },
        CloseCodeInfo {
            code: TOKEN_REVOKED,
            name: "token_revoked",
            description: "The token was revoked.",
        },
        CloseCodeInfo {
            code: MESSAGE_TOO_LARGE,
            name: "message_too_large",
            description: "A message exceeded the size limit.",
        },
        CloseCodeInfo {
            code: RATE_LIMITED,
            name: "rate_limited",
            description: "Rate limited, or the session kept overflowing its send queue.",
        },
        CloseCodeInfo {
            code: SERVER_SHUTTING_DOWN,
            name: "server_shutting_down",
            description: "The server is shutting down; clients should reconnect with backoff.",
        },
        CloseCodeInfo {
            code: SESSION_REPLACED,
            name: "session_replaced",
            description: "Another connection replaced this session.",
        },
    ];
}

/// The `hello` notification the server sends on connect (§8.3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct Hello {
    /// Always `"StreamDen"`.
    pub server: String,
    /// The app version (SemVer).
    pub version: String,
    /// The `protocolVersion` of the served `protocol.json`.
    pub protocol_version: String,
    /// The `rpcVersion` range this server accepts.
    pub rpc_versions: RpcVersionRange,
    /// Supported authentication methods: `token`, `pairing`.
    pub auth: Vec<String>,
    /// Optional features: `batch`, `replay`, `sse`.
    pub features: Vec<String>,
    /// Heartbeat interval; the server closes a session silent for three intervals.
    pub heartbeat_ms: u32,
}

/// An inclusive `rpcVersion` range (§8.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct RpcVersionRange {
    /// Oldest supported version.
    pub min: u32,
    /// Newest supported version.
    pub max: u32,
}

/// Params of `session.identify` (§8.3). `Debug` redacts the token (AGENTS.md: secrets never reach logs).
#[derive(Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct SessionIdentifyParams {
    /// The `rpcVersion` the client speaks.
    pub rpc_version: u32,
    /// An API token (`sdt_…`). Absent only when pairing (spec 06).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    #[schemars(extend("x-secret" = true))]
    pub token: Option<String>,
    /// The client's self-description.
    pub client: ClientInfo,
    /// Initial subscription topics (§8.4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub subscribe: Option<Vec<TopicGlob>>,
    /// High-volume event types to deliver, listed exactly (§8.4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub high_volume: Option<Vec<EventType>>,
    /// Replay cursor: re-deliver events newer than this ID (§8.4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub since: Option<EventId>,
}

impl fmt::Debug for SessionIdentifyParams {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SessionIdentifyParams")
            .field("rpc_version", &self.rpc_version)
            .field("token", &self.token.as_ref().map(|_| "***"))
            .field("client", &self.client)
            .field("subscribe", &self.subscribe)
            .field("high_volume", &self.high_volume)
            .field("since", &self.since)
            .finish()
    }
}

/// Result of `session.identify` (§8.3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct SessionIdentifyResult {
    /// The session ID.
    pub session_id: SessionId,
    /// The `rpcVersion` the session will use.
    pub negotiated_rpc_version: u32,
    /// The token's scopes (§8.7).
    pub scopes: Vec<Scope>,
    /// The server's clock, for skew detection.
    pub server_time: Timestamp,
    /// How many events were replayed for `since`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub replayed: Option<u32>,
    /// `true` when `since` was older than the replay buffer and the client should resync.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub gap: Option<bool>,
}

/// Params of `session.reidentify` (§8.3): replaces the session's subscription set without reconnecting.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct SessionReidentifyParams {
    /// The new topic set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub subscribe: Option<Vec<TopicGlob>>,
    /// The new high-volume set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub high_volume: Option<Vec<EventType>>,
}

/// Params of `events.subscribe` (§8.4): adds to the session's subscription set.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct EventsSubscribeParams {
    /// Topics to add.
    pub topics: Vec<TopicGlob>,
    /// High-volume event types to add, listed exactly.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub high_volume: Option<Vec<EventType>>,
    /// Replay cursor: re-deliver events newer than this ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub since: Option<EventId>,
}

/// Params of `events.unsubscribe` (§8.4): removes from the session's subscription set.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct EventsUnsubscribeParams {
    /// Topics to remove.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub topics: Option<Vec<TopicGlob>>,
    /// High-volume event types to remove.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub high_volume: Option<Vec<EventType>>,
}

/// Params of the `event` notification (§8.4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct EventNotification {
    /// The delivered event.
    pub event: Event,
}

/// Params of the `events.dropped` notification (§8.4): the slow-consumer policy dropped events.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct EventsDroppedNotification {
    /// How many events were dropped.
    pub count: u32,
    /// The last event delivered before the drop; use it as a replay cursor.
    pub since: EventId,
}

/// Params of `batch.execute` (§8.2): ordered execution of up to 64 requests.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct BatchExecuteParams {
    /// Execution mode; only `serial` in v1.
    pub mode: BatchMode,
    /// Stop at the first failing entry.
    #[serde(default)]
    pub halt_on_failure: bool,
    /// The requests, executed in order under the caller's context. At most 64 entries (§8.2).
    #[schemars(length(max = 64))]
    pub requests: Vec<RpcRequest>,
}

/// Execution modes of `batch.execute` (§8.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum BatchMode {
    /// One after another.
    Serial,
}

/// Result of `batch.execute` (§8.2): one response per entry, in order. Entries after a halt are absent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct BatchExecuteResult {
    /// The responses.
    pub results: Vec<RpcResponse>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_id_accepts_string_and_integer() {
        assert_eq!(
            serde_json::from_str::<RequestId>("7").unwrap(),
            RequestId::Number(7)
        );
        assert_eq!(
            serde_json::from_str::<RequestId>("\"a\"").unwrap(),
            RequestId::String("a".into())
        );
        assert!(serde_json::from_str::<RequestId>("1.5").is_err());
        assert!(serde_json::from_str::<RequestId>("null").is_err());
    }

    #[test]
    fn jsonrpc_version_literal() {
        assert!(serde_json::from_str::<JsonRpcVersion>("\"1.0\"").is_err());
        assert_eq!(serde_json::to_string(&JsonRpcVersion).unwrap(), "\"2.0\"");
    }

    #[test]
    fn identify_debug_redacts_the_token() {
        let params = SessionIdentifyParams {
            rpc_version: 1,
            token: Some("sdt_secret".into()),
            client: ClientInfo {
                name: "t".into(),
                version: "1".into(),
            },
            subscribe: None,
            high_volume: None,
            since: None,
        };
        let dbg = format!("{params:?}");
        assert!(dbg.contains("***") && !dbg.contains("sdt_secret"), "{dbg}");
    }

    #[test]
    fn invalid_params_uses_jsonrpc_code() {
        assert_eq!(
            ErrorCode::new(ErrorCode::INVALID_PARAMS).jsonrpc_code(),
            JSONRPC_INVALID_PARAMS
        );
        assert_eq!(
            ErrorCode::new(ErrorCode::NOT_FOUND).jsonrpc_code(),
            JSONRPC_APPLICATION_ERROR
        );
        assert_eq!(
            ErrorCode::new("SOMETHING_NEW").jsonrpc_code(),
            JSONRPC_APPLICATION_ERROR
        );
        assert_eq!(ErrorCode::TABLE.len(), 17);
    }
}
