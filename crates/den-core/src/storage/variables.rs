//! Internal UNFROZEN raw persistence leaf (Spec 03 §11.4/13.2), not a VarStore.
//! Callers supply server-attributed metadata. No namespace moves, memory cache, arithmetic,
//! startup recovery policy, flush scheduling, durable wait or events are implemented here.
use super::{
    documents::{DocumentError, Result, run},
    thread::StorageHandle,
};
use den_protocol::{Timestamp, UpdatedBy, VariableRecord};
use rusqlite::params;
use std::collections::BTreeMap;

/// Exact persisted scope; these strings are bound values, never SQL identifiers.
#[derive(Clone, Debug)]
pub enum Scope {
    Global,
    User { platform: String, user_id: String },
}

/// Ordered raw mutations. Delete also supports removing an old persisted row before a
/// future VarStore moves its entry to memory only. Upsert never accepts a temporary record.
#[derive(Clone, Debug)]
pub enum Mutation {
    Upsert {
        scope: Scope,
        name: String,
        record: VariableRecord,
    },
    Delete {
        scope: Scope,
        name: String,
    },
}

#[derive(Clone)]
pub struct Variables {
    handle: StorageHandle,
}

impl Variables {
    pub fn new(handle: StorageHandle) -> Self {
        Self { handle }
    }

    /// Persisted globals only, keyed by exact name. Malformed rows fail the raw read;
    /// the eventual startup coordinator is responsible for deciding how to handle failure.
    pub async fn read_globals(&self) -> Result<BTreeMap<String, VariableRecord>> {
        run(&self.handle, move |conns| {
            let mut statement = conns
                .config
                .prepare("SELECT name,value,untrusted,updated_at,updated_by FROM globals")?;
            let mut rows = statement.query([])?;
            let mut records = BTreeMap::new();
            while let Some(row) = rows.next()? {
                let record = decode_record(row)?;
                if records.insert(row.get(0)?, record).is_some() {
                    return Err(DocumentError::InvalidStored("duplicate variable name"));
                }
            }
            Ok(records)
        })
        .await
    }

    /// Persisted entries for exactly this platform/user pair; keys are bound unchanged.
    pub async fn read_user(
        &self,
        platform: String,
        user_id: String,
    ) -> Result<BTreeMap<String, VariableRecord>> {
        run(&self.handle, move |conns| {
            let mut statement = conns.config.prepare(
                "SELECT name,value,untrusted,updated_at,updated_by FROM user_vars
                 WHERE platform=?1 AND user_id=?2",
            )?;
            let mut rows = statement.query(rusqlite::params![platform, user_id])?;
            let mut records = BTreeMap::new();
            while let Some(row) = rows.next()? {
                let record = decode_record(row)?;
                if records.insert(row.get(0)?, record).is_some() {
                    return Err(DocumentError::InvalidStored("duplicate variable name"));
                }
            }
            Ok(records)
        })
        .await
    }

    /// One config transaction per admitted batch, including an empty batch. Mutations execute
    /// in order, so repeated keys observe earlier mutations. Any failure rolls back all scopes.
    /// Uses MustLand admission: abandoning an accepted reply does not cancel the transaction.
    pub async fn apply_batch(&self, mutations: Vec<Mutation>) -> Result<()> {
        run(&self.handle, move |conns| {
            let tx = conns.config.transaction()?;
            for mutation in mutations {
                match mutation {
                    Mutation::Upsert { scope, name, record } => {
                        if !record.persisted {
                            return Err(DocumentError::InvalidInput("temporary variable upsert"));
                        }
                        let value = serde_json::to_string(&record.value)
                            .map_err(|_| DocumentError::InvalidInput("unreadable variable JSON"))?;
                        // A serializable Value can exceed the decoder's depth limit. Never
                        // persist a value this leaf cannot read back, or silently change it.
                        let decoded: serde_json::Value = serde_json::from_str(&value)
                            .map_err(|_| DocumentError::InvalidInput("unreadable variable JSON"))?;
                        if decoded != record.value {
                            return Err(DocumentError::InvalidInput("variable JSON changed"));
                        }
                        let at = record.updated_at.to_string();
                        if Timestamp::parse(&at).ok() != Some(record.updated_at) {
                            return Err(DocumentError::InvalidInput("unreadable variable timestamp"));
                        }
                        let by = serde_json::to_string(&record.updated_by)?;
                        match scope {
                            Scope::Global => tx.execute(
                                "INSERT INTO globals(name,value,untrusted,updated_at,updated_by)
                                 VALUES(?1,?2,?3,?4,?5) ON CONFLICT(name) DO UPDATE SET
                                 value=excluded.value,untrusted=excluded.untrusted,
                                 updated_at=excluded.updated_at,updated_by=excluded.updated_by",
                                params![name, value, record.untrusted, at, by],
                            )?,
                            Scope::User { platform, user_id } => tx.execute(
                                "INSERT INTO user_vars(platform,user_id,name,value,untrusted,updated_at,updated_by)
                                 VALUES(?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(platform,user_id,name)
                                 DO UPDATE SET value=excluded.value,untrusted=excluded.untrusted,
                                 updated_at=excluded.updated_at,updated_by=excluded.updated_by",
                                params![platform, user_id, name, value, record.untrusted, at, by],
                            )?,
                        };
                    }
                    Mutation::Delete { scope, name } => {
                        match scope {
                            Scope::Global => tx.execute("DELETE FROM globals WHERE name=?1", [name])?,
                            Scope::User { platform, user_id } => tx.execute(
                                "DELETE FROM user_vars WHERE platform=?1 AND user_id=?2 AND name=?3",
                                params![platform, user_id, name],
                            )?,
                        };
                    }
                }
            }
            tx.commit()?;
            Ok(())
        })
        .await
    }
}

fn decode_record(row: &rusqlite::Row<'_>) -> Result<VariableRecord> {
    let untrusted = match row.get::<_, i64>(2)? {
        0 => false,
        1 => true,
        _ => return Err(DocumentError::InvalidStored("invalid variable flag")),
    };
    let stored: String = row.get(3)?;
    let updated_at = Timestamp::parse(&stored)
        .ok()
        .filter(|timestamp| timestamp.to_string() == stored)
        .ok_or(DocumentError::InvalidStored("invalid variable timestamp"))?;
    let attribution: serde_json::Value = serde_json::from_str(&row.get::<_, String>(4)?)?;
    let updated_by: UpdatedBy = serde_json::from_value(attribution.clone())?;
    if serde_json::to_value(&updated_by)? != attribution {
        return Err(DocumentError::InvalidStored("invalid variable attribution"));
    }
    let record = VariableRecord {
        value: serde_json::from_str(&row.get::<_, String>(1)?)?,
        persisted: true,
        untrusted,
        updated_at,
        updated_by,
    };
    Ok(record)
}
