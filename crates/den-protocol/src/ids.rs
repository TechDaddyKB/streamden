//! Identifier types (Spec 02 §4).
//!
//! Entity IDs are UUIDs (new ones are UUIDv7, any version is accepted so imported Streamer.bot GUIDs
//! keep their identity). Names in namespaces are validated strings with the patterns from §3 and §4.
//! Validation happens on deserialization and in `TryFrom<String>`, so a value of one of these types is
//! always well-formed.

use std::borrow::Cow;
use std::fmt;
use std::str::FromStr;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Deserializer, Serialize};
use ts_rs::TS;
use uuid::Uuid;

/// Error returned when a string does not match the pattern of an identifier type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidId {
    /// The identifier type that rejected the value.
    pub kind: &'static str,
    /// The pattern (or rule) the value had to match.
    pub pattern: &'static str,
    /// The rejected value.
    pub value: String,
}

impl fmt::Display for InvalidId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid {}: {:?} does not match {}",
            self.kind, self.value, self.pattern
        )
    }
}

impl std::error::Error for InvalidId {}

macro_rules! uuid_id {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default,
            Serialize, Deserialize, JsonSchema, TS,
        )]
        #[serde(transparent)]
        pub struct $name(pub Uuid);

        impl $name {
            /// The nil UUID `00000000-0000-0000-0000-000000000000`.
            pub const NIL: Self = Self(Uuid::nil());

            /// Mints a new time-ordered UUIDv7 (§4).
            pub fn new_v7() -> Self {
                Self(Uuid::now_v7())
            }

            /// Returns `true` for the nil UUID.
            pub fn is_nil(&self) -> bool {
                self.0.is_nil()
            }
        }

        impl From<Uuid> for $name {
            fn from(value: Uuid) -> Self {
                Self(value)
            }
        }

        impl From<$name> for Uuid {
            fn from(value: $name) -> Self {
                value.0
            }
        }

        impl FromStr for $name {
            type Err = uuid::Error;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Uuid::parse_str(s).map(Self)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                // Lowercase hyphenated, the only form we emit (§4).
                write!(f, "{}", self.0.as_hyphenated())
            }
        }
    };
}

uuid_id! {
    /// Identifies an action. New IDs are UUIDv7; imported Streamer.bot GUIDs (v4) are kept.
    ActionId
}
uuid_id! {
    /// Identifies a trigger inside an action.
    TriggerId
}
uuid_id! {
    /// Identifies a sub-action node in the tree (§6.4).
    NodeId
}
uuid_id! {
    /// Identifies a queue. The nil UUID is the Default queue (§4).
    QueueId
}
uuid_id! {
    /// Identifies a chat command record.
    CommandId
}
uuid_id! {
    /// Identifies a timer record.
    TimerId
}
uuid_id! {
    /// Identifies a connection (an account or app link of a connector).
    ConnectionId
}
uuid_id! {
    /// Identifies an overlay.
    OverlayId
}
uuid_id! {
    /// Identifies one event occurrence. Always UUIDv7, monotonic per process, used as a replay cursor (§8.4).
    EventId
}
uuid_id! {
    /// Identifies one run of an action, including the synthetic run of a direct operation call (§8.8 rule 1).
    RunId
}
uuid_id! {
    /// Identifies an API token record. The token value itself (`sdt_…`) is a secret and never appears on the wire.
    TokenId
}
uuid_id! {
    /// Identifies any persisted entity when the kind is carried separately (drafts, pending changes).
    EntityId
}

impl QueueId {
    /// The Default queue (§4): the nil UUID, matching Streamer.bot's export convention.
    pub const DEFAULT: Self = Self::NIL;
}

/// Declares a validated string newtype: transparent on the wire, checked on deserialization.
macro_rules! validated_string {
    (
        $(#[$meta:meta])*
        $name:ident, kind = $kind:literal, pattern = $pattern:expr, check = $check:expr
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, TS)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            /// Human-readable name of this identifier type, used in error messages.
            pub const KIND: &'static str = $kind;
            /// The JSON Schema `pattern` (or rule description) values must satisfy.
            pub const PATTERN: &'static str = $pattern;

            /// Validates `value` and wraps it.
            pub fn new(value: impl Into<String>) -> Result<Self, InvalidId> {
                let value: String = value.into();
                if $check(value.as_str()) {
                    Ok(Self(value))
                } else {
                    Err(InvalidId { kind: $kind, pattern: $pattern, value })
                }
            }

            /// Returns `true` when `value` satisfies the pattern.
            pub fn is_valid(value: &str) -> bool {
                $check(value)
            }

            /// The string form.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl TryFrom<String> for $name {
            type Error = InvalidId;
            fn try_from(value: String) -> Result<Self, Self::Error> {
                Self::new(value)
            }
        }

        impl TryFrom<&str> for $name {
            type Error = InvalidId;
            fn try_from(value: &str) -> Result<Self, Self::Error> {
                Self::new(value)
            }
        }

        impl FromStr for $name {
            type Err = InvalidId;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Self::new(s)
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }

        impl From<$name> for String {
            fn from(value: $name) -> Self {
                value.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let value = String::deserialize(deserializer)?;
                Self::new(value).map_err(serde::de::Error::custom)
            }
        }

        impl JsonSchema for $name {
            fn schema_name() -> Cow<'static, str> {
                Cow::Borrowed(stringify!($name))
            }

            fn schema_id() -> Cow<'static, str> {
                Cow::Borrowed(concat!("den_protocol::ids::", stringify!($name)))
            }

            fn json_schema(_: &mut SchemaGenerator) -> Schema {
                let mut schema = json_schema!({ "type": "string" });
                if let Some(pattern) = pattern_for_schema($pattern) {
                    schema.insert("pattern".into(), pattern.into());
                }
                schema
            }
        }
    };
}

/// Patterns are JSON Schema regexes unless they start with `rule:`, which documents a rule the schema
/// cannot express as a single ECMA regex.
fn pattern_for_schema(pattern: &'static str) -> Option<&'static str> {
    pattern
        .strip_prefix("rule:")
        .map_or(Some(pattern), |_| None)
}

fn is_lower_alnum_underscore(c: char) -> bool {
    c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'
}

/// `^[a-z][a-z0-9_]*$`
pub(crate) fn is_lower_snake_segment(s: &str) -> bool {
    let mut chars = s.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_lowercase())
        && chars.all(is_lower_alnum_underscore)
}

/// The JSON Schema `pattern` of a dotted name in a namespace (§3).
pub const DOTTED_NAME_PATTERN: &str = "^[a-z][a-z0-9_]*(\\.[a-z][a-z0-9_]*)+$";

/// `^[a-z][a-z0-9_]*(\.[a-z][a-z0-9_]*)+$` (§3): at least two dotted `lower_snake_case` segments.
pub(crate) fn is_dotted_name(s: &str) -> bool {
    let mut segments = s.split('.');
    let first = segments.next().is_some_and(is_lower_snake_segment);
    let mut rest = 0usize;
    let rest_ok = segments.all(|seg| {
        rest += 1;
        is_lower_snake_segment(seg)
    });
    first && rest_ok && rest >= 1
}

/// `^[a-z0-9]+(\.[a-z0-9-]+)+$`, at most 128 characters (§4).
fn is_plugin_id(s: &str) -> bool {
    if s.len() > 128 {
        return false;
    }
    let mut segments = s.split('.');
    let first = segments.next().is_some_and(|seg| {
        !seg.is_empty()
            && seg
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
    });
    let mut rest = 0usize;
    let rest_ok = segments.all(|seg| {
        rest += 1;
        !seg.is_empty()
            && seg
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    });
    first && rest_ok && rest >= 1
}

/// `^[a-z][a-z0-9_]{1,23}$` (§4).
fn is_slug(s: &str) -> bool {
    (2..=24).contains(&s.len()) && is_lower_snake_segment(s)
}

/// `^[A-Za-z_][A-Za-z0-9_]{0,63}$` (§6.3).
fn is_variable_name(s: &str) -> bool {
    let mut chars = s.chars();
    (1..=64).contains(&s.len())
        && matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// `^[A-Za-z0-9_-]{N}$`
fn is_base64url(s: &str, len: usize) -> bool {
    s.len() == len
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn is_approval_id(s: &str) -> bool {
    s.strip_prefix("apr_")
        .is_some_and(|rest| is_base64url(rest, 22))
}

fn is_session_id(s: &str) -> bool {
    s.strip_prefix("ses_")
        .is_some_and(|rest| is_base64url(rest, 22))
}

/// `^[0-9a-f]{64}$`
fn is_sha256_hex(s: &str) -> bool {
    s.len() == 64
        && s.chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
}

/// `<area>:<verb>[:<qualifier>]` (§8.7). Area and verb are `lower_snake_case`; the qualifier is free text
/// without whitespace (an action ID, a tag, a glob, a plugin ID or a variable name).
fn is_scope(s: &str) -> bool {
    let mut parts = s.splitn(3, ':');
    let area = parts.next().is_some_and(is_lower_snake_segment);
    let verb = parts.next().is_some_and(is_lower_snake_segment);
    let qualifier = parts
        .next()
        .is_none_or(|q| !q.is_empty() && !q.chars().any(char::is_whitespace));
    area && verb && qualifier
}

/// Topic globs are event types where a segment may be `*` and the last segment may be `**` (§8.4).
fn is_topic_glob(s: &str) -> bool {
    let segments: Vec<&str> = s.split('.').collect();
    if segments.is_empty() {
        return false;
    }
    let last = segments.len() - 1;
    segments
        .iter()
        .enumerate()
        .all(|(i, seg)| *seg == "*" || (i == last && *seg == "**") || is_lower_snake_segment(seg))
}

fn is_platform_id(s: &str) -> bool {
    !s.is_empty() && s.len() <= 64 && is_lower_snake_segment(s)
}

validated_string! {
    /// Reverse-DNS plugin ID (`io.github.alice.acmelive`), at most 128 characters (§4).
    PluginId, kind = "plugin ID", pattern = "^[a-z0-9]+(\\.[a-z0-9-]+)+$", check = is_plugin_id
}

validated_string! {
    /// A plugin's short slug (`acme`), the prefix of every event type, operation and variable namespace it
    /// contributes (§4). Unique per installation.
    Slug, kind = "slug", pattern = "^[a-z][a-z0-9_]{1,23}$", check = is_slug
}

validated_string! {
    /// The local part of a contribution ID (`gift` in `acme.gift`): `lower_snake_case` (§4).
    LocalId, kind = "local ID", pattern = "^[a-z][a-z0-9_]*$", check = is_lower_snake_segment
}

validated_string! {
    /// An event type: dotted `lower_snake_case`, `<slug>.<name>` (`twitch.subscription`, §5.2).
    EventType, kind = "event type", pattern = DOTTED_NAME_PATTERN, check = is_dotted_name
}

validated_string! {
    /// An operation name: dotted `lower_snake_case` (`obs.set_scene`, `actions.run`, §8.1).
    OperationName, kind = "operation name", pattern = DOTTED_NAME_PATTERN, check = is_dotted_name
}

validated_string! {
    /// A variable or run-argument name: `^[A-Za-z_][A-Za-z0-9_]{0,63}$`, case-sensitive (§6.3).
    VariableName, kind = "variable name", pattern = "^[A-Za-z_][A-Za-z0-9_]{0,63}$", check = is_variable_name
}

validated_string! {
    /// An MCP approval handle: `apr_` plus 22 base64url characters, 120 s TTL (§4).
    ApprovalId, kind = "approval ID", pattern = "^apr_[A-Za-z0-9_-]{22}$", check = is_approval_id
}

validated_string! {
    /// A WebSocket session ID: `ses_` plus 22 base64url characters (§4).
    SessionId, kind = "session ID", pattern = "^ses_[A-Za-z0-9_-]{22}$", check = is_session_id
}

validated_string! {
    /// A SHA-256 digest as 64 lowercase hex characters (binding hashes, §8.8).
    Sha256, kind = "SHA-256 hex digest", pattern = "^[0-9a-f]{64}$", check = is_sha256_hex
}

validated_string! {
    /// A scope `<area>:<verb>[:<qualifier>]` (`actions:run:<actionId|tag>`, §8.7).
    Scope, kind = "scope", pattern = "^[a-z][a-z0-9_]*:[a-z][a-z0-9_]*(:[^\\s]+)?$", check = is_scope
}

validated_string! {
    /// A subscription topic: an event type where `*` matches one segment and a trailing `**` matches the
    /// rest (`twitch.**`, `*.follow`, §8.4).
    TopicGlob, kind = "topic glob", pattern = "^(\\*|[a-z][a-z0-9_]*)(\\.(\\*|[a-z][a-z0-9_]*))*(\\.\\*\\*)?$", check = is_topic_glob
}

validated_string! {
    /// Open platform identifier: `twitch`, `youtube`, `kick`, `x`, `bluesky`, or a plugin slug (§5.1). Clients
    /// must accept unknown values.
    PlatformId, kind = "platform ID", pattern = "^[a-z][a-z0-9_]{0,63}$", check = is_platform_id
}

impl OperationName {
    /// The MCP tool name: `.` becomes `_` (§9.6). Callers check the result against the MCP name rule.
    pub fn mcp_tool_name(&self) -> String {
        self.0.replace('.', "_")
    }

    /// The slug before the first dot.
    pub fn slug(&self) -> &str {
        self.0.split('.').next().unwrap_or_default()
    }
}

impl EventType {
    /// The slug before the first dot.
    pub fn slug(&self) -> &str {
        self.0.split('.').next().unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_ids() {
        assert!(PluginId::is_valid("io.github.alice.acmelive"));
        assert!(PluginId::is_valid("com.example.my-plugin"));
        assert!(!PluginId::is_valid("acme"));
        assert!(!PluginId::is_valid("Io.Github.x"));
        assert!(!PluginId::is_valid(&"a.".repeat(70)));
    }

    #[test]
    fn slugs_and_names() {
        assert!(Slug::is_valid("acme"));
        assert!(!Slug::is_valid("a"));
        assert!(!Slug::is_valid("Acme"));
        assert!(EventType::is_valid("twitch.channel_point_redemption"));
        assert!(!EventType::is_valid("twitch"));
        assert!(!EventType::is_valid("twitch.Follow"));
        assert!(OperationName::is_valid("obs.set_scene"));
        assert_eq!(
            OperationName::new("obs.set_scene").unwrap().mcp_tool_name(),
            "obs_set_scene"
        );
        assert!(VariableName::is_valid("_x1"));
        assert!(!VariableName::is_valid("1x"));
    }

    #[test]
    fn handles_and_hashes() {
        assert!(ApprovalId::is_valid("apr_AAAAAAAAAAAAAAAAAAAAAA"));
        assert!(!ApprovalId::is_valid("apr_short"));
        assert!(SessionId::is_valid("ses_abcdefghijklmnopqrstuv"));
        assert!(Sha256::is_valid(&"a".repeat(64)));
        assert!(!Sha256::is_valid(&"A".repeat(64)));
    }

    #[test]
    fn scopes_and_globs() {
        for s in [
            "actions:read",
            "actions:run:tag",
            "events:read:twitch.**",
            "admin:all",
            "plugins:call:io.github.a.b",
        ] {
            assert!(Scope::is_valid(s), "{s}");
        }
        assert!(!Scope::is_valid("admin"));
        assert!(!Scope::is_valid("a:b:"));
        for g in ["twitch.**", "*.follow", "chat.message", "*"] {
            assert!(TopicGlob::is_valid(g), "{g}");
        }
        assert!(!TopicGlob::is_valid("twitch.**.x"));
        assert!(!TopicGlob::is_valid("Twitch.*"));
    }

    #[test]
    fn uuid_ids_round_trip() {
        let id: ActionId = "018F7A8C-3E6B-7C1D-9A2B-5F4E3D2C1B0A".parse().unwrap();
        assert_eq!(
            serde_json::to_string(&id).unwrap(),
            "\"018f7a8c-3e6b-7c1d-9a2b-5f4e3d2c1b0a\""
        );
        assert!(QueueId::DEFAULT.is_nil());
        assert!(!RunId::new_v7().is_nil());
    }

    #[test]
    fn invalid_string_is_rejected_on_deserialize() {
        let err = serde_json::from_str::<PluginId>("\"nope\"").unwrap_err();
        assert!(err.to_string().contains("plugin ID"));
    }
}
