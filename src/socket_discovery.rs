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
pub fn per_user_runtime_dir() -> PathBuf {
    #[cfg(unix)]
    {
        if let Some(dir) = std::env::var_os("XDG_RUNTIME_DIR") {
            let p = PathBuf::from(dir);
            if p.exists() {
                return p;
            }
        }
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

/// Socket a peer-serve process for `workspace_root` listens on.
pub fn peer_socket_path_for_workspace(workspace_root: &Path) -> PathBuf {
    per_user_runtime_dir().join(format!(
        "codescout-peer-{}.sock",
        workspace_hash(workspace_root)
    ))
}

/// Lock file guarding a single peer-serve instance per workspace.
pub fn peer_lock_path_for_workspace(workspace_root: &Path) -> PathBuf {
    per_user_runtime_dir().join(format!(
        "codescout-peer-{}.lock",
        workspace_hash(workspace_root)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn peer_socket_differs_from_mux_and_shares_dir() {
        let root = Path::new("/home/u/projB");
        let peer = peer_socket_path_for_workspace(root);
        let name = peer.file_name().unwrap().to_str().unwrap();
        assert!(name.starts_with("codescout-"), "got {name}");
        assert!(name.contains("-peer-"), "expected -peer- infix, got {name}");
        assert!(name.contains(&workspace_hash(root)), "must embed the hash");
        assert!(
            !name.contains("-mux-"),
            "must not collide with the mux name"
        );
        assert_eq!(peer.parent().unwrap(), per_user_runtime_dir());
    }
}
