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
    std::env::temp_dir().join(format!(
        "codescout-{}-mux-{}.sock",
        language,
        workspace_hash(workspace_root)
    ))
}

pub fn lock_path_for_workspace(language: &str, workspace_root: &Path) -> PathBuf {
    std::env::temp_dir().join(format!(
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
