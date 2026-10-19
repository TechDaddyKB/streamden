//! Contract tests (Spec 02 §17.1, §17.2, §17.12, §17.14): every committed fixture round-trips through
//! serde unchanged and validates against the generated schema; invalid fixtures are rejected; the
//! `protocol.json` and `plugin.schema.json` on disk are the golden snapshots of this crate.

use std::path::{Path, PathBuf};

use den_protocol::auth::{AuthContext, Principal};
use den_protocol::catalog::{ArgWrites, Catalog, ResultShape};
use den_protocol::codegen;
use den_protocol::domain::{
    Action, ActionRef, CodeSource, Filter, Origin, PendingChange, Queue, StoredDocument, SubAction,
    Trigger, VariableRecord,
};
use den_protocol::events::Event;
use den_protocol::manifest::PluginManifest;
use den_protocol::results::{DraftResult, DryRunResult, PluginCallParams, PluginCallResult};
use den_protocol::rpc::{
    BatchExecuteParams, BatchExecuteResult, EventNotification, EventsDroppedNotification,
    EventsSubscribeParams, EventsUnsubscribeParams, Hello, RpcNotification, RpcRequest,
    RpcResponse, SessionIdentifyParams, SessionIdentifyResult, SessionReidentifyParams,
};
use den_protocol::schema::{root_schema_for, strict_root_schema_for};
use den_protocol::shared::Fragment;
use schemars::JsonSchema;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root")
}

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn load(rel: &str) -> Value {
    let path = fixtures().join(rel);
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", path.display()))
}

fn fixture_files(dir: &str) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(fixtures().join(dir))
        .unwrap_or_else(|e| panic!("fixture dir {dir}: {e}"))
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".json"))
        .map(|n| format!("{dir}/{n}"))
        .collect();
    names.sort();
    assert!(!names.is_empty(), "no fixtures in {dir}");
    names
}

fn validator(schema: Value) -> jsonschema::Validator {
    jsonschema::validator_for(&schema).unwrap_or_else(|e| panic!("schema does not compile: {e}"))
}

fn assert_valid(v: &jsonschema::Validator, instance: &Value, what: &str) {
    if let Err(e) = v.validate(instance) {
        panic!("{what}: schema rejects fixture: {e}");
    }
}

/// Round-trips every fixture in `dir` through `T` and validates it against `T`'s wire schema (and, when
/// `strict`, its strict document schema).
fn check_dir<T: Serialize + DeserializeOwned + JsonSchema>(dir: &str, strict: bool) {
    let wire = validator(root_schema_for::<T>().to_value());
    let strict_v = strict.then(|| validator(strict_root_schema_for::<T>().to_value()));
    for rel in fixture_files(dir) {
        let fixture = load(&rel);
        let typed: T =
            serde_json::from_value(fixture.clone()).unwrap_or_else(|e| panic!("{rel}: {e}"));
        let back = serde_json::to_value(&typed).expect("serialize");
        assert_eq!(back, fixture, "{rel}: round trip changed the document");
        assert_valid(&wire, &fixture, &rel);
        if let Some(s) = &strict_v {
            assert_valid(s, &fixture, &format!("{rel} (strict)"));
        }
    }
}

fn check_file<T: Serialize + DeserializeOwned + JsonSchema>(rel: &str) {
    let wire = validator(root_schema_for::<T>().to_value());
    let fixture = load(rel);
    let typed: T = serde_json::from_value(fixture.clone()).unwrap_or_else(|e| panic!("{rel}: {e}"));
    assert_eq!(
        serde_json::to_value(&typed).expect("serialize"),
        fixture,
        "{rel}: round trip changed the document"
    );
    assert_valid(&wire, &fixture, rel);
}

#[test]
fn every_node_kind_round_trips() {
    let files = fixture_files("sub_action");
    assert_eq!(
        files.len(),
        19,
        "one fixture per Node kind plus the TemplateOr/ActionRef/CodeSource forms and the unknown-kind fallback"
    );
    check_dir::<SubAction>("sub_action", true);
    let kinds: Vec<String> = files
        .iter()
        .map(|f| load(f)["kind"].as_str().expect("kind").to_owned())
        .collect();
    for kind in [
        "op",
        "group",
        "if",
        "switch",
        "while",
        "break_loop",
        "continue_loop",
        "stop",
        "parallel",
        "delay",
        "run_action",
        "code",
        "placeholder",
    ] {
        assert!(
            kinds.iter().any(|k| k == kind),
            "missing fixture for Node kind {kind}"
        );
    }
}

#[test]
fn every_tagged_union_variant_round_trips() {
    check_dir::<Origin>("origin", true);
    assert_eq!(fixture_files("origin").len(), 5);
    check_dir::<Principal>("principal", false);
    assert_eq!(
        fixture_files("principal").len(),
        5,
        "all five Principal variants (§17.1)"
    );
    check_dir::<Fragment>("fragment", false);
    assert_eq!(
        fixture_files("fragment").len(),
        6,
        "five kinds plus the unknown-kind fallback"
    );
    check_dir::<Filter>("filter", true);
    assert_eq!(fixture_files("filter").len(), 10, "every FilterOp");
    check_dir::<ActionRef>("action_ref", true);
    check_dir::<CodeSource>("code_source", true);
    check_dir::<ArgWrites>("arg_writes", false);
}

#[test]
fn documents_round_trip_and_validate_strictly() {
    check_dir::<Action>("action", true);
    check_dir::<Trigger>("trigger", true);
    check_dir::<Queue>("queue", true);
    check_dir::<VariableRecord>("variable", true);
    check_dir::<PendingChange>("pending_change", true);
    check_dir::<StoredDocument>("stored_document", true);
}

#[test]
fn wire_types_round_trip() {
    check_dir::<Event>("event", false);
    check_file::<RpcRequest>("rpc/request.json");
    check_file::<RpcRequest>("rpc/request_string_id.json");
    check_file::<RpcResponse>("rpc/response_ok.json");
    check_file::<RpcResponse>("rpc/response_error.json");
    check_file::<RpcResponse>("rpc/response_parse_error.json");
    check_file::<RpcResponse>("rpc/response_invalid_params.json");
    check_file::<RpcResponse>("rpc/response_null_result.json");
    check_file::<RpcNotification>("rpc/notification_event.json");
    check_file::<Hello>("rpc/hello.json");
    check_file::<SessionIdentifyParams>("rpc/identify_params.json");
    check_file::<SessionIdentifyResult>("rpc/identify_result.json");
    check_file::<SessionReidentifyParams>("rpc/reidentify_params.json");
    check_file::<EventsSubscribeParams>("rpc/events_subscribe.json");
    check_file::<EventsUnsubscribeParams>("rpc/events_unsubscribe.json");
    check_file::<EventsDroppedNotification>("rpc/events_dropped.json");
    check_file::<BatchExecuteParams>("rpc/batch_params.json");
    check_file::<BatchExecuteResult>("rpc/batch_result.json");
    check_file::<DryRunResult>("results/dry_run.json");
    check_file::<DraftResult>("results/draft.json");
    check_file::<DraftResult>("results/draft_action.json");
    check_file::<PluginCallResult>("results/plugin_call.json");
    check_file::<PluginCallParams>("results/plugin_call_params.json");
    check_dir::<AuthContext>("auth", false);
    let notification: EventNotification =
        serde_json::from_value(load("rpc/notification_event.json")["params"].clone())
            .expect("event notification");
    assert_eq!(notification.event.r#type.as_str(), "twitch.subscription");
}

#[test]
fn rpc_response_carries_exactly_one_outcome() {
    // A successful `null` result is a value, not an absence (§8.2): it survives a round trip and the
    // batch form.
    let ok: RpcResponse = serde_json::from_value(load("rpc/response_null_result.json")).unwrap();
    assert_eq!(ok.result(), Some(&Value::Null));
    assert!(ok.error().is_none());
    assert_eq!(
        serde_json::to_value(&ok).unwrap(),
        load("rpc/response_null_result.json")
    );
    let batch: BatchExecuteResult = serde_json::from_value(serde_json::json!({
        "results": [{ "jsonrpc": "2.0", "id": 1, "result": null }]
    }))
    .unwrap();
    assert_eq!(batch.results[0].result(), Some(&Value::Null));
    assert_eq!(
        serde_json::to_value(&batch).unwrap()["results"][0],
        serde_json::json!({ "jsonrpc": "2.0", "id": 1, "result": null })
    );
    // Neither or both members are malformed envelopes, for serde and for the schema alike.
    let schema = validator(root_schema_for::<RpcResponse>().to_value());
    for rel in [
        "invalid/rpc_response_neither.json",
        "invalid/rpc_response_both.json",
    ] {
        let fixture = load(rel);
        assert!(
            serde_json::from_value::<RpcResponse>(fixture.clone()).is_err(),
            "{rel}: serde must reject it"
        );
        assert!(
            !schema.is_valid(&fixture),
            "{rel}: the schema must reject it"
        );
    }
    assert!(
        serde_json::from_value::<RpcResponse>(serde_json::json!({ "jsonrpc": "2.0", "result": 1 }))
            .is_err(),
        "id is required (null when unknown)"
    );
    let null_error = serde_json::json!({ "jsonrpc": "2.0", "id": 1, "result": 1, "error": null });
    assert!(
        serde_json::from_value::<RpcResponse>(null_error.clone()).is_err(),
        "a present `error: null` is a malformed envelope, as the schema says"
    );
    assert!(!schema.is_valid(&null_error));
    let published = codegen::build(Catalog::core()).unwrap();
    assert!(
        published.types["RpcResponse"]["oneOf"].is_array(),
        "protocol.json expresses the exactly-one rule"
    );
}

#[test]
fn unknown_kinds_decode_to_the_documented_fallbacks() {
    // Fragment: an unknown kind is kept whole and renders as its text (§7.2).
    let fixture = load("fragment/unknown_sticker.json");
    let fragment: Fragment = serde_json::from_value(fixture.clone()).expect("fallback");
    match &fragment {
        Fragment::Unknown(u) => {
            assert_eq!(u.kind, "sticker");
            assert_eq!(u.text.as_str(), "hi");
            assert_eq!(u.extra["image"], "x");
        }
        other => panic!("expected the unknown fallback, got {}", other.kind()),
    }
    assert_eq!(fragment.kind(), "sticker");
    assert_eq!(fragment.text().as_str(), "hi");
    assert_eq!(serde_json::to_value(&fragment).unwrap(), fixture);
    let chat: den_protocol::shared::ChatText = serde_json::from_value(serde_json::json!({
        "text": "hi there",
        "fragments": [fixture, { "kind": "text", "text": " there" }]
    }))
    .expect("a chat message with an unknown fragment still decodes");
    assert_eq!(chat.fragments.len(), 2);

    // Node: an unknown kind inside a SubAction decodes as a read-only placeholder that keeps its fields.
    let fixture = load("sub_action/unknown_kind.json");
    let sub_action: SubAction = serde_json::from_value(fixture.clone()).expect("fallback");
    match &sub_action.node {
        den_protocol::domain::Node::Unknown(u) => {
            assert_eq!(u.kind, "future_node");
            assert_eq!(u.fields["future"]["setting"], 1);
            assert!(
                !u.fields.contains_key("id") && !u.fields.contains_key("note"),
                "the SubAction envelope fields are not part of the node"
            );
        }
        other => panic!("expected the unknown fallback, got {}", other.kind()),
    }
    assert_eq!(sub_action.node.kind(), "future_node");
    assert_eq!(serde_json::to_value(&sub_action).unwrap(), fixture);
    let action: Action = serde_json::from_value(serde_json::json!({
        "id": "018f7a8c-0000-7000-8000-0000000000aa",
        "name": "x",
        "enabled": true,
        "nodes": [fixture]
    }))
    .expect("an action containing an unknown node still decodes");
    assert_eq!(action.nodes.len(), 1);

    // The fallback never swallows a malformed known kind: schema and serde both reject it.
    let strict = validator(strict_root_schema_for::<SubAction>().to_value());
    let wire = validator(root_schema_for::<SubAction>().to_value());
    let bad = load("invalid/sub_action_delay_without_ms.json");
    assert!(serde_json::from_value::<SubAction>(bad.clone()).is_err());
    assert!(!wire.is_valid(&bad));
    assert!(!strict.is_valid(&bad));
    let bad = serde_json::json!({ "kind": "emote", "text": "x" });
    assert!(serde_json::from_value::<Fragment>(bad.clone()).is_err());
    assert!(!validator(root_schema_for::<Fragment>().to_value()).is_valid(&bad));
    for kind in den_protocol::domain::Node::KNOWN_KINDS {
        assert!(
            !strict.is_valid(&serde_json::json!({ "id": "018f7a8c-0000-7000-8000-000000000020", "enabled": true, "kind": kind, "bogus": 1 })),
            "known kind {kind} must not fall through to the open fallback branch"
        );
    }
}

#[test]
fn event_payload_parses_on_demand() {
    let event: Event = serde_json::from_value(load("event/unknown_caused_by.json")).expect("event");
    let payload: den_protocol::shared::UnknownEventPayload = event.payload_as().expect("payload");
    assert_eq!(payload.upstream_type, "channel.new_thing");
    assert!(event.meta.caused_by.is_some());
}

#[test]
fn contribution_ref_rule_is_enforced_by_schema_and_code() {
    let trigger = validator(root_schema_for::<Trigger>().to_value());
    for rel in [
        "invalid/trigger_core_with_plugin.json",
        "invalid/trigger_plugin_without_plugin.json",
    ] {
        let fixture = load(rel);
        assert!(
            !trigger.is_valid(&fixture),
            "{rel}: the generated if/then/else clause must reject it"
        );
        let typed: Trigger = serde_json::from_value(fixture).expect("serde accepts the shape");
        assert!(
            typed.contribution.validate().is_err(),
            "{rel}: the save-time validator must reject it"
        );
    }
    let sub_action = validator(root_schema_for::<SubAction>().to_value());
    let fixture = load("invalid/sub_action_op_core_with_plugin.json");
    assert!(
        !sub_action.is_valid(&fixture),
        "Node::Op embeds the same clause"
    );
    let typed: SubAction = serde_json::from_value(fixture).expect("serde accepts the shape");
    match typed.node {
        den_protocol::domain::Node::Op { contribution, .. } => {
            assert!(contribution.validate().is_err())
        }
        other => panic!("expected op, got {}", other.kind()),
    }
}

#[test]
fn strict_schemas_reject_unknown_keys_and_null_optionals() {
    let wire = validator(root_schema_for::<Action>().to_value());
    let strict = validator(strict_root_schema_for::<Action>().to_value());
    let unknown = load("invalid/action_unknown_key.json");
    assert!(
        wire.is_valid(&unknown),
        "wire types ignore unknown fields (§3)"
    );
    assert!(
        !strict.is_valid(&unknown),
        "persisted documents validate strictly (§3)"
    );
    let mut x_key = unknown.clone();
    x_key.as_object_mut().unwrap().remove("colour");
    x_key
        .as_object_mut()
        .unwrap()
        .insert("x-colour".into(), "red".into());
    assert!(strict.is_valid(&x_key), "keys starting with x- are allowed");
    let null_group = load("invalid/action_null_group.json");
    assert!(
        !wire.is_valid(&null_group),
        "absent means not provided; null is not a valid optional (§3)"
    );
}

#[test]
fn dry_run_discriminator_is_literal_true() {
    let schema = validator(root_schema_for::<DryRunResult>().to_value());
    let fixture = load("invalid/dry_run_false.json");
    assert!(!schema.is_valid(&fixture));
    assert!(serde_json::from_value::<DryRunResult>(fixture).is_err());
}

#[test]
fn plugin_manifest_validates_strictly() {
    let schema = codegen::plugin_manifest_schema();
    let v = validator(schema);
    let valid = load("manifest/valid_acme.json");
    assert_valid(&v, &valid, "manifest/valid_acme.json");
    let typed: PluginManifest = serde_json::from_value(valid.clone()).expect("manifest");
    // `x-*` keys pass strict validation but the typed manifest does not retain them (v0).
    let mut without_extensions = valid.clone();
    without_extensions
        .as_object_mut()
        .unwrap()
        .retain(|k, _| !k.starts_with("x-"));
    assert_eq!(
        serde_json::to_value(&typed).unwrap(),
        without_extensions,
        "manifest round trip"
    );
    assert!(den_protocol::version::is_reserved_slug("twitch"));
    for rel in [
        "manifest/invalid_wasm_runtime.json",
        "manifest/invalid_unknown_key.json",
    ] {
        let fixture = load(rel);
        assert!(!v.is_valid(&fixture), "{rel}: must fail strict validation");
        assert!(
            serde_json::from_value::<PluginManifest>(fixture).is_err()
                || rel.contains("unknown_key"),
            "{rel}"
        );
    }
    let reserved: PluginManifest =
        serde_json::from_value(load("manifest/invalid_reserved_slug.json")).expect("shape is fine");
    assert!(
        den_protocol::version::is_reserved_slug(reserved.slug.as_str()),
        "the reserved-slug rule is enforced by the install-time validator, not the schema (§4)"
    );
}

#[test]
fn golden_protocol_json_matches_the_committed_snapshot() {
    let doc = codegen::build(Catalog::core()).expect("core catalog passes lint");
    let generated = doc.to_json_string();
    let committed = std::fs::read_to_string(repo_root().join("schema/protocol.json"))
        .expect("schema/protocol.json");
    assert!(
        generated == committed,
        "schema/protocol.json is stale; run `cargo xtask codegen` and commit the result"
    );
    let plugin = codegen::to_canonical_json(&codegen::plugin_manifest_schema());
    let committed = std::fs::read_to_string(repo_root().join("schema/plugin.schema.json"))
        .expect("schema/plugin.schema.json");
    assert!(
        plugin == committed,
        "schema/plugin.schema.json is stale; run `cargo xtask codegen`"
    );
}

#[test]
fn golden_protocol_json_is_deterministic_and_complete() {
    let a = codegen::build(Catalog::core()).unwrap().to_json_string();
    let b = codegen::build(Catalog::core()).unwrap().to_json_string();
    assert_eq!(a, b, "codegen must be deterministic");
    let doc = codegen::build(Catalog::core()).unwrap();
    assert_eq!(doc.rpc_version, den_protocol::version::RPC_VERSION);
    assert!(doc.operations.contains_key("plugins.call"));
    for name in [
        "Event",
        "UserRef",
        "ChatText",
        "Money",
        "ContributionRef",
        "DryRunResult",
        "DraftResult",
        "AuthContext",
        "Principal",
        "RpcError",
        "PluginManifest",
        "Action",
        "SubAction",
    ] {
        assert!(
            doc.types.contains_key(name),
            "shared type {name} must be published under types"
        );
    }
    assert_eq!(doc.errors.len(), 17);
    assert!(doc.scopes.iter().any(|s| s == "admin"));
    assert_eq!(doc.close_codes.len(), 9);
    assert!(
        doc.events.is_empty(),
        "core events are registered by their owning specs"
    );
    let json = serde_json::to_value(&doc).unwrap();
    let mut refs = Vec::new();
    collect_refs(&json, &mut refs);
    for r in refs {
        let name = r
            .strip_prefix("#/types/")
            .unwrap_or_else(|| panic!("unexpected $ref form {r}"));
        assert!(doc.types.contains_key(name), "dangling $ref {r}");
    }
}

fn collect_refs(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            if let Some(Value::String(r)) = map.get("$ref") {
                out.push(r.clone());
            }
            map.values().for_each(|v| collect_refs(v, out));
        }
        Value::Array(items) => items.iter().for_each(|v| collect_refs(v, out)),
        _ => {}
    }
}

#[test]
fn examples_validate_against_exactly_one_result_shape() {
    let doc = codegen::build(Catalog::core()).unwrap();
    let entry = &doc.operations["plugins.call"];
    let params = validator(doc.schema_at("/operations/plugins.call/params"));
    let normal = validator(doc.schema_at("/operations/plugins.call/result"));
    let dry_run = validator(doc.schema_at("/operations/plugins.call/dryRunResult"));
    assert!(
        entry.draft_result.is_none(),
        "plugins.call has no draft result (§8.1)"
    );
    assert_eq!(entry.mcp_tool_name.as_deref(), Some("plugins_call"));
    assert!(
        entry
            .def
            .params
            .get("properties")
            .unwrap()
            .get("dryRun")
            .is_some(),
        "codegen adds dryRun"
    );
    assert!(
        entry
            .def
            .params
            .get("properties")
            .unwrap()
            .get("approvalId")
            .is_some(),
        "dynamic tier adds approvalId"
    );
    for example in &entry.def.examples {
        assert_valid(&params, &example.params, &example.title);
        match example.shape {
            ResultShape::Normal => {
                assert_valid(&normal, &example.result, &example.title);
                assert!(
                    !dry_run.is_valid(&example.result),
                    "normal result must not match DryRunResult"
                );
            }
            ResultShape::DryRun => {
                assert_valid(&dry_run, &example.result, &example.title);
                assert!(
                    !normal.is_valid(&example.result),
                    "dry-run result must not match the normal shape"
                );
            }
            ResultShape::Draft => unreachable!(),
        }
    }
    assert!(
        !params.is_valid(&serde_json::json!({ "plugin": "acme", "method": "x" })),
        "plugin ID pattern"
    );
}

#[test]
fn typescript_bindings_on_disk_are_current() {
    let scratch = std::env::temp_dir().join(format!("den-protocol-ts-{}", std::process::id()));
    std::fs::create_dir_all(&scratch).unwrap();
    let files = codegen::generate_typescript(&scratch).expect("ts export");
    let _ = std::fs::remove_dir_all(&scratch);
    let gen_dir = repo_root().join("sdk/ts/src/gen");
    assert!(files.contains_key(Path::new("SubAction.ts")));
    assert!(files.contains_key(Path::new("index.ts")));
    for (rel, content) in &files {
        let on_disk = std::fs::read_to_string(gen_dir.join(rel))
            .unwrap_or_else(|e| panic!("{}: {e}; run `cargo xtask codegen`", rel.display()));
        assert!(
            on_disk == *content,
            "{} is stale; run `cargo xtask codegen`",
            rel.display()
        );
        assert!(
            content.starts_with("// @generated"),
            "{}: missing marker",
            rel.display()
        );
    }
    let sub_action = &files[Path::new("SubAction.ts")];
    assert!(
        sub_action.contains("\"kind\": \"op\""),
        "flattened Node::Op reaches TS (§17.1)"
    );
    assert!(
        sub_action.contains("plugin?: PluginId"),
        "flattened ContributionRef reaches TS (§17.1)"
    );
    assert!(
        sub_action.contains("UnknownNode"),
        "the unknown-kind fallback reaches TS (§7.2)"
    );
    assert!(
        files[Path::new("Fragment.ts")].contains("UnknownFragment"),
        "the unknown-kind fallback reaches TS (§7.2)"
    );
    let response = &files[Path::new("RpcResponse.ts")];
    assert!(
        response.contains("result: JsonValue") && response.contains("error: RpcError"),
        "RpcResponse is a union of the two outcomes (§8.2): {response}"
    );
}
