//! Context-provider contract tests; this test provider does not implement engine behavior.

use std::collections::HashMap;

use den_core::context::{ContextView, Lookup, Root, Seg};
use serde_json::{Value, json};

struct Fixture(HashMap<Root, Value>);

impl ContextView for Fixture {
    fn get(&self, root: Root, path: &[Seg]) -> Lookup<'_> {
        let Some(mut value) = self.0.get(&root) else {
            return Lookup::Missing;
        };
        for seg in path {
            let next = match seg {
                Seg::Key(key) => value.get(key),
                Seg::Index(index) => value.get(*index),
            };
            let Some(next) = next else {
                return Lookup::Missing;
            };
            value = next;
        }
        Lookup::Value(value)
    }
}

#[test]
fn lookup_preserves_null_and_missing() {
    let view = Fixture(HashMap::from([(Root::Args, json!({"present": null}))]));
    let provider: &dyn ContextView = &view;
    assert_eq!(
        provider.get(Root::Args, &[Seg::Key("present".into())]),
        Lookup::Value(&Value::Null)
    );
    assert_eq!(
        provider.get(Root::Args, &[Seg::Key("absent".into())]),
        Lookup::Missing
    );
    assert_eq!(provider.get(Root::Event, &[]), Lookup::Missing);
}

#[test]
fn lookup_borrows_nested_values() {
    let view = Fixture(HashMap::from([(
        Root::Event,
        json!({"payload": [{"text": "hello"}]}),
    )]));
    let path = [
        Seg::Key("payload".into()),
        Seg::Index(0),
        Seg::Key("text".into()),
    ];
    let Lookup::Value(value) = view.get(Root::Event, &path) else {
        panic!("nested value must be present");
    };
    assert!(std::ptr::eq(
        value,
        &view.0[&Root::Event]["payload"][0]["text"]
    ));
    assert_eq!(value, "hello");
    assert_eq!(view.get(Root::Event, &[Seg::Index(0)]), Lookup::Missing);
}

#[test]
fn lookup_keeps_all_reserved_roots_distinct() {
    let roots = [
        Root::Event,
        Root::User,
        Root::Trigger,
        Root::Action,
        Root::Run,
        Root::Globals,
        Root::Plugins,
        Root::Match,
        Root::Args,
        Root::Now,
    ];
    let view = Fixture(
        roots
            .into_iter()
            .enumerate()
            .map(|(i, root)| (root, json!(i)))
            .collect(),
    );
    assert_eq!(view.0.len(), 10);
    for (i, root) in roots.into_iter().enumerate() {
        assert_eq!(view.get(root, &[]), Lookup::Value(&json!(i)));
    }
}
