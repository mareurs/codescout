//! A directory only the current user (or root) can put files in, and the checks that say so.
//!
//! Shared by the attribution engine's extraction directory
//! (`src/tools/run_command/attribution.rs`) and the per-user runtime directory that holds the
//! peer, mux and index-lock sockets (`src/socket_discovery.rs`). Both place files under a
//! predictable name in a shared temp directory, so both need the same answer to "is what is at
//! that name really ours?".

use std::path::Path;

pub(crate) type Uid = u32;

pub(crate) fn effective_uid() -> Uid {
    #[cfg(unix)]
    {
        // SAFETY: geteuid has no preconditions and cannot fail.
        unsafe { libc::geteuid() }
    }
    #[cfg(not(unix))]
    {
        0
    }
}

/// Why [`verify_private_dir`] will not trust a directory. Rendered into the log line that
/// accompanies a fallback, so each variant names what an operator would act on.
#[derive(Debug, PartialEq, Eq)]
pub enum DirRefusal {
    /// Missing, unreadable, a symlink (including one to a perfectly good directory), or a
    /// non-directory squatting on the name. `symlink_metadata` is what makes a link visible:
    /// plain `metadata` would follow it and report the target's owner and mode.
    NotARealDirectory,
    /// Owned by someone other than the effective user, who can therefore put anything in it.
    #[cfg(unix)]
    ForeignOwner { owner: Uid, expected: Uid },
    /// Writable by group or other, so a third party can plant a module beside the scripts.
    #[cfg(unix)]
    GroupOrOtherWritable { mode: u32 },
}

impl std::fmt::Display for DirRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotARealDirectory => write!(f, "not a real directory (missing, symlink or file)"),
            #[cfg(unix)]
            Self::ForeignOwner { owner, expected } => {
                write!(f, "owned by uid {owner}, not by uid {expected}")
            }
            #[cfg(unix)]
            Self::GroupOrOtherWritable { mode } => {
                write!(f, "mode {mode:04o} is writable by group or other")
            }
        }
    }
}

/// Is `dir` a directory only `uid` (or root) can put files in?
///
/// The scripts in it run as this user, so anyone who can add a file beside them can run code
/// as this user: Python puts the script's directory first on `sys.path`. Checked on EVERY use,
/// not only on creation, because on a shared `/tmp` another user may have created the name
/// first — the check is the trust, the name is not.
///
/// Unix only beyond the symlink check: owner and mode bits do not exist elsewhere, and a
/// Windows `%TEMP%` is per-user to begin with. The `cfg(not(unix))` arm is a subset of the
/// unix one for TYPES, not for LINTS: the Windows cross-clippy
/// (`scripts/build-windows.sh clippy --all-targets -- -D warnings`) is what finds the second
/// kind, and found an `unused_mut` in [`create_private_dir`]. Run it after touching this.
pub(crate) fn verify_private_dir(dir: &Path, uid: Uid) -> Result<(), DirRefusal> {
    let meta = std::fs::symlink_metadata(dir).map_err(|_| DirRefusal::NotARealDirectory)?;
    if !meta.file_type().is_dir() {
        return Err(DirRefusal::NotARealDirectory);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let owner = meta.uid();
        if owner != uid {
            return Err(DirRefusal::ForeignOwner {
                owner,
                expected: uid,
            });
        }
        let mode = meta.mode() & 0o777;
        if mode & 0o022 != 0 {
            return Err(DirRefusal::GroupOrOtherWritable { mode });
        }
    }
    #[cfg(not(unix))]
    let _ = uid;
    Ok(())
}

/// Create `dir` with mode 0700; an existing entry of that name is NOT an error here — it is
/// [`verify_private_dir`]'s to judge. Not recursive, so the mode applies to the directory
/// that matters and never to a parent created on the way.
pub(crate) fn create_private_dir(dir: &Path) -> std::io::Result<()> {
    // `mut` is needed only where the mode is set; on Windows clippy would call it unused.
    #[cfg_attr(not(unix), allow(unused_mut))]
    let mut builder = std::fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    match builder.create(dir) {
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
        other => other,
    }
}
