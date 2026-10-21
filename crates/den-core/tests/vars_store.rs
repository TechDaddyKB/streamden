use den_core::storage::{
    documents::DocumentError,
    schema::{bootstrap_config, bootstrap_history},
    thread::{Admission, JobClass, StorageConns, StorageError, StorageHandle, StorageWorker},
    variables::{Mutation, Scope, Variables},
};
use den_protocol::{Timestamp, UpdatedBy, VariableRecord, rpc::ErrorCode};
use rusqlite::{Connection, params};
use serde_json::{Value, json};

async fn harness() -> (tempfile::TempDir, StorageWorker, Variables) {
    let temp = tempfile::tempdir().unwrap();
    let config = temp.path().join("config.db");
    let history = temp.path().join("history.db");
    let worker = StorageWorker::start(
        move || {
            let mut config = Connection::open(config).map_err(StorageError::Sqlite)?;
            let mut history = Connection::open(history).map_err(StorageError::Sqlite)?;
            bootstrap_config(&mut config).map_err(StorageError::Sqlite)?;
            bootstrap_history(&mut history).map_err(StorageError::Sqlite)?;
            Ok(StorageConns { config, history })
        },
        StorageConns::close,
    )
    .await
    .unwrap();
    let vars = Variables::new(worker.handle.clone());
    (temp, worker, vars)
}

async fn raw<T: Send + 'static>(
    handle: &StorageHandle,
    job: impl FnOnce(&mut StorageConns) -> T + Send + 'static,
) -> T {
    match handle.submit(JobClass::MustLand, job).await.unwrap() {
        Admission::Accepted(reply) => reply.wait().await.unwrap(),
        Admission::Dropped => unreachable!(),
    }
}

fn record(value: Value, untrusted: bool, updated_by: UpdatedBy) -> VariableRecord {
    VariableRecord {
        value,
        persisted: true,
        untrusted,
        updated_at: Timestamp::parse("1969-12-31T23:59:59.123Z").unwrap(),
        updated_by,
    }
}

async fn raw_seed(worker: &StorageWorker, name: String, entry: VariableRecord) {
    raw(&worker.handle, move |c| {
        c.config
            .execute(
                "INSERT OR REPLACE INTO globals(name,value,untrusted,updated_at,updated_by)
             VALUES(?1,?2,?3,?4,?5)",
                params![
                    name,
                    serde_json::to_string(&entry.value).unwrap(),
                    entry.untrusted,
                    entry.updated_at.to_string(),
                    serde_json::to_string(&entry.updated_by).unwrap()
                ],
            )
            .unwrap();
    })
    .await;
}

#[tokio::test]
async fn globals_preserve_exact_names_json_flags_and_metadata_without_writes() {
    let (_temp, worker, vars) = harness().await;
    assert!(vars.read_globals().await.unwrap().is_empty());
    let writers = [
        UpdatedBy::User,
        UpdatedBy::Imported,
        UpdatedBy::Mcp,
        UpdatedBy::Plugin {
            plugin_id: "com.example.raw".try_into().unwrap(),
        },
        UpdatedBy::System,
    ];
    let values = [
        Value::Null,
        json!(false),
        json!(true),
        json!(9007199254740993_u64),
        json!({"nested":[null,true,"a\u{0}雪",{"x":1}]}),
        json!([i64::MIN, u64::MAX, 9007199254740993_u64, -0.5, 1e308]),
        json!("x".repeat(256 * 1024 - 2)),
    ];
    assert_eq!(serde_json::to_string(&values[6]).unwrap().len(), 256 * 1024);
    let mut expected = std::collections::BTreeMap::new();
    for (writer_index, writer) in writers.iter().enumerate() {
        for (value_index, value) in values.iter().enumerate() {
            for flag in [false, true] {
                let name = format!("Value_{writer_index}_{value_index}_{flag}");
                let entry = record(value.clone(), flag, writer.clone());
                seed(&worker, name.clone(), entry.clone()).await;
                expected.insert(name, entry);
            }
        }
    }
    for name in ["same", "SAME", "x' OR 1=1; --\u{0}雪"] {
        let entry = record(json!(name), true, UpdatedBy::Mcp);
        seed(&worker, name.into(), entry.clone()).await;
        expected.insert(name.into(), entry);
    }
    let before = raw(&worker.handle, |c| c.config.total_changes()).await;
    let actual = vars.read_globals().await.unwrap();
    assert_eq!(actual, expected);
    assert!(actual.values().all(|entry| entry.persisted));
    // Opaque JSON preserves integers beyond exact binary64 precision; no arithmetic.
    assert_eq!(
        actual["Value_0_5_true"].value[2].as_u64(),
        Some(9007199254740993)
    );
    assert_eq!(
        raw(&worker.handle, |c| c.config.total_changes()).await,
        before
    );
    worker.close().await.unwrap();
}

#[tokio::test]
async fn globals_reject_malformed_fields_and_duplicates_without_repair() {
    let (_temp, worker, vars) = harness().await;
    let entry = record(json!({"ok":true}), false, UpdatedBy::System);
    let corruptions = [
        ("value", "'{'", "json"),
        ("value", "x'FF'", "sqlite"),
        ("untrusted", "2", "stored"),
        ("untrusted", "'bad'", "sqlite"),
        ("updated_at", "'bad'", "stored"),
        ("updated_at", "'2026-10-20T10:00:00+00:00'", "stored"),
        ("updated_by", "'{'", "json"),
        ("updated_by", "'{\"kind\":\"unknown\"}'", "json"),
        ("updated_by", "'{\"kind\":\"user\",\"extra\":1}'", "stored"),
        (
            "updated_by",
            "'{\"kind\":\"plugin\",\"pluginId\":\"INVALID!\"}'",
            "json",
        ),
    ];
    for (field, expression, category) in corruptions {
        raw_seed(&worker, "bad".into(), entry.clone()).await;
        // Static fixture SQL; table/field/expression below never come from product input.
        raw(&worker.handle, move |c| {
            c.config
                .execute_batch(&format!(
                    "PRAGMA ignore_check_constraints=ON; UPDATE globals SET {field}={expression};"
                ))
                .unwrap()
        })
        .await;
        let before = raw(&worker.handle, |c| c.config.total_changes()).await;
        let error = vars.read_globals().await.unwrap_err();
        assert_eq!(error.code(), ErrorCode::new(ErrorCode::INTERNAL));
        assert!(
            match category {
                "json" => matches!(error, DocumentError::Json(_)),
                "stored" => matches!(error, DocumentError::InvalidStored(_)),
                _ => matches!(error, DocumentError::Sqlite(_)),
            },
            "globals.{field}: {error:?}"
        );
        assert_eq!(
            raw(&worker.handle, |c| c.config.total_changes()).await,
            before
        );
        raw(&worker.handle, |c| {
            c.config.execute_batch("DELETE FROM globals;").unwrap()
        })
        .await;
    }
    // Duplicate keys are impossible under the accepted primary keys. Simulate a damaged
    // schema using CTAS; the reader must fail rather than silently deduplicate.
    raw(&worker.handle, |c| {
        c.config
            .execute_batch(
                "CREATE TABLE g AS SELECT * FROM globals;
            DROP TABLE globals; ALTER TABLE g RENAME TO globals;",
            )
            .unwrap();
    })
    .await;
    raw_seed(&worker, "duplicate".into(), entry.clone()).await;
    raw_seed(&worker, "duplicate".into(), entry).await;
    let before = raw(&worker.handle, |c| c.config.total_changes()).await;
    assert!(matches!(
        vars.read_globals().await,
        Err(DocumentError::InvalidStored("duplicate variable name"))
    ));
    assert_eq!(
        raw(&worker.handle, |c| c.config.total_changes()).await,
        before
    );
    worker.close().await.unwrap();
}

#[tokio::test]
async fn globals_query_failure_and_closed_transport_remain_distinct() {
    let (_temp, worker, vars) = harness().await;
    raw(&worker.handle, |c| {
        c.config.execute_batch("DROP TABLE globals;").unwrap()
    })
    .await;
    assert!(matches!(
        vars.read_globals().await,
        Err(DocumentError::Sqlite(_))
    ));
    assert!(matches!(
        vars.apply_batch(vec![delete(Scope::Global, "missing")])
            .await,
        Err(DocumentError::Sqlite(_))
    ));
    worker.close().await.unwrap();
    assert!(matches!(
        vars.read_globals().await,
        Err(DocumentError::Transport(StorageError::Disconnected))
    ));
    assert!(matches!(
        vars.apply_batch(Vec::new()).await,
        Err(DocumentError::Transport(StorageError::Disconnected))
    ));
}

async fn raw_seed_user(
    worker: &StorageWorker,
    platform: &str,
    user_id: &str,
    name: String,
    entry: VariableRecord,
) {
    let (platform, user_id) = (platform.to_owned(), user_id.to_owned());
    raw(&worker.handle, move |c| {
        c.config.execute(
            "INSERT OR REPLACE INTO user_vars(platform,user_id,name,value,untrusted,updated_at,updated_by)
             VALUES(?1,?2,?3,?4,?5,?6,?7)",
            params![platform, user_id, name, serde_json::to_string(&entry.value).unwrap(),
                entry.untrusted, entry.updated_at.to_string(),
                serde_json::to_string(&entry.updated_by).unwrap()],
        ).unwrap();
    }).await;
}

#[tokio::test]
async fn users_preserve_scopes_exact_keys_json_flags_and_metadata_without_writes() {
    let (_temp, worker, vars) = harness().await;
    assert!(
        vars.read_user("absent".into(), "absent".into())
            .await
            .unwrap()
            .is_empty()
    );
    let writers = [
        UpdatedBy::User,
        UpdatedBy::Imported,
        UpdatedBy::Mcp,
        UpdatedBy::Plugin {
            plugin_id: "com.example.raw".try_into().unwrap(),
        },
        UpdatedBy::System,
    ];
    let values = [
        Value::Null,
        json!(false),
        json!(true),
        json!(9007199254740993_u64),
        json!({"nested":[null,true,"a\u{0}雪",{"x":1}]}),
        json!([i64::MIN, u64::MAX, 9007199254740993_u64, -0.5, 1e308]),
        json!("x".repeat(256 * 1024 - 2)),
    ];
    assert_eq!(serde_json::to_string(&values[6]).unwrap().len(), 256 * 1024);
    let global = record(json!("global"), false, UpdatedBy::System);
    seed(&worker, "same".into(), global.clone()).await;
    for platform in ["twitch", "youtube"] {
        for user_id in ["one", "two"] {
            let mut expected = std::collections::BTreeMap::new();
            for (writer_index, writer) in writers.iter().enumerate() {
                for (value_index, value) in values.iter().enumerate() {
                    for flag in [false, true] {
                        let name = format!("Value_{writer_index}_{value_index}_{flag}");
                        let entry = record(value.clone(), flag, writer.clone());
                        seed_user(&worker, platform, user_id, name.clone(), entry.clone()).await;
                        expected.insert(name, entry);
                    }
                }
            }
            for name in ["same", "SAME"] {
                let entry = record(json!([platform, user_id, name]), true, UpdatedBy::Mcp);
                seed_user(&worker, platform, user_id, name.into(), entry.clone()).await;
                expected.insert(name.into(), entry);
            }
            let before = raw(&worker.handle, |c| c.config.total_changes()).await;
            let actual = vars
                .read_user(platform.into(), user_id.into())
                .await
                .unwrap();
            assert_eq!(actual, expected);
            assert!(actual.values().all(|entry| entry.persisted));
            assert_eq!(
                actual["Value_0_5_true"].value[2].as_u64(),
                Some(9007199254740993)
            );
            assert_eq!(vars.read_globals().await.unwrap()["same"], global);
            assert_eq!(
                raw(&worker.handle, |c| c.config.total_changes()).await,
                before
            );
        }
    }
    let weird = "x' OR 1=1; --\u{0}雪";
    let entry = record(json!("bound"), true, UpdatedBy::Mcp);
    seed_user(&worker, weird, weird, weird.into(), entry.clone()).await;
    let before = raw(&worker.handle, |c| c.config.total_changes()).await;
    assert_eq!(
        vars.read_user(weird.into(), weird.into()).await.unwrap(),
        std::collections::BTreeMap::from([(weird.into(), entry)])
    );
    assert!(
        vars.read_user(weird.into(), "other".into())
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        vars.read_user("other".into(), weird.into())
            .await
            .unwrap()
            .is_empty()
    );
    // Previously seeded scopes remain distinct after all four have been populated.
    for platform in ["twitch", "youtube"] {
        for user_id in ["one", "two"] {
            assert_eq!(
                vars.read_user(platform.into(), user_id.into())
                    .await
                    .unwrap()["same"]
                    .value,
                json!([platform, user_id, "same"])
            );
        }
    }
    assert_eq!(
        raw(&worker.handle, |c| c.config.total_changes()).await,
        before
    );
    worker.close().await.unwrap();
}

#[tokio::test]
async fn users_reject_malformed_fields_and_duplicates_without_repair() {
    let (_temp, worker, vars) = harness().await;
    let entry = record(json!({"ok":true}), false, UpdatedBy::System);
    let corruptions = [
        ("value", "'{'", "json"),
        ("value", "x'FF'", "sqlite"),
        ("untrusted", "2", "stored"),
        ("untrusted", "'bad'", "sqlite"),
        ("updated_at", "'bad'", "stored"),
        ("updated_at", "'2026-10-20T10:00:00+00:00'", "stored"),
        ("updated_by", "'{'", "json"),
        ("updated_by", "'{\"kind\":\"unknown\"}'", "json"),
        ("updated_by", "'{\"kind\":\"user\",\"extra\":1}'", "stored"),
        (
            "updated_by",
            "'{\"kind\":\"plugin\",\"pluginId\":\"INVALID!\"}'",
            "json",
        ),
    ];
    for (field, expression, category) in corruptions {
        raw_seed_user(&worker, "p", "u", "bad".into(), entry.clone()).await;
        // Static fixture SQL; table/field/expression below never come from product input.
        raw(&worker.handle, move |c| {
            c.config
                .execute_batch(&format!(
                    "PRAGMA ignore_check_constraints=ON; UPDATE user_vars SET {field}={expression};"
                ))
                .unwrap()
        })
        .await;
        let before = raw(&worker.handle, |c| c.config.total_changes()).await;
        let error = vars.read_user("p".into(), "u".into()).await.unwrap_err();
        assert_eq!(error.code(), ErrorCode::new(ErrorCode::INTERNAL));
        assert!(
            match category {
                "json" => matches!(error, DocumentError::Json(_)),
                "stored" => matches!(error, DocumentError::InvalidStored(_)),
                _ => matches!(error, DocumentError::Sqlite(_)),
            },
            "user_vars.{field}: {error:?}"
        );
        assert_eq!(
            raw(&worker.handle, |c| c.config.total_changes()).await,
            before
        );
        raw(&worker.handle, |c| {
            c.config.execute_batch("DELETE FROM user_vars;").unwrap()
        })
        .await;
    }
    // Duplicate keys are impossible under the accepted primary keys. Simulate a damaged
    // schema using CTAS; the reader must fail rather than silently deduplicate.
    raw(&worker.handle, |c| {
        c.config
            .execute_batch(
                "CREATE TABLE g AS SELECT * FROM user_vars;
            DROP TABLE user_vars; ALTER TABLE g RENAME TO user_vars;",
            )
            .unwrap();
    })
    .await;
    raw_seed_user(&worker, "p", "u", "duplicate".into(), entry.clone()).await;
    raw_seed_user(&worker, "p", "u", "duplicate".into(), entry).await;
    let before = raw(&worker.handle, |c| c.config.total_changes()).await;
    assert!(matches!(
        vars.read_user("p".into(), "u".into()).await,
        Err(DocumentError::InvalidStored("duplicate variable name"))
    ));
    assert_eq!(
        raw(&worker.handle, |c| c.config.total_changes()).await,
        before
    );
    worker.close().await.unwrap();
}

#[tokio::test]
async fn users_query_failure_and_closed_transport_remain_distinct() {
    let (_temp, worker, vars) = harness().await;
    raw(&worker.handle, |c| {
        c.config.execute_batch("DROP TABLE user_vars;").unwrap()
    })
    .await;
    assert!(matches!(
        vars.read_user("p".into(), "u".into()).await,
        Err(DocumentError::Sqlite(_))
    ));
    worker.close().await.unwrap();
    assert!(matches!(
        vars.read_user("p".into(), "u".into()).await,
        Err(DocumentError::Transport(StorageError::Disconnected))
    ));
}

fn user(platform: &str, user_id: &str) -> Scope {
    Scope::User {
        platform: platform.into(),
        user_id: user_id.into(),
    }
}

fn put(scope: Scope, name: &str, record: VariableRecord) -> Mutation {
    Mutation::Upsert {
        scope,
        name: name.into(),
        record,
    }
}

fn delete(scope: Scope, name: &str) -> Mutation {
    Mutation::Delete {
        scope,
        name: name.into(),
    }
}

async fn seed(worker: &StorageWorker, name: String, entry: VariableRecord) {
    Variables::new(worker.handle.clone())
        .apply_batch(vec![put(Scope::Global, &name, entry)])
        .await
        .unwrap();
}

async fn seed_user(
    worker: &StorageWorker,
    platform: &str,
    user_id: &str,
    name: String,
    entry: VariableRecord,
) {
    Variables::new(worker.handle.clone())
        .apply_batch(vec![put(user(platform, user_id), &name, entry)])
        .await
        .unwrap();
}

#[tokio::test]
async fn raw_ordered_batch_repeated_keys_deletes_and_empty_batch() {
    let (_temp, worker, vars) = harness().await;
    let scope = user("twitch", "one");
    vars.apply_batch(Vec::new()).await.unwrap();
    let first = record(json!("first"), true, UpdatedBy::Mcp);
    let mut last = record(json!([null, 42]), false, UpdatedBy::System);
    last.updated_at = Timestamp::parse("2026-10-20T12:34:56.789Z").unwrap();
    vars.apply_batch(vec![
        put(Scope::Global, "key", first.clone()),
        put(scope.clone(), "key", first.clone()),
        delete(Scope::Global, "key"),
        put(Scope::Global, "key", last.clone()),
        put(scope.clone(), "key", last.clone()),
        delete(scope.clone(), "key"),
        delete(scope.clone(), "missing"),
    ])
    .await
    .unwrap();
    assert_eq!(vars.read_globals().await.unwrap()["key"], last);
    assert!(
        vars.read_user("twitch".into(), "one".into())
            .await
            .unwrap()
            .is_empty()
    );
    vars.apply_batch(vec![
        delete(Scope::Global, "key"),
        delete(Scope::Global, "key"),
    ])
    .await
    .unwrap();
    assert!(vars.read_globals().await.unwrap().is_empty());
    worker.close().await.unwrap();
}

#[tokio::test]
async fn raw_batch_rolls_back_all_scopes_on_late_temporary_input() {
    let (_temp, worker, vars) = harness().await;
    let initial = record(json!("original"), false, UpdatedBy::User);
    let mut temporary = record(json!("temp"), true, UpdatedBy::Imported);
    temporary.persisted = false;
    vars.apply_batch(vec![put(Scope::Global, "keep", initial.clone())])
        .await
        .unwrap();
    let error = vars
        .apply_batch(vec![
            delete(Scope::Global, "keep"),
            put(user("p", "one"), "new", initial.clone()),
            put(user("p", "two"), "bad", temporary.clone()),
        ])
        .await
        .unwrap_err();
    assert!(matches!(error, DocumentError::InvalidInput(_)));
    assert_eq!(error.code(), ErrorCode::new(ErrorCode::INVALID_PARAMS));
    assert_eq!(vars.read_globals().await.unwrap()["keep"], initial);
    assert!(
        vars.read_user("p".into(), "one".into())
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        vars.read_user("p".into(), "two".into())
            .await
            .unwrap()
            .is_empty()
    );
    assert!(matches!(
        vars.apply_batch(vec![put(Scope::Global, "keep", temporary)])
            .await,
        Err(DocumentError::InvalidInput(_))
    ));
    assert_eq!(vars.read_globals().await.unwrap()["keep"], initial);
    worker.close().await.unwrap();
}

#[tokio::test]
async fn raw_batch_rolls_back_on_late_sql_trigger_and_keeps_worker_healthy() {
    let (_temp, worker, vars) = harness().await;
    raw(&worker.handle, |c| {
        c.config
            .execute_batch(
                "CREATE TRIGGER reject_bad BEFORE INSERT ON user_vars WHEN NEW.user_id='bad'
         BEGIN SELECT RAISE(ABORT,'fixture rejection'); END;",
            )
            .unwrap()
    })
    .await;
    let entry = record(json!(true), true, UpdatedBy::User);
    let error = vars
        .apply_batch(vec![
            put(Scope::Global, "new", entry.clone()),
            put(user("p", "good"), "new", entry.clone()),
            put(user("p", "bad"), "new", entry.clone()),
        ])
        .await
        .unwrap_err();
    assert!(matches!(error, DocumentError::Sqlite(_)));
    assert!(vars.read_globals().await.unwrap().is_empty());
    for id in ["good", "bad"] {
        assert!(
            vars.read_user("p".into(), id.into())
                .await
                .unwrap()
                .is_empty()
        );
    }
    vars.apply_batch(vec![put(Scope::Global, "healthy", entry)])
        .await
        .unwrap();
    worker.close().await.unwrap();
}

#[tokio::test]
async fn raw_bound_keys_cannot_change_sql_or_normalize_names() {
    let (_temp, worker, vars) = harness().await;
    let weird = "x' OR 1=1; DROP TABLE globals; --\u{0}雪";
    let scope = user(weird, weird);
    let entry = record(json!(null), true, UpdatedBy::System);
    vars.apply_batch(vec![
        put(Scope::Global, weird, entry.clone()),
        put(scope.clone(), weird, entry.clone()),
        put(Scope::Global, "keep", entry.clone()),
        put(user("p", "other"), weird, entry.clone()),
    ])
    .await
    .unwrap();
    assert_eq!(vars.read_globals().await.unwrap()[weird], entry);
    assert_eq!(
        vars.read_user(weird.into(), weird.into()).await.unwrap()[weird],
        entry
    );
    vars.apply_batch(vec![delete(Scope::Global, weird), delete(scope, weird)])
        .await
        .unwrap();
    assert_eq!(vars.read_globals().await.unwrap().len(), 1);
    assert!(
        vars.read_user(weird.into(), weird.into())
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        vars.read_user("p".into(), "other".into()).await.unwrap()[weird],
        entry
    );
    worker.close().await.unwrap();
}

#[tokio::test]
async fn raw_unreadable_deep_json_rejects_without_database_side_effects() {
    let (_temp, worker, vars) = harness().await;
    let initial = record(json!("original"), false, UpdatedBy::User);
    vars.apply_batch(vec![put(Scope::Global, "keep", initial.clone())])
        .await
        .unwrap();
    let before = raw(&worker.handle, |c| c.config.total_changes()).await;
    // A representable negative UTC year cannot roundtrip through this reader's RFC3339.
    let mut unreadable = initial.clone();
    unreadable.updated_at = Timestamp::parse("0000-01-01T00:00:00+23:59").unwrap();
    assert!(matches!(
        vars.apply_batch(vec![put(Scope::Global, "keep", unreadable)])
            .await,
        Err(DocumentError::InvalidInput(_))
    ));
    let mut deep = Value::Null;
    for _ in 0..140 {
        deep = Value::Array(vec![deep]);
    }
    assert!(serde_json::from_str::<Value>(&deep.to_string()).is_err());
    let error = vars
        .apply_batch(vec![put(
            user("p", "u"),
            "deep",
            record(deep.clone(), true, UpdatedBy::Mcp),
        )])
        .await
        .unwrap_err();
    assert!(matches!(error, DocumentError::InvalidInput(_)));
    assert_eq!(error.code(), ErrorCode::new(ErrorCode::INVALID_PARAMS));
    assert_eq!(
        raw(&worker.handle, |c| c.config.total_changes()).await,
        before
    );
    assert!(matches!(
        vars.apply_batch(vec![
            delete(Scope::Global, "keep"),
            put(
                user("p", "u"),
                "deep",
                record(deep, false, UpdatedBy::System)
            )
        ])
        .await,
        Err(DocumentError::InvalidInput(_))
    ));
    assert_eq!(vars.read_globals().await.unwrap()["keep"], initial);
    assert!(
        vars.read_user("p".into(), "u".into())
            .await
            .unwrap()
            .is_empty()
    );
    worker.close().await.unwrap();
}

#[tokio::test]
async fn raw_empty_batch_attempts_a_config_transaction_and_recovers() {
    let (_temp, worker, vars) = harness().await;
    raw(&worker.handle, |c| {
        c.config.execute_batch("BEGIN").unwrap();
    })
    .await;
    // An early return on empty input would incorrectly succeed inside this transaction.
    assert!(matches!(
        vars.apply_batch(Vec::new()).await,
        Err(DocumentError::Sqlite(_))
    ));
    raw(&worker.handle, |c| {
        assert!(!c.config.is_autocommit());
        c.config.execute_batch("ROLLBACK").unwrap();
    })
    .await;
    vars.apply_batch(Vec::new()).await.unwrap();
    assert!(vars.read_globals().await.unwrap().is_empty());
    worker.close().await.unwrap();
}

#[tokio::test]
async fn raw_upserts_replace_full_records_across_and_within_batches_in_both_scopes() {
    for target in [Scope::Global, user("p", "one")] {
        let (_temp, worker, vars) = harness().await;
        let initial = record(json!("initial"), false, UpdatedBy::System);
        let mut plugin = record(
            json!({"plugin": "untrusted"}),
            true,
            UpdatedBy::Plugin {
                plugin_id: "com.example.overwrite".try_into().unwrap(),
            },
        );
        plugin.updated_at = Timestamp::parse("2026-10-20T12:00:00.001Z").unwrap();
        let mut trusted = record(json!(["trusted", 2]), false, UpdatedBy::User);
        trusted.updated_at = Timestamp::parse("2026-10-20T12:00:00.002Z").unwrap();
        let mut final_record = trusted.clone();
        final_record.value = json!({"final": true});
        final_record.updated_at = Timestamp::parse("2026-10-20T12:00:00.004Z").unwrap();
        let mut intermediate = plugin.clone();
        intermediate.value = json!(["intermediate", 3]);
        intermediate.updated_at = Timestamp::parse("2026-10-20T12:00:00.003Z").unwrap();
        // Same name everywhere: an overwrite must leave every other scope unchanged.
        vars.apply_batch(vec![
            put(Scope::Global, "key", initial.clone()),
            put(user("p", "one"), "key", initial.clone()),
            put(user("p", "two"), "key", initial.clone()),
            put(user("q", "one"), "key", initial.clone()),
        ])
        .await
        .unwrap();
        for (batch, expected) in [
            (vec![plugin.clone()], plugin),
            (vec![trusted.clone()], trusted),
            // Neither repeated put is hidden by a delete. The last record must win.
            (vec![intermediate, final_record.clone()], final_record),
        ] {
            vars.apply_batch(
                batch
                    .into_iter()
                    .map(|entry| put(target.clone(), "key", entry))
                    .collect(),
            )
            .await
            .unwrap();
            let expected_global = match target {
                Scope::Global => &expected,
                Scope::User { .. } => &initial,
            };
            let expected_user = match target {
                Scope::Global => &initial,
                Scope::User { .. } => &expected,
            };
            assert_eq!(
                vars.read_globals().await.unwrap(),
                [("key".into(), expected_global.clone())].into()
            );
            for (platform, id, entry) in [
                ("p", "one", expected_user),
                ("p", "two", &initial),
                ("q", "one", &initial),
            ] {
                assert_eq!(
                    vars.read_user(platform.into(), id.into()).await.unwrap(),
                    [("key".into(), entry.clone())].into()
                );
            }
            // Independent raw SQLite readback: the typed reader cannot hide stale columns.
            let scope = target.clone();
            let stored = raw(&worker.handle, move |c| {
                let read = |row: &rusqlite::Row<'_>| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, bool>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                };
                match scope {
                    Scope::Global => c.config.query_row(
                        "SELECT value,untrusted,updated_at,updated_by FROM globals WHERE name=?1",
                        ["key"],
                        read,
                    ),
                    Scope::User { platform, user_id } => c.config.query_row(
                        "SELECT value,untrusted,updated_at,updated_by FROM user_vars
                         WHERE platform=?1 AND user_id=?2 AND name=?3",
                        params![platform, user_id, "key"],
                        read,
                    ),
                }
                .unwrap()
            })
            .await;
            assert_eq!(
                stored,
                (
                    serde_json::to_string(&expected.value).unwrap(),
                    expected.untrusted,
                    expected.updated_at.to_string(),
                    serde_json::to_string(&expected.updated_by).unwrap(),
                )
            );
        }
        worker.close().await.unwrap();
    }
}
