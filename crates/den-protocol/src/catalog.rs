//! The operation and event catalog (Spec 02 §5.2, §8.1, §9.1).
//!
//! Every callable thing is an operation registered once; every known event type has a payload schema
//! registered once. Registration is explicit (`fn register(&mut Catalog)`), never link-time collection.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use crate::domain::FilterOp;
use crate::ids::{EventType, OperationName, Scope};
use crate::shared::{Os, RiskTier};
use crate::version::is_protocol_method;

/// A user-facing string. In plugin manifests it may hold `%key%` placeholders resolved from the plugin's
/// localisation files (§11); these are never templates.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize, JsonSchema, TS)]
#[serde(transparent)]
pub struct L10nString(pub String);

impl L10nString {
    /// Wraps a string.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// The string form.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for L10nString {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl fmt::Display for L10nString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// One operation (§8.1), exposed on up to three surfaces from this one definition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct OperationDef {
    /// `obs.set_scene`, `actions.run`, `acme.send_chat`.
    pub name: OperationName,
    /// Short title.
    pub title: L10nString,
    /// Description. For MCP the first line holds category and search keywords (§9.6).
    pub description: L10nString,
    /// Category path for pickers, e.g. `["OBS", "Scenes"]`.
    #[serde(default)]
    pub category: Vec<String>,
    /// Author params schema (JSON Schema 2020-12). The root MUST be `type: object` (§9.6). Codegen adds the
    /// reserved `dryRun` and `approvalId` params and derives `paramsAuthoring` from it (§10.1).
    #[ts(as = "serde_json::Value")]
    pub params: Schema,
    /// Normal result schema. The root MUST be `type: object` for tier ≥ 1; tier 0 may use any root (§8.1).
    #[ts(as = "serde_json::Value")]
    pub result: Schema,
    /// Risk tier (§8.1).
    pub tier: RiskTier,
    /// Scopes the caller needs (§8.7).
    #[serde(default)]
    pub scopes: Vec<Scope>,
    /// Which surfaces expose it.
    pub expose: Exposure,
    /// Repeating the call has no further effect (MCP `idempotentHint`).
    #[serde(default)]
    pub idempotent: bool,
    /// Which run args the operation can write, for the binding hash (§8.8).
    #[serde(default)]
    pub writes_args: ArgWrites,
    /// Talks to an external platform or app (MCP `openWorldHint`).
    #[serde(default)]
    pub external: bool,
    /// Deadline in milliseconds; absent means the engine default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub timeout_ms: Option<u32>,
    /// OS gate: `["windows"]` for Windows-only apps; hidden elsewhere with a reason (§14).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub platforms: Option<Vec<Os>>,
    /// The result is large or frequent; clients should not log it by default.
    #[serde(default)]
    pub high_volume_result: bool,
    /// The `protocolVersion` that introduced it.
    pub since: String,
    /// Deprecation notice (§7.4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub deprecated: Option<Deprecation>,
    /// At least one example; CI validates every example against the schemas. Tier ≥ 1 operations need a
    /// normal and a dry-run example, tier-2 configuration operations also a draft example (§8.1). In
    /// `protocol.json` every `x-secret` position of an example is published as `"***"` (§3), whatever
    /// its schema says; validation happens on the authored values before that.
    #[serde(default)]
    pub examples: Vec<OperationExample>,
}

/// Which surfaces expose an operation (§8.1). MCP exposure is further filtered by tier (§9.6).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, JsonSchema, TS,
)]
#[serde(rename_all = "camelCase")]
pub struct Exposure {
    /// Callable as a WS/REST method.
    pub rpc: bool,
    /// Usable as a sub-action (`Node::Op`).
    pub sub_action: bool,
    /// Offered as an MCP tool (only when `tier ≤ 3`, §9.6).
    pub mcp: bool,
}

/// Which run args an operation can write (§8.1, §8.8). Tagged by `kind`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize, JsonSchema, TS)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum ArgWrites {
    /// Writes no args (its result may still be stored through `saveAs`).
    #[default]
    None,
    /// Writes the one arg whose name is the literal value at `nameParam` (*Set argument*).
    Named {
        /// Params path (as in `fieldErrors.path`) holding the arg name.
        name_param: String,
    },
    /// Can write any arg (*Split*, *Parse JSON*, …).
    Any,
}

/// Deprecation notice (§7.4). Deprecated items stay for at least two minor releases and six months.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct Deprecation {
    /// The `protocolVersion` that deprecated it.
    pub since: String,
    /// The `protocolVersion` that may remove it.
    pub remove_in: String,
    /// What to use instead.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub replacement: Option<String>,
}

/// An operation example (§8.1). Examples are tests: CI validates `params` against the params schema and
/// `result` against the schema `shape` selects. Built from the operation's Rust types (`typed`) whenever
/// a position holds a secret: the published `params` and `result` are then serialized with every
/// `Secret` withheld, and the authored values the validator checks are kept aside, never serialized (§3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct OperationExample {
    /// Short title.
    pub title: String,
    /// The params sent, as published. For a dry-run example they include `dryRun: true`.
    #[ts(as = "serde_json::Value")]
    pub params: Value,
    /// Which of the three result shapes `result` is.
    pub shape: ResultShape,
    /// The result returned, as published.
    #[ts(as = "serde_json::Value")]
    pub result: Value,
    /// The authored values with their secrets exposed, when the example was built from typed values:
    /// what the validator checks. Never serialized, never published; dropped at the publication
    /// boundary ([`crate::codegen::ProtocolDocument::redact_examples`]).
    #[serde(skip)]
    #[ts(skip)]
    pub authored: Option<Box<AuthoredExample>>,
}

/// The authored `params` and `result` of a typed [`OperationExample`], secrets exposed. `Debug` shows
/// neither (AGENTS.md: secrets never reach logs).
#[derive(Clone, PartialEq)]
pub struct AuthoredExample {
    /// The params as authored.
    pub params: Value,
    /// The result as authored.
    pub result: Value,
}

impl fmt::Debug for AuthoredExample {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuthoredExample")
            .field("params", &"<exposed>")
            .field("result", &"<exposed>")
            .finish()
    }
}

impl OperationExample {
    /// A JSON-authored example, `params` and `result` as written. Any secret in them is found by the
    /// schema walk alone at publication; prefer [`OperationExample::typed`] for an operation whose
    /// schemas mark a position.
    pub fn json(
        title: impl Into<String>,
        params: Value,
        shape: ResultShape,
        result: Value,
    ) -> Self {
        Self {
            title: title.into(),
            params,
            shape,
            result,
            authored: None,
        }
    }

    /// An example from the operation's typed values. `params` and `result` are serialized twice: with
    /// secrets withheld for the published form, with them exposed for the authored form the validator
    /// checks. A dry-run example whose params lack `dryRun` gets `dryRun: true` added to both forms.
    ///
    /// # Panics
    /// When a value does not serialize to JSON (a map with non-string keys): examples are constants.
    pub fn typed<P: Serialize + ?Sized, R: Serialize + ?Sized>(
        title: impl Into<String>,
        params: &P,
        shape: ResultShape,
        result: &R,
    ) -> Self {
        let pair = || {
            (
                serde_json::to_value(params).expect("example params serialize"),
                serde_json::to_value(result).expect("example result serialize"),
            )
        };
        let (mut params, result) = crate::secret::withhold_secrets(pair);
        let (mut authored_params, authored_result) = crate::secret::expose_secrets(pair);
        if shape == ResultShape::DryRun {
            for p in [&mut params, &mut authored_params] {
                if let Some(obj) = p.as_object_mut() {
                    obj.entry("dryRun").or_insert(Value::Bool(true));
                }
            }
        }
        Self {
            title: title.into(),
            params,
            shape,
            result,
            authored: Some(Box::new(AuthoredExample {
                params: authored_params,
                result: authored_result,
            })),
        }
    }

    /// The params the validator checks: the authored ones for a typed example, else `params`.
    pub fn authored_params(&self) -> &Value {
        self.authored.as_ref().map_or(&self.params, |a| &a.params)
    }

    /// The result the validator checks: the authored one for a typed example, else `result`.
    pub fn authored_result(&self) -> &Value {
        self.authored.as_ref().map_or(&self.result, |a| &a.result)
    }
}

/// The three result shapes of §8.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum ResultShape {
    /// The normal result: the effect happened.
    Normal,
    /// A [`crate::DryRunResult`]: nothing executed.
    DryRun,
    /// A [`crate::DraftResult`]: deferred into a draft.
    Draft,
}

/// A known event type (§5.2): its payload schema, trigger filters and flags. `Debug` shows the authored
/// payloads only by count (AGENTS.md: secrets never reach logs).
#[derive(Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct EventDef {
    /// The event type, `<slug>.<name>`.
    #[serde(rename = "type")]
    pub r#type: EventType,
    /// Short title.
    pub title: L10nString,
    /// Description.
    pub description: L10nString,
    /// Category path for pickers, e.g. `["Twitch", "Subscriptions"]`.
    #[serde(default)]
    pub category: Vec<String>,
    /// Current payload schema version, starting at 1 (§5.2).
    pub payload_version: u32,
    /// Payload schema (JSON Schema 2020-12). The root MUST be `type: object`, and no top-level property may
    /// use a reserved run-context root name (§6.3).
    #[ts(as = "serde_json::Value")]
    pub payload: Schema,
    /// Payload fields a trigger can filter on (§6.2).
    #[serde(default)]
    pub filters: Vec<FilterableField>,
    /// Delivered only when a session lists the type in `highVolume` (§8.4).
    #[serde(default)]
    pub high_volume: bool,
    /// Never dropped by the slow-consumer policy (§8.4).
    #[serde(default)]
    pub critical: bool,
    /// At least one example payload; the first is also the "Test trigger" fixture. Published with every
    /// `x-secret` position redacted to `"***"` (§3); an example built from the typed payload is published
    /// with its secrets withheld from the start.
    #[serde(default)]
    #[ts(as = "Vec<serde_json::Value>")]
    pub examples: Vec<Value>,
    /// Aligned with `examples` index by index once any example was built from a typed value (empty
    /// otherwise): `Some` holds the authored payload with its secrets exposed, what the validator
    /// checks; `None` marks a JSON-authored example. Never serialized, never published; cleared at the
    /// publication boundary.
    #[serde(skip)]
    #[ts(skip)]
    pub authored_examples: Vec<Option<Value>>,
}

impl fmt::Debug for EventDef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EventDef")
            .field("type", &self.r#type)
            .field("title", &self.title)
            .field("description", &self.description)
            .field("category", &self.category)
            .field("payload_version", &self.payload_version)
            .field("payload", &self.payload)
            .field("filters", &self.filters)
            .field("high_volume", &self.high_volume)
            .field("critical", &self.critical)
            .field("examples", &self.examples)
            .field(
                "authored_examples",
                &format_args!(
                    "<{} exposed>",
                    self.authored_examples.iter().flatten().count()
                ),
            )
            .finish()
    }
}

impl EventDef {
    /// Pushes an example from the typed payload: serialized with secrets withheld into `examples` and
    /// with them exposed into `authored_examples` (JSON-authored examples already present are marked
    /// `None` there).
    ///
    /// # Panics
    /// When the payload does not serialize to JSON: examples are constants.
    pub fn push_typed_example<P: Serialize + ?Sized>(&mut self, payload: &P) {
        let to_value = || serde_json::to_value(payload).expect("example payload serializes");
        self.authored_examples.resize(self.examples.len(), None);
        self.examples
            .push(crate::secret::withhold_secrets(to_value));
        self.authored_examples
            .push(Some(crate::secret::expose_secrets(to_value)));
    }

    /// Whether example `i` was built from a typed value.
    pub fn is_typed_example(&self, i: usize) -> bool {
        self.authored_examples.get(i).is_some_and(Option::is_some)
    }

    /// The payload the validator checks for example `i`: the authored one when typed, else the example.
    pub fn authored_example(&self, i: usize) -> &Value {
        self.authored_examples
            .get(i)
            .and_then(Option::as_ref)
            .unwrap_or(&self.examples[i])
    }
}

/// A payload field a trigger can filter on (§6.2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct FilterableField {
    /// Dotted path into the payload.
    pub field: String,
    /// Allowed operators.
    pub ops: Vec<FilterOp>,
    /// An operation name whose result feeds the value picker, when the field has known values.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub options_from: Option<String>,
    /// Rank used by spec 03's "specific beats any" precedence; higher is more specific.
    #[serde(default)]
    pub specificity: u32,
}

/// Why a registration was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatalogError {
    /// The operation name is already registered.
    DuplicateOperation(String),
    /// The event type is already registered.
    DuplicateEvent(String),
    /// The name is a protocol method (§12), which is never a catalog operation.
    ProtocolMethod(String),
}

impl fmt::Display for CatalogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateOperation(n) => write!(f, "operation {n:?} is already registered"),
            Self::DuplicateEvent(n) => write!(f, "event type {n:?} is already registered"),
            Self::ProtocolMethod(n) => write!(
                f,
                "{n:?} is a protocol method and cannot be a catalog operation"
            ),
        }
    }
}

impl std::error::Error for CatalogError {}

/// The catalog: operations, events and the shared types their schemas reference (§8.1, §9.1).
///
/// Schemas produced through [`Catalog::schema_for`] share one definitions table, so every `$ref` in the
/// generated `protocol.json` points at `#/types/<Name>`.
pub struct Catalog {
    generator: SchemaGenerator,
    operations: BTreeMap<OperationName, OperationDef>,
    events: BTreeMap<EventType, EventDef>,
    types: BTreeSet<String>,
}

impl Default for Catalog {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for Catalog {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Catalog")
            .field("operations", &self.operations.keys().collect::<Vec<_>>())
            .field("events", &self.events.keys().collect::<Vec<_>>())
            .field("types", &self.types)
            .finish()
    }
}

impl Catalog {
    /// An empty catalog with the protocol's schema settings (§9.1).
    pub fn new() -> Self {
        Self {
            generator: crate::schema::protocol_generator(),
            operations: BTreeMap::new(),
            events: BTreeMap::new(),
            types: BTreeSet::new(),
        }
    }

    /// The core catalog: everything Spec 02 itself defines, registered through [`crate::core::register`].
    pub fn core() -> Self {
        let mut catalog = Self::new();
        crate::core::register(&mut catalog).expect("core catalog registers without conflicts");
        catalog
    }

    /// The schema of `T` itself (an inline object for a struct), with nested `$ref`s pointing into this
    /// catalog's shared types. Use it for operation params and results and for event payloads, whose
    /// roots must be `type: object` (§8.1, §9.6).
    pub fn schema_for<T: JsonSchema + ?Sized>(&mut self) -> Schema {
        T::json_schema(&mut self.generator)
    }

    /// A `$ref` to `T` in this catalog's shared types (or `T`'s schema when `T` is always inlined).
    pub fn ref_for<T: JsonSchema + ?Sized>(&mut self) -> Schema {
        self.generator.subschema_for::<T>()
    }

    /// Registers `T` as a shared type published under `types.<Name>` even when no operation references it.
    pub fn register_type<T: JsonSchema + ?Sized>(&mut self) {
        let name = T::schema_name().into_owned();
        if !T::inline_schema() {
            self.generator.subschema_for::<T>();
        }
        self.types.insert(name);
    }

    /// Registers an operation (§8.1).
    pub fn register_operation(&mut self, def: OperationDef) -> Result<(), CatalogError> {
        if is_protocol_method(def.name.as_str()) {
            return Err(CatalogError::ProtocolMethod(def.name.to_string()));
        }
        if self.operations.contains_key(&def.name) {
            return Err(CatalogError::DuplicateOperation(def.name.to_string()));
        }
        self.operations.insert(def.name.clone(), def);
        Ok(())
    }

    /// Registers an event type (§5.2).
    pub fn register_event(&mut self, def: EventDef) -> Result<(), CatalogError> {
        if self.events.contains_key(&def.r#type) {
            return Err(CatalogError::DuplicateEvent(def.r#type.to_string()));
        }
        self.events.insert(def.r#type.clone(), def);
        Ok(())
    }

    /// The registered operations, by name.
    pub fn operations(&self) -> &BTreeMap<OperationName, OperationDef> {
        &self.operations
    }

    /// The registered event types, by type.
    pub fn events(&self) -> &BTreeMap<EventType, EventDef> {
        &self.events
    }

    /// One operation.
    pub fn operation(&self, name: &str) -> Option<&OperationDef> {
        OperationName::new(name)
            .ok()
            .and_then(|n| self.operations.get(&n))
    }

    /// One event type.
    pub fn event(&self, r#type: &str) -> Option<&EventDef> {
        EventType::new(r#type)
            .ok()
            .and_then(|t| self.events.get(&t))
    }

    /// Names of the explicitly registered shared types.
    pub fn type_names(&self) -> &BTreeSet<String> {
        &self.types
    }

    /// Applies the generator's transforms to `schema` (a subschema obtained from [`Catalog::schema_for`]).
    pub fn finish_schema(&mut self, schema: &mut Schema) {
        for transform in self.generator.transforms_mut() {
            transform.transform(schema);
        }
    }

    /// The shared definitions table as generated so far, without transforms (for lint lookups).
    pub fn peek_definitions(&self) -> serde_json::Map<String, serde_json::Value> {
        self.generator.definitions().clone()
    }

    /// Takes the shared definitions table, transforms applied. The catalog's generator is left empty, so
    /// call this last.
    pub fn take_definitions(&mut self) -> serde_json::Map<String, serde_json::Value> {
        self.generator.take_definitions(true)
    }

    /// Whether `def` passes the MCP exposure predicate `expose.mcp && tier ≤ 3` (§9.6).
    pub fn is_mcp_exposed(def: &OperationDef) -> bool {
        def.expose.mcp && def.tier <= RiskTier::EXTERNAL
    }
}
