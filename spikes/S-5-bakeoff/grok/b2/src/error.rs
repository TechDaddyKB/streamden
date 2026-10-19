use std::fmt;

/// Why a WebSocket text frame could not be parsed.
#[derive(Debug)]
pub enum ParseError {
    /// The frame was not valid JSON.
    Json(serde_json::Error),
    /// The frame was JSON, but not an object.
    ExpectedObject,
    /// A required field was missing. `field` is a dotted path.
    MissingField(&'static str),
    /// `metadata.message_type` was `notification`, and the subscription type is
    /// one this crate models, but the event body did not match that model.
    Event {
        /// Subscription type name, for example `channel.cheer`.
        type_name: String,
        /// Serde's reason.
        source: serde_json::Error,
    },
}

impl fmt::Display for ParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::Json(err) => write!(formatter, "invalid JSON: {err}"),
            ParseError::ExpectedObject => {
                write!(formatter, "EventSub message must be a JSON object")
            }
            ParseError::MissingField(field) => write!(formatter, "missing {field}"),
            ParseError::Event { type_name, source } => {
                write!(
                    formatter,
                    "payload for {type_name} did not match its schema: {source}"
                )
            }
        }
    }
}

impl std::error::Error for ParseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ParseError::Json(err) => Some(err),
            ParseError::Event { source, .. } => Some(source),
            ParseError::ExpectedObject | ParseError::MissingField(_) => None,
        }
    }
}

impl From<serde_json::Error> for ParseError {
    fn from(err: serde_json::Error) -> Self {
        ParseError::Json(err)
    }
}
