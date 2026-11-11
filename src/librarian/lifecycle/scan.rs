//! Scan a git checkout into lifecycle findings: the one place of the lifecycle core that
//! touches the filesystem and git. It lists the Markdown files (and everything under
//! `docs/trackers`) that git knows or would add, reads them through the read door, and hands the
//! texts to the pure `LifecycleTree::build` and `check`.
//!
//! The listing is git's, not a directory walk: it honours `.gitignore`, and it includes a new
//! file that was never `git add`ed. Every `.md` file of the repository is read, not only
//! the trackers, because `closed:` dates come from any input document (a bug under
//! `docs/issues/` that an entry cites).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, bail, Context};
use chrono::NaiveDate;

use super::checks::{check, CheckPolicy, Finding};
use super::tree::{DocInput, LifecycleTree};
use crate::librarian::read_door::{self, ReadError};

/// The folder whose files are read whatever their extension: a non-Markdown file there is a
/// finding (`CollectionProblem::NotMarkdown`), and an unreadable one is an error.
const TRACKERS_DIR: &str = "docs/trackers";

/// A listed file that was left out of the scan, and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkippedDoc {
    pub rel_path: String,
    pub reason: String,
}

/// The documents of a checkout, and the listed files that could not be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoadedDocs {
    pub docs: Vec<DocInput>,
    pub skipped: Vec<SkippedDoc>,
}

/// The findings over a checkout, and the files the scan skipped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LifecycleReport {
    pub findings: Vec<Finding>,
    pub skipped: Vec<SkippedDoc>,
}

/// `git` run against `root`, with the variables that would redirect it removed.
fn git(root: &Path) -> Command {
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(root);
    for var in ["GIT_DIR", "GIT_INDEX_FILE", "GIT_WORK_TREE"] {
        cmd.env_remove(var);
    }
    cmd
}

/// Run `cmd` and return its stdout; a failed run is an error carrying git's message.
fn stdout_of(mut cmd: Command, what: &str) -> anyhow::Result<Vec<u8>> {
    let out = cmd
        .output()
        .with_context(|| format!("cannot run `git` to {what}"))?;
    if !out.status.success() {
        bail!(
            "`git` could not {what}: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(out.stdout)
}

/// `project_root` must be the top of its checkout: git prints paths relative to its working
/// directory, so one run from a subdirectory would hand back paths that are not repo-relative.
fn require_top_level(project_root: &Path) -> anyhow::Result<()> {
    let mut cmd = git(project_root);
    cmd.args(["rev-parse", "--show-toplevel"]);
    let out = stdout_of(cmd, "find the top level of the checkout")
        .with_context(|| format!("`{}` is not inside a git checkout", project_root.display()))?;
    let top = String::from_utf8(out)
        .context("the top level of the checkout is not UTF-8")?
        .trim_end_matches('\n')
        .to_string();
    let top = PathBuf::from(top)
        .canonicalize()
        .context("cannot resolve the top level of the checkout")?;
    let root = project_root
        .canonicalize()
        .with_context(|| format!("cannot resolve `{}`", project_root.display()))?;
    if top != root {
        bail!(
            "`{}` is not the top level of its git checkout (that is `{}`); \
             scan the top level",
            root.display(),
            top.display()
        );
    }
    Ok(())
}

/// The repo-relative paths git lists: tracked or untracked, not ignored, sorted and without
/// duplicates (git lists a path with a merge conflict once per stage).
fn list_paths(project_root: &Path) -> anyhow::Result<Vec<String>> {
    let mut cmd = git(project_root);
    cmd.args([
        "ls-files",
        "-z",
        "--cached",
        "--others",
        "--exclude-standard",
        "--",
        ":(icase)*.md",
        TRACKERS_DIR,
    ]);
    let bytes = stdout_of(cmd, "list the files of the checkout")?;
    let mut paths = BTreeSet::new();
    for raw in bytes.split(|b| *b == 0).filter(|p| !p.is_empty()) {
        let path = std::str::from_utf8(raw).map_err(|_| {
            anyhow!(
                "a listed path is not UTF-8: {}",
                String::from_utf8_lossy(raw)
            )
        })?;
        paths.insert(path.to_string());
    }
    Ok(paths.into_iter().collect())
}

fn is_markdown(rel_path: &str) -> bool {
    rel_path.to_ascii_lowercase().ends_with(".md")
}

fn is_tracker_file(rel_path: &str) -> bool {
    rel_path
        .strip_prefix(TRACKERS_DIR)
        .is_some_and(|rest| rest.starts_with('/'))
}

/// Read every Markdown file of the checkout at `project_root`, and every file under
/// `docs/trackers`, as [`DocInput`]s with repo-relative `/` paths.
///
/// - `project_root` must be the top level of a git checkout, else an error.
/// - A file that is gone from disk but still listed (deleted, not yet staged) is skipped.
/// - Any other failed read is an error naming the path when the file is under `docs/trackers`,
///   and a skip otherwise: outside the trackers a document only supplies a `closed:` date.
/// - A non-Markdown file under `docs/trackers` gets empty text; `place` refuses it by name.
pub fn load_docs(project_root: &Path) -> anyhow::Result<LoadedDocs> {
    require_top_level(project_root)?;
    let paths = list_paths(project_root)?;
    let door = read_door::for_project(project_root);
    let mut docs = Vec::with_capacity(paths.len());
    let mut skipped = Vec::new();
    for rel_path in paths {
        if !is_markdown(&rel_path) {
            docs.push(DocInput {
                rel_path,
                text: String::new(),
            });
            continue;
        }
        match door.text(&project_root.join(&rel_path)) {
            Ok(text) => docs.push(DocInput { rel_path, text }),
            Err(ReadError::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => {
                skipped.push(SkippedDoc {
                    rel_path,
                    reason: "not on disk".to_string(),
                });
            }
            Err(e) if is_tracker_file(&rel_path) => {
                return Err(anyhow!("{rel_path}: {e}")
                    .context("cannot read a tracker file; the scan stops rather than skip it"));
            }
            Err(e) => skipped.push(SkippedDoc {
                rel_path,
                reason: e.to_string(),
            }),
        }
    }
    Ok(LoadedDocs { docs, skipped })
}

/// [`load_docs`], then `LifecycleTree::build`, then `check`.
pub fn lifecycle_findings(
    project_root: &Path,
    today: NaiveDate,
    policy: &CheckPolicy,
) -> anyhow::Result<LifecycleReport> {
    let LoadedDocs { docs, skipped } = load_docs(project_root)?;
    let tree = LifecycleTree::build(docs);
    Ok(LifecycleReport {
        findings: check(&tree, today, policy),
        skipped,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::lifecycle::checks::CollectionProblem;
    use crate::librarian::lifecycle::testkit::{d, doc, entry, k, LEDGER};

    const BUG: &str = "---\nkind: bug\nstatus: fixed\nclosed: 2026-10-06\n---\n";
    const LEDGER_README: &str = "docs/trackers/ledgers/l/readme.md";
    const CITER: &str = "docs/trackers/ledgers/l/2026-10-01-cites-a-fixed-bug.md";

    /// A fixture directory that is a git checkout.
    fn checkout() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        run_git(dir.path(), &["-c", "init.defaultBranch=main", "init", "-q"]);
        dir
    }

    fn run_git(dir: &Path, args: &[&str]) {
        let out = git(dir)
            .args(["-c", "user.name=t", "-c", "user.email=t@t"])
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    fn write(root: &Path, rel: &str, text: &str) {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    fn rel_paths(loaded: &LoadedDocs) -> Vec<&str> {
        loaded.docs.iter().map(|d| d.rel_path.as_str()).collect()
    }

    fn cites_a_fixed_bug() -> String {
        entry("open", "", "**Rests on:** [[2026-10-05-the-bug]]\n")
    }

    #[test]
    fn a_new_unstaged_entry_is_scanned() {
        let dir = checkout();
        write(dir.path(), LEDGER_README, LEDGER);
        write(
            dir.path(),
            "docs/trackers/ledgers/l/2026-10-01-waits.md",
            &entry("waiting", "revisit_by: 2026-11-01\n", ""),
        );
        // Nothing was ever `git add`ed.
        let loaded = load_docs(dir.path()).unwrap();
        assert_eq!(
            rel_paths(&loaded),
            ["docs/trackers/ledgers/l/2026-10-01-waits.md", LEDGER_README]
        );
        let report =
            lifecycle_findings(dir.path(), d("2026-11-11"), &CheckPolicy::default()).unwrap();
        assert!(report.findings.contains(&Finding::RevisitDue {
            entry: k("2026-10-01-waits"),
            revisit_by: d("2026-11-01"),
        }));
        assert!(report.skipped.is_empty());
    }

    #[test]
    fn a_gitignored_file_is_not_scanned() {
        let dir = checkout();
        write(dir.path(), ".gitignore", "ignored.md\n");
        write(dir.path(), "ignored.md", "# ignored\n");
        write(dir.path(), "kept.md", "# kept\n");
        let loaded = load_docs(dir.path()).unwrap();
        assert_eq!(rel_paths(&loaded), ["kept.md"]);
    }

    #[test]
    fn a_cited_closed_target_outside_docs_trackers_is_dated() {
        let dir = checkout();
        write(dir.path(), LEDGER_README, LEDGER);
        write(dir.path(), CITER, &cites_a_fixed_bug());
        write(dir.path(), "docs/issues/2026-10-05-the-bug.md", BUG);
        let report =
            lifecycle_findings(dir.path(), d("2026-11-11"), &CheckPolicy::default()).unwrap();
        assert!(
            report.findings.contains(&Finding::CitedTargetClosed {
                entry: k("2026-10-01-cites-a-fixed-bug"),
                target: k("2026-10-05-the-bug"),
                closed: d("2026-10-06"),
            }),
            "{:?}",
            report.findings
        );
    }

    #[test]
    fn an_upper_case_extension_is_read_as_markdown() {
        let dir = checkout();
        let text = entry("open", "", "body\n");
        write(dir.path(), "docs/trackers/ledgers/l/2026-10-01-x.MD", &text);
        // Outside docs/trackers only the `:(icase)*.md` pathspec lists it.
        write(dir.path(), "docs/issues/Bug.MD", BUG);
        let loaded = load_docs(dir.path()).unwrap();
        assert_eq!(
            loaded.docs,
            [
                doc("docs/issues/Bug.MD", BUG),
                doc("docs/trackers/ledgers/l/2026-10-01-x.MD", &text)
            ]
        );
    }

    #[test]
    fn a_non_markdown_file_under_the_lifecycle_folders_gives_collection_invalid() {
        let dir = checkout();
        write(dir.path(), LEDGER_README, LEDGER);
        write(dir.path(), "docs/trackers/ledgers/l/notes.txt", "scratch\n");
        write(dir.path(), "docs/issues/outside.txt", "scratch\n");
        let loaded = load_docs(dir.path()).unwrap();
        assert_eq!(
            rel_paths(&loaded),
            ["docs/trackers/ledgers/l/notes.txt", LEDGER_README]
        );
        assert_eq!(loaded.docs[0].text, "");
        let report =
            lifecycle_findings(dir.path(), d("2026-11-11"), &CheckPolicy::default()).unwrap();
        assert!(
            report.findings.contains(&Finding::CollectionInvalid {
                subject: "docs/trackers/ledgers/l/notes.txt".into(),
                problem: CollectionProblem::NotMarkdown,
            }),
            "{:?}",
            report.findings
        );
    }

    #[test]
    fn a_file_deleted_but_not_staged_is_skipped_not_fatal() {
        let dir = checkout();
        write(dir.path(), "gone.md", "# gone\n");
        write(dir.path(), "kept.md", "# kept\n");
        run_git(dir.path(), &["add", "gone.md", "kept.md"]);
        std::fs::remove_file(dir.path().join("gone.md")).unwrap();
        let loaded = load_docs(dir.path()).unwrap();
        assert_eq!(rel_paths(&loaded), ["kept.md"]);
        assert_eq!(
            loaded.skipped,
            [SkippedDoc {
                rel_path: "gone.md".into(),
                reason: "not on disk".into()
            }]
        );
    }

    #[test]
    fn a_non_utf8_tracker_file_is_an_error_naming_its_path() {
        let dir = checkout();
        let rel = "docs/trackers/ledgers/l/2026-10-01-bad.md";
        std::fs::create_dir_all(dir.path().join(rel).parent().unwrap()).unwrap();
        std::fs::write(dir.path().join(rel), [0xff, 0xfe, b'\n']).unwrap();
        let err = load_docs(dir.path()).unwrap_err();
        assert!(format!("{err:#}").contains(rel), "{err:#}");
    }

    #[test]
    fn a_non_utf8_file_outside_docs_trackers_is_skipped() {
        let dir = checkout();
        std::fs::create_dir_all(dir.path().join("docs/issues")).unwrap();
        std::fs::write(dir.path().join("docs/issues/bad.md"), [0xff, 0xfe, b'\n']).unwrap();
        write(dir.path(), "docs/issues/ok.md", "# ok\n");
        let loaded = load_docs(dir.path()).unwrap();
        assert_eq!(rel_paths(&loaded), ["docs/issues/ok.md"]);
        assert_eq!(loaded.skipped.len(), 1);
        assert_eq!(loaded.skipped[0].rel_path, "docs/issues/bad.md");
        assert!(!loaded.skipped[0].reason.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn a_listed_path_that_is_not_utf8_is_an_error() {
        use std::os::unix::ffi::OsStrExt;
        let dir = checkout();
        let name = std::ffi::OsStr::from_bytes(b"bad-\xff.md");
        std::fs::write(dir.path().join(name), "# x\n").unwrap();
        let err = load_docs(dir.path()).unwrap_err();
        assert!(format!("{err:#}").contains("not UTF-8"), "{err:#}");
    }

    #[test]
    fn a_merge_conflicted_path_is_loaded_once() {
        let dir = checkout();
        let root = dir.path();
        write(root, "notes.md", "base\n");
        run_git(root, &["add", "notes.md"]);
        run_git(root, &["commit", "-q", "-m", "base"]);
        run_git(root, &["checkout", "-q", "-b", "other"]);
        write(root, "notes.md", "other\n");
        run_git(root, &["commit", "-q", "-am", "other"]);
        run_git(root, &["checkout", "-q", "main"]);
        write(root, "notes.md", "main\n");
        run_git(root, &["commit", "-q", "-am", "main"]);
        let merge = git(root)
            .args(["-c", "user.name=t", "-c", "user.email=t@t"])
            .args(["merge", "other"])
            .output()
            .unwrap();
        assert!(!merge.status.success(), "the merge must conflict");
        // The fixture really lists the path three times (one per stage).
        let mut unmerged = git(root);
        unmerged.args(["ls-files", "-u", "--", "notes.md"]);
        let unmerged = stdout_of(unmerged, "list unmerged files").unwrap();
        assert_eq!(
            unmerged
                .split(|b| *b == b'\n')
                .filter(|l| !l.is_empty())
                .count(),
            3
        );

        let loaded = load_docs(root).unwrap();
        assert_eq!(rel_paths(&loaded), ["notes.md"]);
        assert!(loaded.docs[0].text.contains("<<<<<<<"));
    }

    #[test]
    fn a_directory_that_is_not_the_top_level_is_an_error() {
        let dir = checkout();
        write(dir.path(), "sub/a.md", "# a\n");
        let err = load_docs(&dir.path().join("sub")).unwrap_err();
        assert!(format!("{err:#}").contains("not the top level"), "{err:#}");

        let plain = tempfile::tempdir().unwrap();
        let err = load_docs(plain.path()).unwrap_err();
        assert!(
            format!("{err:#}").contains("not inside a git checkout"),
            "{err:#}"
        );
    }

    #[test]
    fn findings_over_a_small_repo_match_the_pure_check() {
        let dir = checkout();
        let files = [
            (LEDGER_README, LEDGER.to_string()),
            (CITER, cites_a_fixed_bug()),
            (
                "docs/trackers/ledgers/l/2026-10-01-waits.md",
                entry("waiting", "revisit_by: 2026-11-01\n", ""),
            ),
            (
                "docs/trackers/ledgers/l/2026-10-01-fixed-long-ago.md",
                entry("fixed", "closed: 2026-10-02\n", ""),
            ),
            ("docs/trackers/ledgers/l/notes.txt", "scratch\n".to_string()),
            ("docs/issues/2026-10-05-the-bug.md", BUG.to_string()),
        ];
        for (rel, text) in &files {
            write(dir.path(), rel, text);
        }
        let today = d("2026-11-11");
        let policy = CheckPolicy::default();

        let tree = LifecycleTree::build(
            files
                .iter()
                .map(|(rel, text)| doc(rel, if is_markdown(rel) { text } else { "" })),
        );
        let expected = check(&tree, today, &policy);
        assert!(
            expected.len() >= 3,
            "the fixture must produce findings: {expected:?}"
        );

        let report = lifecycle_findings(dir.path(), today, &policy).unwrap();
        assert_eq!(report.findings, expected);
        assert!(report.skipped.is_empty());
    }
}
