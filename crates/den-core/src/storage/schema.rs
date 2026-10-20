//! Fresh schema v1 only (Spec 03 §13). Call on the storage OS thread, outside Tokio.
//! The caller inspects existing files and decides write/checkpoint eligibility (DEN-72/73).
//! Metadata belongs only to config.db; history has no meta table (§13.2).

use den_protocol::Timestamp;
use rusqlite::{Connection, Result};
use uuid::Uuid;

const CONFIG_DDL: &str = r#"-- config.db
CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);            -- schema_version, instance_id, clean_shutdown, created_at
CREATE TABLE documents (
  kind TEXT NOT NULL,             -- action | queue | command | timer | settings | view_state | pending_change | export_meta
  id TEXT NOT NULL,               -- UUID string
  schema_version INTEGER NOT NULL,
  revision INTEGER NOT NULL DEFAULT 1,
  name TEXT,                      -- denormalized from data for display; NULL for kinds without a name
  name_key TEXT,                  -- Unicode case-folded name computed in Rust (SQLite lower() is ASCII-only); uniqueness key
  enabled INTEGER,                -- denormalized; NULL where not applicable
  data TEXT NOT NULL CHECK (json_valid(data)),   -- spec 02 §7.3 `data`
  created_at TEXT NOT NULL, updated_at TEXT NOT NULL, updated_by TEXT NOT NULL CHECK (json_valid(updated_by)),
  PRIMARY KEY (kind, id)
);
CREATE UNIQUE INDEX documents_action_name ON documents (name_key)
  WHERE kind = 'action' AND json_extract(data, '$.origin.kind') IS NOT 'ai_draft';   -- IS NOT: a missing origin.kind must not escape the index
CREATE UNIQUE INDEX documents_queue_name ON documents (name_key) WHERE kind = 'queue';
CREATE UNIQUE INDEX documents_command_name ON documents (name_key) WHERE kind = 'command';
CREATE INDEX documents_kind_updated ON documents (kind, updated_at);
CREATE TABLE globals (
  name TEXT PRIMARY KEY, value TEXT NOT NULL CHECK (json_valid(value)),
  untrusted INTEGER NOT NULL DEFAULT 0,                 -- provenance flag, §21; written with the value
  updated_at TEXT NOT NULL, updated_by TEXT NOT NULL
);                                  -- persisted globals only; temp globals never touch disk
CREATE TABLE user_vars (
  platform TEXT NOT NULL, user_id TEXT NOT NULL, name TEXT NOT NULL,
  value TEXT NOT NULL CHECK (json_valid(value)), untrusted INTEGER NOT NULL DEFAULT 0,
  updated_at TEXT NOT NULL, updated_by TEXT NOT NULL,
  PRIMARY KEY (platform, user_id, name)
);
CREATE INDEX user_vars_name ON user_vars (name);
CREATE TABLE counters (
  owner_kind TEXT NOT NULL,         -- command | timer
  owner_id TEXT NOT NULL, key TEXT NOT NULL,          -- key: '' for the global counter, 'u:<platform>:<userId>' per user
  value INTEGER NOT NULL DEFAULT 0, updated_at TEXT NOT NULL,
  PRIMARY KEY (owner_kind, owner_id, key)
);
CREATE TABLE cron_cursors (                              -- §8.3: last accounted occurrence per cron trigger
  action_id TEXT NOT NULL, trigger_id TEXT NOT NULL, schedule_key TEXT NOT NULL,
  last_scheduled_at TEXT NOT NULL, updated_at TEXT NOT NULL,
  PRIMARY KEY (action_id, trigger_id)
);
CREATE TABLE migrations (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL, app_version TEXT NOT NULL);
"#;
const HISTORY_DDL: &str = r#"-- history.db
CREATE TABLE runs (
  id TEXT PRIMARY KEY, action_id TEXT NOT NULL, action_name TEXT NOT NULL, action_revision INTEGER NOT NULL,
  trigger_id TEXT, event_id TEXT, queue_id TEXT NOT NULL,
  state TEXT NOT NULL, cancel_reason TEXT,
  queued_at TEXT NOT NULL, started_at TEXT, completed_at TEXT, duration_ms INTEGER,
  principal_kind TEXT NOT NULL, surface TEXT NOT NULL, chain TEXT CHECK (chain IS NULL OR json_valid(chain)),
  parent_id TEXT, inline INTEGER NOT NULL DEFAULT 0, test INTEGER NOT NULL DEFAULT 0, replay_of TEXT,
  error TEXT CHECK (error IS NULL OR json_valid(error)), stopped_by TEXT,
  warnings TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(warnings)),
  args_queued TEXT CHECK (args_queued IS NULL OR json_valid(args_queued)),
  args_after  TEXT CHECK (args_after  IS NULL OR json_valid(args_after)),
  event_snapshot TEXT CHECK (event_snapshot IS NULL OR json_valid(event_snapshot)),
  untrusted_args_queued TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(untrusted_args_queued)),
  untrusted_args_after  TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(untrusted_args_after))
);
CREATE INDEX runs_queued_at ON runs (queued_at);
CREATE INDEX runs_action    ON runs (action_id, queued_at);
CREATE INDEX runs_state     ON runs (state, queued_at);
CREATE INDEX runs_parent    ON runs (parent_id);
CREATE TABLE events (
  id TEXT PRIMARY KEY, type TEXT NOT NULL, ts TEXT NOT NULL, platform TEXT, user_id TEXT,   -- denormalized for indexes
  payload_version INTEGER NOT NULL,
  envelope TEXT NOT NULL CHECK (json_valid(envelope))   -- the complete redacted Event (§12.5): source, user (full UserRef), payload, meta
);
CREATE INDEX events_type_ts ON events (type, ts);
CREATE INDEX events_ts      ON events (ts);
CREATE TABLE audit (
  id TEXT PRIMARY KEY, ts TEXT NOT NULL, kind TEXT NOT NULL,       -- tier_exceeds_context | binding_mismatch | unattended | chain_context_lost | chain_hop_limit | lineage_limit | caused_by_ignored | plugin_payload_invalid …
  run_id TEXT, details TEXT NOT NULL CHECK (json_valid(details))
);
CREATE INDEX audit_ts  ON audit (ts);
CREATE INDEX audit_run ON audit (run_id);"#;

/// Apply connection-local settings before any transaction; caller supplies a writable disk DB.
pub fn apply_pragmas(conn: &Connection) -> Result<()> {
    if !conn.is_autocommit() {
        return Err(rusqlite::Error::InvalidQuery);
    }
    let mode: String = conn.query_row("PRAGMA journal_mode=WAL", [], |row| row.get(0))?;
    if mode != "wal" {
        return Err(rusqlite::Error::InvalidQuery);
    }
    conn.execute_batch(
        "PRAGMA synchronous=NORMAL; PRAGMA foreign_keys=ON; PRAGMA busy_timeout=5000;
         PRAGMA journal_size_limit=67108864; PRAGMA temp_store=MEMORY;",
    )?;
    for (name, expected) in [
        ("synchronous", 1),
        ("foreign_keys", 1),
        ("busy_timeout", 5000),
        ("journal_size_limit", 67108864),
        ("temp_store", 2),
    ] {
        if conn.pragma_query_value(None, name, |row| row.get::<_, i64>(0))? != expected {
            return Err(rusqlite::Error::InvalidQuery);
        }
    }
    Ok(())
}

/// Bootstrap a caller-verified fresh config file, with DDL and all metadata committed together.
pub fn bootstrap_config(conn: &mut Connection) -> Result<()> {
    apply_pragmas(conn)?;
    let tx = conn.transaction()?;
    tx.execute_batch(CONFIG_DDL)?;
    let instance = Uuid::now_v7().to_string();
    let created = Timestamp::now().to_string();
    for (key, value) in [
        ("schema_version", "1"),
        ("instance_id", instance.as_str()),
        ("created_at", created.as_str()),
        ("clean_shutdown", "false"),
    ] {
        tx.execute(
            "INSERT INTO meta (key, value) VALUES (?1, ?2)",
            [key, value],
        )?;
    }
    tx.commit()
}

/// Bootstrap a caller-verified fresh history file, independently atomic from config.
pub fn bootstrap_history(conn: &mut Connection) -> Result<()> {
    apply_pragmas(conn)?;
    let tx = conn.transaction()?;
    tx.execute_batch(HISTORY_DDL)?;
    tx.commit()
}

/// Raw SQLite outcome, including busy or -1 page counts when there is no WAL.
#[derive(Debug, PartialEq, Eq)]
pub struct CheckpointOutcome {
    pub busy: i32,
    pub log_pages: i32,
    pub checkpointed_pages: i32,
}

/// Return SQLite's outcome; busy is not a successful truncation.
/// Call only after the caller has established checkpoint eligibility, with no open transaction.
pub fn checkpoint_truncate(conn: &Connection) -> Result<CheckpointOutcome> {
    conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| {
        Ok(CheckpointOutcome {
            busy: row.get(0)?,
            log_pages: row.get(1)?,
            checkpointed_pages: row.get(2)?,
        })
    })
}
