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
                if actual != expected {
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
