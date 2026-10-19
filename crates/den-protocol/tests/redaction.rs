//! Secret redaction at the publication boundary (Spec 02 §3): the walk behind
//! `codegen::redact_secrets` is structurally fail-closed. Regressions for the Codex round-2 review of
//! DEN-34 (overlapping named and pattern properties, conditional schemas) and DEN-47 (long reference
//! chains), plus an exhaustive keyword-combination generator. Every value here is synthetic.

use std::collections::BTreeMap;

use den_protocol::catalog::{
    ArgWrites, Catalog, Exposure, OperationDef, OperationExample, ResultShape,
};
use den_protocol::codegen::{self, REDACTED, redact_secrets};
use den_protocol::ids::OperationName;
use den_protocol::shared::RiskTier;
use schemars::Schema;
use serde_json::{Value, json};

const SECRET: &str = "SYNTHETIC_SECRET";

fn op(name: &str, params: Value, example: Value) -> OperationDef {
    op_at_tier(name, 0, params, example)
}

fn op_at_tier(name: &str, tier: u8, params: Value, example: Value) -> OperationDef {
    OperationDef {
        name: OperationName::new(name).unwrap(),
        title: "T".into(),
        description: "Test. An operation.".into(),
        category: vec!["Test".into()],
        params: Schema::try_from(params).unwrap(),
        result: Schema::try_from(json!({ "type": "object" })).unwrap(),
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
            params: example,
            shape: ResultShape::Normal,
            result: json!({}),
        }],
    }
}

/// Builds a one-operation catalog the way `xtask` does: the authored example must validate against
/// the published invocation schema first, then the document is redacted. Returns the published
/// params of the example and the serialized document.
fn publish(params: Value, example: Value) -> (Value, String) {
    let mut catalog = Catalog::new();
    catalog
        .register_operation(op("demo.op", params, example.clone()))
        .unwrap();
    let authored = codegen::build_unredacted(catalog).unwrap();
    let validator =
        jsonschema::validator_for(&authored.schema_at("/operations/demo.op/params")).unwrap();
    assert!(
        validator.is_valid(&example),
        "the authored example must be valid, or the case proves nothing"
    );
    let mut doc = authored;
    doc.redact_examples();
    let published = doc.to_json_string();
    (
        doc.operations["demo.op"].def.examples[0].params.clone(),
        published,
    )
}

/// Codex round 2, P1 #1: `patternProperties` apply to a member even when `properties` names it.
#[test]
fn named_and_pattern_properties_both_apply() {
    let params = json!({
        "type": "object",
        "properties": { "api_key": { "type": "string" }, "api_user": { "type": "string" } },
        "patternProperties": { "^api_k": { "x-secret": true } }
    });
    let (published, doc) = publish(
        params,
        json!({ "api_key": SECRET, "api_user": "kept-user", "other": "kept-other" }),
    );
    assert_eq!(published["api_key"], json!(REDACTED));
    assert_eq!(
        published["api_user"],
        json!("kept-user"),
        "a non-matching pattern changes nothing"
    );
    assert_eq!(
        published["other"],
        json!("kept-other"),
        "an undescribed member is left alone"
    );
    assert!(!doc.contains(SECRET), "secret published:\n{doc}");

    // The mark on the named property and the pattern on an undeclared member both still work.
    let params = json!({
        "type": "object",
        "properties": { "token": { "type": "string", "x-secret": true } },
        "patternProperties": { "^k_": { "type": "string", "x-secret": true } },
        "additionalProperties": { "type": "string" }
    });
    let (published, doc) = publish(
        params,
        json!({ "token": SECRET, "k_1": SECRET, "plain": "kept" }),
    );
    assert_eq!(published["token"], json!(REDACTED));
    assert_eq!(published["k_1"], json!(REDACTED));
    assert_eq!(
        published["plain"],
        json!("kept"),
        "additionalProperties without a mark keeps the value"
    );
    assert!(!doc.contains(SECRET), "secret published:\n{doc}");

    // A pattern that does not compile matches every member (conservative), named or not.
    let broken = json!({
        "type": "object",
        "properties": { "named": { "type": "string" } },
        "patternProperties": { "(": { "x-secret": true } }
    });
    let mut v = json!({ "named": SECRET, "other": SECRET });
    redact_secrets(&mut v, &broken, &BTreeMap::new());
    assert_eq!(v, json!({ "named": REDACTED, "other": REDACTED }));
}

/// A dry-run or draft plan step is redacted against the operation it names. A step naming a target
/// outside the catalog (a plugin operation, §8.1) cannot be checked: the lint accepts it only with
/// value-free params, and publication redacts anything else whole.
#[test]
fn plan_steps_naming_outside_targets_fail_closed() {
    let params = json!({ "type": "object", "properties": { "key": { "type": "string", "x-secret": true } } });
    let plan_with = |operation: &str, step_params: Value| {
        json!({ "dryRun": true, "plan": [
            { "operation": operation, "params": step_params, "tier": 1, "description": "d" }
        ], "warnings": [], "wouldRequireApproval": false, "effectiveTier": 1 })
    };
    let dry_run = |operation: &str, step_params: Value| OperationExample {
        title: "dry run".into(),
        params: json!({ "dryRun": true, "key": SECRET }),
        shape: ResultShape::DryRun,
        result: plan_with(operation, step_params),
    };
    let catalog_with = |step: OperationExample| {
        let mut catalog = Catalog::new();
        let mut def = op_at_tier("demo.write", 1, params.clone(), json!({ "key": SECRET }));
        def.examples.push(step);
        catalog.register_operation(def).unwrap();
        catalog
    };

    // Lint: a typo'd or external target with values in its params is rejected.
    let err = codegen::build(catalog_with(dry_run(
        "demo.wrte",
        json!({ "key": SECRET, "n": 1 }),
    )))
    .unwrap_err()
    .to_string();
    assert!(
        err.contains("plan step 0 names `demo.wrte` outside this catalog with non-empty params"),
        "the lint rejects it: {err}"
    );
    let err = codegen::build(catalog_with(dry_run(
        "ext.target",
        json!({ "nested": { "n": 1 } }),
    )))
    .unwrap_err()
    .to_string();
    assert!(
        err.contains("outside this catalog"),
        "nested values count: {err}"
    );

    // Lint: value-free params on an outside target pass, and are published unchanged.
    let doc = codegen::build(catalog_with(dry_run(
        "ext.target",
        json!({ "empty": [], "o": {} }),
    )))
    .unwrap();
    let step = &doc.operations["demo.write"].def.examples[1].result["plan"][0];
    assert_eq!(step["params"], json!({ "empty": [], "o": {} }));
    assert!(!doc.to_json_string().contains(SECRET));

    // A known target is redacted against its schema.
    let doc = codegen::build(catalog_with(dry_run(
        "demo.write",
        json!({ "key": SECRET, "n": 1 }),
    )))
    .unwrap();
    let step = &doc.operations["demo.write"].def.examples[1].result["plan"][0];
    assert_eq!(step["params"], json!({ "key": REDACTED, "n": 1 }));

    // Past the lint (a document edited after the build): an outside target with values leaks nothing.
    let mut doc = codegen::build_unredacted(catalog_with(dry_run(
        "demo.write",
        json!({ "key": SECRET, "n": 1 }),
    )))
    .unwrap();
    doc.operations.get_mut("demo.write").unwrap().def.examples[1].result["plan"][0]["operation"] =
        json!("demo.wrte");
    doc.redact_examples();
    let published = doc.to_json_string();
    assert!(
        !published.contains(SECRET),
        "secret published:\n{published}"
    );
    let step = &doc.operations["demo.write"].def.examples[1].result["plan"][0];
    assert_eq!(step["params"], json!(REDACTED), "params redacted whole");
    assert_eq!(step["operation"], json!("demo.wrte"));
}

/// Codex round 2, P1 #2: `if`/`then`/`else` are applicable subschemas; both branches (and the
/// condition) are consulted whatever the condition would decide.
#[test]
fn conditional_branches_redact() {
    let then_marks = json!({
        "type": "object",
        "properties": { "api_key": { "type": "string" }, "mode": { "type": "string" } },
        "if": { "required": ["api_key"] },
        "then": { "properties": { "api_key": { "x-secret": true } } },
        "else": { "properties": { "mode": { "const": "anonymous" } } }
    });
    let (published, doc) = publish(then_marks, json!({ "api_key": SECRET, "mode": "m" }));
    assert_eq!(
        published["api_key"],
        json!(REDACTED),
        "the then branch applies"
    );
    assert_eq!(published["mode"], json!("m"));
    assert!(!doc.contains(SECRET), "secret published:\n{doc}");

    let else_marks = json!({
        "type": "object",
        "properties": { "api_key": { "type": "string" }, "mode": { "type": "string" } },
        "if": { "required": ["mode"] },
        "then": { "properties": { "mode": { "minLength": 1 } } },
        "else": { "properties": { "api_key": { "x-secret": true } } }
    });
    // The example takes the `then` path (it has `mode`), and the secret is still redacted: a branch
    // that does not validate can only redact more.
    let (published, doc) = publish(else_marks, json!({ "api_key": SECRET, "mode": "m" }));
    assert_eq!(
        published["api_key"],
        json!(REDACTED),
        "the else branch applies"
    );
    assert!(!doc.contains(SECRET), "secret published:\n{doc}");

    // Nested conditionals and a mark inside the condition itself.
    let nested = json!({
        "type": "object",
        "properties": { "a": { "type": "object", "properties": { "k": { "type": "string" } } } },
        "if": { "properties": { "a": { "if": { "properties": { "k": { "x-secret": true } } } } } },
        "then": true
    });
    let (published, doc) = publish(nested, json!({ "a": { "k": SECRET } }));
    assert_eq!(published["a"]["k"], json!(REDACTED));
    assert!(!doc.contains(SECRET), "secret published:\n{doc}");
}

fn alias_chain(len: usize, last: Value) -> BTreeMap<String, Value> {
    let mut types = BTreeMap::new();
    for i in 0..len {
        let next = if i + 1 == len {
            last.clone()
        } else {
            json!({ "$ref": format!("#/types/A{}", i + 1), "description": format!("alias {i}") })
        };
        types.insert(format!("A{i}"), next);
    }
    types
}

/// DEN-47: a reference chain is followed to its end; one the walk cannot finish redacts the value
/// instead of passing it through; cycles are handled deterministically.
#[test]
fn reference_chains_are_resolved_or_fail_closed() {
    let secret_leaf = json!({ "type": "string", "x-secret": true });
    let plain_leaf = json!({ "type": "string" });
    let root = json!({ "$ref": "#/types/A0" });

    // The DEN-47 scenario: 19 aliases, beyond the old 16-hop bound.
    let types = alias_chain(19, secret_leaf.clone());
    let mut v = json!(SECRET);
    redact_secrets(&mut v, &root, &types);
    assert_eq!(
        v,
        json!(REDACTED),
        "a long alias chain reaches its secret mark"
    );
    let types = alias_chain(19, plain_leaf.clone());
    let mut v = json!(SECRET);
    redact_secrets(&mut v, &root, &types);
    assert_eq!(
        v,
        json!(SECRET),
        "the same chain without a mark keeps the value"
    );

    // Beyond the applicable-subschema bound: redacted whether or not a mark exists at the end.
    for leaf in [&secret_leaf, &plain_leaf] {
        let types = alias_chain(600, leaf.clone());
        let mut v = json!(SECRET);
        redact_secrets(&mut v, &root, &types);
        assert_eq!(v, json!(REDACTED), "an exhausted chain fails closed");
    }

    // Siblings of a `$ref` are unioned with the target: `x-secret: false` on the reference does not
    // un-mark a marked target, and a mark on the reference redacts an unmarked target.
    let mut types = BTreeMap::new();
    types.insert(
        "Marked".into(),
        json!({ "type": "string", "x-secret": true }),
    );
    types.insert("Plain".into(), json!({ "type": "string" }));
    let mut v = json!(SECRET);
    redact_secrets(
        &mut v,
        &json!({ "$ref": "#/types/Marked", "x-secret": false }),
        &types,
    );
    assert_eq!(
        v,
        json!(REDACTED),
        "a false sibling does not override a marked target"
    );
    let mut v = json!(SECRET);
    redact_secrets(
        &mut v,
        &json!({ "$ref": "#/types/Plain", "x-secret": true }),
        &types,
    );
    assert_eq!(
        v,
        json!(REDACTED),
        "a marked sibling redacts an unmarked target"
    );
    let mut v = json!(SECRET);
    redact_secrets(
        &mut v,
        &json!({ "$ref": "#/types/Plain", "x-secret": false }),
        &types,
    );
    assert_eq!(v, json!(SECRET));

    // Cycles: visited once per location, then decided by what was seen.
    let mut types = BTreeMap::new();
    types.insert("A".into(), json!({ "$ref": "#/types/B" }));
    types.insert("B".into(), json!({ "$ref": "#/types/A", "x-secret": true }));
    let mut v = json!(SECRET);
    redact_secrets(&mut v, &json!({ "$ref": "#/types/A" }), &types);
    assert_eq!(v, json!(REDACTED), "a marked cycle redacts");
    types.insert("B".into(), json!({ "$ref": "#/types/A", "type": "string" }));
    let mut v = json!(SECRET);
    redact_secrets(&mut v, &json!({ "$ref": "#/types/A" }), &types);
    assert_eq!(v, json!(SECRET), "an unmarked cycle keeps the value");

    // A recursive shared type: members are followed through the cycle until the depth bound.
    let mut types = BTreeMap::new();
    types.insert(
        "Node".into(),
        json!({ "type": "object", "properties": {
            "secret": { "type": "string", "x-secret": true },
            "child": { "$ref": "#/types/Node" } } }),
    );
    let mut v = json!({ "secret": SECRET, "child": { "secret": SECRET, "child": { "secret": SECRET, "name": "kept" } } });
    redact_secrets(&mut v, &json!({ "$ref": "#/types/Node" }), &types);
    assert_eq!(v["secret"], json!(REDACTED));
    assert_eq!(v["child"]["secret"], json!(REDACTED));
    assert_eq!(v["child"]["child"]["secret"], json!(REDACTED));
    assert_eq!(v["child"]["child"]["name"], json!("kept"));
}

/// Every position the walk cannot classify is redacted whole; the same schema without the problem
/// keeps the value, so each case proves the fail-closed path and not an accidental mark.
#[test]
fn unclassifiable_positions_fail_closed() {
    let types: BTreeMap<String, Value> =
        BTreeMap::from([("Plain".to_string(), json!({ "type": "string" }))]);
    let cases: Vec<(&str, Value, Value)> = vec![
        (
            "unresolvable local reference",
            json!({ "$ref": "#/types/Missing" }),
            json!({ "$ref": "#/types/Plain" }),
        ),
        (
            "reference outside the shared types",
            json!({ "$ref": "https://example.invalid/schema.json" }),
            json!({ "$ref": "#/types/Plain" }),
        ),
        (
            "reference into the document root",
            json!({ "$ref": "#" }),
            json!({ "$ref": "#/types/Plain" }),
        ),
        (
            "unmodelled keyword",
            json!({ "type": "string", "$dynamicRef": "#x" }),
            json!({ "type": "string" }),
        ),
        (
            "draft-07 dependencies",
            json!({ "type": "object", "dependencies": { "a": ["b"] } }),
            json!({ "type": "object", "dependentRequired": { "a": ["b"] } }),
        ),
        (
            "properties with the wrong shape",
            json!({ "type": "object", "properties": [] }),
            json!({ "type": "object", "properties": {} }),
        ),
        (
            "composition with the wrong shape",
            json!({ "allOf": { "type": "string" } }),
            json!({ "allOf": [{ "type": "string" }] }),
        ),
        (
            "non-boolean secret mark",
            json!({ "type": "string", "x-secret": "yes" }),
            json!({ "type": "string", "x-secret": false }),
        ),
        (
            "draft-07 tuple items",
            json!({ "type": "array", "items": [{ "type": "string" }] }),
            json!({ "type": "array", "prefixItems": [{ "type": "string" }] }),
        ),
        (
            "draft-07 additionalItems",
            json!({ "type": "array", "additionalItems": { "type": "string" } }),
            json!({ "type": "array", "items": { "type": "string" } }),
        ),
        (
            "contentSchema that could hide a mark",
            json!({ "type": "string", "contentEncoding": "base64", "contentSchema": { "type": "string" } }),
            json!({ "type": "string", "contentEncoding": "base64", "contentSchema": true }),
        ),
        (
            "reference into a local $defs",
            json!({ "$defs": { "X": { "type": "string" } }, "$ref": "#/$defs/X" }),
            json!({ "$defs": { "X": { "type": "string" } }, "$ref": "#/types/Plain" }),
        ),
        (
            "schema that is not a schema",
            json!({ "anyOf": [{ "type": "string" }, 7] }),
            json!({ "anyOf": [{ "type": "string" }, true] }),
        ),
    ];
    for (label, bad, good) in cases {
        let mut v = json!(SECRET);
        redact_secrets(&mut v, &bad, &types);
        assert_eq!(v, json!(REDACTED), "{label}: fails closed");
        let mut v = json!(SECRET);
        redact_secrets(&mut v, &good, &types);
        assert_eq!(
            v,
            json!(SECRET),
            "{label}: the well-formed twin keeps the value"
        );
        // The same problem one level down redacts only that member.
        let wrapped_bad =
            json!({ "type": "object", "properties": { "p": bad, "keep": { "type": "string" } } });
        let mut v = json!({ "p": SECRET, "keep": "kept" });
        redact_secrets(&mut v, &wrapped_bad, &types);
        assert_eq!(
            v,
            json!({ "p": REDACTED, "keep": "kept" }),
            "{label}: nested"
        );
    }

    // More subschemas at one location than the bound admits: redacted, marked or not.
    let fan_out = |n: usize, last: Value| {
        let mut branches: Vec<Value> = (0..n)
            .map(|i| json!({ "description": i.to_string() }))
            .collect();
        branches.push(last);
        json!({ "allOf": branches })
    };
    for last in [
        json!({ "type": "string", "x-secret": true }),
        json!({ "type": "string" }),
    ] {
        let mut v = json!(SECRET);
        redact_secrets(&mut v, &fan_out(600, last), &types);
        assert_eq!(
            v,
            json!(REDACTED),
            "a composition past the bound fails closed"
        );
    }
    let mut v = json!(SECRET);
    redact_secrets(
        &mut v,
        &fan_out(100, json!({ "type": "string", "x-secret": true })),
        &types,
    );
    assert_eq!(
        v,
        json!(REDACTED),
        "a wide composition within the bound reaches its mark"
    );
    let mut v = json!(SECRET);
    redact_secrets(&mut v, &fan_out(100, json!({ "type": "string" })), &types);
    assert_eq!(
        v,
        json!(SECRET),
        "a wide unmarked composition keeps the value"
    );

    // Nesting past the depth bound redacts the deep value whole and keeps the shallow ones.
    let mut schema = json!({ "type": "string" });
    let mut value = json!("leaf");
    for _ in 0..70 {
        schema = json!({ "type": "object", "properties": { "n": schema, "keep": { "type": "string" } } });
        value = json!({ "n": value, "keep": "kept" });
    }
    redact_secrets(&mut value, &schema, &types);
    assert_eq!(value["keep"], json!("kept"));
    let mut cursor = &value;
    let mut redacted_at = None;
    for depth in 0..70 {
        if cursor["n"] == json!(REDACTED) {
            redacted_at = Some(depth);
            break;
        }
        cursor = &cursor["n"];
    }
    // The member nested 65 deep is the first past MAX_REDACT_DEPTH = 64; the loop counts its parent.
    assert_eq!(
        redacted_at,
        Some(64),
        "the first value past the depth bound is redacted whole"
    );
}

/// The unions this walk consults beyond the round-2 findings: `dependentSchemas`, `not`,
/// `unevaluatedProperties`, `contains`, `unevaluatedItems`, and a secret mark on a `$ref` sibling.
#[test]
fn every_applicator_is_consulted() {
    let types: BTreeMap<String, Value> = BTreeMap::from([(
        "Creds".to_string(),
        json!({ "type": "object", "properties": { "pw": { "type": "string", "x-secret": true } } }),
    )]);
    let schema = json!({
        "type": "object",
        "properties": {
            "flag": { "type": "boolean" },
            "list": { "type": "array", "contains": { "x-secret": true } },
            "tuple": { "type": "array", "prefixItems": [{ "type": "integer" }], "unevaluatedItems": { "x-secret": true } },
            "creds": { "$ref": "#/types/Creds", "description": "by reference" },
            "whole": { "$ref": "#/types/Creds", "x-secret": true },
            "neg": { "not": { "x-secret": true } },
            "bag": { "type": "object", "properties": { "kept": { "type": "string" } },
                     "unevaluatedProperties": { "x-secret": true } },
            "names": { "type": "object", "propertyNames": { "x-secret": true } }
        },
        "dependentSchemas": { "flag": { "properties": { "dep": { "x-secret": true } } } }
    });
    let mut v = json!({
        "flag": true,
        "list": [SECRET, SECRET],
        "tuple": [1, SECRET],
        "creds": { "pw": SECRET, "user": "kept" },
        "whole": { "pw": SECRET },
        "neg": SECRET,
        "dep": SECRET,
        "bag": { "kept": "kept", "stray": SECRET },
        "names": { "SYNTHETIC_SECRET_KEY": 1 }
    });
    redact_secrets(&mut v, &schema, &types);
    assert_eq!(v["flag"], json!(true));
    assert_eq!(v["list"], json!([REDACTED, REDACTED]), "contains");
    assert_eq!(
        v["tuple"],
        json!([1, REDACTED]),
        "unevaluatedItems applies to the items prefixItems/items did not evaluate"
    );
    assert_eq!(v["creds"]["pw"], json!(REDACTED), "ref target");
    assert_eq!(v["creds"]["user"], json!("kept"));
    assert_eq!(v["whole"], json!(REDACTED), "ref sibling mark");
    assert_eq!(v["neg"], json!(REDACTED), "a mark under not still counts");
    assert_eq!(v["dep"], json!(REDACTED), "dependentSchemas");
    assert_eq!(
        v["bag"],
        json!({ "kept": "kept", "stray": REDACTED }),
        "unevaluatedProperties applies to the members the same subschema did not evaluate"
    );
    assert_eq!(
        v["names"],
        json!(REDACTED),
        "a mark on propertyNames redacts the object, since keys cannot be rewritten"
    );
    assert!(
        !serde_json::to_string(&v)
            .unwrap()
            .contains("SYNTHETIC_SECRET_KEY")
    );
    assert!(!serde_json::to_string(&v).unwrap().contains(SECRET));
}

/// One way of nesting a leaf schema inside a keyword, with the matching value.
#[derive(Clone, Copy, Debug)]
enum Wrapper {
    Properties,
    NamedAndPattern,
    Pattern,
    Additional,
    Unevaluated,
    AllOf,
    AnyOf,
    OneOf,
    Then,
    Else,
    Ref,
    Items,
    PrefixItems,
    Contains,
    DependentSchemas,
    Not,
}

const WRAPPERS: [Wrapper; 16] = [
    Wrapper::Properties,
    Wrapper::NamedAndPattern,
    Wrapper::Pattern,
    Wrapper::Additional,
    Wrapper::Unevaluated,
    Wrapper::AllOf,
    Wrapper::AnyOf,
    Wrapper::OneOf,
    Wrapper::Then,
    Wrapper::Else,
    Wrapper::Ref,
    Wrapper::Items,
    Wrapper::PrefixItems,
    Wrapper::Contains,
    Wrapper::DependentSchemas,
    Wrapper::Not,
];

impl Wrapper {
    /// Wraps `(schema, value)` so that `schema` applies to `value` at the new location, with an
    /// unmarked `KEEP` sibling wherever the keyword admits one.
    fn apply(
        self,
        schema: Value,
        value: Value,
        types: &mut BTreeMap<String, Value>,
        n: usize,
    ) -> (Value, Value) {
        let keep = json!({ "type": "string" });
        match self {
            Wrapper::Properties => (
                json!({ "type": "object", "properties": { "p": schema, "keep": keep }, "required": ["p"] }),
                json!({ "p": value, "keep": "KEEP" }),
            ),
            Wrapper::NamedAndPattern => (
                json!({ "type": "object", "properties": { "api_key": { "description": "named" }, "keep": keep },
                        "patternProperties": { "^api_": schema } }),
                json!({ "api_key": value, "keep": "KEEP" }),
            ),
            Wrapper::Pattern => (
                json!({ "type": "object", "patternProperties": { "^k_": schema, "^keep$": keep } }),
                json!({ "k_1": value, "keep": "KEEP" }),
            ),
            Wrapper::Additional => (
                json!({ "type": "object", "properties": { "keep": keep }, "additionalProperties": schema }),
                json!({ "other": value, "keep": "KEEP" }),
            ),
            Wrapper::Unevaluated => (
                json!({ "type": "object", "properties": { "keep": keep }, "unevaluatedProperties": schema }),
                json!({ "x": value, "keep": "KEEP" }),
            ),
            Wrapper::AllOf => (json!({ "allOf": [{ "description": "c" }, schema] }), value),
            Wrapper::AnyOf => (json!({ "anyOf": [{ "type": "null" }, schema] }), value),
            Wrapper::OneOf => (json!({ "oneOf": [schema, { "type": "null" }] }), value),
            Wrapper::Then => (
                json!({ "if": { "description": "always" }, "then": schema }),
                value,
            ),
            Wrapper::Else => (json!({ "if": { "not": {} }, "else": schema }), value),
            Wrapper::Ref => {
                let name = format!("T{n}");
                types.insert(name.clone(), schema);
                (
                    json!({ "$ref": format!("#/types/{name}"), "description": "alias" }),
                    value,
                )
            }
            Wrapper::Items => (json!({ "type": "array", "items": schema }), json!([value])),
            Wrapper::PrefixItems => (
                json!({ "type": "array", "prefixItems": [keep, schema] }),
                json!(["KEEP", value]),
            ),
            Wrapper::Contains => (
                json!({ "type": "array", "contains": schema }),
                json!([value]),
            ),
            Wrapper::DependentSchemas => (
                json!({ "type": "object", "properties": { "flag": { "type": "boolean" }, "keep": keep },
                        "dependentSchemas": { "flag": { "properties": { "p": schema } } } }),
                json!({ "flag": true, "p": value, "keep": "KEEP" }),
            ),
            Wrapper::Not => (json!({ "not": { "not": schema } }), value),
        }
    }
}

fn sequences(len: usize) -> Vec<Vec<Wrapper>> {
    if len == 0 {
        return vec![vec![]];
    }
    let mut out = Vec::new();
    for rest in sequences(len - 1) {
        for w in WRAPPERS {
            let mut seq = rest.clone();
            seq.push(w);
            out.push(seq);
        }
    }
    out
}

/// Validates `value` against `schema` with the shared types reachable as `#/types/<Name>`, the way
/// `protocol.json` lays them out, so every generated case is a real schema with a valid instance.
fn assert_valid(schema: &Value, value: &Value, types: &BTreeMap<String, Value>, label: &str) {
    let mut root = schema.clone();
    root["types"] = json!(types);
    let validator = jsonschema::validator_for(&root).unwrap_or_else(|e| panic!("{label}: {e}"));
    assert!(
        validator.is_valid(value),
        "{label}: generated instance is invalid\n{root}\n{value}"
    );
}

/// Generated table: every sequence of up to three keyword wrappers around a marked leaf hides the
/// secret and keeps every unmarked sibling; the same sequence around an unmarked leaf changes nothing.
/// `$ref` wrappers go through the shared-types table, so chains, aliases of compositions and
/// compositions of aliases are all covered. Every instance is validated against its schema first.
#[test]
fn keyword_combinations_never_publish_a_marked_secret() {
    let mut cases = 0usize;
    for len in 1..=3 {
        for seq in sequences(len) {
            let mut types = BTreeMap::new();
            let (mut schema, mut value) =
                (json!({ "type": "string", "x-secret": true }), json!(SECRET));
            let (mut twin, mut twin_value) = (json!({ "type": "string" }), json!(SECRET));
            for (i, w) in seq.iter().enumerate() {
                (schema, value) = w.apply(schema, value, &mut types, cases * 8 + i * 2);
                (twin, twin_value) = w.apply(twin, twin_value, &mut types, cases * 8 + i * 2 + 1);
            }
            let label = format!("{seq:?}");
            assert_valid(&schema, &value, &types, &label);
            assert_valid(&twin, &twin_value, &types, &label);
            let keeps = serde_json::to_string(&value)
                .unwrap()
                .matches("KEEP")
                .count();

            let mut redacted = value.clone();
            redact_secrets(&mut redacted, &schema, &types);
            let text = serde_json::to_string(&redacted).unwrap();
            assert!(
                !text.contains(SECRET),
                "{seq:?}: secret published:\n{schema}\n{text}"
            );
            assert!(text.contains(REDACTED), "{seq:?}: nothing was redacted");
            assert_eq!(
                text.matches("KEEP").count(),
                keeps,
                "{seq:?}: an unmarked sibling was redacted\n{schema}\n{text}"
            );
            let mut again = redacted.clone();
            redact_secrets(&mut again, &schema, &types);
            assert_eq!(again, redacted, "{seq:?}: idempotent");

            let mut untouched = twin_value.clone();
            redact_secrets(&mut untouched, &twin, &types);
            assert_eq!(
                untouched, twin_value,
                "{seq:?}: an unmarked schema redacts nothing\n{twin}"
            );
            cases += 1;
        }
    }
    assert_eq!(cases, 16 + 16 * 16 + 16 * 16 * 16);
}
