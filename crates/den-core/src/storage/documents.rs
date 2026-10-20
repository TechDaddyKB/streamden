//! Internal, UNFROZEN storage seam for Spec 03 §13.2; no serialized or wire contract.
//! Reads decode persisted fields only. They do not migrate, validate, disable or install
//! documents. DEN-302 adds those load semantics before consumers may use this as a loader.
//! Writes accept only registered current versions; older rows await DEN-302 migrations.
//! Input attribution is server-supplied; this leaf does not authorize callers or assign Origin.
use super::thread::{Admission, JobClass, StorageConns, StorageError, StorageHandle};
use crate::clock::Clock;
use den_protocol::{EntityId, StoredDocument, Timestamp, UpdatedBy, rpc::ErrorCode};
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
use serde_json::Value;
use std::{collections::BTreeMap, sync::Arc};
use unicode_casefold::UnicodeCaseFold;

pub type Result<T> = std::result::Result<T, DocumentError>;

/// Rust-only diagnostics. Raw errors are retained locally, never serialized to RPC clients.
#[derive(Debug)]
pub enum DocumentError {
    Transport(StorageError),
    Sqlite(rusqlite::Error),
    Json(serde_json::Error),
    InvalidInput(&'static str),
    InvalidStored(&'static str),
    Conflict {
        id: EntityId,
    },
    UnsupportedVersion {
        id: EntityId,
        stored: u32,
        supported: u32,
    },
}
impl DocumentError {
    /// Existing protocol code only; callers must not expose raw diagnostics on the wire.
    pub fn code(&self) -> ErrorCode {
        ErrorCode::new(match self {
            Self::InvalidInput(_) => ErrorCode::INVALID_PARAMS,
            Self::Conflict { .. } => ErrorCode::CONFLICT,
            Self::UnsupportedVersion { .. } => ErrorCode::INVALID_STATE,
            _ => ErrorCode::INTERNAL,
        })
    }
}
impl From<rusqlite::Error> for DocumentError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Sqlite(error)
    }
}
impl From<serde_json::Error> for DocumentError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

/// A persisted row, without any claim of document-schema validity or version support.
#[derive(Debug, Clone)]
pub struct Record {
    pub id: EntityId,
    pub document: StoredDocument,
    pub revision: u64,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    pub updated_by: UpdatedBy,
}

/// Internal UNFROZEN selection, not an operation's filter or a serialized contract.
/// Both predicates are AND-ed; None omits a predicate. Some excludes SQL NULL.
/// Name equality uses full Unicode case-folding, without trimming or normalization.
#[derive(Clone, Debug, Default)]
pub struct Filter {
    pub name: Option<String>,
    pub enabled: Option<bool>,
}

#[derive(Clone)]
pub struct Documents {
    handle: StorageHandle,
}

/// Typed leaf adapter: admission waits for capacity; SQL runs on the dedicated worker.
/// Abandoning a reply does not cancel an admitted job (StorageHandle's MustLand semantics).
pub(super) async fn run<T: Send + 'static>(
    handle: &StorageHandle,
    job: impl FnOnce(&mut StorageConns) -> Result<T> + Send + 'static,
) -> Result<T> {
    match handle
        .submit(JobClass::MustLand, job)
        .await
        .map_err(DocumentError::Transport)?
    {
        Admission::Accepted(reply) => reply.wait().await.map_err(DocumentError::Transport)?,
        Admission::Dropped => unreachable!("MustLand never drops"),
    }
}

impl Documents {
    pub fn new(handle: StorageHandle) -> Self {
        Self { handle }
    }

    /// Raw persisted rows in ascending binary ID order, including drafts and unknown versions.
    /// One read snapshot covers selection and decoding. Like get(), this does not validate,
    /// migrate or install documents; malformed stored fields fail with local typed errors.
    pub async fn list(&self, kind: String, filter: Filter) -> Result<Vec<Record>> {
        check_kind(&kind)?;
        run(&self.handle, move |conns| {
            let tx = conns.config.transaction()?;
            let key = filter
                .name
                .as_deref()
                .map(|name| name.case_fold().collect::<String>());
            let ids = {
                let mut statement = tx.prepare(
                    "SELECT id FROM documents WHERE kind=?1 AND (?2 IS NULL OR name_key=?2)
                     AND (?3 IS NULL OR enabled=?3) ORDER BY id COLLATE BINARY",
                )?;
                statement
                    .query_map(params![kind, key, filter.enabled], |row| {
                        row.get::<_, String>(0)
                    })?
                    .collect::<rusqlite::Result<Vec<_>>>()?
            };
            let records = ids
                .into_iter()
                .map(|stored| {
                    // read() looks up the canonical spelling; an alias could select another row.
                    let id = stored
                        .parse::<EntityId>()
                        .ok()
                        .filter(|id| id.to_string() == stored)
                        .ok_or(DocumentError::InvalidStored("invalid listed id"))?;
                    read(&tx, &kind, id)?
                        .ok_or(DocumentError::InvalidStored("missing listed document"))
                })
                .collect::<Result<Vec<_>>>()?;
            tx.commit()?;
            Ok(records)
        })
        .await
    }

    /// Fetch exactly (kind, id), preserving body, stored version and attribution unchanged.
    /// Missing rows return None. This UNFROZEN API does not imply validation or migration.
    pub async fn get(&self, kind: String, id: EntityId) -> Result<Option<Record>> {
        if !matches!(
            kind.as_str(),
            "action"
                | "queue"
                | "command"
                | "timer"
                | "settings"
                | "view_state"
                | "pending_change"
                | "export_meta"
        ) {
            return Err(DocumentError::InvalidInput(
                "unsupported engine document kind",
            ));
        }
        run(&self.handle, move |conns| read(&conns.config, &kind, id)).await
    }
}

fn read(conn: &Connection, kind: &str, id: EntityId) -> Result<Option<Record>> {
    let raw = conn
        .query_row(
            "SELECT schema_version,revision,data,created_at,updated_at,updated_by
             FROM documents WHERE kind=?1 AND id=?2",
            params![kind, id.to_string()],
            |row| {
                Ok((
                    row.get::<_, u32>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                ))
            },
        )
        .optional()?;
    raw.map(|(schema_version, revision, data, created, updated, by)| {
        Ok(Record {
            id,
            document: StoredDocument {
                kind: kind.into(),
                schema_version,
                data: serde_json::from_str(&data)?,
            },
            revision: u64::try_from(revision)
                .ok()
                .filter(|revision| *revision > 0)
                .ok_or(DocumentError::InvalidStored("invalid stored revision"))?,
            created_at: Timestamp::parse(&created)
                .map_err(|_| DocumentError::InvalidStored("invalid stored timestamp"))?,
            updated_at: Timestamp::parse(&updated)
                .map_err(|_| DocumentError::InvalidStored("invalid stored timestamp"))?,
            updated_by: serde_json::from_str(&by)?,
        })
    })
    .transpose()
}

/// Server-attributed input only: feature handlers own Origin and UpdatedBy policy, not storage.
#[derive(Clone, Debug)]
pub struct Put {
    pub id: EntityId,
    pub document: StoredDocument,
    pub expected_revision: Option<u64>,
    pub updated_by: UpdatedBy,
}

/// Owner-supplied versions. No default versions for feature-owned kinds, no load migrations.
#[derive(Clone)]
pub struct WritePolicy {
    versions: BTreeMap<String, u32>,
}
impl WritePolicy {
    pub fn new(versions: BTreeMap<String, u32>) -> Result<Self> {
        for (kind, version) in &versions {
            check_kind(kind)?;
            let contracted = match kind.as_str() {
                "action" => Some(StoredDocument::ACTION_SCHEMA_VERSION),
                "queue" => Some(StoredDocument::QUEUE_SCHEMA_VERSION),
                "command" => Some(StoredDocument::COMMAND_SCHEMA_VERSION),
                "timer" => Some(StoredDocument::TIMER_SCHEMA_VERSION),
                "pending_change" => Some(StoredDocument::PENDING_CHANGE_SCHEMA_VERSION),
                _ => None,
            };
            if *version == 0 || contracted.is_some_and(|current| current != *version) {
                return Err(DocumentError::InvalidInput("invalid registered version"));
            }
        }
        Ok(Self { versions })
    }
    fn version(&self, kind: &str) -> Result<u32> {
        check_kind(kind)?;
        self.versions
            .get(kind)
            .copied()
            .ok_or(DocumentError::InvalidInput(
                "document version not registered",
            ))
    }
    pub fn check_existing(&self, record: &Record) -> Result<()> {
        let supported = self.version(&record.document.kind)?;
        if record.document.schema_version != supported {
            // Older data must pass future DEN-302 migrations before replacement; newer data is read-only.
            return Err(DocumentError::UnsupportedVersion {
                id: record.id,
                stored: record.document.schema_version,
                supported,
            });
        }
        Ok(())
    }
}
fn check_kind(kind: &str) -> Result<()> {
    if matches!(
        kind,
        "action"
            | "queue"
            | "command"
            | "timer"
            | "settings"
            | "view_state"
            | "pending_change"
            | "export_meta"
    ) {
        Ok(())
    } else {
        Err(DocumentError::InvalidInput(
            "unsupported engine document kind",
        ))
    }
}
fn check_revision(id: EntityId, expected: Option<u64>, record: Option<&Record>) -> Result<()> {
    if expected.is_some_and(|expected| Some(expected) != record.map(|r| r.revision)) {
        return Err(DocumentError::Conflict { id });
    }
    Ok(())
}
/// Pure prepared values; future SQL writers must never accept these from callers.
#[derive(Debug)]
pub struct Prepared {
    pub record: Record,
    pub name: Option<String>,
    pub key: Option<String>,
    pub enabled: Option<bool>,
    pub draft: bool,
}
impl WritePolicy {
    /// Pure preparation only. Future writers must recheck the row under their write transaction.
    pub fn prepare(&self, put: Put, old: Option<&Record>, now: Timestamp) -> Result<Prepared> {
        let version = self.version(&put.document.kind)?;
        if put.document.schema_version != version {
            return Err(DocumentError::InvalidInput(
                "input version must match registered version",
            ));
        }
        if old.is_some_and(|old| old.id != put.id || old.document.kind != put.document.kind) {
            return Err(DocumentError::InvalidInput(
                "previous document identity mismatch",
            ));
        }
        check_revision(put.id, put.expected_revision, old)?;
        if let Some(old) = old {
            self.check_existing(old)?;
        }
        let revision = old.map_or(Ok(1), |old| {
            old.revision
                .checked_add(1)
                .filter(|n| *n <= i64::MAX as u64)
                .ok_or(DocumentError::InvalidStored("revision overflow"))
        })?;
        let mut data = put.document.data.clone();
        let body = data.as_object_mut().ok_or(DocumentError::InvalidInput(
            "document body must be an object",
        ))?;
        if let Some(id) = body.get("id") {
            let id = serde_json::from_value::<EntityId>(id.clone())
                .map_err(|_| DocumentError::InvalidInput("document id must be an entity id"))?;
            if id != put.id {
                return Err(DocumentError::InvalidInput("document id mismatch"));
            }
        }
        let action = put.document.kind == "action";
        if matches!(put.document.kind.as_str(), "action" | "command" | "timer") {
            body.insert("revision".into(), revision.into());
        }
        let draft =
            action && data.pointer("/origin/kind").and_then(Value::as_str) == Some("ai_draft");
        let named = matches!(put.document.kind.as_str(), "action" | "queue" | "command");
        let name = match data.get("name") {
            Some(Value::String(name)) => Some(name.clone()),
            None if !named => None,
            _ => return Err(DocumentError::InvalidInput("name must be a string")),
        };
        let enabled = match data.get("enabled") {
            Some(Value::Bool(enabled)) => Some(*enabled),
            None => None,
            _ => return Err(DocumentError::InvalidInput("enabled must be a boolean")),
        };
        if draft {
            data["enabled"] = false.into();
        }
        let enabled = if draft { Some(false) } else { enabled };
        let key = name.as_deref().map(|name| name.case_fold().collect());
        let record = Record {
            id: put.id,
            document: StoredDocument {
                kind: put.document.kind,
                schema_version: version,
                data,
            },
            revision,
            created_at: old.map_or(now, |old| old.created_at),
            updated_at: now,
            updated_by: put.updated_by,
        };
        Ok(Prepared {
            record,
            name,
            key,
            enabled,
            draft,
        })
    }
}
/// SQL writer, configured independently of raw reads. All admitted writes are MustLand.
#[derive(Clone)]
pub struct DocumentWriter {
    handle: StorageHandle,
    policy: WritePolicy,
    clock: Arc<dyn Clock + Send + Sync>,
}
impl Documents {
    pub fn writer(
        &self,
        policy: WritePolicy,
        clock: Arc<dyn Clock + Send + Sync>,
    ) -> DocumentWriter {
        DocumentWriter {
            handle: self.handle.clone(),
            policy,
            clock,
        }
    }
}
impl DocumentWriter {
    pub async fn put(&self, put: Put) -> Result<Record> {
        Ok(self.batch_put(vec![put]).await?.remove(0))
    }
    /// Commit every body, revision, index and attribution together or none.
    /// Inputs run in order: repeated (kind, id) entries see earlier writes in this batch.
    pub async fn batch_put(&self, puts: Vec<Put>) -> Result<Vec<Record>> {
        let policy = self.policy.clone();
        let clock = self.clock.clone();
        run(&self.handle, move |conns| {
            let tx = conns
                .config
                .transaction_with_behavior(TransactionBehavior::Immediate)?;
            let now = clock.wall();
            let records = puts
                .into_iter()
                .map(|put| write(&tx, &policy, put, now))
                .collect::<Result<Vec<_>>>()?;
            tx.commit()?;
            Ok(records)
        })
        .await
    }
    /// Delete only a registered current-version row, checking its revision under the write lock.
    /// A missing row returns false unless an expected revision was supplied.
    pub async fn delete(
        &self,
        kind: String,
        id: EntityId,
        expected_revision: Option<u64>,
    ) -> Result<bool> {
        let policy = self.policy.clone();
        run(&self.handle, move |conns| {
            policy.version(&kind)?;
            let tx = conns
                .config
                .transaction_with_behavior(TransactionBehavior::Immediate)?;
            let old = read(&tx, &kind, id)?;
            check_revision(id, expected_revision, old.as_ref())?;
            if let Some(old) = &old {
                policy.check_existing(old)?;
            }
            let deleted = tx.execute(
                "DELETE FROM documents WHERE kind=?1 AND id=?2",
                params![kind, id.to_string()],
            )?;
            tx.commit()?;
            Ok(deleted != 0)
        })
        .await
    }
}
fn write(tx: &Transaction<'_>, policy: &WritePolicy, put: Put, now: Timestamp) -> Result<Record> {
    let old = read(tx, &put.document.kind, put.id)?;
    let prepared = policy.prepare(put, old.as_ref(), now)?;
    let record = prepared.record;
    let kind = &record.document.kind;
    let body = serde_json::to_string(&record.document.data)?;
    // Use the same decoder as read(): never persist a body it cannot load again.
    serde_json::from_str::<Value>(&body)
        .map_err(|_| DocumentError::InvalidInput("document body cannot be decoded"))?;
    if !prepared.draft && matches!(kind.as_str(), "action" | "queue" | "command") {
        let clash: Option<String> = tx
            .query_row(
                "SELECT id FROM documents WHERE kind=?1 AND name_key=?2 AND id<>?3
             AND (kind<>'action' OR json_extract(data,'$.origin.kind') IS NOT 'ai_draft')",
                params![kind, prepared.key, record.id.to_string()],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(id) = clash {
            return Err(DocumentError::Conflict {
                id: id
                    .parse()
                    .map_err(|_| DocumentError::InvalidStored("invalid clashing id"))?,
            });
        }
    }
    tx.execute(
        "INSERT INTO documents(kind,id,schema_version,revision,name,name_key,enabled,data,created_at,updated_at,updated_by)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)
         ON CONFLICT(kind,id) DO UPDATE SET schema_version=excluded.schema_version,revision=excluded.revision,
         name=excluded.name,name_key=excluded.name_key,enabled=excluded.enabled,data=excluded.data,
         updated_at=excluded.updated_at,updated_by=excluded.updated_by",
        params![kind,record.id.to_string(),record.document.schema_version,record.revision as i64,prepared.name,prepared.key,prepared.enabled,
            body,record.created_at.to_string(),record.updated_at.to_string(),serde_json::to_string(&record.updated_by)?])?;
    Ok(record)
}
