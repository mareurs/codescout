//! Load one collection from disk: its readme, its template and the entries in its folder.
//!
//! `scan::load_docs` reads the whole checkout through git; a tool that works on one collection
//! must not pay for that, nor fail on a file of another collection. `load_collection_dir` reads
//! the files of one folder (the `archive/` below it included) through the read door, builds a
//! [`LifecycleTree`] from those files alone, and hands back the collection with that tree.
//!
//! The folder is walked with `std::fs`, not git, so it works from any checkout root, a worktree
//! included. A subfolder that is a symbolic link is not entered; a file that is one is offered to
//! the door, which refuses a link that leaves the repository.

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::Value;

use super::layout::{place, Lifecycle, Role, TRACKERS_DIR};
use super::schema::{parse_schema, CollectionSchema, SchemaError, SCHEMA_KEYS};
use super::tree::{CollectionNode, DocInput, LifecycleTree};
use super::write::entry_template;
use crate::librarian::frontmatter::{self, Frontmatter};
use crate::librarian::identity::key::{components, folder_of, is_folder_index, Key};
use crate::librarian::read_door::{self, ReadError};

/// One collection and the tree built from its folder alone.
#[derive(Clone, Debug, PartialEq)]
pub struct LoadedCollection {
    /// Built from this one folder: its `collections` hold exactly this collection, and its
    /// `problems` are the folder's (an entry that does not parse, a second readme) and do not
    /// keep the collection from loading.
    pub tree: LifecycleTree,
    pub key: Key,
    pub lifecycle: Lifecycle,
    /// Repo-relative folder of the collection, `/`-separated, no trailing slash.
    pub dir: String,
    /// The `## Entry template` section of the readme, when it has a non-empty one.
    pub template: Option<String>,
}

impl LoadedCollection {
    /// The collection's node in [`Self::tree`]: live and archived entries both.
    pub fn node(&self) -> &CollectionNode {
        self.tree
            .collections
            .iter()
            .find(|c| c.dir == self.dir)
            .expect("a LoadedCollection is only built from a tree that holds its collection")
    }

    pub fn schema(&self) -> &CollectionSchema {
        &self.node().schema
    }
}

/// Why a collection could not be loaded. Each text is `<problem>; <remedy>`.
#[derive(Debug, thiserror::Error)]
pub enum CollectionLoadError {
    /// The key is no key: refused before anything is read.
    #[error("{0:?} is not a collection key (lowercase letters and digits, a single '-' between runs); pass the key of a collection, such as `frictions`")]
    BadKey(String),
    /// No live folder holds a readme for the key.
    #[error("no collection {0:?} has a readme in docs/trackers/ledgers, streams or temp; pass the key of an existing collection, or the folder of an archived one")]
    NotFound(String),
    /// Two or more live folders hold a readme for the key; `dirs` are sorted.
    #[error("the key {key:?} names {} collections ({}); pass the folder of the one you mean", .dirs.len(), .dirs.join(", "))]
    Ambiguous { key: String, dirs: Vec<String> },
    /// The folder is not the folder of a collection readme, or holds none.
    #[error("{0:?} is not the folder of a collection readme; give a folder such as docs/trackers/ledgers/<key> that holds a readme.md")]
    NotACollectionDir(String),
    /// A file of the folder, or the folder, could not be read: `<path>: <reason>`.
    #[error(
        "{0}; fix the file or folder so that it can be read, a regular file inside the repository"
    )]
    Unreadable(String),
    /// The readme gave no collection: the `Display` of each `TreeProblem` of the folder.
    #[error("the collection cannot be loaded: {}; fix each problem named", .0.join(" | "))]
    Broken(Vec<String>),
}

/// The three live lifecycle folders, in the order their names sort.
const LIFECYCLES: [Lifecycle; 3] = [Lifecycle::Ledger, Lifecycle::Stream, Lifecycle::Temp];

/// True when `dir` (repo-relative) holds a readme, whatever its case, directly in it.
fn has_readme(root: &Path, dir: &str) -> bool {
    let Ok(read) = std::fs::read_dir(root.join(dir)) else {
        return false;
    };
    read.flatten().any(|e| {
        e.file_name().to_str().is_some_and(is_folder_index)
            && e.file_type().is_ok_and(|t| !t.is_dir())
    })
}

/// Load the live collection `key`: the folder `docs/trackers/<lifecycle>/<key>/` that holds a
/// readme, in `ledgers`, `streams` or `temp` under `root`. An archived collection is not looked
/// for: name it by folder with [`load_collection_dir`].
///
/// The key is checked before any file is touched.
pub fn load_collection(root: &Path, key: &str) -> Result<LoadedCollection, CollectionLoadError> {
    let parsed = Key::parse(key).map_err(|_| CollectionLoadError::BadKey(key.to_string()))?;
    let mut dirs: Vec<String> = LIFECYCLES
        .iter()
        .map(|l| format!("{TRACKERS_DIR}/{}/{parsed}", l.dir_name()))
        .filter(|dir| has_readme(root, dir))
        .collect();
    dirs.sort();
    match dirs.len() {
        0 => Err(CollectionLoadError::NotFound(key.to_string())),
        1 => load_collection_dir(root, &dirs[0]),
        _ => Err(CollectionLoadError::Ambiguous {
            key: key.to_string(),
            dirs,
        }),
    }
}

/// The text of a read error without the machine's paths, on one line with no `; ` in it (the
/// caller appends a remedy after one). The file's own `<root>/<rel_path>: ` prefix goes, and any
/// other path under a root loses its `<root>/`.
fn portable_reason(text: &str, roots: &[&Path], rel_path: &str) -> String {
    let mut text = text.to_string();
    for root in roots {
        let root = root.display();
        text = text.replace(&format!("{root}/{rel_path}: "), "");
        text = text.replace(&format!("{root}/"), "");
    }
    text.replace("; ", ", ")
}

/// Every file below `dir` (repo-relative) as a repo-relative `/` path, into `out`. A name that is
/// not UTF-8 cannot be a repo-relative path and is left out.
fn list_files(
    root: &Path,
    dir: &str,
    roots: &[&Path],
    out: &mut Vec<String>,
) -> Result<(), CollectionLoadError> {
    let unreadable = |e: std::io::Error| {
        CollectionLoadError::Unreadable(format!(
            "{dir}: {}",
            portable_reason(&e.to_string(), roots, dir)
        ))
    };
    for entry in std::fs::read_dir(root.join(dir)).map_err(unreadable)? {
        let entry = entry.map_err(unreadable)?;
        let Some(name) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        let rel = format!("{dir}/{name}");
        let kind = entry.file_type().map_err(unreadable)?;
        if kind.is_dir() {
            list_files(root, &rel, roots, out)?;
        } else if kind.is_symlink() && std::fs::metadata(entry.path()).is_ok_and(|m| m.is_dir()) {
            // A linked folder is not entered: it may lead anywhere.
        } else {
            out.push(rel);
        }
    }
    Ok(())
}

/// Read every file below `dir` through the read door, as `scan::load_docs_with` does for a
/// checkout: a Markdown file gets its text, any other file gets none (`place` refuses it by
/// name), and a file gone between the listing and the read is skipped. Any other failed read is
/// an error, not a skip: a collection read without one of its files would be a wrong one.
fn read_docs(root: &Path, dir: &str) -> Result<Vec<DocInput>, CollectionLoadError> {
    let canonical_root = root.canonicalize().ok();
    let roots: Vec<&Path> = std::iter::once(root)
        .chain(canonical_root.as_deref())
        .collect();
    let mut paths = Vec::new();
    list_files(root, dir, &roots, &mut paths)?;
    paths.sort();
    let door = read_door::for_project(root);
    let mut docs = Vec::with_capacity(paths.len());
    for rel_path in paths {
        if !rel_path.to_ascii_lowercase().ends_with(".md") {
            docs.push(DocInput {
                rel_path,
                text: String::new(),
            });
            continue;
        }
        match door.text(&root.join(&rel_path)) {
            Ok(text) => docs.push(DocInput { rel_path, text }),
            Err(ReadError::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                return Err(CollectionLoadError::Unreadable(format!(
                    "{rel_path}: {}",
                    portable_reason(&e.to_string(), &roots, &rel_path)
                )))
            }
        }
    }
    Ok(docs)
}

/// Load the collection whose folder is `dir` (repo-relative; `\` is read as `/`, a trailing
/// separator is dropped): live or archived, whatever `layout::place` calls the folder's readme.
///
/// Only the files below `dir` are read. A problem with an entry, or a second readme, is in
/// `tree.problems` and does not fail the load; a readme that gives no collection is `Broken`.
pub fn load_collection_dir(
    root: &Path,
    dir: &str,
) -> Result<LoadedCollection, CollectionLoadError> {
    let not_a_collection = || CollectionLoadError::NotACollectionDir(dir.to_string());
    let probe = format!("{}/readme.md", components(dir).join("/"));
    let placement = match place(&probe) {
        Some(Ok(p)) if p.role == Role::CollectionReadme => p,
        _ => return Err(not_a_collection()),
    };
    let dir = placement.collection_dir.clone();
    if !has_readme(root, &dir) {
        return Err(not_a_collection());
    }
    let docs = read_docs(root, &dir)?;
    // The readme is the first in path order, as `LifecycleTree::build` reads it.
    let template = docs
        .iter()
        .filter(|d| is_folder_index(&d.rel_path) && folder_of(&d.rel_path) == dir)
        .min_by(|a, b| a.rel_path.cmp(&b.rel_path))
        .and_then(|readme| frontmatter::parse(&readme.text).ok())
        .and_then(|(_, body)| entry_template(body));
    let tree = LifecycleTree::build(docs);
    if !tree.collections.iter().any(|c| c.dir == dir) {
        let mut problems: Vec<String> = tree.problems.iter().map(ToString::to_string).collect();
        if problems.is_empty() {
            problems.push(format!("{dir}: the readme gives no collection"));
        }
        return Err(CollectionLoadError::Broken(problems));
    }
    Ok(LoadedCollection {
        tree,
        key: placement.collection,
        lifecycle: placement.lifecycle,
        dir,
        template,
    })
}
/// The `kind` a collection readme carries.
const README_KIND: &str = "collection";

/// Render the text of a collection readme: a frontmatter block holding `kind: collection`, the
/// `title` and the schema keys of `extra`, then `body`. It carries no `id:`.
///
/// `extra` may hold only the keys of [`SCHEMA_KEYS`]; any other key (an `id`, a `kind` and a
/// misspelt key alike) is `Malformed` with the key in `detail`, because `frontmatter::write`
/// would drop a reserved key without a word. The rendered text is read back with
/// [`parse_schema`], so a text that returns `Ok` is a readme whose schema parses; every problem
/// found is returned, the unknown keys first.
pub fn collection_readme(
    lifecycle: Lifecycle,
    title: &str,
    extra: &BTreeMap<String, Value>,
    body: &str,
) -> Result<String, Vec<SchemaError>> {
    // `Malformed.key` is a `&'static str`: an unknown key is dynamic, so the error is filed
    // under the `extra` map and carries the key in `detail`.
    let mut errs: Vec<SchemaError> = extra
        .keys()
        .filter(|k| !SCHEMA_KEYS.contains(&k.as_str()))
        .map(|k| SchemaError::Malformed {
            key: "extra",
            detail: format!(
                "unknown key {k:?}; a collection readme takes only {}",
                SCHEMA_KEYS.join(", ")
            ),
        })
        .collect();
    let fm = Frontmatter {
        kind: Some(README_KIND.to_string()),
        title: Some(title.trim().to_string()),
        extra: extra
            .iter()
            .filter(|(k, _)| SCHEMA_KEYS.contains(&k.as_str()))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect(),
        ..Frontmatter::default()
    };
    let body = if body.trim().is_empty() {
        String::new()
    } else {
        frontmatter::render_body(body)
    };
    let text = frontmatter::write(&fm, &body);
    match frontmatter::parse(&text) {
        Ok((Some(reread), _)) => {
            if let Err(found) = parse_schema(lifecycle, &reread) {
                errs.extend(found);
            }
        }
        Ok((None, _)) => errs.push(SchemaError::Malformed {
            key: "extra",
            detail: "the rendered readme has no frontmatter; pass schema values that render as YAML"
                .to_string(),
        }),
        Err(e) => errs.push(SchemaError::Malformed {
            key: "extra",
            detail: format!(
                "the rendered readme does not read back ({e}); pass schema values that render as YAML"
            ),
        }),
    }
    if errs.is_empty() {
        Ok(text)
    } else {
        Err(errs)
    }
}

#[cfg(test)]
mod tests;
