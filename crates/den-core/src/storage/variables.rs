//! Internal UNFROZEN raw persistence leaf (Spec 03 §11.4/13.2), not a VarStore.
//! Callers supply server-attributed metadata. No namespace moves, memory cache, arithmetic,
//! startup recovery policy, flush scheduling, durable wait or events are implemented here.
use super::{
    documents::{DocumentError, Result, run},
    thread::StorageHandle,
};
use den_protocol::{Timestamp, UpdatedBy, VariableRecord};
use std::collections::BTreeMap;

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
