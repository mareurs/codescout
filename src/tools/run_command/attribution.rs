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

/// Every script the engine needs, as `(file name, body)`. The FIRST entry is the entry point.
///
/// The engine reaches its siblings by import, with its own directory first on `sys.path`,
/// so a module that is not written beside it is an `ImportError` — which `attribute-red.py`
/// answers with exit 0 and no stdout, and which `wip_author_diagnostic_with` reads as
/// "nothing to say". `eed25579` (2026-10-11) added four sibling modules to
/// `file-provenance.py` while this list held two, and the hint died silently.
///
/// `the_materialized_set_is_closed_under_its_own_local_imports` derives the required set
/// from the imports of what is actually written, so a module added to the engine without
/// being added here reds a test instead of a user's red.
const EMBEDDED_SCRIPTS: [(&str, &str); 6] = [
    (
        "attribute-red.py",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/scripts/attribute-red.py"
        )),
    ),
    (
        "file-provenance.py",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/scripts/file-provenance.py"
        )),
    ),
    (
        "claude_history.py",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/scripts/claude_history.py"
        )),
    ),
    (
        "codex_history.py",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/scripts/codex_history.py"
        )),
    ),
    (
        "provenance_records.py",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/scripts/provenance_records.py"
        )),
    ),
    (
        "session_context.py",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/scripts/session_context.py"
        )),
    ),
];

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

/// Resolved configuration for the attribution hint. One `from_env`, everything else takes
/// values — the `BuildCheckEnv` / `ServerEnv::from_env` shape that
/// `docs/conventions/test-env-isolation.md` § *Established exemplars* prescribes. Tests
/// construct this literally and mutate nothing.
///
/// **Both knobs were read from the process environment inside the async body until
/// 2026-09-13, and two tests drove them with `set_var`.** `cargo test` runs one binary with
/// a thread per core, so a test mutating the environment mutates every sibling's input:
/// six call sites reach [`wip_author_diagnostic`], four of them assert a POSITIVE answer,
/// and any of those scheduled inside the opt-out test's window got `None` and failed on an
/// assertion about its own behaviour — for a reason living in a different test. The comment
/// at that site argued the pattern was safe on two grounds and both were false: the
/// `remove_var` was AFTER the await, not before it, and the siblings did read the key,
/// because they all call the one function that reads it.
///
/// `#[serial]` is NOT the remedy, even though it is already a dependency and would make the
/// symptom go away: `set_var` while another thread calls `getenv` is UB (glibc may
/// `realloc` `environ`), and `#[serial]` locks only against tests that opt in. See
/// `src/config/global.rs` and `src/librarian/tools/audit_doc_refs/mod.rs`, which each
/// removed this same pattern for that reason.
/// `docs/issues/archive/2026-09-13-an-env-var-set-across-an-await-races-every-sibling-test-that-reads-it.md`
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AttributionEnv {
    /// False when [`DISABLE_ENV`] is set to any value at all.
    pub enabled: bool,
    /// Ceiling on the scan — see [`ATTRIBUTION_TIMEOUT_DEFAULT`].
    pub timeout: Duration,
}

impl Default for AttributionEnv {
    fn default() -> Self {
        Self {
            enabled: true,
            timeout: ATTRIBUTION_TIMEOUT_DEFAULT,
        }
    }
}

impl AttributionEnv {
    /// The only thing in this module that reads the environment. Called once, at the edge;
    /// the result is handed inward.
    pub(crate) fn from_env() -> Self {
        Self {
            enabled: std::env::var_os(DISABLE_ENV).is_none(),
            timeout: std::env::var(TIMEOUT_ENV)
                .ok()
                .and_then(|v| v.parse().ok())
                .map(Duration::from_millis)
                .unwrap_or(ATTRIBUTION_TIMEOUT_DEFAULT),
        }
    }
}

static SCRIPT_DIR: OnceLock<Option<PathBuf>> = OnceLock::new();

/// Write the embedded scripts to a per-user, verified temp directory and return the entry point.
///
/// Keyed by the effective uid and a hash of the contents, so every process of one user built
/// from the same source shares one directory and a rebuilt binary lands in a different one.
/// `DefaultHasher` is not stable across Rust releases and does not need to be: an unstable key
/// costs one extra extraction of 120 KB, never a wrong answer. The directory is created 0700
/// and re-verified on every use (a real directory, ours, not group/other writable) — see
/// [`script_dir`] — because the name is computable by anyone and `/tmp` may be shared.
///
/// Each file is written to a unique temporary name and then renamed, which is atomic on
/// every platform we run on. Two servers extracting concurrently is the normal case here
/// — a rebuild leaves a dozen sessions to re-materialize at once — and a reader must
/// never observe a half-written script.
fn materialize() -> Option<PathBuf> {
    materialize_into(&std::env::temp_dir())
}

type Uid = u32;

fn effective_uid() -> Uid {
    #[cfg(unix)]
    {
        // SAFETY: geteuid has no preconditions and cannot fail.
        unsafe { libc::geteuid() }
    }
    #[cfg(not(unix))]
    {
        0
    }
}

fn script_set_hash() -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut h = DefaultHasher::new();
    for (name, body) in EMBEDDED_SCRIPTS {
        name.hash(&mut h);
        body.hash(&mut h);
    }
    h.finish()
}

/// The deterministic extraction directory for `uid` under `base`.
///
/// The name carries the effective uid so two users of one shared `/tmp` never contend for
/// (or can squat on) each other's directory. The hash half stays computable by anyone —
/// `DefaultHasher::new()` has fixed keys — so the NAME is not a secret and protects nothing
/// by itself: [`verify_private_dir`] is what makes the directory trustworthy.
fn extraction_dir(base: &Path, uid: Uid) -> PathBuf {
    base.join(format!(
        "codescout-attribution-{uid}-{:016x}",
        script_set_hash()
    ))
}

/// Why [`verify_private_dir`] will not trust a directory. Rendered into the log line that
/// accompanies a fallback, so each variant names what an operator would act on.
#[derive(Debug, PartialEq, Eq)]
enum DirRefusal {
    /// Missing, unreadable, a symlink (including one to a perfectly good directory), or a
    /// non-directory squatting on the name. `symlink_metadata` is what makes a link visible:
    /// plain `metadata` would follow it and report the target's owner and mode.
    NotARealDirectory,
    /// Owned by someone other than the effective user, who can therefore put anything in it.
    #[cfg(unix)]
    ForeignOwner { owner: Uid, expected: Uid },
    /// Writable by group or other, so a third party can plant a module beside the scripts.
    #[cfg(unix)]
    GroupOrOtherWritable { mode: u32 },
}

impl std::fmt::Display for DirRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotARealDirectory => write!(f, "not a real directory (missing, symlink or file)"),
            #[cfg(unix)]
            Self::ForeignOwner { owner, expected } => {
                write!(f, "owned by uid {owner}, not by uid {expected}")
            }
            #[cfg(unix)]
            Self::GroupOrOtherWritable { mode } => {
                write!(f, "mode {mode:04o} is writable by group or other")
            }
        }
    }
}

/// Is `dir` a directory only `uid` (or root) can put files in?
///
/// The scripts in it run as this user, so anyone who can add a file beside them can run code
/// as this user: Python puts the script's directory first on `sys.path`. Checked on EVERY use,
/// not only on creation, because on a shared `/tmp` another user may have created the name
/// first — the check is the trust, the name is not.
///
/// Unix only beyond the symlink check: owner and mode bits do not exist elsewhere, and a
/// Windows `%TEMP%` is per-user to begin with. (Not compiled or run on Windows from here;
/// only the `cfg(not(unix))` arm is different, and it is a subset.)
fn verify_private_dir(dir: &Path, uid: Uid) -> Result<(), DirRefusal> {
    let meta = std::fs::symlink_metadata(dir).map_err(|_| DirRefusal::NotARealDirectory)?;
    if !meta.file_type().is_dir() {
        return Err(DirRefusal::NotARealDirectory);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let owner = meta.uid();
        if owner != uid {
            return Err(DirRefusal::ForeignOwner {
                owner,
                expected: uid,
            });
        }
        let mode = meta.mode() & 0o777;
        if mode & 0o022 != 0 {
            return Err(DirRefusal::GroupOrOtherWritable { mode });
        }
    }
    #[cfg(not(unix))]
    let _ = uid;
    Ok(())
}

/// Create `dir` with mode 0700; an existing entry of that name is NOT an error here — it is
/// [`verify_private_dir`]'s to judge. Not recursive, so the mode applies to the directory
/// that matters and never to a parent created on the way.
fn create_private_dir(dir: &Path) -> std::io::Result<()> {
    let mut builder = std::fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    match builder.create(dir) {
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
        other => other,
    }
}

/// The directory the engine is extracted into: the deterministic per-user one when it
/// passes [`verify_private_dir`] (the normal case, and the cache-reusing one), otherwise a
/// fresh private directory this call created.
///
/// The fallback is what a refusal costs: attribution keeps working under an attack on the
/// name, at the price of one 120 KB extraction and one leaked directory for this process
/// (it must outlive the process's spawns, and statics are not dropped at exit). The refusal is
/// logged at `warn`, naming the path and the reason — the observer is whoever reads the
/// server log, since the hint itself has no channel for "your temp directory was squatted".
fn script_dir(base: &Path, uid: Uid) -> Option<PathBuf> {
    let _ = std::fs::create_dir_all(base);
    let dir = extraction_dir(base, uid);
    let refusal = match create_private_dir(&dir) {
        Ok(()) => match verify_private_dir(&dir, uid) {
            Ok(()) => return Some(dir),
            Err(r) => r.to_string(),
        },
        Err(e) => format!("cannot create it: {e}"),
    };
    tracing::warn!(
        dir = %dir.display(),
        "attribution: refusing the extraction directory ({refusal}); using a fresh private one"
    );
    fresh_private_dir(base, uid)
}

/// A directory this call created, under a name nobody could have predicted (`mkdtemp`), so
/// there was nothing to pre-create. Verified like any other, then deliberately kept.
///
/// Mode 0700 is requested explicitly: `tempfile` creates DIRECTORIES with the umask's view of
/// 0777 (measured 2026-10-17: 0755 under umask 022), which under umask 002 is group-writable
/// and would fail [`verify_private_dir`] — leaving no fallback at all on exactly the hosts
/// (user-private-group distributions) most likely to have a shared group.
fn fresh_private_dir(base: &Path, uid: Uid) -> Option<PathBuf> {
    let mut builder = tempfile::Builder::new();
    builder.prefix("codescout-attribution-");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        builder.permissions(std::fs::Permissions::from_mode(0o700));
    }
    let tmp = builder.tempdir_in(base).ok()?;
    verify_private_dir(tmp.path(), uid).ok()?;
    Some(tmp.keep())
}

/// The engine's command line: `python3 -I <script>`.
///
/// `-I` (isolated) drops the script's directory from `sys.path` and ignores `PYTHON*`
/// variables and the user site, so the engine's OWN imports (`json`, `re`, `subprocess`,
/// `pathlib`, ...) cannot be satisfied by a file beside it. It is hardening, not the
/// defence: `file-provenance.py` re-inserts its directory at `sys.path[0]` (line 91) to
/// import its sibling modules, so a module planted there and first imported AFTER that
/// line (measured: `typing`) still runs under `-I`. What makes the directory safe is
/// [`verify_private_dir`]. Built here, once, so the shipped path and the tests share it.
fn engine_std_command(script: &Path) -> std::process::Command {
    let mut cmd = std::process::Command::new("python3");
    cmd.arg("-I").arg(script);
    cmd
}

/// The body of [`materialize`], with its base directory as a parameter.
///
/// Split out because the shared directory is keyed by a content hash, which makes the real
/// one *sticky across runs*: a test asserting both scripts get written passed against files
/// an earlier run had left there, and survived a mutation deleting the write entirely. A
/// test that supplies its own empty base observes this function rather than the
/// filesystem's memory of it.
fn materialize_into(base: &Path) -> Option<PathBuf> {
    let dir = script_dir(base, effective_uid())?;

    for (name, body) in EMBEDDED_SCRIPTS {
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
    Some(dir.join(EMBEDDED_SCRIPTS[0].0))
}

/// Turn the engine's finished process into the line a red carries, if any.
///
/// The engine exits 0 for every outcome it chooses, so only a non-zero exit — an uncaught
/// exception, or a signal — is a crash. A crash is reported as a TOOLING failure naming the
/// last line of stderr, never as silence: silence is what the engine says for "nothing to
/// attribute", and the two must not read the same. A timeout is a different case and stays
/// silent (`wip_author_diagnostic_with`).
/// docs/issues/archive/2026-10-14-a-failure-of-the-red-attribution-helper-is-discarded-twice-and-reads-as-nothing-to-say.md
fn engine_outcome(
    success: bool,
    code: Option<i32>,
    stdout: &[u8],
    stderr: &[u8],
) -> Option<String> {
    if !success {
        let cause = String::from_utf8_lossy(stderr)
            .lines()
            .rev()
            .find(|l| !l.trim().is_empty())
            .map(|l| l.trim().chars().take(200).collect::<String>())
            .unwrap_or_else(|| "no stderr".to_string());
        let how = match code {
            Some(c) => format!("exited {c}"),
            None => "was ended by a signal".to_string(),
        };
        return Some(format!(
            "[codescout] red attribution is unavailable: its helper {how} ({cause}). \
             This is a tooling failure, NOT evidence that the red is nobody's."
        ));
    }
    let text = String::from_utf8_lossy(stdout).trim_end().to_string();
    (!text.trim().is_empty()).then_some(text)
}

/// Does this text look like it NAMES a file in a failure?
///
/// A cheap, allocation-free **necessary condition** for `attribute-red.py`'s
/// `DIAGNOSTIC_PATH` list — deliberately WIDER than it, never narrower. Each engine
/// pattern requires one distinctive literal, and this checks for those literals without
/// the span, filename or line-number parts:
///
/// | engine pattern | required literal | checked here |
/// |---|---|---|
/// | `^\s*-->\s+path:N:N`            | `-->`          | `contains` |
/// | `panicked at '?path.rs:N`       | `panicked at`  | `contains` |
/// | `panicked at path.rs:N`         | `panicked at`  | `contains` |
/// | `^error(\[…\])?:.*path.rs`      | line-initial `error` | `starts_with` |
/// | `^\s*Error:\s+path.(rs\|py\|sh\|md)` | `Error:` after optional ws | `trim_start` |
///
/// **Wider is the load-bearing direction.** A false positive costs one python spawn, and
/// the engine then answers correctly anyway. A false negative is a silent wrong answer,
/// which is the entire defect this gate exists to close — so if these two ever disagree,
/// they must disagree in this direction.
///
/// **This duplicates knowledge that lives in Python, and the duplication is guarded rather
/// than trusted.** `the_prefilter_admits_every_shape_the_engine_matches` carries one
/// fixture per engine pattern, and `the_engine_pattern_count_matches_the_prefilters_fixtures`
/// reds when a sixth pattern is added to `scripts/attribute-red.py` without a fixture here
/// — so drift surfaces as a failing test naming the file to edit, not as a silent
/// narrowing. (The same shape as `build.rs`'s hand-copied `extract_surface`, which this
/// repo has already paid for once.)
fn names_a_diagnostic(text: &str) -> bool {
    if text.contains("-->") || text.contains("panicked at") {
        return true;
    }
    // `^error` in the engine is start-of-line with NO leading whitespace; `^\s*Error:`
    // allows it. Mirrored exactly rather than collapsed to one trim, because collapsing
    // would be wider on one and is free to be — but writing the asymmetry down is what
    // lets the next reader check it against the table above.
    text.lines()
        .any(|l| l.starts_with("error") || l.trim_start().starts_with("Error:"))
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
///
/// This is the shipped edge: it resolves [`AttributionEnv`] from the process environment
/// once and hands it to [`wip_author_diagnostic_with`]. Tests that need to drive either
/// knob call that directly with a struct they built — never by mutating the environment,
/// which is shared with every concurrently running test.
pub(crate) async fn wip_author_diagnostic(
    exit_code: i32,
    red_text: &str,
    work_dir: &Path,
) -> Option<String> {
    wip_author_diagnostic_with(exit_code, red_text, work_dir, &AttributionEnv::from_env()).await
}

/// The body of [`wip_author_diagnostic`], with its configuration as a parameter.
///
/// Split out so the opt-out and timeout branches are reachable from a test **without**
/// process-global state. The knobs stay real environment variables on the shipped path —
/// the timeout especially is a genuine operator knob, since the scan's cost is a function
/// of a machine's transcript corpus — but resolving them is [`AttributionEnv::from_env`]'s
/// job, at the edge, exactly once.
pub(crate) async fn wip_author_diagnostic_with(
    exit_code: i32,
    red_text: &str,
    work_dir: &Path,
    env: &AttributionEnv,
) -> Option<String> {
    // Stage 0 — the outermost gate, and the reason the rest can afford to be expensive.
    // A green command with nothing failure-shaped in its output spawns nothing: no
    // python, no git, no extraction.
    //
    // It is NOT `exit_code == 0` alone, and that was the defect. `;` sequencing makes the
    // last statement's status the whole command's, so `<red> ; echo "LEAN exit=$?"` exits
    // 0 with the real 101 sitting in stdout as text — and CLAUDE.md § *Development
    // Commands* MANDATES that shape, for a good reason unrelated to exits (`&&` would skip
    // the default-lane rebuild that clears the librarian-less-binary trap). So the one
    // command sequence every session is told to run was the one this hook could not see,
    // while § *Reaching a Peer Session* told readers the attached line replaced going
    // looking. An unnamed ceiling under that instruction converts into a confident wrong
    // answer: measured 2026-09-12, a reader with the full red on screen and no attribution
    // beside it routed it by ADJACENCY and named the wrong owner.
    if exit_code == 0 && !names_a_diagnostic(red_text) {
        return None;
    }
    if !env.enabled {
        return None;
    }

    let script = SCRIPT_DIR.get_or_init(materialize).clone()?;

    let mut cmd = tokio::process::Command::from(engine_std_command(&script));
    cmd.current_dir(work_dir)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let mut child = cmd.spawn().ok()?;
    if let Some(mut stdin) = child.stdin.take() {
        use tokio::io::AsyncWriteExt;
        // A broken pipe here is the ENGINE deciding it has read enough, not an error:
        // stages 1 and 2 can both conclude before the text is fully consumed.
        let _ = stdin.write_all(red_text.as_bytes()).await;
        let _ = stdin.shutdown().await;
    }

    let out = match tokio::time::timeout(env.timeout, child.wait_with_output()).await {
        Ok(Ok(o)) => o,
        // Timed out or failed. Silent, per the doc above — never "nobody wrote it".
        _ => return None,
    };
    engine_outcome(
        out.status.success(),
        out.status.code(),
        &out.stdout,
        &out.stderr,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Is a `python3` available to run the engine at all?
    ///
    /// The engine tests below may only treat a missing diagnostic as a skip when this is
    /// false. Before 2026-10-13 they skipped on ANY missing diagnostic and printed
    /// "python3 absent?" — a question mark standing where a check belonged. With python3
    /// present, a helper that cannot import its own modules (see
    /// `docs/issues/archive/2026-10-13-embedded-attribution-helper-ships-two-of-its-six-modules-so-a-red-loses-its-author-silently.md`)
    /// produces exactly that missing diagnostic, so those tests went green while the
    /// feature was dead.
    fn python3_present() -> bool {
        std::process::Command::new("python3")
            .arg("--version")
            .output()
            .is_ok_and(|o| o.status.success())
    }

    /// `Some(name)` for each local sibling module that `body` imports and that exists
    /// under `scripts/` in the repo. Stdlib and third-party names never match a file
    /// there, so this needs no allowlist.
    fn local_imports(body: &str, repo_scripts: &Path) -> Vec<String> {
        body.lines()
            .filter_map(|line| {
                let t = line.trim_start();
                let rest = t
                    .strip_prefix("from ")
                    .or_else(|| t.strip_prefix("import "))?;
                let module: String = rest
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                (!module.is_empty() && repo_scripts.join(format!("{module}.py")).is_file())
                    .then_some(module)
            })
            .collect()
    }

    /// The embedded copies must BE the files on disk, or the gate that keeps
    /// `scripts/file-provenance.py`'s tool-name lists current
    /// (`provenance_probes_reference_only_real_tool_names`) is checking a surface this
    /// module does not ship. `include_str!` makes that true at compile time; this asserts
    /// the constants are wired to the paths that gate actually reads, which a typo in
    /// either `concat!` would silently break by embedding some other file.
    #[test]
    fn embedded_scripts_are_the_repo_copies() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        for (name, embedded) in EMBEDDED_SCRIPTS {
            let rel = format!("scripts/{name}");
            let disk = std::fs::read_to_string(root.join(&rel))
                .unwrap_or_else(|e| panic!("{rel} must exist: {e}"));
            assert_eq!(disk, embedded, "{rel} embedded copy diverged from disk");
        }
    }

    /// The engine imports its sibling by name from its own directory, so materializing
    /// one without the other yields an `ImportError` that this module renders as silence
    /// — the exact "plausible answer rather than an error" shape the bug is about.
    #[test]
    fn materialize_writes_every_embedded_script_beside_the_entry() {
        // Its OWN empty base. The production directory is keyed by a content hash and so
        // persists across runs -- this assertion passed against files a previous run had
        // left behind, and survived a mutation deleting the write, until the base became
        // a parameter. A sticky fixture is indistinguishable from a working function.
        let base = tempfile::tempdir().expect("tmp");
        let entry = materialize_into(base.path()).expect("materialization must succeed");
        assert!(entry.ends_with("attribute-red.py"));
        let dir = entry.parent().expect("entry has a parent");
        // The engine imports its siblings by name from its own directory, so a missing
        // sibling is an ImportError this module renders as silence -- the exact
        // "plausible answer rather than an error" shape the bug is about.
        for (name, body) in EMBEDDED_SCRIPTS {
            assert_eq!(
                std::fs::read_to_string(dir.join(name)).ok(),
                Some(body.to_string()),
                "{name} must be materialized beside the entry point"
            );
        }
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

    /// A module planted beside the entry point must not be importable by the engine.
    ///
    /// Python puts the script's own directory first on `sys.path`, and `attribute-red.py`
    /// imports `json`, `re`, `subprocess` and `pathlib` (lines 51-57), so a `json.py` in the
    /// extraction directory runs as the codescout user. The CONTROL half runs the same
    /// directory under a plain `python3 <script>` and requires the plant to FIRE: without it a
    /// green here would also be what a plant that never fires (a typo in the marker path, a
    /// module the engine does not import) produces — a test monotone under the very change it
    /// guards. The guarded half goes through [`engine_std_command`], the constructor the
    /// shipped path uses, so removing `-I` from it reds this.
    ///
    /// What `-I` does NOT close, measured 2026-10-17: `file-provenance.py` re-inserts its own
    /// directory at `sys.path[0]` (line 91), so a planted `typing.py` still runs under `-I`
    /// (that is `from typing import Iterable` at line 95, reached only after the insert).
    /// The directory checks below, not `-I`, are what close the hole; this test pins only the
    /// half `-I` does close.
    #[test]
    fn a_planted_module_beside_the_entry_is_not_imported() {
        if !python3_present() {
            eprintln!("skipping: python3 absent");
            return;
        }
        let Some(repo) = dirty_fixture_repo() else {
            eprintln!("skipping: git unavailable");
            return;
        };
        let base = tempfile::tempdir().expect("tmp");
        let entry = materialize_into(base.path()).expect("materialization must succeed");
        let dir = entry.parent().expect("entry has a parent").to_path_buf();
        let marker = base.path().join("json-py-ran");
        std::fs::write(
            dir.join("json.py"),
            format!(
                "open({:?}, 'w').write('ran')\nraise ImportError('planted json')\n",
                marker.display().to_string()
            ),
        )
        .expect("plant");

        // CONTROL: plain python3 imports the plant.
        let control = std::process::Command::new("python3")
            .arg("-B")
            .arg(&entry)
            .arg("--explain")
            .current_dir(repo.path())
            .stdin(std::process::Stdio::null())
            .output()
            .expect("python3 spawns");
        assert!(
            marker.exists(),
            "control: the plant must run under a plain `python3 <script>` or this test proves \
             nothing; stderr: {}",
            String::from_utf8_lossy(&control.stderr)
        );
        std::fs::remove_file(&marker).expect("clear marker");

        // GUARDED: the shipped constructor.
        let out = engine_std_command(&entry)
            .arg("--explain")
            .current_dir(repo.path())
            .stdin(std::process::Stdio::null())
            .output()
            .expect("python3 spawns");
        assert!(
            !marker.exists(),
            "a json.py planted beside the entry point ran under the shipped command line; \
             stderr: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        // End to end: the engine must still WORK under the isolated command line. A crash or an
        // import failure would also leave the marker absent.
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(
            out.status.success() && !stdout.contains("could not be imported"),
            "the engine must run to completion under the shipped command line; \
             status {:?} stdout: {stdout} stderr: {}",
            out.status,
            String::from_utf8_lossy(&out.stderr)
        );
    }

    /// The extraction directory is the trust root for code that runs as this user, so every use
    /// of it is verified. Unix-only: the owner and mode bits do not exist elsewhere, and a
    /// Windows `%TEMP%` is per-user to begin with.
    #[cfg(unix)]
    mod dir_trust {
        use super::*;
        use std::os::unix::fs::{symlink, MetadataExt, PermissionsExt};

        fn mode_of(p: &Path) -> u32 {
            std::fs::symlink_metadata(p).expect("stat").mode() & 0o777
        }

        fn script_names_in(dir: &Path) -> Vec<String> {
            EMBEDDED_SCRIPTS
                .iter()
                .map(|(n, _)| n.to_string())
                .filter(|n| dir.join(n).exists())
                .collect()
        }

        fn assert_complete_and_private(dir: &Path) {
            for (name, body) in EMBEDDED_SCRIPTS {
                assert_eq!(
                    std::fs::read_to_string(dir.join(name)).ok().as_deref(),
                    Some(body),
                    "{name} must be materialized in {}",
                    dir.display()
                );
            }
            assert_eq!(
                verify_private_dir(dir, effective_uid()),
                Ok(()),
                "the directory the engine runs from must itself pass the trust check"
            );
        }

        #[test]
        fn a_fresh_extraction_directory_is_private_and_ours() {
            let base = tempfile::tempdir().expect("tmp");
            let entry = materialize_into(base.path()).expect("materialize");
            let dir = entry.parent().expect("parent");
            assert_eq!(dir, extraction_dir(base.path(), effective_uid()));
            // Exactly 0700, not merely "not writable by others": umask 022 would turn a
            // missing mode into 0755, which the write-bit check alone accepts.
            assert_eq!(
                mode_of(dir),
                0o700,
                "created private, not left to the umask"
            );
            assert_eq!(std::fs::metadata(dir).expect("stat").uid(), effective_uid());
            assert_complete_and_private(dir);
        }

        /// The normal single-user path keeps its cache: the second call finds the identical
        /// scripts and rewrites nothing (a rename would change the inode).
        #[test]
        fn a_second_call_reuses_the_directory_and_rewrites_nothing() {
            let base = tempfile::tempdir().expect("tmp");
            let first = materialize_into(base.path()).expect("first");
            let inodes = |dir: &Path| -> Vec<u64> {
                EMBEDDED_SCRIPTS
                    .iter()
                    .map(|(n, _)| std::fs::metadata(dir.join(n)).expect("stat").ino())
                    .collect()
            };
            let before = inodes(first.parent().unwrap());
            let second = materialize_into(base.path()).expect("second");
            assert_eq!(first, second, "same directory on reuse");
            assert_eq!(
                before,
                inodes(second.parent().unwrap()),
                "nothing rewritten"
            );
        }

        /// CONTROL for the refusals below: a pre-existing directory that IS trustworthy is
        /// reused, including one that others can read (the scripts are public). Without it the
        /// refusals are indistinguishable from a function that refuses everything.
        #[test]
        fn a_trustworthy_preexisting_directory_is_reused() {
            for mode in [0o700, 0o755, 0o750] {
                let base = tempfile::tempdir().expect("tmp");
                let pre = extraction_dir(base.path(), effective_uid());
                std::fs::create_dir(&pre).expect("create");
                std::fs::set_permissions(&pre, std::fs::Permissions::from_mode(mode))
                    .expect("chmod");
                let entry = materialize_into(base.path()).expect("materialize");
                assert_eq!(
                    entry.parent().unwrap(),
                    pre,
                    "a directory of mode {mode:o} owned by us must be reused"
                );
            }
        }

        #[test]
        fn a_symlinked_extraction_directory_is_refused() {
            let base = tempfile::tempdir().expect("tmp");
            let elsewhere = base.path().join("elsewhere");
            std::fs::create_dir(&elsewhere).expect("create");
            std::fs::set_permissions(&elsewhere, std::fs::Permissions::from_mode(0o700))
                .expect("chmod");
            std::fs::write(elsewhere.join("json.py"), "raise ImportError('planted')\n")
                .expect("plant");
            let link = extraction_dir(base.path(), effective_uid());
            symlink(&elsewhere, &link).expect("symlink");

            let entry = materialize_into(base.path()).expect("falls back, not fails");
            let dir = entry.parent().unwrap();
            assert_ne!(dir, link, "the symlink must not be used");
            assert!(
                !dir.starts_with(&elsewhere),
                "the entry must not live behind the link"
            );
            assert!(
                script_names_in(&elsewhere).is_empty(),
                "nothing may be written through the link; found {:?}",
                script_names_in(&elsewhere)
            );
            assert_complete_and_private(dir);
        }

        /// A regular file squatting on the name is refused too.
        #[test]
        fn a_file_squatting_on_the_directory_name_is_refused() {
            let base = tempfile::tempdir().expect("tmp");
            let squat = extraction_dir(base.path(), effective_uid());
            std::fs::write(&squat, "x").expect("squat");
            let entry = materialize_into(base.path()).expect("falls back");
            assert_ne!(entry.parent().unwrap(), squat);
            assert_complete_and_private(entry.parent().unwrap());
        }

        /// One case per write bit, because the check is a mask and a mask can lose a bit
        /// silently: a test using only 0777 passes with the mask narrowed to either bit alone.
        #[test]
        fn a_group_or_world_writable_directory_is_refused() {
            for mode in [0o777, 0o770, 0o707, 0o720, 0o702] {
                let base = tempfile::tempdir().expect("tmp");
                let pre = extraction_dir(base.path(), effective_uid());
                std::fs::create_dir(&pre).expect("create");
                std::fs::set_permissions(&pre, std::fs::Permissions::from_mode(mode))
                    .expect("chmod");
                std::fs::write(pre.join("json.py"), "raise ImportError('planted')\n")
                    .expect("plant");

                let entry = materialize_into(base.path()).expect("falls back, not fails");
                let dir = entry.parent().unwrap();
                assert_ne!(dir, pre, "a directory of mode {mode:o} must be refused");
                assert!(
                    script_names_in(&pre).is_empty(),
                    "mode {mode:o}: nothing may be written into the refused directory"
                );
                assert_complete_and_private(dir);
            }
        }

        /// The owner predicate, driven through its seam. This process cannot create a directory
        /// owned by ANOTHER uid (that needs root), so the comparison is exercised by claiming a
        /// different expected uid against a directory we really own. What this does not show: a
        /// directory that really belongs to someone else, end to end through `materialize_into`.
        #[test]
        fn a_directory_owned_by_another_uid_is_refused() {
            let base = tempfile::tempdir().expect("tmp");
            let dir = base.path().join("mine");
            create_private_dir(&dir).expect("create");
            let me = effective_uid();
            assert_eq!(verify_private_dir(&dir, me), Ok(()));
            assert_eq!(
                verify_private_dir(&dir, me.wrapping_add(1)),
                Err(DirRefusal::ForeignOwner {
                    owner: me,
                    expected: me.wrapping_add(1)
                })
            );
        }

        #[test]
        fn the_verifier_names_what_it_refused() {
            let base = tempfile::tempdir().expect("tmp");
            let me = effective_uid();
            let real = base.path().join("real");
            create_private_dir(&real).expect("create");
            let link = base.path().join("link");
            symlink(&real, &link).expect("symlink");
            assert_eq!(
                verify_private_dir(&link, me),
                Err(DirRefusal::NotARealDirectory)
            );
            let file = base.path().join("file");
            std::fs::write(&file, "x").expect("file");
            assert_eq!(
                verify_private_dir(&file, me),
                Err(DirRefusal::NotARealDirectory)
            );
            std::fs::set_permissions(&real, std::fs::Permissions::from_mode(0o770)).expect("chmod");
            assert_eq!(
                verify_private_dir(&real, me),
                Err(DirRefusal::GroupOrOtherWritable { mode: 0o770 })
            );
        }

        /// The fallback is a directory this call created, under a name nobody could predict, so
        /// there is nothing for an attacker to pre-create.
        #[test]
        fn the_fallback_directory_is_fresh_each_time() {
            let base = tempfile::tempdir().expect("tmp");
            let squat = extraction_dir(base.path(), effective_uid());
            std::fs::write(&squat, "x").expect("squat");
            let a = materialize_into(base.path()).expect("a");
            let b = materialize_into(base.path()).expect("b");
            assert_ne!(
                a.parent(),
                b.parent(),
                "a fallback is never a reusable name"
            );
            for p in [&a, &b] {
                assert_eq!(mode_of(p.parent().unwrap()), 0o700);
            }
        }

        /// The fallback is verified like any other directory before it is used, and removed
        /// again when it fails. Reached through the uid seam, because a `mkdtemp` directory
        /// that this process just created at 0700 cannot fail the check any other way on a
        /// filesystem that honours modes: without the seam, deleting the re-verification
        /// SURVIVED the mutation set.
        #[test]
        fn a_fallback_that_fails_verification_is_refused_and_removed() {
            let base = tempfile::tempdir().expect("tmp");
            let me = effective_uid();
            assert!(fresh_private_dir(base.path(), me.wrapping_add(1)).is_none());
            let left: Vec<_> = std::fs::read_dir(base.path())
                .expect("readable")
                .filter_map(Result::ok)
                .collect();
            assert!(
                left.is_empty(),
                "a refused fallback must not be left behind"
            );
            // CONTROL: with the right uid the same call succeeds.
            let ok = fresh_private_dir(base.path(), me).expect("control");
            assert_eq!(verify_private_dir(&ok, me), Ok(()));
        }

        /// Two users of one shared `/tmp` must not contend for one name: a squatter would
        /// otherwise push every other user's engine into the leaking fallback.
        #[test]
        fn the_directory_name_is_per_user() {
            let base = Path::new("/base");
            let me = effective_uid();
            assert_ne!(
                extraction_dir(base, me),
                extraction_dir(base, me.wrapping_add(1))
            );
            assert!(
                extraction_dir(base, 4242)
                    .to_string_lossy()
                    .contains("-4242-"),
                "the uid is part of the name"
            );
        }

        /// Parity with the `create_dir_all` this replaced: a base that does not exist yet is
        /// created rather than failing the whole hint into silence.
        #[test]
        fn a_missing_base_directory_is_created() {
            let root = tempfile::tempdir().expect("tmp");
            let base = root.path().join("not").join("yet");
            let entry = materialize_into(&base).expect("a missing base must not fail the hint");
            assert_eq!(
                entry.parent().unwrap(),
                extraction_dir(&base, effective_uid())
            );
        }
    }

    /// Every local module the engine imports must be materialized beside it.
    ///
    /// The engine reaches its siblings with `sys.path[0]` set to its own directory, so a
    /// sibling that is not in that directory is an `ImportError` — and `attribute-red.py`
    /// answers an `ImportError` with exit 0 and no stdout, which `wip_author_diagnostic_with`
    /// reads as "nothing to say". `eed25579` (2026-10-11) added four such siblings and the
    /// two-file assertion above stayed green. This reads the imports out of what was
    /// ACTUALLY written, so a seventh module cannot be forgotten without this reddening.
    #[test]
    fn the_materialized_set_is_closed_under_its_own_local_imports() {
        let base = tempfile::tempdir().expect("tmp");
        let entry = materialize_into(base.path()).expect("materialization must succeed");
        let dir = entry.parent().expect("entry has a parent");
        let repo_scripts = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts");

        let mut missing = Vec::new();
        for file in std::fs::read_dir(dir)
            .expect("dir readable")
            .filter_map(Result::ok)
        {
            let name = file.file_name().to_string_lossy().into_owned();
            if !name.ends_with(".py") {
                continue;
            }
            let body = std::fs::read_to_string(file.path()).expect("script readable");
            for module in local_imports(&body, &repo_scripts) {
                if !dir.join(format!("{module}.py")).is_file() {
                    missing.push(format!("{name} imports {module}.py"));
                }
            }
        }
        missing.sort();
        missing.dedup();
        assert!(
            missing.is_empty(),
            "the materialized directory lacks modules the engine imports: {missing:?}"
        );
    }

    /// The materialized engine must be able to import itself in a directory that holds
    /// nothing else — the shape production runs in, and the shape no other test runs in.
    ///
    /// `tests/attribute-red.sh` runs the engine from `scripts/`, where every sibling exists;
    /// the two tests that DO go through `materialize` used to skip when the hint was absent.
    /// This one runs the entry point directly, with no `PYTHONPATH`, and reads the engine's
    /// own `--explain` account of why it stayed silent: the import-failure sentence is the
    /// one outcome that must never appear.
    #[test]
    fn the_materialized_engine_imports_in_an_empty_directory() {
        if !python3_present() {
            eprintln!("skipping: python3 absent");
            return;
        }
        let Some(repo) = dirty_fixture_repo() else {
            eprintln!("skipping: git unavailable");
            return;
        };
        let base = tempfile::tempdir().expect("tmp");
        let entry = materialize_into(base.path()).expect("materialization must succeed");
        let out = std::process::Command::new("python3")
            .arg("-B")
            .arg(&entry)
            .arg("--explain")
            .current_dir(repo.path())
            .env_remove("PYTHONPATH")
            .stdin(std::process::Stdio::null())
            .output()
            .expect("python3 spawns");
        let stderr = String::from_utf8_lossy(&out.stderr);
        // The failure notice is on STDOUT (the one stream `run_command` keeps), so a
        // stderr-only read of it could never fail. Read both.
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(
            !stdout.contains("could not be imported") && !stderr.contains("could not be imported"),
            "the engine cannot import its own modules from the materialized directory; \
               stdout: {stdout} stderr: {stderr}"
        );
    }

    /// Stage 0 is the whole economics: without it every green command in the session pays
    /// a process spawn. Asserted by TIME rather than by reading the branch, because the
    /// claim is about cost and a branch can be present and still slow.
    ///
    /// The fixture must contain NOTHING failure-shaped. The first version used
    /// `"error: --> src/lib.rs:1:1"`, which was correct while the gate was `exit_code == 0`
    /// alone and became a FALSE PASS the moment the gate learned to read the text — it
    /// would have asserted that a spawn is skipped using the exact input that must now
    /// cause one. Ordinary cargo chatter is the right fixture: it is what a green command
    /// actually prints. The `names_a_diagnostic` assertion below pins that property of the
    /// fixture, so a later edit to the string cannot quietly re-break it.
    #[tokio::test]
    async fn a_zero_exit_with_nothing_failure_shaped_spawns_nothing() {
        let t = std::time::Instant::now();
        let quiet = "   Compiling codescout v0.15.0\n    Finished `dev` profile in 3.21s\n";
        assert!(
            !names_a_diagnostic(quiet),
            "fixture must be non-diagnostic or this test proves nothing"
        );
        let got = wip_author_diagnostic(0, quiet, Path::new(".")).await;
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
        // The knob as a VALUE, not as process state. This test used to `set_var` the real
        // env var and hold it across the await below, which made every concurrently
        // scheduled sibling read it — see `AttributionEnv`'s docstring for what that cost.
        let env = AttributionEnv {
            enabled: false,
            ..Default::default()
        };
        let got = wip_author_diagnostic_with(101, red, dir.path(), &env).await;
        assert!(
            got.is_none(),
            "the opt-out must suppress a hint the same input produces without it; got: {got:?}"
        );
    }

    /// The timeout must END the wait, not become an answer.
    ///
    /// `_ => return None` is the one place where "we could not find out" is closest to being
    /// rendered as "we found out", which is the failure the whole engine refuses — so it has
    /// to be reachable, or a mutation turning it into a verdict survives silently.
    ///
    /// It is reachable because the ceiling is a **parameter**. It used to be reachable because
    /// the ceiling was an env knob a test could `set_var`, which bought the same coverage and
    /// charged every concurrent sibling for it; the knob is still a real env var on the
    /// shipped path, resolved once by `AttributionEnv::from_env`.
    #[tokio::test]
    async fn a_timed_out_scan_is_silence_and_never_a_verdict() {
        let Some(dir) = dirty_fixture_repo() else {
            eprintln!("skipping: git unavailable");
            return;
        };
        let red = "error[E0425]: cannot find value `broken`\n  --> src/held.rs:1:13\n";
        let env = AttributionEnv {
            // 1 ms: no process finishes in that.
            timeout: Duration::from_millis(1),
            ..Default::default()
        };
        let got = wip_author_diagnostic_with(101, red, dir.path(), &env).await;
        assert!(
            got.is_none(),
            "a scan that ran out of time must say nothing at all; got: {got:?}"
        );
    }
    /// A helper that CRASHED is a tooling failure, and it must not read as "nothing to say".
    ///
    /// The engine exits 0 for every outcome it chooses (nothing dirty, nothing named, a notice
    /// of its own). A non-zero exit is therefore never a verdict about the red: it is an
    /// uncaught exception, and its traceback used to go to `Stdio::null()`.
    /// docs/issues/archive/2026-10-14-a-failure-of-the-red-attribution-helper-is-discarded-twice-and-reads-as-nothing-to-say.md
    #[test]
    fn a_crashed_helper_is_a_tooling_notice_and_never_silence() {
        let stderr =
            b"Traceback (most recent call last):\n  File \"x.py\", line 1\nKeyError: 'owners'\n";
        let got = engine_outcome(false, Some(1), b"", stderr).expect("a crash must say so");
        assert!(got.contains("tooling failure"), "got: {got}");
        assert!(got.contains("NOT evidence"), "got: {got}");
        assert!(
            got.contains("KeyError: 'owners'"),
            "must name the cause, got: {got}"
        );
        assert!(
            !got.contains("Traceback"),
            "only the last stderr line is shown, got: {got}"
        );
    }

    /// A crash that left no code (a signal) is still a crash.
    #[test]
    fn a_helper_killed_by_a_signal_is_a_tooling_notice() {
        let got = engine_outcome(false, None, b"", b"").expect("a signal death must say so");
        assert!(got.contains("signal"), "got: {got}");
    }

    /// The outcomes the engine chooses stay exactly as they were: empty is silence, text is text.
    #[test]
    fn a_clean_exit_keeps_its_stdout_and_its_silence() {
        assert_eq!(engine_outcome(true, Some(0), b"", b"noise on stderr"), None);
        assert_eq!(engine_outcome(true, Some(0), b"  \n", b""), None);
        assert_eq!(
            engine_outcome(true, Some(0), b"line one\nline two\n\n", b"ignored").as_deref(),
            Some("line one\nline two")
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
            // Absent python3 is the only legitimate silence here (ceiling 3). With python3
            // present, no diagnostic means the engine is broken — the case 2026-10-13 hid for
            // two days behind a skip that only asked "python3 absent?".
            assert!(
                !python3_present(),
                "python3 is present but the engine produced no hint for a red naming a dirty file"
            );
            eprintln!("skipping: python3 absent");
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

    /// The twin of the case above, on the same fixture — and it USED TO ASSERT THE
    /// DEFECT. Its previous form required the identical red at exit 0 to produce nothing,
    /// which is exactly the silence
    /// `docs/issues/archive/2026-09-12-the-red-attribution-hook-fires-only-on-a-non-zero-exit-the-mandated-gate-never-produces.md`
    /// was filed about: `<red> ; echo "LEAN exit=$?"` exits 0 with the real failure sitting
    /// in stdout, and CLAUDE.md MANDATES that shape for the gate.
    ///
    /// The discrimination the old form provided is preserved and improved rather than
    /// dropped. It existed because "without it, `exit_code == 0` could be deleted and only
    /// a TIMING assertion would notice". The condition is now pinned from BOTH sides by
    /// behaviour: delete the gate entirely and
    /// `a_zero_exit_with_nothing_failure_shaped_spawns_nothing` reds on cost; restore it as
    /// an unconditional `exit_code == 0 -> None` and THIS one reds on coverage. Neither
    /// direction rests on a timing assertion any more — the kind that gets relaxed on a
    /// loaded machine.
    #[tokio::test]
    async fn the_same_red_at_exit_zero_still_answers() {
        let Some(dir) = dirty_fixture_repo() else {
            eprintln!("skipping: git unavailable");
            return;
        };
        let red = "error[E0425]: cannot find value `broken`\n  --> src/held.rs:1:13\n";
        // Establish the engine can answer AT ALL here, at the exit code that always
        // worked. Without it an absent python3 yields the same `None` as a deleted gate,
        // and this test would pass while proving nothing.
        if wip_author_diagnostic(101, red, dir.path()).await.is_none() {
            assert!(
                !python3_present(),
                "python3 is present but the engine produced no hint at exit 101, so this test \
             cannot say anything about exit 0"
            );
            eprintln!("skipping: python3 absent");
            return;
        }
        let got = wip_author_diagnostic(0, red, dir.path())
            .await
            .expect("the engine answers here at 101, so exit 0 with the same red must too");
        assert!(
            got.contains("src/held.rs"),
            "the hint must name the dirty file the red is about; got: {got}"
        );
    }
    /// One fixture per `DIAGNOSTIC_PATH` pattern in `scripts/attribute-red.py`, asserting
    /// the cheap Rust pre-filter admits every shape the Python engine can match.
    ///
    /// This is the half of the coupling that can be checked directly. The pre-filter is
    /// allowed to be WIDER than the engine and must never be narrower, so each of these
    /// must pass; none of them says anything about how much wider it is, and that is fine
    /// — a false positive costs one spawn, a false negative is the silent wrong answer the
    /// whole gate exists to prevent.
    #[test]
    fn the_prefilter_admits_every_shape_the_engine_matches() {
        // (engine pattern it stands for, text)
        let per_pattern = [
            ("rustc span", "  --> src/a.rs:1:1\n"),
            (
                "panic site, quoted",
                "thread 'x' panicked at 'src/a.rs:3', oh no\n",
            ),
            ("panic site, bare", "panicked at src/a.rs:3:9\n"),
            (
                "error: line naming a .rs",
                "error[E0425]: cannot find value in src/a.rs\n",
            ),
            ("Error: line", "  Error: scripts/x.sh\n"),
        ];
        for (which, text) in per_pattern {
            assert!(
                names_a_diagnostic(text),
                "pre-filter rejected the {which} shape, so the engine would never be \
                 spawned for it: {text:?}"
            );
        }
        assert_eq!(
            per_pattern.len(),
            engine_pattern_count(),
            "the fixture set above must hold exactly one entry per DIAGNOSTIC_PATH \
             pattern in scripts/attribute-red.py"
        );
    }

    /// Count `re.compile(` entries inside `attribute-red.py`'s `DIAGNOSTIC_PATH` list.
    ///
    /// Derived at test time from the file that ships, never stored here — a stored number
    /// is the thing this repo refuses, and it would go stale in exactly the direction that
    /// matters (a sixth pattern added, the pre-filter silently too narrow for it, no
    /// signal). Returns 0 rather than panicking if the shape is unrecognisable, so the
    /// assertion above reports a mismatch instead of this helper exploding.
    fn engine_pattern_count() -> usize {
        let src = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/scripts/attribute-red.py"
        ))
        .unwrap_or_default();
        let Some(start) = src.find("DIAGNOSTIC_PATH = [") else {
            return 0;
        };
        let rest = &src[start..];
        let Some(end) = rest.find("\n]") else {
            return 0;
        };
        rest[..end].matches("re.compile(").count()
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
