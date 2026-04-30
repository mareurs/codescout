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

use crate::workspace::WorkspaceConfig;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CurrentProject {
    /// Workspace root name this project lives under.
    pub root: String,
    /// Relative path from the root to the project directory. Empty string
    /// when the project IS the root itself.
    pub subdir: String,
    /// Absolute filesystem path of the project directory. Used to discover
    /// per-project config files (e.g. `<path>/.codescout/librarian.toml`).
    pub path: PathBuf,
    /// Name of the umbrella that includes this project, if any.
    pub umbrella: Option<String>,
}

impl CurrentProject {
    /// `"root"` when subdir is empty, otherwise `"root/subdir"`.
    pub fn member_key(&self) -> String {
        if self.subdir.is_empty() {
            self.root.clone()
        } else {
            format!("{}/{}", self.root, self.subdir)
        }
    }
}

pub fn resolve(cwd: &Path, ws: &WorkspaceConfig) -> Option<CurrentProject> {
    let cwd = cwd.canonicalize().ok().unwrap_or_else(|| cwd.to_path_buf());

    let (root, root_path) = ws
        .roots
        .iter()
        .filter_map(|r| {
            let rp = r.path.canonicalize().ok().unwrap_or_else(|| r.path.clone());
            cwd.starts_with(&rp).then_some((r.name.clone(), rp))
        })
        .max_by_key(|(_, p)| p.as_os_str().len())?;

    let project_dir = nearest_git_root(&cwd, &root_path).unwrap_or_else(|| root_path.clone());

    let subdir = project_dir
        .strip_prefix(&root_path)
        .ok()
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default();

    let umbrella = find_umbrella(&root, &subdir, ws);

    Some(CurrentProject {
        root,
        subdir,
        path: project_dir,
        umbrella,
    })
}

fn nearest_git_root(start: &Path, stop_at: &Path) -> Option<PathBuf> {
    let mut cur = start;
    loop {
        if cur.join(".git").exists() {
            return Some(cur.to_path_buf());
        }
        if cur == stop_at {
            return None;
        }
        cur = cur.parent()?;
    }
}

fn find_umbrella(root: &str, subdir: &str, ws: &WorkspaceConfig) -> Option<String> {
    let key = if subdir.is_empty() {
        root.to_string()
    } else {
        format!("{root}/{subdir}")
    };
    ws.umbrellas
        .iter()
        .find(|u| u.members.iter().any(|m| m == &key))
        .map(|u| u.name.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::{Root, Umbrella};
    use tempfile::TempDir;

    fn ws_with(roots: Vec<Root>, umbrellas: Vec<Umbrella>) -> WorkspaceConfig {
        WorkspaceConfig {
            roots,
            ignore: vec![],
            rules: vec![],
            umbrellas,
        }
    }

    #[test]
    fn resolves_to_subdir_with_git_marker() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("monorepo");
        let proj = root.join("svc-a");
        std::fs::create_dir_all(proj.join(".git")).unwrap();

        let ws = ws_with(
            vec![Root {
                name: "mono".into(),
                path: root.clone(),
            }],
            vec![],
        );

        let cp = resolve(&proj, &ws).unwrap();
        assert_eq!(cp.root, "mono");
        assert_eq!(cp.subdir, "svc-a");
        assert!(cp.umbrella.is_none());
    }

    #[test]
    fn resolves_to_root_when_no_git_marker() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("flat");
        let inner = root.join("docs");
        std::fs::create_dir_all(&inner).unwrap();

        let ws = ws_with(
            vec![Root {
                name: "flat".into(),
                path: root.clone(),
            }],
            vec![],
        );

        let cp = resolve(&inner, &ws).unwrap();
        assert_eq!(cp.root, "flat");
        assert_eq!(cp.subdir, "");
    }

    #[test]
    fn returns_none_outside_all_roots() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("inside");
        let outside = tmp.path().join("elsewhere");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&outside).unwrap();

        let ws = ws_with(
            vec![Root {
                name: "inside".into(),
                path: root,
            }],
            vec![],
        );

        assert!(resolve(&outside, &ws).is_none());
    }

    #[test]
    fn picks_longest_matching_root() {
        let tmp = TempDir::new().unwrap();
        let outer = tmp.path().join("outer");
        let inner = outer.join("inner");
        std::fs::create_dir_all(inner.join(".git")).unwrap();

        let ws = ws_with(
            vec![
                Root {
                    name: "outer".into(),
                    path: outer.clone(),
                },
                Root {
                    name: "inner".into(),
                    path: inner.clone(),
                },
            ],
            vec![],
        );

        let cp = resolve(&inner, &ws).unwrap();
        assert_eq!(cp.root, "inner");
        assert_eq!(cp.subdir, "");
    }

    #[test]
    fn attaches_umbrella_when_member_matches() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("infra");
        let proj = root.join("svc-a");
        std::fs::create_dir_all(proj.join(".git")).unwrap();

        let ws = ws_with(
            vec![Root {
                name: "infra".into(),
                path: root,
            }],
            vec![Umbrella {
                name: "platform".into(),
                members: vec!["infra/svc-a".into(), "infra/svc-b".into()],
            }],
        );

        let cp = resolve(&proj, &ws).unwrap();
        assert_eq!(cp.umbrella.as_deref(), Some("platform"));
    }

    #[test]
    fn member_key_handles_empty_subdir() {
        let cp = CurrentProject {
            root: "r".into(),
            subdir: String::new(),
            umbrella: None,
            ..Default::default()
        };
        assert_eq!(cp.member_key(), "r");
        let cp2 = CurrentProject {
            root: "r".into(),
            subdir: "a/b".into(),
            umbrella: None,
            ..Default::default()
        };
        assert_eq!(cp2.member_key(), "r/a/b");
    }
}
