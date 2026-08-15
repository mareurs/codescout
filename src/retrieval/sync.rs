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
}

#[derive(Debug, Default)]
pub struct SyncReport {
    pub added: usize,
    pub updated: usize,
    pub deleted: usize,
    pub elapsed_ms: u128,
}

impl std::fmt::Display for SyncReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "added={} updated={} deleted={} elapsed_ms={}",
            self.added, self.updated, self.deleted, self.elapsed_ms
        )
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
pub fn chunk_id(project_id: &str, rel_path: &Path, content_hash: &str) -> String {
    format!("{project_id}:{}:{content_hash}", to_forward_slash(rel_path))
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
async fn flush_pending(
    embedder: &dyn crate::retrieval::embedder::BatchEmbedder,
    store: &dyn crate::retrieval::code_store::CodeVectorStore,
    collection: &str,
    pending: &mut Vec<crate::retrieval::payload::CodePayload>,
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
    let embeds = embedder.embed_batch_dyn(&texts).await?;
    let n = pending.len();
    let chunks: Vec<(CodePayload, EmbedOutput)> = pending.drain(..).zip(embeds).collect();
    store.upsert_chunks(collection, &chunks).await?;
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
/// `stream_index` and `sync_worktree` both call this (the latter once,
/// iterating the collected result twice, not walking twice) so a change to
/// the walk predicate (a new `ALWAYS_SKIP_DIRS` entry, changed ignore
/// semantics, a new `lang_for_ext` extension) can never apply to one walk
/// and not another. `dirty_paths`' entire correctness rests on main's chunk
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
) -> Result<(usize, usize)> {
    use crate::embed::ast_chunker::split_file;
    use crate::retrieval::payload::CodePayload;
    use std::collections::HashSet;

    let server_ids: HashSet<&str> = server.iter().map(|c| c.chunk_id.as_str()).collect();
    let mut local_ids: HashSet<String> = HashSet::new();
    let mut pending: Vec<CodePayload> = Vec::new();
    let mut added = 0usize;

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
            let chunk_id = chunk_id(project_id, Path::new(&rel_display), &hash);
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
                added += flush_pending(embedder, store, collection, &mut pending).await?;
            }
        }
    }
    // Flush the tail.
    if !pending.is_empty() {
        added += flush_pending(embedder, store, collection, &mut pending).await?;
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

    Ok((added, deleted))
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
) -> Result<SyncReport> {
    use crate::embed::ast_chunker::split_file;
    use crate::retrieval::drift::{dirty_paths, LocalChunk};
    use crate::retrieval::payload::CodePayload;
    use std::collections::HashSet;

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
    crate::retrieval::index_state::write_index_state_with_dirty(worktree_root, &dirty_vec)
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
            let did = chunk_id(&delta_id, Path::new(rel_display), &hash);
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
                added += flush_pending(embedder, store, collection, &mut pending).await?;
            }
        }
    }
    if !pending.is_empty() {
        added += flush_pending(embedder, store, collection, &mut pending).await?;
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
    })
}

impl crate::retrieval::client::RetrievalClient {
    pub async fn sync_project(
        &self,
        project_id: &str,
        root: &Path,
        opts: SyncOpts,
    ) -> Result<SyncReport> {
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
        self.guard_index_dim(&collection, project_id).await?;
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

        let (added, deleted) = stream_index(
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
        tracing::info!(added, deleted, elapsed_ms, "retrieval sync finished");

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
            if let Err(e) =
                crate::retrieval::index_state::write_index_state_with_dirty(root, &existing_dirty)
            {
                tracing::warn!(error = %e, "failed to write index-state sidecar");
            }
        }

        Ok(SyncReport {
            added,
            deleted,
            updated: 0,
            elapsed_ms,
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
    use crate::retrieval::embedder::{
        BatchEmbedder, CodeEmbedder, EmbedOutput, EmbedderHttp, SparseVector,
    };
    use crate::retrieval::payload::CodePayload;
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
            chunk_id("proj", &windows_shaped, "deadbeef"),
            "proj:src/retrieval/sync.rs:deadbeef",
            "the path component of a chunk id must be forward-slash normalized"
        );

        // Already-forward-slash input is untouched (the Linux/macOS path).
        let posix = std::path::PathBuf::from("src/retrieval/sync.rs");
        assert_eq!(
            chunk_id("proj", &posix, "deadbeef"),
            "proj:src/retrieval/sync.rs:deadbeef"
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
                            chunk_id: chunk_id(project, Path::new(path), &hash),
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

    #[tokio::test]
    async fn stream_index_flushes_in_bounded_batches() {
        let dir = tempfile::tempdir().unwrap();
        write_sources(dir.path(), 10);
        let store = RecordingStore::default();
        let emb = FakeEmbedder {
            dim: 4,
            seen: Mutex::new(Vec::new()),
        };

        let (added, deleted) = stream_index(
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

        let (added, _) = stream_index(
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
        let (added1, _) = stream_index(
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
        let (added2, deleted2) = stream_index(
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
        let (added3, deleted3) = stream_index(
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
        let (added1, _) = stream_index(
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
        let (added2, _) = stream_index(
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
        let (added, _) = stream_index(
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
        let (added2, _) = stream_index(
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
    }

    fn test_retrieval_client(store: impl CodeVectorStore + 'static) -> RetrievalClient {
        RetrievalClient {
            code_store: Arc::new(store),
            embedder: std::sync::Arc::new(EmbedderHttp::new(
                "http://unused.invalid",
                "http://unused.invalid",
                3,
            )),
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
