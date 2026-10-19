//! Secrets as typed values (Spec 02 §3): the value-level half of secret handling, and the primary one.
//!
//! A position that holds a secret is declared with [`Secret<T>`] in the Rust type. The value then
//! carries its own secrecy wherever it goes: its `Serialize` impl writes the placeholder [`REDACTED`]
//! unless the caller is inside [`expose_secrets`], its `Debug` never shows the value, its JSON Schema is
//! `T`'s with `x-secret: true`, and its TypeScript type is `T`'s. An example built from typed values
//! ([`crate::catalog::OperationExample::typed`], [`crate::catalog::EventDef::push_typed_example`]) is
//! serialized with secrets withheld, so the published form never held the secret at all: no schema
//! transform, reference inlining or keyword-scope change can leak what was never there. The schema walk
//! of [`crate::codegen::redact_secrets`] remains as defence in depth, and for JSON-authored examples
//! that no Rust type describes.
//!
//! Exposure is a synchronous, thread-local scope: `expose_secrets(|| serde_json::to_vec(&params))` is
//! the engine's way to put a real token on the wire. The flag is not carried across an `.await` and is
//! restored when the closure returns or unwinds. It is a property of the thread, not of the closure:
//! anything else that runs on the thread meanwhile (a logger that serializes, a `block_on` that polls
//! other tasks) serializes exposed too, so keep the closure to the one serialization and never block
//! on a runtime inside it. Nothing in this crate exposes a secret on its own.

use std::borrow::Cow;
use std::cell::Cell;
use std::fmt;

use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

/// The redacted form of a secret (§3): what a [`Secret`] serializes to outside [`expose_secrets`] and
/// what the publication walk writes at every marked position.
pub const REDACTED: &str = "***";

thread_local! {
    static EXPOSED: Cell<bool> = const { Cell::new(false) };
}

/// Whether the current thread is inside [`expose_secrets`].
pub fn secrets_exposed() -> bool {
    EXPOSED.with(Cell::get)
}

fn with_exposure<R>(exposed: bool, f: impl FnOnce() -> R) -> R {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            EXPOSED.with(|e| e.set(self.0));
        }
    }
    let previous = EXPOSED.with(|e| e.replace(exposed));
    let _restore = Restore(previous);
    f()
}

/// Runs `f` with every [`Secret`] serializing its real value: the wire path (a token in
/// `session.identify`, params handed to a plugin). Synchronous and thread-local; restored on return
/// and on unwind. Never wrap a publication, a log line or an example in it.
pub fn expose_secrets<R>(f: impl FnOnce() -> R) -> R {
    with_exposure(true, f)
}

/// Runs `f` with every [`Secret`] serializing as [`REDACTED`], even inside an enclosing
/// [`expose_secrets`]: the publication and example path.
pub fn withhold_secrets<R>(f: impl FnOnce() -> R) -> R {
    with_exposure(false, f)
}

/// A value that is a secret (§3). Serializes as [`REDACTED`] unless inside [`expose_secrets`];
/// deserializes as `T`; `Debug` never shows the value; the JSON Schema is `T`'s plus `x-secret: true`
/// (inline, so the mark sits on the position that holds it); the TypeScript type is `T`'s.
#[derive(Clone, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Secret<T>(T);

impl<T> Secret<T> {
    /// Wraps a value.
    pub const fn new(value: T) -> Self {
        Self(value)
    }

    /// The value. The name is the reminder: whatever is done with it must not be a log line, an
    /// example or a published artifact.
    pub const fn expose(&self) -> &T {
        &self.0
    }

    /// Unwraps the value.
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> From<T> for Secret<T> {
    fn from(value: T) -> Self {
        Self(value)
    }
}

impl<T> fmt::Debug for Secret<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Secret({REDACTED:?})")
    }
}

impl<T: Serialize> Serialize for Secret<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if secrets_exposed() {
            self.0.serialize(serializer)
        } else {
            serializer.serialize_str(REDACTED)
        }
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Secret<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        T::deserialize(deserializer).map(Self)
    }
}

impl<T: JsonSchema> JsonSchema for Secret<T> {
    fn inline_schema() -> bool {
        true
    }

    fn schema_name() -> Cow<'static, str> {
        format!("Secret_{}", T::schema_name()).into()
    }

    fn schema_id() -> Cow<'static, str> {
        format!("den_protocol::Secret<{}>", T::schema_id()).into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        let mut schema = generator.subschema_for::<T>();
        schema
            .ensure_object()
            .insert("x-secret".into(), Value::Bool(true));
        schema
    }
}

impl<T: ts_rs::TS> ts_rs::TS for Secret<T> {
    type WithoutGenerics = Self;
    type OptionInnerType = Self;

    fn name(cfg: &ts_rs::Config) -> String {
        T::name(cfg)
    }

    fn inline(cfg: &ts_rs::Config) -> String {
        T::inline(cfg)
    }

    fn inline_flattened(cfg: &ts_rs::Config) -> String {
        T::inline_flattened(cfg)
    }

    fn visit_dependencies(v: &mut impl ts_rs::TypeVisitor)
    where
        Self: 'static,
    {
        T::visit_dependencies(v);
    }

    fn visit_generics(v: &mut impl ts_rs::TypeVisitor)
    where
        Self: 'static,
    {
        T::visit_generics(v);
        v.visit::<T>();
    }

    fn decl(_: &ts_rs::Config) -> String {
        panic!("Secret<T> is a wrapper and cannot be declared")
    }

    fn decl_concrete(_: &ts_rs::Config) -> String {
        panic!("Secret<T> is a wrapper and cannot be declared")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
    struct Creds {
        user: String,
        token: Secret<String>,
        pin: Option<Secret<u32>>,
    }

    fn creds() -> Creds {
        Creds {
            user: "alice".into(),
            token: Secret::new("sdt_synthetic".into()),
            pin: Some(Secret::new(1234)),
        }
    }

    #[test]
    fn serializes_redacted_unless_exposed() {
        assert_eq!(
            serde_json::to_value(creds()).unwrap(),
            json!({ "user": "alice", "token": "***", "pin": "***" })
        );
        assert_eq!(
            expose_secrets(|| serde_json::to_value(creds()).unwrap()),
            json!({ "user": "alice", "token": "sdt_synthetic", "pin": 1234 })
        );
        // Withholding wins inside an exposure, and the previous state is restored afterwards.
        expose_secrets(|| {
            assert_eq!(
                withhold_secrets(|| serde_json::to_value(creds()).unwrap()),
                json!({ "user": "alice", "token": "***", "pin": "***" })
            );
            assert!(secrets_exposed());
        });
        assert!(!secrets_exposed());
    }

    #[test]
    fn exposure_is_restored_on_unwind() {
        let result = std::panic::catch_unwind(|| {
            expose_secrets(|| panic!("synthetic"));
        });
        assert!(result.is_err());
        assert!(!secrets_exposed());
    }

    #[test]
    fn round_trips_and_never_debugs_the_value() {
        let parsed: Creds = serde_json::from_value(
            json!({ "user": "alice", "token": "sdt_synthetic", "pin": 1234 }),
        )
        .unwrap();
        assert_eq!(parsed, creds());
        assert_eq!(parsed.token.expose(), "sdt_synthetic");
        let dbg = format!("{parsed:?}");
        assert!(
            dbg.contains("Secret(\"***\")") && !dbg.contains("sdt_synthetic"),
            "{dbg}"
        );
    }

    #[test]
    fn schema_marks_the_position() {
        let schema = schemars::schema_for!(Creds).to_value();
        assert_eq!(
            schema["properties"]["token"],
            json!({ "type": "string", "x-secret": true })
        );
        assert_eq!(
            schema["properties"]["pin"],
            json!({ "type": ["integer", "null"], "format": "uint32", "minimum": 0, "x-secret": true })
        );
    }
}
