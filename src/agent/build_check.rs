//! Author-side build check: tell the author their own uncommitted write broke the
//! shared tree.
//!
//! WHY THIS EXISTS
//! ---------------
//! Several Claude Code sessions share one checkout. When one leaves an uncommitted
//! source edit that does not compile, *every* session's gate goes red — including
//! sessions that never touched the file. `run_command`'s WIP attribution
//! (`crate::tools::run_command::attribution`) is the **reader**-side half: it tells the
//! session that hit the red whose dirty files the failure names.
//!
//! The author is the one party never told, and only the author can end the lock.
//! Measured 2026-09-08: an uncommitted `src/agent/write_guard.rs` reddened peers' gates
//! for ~7 minutes; the author found out from a peer message, minutes later, after two
//! sessions had paid. Full record:
//! `docs/issues/2026-09-08-the-author-of-a-tree-reddening-write-is-the-one-party-never-told.md`
//!
//! WHY IT IS NOT `SessionRegistry`-SHAPED
//! -------------------------------------
//! `crate::librarian::session_registry` sits behind `#[cfg(feature = "librarian")]`
//! (`src/lib.rs:39-40`) while `pub mod agent` (`src/lib.rs:24`) is ungated. Depending on
//! it here would delete this feature from a `--no-default-features` build **and** make
//! every test below invisible to the lean lane — the vacuity CLAUDE.md § Testing
//! Discipline measures. So the *rules* ([`checkout_is_shared`], [`errors_naming`],
//! [`render_notice`]) are pure, ungated, and run in both lanes; only the row *loader* is
//! gated. Consequence, stated rather than discovered later: **a lean build never emits
//! the notice**, because it can enumerate no sessions.
//!
//! COST AND COMMAND, MEASURED — re-derive rather than cite, both track the crate
//! ----------------------------------------------------------------------------
//! The check runs `cargo check --workspace --all-targets`. **`--all-targets` is
//! load-bearing, not thoroughness**, and this was nearly shipped wrong: the incident that
//! motivated this module was an `expect_err` on a `Debug`-less type **inside a
//! `#[cfg(test)]` module**, and plain `cargo check --workspace` does not compile test
//! targets. Measured on this tree 2026-09-09 by reproducing that shape:
//!
//! ```text
//! cargo check --workspace                 exit=0   errors=0   ~3.0 s   <- BLIND
//! cargo check --workspace --all-targets   exit=101 errors=2   ~6.7 s
//! ```
//!
//! A check that omits `--all-targets` is monotone under exactly the failure class it
//! exists to catch, and would have returned clean on the original incident. The gate runs
//! tests, so a broken test module reds peers the same as a broken lib does; the check has
//! to match the gate's blast radius.
//!
//! Incremental cost after one file changes is **~7 s** (`sccache` wrapper; a cold check
//! cache is ~12.6 s, and the first build of a new test module inflates one run to ~21 s —
//! do not read that as steady state). Every reading above carries a control that cargo
//! actually ran (`^\s*(Finished|Checking|Compiling)` line count), because the first
//! attempt at this measurement reported `exit=0` having never invoked cargo at all:
//! `/usr/bin/time` was absent and `tail -5` swallowed the error.
//!
//! CEILINGS — named here because silence looks identical to health
//! ---------------------------------------------------------------
//! * A write that does not go through codescout produces **no trigger**. Native
//!   `Edit`/`Write`/`Bash` reach no tool here. Measured: 8 of 72 sessions run cargo
//!   entirely through `Bash`.
//! * **Rust only.** The trigger is `.rs` + `cargo check`.
//! * A **deliberate mutation** trips this, and mutation runs are routine in this repo.
//!   `CODESCOUT_NO_BUILD_CHECK=1` is the escape.
//! * A **lean build** emits nothing (see above).

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Set to any non-empty value to suppress the check entirely. Mirrors
/// `CODESCOUT_NO_WIP_ATTRIBUTION` on the reader side.
pub(crate) const DISABLE_ENV: &str = "CODESCOUT_NO_BUILD_CHECK";
/// Override the check timeout in milliseconds.
pub(crate) const TIMEOUT_ENV: &str = "CODESCOUT_BUILD_CHECK_TIMEOUT_MS";

const TIMEOUT_DEFAULT: Duration = Duration::from_secs(120);
/// Coalesce an edit burst into one check. Chosen against the measured ~7 s
/// `--all-targets` incremental: long enough that a multi-edit refactor does not queue a
/// check per file, short enough that the author still learns before they move on.
const DEBOUNCE_DEFAULT: Duration = Duration::from_millis(1500);
/// How many failing diagnostics to render. A wall of errors is not more actionable than
/// three, and this notice rides on an unrelated tool's response.
// cap-class: RESULT_CAP agent_build_check.rendered_diagnostics — probed
const MAX_RENDERED: usize = 3;

/// Resolved configuration. One `from_env`, everything else takes values — the
/// `ServerEnv::from_env` shape (`src/server.rs:120-148`). Do **not** reach for
/// `EnvGuard`: `docs/conventions/test-env-isolation.md` marks it NOT VIABLE and forbids
/// copying it into new modules. Tests below construct this struct literally.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BuildCheckEnv {
    pub enabled: bool,
    pub timeout: Duration,
    pub debounce: Duration,
}

impl Default for BuildCheckEnv {
    fn default() -> Self {
        Self {
            enabled: true,
            timeout: TIMEOUT_DEFAULT,
            debounce: DEBOUNCE_DEFAULT,
        }
    }
}

impl BuildCheckEnv {
    pub(crate) fn from_env() -> Self {
        let enabled = std::env::var_os(DISABLE_ENV).is_none();
        let timeout = std::env::var(TIMEOUT_ENV)
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .map(Duration::from_millis)
            .unwrap_or(TIMEOUT_DEFAULT);
        Self {
            enabled,
            timeout,
            debounce: DEBOUNCE_DEFAULT,
        }
    }
}

/// Why a check produced nothing. Kept distinct from `Done { my_break: None }` because
/// "checked, your edit is fine" and "did not check" are different facts — the reader-side
/// twin deliberately collapses them at the *rendering* boundary
/// (`attribution.rs:184-192`), not in its own state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SkipReason {
    /// Another process holds the cargo lock. We never block it.
    LockHeld,
    /// Fewer than two live sessions share this checkout.
    NotShared,
    /// `CODESCOUT_NO_BUILD_CHECK` is set.
    Disabled,
    /// `cargo` could not be spawned.
    NoCargo,
    /// The check exceeded its timeout.
    TimedOut,
}

/// Mirrors [`crate::agent::IndexingState`] on purpose — same shape, same
/// `Arc<Mutex<_>>` storage — so this reads as an existing pattern rather than a new
/// subsystem.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) enum BuildCheckState {
    #[default]
    Idle,
    Running,
    /// A check completed. `my_break` is `Some` only when a compile error named a file
    /// **this session wrote**; a red caused entirely by a peer's dirty file is `None`.
    Done {
        my_break: Option<String>,
        /// Set once the notice has been shown, so it is not repeated on every later call.
        delivered: bool,
    },
    Skipped(SkipReason),
}

/// A live session reduced to the one field the sharing predicate reads.
///
/// Deliberately not `crate::librarian::session_registry::SessionRow` — see the module
/// header for why this module must stay ungated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LiveSession {
    pub cwd: Option<PathBuf>,
}

/// Everything this session knows about its own build health, in one lock.
///
/// Per-session because the codescout server process is per-session — so no disk state,
/// no per-sid record, and nothing to garbage-collect. `Default` is the whole
/// constructor: a session that never writes Rust never leaves `Idle`.
#[derive(Debug, Default)]
pub(crate) struct SessionBuildState {
    /// Absolute paths of source files THIS session wrote. The membership test in
    /// [`errors_naming`] is what keeps a peer's break out of this session's notice.
    pub edits: HashSet<PathBuf>,
    pub state: BuildCheckState,
    /// When the last check started, for [`debounce_elapsed`].
    pub last_started: Option<Instant>,
}

impl SessionBuildState {
    /// Record a source write. Returns whether it is a file worth checking.
    ///
    /// Rust-only, and the ceiling is here rather than in a doc: the trigger is `.rs`
    /// plus `cargo check`, so every other language in this workspace gets nothing.
    pub(crate) fn note_write(&mut self, path: &Path) -> bool {
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            return false;
        }
        self.edits.insert(canonical(path));
        true
    }

    /// May a check start now? Debounce plus "not already running", so an edit burst
    /// yields one check rather than one per file.
    pub(crate) fn may_start(&self, now: Instant, window: Duration) -> bool {
        !matches!(self.state, BuildCheckState::Running)
            && debounce_elapsed(self.last_started, now, window)
    }
}

/// Does this checkout carry work from more than one live session?
///
/// **Counts, and does not identify.** Whether *this* session's own row is in `sessions`
/// does not matter: at two or more, at least one is somebody else. That is the whole
/// reason this feature needs no session identity, and it sidesteps every staleness
/// caveat around `/clear` re-minting ids, `resolve_self` doing no liveness probe, and
/// `.codescout/cc_session_id` being per-project last-writer-wins.
///
/// Callers pass only rows they have already established are LIVE. A dead row here
/// inflates the count toward firing; the conjunction with "your edit broke it" is what
/// keeps the notice rare, not this predicate.
pub(crate) fn checkout_is_shared(sessions: &[LiveSession], root: &Path) -> bool {
    let want = canonical(root);
    sessions
        .iter()
        .filter(|s| s.cwd.as_deref().map(canonical).as_ref() == Some(&want))
        .count()
        >= 2
}

/// Canonicalize for comparison, falling back to the literal path. macOS reaches a
/// tempdir through the `/var` → `/private/var` symlink and Windows `canonicalize` yields
/// the `\\?\` form, so a raw comparison disagrees with itself across platforms — the same
/// trap a since-removed helper in `append_entry.rs` (`ledger_unpushed_commits`, removed
/// 2026-09-11 along with the refusal it served — see
/// docs/issues/archive/2026-09-10-append-entry-refuses-on-unpushed-commits-with-a-remedy-no-session-may-perform.md)
/// once documented identically: canonicalize BOTH sides before comparing, never one.
fn canonical(p: &Path) -> PathBuf {
    p.canonicalize().unwrap_or_else(|_| p.to_path_buf())
}

/// Compile errors from `cargo check --message-format=json` that name a file **this
/// session wrote**, or `None`.
///
/// THIS IS THE CORRECTNESS CORE. Several sessions' dirty files sit in one tree, so a
/// failing check says nothing about *whose* edit broke it. Without the `edited` filter
/// the notice fires on any red anywhere in the tree, tells the author about a peer's
/// break, and is switched off within a day. The mutation that deletes this filter must
/// red a test; if it does not, this function is unguarded.
pub(crate) fn errors_naming(
    edited: &HashSet<PathBuf>,
    cargo_json: &str,
    root: &Path,
) -> Option<String> {
    let edited: HashSet<PathBuf> = edited.iter().map(|p| canonical(p)).collect();
    let mut hits: Vec<String> = Vec::new();

    for line in cargo_json.lines() {
        let line = line.trim();
        if line.is_empty() || !line.starts_with('{') {
            continue;
        }
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if v.get("reason").and_then(|r| r.as_str()) != Some("compiler-message") {
            continue;
        }
        let Some(msg) = v.get("message") else {
            continue;
        };
        if msg.get("level").and_then(|l| l.as_str()) != Some("error") {
            continue;
        }
        let Some(spans) = msg.get("spans").and_then(|s| s.as_array()) else {
            continue;
        };
        for span in spans {
            if span.get("is_primary").and_then(|b| b.as_bool()) != Some(true) {
                continue;
            }
            let Some(file) = span.get("file_name").and_then(|f| f.as_str()) else {
                continue;
            };
            // cargo emits workspace-relative paths; resolve against the root before
            // comparing, or an absolute `edited` entry never matches.
            let abs = canonical(&root.join(file));
            if !edited.contains(&abs) {
                continue;
            }
            let line_no = span.get("line_start").and_then(|n| n.as_u64()).unwrap_or(0);
            let text = msg
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("compile error");
            hits.push(format!("  {file}:{line_no}  {text}"));
            break;
        }
        if hits.len() >= MAX_RENDERED {
            break;
        }
    }

    (!hits.is_empty()).then(|| hits.join("\n"))
}

/// The notice. **Informational, never a request.**
///
/// It states a fact about the author's own working tree and asks for nothing. An
/// automated artefact that asks another party to commit or fix is permission-laundering
/// with a machine's return address — the constraint carried over unchanged from the
/// (unbuildable) messaging design, and it fits a self-collected notice better than it
/// ever fitted a pushed one.
pub(crate) fn render_notice(errors: &str) -> String {
    format!(
        "\n[codescout] your uncommitted edit does not compile, and this checkout is shared \
         with other live sessions.\n{errors}\nThis is a statement about your own working \
         tree, not a request — peers' gates see this break too."
    )
}

/// Is the cargo build lock free right now?
///
/// `false` means another process is building **in this tree**, and we skip rather than
/// wait. `target/` here is 97 GB on a disk 91% full, so a separate `CARGO_TARGET_DIR` is
/// not affordable and the check must share the real one — which makes blocking a way for
/// this feature to become the contention it exists to reduce. Skipping is silence, and
/// silence already means both "nothing to say" and "could not find out" on the reader
/// side, so no new ambiguity is introduced.
///
/// Any I/O failure returns `true` (proceed): a lock we cannot inspect is not one we have
/// grounds to refuse on, and cargo will serialise correctly regardless.
pub(crate) fn cargo_lock_is_free(root: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::io::AsRawFd;
        let path = root.join("target/debug/.cargo-lock");
        let Ok(file) = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
        else {
            return true;
        };
        // SAFETY: `flock` on a live fd owned by `file`, which outlives the call.
        let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
        if rc == 0 {
            // SAFETY: same fd, still owned by `file`.
            unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_UN) };
            true
        } else {
            false
        }
    }
    #[cfg(not(unix))]
    {
        let _ = root;
        true
    }
}

/// Should a check start now, given the last one's age? Pure so the debounce is testable
/// without sleeping.
pub(crate) fn debounce_elapsed(last: Option<Instant>, now: Instant, window: Duration) -> bool {
    match last {
        None => true,
        Some(t) => now.duration_since(t) >= window,
    }
}

/// Run one check and classify the outcome.
///
/// Takes every input as an argument — no env reads, no globals — so the orchestration is
/// exercisable without a process. The shared-checkout gate is the CALLER's: this function
/// is about running, and separating them keeps [`checkout_is_shared`] pure.
///
/// The lock test is advisory and racy by construction: free at the instant we look, held
/// a moment later. That is deliberate — the alternative is holding the lock ourselves
/// across the spawn, which is the blocking this exists to avoid. `env.timeout` bounds the
/// bad case, and a timeout is silence, never a verdict.
pub(crate) async fn run_once(
    root: &Path,
    edited: &HashSet<PathBuf>,
    env: &BuildCheckEnv,
) -> BuildCheckState {
    if !env.enabled {
        return BuildCheckState::Skipped(SkipReason::Disabled);
    }
    if !cargo_lock_is_free(root) {
        return BuildCheckState::Skipped(SkipReason::LockHeld);
    }

    let mut cmd = tokio::process::Command::new("cargo");
    cmd.args([
        "check",
        "--workspace",
        // Load-bearing. Without it a break inside a `#[cfg(test)]` module — the exact
        // shape of the incident this module exists for — compiles clean. See the module
        // header for the measurement.
        "--all-targets",
        "--message-format=json",
    ])
    .current_dir(root)
    .stdin(std::process::Stdio::null())
    .stdout(std::process::Stdio::piped())
    // Diagnostics arrive as JSON on stdout; stderr is cargo's progress chatter and is of
    // no use to a parser.
    .stderr(std::process::Stdio::null());

    let Ok(child) = cmd.spawn() else {
        return BuildCheckState::Skipped(SkipReason::NoCargo);
    };
    let out = match tokio::time::timeout(env.timeout, child.wait_with_output()).await {
        Ok(Ok(o)) => o,
        _ => return BuildCheckState::Skipped(SkipReason::TimedOut),
    };

    let json = String::from_utf8_lossy(&out.stdout);
    BuildCheckState::Done {
        my_break: errors_naming(edited, &json, root),
        delivered: false,
    }
}

/// Live sessions sharing this machine, reduced to the field the predicate reads.
///
/// **This is the only gated part of the module.** With `librarian` off it returns empty,
/// so [`checkout_is_shared`] is false and no notice is ever emitted — stated in the module
/// header rather than left to be discovered. Everything else here compiles and is tested
/// in both lanes.
#[cfg(feature = "librarian")]
pub(crate) fn live_sessions() -> Vec<LiveSession> {
    use crate::librarian::session_registry::{
        default_profile_dirs, RealProcProbe, SessionRegistry,
    };
    let Some(home) = dirs::home_dir() else {
        return Vec::new();
    };
    let registry = SessionRegistry::load(&default_profile_dirs(&home));
    registry
        .live_cwds(&RealProcProbe)
        .into_iter()
        .map(|cwd| LiveSession {
            cwd: cwd.map(PathBuf::from),
        })
        .collect()
}

#[cfg(not(feature = "librarian"))]
pub(crate) fn live_sessions() -> Vec<LiveSession> {
    Vec::new()
}

/// Called by a write tool after it successfully writes a source file.
///
/// Cheap gates first, in the order that costs least — the same staging discipline
/// `attribute-red.py` uses, and for the same reason: the common case must stay free.
/// A non-Rust write, an unshared checkout, or a debounced burst all return without
/// spawning anything.
pub(crate) fn on_source_write(
    slot: &std::sync::Arc<std::sync::Mutex<SessionBuildState>>,
    root: &Path,
    path: &Path,
) {
    let env = BuildCheckEnv::from_env();
    let edits = {
        let Ok(mut st) = slot.lock() else { return };
        // Recorded BEFORE the `enabled` gate, deliberately. `edits` answers "what did this
        // session write", which is true whether or not we go on to check it — and keeping
        // the record unconditional is what lets the wiring test assert on it without
        // reading the environment. A test that branched on `enabled` would carry a
        // skip-guard, and a skip-guard is monotone under the very deletion it exists to
        // catch (measured on this repo 2026-09-09, `59112612`).
        if !st.note_write(path) {
            return;
        }
        if !env.enabled {
            st.state = BuildCheckState::Skipped(SkipReason::Disabled);
            return;
        }
        if !st.may_start(Instant::now(), env.debounce) {
            return;
        }
        // Only reached when a check is actually about to start, so the enumeration —
        // which reads several JSON files — stays off the hot path of an ordinary edit.
        if !checkout_is_shared(&live_sessions(), root) {
            st.state = BuildCheckState::Skipped(SkipReason::NotShared);
            return;
        }
        st.state = BuildCheckState::Running;
        st.last_started = Some(Instant::now());
        st.edits.clone()
    };

    let slot = slot.clone();
    let root = root.to_path_buf();
    tokio::spawn(async move {
        let outcome = run_once(&root, &edits, &env).await;
        if let Ok(mut st) = slot.lock() {
            st.state = outcome;
        }
    });
}

/// The notice to append to this tool response, if the author has an unreported break.
pub(crate) fn pending_notice(
    slot: &std::sync::Arc<std::sync::Mutex<SessionBuildState>>,
) -> Option<String> {
    let mut st = slot.lock().ok()?;
    take_notice(&mut st.state)
}

/// The notice to show on this tool response, if any — and mark it delivered so the next
/// response does not repeat it.
///
/// Takes `&mut` because reading it is what consumes it. A caller that only peeked would
/// re-emit the same notice on every subsequent call, which is how an advisory becomes
/// noise and then gets switched off.
pub(crate) fn take_notice(state: &mut BuildCheckState) -> Option<String> {
    let BuildCheckState::Done {
        my_break: Some(errs),
        delivered,
    } = state
    else {
        return None;
    };
    if *delivered {
        return None;
    }
    let text = render_notice(errs);
    *delivered = true;
    Some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> PathBuf {
        PathBuf::from("/nonexistent-fixture-root")
    }

    fn sess(cwd: &str) -> LiveSession {
        LiveSession {
            cwd: Some(PathBuf::from(cwd)),
        }
    }

    // ---- checkout_is_shared ----

    #[test]
    fn one_session_in_this_checkout_is_not_shared() {
        assert!(!checkout_is_shared(
            &[sess("/w/repo")],
            Path::new("/w/repo")
        ));
    }

    #[test]
    fn two_sessions_in_this_checkout_is_shared() {
        assert!(checkout_is_shared(
            &[sess("/w/repo"), sess("/w/repo")],
            Path::new("/w/repo")
        ));
    }

    /// The count is scoped to THIS checkout. Ten sessions elsewhere on the machine do not
    /// make this tree shared — without the cwd filter the predicate is true on any busy
    /// machine and the notice fires for everyone.
    #[test]
    fn sessions_in_other_checkouts_do_not_make_this_one_shared() {
        let rows = [sess("/w/repo"), sess("/w/other"), sess("/w/elsewhere")];
        assert!(!checkout_is_shared(&rows, Path::new("/w/repo")));
    }

    #[test]
    fn a_session_with_no_recorded_cwd_is_not_counted() {
        let rows = [sess("/w/repo"), LiveSession { cwd: None }];
        assert!(!checkout_is_shared(&rows, Path::new("/w/repo")));
    }

    #[test]
    fn no_sessions_at_all_is_not_shared() {
        assert!(!checkout_is_shared(&[], Path::new("/w/repo")));
    }

    // ---- errors_naming: the correctness core ----

    /// Load-bearing fixture: `file_name` is workspace-RELATIVE and `is_primary` is true,
    /// because that is what cargo emits. Making the path absolute here would let
    /// `errors_naming` pass with its root-joining removed; dropping `is_primary` would
    /// let it pass while matching secondary spans in unrelated files.
    fn cargo_error(file: &str, line: u64, msg: &str) -> String {
        serde_json::json!({
            "reason": "compiler-message",
            "message": {
                "level": "error",
                "message": msg,
                "spans": [{"file_name": file, "line_start": line, "is_primary": true}]
            }
        })
        .to_string()
    }

    fn edited(paths: &[&str]) -> HashSet<PathBuf> {
        paths.iter().map(|p| root().join(p)).collect()
    }

    #[test]
    fn an_error_in_a_file_this_session_wrote_is_reported() {
        let json = cargo_error("src/mine.rs", 12, "mismatched types");
        let got = errors_naming(&edited(&["src/mine.rs"]), &json, &root());
        let got = got.expect("an error in an edited file must be reported");
        assert!(got.contains("src/mine.rs:12"), "{got}");
        assert!(got.contains("mismatched types"), "{got}");
    }

    /// THE test. Deleting the `edited` filter in `errors_naming` must red exactly here.
    /// Without it the notice tells the author about a peer's break, which is a false
    /// alarm generator and gets the feature switched off.
    #[test]
    fn an_error_only_in_a_peers_file_is_not_my_notice() {
        let json = cargo_error("src/theirs.rs", 40, "mismatched types");
        assert_eq!(
            errors_naming(&edited(&["src/mine.rs"]), &json, &root()),
            None
        );
    }

    /// Mixed tree — the common real case. My break must surface and the peer's must not,
    /// which a plain "any error" implementation cannot satisfy in either direction.
    #[test]
    fn a_mixed_red_reports_only_my_file() {
        let json = format!(
            "{}\n{}",
            cargo_error("src/theirs.rs", 40, "peer break"),
            cargo_error("src/mine.rs", 7, "my break")
        );
        let got = errors_naming(&edited(&["src/mine.rs"]), &json, &root()).expect("mine reported");
        assert!(got.contains("my break"), "{got}");
        assert!(
            !got.contains("peer break"),
            "a peer's diagnostic must not appear: {got}"
        );
    }

    /// Warnings are not breaks. Without the `level` check every `#[warn(dead_code)]`
    /// produces a notice, and the feature is noise from its first day.
    #[test]
    fn a_warning_in_my_file_is_not_a_break() {
        let json = serde_json::json!({
            "reason": "compiler-message",
            "message": {
                "level": "warning",
                "message": "unused variable",
                "spans": [{"file_name": "src/mine.rs", "line_start": 3, "is_primary": true}]
            }
        })
        .to_string();
        assert_eq!(
            errors_naming(&edited(&["src/mine.rs"]), &json, &root()),
            None
        );
    }

    /// A secondary span pointing into my file, with the primary elsewhere, is a peer's
    /// error that merely mentions me. Reporting it would name the wrong author.
    #[test]
    fn a_secondary_span_in_my_file_does_not_make_the_error_mine() {
        let json = serde_json::json!({
            "reason": "compiler-message",
            "message": {
                "level": "error",
                "message": "peer break referencing my type",
                "spans": [
                    {"file_name": "src/theirs.rs", "line_start": 9, "is_primary": true},
                    {"file_name": "src/mine.rs", "line_start": 1, "is_primary": false}
                ]
            }
        })
        .to_string();
        assert_eq!(
            errors_naming(&edited(&["src/mine.rs"]), &json, &root()),
            None
        );
    }

    /// cargo interleaves `build-script-executed`, `compiler-artifact` and plain text.
    /// A parser that assumed every line was a diagnostic would panic or mis-count.
    #[test]
    fn non_diagnostic_lines_are_skipped_without_error() {
        let json = format!(
            "{}\n{}\n{}\n{}",
            r#"{"reason":"compiler-artifact","target":{"name":"x"}}"#,
            "warning: unrelated plain text",
            "",
            cargo_error("src/mine.rs", 5, "real break")
        );
        let got = errors_naming(&edited(&["src/mine.rs"]), &json, &root()).expect("reported");
        assert!(got.contains("real break"), "{got}");
    }

    #[test]
    fn a_clean_check_reports_nothing() {
        assert_eq!(errors_naming(&edited(&["src/mine.rs"]), "", &root()), None);
    }

    #[test]
    fn at_most_three_errors_are_rendered() {
        let json = (0..10)
            .map(|i| cargo_error("src/mine.rs", i, "boom"))
            .collect::<Vec<_>>()
            .join("\n");
        let got = errors_naming(&edited(&["src/mine.rs"]), &json, &root()).expect("reported");
        assert_eq!(got.lines().count(), MAX_RENDERED, "{got}");
    }

    // ---- render_notice ----

    /// The constraint is the point of the function, so it gets an assertion rather than a
    /// comment. This checks SHAPE, not wording: it cannot tell you the notice reads well,
    /// only that it has not turned into a demand — which is the regression that matters.
    #[test]
    fn the_notice_states_a_fact_and_asks_for_nothing() {
        let n = render_notice("  src/mine.rs:1  boom");
        assert!(n.contains("src/mine.rs:1"), "{n}");
        assert!(n.contains("not a request"), "{n}");
        for demand in [
            "please ",
            "you should",
            "you must",
            "commit your",
            "fix your",
        ] {
            assert!(
                !n.to_lowercase().contains(demand),
                "notice must not ask for an action, found {demand:?}: {n}"
            );
        }
    }

    // ---- debounce ----

    #[test]
    fn the_first_check_is_never_debounced() {
        assert!(debounce_elapsed(
            None,
            Instant::now(),
            Duration::from_secs(1)
        ));
    }

    #[test]
    fn a_check_inside_the_window_is_debounced() {
        let now = Instant::now();
        let last = now - Duration::from_millis(100);
        assert!(!debounce_elapsed(Some(last), now, Duration::from_secs(1)));
    }

    #[test]
    fn a_check_past_the_window_runs() {
        let now = Instant::now();
        let last = now - Duration::from_secs(5);
        assert!(debounce_elapsed(Some(last), now, Duration::from_secs(1)));
    }

    // ---- env ----

    #[test]
    fn the_default_env_is_enabled_with_the_documented_timeout() {
        let e = BuildCheckEnv::default();
        assert!(e.enabled);
        assert_eq!(e.timeout, TIMEOUT_DEFAULT);
        assert_eq!(e.debounce, DEBOUNCE_DEFAULT);
    }

    // ---- cargo_lock_is_free ----

    /// An absent `target/` is not a held lock. Returning `false` here would make the
    /// check silently never run on a fresh clone — silence that looks like health.
    #[test]
    fn a_missing_target_dir_does_not_read_as_a_held_lock() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(cargo_lock_is_free(tmp.path()));
    }

    #[cfg(unix)]
    #[test]
    fn a_lock_held_by_another_handle_is_observed_as_held() {
        use std::os::unix::io::AsRawFd;
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("target/debug");
        std::fs::create_dir_all(&dir).unwrap();
        let holder = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(dir.join(".cargo-lock"))
            .unwrap();
        // SAFETY: live fd owned by `holder`, unlocked below before it drops.
        assert_eq!(
            unsafe { libc::flock(holder.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) },
            0
        );
        assert!(
            !cargo_lock_is_free(tmp.path()),
            "a lock held elsewhere must read as held, or the check blocks a peer's build"
        );
        // SAFETY: same fd, still owned by `holder`.
        unsafe { libc::flock(holder.as_raw_fd(), libc::LOCK_UN) };
        assert!(cargo_lock_is_free(tmp.path()));
    }

    // ---- run_once: the gates, without needing cargo ----

    #[tokio::test]
    async fn a_disabled_check_never_spawns_cargo() {
        let env = BuildCheckEnv {
            enabled: false,
            ..Default::default()
        };
        let got = run_once(&root(), &edited(&["src/mine.rs"]), &env).await;
        assert_eq!(got, BuildCheckState::Skipped(SkipReason::Disabled));
    }

    /// The skip that keeps this feature from becoming the contention it exists to reduce.
    /// Deleting the `cargo_lock_is_free` guard in `run_once` must red here — otherwise the
    /// check waits on a peer's build, and on a 97 GB shared `target/` that wait is the
    /// whole problem.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_held_cargo_lock_skips_instead_of_waiting() {
        use std::os::unix::io::AsRawFd;
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("target/debug");
        std::fs::create_dir_all(&dir).unwrap();
        let holder = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(dir.join(".cargo-lock"))
            .unwrap();
        // SAFETY: live fd owned by `holder`, unlocked below before it drops.
        assert_eq!(
            unsafe { libc::flock(holder.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) },
            0
        );
        let got = run_once(tmp.path(), &HashSet::new(), &BuildCheckEnv::default()).await;
        // SAFETY: same fd, still owned by `holder`.
        unsafe { libc::flock(holder.as_raw_fd(), libc::LOCK_UN) };
        assert_eq!(got, BuildCheckState::Skipped(SkipReason::LockHeld));
    }

    // ---- take_notice ----

    #[test]
    fn a_break_is_announced_once_and_not_again() {
        let mut st = BuildCheckState::Done {
            my_break: Some("  src/mine.rs:1  boom".to_string()),
            delivered: false,
        };
        let first = take_notice(&mut st).expect("the first read must yield the notice");
        assert!(first.contains("src/mine.rs:1"), "{first}");
        assert_eq!(
            take_notice(&mut st),
            None,
            "a second read must not repeat it — an advisory that reprints on every call \
             becomes noise and then gets switched off"
        );
    }

    #[test]
    fn a_clean_check_announces_nothing() {
        let mut st = BuildCheckState::Done {
            my_break: None,
            delivered: false,
        };
        assert_eq!(take_notice(&mut st), None);
    }

    /// A skip is not a clean bill of health, and must not print one. This is the
    /// reader-side contract held on this side too: silence never means "your tree is
    /// fine", it means we have nothing to say.
    #[test]
    fn a_skip_announces_nothing() {
        for reason in [
            SkipReason::LockHeld,
            SkipReason::NotShared,
            SkipReason::Disabled,
            SkipReason::NoCargo,
            SkipReason::TimedOut,
        ] {
            let mut st = BuildCheckState::Skipped(reason.clone());
            assert_eq!(take_notice(&mut st), None, "{reason:?} must stay silent");
        }
    }

    #[test]
    fn an_idle_or_running_check_announces_nothing() {
        assert_eq!(take_notice(&mut BuildCheckState::Idle), None);
        assert_eq!(take_notice(&mut BuildCheckState::Running), None);
    }
}
