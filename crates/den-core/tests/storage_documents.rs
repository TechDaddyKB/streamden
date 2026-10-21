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

#[tokio::test]
async fn batch_rolls_back_bodies_revisions_names_and_metadata() {
    let (_temp, worker, docs) = harness().await;
    let (writer, clock) = writer(&docs);
    let id = EntityId::new_v7();
    let before = writer.put(action(id, "original")).await.unwrap();
    *clock.0.lock().unwrap() = Timestamp::parse("2026-10-20T12:00:00Z").unwrap();
    let fresh = EntityId::new_v7();
    let mut stale = action(id, "stale");
    stale.expected_revision = Some(1);
    conflict(
        writer
            .batch_put(vec![action(id, "changed"), action(fresh, "fresh"), stale])
            .await
            .unwrap_err(),
        id,
    );
    conflict(
        writer
            .batch_put(vec![action(id, "changed"), action(fresh, "CHANGED")])
            .await
            .unwrap_err(),
        id,
    );
    let after = docs.get("action".into(), id).await.unwrap().unwrap();
    assert_eq!(after.document, before.document);
    assert_eq!(after.revision, before.revision);
    assert_eq!(after.created_at, before.created_at);
    assert_eq!(after.updated_at, before.updated_at);
    assert_eq!(after.updated_by, before.updated_by);
    assert!(docs.get("action".into(), fresh).await.unwrap().is_none());
    // Rollback restores the name index as well as the body.
    writer.put(action(fresh, "changed")).await.unwrap();
    raw(&worker.handle, move |c| {
        let metadata: (String, String, bool) = c
            .config
            .query_row(
                "SELECT name,name_key,enabled FROM documents WHERE id=?1",
                [id.to_string()],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(metadata, ("original".into(), "original".into(), true));
    })
    .await;
    assert!(writer.batch_put(vec![]).await.unwrap().is_empty());
    worker.close().await.unwrap();
}

#[tokio::test]
async fn batch_success_assigns_revisions_in_order_and_keeps_draft_guards() {
    let (_temp, worker, docs) = harness().await;
    let (writer, clock) = writer(&docs);
    for kind in ["action", "command", "timer"] {
        let id = EntityId::new_v7();
        let mut second = put(kind, id, json!({"name":kind,"revision":99}));
        second.expected_revision = Some(1);
        second.updated_by = UpdatedBy::Mcp;
        let records = writer
            .batch_put(vec![put(kind, id, json!({"name":kind})), second])
            .await
            .unwrap();
        assert_eq!(
            records.iter().map(|r| r.revision).collect::<Vec<_>>(),
            vec![1, 2]
        );
        for record in &records {
            assert_eq!(record.document.data["revision"], record.revision);
            assert_eq!(record.created_at, clock.wall());
            assert_eq!(record.updated_at, clock.wall());
        }
        let stored = docs.get(kind.into(), id).await.unwrap().unwrap();
        assert_eq!(stored.document, records[1].document);
        assert_eq!(stored.revision, 2);
        assert_eq!(stored.updated_by, UpdatedBy::Mcp);
    }
    let live = EntityId::new_v7();
    let draft = EntityId::new_v7();
    let draft_input = put(
        "action",
        draft,
        json!({"name":"STRASSE","enabled":true,"origin":{"kind":"ai_draft","draftOf":live}}),
    );
    let records = writer
        .batch_put(vec![action(live, "Straße"), draft_input])
        .await
        .unwrap();
    assert_eq!(records[1].document.data["enabled"], false);
    conflict(
        writer
            .batch_put(vec![action(draft, "strasse")])
            .await
            .unwrap_err(),
        live,
    );
    assert_eq!(
        docs.get("action".into(), draft)
            .await
            .unwrap()
            .unwrap()
            .revision,
        1
    );
    worker.close().await.unwrap();
}

#[tokio::test]
async fn delete_checks_revision_kind_version_and_view_state_isolation() {
    let (temp, worker, docs) = harness().await;
    let (writer, _) = writer(&docs);
    let id = EntityId::new_v7();
    let kinds = [
        "view_state",
        "action",
        "queue",
        "command",
        "timer",
        "settings",
        "pending_change",
        "export_meta",
    ];
    for kind in kinds {
        writer
            .put(put(kind, id, json!({"name":"same","enabled":true})))
            .await
            .unwrap();
    }
    // A separate writer lock must prevent deletion; the row stays intact.
    let conn = Connection::open(temp.path().join("config.db")).unwrap();
    conn.execute_batch("BEGIN IMMEDIATE").unwrap();
    assert!(matches!(
        writer.delete("action".into(), id, Some(1)).await,
        Err(DocumentError::Sqlite(_))
    ));
    conn.execute_batch("ROLLBACK").unwrap();
    for kind in kinds {
        conflict(
            writer.delete(kind.into(), id, Some(2)).await.unwrap_err(),
            id,
        );
        assert!(docs.get(kind.into(), id).await.unwrap().is_some());
        assert!(writer.delete(kind.into(), id, Some(1)).await.unwrap());
        assert!(docs.get(kind.into(), id).await.unwrap().is_none());
        assert!(!writer.delete(kind.into(), id, None).await.unwrap());
        conflict(
            writer.delete(kind.into(), id, Some(1)).await.unwrap_err(),
            id,
        );
        if kind == "view_state" {
            assert_eq!(
                docs.get("action".into(), id)
                    .await
                    .unwrap()
                    .unwrap()
                    .revision,
                1
            );
        }
    }
    let empty = docs.writer(
        WritePolicy::new(Default::default()).unwrap(),
        Arc::new(clock()),
    );
    for error in [
        empty.delete("action".into(), id, None).await.unwrap_err(),
        empty
            .batch_put(vec![action(id, "unregistered")])
            .await
            .unwrap_err(),
        writer.delete("export".into(), id, None).await.unwrap_err(),
    ] {
        assert_eq!(error.code(), ErrorCode::new(ErrorCode::INVALID_PARAMS));
    }
    worker.close().await.unwrap();
}

#[tokio::test]
async fn delete_and_batch_preserve_older_and_newer_rows_without_migrations() {
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
        let fresh = EntityId::new_v7();
        for error in [
            writer
                .delete("view_state".into(), id, None)
                .await
                .unwrap_err(),
            writer
                .batch_put(vec![
                    action(fresh, "rolled-back"),
                    put("view_state", id, json!({})),
                ])
                .await
                .unwrap_err(),
        ] {
            assert_eq!(error.code(), ErrorCode::new(ErrorCode::INVALID_STATE));
            assert!(
                matches!(error, DocumentError::UnsupportedVersion { id: clash, stored, supported: 3 } if clash == id && stored == version)
            );
        }
        let after = docs.get("view_state".into(), id).await.unwrap().unwrap();
        assert_eq!(after.document, before.document);
        assert_eq!(after.revision, before.revision);
        assert_eq!(after.created_at, before.created_at);
        assert_eq!(after.updated_at, before.updated_at);
        assert_eq!(after.updated_by, before.updated_by);
        assert!(docs.get("action".into(), fresh).await.unwrap().is_none());
    }
    worker.close().await.unwrap();
}

#[tokio::test]
async fn batch_invalid_inputs_and_overflow_roll_back_prior_writes() {
    let (_temp, worker, docs) = harness().await;
    let (writer, _) = writer(&docs);
    let id = EntityId::new_v7();
    let mut version = action(id, "bad");
    version.document.schema_version = 99;
    for input in [
        version,
        put("export", id, json!({})),
        put("action", id, json!({})),
        put("queue", id, json!({"name":null})),
        put("command", id, json!({"name":42})),
        put("timer", id, json!(null)),
        put("timer", id, json!({"enabled":"yes"})),
        put("timer", id, json!({"id":EntityId::new_v7()})),
        put("timer", id, json!({"id":"bad"})),
        put("view_state", id, nested_body(200)),
    ] {
        let fresh = EntityId::new_v7();
        let error = writer
            .batch_put(vec![action(fresh, "rolled-back"), input])
            .await
            .unwrap_err();
        assert_eq!(error.code(), ErrorCode::new(ErrorCode::INVALID_PARAMS));
        assert!(docs.get("action".into(), fresh).await.unwrap().is_none());
    }
    writer.put(action(id, "max")).await.unwrap();
    raw(&worker.handle, move |c| {
        c.config
            .execute(
                "UPDATE documents SET revision=?1 WHERE id=?2",
                params![i64::MAX, id.to_string()],
            )
            .unwrap();
    })
    .await;
    let fresh = EntityId::new_v7();
    assert!(matches!(
        writer
            .batch_put(vec![action(fresh, "rolled-back"), action(id, "overflow")])
            .await,
        Err(DocumentError::InvalidStored("revision overflow"))
    ));
    let after = docs.get("action".into(), id).await.unwrap().unwrap();
    assert_eq!(after.revision, i64::MAX as u64);
    assert_eq!(after.document.data["name"], "max");
    assert!(docs.get("action".into(), fresh).await.unwrap().is_none());
    worker.close().await.unwrap();
}

#[tokio::test]
async fn batch_sql_failure_rolls_back_other_kinds_and_success_keeps_view_state_separate() {
    let (_temp, worker, docs) = harness().await;
    let (writer, _) = writer(&docs);
    let id = EntityId::new_v7();
    let before = writer.put(action(id, "before")).await.unwrap();
    raw(&worker.handle, |c| {
        c.config
            .execute_batch(
                "CREATE TRIGGER fail_timer AFTER INSERT ON documents
            WHEN NEW.kind='timer' BEGIN SELECT RAISE(ABORT,'injected batch failure'); END",
            )
            .unwrap();
    })
    .await;
    let requests = vec![
        action(id, "after"),
        put("view_state", id, json!({"x":1})),
        put("timer", id, json!({})),
    ];
    assert!(matches!(
        writer.batch_put(requests.clone()).await,
        Err(DocumentError::Sqlite(_))
    ));
    let after = docs.get("action".into(), id).await.unwrap().unwrap();
    assert_eq!(after.document, before.document);
    assert_eq!(after.revision, before.revision);
    for kind in ["view_state", "timer"] {
        assert!(docs.get(kind.into(), id).await.unwrap().is_none());
    }
    raw(&worker.handle, |c| {
        c.config.execute_batch("DROP TRIGGER fail_timer").unwrap();
    })
    .await;
    let records = writer.batch_put(requests).await.unwrap();
    assert_eq!(
        records.iter().map(|r| r.revision).collect::<Vec<_>>(),
        vec![2, 1, 1]
    );
    assert!(records[1].document.data.get("revision").is_none());
    for record in records {
        assert_eq!(
            docs.get(record.document.kind.clone(), id)
                .await
                .unwrap()
                .unwrap()
                .document,
            record.document
        );
    }
    worker.close().await.unwrap();
}

use den_core::storage::documents::{Filter, Record};

async fn listed(
    docs: &Documents,
    kind: &str,
    name: Option<&str>,
    enabled: Option<bool>,
) -> Vec<Record> {
    docs.list(
        kind.into(),
        Filter {
            name: name.map(str::to_owned),
            enabled,
        },
    )
    .await
    .unwrap()
}
fn ids(records: &[Record]) -> Vec<EntityId> {
    records.iter().map(|record| record.id).collect()
}

#[tokio::test]
async fn all_kinds_get_list_delete_and_filters() {
    let (_temp, worker, docs) = harness().await;
    let (writer, _) = writer(&docs);
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
    let keys: Vec<EntityId> = [
        "00000000-0000-7000-8000-000000000001",
        "00000000-0000-7000-8000-000000000002",
        "00000000-0000-7000-8000-000000000003",
    ]
    .into_iter()
    .map(|s| s.parse().unwrap())
    .collect();
    for kind in kinds {
        assert!(listed(&docs, kind, None, None).await.is_empty());
        for index in (0..3).rev() {
            let name = ["Straße", "other", "third"][index];
            let mut data = json!({"name":name, "extra":{"kind":kind}});
            if index < 2 {
                data["enabled"] = json!(index == 0);
            }
            writer.put(put(kind, keys[index], data)).await.unwrap();
        }
    }
    for kind in kinds {
        let all = listed(&docs, kind, None, None).await;
        assert_eq!(ids(&all), keys);
        assert_eq!(ids(&listed(&docs, kind, None, None).await), keys);
        for record in all {
            let got = docs.get(kind.into(), record.id).await.unwrap().unwrap();
            assert_eq!(record.document, got.document);
            assert_eq!(record.revision, got.revision);
            assert_eq!(record.created_at, got.created_at);
            assert_eq!(record.updated_at, got.updated_at);
            assert_eq!(record.updated_by, got.updated_by);
            assert_eq!(record.document.data["extra"]["kind"], kind);
        }
        for (name, enabled, expected) in [
            (None, Some(true), vec![keys[0]]),
            (None, Some(false), vec![keys[1]]),
            (Some("STRASSE"), None, vec![keys[0]]),
            (Some("strasse"), Some(true), vec![keys[0]]),
            (Some("STRASSE"), Some(false), vec![]),
            (Some("missing"), None, vec![]),
        ] {
            assert_eq!(ids(&listed(&docs, kind, name, enabled).await), expected);
        }
        assert!(writer.delete(kind.into(), keys[0], Some(1)).await.unwrap());
        assert!(docs.get(kind.into(), keys[0]).await.unwrap().is_none());
        assert_eq!(ids(&listed(&docs, kind, None, None).await), keys[1..]);
        assert!(listed(&docs, kind, Some("STRASSE"), None).await.is_empty());
        // All not-yet-deleted kinds retain the same ID and revision.
        for other in kinds.into_iter().skip_while(|other| *other != kind).skip(1) {
            assert_eq!(
                docs.get(other.into(), keys[0])
                    .await
                    .unwrap()
                    .unwrap()
                    .revision,
                1
            );
        }
    }
    worker.close().await.unwrap();
}

#[tokio::test]
async fn raw_listing_preserves_unknown_versions_and_reports_local_errors() {
    let (_temp, worker, docs) = harness().await;
    let (writer, _) = writer(&docs);
    let id = EntityId::new_v7();
    writer
        .put(put("timer", id, json!({"enabled":true,"unknown":[1,null]})))
        .await
        .unwrap();
    raw(&worker.handle, |c| {
        c.config
            .execute_batch("UPDATE documents SET schema_version=99")
            .unwrap();
    })
    .await;
    let row = listed(&docs, "timer", None, None).await.remove(0);
    assert_eq!(row.document.schema_version, 99);
    assert_eq!(row.document.data["unknown"], json!([1, null]));
    for (sql, expected) in [
        ("UPDATE documents SET revision=0", "revision"),
        (
            "UPDATE documents SET revision=1,created_at='bad'",
            "timestamp",
        ),
        (
            "UPDATE documents SET created_at='2026-10-20T10:00:00Z',updated_by='{}'",
            "json",
        ),
        (
            "UPDATE documents SET updated_by='{\"kind\":\"system\"}',id='bad'",
            "id",
        ),
    ] {
        raw(&worker.handle, move |c| {
            c.config.execute_batch(sql).unwrap();
        })
        .await;
        let error = docs
            .list("timer".into(), Filter::default())
            .await
            .unwrap_err();
        assert_eq!(error.code(), ErrorCode::new(ErrorCode::INTERNAL));
        assert!(match expected {
            "revision" => matches!(
                error,
                DocumentError::InvalidStored("invalid stored revision")
            ),
            "timestamp" => matches!(
                error,
                DocumentError::InvalidStored("invalid stored timestamp")
            ),
            "json" => matches!(error, DocumentError::Json(_)),
            "id" => matches!(error, DocumentError::InvalidStored("invalid listed id")),
            _ => unreachable!(),
        });
        // Failure rolls back the read snapshot and leaves unrelated kinds listable.
        assert!(listed(&docs, "queue", None, None).await.is_empty());
    }
    let error = docs
        .list("export".into(), Filter::default())
        .await
        .unwrap_err();
    assert_eq!(error.code(), ErrorCode::new(ErrorCode::INVALID_PARAMS));
    raw(&worker.handle, |c| {
        c.config.execute_batch("DROP TABLE documents").unwrap();
    })
    .await;
    assert!(matches!(
        docs.list("timer".into(), Filter::default()).await,
        Err(DocumentError::Sqlite(_))
    ));
    worker.close().await.unwrap();
    assert!(matches!(
        docs.list("timer".into(), Filter::default()).await,
        Err(DocumentError::Transport(StorageError::Disconnected))
    ));
}

fn stored_document_rows(conn: &Connection) -> Vec<Vec<rusqlite::types::Value>> {
    let mut statement = conn
        .prepare("SELECT * FROM documents ORDER BY kind COLLATE BINARY,id COLLATE BINARY")
        .unwrap();
    let columns = statement.column_count();
    statement
        .query_map([], |row| {
            (0..columns).map(|column| row.get(column)).collect()
        })
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}

fn same_record(actual: &Record, expected: &Record) {
    assert_eq!(actual.id, expected.id);
    assert_eq!(actual.document, expected.document);
    assert_eq!(actual.revision, expected.revision);
    assert_eq!(actual.created_at, expected.created_at);
    assert_eq!(actual.updated_at, expected.updated_at);
    assert_eq!(actual.updated_by, expected.updated_by);
}

async fn noncanonical_listed_id(alias: &str) {
    let canonical = "0190a000-0000-7000-8000-00000000abcd";
    let id: EntityId = canonical.parse().unwrap();
    // Public parsing remains permissive; only stored row identity must be canonical.
    assert_eq!(alias.parse::<EntityId>().unwrap(), id);
    assert_ne!(alias, canonical);
    let mut failures = Vec::new();
    for coexist in [false, true] {
        let (temp, worker, docs) = harness().await;
        let (writer, _) = writer(&docs);
        let canonical_record = if coexist {
            Some(
                writer
                    .put(put(
                        "timer",
                        id,
                        json!({"name":"canonical","enabled":true,"tag":"canonical"}),
                    ))
                    .await
                    .unwrap(),
            )
        } else {
            None
        };
        let unrelated = writer
            .put(put("queue", id, json!({"name":"unrelated"})))
            .await
            .unwrap();
        let stored = alias.to_owned();
        raw(&worker.handle, move |c| {
            c.config
                .execute(
                    "INSERT INTO documents(kind,id,schema_version,revision,name,name_key,enabled,
                 data,created_at,updated_at,updated_by)
                 VALUES('timer',?1,99,7,'alias','alias',0,'{\"tag\":\"alias\"}',
                 '2026-10-19T10:00:00Z','2026-10-20T11:00:00Z','{\"kind\":\"imported\"}')",
                    [stored],
                )
                .unwrap();
        })
        .await;
        let conn = Connection::open(temp.path().join("config.db")).unwrap();
        let before = stored_document_rows(&conn);
        assert_eq!(before.len(), if coexist { 3 } else { 2 });
        for filter in [
            Filter::default(),
            Filter {
                name: Some("alias".into()),
                enabled: None,
            },
            Filter {
                name: None,
                enabled: Some(false),
            },
            Filter {
                name: Some("alias".into()),
                enabled: Some(false),
            },
        ] {
            let result = docs.list("timer".into(), filter.clone()).await;
            let correct = matches!(
                &result,
                Err(DocumentError::InvalidStored("invalid listed id"))
            );
            if !correct {
                failures.push(format!(
                    "{alias} coexist={coexist} filter={filter:?}: {result:?}"
                ));
            }
            if let Err(error) = result {
                assert_eq!(error.code(), ErrorCode::new(ErrorCode::INTERNAL));
            }
            // A failed list never repairs, deletes, renames or rewrites any row/metadata.
            assert_eq!(stored_document_rows(&conn), before);
            raw(&worker.handle, |c| assert!(c.config.is_autocommit())).await;
            let recovered = listed(&docs, "queue", None, None).await;
            assert_eq!(recovered.len(), 1);
            same_record(&recovered[0], &unrelated);
        }
        // Either legitimate predicate can exclude the bad row before identity decoding.
        for (name, enabled) in [
            (Some("canonical"), None),
            (None, Some(true)),
            (Some("canonical"), Some(true)),
        ] {
            let records = listed(&docs, "timer", name, enabled).await;
            assert_eq!(records.len(), usize::from(coexist));
            if let Some(expected) = &canonical_record {
                same_record(&records[0], expected);
                same_record(
                    &docs.get("timer".into(), id).await.unwrap().unwrap(),
                    expected,
                );
            } else {
                assert!(docs.get("timer".into(), id).await.unwrap().is_none());
            }
        }
        assert_eq!(stored_document_rows(&conn), before);
        worker.close().await.unwrap();
    }
    assert!(
        failures.is_empty(),
        "alias identity failures:\n{}",
        failures.join("\n")
    );
}

#[tokio::test]
async fn listing_rejects_uppercase_stored_ids_alone_and_beside_canonical() {
    noncanonical_listed_id("0190A000-0000-7000-8000-00000000ABCD").await;
}

#[tokio::test]
async fn listing_rejects_braced_stored_ids_alone_and_beside_canonical() {
    noncanonical_listed_id("{0190a000-0000-7000-8000-00000000abcd}").await;
}

#[tokio::test]
async fn listing_rejects_simple_stored_ids_alone_and_beside_canonical() {
    noncanonical_listed_id("0190a00000007000800000000000abcd").await;
}

#[tokio::test]
async fn listing_rejects_urn_stored_ids_alone_and_beside_canonical() {
    noncanonical_listed_id("urn:uuid:0190a000-0000-7000-8000-00000000abcd").await;
}

#[tokio::test]
async fn listed_names_follow_rename_conflict_and_delete_indexes() {
    let (_temp, worker, docs) = harness().await;
    let (writer, _) = writer(&docs);
    for kind in ["action", "queue", "command"] {
        for (name, alias) in [
            ("Straße", "STRASSE"),
            ("Σ", "ς"),
            ("ﬃ", "FFI"),
            ("İ", "i\u{307}"),
        ] {
            let first = EntityId::new_v7();
            let second = EntityId::new_v7();
            writer
                .put(put(kind, first, json!({"name":name})))
                .await
                .unwrap();
            assert_eq!(
                ids(&listed(&docs, kind, Some(alias), None).await),
                vec![first]
            );
            conflict(
                writer
                    .put(put(kind, second, json!({"name":alias})))
                    .await
                    .unwrap_err(),
                first,
            );
            assert!(docs.get(kind.into(), second).await.unwrap().is_none());
            // Equivalent self-renames keep the same index owner and remain listable.
            let equivalent = writer
                .put(put(kind, first, json!({"name":alias})))
                .await
                .unwrap();
            assert_eq!(equivalent.revision, 2);
            let rows = listed(&docs, kind, Some(name), None).await;
            assert_eq!(ids(&rows), vec![first]);
            same_record(&rows[0], &equivalent);
            let renamed = writer
                .put(put(kind, first, json!({"name":"renamed"})))
                .await
                .unwrap();
            assert_eq!(renamed.revision, 3);
            assert!(listed(&docs, kind, Some(alias), None).await.is_empty());
            writer
                .put(put(kind, second, json!({"name":alias})))
                .await
                .unwrap();
            conflict(
                writer
                    .put(put(kind, first, json!({"name":name})))
                    .await
                    .unwrap_err(),
                second,
            );
            assert_eq!(
                ids(&listed(&docs, kind, Some("renamed"), None).await),
                vec![first]
            );
            assert!(writer.delete(kind.into(), second, Some(1)).await.unwrap());
            writer
                .put(put(kind, first, json!({"name":alias})))
                .await
                .unwrap();
            assert_eq!(
                ids(&listed(&docs, kind, Some(name), None).await),
                vec![first]
            );
            assert!(writer.delete(kind.into(), first, Some(4)).await.unwrap());
        }
    }
    worker.close().await.unwrap();
}

#[tokio::test]
async fn listing_drafts_and_missing_origin_keeps_live_name_reservations() {
    let (_temp, worker, docs) = harness().await;
    let (writer, _) = writer(&docs);
    let live = EntityId::new_v7();
    let drafts = [EntityId::new_v7(), EntityId::new_v7()];
    for id in drafts {
        writer
            .put(put(
                "action",
                id,
                json!({"name":"STRASSE","enabled":true,"origin":{"kind":"ai_draft","draftOf":live}}),
            ))
            .await
            .unwrap();
    }
    // Drafts inserted first must not prevent a subsequent live action with the same folded name.
    writer.put(action(live, "Straße")).await.unwrap();
    let mut all_ids = vec![live, drafts[0], drafts[1]];
    all_ids.sort_by_key(ToString::to_string);
    assert_eq!(
        ids(&listed(&docs, "action", Some("strasse"), None).await),
        all_ids
    );
    assert_eq!(
        ids(&listed(&docs, "action", Some("STRASSE"), Some(true)).await),
        vec![live]
    );
    let disabled = listed(&docs, "action", Some("Straße"), Some(false)).await;
    assert_eq!(disabled.len(), 2);
    for row in disabled {
        assert!(drafts.contains(&row.id));
        assert_eq!(row.document.data["enabled"], false);
        assert_eq!(row.document.data["origin"]["kind"], "ai_draft");
        conflict(
            writer.put(action(row.id, "strasse")).await.unwrap_err(),
            live,
        );
        same_record(
            &docs.get("action".into(), row.id).await.unwrap().unwrap(),
            &row,
        );
    }
    assert_eq!(listed(&docs, "action", None, Some(false)).await.len(), 2);
    assert!(writer.delete("action".into(), live, Some(1)).await.unwrap());
    writer.put(action(drafts[0], "Straße")).await.unwrap();
    assert_eq!(
        ids(&listed(&docs, "action", None, Some(true)).await),
        vec![drafts[0]]
    );
    assert_eq!(
        ids(&listed(&docs, "action", None, Some(false)).await),
        vec![drafts[1]]
    );
    for (index, origin) in [
        None,
        Some(json!({})),
        Some(json!(null)),
        Some(json!({"kind":null})),
        Some(json!({"kind":"future"})),
    ]
    .into_iter()
    .enumerate()
    {
        let id = EntityId::new_v7();
        let name = format!("reserved{index}");
        let mut data = json!({"name":name,"enabled":false});
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
        assert_eq!(
            ids(&listed(&docs, "action", Some(&name), Some(false)).await),
            vec![id]
        );
    }
    worker.close().await.unwrap();
}

#[tokio::test]
async fn list_filters_are_literal_and_null_is_not_false_or_empty_name() {
    let (_temp, worker, docs) = harness().await;
    let (writer, _) = writer(&docs);
    for kind in [
        "timer",
        "settings",
        "view_state",
        "pending_change",
        "export_meta",
    ] {
        let unnamed = EntityId::new_v7();
        writer
            .put(put(kind, unnamed, json!({"opaque":[1,null]})))
            .await
            .unwrap();
        assert!(listed(&docs, kind, Some(""), None).await.is_empty());
        assert!(listed(&docs, kind, None, Some(false)).await.is_empty());
        let names = [
            "",
            "%_'; DROP TABLE documents;--",
            " spaced ",
            "é",
            "e\u{301}",
        ];
        let mut named = Vec::new();
        for name in names {
            let id = EntityId::new_v7();
            writer
                .put(put(kind, id, json!({"name":name,"enabled":false})))
                .await
                .unwrap();
            named.push((name, id));
            assert_eq!(
                ids(&listed(&docs, kind, Some(name), Some(false)).await),
                vec![id]
            );
        }
        // Recheck after both canonical-equivalent spellings exist: folding is not normalization.
        for (name, id) in named {
            assert_eq!(ids(&listed(&docs, kind, Some(name), None).await), vec![id]);
        }
        for absent in ["spaced", "%", "_", "\" OR 1=1 --"] {
            assert!(listed(&docs, kind, Some(absent), None).await.is_empty());
        }
        assert_eq!(listed(&docs, kind, None, Some(false)).await.len(), 5);
        assert_eq!(listed(&docs, kind, None, None).await.len(), 6);
        assert!(docs.get(kind.into(), unnamed).await.unwrap().is_some());
    }
    worker.close().await.unwrap();
}

use den_core::storage::documents::Invalid;
use rusqlite::types::Value as SqlValue;

async fn seed_document(
    worker: &StorageWorker,
    kind: &str,
    id: EntityId,
    version: u32,
    data: Value,
) {
    let kind = kind.to_owned();
    raw(&worker.handle, move |c| {
        c.config.execute(
            "INSERT INTO documents(kind,id,schema_version,revision,data,created_at,updated_at,updated_by)
             VALUES(?1,?2,?3,1,?4,'2026-10-20T10:00:00Z','2026-10-20T10:00:00Z','{\"kind\":\"system\"}')",
            params![kind, id.to_string(), version, data.to_string()],
        ).unwrap();
    }).await;
}

async fn raw_snapshot(worker: &StorageWorker) -> Vec<Vec<(rusqlite::types::Type, SqlValue)>> {
    let result = Arc::new(Mutex::new(Vec::new()));
    let output = result.clone();
    raw(&worker.handle, move |c| {
        let mut query = c
            .config
            .prepare("SELECT * FROM documents ORDER BY kind,id")
            .unwrap();
        let columns = query.column_count();
        *output.lock().unwrap() = query
            .query_map([], |r| {
                (0..columns)
                    .map(|i| {
                        let field = r.get_ref(i)?;
                        let value = match field {
                            rusqlite::types::ValueRef::Text(bytes) => {
                                SqlValue::Blob(bytes.to_vec())
                            }
                            _ => r.get(i)?,
                        };
                        Ok((field.data_type(), value))
                    })
                    .collect::<rusqlite::Result<Vec<_>>>()
            })
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
    })
    .await;
    Arc::try_unwrap(result).unwrap().into_inner().unwrap()
}

#[tokio::test]
async fn row_corruption_is_nonfatal_but_query_and_transport_failures_are_not() {
    let (_temp, worker, docs) = harness().await;
    let good = EntityId::new_v7();
    seed_document(&worker, "view_state", good, 3, json!({"ok":true})).await;
    let expected_good = docs.get("view_state".into(), good).await.unwrap().unwrap();
    // Corruption fixture has no affinities/checks so every wrong storage class stays wrong.
    raw(&worker.handle, |c| c.config.execute_batch(
        "ALTER TABLE documents RENAME TO saved_documents;
         CREATE TABLE documents(kind,id,schema_version,revision,data,created_at,updated_at,updated_by);
         INSERT INTO documents SELECT kind,id,schema_version,revision,data,created_at,updated_at,updated_by FROM saved_documents;
         DROP TABLE saved_documents;"
    ).unwrap()).await;
    let cases = [
        ("schema_version", "-1"),
        ("schema_version", "4294967296"),
        ("schema_version", "1.5"),
        ("schema_version", "'bad'"),
        ("schema_version", "X'01'"),
        ("revision", "0"),
        ("revision", "-1"),
        ("revision", "1.5"),
        ("revision", "'bad'"),
        ("revision", "X'01'"),
        ("created_at", "'bad'"),
        ("updated_at", "'bad'"),
        ("created_at", "1.5"),
        ("updated_at", "X'01'"),
        ("data", "'broken'"),
        ("data", "1.5"),
        ("data", "X'01'"),
        ("updated_by", "'{\"kind\":\"unknown\"}'"),
        ("updated_by", "'broken'"),
        ("updated_by", "1.5"),
        ("updated_by", "X'01'"),
        ("id", "'bad'"),
        ("id", "1.5"),
        ("id", "X'01'"),
        ("id", "CAST(X'FF' AS TEXT)"),
        ("data", "CAST(X'FF' AS TEXT)"),
        ("updated_by", "CAST(X'FF' AS TEXT)"),
    ];
    for (column, value) in cases {
        let bad = EntityId::new_v7();
        seed_document(&worker, "view_state", bad, 3, json!({"secret":"local"})).await;
        raw(&worker.handle, move |c| {
            c.config
                .execute(
                    &format!("UPDATE documents SET {column}={value} WHERE id=?1"),
                    [bad.to_string()],
                )
                .unwrap();
        })
        .await;
    }
    let before = raw_snapshot(&worker).await;
    let reports = docs.inspect_rows("view_state".into()).await.unwrap();
    assert_eq!(reports.len(), cases.len() + 1);
    assert_eq!(reports.iter().filter(|r| r.is_ok()).count(), 1);
    // Compare every diagnostic ID/body to the original SQLite bytes, including invalid UTF-8.
    let bytes = |value: SqlValue| match value {
        SqlValue::Text(text) => SqlValue::Blob(text.into_bytes()),
        other => other,
    };
    let expected_raw: Vec<_> = before
        .iter()
        .filter(|row| row[1].1 != SqlValue::Blob(good.to_string().into_bytes()))
        .map(|row| (row[1].1.clone(), row[4].1.clone()))
        .collect();
    let actual_raw: Vec<_> = reports
        .iter()
        .filter_map(|report| report.as_ref().err())
        .map(|invalid| (bytes(invalid.id.clone()), bytes(invalid.data.clone())))
        .collect();
    assert_eq!(actual_raw, expected_raw);
    let mut json_errors = 0;
    let mut class_errors = 0;
    for report in reports {
        match report {
            Ok(record) => same_record(&record, &expected_good),
            Err(Invalid {
                kind,
                error,
                id,
                data,
            }) => {
                assert_eq!(kind, "view_state");
                assert_ne!(id, SqlValue::Text(good.to_string()));
                assert_ne!(data, SqlValue::Null);
                match error {
                    DocumentError::Json(e) => {
                        assert!(e.line() > 0);
                        json_errors += 1;
                    }
                    DocumentError::Sqlite(_) => class_errors += 1,
                    DocumentError::InvalidStored(_) => (),
                    other => panic!("unexpected local error: {other:?}"),
                }
            }
        }
    }
    assert_eq!(json_errors, 3);
    assert_eq!(class_errors, 19);
    assert_eq!(raw_snapshot(&worker).await, before);
    assert!(
        docs.list("view_state".into(), Default::default())
            .await
            .is_err()
    );
    assert!(docs.inspect_rows("queue".into()).await.unwrap().is_empty());
    raw(&worker.handle, |c| {
        c.config.execute_batch("DROP TABLE documents").unwrap()
    })
    .await;
    assert!(matches!(docs.inspect_rows("view_state".into()).await,
        Err(DocumentError::Sqlite(rusqlite::Error::SqliteFailure(_,Some(message)))) if message.contains("no such table")));
    worker.close().await.unwrap();
    assert!(matches!(
        docs.inspect_rows("view_state".into()).await,
        Err(DocumentError::Transport(StorageError::Disconnected))
    ));
    assert!(matches!(
        docs.inspect_rows("plugin_data".into()).await,
        Err(DocumentError::InvalidInput(_))
    ));
}

#[tokio::test]
async fn load_report_rejects_all_uuid_aliases_without_repair_or_deduplication() {
    let (_temp, worker, docs) = harness().await;
    let id: EntityId = "abcdef01-2345-6789-abcd-ef0123456789".parse().unwrap();
    seed_document(&worker, "view_state", id, 3, json!({"canonical":true})).await;
    let text = id.to_string();
    for alias in [
        text.to_uppercase(),
        format!("{{{text}}}"),
        text.replace('-', ""),
        format!("urn:uuid:{text}"),
    ] {
        let stored = alias.clone();
        raw(&worker.handle, move |c| {
            c.config.execute("INSERT INTO documents SELECT kind,?1,schema_version,revision,name,name_key,enabled,data,created_at,updated_at,updated_by FROM documents WHERE id=?2",
                params![stored,id.to_string()]).unwrap();
        }).await;
    }
    let before = raw_snapshot(&worker).await;
    let reports = docs.inspect_rows("view_state".into()).await.unwrap();
    assert_eq!(reports.len(), 5);
    assert_eq!(reports.iter().filter(|r| r.is_ok()).count(), 1);
    for invalid in reports.into_iter().filter_map(|r| r.err()) {
        assert!(matches!(
            invalid.error,
            DocumentError::InvalidStored("invalid listed id")
        ));
        assert_ne!(invalid.id, SqlValue::Text(text.clone()));
    }
    assert!(matches!(
        docs.list("view_state".into(), Default::default()).await,
        Err(DocumentError::InvalidStored("invalid listed id"))
    ));
    assert_eq!(
        docs.get("view_state".into(), id)
            .await
            .unwrap()
            .unwrap()
            .document
            .data,
        json!({"canonical":true})
    );
    assert_eq!(raw_snapshot(&worker).await, before);
    worker.close().await.unwrap();
}

use den_core::storage::documents::{DocumentSchema, Registry};
use std::collections::BTreeMap;

fn schema_registry(migrations: BTreeMap<u32, den_core::storage::documents::Migration>) -> Registry {
    Registry::new(BTreeMap::from([(
        "view_state".into(),
        DocumentSchema {
            version: 3,
            migrations,
            validate: fixture_validator,
        },
    )]))
    .unwrap()
}
fn fixture_validator(data: &Value) -> Result<(), String> {
    if data.get("invalid").is_some() {
        Err("fixture validation failed".into())
    } else {
        Ok(())
    }
}
fn fixture_step_one(mut data: Value) -> Result<Value, String> {
    let old = data
        .as_object_mut()
        .ok_or("fixture requires object")?
        .remove("old")
        .ok_or("missing old")?;
    data["new"] = old;
    Ok(data)
}
fn fixture_step_two(mut data: Value) -> Result<Value, String> {
    data["step2"] = json!(true);
    Ok(data)
}
fn full_registry() -> Registry {
    schema_registry(BTreeMap::from([
        (
            1,
            fixture_step_one as den_core::storage::documents::Migration,
        ),
        (
            2,
            fixture_step_two as den_core::storage::documents::Migration,
        ),
    ]))
}

#[test]
fn supplied_registry_checks_versions_and_migration_edges() {
    for (kind, version) in [
        ("action", StoredDocument::ACTION_SCHEMA_VERSION),
        ("queue", StoredDocument::QUEUE_SCHEMA_VERSION),
        ("command", StoredDocument::COMMAND_SCHEMA_VERSION),
        ("timer", StoredDocument::TIMER_SCHEMA_VERSION),
        (
            "pending_change",
            StoredDocument::PENDING_CHANGE_SCHEMA_VERSION,
        ),
    ] {
        let schema = DocumentSchema {
            version,
            migrations: BTreeMap::new(),
            validate: fixture_validator,
        };
        assert!(Registry::new(BTreeMap::from([(kind.into(), schema.clone())])).is_ok());
        assert!(
            Registry::new(BTreeMap::from([(
                kind.into(),
                DocumentSchema {
                    version: version + 1,
                    ..schema
                }
            )]))
            .is_err()
        );
    }
    for (kind, version, from) in [
        ("view_state", 0, 0),
        ("view_state", 3, 3),
        ("view_state", 3, 4),
        ("plugin_data", 3, 1),
    ] {
        assert!(
            Registry::new(BTreeMap::from([(
                kind.into(),
                DocumentSchema {
                    version,
                    migrations: BTreeMap::from([(
                        from,
                        fixture_step_one as den_core::storage::documents::Migration
                    )]),
                    validate: fixture_validator
                }
            )]))
            .is_err()
        );
    }
}

fn panicking_step(data: Value) -> Result<Value, String> {
    if data.get("panic").is_some() {
        panic!("fixture migration panic")
    }
    fixture_step_one(data)
}

fn panicking_validator(data: &Value) -> Result<(), String> {
    if data.get("panic").is_some() {
        panic!("fixture validator panic")
    }
    fixture_validator(data)
}

#[tokio::test]
async fn supplied_registry_pure_conversion_preserves_metadata_and_contains_all_failures() {
    let (_temp, worker, docs) = harness().await;
    let id = EntityId::new_v7();
    seed_document(
        &worker,
        "view_state",
        id,
        1,
        json!({"old":7,"x-migrated":{"kept":true},"unmapped":[null]}),
    )
    .await;
    let original = docs.get("view_state".into(), id).await.unwrap().unwrap();
    let snapshot = raw_snapshot(&worker).await;
    let converted = full_registry().migrate(original.clone()).unwrap();
    assert_eq!(converted.document.schema_version, 3);
    assert_eq!(
        converted.document.data,
        json!({"new":7,"step2":true,"x-migrated":{"kept":true},"unmapped":[null]})
    );
    assert_eq!(converted.document.kind, original.document.kind);
    assert_eq!(converted.id, original.id);
    assert_eq!(converted.revision, original.revision);
    assert_eq!(converted.created_at, original.created_at);
    assert_eq!(converted.updated_at, original.updated_at);
    assert_eq!(converted.updated_by, original.updated_by);
    assert!(matches!(
        schema_registry(BTreeMap::new()).migrate(original.clone()),
        Err(DocumentError::InvalidStored("missing document migration"))
    ));
    assert!(matches!(
        Registry::new(BTreeMap::new())
            .unwrap()
            .migrate(original.clone()),
        Err(DocumentError::InvalidInput(_))
    ));
    let mut invalid = original.clone();
    invalid.document.schema_version = 3;
    invalid.document.data = json!({"invalid":true});
    assert!(matches!(
        full_registry().migrate(invalid),
        Err(DocumentError::Schema(_))
    ));
    for (version, stage) in [(1, "migration"), (3, "validator")] {
        let registry = Registry::new(BTreeMap::from([(
            "view_state".into(),
            DocumentSchema {
                version: 3,
                migrations: BTreeMap::from([
                    (1, panicking_step as den_core::storage::documents::Migration),
                    (2, fixture_step_two),
                ]),
                validate: panicking_validator,
            },
        )]))
        .unwrap();
        let mut record = original.clone();
        record.document.schema_version = version;
        record.document.data = json!({"panic":true});
        assert!(
            matches!(registry.migrate(record), Err(DocumentError::CallbackPanicked(actual)) if actual == stage)
        );
        let mut newer = original.clone();
        newer.document.schema_version = 99;
        newer.document.data = json!({"panic":true});
        assert!(matches!(
            registry.migrate(newer),
            Err(DocumentError::UnsupportedVersion {
                stored: 99,
                supported: 3,
                ..
            })
        ));
        assert_eq!(
            registry.migrate(original.clone()).unwrap().document.data,
            converted.document.data
        );
        assert!(docs.get("view_state".into(), id).await.unwrap().is_some());
    }
    let mut missing = original.clone();
    missing.document.data = json!({"no-old":true});
    assert!(
        matches!(full_registry().migrate(missing), Err(DocumentError::Schema(message)) if message == "missing old")
    );
    let first = fixture_step_one(original.document.data.clone()).unwrap();
    assert_eq!(
        first,
        json!({"new":7,"x-migrated":{"kept":true},"unmapped":[null]})
    );
    assert_eq!(fixture_step_two(first).unwrap(), converted.document.data);
    assert_eq!(
        full_registry()
            .migrate(converted.clone())
            .unwrap()
            .document
            .data,
        converted.document.data
    );
    let partial = schema_registry(BTreeMap::from([(
        1,
        fixture_step_one as den_core::storage::documents::Migration,
    )]));
    assert!(matches!(
        partial.migrate(original.clone()),
        Err(DocumentError::InvalidStored("missing document migration"))
    ));
    let mut invalid_after_steps = original.clone();
    invalid_after_steps.document.data["invalid"] = json!(true);
    assert!(
        matches!(full_registry().migrate(invalid_after_steps), Err(DocumentError::Schema(message)) if message == "fixture validation failed")
    );
    assert_eq!(raw_snapshot(&worker).await, snapshot);
    worker.close().await.unwrap();
}

use den_core::storage::documents::Loaded;

#[tokio::test]
async fn invalid_document_reported_not_fatal() {
    let (_temp, worker, docs) = harness().await;
    let bad = EntityId::new_v7();
    let good = EntityId::new_v7();
    seed_document(
        &worker,
        "view_state",
        bad,
        3,
        json!({"invalid":true,"enabled":true}),
    )
    .await;
    seed_document(&worker, "view_state", good, 3, json!({"enabled":true})).await;
    let before = raw_snapshot(&worker).await;
    let reports = docs
        .load("view_state".into(), &full_registry())
        .await
        .unwrap();
    assert_eq!(reports.len(), 2);
    let mut invalid_count = 0;
    for report in reports {
        match report {
            Loaded::Valid(record) => assert_eq!(record.id, good),
            Loaded::Invalid(Invalid {
                id,
                kind,
                error,
                data,
            }) => {
                invalid_count += 1;
                assert_eq!(id, SqlValue::Text(bad.to_string()));
                assert_eq!(kind, "view_state");
                assert!(
                    matches!(error,DocumentError::Schema(message) if message=="fixture validation failed")
                );
                let SqlValue::Text(body) = data else {
                    panic!("JSON diagnostic")
                };
                assert_eq!(
                    serde_json::from_str::<Value>(&body).unwrap(),
                    json!({"invalid":true,"enabled":true})
                );
            }
            _ => panic!("unexpected newer fixture"),
        }
    }
    assert_eq!(invalid_count, 1);
    assert_eq!(raw_snapshot(&worker).await, before);
    let malformed = EntityId::new_v7();
    seed_document(&worker, "view_state", malformed, 3, json!({})).await;
    raw(&worker.handle, move |c| {
        c.config
            .pragma_update(None, "ignore_check_constraints", true)
            .unwrap();
        c.config
            .execute(
                "UPDATE documents SET data='broken JSON' WHERE id=?1",
                [malformed.to_string()],
            )
            .unwrap();
        c.config
            .pragma_update(None, "ignore_check_constraints", false)
            .unwrap();
    })
    .await;
    let snapshot = raw_snapshot(&worker).await;
    let reports = docs
        .load("view_state".into(), &full_registry())
        .await
        .unwrap();
    assert_eq!(reports.len(), 3);
    assert!(reports.iter().any(|report| {
        matches!(report, Loaded::Invalid(Invalid { id, error: DocumentError::Json(_), data, .. })
            if *id == SqlValue::Text(malformed.to_string()) && *data == SqlValue::Text("broken JSON".into()))
    }));
    assert_eq!(raw_snapshot(&worker).await, snapshot);
    worker.close().await.unwrap();
}

#[tokio::test]
async fn newer_versions_and_missing_migrations_are_isolated() {
    let (_temp, worker, docs) = harness().await;
    let old = EntityId::new_v7();
    let newer = EntityId::new_v7();
    let current = EntityId::new_v7();
    seed_document(&worker, "view_state", old, 1, json!({"old":7})).await;
    seed_document(
        &worker,
        "view_state",
        newer,
        99,
        json!({"invalid":true,"future":[1]}),
    )
    .await;
    seed_document(&worker, "view_state", current, 3, json!({"ok":true})).await;
    let before = raw_snapshot(&worker).await;
    let registry = schema_registry(BTreeMap::from([(
        1,
        fixture_step_one as den_core::storage::documents::Migration,
    )]));
    let reports = docs.load("view_state".into(), &registry).await.unwrap();
    assert_eq!(reports.len(), 3);
    assert_eq!(
        reports
            .iter()
            .filter(|r| matches!(r, Loaded::Valid(_)))
            .count(),
        1
    );
    assert!(reports.iter().any(|report| {
        matches!(report, Loaded::Newer(record)
            if record.id == newer && record.document.schema_version == 99
                && record.document.data["future"] == json!([1]))
    }));
    assert!(reports.iter().any(|report| {
        matches!(report, Loaded::Invalid(Invalid {
            id, error: DocumentError::InvalidStored("missing document migration"), ..
        }) if *id == SqlValue::Text(old.to_string()))
    }));
    assert_eq!(raw_snapshot(&worker).await, before);
    let unregistered = Registry::new(BTreeMap::new()).unwrap();
    assert!(matches!(
        docs.load("view_state".into(), &unregistered).await,
        Err(DocumentError::InvalidInput(
            "document version not registered"
        ))
    ));
    worker.close().await.unwrap();
}

#[tokio::test]
async fn lazy_load_chains_golden_steps_without_persisting_or_losing_fields() {
    let (_temp, worker, docs) = harness().await;
    let id = EntityId::new_v7();
    let before_body =
        json!({"old":7,"unmapped":{"nested":[null,"x"]},"x-migrated":{"legacy":true}});
    let after_one = json!({"new":7,"unmapped":{"nested":[null,"x"]},"x-migrated":{"legacy":true}});
    let after_two =
        json!({"new":7,"step2":true,"unmapped":{"nested":[null,"x"]},"x-migrated":{"legacy":true}});
    assert_eq!(fixture_step_one(before_body.clone()).unwrap(), after_one);
    assert_eq!(fixture_step_two(after_one).unwrap(), after_two);
    seed_document(&worker, "view_state", id, 1, before_body).await;
    let original = docs.get("view_state".into(), id).await.unwrap().unwrap();
    let original_list = docs
        .list("view_state".into(), Filter::default())
        .await
        .unwrap();
    let snapshot = raw_snapshot(&worker).await;
    for _ in 0..2 {
        let reports = docs
            .load("view_state".into(), &full_registry())
            .await
            .unwrap();
        let [Loaded::Valid(record)] = reports.as_slice() else {
            panic!("valid migrated row")
        };
        assert_eq!(record.document.schema_version, 3);
        assert_eq!(record.document.data, after_two);
        assert_eq!(record.id, original.id);
        assert_eq!(record.revision, original.revision);
        assert_eq!(record.created_at, original.created_at);
        assert_eq!(record.updated_at, original.updated_at);
        assert_eq!(record.updated_by, original.updated_by);
    }
    assert_eq!(raw_snapshot(&worker).await, snapshot);
    assert_eq!(
        format!(
            "{:?}",
            docs.get("view_state".into(), id).await.unwrap().unwrap()
        ),
        format!("{original:?}")
    );
    assert_eq!(
        format!(
            "{:?}",
            docs.list("view_state".into(), Filter::default())
                .await
                .unwrap()
        ),
        format!("{original_list:?}")
    );
    worker.close().await.unwrap();
}

#[tokio::test]
async fn callback_panics_are_row_local_and_caller_worker_and_neighbors_recover() {
    let (_temp, worker, docs) = harness().await;
    let bad = EntityId::new_v7();
    let good = EntityId::new_v7();
    let newer = EntityId::new_v7();
    for (version, callback) in [(1, "migration"), (3, "validator")] {
        seed_document(&worker, "view_state", bad, version, json!({"panic":true})).await;
        seed_document(&worker, "view_state", good, 3, json!({"ok":true})).await;
        seed_document(&worker, "view_state", newer, 99, json!({"panic":true})).await;
        let registry = Registry::new(BTreeMap::from([(
            "view_state".into(),
            DocumentSchema {
                version: 3,
                migrations: BTreeMap::from([
                    (1, panicking_step as den_core::storage::documents::Migration),
                    (2, fixture_step_two),
                ]),
                validate: panicking_validator,
            },
        )]))
        .unwrap();
        let snapshot = raw_snapshot(&worker).await;
        for _ in 0..2 {
            let reports = docs.load("view_state".into(), &registry).await.unwrap();
            assert_eq!(reports.len(), 3);
            assert!(reports.iter().any(|report| {
                matches!(report, Loaded::Invalid(Invalid {
                    id, error: DocumentError::CallbackPanicked(stage), ..
                }) if *id == SqlValue::Text(bad.to_string()) && *stage == callback)
            }));
            assert!(reports.iter().any(|report| {
                matches!(report, Loaded::Newer(record) if record.id == newer
                    && record.document.schema_version == 99 && record.document.data == json!({"panic":true}))
            }));
            assert!(
                reports
                    .iter()
                    .any(|r| matches!(r, Loaded::Valid(record) if record.id == good))
            );
            assert!(docs.get("view_state".into(), good).await.unwrap().is_some());
        }
        assert_eq!(raw_snapshot(&worker).await, snapshot);
        raw(&worker.handle, |c| {
            c.config.execute("DELETE FROM documents", []).unwrap();
        })
        .await;
    }
    worker.close().await.unwrap();
}

#[tokio::test]
async fn registry_preflight_precedes_rows_and_query_and_transport_failures_stay_call_level() {
    let (_temp, worker, docs) = harness().await;
    let empty = Registry::new(BTreeMap::new()).unwrap();
    assert!(
        docs.load("view_state".into(), &full_registry())
            .await
            .unwrap()
            .is_empty()
    );
    assert!(matches!(
        docs.load("view_state".into(), &empty).await,
        Err(DocumentError::InvalidInput(_))
    ));
    raw(&worker.handle, |c| {
        c.config.execute("DROP TABLE documents", []).unwrap();
    })
    .await;
    assert!(matches!(
        docs.load("view_state".into(), &empty).await,
        Err(DocumentError::InvalidInput(_))
    ));
    assert!(matches!(
        docs.load("view_state".into(), &full_registry()).await,
        Err(DocumentError::Sqlite(_))
    ));
    worker.close().await.unwrap();
    assert!(matches!(
        docs.load("view_state".into(), &empty).await,
        Err(DocumentError::InvalidInput(_))
    ));
    assert!(matches!(
        docs.load("view_state".into(), &full_registry()).await,
        Err(DocumentError::Transport(_))
    ));
}

#[tokio::test]
async fn failed_migration_retains_original_diagnostic_and_valid_neighbor() {
    let (_temp, worker, docs) = harness().await;
    let bad = EntityId::new_v7();
    let good = EntityId::new_v7();
    let body = json!({"unmapped":[1,null],"no-old-field":true});
    seed_document(&worker, "view_state", bad, 1, body.clone()).await;
    seed_document(&worker, "view_state", good, 3, json!({"ok":true})).await;
    let snapshot = raw_snapshot(&worker).await;
    let reports = docs
        .load("view_state".into(), &full_registry())
        .await
        .unwrap();
    assert_eq!(reports.len(), 2);
    assert!(reports.iter().any(|report| {
        matches!(report, Loaded::Invalid(Invalid {
            id, error: DocumentError::Schema(message), data: SqlValue::Text(data), ..
        }) if *id == SqlValue::Text(bad.to_string()) && message == "missing old"
            && serde_json::from_str::<Value>(data).unwrap() == body)
    }));
    assert!(
        reports
            .iter()
            .any(|r| matches!(r, Loaded::Valid(record) if record.id == good))
    );
    assert_eq!(raw_snapshot(&worker).await, snapshot);
    worker.close().await.unwrap();
}

#[tokio::test]
async fn loader_failures_retain_raw_json_bytes_and_public_inspection_behavior() {
    let (_temp, worker, docs) = harness().await;
    let bad = EntityId::new_v7();
    let good = EntityId::new_v7();
    // Raw SQLite text, deliberately not serialized from Value: every lexeme must survive.
    let body = " { \"z\": 1, \"invalid\": true, \"panic\": true, \"big\": 123456789012345678901234567890, \"f\": 1.50e+02, \"dup\": 1, \"dup\": 2 }\n";
    seed_document(&worker, "view_state", good, 3, json!({"ok":true})).await;
    seed_document(&worker, "view_state", bad, 1, json!({})).await;
    for failure in [
        "migration",
        "validator",
        "missing",
        "migration_panic",
        "validator_panic",
    ] {
        let version = if failure.starts_with("validator") {
            3
        } else {
            1
        };
        raw(&worker.handle, move |c| {
            c.config
                .execute(
                    "UPDATE documents SET data=?1,schema_version=?2 WHERE kind='view_state' AND id=?3",
                    params![body, version, bad.to_string()],
                )
                .unwrap();
        })
        .await;
        let step: den_core::storage::documents::Migration = if failure == "migration_panic" {
            panicking_step
        } else {
            fixture_step_one
        };
        let registry = Registry::new(BTreeMap::from([(
            "view_state".into(),
            DocumentSchema {
                version: 3,
                migrations: if failure == "missing" {
                    BTreeMap::new()
                } else {
                    BTreeMap::from([(1, step), (2, fixture_step_two)])
                },
                validate: if failure == "validator_panic" {
                    panicking_validator
                } else {
                    fixture_validator
                },
            },
        )]))
        .unwrap();
        let snapshot = raw_snapshot(&worker).await;
        let original = docs.get("view_state".into(), bad).await.unwrap().unwrap();
        let listed = format!(
            "{:?}",
            docs.list("view_state".into(), Filter::default())
                .await
                .unwrap()
        );
        let inspected = format!(
            "{:?}",
            docs.inspect_rows("view_state".into()).await.unwrap()
        );
        for _ in 0..2 {
            let reports = docs.load("view_state".into(), &registry).await.unwrap();
            assert_eq!(reports.len(), 2);
            assert!(
                reports
                    .iter()
                    .any(|report| matches!(report, Loaded::Valid(record) if record.id == good))
            );
            let invalid = reports
                .iter()
                .find_map(|report| match report {
                    Loaded::Invalid(invalid) => Some(invalid),
                    _ => None,
                })
                .unwrap();
            assert_eq!(invalid.id, SqlValue::Text(bad.to_string()));
            assert_eq!(invalid.kind, "view_state");
            assert_eq!(invalid.data, SqlValue::Text(body.into()));
            match (&invalid.error, failure) {
                (DocumentError::Schema(message), "migration") => assert_eq!(message, "missing old"),
                (DocumentError::Schema(message), "validator") => {
                    assert_eq!(message, "fixture validation failed")
                }
                (DocumentError::InvalidStored(message), "missing") => {
                    assert_eq!(*message, "missing document migration")
                }
                (DocumentError::CallbackPanicked(stage), "migration_panic") => {
                    assert_eq!(*stage, "migration")
                }
                (DocumentError::CallbackPanicked(stage), "validator_panic") => {
                    assert_eq!(*stage, "validator")
                }
                other => panic!("unexpected failure: {other:?}"),
            }
            same_record(
                &docs.get("view_state".into(), bad).await.unwrap().unwrap(),
                &original,
            );
            assert!(docs.get("view_state".into(), good).await.unwrap().is_some());
            assert_eq!(
                format!(
                    "{:?}",
                    docs.list("view_state".into(), Filter::default())
                        .await
                        .unwrap()
                ),
                listed
            );
            assert_eq!(
                format!(
                    "{:?}",
                    docs.inspect_rows("view_state".into()).await.unwrap()
                ),
                inspected
            );
            assert_eq!(raw_snapshot(&worker).await, snapshot);
        }
    }
    worker.close().await.unwrap();
}
