//! The key-space uniqueness gate, pure.
//!
//! [`check_tree`] turns the git-root-relative `.md` paths of one tree and a [`KeySpace`]
//! into a [`GateReport`]: which paths are outside the key space, and what is wrong with the
//! keys of the rest. It reads nothing; the caller lists the files (a commit hook lists
//! `HEAD`'s tree and the staged index) and loads the key-space file from the same tree.
//!
//! A path outside the key space has no key and takes no part in uniqueness. Only the paths
//! inside are indexed, with empty text: the gate asks about keys, not about tokens.
//!
//! [`GateReport::new_since`] compares two reports so a hook can refuse only the problems a
//! commit introduces: a collision between two files that were already colliding at `HEAD`
//! is not new.

use std::collections::{BTreeSet, HashMap, HashSet};

use super::index::{DocInput, IndexProblem, KeyIndex};
use super::key::{components, is_folder_index, Key, KeyError};
use super::key_space::{KeySpace, KEY_SPACE_FILE_REL};
use super::repo_name::RepoName;

/// What [`check_tree`] found in one tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GateReport {
    /// How many distinct paths are inside the key space.
    pub in_space: usize,
    /// The paths outside the key space, sorted. They have no key.
    pub outside: Vec<String>,
    /// What is wrong with the keys inside: invalid keys by path, then collisions by key, then
    /// partial folder exclusions by folder.
    pub problems: Vec<GateProblem>,
}

/// One thing wrong with the document keys of a tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GateProblem {
    /// A path inside the key space that gives no valid key.
    InvalidKey(KeyError),
    /// Two or more paths inside the key space with one key; `rel_paths` lists them all, sorted.
    DuplicateKey { key: Key, rel_paths: Vec<String> },
    /// A folder index (`readme.md`) is outside the key space while a sibling in the same
    /// folder is inside it. `folder` is the folder, relative to the git root and normalised
    /// (`""` is the git root). The siblings keep the folder-index ownership they would lose
    /// if the exclusion were honoured silently, so the gate asks for the whole folder instead.
    PartialFolderExclusion { folder: String },
}

/// What makes two problems the same problem across two reports, for the problems that one
/// value identifies. A collision is not one of them: see [`GateReport::new_since`].
#[derive(PartialEq, Eq, Hash)]
enum Identity<'a> {
    /// An invalid key is identified by its path; the path fixes the error.
    Invalid(&'a str),
    /// A partial exclusion is identified by its folder.
    Folder(&'a str),
}

impl GateProblem {
    fn identity(&self) -> Option<Identity<'_>> {
        match self {
            GateProblem::InvalidKey(e) => Some(Identity::Invalid(match e {
                KeyError::NotMarkdown(p) => p,
                KeyError::NonCanonical { rel_path, .. } | KeyError::HexShaped { rel_path, .. } => {
                    rel_path
                }
            })),
            GateProblem::DuplicateKey { .. } => None,
            GateProblem::PartialFolderExclusion { folder } => Some(Identity::Folder(folder)),
        }
    }

    /// One line that names what is wrong and how to fix it.
    pub fn render(&self) -> String {
        match self {
            GateProblem::InvalidKey(e) => e.to_string(),
            GateProblem::DuplicateKey { key, rel_paths } => format!(
                "the key {key} is held by {} documents ({}); rename all but one, or exclude a \
                 subtree in {KEY_SPACE_FILE_REL}",
                rel_paths.len(),
                rel_paths.join(", ")
            ),
            GateProblem::PartialFolderExclusion { folder } => {
                let place = if folder.is_empty() {
                    "the repo root".to_string()
                } else {
                    format!("the folder {folder:?}")
                };
                format!(
                    "the readme.md of {place} is outside the key space while a sibling document is \
                     inside it; exclude the whole folder in {KEY_SPACE_FILE_REL}, or none of it"
                )
            }
        }
    }
}

impl From<IndexProblem> for GateProblem {
    fn from(p: IndexProblem) -> GateProblem {
        match p {
            IndexProblem::InvalidKey(e) => GateProblem::InvalidKey(e),
            IndexProblem::DuplicateKey { key, rel_paths } => {
                GateProblem::DuplicateKey { key, rel_paths }
            }
        }
    }
}

impl GateReport {
    /// The problems in `self` that `before` does not have.
    ///
    /// An invalid key is the same problem when it is the same path; a partial exclusion, when
    /// it is the same folder. A collision is not new when every path it lists already collided
    /// under that key in `before`, so a collision that is equal or smaller (a member deleted or
    /// renamed away) is unchanged. It is new when it lists a path that was not in that
    /// collision before: a collision that gains a member, or swaps one in, or appears.
    pub fn new_since(&self, before: &GateReport) -> Vec<&GateProblem> {
        let mut known: HashSet<Identity<'_>> = HashSet::new();
        let mut collided: HashMap<&Key, HashSet<&str>> = HashMap::new();
        for p in &before.problems {
            match p {
                GateProblem::DuplicateKey { key, rel_paths } => {
                    collided
                        .entry(key)
                        .or_default()
                        .extend(rel_paths.iter().map(String::as_str));
                }
                other => known.extend(other.identity()),
            }
        }
        self.problems
            .iter()
            .filter(|p| match p {
                GateProblem::DuplicateKey { key, rel_paths } => !collided
                    .get(key)
                    .is_some_and(|was| rel_paths.iter().all(|path| was.contains(path.as_str()))),
                other => other.identity().is_none_or(|id| !known.contains(&id)),
            })
            .collect()
    }
}

/// The folder of a path, normalised as `key_of` reads it (`""` is the repo root).
fn folder_of(rel_path: &str) -> String {
    let parts = components(rel_path);
    parts[..parts.len().saturating_sub(1)].join("/")
}

/// Check the keys of `md_files`, the git-root-relative `.md` paths of one tree.
///
/// The result does not depend on the order of `md_files`, and a path listed twice counts once.
pub fn check_tree(this_repo: RepoName, space: &KeySpace, md_files: &[String]) -> GateReport {
    let files: BTreeSet<&str> = md_files.iter().map(String::as_str).collect();
    let (outside, inside): (Vec<&str>, Vec<&str>) =
        files.into_iter().partition(|p| space.is_outside(p));

    let index = KeyIndex::build(
        this_repo,
        inside.iter().map(|p| DocInput {
            rel_path: p.to_string(),
            text: String::new(),
        }),
    );
    let mut problems: Vec<GateProblem> = index
        .problems()
        .iter()
        .cloned()
        .map(GateProblem::from)
        .collect();

    let inside_folders: HashSet<String> = inside.iter().map(|p| folder_of(p)).collect();
    let partial: BTreeSet<String> = outside
        .iter()
        .filter(|p| is_folder_index(p))
        .map(|p| folder_of(p))
        .filter(|folder| inside_folders.contains(folder))
        .collect();
    problems.extend(
        partial
            .into_iter()
            .map(|folder| GateProblem::PartialFolderExclusion { folder }),
    );

    GateReport {
        in_space: inside.len(),
        outside: outside.into_iter().map(str::to_string).collect(),
        problems,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo() -> RepoName {
        RepoName::parse("codescout").unwrap()
    }
    fn files(paths: &[&str]) -> Vec<String> {
        paths.iter().map(|p| p.to_string()).collect()
    }
    fn check(exclude: &[&str], paths: &[&str]) -> GateReport {
        let space = KeySpace::from_patterns(exclude).unwrap();
        check_tree(repo(), &space, &files(paths))
    }
    fn dup(key: &str, paths: &[&str]) -> GateProblem {
        GateProblem::DuplicateKey {
            key: Key::parse(key).unwrap(),
            rel_paths: files(paths),
        }
    }

    #[test]
    fn outside_paths_are_listed_and_take_no_part_in_uniqueness() {
        let paths = ["contrib/pi/notes.md", "docs/notes.md"];
        let r = check(&["contrib"], &paths);
        assert_eq!(r.problems, vec![]);
        assert_eq!(r.outside, files(&["contrib/pi/notes.md"]));
        assert_eq!(r.in_space, 1);
        // Control: with nothing excluded the same two files collide.
        let r = check(&[], &paths);
        assert_eq!(
            r.problems,
            vec![dup("notes", &["contrib/pi/notes.md", "docs/notes.md"])]
        );
        assert_eq!((r.in_space, r.outside.len()), (2, 0));
    }

    #[test]
    fn a_collision_inside_the_space_is_reported_with_every_path() {
        let r = check(&[], &["c/DUP.md", "a/dup.md", "docs/unique.md", "b/dup.md"]);
        assert_eq!(
            r.problems,
            vec![dup("dup", &["a/dup.md", "b/dup.md", "c/DUP.md"])]
        );
        assert_eq!(r.in_space, 4);
        // The result does not depend on the order of the input.
        let reversed = check(&[], &["b/dup.md", "docs/unique.md", "a/dup.md", "c/DUP.md"]);
        assert_eq!(reversed, r);
        // A path listed twice counts once and does not collide with itself.
        let twice = check(&[], &["a/dup.md", "a/dup.md"]);
        assert_eq!((twice.in_space, twice.problems), (1, vec![]));
    }

    #[test]
    fn a_non_canonical_name_is_reported_not_skipped() {
        let r = check(&[], &["docs/v1.2-notes.md", "docs/Ünïcode Notes.md"]);
        assert_eq!(r.in_space, 2);
        assert_eq!(r.problems.len(), 2, "{:?}", r.problems);
        let invalid: Vec<&str> = r
            .problems
            .iter()
            .map(|p| match p {
                GateProblem::InvalidKey(KeyError::NonCanonical { rel_path, .. }) => {
                    rel_path.as_str()
                }
                other => panic!("expected a NonCanonical invalid key, got {other:?}"),
            })
            .collect();
        assert_eq!(invalid, ["docs/v1.2-notes.md", "docs/Ünïcode Notes.md"]);
    }

    #[test]
    fn an_excluded_folder_index_beside_included_siblings_is_a_partial_exclusion() {
        let paths = [
            "docs/x/readme.md",
            "docs/x/a.md",
            "docs/x/b.md",
            "docs/y/readme.md",
        ];
        let r = check(&["docs/x/readme.md"], &paths);
        assert_eq!(
            r.problems,
            vec![GateProblem::PartialFolderExclusion {
                folder: "docs/x".into()
            }],
            "reported once for the folder, not once per sibling"
        );
        assert_eq!(r.outside, files(&["docs/x/readme.md"]));
        // The whole folder excluded is a clean exclusion.
        assert_eq!(check(&["docs/x"], &paths).problems, vec![]);
        // An excluded sibling with the index inside is not a partial exclusion of the index.
        assert_eq!(check(&["docs/x/a.md"], &paths).problems, vec![]);
        // The index is the only document of its folder: nothing is left behind.
        assert_eq!(check(&["docs/y/readme.md"], &paths).problems, vec![]);
        // The readme name is matched case-insensitively, and the root is a folder too.
        let r = check(&["README.md"], &["README.md", "notes.md"]);
        assert_eq!(
            r.problems,
            vec![GateProblem::PartialFolderExclusion { folder: "".into() }]
        );
    }

    #[test]
    fn new_since_ignores_a_problem_that_is_unchanged() {
        let before = check(&[], &["a/x.md", "b/x.md", "docs/y.md"]);
        // Editing one of the colliding files, or adding an unrelated one, changes no problem.
        let after = check(&[], &["a/x.md", "b/x.md", "docs/y.md", "docs/z.md"]);
        assert_eq!(before.problems.len(), 1);
        assert_eq!(after.new_since(&before), Vec::<&GateProblem>::new());
        // Control: against a clean report the same collision is new.
        let clean = check(&[], &["a/x.md", "docs/y.md"]);
        assert_eq!(
            after.new_since(&clean),
            vec![&dup("x", &["a/x.md", "b/x.md"])]
        );
    }

    #[test]
    fn new_since_compares_invalid_keys_by_path_and_folders_by_name() {
        let paths = ["docs/Bad Name.md", "docs/x/readme.md", "docs/x/a.md"];
        let before = check(&["docs/x/readme.md"], &paths);
        assert_eq!(before.problems.len(), 2);
        assert_eq!(before.new_since(&before), Vec::<&GateProblem>::new());
        // A second bad name and a second partial folder are new; the first two are not.
        let after = check(
            &["docs/x/readme.md", "docs/z/readme.md"],
            &[
                "docs/Bad Name.md",
                "docs/Other Bad.md",
                "docs/x/readme.md",
                "docs/x/a.md",
                "docs/z/readme.md",
                "docs/z/b.md",
            ],
        );
        let new: Vec<String> = after
            .new_since(&before)
            .iter()
            .map(|p| p.render())
            .collect();
        assert_eq!(new.len(), 2, "{new:?}");
        assert!(new.iter().any(|l| l.contains("Other Bad.md")), "{new:?}");
        assert!(new.iter().any(|l| l.contains("docs/z")), "{new:?}");
    }

    #[test]
    fn new_since_reports_a_collision_that_gains_a_member() {
        let before = check(&[], &["a/x.md", "b/x.md"]);
        let after = check(&[], &["a/x.md", "b/x.md", "c/x.md"]);
        assert_eq!(
            after.new_since(&before),
            vec![&dup("x", &["a/x.md", "b/x.md", "c/x.md"])]
        );
    }
    #[test]
    fn new_since_ignores_a_collision_that_loses_a_member() {
        let before = check(&[], &["a/x.md", "b/x.md", "c/x.md"]);
        let after = check(&[], &["a/x.md", "b/x.md"]);
        assert_eq!(before.problems.len(), 1);
        assert_eq!(after.problems.len(), 1, "still a collision, only smaller");
        assert_eq!(after.new_since(&before), Vec::<&GateProblem>::new());
    }

    #[test]
    fn new_since_reports_a_collision_that_swaps_in_a_new_path() {
        let before = check(&[], &["a/x.md", "b/x.md"]);
        let after = check(&[], &["a/x.md", "c/x.md"]);
        assert_eq!(
            after.new_since(&before),
            vec![&dup("x", &["a/x.md", "c/x.md"])]
        );
    }

    #[test]
    fn new_since_reports_a_staged_file_that_collides_with_an_unchanged_one() {
        let at_head = check(&[], &["docs/old.md", "a/x.md"]);
        let staged = check(&[], &["docs/old.md", "a/x.md", "b/x.md"]);
        let new = staged.new_since(&at_head);
        assert_eq!(new, vec![&dup("x", &["a/x.md", "b/x.md"])]);
        let line = new[0].render();
        assert!(
            line.contains("a/x.md") && line.contains("b/x.md"),
            "the line names both paths: {line}"
        );
        // Control: a collision between two unchanged files is not new.
        let unchanged = check(&[], &["a/x.md", "b/x.md"]);
        let staged = check(&[], &["a/x.md", "b/x.md", "docs/fresh.md"]);
        assert_eq!(staged.new_since(&unchanged), Vec::<&GateProblem>::new());
    }

    #[test]
    fn un_excluding_a_tree_in_the_key_space_file_reports_the_problems_it_creates() {
        let paths = ["contrib/x.md", "docs/x.md", "contrib/Bad Name.md"];
        let at_head = check(&["contrib"], &paths);
        assert_eq!(at_head.problems, vec![]);
        // The commit touches only the key-space file: same tree, narrower exclusion.
        let staged = check(&[], &paths);
        let new = staged.new_since(&at_head);
        assert_eq!(new.len(), 2, "{new:?}");
        assert!(new.contains(&&dup("x", &["contrib/x.md", "docs/x.md"])));
        assert!(new.iter().any(|p| matches!(p, GateProblem::InvalidKey(_))));
        // Widening the exclusion creates nothing.
        assert_eq!(at_head.new_since(&staged), Vec::<&GateProblem>::new());
    }

    #[test]
    fn an_empty_tree_is_clean() {
        let r = check_tree(repo(), &KeySpace::empty(), &[]);
        assert_eq!(
            r,
            GateReport {
                in_space: 0,
                outside: vec![],
                problems: vec![]
            }
        );
        assert_eq!(r.new_since(&r), Vec::<&GateProblem>::new());
    }

    #[test]
    fn render_is_one_line_with_the_remedy_for_each_problem() {
        let r = check(
            &["docs/x/readme.md"],
            &[
                "a/x.md",
                "b/x.md",
                "docs/Bad Name.md",
                "docs/x/readme.md",
                "docs/x/a.md",
            ],
        );
        assert_eq!(r.problems.len(), 3, "{:?}", r.problems);
        for p in &r.problems {
            let line = p.render();
            assert!(!line.contains('\n'), "{line}");
            assert!(!line.is_empty());
        }
        let lines: Vec<String> = r.problems.iter().map(GateProblem::render).collect();
        assert!(lines[0].contains("rename the file"), "{}", lines[0]);
        assert!(
            lines[1].contains("rename all but one") && lines[1].contains(KEY_SPACE_FILE_REL),
            "{}",
            lines[1]
        );
        assert!(
            lines[2].contains("docs/x") && lines[2].contains("exclude the whole folder"),
            "{}",
            lines[2]
        );
    }
}
