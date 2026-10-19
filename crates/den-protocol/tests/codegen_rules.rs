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
        }],
    }
}

fn with_dry_run(mut def: OperationDef) -> OperationDef {
    def.examples.push(OperationExample {
        title: "dry run".into(),
        params: json!({ "dryRun": true }),
        shape: ResultShape::DryRun,
        result: json!({ "dryRun": true, "plan": [], "warnings": [], "wouldRequireApproval": false, "effectiveTier": 1 }),
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
            "t.read",
            0,
            json!({ "type": "object", "properties": {} }),
            json!({ "type": "array" }),
        ))
        .unwrap();
    catalog
        .register_operation(with_dry_run(op(
            "t.write",
            1,
            json!({ "type": "object", "properties": {} }),
            json!({ "type": "object", "properties": { "ok": { "type": "boolean" } } }),
        )))
        .unwrap();
    let mut cfg = with_dry_run(op(
        "t.config",
        2,
        json!({ "type": "object", "properties": {} }),
        json!({ "type": "object" }),
    ));
    cfg.examples.push(OperationExample {
        title: "draft".into(),
        params: json!({}),
        shape: ResultShape::Draft,
        result: json!({ "draft": { "id": "018f7a8c-0000-7000-8000-000000000001", "kind": "pending_change", "operation": "t.config" }, "plan": [], "warnings": [] }),
    });
    catalog.register_operation(cfg).unwrap();
    catalog
        .register_operation(with_dry_run(op(
            "t.external",
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
        props("t.read").get("dryRun").is_none(),
        "tier 0 has no dryRun"
    );
    assert!(props("t.write").get("dryRun").is_some());
    assert!(props("t.write").get("approvalId").is_none());
    assert!(
        props("t.external").get("approvalId").is_some(),
        "tier 3 gets approvalId"
    );
    assert!(doc.operations["t.read"].dry_run_result.is_none());
    assert!(doc.operations["t.write"].dry_run_result.is_some());
    assert!(doc.operations["t.write"].draft_result.is_none());
    assert!(
        doc.operations["t.config"].draft_result.is_some(),
        "tier 2 gets draftResult"
    );
    let result = doc.operations["t.write"].def.result.as_value().clone();
    assert_eq!(
        result["properties"]["dryRun"],
        json!(false),
        "tier ≥ 1 results forbid dryRun"
    );
    assert_eq!(result["properties"]["draft"], json!(false));
    assert_eq!(
        doc.operations["t.read"].def.result.get("properties"),
        None,
        "tier-0 result left untouched"
    );
    assert_eq!(
        doc.operations["t.external"].mcp_tool_name.as_deref(),
        Some("t_external")
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
        "t.bad",
        1,
        json!({ "type": "object", "properties": { "dryRun": { "type": "boolean" }, "Weird Name!": { "type": "string" } } }),
        json!({ "type": "string" }),
    );
    bad.examples.clear();
    catalog.register_operation(bad).unwrap();
    let mut dangerous = with_dry_run(op(
        "t.code",
        4,
        json!({ "type": "object" }),
        json!({ "type": "object" }),
    ));
    dangerous.expose.mcp = true;
    catalog.register_operation(dangerous).unwrap();
    let mut union_root = with_dry_run(op(
        "t.union",
        1,
        json!({ "oneOf": [{ "type": "object" }] }),
        json!({ "type": "object", "properties": { "draft": {} } }),
    ));
    union_root.description = "".into();
    catalog.register_operation(union_root).unwrap();
    catalog.register_operation(with_dry_run(op("t.brace_enum", 1, json!({ "type": "object", "properties": { "scene": { "type": "string", "enum": ["{x}"] }, "mode": { "type": "integer", "x-den-template": "bogus" }, "n": { "type": "number", "x-den-template": "text" } } }), json!({ "type": "object" })))).unwrap();
    catalog
        .register_operation(op(
            "t.read_with_dry_run",
            0,
            json!({ "type": "object" }),
            json!({}),
        ))
        .unwrap();
    let last = catalog.operations().len();
    assert_eq!(last, 5);

    catalog
        .register_event(EventDef {
            r#type: EventType::new("t.happened").unwrap(),
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
            }],
            high_volume: false,
            critical: false,
            examples: vec![],
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
    assert!(has("event:t.happened: missing title"), "{msgs:?}");
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
            "a_b.c",
            1,
            json!({ "type": "object" }),
            json!({ "type": "object" }),
        )))
        .unwrap();
    catalog
        .register_operation(with_dry_run(op(
            "a.b_c",
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

/// Codex review round 1, P1: a `$ref` property's siblings (security annotations and constraints) must
/// survive inlining in both generated schemas.
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
            json!(5),
            "{label}: the target's constraint stays"
        );
        assert_eq!(
            prop["allOf"],
            json!([{ "maxProperties": 1 }]),
            "{label}: the colliding local constraint is applied in conjunction"
        );
        assert_eq!(prop["properties"]["n"]["x-den-template"], json!("value"));
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
        "demo.read",
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
    }];
    catalog.register_operation(def).unwrap();
    let mut write = with_dry_run(op(
        "demo.write",
        1,
        json!({ "type": "object", "properties": { "key": { "type": "string", "x-secret": true } } }),
        json!({ "type": "object" }),
    ));
    write.examples[1].params = json!({ "dryRun": true, "key": "example-secret-9" });
    write.examples[1].result["plan"] = json!([
        { "operation": "demo.write", "params": { "key": "example-secret-10" }, "tier": 1, "description": "d" },
        { "operation": "demo.read", "params": { "password": "example-secret-11" }, "tier": 0, "description": "d" }
    ]);
    catalog.register_operation(write).unwrap();
    catalog
        .register_event(EventDef {
            r#type: EventType::new("demo.happened").unwrap(),
            title: "T".into(),
            description: "d".into(),
            category: vec![],
            payload_version: 1,
            payload: Schema::try_from(json!({ "type": "object", "properties": { "secret": { "type": "string", "x-secret": true }, "n": { "type": "integer" } } })).unwrap(),
            filters: vec![],
            high_volume: false,
            critical: false,
            examples: vec![json!({ "secret": "example-secret-12", "n": 1 })],
        })
        .unwrap();

    // The authored examples are what xtask validates: a redacted object cannot match an object schema.
    let authored = codegen::build_unredacted(catalog).unwrap();
    let params =
        jsonschema::validator_for(&authored.schema_at("/operations/demo.read/params")).unwrap();
    let result =
        jsonschema::validator_for(&authored.schema_at("/operations/demo.read/result")).unwrap();
    let example = &authored.operations["demo.read"].def.examples[0];
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
    let example = &doc.operations["demo.read"].def.examples[0];
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
    let dry_run = &doc.operations["demo.write"].def.examples[1];
    assert_eq!(dry_run.params["key"], json!("***"));
    assert_eq!(dry_run.result["plan"][0]["params"]["key"], json!("***"));
    assert_eq!(
        dry_run.result["plan"][1]["params"]["password"],
        json!("***")
    );
    assert_eq!(
        doc.events["demo.happened"].def.examples[0]["secret"],
        json!("***")
    );
    assert_eq!(doc.events["demo.happened"].def.examples[0]["n"], json!(1));
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
            "t.union",
            0,
            params.to_value(),
            json!({ "type": "object" }),
        ))
        .unwrap();
    let doc = codegen::build(catalog).unwrap();
    let authoring =
        jsonschema::validator_for(&doc.schema_at("/operations/t.union/paramsAuthoring")).unwrap();
    let invocation =
        jsonschema::validator_for(&doc.schema_at("/operations/t.union/params")).unwrap();
    let base = json!({ "target": { "kind": "name", "name": "x" }, "list": [], "inline": true, "delay": "{wait}" });
    let delay = &doc.operations["t.union"].params_authoring.as_value()["properties"]["delay"];
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
    let entry = &doc.operations["t.union"];
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
