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

use crate::librarian::workspace::{Umbrella, WorkspaceConfig};

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

/// True iff `root` is a *linked* git worktree (created by `git worktree add`),
/// as opposed to a main checkout, a submodule, or a non-git directory.
///
/// Filesystem-only (no `git` subprocess): a linked worktree's `.git` is a
/// *file* containing `gitdir: <main>/.git/worktrees/<name>`. A submodule's
/// `.git` file points into `.git/modules/<name>` instead, so we require a
/// `worktrees` path component — skipping a submodule root would be wrong.
pub(crate) fn is_linked_worktree(root: &Path) -> bool {
    let dot_git = root.join(".git");
    let Ok(meta) = std::fs::symlink_metadata(&dot_git) else {
        return false;
    };
    if !meta.file_type().is_file() {
        return false;
    }
    let Ok(pointer) = std::fs::read_to_string(&dot_git) else {
        return false;
    };
    pointer
        .lines()
        .find_map(|l| l.strip_prefix("gitdir:").map(str::trim))
        .map(|gitdir| {
            Path::new(gitdir)
                .components()
                .any(|c| c.as_os_str() == "worktrees")
        })
        .unwrap_or(false)
}

/// Given a linked-worktree root, derives its MAIN repo root from the
/// `.git`-file `gitdir: <main>/.git/worktrees/<name>` pointer — the same file
/// [`is_linked_worktree`] reads. Filesystem-only (no `git` subprocess).
///
/// Returns `None` if `root` has no readable `.git` file, or if the pointer's
/// `gitdir:` path has no `.git` path component to split the main root on
/// (i.e. `root` is not actually a linked worktree).
pub(crate) fn worktree_main_root(root: &Path) -> Option<PathBuf> {
    let pointer = std::fs::read_to_string(root.join(".git")).ok()?;
    let gitdir = pointer
        .lines()
        .find_map(|l| l.strip_prefix("gitdir:").map(str::trim))?;
    let mut main = PathBuf::new();
    for component in Path::new(gitdir).components() {
        if component.as_os_str() == ".git" {
            return Some(main);
        }
        main.push(component);
    }
    None
}

pub fn lookup_umbrella(abs_path: &Path, ws: &WorkspaceConfig) -> Option<String> {
    lookup_umbrella_in(abs_path, &ws.umbrellas)
}

/// Return the name of the first umbrella in `umbrellas` whose member set
/// contains `abs_path` (by path-prefix). The slice-based twin of
/// [`lookup_umbrella`], so callers can resolve against a merged list.
pub fn lookup_umbrella_in(abs_path: &Path, umbrellas: &[Umbrella]) -> Option<String> {
    umbrellas.iter().find_map(|u| {
        u.members
            .iter()
            .any(|m| abs_path.starts_with(m))
            .then(|| u.name.clone())
    })
}

/// Resolve the umbrella for `abs_path`, preferring project-local declarations
/// over the machine-global registry. A project's own `.codescout/workspace.toml`
/// expresses the owning project's explicit, colocated intent, so it wins; the
/// global registry (`~/.config/librarian/workspace.toml`) is the fallback for
/// peer/cross-linked projects that have no single owner.
pub fn resolve_umbrella(
    abs_path: &Path,
    project_local: &[Umbrella],
    global: &[Umbrella],
) -> Option<String> {
    lookup_umbrella_in(abs_path, project_local).or_else(|| lookup_umbrella_in(abs_path, global))
}

/// Load project-local umbrellas from a project's own `.codescout/workspace.toml`.
/// Only the `[[umbrella]]` array is read; codescout's own sections (`[workspace]`,
/// `[[project]]`, ...) are ignored. A missing or unparseable file yields an empty
/// list, so a project without local umbrellas simply falls through to the global
/// registry.
pub fn load_project_umbrellas(project_root: &Path) -> Vec<Umbrella> {
    // A minimal view over the project's .codescout/workspace.toml: no
    // deny_unknown_fields, so codescout's own [workspace]/[[project]]/... are
    // ignored and only [[umbrella]] is extracted.
    #[derive(serde::Deserialize, Default)]
    struct ProjectUmbrellas {
        #[serde(default, rename = "umbrella")]
        umbrellas: Vec<Umbrella>,
    }
    let path = project_root.join(".codescout").join("workspace.toml");
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| toml::from_str::<ProjectUmbrellas>(&s).ok())
        .map(|p| p.umbrellas)
        .unwrap_or_default()
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
    fn is_linked_worktree_detects_worktree_not_submodule_or_main() {
        let tmp = TempDir::new().unwrap();
        // Linked worktree: .git is a FILE → gitdir: .../worktrees/<name>
        let wt = tmp.path().join("wt");
        std::fs::create_dir_all(&wt).unwrap();
        std::fs::write(
            wt.join(".git"),
            format!(
                "gitdir: {}/main/.git/worktrees/feat\n",
                tmp.path().display()
            ),
        )
        .unwrap();
        assert!(is_linked_worktree(&wt), "linked worktree detected");

        // Submodule: .git file → gitdir: .../modules/<name> (NOT a worktree)
        let sub = tmp.path().join("sub");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(
            sub.join(".git"),
            format!(
                "gitdir: {}/super/.git/modules/libfoo\n",
                tmp.path().display()
            ),
        )
        .unwrap();
        assert!(
            !is_linked_worktree(&sub),
            "submodule root is not a worktree"
        );

        // Main checkout: .git is a DIRECTORY
        let main = tmp.path().join("main");
        std::fs::create_dir_all(main.join(".git")).unwrap();
        assert!(
            !is_linked_worktree(&main),
            "main checkout is not a worktree"
        );

        // No .git at all
        let plain = tmp.path().join("plain");
        std::fs::create_dir_all(&plain).unwrap();
        assert!(!is_linked_worktree(&plain), "non-git dir is not a worktree");
    }

    #[test]
    fn worktree_main_root_from_gitdir_pointer() {
        let tmp = TempDir::new().unwrap();
        let wt = tmp.path().join("main/.worktrees/feat");
        std::fs::create_dir_all(&wt).unwrap();
        std::fs::write(
            wt.join(".git"),
            format!(
                "gitdir: {}/main/.git/worktrees/feat\n",
                tmp.path().display()
            ),
        )
        .unwrap();
        let main = worktree_main_root(&wt).unwrap();
        assert_eq!(main, tmp.path().join("main"));
    }

    #[test]
    fn worktree_main_root_returns_none_for_non_worktree() {
        let tmp = TempDir::new().unwrap();
        // Main checkout: no .git file (it's a directory) -> read_to_string fails.
        let main = tmp.path().join("main");
        std::fs::create_dir_all(main.join(".git")).unwrap();
        assert!(worktree_main_root(&main).is_none());

        // No .git at all.
        let plain = tmp.path().join("plain");
        std::fs::create_dir_all(&plain).unwrap();
        assert!(worktree_main_root(&plain).is_none());
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

    #[test]
    fn resolve_umbrella_prefers_project_local_over_global() {
        let p = Path::new("/home/x/proj");
        let local = vec![Umbrella {
            name: "local".into(),
            members: vec!["/home/x/proj".into()],
        }];
        let global = vec![Umbrella {
            name: "global".into(),
            members: vec!["/home/x".into()],
        }];
        assert_eq!(
            resolve_umbrella(p, &local, &global).as_deref(),
            Some("local"),
            "project-local umbrella must win when both match"
        );
    }

    #[test]
    fn resolve_umbrella_falls_back_to_global() {
        let p = Path::new("/home/x/proj");
        let local = vec![Umbrella {
            name: "unrelated".into(),
            members: vec!["/elsewhere".into()],
        }];
        let global = vec![Umbrella {
            name: "global".into(),
            members: vec!["/home/x".into()],
        }];
        assert_eq!(
            resolve_umbrella(p, &local, &global).as_deref(),
            Some("global"),
            "global registry is the fallback when no project-local umbrella matches"
        );
    }

    #[test]
    fn resolve_umbrella_none_when_neither_matches() {
        let p = Path::new("/home/x/proj");
        let local = vec![Umbrella {
            name: "a".into(),
            members: vec!["/no/a".into()],
        }];
        let global = vec![Umbrella {
            name: "b".into(),
            members: vec!["/no/b".into()],
        }];
        assert_eq!(resolve_umbrella(p, &local, &global), None);
    }

    #[test]
    fn load_project_umbrellas_reads_umbrella_ignoring_other_sections() {
        let tmp = TempDir::new().unwrap();
        let cs = tmp.path().join(".codescout");
        std::fs::create_dir_all(&cs).unwrap();
        // A real project workspace.toml also carries codescout's own sections;
        // the loader must read [[umbrella]] and ignore the rest (no deny_unknown).
        std::fs::write(
            cs.join("workspace.toml"),
            r#"
    [workspace]
    name = "hub"

    [[project]]
    id = "hub"
    root = "."

    [[umbrella]]
    name = "hub-and-spokes"
    members = ["/a", "/b"]
    "#,
        )
        .unwrap();
        let u = load_project_umbrellas(tmp.path());
        assert_eq!(u.len(), 1);
        assert_eq!(u[0].name, "hub-and-spokes");
        assert_eq!(u[0].members.len(), 2);
    }

    #[test]
    fn load_project_umbrellas_empty_when_file_absent() {
        let tmp = TempDir::new().unwrap();
        assert!(load_project_umbrellas(tmp.path()).is_empty());
    }
}
