//! Renames of the files whose names give no valid key (D-R14, D-R21, D-R25, D-R26).
//!
//! A file such as `docs/My_Notes.md` has no key: the key is its lowercase stem and `my_notes` is
//! not a canonical name. The cutover renames it in its own folder to `docs/my-notes.md`, unless a
//! reason in the list below stops it; a stopped rename is reported, never forced. A rename is
//! refused when:
//!
//! - the file is a folder index (its folder is the problem, not its name),
//! - nothing of the stem is left, or what is left is shaped like a repo id,
//! - the target path exists, or the new key is a key `HEAD` already holds,
//! - code reads the file by path, a dot directory holds it or an mdBook reads it (frozen, D-R26),
//! - non-markdown text anywhere in `HEAD` names the file (D-R21 b),
//! - another rename of the same run would give the same key.
//!
//! [`renames_already_in_head`] is the same rule read backwards for a run after the commit: the old
//! catalog still knows `docs/My_Notes.md`, `HEAD` holds `docs/my-notes.md`.

use std::collections::{BTreeMap, BTreeSet};

use crate::librarian::cutover::code_refs::CodeRefs;
use crate::librarian::cutover::old_rows::OldRows;
use crate::librarian::cutover::plan::{Class, Item, Status};
use crate::librarian::identity::index::IndexProblem;
use crate::librarian::identity::key::{
    folder_of, is_folder_index, key_of, markdown_stem, Key, KeyError,
};
use crate::librarian::identity::repo_name::is_hex16;

/// The refusal reasons, as the report prints them.
const REASON_FOLDER_INDEX: &str = "folder index: rename the folder by hand";
const REASON_NO_NAME: &str = "no canonical name";
const REASON_HEX: &str = "hex-shaped name";
const REASON_TARGET: &str = "target exists";
const REASON_COLLISION: &str = "key collision";
const REASON_NAMED_BY_CODE: &str = "named by code";

/// A file renamed in its own folder to a name that gives a valid key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rename {
    pub from: String,
    pub to: String,
    /// What [`key_of`] gave for `from`: its stem in ASCII lowercase. A wiki link written with it
    /// (`[[my_notes]]`) is not in the grammar, so [`RenameSet::keys`] is how it finds the new key.
    pub old_key: String,
    pub new_key: Key,
}

/// The renames of one plan: the ones this run applies, and the ones `HEAD` already holds.
#[derive(Debug, Default)]
pub struct RenameSet {
    pub planned: Vec<Rename>,
    pub in_head: Vec<Rename>,
}

impl RenameSet {
    fn all(&self) -> impl Iterator<Item = &Rename> {
        self.planned.iter().chain(&self.in_head)
    }

    /// Old path to final path, for both kinds.
    pub fn paths(&self) -> BTreeMap<String, String> {
        self.all().map(|r| (r.from.clone(), r.to.clone())).collect()
    }

    /// Old key (the lowercase stem) to the key of the renamed file.
    pub fn keys(&self) -> BTreeMap<String, Key> {
        self.all()
            .map(|r| (r.old_key.clone(), r.new_key.clone()))
            .collect()
    }

    /// Old key to every document of `HEAD` it could have named, for each old key that a document
    /// other than the renamed file also has: the same lowercase stem, its rename not applied (it was
    /// refused, or it has no rename). A link written with such a key is not followed to the renamed
    /// file. The renamed file is named by the path `HEAD` holds: the old one while this run renames
    /// it, the new one after the commit.
    pub fn ambiguous_keys<'a>(
        &self,
        head_paths: impl Iterator<Item = &'a String>,
    ) -> BTreeMap<String, Vec<String>> {
        let moved = self.paths();
        let renamed: BTreeSet<&str> = self.all().map(|r| r.old_key.as_str()).collect();
        let mut found: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for path in head_paths.filter(|p| !moved.contains_key(*p)) {
            // The key `key_of` gives, not the stem: a folder index has its folder's.
            let Err(KeyError::NonCanonical { key, .. }) = key_of(path) else {
                continue;
            };
            let key = key.to_ascii_lowercase();
            if renamed.contains(key.as_str()) {
                found.entry(key).or_default().push(path.clone());
            }
        }
        let held_by_renamed = self
            .planned
            .iter()
            .map(|r| (&r.old_key, &r.from))
            .chain(self.in_head.iter().map(|r| (&r.old_key, &r.to)));
        for (old_key, path) in held_by_renamed {
            if let Some(paths) = found.get_mut(old_key) {
                paths.push(path.clone());
            }
        }
        for paths in found.values_mut() {
            paths.sort();
            paths.dedup();
        }
        found
    }
}

/// The new stem for `stem`: ASCII lowercase, every run of other characters replaced by one `-`,
/// no `-` at either end. `None` when no letter or digit is left.
pub fn canonical_stem(stem: &str) -> Option<String> {
    let mut out = String::with_capacity(stem.len());
    let mut gap = false;
    for c in stem.chars() {
        if c.is_ascii_alphanumeric() {
            if gap && !out.is_empty() {
                out.push('-');
            }
            gap = false;
            out.push(c.to_ascii_lowercase());
        } else {
            gap = true;
        }
    }
    (!out.is_empty()).then_some(out)
}

fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn stem_of(path: &str) -> &str {
    markdown_stem(file_name(path)).unwrap_or("")
}

/// `path` with its stem replaced, in the same folder.
fn with_stem(path: &str, stem: &str) -> String {
    let folder = folder_of(path);
    if folder.is_empty() {
        format!("{stem}.md")
    } else {
        format!("{folder}/{stem}.md")
    }
}

/// How the name of one file with no valid key can be repaired.
enum Repair {
    /// The rename the rule gives.
    To(Rename),
    /// The rule gives none, for this reason.
    Refused(&'static str),
}

/// The repair of a file whose key is `NonCanonical`: what the rename rule gives.
fn repair(path: &str) -> Repair {
    if is_folder_index(path) {
        return Repair::Refused(REASON_FOLDER_INDEX);
    }
    let Some(new_stem) = canonical_stem(stem_of(path)) else {
        return Repair::Refused(REASON_NO_NAME);
    };
    if is_hex16(&new_stem) {
        return Repair::Refused(REASON_HEX);
    }
    let Ok(new_key) = Key::parse(&new_stem) else {
        return Repair::Refused(REASON_NO_NAME);
    };
    Repair::To(Rename {
        from: path.to_owned(),
        to: with_stem(path, &new_stem),
        old_key: stem_of(path).to_ascii_lowercase(),
        new_key,
    })
}

/// `true` when `path` has no valid key because of its name.
fn has_non_canonical_key(path: &str) -> bool {
    matches!(key_of(path), Err(KeyError::NonCanonical { .. }))
}

fn blocked(path: &str, reason: &str, detail: Option<String>, suggestion: Option<String>) -> Item {
    Item {
        class: Class::Rename,
        status: Status::Unrecoverable,
        reason: reason.to_owned(),
        path: Some(path.to_owned()),
        line: None,
        detail,
        suggestion,
    }
}

/// The item of a rename the plan applies.
pub fn applied_item(rename: &Rename) -> Item {
    Item {
        class: Class::Rename,
        status: Status::Moved,
        reason: "renamed to a name that gives a valid key".to_owned(),
        path: Some(rename.from.clone()),
        line: None,
        detail: Some(format!("{} -> {}", rename.from, rename.to)),
        suggestion: None,
    }
}

/// The item of a rename `HEAD` already holds (D-R25).
pub fn in_head_item(rename: &Rename) -> Item {
    Item {
        class: Class::Rename,
        status: Status::AlreadyExported,
        reason: "the rename is already in HEAD".to_owned(),
        path: Some(rename.to.clone()),
        line: None,
        detail: Some(format!("{} -> {}", rename.from, rename.to)),
        suggestion: None,
    }
}

/// A name for a file whose key another file holds: `<parent folder>-<stem>`, in its folder. `None`
/// when the folder is the repository root or an index, the name gives nothing, the result is no
/// valid key, or the key and the path are taken.
fn suggestion_for(
    path: &str,
    taken_paths: &BTreeSet<&str>,
    taken_keys: &BTreeSet<Key>,
) -> Option<String> {
    if is_folder_index(path) {
        return None;
    }
    let folder = folder_of(path);
    let parent = canonical_stem(folder.rsplit('/').next().filter(|f| !f.is_empty())?)?;
    let stem = canonical_stem(stem_of(path))?;
    let suggested = format!("{parent}-{stem}");
    let key = Key::parse(&suggested).ok()?;
    let target = with_stem(path, &suggested);
    (!taken_keys.contains(&key) && !taken_paths.contains(target.as_str())).then_some(target)
}

/// Plan the renames of the files whose names give no valid key.
///
/// `paths` is every markdown path of `HEAD`, inside the key space or outside it (a target there is
/// taken as well); `keys` is every valid key `HEAD` holds; `problems` are the index's findings for
/// the documents inside the key space. Returns the renames this run applies and one `unrecoverable`
/// item for every file that has no valid key and is not renamed, and for each document of a key
/// `HEAD` already shares.
pub fn plan_renames(
    paths: &[String],
    keys: &BTreeSet<Key>,
    problems: &[IndexProblem],
    refs: &CodeRefs,
) -> (Vec<Rename>, Vec<Item>) {
    plan_renames_against(paths, &BTreeSet::new(), keys, problems, refs)
}

/// [`plan_renames`] with the paths the old catalog still holds a live row for but `HEAD` lacks
/// (`held`): a rename onto one of them would put two documents' rows on one path (D-R25).
pub(crate) fn plan_renames_against(
    paths: &[String],
    held: &BTreeSet<String>,
    keys: &BTreeSet<Key>,
    problems: &[IndexProblem],
    refs: &CodeRefs,
) -> (Vec<Rename>, Vec<Item>) {
    let taken_paths: BTreeSet<&str> = paths.iter().map(String::as_str).collect();
    let mut items = Vec::new();
    let mut survivors: Vec<Rename> = Vec::new();
    for problem in problems {
        match problem {
            IndexProblem::InvalidKey(KeyError::NonCanonical { rel_path, .. }) => {
                match repair(rel_path) {
                    Repair::Refused(reason) => items.push(blocked(rel_path, reason, None, None)),
                    Repair::To(rename) => {
                        match blocker_of(&rename, &taken_paths, held, keys, refs) {
                            Some(item) => items.push(item),
                            None => survivors.push(rename),
                        };
                    }
                }
            }
            IndexProblem::InvalidKey(KeyError::HexShaped { rel_path, .. }) => {
                items.push(blocked(rel_path, REASON_HEX, None, None));
            }
            IndexProblem::InvalidKey(KeyError::NotMarkdown(_)) => {}
            IndexProblem::DuplicateKey { key, rel_paths } => {
                for path in rel_paths {
                    let others: Vec<&str> = rel_paths
                        .iter()
                        .map(String::as_str)
                        .filter(|p| *p != path)
                        .collect();
                    let detail = format!("the key {key} is also held by {}", others.join(", "));
                    let suggestion = suggestion_for(path, &taken_paths, keys);
                    items.push(blocked(path, REASON_COLLISION, Some(detail), suggestion));
                }
            }
        }
    }

    // Two renames of one run that give one key: neither is applied. Renames that another reason
    // already stopped do not count, because they will not happen.
    let mut by_key: BTreeMap<&Key, Vec<&Rename>> = BTreeMap::new();
    for rename in &survivors {
        by_key.entry(&rename.new_key).or_default().push(rename);
    }
    let mut renames = Vec::new();
    for rename in &survivors {
        let sharing = &by_key[&rename.new_key];
        if sharing.len() == 1 {
            renames.push(rename.clone());
            continue;
        }
        let others: Vec<&str> = sharing
            .iter()
            .map(|r| r.from.as_str())
            .filter(|p| *p != rename.from)
            .collect();
        let detail = format!(
            "{} would give the key {}, which {} would also get",
            rename.to,
            rename.new_key,
            others.join(", ")
        );
        let suggestion = suggestion_for(&rename.from, &taken_paths, keys);
        items.push(blocked(
            &rename.from,
            REASON_COLLISION,
            Some(detail),
            suggestion,
        ));
    }
    renames.sort_by(|a, b| a.from.cmp(&b.from));
    items.sort_by(|a, b| a.path.cmp(&b.path));
    (renames, items)
}

/// Why `rename` cannot be applied, as an item; `None` when nothing in `HEAD` stops it.
fn blocker_of(
    rename: &Rename,
    taken_paths: &BTreeSet<&str>,
    held: &BTreeSet<String>,
    keys: &BTreeSet<Key>,
    refs: &CodeRefs,
) -> Option<Item> {
    let would_be = format!("would be {}", rename.to);
    if taken_paths.contains(rename.to.as_str()) {
        return Some(blocked(&rename.from, REASON_TARGET, Some(would_be), None));
    }
    if held.contains(&rename.to) {
        let detail = format!("{would_be}; the old catalog still holds a live row for it");
        return Some(blocked(&rename.from, REASON_TARGET, Some(detail), None));
    }
    if keys.contains(&rename.new_key) {
        let detail = Some(format!(
            "{} would give the key {}, which a document of HEAD holds",
            rename.to, rename.new_key
        ));
        let suggestion = suggestion_for(&rename.from, taken_paths, keys);
        return Some(blocked(&rename.from, REASON_COLLISION, detail, suggestion));
    }
    if let Some(why) = refs.frozen(&rename.from) {
        return Some(blocked(
            &rename.from,
            &format!("frozen: {why}"),
            Some(would_be),
            None,
        ));
    }
    // D-R21 b: a name tools read is not ours to change, wherever the text that holds it sits. The
    // reason stays one constant (it is the key of the report's tally); the first place and the
    // count go in the detail, as `frozen` does it.
    let mut named = refs.mentions(file_name(&rename.from));
    named.extend(refs.mentions(&rename.from));
    named.sort();
    named.dedup();
    let first = named.first()?;
    let more = match named.len() {
        1 => String::new(),
        n => format!(" and {} more", n - 1),
    };
    let detail = format!("{first}{more}; {would_be}");
    Some(blocked(
        &rename.from,
        REASON_NAMED_BY_CODE,
        Some(detail),
        None,
    ))
}

/// D-R25: the renames a run after the commit finds already made. An old-catalog path that `HEAD`
/// lacks maps to its canonical sibling in the same folder when `HEAD` holds that file and the old
/// catalog has no live row for it (so the sibling did not exist before the cutover).
///
/// Two old paths that name one sibling are no evidence for either: neither maps.
pub fn renames_already_in_head(old: &OldRows, head_paths: &BTreeSet<String>) -> Vec<Rename> {
    let live: BTreeSet<&str> = old
        .documents
        .values()
        .filter(|d| !d.missing)
        .map(|d| d.rel_path.as_str())
        .collect();
    let mut found: BTreeMap<String, Rename> = BTreeMap::new();
    let mut claimed: BTreeMap<String, usize> = BTreeMap::new();
    for doc in old.documents.values() {
        let from = &doc.rel_path;
        if head_paths.contains(from) || found.contains_key(from) || !has_non_canonical_key(from) {
            continue;
        }
        let Repair::To(rename) = repair(from) else {
            continue;
        };
        if !head_paths.contains(&rename.to) || live.contains(rename.to.as_str()) {
            continue;
        }
        *claimed.entry(rename.to.clone()).or_default() += 1;
        found.insert(from.clone(), rename);
    }
    let mut renames: Vec<Rename> = found
        .into_values()
        .filter(|r| claimed[&r.to] == 1)
        .collect();
    renames.sort_by(|a, b| a.from.cmp(&b.from));
    renames
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::cutover::old_rows::OldDocument;
    use crate::librarian::derived::scan::open_commit;
    use crate::librarian::derived::testkit::{repo_with, FILE};
    use crate::librarian::identity::index::{DocInput, KeyIndex};
    use crate::librarian::identity::repo_name::RepoName;

    /// What `plan_renames` gives for the committed `files` (markdown ones are documents).
    fn run(files: &[(&str, &str)]) -> (Vec<Rename>, Vec<Item>) {
        run_held(files, &[])
    }

    /// [`run`] with paths the old catalog still holds a live row for.
    fn run_held(files: &[(&str, &str)], held: &[&str]) -> (Vec<Rename>, Vec<Item>) {
        let dir = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(dir.path()).unwrap();
        let entries: Vec<(&str, u32, &[u8])> = files
            .iter()
            .map(|&(p, text)| (p, FILE, text.as_bytes()))
            .collect();
        let repo = repo_with(&root, &entries);
        let commit = open_commit(&repo, "HEAD").unwrap();
        let refs = CodeRefs::load(&repo, &commit).unwrap();
        let docs: Vec<DocInput> = files
            .iter()
            .filter(|(p, _)| p.ends_with(".md"))
            .map(|&(p, text)| DocInput {
                rel_path: p.to_owned(),
                text: text.to_owned(),
            })
            .collect();
        let paths: Vec<String> = docs.iter().map(|d| d.rel_path.clone()).collect();
        let keys: BTreeSet<Key> = paths.iter().filter_map(|p| key_of(p).ok()).collect();
        let held: BTreeSet<String> = held.iter().map(|p| (*p).to_owned()).collect();
        let index = KeyIndex::build(RepoName::parse("demo").unwrap(), docs);
        plan_renames_against(&paths, &held, &keys, index.problems(), &refs)
    }

    #[test]
    fn a_path_the_old_catalog_holds_a_live_row_for_is_taken() {
        let files = [("docs/My_Notes.md", "# M\n")];
        let (renames, items) = run_held(&files, &["docs/my-notes.md"]);
        assert!(renames.is_empty());
        assert_eq!(reasons(&items), [("docs/My_Notes.md", "target exists")]);
        assert_eq!(
            items[0].detail.as_deref(),
            Some("would be docs/my-notes.md; the old catalog still holds a live row for it")
        );
        // Another path held changes nothing.
        let (renames, _) = run_held(&files, &["docs/other.md"]);
        assert_eq!(renames.len(), 1);
        // `plan_renames` is the same call with nothing held.
        assert_eq!(run(&files).0.len(), 1);
    }

    fn reasons(items: &[Item]) -> Vec<(&str, &str)> {
        items
            .iter()
            .map(|i| (i.path.as_deref().unwrap(), i.reason.as_str()))
            .collect()
    }

    #[test]
    fn canonical_stem_examples() {
        assert_eq!(canonical_stem("My_Notes").as_deref(), Some("my-notes"));
        assert_eq!(canonical_stem("a__b").as_deref(), Some("a-b"));
        assert_eq!(canonical_stem("--x--").as_deref(), Some("x"));
        assert_eq!(canonical_stem("日本"), None);
        assert_eq!(canonical_stem(""), None);
        assert_eq!(canonical_stem("CLAUDE").as_deref(), Some("claude"));
        assert_eq!(canonical_stem("v1.2 draft").as_deref(), Some("v1-2-draft"));
        assert_eq!(canonical_stem("a日本b").as_deref(), Some("a-b"));
        assert_eq!(
            canonical_stem("already-fine").as_deref(),
            Some("already-fine")
        );
    }

    #[test]
    fn a_non_canonical_file_is_renamed_in_its_folder() {
        let (renames, items) = run(&[("docs/My_Notes.md", "# M\n"), ("top_level.md", "# T\n")]);
        assert!(items.is_empty(), "{items:?}");
        assert_eq!(
            renames,
            [
                Rename {
                    from: "docs/My_Notes.md".into(),
                    to: "docs/my-notes.md".into(),
                    old_key: "my_notes".into(),
                    new_key: Key::parse("my-notes").unwrap(),
                },
                Rename {
                    from: "top_level.md".into(),
                    to: "top-level.md".into(),
                    old_key: "top_level".into(),
                    new_key: Key::parse("top-level").unwrap(),
                },
            ]
        );
    }

    #[test]
    fn claude_md_and_readme_md_are_not_renamed() {
        let (renames, items) = run(&[
            ("CLAUDE.md", "# C\n"),
            ("README.md", "# R\n"),
            ("docs/README.md", "# D\n"),
            ("docs/Ok.MD", "# O\n"),
        ]);
        assert!(renames.is_empty(), "{renames:?}");
        assert!(items.is_empty(), "{items:?}");
    }

    #[test]
    fn a_readme_in_a_non_canonical_folder_is_reported() {
        let (renames, items) = run(&[("Docs_X/README.md", "# R\n")]);
        assert!(renames.is_empty());
        assert_eq!(
            reasons(&items),
            [(
                "Docs_X/README.md",
                "folder index: rename the folder by hand"
            )]
        );
        assert_eq!(items[0].status, Status::Unrecoverable);
        assert_eq!(items[0].class, Class::Rename);
    }

    #[test]
    fn a_stem_with_nothing_left_is_reported() {
        let (renames, items) = run(&[("docs/日本.md", "# J\n"), ("docs/__.md", "# U\n")]);
        assert!(renames.is_empty());
        assert_eq!(
            reasons(&items),
            [
                ("docs/__.md", "no canonical name"),
                ("docs/日本.md", "no canonical name")
            ]
        );
    }

    #[test]
    fn a_hex_shaped_result_is_reported() {
        let (renames, items) = run(&[
            ("docs/0123456789abcdef_.md", "# H\n"),
            ("docs/0123456789ABCDEF.md", "# H\n"),
        ]);
        assert!(renames.is_empty());
        assert_eq!(
            reasons(&items),
            [
                ("docs/0123456789ABCDEF.md", "hex-shaped name"),
                ("docs/0123456789abcdef_.md", "hex-shaped name")
            ]
        );
    }

    #[test]
    fn a_taken_target_path_is_reported() {
        let (renames, items) = run(&[("docs/My_Notes.md", "# M\n"), ("docs/my-notes.md", "# N\n")]);
        assert!(renames.is_empty());
        assert_eq!(reasons(&items), [("docs/My_Notes.md", "target exists")]);
    }

    #[test]
    fn two_files_that_would_share_a_key_are_both_reported() {
        let (renames, items) = run(&[
            ("src/a/render_prompt.md", "# A\n"),
            ("src/b/render_prompt.md", "# B\n"),
        ]);
        assert!(renames.is_empty(), "{renames:?}");
        assert_eq!(
            reasons(&items),
            [
                ("src/a/render_prompt.md", "key collision"),
                ("src/b/render_prompt.md", "key collision")
            ]
        );
        assert_eq!(
            items[0].suggestion.as_deref(),
            Some("src/a/a-render-prompt.md")
        );
        assert_eq!(
            items[1].suggestion.as_deref(),
            Some("src/b/b-render-prompt.md")
        );
        assert!(
            items[0]
                .detail
                .as_deref()
                .unwrap()
                .contains("src/b/render_prompt.md"),
            "{:?}",
            items[0].detail
        );
    }

    #[test]
    fn a_rename_onto_a_key_another_folder_holds_is_reported() {
        let (renames, items) = run(&[
            ("docs/My_Notes.md", "# M\n"),
            ("notes/my-notes.md", "# N\n"),
        ]);
        assert!(renames.is_empty());
        assert_eq!(reasons(&items), [("docs/My_Notes.md", "key collision")]);
        assert_eq!(
            items[0].suggestion.as_deref(),
            Some("docs/docs-my-notes.md")
        );
    }

    #[test]
    fn a_file_code_reads_is_frozen() {
        let (renames, items) = run(&[
            ("src/render_prompt.md", "# P\n"),
            (
                "src/lib.rs",
                "const P: &str = include_str!(\"./render_prompt.md\");\n",
            ),
        ]);
        assert!(renames.is_empty());
        assert_eq!(items.len(), 1, "{items:?}");
        assert!(
            items[0].reason.starts_with("frozen: "),
            "{}",
            items[0].reason
        );
        assert!(
            items[0].reason.contains("src/lib.rs:1"),
            "{}",
            items[0].reason
        );
    }

    #[test]
    fn a_file_name_anywhere_in_code_blocks_its_rename() {
        let (renames, items) = run(&[
            ("scripts/m/judge_prompt.md", "# J\n"),
            ("scripts/x.py", "print(open('judge_prompt.md'))\n"),
        ]);
        assert!(renames.is_empty());
        assert_eq!(
            reasons(&items),
            [("scripts/m/judge_prompt.md", "named by code")],
            "blocked by the name, though the file is not frozen"
        );
        assert_eq!(
            items[0].detail.as_deref(),
            Some("scripts/x.py:1; would be scripts/m/judge-prompt.md")
        );
    }

    #[test]
    fn many_places_that_name_a_file_are_shortened_to_the_first_and_a_count() {
        let (_, items) = run(&[
            ("docs/My_Notes.md", "# M\n"),
            ("scripts/b.sh", "cat My_Notes.md\ncat My_Notes.md\n"),
            ("scripts/a.sh", "cat My_Notes.md\n"),
        ]);
        assert_eq!(
            items[0].detail.as_deref(),
            Some("scripts/a.sh:1 and 2 more; would be docs/my-notes.md")
        );
    }

    #[test]
    fn a_file_name_in_markdown_does_not_block_its_rename() {
        let (renames, items) = run(&[
            ("docs/My_Notes.md", "# M\n"),
            ("docs/b.md", "see My_Notes.md and docs/My_Notes.md\n"),
        ]);
        assert!(items.is_empty(), "{items:?}");
        assert_eq!(renames.len(), 1);
    }

    #[test]
    fn a_dot_directory_file_is_not_renamed() {
        let (renames, items) = run(&[(".github/ISSUE_TEMPLATE/Bug_Report.md", "# B\n")]);
        assert!(renames.is_empty());
        assert_eq!(items.len(), 1, "{items:?}");
        assert!(
            items[0].reason.starts_with("frozen: "),
            "{}",
            items[0].reason
        );
        assert!(items[0].reason.contains(".github"), "{}", items[0].reason);
    }

    #[test]
    fn a_key_collision_in_head_is_reported_with_a_suggestion() {
        let (renames, items) = run(&[
            ("src/a/render.md", "# A\n"),
            ("src/b/render.md", "# B\n"),
            ("docs/fine.md", "# F\n"),
        ]);
        assert!(renames.is_empty());
        assert_eq!(
            reasons(&items),
            [
                ("src/a/render.md", "key collision"),
                ("src/b/render.md", "key collision")
            ]
        );
        assert_eq!(items[0].suggestion.as_deref(), Some("src/a/a-render.md"));
        assert_eq!(items[1].suggestion.as_deref(), Some("src/b/b-render.md"));
        assert_eq!(
            items[0].detail.as_deref(),
            Some("the key render is also held by src/b/render.md")
        );
    }

    #[test]
    fn a_suggestion_is_withheld_when_it_would_collide_too() {
        let (_, items) = run(&[
            ("src/a/render.md", "# A\n"),
            ("src/b/render.md", "# B\n"),
            ("docs/a-render.md", "# taken\n"),
        ]);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].suggestion, None, "a-render is a key HEAD holds");
        assert_eq!(items[1].suggestion.as_deref(), Some("src/b/b-render.md"));
        // A file at the repository root has no parent folder to name it after.
        let (_, items) = run(&[("render.md", "# A\n"), ("src/render.md", "# B\n")]);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].suggestion, None);
    }

    #[test]
    fn a_blocked_file_does_not_hold_the_key_against_a_free_one() {
        // The dot-directory file is frozen and stays as it is; `scripts/b` can still take the key.
        let (renames, items) = run(&[
            ("scripts/.cfg/x_y.md", "# A\n"),
            ("scripts/b/x_y.md", "# B\n"),
        ]);
        assert_eq!(renames.len(), 1, "{renames:?}");
        assert_eq!(renames[0].from, "scripts/b/x_y.md");
        assert_eq!(items.len(), 1, "{items:?}");
        assert_eq!(items[0].path.as_deref(), Some("scripts/.cfg/x_y.md"));
        assert!(
            items[0].reason.starts_with("frozen: "),
            "{}",
            items[0].reason
        );
    }

    fn old_rows(docs: &[(&str, bool)]) -> OldRows {
        let mut rows = OldRows::default();
        for (n, (rel_path, missing)) in docs.iter().enumerate() {
            rows.documents.insert(
                format!("id{n}"),
                OldDocument {
                    id: format!("id{n}"),
                    rel_path: (*rel_path).to_owned(),
                    missing: *missing,
                },
            );
        }
        rows
    }

    fn head(paths: &[&str]) -> BTreeSet<String> {
        paths.iter().map(|p| (*p).to_owned()).collect()
    }

    #[test]
    fn renames_already_in_head_map_the_old_path() {
        let old = old_rows(&[("docs/My_Notes.md", false), ("docs/b.md", false)]);
        let found = renames_already_in_head(&old, &head(&["docs/my-notes.md", "docs/b.md"]));
        assert_eq!(
            found,
            [Rename {
                from: "docs/My_Notes.md".into(),
                to: "docs/my-notes.md".into(),
                old_key: "my_notes".into(),
                new_key: Key::parse("my-notes").unwrap(),
            }]
        );
    }

    #[test]
    fn a_canonical_sibling_the_old_catalog_already_held_is_not_a_rename() {
        // Both files had rows: the second existed before the cutover, so it is not the first renamed.
        let old = old_rows(&[("docs/My_Notes.md", false), ("docs/my-notes.md", false)]);
        let found = renames_already_in_head(&old, &head(&["docs/my-notes.md"]));
        assert!(found.is_empty(), "{found:?}");
        // A row the catalog marked missing is no live row.
        let old = old_rows(&[("docs/My_Notes.md", false), ("docs/my-notes.md", true)]);
        let found = renames_already_in_head(&old, &head(&["docs/my-notes.md"]));
        assert_eq!(found.len(), 1, "{found:?}");
    }

    #[test]
    fn a_path_head_still_holds_or_that_needs_no_rename_is_not_a_rename() {
        // HEAD holds the old path: the rename has not happened.
        let old = old_rows(&[("docs/My_Notes.md", false)]);
        let found = renames_already_in_head(&old, &head(&["docs/My_Notes.md", "docs/my-notes.md"]));
        assert!(found.is_empty(), "{found:?}");
        // A deleted file with a valid key has no sibling to map to, and a folder index never does.
        let old = old_rows(&[("docs/gone.md", false), ("Docs_X/README.md", false)]);
        let found = renames_already_in_head(&old, &head(&["docs/gone-2.md", "Docs_X/readme.md"]));
        assert!(found.is_empty(), "{found:?}");
        // A deleted file with a valid key needs no rename: its lowercase sibling is another file.
        let old = old_rows(&[("docs/Claude.md", false)]);
        let found = renames_already_in_head(&old, &head(&["docs/claude.md"]));
        assert!(found.is_empty(), "{found:?}");
        // Nothing of HEAD at the sibling: the file is just gone.
        let old = old_rows(&[("docs/My_Notes.md", false)]);
        assert!(renames_already_in_head(&old, &head(&["docs/b.md"])).is_empty());
    }

    #[test]
    fn two_old_paths_for_one_sibling_map_neither() {
        let old = old_rows(&[("docs/My_Notes.md", false), ("docs/my_notes.md", false)]);
        let found = renames_already_in_head(&old, &head(&["docs/my-notes.md"]));
        assert!(found.is_empty(), "{found:?}");
    }

    #[test]
    fn the_set_maps_paths_and_keys_for_both_kinds() {
        let rename = |from: &str, to: &str, old: &str, new: &str| Rename {
            from: from.into(),
            to: to.into(),
            old_key: old.into(),
            new_key: Key::parse(new).unwrap(),
        };
        let set = RenameSet {
            planned: vec![rename("a/X_Y.md", "a/x-y.md", "x_y", "x-y")],
            in_head: vec![rename(
                "b/My_Notes.md",
                "b/my-notes.md",
                "my_notes",
                "my-notes",
            )],
        };
        let paths = set.paths();
        assert_eq!(paths["a/X_Y.md"], "a/x-y.md");
        assert_eq!(paths["b/My_Notes.md"], "b/my-notes.md");
        let keys = set.keys();
        assert_eq!(keys["x_y"].as_str(), "x-y");
        assert_eq!(keys["my_notes"].as_str(), "my-notes");
    }
}
