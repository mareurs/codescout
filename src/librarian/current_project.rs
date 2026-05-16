//! Resolve the caller's current project from the MCP server's working
//! directory and the configured workspace roots / umbrellas.
//!
//! "Current project" = the deepest directory that:
//!   1. Is an ancestor of (or equal to) `cwd`,
//!   2. Lies under one of the workspace `roots`,
//!   3. Looks like an independent project (`.git` directory present), OR
//!      the root path itself when no `.git` ancestor is found.
//!
//! The result drives default scoping for listing tools so that, by default,
//! they only return artifacts belonging to the project the agent is working
//! in — not every doc across every repo on disk.

use std::path::{Path, PathBuf};

use crate::librarian::workspace::WorkspaceConfig;

#[derive(Debug, Clone)]
pub struct CurrentProject {
    /// Absolute path of the active project (canonicalized).
    pub abs_path: PathBuf,
    /// Nearest enclosing `.git/` ancestor; falls back to abs_path.
    pub git_root: PathBuf,
    /// Umbrella name if this project is a descendant of any umbrella member.
    pub umbrella: Option<String>,
}

pub fn resolve(active_path: &Path, ws: &WorkspaceConfig) -> Option<CurrentProject> {
    let abs_path = std::fs::canonicalize(active_path).ok()?;
    let git_root = lookup_git_root(&abs_path).unwrap_or_else(|| abs_path.clone());
    let umbrella = lookup_umbrella(&abs_path, ws);
    Some(CurrentProject {
        abs_path,
        git_root,
        umbrella,
    })
}

pub fn lookup_git_root(start: &Path) -> Option<PathBuf> {
    let mut cur = start;
    loop {
        if cur.join(".git").exists() {
            return Some(cur.to_path_buf());
        }
        cur = cur.parent()?;
    }
}

pub fn lookup_umbrella(abs_path: &Path, ws: &WorkspaceConfig) -> Option<String> {
    ws.umbrellas.iter().find_map(|u| {
        u.members
            .iter()
            .any(|m| abs_path.starts_with(m))
            .then(|| u.name.clone())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::workspace::Umbrella;
    use tempfile::TempDir;

    #[test]
    fn resolve_from_active_path_returns_self() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().to_path_buf();
        let ws = WorkspaceConfig::default();
        let cp = resolve(&p, &ws).unwrap();
        assert_eq!(cp.abs_path, std::fs::canonicalize(&p).unwrap());
    }

    #[test]
    fn resolve_finds_git_root_when_nested() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join(".git")).unwrap();
        let nested = tmp.path().join("a/b/c");
        std::fs::create_dir_all(&nested).unwrap();
        let cp = resolve(&nested, &WorkspaceConfig::default()).unwrap();
        assert_eq!(cp.git_root, std::fs::canonicalize(tmp.path()).unwrap());
    }

    #[test]
    fn resolve_falls_back_to_abs_path_when_no_git() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().to_path_buf();
        let cp = resolve(&p, &WorkspaceConfig::default()).unwrap();
        assert_eq!(cp.git_root, cp.abs_path);
    }

    #[test]
    fn resolve_returns_none_for_non_existent_path() {
        let p = std::path::Path::new("/nonexistent/zzz/qqq");
        assert!(resolve(p, &WorkspaceConfig::default()).is_none());
    }

    #[test]
    fn umbrella_lookup_includes_descendants() {
        let tmp = TempDir::new().unwrap();
        let umb_root = tmp.path().to_path_buf();
        let nested = umb_root.join("sub");
        std::fs::create_dir_all(&nested).unwrap();
        let ws = WorkspaceConfig {
            roots: vec![],
            ignore: vec![],
            rules: vec![],
            umbrellas: vec![Umbrella {
                name: "team".into(),
                members: vec![std::fs::canonicalize(&umb_root).unwrap()],
            }],
        };
        let cp = resolve(&nested, &ws).unwrap();
        assert_eq!(cp.umbrella, Some("team".to_string()));
    }
}
