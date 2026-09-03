use anyhow::{Context, Result};
use ignore::WalkBuilder;
use sha2::{Digest, Sha256};
use std::path::Path;
use std::time::SystemTime;

use crate::librarian::catalog::artifact::ArtifactRow;
use crate::librarian::catalog::{artifact, Catalog};
use crate::librarian::classify::{classify, CompiledRule};
use crate::librarian::frontmatter;

#[derive(Debug, Default)]
pub struct IndexReport {
    pub added: usize,
    pub updated: usize,
    pub unchanged: usize,
    pub removed: usize,
    pub embedded: usize,
    /// Artifacts under the indexed root that want an embedding and have **no
    /// chunk rows** — never queued for embedding, by any run. `None` means *not
    /// measured* (embeddings are off), which is why this is not a bare `usize`:
    /// `0` would otherwise read the same for a clean corpus and for a run that
    /// never looked.
    ///
    /// Fix (c) of
    /// `docs/issues/2026-09-02-indexer-stamps-content-seen-before-it-embeds.md`.
    /// The stamp/gate ordering in [`index_repo_sync`] can commit content as
    /// "seen" while leaving it unembedded, and the resulting state is ABSORBING —
    /// every later run computes `content_unchanged == true` and skips it. This
    /// count is the only thing that makes a re-formed hole visible.
    ///
    /// It measures CHUNK ROWS rather than vectors on purpose; see the comment at
    /// the query for why the vector table would false-alarm at 100% under an
    /// external store, and for the neighbouring state it deliberately excludes.
    pub vectorless: Option<usize>,
    pub unknown_ids: Vec<String>,
}

/// One item queued for embedding — one per CHUNK, not per artifact.
///
/// **Both ids travel together, and that is the whole point of this being a
/// struct rather than the `(String, Option<String>, String)` tuple it used to
/// be.** A vector needs two identities: `chunk_id` is what it IS (the key
/// `artifact_vec_v2` and the Qdrant point are keyed on, and what `knn` returns),
/// while `artifact_id` is what it BELONGS TO (the catalog key every filter,
/// scope and hydration step downstream is expressed in).
///
/// Carrying only one of them is the defect this shape exists to prevent. Before
/// 2026-09-03 the queue carried the chunk id alone, so `ArtifactVectorStore::upsert`
/// had a single `id` slot; sqlite-vec survived because it can recover the
/// artifact id by joining `artifact_chunk`, and Qdrant could not — it has no
/// join, so it spent the one id twice, as both the point id and the payload
/// field named `artifact_id`. See
/// `docs/issues/2026-09-03-editing-an-artifact-removes-it-from-qdrant-backed-semantic-search.md`.
#[derive(Debug, Clone)]
pub struct EmbedQueueItem {
    /// UUID v4, minted by `replace_chunks`. The identity of the vector itself.
    pub chunk_id: String,
    /// `sha256(abs_path)` hex[..16]. The catalog key this chunk belongs to.
    pub artifact_id: String,
    pub title: Option<String>,
    pub text: String,
}

/// Return the text of the first H1 in a markdown body, or `None` if none is
/// found. Handles both ATX (`# Title`) and setext (`Title\n=====`) headings.
/// Text inside fenced code blocks is correctly ignored.
pub fn first_h1(body: &str) -> Option<String> {
    use pulldown_cmark::{Event, HeadingLevel, Parser, Tag, TagEnd};
    let parser = Parser::new(body);
    let mut in_h1 = false;
    let mut title = String::new();
    for event in parser {
        match event {
            Event::Start(Tag::Heading {
                level: HeadingLevel::H1,
                ..
            }) => in_h1 = true,
            Event::End(TagEnd::Heading(HeadingLevel::H1)) => {
                if !title.trim().is_empty() {
                    return Some(title.trim().to_string());
                }
                in_h1 = false;
                title.clear();
            }
            Event::Text(t) if in_h1 => title.push_str(&t),
            Event::Code(t) if in_h1 => title.push_str(&t),
            _ => {}
        }
    }
    None
}

/// Build the embed-queue entries for `doc`: ONE PER CHUNK, keyed by chunk id.
///
/// **`doc` is the WHOLE file, frontmatter and all** — not a pre-stripped body.
/// The split happens here rather than at the call sites because the body and
/// its line offset have to come from ONE parse: a caller that passed a body
/// from one parse and an offset from another (or forgot the offset) would
/// store ranges silently short by the frontmatter's height, which is the
/// defect this signature exists to prevent recurring. See
/// `docs/issues/2026-09-02-chunk-line-ranges-are-body-relative-but-published-as-file-lines.md`.
///
/// A document whose frontmatter does not parse is chunked whole, at offset 0.
/// That aligns the two former callers, which disagreed: `index_repo_sync` fell
/// back to an EMPTY body (so a malformed-frontmatter file was chunked into
/// nothing and silently never became searchable), while
/// [`backfill_chunk_vectors`] fell back to the full content. Whole-content is
/// the better of the two — a file with a bad `---` block is still a file
/// someone wants to find.
///
/// Writes the artifact's `artifact_chunk` rows as a side effect, because the
/// chunk ids the queue is keyed on are assigned there — the queue and the rows
/// cannot be built independently without the two disagreeing.
///
/// Empty/whitespace-only chunks are filtered PER CHUNK, not per artifact: the
/// embedder's guard bails the WHOLE batch on a single empty input (see
/// `docs/issues/archive/2026-05-17-reindex-embedding-dim-mismatch.md`), and
/// with N chunks per artifact one blank section would otherwise abort an entire
/// bulk reindex.
///
/// Shared by both enqueue sites in [`index_repo_sync`] — the changed-content
/// path and the forced-re-embed path through the unchanged-row early return —
/// and by [`backfill_chunk_vectors`]. Keeping it in one place is what stops
/// those three from drifting apart.
pub(crate) fn embed_queue_items(
    cat: &Catalog,
    id: &str,
    title: Option<String>,
    doc: &str,
) -> Result<Vec<EmbedQueueItem>> {
    // 2048 chars = 512 tokens. Do NOT swap this for chunk_size_for_model:
    // that returns a CEILING (2048 tokens for CodeRankEmbed), and this project
    // deliberately chunks below it for ranking sharpness. See
    // docs/issues/archive/2026-08-11-chunk-size-for-model-dead-on-production-path.md.
    const CHUNK_CHARS: usize = 512 * 4;

    // One parse, two derived values. `body_line_offset` re-derives the height
    // from the pair rather than being told it, and returns 0 for a pair that is
    // not a suffix match — so the worst this can do is fall back to the old
    // behaviour, never invent a plausible wrong line.
    let (_, body) = frontmatter::parse(doc).unwrap_or((None, doc));
    let line_offset = frontmatter::body_line_offset(doc, body);

    let built = crate::librarian::catalog::chunk::build_chunks(id, body, CHUNK_CHARS, line_offset);
    let stored = crate::librarian::catalog::chunk::replace_chunks(cat, id, &built)?;
    Ok(stored
        .into_iter()
        .filter(|r| !r.content.trim().is_empty())
        .map(|r| {
            // Give a MID-entry chunk its entry's identity. `## W-81 — Choose a
            // gate's surface` may be thousands of characters upstream, so a
            // chunk from the middle of a five-chunk entry would otherwise embed
            // with no idea what it belongs to. Skipped when the chunk already
            // opens with its own heading, which is the common case.
            let text = match &r.entry_token {
                Some(tok) if !r.content.trim_start().starts_with('#') => {
                    format!("{tok}\n\n{}", r.content)
                }
                _ => r.content,
            };
            EmbedQueueItem {
                chunk_id: r.chunk_id,
                // `id` is the artifact this whole call is about. It was being
                // dropped here; carrying it is what lets a backend with no join
                // (Qdrant) key the point by chunk and still say what it belongs to.
                artifact_id: id.to_string(),
                title: title.clone(),
                text,
            }
        })
        .collect())
}

/// Synchronous part of indexing: walk files, upsert artifact rows, collect embedding queue.
/// Returns `(report, embed_queue)` where `embed_queue` is a list of [`EmbedQueueItem`].
///
/// `force_rewalk` bypasses the unchanged-row early-return (metadata is
/// re-derived/re-written even when nothing changed) but does NOT by itself
/// force re-embedding — re-classification alone doesn't need a new vector.
/// `force_embed` is the separate, explicit lever for "queue this file for
/// embedding even though its content hash is unchanged": the backfill case
/// when embeddings are enabled/reconfigured (new model, new backend) for a
/// project that was already indexed without them. Without it, already-indexed
/// unchanged content never gets embedded, silently, forever.
pub fn index_repo_sync(
    cat: &Catalog,
    rules: &[CompiledRule],
    abs_root: &Path,
    ignore: &globset::GlobSet,
    want_embeddings: bool,
    force_rewalk: bool,
    force_embed: bool,
) -> Result<(IndexReport, Vec<EmbedQueueItem>)> {
    let mut report = IndexReport::default();

    // A linked git worktree is a duplicate, stale-on-merge checkout of its main
    // tree — never index it into the (machine-global) catalog. The root-anchored
    // `/.worktrees/` gitignore that excludes it from the MAIN tree does not match
    // when the worktree is itself the walk root, so without this guard every
    // worktree file is indexed as a separate artifact (32b58e13).
    if crate::librarian::current_project::is_linked_worktree(abs_root) {
        tracing::warn!(
            "skipping index of linked git worktree {} — index its main worktree instead",
            abs_root.display()
        );
        return Ok((report, Vec::new()));
    }

    let mut seen_ids: Vec<String> = Vec::new();
    let mut embed_queue: Vec<EmbedQueueItem> = Vec::new();

    // Candidate .md files: the normal ignore-respecting walk, PLUS a
    // supplemental scan for any `[ignored_paths] force_include` patterns
    // declared in <abs_root>/.codescout/project.toml — directories that are
    // gitignore/git-exclude'd from the repo's publish branch but should still
    // be walked into the librarian catalog (e.g. a locally-tracked-only
    // `docs/trackers/`). Deduplicated by path; force_include entries win no
    // priority over the main walk, they just fill in what it skipped.
    let mut seen_paths: std::collections::HashSet<std::path::PathBuf> =
        std::collections::HashSet::new();
    let mut candidate_paths: Vec<std::path::PathBuf> = Vec::new();

    let walker = WalkBuilder::new(abs_root).standard_filters(true).build();
    for entry in walker.flatten() {
        let path = entry.path().to_path_buf();
        if seen_paths.insert(path.clone()) {
            candidate_paths.push(path);
        }
    }

    for path in force_include_candidates(abs_root)? {
        if seen_paths.insert(path.clone()) {
            candidate_paths.push(path);
        }
    }

    for path in &candidate_paths {
        let path = path.as_path();
        if !path.is_file() || path.extension().and_then(|s| s.to_str()) != Some("md") {
            continue;
        }
        let rel = crate::librarian::util::normalize_rel_path(
            &path.strip_prefix(abs_root)?.to_string_lossy(),
        );
        if ignore.is_match(&rel) {
            continue;
        }
        let id = crate::librarian::ids::artifact_id_from_abs(path);
        let bytes = std::fs::read(path)?;
        let content = String::from_utf8_lossy(&bytes);
        let sha = {
            let mut h = Sha256::new();
            h.update(&bytes);
            format!("{:x}", h.finalize())
        };
        let mtime = path
            .metadata()?
            .modified()?
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;

        let existing = artifact::get(cat, &id)?;

        // Always compute classification — rule changes must invalidate prior
        // `kind`/`status` regardless of content staleness.
        let (fm, body) = frontmatter::parse(&content).unwrap_or((None, ""));
        let rule_match = classify(rules, &rel);

        let kind = fm
            .as_ref()
            .and_then(|f| f.kind.clone())
            .or_else(|| rule_match.as_ref().map(|r| r.kind.clone()))
            .unwrap_or_else(|| "unknown".into());
        let status = fm
            .as_ref()
            .and_then(|f| f.status.clone())
            .or_else(|| rule_match.as_ref().and_then(|r| r.status.clone()))
            .unwrap_or_else(|| {
                if kind == "unknown" {
                    "unknown".into()
                } else {
                    "draft".into()
                }
            });
        let time_scope = fm
            .as_ref()
            .and_then(|f| f.time_scope.clone())
            .or_else(|| rule_match.as_ref().and_then(|r| r.time_scope.clone()));
        let confidence = if fm.as_ref().and_then(|f| f.kind.as_ref()).is_some() {
            1.0
        } else {
            0.5
        };
        let title = fm
            .as_ref()
            .and_then(|f| f.title.clone())
            .or_else(|| first_h1(body));
        let owners = fm.as_ref().map(|f| f.owners.clone()).unwrap_or_default();
        // Tags are the union of frontmatter tags and any tags the matching
        // classifier rule contributes. Rule tags never overwrite — they add,
        // so a hand-authored `tags:` list is preserved and augmented.
        let mut tags = fm.as_ref().map(|f| f.tags.clone()).unwrap_or_default();
        if let Some(rm) = rule_match.as_ref() {
            for t in &rm.tags {
                if !tags.contains(t) {
                    tags.push(t.clone());
                }
            }
        }
        let topic = fm.as_ref().and_then(|f| f.topic.clone());

        // Decide whether anything needs writing.
        let content_unchanged = existing
            .as_ref()
            .map(|ex| ex.file_sha256 == sha)
            .unwrap_or(false);
        let meta_unchanged = existing
            .as_ref()
            .map(|ex| {
                ex.kind == kind
                    && ex.status == status
                    && ex.time_scope == time_scope
                    && ex.title == title
                    && ex.owners == owners
                    && ex.tags == tags
                    && ex.topic == topic
                    && (ex.confidence - confidence).abs() < f32::EPSILON as f64
            })
            .unwrap_or(false);

        if !force_rewalk && content_unchanged && meta_unchanged {
            // A forced re-embed must still queue the file even though the row
            // itself needs no rewrite. `force_embed` is documented as being
            // independent of `force_rewalk`, but this early return used to skip
            // the embed branch below outright — which made
            // `librarian(reindex, reembed=true)` a silent no-op on any
            // already-indexed project: it reported `unchanged: N` with
            // `backfill_error_count: 0` while sending the embedder nothing.
            // See docs/issues/archive/2026-07-25-reindex-reembed-noop-without-force.md.
            //
            // The row stays `unchanged` on purpose: nothing about it changed,
            // only its vector needs recomputing. Falling through to the upsert
            // path instead would rewrite every row and misreport them as
            // `updated`.
            if want_embeddings && force_embed {
                embed_queue.extend(embed_queue_items(cat, &id, title, &content)?);
            }
            seen_ids.push(id);
            report.unchanged += 1;
            continue;
        }

        let now = chrono::Utc::now().timestamp_millis();
        let row = ArtifactRow {
            id: id.clone(),
            abs_path: path.to_path_buf(),
            kind: kind.clone(),
            status,
            title: title.clone(),
            owners,
            tags,
            topic,
            time_scope,
            source: Some("repo".into()),
            created_at: existing.as_ref().map(|ex| ex.created_at).unwrap_or(now),
            updated_at: now,
            file_mtime: mtime,
            file_sha256: sha,
            confidence,
        };
        artifact::upsert_and_mint_slug(cat, &row)?;

        // (Re-)embed when content actually changed, OR when the caller
        // explicitly opted into a re-embed backfill via `force_embed` (e.g.
        // embeddings were just enabled/reconfigured for an already-indexed
        // project). Re-classification alone, without either signal, does not
        // require recomputing the embedding.
        if want_embeddings && (!content_unchanged || force_embed) {
            embed_queue.extend(embed_queue_items(cat, &id, title, &content)?);
        }

        seen_ids.push(id.clone());
        if existing.is_some() {
            report.updated += 1;
        } else {
            report.added += 1;
        }
        if kind == "unknown" {
            report.unknown_ids.push(id);
        }
    }

    // Delete rows under abs_root that were not seen in this walk AND whose
    // underlying file is genuinely gone from disk.
    //
    // "Not seen in this walk" alone is NOT equivalent to "file no longer
    // exists": the walker (`ignore::WalkBuilder::standard_filters(true)`)
    // also skips paths matched by `.gitignore`, `.git/info/exclude`, or a
    // global excludesfile — none of which mean the file was deleted. Before
    // this existence check, any reindex over a repo with such an ignore rule
    // (e.g. a `.git/info/exclude` entry for a locally-tracked-but-never-
    // published `docs/trackers/` directory) silently deleted the catalog rows
    // for every file under it on every single reindex, even though the files
    // were sitting right there on disk the whole time (found live, 2026-07-07,
    // debugging Mercury BOM's "reindex succeeds but find/get come back empty"
    // — the docs/trackers/*.md rows were being deleted by this exact path).
    let root_prefix = format!(
        "{}/",
        crate::util::fs::RepoPath::from(abs_root)
            .as_str()
            .replace('\'', "''")
    );
    let candidates: Vec<(String, String)> = if seen_ids.is_empty() {
        cat.conn
            .prepare("SELECT id, abs_path FROM artifact WHERE abs_path LIKE ?1")?
            .query_map(rusqlite::params![format!("{root_prefix}%")], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })?
            .collect::<rusqlite::Result<_>>()?
    } else {
        let placeholders = seen_ids
            .iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", i + 2))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT id, abs_path FROM artifact WHERE abs_path LIKE ?1 AND id NOT IN ({})",
            placeholders
        );
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(format!("{root_prefix}%"))];
        for id in &seen_ids {
            params.push(Box::new(id.clone()));
        }
        cat.conn
            .prepare(&sql)?
            .query_map(
                rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())),
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
            )?
            .collect::<rusqlite::Result<_>>()?
    };

    let mut removed = 0usize;
    for (cand_id, cand_abs_path) in &candidates {
        if !std::path::Path::new(cand_abs_path).exists() {
            cat.conn.execute(
                "DELETE FROM artifact WHERE id = ?1",
                rusqlite::params![cand_id],
            )?;
            removed += 1;
        }
    }
    report.removed = removed;

    // FIX (c) of docs/issues/2026-09-02-indexer-stamps-content-seen-before-it-embeds.md.
    //
    // 729 of codescout's 1373 artifacts were measured with no searchable
    // representation of any kind — 53% of the corpus invisible to semantic
    // search, with no error, no warning, and NO COUNT ANYWHERE reporting it.
    // The stamp/gate ordering above (`:302` writes `file_sha256`, the branch
    // seven lines later reads it) is what puts an artifact there; this is what
    // makes the state observable, and it is the half that must ship whichever
    // reorder lands, because a repair without it leaves the next occurrence as
    // unreconstructable as this one was.
    //
    // `Option`, not a bare count, and the distinction is the whole point: `0`
    // would read identically for "measured, no hole" and "embeddings are off so
    // everything is vectorless and the number is noise". Same reasoning as
    // `embedded` in reindex.rs, which exists because `unchanged: N` could not
    // distinguish work-not-needed from work-skipped-by-mistake.
    // `artifact_chunk`, NOT the vector table, and that choice is load-bearing.
    // `embed_queue_items` writes an artifact's chunk rows as a side effect of
    // QUEUEING it, so "has no chunk rows" means "was never queued" — which is the
    // absorbing state's exact signature, and is true whichever backend the vector
    // later lands in. Counting `artifact_vec_v2` instead would report EVERY
    // artifact as vectorless whenever an external store is configured, because
    // `index_repo` writes to `store.upsert` and leaves that table empty: a false
    // alarm at 100%, which is worse than the silence it replaces.
    //
    // Deliberately NOT detected here: chunks present but the vector write failed.
    // That is a different state — recoverable, non-absorbing — and `reindex`
    // already reports it through `embed_errors` / `embed_note`.
    if want_embeddings {
        let vectorless: i64 = cat.conn.query_row(
            "SELECT COUNT(*) FROM artifact a \
             WHERE a.abs_path LIKE ?1 \
               AND NOT EXISTS ( \
                 SELECT 1 FROM artifact_chunk c WHERE c.artifact_id = a.id \
               )",
            rusqlite::params![format!("{root_prefix}%")],
            |r| r.get(0),
        )?;
        report.vectorless = Some(vectorless.max(0) as usize);
    }

    Ok((report, embed_queue))
}

/// Read `[ignored_paths] force_include` from `<abs_root>/.codescout/project.toml`,
/// if present. Absent file / unparseable TOML / missing key all resolve to
/// "no force-includes" — this is a best-effort opt-in, never a hard error.
///
/// Mirrors `ArtifactBackend::resolve`'s raw-TOML-read pattern (no project.toml
/// struct threading needed here — librarian's `ToolContext` doesn't carry the
/// main server's parsed `IgnoredPathsSection`, and reading it fresh keeps this
/// self-contained and independent of the config-loading path).
fn read_force_include(abs_root: &Path) -> Vec<String> {
    let cfg_path = abs_root.join(".codescout").join("project.toml");
    let Ok(text) = std::fs::read_to_string(&cfg_path) else {
        return Vec::new();
    };
    let Ok(parsed) = toml::from_str::<toml::Value>(&text) else {
        return Vec::new();
    };
    parsed
        .get("ignored_paths")
        .and_then(|t| t.get("force_include"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

/// The literal (non-glob) prefix of a glob pattern — the portion before the
/// first metacharacter. `"docs/trackers/**"` → `"docs/trackers"`;
/// `"docs/trackers"` (no metacharacters at all) → `"docs/trackers"` itself.
/// Used to scope the force_include supplemental walk to a specific
/// subdirectory instead of re-walking the whole repo with ignore files
/// disabled (which would be needlessly slow over large ignored trees like
/// `node_modules`/`.venv` that force_include was never meant to reach).
fn literal_glob_prefix(pattern: &str) -> &str {
    let end = pattern.find(['*', '?', '[', '{']).unwrap_or(pattern.len());
    pattern[..end].trim_end_matches('/')
}

/// Resolve `[ignored_paths] force_include` patterns (if any) into concrete
/// `.md` file paths that the main ignore-respecting walk would have skipped.
///
/// For each pattern, scopes a supplemental walk to its literal directory
/// prefix (see [`literal_glob_prefix`]) with `.gitignore`/`.git/info/exclude`/
/// global-excludesfile checks all disabled — bypassing exactly the mechanism
/// that made these paths invisible to the main walk in the first place — then
/// confirms each candidate file actually matches the full force_include
/// globset before including it (defensive: the anchor directory may contain
/// files the glob itself doesn't cover, e.g. a non-recursive pattern).
fn force_include_candidates(abs_root: &Path) -> Result<Vec<std::path::PathBuf>> {
    let patterns = read_force_include(abs_root);
    if patterns.is_empty() {
        return Ok(Vec::new());
    }
    let force_include_globset = crate::librarian::workspace::compile_ignore(&patterns)?;

    let mut anchors: Vec<String> = patterns
        .iter()
        .map(|p| literal_glob_prefix(p).to_string())
        .filter(|a| !a.is_empty())
        .collect();
    anchors.sort();
    anchors.dedup();

    let mut results = Vec::new();
    for anchor in anchors {
        let anchor_dir = abs_root.join(&anchor);
        if !anchor_dir.is_dir() {
            continue;
        }
        let supplemental = WalkBuilder::new(&anchor_dir)
            .git_ignore(false)
            .git_exclude(false)
            .git_global(false)
            .ignore(false)
            .build();
        for entry in supplemental.flatten() {
            let path = entry.path();
            if !path.is_file() || path.extension().and_then(|s| s.to_str()) != Some("md") {
                continue;
            }
            let Ok(rel_raw) = path.strip_prefix(abs_root) else {
                continue;
            };
            let rel = crate::librarian::util::normalize_rel_path(&rel_raw.to_string_lossy());
            if force_include_globset.is_match(&rel) {
                results.push(path.to_path_buf());
            }
        }
    }
    Ok(results)
}

/// Does this raw `LIBRARIAN_ARTIFACT_VEC_MIGRATE` value opt into the destructive
/// `artifact_vec` rebuild?
///
/// Pure on purpose. The opt-in used to be read from process-global env deep inside
/// `write_embeddings`, which forced tests to mutate that global — the exact
/// `set_var`-in-tests race that `a656f8cec220d347` removed project-wide. Keeping the
/// decision a value means the mapping is testable without touching env at all, and
/// the only untestable line left is the `env::var` read itself.
/// See `docs/issues/archive/2026-07-27-embedder-batch-env-test-race-reintroduces-fixed-ub.md`.
fn migrate_opt_in(raw: Option<&str>) -> bool {
    raw == Some("1")
}

/// Write pre-computed embedding vectors into `artifact_vec`.
///
/// Reads the migration opt-in from the environment once, here at the edge, and
/// hands it to [`write_embeddings_with`] as data. Tests call that function
/// directly with an explicit flag rather than mutating process-global env.
pub fn write_embeddings(cat: &Catalog, embeddings: &[(String, Vec<f32>)]) -> Result<()> {
    let raw = std::env::var(ARTIFACT_VEC_MIGRATE_ENV).ok();
    write_embeddings_with(cat, embeddings, migrate_opt_in(raw.as_deref()))
}

/// Write pre-computed embedding vectors into `artifact_vec`.
///
/// The `vec0` virtual table does not honor `INSERT OR REPLACE` conflict
/// resolution, so we explicitly `DELETE` any existing row for the id before
/// inserting the new embedding. This keeps re-embedding idempotent.
///
/// Validates dimension consistency before any INSERT (F-6b fix per
/// bug-tracker #6): all vectors in the batch must share the same length,
/// and that length must match any existing row in `artifact_vec`. A 1-element
/// vector (the empirical F-6b case — embedder returning an error sentinel)
/// fails here with a clear message instead of at the SQL layer post-DELETE.
///
/// A dimension mismatch against existing rows (e.g. after switching embedding
/// models) is a loud, safe stop unless `allow_dim_migration` — see
/// [`rebuild_artifact_vec_at_dim`] for the explicit, backed-up migration path.
pub fn write_embeddings_with(
    cat: &Catalog,
    embeddings: &[(String, Vec<f32>)],
    allow_dim_migration: bool,
) -> Result<()> {
    use rusqlite::OptionalExtension;

    if embeddings.is_empty() {
        return Ok(());
    }

    // Validate intra-batch dim consistency.
    let batch_dim = embeddings[0].1.len();
    if batch_dim == 0 {
        anyhow::bail!(
            "embedding dim is 0 — embedder produced an empty vector. \
             Likely an embedder misconfiguration or error sentinel returned by \
             the backend. Inspect the embedder service before retrying."
        );
    }
    for (id, vec) in embeddings {
        if vec.len() != batch_dim {
            anyhow::bail!(
                "embedding dim mismatch within batch: id={} expected {} got {}. \
                 Inspect the embedder service — all embeddings in one batch must share \
                 the same dimensionality.",
                id,
                batch_dim,
                vec.len()
            );
        }
    }

    // Validate against existing rows (if any) — the schema's effective dim is
    // pinned by the first inserted row; subsequent inserts must match.
    let existing_blob_len: Option<i64> = cat
        .conn
        .query_row(
            "SELECT length(embedding) FROM artifact_vec LIMIT 1",
            [],
            |r| r.get(0),
        )
        .optional()?;
    if let Some(blob_len) = existing_blob_len {
        // Each f32 takes 4 bytes in the little-endian blob serialization.
        let existing_dim = (blob_len / 4) as usize;
        if batch_dim != existing_dim {
            if !allow_dim_migration {
                anyhow::bail!(
                    "embedding dim mismatch vs catalog: batch={}, existing={}. \
                     Likely causes: (1) embedder is misconfigured and returns error \
                     sentinels with wrong dim (the F-6b case — vec.len()=1), (2) the \
                     configured embedder model changed without a full re-embed pipeline. \
                     To rebuild `artifact_vec` for the new model, set \
                     {ARTIFACT_VEC_MIGRATE_ENV}=1 and retry: this backs up catalog.db, \
                     then drops + recreates artifact_vec at the new dimension. \
                     artifact_vec is a SHARED table (one catalog.db per user, not per \
                     repo) — this destroys existing vectors for EVERY project sharing \
                     this catalog (artifact metadata/files are untouched; a full \
                     reindex regenerates them). Do NOT use `reindex(force=true)` alone \
                     to route around this (bug-tracker #6/#7).",
                    batch_dim,
                    existing_dim
                );
            }
            tracing::warn!(
                "{ARTIFACT_VEC_MIGRATE_ENV}=1: rebuilding artifact_vec {existing_dim}->{batch_dim} \
                 — this deletes vectors for every project sharing this catalog; each will \
                 need a full reindex to regenerate them"
            );
            rebuild_artifact_vec_at_dim(&cat.conn, batch_dim)?;
        }
    }

    for (id, vec) in embeddings {
        let blob: Vec<u8> = vec.iter().flat_map(|f| f.to_le_bytes()).collect();
        cat.conn.execute(
            "DELETE FROM artifact_vec WHERE id = ?1",
            rusqlite::params![id],
        )?;
        cat.conn.execute(
            "INSERT INTO artifact_vec (id, embedding) VALUES (?1, ?2)",
            rusqlite::params![id, blob],
        )?;
    }
    Ok(())
}

/// Drop the artifact id from a `(chunk_id, artifact_id, vector)` batch for the
/// sqlite-vec fallback, which is chunk-keyed and recovers the artifact id by
/// joining `artifact_chunk`.
///
/// The clone is deliberate and cheap next to what produced these vectors: one
/// batch is 100 x 768 f32, and every element of it just came back from an HTTP
/// embedding call. Threading a borrowed form through `write_embeddings_v2`
/// would mean re-shaping its two-pass dimension guard for no measurable gain.
fn pairs_for_sqlite(batch: &[(String, String, Vec<f32>)]) -> Vec<(String, Vec<f32>)> {
    batch
        .iter()
        .map(|(chunk_id, _artifact_id, vector)| (chunk_id.clone(), vector.clone()))
        .collect()
}

/// Write pre-computed CHUNK vectors into `artifact_vec_v2`.
///
/// The v2 table is keyed by `chunk_id`, not `artifact_id` — one row per chunk.
/// As with [`write_embeddings_with`], `vec0` does not honor `INSERT OR REPLACE`
/// conflict resolution, so each id is explicitly `DELETE`d before insert, which
/// keeps re-embedding idempotent.
///
/// Mirrors [`write_embeddings_with`]'s dimension guard in all three parts —
/// zero-dim, intra-batch consistency, and agreement with existing rows — and
/// runs every check BEFORE the first `DELETE`, so a rejected batch is a loud,
/// safe stop rather than a silent partial write. The intra-batch check is the
/// F-6b case: an embedder returning a 1-element error sentinel mixed into an
/// otherwise good batch.
///
/// Unlike v1 there is deliberately no `allow_dim_migration` path. `artifact_vec_v2`
/// is populated by a backfill, so a model change is handled by re-running that
/// rather than by rebuilding the table in place.
pub fn write_embeddings_v2(cat: &Catalog, embeddings: &[(String, Vec<f32>)]) -> Result<()> {
    use rusqlite::OptionalExtension;

    if embeddings.is_empty() {
        return Ok(());
    }

    let batch_dim = embeddings[0].1.len();
    if batch_dim == 0 {
        anyhow::bail!(
            "embedding dim is 0 — embedder produced an empty vector. \
             Likely an embedder misconfiguration or an error sentinel returned by \
             the backend. Inspect the embedder service before retrying."
        );
    }
    for (id, vec) in embeddings {
        if vec.len() != batch_dim {
            anyhow::bail!(
                "embedding dim mismatch within batch: id={} expected {} got {}. \
                 All embeddings in one batch must share the same dimensionality.",
                id,
                batch_dim,
                vec.len()
            );
        }
    }

    let existing_blob_len: Option<i64> = cat
        .conn
        .query_row(
            "SELECT length(embedding) FROM artifact_vec_v2 LIMIT 1",
            [],
            |r| r.get(0),
        )
        .optional()?;
    if let Some(blob_len) = existing_blob_len {
        // Each f32 takes 4 bytes in the little-endian blob serialization.
        let existing_dim = (blob_len / 4) as usize;
        if batch_dim != existing_dim {
            anyhow::bail!(
                "embedding dim mismatch vs artifact_vec_v2: batch={}, existing={}. \
                 The chunk vector table is populated by a backfill — re-run it for the \
                 new model rather than mixing dimensions in one table.",
                batch_dim,
                existing_dim
            );
        }
    }

    for (chunk_id, vec) in embeddings {
        let blob: Vec<u8> = vec.iter().flat_map(|f| f.to_le_bytes()).collect();
        cat.conn.execute(
            "DELETE FROM artifact_vec_v2 WHERE id = ?1",
            rusqlite::params![chunk_id],
        )?;
        cat.conn.execute(
            "INSERT INTO artifact_vec_v2 (id, embedding) VALUES (?1, ?2)",
            rusqlite::params![chunk_id, blob],
        )?;
    }
    Ok(())
}

/// Delete every `artifact_vec_v2` row belonging to `artifact_id`, returning the
/// number removed.
///
/// **This is NOT what collects vectors when an artifact is deleted**, and an
/// earlier draft of the chunk-grain plan said it was. That path is handled by
/// the schema: `artifact_chunk.artifact_id` is `ON DELETE CASCADE`, and the
/// `artifact_vec_v2_cascade_delete` trigger fires `AFTER DELETE ON
/// artifact_chunk` (both in `catalog::apply_migrations_in_txn`), so deleting an
/// artifact already collects its chunk vectors with no help from here.
///
/// This exists for the one caller that removes an artifact's vectors **without
/// deleting the artifact row** — `SqliteVecArtifactStore::delete`. No
/// `artifact_chunk` delete occurs there, so the trigger is unavailable and the
/// chunk ids have to be read out and deleted explicitly.
///
/// It follows that this must run while the chunk rows still exist. That is not
/// a hazard to guard against so much as a statement of when the function is
/// useful at all: once the artifact is gone the trigger has already done the
/// work, and this would find nothing to delete.
pub fn delete_chunk_vectors(cat: &Catalog, artifact_id: &str) -> Result<usize> {
    let ids: Vec<String> = {
        let mut stmt = cat
            .conn
            .prepare("SELECT chunk_id FROM artifact_chunk WHERE artifact_id = ?1")?;
        // Bound to a local before the block ends: returning the collect()
        // directly keeps a temporary alive past `stmt`'s drop (E0597).
        let rows = stmt
            .query_map([artifact_id], |r| r.get(0))?
            .collect::<rusqlite::Result<Vec<String>>>()?;
        rows
    };
    for id in &ids {
        cat.conn.execute(
            "DELETE FROM artifact_vec_v2 WHERE id = ?1",
            rusqlite::params![id],
        )?;
    }
    Ok(ids.len())
}

/// What a [`backfill_chunk_vectors`] run did. Four counts rather than three,
/// because a file that is *gone* and a file that is *empty* are different
/// outcomes with different remedies, and folding them into one number would make
/// the report say "skipped" about two unrelated things.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct BackfillReport {
    /// Chunk vectors written.
    pub embedded: usize,
    /// Artifacts whose body yielded no embeddable chunk (blank, or
    /// frontmatter-only). These are correctly vectorless and will stay so.
    pub skipped_empty: usize,
    /// Artifacts whose file is no longer on disk. A catalog-vs-filesystem drift
    /// this run declines to repair — `librarian(action="reindex")` owns removal.
    pub missing_file: usize,
    /// Artifacts visited.
    pub artifacts: usize,
}

/// `catalog_meta` key holding the resume cursor, as `"<updated_at>|<id>"`.
const BACKFILL_CURSOR: &str = "chunk_backfill_cursor";

/// Give every artifact that has **no chunk rows** a chunked, embedded
/// representation, without going through [`index_repo_sync`] at all.
///
/// **Why it bypasses the indexer rather than calling it.** The artifacts this
/// exists for are exactly the ones the indexer *declines to process*: their
/// content is stamped as seen, so `content_unchanged` is true and the embed is
/// skipped — an absorbing state
/// (`docs/issues/2026-09-02-indexer-stamps-content-seen-before-it-embeds.md`).
/// A backfill routed through the normal walk would inherit that gate and report
/// success having done nothing. This one consults `content_unchanged` NOWHERE.
///
/// **What it does not fix.** The stamp/gate ordering is untouched, so this
/// empties the hole without closing it: a later run can dig a new one. That is
/// what `IndexReport::vectorless` is for, and it is why Task 11's *swap* stays
/// blocked while this half does not — filling a hole is safe; renaming the table
/// out from under a still-broken writer is not.
///
/// **Resumable**, because a full corpus is ~90k chunks of remote round-trips and
/// an interrupted run must not start over. The cursor is `(updated_at, id)` —
/// `updated_at` alone is not unique, and a cursor that is not a total order
/// silently skips every artifact sharing a timestamp with the last one written.
/// It advances only after a batch is durably written.
///
/// Takes the `Mutex` rather than a `&Catalog` so the lock can be RELEASED across
/// each embedding await. Holding it for the whole run would work in the one-shot
/// CLI and would be wrong anywhere else, and a signature that only works in one
/// caller is a trap for the second one.
///
/// `batch` is the flush size; 100 matches [`index_repo`]'s own flush.
pub async fn backfill_chunk_vectors(
    catalog: &parking_lot::Mutex<Catalog>,
    svc: &crate::librarian::embedding::EmbeddingService,
    batch: usize,
) -> Result<BackfillReport> {
    use crate::librarian::catalog::gc::{get_meta, set_meta};

    let flush = batch.max(1);
    let mut report = BackfillReport::default();

    let (mut cur_updated, mut cur_id) = {
        let cat = catalog.lock();
        match get_meta(&cat.conn, BACKFILL_CURSOR)? {
            Some(s) => match s.split_once('|') {
                Some((u, i)) => (u.parse::<i64>().unwrap_or(i64::MIN), i.to_string()),
                None => (i64::MIN, String::new()),
            },
            None => (i64::MIN, String::new()),
        }
    };

    let mut pending: Vec<(String, Vec<f32>)> = Vec::with_capacity(flush);

    loop {
        // One page of vectorless artifacts past the cursor. Re-queried each round
        // rather than collected up front: the page is small, and holding a
        // 700-row snapshot across awaits would go stale against concurrent
        // writers on a shared catalog.
        let page: Vec<(String, String, Option<String>, i64)> = {
            let cat = catalog.lock();
            let mut stmt = cat.conn.prepare(
                "SELECT a.id, a.abs_path, a.title, a.updated_at FROM artifact a \
                 WHERE NOT EXISTS ( \
                   SELECT 1 FROM artifact_chunk c WHERE c.artifact_id = a.id \
                 ) \
                 AND (a.updated_at > ?1 OR (a.updated_at = ?1 AND a.id > ?2)) \
                 ORDER BY a.updated_at, a.id LIMIT ?3",
            )?;
            let rows = stmt
                .query_map(rusqlite::params![cur_updated, cur_id, flush as i64], |r| {
                    Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            rows
        };
        if page.is_empty() {
            break;
        }

        for (id, abs_path, title, updated_at) in page {
            report.artifacts += 1;
            cur_updated = updated_at;
            cur_id = id.clone();

            let content = match std::fs::read_to_string(&abs_path) {
                Ok(c) => c,
                Err(_) => {
                    report.missing_file += 1;
                    continue;
                }
            };
            // The whole file, frontmatter included — embed_queue_items owns
            // the split now, so this path and index_repo_sync's cannot land
            // chunks in two different coordinate spaces.
            let items = {
                let cat = catalog.lock();
                embed_queue_items(&cat, &id, title, &content)?
            };
            if items.is_empty() {
                report.skipped_empty += 1;
                continue;
            }

            let futures_iter = items.into_iter().map(|item| async move {
                let vec = svc
                    .embed_artifact(item.title.as_deref(), &item.text)
                    .await?;
                anyhow::Ok((item.chunk_id, vec))
            });
            let mut s = stream::iter(futures_iter).buffer_unordered(EMBED_CONCURRENCY);
            while let Some(res) = s.next().await {
                pending.push(res?);
            }
        }

        // Write, THEN advance the cursor: the other order loses a batch on a
        // crash between the two and calls the range done.
        if pending.len() >= flush {
            let cat = catalog.lock();
            write_embeddings_v2(&cat, &pending)?;
            report.embedded += pending.len();
            pending.clear();
        }
        {
            let cat = catalog.lock();
            set_meta(
                &cat.conn,
                BACKFILL_CURSOR,
                &format!("{cur_updated}|{cur_id}"),
            )?;
        }
    }

    if !pending.is_empty() {
        let cat = catalog.lock();
        write_embeddings_v2(&cat, &pending)?;
        report.embedded += pending.len();
    }
    // A completed run CLEARS the cursor, and this is not tidiness. Left at the
    // end of the corpus, the next run's `updated_at > cursor` predicate would
    // skip any artifact that becomes vectorless later with an earlier timestamp —
    // which is precisely what a re-formed hole looks like, since the trap is
    // entered by content the walk considers old. The cursor's job is to make one
    // run terminate over rows it cannot fix (empty bodies and missing files keep
    // no chunk rows, so `NOT EXISTS` re-selects them forever); it is not a
    // watermark over the corpus's lifetime.
    {
        let cat = catalog.lock();
        set_meta(&cat.conn, BACKFILL_CURSOR, "")?;
    }
    Ok(report)
}

/// Opt-in gate for [`rebuild_artifact_vec_at_dim`]. Default OFF — a dimension
/// mismatch is a loud, safe stop by default (see F-6b/F-9 history in
/// `docs/archive/old-trackers/bug-tracker.md` and
/// `docs/trackers/archive/artifact-code-linkage-session-log.md`); this env var
/// is the explicit, backed-up escape hatch for a deliberate embedder-model
/// change, never a silent auto-heal.
const ARTIFACT_VEC_MIGRATE_ENV: &str = "LIBRARIAN_ARTIFACT_VEC_MIGRATE";

/// Back up `catalog.db` (if file-backed), then drop + recreate `artifact_vec`
/// at `new_dim`.
///
/// Only called from [`write_embeddings`] when `LIBRARIAN_ARTIFACT_VEC_MIGRATE=1`
/// is set AND a dimension mismatch was detected — a deliberate, opt-in
/// migration for "I changed my embedding model", not an automatic silent-heal
/// path. `artifact_vec` is a shared, cross-project table (one catalog.db per
/// user, not per-repo), so this affects every project using this catalog —
/// the caller logs a loud warning either way.
///
/// Reuses the exact `DROP`+`CREATE VIRTUAL TABLE ... USING vec0(...)` shape
/// from `schema.sql`, just with `new_dim` substituted for the column width —
/// safe to interpolate directly since it is a `usize` computed from an
/// embedder's own vector length, never user input. `CREATE VIRTUAL TABLE IF
/// NOT EXISTS` in `schema.sql` no-ops once the table exists, regardless of its
/// actual column width, so the new dimension survives future `Catalog::open`
/// calls (no permanent change to `schema.sql`'s public default of `FLOAT[768]`
/// is needed for this to persist).
///
/// Backup mirrors the existing v6-migration `backup_db` pattern: a timestamped
/// sibling file, `catalog.db.pre-vec-dim-bak.<unix_ts>`. In-memory catalogs
/// (`conn.path()` empty, i.e. [`Catalog::open_in_memory`]) skip the backup —
/// there is no file to copy.
fn rebuild_artifact_vec_at_dim(conn: &rusqlite::Connection, new_dim: usize) -> Result<()> {
    if let Some(path) = conn.path().filter(|p| !p.is_empty()) {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let db_path = std::path::Path::new(path);
        let bak = db_path.with_extension(format!("db.pre-vec-dim-bak.{ts}"));
        std::fs::copy(db_path, &bak).with_context(|| {
            format!(
                "backing up catalog before artifact_vec dimension migration: {} -> {}",
                db_path.display(),
                bak.display()
            )
        })?;
        tracing::warn!(
            "artifact_vec dimension migration: backup created at {} before rebuilding at dim={new_dim}",
            bak.display()
        );
    }
    conn.execute_batch(&format!(
        "DROP TABLE IF EXISTS artifact_vec; \
         CREATE VIRTUAL TABLE artifact_vec USING vec0(id TEXT PRIMARY KEY, embedding FLOAT[{new_dim}]);"
    ))
    .context("rebuilding artifact_vec at new dimension")?;
    Ok(())
}

use futures::stream::{self, StreamExt};

const EMBED_CONCURRENCY: usize = 8;

/// High-level async entry point: sync walk + optional async embedding with bounded concurrency.
/// Embedding calls are streamed with `buffer_unordered(EMBED_CONCURRENCY)` so up to 8 remote
/// round-trips run in parallel, and vectors are flushed to SQLite in chunks of 100 to cap memory.
pub async fn index_repo(
    cat: &Catalog,
    rules: &[CompiledRule],
    abs_root: &Path,
    ignore: &globset::GlobSet,
    embedding: Option<&crate::librarian::embedding::EmbeddingService>,
    // Vector backend + the artifact's project_id. `None` store → legacy
    // sqlite-vec write via `write_embeddings` (the offline default).
    store: Option<&dyn crate::librarian::artifact_store::ArtifactVectorStore>,
    project_id: &str,
) -> Result<IndexReport> {
    let want = embedding.is_some();
    let (mut report, embed_queue) =
        index_repo_sync(cat, rules, abs_root, ignore, want, false, false)?;

    if let Some(svc) = embedding {
        let futures_iter = embed_queue.into_iter().map(|item| async move {
            let vec = svc
                .embed_artifact(item.title.as_deref(), &item.text)
                .await?;
            anyhow::Ok((item.chunk_id, item.artifact_id, vec))
        });
        let mut stream = stream::iter(futures_iter).buffer_unordered(EMBED_CONCURRENCY);
        // (chunk_id, artifact_id, vector) — both ids ride to the store, because
        // a backend with no join cannot recover the second one.
        let mut batch: Vec<(String, String, Vec<f32>)> = Vec::with_capacity(100);
        while let Some(res) = stream.next().await {
            batch.push(res?);
            if batch.len() >= 100 {
                if let Some(s) = store {
                    for (chunk_id, artifact_id, vec) in &batch {
                        s.upsert(project_id, chunk_id, artifact_id, vec).await?;
                    }
                } else {
                    write_embeddings_v2(cat, &pairs_for_sqlite(&batch))?;
                }
                report.embedded += batch.len();
                batch.clear();
            }
        }
        if !batch.is_empty() {
            report.embedded += batch.len();
            if let Some(s) = store {
                for (chunk_id, artifact_id, vec) in &batch {
                    s.upsert(project_id, chunk_id, artifact_id, vec).await?;
                }
            } else {
                write_embeddings_v2(cat, &pairs_for_sqlite(&batch))?;
            }
        }
    }

    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::classify::load_rules;
    use std::path::PathBuf;

    #[test]
    fn indexes_fixture_repo_with_mixed_classifications() {
        let cat = Catalog::open_in_memory().unwrap();
        let rules = load_rules(
            r#"
[[rule]]
glob = "**/docs/superpowers/specs/*.md"
kind = "spec"
status = "active"

[[rule]]
glob = "**/docs/research/*.md"
kind = "memory"
"#,
        )
        .unwrap();
        let ignore = globset::GlobSet::empty();
        let fixture =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/librarian/fixtures/repo_a");
        let (report, _) =
            index_repo_sync(&cat, &rules, &fixture, &ignore, false, false, false).unwrap();
        assert_eq!(report.added, 3, "should index 3 .md files");
        assert_eq!(report.unknown_ids.len(), 1, "README.md is unknown");

        let (r2, _) =
            index_repo_sync(&cat, &rules, &fixture, &ignore, false, false, false).unwrap();
        assert_eq!(r2.unchanged, 3);
        assert_eq!(r2.added, 0);
    }

    #[test]
    fn reindex_mints_a_slug_for_every_newly_indexed_row() {
        let cat = Catalog::open_in_memory().unwrap();
        let rules = load_rules(
            r#"
[[rule]]
glob = "**/docs/superpowers/specs/*.md"
kind = "spec"
status = "active"

[[rule]]
glob = "**/docs/research/*.md"
kind = "memory"
"#,
        )
        .unwrap();
        let ignore = globset::GlobSet::empty();
        let fixture =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/librarian/fixtures/repo_a");
        index_repo_sync(&cat, &rules, &fixture, &ignore, false, false, false).unwrap();

        let without_slug: i64 = cat
            .conn
            .query_row(
                "SELECT COUNT(*) FROM artifact WHERE slug IS NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            without_slug, 0,
            "every row the walk touched must have a slug minted, not left for a manual backfill"
        );
    }

    /// A working embedder for the backfill tests. Constant vector — the
    /// backfill's contract is *which rows it reaches*, never ranking. 768 dims
    /// because `artifact_vec_v2` is declared `FLOAT[768]` and sqlite-vec rejects
    /// anything else at INSERT, which is a loud failure and not a silent one.
    fn backfill_svc() -> crate::librarian::embedding::EmbeddingService {
        struct OkEmbedder;
        #[async_trait::async_trait]
        impl codescout_embed::Embedder for OkEmbedder {
            fn dimensions(&self) -> usize {
                768
            }
            async fn embed(&self, texts: &[&str]) -> anyhow::Result<Vec<Vec<f32>>> {
                Ok(texts
                    .iter()
                    .map(|_| {
                        let mut v = vec![0.0f32; 768];
                        v[0] = 1.0;
                        v
                    })
                    .collect())
            }
        }
        crate::librarian::embedding::EmbeddingService::new(std::sync::Arc::new(OkEmbedder))
    }

    fn md_rules() -> Vec<CompiledRule> {
        load_rules("[[rule]]\nglob = \"**/*.md\"\nkind = \"doc\"\n").unwrap()
    }

    /// **The backfill's whole reason to exist: it escapes a state the indexer
    /// cannot.**
    ///
    /// The absorbing state is REPRODUCED, not simulated — index with embeddings
    /// off (stamps `file_sha256`, writes nothing), then on (the row is now
    /// `content_unchanged`, so the early return skips the embed). The test then
    /// runs `index_repo_sync` once more and asserts it STILL refuses, in the same
    /// test: without that control, a backfill doing exactly what an ordinary
    /// reindex would have done looks identical and is not worth a function.
    #[tokio::test]
    async fn the_backfill_embeds_what_the_indexer_permanently_refuses_to() {
        let m = parking_lot::Mutex::new(Catalog::open_in_memory().unwrap());
        let rules = md_rules();
        let ignore = globset::GlobSet::empty();
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(
            tmp.path().join("ledger.md"),
            "# L\n\nintro\n\n## W-1 — a\n\nalpha\n\n## W-2 — b\n\nbeta\n",
        )
        .unwrap();

        index_repo_sync(&m.lock(), &rules, tmp.path(), &ignore, false, false, false).unwrap();
        let (trapped, q) =
            index_repo_sync(&m.lock(), &rules, tmp.path(), &ignore, true, false, false).unwrap();
        assert_eq!(trapped.vectorless, Some(1), "fixture must be in the trap");
        assert!(q.is_empty(), "and the indexer must be refusing it");

        let report = backfill_chunk_vectors(&m, &backfill_svc(), 100)
            .await
            .unwrap();
        assert_eq!(report.artifacts, 1);
        assert_eq!(report.skipped_empty, 0);
        assert_eq!(report.missing_file, 0);
        assert!(
            report.embedded >= 3,
            "the ledger has a preamble plus two entries; the backfill must write a \
             vector PER CHUNK, not one per artifact — got {}",
            report.embedded
        );

        let vecs: i64 = m
            .lock()
            .conn
            .query_row("SELECT COUNT(*) FROM artifact_vec_v2", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            vecs as usize, report.embedded,
            "the report must not overstate what landed"
        );

        // The control: the indexer is STILL refusing. The backfill emptied the
        // hole; it did not close it — which is exactly why the swap stays blocked.
        let (after, q2) =
            index_repo_sync(&m.lock(), &rules, tmp.path(), &ignore, true, false, false).unwrap();
        assert_eq!(after.vectorless, Some(0), "the hole is empty");
        assert!(
            q2.is_empty(),
            "and the indexer still declines to queue it — the ORDERING defect is \
             untouched, so a new artifact can still fall in"
        );
    }

    /// An empty body and a missing file are different outcomes with different
    /// remedies, and a single `skipped` count would say the same word about both.
    /// Neither is an error, and neither may be counted as `embedded`.
    #[tokio::test]
    async fn a_blank_artifact_and_a_vanished_one_are_counted_apart() {
        let m = parking_lot::Mutex::new(Catalog::open_in_memory().unwrap());
        let rules = md_rules();
        let ignore = globset::GlobSet::empty();
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("blank.md"), "---\ntitle: B\n---\n").unwrap();
        std::fs::write(tmp.path().join("gone.md"), "# G\n\nbody\n").unwrap();

        index_repo_sync(&m.lock(), &rules, tmp.path(), &ignore, false, false, false).unwrap();
        std::fs::remove_file(tmp.path().join("gone.md")).unwrap();

        let report = backfill_chunk_vectors(&m, &backfill_svc(), 100)
            .await
            .unwrap();
        assert_eq!(report.artifacts, 2);
        assert_eq!(report.embedded, 0, "neither had anything to embed");
        assert_eq!(report.skipped_empty, 1, "the blank one");
        assert_eq!(report.missing_file, 1, "the deleted one");
    }

    /// **A completed run must clear its cursor.** Parked at the end of the
    /// corpus, the next run's `updated_at > cursor` predicate skips any artifact
    /// that becomes vectorless later with an EARLIER timestamp — which is exactly
    /// what a re-formed hole looks like, since the trap is entered by content the
    /// walk considers old.
    ///
    /// This is the assertion a cursor-as-watermark implementation fails, and it
    /// fails SILENTLY: the second run returns `artifacts: 0, embedded: 0`, which
    /// reads as "nothing to do" rather than as a skip.
    #[tokio::test]
    async fn a_second_run_still_finds_an_artifact_that_fell_in_afterwards() {
        let m = parking_lot::Mutex::new(Catalog::open_in_memory().unwrap());
        let rules = md_rules();
        let ignore = globset::GlobSet::empty();
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("a.md"), "# A\n\nalpha\n").unwrap();
        std::fs::write(tmp.path().join("b.md"), "# B\n\nbeta\n").unwrap();
        index_repo_sync(&m.lock(), &rules, tmp.path(), &ignore, false, false, false).unwrap();

        let first = backfill_chunk_vectors(&m, &backfill_svc(), 100)
            .await
            .unwrap();
        assert_eq!(first.artifacts, 2);
        assert!(first.embedded >= 2);

        // Both fall back in, keeping their ORIGINAL `updated_at` — behind
        // wherever a watermark cursor would have been parked.
        {
            let cat = m.lock();
            cat.conn.execute("DELETE FROM artifact_chunk", []).unwrap();
            cat.conn.execute("DELETE FROM artifact_vec_v2", []).unwrap();
        }

        let second = backfill_chunk_vectors(&m, &backfill_svc(), 100)
            .await
            .unwrap();
        assert_eq!(
            second.artifacts, 2,
            "a run that parks its cursor at the end of the corpus reports 0 here, and \
             0 reads as 'nothing to do' rather than as a skip"
        );
        assert!(second.embedded >= 2, "and it must actually re-embed them");
    }

    /// **The absorbing state, reproduced end to end and then COUNTED.**
    ///
    /// Run 1 indexes with embeddings OFF: `file_sha256` is stamped and no vector
    /// is written. Run 2 turns embeddings ON — and the artifact is now
    /// `content_unchanged`, so the unchanged-row early return skips the embed
    /// (its branch is gated on `force_embed`, which every automatic caller passes
    /// as `false`). Nothing errors, nothing warns, and no ordinary reindex will
    /// ever give this artifact a vector.
    ///
    /// That is the trap in
    /// `docs/issues/2026-09-02-indexer-stamps-content-seen-before-it-embeds.md`,
    /// entered through its first candidate path — a run with no embedder. Before
    /// fix (c) the only difference between this catalog and a healthy one was
    /// INVISIBLE: both report `unchanged: 1`, both report no error.
    ///
    /// LOAD-BEARING that run 1 asserts `None` and not `Some(0)`. A run with
    /// embeddings off has measured nothing, and a bare `usize` would report `0` —
    /// indistinguishable from a clean corpus, which is the exact confusion this
    /// field exists to prevent.
    #[test]
    fn an_artifact_stamped_without_a_vector_is_counted_rather_than_silent() {
        let cat = Catalog::open_in_memory().unwrap();
        let rules = load_rules("[[rule]]\nglob = \"**/*.md\"\nkind = \"doc\"\n").unwrap();
        let ignore = globset::GlobSet::empty();
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("note.md"), "# Note\n\nbody\n").unwrap();

        // Run 1 — embeddings off. Stamps the content, writes no vector.
        let (r1, q1) =
            index_repo_sync(&cat, &rules, tmp.path(), &ignore, false, false, false).unwrap();
        assert_eq!(r1.added, 1);
        assert!(q1.is_empty(), "embeddings off must queue nothing");
        assert_eq!(
            r1.vectorless, None,
            "a run that did not want embeddings measured nothing, and must not report \
             a number that reads as a clean corpus"
        );

        // Run 2 — embeddings on. The row is unchanged, so the early return skips
        // the embed and the artifact is now permanently unembeddable.
        let (r2, q2) =
            index_repo_sync(&cat, &rules, tmp.path(), &ignore, true, false, false).unwrap();
        assert_eq!(r2.unchanged, 1);
        assert!(
            q2.is_empty(),
            "this IS the trap: unchanged content is not queued, so the artifact stays \
             vectorless and nothing says so"
        );
        assert_eq!(
            r2.vectorless,
            Some(1),
            "the hole must be counted — it is the only observable difference between \
             this catalog and a healthy one"
        );

        // Run 3 — the documented escape hatch (`reembed=true` sets `force_embed`)
        // queues the trapped artifact, which is what makes the count actionable
        // rather than merely alarming.
        let (r3, q3) =
            index_repo_sync(&cat, &rules, tmp.path(), &ignore, true, false, true).unwrap();
        assert!(
            !q3.is_empty(),
            "force_embed must queue the trapped artifact"
        );
        assert_eq!(
            r3.vectorless,
            Some(0),
            "queueing IS the escape, and the count says so: `embed_queue_items` writes \
             the chunk rows, so the artifact has left the absorbing state. Whether the \
             vector then lands is a separate, RECOVERABLE failure that `reindex` \
             reports through `embed_errors` — conflating the two would make a \
             transient embedder outage look permanent."
        );
    }

    #[test]
    fn index_repo_sync_skips_linked_worktree() {
        let tmp = tempfile::TempDir::new().unwrap();
        let wt = tmp.path().join("wt");
        std::fs::create_dir_all(wt.join("docs")).unwrap();
        // .git as a FILE pointing into .../worktrees/<name> → linked worktree.
        std::fs::write(
            wt.join(".git"),
            format!(
                "gitdir: {}/main/.git/worktrees/feat\n",
                tmp.path().display()
            ),
        )
        .unwrap();
        std::fs::write(wt.join("docs/a.md"), "# a\n").unwrap();

        let cat = Catalog::open_in_memory().unwrap();
        let rules: Vec<CompiledRule> = Vec::new();
        let ignore = globset::GlobSet::empty();
        let (report, queue) =
            index_repo_sync(&cat, &rules, &wt, &ignore, false, false, false).unwrap();
        assert_eq!(report.added, 0, "a linked worktree must not be indexed");
        assert!(queue.is_empty());
        let n: i64 = cat
            .conn
            .query_row("SELECT COUNT(*) FROM artifact", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0, "no artifact rows created for the worktree");
    }

    #[test]
    fn index_removes_deleted_files() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join("docs/specs")).unwrap();
        std::fs::write(root.join("docs/specs/a.md"), "# a\n").unwrap();
        std::fs::write(root.join("docs/specs/b.md"), "# b\n").unwrap();

        let cat = Catalog::open_in_memory().unwrap();
        let rules = crate::librarian::classify::load_rules(
            "[[rule]]\nglob = \"**/docs/specs/*.md\"\nkind = \"spec\"\n",
        )
        .unwrap();
        let ignore = globset::GlobSet::empty();

        let (r1, _) = index_repo_sync(&cat, &rules, root, &ignore, false, false, false).unwrap();
        assert_eq!(r1.added, 2);

        std::fs::remove_file(root.join("docs/specs/b.md")).unwrap();
        let (r2, _) = index_repo_sync(&cat, &rules, root, &ignore, false, false, false).unwrap();
        assert_eq!(r2.removed, 1);
    }

    #[test]
    fn index_does_not_delete_still_existing_file_newly_matched_by_ignore() {
        // Found live (2026-07-07) debugging Mercury BOM's "reindex succeeds
        // but find/get come back empty": a `.git/info/exclude` entry for
        // docs/trackers/ made the walker (standard_filters) silently skip
        // that whole directory, and the orphan-cleanup then deleted every
        // row under it on every reindex, purely because it was "not seen in
        // this walk" — even though the files were sitting right there on
        // disk. A file must survive reindex as long as it still exists,
        // regardless of WHY the walker didn't visit it this pass.
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join("docs/trackers")).unwrap();
        std::fs::write(root.join("docs/trackers/a.md"), "# a\nbody\n").unwrap();

        let cat = Catalog::open_in_memory().unwrap();
        let rules = crate::librarian::classify::load_rules(
            "[[rule]]\nglob = \"**/docs/trackers/*.md\"\nkind = \"tracker\"\n",
        )
        .unwrap();

        // Pass 1: no ignore rule yet — the file gets indexed normally.
        let no_ignore = globset::GlobSet::empty();
        let (r1, _) = index_repo_sync(&cat, &rules, root, &no_ignore, false, false, false).unwrap();
        assert_eq!(r1.added, 1);
        let id = crate::librarian::ids::artifact_id_from_abs(&root.join("docs/trackers/a.md"));
        assert!(crate::librarian::catalog::artifact::get(&cat, &id)
            .unwrap()
            .is_some());

        // Pass 2: an ignore rule now matches the SAME file (simulating a
        // .git/info/exclude entry the standard_filters walker would also
        // respect) — the file is not walked this pass, but it still exists
        // on disk. Its row must survive.
        let now_ignored =
            crate::librarian::workspace::compile_ignore(&["**/docs/trackers/**".to_string()])
                .unwrap();
        let (r2, _) =
            index_repo_sync(&cat, &rules, root, &now_ignored, false, false, false).unwrap();
        assert_eq!(
            r2.removed, 0,
            "a file that still exists must not be deleted just because this \
             walk didn't visit it"
        );
        assert!(
            crate::librarian::catalog::artifact::get(&cat, &id)
                .unwrap()
                .is_some(),
            "row for the still-existing, now-ignored file must survive"
        );
    }

    #[test]
    fn force_include_recovers_files_hidden_by_gitignore() {
        // Companion to the orphan-cleanup fix above: that fix stops
        // deletion, but the walker still never VISITS an ignored
        // directory, so a never-before-indexed file under it stays
        // invisible forever without an explicit opt-in. `[ignored_paths]
        // force_include` in `.codescout/project.toml` is that opt-in —
        // this proves it actually works end to end (config → walk →
        // catalog row), not just that it's a no-op key some other
        // session assumed existed (Mercury BOM's project.toml already had
        // it, silently doing nothing, before this feature existed).
        //
        // Uses a plain `.ignore` file (the `ignore` crate's own,
        // git-independent ignore mechanism) rather than `.gitignore` /
        // `.git/info/exclude` — those require an actual `.git` directory
        // to be detected before `WalkBuilder` will honor them, which
        // `.ignore` does not. The real Mercury BOM bug used
        // `.git/info/exclude`; the earlier
        // `index_does_not_delete_still_existing_file_newly_matched_by_ignore`
        // test already covers that specific mechanism via a custom deny
        // globset. This test only needs SOME walker-level exclusion that
        // is independent of the `ignore: &GlobSet` deny-list parameter
        // (force_include candidates are still filtered by that param —
        // see `index_repo_sync` — so it can't double as the "hidden"
        // mechanism here).
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join("docs/trackers")).unwrap();
        std::fs::write(root.join("docs/trackers/a.md"), "# a\nbody\n").unwrap();
        std::fs::write(root.join(".ignore"), "docs/trackers/\n").unwrap();

        let cat = Catalog::open_in_memory().unwrap();
        let rules = crate::librarian::classify::load_rules("").unwrap();
        let no_ignore = globset::GlobSet::empty();

        // Baseline: no project.toml at all — the .ignore'd file is
        // never walked, so it's never indexed in the first place.
        let (r1, _) = index_repo_sync(&cat, &rules, root, &no_ignore, false, false, false).unwrap();
        assert_eq!(
            r1.added, 0,
            "ignore'd file must be invisible without force_include"
        );

        // Opt in via project.toml — same shape as Mercury BOM's existing
        // (previously inert) config.
        std::fs::create_dir_all(root.join(".codescout")).unwrap();
        std::fs::write(
            root.join(".codescout").join("project.toml"),
            "[ignored_paths]\nforce_include = [\"docs/trackers\", \"docs/trackers/**\"]\n",
        )
        .unwrap();

        let (r2, _) = index_repo_sync(&cat, &rules, root, &no_ignore, false, false, false).unwrap();
        assert_eq!(r2.added, 1, "force_include must recover the ignore'd file");
        let id = crate::librarian::ids::artifact_id_from_abs(&root.join("docs/trackers/a.md"));
        assert!(crate::librarian::catalog::artifact::get(&cat, &id)
            .unwrap()
            .is_some());
    }

    #[test]
    fn reindex_refreshes_stale_metadata() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join("docs/specs")).unwrap();
        let path = root.join("docs/specs/a.md");
        std::fs::write(&path, "---\ntitle: Original\n---\nbody\n").unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        let rules = crate::librarian::classify::load_rules(
            "[[rule]]\nglob = \"**/docs/specs/*.md\"\nkind = \"spec\"\n",
        )
        .unwrap();
        let ignore = globset::GlobSet::empty();
        index_repo_sync(&cat, &rules, root, &ignore, false, false, false).unwrap();
        let id = crate::librarian::ids::artifact_id_from_abs(&root.join("docs/specs/a.md"));

        // 1. Baseline
        let before = crate::librarian::catalog::artifact::get(&cat, &id)
            .unwrap()
            .unwrap();
        assert_eq!(before.title.as_deref(), Some("Original"));

        // 2. Mutate file on disk (NOT via our API).
        std::fs::write(&path, "---\ntitle: Updated\n---\nbody\n").unwrap();

        // 3. Assert stale.
        let stale = crate::librarian::catalog::artifact::get(&cat, &id)
            .unwrap()
            .unwrap();
        assert_eq!(
            stale.title.as_deref(),
            Some("Original"),
            "must be stale before reindex"
        );

        // 4. Reindex.
        index_repo_sync(&cat, &rules, root, &ignore, false, false, false).unwrap();

        // 5. Fresh.
        let fresh = crate::librarian::catalog::artifact::get(&cat, &id)
            .unwrap()
            .unwrap();
        assert_eq!(fresh.title.as_deref(), Some("Updated"));
    }

    // FINDING C ANNOTATION (post-Task-6): this fixture's body has no blank line
    // after the frontmatter, so it happens to yield exactly one chunk — the
    // `count == 1` assertion below passes for that reason, not because
    // embed_queue_items/write_embeddings only ever produce one vector per
    // artifact. It does NOT cover a multi-chunk body; on one, this test would
    // fail for the wrong reason (a real N-to-1 regression looks identical to
    // this passing for the right reason). Task 7/8 own re-pointing this area.
    #[tokio::test]
    async fn embeds_artifact_into_vec_table() {
        use crate::librarian::embedding::EmbeddingService;
        use async_trait::async_trait;
        use codescout_embed::{Embedder, Embedding};
        use std::sync::Arc;

        struct MockEmbedder;

        #[async_trait]
        impl Embedder for MockEmbedder {
            fn dimensions(&self) -> usize {
                768
            }
            async fn embed(&self, texts: &[&str]) -> anyhow::Result<Vec<Embedding>> {
                Ok(texts.iter().map(|_| vec![0.1f32; 768]).collect())
            }
        }

        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join("docs/specs")).unwrap();
        std::fs::write(
            root.join("docs/specs/a.md"),
            "---\ntitle: Test\n---\n# Body\n\nSome content.\n",
        )
        .unwrap();

        let cat = Catalog::open_in_memory().unwrap();
        let rules = crate::librarian::classify::load_rules(
            "[[rule]]\nglob = \"**/docs/specs/*.md\"\nkind = \"spec\"\n",
        )
        .unwrap();
        let ignore = globset::GlobSet::empty();

        let svc = EmbeddingService::new(Arc::new(MockEmbedder));

        // Phase 1: sync walk
        let (report, embed_queue) =
            index_repo_sync(&cat, &rules, root, &ignore, true, false, false).unwrap();
        assert_eq!(report.added, 1);

        // Phase 2: embed
        let mut computed: Vec<(String, Vec<f32>)> = Vec::new();
        for item in &embed_queue {
            let vec = svc
                .embed_artifact(item.title.as_deref(), &item.text)
                .await
                .unwrap();
            computed.push((item.chunk_id.clone(), vec));
        }

        // Phase 3: write
        write_embeddings(&cat, &computed).unwrap();

        let count: i64 = cat
            .conn
            .query_row("SELECT count(*) FROM artifact_vec", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1, "embedding should be written to artifact_vec");
    }

    #[test]
    fn rule_change_reclassifies_existing_rows_without_content_change() {
        // BUG-046: reindex after rule change must update kind/status on rows
        // whose content (SHA) did not change.
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join("docs/trackers")).unwrap();
        let path = root.join("docs/trackers/foo.md");
        std::fs::write(&path, "# Foo\nbody\n").unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        let ignore = globset::GlobSet::empty();
        let id = crate::librarian::ids::artifact_id_from_abs(&root.join("docs/trackers/foo.md"));

        // 1. Index with no matching rules → kind=unknown.
        let no_rules = crate::librarian::classify::load_rules("").unwrap();
        index_repo_sync(&cat, &no_rules, root, &ignore, false, false, false).unwrap();
        let before = crate::librarian::catalog::artifact::get(&cat, &id)
            .unwrap()
            .unwrap();
        assert_eq!(before.kind, "unknown");
        assert_eq!(before.status, "unknown");

        // 2. Sanity: row still unknown without reindex.
        let stale = crate::librarian::catalog::artifact::get(&cat, &id)
            .unwrap()
            .unwrap();
        assert_eq!(stale.kind, "unknown", "must be stale before reindex");

        // 3. Add rule that matches; content unchanged so SHA matches.
        let with_rules = crate::librarian::classify::load_rules(
            "[[rule]]\nglob = \"**/docs/trackers/*.md\"\nkind = \"tracker\"\nstatus = \"active\"\n",
        )
        .unwrap();
        index_repo_sync(&cat, &with_rules, root, &ignore, false, false, false).unwrap();

        // 4. Row must be reclassified.
        let after = crate::librarian::catalog::artifact::get(&cat, &id)
            .unwrap()
            .unwrap();
        assert_eq!(after.kind, "tracker");
        assert_eq!(after.status, "active");
    }

    #[test]
    fn write_embeddings_is_idempotent_on_same_id() {
        // BUG-045: re-embedding an artifact must not fail on vec0 primary key.
        // artifact_vec has a FK/trigger tied to artifact, so seed an artifact row first.
        let cat = Catalog::open_in_memory().unwrap();
        let now = chrono::Utc::now().timestamp_millis();
        let row = crate::librarian::catalog::artifact::ArtifactRow {
            id: "r:docs/a.md".into(),
            abs_path: std::path::PathBuf::from("/test/r/docs/a.md"),
            kind: "spec".into(),
            status: "draft".into(),
            title: None,
            owners: vec![],
            tags: vec![],
            topic: None,
            time_scope: None,
            source: None,
            created_at: now,
            updated_at: now,
            file_mtime: now,
            file_sha256: "deadbeef".into(),
            confidence: 1.0,
        };
        crate::librarian::catalog::artifact::upsert(&cat, &row).unwrap();

        let id = "r:docs/a.md".to_string();
        let vec_a: Vec<f32> = vec![0.1f32; 768];
        let vec_b: Vec<f32> = vec![0.2f32; 768];

        write_embeddings(&cat, &[(id.clone(), vec_a)]).unwrap();
        // Second write with same id must succeed (replace, not error).
        write_embeddings(&cat, &[(id.clone(), vec_b)]).unwrap();

        let count: i64 = cat
            .conn
            .query_row("SELECT count(*) FROM artifact_vec", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1, "second write must replace, not duplicate");
    }

    fn seed_artifact_row(cat: &Catalog, id: &str) {
        let now = chrono::Utc::now().timestamp_millis();
        let row = crate::librarian::catalog::artifact::ArtifactRow {
            id: id.to_string(),
            abs_path: std::path::PathBuf::from(format!("/test/{id}.md")),
            kind: "spec".into(),
            status: "draft".into(),
            title: None,
            owners: vec![],
            tags: vec![],
            topic: None,
            time_scope: None,
            source: None,
            created_at: now,
            updated_at: now,
            file_mtime: now,
            file_sha256: "deadbeef".into(),
            confidence: 1.0,
        };
        crate::librarian::catalog::artifact::upsert(cat, &row).unwrap();
    }

    #[test]
    fn write_embeddings_dim_mismatch_bails_by_default() {
        let cat = Catalog::open_in_memory().unwrap();
        seed_artifact_row(&cat, "a");
        seed_artifact_row(&cat, "b");
        write_embeddings_with(&cat, &[("a".into(), vec![0.1f32; 768])], false).unwrap();

        let err =
            write_embeddings_with(&cat, &[("b".into(), vec![0.2f32; 3072])], false).unwrap_err();
        assert!(
            err.to_string()
                .contains("embedding dim mismatch vs catalog"),
            "got: {err}"
        );
        assert!(
            err.to_string().contains(ARTIFACT_VEC_MIGRATE_ENV),
            "error must name the opt-in escape hatch: {err}"
        );

        // The mismatched write must not have landed.
        let count: i64 = cat
            .conn
            .query_row("SELECT count(*) FROM artifact_vec", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1, "rejected batch must not be inserted");
    }

    /// The env value → opt-in mapping, tested as a pure function so no test in
    /// this module mutates process-global env. `set_var` in a concurrently-run
    /// test is the UB that `a656f8cec220d347` removed project-wide; see
    /// `docs/issues/archive/2026-07-27-embedder-batch-env-test-race-reintroduces-fixed-ub.md`.
    #[test]
    fn migrate_opt_in_requires_exactly_one() {
        assert!(migrate_opt_in(Some("1")));
        assert!(!migrate_opt_in(None), "unset must not opt in");
        assert!(!migrate_opt_in(Some("0")));
        assert!(
            !migrate_opt_in(Some("true")),
            "only the literal \"1\" opts into a destructive rebuild"
        );
        assert!(!migrate_opt_in(Some("")));
    }

    #[test]
    fn write_embeddings_dim_mismatch_migrates_when_opted_in() {
        let cat = Catalog::open_in_memory().unwrap();
        seed_artifact_row(&cat, "a");
        seed_artifact_row(&cat, "b");
        write_embeddings_with(&cat, &[("a".into(), vec![0.1f32; 768])], true).unwrap();

        // Opted in: the 3072-dim batch must migrate the table instead of erroring.
        write_embeddings_with(&cat, &[("b".into(), vec![0.2f32; 3072])], true).unwrap();

        // The old 768-dim row is gone (table was dropped + recreated), only
        // the new 3072-dim row survives.
        let count: i64 = cat
            .conn
            .query_row("SELECT count(*) FROM artifact_vec", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1, "table rebuild must drop the old-dim row");
        let blob_len: i64 = cat
            .conn
            .query_row(
                "SELECT length(embedding) FROM artifact_vec WHERE id = 'b'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(blob_len / 4, 3072, "surviving row must be at the new dim");

        // The new dim is now the catalog's baseline — a second 3072-dim batch
        // must succeed without further migration.
        write_embeddings_with(&cat, &[("a".into(), vec![0.3f32; 3072])], true).unwrap();
    }

    #[test]
    fn write_embeddings_migration_backs_up_file_backed_catalog() {
        let tmp = tempfile::TempDir::new().unwrap();
        let db_path = tmp.path().join("catalog.db");
        let cat = Catalog::open(&db_path).unwrap();
        seed_artifact_row(&cat, "a");
        seed_artifact_row(&cat, "b");
        write_embeddings_with(&cat, &[("a".into(), vec![0.1f32; 768])], true).unwrap();

        write_embeddings_with(&cat, &[("b".into(), vec![0.2f32; 3072])], true).unwrap();

        let backups: Vec<_> = std::fs::read_dir(tmp.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains("pre-vec-dim-bak"))
            .collect();
        assert_eq!(
            backups.len(),
            1,
            "exactly one backup file must be created before the migration"
        );
    }

    #[test]
    fn index_repo_sync_force_embed_requeues_unchanged_content() {
        // Without force_embed, content_unchanged short-circuits the embed
        // queue even when want_embeddings=true — the "embeddings were just
        // enabled/reconfigured for an already-indexed project" gap this
        // parameter exists to close.
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join("docs/specs")).unwrap();
        std::fs::write(root.join("docs/specs/a.md"), "# a\nbody\n").unwrap();

        let cat = Catalog::open_in_memory().unwrap();
        let rules = crate::librarian::classify::load_rules(
            "[[rule]]\nglob = \"**/docs/specs/*.md\"\nkind = \"spec\"\n",
        )
        .unwrap();
        let ignore = globset::GlobSet::empty();

        // First pass: index without embeddings (simulates "already indexed
        // before embeddings were configured").
        let (r1, q1) = index_repo_sync(&cat, &rules, root, &ignore, false, false, false).unwrap();
        assert_eq!(r1.added, 1);
        assert!(q1.is_empty());

        // Second pass: embeddings now wanted, force_rewalk=true (bypasses the
        // unchanged-row skip), but force_embed=false — content is unchanged
        // on disk, so the file must NOT be queued for embedding.
        let (r2, q2) = index_repo_sync(&cat, &rules, root, &ignore, true, true, false).unwrap();
        assert_eq!(r2.updated, 1, "force_rewalk must still process the row");
        assert!(
            q2.is_empty(),
            "unchanged content must not be queued without force_embed"
        );

        // Third pass: force_embed=true must queue it despite unchanged content.
        let (r3, q3) = index_repo_sync(&cat, &rules, root, &ignore, true, true, true).unwrap();
        assert_eq!(r3.updated, 1);
        assert_eq!(
            q3.len(),
            1,
            "force_embed must queue unchanged content for re-embedding"
        );
    }

    #[test]
    fn index_repo_sync_force_embed_alone_requeues_without_force_rewalk() {
        // Regression: `force_embed` is documented as an INDEPENDENT lever —
        // "the separate, explicit lever for 'queue this file for embedding even
        // though its content hash is unchanged'". It was in fact reachable only
        // when `force_rewalk` was also set, because the unchanged-row
        // early-return `continue`d before the embed-queue branch ever ran.
        //
        // The sibling test above passes force_rewalk=true in every pass, so it
        // proved force_embed works GIVEN force_rewalk and never covered the
        // combination the `reindex(reembed=true)` tool call actually produces.
        // See docs/issues/archive/2026-07-25-reindex-reembed-noop-without-force.md.
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join("docs/specs")).unwrap();
        std::fs::write(root.join("docs/specs/a.md"), "# a\nbody\n").unwrap();

        let cat = Catalog::open_in_memory().unwrap();
        let rules = crate::librarian::classify::load_rules(
            "[[rule]]\nglob = \"**/docs/specs/*.md\"\nkind = \"spec\"\n",
        )
        .unwrap();
        let ignore = globset::GlobSet::empty();

        // Already indexed, embeddings were not configured at the time.
        let (r1, q1) = index_repo_sync(&cat, &rules, root, &ignore, false, false, false).unwrap();
        assert_eq!(r1.added, 1);
        assert!(q1.is_empty());

        // force_embed=true, force_rewalk=FALSE — exactly what
        // `librarian(reindex, reembed=true)` passes. The row itself genuinely
        // needs no rewrite, so it must still be reported `unchanged`; the file
        // must nevertheless be queued for embedding.
        let (r2, q2) = index_repo_sync(&cat, &rules, root, &ignore, true, false, true).unwrap();
        assert_eq!(
            q2.len(),
            1,
            "force_embed alone must queue unchanged content — it is documented \
             as independent of force_rewalk"
        );
        assert_eq!(
            r2.unchanged, 1,
            "a re-embed pass must not claim the row was updated — nothing about \
             the row changed, only its vector needs recomputing"
        );
        assert_eq!(r2.updated, 0);

        // Neither flag: still a true no-op, nothing queued.
        let (r3, q3) = index_repo_sync(&cat, &rules, root, &ignore, true, false, false).unwrap();
        assert!(
            q3.is_empty(),
            "no flags: unchanged content must not be re-queued"
        );
        assert_eq!(r3.unchanged, 1);
    }

    #[test]
    fn index_repo_sync_skips_empty_body_from_embed_queue() {
        // BUG (found live during an Azure-embedder bulk backfill): a single
        // empty/frontmatter-only body aborted the ENTIRE reindex, because
        // the embedder's own guard bails the whole batch on any empty input.
        // Empty bodies must never reach the embed queue in the first place.
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join("docs/specs")).unwrap();
        // Frontmatter-only, no body content at all.
        std::fs::write(root.join("docs/specs/empty.md"), "---\ntitle: Empty\n---\n").unwrap();
        // Body is present but whitespace-only.
        std::fs::write(
            root.join("docs/specs/whitespace.md"),
            "---\ntitle: Whitespace\n---\n   \n\n\t\n",
        )
        .unwrap();
        std::fs::write(
            root.join("docs/specs/real.md"),
            "# Real\n\nSome body text.\n",
        )
        .unwrap();

        let cat = Catalog::open_in_memory().unwrap();
        let rules = crate::librarian::classify::load_rules(
            "[[rule]]\nglob = \"**/docs/specs/*.md\"\nkind = \"spec\"\n",
        )
        .unwrap();
        let ignore = globset::GlobSet::empty();

        let (report, queue) =
            index_repo_sync(&cat, &rules, root, &ignore, true, false, false).unwrap();
        assert_eq!(report.added, 3);
        assert_eq!(
            queue.len(),
            1,
            "only the file with real body content may reach the embed queue, got: {queue:?}"
        );

        // Element 0 is a CHUNK id now (Task 6), not an artifact id — resolve it
        // back to `real.md`'s artifact via the artifact_chunk row it names.
        let real_id = crate::librarian::ids::artifact_id_from_abs(&root.join("docs/specs/real.md"));
        let owner: String = cat
            .conn
            .query_row(
                "SELECT artifact_id FROM artifact_chunk WHERE chunk_id = ?1",
                [&queue[0].chunk_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            owner, real_id,
            "the queued chunk must belong to the only artifact with real content"
        );
    }

    #[test]
    fn embed_queue_items_emits_every_chunk_not_just_the_first() {
        // The regression test for this whole plan. Mutating the implementation back
        // to `.next()` must fail HERE.
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(
            &cat,
            &crate::librarian::catalog::artifact::TestArtifactRowBuilder::new("a")
                .with_kind("tracker")
                .with_status("active")
                .build(),
        )
        .unwrap();
        let body = "# Log\n\npreamble\n\n## W-1 — first\n\nalpha\n\n## W-2 — second\n\nbeta\n";
        let items = embed_queue_items(&cat, "a", Some("Log".into()), body).unwrap();
        assert!(
            items.len() >= 3,
            "preamble + two entries, got {}",
            items.len()
        );
        let texts: Vec<&str> = items.iter().map(|i| i.text.as_str()).collect();
        assert!(
            texts.iter().any(|t| t.contains("alpha")),
            "W-1's body must be embedded"
        );
        assert!(
            texts.iter().any(|t| t.contains("beta")),
            "W-2's body must be embedded"
        );
    }

    #[test]
    fn embed_queue_items_keys_on_chunk_ids_that_exist_in_artifact_chunk() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(
            &cat,
            &crate::librarian::catalog::artifact::TestArtifactRowBuilder::new("a")
                .with_kind("tracker")
                .with_status("active")
                .build(),
        )
        .unwrap();
        let items = embed_queue_items(&cat, "a", None, "# T\n\n## W-1 — t\n\nx\n").unwrap();
        for item in &items {
            let chunk_id = &item.chunk_id;
            let n: i64 = cat
                .conn
                .query_row(
                    "SELECT COUNT(*) FROM artifact_chunk WHERE chunk_id = ?1",
                    [chunk_id],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(n, 1, "every queued id must be a real chunk row");
        }
    }

    #[test]
    fn a_stored_chunks_range_points_at_that_text_in_the_file() {
        // The Task-12 regression guard. Chunk ranges leave the process through
        // `artifact(find, semantic=)`'s `matched` block as FILE lines and a
        // caller opens the file at them. Until 2026-09-02 they were
        // body-relative, so every hit on a frontmatter-carrying document was
        // short by the block's height and landed inside the PREVIOUS entry
        // (measured: 7793 against a true 7808 on bug-fix-session-log.md).
        //
        // LOAD-BEARING — the fixture's frontmatter must stay NON-EMPTY. At zero
        // frontmatter lines the offset is 0, body- and file-relative coincide,
        // and the defect is UNREPRESENTABLE: the test would pass against the
        // broken implementation. That is exactly how every other
        // embed_queue_items test here missed it — all of them pass a bare body.
        let doc =
            "---\nkind: tracker\nstatus: active\n---\n# Log\n\npre\n\n## W-2 — second\n\nbeta\n";
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(
            &cat,
            &crate::librarian::catalog::artifact::TestArtifactRowBuilder::new("a")
                .with_kind("tracker")
                .with_status("active")
                .build(),
        )
        .unwrap();
        embed_queue_items(&cat, "a", None, doc).unwrap();

        let file_lines: Vec<&str> = doc.lines().collect();
        let mut stmt = cat
            .conn
            .prepare(
                "SELECT start_line, end_line, entry_token, content FROM artifact_chunk \
                 WHERE artifact_id = 'a' ORDER BY chunk_ix",
            )
            .unwrap();
        let rows: Vec<(i64, i64, Option<String>, String)> = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        assert!(
            rows.len() >= 2,
            "fixture must yield a preamble plus an entry chunk, got {}",
            rows.len()
        );

        // The round trip, asserted PER CHUNK rather than over the set: slice the
        // FILE at the stored range and the chunk's own text must come back. This
        // is monotone in neither direction — it fails under a dropped offset, a
        // doubled one, and an off-by-one either way, none of which a
        // `start_line >= 1`-shaped assertion can see.
        for (start, end, _, content) in &rows {
            assert_eq!(
                file_lines[*start as usize - 1..*end as usize].join("\n"),
                *content,
                "chunk stored as file lines {start}-{end} is not the text at those lines"
            );
        }

        // And the token survived the shift. It is looked up body-relative
        // BEFORE the offset is applied; folding the offset in earlier slides
        // every token one entry along — measured, the preamble chunk inherited
        // `W-2` at range (5, 8) while the real one lost it.
        let w2 = rows
            .iter()
            .find(|(_, _, t, _)| t.as_deref() == Some("W-2"))
            .expect("the W-2 entry chunk must still carry its token");
        assert_eq!(
            (w2.0, w2.1),
            (9, 11),
            "`## W-2 — second` is line 9 of the fixture and `beta` is line 11"
        );
    }

    #[test]
    fn a_whitespace_only_section_is_dropped_without_dropping_the_batch() {
        // The embedder's guard bails the WHOLE batch on one empty input
        // (archive/2026-05-17-reindex-embedding-dim-mismatch.md). With N chunks the
        // filter has to be per-chunk, or one blank section aborts a full reindex.
        //
        // FINDING A FIX: `"##    "` is a valid H2 with empty text per
        // `heading_level` (crates/codescout-embed/src/chunker.rs:138-145, "1-6
        // hashes followed by a space") — its own section trims to "##", which is
        // NOT empty, so a fixture built only from that shape never produces an
        // empty chunk and never reaches the per-chunk filter. The genuinely empty
        // chunk here is the body's own leading blank line before the first
        // heading (the dominant real shape: frontmatter.rs starts the body right
        // after the closing `---\n`, so any file with a blank line after
        // frontmatter yields an empty first section).
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(
            &cat,
            &crate::librarian::catalog::artifact::TestArtifactRowBuilder::new("a")
                .with_kind("tracker")
                .with_status("active")
                .build(),
        )
        .unwrap();
        let body = "\n# T\n\n## W-1 — real\n\ncontent\n\n## W-2 — also real\n\nmore\n";
        let built = crate::librarian::catalog::chunk::build_chunks("a", body, 512 * 4, 0);
        let empties = built.iter().filter(|r| r.content.trim().is_empty()).count();
        // LOAD-BEARING: this is what stops the fixture silently going vacuous
        // again if the chunker's heading/section rules change — without it, a
        // future rule change could make every chunk non-empty (as `"##    "` did
        // here) and this test would keep passing while proving nothing.
        assert_eq!(
            empties, 1,
            "fixture must contain exactly one empty chunk or this test proves nothing"
        );
        let items = embed_queue_items(&cat, "a", None, body).unwrap();
        assert!(!items.is_empty(), "the real chunks survive");
        assert!(
            items.iter().all(|i| !i.text.trim().is_empty()),
            "no empty text may reach the embedder"
        );
        assert_eq!(
            items.len(),
            built.len() - empties,
            "the empty chunk is dropped and the real ones are NOT"
        );
    }

    #[test]
    fn embed_queue_items_on_empty_body_deletes_every_chunk_and_vector_via_the_cascade() {
        // build_chunks(id, "", _) returns vec![], so replace_chunks deletes every
        // existing row for the artifact — and, via
        // `artifact_vec_v2_cascade_delete`, every vector that named one of those
        // rows. Correct for an emptied artifact, but nothing asserted it before,
        // and embed_queue_items is the path that reaches it.
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(
            &cat,
            &crate::librarian::catalog::artifact::TestArtifactRowBuilder::new("a")
                .with_kind("tracker")
                .with_status("active")
                .build(),
        )
        .unwrap();

        // Seed non-empty chunks first, so there is something for an empty body to delete.
        let seeded =
            embed_queue_items(&cat, "a", None, "# T\n\n## W-1 — t\n\nsome content\n").unwrap();
        assert!(!seeded.is_empty(), "sanity: the seed body produced chunks");
        let before: i64 = cat
            .conn
            .query_row(
                "SELECT COUNT(*) FROM artifact_chunk WHERE artifact_id = 'a'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(before > 0);

        // FINDING B FIX: insert a real artifact_vec_v2 row keyed by one of the
        // seeded chunk ids, so the "and vector" half of this test's name is
        // actually exercised rather than just mentioned in a comment.
        // `artifact_vec_v2` is keyed by chunk_id, not artifact_id — see
        // src/librarian/catalog/mod.rs:285.
        let seeded_chunk_id = &seeded[0].chunk_id;
        cat.conn
            .execute(
                "INSERT INTO artifact_vec_v2 (id, embedding) VALUES (?1, ?2)",
                rusqlite::params![seeded_chunk_id, vec![0u8; 768 * 4]],
            )
            .unwrap();
        let vec_before: i64 = cat
            .conn
            .query_row(
                "SELECT COUNT(*) FROM artifact_vec_v2 WHERE id = ?1",
                [seeded_chunk_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(vec_before, 1, "sanity: the vector row was seeded");

        // Now reindex with an empty body.
        let items = embed_queue_items(&cat, "a", None, "").unwrap();
        assert!(
            items.is_empty(),
            "an empty body queues nothing for embedding"
        );

        let after: i64 = cat
            .conn
            .query_row(
                "SELECT COUNT(*) FROM artifact_chunk WHERE artifact_id = 'a'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            after, 0,
            "every chunk row for the emptied artifact must be gone"
        );

        let vec_after: i64 = cat
            .conn
            .query_row(
                "SELECT COUNT(*) FROM artifact_vec_v2 WHERE id = ?1",
                [seeded_chunk_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            vec_after, 0,
            "artifact_vec_v2_cascade_delete must remove the vector for the deleted chunk"
        );
    }

    #[test]
    fn delete_chunk_vectors_removes_every_chunk_vector_not_only_the_first() {
        // Mutation target: narrowing the SELECT that feeds the delete (a
        // `LIMIT 1`, or an added `AND chunk_ix = 0`). The fixture MUST yield
        // >1 chunk or that mutation is UNREPRESENTABLE — a single-chunk
        // artifact behaves identically under "delete all" and "delete first".
        //
        // SCOPE, verified 2026-09-02 — do not widen this test's name to
        // "deleting an artifact": that path does NOT go through this function.
        // `artifact_chunk.artifact_id` is `ON DELETE CASCADE` (catalog/mod.rs)
        // and the `artifact_vec_v2_cascade_delete` trigger collects the vectors,
        // pinned by `deleting_an_artifact_cascades_its_vec_v2_row_via_the_chunk_trigger`.
        // `delete_chunk_vectors` exists for `SqliteVecArtifactStore::delete`,
        // which drops an artifact's vectors WITHOUT deleting the artifact row,
        // so no `artifact_chunk` delete occurs and the trigger cannot fire.
        let cat = Catalog::open_in_memory().unwrap();
        crate::librarian::catalog::artifact::upsert(
            &cat,
            &crate::librarian::catalog::artifact::TestArtifactRowBuilder::new("a")
                .with_kind("tracker")
                .with_status("active")
                .build(),
        )
        .unwrap();
        let built = crate::librarian::catalog::chunk::build_chunks(
            "a",
            "# T\n\n## W-1 — x\n\nalpha\n\n## W-2 — y\n\nbeta\n",
            2048,
            0,
        );
        let stored = crate::librarian::catalog::chunk::replace_chunks(&cat, "a", &built).unwrap();
        assert!(
            stored.len() > 1,
            "fixture must yield >1 chunk or this proves nothing, got {}",
            stored.len()
        );

        let vecs: Vec<(String, Vec<f32>)> = stored
            .iter()
            .map(|r| (r.chunk_id.clone(), vec![0.5f32; 768]))
            .collect();
        write_embeddings_v2(&cat, &vecs).unwrap();
        // Pairs with the `== 0` below. Without this, a writer that silently
        // wrote nothing satisfies the final assertion — the absence assertion
        // is monotone under the write never happening.
        let before: i64 = cat
            .conn
            .query_row("SELECT COUNT(*) FROM artifact_vec_v2", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            before as usize,
            stored.len(),
            "every chunk vector must be written before the delete is meaningful"
        );

        let removed = delete_chunk_vectors(&cat, "a").unwrap();
        assert_eq!(removed, stored.len(), "must report every vector it removed");
        let n: i64 = cat
            .conn
            .query_row("SELECT COUNT(*) FROM artifact_vec_v2", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0, "every chunk vector must go, not only the first");
    }

    #[test]
    fn write_embeddings_v2_refuses_a_dim_mismatch_against_existing_rows() {
        let cat = Catalog::open_in_memory().unwrap();
        write_embeddings_v2(&cat, &[("c1".into(), vec![0.1f32; 768])]).unwrap();
        let err = write_embeddings_v2(&cat, &[("c2".into(), vec![0.1f32; 384])]).unwrap_err();
        assert!(err.to_string().contains("dim"), "got: {err}");
        // The doc-comment promises a "loud, safe stop rather than a silent
        // partial write". Asserting only the Err leaves that half unchecked —
        // a writer that DELETEs then bails also returns Err.
        let n: i64 = cat
            .conn
            .query_row(
                "SELECT COUNT(*) FROM artifact_vec_v2 WHERE id = 'c2'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 0, "a refused batch must leave nothing behind");
    }

    #[test]
    fn write_embeddings_v2_refuses_a_dim_mismatch_within_one_batch() {
        // The F-6b shape: an embedder returns a 1-element error sentinel mixed
        // into an otherwise good batch. `write_embeddings_with` catches this
        // BEFORE any DELETE; v2 must too, or the delete-then-insert loop
        // half-writes the batch and only then bails.
        let cat = Catalog::open_in_memory().unwrap();
        let err = write_embeddings_v2(
            &cat,
            &[
                ("c1".into(), vec![0.1f32; 768]),
                ("c2".into(), vec![0.1f32; 1]),
            ],
        )
        .unwrap_err();
        assert!(err.to_string().contains("within batch"), "got: {err}");
        let n: i64 = cat
            .conn
            .query_row("SELECT COUNT(*) FROM artifact_vec_v2", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0, "nothing may be written when the batch is rejected");
    }

    #[test]
    fn ignore_globs_skip_matching_files() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join("docs/specs")).unwrap();
        std::fs::create_dir_all(root.join("tests/fixtures")).unwrap();
        std::fs::write(root.join("docs/specs/a.md"), "# a\n").unwrap();
        std::fs::write(root.join("tests/fixtures/b.md"), "# fixture\n").unwrap();

        let cat = Catalog::open_in_memory().unwrap();
        let rules = crate::librarian::classify::load_rules(
            "[[rule]]\nglob = \"**/*.md\"\nkind = \"doc\"\n",
        )
        .unwrap();
        let ignore =
            crate::librarian::workspace::compile_ignore(&["**/tests/fixtures/**".to_string()])
                .unwrap();

        let (r, _) = index_repo_sync(&cat, &rules, root, &ignore, false, false, false).unwrap();
        assert_eq!(r.added, 1, "fixture file must be skipped by ignore glob");
    }

    #[test]
    fn first_h1_extracts_title() {
        assert_eq!(first_h1("# Hello\n\nbody text"), Some("Hello".to_string()));
    }

    #[test]
    fn first_h1_skips_blank_and_code_fences() {
        let body = "\n```\n# not a header\n```\n\n# Real\n\nbody";
        assert_eq!(first_h1(body), Some("Real".to_string()));
    }

    #[test]
    fn first_h1_none_when_missing() {
        assert_eq!(first_h1("## Only H2\n\nno h1 here"), None);
        assert_eq!(first_h1(""), None);
    }

    #[test]
    fn first_h1_extracts_setext_heading() {
        let body = "Setext Title\n===========\n\nbody";
        assert_eq!(first_h1(body), Some("Setext Title".into()));
    }

    #[test]
    fn first_h1_ignores_h1_inside_code_fence() {
        let body = "```\n# not a heading\n```\n\n# Real Heading\n";
        assert_eq!(first_h1(body), Some("Real Heading".into()));
    }

    #[test]
    fn index_derives_title_from_h1_when_no_frontmatter() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join("docs")).unwrap();
        // No frontmatter, just an H1 heading.
        std::fs::write(root.join("docs/page.md"), "# Title X\n\nSome body text.\n").unwrap();

        let cat = Catalog::open_in_memory().unwrap();
        let rules = crate::librarian::classify::load_rules(
            "[[rule]]\nglob = \"**/*.md\"\nkind = \"doc\"\n",
        )
        .unwrap();
        let ignore = globset::GlobSet::empty();

        let (report, _) =
            index_repo_sync(&cat, &rules, root, &ignore, false, false, false).unwrap();
        assert_eq!(report.added, 1);

        let id = crate::librarian::ids::artifact_id_from_abs(&root.join("docs/page.md"));
        let row = crate::librarian::catalog::artifact::get(&cat, &id)
            .unwrap()
            .unwrap();
        assert_eq!(row.title.as_deref(), Some("Title X"));
    }
    #[test]
    fn index_unions_rule_tags_with_frontmatter_tags() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join("src/tools")).unwrap();
        // No frontmatter — the tag comes solely from the matching rule, and
        // the file is rescued from kind=unknown. Mirrors the embedded
        // render_prompt.md templates under src/**/tools/**.
        std::fs::write(
            root.join("src/tools/render_prompt.md"),
            "# Render Prompt\n\nbody\n",
        )
        .unwrap();
        // Hand-authored frontmatter tag must be preserved AND augmented with
        // the rule tag — union, not overwrite.
        std::fs::write(
            root.join("src/tools/with_fm.md"),
            "---\nkind: doc\ntags:\n  - manual\n---\n\n# With FM\n",
        )
        .unwrap();

        let cat = Catalog::open_in_memory().unwrap();
        let rules = crate::librarian::classify::load_rules(
            "[[rule]]\nglob = \"src/**/*.md\"\nkind = \"doc\"\ntags = [\"codescout\"]\n",
        )
        .unwrap();
        let ignore = globset::GlobSet::empty();
        let (report, _) =
            index_repo_sync(&cat, &rules, root, &ignore, false, false, false).unwrap();
        assert_eq!(report.added, 2);

        let id_no_fm =
            crate::librarian::ids::artifact_id_from_abs(&root.join("src/tools/render_prompt.md"));
        let row = crate::librarian::catalog::artifact::get(&cat, &id_no_fm)
            .unwrap()
            .unwrap();
        assert_eq!(row.kind, "doc", "rule rescues the file from kind=unknown");
        assert_eq!(row.tags, vec!["codescout".to_string()]);

        let id_fm = crate::librarian::ids::artifact_id_from_abs(&root.join("src/tools/with_fm.md"));
        let row = crate::librarian::catalog::artifact::get(&cat, &id_fm)
            .unwrap()
            .unwrap();
        // Frontmatter tag first (preserved), rule tag appended, no dupes.
        assert_eq!(
            row.tags,
            vec!["manual".to_string(), "codescout".to_string()]
        );
    }

    // FINDING C ANNOTATION (post-Task-6, corrected): this fixture hand-inserts
    // rows into `artifact_vec` keyed by ARTIFACT id. Task 6 re-keyed the embed
    // queue to CHUNK ids, but the write path is unchanged: `write_embeddings`
    // (indexer.rs:603) still inserts into this same artifact-keyed
    // `artifact_vec` table (v1) — the move to `artifact_vec_v2` (chunk-keyed)
    // is Task 7's open work, not something already shipped. So post-Task-6, a
    // real production write puts a CHUNK id into a table this fixture only
    // ever populates with ARTIFACT ids — those rows are orphans no cascade
    // collects, since `artifact_vec` is a `vec0` virtual table with no
    // foreign key (schema.sql:49) and `artifact_vec_cascade_delete` matches
    // on `WHERE id = OLD.id` (schema.sql:54), i.e. an artifact id, which a
    // chunk id will never equal. This fixture is therefore inert with
    // respect to the current write path: it still asserts the trigger works
    // (true), but that isn't the shape production writes anymore. Left in
    // place (not deleted/rewritten) for Task 7, which owns re-pointing this
    // area to `artifact_vec_v2`.
    #[test]
    fn removed_file_also_removes_embedding_row() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join("docs/specs")).unwrap();
        std::fs::write(root.join("docs/specs/a.md"), "# a\n").unwrap();
        std::fs::write(root.join("docs/specs/b.md"), "# b\n").unwrap();

        let cat = Catalog::open_in_memory().unwrap();
        let rules = crate::librarian::classify::load_rules(
            "[[rule]]\nglob = \"**/docs/specs/*.md\"\nkind = \"spec\"\n",
        )
        .unwrap();
        let ignore = globset::GlobSet::empty();

        // Index both files so artifact rows exist.
        index_repo_sync(&cat, &rules, root, &ignore, false, false, false).unwrap();

        let id_a = crate::librarian::ids::artifact_id_from_abs(&root.join("docs/specs/a.md"));
        let id_b = crate::librarian::ids::artifact_id_from_abs(&root.join("docs/specs/b.md"));

        // Manually insert embedding rows to simulate post-embed state.
        let bytes: Vec<u8> = std::iter::repeat_n(0f32, 768)
            .flat_map(|f: f32| f.to_le_bytes())
            .collect();
        cat.conn
            .execute(
                "INSERT INTO artifact_vec (id, embedding) VALUES (?, ?)",
                rusqlite::params![id_a, bytes],
            )
            .unwrap();
        cat.conn
            .execute(
                "INSERT INTO artifact_vec (id, embedding) VALUES (?, ?)",
                rusqlite::params![id_b, bytes],
            )
            .unwrap();

        // Delete file b and reindex — trigger must cascade delete into artifact_vec.
        std::fs::remove_file(root.join("docs/specs/b.md")).unwrap();
        index_repo_sync(&cat, &rules, root, &ignore, false, false, false).unwrap();

        let count_b: i64 = cat
            .conn
            .query_row(
                "SELECT count(*) FROM artifact_vec WHERE id = ?",
                rusqlite::params![id_b],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count_b, 0, "trigger must cascade to artifact_vec");

        let count_a: i64 = cat
            .conn
            .query_row(
                "SELECT count(*) FROM artifact_vec WHERE id = ?",
                rusqlite::params![id_a],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count_a, 1, "surviving file keeps embedding");
    }

    #[tokio::test]
    async fn concurrent_embed_queue_completes_all() {
        use crate::librarian::embedding::EmbeddingService;
        use async_trait::async_trait;
        use codescout_embed::{Embedder, Embedding};
        use std::sync::Arc;

        struct MockEmbedder;

        #[async_trait]
        impl Embedder for MockEmbedder {
            fn dimensions(&self) -> usize {
                768
            }
            async fn embed(&self, texts: &[&str]) -> anyhow::Result<Vec<Embedding>> {
                Ok(texts.iter().map(|_| vec![0.1f32; 768]).collect())
            }
        }

        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join("docs/specs")).unwrap();
        // Create 16 files so the queue exceeds EMBED_CONCURRENCY (8).
        for i in 0..16u32 {
            std::fs::write(
                root.join(format!("docs/specs/{i}.md")),
                format!("---\ntitle: File {i}\n---\n# File {i}\n\nContent {i}.\n"),
            )
            .unwrap();
        }

        let cat = Catalog::open_in_memory().unwrap();
        let rules = crate::librarian::classify::load_rules(
            "[[rule]]\nglob = \"**/docs/specs/*.md\"\nkind = \"spec\"\n",
        )
        .unwrap();
        let ignore = globset::GlobSet::empty();
        let svc = EmbeddingService::new(Arc::new(MockEmbedder));

        let report = index_repo(&cat, &rules, root, &ignore, Some(&svc), None, "")
            .await
            .unwrap();

        assert_eq!(report.added, 16);
        assert_eq!(report.embedded, 16);

        let count: i64 = cat
            .conn
            .query_row("SELECT count(*) FROM artifact_vec_v2", [], |row| row.get(0))
            .unwrap();
        assert_eq!(
            count, 16,
            "all 16 embeddings must be written via buffer_unordered"
        );
        // CHANGED from `artifact_vec` (v1) when `index_repo`'s two flush
        // sites were re-pointed to `write_embeddings_v2`. Do NOT restore the
        // v1 form: the embed queue is keyed by `chunk_id`, so a v1 row here
        // means a chunk id sitting in the artifact-keyed table, which is the
        // defect the chunk-grain work exists to remove.
        //
        // This pair is what makes the test a guard for the re-point rather
        // than a casualty of it: `v2 == 16` alone is monotone under a writer
        // that writes BOTH tables, which is exactly what a half-finished
        // re-point looks like.
        let v1: i64 = cat
            .conn
            .query_row("SELECT count(*) FROM artifact_vec", [], |row| row.get(0))
            .unwrap();
        assert_eq!(v1, 0, "no chunk id may reach the artifact-keyed v1 table");
    }
}
