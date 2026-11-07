//! Cooperative provenance fence, separate from Codescout's ordinary write lock.
//! Close descriptors on drop; never explicitly unlock an inherited open-file
//! description while its formatter process may still hold a duplicate.
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

pub const LOCK_NAME: &str = "native-provenance-v1.lock";

#[derive(Debug)]
pub struct Fence {
    file: File,
    root: PathBuf,
    #[cfg(test)]
    path: PathBuf,
    counter_path: PathBuf,
    generation: u64,
}

fn path(root: &Path) -> io::Result<PathBuf> {
    Ok(root.canonicalize()?.join(".codescout").join(LOCK_NAME))
}

fn open(path: &Path, create: bool) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options
        .read(true)
        .write(create)
        .create(create)
        .truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = options.open(path)?;
    if !file.metadata()?.is_file() {
        return Err(io::Error::other(
            "native provenance fence is not a regular file",
        ));
    }
    Ok(file)
}

fn counter(file: &mut File) -> io::Result<u64> {
    file.seek(SeekFrom::Start(0))?;
    let mut value = Vec::new();
    file.take(9).read_to_end(&mut value)?;
    match value.len() {
        0 => Ok(0),
        8 => Ok(u64::from_le_bytes(
            value
                .try_into()
                .map_err(|_| io::Error::other("invalid fence counter"))?,
        )),
        _ => Err(io::Error::other(
            "native provenance fence counter is malformed",
        )),
    }
}

pub fn coordination_root() -> io::Result<PathBuf> {
    #[cfg(not(unix))]
    {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "native coordination requires Unix",
        ))
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
        // SAFETY: geteuid has no preconditions.
        let uid = unsafe { libc::geteuid() };
        #[cfg(test)]
        let root = Path::new("/tmp").join(format!(
            "codescout-native-provenance-test-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        #[cfg(not(test))]
        let root = {
            let mut entry = std::mem::MaybeUninit::<libc::passwd>::uninit();
            let mut result = std::ptr::null_mut();
            let mut buffer = vec![0u8; 65536];
            // SAFETY: the passwd and buffer allocations remain live through this call
            // and the returned pw_dir is copied before either allocation is dropped.
            let status = unsafe {
                libc::getpwuid_r(
                    uid,
                    entry.as_mut_ptr(),
                    buffer.as_mut_ptr().cast(),
                    buffer.len(),
                    &mut result,
                )
            };
            if status != 0 || result.is_null() {
                return Err(io::Error::other(
                    "native coordinator account home unavailable",
                ));
            }
            // SAFETY: a successful getpwuid_r initialized entry and pw_dir.
            let directory_pointer = unsafe { (*result).pw_dir };
            if directory_pointer.is_null() {
                return Err(io::Error::other(
                    "native coordinator account home is missing",
                ));
            }
            // SAFETY: the non-null pw_dir belongs to the live getpwuid_r buffer.
            let directory = unsafe { std::ffi::CStr::from_ptr(directory_pointer) }.to_bytes();
            use std::os::unix::ffi::OsStrExt;
            let home = Path::new(std::ffi::OsStr::from_bytes(directory));
            if !home.is_absolute() {
                return Err(io::Error::other(
                    "native coordinator account home is not absolute",
                ));
            }
            home.join(".local/state/codescout/native-provenance-v2")
        };
        match std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&root)
        {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
        let metadata = std::fs::symlink_metadata(&root)?;
        if !metadata.is_dir()
            || metadata.file_type().is_symlink()
            || metadata.uid() != uid
            || metadata.permissions().mode() & 0o077 != 0
        {
            return Err(io::Error::other(
                "native coordination directory is not private and owned",
            ));
        }
        root.canonicalize()
    }
}

pub fn has_unverified_shell(root: &Path) -> io::Result<bool> {
    match std::fs::symlink_metadata(root.join(".codescout/unverified-shell-v1")) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

pub fn mark_unverified_shell(root: &Path) -> io::Result<()> {
    let directory = root.join(".codescout");
    std::fs::create_dir_all(&directory)?;
    let path = directory.join("unverified-shell-v1");
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    match options.open(&path) {
        Ok(mut file) => {
            file.write_all(b"UNKNOWN: arbitrary shell descendants are not certified stopped\\n")?;
            file.sync_all()?;
            File::open(directory)?.sync_all()?;
            Ok(())
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Ok(()),
        Err(error) => Err(error),
    }
}

pub fn canonical_checkout(directory: &Path) -> io::Result<PathBuf> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(directory)
        .args(["rev-parse", "--show-toplevel"])
        .output()?;
    if !output.status.success() {
        return Err(io::Error::other(
            "native provenance fencing requires a Git checkout",
        ));
    }
    let root = std::str::from_utf8(&output.stdout)
        .map_err(|_| io::Error::other("checkout path is not UTF-8"))?
        .trim_end_matches(['\r', '\n']);
    Path::new(root).canonicalize()
}

#[cfg(test)]
pub fn read_generation(root: &Path) -> io::Result<u64> {
    // A snapshot cannot be taken in the middle of a participating effect.
    Fence::try_acquire(root).map(|fence| fence.generation())
}

impl Fence {
    pub fn try_acquire(root: &Path) -> io::Result<Self> {
        Self::try_mode(root, true)
    }
    pub fn try_participate(root: &Path) -> io::Result<Self> {
        Self::try_mode(root, false)
    }

    fn try_mode(root: &Path, exclusive: bool) -> io::Result<Self> {
        #[cfg(not(unix))]
        {
            let _ = (root, exclusive);
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "native provenance fencing requires a supported Unix host",
            ))
        }
        #[cfg(unix)]
        {
            let root = root.canonicalize()?;
            let path = path(&root)?;
            std::fs::create_dir_all(
                path.parent()
                    .ok_or_else(|| io::Error::other("fence parent missing"))?,
            )?;
            let file = open(&path, true)?;
            if exclusive {
                file.try_lock()
            } else {
                file.try_lock_shared()
            }
            .map_err(io::Error::other)?;
            let counter_path = path.with_extension("counter");
            let mut counter_file = open(&counter_path, true)?;
            counter_file.lock()?;
            let generation = counter(&mut counter_file)?;
            if counter_file.metadata()?.len() == 0 {
                counter_file.write_all(&0u64.to_le_bytes())?;
                counter_file.sync_data()?;
            }
            Ok(Self {
                file,
                root,
                #[cfg(test)]
                path,
                counter_path,
                generation,
            })
        }
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }
    #[cfg(test)]
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn bump(&mut self) -> io::Result<u64> {
        // Ordinary participants share the effect fence, but increments must
        // still serialize. The formatter holds the effect fence exclusively.
        let mut file = open(&self.counter_path, true)?;
        file.lock()?;
        let generation = counter(&mut file)?
            .checked_add(1)
            .ok_or_else(|| io::Error::other("fence generation exhausted"))?;
        file.seek(SeekFrom::Start(0))?;
        file.write_all(&generation.to_le_bytes())?;
        file.sync_data()?;
        self.generation = generation;
        Ok(generation)
    }

    pub fn apply_to_command_named(
        &self,
        command: &mut std::process::Command,
        prefix: &str,
    ) -> io::Result<()> {
        #[cfg(not(unix))]
        {
            let _ = (command, prefix);
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "native fence inheritance unsupported",
            ))
        }
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            use std::os::unix::process::CommandExt;
            let inherited = self.file.try_clone()?;
            command.env(format!("{prefix}_FD"), inherited.as_raw_fd().to_string());
            command.env(format!("{prefix}_ROOT"), &self.root);
            command.env(format!("{prefix}_GENERATION"), self.generation.to_string());
            // SAFETY: only fcntl and raw fd access run after fork, before exec.
            // The owned File stays live in the child until exec inherits it.
            unsafe {
                command.pre_exec(move || {
                    if libc::fcntl(inherited.as_raw_fd(), libc::F_SETFD, 0) == -1 {
                        return Err(io::Error::last_os_error());
                    }
                    Ok(())
                });
            }
            Ok(())
        }
    }

    pub fn apply_to_command(&self, command: &mut std::process::Command) -> io::Result<()> {
        self.apply_to_command_named(command, "CODESCOUT_PROVENANCE_FENCE")
    }
}

#[cfg(test)]
#[path = "native_provenance_fence/tests.rs"]
mod tests;
