//! Fixtures the derived index's tests share: a commit built through `git2` alone.
//!
//! Every repo is made with an explicit signature and a fixed time, never from the global git
//! config and never from where the temp directory lies, so a commit id depends on content only.

/// The mode of a regular file tree entry.
pub(crate) const FILE: u32 = 0o100644;
/// The mode of a symlink tree entry.
pub(crate) const LINK: u32 = 0o120000;

/// A fixed author time, so a commit id depends on the content alone.
pub(crate) fn fixed_sig() -> git2::Signature<'static> {
    git2::Signature::new(
        "Test",
        "test@example.invalid",
        &git2::Time::new(1_700_000_000, 0),
    )
    .unwrap()
}

/// A repo in `dir` with one commit holding `entries` (`path`, `mode`, `bytes`). Built through
/// the index and an explicit signature: no working tree, no global git config.
pub(crate) fn repo_with(dir: &std::path::Path, entries: &[(&str, u32, &[u8])]) -> git2::Repository {
    let repo = git2::Repository::init(dir).unwrap();
    // The repository's own index, used only to assemble a tree: it is never written to disk.
    let mut index = repo.index().unwrap();
    for &(path, mode, bytes) in entries {
        let entry = git2::IndexEntry {
            ctime: git2::IndexTime::new(0, 0),
            mtime: git2::IndexTime::new(0, 0),
            dev: 0,
            ino: 0,
            mode,
            uid: 0,
            gid: 0,
            file_size: bytes.len() as u32,
            id: git2::Oid::zero(),
            flags: 0,
            flags_extended: 0,
            path: path.as_bytes().to_vec(),
        };
        index.add_frombuffer(&entry, bytes).unwrap();
    }
    let tree_id = index.write_tree().unwrap();
    {
        let tree = repo.find_tree(tree_id).unwrap();
        let sig = fixed_sig();
        repo.commit(Some("HEAD"), &sig, &sig, "fixture", &tree, &[])
            .unwrap();
    }
    repo
}

/// A repo whose root tree is built by hand, so an entry name can be raw bytes.
pub(crate) fn repo_with_raw_tree(
    dir: &std::path::Path,
    build: impl FnOnce(&git2::Repository, &mut git2::TreeBuilder),
) -> git2::Repository {
    let repo = git2::Repository::init(dir).unwrap();
    let tree_id = {
        let mut root = repo.treebuilder(None).unwrap();
        build(&repo, &mut root);
        root.write().unwrap()
    };
    {
        let tree = repo.find_tree(tree_id).unwrap();
        let sig = fixed_sig();
        repo.commit(Some("HEAD"), &sig, &sig, "fixture", &tree, &[])
            .unwrap();
    }
    repo
}
