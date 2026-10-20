use den_core::storage::{
    documents::{DocumentError, Documents},
    schema::{bootstrap_config, bootstrap_history},
    thread::{Admission, JobClass, StorageConns, StorageError, StorageHandle, StorageWorker},
};
use den_protocol::{EntityId, Timestamp, UpdatedBy, rpc::ErrorCode};
use rusqlite::{Connection, params};
use serde_json::json;

async fn harness() -> (tempfile::TempDir, StorageWorker, Documents) {
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
    let docs = Documents::new(worker.handle.clone());
    (temp, worker, docs)
}

async fn raw(handle: &StorageHandle, job: impl FnOnce(&mut StorageConns) + Send + 'static) {
    match handle.submit(JobClass::MustLand, job).await.unwrap() {
        Admission::Accepted(reply) => reply.wait().await.unwrap(),
        Admission::Dropped => unreachable!(),
    }
}

#[tokio::test]
async fn missing_and_persisted_get_preserve_metadata_body_id_and_attribution() {
    let (_temp, worker, docs) = harness().await;
    let id = EntityId::new_v7();
    let created = Timestamp::parse("2026-10-19T10:00:00Z").unwrap();
    let updated = Timestamp::parse("2026-10-20T11:00:00Z").unwrap();
    let data = json!({"id":id,"name":"Straße","enabled":true,"extra":{"nested":[1,null]}});
    let kinds = [
        "action",
        "queue",
        "command",
        "timer",
        "settings",
        "view_state",
        "pending_change",
        "export_meta",
    ];
    let writers = [
        UpdatedBy::User,
        UpdatedBy::Imported,
        UpdatedBy::Mcp,
        UpdatedBy::Plugin {
            plugin_id: "com.example.fixture".try_into().unwrap(),
        },
        UpdatedBy::System,
    ];
    for (index, kind) in kinds.into_iter().enumerate() {
        assert!(docs.get(kind.into(), id).await.unwrap().is_none());
        let by = writers[index % writers.len()].clone();
        let (body, attribution) = (data.clone(), by.clone());
        raw(&worker.handle, move |c| {
            c.config.execute(
                "INSERT INTO documents(kind,id,schema_version,revision,data,created_at,updated_at,updated_by)
                 VALUES(?1,?2,99,9223372036854775807,?3,?4,?5,?6)",
                params![kind, id.to_string(), body.to_string(), created.to_string(),
                    updated.to_string(), serde_json::to_string(&attribution).unwrap()],
            ).unwrap();
        }).await;
        let record = docs.get(kind.into(), id).await.unwrap().unwrap();
        assert_eq!(record.id, id);
        assert_eq!(record.document.kind, kind);
        assert_eq!(record.document.schema_version, 99);
        assert_eq!(record.document.data, data);
        assert_eq!(record.revision, i64::MAX as u64);
        assert_eq!(record.created_at, created);
        assert_eq!(record.updated_at, updated);
        assert_eq!(record.updated_by, by);
        assert!(
            docs.get(kind.into(), EntityId::new_v7())
                .await
                .unwrap()
                .is_none()
        );
    }
    // A getter has no migration/validation registry: unknown versions and fields stay intact.
    worker.close().await.unwrap();
}

#[tokio::test]
async fn malformed_persisted_fields_and_raw_errors_are_distinct() {
    let (_temp, worker, docs) = harness().await;
    let id = EntityId::new_v7();
    raw(&worker.handle, move |c| {
        c.config.execute(
            "INSERT INTO documents(kind,id,schema_version,revision,data,created_at,updated_at,updated_by)
             VALUES('timer',?1,1,1,'{}','2026-10-20T10:00:00Z','2026-10-20T10:00:00Z','{\"kind\":\"system\"}')",
            [id.to_string()],
        ).unwrap();
    }).await;
    assert_eq!(
        docs.get("timer".into(), id)
            .await
            .unwrap()
            .unwrap()
            .revision,
        1
    );
    for revision in [0, -1, i64::MIN] {
        raw(&worker.handle, move |c| {
            c.config
                .execute("UPDATE documents SET revision=?1", [revision])
                .unwrap();
        })
        .await;
        let error = docs.get("timer".into(), id).await.unwrap_err();
        assert_eq!(error.code(), ErrorCode::new(ErrorCode::INTERNAL));
        assert!(matches!(
            error,
            DocumentError::InvalidStored("invalid stored revision")
        ));
    }
    for column in ["created_at", "updated_at"] {
        raw(&worker.handle, move |c| {
            c.config
                .execute_batch(&format!("UPDATE documents SET revision=1,{column}='bad'"))
                .unwrap();
        })
        .await;
        assert!(matches!(
            docs.get("timer".into(), id).await,
            Err(DocumentError::InvalidStored("invalid stored timestamp"))
        ));
        raw(&worker.handle, move |c| {
            c.config
                .execute_batch(&format!(
                    "UPDATE documents SET {column}='2026-10-20T10:00:00Z'"
                ))
                .unwrap();
        })
        .await;
    }
    raw(&worker.handle, |c| {
        c.config
            .execute_batch("UPDATE documents SET updated_by='{\"kind\":\"unknown\"}'")
            .unwrap();
    })
    .await;
    let error = docs.get("timer".into(), id).await.unwrap_err();
    assert_eq!(error.code(), ErrorCode::new(ErrorCode::INTERNAL));
    assert!(matches!(error, DocumentError::Json(e) if e.is_data() && e.line() == 1));
    // Corruption fixture only: normal schema writes enforce json_valid(data).
    raw(&worker.handle, |c| {
        c.config
            .execute_batch(
                "PRAGMA ignore_check_constraints=ON;
             UPDATE documents SET data='broken';
             PRAGMA ignore_check_constraints=OFF;",
            )
            .unwrap();
    })
    .await;
    let error = docs.get("timer".into(), id).await.unwrap_err();
    assert_eq!(error.code(), ErrorCode::new(ErrorCode::INTERNAL));
    assert!(matches!(error, DocumentError::Json(e) if e.is_syntax() && e.column() > 0));
    raw(&worker.handle, |c| {
        c.config.execute_batch("DROP TABLE documents").unwrap()
    })
    .await;
    let error = docs.get("timer".into(), id).await.unwrap_err();
    assert_eq!(error.code(), ErrorCode::new(ErrorCode::INTERNAL));
    assert!(
        matches!(error, DocumentError::Sqlite(rusqlite::Error::SqliteFailure(_, Some(message)))
        if message.contains("no such table: documents"))
    );
    worker.close().await.unwrap();
    let error = docs.get("timer".into(), id).await.unwrap_err();
    assert_eq!(error.code(), ErrorCode::new(ErrorCode::INTERNAL));
    assert!(matches!(
        error,
        DocumentError::Transport(StorageError::Disconnected)
    ));
    let error = docs.get("export".into(), id).await.unwrap_err();
    assert_eq!(error.code(), ErrorCode::new(ErrorCode::INVALID_PARAMS));
    assert!(matches!(error, DocumentError::InvalidInput(_)));
}

use den_core::{
    clock::Clock,
    storage::documents::{DocumentWriter, Put, WritePolicy},
};
use den_protocol::StoredDocument;
use serde_json::Value;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
struct TestClock(Arc<Mutex<Timestamp>>);
impl Clock for TestClock {
    fn monotonic(&self) -> tokio::time::Instant {
        tokio::time::Instant::now()
    }
    fn wall(&self) -> Timestamp {
        *self.0.lock().unwrap()
    }
}
fn clock() -> TestClock {
    TestClock(Arc::new(Mutex::new(
        Timestamp::parse("2026-10-20T10:00:00Z").unwrap(),
    )))
}
fn policy() -> WritePolicy {
    // Arbitrary fixture versions for feature-owned kinds; no production defaults are declared.
    let versions = [
        ("action", StoredDocument::ACTION_SCHEMA_VERSION),
        ("queue", StoredDocument::QUEUE_SCHEMA_VERSION),
        ("command", StoredDocument::COMMAND_SCHEMA_VERSION),
        ("timer", StoredDocument::TIMER_SCHEMA_VERSION),
        (
            "pending_change",
            StoredDocument::PENDING_CHANGE_SCHEMA_VERSION,
        ),
        ("settings", 3),
        ("view_state", 3),
        ("export_meta", 3),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), v))
    .collect();
    WritePolicy::new(versions).unwrap()
}
fn writer(docs: &Documents) -> (DocumentWriter, TestClock) {
    let clock = clock();
    (docs.writer(policy(), Arc::new(clock.clone())), clock)
}
fn put(kind: &str, id: EntityId, data: Value) -> Put {
    Put {
        id,
        document: StoredDocument {
            kind: kind.into(),
            schema_version: if matches!(kind, "settings" | "view_state" | "export_meta") {
                3
            } else {
                1
            },
            data,
        },
        expected_revision: None,
        updated_by: UpdatedBy::System,
    }
}
fn action(id: EntityId, name: &str) -> Put {
    put(
        "action",
        id,
        json!({"id":id,"name":name,"enabled":true,"origin":{"kind":"user"},"extra":[1,null]}),
    )
}
fn conflict(error: DocumentError, id: EntityId) {
    assert_eq!(error.code(), ErrorCode::new(ErrorCode::CONFLICT));
    assert!(matches!(error,DocumentError::Conflict{id:clash} if clash==id));
}

#[test]
fn pure_preparation_guards_versions_identity_revisions_names_and_drafts() {
    let policy = policy();
    let now = clock().wall();
    let id = EntityId::new_v7();
    let first = policy.prepare(action(id, "Straße"), None, now).unwrap();
    assert_eq!(first.key.as_deref(), Some("strasse"));
    assert_eq!(first.name.as_deref(), Some("Straße"));
    assert_eq!(first.enabled, Some(true));
    assert_eq!(first.record.revision, 1);
    assert!(!first.draft);
    let mut change = action(id, "STRASSE");
    change.expected_revision = Some(1);
    let later = Timestamp::parse("2026-10-20T11:00:00Z").unwrap();
    let second = policy.prepare(change, Some(&first.record), later).unwrap();
    assert_eq!(second.record.created_at, now);
    assert_eq!(second.record.updated_at, later);
    assert_eq!(second.record.revision, 2);
    assert_eq!(second.record.document.data["revision"], 2);
    for origin in [
        None,
        Some(json!({})),
        Some(json!({"kind":null})),
        Some(json!({"kind":"future"})),
    ] {
        let mut input = action(id, "absent");
        input
            .document
            .data
            .as_object_mut()
            .unwrap()
            .remove("origin");
        if let Some(origin) = origin {
            input.document.data["origin"] = origin;
        }
        assert!(!policy.prepare(input, None, now).unwrap().draft);
    }
    let draft = policy
        .prepare(
            put(
                "action",
                id,
                json!({"name":"draft","enabled":true,"origin":{"kind":"ai_draft"}}),
            ),
            None,
            now,
        )
        .unwrap();
    assert!(draft.draft);
    assert_eq!(draft.enabled, Some(false));
    assert_eq!(draft.record.document.data["enabled"], false);
    let mut old = first.record;
    for version in [0, 99] {
        old.document.schema_version = version;
        assert!(
            matches!(policy.prepare(action(id,"blocked"),Some(&old),now), Err(DocumentError::UnsupportedVersion { stored,.. }) if stored==version)
        );
    }
    old.document.schema_version = StoredDocument::ACTION_SCHEMA_VERSION;
    old.revision = i64::MAX as u64;
    assert!(matches!(
        policy.prepare(action(id, "overflow"), Some(&old), now),
        Err(DocumentError::InvalidStored("revision overflow"))
    ));
    old.revision = 1;
    assert!(matches!(
        policy.prepare(action(EntityId::new_v7(), "identity"), Some(&old), now),
        Err(DocumentError::InvalidInput(
            "previous document identity mismatch"
        ))
    ));
    let mut stale = action(id, "stale");
    stale.expected_revision = Some(2);
    conflict(policy.prepare(stale, Some(&old), now).unwrap_err(), id);
    for input in [
        put("action", id, json!({})),
        put("timer", id, json!([])),
        put("timer", id, json!({"enabled":"yes"})),
    ] {
        assert!(matches!(
            policy.prepare(input, None, now),
            Err(DocumentError::InvalidInput(_))
        ));
    }
}

#[test]
fn pure_registered_versions_and_full_unicode_casefold() {
    for (kind, version) in [
        ("action", 2),
        ("queue", 2),
        ("command", 2),
        ("timer", 2),
        ("pending_change", 2),
        ("settings", 0),
        ("export", 1),
    ] {
        assert!(matches!(
            WritePolicy::new([(kind.into(), version)].into()),
            Err(DocumentError::InvalidInput(_))
        ));
    }
    let id = EntityId::new_v7();
    let now = clock().wall();
    let empty = WritePolicy::new(Default::default()).unwrap();
    assert!(matches!(
        empty.prepare(action(id, "unregistered"), None, now),
        Err(DocumentError::InvalidInput(
            "document version not registered"
        ))
    ));
    let policy = policy();
    let mut future = action(id, "future");
    future.document.schema_version = 99;
    assert!(matches!(
        policy.prepare(future, None, now),
        Err(DocumentError::InvalidInput(
            "input version must match registered version"
        ))
    ));
    for (name, expected) in [
        ("Straße", "strasse"),
        ("Σ", "σ"),
        ("ς", "σ"),
        ("ﬃ", "ffi"),
        ("İ", "i\u{307}"),
    ] {
        let prepared = policy.prepare(action(id, name), None, now).unwrap();
        assert_eq!(prepared.key.as_deref(), Some(expected));
    }
}

#[test]
fn caller_body_ids_are_input_errors() {
    let policy = policy();
    let now = clock().wall();
    let id = EntityId::new_v7();
    for body_id in [json!("not-a-uuid"), json!(null), json!(42)] {
        let mut input = action(id, "body id");
        input.document.data["id"] = body_id;
        let error = policy.prepare(input, None, now).unwrap_err();
        assert_eq!(error.code(), ErrorCode::new(ErrorCode::INVALID_PARAMS));
        assert!(matches!(
            error,
            DocumentError::InvalidInput("document id must be an entity id")
        ));
    }
    let mut input = action(id, "different id");
    input.document.data["id"] = json!(EntityId::new_v7());
    let error = policy.prepare(input, None, now).unwrap_err();
    assert_eq!(error.code(), ErrorCode::new(ErrorCode::INVALID_PARAMS));
    assert!(matches!(
        error,
        DocumentError::InvalidInput("document id mismatch")
    ));
}

#[test]
fn expected_revision_on_absent_row_conflicts_with_input_id() {
    let id = EntityId::new_v7();
    let mut input = action(id, "absent row");
    input.expected_revision = Some(1);
    conflict(
        policy().prepare(input, None, clock().wall()).unwrap_err(),
        id,
    );
}

#[test]
fn enabled_is_validated_before_draft_safety_override() {
    let policy = policy();
    let now = clock().wall();
    let id = EntityId::new_v7();
    for origin in ["user", "ai_draft"] {
        for enabled in [json!("yes"), json!(null), json!(42), json!({}), json!([])] {
            let input = put(
                "action",
                id,
                json!({"name":"enabled type","enabled":enabled,"origin":{"kind":origin}}),
            );
            let error = policy.prepare(input, None, now).unwrap_err();
            assert_eq!(error.code(), ErrorCode::new(ErrorCode::INVALID_PARAMS));
            assert!(matches!(
                error,
                DocumentError::InvalidInput("enabled must be a boolean")
            ));
        }
        for enabled in [None, Some(false), Some(true)] {
            let mut input = put(
                "action",
                id,
                json!({"name":"optional enabled","origin":{"kind":origin}}),
            );
            if let Some(enabled) = enabled {
                input.document.data["enabled"] = json!(enabled);
            }
            let prepared = policy.prepare(input, None, now).unwrap();
            let expected = if origin == "ai_draft" {
                Some(false)
            } else {
                enabled
            };
            assert_eq!(prepared.enabled, expected);
            assert_eq!(prepared.draft, origin == "ai_draft");
            assert_eq!(
                prepared.record.document.data.get("enabled"),
                expected.map(|v| json!(v)).as_ref()
            );
        }
    }
}

#[tokio::test]
async fn put_bumps_revision_atomically() {
    let (_temp, worker, docs) = harness().await;
    let (writer, clock) = writer(&docs);
    let id = EntityId::new_v7();
    let first = writer.put(action(id, "before")).await.unwrap();
    assert_eq!(
        (first.revision, first.document.data["revision"].as_u64()),
        (1, Some(1))
    );
    assert_eq!(first.created_at, clock.wall());
    *clock.0.lock().unwrap() = Timestamp::parse("2026-10-20T11:00:00Z").unwrap();
    let mut change = action(id, "after");
    change.expected_revision = Some(1);
    change.updated_by = UpdatedBy::Mcp;
    let second = writer.put(change).await.unwrap();
    assert_eq!(
        (second.revision, second.document.data["revision"].as_u64()),
        (2, Some(2))
    );
    assert_eq!(second.created_at, first.created_at);
    assert_eq!(second.updated_at, clock.wall());
    assert_eq!(second.updated_by, UpdatedBy::Mcp);
    assert_eq!(second.document.data["extra"], json!([1, null]));
    raw(&worker.handle, |c| {
        c.config
            .execute_batch(
                "CREATE TRIGGER fail_write AFTER UPDATE ON documents
             BEGIN SELECT RAISE(ABORT,'injected'); END",
            )
            .unwrap();
    })
    .await;
    *clock.0.lock().unwrap() = Timestamp::parse("2026-10-20T12:00:00Z").unwrap();
    let error = writer.put(action(id, "failed")).await.unwrap_err();
    assert_eq!(error.code(), ErrorCode::new(ErrorCode::INTERNAL));
    assert!(matches!(error, DocumentError::Sqlite(_)));
    let after = docs.get("action".into(), id).await.unwrap().unwrap();
    assert_eq!(after.document, second.document);
    assert_eq!(after.revision, second.revision);
    assert_eq!(after.created_at, first.created_at);
    assert_eq!(after.updated_at, second.updated_at);
    assert_eq!(after.updated_by, second.updated_by);
    worker.close().await.unwrap();
}
#[tokio::test]
async fn stale_revision_conflicts() {
    let (_temp, worker, docs) = harness().await;
    let (writer, _) = writer(&docs);
    let id = EntityId::new_v7();
    writer.put(action(id, "live")).await.unwrap();
    let mut stale = action(id, "stale");
    stale.expected_revision = Some(0);
    conflict(writer.put(stale).await.unwrap_err(), id);
    let missing = EntityId::new_v7();
    let mut request = action(missing, "missing");
    request.expected_revision = Some(1);
    conflict(writer.put(request).await.unwrap_err(), missing);
    assert!(docs.get("action".into(), missing).await.unwrap().is_none());
    let mut left = action(id, "left");
    left.expected_revision = Some(1);
    let mut right = action(id, "right");
    right.expected_revision = Some(1);
    let (left, right) = tokio::join!(writer.put(left), writer.put(right));
    assert_ne!(left.is_ok(), right.is_ok());
    conflict(
        if let Err(error) = left {
            error
        } else {
            right.unwrap_err()
        },
        id,
    );
    assert_eq!(
        docs.get("action".into(), id)
            .await
            .unwrap()
            .unwrap()
            .revision,
        2
    );
    worker.close().await.unwrap();
}
#[tokio::test]
async fn casefold_name_uniqueness() {
    let (_temp, worker, docs) = harness().await;
    let (writer, _) = writer(&docs);
    for kind in ["action", "queue", "command"] {
        for (name, alias) in [
            ("Straße", "STRASSE"),
            ("Σ", "ς"),
            ("ﬃ", "FFI"),
            ("İ", "i\u{307}"),
        ] {
            let id = EntityId::new_v7();
            writer
                .put(put(kind, id, json!({"name":name,"enabled":true})))
                .await
                .unwrap();
            let clashing = EntityId::new_v7();
            conflict(
                writer
                    .put(put(kind, clashing, json!({"name":alias})))
                    .await
                    .unwrap_err(),
                id,
            );
            assert!(docs.get(kind.into(), clashing).await.unwrap().is_none());
            // Self-renaming to an equivalent spelling is allowed and bumps the revision.
            assert_eq!(
                writer
                    .put(put(kind, id, json!({"name":alias})))
                    .await
                    .unwrap()
                    .revision,
                2
            );
        }
    }
    raw(&worker.handle, |c| {
        let (name, key, enabled): (String, String, Option<bool>) = c.config.query_row(
            "SELECT name,name_key,enabled FROM documents WHERE kind='action' AND name_key='strasse'",
            [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        ).unwrap();
        assert_eq!((name.as_str(), key.as_str(), enabled), ("STRASSE", "strasse", None));
    }).await;
    worker.close().await.unwrap();
}
#[tokio::test]
async fn ai_draft_exempt_from_name_index() {
    let (_temp, worker, docs) = harness().await;
    let (writer, _) = writer(&docs);
    let live = EntityId::new_v7();
    writer.put(action(live, "Straße")).await.unwrap();
    for _ in 0..2 {
        let id = EntityId::new_v7();
        let draft = writer.put(put(
            "action", id,
            json!({"name":"STRASSE","enabled":true,"origin":{"kind":"ai_draft","draftOf":live}}),
        )).await.unwrap();
        assert_eq!(draft.document.data["enabled"], false);
        conflict(writer.put(action(id, "strasse")).await.unwrap_err(), live);
        assert_eq!(
            docs.get("action".into(), id)
                .await
                .unwrap()
                .unwrap()
                .revision,
            1
        );
    }
    raw(&worker.handle, |c| {
        let enabled: bool = c.config.query_row(
            "SELECT enabled FROM documents WHERE json_extract(data,'$.origin.kind')='ai_draft' LIMIT 1",
            [], |r| r.get(0),
        ).unwrap();
        assert!(!enabled);
    }).await;
    // Attribution alone does not turn a user-origin storage input into a draft.
    assert_eq!(
        docs.get("action".into(), live)
            .await
            .unwrap()
            .unwrap()
            .document
            .data["enabled"],
        true
    );
    worker.close().await.unwrap();
}
#[tokio::test]
async fn missing_origin_kind_not_exempt() {
    let (_temp, worker, docs) = harness().await;
    let (writer, _) = writer(&docs);
    for (index, origin) in [
        None,
        Some(json!({})),
        Some(json!(null)),
        Some(json!({"kind":""})),
        Some(json!({"kind":null})),
        Some(json!({"kind":"future"})),
    ]
    .into_iter()
    .enumerate()
    {
        let id = EntityId::new_v7();
        let name = format!("absent{index}");
        let mut data = json!({"name":name});
        if let Some(origin) = origin {
            data["origin"] = origin;
        }
        writer.put(put("action", id, data)).await.unwrap();
        conflict(
            writer
                .put(action(EntityId::new_v7(), &name.to_uppercase()))
                .await
                .unwrap_err(),
            id,
        );
    }
    worker.close().await.unwrap();
}
#[tokio::test]
async fn view_state_does_not_bump_action() {
    let (_temp, worker, docs) = harness().await;
    let (writer, _) = writer(&docs);
    let id = EntityId::new_v7();
    let original = writer.put(action(id, "live")).await.unwrap();
    for x in [1, 2] {
        let view = writer
            .put(put("view_state", id, json!({"viewport":{"x":x}})))
            .await
            .unwrap();
        assert_eq!(view.revision, x);
        assert!(view.document.data.get("revision").is_none());
    }
    let after = docs.get("action".into(), id).await.unwrap().unwrap();
    assert_eq!(after.document, original.document);
    assert_eq!(after.revision, 1);
    worker.close().await.unwrap();
}
#[tokio::test]
async fn newer_versions_and_missing_migrations_are_write_protected() {
    let (_temp, worker, docs) = harness().await;
    let (writer, _) = writer(&docs);
    for version in [2, 99] {
        let id = EntityId::new_v7();
        writer
            .put(put("view_state", id, json!({"forward":{"unknown":true}})))
            .await
            .unwrap();
        raw(&worker.handle, move |c| {
            c.config
                .execute(
                    "UPDATE documents SET schema_version=?1 WHERE id=?2",
                    params![version, id.to_string()],
                )
                .unwrap();
        })
        .await;
        let before = docs.get("view_state".into(), id).await.unwrap().unwrap();
        let error = writer
            .put(put("view_state", id, json!({})))
            .await
            .unwrap_err();
        assert_eq!(error.code(), ErrorCode::new(ErrorCode::INVALID_STATE));
        assert!(
            matches!(error,DocumentError::UnsupportedVersion{id:clash,stored,supported:3} if clash==id && stored==version)
        );
        let after = docs.get("view_state".into(), id).await.unwrap().unwrap();
        assert_eq!(after.document, before.document);
        assert_eq!(after.revision, before.revision);
        assert_eq!(after.updated_at, before.updated_at);
        assert_eq!(after.updated_by, before.updated_by);
    }
    worker.close().await.unwrap();
}

#[tokio::test]
async fn writer_rejects_invalid_ids_and_enabled_before_persistence() {
    let (_temp, worker, docs) = harness().await;
    let (writer, _) = writer(&docs);
    let id = EntityId::new_v7();
    for body_id in [
        json!("not-a-uuid"),
        json!(null),
        json!(42),
        json!(EntityId::new_v7()),
    ] {
        let mut input = action(id, "invalid id");
        input.document.data["id"] = body_id;
        let error = writer.put(input).await.unwrap_err();
        assert!(matches!(error, DocumentError::InvalidInput(_)));
        assert_eq!(error.code(), ErrorCode::new(ErrorCode::INVALID_PARAMS));
    }
    for origin in ["user", "ai_draft"] {
        for enabled in [json!("yes"), json!(null), json!(42), json!({}), json!([])] {
            let input = put(
                "action",
                id,
                json!({"name":"invalid enabled","enabled":enabled,"origin":{"kind":origin}}),
            );
            let error = writer.put(input).await.unwrap_err();
            assert!(matches!(
                error,
                DocumentError::InvalidInput("enabled must be a boolean")
            ));
            assert_eq!(error.code(), ErrorCode::new(ErrorCode::INVALID_PARAMS));
        }
    }
    assert!(docs.get("action".into(), id).await.unwrap().is_none());
    worker.close().await.unwrap();
}

fn nested_body(depth: usize) -> Value {
    let mut value = Value::Null;
    for _ in 0..depth {
        value = Value::Array(vec![value]);
    }
    json!({"deep": value})
}

#[tokio::test]
async fn unreadable_bodies_are_invalid_input_and_never_inserted() {
    let (_temp, worker, docs) = harness().await;
    let (writer, _) = writer(&docs);
    // Include the reader boundary, the reported 200-level defect, and SQLite's 1000 limit.
    // Bounded fixtures avoid arbitrarily deep recursive Value clone/serialize/drop stacks.
    for depth in [200, 125, 126, 127, 1001] {
        let id = EntityId::new_v7();
        let body = nested_body(depth);
        let readable = serde_json::from_str::<Value>(&body.to_string()).is_ok();
        let result = writer.put(put("view_state", id, body)).await;
        if readable {
            let record = result.unwrap();
            let stored = docs.get("view_state".into(), id).await.unwrap().unwrap();
            assert_eq!(record.document, stored.document);
        } else {
            let error = result.unwrap_err();
            assert!(matches!(error, DocumentError::InvalidInput(_)), "{error:?}");
            assert_eq!(error.code(), ErrorCode::new(ErrorCode::INVALID_PARAMS));
            assert!(docs.get("view_state".into(), id).await.unwrap().is_none());
        }
    }
    worker.close().await.unwrap();
}

#[tokio::test]
async fn unreadable_update_preserves_body_revision_and_all_metadata() {
    let (_temp, worker, docs) = harness().await;
    let (writer, clock) = writer(&docs);
    let id = EntityId::new_v7();
    let before = writer.put(action(id, "before")).await.unwrap();
    *clock.0.lock().unwrap() = Timestamp::parse("2026-10-20T12:00:00Z").unwrap();
    let mut invalid = action(id, "after");
    invalid.document.data["deep"] = nested_body(200);
    invalid.expected_revision = Some(1);
    invalid.updated_by = UpdatedBy::Mcp;
    let error = writer.put(invalid).await.unwrap_err();
    assert!(matches!(error, DocumentError::InvalidInput(_)));
    assert_eq!(error.code(), ErrorCode::new(ErrorCode::INVALID_PARAMS));
    let after = docs.get("action".into(), id).await.unwrap().unwrap();
    assert_eq!(after.document, before.document);
    assert_eq!(after.revision, before.revision);
    assert_eq!(after.created_at, before.created_at);
    assert_eq!(after.updated_at, before.updated_at);
    assert_eq!(after.updated_by, before.updated_by);
    raw(&worker.handle, |c| {
        let metadata: (String, String, bool) = c
            .config
            .query_row("SELECT name,name_key,enabled FROM documents", [], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .unwrap();
        assert_eq!(metadata, ("before".into(), "before".into(), true));
    })
    .await;
    let mut valid = action(id, "recovered");
    valid.expected_revision = Some(1);
    assert_eq!(writer.put(valid).await.unwrap().revision, 2);
    assert_eq!(
        docs.get("action".into(), id)
            .await
            .unwrap()
            .unwrap()
            .revision,
        2
    );
    worker.close().await.unwrap();
}

#[tokio::test]
async fn command_and_timer_body_revisions_are_server_assigned() {
    let (_temp, worker, docs) = harness().await;
    let (writer, _) = writer(&docs);
    for kind in ["command", "timer"] {
        for supplied in [None, Some(77)] {
            let id = EntityId::new_v7();
            let mut body = json!({"name":id.to_string(),"enabled":true});
            if let Some(revision) = supplied {
                body["revision"] = json!(revision);
            }
            for revision in [1, 2] {
                let mut input = put(kind, id, body.clone());
                input.expected_revision = (revision > 1).then_some(revision - 1);
                let returned = writer.put(input).await.unwrap();
                let stored = docs.get(kind.into(), id).await.unwrap().unwrap();
                assert_eq!(returned.revision, revision);
                assert_eq!(stored.revision, revision);
                assert_eq!(returned.document.data["revision"], revision);
                assert_eq!(stored.document, returned.document);
                raw(&worker.handle, move |c| {
                    let revisions: (i64, i64) = c.config.query_row(
                        "SELECT revision,json_extract(data,'$.revision') FROM documents WHERE kind=?1 AND id=?2",
                        params![kind, id.to_string()], |r| Ok((r.get(0)?, r.get(1)?)),
                    ).unwrap();
                    assert_eq!(revisions, (revision as i64, revision as i64));
                }).await;
            }
            let mut stale = put(kind, id, body);
            stale.expected_revision = Some(1);
            conflict(writer.put(stale).await.unwrap_err(), id);
            assert_eq!(
                docs.get(kind.into(), id).await.unwrap().unwrap().revision,
                2
            );
        }
    }
    // Kinds without a contracted body revision retain caller data exactly.
    for kind in ["queue", "view_state"] {
        for supplied in [None, Some(77)] {
            let id = EntityId::new_v7();
            let mut body = json!({"name":id.to_string()});
            if let Some(revision) = supplied {
                body["revision"] = json!(revision);
            }
            for revision in [1, 2] {
                let returned = writer.put(put(kind, id, body.clone())).await.unwrap();
                assert_eq!(returned.revision, revision);
                assert_eq!(returned.document.data, body);
                assert_eq!(
                    docs.get(kind.into(), id)
                        .await
                        .unwrap()
                        .unwrap()
                        .document
                        .data,
                    body
                );
            }
        }
    }
    worker.close().await.unwrap();
}

#[tokio::test]
async fn failed_after_insert_rolls_back_entire_new_row() {
    let (_temp, worker, docs) = harness().await;
    let (writer, _) = writer(&docs);
    let id = EntityId::new_v7();
    raw(&worker.handle, |c| {
        c.config
            .execute_batch(
                "CREATE TRIGGER fail_insert AFTER INSERT ON documents
             BEGIN SELECT RAISE(ABORT,'injected insert'); END",
            )
            .unwrap();
    })
    .await;
    let error = writer.put(action(id, "insert")).await.unwrap_err();
    assert!(matches!(error, DocumentError::Sqlite(_)));
    assert_eq!(error.code(), ErrorCode::new(ErrorCode::INTERNAL));
    assert!(docs.get("action".into(), id).await.unwrap().is_none());
    raw(&worker.handle, |c| {
        let count: i64 = c
            .config
            .query_row("SELECT count(*) FROM documents", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);
        c.config.execute_batch("DROP TRIGGER fail_insert").unwrap();
    })
    .await;
    assert_eq!(writer.put(action(id, "insert")).await.unwrap().revision, 1);
    worker.close().await.unwrap();
}

struct LockProbeClock {
    path: std::path::PathBuf,
    observed: Arc<Mutex<Option<bool>>>,
}
impl Clock for LockProbeClock {
    fn monotonic(&self) -> tokio::time::Instant {
        tokio::time::Instant::now()
    }
    fn wall(&self) -> Timestamp {
        // Called after BEGIN and before read/prepare: the second writer must already be blocked.
        let conn = Connection::open(&self.path).unwrap();
        conn.busy_timeout(std::time::Duration::ZERO).unwrap();
        let result = conn.execute_batch("BEGIN IMMEDIATE");
        let blocked = matches!(&result, Err(rusqlite::Error::SqliteFailure(e, _))
            if e.code == rusqlite::ErrorCode::DatabaseBusy);
        if result.is_ok() {
            conn.execute_batch("ROLLBACK").unwrap();
        }
        *self.observed.lock().unwrap() = Some(blocked);
        clock().wall()
    }
}

#[tokio::test]
async fn immediate_write_lock_precedes_read_and_prepare() {
    let (temp, worker, docs) = harness().await;
    let path = temp.path().join("config.db");
    let conn = Connection::open(&path).unwrap();
    conn.busy_timeout(std::time::Duration::ZERO).unwrap();
    conn.execute_batch("BEGIN IMMEDIATE; ROLLBACK").unwrap();
    let observed = Arc::new(Mutex::new(None));
    let writer = docs.writer(
        policy(),
        Arc::new(LockProbeClock {
            path,
            observed: observed.clone(),
        }),
    );
    let id = EntityId::new_v7();
    for revision in [1, 2] {
        *observed.lock().unwrap() = None;
        assert_eq!(
            writer.put(action(id, "locked")).await.unwrap().revision,
            revision
        );
        assert_eq!(*observed.lock().unwrap(), Some(true));
        // Successful commit releases the lock, including on the update path.
        conn.execute_batch("BEGIN IMMEDIATE; ROLLBACK").unwrap();
    }
    worker.close().await.unwrap();
}
