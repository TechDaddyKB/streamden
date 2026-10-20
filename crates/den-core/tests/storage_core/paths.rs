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
        for unsafe_mode in [0o644, 0o400, 0o1600] {
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
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(
            create_private_dir(&dir)
                .unwrap_err()
                .to_string()
                .contains("mode")
        );
        assert!(create_private_file(&dir.join("new")).is_err());
        assert!(!dir.join("new").exists());
        assert_eq!(mode(&dir), 0o755);
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
