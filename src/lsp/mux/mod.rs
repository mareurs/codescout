pub mod process;
pub mod protocol;
pub mod transport;

#[cfg(test)]
mod coherence_rust;
#[cfg(test)]
pub(crate) mod test_support;

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

/// Compute a stable workspace hash used for naming mux endpoints and lock files.
///
/// On Windows, the path is normalised before hashing so that
/// `C:\foo`, `c:\foo`, and `C:/foo` all collapse to a single mux instance:
///
/// * forward slashes are replaced with backslashes (Win32 accepts both),
/// * the result is ASCII-lowercased (drive-letter case insensitivity).
///
/// Known limitations: verbatim paths (`\\?\C:\foo`), UNC paths
/// (`\\server\share`), and full-Unicode case folding are NOT collapsed.
/// The first two require deeper canonicalisation (tracker W17/W20);
/// non-ASCII case folding would need ICU and is out of scope for the
/// 16-hex-char mux endpoint name.
pub fn workspace_hash(workspace_root: &Path) -> String {
    let mut hasher = DefaultHasher::new();
    #[cfg(windows)]
    {
        let normalized = workspace_root
            .to_string_lossy()
            .replace('/', "\\")
            .to_ascii_lowercase();
        normalized.hash(&mut hasher);
    }
    #[cfg(not(windows))]
    {
        workspace_root.hash(&mut hasher);
    }
    format!("{:016x}", hasher.finish())
}

pub fn socket_path_for_workspace(language: &str, workspace_root: &Path) -> PathBuf {
    transport::endpoint_path(
        &per_user_mux_dir(),
        language,
        &workspace_hash(workspace_root),
    )
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

    #[cfg(windows)]
    #[test]
    fn mixed_case_drive_letter_collapses_on_windows() {
        // C:\foo and c:\foo are the same workspace on Windows; the hash
        // (and therefore the mux endpoint) must collapse to one.
        let upper = workspace_hash(Path::new(r"C:\Users\me\project"));
        let lower = workspace_hash(Path::new(r"c:\users\me\project"));
        assert_eq!(upper, lower);
    }

    #[cfg(windows)]
    #[test]
    fn forward_slash_path_collapses_with_backslash_on_windows() {
        // Cargo, many tools, and humans frequently emit forward slashes on
        // Windows. The mux must treat them as the same workspace.
        let backslash = workspace_hash(Path::new(r"C:\Users\me\project"));
        let slash = workspace_hash(Path::new("C:/Users/me/project"));
        assert_eq!(backslash, slash);
    }
}
