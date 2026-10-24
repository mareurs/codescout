//! Per-user socket-path discovery, shared by the LSP mux (`lsp::mux`) and the
//! peer-delegation server (`peer`). Transport-neutral: knows about per-user
//! runtime directories and workspace hashing, nothing about LSP or peers.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

/// Stable-within-a-build hash of a workspace root.
pub fn workspace_hash(workspace_root: &Path) -> String {
    let mut hasher = DefaultHasher::new();
    workspace_root.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// A directory for socket/lock files private to the current user.
pub fn per_user_runtime_dir() -> Result<PathBuf, RuntimeDirError> {
    #[cfg(unix)]
    {
        // SAFETY: getuid is always safe; returns the real UID.
        let name_uid = unsafe { libc::getuid() };
        runtime_dir_from(
            std::env::var_os("XDG_RUNTIME_DIR"),
            &std::env::temp_dir(),
            name_uid,
            crate::util::private_dir::effective_uid(),
        )
    }
    #[cfg(not(unix))]
    {
        Ok(std::env::temp_dir())
    }
}
/// Why the per-user runtime directory cannot be trusted.
///
/// A refusal, not a fallback: the peer, mux and index-lock sockets live in this directory and
/// are found again BY NAME, so a different directory chosen quietly would split processes that
/// must meet. The error names the path and the way out.
#[derive(Debug)]
#[cfg_attr(not(unix), allow(dead_code))]
pub enum RuntimeDirError {
    /// `temp_dir()` was empty or relative (`TMPDIR=""` returns `""` verbatim), so the directory
    /// would land under whatever the working directory is.
    NotAbsolute(PathBuf),
    /// The directory exists but is not private to this user.
    Refused {
        dir: PathBuf,
        why: crate::util::private_dir::DirRefusal,
    },
    /// The directory could not be created.
    Create {
        dir: PathBuf,
        source: std::io::Error,
    },
}

impl std::fmt::Display for RuntimeDirError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotAbsolute(p) => write!(
                f,
                "the temp directory {:?} is not an absolute path; set TMPDIR (or XDG_RUNTIME_DIR) to an absolute path you own",
                p
            ),
            Self::Refused { dir, why } => write!(
                f,
                "refusing the runtime directory {}: {why}; remove it or point XDG_RUNTIME_DIR / TMPDIR at a private (mode 0700) directory you own",
                dir.display()
            ),
            Self::Create { dir, source } => {
                write!(f, "cannot create the runtime directory {}: {source}", dir.display())
            }
        }
    }
}

impl std::error::Error for RuntimeDirError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Create { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// Pure core of [`per_user_runtime_dir`]: every input is a parameter, so tests never touch the
/// process environment (concurrent `set_var` is UB and this suite is not serialized).
#[cfg(unix)]
fn runtime_dir_from(
    xdg: Option<std::ffi::OsString>,
    temp: &Path,
    name_uid: crate::util::private_dir::Uid,
    verify_uid: crate::util::private_dir::Uid,
) -> Result<PathBuf, RuntimeDirError> {
    use crate::util::private_dir::{create_private_dir, verify_private_dir};
    if let Some(dir) = xdg {
        let p = PathBuf::from(dir);
        // The XDG basedir spec: a relative value must be treated as unset.
        if p.is_absolute() && p.exists() {
            return match verify_private_dir(&p, verify_uid) {
                Ok(()) => Ok(p),
                Err(why) => Err(RuntimeDirError::Refused { dir: p, why }),
            };
        }
    }
    if !temp.is_absolute() {
        return Err(RuntimeDirError::NotAbsolute(temp.to_path_buf()));
    }
    let dir = temp.join(format!("codescout-{name_uid}"));
    // Not recursive, and an existing entry is not an error here: whether it is OURS is
    // `verify_private_dir`'s to judge, on every call, because on a shared `/tmp` another user
    // may have created the name first.
    if let Err(source) = create_private_dir(&dir) {
        return Err(RuntimeDirError::Create { dir, source });
    }
    match verify_private_dir(&dir, verify_uid) {
        Ok(()) => Ok(dir),
        Err(why) => Err(RuntimeDirError::Refused { dir, why }),
    }
}

/// Socket a peer-serve process for `workspace_root` listens on.
pub fn peer_socket_path_for_workspace(workspace_root: &Path) -> Result<PathBuf, RuntimeDirError> {
    Ok(per_user_runtime_dir()?.join(format!(
        "codescout-peer-{}.sock",
        workspace_hash(workspace_root)
    )))
}

/// Lock file guarding a single peer-serve instance per workspace.
pub fn peer_lock_path_for_workspace(workspace_root: &Path) -> Result<PathBuf, RuntimeDirError> {
    Ok(per_user_runtime_dir()?.join(format!(
        "codescout-peer-{}.lock",
        workspace_hash(workspace_root)
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    // "shares_dir" here means the PEER socket sits directly in
    // `per_user_runtime_dir()` — it is not a claim that peer and mux are
    // co-located. Since 2026-07-28 they are not: `lsp::mux::mux_dir()` redirects
    // to a per-process scratch subdirectory under `cfg(test)` so unit tests stop
    // leaking mux lock files into the shared runtime dir
    // (docs/issues/archive/2026-07-28-index-lock-tests-pollute-runtime-dir.md). The
    // assertions below only ever concerned the peer path, so they still hold.
    //
    // Name kept despite the imprecision: it is cited as a copy-pasteable
    // `cargo test` invocation in
    // docs/superpowers/plans/2026-06-01-peer-delegation-phase1.md:105.
    #[test]
    fn peer_socket_differs_from_mux_and_shares_dir() {
        let root = Path::new("/home/u/projB");
        let peer = peer_socket_path_for_workspace(root).unwrap();
        let name = peer.file_name().unwrap().to_str().unwrap();
        assert!(name.starts_with("codescout-"), "got {name}");
        assert!(name.contains("-peer-"), "expected -peer- infix, got {name}");
        assert!(name.contains(&workspace_hash(root)), "must embed the hash");
        assert!(
            !name.contains("-mux-"),
            "must not collide with the mux name"
        );
        assert_eq!(peer.parent().unwrap(), per_user_runtime_dir().unwrap());
    }
}

/// The runtime directory holds the peer, mux and index-lock sockets, so it must be a directory
/// only this user can write to. Every case here passes the inputs as parameters
/// ([`runtime_dir_from`]), never through the process environment.
/// (`docs/issues/archive/2026-10-17-per-user-runtime-dir-trusts-a-predictable-name-in-a-shared-temp-directory-without-owner-mode-or-symlink-checks.md`)
#[cfg(all(test, unix))]
mod runtime_dir_trust {
    use super::*;
    use crate::util::private_dir::{effective_uid, DirRefusal};
    use std::os::unix::fs::{symlink, PermissionsExt};

    fn me() -> u32 {
        effective_uid()
    }

    fn chmod(p: &Path, mode: u32) {
        std::fs::set_permissions(p, std::fs::Permissions::from_mode(mode)).unwrap();
    }

    fn mode_of(p: &Path) -> u32 {
        std::fs::symlink_metadata(p).unwrap().permissions().mode() & 0o777
    }

    fn named(base: &Path) -> PathBuf {
        base.join(format!("codescout-{}", me()))
    }

    #[test]
    fn a_missing_directory_is_created_private_and_reused() {
        let tmp = tempfile::tempdir().unwrap();
        let got = runtime_dir_from(None, tmp.path(), me(), me()).unwrap();
        assert_eq!(got, named(tmp.path()));
        assert_eq!(mode_of(&got), 0o700);
        assert_eq!(runtime_dir_from(None, tmp.path(), me(), me()).unwrap(), got);
    }

    #[test]
    fn a_symlink_at_the_name_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let elsewhere = tmp.path().join("elsewhere");
        std::fs::create_dir(&elsewhere).unwrap();
        chmod(&elsewhere, 0o700);
        symlink(&elsewhere, named(tmp.path())).unwrap();
        let err = runtime_dir_from(None, tmp.path(), me(), me()).unwrap_err();
        assert!(
            matches!(
                err,
                RuntimeDirError::Refused {
                    why: DirRefusal::NotARealDirectory,
                    ..
                }
            ),
            "a link to a perfectly good directory must be refused, got: {err}"
        );
    }

    #[test]
    fn a_group_or_world_writable_directory_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = named(tmp.path());
        std::fs::create_dir(&dir).unwrap();
        chmod(&dir, 0o777);
        let err = runtime_dir_from(None, tmp.path(), me(), me()).unwrap_err();
        assert!(
            matches!(
                err,
                RuntimeDirError::Refused {
                    why: DirRefusal::GroupOrOtherWritable { mode: 0o777 },
                    ..
                }
            ),
            "a 0777 directory must be refused, got: {err}"
        );
    }

    #[test]
    fn a_directory_owned_by_another_uid_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let err = runtime_dir_from(None, tmp.path(), me(), me().wrapping_add(1)).unwrap_err();
        assert!(
            matches!(
                err,
                RuntimeDirError::Refused {
                    why: DirRefusal::ForeignOwner { .. },
                    ..
                }
            ),
            "a directory not owned by the verifying uid must be refused, got: {err}"
        );
    }

    #[test]
    fn an_empty_or_relative_temp_dir_is_refused_and_creates_nothing() {
        for bad in ["", "relative/tmp"] {
            let temp = Path::new(bad);
            let err = runtime_dir_from(None, temp, me(), me()).unwrap_err();
            assert!(
                matches!(err, RuntimeDirError::NotAbsolute(_)),
                "{bad:?} must be refused as not absolute, got: {err}"
            );
            assert!(
                !named(temp).exists(),
                "{bad:?}: a refused temp dir must not leave a directory under the working directory"
            );
        }
    }

    #[test]
    fn a_world_writable_xdg_directory_is_refused() {
        let xdg = tempfile::tempdir().unwrap();
        chmod(xdg.path(), 0o777);
        let err = runtime_dir_from(
            Some(xdg.path().as_os_str().to_owned()),
            Path::new("/nonexistent-temp"),
            me(),
            me(),
        )
        .unwrap_err();
        assert!(
            matches!(
                err,
                RuntimeDirError::Refused {
                    why: DirRefusal::GroupOrOtherWritable { .. },
                    ..
                }
            ),
            "XDG_RUNTIME_DIR must meet the same bar as the temp directory, got: {err}"
        );
    }

    #[test]
    fn a_private_xdg_directory_is_used() {
        let xdg = tempfile::tempdir().unwrap();
        chmod(xdg.path(), 0o700);
        let got = runtime_dir_from(
            Some(xdg.path().as_os_str().to_owned()),
            Path::new("/nonexistent-temp"),
            me(),
            me(),
        )
        .unwrap();
        assert_eq!(got, xdg.path());
    }

    #[test]
    fn the_refusal_names_the_path_and_the_way_out() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = named(tmp.path());
        std::fs::create_dir(&dir).unwrap();
        chmod(&dir, 0o777);
        let text = runtime_dir_from(None, tmp.path(), me(), me())
            .unwrap_err()
            .to_string();
        assert!(text.contains(&dir.display().to_string()), "{text}");
        assert!(text.contains("XDG_RUNTIME_DIR"), "{text}");
    }
}
