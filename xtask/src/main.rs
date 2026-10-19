//! Repository automation (Spec 02 §9.1, §9.7).
//!
//! ```text
//! cargo xtask codegen            # regenerate schema/ and sdk/ts/src/gen/
//! cargo xtask codegen --check    # regenerate into memory and fail on any drift
//! cargo xtask lint-catalog       # catalog lint + example validation only
//! ```
//!
//! Content comes from `den_protocol::codegen`; this binary only validates examples, decides where files
//! go and compares them. Output is byte-identical across OSes: sorted keys, `\n` endings (§14).

use std::collections::BTreeMap;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use den_protocol::catalog::{Catalog, ResultShape};
use den_protocol::codegen::{self, ProtocolDocument};
use serde_json::Value;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// Generated files relative to the repository root.
const PROTOCOL_JSON: &str = "schema/protocol.json";
const PLUGIN_SCHEMA_JSON: &str = "schema/plugin.schema.json";
const TS_GEN_DIR: &str = "sdk/ts/src/gen";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("codegen") => match args.get(1).map(String::as_str) {
            None => codegen_write(),
            Some("--check") => codegen_check(),
            Some(other) => Err(format!("unknown codegen flag {other:?}; expected --check").into()),
        },
        Some("lint-catalog") => lint_catalog(),
        Some("help") | Some("--help") | Some("-h") | None => {
            eprintln!("usage: cargo xtask <codegen [--check] | lint-catalog>");
            Ok(())
        }
        Some(other) => Err(format!("unknown command {other:?}").into()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives one level below the root")
        .to_path_buf()
}

/// Builds the document, validates every example, and returns all generated files keyed by their
/// repository-relative path.
fn generate() -> Result<BTreeMap<PathBuf, String>> {
    let doc = build_validated()?;
    let mut files = BTreeMap::new();
    files.insert(PathBuf::from(PROTOCOL_JSON), doc.to_json_string());
    files.insert(
        PathBuf::from(PLUGIN_SCHEMA_JSON),
        codegen::to_canonical_json(&codegen::plugin_manifest_schema()),
    );

    let scratch = std::env::temp_dir().join(format!(
        "den-xtask-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&scratch)?;
    let ts = codegen::generate_typescript(&scratch);
    let _ = std::fs::remove_dir_all(&scratch);
    for (rel, content) in ts? {
        files.insert(Path::new(TS_GEN_DIR).join(rel), content);
    }
    Ok(files)
}

/// Builds the document, validates every example as authored, then redacts secrets for publication
/// (§3): a redacted value need not match its schema, so validation comes first.
fn build_validated() -> Result<ProtocolDocument> {
    let mut doc = codegen::build_unredacted(Catalog::core())?;
    validate_examples(&doc)?;
    doc.redact_examples();
    Ok(doc)
}

fn lint_catalog() -> Result<()> {
    let doc = build_validated()?;
    println!(
        "lint-catalog: ok ({} operations, {} events, {} shared types)",
        doc.operations.len(),
        doc.events.len(),
        doc.types.len()
    );
    Ok(())
}

/// A validation failure without the instance: the values checked are the authored ones, secrets
/// exposed, and the validator's `Display` would echo them into CI output.
fn describe(e: &jsonschema::ValidationError<'_>) -> String {
    format!(
        "{:?} at instance path {:?}",
        e.kind(),
        e.instance_path().as_str()
    )
}

/// Examples are tests (§8.1, §17.2): every example's params validate against the invocation schema and
/// its result against exactly the shape it declares. The values checked are the authored ones (a typed
/// example keeps them aside with secrets exposed; its published form is already withheld, §3).
fn validate_examples(doc: &ProtocolDocument) -> Result<()> {
    let mut failures = Vec::new();
    for (name, entry) in &doc.operations {
        let params_schema = doc.schema_at(&format!("/operations/{name}/params"));
        let params_validator = jsonschema::validator_for(&params_schema)
            .map_err(|e| format!("operation {name}: params schema does not compile: {e}"))?;
        let shapes: Vec<(ResultShape, String)> = [
            (ResultShape::Normal, format!("/operations/{name}/result")),
            (
                ResultShape::DryRun,
                format!("/operations/{name}/dryRunResult"),
            ),
            (
                ResultShape::Draft,
                format!("/operations/{name}/draftResult"),
            ),
        ]
        .into_iter()
        .filter(|(shape, _)| match shape {
            ResultShape::Normal => true,
            ResultShape::DryRun => entry.dry_run_result.is_some(),
            ResultShape::Draft => entry.draft_result.is_some(),
        })
        .collect();
        let validators: Vec<(ResultShape, jsonschema::Validator)> = shapes
            .iter()
            .map(|(shape, pointer)| {
                jsonschema::validator_for(&doc.schema_at(pointer))
                    .map(|v| (*shape, v))
                    .map_err(|e| format!("operation {name}: {pointer} does not compile: {e}"))
            })
            .collect::<std::result::Result<_, _>>()?;
        for example in &entry.def.examples {
            let label = format!("operation {name}, example {:?}", example.title);
            let (params, result) = (example.authored_params(), example.authored_result());
            if let Err(e) = params_validator.validate(params) {
                failures.push(format!("{label}: params invalid: {}", describe(&e)));
            }
            for (shape, validator) in &validators {
                let valid = validator.is_valid(result);
                let expected = *shape == example.shape;
                if expected && !valid {
                    let e = validator
                        .validate(result)
                        .err()
                        .map(|e| describe(&e))
                        .unwrap_or_default();
                    failures.push(format!(
                        "{label}: result does not match its {shape:?} shape: {e}"
                    ));
                }
                if !expected && valid && entry.dry_run_result.is_some() {
                    failures.push(format!(
                        "{label}: result also matches the {shape:?} shape; shapes must be disjoint"
                    ));
                }
            }
        }
    }
    for (r#type, entry) in &doc.events {
        let schema = doc.schema_at(&format!("/events/{type}/payload"));
        let validator = jsonschema::validator_for(&schema)
            .map_err(|e| format!("event {type}: payload schema does not compile: {e}"))?;
        for i in 0..entry.def.examples.len() {
            if let Err(e) = validator.validate(entry.def.authored_example(i)) {
                failures.push(format!("event {type}, example {i}: {}", describe(&e)));
            }
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "example validation failed:\n  - {}",
            failures.join("\n  - ")
        )
        .into())
    }
}

fn codegen_write() -> Result<()> {
    let root = repo_root();
    let files = generate()?;
    let gen_dir = root.join(TS_GEN_DIR);
    if gen_dir.exists() {
        std::fs::remove_dir_all(&gen_dir)?;
    }
    for (rel, content) in &files {
        let path = root.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, content)?;
    }
    println!("codegen: wrote {} files", files.len());
    Ok(())
}

fn codegen_check() -> Result<()> {
    let root = repo_root();
    let files = generate()?;
    let mut drift = Vec::new();
    for (rel, content) in &files {
        // Generated files are LF; tolerate a CRLF checkout (core.autocrlf) when comparing (§14).
        match std::fs::read_to_string(root.join(rel)).map(|s| s.replace("\r\n", "\n")) {
            Ok(existing) if existing == *content => {}
            Ok(_) => drift.push(format!("{} (content differs)", rel.display())),
            Err(_) => drift.push(format!("{} (missing)", rel.display())),
        }
    }
    let gen_dir = root.join(TS_GEN_DIR);
    let mut existing = Vec::new();
    walk(&gen_dir, &mut existing);
    for path in existing {
        let rel = path.strip_prefix(&root).unwrap_or(&path).to_path_buf();
        if !files.contains_key(&rel) {
            drift.push(format!("{} (stale: not generated any more)", rel.display()));
        }
    }
    if drift.is_empty() {
        println!(
            "codegen --check: {} generated files are up to date",
            files.len()
        );
        Ok(())
    } else {
        Err(format!(
            "generated files are out of date; run `cargo xtask codegen` and commit the result:\n  - {}",
            drift.join("\n  - ")
        )
        .into())
    }
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, out)
        } else {
            out.push(path)
        }
    }
}

/// Keep the document type referenced so `schema_at` stays part of the public contract this binary uses.
#[allow(dead_code)]
fn _assert_value(_: &Value) {}
