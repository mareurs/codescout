//! Observe whether the working tree changed while a `run_command` declared `read` ran.
//!
//! `effect_class` records what the caller DECLARED; this records the one observation that
//! can contradict a `read` declaration, into `usage.db`'s `observed_changed` via
//! `usage::observed`. The ruling that shaped it (operator, 2026-09-24 and its follow-up):
//! observe only calls that declared `read`, from a `git status` snapshot before and after,
//! and record it as "the tree changed during the call", never as "this command wrote".
//!
//! # What a value does and does not mean
//!
//! - `true` — the git-visible tree differed between the two snapshots. It does NOT say
//!   the command did it: on a checkout shared with other sessions a peer's edit that lands
//!   during the call reads identically. It is a falsifier of "this was quiet", not an
//!   attribution. It also counts codescout's own writes into a `.codescout/` that the
//!   project does not gitignore.
//! - `false` — the git-visible tree was the same. It does NOT say the command wrote
//!   nothing: gitignored paths (`target/`, `.codescout/`) are invisible to `git status`,
//!   and a command that undid its own change, or a peer that did, leaves no difference.
//! - no value — nothing was observed: the call declared `write`, ran in the background
//!   (the call returns before the command finishes, so "unchanged" would be a false
//!   reassurance), named a buffer handle (a query of captured output, not a tree
//!   operation), refused or failed, or either snapshot could not be taken (not a git
//!   checkout, `git` missing, the 2 s budget exceeded).
//!
//! # What the snapshot compares
//!
//! The porcelain status list alone is not enough: a file already modified and modified
//! AGAIN keeps the same ` M` entry. Each listed path is therefore fingerprinted with its
//! status letters, size and mtime, so a second edit of a dirty file is seen.
//!
//! # Cost, and its bound
//!
//! One `git rev-parse` before, one `git status` before and one after, only for calls
//! declared `read`. `GIT_OPTIONAL_LOCKS=0` stops `git status` from refreshing the index —
//! otherwise an observation would contend with a peer's `git commit` for `index.lock`.
//! A snapshot that does not finish in [`SNAPSHOT_TIMEOUT`] is killed and records nothing;
//! it can never fail the command, only forgo its observation.

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::{declared_effect, parse_bool_param, ToolContext};

/// Longest one `git` call may take before the observation is abandoned.
const SNAPSHOT_TIMEOUT: Duration = Duration::from_secs(2);

/// One path `git status` lists, with enough of its stat to see a second edit.
#[derive(Debug, PartialEq, Eq)]
struct Entry {
    status: String,
    path: String,
    /// `(len, mtime in ns since the epoch)`, or `None` for a path that no longer exists
    /// (a deletion) or cannot be stat-ed.
    stat: Option<(u64, u128)>,
}

/// The git-visible state of a tree at one instant.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct TreeFingerprint(Vec<Entry>);

/// A taken "before" snapshot, waiting for its "after".
pub(super) struct Observation {
    top: PathBuf,
    before: TreeFingerprint,
}

/// A handle to one of the session's output buffers, anywhere in the command. Such a
/// command queries captured output (`grep PATTERN @cmd_ab12cd34`) and is by far the most
/// common `run_command` shape; a snapshot there would cost latency to learn nothing.
fn names_a_buffer_handle(command: &str) -> bool {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(r"@(?:cmd|file|tool|bg|ack)_[0-9a-f]{8}").expect("valid regex")
    })
    .is_match(command)
}

/// Whether this call is one whose tree effect is worth observing: it declared `read`, runs
/// in the foreground, and names no buffer handle.
///
/// Keyed on the SAME [`declared_effect`] the tool's own validation and the recorder use, so
/// the three cannot disagree about what a declared class is. A call that declared `write`
/// (or nothing, which means write) pays nothing: the observation exists to falsify a `read`.
pub(super) fn should_observe(input: &Value) -> bool {
    declared_effect(input) == Some("read")
        && !parse_bool_param(&input["run_in_background"])
        && !input["command"].as_str().is_none_or(names_a_buffer_handle)
}

/// Run `cmd` and return its stdout, or `None` when it fails to spawn, exits non-zero, or
/// outlives `budget`. The child is killed when the timeout drops its future
/// (`kill_on_drop`), so a wedged process cannot outlive the snapshot that spawned it.
async fn output_within(mut cmd: tokio::process::Command, budget: Duration) -> Option<Vec<u8>> {
    cmd.stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true);
    let out = tokio::time::timeout(budget, cmd.output())
        .await
        .ok()?
        .ok()?;
    out.status.success().then_some(out.stdout)
}

/// Run `git <args>` in `dir` and return its stdout, or `None` on any failure.
///
/// `GIT_OPTIONAL_LOCKS=0` is load-bearing: without it `git status` opportunistically
/// refreshes `.git/index`, which takes `index.lock` and would make an observation contend
/// with a peer's `git commit`. Pinned by
/// `a_snapshot_never_rewrites_the_index_it_is_observing`.
async fn git(dir: &Path, args: &[&str]) -> Option<Vec<u8>> {
    let mut cmd = tokio::process::Command::new("git");
    cmd.args(args)
        .current_dir(dir)
        .env("GIT_OPTIONAL_LOCKS", "0");
    output_within(cmd, SNAPSHOT_TIMEOUT).await
}

/// Fingerprint the tree rooted at `top` (a git toplevel), or `None` when `git status`
/// fails or exceeds [`SNAPSHOT_TIMEOUT`].
///
/// `--no-renames` turns every rename into a plain delete + add, so each `-z` record is
/// exactly `XY<space>path` and parsing needs no look-ahead.
pub(super) async fn fingerprint(top: &Path) -> Option<TreeFingerprint> {
    use std::time::UNIX_EPOCH;
    let out = git(
        top,
        &[
            "status",
            "--porcelain=v1",
            "-z",
            "--no-renames",
            "--untracked-files=all",
        ],
    )
    .await?;
    let mut entries = Vec::new();
    for record in out.split(|b| *b == 0).filter(|r| !r.is_empty()) {
        // `XY` + one space + at least a one-byte path.
        if record.len() < 4 {
            return None;
        }
        let status = String::from_utf8_lossy(&record[..2]).into_owned();
        let path = String::from_utf8_lossy(&record[3..]).into_owned();
        let stat = std::fs::symlink_metadata(top.join(&path)).ok().map(|m| {
            let mtime = m
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_nanos());
            (m.len(), mtime)
        });
        entries.push(Entry { status, path, stat });
    }
    Some(TreeFingerprint(entries))
}

/// The git toplevel containing `root`, or `None` when it is not inside a checkout. Needed
/// because `git status --porcelain` paths are relative to the toplevel, not to `root`.
async fn toplevel(root: &Path) -> Option<PathBuf> {
    let out = git(root, &["rev-parse", "--show-toplevel"]).await?;
    let text = String::from_utf8(out).ok()?;
    let top = text.trim_end_matches('\n');
    (!top.is_empty()).then(|| PathBuf::from(top))
}

/// Take the "before" snapshot when the call is observable, else `None`.
///
/// Every failure is `None`, never an error: the observation is a side channel and must not
/// be able to fail, or noticeably delay, the command it describes.
pub(super) async fn begin(input: &Value, ctx: &ToolContext) -> Option<Observation> {
    if !should_observe(input) {
        return None;
    }
    let root = ctx
        .agent
        .require_project_root_for(ctx.workspace_override.as_deref())
        .await
        .ok()?;
    let top = toplevel(&root).await?;
    let before = fingerprint(&top).await?;
    Some(Observation { top, before })
}

impl Observation {
    /// Take the "after" snapshot and record the verdict. Records nothing when it fails.
    pub(super) async fn finish(self) {
        if let Some(after) = fingerprint(&self.top).await {
            crate::usage::observed::record(after != self.before);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::cell::RefCell;

    fn git_sync(dir: &Path, args: &[&str]) {
        let out = std::process::Command::new("git")
            .args(args)
            .current_dir(dir)
            .output()
            .expect("git must be installed to run these tests");
        assert!(
            out.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    /// A committed one-file repo with `.gitignore` covering `ignored/`.
    fn repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path();
        git_sync(p, &["init", "-q"]);
        git_sync(p, &["config", "user.email", "t@example.invalid"]);
        git_sync(p, &["config", "user.name", "t"]);
        git_sync(p, &["config", "commit.gpgsign", "false"]);
        // `.codescout/` is ignored as in a real project: `Agent::new` and the usage
        // recorder write there, and an unignored copy would read as a tree change.
        std::fs::write(p.join(".gitignore"), "ignored/\n.codescout/\n").unwrap();
        std::fs::write(p.join("tracked.txt"), "one\n").unwrap();
        git_sync(p, &["add", "."]);
        git_sync(p, &["commit", "-q", "-m", "init"]);
        dir
    }

    async fn snap(p: &Path) -> TreeFingerprint {
        fingerprint(p)
            .await
            .expect("a git checkout must fingerprint")
    }

    #[tokio::test]
    async fn an_untouched_tree_fingerprints_equal() {
        let dir = repo();
        assert_eq!(snap(dir.path()).await, snap(dir.path()).await);
    }

    #[tokio::test]
    async fn editing_a_tracked_file_changes_the_fingerprint() {
        let dir = repo();
        let before = snap(dir.path()).await;
        std::fs::write(dir.path().join("tracked.txt"), "one\ntwo\n").unwrap();
        assert_ne!(before, snap(dir.path()).await);
    }

    /// The case the porcelain list alone cannot see: ` M` stays ` M` when an already
    /// dirty file is written again. The edit keeps the SAME length so only the mtime
    /// can tell, and the mtime is pinned apart explicitly rather than trusting the
    /// filesystem's clock granularity.
    #[tokio::test]
    async fn rewriting_an_already_dirty_file_changes_the_fingerprint() {
        let dir = repo();
        let f = dir.path().join("tracked.txt");
        std::fs::write(&f, "two\n").unwrap();
        let before = snap(dir.path()).await;
        std::fs::write(&f, "TWO\n").unwrap();
        let later = std::time::SystemTime::now() + Duration::from_secs(5);
        std::fs::File::options()
            .write(true)
            .open(&f)
            .unwrap()
            .set_modified(later)
            .unwrap();
        assert_ne!(before, snap(dir.path()).await);
    }

    #[tokio::test]
    async fn a_new_untracked_file_changes_the_fingerprint() {
        let dir = repo();
        let before = snap(dir.path()).await;
        std::fs::write(dir.path().join("new.txt"), "x").unwrap();
        assert_ne!(before, snap(dir.path()).await);
    }

    #[tokio::test]
    async fn a_deleted_file_changes_the_fingerprint() {
        let dir = repo();
        let before = snap(dir.path()).await;
        std::fs::remove_file(dir.path().join("tracked.txt")).unwrap();
        assert_ne!(before, snap(dir.path()).await);
    }

    /// The documented blind spot, pinned so it cannot silently change: a gitignored write
    /// is not visible. If this ever fails, the column's stated meaning is out of date.
    #[tokio::test]
    async fn a_gitignored_write_is_invisible() {
        let dir = repo();
        let before = snap(dir.path()).await;
        std::fs::create_dir_all(dir.path().join("ignored")).unwrap();
        std::fs::write(dir.path().join("ignored/out.bin"), "x").unwrap();
        assert_eq!(before, snap(dir.path()).await);
    }

    #[tokio::test]
    async fn a_directory_outside_any_checkout_fingerprints_to_nothing() {
        // Isolated from any enclosing repo: `GIT_CEILING_DIRECTORIES` stops git walking
        // up from the tempdir into whatever contains `$TMPDIR`.
        let dir = tempfile::tempdir().unwrap();
        let parent = dir.path().parent().unwrap().to_path_buf();
        let out = std::process::Command::new("git")
            .args(["rev-parse", "--show-toplevel"])
            .current_dir(dir.path())
            .env("GIT_CEILING_DIRECTORIES", &parent)
            .output()
            .unwrap();
        assert!(
            !out.status.success(),
            "precondition: the tempdir must not be inside a checkout"
        );
        // The code under test runs `git` without the ceiling, so skip when the ambient
        // `$TMPDIR` is itself inside a repo rather than assert something untrue.
        if toplevel(dir.path()).await.is_some() {
            return;
        }
        assert!(fingerprint(dir.path()).await.is_none());
    }

    #[test]
    fn only_a_foreground_read_that_names_no_buffer_is_observed() {
        assert!(should_observe(&json!({"command": "ls", "effect": "read"})));
        assert!(
            !should_observe(&json!({"command": "ls"})),
            "absent effect means write: never observed"
        );
        assert!(!should_observe(
            &json!({"command": "ls", "effect": "write"})
        ));
        assert!(
            !should_observe(&json!({"command": "ls", "effect": "bogus"})),
            "an unknown effect is refused by `call`, so it is not observed either"
        );
        assert!(
            !should_observe(
                &json!({"command": "sleep 1", "effect": "read", "run_in_background": true})
            ),
            "the call returns before a background command finishes"
        );
        assert!(
            !should_observe(&json!({"command": "grep ERROR @cmd_0123abcd", "effect": "read"})),
            "a buffer query is not a tree operation"
        );
        assert!(!should_observe(
            &json!({"command": "@ack_0123abcd", "effect": "read"})
        ));
    }

    async fn ctx_for(root: &Path) -> ToolContext {
        let agent = crate::agent::Agent::new(Some(root.to_path_buf()))
            .await
            .unwrap();
        ToolContext {
            agent,
            lsp: crate::lsp::LspManager::new_arc(),
            output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
            section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
                crate::tools::section_coverage::SectionCoverage::new(),
            )),
            guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
            workspace_override: None,
        }
    }

    /// Drive `RunCommand::call` for real inside the sink's scope and return what the
    /// sink ended with. `None` = nothing observed.
    async fn observed_by_a_real_call(root: &Path, input: Value) -> Option<bool> {
        use crate::tools::Tool;
        let ctx = ctx_for(root).await;
        crate::usage::observed::OBSERVED_SINK
            .scope(RefCell::new(None), async {
                let _ = super::super::RunCommand.call(input, &ctx).await;
                crate::usage::observed::OBSERVED_SINK.with(|s| *s.borrow())
            })
            .await
    }

    /// End to end through the real tool: the command itself writes the file. Paired with
    /// the unchanged case below so neither passes on a constant.
    #[tokio::test]
    async fn a_read_declared_command_that_writes_is_observed_as_changed() {
        let dir = repo();
        let seen = observed_by_a_real_call(
            dir.path(),
            json!({"command": "echo surprise > surprise.txt", "effect": "read"}),
        )
        .await;
        assert_eq!(seen, Some(true));
    }

    #[tokio::test]
    async fn a_read_declared_command_that_only_reads_is_observed_as_unchanged() {
        let dir = repo();
        let seen = observed_by_a_real_call(
            dir.path(),
            json!({"command": "cat tracked.txt", "effect": "read"}),
        )
        .await;
        assert_eq!(seen, Some(false));
    }

    #[tokio::test]
    async fn a_write_declared_command_is_never_observed() {
        let dir = repo();
        let seen = observed_by_a_real_call(
            dir.path(),
            json!({"command": "echo x > w.txt", "effect": "write"}),
        )
        .await;
        assert_eq!(seen, None);
    }

    #[tokio::test]
    async fn a_background_read_is_never_observed() {
        let dir = repo();
        let seen = observed_by_a_real_call(
            dir.path(),
            json!({"command": "cat tracked.txt", "effect": "read", "run_in_background": true}),
        )
        .await;
        assert_eq!(seen, None);
    }

    /// A project that is not a git checkout must neither fail the command nor record a
    /// guess. The ceiling keeps git from finding a repo above the tempdir.
    #[tokio::test]
    async fn a_read_in_a_non_git_project_runs_and_observes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        if toplevel(dir.path()).await.is_some() {
            return; // ambient $TMPDIR is inside a repo; the premise does not hold
        }
        std::fs::write(dir.path().join("f.txt"), "x").unwrap();
        use crate::tools::Tool;
        let ctx = ctx_for(dir.path()).await;
        let (result, seen) = crate::usage::observed::OBSERVED_SINK
            .scope(RefCell::new(None), async {
                let r = super::super::RunCommand
                    .call(json!({"command": "cat f.txt", "effect": "read"}), &ctx)
                    .await;
                (
                    r,
                    crate::usage::observed::OBSERVED_SINK.with(|s| *s.borrow()),
                )
            })
            .await;
        let out = result.expect("the command must still run");
        assert!(
            out.to_string().contains('x'),
            "command output must come through: {out}"
        );
        assert_eq!(seen, None);
    }

    /// A wedged child is abandoned at the budget and does not hold the call: `sleep 30`
    /// against a 100 ms budget must return `None` in well under the 30 s it would take.
    /// The paired positive keeps "returns None" from being what any failure looks like.
    #[tokio::test]
    async fn a_process_that_outlives_its_budget_yields_nothing_promptly() {
        let started = std::time::Instant::now();
        let mut slow = tokio::process::Command::new("sleep");
        slow.arg("30");
        assert!(output_within(slow, Duration::from_millis(100))
            .await
            .is_none());
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "the budget did not bound the wait: {:?}",
            started.elapsed()
        );

        let mut quick = tokio::process::Command::new("echo");
        quick.arg("hi");
        assert_eq!(
            output_within(quick, Duration::from_secs(10)).await,
            Some(b"hi\n".to_vec()),
            "a process inside its budget must still deliver its output"
        );
    }

    /// The claim `git()`'s doc makes: an observation must not rewrite `.git/index`.
    /// Setup makes a tracked file stat-dirty WITHOUT changing its content (a later mtime),
    /// which is exactly the state in which `git status` would refresh and rewrite the index
    /// unless told not to. The index's mtime is first pinned into the past so the
    /// comparison does not depend on the filesystem's clock granularity.
    #[tokio::test]
    async fn a_snapshot_never_rewrites_the_index_it_is_observing() {
        let dir = repo();
        let index = dir.path().join(".git/index");
        let past = std::time::SystemTime::now() - Duration::from_secs(1000);
        std::fs::File::options()
            .write(true)
            .open(&index)
            .unwrap()
            .set_modified(past)
            .unwrap();
        std::fs::File::options()
            .write(true)
            .open(dir.path().join("tracked.txt"))
            .unwrap()
            .set_modified(std::time::SystemTime::now() + Duration::from_secs(5))
            .unwrap();
        let before = std::fs::metadata(&index).unwrap().modified().unwrap();

        let _ = snap(dir.path()).await;

        assert_eq!(
            std::fs::metadata(&index).unwrap().modified().unwrap(),
            before,
            "`git status` refreshed and rewrote the index; GIT_OPTIONAL_LOCKS=0 is not in effect"
        );
    }

    /// A refused call ran no command, so it records no observation: `Err` must not become
    /// "observed, unchanged". An unbounded `rg` piped to a trimmer is refused by the IL-3
    /// gate AFTER the before-snapshot is taken, which is the path that needs this guard.
    #[tokio::test]
    async fn a_refused_call_records_no_observation() {
        let dir = repo();
        let ctx = ctx_for(dir.path()).await;
        use crate::tools::Tool;
        let (result, seen) = crate::usage::observed::OBSERVED_SINK
            .scope(RefCell::new(None), async {
                let r = super::super::RunCommand
                    .call(
                        json!({"command": "rg needle . | head -1", "effect": "read"}),
                        &ctx,
                    )
                    .await;
                (
                    r,
                    crate::usage::observed::OBSERVED_SINK.with(|s| *s.borrow()),
                )
            })
            .await;
        assert!(result.is_err(), "premise: the IL-3 gate must refuse this");
        assert_eq!(seen, None);
    }

    /// An "after" snapshot that cannot be taken records nothing rather than a guess.
    #[tokio::test]
    async fn a_failed_after_snapshot_records_nothing() {
        let dir = repo();
        let before = snap(dir.path()).await;
        let gone = tempfile::tempdir().unwrap();
        let top = gone.path().to_path_buf();
        drop(gone); // the directory no longer exists, so `git status` cannot run in it
        let seen = crate::usage::observed::OBSERVED_SINK
            .scope(RefCell::new(None), async {
                Observation { top, before }.finish().await;
                crate::usage::observed::OBSERVED_SINK.with(|s| *s.borrow())
            })
            .await;
        assert_eq!(seen, None);
    }
}
