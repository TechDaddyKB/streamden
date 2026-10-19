//! Engine failures mapped to existing protocol codes (Spec 02 §8.5, Spec 03).

use den_protocol::rpc::{ErrorCode, ErrorData};
use serde_json::{Value, json};

/// Internal failure metadata. Private fields prevent reason/details drift; payloads and panic
/// messages are deliberately not accepted. Extend with typed, safe metadata when a consumer needs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineError {
    code: ErrorCode,
    reason: Option<&'static str>,
}

impl EngineError {
    /// An existing protocol error without additional detail.
    pub fn new(code: ErrorCode) -> Self {
        Self { code, reason: None }
    }

    fn with_reason(code: &'static str, reason: &'static str) -> Self {
        Self {
            code: ErrorCode(code.to_owned()),
            reason: Some(reason),
        }
    }

    pub fn code(&self) -> &ErrorCode {
        &self.code
    }

    pub fn reason(&self) -> Option<&'static str> {
        self.reason
    }

    pub fn details(&self) -> Option<Value> {
        self.reason.map(|reason| json!({ "reason": reason }))
    }

    // Spec 03 §§9.8, 10.2, 10.8, 16.
    pub fn queue_full() -> Self {
        Self::with_reason(ErrorCode::INVALID_STATE, "queue_full")
    }
    pub fn shutting_down() -> Self {
        Self::with_reason(ErrorCode::INVALID_STATE, "shutting_down")
    }
    // Spec 03 §§9.6, 9.8.
    pub fn node_budget() -> Self {
        Self::with_reason(ErrorCode::INVALID_STATE, "node_budget")
    }
    pub fn run_budget() -> Self {
        Self::with_reason(ErrorCode::PAYLOAD_TOO_LARGE, "run_budget")
    }
    pub fn inline_depth() -> Self {
        Self::with_reason(ErrorCode::INVALID_STATE, "inline_depth")
    }
    // Spec 03 §10.4. Never retain a caught panic's payload or Debug output.
    pub fn panic() -> Self {
        Self::with_reason(ErrorCode::INTERNAL, "panic")
    }
    // Spec 02 §5.2 / Spec 03 §12.5.
    pub fn payload_version() -> Self {
        Self::with_reason(ErrorCode::INVALID_STATE, "payload_version")
    }
    // Spec 02 §6.4 / Spec 03 §9.5.
    pub fn not_iterable() -> Self {
        Self::with_reason(ErrorCode::INVALID_PARAMS, "not_iterable")
    }
    // Spec 03 §§9.3, 16.
    pub fn invalid_result() -> Self {
        Self::with_reason(ErrorCode::UPSTREAM_ERROR, "invalid_result")
    }
    pub fn read_only() -> Self {
        Self::with_reason(ErrorCode::INVALID_STATE, "read_only")
    }
}

impl From<EngineError> for ErrorData {
    fn from(error: EngineError) -> Self {
        Self {
            details: error.details(),
            code: error.code,
            field_errors: None,
            retry_after_ms: None,
            docs_url: None,
        }
    }
}

impl std::fmt::Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.code.0)?;
        if let Some(reason) = self.reason {
            write!(f, ": {reason}")?;
        }
        Ok(())
    }
}

impl std::error::Error for EngineError {}
