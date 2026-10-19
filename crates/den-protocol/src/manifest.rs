//! Plugin manifest schema v0 (Spec 02 §11): `plugin.json`, validated strictly against
//! `schema/plugin.schema.json` (generated from these types).
//!
//! Keys the spec defers to spec 12 (`activation`, `permissions`, `connection`, `settings`, the
//! contribution kinds other than events and operations, `l10n`) are carried as opaque JSON in v0 and
//! typed when spec 12 fixes them.

use std::collections::BTreeMap;
use std::fmt;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::catalog::{Exposure, FilterableField, L10nString, OperationExample};
use crate::ids::{LocalId, PluginId, Scope, Slug};
use crate::shared::{Os, RiskTier};
use crate::version::{RESERVED_SLUGS, is_reserved_slug};

/// `plugin.json` (§11). Required: `id`, `slug`, `name`, `version`, `license`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct PluginManifest {
    /// URL of the schema this manifest follows; the app always validates against its bundled copy.
    #[serde(rename = "$schema", default, skip_serializing_if = "Option::is_none")]
    #[ts(rename = "$schema", optional)]
    pub schema: Option<String>,
    /// Always `0` for this schema. v0 is pre-stable.
    pub manifest_version: u32,
    /// Reverse-DNS plugin ID (§4).
    pub id: PluginId,
    /// Short slug: the prefix of every contribution (§4). A reserved slug fails manifest validation (the
    /// schema and [`PluginManifest::validate`] alike) and is refused at install.
    #[schemars(schema_with = "plugin_slug_schema")]
    pub slug: Slug,
    /// Display name.
    pub name: L10nString,
    /// SemVer version.
    pub version: String,
    /// Short description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub description: Option<L10nString>,
    /// The author.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub author: Option<Author>,
    /// SPDX license expression.
    pub license: String,
    /// Source repository URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub repository: Option<String>,
    /// Homepage URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub homepage: Option<String>,
    /// Path of a monochrome SVG icon inside the package.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub icon: Option<String>,
    /// Marketplace categories.
    #[serde(default)]
    pub categories: Vec<String>,
    /// Search keywords.
    #[serde(default)]
    pub keywords: Vec<String>,
    /// Host version requirements.
    pub engines: Engines,
    /// How the plugin runs.
    pub runtime: Runtime,
    /// Activation events (B2); fixed in spec 12.
    #[serde(default)]
    pub activation: Vec<String>,
    /// Permissions (B2); fixed in spec 12. Carried as opaque JSON in v0.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional, as = "Option<serde_json::Value>")]
    pub permissions: Option<serde_json::Value>,
    /// Connection definition (B2); fixed in spec 12. Carried as opaque JSON in v0.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional, as = "Option<serde_json::Value>")]
    pub connection: Option<serde_json::Value>,
    /// Plugin-wide settings: a JSON Schema with `x-ui` hints and `x-secret` (B2.4). Always literal (§10.1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional, as = "Option<serde_json::Value>")]
    pub settings: Option<Schema>,
    /// What the plugin contributes.
    #[serde(default)]
    pub contributes: Contributes,
    /// Local JSON Schema definitions, referenced as `#/definitions/<Name>`.
    #[serde(default)]
    #[ts(as = "BTreeMap<String, serde_json::Value>")]
    pub definitions: BTreeMap<String, Schema>,
    /// Localisation configuration for `%key%` placeholders; fixed in spec 12. Carried as opaque JSON in v0.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional, as = "Option<serde_json::Value>")]
    pub l10n: Option<serde_json::Value>,
}

/// The schema of `PluginManifest.slug` (§4): a [`Slug`] that is not reserved for core.
fn plugin_slug_schema(generator: &mut SchemaGenerator) -> Schema {
    let slug = generator.subschema_for::<Slug>();
    json_schema!({
        "allOf": [slug],
        "not": { "enum": RESERVED_SLUGS }
    })
}

/// Why a manifest is refused by the install-time validator (§4, §11) beyond what the schema says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestError {
    /// `slug` is reserved for core (§4).
    ReservedSlug(String),
}

impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReservedSlug(slug) => write!(
                f,
                "plugin slug {slug:?} is reserved for core and cannot be claimed by a plugin (§4)"
            ),
        }
    }
}

impl std::error::Error for ManifestError {}

impl PluginManifest {
    /// The install-time checks of §4 that the typed shape alone does not enforce: the slug is not one
    /// reserved for core. Run after schema validation, before anything the manifest declares is registered.
    pub fn validate(&self) -> Result<(), ManifestError> {
        if is_reserved_slug(self.slug.as_str()) {
            return Err(ManifestError::ReservedSlug(self.slug.to_string()));
        }
        Ok(())
    }
}

/// The plugin author (§11, B2).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct Author {
    /// Name.
    pub name: String,
    /// Homepage or profile URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub url: Option<String>,
    /// Contact email.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub email: Option<String>,
}

/// Host version requirements (§11).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct Engines {
    /// App version range (SemVer range), e.g. `>=0.1.0 <1.0.0`.
    pub streamden: String,
    /// Plugin API range (SemVer range), e.g. `^0.1`.
    pub plugin_api: String,
}

/// How a plugin runs (§11), tagged by `kind`. `wasm` is reserved and rejected in v0.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, TS)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum Runtime {
    /// No code: HTTP/WS requests described in the manifest.
    Declarative,
    /// JavaScript in the host's extension process.
    Js {
        /// Entry module, relative to the package root.
        entry: String,
        /// `process` (shared extension host, default) or `dedicated` (own process).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        isolation: Option<String>,
    },
    /// A separate process speaking JSON-RPC over stdio (§8.6).
    Sidecar {
        /// Executable per OS (`windows`, `macos`, `linux`), relative to the package root.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        command: Option<BTreeMap<String, String>>,
        /// An interpreter requirement such as `python>=3.12`, when the sidecar is a script.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        interpreter: Option<String>,
        /// The script entry point, when `interpreter` is set.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        entry: Option<String>,
        /// A lock file of the script's dependencies, when `interpreter` is set.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        requirements: Option<String>,
    },
}

/// What a plugin contributes (§11). Kinds other than events and operations are carried as opaque JSON
/// until spec 12 fixes their schemas.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct Contributes {
    /// Event types (also trigger types, §6.2). Each becomes `<slug>.<id>`.
    #[serde(default)]
    pub events: Vec<EventContribution>,
    /// Operations (sub-actions, methods and MCP tools in one, §8.1). Each becomes `<slug>.<id>`.
    #[serde(default)]
    pub operations: Vec<OperationContribution>,
    /// Conditions usable in `If` nodes (spec 12).
    #[serde(default)]
    #[ts(as = "Vec<serde_json::Value>")]
    pub conditions: Vec<serde_json::Value>,
    /// Variables the plugin maintains (spec 12).
    #[serde(default)]
    #[ts(as = "Vec<serde_json::Value>")]
    pub variables: Vec<serde_json::Value>,
    /// Multichat sources (spec 08/12).
    #[serde(default)]
    #[ts(as = "Vec<serde_json::Value>")]
    pub chat_sources: Vec<serde_json::Value>,
    /// TTS voice providers (spec 10/12).
    #[serde(default)]
    #[ts(as = "Vec<serde_json::Value>")]
    pub tts_voices: Vec<serde_json::Value>,
    /// Overlay widgets (spec 09/12).
    #[serde(default)]
    #[ts(as = "Vec<serde_json::Value>")]
    pub overlay_widgets: Vec<serde_json::Value>,
    /// UI panels (spec 07/12).
    #[serde(default)]
    #[ts(as = "Vec<serde_json::Value>")]
    pub panels: Vec<serde_json::Value>,
    /// Chat commands (spec 12).
    #[serde(default)]
    #[ts(as = "Vec<serde_json::Value>")]
    pub commands: Vec<serde_json::Value>,
    /// Action templates (spec 12).
    #[serde(default)]
    #[ts(as = "Vec<serde_json::Value>")]
    pub templates: Vec<serde_json::Value>,
}

/// An event type a plugin contributes (§11). Becomes event type `<slug>.<id>`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct EventContribution {
    /// Local ID, `lower_snake_case`.
    pub id: LocalId,
    /// Short title.
    pub title: L10nString,
    /// Description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub description: Option<L10nString>,
    /// Category path for pickers.
    #[serde(default)]
    pub category: Vec<String>,
    /// Payload schema version, starting at 1. A breaking change bumps it and the plugin's major version (§5.2).
    pub payload_version: u32,
    /// Payload schema (JSON Schema 2020-12, object root). Shared types are referenced as `den://schemas/v0/<Name>.json`.
    #[ts(as = "serde_json::Value")]
    pub payload: Schema,
    /// Payload fields a trigger can filter on.
    #[serde(default)]
    pub filters: Vec<FilterableField>,
    /// Delivered only when a session lists the type in `highVolume` (§8.4).
    #[serde(default)]
    pub high_volume: bool,
    /// Path of a JSON fixture inside the package used by "Test trigger" and the simulator.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub test_payload: Option<String>,
}

/// An operation a plugin contributes (§11). Becomes method `<slug>.<id>`. A declared tier lower than the
/// host's classification of the permissions it uses is raised.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct OperationContribution {
    /// Local ID, `lower_snake_case`.
    pub id: LocalId,
    /// Short title.
    pub title: L10nString,
    /// Sentence shown in the sub-action list, with `{param}` placeholders: `Send {message} to Acme chat`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub sentence: Option<L10nString>,
    /// Description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub description: Option<L10nString>,
    /// Category path for pickers.
    #[serde(default)]
    pub category: Vec<String>,
    /// Author params schema (object root). Leaf fields may carry `x-den-template` (§10.1).
    #[ts(as = "serde_json::Value")]
    pub params: Schema,
    /// Result schema. Object root for tier ≥ 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional, as = "Option<serde_json::Value>")]
    pub result: Option<Schema>,
    /// Declared risk tier (§8.1); the host may raise it.
    pub tier: RiskTier,
    /// Scopes the caller needs.
    #[serde(default)]
    pub scopes: Vec<Scope>,
    /// Which surfaces expose it.
    pub expose: Exposure,
    /// Deadline in milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub timeout_ms: Option<u32>,
    /// OS gate (§14).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub platforms: Option<Vec<Os>>,
    /// Repeating the call has no further effect.
    #[serde(default)]
    pub idempotent: bool,
    /// Talks to an external platform or app.
    #[serde(default)]
    pub external: bool,
    /// Examples (§8.1).
    #[serde(default)]
    pub examples: Vec<OperationExample>,
}
