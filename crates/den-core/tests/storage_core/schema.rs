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

fn assert_schema_v1(spec: &str, source: &str) {
    let spec = spec.replace("\r\n", "\n");
    let source = source.replace("\r\n", "\n");
    let (_temp, mut config, mut history) = files();
    bootstrap_config(&mut config).unwrap();
    bootstrap_history(&mut history).unwrap();
    let ddl = spec.split("### 13.2 Schema v1\n\n```sql\n").nth(1).unwrap();
    let (config_ddl, history_ddl) = ddl
        .split("\n```")
        .next()
        .unwrap()
        .split_once("\n-- history.db\n")
        .unwrap();
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
fn fresh_files_get_schema_v1() {
    assert_schema_v1(
        include_str!("../../../../docs/spec/03-engine.md"),
        include_str!("../../src/storage/schema.rs"),
    );
}

#[test]
fn schema_v1_accepts_lf_and_crlf_inputs() {
    let spec = include_str!("../../../../docs/spec/03-engine.md").replace("\r\n", "\n");
    let source = include_str!("../../src/storage/schema.rs").replace("\r\n", "\n");
    for spec in [&spec, &spec.replace('\n', "\r\n")] {
        for source in [&source, &source.replace('\n', "\r\n")] {
            assert_schema_v1(spec, source);
        }
    }
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
fn constraint(result: rusqlite::Result<usize>, code: i32) {
    match result.unwrap_err() {
        rusqlite::Error::SqliteFailure(error, _) => assert_eq!(error.extended_code, code),
        other => panic!("expected SQLite constraint {code}, got {other:?}"),
    }
}

#[test]
fn all_tables_enforce_primary_keys_not_null_and_json_checks() {
    let (_temp, mut config, mut history) = files();
    bootstrap_config(&mut config).unwrap();
    bootstrap_history(&mut history).unwrap();
    config
        .execute_batch(
            "INSERT INTO documents(kind,id,schema_version,data,created_at,updated_at,updated_by)
         VALUES ('action','id',1,'{}','created','updated','{}');
         INSERT INTO globals(name,value,updated_at,updated_by) VALUES ('g','{}','t','u');
         INSERT INTO user_vars(platform,user_id,name,value,updated_at,updated_by)
         VALUES ('p','u','v','{}','t','u');
         INSERT INTO counters(owner_kind,owner_id,key,updated_at) VALUES ('command','id','','t');
         INSERT INTO cron_cursors VALUES ('a','t','s','last','updated');
         INSERT INTO migrations VALUES (1,'t','v');",
        )
        .unwrap();
    history.execute_batch(
        "INSERT INTO runs(id,action_id,action_name,action_revision,queue_id,state,queued_at,principal_kind,surface)
         VALUES ('r','a','action',1,'q','queued','t','owner','ui');
         INSERT INTO events(id,type,ts,payload_version,envelope) VALUES ('e','type','t',1,'{}');
         INSERT INTO audit(id,ts,kind,details) VALUES ('a','t','kind','{}');",
    ).unwrap();
    let mut tested = (0, 0, 0);
    for (conn, tables) in [
        (
            &config,
            "meta documents globals user_vars counters cron_cursors migrations",
        ),
        (&history, "runs events audit"),
    ] {
        for table in tables.split_whitespace() {
            let count = || {
                conn.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| {
                    r.get::<_, i64>(0)
                })
                .unwrap()
            };
            let before = count();
            assert!(before > 0, "{table} fixture must exist");
            constraint(
                conn.execute(&format!("INSERT INTO {table} SELECT * FROM {table}"), []),
                rusqlite::ffi::SQLITE_CONSTRAINT_PRIMARYKEY,
            );
            tested.0 += 1;
            let columns: Vec<(String, bool)> = conn
                .prepare(&format!("PRAGMA table_info({table})"))
                .unwrap()
                .query_map([], |r| Ok((r.get(1)?, r.get(3)?)))
                .unwrap()
                .collect::<rusqlite::Result<_>>()
                .unwrap();
            for (column, required) in columns {
                if required {
                    constraint(
                        conn.execute(&format!("UPDATE {table} SET {column}=NULL"), []),
                        rusqlite::ffi::SQLITE_CONSTRAINT_NOTNULL,
                    );
                    tested.1 += 1;
                }
            }
            assert_eq!(count(), before);
        }
    }
    let defaults: (i64, i64, String, String, String) = history
        .query_row(
            "SELECT inline,test,warnings,untrusted_args_queued,untrusted_args_after FROM runs",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .unwrap();
    assert_eq!(defaults, (0, 0, "[]".into(), "[]".into(), "[]".into()));
    for (conn, table, columns) in [
        (&config, "documents", "data updated_by"),
        (&config, "globals", "value"),
        (&config, "user_vars", "value"),
        (
            &history,
            "runs",
            "chain error warnings args_queued args_after event_snapshot untrusted_args_queued untrusted_args_after",
        ),
        (&history, "events", "envelope"),
        (&history, "audit", "details"),
    ] {
        for column in columns.split_whitespace() {
            let sql = format!("UPDATE {table} SET {column}=?1");
            for valid in ["{}", "[]", "null", "true", "42", "\"text\""] {
                assert_eq!(conn.execute(&sql, [valid]).unwrap(), 1);
            }
            for invalid in ["{broken", "", "undefined"] {
                constraint(
                    conn.execute(&sql, [invalid]),
                    rusqlite::ffi::SQLITE_CONSTRAINT_CHECK,
                );
                assert_eq!(
                    conn.query_row(&format!("SELECT {column} FROM {table}"), [], |r| r
                        .get::<_, String>(0))
                        .unwrap(),
                    "\"text\""
                );
            }
            tested.2 += 1;
        }
    }
    eprintln!("constraint outcomes (PK, NOT NULL, JSON): {tested:?}");
    assert_eq!(tested, (10, 52, 14));
    for column in [
        "chain",
        "error",
        "args_queued",
        "args_after",
        "event_snapshot",
    ] {
        assert_eq!(
            history
                .execute(&format!("UPDATE runs SET {column}=NULL"), [])
                .unwrap(),
            1
        );
    }
    assert_eq!(
        config
            .query_row("SELECT revision FROM documents", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        1
    );
    for table in ["globals", "user_vars"] {
        assert_eq!(
            config
                .query_row(&format!("SELECT untrusted FROM {table}"), [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );
    }
    assert_eq!(
        config
            .query_row("SELECT value FROM counters", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        0
    );
}

fn document(
    conn: &Connection,
    kind: &str,
    id: &str,
    name: &str,
    key: Option<&str>,
    data: &str,
) -> rusqlite::Result<usize> {
    conn.execute(
        "INSERT INTO documents(kind,id,schema_version,name,name_key,data,created_at,updated_at,updated_by)
         VALUES (?1,?2,1,?3,?4,?5,'t','t','{}')",
        rusqlite::params![kind, id, name, key, data],
    )
}

#[test]
fn partial_name_indexes_use_name_key_and_exclude_only_ai_drafts() {
    let (_temp, mut config, _) = files();
    bootstrap_config(&mut config).unwrap();
    let live = [
        "{}",
        r#"{"origin":{}}"#,
        r#"{"origin":null}"#,
        r#"{"origin":{"kind":null}}"#,
        r#"{"origin":{"kind":"manual"}}"#,
        r#"{"origin":{"kind":"AI_DRAFT"}}"#,
    ];
    let draft = r#"{"origin":{"kind":"ai_draft"}}"#;
    for first in live {
        for second in live {
            config.execute("DELETE FROM documents", []).unwrap();
            document(&config, "action", "one", "Straße", Some("strasse"), first).unwrap();
            constraint(
                document(&config, "action", "two", "STRASSE", Some("strasse"), second),
                rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE,
            );
            document(&config, "action", "draft1", "draft", Some("strasse"), draft).unwrap();
            document(&config, "action", "draft2", "draft", Some("strasse"), draft).unwrap();
            constraint(
                config.execute("UPDATE documents SET data='{}' WHERE id='draft1'", []),
                rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE,
            );
            assert_eq!(
                config
                    .query_row("SELECT count(*) FROM documents", [], |r| r.get::<_, i64>(0))
                    .unwrap(),
                3
            );
        }
    }
    config.execute("DELETE FROM documents", []).unwrap();
    document(&config, "action", "draft", "same", Some("key"), draft).unwrap();
    document(&config, "action", "live", "same", Some("key"), "{}").unwrap();
    for kind in ["queue", "command"] {
        document(
            &config,
            kind,
            "one",
            "different display",
            Some("key"),
            draft,
        )
        .unwrap();
        constraint(
            document(&config, kind, "two", "same", Some("key"), "{}"),
            rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE,
        );
    }
    for kind in [
        "action",
        "queue",
        "command",
        "timer",
        "settings",
        "view_state",
        "pending_change",
        "export_meta",
    ] {
        document(&config, kind, "null1", "same", None, "{}").unwrap();
        document(&config, kind, "null2", "same", None, "{}").unwrap();
        document(&config, kind, "case1", "same", Some("Key"), "{}").unwrap();
        document(&config, kind, "case2", "same", Some("KEY"), "{}").unwrap();
        if !["action", "queue", "command"].contains(&kind) {
            document(&config, kind, "one", "same", Some("key"), "{}").unwrap();
            document(&config, kind, "two", "same", Some("key"), "{}").unwrap();
        }
    }
}

#[test]
fn checkpoint_reports_blocked_reader_then_truncates_without_losing_data() {
    let (temp, mut config, mut history) = files();
    bootstrap_config(&mut config).unwrap();
    bootstrap_history(&mut history).unwrap();
    for (conn, name) in [(&mut config, "config.db"), (&mut history, "history.db")] {
        conn.execute_batch(
            "CREATE TABLE checkpoint_data(value INTEGER); INSERT INTO checkpoint_data VALUES (1);",
        )
        .unwrap();
        assert_eq!(checkpoint_truncate(conn).unwrap().busy, 0);
        let reader = Connection::open(temp.path().join("data").join(name)).unwrap();
        reader.execute_batch("BEGIN").unwrap();
        assert_eq!(
            reader
                .query_row("SELECT sum(value) FROM checkpoint_data", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
        conn.execute("INSERT INTO checkpoint_data VALUES (2)", [])
            .unwrap();
        // Only this fixture avoids SQLite's default wait; production remains 5000 ms.
        conn.pragma_update(None, "busy_timeout", 0).unwrap();
        let blocked = checkpoint_truncate(conn).unwrap();
        eprintln!("{name}: blocked checkpoint {blocked:?}");
        assert_eq!(blocked.busy, 1);
        assert!(blocked.log_pages > 0);
        assert!(blocked.checkpointed_pages >= 0 && blocked.checkpointed_pages < blocked.log_pages);
        let wal = temp.path().join("data").join(format!("{name}-wal"));
        assert!(std::fs::metadata(&wal).unwrap().len() > 0);
        assert_eq!(
            reader
                .query_row("SELECT sum(value) FROM checkpoint_data", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
        reader.execute_batch("ROLLBACK").unwrap();
        assert_eq!(
            checkpoint_truncate(conn).unwrap(),
            CheckpointOutcome {
                busy: 0,
                log_pages: 0,
                checkpointed_pages: 0
            }
        );
        eprintln!("{name}: successful checkpoint (0,0,0), WAL bytes=0, reader sum=3");
        assert_eq!(std::fs::metadata(&wal).unwrap().len(), 0);
        assert_eq!(
            reader
                .query_row("SELECT sum(value) FROM checkpoint_data", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            3
        );
        let tx = conn.transaction().unwrap();
        tx.execute("INSERT INTO checkpoint_data VALUES (4)", [])
            .unwrap();
        match checkpoint_truncate(&tx).unwrap_err() {
            rusqlite::Error::SqliteFailure(error, _) => {
                assert_eq!(error.code, rusqlite::ErrorCode::DatabaseLocked)
            }
            other => panic!("expected checkpoint lock error, got {other:?}"),
        }
        tx.rollback().unwrap();
        apply_pragmas(conn).unwrap();
        assert_eq!(
            conn.pragma_query_value(None, "busy_timeout", |r| r.get::<_, i64>(0))
                .unwrap(),
            5000
        );
        assert_eq!(
            conn.query_row("SELECT sum(value) FROM checkpoint_data", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            3
        );
    }
}

#[test]
fn independent_fresh_configs_have_distinct_canonical_metadata() {
    let mut ids = std::collections::BTreeSet::new();
    for _ in 0..2 {
        let (_temp, mut config, _) = files();
        let before = Timestamp::now();
        bootstrap_config(&mut config).unwrap();
        let after = Timestamp::now();
        let (id, created): (String, String) = config.query_row(
            "SELECT (SELECT value FROM meta WHERE key='instance_id'), (SELECT value FROM meta WHERE key='created_at')", [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        ).unwrap();
        let parsed = uuid::Uuid::parse_str(&id).unwrap();
        assert_eq!(parsed.get_version_num(), 7);
        assert_eq!(parsed.to_string(), id);
        assert!(ids.insert(id));
        let timestamp = Timestamp::parse(&created).unwrap();
        assert_eq!(timestamp.to_string(), created);
        eprintln!("fresh config: {parsed}; {before} <= {timestamp} <= {after}");
        assert!(before <= timestamp && timestamp <= after);
    }
    assert_eq!(ids.len(), 2);
}
