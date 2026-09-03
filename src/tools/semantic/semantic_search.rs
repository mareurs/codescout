//! SemanticSearch tool — vector + BM25 hybrid search.

use super::super::format::overflow_head;
use super::super::{optional_u64_param, parse_bool_param, Tool, ToolContext};
use serde_json::{json, Value};

/// Map a `RetrievalClient::from_env` error into what `SemanticSearch::call`
/// returns to its caller.
///
/// A `RecoverableError` (e.g. `build_embedder`'s `guard_sparse` conflict — a
/// local backend with the hybrid sparse leg still enabled) already carries
/// its own accurate message and hint; passed through unchanged here, rather
/// than relabelled below, is what keeps a config conflict from being
/// reported as a down service. Anything else (a genuine connect/build
/// failure) gets the "retrieval stack offline" framing, tailored to the
/// active backend — the lite stack has no local daemon to start; its
/// failure mode is an unreachable remote embedding endpoint, not a down
/// Qdrant/sparse/reranker service.
pub(crate) fn map_from_env_error(
    e: anyhow::Error,
    backend: crate::retrieval::code_store::VectorBackend,
) -> anyhow::Error {
    if e.downcast_ref::<crate::tools::RecoverableError>().is_some() {
        return e;
    }
    let hint = match backend {
        crate::retrieval::code_store::VectorBackend::SqliteVec => {
            "Lite stack: verify CODESCOUT_EMBEDDER_URL and EMBED_API_KEY — \
             the remote embedding endpoint is unreachable (no local daemon to start)."
        }
        crate::retrieval::code_store::VectorBackend::Qdrant => {
            "Run `./scripts/retrieval-stack.sh up` to start the retrieval stack."
        }
    };
    crate::tools::RecoverableError::with_hint(format!("retrieval stack offline: {e}"), hint).into()
}

/// Map a qdrant/search error string to an actionable recovery hint.
///
/// Patterns are checked in order of specificity: collection-missing first
/// (most common after first-time setup), then dim-mismatch (model/index
/// drift), then TEI errors (dense embedding service unhealthy), then
/// embedder-connect (client-side: dense/sparse endpoint unreachable via
/// `EmbedderHttp`), then the resolver-path embedder bucket (client-side:
/// `ollama:`/`openai:`/etc. via `RemoteEmbedder`, reached with no
/// `embedder_url` configured), then transport (stack went away), then a
/// generic fallback.
///
/// The embedder-connect bucket must precede transport: a client-side connect
/// failure's message carries reqwest's "Connection refused", which would
/// otherwise be misrouted to the qdrant-oriented transport hint.
/// The resolver-path bucket exists for the same reason: `RemoteEmbedder`'s
/// failure shapes ("embedding server unavailable after N attempts", "HTTP
/// {status} from embedding server", `crates/codescout-embed/src/remote.rs`)
/// match neither the TEI bucket's "tei status" wording nor the
/// `EmbedderHttp`-specific bucket's "embed connect failed"/"openai send"/
/// "embed sparse" wording, and would otherwise fall all the way to the
/// generic qdrant-oriented fallback — the identical misrouting class.
pub(crate) fn classify_search_error(err_str: &str, project_id: &str) -> String {
    // These two arms come FIRST, ahead of the collection bucket, for two reasons.
    //
    // (1) `dense openai status` matched NO bucket before this. The embedder arm below
    //     carries "openai send" but not "openai status", so every dense-embedder HTTP
    //     failure fell through to the generic fallback and sent operators to qdrant
    //     logs — the exact misrouting class this function's doc comment exists to
    //     prevent. (This comment used to add "the sparse path was fine: 'embed
    //     sparse status' contains 'embed sparse', which the embedder arm already
    //     matches". That was true of ONE of the two sparse producers and false of
    //     the other: `EmbedderHttp::embed` says `embed sparse …`, but
    //     `embed_one_batch` said `embed_batch sparse …`, which contains no `embed
    //     sparse` at all. Both now render SPARSE_MARKER, and the claim is enforced
    //     by a constant instead of asserted by a comment.)
    //
    // (2) That message now carries the embedder's RESPONSE BODY, which is arbitrary
    //     remote text. A body containing "not found" or "Collection" would hijack the
    //     collection bucket. Specificity first, per this function's own contract.
    //
    // (3) `STATUS_FAILED_MARKER` joins them for reason (2) exactly, and it was LIVE,
    //     not prospective. `RemoteEmbedder` reported a non-2xx as "HTTP {status} from
    //     embedding server: {body}", which matches only the `embedding server` arm far
    //     below the collection bucket — so an embedder 404 whose body read `model
    //     'coderank' not found` was already being reported as a missing Qdrant
    //     collection on the `ollama:`/`openai:` resolver path. The fix for root's own
    //     producer had never been extended to the crate's.
    //     docs/issues/archive/2026-08-30-crate-status-errors-hijack-the-qdrant-collection-bucket.md
    //
    //     Matching the imported constant rather than a literal is what survives T6:
    //     once the dense leg delegates to the crate, `dense openai status` has no
    //     producer left and this arm would otherwise go quietly dead.
    if err_str.contains("larger than the max context size")
        || err_str.contains("exceed_context_size")
        || err_str.contains("input is too large")
        || err_str.contains("physical batch size")
    {
        // FOUR patterns, because llama.cpp has (at least) TWO distinct payload-size
        // refusals and they differ in status, type AND remedy. Measured against the
        // running CodeRankEmbed server on 2026-08-26 by binary search on input size:
        //
        //   8000 chars  -> HTTP 200
        //   8250 chars  -> HTTP 500 {"message":"input is too large to process.
        //                  increase the physical batch size","type":"server_error"}
        //
        // The n_batch refusal above is what this stack actually emits. The n_ctx
        // refusal — HTTP 400, `exceed_context_size_error`, "input (N tokens) is larger
        // than the max context size (M tokens)" — is what a slot-constrained
        // configuration emits (`--ctx-size N --parallel P` gives N/P per slot) and is
        // the one the originating bug report documented.
        //
        // Matching only the reported wording is the mistake this comment exists to
        // prevent: the first version of this arm did exactly that, so it would not
        // have fired on the very stack it was written for. Green tests, dead branch.
        "The text could not be embedded because it exceeds a size limit on the \
         embedding server. This is a PAYLOAD SIZE problem, not an outage — the server \
         is up and retrying the same input will fail again. Shortening the input \
         always works. Raising a server limit sometimes does: `--ubatch-size` / \
         `--batch-size` lifts the physical-batch ceiling, but `--ctx-size` CANNOT lift \
         the per-request ceiling past the model's own n_ctx_train (2048 for \
         CodeRankEmbed), so no server configuration makes an over-long single input \
         embeddable. If you are seeing this while indexing, the affected chunks are \
         listed as `skipped` in the index status and the index is INCOMPLETE until \
         they embed."
            .to_string()
    } else if err_str.contains("dense openai status")
        || err_str.contains("sparse openai status")
        || err_str.contains(codescout_embed::STATUS_FAILED_MARKER)
        // The sparse leg joins them because it now carries the server's body too.
        // Until 2026-08-30 it used `error_for_status()`, which discards the body —
        // so it had nothing to hijack with, and nothing useful to say either. The
        // arm and the body-restoring fix have to land together: either alone is a
        // regression.
        || err_str.contains(crate::retrieval::embedder::SPARSE_STATUS_MARKER)
    {
        "The embedder returned an error status. This is an embedder problem, \
         NOT a Qdrant issue (don't check qdrant logs). The embedder's own response \
         body is included in the error above — read it first; it usually names the \
         cause exactly. Then `./scripts/retrieval-stack.sh ps`."
            .to_string()
    } else if err_str.contains("doesn't exist")
        || err_str.contains("not found")
        || err_str.contains("Collection")
    {
        format!(
            "Qdrant collection is missing for project `{project_id}`. \
             Populate it: `cargo run --release --bin sync_project -- . {project_id}`"
        )
    } else if err_str.contains("Vector dimension") || err_str.contains("expected dim") {
        "Embedding dim mismatch between index and configured model. \
         Drop the collection and re-index: \
         `curl -X DELETE $CODESCOUT_QDRANT_URL/../collections/code_chunks` \
         then `cargo run --release --bin sync_project -- . <project-id>`"
            .to_string()
    } else if err_str.contains("dense tei")
        || err_str.contains("sparse tei")
        || err_str.contains("tei status")
    {
        "Embedding service (TEI) is unhealthy. Most common cause: dense \
         or sparse TEI container OOM'd or returned non-200. \
         Check: `docker logs codescout-tei-dense` and \
         `docker logs codescout-tei-sparse`. \
         Restart: `./scripts/retrieval-stack.sh up`. \
         If persistent, inspect container memory limits + model file. \
         Workaround: fall back to `grep` / `symbols` for this query while TEI recovers."
            .to_string()
    // The marker comes from the crate that PRODUCES it, not from a literal here.
    // Once the dense leg moves to `codescout_embed::RemoteEmbedder` the producer is
    // across a crate boundary, where nothing would make a drifted literal and its
    // test fail together (`resume-embedding-transport-stages-1-3:ET-5`). Importing
    // the constant makes that impossible: change the wording and this follows in
    // the same compile.
    } else if err_str.contains(codescout_embed::CONNECT_FAILED_MARKER)
        || err_str.contains("openai send")
        || err_str.contains(crate::retrieval::embedder::SPARSE_MARKER)
    {
        // Client-side: the query could not be embedded because the embedder
        // endpoint is unreachable/erroring. Distinct from a Qdrant fault —
        // must NOT send the operator to qdrant logs (the 2026-07-13 bug).
        "Query could not be embedded — the dense/sparse embedder is unreachable \
         or returned an error. This is an embedder problem, NOT a Qdrant issue \
         (don't check qdrant logs). Verify CODESCOUT_EMBEDDER_URL / \
         CODESCOUT_SPARSE_EMBEDDER_URL point at the running embedder, then \
         `./scripts/retrieval-stack.sh ps`."
            .to_string()
    } else if err_str.contains("embedding server") {
        // The no-url resolver path (`ollama:`/`openai:`, wired in
        // src/retrieval/client.rs::build_embedder) constructs a
        // `RemoteEmbedder` whose failures carry this wording, not
        // "embed connect failed" — same client-side class as the bucket
        // above, distinct wording, so it needs its own match.
        "The configured embedding model's server is unreachable or returned an \
         error. This is an embedder problem, NOT a Qdrant issue (don't check \
         qdrant logs). Verify [embeddings].model (or CODESCOUT_EMBEDDER_MODEL) \
         names a reachable server — e.g. `ollama list` / `OLLAMA_HOST` for an \
         `ollama:` model, or your OpenAI-compatible endpoint's status for \
         `openai:`."
            .to_string()
    } else if err_str.contains("Connection refused")
        || err_str.contains("transport error")
        || err_str.contains("tonic")
    {
        "Stack went offline mid-query. \
         Restart with `./scripts/retrieval-stack.sh up` and retry."
            .to_string()
    } else {
        "Stack reachable but query failed. \
         Check `./scripts/retrieval-stack.sh ps` and qdrant logs \
         (`docker logs codescout-qdrant`)."
            .to_string()
    }
}

#[allow(dead_code)] // re-wire when the stack search gains file-diversity capping (tracker L-15)
/// Apply a per-file cap to a score-sorted list of search results. Iterates in
/// order and keeps at most `max_per_file` entries sharing the same `file_path`;
/// later duplicates are dropped. Passing 0 disables the cap (returns input).
pub(crate) fn apply_file_diversity_cap(
    results: Vec<crate::embed::schema::SearchResult>,
    max_per_file: usize,
) -> Vec<crate::embed::schema::SearchResult> {
    if max_per_file == 0 {
        return results;
    }
    let mut seen: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    results
        .into_iter()
        .filter(|r| {
            let count = seen.entry(r.file_path.clone()).or_insert(0);
            if *count < max_per_file {
                *count += 1;
                true
            } else {
                false
            }
        })
        .collect()
}

/// Classification of a linked worktree's index state, used by `SemanticSearch::call`'s
/// worktree branch to decide whether to query main+delta or return the
/// not-yet-indexed hint. See `classify_worktree_index_state` for the decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WorktreeIndexState {
    /// No sidecar, or an empty dirty set with an empty delta: the worktree sync
    /// has simply never run. `symbols`/`grep`/`references` are still correct;
    /// only the vector index is missing.
    NotIndexed,
    /// A non-empty dirty set: the normal, healthy in-progress state. Main
    /// excludes exactly these paths; the delta supplies them.
    Healthy,
    /// HUMAN RULING: an empty dirty set but a non-empty delta. Impossible in a
    /// healthy state -- a delta only exists because something was dirty. Arises
    /// when a plain `sync_project` rewrites the sidecar with `dirty_paths: []`
    /// after a worktree sync had recorded a real set (`write_index_state`
    /// delegates to `write_index_state_with_dirty(root, &[])`). Must NOT be read
    /// as "nothing is dirty": that would query main with `exclude_paths` empty
    /// and let it silently serve every stale chunk.
    Suspect,
}

/// Pure decision at the core of the worktree branch -- see [`WorktreeIndexState`]
/// for what each variant means and why. Kept free of any store/filesystem access
/// so all meaningful combinations are testable without a live index.
/// `delta_has_chunks` only matters when `dirty_paths_empty`; callers may skip
/// that remote check otherwise (the common, non-empty case).
pub(crate) fn classify_worktree_index_state(
    sidecar_present: bool,
    dirty_paths_empty: bool,
    delta_has_chunks: bool,
) -> WorktreeIndexState {
    if !sidecar_present {
        return WorktreeIndexState::NotIndexed;
    }
    if !dirty_paths_empty {
        return WorktreeIndexState::Healthy;
    }
    if delta_has_chunks {
        WorktreeIndexState::Suspect
    } else {
        WorktreeIndexState::NotIndexed
    }
}

/// Hint returned when a linked worktree has no usable index yet
/// (`WorktreeIndexState::NotIndexed`). Names the resolved delta project id and
/// both exits available while the vector index catches up: `index(action="build")`
/// to fix it, or the filesystem-backed tools that are already correct here.
pub(crate) fn worktree_no_index_hint(delta_id: &str) -> String {
    format!(
        "No index for worktree project `{delta_id}`. A worktree's files differ from the main \
         checkout, so main's vectors are not served for changed files. Run index(action=\"build\") \
         here to index them — it only embeds what differs. `symbols`, `grep` and `references` \
         are computed from the filesystem and are already correct in this worktree."
    )
}

/// `Some(hint)` when the delta is not usable yet, `None` once it is. Split from
/// [`worktree_no_index_hint`] so both directions are testable without a live
/// store: a hint that fires unconditionally would pass a test that only checks
/// the positive direction while telling the caller nothing.
pub(crate) fn worktree_hint_for(delta_present: bool, delta_id: &str) -> Option<String> {
    if delta_present {
        None
    } else {
        Some(worktree_no_index_hint(delta_id))
    }
}

/// State-condition note for `WorktreeIndexState::Suspect` (see the human ruling on
/// that variant above). Surfaced in the response payload, never as a
/// `RecoverableError` -- this is evidence about the index's own bookkeeping, not
/// something the caller did wrong.
pub(crate) fn worktree_suspect_note(delta_id: &str) -> String {
    format!(
        "Worktree dirty-path tracking looks inconsistent for `{delta_id}`: the delta index has \
         chunks but no dirty paths are recorded, which should never happen in a healthy state (a \
         delta only exists because something was dirty). Main was queried with no path exclusions \
         below and may be serving stale chunks for files this worktree has changed. Re-run \
         index(action=\"build\") here to repair the dirty-path record."
    )
}

/// `true` when `main_last_indexed_at` parses to a later instant than
/// `worktree_last_indexed_at`. `false` on any parse failure or absence --
/// silence rather than a false claim of freshness, matching the plan's rule that
/// undetectable drift is reported as silence, not reassurance.
pub(crate) fn main_reindexed_after_worktree(
    main_last_indexed_at: &str,
    worktree_last_indexed_at: &str,
) -> bool {
    let parse = |s: &str| chrono::DateTime::parse_from_rfc3339(s).ok();
    match (parse(main_last_indexed_at), parse(worktree_last_indexed_at)) {
        (Some(m), Some(w)) => m > w,
        _ => false,
    }
}

/// Note attached when [`main_reindexed_after_worktree`] fires. Also mentions --
/// cheaply, since this string is already being constructed -- the residual
/// double-serve window a partially-failed sync can still leave.
///
/// That window is NOT the one this note used to describe. I3 moved
/// `sync_worktree`'s sidecar write to *before* the delta upserts and made it a
/// hard error, so a failed sync can no longer leave a non-empty but incomplete
/// `dirty_paths` -- the recorded set is now always the complete one computed for
/// that run, or the sync refuses to proceed at all. What remains is the
/// opposite ordering: a path that went dirty -> clean since the last sync is
/// absent from the new sidecar (so main serves it, correctly), while its old
/// delta chunks survive until the prune, which an early return between the
/// upserts and that prune skips. Both copies then answer.
///
/// Still not checked here, for the same reason as before: detecting it needs the
/// delta's full chunk list, too expensive to scroll on a read path. It is named
/// rather than checked -- and it now requires the user to have reverted a file
/// to main's exact bytes, which is strictly rarer than the edit case I3 closed.
pub(crate) fn worktree_main_ahead_note() -> &'static str {
    "Note: the main checkout was re-indexed after this worktree's delta was built, so results \
     for unchanged files may reflect main's newer content. Re-run index(action=\"build\") here \
     to refresh. (A sync that failed part-way can also leave stale delta chunks for a file you \
     have since reverted to match main: main serves it and the delta's leftover copy does too, \
     so the same path can appear twice.)"
}

/// `(main_exclude_paths, delta_exclude_paths)` for the two-source worktree query.
/// Main must exclude the worktree's dirty paths -- it would otherwise serve stale
/// content for files the worktree changed; the delta must exclude nothing -- it
/// holds exactly those dirty files and nothing else. Extracted so the wiring is
/// mutation-verifiable without a live store: swapping the two would starve the
/// delta query of the very paths `sync_worktree` upserted, while main keeps
/// serving what it should have excluded.
pub(crate) fn worktree_query_exclusions(dirty_paths: &[String]) -> (Vec<String>, Vec<String>) {
    (dirty_paths.to_vec(), Vec::new())
}

/// State-condition note for `main_project_id` having no indexed chunks at
/// all (pinned requirement 6's "main never indexed" arm). Surfaced in the
/// response payload, never as a `RecoverableError` -- the query still ran
/// and the delta's results (if any) are real; this only explains why main
/// contributed nothing.
pub(crate) fn worktree_main_never_indexed_note(main_id: &str) -> String {
    format!(
        "Main project `{main_id}` has no indexed chunks yet. Results below come only \
         from this worktree's delta -- main's own code is not represented. Run \
         index(action=\"build\") in the main checkout to populate it."
    )
}

/// Every decision the worktree branch needs once it has already committed to
/// proceeding (`classification != NotIndexed`): which paths each of the two
/// queries excludes, and which state-condition notes (if any) the response
/// carries. Fields, not statements -- a caller wires each straight into the
/// two `SearchOpts` and the response `Value` with nothing left to swap by
/// accident; see `worktree_search_opts` and `apply_worktree_plan_notes`.
///
/// Deliberately takes no store/client reference -- `delta_has_chunks` (via
/// `classification`, already decided) and `main_has_chunks` are supplied by
/// the caller, which is the only thing that talks to the store. The one
/// filesystem read this function itself performs (`read_index_state` on
/// `main_repo`, for the drift note) is not a store call, so every branch is
/// fixturable with a plain `tempfile::tempdir()` and no live index -- see
/// `worktree_query_plan_tests`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorktreeQueryPlan {
    pub main_id: String,
    pub delta_id: String,
    pub main_exclude: Vec<String>,
    pub delta_exclude: Vec<String>,
    /// Is main worth querying at all?
    ///
    /// `false` when main holds no indexed chunks: the query could then only
    /// ever return an empty page, and it would ship the entire
    /// `main_exclude` list to the store to find that out. On the state this
    /// design models most often -- main never indexed, so `sync_worktree`
    /// marks *every* file dirty -- that list is the whole repository. A
    /// field rather than an `if` at the call site so the skip is pinned by
    /// `worktree_query_plan`'s own tests.
    pub query_main: bool,
    pub warning: Option<String>,
    pub main_never_indexed_note: Option<String>,
    pub drift_note: Option<String>,
}

pub(crate) fn worktree_query_plan(
    main_repo: &std::path::Path,
    main_id: &str,
    delta_id: &str,
    classification: WorktreeIndexState,
    dirty_paths: &[String],
    worktree_last_indexed_at: &str,
    main_has_chunks: bool,
) -> WorktreeQueryPlan {
    let (main_exclude, delta_exclude) = worktree_query_exclusions(dirty_paths);

    let warning =
        (classification == WorktreeIndexState::Suspect).then(|| worktree_suspect_note(delta_id));

    let main_never_indexed_note =
        (!main_has_chunks).then(|| worktree_main_never_indexed_note(main_id));

    let drift_note = crate::retrieval::index_state::read_index_state(main_repo)
        .filter(|main_state| {
            main_reindexed_after_worktree(&main_state.last_indexed_at, worktree_last_indexed_at)
        })
        .map(|_| worktree_main_ahead_note().to_string());

    WorktreeQueryPlan {
        main_id: main_id.to_string(),
        delta_id: delta_id.to_string(),
        main_exclude,
        delta_exclude,
        query_main: main_has_chunks,
        warning,
        main_never_indexed_note,
        drift_note,
    }
}

/// The two `SearchOpts` the worktree branch queries with, built from a
/// shared base plus the plan's exclusions. This is the exact assignment
/// requirement 2's hazard lives in -- "does main get the dirty list, or does
/// the delta" -- turned into a value a test can assert on, rather than a
/// pair of statements only an integration test could observe.
pub(crate) fn worktree_search_opts(
    base: &crate::retrieval::search::SearchOpts,
    plan: &WorktreeQueryPlan,
) -> (
    crate::retrieval::search::SearchOpts,
    crate::retrieval::search::SearchOpts,
) {
    let mut main_opts = base.clone();
    main_opts.exclude_paths = plan.main_exclude.clone();
    let mut delta_opts = base.clone();
    delta_opts.exclude_paths = plan.delta_exclude.clone();
    (main_opts, delta_opts)
}

/// Copies whichever of the plan's notes are present onto the response
/// `Value`. The other half of the same "value, not statements" treatment as
/// `worktree_search_opts`: deleting the human-ruled `Suspect` warning, or
/// the drift note, now breaks this function's own test rather than only
/// being observable end-to-end.
pub(crate) fn apply_worktree_plan_notes(out: &mut Value, plan: &WorktreeQueryPlan) {
    if let Some(warning) = &plan.warning {
        out["worktree_state_warning"] = serde_json::json!(warning);
    }
    if let Some(note) = &plan.main_never_indexed_note {
        out["main_never_indexed_note"] = serde_json::json!(note);
    }
    if let Some(note) = &plan.drift_note {
        out["drift_note"] = serde_json::json!(note);
    }
}

pub struct SemanticSearch;

#[async_trait::async_trait]
impl Tool for SemanticSearch {
    fn name(&self) -> &str {
        "semantic_search"
    }

    /// `openWorld` stays at its `true` default: `RequiresEmbeddings`, and the embedder
    /// factory resolves `openai:` / a bare `url` to an arbitrary host.
    fn annotations(&self) -> Option<rmcp::model::ToolAnnotations> {
        crate::tools::annot::read_only_open()
    }

    fn description(&self) -> &str {
        "Find code by natural language description or code snippet. \
             Returns ranked chunks with file path and line range."
    }

    fn relevant_guide_topic(&self, _result: &Value) -> Option<&str> {
        Some("progressive-disclosure")
    }

    fn long_docs(&self) -> Option<&str> {
        Some(
                    "## When to use\n\
                 \n\
                 Use `semantic_search` when you know the *concept* but not the symbol name.\n\
                 Examples: \"retry logic\", \"parse JWT token\", \"database connection pool\".\n\
                 For known symbol names, prefer `symbols` (faster, exact).\n\
                 \n\
                 ## Prerequisites\n\
                 \n\
                 The project index must be built: run `index(action='build')` first.\n\
                 Check status with `index(action='status')`.\n\
                 \n\
                 ## Key parameters\n\
                 \n\
                 - `query`: natural language or a code snippet.\n\
                 - `limit`: number of results (default 10). Raise to 20-30 for broad concepts.\n\
                 - `scope`: `\"project\"` (default), `\"libraries\"`, `\"all\"`, or `\"lib:<name>\"`.\n\
                 - `include_memories=true`: also search semantic memories.\n\
                 - `project_id`: filter to a specific workspace sub-project.\n\
                 - `mode`: `\"code\"` (default) excludes markdown chunks — best for finding implementations.\n\
                           `\"full\"` includes all indexed content.\n\
                 \n\
                 ## Output\n\
                 \n\
                 Each result has `file_path`, `start_line`, `end_line`, and `content`. `source` is\n\
                 present only when it is not `\"project\"` (e.g. a memory hit). There is no `score`\n\
                 field. Use `symbols` or `read_file(start_line=N, end_line=M)` for more context\n\
                 around the chunk.\n\
                 \n\
                 ## Worktree response fields (linked git worktree only)\n\
                 \n\
                 Called from inside a linked git worktree with no explicit `project_id`, the query\n\
                 ranks main's index and a per-worktree delta as one list, and the response may\n\
                 carry:\n\
                 \n\
                 - `drift_note`: main was reindexed after this worktree's own delta was built, so\n\
                   unchanged-file results may reflect main's newer content. Re-run\n\
                   `index(action=\"build\")` here.\n\
                 - `worktree_state_warning`: the delta has chunks but no recorded dirty paths, an\n\
                   inconsistent state — main was queried with no path exclusions and may serve\n\
                   stale chunks. Re-run `index(action=\"build\")` here to repair the record.\n\
                 - `main_never_indexed_note`: main has no indexed chunks at all, so every result\n\
                   below comes only from this worktree's own delta. Run `index(action=\"build\")`\n\
                   in the main checkout.\n\
                 \n\
                 Each is an informational string, present only when its condition applies. None is\n\
                 an error — the query still ran.\n\
                 \n\
                 ## Tips\n\
                 \n\
                 - Short, specific queries beat long prose.\n\
                 - Results are ranked by relevance; re-query with a different angle if the top\n\
                   hits miss.",
                )
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["query"],
            "properties": {
                "query": { "type": "string", "description": "Natural language or code snippet to search for" },
                "limit": { "type": "integer", "default": 10, "description": "Max results to return (default 10)." },
                "detail_level": { "type": "string", "description": "'full' for complete chunks (default: compact)" },
                "offset": { "type": "integer", "description": "Pagination offset" },
                "scope": { "type": "string", "description": "'project' (default), 'libraries', 'all', or 'lib:<name>'" },
                "include_memories": { "type": "boolean", "default": false, "description": "Also search semantic memories." },
                "project_id": { "type": "string", "description": "Filter to a workspace project ID." },
                "mode": { "type": "string", "enum": ["code", "full"], "default": "code", "description": "'code' (default) excludes markdown chunks — best for finding implementations. 'full' includes all indexed content (code + docs)." }
            }
        })
    }
    async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<Value> {
        use crate::tools::output::OutputGuard;

        let query = crate::tools::require_str_param(&input, "query")?;
        if query.trim().is_empty() {
            return Err(crate::tools::RecoverableError::with_hint(
                    "'query' must be a non-empty string",
                    "Provide a natural-language phrase or code snippet describing what to search for (e.g. query=\"how does the embedding cache evict entries\").",
                )
                .into());
        }
        let limit = optional_u64_param(&input, "limit").unwrap_or(10) as usize;
        let _guard = OutputGuard::from_input(&input);

        // Phase 7 (narrow): stack is the only retrieval backend for code search.
        // The legacy sqlite-vec + tantivy path is gone. Memory storage and recall
        // still live on the legacy index — see
        // docs/trackers/2026-05-07-legacy-retrieval-removal.md (L-01..L-11).
        if parse_bool_param(&input["include_memories"]) {
            return Err(crate::tools::RecoverableError::with_hint(
                "include_memories is not supported by the Qdrant retrieval stack",
                "Use `memory(action=\"recall\", query=...)` for semantic memory search.",
            )
            .into());
        }
        if input
            .get("scope")
            .and_then(|v| v.as_str())
            .map(|s| s.starts_with("lib:"))
            .unwrap_or(false)
        {
            return Err(crate::tools::RecoverableError::with_hint(
                "library scope is not yet supported by the Qdrant retrieval stack",
                "Track L-12 in docs/trackers/2026-05-07-legacy-retrieval-removal.md; \
             use `symbols(name=...)` against the library project as a workaround.",
            )
            .into());
        }

        if let Some(p) = ctx.progress.as_ref() {
            p.report_text("loading embedding model").await;
        }
        let project_id = if let Some(pid) = input.get("project_id").and_then(|v| v.as_str()) {
            pid.to_string()
        } else {
            ctx.agent
                    .with_project_at(ctx.workspace_override.as_deref(), |p| {
                        Ok(p.config.project.name.clone())
                    })
                    .await
                    .map_err(|_| {
                        crate::tools::RecoverableError::with_hint(
                            "No active project. Use workspace(action='activate') first.",
                            "Call workspace(action='activate', path=\"/path/to/project\") to set the active project.",
                        )
                    })?
        };
        let root = ctx
            .agent
            .project_root_for(ctx.workspace_override.as_deref())
            .await;
        let client = crate::retrieval::client::RetrievalClient::from_env(root.as_deref())
            .await
            .map_err(|e| {
                map_from_env_error(e, crate::retrieval::code_store::VectorBackend::resolve())
            })?;
        let opts = crate::retrieval::search::SearchOpts {
            limit,
            overfetch: limit * 2,
            rerank: true,
            exclude_languages: match input.get("mode").and_then(|v| v.as_str()).unwrap_or("code") {
                "full" => Vec::new(),
                _ => vec!["markdown".to_string()],
            },
            exclude_paths: Vec::new(),
        };
        if let Some(p) = ctx.progress.as_ref() {
            p.report_text("searching").await;
        }

        // A linked worktree's own project id never gets a `sync_project` collection
        // -- `index(action="build")` routes a worktree root to `sync_worktree`
        // instead (Task 6), which populates `<main>@<worktree-name>` (git's own
        // worktree name, not the checkout directory's basename -- see
        // `retrieval::sync::worktree_key`) and records the worktree's dirty paths
        // in its own `.codescout/index-state.json`. So when `root` is a worktree,
        // rank main (with the dirty paths excluded) and that delta project as ONE
        // list -- never the worktree's own `project_id`, which has nothing indexed
        // under it at all. The union happens at the store, not here; see the call
        // to `search_code_overlay` below.
        //
        // Skipped entirely when the caller passed an explicit `project_id`: that
        // names a specific collection to search, and redirecting it to main+delta
        // would silently ignore the caller's choice.
        let explicit_project_id = input.get("project_id").and_then(|v| v.as_str()).is_some();
        if !explicit_project_id {
            let worktree_main_repo = root
                .as_deref()
                .and_then(crate::prompts::detect_worktree_info)
                .and_then(|info| info.main_repo);
            if let Some(main_repo) = worktree_main_repo {
                let worktree_root = root
                    .clone()
                    .expect("main_repo only resolves from a Some(root)");
                // Single shared derivation -- see `worktree_ids`'s doc comment.
                // Task 6's producer side goes through the exact same function.
                let (main_project_id, delta_id) =
                    crate::retrieval::sync::worktree_ids(&main_repo, &worktree_root);
                let collection = client.config.collection("code_chunks");

                let state = crate::retrieval::index_state::read_index_state(&worktree_root);
                let dirty_paths_empty = state
                    .as_ref()
                    .map(|s| s.dirty_paths.is_empty())
                    .unwrap_or(true);
                // Only worth the remote check when dirty_paths is empty -- see
                // classify_worktree_index_state: it's irrelevant otherwise.
                let delta_has_chunks = if state.is_some() && dirty_paths_empty {
                    client
                        .project_has_chunks(&collection, &delta_id)
                        .await
                        .map_err(|e| {
                            let hint = classify_search_error(&e.to_string(), &delta_id);
                            crate::tools::RecoverableError::with_hint(
                                format!("stack search failed: {e}"),
                                hint,
                            )
                        })?
                } else {
                    false
                };
                let classification = classify_worktree_index_state(
                    state.is_some(),
                    dirty_paths_empty,
                    delta_has_chunks,
                );

                if let Some(hint) =
                    worktree_hint_for(classification != WorktreeIndexState::NotIndexed, &delta_id)
                {
                    let mut out = search_response(&[], limit);
                    out["hint"] = serde_json::json!(hint);
                    return Ok(out);
                }

                let state =
                    state.expect("classification other than NotIndexed implies a parsed sidecar");

                // Requirement 6's "main never indexed" arm: one additional
                // remote call, only in this already-worktree-specific branch
                // that is about to query main anyway -- never unconditional on
                // every search.
                let main_has_chunks = client
                    .project_has_chunks(&collection, &main_project_id)
                    .await
                    .map_err(|e| {
                        let hint = classify_search_error(&e.to_string(), &main_project_id);
                        crate::tools::RecoverableError::with_hint(
                            format!("stack search failed: {e}"),
                            hint,
                        )
                    })?;

                let plan = worktree_query_plan(
                    &main_repo,
                    &main_project_id,
                    &delta_id,
                    classification,
                    &state.dirty_paths,
                    &state.last_indexed_at,
                    main_has_chunks,
                );
                let (main_opts, delta_opts) = worktree_search_opts(&opts, &plan);

                // ONE ranking over main + delta, not two lists merged by score.
                // Scores are only comparable across queries on some backends,
                // and the default one (Qdrant, hybrid RRF) is not among them --
                // `CodeVectorStore::query_overlay` carries the full argument.
                // The tool layer must not learn which backend is underneath, so
                // the union is expressed at the store and this stays one call.
                //
                // `plan.query_main` is false exactly when main holds no indexed
                // chunks; the query would then return an empty page after
                // shipping the whole exclusion list to find that out, so the
                // delta is queried alone. `plan.main_never_indexed_note` still
                // explains the empty contribution in the response payload.
                let hits = if plan.query_main {
                    client
                        .search_code_overlay(&main_project_id, &delta_id, query, main_opts)
                        .await
                        .map_err(|e| {
                            let hint = classify_search_error(&e.to_string(), &main_project_id);
                            crate::tools::RecoverableError::with_hint(
                                format!("stack search failed: {e}"),
                                hint,
                            )
                        })?
                } else {
                    client
                        .search_code(&delta_id, query, delta_opts)
                        .await
                        .map_err(|e| {
                            let hint = classify_search_error(&e.to_string(), &delta_id);
                            crate::tools::RecoverableError::with_hint(
                                format!("stack search failed: {e}"),
                                hint,
                            )
                        })?
                };

                let mut out = search_response(&hits, limit);
                apply_worktree_plan_notes(&mut out, &plan);
                if let Some(note) = index_skip_note(root.as_deref()) {
                    out["index_degraded_note"] = serde_json::json!(note);
                }
                return Ok(out);
            }
        }

        let hits = client
            .search_code(&project_id, query, opts)
            .await
            .map_err(|e| {
                let hint = classify_search_error(&e.to_string(), &project_id);
                crate::tools::RecoverableError::with_hint(format!("stack search failed: {e}"), hint)
            })?;
        let mut out = search_response(&hits, limit);
        // Both return paths stamp it, not just this one: the worktree branch above
        // returns its own response and would otherwise be the single search shape
        // that never reports an incomplete index.
        if let Some(note) = index_skip_note(root.as_deref()) {
            out["index_degraded_note"] = serde_json::json!(note);
        }
        Ok(out)
    }

    fn format_compact(&self, result: &Value) -> Option<String> {
        Some(format_semantic_search(result))
    }

    fn availability(&self, _caps: &crate::tools::ToolCapabilities) -> crate::tools::Availability {
        crate::tools::Availability::RequiresEmbeddings
    }
}

pub(crate) fn format_search_result_item(
    file_path: &str,
    start_line: usize,
    end_line: usize,
    source: &str,
    content: String,
) -> Value {
    // Field order: identity → location → metadata → content (bulk payload last)
    let mut map = serde_json::Map::new();
    map.insert("file_path".into(), json!(file_path));
    map.insert("start_line".into(), json!(start_line));
    map.insert("end_line".into(), json!(end_line));
    if source != "project" {
        map.insert("source".into(), json!(source));
    }
    map.insert("content".into(), json!(content));
    Value::Object(map)
}

/// Shape the final `{results, total, truncated, truncated_hint?}` envelope
/// from a score-ordered hit list. Used by both the worktree branch (on the
/// merged hits) and the plain path (on `search_code`'s hits directly) so a
/// field added to one response shape can't land in only one -- the exact bug
/// class that hit `format_semantic_search` one function over.
fn search_response(hits: &[crate::retrieval::search::Hit], limit: usize) -> Value {
    let result_items: Vec<Value> = hits
        .iter()
        .map(|h| {
            format_search_result_item(
                &h.file_path,
                h.start_line as usize,
                h.end_line as usize,
                "stack",
                h.content.clone(),
            )
        })
        .collect();
    let count = result_items.len();
    // KNN over a large index: a full page almost always means more relevant
    // chunks exist. Signal it rather than let `total` read as complete.
    let truncated = count >= limit;
    let mut out =
        serde_json::json!({ "results": result_items, "total": count, "truncated": truncated });
    if truncated {
        out["truncated_hint"] = serde_json::json!(
            "results capped at `limit`; raise `limit` for more (ranked by relevance, so later results matter less)"
        );
    }
    out
}

/// The durable code-index skip marker, rendered as a one-line note, or `None`
/// when the last sync was clean.
///
/// `index(action="status")` has surfaced `last_sync_skipped` since
/// `docs/issues/archive/2026-08-26-index-status-claims-complete-without-checking-coverage.md`,
/// and `doc(action="find")` surfaces its catalog twin (`catalog_degraded`)
/// on every call. `semantic_search` — the tool that actually CONSUMES this index
/// — surfaced neither, so the one health signal that existed sat where nobody
/// reads it: a caller learned their index was incomplete only if they thought to
/// run a status command, which is exactly what you do not think to do when the
/// search returned plausible-looking results.
///
/// Returns a STRING, deliberately, so it drops into `format_semantic_search`'s
/// `state_lines` list and inherits its head placement. The sample stays in
/// `index(action="status")`, the surface built to carry it.
///
/// Presence means a problem; absent when clean, never `count: 0`. Same
/// convention as `index(status)`'s `last_sync_skipped` and `model_mismatch`.
///
/// BUG docs/issues/archive/2026-08-26-catalog-reindex-fails-closed-on-embedding-error.md
fn index_skip_note(root: Option<&std::path::Path>) -> Option<String> {
    let st = crate::retrieval::index_state::read_index_state(root?)?;
    if st.last_sync_skipped_count == 0 {
        return None;
    }
    Some(format!(
        "the last index sync SKIPPED {} file(s), so these results are computed over an \
         incomplete index — an absent match here does not mean the code lacks it. \
         `index(action=\"status\")` lists them; `index(action=\"build\")` retries.",
        st.last_sync_skipped_count
    ))
}

pub(crate) fn format_semantic_search(val: &Value) -> String {
    let results = match val["results"].as_array() {
        Some(arr) => arr,
        None => return String::new(),
    };
    let total = val["total"].as_u64().unwrap_or(results.len() as u64);

    // State-condition fields (worktree not-indexed hint, drift/suspect/main-
    // never-indexed notes, the pre-existing truncated_hint) are built FIRST
    // and placed ahead of everything else, for two reasons:
    //
    // 1. They must reach the `results: []` case too. The worktree
    //    "NotIndexed" response is the ONE place `hint` is ever set, and it
    //    always has zero results -- built after the early return below (as a
    //    prior version of this function did), it can never be reached: the
    //    exact "0 results, no hint" shape this whole task exists to
    //    eliminate, reproduced one layer up.
    // 2. `truncate_compact` (src/tools/core/types.rs) cuts everything AFTER
    //    its budget from the string's TAIL, keeping only the prefix -- and
    //    `call_content`'s overflow path uses this function's OUTPUT,
    //    verbatim, as the only summary the caller ever sees. Placed ahead of
    //    the result rows, these fields survive that cut regardless of how
    //    many/how large the rows are; placed after them, a summary that
    //    merely exceeds the hard cap silently drops every one of them along
    //    with the rows -- see `format_semantic_search_keeps_state_fields_above_the_truncation_cap`.
    let mut state_lines = String::new();
    for (field, label) in [
        ("hint", "Hint"),
        ("worktree_state_warning", "Warning"),
        ("index_degraded_note", "Warning"),
        ("main_never_indexed_note", "Note"),
        ("drift_note", "Note"),
        ("truncated_hint", "Note"),
    ] {
        if let Some(text) = val[field].as_str() {
            state_lines.push('\n');
            state_lines.push_str(&format!("\n  {label}: {text}"));
        }
    }

    if results.is_empty() {
        let mut out = "0 results".to_string();
        out.push_str(&state_lines);
        return out;
    }

    let result_word = if total == 1 { "result" } else { "results" };
    let mut out = format!("{total} {result_word}\n");
    // The overflow note joins the state fields at the head for the same reason they are
    // here: `truncate_compact` keeps only the prefix. Until 2026-08-16 this function
    // protected `hint`/`drift_note`/`truncated_hint` from that cut while still appending
    // `format_overflow` after the rows — so the reference implementation for head
    // placement was itself only half-protected.
    out.push_str(&overflow_head(val));
    out.push_str(&state_lines);

    // Build rows: (location, preview)
    let rows: Vec<(String, String)> = results
        .iter()
        .map(|r| {
            let file = r["file_path"].as_str().unwrap_or("?");
            let start = r["start_line"].as_u64().unwrap_or(0);
            let end = r["end_line"].as_u64().unwrap_or(0);
            let location = if start > 0 && end > 0 && start != end {
                format!("{file}:{start}-{end}")
            } else if start > 0 {
                format!("{file}:{start}")
            } else {
                file.to_string()
            };

            // Content preview: first line, truncated to ~50 chars
            let content = r["content"].as_str().unwrap_or("");
            let first_line = content.lines().next().unwrap_or("").trim();
            let preview = if first_line.chars().count() > 50 {
                let mut end = 47.min(first_line.len());
                while !first_line.is_char_boundary(end) {
                    end -= 1;
                }
                format!("{}...", &first_line[..end])
            } else {
                first_line.to_string()
            };

            (location, preview)
        })
        .collect();

    let max_loc_len = rows.iter().map(|(l, _)| l.len()).max().unwrap_or(0);

    for (location, preview) in &rows {
        out.push('\n');
        out.push_str("  ");
        out.push_str(location);
        if !preview.is_empty() {
            let loc_pad = max_loc_len - location.len();
            for _ in 0..loc_pad {
                out.push(' ');
            }
            out.push_str("  ");
            out.push_str(preview);
        }
    }

    // Git sync info (informational only — does not affect result quality)
    if val["git_sync"]["status"].as_str() == Some("behind") {
        out.push('\n');
        if let Some(n) = val["git_sync"]["behind_commits"].as_u64() {
            out.push_str(&format!(
                "\n  {n} commits not yet indexed (results still valid — run index(action='build') to include new code)"
            ));
        }
    }

    out
}

#[cfg(test)]
mod classify_search_error_tests {
    use super::classify_search_error;

    #[test]
    fn missing_collection_routes_to_sync_project_hint() {
        let err = "hybrid_query: Collection `code_chunks` doesn't exist!";
        let hint = classify_search_error(err, "codescout");
        assert!(hint.contains("sync_project"), "hint: {hint}");
        assert!(hint.contains("codescout"), "hint must name project: {hint}");
    }

    #[test]
    fn dim_mismatch_routes_to_drop_and_reindex_hint() {
        let err = "upsert_points: Vector dimension error: expected dim: 512, got 768";
        let hint = classify_search_error(err, "codescout");
        assert!(hint.contains("dim mismatch"), "hint: {hint}");
        assert!(
            hint.contains("DELETE"),
            "hint must give drop command: {hint}"
        );
        assert!(
            hint.contains("sync_project"),
            "hint must follow with reindex: {hint}"
        );
    }

    #[test]
    fn transport_error_routes_to_restart_hint() {
        let err = "tonic::transport::Error: Connection refused (os error 111)";
        let hint = classify_search_error(err, "codescout");
        assert!(hint.contains("offline"), "hint: {hint}");
        assert!(
            hint.contains("retrieval-stack.sh up"),
            "hint must restart: {hint}"
        );
    }

    #[test]
    fn unknown_error_routes_to_diagnostic_hint() {
        let err = "some weird unrelated failure";
        let hint = classify_search_error(err, "codescout");
        assert!(hint.contains("ps"), "fallback must check stack: {hint}");
        assert!(
            hint.contains("docker logs"),
            "fallback must point at logs: {hint}"
        );
    }

    #[test]
    fn collection_missing_takes_priority_over_transport() {
        // If both signals present, collection-missing wins (more actionable).
        let err = "Collection `code_chunks` not found via tonic transport";
        let hint = classify_search_error(err, "codescout");
        assert!(
            hint.contains("sync_project"),
            "specificity ordering: {hint}"
        );
    }

    #[test]
    fn tei_status_routes_to_embedding_service_hint() {
        // I-7: 45 of 53 'dense tei status' errors in the 2026-05-27 usage
        // analysis fell into the generic bucket because TEI didn't have its
        // own classification. New TEI bucket gives a concrete recovery path.
        let err = "stack search failed: dense tei status: HTTP 503";
        let hint = classify_search_error(err, "codescout");
        assert!(
            hint.contains("TEI") || hint.contains("tei"),
            "hint must name TEI explicitly: {hint}"
        );
        assert!(
            hint.contains("docker logs"),
            "hint must point at container logs: {hint}"
        );
        assert!(
            hint.contains("retrieval-stack.sh up"),
            "hint must give restart command: {hint}"
        );
    }

    #[test]
    fn tei_bucket_takes_priority_over_generic_fallback() {
        // A bare TEI error string that doesn't also match collection/dim/
        // transport should resolve to the new TEI hint, not the generic one.
        let err = "search_code: dense tei status (HTTP 504, upstream timeout)";
        let hint = classify_search_error(err, "codescout");
        assert!(
            !hint.contains("Stack reachable but query failed"),
            "must not hit generic fallback: {hint}"
        );
    }

    #[test]
    fn dense_connect_failure_routes_to_embedder_hint_not_qdrant() {
        // Regression: a client-side connect failure (wrong CODESCOUT_EMBEDDER_URL
        // or embedder down) used to fall through to the generic "check qdrant
        // logs" fallback, sending operators to debug a healthy stack.
        let err = "stack search failed: dense embed connect failed: \
                   http://127.0.0.1:8081/v1/embeddings — the dense embedder is unreachable";
        let hint = classify_search_error(err, "codescout");
        assert!(
            hint.contains("CODESCOUT_EMBEDDER_URL"),
            "hint must point at the embedder URL env var: {hint}"
        );
        // The regression marker is the generic fallback's qdrant directive; our
        // hint may *mention* qdrant (to steer away from it), so match the precise
        // fallback text rather than the bare substring "qdrant logs".
        assert!(
            !hint.contains("docker logs codescout-qdrant")
                && !hint.contains("Stack reachable but query failed"),
            "must NOT route a client-side connect failure to the qdrant-logs fallback: {hint}"
        );
    }

    /// ET-5: routes the crate's **real** error, not a hand-written imitation.
    ///
    /// The test above uses a literal, which is right for pinning operator-facing
    /// wording and structurally cannot catch what this one exists for — the
    /// producer moving into `codescout-embed` and drifting away from what this
    /// classifier matches. Rendering the actual `EmbedError::Connect` is what
    /// couples the two sides: change the crate's `Display` and this fails here,
    /// in the consumer, which is precisely what "nothing makes the two tests fail
    /// together" described.
    ///
    /// Measured 2026-08-30, before `EmbedError` existed: a connect failure from
    /// `RemoteEmbedder` rendered as `error sending request for url (...)` and
    /// matched NO bucket in this function, falling through to the generic
    /// Qdrant-oriented fallback. That was live on the `ollama:`/`openai:`
    /// resolver path, not merely a risk the swap would have introduced.
    #[test]
    fn the_crates_own_connect_error_routes_where_roots_does() {
        let produced = codescout_embed::EmbedError::Connect {
            url: "http://127.0.0.1:48081/v1/embeddings".into(),
            detail: "connection refused".into(),
        }
        .to_string();

        let from_crate =
            classify_search_error(&format!("stack search failed: {produced}"), "codescout");
        let from_root = classify_search_error(
            "stack search failed: dense embed connect failed: \
             http://127.0.0.1:48081/v1/embeddings — the dense embedder is unreachable",
            "codescout",
        );

        assert_eq!(
            from_crate, from_root,
            "root's producer and the crate's must land in the SAME bucket, or the \
             swap silently changes which hint an operator sees.\nproduced: {produced}"
        );
        assert!(
            from_crate.contains("CODESCOUT_EMBEDDER_URL"),
            "and that bucket must be the embedder hint. hint: {from_crate}"
        );
    }

    /// The sibling of the test above, for the other half of the failure space.
    ///
    /// Root's `dense openai status` and the crate's `EmbedError::Status` describe the
    /// same event — the server was reached and answered unusably — so they must reach
    /// the same operator. After T6 the crate's producer is the *only* one, and this is
    /// what will notice if the arm stops matching it.
    #[test]
    fn the_crates_own_status_error_routes_where_roots_does() {
        let produced = codescout_embed::EmbedError::Status {
            url: "http://127.0.0.1:48081/v1/embeddings".into(),
            // Deliberately bland. A body of "input is too large" would be routed by
            // the payload-size arm above — correctly, since that is more specific —
            // and this test would then be comparing two buckets neither producer
            // reached. The fixture has to avoid every earlier arm to test this one.
            status: 500,
            body: "internal server error".into(),
        }
        .to_string();

        let from_crate =
            classify_search_error(&format!("stack search failed: {produced}"), "codescout");
        let from_root = classify_search_error(
            "stack search failed: dense openai status 500: something went wrong",
            "codescout",
        );

        assert_eq!(
            from_crate, from_root,
            "root's producer and the crate's must land in the SAME bucket, or the swap \
             silently changes which hint an operator sees.\nproduced: {produced}"
        );
    }

    /// The regression that motivated typing this at all: a status body is untrusted
    /// text, and it must not be able to impersonate another arm.
    ///
    /// `model 'coderank' not found` is the ordinary shape of an embedder 404. Before
    /// `STATUS_FAILED_MARKER`, its `not found` reached the collection arm — which
    /// precedes the `embedding server` arm — and the operator was told to re-index a
    /// Qdrant collection that was perfectly healthy. Live on the `ollama:`/`openai:`
    /// resolver path, not a hazard the swap would have introduced.
    ///
    /// Asserting the *negative* is the point. A test that only checked the right hint
    /// appears would pass on an arm ordered after the collection bucket, because both
    /// hints mention the embedder.
    #[test]
    fn a_status_body_saying_not_found_is_not_reported_as_a_missing_collection() {
        let produced = codescout_embed::EmbedError::Status {
            url: "http://127.0.0.1:48081/v1/embeddings".into(),
            status: 404,
            body: "model 'coderank' not found".into(),
        }
        .to_string();

        let hint = classify_search_error(&format!("stack search failed: {produced}"), "codescout");

        assert!(
            !hint.contains("collection is missing"),
            "an embedder 404 must not be reported as a missing Qdrant collection — the \
             body is the server's text, not a statement about the store.\nhint: {hint}"
        );
        assert!(
            !hint.contains("sync_project"),
            "and it must not tell the operator to re-index a healthy store.\nhint: {hint}"
        );
        assert!(
            hint.contains("embedder"),
            "it must reach the embedder bucket.\nhint: {hint}"
        );
    }

    /// The sparse leg's twin of the test above, and it became necessary the moment
    /// the sparse path started carrying the server's body.
    ///
    /// Until 2026-08-30 that path used `error_for_status()`, so it had no body to be
    /// hijacked by — and nothing useful to tell an operator either. Restoring the
    /// body without hoisting this arm above the collection bucket would have traded
    /// one defect for the other, which is why both landed together.
    #[test]
    fn a_sparse_status_body_saying_not_found_is_not_reported_as_a_missing_collection() {
        let err = format!(
            "stack search failed: {} 404: model 'splade' not found",
            crate::retrieval::embedder::SPARSE_STATUS_MARKER
        );

        let hint = classify_search_error(&err, "codescout");

        assert!(
            !hint.contains("collection is missing"),
            "a sparse 404 must not be reported as a missing Qdrant collection — the \
             body is the server's text, not a statement about the store.\nhint: {hint}"
        );
        assert!(
            !hint.contains("sync_project"),
            "and it must not tell the operator to re-index a healthy store.\nhint: {hint}"
        );
        assert!(
            hint.contains("embedder"),
            "it must reach the embedder bucket.\nhint: {hint}"
        );
    }

    /// End-to-end: the **real** batch producer's error, through the **real**
    /// classifier.
    ///
    /// This is the test that would have caught the original defect, and the reason
    /// it drives `EmbedderHttp` rather than formatting a string. Every other test in
    /// this module builds its input from a literal or from `SPARSE_MARKER`, and
    /// therefore cannot notice a producer that has stopped rendering the marker —
    /// which is exactly what had happened. `embed_one_batch` said `embed_batch
    /// sparse …`, which shares no substring with the `embed sparse` matched here
    /// (`embed` is followed by `_`, not a space), while this function's own comment
    /// asserted that it did. Both sides were individually covered; nothing joined
    /// them, so nothing failed.
    ///
    /// Mutating the producer's wording back turns this red and moves nothing else.
    #[cfg(feature = "remote-embed")]
    #[tokio::test]
    async fn the_batch_sparse_producers_real_error_reaches_an_embedder_bucket() {
        let mut dense_server = mockito::Server::new_async().await;
        let mut sparse_server = mockito::Server::new_async().await;
        dense_server
            .mock("POST", "/v1/embeddings")
            .with_status(200)
            .with_body(r#"{"data":[{"embedding":[0.1,0.2,0.3],"index":0}]}"#)
            .create_async()
            .await;
        // 400 is non-retryable, so this terminates on the first attempt rather than
        // walking the eight-step backoff ladder.
        sparse_server
            .mock("POST", "/embed_sparse")
            .with_status(400)
            .with_body("bad input")
            .create_async()
            .await;

        let e = crate::retrieval::embedder::EmbedderHttp::new(
            dense_server.url(),
            sparse_server.url(),
            3,
        );
        // `embed_batch` rather than the private per-sub-batch unit: it is the entry
        // point production actually calls, so this exercises the chunking driver and
        // the `/info` probe (which the mock refuses, falling back to 8) on the way.
        let err = e
            .embed_batch(&["x".to_string()])
            .await
            .expect_err("a 400 from the sparse server cannot produce an embedding")
            .to_string();

        let hint = classify_search_error(&err, "codescout");

        assert!(
            hint.contains("embedder"),
            "the batch sparse producer's real error must reach an embedder bucket, \
             not the generic fallback that sends operators to `docker logs \
             codescout-qdrant` for an embedder fault.\nerr:  {err}\nhint: {hint}"
        );
        // Match the fallback's OWN opening words, not "qdrant logs" — the correct
        // hint contains that phrase too, in "don't check qdrant logs". A negative
        // assertion has to name something only the wrong answer says.
        assert!(
            !hint.starts_with("Stack reachable but query failed"),
            "and specifically not the generic fallback.\nerr:  {err}\nhint: {hint}"
        );
    }

    #[test]
    fn openai_send_does_not_hit_generic_qdrant_fallback() {
        let err = "stack search failed: dense openai send";
        let hint = classify_search_error(err, "codescout");
        assert!(
            !hint.contains("Stack reachable but query failed"),
            "a dense send failure must route to the embedder bucket, not the generic fallback: {hint}"
        );
    }

    /// The sibling gap the test above did not cover, and it was live.
    ///
    /// The embedder bucket matched "openai send" but not "openai **status**", so every
    /// dense-embedder HTTP failure — the 400 a payload-size refusal produces, among
    /// others — fell all the way to the generic fallback and told the operator to read
    /// qdrant logs for an embedder fault. Exactly the misrouting class the 2026-07-13
    /// bug and this module's other tests exist to prevent, one word away from the
    /// case that was covered.
    #[test]
    fn openai_status_does_not_hit_generic_qdrant_fallback() {
        let err = "stack search failed: dense openai status 500: upstream unavailable";
        let hint = classify_search_error(err, "codescout");
        assert!(
            !hint.contains("Stack reachable but query failed")
                && !hint.contains("docker logs codescout-qdrant"),
            "a dense STATUS failure must route to the embedder bucket, not the generic \
             qdrant fallback: {hint}"
        );
    }

    /// A payload-size refusal is not an outage, and must not be reported as one.
    ///
    /// Retrying will not help and neither will raising llama.cpp's `--ctx-size`, since
    /// the model's own `n_ctx_train` is the binding ceiling — so a hint that says
    /// "restart the stack" actively misleads. Checked ahead of every other arm.
    #[test]
    fn an_oversized_payload_routes_to_a_size_hint_not_an_outage_hint() {
        let err = "dense openai status 400: {\"error\":{\"code\":400,\"message\":\"input \
                   (1682 tokens) is larger than the max context size (1024 tokens). \
                   skipping\",\"type\":\"exceed_context_size_error\"}}";
        let hint = classify_search_error(err, "codescout");
        assert!(
            hint.contains("PAYLOAD SIZE"),
            "must name the cause as size, not reachability: {hint}"
        );
        assert!(
            !hint.contains("retrieval-stack.sh up"),
            "must NOT suggest restarting the stack — the payload is too big, the stack \
             is fine: {hint}"
        );
    }

    /// The variant this stack actually emits, which the first version of the arm above
    /// did NOT match — so the hint was dead on the very configuration it was written
    /// for. Green tests, dead branch, caught only by calling the real server.
    ///
    /// Measured 2026-08-26 against the running CodeRankEmbed llama-server by binary
    /// search on input length: 8000 chars → HTTP 200; 8250 chars → HTTP 500 with
    /// `{"message":"input is too large to process. increase the physical batch size",
    /// "type":"server_error"}`. Note it is a 500 and a `server_error`, not the 400
    /// `exceed_context_size_error` the originating issue documented — same class of
    /// problem, different llama.cpp code path (n_batch vs n_ctx-per-slot), different
    /// remedy.
    #[test]
    fn the_n_batch_payload_refusal_this_stack_emits_also_routes_to_the_size_hint() {
        let err = "dense openai status 500: {\"error\":{\"code\":500,\"message\":\"input is \
                   too large to process. increase the physical batch size\",\"type\":\
                   \"server_error\"}}";
        let hint = classify_search_error(err, "codescout");
        assert!(
            hint.contains("PAYLOAD SIZE"),
            "the n_batch refusal is a size problem too, not an outage: {hint}"
        );
        assert!(
            !hint.contains("retrieval-stack.sh up"),
            "must not tell the operator to restart a server that is up: {hint}"
        );
        // A 500 must not be read as "the stack went away" — that is the misrouting the
        // generic and transport buckets would produce.
        assert!(
            !hint.contains("Stack went offline") && !hint.contains("Stack reachable but"),
            "a 500 carrying a size message is not an outage: {hint}"
        );
    }

    /// The response body is arbitrary remote text, and the collection bucket matches
    /// "not found" / "Collection" very broadly. Lifting the body into the message (so
    /// the real cause is readable) therefore created a hijack path: an embedder 404
    /// whose body happens to say "not found" would be reported as a missing Qdrant
    /// collection, sending the operator to re-run sync_project for an embedder fault.
    ///
    /// This is the regression guard for the ordering that prevents it.
    #[test]
    fn an_embedder_body_mentioning_not_found_does_not_hijack_the_collection_bucket() {
        let err = "dense openai status 404: {\"error\":\"model not found\"}";
        let hint = classify_search_error(err, "codescout");
        assert!(
            !hint.contains("Qdrant collection is missing"),
            "an embedder error carrying 'not found' in its BODY must stay in the \
             embedder bucket: {hint}"
        );
        assert!(
            hint.contains("embedder problem"),
            "and it must reach the embedder hint: {hint}"
        );
    }

    #[test]
    fn resolver_path_retry_exhaustion_routes_to_embedder_hint_not_qdrant() {
        // RemoteEmbedder's retry-exhaustion wording (reached via the no-url
        // resolver path an `ollama:`/`openai:` model takes, `remote.rs`'s
        // `anyhow!("embedding server unavailable after {MAX_RETRIES}
        // attempts")`) is distinct from `EmbedderHttp`'s "embed connect
        // failed" and must not fall through to the generic qdrant-oriented
        // fallback — same misrouting class as the 2026-07-13 bug.
        let err = "could not build the 'ollama:nomic-embed-text' embedder: \
                   embedding server unavailable after 3 attempts";
        let hint = classify_search_error(err, "codescout");
        assert!(
            hint.contains("[embeddings].model") || hint.contains("CODESCOUT_EMBEDDER_MODEL"),
            "hint must point at the model config: {hint}"
        );
        assert!(
            !hint.contains("docker logs codescout-qdrant")
                && !hint.contains("Stack reachable but query failed"),
            "must NOT route a resolver-path failure to the qdrant-logs fallback: {hint}"
        );
    }

    /// Rewritten 2026-08-30 to render the crate's real error rather than a literal.
    ///
    /// The hand-written string it used — `HTTP 401 from embedding server: invalid api
    /// key` — stopped being a wording any producer emits when `EmbedError::Status`
    /// landed, so the test was pinning a shape that could no longer occur. It still
    /// passed, which is the whole problem with a literal standing in for a producer.
    ///
    /// Its assertion is unchanged and still its own: a 401 must not reach the generic
    /// Qdrant fallback. Which embedder bucket it lands in is the differential test's
    /// question, not this one's.
    #[test]
    fn resolver_path_http_status_failure_does_not_hit_generic_fallback() {
        let produced = codescout_embed::EmbedError::Status {
            url: "https://api.openai.com/v1/embeddings".into(),
            status: 401,
            body: "invalid api key".into(),
        }
        .to_string();
        let err =
            format!("could not build the 'openai:text-embedding-3-small' embedder: {produced}");
        let hint = classify_search_error(&err, "codescout");
        assert!(
            !hint.contains("Stack reachable but query failed"),
            "an embedding-server HTTP failure must route to the resolver-path \
             embedder bucket, not the generic fallback: {hint}"
        );
    }
}

#[cfg(test)]
mod map_from_env_error_tests {
    use super::map_from_env_error;
    use crate::retrieval::code_store::VectorBackend;
    use crate::tools::RecoverableError;

    #[test]
    fn a_recoverable_error_passes_through_without_the_stack_offline_headline() {
        // I3(b): a `RecoverableError` from `from_env` (e.g. `build_embedder`'s
        // `guard_sparse` sparse-conflict guard) must reach the caller with its
        // own message intact, not relabelled "retrieval stack offline" — that
        // headline points at a restart script that cannot fix a config
        // conflict.
        let original = RecoverableError::with_hint(
            "the local embedding backend produces no sparse vector, but the \
             hybrid sparse leg is enabled.",
            "Either set CODESCOUT_DISABLE_SPARSE=1 to run dense-only, or \
             configure an embedder url that serves both dense and sparse.",
        );
        let original_msg = original.to_string();
        let mapped = map_from_env_error(original.into(), VectorBackend::Qdrant);
        assert!(
            mapped.downcast_ref::<RecoverableError>().is_some(),
            "must remain a RecoverableError: {mapped}"
        );
        assert_eq!(
            mapped.to_string(),
            original_msg,
            "must pass through unchanged, not acquire a new headline"
        );
        assert!(
            !mapped.to_string().contains("retrieval stack offline"),
            "a config conflict must not be reported as a down service: {mapped}"
        );
    }

    #[test]
    fn a_non_recoverable_error_gets_the_stack_offline_headline() {
        let original = anyhow::anyhow!("connection refused");
        let mapped = map_from_env_error(original, VectorBackend::Qdrant);
        assert!(
            mapped.to_string().contains("retrieval stack offline"),
            "a genuine connect failure must still get the stack-offline headline: {mapped}"
        );
        assert!(
            mapped.downcast_ref::<RecoverableError>().is_some(),
            "must still be RecoverableError so sibling calls survive: {mapped}"
        );
    }
}

#[cfg(test)]
mod worktree_search_tests {
    use super::*;

    #[test]
    fn worktree_hint_names_the_delta_project_and_both_exits() {
        let h = worktree_no_index_hint("codescout@wt");
        assert!(
            h.contains("codescout@wt"),
            "hint must name the resolved project id"
        );
        assert!(h.contains("index(action=\"build\")"));
        assert!(
            h.contains("grep"),
            "hint must offer the tools that ARE correct here"
        );
    }

    #[test]
    fn no_hint_when_the_delta_is_indexed() {
        // The negative direction. A hint that fires unconditionally passes the
        // positive test while telling the user nothing.
        assert!(worktree_hint_for(/* delta present */ true, "codescout@wt").is_none());
    }

    #[test]
    fn hint_fires_when_the_delta_is_not_present() {
        let hint = worktree_hint_for(/* delta present */ false, "codescout@wt");
        assert_eq!(hint, Some(worktree_no_index_hint("codescout@wt")));
    }

    /// Exhaustive over the meaningful combinations (sidecar-absent collapses the
    /// other two dimensions). HUMAN RULING: `(sidecar=true, dirty_empty=true,
    /// delta_has_chunks=true)` is `Suspect`, never read as "nothing is dirty" --
    /// that combination is impossible in a healthy state (a delta only exists
    /// because something was dirty).
    #[test]
    fn classify_worktree_index_state_covers_every_combination() {
        use WorktreeIndexState::*;
        assert_eq!(
            classify_worktree_index_state(false, true, true),
            NotIndexed,
            "no sidecar at all, regardless of the delta"
        );
        assert_eq!(
            classify_worktree_index_state(false, true, false),
            NotIndexed
        );
        assert_eq!(
            classify_worktree_index_state(false, false, true),
            NotIndexed,
            "sidecar-absent short-circuits before dirty_paths is even consulted"
        );
        assert_eq!(
            classify_worktree_index_state(true, false, false),
            Healthy,
            "a non-empty dirty set is the normal, healthy in-progress state"
        );
        assert_eq!(
            classify_worktree_index_state(true, false, true),
            Healthy,
            "non-empty dirty set stays healthy even when the delta also has chunks"
        );
        assert_eq!(
            classify_worktree_index_state(true, true, false),
            NotIndexed,
            "empty dirty set + empty delta = the worktree sync has simply not run yet"
        );
        assert_eq!(
            classify_worktree_index_state(true, true, true),
            Suspect,
            "HUMAN RULING: empty dirty set + non-empty delta is impossible in a healthy state"
        );
    }

    #[test]
    fn main_reindexed_after_worktree_flags_when_main_is_strictly_newer() {
        assert!(main_reindexed_after_worktree(
            "2026-08-13T10:00:00+00:00",
            "2026-08-13T09:00:00+00:00",
        ));
        assert!(!main_reindexed_after_worktree(
            "2026-08-13T09:00:00+00:00",
            "2026-08-13T10:00:00+00:00",
        ));
        assert!(
            !main_reindexed_after_worktree(
                "2026-08-13T09:00:00+00:00",
                "2026-08-13T09:00:00+00:00",
            ),
            "equal timestamps are not 'after' -- must be strictly greater"
        );
    }

    #[test]
    fn main_reindexed_after_worktree_is_false_on_unparseable_timestamps() {
        // Undetectable drift is reported as silence, never as a false claim of
        // freshness.
        assert!(!main_reindexed_after_worktree(
            "not-a-timestamp",
            "2026-08-13T09:00:00+00:00",
        ));
        assert!(!main_reindexed_after_worktree(
            "2026-08-13T09:00:00+00:00",
            "",
        ));
    }

    #[test]
    fn worktree_query_exclusions_puts_dirty_paths_on_main_only() {
        let dirty = vec!["src/a.rs".to_string(), "src/b.rs".to_string()];
        let (main_exclude, delta_exclude) = worktree_query_exclusions(&dirty);
        assert_eq!(
            main_exclude, dirty,
            "main must exclude the worktree's dirty paths"
        );
        assert!(
            delta_exclude.is_empty(),
            "delta must exclude nothing -- it holds exactly the dirty files"
        );
    }

    /// Writes a real `.codescout/index-state.json` sidecar under `root` with
    /// an explicit `last_indexed_at`, so `worktree_query_plan`'s drift check
    /// (a plain filesystem read, not a store call) has something real to
    /// read. Used only by the tests below.
    fn write_sidecar_at(root: &std::path::Path, last_indexed_at: &str) {
        std::fs::create_dir_all(root.join(".codescout")).unwrap();
        let state = crate::retrieval::index_state::IndexState {
            last_indexed_commit: String::new(),
            last_indexed_at: last_indexed_at.to_string(),
            schema_version: crate::retrieval::index_state::INDEX_STATE_SCHEMA_VERSION,
            indexed_with_model: None,
            dirty_paths: Vec::new(),
            last_sync_skipped_count: 0,
            last_sync_skipped_sample: Vec::new(),
            // No process wrote this fixture, so `None` is the honest value -- and
            // it doubles as the pre-field sidecar case these readers must tolerate.
            written_by: None,
        };
        std::fs::write(
            root.join(".codescout").join("index-state.json"),
            serde_json::to_string(&state).unwrap(),
        )
        .unwrap();
    }

    /// Writes a sidecar recording `skipped` skipped files, so `index_skip_note`
    /// has a real durable marker to read — the same one `sync_project` writes.
    fn write_sidecar_with_skips(root: &std::path::Path, skipped: &[&str]) {
        std::fs::create_dir_all(root.join(".codescout")).unwrap();
        let state = crate::retrieval::index_state::IndexState {
            last_indexed_commit: String::new(),
            last_indexed_at: "2026-08-27T00:00:00Z".to_string(),
            schema_version: crate::retrieval::index_state::INDEX_STATE_SCHEMA_VERSION,
            indexed_with_model: None,
            dirty_paths: Vec::new(),
            last_sync_skipped_count: skipped.len(),
            last_sync_skipped_sample: skipped.iter().map(|s| s.to_string()).collect(),
            written_by: None,
        };
        std::fs::write(
            root.join(".codescout").join("index-state.json"),
            serde_json::to_string(&state).unwrap(),
        )
        .unwrap();
    }

    /// The marker `sync_project` persists must reach the tool that CONSUMES the
    /// index, not only `index(action="status")` — which a caller only runs if they
    /// already suspect a problem.
    ///
    /// Both controls are the point: a warning that fires on a clean index, or with
    /// no sidecar at all, is one a reader learns to ignore, and it would look
    /// correct in the positive case alone.
    ///
    /// BUG docs/issues/archive/2026-08-26-catalog-reindex-fails-closed-on-embedding-error.md
    #[test]
    fn index_skip_note_fires_only_when_the_last_sync_actually_skipped() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();

        // No sidecar at all — nothing is known, so nothing may be claimed.
        assert!(
            index_skip_note(Some(root)).is_none(),
            "absent sidecar must not read as degraded"
        );

        // Clean sync — the marker exists and says zero.
        write_sidecar_with_skips(root, &[]);
        assert!(
            index_skip_note(Some(root)).is_none(),
            "a clean sync must not warn"
        );

        // Dirty sync.
        write_sidecar_with_skips(root, &["src/a.rs", "src/b.rs", "src/c.rs"]);
        let note = index_skip_note(Some(root)).expect("a skipped sync must warn");
        assert!(note.contains('3'), "the note must name the count: {note}");
        assert!(
            note.contains("index(action=\"status\")"),
            "the note must route to the surface holding the sample: {note}"
        );
    }

    /// The note must reach a READER, not merely sit in the JSON. `call_content`'s
    /// overflow path shows `truncate_compact(format_semantic_search(val), …)` and
    /// nothing else, so a field appended after the result rows is cut away on
    /// exactly the searches large enough to need it.
    ///
    /// Asserting on the rendered string rather than the Value is the point of this
    /// test: a correctly-populated field that no formatter renders is an inert
    /// field, and reads as done.
    #[test]
    fn the_incomplete_index_warning_survives_rendering_and_truncation() {
        let long_content = "x".repeat(200);
        let results: Vec<serde_json::Value> = (0..80)
            .map(|i| {
                serde_json::json!({
                    "file_path": format!("src/f_{i}.rs"),
                    "start_line": 1,
                    "end_line": 2,
                    "content": long_content.clone(),
                })
            })
            .collect();
        let val = serde_json::json!({
            "results": results,
            "total": 80,
            "index_degraded_note": "SKIP-MARKER-MUST-SURVIVE",
        });

        let formatted = format_semantic_search(&val);
        assert!(
            formatted.contains("SKIP-MARKER-MUST-SURVIVE"),
            "the note must be rendered at all: {formatted}"
        );
        assert!(
            formatted.len() > 3_000,
            "fixture must exceed the hard cap to test anything: {} bytes",
            formatted.len()
        );
        let truncated = crate::tools::core::types::truncate_compact(&formatted, 2_000, 3_000);
        assert!(
            truncated.contains("SKIP-MARKER-MUST-SURVIVE"),
            "the note must survive the tail cut, or it vanishes on exactly the \
             searches big enough to need it: {truncated}"
        );
    }

    /// And it must reach the zero-results case, which is where an incomplete index
    /// is most likely to be misread: "0 results" over a partial index is the shape
    /// that reads as "the code does not contain this".
    #[test]
    fn the_incomplete_index_warning_reaches_the_zero_results_case() {
        let val = serde_json::json!({
            "results": [],
            "total": 0,
            "index_degraded_note": "SKIP-MARKER-MUST-SURVIVE",
        });
        let formatted = format_semantic_search(&val);
        assert!(
            formatted.contains("SKIP-MARKER-MUST-SURVIVE"),
            "an empty result set is exactly when this matters most: {formatted}"
        );
    }

    #[test]
    fn worktree_query_plan_excludes_dirty_paths_from_main_only() {
        let dirty = vec!["src/a.rs".to_string()];
        let dir = tempfile::tempdir().unwrap();
        let main_repo = dir.path().join("main"); // no sidecar -> no drift note
        std::fs::create_dir_all(&main_repo).unwrap();

        let plan = worktree_query_plan(
            &main_repo,
            "main-id",
            "main-id@wt",
            WorktreeIndexState::Healthy,
            &dirty,
            "2026-08-13T09:00:00+00:00",
            true,
        );
        assert_eq!(
            plan.main_exclude, dirty,
            "main must exclude the dirty paths"
        );
        assert!(plan.delta_exclude.is_empty(), "delta must exclude nothing");
        assert_eq!(plan.main_id, "main-id");
        assert_eq!(plan.delta_id, "main-id@wt");
    }

    #[test]
    fn worktree_query_plan_sets_warning_only_when_suspect() {
        let dir = tempfile::tempdir().unwrap();
        let main_repo = dir.path().join("main");
        std::fs::create_dir_all(&main_repo).unwrap();

        let healthy = worktree_query_plan(
            &main_repo,
            "m",
            "m@wt",
            WorktreeIndexState::Healthy,
            &[],
            "2026-08-13T09:00:00+00:00",
            true,
        );
        assert!(healthy.warning.is_none(), "Healthy must carry no warning");

        let suspect = worktree_query_plan(
            &main_repo,
            "m",
            "m@wt",
            WorktreeIndexState::Suspect,
            &[],
            "2026-08-13T09:00:00+00:00",
            true,
        );
        assert!(
            suspect.warning.is_some(),
            "Suspect must carry the human-ruled warning"
        );
        assert!(suspect.warning.unwrap().contains("m@wt"));
    }

    #[test]
    fn worktree_query_plan_sets_main_never_indexed_note_from_the_bool() {
        let dir = tempfile::tempdir().unwrap();
        let main_repo = dir.path().join("main");
        std::fs::create_dir_all(&main_repo).unwrap();

        let indexed = worktree_query_plan(
            &main_repo,
            "m",
            "m@wt",
            WorktreeIndexState::Healthy,
            &[],
            "2026-08-13T09:00:00+00:00",
            true,
        );
        assert!(
            indexed.main_never_indexed_note.is_none(),
            "main_has_chunks=true must carry no note"
        );
        assert!(
            indexed.query_main,
            "main holds chunks, so it must still be queried"
        );

        let not_indexed = worktree_query_plan(
            &main_repo,
            "m",
            "m@wt",
            WorktreeIndexState::Healthy,
            &[],
            "2026-08-13T09:00:00+00:00",
            false,
        );
        assert!(
            not_indexed.main_never_indexed_note.is_some(),
            "main_has_chunks=false must carry the requirement-6 note"
        );
        assert!(not_indexed.main_never_indexed_note.unwrap().contains('m'));
        // C2: the note alone is not the fix. Main holding nothing means the
        // query can only return an empty page, and the shipped code still ran
        // it -- shipping every excluded path across the wire to learn that.
        assert!(
            !not_indexed.query_main,
            "main holds no chunks: the query must be skipped entirely, not just \
             annotated"
        );
    }

    #[test]
    fn worktree_query_plan_sets_drift_note_when_main_is_strictly_newer() {
        // Real tmpdir fixture with an actual, asymmetric sidecar -- this is
        // what catches an argument-order swap inside `worktree_query_plan`'s
        // `main_reindexed_after_worktree` call. Main's real sidecar says
        // 10:00; the worktree's timestamp passed in is 09:00. Swap the two
        // arguments at the call site and this flips to `None`.
        let dir = tempfile::tempdir().unwrap();
        let main_repo = dir.path().join("main");
        write_sidecar_at(&main_repo, "2026-08-13T10:00:00+00:00");

        let plan = worktree_query_plan(
            &main_repo,
            "m",
            "m@wt",
            WorktreeIndexState::Healthy,
            &[],
            "2026-08-13T09:00:00+00:00",
            true,
        );
        assert!(
            plan.drift_note.is_some(),
            "main's sidecar is strictly newer than the worktree's -- drift_note must fire"
        );
    }

    #[test]
    fn worktree_query_plan_no_drift_note_when_worktree_is_newer_or_equal() {
        // The negative direction for the same argument-order hazard: if the
        // swap flipped it the OTHER way, this test (not the one above) would
        // catch it.
        let dir = tempfile::tempdir().unwrap();
        let main_repo = dir.path().join("main");
        write_sidecar_at(&main_repo, "2026-08-13T09:00:00+00:00");

        let plan = worktree_query_plan(
            &main_repo,
            "m",
            "m@wt",
            WorktreeIndexState::Healthy,
            &[],
            "2026-08-13T10:00:00+00:00", // worktree's delta built AFTER main
            true,
        );
        assert!(plan.drift_note.is_none());
    }

    #[test]
    fn worktree_query_plan_no_drift_note_when_main_has_no_sidecar() {
        // Undetectable drift is reported as silence, never as a false claim
        // of freshness.
        let dir = tempfile::tempdir().unwrap();
        let main_repo = dir.path().join("main");
        std::fs::create_dir_all(&main_repo).unwrap();

        let plan = worktree_query_plan(
            &main_repo,
            "m",
            "m@wt",
            WorktreeIndexState::Healthy,
            &[],
            "2026-08-13T09:00:00+00:00",
            true,
        );
        assert!(plan.drift_note.is_none());
    }

    #[test]
    fn worktree_search_opts_gives_main_the_dirty_list_and_delta_nothing() {
        // Targets the assignment hazard directly: `call()` no longer has raw
        // `main_opts.exclude_paths = ...` / `delta_opts.exclude_paths = ...`
        // statements to swap -- it just calls this function and uses the
        // tuple. A swap now has exactly one place to hide, and this is the
        // test that finds it there.
        let base = crate::retrieval::search::SearchOpts::new(10);
        let dir = tempfile::tempdir().unwrap();
        let main_repo = dir.path().join("main");
        std::fs::create_dir_all(&main_repo).unwrap();
        let plan = worktree_query_plan(
            &main_repo,
            "m",
            "m@wt",
            WorktreeIndexState::Healthy,
            &["src/a.rs".to_string()],
            "2026-08-13T09:00:00+00:00",
            true,
        );

        let (main_opts, delta_opts) = worktree_search_opts(&base, &plan);
        assert_eq!(
            main_opts.exclude_paths,
            vec!["src/a.rs".to_string()],
            "main must get the dirty list"
        );
        assert!(
            delta_opts.exclude_paths.is_empty(),
            "delta must get nothing"
        );
        // Base fields survive the clone untouched.
        assert_eq!(main_opts.limit, 10);
        assert_eq!(delta_opts.limit, 10);
    }

    #[test]
    fn apply_worktree_plan_notes_sets_only_present_fields() {
        let plan = WorktreeQueryPlan {
            main_id: "m".to_string(),
            delta_id: "m@wt".to_string(),
            main_exclude: Vec::new(),
            delta_exclude: Vec::new(),
            warning: Some("SUSPECT-TEXT".to_string()),
            query_main: true,
            main_never_indexed_note: None,
            drift_note: Some("DRIFT-TEXT".to_string()),
        };
        let mut out = serde_json::json!({});
        apply_worktree_plan_notes(&mut out, &plan);
        assert_eq!(out["worktree_state_warning"], "SUSPECT-TEXT");
        assert!(out.get("main_never_indexed_note").is_none());
        assert_eq!(out["drift_note"], "DRIFT-TEXT");
    }

    #[test]
    fn search_response_flags_truncated_only_at_the_limit() {
        fn hit(id: &str) -> crate::retrieval::search::Hit {
            crate::retrieval::search::Hit {
                chunk_id: id.to_string(),
                file_path: "src/a.rs".to_string(),
                start_line: 1,
                end_line: 1,
                content: String::new(),
                score: 1.0,
                rerank_score: None,
            }
        }
        let short = vec![hit("a")];
        let resp = search_response(&short, 5);
        assert_eq!(resp["truncated"], false);
        assert!(resp.get("truncated_hint").is_none());

        let full = vec![hit("a"), hit("b")];
        let resp = search_response(&full, 2);
        assert_eq!(resp["truncated"], true);
        assert!(resp["truncated_hint"].as_str().is_some());
    }

    #[test]
    fn format_semantic_search_surfaces_worktree_state_fields() {
        // These fields are only reachable this way when the response is small
        // enough to stay inline. When it's large enough to buffer,
        // `call_content` builds its summary from THIS function's output, so a
        // field this function doesn't mention is invisible to the caller even
        // though it is sitting right there in the full JSON.
        let val = serde_json::json!({
            "results": [{"file_path": "src/a.rs", "start_line": 1, "end_line": 2, "content": "fn a() {}"}],
            "total": 1,
            "truncated": false,
            "hint": "HINT-MARKER",
            "worktree_state_warning": "SUSPECT-MARKER",
            "drift_note": "DRIFT-MARKER",
        });
        let out = format_semantic_search(&val);
        assert!(out.contains("HINT-MARKER"), "must surface hint: {out}");
        assert!(
            out.contains("SUSPECT-MARKER"),
            "must surface worktree_state_warning: {out}"
        );
        assert!(
            out.contains("DRIFT-MARKER"),
            "must surface drift_note: {out}"
        );
    }

    #[test]
    fn format_semantic_search_omits_absent_state_fields() {
        let val = serde_json::json!({
            "results": [{"file_path": "src/a.rs", "start_line": 1, "end_line": 2, "content": "fn a() {}"}],
            "total": 1,
            "truncated": false,
        });
        let out = format_semantic_search(&val);
        assert!(
            !out.contains("Warning:") && !out.contains("Note:") && !out.contains("Hint:"),
            "must not fabricate a label when the field is absent: {out}"
        );
    }

    #[test]
    fn format_semantic_search_surfaces_hint_even_with_zero_results() {
        // The worktree "NotIndexed" response is the ONE place `hint` is ever
        // set, and it always carries `results: []`. A version of this
        // function that builds state fields AFTER an `if results.is_empty()`
        // early return can never reach them for this tool's own hint --
        // reproducing, one layer up, the exact "0 results, no explanation"
        // shape the worktree feature exists to eliminate.
        let val = serde_json::json!({
            "results": [],
            "total": 0,
            "truncated": false,
            "hint": "NOT-INDEXED-HINT",
        });
        let out = format_semantic_search(&val);
        assert!(
            out.contains("NOT-INDEXED-HINT"),
            "hint must reach the summary even with zero results: {out}"
        );
    }

    #[test]
    fn format_semantic_search_keeps_state_fields_above_the_truncation_cap() {
        // `call_content`'s overflow path shows the caller
        // `truncate_compact(format_semantic_search(val), soft, hard)` and
        // NOTHING else -- truncate_compact keeps only the PREFIX up to the
        // last newline within the hard cap. A large-enough result set pushes
        // this well past 3_000 bytes; state fields placed after the rows
        // (as a prior version of this function did) would be cut away here.
        let long_content = "x".repeat(200);
        let results: Vec<serde_json::Value> = (0..80)
            .map(|i| {
                serde_json::json!({
                    "file_path": format!("src/f_{i}.rs"),
                    "start_line": 1,
                    "end_line": 2,
                    "content": long_content.clone(),
                })
            })
            .collect();
        let val = serde_json::json!({
            "results": results,
            "total": 80,
            "truncated": true,
            "worktree_state_warning": "SUSPECT-MARKER-MUST-SURVIVE",
        });
        let formatted = format_semantic_search(&val);
        assert!(
            formatted.len() > 3_000,
            "fixture must actually exceed the hard cap to test anything: {} bytes",
            formatted.len()
        );
        let truncated = crate::tools::core::types::truncate_compact(&formatted, 2_000, 3_000);
        assert!(
            truncated.contains("SUSPECT-MARKER-MUST-SURVIVE"),
            "state field must survive truncation from the tail: {truncated}"
        );
    }
}
