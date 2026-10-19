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
/// provided", and `null` is only valid where a schema says so explicitly.
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
        if let Value::Object(inner) = any_of.remove(0) {
            for (k, v) in inner {
                obj.entry(k).or_insert(v);
            }
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
    }
}
