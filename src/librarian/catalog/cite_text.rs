//! A resolved cite as grammar text: `[[key]]`, `[[key:TOKEN]]`, `[[repo/key]]`, `[[repo/key:TOKEN]]`.
//!
//! The old catalog resolves a cite to an artifact id or a `<slug>:<local>` entry id
//! (`resolve_cite_ref`). The committed markdown carries the same cite in the citation grammar,
//! on one `**Cites:**` line at the end of the entry's section. This module renders that text and
//! refuses a cite the grammar cannot name, so a cite is never written to the catalog while its
//! text twin is missing or wrong.

use crate::librarian::catalog::augmentation::PendingSection;
use crate::librarian::current_project::lookup_git_root;
use crate::librarian::derived::diff::{scope_of, Scope};
use crate::librarian::identity::gate::{check_tree, GateProblem};
use crate::librarian::identity::grammar::{parse_target, Reference};
use crate::librarian::identity::key::{key_of, KeyError};
use crate::librarian::identity::key_space::{self, KEY_SPACE_FILE_REL};
use crate::librarian::identity::repo_name::{self, RepoName, REPO_FILE_REL};
use crate::librarian::tools::LibrarianRecoverableError;
use crate::librarian::util::{path_at_or_under_sql, root_and_prefix};
use anyhow::Result;
use rusqlite::OptionalExtension;
use std::path::Path;

/// The label of the line that holds an entry's citations.
#[cfg_attr(not(test), allow(dead_code))] // removed by Task 7
pub(crate) const CITES_LABEL: &str = "**Cites:**";

/// The grammar text of a resolved cite: `[[key]]` for a document, `[[key:TOKEN]]` for an entry of
/// one, each prefixed `repo/` when the target lies in another repository.
///
/// `dst_ref` is what `resolve_cite_ref` returns: `<slug>:<local>` for an entry, else an artifact
/// id. The target's repository is the git root above its file. A cite is refused, with a text that
/// names the target by its file name and never by an absolute path, a row id or a 16-hex id, when
/// the target
///
/// - is not in the catalog, or lies in no git repository, or lies under a `.worktrees/<name>/`
///   directory that holds no `.git` (a linked worktree has a `.git` file, so it is its own git
///   root and is not refused);
/// - lies outside the key space, or has no valid key, or shares its key with another document of
///   its repository (judged by [`check_tree`] over the catalog's rows of that repository, so a
///   worktree shadow, a `//` spelling of one file and a row marked missing are not collisions);
/// - lies in another repository that has no committed name, so no `[[repo/key]]` form exists;
/// - yields text the grammar does not read as a file or an entry, such as a token with four
///   letters.
///
/// Two git roots are one repository when they are the same root, or a linked worktree and its
/// main checkout, or two linked worktrees of one main checkout, the main checkout being named by
/// an absolute pointer (see [`repository_root`]). Anything else is its own repository, a
/// submodule and a worktree with a relative pointer included; it falls to the committed-name
/// path, where equal committed names (another clone of it) make the roots one repository too. The
/// target's documents are then cited unqualified, whether or not either root has a committed
/// name.
///
/// `source_git_root` is the git root of the document that carries the cite: callers pass
/// `CurrentProject.git_root` exactly as stored (canonical). Roots are compared as paths by
/// component, so a trailing or doubled slash does not matter; only a symlinked spelling of one
/// root differs.
///
/// Accepted cost: the key check reads the catalog's rows, not the repository's commit tree. A
/// catalog row for a file the tree lacks (ignored build output, files of a nested repository) can
/// make a key look shared, so a cite can be refused for a collision the tree does not have.
/// A document the catalog has not indexed yet is not seen at all.
#[cfg_attr(not(test), allow(dead_code))] // removed by Task 7
pub(crate) fn grammar_form_of(
    conn: &rusqlite::Connection,
    source_git_root: &Path,
    dst_ref: &str,
) -> Result<String> {
    let (abs_path, token) = target_of(conn, dst_ref)?;
    let name = file_name_of(&abs_path);
    let Some(target_root) = lookup_git_root(Path::new(&abs_path)) else {
        return Err(refused(format!(
            "`{name}` lies in no git repository, so it has no key to cite"
        )));
    };
    let rel = match scope_of(&target_root, &abs_path) {
        Scope::Inside(rel) => rel,
        Scope::Worktree => {
            return Err(refused(format!(
                "`{name}` is a copy under .worktrees/; cite the document in the main checkout"
            )))
        }
        Scope::OtherRoot => {
            return Err(refused(format!(
                "`{name}` cannot be named as a file of its repository"
            )))
        }
    };

    let space = key_space::load(&target_root).map_err(|e| unreadable(e, &target_root))?;
    if space.is_outside(&rel) {
        return Err(refused(format!(
            "`{name}` lies outside the key space ({KEY_SPACE_FILE_REL}), so it has no key to cite"
        )));
    }
    let committed = repo_name::read(&target_root).map_err(|e| unreadable(e, &target_root))?;
    let gate_name = match &committed {
        Some(name) => name.clone(),
        None => RepoName::parse("repo")?,
    };
    let population = population_of(conn, &target_root, &rel)?;
    for problem in check_tree(gate_name, &space, &population).problems {
        match &problem {
            GateProblem::InvalidKey(e) if invalid_rel(e) == rel => {
                return Err(refused(format!(
                    "`{name}` cannot be cited: {}",
                    problem.render()
                )));
            }
            GateProblem::DuplicateKey { rel_paths, .. } if rel_paths.contains(&rel) => {
                return Err(refused(format!(
                    "`{name}` cannot be cited: {}",
                    problem.render()
                )));
            }
            _ => {}
        }
    }
    let key = key_of(&rel).map_err(|_| {
        anyhow::anyhow!("internal: the key check passed a document that has no key")
    })?;

    let qualifier = if repository_root(&target_root) == repository_root(source_git_root) {
        None
    } else {
        let Some(target_name) = committed else {
            return Err(refused(format!(
                "`{name}` lies in another repository that has no committed name, so it cannot \
                 be cited as `[[repo/key]]`; commit {REPO_FILE_REL} there (name = \"...\")"
            )));
        };
        let source_name =
            repo_name::read(source_git_root).map_err(|e| unreadable(e, source_git_root))?;
        (source_name.as_ref() != Some(&target_name)).then_some(target_name)
    };

    let mut text = String::new();
    if let Some(qualifier) = &qualifier {
        text.push_str(qualifier.as_str());
        text.push('/');
    }
    text.push_str(key.as_str());
    if let Some(token) = token {
        text.push(':');
        text.push_str(token);
    }
    match parse_target(&text) {
        Reference::File { .. } | Reference::Entry { .. } => Ok(format!("[[{text}]]")),
        Reference::Bare { .. } | Reference::Malformed { .. } => Err(refused(match token {
            Some(token) => format!(
                "the entry id `{token}` is not in the citation grammar (one to three capital \
                 letters, a hyphen, digits), so `{name}` cannot be cited with it"
            ),
            None => format!("`{name}` has no citation form in the grammar"),
        })),
    }
}

/// A refusal of a cite: an input-driven failure the caller can correct.
fn refused(message: String) -> anyhow::Error {
    LibrarianRecoverableError::new(format!("cite refused: {message}"))
}

/// A refusal for a configuration file of `root` that cannot be read: the lower layer's text with
/// the repository's path taken out.
fn unreadable(e: anyhow::Error, root: &Path) -> anyhow::Error {
    let root = root.display().to_string();
    let text = format!("{e:#}")
        .replace(&format!("{root}/"), "")
        .replace(&root, "the repository");
    refused(text)
}

fn file_name_of(abs_path: &str) -> String {
    Path::new(abs_path).file_name().map_or_else(
        || "the document".to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}

/// The main checkout a git root belongs to, for the comparison that decides whether two git roots
/// are one repository: the root itself, except for a linked worktree.
///
/// A linked worktree is a root whose `.git` is a pointer file naming `<main>/.git/worktrees/<name>`
/// (`is_linked_worktree` requires the `worktrees` component, so a submodule's `.git/modules/...`
/// pointer is not one). `lookup_git_root` stops at any `.git`, so a worktree session's git root is
/// the worktree while the catalog can hold the main checkout's row for the same document. The main
/// root is used only when it is absolute and holds a `.git` directory: a relative pointer cannot
/// be told from another repository's (`../..` is the same text everywhere), and a pointer into a
/// directory that is no repository names nothing. In both cases, and for a submodule, the root is
/// its own repository.
fn repository_root(git_root: &Path) -> std::path::PathBuf {
    use crate::util::path_security::{is_linked_worktree, worktree_main_root};

    if !is_linked_worktree(git_root) {
        return git_root.to_path_buf();
    }
    match worktree_main_root(git_root) {
        Some(main)
            if main.is_absolute()
                && std::fs::symlink_metadata(main.join(".git"))
                    .is_ok_and(|meta| meta.file_type().is_dir()) =>
        {
            main
        }
        _ => git_root.to_path_buf(),
    }
}

/// The path of the artifact `dst_ref` names, and the entry id when it names an entry.
fn target_of<'a>(
    conn: &rusqlite::Connection,
    dst_ref: &'a str,
) -> Result<(String, Option<&'a str>)> {
    let (column, value, token) = match dst_ref.split_once(':') {
        Some((slug, local)) => ("slug", slug, Some(local)),
        None => ("id", dst_ref, None),
    };
    let abs_path: Option<String> = conn
        .query_row(
            &format!("SELECT abs_path FROM artifact WHERE {column} = ?1"),
            [value],
            |r| r.get(0),
        )
        .optional()?;
    match abs_path {
        Some(abs_path) => Ok((abs_path, token)),
        None => Err(refused(
            "the cited document is not in the catalog".to_string(),
        )),
    }
}

/// The repository-relative paths of the documents the catalog holds under `target_root`, the
/// way the shadow diff reads them: a row marked missing, a `.worktrees/` copy and a path that is
/// not a plain path below the root are dropped, and `a//b` is `a/b`. `target_rel` is always in.
fn population_of(
    conn: &rusqlite::Connection,
    target_root: &Path,
    target_rel: &str,
) -> Result<Vec<String>> {
    let (root, prefix) = root_and_prefix(&target_root.to_string_lossy());
    let mut stmt = conn.prepare(&format!(
        "SELECT abs_path FROM artifact WHERE missing_since IS NULL AND {}",
        path_at_or_under_sql("abs_path", "?1", "?2")
    ))?;
    let rows = stmt.query_map(rusqlite::params![root, prefix], |r| r.get::<_, String>(0))?;
    let mut rels = vec![target_rel.to_string()];
    for abs_path in rows {
        if let Scope::Inside(rel) = scope_of(target_root, &abs_path?) {
            rels.push(rel);
        }
    }
    Ok(rels)
}

/// The path a key error was raised for.
fn invalid_rel(e: &KeyError) -> &str {
    match e {
        KeyError::NotMarkdown(path) => path,
        KeyError::NonCanonical { rel_path, .. } | KeyError::HexShaped { rel_path, .. } => rel_path,
    }
}

/// The one line that holds an entry's citations: `**Cites:** [[a]], [[b:F-3]]`, in input order,
/// an exact repeat dropped. No forms give an empty string: a label with nothing after it is not a
/// citation line.
#[cfg_attr(not(test), allow(dead_code))] // removed by Task 7
pub(crate) fn cites_line(forms: &[String]) -> String {
    let mut seen = std::collections::HashSet::new();
    let kept: Vec<&str> = forms
        .iter()
        .map(String::as_str)
        .filter(|form| seen.insert(*form))
        .collect();
    if kept.is_empty() {
        String::new()
    } else {
        format!("{CITES_LABEL} {}", kept.join(", "))
    }
}

/// `section` with its citations as one labelled line at the end of its body, after a blank line;
/// a plain clone when there are none.
#[cfg_attr(not(test), allow(dead_code))] // removed by Task 7
pub(crate) fn with_cites(section: &PendingSection, forms: &[String]) -> PendingSection {
    if forms.is_empty() {
        return section.clone();
    }
    PendingSection {
        body: format!("{}\n\n{}", section.body.trim_end(), cites_line(forms)),
        ..section.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::artifact::{upsert as art_upsert, TestArtifactRowBuilder};
    use crate::librarian::catalog::Catalog;
    use std::path::{Path, PathBuf};

    /// A directory that is a git root (it holds `.git/`), under `base`.
    fn git_root(base: &Path, name: &str) -> PathBuf {
        let root = base.join(name);
        std::fs::create_dir_all(root.join(".git")).unwrap();
        root
    }

    /// `.codescout/<file>` under `root` holding `text`.
    fn config(root: &Path, file: &str, text: &str) {
        let dir = root.join(".codescout");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(file), text).unwrap();
    }

    fn name_repo(root: &Path, name: &str) {
        config(root, "repo.toml", &format!("name = \"{name}\"\n"));
    }

    /// One artifact row at `abs_path`; `slug` is set when given.
    fn row(cat: &Catalog, id: &str, abs_path: &str, slug: Option<&str>) {
        art_upsert(
            cat,
            &TestArtifactRowBuilder::new(id)
                .with_abs_path(abs_path)
                .build(),
        )
        .unwrap();
        if let Some(slug) = slug {
            cat.conn
                .execute(
                    "UPDATE artifact SET slug = ?1 WHERE id = ?2",
                    rusqlite::params![slug, id],
                )
                .unwrap();
        }
    }

    fn at(root: &Path, rel: &str) -> String {
        format!("{}/{rel}", root.display())
    }

    fn refusal(r: anyhow::Result<String>) -> String {
        format!("{:#}", r.expect_err("the cite must be refused"))
    }

    #[test]
    fn a_file_ref_becomes_a_key_and_an_entry_ref_a_key_with_its_token() {
        let tmp = tempfile::tempdir().unwrap();
        let root = git_root(tmp.path(), "mine");
        let cat = Catalog::open_in_memory().unwrap();
        row(
            &cat,
            "id-target",
            &at(&root, "docs/target.md"),
            Some("target-slug"),
        );
        row(
            &cat,
            "id-area-readme",
            &at(&root, "docs/area/readme.md"),
            None,
        );

        assert_eq!(
            grammar_form_of(&cat.conn, &root, "id-target").unwrap(),
            "[[target]]"
        );
        assert_eq!(
            grammar_form_of(&cat.conn, &root, "target-slug:F-3").unwrap(),
            "[[target:F-3]]"
        );
        // A folder index is cited by its folder's name.
        assert_eq!(
            grammar_form_of(&cat.conn, &root, "id-area-readme").unwrap(),
            "[[area]]"
        );
    }

    #[test]
    fn a_non_canonical_target_name_is_refused_by_name() {
        let tmp = tempfile::tempdir().unwrap();
        let root = git_root(tmp.path(), "mine");
        let cat = Catalog::open_in_memory().unwrap();
        row(&cat, "id-bad", &at(&root, "docs/Bad_Name.md"), None);

        let msg = refusal(grammar_form_of(&cat.conn, &root, "id-bad"));
        assert!(msg.contains("Bad_Name.md"), "{msg}");
        assert!(msg.contains("^[a-z0-9]+(-[a-z0-9]+)*$"), "{msg}");
        assert!(!msg.contains(&root.display().to_string()), "{msg}");
    }

    #[test]
    fn a_target_outside_the_key_space_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let root = git_root(tmp.path(), "mine");
        config(&root, "key-space.toml", "exclude = [\"contrib/**\"]\n");
        let cat = Catalog::open_in_memory().unwrap();
        row(&cat, "id-x", &at(&root, "contrib/x.md"), None);

        let msg = refusal(grammar_form_of(&cat.conn, &root, "id-x"));
        assert!(
            msg.contains("x.md") && msg.contains("key-space.toml"),
            "{msg}"
        );
        assert!(!msg.contains(&root.display().to_string()), "{msg}");
    }

    #[test]
    fn a_shared_key_is_refused_naming_both_files() {
        let tmp = tempfile::tempdir().unwrap();
        let root = git_root(tmp.path(), "mine");
        let cat = Catalog::open_in_memory().unwrap();
        row(&cat, "id-a-notes", &at(&root, "docs/a/notes.md"), None);
        row(&cat, "id-b-notes", &at(&root, "docs/b/notes.md"), None);

        let msg = format!(
            "{:#}",
            grammar_form_of(&cat.conn, &root, "id-a-notes").unwrap_err()
        );
        assert!(
            msg.contains("docs/a/notes.md") && msg.contains("docs/b/notes.md"),
            "{msg}"
        );
        assert!(!msg.contains(&root.display().to_string()), "{msg}");
    }

    #[test]
    fn worktree_shadows_and_double_slash_rows_are_not_collisions() {
        let tmp = tempfile::tempdir().unwrap();
        let root = git_root(tmp.path(), "mine");
        let cat = Catalog::open_in_memory().unwrap();
        row(&cat, "id-t", &at(&root, "docs/t.md"), None);
        row(
            &cat,
            "id-shadow",
            &at(&root, ".worktrees/feat/docs/t.md"),
            None,
        );
        row(
            &cat,
            "id-slashes",
            &format!("{}//docs/t.md", root.display()),
            None,
        );
        row(&cat, "id-gone", &at(&root, "docs/old/t.md"), None);
        cat.conn
            .execute(
                "UPDATE artifact SET missing_since = 1 WHERE id = 'id-gone'",
                [],
            )
            .unwrap();

        assert_eq!(grammar_form_of(&cat.conn, &root, "id-t").unwrap(), "[[t]]");
    }

    #[test]
    fn a_foreign_target_is_qualified_by_its_committed_repo_name() {
        let tmp = tempfile::tempdir().unwrap();
        let mine = git_root(tmp.path(), "mine");
        let other = git_root(tmp.path(), "other");
        name_repo(&other, "other-repo");
        let cat = Catalog::open_in_memory().unwrap();
        row(
            &cat,
            "id-notes",
            &at(&other, "docs/notes.md"),
            Some("notes-slug"),
        );

        assert_eq!(
            grammar_form_of(&cat.conn, &mine, "id-notes").unwrap(),
            "[[other-repo/notes]]"
        );
        assert_eq!(
            grammar_form_of(&cat.conn, &mine, "notes-slug:F-3").unwrap(),
            "[[other-repo/notes:F-3]]"
        );
    }

    #[test]
    fn a_foreign_target_with_no_committed_name_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let mine = git_root(tmp.path(), "mine");
        let other = git_root(tmp.path(), "other");
        let cat = Catalog::open_in_memory().unwrap();
        row(&cat, "id-notes", &at(&other, "docs/notes.md"), None);

        let msg = refusal(grammar_form_of(&cat.conn, &mine, "id-notes"));
        assert!(msg.contains("repo.toml"), "{msg}");
        assert!(!msg.contains(&other.display().to_string()), "{msg}");
    }

    #[test]
    fn another_checkout_of_the_same_repository_stays_unqualified() {
        let tmp = tempfile::tempdir().unwrap();
        let mine = git_root(tmp.path(), "mine");
        let twin = git_root(tmp.path(), "twin");
        name_repo(&mine, "same");
        name_repo(&twin, "same");
        let cat = Catalog::open_in_memory().unwrap();
        row(&cat, "id-t", &at(&twin, "docs/t.md"), None);

        assert_eq!(grammar_form_of(&cat.conn, &mine, "id-t").unwrap(), "[[t]]");
    }

    #[test]
    fn a_token_outside_the_grammar_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let root = git_root(tmp.path(), "mine");
        let cat = Catalog::open_in_memory().unwrap();
        row(
            &cat,
            "id-target",
            &at(&root, "docs/target.md"),
            Some("target-slug"),
        );

        let msg = refusal(grammar_form_of(&cat.conn, &root, "target-slug:ABCD-1"));
        assert!(msg.contains("ABCD-1"), "{msg}");
    }

    #[test]
    fn an_unknown_or_rootless_target_is_refused_without_naming_the_reference() {
        let tmp = tempfile::tempdir().unwrap();
        let root = git_root(tmp.path(), "mine");
        let cat = Catalog::open_in_memory().unwrap();
        row(
            &cat,
            "0123456789abcdef",
            "/nonexistent-codescout-test-root/docs/x.md",
            None,
        );

        let unknown = refusal(grammar_form_of(&cat.conn, &root, "fedcba9876543210"));
        assert!(!unknown.contains("fedcba9876543210"), "{unknown}");
        let rootless = refusal(grammar_form_of(&cat.conn, &root, "0123456789abcdef"));
        assert!(rootless.contains("git"), "{rootless}");
        assert!(!rootless.contains("0123456789abcdef"), "{rootless}");
        assert!(
            !rootless.contains("/nonexistent-codescout-test-root"),
            "{rootless}"
        );
    }

    /// A linked worktree of `main`: a directory whose `.git` is a pointer file into `main`.
    fn linked_worktree(base: &Path, main: &Path, name: &str) -> PathBuf {
        let worktree = base.join(name);
        std::fs::create_dir_all(&worktree).unwrap();
        std::fs::write(
            worktree.join(".git"),
            format!("gitdir: {}/.git/worktrees/{name}\n", main.display()),
        )
        .unwrap();
        worktree
    }

    #[test]
    fn a_cite_from_a_worktree_to_its_main_checkout_needs_no_committed_name() {
        let tmp = tempfile::tempdir().unwrap();
        let main = git_root(tmp.path(), "main");
        let worktree = linked_worktree(tmp.path(), &main, "wt");
        let cat = Catalog::open_in_memory().unwrap();
        row(&cat, "id-t", &at(&main, "docs/t.md"), Some("t-slug"));

        // Neither checkout holds a repo.toml: one repository, so the plain key is right.
        assert_eq!(
            grammar_form_of(&cat.conn, &worktree, "id-t").unwrap(),
            "[[t]]"
        );
        assert_eq!(
            grammar_form_of(&cat.conn, &worktree, "t-slug:F-3").unwrap(),
            "[[t:F-3]]"
        );
    }

    #[test]
    fn a_cite_from_the_main_checkout_to_one_of_its_worktrees_needs_no_committed_name() {
        let tmp = tempfile::tempdir().unwrap();
        let main = git_root(tmp.path(), "main");
        let worktree = linked_worktree(tmp.path(), &main, "wt");
        let cat = Catalog::open_in_memory().unwrap();
        row(&cat, "id-t", &at(&worktree, "docs/t.md"), None);

        assert_eq!(grammar_form_of(&cat.conn, &main, "id-t").unwrap(), "[[t]]");
    }

    #[test]
    fn a_worktree_of_another_repository_is_still_another_repository() {
        let tmp = tempfile::tempdir().unwrap();
        let mine = git_root(tmp.path(), "mine");
        let other = git_root(tmp.path(), "other");
        let other_worktree = linked_worktree(tmp.path(), &other, "other-wt");
        let cat = Catalog::open_in_memory().unwrap();
        row(&cat, "id-t", &at(&other_worktree, "docs/t.md"), None);

        let msg = refusal(grammar_form_of(&cat.conn, &mine, "id-t"));
        assert!(msg.contains("repo.toml"), "{msg}");
        assert!(!msg.contains(&other.display().to_string()), "{msg}");
    }

    #[test]
    fn a_folder_index_entry_ref_is_cited_by_its_folder_with_the_token() {
        let tmp = tempfile::tempdir().unwrap();
        let root = git_root(tmp.path(), "mine");
        let cat = Catalog::open_in_memory().unwrap();
        row(
            &cat,
            "id-area-readme",
            &at(&root, "docs/area/readme.md"),
            Some("area-slug"),
        );

        assert_eq!(
            grammar_form_of(&cat.conn, &root, "area-slug:F-3").unwrap(),
            "[[area:F-3]]"
        );
    }

    /// A directory under `base` whose `.git` is a pointer file holding `gitdir: <pointer>`.
    fn pointer_root(base: &Path, rel: &str, pointer: &str) -> PathBuf {
        let root = base.join(rel);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join(".git"), format!("gitdir: {pointer}\n")).unwrap();
        root
    }

    #[test]
    fn two_worktrees_of_one_main_checkout_are_one_repository() {
        let tmp = tempfile::tempdir().unwrap();
        let main = git_root(tmp.path(), "main");
        let first = linked_worktree(tmp.path(), &main, "wt-one");
        let second = linked_worktree(tmp.path(), &main, "wt-two");
        let cat = Catalog::open_in_memory().unwrap();
        row(&cat, "id-t", &at(&second, "docs/t.md"), None);

        assert_eq!(grammar_form_of(&cat.conn, &first, "id-t").unwrap(), "[[t]]");
    }

    #[test]
    fn two_sibling_submodules_are_two_repositories() {
        let tmp = tempfile::tempdir().unwrap();
        let _superproject = git_root(tmp.path(), "super");
        let sub_a = pointer_root(&tmp.path().join("super"), "a", "../.git/modules/a");
        let sub_b = pointer_root(&tmp.path().join("super"), "b", "../.git/modules/b");
        let cat = Catalog::open_in_memory().unwrap();
        row(&cat, "id-t", &at(&sub_a, "docs/t.md"), None);

        let msg = refusal(grammar_form_of(&cat.conn, &sub_b, "id-t"));
        assert!(msg.contains("repo.toml"), "{msg}");
    }

    #[test]
    fn a_superproject_and_its_submodule_are_two_repositories() {
        let tmp = tempfile::tempdir().unwrap();
        let superproject = git_root(tmp.path(), "super");
        let absolute = format!("{}/.git/modules/sub", superproject.display());
        let sub = pointer_root(&superproject, "sub", &absolute);
        let cat = Catalog::open_in_memory().unwrap();
        row(&cat, "id-t", &at(&sub, "docs/t.md"), None);
        row(&cat, "id-s", &at(&superproject, "docs/s.md"), None);

        let into_submodule = refusal(grammar_form_of(&cat.conn, &superproject, "id-t"));
        assert!(into_submodule.contains("repo.toml"), "{into_submodule}");
        let out_of_submodule = refusal(grammar_form_of(&cat.conn, &sub, "id-s"));
        assert!(out_of_submodule.contains("repo.toml"), "{out_of_submodule}");
    }

    #[test]
    fn worktrees_with_relative_pointers_of_different_repositories_are_two_repositories() {
        let tmp = tempfile::tempdir().unwrap();
        let _a = git_root(tmp.path(), "a");
        let _b = git_root(tmp.path(), "b");
        let pointer = "../../.git/worktrees/x";
        let wt_a = pointer_root(&tmp.path().join("a"), ".worktrees/x", pointer);
        let wt_b = pointer_root(&tmp.path().join("b"), ".worktrees/x", pointer);
        let cat = Catalog::open_in_memory().unwrap();
        row(&cat, "id-t", &at(&wt_b, "docs/t.md"), None);

        let msg = refusal(grammar_form_of(&cat.conn, &wt_a, "id-t"));
        assert!(msg.contains("repo.toml"), "{msg}");
    }

    #[test]
    fn worktrees_pointing_into_a_main_with_no_git_directory_are_not_one_repository() {
        let tmp = tempfile::tempdir().unwrap();
        // `ghost` holds no `.git` directory: the pointers name a repository that is not there.
        let ghost = tmp.path().join("ghost");
        let first = linked_worktree(tmp.path(), &ghost, "wt-one");
        let second = linked_worktree(tmp.path(), &ghost, "wt-two");
        let cat = Catalog::open_in_memory().unwrap();
        row(&cat, "id-t", &at(&second, "docs/t.md"), None);

        let msg = refusal(grammar_form_of(&cat.conn, &first, "id-t"));
        assert!(msg.contains("repo.toml"), "{msg}");
    }

    #[test]
    fn cites_line_of_nothing_is_empty() {
        assert_eq!(cites_line(&[]), "");
    }

    #[test]
    fn cites_line_keeps_order_and_drops_repeats() {
        assert_eq!(
            cites_line(&["[[b]]".into(), "[[a:F-1]]".into(), "[[b]]".into()]),
            "**Cites:** [[b]], [[a:F-1]]"
        );
    }

    #[test]
    fn with_cites_appends_one_labelled_line() {
        let section = PendingSection {
            title: "t".into(),
            body: "x\n".into(),
            anchor_heading: "## Template".into(),
            index_row: None,
        };
        let with = with_cites(&section, &["[[a]]".into()]);
        assert_eq!(with.body, "x\n\n**Cites:** [[a]]");
        assert_eq!(
            (with.title.as_str(), with.anchor_heading.as_str()),
            ("t", "## Template")
        );
        assert_eq!(with_cites(&section, &[]).body, "x\n");
    }
}
