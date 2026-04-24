pub mod process;
pub mod protocol;

#[cfg(test)]
mod coherence_rust;
#[cfg(test)]
pub(crate) mod test_support;

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

pub fn workspace_hash(workspace_root: &Path) -> String {
    let mut hasher = DefaultHasher::new();
    workspace_root.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

pub fn socket_path_for_workspace(language: &str, workspace_root: &Path) -> PathBuf {
    per_user_mux_dir().join(format!(
        "codescout-{}-mux-{}.sock",
        language,
        workspace_hash(workspace_root)
    ))
}

pub fn lock_path_for_workspace(language: &str, workspace_root: &Path) -> PathBuf {
    per_user_mux_dir().join(format!(
        "codescout-{}-mux-{}.lock",
        language,
        workspace_hash(workspace_root)
    ))
}

/// Return a directory for mux socket/lock files that is private to the
/// current user.
///
/// Unix: prefers `$XDG_RUNTIME_DIR` (typically `/run/user/$UID`, already mode
/// `0700`). Falls back to `$TMPDIR/codescout-$UID` created with mode `0700`
/// when XDG_RUNTIME_DIR is unset (e.g. headless servers, macOS).
///
/// Windows: uses the process's temp dir. Windows temp is already per-user
/// under `%LOCALAPPDATA%\Temp` and subject to per-user ACLs, so no extra
/// hardening is applied at this layer.
fn per_user_mux_dir() -> PathBuf {
    #[cfg(unix)]
    {
        if let Some(dir) = std::env::var_os("XDG_RUNTIME_DIR") {
            let p = PathBuf::from(dir);
            if p.exists() {
                return p;
            }
        }
        // Fallback: create a 0700 subdir under temp, keyed by UID.
        use std::os::unix::fs::DirBuilderExt;
        // SAFETY: getuid is always safe; returns the real UID.
        let uid = unsafe { libc::getuid() };
        let dir = std::env::temp_dir().join(format!("codescout-{uid}"));
        let _ = std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&dir);
        dir
    }
    #[cfg(not(unix))]
    {
        std::env::temp_dir()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn socket_path_deterministic_for_same_workspace() {
        let p1 = socket_path_for_workspace("kotlin", Path::new("/home/user/project"));
        let p2 = socket_path_for_workspace("kotlin", Path::new("/home/user/project"));
        assert_eq!(p1, p2);

        let p3 = socket_path_for_workspace("kotlin", Path::new("/home/user/other"));
        assert_ne!(p1, p3);
    }

    #[test]
    fn different_languages_get_different_paths() {
        let p1 = socket_path_for_workspace("kotlin", Path::new("/project"));
        let p2 = socket_path_for_workspace("java", Path::new("/project"));
        assert_ne!(p1, p2);
    }
}
