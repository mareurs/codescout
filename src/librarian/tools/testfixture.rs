//! Git fixtures shared by the librarian tool tests: a real repository with one commit, and one
//! with a registered linked worktree, both made by git itself rather than faked.

use std::path::PathBuf;
use tempfile::TempDir;

/// A real git repo with one real commit, so resolution is genuine rather than mocked.
pub(crate) fn git_fixture_with_commit() -> (TempDir, PathBuf, String) {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("repo");
    std::fs::create_dir_all(&root).unwrap();
    let repo = git2::Repository::init(&root).unwrap();
    let mut config = repo.config().unwrap();
    config.set_str("user.name", "Test").unwrap();
    config.set_str("user.email", "test@test.com").unwrap();
    std::fs::write(root.join("a.txt"), "hello").unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(std::path::Path::new("a.txt")).unwrap();
    index.write().unwrap();
    let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
    let sig = repo.signature().unwrap();
    let sha = repo
        .commit(Some("HEAD"), &sig, &sig, "init", &tree, &[])
        .unwrap()
        .to_string();
    (tmp, root, sha)
}

/// A real repo (one commit, so a worktree can be added) whose `.worktrees/` holds one
/// REGISTERED linked worktree, `registered`, made by git itself rather than faked, so the
/// registration a reader sees is the one `git worktree add` writes. Returns the temp dir, the
/// main checkout and the worktree.
pub(crate) fn repo_with_worktree() -> (TempDir, PathBuf, PathBuf) {
    let (tmp, root, _sha) = git_fixture_with_commit();
    let wt_parent = root.join(".worktrees");
    std::fs::create_dir_all(&wt_parent).unwrap();
    let worktree = wt_parent.join("registered");
    let repo = git2::Repository::open(&root).unwrap();
    repo.worktree("registered", &worktree, None).unwrap();
    (tmp, root, worktree)
}

/// [`repo_with_worktree`] without the worktree's path.
pub(crate) fn repo_with_registered_worktree() -> (TempDir, PathBuf) {
    let (tmp, root, _worktree) = repo_with_worktree();
    (tmp, root)
}

#[test]
fn repo_with_worktree_gives_a_worktree_git_has_registered() {
    let (_tmp, main, worktree) = repo_with_worktree();
    let repo = git2::Repository::open(&main).unwrap();
    let names = repo.worktrees().unwrap();
    assert_eq!(
        names.iter().flatten().collect::<Vec<_>>(),
        vec!["registered"]
    );
    assert_eq!(
        repo.find_worktree("registered")
            .unwrap()
            .path()
            .canonicalize()
            .unwrap(),
        worktree.canonicalize().unwrap()
    );
    assert!(
        worktree.join("a.txt").is_file(),
        "the worktree is checked out"
    );
}
