//! `codescout key-gate` — refuse NEW document-key problems, reading git only.
//!
//! The gate wraps [`check_tree`] (pure) in the git reads a pre-commit hook needs. With
//! `--staged` it builds one [`GateReport`] over the staged index and one over `HEAD` and prints
//! the problems the index has that `HEAD` did not ([`GateReport::new_since`]); without it, it
//! prints every problem in the index. Each side reads the key-space file from its own tree
//! (`git show HEAD:<file>`, `git show :<file>`), never from the working tree.
//!
//! No catalog and no `.codescout` directory are needed: the command lists tracked paths with
//! `git ls-files -z` / `git ls-tree -r -z`, splits on NUL (a path with a space, a quote or a
//! non-ASCII byte survives), and keeps a path when its file name has a markdown extension
//! (`key::markdown_stem`, which accepts any case; a git pathspec would not).
//!
//! Exit codes: `0` clean, `1` problems, `2` it cannot ask (not a git repository, a malformed
//! key-space file at a revision it must read, a failing git). A `2` prints
//! `NOT CHECKED — <reason>` on stdout, so a hook can degrade open and say why.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, bail, Context, Result};
use clap::Args;
use serde_json::{json, Value};

use crate::librarian::current_project::lookup_git_root;
use crate::librarian::identity::gate::{check_tree, GateProblem, GateReport};
use crate::librarian::identity::key::markdown_stem;
use crate::librarian::identity::key_space::{KeySpace, KEY_SPACE_FILE_REL};
use crate::librarian::identity::repo_name::{self, RepoName};

#[derive(Debug, Args)]
pub struct KeyGateArgs {
    /// Project root override (defaults to cwd). The git root at or above it is checked.
    #[arg(long)]
    pub project: Option<PathBuf>,
    /// Judge only the problems the staged index has that HEAD did not (what a commit would
    /// add). Without it, every problem in the index is reported.
    #[arg(long)]
    pub staged: bool,
    /// Emit one JSON object on stdout instead of text.
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug)]
/// What a run decided and what it prints. [`run`] prints it and exits with `code`.
pub struct Outcome {
    /// `0` clean, `1` problems, `2` cannot ask.
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

/// A revision the gate reads a tree from.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Rev {
    Head,
    Index,
}

impl Rev {
    fn label(self) -> &'static str {
        match self {
            Rev::Head => "HEAD",
            Rev::Index => "the index",
        }
    }

    /// The `git show` object name of the key-space file in this tree.
    fn key_space_spec(self) -> String {
        match self {
            Rev::Head => format!("HEAD:{KEY_SPACE_FILE_REL}"),
            Rev::Index => format!(":{KEY_SPACE_FILE_REL}"),
        }
    }
}

/// The markdown paths of one tree, as UTF-8 strings, and how many it could not read.
#[derive(Debug, Default, PartialEq, Eq)]
struct Listing {
    md: Vec<String>,
    /// Paths with a markdown extension that are not valid UTF-8; they are no candidates.
    unreadable: usize,
}

/// True when the last component of `path` has an ASCII `.md` extension in any case. Works on
/// bytes so a path that is not UTF-8 can still be recognised as a markdown candidate.
fn looks_markdown(path: &[u8]) -> bool {
    let name = path.rsplit(|&b| b == b'/').next().unwrap_or(path);
    name.iter()
        .rposition(|&b| b == b'.')
        .is_some_and(|dot| name[dot + 1..].eq_ignore_ascii_case(b"md"))
}

/// Split NUL-terminated git output into paths and keep the markdown ones.
fn parse_listing(raw: &[u8]) -> Listing {
    let mut out = Listing::default();
    for chunk in raw.split(|&b| b == 0).filter(|c| !c.is_empty()) {
        match std::str::from_utf8(chunk) {
            Ok(path) => {
                let file_name = path.rsplit('/').next().unwrap_or(path);
                if markdown_stem(file_name).is_some() {
                    out.md.push(path.to_string());
                }
            }
            Err(_) if looks_markdown(chunk) => out.unreadable += 1,
            Err(_) => {}
        }
    }
    out
}

/// Run git at `root` and return its stdout; a non-zero exit is an error carrying its stderr.
/// The environment is inherited on purpose: inside a hook `GIT_INDEX_FILE` names the index the
/// commit will use.
fn git_stdout(root: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let out = git_raw(root, args)?;
    if !out.status.success() {
        bail!(
            "`git {}` failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(out.stdout)
}

fn git_raw(root: &Path, args: &[&str]) -> Result<std::process::Output> {
    Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .with_context(|| format!("running `git {}`", args.join(" ")))
}

/// True when `HEAD` names a commit; false for an unborn branch.
fn head_exists(root: &Path) -> Result<bool> {
    let out = git_raw(root, &["rev-parse", "--verify", "--quiet", "HEAD"])?;
    match out.status.code() {
        Some(0) => Ok(true),
        // `--verify --quiet` exits 1, silently, when the name resolves to nothing.
        Some(1) => Ok(false),
        _ => bail!(
            "`git rev-parse HEAD` failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ),
    }
}

/// The markdown paths of `HEAD`'s tree. An unborn `HEAD` has none.
fn list_md_at_head(root: &Path) -> Result<Listing> {
    if !head_exists(root)? {
        return Ok(Listing::default());
    }
    Ok(parse_listing(&git_stdout(
        root,
        &["ls-tree", "-r", "-z", "--name-only", "HEAD"],
    )?))
}

/// The markdown paths of the index.
fn list_md_in_index(root: &Path) -> Result<Listing> {
    Ok(parse_listing(&git_stdout(root, &["ls-files", "-z"])?))
}

/// Whether the key-space file is tracked at `rev`: an exact-path query, so absence is never
/// confused with a failing `git show`.
fn key_space_present(root: &Path, rev: Rev) -> Result<bool> {
    let raw = match rev {
        Rev::Head => git_stdout(root, &["ls-tree", "-z", "HEAD", "--", KEY_SPACE_FILE_REL])?,
        Rev::Index => git_stdout(root, &["ls-files", "-z", "--", KEY_SPACE_FILE_REL])?,
    };
    Ok(!raw.is_empty())
}

/// The key space of `rev`'s own tree, and whether the file exists there. An absent file is
/// [`KeySpace::empty`]; an unreadable or malformed one is an error naming the file and the
/// revision.
fn key_space_from(root: &Path, rev: Rev) -> Result<(KeySpace, bool)> {
    if !key_space_present(root, rev)? {
        return Ok((KeySpace::empty(), false));
    }
    let blob = git_stdout(root, &["show", &rev.key_space_spec()])?;
    let text = String::from_utf8(blob)
        .map_err(|_| anyhow!("{KEY_SPACE_FILE_REL} in {} is not valid UTF-8", rev.label()))?;
    let space = KeySpace::from_text(&text).map_err(|e| {
        anyhow!(
            "{KEY_SPACE_FILE_REL} in {} is malformed: {e:#}",
            rev.label()
        )
    })?;
    Ok((space, true))
}

/// One tree's report with the scope it was built from.
struct Side {
    label: &'static str,
    report: GateReport,
    space: KeySpace,
    /// A key-space file exists in this tree.
    file_present: bool,
    /// The tree has no commit yet (an unborn `HEAD`): an empty report and an empty key space.
    no_commit: bool,
    unreadable: usize,
}

fn side(root: &Path, rev: Rev, repo: &RepoName) -> Result<Side> {
    if rev == Rev::Head && !head_exists(root)? {
        return Ok(Side {
            label: rev.label(),
            report: check_tree(repo.clone(), &KeySpace::empty(), &[]),
            space: KeySpace::empty(),
            file_present: false,
            no_commit: true,
            unreadable: 0,
        });
    }
    let listing = match rev {
        Rev::Head => list_md_at_head(root)?,
        Rev::Index => list_md_in_index(root)?,
    };
    let (space, file_present) = key_space_from(root, rev)?;
    Ok(Side {
        label: rev.label(),
        report: check_tree(repo.clone(), &space, &listing.md),
        space,
        file_present,
        no_commit: false,
        unreadable: listing.unreadable,
    })
}

impl Side {
    /// Where this side's key space came from, for a message that names its scope.
    fn key_space_note(&self) -> String {
        if self.no_commit {
            return "no commit yet (nothing to compare with)".to_string();
        }
        if !self.file_present {
            return format!(
                "no {KEY_SPACE_FILE_REL} in {} (nothing is excluded)",
                self.label
            );
        }
        if self.space.patterns().is_empty() {
            return format!("{KEY_SPACE_FILE_REL} in {} (excludes nothing)", self.label);
        }
        format!(
            "{KEY_SPACE_FILE_REL} in {} (excludes: {})",
            self.label,
            self.space.patterns().join(", ")
        )
    }

    /// `N markdown file(s) in the key space, M outside; <key-space note>`.
    fn scope(&self) -> String {
        format!(
            "{} markdown file{} in the key space, {} outside; {}",
            self.report.in_space,
            if self.report.in_space == 1 { "" } else { "s" },
            self.report.outside.len(),
            self.key_space_note()
        )
    }

    /// Each outside path with the key-space patterns that exclude it.
    fn outside_with_patterns(&self) -> Vec<(&str, Vec<&str>)> {
        // One matcher per pattern, so the scope can name which pattern excluded a path.
        let singles: Vec<(&str, KeySpace)> = self
            .space
            .patterns()
            .iter()
            .filter_map(|p| Some((p.as_str(), KeySpace::from_patterns(&[p]).ok()?)))
            .collect();
        self.report
            .outside
            .iter()
            .map(|path| {
                let by = singles
                    .iter()
                    .filter(|(_, k)| k.is_outside(path))
                    .map(|(p, _)| *p)
                    .collect();
                (path.as_str(), by)
            })
            .collect()
    }

    fn key_space_json(&self) -> Value {
        json!({
            "revision": self.label,
            "file": KEY_SPACE_FILE_REL,
            "present": self.file_present,
            "no_commit": self.no_commit,
            "patterns": self.space.patterns(),
        })
    }
}

fn problem_json(p: &GateProblem) -> Value {
    match p {
        GateProblem::InvalidKey(_) => json!({"kind": "invalid_key", "message": p.render()}),
        GateProblem::DuplicateKey { key, rel_paths } => json!({
            "kind": "duplicate_key",
            "key": key.to_string(),
            "paths": rel_paths,
            "message": p.render(),
        }),
        GateProblem::PartialFolderExclusion { folder } => json!({
            "kind": "partial_folder_exclusion",
            "folder": folder,
            "message": p.render(),
        }),
    }
}

/// The reason on one line: a TOML error spans several.
fn one_line(e: &anyhow::Error) -> String {
    format!("{e:#}")
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn not_checked(json: bool, reason: &str) -> Outcome {
    let stdout = if json {
        format!(
            "{}\n",
            json!({"checked": false, "exit_code": 2, "not_checked": reason})
        )
    } else {
        format!("NOT CHECKED — {reason}\n")
    };
    Outcome {
        code: 2,
        stdout,
        stderr: String::new(),
    }
}

/// Decide the outcome of `args`, resolving `--project` against `cwd`. It returns no error:
/// whatever stops it from asking becomes a `NOT CHECKED` outcome with code 2.
pub fn evaluate(args: &KeyGateArgs, cwd: &Path) -> Outcome {
    check(args, cwd).unwrap_or_else(|e| not_checked(args.json, &one_line(&e)))
}

fn check(args: &KeyGateArgs, cwd: &Path) -> Result<Outcome> {
    let start = args.project.clone().unwrap_or_else(|| cwd.to_path_buf());
    let start = std::fs::canonicalize(&start)
        .with_context(|| format!("cannot resolve {}", start.display()))?;
    let root = lookup_git_root(&start)
        .ok_or_else(|| anyhow!("{} is not inside a git repository", start.display()))?;
    // Keys do not depend on the repo name; it only has to type-check. A repo with no
    // `.codescout/repo.toml`, or one that does not parse, gets the suggested name instead of
    // failing the gate.
    let repo = repo_name::read(&root)
        .ok()
        .flatten()
        .unwrap_or_else(|| repo_name::suggest(&root));

    let after = side(&root, Rev::Index, &repo)?;
    let before = if args.staged {
        Some(side(&root, Rev::Head, &repo)?)
    } else {
        None
    };

    let problems: Vec<&GateProblem> = match &before {
        Some(b) => after.report.new_since(&b.report),
        None => after.report.problems.iter().collect(),
    };
    let code = i32::from(!problems.is_empty());

    let mut stderr = String::new();
    for s in std::iter::once(&after).chain(before.as_ref()) {
        if s.unreadable > 0 {
            stderr.push_str(&format!(
                "key-gate: note: {} markdown path{} in {} not valid UTF-8, not checked\n",
                s.unreadable,
                if s.unreadable == 1 { " is" } else { "s are" },
                s.label
            ));
        }
    }

    let stdout = if args.json {
        render_json(
            args.staged,
            code,
            &after,
            before.as_ref(),
            &problems,
            &stderr,
        )
    } else {
        render_text(args.staged, &after, before.as_ref(), &problems)
    };
    Ok(Outcome {
        code,
        stdout,
        stderr,
    })
}

fn render_text(
    staged: bool,
    after: &Side,
    before: Option<&Side>,
    problems: &[&GateProblem],
) -> String {
    use std::fmt::Write;
    let mut s = String::new();
    let what = if staged {
        "new key problem"
    } else {
        "key problem"
    };
    let place = if staged {
        "in the staged changes (index vs HEAD)"
    } else {
        "in the index"
    };
    if problems.is_empty() {
        let _ = writeln!(s, "key-gate: OK — no {what}s {place}");
    } else {
        let n = problems.len();
        let _ = writeln!(
            s,
            "key-gate: {n} {what}{} {place}",
            if n == 1 { "" } else { "s" }
        );
        for p in problems {
            let _ = writeln!(s, "  - {}", p.render());
        }
    }
    let _ = writeln!(s, "scope: {}", after.scope());
    if let Some(b) = before {
        let _ = writeln!(s, "compared with: {}", b.scope());
    }
    if !staged {
        let outside = after.outside_with_patterns();
        let _ = writeln!(s, "outside the key space ({}):", outside.len());
        for (path, by) in outside {
            let _ = writeln!(s, "  {path}  (excluded by: {})", by.join(", "));
        }
    }
    s
}

fn render_json(
    staged: bool,
    code: i32,
    after: &Side,
    before: Option<&Side>,
    problems: &[&GateProblem],
    stderr: &str,
) -> String {
    let outside: Vec<Value> = after
        .outside_with_patterns()
        .into_iter()
        .map(|(path, by)| json!({"path": path, "excluded_by": by}))
        .collect();
    let compared_with = before.map(|b| {
        json!({
            "in_space": b.report.in_space,
            "outside_count": b.report.outside.len(),
            "key_space": b.key_space_json(),
        })
    });
    let v = json!({
        "checked": true,
        "mode": if staged { "staged" } else { "index" },
        "exit_code": code,
        "in_space": after.report.in_space,
        "outside_count": after.report.outside.len(),
        "outside": outside,
        "problems": problems.iter().map(|p| problem_json(p)).collect::<Vec<_>>(),
        "key_space": after.key_space_json(),
        "compared_with": compared_with,
        "unreadable": after.unreadable + before.map_or(0, |b| b.unreadable),
        "notes": stderr.lines().collect::<Vec<_>>(),
    });
    format!("{v}\n")
}

pub async fn run(args: KeyGateArgs) -> Result<()> {
    use std::io::Write;

    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let out = evaluate(&args, &cwd);
    let mut stdout = std::io::stdout();
    let _ = stdout.write_all(out.stdout.as_bytes());
    let _ = stdout.flush();
    eprint!("{}", out.stderr);
    if out.code != 0 {
        std::process::exit(out.code);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    /// A temp git repo. Every git call is isolated from the machine's git config through the
    /// child's environment (never `std::env::set_var`).
    struct Repo {
        dir: tempfile::TempDir,
    }

    fn git_in(dir: &Path) -> Command {
        let mut c = Command::new("git");
        c.arg("-C")
            .arg(dir)
            .args([
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "-c",
                "commit.gpgsign=false",
                "-c",
                "init.defaultBranch=main",
            ])
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .env_remove("GIT_INDEX_FILE");
        c
    }

    impl Repo {
        fn new() -> Repo {
            let repo = Repo {
                dir: tempfile::tempdir().unwrap(),
            };
            repo.git(&["init", "-q", "."]);
            repo
        }

        fn path(&self) -> &Path {
            self.dir.path()
        }

        fn git(&self, args: &[&str]) -> String {
            let out = git_in(self.path()).args(args).output().unwrap();
            assert!(
                out.status.success(),
                "git {args:?} failed: {}",
                String::from_utf8_lossy(&out.stderr)
            );
            String::from_utf8_lossy(&out.stdout).into_owned()
        }

        fn write(&self, rel: &str, text: &str) {
            let p = self.path().join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, text).unwrap();
        }

        /// Write, `git add` and (when `commit`) commit the files at once.
        fn stage(&self, files: &[(&str, &str)]) {
            for (rel, text) in files {
                self.write(rel, text);
                self.git(&["add", "--", rel]);
            }
        }

        fn commit(&self) {
            self.git(&["commit", "-q", "-m", "c"]);
        }

        fn args(&self, staged: bool, json: bool) -> KeyGateArgs {
            KeyGateArgs {
                project: Some(self.path().to_path_buf()),
                staged,
                json,
            }
        }

        fn staged(&self) -> Outcome {
            evaluate(&self.args(true, false), self.path())
        }

        fn full(&self) -> Outcome {
            evaluate(&self.args(false, false), self.path())
        }
    }

    const KS: &str = ".codescout/key-space.toml";

    #[test]
    fn a_staged_new_file_that_collides_with_an_unchanged_one_exits_1() {
        let r = Repo::new();
        r.stage(&[("docs/a/notes.md", "# a\n")]);
        r.commit();
        r.stage(&[("docs/b/notes.md", "# b\n")]);
        let out = r.staged();
        assert_eq!(out.code, 1, "{out:?}");
        assert!(out.stdout.contains("docs/a/notes.md"), "{out:?}");
        assert!(out.stdout.contains("docs/b/notes.md"), "{out:?}");
        assert!(out.stdout.contains("notes"), "{out:?}");
    }

    #[test]
    fn a_collision_between_unchanged_files_exits_0_in_staged_mode_and_1_in_full_mode() {
        let r = Repo::new();
        r.stage(&[("docs/a/notes.md", "# a\n"), ("docs/b/notes.md", "# b\n")]);
        r.commit();
        r.stage(&[("docs/c/other.md", "# c\n")]);
        let staged = r.staged();
        assert_eq!(staged.code, 0, "{staged:?}");
        assert!(
            staged.stdout.contains("3 markdown"),
            "scope named: {staged:?}"
        );
        let full = r.full();
        assert_eq!(full.code, 1, "{full:?}");
        assert!(full.stdout.contains("docs/a/notes.md"), "{full:?}");
        assert!(full.stdout.contains("docs/b/notes.md"), "{full:?}");
    }

    #[test]
    fn editing_a_file_already_in_a_collision_exits_0() {
        let r = Repo::new();
        r.stage(&[("docs/a/notes.md", "# a\n"), ("docs/b/notes.md", "# b\n")]);
        r.commit();
        r.stage(&[("docs/a/notes.md", "# a, edited\n")]);
        assert!(
            r.git(&["diff", "--cached", "--name-only"])
                .contains("docs/a/notes.md"),
            "the edit is staged"
        );
        let out = r.staged();
        assert_eq!(out.code, 0, "{out:?}");
    }

    #[test]
    fn a_commit_that_only_changes_the_key_space_file_is_judged_by_what_it_creates() {
        // Un-excluding a tree creates a collision. The working-tree copy of the key-space file
        // says "exclude contrib" again, so a gate that read it would see no problem.
        let r = Repo::new();
        r.stage(&[
            ("contrib/x.md", "# x\n"),
            ("docs/x.md", "# x\n"),
            (KS, "exclude = [\"contrib\"]\n"),
        ]);
        r.commit();
        r.stage(&[(KS, "exclude = []\n")]);
        r.write(KS, "exclude = [\"contrib\"]\n");
        let out = r.staged();
        assert_eq!(out.code, 1, "reads the staged copy: {out:?}");
        assert!(out.stdout.contains("contrib/x.md"), "{out:?}");
        assert!(out.stdout.contains("docs/x.md"), "{out:?}");

        // The same change with the working tree equal to the index: HEAD's side must still
        // come from HEAD's own copy, which excluded `contrib`, so the collision is new.
        let r = Repo::new();
        r.stage(&[
            ("contrib/x.md", "# x\n"),
            ("docs/x.md", "# x\n"),
            (KS, "exclude = [\"contrib\"]\n"),
        ]);
        r.commit();
        r.stage(&[(KS, "exclude = []\n")]);
        let out = r.staged();
        assert_eq!(out.code, 1, "reads HEAD's own copy: {out:?}");

        // The other direction: a collision at HEAD that the key-space change removes is not a
        // problem the commit creates, and the working tree (un-excluding) is not consulted.
        let r = Repo::new();
        r.stage(&[("contrib/x.md", "# x\n"), ("docs/x.md", "# x\n")]);
        r.commit();
        r.stage(&[(KS, "exclude = [\"contrib\"]\n")]);
        r.write(KS, "exclude = []\n");
        let out = r.staged();
        assert_eq!(out.code, 0, "{out:?}");
        assert!(out.stdout.contains("1 markdown"), "scope named: {out:?}");
        assert!(
            out.stdout.contains("contrib"),
            "scope names the pattern: {out:?}"
        );
    }

    #[test]
    fn the_first_commit_with_no_head_works() {
        let r = Repo::new();
        r.stage(&[("a/notes.md", "# a\n")]);
        let ok = r.staged();
        assert_eq!(ok.code, 0, "{ok:?}");
        assert!(ok.stdout.contains("1 markdown"), "{ok:?}");

        r.stage(&[("b/notes.md", "# b\n")]);
        let bad = r.staged();
        assert_eq!(bad.code, 1, "{bad:?}");
        assert!(bad.stdout.contains("a/notes.md") && bad.stdout.contains("b/notes.md"));

        // Full mode works with no HEAD too.
        assert_eq!(r.full().code, 1);
    }

    #[test]
    fn a_staged_rename_is_judged_on_the_new_path_only() {
        let r = Repo::new();
        r.stage(&[("a/foo.md", "# foo\n"), ("keep/bar.md", "# bar\n")]);
        r.commit();

        // The old key `foo` is free in the same commit: `c/foo.md` is the only holder.
        std::fs::create_dir_all(r.path().join("b")).unwrap();
        r.git(&["mv", "a/foo.md", "b/baz.md"]);
        r.stage(&[("c/foo.md", "# foo again\n")]);
        let freed = r.staged();
        assert_eq!(freed.code, 0, "{freed:?}");

        // A rename onto a key that exists elsewhere collides on the NEW path, and the old
        // path is not named.
        let r = Repo::new();
        r.stage(&[("a/foo.md", "# foo\n"), ("keep/bar.md", "# bar\n")]);
        r.commit();
        std::fs::create_dir_all(r.path().join("b")).unwrap();
        r.git(&["mv", "a/foo.md", "b/bar.md"]);
        let clash = r.staged();
        assert_eq!(clash.code, 1, "{clash:?}");
        assert!(clash.stdout.contains("b/bar.md"), "{clash:?}");
        assert!(clash.stdout.contains("keep/bar.md"), "{clash:?}");
        assert!(!clash.stdout.contains("a/foo.md"), "{clash:?}");
    }
    #[test]
    fn a_git_mv_of_a_file_in_an_existing_collision_exits_0_in_staged_mode() {
        let r = Repo::new();
        r.stage(&[
            ("a/x.md", "# a\n"),
            ("b/x.md", "# b\n"),
            ("c/x.md", "# c\n"),
        ]);
        r.commit();
        std::fs::create_dir_all(r.path().join("d")).unwrap();
        r.git(&["mv", "c/x.md", "d/x.md"]);
        let moved = r.staged();
        assert_eq!(moved.code, 0, "a pure move creates no problem: {moved:?}");
        // Control: the collision is real, and full mode still reports it with the new path.
        let full = r.full();
        assert_eq!(full.code, 1, "{full:?}");
        assert!(full.stdout.contains("d/x.md"), "{full:?}");
    }

    #[test]
    fn a_malformed_key_space_file_exits_2_and_says_not_checked() {
        let r = Repo::new();
        r.stage(&[("a/notes.md", "# a\n")]);
        r.commit();
        r.stage(&[(KS, "exclude = [\n")]);
        for out in [r.staged(), r.full()] {
            assert_eq!(out.code, 2, "{out:?}");
            assert!(out.stdout.starts_with("NOT CHECKED — "), "{out:?}");
            assert!(out.stdout.contains(KS), "names the file: {out:?}");
            assert!(out.stdout.contains("index"), "names the revision: {out:?}");
        }

        // A malformed copy at HEAD makes the HEAD side unreadable, in staged mode only.
        let r = Repo::new();
        r.stage(&[("a/notes.md", "# a\n"), (KS, "exclude = [\n")]);
        r.commit();
        r.stage(&[(KS, "exclude = []\n")]);
        let staged = r.staged();
        assert_eq!(staged.code, 2, "{staged:?}");
        assert!(staged.stdout.starts_with("NOT CHECKED — "), "{staged:?}");
        assert!(staged.stdout.contains("HEAD"), "{staged:?}");
        assert_eq!(r.full().code, 0, "full mode reads only the index");
    }

    #[test]
    fn not_a_git_repo_exits_2_and_says_not_checked() {
        let dir = tempfile::tempdir().unwrap();
        assert!(
            crate::librarian::current_project::lookup_git_root(dir.path()).is_none(),
            "the temp dir must not sit inside a git repository"
        );
        let args = KeyGateArgs {
            project: Some(dir.path().to_path_buf()),
            staged: true,
            json: false,
        };
        let out = evaluate(&args, dir.path());
        assert_eq!(out.code, 2, "{out:?}");
        assert!(out.stdout.starts_with("NOT CHECKED — "), "{out:?}");
        assert!(out.stdout.contains("git"), "{out:?}");
    }

    #[test]
    fn outside_files_are_listed_with_the_scope_that_excluded_them() {
        let r = Repo::new();
        r.stage(&[
            (KS, "exclude = [\"contrib\", \"docs/evals/**\"]\n"),
            ("contrib/pi/AGENTS.md", "# a\n"),
            ("docs/evals/run.md", "# r\n"),
            ("docs/guide.md", "# g\n"),
        ]);
        let out = r.full();
        assert_eq!(out.code, 0, "{out:?}");
        let line_of = |needle: &str| {
            out.stdout
                .lines()
                .find(|l| l.contains(needle))
                .unwrap_or_else(|| panic!("no line with {needle:?}: {out:?}"))
                .to_string()
        };
        assert!(line_of("contrib/pi/AGENTS.md").contains("contrib"));
        assert!(!line_of("contrib/pi/AGENTS.md").contains("docs/evals/**"));
        assert!(line_of("docs/evals/run.md").contains("docs/evals/**"));
        assert!(
            !out.stdout.lines().any(|l| l.contains("docs/guide.md")),
            "an inside file is not listed as outside: {out:?}"
        );
        assert!(out.stdout.contains("1 markdown"), "inside count: {out:?}");
        assert!(out.stdout.contains(KS), "names the key-space file: {out:?}");
    }

    #[test]
    fn json_output_has_counts_problems_and_outside() {
        let r = Repo::new();
        r.stage(&[
            (KS, "exclude = [\"contrib\"]\n"),
            ("contrib/x.md", "# c\n"),
            ("a/notes.md", "# a\n"),
            ("b/notes.md", "# b\n"),
        ]);
        let out = evaluate(&r.args(false, true), r.path());
        assert_eq!(out.code, 1, "{out:?}");
        let v: serde_json::Value = serde_json::from_str(&out.stdout).expect("stdout is JSON");
        assert_eq!(v["checked"], true);
        assert_eq!(v["exit_code"], 1);
        assert_eq!(v["in_space"], 2);
        assert_eq!(v["outside_count"], 1);
        assert_eq!(v["outside"][0]["path"], "contrib/x.md");
        assert_eq!(v["outside"][0]["excluded_by"][0], "contrib");
        assert_eq!(v["problems"].as_array().unwrap().len(), 1);
        assert_eq!(v["problems"][0]["kind"], "duplicate_key");
        assert_eq!(v["problems"][0]["key"], "notes");
        assert_eq!(v["problems"][0]["paths"][0], "a/notes.md");
        assert_eq!(v["key_space"]["patterns"][0], "contrib");

        // Staged mode: only the new problems, and the same shape.
        let out = evaluate(&r.args(true, true), r.path());
        let v: serde_json::Value = serde_json::from_str(&out.stdout).unwrap();
        assert_eq!(v["mode"], "staged");
        assert_eq!(v["problems"].as_array().unwrap().len(), 1);

        // Cannot ask: JSON too.
        r.stage(&[(KS, "exclude = [\n")]);
        let out = evaluate(&r.args(false, true), r.path());
        assert_eq!(out.code, 2);
        let v: serde_json::Value = serde_json::from_str(&out.stdout).unwrap();
        assert_eq!(v["checked"], false);
        assert!(v["not_checked"].as_str().unwrap().contains(KS));
    }

    #[test]
    fn a_path_with_spaces_quotes_and_non_ascii_survives() {
        let r = Repo::new();
        r.stage(&[
            ("my docs/a \"quoted\" note.md", "# a\n"),
            ("dokumentacja/zażółć.md", "# z\n"),
            ("other/a \"quoted\" note.md", "# b\n"),
        ]);
        // Each path gives a non-canonical key. Git's own output would quote and octal-escape
        // them; with `-z` all three arrive intact, so there are exactly three problems and
        // each message names its path.
        let out = evaluate(&r.args(false, true), r.path());
        assert_eq!(out.code, 1, "{out:?}");
        let v: serde_json::Value = serde_json::from_str(&out.stdout).unwrap();
        assert_eq!(v["in_space"], 3);
        let messages: Vec<&str> = v["problems"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["message"].as_str().unwrap())
            .collect();
        assert_eq!(messages.len(), 3, "{messages:?}");
        for needle in ["my docs/a ", "dokumentacja/zażółć.md", "other/a "] {
            assert!(
                messages.iter().any(|m| m.contains(needle)),
                "no message names {needle:?}: {messages:?}"
            );
        }
    }

    #[test]
    fn an_uppercase_md_extension_is_a_markdown_file() {
        let r = Repo::new();
        r.stage(&[("a/notes.md", "# a\n"), ("b/NOTES.MD", "# b\n")]);
        let out = r.full();
        assert_eq!(out.code, 1, "{out:?}");
        assert!(out.stdout.contains("b/NOTES.MD"), "{out:?}");
        assert!(out.stdout.contains("2 markdown"), "{out:?}");
    }

    #[cfg(unix)]
    #[test]
    fn a_non_utf8_path_is_skipped_and_counted_not_fatal() {
        use std::os::unix::ffi::OsStrExt;
        let r = Repo::new();
        r.stage(&[("a/notes.md", "# a\n")]);
        let odd = std::ffi::OsStr::from_bytes(b"b/caf\xe9.md");
        std::fs::create_dir_all(r.path().join("b")).unwrap();
        std::fs::write(r.path().join(odd), "# odd\n").unwrap();
        let add = git_in(r.path())
            .arg("add")
            .arg("--")
            .arg(odd)
            .output()
            .unwrap();
        assert!(add.status.success());
        let out = r.full();
        assert_eq!(out.code, 0, "{out:?}");
        assert!(out.stdout.contains("1 markdown"), "{out:?}");
        assert!(
            out.stderr.contains("1 ") && out.stderr.contains("UTF-8"),
            "the skipped path is counted on stderr: {out:?}"
        );
    }
}
