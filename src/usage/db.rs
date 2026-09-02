use anyhow::Result;
use rusqlite::{params, Connection};
use std::path::Path;

pub fn open_db(project_root: &Path) -> Result<Connection> {
    let path = project_root.join(".codescout").join("usage.db");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(&path)?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;

        CREATE TABLE IF NOT EXISTS tool_calls (
            id         INTEGER PRIMARY KEY AUTOINCREMENT,
            tool_name  TEXT NOT NULL,
            called_at  TEXT NOT NULL DEFAULT (datetime('now')),
            latency_ms INTEGER NOT NULL,
            outcome    TEXT NOT NULL,
            overflowed INTEGER NOT NULL DEFAULT 0,
            error_msg  TEXT
        );

        CREATE TABLE IF NOT EXISTS lsp_events (
            id                INTEGER PRIMARY KEY AUTOINCREMENT,
            language          TEXT NOT NULL,
            started_at        TEXT NOT NULL DEFAULT (datetime('now')),
            reason            TEXT NOT NULL,
            handshake_ms      INTEGER NOT NULL,
            first_response_ms INTEGER
        );

        CREATE TABLE IF NOT EXISTS call_edges (
            project_id   TEXT NOT NULL,
            caller_sym   TEXT NOT NULL,
            callee_sym   TEXT NOT NULL,
            file         TEXT NOT NULL,
            line         INTEGER NOT NULL,
            col          INTEGER NOT NULL,
            source       TEXT NOT NULL,
            computed_at  INTEGER NOT NULL,
            PRIMARY KEY (project_id, caller_sym, callee_sym, file, line, col)
        );
        CREATE INDEX IF NOT EXISTS call_edges_caller ON call_edges(project_id, caller_sym);
        CREATE INDEX IF NOT EXISTS call_edges_callee ON call_edges(project_id, callee_sym);
        CREATE INDEX IF NOT EXISTS call_edges_file   ON call_edges(project_id, file);",
    )?;

    // Migration: add traceability columns (v0.9)
    let has_session_id: bool = conn
        .prepare("SELECT session_id FROM tool_calls LIMIT 0")
        .is_ok();
    if !has_session_id {
        conn.execute_batch(
            "ALTER TABLE tool_calls ADD COLUMN codescout_sha TEXT;
             ALTER TABLE tool_calls ADD COLUMN project_sha TEXT;
             ALTER TABLE tool_calls ADD COLUMN session_id TEXT;
             ALTER TABLE tool_calls ADD COLUMN input_json TEXT;
             ALTER TABLE tool_calls ADD COLUMN output_json TEXT;",
        )?;
    }

    // Migration: add CC session link (v0.10)
    let has_cc_session_id: bool = conn
        .prepare("SELECT cc_session_id FROM tool_calls LIMIT 0")
        .is_ok();
    if !has_cc_session_id {
        conn.execute_batch("ALTER TABLE tool_calls ADD COLUMN cc_session_id TEXT;")?;
    }

    // Migration: record failed LSP starts, not just completed handshakes.
    // Without this, a server that dies during `initialize` (e.g. an expired LSP
    // build) leaves zero lsp_events rows — a chronically-failing LSP is invisible
    // to usage analytics. `outcome` defaults to 'success' so the unchanged
    // `write_lsp_event` INSERT and every pre-existing row stay correct.
    let has_lsp_outcome: bool = conn
        .prepare("SELECT outcome FROM lsp_events LIMIT 0")
        .is_ok();
    if !has_lsp_outcome {
        conn.execute_batch(
            "ALTER TABLE lsp_events ADD COLUMN outcome TEXT NOT NULL DEFAULT 'success';
             ALTER TABLE lsp_events ADD COLUMN error TEXT;",
        )?;
    }

    // Migration: legibility friction fields (v0.11). Additive + nullable so every
    // pre-existing row and the unchanged INSERTs stay correct.
    let has_friction_target: bool = conn
        .prepare("SELECT friction_target FROM tool_calls LIMIT 0")
        .is_ok();
    if !has_friction_target {
        conn.execute_batch(
            "ALTER TABLE tool_calls ADD COLUMN friction_target TEXT;
                 ALTER TABLE tool_calls ADD COLUMN overflow_tokens INTEGER;
                 ALTER TABLE tool_calls ADD COLUMN err_family TEXT;
                 ALTER TABLE tool_calls ADD COLUMN project_root TEXT;",
        )?;
    }

    // Migration: the build's dirty bit (BL-24). `codescout_sha` alone is not an
    // identity — a dirty build of commit X contains arbitrary uncommitted work while
    // claiming to be X, and that misread a live fix as absent during a real
    // acceptance measurement. `build.rs` already computed this flag and exported it;
    // it reached exactly one consumer (`codescout version`) and stopped one call short
    // of the table that needed it. Additive + nullable, so every pre-existing row and
    // the unchanged SELECTs stay correct; NULL reads as "recorded before the column
    // existed", which is honestly different from "recorded clean".
    let has_dirty: bool = conn
        .prepare("SELECT codescout_dirty FROM tool_calls LIMIT 0")
        .is_ok();
    if !has_dirty {
        conn.execute_batch("ALTER TABLE tool_calls ADD COLUMN codescout_dirty INTEGER;")?;
    }

    backfill_legacy_rows(&conn, &project_root.to_string_lossy())?;

    Ok(conn)
}

/// The build's identity: which commit, and whether the tree was clean.
///
/// A sha on its own is **not** an identity. A dirty build of commit X contains
/// arbitrary uncommitted work while claiming to be X, and `codescout_sha` is the column
/// an acceptance measurement is supposed to rank on — so the missing flag turned a
/// live fix into "the fix is not in this build" and cost a behavioural re-check to
/// disprove. BL-24.
///
/// The two travel as one value so a caller cannot record the sha and drop the flag,
/// which is exactly what happened: `build.rs` computed all three values, and
/// `CODESCOUT_GIT_DIRTY` reached one consumer (`codescout version`) while the recorder
/// passed only `env!("CODESCOUT_GIT_SHA")`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildProvenance<'a> {
    pub sha: &'a str,
    pub dirty: bool,
}

impl BuildProvenance<'static> {
    /// This binary's own provenance, baked by `build.rs`.
    ///
    /// Note what this does **not** promise: `build.rs` declares `rerun-if-changed` on
    /// `.git/HEAD`, `.git/index` and `.git/refs/heads/` only, so editing a source file
    /// without staging it and rebuilding recompiles the crate *without* re-running
    /// `build.rs` — both values keep their previous contents. That narrowing is a
    /// deliberate trade (three `git` invocations per build otherwise); the honest label
    /// is the cheaper cure, and `dirty` is what makes the staleness visible.
    pub fn current() -> Self {
        Self {
            sha: env!("CODESCOUT_GIT_SHA"),
            dirty: env!("CODESCOUT_GIT_DIRTY") == "1",
        }
    }
}

impl<'a> From<&'a str> for BuildProvenance<'a> {
    /// Fixture convenience: a bare sha, assumed clean.
    ///
    /// Deliberately **not** how production records. `UsageRecorder` passes
    /// [`BuildProvenance::current`], and `the_recorder_never_assumes_a_clean_build` pins
    /// that — because this impl re-opens, for fixtures, the exact affordance that caused
    /// BL-24: a sha travelling without a measured flag.
    fn from(sha: &'a str) -> Self {
        Self { sha, dirty: false }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn write_record<'a, B: Into<BuildProvenance<'a>>>(
    conn: &Connection,
    tool_name: &str,
    latency_ms: i64,
    outcome: &str,
    overflowed: bool,
    error_msg: Option<&str>,
    build: B,
    project_sha: Option<&str>,
    session_id: &str,
    input_json: Option<&str>,
    output_json: Option<&str>,
    cc_session_id: Option<&str>,
    friction_target: Option<&str>,
    overflow_tokens: Option<i64>,
    err_family: Option<&str>,
    project_root: Option<&str>,
) -> Result<()> {
    // Taken by value so the sha and its dirty bit cannot be separated at the call site.
    // They were separable before BL-24, and the flag was the half that got dropped.
    let build = build.into();
    conn.execute(
        "INSERT INTO tool_calls (tool_name, called_at, latency_ms, outcome, overflowed, error_msg, codescout_sha, codescout_dirty, project_sha, session_id, input_json, output_json, cc_session_id, friction_target, overflow_tokens, err_family, project_root)
         VALUES (?1, datetime('now'), ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
        params![
            tool_name,
            latency_ms,
            outcome,
            overflowed as i64,
            error_msg,
            build.sha,
            build.dirty as i64,
            project_sha,
            session_id,
            input_json,
            output_json,
            cc_session_id,
            friction_target,
            overflow_tokens,
            err_family,
            project_root,
        ],
    )?;
    // `pika_observations` is not codescout's table (a buddy-plugin skill creates it,
    // zero references in this crate), but usage.db is opened without
    // `PRAGMA foreign_keys`, so its `ON DELETE CASCADE` never fires and a pruned
    // parent leaves an orphaned observation behind instead. Exempt a referenced row
    // from the sweep when the table exists; skip the exemption entirely on a
    // project that has never seen the plugin run, rather than reference a table
    // that isn't there.
    // docs/issues/archive/2026-08-20-pika-observations-orphaned-by-the-retention-sweep.md
    let has_pika_observations = conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name='pika_observations'",
            [],
            |_| Ok(true),
        )
        .unwrap_or(false);
    if has_pika_observations {
        conn.execute(
            "DELETE FROM tool_calls WHERE called_at < datetime('now', '-30 days') \
             AND id NOT IN (SELECT tool_call_id FROM pika_observations)",
            [],
        )?;
    } else {
        conn.execute(
            "DELETE FROM tool_calls WHERE called_at < datetime('now', '-30 days')",
            [],
        )?;
    }
    Ok(())
}

/// Map an error message to a stable, low-cardinality family tag for the probe.
/// Order matters: more specific patterns first. `None` for unrecognized messages.
///
/// Lives here (not in the parent module) so the one-time backfill in `open_db`
/// can re-classify historical `error_msg` values with the same logic
/// `write_content` applies to new rows.
pub(crate) fn normalize_err_family(tool_name: &str, msg: &str) -> Option<&'static str> {
    // infra / tool-class (excluded from the probe's code-class score)
    if msg.contains("index is locked") {
        return Some("lsp_index_locked");
    }
    if msg.contains("Failed to spawn mux") || msg.contains("mux startup failed") {
        return Some("mux_startup_fail");
    }
    if msg.contains("LSP server is not running") {
        return Some("lsp_not_running");
    }
    if msg.contains("LSP server disconnected") {
        return Some("lsp_disconnect");
    }
    // read_markdown's OWN errors — tool-scoped so an unrelated tool emitting
    // similar-looking text never mis-attributes. Distinct from
    // `il4_read_markdown_routing` below, which is read_file's redirect-TO-
    // read_markdown message, not read_markdown's own failure.
    if tool_name == "read_markdown" {
        if msg.contains("only supports .md files") {
            return Some("read_markdown_wrong_ext");
        }
        if msg.starts_with("file not found:") {
            return Some("read_markdown_file_not_found");
        }
        if msg.contains("is a directory, not a file") {
            return Some("read_markdown_path_is_directory");
        }
        if msg.contains("exceeds inline threshold") {
            return Some("read_markdown_overflow_threshold");
        }
        if msg.contains("mutually exclusive")
            || msg.contains("both start_line and end_line are required")
        {
            return Some("read_markdown_param_conflict");
        }
        if msg.contains("invalid line range") || msg.contains("exceeds file length") {
            return Some("read_markdown_invalid_line_range");
        }
    }
    // Shared heading-resolution error (file_summary::resolve_section_range) —
    // raised by read_markdown and edit_markdown; NOT by doc(get), which
    // swallows the same miss into body_meta.heading_missing and stays success.
    // Quote-style agnostic: read_markdown's runtime message quotes the heading
    // via `{:?}` (Rust Debug — double quotes), while edit_markdown's propagated
    // message uses `{}` (Display — single quotes). Match on the shape shared
    // by both rather than a specific quote character.
    if (tool_name == "read_markdown" || tool_name == "edit_markdown")
        && msg.starts_with("heading ")
        && msg.ends_with(" not found")
    {
        return Some("heading_not_found");
    }
    // iron-law routing / wrong-tool class — the agent reached for the wrong tool
    // and the server gate rejected + re-routed it. These dominate the real error
    // population; the original taxonomy missed them all, leaving err_family NULL
    // on ~90% of errors even on fresh rows.
    if msg.contains("overlaps named symbol") {
        return Some("il1_read_overlaps_symbol");
    }
    if msg.contains("Use read_markdown") {
        return Some("il4_read_markdown_routing");
    }
    if msg.contains("Use edit_markdown") {
        return Some("il5_edit_markdown_routing");
    }
    if msg.contains("contains a symbol definition")
        || msg.contains("is blocked for structural edits")
    {
        return Some("il2_structural_edit");
    }
    if msg.contains("shell access to source files is blocked") {
        return Some("il3_shell_on_source");
    }
    if msg.contains("IL3 violation") {
        return Some("il3_pipe_to_trimmer");
    }
    // security / scope class — deliberately tool-agnostic: `write denied` comes
    // from `path_security.rs`'s shared write-gate, reused by every write tool.
    // Same underlying mechanism regardless of caller, so one family is correct.
    if msg.contains("write denied") {
        return Some("write_scope_denied");
    }
    // input-shape / extractor class
    if msg.contains("unsupported json_path") {
        return Some("json_path_unsupported");
    }
    // deliberately tool-agnostic: `edit_file` and `edit_markdown` both raise
    // "old_string not found" for the identical root cause (stale re-read before
    // editing) — confirmed by grep across src/tools/edit_file/mod.rs and
    // src/tools/markdown/edit_markdown.rs; one family is correct here too.
    if msg.contains("old_string not found") {
        return Some("edit_stale_match");
    }
    // code / extractor-shape class
    if msg.contains("AST parse failed") || msg.contains("cannot determine end of") {
        return Some("ast_extent_fail");
    }
    if msg.contains("ambiguous name_path") {
        return Some("ambiguous_name_path");
    }
    if msg.contains("dropped sibling") || msg.contains("dropped the symbol") {
        return Some("replace_dropped_sibling");
    }
    if msg.contains("symbol not found") {
        return Some("symbol_not_found");
    }
    // ---- 2026-08-15 extension (TU-5): the unclassified head ----
    // Ordered most-specific first, like the block above. Sizes are hits in the
    // live-DB unclassified population (197 errors, six actively-written DBs);
    // the lifetime corpus is not used for ranking because 77% of its
    // unclassified mass sits in dead DBs frozen at `user_version=0`.

    // json_path key miss (27). Deliberately NOT folded into
    // `json_path_unsupported`: there the syntax is rejected, here the syntax is
    // valid and the key is absent from THIS buffer's shape. The fixes diverge
    // (rewrite the expression vs. inspect the shape first), so one family would
    // make the ranking undecidable.
    if msg.contains("path segment ") && msg.contains(" not found") {
        return Some("json_path_key_miss");
    }
    // Librarian routing guard (25) — same class as the IL routing families:
    // the gate rejected the call and re-routed it to `artifact`.
    if msg.contains("is a librarian-managed artifact") {
        return Some("librarian_managed_artifact");
    }
    // Healthy guards, tagged so they are visible in the ranking rather than
    // invisible in the NULL bucket. cf. TU-7: a high-volume guard doing its job
    // must not read as an error family worth "fixing".
    if msg.contains("would wipe") {
        return Some("destructive_replace_blocked");
    }
    if msg.contains("would introduce syntax errors")
        || msg.contains("left the file syntactically invalid")
    {
        return Some("edit_would_break_syntax");
    }
    // Heading resolution reached from artifact's body_edits batch. The arm above
    // is anchored with `starts_with("heading ")`, which the batch form
    // (`body_edits[0]: heading '## Fix' not found`) never satisfies. Ambiguity is
    // split from absence — add-context vs. fix-the-name are different repairs.
    if msg.contains("heading ") && msg.contains(" not found") {
        return Some("heading_not_found");
    }
    if msg.contains("heading ") && msg.contains(" times") {
        return Some("ambiguous_heading");
    }
    // Target already exists (17).
    if msg.starts_with("file already exists:") {
        return Some("target_already_exists");
    }
    // Target missing (13) — three shapes across four tools. read_markdown's own
    // `file not found:` is claimed by its tool-scoped arm well above this.
    if msg.starts_with("path not found:")
        || msg.starts_with("file not found:")
        || msg.contains("no file to edit at")
        || msg.contains("No such file or directory")
    {
        return Some("path_not_found");
    }
    // Unknown enum / field value (8) — the agent supplied a well-formed call
    // with a value outside the accepted set.
    if msg.contains("unknown action ")
        || msg.contains("unknown field ")
        || msg.contains("unknown repo ")
        || msg.contains("unknown id ")
        || msg.contains("unknown topic ")
        || msg.contains("invalid at=")
        || msg.contains("is not a bug status")
    {
        return Some("unknown_enum_value");
    }
    // Expired buffer handle (6) — the overflow-recovery failure mode: an @ref
    // held past the session that owned it.
    if msg.contains("buffer reference not found") || msg.contains("background job ref not found") {
        return Some("buffer_ref_expired");
    }
    // Line range past EOF / inverted (4). Tool-agnostic, and reached only after
    // the read_markdown block above, which keeps its own tool-scoped family:
    // unifying the two would require re-mapping already-classified rows, and the
    // backfill only fills NULLs — there is no re-map path. Deferred, not intended.
    if msg.contains("invalid line range") || msg.contains("is past end of file") {
        return Some("invalid_line_range");
    }
    // Invalid regex (5).
    if msg.contains("invalid regex") || msg.contains("regex parse error") {
        return Some("invalid_regex");
    }
    // old_string ambiguous, not absent (4). `edit_stale_match` above means
    // re-read the file; this means add disambiguating context. Opposite repairs,
    // so they must not share a family.
    if msg.contains("old_string found ") || msg.contains("old_string matches ") {
        return Some("ambiguous_old_string");
    }
    // edit_markdown's own wrong-extension error (1). read_markdown has carried an
    // arm for this since the taxonomy was written; its twin never did, so every
    // edit_markdown-on-a-non-md-file landed unclassified. Tool-scoped for the
    // same reason the read_markdown block is.
    if tool_name == "edit_markdown" && msg.contains("only supports .md files") {
        return Some("edit_markdown_wrong_ext");
    }
    // ---- 2026-08-20 extension: the librarian surface and one write gate ----
    // Measured on codescout's own DB: 73 of 1,139 errors unclassified, and the
    // population is two concentrations rather than a tail — 49% the artifact/librarian
    // API surface, 31% a single worktree write gate. Coverage by tool says the same:
    // run_command 0.2% unclassified and read_markdown/grep/references 0%, against
    // artifact 37.5% and memory/symbols ~50%. The taxonomy maps where someone did the
    // work, not which errors are hard.
    //
    // NOT a friction ranking: the unclassified bucket's immediate-repeat rate is 2.8%
    // against a ~4-5% corpus average, so these are among the healthiest errors here.
    // Classified because an unnamed family cannot be counted, trended, or given a
    // `refusal_predicate` — see `capability-proposals:CAP-9` for why the opposite claim
    // (NULL as a 1.97x friction bucket) did not reproduce.

    // The largest single member (23) — one gate, four write tools.
    if msg.contains("git worktrees detected") {
        return Some("worktree_activate_required");
    }
    // One repair — declare `entry_collection` — reached from three call sites (9).
    // Splitting by entry point would scatter a single fix across three families.
    if msg.contains("entry_filter set but") || msg.contains("without an `entry_collection`") {
        return Some("entry_collection_missing");
    }
    // Frontmatter field passed through `extra` that the schema already models (6).
    if msg.contains("must not contain frontmatter field") {
        return Some("extra_models_reserved_field");
    }
    // Schema rejection on an entry write (5) — well-formed call, out-of-range value.
    // Anchored on the verb so a hint merely naming `params_schema` cannot match.
    if msg.contains("violates params_schema") || msg.contains("violate params_schema") {
        return Some("params_schema_violation");
    }
    // Ledger identity missing (4). MUST precede `artifact_not_augmented`: one of these
    // messages also says "has no augmentation", and the repair here is to declare the
    // ledger, not merely to augment. Pinned by an ordering guard in
    // `normalize_err_family_maps_the_2026_08_20_unclassified_head`.
    if msg.contains("allocate_entry_id:") {
        return Some("ledger_not_declared");
    }
    if msg.contains("no augmentation for artifact") {
        return Some("artifact_not_augmented");
    }
    // Tool-scoped (4): `memory` named a topic/section/project that does not exist and
    // listed the valid ones. Scoped so no other tool's "not found" lands here.
    if tool_name == "memory"
        && (msg.contains("no sections matched")
            || msg.contains(" not found")
            || msg.starts_with("No project "))
    {
        return Some("memory_target_not_found");
    }
    // Two parameters that cannot both be set (3). Distinct from read_markdown's
    // tool-scoped `read_markdown_param_conflict`, which is claimed far above.
    if msg.contains("at most one of") {
        return Some("mutually_exclusive_params");
    }
    // append_entry's parameter sent to update_entry, or an empty patch (3).
    if msg.contains("is append_entry's parameter") || msg.contains("`fields` is empty") {
        return Some("entry_patch_param_misuse");
    }
    // Two json_path failures that are neither `json_path_unsupported` (syntax rejected)
    // nor `json_path_key_miss` (key absent from the shape). Wrong buffer KIND and wrong
    // value SHAPE each have their own repair, so they get their own families.
    if msg.contains("json_path is only supported on") {
        return Some("json_path_wrong_buffer_kind");
    }
    if msg.contains("needs an array, found") || msg.contains("out of bounds for array") {
        return Some("json_path_shape_mismatch");
    }
    // Retryable-as-is, which none of the other LSP families are: the server is coming
    // up, so the same call succeeds shortly. lsp_not_running/lsp_disconnect both mean
    // the call must change or the server must be repaired.
    if msg.contains("is still starting") {
        return Some("lsp_still_starting");
    }
    // A security-class gate (1), tagged so it is visible in the ranking rather than
    // invisible in the NULL bucket — TU-7's lesson applied to a guard doing its job.
    if msg.contains("escapes project root") {
        return Some("cwd_escapes_root");
    }

    // Missing / conditionally-required params (38, the largest family). Kept LAST
    // because its shapes are the broadest: any earlier arm that also matches is
    // by definition the more specific reading.
    if (msg.starts_with("missing ") && (msg.contains(" parameter") || msg.contains("field ")))
        || msg.contains("requires '")
        || msg.contains("is required for")
        || msg.ends_with(" required")
    {
        return Some("missing_required_param");
    }
    None
}

/// Every family [`normalize_err_family`] can emit, sorted and deduplicated.
///
/// The classifier is an if-chain returning string literals, so before this const
/// existed the family set was not enumerable by anything — which is exactly why
/// the backfill gate had to be a hand-maintained integer. Pinned to the classifier
/// in both directions by `err_families_lists_exactly_what_the_classifier_can_emit`,
/// which reads this file's own source: a new arm cannot ship without appearing
/// here, and appearing here changes the fingerprint the gate derives. BL-4.
const ERR_FAMILIES: &[&str] = &[
    "ambiguous_heading",
    "ambiguous_name_path",
    "ambiguous_old_string",
    "artifact_not_augmented",
    "ast_extent_fail",
    "buffer_ref_expired",
    "cwd_escapes_root",
    "destructive_replace_blocked",
    "edit_markdown_wrong_ext",
    "edit_stale_match",
    "edit_would_break_syntax",
    "entry_collection_missing",
    "entry_patch_param_misuse",
    "extra_models_reserved_field",
    "heading_not_found",
    "il1_read_overlaps_symbol",
    "il2_structural_edit",
    "il3_pipe_to_trimmer",
    "il3_shell_on_source",
    "il4_read_markdown_routing",
    "il5_edit_markdown_routing",
    "invalid_line_range",
    "invalid_regex",
    "json_path_key_miss",
    "json_path_shape_mismatch",
    "json_path_unsupported",
    "json_path_wrong_buffer_kind",
    "ledger_not_declared",
    "librarian_managed_artifact",
    "lsp_disconnect",
    "lsp_index_locked",
    "lsp_not_running",
    "lsp_still_starting",
    "memory_target_not_found",
    "missing_required_param",
    "mutually_exclusive_params",
    "mux_startup_fail",
    "params_schema_violation",
    "path_not_found",
    "read_markdown_file_not_found",
    "read_markdown_invalid_line_range",
    "read_markdown_overflow_threshold",
    "read_markdown_param_conflict",
    "read_markdown_path_is_directory",
    "read_markdown_wrong_ext",
    "replace_dropped_sibling",
    "symbol_not_found",
    "target_already_exists",
    "unknown_enum_value",
    "worktree_activate_required",
    "write_scope_denied",
];

/// FNV-1a over a family list, mapped into `PRAGMA user_version`'s range.
///
/// Written out rather than delegated to `DefaultHasher`, whose output std does
/// **not** guarantee across Rust releases. This value is *persisted*: a silent
/// implementation change would re-run the backfill on every DB on every open,
/// forever, and nothing would say why.
///
/// The result is forced odd and positive, so it is never `0` — `user_version`
/// defaults to `0`, which must always read as "never backfilled".
fn fingerprint_families(families: &[&str]) -> i64 {
    let mut hash: u32 = 0x811c_9dc5;
    for family in families {
        for byte in family.as_bytes() {
            hash ^= u32::from(*byte);
            hash = hash.wrapping_mul(0x0100_0193);
        }
        // Separator: without it, ["ab", "c"] and ["a", "bc"] fingerprint alike.
        hash ^= u32::from(b'\n');
        hash = hash.wrapping_mul(0x0100_0193);
    }
    i64::from((hash >> 1) | 1)
}

/// The taxonomy marker stored in `PRAGMA user_version`, **derived** from
/// [`ERR_FAMILIES`] rather than maintained by hand.
///
/// This replaces a `const BACKFILL_VERSION: i64` that a human had to remember to
/// bump whenever [`normalize_err_family`] gained an arm. Nothing computed it from
/// the classifier, so the invariant "the taxonomy changed ⇒ re-classify" was held
/// by convention — and forgetting it silently froze every already-backfilled DB's
/// history: no error, no warning, no failing test. Measured 2026-08-15: four
/// distinct `user_version` values live at once across twelve DBs, with rows
/// classified under four taxonomies pooled into one queryable corpus. BL-4.
///
/// The gate compares for **equality**, not `>=`, because a fingerprint is not
/// ordered. A DB carrying any other value — including one of the old sequential
/// versions — re-runs the backfill once and is then stamped. That re-run is
/// idempotent (only `NULL` families are filled), so the migration costs one pass.
fn err_family_fingerprint() -> i64 {
    fingerprint_families(ERR_FAMILIES)
}

/// One-time, idempotent repair of rows written before the friction columns were
/// populated. Gated on `PRAGMA user_version` so it runs once per DB and is a
/// cheap no-op (one pragma read) on every subsequent open.
///
/// Two columns are reconstructable from data still on the row:
/// - `project_root`: every row in a given `usage.db` belongs to that file's
///   project (the DB lives at `<root>/.codescout/usage.db`), so a blanket fill
///   of the NULLs is correct.
/// - `err_family`: `error_msg` is retained on every error row, so re-running the
///   classifier recovers the family. Only NULL families are touched — to re-map
///   an already-classified family after a taxonomy change, clear it first.
///
/// `friction_target` and `overflow_tokens` are NOT backfillable: their source
/// (the call's input / buffered output) is only persisted in debug mode, so old
/// rows can't be reconstructed. They self-heal as pre-migration rows age out
/// under the 30-day retention sweep in `write_record`.
fn backfill_legacy_rows(conn: &Connection, project_root: &str) -> Result<()> {
    let current: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    let fingerprint = err_family_fingerprint();
    if current == fingerprint {
        return Ok(());
    }

    conn.execute(
        "UPDATE tool_calls SET project_root = ?1 WHERE project_root IS NULL",
        params![project_root],
    )?;

    let unclassified: Vec<(i64, String, String)> = {
        let mut stmt = conn.prepare(
            "SELECT id, tool_name, error_msg FROM tool_calls \
             WHERE err_family IS NULL AND error_msg IS NOT NULL",
        )?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        rows.collect::<std::result::Result<_, _>>()?
    };
    for (id, tool_name, msg) in unclassified {
        if let Some(family) = normalize_err_family(&tool_name, &msg) {
            conn.execute(
                "UPDATE tool_calls SET err_family = ?1 WHERE id = ?2",
                params![family, id],
            )?;
        }
    }

    conn.execute_batch(&format!("PRAGMA user_version = {fingerprint};"))?;
    Ok(())
}

/// Record an LSP cold-start event. Returns the inserted row id for the
/// two-phase write (first_response_ms is filled in later by `update_lsp_first_response`).
pub fn write_lsp_event(
    conn: &Connection,
    language: &str,
    reason: &str,
    handshake_ms: i64,
) -> Result<i64> {
    conn.execute(
        "INSERT INTO lsp_events (language, reason, handshake_ms) VALUES (?1, ?2, ?3)",
        params![language, reason, handshake_ms],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Record a *failed* LSP start: the server disconnected or errored during the
/// `initialize` handshake, so no session was established. `handshake_ms` is the
/// time elapsed until the failure. Recorded as a separate `outcome='failed'`
/// row so a chronically-failing server (e.g. an expired LSP build) is visible
/// in lsp_events rather than as a silent absence of `success` rows.
pub fn write_lsp_failure(
    conn: &Connection,
    language: &str,
    reason: &str,
    handshake_ms: i64,
    error: &str,
) -> Result<i64> {
    conn.execute(
        "INSERT INTO lsp_events (language, reason, handshake_ms, outcome, error)
         VALUES (?1, ?2, ?3, 'failed', ?4)",
        params![language, reason, handshake_ms, error],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Fill in the first_response_ms for a previously inserted lsp_events row.
/// Best-effort — if the row was already updated or is missing, this is a no-op.
pub fn update_lsp_first_response(
    conn: &Connection,
    rowid: i64,
    first_response_ms: i64,
) -> Result<()> {
    conn.execute(
        "UPDATE lsp_events SET first_response_ms = ?1 WHERE id = ?2 AND first_response_ms IS NULL",
        params![first_response_ms, rowid],
    )?;
    Ok(())
}

#[derive(Debug, serde::Serialize)]
pub struct ToolStats {
    pub tool: String,
    pub calls: i64,
    pub errors: i64,
    pub error_rate_pct: f64,
    pub overflows: i64,
    pub overflow_rate_pct: f64,
    pub p50_ms: i64,
    pub p99_ms: i64,
}

#[derive(Debug, serde::Serialize)]
pub struct UsageStats {
    pub window: String,
    pub total_calls: i64,
    pub by_tool: Vec<ToolStats>,
}

#[derive(Debug, Default, serde::Serialize)]
pub struct LspReasonCounts {
    pub new_session: i64,
    pub idle_evicted: i64,
    pub lru_evicted: i64,
    pub crashed: i64,
}

#[derive(Debug, serde::Serialize)]
pub struct LspLanguageStats {
    pub language: String,
    pub starts: i64,
    pub failures: i64,
    pub reasons: LspReasonCounts,
    pub avg_handshake_ms: i64,
    pub p95_handshake_ms: i64,
    pub avg_first_response_ms: Option<i64>,
    pub p95_first_response_ms: Option<i64>,
}

#[derive(Debug, serde::Serialize)]
pub struct LspEvent {
    pub language: String,
    pub started_at: String,
    pub reason: String,
    pub handshake_ms: i64,
    pub first_response_ms: Option<i64>,
}

/// A failed LSP start (server died during `initialize`). `error` is the
/// caller-facing message (e.g. "LSP server disconnected").
#[derive(Debug, serde::Serialize)]
pub struct LspFailure {
    pub language: String,
    pub started_at: String,
    pub reason: String,
    pub error: Option<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct LspStats {
    pub window: String,
    pub by_language: Vec<LspLanguageStats>,
    pub recent: Vec<LspEvent>,
    pub recent_failures: Vec<LspFailure>,
}

pub fn query_lsp_stats(conn: &Connection, window: &str) -> Result<LspStats> {
    let modifier = window_to_modifier(window);

    // Aggregate per language. `starts` and the handshake metrics count only
    // successful starts; `failures` counts starts that died during `initialize`
    // (e.g. an expired LSP build). A language that fails *every* start still
    // appears here with starts=0, failures>0 — the case we most want visible.
    let mut agg_stmt = conn.prepare(
        "SELECT language,
                SUM(CASE WHEN outcome = 'success' THEN 1 ELSE 0 END) as starts,
                SUM(CASE WHEN outcome = 'failed'  THEN 1 ELSE 0 END) as failures,
                SUM(CASE WHEN outcome = 'success' AND reason = 'new_session'  THEN 1 ELSE 0 END),
                SUM(CASE WHEN outcome = 'success' AND reason = 'idle_evicted' THEN 1 ELSE 0 END),
                SUM(CASE WHEN outcome = 'success' AND reason = 'lru_evicted'  THEN 1 ELSE 0 END),
                SUM(CASE WHEN outcome = 'success' AND reason = 'crashed'      THEN 1 ELSE 0 END),
                AVG(CASE WHEN outcome = 'success' THEN handshake_ms END),
                AVG(CASE WHEN outcome = 'success' THEN first_response_ms END)
         FROM lsp_events
         WHERE started_at >= datetime('now', ?)
         GROUP BY language
         ORDER BY starts DESC, failures DESC",
    )?;

    #[allow(clippy::type_complexity)]
    let rows: Vec<(
        String,
        i64,
        i64,
        i64,
        i64,
        i64,
        i64,
        Option<f64>,
        Option<f64>,
    )> = agg_stmt
        .query_map([modifier], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
                r.get(6)?,
                r.get(7)?,
                r.get(8)?,
            ))
        })?
        .collect::<rusqlite::Result<_>>()?;

    let mut by_language = Vec::new();
    for (
        language,
        starts,
        failures,
        new_session,
        idle_evicted,
        lru_evicted,
        crashed,
        avg_handshake,
        avg_first,
    ) in rows
    {
        let p95_handshake = lsp_percentile(conn, &language, modifier, 95, "handshake_ms")?;
        // `.ok()` is intentional: `p95_first_response_ms` is an Optional field in the response.
        // `lsp_percentile` returns `Ok(0)` when count=0 (all NULL values), so the only case
        // `.ok()` silently discards is a genuine DB error — acceptable for a best-effort
        // observability field.
        let p95_first = lsp_percentile(conn, &language, modifier, 95, "first_response_ms").ok();

        by_language.push(LspLanguageStats {
            language,
            starts,
            failures,
            reasons: LspReasonCounts {
                new_session,
                idle_evicted,
                lru_evicted,
                crashed,
            },
            // None = no successful start in the window (e.g. a fail-only language) → 0.
            avg_handshake_ms: avg_handshake.map(|v| v.round() as i64).unwrap_or(0),
            p95_handshake_ms: p95_handshake,
            avg_first_response_ms: avg_first.map(|v| v.round() as i64),
            p95_first_response_ms: p95_first,
        });
    }

    // Recent successful events (last 20, not window-filtered — always shows the most
    // recent cold starts regardless of the selected window, so the list is never empty
    // while data exists).
    let mut recent_stmt = conn.prepare(
        "SELECT language, started_at, reason, handshake_ms, first_response_ms
         FROM lsp_events
         WHERE outcome = 'success'
         ORDER BY started_at DESC
         LIMIT 20",
    )?;
    let recent: Vec<LspEvent> = recent_stmt
        .query_map([], |r| {
            Ok(LspEvent {
                language: r.get(0)?,
                started_at: r.get(1)?,
                reason: r.get(2)?,
                handshake_ms: r.get(3)?,
                first_response_ms: r.get(4)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;

    // Recent failed starts (last 20, not window-filtered) — the actionable signal:
    // which server keeps dying during `initialize`, and the error it reported.
    let mut fail_stmt = conn.prepare(
        "SELECT language, started_at, reason, error
         FROM lsp_events
         WHERE outcome = 'failed'
         ORDER BY started_at DESC
         LIMIT 20",
    )?;
    let recent_failures: Vec<LspFailure> = fail_stmt
        .query_map([], |r| {
            Ok(LspFailure {
                language: r.get(0)?,
                started_at: r.get(1)?,
                reason: r.get(2)?,
                error: r.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;

    Ok(LspStats {
        window: window.to_string(),
        by_language,
        recent,
        recent_failures,
    })
}

fn lsp_percentile(
    conn: &Connection,
    language: &str,
    modifier: &str,
    pct: i64,
    column: &str,
) -> Result<i64> {
    let column = match column {
        "handshake_ms" => "handshake_ms",
        "first_response_ms" => "first_response_ms",
        _ => anyhow::bail!("lsp_percentile: unexpected column '{column}' — only hardcoded column literals are safe"),
    };
    // Only count non-NULL values for the given column
    let count: i64 = conn.query_row(
        &format!(
            "SELECT COUNT({}) FROM lsp_events\n             WHERE language = ? AND outcome = 'success' AND started_at >= datetime('now', ?) AND {} IS NOT NULL",
            column, column
        ),
        params![language, modifier],
        |r| r.get(0),
    )?;
    if count == 0 {
        return Ok(0);
    }
    let offset = ((count * pct + 99) / 100 - 1).max(0);
    let val: i64 = conn.query_row(
        &format!(
            "SELECT {} FROM lsp_events\n             WHERE language = ? AND outcome = 'success' AND started_at >= datetime('now', ?) AND {} IS NOT NULL\n             ORDER BY {} LIMIT 1 OFFSET ?",
            column, column, column
        ),
        params![language, modifier, offset],
        |r| r.get(0),
    )?;
    Ok(val)
}
pub fn query_stats(conn: &Connection, window: &str) -> Result<UsageStats> {
    let modifier = window_to_modifier(window);
    let mut stmt = conn.prepare(
        "SELECT tool_name,
                COUNT(*) as calls,
                SUM(CASE WHEN outcome IN ('error', 'recoverable_error') THEN 1 ELSE 0 END) as errors,
                SUM(overflowed) as overflows
         FROM tool_calls
         WHERE called_at >= datetime('now', ?)
         GROUP BY tool_name
         ORDER BY calls DESC",
    )?;

    let rows: Vec<(String, i64, i64, i64)> = stmt
        .query_map([modifier], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
        })?
        .collect::<rusqlite::Result<_>>()?;

    let total_calls: i64 = rows.iter().map(|r| r.1).sum();

    let mut by_tool = Vec::new();
    for (tool_name, calls, errors, overflows) in rows {
        let p50_ms = percentile(conn, &tool_name, modifier, 50)?;
        let p99_ms = percentile(conn, &tool_name, modifier, 99)?;
        by_tool.push(ToolStats {
            error_rate_pct: if calls > 0 {
                errors as f64 / calls as f64 * 100.0
            } else {
                0.0
            },
            overflow_rate_pct: if calls > 0 {
                overflows as f64 / calls as f64 * 100.0
            } else {
                0.0
            },
            tool: tool_name,
            calls,
            errors,
            overflows,
            p50_ms,
            p99_ms,
        });
    }

    Ok(UsageStats {
        window: window.to_string(),
        total_calls,
        by_tool,
    })
}

fn percentile(conn: &Connection, tool_name: &str, modifier: &str, pct: i64) -> Result<i64> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM tool_calls WHERE tool_name = ? AND called_at >= datetime('now', ?)",
        params![tool_name, modifier],
        |r| r.get(0),
    )?;
    if count == 0 {
        return Ok(0);
    }
    // Nearest-rank method: ceil(count * pct / 100) - 1 (0-indexed)
    let offset = ((count * pct + 99) / 100 - 1).max(0);
    let val: i64 = conn.query_row(
        "SELECT latency_ms FROM tool_calls
         WHERE tool_name = ? AND called_at >= datetime('now', ?)
         ORDER BY latency_ms
         LIMIT 1 OFFSET ?",
        params![tool_name, modifier, offset],
        |r| r.get(0),
    )?;
    Ok(val)
}

fn window_to_modifier(window: &str) -> &'static str {
    match window {
        "1h" => "-1 hours",
        "24h" => "-24 hours",
        "7d" => "-7 days",
        _ => "-30 days",
    }
}

#[derive(Debug, serde::Serialize)]
pub struct ErrorRecord {
    pub tool: String,
    pub timestamp: String,
    pub outcome: String,
    pub message: Option<String>,
}

pub fn recent_errors(conn: &Connection, limit: i64) -> Result<Vec<ErrorRecord>> {
    let mut stmt = conn.prepare(
        "SELECT tool_name, called_at, outcome, error_msg
         FROM tool_calls
         WHERE outcome IN ('error', 'recoverable_error')
         ORDER BY called_at DESC, rowid DESC
         LIMIT ?",
    )?;
    let rows = stmt
        .query_map([limit], |r| {
            Ok(ErrorRecord {
                tool: r.get(0)?,
                timestamp: r.get(1)?,
                outcome: r.get(2)?,
                message: r.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn tmp() -> (TempDir, Connection) {
        let dir = TempDir::new().unwrap();
        let conn = open_db(dir.path()).unwrap();
        (dir, conn)
    }

    #[test]
    fn open_db_creates_table() {
        let (_dir, conn) = tmp();
        // table exists if this doesn't error
        conn.execute("SELECT 1 FROM tool_calls LIMIT 0", [])
            .unwrap();
    }

    #[test]
    fn write_record_roundtrip() {
        let (_dir, conn) = tmp();
        write_record(
            &conn,
            "symbols",
            42,
            "success",
            false,
            None,
            "unknown",
            None,
            "test-session",
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM tool_calls", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn write_record_stores_all_fields() {
        let (_dir, conn) = tmp();
        write_record(
            &conn,
            "semantic_search",
            150,
            "recoverable_error",
            false,
            Some("path not found"),
            "unknown",
            None,
            "test-session",
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        let (name, latency, outcome, overflowed, msg): (String, i64, String, i64, Option<String>) =
            conn.query_row(
                "SELECT tool_name, latency_ms, outcome, overflowed, error_msg FROM tool_calls",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )
            .unwrap();
        assert_eq!(name, "semantic_search");
        assert_eq!(latency, 150);
        assert_eq!(outcome, "recoverable_error");
        assert_eq!(overflowed, 0);
        assert_eq!(msg.as_deref(), Some("path not found"));
    }

    #[test]
    fn write_record_overflow_flag() {
        let (_dir, conn) = tmp();
        write_record(
            &conn,
            "references",
            80,
            "success",
            true,
            None,
            "unknown",
            None,
            "test-session",
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        let overflowed: i64 = conn
            .query_row("SELECT overflowed FROM tool_calls", [], |r| r.get(0))
            .unwrap();
        assert_eq!(overflowed, 1);
    }

    #[test]
    fn retention_prunes_old_rows() {
        let (_dir, conn) = tmp();
        // Insert a row with a timestamp 31 days ago
        conn.execute(
            "INSERT INTO tool_calls (tool_name, called_at, latency_ms, outcome, overflowed)
             VALUES ('old_tool', datetime('now', '-31 days'), 10, 'success', 0)",
            [],
        )
        .unwrap();
        let before: i64 = conn
            .query_row("SELECT COUNT(*) FROM tool_calls", [], |r| r.get(0))
            .unwrap();
        assert_eq!(before, 1);

        // Next write triggers pruning
        write_record(
            &conn,
            "new_tool",
            5,
            "success",
            false,
            None,
            "unknown",
            None,
            "test-session",
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        let after: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM tool_calls WHERE tool_name = 'old_tool'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(after, 0);
    }

    #[test]
    /// Regression for
    /// docs/issues/archive/2026-08-20-pika-observations-orphaned-by-the-retention-sweep.md:
    /// `pika_observations` declares `ON DELETE CASCADE` but usage.db never enables
    /// `PRAGMA foreign_keys`, so the sweep used to delete a referenced parent and leave
    /// the observation pointing at a row that no longer exists. The sweep must now skip
    /// a `tool_calls` row that a `pika_observations` row still points at.
    fn retention_spares_a_row_referenced_by_a_pika_observation() {
        let (_dir, conn) = tmp();
        // Shape the plugin's table exactly as its own bootstrap SQL does — codescout
        // does not own this DDL, it only has to detect the table by name.
        conn.execute_batch(
            "CREATE TABLE pika_observations (
                 id            INTEGER PRIMARY KEY AUTOINCREMENT,
                 tool_call_id  INTEGER REFERENCES tool_calls(id) ON DELETE CASCADE,
                 kind          TEXT,
                 severity      TEXT
             );",
        )
        .unwrap();

        conn.execute(
            "INSERT INTO tool_calls (tool_name, called_at, latency_ms, outcome, overflowed)
             VALUES ('observed_tool', datetime('now', '-31 days'), 10, 'success', 0)",
            [],
        )
        .unwrap();
        let observed_id: i64 = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO pika_observations (tool_call_id, kind, severity) VALUES (?1, 'tool_bug', 'low')",
            params![observed_id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tool_calls (tool_name, called_at, latency_ms, outcome, overflowed)
             VALUES ('unobserved_tool', datetime('now', '-31 days'), 10, 'success', 0)",
            [],
        )
        .unwrap();

        // Next write triggers the sweep.
        write_record(
            &conn,
            "new_tool",
            5,
            "success",
            false,
            None,
            "unknown",
            None,
            "test-session",
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();

        let observed_survives: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM tool_calls WHERE tool_name = 'observed_tool'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            observed_survives, 1,
            "a row a pika_observations row still points at must survive the sweep"
        );

        let unobserved_pruned: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM tool_calls WHERE tool_name = 'unobserved_tool'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            unobserved_pruned, 0,
            "an equally old row with no observation must still be pruned normally"
        );

        let orphaned_observations: i64 = conn
            .query_row("SELECT COUNT(*) FROM pika_observations", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            orphaned_observations, 1,
            "the observation itself is untouched — codescout only spares its parent"
        );
    }

    fn insert_call(conn: &Connection, tool: &str, latency: i64, outcome: &str, overflowed: bool) {
        conn.execute(
            "INSERT INTO tool_calls (tool_name, called_at, latency_ms, outcome, overflowed)
             VALUES (?1, datetime('now'), ?2, ?3, ?4)",
            params![tool, latency, outcome, overflowed as i64],
        )
        .unwrap();
    }

    #[test]
    fn query_stats_empty_db() {
        let (_dir, conn) = tmp();
        let stats = query_stats(&conn, "30d").unwrap();
        assert_eq!(stats.total_calls, 0);
        assert!(stats.by_tool.is_empty());
    }

    #[test]
    fn query_stats_counts_correctly() {
        let (_dir, conn) = tmp();
        insert_call(&conn, "symbols", 100, "success", false);
        insert_call(&conn, "symbols", 200, "success", false);
        insert_call(&conn, "symbols", 300, "error", false);
        insert_call(&conn, "semantic_search", 500, "success", true);

        let stats = query_stats(&conn, "30d").unwrap();
        assert_eq!(stats.total_calls, 4);
        assert_eq!(stats.by_tool.len(), 2);

        // symbols should be first (3 calls > 1)
        let fs = &stats.by_tool[0];
        assert_eq!(fs.tool, "symbols");
        assert_eq!(fs.calls, 3);
        assert_eq!(fs.errors, 1);
        assert_eq!(fs.overflows, 0);

        let ss = &stats.by_tool[1];
        assert_eq!(ss.tool, "semantic_search");
        assert_eq!(ss.overflows, 1);
    }

    #[test]
    fn query_stats_percentiles() {
        let (_dir, conn) = tmp();
        // Insert 10 calls with known latencies 10..100ms
        for i in 1..=10 {
            insert_call(&conn, "symbols", i * 10, "success", false);
        }
        let stats = query_stats(&conn, "30d").unwrap();
        let fs = &stats.by_tool[0];
        // p50 = 50ms (5th of 10, 0-indexed offset 5)
        assert_eq!(fs.p50_ms, 50);
        // p99 = ~100ms (last item)
        assert_eq!(fs.p99_ms, 100);
    }

    #[test]
    fn query_stats_window_excludes_old_rows() {
        let (_dir, conn) = tmp();
        // Insert a row 2 days ago
        conn.execute(
            "INSERT INTO tool_calls (tool_name, called_at, latency_ms, outcome, overflowed)
             VALUES ('old_tool', datetime('now', '-2 days'), 50, 'success', 0)",
            [],
        )
        .unwrap();
        insert_call(&conn, "new_tool", 10, "success", false);

        let stats_1h = query_stats(&conn, "1h").unwrap();
        // Only new_tool (inserted now) should appear in 1h window
        assert_eq!(stats_1h.total_calls, 1);
        assert_eq!(stats_1h.by_tool[0].tool, "new_tool");
    }

    #[test]
    fn recent_errors_returns_latest_errors() {
        let (_dir, conn) = tmp();
        write_record(
            &conn,
            "symbols",
            50,
            "success",
            false,
            None,
            "unknown",
            None,
            "test-session",
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        write_record(
            &conn,
            "semantic_search",
            100,
            "error",
            false,
            Some("index missing"),
            "unknown",
            None,
            "test-session",
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        write_record(
            &conn,
            "references",
            30,
            "recoverable_error",
            false,
            Some("path not found"),
            "unknown",
            None,
            "test-session",
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();

        let errors = recent_errors(&conn, 10).unwrap();
        assert_eq!(errors.len(), 2);
        // Most recent first
        assert_eq!(errors[0].tool, "references");
        assert_eq!(errors[1].tool, "semantic_search");
    }

    #[test]
    fn recent_errors_respects_limit() {
        let (_dir, conn) = tmp();
        for i in 0..5 {
            write_record(
                &conn,
                &format!("tool_{}", i),
                10,
                "error",
                false,
                Some("fail"),
                "unknown",
                None,
                "test-session",
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .unwrap();
        }
        let errors = recent_errors(&conn, 3).unwrap();
        assert_eq!(errors.len(), 3);
    }

    #[test]
    fn write_lsp_event_returns_rowid() {
        let (_dir, conn) = tmp();
        let rowid = write_lsp_event(&conn, "rust", "new_session", 820).unwrap();
        assert!(rowid > 0);
    }

    #[test]
    fn write_lsp_failure_records_failed_outcome() {
        let (_dir, conn) = tmp();
        let rowid = write_lsp_failure(
            &conn,
            "kotlin",
            "new_session",
            813,
            "LSP server disconnected",
        )
        .unwrap();
        assert!(rowid > 0);
        let (outcome, error): (String, Option<String>) = conn
            .query_row(
                "SELECT outcome, error FROM lsp_events WHERE id = ?",
                [rowid],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(outcome, "failed");
        assert_eq!(error.as_deref(), Some("LSP server disconnected"));
    }

    #[test]
    fn write_lsp_event_defaults_outcome_to_success() {
        let (_dir, conn) = tmp();
        let rowid = write_lsp_event(&conn, "rust", "new_session", 820).unwrap();
        let outcome: String = conn
            .query_row(
                "SELECT outcome FROM lsp_events WHERE id = ?",
                [rowid],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(outcome, "success");
    }

    #[test]
    fn query_lsp_stats_excludes_failed_starts() {
        let (_dir, conn) = tmp();
        write_lsp_event(&conn, "kotlin", "new_session", 3000).unwrap();
        write_lsp_failure(
            &conn,
            "kotlin",
            "new_session",
            800,
            "LSP server disconnected",
        )
        .unwrap();

        let stats = query_lsp_stats(&conn, "30d").unwrap();
        let kotlin = stats
            .by_language
            .iter()
            .find(|l| l.language == "kotlin")
            .unwrap();
        // A failed start must not inflate the success count or skew the handshake avg.
        assert_eq!(kotlin.starts, 1);
        assert_eq!(kotlin.avg_handshake_ms, 3000);
        // ...but it IS counted as a failure and surfaced in recent_failures.
        assert_eq!(kotlin.failures, 1);
        assert_eq!(stats.recent_failures.len(), 1);
        assert_eq!(stats.recent_failures[0].language, "kotlin");
        assert_eq!(
            stats.recent_failures[0].error.as_deref(),
            Some("LSP server disconnected")
        );
    }

    #[test]
    fn query_lsp_stats_surfaces_fail_only_language() {
        let (_dir, conn) = tmp();
        // kotlin has ONLY a failed start — it must still appear in by_language
        // (starts=0, failures=1), not vanish the way a success-only aggregate would.
        write_lsp_failure(
            &conn,
            "kotlin",
            "new_session",
            800,
            "LSP server disconnected",
        )
        .unwrap();

        let stats = query_lsp_stats(&conn, "30d").unwrap();
        let kotlin = stats
            .by_language
            .iter()
            .find(|l| l.language == "kotlin")
            .expect("a fail-only language must still appear in by_language");
        assert_eq!(kotlin.starts, 0);
        assert_eq!(kotlin.failures, 1);
        assert_eq!(kotlin.avg_handshake_ms, 0);
        assert_eq!(stats.recent_failures.len(), 1);
    }

    #[test]
    fn update_lsp_first_response_fills_null() {
        let (_dir, conn) = tmp();
        let rowid = write_lsp_event(&conn, "rust", "new_session", 820).unwrap();
        // Before update: first_response_ms should be NULL
        let val: Option<i64> = conn
            .query_row(
                "SELECT first_response_ms FROM lsp_events WHERE id = ?",
                [rowid],
                |r| r.get(0),
            )
            .unwrap();
        assert!(val.is_none());
        // After update: should be set
        update_lsp_first_response(&conn, rowid, 9100).unwrap();
        let val: Option<i64> = conn
            .query_row(
                "SELECT first_response_ms FROM lsp_events WHERE id = ?",
                [rowid],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(val, Some(9100));
    }

    #[test]
    fn query_lsp_stats_aggregates_correctly() {
        let (_dir, conn) = tmp();
        write_lsp_event(&conn, "rust", "new_session", 800).unwrap();
        write_lsp_event(&conn, "rust", "idle_evicted", 1200).unwrap();
        write_lsp_event(&conn, "kotlin", "new_session", 5000).unwrap();

        let stats = query_lsp_stats(&conn, "30d").unwrap();
        assert_eq!(stats.by_language.len(), 2);

        let rust = stats
            .by_language
            .iter()
            .find(|l| l.language == "rust")
            .unwrap();
        assert_eq!(rust.starts, 2);
        assert_eq!(rust.reasons.new_session, 1);
        assert_eq!(rust.reasons.idle_evicted, 1);
        assert_eq!(rust.avg_handshake_ms, 1000); // (800 + 1200) / 2
        assert!(rust.p95_handshake_ms >= 800);

        let kotlin = stats
            .by_language
            .iter()
            .find(|l| l.language == "kotlin")
            .unwrap();
        assert_eq!(kotlin.starts, 1);
        assert_eq!(kotlin.avg_handshake_ms, 5000);
    }

    #[test]
    fn query_lsp_stats_window_excludes_old_rows() {
        let (_dir, conn) = tmp();
        // Insert an old row manually with an ancient timestamp
        conn.execute(
            "INSERT INTO lsp_events (language, started_at, reason, handshake_ms)
             VALUES ('rust', datetime('now', '-60 days'), 'new_session', 999)",
            [],
        )
        .unwrap();
        // Insert a recent row
        write_lsp_event(&conn, "rust", "new_session", 800).unwrap();

        let stats = query_lsp_stats(&conn, "30d").unwrap();
        let rust = stats
            .by_language
            .iter()
            .find(|l| l.language == "rust")
            .unwrap();
        // Only the recent row should be counted
        assert_eq!(rust.starts, 1);
        assert_eq!(rust.avg_handshake_ms, 800);
    }

    #[test]
    fn query_lsp_stats_recent_returns_last_20() {
        let (_dir, conn) = tmp();
        for i in 0..25i64 {
            write_lsp_event(&conn, "rust", "new_session", i * 10).unwrap();
        }
        let stats = query_lsp_stats(&conn, "30d").unwrap();
        assert_eq!(stats.recent.len(), 20);
    }

    #[test]
    fn open_db_migrates_traceability_columns() {
        let dir = TempDir::new().unwrap();
        let conn = open_db(dir.path()).unwrap();
        conn.execute(
            "INSERT INTO tool_calls (tool_name, called_at, latency_ms, outcome, codescout_sha, project_sha, session_id, input_json, output_json)
             VALUES ('test', datetime('now'), 10, 'success', 'abc1234', 'def5678', 'sess-1', '{\"q\":\"x\"}', NULL)",
            [],
        )
        .unwrap();
        type Row = (
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
        );
        let (cs, ps, sid, inp, out): Row = conn
            .query_row(
                "SELECT codescout_sha, project_sha, session_id, input_json, output_json FROM tool_calls",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )
            .unwrap();
        assert_eq!(cs.as_deref(), Some("abc1234"));
        assert_eq!(ps.as_deref(), Some("def5678"));
        assert_eq!(sid.as_deref(), Some("sess-1"));
        assert_eq!(inp.as_deref(), Some("{\"q\":\"x\"}"));
        assert!(out.is_none());
    }

    #[test]
    fn open_db_migrates_friction_columns() {
        let dir = TempDir::new().unwrap();
        let conn = open_db(dir.path()).unwrap();
        conn.execute(
            "INSERT INTO tool_calls (tool_name, latency_ms, outcome, friction_target, overflow_tokens, err_family, project_root)
             VALUES ('symbols', 10, 'success', 'LspManager/get_or_start', 1045, NULL, '/repo')",
            [],
        )
        .unwrap();
        let (ft, tok, ef, pr): (Option<String>, Option<i64>, Option<String>, Option<String>) = conn
            .query_row(
                "SELECT friction_target, overflow_tokens, err_family, project_root FROM tool_calls",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap();
        assert_eq!(ft.as_deref(), Some("LspManager/get_or_start"));
        assert_eq!(tok, Some(1045));
        assert_eq!(ef, None);
        assert_eq!(pr.as_deref(), Some("/repo"));
    }

    #[test]
    fn write_record_stores_traceability_fields() {
        let (_dir, conn) = tmp();
        write_record(
            &conn,
            "symbols",
            42,
            "error",
            false,
            Some("not found"),
            "abc1234",
            Some("def5678"),
            "sess-1",
            Some("{\"query\":\"foo\"}"),
            Some("{\"error\":\"not found\"}"),
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        let (cs, ps, sid, inp, out): (String, String, String, String, String) = conn
            .query_row(
                "SELECT codescout_sha, project_sha, session_id, input_json, output_json FROM tool_calls",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )
            .unwrap();
        assert_eq!(cs, "abc1234");
        assert_eq!(ps, "def5678");
        assert_eq!(sid, "sess-1");
        assert_eq!(inp, "{\"query\":\"foo\"}");
        assert_eq!(out, "{\"error\":\"not found\"}");
    }

    #[test]
    fn write_record_traceability_fields_nullable() {
        let (_dir, conn) = tmp();
        write_record(
            &conn, "symbols", 42, "success", false, None, "abc1234", None, "sess-1", None, None,
            None, None, None, None, None,
        )
        .unwrap();
        let (ps, inp, out): (Option<String>, Option<String>, Option<String>) = conn
            .query_row(
                "SELECT project_sha, input_json, output_json FROM tool_calls",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert!(ps.is_none());
        assert!(inp.is_none());
        assert!(out.is_none());
    }

    #[test]
    fn write_record_stores_friction_fields() {
        let (_dir, conn) = tmp();
        write_record(
            &conn,
            "symbols",
            42,
            "success",
            true,
            None,
            "cs-sha",
            Some("proj-sha"),
            "sess-1",
            None,
            None,
            None,
            Some("LspManager/get_or_start"),
            Some(1045),
            None,
            Some("/repo"),
        )
        .unwrap();
        let (ft, tok, ef, pr): (Option<String>, Option<i64>, Option<String>, Option<String>) = conn
            .query_row(
                "SELECT friction_target, overflow_tokens, err_family, project_root FROM tool_calls",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap();
        assert_eq!(ft.as_deref(), Some("LspManager/get_or_start"));
        assert_eq!(tok, Some(1045));
        assert_eq!(ef, None);
        assert_eq!(pr.as_deref(), Some("/repo"));
    }

    /// BL-24. `codescout_sha` is the column an acceptance measurement ranks on, and a
    /// sha without a dirty bit is not an identity: a dirty build of commit X contains
    /// arbitrary uncommitted work while claiming to be X. That misread a live fix as
    /// absent and took a behavioural re-check to disprove.
    #[test]
    fn write_record_records_the_builds_dirty_bit() {
        for (dirty, expected) in [(true, 1i64), (false, 0i64)] {
            let (_dir, conn) = tmp();
            write_record(
                &conn,
                "symbols",
                1,
                "success",
                false,
                None,
                BuildProvenance {
                    sha: "8ad83c42",
                    dirty,
                },
                None,
                "sess",
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .unwrap();
            let (sha, got): (String, Option<i64>) = conn
                .query_row(
                    "SELECT codescout_sha, codescout_dirty FROM tool_calls",
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .unwrap();
            assert_eq!(sha, "8ad83c42");
            assert_eq!(
                got,
                Some(expected),
                "a dirty={dirty} build must record it — the flag existing in the binary \
                 and not in the row is the whole defect"
            );
        }
    }

    /// The hole the `From<&str>` fixture convenience re-opens, closed where it matters.
    ///
    /// `write_record` accepts a bare `&str` so ~16 fixtures can keep passing a literal,
    /// and that path assumes `dirty: false`. Production must never take it — passing
    /// `env!("CODESCOUT_GIT_SHA")` there is *precisely* BL-24, and it would compile
    /// silently. Asserted against the recorder's own source, because no runtime
    /// assertion can distinguish "recorded clean" from "assumed clean".
    #[test]
    fn the_recorder_never_assumes_a_clean_build() {
        const RECORDER_SRC: &str = include_str!("mod.rs");

        assert!(
            RECORDER_SRC.contains("BuildProvenance::current()"),
            "the production recorder must pass measured provenance, not a bare sha"
        );
        let bare_sha = concat!("env!(\"CODESCOUT_", "GIT_SHA\")");
        assert!(
            !RECORDER_SRC.contains(bare_sha),
            "a bare sha routes through `From<&str>` and is recorded as CLEAN whatever the \
             tree actually was — that is BL-24 exactly, and it compiles without complaint"
        );
    }

    #[test]
    fn normalize_err_family_maps_iron_law_routing_errors() {
        // The families that dominate the real error population — previously all NULL.
        let cases = [
            (
                "read_file",
                "source range overlaps named symbol(s): 'open_db'",
                Some("il1_read_overlaps_symbol"),
            ),
            (
                "read_file",
                "Use read_markdown for markdown files",
                Some("il4_read_markdown_routing"),
            ),
            (
                "edit_file",
                "Use edit_markdown for markdown files",
                Some("il5_edit_markdown_routing"),
            ),
            (
                "edit_file",
                "edit contains a symbol definition (\"def \") — use symbol tools",
                Some("il2_structural_edit"),
            ),
            (
                "edit_file",
                "edit_file is blocked for structural edits on source code files",
                Some("il2_structural_edit"),
            ),
            (
                "run_command",
                "shell access to source files is blocked",
                Some("il3_shell_on_source"),
            ),
            (
                "run_command",
                "IL3 violation — piped `cargo test` to a log-trimmer. BLOCKED.",
                Some("il3_pipe_to_trimmer"),
            ),
            (
                "create_file",
                "write denied: '/x/INDEX.md' is outside the project root",
                Some("write_scope_denied"),
            ),
            (
                "read_file",
                "unsupported json_path segment '[*]'",
                Some("json_path_unsupported"),
            ),
            (
                "edit_file",
                "old_string not found in src/x.rs",
                Some("edit_stale_match"),
            ),
            (
                "edit_markdown",
                "old_string not found in section 'Foo'. The text must match exactly (whitespace-sensitive).",
                Some("edit_stale_match"),
            ),
            // Pre-existing families still resolve.
            ("symbols", "LSP server disconnected", Some("lsp_disconnect")),
            (
                "symbols",
                "symbol not found: Foo/bar",
                Some("symbol_not_found"),
            ),
            ("read_file", "some unrecognized failure", None),
            // New: read_markdown's own errors, previously untagged (the biggest
            // untagged bucket in usage.db — see the design spec's Problem section).
            (
                "read_markdown",
                "read_markdown only supports .md files",
                Some("read_markdown_wrong_ext"),
            ),
            (
                "read_markdown",
                "file not found: 'docs/MISSING.md'",
                Some("read_markdown_file_not_found"),
            ),
            (
                "read_markdown",
                "'docs/trackers' is a directory, not a file",
                Some("read_markdown_path_is_directory"),
            ),
            (
                "read_markdown",
                "combined headings span 812 lines — exceeds inline threshold",
                Some("read_markdown_overflow_threshold"),
            ),
            (
                "read_markdown",
                "section \"## Foo\" spans 900 lines — exceeds inline threshold",
                Some("read_markdown_overflow_threshold"),
            ),
            (
                "read_markdown",
                "heading and headings are mutually exclusive",
                Some("read_markdown_param_conflict"),
            ),
            (
                "read_markdown",
                "both start_line and end_line are required",
                Some("read_markdown_param_conflict"),
            ),
            (
                "read_markdown",
                "invalid line range: start_line=5 end_line=2",
                Some("read_markdown_invalid_line_range"),
            ),
            (
                "read_markdown",
                "start_line 900 exceeds file length 500",
                Some("read_markdown_invalid_line_range"),
            ),
            (
                "read_markdown",
                "heading 'SI-99' not found",
                Some("heading_not_found"),
            ),
            (
                "edit_markdown",
                "heading 'SI-99' not found",
                Some("heading_not_found"),
            ),
            // Double-quoted form: read_markdown's actual runtime message uses
            // `{:?}` (Rust Debug) on the heading query, which renders with double
            // quotes — distinct from edit_markdown's single-quoted Display text
            // above. The classifier must recognize both quote styles.
            (
                "read_markdown",
                "heading \"SI-99\" not found",
                Some("heading_not_found"),
            ),
            // Scoping proof: the same message text from an unrelated tool must NOT
            // pick up a read_markdown-specific family.
            (
                "some_other_tool",
                "read_markdown only supports .md files",
                None,
            ),
        ];
        for (tool_name, msg, want) in cases {
            assert_eq!(
                normalize_err_family(tool_name, msg),
                want,
                "tool={tool_name} msg={msg}"
            );
        }
    }

    /// Families measured against the *live-DB* unclassified population on
    /// 2026-08-15: 197 errors across the six actively-written `usage.db` files.
    /// Hit counts in the comments are from that population deliberately — the
    /// lifetime figure is dominated by five dead DBs frozen at `user_version=0`
    /// which no longer receive the backfill, so ranking on it measures history
    /// rather than the current surface.
    #[test]
    fn normalize_err_family_maps_the_unclassified_head() {
        let cases = [
            // Missing / conditionally-required params — 38 hits, the largest
            // family. Spans four distinct message shapes across five tools.
            (
                "edit_code",
                "missing 'symbol' parameter",
                Some("missing_required_param"),
            ),
            (
                "edit_code",
                "action 'insert' requires 'body'",
                Some("missing_required_param"),
            ),
            (
                "artifact",
                "missing field `patch`",
                Some("missing_required_param"),
            ),
            (
                "artifact_event",
                "note.text required",
                Some("missing_required_param"),
            ),
            (
                "artifact",
                "body_edits[1]: content is required for the insert_after action",
                Some("missing_required_param"),
            ),
            // json_path key miss — 27 hits. Distinct from `json_path_unsupported`:
            // there the syntax is rejected, here the syntax is fine and the key
            // simply is not in THIS buffer's shape. Different fix, so different
            // family — merging them would make the ranking undecidable.
            (
                "read_file",
                "path segment 'summary' not found — hint: Available keys: content, lines, hint",
                Some("json_path_key_miss"),
            ),
            (
                "read_file",
                "unsupported json_path segment '[*]'",
                Some("json_path_unsupported"),
            ),
            // Librarian routing guard — 25 hits. Same class as the IL routing
            // families: the gate rejected and re-routed to `artifact`.
            (
                "read_markdown",
                "'docs/trackers/x.md' is a librarian-managed artifact — do not read or edit it directly",
                Some("librarian_managed_artifact"),
            ),
            // Heading resolution, now also reachable from artifact's body_edits
            // batch, whose message is prefixed by the edit index so the existing
            // `starts_with("heading ")` arm never matched it.
            (
                "artifact",
                "body_edits[0]: heading '## Fix' not found",
                Some("heading_not_found"),
            ),
            (
                "artifact",
                "body_edits[2]: heading '### Git state' found 3 times (lines 10, 40, 90)",
                Some("ambiguous_heading"),
            ),
            // Healthy guards. Recorded so they are VISIBLE in the ranking, not so
            // they get "fixed" — cf. TU-7, where two high-volume guards were
            // working exactly as designed.
            (
                "artifact",
                "body_edits[0]: replace on '## Fix' would wipe 3 nested heading(s): ### A",
                Some("destructive_replace_blocked"),
            ),
            (
                "edit_code",
                "inserting near 'project_has_chunks' would introduce syntax errors — not written",
                Some("edit_would_break_syntax"),
            ),
            // Target already exists — 17 hits.
            (
                "create_file",
                "file already exists: /repo/docs/x.md",
                Some("target_already_exists"),
            ),
            // Target missing — 13 hits, three message shapes across four tools.
            (
                "symbols",
                "path not found: /repo/src/lib.rs",
                Some("path_not_found"),
            ),
            (
                "edit_file",
                "No such file or directory (os error 2)",
                Some("path_not_found"),
            ),
            // Unknown enum / field value — 8 hits.
            (
                "memory",
                "unknown action 'get'. Must be one of: read, write, list",
                Some("unknown_enum_value"),
            ),
            (
                "artifact",
                "status \"wip\" is not a bug status; use one of: open, investigating",
                Some("unknown_enum_value"),
            ),
            // Expired buffer handle — 6 hits. The overflow-recovery failure mode:
            // the agent held an @ref past the session that owned it.
            (
                "grep",
                "buffer reference not found: '@bg_00000033'",
                Some("buffer_ref_expired"),
            ),
            // Invalid regex — 5 hits.
            (
                "grep",
                "invalid regex: regex parse error: ^path = \"docs/(?!archive/)",
                Some("invalid_regex"),
            ),
            // old_string AMBIGUOUS, not absent. `edit_stale_match` means re-read
            // the file; this means add disambiguating context. Opposite actions,
            // so they must not share a family.
            (
                "edit_file",
                "old_string found 2 times (lines 115, 202). Include more surrounding context",
                Some("ambiguous_old_string"),
            ),
            (
                "edit_file",
                "old_string matches 2 regions after whitespace normalization (lines 1528-1528, 1540-1540)",
                Some("ambiguous_old_string"),
            ),
            (
                "edit_file",
                "old_string not found in src/x.rs",
                Some("edit_stale_match"),
            ),
            // edit_markdown's own wrong-extension error. read_markdown had an arm
            // from the start; its twin never did, so every edit_markdown-on-non-md
            // landed unclassified.
            (
                "edit_markdown",
                "edit_markdown only supports .md files",
                Some("edit_markdown_wrong_ext"),
            ),
            // Same failure, twin tool, no arm. The taxonomy was written
            // tool-by-tool, so wherever two tools share a failure mode only the
            // first-written one got an arm. `grep` had `buffer reference not
            // found`; `run_command`'s wording for the identical expiry differs,
            // and its hint text is word-for-word the same — proof they are one
            // family, not two.
            (
                "run_command",
                "background job ref not found: @bg_0000000b — hint: Buffer refs expire when the session ends",
                Some("buffer_ref_expired"),
            ),
            // Third instance of the same twin-gap: read_markdown has had a
            // line-range family since the taxonomy was written, read_file never did.
            (
                "read_file",
                "line range 136-145 is past end of file (104 lines)",
                Some("invalid_line_range"),
            ),
            (
                "read_file",
                "invalid line range: start_line=746 end_line=700 (start_line must be >= 1)",
                Some("invalid_line_range"),
            ),
            // read_markdown deliberately KEEPS its own tool-scoped family rather
            // than being folded into the generic one: unifying them would have to
            // re-map already-classified rows, and the backfill only ever fills
            // NULLs — it has no re-map path. Recorded as deferred, not as intent.
            (
                "read_markdown",
                "invalid line range: start_line=5 end_line=2",
                Some("read_markdown_invalid_line_range"),
            ),
            // Scoping proof, mirroring the read_markdown one above: the same text
            // from an unrelated tool must not pick up the markdown-specific family.
            (
                "some_other_tool",
                "edit_markdown only supports .md files",
                None,
            ),
            // The residual tail is genuinely one-off and stays None by design;
            // inventing a family per message would break the low-cardinality
            // contract this function is documented to hold.
            (
                "edit_code",
                "some brand new failure nobody has seen before",
                None,
            ),
        ];
        for (tool_name, msg, want) in cases {
            assert_eq!(
                normalize_err_family(tool_name, msg),
                want,
                "tool={tool_name} msg={msg}"
            );
        }
    }

    /// The 2026-08-20 unclassified head, measured on codescout's own `usage.db`:
    /// 73 of 1,139 errors carried no family.
    ///
    /// The population is not a general "untaught" tail — it is two concentrations.
    /// **49% is the librarian/artifact API surface** and **31% is a single worktree
    /// write gate**. Coverage by tool makes the same point: `run_command` sits at 0.2%
    /// unclassified and `read_markdown`/`grep`/`references` at 0%, against `artifact`
    /// 37.5% and `memory`/`symbols` ~50%. The taxonomy is a map of where someone did the
    /// work, not of which errors are hard.
    ///
    /// Do NOT read this as a friction ranking. Measured the same day, the unclassified
    /// bucket's immediate-repeat rate is **2.8%** against a ~4-5% corpus average — these
    /// are among the *healthiest* errors here, and an earlier claim that
    /// `err_family IS NULL` carried a 1.97x friction lift did not reproduce
    /// (`capability-proposals:CAP-9`, correction of 2026-08-20). Classification is worth
    /// doing because an unnamed family cannot be counted, trended, or given a
    /// `refusal_predicate` — not because it is where the pain is.
    #[test]
    fn normalize_err_family_maps_the_2026_08_20_unclassified_head() {
        let cases = [
            // The single largest member — 23 hits across four write tools, one gate.
            (
                "edit_markdown",
                "Write blocked: git worktrees detected but workspace(action='activate') \
                 has not been called. Worktrees: [/home/u/p/.claude/worktrees/x]",
                Some("worktree_activate_required"),
            ),
            (
                "create_file",
                "Write blocked: git worktrees detected but workspace(action='activate') \
                 has not been called.",
                Some("worktree_activate_required"),
            ),
            // One repair — "declare entry_collection" — reached from three call sites,
            // so one family. Splitting by entry point would scatter a single fix.
            (
                "artifact",
                "entry_filter set but this artifact is not augmented — declare \
                 entry_collection on its augmentation, or retrofit it",
                Some("entry_collection_missing"),
            ),
            (
                "artifact",
                "entry_filter set but the augmentation has no entry_collection — declare \
                 which params array holds the filterable rows",
                Some("entry_collection_missing"),
            ),
            (
                "artifact",
                "append_entry: `entry` fields cannot be stored without an \
                 `entry_collection` — hint: This ledger has no params array",
                Some("entry_collection_missing"),
            ),
            (
                "artifact",
                "extra must not contain frontmatter field(s) the schema already models: \
                 status — hint: pass `status=` as its own parameter instead.",
                Some("extra_models_reserved_field"),
            ),
            // Schema rejection on a write — the value is well-formed and out of range.
            (
                "artifact",
                "update_entry: patched entry violates params_schema: /tasks/38/status: \
                 \"partial\" is not one of [\"open\",\"in-progress\",\"done\"]",
                Some("params_schema_violation"),
            ),
            (
                "artifact_augment",
                "merged params violate params_schema: /tasks/0/id: \"T-1\" does not match \
                 \"^FT-[0-9]+$\"",
                Some("params_schema_violation"),
            ),
            // ORDERING GUARD: this message contains "has no augmentation", which
            // `artifact_not_augmented` also matches. The repair here is "declare the
            // ledger", so `ledger_not_declared` must win — it is placed first, and this
            // case fails if that order is ever reversed.
            (
                "artifact",
                "allocate_entry_id: artifact `c43df94e69ca915f` has no augmentation — \
                 hint: A ledger must be declared before ids can be allocated",
                Some("ledger_not_declared"),
            ),
            (
                "artifact",
                "allocate_entry_id: `/p/docs/trackers/x.md` does not declare an entry_prefix",
                Some("ledger_not_declared"),
            ),
            (
                "artifact_augment",
                "no augmentation for artifact '5696563f06b2c222' — call artifact_augment first",
                Some("artifact_not_augmented"),
            ),
            // Tool-scoped: `memory` names a thing that does not exist and lists the valid
            // ones. Scoped so no other tool's "not found" is swallowed by this arm.
            (
                "memory",
                "no sections matched — hint: available sections: Module Structure (src/)",
                Some("memory_target_not_found"),
            ),
            (
                "memory",
                "topic 'nonexistent-topic' not found — hint: no memory topics exist yet",
                Some("memory_target_not_found"),
            ),
            (
                "memory",
                "No project 'zz-not-a-project'. — hint: Valid project ids: codescout",
                Some("memory_target_not_found"),
            ),
            (
                "artifact",
                "at most one of `full`, `heading`, `headings`, `start_line`+`end_line` \
                 may be set",
                Some("mutually_exclusive_params"),
            ),
            (
                "artifact",
                "update_entry: `entry` is append_entry's parameter — this action takes \
                 `fields`",
                Some("entry_patch_param_misuse"),
            ),
            (
                "artifact",
                "update_entry: `fields` is empty — there is nothing to patch",
                Some("entry_patch_param_misuse"),
            ),
            // Three json_path failures that are NOT `json_path_unsupported` (which means
            // the SYNTAX was rejected) and NOT `json_path_key_miss` (the key is absent).
            // Wrong buffer KIND and wrong value SHAPE have their own repairs.
            (
                "read_file",
                "json_path is only supported on @tool_* refs, not '@cmd_0fd3b8c1'",
                Some("json_path_wrong_buffer_kind"),
            ),
            (
                "read_file",
                "json_path '[*]' needs an array, found object",
                Some("json_path_shape_mismatch"),
            ),
            (
                "read_file",
                "index 5 out of bounds for array of length 5 — hint: Use an index in 0..5",
                Some("json_path_shape_mismatch"),
            ),
            // Distinct from lsp_not_running / lsp_disconnect: the server is coming up and
            // the call is retryable as-is, which none of the other LSP families are.
            (
                "symbols",
                "language server for 'markdown' is still starting and there is no \
                 tree-sitter fallback for this language",
                Some("lsp_still_starting"),
            ),
            // A guard, tagged so it is visible in the ranking rather than invisible in the
            // NULL bucket — the TU-7 lesson applied to a security-class gate.
            (
                "run_command",
                "cwd '/home/u/other/hooks' escapes project root",
                Some("cwd_escapes_root"),
            ),
            // --- extensions to EXISTING families, not new ones ---
            (
                "get_guide",
                "unknown topic 'edit_code' — hint: available topics: error-handling",
                Some("unknown_enum_value"),
            ),
            (
                "artifact",
                "body_edits[0]: invalid at=\"end\"; expected 'end-of-section' (default) \
                 or 'after-heading-line'",
                Some("unknown_enum_value"),
            ),
            (
                "edit_file",
                "no file to edit at /p/.superpowers/sdd/x/t.md",
                Some("path_not_found"),
            ),
            // --- regression guards: the new arms must not steal existing families ---
            (
                "read_markdown",
                "combined headings span 142 lines — exceeds inline threshold",
                Some("read_markdown_overflow_threshold"),
            ),
            (
                "read_file",
                "path segment 'summary' not found — hint: Available keys: content",
                Some("json_path_key_miss"),
            ),
            (
                "edit_file",
                "old_string not found in file",
                Some("edit_stale_match"),
            ),
            (
                "run_command",
                "IL3 violation — piped `cargo test` to a log-trimmer. BLOCKED.",
                Some("il3_pipe_to_trimmer"),
            ),
        ];
        for (tool, msg, want) in cases {
            assert_eq!(
                normalize_err_family(tool, msg),
                want,
                "tool={tool} msg={msg}"
            );
        }
    }

    /// The coupling that silently freezes the corpus: `backfill_legacy_rows` skips
    /// a DB whose stored marker already matches, so one that ran the backfill never
    /// re-runs it. Under the old hand-maintained `BACKFILL_VERSION`, extending
    /// `normalize_err_family` without also editing that integer left every live
    /// DB's historical rows NULL, and new families tagged only future rows.
    ///
    /// This proves the *mechanism* re-runs on a marker mismatch. It does NOT prove
    /// the marker moves when the taxonomy grows — that is
    /// `err_families_lists_exactly_what_the_classifier_can_emit` plus
    /// `extending_the_taxonomy_moves_the_backfill_fingerprint`, and that pair is
    /// what actually closed BL-4. Said explicitly because the previous version of
    /// this test was mistaken for coverage of the bug it structurally could not
    /// see: it seeded below the constant, which makes the backfill run
    /// unconditionally, so the probe family filled either way.
    #[test]
    fn backfill_reruns_when_the_stored_marker_does_not_match() {
        let dir = TempDir::new().unwrap();
        let conn = open_db(dir.path()).unwrap();
        conn.execute(
            "INSERT INTO tool_calls (tool_name, latency_ms, outcome, error_msg, project_root, err_family) \
             VALUES ('read_file', 5, 'recoverable_error', 'path segment ''summary'' not found', '/repo', NULL)",
            [],
        )
        .unwrap();

        // A marker from an older taxonomy — here one of the pre-fingerprint
        // sequential versions, which is the state every real usage.db is in.
        conn.execute_batch("PRAGMA user_version = 4;").unwrap();
        drop(conn);

        let conn = open_db(dir.path()).unwrap();
        let fam: Option<String> = conn
            .query_row("SELECT err_family FROM tool_calls", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            fam.as_deref(),
            Some("json_path_key_miss"),
            "a DB whose stored marker differs from the current taxonomy fingerprint \
             must re-classify its historical rows on open"
        );

        let stamped: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            stamped,
            err_family_fingerprint(),
            "and must then be stamped with the current fingerprint, so the next open \
             is the cheap no-op this gate exists to be"
        );
    }

    /// BL-4. The classifier's emittable families existed only as return values in
    /// an if-chain, so nothing could enumerate them — which is why the backfill
    /// gate had to be a hand-maintained integer, and why extending the taxonomy
    /// without also editing that integer silently froze every already-backfilled
    /// DB's history. No error, no warning, no failing test.
    ///
    /// This is the guard that deletes the human step. The const and the classifier
    /// are pinned to each other in BOTH directions by reading this file's own
    /// source, so a new arm cannot ship without appearing in `ERR_FAMILIES` — and
    /// appearing there changes the fingerprint the gate derives from it.
    #[test]
    fn err_families_lists_exactly_what_the_classifier_can_emit() {
        const SRC: &str = include_str!("db.rs");
        // Scan the classifier only, never this test module: a `return Some("…")`
        // written in a test would otherwise read as a family.
        let classifier = SRC
            .split("mod tests {")
            .next()
            .expect("source is non-empty");

        let re = regex::Regex::new("return Some\\(\"([a-z0-9_]+)\"\\)").unwrap();
        let emitted: std::collections::BTreeSet<&str> = re
            .captures_iter(classifier)
            .map(|c| c.get(1).unwrap().as_str())
            .collect();
        assert!(
            !emitted.is_empty(),
            "the source scan found no families at all — the classifier's shape changed \
             and this guard is now vacuous, which is worse than a failing assert"
        );

        let listed: std::collections::BTreeSet<&str> = ERR_FAMILIES.iter().copied().collect();
        assert_eq!(
            emitted, listed,
            "ERR_FAMILIES must equal the set normalize_err_family returns — add the new \
             arm's family to the const, which is what re-runs the backfill on every DB"
        );

        assert_eq!(
            ERR_FAMILIES.len(),
            listed.len(),
            "ERR_FAMILIES must not repeat a family — the fingerprint is multiplicity-sensitive"
        );
        let mut sorted = ERR_FAMILIES.to_vec();
        sorted.sort_unstable();
        assert_eq!(
            ERR_FAMILIES,
            sorted.as_slice(),
            "keep ERR_FAMILIES sorted so a pure reordering cannot change the fingerprint"
        );
    }

    /// The other half of BL-4's fix, and the half the old test could not express:
    /// growing the taxonomy must move the marker *by itself*. With a hand-edited
    /// integer this was a convention; derived from `ERR_FAMILIES` it is arithmetic.
    #[test]
    fn extending_the_taxonomy_moves_the_backfill_fingerprint() {
        let mut extended = ERR_FAMILIES.to_vec();
        extended.push("a_newly_added_family");
        extended.sort_unstable();

        assert_ne!(
            fingerprint_families(&extended),
            err_family_fingerprint(),
            "adding an arm must change the stored marker, or every already-backfilled \
             DB keeps its historical rows unclassified"
        );

        // Renaming one family must move it too — a same-size taxonomy is still a
        // different taxonomy, and the old sequential scheme could not see that
        // either without a human noticing.
        let mut renamed = ERR_FAMILIES.to_vec();
        renamed[0] = "renamed_first_family";
        renamed.sort_unstable();
        assert_ne!(
            fingerprint_families(&renamed),
            err_family_fingerprint(),
            "re-mapping a family must change the stored marker"
        );
    }

    /// The fingerprint is PERSISTED, so its stability is part of the contract: if
    /// the hash changed between builds, every DB would re-run the backfill on every
    /// open, forever, and nothing would say why.
    ///
    /// Pinned against a fixed vector rather than against `ERR_FAMILIES`, so adding a
    /// family is a one-line change here — while an accidental rewrite of the hash
    /// itself still fails.
    #[test]
    fn the_family_fingerprint_is_stable_odd_and_never_zero() {
        assert_eq!(
            fingerprint_families(&["a"]),
            fingerprint_families(&["a"]),
            "the same input must fingerprint identically"
        );
        assert_eq!(
            fingerprint_families(&["a"]),
            0x1292_6369,
            "FNV-1a over \"a\\n\", halved and forced odd — changing the hash \
             implementation re-runs the backfill on every DB in existence"
        );

        // A separator has to be mixed in, or a list boundary is invisible.
        assert_ne!(
            fingerprint_families(&["ab", "c"]),
            fingerprint_families(&["a", "bc"]),
            "concatenating the families without a separator loses list boundaries"
        );

        let current = err_family_fingerprint();
        assert!(
            current > 0,
            "`PRAGMA user_version` is signed, and 0 means `never backfilled` — a \
             fingerprint that is negative or zero is not a usable marker"
        );
        assert_eq!(
            current % 2,
            1,
            "the marker is forced odd so it can never be 0"
        );
    }

    #[test]
    fn backfill_fills_project_root_and_err_family_once() {
        let dir = TempDir::new().unwrap();
        // First open runs the backfill on an empty DB and stamps user_version.
        let conn = open_db(dir.path()).unwrap();

        // Simulate legacy rows: friction columns NULL, but error_msg retained.
        conn.execute(
            "INSERT INTO tool_calls (tool_name, latency_ms, outcome, error_msg, project_root, err_family) VALUES \
             ('read_file', 5, 'recoverable_error', 'source range overlaps named symbol(s): foo', NULL, NULL), \
             ('edit_file', 5, 'recoverable_error', 'Use edit_markdown for markdown files', NULL, NULL), \
             ('symbols',   5, 'success', NULL, NULL, NULL)",
            [],
        )
        .unwrap();
        // Roll the marker back to simulate a pre-backfill DB, then re-open.
        conn.execute_batch("PRAGMA user_version = 0;").unwrap();
        drop(conn);

        let conn = open_db(dir.path()).unwrap();

        let pr_nulls: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM tool_calls WHERE project_root IS NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(pr_nulls, 0, "project_root backfilled for every row");

        let fam = |tool: &str| -> Option<String> {
            conn.query_row(
                "SELECT err_family FROM tool_calls WHERE tool_name = ?1",
                [tool],
                |r| r.get(0),
            )
            .unwrap()
        };
        assert_eq!(
            fam("read_file").as_deref(),
            Some("il1_read_overlaps_symbol")
        );
        assert_eq!(
            fam("edit_file").as_deref(),
            Some("il5_edit_markdown_routing")
        );
        assert_eq!(fam("symbols"), None, "no error_msg → family stays NULL");

        // Idempotent: a third open is a no-op and does not error.
        drop(conn);
        let conn = open_db(dir.path()).unwrap();
        let still_null: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM tool_calls WHERE project_root IS NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(still_null, 0);
    }
}
