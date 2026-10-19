//! Stand-in for a contract crate: one public type the codegen job exports to TypeScript.

/// Viewer event kinds, mirrored into `packages/b4-web/src/generated/event-kind.ts`.
pub const EVENT_KINDS: &[&str] = &["chat", "follow", "raid", "subscribe"];

/// Returns `true` when `kind` is a known event kind.
#[must_use]
pub fn is_event_kind(kind: &str) -> bool {
    EVENT_KINDS.contains(&kind)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_and_unknown_kinds() {
        assert!(is_event_kind("raid"));
        assert!(!is_event_kind("Raid"));
    }

    #[test]
    fn kinds_are_sorted_and_unique() {
        assert!(EVENT_KINDS.windows(2).all(|w| w[0] < w[1]));
    }
}
