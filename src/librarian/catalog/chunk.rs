//! `artifact_chunk` rows: the line-anchored, entry-tagged pieces an artifact is
//! embedded as. One row per chunk; `artifact_vec_v2` is keyed by `chunk_id`.

use anyhow::Result;
use sha2::{Digest, Sha256};

use crate::librarian::catalog::Catalog;
use crate::librarian::entry_token::entry_tokens_by_line;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkRow {
    pub chunk_id: String,
    pub artifact_id: String,
    pub chunk_ix: usize,
    /// 1-indexed line **in the file**, frontmatter included. See
    /// [`build_chunks`]'s `line_offset`: this is the coordinate space the
    /// number is published in, so it is the one it is stored in.
    pub start_line: usize,
    /// 1-indexed, inclusive, **in the file**. See [`ChunkRow::start_line`].
    pub end_line: usize,
    pub entry_token: Option<String>,
    pub content: String,
    pub content_hash: String,
}

/// Chunk `body` at the librarian's grain: heading depth 6, so `####`-defined
/// entries start their own chunk. `chunk_size` is the CHARACTER budget and
/// stays 2,048 (512 tokens) — see the plan's Global Constraints before
/// touching it.
///
/// `line_offset` is how many lines sit ABOVE `body` in the file it came from —
/// the frontmatter block, for a librarian artifact. It is added to every
/// returned range, so [`ChunkRow`]'s `start_line` / `end_line` mean
/// **the line in the FILE, in every ChunkRow that exists** — freshly built or
/// read back out of `artifact_chunk`. One meaning is the whole point: these
/// numbers leave the process through `doc(action="find", semantic=)`'s `matched`
/// block, where a caller opens the file at them. Publishing body-relative
/// numbers as file lines put every hit on a tracker short by the frontmatter's
/// height, landing inside the PREVIOUS entry — measured 2026-09-02 at 7793 vs
/// a true 7808, see
/// `docs/issues/2026-09-02-chunk-line-ranges-are-body-relative-but-published-as-file-lines.md`.
/// A caller with nothing above its body passes `0`; there is deliberately no
/// 3-argument form that means `0` implicitly, because an implicit `0` is
/// exactly how that defect was shipped.
///
/// The offset is applied AFTER the entry-token lookup and must stay there:
/// `entry_tokens_by_line` is computed over `body`, so its keys are
/// body-relative. Folding `line_offset` into `raw.start_line` before the lookup
/// leaves every range correct and slides every token onto the WRONG chunk —
/// measured under mutation 2026-09-03: the preamble inherited `W-2` while the
/// real `W-2` chunk read `None`.
pub fn build_chunks(
    artifact_id: &str,
    body: &str,
    chunk_size: usize,
    line_offset: usize,
) -> Vec<ChunkRow> {
    let tokens = entry_tokens_by_line(body);
    codescout_embed::chunker::split_markdown_with_depth(body, chunk_size, 0, 6)
        .into_iter()
        .enumerate()
        .map(|(ix, raw)| {
            let mut hasher = Sha256::new();
            hasher.update(raw.content.as_bytes());
            ChunkRow {
                // Placeholder — replace_chunks assigns or preserves the real id.
                chunk_id: String::new(),
                artifact_id: artifact_id.to_string(),
                chunk_ix: ix,
                // Keyed on the UNSHIFTED line — see the note above.
                entry_token: tokens.get(raw.start_line).cloned().flatten(),
                start_line: raw.start_line + line_offset,
                end_line: raw.end_line + line_offset,
                content: raw.content,
                // Over `content` only, so re-chunking a file at a corrected
                // offset preserves every hash — which is why the migration off
                // body-relative ranges costs no re-embedding: replace_chunks
                // keeps the id and the vector and re-syncs the position.
                content_hash: format!("{:x}", hasher.finalize()),
            }
        })
        .collect()
}

/// Which grain artifact vectors are written at, decided per project.
///
/// **On by default; opt OUT with `[librarian] chunk_grain = false`.** The default
/// was `Artifact` for one day (2026-09-03, `4f172f70`) on the theory that chunk
/// grain was a 20× cost for better ranking and therefore the project's call.
/// Measuring the two grains head to head that night refuted the premise: it is
/// not better-versus-worse, it is working-versus-not.
///
/// **Measured 2026-09-03 on this corpus, same 12 queries, same embedder:**
///
/// | | chunk grain | artifact grain |
/// |---|---|---|
/// | file-level hits@5 (did the right DOCUMENT come back) | **6/12** | **0/12** |
/// | entry-level hits@5 | 3/12 | 0/12 — impossible by construction |
/// | vectors | 29,138 | 1,475 (19.8× cheaper) |
/// | artifacts the embedder REFUSED | 0 | **473 of 1,475 (32%)** |
///
/// The `0/12` survived a positive control, which is why it is quoted: querying an
/// artifact-grain collection with a document's own *title* ranks that document #1
/// with clear margin every time. The collection retrieves fine; the grain cannot
/// answer the question. A per-artifact vector represents the document's opening
/// ~2,048 characters — for a ledger that is frontmatter, an index table and
/// conventions boilerplate — so an entry at line 7,956 of a 10,752-line file has
/// no representation in it at all. Artifact grain answers *"which document is
/// this?"* and cannot answer *"which document says this?"*.
///
/// **[`ChunkGrain::Artifact`] is therefore a degraded mode, not a cheap one**, and
/// on this corpus it is additionally *broken*: the embedder rejects oversized
/// input with HTTP 500 rather than truncating, and nothing in the librarian embed
/// path cuts the text first, so a third of artifacts would be left permanently
/// vectorless in the absorbing state
/// `docs/issues/2026-09-02-indexer-stamps-content-seen-before-it-embeds.md`
/// describes. Do not reach for it as a hardware concession until that is fixed.
/// It is kept, rather than deleted, because the *storage* shape is sound (one
/// whole-body chunk row, so `matched` reports a true span) and a larger-context
/// embedder would make it viable without further change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ChunkGrain {
    /// One vector per chunk — [`build_chunks`]. The default, and the ONLY place
    /// that default is written down: every fallback below says
    /// `ChunkGrain::default()` rather than naming a variant, because when this
    /// default was flipped on 2026-09-04 the literal fallbacks in
    /// `chunk_grain_for_file` were missed and the flip silently did not reach the
    /// backfill. One `#[default]` marker cannot be half-changed.
    #[default]
    Chunk,
    /// One vector per artifact — [`build_single_chunk`]. Degraded; see the type
    /// doc before selecting it.
    Artifact,
}

impl ChunkGrain {
    /// Resolve the grain for the project rooted at `project_path`:
    /// `[librarian] chunk_grain = true|false` in `<project>/.codescout/project.toml`,
    /// defaulting to [`ChunkGrain::Chunk`].
    ///
    /// **Only a literal `false` opts out.** Absent file / unparseable TOML /
    /// missing key / wrong type all resolve to the default, like
    /// [`crate::librarian::indexer`]'s `read_force_include`: a config typo must
    /// not fail a reindex. Since the default flipped, that silence now fails
    /// SAFE — a mistyped opt-out costs embedding time and leaves search working,
    /// where under the old default it cost a third of the corpus its vectors.
    ///
    /// Mirrors `ArtifactBackend::resolve`'s raw-TOML-read pattern — librarian's
    /// `ToolContext` does not carry the main server's parsed config (see
    /// `ProjectConfig`'s `force_include` note), so the file is read fresh rather
    /// than threaded through.
    ///
    /// **The setting does not travel**, because `.codescout/project.toml` is
    /// gitignored — as is already true of `[librarian] vector_backend` beside it.
    /// That was a hazard under the old default and is benign under this one: a
    /// clone on a second machine now inherits the WORKING grain, and only a
    /// deliberate local opt-out is lost. Nothing reports the difference either
    /// way, since both grains produce a populated index.
    ///
    /// **Deliberately has no env override, unlike its sibling `ArtifactBackend`.**
    /// That sibling's env branch is reachable in tests only because `EnvGuard`
    /// lives behind `#[cfg(feature = "server-stack")]`, whose own comment says
    /// *"Do NOT copy this pattern into a default-feature test"* — env mutation is
    /// UB-racy against non-serial writers. This flag decides what gets written to
    /// every vector store, so an untestable branch on that path costs more than
    /// the convenience is worth. If an override is ever needed, it needs a
    /// serialisation story first, not a `var()` call.
    pub fn resolve(project_path: Option<&str>) -> Self {
        let Some(root) = project_path else {
            return ChunkGrain::default();
        };
        let cfg = std::path::Path::new(root)
            .join(".codescout")
            .join("project.toml");
        let Ok(text) = std::fs::read_to_string(&cfg) else {
            return ChunkGrain::default();
        };
        let Ok(parsed) = toml::from_str::<toml::Value>(&text) else {
            return ChunkGrain::default();
        };
        match parsed
            .get("librarian")
            .and_then(|t| t.get("chunk_grain"))
            .and_then(|v| v.as_bool())
        {
            Some(false) => ChunkGrain::Artifact,
            _ => ChunkGrain::default(),
        }
    }
}

/// The [`ChunkGrain::Artifact`] builder: ONE row spanning the whole `body`.
///
/// Not expressible as [`build_chunks`] with a large `chunk_size`, and the reason
/// is worth stating because it is the natural first attempt:
/// `split_markdown_with_depth` starts a new section at **every** heading of level
/// ≤ depth before it ever consults the character budget, so a 1 MB `chunk_size`
/// still returns one chunk per heading. The budget bounds a section, it does not
/// merge them.
///
/// `line_offset` means what it means in [`build_chunks`] — lines above `body` in
/// the file — and is applied for the same reason: these numbers leave the process
/// through `doc(action="find", semantic=)`'s `matched` block as FILE lines.
///
/// `entry_token` is `None` by construction, not by omission. A whole-document
/// chunk belongs to no single entry, and naming the first one would make
/// `matched.entry` assert an entry the vector mostly is not about — a wrong
/// answer where `None` is a true one.
///
/// An empty `body` yields no rows, matching [`build_chunks`] (whose
/// `split_markdown_with_depth` early-returns on empty). Whitespace-only content
/// is left to the caller's `content.trim().is_empty()` filter, again matching.
pub fn build_single_chunk(artifact_id: &str, body: &str, line_offset: usize) -> Vec<ChunkRow> {
    if body.is_empty() {
        return Vec::new();
    }
    let mut hasher = Sha256::new();
    hasher.update(body.as_bytes());
    vec![ChunkRow {
        // Placeholder — replace_chunks assigns or preserves the real id.
        chunk_id: String::new(),
        artifact_id: artifact_id.to_string(),
        chunk_ix: 0,
        entry_token: None,
        start_line: 1 + line_offset,
        end_line: body.lines().count() + line_offset,
        content: body.to_string(),
        content_hash: format!("{:x}", hasher.finalize()),
    }]
}

/// Replace an artifact's chunk rows with a targeted diff, preserving `chunk_id`
/// wherever `(chunk_ix, content_hash)` is unchanged so untouched chunks keep
/// their vectors. Returns the rows as stored.
///
/// This must NOT be a blanket delete-then-insert: `artifact_vec_v2_cascade_delete`
/// (`AFTER DELETE ON artifact_chunk`) fires on every deleted row, including ones
/// whose `chunk_id` a blanket delete-then-insert would otherwise "preserve" —
/// the id survives but the embedding it was preserving the id for does not.
///
/// The vector and the position fields have different dependencies and are kept
/// in sync separately: the vector depends on `content` alone, so it is keyed by
/// `content_hash`; `start_line`/`end_line`/`entry_token` depend on the body's
/// layout, so they are re-synced on every content-hash match whose position
/// actually moved — an ordinary edit above an unchanged chunk shifts it without
/// touching its hash, and a stale line range is worse than a miss: the caller
/// follows it to the wrong place with no error. Four branches, keyed on
/// `chunk_ix`:
///   - same `chunk_ix`, same `content_hash` → keep the id and the vector
///     (no DELETE, no INSERT); UPDATE the position fields only if they moved.
///   - same `chunk_ix`, different `content_hash` → DELETE + INSERT (a new
///     `chunk_id`); the vector is correctly destroyed, content changed.
///   - old `chunk_ix` absent from the new rows (body shrank) → DELETE.
///   - new `chunk_ix` absent from the old rows → INSERT.
///
/// The resync uses a plain `UPDATE`, never `INSERT OR REPLACE`: SQLite only
/// fires delete triggers on a REPLACE-conflict deletion when `recursive_triggers`
/// is enabled, which defaults OFF — so `REPLACE` would preserve the vector today
/// by accident of that pragma, and silently destroy it again the moment anything
/// turns the pragma on. `UPDATE` is safe unconditionally.
///
/// Deletes run before inserts so a body that shrinks AND changes in the same
/// edit never collides with a stale row still holding the freed `chunk_ix`
/// under `UNIQUE (artifact_id, chunk_ix)`.
///
/// Not wrapped in a transaction: this is deliberate house style, not an
/// oversight. Leaf `&Catalog` writers here (`commits::upsert_many`,
/// `event_edges::insert_many`, `link_scan/diff.rs`'s apply step,
/// `merge_worktree.rs` — see its rationale at `:217-221`) don't open
/// `unchecked_transaction()` themselves, so a composite caller can wrap the
/// whole multi-step operation (e.g. chunking + re-embedding) in one. Two facts
/// the next caller needs: `Connection::transaction()` requires `&mut
/// Connection` and is unreachable through `&Catalog`, so the option here is
/// `conn.unchecked_transaction()`; and `unchecked_transaction()` must never be
/// nested — the caller opens at most one across the whole composite operation.
pub fn replace_chunks(
    cat: &Catalog,
    artifact_id: &str,
    rows: &[ChunkRow],
) -> Result<Vec<ChunkRow>> {
    let existing = chunks_for(cat, artifact_id)?;
    let existing_by_ix: std::collections::HashMap<usize, &ChunkRow> =
        existing.iter().map(|e| (e.chunk_ix, e)).collect();
    let new_ixs: std::collections::HashSet<usize> = rows.iter().map(|r| r.chunk_ix).collect();

    // Old chunk_ix values with no surviving row at all — the shrunk tail.
    let mut delete_ixs: Vec<usize> = existing
        .iter()
        .filter(|e| !new_ixs.contains(&e.chunk_ix))
        .map(|e| e.chunk_ix)
        .collect();

    let mut out = Vec::with_capacity(rows.len());
    let mut to_insert: Vec<ChunkRow> = Vec::new();
    let mut to_resync: Vec<ChunkRow> = Vec::new();
    for row in rows {
        let mut stored = row.clone();
        match existing_by_ix.get(&row.chunk_ix) {
            Some(e) if e.content_hash == row.content_hash => {
                // Unchanged content: preserve the id AND the vector (no DELETE),
                // but the body's layout may have shifted — a preamble edit moves
                // every unchanged chunk below it without touching its hash. Only
                // the position fields depend on layout, so only they are synced;
                // an UPDATE (never INSERT OR REPLACE — that relies on
                // recursive_triggers, off by default, to avoid firing the vector
                // cascade, which silently breaks the moment that pragma flips).
                stored.chunk_id = e.chunk_id.clone();
                if e.start_line != stored.start_line
                    || e.end_line != stored.end_line
                    || e.entry_token != stored.entry_token
                {
                    to_resync.push(stored.clone());
                }
            }
            Some(e) => {
                // Same ordinal, different content: the old row must go so the
                // vector cascade fires, and a fresh row (fresh id) replaces it.
                delete_ixs.push(e.chunk_ix);
                stored.chunk_id = uuid::Uuid::new_v4().to_string();
                to_insert.push(stored.clone());
            }
            None => {
                // A genuinely new ordinal.
                stored.chunk_id = uuid::Uuid::new_v4().to_string();
                to_insert.push(stored.clone());
            }
        }
        out.push(stored);
    }

    if !delete_ixs.is_empty() {
        let mut del_stmt = cat
            .conn
            .prepare("DELETE FROM artifact_chunk WHERE artifact_id = ?1 AND chunk_ix = ?2")?;
        for ix in &delete_ixs {
            del_stmt.execute(rusqlite::params![artifact_id, *ix as i64])?;
        }
    }

    if !to_resync.is_empty() {
        let mut upd_stmt = cat.conn.prepare(
            "UPDATE artifact_chunk
                SET start_line = ?1, end_line = ?2, entry_token = ?3
              WHERE chunk_id = ?4",
        )?;
        for r in &to_resync {
            upd_stmt.execute(rusqlite::params![
                r.start_line as i64,
                r.end_line as i64,
                r.entry_token,
                r.chunk_id
            ])?;
        }
    }

    if !to_insert.is_empty() {
        let mut ins_stmt = cat.conn.prepare(
            "INSERT INTO artifact_chunk
               (chunk_id, artifact_id, chunk_ix, start_line, end_line, entry_token, content, content_hash)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )?;
        for r in &to_insert {
            // Bind the PARAMETER, not `r.artifact_id`: the row field is
            // caller-supplied data, and if it ever disagreed with `artifact_id`
            // (the id this function queried and deleted under), binding the
            // field would silently insert under the wrong artifact — a
            // mismatch `chunks_for` on the real id would report as an empty
            // result, not an error. The debug_assert below is belt-and-suspenders
            // on top of that fix, not a substitute for it: it only fires in
            // debug builds, so the correctness the fix provides is unconditional
            // and holds in release too; the assert exists to catch a caller bug
            // loudly during development rather than let it ship silent.
            debug_assert_eq!(
                r.artifact_id, artifact_id,
                "replace_chunks: row.artifact_id disagrees with the artifact_id \
                 parameter — this row would be inserted under the wrong artifact \
                 if this assert weren't here to catch it in a debug build"
            );
            ins_stmt.execute(rusqlite::params![
                r.chunk_id,
                artifact_id,
                r.chunk_ix as i64,
                r.start_line as i64,
                r.end_line as i64,
                r.entry_token,
                r.content,
                r.content_hash
            ])?;
        }
    }

    Ok(out)
}

/// An artifact's chunk rows, ordered by `chunk_ix`.
pub fn chunks_for(cat: &Catalog, artifact_id: &str) -> Result<Vec<ChunkRow>> {
    let mut stmt = cat.conn.prepare(
        "SELECT chunk_id, artifact_id, chunk_ix, start_line, end_line, entry_token,
                content, content_hash
           FROM artifact_chunk WHERE artifact_id = ?1 ORDER BY chunk_ix",
    )?;
    let rows = stmt
        .query_map([artifact_id], |r| {
            Ok(ChunkRow {
                chunk_id: r.get(0)?,
                artifact_id: r.get(1)?,
                chunk_ix: r.get::<_, i64>(2)? as usize,
                start_line: r.get::<_, i64>(3)? as usize,
                end_line: r.get::<_, i64>(4)? as usize,
                entry_token: r.get(5)?,
                content: r.get(6)?,
                content_hash: r.get(7)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// Chunk rows for a set of chunk ids, keyed by `chunk_id`.
///
/// Ids with no row are simply absent from the map rather than an error: a
/// vector whose chunk row is gone is **stale, not corrupt**. That happens
/// normally — an artifact is re-chunked and its old chunk ids stop existing
/// while a vector store that has not been re-indexed still returns them.
/// Erroring there would turn an ordinary staleness window into a failed query.
pub fn rows_by_chunk_ids(
    cat: &Catalog,
    chunk_ids: &[String],
) -> Result<std::collections::HashMap<String, ChunkRow>> {
    if chunk_ids.is_empty() {
        return Ok(Default::default());
    }
    let placeholders = std::iter::repeat_n("?", chunk_ids.len())
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "SELECT chunk_id, artifact_id, chunk_ix, start_line, end_line, entry_token,
                content, content_hash
           FROM artifact_chunk WHERE chunk_id IN ({placeholders})"
    );
    let mut stmt = cat.conn.prepare(&sql)?;
    let rows = stmt
        .query_map(rusqlite::params_from_iter(chunk_ids.iter()), |r| {
            Ok(ChunkRow {
                chunk_id: r.get(0)?,
                artifact_id: r.get(1)?,
                chunk_ix: r.get::<_, i64>(2)? as usize,
                start_line: r.get::<_, i64>(3)? as usize,
                end_line: r.get::<_, i64>(4)? as usize,
                entry_token: r.get(5)?,
                content: r.get(6)?,
                content_hash: r.get(7)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows.into_iter().map(|r| (r.chunk_id.clone(), r)).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::artifact::{self, TestArtifactRowBuilder};
    use crate::librarian::catalog::Catalog;

    /// `catalog/mod.rs`'s test module has no shared `art()` helper (Task 4
    /// noted this too) — build the row locally via `TestArtifactRowBuilder`.
    fn art(id: &str, kind: &str, status: &str) -> artifact::ArtifactRow {
        TestArtifactRowBuilder::new(id)
            .with_kind(kind)
            .with_status(status)
            .build()
    }

    #[test]
    fn build_chunks_carries_line_ranges_and_entry_tokens() {
        let body = "# Log\n\npreamble\n\n## W-81 — a title\n\nbody text\n";
        let rows = build_chunks("a", body, 2048, 0);
        assert!(rows.len() >= 2, "preamble and entry are separate chunks");
        assert_eq!(rows[0].entry_token, None, "the preamble is inside no entry");
        // Pinned to real output (verified by a one-off probe run before writing
        // this): chunk 0 is the discriminating case for an off-by-one — it's
        // the only chunk here whose end is not the file's last line, so an
        // inclusive vs exclusive end-line reading would disagree on it. The
        // brief's `w.start_line <= 5 && w.end_line >= 7` is one-sided in BOTH
        // directions and cannot see `start_line.saturating_sub(1)` or
        // `end_line + 1` — both mutations survived the old assertion.
        assert_eq!((rows[0].start_line, rows[0].end_line), (1, 4));
        let w = rows
            .iter()
            .find(|r| r.entry_token.as_deref() == Some("W-81"))
            .unwrap();
        assert_eq!(
            (w.start_line, w.end_line),
            (5, 7),
            "range brackets the entry exactly"
        );
    }

    #[test]
    fn a_line_offset_shifts_every_range_and_moves_no_entry_token() {
        // Guards the ARITHMETIC site. The threading site — whether production
        // ever passes a non-zero offset — is guarded separately in
        // indexer.rs's `a_stored_chunks_range_points_at_that_text_in_the_file`;
        // one kill here says nothing about the other, per the once-per-site law.
        let body = "# Log\n\npre\n\n## W-2 — second\n\nbeta\n";
        let at_zero = build_chunks("a", body, 2048, 0);
        let at_four = build_chunks("a", body, 2048, 4);

        assert_eq!(at_zero[0].start_line, 1, "offset 0 is still body-relative");
        assert!(
            at_four
                .iter()
                .any(|r| r.entry_token.as_deref() == Some("W-2")),
            "LOAD-BEARING: the fixture must contain an entry token, or the \
             token half of this test is vacuous"
        );
        assert_eq!(at_zero.len(), at_four.len());

        for (z, f) in at_zero.iter().zip(at_four.iter()) {
            assert_eq!(f.start_line, z.start_line + 4);
            assert_eq!(f.end_line, z.end_line + 4);
            // The token is keyed on the UNSHIFTED line and must NOT travel with
            // the range. Folding the offset in before `tokens.get(..)` leaves
            // every range correct and slides every token one entry along:
            // measured, the preamble inherited `W-2` and the real `W-2` chunk
            // read `None`. No range assertion can see that.
            assert_eq!(f.entry_token, z.entry_token);
            // The hash is over `content` alone, so shifting the coordinate
            // space preserves it. That is not a detail — it is why correcting
            // the offset on an already-indexed corpus costs no re-embedding:
            // replace_chunks matches on (chunk_ix, content_hash), keeps the id
            // and the vector, and re-syncs the position fields only.
            assert_eq!(f.content, z.content);
            assert_eq!(f.content_hash, z.content_hash);
        }
    }

    fn write_project_toml(dir: &std::path::Path, body: &str) {
        let cfg = dir.join(".codescout");
        std::fs::create_dir_all(&cfg).unwrap();
        std::fs::write(cfg.join("project.toml"), body).unwrap();
    }

    #[test]
    fn chunk_grain_is_on_unless_a_project_opts_out() {
        let d = tempfile::tempdir().unwrap();
        assert_eq!(
            ChunkGrain::resolve(None),
            ChunkGrain::Chunk,
            "no project at all"
        );
        assert_eq!(
            ChunkGrain::resolve(d.path().to_str()),
            ChunkGrain::Chunk,
            "a project with no .codescout/project.toml"
        );
        write_project_toml(d.path(), "[project]\nname = \"x\"\n");
        assert_eq!(
            ChunkGrain::resolve(d.path().to_str()),
            ChunkGrain::Chunk,
            "a project.toml with no [librarian] section"
        );
        write_project_toml(d.path(), "[librarian]\nvector_backend = \"qdrant\"\n");
        assert_eq!(
            ChunkGrain::resolve(d.path().to_str()),
            ChunkGrain::Chunk,
            "a [librarian] section that does not mention chunk_grain"
        );
    }

    #[test]
    fn only_a_literal_false_opts_out_and_every_near_miss_stays_on() {
        let d = tempfile::tempdir().unwrap();
        write_project_toml(d.path(), "[librarian]\nchunk_grain = false\n");
        assert_eq!(
            ChunkGrain::resolve(d.path().to_str()),
            ChunkGrain::Artifact,
            "the opt-out leg — without it every assertion below is satisfied \
             by a resolve() that returns Chunk unconditionally"
        );

        // Each row is a plausible way to write "off" that is not `= false`. All are
        // silently ignored, and since the default was inverted that silence now
        // fails SAFE: a mistyped opt-out costs embedding time and leaves search
        // working. Under the previous default the same typo cost a third of this
        // corpus its vectors, because artifact grain's oversize failures are silent
        // — which is the asymmetry that decided the flip, not the ranking numbers
        // alone.
        for (label, text) in [
            ("explicit true", "[librarian]\nchunk_grain = true\n"),
            ("a quoted string", "[librarian]\nchunk_grain = \"false\"\n"),
            ("an integer 0", "[librarian]\nchunk_grain = 0\n"),
            ("the wrong section", "[project]\nchunk_grain = false\n"),
            ("unparseable TOML", "[librarian\nchunk_grain = false\n"),
        ] {
            write_project_toml(d.path(), text);
            assert_eq!(
                ChunkGrain::resolve(d.path().to_str()),
                ChunkGrain::Chunk,
                "{label} must not opt out"
            );
        }
    }

    #[test]
    fn a_huge_chunk_size_still_splits_build_chunks_at_every_heading() {
        // LOAD-BEARING, and the reason `build_single_chunk` exists as code rather
        // than as a number. `split_markdown_with_depth` starts a section at every
        // heading of level <= depth BEFORE consulting the character budget, so the
        // budget bounds a section and can never merge two. The shortcut this reds
        // is "artifact grain is just build_chunks with a big chunk_size", which
        // compiles, runs, and silently keeps chunk-grain costs at every heading.
        let body = "# A\n\nx\n\n## B\n\ny\n\n### C\n\nz\n";
        let rows = build_chunks("a", body, 1_000_000, 0);
        assert_eq!(
            rows.len(),
            3,
            "headings split regardless of the character budget"
        );
    }

    #[test]
    fn a_single_chunk_spans_the_whole_body_and_claims_no_entry() {
        let body = "# Log\n\npreamble\n\n## W-1 — first\n\nalpha\n\n## W-2 — second\n\nbeta\n";

        // The CONTRAST is the assertion. Without this leg, a build_single_chunk
        // that returned the first section only would satisfy everything below on
        // a fixture that happened to be single-section anyway.
        let chunked = build_chunks("a", body, 2048, 0);
        assert!(
            chunked.len() >= 3,
            "fixture must be multi-chunk at chunk grain or this test proves nothing (got {})",
            chunked.len()
        );

        let rows = build_single_chunk("a", body, 0);
        assert_eq!(rows.len(), 1, "artifact grain is exactly one row");
        let r = &rows[0];
        assert_eq!(r.content, body, "the WHOLE body, not its first section");
        assert_eq!(
            (r.start_line, r.end_line),
            (1, 11),
            "the range brackets the entire document"
        );
        assert_eq!(
            r.entry_token, None,
            "a whole-document chunk belongs to no single entry — None is the true \
             answer, and naming W-1 would be a wrong one"
        );
        assert_eq!(r.chunk_ix, 0);
    }

    #[test]
    fn a_single_chunks_range_is_file_relative_like_every_other_chunk_row() {
        // Same coordinate space as build_chunks, for the same reason: these
        // numbers leave the process as FILE lines through `matched`. A grain
        // switch must not change what a line number means.
        let body = "# A\n\nx\n"; // 3 lines
        let rows = build_single_chunk("a", body, 7);
        assert_eq!(
            (rows[0].start_line, rows[0].end_line),
            (8, 10),
            "1+offset ..= lines+offset"
        );
    }

    #[test]
    fn an_empty_body_yields_no_rows_at_either_grain() {
        // Absence assertions, so the sibling call is what makes them mean
        // anything: the claim is that the two grains AGREE on empty, and a
        // build_single_chunk that returned a row for "" would break the
        // `items.is_empty()` contract indexer.rs's empty-body test relies on.
        assert!(build_single_chunk("a", "", 0).is_empty());
        assert!(
            build_chunks("a", "", 2048, 0).is_empty(),
            "the behaviour being matched"
        );
    }

    #[test]
    fn replace_chunks_preserves_ids_for_unchanged_chunks() {
        // This is what stops a re-index re-embedding an untouched 766 KB tracker.
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &art("a", "tracker", "active")).unwrap();
        let first = build_chunks("a", "# T\n\nx\n\n## W-1 — t\n\ny\n", 2048, 0);
        let stored1 = replace_chunks(&cat, "a", &first).unwrap();
        let stored2 = replace_chunks(&cat, "a", &first).unwrap();
        assert_eq!(
            stored1.iter().map(|r| &r.chunk_id).collect::<Vec<_>>(),
            stored2.iter().map(|r| &r.chunk_id).collect::<Vec<_>>(),
            "identical content must keep identical chunk ids"
        );
    }

    #[test]
    fn replace_chunks_drops_chunks_that_no_longer_exist() {
        // Absence assertion — pair it with the positive leg below, or a
        // replace that deletes EVERYTHING also passes.
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &art("a", "tracker", "active")).unwrap();
        let long = build_chunks("a", "# T\n\n## A-1 — x\n\na\n\n## A-2 — y\n\nb\n", 2048, 0);
        replace_chunks(&cat, "a", &long).unwrap();
        let short = build_chunks("a", "# T\n\n## A-1 — x\n\na\n", 2048, 0);
        replace_chunks(&cat, "a", &short).unwrap();
        let stored = chunks_for(&cat, "a").unwrap();
        assert_eq!(
            stored.len(),
            short.len(),
            "shrunk body drops the trailing chunks"
        );
        assert!(
            stored
                .iter()
                .any(|r| r.entry_token.as_deref() == Some("A-1")),
            "and KEEPS the surviving one — without this the test passes on total deletion"
        );
    }

    fn vec_row_count(cat: &Catalog, chunk_id: &str) -> i64 {
        cat.conn
            .query_row(
                "SELECT COUNT(*) FROM artifact_vec_v2 WHERE id = ?1",
                [chunk_id],
                |r| r.get(0),
            )
            .unwrap()
    }

    fn seed_vec_row(cat: &Catalog, chunk_id: &str) {
        cat.conn
            .execute(
                "INSERT INTO artifact_vec_v2 (id, embedding) VALUES (?1, ?2)",
                rusqlite::params![chunk_id, vec![0u8; 768 * 4]],
            )
            .unwrap();
    }

    #[test]
    fn an_unchanged_chunk_keeps_its_vector_across_a_reindex() {
        // LOAD-BEARING: the id-preservation test alone cannot see this — vectors live
        // in artifact_vec_v2, which Task 5 never writes. A blanket DELETE fires
        // artifact_vec_v2_cascade_delete and destroys the embedding whose id was just
        // preserved, making preservation pointless while every other test stays green.
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &art("a", "tracker", "active")).unwrap();
        let body = "# T\n\nx\n\n## W-1 — t\n\ny\n";
        let first = build_chunks("a", body, 2048, 0);
        let stored1 = replace_chunks(&cat, "a", &first).unwrap();
        let chunk_id = stored1[0].chunk_id.clone();
        seed_vec_row(&cat, &chunk_id);

        // Re-index with IDENTICAL content — the case replace_chunks exists for.
        let second = build_chunks("a", body, 2048, 0);
        let stored2 = replace_chunks(&cat, "a", &second).unwrap();
        assert_eq!(stored2[0].chunk_id, chunk_id, "id preservation still holds");
        assert_eq!(
            vec_row_count(&cat, &chunk_id),
            1,
            "unchanged content must keep its vector across a re-index"
        );
    }
    #[test]
    fn an_unchanged_chunk_gets_its_line_range_resynced_when_content_above_it_shifts() {
        // LOAD-BEARING: edit the PREAMBLE only. Chunk 0's hash changes; every chunk
        // below keeps byte-identical content at a shifted start_line. Asserting only
        // that the chunk_id survived (as the round-1 test does) passes against a
        // stale-position bug — assert the LINE NUMBERS moved, and that the vector
        // (keyed by content_hash, not position) still survived the resync.
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &art("a", "tracker", "active")).unwrap();
        let body = "# T\n\nx\n\n## W-1 — t\n\ny\n";
        let first = build_chunks("a", body, 2048, 0);
        let stored1 = replace_chunks(&cat, "a", &first).unwrap();
        let w1 = stored1
            .iter()
            .find(|r| r.entry_token.as_deref() == Some("W-1"))
            .unwrap();
        let chunk_id = w1.chunk_id.clone();
        let original_start_line = w1.start_line;
        seed_vec_row(&cat, &chunk_id);

        // Insert a line into the preamble only — the W-1 entry's own content is
        // byte-identical, but its position in the body has shifted down by one.
        let shifted_body = "# T\n\nx\nANOTHER LINE\n\n## W-1 — t\n\ny\n";
        let second = build_chunks("a", shifted_body, 2048, 0);
        replace_chunks(&cat, "a", &second).unwrap();

        // Re-fetch from the DB — the return value of replace_chunks is built
        // from the freshly computed rows regardless of what was persisted, so
        // asserting on it would pass even if the UPDATE never ran. Only a
        // fresh chunks_for() proves what actually landed in artifact_chunk.
        let persisted = chunks_for(&cat, "a").unwrap();
        let w2 = persisted
            .iter()
            .find(|r| r.entry_token.as_deref() == Some("W-1"))
            .unwrap();

        assert_eq!(w2.chunk_id, chunk_id, "id preservation still holds");
        assert_eq!(
            w2.start_line,
            original_start_line + 1,
            "persisted line range must resync to the new position"
        );
        assert_eq!(
            vec_row_count(&cat, &chunk_id),
            1,
            "resyncing position must not disturb the content-keyed vector"
        );
    }

    #[test]
    fn a_changed_chunk_loses_its_vector_across_a_reindex() {
        // Negative leg — without it, "the vector survived" above would also be
        // satisfied by a replace_chunks that never deletes anything at all.
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &art("a", "tracker", "active")).unwrap();
        let first = build_chunks("a", "# T\n\nx\n\n## W-1 — t\n\ny\n", 2048, 0);
        let stored1 = replace_chunks(&cat, "a", &first).unwrap();
        let chunk_id = stored1[0].chunk_id.clone();
        seed_vec_row(&cat, &chunk_id);

        // Re-index with DIFFERENT content at the same chunk_ix (the preamble).
        let second = build_chunks("a", "# T\n\nCHANGED\n\n## W-1 — t\n\ny\n", 2048, 0);
        replace_chunks(&cat, "a", &second).unwrap();

        assert_eq!(
            vec_row_count(&cat, &chunk_id),
            0,
            "changed content must lose its now-stale vector"
        );
    }
}
