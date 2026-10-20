use den_core::storage::{paths::PreparedPaths, schema::*};
use den_protocol::Timestamp;
use rusqlite::Connection;

fn files() -> (tempfile::TempDir, Connection, Connection) {
    let temp = tempfile::tempdir().unwrap();
    let paths = PreparedPaths::prepare(
        Some(temp.path().join("data")),
        |_| unreachable!(),
        || unreachable!(),
    )
    .unwrap();
    let config = Connection::open(paths.config()).unwrap();
    let history = Connection::open(paths.history()).unwrap();
    (temp, config, history)
}

fn schema(conn: &Connection) -> Vec<(String, String)> {
    conn.prepare("SELECT name, sql FROM sqlite_schema WHERE sql IS NOT NULL ORDER BY name")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}

#[test]
fn fresh_files_get_schema_v1() {
    let (_temp, mut config, mut history) = files();
    bootstrap_config(&mut config).unwrap();
    bootstrap_history(&mut history).unwrap();
    let spec = include_str!("../../../../docs/spec/03-engine.md");
    let ddl = spec.split("### 13.2 Schema v1\n\n```sql\n").nth(1).unwrap();
    let (config_ddl, history_ddl) = ddl
        .split("\n```")
        .next()
        .unwrap()
        .split_once("\n-- history.db\n")
        .unwrap();
    let source = include_str!("../../src/storage/schema.rs");
    for (conn, ddl) in [(&config, config_ddl), (&history, history_ddl)] {
        assert!(source.contains(ddl), "DDL must be verbatim accepted §13.2");
        let expected = Connection::open_in_memory().unwrap();
        expected.execute_batch(ddl).unwrap();
        assert_eq!(schema(conn), schema(&expected));
        assert_eq!(
            conn.query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0))
                .unwrap(),
            "ok"
        );
    }
    assert_eq!(
        config
            .query_row(
                "SELECT value FROM meta WHERE key='schema_version'",
                [],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
        "1"
    );
}

#[test]
fn meta_rows_present() {
    let (_temp, mut config, mut history) = files();
    let before = Timestamp::now();
    bootstrap_config(&mut config).unwrap();
    bootstrap_history(&mut history).unwrap();
    let rows: std::collections::BTreeMap<String, String> = config
        .prepare("SELECT key, value FROM meta")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    assert_eq!(rows.len(), 4);
    assert_eq!(rows["clean_shutdown"], "false");
    assert_eq!(rows["schema_version"], "1");
    let id = uuid::Uuid::parse_str(&rows["instance_id"]).unwrap();
    assert_eq!(id.get_version_num(), 7);
    assert_eq!(id.to_string(), rows["instance_id"]);
    let created = Timestamp::parse(&rows["created_at"]).unwrap();
    assert_eq!(created.to_string(), rows["created_at"]);
    assert!(created >= before && created <= Timestamp::now());
    assert!(!schema(&history).iter().any(|(name, _)| name == "meta"));
    assert!(bootstrap_config(&mut config).is_err());
    assert_eq!(
        config
            .query_row("SELECT value FROM meta WHERE key='instance_id'", [], |r| {
                r.get::<_, String>(0)
            })
            .unwrap(),
        rows["instance_id"]
    );
}

#[test]
fn pragmas_applied_both_files() {
    let (_temp, mut config, mut history) = files();
    bootstrap_config(&mut config).unwrap();
    bootstrap_history(&mut history).unwrap();
    for conn in [&mut config, &mut history] {
        assert_eq!(
            conn.query_row("PRAGMA journal_mode", [], |r| r.get::<_, String>(0))
                .unwrap(),
            "wal"
        );
        for (name, expected) in [
            ("synchronous", 1),
            ("foreign_keys", 1),
            ("busy_timeout", 5000),
            ("journal_size_limit", 67108864),
            ("temp_store", 2),
        ] {
            assert_eq!(
                conn.pragma_query_value(None, name, |r| r.get::<_, i64>(0))
                    .unwrap(),
                expected,
                "{name}"
            );
        }
        conn.execute_batch("CREATE TABLE parent(id INTEGER PRIMARY KEY); CREATE TABLE child(id INTEGER REFERENCES parent(id));").unwrap();
        assert!(conn.execute("INSERT INTO child VALUES (99)", []).is_err());
        assert!(
            conn.prepare("PRAGMA foreign_key_check")
                .unwrap()
                .query([])
                .unwrap()
                .next()
                .unwrap()
                .is_none()
        );
        assert_eq!(
            checkpoint_truncate(conn).unwrap(),
            CheckpointOutcome {
                busy: 0,
                log_pages: 0,
                checkpointed_pages: 0
            }
        );
        let tx = conn.transaction().unwrap();
        assert!(apply_pragmas(&tx).is_err());
        tx.rollback().unwrap();
    }
}

#[test]
fn ddl_and_metadata_failures_roll_back() {
    let (_temp, mut config, mut history) = files();
    for (conn, blocker, bootstrap) in [
        (
            &mut config,
            "migrations",
            bootstrap_config as fn(&mut Connection) -> rusqlite::Result<()>,
        ),
        (&mut history, "audit", bootstrap_history),
    ] {
        conn.execute_batch(&format!(
            "CREATE TABLE {blocker}(sentinel TEXT); INSERT INTO {blocker} VALUES ('preserved');"
        ))
        .unwrap();
        let before = schema(conn);
        assert!(bootstrap(conn).is_err());
        assert!(conn.is_autocommit());
        assert_eq!(schema(conn), before);
        assert_eq!(
            conn.query_row(&format!("SELECT sentinel FROM {blocker}"), [], |r| r
                .get::<_, String>(0))
                .unwrap(),
            "preserved"
        );
    }
    let (_temp, mut config, _) = files();
    unsafe extern "C" fn deny_fourth_meta_insert(
        count: *mut std::ffi::c_void,
        action: i32,
        table: *const std::ffi::c_char,
        _: *const std::ffi::c_char,
        _: *const std::ffi::c_char,
        _: *const std::ffi::c_char,
    ) -> i32 {
        // SQLite supplies valid callback arguments; count lives until the callback is removed.
        if action == rusqlite::ffi::SQLITE_INSERT
            && unsafe { std::ffi::CStr::from_ptr(table) }.to_bytes() == b"meta"
        {
            let count = unsafe { &mut *count.cast::<u32>() };
            *count += 1;
            if *count == 4 {
                return rusqlite::ffi::SQLITE_DENY;
            }
        }
        rusqlite::ffi::SQLITE_OK
    }
    let mut inserts = 0_u32;
    // Connection-local test fault only; the counter stays live and no statements escape this call.
    let (install, result, remove) = unsafe {
        let db = config.handle();
        let install = rusqlite::ffi::sqlite3_set_authorizer(
            db,
            Some(deny_fourth_meta_insert),
            (&mut inserts as *mut u32).cast(),
        );
        let result = bootstrap_config(&mut config);
        let remove = rusqlite::ffi::sqlite3_set_authorizer(db, None, std::ptr::null_mut());
        (install, result, remove)
    };
    assert_eq!((install, remove), (0, 0));
    assert_eq!(inserts, 4);
    assert!(result.is_err());
    assert!(config.is_autocommit());
    assert!(schema(&config).is_empty());
    bootstrap_config(&mut config).unwrap();
}

#[test]
fn bootstrap_rejects_active_transactions() {
    let (_temp, mut config, mut history) = files();
    for (conn, bootstrap) in [
        (
            &mut config,
            bootstrap_config as fn(&mut Connection) -> rusqlite::Result<()>,
        ),
        (&mut history, bootstrap_history),
    ] {
        conn.execute_batch("BEGIN").unwrap();
        assert!(bootstrap(conn).is_err());
        assert!(!conn.is_autocommit());
        assert!(schema(conn).is_empty());
        conn.execute_batch("ROLLBACK").unwrap();
        bootstrap(conn).unwrap();
    }
}
