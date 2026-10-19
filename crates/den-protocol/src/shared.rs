//! Shared core types referenced by every connector and plugin (Spec 02 §5.3), plus the small value
//! types of §3 and §4.

use std::borrow::Cow;
use std::fmt;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::ids::PluginId;
use crate::version::is_reserved_slug;

/// A string that carries viewer- or third-party-controlled text (chat text, usernames, titles, plugin
/// output). Marked `x-den-untrusted: true` in schemas; MCP and the AI layer wrap such values (§3, §9.6)
/// and untrusted text never satisfies an approval (§16).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize, JsonSchema, TS)]
#[serde(transparent)]
#[schemars(extend("x-den-untrusted" = true))]
pub struct UntrustedString(pub String);

impl UntrustedString {
    /// Wraps a string.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// The string form.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for UntrustedString {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl From<String> for UntrustedString {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl fmt::Display for UntrustedString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Risk tier of an operation (§8.1): 0 read · 1 reversible write · 2 configuration change ·
/// 3 external or irreversible · 4 dangerous. Serialized as an integer 0–4.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize, TS,
)]
#[serde(try_from = "u8", into = "u8")]
#[ts(type = "number")]
pub struct RiskTier(u8);

impl RiskTier {
    /// Tier 0: read-only.
    pub const READ: Self = Self(0);
    /// Tier 1: reversible runtime write (`variables.set`, `queues.pause`).
    pub const REVERSIBLE_WRITE: Self = Self(1);
    /// Tier 2: configuration change that writes a persisted document (draft-first under MCP, §6.1).
    pub const CONFIG: Self = Self(2);
    /// Tier 3: external or irreversible effect; needs in-app approval under MCP (§8.8).
    pub const EXTERNAL: Self = Self(3);
    /// Tier 4: dangerous (code execution, ending the stream); never exposed over MCP (D-030).
    pub const DANGEROUS: Self = Self(4);

    /// Builds a tier, rejecting values above 4.
    pub const fn new(value: u8) -> Option<Self> {
        if value <= 4 { Some(Self(value)) } else { None }
    }

    /// The integer value.
    pub const fn get(self) -> u8 {
        self.0
    }
}

impl TryFrom<u8> for RiskTier {
    type Error = String;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Self::new(value).ok_or_else(|| format!("risk tier must be 0–4, got {value}"))
    }
}

impl From<RiskTier> for u8 {
    fn from(value: RiskTier) -> Self {
        value.0
    }
}

impl fmt::Display for RiskTier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl JsonSchema for RiskTier {
    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("RiskTier")
    }

    fn schema_id() -> Cow<'static, str> {
        Cow::Borrowed("den_protocol::shared::RiskTier")
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "integer",
            "minimum": 0,
            "maximum": 4,
            "description": "Risk tier (Spec 02 §8.1): 0 read, 1 reversible write, 2 configuration change, 3 external or irreversible, 4 dangerous."
        })
    }
}

/// Operating systems an operation can be gated to (§8.1, §14).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum Os {
    /// Windows.
    Windows,
    /// Linux.
    Linux,
    /// macOS.
    Macos,
}

/// A normalised reference to the actor of an event (§5.3). `login` and `displayName` are untrusted text.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct UserRef {
    /// The platform the user belongs to (open enum).
    pub platform: crate::ids::PlatformId,
    /// The platform's stable user ID.
    pub id: String,
    /// The login name (untrusted).
    pub login: UntrustedString,
    /// The display name (untrusted).
    pub display_name: UntrustedString,
    /// Avatar image URL, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub avatar_url: Option<String>,
    /// The user's chat colour as a CSS colour string, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub color: Option<String>,
    /// Roles on the platform: `broadcaster`, `moderator`, `vip`, `subscriber`, `founder`, `member`… (open).
    #[serde(default)]
    pub roles: Vec<String>,
    /// Badges the platform shows for this user.
    #[serde(default)]
    pub badges: Vec<Badge>,
    /// Pronouns, when the platform or a pronoun service provides them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub pronouns: Option<String>,
}

/// A platform badge shown next to a user (§5.3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct Badge {
    /// The badge set ID on the platform (`subscriber`, `moderator`).
    pub id: String,
    /// The badge version within the set (`12` for a 12-month subscriber badge), when the platform has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub version: Option<String>,
    /// Human-readable title, when known (untrusted on third-party platforms).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub title: Option<UntrustedString>,
    /// The badge image, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub image: Option<ImageRef>,
}

/// Chat text with its parsed fragments (§5.3). `text` is the plain-text rendering and is untrusted.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct ChatText {
    /// The plain text (untrusted).
    pub text: UntrustedString,
    /// The message split into typed fragments, in order. Concatenating their `text` gives `text`.
    #[serde(default)]
    pub fragments: Vec<Fragment>,
}

/// One fragment of a chat message, tagged by `kind` (§5.3). The union is extensible: a `kind` this build
/// does not know decodes as [`Fragment::Unknown`], which keeps the raw object and exposes its `text`, so
/// a newer server never breaks an older client's chat rendering (§7.2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum Fragment {
    /// Plain text.
    Text {
        /// The text (untrusted).
        text: UntrustedString,
    },
    /// An emote.
    Emote {
        /// The emote code as typed (untrusted).
        text: UntrustedString,
        /// The platform's emote ID.
        id: String,
        /// The emote provider (`twitch`, `7tv`, `bttv`…), when not the platform itself.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        provider: Option<String>,
        /// The emote image, when known.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        image: Option<ImageRef>,
    },
    /// A mention of another user.
    Mention {
        /// The mention as typed, including any `@` (untrusted).
        text: UntrustedString,
        /// The mentioned user's platform ID, when resolved.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        user_id: Option<String>,
        /// The mentioned user's login, when resolved (untrusted).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        login: Option<UntrustedString>,
    },
    /// A link.
    Link {
        /// The link text as typed (untrusted).
        text: UntrustedString,
        /// The URL (untrusted).
        url: UntrustedString,
    },
    /// A cheermote (Twitch bits).
    Cheermote {
        /// The cheermote as typed, e.g. `Cheer100` (untrusted).
        text: UntrustedString,
        /// The cheermote prefix (`Cheer`).
        prefix: String,
        /// Bits in this cheermote.
        bits: u32,
        /// The cheermote tier the amount falls into.
        tier: u32,
    },
    /// Receiving fallback for a `kind` this build does not know (§7.2): rendered as its `text`. Never
    /// produced by this build's connectors; serialises back to the raw object it was decoded from.
    #[serde(untagged)]
    #[ts(untagged)]
    Unknown(UnknownFragment),
}

impl Fragment {
    /// The `kind` tags this build knows. The schema of [`Fragment::Unknown`] excludes them.
    pub const KNOWN_KINDS: &'static [&'static str] =
        &["text", "emote", "mention", "link", "cheermote"];

    /// The `kind` tag of this fragment on the wire.
    pub fn kind(&self) -> &str {
        match self {
            Self::Text { .. } => "text",
            Self::Emote { .. } => "emote",
            Self::Mention { .. } => "mention",
            Self::Link { .. } => "link",
            Self::Cheermote { .. } => "cheermote",
            Self::Unknown(u) => &u.kind,
        }
    }

    /// The fragment's plain text, which every kind carries (untrusted).
    pub fn text(&self) -> &UntrustedString {
        match self {
            Self::Text { text }
            | Self::Emote { text, .. }
            | Self::Mention { text, .. }
            | Self::Link { text, .. }
            | Self::Cheermote { text, .. } => text,
            Self::Unknown(u) => &u.text,
        }
    }
}

/// A fragment whose `kind` this build does not know (§7.2). Every fragment kind carries `text`, so a
/// client can always render it; the other members are kept verbatim.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct UnknownFragment {
    /// The unknown `kind` tag. Never one of the kinds this build knows: decoding refuses a known kind
    /// here, so a malformed known fragment is an error and never falls through to this variant.
    #[serde(deserialize_with = "unknown_fragment_kind")]
    #[schemars(schema_with = "unknown_fragment_kind_schema")]
    pub kind: String,
    /// The plain text (untrusted).
    pub text: UntrustedString,
    /// The remaining members, untouched.
    #[serde(flatten)]
    #[ts(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

fn unknown_fragment_kind_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    crate::schema::unknown_tag_schema(Fragment::KNOWN_KINDS)
}

fn unknown_fragment_kind<'de, D: serde::Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    crate::schema::deserialize_unknown_tag(d, Fragment::KNOWN_KINDS)
}

/// An amount of money or a platform unit (§3). Floats are never used for money.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct Money {
    /// Amount in minor units (cents, bits).
    pub amount_minor: i64,
    /// ISO 4217 code (`USD`) or a platform unit such as `bits`.
    pub currency: String,
    /// Number of decimal places between minor and major units (2 for USD, 0 for bits).
    pub exponent: u8,
}

/// A reference to an image (§5.3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct ImageRef {
    /// The image URL.
    pub url: String,
    /// Intrinsic width in pixels, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub width: Option<u32>,
    /// Intrinsic height in pixels, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub height: Option<u32>,
    /// Device pixel ratio the image is meant for (1, 2, 3…), when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub scale: Option<f32>,
}

/// A persisted reference to a contribution (an event type, trigger type or operation), §4 and §5.3.
/// Flattened into [`crate::Trigger`] and [`crate::Node::Op`]. `plugin` is **required** when the slug of
/// `type` is not reserved for core and **forbidden** when it is; codegen adds the matching
/// `if/then/else` clause to every schema that embeds this type, and [`ContributionRef::validate`]
/// enforces the same rule in code.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct ContributionRef {
    /// The contribution ID `<slug>.<localId>` (`twitch.cheer`, `acme.gift`).
    #[serde(rename = "type")]
    #[schemars(regex(pattern = crate::ids::DOTTED_NAME_PATTERN))]
    pub r#type: String,
    /// The contributing plugin's ID. Required for a non-core slug, forbidden for a core slug.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub plugin: Option<PluginId>,
}

/// Why a [`ContributionRef`] is invalid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContributionRefError {
    /// `type` is not a dotted `lower_snake_case` name.
    MalformedType(String),
    /// The slug is reserved for core but `plugin` is set.
    PluginOnCoreSlug(String),
    /// The slug is not reserved for core but `plugin` is missing.
    MissingPlugin(String),
}

impl fmt::Display for ContributionRefError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MalformedType(t) => write!(
                f,
                "contribution type {t:?} is not a dotted lower_snake_case name"
            ),
            Self::PluginOnCoreSlug(t) => write!(
                f,
                "contribution {t:?} uses a core slug and must not set `plugin`"
            ),
            Self::MissingPlugin(t) => write!(
                f,
                "contribution {t:?} uses a plugin slug and must set `plugin`"
            ),
        }
    }
}

impl std::error::Error for ContributionRefError {}

impl ContributionRef {
    /// A reference to a core contribution.
    pub fn core(r#type: impl Into<String>) -> Self {
        Self {
            r#type: r#type.into(),
            plugin: None,
        }
    }

    /// A reference to a plugin contribution.
    pub fn plugin(r#type: impl Into<String>, plugin: PluginId) -> Self {
        Self {
            r#type: r#type.into(),
            plugin: Some(plugin),
        }
    }

    /// The slug before the first dot.
    pub fn slug(&self) -> &str {
        self.r#type.split('.').next().unwrap_or_default()
    }

    /// Returns `true` when the slug is reserved for core (§4).
    pub fn is_core(&self) -> bool {
        is_reserved_slug(self.slug())
    }

    /// Checks the §4 rule: `plugin` is required for a plugin slug and forbidden for a core slug.
    pub fn validate(&self) -> Result<(), ContributionRefError> {
        if !crate::ids::is_dotted_name(&self.r#type) {
            return Err(ContributionRefError::MalformedType(self.r#type.clone()));
        }
        match (self.is_core(), self.plugin.is_some()) {
            (true, true) => Err(ContributionRefError::PluginOnCoreSlug(self.r#type.clone())),
            (false, false) => Err(ContributionRefError::MissingPlugin(self.r#type.clone())),
            _ => Ok(()),
        }
    }
}

/// Payload of a `<slug>.unknown` event (§5.2): an upstream event the connector has no type for yet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct UnknownEventPayload {
    /// The upstream event type as the platform names it (an EventSub subscription type, for example).
    pub upstream_type: String,
    /// The upstream event version, when the platform versions its events.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub upstream_version: Option<String>,
    /// The raw upstream payload (untrusted).
    #[ts(as = "serde_json::Value")]
    pub data: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn risk_tier_bounds() {
        assert!(serde_json::from_str::<RiskTier>("4").is_ok());
        assert!(serde_json::from_str::<RiskTier>("5").is_err());
        assert_eq!(serde_json::to_string(&RiskTier::CONFIG).unwrap(), "2");
    }

    #[test]
    fn contribution_ref_rule() {
        assert!(ContributionRef::core("twitch.cheer").validate().is_ok());
        let acme = PluginId::new("io.github.alice.acmelive").unwrap();
        assert!(
            ContributionRef::plugin("acme.gift", acme.clone())
                .validate()
                .is_ok()
        );
        assert_eq!(
            ContributionRef::core("acme.gift").validate(),
            Err(ContributionRefError::MissingPlugin("acme.gift".into()))
        );
        assert_eq!(
            ContributionRef::plugin("twitch.cheer", acme).validate(),
            Err(ContributionRefError::PluginOnCoreSlug(
                "twitch.cheer".into()
            ))
        );
        assert!(ContributionRef::core("nodot").validate().is_err());
    }
}
