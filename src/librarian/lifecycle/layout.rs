//! Where a tracker file sits under `docs/trackers/<lifecycle>/`, and what that makes it.
//!
//! Pure path logic: nothing here reads the filesystem. `place` sees one path; whether a
//! subfolder holds its own `readme.md` is a fact about sibling files, so the caller passes
//! that set to [`has_nested_readme`].

use std::collections::BTreeSet;

use crate::librarian::identity::key::{is_folder_index, Key, KeyError};

/// The tracker root, repo-relative.
pub const TRACKERS_DIR: &str = "docs/trackers";
/// The folder name that holds retired collections and retired entries.
pub const ARCHIVE_DIR: &str = "archive";

/// How a collection ages: a ledger lives on, a stream or a temp collection must end.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Lifecycle {
    Ledger,
    Stream,
    Temp,
}

impl Lifecycle {
    /// The folder name directly under [`TRACKERS_DIR`].
    pub fn dir_name(self) -> &'static str {
        match self {
            Lifecycle::Ledger => "ledgers",
            Lifecycle::Stream => "streams",
            Lifecycle::Temp => "temp",
        }
    }

    /// The lifecycle whose folder is named `s`; `None` for any other name.
    pub fn from_dir(s: &str) -> Option<Lifecycle> {
        match s {
            "ledgers" => Some(Lifecycle::Ledger),
            "streams" => Some(Lifecycle::Stream),
            "temp" => Some(Lifecycle::Temp),
            _ => None,
        }
    }

    /// True when a collection of this lifecycle must name its exit condition.
    pub fn requires_exit(self) -> bool {
        matches!(self, Lifecycle::Stream | Lifecycle::Temp)
    }
}

/// What a file is within its collection.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Role {
    CollectionReadme,
    Entry,
}

/// A tracker file's place in the tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Placement {
    pub lifecycle: Lifecycle,
    pub collection: Key,
    /// Repo-relative folder of the collection, for example
    /// `docs/trackers/ledgers/frictions` or `docs/trackers/streams/archive/x`.
    pub collection_dir: String,
    pub role: Role,
    /// True when the collection sits under `<lifecycle>/archive/`, or the file sits under an
    /// `archive/` folder below the collection.
    pub archived: bool,
}

/// Why a file below a lifecycle folder has no valid place.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlacementError {
    /// The file is not a markdown file.
    NotMarkdown(String),
    /// The file sits directly under a lifecycle folder (or its `archive/`), in no collection.
    OutsideCollection(String),
    /// The file sits in a subfolder that holds its own `readme.md`. `place` cannot see
    /// siblings, so [`has_nested_readme`] is how a caller finds this case.
    NestedCollection(String),
    /// A name that must be a key is not one: the collection folder name, or an entry file's
    /// stem. `rel_path` is the FILE's repo-relative path in both cases, so a problem names the
    /// file to fix; `error` carries the offending name.
    BadKey { rel_path: String, error: KeyError },
}

fn segments(rel_path: &str) -> Vec<String> {
    rel_path
        .replace('\\', "/")
        .split('/')
        .filter(|s| !s.is_empty() && *s != ".")
        .map(str::to_string)
        .collect()
}

/// The lifecycle when `segs` is below `docs/trackers/<lifecycle>/`; `None` otherwise.
fn lifecycle_of(segs: &[String]) -> Option<Lifecycle> {
    let root = segments(TRACKERS_DIR);
    if segs.len() < 4 || segs[..root.len()] != root[..] {
        return None;
    }
    Lifecycle::from_dir(&segs[root.len()])
}

/// Number of leading segments that make up the collection folder, and whether that folder
/// sits under `<lifecycle>/archive/`. `None` when no file lies below a collection folder.
/// `segs` must already be below `docs/trackers/<lifecycle>/`.
fn collection_depth(segs: &[String]) -> Option<(usize, bool)> {
    let first_below_lifecycle = segments(TRACKERS_DIR).len() + 1;
    let archived = segs
        .get(first_below_lifecycle)
        .is_some_and(|s| s == ARCHIVE_DIR);
    let depth = first_below_lifecycle + 1 + usize::from(archived);
    (segs.len() > depth).then_some((depth, archived))
}

/// Place a repo-relative path. `None` when it is not below `docs/trackers/<lifecycle>/`
/// (legacy flat trackers, other docs). `\` is read as `/`.
pub fn place(rel_path: &str) -> Option<Result<Placement, PlacementError>> {
    let segs = segments(rel_path);
    let lifecycle = lifecycle_of(&segs)?;
    let file = segs.last()?;
    if !file.to_ascii_lowercase().ends_with(".md") {
        return Some(Err(PlacementError::NotMarkdown(rel_path.to_string())));
    }
    let Some((depth, collection_archived)) = collection_depth(&segs) else {
        return Some(Err(PlacementError::OutsideCollection(rel_path.to_string())));
    };
    let collection = match Key::parse(&segs[depth - 1]) {
        Ok(k) => k,
        Err(error) => {
            return Some(Err(PlacementError::BadKey {
                rel_path: rel_path.to_string(),
                error,
            }))
        }
    };
    let role = if segs.len() == depth + 1 && is_folder_index(file) {
        Role::CollectionReadme
    } else {
        Role::Entry
    };
    // Folders strictly between the collection folder and the file.
    let below = &segs[depth..segs.len() - 1];
    let archived = collection_archived || below.iter().any(|s| s == ARCHIVE_DIR);
    Some(Ok(Placement {
        lifecycle,
        collection,
        collection_dir: segs[..depth].join("/"),
        role,
        archived,
    }))
}

/// True when a folder strictly between the collection folder and the file is in
/// `readme_dirs` (repo-relative, `/`-separated folders that hold a `readme.md`). A path that
/// does not place validly has no collection, so the answer is `false`.
pub fn has_nested_readme(rel_path: &str, readme_dirs: &BTreeSet<String>) -> bool {
    let Some(Ok(placement)) = place(rel_path) else {
        return false;
    };
    let segs = segments(rel_path);
    let depth = segments(&placement.collection_dir).len();
    (depth + 1..segs.len()).any(|k| readme_dirs.contains(&segs[..k].join("/")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(path: &str) -> Placement {
        place(path).expect("in tree").expect("valid")
    }

    #[test]
    fn readmes_and_entries_in_each_lifecycle() {
        let r = p("docs/trackers/ledgers/frictions/readme.md");
        assert_eq!(
            (r.lifecycle, r.collection.as_str(), r.role, r.archived),
            (
                Lifecycle::Ledger,
                "frictions",
                Role::CollectionReadme,
                false
            )
        );
        let e = p("docs/trackers/streams/tracker-organization/2026-11-11-a-friction.md");
        assert_eq!(
            (e.lifecycle, e.collection.as_str(), e.role),
            (Lifecycle::Stream, "tracker-organization", Role::Entry)
        );
        assert_eq!(
            e.collection_dir,
            "docs/trackers/streams/tracker-organization"
        );
        assert_eq!(
            p("docs/trackers/temp/x/README.md").role,
            Role::CollectionReadme
        );
        assert_eq!(
            p("docs/trackers/ledgers/frictions/reports/2026-01-01-r.md").role,
            Role::Entry
        );
    }

    #[test]
    fn archived_entries_and_archived_collections() {
        assert!(p("docs/trackers/ledgers/frictions/archive/2026-01-01-old.md").archived);
        let s = p("docs/trackers/streams/archive/local-onnx/readme.md");
        assert_eq!(
            (s.collection.as_str(), s.role, s.archived),
            ("local-onnx", Role::CollectionReadme, true)
        );
        assert_eq!(s.collection_dir, "docs/trackers/streams/archive/local-onnx");
        assert!(p("docs/trackers/streams/archive/local-onnx/archive/2026-01-01-e.md").archived);
    }

    #[test]
    fn outside_the_tree_is_none_and_misplaced_files_are_errors() {
        assert!(place("docs/trackers/bug-fix-session-log.md").is_none());
        assert!(place("docs/trackers/archive/old.md").is_none());
        assert!(place("docs/issues/2026-01-01-x.md").is_none());
        assert!(matches!(
            place("docs/trackers/ledgers/loose.md"),
            Some(Err(PlacementError::OutsideCollection(_)))
        ));
        assert!(matches!(
            place("docs/trackers/ledgers/archive/loose.md"),
            Some(Err(PlacementError::OutsideCollection(_)))
        ));
        assert!(matches!(
            place("docs/trackers/ledgers/Bad_Name/readme.md"),
            Some(Err(PlacementError::BadKey { rel_path, .. }))
                if rel_path == "docs/trackers/ledgers/Bad_Name/readme.md"
        ));
        assert!(matches!(
            place("docs/trackers/ledgers/x/data.json"),
            Some(Err(PlacementError::NotMarkdown(_)))
        ));
        assert_eq!(
            p(r"docs\trackers\ledgers\frictions\readme.md")
                .collection
                .as_str(),
            "frictions"
        );
    }

    #[test]
    fn a_subfolder_with_its_own_readme_is_a_nested_collection() {
        let dirs: std::collections::BTreeSet<String> = [
            "docs/trackers/ledgers/frictions".into(),
            "docs/trackers/ledgers/frictions/reports".into(),
        ]
        .into();
        assert!(has_nested_readme(
            "docs/trackers/ledgers/frictions/reports/2026-01-01-r.md",
            &dirs
        ));
        assert!(!has_nested_readme(
            "docs/trackers/ledgers/frictions/2026-01-01-r.md",
            &dirs
        ));
    }
}
