//! `IC-4` — config propagation is additive: updates land, removals and renames do not.
//!
//! Home for the surfaces of that class which are not hook configuration. The `core.hooksPath`
//! surface is the exception and stays in `tests/hook_config.rs`, beside the other thing that
//! reads `.pre-commit-config.yaml`. Everything else `IC-4` enumerates — MCP env, shell env, git
//! config beyond `hooksPath`, worktree gitdir, hook scripts, sweep scripts, the env-copy flow,
//! memory keys — belongs here.
//!
//! **Two of eight surfaces are covered.** Do not read this file's existence as coverage of the
//! class; read the list above and subtract.
//!
//! # Surface: a linked worktree's forward pointer
//!
//! `git worktree add` writes an **absolute** path into `<worktree>/.git`, as
//! `gitdir: <main>/.git/worktrees/<name>`. Rename or move the main repository and that path
//! dies while the main checkout keeps working perfectly — the additive half of the rename
//! lands, the removal half does not. That is `IC-4`'s claim with nothing left over.
//!
//! ## Why the filesystem and not `git worktree list`
//!
//! Not because git is blind to this. Because **git's report of it expires.** Measured
//! 2026-09-02 in a throwaway repo, renaming the parent of a linked worktree:
//!
//! - Immediately after the rename, `git worktree list` DOES show the entry, tagged
//!   `prunable gitdir file points to non-existent location`. Loud, and repairable.
//! - `git gc` runs `git worktree prune --expire 3.months.ago` on its own (`gc.worktreePruneExpire`).
//!   That **deletes the admin directory** — for a worktree whose files are still on disk, because
//!   git is judging it by a path that moved.
//! - After the prune, `git worktree list` shows only the main checkout, `.git/worktrees/` is
//!   gone, and the worktree sits on disk intact with a dead pointer. `git -C <worktree>` any
//!   command answers `fatal: not a git repository: (null)`.
//!
//! So the loud state converts itself into a silent one, on a timer, with no event. A gate built
//! on `git worktree list` would pass precisely once the defect became invisible — the instrument
//! that reports it is the one that later erases the evidence.
//!
//! The archived instance is the post-expiry state exactly:
//! `docs/issues/archive/2026-08-16-bench-worktree-gitdir-points-at-pre-rename-path.md` records
//! `.git/worktrees/` holding no `bench` entry, `git worktree list` not showing it, and every
//! corpus file present on disk — a pinned benchmark corpus that reads fine and cannot name its
//! own baseline commit. That file's `unverified:` field reports the defect still live 14 days
//! after being marked fixed.
//!
//! ## What this deliberately does NOT gate
//!
//! The **reverse** pointer — an admin dir under `.git/worktrees/<n>` whose `gitdir` names a
//! worktree that is gone — is not gated, and this file claims no coverage of it. git already
//! prints `prunable` for that, and the reason string is byte-identical for the rename shape and
//! for a worktree someone removed with `rm -rf` instead of `git worktree remove`. The two are
//! indistinguishable at the point of measurement, so a gate on it would red on benign cleanup.
//! Left to `git worktree prune`, which is the correct owner of that direction.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Directories under the repo root whose children may be linked worktrees.
///
/// Both are real for this repo: `.worktrees/` is the SDD/superpowers convention and is
/// gitignored at `.gitignore:7`; `.claude/worktrees/` is where the harness put
/// `peer-delegation`, named in the archived `bench` instance. Scratch worktrees under `/tmp`
/// are outside the repo and out of scope on purpose — they are probe fixtures with a lifetime
/// of minutes, and [`every_registered_worktree_inside_the_repo_is_under_a_scanned_root`] is
/// scoped to exclude them rather than chase them.
const SCAN_ROOTS: [&str; 2] = [".worktrees", ".claude/worktrees"];

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// What a linked worktree's `.git` file resolves to.
#[derive(Debug, PartialEq, Eq)]
enum WorktreeLink {
    /// No parsable `gitdir:` line. Not a shape git writes; treated as broken, not skipped.
    Unparsable,
    /// `gitdir:` names an admin directory that exists. Healthy.
    Wired(String),
    /// `gitdir:` names a path that is gone. The worktree is orphaned and git is silent.
    Orphaned(String),
}

/// Judge a `.git` file's contents against a path-existence oracle.
///
/// Pure over both inputs so [`the_worktree_link_verdict_discriminates`] can reach every arm
/// without renaming this repository — which is the only way to manufacture the defect in
/// place, and would break four other sessions' checkouts to do it.
fn worktree_link(dot_git: &str, exists: impl Fn(&str) -> bool) -> WorktreeLink {
    match dot_git
        .lines()
        .find_map(|l| l.trim().strip_prefix("gitdir:"))
        .map(str::trim)
        .filter(|t| !t.is_empty())
    {
        None => WorktreeLink::Unparsable,
        Some(t) if exists(t) => WorktreeLink::Wired(t.to_owned()),
        Some(t) => WorktreeLink::Orphaned(t.to_owned()),
    }
}

/// Every on-disk linked worktree must still reach its admin directory.
///
/// **Vacuous on a healthy checkout, and vacuous in CI — say so rather than bank it.** CI clones
/// fresh and has no linked worktrees at all, so this passes there having examined nothing. The
/// party it protects is the developer who renamed a directory: per `IC-4`'s `Blind party:`
/// field, someone who verified the change they could see and got positive evidence for the
/// wrong proposition.
///
/// **Vacuous from inside a worktree too**, for a different reason: `CARGO_MANIFEST_DIR` is then
/// the worktree, which has no `.worktrees/` of its own. That is correct rather than a gap — a
/// linked worktree has no sub-worktrees to check — but it means a green here from
/// `.worktrees/<x>` says nothing about the main checkout.
#[test]
fn no_linked_worktree_points_at_a_gitdir_that_is_gone() {
    let root = repo_root();
    let mut broken: Vec<(PathBuf, String)> = Vec::new();

    for rel in SCAN_ROOTS {
        let Ok(entries) = std::fs::read_dir(root.join(rel)) else {
            continue; // Root absent is the normal case; nothing to say about it.
        };
        for entry in entries.flatten() {
            let dot_git = entry.path().join(".git");
            // A `.git` DIRECTORY is a nested independent checkout, not a linked worktree.
            // Only the file form carries the absolute pointer this gate is about.
            if !dot_git.is_file() {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&dot_git) else {
                continue;
            };
            match worktree_link(&text, |p| Path::new(p).exists()) {
                WorktreeLink::Wired(_) => {}
                WorktreeLink::Unparsable => {
                    broken.push((entry.path(), "<no `gitdir:` line>".to_owned()))
                }
                WorktreeLink::Orphaned(target) => broken.push((entry.path(), target)),
            }
        }
    }

    assert!(
        broken.is_empty(),
        "orphaned linked worktree(s) — the directory is on disk, its `.git` names a path that \
         is gone, and `git -C <it> …` answers `fatal: not a git repository: (null)`:\n{}\n\n\
         Usual cause: this repository was renamed or moved. The worktree's `.git` stores an \
         ABSOLUTE path, so the new location propagated and the old pointer did not.\n\n\
         Remedy depends on whether `git gc` has pruned the admin dir yet — check with \
         `ls .git/worktrees/`:\n\
         \x20 admin dir PRESENT: `git worktree repair <ABSOLUTE path to the worktree>`, run \
         from the main checkout. The path argument is load-bearing — measured 2026-09-02, bare \
         `git worktree repair` exits 0, prints nothing and fixes nothing, and running it from \
         INSIDE the worktree cannot work at all.\n\
         \x20 admin dir GONE: `repair` answers `unable to locate repository`. There is nothing \
         left to repair; re-create the worktree or delete the directory.",
        broken
            .iter()
            .map(|(wt, target)| format!("  {} -> {target}", wt.display()))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// The scan roots must still be where worktrees actually get made.
///
/// [`SCAN_ROOTS`] is a selector, and a selector silently narrower than the population it names
/// is `IC-18`. This does not widen the scan; it makes the scan announce when its own scope has
/// gone stale, which is the part a reader cannot otherwise see. A registered worktree is by
/// definition not yet orphaned — the point is that if it ever orphans, nothing above would look
/// there.
///
/// Scoped to worktrees *inside* the repo root: `/tmp` probe worktrees are legitimate and
/// transient, and flagging them would make this fail for anyone mid-probe.
#[test]
fn every_registered_worktree_inside_the_repo_is_under_a_scanned_root() {
    let root = repo_root();
    let out = Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .current_dir(&root)
        .output()
        .expect("git worktree list failed to run");

    let unscanned: Vec<String> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| l.strip_prefix("worktree "))
        .map(Path::new)
        // The entry for this checkout itself is not a nested worktree.
        .filter(|p| *p != root.as_path())
        .filter(|p| p.starts_with(&root))
        .filter(|p| !SCAN_ROOTS.iter().any(|rel| p.starts_with(root.join(rel))))
        .map(|p| p.display().to_string())
        .collect();

    assert!(
        unscanned.is_empty(),
        "registered worktree(s) inside this repo that `SCAN_ROOTS` does not cover:\n  {}\n\n\
         `no_linked_worktree_points_at_a_gitdir_that_is_gone` would not notice these orphaning. \
         Add the containing directory to `SCAN_ROOTS` in this file.",
        unscanned.join("\n  ")
    );
}

/// The verdict must reach every arm, or the gate above is decoration.
///
/// The live test can only observe `Wired` on a healthy checkout, and observes nothing at all in
/// CI — so without this, `no_linked_worktree_points_at_a_gitdir_that_is_gone` is a test whose
/// only exercised path is the one that cannot fail.
#[test]
fn the_worktree_link_verdict_discriminates() {
    let never = |_: &str| false;
    let always = |_: &str| true;

    // The exact byte shape `git worktree add` writes, trailing newline included.
    let real = "gitdir: /home/u/work/proj/.git/worktrees/feat\n";
    assert_eq!(
        worktree_link(real, always),
        WorktreeLink::Wired("/home/u/work/proj/.git/worktrees/feat".into())
    );
    assert_eq!(
        worktree_link(real, never),
        WorktreeLink::Orphaned("/home/u/work/proj/.git/worktrees/feat".into()),
        "the pre-rename absolute path is the whole of the archived `bench` bug"
    );

    // A `.git` file with no pointer is broken, and must not read as healthy by omission.
    assert_eq!(worktree_link("", always), WorktreeLink::Unparsable);
    assert_eq!(
        worktree_link("ref: refs/heads/x\n", always),
        WorktreeLink::Unparsable
    );
    assert_eq!(worktree_link("gitdir:\n", always), WorktreeLink::Unparsable);
    assert_eq!(
        worktree_link("gitdir:    \n", always),
        WorktreeLink::Unparsable
    );

    // Only `Orphaned`/`Unparsable` fail the gate, so `Wired` must not be reachable by accident:
    // a target that exists is healthy no matter how the line is spaced.
    assert_eq!(
        worktree_link("  gitdir:   /a/b  \n", always),
        WorktreeLink::Wired("/a/b".into()),
        "git does not write this spacing, but tolerating it must not change the verdict"
    );
}
