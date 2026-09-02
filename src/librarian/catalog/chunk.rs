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
    pub start_line: usize,
    pub end_line: usize,
    pub entry_token: Option<String>,
    pub content: String,
    pub content_hash: String,
}

/// Chunk `body` at the librarian's grain: heading depth 6, so `####`-defined
/// entries start their own chunk. `chunk_size` is the CHARACTER budget and
/// stays 2,048 (512 tokens) — see the plan's Global Constraints before
/// touching it.
pub fn build_chunks(artifact_id: &str, body: &str, chunk_size: usize) -> Vec<ChunkRow> {
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
                entry_token: tokens.get(raw.start_line).cloned().flatten(),
                start_line: raw.start_line,
                end_line: raw.end_line,
                content: raw.content,
                content_hash: format!("{:x}", hasher.finalize()),
            }
        })
        .collect()
}

/// Replace an artifact's chunk rows, preserving `chunk_id` wherever
/// `(chunk_ix, content_hash)` is unchanged so untouched chunks keep their
/// vectors. Returns the rows as stored.
pub fn replace_chunks(
    cat: &Catalog,
    artifact_id: &str,
    rows: &[ChunkRow],
) -> Result<Vec<ChunkRow>> {
    let existing = chunks_for(cat, artifact_id)?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let reuse = existing
            .iter()
            .find(|e| e.chunk_ix == row.chunk_ix && e.content_hash == row.content_hash)
            .map(|e| e.chunk_id.clone());
        let mut stored = row.clone();
        stored.chunk_id = reuse.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        out.push(stored);
    }

    cat.conn.execute(
        "DELETE FROM artifact_chunk WHERE artifact_id = ?1",
        [artifact_id],
    )?;
    let mut stmt = cat.conn.prepare(
        "INSERT INTO artifact_chunk
           (chunk_id, artifact_id, chunk_ix, start_line, end_line, entry_token, content, content_hash)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
    )?;
    for r in &out {
        stmt.execute(rusqlite::params![
            r.chunk_id,
            r.artifact_id,
            r.chunk_ix as i64,
            r.start_line as i64,
            r.end_line as i64,
            r.entry_token,
            r.content,
            r.content_hash
        ])?;
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
        let rows = build_chunks("a", body, 2048);
        assert!(rows.len() >= 2, "preamble and entry are separate chunks");
        assert_eq!(rows[0].entry_token, None, "the preamble is inside no entry");
        let w = rows
            .iter()
            .find(|r| r.entry_token.as_deref() == Some("W-81"))
            .unwrap();
        assert!(
            w.start_line <= 5 && w.end_line >= 7,
            "range brackets the entry"
        );
    }

    #[test]
    fn replace_chunks_preserves_ids_for_unchanged_chunks() {
        // This is what stops a re-index re-embedding an untouched 766 KB tracker.
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &art("a", "tracker", "active")).unwrap();
        let first = build_chunks("a", "# T\n\nx\n\n## W-1 — t\n\ny\n", 2048);
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
        let long = build_chunks("a", "# T\n\n## A-1 — x\n\na\n\n## A-2 — y\n\nb\n", 2048);
        replace_chunks(&cat, "a", &long).unwrap();
        let short = build_chunks("a", "# T\n\n## A-1 — x\n\na\n", 2048);
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
}
