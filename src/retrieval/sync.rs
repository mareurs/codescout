use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

use crate::util::fs::to_forward_slash;

/// Directories the code index never walks, regardless of `.gitignore` or
/// user-supplied ignore patterns.
///
/// These hold *tool state derived from the project*, not project content, so
/// embedding them makes the corpus self-referential: `semantic_search` starts
/// returning codescout's own memories and catalog rows as if they were source.
/// `.git` additionally carries every historical blob, which no search should see.
///
/// Deliberately not listed: `.claude`, `.serena`, `.buddy`. Those can hold real
/// project documentation — skills, command definitions, prompts — that a caller
/// may legitimately want indexed. They belong in per-project `ignore_patterns`,
/// which is a decision, not a default.
pub(crate) const ALWAYS_SKIP_DIRS: &[&str] = &[".git", ".codescout"];

#[derive(Debug, Clone, Default)]
pub struct SyncOpts {
    pub languages: Option<Vec<String>>,
    pub force_reindex: bool,
    /// When true, `sync_project` records the indexed git HEAD to
    /// `.codescout/index-state.json` on success (the freshness sidecar that
    /// external consumers and `index(action="status")` read). Set by *project*
    /// syncs; left false by *library* syncs so library checkouts aren't polluted.
    pub record_index_state: bool,
    /// Glob/gitignore-style patterns to exclude from the index walk. Sourced from
    /// `config.ignored_paths.patterns`; an empty vec ignores nothing.
    pub ignore_patterns: Vec<String>,
    /// Directory to site the per-project index lock in. `None` — every production
    /// caller — resolves `per_user_runtime_dir()`.
    ///
    /// A test seam, and the only one available here: `sync_project` takes the lock
    /// internally, so a test that drives it end-to-end otherwise writes into the
    /// real runtime dir, and lock files are deliberately never unlinked. See
    /// docs/issues/archive/2026-07-28-index-lock-tests-pollute-runtime-dir.md.
    pub index_lock_dir: Option<std::path::PathBuf>,
    /// Provenance of the process performing this sync. `None` — every production
    /// caller — snapshots the live process via `index_state::current_writer()`.
    ///
    /// A test seam, for the same reason `index_lock_dir` is one: the staleness
    /// signal `guard_stale_binary` acts on comes from reading this process's own
    /// `/proc/self/exe`, which a test cannot make say "deleted" without unlinking
    /// the running test binary. Injecting the provenance keeps the policy
    /// testable while leaving detection where it belongs.
    pub writer: Option<crate::retrieval::index_state::WriterProvenance>,
}

#[derive(Debug, Default)]
pub struct SyncReport {
    pub added: usize,
    pub updated: usize,
    pub deleted: usize,
    pub elapsed_ms: u128,
    /// Chunks the embedder refused, as `file_path:start_line`. Non-empty means the
    /// index is INCOMPLETE even though the sync returned `Ok` — a sync that skips
    /// content must say so, because nothing marks a skipped chunk dirty and a
    /// later no-op sync will never reconcile it.
    pub skipped: Vec<String>,
    /// `Some((from, to))` when a `force=true` sync discarded this project's index to
    /// rebuild it at a new embedding dimension. Reported because a silent successful
    /// rebuild at a different width is nearly as confusing as the failure it
    /// replaced.
    pub dim_migration: Option<(u64, u64)>,
}

/// Refuse to (re-)index from a process whose own executable has been unlinked.
///
/// A `cargo rb` replaces the binary on disk, but already-running servers keep
/// executing the old inode. Such a process would embed with code and config that
/// no longer exist anywhere, then stamp the shared per-project sidecar with the
/// result — and whoever syncs last wins, so it can overwrite a current server's
/// state.
///
/// **The refusal sits before the embed pass, deliberately.** Direction 2 of the
/// zombie-server bug originally refused the *sidecar write*, which is inverted at
/// `sync_project`: the vectors are already in the store by then, so declining the
/// write destroys the honest record of what happened while keeping the damage
/// itself. Declining to re-index at all is the only placement that helps. See
/// `docs/issues/archive/2026-08-26-zombie-servers-on-deleted-binaries-stamp-stale-config-into-shared-state.md`
/// § *Re-costed 2026-08-28*, and `open-issue-work-queue:BL-45`.
///
/// `exe_deleted` is `Option<bool>` and the `Some(true)` test is exact on purpose.
/// `None` means the check could not run — `exe_is_deleted()` returns `None` off
/// Linux, where `/proc/self/exe` does not exist — and reading it as "deleted"
/// would refuse to index on every non-Linux platform. Absence is "not recorded",
/// never "yes".
///
/// `RecoverableError`, not `bail!`: the operator can fix this by restarting the
/// server, and `isError: false` lets sibling parallel tool calls survive.
pub(crate) fn guard_stale_binary(exe_deleted: Option<bool>) -> Result<()> {
    if exe_deleted == Some(true) {
        return Err(crate::tools::RecoverableError::with_hint(
            "this codescout server is running a binary that has been deleted from disk, \
             so re-indexing would write vectors produced by code that no longer exists \
             and stamp them into shared per-project state."
                .to_string(),
            "Restart the MCP server, then re-run the index — in Claude Code, run `/mcp` \
             to reconnect. A release build (`cargo rb`) unlinks the running binary, so \
             every server started before that build is in this state.",
        )
        .into());
    }
    Ok(())
}

/// The staleness signal for a sync whose caller injected no [`WriterProvenance`].
///
/// Production reads the live process. **In test builds this is always
/// `Some(false)`, and that is a fix rather than a convenience.**
///
/// The hazard [`guard_stale_binary`] exists for is a long-lived **server** that
/// keeps re-indexing after its binary was replaced, stamping vectors from code that
/// no longer exists into shared per-project state. A test process has neither
/// property: it writes to a tempdir and exits. So the guard was answering a question
/// nobody had asked, with the live process's own `/proc/self/exe`.
///
/// What that cost, measured 2026-09-02: a peer running `cargo build` on this shared
/// checkout unlinks `target/debug/deps/codescout-<hash>` **while the suite is
/// running**, the guard correctly observes its own executable is gone, and every
/// `sync_*` test that did not inject a writer refuses before doing any work — 13 of
/// them, reproducibly, in the same order. Under `--workspace` there are ~68s in
/// which a peer build can land; in isolation 0.63s, which is why the same 13 pass
/// alone and fail together. Not timing-sensitivity: it tracks *who else is
/// building*, not what the tests do, which is why it appeared and vanished without
/// anyone touching test code.
///
/// **What this deliberately stops covering.** The production branch — snapshotting
/// the live process — now has no test exercising it. It never had a deliberate one:
/// those 13 tests reached it incidentally, and that incidental coverage is precisely
/// what broke. The policy itself is covered directly and does not go through here
/// ([`guard_stale_binary`]'s three unit tests, plus the call-site wiring proof that
/// injects `exe_deleted: Some(true)`), so what is untested is the one-line snapshot,
/// not the rule.
fn ambient_exe_deleted() -> Option<bool> {
    #[cfg(test)]
    {
        Some(false)
    }
    #[cfg(not(test))]
    {
        crate::retrieval::index_state::current_writer().exe_deleted
    }
}

impl std::fmt::Display for SyncReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "added={} updated={} deleted={} elapsed_ms={}",
            self.added, self.updated, self.deleted, self.elapsed_ms
        )?;
        // Only rendered when non-empty: a skipped chunk means the index is
        // incomplete, and that must not be a field a reader learns to skim past.
        if !self.skipped.is_empty() {
            write!(f, " SKIPPED={} (index incomplete)", self.skipped.len())?;
        }
        if let Some((from, to)) = self.dim_migration {
            write!(f, " dim_migrated={from}->{to}")?;
        }
        Ok(())
    }
}

pub fn content_hash(text: &str) -> String {
    let mut h = Sha256::new();
    h.update(text.as_bytes());
    format!("{:x}", h.finalize())
}

/// Build the vector-store chunk id for a file chunk.
///
/// The path component MUST be forward-slash normalized. `rel_path` is OS-derived
/// (it comes from a `strip_prefix` of a filesystem-walk path), so on Windows a raw
/// `.display()` yields `src\lib.rs` — which would (a) persist native separators as
/// the vector store's primary key, (b) make the id disagree with the `file_path`
/// field of its own payload (which normalizes one line below), and (c) make the id
/// platform-dependent, so the `local_ids` / `server_ids` delete-set diff cannot be
/// compared across hosts. See
/// `docs/issues/archive/2026-07-07-display-audit-scope-gap-non-to-string-sites.md`.
///
/// `start_line` disambiguates two chunks in the same file with byte-identical
/// content — without it the id was `(project, path, content_hash)` alone, so
/// duplicate-content chunks collided on the same Qdrant point id and the second
/// silently overwrote the first (measured 2026-08-16: 10.97% of a fresh index's
/// chunks lost this way, with both the writer and the drift check agreeing nothing
/// was wrong). See
/// `docs/issues/archive/2026-08-16-chunk-id-omits-index-so-duplicate-chunks-collapse.md`.
/// `dirty_paths` (`drift.rs`) is unaffected by this: it keys on the separate
/// `(file_path, content_hash)` fields on `ChunkRef`/`LocalChunk`, never on the
/// `chunk_id` string's internal shape.
pub fn chunk_id(
    project_id: &str,
    rel_path: &Path,
    start_line: usize,
    content_hash: &str,
) -> String {
    format!(
        "{project_id}:{}:{start_line}:{content_hash}",
        to_forward_slash(rel_path)
    )
}

/// Project id for a worktree's delta index: the changed files only.
///
/// `worktree_dir` MUST be a single segment (e.g. `"wt"`), not a path — Task 6
/// writes chunks under this id and Task 7 queries them; if either passes a path
/// where the other passes a segment, the two disagree on the id and the delta
/// becomes invisible with no error (main silently serves stale chunks).
///
/// It must also be a segment that is **unique per repository**, which a
/// directory basename is not. Both callers derive it through [`worktree_key`],
/// which returns git's own worktree name and falls back to the basename only
/// when there is no linked-worktree pointer to read — see that function for
/// what keying on the basename cost.
///
/// `@` rather than `:` deliberately — [`chunk_id`] joins on `:`, and the
/// `delete_chunks` comment at `sqlite_code_store.rs:234-238` (pinned by the
/// regression test `delete_resolves_db_by_project_id_not_chunk_prefix` at
/// `sqlite_code_store.rs:663`) documents a real regression from colon-bearing
/// project ids — library ids are `lib:{name}`. `@` keeps the delta id
/// unambiguous under that join.
///
/// The lite (sqlite-vec) store additionally maps a project id to a DB *file*
/// stem via [`crate::sqlite_vec_ext::sanitize_db_name`], which is NOT
/// injective: every character outside `[A-Za-z0-9_-]` — including `@` —
/// collapses to `_`. So `{main}@{worktree_dir}` can land in the same file a
/// real project literally named `{main}_{worktree_dir}` would use, exactly as
/// `lib:{name}` already aliases the file `lib_{name}` would use. That file
/// aliasing is harmless, not a bug: every sqlite read additionally filters
/// rows on the *unsanitized* `project_id` column (`query`, `chunk_refs`,
/// `project_index_stats`, `project_has_chunks` in `sqlite_code_store.rs`), so
/// two logical projects sharing a file never share rows — the file is a
/// locality partition, the column is the isolation boundary. On Qdrant,
/// `project_id` is a payload filter value in one global collection rather
/// than a filename, so `@` survives unchanged there and no file-level
/// aliasing occurs at all.
///
/// The column-filtering argument above depends on this string never being
/// *equal* to a real project's id. `delta_project_id_is_distinct_and_separator_is_not_a_colon`
/// below only pins inequality against `main_project_id` itself — it says
/// nothing about an unrelated project's id. That gap is real:
/// `ProjectSection.name` (`src/config/project.rs:26`) is a bare, unvalidated
/// `String`, and `ActiveProject::project_id()` (`src/agent/mod.rs:311-313`)
/// returns it verbatim, so a project hand-named literally `"codescout@wt"`
/// produces an id byte-identical to this function's output for
/// `("codescout", "wt")` — same file AND same column value, a genuine row
/// merge. This is not a new hole: it is the exact exposure `lib:{name}` ids
/// already carry today (a project could be named `"lib:foo"` to collide
/// with library `foo`), and is accepted on the same basis — no charset
/// validation exists on project names, and this function does not add one.
pub fn delta_project_id(main_project_id: &str, worktree_dir: &str) -> String {
    format!("{main_project_id}@{worktree_dir}")
}

/// The key a worktree's delta project id is built on.
///
/// **Git's worktree name first, the directory basename only as a fallback.**
///
/// The basename alone is not unique: `/a/wt` and `/b/wt` can be two different
/// worktrees of the same repository, and keying on `file_name()` collapsed
/// them onto one `delta_project_id`. The consequence is the worst of the known
/// double-serves and is entirely silent -- B's sync deletes A's chunks via the
/// prune (they are not in B's local id set), then A's query serves main's
/// `fileB` *and* B's `fileB`: the same path twice, one copy from another
/// branch. `classify_worktree_index_state` reports `Healthy`, no warning and no
/// drift note fire, and A's own `fileA` is served by nothing at all.
///
/// [`crate::prompts::detect_worktree_info`] already parses
/// `gitdir: <main>/.git/worktrees/<NAME>`, and git guarantees `<NAME>` is
/// unique per repository, so it is the correct key. The basename fallback
/// covers a root that is not a linked worktree (a plain directory, or a `.git`
/// pointer whose shape does not parse) -- there is no git name to use there,
/// and the previous behaviour is what remains.
///
/// This is the ONE place the decision is made. Both sides go through it:
/// `sync_worktree` (producer) and [`worktree_ids`] (consumer). A divergence
/// makes the delta invisible with no error -- see [`delta_project_id`].
///
/// Costs one small `.git` read per call. `semantic_search` calls
/// `detect_worktree_info` itself moments earlier and this repeats that read
/// rather than threading the value through, deliberately: a parameter here is
/// one more thing the two sides could pass differently, which is exactly the
/// failure mode above.
fn worktree_key(worktree_root: &Path) -> String {
    crate::prompts::detect_worktree_info(worktree_root)
        .and_then(|info| info.name)
        .or_else(|| {
            worktree_root
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| "worktree".to_string())
}

/// `(main_project_id, delta_id)` for a linked worktree, derived from the two
/// real directories involved: `main_repo` (read via
/// [`crate::config::project::ProjectConfig::load_or_default`], which already
/// falls back to `main_repo`'s own basename when no `project.toml` exists --
/// see that function's doc comment) and `worktree_root` (via
/// [`worktree_key`], which prefers git's own unique worktree name over the
/// directory basename).
///
/// This is the single derivation both the producer (`index`'s `sync_worktree`
/// call site) and the consumer (`semantic_search`'s worktree query branch)
/// must agree on -- collapses what used to be three independent copies
/// (`sync.rs`, `index.rs`, `semantic_search.rs`) into one. `sync_worktree`
/// itself cannot call this directly (it receives an already-resolved
/// `main_project_id: &str`, not a `main_repo: &Path`), so it shares only the
/// [`worktree_key`] half.
pub fn worktree_ids(main_repo: &Path, worktree_root: &Path) -> (String, String) {
    let main_project_id = crate::config::project::ProjectConfig::load_or_default(main_repo)
        .map(|c| c.project.name)
        .unwrap_or_else(|_| {
            main_repo
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "main".to_string())
        });
    let wt_dir = worktree_key(worktree_root);
    let delta_id = delta_project_id(&main_project_id, &wt_dir);
    (main_project_id, delta_id)
}

/// Embed `pending`'s chunk content and upsert it, then clear `pending` so the
/// content + embeddings are dropped — keeping peak memory at O(flush_batch).
///
/// **A batch failure is not fatal, and must not be.** `POST /v1/embeddings`
/// rejects the WHOLE request when a single member exceeds the model's context
/// ceiling (`exceed_context_size_error`), so one oversized chunk used to fail
/// its entire flush batch and then abort the walk through `?` at both
/// `stream_index` call sites. Batches already flushed stay committed, so the
/// result was a durably truncated index that `index(action="status")` reports as
/// `indexed: true, queryable: true` — its only check being a non-zero chunk
/// count. That is the confirmed mechanism behind an index missing an entire
/// top-level directory:
/// `docs/issues/archive/2026-08-26-index-status-claims-complete-without-checking-coverage.md`
/// and `docs/issues/archive/2026-08-26-dense-embedder-slot-context-drops-large-embeds.md`.
///
/// So on a batch error we retry chunk-by-chunk to isolate the offenders, store
/// everything that does embed, and report the rest through `skipped` rather than
/// aborting. A skipped chunk's id still goes into the caller's `local_ids`, so a
/// previously-good server vector for that chunk is never pruned merely because
/// this run could not re-embed it.
async fn flush_pending(
    embedder: &dyn crate::retrieval::embedder::BatchEmbedder,
    store: &dyn crate::retrieval::code_store::CodeVectorStore,
    collection: &str,
    pending: &mut Vec<crate::retrieval::payload::CodePayload>,
    skipped: &mut Vec<String>,
) -> Result<usize> {
    use crate::retrieval::embedder::EmbedOutput;
    use crate::retrieval::payload::{embed_text, CodePayload};
    if pending.is_empty() {
        return Ok(0);
    }
    // What a chunk looks like to the embedder is `embed_text`'s decision, not this
    // function's. Reading `p.content` inline here is what silently dropped the AST
    // header when its previous consumer was deleted.
    let texts: Vec<String> = pending.iter().map(embed_text).collect();
    let batch_err = match embedder.embed_batch_dyn(&texts).await {
        Ok(embeds) => {
            let n = pending.len();
            let chunks: Vec<(CodePayload, EmbedOutput)> = pending.drain(..).zip(embeds).collect();
            store.upsert_chunks(collection, &chunks).await?;
            return Ok(n);
        }
        Err(e) => e,
    };

    // Isolation pass: one request per chunk, so a single unembeddable payload
    // costs only itself.
    let mut good: Vec<(CodePayload, EmbedOutput)> = Vec::new();
    let mut failed: Vec<String> = Vec::new();
    for p in pending.drain(..) {
        let text = embed_text(&p);
        let label = format!("{}:{}", p.file_path, p.start_line);
        match embedder.embed_batch_dyn(std::slice::from_ref(&text)).await {
            Ok(mut embeds) if !embeds.is_empty() => good.push((p, embeds.remove(0))),
            // Both are unusable — an `Ok` carrying no vector would let a zip silently
            // drop the payload — but they are recorded DISTINCTLY and with their cause.
            // The first version of this collapsed them into a bare `file:line`, so the
            // report said which chunks were skipped and never why; "too large, re-chunk
            // it" and "embedder down, retry" call for opposite responses, and an empty
            // `Ok` is a backend contract violation rather than either.
            Ok(_) => failed.push(format!("{label} — embedder returned no vector")),
            Err(e) => {
                let mut why = format!("{e:#}");
                // Bounded: one entry per skipped chunk lands in `SyncReport.skipped`.
                if why.chars().count() > 200 {
                    why = why.chars().take(200).collect::<String>() + "…";
                }
                failed.push(format!("{label} — {why}"));
            }
        }
    }

    // Every chunk failed on its own. That is either an unhealthy embedder or a
    // batch of uniformly oversized chunks, and the two need OPPOSITE handling —
    // abort vs skip-and-continue. "All failed" cannot by itself mean "abort",
    // because a lone oversized chunk in the tail flush is the common case and has
    // to survive. Distinguish them empirically with a minimal probe rather than by
    // pattern-matching error strings, which differ per backend and per version.
    if good.is_empty()
        && embedder
            .embed_batch_dyn(std::slice::from_ref(&"ok".to_string()))
            .await
            .is_err()
    {
        return Err(batch_err.context(
            "embedder rejected a minimal probe after every chunk in the batch failed — \
             treating it as unhealthy and aborting, rather than skipping real content",
        ));
    }

    tracing::warn!(
        skipped = failed.len(),
        stored = good.len(),
        cause = %batch_err,
        "embed batch failed; stored the chunks that embed individually and skipped the rest"
    );
    skipped.extend(failed);
    let n = good.len();
    if !good.is_empty() {
        store.upsert_chunks(collection, &good).await?;
    }
    Ok(n)
}

/// Whether a walk entry is tool state the code index must never descend into.
///
/// Directory-only by design: a *file* named `.git` is a worktree pointer and a
/// file named `.codescout` is just a file — neither is a state tree, and neither
/// should be skipped on the strength of its name alone.
pub(crate) fn is_always_skipped(name: &str, is_dir: bool) -> bool {
    is_dir && ALWAYS_SKIP_DIRS.contains(&name)
}

/// Enumerate every file the code indexer considers indexable under `root`:
/// walked with `ALWAYS_SKIP_DIRS` pruned, `ignore_patterns` applied, and
/// gated on `lang_for_ext` recognising the extension. Returns
/// `(absolute_path, language, forward-slashed project-relative path)`
/// triples; reading file content is left to the caller, since callers read
/// it at different times (`stream_index` always reads immediately;
/// `sync_worktree`'s second pass reads only for files a prior decision
/// already marked dirty).
///
/// Single source of truth for "which files does the code index see" --
/// `stream_index`, `sync_worktree` and `index(action="verify")` all call this
/// (the second once, iterating the collected result twice, not walking twice)
/// so a change to the walk predicate (a new `ALWAYS_SKIP_DIRS` entry, changed
/// ignore semantics, a new `lang_for_ext` extension) can never apply to one
/// walk and not another.
///
/// The verify caller ([`verify_index_coverage`]) lives in this module for that
/// reason — so it can use this walk directly while the function stays private.
/// It inherits the obligation above with a twist: a coverage check that
/// reimplemented the walk would measure its own reimplementation and report a
/// healthy index as broken (or the reverse) with nothing able to say which. It
/// must call THIS function or it is not a coverage check. `dirty_paths`' entire correctness rests on main's chunk
/// set (produced by this walk when main was last synced) and the worktree's
/// chunk set (produced by this SAME walk under `sync_worktree`) agreeing on
/// what a "file" even is -- a silent divergence would make files appear
/// locally that main "doesn't have", permanently dirtying and re-embedding
/// them into every delta forever, with no test able to see it (both walks
/// would still agree with themselves).
fn indexable_files(
    root: &Path,
    ignore_patterns: &[String],
) -> Vec<(PathBuf, &'static str, String)> {
    let mut out = Vec::new();
    let ignore_matcher = crate::embed::build_ignore_matcher(root, ignore_patterns);
    for entry in ignore::WalkBuilder::new(root)
        // Index tracked dotfiles (`.github/`, `.cargo/config.toml`), which means
        // turning off the crate's hidden-entry filter -- and that filter is what
        // normally keeps `.git/` out of a walk. `.gitignore` does not cover it:
        // git has no reason to ignore its own directory. So the denylist below is
        // load-bearing, not belt-and-braces.
        .hidden(false)
        .filter_entry(move |e| {
            let is_dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
            if e.file_name()
                .to_str()
                .is_some_and(|n| is_always_skipped(n, is_dir))
            {
                return false;
            }
            !ignore_matcher.matched(e.path(), is_dir).is_ignore()
        })
        .build()
        .filter_map(|e| e.ok())
    {
        let Some(ft) = entry.file_type() else {
            continue;
        };
        if !ft.is_file() {
            continue;
        }
        let path = entry.path();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let Some(lang) = crate::embed::lang_for_ext(ext) else {
            continue;
        };
        let rel_path = path.strip_prefix(root).unwrap_or(path);
        let rel_display = to_forward_slash(rel_path);
        // An empty file_path is never a real path -- Task 2's `dirty_paths`
        // treats it as "unknown", which would silently drop the chunk out of
        // consideration rather than surfacing anything. Skip defensively.
        if rel_display.is_empty() {
            continue;
        }
        out.push((path.to_path_buf(), lang, rel_display));
    }
    out
}

/// What an integrity check found. Counts are exact; the `*_sample` lists are capped,
/// because a genuinely broken index can be missing tens of thousands of files and the
/// report has to stay readable.
#[derive(Debug, Default)]
pub(crate) struct IndexIntegrity {
    /// Files the walk says belong in the index.
    pub expected_files: usize,
    /// Distinct `file_path`s the store actually holds.
    pub stored_files: usize,
    /// Eligible on disk, absent from the store.
    pub missing_count: usize,
    pub missing_sample: Vec<String>,
    /// In the store, not eligible on disk — deleted, renamed, or newly ignored.
    pub orphan_count: usize,
    pub orphan_sample: Vec<String>,
    /// Top-level directories where the walk found files and the store holds NONE.
    /// This is the invariant the original report asked for as a minimum bar.
    pub empty_eligible_dirs: Vec<String>,
    /// Chunk rows with no vector. Backend-dependent whether this can be non-zero.
    pub chunks_without_vectors: usize,
}

/// Cap on every sample list. Counts stay exact.
const INTEGRITY_SAMPLE: usize = 20;

/// First path component of a forward-slashed project-relative path, or `<root>` for a
/// file sitting directly in the project root.
///
/// `<root>` rather than the filename: grouping a top-level file under its own name
/// would make every deleted root file look like an "empty eligible directory", which
/// is the check's loudest signal and must not fire on a one-file change.
fn top_level_of(rel_path: &str) -> &str {
    match rel_path.split_once('/') {
        Some((head, _)) => head,
        None => "<root>",
    }
}

/// Compare what the walk says belongs in the index against what the store holds.
///
/// Deliberately uses [`indexable_files`] — the same walk `stream_index` uses — rather
/// than re-deriving eligibility. A coverage check built on its own walk measures its
/// own reimplementation: it would report a healthy index as broken, or a broken one as
/// healthy, with nothing able to say which. `ignore_patterns` must likewise be the
/// caller's RESOLVED config, not a re-read of the file — `scripts/` in this repo sits
/// at 15 indexed of 19 tracked and is entirely correct, because two entries are in
/// `[ignored_paths]` and two are extensions outside `languages`.
///
/// Read-only, and that is load-bearing: a negative result must never authorise a
/// deletion. A bad walk that reported every file as an orphan would, if this pruned,
/// delete a live index. Reporting leaves the repair to `index(action="build")`, whose
/// prune runs against a walk it performed itself.
pub(crate) async fn verify_index_coverage(
    root: &Path,
    project_id: &str,
    collection: &str,
    store: &dyn crate::retrieval::code_store::CodeVectorStore,
    ignore_patterns: &[String],
) -> Result<IndexIntegrity> {
    use std::collections::{BTreeMap, BTreeSet};

    let expected: BTreeSet<String> = indexable_files(root, ignore_patterns)
        .into_iter()
        .map(|(_, _, rel)| rel)
        .collect();

    let stored: BTreeSet<String> = store
        .chunk_refs(collection, project_id)
        .await?
        .into_iter()
        .map(|c| c.file_path)
        .collect();

    let missing: Vec<String> = expected.difference(&stored).cloned().collect();
    let orphans: Vec<String> = stored.difference(&expected).cloned().collect();

    // Per-top-level-directory coverage. Only a directory the walk found files in can
    // be "empty" — a directory absent from both sides is simply not part of the
    // project and must not appear.
    let mut per_dir: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    for p in &expected {
        per_dir.entry(top_level_of(p)).or_default().0 += 1;
    }
    for p in &stored {
        per_dir.entry(top_level_of(p)).or_default().1 += 1;
    }
    let empty_eligible_dirs: Vec<String> = per_dir
        .iter()
        .filter(|(_, (exp, got))| *exp > 0 && *got == 0)
        .map(|(d, _)| (*d).to_string())
        .collect();

    Ok(IndexIntegrity {
        expected_files: expected.len(),
        stored_files: stored.len(),
        missing_count: missing.len(),
        missing_sample: missing.into_iter().take(INTEGRITY_SAMPLE).collect(),
        orphan_count: orphans.len(),
        orphan_sample: orphans.into_iter().take(INTEGRITY_SAMPLE).collect(),
        empty_eligible_dirs,
        chunks_without_vectors: store
            .count_chunks_without_vectors(collection, project_id)
            .await?,
    })
}

/// Walk `root`, diff against `server` chunk refs, and embed+upsert changed chunks
/// in bounded batches so peak memory is O(flush_batch), not O(all_files).
///
/// Split out of [`RetrievalClient::sync_project`] both as a test seam (driven by
/// `&dyn BatchEmbedder` + `&dyn CodeVectorStore`) and to bound the index pass: the
/// previous whole-tree materialisation grew to 68 GB and OOM-killed the host
/// (docs/issues/archive/2026-06-19-mcp-server-oom-68gb.md). `chunk_id` encodes the content
/// hash, so the delete-set needs only the cheap id sets — never the chunk content.
///
/// Also the single caller of `indexable_files` on the main-project sync path --
/// `sync_worktree` is the other, sharing the exact same walk. See that helper's
/// doc comment for why the two must never diverge.
///
/// Returns `(added, deleted)`.
#[allow(clippy::too_many_arguments)]
async fn stream_index(
    root: &Path,
    project_id: &str,
    collection: &str,
    server: &[crate::retrieval::drift::ChunkRef],
    embedder: &dyn crate::retrieval::embedder::BatchEmbedder,
    store: &dyn crate::retrieval::code_store::CodeVectorStore,
    force_reindex: bool,
    chunk_target: usize,
    flush_batch: usize,
    ignore_patterns: &[String],
) -> Result<(usize, usize, Vec<String>)> {
    use crate::embed::ast_chunker::split_file;
    use crate::retrieval::payload::CodePayload;
    use std::collections::HashSet;

    let server_ids: HashSet<&str> = server.iter().map(|c| c.chunk_id.as_str()).collect();
    let mut local_ids: HashSet<String> = HashSet::new();
    let mut pending: Vec<CodePayload> = Vec::new();
    let mut added = 0usize;
    // Chunks that could not be embedded. Reported rather than fatal — see
    // `flush_pending`'s doc comment for why aborting here truncated the index.
    let mut skipped: Vec<String> = Vec::new();

    for (path, lang, rel_display) in indexable_files(root, ignore_patterns) {
        let source = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(_) => continue,
        };
        // The header this produces is now embedded, so it has to be
        // checkout-independent and separator-stable: hand the chunker the same
        // forward-slashed relative path the payload stores, never the absolute one.
        for c in split_file(&source, lang, Path::new(&rel_display), chunk_target) {
            // Skip empty/whitespace-only chunks — embedders reject empty inputs.
            if c.content.trim().is_empty() {
                continue;
            }
            let hash = content_hash(&c.content);
            let chunk_id = chunk_id(project_id, Path::new(&rel_display), c.start_line, &hash);
            // Every local chunk id participates in the delete-set diff, even when
            // it is already indexed and skipped for re-embedding.
            local_ids.insert(chunk_id.clone());
            // chunk_id encodes the content hash, so a content change yields a new
            // id; skip re-embedding ids the server already has unless force_reindex.
            if !force_reindex && server_ids.contains(chunk_id.as_str()) {
                continue;
            }
            pending.push(CodePayload {
                project_id: project_id.into(),
                file_path: rel_display.clone(),
                language: lang.into(),
                start_line: c.start_line as i64,
                end_line: c.end_line as i64,
                // The chunker's identity line for this chunk, empty for non-AST
                // languages. `embed_text` prepends it; discarding it here is what
                // made it unreachable for four hundred thousand chunks.
                ast_header: c.metadata.unwrap_or_default(),
                content: c.content,
                content_hash: hash,
                last_indexed_commit: String::new(),
                chunk_id,
            });
            // Flush when the buffer fills so peak memory stays O(flush_batch), not
            // O(all_files) — the whole-tree materialisation grew to 68 GB and
            // OOM-killed the host (docs/issues/archive/2026-06-19-mcp-server-oom-68gb.md).
            if pending.len() >= flush_batch {
                added +=
                    flush_pending(embedder, store, collection, &mut pending, &mut skipped).await?;
            }
        }
    }
    // Flush the tail.
    if !pending.is_empty() {
        added += flush_pending(embedder, store, collection, &mut pending, &mut skipped).await?;
    }

    // Delete server chunks that are no longer present locally.
    let to_delete: Vec<String> = server
        .iter()
        .filter(|c| !local_ids.contains(c.chunk_id.as_str()))
        .map(|c| c.chunk_id.clone())
        .collect();
    let deleted = to_delete.len();
    if !to_delete.is_empty() {
        store
            .delete_chunks(collection, project_id, &to_delete)
            .await?;
    }

    Ok((added, deleted, skipped))
}

/// Sync a linked worktree: reuse main's vectors for byte-identical files, embed
/// only what differs under the worktree's delta project id, and record the paths
/// main must not be asked for.
///
/// Called only from the `index` tool path -- `semantic_search` must never call
/// this. A read tool that writes has no intent gate and surfaces embedder
/// failures under the wrong operation.
///
/// Serializes on the DELTA project id (`delta_project_id(main_project_id, ...)`),
/// not `main_project_id`: this function only READS main's chunks and MUTATES the
/// delta's, so the delta is the resource a second concurrent run of THIS function
/// must be locked against -- mirrors `sync_project`'s own invariant of acquiring
/// its index lock before the `chunk_refs` baseline read that follows, because the
/// indexing that follows then mutates what that read established. Without this,
/// two overlapping worktree syncs of the SAME delta would each diff `to_delete`
/// against a `delta_refs` snapshot the other is invalidating -- run A could
/// delete chunks run B just wrote.
///
/// Uses one walk (`indexable_files`), collected once and iterated twice, never a
/// second independent walk -- see that helper's doc comment for why a second,
/// independently-written walk here is the exact hazard the worktree design
/// depends on not existing.
///
/// - Pass 1 hashes every chunk into a cheap [`crate::retrieval::drift::LocalChunk`]
///   (file_path + hash; each chunk's content is read to hash it, then dropped
///   immediately) so [`crate::retrieval::drift::dirty_paths`] gets one
///   authoritative view of the whole tree. The dirty/clean decision itself lives
///   entirely in `dirty_paths`, not here.
/// - Pass 2 re-visits the same file list and materialises full chunk content only
///   for files `dirty_paths` marked dirty, flushed through the same bounded-batch
///   [`flush_pending`] path `stream_index` uses. `force_reindex` gates the skip
///   for chunks the delta already has, mirroring `stream_index`'s own
///   `force_reindex` escape hatch -- without it, a delta with bad embeddings
///   (model change, dimension migration, a half-written vector) has no way to be
///   rebuilt: every chunk id would still match and `force=true` would silently
///   re-embed nothing.
///
/// A single-pass design that keeps every chunk's content in memory until the
/// dirty decision is known would reproduce the exact whole-tree materialisation
/// that OOM-killed the host at 68 GB (docs/issues/archive/2026-06-19-mcp-server-oom-68gb.md)
/// -- the worktree delta's entire premise is that embedding cost is proportional
/// to the diff, and that guarantee is worthless if peak memory still scales with
/// the corpus.
///
/// Assumes `main_project_id`'s collection already exists (main has been indexed
/// at least once) -- `ensure_collection` is deliberately not called here.
#[allow(clippy::too_many_arguments)]
pub async fn sync_worktree(
    store: &dyn crate::retrieval::code_store::CodeVectorStore,
    worktree_root: &Path,
    main_project_id: &str,
    collection: &str,
    embedder: &dyn crate::retrieval::embedder::BatchEmbedder,
    force_reindex: bool,
    ignore_patterns: &[String],
    index_lock_dir: Option<&Path>,
    // Provenance of the syncing process. `None` — the production caller — snapshots
    // the live process via `index_state::current_writer()`. Injected by tests, which
    // cannot make this binary's own `/proc/self/exe` read "deleted" without unlinking
    // the running test binary. Same seam as `SyncOpts::writer`.
    writer: Option<crate::retrieval::index_state::WriterProvenance>,
) -> Result<SyncReport> {
    use crate::embed::ast_chunker::split_file;
    use crate::retrieval::drift::{dirty_paths, LocalChunk};
    use crate::retrieval::payload::CodePayload;
    use std::collections::HashSet;

    // Refuse before ANY work — ahead of the index lock, the dirty-set sidecar and the
    // embed pass. This call site writes its sidecar BEFORE embedding and `sync_project`
    // writes it after, which is precisely why the refusal cannot live at the writer:
    // no single rule there is correct for both. See `guard_stale_binary`.
    guard_stale_binary(
        writer
            .as_ref()
            .map(|w| w.exe_deleted)
            .unwrap_or_else(ambient_exe_deleted),
    )?;

    const STACK_CHUNK_TARGET: usize = 1200;
    let chunk_target: usize = std::env::var("CODESCOUT_CHUNK_TARGET")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(STACK_CHUNK_TARGET);
    const DEFAULT_FLUSH_BATCH: usize = 256;
    let flush_batch: usize = std::env::var("CODESCOUT_INDEX_FLUSH_BATCH")
        .ok()
        .and_then(|s| s.parse().ok())
        .filter(|&n| n > 0)
        .unwrap_or(DEFAULT_FLUSH_BATCH);

    let started = std::time::Instant::now();

    // `worktree_dir` MUST be a single segment, never a path, and MUST be the
    // same segment the query path derives -- if the two disagree the delta
    // becomes invisible with no error (see `delta_project_id`'s doc comment).
    // Shares `worktree_key` with `worktree_ids` so this rule lives in exactly
    // one place; that function is also what makes the key git's unique
    // worktree name rather than the ambiguous directory basename.
    let wt_dir = worktree_key(worktree_root);
    let delta_id = delta_project_id(main_project_id, &wt_dir);

    // Serialize delta syncs for THIS worktree -- see the doc comment above for
    // why this locks on `delta_id` rather than `main_project_id`. MUST be
    // acquired before the `chunk_refs` baseline reads directly below, for the
    // same reason `sync_project` acquires its own lock before its equivalent
    // read: those reads establish the drift baseline that the rest of this
    // function then mutates.
    let _index_lock = match index_lock_dir {
        Some(dir) => crate::retrieval::index_lock::acquire_in(dir, &delta_id)?,
        None => crate::retrieval::index_lock::acquire(&delta_id)?,
    };

    // I2: these two reads are the drift baseline, and `unwrap_or_default()` on
    // them turned a transient store error into a *factual claim* -- "main holds
    // nothing" -- which `dirty_paths` then reads as "every file in the worktree
    // is dirty". The cost of that lie is not a warning: it is a full re-embed of
    // the entire corpus, and (composed with C2's filter) every subsequent query
    // shipping every path in the repo as an exclusion. Propagate instead; an
    // *empty* baseline is still `Ok(vec![])` and still means what it says, so
    // the genuinely-unindexed-main case is untouched.
    //
    // `sync_worktree` deliberately does not call `ensure_collection` (see this
    // function's doc comment); with the error swallowed, a missing collection
    // used to surface much later as an upsert failure with no mention of the
    // real cause.
    let main_refs = store
        .chunk_refs(collection, main_project_id)
        .await
        .with_context(|| {
            format!(
                "reading main project `{main_project_id}`'s chunk refs to compute this \
                 worktree's dirty set (has the main checkout been indexed?)"
            )
        })?;
    let delta_refs = store
        .chunk_refs(collection, &delta_id)
        .await
        .with_context(|| format!("reading worktree delta `{delta_id}`'s existing chunk refs"))?;
    let delta_ids_present: HashSet<&str> = delta_refs.iter().map(|c| c.chunk_id.as_str()).collect();

    // One walk, shared with `stream_index` via `indexable_files`. Collected
    // once and iterated twice below (hash pass, then dirty-content pass)
    // rather than walked twice, so the two passes cannot even in principle
    // disagree with each other on the file set within a single call.
    let files = indexable_files(worktree_root, ignore_patterns);

    // Pass 1: cheap hash-only pass over every file.
    let mut local: Vec<LocalChunk> = Vec::new();
    for (path, lang, rel_display) in &files {
        let Ok(source) = std::fs::read_to_string(path) else {
            continue;
        };
        for c in split_file(&source, lang, Path::new(rel_display), chunk_target) {
            if c.content.trim().is_empty() {
                continue;
            }
            local.push(LocalChunk {
                file_path: rel_display.clone(),
                content_hash: content_hash(&c.content),
            });
        }
    }

    // The one authoritative dirty/clean call -- see the doc comment above.
    let dirty = dirty_paths(&main_refs, &local);

    // I3: the sidecar is written BEFORE the first upsert, and the ordering is
    // the whole point.
    //
    // Every `flush_pending(...).await?` below is an early return sitting between
    // a committed upsert and this write. With the write at the end, an embedder
    // timeout or a store blip left delta chunks committed for freshly-edited
    // paths while the sidecar still listed the *previous* dirty set -- so main
    // was never told to exclude those paths and served its stale copy alongside
    // the delta's new one. That is a double-serve of the files the user just
    // edited: the common case, not a corner.
    //
    // Written first, the same failure leaves the sidecar naming paths the delta
    // does not hold yet. Main excludes them, the delta has nothing for them, and
    // they return no results until the next `index(action="build")` -- an
    // under-serve. Both directions are wrong; only one of them shows the user
    // content from a branch they are not on.
    //
    // A residual window remains and is not closed here: a path that went dirty
    // -> clean since the last sync is absent from the new sidecar (so main
    // serves it) while its old delta chunks survive until the prune below,
    // which an early return also skips. That needs the user to have reverted a
    // file to main's exact bytes, and it is strictly rarer than the edit case
    // this ordering fixes.
    //
    // MUST go through `write_index_state_with_dirty`, never plain
    // `write_index_state` -- see the routing guard in `sync_project`.
    //
    // A hard error, not the fail-soft warning this used to be: nothing has been
    // committed to the store at this point, so failing here leaves the index
    // exactly as it was rather than half-updated with no record of it.
    let dirty_vec: Vec<String> = dirty.paths.iter().cloned().collect();
    // `Preserve`, not `Record`: this function has no config to read a model from, and
    // by its own design it is not where a model change lands — see the `dim_migration:
    // None` note at the end of this function, and the fact that it deliberately does not
    // call `ensure_collection`. Stamping a model here would let a worktree delta
    // overwrite main's record of what built the index.
    // `&[]` for `skipped`, not "nothing was skipped": this write happens BEFORE the
    // embed pass below runs (see the I3 ordering comment above), by design, so this
    // run's own skip count is not yet knowable at this point in the flow. A worktree
    // delta sync's `last_sync_skipped` therefore never reflects the run that follows
    // this write -- a known gap, not a claim of cleanliness. Closing it would mean a
    // second sidecar write after the embed pass, which needs its own reasoning about
    // ordering against the same early-return hazard this comment block documents, and
    // is left for a follow-up rather than folded in here.
    crate::retrieval::index_state::write_index_state_with_dirty(
        worktree_root,
        &dirty_vec,
        crate::retrieval::index_state::ModelStamp::Preserve,
        &[],
    )
    .with_context(|| {
        format!(
            "recording this worktree's dirty set ({} paths) before upserting the \
                 delta -- refusing to index without it, since main would keep serving \
                 stale chunks for every path listed",
            dirty_vec.len()
        )
    })?;

    // Pass 2: re-visit the same file list, materialising full chunk content
    // only for files `dirty_paths` marked dirty.
    let mut pending: Vec<CodePayload> = Vec::new();
    let mut local_delta_ids: HashSet<String> = HashSet::new();
    let mut added = 0usize;
    let mut skipped: Vec<String> = Vec::new();
    for (path, lang, rel_display) in &files {
        if !dirty.paths.contains(rel_display) {
            continue;
        }
        let Ok(source) = std::fs::read_to_string(path) else {
            continue;
        };
        for c in split_file(&source, lang, Path::new(rel_display), chunk_target) {
            if c.content.trim().is_empty() {
                continue;
            }
            let hash = content_hash(&c.content);
            let did = chunk_id(&delta_id, Path::new(rel_display), c.start_line, &hash);
            // Every dirty local chunk id participates in the delta's own
            // delete-set diff below, even when it's already indexed and
            // skipped for re-embedding.
            local_delta_ids.insert(did.clone());
            // The delta already has this exact content -- skip re-embedding,
            // mirroring stream_index's own incremental skip. `force_reindex`
            // bypasses the skip, mirroring stream_index's own escape hatch --
            // without it there is no way to rebuild a delta whose embeddings
            // are bad, since every chunk id still matches.
            if !force_reindex && delta_ids_present.contains(did.as_str()) {
                continue;
            }
            pending.push(CodePayload {
                project_id: delta_id.clone(),
                file_path: rel_display.clone(),
                language: (*lang).into(),
                start_line: c.start_line as i64,
                end_line: c.end_line as i64,
                ast_header: c.metadata.unwrap_or_default(),
                content: c.content,
                content_hash: hash,
                last_indexed_commit: String::new(),
                chunk_id: did,
            });
            if pending.len() >= flush_batch {
                added +=
                    flush_pending(embedder, store, collection, &mut pending, &mut skipped).await?;
            }
        }
    }
    if !pending.is_empty() {
        added += flush_pending(embedder, store, collection, &mut pending, &mut skipped).await?;
    }

    // Prune delta chunks that are no longer part of the current dirty set (a
    // file went from dirty back to clean, or a dirty file's chunk boundaries
    // shifted).
    let to_delete: Vec<String> = delta_refs
        .iter()
        .filter(|c| !local_delta_ids.contains(c.chunk_id.as_str()))
        .map(|c| c.chunk_id.clone())
        .collect();
    let deleted = to_delete.len();
    if !to_delete.is_empty() {
        store
            .delete_chunks(collection, &delta_id, &to_delete)
            .await?;
    }

    Ok(SyncReport {
        added,
        deleted,
        updated: 0,
        elapsed_ms: started.elapsed().as_millis(),
        skipped,
        // A worktree delta is never the surface a model migration happens on —
        // `sync_worktree` deliberately does not call `ensure_collection` either.
        dim_migration: None,
    })
}

impl crate::retrieval::client::RetrievalClient {
    pub async fn sync_project(
        &self,
        project_id: &str,
        root: &Path,
        opts: SyncOpts,
    ) -> Result<SyncReport> {
        // Refuse before ANY work — ahead of the index lock, the store, and the embed
        // pass. A stale process must decline to re-index, not decline to record what
        // it already did; see `guard_stale_binary` for why the placement is the whole
        // point. `None` writer = production, which snapshots the live process.
        guard_stale_binary(
            opts.writer
                .as_ref()
                .map(|w| w.exe_deleted)
                .unwrap_or_else(ambient_exe_deleted),
        )?;

        // chunk=1200 was the universal sweet spot in the Phase 5.5 chunk×model matrix
        // (see docs/research/2026-05-06-retrieval-stack-benchmark.md). Override with
        // CODESCOUT_CHUNK_TARGET when retuning.
        const STACK_CHUNK_TARGET: usize = 1200;
        let chunk_target: usize = std::env::var("CODESCOUT_CHUNK_TARGET")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(STACK_CHUNK_TARGET);
        // Flush the embed/upsert buffer every FLUSH_BATCH chunks so peak memory is
        // O(batch), not O(all_files). The previous whole-tree materialisation here
        // grew to 68 GB and OOM-killed the host
        // (docs/issues/archive/2026-06-19-mcp-server-oom-68gb.md).
        const DEFAULT_FLUSH_BATCH: usize = 256;
        let flush_batch: usize = std::env::var("CODESCOUT_INDEX_FLUSH_BATCH")
            .ok()
            .and_then(|s| s.parse().ok())
            .filter(|&n| n > 0)
            .unwrap_or(DEFAULT_FLUSH_BATCH);
        // `backend` and `sparse` are logged because their being WRONG is silent otherwise.
        // A plain `cargo build --release` omits the `server-stack` feature, so
        // `VectorBackend::resolve()` defaults to sqlite-vec, which sets `lite` -> `dense_only`
        // -> the sparse leg is skipped AND the writes go to `.codescout/embeddings/project.db`
        // instead of Qdrant. Measured 2026-08-07: a `--force` rebuild ran seven minutes
        // hammering the dense embedder with zero sparse requests before anyone noticed, and the
        // only reason it was caught is that someone compared container logs. Use `cargo rb`
        // (aliased to `--features server-stack`) for the hybrid stack; these two fields make the
        // difference visible in line one instead of two hours later.
        tracing::info!(
            chunk_target,
            flush_batch,
            force_reindex = opts.force_reindex,
            backend = if self.lite { "sqlite-vec" } else { "qdrant" },
            sparse = if self.lite || self.config.disable_sparse {
                "SKIPPED"
            } else {
                "on"
            },
            "retrieval sync starting"
        );

        // Serialize index passes per project. MUST be acquired before the
        // `chunk_refs` call below: that read establishes the drift baseline, and
        // `stream_index` then mutates it. Two overlapping runs would each diff
        // against a snapshot the other is invalidating.
        //
        // Bound to `_index_lock` (not `_`) so it lives until the end of this
        // function — `let _ = ...` would drop it immediately and release the lock.
        // Guarded by `sync_project_holds_index_lock_for_its_full_duration`.
        let _index_lock = match opts.index_lock_dir.as_deref() {
            Some(dir) => crate::retrieval::index_lock::acquire_in(dir, project_id)?,
            None => crate::retrieval::index_lock::acquire(project_id)?,
        };

        let started = std::time::Instant::now();
        let collection = self.config.collection("code_chunks");
        // force=false delegates to `guard_index_dim` unchanged; force=true treats a
        // dimension mismatch as the reason for the rebuild and migrates instead of
        // erroring. Previously this was a bare guard call, which is why force=true
        // could not perform the one reindex that genuinely needs one.
        let dim_migration = self
            .migrate_or_guard_index_dim(&collection, project_id, opts.force_reindex)
            .await?;
        self.code_store
            .ensure_collection(
                &collection,
                self.effective_model_dim(crate::retrieval::config::DEFAULT_MODEL_DIM),
            )
            .await?;

        // Fetch existing chunk refs (id + hash only — bounded) for drift diffing.
        let server = self
            .code_store
            .chunk_refs(&collection, project_id)
            .await
            .unwrap_or_default();

        let (added, deleted, skipped) = stream_index(
            root,
            project_id,
            &collection,
            &server,
            &*self.embedder,
            self.code_store.as_ref(),
            opts.force_reindex,
            chunk_target,
            flush_batch,
            &opts.ignore_patterns,
        )
        .await?;

        let elapsed_ms = started.elapsed().as_millis();
        tracing::info!(
            added,
            deleted,
            skipped = skipped.len(),
            elapsed_ms,
            "retrieval sync finished"
        );

        // Record the indexed HEAD for external-change freshness detection
        // (checkout/pull/HEAD move). Gated to *project* syncs — library syncs
        // leave record_index_state false so library checkouts aren't polluted.
        // Fail-soft: a sidecar write must never break the sync.
        //
        // MUST NOT call the plain `write_index_state` helper: it delegates to
        // `write_index_state_with_dirty(root, &[])`, which would silently wipe a
        // worktree's recorded dirty set if this ordinary project-sync path is
        // ever invoked on a worktree's root -- e.g. the CLI binaries
        // (`src/bin/sync_project.rs`, `src/main.rs`) call `sync_project` directly
        // with no worktree awareness at all, and `sync_worktree` (the dedicated
        // worktree path) is the only thing that is *supposed* to populate this
        // list. Reading back whatever is currently on disk and re-writing it
        // unchanged makes this path a no-op for an ordinary project (which never
        // has a dirty set) and dirty-set-*preserving* for a worktree, regardless
        // of why this path got called on one.
        if opts.record_index_state {
            let existing_dirty = crate::retrieval::index_state::read_index_state(root)
                .map(|s| s.dirty_paths)
                .unwrap_or_default();
            // `Record`, because this is the path that just did the embedding: the
            // vectors now in the store were produced by `self.config.model`, and that
            // is the only fact `index(action="status")` can honestly report as
            // `indexed_with_model`. Reporting the CONFIGURED model under that name
            // would make a mismatch invisible by construction.
            if let Err(e) = crate::retrieval::index_state::write_index_state_with_dirty(
                root,
                &existing_dirty,
                crate::retrieval::index_state::ModelStamp::Record(&self.config.model),
                &skipped,
            ) {
                tracing::warn!(error = %e, "failed to write index-state sidecar");
            }
        }

        Ok(SyncReport {
            added,
            deleted,
            updated: 0,
            elapsed_ms,
            skipped,
            dim_migration,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::retrieval::client::RetrievalClient;
    use crate::retrieval::code_store::CodeVectorStore;
    use crate::retrieval::config::RetrievalConfig;
    use crate::retrieval::drift::ChunkRef;
    use crate::retrieval::embedder::{BatchEmbedder, CodeEmbedder, EmbedOutput, SparseVector};
    use crate::retrieval::payload::CodePayload;
    #[cfg(feature = "remote-embed")]
    use crate::retrieval::reranker::RerankerHttp;
    use crate::retrieval::search::Hit;
    use std::sync::{Arc, Mutex};

    #[test]
    fn always_skipped_covers_git_and_codescout_state_only_as_directories() {
        // `.hidden(false)` on the walker is what lets tracked dotfiles in, and it is
        // also what lets `.git/` in — `.gitignore` never lists it, so nothing else
        // stops the walk. This denylist is the only thing that does.
        assert!(is_always_skipped(".git", true));
        assert!(is_always_skipped(".codescout", true));

        // A FILE named `.git` is a worktree pointer, not a state tree. Skipping it by
        // name would be a different, and wrong, decision.
        assert!(!is_always_skipped(".git", false));
        assert!(!is_always_skipped(".codescout", false));

        // Deliberately absent: agent dirs that can hold real project documentation.
        // They belong in per-project `ignore_patterns` — a decision, not a default.
        assert!(!is_always_skipped(".claude", true));
        assert!(!is_always_skipped(".serena", true));
        assert!(!is_always_skipped(".github", true));

        // Whole-name match, not a prefix: a real directory must survive.
        assert!(!is_always_skipped(".gitlab-ci", true));
        assert!(!is_always_skipped("src", true));
    }

    #[test]
    fn chunk_id_normalizes_native_separators() {
        // BUG (docs/issues/archive/2026-07-07-display-audit-scope-gap-non-to-string-sites.md):
        // chunk_id was built with `rel_path.display()`, which renders a PathBuf's
        // internal string VERBATIM. rel_path is OS-derived (strip_prefix of a
        // filesystem-walk path), so on Windows it carries backslashes — persisting
        // native separators as the vector store's primary key, and disagreeing with
        // the `file_path` field of its own payload, which normalizes one line below.
        //
        // to_forward_slash is not cfg(windows)-gated, so a PathBuf built from a
        // literal backslash string reproduces the Windows shape on any host — the
        // same technique util/fs.rs's own tests use.
        let windows_shaped = std::path::PathBuf::from("src\\retrieval\\sync.rs");
        assert_eq!(
            chunk_id("proj", &windows_shaped, 12, "deadbeef"),
            "proj:src/retrieval/sync.rs:12:deadbeef",
            "the path component of a chunk id must be forward-slash normalized"
        );

        // Already-forward-slash input is untouched (the Linux/macOS path).
        let posix = std::path::PathBuf::from("src/retrieval/sync.rs");
        assert_eq!(
            chunk_id("proj", &posix, 12, "deadbeef"),
            "proj:src/retrieval/sync.rs:12:deadbeef"
        );
    }

    /// docs/issues/archive/2026-08-16-chunk-id-omits-index-so-duplicate-chunks-collapse.md
    ///
    /// Two chunks in the same file with byte-identical content must not collide —
    /// this is the load-bearing regression test: it fails on the pre-fix 3-tuple id.
    #[test]
    fn chunk_id_disambiguates_duplicate_content_in_the_same_file() {
        let path = std::path::PathBuf::from("src/lib.rs");
        let first = chunk_id("proj", &path, 10, "deadbeef");
        let second = chunk_id("proj", &path, 40, "deadbeef");
        assert_ne!(
            first, second,
            "same file, same content hash, different position — must not collide"
        );
    }

    #[test]
    fn delta_project_id_is_distinct_and_separator_is_not_a_colon() {
        // chunk_id joins on ':' (chunk_id() above), and sqlite_code_store.rs:234-238
        // documents a real regression from colon-bearing project ids. '@' keeps the
        // delta id unambiguous under that join.
        let id = delta_project_id("codescout", "peer-delegation");
        assert_eq!(id, "codescout@peer-delegation");
        assert!(!id.contains(':'), "delta id must not introduce a colon");
        assert_ne!(id, "codescout", "delta must not alias the main project");
    }

    #[test]
    fn worktree_ids_uses_the_worktree_basename_not_the_full_path() {
        // The FALLBACK arm of `worktree_key`: a plain directory with no `.git`
        // pointer has no git worktree name, so the basename is what remains.
        //
        // Regression target: `worktree_root.file_name()` mutated to
        // `worktree_root.to_string_lossy()` compiles and would smuggle the
        // FULL PATH into the delta id -- exactly the hazard `delta_project_id`'s
        // doc comment warns about, since `sync_worktree` (the producer) writes
        // under the basename-derived id and would then disagree with a
        // full-path-derived id computed here.
        let dir = tempfile::tempdir().unwrap();
        let main_repo = dir.path().join("main-project");
        std::fs::create_dir_all(&main_repo).unwrap();
        // Nested so `.to_string_lossy()` of the worktree root differs sharply
        // from its basename.
        let worktree_root = dir.path().join("nested").join("deeper").join("wt");
        std::fs::create_dir_all(&worktree_root).unwrap();

        let (main_id, delta_id) = worktree_ids(&main_repo, &worktree_root);
        assert_eq!(
            main_id, "main-project",
            "no project.toml -> falls back to main_repo's basename"
        );
        assert_eq!(
            delta_id, "main-project@wt",
            "delta id must use the worktree's BASENAME only"
        );
        assert!(
            !delta_id.contains("nested") && !delta_id.contains("deeper"),
            "delta id must not leak any parent path segment: {delta_id}"
        );
    }

    /// I1: two worktrees of the SAME repo sharing a directory basename under
    /// different parents must not collapse onto one delta project id.
    ///
    /// Keying on `file_name()` made `/a/wt` and `/b/wt` the same delta, and the
    /// failure is silent and destructive rather than merely confusing: B's sync
    /// prunes A's chunks (they are absent from B's local id set), after which
    /// A's query serves main's `fileB` *and* B's `fileB` -- the same path twice,
    /// one copy from another branch -- while A's own `fileA` is served by
    /// nothing. `classify_worktree_index_state` calls that `Healthy`; no
    /// warning, no drift note.
    ///
    /// Git's worktree name (`gitdir: <main>/.git/worktrees/<NAME>`) is unique
    /// per repository, so it is what the key is built on. Both the producer
    /// (`sync_worktree`) and the consumer (`worktree_ids`) go through
    /// `worktree_key`, asserted directly here alongside the ids so the shared
    /// derivation is pinned and not merely implied.
    #[test]
    fn worktree_ids_distinguishes_two_worktrees_that_share_a_basename() {
        let dir = tempfile::tempdir().unwrap();
        let main_repo = dir.path().join("main");
        std::fs::create_dir_all(&main_repo).unwrap();

        // Two linked worktrees of `main`, git-named `alpha` and `beta`, whose
        // checkout directories are BOTH called `wt`.
        let mut roots = Vec::new();
        for (parent, git_name) in [("a", "alpha"), ("b", "beta")] {
            let meta = main_repo.join(".git").join("worktrees").join(git_name);
            std::fs::create_dir_all(&meta).unwrap();
            std::fs::write(meta.join("HEAD"), format!("ref: refs/heads/{git_name}\n")).unwrap();

            let root = dir.path().join(parent).join("wt");
            std::fs::create_dir_all(&root).unwrap();
            std::fs::write(root.join(".git"), format!("gitdir: {}\n", meta.display())).unwrap();
            roots.push(root);
        }

        assert_eq!(
            roots[0].file_name(),
            roots[1].file_name(),
            "fixture precondition: the two worktrees must share a basename"
        );

        // The shared derivation both sides go through.
        assert_eq!(worktree_key(&roots[0]), "alpha");
        assert_eq!(worktree_key(&roots[1]), "beta");

        let (main_a, delta_a) = worktree_ids(&main_repo, &roots[0]);
        let (main_b, delta_b) = worktree_ids(&main_repo, &roots[1]);

        assert_eq!(main_a, "main");
        assert_eq!(main_b, "main");
        assert_eq!(delta_a, "main@alpha");
        assert_eq!(delta_b, "main@beta");
        assert_ne!(
            delta_a, delta_b,
            "two worktrees of one repo must never share a delta project id"
        );
    }

    #[test]
    fn worktree_ids_reads_a_real_project_toml_for_main_id() {
        // Positive control: main_id is not ALWAYS just a basename -- when
        // main_repo has a real project.toml naming it something else, that
        // name must win. Guards against a regression that hardcodes the
        // basename fallback and stops reading the config at all.
        let dir = tempfile::tempdir().unwrap();
        let main_repo = dir.path().join("checkout-dir-name");
        let codescout_dir = main_repo.join(".codescout");
        std::fs::create_dir_all(&codescout_dir).unwrap();
        std::fs::write(
            codescout_dir.join("project.toml"),
            "[project]\nname = \"configured-main-name\"\n",
        )
        .unwrap();
        let worktree_root = dir.path().join("wt");
        std::fs::create_dir_all(&worktree_root).unwrap();

        let (main_id, delta_id) = worktree_ids(&main_repo, &worktree_root);
        assert_eq!(main_id, "configured-main-name");
        assert_eq!(delta_id, "configured-main-name@wt");
    }

    #[test]
    fn delta_db_file_is_distinct_from_mains() {
        // This delta/main pair happens to also land in separate DB files on the
        // lite (sqlite-vec) store, which maps project_id to a FILENAME via
        // sanitize_db_name (sqlite_code_store.rs:70 -> sqlite_vec_ext::sanitize_db_name).
        // That is a locality nicety pinned here for this pair, NOT a safety
        // property: row isolation comes from the *unsanitized* project_id column
        // filter (see delta_project_id's doc comment above), not from file
        // separation, which this sanitizer does not guarantee in general.
        use crate::sqlite_vec_ext::sanitize_db_name;
        let main = sanitize_db_name("codescout");
        let delta = sanitize_db_name(&delta_project_id("codescout", "peer-delegation"));
        assert_ne!(main, delta);
        assert_eq!(delta, "codescout_peer-delegation");
    }

    #[derive(Default)]
    /// Records every `upsert_chunks` batch size + the refs it upserted, so a test
    /// can assert the indexer flushes in bounded batches (regression guard for the
    /// 68 GB OOM: docs/issues/archive/2026-06-19-mcp-server-oom-68gb.md).
    struct RecordingStore {
        upsert_batches: Mutex<Vec<usize>>,
        upserted: Mutex<Vec<ChunkRef>>,
        /// `project_id` each entry in `upserted` was written under, same index
        /// correspondence as `upserted`. Additive: existing tests only read
        /// `upserted`/`upsert_batches` directly. Backs `chunk_refs`'s project
        /// filter and the `upserted_project_ids`/`upserted_file_paths` helpers a
        /// worktree-sync test uses to assert on what a sync itself wrote.
        upserted_projects: Mutex<Vec<String>>,
        deleted: Mutex<Vec<String>>,
        /// Reported by `collection_dim`. `None` (the default) means "no index
        /// yet" — a test overrides it to `Some(n)` to exercise `guard_index_dim`
        /// against a pre-existing index at dimension `n`.
        dim: Mutex<Option<u64>>,
        /// The `dim` argument `sync_project` actually passed to
        /// `ensure_collection` — captured so a test can prove the call site uses
        /// `RetrievalClient::effective_model_dim` (review round-2 I5) rather than
        /// the bare `config.model_dim.unwrap_or(DEFAULT_MODEL_DIM)` this sibling
        /// used to use, without needing to inspect a real Qdrant collection.
        ensured_dim: Mutex<Option<u64>>,
        /// Baseline chunks seeded via `seeded_for_main`, standing in for "main's
        /// index already has these bytes" before the call under test runs. Kept
        /// separate from `upserted` so a worktree-sync test can assert on what
        /// the sync ITSELF wrote without the seed polluting the answer.
        /// `(project_id, ref)` pairs; `chunk_refs` unions this (filtered by the
        /// queried project_id) with whatever `upsert_chunks` has recorded.
        seeded: Mutex<Vec<(String, ChunkRef)>>,
        /// When set, `chunk_refs` returns `Err` for this project id and this one
        /// only. Stands in for a transient store failure (I2): the point of the
        /// double is that an error and an empty result are DIFFERENT answers,
        /// which `unwrap_or_default()` used to flatten into the same one.
        chunk_refs_error_for: Mutex<Option<String>>,
        /// Reported by `count_chunks_without_vectors`. Default 0 (healthy); a verify
        /// test sets it non-zero to prove the count actually reaches the envelope
        /// rather than the envelope hardcoding the healthy answer.
        vector_holes: Mutex<usize>,
    }

    #[async_trait::async_trait]
    impl CodeVectorStore for RecordingStore {
        async fn ensure_collection(&self, _c: &str, d: u64) -> Result<()> {
            *self.ensured_dim.lock().unwrap() = Some(d);
            Ok(())
        }
        async fn chunk_refs(&self, _c: &str, p: &str) -> Result<Vec<ChunkRef>> {
            if self.chunk_refs_error_for.lock().unwrap().as_deref() == Some(p) {
                anyhow::bail!("simulated transient store failure reading `{p}`");
            }
            // Union the seeded baseline (filtered to the queried project) with
            // whatever `upsert_chunks` has actually recorded under that project
            // -- see the `seeded` field's doc comment on why these stay separate.
            let mut out: Vec<ChunkRef> = self
                .seeded
                .lock()
                .unwrap()
                .iter()
                .filter(|(proj, _)| proj == p)
                .map(|(_, r)| r.clone())
                .collect();
            let refs = self.upserted.lock().unwrap();
            let projs = self.upserted_projects.lock().unwrap();
            out.extend(
                refs.iter()
                    .zip(projs.iter())
                    .filter(|(_, proj)| proj.as_str() == p)
                    .map(|(r, _)| r.clone()),
            );
            Ok(out)
        }
        async fn upsert_chunks(
            &self,
            _c: &str,
            chunks: &[(CodePayload, EmbedOutput)],
        ) -> Result<()> {
            self.upsert_batches.lock().unwrap().push(chunks.len());
            let mut u = self.upserted.lock().unwrap();
            let mut up = self.upserted_projects.lock().unwrap();
            for (p, _) in chunks {
                u.push(ChunkRef {
                    chunk_id: p.chunk_id.clone(),
                    content_hash: p.content_hash.clone(),
                    file_path: p.file_path.clone(),
                });
                up.push(p.project_id.clone());
            }
            Ok(())
        }
        async fn delete_chunks(&self, _c: &str, _p: &str, ids: &[String]) -> Result<()> {
            self.deleted.lock().unwrap().extend(ids.iter().cloned());
            Ok(())
        }
        #[allow(clippy::too_many_arguments)]
        async fn query(
            &self,
            _c: &str,
            _p: &str,
            _dense: &[f32],
            _sparse: &SparseVector,
            _limit: usize,
            _bm25: f32,
            _disable_sparse: bool,
            _excl: &[String],
            _paths: &[String],
        ) -> Result<Vec<Hit>> {
            Ok(vec![])
        }
        async fn project_index_stats(&self, _c: &str, _p: &str) -> Result<(usize, usize)> {
            Ok((0, 0))
        }

        async fn project_has_chunks(&self, _c: &str, _p: &str) -> Result<bool> {
            Ok(false)
        }

        async fn collection_dim(&self, _c: &str, _p: &str) -> Result<Option<u64>> {
            Ok(*self.dim.lock().unwrap())
        }

        /// Mirrors the real store's observable effects, which is what makes a broken
        /// migration fail here: dropping `code_vec` means `collection_dim` now reports
        /// `None`, and clearing `code_chunk` means `chunk_refs` reports nothing for
        /// this project. A double that kept answering `Some(dim)` would let a
        /// migration that never actually reset anything pass.
        async fn reset_project_index(&self, _c: &str, p: &str) -> Result<()> {
            *self.dim.lock().unwrap() = None;
            self.seeded.lock().unwrap().retain(|(proj, _)| proj != p);
            let mut u = self.upserted.lock().unwrap();
            let mut up = self.upserted_projects.lock().unwrap();
            let (refs, projs): (Vec<ChunkRef>, Vec<String>) = u
                .drain(..)
                .zip(up.drain(..))
                .filter(|(_, proj)| proj.as_str() != p)
                .unzip();
            *u = refs;
            *up = projs;
            Ok(())
        }

        /// Overridable so a verify test can assert the hole count reaches the envelope
        /// rather than being hardcoded to the healthy answer — a double that always
        /// said 0 could not distinguish "checked and clean" from "not wired up".
        async fn count_chunks_without_vectors(&self, _c: &str, _p: &str) -> Result<usize> {
            Ok(*self.vector_holes.lock().unwrap())
        }
    }

    impl RecordingStore {
        /// Seed the double as if `project`'s index already holds these files,
        /// chunked and hashed exactly as a real sync would (via the same
        /// `split_file` + `content_hash` a production walk uses), so a
        /// worktree-sync test can set up "main already has these bytes" without
        /// running a real sync first. Written into `seeded`, never `upserted` --
        /// see that field's doc comment.
        fn seeded_for_main(project: &str, files: &[(&str, &str)]) -> Self {
            let store = Self::default();
            let mut seeded = store.seeded.lock().unwrap();
            for (path, content) in files {
                let ext = Path::new(path)
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("");
                let lang = crate::embed::lang_for_ext(ext).unwrap_or("text");
                for c in crate::embed::ast_chunker::split_file(content, lang, Path::new(path), 1200)
                {
                    if c.content.trim().is_empty() {
                        continue;
                    }
                    let hash = content_hash(&c.content);
                    seeded.push((
                        project.to_string(),
                        ChunkRef {
                            chunk_id: chunk_id(project, Path::new(path), c.start_line, &hash),
                            content_hash: hash,
                            file_path: path.to_string(),
                        },
                    ));
                }
            }
            drop(seeded);
            store
        }

        /// `project_id` each upserted chunk was written under, in upsert order.
        /// Excludes anything set up via `seeded_for_main` -- this answers "what
        /// did the call under test itself write", which is what a worktree-sync
        /// test needs to assert it never wrote under main's project_id.
        fn upserted_project_ids(&self) -> Vec<String> {
            self.upserted_projects.lock().unwrap().clone()
        }

        /// `file_path` of each upserted chunk, in upsert order. Same exclusion of
        /// seeded state as `upserted_project_ids`.
        fn upserted_file_paths(&self) -> Vec<String> {
            self.upserted
                .lock()
                .unwrap()
                .iter()
                .map(|c| c.file_path.clone())
                .collect()
        }
    }

    /// Deterministic embedder fake: one dense vector per input, no HTTP. Output
    /// length matches `texts` so the zip in `flush_pending` stays aligned.
    struct FakeEmbedder {
        dim: usize,
        /// Every text handed to `embed_batch_dyn`, in order.
        ///
        /// This is what lets a test assert on the EMBEDDING INPUT rather than on the
        /// stored payload — and that distinction is the whole bug: the legacy path
        /// stored raw content while embedding `{header}\n{content}`, so inspecting
        /// stored content would have "confirmed" correct behaviour either way.
        seen: Mutex<Vec<String>>,
    }

    #[async_trait::async_trait]
    impl BatchEmbedder for FakeEmbedder {
        async fn embed_batch_dyn(&self, texts: &[String]) -> Result<Vec<EmbedOutput>> {
            self.seen.lock().unwrap().extend(texts.iter().cloned());
            Ok(texts
                .iter()
                .map(|_| EmbedOutput {
                    dense: vec![0.1; self.dim],
                    sparse: SparseVector {
                        indices: vec![],
                        values: vec![],
                    },
                })
                .collect())
        }
    }

    fn write_sources(dir: &std::path::Path, n: usize) {
        for i in 0..n {
            std::fs::write(
                dir.join(format!("file_{i}.rs")),
                format!("fn f{i}() {{ let x = {i}; println!(\"{{}}\", x); }}\n"),
            )
            .unwrap();
        }
    }

    /// The invariant the originating report asked for as a minimum bar: an eligible
    /// top-level directory with files on disk and NONE in the index.
    ///
    /// This is the check that would have caught `docs/` at 0 of 1086 while `src/` held
    /// 298 — a state `index(action="status")` reported as `indexed: true,
    /// queryable: true`, because its only test was a non-zero chunk count.
    #[tokio::test]
    async fn verify_names_an_eligible_directory_the_index_holds_nothing_for() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::create_dir_all(dir.path().join("docs")).unwrap();
        std::fs::write(dir.path().join("src/a.rs"), "fn a() {}\n").unwrap();
        std::fs::write(dir.path().join("src/b.rs"), "fn b() {}\n").unwrap();
        std::fs::write(dir.path().join("docs/one.md"), "# one\n").unwrap();
        std::fs::write(dir.path().join("docs/two.md"), "# two\n").unwrap();

        // A store holding ONLY src/ — the shape of the reported defect.
        let store = RecordingStore::seeded_for_main("p", &[("src/a.rs", "fn a() {}\n")]);

        let report = verify_index_coverage(dir.path(), "p", "coll", &store, &[])
            .await
            .unwrap();

        assert_eq!(
            report.empty_eligible_dirs,
            vec!["docs".to_string()],
            "docs/ has files on disk and none indexed — it must be named, not summed \
             into a missing count that reads as ordinary lag"
        );
        assert!(
            report.missing_count >= 3,
            "src/b.rs plus both docs files are missing, got {}",
            report.missing_count
        );
        assert!(
            report.missing_sample.iter().any(|p| p.contains("docs/")),
            "the sample must surface the affected directory: {:?}",
            report.missing_sample
        );
    }

    /// Coverage is measured against the indexer's OWN walk, so config-ignored files
    /// are not defects.
    ///
    /// This is the mistake the original report made and I nearly repeated: `scripts/`
    /// sat at 15 indexed of 19 tracked and was entirely correct, because two entries
    /// were in `[ignored_paths]` and two were extensions outside `languages`. A verify
    /// that re-derived eligibility would have called that a hole.
    #[tokio::test]
    async fn verify_does_not_count_config_ignored_files_as_missing() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("scripts")).unwrap();
        std::fs::write(dir.path().join("scripts/keep.rs"), "fn keep() {}\n").unwrap();
        std::fs::write(dir.path().join("scripts/skip.rs"), "fn skip() {}\n").unwrap();

        let store = RecordingStore::seeded_for_main("p", &[("scripts/keep.rs", "fn keep() {}\n")]);

        // Unignored: the second file is a genuine hole.
        let bare = verify_index_coverage(dir.path(), "p", "coll", &store, &[])
            .await
            .unwrap();
        assert_eq!(
            bare.missing_count, 1,
            "without the ignore pattern skip.rs is genuinely missing"
        );

        // Ignored: the same on-disk state is now complete.
        let ignored = verify_index_coverage(
            dir.path(),
            "p",
            "coll",
            &store,
            &["scripts/skip.rs".to_string()],
        )
        .await
        .unwrap();
        assert_eq!(
            ignored.missing_count, 0,
            "an ignored file is not a coverage defect — this is the `scripts/` \
             false positive, pinned"
        );
        assert!(
            ignored.empty_eligible_dirs.is_empty(),
            "and it must not make scripts/ look empty either"
        );
    }

    /// Rows for files the walk no longer sees are reported, never deleted.
    #[tokio::test]
    async fn verify_reports_orphans_and_never_prunes_them() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/live.rs"), "fn live() {}\n").unwrap();

        let store = RecordingStore::seeded_for_main(
            "p",
            &[
                ("src/live.rs", "fn live() {}\n"),
                ("src/deleted.rs", "fn gone() {}\n"),
            ],
        );

        let report = verify_index_coverage(dir.path(), "p", "coll", &store, &[])
            .await
            .unwrap();

        assert_eq!(report.orphan_count, 1, "src/deleted.rs is an orphan");
        assert!(report.orphan_sample[0].contains("deleted.rs"));
        assert!(
            store.deleted.lock().unwrap().is_empty(),
            "verify is READ-ONLY: a bad walk that reported every file as an orphan \
             must not be able to delete a live index"
        );
    }

    /// The hole count reaches the report rather than being hardcoded healthy.
    #[tokio::test]
    async fn verify_surfaces_chunks_that_have_no_vector() {
        let dir = tempfile::tempdir().unwrap();
        let store = RecordingStore::default();
        *store.vector_holes.lock().unwrap() = 7;

        let report = verify_index_coverage(dir.path(), "p", "coll", &store, &[])
            .await
            .unwrap();
        assert_eq!(
            report.chunks_without_vectors, 7,
            "a double that always reported 0 could not tell 'checked and clean' from \
             'never wired up'"
        );
    }

    /// `<root>` grouping, so a deleted top-level file cannot masquerade as an empty
    /// eligible directory — the check's loudest signal must not fire on a one-file
    /// change.
    #[test]
    fn a_root_level_file_is_grouped_under_root_not_its_own_name() {
        assert_eq!(super::top_level_of("README.md"), "<root>");
        assert_eq!(super::top_level_of("docs/x.md"), "docs");
        assert_eq!(super::top_level_of("docs/deep/x.md"), "docs");
    }

    /// Mimics llama-server's `exceed_context_size_error`: a batch containing ANY
    /// text longer than `limit` fails *entirely*. That whole-request granularity is
    /// the point — `POST /v1/embeddings` does not embed the members it could and
    /// reject the rest, so a fake that failed only the oversized member would not
    /// reproduce the bug at all.
    struct CeilingEmbedder {
        dim: usize,
        limit: usize,
    }

    #[async_trait::async_trait]
    impl BatchEmbedder for CeilingEmbedder {
        async fn embed_batch_dyn(&self, texts: &[String]) -> Result<Vec<EmbedOutput>> {
            if let Some(t) = texts.iter().find(|t| t.len() > self.limit) {
                anyhow::bail!(
                    "input ({} tokens) is larger than the max context size ({} tokens). skipping",
                    t.len(),
                    self.limit
                );
            }
            Ok(texts
                .iter()
                .map(|_| EmbedOutput {
                    dense: vec![0.1; self.dim],
                    sparse: SparseVector {
                        indices: vec![],
                        values: vec![],
                    },
                })
                .collect())
        }
    }

    #[async_trait::async_trait]
    impl CodeEmbedder for CeilingEmbedder {
        // Every method but `known_dim` is unreachable: `sync_project` drives this
        // fixture through `BatchEmbedder` only (`&*self.embedder` cast in
        // `stream_index`), mirroring `FixedDimEmbedder` above.
        async fn embed_one(&self, _text: &str) -> Result<EmbedOutput> {
            unreachable!("CeilingEmbedder is driven through sync_project's BatchEmbedder path")
        }
        async fn embed_dense_one(&self, _text: &str) -> Result<Vec<f32>> {
            unreachable!("CeilingEmbedder is driven through sync_project's BatchEmbedder path")
        }
        async fn embed_document_one(&self, _text: &str) -> Result<Vec<f32>> {
            unreachable!("CeilingEmbedder is driven through sync_project's BatchEmbedder path")
        }
        fn known_dim(&self) -> Option<usize> {
            Some(self.dim)
        }
    }

    /// An embedder that refuses everything — a down or unhealthy backend, as
    /// opposed to a healthy one handed a single oversized payload.
    struct DeadEmbedder;

    #[async_trait::async_trait]
    impl BatchEmbedder for DeadEmbedder {
        async fn embed_batch_dyn(&self, _texts: &[String]) -> Result<Vec<EmbedOutput>> {
            anyhow::bail!("connection refused")
        }
    }

    /// A single oversized chunk must cost only itself.
    ///
    /// Before the fix, `flush_pending`'s `embed_batch_dyn(...).await?` propagated
    /// through `?` at both `stream_index` call sites, aborting the walk mid-tree.
    /// Batches already flushed stayed committed, so the build left a durably
    /// TRUNCATED index — and `index(action="status")` reported it as
    /// `indexed: true, queryable: true`, because its only check is a non-zero chunk
    /// count. That combination is why an entire top-level directory could be missing
    /// from a "healthy" index without anything failing loudly:
    /// docs/issues/archive/2026-08-26-index-status-claims-complete-without-checking-coverage.md
    #[tokio::test]
    async fn one_oversized_chunk_is_skipped_and_the_rest_of_the_walk_still_indexes() {
        let dir = tempfile::tempdir().unwrap();
        write_sources(dir.path(), 6);
        // One file whose chunk cannot fit the ceiling however it is split: a single
        // line, so the chunker has no boundary to break it on.
        std::fs::write(
            dir.path().join("huge.rs"),
            format!("fn huge() {{ let s = \"{}\"; }}\n", "x".repeat(4000)),
        )
        .unwrap();

        let store = RecordingStore::default();
        let emb = CeilingEmbedder { dim: 4, limit: 500 };

        // flush_batch=32 puts all 7 files in ONE batch, so the oversized chunk takes
        // every sibling down with it unless the isolation retry works.
        let (added, _deleted, skipped) = stream_index(
            dir.path(),
            "p",
            "coll",
            &[],
            &emb,
            &store,
            false,
            1200,
            32,
            &[],
        )
        .await
        .expect("an oversized chunk must not abort the whole index build");

        // Deliberately NOT asserting an exact count: how many chunks a 4000-char
        // single line splits into is the chunker's business, and pinning it here
        // would make this test fail on an unrelated chunker change. What must hold
        // is that every skip belongs to the offending file and nothing else does.
        assert!(
            !skipped.is_empty(),
            "the oversized chunk(s) must be reported, not silently dropped"
        );
        assert!(
            skipped.iter().all(|s| s.contains("huge.rs")),
            "only the offending file's chunks may be skipped, got {skipped:?}"
        );
        // The REASON must survive, not just the location. A report naming which chunks
        // were skipped without saying why cannot distinguish "too large, re-chunk it"
        // from "embedder down, retry" — opposite responses. The first version of this
        // fix dropped the per-chunk error and this assertion is what pins it back.
        assert!(
            skipped
                .iter()
                .all(|s| s.contains("larger than the max context size")),
            "each skip must carry the embedder's stated cause, got {skipped:?}"
        );

        let stored: Vec<String> = store
            .upserted
            .lock()
            .unwrap()
            .iter()
            .map(|c| c.file_path.clone())
            .collect();
        assert_eq!(
            added,
            stored.len(),
            "`added` must count what was actually stored"
        );
        for i in 0..6 {
            assert!(
                stored.iter().any(|p| p.contains(&format!("file_{i}.rs"))),
                "file_{i}.rs must survive a sibling's embed failure; stored={stored:?}"
            );
        }
        // NOTE `huge.rs` legitimately appears in `stored` too. The chunker splits it
        // into several chunks and the small leading/trailing ones embed fine; only
        // the oversized ones are skipped. Isolation is per CHUNK, not per file, and
        // storing the parts that fit is the intended granularity — a file is not
        // condemned wholesale because one of its chunks is too big.
        //
        // The invariant that actually matters for this bug is CONSERVATION: every
        // chunk the walk produced must be either stored or reported, never silently
        // dropped. Establish the true total by running the identical tree past a
        // healthy embedder, then assert the ceiling run accounts for all of it.
        let control_store = RecordingStore::default();
        let (control_added, _, control_skipped) = stream_index(
            dir.path(),
            "p",
            "coll",
            &[],
            &FakeEmbedder {
                dim: 4,
                seen: Mutex::new(Vec::new()),
            },
            &control_store,
            false,
            1200,
            32,
            &[],
        )
        .await
        .expect("control run with a healthy embedder");
        assert!(control_skipped.is_empty(), "control must skip nothing");
        assert_eq!(
            added + skipped.len(),
            control_added,
            "conservation violated: the ceiling run stored {added} and reported \
             {} skipped, but the tree has {control_added} chunks — the difference \
             vanished silently, which is the exact defect this test guards",
            skipped.len()
        );
    }

    /// The skip count `flush_pending`'s isolation retry discovers must reach the
    /// DURABLE sidecar, not just the in-memory `SyncReport` -- otherwise a caller
    /// who checks `index(action="status")` on a LATER call still cannot tell the
    /// last sync was partial. This is the wiring gap
    /// docs/issues/archive/2026-08-26-index-status-claims-complete-without-checking-coverage.md
    /// closes: `sync_project` must thread its own `skipped` list into
    /// `write_index_state_with_dirty`, not pass `&[]`.
    #[tokio::test]
    async fn sync_project_records_its_own_skip_count_in_the_durable_sidecar() {
        let dir = tempfile::tempdir().unwrap();
        let lock_dir = tempfile::tempdir().unwrap();
        write_sources(dir.path(), 3);
        std::fs::write(
            dir.path().join("huge.rs"),
            format!("fn huge() {{ let s = \"{}\"; }}\n", "x".repeat(4000)),
        )
        .unwrap();

        let store: Arc<dyn CodeVectorStore> = Arc::new(RecordingStore::default());
        let embedder: Arc<dyn CodeEmbedder> = Arc::new(CeilingEmbedder { dim: 4, limit: 500 });
        let client = test_retrieval_client_with_embedder(store, embedder);
        let opts = SyncOpts {
            record_index_state: true,
            index_lock_dir: Some(lock_dir.path().to_path_buf()),
            ..SyncOpts::default()
        };
        let report = client
            .sync_project("p", dir.path(), opts)
            .await
            .expect("an oversized chunk must not abort the sync");
        assert!(
            !report.skipped.is_empty(),
            "fixture must actually skip something, or this test proves nothing"
        );

        let st = crate::retrieval::index_state::read_index_state(dir.path())
            .expect("sidecar must exist after a record_index_state sync");
        assert_eq!(
            st.last_sync_skipped_count,
            report.skipped.len(),
            "the sidecar's count must match what THIS sync actually skipped"
        );
        assert!(
            st.last_sync_skipped_sample
                .iter()
                .all(|s| s.contains("huge.rs")),
            "the sample must name the offending file, got {:?}",
            st.last_sync_skipped_sample
        );
    }

    /// The converse: a clean sync must record 0, never carrying forward a
    /// previous partial sync's count -- see `a_clean_sync_records_zero_skipped_not_a_stale_carry_over`
    /// in `index_state.rs` for the same invariant at the writer layer; this pins
    /// it through the real `sync_project` path.
    #[tokio::test]
    async fn sync_project_clears_a_previously_recorded_skip_count_on_a_clean_run() {
        let dir = tempfile::tempdir().unwrap();
        let lock_dir = tempfile::tempdir().unwrap();
        write_sources(dir.path(), 2);

        crate::retrieval::index_state::write_index_state_with_dirty(
            dir.path(),
            &[],
            crate::retrieval::index_state::ModelStamp::Preserve,
            &["stale.rs:1 -- a previous sync's failure".to_string()],
        )
        .unwrap();

        let store: Arc<dyn CodeVectorStore> = Arc::new(RecordingStore::default());
        // A ceiling no real test content will ever hit -- standing in for a
        // healthy embedder without adding a second `CodeEmbedder` fake.
        let embedder: Arc<dyn CodeEmbedder> = Arc::new(CeilingEmbedder {
            dim: 4,
            limit: 100_000,
        });
        let client = test_retrieval_client_with_embedder(store, embedder);
        let opts = SyncOpts {
            record_index_state: true,
            index_lock_dir: Some(lock_dir.path().to_path_buf()),
            ..SyncOpts::default()
        };
        client
            .sync_project("p", dir.path(), opts)
            .await
            .expect("a healthy embedder must sync cleanly");

        let st = crate::retrieval::index_state::read_index_state(dir.path()).unwrap();
        assert_eq!(
            st.last_sync_skipped_count, 0,
            "a clean sync must clear a stale skip count, not carry it forward"
        );
        assert!(st.last_sync_skipped_sample.is_empty());
    }

    /// The companion guard to the test above, and the reason the fix probes rather
    /// than pattern-matching error strings.
    ///
    /// "Every chunk in the batch failed individually" is ambiguous: it is what a
    /// batch of uniformly oversized chunks looks like, AND what a dead backend looks
    /// like. They need opposite handling. If `flush_pending` simply skipped whatever
    /// failed, a down embedder would produce an EMPTY index reported as a successful
    /// sync — strictly worse than today's loud abort. So a minimal probe decides,
    /// and this test pins that a dead embedder still aborts.
    #[tokio::test]
    async fn a_dead_embedder_aborts_instead_of_skipping_every_chunk() {
        let dir = tempfile::tempdir().unwrap();
        write_sources(dir.path(), 3);
        let store = RecordingStore::default();

        let err = stream_index(
            dir.path(),
            "p",
            "coll",
            &[],
            &DeadEmbedder,
            &store,
            false,
            1200,
            32,
            &[],
        )
        .await
        .expect_err("an embedder that refuses even a minimal probe must abort, not skip");

        let msg = format!("{err:#}");
        assert!(
            msg.contains("probe"),
            "the error must name the probe that classified the backend as unhealthy, \
             so the next reader knows skipping was considered and rejected: {msg}"
        );
        assert!(
            store.upserted.lock().unwrap().is_empty(),
            "nothing should be stored when the embedder is down"
        );
    }

    #[tokio::test]
    async fn stream_index_flushes_in_bounded_batches() {
        let dir = tempfile::tempdir().unwrap();
        write_sources(dir.path(), 10);
        let store = RecordingStore::default();
        let emb = FakeEmbedder {
            dim: 4,
            seen: Mutex::new(Vec::new()),
        };

        let (added, deleted, skipped) = stream_index(
            dir.path(),
            "p",
            "coll",
            &[],
            &emb,
            &store,
            false,
            1200,
            3,
            &[],
        )
        .await
        .unwrap();

        let batches = store.upsert_batches.lock().unwrap().clone();
        // Pre-fix, the whole-tree sync did ONE upsert of every chunk. Streaming must
        // flush in multiple batches, none larger than flush_batch — the regression
        // guard for the 68 GB OOM.
        assert!(
            batches.len() >= 2,
            "expected multiple bounded flushes, got {batches:?}"
        );
        assert!(
            batches.iter().all(|&n| n <= 3),
            "a flush exceeded flush_batch=3: {batches:?}"
        );
        assert!(
            skipped.is_empty(),
            "a healthy embedder must skip nothing: {skipped:?}"
        );
        assert_eq!(batches.iter().sum::<usize>(), added);
        assert!(
            added >= 10,
            "10 files should yield >=1 chunk each; added={added}"
        );
        assert_eq!(deleted, 0);
    }

    /// The regression guard the deleted `embed::index` module took with it.
    ///
    /// `embed_text_format_includes_metadata_prefix` asserted the text sent for
    /// embedding is `{metadata}\n{content}` and not just content. It lived inside
    /// the module removed in `66db4c70`, so when the surviving path turned out not
    /// to implement the contract, nothing failed and the header quietly stopped
    /// being embedded — 579,311 chunks' worth.
    ///
    /// This asserts on what the embedder RECEIVED. Asserting on the stored payload
    /// would not have caught the original defect: the legacy path stored raw
    /// content while embedding header+content, so stored content is raw in both
    /// the working and the broken world.
    #[tokio::test]
    async fn stream_index_embeds_the_ast_header_ahead_of_content() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("widget.rs"),
            "fn assemble_widget(n: usize) -> usize {\n    let total = n * 2;\n    total\n}\n",
        )
        .unwrap();
        let store = RecordingStore::default();
        let emb = FakeEmbedder {
            dim: 4,
            seen: Mutex::new(Vec::new()),
        };

        let (added, _, _) = stream_index(
            dir.path(),
            "p",
            "coll",
            &[],
            &emb,
            &store,
            false,
            1200,
            8,
            &[],
        )
        .await
        .unwrap();
        assert!(added > 0, "expected at least one chunk");

        let seen = emb.seen.lock().unwrap().clone();
        let headed: Vec<&String> = seen
            .iter()
            .filter(|t| t.starts_with("widget.rs ::"))
            .collect();
        assert!(
            !headed.is_empty(),
            "no embedded text carried an AST header; got {seen:?}"
        );

        // Header is a PREFIX, not a replacement — the body has to survive it.
        let (header, body) = headed[0]
            .split_once('\n')
            .expect("header line, then content");
        assert!(
            header.contains("assemble_widget"),
            "header should name the symbol, got {header:?}"
        );
        assert!(
            body.contains("let total = n * 2;"),
            "content must survive the prepend, got {body:?}"
        );

        // Checkout-independence: the absolute temp path must not reach the vector.
        let root = dir.path().to_string_lossy().to_string();
        assert!(
            !seen.iter().any(|t| t.contains(&root)),
            "an absolute path leaked into the embedding input"
        );
    }

    #[tokio::test]
    async fn stream_index_incremental_skips_unchanged_and_prunes_stale() {
        let dir = tempfile::tempdir().unwrap();
        write_sources(dir.path(), 6);
        let emb = FakeEmbedder {
            dim: 4,
            seen: Mutex::new(Vec::new()),
        };

        // First pass: empty server -> everything embedded.
        let store1 = RecordingStore::default();
        let (added1, _, _) = stream_index(
            dir.path(),
            "p",
            "coll",
            &[],
            &emb,
            &store1,
            false,
            1200,
            256,
            &[],
        )
        .await
        .unwrap();
        let server: Vec<ChunkRef> = store1.upserted.lock().unwrap().clone();
        assert!(added1 >= 6);

        // Second pass: server already has every chunk -> nothing re-embedded or deleted.
        let store2 = RecordingStore::default();
        let (added2, deleted2, _) = stream_index(
            dir.path(),
            "p",
            "coll",
            &server,
            &emb,
            &store2,
            false,
            1200,
            256,
            &[],
        )
        .await
        .unwrap();
        assert_eq!(added2, 0, "unchanged tree must not re-embed");
        assert_eq!(deleted2, 0);
        assert!(store2.upsert_batches.lock().unwrap().is_empty());

        // Change one file -> its new chunk upserts, its old chunk id is pruned.
        std::fs::write(
            dir.path().join("file_0.rs"),
            "fn f0() { let changed = 4242; println!(\"{}\", changed); }\n",
        )
        .unwrap();
        let store3 = RecordingStore::default();
        let (added3, deleted3, _) = stream_index(
            dir.path(),
            "p",
            "coll",
            &server,
            &emb,
            &store3,
            false,
            1200,
            256,
            &[],
        )
        .await
        .unwrap();
        assert!(added3 >= 1, "changed file should re-embed");
        assert!(deleted3 >= 1, "stale chunk id should be pruned");
    }

    #[tokio::test]
    async fn stream_index_force_reembeds_all_present_chunks() {
        let dir = tempfile::tempdir().unwrap();
        write_sources(dir.path(), 5);
        let emb = FakeEmbedder {
            dim: 4,
            seen: Mutex::new(Vec::new()),
        };

        let store1 = RecordingStore::default();
        let (added1, _, _) = stream_index(
            dir.path(),
            "p",
            "coll",
            &[],
            &emb,
            &store1,
            false,
            1200,
            256,
            &[],
        )
        .await
        .unwrap();
        let server: Vec<ChunkRef> = store1.upserted.lock().unwrap().clone();

        // force_reindex re-embeds every present chunk even though the server has them.
        let store2 = RecordingStore::default();
        let (added2, _, _) = stream_index(
            dir.path(),
            "p",
            "coll",
            &server,
            &emb,
            &store2,
            true,
            1200,
            256,
            &[],
        )
        .await
        .unwrap();
        assert_eq!(added2, added1, "force should re-embed all current chunks");
    }

    #[tokio::test]
    async fn stream_index_excludes_ignored_dirs() {
        let dir = tempfile::tempdir().unwrap();
        write_sources(dir.path(), 3); // file_0.rs..file_2.rs at root
        std::fs::create_dir_all(dir.path().join("node_modules")).unwrap();
        std::fs::write(
            dir.path().join("node_modules/dep.js"),
            "function x() { return 1; }\n",
        )
        .unwrap();
        std::fs::create_dir_all(dir.path().join("svc/.venv")).unwrap();
        std::fs::write(
            dir.path().join("svc/.venv/lib.py"),
            "def y():\n    return 2\n",
        )
        .unwrap();
        let emb = FakeEmbedder {
            dim: 4,
            seen: Mutex::new(Vec::new()),
        };

        let patterns = vec!["node_modules".to_string(), ".venv".to_string()];
        let store = RecordingStore::default();
        let (added, _, _) = stream_index(
            dir.path(),
            "p",
            "coll",
            &[],
            &emb,
            &store,
            false,
            1200,
            256,
            &patterns,
        )
        .await
        .unwrap();
        let ids: Vec<String> = store
            .upserted
            .lock()
            .unwrap()
            .iter()
            .map(|r| r.chunk_id.clone())
            .collect();
        assert!(
            ids.iter()
                .all(|id| !id.contains("node_modules") && !id.contains(".venv")),
            "ignored dirs must not be indexed: {ids:?}"
        );
        assert!(
            added >= 3,
            "the 3 root .rs files should still index; added={added}"
        );

        // With no patterns, the dep files ARE indexed (more chunks).
        let store2 = RecordingStore::default();
        let (added2, _, _) = stream_index(
            dir.path(),
            "p",
            "coll",
            &[],
            &emb,
            &store2,
            false,
            1200,
            256,
            &[],
        )
        .await
        .unwrap();
        assert!(
            added2 > added,
            "empty patterns must index everything: {added2} vs {added}"
        );
    }

    /// docs/issues/archive/2026-08-16-chunk-id-omits-index-so-duplicate-chunks-collapse.md
    ///
    /// Two chunks in the same file with byte-identical content must get distinct
    /// ids — the load-bearing regression test; it fails on the pre-fix 3-tuple id.
    ///
    /// `.toml` has no tree-sitter grammar (`get_ts_language("toml")` is `None`),
    /// so `split_file` falls through to the plain-text line-based splitter, which
    /// is size-driven and therefore fully controllable: a block repeated
    /// back-to-back, with `chunk_target` set to exactly that block's packed size,
    /// deterministically produces two chunks whose content is byte-identical.
    #[tokio::test]
    async fn stream_index_disambiguates_duplicate_content_chunks_in_one_file() {
        let block = "value_one = 111\nvalue_two = 222\nvalue_three = 333";
        let block_cost: usize = block.lines().map(|l| l.len() + 1).sum();
        let source = format!("{block}\n{block}");

        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("dup.toml"), &source).unwrap();

        let store = RecordingStore::default();
        let emb = FakeEmbedder {
            dim: 4,
            seen: Mutex::new(Vec::new()),
        };

        let (added, _, _) = stream_index(
            dir.path(),
            "p",
            "coll",
            &[],
            &emb,
            &store,
            false,
            block_cost,
            256,
            &[],
        )
        .await
        .unwrap();

        assert_eq!(
            added, 2,
            "the doubled block must produce exactly two chunks"
        );

        let upserted = store.upserted.lock().unwrap();
        assert_eq!(upserted.len(), 2);
        assert_eq!(
            upserted[0].content_hash, upserted[1].content_hash,
            "the fixture's premise: both chunks must be byte-identical"
        );
        let ids: std::collections::HashSet<&str> =
            upserted.iter().map(|r| r.chunk_id.as_str()).collect();
        assert_eq!(
            ids.len(),
            2,
            "same file, same content hash, different position — chunk ids must not \
         collide, or a real store's last-wins upsert silently drops one: {:?}",
            upserted.iter().map(|r| &r.chunk_id).collect::<Vec<_>>()
        );
    }

    #[tokio::test]
    async fn worktree_sync_embeds_only_dirty_files_and_records_them() {
        // Fixture is BUILT, never assumed: a worktree that exists only on the
        // developer's machine is not a test. Pattern copied from
        // src/prompts/mod.rs:774 (detect_worktree_info_identifies_linked_worktree).
        let tmp = tempfile::tempdir().unwrap();
        let main = tmp.path().join("main");
        let wt = tmp.path().join("wt");
        let meta = main.join(".git").join("worktrees").join("feat");
        std::fs::create_dir_all(&meta).unwrap();
        std::fs::write(meta.join("HEAD"), "ref: refs/heads/feat\n").unwrap();
        std::fs::create_dir_all(wt.join("src")).unwrap();
        std::fs::write(wt.join(".git"), format!("gitdir: {}\n", meta.display())).unwrap();

        // main's index already holds src/same.rs with the SAME bytes the worktree
        // has, src/gone.rs (which the worktree does not have at all), and
        // src/changed.rs at an OLDER version than what's on disk in the worktree.
        std::fs::write(wt.join("src").join("same.rs"), "fn same() {}\n").unwrap();
        std::fs::write(wt.join("src").join("changed.rs"), "fn changed_v2() {}\n").unwrap();

        let store = RecordingStore::seeded_for_main(
            "codescout",
            &[
                ("src/same.rs", "fn same() {}\n"),
                ("src/gone.rs", "fn gone() {}\n"),
                ("src/changed.rs", "fn changed_v1() {}\n"),
            ],
        );
        let emb = FakeEmbedder {
            dim: 4,
            seen: Mutex::new(Vec::new()),
        };
        // A dir of its own, not `tmp`: the lock must not land inside the tree
        // being synced, and each test needs its own lock namespace so parallel
        // tests using the same delta id don't contend with each other or with
        // the real per-user runtime dir.
        let lock_dir = tempfile::tempdir().unwrap();

        sync_worktree(
            &store,
            &wt,
            "codescout",
            "coll",
            &emb,
            false,
            &[],
            Some(lock_dir.path()),
            None,
        )
        .await
        .unwrap();

        let upserted = store.upserted_project_ids();
        assert!(
            !upserted.is_empty(),
            "the changed file must have produced at least one upsert"
        );
        // `codescout@feat`, not `codescout@wt`: this fixture's checkout
        // directory is `wt` while its git worktree name is `feat`, and the
        // delta is keyed on the git name (I1 -- see `worktree_key`). The
        // divergence is deliberate; it is what makes this assertion notice if
        // the key ever falls back to the basename.
        assert!(
            upserted.iter().all(|p| p == "codescout@feat"),
            "a worktree sync must write under the git-name-keyed delta id and \
             never under main's project_id, got {upserted:?}"
        );
        let files = store.upserted_file_paths();
        assert!(files.contains(&"src/changed.rs".to_string()));
        assert!(
            !files.contains(&"src/same.rs".to_string()),
            "identical bytes must reuse main's vector, not be re-embedded"
        );
        assert!(
            !files.contains(&"src/gone.rs".to_string()),
            "a file absent from the worktree has nothing to embed"
        );

        let st = crate::retrieval::index_state::read_index_state(&wt).unwrap();
        let dirty: std::collections::BTreeSet<_> = st.dirty_paths.iter().cloned().collect();
        assert!(dirty.contains("src/changed.rs"));
        assert!(
            dirty.contains("src/gone.rs"),
            "a file main holds and the worktree lacks must be excluded from main's results"
        );
        assert!(!dirty.contains("src/same.rs"));
    }

    /// Build the fixture both I2 and I3 use: a linked worktree of `main` whose
    /// git name is `feat`, holding one file byte-identical to main's index and
    /// one that differs. Returns `(tmpdir, worktree_root)`.
    fn worktree_fixture() -> (tempfile::TempDir, PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let main = tmp.path().join("main");
        let wt = tmp.path().join("wt");
        let meta = main.join(".git").join("worktrees").join("feat");
        std::fs::create_dir_all(&meta).unwrap();
        std::fs::write(meta.join("HEAD"), "ref: refs/heads/feat\n").unwrap();
        std::fs::create_dir_all(wt.join("src")).unwrap();
        std::fs::write(wt.join(".git"), format!("gitdir: {}\n", meta.display())).unwrap();
        std::fs::write(wt.join("src").join("same.rs"), "fn same() {}\n").unwrap();
        std::fs::write(wt.join("src").join("changed.rs"), "fn changed_v2() {}\n").unwrap();
        (tmp, wt)
    }

    fn seeded_main_store() -> RecordingStore {
        RecordingStore::seeded_for_main(
            "codescout",
            &[
                ("src/same.rs", "fn same() {}\n"),
                ("src/changed.rs", "fn changed_v1() {}\n"),
            ],
        )
    }

    /// I2: a store error reading main's chunk refs must NOT be read as "main
    /// holds nothing".
    ///
    /// `unwrap_or_default()` turned a transient failure into that factual
    /// claim, and `dirty_paths` then concluded every file in the worktree is
    /// dirty. The visible cost is not a warning: it is a full re-embed of the
    /// whole corpus, and every later query shipping every path in the repo as
    /// an exclusion filter. Both are silent.
    ///
    /// The assertions are deliberately about *effects*, not just the `Err`: a
    /// version that returned the error but had already re-embedded, or had
    /// already written a whole-corpus dirty set, would still have done the
    /// damage.
    #[tokio::test]
    async fn worktree_sync_propagates_a_chunk_refs_failure_instead_of_reading_it_as_empty() {
        let (_tmp, wt) = worktree_fixture();
        let store = seeded_main_store();
        *store.chunk_refs_error_for.lock().unwrap() = Some("codescout".to_string());
        let emb = FakeEmbedder {
            dim: 4,
            seen: Mutex::new(Vec::new()),
        };
        let lock_dir = tempfile::tempdir().unwrap();

        let err = sync_worktree(
            &store,
            &wt,
            "codescout",
            "coll",
            &emb,
            false,
            &[],
            Some(lock_dir.path()),
            None,
        )
        .await
        .expect_err("a store failure on the drift baseline must not be swallowed");

        let msg = format!("{err:#}");
        assert!(
            msg.contains("codescout"),
            "the error must name the project whose refs could not be read: {msg}"
        );

        assert!(
            store.upserted_project_ids().is_empty(),
            "nothing may be embedded off a baseline we could not read -- that is \
             the full-corpus re-embed this guards against"
        );
        assert!(
            crate::retrieval::index_state::read_index_state(&wt).is_none(),
            "no dirty set may be recorded off an unreadable baseline: writing one \
             here would list every file in the worktree and make every later query \
             carry the whole repo as an exclusion filter"
        );
    }

    /// I3: the dirty-path sidecar is written BEFORE the first upsert, so a
    /// failure part-way through leaves the index over-excluded, never
    /// under-excluded.
    ///
    /// With the write at the end (as shipped), an embedder timeout between the
    /// first committed flush and that write left delta chunks committed for
    /// freshly-edited paths while the sidecar still named the PREVIOUS dirty
    /// set. Main was therefore never told to exclude those paths and served its
    /// stale copy alongside the delta's new one -- a double-serve of exactly the
    /// files the user just edited.
    ///
    /// Here the embedder fails on its first call, so the sync cannot complete;
    /// the sidecar must nonetheless already name the dirty path. Reversing the
    /// order leaves no sidecar at all and this fails.
    #[tokio::test]
    async fn worktree_sync_records_the_dirty_set_before_it_upserts_anything() {
        struct FailingEmbedder;
        #[async_trait::async_trait]
        impl BatchEmbedder for FailingEmbedder {
            async fn embed_batch_dyn(&self, _texts: &[String]) -> Result<Vec<EmbedOutput>> {
                anyhow::bail!("simulated embedder timeout")
            }
        }

        let (_tmp, wt) = worktree_fixture();
        let store = seeded_main_store();
        let lock_dir = tempfile::tempdir().unwrap();

        let err = sync_worktree(
            &store,
            &wt,
            "codescout",
            "coll",
            &FailingEmbedder,
            false,
            &[],
            Some(lock_dir.path()),
            None,
        )
        .await
        .expect_err("the embedder failed, so the sync must not report success");
        assert!(
            format!("{err:#}").contains("simulated embedder timeout"),
            "unexpected failure: {err:#}"
        );

        assert!(
            store.upserted_project_ids().is_empty(),
            "fixture precondition: the embedder failed before any upsert landed"
        );

        let st = crate::retrieval::index_state::read_index_state(&wt).expect(
            "the dirty set must already be on disk when the upserts fail -- without \
             it main is never told to exclude the changed path and serves its stale \
             copy, a double-serve rather than mere staleness",
        );
        assert!(
            st.dirty_paths.contains(&"src/changed.rs".to_string()),
            "the recorded dirty set must be the COMPLETE one computed before the \
             upserts, got {:?}",
            st.dirty_paths
        );
        assert!(
            !st.dirty_paths.contains(&"src/same.rs".to_string()),
            "a byte-identical file is not dirty and must not be excluded from main, \
             got {:?}",
            st.dirty_paths
        );
    }

    /// The worktree path embeds too, so it carries the same refusal.
    ///
    /// Sibling of `sync_project_refuses_an_unlinked_binary_before_acquiring_the_index_lock`.
    /// The pair is what stops the guard being wired into one indexing path only: a
    /// zombie barred from re-indexing a project must not be able to re-index a
    /// worktree delta instead. Note the two call sites disagree on sidecar ordering
    /// (this one writes BEFORE its embed pass, `sync_project` after), which is
    /// exactly why the refusal belongs ahead of both rather than at the writer.
    ///
    /// `FakeEmbedder` here succeeds, so the guard is the ONLY thing that can make
    /// this return `Err` — delete the guard call and the sync completes, making
    /// `expect_err` panic.
    #[tokio::test]
    async fn sync_worktree_refuses_an_unlinked_binary_before_acquiring_the_index_lock() {
        let (_tmp, wt) = worktree_fixture();
        let store = seeded_main_store();
        let emb = FakeEmbedder {
            dim: 4,
            seen: Mutex::new(Vec::new()),
        };
        let lock_dir = tempfile::tempdir().unwrap();

        let err = sync_worktree(
            &store,
            &wt,
            "codescout",
            "coll",
            &emb,
            false,
            &[],
            Some(lock_dir.path()),
            Some(crate::retrieval::index_state::WriterProvenance {
                git_sha: "deadbee".to_string(),
                git_dirty: false,
                pid: 4242,
                exe_deleted: Some(true),
            }),
        )
        .await
        .expect_err("sync_worktree must refuse to re-index from an unlinked binary");
        assert!(
            err.downcast_ref::<crate::tools::RecoverableError>()
                .is_some(),
            "must be RecoverableError (isError: false) so sibling parallel tool calls \
             survive the refusal; got: {err:#}"
        );
        assert!(
            store.upserted_project_ids().is_empty(),
            "the refusal must precede the embed pass — nothing may reach the store"
        );
        assert_eq!(
            std::fs::read_dir(lock_dir.path()).unwrap().count(),
            0,
            "the refusal must precede the index-lock acquisition, so no lock file \
             should exist; a non-empty lock dir means the guard runs too late"
        );
    }

    #[tokio::test]
    async fn sync_worktree_force_reindex_reembeds_all_present_chunks() {
        // Mirrors stream_index_force_reembeds_all_present_chunks. main never has
        // this file at all, so every sync classifies it dirty regardless of
        // force -- the point is what happens to a chunk id the DELTA already
        // has: without the `!force_reindex` guard on that skip,
        // `index(action='build', force=true)` inside a worktree is a silent
        // no-op whenever chunk ids already match, which is exactly the case a
        // bad delta (model change, dimension migration, a half-written vector)
        // needs force to fix.
        let tmp = tempfile::tempdir().unwrap();
        let main = tmp.path().join("main");
        let wt = tmp.path().join("wt");
        let meta = main.join(".git").join("worktrees").join("feat");
        std::fs::create_dir_all(&meta).unwrap();
        std::fs::write(meta.join("HEAD"), "ref: refs/heads/feat\n").unwrap();
        std::fs::create_dir_all(wt.join("src")).unwrap();
        std::fs::write(wt.join(".git"), format!("gitdir: {}\n", meta.display())).unwrap();
        std::fs::write(wt.join("src").join("changed.rs"), "fn changed_v2() {}\n").unwrap();

        let store = RecordingStore::seeded_for_main("codescout", &[]);
        let emb = FakeEmbedder {
            dim: 4,
            seen: Mutex::new(Vec::new()),
        };
        let lock_dir = tempfile::tempdir().unwrap();

        // First sync populates the delta.
        sync_worktree(
            &store,
            &wt,
            "codescout",
            "coll",
            &emb,
            false,
            &[],
            Some(lock_dir.path()),
            None,
        )
        .await
        .unwrap();
        let batches_after_first = store.upsert_batches.lock().unwrap().len();
        assert!(
            batches_after_first > 0,
            "the first sync must embed the dirty file"
        );

        // Second sync, force_reindex=false: same content, same chunk id already
        // present in the delta -- nothing should re-embed.
        sync_worktree(
            &store,
            &wt,
            "codescout",
            "coll",
            &emb,
            false,
            &[],
            Some(lock_dir.path()),
            None,
        )
        .await
        .unwrap();
        assert_eq!(
            store.upsert_batches.lock().unwrap().len(),
            batches_after_first,
            "an unforced repeat sync of unchanged dirty content must not re-embed"
        );

        // Third sync, force_reindex=true: same content, same chunk id -- must
        // re-embed anyway.
        sync_worktree(
            &store,
            &wt,
            "codescout",
            "coll",
            &emb,
            true,
            &[],
            Some(lock_dir.path()),
            None,
        )
        .await
        .unwrap();
        assert!(
            store.upsert_batches.lock().unwrap().len() > batches_after_first,
            "force_reindex=true must re-embed chunks the delta already has"
        );
    }

    /// A `CodeVectorStore` whose `chunk_refs` sleeps briefly before returning.
    /// `sync_worktree` calls `chunk_refs` (establishing its main/delta baseline)
    /// immediately after acquiring its own index lock, so this gives a
    /// controllable window in which the lock is provably still held -- the
    /// `sync_worktree` sibling of `SlowEnsureStore` below, which serves the
    /// same purpose for `sync_project` (whose first post-lock call is
    /// `ensure_collection`, which `sync_worktree` never calls at all).
    struct SlowChunkRefsStore;

    #[async_trait::async_trait]
    impl CodeVectorStore for SlowChunkRefsStore {
        async fn ensure_collection(&self, _c: &str, _d: u64) -> Result<()> {
            Ok(())
        }
        async fn chunk_refs(&self, _c: &str, _p: &str) -> Result<Vec<ChunkRef>> {
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            Ok(vec![])
        }
        async fn upsert_chunks(
            &self,
            _c: &str,
            _chunks: &[(CodePayload, EmbedOutput)],
        ) -> Result<()> {
            Ok(())
        }
        async fn delete_chunks(&self, _c: &str, _p: &str, _ids: &[String]) -> Result<()> {
            Ok(())
        }
        #[allow(clippy::too_many_arguments)]
        async fn query(
            &self,
            _c: &str,
            _p: &str,
            _dense: &[f32],
            _sparse: &SparseVector,
            _limit: usize,
            _bm25: f32,
            _disable_sparse: bool,
            _excl: &[String],
            _paths: &[String],
        ) -> Result<Vec<Hit>> {
            Ok(vec![])
        }
        async fn project_index_stats(&self, _c: &str, _p: &str) -> Result<(usize, usize)> {
            Ok((0, 0))
        }

        async fn project_has_chunks(&self, _c: &str, _p: &str) -> Result<bool> {
            Ok(false)
        }

        async fn collection_dim(&self, _c: &str, _p: &str) -> Result<Option<u64>> {
            Ok(None)
        }

        /// Never exercised: this double exists only to make `chunk_refs` slow so a
        /// lock-duration test can observe the window. `unreachable!` rather than
        /// `Ok(())` so that if a future change routes a reset through this store, the
        /// test fails loudly instead of silently asserting on a no-op.
        async fn reset_project_index(&self, _c: &str, _p: &str) -> Result<()> {
            unreachable!("SlowChunkRefsStore is only used for lock-duration timing")
        }

        async fn count_chunks_without_vectors(&self, _c: &str, _p: &str) -> Result<usize> {
            unreachable!("SlowChunkRefsStore is only used for lock-duration timing")
        }
    }

    #[tokio::test]
    async fn sync_worktree_holds_index_lock_for_its_full_duration() {
        // Mirrors sync_project_holds_index_lock_for_its_full_duration, adapted
        // for sync_worktree's own lock (acquired on the DELTA project id -- see
        // sync_worktree's doc comment for why). Proves the lock is genuinely
        // held across the whole call, not acquired then immediately dropped by
        // a `let _ = ...` binding mistake -- a bare `_index_lock -> _` rename
        // compiles clean and passes every other test.
        let dir = tempfile::tempdir().unwrap();
        let lock_dir = tempfile::tempdir().unwrap();
        let main_project_id = "test-sync-worktree-holds-lock".to_string();
        let wt_dir_name = "wt";
        let root = dir.path().join(wt_dir_name);
        std::fs::create_dir_all(&root).unwrap();

        let store = Arc::new(SlowChunkRefsStore);
        let emb = Arc::new(FakeEmbedder {
            dim: 4,
            seen: Mutex::new(Vec::new()),
        });
        let lock_dir_path = lock_dir.path().to_path_buf();
        let root_for_task = root.clone();
        let mid = main_project_id.clone();
        let handle = tokio::spawn(async move {
            sync_worktree(
                store.as_ref(),
                &root_for_task,
                &mid,
                "coll",
                emb.as_ref(),
                false,
                &[],
                Some(&lock_dir_path),
                None,
            )
            .await
        });

        // Give the spawned call time to acquire the lock and enter chunk_refs'
        // 300ms sleep, but stay well inside that window.
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        let delta_id = delta_project_id(&main_project_id, wt_dir_name);
        let contended = crate::retrieval::index_lock::acquire_in(lock_dir.path(), &delta_id);
        assert!(
            contended.is_err(),
            "sync_worktree's index-lock guard must still be held while the call is in flight"
        );
        let msg = format!("{:#}", contended.unwrap_err());
        assert!(
            msg.contains("already running"),
            "error should surface lock-contention wording, got: {msg}"
        );

        handle
            .await
            .expect("spawned task must not panic")
            .expect("sync_worktree should still succeed once it completes");
    }

    /// A `CodeVectorStore` whose `ensure_collection` sleeps briefly before
    /// returning. `sync_project` calls `ensure_collection` immediately after
    /// acquiring the index lock (before any real indexing work), so this gives
    /// a controllable window in which the lock is provably still held —
    /// without needing real files, a real embedder, or a real Qdrant.
    struct SlowEnsureStore;

    #[async_trait::async_trait]
    impl CodeVectorStore for SlowEnsureStore {
        async fn ensure_collection(&self, _c: &str, _d: u64) -> Result<()> {
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            Ok(())
        }
        async fn chunk_refs(&self, _c: &str, _p: &str) -> Result<Vec<ChunkRef>> {
            Ok(vec![])
        }
        async fn upsert_chunks(
            &self,
            _c: &str,
            _chunks: &[(CodePayload, EmbedOutput)],
        ) -> Result<()> {
            Ok(())
        }
        async fn delete_chunks(&self, _c: &str, _p: &str, _ids: &[String]) -> Result<()> {
            Ok(())
        }
        #[allow(clippy::too_many_arguments)]
        async fn query(
            &self,
            _c: &str,
            _p: &str,
            _dense: &[f32],
            _sparse: &SparseVector,
            _limit: usize,
            _bm25: f32,
            _disable_sparse: bool,
            _excl: &[String],
            _paths: &[String],
        ) -> Result<Vec<Hit>> {
            Ok(vec![])
        }
        async fn project_index_stats(&self, _c: &str, _p: &str) -> Result<(usize, usize)> {
            Ok((0, 0))
        }

        async fn project_has_chunks(&self, _c: &str, _p: &str) -> Result<bool> {
            Ok(false)
        }

        async fn collection_dim(&self, _c: &str, _p: &str) -> Result<Option<u64>> {
            Ok(None)
        }

        /// Never exercised — see `SlowChunkRefsStore::reset_project_index`.
        async fn reset_project_index(&self, _c: &str, _p: &str) -> Result<()> {
            unreachable!("SlowEnsureStore is only used for lock-duration timing")
        }

        async fn count_chunks_without_vectors(&self, _c: &str, _p: &str) -> Result<usize> {
            unreachable!("SlowEnsureStore is only used for lock-duration timing")
        }
    }

    fn test_retrieval_client(store: impl CodeVectorStore + 'static) -> RetrievalClient {
        RetrievalClient {
            code_store: Arc::new(store),
            embedder: std::sync::Arc::new(UnknownDimEmbedder),
            #[cfg(feature = "remote-embed")]
            reranker: RerankerHttp::new("http://unused.invalid"),
            config: RetrievalConfig {
                qdrant_url: "http://unused.invalid".into(),
                embedder_url: Some("http://unused.invalid".into()),
                sparse_embedder_url: "http://unused.invalid".into(),
                reranker_url: "http://unused.invalid".into(),
                model_dim: Some(3),
                model: "local:AllMiniLML6V2Q".into(),
                api_key: None,
                profile: "cpu".into(),
                bm25_boost: 1.0,
                disable_sparse: false,
                rerank: false,
                collection_prefix: String::new(),
                // Inert: these fixtures inject `code_store` directly, so nothing
                // ever resolves a path under this. Mirrors the `unused.invalid`
                // idiom above — a value that fails loudly if it is ever read.
                sqlite_dir: std::path::PathBuf::from("/unused-in-tests/sqlite"),
            },
            lite: false,
        }
    }

    /// A `CodeEmbedder` fake standing in for `CodeEmbedderAdapter` (a local
    /// backend that self-describes its dimension) without a real ONNX load.
    /// Every method but `known_dim` is unreachable — the one test using this
    /// never calls embed.
    struct FixedDimEmbedder(usize);

    #[async_trait::async_trait]
    impl BatchEmbedder for FixedDimEmbedder {
        async fn embed_batch_dyn(&self, _texts: &[String]) -> Result<Vec<EmbedOutput>> {
            unreachable!("FixedDimEmbedder is only used to answer known_dim()")
        }
    }

    #[async_trait::async_trait]
    impl CodeEmbedder for FixedDimEmbedder {
        async fn embed_one(&self, _text: &str) -> Result<EmbedOutput> {
            unreachable!("FixedDimEmbedder is only used to answer known_dim()")
        }
        async fn embed_dense_one(&self, _text: &str) -> Result<Vec<f32>> {
            unreachable!("FixedDimEmbedder is only used to answer known_dim()")
        }
        async fn embed_document_one(&self, _text: &str) -> Result<Vec<f32>> {
            unreachable!("FixedDimEmbedder is only used to answer known_dim()")
        }

        fn known_dim(&self) -> Option<usize> {
            Some(self.0)
        }
    }

    /// A `CodeEmbedder` fake standing in for a **remote** backend: one that
    /// cannot self-describe its dimension without a network round trip, so
    /// `known_dim()` is always `None`. That is `EmbedderHttp`'s own answer, and
    /// it is the only property of `EmbedderHttp` that `test_retrieval_client`
    /// ever depended on — so using this instead lets the fixture, and the six
    /// `sync_project` tests built on it, run in a build with no HTTP transport
    /// rather than being `remote-embed`-gated out of it.
    ///
    /// Deliberately NOT `FixedDimEmbedder`, and the difference is load-bearing.
    /// `effective_model_dim` is `known_dim().or(config.model_dim).unwrap_or(fallback)`,
    /// so an embedder answering `Some(_)` **shadows the operator's `model_dim`
    /// pin** — and `test_retrieval_client` pins `model_dim: Some(3)`, which is
    /// exactly what its dim-guard tests turn on. See
    /// `resume-embedding-transport-stages-1-3:ET-7`, which gated these tests
    /// rather than swapping `FixedDimEmbedder` in for this reason.
    struct UnknownDimEmbedder;

    #[async_trait::async_trait]
    impl BatchEmbedder for UnknownDimEmbedder {
        async fn embed_batch_dyn(&self, _texts: &[String]) -> Result<Vec<EmbedOutput>> {
            unreachable!("UnknownDimEmbedder is only used to answer known_dim()")
        }
    }

    #[async_trait::async_trait]
    impl CodeEmbedder for UnknownDimEmbedder {
        async fn embed_one(&self, _text: &str) -> Result<EmbedOutput> {
            unreachable!("UnknownDimEmbedder is only used to answer known_dim()")
        }
        async fn embed_dense_one(&self, _text: &str) -> Result<Vec<f32>> {
            unreachable!("UnknownDimEmbedder is only used to answer known_dim()")
        }
        async fn embed_document_one(&self, _text: &str) -> Result<Vec<f32>> {
            unreachable!("UnknownDimEmbedder is only used to answer known_dim()")
        }

        fn known_dim(&self) -> Option<usize> {
            None
        }
    }

    /// Like `test_retrieval_client`, but with an injectable embedder and no
    /// `model_dim` pin — for exercising `RetrievalClient::effective_model_dim`'s
    /// embedder-first priority directly, rather than the pin-or-default shape
    /// `test_retrieval_client` is set up for. Takes an already-`Arc`'d store
    /// (rather than `impl CodeVectorStore + 'static`, as `test_retrieval_client`
    /// does) so a caller can keep a concrete-typed clone to inspect afterward —
    /// `CodeVectorStore` has no `as_any()`/downcast seam to recover one later.
    fn test_retrieval_client_with_embedder(
        store: Arc<dyn CodeVectorStore>,
        embedder: Arc<dyn CodeEmbedder>,
    ) -> RetrievalClient {
        RetrievalClient {
            code_store: store,
            embedder,
            #[cfg(feature = "remote-embed")]
            reranker: RerankerHttp::new("http://unused.invalid"),
            config: RetrievalConfig {
                qdrant_url: "http://unused.invalid".into(),
                embedder_url: Some("http://unused.invalid".into()),
                sparse_embedder_url: "http://unused.invalid".into(),
                reranker_url: "http://unused.invalid".into(),
                model_dim: None,
                model: "local:AllMiniLML6V2Q".into(),
                api_key: None,
                profile: "cpu".into(),
                bm25_boost: 1.0,
                disable_sparse: false,
                rerank: false,
                collection_prefix: String::new(),
                // Inert: these fixtures inject `code_store` directly, so nothing
                // ever resolves a path under this. Mirrors the `unused.invalid`
                // idiom above — a value that fails loudly if it is ever read.
                sqlite_dir: std::path::PathBuf::from("/unused-in-tests/sqlite"),
            },
            lite: false,
        }
    }

    /// Regression guard for the index-lock wiring in `sync_project` (the
    /// `let _index_lock = ...acquire_in/acquire(project_id)?;` at the top of the
    /// function). Binding the acquired guard to `_` instead
    /// of `_index_lock` compiles clean and passes every OTHER retrieval test,
    /// but drops the guard immediately — releasing the flock right away
    /// instead of holding it for the sync pass — which is exactly how the
    /// concurrent-index duplication bug this branch fixes would return.
    ///
    /// A single "acquire first, then call sync_project once" test cannot
    /// distinguish `_index_lock` from `_`: if the lock is already held
    /// externally, `sync_project`'s own `acquire(project_id)?` fails
    /// identically either way, since that failure happens at the
    /// `try_lock_exclusive` call itself, before the binding pattern is even
    /// reached. So instead this spawns `sync_project` (slowed down via
    /// `SlowEnsureStore` so it is provably still in flight) and, from the
    /// OUTSIDE, tries to acquire the same lock while it runs: that outside
    /// acquire must fail iff `sync_project`'s guard is still alive at that
    /// moment.
    #[tokio::test]
    async fn sync_project_holds_index_lock_for_its_full_duration() {
        let dir = tempfile::tempdir().unwrap();
        // A dir of its own, not `dir`: the lock must not land inside the tree
        // being indexed. Also the reason `project_id` can be a plain literal —
        // the scratch dir, not the id, is what isolates concurrent runs.
        let lock_dir = tempfile::tempdir().unwrap();
        let project_id = "test-sync-holds-index-lock".to_string();

        let client = test_retrieval_client(SlowEnsureStore);
        let opts = SyncOpts {
            index_lock_dir: Some(lock_dir.path().to_path_buf()),
            ..SyncOpts::default()
        };
        let pid = project_id.clone();
        let root = dir.path().to_path_buf();
        let handle = tokio::spawn(async move { client.sync_project(&pid, &root, opts).await });

        // Give the spawned call time to acquire the lock and enter
        // ensure_collection's 300ms sleep, but stay well inside that window.
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        let contended = crate::retrieval::index_lock::acquire_in(lock_dir.path(), &project_id);
        assert!(
            contended.is_err(),
            "sync_project's index-lock guard must still be held while the call is in flight"
        );
        let msg = format!("{:#}", contended.unwrap_err());
        assert!(
            msg.contains("already running"),
            "error should surface lock-contention wording, got: {msg}"
        );

        handle
            .await
            .expect("spawned task must not panic")
            .expect("sync_project should still succeed once it completes");
    }

    /// A process whose own executable has been unlinked must decline to re-index.
    ///
    /// The refusal has to sit BEFORE the embed pass, not at the sidecar write.
    /// Refusing the sidecar write is inverted at this call site: by the time that
    /// write runs the vectors are already in the store, so declining only the
    /// record destroys the evidence and keeps the damage. See
    /// `docs/issues/archive/2026-08-26-zombie-servers-on-deleted-binaries-stamp-stale-config-into-shared-state.md`
    /// § *Re-costed 2026-08-28 — direction 2 is INVERTED at one of its two call
    /// sites*, and `open-issue-work-queue:BL-45`.
    ///
    /// `RecoverableError`, not `bail!`: this is operator-fixable (restart the
    /// server) and `isError: false` lets sibling parallel tool calls survive.
    #[test]
    fn an_unlinked_binary_is_refused_with_a_recoverable_error_naming_the_restart() {
        let err = guard_stale_binary(Some(true))
            .expect_err("a process running a deleted binary must decline to re-index");
        assert!(
            err.downcast_ref::<crate::tools::RecoverableError>()
                .is_some(),
            "must be RecoverableError (isError: false) so sibling parallel tool calls \
             survive the refusal; got: {err:#}"
        );
        let msg = format!("{err:#}");
        assert!(
            msg.to_lowercase().contains("restart"),
            "the refusal must tell the operator how to recover (restart the server), \
             otherwise it is a dead end rather than a recoverable error; got: {msg}"
        );
        assert!(
            msg.contains("/mcp"),
            "the remedy must name the concrete command that performs the restart, \
             not just the word — a hint the reader cannot act on is not a remedy; \
             got: {msg}"
        );
    }

    /// The ordinary case: a live binary indexes normally. Without this, deleting
    /// the `Some(true)` discriminant in `guard_stale_binary` — refusing
    /// unconditionally — would still pass the sibling above.
    #[test]
    fn a_live_binary_is_not_refused() {
        assert!(
            guard_stale_binary(Some(false)).is_ok(),
            "a process on a live binary must index normally"
        );
    }

    /// `None` means "could not tell", never "not deleted" — `exe_is_deleted()`
    /// returns `None` off Linux, where `/proc/self/exe` does not exist. A guard
    /// that treated `None` as deleted would refuse to index on every non-Linux
    /// platform, which is why this is a separate test from the `Some(false)` one:
    /// the two differ in meaning, not just in value.
    #[test]
    fn an_undetectable_binary_state_is_not_refused() {
        assert!(
            guard_stale_binary(None).is_ok(),
            "None means the check could not run (non-Linux); it must never be read \
             as 'deleted' or the guard refuses to index on every such platform"
        );
    }

    /// Call-site wiring proof for `guard_stale_binary`, and the mutation target
    /// that catches the guard being written but never called.
    ///
    /// Asserts the refusal happens before ANY work: the index lock is acquired
    /// near the top of `sync_project` and lock files are deliberately never
    /// unlinked, so an empty lock dir proves we returned ahead of even that. If
    /// the guard call is deleted, this empty tempdir walks to completion and
    /// returns `Ok` with `added: 0`, so `expect_err` panics.
    #[tokio::test]
    async fn sync_project_refuses_an_unlinked_binary_before_acquiring_the_index_lock() {
        let dir = tempfile::tempdir().unwrap();
        let lock_dir = tempfile::tempdir().unwrap();
        let client = test_retrieval_client(RecordingStore::default());
        let opts = SyncOpts {
            index_lock_dir: Some(lock_dir.path().to_path_buf()),
            writer: Some(crate::retrieval::index_state::WriterProvenance {
                git_sha: "deadbee".to_string(),
                git_dirty: false,
                pid: 4242,
                exe_deleted: Some(true),
            }),
            ..SyncOpts::default()
        };
        let err = client
            .sync_project("stale-binary-project", dir.path(), opts)
            .await
            .expect_err("sync_project must refuse to re-index from an unlinked binary");
        assert!(
            err.downcast_ref::<crate::tools::RecoverableError>()
                .is_some(),
            "must be RecoverableError (isError: false); got: {err:#}"
        );
        assert_eq!(
            std::fs::read_dir(lock_dir.path()).unwrap().count(),
            0,
            "the refusal must precede the index-lock acquisition, so no lock file \
             should exist; a non-empty lock dir means the guard runs too late"
        );
    }

    /// Call-site mutation target for `guard_index_dim`'s wiring into
    /// `sync_project`. `test_retrieval_client` pins `model_dim: Some(3)`; this
    /// store reports an EXISTING index already baked at a different dim (999).
    /// The project root is an empty tempdir (nothing to walk, nothing to
    /// embed) and every other `RecordingStore` method trivially succeeds — so
    /// absent the `self.guard_index_dim(&collection, project_id).await?;` line
    /// in `sync_project`, this exact setup returns `Ok` with `added: 0`, not
    /// an error. Deleting that line makes `unwrap_err()` below panic.
    #[tokio::test]
    async fn sync_project_fails_fast_on_a_dim_mismatch_before_touching_the_store() {
        let dir = tempfile::tempdir().unwrap();
        let lock_dir = tempfile::tempdir().unwrap();
        let store = RecordingStore {
            dim: Mutex::new(Some(999)),
            ..Default::default()
        };
        let client = test_retrieval_client(store);
        let opts = SyncOpts {
            index_lock_dir: Some(lock_dir.path().to_path_buf()),
            ..SyncOpts::default()
        };
        let err = client
            .sync_project("dim-mismatch-project", dir.path(), opts)
            .await
            .expect_err("a stored dim of 999 must fail against the configured model_dim of 3");
        // Review round-2 I2: assert the error CLASS + remedy, not just the
        // numbers in its Display — `RecoverableError`'s Display appends the
        // hint, so a version of this test asserting only on `format!("{err:#}")`
        // stays green even if `RecoverableError::with_hint(...)` were replaced
        // wholesale with a bare `anyhow::anyhow!(...)`, which drops the hint AND
        // flips the MCP contract from `isError: false` to `true`.
        assert!(
            err.downcast_ref::<crate::tools::RecoverableError>()
                .is_some(),
            "must be RecoverableError (isError: false) so sibling parallel tool calls \
             survive a dimension mismatch; got: {err:#}"
        );
        let msg = format!("{err:#}");
        assert!(
            msg.contains("Delete the code index"),
            "must carry the reindex remedy, got: {msg}"
        );
        assert!(
            msg.contains("999") && msg.contains('3'),
            "error should name both the stored and configured dims, got: {msg}"
        );
    }

    /// `force=true` must MIGRATE across a dimension change rather than refuse it.
    ///
    /// The defect: `sync_project` called `guard_index_dim` unconditionally *ahead* of
    /// the force-capable indexing work, so `force=true` — which advertises a full
    /// reindex — could not perform the one rebuild that genuinely requires one.
    /// `docs/issues/archive/2026-08-26-force-reindex-cannot-migrate-embedding-dimensions.md`.
    ///
    /// Identical setup to the sibling above (stored dim 999, `test_retrieval_client`
    /// pins `model_dim: Some(3)`) with only `force_reindex` flipped on. Keeping that
    /// sibling — which asserts the NON-forced path still errors — is what stops this
    /// bug being "fixed" by deleting the guard: the two tests fail in opposite
    /// directions, so no single change satisfies both unless `force` is what
    /// discriminates.
    #[tokio::test]
    async fn sync_project_force_migrates_a_dimension_mismatch_instead_of_refusing() {
        let dir = tempfile::tempdir().unwrap();
        let lock_dir = tempfile::tempdir().unwrap();
        let store = RecordingStore {
            dim: Mutex::new(Some(999)),
            ..Default::default()
        };
        let client = test_retrieval_client(store);
        let opts = SyncOpts {
            force_reindex: true,
            index_lock_dir: Some(lock_dir.path().to_path_buf()),
            ..SyncOpts::default()
        };

        let report = client
            .sync_project("dim-migration-project", dir.path(), opts)
            .await
            .expect(
                "force=true must migrate a 999-dim index to the configured 3, not \
                 refuse it — the guard is the thing force is supposed to override here",
            );

        assert_eq!(
            report.dim_migration,
            Some((999, 3)),
            "the report must name both widths: a silent successful rebuild at a \
             different dimension is nearly as confusing as the failure it replaced"
        );
    }

    /// A forced sync whose dimensions already AGREE must not report a migration.
    ///
    /// Without this, `migrate_or_guard_index_dim` could return `Some(..)`
    /// unconditionally under `force` — every forced reindex would claim to have
    /// discarded and rebuilt the index across a model change, and the field would
    /// become noise a reader learns to ignore. It also pins that `force=true` does
    /// not reset an index that needs no reset.
    #[tokio::test]
    async fn a_forced_sync_at_a_matching_dimension_reports_no_migration() {
        let dir = tempfile::tempdir().unwrap();
        let lock_dir = tempfile::tempdir().unwrap();
        // 3 == test_retrieval_client's pinned model_dim, so there is nothing to migrate.
        let store = RecordingStore {
            dim: Mutex::new(Some(3)),
            ..Default::default()
        };
        let client = test_retrieval_client(store);
        let opts = SyncOpts {
            force_reindex: true,
            index_lock_dir: Some(lock_dir.path().to_path_buf()),
            ..SyncOpts::default()
        };

        let report = client
            .sync_project("matching-dim-project", dir.path(), opts)
            .await
            .expect("a forced sync at a matching dim is an ordinary reindex");

        assert_eq!(
            report.dim_migration, None,
            "no dimension changed, so nothing was migrated — reporting one here would \
             make the field meaningless"
        );
    }

    /// Review round-2 I5: the code-collection sibling of the `memories`-collection
    /// bug named in the task 8 brief. `sync_project`'s `ensure_collection` call
    /// used to size a *fresh* collection with
    /// `self.config.model_dim.unwrap_or(DEFAULT_MODEL_DIM)` — 768 — regardless
    /// of the model actually configured. With an unpinned local embedder
    /// reporting 384 (mirroring `local:AllMiniLML6V2Q`), this test proves the
    /// call site now goes through `effective_model_dim` instead: it must pass
    /// 384 to `ensure_collection`, not the 768 compatibility default. Deleting
    /// the `self.effective_model_dim(...)` call from that line (reverting to
    /// the bare `unwrap_or(DEFAULT_MODEL_DIM)`) makes this test's assertion
    /// fail with `ensured_dim == Some(768)`.
    #[tokio::test]
    async fn sync_project_sizes_a_fresh_collection_from_the_unpinned_local_embedder() {
        let dir = tempfile::tempdir().unwrap();
        let lock_dir = tempfile::tempdir().unwrap();
        // Concrete-typed `Arc` kept alongside the trait-object clone handed to
        // the client, so `ensured_dim` can be read back afterward —
        // `CodeVectorStore` has no downcast seam.
        let store = Arc::new(RecordingStore::default());
        let embedder: Arc<dyn CodeEmbedder> = Arc::new(FixedDimEmbedder(384));
        let client = test_retrieval_client_with_embedder(store.clone(), embedder);
        let opts = SyncOpts {
            index_lock_dir: Some(lock_dir.path().to_path_buf()),
            ..SyncOpts::default()
        };
        client
            .sync_project("fresh-local-project", dir.path(), opts)
            .await
            .expect("an empty project tree with an all-Ok store must sync cleanly");
        assert_eq!(
            *store.ensured_dim.lock().unwrap(),
            Some(384),
            "must size the fresh collection from the unpinned local embedder's own \
             dimension (384), not the DEFAULT_MODEL_DIM compatibility constant (768)"
        );
    }

    /// HUMAN RULING (Task 4 Important #2, carried forward into the worktree-sync
    /// design): `sync_project`'s `record_index_state` write must never go through
    /// the plain `write_index_state` helper, because that helper delegates to
    /// `write_index_state_with_dirty(root, &[])` -- clearing any recorded dirty
    /// set. `sync_worktree` is the only thing meant to populate that list, but
    /// `sync_project` itself has no worktree awareness and is called directly by
    /// the CLI binaries (`src/bin/sync_project.rs`, `src/main.rs`) with no
    /// detection at all -- so the guard has to live in `sync_project`, not just
    /// in the `index` tool's worktree branch. This test simulates a prior
    /// worktree sync having already recorded a dirty set, then runs an ordinary
    /// `sync_project` pass over the same root: the dirty set must survive.
    /// Reverting the fix (using `write_index_state(root)` instead of reading back
    /// and re-writing the existing `dirty_paths`) makes this assertion fail with
    /// an empty vec.
    #[tokio::test]
    async fn sync_project_preserves_existing_dirty_paths_never_clears_via_plain_write_index_state()
    {
        let dir = tempfile::tempdir().unwrap();
        let lock_dir = tempfile::tempdir().unwrap();

        crate::retrieval::index_state::write_index_state_with_dirty(
            dir.path(),
            &["src/changed.rs".to_string()],
            crate::retrieval::index_state::ModelStamp::Preserve,
            &[],
        )
        .unwrap();

        let client = test_retrieval_client(RecordingStore::default());
        let opts = SyncOpts {
            record_index_state: true,
            index_lock_dir: Some(lock_dir.path().to_path_buf()),
            ..SyncOpts::default()
        };
        client
            .sync_project("some-project", dir.path(), opts)
            .await
            .expect("an empty project tree with an all-Ok store must sync cleanly");

        let state = crate::retrieval::index_state::read_index_state(dir.path())
            .expect("sidecar must still exist after the sync");
        assert_eq!(
            state.dirty_paths,
            vec!["src/changed.rs".to_string()],
            "an ordinary sync_project call must never clear a previously recorded \
                 dirty set -- it must route through write_index_state_with_dirty, \
                 never plain write_index_state"
        );
    }
}
