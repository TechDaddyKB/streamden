//! Secret redaction at the publication boundary (Spec 02 §3): the walk behind
//! `codegen::redact_secrets` is structurally fail-closed. Regressions for the Codex round-2 review of
//! DEN-34 (overlapping named and pattern properties, conditional schemas), round 3 (a `$ref` sibling
//! erasing the target's mark during inlining; pattern dialect drift between redactor and validator) and
//! DEN-47 (long reference chains), round 4 (optional-null normalisation dropping the keys an optional
//! property shares with its surviving branch), plus an exhaustive keyword-combination generator and a
//! differential test of pattern applicability against the validator. Every value here is synthetic.

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
    publish_from(Catalog::new(), params, example)
}

/// [`publish`] with a catalog that may already hold shared types (registered through
/// `Catalog::ref_for`, the way a real operation references them).
fn publish_from(mut catalog: Catalog, params: Value, example: Value) -> (Value, String) {
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

/// A shared type marked secret as a whole (every value of it is a secret), the round-3 reproduction.
#[derive(schemars::JsonSchema)]
#[schemars(extend("x-secret" = true))]
#[allow(dead_code)]
struct Marked {
    /// The token.
    token: String,
}

/// A shared type with one marked field, not marked as a whole.
#[derive(schemars::JsonSchema)]
#[allow(dead_code)]
struct Creds {
    /// The user.
    user: String,
    /// The token.
    #[schemars(extend("x-secret" = true))]
    token: String,
}

/// Codex round 3, P1 #1: secrecy is monotonic across reference inlining. The derived invocation schema
/// inlines a structured shared target with the property's siblings merged in; a sibling
/// `x-secret: false` must not replace the target's `true` there. End to end through `Catalog::ref_for`
/// and `build_unredacted`: the lint rejects the contradiction before anything is generated, and should
/// the lint ever be bypassed, the derivation keeps the mark and publication redacts.
#[test]
fn inlined_references_keep_the_targets_mark() {
    // 1. End to end: the contradictory sibling fails the catalog lint.
    let mut catalog = Catalog::new();
    let mut creds = catalog.ref_for::<Marked>().to_value();
    creds["x-secret"] = json!(false);
    let params = json!({ "type": "object", "properties": { "creds": creds } });
    catalog
        .register_operation(op(
            "demo.op",
            params.clone(),
            json!({ "creds": { "token": SECRET } }),
        ))
        .unwrap();
    let err = codegen::build_unredacted(catalog)
        .expect_err("x-secret: false beside a $ref to a marked type fails lint");
    let text = err.to_string();
    assert!(
        text.contains("params/properties/creds")
            && text.contains("Marked")
            && text.contains("monotonic"),
        "{text}"
    );

    // 2. Defence in depth: derivation alone keeps the target's mark, and the derived schema redacts.
    let types: BTreeMap<String, Value> = BTreeMap::from([(
        "Marked".to_string(),
        json!({ "type": "object", "x-secret": true, "properties": { "token": { "type": "string" } } }),
    )]);
    let (invocation, authoring) =
        codegen::derive_param_schemas(&Schema::try_from(params.clone()).unwrap(), &types);
    for (label, schema) in [("params", invocation), ("paramsAuthoring", authoring)] {
        let v = schema.to_value();
        let creds = &v["properties"]["creds"];
        assert!(
            creds.get("$ref").is_none(),
            "{label}: the structured target was inlined"
        );
        assert_eq!(
            creds["x-secret"],
            json!(true),
            "{label}: the target's mark survives a false sibling"
        );
        let example = json!({ "creds": { "token": SECRET } });
        assert_valid(&v, &example, &types, label);
        let mut published = example;
        redact_secrets(&mut published, &v, &types);
        assert_eq!(published, json!({ "creds": REDACTED }), "{label}");
    }

    // A malformed mark on either side stays malformed through the merge, so publication fails closed.
    for (sibling, target) in [
        (json!(false), json!("yes")),
        (json!("yes"), json!(true)),
        (json!("yes"), json!(false)),
    ] {
        let types: BTreeMap<String, Value> = BTreeMap::from([(
            "T".to_string(),
            json!({ "type": "object", "x-secret": target, "properties": { "token": { "type": "string" } } }),
        )]);
        let params = json!({ "type": "object", "properties": { "creds": { "$ref": "#/types/T", "x-secret": sibling } } });
        let (invocation, _) =
            codegen::derive_param_schemas(&Schema::try_from(params).unwrap(), &types);
        let v = invocation.to_value();
        assert_eq!(
            v["properties"]["creds"]["x-secret"],
            json!("yes"),
            "{sibling} over {target}"
        );
        let mut published = json!({ "creds": { "token": SECRET }, "other": "kept" });
        redact_secrets(&mut published, &v, &types);
        assert_eq!(published, json!({ "creds": REDACTED, "other": "kept" }));
    }

    // 3. The shapes the lint accepts publish correctly through the whole pipeline: an annotation
    // sibling on a reference to an unmarked type whose field is marked (the field stays marked after
    // inlining), and a plain annotation sibling on a reference to a type marked as a whole.
    let mut catalog = Catalog::new();
    let mut creds = catalog.ref_for::<Creds>().to_value();
    creds["x-ui"] = json!({ "variables": false });
    let (published, doc) = publish_from(
        catalog,
        json!({ "type": "object", "properties": { "creds": creds } }),
        json!({ "creds": { "user": "kept", "token": SECRET } }),
    );
    assert_eq!(
        published["creds"],
        json!({ "user": "kept", "token": REDACTED })
    );
    assert!(!doc.contains(SECRET), "secret published:\n{doc}");

    let mut catalog = Catalog::new();
    let mut whole = catalog.ref_for::<Marked>().to_value();
    whole["description"] = json!("Local description");
    let (published, doc) = publish_from(
        catalog,
        json!({ "type": "object", "properties": { "whole": whole, "name": { "type": "string" } } }),
        json!({ "whole": { "token": SECRET }, "name": "kept" }),
    );
    assert_eq!(published, json!({ "whole": REDACTED, "name": "kept" }));
    assert!(!doc.contains(SECRET), "secret published:\n{doc}");
}

/// The lint on `x-secret` marks: a non-boolean mark anywhere and every `x-secret: false` are reported
/// with their position; beside a `$ref` whose target chain is marked (directly or through aliases) the
/// finding names the type. A `false` is never a no-op worth keeping: §3 marks with `true` only, and
/// (round 5) a field-level `#[schemars(extend("x-secret" = false))]` replaces the mark an inline type
/// carries before this crate sees the schema, so the only safe rule is to reject it everywhere.
#[test]
fn lint_reports_marks_that_cannot_hold() {
    let lint_of = |params: Value, result: Value| -> Vec<String> {
        let mut catalog = Catalog::new();
        catalog.register_type::<Marked>();
        catalog.register_type::<Creds>();
        let mut def = op("demo.op", params, json!({}));
        def.result = Schema::try_from(result).unwrap();
        catalog.register_operation(def).unwrap();
        codegen::lint(&catalog)
            .into_iter()
            .map(|e| e.to_string())
            .collect()
    };
    let object = json!({ "type": "object" });

    let findings = lint_of(
        json!({ "type": "object", "properties": {
            "a": { "$ref": "#/types/Marked", "x-secret": false },
            "b": { "$ref": "#/types/Creds", "x-secret": false },
            "c": { "type": "string", "x-secret": "yes" },
            "d": { "type": "array", "items": { "anyOf": [{ "$ref": "#/types/Marked", "x-secret": false }] } }
        } }),
        json!({ "type": "object", "properties": { "r": { "$ref": "#/types/Marked", "x-secret": false } } }),
    );
    let expect = |needle: &str| {
        assert!(
            findings.iter().any(|f| f.contains(needle)),
            "missing {needle:?} in {findings:#?}"
        );
    };
    expect("params/properties/a: `x-secret: false` beside a $ref to `Marked`");
    expect("params/properties/c: x-secret must be a boolean, got \"yes\"");
    expect("params/properties/d/items/anyOf/0: `x-secret: false` beside a $ref to `Marked`");
    expect("result/properties/r: `x-secret: false` beside a $ref to `Marked`");
    expect("params/properties/b: `x-secret: false` has no effect");
    assert_eq!(findings.len(), 5, "{findings:#?}");

    // Through an alias chain or a composition that applies to the same value, and a malformed mark
    // on the target counts as marked.
    let mut catalog = Catalog::new();
    catalog.register_type::<Marked>();
    #[derive(schemars::JsonSchema)]
    #[allow(dead_code)]
    struct Alias(Marked);
    catalog.register_type::<Alias>();
    fn wrap_in_all_of(schema: &mut Schema) {
        *schema = schemars::json_schema!({ "allOf": [{ "$ref": "#/types/Marked" }] });
    }
    #[derive(schemars::JsonSchema)]
    #[schemars(transform = wrap_in_all_of)]
    #[allow(dead_code)]
    struct Wrap(Marked);
    catalog.register_type::<Wrap>();
    assert_eq!(
        catalog.peek_definitions()["Wrap"],
        json!({ "allOf": [{ "$ref": "#/types/Marked" }] })
    );
    assert_eq!(
        catalog.peek_definitions()["Alias"],
        json!({ "$ref": "#/types/Marked" }),
        "a newtype is a reference alias of its inner type"
    );
    assert_eq!(
        lint_of(object.clone(), object.clone()),
        Vec::<String>::new()
    );
    let clean: Vec<String> = codegen::lint(&catalog)
        .iter()
        .map(|e| e.to_string())
        .collect();
    assert_eq!(clean, Vec::<String>::new());
    catalog
        .register_operation(op(
            "demo.op",
            json!({ "type": "object", "properties": {
                "a": { "$ref": "#/types/Alias", "x-secret": false },
                "w": { "$ref": "#/types/Wrap", "x-secret": false }
            } }),
            json!({}),
        ))
        .unwrap();
    let findings: Vec<String> = codegen::lint(&catalog)
        .iter()
        .map(|e| e.to_string())
        .collect();
    for at in ["params/properties/a", "params/properties/w"] {
        assert!(
            findings
                .iter()
                .any(|f| f.contains(at) && f.contains("beside a $ref to `Marked`")),
            "{at}: {findings:#?}"
        );
    }
    assert_eq!(findings.len(), 2, "{findings:#?}");
}

/// The two round-3 P1 #2 reproductions, and the applicability control Codex used: the validator
/// applies the entry (a `type: integer` twin rejects the string member), so the published example must
/// redact it, whatever a Rust regex thinks of `\W` (Unicode-aware there, ASCII in ECMA-262) or `\s`
/// (ECMA-262 includes U+FEFF).
#[test]
fn pattern_properties_match_in_the_validators_dialect() {
    for (pattern, key) in [(r"^\W+$", "é"), (r"^\skey$", "\u{FEFF}key")] {
        let control = jsonschema::validator_for(
            &json!({ "type": "object", "patternProperties": { pattern: { "type": "integer" } } }),
        )
        .unwrap();
        assert!(
            !control.is_valid(&json!({ key: "s" })),
            "the validator applies {pattern} to {key:?}, or the case proves nothing"
        );
        let params = json!({ "type": "object", "patternProperties": { pattern: { "type": "string", "x-secret": true } } });
        let (published, doc) = publish(params, json!({ key: SECRET }));
        assert_eq!(published[key], json!(REDACTED), "{pattern} on {key:?}");
        assert!(
            !doc.contains(SECRET),
            "{pattern} on {key:?}: secret published:\n{doc}"
        );
    }
}

/// Differential test: over generated member names and patterns, the redactor applies a marked
/// `patternProperties` entry exactly when the validator does. The validator's verdict is observed
/// without the redactor, through a `type: integer` twin of the entry (a string member is rejected
/// exactly when the pattern selects it). Patterns the validator cannot compile redact every member,
/// marked or not, and whatever the fallback entries say.
#[test]
fn pattern_applicability_agrees_with_the_validator() {
    const KEYS: &[&str] = &[
        "key",
        "api_key",
        "é",
        "\u{FEFF}key",
        " key",
        "\tkey",
        "k",
        "1",
        "١",
        "ß",
        "ｋey",
        "😀",
        "",
        "a b",
        "KEY",
        "key\n",
        "_",
        "-",
        "\u{00A0}key",
        "x\u{200B}",
        "Ab",
        "a.b",
        "\\",
        "$",
        "^",
    ];
    const PATTERNS: &[&str] = &[
        r"^\W+$",
        r"^\skey$",
        r"^\w+$",
        r"^\d$",
        r"^\D+$",
        r"^\S+$",
        r"^.$",
        r"^[^a-z]+$",
        r"^\bkey$",
        r"^[\s\S]+$",
        r"^é$",
        r"^\x41b$",
        r"^(?i)key$",
        r"^(?<n>k)ey$",
        r"^\p{L}+$",
        r"^[[:alpha:]]+$",
        r"^key$",
        "key",
        r"^(key|api_key)$",
        "(",
        "[",
        r"^\",
        r"^\cJ$",
        r"^$",
        r"^[é]$",
        r"^.+$",
        r"^\u{1F600}$",
        r"(?<=a)b",
        r"^(a)\1$",
        r"^\$$",
        r"^[\w-]+$",
        r"^[^\W_]+$",
        r"^.*\.b$",
        r"^\p{Letter}$",
        r"^\x{41}$",
    ];
    let mut applied = 0usize;
    let mut skipped = 0usize;
    let mut uncompilable = 0usize;
    for pattern in PATTERNS {
        let control = jsonschema::validator_for(
            &json!({ "type": "object", "patternProperties": { *pattern: { "type": "integer" } } }),
        );
        for key in KEYS {
            let params = json!({
                "type": "object",
                "patternProperties": { *pattern: { "type": "string", "x-secret": true } },
                "additionalProperties": { "type": "string" }
            });
            let example = json!({ *key: SECRET });
            match &control {
                Err(_) => {
                    // The validator rejects the pattern: no example could be validated against this
                    // schema, and the redactor cannot tell which members the entry (or the fallback)
                    // covers, so every member redacts whole: with the mark on the entry, with it on
                    // the fallback instead, and with no mark anywhere.
                    let unmarked_entry = json!({
                        "type": "object",
                        "patternProperties": { *pattern: { "type": "string" } },
                        "additionalProperties": { "type": "string", "x-secret": true }
                    });
                    let no_mark = json!({
                        "type": "object",
                        "patternProperties": { *pattern: { "type": "string" } },
                        "unevaluatedProperties": { "type": "string" }
                    });
                    for (label, schema) in [
                        ("marked entry", &params),
                        ("marked fallback", &unmarked_entry),
                        ("no mark", &no_mark),
                    ] {
                        let mut v = example.clone();
                        redact_secrets(&mut v, schema, &BTreeMap::new());
                        assert_eq!(
                            v[*key],
                            json!(REDACTED),
                            "{pattern} on {key:?}: uncompilable fails closed ({label})"
                        );
                    }
                    uncompilable += 1;
                }
                Ok(control) => {
                    let applies = !control.is_valid(&json!({ *key: "s" }));
                    let (published, doc) = publish(params, example);
                    if applies {
                        assert_eq!(
                            published[*key],
                            json!(REDACTED),
                            "{pattern} on {key:?}: the validator applies the entry"
                        );
                        assert!(
                            !doc.contains(SECRET),
                            "{pattern} on {key:?}: secret published:\n{doc}"
                        );
                        applied += 1;
                    } else {
                        assert_eq!(
                            published[*key],
                            json!(SECRET),
                            "{pattern} on {key:?}: the validator does not apply the entry, so the (unmarked additionalProperties) member is published"
                        );
                        skipped += 1;
                    }
                }
            }
        }
    }
    assert!(
        applied > 0 && skipped > 0 && uncompilable > 0,
        "{applied} {skipped} {uncompilable}"
    );
    assert_eq!(
        applied + skipped + uncompilable,
        KEYS.len() * PATTERNS.len()
    );
}

/// Codex round 4, P1: an optional property arrives as `anyOf: [T, null]` beside its own keys; the
/// `DenyNullForOptional` transform lifted `T` into the property with `entry().or_insert()`, so every
/// key the property already had shadowed the branch's, secret marks and all, before the publication
/// redactor ever saw the schema. Both repros go through the real pipeline (`register_operation →
/// build_unredacted → validate → redact_examples → to_json_string`).
#[test]
fn optional_properties_keep_both_schemas() {
    // Repro A: the property's `x-secret: false` sat on the branch's `x-secret: true`. The pipeline
    // now fails closed at the lint (an explicit `false` is never accepted, see
    // `lint_reports_marks_that_cannot_hold`); should one ever get through, the normalisation joins the
    // two marks to `true` (`schema::tests::strip_null_keeps_both_schemas`) and the published example
    // is redacted whole.
    let mut catalog = Catalog::new();
    catalog
        .register_operation(op(
            "demo.op",
            json!({
                "type": "object",
                "properties": {
                    "creds": {
                        "x-secret": false,
                        "anyOf": [
                            { "type": "object", "x-secret": true, "properties": { "token": { "type": "string" } } },
                            { "type": "null" }
                        ]
                    }
                }
            }),
            json!({ "creds": { "token": SECRET } }),
        ))
        .unwrap();
    let err = codegen::build_unredacted(catalog)
        .expect_err("an explicit `x-secret: false` fails the lint")
        .to_string();
    assert!(
        err.contains("params/properties/creds") && err.contains("`x-secret: false` has no effect"),
        "{err}"
    );
    let params = json!({
        "type": "object",
        "properties": {
            "creds": {
                "description": "A joined mark.",
                "anyOf": [
                    { "type": "object", "x-secret": true, "properties": { "token": { "type": "string" } } },
                    { "type": "null" }
                ]
            }
        }
    });
    let (published, doc) = publish(params, json!({ "creds": { "token": SECRET } }));
    assert_eq!(published, json!({ "creds": REDACTED }));
    assert!(!doc.contains(SECRET), "secret published:\n{doc}");
    let doc: Value = serde_json::from_str(&doc).unwrap();
    let creds = &doc["operations"]["demo.op"]["params"]["properties"]["creds"];
    assert_eq!(
        (creds["x-secret"].clone(), creds["description"].clone()),
        (json!(true), json!("A joined mark.")),
        "the branch is lifted into the property, mark included"
    );

    // Repro B: no false mark anywhere; the property's own `properties` shadowed the branch's, which
    // held the mark.
    let params = json!({
        "type": "object",
        "properties": {
            "creds": {
                "properties": { "token": { "type": "string" } },
                "anyOf": [
                    { "type": "object", "properties": { "token": { "type": "string", "x-secret": true } } },
                    { "type": "null" }
                ]
            }
        }
    });
    let (published, doc) = publish(
        params,
        json!({ "creds": { "token": SECRET, "user": "kept" } }),
    );
    assert_eq!(
        published,
        json!({ "creds": { "token": REDACTED, "user": "kept" } })
    );
    assert!(!doc.contains(SECRET), "secret published:\n{doc}");
    let doc: Value = serde_json::from_str(&doc).unwrap();
    let creds = &doc["operations"]["demo.op"]["params"]["properties"]["creds"];
    assert_eq!(
        creds["properties"]["token"]["type"],
        json!("string"),
        "the property's own constraint is still there"
    );
    assert_eq!(
        creds["allOf"][0]["properties"]["token"]["x-secret"],
        json!(true),
        "and so is the branch, whole, in conjunction"
    );

    // Both sides still constrain after normalisation: the property's `required` and the branch's
    // member type each reject on their own.
    let mut catalog = Catalog::new();
    catalog
        .register_operation(op(
            "demo.op",
            json!({
                "type": "object",
                "properties": {
                    "creds": {
                        "required": ["token"],
                        "anyOf": [
                            { "type": "object", "properties": { "token": { "type": "string", "x-secret": true } } },
                            { "type": "null" }
                        ]
                    }
                }
            }),
            json!({ "creds": { "token": SECRET } }),
        ))
        .unwrap();
    let doc = codegen::build_unredacted(catalog).unwrap();
    let validator =
        jsonschema::validator_for(&doc.schema_at("/operations/demo.op/params")).unwrap();
    assert!(validator.is_valid(&json!({ "creds": { "token": "t" } })));
    assert!(
        !validator.is_valid(&json!({ "creds": {} })),
        "the property's `required` still applies"
    );
    assert!(
        !validator.is_valid(&json!({ "creds": { "token": 1 } })),
        "the branch's member type still applies"
    );
    assert!(
        !validator.is_valid(&json!({ "creds": null })),
        "null is still gone (§3)"
    );
    let mut doc = doc;
    doc.redact_examples();
    assert!(!doc.to_json_string().contains(SECRET));

    // What the real generator does with an optional field, pinned: a referenced type keeps the
    // `anyOf: [$ref, null]` form the normalisation lifts; an `#[schemars(inline)]` type is merged into
    // the field by schemars itself (`type: ["object", "null"]`, keys beside the field's own) before any
    // transform of this crate runs, and a field-level `#[schemars(extend(...))]` then *replaces* the
    // keys the inline type brought. That is why the lint rejects every `x-secret: false` and why
    // `crates/den-protocol/AGENTS.md` allows `extend` for `x-` annotations only.
    #[derive(schemars::JsonSchema)]
    #[schemars(inline)]
    struct InlineCreds {
        #[allow(dead_code)]
        user: String,
        #[allow(dead_code)]
        #[schemars(extend("x-secret" = true))]
        token: String,
    }
    #[derive(schemars::JsonSchema)]
    #[schemars(inline, extend("x-secret" = true))]
    struct InlineWhole {
        #[allow(dead_code)]
        token: String,
    }
    #[derive(schemars::JsonSchema)]
    struct Params {
        #[allow(dead_code)]
        creds: Option<Creds>,
        #[allow(dead_code)]
        inline_creds: Option<InlineCreds>,
        #[allow(dead_code)]
        inline_whole: Option<InlineWhole>,
    }
    let mut catalog = Catalog::new();
    let params = catalog.schema_for::<Params>().to_value();
    assert!(
        params["properties"]["creds"]["anyOf"].is_array(),
        "a referenced optional is anyOf [$ref, null]: {params}"
    );
    assert_eq!(
        params["properties"]["inline_creds"]["type"],
        json!(["object", "null"]),
        "an inline optional is merged by schemars: {params}"
    );
    assert_eq!(
        params["properties"]["inline_whole"]["x-secret"],
        json!(true)
    );
    let (published, doc) = publish_from(
        catalog,
        params,
        json!({
            "creds": { "user": "kept", "token": SECRET },
            "inline_creds": { "user": "kept", "token": SECRET },
            "inline_whole": { "token": SECRET }
        }),
    );
    assert_eq!(
        published,
        json!({
            "creds": { "user": "kept", "token": REDACTED },
            "inline_creds": { "user": "kept", "token": REDACTED },
            "inline_whole": REDACTED
        })
    );
    assert!(!doc.contains(SECRET), "secret published:\n{doc}");

    #[derive(schemars::JsonSchema)]
    struct Overridden {
        /// A field-level `false` replaces the inline type's `true` inside schemars.
        #[allow(dead_code)]
        #[schemars(extend("x-secret" = false))]
        whole: Option<InlineWhole>,
    }
    let mut catalog = Catalog::new();
    let params = catalog.schema_for::<Overridden>().to_value();
    assert_eq!(
        params["properties"]["whole"]["x-secret"],
        json!(false),
        "schemars `extend` overrides, it does not join: {params}"
    );
    catalog
        .register_operation(op(
            "demo.op",
            params,
            json!({ "whole": { "token": SECRET } }),
        ))
        .unwrap();
    let err = codegen::build_unredacted(catalog)
        .expect_err("the lint is the only place left to catch it")
        .to_string();
    assert!(
        err.contains("params/properties/whole") && err.contains("`x-secret: false` has no effect"),
        "{err}"
    );
}
