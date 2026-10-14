//! File identity and path preflight for the explicit vector rebuild.
use anyhow::{ensure, Context, Result};
use std::fs::File;
use std::path::{Path, PathBuf};

pub(super) struct FileIdentity {
    path: PathBuf,
    handle: same_file::Handle,
}

impl FileIdentity {
    pub(super) fn pin(path: &Path) -> Result<Self> {
        ensure!(
            std::fs::symlink_metadata(path)?.file_type().is_file(),
            "memory database must be an existing regular file, not a symlink"
        );
        Self::reserved(path, File::open(path)?)
    }

    pub(super) fn reserved(path: &Path, file: File) -> Result<Self> {
        let identity = Self {
            path: path.to_path_buf(),
            handle: same_file::Handle::from_file(file)?,
        };
        identity.check()?;
        Ok(identity)
    }

    pub(super) fn check(&self) -> Result<()> {
        ensure!(
            std::fs::symlink_metadata(&self.path)?.file_type().is_file(),
            "database/backup file identity changed; rebuild refused"
        );
        ensure!(
            self.handle == same_file::Handle::from_path(&self.path)?,
            "database/backup file identity changed; rebuild refused"
        );
        Ok(())
    }

    pub(super) fn file(&self) -> &File {
        self.handle.as_file()
    }
    pub(super) fn path(&self) -> &Path {
        &self.path
    }
}

pub(super) fn validate_backup_path(source: &Path, backup: &Path) -> Result<()> {
    let parent = backup
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let parent = parent
        .canonicalize()
        .context("backup parent directory must already exist")?;
    let destination = parent.join(backup.file_name().context("backup path has no filename")?);
    ensure!(
        destination != source,
        "backup must differ from the source database"
    );
    if destination.parent() == source.parent() {
        let source_name = source.file_name().context("database has no filename")?;
        let backup_name = destination.file_name().expect("validated filename");
        let mut names = Vec::new();
        for suffix in ["-wal", "-shm", "-journal"] {
            let mut name = source_name.to_os_string();
            name.push(suffix);
            names.push(name);
        }
        let mut master_prefix = source_name.to_os_string();
        master_prefix.push("-mj");
        ensure!(
            !names.iter().any(|name| name == backup_name)
                && !backup_name
                    .as_encoded_bytes()
                    .starts_with(master_prefix.as_encoded_bytes()),
            "backup must not use a SQLite sidecar path"
        );
    }
    Ok(())
}

pub(super) fn require_non_wal_preview(source: &Path) -> Result<()> {
    use std::io::Read;
    let mut header = [0_u8; 20];
    File::open(source)?.read_exact(&mut header)?;
    ensure!(
        header[18] != 2 && header[19] != 2,
        "strict dry-run requires a coherent non-WAL copy; SQLite read-only access can change shared-memory files; no SQLite connection was opened"
    );
    Ok(())
}

pub(super) fn sync_parent(path: &Path) -> Result<()> {
    #[cfg(unix)]
    File::open(
        path.parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new(".")),
    )?
    .sync_all()?;
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}
