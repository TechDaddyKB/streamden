//! Codegen rules (Spec 02 §9.6, §9.7, §10.1): template-mode derivation, reserved params and the
//! catalog lint, exercised with synthetic operations.

use den_protocol::catalog::{
    ArgWrites, Catalog, EventDef, Exposure, OperationDef, OperationExample, ResultShape,
};
use den_protocol::codegen::{self, TemplateMode, resolve_template_mode};
use den_protocol::ids::{EventType, OperationName};
use den_protocol::shared::RiskTier;
use schemars::Schema;
use serde_json::{Value, json};

fn op(name: &str, tier: u8, params: Value, result: Value) -> OperationDef {
    OperationDef {
        name: OperationName::new(name).unwrap(),
        title: "T".into(),
        description: "Test. An operation.".into(),
        category: vec!["Test".into()],
        params: Schema::try_from(params).unwrap(),
        result: Schema::try_from(result).unwrap(),
        tier: RiskTier::new(tier).unwrap(),
        scopes: vec![],
        expose: Exposure {
            rpc: true,
            sub_action: true,
            mcp: true,
        },
        idempotent: false,
        writes_args: ArgWrites::None,
        external: false,
        timeout_ms: None,
        platforms: None,
        high_volume_result: false,
        since: "0.1.0".into(),
        deprecated: None,
        examples: vec![OperationExample {
            title: "normal".into(),
            params: json!({}),
            shape: ResultShape::Normal,
            result: json!({}),
            authored: None,
        }],
    }
}

fn with_dry_run(mut def: OperationDef) -> OperationDef {
    def.examples.push(OperationExample {
        title: "dry run".into(),
        params: json!({ "dryRun": true }),
        shape: ResultShape::DryRun,
        result: json!({ "dryRun": true, "plan": [], "warnings": [], "wouldRequireApproval": false, "effectiveTier": 1 }),
        authored: None,
    });
    def
}

fn lint_messages(catalog: &Catalog) -> Vec<String> {
    codegen::lint(catalog)
        .into_iter()
        .map(|e| e.to_string())
        .collect()
}

#[test]
fn template_modes_follow_the_defaults_table() {
    let types = Default::default();
    assert_eq!(
        resolve_template_mode(&json!({ "type": "number" }), &types),
        TemplateMode::Value
    );
    assert_eq!(
        resolve_template_mode(&json!({ "type": "integer" }), &types),
        TemplateMode::Value
    );
    assert_eq!(
        resolve_template_mode(&json!({ "type": "boolean" }), &types),
        TemplateMode::Value
    );
    assert_eq!(
        resolve_template_mode(&json!({ "type": "string", "enum": ["a"] }), &types),
        TemplateMode::Value
    );
    assert_eq!(
        resolve_template_mode(&json!({ "type": "string" }), &types),
        TemplateMode::Literal
    );
    assert_eq!(
        resolve_template_mode(
            &json!({ "type": "string", "x-ui": { "variables": true } }),
            &types
        ),
        TemplateMode::Text
    );
    assert_eq!(
        resolve_template_mode(&json!({ "type": "array" }), &types),
        TemplateMode::Literal
    );
    assert_eq!(
        resolve_template_mode(
            &json!({ "type": "number", "x-den-template": false }),
            &types
        ),
        TemplateMode::Literal
    );
    assert_eq!(
        resolve_template_mode(
            &json!({ "type": "string", "x-den-template": "value" }),
            &types
        ),
        TemplateMode::Value
    );
}

#[test]
fn authoring_schema_widens_value_leaves_only() {
    let params = json!({
        "type": "object",
        "properties": {
            "volume": { "type": "integer", "minimum": 0, "maximum": 100, "description": "Volume" },
            "scene": { "type": "string", "enum": ["main", "brb"] },
            "message": { "type": "string", "x-ui": { "variables": true } },
            "body": { "type": "string" },
            "mute": { "type": "boolean" },
            "source": { "type": "object", "properties": { "name": { "type": "string", "x-ui": { "variables": true } } } },
            "tags": { "type": "array", "items": { "type": "string" } }
        },
        "required": ["volume"]
    });
    let (invocation, authoring) =
        codegen::derive_param_schemas(&Schema::try_from(params).unwrap(), &Default::default());
    let inv = invocation.to_value();
    let aut = authoring.to_value();
    assert_eq!(
        inv["properties"]["volume"]["x-den-template"],
        json!("value")
    );
    assert_eq!(
        inv["properties"]["volume"]["type"],
        json!("integer"),
        "invocation stays concrete"
    );
    assert_eq!(
        aut["properties"]["volume"]["anyOf"][0]["maximum"],
        json!(100)
    );
    assert_eq!(
        aut["properties"]["volume"]["anyOf"][1]["x-den-template"],
        json!("value")
    );
    assert_eq!(
        aut["properties"]["volume"]["description"],
        json!("Volume"),
        "description kept on the widened leaf"
    );
    assert!(
        aut["properties"]["scene"]["anyOf"].is_array(),
        "enum strings are value mode"
    );
    assert!(aut["properties"]["mute"]["anyOf"].is_array());
    assert_eq!(
        aut["properties"]["message"]["x-den-template"],
        json!("text")
    );
    assert_eq!(
        aut["properties"]["message"]["type"],
        json!("string"),
        "text leaves stay strings"
    );
    assert_eq!(aut["properties"]["body"]["x-den-template"], json!(false));
    assert_eq!(
        aut["properties"]["source"]["properties"]["name"]["x-den-template"],
        json!("text")
    );
    assert_eq!(aut["properties"]["tags"]["x-den-template"], json!(false));
    assert_eq!(
        aut["required"],
        json!(["volume"]),
        "required-ness unchanged"
    );
    assert!(aut.get("anyOf").is_none(), "anyOf never at the root");

    let v = jsonschema::validator_for(&aut).unwrap();
    assert!(v.is_valid(
        &json!({ "volume": "{= base * 2}", "scene": "{globals.scene}", "mute": "{flag}" })
    ));
    assert!(v.is_valid(&json!({ "volume": 40 })));
    assert!(
        !v.is_valid(&json!({ "volume": "40" })),
        "a plain string is not a template"
    );
    assert!(
        !v.is_valid(&json!({ "volume": "{= x" })),
        "an unterminated form is rejected"
    );
    let inv_v = jsonschema::validator_for(&inv).unwrap();
    assert!(
        !inv_v.is_valid(&json!({ "volume": "{amount}" })),
        "direct calls never carry templates (§10.1)"
    );
}

#[test]
fn enums_emitted_as_one_of_consts_are_value_mode() {
    let mut types = std::collections::BTreeMap::new();
    types.insert("Os".to_owned(), json!({ "oneOf": [{ "type": "string", "const": "windows" }, { "type": "string", "const": "linux" }] }));
    types.insert("Tree".to_owned(), json!({ "type": "object", "properties": { "child": { "$ref": "#/types/Tree" }, "n": { "type": "integer" } } }));
    assert_eq!(
        resolve_template_mode(&json!({ "$ref": "#/types/Os" }), &types),
        TemplateMode::Value
    );
    assert_eq!(
        resolve_template_mode(&json!({ "type": "string", "const": "x" }), &types),
        TemplateMode::Literal,
        "a const is a discriminator or fixed value, never a template"
    );
    let params = json!({
        "type": "object",
        "properties": {
            "os": { "$ref": "#/types/Os" },
            "tree": { "$ref": "#/types/Tree" },
            "blob": { "type": "object", "properties": { "a": { "type": "integer" } }, "x-den-template": "value" }
        }
    });
    let (_, authoring) = codegen::derive_param_schemas(&Schema::try_from(params).unwrap(), &types);
    let aut = authoring.to_value();
    assert!(
        aut["properties"]["os"]["anyOf"].is_array(),
        "a $ref to a oneOf-const enum is widened"
    );
    assert!(
        aut["properties"]["blob"]["anyOf"].is_array(),
        "an explicit value-mode object is widened"
    );
    assert_eq!(
        aut["properties"]["tree"]["properties"]["n"]["x-den-template"],
        json!("value"),
        "one level inlined"
    );
    assert_eq!(
        aut["properties"]["tree"]["properties"]["child"]["$ref"],
        json!("#/types/Tree"),
        "the recursive edge keeps its $ref"
    );
    let root = json!({ "$ref": "#/params", "params": aut, "types": types });
    let v = jsonschema::validator_for(&root).unwrap();
    assert!(v.is_valid(&json!({ "os": "{globals.os}", "blob": "{args.blob}" })));
}

#[test]
fn reserved_params_and_result_keys_are_generated_per_tier() {
    let mut catalog = Catalog::new();
    catalog
        .register_operation(op(
            "core.read",
            0,
            json!({ "type": "object", "properties": {} }),
            json!({ "type": "array" }),
        ))
        .unwrap();
    catalog
        .register_operation(with_dry_run(op(
            "core.write",
            1,
            json!({ "type": "object", "properties": {} }),
            json!({ "type": "object", "properties": { "ok": { "type": "boolean" } } }),
        )))
        .unwrap();
    let mut cfg = with_dry_run(op(
        "core.config",
        2,
        json!({ "type": "object", "properties": {} }),
        json!({ "type": "object" }),
    ));
    cfg.examples.push(OperationExample {
        title: "draft".into(),
        params: json!({}),
        shape: ResultShape::Draft,
        result: json!({ "draft": { "id": "018f7a8c-0000-7000-8000-000000000001", "kind": "pending_change", "operation": "core.config" }, "plan": [], "warnings": [] }),
        authored: None,
    });
    catalog.register_operation(cfg).unwrap();
    catalog
        .register_operation(with_dry_run(op(
            "core.external",
            3,
            json!({ "type": "object", "properties": {} }),
            json!({ "type": "object" }),
        )))
        .unwrap();
    let doc = codegen::build(catalog).unwrap();

    let props = |name: &str| {
        doc.operations[name]
            .def
            .params
            .get("properties")
            .cloned()
            .unwrap()
    };
    assert!(
        props("core.read").get("dryRun").is_none(),
        "tier 0 has no dryRun"
    );
    assert!(props("core.write").get("dryRun").is_some());
    assert!(props("core.write").get("approvalId").is_none());
    assert!(
        props("core.external").get("approvalId").is_some(),
        "tier 3 gets approvalId"
    );
    assert!(doc.operations["core.read"].dry_run_result.is_none());
    assert!(doc.operations["core.write"].dry_run_result.is_some());
    assert!(doc.operations["core.write"].draft_result.is_none());
    assert!(
        doc.operations["core.config"].draft_result.is_some(),
        "tier 2 gets draftResult"
    );
    let result = doc.operations["core.write"].def.result.as_value().clone();
    assert_eq!(
        result["properties"]["dryRun"],
        json!(false),
        "tier ≥ 1 results forbid dryRun"
    );
    assert_eq!(result["properties"]["draft"], json!(false));
    assert_eq!(
        doc.operations["core.read"].def.result.get("properties"),
        None,
        "tier-0 result left untouched"
    );
    assert_eq!(
        doc.operations["core.external"].mcp_tool_name.as_deref(),
        Some("core_external")
    );
}

#[test]
fn lint_catches_the_spec_rules() {
    let mut catalog = Catalog::new();
    assert!(
        catalog
            .register_operation(op(
                "session.identify",
                0,
                json!({ "type": "object" }),
                json!({})
            ))
            .is_err(),
        "protocol methods are refused"
    );

    let mut bad = op(
        "core.bad",
        1,
        json!({ "type": "object", "properties": { "dryRun": { "type": "boolean" }, "Weird Name!": { "type": "string" } } }),
        json!({ "type": "string" }),
    );
    bad.examples.clear();
    catalog.register_operation(bad).unwrap();
    let mut dangerous = with_dry_run(op(
        "core.code",
        4,
        json!({ "type": "object" }),
        json!({ "type": "object" }),
    ));
    dangerous.expose.mcp = true;
    catalog.register_operation(dangerous).unwrap();
    let mut union_root = with_dry_run(op(
        "core.union",
        1,
        json!({ "oneOf": [{ "type": "object" }] }),
        json!({ "type": "object", "properties": { "draft": {} } }),
    ));
    union_root.description = "".into();
    catalog.register_operation(union_root).unwrap();
    catalog.register_operation(with_dry_run(op("core.brace_enum", 1, json!({ "type": "object", "properties": { "scene": { "type": "string", "enum": ["{x}"] }, "mode": { "type": "integer", "x-den-template": "bogus" }, "n": { "type": "number", "x-den-template": "text" } } }), json!({ "type": "object" })))).unwrap();
    catalog
        .register_operation(op(
            "core.read_with_dry_run",
            0,
            json!({ "type": "object" }),
            json!({}),
        ))
        .unwrap();
    let last = catalog.operations().len();
    assert_eq!(last, 5);

    catalog
        .register_event(EventDef {
            r#type: EventType::new("core.happened").unwrap(),
            title: "".into(),
            description: "d".into(),
            category: vec![],
            payload_version: 0,
            payload: Schema::try_from(
                json!({ "type": "object", "properties": { "event": {}, "amount": {} } }),
            )
            .unwrap(),
            filters: vec![den_protocol::catalog::FilterableField {
                field: "missing".into(),
                ops: vec![],
                options_from: None,
                specificity: 0,
                case_insensitive: false,
            }],
            high_volume: false,
            critical: false,
            examples: vec![],
            authored_examples: vec![],
        })
        .unwrap();

    let msgs = lint_messages(&catalog);
    let has = |needle: &str| msgs.iter().any(|m| m.contains(needle));
    assert!(has("reserved param `dryRun`"), "{msgs:?}");
    assert!(has("MCP property name \"Weird Name!\""), "{msgs:?}");
    assert!(
        has("result (tier ≥ 1) root must be `type: object`"),
        "{msgs:?}"
    );
    assert!(has("needs at least one example"), "{msgs:?}");
    assert!(
        has("tier ≥ 1 operations need a dry-run example"),
        "{msgs:?}"
    );
    assert!(
        has("core tier-4 operation must not be marked `expose.mcp`"),
        "{msgs:?}"
    );
    assert!(has("params root must not use `oneOf`"), "{msgs:?}");
    assert!(has("reserved result key `draft`"), "{msgs:?}");
    assert!(has("missing description"), "{msgs:?}");
    assert!(has("enum member starts with `{`"), "{msgs:?}");
    assert!(has("x-den-template must be false"), "{msgs:?}");
    assert!(has("applies to `type: string` leaves only"), "{msgs:?}");
    assert!(has("event:core.happened: missing title"), "{msgs:?}");
    assert!(has("payloadVersion starts at 1"), "{msgs:?}");
    assert!(has("reserved run-context root \"event\""), "{msgs:?}");
    assert!(has("filter field \"missing\""), "{msgs:?}");
    assert!(has("needs at least one example payload"), "{msgs:?}");
    assert!(
        matches!(codegen::build(catalog), Err(codegen::BuildError::Lint(_))),
        "build refuses a catalog that fails lint"
    );
}

#[test]
fn mcp_tool_names_must_not_collide() {
    let mut catalog = Catalog::new();
    catalog
        .register_operation(with_dry_run(op(
            "util.a_b.c",
            1,
            json!({ "type": "object" }),
            json!({ "type": "object" }),
        )))
        .unwrap();
    catalog
        .register_operation(with_dry_run(op(
            "util.a.b_c",
            1,
            json!({ "type": "object" }),
            json!({ "type": "object" }),
        )))
        .unwrap();
    let msgs = lint_messages(&catalog);
    assert!(
        msgs.iter().any(|m| m.contains("MCP tool name collides")),
        "{msgs:?}"
    );
    assert!(codegen::is_valid_mcp_tool_name("obs_set_scene"));
    assert!(!codegen::is_valid_mcp_tool_name("Obs.SetScene"));
}

#[test]
fn core_catalog_passes_lint() {
    assert!(codegen::lint(&Catalog::core()).is_empty());
}

/// Round 6 (§3): every core example is built from the operation's typed values, so a secret-bearing
/// position is published withheld by construction (`Secret<T>`) and never by the schema walk alone.
#[test]
fn core_examples_are_typed() {
    let catalog = Catalog::core();
    for (name, def) in catalog.operations() {
        for example in &def.examples {
            assert!(
                example.authored.is_some(),
                "operation {name}, example {:?}: build it with OperationExample::typed",
                example.title
            );
        }
    }
    for (r#type, def) in catalog.events() {
        assert_eq!(
            def.authored_examples.len(),
            def.examples.len(),
            "event {type}: push examples with EventDef::push_typed_example"
        );
    }
}

/// Codex review round 1, P1: a `$ref` property's siblings (security annotations and constraints) must
/// survive inlining in both generated schemas. Round 5, P1: they must also keep their scope. Keywords
/// interact only within their own schema object, so a validation sibling is never laid beside the
/// target's keywords: the target is kept whole under `allOf` and the siblings stay outside, the shape
/// `{ "$ref", … }` had. Annotations interact with nothing and are lifted, the property's winning.
#[test]
fn reference_expansion_keeps_siblings() {
    let mut types = std::collections::BTreeMap::new();
    types.insert(
        "Settings".to_owned(),
        json!({ "type": "object", "properties": { "n": { "type": "integer" }, "m": { "type": "integer" } }, "required": ["n"], "maxProperties": 5, "description": "Shared" }),
    );
    let params = json!({
        "type": "object",
        "properties": {
            "secret": { "$ref": "#/types/Settings", "x-secret": true, "maxProperties": 1, "description": "Local", "x-ui": { "widget": "json" } }
        }
    });
    let (invocation, authoring) =
        codegen::derive_param_schemas(&Schema::try_from(params).unwrap(), &types);
    for (label, schema) in [("params", invocation), ("paramsAuthoring", authoring)] {
        let v = schema.to_value();
        let prop = &v["properties"]["secret"];
        assert_eq!(prop["x-secret"], json!(true), "{label}: x-secret kept");
        assert_eq!(prop["x-ui"]["widget"], json!("json"), "{label}: x-ui kept");
        assert_eq!(
            prop["description"],
            json!("Local"),
            "{label}: the property's annotation wins"
        );
        assert_eq!(
            prop["maxProperties"],
            json!(1),
            "{label}: the property's own constraint stays in the property's schema object"
        );
        let branches = prop["allOf"]
            .as_array()
            .expect("the target is kept whole in allOf");
        assert_eq!(branches.len(), 1, "{label}: {branches:?}");
        let target = &branches[0];
        assert_eq!(
            target["maxProperties"],
            json!(5),
            "{label}: the target's constraint stays in the target's schema object"
        );
        assert_eq!(target["required"], json!(["n"]), "{label}");
        assert_eq!(
            target["description"],
            json!("Shared"),
            "{label}: the branch keeps the target's annotation; the lifted copy is the property's"
        );
        assert!(
            prop.get("properties").is_none() && prop.get("required").is_none(),
            "{label}: nothing of the target is laid beside the property's keywords: {prop}"
        );
        assert_eq!(
            target["properties"]["n"]["x-den-template"],
            json!("value"),
            "{label}: leaves inside the branch are still derived"
        );
        let validator = jsonschema::validator_for(&v).unwrap();
        assert!(
            validator.is_valid(&json!({ "secret": { "n": 1 } })),
            "{label}"
        );
        assert!(
            !validator.is_valid(&json!({ "secret": { "n": 1, "m": 2 } })),
            "{label}: maxProperties 1 from the reference still applies"
        );
    }
}

/// Codex review round 1, P1: marked secrets never reach `protocol.json` examples (§3).
#[test]
fn published_examples_redact_marked_secrets() {
    let mut catalog = Catalog::new();
    #[derive(schemars::JsonSchema)]
    #[allow(dead_code)]
    struct Creds {
        /// The user.
        user: String,
        /// The password.
        #[schemars(extend("x-secret" = true))]
        password: String,
    }
    #[derive(schemars::JsonSchema)]
    #[serde(tag = "kind", rename_all = "snake_case")]
    #[allow(dead_code)]
    enum Auth {
        /// A token.
        Token {
            /// The token.
            #[schemars(extend("x-secret" = true))]
            token: String,
        },
        /// Nothing.
        None,
    }
    #[derive(schemars::JsonSchema)]
    #[allow(dead_code)]
    struct Params {
        /// Plain.
        password: String,
        /// Nested.
        creds: Creds,
        /// Via a reference marked on the field.
        #[schemars(extend("x-secret" = true))]
        whole: Creds,
        /// A list of secrets.
        keys: Vec<Creds>,
        /// In a tagged union.
        auth: Auth,
        /// Marked on a map value.
        extra: std::collections::BTreeMap<String, Creds>,
    }
    let mut params = catalog.schema_for::<Params>().to_value();
    params["properties"]["password"]["x-secret"] = json!(true);
    // A pattern property (review round 2); its regex is evaluated against every member name, named in
    // `properties` or not (round 3; `tests/redaction.rs` covers the overlap).
    params["patternProperties"] = json!({ "^k_": { "type": "string", "x-secret": true } });
    let mut def = op(
        "util.read",
        0,
        params,
        json!({ "type": "object", "properties": { "token": { "type": "string", "x-secret": true }, "ok": { "type": "boolean" } } }),
    );
    def.examples = vec![OperationExample {
        title: "normal".into(),
        params: json!({
            "password": "example-secret-1",
            "creds": { "user": "u", "password": "example-secret-2" },
            "whole": { "user": "example-secret-3", "password": "example-secret-4" },
            "keys": [{ "user": "u", "password": "example-secret-5" }],
            "auth": { "kind": "token", "token": "example-secret-6" },
            "extra": { "a": { "user": "u", "password": "example-secret-7" } },
            "k_api": "example-secret-13",
            "plain": "kept"
        }),
        shape: ResultShape::Normal,
        result: json!({ "token": "example-secret-8", "ok": true }),
        authored: None,
    }];
    catalog.register_operation(def).unwrap();
    let mut write = with_dry_run(op(
        "util.write",
        1,
        json!({ "type": "object", "properties": { "key": { "type": "string", "x-secret": true } } }),
        json!({ "type": "object" }),
    ));
    write.examples[1].params = json!({ "dryRun": true, "key": "example-secret-9" });
    write.examples[1].result["plan"] = json!([
        { "operation": "util.write", "params": { "key": "example-secret-10" }, "tier": 1, "description": "d" },
        { "operation": "util.read", "params": { "password": "example-secret-11" }, "tier": 0, "description": "d" }
    ]);
    catalog.register_operation(write).unwrap();
    catalog
        .register_event(EventDef {
            r#type: EventType::new("util.happened").unwrap(),
            title: "T".into(),
            description: "d".into(),
            category: vec![],
            payload_version: 1,
            payload: Schema::try_from(json!({ "type": "object", "properties": { "secret": { "type": "string", "x-secret": true }, "n": { "type": "integer" } } })).unwrap(),
            filters: vec![],
            high_volume: false,
            critical: false,
            examples: vec![json!({ "secret": "example-secret-12", "n": 1 })],
            authored_examples: vec![],
        })
        .unwrap();

    // The authored examples are what xtask validates: a redacted object cannot match an object schema.
    let authored = codegen::build_unredacted(catalog).unwrap();
    let params =
        jsonschema::validator_for(&authored.schema_at("/operations/util.read/params")).unwrap();
    let result =
        jsonschema::validator_for(&authored.schema_at("/operations/util.read/result")).unwrap();
    let example = &authored.operations["util.read"].def.examples[0];
    assert!(params.is_valid(&example.params), "authored params validate");
    assert!(
        result.is_valid(&example.result),
        "authored result validates"
    );
    assert!(
        authored.to_json_string().contains("example-secret-1"),
        "the unredacted form keeps the authored values"
    );
    let mut doc = authored.clone();
    doc.redact_examples();
    let published = doc.to_json_string();
    assert!(
        !published.contains("example-secret"),
        "a marked secret leaked into protocol.json:\n{published}"
    );
    doc.redact_examples();
    assert_eq!(doc.to_json_string(), published, "redaction is idempotent");
    let example = &doc.operations["util.read"].def.examples[0];
    assert_eq!(example.params["password"], json!("***"));
    assert_eq!(example.params["creds"]["password"], json!("***"));
    assert_eq!(
        example.params["creds"]["user"],
        json!("u"),
        "non-secrets stay"
    );
    assert_eq!(
        example.params["whole"],
        json!("***"),
        "a reference marked secret redacts the whole value"
    );
    assert_eq!(example.params["keys"][0]["password"], json!("***"));
    assert_eq!(example.params["auth"]["token"], json!("***"));
    assert_eq!(example.params["auth"]["kind"], json!("token"));
    assert_eq!(example.params["extra"]["a"]["password"], json!("***"));
    assert_eq!(
        example.params["k_api"],
        json!("***"),
        "pattern properties are followed"
    );
    assert_eq!(example.params["plain"], json!("kept"));
    assert_eq!(example.result["token"], json!("***"));
    assert_eq!(example.result["ok"], json!(true));
    let dry_run = &doc.operations["util.write"].def.examples[1];
    assert_eq!(dry_run.params["key"], json!("***"));
    assert_eq!(dry_run.result["plan"][0]["params"]["key"], json!("***"));
    assert_eq!(
        dry_run.result["plan"][1]["params"]["password"],
        json!("***")
    );
    assert_eq!(
        doc.events["util.happened"].def.examples[0]["secret"],
        json!("***")
    );
    assert_eq!(doc.events["util.happened"].def.examples[0]["n"], json!(1));
    assert!(
        !params.is_valid(&example.params),
        "an object-valued secret is published as \"***\" and no longer matches its schema, which is why xtask validates the authored form"
    );
}

/// Codex review round 1, P2: leaves inside tagged unions (schemars `oneOf` of object variants) get
/// their modes and authoring widening; discriminators stay literal.
#[test]
fn template_derivation_descends_into_tagged_unions() {
    #[derive(schemars::JsonSchema)]
    #[serde(tag = "kind", rename_all = "snake_case")]
    #[allow(dead_code)]
    enum Choice {
        /// Wait.
        Delay {
            /// Milliseconds.
            ms: u64,
        },
        /// Stop.
        Stop,
    }
    #[derive(schemars::JsonSchema)]
    #[allow(dead_code)]
    struct Params {
        /// Shared type reference to a tagged union.
        choice: Choice,
        /// A real domain union, by reference.
        target: den_protocol::domain::ActionRef,
        /// A list of unions.
        list: Vec<Choice>,
        /// An inline union.
        #[schemars(schema_with = "inline_union")]
        inline: (),
        /// A value-mode domain field, marked value again by the author.
        #[schemars(extend("x-den-template" = "value"))]
        delay: den_protocol::template::TemplateOr<u64>,
    }
    fn inline_union(_: &mut schemars::SchemaGenerator) -> Schema {
        schemars::json_schema!({ "anyOf": [{ "type": "object", "properties": { "n": { "type": "number" } } }, { "type": "boolean" }] })
    }
    let mut catalog = Catalog::new();
    let params = catalog.schema_for::<Params>();
    catalog
        .register_operation(op(
            "core.union",
            0,
            params.to_value(),
            json!({ "type": "object" }),
        ))
        .unwrap();
    let doc = codegen::build(catalog).unwrap();
    let authoring =
        jsonschema::validator_for(&doc.schema_at("/operations/core.union/paramsAuthoring"))
            .unwrap();
    let invocation =
        jsonschema::validator_for(&doc.schema_at("/operations/core.union/params")).unwrap();
    let base = json!({ "target": { "kind": "name", "name": "x" }, "list": [], "inline": true, "delay": "{wait}" });
    let delay = &doc.operations["core.union"].params_authoring.as_value()["properties"]["delay"];
    assert_eq!(delay["x-den-template"], json!("value"));
    assert_eq!(
        delay["anyOf"].as_array().map(Vec::len),
        Some(2),
        "a TemplateOr field marked value explicitly is not widened twice"
    );
    assert!(delay["anyOf"][0].get("anyOf").is_none());
    let with = |choice: Value| {
        let mut v = base.clone();
        v["choice"] = choice;
        v
    };
    assert!(
        authoring.is_valid(&with(json!({ "kind": "delay", "ms": "{delay}" }))),
        "a value leaf inside a variant takes a template"
    );
    assert!(authoring.is_valid(&with(json!({ "kind": "delay", "ms": 5 }))));
    assert!(authoring.is_valid(&with(json!({ "kind": "stop" }))));
    assert!(
        !authoring.is_valid(&with(json!({ "kind": "{k}", "ms": 5 }))),
        "the discriminator stays literal"
    );
    assert!(!authoring.is_valid(&with(json!({ "kind": "delay", "ms": "5" }))));
    assert!(
        !invocation.is_valid(&with(json!({ "kind": "delay", "ms": "{delay}" }))),
        "invocation stays concrete"
    );
    assert!(invocation.is_valid(&with(json!({ "kind": "delay", "ms": 5 }))));
    let mut v = with(json!({ "kind": "stop" }));
    v["list"] = json!([{ "kind": "delay", "ms": "{x}" }]);
    v["inline"] = json!({ "n": "{= 1 + 1}" });
    assert!(
        authoring.is_valid(&v),
        "arrays of unions and inline compositions are derived too"
    );
    let entry = &doc.operations["core.union"];
    let aut = entry.params_authoring.as_value();
    let target = &aut["properties"]["target"]["oneOf"];
    assert_eq!(
        target[0]["properties"]["id"]["x-den-template"],
        json!(false),
        "an ID reference stays a literal leaf"
    );
    assert_eq!(
        target[0]["properties"]["id"]["$ref"],
        json!("#/types/ActionId")
    );
    assert_eq!(
        target[2]["properties"]["template"]["x-den-template"],
        json!("text"),
        "the Template newtype's own mode is kept through the reference"
    );
    let delay = &aut["properties"]["choice"]["oneOf"][0];
    assert_eq!(delay["properties"]["kind"]["x-den-template"], json!(false));
    assert_eq!(delay["properties"]["ms"]["x-den-template"], json!("value"));
    assert!(delay["properties"]["ms"]["anyOf"].is_array());
    let inv = entry.def.params.as_value();
    assert_eq!(
        inv["properties"]["choice"]["oneOf"][0]["properties"]["ms"]["x-den-template"],
        json!("value")
    );
    assert!(
        inv["properties"]["choice"]["oneOf"][0]["properties"]["ms"]
            .get("anyOf")
            .is_none()
    );
}

/// Codex review round 1, P2: a `$ref` to a string newtype keeps the field's own `x-ui.variables` hint.
#[test]
fn referenced_strings_honour_local_variable_hints() {
    let mut types = std::collections::BTreeMap::new();
    types.insert("Text".to_owned(), json!({ "type": "string" }));
    types.insert("Count".to_owned(), json!({ "type": "integer" }));
    let leaf = json!({ "$ref": "#/types/Text", "x-ui": { "variables": true } });
    assert_eq!(resolve_template_mode(&leaf, &types), TemplateMode::Text);
    assert_eq!(
        resolve_template_mode(&json!({ "$ref": "#/types/Text" }), &types),
        TemplateMode::Literal
    );
    assert_eq!(
        resolve_template_mode(
            &json!({ "$ref": "#/types/Count", "x-den-template": false }),
            &types
        ),
        TemplateMode::Literal,
        "an explicit local override still wins"
    );
    let params = json!({ "type": "object", "properties": { "message": leaf, "count": { "$ref": "#/types/Count" } } });
    let (invocation, authoring) =
        codegen::derive_param_schemas(&Schema::try_from(params).unwrap(), &types);
    for schema in [invocation, authoring] {
        let v = schema.to_value();
        assert_eq!(v["properties"]["message"]["x-den-template"], json!("text"));
        assert_eq!(
            v["properties"]["message"]["$ref"],
            json!("#/types/Text"),
            "a leaf reference is kept"
        );
        assert_eq!(v["properties"]["message"]["x-ui"]["variables"], json!(true));
        assert_eq!(v["properties"]["count"]["x-den-template"], json!("value"));
    }
}

/// Review round 2: a `$ref` whose target is itself a `$ref` is followed to the end, and the walk fails
/// closed past its depth bound.
#[test]
fn redaction_follows_reference_chains() {
    let mut types = std::collections::BTreeMap::new();
    types.insert("A".to_owned(), json!({ "$ref": "#/types/B" }));
    types.insert(
        "B".to_owned(),
        json!({ "type": "string", "x-secret": true }),
    );
    types.insert("Loop".to_owned(), json!({ "$ref": "#/types/Loop" }));
    types.insert(
        "Deep".to_owned(),
        json!({ "type": "object", "properties": { "d": { "$ref": "#/types/Deep" } } }),
    );
    let schema = json!({
        "type": "object",
        "properties": {
            "chain": { "$ref": "#/types/A" },
            "looped": { "$ref": "#/types/Loop" },
            "deep": { "$ref": "#/types/Deep" }
        }
    });
    let mut deep = json!("leaf");
    for _ in 0..80 {
        deep = json!({ "d": deep });
    }
    let mut value = json!({ "chain": "LEAK1", "looped": "fine", "deep": deep, "other": "kept" });
    codegen::redact_secrets(&mut value, &schema, &types);
    assert_eq!(value["chain"], json!("***"), "the chain A -> B is followed");
    assert_eq!(
        value["looped"],
        json!("fine"),
        "a self-referential alias terminates"
    );
    assert_eq!(value["other"], json!("kept"));
    assert!(
        serde_json::to_string(&value["deep"])
            .unwrap()
            .contains("***")
            && !serde_json::to_string(&value["deep"])
                .unwrap()
                .contains("leaf"),
        "past the depth bound the remainder is redacted whole: {}",
        value["deep"]
    );
}

/// Spec 02 r6 §4: every namespace a core `register(&mut Catalog)` uses must be reserved, and the
/// generated `ContributionRef` clause follows the list.
#[test]
fn core_registrations_need_a_reserved_prefix() {
    let mut catalog = Catalog::new();
    catalog
        .register_operation(op("acme.read", 0, json!({ "type": "object" }), json!({})))
        .unwrap();
    catalog
        .register_event(EventDef {
            r#type: EventType::new("acme.happened").unwrap(),
            title: "T".into(),
            description: "d".into(),
            category: vec![],
            payload_version: 1,
            payload: Schema::try_from(
                json!({ "type": "object", "properties": { "n": { "type": "integer" } } }),
            )
            .unwrap(),
            filters: vec![],
            high_volume: false,
            critical: false,
            examples: vec![json!({ "n": 1 })],
            authored_examples: vec![],
        })
        .unwrap();
    let msgs = lint_messages(&catalog);
    assert!(
        msgs.iter().any(|m| m
            .contains("operation:acme.read: core operation under the unreserved prefix \"acme\"")),
        "{msgs:?}"
    );
    assert!(
        msgs.iter()
            .any(|m| m
                .contains("event:acme.happened: core event under the unreserved prefix \"acme\"")),
        "{msgs:?}"
    );
    for slug in [
        "runs", "signals", "commands", "timers", "schedule", "settings", "catalog", "http",
    ] {
        assert!(den_protocol::version::is_reserved_slug(slug), "{slug}");
    }
    let pattern = den_protocol::schema::ContributionRefRule::core_type_pattern();
    assert!(
        pattern.contains("|runs|") && pattern.contains("|http)"),
        "{pattern}"
    );
}

/// §6.2, A12: `platform` is a reserved filter path that resolves against the envelope, so a payload
/// never declares it at the top level.
#[test]
fn event_payloads_must_not_declare_platform() {
    let mut catalog = Catalog::new();
    catalog
        .register_event(EventDef {
            r#type: EventType::new("core.happened").unwrap(),
            title: "T".into(),
            description: "d".into(),
            category: vec![],
            payload_version: 1,
            payload: Schema::try_from(
                json!({ "type": "object", "properties": { "platform": { "type": "string" } } }),
            )
            .unwrap(),
            filters: vec![],
            high_volume: false,
            critical: false,
            examples: vec![json!({ "platform": "twitch" })],
            authored_examples: vec![],
        })
        .unwrap();
    let msgs = lint_messages(&catalog);
    assert!(
        msgs.iter()
            .any(|m| m.contains("payload declares a top-level \"platform\" field")),
        "{msgs:?}"
    );
}

/// §3, §9.7 (A10): `x-den-untrusted: "conditional"` names a sibling boolean (or required-nullable
/// boolean) that gates it; anything else is a lint failure.
#[test]
fn conditional_untrusted_marks_name_a_boolean_sibling() {
    fn lint_result(result: Value) -> Vec<String> {
        let mut catalog = Catalog::new();
        catalog
            .register_operation(op("core.read", 0, json!({ "type": "object" }), result))
            .unwrap();
        lint_messages(&catalog)
            .into_iter()
            .filter(|m| m.to_lowercase().contains("untrusted"))
            .collect()
    }
    let good = json!({
        "type": "object",
        "properties": {
            "value": { "x-den-untrusted": "conditional" },
            "untrusted": { "type": "boolean" },
            "old": { "x-den-untrusted": "conditional", "x-den-untrusted-flag": "oldUntrusted" },
            "oldUntrusted": { "anyOf": [{ "type": "boolean" }, { "type": "null" }], "x-den-nullable": true },
            "new": { "x-den-untrusted": "conditional", "x-den-untrusted-flag": "fresh" },
            "fresh": { "type": ["boolean", "null"] },
            "text": { "type": "string", "x-den-untrusted": true }
        }
    });
    assert_eq!(lint_result(good), Vec::<String>::new());

    let msgs = lint_result(json!({
        "type": "object",
        "properties": {
            "value": { "x-den-untrusted": "conditional" },
            "old": { "x-den-untrusted": "conditional", "x-den-untrusted-flag": "oldUntrusted" },
            "oldUntrusted": { "type": "string" },
            "odd": { "x-den-untrusted": "conditional", "x-den-untrusted-flag": 3 },
            "stray": { "type": "string", "x-den-untrusted-flag": "untrusted" },
            "bogus": { "type": "string", "x-den-untrusted": "yes" }
        }
    }));
    let has = |needle: &str| msgs.iter().any(|m| m.contains(needle));
    assert!(
        has(
            "result/properties/value: conditional field names the sibling flag \"untrusted\", which the object does not declare"
        ),
        "{msgs:?}"
    );
    assert!(
        has("result/properties/old: the sibling flag \"oldUntrusted\" must be a boolean"),
        "{msgs:?}"
    );
    assert!(
        has("result/properties/odd: `x-den-untrusted-flag` must be a string"),
        "{msgs:?}"
    );
    assert!(
        has("result/properties/stray: `x-den-untrusted-flag` only goes with"),
        "{msgs:?}"
    );
    assert!(
        has("result/properties/bogus: x-den-untrusted must be `true` or \"conditional\""),
        "{msgs:?}"
    );
    // A conditional mark at a root (no sibling possible) is refused too.
    let msgs = lint_result(json!({ "type": "object", "x-den-untrusted": "conditional" }));
    assert!(
        msgs.iter().any(|m| m.contains("must sit on a property")),
        "{msgs:?}"
    );
    // The core catalog carries the mark on `VariableRecord.value`, gated on `untrusted`.
    let doc = codegen::build(Catalog::core()).unwrap();
    let record = &doc.types["VariableRecord"]["properties"];
    assert_eq!(record["value"]["x-den-untrusted"], json!("conditional"));
    assert_eq!(record["untrusted"]["type"], json!("boolean"));
    assert_eq!(
        den_protocol::codegen::untrusted_flag_of(record["value"].as_object().unwrap()),
        Some("untrusted")
    );
}

/// §5.2, §9.7 (A13): a step in an event's `payloadVersions` table needs both its upgrade and its
/// downgrade function; the newest served `rpcVersion` delivers the current payload version.
#[test]
fn payload_version_steps_need_both_functions() {
    use den_protocol::catalog::{CatalogError, PayloadVersions};
    fn bump(v: Value) -> Result<Value, String> {
        Ok(v)
    }
    let ty = EventType::new("core.evolved").unwrap();
    let mut catalog = Catalog::new();
    assert!(matches!(
        catalog.register_payload_upgrade(ty.clone(), 2, 1, bump),
        Err(CatalogError::InvalidPayloadStep { from: 2, to: 1, .. })
    ));
    catalog.register_payload_version(ty.clone(), 1, 1);
    catalog.register_payload_version(ty.clone(), 2, 2);
    catalog.register_payload_version(ty.clone(), 3, 2);
    let versions: PayloadVersions = catalog.payload_versions(&ty);
    assert_eq!(
        versions.table_for(1..=3, 2),
        [(1, 1), (2, 2), (3, 2)].into_iter().collect()
    );
    assert_eq!(
        PayloadVersions::delivered(&versions.table_for(1..=3, 2)),
        vec![1, 2]
    );
    assert_eq!(
        versions.table_for(1..=4, 3),
        [(1, 1), (2, 2), (3, 2), (4, 3)].into_iter().collect(),
        "an unregistered rpcVersion delivers the current payload version"
    );

    let msgs = codegen::lint_payload_versions(&versions, 2, 1..=3);
    assert!(
        msgs.iter()
            .any(|m| m.contains("step 1 → 2 has no upgrade function")),
        "{msgs:?}"
    );
    assert!(
        msgs.iter()
            .any(|m| m.contains("step 1 → 2 has no downgrade function")),
        "{msgs:?}"
    );
    catalog
        .register_payload_upgrade(ty.clone(), 1, 2, bump)
        .unwrap();
    let msgs = codegen::lint_payload_versions(&catalog.payload_versions(&ty), 2, 1..=3);
    assert!(!msgs.iter().any(|m| m.contains("no upgrade")), "{msgs:?}");
    assert!(msgs.iter().any(|m| m.contains("no downgrade")), "{msgs:?}");
    catalog
        .register_payload_downgrade(ty.clone(), 1, 2, bump)
        .unwrap();
    assert_eq!(
        codegen::lint_payload_versions(&catalog.payload_versions(&ty), 2, 1..=3),
        Vec::<String>::new()
    );
    // The newest served rpcVersion must deliver the event's current payloadVersion.
    let msgs = codegen::lint_payload_versions(&catalog.payload_versions(&ty), 3, 1..=3);
    assert!(
        msgs.iter().any(|m| m.contains("maps the current rpcVersion 3 to payload version Some(2), not the event's payloadVersion 3")),
        "{msgs:?}"
    );
    // A table entry above the current version is refused.
    let msgs = codegen::lint_payload_versions(&catalog.payload_versions(&ty), 1, 1..=3);
    assert!(
        msgs.iter()
            .any(|m| m.contains("above the event's payloadVersion 1")),
        "{msgs:?}"
    );
    // A registration for an rpcVersion the build does not serve, or a payload version 0, never vanishes
    // silently.
    let msgs = codegen::lint_payload_versions(&catalog.payload_versions(&ty), 2, 1..=1);
    assert!(
        msgs.iter()
            .any(|m| m.contains("registers rpcVersion 2, which this build does not serve (1..=1)"))
            && msgs
                .iter()
                .any(|m| m.contains("registers rpcVersion 3, which this build does not serve")),
        "{msgs:?}"
    );
    let mut zero = PayloadVersions::default();
    zero.by_rpc_version.insert(1, 0);
    let msgs = codegen::lint_payload_versions(&zero, 1, 1..=1);
    assert!(
        msgs.iter()
            .any(|m| m.contains("payload version 0; payload versions start at 1")),
        "{msgs:?}"
    );

    // Through the catalog lint, with the build's served range (one rpcVersion today): functions for an
    // unregistered event are flagged, and the registered table is published.
    let mut catalog = Catalog::new();
    catalog.register_payload_version(ty.clone(), 1, 1);
    let msgs = lint_messages(&catalog);
    assert!(
        msgs.iter().any(|m| m.contains("event:core.evolved: payload version functions registered for an event type that is not registered")),
        "{msgs:?}"
    );
    catalog
        .register_event(EventDef {
            r#type: ty.clone(),
            title: "T".into(),
            description: "d".into(),
            category: vec![],
            payload_version: 2,
            payload: Schema::try_from(
                json!({ "type": "object", "properties": { "n": { "type": "integer" } } }),
            )
            .unwrap(),
            filters: vec![],
            high_volume: false,
            critical: false,
            examples: vec![json!({ "n": 1 })],
            authored_examples: vec![],
        })
        .unwrap();
    let msgs = lint_messages(&catalog);
    assert!(
        msgs.iter().any(|m| m.contains("event:core.evolved: payloadVersions maps the current rpcVersion 1 to payload version Some(1), not the event's payloadVersion 2")),
        "{msgs:?}"
    );
    catalog.register_payload_version(ty.clone(), 1, 2);
    assert_eq!(lint_messages(&catalog), Vec::<String>::new());
    let doc = codegen::build(catalog).unwrap();
    assert_eq!(
        doc.events["core.evolved"].payload_versions,
        [("1".to_owned(), 2)].into_iter().collect()
    );
}

/// §7.2, §9.7: the receiving-only fallback is published for receivers, but an example (a producer) whose
/// `kind` is not a known kind fails validation.
#[test]
fn examples_with_unknown_kinds_are_rejected() {
    use den_protocol::domain::SubAction;
    #[derive(schemars::JsonSchema)]
    #[allow(dead_code)]
    struct Params {
        /// A node.
        node: SubAction,
    }
    let known = json!({ "node": { "id": "018f7a8c-0000-7000-8000-000000000001", "enabled": true, "kind": "stop" } });
    let unknown = json!({ "node": { "id": "018f7a8c-0000-7000-8000-000000000001", "enabled": true, "kind": "future_node", "future": 1 } });
    for (example, ok) in [(known, true), (unknown, false)] {
        let mut catalog = Catalog::new();
        let params = catalog.schema_for::<Params>();
        let mut def = op("core.nodes", 0, params.to_value(), json!({}));
        def.examples = vec![OperationExample::json(
            "n",
            example.clone(),
            ResultShape::Normal,
            json!({}),
        )];
        catalog.register_operation(def).unwrap();
        let doc = codegen::build_unredacted(catalog).unwrap();
        let receiver =
            jsonschema::validator_for(&doc.schema_at("/operations/core.nodes/params")).unwrap();
        let producer =
            jsonschema::validator_for(&doc.producer_schema_at("/operations/core.nodes/params"))
                .unwrap();
        assert!(
            receiver.is_valid(&example),
            "receivers accept the fallback (§7.2)"
        );
        assert_eq!(producer.is_valid(&example), ok);
        let result = codegen::validate_examples(&doc);
        if ok {
            assert_eq!(result, Ok(()));
        } else {
            let failures = result.expect_err("an unknown kind is not a valid example");
            assert!(
                failures
                    .iter()
                    .any(|f| f.contains("operation core.nodes, example \"n\": params invalid")),
                "{failures:?}"
            );
        }
    }
    let doc = codegen::build(Catalog::core()).unwrap();
    let producer = doc.producer_schema_at("/types/Fragment");
    assert!(
        !serde_json::to_string(&producer["types"]["Fragment"])
            .unwrap()
            .contains("UnknownFragment"),
        "the fallback branch is stripped from the producer view"
    );
    assert!(
        serde_json::to_string(&doc.schema_at("/types/Fragment")["types"]["Fragment"])
            .unwrap()
            .contains("UnknownFragment"),
        "and kept in the receiver view"
    );
    let v = jsonschema::validator_for(&producer).unwrap();
    assert!(v.is_valid(&json!({ "kind": "text", "text": "hi" })));
    assert!(!v.is_valid(&json!({ "kind": "sticker", "text": "hi" })));
}

/// DEN-122: map entries are schemas even when their names are annotation keywords.
#[test]
fn producer_strips_inline_fallbacks_under_annotation_named_properties() {
    let core = codegen::build(Catalog::core()).unwrap();
    for (ty, known, unknown) in [
        (
            "Node",
            json!({ "kind": "stop" }),
            json!({ "kind": "future_node" }),
        ),
        (
            "Fragment",
            json!({ "kind": "text", "text": "hi" }),
            json!({ "kind": "future_fragment", "text": "hi" }),
        ),
    ] {
        for name in ["ordinary", "default", "title", "x-custom"] {
            let mut catalog = Catalog::core();
            let mut def = op(
                "core.inline",
                0,
                json!({
                    "type": "object",
                    "properties": { name: core.types[ty].clone() },
                    "required": [name],
                }),
                json!({}),
            );
            let example = json!({ name: known });
            def.examples = vec![OperationExample::json(
                "inline",
                example.clone(),
                ResultShape::Normal,
                json!({}),
            )];
            catalog.register_operation(def).unwrap();
            let mut doc = codegen::build_unredacted(catalog).unwrap();
            let pointer = "/operations/core.inline/params";
            let producer = jsonschema::validator_for(&doc.producer_schema_at(pointer)).unwrap();
            assert!(producer.is_valid(&example), "{ty} under {name}: known kind");
            assert_eq!(codegen::validate_examples(&doc), Ok(()));

            let example = json!({ name: unknown });
            let receiver = jsonschema::validator_for(&doc.schema_at(pointer)).unwrap();
            assert!(receiver.is_valid(&example), "{ty} under {name}: receiver");
            assert!(!producer.is_valid(&example), "{ty} under {name}: producer");
            doc.operations.get_mut("core.inline").unwrap().def.examples =
                vec![OperationExample::json(
                    "inline",
                    example,
                    ResultShape::Normal,
                    json!({}),
                )];
            assert!(
                codegen::validate_examples(&doc).is_err(),
                "{ty} under {name}"
            );
        }
    }
}

/// Actual annotations can contain schema-looking data and must remain unchanged.
#[test]
fn producer_preserves_annotations_while_visiting_schema_maps() {
    let mut doc = codegen::build(Catalog::core()).unwrap();
    let fragment = doc.types["Fragment"].clone();
    let mut wrapper = json!({ "type": "object" });
    for key in ["default", "x-custom"] {
        wrapper[key] = fragment.clone();
    }
    wrapper["examples"] = json!([fragment.clone()]);
    wrapper["title"] = json!("Preserved title");
    for key in [
        "properties",
        "patternProperties",
        "dependentSchemas",
        "$defs",
        "definitions",
    ] {
        wrapper[key] = json!({ "default": fragment.clone(), "title": fragment.clone(), "x-custom": fragment.clone() });
    }
    doc.types.insert("Wrapper".into(), wrapper.clone());
    let producer = doc.producer_schema_at("/types/Wrapper");
    let transformed = &producer["types"]["Wrapper"];
    for key in ["default", "examples", "title", "x-custom"] {
        assert_eq!(transformed[key], wrapper[key], "annotation {key}");
    }
    let known = json!({ "kind": "text", "text": "hi" });
    let unknown = json!({ "kind": "future_fragment", "text": "hi" });
    for key in [
        "properties",
        "patternProperties",
        "dependentSchemas",
        "$defs",
        "definitions",
    ] {
        for name in ["default", "title", "x-custom"] {
            let pointer = format!("/types/Wrapper/{key}/{name}");
            let mut schema = producer.clone();
            schema["$ref"] = json!(format!("#{pointer}"));
            let validator = jsonschema::validator_for(&schema).unwrap();
            assert!(validator.is_valid(&known), "{pointer}: known kind");
            assert!(!validator.is_valid(&unknown), "{pointer}: unknown kind");
        }
    }
    assert_eq!(
        doc.types["Wrapper"], wrapper,
        "receiver schema is unchanged"
    );
}

/// DEN-53 (§3): a validation failure is described without the instance. The validator's `Display` and
/// the `Debug` of an `anyOf`/`oneOf` error kind carry the authored instance (every branch's error with
/// its instance), which for a typed example is a `Secret<T>` value exposed for validation; the shared
/// describer names the keyword and its schema location only, and masks authored map keys.
#[test]
fn validation_diagnostics_never_carry_the_authored_instance() {
    use den_protocol::secret::{Secret, expose_secrets};
    use std::collections::BTreeMap;
    #[derive(serde::Serialize)]
    struct Authored {
        token: Secret<String>,
        pick: Secret<String>,
        extra: BTreeMap<Secret<String>, u32>,
        plain: u32,
    }
    let sentinel = "sdt_den53_synthetic_sentinel_value";
    let key_sentinel = "den53_synthetic_sentinel_key";
    let authored = expose_secrets(|| {
        serde_json::to_value(Authored {
            token: Secret::new(sentinel.into()),
            pick: Secret::new(sentinel.into()),
            extra: [(Secret::new(key_sentinel.into()), 1)]
                .into_iter()
                .collect(),
            plain: 1,
        })
        .unwrap()
    });
    let schema = json!({
        "type": "object",
        "properties": {
            "token": { "anyOf": [{ "type": "integer" }, { "type": "boolean" }], "x-secret": true },
            "pick": { "oneOf": [{ "type": "integer" }, { "type": "boolean" }], "x-secret": true },
            "extra": { "type": "object", "additionalProperties": { "type": "string" } },
            "plain": { "type": "string" }
        }
    });
    let validator = jsonschema::validator_for(&schema).unwrap();
    let errors: Vec<_> = validator.iter_errors(&authored).collect();
    assert_eq!(errors.len(), 4, "{errors:?}");
    let mut saw_any_of = false;
    let mut saw_one_of = false;
    let mut saw_key = false;
    for e in &errors {
        let unsafe_form = format!("{e} {:?}", e.kind());
        let safe = codegen::describe_validation_error(e);
        assert!(
            !safe.contains(sentinel) && !safe.contains(key_sentinel),
            "the diagnostic leaks the authored value: {safe}"
        );
        match e.kind().keyword() {
            "anyOf" => {
                saw_any_of = true;
                assert!(
                    unsafe_form.contains(sentinel),
                    "the reproduction is real: {unsafe_form}"
                );
                assert!(
                    safe.contains("`anyOf` failed") && safe.contains("instance path \"/token\""),
                    "{safe}"
                );
            }
            "oneOf" => {
                saw_one_of = true;
                assert!(
                    unsafe_form.contains(sentinel),
                    "the reproduction is real: {unsafe_form}"
                );
                assert!(
                    safe.contains("`oneOf` failed") && safe.contains("instance path \"/pick\""),
                    "{safe}"
                );
            }
            "type" if e.instance_path().as_str().starts_with("/extra") => {
                saw_key = true;
                assert!(
                    e.instance_path().as_str().contains(key_sentinel),
                    "the raw path carries the authored key: {}",
                    e.instance_path().as_str()
                );
                assert!(safe.contains("instance path \"/extra/<key>\""), "{safe}");
            }
            _ => assert!(safe.contains("instance path \"/plain\""), "{safe}"),
        }
    }
    assert!(saw_any_of && saw_one_of && saw_key, "{errors:?}");

    // Codex-style follow-up on the fallback: a keyword that fails without descending (here
    // `additionalProperties: false` on the nested object) leaves the evaluation path one step longer than
    // the instance path, so the two do not line up; every segment is masked then, a digits-only authored
    // key included.
    let schema = json!({
        "type": "object",
        "properties": {
            "extra": { "type": "object", "additionalProperties": { "type": "object", "additionalProperties": false } }
        }
    });
    let digits_key = "483921";
    let instance = json!({ "extra": { digits_key: { "pin": 1 } } });
    let validator = jsonschema::validator_for(&schema).unwrap();
    let errors: Vec<_> = validator.iter_errors(&instance).collect();
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert!(errors[0].instance_path().as_str().contains(digits_key));
    let safe = codegen::describe_validation_error(&errors[0]);
    assert!(
        !safe.contains(digits_key) && safe.contains("instance path \"/<key>/<key>\""),
        "{safe}"
    );

    // End to end: a typed example whose authored secret fails a union is reported without it.
    #[derive(serde::Serialize)]
    struct Params {
        token: Secret<String>,
    }
    let mut catalog = Catalog::new();
    let mut def = op(
        "core.union_secret",
        0,
        json!({ "type": "object", "properties": { "token": { "anyOf": [{ "type": "integer" }, { "type": "boolean" }], "x-secret": true } } }),
        json!({}),
    );
    def.examples = vec![OperationExample::typed(
        "typed",
        &Params {
            token: Secret::new(sentinel.into()),
        },
        ResultShape::Normal,
        &json!({}),
    )];
    catalog.register_operation(def).unwrap();
    let doc = codegen::build_unredacted(catalog).unwrap();
    let failures =
        codegen::validate_examples(&doc).expect_err("the authored string fails the union");
    let joined = failures.join("\n");
    assert!(
        joined.contains(
            "operation core.union_secret, example \"typed\": params invalid: `anyOf` failed"
        ),
        "{joined}"
    );
    assert!(
        !joined.contains(sentinel),
        "the diagnostic leaks the authored secret: {joined}"
    );
}

/// A5: `FilterableField.specificity` defaults to 1, `caseInsensitive` to false, and `optionsFrom` is an
/// operation name.
#[test]
fn filterable_field_defaults() {
    use den_protocol::catalog::FilterableField;
    let f: FilterableField =
        serde_json::from_value(json!({ "field": "tier", "ops": ["eq"] })).unwrap();
    assert_eq!(f.specificity, 1);
    assert!(!f.case_insensitive);
    assert_eq!(
        f,
        FilterableField::new("tier", vec![den_protocol::domain::FilterOp::Eq])
    );
    assert!(
        serde_json::from_value::<FilterableField>(
            json!({ "field": "tier", "ops": [], "optionsFrom": "nodot" })
        )
        .is_err(),
        "optionsFrom is an OperationName (§6.2)"
    );
    let doc = codegen::build(Catalog::core()).unwrap();
    let props = &doc.types["FilterableField"]["properties"];
    assert_eq!(props["specificity"]["default"], json!(1));
    assert_eq!(props["caseInsensitive"]["default"], json!(false));
    assert_eq!(props["optionsFrom"]["$ref"], json!("#/types/OperationName"));
}
