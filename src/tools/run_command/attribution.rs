//! Name the author of the uncommitted WIP behind a red, at the moment of the red.
//!
//! # Why this exists
//!
//! On a checkout shared by several Claude Code sessions, uncommitted work that does not
//! compile reds every session's gate. The reader sees a diagnostic pointing at a file
//! they never touched, has no channel to whoever holds it, and routes by proximity —
//! which is *anti*-evidence when three sessions touched the tree in an hour. Measured
//! 2026-09-08: the identifier was sitting in a bug file's `claimed_by` twenty minutes
//! before the red and still went unread, and the cost landed on a third party's ship
//! decision. Full record:
//! `docs/issues/2026-09-08-a-claimed-bug-file-names-the-author-of-the-wip-that-reds-the-build.md`
//! (option 3, the `H`-target `IC-10` records as absent).
//!
//! # Two stages, because the answer is not cheap
//!
//! The engine is `scripts/attribute-red.py`; the ordering and its measurements live
//! there. In one line: `git status` costs 0.01 s and usually ends it, the transcript
//! scan costs ~7 s and runs only when a red actually names a dirty path. This module
//! adds the outermost gate — a zero exit never spawns anything at all.
//!
//! # Why the scripts are EMBEDDED and not read from disk
//!
//! `include_str!` at compile time, materialized to a temp dir on first use. The two
//! alternatives are both wrong:
//!
//! - Resolving `scripts/` under `CARGO_MANIFEST_DIR` at *runtime* points a shipped
//!   binary at a build-machine path that may not exist — or, worse, may exist and hold
//!   something else.
//! - Reading the script from **the workspace being analysed** would hand arbitrary code
//!   execution to any checkout codescout is pointed at, triggered by a failing command.
//!   That is a security hole, not a convenience.
//!
//! Embedding also removes drift by construction: the copy that runs is the copy that
//! was built.
//!
//! # Reachability ceilings — named here, at the site
//!
//! An alarm nothing reaches is exactly as informative as no alarm (`CLAUDE.md`
//! § *Testing Discipline*), so a reader owed the ceiling must find it beside the
//! mechanism rather than discover it later. This hint is **silent**, not absent, in each
//! of these:
//!
//! 1. **Native `Bash` bypasses `run_command` entirely.** A session running its gate
//!    through `Bash` gets nothing and cannot tell that from "nothing to report" — the
//!    sharper failure, because it degrades *silently to reassuring* rather than to an
//!    error.
//!
//!    **The quotable figure is a SESSION count, not a percentage.** Measured 2026-09-08
//!    across 3 profiles and 72 sessions with any cargo activity in this project:
//!    **8 of those 72 run their cargo entirely through `Bash`** and would never receive
//!    this hint once. Coverage is not fungible across sessions — such a session is not
//!    "20% degraded", it gets nothing, and no volume of covered invocations elsewhere
//!    buys it a single hint. One of the eight is the session that nearly shipped against
//!    the stale red this feature was built for.
//!
//!    The call-level ratio (1058 of 5256 invocations, 20.1%) is reported here only as a
//!    **proxy**, and it invites the wrong reading — "~80% covered" is a per-population
//!    claim standing in for a per-member one. Two things bound it further, both worth
//!    more than the number:
//!    - **The right denominator is REDS, not invocations, and it is not measured.** A
//!      green command spawns nothing, so only a failing invocation can carry the hint. A
//!      session running 200 green `cargo` calls through `run_command` and its one failing
//!      gate through `Bash` scores ~99.5% by this proxy and receives zero. The two are
//!      equal only if shell choice is independent of failure, which memory `gotchas`
//!      gives direct reason to doubt (the env divergence makes `cargo test` behave
//!      differently under `Bash`).
//!    - **The denominator is under active manipulation.** `.codescout/project.toml`'s
//!      `shell_command_mode` is a live eval arm comparing the two shells, so these
//!      transcripts span an experiment in flight. Decomposed: only 6 of the 72 sessions
//!      carry the auto-mode directive, and they account for **182 of 1062** `Bash` cargo
//!      runs (17%) — so the lane is mostly *not* mode-driven and will not vanish when the
//!      eval ends, but the figure will move for reasons unrelated to tool ergonomics.
//!
//!    Counted from **transcripts**, never `usage.db`: that database records MCP calls
//!    only and would have returned `Bash = 0` — a clean-looking number, no error, and
//!    exactly backwards. Closing the ceiling needs a `PostToolUse` hook in
//!    `codescout-companion`, cross-repo and inert until its version bumps in every
//!    profile. Reported and decomposed by sessionId
//!    `59112612-5fc8-4b31-8c8c-e19220d99eac`, whose own gate runs are entirely `Bash`;
//!    the per-member correction is `5399543d-22d6-4ed9-9ebb-876be459989f`'s.
//! 2. **A red that exits 0 is invisible.** The gate here is the exit code, so a harness
//!    that swallows a failure into a successful exit never reaches stage 1.
//! 3. **No `python3`, no answer.** The spawn failing and the tree being clean produce the
//!    same silence at this layer.
//!
//! Ceiling 1 is measured; 2 and 3 are structural and cost nothing to state. **The tool's
//! name is wider than its coverage** — it attributes a red *that came through
//! `run_command`* — which is `IC-14`, guard-narrower-than-its-name, and is recorded here
//! rather than left for a reader to discover from the silence.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

const ATTRIBUTE_RED_PY: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/scripts/attribute-red.py"
));
const FILE_PROVENANCE_PY: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/scripts/file-provenance.py"
));

/// Ceiling on stage 3. The scan measured ~7 s on this corpus and grows with it (the bug
/// file recorded 5.0 s six days earlier), so the budget is generous rather than tight —
/// but it is finite, because a pathological transcript set must not hang the tool that
/// was only trying to explain someone else's build failure.
const ATTRIBUTION_TIMEOUT_DEFAULT: Duration = Duration::from_secs(30);

/// Opt-out. Set to any value to disable the hint entirely.
const DISABLE_ENV: &str = "CODESCOUT_NO_WIP_ATTRIBUTION";

/// Override for [`ATTRIBUTION_TIMEOUT_DEFAULT`], in milliseconds.
///
/// A real knob, not a test seam: the scan's cost is a function of a machine's transcript
/// corpus, which this repo has already watched grow 5.0 s -> 7.0 s in six days, so the
/// right ceiling is not a constant anyone here can pick for every machine. That it also
/// makes the timeout branch reachable from a test is deliberate — a branch only a
/// `#[cfg(test)]` construct can reach is a branch the shipped path never proves.
const TIMEOUT_ENV: &str = "CODESCOUT_WIP_ATTRIBUTION_TIMEOUT_MS";

fn attribution_timeout() -> Duration {
    std::env::var(TIMEOUT_ENV)
        .ok()
        .and_then(|v| v.parse().ok())
        .map(Duration::from_millis)
        .unwrap_or(ATTRIBUTION_TIMEOUT_DEFAULT)
}

static SCRIPT_DIR: OnceLock<Option<PathBuf>> = OnceLock::new();

/// Write the embedded scripts to a shared temp directory and return the entry point.
///
/// Keyed by a hash of the contents, so every process built from the same source shares
/// one directory and a rebuilt binary lands in a different one. `DefaultHasher` is not
/// stable across Rust releases and does not need to be: an unstable key costs one extra
/// extraction of 36 KB, never a wrong answer.
///
/// Each file is written to a unique temporary name and then renamed, which is atomic on
/// every platform we run on. Two servers extracting concurrently is the normal case here
/// — a rebuild leaves a dozen sessions to re-materialize at once — and a reader must
/// never observe a half-written script.
fn materialize() -> Option<PathBuf> {
    materialize_into(&std::env::temp_dir())
}

/// The body of [`materialize`], with its base directory as a parameter.
///
/// Split out because the shared directory is keyed by a content hash, which makes the real
/// one *sticky across runs*: a test asserting both scripts get written passed against files
/// an earlier run had left there, and survived a mutation deleting the write entirely. A
/// test that supplies its own empty base observes this function rather than the
/// filesystem's memory of it.
fn materialize_into(base: &Path) -> Option<PathBuf> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut h = DefaultHasher::new();
    ATTRIBUTE_RED_PY.hash(&mut h);
    FILE_PROVENANCE_PY.hash(&mut h);
    let dir = base.join(format!("codescout-attribution-{:016x}", h.finish()));
    std::fs::create_dir_all(&dir).ok()?;

    for (name, body) in [
        ("attribute-red.py", ATTRIBUTE_RED_PY),
        ("file-provenance.py", FILE_PROVENANCE_PY),
    ] {
        let final_path = dir.join(name);
        if std::fs::read_to_string(&final_path).is_ok_and(|got| got == body) {
            continue; // already materialized by this or another process
        }
        let tmp = dir.join(format!("{name}.{}.tmp", std::process::id()));
        std::fs::write(&tmp, body).ok()?;
        // A failed rename leaves the temp file behind rather than a truncated script.
        if std::fs::rename(&tmp, &final_path).is_err() {
            let _ = std::fs::remove_file(&tmp);
            return None;
        }
    }
    Some(dir.join("attribute-red.py"))
}

/// `None` whenever there is nothing to say, and — deliberately — also whenever the tool
/// could not find out. The two are indistinguishable here on purpose: the alternative is
/// emitting a diagnostic about the diagnostic into a failure the reader is already trying
/// to parse. `scripts/attribute-red.py --explain` is where the distinction lives, and it
/// is a thing a human runs, not a thing this path prints.
///
/// What this NEVER does is turn silence into an exoneration. The engine refuses to render
/// "no record" as "not yours"; this wrapper must not undo that by treating its own
/// failures as evidence.
pub(crate) async fn wip_author_diagnostic(
    exit_code: i32,
    red_text: &str,
    work_dir: &Path,
) -> Option<String> {
    // Stage 0 — the outermost gate, and the reason the rest can afford to be expensive.
    // A green command spawns nothing: no python, no git, no extraction.
    if exit_code == 0 {
        return None;
    }
    if std::env::var_os(DISABLE_ENV).is_some() {
        return None;
    }

    let script = SCRIPT_DIR.get_or_init(materialize).clone()?;

    let mut cmd = tokio::process::Command::new("python3");
    cmd.arg(&script)
        .current_dir(work_dir)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null());

    let mut child = cmd.spawn().ok()?;
    if let Some(mut stdin) = child.stdin.take() {
        use tokio::io::AsyncWriteExt;
        // A broken pipe here is the ENGINE deciding it has read enough, not an error:
        // stages 1 and 2 can both conclude before the text is fully consumed.
        let _ = stdin.write_all(red_text.as_bytes()).await;
        let _ = stdin.shutdown().await;
    }

    let out = match tokio::time::timeout(attribution_timeout(), child.wait_with_output()).await {
        Ok(Ok(o)) => o,
        // Timed out or failed. Silent, per the doc above — never "nobody wrote it".
        _ => return None,
    };
    let text = String::from_utf8_lossy(&out.stdout).trim_end().to_string();
    (!text.is_empty()).then_some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The embedded copies must BE the files on disk, or the gate that keeps
    /// `scripts/file-provenance.py`'s tool-name lists current
    /// (`provenance_probes_reference_only_real_tool_names`) is checking a surface this
    /// module does not ship. `include_str!` makes that true at compile time; this asserts
    /// the constants are wired to the paths that gate actually reads, which a typo in
    /// either `concat!` would silently break by embedding some other file.
    #[test]
    fn embedded_scripts_are_the_repo_copies() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        for (rel, embedded) in [
            ("scripts/attribute-red.py", ATTRIBUTE_RED_PY),
            ("scripts/file-provenance.py", FILE_PROVENANCE_PY),
        ] {
            let disk = std::fs::read_to_string(root.join(rel))
                .unwrap_or_else(|e| panic!("{rel} must exist: {e}"));
            assert_eq!(disk, embedded, "{rel} embedded copy diverged from disk");
        }
    }

    /// The engine imports its sibling by name from its own directory, so materializing
    /// one without the other yields an `ImportError` that this module renders as silence
    /// — the exact "plausible answer rather than an error" shape the bug is about.
    #[test]
    fn materialize_writes_both_scripts_beside_each_other() {
        // Its OWN empty base. The production directory is keyed by a content hash and so
        // persists across runs -- this assertion passed against files a previous run had
        // left behind, and survived a mutation deleting the write, until the base became
        // a parameter. A sticky fixture is indistinguishable from a working function.
        let base = tempfile::tempdir().expect("tmp");
        let entry = materialize_into(base.path()).expect("materialization must succeed");
        assert!(entry.ends_with("attribute-red.py"));
        let dir = entry.parent().expect("entry has a parent");
        // The engine imports its sibling by name from its own directory, so a missing
        // sibling is an ImportError this module renders as silence -- the exact
        // "plausible answer rather than an error" shape the bug is about.
        assert_eq!(
            std::fs::read_to_string(dir.join("file-provenance.py")).ok(),
            Some(FILE_PROVENANCE_PY.to_string()),
            "the engine's sibling import target must be materialized too"
        );
        assert_eq!(
            std::fs::read_to_string(dir.join("attribute-red.py")).ok(),
            Some(ATTRIBUTE_RED_PY.to_string()),
        );
        // No `.tmp` may survive a successful run: a leftover means a rename failed and a
        // reader could see a partial file under the real name.
        let strays: Vec<_> = std::fs::read_dir(dir)
            .expect("dir readable")
            .filter_map(Result::ok)
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".tmp"))
            .collect();
        assert!(
            strays.is_empty(),
            "stray temp files left behind: {strays:?}"
        );
    }

    /// Stage 0 is the whole economics: without it every green command in the session pays
    /// a process spawn. Asserted by TIME rather than by reading the branch, because the
    /// claim is about cost and a branch can be present and still slow.
    #[tokio::test]
    async fn a_zero_exit_spawns_nothing() {
        let t = std::time::Instant::now();
        let got = wip_author_diagnostic(0, "error: --> src/lib.rs:1:1", Path::new(".")).await;
        assert!(got.is_none(), "a green command must produce no hint");
        assert!(
            t.elapsed() < Duration::from_millis(50),
            "exit 0 took {:?} — that is a spawn, so the gate is not short-circuiting",
            t.elapsed()
        );
    }

    /// The opt-out must be observed on a red that WOULD otherwise answer.
    ///
    /// The first version of this asserted `is_none()` against a red naming `src/lib.rs`
    /// in this repo -- a path that is not dirty, so stage 2 produced the silence and
    /// deleting the opt-out entirely left the test green. An absence assertion is
    /// satisfied by any mechanism that produces absence, so it has to be paired with a
    /// demonstrated presence: `a_red_naming_a_dirty_file_reaches_the_engine` runs this
    /// same fixture and red WITHOUT the flag and gets output. That pairing is the whole
    /// discrimination -- neither case proves anything alone.
    #[tokio::test]
    async fn the_opt_out_suppresses_a_hint_that_would_otherwise_fire() {
        let Some(dir) = dirty_fixture_repo() else {
            eprintln!("skipping: git unavailable");
            return;
        };
        let red = "error[E0425]: cannot find value `broken`\n  --> src/held.rs:1:13\n";
        // Process-global, and `src/agent/mod.rs`'s EnvGuard warns against exactly this in
        // a default-feature test. Tolerated on a narrow basis: this key is read by one
        // function in one module, no sibling test sets or reads it, and it is removed
        // before the await point that could interleave. Do not widen the pattern.
        std::env::set_var(DISABLE_ENV, "1");
        let got = wip_author_diagnostic(101, red, dir.path()).await;
        std::env::remove_var(DISABLE_ENV);
        assert!(
            got.is_none(),
            "the opt-out must suppress a hint the same input produces without it; got: {got:?}"
        );
    }

    /// The timeout must END the wait, not become an answer.
    ///
    /// Reachable only because the ceiling is an env knob: with a `const` there is no input
    /// that makes a 30 s branch fire inside a test, so `_ => return None` would be
    /// unguarded and a mutation turning it into a verdict survives silently. That branch
    /// is the one place where "we could not find out" is closest to being rendered as
    /// "we found out", which is the failure the whole engine refuses.
    #[tokio::test]
    async fn a_timed_out_scan_is_silence_and_never_a_verdict() {
        let Some(dir) = dirty_fixture_repo() else {
            eprintln!("skipping: git unavailable");
            return;
        };
        let red = "error[E0425]: cannot find value `broken`\n  --> src/held.rs:1:13\n";
        std::env::set_var(TIMEOUT_ENV, "1"); // 1 ms: no process finishes in that
        let got = wip_author_diagnostic(101, red, dir.path()).await;
        std::env::remove_var(TIMEOUT_ENV);
        assert!(
            got.is_none(),
            "a scan that ran out of time must say nothing at all; got: {got:?}"
        );
    }

    /// A fixture repo with a dirty file and NO transcripts anywhere, driven through the
    /// real `wip_author_diagnostic` — materialize, spawn, git, engine, back.
    ///
    /// Hermetic without touching the environment: `transcript_roots()` derives its search
    /// from the repo path, and no profile holds a `projects/-tmp-…` directory for a
    /// tempdir minted seconds ago. So the engine reaches stage 3 and takes the COVERAGE
    /// branch, which is the assertion — the run proves the whole chain executed AND that
    /// an unattributable file is still reported rather than dropped.
    ///
    /// Deliberately NOT an env-var test. `EnvGuard` in `src/agent/mod.rs` carries an
    /// explicit "do NOT copy this pattern into a default-feature test": `set_var` is
    /// process-global and races every sibling thread.
    fn dirty_fixture_repo() -> Option<tempfile::TempDir> {
        let dir = tempfile::tempdir().ok()?;
        let p = dir.path();
        let git = |args: &[&str]| {
            std::process::Command::new("git")
                .arg("-C")
                .arg(p)
                .args(args)
                .output()
                .ok()
                .filter(|o| o.status.success())
        };
        git(&["init", "-q"])?;
        git(&["config", "user.email", "t@t"])?;
        git(&["config", "user.name", "t"])?;
        std::fs::create_dir_all(p.join("src")).ok()?;
        std::fs::write(p.join("src/held.rs"), "fn main() {}\n").ok()?;
        git(&["add", "-A"])?;
        git(&["commit", "-q", "-m", "seed"])?;
        // Dirty AFTER the commit, so the engine's window has a floor to derive and the
        // write is on the live side of it.
        std::fs::write(p.join("src/held.rs"), "fn main() { broken\n").ok()?;
        Some(dir)
    }

    #[tokio::test]
    async fn a_red_naming_a_dirty_file_reaches_the_engine() {
        let Some(dir) = dirty_fixture_repo() else {
            eprintln!("skipping: git unavailable");
            return;
        };
        let red = "error[E0425]: cannot find value `broken`\n  --> src/held.rs:1:13\n";
        let Some(got) = wip_author_diagnostic(101, red, dir.path()).await else {
            // Not an assert: python3 may be absent, and ceiling 3 says that is silence
            // rather than a failure. Loud on stderr so a skipped run is never mistaken
            // for a passing one.
            eprintln!("skipping: no diagnostic (python3 absent?)");
            return;
        };
        assert!(
            got.contains("src/held.rs"),
            "the hint must name the dirty file it is about; got: {got}"
        );
        assert!(
            got.contains("COVERAGE"),
            "an unattributable file must be reported as a coverage gap, never dropped \
             and never exonerated; got: {got}"
        );
        assert!(
            !got.contains("nobody wrote"),
            "absence must never be rendered as an assertion about authorship; got: {got}"
        );
    }

    /// The twin of the case above, on the same fixture. Without it, `exit_code == 0`
    /// could be deleted and only a TIMING assertion would notice — and a timing
    /// assertion is exactly the kind that gets relaxed on a loaded machine.
    #[tokio::test]
    async fn the_same_red_at_exit_zero_says_nothing() {
        let Some(dir) = dirty_fixture_repo() else {
            eprintln!("skipping: git unavailable");
            return;
        };
        let red = "error[E0425]: cannot find value `broken`\n  --> src/held.rs:1:13\n";
        assert!(
            wip_author_diagnostic(0, red, dir.path()).await.is_none(),
            "the identical text at exit 0 must produce nothing"
        );
    }

    /// The field must be RENDERED, not merely stored. `format_run_command` is what
    /// `call_content` shows, so a key it does not read reaches nobody — the defect its
    /// own comment cites for `shell_cause`, one field over.
    #[test]
    fn the_compact_renderer_actually_shows_the_hint() {
        use serde_json::json;
        let rendered = super::super::output::format_run_command(&json!({
            "exit_code": 101,
            "stdout": "",
            "wip_authors": "  src/held.rs\n      written by 22222222-… [LIVE]",
        }));
        assert!(
            rendered.contains("src/held.rs"),
            "the compact render must carry the hint; got: {rendered}"
        );
        assert!(
            rendered.contains("22222222"),
            "including the session it names; got: {rendered}"
        );
    }
}
