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
    catalog.register_operation(with_dry_run(op("t.brace_enum", 1, json!({ "type": "object", "properties": { "scene": { "type": "string", "enum": ["{x}"] } } }), json!({ "type": "object" })))).unwrap();
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
