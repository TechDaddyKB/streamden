use den_core::storage::{
    documents::DocumentError,
    schema::{bootstrap_config, bootstrap_history},
    thread::{Admission, JobClass, StorageConns, StorageError, StorageHandle, StorageWorker},
    variables::Variables,
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

async fn seed(worker: &StorageWorker, name: String, entry: VariableRecord) {
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
        seed(&worker, "bad".into(), entry.clone()).await;
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
    seed(&worker, "duplicate".into(), entry.clone()).await;
    seed(&worker, "duplicate".into(), entry).await;
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
    worker.close().await.unwrap();
    assert!(matches!(
        vars.read_globals().await,
        Err(DocumentError::Transport(StorageError::Disconnected))
    ));
}

async fn seed_user(
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
        seed_user(&worker, "p", "u", "bad".into(), entry.clone()).await;
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
    seed_user(&worker, "p", "u", "duplicate".into(), entry.clone()).await;
    seed_user(&worker, "p", "u", "duplicate".into(), entry).await;
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

#[tokio::test]
async fn users_match_platform_and_user_case_exactly_without_writes() {
    let (_temp, worker, vars) = harness().await;
    let scopes = [("p", "u"), ("P", "u"), ("p", "U"), ("P", "U")];
    for (platform, user_id) in scopes {
        let entry = record(json!([platform, user_id]), false, UpdatedBy::User);
        seed_user(&worker, platform, user_id, "same".into(), entry).await;
    }
    let before = raw(&worker.handle, |c| c.config.total_changes()).await;
    for (platform, user_id) in scopes {
        let actual = vars
            .read_user(platform.into(), user_id.into())
            .await
            .unwrap();
        let expected = std::collections::BTreeMap::from([(
            "same".into(),
            record(json!([platform, user_id]), false, UpdatedBy::User),
        )]);
        assert!(actual == expected, "scope record map must match exactly");
    }
    assert_eq!(
        raw(&worker.handle, |c| c.config.total_changes()).await,
        before
    );
    worker.close().await.unwrap();
}

#[tokio::test]
async fn users_isolate_unrelated_corruption_and_fail_matching_scope_without_repair() {
    let (_temp, worker, vars) = harness().await;
    let entry = record(json!({"valid": true}), true, UpdatedBy::Mcp);
    seed_user(&worker, "p", "u", "good".into(), entry.clone()).await;
    let corrupt_scopes = [("other", "u"), ("p", "other"), ("P", "u"), ("p", "U")];
    for (platform, user_id) in corrupt_scopes {
        // Each damaged scope also has a valid row: its read must fail as a whole.
        seed_user(&worker, platform, user_id, "good".into(), entry.clone()).await;
        seed_user(&worker, platform, user_id, "bad".into(), entry.clone()).await;
    }
    raw(&worker.handle, |c| {
        c.config
            .execute_batch(
                "PRAGMA ignore_check_constraints=ON;
             UPDATE user_vars SET untrusted=2 WHERE name='bad';",
            )
            .unwrap();
    })
    .await;
    let before = raw(&worker.handle, |c| c.config.total_changes()).await;
    assert_eq!(
        vars.read_user("p".into(), "u".into()).await.unwrap(),
        std::collections::BTreeMap::from([("good".into(), entry.clone())])
    );
    for (platform, user_id) in corrupt_scopes {
        let error = vars
            .read_user(platform.into(), user_id.into())
            .await
            .unwrap_err();
        assert_eq!(error.code(), ErrorCode::new(ErrorCode::INTERNAL));
        assert!(matches!(
            error,
            DocumentError::InvalidStored("invalid variable flag")
        ));
    }
    // Re-read after all failures to catch repairs or cross-scope side effects.
    assert_eq!(
        vars.read_user("p".into(), "u".into()).await.unwrap(),
        std::collections::BTreeMap::from([("good".into(), entry)])
    );
    assert_eq!(
        raw(&worker.handle, |c| {
            c.config
                .query_row(
                    "SELECT COUNT(*) FROM user_vars WHERE name='bad' AND untrusted=2",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap()
        })
        .await,
        4
    );
    assert_eq!(
        raw(&worker.handle, |c| c.config.total_changes()).await,
        before
    );
    worker.close().await.unwrap();
}

#[tokio::test]
async fn users_preserve_embedded_nul_in_each_scope_key_without_writes() {
    let (_temp, worker, vars) = harness().await;
    let scopes = [
        ("x", "x"),
        ("x\u{0}y", "x"),
        ("x", "x\u{0}y"),
        ("x\u{0}y", "x\u{0}y"),
    ];
    for (platform, user_id) in scopes {
        let entry = record(json!([platform, user_id]), false, UpdatedBy::System);
        seed_user(&worker, platform, user_id, "same".into(), entry).await;
    }
    let before = raw(&worker.handle, |c| c.config.total_changes()).await;
    for (platform, user_id) in scopes {
        assert_eq!(
            vars.read_user(platform.into(), user_id.into())
                .await
                .unwrap(),
            std::collections::BTreeMap::from([(
                "same".into(),
                record(json!([platform, user_id]), false, UpdatedBy::System),
            )])
        );
    }
    for (platform, user_id) in [("x\u{0}", "x"), ("x", "x\u{0}"), ("x\u{0}z", "x\u{0}y")] {
        assert!(
            vars.read_user(platform.into(), user_id.into())
                .await
                .unwrap()
                .is_empty()
        );
    }
    assert_eq!(
        raw(&worker.handle, |c| c.config.total_changes()).await,
        before
    );
    worker.close().await.unwrap();
}
