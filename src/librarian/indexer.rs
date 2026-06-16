use anyhow::Result;
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
    pub unknown_ids: Vec<String>,
}

/// Items queued for embedding: `(artifact_id, title, body_chunk_text)`.
pub type EmbedQueueItem = (String, Option<String>, String);

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

/// Synchronous part of indexing: walk files, upsert artifact rows, collect embedding queue.
/// Returns `(report, embed_queue)` where `embed_queue` is a list of [`EmbedQueueItem`].
pub fn index_repo_sync(
    cat: &Catalog,
    rules: &[CompiledRule],
    abs_root: &Path,
    ignore: &globset::GlobSet,
    want_embeddings: bool,
    force_rewalk: bool,
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

    let walker = WalkBuilder::new(abs_root).standard_filters(true).build();
    for entry in walker.flatten() {
        let path = entry.path();
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
        artifact::upsert(cat, &row)?;

        // Only (re-)embed when content actually changed. Re-classification
        // alone does not require recomputing the embedding.
        if want_embeddings && !content_unchanged {
            let chunks = codescout_embed::chunk_markdown(body, 512);
            let first_chunk = chunks
                .into_iter()
                .next()
                .unwrap_or_else(|| body.to_string());
            embed_queue.push((id.clone(), title, first_chunk));
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

    // Delete rows under abs_root that were not seen in this walk.
    let root_prefix = format!(
        "{}/",
        crate::util::fs::RepoPath::from(abs_root)
            .as_str()
            .replace('\'', "''")
    );
    let removed = if seen_ids.is_empty() {
        cat.conn.execute(
            "DELETE FROM artifact WHERE abs_path LIKE ?1",
            rusqlite::params![format!("{root_prefix}%")],
        )?
    } else {
        let placeholders = seen_ids
            .iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", i + 2))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "DELETE FROM artifact WHERE abs_path LIKE ?1 AND id NOT IN ({})",
            placeholders
        );
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(format!("{root_prefix}%"))];
        for id in &seen_ids {
            params.push(Box::new(id.clone()));
        }
        cat.conn.execute(
            &sql,
            rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())),
        )?
    };
    report.removed = removed;

    Ok((report, embed_queue))
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
pub fn write_embeddings(cat: &Catalog, embeddings: &[(String, Vec<f32>)]) -> Result<()> {
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
            anyhow::bail!(
                "embedding dim mismatch vs catalog: batch={}, existing={}. \
                 Likely causes: (1) embedder is misconfigured and returns error \
                 sentinels with wrong dim (the F-6b case — vec.len()=1), (2) the \
                 configured embedder model changed without a full re-embed pipeline. \
                 To rebuild with a new model, drop `artifact_vec` rows explicitly \
                 first; do NOT use `reindex(force=true)` (bug-tracker #6/#7).",
                batch_dim,
                existing_dim
            );
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
    let (mut report, embed_queue) = index_repo_sync(cat, rules, abs_root, ignore, want, false)?;

    if let Some(svc) = embedding {
        let futures_iter = embed_queue
            .into_iter()
            .map(|(id, title, chunk_text)| async move {
                let vec = svc.embed_artifact(title.as_deref(), &chunk_text).await?;
                anyhow::Ok((id, vec))
            });
        let mut stream = stream::iter(futures_iter).buffer_unordered(EMBED_CONCURRENCY);
        let mut batch: Vec<(String, Vec<f32>)> = Vec::with_capacity(100);
        while let Some(res) = stream.next().await {
            batch.push(res?);
            if batch.len() >= 100 {
                if let Some(s) = store {
                    for (id, vec) in &batch {
                        s.upsert(project_id, id, vec).await?;
                    }
                } else {
                    write_embeddings(cat, &batch)?;
                }
                report.embedded += batch.len();
                batch.clear();
            }
        }
        if !batch.is_empty() {
            report.embedded += batch.len();
            if let Some(s) = store {
                for (id, vec) in &batch {
                    s.upsert(project_id, id, vec).await?;
                }
            } else {
                write_embeddings(cat, &batch)?;
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
        let (report, _) = index_repo_sync(&cat, &rules, &fixture, &ignore, false, false).unwrap();
        assert_eq!(report.added, 3, "should index 3 .md files");
        assert_eq!(report.unknown_ids.len(), 1, "README.md is unknown");

        let (r2, _) = index_repo_sync(&cat, &rules, &fixture, &ignore, false, false).unwrap();
        assert_eq!(r2.unchanged, 3);
        assert_eq!(r2.added, 0);
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
        let (report, queue) = index_repo_sync(&cat, &rules, &wt, &ignore, false, false).unwrap();
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

        let (r1, _) = index_repo_sync(&cat, &rules, root, &ignore, false, false).unwrap();
        assert_eq!(r1.added, 2);

        std::fs::remove_file(root.join("docs/specs/b.md")).unwrap();
        let (r2, _) = index_repo_sync(&cat, &rules, root, &ignore, false, false).unwrap();
        assert_eq!(r2.removed, 1);
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
        index_repo_sync(&cat, &rules, root, &ignore, false, false).unwrap();
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
        index_repo_sync(&cat, &rules, root, &ignore, false, false).unwrap();

        // 5. Fresh.
        let fresh = crate::librarian::catalog::artifact::get(&cat, &id)
            .unwrap()
            .unwrap();
        assert_eq!(fresh.title.as_deref(), Some("Updated"));
    }

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
            index_repo_sync(&cat, &rules, root, &ignore, true, false).unwrap();
        assert_eq!(report.added, 1);

        // Phase 2: embed
        let mut computed: Vec<(String, Vec<f32>)> = Vec::new();
        for (id, title, chunk_text) in &embed_queue {
            let vec = svc
                .embed_artifact(title.as_deref(), chunk_text)
                .await
                .unwrap();
            computed.push((id.clone(), vec));
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
        index_repo_sync(&cat, &no_rules, root, &ignore, false, false).unwrap();
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
        index_repo_sync(&cat, &with_rules, root, &ignore, false, false).unwrap();

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

        let (r, _) = index_repo_sync(&cat, &rules, root, &ignore, false, false).unwrap();
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

        let (report, _) = index_repo_sync(&cat, &rules, root, &ignore, false, false).unwrap();
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
        let (report, _) = index_repo_sync(&cat, &rules, root, &ignore, false, false).unwrap();
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
        index_repo_sync(&cat, &rules, root, &ignore, false, false).unwrap();

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
        index_repo_sync(&cat, &rules, root, &ignore, false, false).unwrap();

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
            .query_row("SELECT count(*) FROM artifact_vec", [], |row| row.get(0))
            .unwrap();
        assert_eq!(
            count, 16,
            "all 16 embeddings must be written via buffer_unordered"
        );
    }
}
