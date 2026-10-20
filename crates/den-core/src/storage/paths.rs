//! Private storage primitives for database precreation, backups and exports.
//!
//! Existing ancestors are trusted, including OS symlinks such as macOS `/var`.
//! Final targets must not be symlinks; Unix files must have one hard link and
//! exact private mode bits. Ownership, ACLs and concurrent ancestor/target swaps
//! are not secured by these path-based checks. Callers must control the parent
//! namespace, keep returned file handles for writes, and revalidate before reuse.
//! No existing data or permissions are repaired. Non-Unix mode checks are no-ops;
//! callers rely on the per-user directory ACL there.

use std::fs::{self, DirBuilder, File, Metadata, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};

/// Create missing directories privately, or validate an existing final directory.
/// Existing ancestors need not be private; every newly created component is.
/// A failure may leave newly created empty directories in place.
pub fn create_private_dir(path: &Path) -> io::Result<()> {
    let mut missing = Vec::new();
    let mut ancestor = path;
    loop {
        match fs::symlink_metadata(ancestor) {
            Ok(_) => break,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                missing.push(ancestor);
                ancestor = ancestor
                    .parent()
                    .filter(|p| !p.as_os_str().is_empty())
                    .unwrap_or(Path::new("."));
            }
            Err(e) => return Err(e),
        }
    }
    for directory in missing.into_iter().rev() {
        // Earlier creations can make `..` and following ancestors resolve.
        // Recheck with the OS; never collapse components through symlinks.
        match fs::symlink_metadata(directory) {
            Ok(_) => continue,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
        let mut builder = DirBuilder::new();
        private_mode(ModeTarget::Directory(&mut builder))?;
        match builder.create(directory) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e),
        }
        validate_private_dir(directory)?;
    }
    validate_private_dir(path)
}

/// Exclusively create an empty file, including private missing parent directories.
/// With a valid parent, an existing final entry returns AlreadyExists unopened.
/// Keep this handle for backup/export writes; SQLite may reopen the empty DB.
/// A restrictive umask can cause validation to fail, leaving a new empty file.
pub fn create_private_file(path: &Path) -> io::Result<File> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    create_private_dir(parent)?;
    let mut options = OpenOptions::new();
    options.read(true).write(true).create_new(true);
    private_mode(ModeTarget::File(&mut options))?;
    let file = options.open(path)?;
    private_mode(ModeTarget::Existing(&file.metadata()?, false))?;
    Ok(file)
}

/// Validate an existing directory without modifying it or following a final symlink.
pub fn validate_private_dir(path: &Path) -> io::Result<()> {
    validate(path, true)
}

/// Validate a regular file without opening it. This is a snapshot, not a safe-open API.
pub fn validate_private_file(path: &Path) -> io::Result<()> {
    validate(path, false)
}

fn validate(path: &Path, directory: bool) -> io::Result<()> {
    // Strip trailing separators/dot components so a final symlink stays visible.
    let normalized: PathBuf = path.components().collect();
    let metadata = fs::symlink_metadata(normalized)?;
    let result = if metadata.file_type().is_symlink() {
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "symlink target refused",
        ))
    } else if (directory && !metadata.is_dir()) || (!directory && !metadata.is_file()) {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "inappropriate storage target type",
        ))
    } else {
        private_mode(ModeTarget::Existing(&metadata, directory))
    };
    result.map_err(|e| {
        io::Error::new(
            e.kind(),
            format!("unsafe storage target {}: {e}", path.display()),
        )
    })
}

enum ModeTarget<'a> {
    Directory(&'a mut DirBuilder),
    File(&'a mut OpenOptions),
    Existing(&'a Metadata, bool),
}

fn private_mode(target: ModeTarget<'_>) -> io::Result<()> {
    // Read each variant on every platform; the mode operation is Unix-only.
    match &target {
        ModeTarget::Directory(builder) => {
            let _ = builder;
        }
        ModeTarget::File(options) => {
            let _ = options;
        }
        ModeTarget::Existing(metadata, directory) => {
            let _ = (metadata, directory);
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
        match target {
            ModeTarget::Directory(builder) => {
                builder.mode(0o700);
            }
            ModeTarget::File(options) => {
                options.mode(0o600);
            }
            ModeTarget::Existing(metadata, directory) => {
                let expected = if directory { 0o700 } else { 0o600 };
                let actual = metadata.permissions().mode() & 0o7777;
                // mkdir may inherit setgid; on directories it grants no access.
                // Ignore only that bit, retaining strict access, setuid and sticky checks.
                let checked = if directory { actual & !0o2000 } else { actual };
                if checked != expected {
                    return Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        format!("unsafe mode {actual:04o}; expected {expected:04o}"),
                    ));
                }
                if !directory && metadata.nlink() != 1 {
                    return Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "hard-linked file refused",
                    ));
                }
            }
        }
    }
    Ok(())
}

/// Prepared owned paths, before advisory discovery or SQLite opening.
/// This startup-only building block performs blocking IO on a dedicated `std::thread`
/// outside any entered Tokio context. Tokio `spawn_blocking` is also rejected.
/// Validation is a snapshot: trusted ancestors, ownership/ACL and TOCTOU limits above apply.
/// Failure may leave newly created empty directories, DBs or owned access probes;
/// existing bytes/modes are kept. No foreign entries are removed on failure.
#[derive(Debug)]
pub struct PreparedPaths {
    root: PathBuf,
}

impl PreparedPaths {
    /// Inject environment/current-directory readers; lower-priority inputs are lazy.
    /// The final root is validated before canonicalization; `..` follows OS semantics.
    pub fn prepare(
        explicit: Option<PathBuf>,
        mut environment: impl FnMut(&str) -> Option<std::ffi::OsString>,
        current_dir: impl FnOnce() -> io::Result<PathBuf>,
    ) -> io::Result<Self> {
        if tokio::runtime::Handle::try_current().is_ok() {
            return Err(io::Error::other(
                "storage preparation requires a thread outside Tokio",
            ));
        }
        let selected = select_path(explicit, std::env::consts::OS, &mut environment)?;
        let absolute = if selected.is_absolute() {
            selected
        } else {
            current_dir()?.join(selected)
        };
        if !absolute.is_absolute() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "data directory must resolve to an absolute path",
            ));
        }
        create_private_dir(&absolute)?;
        let paths = Self {
            root: fs::canonicalize(absolute)?,
        };
        for dir in [paths.root(), paths.backups(), paths.exports()] {
            create_private_dir(&dir)?;
            probe_directory(&dir)?;
        }
        for db in [paths.config(), paths.history()] {
            match validate_private_file(&db) {
                Ok(()) => drop(OpenOptions::new().read(true).write(true).open(&db)?),
                Err(e) if e.kind() == io::ErrorKind::NotFound => drop(create_private_file(&db)?),
                Err(e) => return Err(e),
            }
            for suffix in ["-wal", "-shm"] {
                let mut sidecar = db.as_os_str().to_os_string();
                sidecar.push(suffix);
                let sidecar = PathBuf::from(sidecar);
                match validate_private_file(&sidecar) {
                    Ok(()) => drop(OpenOptions::new().read(true).write(true).open(sidecar)?),
                    Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                    Err(e) => return Err(e),
                }
            }
        }
        Ok(paths)
    }

    pub fn root(&self) -> PathBuf {
        self.root.clone()
    }
    pub fn config(&self) -> PathBuf {
        self.root.join("config.db")
    }
    pub fn history(&self) -> PathBuf {
        self.root.join("history.db")
    }
    pub fn backups(&self) -> PathBuf {
        self.root.join("backups")
    }
    pub fn exports(&self) -> PathBuf {
        self.root.join("exports")
    }
}

fn select_path(
    explicit: Option<PathBuf>,
    platform: &str,
    environment: &mut impl FnMut(&str) -> Option<std::ffi::OsString>,
) -> io::Result<PathBuf> {
    let path = explicit.or_else(|| environment("STREAMDEN_DATA_DIR").map(PathBuf::from));
    let path = match path {
        Some(path) => path,
        None => {
            let mut absolute = |key| {
                environment(key)
                    .map(PathBuf::from)
                    .filter(|p| p.is_absolute())
            };
            match platform {
                "linux" => absolute("XDG_DATA_HOME")
                    .map(|p| p.join("streamden"))
                    .or_else(|| absolute("HOME").map(|p| p.join(".local/share/streamden"))),
                "windows" => absolute("LOCALAPPDATA").map(|p| p.join("StreamDen")),
                "macos" => {
                    absolute("HOME").map(|p| p.join("Library/Application Support/StreamDen"))
                }
                _ => None,
            }
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "missing absolute platform data directory",
                )
            })?
        }
    };
    if path.as_os_str().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "empty data directory",
        ));
    }
    Ok(path)
}

fn probe_directory(dir: &Path) -> io::Result<()> {
    use std::io::Write;
    fs::read_dir(dir)?.next().transpose()?;
    let path = dir.join(format!(".streamden-access-{}", uuid::Uuid::now_v7()));
    let mut file = create_private_file(&path)?;
    let result = file.write_all(b"probe");
    drop(file);
    let cleanup = fs::remove_file(path);
    result.and(cleanup)
}

#[cfg(test)]
mod resolution_tests {
    use super::*;
    use std::ffi::OsString;

    #[test]
    fn precedence_defaults_and_invalid_inputs() {
        let base = std::env::current_dir().unwrap();
        let value = base.clone().into_os_string();
        let mut never = |_: &str| -> Option<OsString> { panic!("unexpected environment read") };
        assert_eq!(
            select_path(Some(base.clone()), "unused", &mut never).unwrap(),
            base
        );
        assert!(select_path(Some(PathBuf::new()), "linux", &mut never).is_err());
        let mut override_only = |key: &str| {
            assert_eq!(key, "STREAMDEN_DATA_DIR");
            Some(value.clone())
        };
        assert_eq!(
            select_path(None, "unused", &mut override_only).unwrap(),
            base
        );
        assert!(select_path(None, "unused", &mut |_| Some(OsString::new())).is_err());
        for (platform, variable, suffix) in [
            ("linux", "XDG_DATA_HOME", "streamden"),
            ("linux", "HOME", ".local/share/streamden"),
            ("windows", "LOCALAPPDATA", "StreamDen"),
            ("macos", "HOME", "Library/Application Support/StreamDen"),
        ] {
            let mut env = |key: &str| (key == variable).then(|| value.clone());
            assert_eq!(
                select_path(None, platform, &mut env).unwrap(),
                base.join(suffix)
            );
            for bad in ["", "relative"] {
                assert!(
                    select_path(None, platform, &mut |key| (key == variable)
                        .then(|| OsString::from(bad)))
                    .is_err()
                );
            }
            assert!(select_path(None, platform, &mut |_| None).is_err());
        }
        for xdg in ["", "relative"] {
            let mut env = |key: &str| match key {
                "XDG_DATA_HOME" => Some(OsString::from(xdg)),
                "HOME" => Some(value.clone()),
                _ => None,
            };
            assert_eq!(
                select_path(None, "linux", &mut env).unwrap(),
                base.join(".local/share/streamden")
            );
        }
    }
}

#[cfg(test)]
#[test]
fn directory_probe_preserves_actual_os_errors() {
    let temp = tempfile::tempdir().unwrap();
    let file = temp.path().join("file");
    fs::write(&file, b"untouched").unwrap();
    let error = probe_directory(&file).unwrap_err();
    assert!(error.raw_os_error().is_some());
    assert_eq!(fs::read(file).unwrap(), b"untouched");
}

/// Resolved startup storage paths and advisory warnings from Spec 03 §19.
/// Call on a dedicated `std::thread` outside entered Tokio (including
/// `spawn_blocking`). Mount enumeration can block in statvfs indefinitely;
/// this synchronous API offers no cancellation or latency bound. Detection is
/// heuristic, including on Windows: mapped remote-drive coverage is not promised.
/// PreparedPaths' snapshot, ACL, trusted-namespace and failure-leftover limits apply.
#[derive(Debug)]
pub struct DataDir {
    paths: PreparedPaths,
    warnings: Vec<&'static str>,
}

impl DataDir {
    pub fn resolve(explicit: Option<PathBuf>) -> io::Result<Self> {
        Self::resolve_with(
            explicit,
            |key| std::env::var_os(key),
            std::env::current_dir,
            || {
                mountpoints::mountinfos()
                    .map(|mounts| mounts.into_iter().map(|m| (m.path, m.format)).collect())
                    .map_err(|e| e.to_string())
            },
        )
    }

    fn resolve_with(
        explicit: Option<PathBuf>,
        environment: impl FnMut(&str) -> Option<std::ffi::OsString>,
        current_dir: impl FnOnce() -> io::Result<PathBuf>,
        discovery: impl FnOnce() -> Result<Vec<MountRecord>, String>,
    ) -> io::Result<Self> {
        // Preparation rejects entered runtimes before reading any input or doing IO.
        let paths = PreparedPaths::prepare(explicit, environment, current_dir)?;
        let warnings = discover(&paths.root, discovery());
        Ok(Self { paths, warnings })
    }

    pub fn root(&self) -> PathBuf {
        self.paths.root()
    }
    pub fn config(&self) -> PathBuf {
        self.paths.config()
    }
    pub fn history(&self) -> PathBuf {
        self.paths.history()
    }
    pub fn backups(&self) -> PathBuf {
        self.paths.backups()
    }
    pub fn exports(&self) -> PathBuf {
        self.paths.exports()
    }
    pub fn warnings(&self) -> &[&'static str] {
        &self.warnings
    }
}

type MountRecord = (PathBuf, Option<String>);

fn mount_path(path: &Path) -> PathBuf {
    let spelling = path.as_os_str().to_string_lossy();
    // Windows canonicalization uses verbatim roots; mount records may not.
    if spelling
        .get(..8)
        .is_some_and(|s| s.eq_ignore_ascii_case(r"\\?\UNC\"))
    {
        return PathBuf::from(format!(r"\\{}", &spelling[8..]));
    }
    spelling
        .strip_prefix(r"\\?\")
        .map_or_else(|| path.to_owned(), PathBuf::from)
}

fn is_unc(path: &Path) -> bool {
    let spelling = path.as_os_str().to_string_lossy().replace('\\', "/");
    let lower = spelling.to_ascii_lowercase();
    let rest = if let Some(rest) = lower.strip_prefix("//?/unc/") {
        rest
    } else if lower.starts_with("//?/") || lower.starts_with("//./") {
        return false;
    } else if let Some(rest) = lower.strip_prefix("//") {
        rest
    } else {
        return false;
    };
    let mut parts = rest.split('/');
    parts.next().is_some_and(|p| !p.is_empty()) && parts.next().is_some_and(|p| !p.is_empty())
}

fn discover(root: &Path, records: Result<Vec<MountRecord>, String>) -> Vec<&'static str> {
    let mut network = is_unc(root);
    match records {
        Ok(records) => {
            let root = mount_path(root);
            let selected = records
                .iter()
                .filter(|(path, _)| root.starts_with(mount_path(path)))
                .max_by_key(|(path, _)| mount_path(path).components().count());
            match selected {
                Some((_, Some(format))) => {
                    let format = format.to_ascii_lowercase();
                    if [
                        "nfs",
                        "nfs4",
                        "cifs",
                        "smb",
                        "smbfs",
                        "smb3",
                        "9p",
                        "sshfs",
                        "fuse.sshfs",
                        "afpfs",
                        "davfs",
                        "davfs2",
                        "fuse.davfs",
                    ]
                    .contains(&format.as_str())
                    {
                        network = true;
                    } else {
                        tracing::debug!(
                            format,
                            "mount format not classified as network; advisory only"
                        );
                    }
                }
                Some((_, None)) => {
                    tracing::debug!("mount format missing; network status incomplete")
                }
                None => tracing::debug!("no matching mount; network status incomplete"),
            }
        }
        Err(error) => tracing::debug!(error, "mount enumeration failed; network status incomplete"),
    }
    let spelling = root.as_os_str().to_string_lossy();
    let synced = spelling.split(['/', '\\']).any(|part| {
        [
            "Dropbox",
            "OneDrive",
            "iCloud Drive",
            "iCloudDrive",
            "com~apple~CloudDocs",
            "Nextcloud",
        ]
        .iter()
        .any(|marker| part.eq_ignore_ascii_case(marker))
    });
    let mut warnings = Vec::new();
    if network {
        warnings.push("network_share");
    }
    if synced {
        warnings.push("synced_folder");
    }
    warnings
}

#[cfg(test)]
mod discovery_tests {
    use super::*;

    fn resolve(root: PathBuf, format: Option<&str>) -> DataDir {
        DataDir::resolve_with(
            Some(root.clone()),
            |_| panic!("env read"),
            || panic!("cwd read"),
            || {
                Ok(vec![(
                    fs::canonicalize(root).unwrap(),
                    format.map(str::to_owned),
                )])
            },
        )
        .unwrap()
    }

    #[test]
    fn network_and_sync_are_warnings_without_refusal() {
        let temp = tempfile::tempdir().unwrap();
        for format in [
            "nfs",
            "nfs4",
            "cifs",
            "smb",
            "smbfs",
            "smb3",
            "9p",
            "sshfs",
            "fuse.sshfs",
            "afpfs",
            "davfs",
            "davfs2",
            "fuse.davfs",
        ] {
            for marker in [
                "Dropbox",
                "OneDrive",
                "iCloud Drive",
                "iCloudDrive",
                "com~apple~CloudDocs",
                "Nextcloud",
            ] {
                let paths = resolve(
                    temp.path().join(format).join(marker),
                    Some(&format.to_uppercase()),
                );
                assert_eq!(paths.warnings(), ["network_share", "synced_folder"]);
                assert!(paths.config().is_file() && paths.history().is_file());
            }
        }
        for format in [None, Some("ext4"), Some("unknown")] {
            assert!(
                resolve(temp.path().join("local/Dropboxish"), format)
                    .warnings()
                    .is_empty()
            );
        }
    }

    #[test]
    fn longest_mount_boundaries_and_incomplete_discovery() {
        let root = std::env::current_dir().unwrap().join("network/local/data");
        let network = root.parent().unwrap().parent().unwrap().to_owned();
        let local = root.parent().unwrap().to_owned();
        let records = vec![
            (network.clone(), Some("nfs".into())),
            (local, Some("ext4".into())),
        ];
        assert!(discover(&root, Ok(records)).is_empty());
        assert!(
            discover(
                &network.with_file_name("network-other"),
                Ok(vec![(network.clone(), Some("nfs".into()))])
            )
            .is_empty()
        );
        for records in [
            Ok(vec![]),
            Ok(vec![(network.clone(), None)]),
            Ok(vec![(network.clone(), Some("futurefs".into()))]),
            Err("unavailable".into()),
        ] {
            assert!(discover(&root, records).is_empty());
        }
        for path in [
            r"\\server\share\data",
            r"\\?\UNC\server\share\data",
            "//server/share/data",
        ] {
            assert_eq!(
                discover(Path::new(path), Err("unavailable".into())),
                ["network_share"]
            );
        }
        for path in [r"\\?\C:\data", r"\\.\C:\data", "local/data", "//server"] {
            assert!(!is_unc(Path::new(path)));
        }
        assert_eq!(mount_path(Path::new(r"\\?\C:\data")), Path::new(r"C:\data"));
        assert_eq!(
            mount_path(Path::new(r"\\?\UNC\server\share")),
            Path::new(r"\\server\share")
        );
    }

    #[test]
    fn runtime_gate_precedes_all_inputs_including_discovery() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let attempt = || {
            DataDir::resolve_with(
                None,
                |_| panic!("env"),
                || panic!("cwd"),
                || panic!("mounts"),
            )
        };
        {
            let _entered = runtime.enter();
            assert!(attempt().unwrap_err().to_string().contains("outside Tokio"));
            assert!(DataDir::resolve(None).is_err());
        }
        runtime.block_on(async {
            tokio::task::spawn_blocking(move || {
                assert!(attempt().unwrap_err().to_string().contains("outside Tokio"));
            })
            .await
            .unwrap();
        });
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("Dropbox");
        let data = DataDir::resolve_with(
            Some(root),
            |_| unreachable!(),
            || unreachable!(),
            || Err("enumeration failure".into()),
        )
        .unwrap();
        assert_eq!(data.warnings(), ["synced_folder"]);
    }
}
