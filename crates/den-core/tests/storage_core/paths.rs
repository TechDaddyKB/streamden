use den_core::storage::paths::{
    create_private_dir, create_private_file, validate_private_dir, validate_private_file,
};
use std::{fs, io::Write};

#[test]
fn private_creation_and_existing_data_preserved() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("nested/data");
    create_private_dir(&root).unwrap();
    create_private_dir(&root).unwrap();
    validate_private_dir(&root).unwrap();
    let path = root.join("config.db");
    create_private_file(&path)
        .unwrap()
        .write_all(b"keep me")
        .unwrap();
    validate_private_file(&path).unwrap();
    assert_eq!(
        create_private_file(&path).unwrap_err().kind(),
        std::io::ErrorKind::AlreadyExists
    );
    assert_eq!(fs::read(&path).unwrap(), b"keep me");
    assert!(
        create_private_dir(&path)
            .unwrap_err()
            .to_string()
            .contains("type")
    );
    assert!(
        validate_private_file(&root)
            .unwrap_err()
            .to_string()
            .contains("type")
    );
    assert!(create_private_file(&root).is_err());
    assert!(create_private_dir(&path.join("child")).is_err());
}

#[cfg(unix)]
mod unix {
    use super::*;
    use std::os::unix::fs::{PermissionsExt, symlink};
    fn mode(path: &std::path::Path) -> u32 {
        fs::symlink_metadata(path).unwrap().permissions().mode() & 0o7777
    }

    #[test]
    fn unix_file_modes() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("nested/data");
        create_private_dir(&root).unwrap();
        assert_eq!(mode(&temp.path().join("nested")), 0o700);
        assert_eq!(mode(&root), 0o700);
        for name in [
            "backups/daily.db",
            "exports/export.json",
            "config.db",
            "history.db",
        ] {
            let path = root.join(name);
            drop(create_private_file(&path).unwrap());
            assert_eq!(mode(path.parent().unwrap()), 0o700);
            assert_eq!(mode(&path), 0o600);
            if name == "config.db" || name == "history.db" {
                assert_eq!(fs::metadata(&path).unwrap().len(), 0);
                let conn = rusqlite::Connection::open(&path).unwrap();
                conn.execute_batch("PRAGMA journal_mode=WAL; CREATE TABLE test (id INTEGER); INSERT INTO test VALUES (1);").unwrap();
                for suffix in ["-wal", "-shm"] {
                    let sidecar = root.join(format!("{name}{suffix}"));
                    assert_eq!(mode(&sidecar), 0o600);
                    validate_private_file(&sidecar).unwrap();
                }
            }
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn inherited_setgid_directories_remain_private() {
        let temp = tempfile::tempdir().unwrap();
        let parent = temp.path().join("setgid");
        create_private_dir(&parent).unwrap();
        fs::set_permissions(&parent, fs::Permissions::from_mode(0o2700)).unwrap();
        assert_eq!(mode(&parent), 0o2700, "filesystem must retain setgid");
        let nested = parent.join("a/b");
        create_private_dir(&nested).unwrap();
        for dir in [&parent, &parent.join("a"), &nested] {
            assert_eq!(mode(dir), 0o2700, "filesystem must inherit setgid");
            validate_private_dir(dir).unwrap();
            create_private_dir(dir).unwrap();
            assert_eq!(mode(dir), 0o2700);
        }
        let file = nested.join("exports/data");
        drop(create_private_file(&file).unwrap());
        assert_eq!(mode(file.parent().unwrap()), 0o2700);
        assert_eq!(mode(&file), 0o600);
        validate_private_file(&file).unwrap();
    }

    #[test]
    fn unsafe_existing_modes_are_never_repaired() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join("data");
        create_private_dir(&dir).unwrap();
        let file = dir.join("existing");
        create_private_file(&file)
            .unwrap()
            .write_all(b"preserve")
            .unwrap();
        for unsafe_mode in [0o644, 0o400, 0o1600, 0o2600, 0o4600, 0o6600] {
            fs::set_permissions(&file, fs::Permissions::from_mode(unsafe_mode)).unwrap();
            assert!(
                validate_private_file(&file)
                    .unwrap_err()
                    .to_string()
                    .contains("mode")
            );
            assert!(create_private_file(&file).is_err());
            assert_eq!(mode(&file), unsafe_mode);
            assert_eq!(fs::read(&file).unwrap(), b"preserve");
        }
        for unsafe_mode in [0o755, 0o2750, 0o500, 0o4700, 0o1700, 0o6700, 0o3700] {
            fs::set_permissions(&dir, fs::Permissions::from_mode(unsafe_mode)).unwrap();
            assert!(
                create_private_dir(&dir)
                    .unwrap_err()
                    .to_string()
                    .contains("mode")
            );
            assert!(create_private_file(&dir.join("new")).is_err());
            assert!(!dir.join("new").exists());
            assert_eq!(mode(&dir), unsafe_mode);
        }
    }

    #[test]
    fn final_links_refused_but_trusted_symlink_ancestors_allowed() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("private");
        create_private_dir(&root).unwrap();
        let file = root.join("original");
        create_private_file(&file)
            .unwrap()
            .write_all(b"preserve")
            .unwrap();
        let link = root.join("link");
        symlink(&file, &link).unwrap();
        assert!(
            validate_private_file(&link)
                .unwrap_err()
                .to_string()
                .contains("symlink")
        );
        assert!(create_private_file(&link).is_err());
        fs::remove_file(&link).unwrap();
        symlink(root.join("absent"), &link).unwrap();
        assert!(create_private_file(&link).is_err());
        assert!(!root.join("absent").exists());
        fs::remove_file(&link).unwrap();
        fs::hard_link(&file, &link).unwrap();
        assert!(
            validate_private_file(&file)
                .unwrap_err()
                .to_string()
                .contains("hard-linked")
        );
        assert!(validate_private_file(&link).is_err());
        assert!(create_private_file(&link).is_err());
        assert_eq!(fs::read(&file).unwrap(), b"preserve");
        assert_eq!(mode(&file), 0o600);
        let alias = root.join("alias");
        symlink(&root, &alias).unwrap();
        assert!(
            create_private_dir(&alias)
                .unwrap_err()
                .to_string()
                .contains("symlink")
        );
        for suffix in ["/", "/."] {
            let decorated = std::path::PathBuf::from(format!("{}{suffix}", alias.display()));
            assert!(create_private_dir(&decorated).is_err());
        }
        let nested = alias.join("nested/data");
        create_private_dir(&nested).unwrap();
        create_private_file(&nested.join("db")).unwrap();
        assert_eq!(mode(&root.join("nested/data/db")), 0o600);
    }
}

use den_core::storage::paths::PreparedPaths;

fn prepare(path: std::path::PathBuf) -> std::io::Result<PreparedPaths> {
    PreparedPaths::prepare(
        Some(path),
        |_| panic!("environment must be lazy"),
        || panic!("cwd must be lazy"),
    )
}

#[test]
fn prepared_paths_preserve_data_and_probe_without_residue() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("nested/data");
    let paths = prepare(root.clone()).unwrap();
    assert_eq!(paths.root(), fs::canonicalize(&root).unwrap());
    for path in [paths.config(), paths.history()] {
        fs::write(&path, b"database bytes").unwrap();
        for suffix in ["-wal", "-shm"] {
            let sidecar = path.with_extension(format!("db{suffix}"));
            create_private_file(&sidecar)
                .unwrap()
                .write_all(b"sidecar bytes")
                .unwrap();
        }
    }
    prepare(root.clone()).unwrap();
    for (accessor, name) in [(paths.backups(), "backups"), (paths.exports(), "exports")] {
        assert_eq!(accessor, paths.root().join(name));
        assert_eq!(fs::read_dir(accessor).unwrap().count(), 0);
    }
    for (path, name) in [
        (paths.config(), "config.db"),
        (paths.history(), "history.db"),
    ] {
        assert_eq!(path, paths.root().join(name));
        assert_eq!(fs::read(&path).unwrap(), b"database bytes");
        for suffix in ["-wal", "-shm"] {
            assert_eq!(
                fs::read(root.join(format!("{name}{suffix}"))).unwrap(),
                b"sidecar bytes"
            );
        }
    }
    assert_eq!(fs::read_dir(root).unwrap().count(), 8);
}

#[test]
fn relative_dotdot_and_native_errors_without_global_changes() {
    let temp = tempfile::tempdir().unwrap();
    create_private_dir(&temp.path().join("parent")).unwrap();
    let paths = PreparedPaths::prepare(
        Some("parent/../data".into()),
        |_| unreachable!(),
        || Ok(temp.path().into()),
    )
    .unwrap();
    assert_eq!(
        paths.root(),
        fs::canonicalize(temp.path().join("data")).unwrap()
    );
    let error = PreparedPaths::prepare(
        Some("relative".into()),
        |_| unreachable!(),
        || Err(std::io::Error::from_raw_os_error(13)),
    )
    .unwrap_err();
    assert_eq!(error.raw_os_error(), Some(13));
    assert!(
        PreparedPaths::prepare(
            Some("relative".into()),
            |_| unreachable!(),
            || Ok("relative".into())
        )
        .is_err()
    );
    let file = temp.path().join("file");
    fs::write(&file, b"preserved").unwrap();
    assert!(prepare(file.clone()).is_err());
    assert!(
        prepare(file.join("child"))
            .unwrap_err()
            .raw_os_error()
            .is_some()
    );
    assert_eq!(fs::read(file).unwrap(), b"preserved");
}

#[test]
fn entered_runtime_rejected_before_inputs_or_filesystem() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let _entered = runtime.enter();
    let error = PreparedPaths::prepare(None, |_| panic!("environment read"), || panic!("cwd read"))
        .unwrap_err();
    assert!(error.to_string().contains("outside Tokio"));
}

#[cfg(unix)]
#[test]
fn preparation_modes_links_sidecars_and_sqlite() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let temp = tempfile::tempdir().unwrap();
    let paths = prepare(temp.path().join("data")).unwrap();
    for dir in [paths.root(), paths.backups(), paths.exports()] {
        assert_eq!(
            fs::metadata(dir).unwrap().permissions().mode() & 0o7777,
            0o700
        );
    }
    for db in [paths.config(), paths.history()] {
        assert_eq!(
            fs::metadata(&db).unwrap().permissions().mode() & 0o7777,
            0o600
        );
        let conn = rusqlite::Connection::open(&db).unwrap();
        conn.execute_batch(
            "PRAGMA journal_mode=WAL; CREATE TABLE t (x); INSERT INTO t VALUES (1);",
        )
        .unwrap();
        for suffix in ["-wal", "-shm"] {
            let path = db.with_extension(format!("db{suffix}"));
            assert_eq!(
                fs::metadata(path).unwrap().permissions().mode() & 0o7777,
                0o600
            );
        }
        prepare(paths.root()).unwrap();
    }
    let alias = temp.path().join("alias");
    symlink(paths.root(), &alias).unwrap();
    assert!(
        prepare(alias.clone())
            .unwrap_err()
            .to_string()
            .contains("symlink")
    );
    assert_eq!(
        prepare(alias.join("child/../nested")).unwrap().root(),
        fs::canonicalize(paths.root().join("nested")).unwrap()
    );
    for name in [
        "backups",
        "exports",
        "config.db",
        "history.db",
        "config.db-wal",
        "config.db-shm",
        "history.db-wal",
        "history.db-shm",
        "",
    ] {
        let path = paths.root().join(name);
        if !path.exists() {
            drop(create_private_file(&path).unwrap());
        }
        let old = fs::metadata(&path).unwrap().permissions();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o444)).unwrap();
        assert!(
            prepare(paths.root())
                .unwrap_err()
                .to_string()
                .contains("mode")
        );
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o7777,
            0o444
        );
        fs::set_permissions(path, old).unwrap();
    }
}

#[test]
fn prepared_owned_paths_reject_wrong_types() {
    for name in ["backups", "exports", "config.db", "history.db"] {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("private");
        create_private_dir(&root).unwrap();
        let path = root.join(name);
        if name.ends_with("db") {
            create_private_dir(&path).unwrap();
        } else {
            drop(create_private_file(&path).unwrap());
        }
        assert!(prepare(root).unwrap_err().to_string().contains("type"));
    }
}

#[cfg(unix)]
#[test]
fn missing_dotdot_preserves_nonprivate_ancestors() {
    use std::os::unix::fs::PermissionsExt;
    let mode = |path: &std::path::Path| fs::metadata(path).unwrap().permissions().mode() & 0o7777;
    for parent_mode in [0o755, 0o750] {
        let temp = tempfile::tempdir().unwrap();
        let parent = temp.path().join("parent");
        fs::create_dir(&parent).unwrap();
        fs::set_permissions(&parent, fs::Permissions::from_mode(parent_mode)).unwrap();
        let root = parent.join("sd");
        let selected = parent.join("missing/../sd");
        let result = prepare(selected.clone());
        if let Err(error) = &result {
            assert_eq!(mode(&parent), parent_mode);
            assert_eq!(mode(&parent.join("missing")), 0o700);
            assert_eq!(fs::read_dir(parent.join("missing")).unwrap().count(), 0);
            panic!("missing/../sd under {parent_mode:04o} must succeed: {error}");
        }
        let paths = result.unwrap();
        let existing = parent.join("existing");
        fs::create_dir(&existing).unwrap();
        fs::set_permissions(&existing, fs::Permissions::from_mode(parent_mode)).unwrap();
        prepare(parent.join("missing-again/../existing/sd")).unwrap();
        assert_eq!(mode(&existing), parent_mode);
        validate_private_dir(&existing.join("sd")).unwrap();
        assert_eq!(paths.root(), fs::canonicalize(&root).unwrap());
        for db in [paths.config(), paths.history()] {
            fs::write(db, b"preserve database bytes").unwrap();
        }
        prepare(selected).unwrap();
        for dir in [
            parent.join("missing"),
            root,
            paths.backups(),
            paths.exports(),
        ] {
            assert_eq!(mode(&dir), 0o700);
        }
        assert_eq!(mode(&parent), parent_mode);
        assert_eq!(fs::read_dir(parent.join("missing")).unwrap().count(), 0);
        for db in [paths.config(), paths.history()] {
            assert_eq!(mode(&db), 0o600);
            assert_eq!(fs::read(db).unwrap(), b"preserve database bytes");
        }
    }
}

#[cfg(unix)]
#[test]
fn missing_dotdot_uses_symlink_target_parent() {
    use std::os::unix::fs::symlink;
    let temp = tempfile::tempdir().unwrap();
    let target = temp.path().join("actual/nested");
    create_private_dir(&target).unwrap();
    let alias = temp.path().join("alias");
    symlink(&target, &alias).unwrap();
    let paths = prepare(alias.join("missing/../../sd")).unwrap();
    assert_eq!(
        paths.root(),
        fs::canonicalize(temp.path().join("actual/sd")).unwrap()
    );
    assert!(!temp.path().join("sd").exists());
    validate_private_dir(&target.join("missing")).unwrap();
    assert!(prepare(alias).unwrap_err().to_string().contains("symlink"));
}

#[test]
fn data_dir_override_order() {
    use den_core::storage::paths::DataDir;
    let temp = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
    let relative = temp
        .path()
        .strip_prefix(std::env::current_dir().unwrap())
        .unwrap()
        .join("data");
    let paths = DataDir::resolve(Some(relative.clone())).unwrap();
    assert_eq!(paths.root(), fs::canonicalize(&relative).unwrap());
    let mut entries = vec![paths.config(), paths.history()];
    for dir in [paths.backups(), paths.exports()] {
        entries.push(dir.join("preserved"));
        drop(create_private_file(entries.last().unwrap()).unwrap());
    }
    for db in [paths.config(), paths.history()] {
        for suffix in ["-wal", "-shm"] {
            entries.push(db.with_extension(format!("db{suffix}")));
            drop(create_private_file(entries.last().unwrap()).unwrap());
        }
    }
    for path in &entries {
        fs::write(path, b"unchanged").unwrap();
    }
    DataDir::resolve(Some(relative)).unwrap();
    for path in entries {
        assert_eq!(fs::read(path).unwrap(), b"unchanged");
    }
    let file = temp.path().join("file");
    fs::write(&file, b"preserved").unwrap();
    assert!(DataDir::resolve(Some(file.clone())).is_err());
    assert_eq!(fs::read(file).unwrap(), b"preserved");
}

#[cfg(unix)]
#[test]
fn preparation_rejects_linked_databases_and_sidecars() {
    use std::os::unix::fs::symlink;
    for name in [
        "config.db",
        "history.db",
        "config.db-wal",
        "config.db-shm",
        "history.db-wal",
        "history.db-shm",
    ] {
        for hard in [false, true] {
            let temp = tempfile::tempdir().unwrap();
            let root = temp.path().join("data");
            create_private_dir(&root).unwrap();
            let target = temp.path().join("private-target/target");
            create_private_file(&target)
                .unwrap()
                .write_all(b"preserved")
                .unwrap();
            let link = root.join(name);
            if hard {
                fs::hard_link(&target, &link).unwrap();
            } else {
                symlink(&target, &link).unwrap();
            }
            let error = prepare(root).unwrap_err();
            assert!(
                error
                    .to_string()
                    .contains(if hard { "hard-linked" } else { "symlink" })
            );
            assert_eq!(fs::read(target).unwrap(), b"preserved");
            assert!(fs::symlink_metadata(link).is_ok());
        }
    }
}

#[cfg(unix)]
#[test]
fn access_denial_keeps_raw_os_error() {
    use std::os::unix::fs::PermissionsExt;
    let temp = tempfile::tempdir().unwrap();
    let parent = temp.path().join("denied");
    fs::create_dir(&parent).unwrap();
    fs::set_permissions(&parent, fs::Permissions::from_mode(0o0)).unwrap();
    let denied = match fs::read_dir(&parent) {
        Ok(_) => {
            fs::set_permissions(&parent, fs::Permissions::from_mode(0o700)).unwrap();
            eprintln!("SKIPPED denial assertion: host can read the owned 0o000 fixture");
            return;
        }
        Err(error) => error,
    };
    let result = prepare(parent.join("data"));
    fs::set_permissions(&parent, fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(denied.kind(), std::io::ErrorKind::PermissionDenied);
    let error = result.unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
    assert!(error.raw_os_error().is_some());
    assert_eq!(error.raw_os_error(), denied.raw_os_error());
    eprintln!("actual fixture denial: {error:?}; read_dir: {denied:?}");
}
