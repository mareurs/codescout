//! Other checkouts on this machine that use a repo name.
//!
//! A scan walks the given roots, finds projects (a directory holding `.codescout/project.toml`),
//! and for each repository whose `.codescout/repo.toml` names the same repo name tells a clone of
//! this repository (the same repo: fine) from an unrelated repository that picked the same name
//! (a collision). The two are told apart by root commits: a clone shares at least one parentless
//! commit with its source. The walk is bounded (depth, directory count, wall clock) and a result
//! always says what was examined.
//!
//! A shared root commit is evidence, not proof. A repository that vendors another's history shares
//! its root commits and is told `Same` although it is a different project, and a history rewrite
//! gives a real clone no shared root, so it is told `Collision`. A caller must not present
//! `same_repo` as a certainty.
//!
//! A project is judged against the repository found at or below the scan root that holds it. A
//! project with no `.git` of its own inside the scan is judged as itself, never as a repository
//! above the scan root.

use std::collections::{BTreeSet, VecDeque};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::Context;

use super::repo_name::{self, RepoName};

/// The bounds of one scan.
#[derive(Debug, Clone)]
pub struct ScanLimits {
    /// Directories deeper than this below a root are not entered (the root is depth 0).
    pub max_depth: usize,
    /// At most this many directories are visited.
    pub max_dirs: usize,
    /// The scan stops after this much wall-clock time.
    pub deadline: Duration,
}

impl Default for ScanLimits {
    fn default() -> Self {
        ScanLimits {
            max_depth: 3,
            max_dirs: 20_000,
            deadline: Duration::from_secs(2),
        }
    }
}

/// The strongest bound a scan hit. `Deadline` and `Dirs` outrank `Depth`, because they mean even
/// the depth-bounded set was not fully examined; between `Dirs` and `Deadline` the first one
/// hit stays.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bound {
    Depth,
    Dirs,
    Deadline,
}

/// What a scan found and what it examined.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScanReport {
    pub roots: Vec<PathBuf>,
    pub dirs_visited: usize,
    pub bounded_by: Option<Bound>,
    pub same_repo: Vec<PathBuf>,
    pub collisions: Vec<PathBuf>,
    pub unverified: Vec<PathBuf>,
    pub unreadable: Vec<(PathBuf, String)>,
}

/// The sorted ids of every parentless commit reachable from HEAD: a repository with several root
/// commits returns several, and an unborn HEAD returns an empty list. A shallow repository's
/// boundary commit counts as parentless; [`scan`] checks `is_shallow` before it trusts the list.
pub fn first_commits(git_root: &Path) -> anyhow::Result<Vec<String>> {
    // Without a deadline the walk always completes.
    Ok(first_commits_until(git_root, &|| false)?.unwrap_or_default())
}

/// How many commits the walk takes between two looks at the clock.
const CLOCK_EVERY: usize = 512;

/// [`first_commits`] that gives up once `expired()`: `Ok(None)` is "ran out of time", never a partial
/// list (a partial root set would read as a collision).
fn first_commits_until(
    git_root: &Path,
    expired: &dyn Fn() -> bool,
) -> anyhow::Result<Option<Vec<String>>> {
    let repo = git2::Repository::open(git_root)
        .with_context(|| format!("opening the repository at {}", git_root.display()))?;
    first_commits_in(&repo, expired)
}

/// [`first_commits_until`] on a repository that is already open.
fn first_commits_in(
    repo: &git2::Repository,
    expired: &dyn Fn() -> bool,
) -> anyhow::Result<Option<Vec<String>>> {
    let mut walk = repo.revwalk()?;
    // `push_head` on an unborn branch reports a generic error, not a code to match on; `head`
    // reports `UnbornBranch`.
    match repo.head() {
        Ok(_) => {}
        Err(e) if e.code() == git2::ErrorCode::UnbornBranch => return Ok(Some(Vec::new())),
        Err(e) => return Err(e.into()),
    }
    walk.push_head()?;
    let mut roots = Vec::new();
    for (i, oid) in walk.enumerate() {
        if i % CLOCK_EVERY == 0 && expired() {
            return Ok(None);
        }
        let oid = oid?;
        if repo.find_commit(oid)?.parent_count() == 0 {
            roots.push(oid.to_string());
        }
    }
    roots.sort();
    roots.dedup();
    Ok(Some(roots))
}

/// What one side of a comparison offers.
enum Inspected {
    /// A shallow repository: its boundary commit looks parentless, so its root set proves nothing.
    Shallow,
    /// Not a git repository at all: there is no history to compare.
    NoHistory,
    /// The sorted root commits (empty for an unborn HEAD).
    Roots(Vec<String>),
    /// The walk did not finish before the deadline.
    TimedOut,
}

fn inspect(git_root: &Path, expired: &dyn Fn() -> bool) -> anyhow::Result<Inspected> {
    let repo = match git2::Repository::open(git_root) {
        Ok(repo) => repo,
        // A project that is not in a git repository has no history to compare.
        Err(e) if e.code() == git2::ErrorCode::NotFound => return Ok(Inspected::NoHistory),
        Err(e) => {
            return Err(anyhow::Error::new(e))
                .with_context(|| format!("opening the repository at {}", git_root.display()));
        }
    };
    if repo.is_shallow() {
        return Ok(Inspected::Shallow);
    }
    Ok(match first_commits_in(&repo, expired)? {
        Some(roots) => Inspected::Roots(roots),
        None => Inspected::TimedOut,
    })
}

/// What a same-named repository is, relative to this one.
enum Verdict {
    Same,
    Collision,
    Unverified,
    TimedOut,
}

fn compare(this: &Inspected, other: &Inspected) -> Verdict {
    match (this, other) {
        (Inspected::TimedOut, _) | (_, Inspected::TimedOut) => Verdict::TimedOut,
        (Inspected::Shallow | Inspected::NoHistory, _)
        | (_, Inspected::Shallow | Inspected::NoHistory) => Verdict::Unverified,
        // Nothing to compare yet: an unborn repository neither shares nor contradicts.
        (Inspected::Roots(a), Inspected::Roots(b)) if a.is_empty() || b.is_empty() => {
            Verdict::Unverified
        }
        (Inspected::Roots(a), Inspected::Roots(b)) => {
            if a.iter().any(|id| b.binary_search(id).is_ok()) {
                Verdict::Same
            } else {
                Verdict::Collision
            }
        }
    }
}

fn canonical(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn is_absent(e: &std::io::Error) -> bool {
    matches!(
        e.kind(),
        std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
    )
}

/// The mutable state of one scan.
struct Scan<'a> {
    name: &'a RepoName,
    this_git_root: &'a Path,
    this_canonical: PathBuf,
    /// `None` until a same-named candidate needs it; computed once.
    this_side: Option<Result<Inspected, String>>,
    /// Whether the deadline has passed.
    expired: &'a dyn Fn() -> bool,
    /// Canonical git roots already judged, so a monorepo with several projects counts once.
    judged: BTreeSet<PathBuf>,
    report: ScanReport,
}

impl Scan<'_> {
    /// Record a bound. `Dirs` and `Deadline` mean even the depth-bounded set was not fully
    /// examined, so they replace a recorded `Depth`; between those two, and for `Depth` itself,
    /// the first one hit stays.
    fn note(&mut self, bound: Bound) {
        match (self.report.bounded_by, bound) {
            (None, _) => self.report.bounded_by = Some(bound),
            (Some(Bound::Depth), Bound::Dirs | Bound::Deadline) => {
                self.report.bounded_by = Some(bound);
            }
            (Some(_), _) => {}
        }
    }

    /// Judge the project at `dir`, when `dir` is one. `top` is the scan root that holds `dir`:
    /// the search for the project's git root stops there, so a git-less project is not read as
    /// the repository (a dotfiles repo in `$HOME`, say) that happens to sit above the scan.
    fn visit_project(&mut self, dir: &Path, top: &Path) {
        let project_file = dir.join(".codescout/project.toml");
        // Existence only: the file is never read.
        match std::fs::metadata(&project_file) {
            Ok(m) if m.is_file() => {}
            Ok(_) => return,
            Err(e) if is_absent(&e) => return,
            Err(e) => {
                self.report.unreadable.push((project_file, e.to_string()));
                return;
            }
        }
        let git_root = git_root_within(dir, top).unwrap_or_else(|| dir.to_path_buf());
        let git_canonical = canonical(&git_root);
        if git_canonical == self.this_canonical || !self.judged.insert(git_canonical) {
            return;
        }
        match repo_name::read(&git_root) {
            Err(e) => self.report.unreadable.push((git_root, format!("{e:#}"))),
            Ok(Some(found)) if &found == self.name => self.judge(git_root),
            Ok(_) => {}
        }
    }

    /// `git_root` uses this repo's name: a clone, a collision, or not decidable.
    fn judge(&mut self, git_root: PathBuf) {
        let (this_root, expired) = (self.this_git_root, self.expired);
        let this = self
            .this_side
            .get_or_insert_with(|| inspect(this_root, expired).map_err(|e| format!("{e:#}")));
        let this = match this {
            Ok(this) => this,
            Err(e) => {
                let why = format!("this repository could not be inspected: {e}");
                self.report.unreadable.push((git_root, why));
                return;
            }
        };
        let other = match inspect(&git_root, expired) {
            Ok(other) => other,
            Err(e) => {
                self.report.unreadable.push((git_root, format!("{e:#}")));
                return;
            }
        };
        match compare(this, &other) {
            Verdict::Same => self.report.same_repo.push(git_root),
            Verdict::Collision => self.report.collisions.push(git_root),
            Verdict::Unverified => self.report.unverified.push(git_root),
            Verdict::TimedOut => {
                self.note(Bound::Deadline);
                self.report.unverified.push(git_root);
            }
        }
    }
}

/// Find the checkouts under `roots` that use `name`.
///
/// Breadth first, so a bound leaves the nearest directories examined. A directory is entered once
/// (by canonical path), symbolic links are never followed, and hidden directories (`.git`,
/// `.cache`, `.codescout` itself, ...) are not entered; a project is recognised by
/// `<dir>/.codescout/project.toml`, which needs no descent. Every vector of the report is sorted
/// and free of duplicates.
pub fn scan(
    name: &RepoName,
    this_git_root: &Path,
    roots: &[PathBuf],
    limits: &ScanLimits,
) -> ScanReport {
    let started = Instant::now();
    scan_with(name, this_git_root, roots, limits, &|| {
        started.elapsed() >= limits.deadline
    })
}

/// Directories a scan never enters, by name: dependency and build output hold no checkout of the
/// user's, only vendored or generated trees that can be very large.
const SKIPPED_DIRS: [&str; 2] = ["node_modules", "target"];

/// The nearest directory at or above `dir` that holds `.git`, looking no higher than `top`.
/// `None` when there is none: the project owns no repository of its own inside the scan.
fn git_root_within(dir: &Path, top: &Path) -> Option<PathBuf> {
    dir.ancestors()
        .take_while(|a| a.starts_with(top))
        .find(|a| a.join(".git").exists())
        .map(Path::to_path_buf)
}

/// [`scan`] with the clock injected: `expired()` is asked once per directory and every few
/// hundred commits of a root-commit walk. A test drives it by call count instead of by sleeping.
fn scan_with(
    name: &RepoName,
    this_git_root: &Path,
    roots: &[PathBuf],
    limits: &ScanLimits,
    expired: &dyn Fn() -> bool,
) -> ScanReport {
    let mut given: Vec<PathBuf> = roots.to_vec();
    given.sort();
    given.dedup();
    let mut state = Scan {
        name,
        this_git_root,
        this_canonical: canonical(this_git_root),
        this_side: None,
        expired,
        judged: BTreeSet::new(),
        report: ScanReport {
            roots: given.clone(),
            ..ScanReport::default()
        },
    };

    // (path as the caller spelled it, canonical path, depth below its root, that root as spelled)
    let mut queue: VecDeque<(PathBuf, PathBuf, usize, PathBuf)> = VecDeque::new();
    for root in &given {
        match std::fs::canonicalize(root) {
            Ok(canon) => queue.push_back((root.clone(), canon, 0, root.clone())),
            Err(e) => state.report.unreadable.push((root.clone(), e.to_string())),
        }
    }
    let mut visited: BTreeSet<PathBuf> = BTreeSet::new();
    while let Some((shown, canon, depth, top)) = queue.pop_front() {
        if !visited.insert(canon.clone()) {
            continue;
        }
        if state.report.dirs_visited >= limits.max_dirs {
            state.note(Bound::Dirs);
            break;
        }
        if expired() {
            state.note(Bound::Deadline);
            break;
        }
        state.report.dirs_visited += 1;
        state.visit_project(&shown, &top);

        let entries = match std::fs::read_dir(&shown) {
            Ok(entries) => entries,
            Err(e) => {
                state.report.unreadable.push((shown, e.to_string()));
                continue;
            }
        };
        let mut children: Vec<std::ffi::OsString> = Vec::new();
        for entry in entries {
            match entry {
                // `file_type` does not follow a link: a symlink is never a directory here.
                Ok(entry) => match entry.file_type() {
                    Ok(t) if t.is_dir() => {
                        let child = entry.file_name();
                        if !child.to_string_lossy().starts_with('.')
                            && !SKIPPED_DIRS.contains(&child.to_string_lossy().as_ref())
                        {
                            children.push(child);
                        }
                    }
                    Ok(_) => {}
                    Err(e) => state
                        .report
                        .unreadable
                        .push((shown.join(entry.file_name()), e.to_string())),
                },
                Err(e) => state.report.unreadable.push((shown.clone(), e.to_string())),
            }
        }
        children.sort();
        if depth >= limits.max_depth {
            if !children.is_empty() {
                state.note(Bound::Depth);
            }
            continue;
        }
        for child in children {
            queue.push_back((
                shown.join(&child),
                canon.join(&child),
                depth + 1,
                top.clone(),
            ));
        }
    }

    let mut report = state.report;
    report.same_repo.sort();
    report.same_repo.dedup();
    report.collisions.sort();
    report.collisions.dedup();
    report.unverified.sort();
    report.unverified.dedup();
    report.unreadable.sort();
    report.unreadable.dedup();
    report
}

/// One line naming the roots, the directories visited and any bound hit, so that a result of
/// "nothing found" says what was examined.
pub fn render_scope(report: &ScanReport) -> String {
    let roots = if report.roots.is_empty() {
        "no roots".to_string()
    } else {
        report
            .roots
            .iter()
            .map(|r| r.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    };
    let bound = match report.bounded_by {
        None => "no bound was hit",
        Some(Bound::Depth) => "not exhaustive: directories below the depth limit were not entered",
        Some(Bound::Dirs) => {
            "not exhaustive: the directory limit stopped the scan, so some directories were not examined"
        }
        Some(Bound::Deadline) => {
            "not exhaustive: time ran out, so some directories or repositories were not examined"
        }
    };
    let visited = match report.dirs_visited {
        1 => "1 directory visited".to_string(),
        n => format!("{n} directories visited"),
    };
    format!("scanned {roots}: {visited}; {bound}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn git_cmd(dir: &Path) -> Command {
        let mut c = Command::new("git");
        c.current_dir(dir)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .env_remove("GIT_INDEX_FILE")
            .args([
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "-c",
                "commit.gpgsign=false",
            ]);
        c
    }

    fn git(dir: &Path, args: &[&str]) -> String {
        let out = git_cmd(dir).args(args).output().unwrap();
        assert!(
            out.status.success(),
            "git {args:?} in {}: {}",
            dir.display(),
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap()
    }

    /// A git repo at `dir` with a project file, a repo file naming `name` (none when `None`) and a
    /// `marker.txt`. The marker makes the root commit differ between two repos built with the same
    /// author and the same second: identical trees would give identical commit ids.
    fn make_repo(dir: &Path, name: Option<&str>, marker: &str) {
        std::fs::create_dir_all(dir.join(".codescout")).unwrap();
        git(dir, &["init", "-q", "-b", "main"]);
        std::fs::write(dir.join(".codescout/project.toml"), "").unwrap();
        if let Some(n) = name {
            std::fs::write(
                dir.join(".codescout/repo.toml"),
                format!("name = \"{n}\"\n"),
            )
            .unwrap();
        }
        std::fs::write(dir.join("marker.txt"), marker).unwrap();
        git(dir, &["add", "-A"]);
        git(dir, &["commit", "-q", "-m", "init"]);
    }

    fn commit_file(dir: &Path, file: &str, text: &str) {
        std::fs::write(dir.join(file), text).unwrap();
        git(dir, &["add", "-A"]);
        git(dir, &["commit", "-q", "-m", file]);
    }

    fn clone(src: &Path, dst: &Path, extra: &[&str]) {
        std::fs::create_dir_all(dst.parent().unwrap()).unwrap();
        let mut args = vec!["clone", "-q"];
        args.extend_from_slice(extra);
        let src_s = src.to_str().unwrap();
        let src_arg = if extra.contains(&"--depth") {
            format!("file://{src_s}")
        } else {
            src_s.to_string()
        };
        args.push(&src_arg);
        args.push(dst.to_str().unwrap());
        git(dst.parent().unwrap(), &args);
    }

    /// Give `dir` a second, unrelated root commit and merge it in.
    fn merge_in_an_unrelated_root(dir: &Path) {
        git(dir, &["checkout", "-q", "--orphan", "other"]);
        git(dir, &["rm", "-rfq", "."]);
        git(dir, &["commit", "-q", "--allow-empty", "-m", "other root"]);
        git(dir, &["checkout", "-q", "main"]);
        git(
            dir,
            &[
                "merge",
                "-q",
                "other",
                "--allow-unrelated-histories",
                "-m",
                "merge",
            ],
        );
    }

    fn n(s: &str) -> RepoName {
        RepoName::parse(s).unwrap()
    }

    fn scan_default(name: &str, this: &Path, roots: &[&Path]) -> ScanReport {
        let roots: Vec<PathBuf> = roots.iter().map(|p| p.to_path_buf()).collect();
        scan(&n(name), this, &roots, &ScanLimits::default())
    }

    #[test]
    fn a_clone_of_this_repo_is_the_same_repo_not_a_collision() {
        let tmp = tempfile::tempdir().unwrap();
        let this = tmp.path().join("this");
        make_repo(&this, Some("alpha"), "a");
        let copy = tmp.path().join("work/copy");
        clone(&this, &copy, &[]);
        // `this` is reached through the root too and must not count as another checkout.
        let r = scan_default("alpha", &this, &[tmp.path()]);
        assert_eq!(r.same_repo, vec![copy], "{r:?}");
        assert!(r.collisions.is_empty(), "{r:?}");
        assert!(r.unverified.is_empty(), "{r:?}");
        assert!(r.unreadable.is_empty(), "{r:?}");
        assert_eq!(r.bounded_by, None, "{r:?}");
    }

    #[test]
    fn an_independent_repo_with_the_same_name_is_a_collision() {
        let tmp = tempfile::tempdir().unwrap();
        let this = tmp.path().join("this");
        make_repo(&this, Some("alpha"), "a");
        let other = tmp.path().join("work/other");
        make_repo(&other, Some("alpha"), "b");
        let r = scan_default("alpha", &this, &[tmp.path()]);
        assert_eq!(r.collisions, vec![other], "{r:?}");
        assert!(r.same_repo.is_empty(), "{r:?}");
        assert!(r.unverified.is_empty(), "{r:?}");
    }

    #[test]
    fn a_project_with_another_name_is_ignored() {
        let tmp = tempfile::tempdir().unwrap();
        let this = tmp.path().join("this");
        make_repo(&this, Some("alpha"), "a");
        make_repo(&tmp.path().join("work/beta"), Some("beta"), "b");
        make_repo(&tmp.path().join("work/nameless"), None, "c");
        let r = scan_default("alpha", &this, &[tmp.path()]);
        assert!(r.same_repo.is_empty(), "{r:?}");
        assert!(r.collisions.is_empty(), "{r:?}");
        assert!(r.unverified.is_empty(), "{r:?}");
        assert!(r.unreadable.is_empty(), "{r:?}");
        assert!(r.dirs_visited >= 3, "the scan must have looked: {r:?}");
    }

    #[test]
    fn a_directory_deeper_than_max_depth_sets_bounded_by_depth() {
        let tmp = tempfile::tempdir().unwrap();
        let this = tmp.path().join("this");
        make_repo(&this, Some("alpha"), "a");
        let deep = tmp.path().join("scan/a/b/c/other");
        make_repo(&deep, Some("alpha"), "b");
        let roots = vec![tmp.path().join("scan")];
        let shallow_limits = ScanLimits {
            max_depth: 2,
            ..ScanLimits::default()
        };
        let r = scan(&n("alpha"), &this, &roots, &shallow_limits);
        assert_eq!(r.bounded_by, Some(Bound::Depth), "{r:?}");
        assert!(r.collisions.is_empty(), "{r:?}");
        // With room for it the same repo is found and no bound is hit.
        let roomy = ScanLimits {
            max_depth: 4,
            ..ScanLimits::default()
        };
        let r = scan(&n("alpha"), &this, &roots, &roomy);
        assert_eq!(r.collisions, vec![deep], "{r:?}");
        assert_eq!(r.bounded_by, None, "{r:?}");
    }

    #[test]
    fn the_directory_bound_stops_the_scan_and_is_recorded() {
        let tmp = tempfile::tempdir().unwrap();
        let this = tmp.path().join("this");
        make_repo(&this, Some("alpha"), "a");
        for d in ["d1", "d2", "d3", "d4"] {
            std::fs::create_dir_all(tmp.path().join("scan").join(d)).unwrap();
        }
        let limits = ScanLimits {
            max_dirs: 3,
            ..ScanLimits::default()
        };
        let r = scan(&n("alpha"), &this, &[tmp.path().join("scan")], &limits);
        assert_eq!(r.bounded_by, Some(Bound::Dirs), "{r:?}");
        assert_eq!(r.dirs_visited, 3, "{r:?}");
    }

    #[test]
    fn an_expired_deadline_stops_the_scan_and_is_recorded() {
        let tmp = tempfile::tempdir().unwrap();
        let this = tmp.path().join("this");
        make_repo(&this, Some("alpha"), "a");
        make_repo(&tmp.path().join("scan/other"), Some("alpha"), "b");
        let limits = ScanLimits {
            deadline: Duration::ZERO,
            ..ScanLimits::default()
        };
        let r = scan(&n("alpha"), &this, &[tmp.path().join("scan")], &limits);
        assert_eq!(r.bounded_by, Some(Bound::Deadline), "{r:?}");
        assert_eq!(r.dirs_visited, 0, "{r:?}");
        assert!(r.collisions.is_empty(), "{r:?}");
    }

    #[test]
    fn an_unreadable_repo_file_is_listed_not_skipped() {
        let tmp = tempfile::tempdir().unwrap();
        let this = tmp.path().join("this");
        make_repo(&this, Some("alpha"), "a");
        let broken = tmp.path().join("work/broken");
        make_repo(&broken, Some("alpha"), "b");
        std::fs::write(broken.join(".codescout/repo.toml"), "name = [not toml").unwrap();
        let r = scan_default("alpha", &this, &[tmp.path()]);
        assert_eq!(r.unreadable.len(), 1, "{r:?}");
        assert_eq!(r.unreadable[0].0, broken, "{r:?}");
        assert!(r.unreadable[0].1.contains("repo.toml"), "{r:?}");
        assert!(r.collisions.is_empty() && r.same_repo.is_empty(), "{r:?}");
    }

    #[test]
    fn a_shallow_clone_is_unverified_not_a_collision() {
        let tmp = tempfile::tempdir().unwrap();
        let this = tmp.path().join("this");
        make_repo(&this, Some("alpha"), "a");
        commit_file(&this, "two.txt", "2");
        // A shallow clone's boundary commit looks parentless: its root set is {c2}, disjoint from
        // this repo's {c1}.
        let shallow = tmp.path().join("work/shallow");
        clone(&this, &shallow, &["--depth", "1"]);
        let r = scan_default("alpha", &this, &[tmp.path()]);
        assert_eq!(r.unverified, vec![shallow.clone()], "{r:?}");
        assert!(r.collisions.is_empty() && r.same_repo.is_empty(), "{r:?}");

        // The same holds when THIS repository is the shallow one.
        let r = scan_default("alpha", &shallow, &[&tmp.path().join("this")]);
        assert_eq!(r.unverified, vec![this], "{r:?}");
        assert!(r.collisions.is_empty() && r.same_repo.is_empty(), "{r:?}");
    }

    #[test]
    fn two_repos_sharing_one_root_commit_are_the_same_repo() {
        let tmp = tempfile::tempdir().unwrap();
        let this = tmp.path().join("this");
        make_repo(&this, Some("alpha"), "a");
        let copy = tmp.path().join("work/copy");
        clone(&this, &copy, &[]);
        merge_in_an_unrelated_root(&copy);
        // {r1} against {r1, r2}: sharing one is enough, in both directions.
        let r = scan_default("alpha", &this, &[&tmp.path().join("work")]);
        assert_eq!(r.same_repo, vec![copy.clone()], "{r:?}");
        assert!(r.collisions.is_empty(), "{r:?}");
        let r = scan_default("alpha", &copy, &[&tmp.path().join("this")]);
        assert_eq!(r.same_repo, vec![this], "{r:?}");
        assert!(r.collisions.is_empty(), "{r:?}");
    }

    #[test]
    fn first_commits_returns_every_root_commit() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("repo");
        make_repo(&repo, Some("alpha"), "a");
        commit_file(&repo, "two.txt", "2");
        let one = first_commits(&repo).unwrap();
        assert_eq!(one.len(), 1, "{one:?}");
        merge_in_an_unrelated_root(&repo);
        let two = first_commits(&repo).unwrap();
        let mut expected: Vec<String> = git(&repo, &["rev-list", "--max-parents=0", "HEAD"])
            .lines()
            .map(str::to_string)
            .collect();
        expected.sort();
        assert_eq!(expected.len(), 2);
        assert_eq!(two, expected);
        assert!(two.contains(&one[0]));
    }

    #[test]
    fn first_commits_of_an_unborn_head_is_empty() {
        let tmp = tempfile::tempdir().unwrap();
        git(tmp.path(), &["init", "-q", "-b", "main"]);
        assert_eq!(first_commits(tmp.path()).unwrap(), Vec::<String>::new());
    }

    #[test]
    fn a_nested_project_resolves_to_its_git_root_and_a_repo_counts_once() {
        let tmp = tempfile::tempdir().unwrap();
        let this = tmp.path().join("this");
        make_repo(&this, Some("alpha"), "a");
        // A monorepo: the repo file is at the git root, which is NOT a project itself; only two
        // nested projects below it are. Found through either, the repo must be the git root, read
        // there; with the project directory used instead, `svc1` and `svc2` hold no repo file.
        let mono = tmp.path().join("work/mono");
        make_repo(&mono, Some("alpha"), "b");
        std::fs::remove_file(mono.join(".codescout/project.toml")).unwrap();
        for p in ["svc1", "svc2"] {
            std::fs::create_dir_all(mono.join(p).join(".codescout")).unwrap();
            std::fs::write(mono.join(p).join(".codescout/project.toml"), "").unwrap();
        }
        let r = scan_default("alpha", &this, &[tmp.path()]);
        assert_eq!(r.collisions, vec![mono], "{r:?}");
        assert!(r.unreadable.is_empty(), "{r:?}");
    }
    /// `scan` sorts and dedups every vector at the end, so a repo judged twice cannot be seen in
    /// its report. This drives `visit_project` directly and reads the vector before that cleanup.
    #[test]
    fn two_projects_in_one_repo_are_judged_once() {
        let tmp = tempfile::tempdir().unwrap();
        let this = tmp.path().join("this");
        make_repo(&this, Some("alpha"), "a");
        let mono = tmp.path().join("work/mono");
        make_repo(&mono, Some("alpha"), "b");
        std::fs::remove_file(mono.join(".codescout/project.toml")).unwrap();
        for p in ["svc1", "svc2"] {
            std::fs::create_dir_all(mono.join(p).join(".codescout")).unwrap();
            std::fs::write(mono.join(p).join(".codescout/project.toml"), "").unwrap();
        }
        let name = n("alpha");
        let never = || false;
        let mut state = Scan {
            name: &name,
            this_git_root: &this,
            this_canonical: canonical(&this),
            this_side: None,
            expired: &never,
            judged: BTreeSet::new(),
            report: ScanReport::default(),
        };
        state.visit_project(&mono.join("svc1"), tmp.path());
        state.visit_project(&mono.join("svc2"), tmp.path());
        assert_eq!(state.report.collisions, vec![mono], "{:?}", state.report);
    }

    #[test]
    fn the_directory_limit_outranks_the_depth_limit_in_the_scope() {
        let tmp = tempfile::tempdir().unwrap();
        let this = tmp.path().join("this");
        make_repo(&this, Some("alpha"), "a");
        let root = tmp.path().join("scan");
        for d in ["x", "y"] {
            std::fs::create_dir_all(root.join(d).join("sub")).unwrap();
        }
        // `z` is reached after the limit: it is not examined, and the scope must say so rather
        // than blame depth (x and y, visited first, already hit the depth limit).
        let z = root.join("z");
        make_repo(&z, Some("alpha"), "b");
        std::fs::create_dir_all(z.join("sub")).unwrap();
        let limits = ScanLimits {
            max_depth: 1,
            max_dirs: 3,
            ..ScanLimits::default()
        };
        let r = scan(&n("alpha"), &this, &[root], &limits);
        assert_eq!(r.dirs_visited, 3, "{r:?}");
        assert!(r.collisions.is_empty(), "z was never examined: {r:?}");
        assert_eq!(r.bounded_by, Some(Bound::Dirs), "{r:?}");
    }

    #[test]
    fn a_root_commit_walk_timeout_outranks_the_depth_limit_in_the_scope() {
        let tmp = tempfile::tempdir().unwrap();
        let this = tmp.path().join("this");
        make_repo(&this, Some("alpha"), "a");
        let root = tmp.path().join("scan");
        std::fs::create_dir_all(root.join("a/sub")).unwrap();
        let z = root.join("z");
        make_repo(&z, Some("alpha"), "b");
        // `z` has a directory below the depth limit too, so the notes arrive in the order Depth
        // (`a`), Deadline (the walk in `z`), Depth (`z`'s own children): the last must not win.
        std::fs::create_dir_all(z.join("sub")).unwrap();
        let limits = ScanLimits {
            max_depth: 1,
            ..ScanLimits::default()
        };
        // No sleeping: the injected clock reads "expired" from its 4th question on. The scan asks
        // once per directory (`scan`, `a`, `z`: three, and `a` hits the depth limit), then the
        // first commit of this repository's root-commit walk asks the 4th time.
        let asked = std::cell::Cell::new(0usize);
        let expired = || {
            asked.set(asked.get() + 1);
            asked.get() > 3
        };
        let r = scan_with(&n("alpha"), &this, &[root], &limits, &expired);
        assert_eq!(
            r.dirs_visited, 3,
            "the timeout came after the directories: {r:?}"
        );
        assert_eq!(r.unverified, vec![z], "the walk timed out on z: {r:?}");
        assert_eq!(r.bounded_by, Some(Bound::Deadline), "{r:?}");
    }

    #[test]
    fn overlapping_roots_visit_a_directory_once_and_report_sorted_deduped_vectors() {
        let tmp = tempfile::tempdir().unwrap();
        let this = tmp.path().join("this");
        make_repo(&this, Some("alpha"), "a");
        let o1 = tmp.path().join("work/o1");
        let o2 = tmp.path().join("work/o2");
        make_repo(&o2, Some("alpha"), "b");
        make_repo(&o1, Some("alpha"), "c");
        let once = scan_default("alpha", &this, &[tmp.path()]);
        let twice = scan_default(
            "alpha",
            &this,
            &[
                &tmp.path().join("work"),
                tmp.path(),
                &tmp.path().join("work"),
            ],
        );
        assert_eq!(twice.collisions, vec![o1, o2], "{twice:?}");
        assert_eq!(twice.dirs_visited, once.dirs_visited, "{twice:?} {once:?}");
        assert_eq!(twice.roots.len(), 2, "{twice:?}");
    }

    #[test]
    fn hidden_directories_are_not_scanned() {
        let tmp = tempfile::tempdir().unwrap();
        let this = tmp.path().join("this");
        make_repo(&this, Some("alpha"), "a");
        make_repo(&tmp.path().join("scan/.cache/other"), Some("alpha"), "b");
        let r = scan_default("alpha", &this, &[&tmp.path().join("scan")]);
        assert!(r.collisions.is_empty(), "{r:?}");
        assert_eq!(r.dirs_visited, 1, "only the root: {r:?}");
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_are_not_followed_so_a_loop_and_an_outside_repo_stay_out() {
        let tmp = tempfile::tempdir().unwrap();
        let this = tmp.path().join("this");
        make_repo(&this, Some("alpha"), "a");
        let outside = tmp.path().join("outside/other");
        make_repo(&outside, Some("alpha"), "b");
        let scan_root = tmp.path().join("scan");
        std::fs::create_dir_all(scan_root.join("a")).unwrap();
        std::os::unix::fs::symlink(&scan_root, scan_root.join("a/loop")).unwrap();
        std::os::unix::fs::symlink(tmp.path().join("outside"), scan_root.join("out")).unwrap();
        let r = scan_default("alpha", &this, &[&scan_root]);
        assert!(r.collisions.is_empty(), "{r:?}");
        assert_eq!(r.dirs_visited, 2, "the root and `a`: {r:?}");
        assert_eq!(r.bounded_by, None, "{r:?}");
    }

    #[test]
    fn a_same_named_project_without_git_is_unverified() {
        let tmp = tempfile::tempdir().unwrap();
        let this = tmp.path().join("this");
        make_repo(&this, Some("alpha"), "a");
        let plain = tmp.path().join("work/plain");
        std::fs::create_dir_all(plain.join(".codescout")).unwrap();
        std::fs::write(plain.join(".codescout/project.toml"), "").unwrap();
        std::fs::write(plain.join(".codescout/repo.toml"), "name = \"alpha\"\n").unwrap();
        let r = scan_default("alpha", &this, &[&tmp.path().join("work")]);
        assert!(r.collisions.is_empty() && r.same_repo.is_empty(), "{r:?}");
        assert_eq!(r.unverified.len(), 1, "{r:?}");
    }
    #[test]
    fn dependency_and_build_directories_are_not_scanned() {
        let tmp = tempfile::tempdir().unwrap();
        let this = tmp.path().join("this");
        make_repo(&this, Some("alpha"), "a");
        let work = tmp.path().join("work");
        // Same name, different history: scanned, each would be a collision.
        make_repo(&work.join("node_modules/p"), Some("alpha"), "b");
        make_repo(&work.join("target/q"), Some("alpha"), "c");
        make_repo(&work.join("real"), Some("alpha"), "d");
        let r = scan_default("alpha", &this, &[&work]);
        assert_eq!(r.collisions, vec![work.join("real")], "{r:?}");
    }

    #[test]
    fn a_same_named_unborn_repo_is_unverified_through_a_scan() {
        let tmp = tempfile::tempdir().unwrap();
        let this = tmp.path().join("this");
        make_repo(&this, Some("alpha"), "a");
        let fresh = tmp.path().join("work/fresh");
        std::fs::create_dir_all(fresh.join(".codescout")).unwrap();
        git(&fresh, &["init", "-q", "-b", "main"]);
        std::fs::write(fresh.join(".codescout/project.toml"), "").unwrap();
        std::fs::write(fresh.join(".codescout/repo.toml"), "name = \"alpha\"\n").unwrap();
        let r = scan_default("alpha", &this, &[&tmp.path().join("work")]);
        assert!(r.collisions.is_empty() && r.same_repo.is_empty(), "{r:?}");
        assert_eq!(r.unverified, vec![fresh], "{r:?}");
    }

    #[test]
    fn a_git_less_project_under_an_ancestor_repo_is_judged_as_itself() {
        let tmp = tempfile::tempdir().unwrap();
        let this = tmp.path().join("this");
        make_repo(&this, Some("alpha"), "a");
        // An ancestor repo ABOVE the scan root that carries the same name (a dotfiles repo in
        // `$HOME` is the real case). The project below it has no `.git` of its own.
        let ancestor = tmp.path().join("ancestor");
        make_repo(&ancestor, Some("alpha"), "b");
        let plain = ancestor.join("work/plain");
        std::fs::create_dir_all(plain.join(".codescout")).unwrap();
        std::fs::write(plain.join(".codescout/project.toml"), "").unwrap();
        std::fs::write(plain.join(".codescout/repo.toml"), "name = \"alpha\"\n").unwrap();
        let r = scan_default("alpha", &this, &[&ancestor.join("work")]);
        assert!(r.collisions.is_empty() && r.same_repo.is_empty(), "{r:?}");
        assert_eq!(r.unverified, vec![plain], "{r:?}");
    }

    #[test]
    fn render_scope_says_not_exhaustive_for_every_bound_and_for_none() {
        let report = ScanReport {
            roots: vec![PathBuf::from("/a")],
            dirs_visited: 3,
            ..ScanReport::default()
        };
        let none = render_scope(&report);
        assert!(none.contains("no bound was hit"), "{none}");
        assert!(!none.contains("not exhaustive"), "{none}");
        for bound in [Bound::Depth, Bound::Dirs, Bound::Deadline] {
            let line = render_scope(&ScanReport {
                bounded_by: Some(bound),
                ..report.clone()
            });
            assert!(line.contains("not exhaustive"), "{line}");
            assert!(!line.contains("no bound was hit"), "{line}");
        }
    }

    #[test]
    fn render_scope_counts_one_directory_in_the_singular() {
        let one = ScanReport {
            roots: vec![PathBuf::from("/a")],
            dirs_visited: 1,
            ..ScanReport::default()
        };
        let line = render_scope(&one);
        assert!(line.contains("1 directory visited"), "{line}");
        assert!(!line.contains("1 directories"), "{line}");
        let many = render_scope(&ScanReport {
            dirs_visited: 0,
            ..one
        });
        assert!(many.contains("0 directories visited"), "{many}");
    }

    #[test]
    fn render_scope_names_roots_directories_and_the_bound() {
        let report = ScanReport {
            roots: vec![PathBuf::from("/a"), PathBuf::from("/b")],
            dirs_visited: 42,
            bounded_by: None,
            ..ScanReport::default()
        };
        let line = render_scope(&report);
        assert!(!line.contains('\n'), "{line}");
        assert!(line.contains("/a") && line.contains("/b"), "{line}");
        assert!(line.contains("42"), "{line}");
        for (bound, word) in [
            (Bound::Depth, "depth"),
            (Bound::Dirs, "director"),
            (Bound::Deadline, "time"),
        ] {
            let bounded = render_scope(&ScanReport {
                bounded_by: Some(bound),
                ..report.clone()
            });
            assert_ne!(bounded, line);
            assert!(bounded.contains(word), "{bounded}");
            assert!(!bounded.contains('\n'), "{bounded}");
        }
    }
}
