//! JSON Schema generation settings and transforms (Spec 02 §3, §4, §9.1, §11).
//!
//! Every schema this crate publishes comes from one [`SchemaGenerator`] configuration so that
//! `protocol.json`, `plugin.schema.json` and the test fixtures agree.

use std::collections::BTreeSet;

use schemars::generate::SchemaSettings;
use schemars::transform::{Transform, transform_subschemas};
use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::codegen::{is_annotation_key, merged_secret_mark};
use crate::version::RESERVED_SLUGS;

/// JSON pointer (without `#`) under which `protocol.json` keeps its shared types (§9.1).
pub const TYPES_PATH: &str = "/types";

/// The generator settings behind `protocol.json`: draft 2020-12, shared types under `#/types`, and the
/// protocol transforms ([`DenyNullForOptional`], [`ContributionRefRule`]).
pub fn protocol_settings() -> SchemaSettings {
    SchemaSettings::draft2020_12()
        .with(|s| {
            s.definitions_path = TYPES_PATH.into();
            s.meta_schema = None;
        })
        .with_transform(DenyNullForOptional)
        .with_transform(ContributionRefRule)
}

/// A generator with [`protocol_settings`].
pub fn protocol_generator() -> SchemaGenerator {
    protocol_settings().into_generator()
}

/// The generator settings behind a standalone document schema (`plugin.schema.json`, persisted-document
/// fixtures): draft 2020-12 with `$defs`, the protocol transforms and strict validation ([`Strict`]).
pub fn strict_settings() -> SchemaSettings {
    SchemaSettings::draft2020_12()
        .with_transform(DenyNullForOptional)
        .with_transform(ContributionRefRule)
        .with_transform(Strict)
}

/// A root schema for `T` with [`protocol_settings`] (wire types: unknown fields ignored).
pub fn root_schema_for<T: JsonSchema + ?Sized>() -> Schema {
    protocol_settings()
        .with(|s| s.definitions_path = "/$defs".into())
        .into_generator()
        .into_root_schema_for::<T>()
}

/// A root schema for `T` with [`strict_settings`] (manifests and persisted documents: unknown keys fail
/// unless they start with `x-`).
pub fn strict_root_schema_for<T: JsonSchema + ?Sized>() -> Schema {
    strict_settings()
        .into_generator()
        .into_root_schema_for::<T>()
}

/// The schema of the `kind` tag of an unknown-kind fallback variant (§7.2): any string except the tags
/// the build knows, so the fallback branch of the union never overlaps a known branch and a malformed
/// known-kind value is still rejected.
pub fn unknown_tag_schema(known: &[&str]) -> Schema {
    schemars::json_schema!({
        "type": "string",
        "not": { "enum": known }
    })
}

/// Deserializes the `kind` tag of an unknown-kind fallback variant, refusing the tags the build knows
/// (the serde counterpart of [`unknown_tag_schema`]): serde tries an `untagged` fallback variant
/// whenever the tagged variant fails, so without this a malformed known kind would decode as unknown.
pub fn deserialize_unknown_tag<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
    known: &[&str],
) -> Result<String, D::Error> {
    let kind = String::deserialize(deserializer)?;
    if known.contains(&kind.as_str()) {
        return Err(serde::de::Error::custom(format!(
            "`{kind}` is a known kind; its members are invalid"
        )));
    }
    Ok(kind)
}

/// Removes `null` from the schema of optional (non-required) properties (§3): absent means "not
/// provided", and `null` is only valid where a schema says so explicitly. An `Option<T>` property arrives
/// as `anyOf: [T, { "type": "null" }]` beside the property's own keys (its description, `x-secret`,
/// `x-ui`, …); the surviving branch and those keys applied in conjunction before the null branch went
/// and still do after it, so neither side may lose a key to the other (see [`absorb_branch`]).
#[derive(Debug, Clone, Default)]
pub struct DenyNullForOptional;

impl Transform for DenyNullForOptional {
    fn transform(&mut self, schema: &mut Schema) {
        if let Some(obj) = schema.as_object_mut() {
            let required: BTreeSet<String> = obj
                .get("required")
                .and_then(Value::as_array)
                .map(|r| {
                    r.iter()
                        .filter_map(Value::as_str)
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default();
            if let Some(Value::Object(props)) = obj.get_mut("properties") {
                for (name, prop) in props.iter_mut() {
                    if !required.contains(name) {
                        strip_null(prop);
                    }
                }
            }
        }
        transform_subschemas(self, schema);
    }
}

fn strip_null(prop: &mut Value) {
    let Value::Object(obj) = prop else { return };
    if let Some(Value::Array(types)) = obj.get_mut("type") {
        types.retain(|t| t != "null");
        if types.len() == 1 {
            let single = types.remove(0);
            obj.insert("type".into(), single);
        }
    }
    if let Some(Value::Array(any_of)) = obj.get("anyOf")
        && any_of.len() == 2
        && let Some(idx) = any_of.iter().position(|s| s == &json!({ "type": "null" }))
    {
        let mut any_of = match obj.remove("anyOf") {
            Some(Value::Array(a)) => a,
            _ => unreachable!("checked above"),
        };
        any_of.remove(idx);
        absorb_branch(obj, any_of.remove(0));
    }
}

/// Joins the surviving branch of an optional property's `anyOf` with the property's own keys without
/// losing either. When the property holds nothing but annotations ([`is_annotation_key`]: the common
/// schemars output, a description and `x-` hints beside the `anyOf`) the branch is lifted into it; an
/// annotation the property also carries stays the property's, the way a `$ref` sibling wins over its
/// target, except `x-secret`, which is joined monotonically ([`merged_secret_mark`], §3): `true` on
/// either side wins, and a malformed mark is kept so publication fails closed on it. When the property
/// carries a constraint of its own the branch is kept whole as an `allOf` member instead: keywords of one
/// schema object interact (`additionalProperties` with `properties`, `items` with `prefixItems`, …), so
/// lifting the branch's keys beside the property's could change what the schema accepts, and taking only
/// the keys the property lacks would silently drop the rest, secret marks included.
fn absorb_branch(obj: &mut Map<String, Value>, branch: Value) {
    let Value::Object(mut branch) = branch else {
        // A boolean (or malformed) branch has no keys to lift; keep it as it was.
        keep_in_conjunction(obj, branch);
        return;
    };
    if let Some(mark) = merged_secret_mark(obj.get("x-secret"), branch.get("x-secret")) {
        obj.insert("x-secret".into(), mark);
    }
    if obj.keys().all(|key| is_annotation_key(key)) {
        branch.remove("x-secret");
        for (key, value) in branch {
            obj.entry(key).or_insert(value);
        }
    } else {
        keep_in_conjunction(obj, Value::Object(branch));
    }
}

/// Adds `branch` to the property's `allOf`. The property's `anyOf` was just taken out, so when `allOf`
/// is present but not an array (not a schema this crate would ever generate) the free `anyOf` slot
/// holds the branch instead of dropping it.
fn keep_in_conjunction(obj: &mut Map<String, Value>, branch: Value) {
    match obj.get_mut("allOf") {
        Some(Value::Array(all_of)) => all_of.push(branch),
        Some(_) => {
            obj.insert("anyOf".into(), json!([branch]));
        }
        None => {
            obj.insert("allOf".into(), json!([branch]));
        }
    }
}

/// Adds the generated `if/then/else` clause of §4 to every object schema that embeds a
/// [`crate::ContributionRef`] (a `type` string next to a `plugin` reference): `plugin` is forbidden for
/// a reserved slug and required otherwise.
#[derive(Debug, Clone, Default)]
pub struct ContributionRefRule;

impl ContributionRefRule {
    /// The regex matched against `type` to recognise a core contribution.
    pub fn core_type_pattern() -> String {
        format!("^({})\\.", RESERVED_SLUGS.join("|"))
    }

    fn embeds_contribution_ref(obj: &Map<String, Value>) -> bool {
        let Some(Value::Object(props)) = obj.get("properties") else {
            return false;
        };
        let has_type = props
            .get("type")
            .and_then(Value::as_object)
            .is_some_and(|t| {
                t.get("type").and_then(Value::as_str) == Some("string") || t.get("$ref").is_some()
            });
        let plugin_ref = props
            .get("plugin")
            .and_then(Value::as_object)
            .and_then(|p| p.get("$ref"))
            .and_then(Value::as_str)
            .is_some_and(|r| r.ends_with("/PluginId"));
        has_type && plugin_ref
    }
}

impl Transform for ContributionRefRule {
    fn transform(&mut self, schema: &mut Schema) {
        if let Some(obj) = schema.as_object_mut()
            && Self::embeds_contribution_ref(obj)
            && !obj.contains_key("if")
        {
            obj.insert(
                "if".into(),
                json!({ "properties": { "type": { "pattern": Self::core_type_pattern() } }, "required": ["type"] }),
            );
            obj.insert("then".into(), json!({ "not": { "required": ["plugin"] } }));
            obj.insert("else".into(), json!({ "required": ["plugin"] }));
        }
        transform_subschemas(self, schema);
    }
}

/// Strict validation for manifests and persisted documents (§3, §11): unknown keys fail unless they
/// start with `x-`. Plain objects get `additionalProperties: false`; objects composed with a flattened
/// union (`SubAction`, `Trigger`) get `unevaluatedProperties: false` instead, and the branches of that
/// union stay open because the composed object evaluates them (§17.1).
#[derive(Debug, Clone, Default)]
pub struct Strict;

impl Strict {
    fn apply(&mut self, schema: &mut Schema, inside_composition: bool) {
        let Some(obj) = schema.as_object_mut() else {
            return;
        };
        let has_properties = obj.contains_key("properties");
        let composed = ["allOf", "anyOf", "oneOf", "$ref", "if"]
            .iter()
            .any(|k| obj.contains_key(*k));
        if has_properties && !inside_composition {
            let key = if composed {
                "unevaluatedProperties"
            } else {
                "additionalProperties"
            };
            if !obj.contains_key("additionalProperties")
                && !obj.contains_key("unevaluatedProperties")
            {
                obj.insert(key.into(), Value::Bool(false));
                obj.entry("patternProperties")
                    .or_insert_with(|| json!({}))
                    .as_object_mut()
                    .map(|pp| pp.entry("^x-").or_insert(Value::Bool(true)));
            }
        }
        // Recurse. Branches of a union that sits next to `properties` belong to a composition and must
        // not deny properties the composed object declares.
        let union_in_composition = has_properties && composed;
        for key in ["allOf", "anyOf", "oneOf"] {
            if let Some(Value::Array(branches)) = obj.get_mut(key) {
                for branch in branches {
                    if let Ok(branch) = <&mut Schema>::try_from(branch) {
                        self.apply(branch, union_in_composition);
                    }
                }
            }
        }
        for key in [
            "then",
            "else",
            "if",
            "not",
            "items",
            "additionalProperties",
            "unevaluatedProperties",
        ] {
            if let Some(child) = obj.get_mut(key)
                && let Ok(child) = <&mut Schema>::try_from(child)
            {
                self.apply(child, union_in_composition && key != "items");
            }
        }
        for key in ["properties", "patternProperties", "$defs", "definitions"] {
            if let Some(Value::Object(children)) = obj.get_mut(key) {
                for child in children.values_mut() {
                    if let Ok(child) = <&mut Schema>::try_from(child) {
                        self.apply(child, false);
                    }
                }
            }
        }
    }
}

impl Transform for Strict {
    fn transform(&mut self, schema: &mut Schema) {
        self.apply(schema, false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_null_handles_both_forms() {
        let mut v = json!({ "type": ["string", "null"], "description": "d" });
        strip_null(&mut v);
        assert_eq!(v, json!({ "type": "string", "description": "d" }));
        let mut v =
            json!({ "anyOf": [{ "$ref": "#/types/X" }, { "type": "null" }], "description": "d" });
        strip_null(&mut v);
        assert_eq!(v, json!({ "$ref": "#/types/X", "description": "d" }));
        // Only a two-branch `anyOf` with a null branch is an optional; anything else is left alone.
        let other = json!({ "anyOf": [{ "type": "string" }, { "type": "integer" }] });
        let mut v = other.clone();
        strip_null(&mut v);
        assert_eq!(v, other);
    }

    /// Codex round-4 P1: keys the property shares with the surviving branch used to be dropped.
    #[test]
    fn strip_null_keeps_both_schemas() {
        // Repro A: the property's `x-secret: false` sat on the branch's `x-secret: true`; the join is
        // `true` (§3, monotonic) and the branch is lifted, the property holding only annotations.
        let mut v = json!({
            "x-secret": false,
            "description": "d",
            "anyOf": [
                { "type": "object", "x-secret": true, "properties": { "token": { "type": "string" } } },
                { "type": "null" }
            ]
        });
        strip_null(&mut v);
        assert_eq!(
            v,
            json!({
                "x-secret": true,
                "description": "d",
                "type": "object",
                "properties": { "token": { "type": "string" } }
            })
        );

        // Repro B: no false mark anywhere; the property's own `properties` would have shadowed the
        // branch's, which held the mark. A property with a constraint keeps the branch whole.
        let mut v = json!({
            "properties": { "token": { "type": "string" } },
            "anyOf": [
                { "type": "object", "properties": { "token": { "type": "string", "x-secret": true } } },
                { "type": "null" }
            ]
        });
        strip_null(&mut v);
        assert_eq!(
            v,
            json!({
                "properties": { "token": { "type": "string" } },
                "allOf": [
                    { "type": "object", "properties": { "token": { "type": "string", "x-secret": true } } }
                ]
            })
        );

        // A malformed mark on either side survives the join, so publication can still fail closed.
        for (outer, inner, joined) in [
            (json!("yes"), json!(true), json!("yes")),
            (json!(false), json!("yes"), json!("yes")),
            (json!(true), json!(false), json!(true)),
            (json!(false), json!(true), json!(true)),
        ] {
            let mut v = json!({
                "x-secret": outer,
                "anyOf": [{ "$ref": "#/types/X", "x-secret": inner }, { "type": "null" }]
            });
            strip_null(&mut v);
            assert_eq!(v, json!({ "x-secret": joined, "$ref": "#/types/X" }));
        }

        // Lifting: the property's annotation wins over the branch's, the branch's other annotations and
        // every constraint come along.
        let mut v = json!({
            "description": "property",
            "x-ui": { "variables": true },
            "anyOf": [
                { "description": "branch", "default": "x", "type": "string", "minLength": 1 },
                { "type": "null" }
            ]
        });
        strip_null(&mut v);
        assert_eq!(
            v,
            json!({
                "description": "property",
                "x-ui": { "variables": true },
                "default": "x",
                "type": "string",
                "minLength": 1
            })
        );

        // Conjunction: an existing `allOf` array takes the branch; a non-array `allOf` is not touched and
        // the freed `anyOf` slot holds it; a boolean branch is kept too.
        let mut v = json!({
            "maxProperties": 1,
            "allOf": [{ "required": ["a"] }],
            "anyOf": [{ "type": "object", "x-secret": true }, { "type": "null" }]
        });
        strip_null(&mut v);
        assert_eq!(
            v,
            json!({
                "maxProperties": 1,
                "x-secret": true,
                "allOf": [{ "required": ["a"] }, { "type": "object", "x-secret": true }]
            })
        );
        let mut v = json!({
            "maxProperties": 1,
            "allOf": "garbage",
            "anyOf": [{ "type": "object" }, { "type": "null" }]
        });
        strip_null(&mut v);
        assert_eq!(
            v,
            json!({ "maxProperties": 1, "allOf": "garbage", "anyOf": [{ "type": "object" }] })
        );
        let mut v = json!({ "description": "d", "anyOf": [true, { "type": "null" }] });
        strip_null(&mut v);
        assert_eq!(v, json!({ "description": "d", "allOf": [true] }));
    }
}
