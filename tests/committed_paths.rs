//! No committed script may hardcode a path under someone's home directory.
//!
//! Why this exists. Both bm25 sweep scripts shipped an absolute default under
//! `/home/marius/.../code-explorer` — codescout's own pre-rename name. The rename
//! landed, the directory stopped existing, and nothing noticed: a shell default is
//! not a symbol, not a doc link, and not a feature flag, so none of the gates that
//! catch drift elsewhere look at it. Both scripts were unrunnable out of the box on
//! every machine, including the author's. Full account:
//! `docs/issues/2026-08-14-sweep-scripts-hardcode-dead-machine-specific-paths.md`.
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

/// Every `file:line` in `scripts/` naming a personal home directory.
fn offenders() -> Vec<String> {
    let root = scripts_dir();
    let mut files = Vec::new();
    text_files(&root, &mut files);
    files.sort();

    let mut found = Vec::new();
    for file in files {
        // Binary files (compiled python, images) are not authored scripts.
        let Ok(content) = std::fs::read_to_string(&file) else {
            continue;
        };
        let rel = file.strip_prefix(&root).unwrap_or(&file).display();
        for (idx, line) in content.lines().enumerate() {
            for marker in ["/home/", "/Users/"] {
                let mut from = 0usize;
                while let Some(rel_at) = line[from..].find(marker) {
                    let at = from + rel_at + marker.len();
                    if let Some(account) = account_after(&line[at..]) {
                        if !UNIVERSAL_ACCOUNTS.contains(&account) {
                            found.push(format!("scripts/{rel}:{} — {}{account}", idx + 1, marker));
                        }
                    }
                    from = at;
                }
            }
        }
    }
    found
}

#[test]
fn no_committed_script_hardcodes_a_personal_home_path() {
    let found = offenders();
    assert!(
        found.is_empty(),
        "committed scripts hardcode machine-specific home paths:\n  {}\n\n\
         Derive the value instead — the repo root is `$(cd \"$(dirname \"$0\")/..\" && pwd)` \
         — or take it from an environment variable with a portable fallback. If the account \
         is genuinely universal (a CI runner image), add it to UNIVERSAL_ACCOUNTS with the \
         reason.",
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
