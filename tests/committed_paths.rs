//! No committed script may hardcode a path under someone's home directory.
//!
//! Why this exists. Both bm25 sweep scripts shipped an absolute default under
//! `/home/marius/.../code-explorer` — codescout's own pre-rename name. The rename
//! landed, the directory stopped existing, and nothing noticed: a shell default is
//! not a symbol, not a doc link, and not a feature flag, so none of the gates that
//! catch drift elsewhere look at it. Both scripts were unrunnable out of the box on
//! every machine, including the author's. Full account:
//! `docs/issues/archive/2026-08-14-sweep-scripts-hardcode-dead-machine-specific-paths.md`.
//!
//! The rule this enforces is CLAUDE.md's: per-machine values belong outside every
//! repo, because committing one makes the file read as *false* to anyone standing on
//! a different host. A benchmark convenience default is exactly how such a value gets
//! committed without review friction — it looks like configuration, not like a claim.
//!
//! Scope is `scripts/` deliberately. `docs/` is full of home paths that are *records*
//! — measured output, quoted terminal sessions, archived bug reports — and rewriting
//! those would falsify history rather than fix anything. `.github/workflows/` is out
//! for a different reason: `/home/runner` is GitHub's path on every runner alive, so
//! it is not machine-specific at all.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Home-directory prefixes that are universal rather than personal.
///
/// An entry here asserts "this account exists identically on every machine that runs
/// this file", which is true of CI runner images and nothing else so far.
const UNIVERSAL_ACCOUNTS: &[&str] = &["runner"];

/// Directories that hold generated or vendored content, not authored scripts.
const SKIP_DIRS: &[&str] = &["__pycache__", "node_modules"];

fn scripts_dir() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/scripts"))
}

/// Collect every readable text file under `dir`, skipping generated directories.
fn text_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if path.is_dir() {
            if !SKIP_DIRS.contains(&name.as_ref()) {
                text_files(&path, out);
            }
        } else {
            out.push(path);
        }
    }
}

/// The account segment following a `/home/` or `/Users/` marker, if any.
///
/// Returns `None` when the marker is a bare prefix with no account after it (`/home/`
/// at end of line, or `/home//`), which no real path takes.
fn account_after(rest: &str) -> Option<&str> {
    let end = rest
        .find(|c: char| c == '/' || c == '"' || c == '\'' || c.is_whitespace())
        .unwrap_or(rest.len());
    let account = &rest[..end];
    if account.is_empty() {
        None
    } else {
        Some(account)
    }
}

/// Every `file:line` in a TRACKED `scripts/` file naming a personal home directory.
///
/// Split from [`offenders_in`] so the filtering can be exercised against a fixture
/// tree — creating a real untracked file under `scripts/` to test the exclusion
/// would put it in every concurrent session's `git status`, which is the exact cost
/// this function exists to stop paying.
fn offenders() -> Vec<String> {
    offenders_in(&scripts_dir(), &tracked_scripts())
}

/// Paths git TRACKS under `scripts/`, repo-relative, as `git ls-files` prints them.
///
/// `git ls-files` reports index ENTRIES, not files: while a merge holds an unresolved
/// conflict the same path is listed once per stage (1/2/3). Collecting into a
/// `BTreeSet` collapses those to one, which matters here only for determinism — this
/// set is membership-tested, never counted — but the dedup is cheap and the sibling
/// sites in `tests/issue_clusters.rs` and `tests/result_caps.rs` carry the same guard
/// for populations that ARE counted.
fn tracked_scripts() -> BTreeSet<String> {
    let out = std::process::Command::new("git")
        .args(["ls-files", "--", "scripts"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("git ls-files must be runnable from the repo root");
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::to_owned)
        .collect()
}

/// The scan, over one tree, restricted to the paths in `tracked`.
///
/// `tracked` keys are repo-relative (`scripts/foo.py`), matching `git ls-files` output,
/// which is also how the findings are formatted — so one string serves as both the
/// membership key and the citation.
fn offenders_in(root: &Path, tracked: &BTreeSet<String>) -> Vec<String> {
    let mut files = Vec::new();
    text_files(root, &mut files);
    files.sort();

    let mut found = Vec::new();
    for file in files {
        let rel = file
            .strip_prefix(root)
            .unwrap_or(&file)
            .display()
            .to_string();
        let key = format!("scripts/{rel}");
        // The whole point of the tracked filter. An untracked file under `scripts/`
        // is another session's work in progress, or your own scratch: it is not in
        // the repo, so a hardcoded path in it is not yet anybody's problem. Scanning
        // it reds the SHARED gate for every session in the checkout, and names the
        // file as "committed" when it has never been committed at all — sending the
        // reader to look for a tracked-file problem that does not exist.
        // docs/issues/archive/2026-09-11-the-committed-scripts-gate-scans-the-filesystem-so-an-untracked-file-reds-it.md
        if !tracked.contains(&key) {
            continue;
        }
        // Binary files (compiled python, images) are not authored scripts.
        let Ok(content) = std::fs::read_to_string(&file) else {
            continue;
        };
        for (idx, line) in content.lines().enumerate() {
            for marker in ["/home/", "/Users/"] {
                let mut from = 0usize;
                while let Some(rel_at) = line[from..].find(marker) {
                    let at = from + rel_at + marker.len();
                    if let Some(account) = account_after(&line[at..]) {
                        if !UNIVERSAL_ACCOUNTS.contains(&account) {
                            found.push(format!("{key}:{} — {}{account}", idx + 1, marker));
                        }
                    }
                    from = at;
                }
            }
        }
    }
    found
}

/// No script git TRACKS may hardcode a path under someone's home directory.
///
/// "Tracked" rather than "committed", and the word is load-bearing in both
/// directions. `git ls-files` lists index entries, so a file is in scope the moment
/// it is `git add`-ed — which is strictly before any commit exists, so narrowing the
/// population from the filesystem to the index loses no catch. And an UNTRACKED file
/// is out of scope, which is the fix: this gate previously walked the filesystem, so
/// one session's scratch script under `scripts/` red the shared gate for every
/// session in the checkout while the message told each of them their *committed*
/// scripts were broken.
///
/// The cost was the abort more than the false positive: `cargo test` is fail-fast
/// across binaries, so this failing hid every integration target ordered after it.
/// docs/issues/archive/2026-09-11-the-committed-scripts-gate-scans-the-filesystem-so-an-untracked-file-reds-it.md
#[test]
fn no_tracked_script_hardcodes_a_personal_home_path() {
    let found = offenders();
    assert!(
        found.is_empty(),
        "tracked scripts hardcode machine-specific home paths:\n  {}\n\n\
         Derive the value instead — the repo root is `$(cd \"$(dirname \"$0\")/..\" && pwd)` \
         — or take it from an environment variable with a portable fallback. If the account \
         is genuinely universal (a CI runner image), add it to UNIVERSAL_ACCOUNTS with the \
         reason.\n\n\
         Every path above is tracked by git (staged or committed). An untracked file is \
         not scanned, so a scratch script in your working tree cannot be the cause.",
        found.join("\n  ")
    );
}

/// The scan must be able to fail, and must not fire on the universal accounts.
///
/// Without this, a typo in `account_after` that returned `None` for everything would
/// leave the gate green forever and indistinguishable from a clean tree.
#[test]
fn the_home_path_scan_discriminates() {
    assert_eq!(account_after("marius/work/x"), Some("marius"));
    assert_eq!(account_after("marius\""), Some("marius"));
    assert_eq!(account_after("runner/work/_temp"), Some("runner"));
    assert_eq!(account_after(""), None);
    assert_eq!(account_after("/oops"), None);

    // The exemption is checked against the account, not the whole line, so a personal
    // account never rides in on a line that also mentions a universal one.
    assert!(UNIVERSAL_ACCOUNTS.contains(&"runner"));
    assert!(!UNIVERSAL_ACCOUNTS.contains(&"marius"));
}

/// `scripts/` must actually be reachable from the test binary.
///
/// A wrong `CARGO_MANIFEST_DIR` join would make `text_files` return nothing and the
/// gate above pass vacuously — the same false-green that let the original bug ship.
#[test]
fn the_scan_actually_reads_files() {
    let mut files = Vec::new();
    text_files(&scripts_dir(), &mut files);
    assert!(
        files.len() > 5,
        "expected scripts/ to contain files; found {} — the scan is looking in the wrong place",
        files.len()
    );
}

/// The fix, tested directly: a file on disk that git does not track is not scanned,
/// and an otherwise-identical tracked one still is.
///
/// Both fixture files carry the SAME offending line. That is the load-bearing detail
/// — it is what makes tracked-ness the only variable, so a pass cannot come from the
/// scanner simply failing to read one of them. Give them different content and this
/// test still passes while no longer discriminating.
///
/// Run against a fixture tree rather than `scripts/`: creating a real untracked file
/// there to test the exclusion would place it in every concurrent session's
/// `git status`, which is precisely the shared cost this filter exists to stop.
#[test]
fn an_untracked_script_is_excluded_and_a_tracked_one_is_not() {
    let dir = tempfile::tempdir().unwrap();
    let offending = "CODESCOUT = \"/home/someone/work/codescout\"\n";
    std::fs::write(dir.path().join("tracked.py"), offending).unwrap();
    std::fs::write(dir.path().join("untracked.py"), offending).unwrap();

    let tracked: BTreeSet<String> = ["scripts/tracked.py".to_string()].into_iter().collect();
    let found = offenders_in(dir.path(), &tracked);

    assert_eq!(
        found.len(),
        1,
        "exactly the tracked file should be reported; got {found:?}"
    );
    assert!(
        found[0].starts_with("scripts/tracked.py:1"),
        "the tracked file must still be caught — narrowing the population must not \
         disarm the gate. got: {found:?}"
    );
    assert!(
        !found.iter().any(|f| f.contains("untracked.py")),
        "an untracked file must not be scanned: it is not in the repo, and reporting \
         it reds the shared gate for every session in the checkout. got: {found:?}"
    );
}

/// `tracked_scripts()` is the population every finding is filtered through, so an
/// empty one makes the gate above pass by scanning nothing — vacuous green, and
/// indistinguishable from a clean tree.
///
/// This is not hypothetical for a subprocess: `git ls-files` returns empty and exits
/// 0 for a wrong `current_dir`, a pathspec typo, or a repo it cannot read. None of
/// those is an error at the call site.
///
/// `scripts/fmt-mine.sh` is the known-answer fixture — CLAUDE.md's gate step 1, so
/// it is tracked and will not quietly disappear. If it is ever renamed, this test
/// reds and wants its new name, which is the correct outcome rather than a nuisance.
#[test]
fn the_tracked_population_is_not_vacuous() {
    let tracked = tracked_scripts();
    assert!(
        !tracked.is_empty(),
        "git ls-files returned nothing for scripts/ — it exits 0 on a wrong cwd or a \
         pathspec typo, so an empty set here is a broken call, not a clean tree"
    );
    assert!(
        tracked.contains("scripts/fmt-mine.sh"),
        "expected the gate's own step-1 script in the tracked set; got {} entries \
         without it, so the pathspec or the prefix has moved",
        tracked.len()
    );
    assert!(
        tracked.iter().all(|p| p.starts_with("scripts/")),
        "ls-files output must be repo-relative and scoped to scripts/ — the keys are \
         matched against `scripts/{{rel}}` in offenders_in, so a different prefix \
         silently matches nothing and empties the population"
    );
}
