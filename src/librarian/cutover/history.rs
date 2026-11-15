//! Recover the path an old artifact id hashes, from git history.
//!
//! An old artifact id is `sha256(<root>/<path>)[..16]` (`ids::artifact_id_from_abs`). A citation
//! by id of a file that the old catalog no longer holds can still be recovered when the path it
//! hashes appears in history. History binds an id to exactly the path it hashes and nothing
//! else (D-R28): a deleted file is never matched to another file with the same name or key.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::librarian::ids::artifact_id_from_abs;

/// Every markdown path git history of a commit has seen, hashed under each root spelling, and
/// the rename edges between those paths.
pub struct HistoryIds {
    /// The id to the historical path it hashes.
    ids: HashMap<String, String>,
    /// Old path to `(position in the walk, new path)`: a smaller position is a newer commit.
    renames: HashMap<String, Vec<(usize, String)>>,
    /// The markdown paths the commit itself holds.
    head: HashSet<String>,
}

impl HistoryIds {
    /// Walk every commit reachable from `head`: each diff to its first parent (none for a root
    /// commit) runs rename detection, and every markdown path it names is hashed under each of
    /// `spellings`.
    ///
    /// The cost is bounded by what changed: a tree diff skips every subtree whose object is
    /// unchanged, the diff is limited to markdown paths, no blob of a file that was only added
    /// or only deleted is read, and libgit2's own rename limit caps the pairs it compares per
    /// commit. A rename it cannot see leaves the id with its historical path, which is reported.
    pub fn load(
        repo: &git2::Repository,
        head: &git2::Commit,
        spellings: &[&Path],
    ) -> anyhow::Result<HistoryIds> {
        let mut held = HashSet::new();
        head.tree()?
            .walk(git2::TreeWalkMode::PreOrder, |dir, entry| {
                if let Some(name) = entry.name().filter(|n| is_markdown(n)) {
                    held.insert(format!("{dir}{name}"));
                }
                git2::TreeWalkResult::Ok
            })?;

        let mut seen: HashSet<String> = HashSet::new();
        let mut renames: HashMap<String, Vec<(usize, String)>> = HashMap::new();
        let mut walk = repo.revwalk()?;
        walk.push(head.id())?;
        walk.set_sorting(git2::Sort::TOPOLOGICAL | git2::Sort::TIME)?;
        for (position, oid) in walk.enumerate() {
            let commit = repo.find_commit(oid?)?;
            let tree = commit.tree()?;
            let parent_tree = match commit.parent(0) {
                Ok(parent) => Some(parent.tree()?),
                Err(_) => None,
            };
            let mut options = git2::DiffOptions::new();
            options.pathspec("*.md");
            let mut diff =
                repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), Some(&mut options))?;
            let mut similar = git2::DiffFindOptions::new();
            similar.renames(true);
            similar.rename_threshold(RENAME_SIMILARITY);
            diff.find_similar(Some(&mut similar))?;
            for delta in diff.deltas() {
                let old = delta.old_file().path().and_then(Path::to_str);
                let new = delta.new_file().path().and_then(Path::to_str);
                match (delta.status(), old, new) {
                    (git2::Delta::Renamed, Some(old), Some(new)) => {
                        if is_markdown(old) && is_markdown(new) {
                            seen.insert(old.to_owned());
                            seen.insert(new.to_owned());
                            renames
                                .entry(old.to_owned())
                                .or_default()
                                .push((position, new.to_owned()));
                        }
                    }
                    (git2::Delta::Deleted, Some(old), _) if is_markdown(old) => {
                        seen.insert(old.to_owned());
                    }
                    (_, _, Some(new)) if is_markdown(new) => {
                        seen.insert(new.to_owned());
                    }
                    _ => {}
                }
            }
        }

        let mut ids = HashMap::new();
        let mut paths: Vec<&String> = seen.iter().collect();
        paths.sort();
        for spelling in spellings {
            for path in &paths {
                ids.entry(artifact_id_from_abs(&spelling.join(path.as_str())))
                    .or_insert_with(|| (*path).clone());
            }
        }
        Ok(HistoryIds {
            ids,
            renames,
            head: held,
        })
    }

    /// The path of `head` that the id's own path became: the path itself when `head` holds it,
    /// else the end of the chain of git renames from it. `Err(historical path)` when the id's
    /// path is known and no such chain reaches `head`, or when two renames of the path lead
    /// to different places; `None` when no history path hashes to it.
    pub fn head_path(&self, id: &str) -> Option<Result<String, String>> {
        let origin = self.ids.get(id)?;
        let mut path = origin.clone();
        // Only an edge from a newer commit than the last one taken may follow it.
        let mut older_than = usize::MAX;
        loop {
            if self.head.contains(&path) {
                return Some(Ok(path));
            }
            let next: Vec<&(usize, String)> = self
                .renames
                .get(&path)
                .into_iter()
                .flatten()
                .filter(|(position, _)| *position < older_than)
                .collect();
            let Some((_, to)) = next.first().copied() else {
                return Some(Err(origin.clone()));
            };
            if next.iter().any(|(_, other)| other != to) {
                return Some(Err(origin.clone()));
            }
            older_than = next.iter().map(|(p, _)| *p).max().unwrap_or(older_than);
            path = to.clone();
        }
    }
}

/// How alike two files must be for git to call one the rename of the other, in percent. Higher
/// than git's default of 50: a wrong match would send a citation to another file, a missed one
/// is reported with a suggestion.
const RENAME_SIMILARITY: u16 = 90;

fn is_markdown(path: &str) -> bool {
    path.len() >= 3 && path.as_bytes()[path.len() - 3..].eq_ignore_ascii_case(b".md")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::derived::testkit::{commit_files, repo_with};
    use crate::librarian::ids::artifact_id_from_abs;

    fn id(spelling: &str, path: &str) -> String {
        artifact_id_from_abs(&Path::new(spelling).join(path))
    }

    /// A repo whose history is `steps` in order (the last is `HEAD`), and its history ids under
    /// `spellings`.
    fn history(steps: &[&[(&str, &[u8])]], spellings: &[&str]) -> HistoryIds {
        let dir = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(dir.path()).unwrap();
        let first: Vec<(&str, u32, &[u8])> = steps[0]
            .iter()
            .map(|&(p, b)| (p, crate::librarian::derived::testkit::FILE, b))
            .collect();
        let repo = repo_with(&root, &first);
        let mut tip = repo.head().unwrap().peel_to_commit().unwrap().id();
        for files in &steps[1..] {
            let parent = repo.find_commit(tip).unwrap();
            tip = commit_files(&repo, files, &[&parent], "step");
        }
        let head = repo.find_commit(tip).unwrap();
        let spellings: Vec<&Path> = spellings.iter().map(Path::new).collect();
        HistoryIds::load(&repo, &head, &spellings).unwrap()
    }

    const BODY: &[u8] = b"# Notes\n\nSome text that makes the file big enough to match.\n\nMore.\n";

    #[test]
    fn an_id_maps_through_a_rename_to_its_head_path() {
        let h = history(
            &[&[("old/a.md", BODY)], &[("new/a.md", BODY)]],
            &["/old/root"],
        );
        assert_eq!(
            h.head_path(&id("/old/root", "old/a.md")),
            Some(Ok("new/a.md".to_owned()))
        );
        assert_eq!(
            h.head_path(&id("/old/root", "new/a.md")),
            Some(Ok("new/a.md".to_owned())),
            "a path HEAD holds is itself"
        );
    }

    #[test]
    fn a_rename_chain_is_followed_to_the_end() {
        let h = history(
            &[&[("a.md", BODY)], &[("b.md", BODY)], &[("sub/c.md", BODY)]],
            &["/r"],
        );
        assert_eq!(
            h.head_path(&id("/r", "a.md")),
            Some(Ok("sub/c.md".to_owned()))
        );
        assert_eq!(
            h.head_path(&id("/r", "b.md")),
            Some(Ok("sub/c.md".to_owned()))
        );
    }

    #[test]
    fn a_deleted_files_id_never_maps_to_another_file_with_its_key() {
        let h = history(
            &[
                &[
                    ("docs/a/notes.md", BODY),
                    (
                        "docs/b/notes.md",
                        b"# Unrelated\n\nTotally different words here.\n",
                    ),
                ],
                &[(
                    "docs/b/notes.md",
                    b"# Unrelated\n\nTotally different words here.\n",
                )],
            ],
            &["/r"],
        );
        assert_eq!(
            h.head_path(&id("/r", "docs/a/notes.md")),
            Some(Err("docs/a/notes.md".to_owned())),
            "reported with its own path, bound to nothing"
        );
        assert_eq!(
            h.head_path(&id("/r", "docs/b/notes.md")),
            Some(Ok("docs/b/notes.md".to_owned()))
        );
    }

    #[test]
    fn two_renames_of_one_path_to_different_places_bind_nothing() {
        const OTHER: &[u8] =
            b"# Other\n\nCompletely different words fill this second file instead.\n\nEnd.\n";
        // `a.md` becomes `b.md`; a new `a.md` is later renamed to `c.md`. Both end in HEAD, and
        // nothing says which one the id of `a.md` meant.
        let h = history(
            &[
                &[("a.md", BODY)],
                &[("b.md", BODY)],
                &[("b.md", BODY), ("a.md", OTHER)],
                &[("b.md", BODY), ("c.md", OTHER)],
            ],
            &["/r"],
        );
        assert_eq!(h.head_path(&id("/r", "a.md")), Some(Err("a.md".to_owned())));
        assert_eq!(h.head_path(&id("/r", "b.md")), Some(Ok("b.md".to_owned())));
    }

    #[test]
    fn every_spelling_mints_its_own_id() {
        let h = history(&[&[("docs/a.md", BODY)]], &["/one", "/two/three"]);
        let one = id("/one", "docs/a.md");
        let two = id("/two/three", "docs/a.md");
        assert_ne!(one, two);
        assert_eq!(h.head_path(&one), Some(Ok("docs/a.md".to_owned())));
        assert_eq!(h.head_path(&two), Some(Ok("docs/a.md".to_owned())));
        assert_eq!(
            h.head_path(&id("/other", "docs/a.md")),
            None,
            "a spelling that was not given mints nothing"
        );
    }

    #[test]
    fn the_id_is_artifact_id_from_abs_of_the_joined_path() {
        let h = history(&[&[("docs/a.md", BODY)]], &["/x/y"]);
        let want = artifact_id_from_abs(&Path::new("/x/y").join("docs/a.md"));
        assert_eq!(h.head_path(&want), Some(Ok("docs/a.md".to_owned())));
    }

    #[test]
    fn a_path_history_never_saw_and_a_non_markdown_path_mint_nothing() {
        let h = history(
            &[&[("docs/a.md", BODY), ("src/x.rs", b"fn x() {}\n")]],
            &["/r"],
        );
        assert_eq!(h.head_path(&id("/r", "docs/never.md")), None);
        assert_eq!(h.head_path(&id("/r", "src/x.rs")), None);
        assert_eq!(h.head_path("0000000000000000"), None);
    }
}
