#[cfg(unix)]
pub mod process;
pub mod protocol;

#[cfg(test)]
mod coherence_rust;
#[cfg(test)]
pub(crate) mod test_support;

use std::path::{Path, PathBuf};

use crate::socket_discovery::per_user_runtime_dir as per_user_mux_dir;
pub use crate::socket_discovery::workspace_hash;

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
