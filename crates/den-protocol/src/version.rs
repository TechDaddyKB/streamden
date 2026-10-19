//! Protocol version numbers and reserved names (Spec 02 §4, §6.3, §7.1, §8.2, §8.3).

/// `protocolVersion` (SemVer) written at the top of `protocol.json`. Minor bumps on additive changes,
/// major together with [`RPC_VERSION`] (§7.1). Pre-1.0: unstable.
pub const PROTOCOL_VERSION: &str = "0.1.0";

/// Current `rpcVersion`, negotiated in `session.identify` and used as the `/api/v<N>` path segment (§7.1).
pub const RPC_VERSION: u32 = 1;

/// Oldest `rpcVersion` this build still serves (§7.1, §7.4). Equal to [`RPC_VERSION`] until the first
/// breaking change opens a deprecation window.
pub const RPC_VERSION_MIN: u32 = 1;

/// WebSocket subprotocol name for the JSON encoding (§8.2). This is the *encoding* version and does
/// not follow `rpcVersion`.
pub const WS_SUBPROTOCOL_JSON: &str = "streamden.v1.json";

/// Reserved WebSocket subprotocol name for a later MessagePack encoding (§8.2). Not required in v1.
pub const WS_SUBPROTOCOL_MSGPACK: &str = "streamden.v1.msgpack";

/// `manifestVersion` of the plugin manifest schema this build validates (§11). v0 is pre-stable.
pub const MANIFEST_VERSION: u32 = 0;

/// `engines.pluginApi` (SemVer major.minor) implemented by this build (§7.1).
pub const PLUGIN_API_VERSION: &str = "0.1";

/// Base of the `den://schemas/v0/<Name>.json` URIs manifests use to reference shared types (§5.3, §11).
/// Resolved offline from the bundled catalog; the public `$id` host is still open (Q-B7).
pub const SCHEMA_URI_BASE: &str = "den://schemas/v0/";

/// Default maximum size of one WebSocket message in bytes (§3). Spec 06 makes it configurable.
pub const MAX_WS_MESSAGE_BYTES: usize = 1024 * 1024;

/// Maximum JSON nesting depth accepted from plugins or clients (§3).
pub const MAX_JSON_DEPTH: u32 = 32;

/// Maximum number of entries in a JSON-RPC batch array or a `batch.execute` call (§8.2).
pub const MAX_BATCH_ENTRIES: usize = 64;

/// Time a client has to call `session.identify` after connecting, in milliseconds (§8.3).
pub const IDENTIFY_TIMEOUT_MS: u32 = 10_000;

/// Default heartbeat interval announced in `hello`, in milliseconds (§8.3).
pub const DEFAULT_HEARTBEAT_MS: u32 = 30_000;

/// Lifetime of an MCP approval handle, in milliseconds (§4).
pub const APPROVAL_TTL_MS: u32 = 120_000;

/// Slugs reserved for core contributions (§4). A plugin manifest must not use any of them, and a
/// [`crate::ContributionRef`] whose slug is reserved must not carry `plugin`.
pub const RESERVED_SLUGS: &[&str] = &[
    "core",
    "system",
    "session",
    "auth",
    "actions",
    "triggers",
    "queues",
    "variables",
    "events",
    "chat",
    "tts",
    "audio",
    "plugins",
    "obs",
    "twitch",
    "youtube",
    "kick",
    "x",
    "bluesky",
    "discord",
    "vts",
    "vmc",
    "mcp",
    "ai",
    "code",
    "ui",
    "overlay",
    "deck",
    "util",
    "batch",
    "global",
    "globals",
    "user",
    "event",
    "run",
    "trigger",
    "action",
];

/// Reserved root names of the run context (§6.3). An event payload must not declare a top-level
/// field with one of these names, and `Set argument` refuses them.
pub const RESERVED_RUN_CONTEXT_ROOTS: &[&str] = &[
    "event", "user", "trigger", "action", "run", "globals", "plugins", "match", "args", "now",
];

/// Operations whose effective tier is only known at call time (§8.1, §8.8 rule 2). Codegen adds the
/// reserved `approvalId` param to them, and MCP annotates them like tier 3.
pub const DYNAMIC_TIER_OPERATIONS: &[&str] = &["actions.run", "plugins.call"];

/// Protocol methods and notifications owned by Spec 02 §12. They are **not** catalog operations:
/// registering one as an operation is a catalog lint failure (§8.1, §9.7).
pub const PROTOCOL_METHODS: &[&str] = &[
    "hello",
    "session.identify",
    "session.reidentify",
    "auth.request_pairing",
    "system.ping",
    "system.describe",
    "events.subscribe",
    "events.unsubscribe",
    "event",
    "events.dropped",
    "system.deprecation_used",
    "catalog.changed",
    "batch.execute",
];

/// Returns `true` when `slug` is reserved for core (§4).
pub fn is_reserved_slug(slug: &str) -> bool {
    RESERVED_SLUGS.contains(&slug)
}

/// Returns `true` when `name` is a reserved run-context root (§6.3).
pub fn is_reserved_run_context_root(name: &str) -> bool {
    RESERVED_RUN_CONTEXT_ROOTS.contains(&name)
}

/// Returns `true` when `method` is a protocol method or notification (§12), never a catalog operation.
pub fn is_protocol_method(method: &str) -> bool {
    PROTOCOL_METHODS.contains(&method)
}
