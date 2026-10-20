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
    storage::documents::{Put, WritePolicy},
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
