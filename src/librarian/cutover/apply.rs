//! `cutover export`'s writes: an [`ExportPlan`] applied to the working tree.
//!
//! This is the first step of the cutover that changes the working tree, so every rule here
//! refuses rather than guesses. The input is the plan of `HEAD`; the rule for each planned file is
//! "the file is as `HEAD` has it, or already as the plan wants it, or the tool leaves it alone":
//!
//! - Files are handled in path order, and a refusal never stops the files after it.
//! - A rename (`renamed_from`) moves the old path to the new one. The source must be a regular
//!   file (not a link) whose bytes equal the plan's `head_text`, and the target must not exist.
//!   A source that is gone while the target is there is a rename already done, not an error.
//! - The text of a file is then read through the read door. Bytes equal to the plan's new text:
//!   `unchanged`. Bytes equal to `head_text` (a new file has no `head_text` and is absent): written
//!   with the atomic write door. Anything else: `refused`, because the operator changed the file
//!   since `HEAD`. A second run therefore finds every file `unchanged` and writes nothing.
//! - Every path passes the repository-boundary refusal before the tool creates a directory or
//!   moves a file, so a link that leaves the repository builds nothing outside it.
//!
//! The tool never commits, stages or touches git state (D-R2). A refusal reason names no path:
//! the reasons are fixed texts, because the report that holds them is kept and shared.

use std::path::Path;

use serde::Serialize;

use crate::librarian::cutover::plan::{ExportPlan, FileChange};
use crate::librarian::read_door::{ReadDoor, ReadError};
use crate::librarian::util::{refuse_catalogued_path_outside_repo, write_catalogued_file};

/// Why a file whose bytes are neither the plan's text nor `HEAD`'s is left alone.
const CHANGED_SINCE_HEAD: &str = "changed since HEAD: commit or revert it, then run again";
/// Why a path that resolves outside the repository is left alone.
const OUTSIDE_THE_REPOSITORY: &str = "outside the repository: the path passes through a symbolic \
     link or a `..` that leaves the repository root";
/// Why a path that is itself a link is left alone.
const IS_A_LINK: &str = "the path is a symbolic link, and the tool never renames or writes a link";

/// What [`apply`] did with each planned file. Every path is repository-relative.
#[derive(Debug, Default, Serialize)]
pub struct ApplyOutcome {
    /// Files whose new text was written (a renamed file whose text changed is here too).
    pub written: Vec<String>,
    /// `(old path, new path)` of each rename this run did.
    pub renamed: Vec<(String, String)>,
    /// Files that already hold the planned text.
    pub unchanged: Vec<String>,
    /// `(path, reason)` of each file left alone. A reason names no path.
    pub refused: Vec<(String, String)>,
}

/// Apply `plan` to the working tree of the repository at `git_root`, one file at a time, in path
/// order. See the module doc for the rules; the result lists every planned file once, under
/// `unchanged` or `refused`, or under `written` (and `renamed`).
pub fn apply(plan: &ExportPlan, git_root: &Path) -> ApplyOutcome {
    let roots = vec![git_root.to_path_buf()];
    // A door with no memo: a write starts from the file as it is now.
    let door = ReadDoor::new(roots.clone());
    let mut out = ApplyOutcome::default();
    for (path, change) in &plan.files {
        if let Some(from) = &change.renamed_from {
            match rename_step(&roots, &door, git_root, from, change) {
                Ok(true) => out.renamed.push((from.clone(), path.clone())),
                Ok(false) => {}
                Err(reason) => {
                    out.refused.push((path.clone(), reason));
                    continue;
                }
            }
        }
        match text_step(&roots, &door, git_root, change) {
            Ok(true) => out.written.push(path.clone()),
            Ok(false) => out.unchanged.push(path.clone()),
            Err(reason) => out.refused.push((path.clone(), reason)),
        }
    }
    out
}

/// Move `from` to the file's path. `Ok(true)` when it moved, `Ok(false)` when the source is gone
/// and the target is there (a rename already done), and the reason it was left alone otherwise.
fn rename_step(
    roots: &[std::path::PathBuf],
    door: &ReadDoor,
    git_root: &Path,
    from: &str,
    change: &FileChange,
) -> Result<bool, String> {
    let src = git_root.join(from);
    let dst = git_root.join(&change.path);
    for path in [&dst, &src] {
        refuse_catalogued_path_outside_repo(roots, path, "move")
            .map_err(|_| OUTSIDE_THE_REPOSITORY.to_owned())?;
    }
    let meta = match std::fs::symlink_metadata(&src) {
        Ok(meta) => meta,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return if std::fs::symlink_metadata(&dst).is_ok() {
                Ok(false)
            } else {
                Err("neither the old path nor the new one exists in the working tree".to_owned())
            };
        }
        Err(e) => {
            return Err(format!(
                "the file to rename cannot be inspected: {}",
                e.kind()
            ))
        }
    };
    if meta.file_type().is_symlink() {
        return Err(IS_A_LINK.to_owned());
    }
    if !meta.is_file() {
        return Err("the file to rename is not a regular file".to_owned());
    }
    match door.text(&src) {
        Ok(text) if Some(text.as_str()) == change.head_text.as_deref() => {}
        Ok(_) => return Err(CHANGED_SINCE_HEAD.to_owned()),
        Err(e) => return Err(read_reason(&e)),
    }
    if std::fs::symlink_metadata(&dst).is_ok() {
        return Err("the new name already exists in the working tree".to_owned());
    }
    make_parent_dirs(&dst)?;
    std::fs::rename(&src, &dst).map_err(|e| format!("the rename failed: {}", e.kind()))?;
    Ok(true)
}

/// Bring the file at `change.path` to the planned text. `Ok(true)` when it was written,
/// `Ok(false)` when it already held that text, and the reason it was left alone otherwise.
fn text_step(
    roots: &[std::path::PathBuf],
    door: &ReadDoor,
    git_root: &Path,
    change: &FileChange,
) -> Result<bool, String> {
    let path = git_root.join(&change.path);
    refuse_catalogued_path_outside_repo(roots, &path, "write")
        .map_err(|_| OUTSIDE_THE_REPOSITORY.to_owned())?;
    if std::fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(IS_A_LINK.to_owned());
    }
    let current = match door.text(&path) {
        Ok(text) => Some(text),
        Err(ReadError::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(read_reason(&e)),
    };
    if current.as_deref() == Some(change.new_text.as_str()) {
        return Ok(false);
    }
    // `HEAD`'s text, or nothing for a new file: anything else is the operator's edit.
    if current.as_deref() != change.head_text.as_deref() {
        return Err(CHANGED_SINCE_HEAD.to_owned());
    }
    make_parent_dirs(&path)?;
    write_catalogued_file(roots, &path, &change.new_text).map_err(|e| {
        match e.downcast_ref::<std::io::Error>() {
            Some(io) => format!("the write failed: {}", io.kind()),
            None => "the write was refused: a link, or a path outside the repository".to_owned(),
        }
    })?;
    Ok(true)
}

/// Create the directories above `path`. Called only after the boundary refusal passed for `path`.
fn make_parent_dirs(path: &Path) -> Result<(), String> {
    match path.parent() {
        Some(dir) => std::fs::create_dir_all(dir)
            .map_err(|e| format!("a directory cannot be created: {}", e.kind())),
        None => Ok(()),
    }
}

/// A read that did not return text, as a reason that names no path.
fn read_reason(e: &ReadError) -> String {
    match e {
        ReadError::Refused(_) => "the read door refused the file: it is not a regular file, or \
                                  it resolves outside the repository"
            .to_owned(),
        ReadError::Io(io) if io.kind() == std::io::ErrorKind::InvalidData => {
            "the file is not UTF-8 text".to_owned()
        }
        ReadError::Io(io) => format!("the file cannot be read: {}", io.kind()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::cutover::old_rows::OutOfScope;
    use crate::librarian::cutover::plan::FileChange;
    use crate::librarian::derived::testkit::{repo_with, FILE};
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    const CHANGED: &str = "changed since HEAD: commit or revert it, then run again";

    /// A temporary git repository whose working tree equals `HEAD`.
    struct Repo {
        _dir: tempfile::TempDir,
        root: PathBuf,
    }

    fn repo(files: &[(&str, &str)]) -> Repo {
        let dir = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(dir.path()).unwrap();
        let entries: Vec<(&str, u32, &[u8])> = files
            .iter()
            .map(|&(path, text)| (path, FILE, text.as_bytes()))
            .collect();
        let git = repo_with(&root, &entries);
        let mut checkout = git2::build::CheckoutBuilder::new();
        checkout.force();
        git.checkout_head(Some(&mut checkout)).unwrap();
        for (path, text) in files {
            assert_eq!(std::fs::read_to_string(root.join(path)).unwrap(), *text);
        }
        Repo { _dir: dir, root }
    }

    fn change(path: &str, head: Option<&str>, new: &str) -> FileChange {
        FileChange {
            path: path.to_owned(),
            renamed_from: None,
            head_text: head.map(str::to_owned),
            new_text: new.to_owned(),
        }
    }

    fn renamed(from: &str, to: &str, head: &str, new: &str) -> FileChange {
        FileChange {
            path: to.to_owned(),
            renamed_from: Some(from.to_owned()),
            head_text: Some(head.to_owned()),
            new_text: new.to_owned(),
        }
    }

    fn plan(changes: Vec<FileChange>) -> ExportPlan {
        ExportPlan {
            repo_name: "proj".to_owned(),
            commit: "0123456789abcdef0123456789abcdef01234567".to_owned(),
            shallow: false,
            population: BTreeMap::new(),
            items: Vec::new(),
            files: changes.into_iter().map(|c| (c.path.clone(), c)).collect(),
            out_of_scope: OutOfScope::default(),
        }
    }

    fn read(repo: &Repo, rel: &str) -> String {
        std::fs::read_to_string(repo.root.join(rel)).unwrap()
    }

    fn no_absolute_path(outcome: &ApplyOutcome, repo: &Repo) {
        for (path, reason) in &outcome.refused {
            for text in [path, reason] {
                assert!(
                    !text.contains(repo.root.to_str().unwrap()) && !text.contains("/tmp"),
                    "an absolute path in a refusal: {text}"
                );
            }
        }
    }

    // mutation: skip the write for every other file; write a file twice; list a file twice.
    #[test]
    fn apply_writes_each_changed_file_once() {
        let r = repo(&[("a.md", "# A\n"), ("sub/b.md", "# B\n"), ("c.md", "# C\n")]);
        let p = plan(vec![
            change("a.md", Some("# A\n"), "# A\nmore\n"),
            change("sub/b.md", Some("# B\n"), "# B\nmore\n"),
        ]);
        let out = apply(&p, &r.root);
        assert_eq!(out.written, vec!["a.md", "sub/b.md"], "{out:?}");
        assert!(out.refused.is_empty() && out.unchanged.is_empty() && out.renamed.is_empty());
        assert_eq!(read(&r, "a.md"), "# A\nmore\n");
        assert_eq!(read(&r, "sub/b.md"), "# B\nmore\n");
        assert_eq!(
            read(&r, "c.md"),
            "# C\n",
            "a file outside the plan is untouched"
        );
    }

    // mutation: drop the `head_text` comparison (write whatever is there); stop at the first refusal.
    #[test]
    fn a_file_edited_since_head_is_refused_and_left_unchanged() {
        let r = repo(&[("a.md", "# A\n"), ("b.md", "# B\n")]);
        std::fs::write(r.root.join("a.md"), "# A\nedited by hand\n").unwrap();
        let p = plan(vec![
            change("a.md", Some("# A\n"), "# A\nmore\n"),
            change("b.md", Some("# B\n"), "# B\nmore\n"),
        ]);
        let out = apply(&p, &r.root);
        assert_eq!(
            out.refused,
            vec![("a.md".to_owned(), CHANGED.to_owned())],
            "{out:?}"
        );
        assert_eq!(
            read(&r, "a.md"),
            "# A\nedited by hand\n",
            "left as the operator has it"
        );
        assert_eq!(
            out.written,
            vec!["b.md"],
            "a refusal does not stop the next file"
        );
        assert_eq!(read(&r, "b.md"), "# B\nmore\n");
        no_absolute_path(&out, &r);
    }

    // mutation: remove the `current == new_text` branch (the file then reads as changed since HEAD);
    // always write.
    #[test]
    fn a_second_export_finds_everything_written_and_writes_nothing() {
        let r = repo(&[("a.md", "# A\n"), ("Old_Name.md", "# Old\n")]);
        let p = plan(vec![
            change("a.md", Some("# A\n"), "# A\nmore\n"),
            renamed("Old_Name.md", "old-name.md", "# Old\n", "# Old\nmore\n"),
            change("docs/new.sidecar.md", None, "side\n"),
        ]);
        let first = apply(&p, &r.root);
        assert!(first.refused.is_empty(), "{first:?}");
        assert_eq!(first.written.len(), 3, "{first:?}");

        let stamp = |rel: &str| {
            let meta = std::fs::metadata(r.root.join(rel)).unwrap();
            (
                std::fs::read(r.root.join(rel)).unwrap(),
                meta.modified().unwrap(),
            )
        };
        let before: Vec<_> = ["a.md", "old-name.md", "docs/new.sidecar.md"]
            .iter()
            .map(|p| stamp(p))
            .collect();
        std::thread::sleep(std::time::Duration::from_millis(30));

        let second = apply(&p, &r.root);
        assert!(second.written.is_empty(), "{second:?}");
        assert!(second.renamed.is_empty(), "{second:?}");
        assert!(second.refused.is_empty(), "{second:?}");
        assert_eq!(
            second.unchanged,
            vec!["a.md", "docs/new.sidecar.md", "old-name.md"],
            "every path, in path order"
        );
        let after: Vec<_> = ["a.md", "old-name.md", "docs/new.sidecar.md"]
            .iter()
            .map(|p| stamp(p))
            .collect();
        assert_eq!(before, after, "bytes and mtimes are equal");
    }

    // mutation: skip `std::fs::rename`; write the new text before the rename; report no rename.
    #[test]
    fn a_rename_moves_the_file_then_writes_its_new_text() {
        let r = repo(&[("docs/Old_Name.md", "# Old\n")]);
        let p = plan(vec![renamed(
            "docs/Old_Name.md",
            "docs/old-name.md",
            "# Old\n",
            "# Old\nrewritten\n",
        )]);
        let out = apply(&p, &r.root);
        assert!(out.refused.is_empty(), "{out:?}");
        assert_eq!(
            out.renamed,
            vec![("docs/Old_Name.md".to_owned(), "docs/old-name.md".to_owned())]
        );
        assert_eq!(out.written, vec!["docs/old-name.md"]);
        assert!(
            !r.root.join("docs/Old_Name.md").exists(),
            "the old path is not kept"
        );
        assert_eq!(read(&r, "docs/old-name.md"), "# Old\nrewritten\n");
    }

    // mutation: refuse when the source is gone; rename without checking the target is the answer.
    #[test]
    fn a_rename_already_done_is_unchanged() {
        let r = repo(&[("Old_Name.md", "# Old\n")]);
        let p = plan(vec![renamed(
            "Old_Name.md",
            "old-name.md",
            "# Old\n",
            "# Old\nrewritten\n",
        )]);
        // The operator did the rename and the text change by hand.
        std::fs::rename(r.root.join("Old_Name.md"), r.root.join("old-name.md")).unwrap();
        std::fs::write(r.root.join("old-name.md"), "# Old\nrewritten\n").unwrap();
        let out = apply(&p, &r.root);
        assert_eq!(out.unchanged, vec!["old-name.md"], "{out:?}");
        assert!(out.renamed.is_empty() && out.written.is_empty() && out.refused.is_empty());

        // The operator did only the rename: the text is still written.
        let r2 = repo(&[("Old_Name.md", "# Old\n")]);
        std::fs::rename(r2.root.join("Old_Name.md"), r2.root.join("old-name.md")).unwrap();
        let out = apply(&p, &r2.root);
        assert_eq!(out.written, vec!["old-name.md"], "{out:?}");
        assert!(out.renamed.is_empty() && out.refused.is_empty());
        assert_eq!(read(&r2, "old-name.md"), "# Old\nrewritten\n");

        // A source that is gone and a target that is absent is not a rename already done.
        let r3 = repo(&[("other.md", "# O\n")]);
        let out = apply(&p, &r3.root);
        assert_eq!(out.refused.len(), 1, "{out:?}");
        assert!(!r3.root.join("old-name.md").exists());
    }

    // mutation: drop the `symlink_metadata` check on the source.
    #[cfg(unix)]
    #[test]
    fn a_rename_whose_source_is_a_symlink_is_refused() {
        let r = repo(&[("real.md", "# Old\n"), ("Old_Name.md", "# Old\n")]);
        // The link reads, through the door, as the bytes of HEAD; only its type refuses it.
        std::fs::remove_file(r.root.join("Old_Name.md")).unwrap();
        std::os::unix::fs::symlink("real.md", r.root.join("Old_Name.md")).unwrap();
        let p = plan(vec![renamed(
            "Old_Name.md",
            "old-name.md",
            "# Old\n",
            "# Old\nrewritten\n",
        )]);
        let out = apply(&p, &r.root);
        assert_eq!(out.refused.len(), 1, "{out:?}");
        assert!(out.refused[0].1.contains("symbolic link"), "{out:?}");
        assert!(out.renamed.is_empty() && out.written.is_empty());
        assert!(
            std::fs::symlink_metadata(r.root.join("Old_Name.md"))
                .unwrap()
                .file_type()
                .is_symlink(),
            "the link stays"
        );
        assert!(!r.root.join("old-name.md").exists());
        no_absolute_path(&out, &r);
    }

    // mutation: drop the source-bytes comparison.
    #[test]
    fn a_rename_of_an_edited_source_is_refused_and_moves_nothing() {
        let r = repo(&[("Old_Name.md", "# Old\n")]);
        std::fs::write(r.root.join("Old_Name.md"), "# Old\nedited\n").unwrap();
        let p = plan(vec![renamed(
            "Old_Name.md",
            "old-name.md",
            "# Old\n",
            "# Old\nrewritten\n",
        )]);
        let out = apply(&p, &r.root);
        assert_eq!(
            out.refused,
            vec![("old-name.md".to_owned(), CHANGED.to_owned())]
        );
        assert!(r.root.join("Old_Name.md").exists());
        assert!(!r.root.join("old-name.md").exists());
        assert_eq!(read(&r, "Old_Name.md"), "# Old\nedited\n");
    }

    // mutation: drop the target-exists check.
    #[test]
    fn a_rename_onto_an_existing_file_is_refused_and_moves_nothing() {
        let r = repo(&[("Old_Name.md", "# Old\n"), ("old-name.md", "# Taken\n")]);
        let p = plan(vec![renamed(
            "Old_Name.md",
            "old-name.md",
            "# Old\n",
            "# Old\nrewritten\n",
        )]);
        let out = apply(&p, &r.root);
        assert_eq!(out.refused.len(), 1, "{out:?}");
        assert!(out.refused[0].1.contains("already exists"), "{out:?}");
        assert!(out.renamed.is_empty() && out.written.is_empty());
        assert_eq!(read(&r, "Old_Name.md"), "# Old\n");
        assert_eq!(read(&r, "old-name.md"), "# Taken\n");
    }

    // mutation: drop the `create_dir_all` call.
    #[test]
    fn a_new_sidecar_creates_its_directory() {
        let r = repo(&[("a.md", "# A\n")]);
        let p = plan(vec![change("docs/deep/er/a.sidecar.md", None, "side\n")]);
        assert!(!r.root.join("docs").exists());
        let out = apply(&p, &r.root);
        assert!(out.refused.is_empty(), "{out:?}");
        assert_eq!(out.written, vec!["docs/deep/er/a.sidecar.md"]);
        assert_eq!(read(&r, "docs/deep/er/a.sidecar.md"), "side\n");
    }

    // mutation: treat an existing file as absent when the plan has no HEAD text.
    #[test]
    fn a_new_file_that_exists_with_other_text_is_refused() {
        let r = repo(&[("a.sidecar.md", "# Mine\n")]);
        let p = plan(vec![change("a.sidecar.md", None, "side\n")]);
        let out = apply(&p, &r.root);
        assert_eq!(
            out.refused,
            vec![("a.sidecar.md".to_owned(), CHANGED.to_owned())]
        );
        assert_eq!(read(&r, "a.sidecar.md"), "# Mine\n");
    }

    // mutation: drop `refuse_catalogued_path_outside_repo` for a written path (the `create_dir_all`
    // then builds the directory outside); join `..` paths without the check.
    #[cfg(unix)]
    #[test]
    fn a_path_that_leaves_the_root_is_refused() {
        let r = repo(&[("a.md", "# A\n")]);
        let outside = tempfile::tempdir().unwrap();
        let outside_dir = std::fs::canonicalize(outside.path()).unwrap();
        std::os::unix::fs::symlink(&outside_dir, r.root.join("esc")).unwrap();
        let p = plan(vec![
            change("esc/newdir/x.md", None, "x\n"),
            change("esc/y.md", None, "y\n"),
            change("../escaped.md", None, "z\n"),
        ]);
        let out = apply(&p, &r.root);
        assert_eq!(out.refused.len(), 3, "{out:?}");
        assert!(out.written.is_empty(), "{out:?}");
        assert_eq!(
            std::fs::read_dir(&outside_dir).unwrap().count(),
            0,
            "nothing is created outside the root, not even a directory"
        );
        assert!(!r.root.parent().unwrap().join("escaped.md").exists());
        for (_, reason) in &out.refused {
            assert!(reason.contains("outside the repository"), "{reason}");
        }
        no_absolute_path(&out, &r);
        for (path, reason) in &out.refused {
            assert!(
                !path.contains(outside_dir.to_str().unwrap())
                    && !reason.contains(outside_dir.to_str().unwrap()),
                "the resolved location outside the root is not named"
            );
        }
    }

    // mutation: drop the boundary check on the rename's target (the file is then renamed out), or
    // on its source (an outside file is renamed in).
    #[cfg(unix)]
    #[test]
    fn a_rename_that_leaves_the_root_is_refused() {
        let r = repo(&[("Old_Name.md", "# Old\n")]);
        let outside = tempfile::tempdir().unwrap();
        let outside_dir = std::fs::canonicalize(outside.path()).unwrap();
        std::fs::write(outside_dir.join("src.md"), "# Old\n").unwrap();
        std::os::unix::fs::symlink(&outside_dir, r.root.join("esc")).unwrap();
        let p = plan(vec![
            // The target is outside through a link.
            renamed("Old_Name.md", "esc/moved.md", "# Old\n", "# Old\nnew\n"),
            // The source is outside through a link.
            renamed("esc/src.md", "inside.md", "# Old\n", "# Old\nnew\n"),
        ]);
        let out = apply(&p, &r.root);
        assert_eq!(out.refused.len(), 2, "{out:?}");
        assert!(out.renamed.is_empty() && out.written.is_empty(), "{out:?}");
        assert_eq!(read(&r, "Old_Name.md"), "# Old\n", "the source stays");
        assert!(!outside_dir.join("moved.md").exists(), "nothing moved out");
        assert!(
            outside_dir.join("src.md").exists(),
            "the outside file stays"
        );
        assert!(!r.root.join("inside.md").exists(), "nothing moved in");
        no_absolute_path(&out, &r);
    }
}
