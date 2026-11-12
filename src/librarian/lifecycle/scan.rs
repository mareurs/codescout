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

use anyhow::{anyhow, bail};
use chrono::NaiveDate;
use serde::Serialize;

use super::checks::{check, CheckPolicy, Finding};
use super::layout::TRACKERS_DIR;
use super::tree::{DocInput, LifecycleTree};
use crate::librarian::read_door::{self, ReadError};

/// A listed file that was left out of the scan, and why.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
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
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
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

/// Builds the `git` command a scan runs against a directory. Production passes [`git`]; a test
/// passes one that also shuts out the machine's git configuration.
type GitFor<'a> = &'a dyn Fn(&Path) -> Command;

/// Run `cmd` and return its stdout. A failed run is an error whose own message says what was
/// attempted and why it failed (git's stderr, or the OS error): a tool returns only the
/// outermost message, so nothing is left to a `.context()` above it.
fn stdout_of(mut cmd: Command, what: &str) -> anyhow::Result<Vec<u8>> {
    let out = cmd
        .output()
        .map_err(|e| anyhow!("cannot {what}: cannot run `git`: {e}"))?;
    if !out.status.success() {
        bail!(
            "cannot {what}: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(out.stdout)
}

/// `project_root` must be the top of its checkout: git prints paths relative to its working
/// directory, so one run from a subdirectory would hand back paths that are not repo-relative.
fn require_top_level(project_root: &Path, git_for: GitFor) -> anyhow::Result<()> {
    let shown = project_root.display();
    let mut cmd = git_for(project_root);
    cmd.args(["rev-parse", "--show-toplevel"]);
    let out = stdout_of(cmd, &format!("determine the git top level of `{shown}`"))?;
    let top = String::from_utf8(out)
        .map_err(|_| anyhow!("the git top level of `{shown}` is not UTF-8"))?
        .trim_end_matches('\n')
        .to_string();
    let top = PathBuf::from(&top)
        .canonicalize()
        .map_err(|e| anyhow!("cannot resolve the git top level `{top}` of `{shown}`: {e}"))?;
    let root = project_root
        .canonicalize()
        .map_err(|e| anyhow!("cannot resolve `{shown}`: {e}"))?;
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
fn list_paths(project_root: &Path, git_for: GitFor) -> anyhow::Result<Vec<String>> {
    let mut cmd = git_for(project_root);
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
    let bytes = stdout_of(
        cmd,
        &format!("list the files of `{}`", project_root.display()),
    )?;
    let mut paths = BTreeSet::new();
    for raw in bytes.split(|b| *b == 0).filter(|p| !p.is_empty()) {
        let path = std::str::from_utf8(raw).map_err(|_| {
            anyhow!(
                "cannot scan `{}`: a listed path is not UTF-8: {}",
                project_root.display(),
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

/// The text of a read error without the machine's paths: the read door names a file by its
/// absolute path, and a tool's error or a `skipped` row should read the same on every machine.
/// The file's own `<root>/<rel_path>: ` prefix goes (the caller names the file), and any other
/// path under a root loses its `<root>/`.
fn portable_reason(e: &ReadError, roots: &[&Path], rel_path: &str) -> String {
    let mut text = e.to_string();
    for root in roots {
        let root = root.display();
        text = text.replace(&format!("{root}/{rel_path}: "), "");
        text = text.replace(&format!("{root}/"), "");
    }
    text
}

/// Read every Markdown file of the checkout at `project_root`, and every file under
/// `docs/trackers`, as [`DocInput`]s with repo-relative `/` paths.
///
/// - `project_root` must be the top level of a git checkout, else an error.
/// - A file that is gone from disk but still listed (deleted, not yet staged) is skipped.
/// - Any other failed read is an error naming the path when the file is under `docs/trackers`,
///   and a skip otherwise: outside the trackers a document only supplies a `closed:` date.
/// - A non-Markdown file under `docs/trackers` gets empty text; `place` refuses it by name.
/// - An error message names the path and the cause itself: a tool returns only the outermost
///   message of an error.
pub fn load_docs(project_root: &Path) -> anyhow::Result<LoadedDocs> {
    load_docs_with(project_root, &git)
}

fn load_docs_with(project_root: &Path, git_for: GitFor) -> anyhow::Result<LoadedDocs> {
    require_top_level(project_root, git_for)?;
    let paths = list_paths(project_root, git_for)?;
    let door = read_door::for_project(project_root);
    let canonical_root = project_root.canonicalize().ok();
    let roots: Vec<&Path> = std::iter::once(project_root)
        .chain(canonical_root.as_deref())
        .collect();
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
                bail!(
                    "cannot read tracker file `{rel_path}`: {}; \
                     the scan stops rather than skip it",
                    portable_reason(&e, &roots, &rel_path)
                );
            }
            Err(e) => {
                let reason = portable_reason(&e, &roots, &rel_path);
                skipped.push(SkippedDoc { rel_path, reason });
            }
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
    lifecycle_findings_with(project_root, today, policy, &git)
}

fn lifecycle_findings_with(
    project_root: &Path,
    today: NaiveDate,
    policy: &CheckPolicy,
    git_for: GitFor,
) -> anyhow::Result<LifecycleReport> {
    let LoadedDocs { docs, skipped } = load_docs_with(project_root, git_for)?;
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

    /// `git` for a fixture: the production builder, with the machine's git configuration shut
    /// out. This machine signs commits (`commit.gpgsign`), and a global ignore file or
    /// `init.templateDir` would change what the fixture lists.
    fn hermetic(dir: &Path) -> Command {
        let mut cmd = git(dir);
        cmd.env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            // Where git looks for its default global ignore file.
            .env("XDG_CONFIG_HOME", dir);
        cmd
    }

    /// [`load_docs`] through the hermetic `git`.
    fn load(root: &Path) -> anyhow::Result<LoadedDocs> {
        load_docs_with(root, &hermetic)
    }

    /// [`lifecycle_findings`] through the hermetic `git`.
    fn findings(
        root: &Path,
        today: NaiveDate,
        policy: &CheckPolicy,
    ) -> anyhow::Result<LifecycleReport> {
        lifecycle_findings_with(root, today, policy, &hermetic)
    }

    /// A fixture directory that is a git checkout.
    fn checkout() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        run_git(dir.path(), &["-c", "init.defaultBranch=main", "init", "-q"]);
        dir
    }

    fn run_git(dir: &Path, args: &[&str]) {
        let out = hermetic(dir)
            .args(["-c", "user.name=t", "-c", "user.email=t@t"])
            .args(["-c", "commit.gpgsign=false"])
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
        let loaded = load(dir.path()).unwrap();
        assert_eq!(
            rel_paths(&loaded),
            ["docs/trackers/ledgers/l/2026-10-01-waits.md", LEDGER_README]
        );
        let report = findings(dir.path(), d("2026-11-11"), &CheckPolicy::default()).unwrap();
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
        let loaded = load(dir.path()).unwrap();
        assert_eq!(rel_paths(&loaded), ["kept.md"]);
    }

    #[test]
    fn a_cited_closed_target_outside_docs_trackers_is_dated() {
        let dir = checkout();
        write(dir.path(), LEDGER_README, LEDGER);
        write(dir.path(), CITER, &cites_a_fixed_bug());
        write(dir.path(), "docs/issues/2026-10-05-the-bug.md", BUG);
        let report = findings(dir.path(), d("2026-11-11"), &CheckPolicy::default()).unwrap();
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
        let loaded = load(dir.path()).unwrap();
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
        let loaded = load(dir.path()).unwrap();
        assert_eq!(
            rel_paths(&loaded),
            ["docs/trackers/ledgers/l/notes.txt", LEDGER_README]
        );
        assert_eq!(loaded.docs[0].text, "");
        let report = findings(dir.path(), d("2026-11-11"), &CheckPolicy::default()).unwrap();
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
        let loaded = load(dir.path()).unwrap();
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
    fn a_tracker_entry_moved_to_archive_but_not_staged_is_skipped_not_fatal() {
        let dir = checkout();
        let old = "docs/trackers/ledgers/l/2026-10-01-x.md";
        let new = "docs/trackers/ledgers/l/archive/2026-10-01-x.md";
        write(dir.path(), LEDGER_README, LEDGER);
        write(dir.path(), old, &entry("fixed", "closed: 2026-10-02\n", ""));
        run_git(dir.path(), &["add", LEDGER_README, old]);
        // A plain `mv` into archive/ before `git add`: the index still lists the old path.
        std::fs::create_dir_all(dir.path().join(new).parent().unwrap()).unwrap();
        std::fs::rename(dir.path().join(old), dir.path().join(new)).unwrap();
        let loaded = load(dir.path()).unwrap();
        assert_eq!(rel_paths(&loaded), [new, LEDGER_README]);
        assert_eq!(
            loaded.skipped,
            [SkippedDoc {
                rel_path: old.into(),
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
        // A tool returns `err.to_string()` alone, so the path and the cause are in it.
        let msg = load(dir.path()).unwrap_err().to_string();
        assert!(
            msg.starts_with(&format!("cannot read tracker file `{rel}`: ")),
            "{msg}"
        );
        assert!(msg.contains("invalid utf-8"), "{msg}");
        // The path is named once, and the machine's directory is not in the text.
        assert_eq!(msg.matches(rel).count(), 1, "{msg}");
        let root = dir.path().display().to_string();
        assert!(!msg.contains(&root), "{msg}");
        assert!(msg.ends_with("the scan stops rather than skip it"), "{msg}");
    }

    #[test]
    fn a_non_utf8_file_outside_docs_trackers_is_skipped() {
        let dir = checkout();
        std::fs::create_dir_all(dir.path().join("docs/issues")).unwrap();
        std::fs::write(dir.path().join("docs/issues/bad.md"), [0xff, 0xfe, b'\n']).unwrap();
        write(dir.path(), "docs/issues/ok.md", "# ok\n");
        let loaded = load(dir.path()).unwrap();
        assert_eq!(rel_paths(&loaded), ["docs/issues/ok.md"]);
        assert_eq!(loaded.skipped.len(), 1);
        assert_eq!(loaded.skipped[0].rel_path, "docs/issues/bad.md");
        let reason = &loaded.skipped[0].reason;
        assert!(reason.contains("invalid utf-8"), "{reason}");
        // Portable: no directory of this machine.
        assert!(
            !reason.contains(&dir.path().display().to_string()),
            "{reason}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_read_error_that_names_a_path_is_stripped_of_this_machine_s_directory() {
        use std::os::unix::fs::PermissionsExt;
        let dir = checkout();
        let outside = "docs/issues/secret.md";
        let tracker = "docs/trackers/ledgers/l/2026-10-01-secret.md";
        for rel in [outside, tracker] {
            write(dir.path(), rel, "# secret\n");
            let path = dir.path().join(rel);
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).unwrap();
        }
        if std::fs::read(dir.path().join(outside)).is_ok() {
            return; // root reads a mode-000 file: nothing to observe
        }
        let root = dir.path().display().to_string();

        // Outside the trackers: skipped, and the reason is portable.
        std::fs::remove_file(dir.path().join(tracker)).unwrap();
        let loaded = load(dir.path()).unwrap();
        let reason = &loaded.skipped[0].reason;
        assert_eq!(loaded.skipped[0].rel_path, outside);
        assert!(reason.contains("Permission denied"), "{reason}");
        assert!(!reason.contains(&root), "{reason}");
        assert!(!reason.contains(outside), "{reason}");

        // Under the trackers: an error that names the file once, and not this machine's directory.
        write(dir.path(), tracker, "# secret\n");
        std::fs::set_permissions(
            dir.path().join(tracker),
            std::fs::Permissions::from_mode(0o000),
        )
        .unwrap();
        let msg = load(dir.path()).unwrap_err().to_string();
        assert!(msg.contains("Permission denied"), "{msg}");
        assert_eq!(msg.matches(tracker).count(), 1, "{msg}");
        assert!(!msg.contains(&root), "{msg}");
    }

    #[test]
    fn a_sibling_folder_of_the_trackers_is_not_a_tracker_folder() {
        let dir = checkout();
        std::fs::create_dir_all(dir.path().join("docs/trackers-old")).unwrap();
        std::fs::write(
            dir.path().join("docs/trackers-old/bad.md"),
            [0xff, 0xfe, b'\n'],
        )
        .unwrap();
        let loaded = load(dir.path()).unwrap();
        assert!(loaded.docs.is_empty());
        assert_eq!(loaded.skipped.len(), 1);
        assert_eq!(loaded.skipped[0].rel_path, "docs/trackers-old/bad.md");
    }

    #[cfg(unix)]
    #[test]
    #[cfg_attr(
        target_os = "macos",
        ignore = "APFS refuses a non-UTF-8 file name (EILSEQ); see FS_KEEPS_NON_UTF8_NAMES"
    )]
    fn a_listed_path_that_is_not_utf8_is_an_error() {
        use std::os::unix::ffi::OsStrExt;
        let dir = checkout();
        let name = std::ffi::OsStr::from_bytes(b"bad-\xff.md");
        std::fs::write(dir.path().join(name), "# x\n").unwrap();
        let msg = load(dir.path()).unwrap_err().to_string();
        assert!(msg.contains("a listed path is not UTF-8"), "{msg}");
        assert!(msg.contains(&dir.path().display().to_string()), "{msg}");
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
        let merge = hermetic(root)
            .args(["-c", "user.name=t", "-c", "user.email=t@t"])
            .args(["-c", "commit.gpgsign=false"])
            .args(["merge", "other"])
            .output()
            .unwrap();
        assert!(!merge.status.success(), "the merge must conflict");
        // The fixture really lists the path three times (one per stage).
        let mut unmerged = hermetic(root);
        unmerged.args(["ls-files", "-u", "--", "notes.md"]);
        let unmerged = stdout_of(unmerged, "list unmerged files").unwrap();
        assert_eq!(
            unmerged
                .split(|b| *b == b'\n')
                .filter(|l| !l.is_empty())
                .count(),
            3
        );

        let loaded = load(root).unwrap();
        assert_eq!(rel_paths(&loaded), ["notes.md"]);
        assert!(loaded.docs[0].text.contains("<<<<<<<"));
    }

    #[test]
    fn a_directory_that_is_not_the_top_level_is_an_error() {
        let dir = checkout();
        write(dir.path(), "sub/a.md", "# a\n");
        let msg = load(&dir.path().join("sub")).unwrap_err().to_string();
        assert!(msg.contains("is not the top level"), "{msg}");
        assert!(msg.contains("scan the top level"), "{msg}");
    }

    #[test]
    fn a_directory_that_is_not_a_checkout_is_an_error_naming_it_and_git_s_reason() {
        // The temp directory may itself sit inside a checkout: git must not look above it.
        let plain = tempfile::tempdir().unwrap();
        let ceiling = plain.path().parent().unwrap().canonicalize().unwrap();
        let outside = |dir: &Path| {
            let mut cmd = hermetic(dir);
            cmd.env("GIT_CEILING_DIRECTORIES", &ceiling);
            cmd
        };
        let msg = load_docs_with(plain.path(), &outside)
            .unwrap_err()
            .to_string();
        let shown = plain.path().display();
        assert!(
            msg.starts_with(&format!(
                "cannot determine the git top level of `{shown}`: "
            )),
            "{msg}"
        );
        assert!(msg.contains("not a git repository"), "{msg}");
    }

    #[test]
    fn a_missing_git_is_an_error_naming_what_was_attempted() {
        let dir = checkout();
        let no_git = |_: &Path| Command::new("codescout-no-such-git-binary");
        let msg = load_docs_with(dir.path(), &no_git).unwrap_err().to_string();
        assert!(
            msg.starts_with("cannot determine the git top level of `"),
            "{msg}"
        );
        assert!(msg.contains("cannot run `git`"), "{msg}");
    }

    #[test]
    fn the_git_builder_removes_the_variables_that_redirect_git() {
        let cmd = git(Path::new("."));
        for var in ["GIT_DIR", "GIT_INDEX_FILE", "GIT_WORK_TREE"] {
            let entry = cmd
                .get_envs()
                .find(|(k, _)| *k == std::ffi::OsStr::new(var));
            assert_eq!(entry, Some((std::ffi::OsStr::new(var), None)), "{var}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_checkout_reached_through_a_symlink_is_its_own_top_level() {
        let dir = checkout();
        write(dir.path(), "kept.md", "# kept\n");
        let holder = tempfile::tempdir().unwrap();
        let link = holder.path().join("link");
        std::os::unix::fs::symlink(dir.path(), &link).unwrap();
        let loaded = load(&link).unwrap();
        assert_eq!(rel_paths(&loaded), ["kept.md"]);
    }

    #[test]
    fn the_public_entry_points_scan_a_checkout() {
        // Production `git`, not the hermetic builder: one plain fixture, no signing, no ignores.
        let dir = checkout();
        write(dir.path(), "kept.md", "# kept\n");
        let loaded = load_docs(dir.path()).unwrap();
        assert_eq!(rel_paths(&loaded), ["kept.md"]);
        let report =
            lifecycle_findings(dir.path(), d("2026-11-11"), &CheckPolicy::default()).unwrap();
        assert!(report.findings.is_empty() && report.skipped.is_empty());
    }

    #[test]
    fn the_report_serializes_to_this_exact_json() {
        let report = LifecycleReport {
            findings: vec![Finding::RevisitDue {
                entry: k("2026-10-01-waits"),
                revisit_by: d("2026-11-01"),
            }],
            skipped: vec![SkippedDoc {
                rel_path: "gone.md".into(),
                reason: "not on disk".into(),
            }],
        };
        assert_eq!(
            serde_json::to_string(&report).unwrap(),
            concat!(
                r#"{"findings":[{"check":"revisit_due","entry":"2026-10-01-waits","revisit_by":"2026-11-01"}],"#,
                r#""skipped":[{"rel_path":"gone.md","reason":"not on disk"}]}"#
            )
        );
    }

    #[test]
    fn the_report_carries_the_skipped_files_and_obeys_the_policy_it_is_given() {
        let dir = checkout();
        let closed_entry = "docs/trackers/ledgers/l/2026-10-01-fixed-recently.md";
        write(dir.path(), LEDGER_README, LEDGER);
        // Closed on 2026-11-05: six days before `today`.
        write(
            dir.path(),
            closed_entry,
            &entry("fixed", "closed: 2026-11-05\n", ""),
        );
        write(dir.path(), "gone.md", "# gone\n");
        run_git(dir.path(), &["add", "gone.md"]);
        std::fs::remove_file(dir.path().join("gone.md")).unwrap();
        let today = d("2026-11-11");
        let late = Finding::TerminalInLiveDir {
            entry: k("2026-10-01-fixed-recently"),
            closed: d("2026-11-05"),
        };

        // The default grace is 30 days: six days is not late.
        let default = findings(dir.path(), today, &CheckPolicy::default()).unwrap();
        assert!(!default.findings.contains(&late), "{:?}", default.findings);

        // A grace of 0 days makes the same entry late; the scan is the same one.
        let strict = CheckPolicy {
            terminal_grace_days: 0,
        };
        let report = findings(dir.path(), today, &strict).unwrap();
        assert!(report.findings.contains(&late), "{:?}", report.findings);

        // Both reports say what the scan could not read.
        let gone = [SkippedDoc {
            rel_path: "gone.md".into(),
            reason: "not on disk".into(),
        }];
        assert_eq!(default.skipped, gone);
        assert_eq!(report.skipped, gone);
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
            // A non-Markdown file is never read: the tree gets empty text for it.
            ("docs/trackers/ledgers/l/notes.txt", String::new()),
            ("docs/issues/2026-10-05-the-bug.md", BUG.to_string()),
        ];
        for (rel, text) in &files {
            write(dir.path(), rel, text);
        }
        write(dir.path(), "docs/trackers/ledgers/l/notes.txt", "scratch\n");
        let today = d("2026-11-11");
        let policy = CheckPolicy::default();

        let tree = LifecycleTree::build(files.iter().map(|(rel, text)| doc(rel, text)));
        let expected = check(&tree, today, &policy);
        assert!(
            expected.len() >= 3,
            "the fixture must produce findings: {expected:?}"
        );

        let report = findings(dir.path(), today, &policy).unwrap();
        assert_eq!(report.findings, expected);
        assert!(report.skipped.is_empty());
    }
}
