//! The publication invariant of Spec 02 §3, end to end: a unique sentinel planted at every position
//! the authored Rust types mark `x-secret` never reaches a published artifact, whatever this crate's
//! schema transforms do in between (optional-null normalisation, the contribution rule, reference
//! inlining, template derivation, reserved params) and however the redactor walks the result.
//!
//! The positions come from the schemas exactly as schemars generates them from the Rust types, read
//! through `Catalog::peek_definitions` and the registered definitions *before* any transform runs: the
//! authored truth, independent of the pipeline under test. The pipeline is the real one, the same
//! functions `cargo xtask codegen` calls in the same order: `build_unredacted`, then
//! `ProtocolDocument::redact_examples`, then `to_json_string`; plus `redact_secrets` against the
//! published type table for every shared type and every fixture, the TypeScript export and the plugin
//! schema. It runs over the core catalog, and over the core catalog plus a stress catalog of
//! secret-bearing shapes written as Rust types (optional, inline, flattened, tagged, listed, mapped,
//! templated, nested) plus a JSON-authored operation with the round-4 collision shape. Every value here
//! is synthetic.
//!
//! Reach of the planter, deliberately the redactor's own conjunction: `$ref` chains, `allOf`, one
//! branch at a time of `anyOf`/`oneOf`, `if`, `then`, `else` and every `dependentSchemas` entry (all
//! taken as applying, the way `codegen::redact_secrets` unions them), `properties`, selecting
//! `patternProperties`, `additionalProperties`/`unevaluatedProperties` for undeclared members,
//! `prefixItems`, `items`/`unevaluatedItems` and `contains` for items. Not planted: marks under `not`
//! and `propertyNames` (an instance satisfying them is not a valid one; `redaction.rs` covers the
//! redactor side), and alternatives past `MAX_ALTERNATIVES` per position (unions are sampled, not
//! exhausted, past that). schemars emits none of those from Rust types.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use den_protocol::catalog::{
    ArgWrites, Catalog, EventDef, Exposure, OperationDef, OperationExample, ResultShape,
};
use den_protocol::codegen::{self, ProtocolDocument, redact_secrets};
use den_protocol::ids::{EventType, OperationName};
use den_protocol::shared::RiskTier;
use den_protocol::template::TemplateOr;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

/// Every sentinel starts with this; no authored string in this crate does.
const SENTINEL_PREFIX: &str = "SENTINEL";

/// Alternatives kept per position; a union with more branches than this is sampled, not exhausted.
const MAX_ALTERNATIVES: usize = 24;

/// Numeric sentinels count up from here: inside `uint32`, and no authored number comes near it.
const NUMERIC_SENTINEL_BASE: i64 = 4_000_000_000;

/// Strings tried, in order, until one satisfies every `pattern`, `minLength` and `maxLength` at a leaf.
const STRING_CANDIDATES: &[&str] = &[
    "a",
    "a.b",
    "ab",
    "a_b",
    "a.b.c",
    "sdt_a",
    "{x}",
    "0.1.0",
    "x-a",
    "a:b",
    "apr_AAAAAAAAAAAAAAAAAAAAAA",
    "ses_AAAAAAAAAAAAAAAAAAAAAA",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "2026-01-01T00:00:00.000Z",
    "00000000-0000-4000-8000-000000000000",
    "https://example.invalid/a",
    "A",
    "0",
];

/// Member names tried for a `patternProperties` entry, and (the first that no pattern selects and no
/// property declares) for `additionalProperties`.
const NAME_CANDIDATES: &[&str] = &[
    "a", "k_1", "api_key", "x-a", "a.b", "A", "extra", "zz_extra",
];

/// Everything that applies at one instance position: the schema objects in conjunction, and each
/// `anyOf`/`oneOf` met on the way, still to be chosen from.
#[derive(Default, Clone)]
struct Expanded<'b> {
    conj: Vec<&'b Map<String, Value>>,
    choices: Vec<&'b Vec<Value>>,
}

/// Generates instances from the authored schemas with a unique sentinel at every marked position, and
/// plants sentinels into existing instances (fixtures) the same way.
struct Planter<'a> {
    /// The shared types as schemars generated them, before any transform.
    types: &'a BTreeMap<String, Value>,
    /// Shared types whose instance is being generated up the stack: a reference back is a cycle.
    visiting: Vec<String>,
    /// `(position, sentinel)` for every sentinel handed out; a numeric sentinel is kept as its digits.
    planted: Vec<(String, String)>,
    /// `pattern` probes, compiled by the validator that checks the examples (ECMA-262 dialect).
    probes: BTreeMap<String, Option<jsonschema::Validator>>,
    /// Validators for choosing the branch an existing value sits in, keyed by the branch's JSON.
    branch_validators: BTreeMap<String, jsonschema::Validator>,
}

impl<'a> Planter<'a> {
    fn new(types: &'a BTreeMap<String, Value>) -> Self {
        Self {
            types,
            visiting: Vec::new(),
            planted: Vec::new(),
            probes: BTreeMap::new(),
            branch_validators: BTreeMap::new(),
        }
    }

    fn sentinel(&mut self, path: &str) -> Value {
        let sentinel = format!("{SENTINEL_PREFIX}{:05}", self.planted.len());
        self.planted.push((path.to_owned(), sentinel.clone()));
        Value::String(sentinel)
    }

    /// The planted sentinels that actually occur in `instances` (an alternative dropped on the way,
    /// because a required sibling had no instance or the cap was hit, never leaves the generator).
    fn present_in(&self, instances: &[&Value]) -> Vec<(String, String)> {
        let text: String = instances.iter().map(|v| v.to_string()).collect();
        self.planted
            .iter()
            .filter(|(_, s)| text.contains(s))
            .cloned()
            .collect()
    }

    /// Whether `pattern` (ECMA-262, unanchored, as `pattern` and `patternProperties` use it) selects
    /// `text`, decided by the validator; `None` when the validator cannot compile it.
    fn matches(&mut self, pattern: &str, text: &str) -> Option<bool> {
        let probe = self.probes.entry(pattern.to_owned()).or_insert_with(|| {
            jsonschema::validator_for(&json!({ "type": "string", "pattern": pattern })).ok()
        });
        probe
            .as_ref()
            .map(|v| v.is_valid(&Value::String(text.to_owned())))
    }

    /// Expands `roots` into everything that applies at one position: `$ref` chains through the
    /// authored types, `allOf` branches, and the unions still to choose from. With `guard`, a type
    /// already being generated up the stack is a cycle and the position has no instance (`false`);
    /// without it (planting into an existing, finite value) a reference is just followed once.
    fn expand<'b>(&mut self, roots: &[&'b Value], ex: &mut Expanded<'b>, guard: bool) -> bool
    where
        'a: 'b,
    {
        let types = self.types;
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        let mut stack: Vec<&'b Value> = roots.iter().rev().copied().collect();
        while let Some(schema) = stack.pop() {
            let Some(obj) = schema.as_object() else {
                continue;
            };
            ex.conj.push(obj);
            if let Some(reference) = obj.get("$ref").and_then(Value::as_str) {
                let name = reference
                    .strip_prefix("#/types/")
                    .unwrap_or_else(|| panic!("unexpected reference {reference}"));
                let target = types
                    .get(name)
                    .unwrap_or_else(|| panic!("{reference} names no authored type"));
                if guard {
                    if self.visiting.iter().any(|v| v == name) {
                        return false;
                    }
                    self.visiting.push(name.to_owned());
                    stack.push(target);
                } else if seen.insert(name) {
                    stack.push(target);
                }
            }
            if let Some(Value::Array(branches)) = obj.get("allOf") {
                stack.extend(branches.iter().rev());
            }
            // Taken as applying whether or not the condition holds, like the redactor does.
            for key in ["if", "then", "else"] {
                if let Some(conditional) = obj.get(key) {
                    stack.push(conditional);
                }
            }
            if let Some(Value::Object(dependents)) = obj.get("dependentSchemas") {
                stack.extend(dependents.values().rev());
            }
            for key in ["anyOf", "oneOf"] {
                if let Some(Value::Array(branches)) = obj.get(key) {
                    ex.choices.push(branches);
                }
            }
        }
        true
    }

    /// Instances of the position described by `roots` (in conjunction): one per union branch met,
    /// otherwise one. Wherever any applicable schema marks a position, the instance there is typed as
    /// the schema says and every scalar leaf inside it that can carry a sentinel does: a string becomes
    /// a string sentinel, an integer or number a numeric one when its range allows, so the instance
    /// still validates as an author could have written it while the whole marked value is covered.
    fn instances<'b>(&mut self, roots: &[&'b Value], path: &str) -> Vec<Value>
    where
        'a: 'b,
    {
        self.instances_marked(roots, path, false)
    }

    fn instances_marked<'b>(&mut self, roots: &[&'b Value], path: &str, marked: bool) -> Vec<Value>
    where
        'a: 'b,
    {
        let mark = self.visiting.len();
        let mut ex = Expanded::default();
        let out = if self.expand(roots, &mut ex, true) {
            self.expanded_instances(ex, path, marked)
        } else {
            Vec::new()
        };
        self.visiting.truncate(mark);
        out
    }

    fn expanded_instances<'b>(
        &mut self,
        mut ex: Expanded<'b>,
        path: &str,
        marked: bool,
    ) -> Vec<Value>
    where
        'a: 'b,
    {
        let marked = marked || ex.conj.iter().any(|o| is_marked(o));
        if let Some(choice) = ex.choices.pop() {
            let null = json!({ "type": "null" });
            let has_value_branch = choice.iter().any(|b| *b != null);
            let mut out = Vec::new();
            for branch in choice {
                if has_value_branch && *branch == null {
                    continue;
                }
                let mark = self.visiting.len();
                let mut next = ex.clone();
                if self.expand(&[branch], &mut next, true) {
                    out.extend(self.expanded_instances(next, path, marked));
                }
                self.visiting.truncate(mark);
                if out.len() >= MAX_ALTERNATIVES {
                    break;
                }
            }
            return out;
        }
        self.leaf_instances(&ex.conj, path, marked)
    }

    fn leaf_instances<'b>(
        &mut self,
        conj: &[&'b Map<String, Value>],
        path: &str,
        marked: bool,
    ) -> Vec<Value>
    where
        'a: 'b,
    {
        let get = |key: &str| conj.iter().find_map(|o| o.get(key));
        if let Some(c) = get("const") {
            return vec![c.clone()];
        }
        if let Some(Value::Array(members)) = get("enum")
            && let Some(first) = members.first()
        {
            return vec![first.clone()];
        }
        let declared = get("type").map(|t| match t {
            Value::String(s) => s.clone(),
            Value::Array(a) => a
                .iter()
                .filter_map(Value::as_str)
                .find(|s| *s != "null")
                .unwrap_or("null")
                .to_owned(),
            _ => "any".to_owned(),
        });
        let inferred = if ["properties", "patternProperties", "additionalProperties"]
            .iter()
            .any(|k| get(k).is_some())
        {
            Some("object".to_owned())
        } else if ["items", "prefixItems"].iter().any(|k| get(k).is_some()) {
            Some("array".to_owned())
        } else {
            None
        };
        match declared.or(inferred).as_deref() {
            Some("object") => self.object_instances(conj, path, marked),
            Some("array") => self.array_instances(conj, path, marked),
            Some("string") if marked => vec![self.sentinel(path)],
            Some("string") => vec![Value::String(self.string_instance(conj))],
            Some("integer" | "number") => vec![self.number_instance(conj, path, marked)],
            Some("boolean") => vec![Value::Bool(true)],
            Some("null") => vec![Value::Null],
            // `true` / `{}`: a value the schema does not describe; any string will do.
            _ if marked => vec![self.sentinel(path)],
            _ => vec![Value::String("a".into())],
        }
    }

    fn object_instances<'b>(
        &mut self,
        conj: &[&'b Map<String, Value>],
        path: &str,
        marked: bool,
    ) -> Vec<Value>
    where
        'a: 'b,
    {
        let mut names: BTreeSet<&'b str> = BTreeSet::new();
        let mut required: BTreeSet<&'b str> = BTreeSet::new();
        for obj in conj {
            if let Some(Value::Object(props)) = obj.get("properties") {
                names.extend(props.keys().map(String::as_str));
            }
            if let Some(Value::Array(req)) = obj.get("required") {
                required.extend(req.iter().filter_map(Value::as_str));
            }
        }
        // Every declared property, with every schema that applies to it: the named entry and every
        // selecting `patternProperties` entry of every applicable schema.
        let mut members: Vec<(String, Vec<Value>)> = Vec::new();
        for name in names
            .iter()
            .chain(required.iter())
            .copied()
            .collect::<BTreeSet<_>>()
        {
            let mut schemas: Vec<&'b Value> = Vec::new();
            for obj in conj {
                if let Some(prop) = obj.get("properties").and_then(|p| p.get(name)) {
                    schemas.push(prop);
                }
                schemas.extend(self.selecting_patterns(obj, name));
            }
            let alternatives = self.instances_marked(&schemas, &format!("{path}/{name}"), marked);
            if alternatives.is_empty() {
                if required.contains(name) {
                    return Vec::new();
                }
                continue;
            }
            members.push((name.to_owned(), alternatives));
        }
        // One member per `patternProperties` entry and one the fallback describes.
        for obj in conj {
            if let Some(Value::Object(patterns)) = obj.get("patternProperties") {
                for (pattern, schema) in patterns {
                    let Some(name) = NAME_CANDIDATES
                        .iter()
                        .find(|n| self.matches(pattern, n) == Some(true))
                    else {
                        continue;
                    };
                    if members.iter().any(|(m, _)| m == name) {
                        continue;
                    }
                    let alternatives =
                        self.instances_marked(&[schema], &format!("{path}/{name}"), marked);
                    if !alternatives.is_empty() {
                        members.push(((*name).to_owned(), alternatives));
                    }
                }
            }
            for key in ["additionalProperties", "unevaluatedProperties"] {
                let Some(fallback) = obj.get(key) else {
                    continue;
                };
                if !fallback.is_object() {
                    continue;
                }
                let selected_by_any = |this: &mut Self, name: &str| {
                    conj.iter().any(|o| {
                        this.selecting_patterns(o, name).next().is_some()
                            || o.get("properties").is_some_and(|p| p.get(name).is_some())
                    })
                };
                let Some(name) = NAME_CANDIDATES
                    .iter()
                    .copied()
                    .find(|n| !members.iter().any(|(m, _)| m == n) && !selected_by_any(self, n))
                else {
                    continue;
                };
                let alternatives =
                    self.instances_marked(&[fallback], &format!("{path}/{name}"), marked);
                if !alternatives.is_empty() {
                    members.push((name.to_owned(), alternatives));
                }
            }
        }
        let first: Map<String, Value> = members
            .iter()
            .map(|(name, alternatives)| (name.clone(), alternatives[0].clone()))
            .collect();
        let mut out = vec![Value::Object(first.clone())];
        for (name, alternatives) in &members {
            for alternative in alternatives.iter().skip(1) {
                let mut instance = first.clone();
                instance.insert(name.clone(), alternative.clone());
                out.push(Value::Object(instance));
                if out.len() >= MAX_ALTERNATIVES {
                    return out;
                }
            }
        }
        out
    }

    /// The `patternProperties` entries of `obj` that select `name`.
    fn selecting_patterns<'b>(
        &mut self,
        obj: &'b Map<String, Value>,
        name: &str,
    ) -> impl Iterator<Item = &'b Value> {
        let mut selected = Vec::new();
        if let Some(Value::Object(patterns)) = obj.get("patternProperties") {
            for (pattern, schema) in patterns {
                if self.matches(pattern, name) == Some(true) {
                    selected.push(schema);
                }
            }
        }
        selected.into_iter()
    }

    fn array_instances<'b>(
        &mut self,
        conj: &[&'b Map<String, Value>],
        path: &str,
        marked: bool,
    ) -> Vec<Value>
    where
        'a: 'b,
    {
        let get = |key: &str| conj.iter().find_map(|o| o.get(key));
        let min_items = get("minItems").and_then(Value::as_u64).unwrap_or(0) as usize;
        if let Some(Value::Array(prefix)) = get("prefixItems") {
            let mut tuple = Vec::new();
            for (i, item) in prefix.iter().enumerate() {
                let alternatives = self.instances_marked(&[item], &format!("{path}/{i}"), marked);
                let Some(first) = alternatives.into_iter().next() else {
                    return Vec::new();
                };
                tuple.push(first);
            }
            return vec![Value::Array(tuple)];
        }
        let mut items: Vec<&'b Value> = Vec::new();
        for obj in conj {
            if let Some(schema) = obj.get("items").or_else(|| obj.get("unevaluatedItems")) {
                items.push(schema);
            }
            if let Some(contains) = obj.get("contains") {
                items.push(contains);
            }
        }
        let alternatives = self.instances_marked(&items, &format!("{path}/0"), marked);
        if alternatives.is_empty() {
            return vec![Value::Array(Vec::new())];
        }
        alternatives
            .into_iter()
            .map(|item| Value::Array(vec![item; min_items.max(1)]))
            .collect()
    }

    fn string_instance(&mut self, conj: &[&Map<String, Value>]) -> String {
        let get = |key: &str| conj.iter().find_map(|o| o.get(key));
        match get("format").and_then(Value::as_str) {
            Some("uuid") => return "00000000-0000-4000-8000-000000000000".into(),
            Some("date-time") => return "2026-01-01T00:00:00.000Z".into(),
            _ => {}
        }
        let patterns: Vec<String> = conj
            .iter()
            .filter_map(|o| o.get("pattern")?.as_str().map(str::to_owned))
            .collect();
        let min = get("minLength").and_then(Value::as_u64).unwrap_or(0) as usize;
        let max = get("maxLength").and_then(Value::as_u64).unwrap_or(u64::MAX) as usize;
        for candidate in STRING_CANDIDATES {
            let len = candidate.chars().count();
            if len >= min
                && len <= max
                && patterns
                    .iter()
                    .all(|p| self.matches(p, candidate) == Some(true))
            {
                return (*candidate).to_owned();
            }
        }
        "a".repeat(min.max(1))
    }

    /// A number in range; at a marked position a numeric sentinel (`NUMERIC_SENTINEL_BASE + n`) when
    /// the range admits it, otherwise the plain number (the position is then covered by its siblings).
    fn number_instance(&mut self, conj: &[&Map<String, Value>], path: &str, marked: bool) -> Value {
        let get = |key: &str| conj.iter().find_map(|o| o.get(key)?.as_f64());
        let min = get("minimum")
            .or_else(|| get("exclusiveMinimum").map(|m| m + 1.0))
            .unwrap_or(0.0);
        let max = get("maximum")
            .or_else(|| get("exclusiveMaximum").map(|m| m - 1.0))
            .unwrap_or(f64::MAX);
        if marked {
            let candidate = NUMERIC_SENTINEL_BASE + self.planted.len() as i64;
            if (candidate as f64) >= min && (candidate as f64) <= max {
                self.planted.push((path.to_owned(), candidate.to_string()));
                return json!(candidate);
            }
        }
        json!(min.max(0.0).min(max) as i64)
    }

    /// Replaces every leaf at a marked position of `value` (described by `roots`) with a sentinel,
    /// and adds every absent declared property whose instance would hold one. Unions are resolved by
    /// asking the validator which branches the existing value satisfies.
    fn plant_into<'b>(&mut self, value: &mut Value, roots: &[&'b Value], path: &str)
    where
        'a: 'b,
    {
        let mut ex = Expanded::default();
        self.expand(roots, &mut ex, false);
        while let Some(choice) = ex.choices.pop() {
            for branch in choice {
                if self.satisfies(branch, value) {
                    self.expand(&[branch], &mut ex, false);
                }
            }
        }
        if ex.conj.iter().any(|o| is_marked(o)) {
            self.stamp(value, path);
            return;
        }
        let conj = ex.conj;
        match value {
            Value::Object(map) => {
                let mut names: BTreeSet<String> = map.keys().cloned().collect();
                for obj in &conj {
                    if let Some(Value::Object(props)) = obj.get("properties") {
                        names.extend(props.keys().cloned());
                    }
                }
                for name in names {
                    let mut schemas: Vec<&'b Value> = Vec::new();
                    for obj in &conj {
                        let mut matched = false;
                        if let Some(prop) = obj.get("properties").and_then(|p| p.get(&name)) {
                            matched = true;
                            schemas.push(prop);
                        }
                        let patterns: Vec<&'b Value> =
                            self.selecting_patterns(obj, &name).collect();
                        matched |= !patterns.is_empty();
                        schemas.extend(patterns);
                        if !matched {
                            for key in ["additionalProperties", "unevaluatedProperties"] {
                                if let Some(fallback) = obj.get(key) {
                                    schemas.push(fallback);
                                }
                            }
                        }
                    }
                    let member_path = format!("{path}/{name}");
                    if let Some(member) = map.get_mut(&name) {
                        self.plant_into(member, &schemas, &member_path);
                    } else {
                        let before = self.planted.len();
                        let generated = self.instances(&schemas, &member_path).into_iter().next();
                        match generated {
                            Some(instance) if self.planted.len() > before => {
                                map.insert(name, instance);
                            }
                            _ => self.planted.truncate(before),
                        }
                    }
                }
            }
            Value::Array(items) => {
                for (i, item) in items.iter_mut().enumerate() {
                    let mut schemas: Vec<&'b Value> = Vec::new();
                    for obj in &conj {
                        if let Some(prefixed) = obj.get("prefixItems").and_then(|p| p.get(i)) {
                            schemas.push(prefixed);
                        } else if let Some(items_schema) =
                            obj.get("items").or_else(|| obj.get("unevaluatedItems"))
                        {
                            schemas.push(items_schema);
                        }
                        if let Some(contains) = obj.get("contains") {
                            schemas.push(contains);
                        }
                    }
                    self.plant_into(item, &schemas, &format!("{path}/{i}"));
                }
            }
            _ => {}
        }
    }

    /// Sentinels into every string and number leaf of an existing marked value; a value with no such
    /// leaf (`{}`, `true`) becomes a string sentinel so the position is covered anyway.
    fn stamp(&mut self, value: &mut Value, path: &str) {
        let before = self.planted.len();
        self.stamp_leaves(value, path);
        if self.planted.len() == before {
            *value = self.sentinel(path);
        }
    }

    fn stamp_leaves(&mut self, value: &mut Value, path: &str) {
        match value {
            Value::String(_) => *value = self.sentinel(path),
            Value::Number(_) => {
                let candidate = NUMERIC_SENTINEL_BASE + self.planted.len() as i64;
                self.planted.push((path.to_owned(), candidate.to_string()));
                *value = json!(candidate);
            }
            Value::Object(map) => {
                for (name, member) in map.iter_mut() {
                    self.stamp_leaves(member, &format!("{path}/{name}"));
                }
            }
            Value::Array(items) => {
                for (i, item) in items.iter_mut().enumerate() {
                    self.stamp_leaves(item, &format!("{path}/{i}"));
                }
            }
            _ => {}
        }
    }

    /// Whether `value` satisfies `branch` with the authored types in scope.
    fn satisfies(&mut self, branch: &Value, value: &Value) -> bool {
        let key = branch.to_string();
        if !self.branch_validators.contains_key(&key) {
            let root = json!({ "allOf": [branch], "types": self.types });
            let validator = jsonschema::validator_for(&root)
                .unwrap_or_else(|e| panic!("authored branch does not compile: {e}\n{branch}"));
            self.branch_validators.insert(key.clone(), validator);
        }
        self.branch_validators[&key].is_valid(value)
    }
}

/// An `x-secret` that is not `false`: `true`, or malformed (publication treats that as secret too).
fn is_marked(obj: &Map<String, Value>) -> bool {
    obj.get("x-secret")
        .is_some_and(|m| *m != Value::Bool(false))
}

// ----- the stress catalog: secret-bearing shapes as an author would write them ---------------------

#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct Creds {
    user: String,
    #[schemars(extend("x-secret" = true))]
    token: String,
}

/// Marked as a whole.
#[derive(Serialize, Deserialize, JsonSchema)]
#[schemars(extend("x-secret" = true))]
struct WholeSecret {
    value: String,
    n: u32,
}

#[derive(Serialize, Deserialize, JsonSchema)]
#[schemars(inline)]
struct InlineCreds {
    user: String,
    #[schemars(extend("x-secret" = true))]
    token: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
#[schemars(inline, extend("x-secret" = true))]
struct InlineWhole {
    value: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
enum Choice {
    Plain {
        text: String,
    },
    Secret {
        #[schemars(extend("x-secret" = true))]
        key: String,
    },
    Nested {
        creds: Creds,
        whole: Option<WholeSecret>,
    },
}

#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct Flat {
    flat_user: String,
    #[schemars(extend("x-secret" = true))]
    flat_token: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct Deeper {
    creds: Option<Creds>,
    #[schemars(extend("x-secret" = true))]
    pin: Option<u32>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct Deep {
    inner: Option<Deeper>,
    inners: Vec<Deeper>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct StressParams {
    #[schemars(extend("x-secret" = true))]
    plain: String,
    #[schemars(extend("x-secret" = true))]
    optional_string: Option<String>,
    optional_creds: Option<Creds>,
    optional_whole: Option<WholeSecret>,
    #[schemars(extend("x-secret" = true))]
    optional_marked_ref: Option<Creds>,
    /// schemars merges an inline type into the field (`type: ["object", "null"]`), so this takes the
    /// type-array path of the normalisation, not the `anyOf` one.
    inline_optional: Option<InlineCreds>,
    inline_whole: Option<InlineWhole>,
    list: Vec<Creds>,
    optional_list: Option<Vec<WholeSecret>>,
    map: BTreeMap<String, Creds>,
    secret_map: BTreeMap<String, WholeSecret>,
    choice: Choice,
    choices: Vec<Choice>,
    optional_choice: Option<Choice>,
    #[serde(flatten)]
    flat: Flat,
    pair: (String, WholeSecret),
    nested: Option<Vec<Option<Creds>>>,
    templated: TemplateOr<Creds>,
    #[schemars(extend("x-secret" = true))]
    untyped: Value,
    deep: Deep,
}

#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct StressResult {
    #[schemars(extend("x-secret" = true))]
    session: String,
    items: Vec<Choice>,
    creds: Option<Creds>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct StressPayload {
    viewer: String,
    #[schemars(extend("x-secret" = true))]
    session: String,
    creds: Option<Creds>,
    choice: Choice,
}

/// The core catalog plus one operation per tiered shape (tier 1: dry-run result; tier 2: dry-run and
/// draft results) and one event, all over the stress types. Their registration examples are planted
/// too, so the catalog lint sees them.
fn stress_catalog() -> Catalog {
    let mut catalog = Catalog::core();
    let params = catalog.schema_for::<StressParams>();
    let result = catalog.schema_for::<StressResult>();
    let payload = catalog.schema_for::<StressPayload>();
    let types: BTreeMap<String, Value> = catalog.peek_definitions().into_iter().collect();
    let mut planter = Planter::new(&types);
    let params_instance = planter
        .instances(&[params.as_value()], "stress/params")
        .into_iter()
        .next()
        .expect("params instance");
    let result_instance = planter
        .instances(&[result.as_value()], "stress/result")
        .into_iter()
        .next()
        .expect("result instance");
    let payload_instance = planter
        .instances(&[payload.as_value()], "stress/payload")
        .into_iter()
        .next()
        .expect("payload instance");
    let dry_run_ref = json!({ "$ref": "#/types/DryRunResult" });
    let draft_ref = json!({ "$ref": "#/types/DraftResult" });
    let dry_run = planter
        .instances(&[&dry_run_ref], "stress/dryRun")
        .into_iter()
        .next()
        .expect("dry-run instance");
    let draft = planter
        .instances(&[&draft_ref], "stress/draft")
        .into_iter()
        .next()
        .expect("draft instance");
    let mut dry_run_params = params_instance.clone();
    dry_run_params["dryRun"] = json!(true);
    let step_ref = json!({ "$ref": "#/types/PlanStep" });
    for (name, tier) in [
        ("stress.write", RiskTier::REVERSIBLE_WRITE),
        ("stress.config", RiskTier::CONFIG),
    ] {
        // A plan step naming this operation, with its params planted (the lint only accepts
        // non-empty step params for operations of this catalog).
        let mut step = planter
            .instances(&[&step_ref], "stress/plan/0")
            .into_iter()
            .next()
            .expect("plan step");
        step["operation"] = json!(name);
        step["params"] = params_instance.clone();
        let mut dry_run = dry_run.clone();
        dry_run["plan"] = json!([step]);
        let mut draft = draft.clone();
        draft["plan"] = json!([step]);
        let mut examples = vec![
            OperationExample {
                title: "planted".into(),
                params: params_instance.clone(),
                shape: ResultShape::Normal,
                result: result_instance.clone(),
            },
            OperationExample {
                title: "planted dry run".into(),
                params: dry_run_params.clone(),
                shape: ResultShape::DryRun,
                result: dry_run.clone(),
            },
        ];
        if tier == RiskTier::CONFIG {
            examples.push(OperationExample {
                title: "planted draft".into(),
                params: params_instance.clone(),
                shape: ResultShape::Draft,
                result: draft.clone(),
            });
        }
        catalog
            .register_operation(OperationDef {
                name: OperationName::new(name).unwrap(),
                title: "Stress".into(),
                description: "Stress. Secret-bearing shapes.".into(),
                category: vec!["Stress".into()],
                params: params.clone(),
                result: result.clone(),
                tier,
                scopes: vec![],
                expose: Exposure {
                    rpc: true,
                    sub_action: true,
                    mcp: true,
                },
                idempotent: false,
                writes_args: ArgWrites::None,
                external: false,
                timeout_ms: None,
                platforms: None,
                high_volume_result: false,
                since: "0.1.0".into(),
                deprecated: None,
                examples,
            })
            .unwrap();
    }
    // A JSON-authored operation (what a connector spec may register), with the shapes schemars never
    // emits from Rust types: the round-4 collision (a property's own `properties` beside the optional
    // branch that carries the mark), marked `patternProperties`, a marked `additionalProperties`, and a
    // referenced marked type inside a union.
    let creds_ref = catalog.ref_for::<Creds>().to_value();
    let authored = json!({
        "type": "object",
        "properties": {
            "creds": {
                "properties": { "token": { "type": "string" } },
                "anyOf": [
                    { "type": "object", "properties": { "token": { "type": "string", "x-secret": true } } },
                    { "type": "null" }
                ]
            },
            "keys": {
                "type": "object",
                "patternProperties": { "^k_": { "type": "string", "x-secret": true } },
                "additionalProperties": { "type": "string" }
            },
            "bag": { "type": "object", "additionalProperties": { "type": "string", "x-secret": true } },
            "either": { "anyOf": [creds_ref, { "type": "integer" }] }
        }
    });
    let types: BTreeMap<String, Value> = catalog.peek_definitions().into_iter().collect();
    let mut planter = Planter::new(&types);
    let example = planter
        .instances(&[&authored], "stress.authored/params")
        .into_iter()
        .next()
        .expect("authored instance");
    catalog
        .register_operation(OperationDef {
            name: OperationName::new("stress.authored").unwrap(),
            title: "Stress".into(),
            description: "Stress. JSON-authored shapes.".into(),
            category: vec!["Stress".into()],
            params: schemars::Schema::try_from(authored).unwrap(),
            result: schemars::Schema::try_from(json!({ "type": "object" })).unwrap(),
            tier: RiskTier::READ,
            scopes: vec![],
            expose: Exposure {
                rpc: true,
                sub_action: true,
                mcp: true,
            },
            idempotent: true,
            writes_args: ArgWrites::None,
            external: false,
            timeout_ms: None,
            platforms: None,
            high_volume_result: false,
            since: "0.1.0".into(),
            deprecated: None,
            examples: vec![OperationExample {
                title: "planted".into(),
                params: example,
                shape: ResultShape::Normal,
                result: json!({}),
            }],
        })
        .unwrap();
    catalog
        .register_event(EventDef {
            r#type: EventType::new("stress.happened").unwrap(),
            title: "Stress".into(),
            description: "Stress. A secret-bearing payload.".into(),
            category: vec!["Stress".into()],
            payload_version: 1,
            payload,
            filters: vec![],
            high_volume: false,
            critical: false,
            examples: vec![payload_instance],
        })
        .unwrap();
    catalog
}

// ----- the pipeline ----------------------------------------------------------------------------------

/// What one run planted and published.
struct Report {
    /// Sentinels present in the planted examples.
    planted_examples: usize,
    examples: usize,
    /// Examples the pipeline's own validation rejects, with the reason.
    invalid_examples: Vec<String>,
    type_instances: usize,
    /// Sentinels present in the shared-type instances.
    planted_types: usize,
}

/// Runs the real publication pipeline over `catalog` with planted examples, then checks every shared
/// type through the redactor against the published type table. Panics on the first sentinel that
/// survives, naming the positions.
fn run(catalog: Catalog, label: &str) -> Report {
    // The authored truth: schemas as generated, before any transform of this crate runs.
    let authored_types: BTreeMap<String, Value> = catalog.peek_definitions().into_iter().collect();
    let authored_ops: Vec<OperationDef> = catalog.operations().values().cloned().collect();
    let authored_events: Vec<EventDef> = catalog.events().values().cloned().collect();
    let type_refs: BTreeMap<String, Value> = authored_types
        .keys()
        .map(|name| (name.clone(), json!({ "$ref": format!("#/types/{name}") })))
        .collect();

    // The real pipeline, step 1: build (lint, transforms, reserved params, derivation).
    let mut doc = codegen::build_unredacted(catalog).expect("the catalog passes lint");
    for name in authored_types.keys() {
        assert!(
            doc.types.contains_key(name),
            "{label}: authored type {name} is missing from the published table"
        );
    }

    // Plant examples from the authored schemas into the built document, the way an author's examples
    // sit in it when xtask validates and redacts them.
    let mut planter = Planter::new(&authored_types);
    let dry_run_ref = &type_refs["DryRunResult"];
    let draft_ref = &type_refs["DraftResult"];
    let step_ref = &type_refs["PlanStep"];
    for def in &authored_ops {
        let name = def.name.to_string();
        let params = planter.instances(
            &[def.params.as_value()],
            &format!("operation:{name}/params"),
        );
        let results = planter.instances(
            &[def.result.as_value()],
            &format!("operation:{name}/result"),
        );
        assert!(
            !params.is_empty() && !results.is_empty(),
            "{label}: no instance for {name}"
        );
        let entry = doc.operations.get_mut(&name).expect("built");
        for i in 0..params.len().max(results.len()) {
            entry.def.examples.push(OperationExample {
                title: format!("planted normal {i}"),
                params: params[i % params.len()].clone(),
                shape: ResultShape::Normal,
                result: results[i % results.len()].clone(),
            });
        }
        // A dry run plans a step per catalog operation, each with that operation's params planted:
        // plan steps are redacted against the named operation's invocation schema.
        let plan: Vec<Value> = authored_ops
            .iter()
            .map(|target| {
                let mut step = planter
                    .instances(
                        &[step_ref],
                        &format!("operation:{name}/plan/{}", target.name),
                    )
                    .into_iter()
                    .next()
                    .expect("plan step");
                step["operation"] = json!(target.name.to_string());
                step["params"] = planter
                    .instances(
                        &[target.params.as_value()],
                        &format!("operation:{name}/plan/{}/params", target.name),
                    )
                    .into_iter()
                    .next()
                    .expect("step params");
                step
            })
            .collect();
        if entry.dry_run_result.is_some() {
            let mut dry_run = planter
                .instances(&[dry_run_ref], &format!("operation:{name}/dryRunResult"))
                .into_iter()
                .next()
                .expect("dry-run instance");
            dry_run["plan"] = Value::Array(plan.clone());
            let mut dry_run_params = params[0].clone();
            dry_run_params["dryRun"] = json!(true);
            entry.def.examples.push(OperationExample {
                title: "planted dry run".into(),
                params: dry_run_params,
                shape: ResultShape::DryRun,
                result: dry_run,
            });
        }
        if entry.draft_result.is_some() {
            let mut draft = planter
                .instances(&[draft_ref], &format!("operation:{name}/draftResult"))
                .into_iter()
                .next()
                .expect("draft instance");
            draft["plan"] = Value::Array(plan);
            entry.def.examples.push(OperationExample {
                title: "planted draft".into(),
                params: params[0].clone(),
                shape: ResultShape::Draft,
                result: draft,
            });
        }
    }
    for def in &authored_events {
        let r#type = def.r#type.to_string();
        let payloads =
            planter.instances(&[def.payload.as_value()], &format!("event:{type}/payload"));
        assert!(!payloads.is_empty(), "{label}: no payload for {type}");
        doc.events
            .get_mut(&r#type)
            .expect("built")
            .def
            .examples
            .extend(payloads);
    }

    // How many of the planted examples the pipeline's own validation (xtask) would accept.
    let (examples, invalid_examples) = count_valid(&doc);

    let instances: Vec<&Value> = doc
        .operations
        .values()
        .flat_map(|e| e.def.examples.iter().flat_map(|x| [&x.params, &x.result]))
        .chain(doc.events.values().flat_map(|e| e.def.examples.iter()))
        .collect();
    let present = planter.present_in(&instances);

    // The real pipeline, steps 2 and 3: redact, then serialise what goes to disk.
    doc.redact_examples();
    let text = doc.to_json_string();
    let leaked: Vec<&str> = present
        .iter()
        .filter(|(_, s)| text.contains(s))
        .map(|(path, _)| path.as_str())
        .collect();
    assert!(
        leaked.is_empty(),
        "{label}: {} planted secret(s) published in protocol.json at:\n  - {}",
        leaked.len(),
        leaked.join("\n  - ")
    );
    assert!(
        !text.contains(SENTINEL_PREFIX),
        "{label}: a sentinel reached protocol.json outside the ledger"
    );
    // Idempotent: a second redaction changes nothing.
    let mut again = doc.clone();
    again.redact_examples();
    assert_eq!(
        again.to_json_string(),
        text,
        "{label}: redaction is idempotent"
    );

    // Every shared type, through the redactor against the published table.
    let mut type_instances = 0;
    let mut planted_types = 0;
    let mut leaked: Vec<String> = Vec::new();
    for (name, reference) in &type_refs {
        let instances = planter.instances(&[reference], &format!("type:{name}"));
        for mut instance in instances {
            let before = planter.present_in(&[&instance]);
            type_instances += 1;
            planted_types += before.len();
            redact_secrets(&mut instance, reference, &doc.types);
            let after = instance.to_string();
            leaked.extend(
                before
                    .iter()
                    .filter(|(_, s)| after.contains(s))
                    .map(|(path, _)| path.clone()),
            );
        }
    }
    assert!(
        leaked.is_empty(),
        "{label}: {} planted secret(s) survive redaction of a shared type at:\n  - {}",
        leaked.len(),
        leaked.join("\n  - ")
    );
    assert!(
        present.len() + planted_types > 0,
        "{label}: nothing planted; the catalog marks no secret the generator can reach"
    );
    Report {
        planted_examples: present.len(),
        examples,
        invalid_examples,
        type_instances,
        planted_types,
    }
}

/// Validates every example the way `xtask` does (params against the invocation schema, the result
/// against the shape it declares, event payloads against the payload schema). Returns the count and
/// the examples that do not validate, each with the validator's first complaint.
fn count_valid(doc: &ProtocolDocument) -> (usize, Vec<String>) {
    let mut all = 0;
    let mut invalid = Vec::new();
    for (name, entry) in &doc.operations {
        let params =
            jsonschema::validator_for(&doc.schema_at(&format!("/operations/{name}/params")))
                .expect("params schema compiles");
        let shapes: Vec<(ResultShape, jsonschema::Validator)> = [
            (ResultShape::Normal, Some("result")),
            (
                ResultShape::DryRun,
                entry.dry_run_result.as_ref().map(|_| "dryRunResult"),
            ),
            (
                ResultShape::Draft,
                entry.draft_result.as_ref().map(|_| "draftResult"),
            ),
        ]
        .into_iter()
        .filter_map(|(shape, key)| {
            let pointer = format!("/operations/{name}/{}", key?);
            Some((
                shape,
                jsonschema::validator_for(&doc.schema_at(&pointer))
                    .expect("result schema compiles"),
            ))
        })
        .collect();
        for example in &entry.def.examples {
            all += 1;
            let label = format!("operation {name}, example {:?}", example.title);
            if let Err(e) = params.validate(&example.params) {
                invalid.push(format!("{label}: params: {e}"));
                continue;
            }
            match shapes.iter().find(|(shape, _)| *shape == example.shape) {
                Some((_, v)) => {
                    if let Err(e) = v.validate(&example.result) {
                        invalid.push(format!("{label}: result: {e}"));
                    }
                }
                None => invalid.push(format!("{label}: no {:?} shape", example.shape)),
            }
        }
    }
    for (r#type, entry) in &doc.events {
        let payload = jsonschema::validator_for(&doc.schema_at(&format!("/events/{type}/payload")))
            .expect("payload schema compiles");
        for (i, example) in entry.def.examples.iter().enumerate() {
            all += 1;
            if let Err(e) = payload.validate(example) {
                invalid.push(format!("event {type}, example {i}: {e}"));
            }
        }
    }
    (all, invalid)
}

#[test]
fn planted_secrets_never_reach_a_published_artifact() {
    let core = run(Catalog::core(), "core catalog");
    let stress = run(stress_catalog(), "core + stress catalog");
    for (label, report) in [("core", &core), ("core + stress", &stress)] {
        println!(
            "{label}: {} sentinels planted across {} examples ({} valid as authored); {} planted across {} shared-type instances",
            report.planted_examples,
            report.examples,
            report.examples - report.invalid_examples.len(),
            report.planted_types,
            report.type_instances
        );
    }
    assert!(
        core.planted_types >= 1,
        "the core catalog marks at least `SessionIdentifyParams.token`"
    );
    assert!(
        stress.planted_examples > 20,
        "the stress catalog plants its shapes in examples ({})",
        stress.planted_examples
    );
    assert!(
        stress.invalid_examples.is_empty(),
        "every planted stress example validates as authored, so each is one an author could have written; these do not:\n  - {}",
        stress.invalid_examples.join("\n  - ")
    );

    // The other published artifacts carry no example and so no sentinel, whatever the catalog.
    let scratch =
        std::env::temp_dir().join(format!("den-protocol-sentinel-{}", std::process::id()));
    std::fs::create_dir_all(&scratch).unwrap();
    let ts = codegen::generate_typescript(&scratch).expect("ts export");
    let _ = std::fs::remove_dir_all(&scratch);
    for (rel, content) in &ts {
        assert!(
            !content.contains(SENTINEL_PREFIX),
            "{}: sentinel in the TypeScript bindings",
            rel.display()
        );
    }
    assert!(
        !codegen::to_canonical_json(&codegen::plugin_manifest_schema()).contains(SENTINEL_PREFIX)
    );
}

/// Every committed fixture, with a sentinel planted at each marked position of its type (and each
/// absent optional position that would hold one), through the redactor against the published table.
#[test]
fn fixtures_with_planted_secrets_redact() {
    let mut catalog = Catalog::core();
    let mut subjects: Vec<(String, Value)> = Vec::new();
    fn dir<T: JsonSchema>(catalog: &mut Catalog, subjects: &mut Vec<(String, Value)>, dir: &str) {
        let reference = catalog.ref_for::<T>().to_value();
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(dir);
        let mut names: Vec<String> = std::fs::read_dir(&root)
            .unwrap_or_else(|e| panic!("{dir}: {e}"))
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".json") && !n.starts_with("invalid_"))
            .collect();
        names.sort();
        assert!(!names.is_empty(), "no fixtures in {dir}");
        for name in names {
            subjects.push((format!("{dir}/{name}"), reference.clone()));
        }
    }
    fn file<T: JsonSchema>(catalog: &mut Catalog, subjects: &mut Vec<(String, Value)>, rel: &str) {
        subjects.push((rel.to_owned(), catalog.ref_for::<T>().to_value()));
    }
    use den_protocol::auth::{AuthContext, Principal};
    use den_protocol::domain::{
        Action, ActionRef, CodeSource, Filter, Origin, PendingChange, Queue, StoredDocument,
        SubAction, Trigger, VariableRecord,
    };
    use den_protocol::events::Event;
    use den_protocol::manifest::PluginManifest;
    use den_protocol::results::{DraftResult, DryRunResult, PluginCallParams, PluginCallResult};
    use den_protocol::rpc::{
        BatchExecuteParams, BatchExecuteResult, EventsDroppedNotification, EventsSubscribeParams,
        EventsUnsubscribeParams, Hello, RpcNotification, RpcRequest, RpcResponse,
        SessionIdentifyParams, SessionIdentifyResult, SessionReidentifyParams,
    };
    use den_protocol::shared::Fragment;
    dir::<SubAction>(&mut catalog, &mut subjects, "sub_action");
    dir::<Origin>(&mut catalog, &mut subjects, "origin");
    dir::<Principal>(&mut catalog, &mut subjects, "principal");
    dir::<Fragment>(&mut catalog, &mut subjects, "fragment");
    dir::<Filter>(&mut catalog, &mut subjects, "filter");
    dir::<ActionRef>(&mut catalog, &mut subjects, "action_ref");
    dir::<CodeSource>(&mut catalog, &mut subjects, "code_source");
    dir::<ArgWrites>(&mut catalog, &mut subjects, "arg_writes");
    dir::<Action>(&mut catalog, &mut subjects, "action");
    dir::<Trigger>(&mut catalog, &mut subjects, "trigger");
    dir::<Queue>(&mut catalog, &mut subjects, "queue");
    dir::<VariableRecord>(&mut catalog, &mut subjects, "variable");
    dir::<PendingChange>(&mut catalog, &mut subjects, "pending_change");
    dir::<StoredDocument>(&mut catalog, &mut subjects, "stored_document");
    dir::<Event>(&mut catalog, &mut subjects, "event");
    dir::<AuthContext>(&mut catalog, &mut subjects, "auth");
    dir::<PluginManifest>(&mut catalog, &mut subjects, "manifest");
    for rel in ["rpc/request.json", "rpc/request_string_id.json"] {
        file::<RpcRequest>(&mut catalog, &mut subjects, rel);
    }
    for rel in [
        "rpc/response_ok.json",
        "rpc/response_error.json",
        "rpc/response_parse_error.json",
        "rpc/response_invalid_params.json",
        "rpc/response_null_result.json",
    ] {
        file::<RpcResponse>(&mut catalog, &mut subjects, rel);
    }
    file::<RpcNotification>(&mut catalog, &mut subjects, "rpc/notification_event.json");
    file::<Hello>(&mut catalog, &mut subjects, "rpc/hello.json");
    file::<SessionIdentifyParams>(&mut catalog, &mut subjects, "rpc/identify_params.json");
    file::<SessionIdentifyResult>(&mut catalog, &mut subjects, "rpc/identify_result.json");
    file::<SessionReidentifyParams>(&mut catalog, &mut subjects, "rpc/reidentify_params.json");
    file::<EventsSubscribeParams>(&mut catalog, &mut subjects, "rpc/events_subscribe.json");
    file::<EventsUnsubscribeParams>(&mut catalog, &mut subjects, "rpc/events_unsubscribe.json");
    file::<EventsDroppedNotification>(&mut catalog, &mut subjects, "rpc/events_dropped.json");
    file::<BatchExecuteParams>(&mut catalog, &mut subjects, "rpc/batch_params.json");
    file::<BatchExecuteResult>(&mut catalog, &mut subjects, "rpc/batch_result.json");
    file::<DryRunResult>(&mut catalog, &mut subjects, "results/dry_run.json");
    file::<DraftResult>(&mut catalog, &mut subjects, "results/draft.json");
    file::<DraftResult>(&mut catalog, &mut subjects, "results/draft_action.json");
    file::<PluginCallResult>(&mut catalog, &mut subjects, "results/plugin_call.json");
    file::<PluginCallParams>(
        &mut catalog,
        &mut subjects,
        "results/plugin_call_params.json",
    );

    let authored_types: BTreeMap<String, Value> = catalog.peek_definitions().into_iter().collect();
    let published = codegen::build_unredacted(catalog).expect("core passes lint");
    let mut planter = Planter::new(&authored_types);
    let mut planted_total = 0;
    let mut leaked: Vec<String> = Vec::new();
    for (rel, reference) in &subjects {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(rel);
        let mut fixture: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap())
            .unwrap_or_else(|e| panic!("{rel}: {e}"));
        let before = planter.planted.len();
        planter.plant_into(&mut fixture, &[reference], rel);
        let present = planter.present_in(&[&fixture]);
        planted_total += present.len().saturating_sub(
            present
                .iter()
                .filter(|(p, _)| !p.starts_with(rel.as_str()))
                .count(),
        );
        assert!(
            planter.planted.len() == before || !present.is_empty(),
            "{rel}: planted but nothing present"
        );
        redact_secrets(&mut fixture, reference, &published.types);
        let after = fixture.to_string();
        leaked.extend(
            present
                .iter()
                .filter(|(_, s)| after.contains(s))
                .map(|(p, _)| p.clone()),
        );
    }
    println!(
        "{} fixtures, {} sentinels planted",
        subjects.len(),
        planted_total
    );
    assert!(
        planted_total > 0,
        "no fixture reaches a marked position; the identify params carry one"
    );
    assert!(
        leaked.is_empty(),
        "{} planted secret(s) survive fixture redaction at:\n  - {}",
        leaked.len(),
        leaked.join("\n  - ")
    );
}
