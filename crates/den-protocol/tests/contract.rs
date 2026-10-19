//! Contract tests (Spec 02 §17.1, §17.2, §17.12, §17.14): every committed fixture round-trips through
//! serde unchanged and validates against the generated schema; invalid fixtures are rejected; the
//! `protocol.json` and `plugin.schema.json` on disk are the golden snapshots of this crate.

use std::path::{Path, PathBuf};

use den_protocol::auth::{AuthContext, Principal};
use den_protocol::catalog::{ArgWrites, Catalog, ResultShape};
use den_protocol::codegen;
use den_protocol::domain::{
    Action, ActionRef, CodeSource, Filter, Node, OnError, Origin, PendingChange, Queue,
    StoredDocument, SubAction, Trigger, UpdatedBy, VariableRecord,
};
use den_protocol::events::Event;
use den_protocol::manifest::{ManifestError, PluginManifest, Runtime};
use den_protocol::records::{Command, CommandMode, Run, Timer};
use den_protocol::results::{DraftResult, DryRunResult, PluginCallParams, PluginCallResult};
use den_protocol::rpc::{
    BatchExecuteParams, BatchExecuteResult, EventNotification, EventsDroppedNotification,
    EventsSubscribeParams, EventsUnsubscribeParams, Hello, RpcNotification, RpcRequest,
    RpcResponse, SessionIdentifyParams, SessionIdentifyResult, SessionReidentifyParams,
};
use den_protocol::schema::{root_schema_for, strict_root_schema_for};
use den_protocol::secret::expose_secrets;
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
        // The wire form: a `Secret` carries its value only inside an explicit exposure (§3).
        let back = expose_secrets(|| serde_json::to_value(&typed)).expect("serialize");
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
        expose_secrets(|| serde_json::to_value(&typed)).expect("serialize"),
        fixture,
        "{rel}: round trip changed the document"
    );
    assert_valid(&wire, &fixture, rel);
}

/// Round 6: the token of `session.identify` is a `Secret<String>` (§3). Outside an explicit exposure the
/// wire form withholds it, so a client library can never send or log it by accident; `Debug` never shows
/// it; the fixture itself still round-trips inside `expose_secrets` (above).
#[test]
fn identify_params_withhold_the_token_unless_exposed() {
    let fixture = load("rpc/identify_params.json");
    let typed: SessionIdentifyParams = serde_json::from_value(fixture.clone()).unwrap();
    assert!(
        typed
            .token
            .as_ref()
            .is_some_and(|t| t.expose().starts_with("sdt_"))
    );
    let withheld = serde_json::to_value(&typed).unwrap();
    let mut expected = fixture.clone();
    expected["token"] = Value::String("***".into());
    assert_eq!(withheld, expected);
    assert!(!format!("{typed:?}").contains("sdt_"));
    assert_eq!(
        expose_secrets(|| serde_json::to_value(&typed)).unwrap(),
        fixture
    );
}

#[test]
fn every_node_kind_round_trips() {
    let files = fixture_files("sub_action");
    assert_eq!(
        files.len(),
        25,
        "one fixture per Node kind plus the TemplateOr/ActionRef/CodeSource forms, the unknown-kind fallback, \
         `onError`, both ForEach forms and the §17.12 engine-namespace ops"
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
        "for_each",
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
        assert!(
            Node::KNOWN_KINDS.contains(&kind),
            "{kind} must be a known kind"
        );
    }
}

/// Spec 02 r6 (§6.4, §17.1): `onError` on a node, `ForEach` with a literal and with a templated
/// collection, and a `Case` with two values.
#[test]
fn r6_node_shapes_decode() {
    let node: SubAction =
        serde_json::from_value(load("sub_action/on_error_continue.json")).unwrap();
    assert_eq!(node.on_error, OnError::Continue);
    let plain: SubAction = serde_json::from_value(load("sub_action/op_core.json")).unwrap();
    assert_eq!(plain.on_error, OnError::Stop, "the default is stop (A1)");
    assert!(
        serde_json::to_value(&plain)
            .unwrap()
            .get("onError")
            .is_none(),
        "the default is omitted on the wire so existing documents round-trip unchanged"
    );

    let literal: SubAction =
        serde_json::from_value(load("sub_action/for_each_literal.json")).unwrap();
    match &literal.node {
        Node::ForEach {
            items,
            item_var,
            index_var,
            max_iterations,
            children,
        } => {
            assert_eq!(items.as_value(), Some(&serde_json::json!(["alice", "bob"])));
            assert_eq!(item_var, "name");
            assert_eq!(index_var.as_deref(), Some("i"));
            assert_eq!(*max_iterations, Some(50));
            assert_eq!(children.len(), 1);
        }
        other => panic!("expected for_each, got {}", other.kind()),
    }
    let templated: SubAction =
        serde_json::from_value(load("sub_action/for_each_template.json")).unwrap();
    match &templated.node {
        Node::ForEach { items, .. } => {
            assert_eq!(
                items.as_template().map(|t| t.as_str()),
                Some("{globals.raiders}")
            );
        }
        other => panic!("expected for_each, got {}", other.kind()),
    }

    let switch: SubAction = serde_json::from_value(load("sub_action/switch.json")).unwrap();
    match &switch.node {
        Node::Switch { cases, .. } => {
            assert_eq!(cases[0].values.len(), 2, "a Case with two values (§17.1)");
        }
        other => panic!("expected switch, got {}", other.kind()),
    }
    let strict = validator(strict_root_schema_for::<SubAction>().to_value());
    let empty = load("invalid/switch_case_without_values.json");
    assert!(
        !strict.is_valid(&empty),
        "a case needs at least one value (§6.4)"
    );
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
    check_dir::<UpdatedBy>("updated_by", true);
    assert_eq!(
        fixture_files("updated_by").len(),
        5,
        "all five UpdatedBy kinds (§6.1, §17.1)"
    );
    check_dir::<CommandMode>("command_mode", true);
    assert_eq!(
        fixture_files("command_mode").len(),
        2,
        "both CommandMode kinds (§6.5, §17.1)"
    );
}

#[test]
fn documents_round_trip_and_validate_strictly() {
    check_dir::<Action>("action", true);
    check_dir::<Trigger>("trigger", true);
    check_dir::<Queue>("queue", true);
    check_dir::<VariableRecord>("variable", true);
    check_dir::<PendingChange>("pending_change", true);
    check_dir::<StoredDocument>("stored_document", true);
    // §6.5 engine records: Command and Timer are configuration documents, Run a history record.
    check_dir::<Command>("command", true);
    check_dir::<Timer>("timer", true);
    check_dir::<Run>("run", false);
    let strict = validator(strict_root_schema_for::<Timer>().to_value());
    assert!(
        !strict.is_valid(&load("invalid/timer_interval_too_short.json")),
        "intervalMs is at least 1000 (§6.5)"
    );
    let strict = validator(strict_root_schema_for::<Command>().to_value());
    assert!(
        !strict.is_valid(&load("invalid/command_without_aliases.json")),
        "a command has at least one alias (§6.5)"
    );
    let pending: PendingChange =
        serde_json::from_value(load("pending_change/delete.json")).unwrap();
    assert_eq!(
        pending.target_kind, "action",
        "renamed from `kind` in r6 (§6.1)"
    );
    let record: VariableRecord = serde_json::from_value(load("variable/record.json")).unwrap();
    assert!(record.persisted && !record.untrusted);
    assert_eq!(record.updated_by, UpdatedBy::User);
    let queue: Queue = serde_json::from_value(load("queue/bounded.json")).unwrap();
    assert_eq!(queue.max_run_ms, Some(60_000), "A9");
    for kind in [
        "command",
        "timer",
        "view_state",
        "export_meta",
        "pending_change",
    ] {
        assert!(StoredDocument::is_known_kind(kind), "{kind} (§7.3)");
    }
}

/// §17.12: for every namespace the engine registers, a core reference without `plugin` saves, the same
/// reference with `plugin` set is rejected by the generated `if/then/else` clause and by the save-time
/// validator, and a plugin cannot claim the slug.
#[test]
fn engine_namespaces_are_reserved_for_core() {
    for slug in [
        "runs", "signals", "commands", "timers", "schedule", "settings", "catalog", "http",
    ] {
        assert!(
            den_protocol::version::is_reserved_slug(slug),
            "{slug} is reserved (§4)"
        );
    }
    let trigger_schema = validator(root_schema_for::<Trigger>().to_value());
    for rel in [
        "trigger/core_commands_triggered.json",
        "trigger/core_schedule_timer_fired.json",
    ] {
        let fixture = load(rel);
        assert_valid(&trigger_schema, &fixture, rel);
        let typed: Trigger = serde_json::from_value(fixture.clone()).unwrap();
        assert!(typed.contribution.is_core() && typed.contribution.validate().is_ok());
        let mut with_plugin = fixture;
        with_plugin["plugin"] = serde_json::json!("io.github.alice.acmelive");
        assert!(
            !trigger_schema.is_valid(&with_plugin),
            "{rel}: a core reference with `plugin` fails the generated clause"
        );
        let typed: Trigger = serde_json::from_value(with_plugin).unwrap();
        assert!(typed.contribution.validate().is_err());
    }
    let node_schema = validator(root_schema_for::<SubAction>().to_value());
    for rel in [
        "sub_action/op_core_signals_wait.json",
        "sub_action/op_core_timers_set_state.json",
        "sub_action/op_core_runs_replay.json",
    ] {
        let fixture = load(rel);
        assert_valid(&node_schema, &fixture, rel);
        let mut with_plugin = fixture;
        with_plugin["plugin"] = serde_json::json!("io.github.alice.acmelive");
        assert!(
            !node_schema.is_valid(&with_plugin),
            "{rel}: a core op with `plugin` fails the generated clause"
        );
        let typed: SubAction = serde_json::from_value(with_plugin).unwrap();
        match typed.node {
            Node::Op { contribution, .. } => assert!(contribution.validate().is_err()),
            other => panic!("expected op, got {}", other.kind()),
        }
    }
    let manifest_schema = validator(codegen::plugin_manifest_schema());
    let valid = load("manifest/valid_acme.json");
    for slug in ["runs", "signals", "commands", "timers", "schedule"] {
        let mut claimed = valid.clone();
        claimed["slug"] = serde_json::json!(slug);
        assert!(
            !manifest_schema.is_valid(&claimed),
            "a manifest claiming the reserved slug {slug} fails schema validation (§4)"
        );
        let typed: PluginManifest = serde_json::from_value(claimed).unwrap();
        assert_eq!(
            typed.validate(),
            Err(ManifestError::ReservedSlug(slug.into())),
            "and is refused at install"
        );
    }
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
    let fixture = load("manifest/invalid_reserved_slug.json");
    assert!(
        !v.is_valid(&fixture),
        "a reserved slug fails manifest validation (§4, r6)"
    );
    let reserved: PluginManifest = serde_json::from_value(fixture).expect("shape is fine");
    assert_eq!(
        reserved.validate(),
        Err(ManifestError::ReservedSlug("twitch".into())),
        "and the install-time validator refuses it too"
    );
    let valid: PluginManifest = serde_json::from_value(load("manifest/valid_acme.json")).unwrap();
    assert_eq!(valid.validate(), Ok(()));
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
        "OnError",
        "UpdatedBy",
        "Run",
        "RunState",
        "CancelReason",
        "RunError",
        "RunWarning",
        "Command",
        "CommandMode",
        "Cooldown",
        "Permissions",
        "UserKey",
        "CounterPolicy",
        "Timer",
        "GroupId",
    ] {
        assert!(
            doc.types.contains_key(name),
            "shared type {name} must be published under types"
        );
    }
    assert_eq!(doc.errors.len(), 18);
    assert!(doc.errors.contains_key("INTERNAL"), "A8 (§8.5)");
    assert!(doc.scopes.iter().any(|s| s == "admin"));
    for scope in ["events:simulate", "events:emit"] {
        assert!(doc.scopes.iter().any(|s| s == scope), "{scope} (§8.7)");
    }
    assert_eq!(
        doc.dynamic_tier_operations,
        vec!["actions.run", "plugins.call", "runs.replay"],
        "the constant dynamic-tier set is published (§8.1, A8)"
    );
    for slug in [
        "runs", "signals", "commands", "timers", "schedule", "settings", "catalog", "http",
    ] {
        assert!(doc.reserved_slugs.iter().any(|s| s == slug), "{slug}");
    }
    let node_kinds = &doc.types["UnknownNode"]["properties"]["kind"]["not"]["enum"];
    assert!(
        node_kinds
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("for_each")),
        "the fallback excludes the new kind"
    );
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

// Exercise the real wire deserializers, including serde's buffered `from_value` path.
fn check_string_tag<T: DeserializeOwned + JsonSchema>(name: &str, kind: &str) {
    let rel = format!("invalid/{name}_integer_kind.json");
    let fixture = load(&rel);
    let wire = validator(root_schema_for::<T>().to_value());
    let strict = validator(strict_root_schema_for::<T>().to_value());
    let mut valid = fixture.clone();
    valid["kind"] = Value::String(kind.into());
    assert_valid(&wire, &valid, &rel);
    assert_valid(&strict, &valid, &rel);
    serde_json::from_str::<T>(&valid.to_string()).unwrap_or_else(|e| panic!("{rel}: {e}"));
    serde_json::from_value::<T>(valid).unwrap_or_else(|e| panic!("{rel}: {e}"));
    let mut missing = fixture.clone();
    missing.as_object_mut().unwrap().remove("kind");
    assert!(!wire.is_valid(&missing));
    assert!(!strict.is_valid(&missing));
    assert!(serde_json::from_str::<T>(&missing.to_string()).is_err());
    assert!(serde_json::from_value::<T>(missing).is_err());
    for tag in [
        serde_json::json!(0),
        serde_json::json!(999),
        serde_json::json!(-1),
        serde_json::json!(1.0),
        serde_json::json!(true),
        Value::Null,
        serde_json::json!([]),
        serde_json::json!({}),
        fixture["kind"].clone(),
    ] {
        let mut bad = fixture.clone();
        bad["kind"] = tag;
        assert!(
            !wire.is_valid(&bad),
            "{rel}: wire schema accepted non-string tag"
        );
        assert!(
            !strict.is_valid(&bad),
            "{rel}: strict schema accepted non-string tag"
        );
        assert!(
            serde_json::from_str::<T>(&bad.to_string()).is_err(),
            "{rel}: wire serde accepted non-string tag"
        );
        assert!(
            serde_json::from_value::<T>(bad).is_err(),
            "{rel}: buffered serde accepted non-string tag"
        );
    }
}

macro_rules! string_tag_test {
    ($test:ident, $ty:ty, $fixture:literal, $kind:literal) => {
        #[test]
        fn $test() {
            check_string_tag::<$ty>($fixture, $kind);
        }
    };
}
string_tag_test!(node_requires_string_kind, Node, "node", "break_loop");
string_tag_test!(fragment_requires_string_kind, Fragment, "fragment", "text");
string_tag_test!(origin_requires_string_kind, Origin, "origin", "user");
string_tag_test!(
    updated_by_requires_string_kind,
    UpdatedBy,
    "updated_by",
    "user"
);
string_tag_test!(
    action_ref_requires_string_kind,
    ActionRef,
    "action_ref",
    "name"
);
string_tag_test!(
    code_source_requires_string_kind,
    CodeSource,
    "code_source",
    "inline"
);
string_tag_test!(
    principal_requires_string_kind,
    Principal,
    "principal",
    "system"
);
string_tag_test!(
    arg_writes_requires_string_kind,
    ArgWrites,
    "arg_writes",
    "none"
);
string_tag_test!(
    command_mode_requires_string_kind,
    CommandMode,
    "command_mode",
    "regex"
);
string_tag_test!(
    runtime_requires_string_kind,
    Runtime,
    "runtime",
    "declarative"
);

#[test]
fn string_tag_guard_covers_nested_wire_values_and_direct_deserialization() {
    use serde::Deserialize;
    let bad = load("invalid/fragment_integer_kind.json");
    assert!(
        Fragment::deserialize(&mut serde_json::Deserializer::from_str(&bad.to_string())).is_err()
    );
    let chat = serde_json::json!({"text": "t", "fragments": [bad]});
    assert!(serde_json::from_value::<den_protocol::ChatText>(chat.clone()).is_err());
    assert!(serde_json::from_str::<den_protocol::ChatText>(&chat.to_string()).is_err());

    let mut bad = load("sub_action/break_loop.json");
    bad["kind"] = serde_json::json!(6);
    assert!(Node::deserialize(&mut serde_json::Deserializer::from_str(&bad.to_string())).is_err());
    assert!(serde_json::from_value::<SubAction>(bad.clone()).is_err());
    assert!(serde_json::from_str::<SubAction>(&bad.to_string()).is_err());
    let mut action = load("action/cheer_alert.json");
    action["nodes"] = serde_json::json!([bad]);
    assert!(serde_json::from_value::<Action>(action.clone()).is_err());
    assert!(serde_json::from_str::<Action>(&action.to_string()).is_err());

    // Do not rewrite opaque future fields, and preserve unknown-string fallbacks on both paths.
    let future = serde_json::json!({"kind": "future", "text": "t", "payload": {"kind": 0}});
    for value in [
        serde_json::to_value(serde_json::from_str::<Fragment>(&future.to_string()).unwrap())
            .unwrap(),
        serde_json::to_value(serde_json::from_value::<Fragment>(future.clone()).unwrap()).unwrap(),
        serde_json::to_value(serde_json::from_str::<Node>(&future.to_string()).unwrap()).unwrap(),
        serde_json::to_value(serde_json::from_value::<Node>(future.clone()).unwrap()).unwrap(),
    ] {
        assert_eq!(value, future);
    }
    for raw in [
        r#"{"kind":0,"kind":"text","text":"t"}"#,
        r#"{"kind":"text","kind":0,"text":"t"}"#,
    ] {
        assert!(serde_json::from_str::<Fragment>(raw).is_err());
    }
}
