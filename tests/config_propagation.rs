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
//!
//! ## Where it looks
//!
//! Everywhere below the repo root. There is no list of worktree homes to keep current — see
//! [`discover_linked_worktrees`] for why one was written, then deleted.

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Directory names never descended into: not worktree homes, and expensive or circular.
const PRUNED: [&str; 3] = [".git", "target", "node_modules"];

/// Every linked worktree under `root`, found by **walking** rather than by a list of the
/// places worktrees are supposed to live.
///
/// **Why discovery and not a root list.** This started as `SCAN_ROOTS = [".worktrees",
/// ".claude/worktrees"]` — the two roots this repo's entire corpus mentions, established by
/// grep rather than guessed. A hardcoded selector over a namespace that grows is `IC-18`, and
/// it decays in exactly this class's own shape: a new tool adds a third root, that root
/// propagates into the tool and not into the list, and the gate reports green over a place it
/// never looked. Nothing fires when that happens. Walking deletes the list, so a worktree at
/// `.superpowers/worktrees/x` — or anywhere else — is found because it *is* one, not because
/// somebody predicted where it would be.
///
/// **The marker is `.git` being a FILE.** That is exactly what a linked worktree has and an
/// ordinary directory does not. A `.git` *directory* is an independent checkout, so it is never
/// **reported** — but the walk does continue **through** it, and will report that checkout's own
/// linked worktrees as if they were ours. Pinned deliberately by
/// [`a_nested_independent_checkout_is_walked_through_not_reported`]: over-reporting is benign
/// here (a healthy nested worktree resolves and passes), while pruning at `.git`-the-directory
/// would create a place this gate structurally cannot look.
/// A git **submodule** carries the same file shape (`gitdir: <super>/.git/modules/<name>`) and
/// breaks on a rename identically, so it is deliberately in scope. This repo has no
/// `.gitmodules` today — stated rather than demonstrated, and the reason no test asserts it.
///
/// **Do not reach for `ignore::WalkBuilder` here, though it is already a dependency and is what
/// the rest of this repo walks with.** It honours `.gitignore`, and `.worktrees/` is gitignored
/// (`.gitignore:7` and `:130`) — so the obvious walker skips the one directory this check
/// exists for and returns a clean zero. The trap is that its output is indistinguishable from a
/// healthy tree; [`the_walk_finds_every_worktree_git_itself_reports`] is what would catch it.
///
/// A found worktree is not descended into: it is a whole other checkout, and its own
/// sub-worktrees are its own business.
fn discover_linked_worktrees(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut queue = vec![root.to_path_buf()];
    while let Some(dir) = queue.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            if PRUNED
                .iter()
                .any(|p| entry.file_name() == std::ffi::OsStr::new(p))
            {
                continue;
            }
            if path.join(".git").is_file() {
                found.push(path); // Another checkout entirely; do not descend.
            } else {
                queue.push(path);
            }
        }
    }
    found.sort();
    found
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
/// wrong proposition. [`the_walk_finds_every_worktree_git_itself_reports`] is the positive
/// control that keeps that vacuity honest — without it, a walk that found nothing because its
/// pruning was wrong would be indistinguishable from a clean tree.
///
/// **Near-vacuous from inside a worktree too**, for a different reason: `CARGO_MANIFEST_DIR` is
/// then the worktree, and the walk starts there rather than at the main checkout. Correct — a
/// linked worktree's siblings are not below it — but a green here from `.worktrees/<x>` says
/// nothing about the main checkout.
#[test]
fn no_linked_worktree_points_at_a_gitdir_that_is_gone() {
    let root = repo_root();
    let mut broken: Vec<(PathBuf, String)> = Vec::new();

    for wt in discover_linked_worktrees(&root) {
        let Ok(text) = std::fs::read_to_string(wt.join(".git")) else {
            continue;
        };
        match worktree_link(&text, |p| Path::new(p).exists()) {
            WorktreeLink::Wired(_) => {}
            WorktreeLink::Unparsable => broken.push((wt, "<no `gitdir:` line>".to_owned())),
            WorktreeLink::Orphaned(target) => broken.push((wt, target)),
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

/// The walk must find every worktree git itself knows about — a positive control on discovery.
///
/// This replaces an earlier test that asked the opposite question ("is every registered
/// worktree under one of the two hardcoded `SCAN_ROOTS`?"). That question stopped existing when
/// the roots did, but the need behind it did not, and it inverted into something stronger.
///
/// The check above is an **absence** assertion — `broken.is_empty()` — which is monotone under
/// removal: a walk that returns nothing at all satisfies it perfectly. Pruning one directory
/// too many, an `ignore`-crate walker silently honouring `.gitignore`, a permission error
/// swallowed by `read_dir`'s `let Ok(…) else` — each produces a green tick over zero coverage.
/// So the discovery needs a witness from an instrument that does not share its mechanism, and
/// `git worktree list` is exactly that: git reads `.git/worktrees/` admin dirs, the walk reads
/// the filesystem, and they agree only if both work.
///
/// **It transfers to the orphan case even though git cannot see orphans**, which is the whole
/// reason the check above exists. On disk a registered worktree and an orphaned one are the
/// same shape — a directory whose `.git` is a file — and the walk cannot tell them apart,
/// because it never asks git anything. Whatever finds one finds the other.
///
/// Scoped to worktrees *inside* the repo root: a `/tmp` scratch worktree is legitimate, common
/// during probing, and not something a walk rooted here could reach.
#[test]
fn the_walk_finds_every_worktree_git_itself_reports() {
    let root = repo_root();
    let out = Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .current_dir(&root)
        .output()
        .expect("git worktree list failed to run");

    let found = discover_linked_worktrees(&root);
    let missed: Vec<String> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| l.strip_prefix("worktree "))
        .map(Path::new)
        // The entry for this checkout itself is not a linked worktree below it.
        .filter(|p| *p != root.as_path())
        .filter(|p| p.starts_with(&root))
        .filter(|p| !found.iter().any(|f| f == p))
        .map(|p| p.display().to_string())
        .collect();

    assert!(
        missed.is_empty(),
        "`git worktree list` reports worktree(s) inside this repo that the walk did not \
         find:\n  {}\n\n\
         `discover_linked_worktrees` walked from {} and found {}: {:?}\n\n\
         Something is hiding them from the walk — most likely an over-broad entry in `PRUNED`, \
         a swallowed `read_dir` permission error, or a walker that honours `.gitignore` \
         (`.worktrees/` is gitignored). Whatever it is hides ORPHANED worktrees from \
         `no_linked_worktree_points_at_a_gitdir_that_is_gone` too, and that test would stay \
         green while covering nothing.",
        missed.join("\n  "),
        root.display(),
        found.len(),
        found
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
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

/// Build a tree exercising every rule in [`discover_linked_worktrees`], and return it.
///
/// A linked worktree on disk is exactly "a directory whose `.git` is a FILE" — the file's
/// contents are irrelevant to discovery (they are [`worktree_link`]'s business), so these
/// fixtures write a plausible `gitdir:` line and nothing else. No `git` invocation is needed
/// or wanted: the point is to test the walk against shapes a real repo makes it hard to
/// arrange, several of which cannot be created here at all without breaking the checkout.
fn make_walk_fixture(root: &Path) {
    let wt = |rel: &str| {
        let dir = root.join(rel);
        std::fs::create_dir_all(&dir).expect("mkdir");
        std::fs::write(dir.join(".git"), "gitdir: /somewhere/.git/worktrees/x\n").expect("write");
    };

    wt(".worktrees/wt-a"); // gitignored-shaped: the `ignore` crate would skip this
    wt("deep/a/b/c/wt-b"); // depth 5: no depth cap, no root list
    wt("target/wt-c"); // pruned
    wt(".git/wt-d"); // pruned
    wt("node_modules/wt-e"); // pruned
    wt(".worktrees/wt-a/nested/wt-f"); // inside a FOUND worktree: not descended into

    // A plain directory, and a nested INDEPENDENT checkout (`.git` is a directory).
    std::fs::create_dir_all(root.join("plain/deeper")).expect("mkdir");
    std::fs::create_dir_all(root.join("checkout/.git/objects")).expect("mkdir");
    wt("checkout/inner-wt"); // a worktree belonging to that other checkout
}

/// The walk must find a worktree wherever it sits, and must skip exactly what it claims to.
///
/// **This is the test the live checks cannot be.** Both of those scan the real repo root, so
/// on a checkout with no linked worktrees — CI always, and this machine since
/// `.worktrees/audit-shards-t7` was removed — they pass having examined nothing, and
/// [`the_walk_finds_every_worktree_git_itself_reports`] goes vacuous with them, because a
/// positive control needs a positive to witness. Every rule below was therefore unexercised
/// in CI from the moment the walk replaced the hardcoded `SCAN_ROOTS` list.
///
/// Mutations caught: removing any entry from [`PRUNED`]; adding a depth cap below 5; swapping
/// `.git`-is-a-file for `.git`-exists (which would report `checkout/` itself); and deleting the
/// don't-descend rule, which would report `wt-f` nested inside `wt-a`.
#[test]
fn the_walk_finds_worktrees_anywhere_and_skips_what_it_must() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let root = tmp.path();
    make_walk_fixture(root);

    let found: Vec<String> = discover_linked_worktrees(root)
        .iter()
        .map(|p| {
            p.strip_prefix(root)
                .unwrap_or(p)
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect();

    assert_eq!(
        found,
        vec![
            ".worktrees/wt-a".to_string(),
            "checkout/inner-wt".to_string(),
            "deep/a/b/c/wt-b".to_string(),
        ],
        "exact set, sorted. Present: a gitignored-shaped path (the `ignore` crate would skip \
         it), a depth-5 path (no cap, no root list), and a worktree owned by a nested \
         independent checkout. Absent: everything under `target/`, `.git/` and \
         `node_modules/`, and `wt-f` nested inside the already-found `wt-a`."
    );
}

/// A nested independent checkout is descended THROUGH, not skipped — pinned because the
/// alternative is defensible and the difference is invisible until it fires.
///
/// `checkout/` has a `.git` DIRECTORY, so it is correctly not reported as a linked worktree.
/// The walk then continues into it and reports `checkout/inner-wt`, a worktree that belongs to
/// that other repository rather than to this one.
///
/// Kept rather than changed, because over-reporting here is benign and under-reporting is not:
/// an orphaned worktree is broken wherever it lives, and [`worktree_link`] passes it as `Wired`
/// whenever its own admin dir resolves — so a healthy nested checkout costs nothing, while
/// pruning at `.git`-the-directory would create a place this gate structurally cannot look.
/// If that trade is ever revisited, this test is the record of which way it was made.
#[test]
fn a_nested_independent_checkout_is_walked_through_not_reported() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let root = tmp.path();
    make_walk_fixture(root);

    let found = discover_linked_worktrees(root);
    let rel: Vec<String> = found
        .iter()
        .map(|p| p.strip_prefix(root).unwrap_or(p).to_string_lossy().into())
        .collect();

    assert!(
        !rel.iter().any(|p| p == "checkout"),
        "a `.git` DIRECTORY is an independent checkout, never a linked worktree: {rel:?}"
    );
    assert!(
        rel.iter().any(|p| p == "checkout/inner-wt"),
        "the walk continues through it, so that checkout's own worktree IS reported: {rel:?}"
    );
}
