//! One cooperative lease for the read/check/backup/replace/anchor transaction.
//! External writers that ignore this lease are outside this guarantee.
use super::MemoryStore;
use anyhow::{bail, Context, Result};
use fs4::fs_std::FileExt;
use std::fs::{File, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub(super) struct WriteTransaction {
    // Independent open-file descriptions contend even inside the same process.
    // Keep this stable inode; unlinking it would split the lock population.
    _lease: File,
    path: PathBuf,
    relative: PathBuf,
    history: PathBuf,
    existing: Option<String>,
}

impl WriteTransaction {
    pub(super) fn begin(store: &MemoryStore, topic: &str) -> Result<Self> {
        Self::prepare(store, topic, true)
    }

    pub(super) fn begin_delete(store: &MemoryStore, topic: &str) -> Result<Self> {
        Self::prepare(store, topic, false)
    }

    fn prepare(store: &MemoryStore, topic: &str, read_existing: bool) -> Result<Self> {
        std::fs::create_dir_all(store.dir())?;
        let root = store
            .dir()
            .canonicalize()
            .context("pin memory store root")?;
        let relative = store
            .topic_path(topic)
            .strip_prefix(store.dir())?
            .to_owned();
        if relative
            .components()
            .next()
            .is_some_and(|c| c.as_os_str() == ".history")
        {
            bail!("the .history memory namespace is reserved");
        }
        let history = root.join(".history");
        private_dir(&history)?;
        let lease = open_lock(&history.join("write.lock"))?;
        let started = Instant::now();
        loop {
            match lease.try_lock_exclusive() {
                Ok(()) => break,
                Err(e) if e.raw_os_error() == fs4::lock_contended_error().raw_os_error() => {
                    if started.elapsed() >= Duration::from_secs(5) {
                        bail!("memory write lock timed out; another process is writing this store");
                    }
                    std::thread::sleep(Duration::from_millis(25));
                }
                Err(e) => return Err(e).context("acquire memory write lock"),
            }
        }
        ensure_ignore(&history)?;
        let path = root.join(&relative);
        safe_topic_parent(&root, relative.parent().unwrap_or(Path::new("")))?;
        let exists = match std::fs::symlink_metadata(&path) {
            Ok(m) if m.file_type().is_file() => true,
            Ok(_) => bail!("memory target must be a regular file: {}", path.display()),
            Err(e) if e.kind() == ErrorKind::NotFound => false,
            Err(e) => return Err(e).context("inspect existing memory"),
        };
        let existing = if exists && read_existing {
            Some(
                String::from_utf8(
                    std::fs::read(&path).context("read existing memory before overwrite")?,
                )
                .context("existing memory is not valid UTF-8; preserve it and repair explicitly")?,
            )
        } else {
            None
        };
        Ok(Self {
            _lease: lease,
            path,
            relative,
            history,
            existing,
        })
    }

    pub(super) fn dir(&self) -> &Path {
        self.history
            .parent()
            .expect("history is below the pinned memory root")
    }

    pub(super) fn existing(&self) -> Option<&str> {
        self.existing.as_deref()
    }

    pub(super) fn write(&self, content: &str) -> Result<()> {
        if let Some(old) = &self.existing {
            let mut backup_dir = self.history.clone();
            for component in self.relative.components() {
                backup_dir.push(component.as_os_str());
                private_dir(&backup_dir)?;
            }
            let backup = backup_dir.join(format!("{}.md.bak", uuid::Uuid::new_v4()));
            // Never expose partial bytes under a recoverable history filename.
            let mut file = tempfile::Builder::new()
                .prefix(".backup-")
                .suffix(".pending")
                .tempfile_in(&backup_dir)
                .context("create exclusive memory backup staging")?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                file.as_file()
                    .set_permissions(std::fs::Permissions::from_mode(0o600))?;
            }
            file.as_file_mut()
                .write_all(old.as_bytes())
                .context("write exact memory backup")?;
            file.as_file().sync_all().context("sync memory backup")?;
            file.persist_noclobber(&backup)
                .map_err(|e| e.error)
                .context("publish complete memory backup")?;
            sync_directory(&backup_dir)?;
        }
        let parent = self.path.parent().context("memory target has no parent")?;
        let mut staged = tempfile::NamedTempFile::new_in(parent).context("stage memory write")?;
        if let Ok(meta) = std::fs::symlink_metadata(&self.path) {
            staged.as_file().set_permissions(meta.permissions())?;
        }
        staged.as_file_mut().write_all(content.as_bytes())?;
        staged.as_file().sync_all()?;
        staged
            .persist(&self.path)
            .map_err(|e| e.error)
            .context("publish memory write")?;
        Ok(())
    }

    pub(super) fn delete(&self) -> Result<()> {
        match std::fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}

fn private_dir(path: &Path) -> Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(m) if m.file_type().is_dir() => {}
        Ok(_) => bail!(
            "memory history must contain real directories: {}",
            path.display()
        ),
        Err(e) if e.kind() == ErrorKind::NotFound => {
            let mut builder = std::fs::DirBuilder::new();
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                builder.mode(0o700);
            }
            match builder.create(path) {
                Ok(()) => {}
                Err(e) if e.kind() == ErrorKind::AlreadyExists => {
                    let m = std::fs::symlink_metadata(path)?;
                    if !m.file_type().is_dir() {
                        bail!("unsafe memory history directory: {}", path.display());
                    }
                }
                Err(e) => return Err(e.into()),
            }
        }
        Err(e) => return Err(e.into()),
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))?;
    }
    sync_directory(path.parent().context("history directory has no parent")?)?;
    Ok(())
}

fn safe_topic_parent(root: &Path, relative: &Path) -> Result<()> {
    let mut path = root.to_owned();
    for component in relative.components() {
        path.push(component.as_os_str());
        match std::fs::symlink_metadata(&path) {
            Ok(m) if m.file_type().is_dir() => {}
            Ok(_) => bail!("memory parent must be a real directory: {}", path.display()),
            Err(e) if e.kind() == ErrorKind::NotFound => match std::fs::create_dir(&path) {
                Ok(()) => {}
                Err(e) if e.kind() == ErrorKind::AlreadyExists => {
                    if !std::fs::symlink_metadata(&path)?.file_type().is_dir() {
                        bail!("unsafe memory parent: {}", path.display());
                    }
                }
                Err(e) => return Err(e.into()),
            },
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}

fn ensure_ignore(history: &Path) -> Result<()> {
    let path = history.join(".gitignore");
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    match options.open(&path) {
        Ok(mut file) => {
            file.write_all(b"*\n")?;
            file.sync_all()?;
        }
        Err(e) if e.kind() == ErrorKind::AlreadyExists => {
            if !std::fs::symlink_metadata(&path)?.file_type().is_file() {
                bail!("memory history .gitignore must be a regular file");
            }
            if std::fs::read(&path)? != b"*\n" {
                bail!("memory history .gitignore must ignore all history files (*)");
            }
        }
        Err(e) => return Err(e).context("protect memory history from git"),
    }
    sync_directory(history)?;
    Ok(())
}

fn open_lock(path: &Path) -> Result<File> {
    if let Ok(m) = std::fs::symlink_metadata(path) {
        if !m.file_type().is_file() {
            bail!("memory write lock must be a regular file");
        }
    }
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    let file = options
        .open(path)
        .context("open stable memory write lock")?;
    if !file.metadata()?.file_type().is_file() {
        bail!("memory write lock must be a regular file");
    }
    Ok(file)
}

fn sync_directory(path: &Path) -> Result<()> {
    #[cfg(unix)]
    File::open(path)?
        .sync_all()
        .context("sync memory history directory")?;
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}
