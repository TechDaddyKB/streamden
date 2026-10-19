//! Internal lookup boundary shared by templates and run-context providers (Spec 02 §6.3).

use serde_json::Value;

/// Reserved context roots; ordinary template names resolve through `Args`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Root {
    Event,
    User,
    Trigger,
    Action,
    Run,
    Globals,
    Plugins,
    Match,
    Args,
    Now,
}

/// A resolved path segment, independent of the template parser's wire AST.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Seg {
    Key(String),
    Index(usize),
}

/// A present JSON null is a value, distinct from an absent path.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Lookup<'a> {
    Value(&'a Value),
    Missing,
}

/// Providers retain ownership of returned values for the lifetime of the lookup.
pub trait ContextView {
    fn get(&self, root: Root, path: &[Seg]) -> Lookup<'_>;
}
