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

    // "shares_dir" here means the PEER socket sits directly in
    // `per_user_runtime_dir()` — it is not a claim that peer and mux are
    // co-located. Since 2026-07-28 they are not: `lsp::mux::mux_dir()` redirects
    // to a per-process scratch subdirectory under `cfg(test)` so unit tests stop
    // leaking mux lock files into the shared runtime dir
    // (docs/issues/2026-07-28-index-lock-tests-pollute-runtime-dir.md). The
    // assertions below only ever concerned the peer path, so they still hold.
    //
    // Name kept despite the imprecision: it is cited as a copy-pasteable
    // `cargo test` invocation in
    // docs/superpowers/plans/2026-06-01-peer-delegation-phase1.md:105.
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
