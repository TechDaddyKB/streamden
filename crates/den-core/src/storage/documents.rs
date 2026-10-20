//! Internal, UNFROZEN read seam for Spec 03 §13.2; no serialized or wire contract.
//! Reads decode persisted fields only. They do not migrate, validate, disable or install
//! documents. DEN-302 adds those load semantics before consumers may use this as a loader.
use super::thread::{Admission, JobClass, StorageConns, StorageError, StorageHandle};
use den_protocol::{EntityId, StoredDocument, Timestamp, UpdatedBy, rpc::ErrorCode};
use rusqlite::{Connection, OptionalExtension, params};

pub type Result<T> = std::result::Result<T, DocumentError>;

/// Rust-only diagnostics. Raw errors are retained locally, never serialized to RPC clients.
#[derive(Debug)]
pub enum DocumentError {
    Transport(StorageError),
    Sqlite(rusqlite::Error),
    Json(serde_json::Error),
    InvalidInput(&'static str),
    InvalidStored(&'static str),
}
impl DocumentError {
    /// Existing protocol code only; callers must not expose raw diagnostics on the wire.
    pub fn code(&self) -> ErrorCode {
        ErrorCode::new(match self {
            Self::InvalidInput(_) => ErrorCode::INVALID_PARAMS,
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
