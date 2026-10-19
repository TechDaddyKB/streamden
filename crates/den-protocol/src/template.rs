//! Template fields (Spec 02 §10, §10.1).
//!
//! The parser lives in `den-core`; this crate only fixes the wire shape: a [`Template`] is a string in
//! the `{…}` syntax, and a [`TemplateOr<T>`] is a value-mode field that holds either a literal `T` or
//! exactly one `{…}` form.

use std::borrow::Cow;
use std::fmt;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use ts_rs::TS;

/// A string in the template syntax of §10 (`{user.displayName}`, `{= count * 2}`, mixed text). Parsed
/// when saved; the grammar and AST live in a later card.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize, JsonSchema, TS)]
#[serde(transparent)]
#[schemars(extend("x-den-template" = "text"))]
pub struct Template(pub String);

impl Template {
    /// Wraps a template string without parsing it.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// The string form.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for Template {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl From<String> for Template {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl fmt::Display for Template {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// The JSON Schema `pattern` of a value-mode template: exactly one `{…}` form (§10.1).
pub const TEMPLATE_STRING_PATTERN: &str = "^\\{(=|[A-Za-z_]).*\\}$";

/// Returns `true` when `s` is exactly one `{…}` form, which a value-mode field treats as a template and
/// never as a literal (§10.1).
pub fn is_template_string(s: &str) -> bool {
    let Some(inner) = s.strip_prefix('{').and_then(|rest| rest.strip_suffix('}')) else {
        return false;
    };
    let Some(first) = inner.chars().next() else {
        return false;
    };
    (first == '=' || first.is_ascii_alphabetic() || first == '_')
        && !inner.contains(['\n', '\r', '\u{2028}', '\u{2029}'])
}

/// The `TemplateString` schema that `paramsAuthoring` admits at value-mode leaves (§10.1).
pub fn template_string_schema() -> Schema {
    json_schema!({
        "type": "string",
        "pattern": TEMPLATE_STRING_PATTERN,
        "x-den-template": "value"
    })
}

/// A value-mode field (§10.1): a literal `T`, or one whole `{…}` form rendered at run time. The schema is
/// `anyOf: [T, TemplateString]`, never at a root. This is the one permitted untagged union (§3).
#[derive(Debug, Clone, PartialEq, TS)]
#[ts(untagged)]
pub enum TemplateOr<T> {
    /// A literal value.
    Value(T),
    /// A template rendered against the run context.
    Template(Template),
}

impl<T> TemplateOr<T> {
    /// Returns the literal value, if this is one.
    pub fn as_value(&self) -> Option<&T> {
        match self {
            Self::Value(v) => Some(v),
            Self::Template(_) => None,
        }
    }

    /// Returns the template, if this is one.
    pub fn as_template(&self) -> Option<&Template> {
        match self {
            Self::Value(_) => None,
            Self::Template(t) => Some(t),
        }
    }
}

impl<T> From<T> for TemplateOr<T> {
    fn from(value: T) -> Self {
        Self::Value(value)
    }
}

impl<T: Serialize> Serialize for TemplateOr<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Value(v) => v.serialize(serializer),
            Self::Template(t) => t.serialize(serializer),
        }
    }
}

impl<'de, T: DeserializeOwned> Deserialize<'de> for TemplateOr<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = serde_json::Value::deserialize(deserializer)?;
        if let serde_json::Value::String(s) = &raw
            && is_template_string(s)
        {
            return Ok(Self::Template(Template(s.clone())));
        }
        T::deserialize(raw)
            .map(Self::Value)
            .map_err(serde::de::Error::custom)
    }
}

impl<T: JsonSchema> JsonSchema for TemplateOr<T> {
    fn schema_name() -> Cow<'static, str> {
        Cow::Owned(format!("TemplateOr_{}", T::schema_name()))
    }

    fn schema_id() -> Cow<'static, str> {
        Cow::Owned(format!(
            "den_protocol::template::TemplateOr<{}>",
            T::schema_id()
        ))
    }

    fn inline_schema() -> bool {
        true
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        let inner = generator.subschema_for::<T>();
        json_schema!({
            "anyOf": [inner, template_string_schema()]
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_string_detection() {
        assert!(is_template_string("{amount}"));
        assert!(is_template_string("{= count * 2}"));
        assert!(is_template_string("{_x}"));
        assert!(!is_template_string("{\"a\": 1}"));
        assert!(!is_template_string("{{literal}}"));
        assert!(!is_template_string("text {amount}"));
        assert!(!is_template_string("{}"));
    }

    #[test]
    fn template_or_round_trips() {
        let v: TemplateOr<u64> = serde_json::from_str("500").unwrap();
        assert_eq!(v, TemplateOr::Value(500));
        let t: TemplateOr<u64> = serde_json::from_str("\"{delay}\"").unwrap();
        assert_eq!(t, TemplateOr::Template(Template::new("{delay}")));
        assert_eq!(serde_json::to_string(&t).unwrap(), "\"{delay}\"");
        assert!(serde_json::from_str::<TemplateOr<u64>>("\"500\"").is_err());
    }
}
