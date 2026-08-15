use anyhow::{Context, Result};
use qdrant_client::qdrant::{
    Condition, DeletePointsBuilder, Distance, Filter, Fusion, Modifier, PointStruct, PointsIdsList,
    PrefetchQueryBuilder, Query, QueryPointsBuilder, ScrollPointsBuilder, SparseVectorBuilder,
    UpsertPointsBuilder, Vector, VectorInput,
};
use qdrant_client::qdrant::{
    CreateCollectionBuilder, SparseVectorParamsBuilder, SparseVectorsConfigBuilder,
    VectorParamsBuilder, VectorsConfigBuilder,
};
use qdrant_client::Qdrant;

/// Ceiling for a Qdrant *bootstrap* operation — either `QdrantWrap::connect`'s
/// own connection handshake, or a first-use collection-ensure call such as
/// `ensure_memories_collection`. Distinct from the 120s *operation* timeout
/// baked into the qdrant-client builder, which bounds individual RPCs once
/// connected but does not cover connection establishment itself. A
/// reachable-but-hung Qdrant (TCP accepts, no reply) would otherwise block a
/// caller for the full 120s (or longer — connection establishment isn't
/// covered by that timeout at all), which can exceed a host's session-init
/// budget. See docs/issues/archive/2026-06-24-qdrant-hang-wedges-mcp-startup.md.
pub const QDRANT_BOOTSTRAP_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

pub struct QdrantWrap {
    pub client: Qdrant,
}
/// Qdrant point IDs must be u64 or UUID — hash the chunk_id string to u64.
fn chunk_id_to_point_id(s: &str) -> u64 {
    use sha2::Digest;
    let hash = sha2::Sha256::digest(s.as_bytes());
    u64::from_le_bytes(hash[..8].try_into().unwrap())
}

/// The `Filter` every `hybrid_query` leg runs with.
///
/// Extracted as a pure function for two reasons. It is the ONE place the
/// filter shape is decided, so the dense prefetch, the sparse prefetch and the
/// dense-only branch cannot drift apart; and the shape is unit-testable
/// without a live Qdrant.
///
/// ## One `MatchAny`, not one condition per value (C2)
///
/// `exclude_paths` collapses into a **single** `MatchAny` condition. Qdrant
/// evaluates `must_not` conditions one at a time, and this collection carries
/// no payload index on `file_path`, so N conditions cost N passes. Measured on
/// the live 576k-point `code_chunks` collection, same query shape, identical
/// top-10 ids and scores both ways:
///
/// | excluded paths | one condition each | single `MatchAny` |
/// |---|---|---|
/// | 500   | 2.35 s  | 0.58 s |
/// | 2 000 | 9.32 s  | 0.56 s |
/// | 8 000 | 37.29 s | 0.57 s |
///
/// This is not a tail case. `sync_worktree` marks every file dirty whenever
/// main has never been indexed, so "every path in the repo lands in this
/// filter" is the *certain* state on a fresh main, not an unlucky one.
///
/// ## `overlay_project_id`: one ranking over two projects (C1)
///
/// With an overlay the filter covers **both** projects in a single query, and
/// the path exclusion is *nested* so it binds only to `project_id`:
///
/// ```text
/// must:     [ project_id MatchAny [primary, overlay] ]
/// must_not: [ language MatchAny [...],                       // both projects
///             Filter { must: [ project_id = primary,
///                              file_path MatchAny [...] ] } ] // primary only
/// ```
///
/// The nesting is the whole point. Flattening those two inner conditions up
/// into the outer `must_not` would exclude the paths from the **overlay** too
/// — and the overlay (a worktree delta) holds nothing *but* those paths, so
/// the flattened form silently returns main-minus-dirty and nothing else.
/// Dropping the nested `project_id` condition does the same thing.
///
/// Why one query rather than two merged lists: with `disable_sparse == false`
/// (the default) Qdrant's `Hit.score` is the RRF fusion score, a function of
/// **rank position only** — measured on the live collection as 0.5, 0.333,
/// 0.25, 0.2, 0.167 … = `1/(1 + rank)`. It carries no information about
/// content, corpus size or similarity, so a 3-chunk delta produces the same
/// top-3 scores as a 500k-chunk main index and a score-sorted merge hands the
/// delta half of every page regardless of relevance. Ranking both projects in
/// one query is what makes the scores mean the same thing. See
/// [`crate::retrieval::search::merge_hits`] and
/// [`crate::retrieval::code_store::CodeVectorStore::query_overlay`].
fn build_query_filter(
    project_id: &str,
    overlay_project_id: Option<&str>,
    exclude_languages: &[String],
    exclude_paths: &[String],
) -> Filter {
    let must = match overlay_project_id {
        Some(overlay) => vec![Condition::matches(
            "project_id",
            vec![project_id.to_string(), overlay.to_string()],
        )],
        None => vec![Condition::matches("project_id", project_id.to_string())],
    };

    let mut must_not: Vec<Condition> = Vec::new();
    // Guarded on non-empty: `Condition::matches(field, vec![])` would emit a
    // `MatchAny` over an empty keyword set, which is a real condition Qdrant
    // still has to evaluate rather than the no-op an empty list means here.
    if !exclude_languages.is_empty() {
        // Deliberately NOT nested: a language exclusion is the caller's
        // `mode="code"` preference and applies to everything on the page.
        must_not.push(Condition::matches("language", exclude_languages.to_vec()));
    }
    if !exclude_paths.is_empty() {
        must_not.push(match overlay_project_id {
            Some(_) => Condition::from(Filter {
                must: vec![
                    Condition::matches("project_id", project_id.to_string()),
                    Condition::matches("file_path", exclude_paths.to_vec()),
                ],
                ..Default::default()
            }),
            None => Condition::matches("file_path", exclude_paths.to_vec()),
        });
    }

    Filter {
        must,
        must_not,
        ..Default::default()
    }
}

impl QdrantWrap {
    /// Build the Qdrant client. `Qdrant::from_url(...).build()` is a plain,
    /// synchronous call that — despite appearances — performs a blocking
    /// connection handshake: against a reachable-but-unresponsive Qdrant (TCP
    /// accepts, no reply), it blocks the calling thread indefinitely with no
    /// yield point, so an `await`-side `tokio::time::timeout` around it cannot
    /// preempt it. Routing it through `spawn_blocking` moves that blocking work
    /// off the async executor and makes it interruptible via `timeout` here.
    /// See docs/issues/archive/2026-06-24-qdrant-hang-wedges-mcp-startup.md.
    pub async fn connect(url: &str) -> Result<Self> {
        let owned_url = url.to_string();
        let client = tokio::time::timeout(
            QDRANT_BOOTSTRAP_TIMEOUT,
            tokio::task::spawn_blocking(move || {
                Qdrant::from_url(&owned_url)
                    .timeout(std::time::Duration::from_secs(120))
                    // Disarm qdrant-client's version probe. It is not just
                    // redundant work: on failure it `println!`s onto OUR
                    // stdout (qdrant-client 1.17 `qdrant_client/mod.rs:143`),
                    // which prepends prose to every `--json` CLI envelope
                    // whenever Qdrant is unreachable — and it blocks on a
                    // health check inside the very `build()` this function
                    // already wraps to survive. Both failure modes, one knob.
                    // docs/issues/archive/2026-08-08-qdrant-compat-check-printlns-to-stdout.md
                    .skip_compatibility_check()
                    .build()
                    .context("qdrant connect")
            }),
        )
        .await
        .map_err(|_| {
            anyhow::anyhow!(
                "timed out connecting to Qdrant at {url} after {QDRANT_BOOTSTRAP_TIMEOUT:?} \
                 (reachable but unresponsive?)"
            )
        })?
        .context("qdrant connect task panicked")??;
        Ok(Self { client })
    }

    pub async fn collection_exists(&self, name: &str) -> Result<bool> {
        self.client
            .collection_exists(name)
            .await
            .context("collection_exists")
    }

    /// Ensure a collection exists with a named dense vector ("dense", Cosine, `dim` dimensions)
    /// and a named sparse vector ("sparse", IDF modifier). Idempotent — no-op if the collection
    /// already exists.
    pub async fn ensure_collection(&self, name: &str, dim: u64) -> Result<()> {
        if self.collection_exists(name).await? {
            return Ok(());
        }

        let mut vectors = VectorsConfigBuilder::default();
        vectors.add_named_vector_params("dense", VectorParamsBuilder::new(dim, Distance::Cosine));

        let mut sparse = SparseVectorsConfigBuilder::default();
        sparse.add_named_vector_params(
            "sparse",
            SparseVectorParamsBuilder::default().modifier(Modifier::Idf),
        );

        self.client
            .create_collection(
                CreateCollectionBuilder::new(name)
                    .vectors_config(vectors)
                    .sparse_vectors_config(sparse),
            )
            .await
            .context("create_collection")?;

        Ok(())
    }

    /// Scroll all chunk refs for a project, paginating until exhausted.
    pub async fn scroll_chunk_refs(
        &self,
        collection: &str,
        project_id: &str,
    ) -> Result<Vec<crate::retrieval::drift::ChunkRef>> {
        let filter = Filter::must([Condition::matches("project_id", project_id.to_string())]);

        let mut refs = Vec::new();
        let mut offset: Option<qdrant_client::qdrant::PointId> = None;

        loop {
            let mut builder = ScrollPointsBuilder::new(collection)
                .filter(filter.clone())
                // Only these three keys are read below. `with_payload(true)` pulled
                // every chunk's `content` over the wire — on every sync, since
                // `stream_index` diffs against this — to compare two hashes.
                .with_payload(qdrant_client::qdrant::PayloadIncludeSelector {
                    fields: vec![
                        "chunk_id".to_string(),
                        "content_hash".to_string(),
                        "file_path".to_string(),
                    ],
                })
                .with_vectors(false)
                .limit(1000u32);

            if let Some(off) = offset.take() {
                builder = builder.offset(off);
            }

            let resp = self
                .client
                .scroll(builder)
                .await
                .context("scroll_chunk_refs")?;

            for pt in &resp.result {
                let chunk_id = pt
                    .get("chunk_id")
                    .as_str()
                    .map(|s| s.as_str().to_owned())
                    .unwrap_or_default();
                let content_hash = pt
                    .get("content_hash")
                    .as_str()
                    .map(|s| s.as_str().to_owned())
                    .unwrap_or_default();
                let file_path = pt
                    .get("file_path")
                    .as_str()
                    .map(|s| s.as_str().to_owned())
                    .unwrap_or_default();
                if !chunk_id.is_empty() {
                    refs.push(crate::retrieval::drift::ChunkRef {
                        chunk_id,
                        content_hash,
                        file_path,
                    });
                }
            }

            match resp.next_page_offset {
                None => break,
                Some(next) => offset = Some(next),
            }
        }

        Ok(refs)
    }

    /// Existence only: does this project have at least one chunk?
    ///
    /// One scroll, one page, no payloads, no vectors — constant work regardless of
    /// corpus size. The sibling `project_index_stats` below cannot answer this
    /// cheaply because `file_count` requires enumerating every point.
    pub async fn project_has_chunks(&self, collection: &str, project_id: &str) -> Result<bool> {
        use qdrant_client::qdrant::{Condition, Filter, ScrollPointsBuilder};

        let filter = Filter::must([Condition::matches("project_id", project_id.to_string())]);
        let resp = self
            .client
            .scroll(
                ScrollPointsBuilder::new(collection)
                    .filter(filter)
                    .with_payload(false)
                    .with_vectors(false)
                    .limit(1u32),
            )
            .await
            .context("project_has_chunks")?;

        Ok(!resp.result.is_empty())
    }

    /// Scroll all chunks for a project and return summary stats:
    /// `(chunk_count, file_count)` where `file_count` is distinct `file_path`
    /// values in the payload. Used by IndexStatus to surface the same numbers
    /// the legacy sqlite stats used to report.
    pub async fn project_index_stats(
        &self,
        collection: &str,
        project_id: &str,
    ) -> Result<(usize, usize)> {
        use qdrant_client::qdrant::{Condition, Filter, ScrollPointsBuilder};

        let filter = Filter::must([Condition::matches("project_id", project_id.to_string())]);

        let mut chunk_count: usize = 0;
        let mut files: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut offset: Option<qdrant_client::qdrant::PointId> = None;

        loop {
            let mut builder = ScrollPointsBuilder::new(collection)
                .filter(filter.clone())
                // Only `file_path` is read below; `with_payload(true)` pulled every
                // chunk's `content` over the wire to count distinct files.
                .with_payload(qdrant_client::qdrant::PayloadIncludeSelector {
                    fields: vec!["file_path".to_string()],
                })
                .with_vectors(false)
                .limit(1000u32);

            if let Some(off) = offset.take() {
                builder = builder.offset(off);
            }

            let resp = self
                .client
                .scroll(builder)
                .await
                .context("project_index_stats")?;

            for pt in &resp.result {
                chunk_count += 1;
                if let Some(s) = pt.get("file_path").as_str() {
                    files.insert(s.as_str().to_string());
                }
            }

            match resp.next_page_offset {
                None => break,
                Some(next) => offset = Some(next),
            }
        }

        Ok((chunk_count, files.len()))
    }

    pub async fn upsert_points(
        &self,
        collection: &str,
        points: &[(
            String,
            std::collections::HashMap<String, qdrant_client::qdrant::Value>,
            crate::retrieval::embedder::EmbedOutput,
        )],
    ) -> Result<()> {
        if points.is_empty() {
            return Ok(());
        }

        let structs: Vec<PointStruct> = points
            .iter()
            .map(|(chunk_id, payload, embed)| {
                let mut named: std::collections::HashMap<String, Vector> =
                    std::collections::HashMap::new();
                named.insert("dense".to_owned(), embed.dense.clone().into());
                named.insert(
                    "sparse".to_owned(),
                    SparseVectorBuilder::new(
                        embed.sparse.indices.clone(),
                        embed.sparse.values.clone(),
                    )
                    .into(),
                );
                PointStruct::new(chunk_id_to_point_id(chunk_id), named, payload.clone())
            })
            .collect();

        // Upsert in bounded chunks: a single large upsert (thousands of
        // dense+sparse points) can exceed the Qdrant client timeout
        // ("operation was cancelled / Timeout expired"). Smaller batches keep
        // each gRPC call well under it.
        const UPSERT_BATCH: usize = 256;
        for batch in structs.chunks(UPSERT_BATCH) {
            self.client
                .upsert_points(UpsertPointsBuilder::new(collection, batch.to_vec()).wait(true))
                .await
                .context("upsert_points")?;
        }

        Ok(())
    }

    /// Hybrid RRF query: two prefetch legs (dense cosine + sparse BM25), fused
    /// with Reciprocal Rank Fusion. Returns decoded `Hit` values. Points whose
    /// payload cannot be decoded are silently skipped.
    ///
    /// `bm25_boost` multiplies the sparse candidate pool relative to dense.
    /// 1.0 = equal weight; 2.0 = sparse fetches 2× more candidates before RRF.
    /// `disable_sparse` skips the sparse leg entirely → pure dense ANN ranking.
    /// `exclude_languages` adds a `must_not` clause on the payload `language`
    /// field (empty = no filter). `exclude_paths` adds a `must_not` clause on the
    /// payload `file_path` field (empty = no filter). Used for
    /// `semantic_search(mode="code")` and worktree search respectively. Each is
    /// exactly ONE `MatchAny` condition however long the list — see
    /// [`build_query_filter`], which owns the shape and carries the measurements.
    ///
    /// `overlay_project_id` unions a second project into the **same** RRF
    /// ranking, with `exclude_paths` nested so it binds only to `project_id`.
    /// That is the worktree main+delta query, and it must stay one query:
    /// running two and merging by score is what C1 was. Again, see
    /// [`build_query_filter`].
    #[allow(clippy::too_many_arguments)]
    pub async fn hybrid_query(
        &self,
        collection: &str,
        project_id: &str,
        overlay_project_id: Option<&str>,
        dense: &[f32],
        sparse: &crate::retrieval::embedder::SparseVector,
        limit: usize,
        bm25_boost: f32,
        disable_sparse: bool,
        exclude_languages: &[String],
        exclude_paths: &[String],
    ) -> Result<Vec<crate::retrieval::search::Hit>> {
        let filter = build_query_filter(
            project_id,
            overlay_project_id,
            exclude_languages,
            exclude_paths,
        );

        let resp = if disable_sparse {
            // Pure dense ANN — no fusion, no sparse leg.
            let req = QueryPointsBuilder::new(collection)
                .query(Query::new_nearest(VectorInput::new_dense(dense.to_vec())))
                .using("dense")
                .filter(filter)
                .limit(limit as u64)
                .with_payload(true)
                .build();
            self.client
                .query(req)
                .await
                .context("hybrid_query (dense-only)")?
        } else {
            let sparse_limit = ((limit as f32) * bm25_boost.max(0.1)).ceil() as u64;

            let dense_prefetch = PrefetchQueryBuilder::default()
                .query(Query::new_nearest(VectorInput::new_dense(dense.to_vec())))
                .using("dense")
                .filter(filter.clone())
                .limit(limit as u64)
                .build();

            let sparse_prefetch = PrefetchQueryBuilder::default()
                .query(Query::new_nearest(VectorInput::new_sparse(
                    sparse.indices.clone(),
                    sparse.values.clone(),
                )))
                .using("sparse")
                .filter(filter.clone())
                .limit(sparse_limit)
                .build();

            let req = QueryPointsBuilder::new(collection)
                .add_prefetch(dense_prefetch)
                .add_prefetch(sparse_prefetch)
                .query(Query::new_fusion(Fusion::Rrf))
                .limit(limit as u64)
                .with_payload(true)
                .build();

            self.client.query(req).await.context("hybrid_query")?
        };

        let hits = resp
            .result
            .into_iter()
            .filter_map(|pt| {
                let score = pt.score;
                let p = crate::retrieval::payload::map_to_payload(&pt.payload).ok()?;
                Some(crate::retrieval::search::Hit {
                    chunk_id: p.chunk_id,
                    file_path: p.file_path,
                    start_line: p.start_line,
                    end_line: p.end_line,
                    content: p.content,
                    score,
                    rerank_score: None,
                })
            })
            .collect();

        Ok(hits)
    }

    pub async fn delete_points(&self, collection: &str, ids: &[String]) -> Result<()> {
        if ids.is_empty() {
            return Ok(());
        }

        let point_ids: Vec<qdrant_client::qdrant::PointId> = ids
            .iter()
            .map(|id| chunk_id_to_point_id(id).into())
            .collect();

        self.client
            .delete_points(
                DeletePointsBuilder::new(collection)
                    .points(PointsIdsList { ids: point_ids })
                    .wait(true),
            )
            .await
            .context("delete_points")?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pure shape test for [`build_query_filter`] — no Qdrant required, so it
    /// runs in the ordinary suite rather than behind `--ignored`.
    ///
    /// The assertion that carries the weight is `must_not.len() == 2`: the
    /// shipped code emitted one condition **per excluded path**, which cost
    /// 37 s for 8 000 paths against the live collection (0.56 s collapsed).
    /// Re-expanding the list into per-path conditions — the natural thing for a
    /// later edit to do, since it reads more simply — fails right here instead
    /// of only showing up as latency nobody attributes to this function.
    #[test]
    fn build_query_filter_collapses_each_exclusion_list_into_one_match_any() {
        use qdrant_client::qdrant::{condition::ConditionOneOf, r#match::MatchValue};

        let paths: Vec<String> = (0..8000).map(|i| format!("src/f{i}.rs")).collect();
        let langs = vec!["markdown".to_string(), "json".to_string()];
        let filter = build_query_filter("proj", None, &langs, &paths);

        assert_eq!(filter.must.len(), 1, "exactly one project_id condition");
        assert_eq!(
            filter.must_not.len(),
            2,
            "8000 paths + 2 languages must collapse to ONE condition each, not \
             8002 conditions — got {}",
            filter.must_not.len()
        );

        // …and each one really is a MatchAny carrying the whole list, not a
        // single-value Keyword condition that silently drops the rest.
        let keywords = |c: &Condition| -> (String, Vec<String>) {
            match c.condition_one_of.as_ref().expect("condition set") {
                ConditionOneOf::Field(f) => match f
                    .r#match
                    .as_ref()
                    .and_then(|m| m.match_value.as_ref())
                    .expect("match value set")
                {
                    MatchValue::Keywords(k) => (f.key.clone(), k.strings.clone()),
                    other => panic!("expected MatchAny/Keywords, got {other:?}"),
                },
                other => panic!("expected a field condition, got {other:?}"),
            }
        };
        let (lang_key, lang_vals) = keywords(&filter.must_not[0]);
        assert_eq!(lang_key, "language");
        assert_eq!(lang_vals, langs);
        let (path_key, path_vals) = keywords(&filter.must_not[1]);
        assert_eq!(path_key, "file_path");
        assert_eq!(path_vals.len(), 8000);
        assert_eq!(path_vals[7999], "src/f7999.rs");
    }

    /// The empty case is a no-op, not a `MatchAny` over nothing. An empty
    /// `Keywords` set is still a condition Qdrant evaluates, and — worse — it
    /// matches nothing, so an empty `must_not` entry is harmless only by
    /// accident. Pinned so the non-empty guard cannot be dropped as redundant.
    #[test]
    fn build_query_filter_emits_no_condition_for_an_empty_exclusion_list() {
        let filter = build_query_filter("proj", None, &[], &[]);
        assert_eq!(filter.must.len(), 1);
        assert!(
            filter.must_not.is_empty(),
            "empty exclusion lists must produce zero must_not conditions, got {}",
            filter.must_not.len()
        );

        let only_paths = build_query_filter("proj", None, &[], &["a.rs".to_string()]);
        assert_eq!(
            only_paths.must_not.len(),
            1,
            "an empty language list must not leave a stray condition behind"
        );
    }

    /// C1's filter shape, pinned without a live Qdrant.
    ///
    /// With an overlay the query must cover BOTH projects (`must` is one
    /// `MatchAny` over the pair) while the path exclusion binds to the primary
    /// ONLY — expressed as a *nested* `Filter` inside `must_not`, whose inner
    /// `must` carries both `project_id = primary` and the path list.
    ///
    /// Two mutations this catches, both of which read as harmless
    /// simplifications:
    /// - flattening the nested conditions into the outer `must_not`: the paths
    ///   would then be excluded from the overlay too, and the overlay holds
    ///   *nothing but* those paths, so the worktree's own edits silently
    ///   disappear from every search;
    /// - dropping the inner `project_id` condition: same outcome, by the same
    ///   mechanism.
    #[test]
    fn build_query_filter_nests_the_path_exclusion_under_the_primary_project() {
        use qdrant_client::qdrant::{condition::ConditionOneOf, r#match::MatchValue};

        let keywords = |c: &Condition| -> (String, Vec<String>) {
            match c.condition_one_of.as_ref().expect("condition set") {
                ConditionOneOf::Field(f) => match f
                    .r#match
                    .as_ref()
                    .and_then(|m| m.match_value.as_ref())
                    .expect("match value set")
                {
                    MatchValue::Keywords(k) => (f.key.clone(), k.strings.clone()),
                    MatchValue::Keyword(s) => (f.key.clone(), vec![s.clone()]),
                    other => panic!("expected a keyword match, got {other:?}"),
                },
                other => panic!("expected a field condition, got {other:?}"),
            }
        };

        let paths = vec!["src/a.rs".to_string(), "src/b.rs".to_string()];
        let filter = build_query_filter("main", Some("main@wt"), &["markdown".into()], &paths);

        // Both projects are in scope for the ranking.
        assert_eq!(filter.must.len(), 1);
        let (must_key, must_vals) = keywords(&filter.must[0]);
        assert_eq!(must_key, "project_id");
        assert_eq!(must_vals, vec!["main".to_string(), "main@wt".to_string()]);

        assert_eq!(filter.must_not.len(), 2);

        // The language exclusion stays flat: it is the caller's mode="code"
        // preference and applies to everything on the page.
        let (lang_key, lang_vals) = keywords(&filter.must_not[0]);
        assert_eq!(lang_key, "language");
        assert_eq!(lang_vals, vec!["markdown".to_string()]);

        // The path exclusion is nested, and the nest names the primary.
        let nested = match filter.must_not[1]
            .condition_one_of
            .as_ref()
            .expect("condition set")
        {
            ConditionOneOf::Filter(f) => f,
            other => panic!(
                "path exclusion must be a NESTED Filter so it binds to the primary \
                 project only — got {other:?}"
            ),
        };
        assert_eq!(
            nested.must.len(),
            2,
            "the nest must carry BOTH project_id and file_path: dropping either \
             makes it exclude the paths from the overlay as well"
        );
        let (p_key, p_vals) = keywords(&nested.must[0]);
        assert_eq!(p_key, "project_id");
        assert_eq!(p_vals, vec!["main".to_string()]);
        let (f_key, f_vals) = keywords(&nested.must[1]);
        assert_eq!(f_key, "file_path");
        assert_eq!(f_vals, paths);
        assert!(
            nested.must_not.is_empty(),
            "the nest is a positive match on what to remove, not a double negation"
        );
    }

    /// The no-overlay shape must stay flat. An overlay-less query has one
    /// project, so nesting there would be pure overhead — and, more usefully,
    /// this pins that the `Some`/`None` arms did not get collapsed into one.
    #[test]
    fn build_query_filter_without_an_overlay_stays_a_flat_single_project_filter() {
        use qdrant_client::qdrant::{condition::ConditionOneOf, r#match::MatchValue};

        let filter = build_query_filter("main", None, &[], &["src/a.rs".to_string()]);
        assert_eq!(filter.must.len(), 1);
        assert_eq!(filter.must_not.len(), 1);
        match filter.must_not[0]
            .condition_one_of
            .as_ref()
            .expect("condition set")
        {
            ConditionOneOf::Field(f) => {
                assert_eq!(f.key, "file_path");
                assert!(matches!(
                    f.r#match.as_ref().and_then(|m| m.match_value.as_ref()),
                    Some(MatchValue::Keywords(_))
                ));
            }
            other => panic!("expected a flat field condition without an overlay, got {other:?}"),
        }
        match filter.must[0]
            .condition_one_of
            .as_ref()
            .expect("condition set")
        {
            ConditionOneOf::Field(f) => {
                assert_eq!(f.key, "project_id");
                assert!(
                    matches!(
                        f.r#match.as_ref().and_then(|m| m.match_value.as_ref()),
                        Some(MatchValue::Keyword(k)) if k == "main"
                    ),
                    "a single project must match one keyword, not a MatchAny pair"
                );
            }
            other => panic!("expected a field condition, got {other:?}"),
        }
    }

    /// Full E2E test — requires a running Qdrant instance (testcontainers).
    /// Run with: cargo test -- --ignored qdrant_creates_collection_with_dense_and_sparse
    #[tokio::test]
    #[ignore]
    async fn qdrant_creates_collection_with_dense_and_sparse() {
        let wrap = QdrantWrap::connect("http://localhost:6334")
            .await
            .expect("connect");

        let coll = "test_ensure_collection";

        // Clean up from any previous run.
        let _ = wrap.client.delete_collection(coll).await;

        assert!(
            !wrap.collection_exists(coll).await.unwrap(),
            "should not exist yet"
        );

        wrap.ensure_collection(coll, 384).await.expect("ensure");

        assert!(
            wrap.collection_exists(coll).await.unwrap(),
            "should exist after ensure"
        );

        // Idempotent — second call must not error.
        wrap.ensure_collection(coll, 384).await.expect("idempotent");

        // Cleanup.
        wrap.client.delete_collection(coll).await.unwrap();
    }

    /// Full E2E test — requires a running Qdrant instance. `hybrid_query`'s
    /// `must_not` clause has no coverage anywhere else in the suite (the
    /// contract tests in code_store.rs run only against `InMemoryCodeStore`),
    /// so this is what stands between a broken filter and a silent regression
    /// on the backend most worktree-search users run.
    /// Run with: cargo test --features server-stack -- --ignored qdrant_hybrid_query_excludes_paths
    #[tokio::test]
    #[ignore]
    async fn qdrant_hybrid_query_excludes_paths() {
        use crate::retrieval::embedder::{EmbedOutput, SparseVector};
        use crate::retrieval::payload::{payload_to_map, CodePayload};

        let wrap = QdrantWrap::connect("http://localhost:6334")
            .await
            .expect("connect");

        let coll = "test_hybrid_query_excludes_paths";

        // Clean up from any previous run.
        let _ = wrap.client.delete_collection(coll).await;
        wrap.ensure_collection(coll, 2).await.expect("ensure");

        let payload = |file: &str, chunk_id: &str| CodePayload {
            project_id: "proj".into(),
            file_path: file.into(),
            language: "rust".into(),
            start_line: 1,
            end_line: 2,
            ast_header: String::new(),
            content: format!("content of {chunk_id}"),
            content_hash: "h".into(),
            last_indexed_commit: String::new(),
            chunk_id: chunk_id.into(),
        };
        let embed = |dense: Vec<f32>| EmbedOutput {
            dense,
            sparse: SparseVector {
                indices: vec![],
                values: vec![],
            },
        };

        let points = vec![
            (
                "keep".to_string(),
                payload_to_map(&payload("src/keep.rs", "keep")),
                embed(vec![1.0, 0.0]),
            ),
            (
                "drop".to_string(),
                payload_to_map(&payload("src/drop.rs", "drop")),
                embed(vec![1.0, 0.0]),
            ),
        ];
        wrap.upsert_points(coll, &points).await.expect("upsert");

        let hits = wrap
            .hybrid_query(
                coll,
                "proj",
                None,
                &[1.0, 0.0],
                &SparseVector {
                    indices: vec![],
                    values: vec![],
                },
                10,
                3.0,
                true,
                &[],
                &["src/drop.rs".to_string()],
            )
            .await
            .expect("query");

        assert!(
            hits.iter().all(|h| h.file_path != "src/drop.rs"),
            "excluded path must not appear in results"
        );
        assert!(
            hits.iter().any(|h| h.file_path == "src/keep.rs"),
            "exclusion must not empty the result set — the accepting case needs pinning too"
        );

        // Cleanup.
        wrap.client.delete_collection(coll).await.unwrap();
    }

    /// The **hybrid/RRF arm** of `hybrid_query` — the DEFAULT production path.
    /// `RetrievalConfig::disable_sparse` is `false` unless `CODESCOUT_DISABLE_SPARSE`
    /// is set (`src/retrieval/config.rs`), so this is the branch nearly every real
    /// `semantic_search` call takes, and `qdrant_hybrid_query_excludes_paths` above
    /// covers only the `disable_sparse: true` branch.
    ///
    /// Measured before this test existed: deleting `.filter(...)` from EITHER
    /// prefetch leg left the entire 3701-test suite green. The exclusion has to
    /// hold on BOTH legs — the fused result is the union of the two prefetches,
    /// so one unfiltered leg is enough to leak an excluded path back into the
    /// page. `drop` is given the stronger sparse vector precisely so the sparse
    /// leg would retrieve it if its filter went missing.
    ///
    /// Run with: cargo test --features server-stack -- --ignored qdrant_hybrid_rrf
    #[tokio::test]
    #[ignore]
    async fn qdrant_hybrid_rrf_query_excludes_paths_on_both_prefetch_legs() {
        use crate::retrieval::embedder::{EmbedOutput, SparseVector};
        use crate::retrieval::payload::{payload_to_map, CodePayload};

        let wrap = QdrantWrap::connect("http://localhost:6334")
            .await
            .expect("connect");

        let coll = "test_hybrid_rrf_excludes_paths";

        // Clean up from any previous run.
        let _ = wrap.client.delete_collection(coll).await;
        wrap.ensure_collection(coll, 2).await.expect("ensure");

        let payload = |file: &str, chunk_id: &str| CodePayload {
            project_id: "proj".into(),
            file_path: file.into(),
            language: "rust".into(),
            start_line: 1,
            end_line: 2,
            ast_header: String::new(),
            content: format!("content of {chunk_id}"),
            content_hash: "h".into(),
            last_indexed_commit: String::new(),
            chunk_id: chunk_id.into(),
        };
        let embed = |dense: Vec<f32>, values: Vec<f32>| EmbedOutput {
            dense,
            sparse: SparseVector {
                indices: vec![1, 2],
                values,
            },
        };

        let points = vec![
            (
                "keep".to_string(),
                payload_to_map(&payload("src/keep.rs", "keep")),
                embed(vec![1.0, 0.0], vec![1.0, 1.0]),
            ),
            (
                // Ranks FIRST on the sparse leg (stronger term weights) and is
                // retrievable on the dense leg too, so a missing filter on
                // either prefetch surfaces it in the fused page.
                "drop".to_string(),
                payload_to_map(&payload("src/drop.rs", "drop")),
                embed(vec![0.9, 0.1], vec![5.0, 5.0]),
            ),
        ];
        wrap.upsert_points(coll, &points).await.expect("upsert");

        let hits = wrap
            .hybrid_query(
                coll,
                "proj",
                None,
                &[1.0, 0.0],
                &SparseVector {
                    indices: vec![1, 2],
                    values: vec![1.0, 1.0],
                },
                10,
                3.0,
                // The whole point of this test: the hybrid/RRF arm.
                false,
                &[],
                &["src/drop.rs".to_string()],
            )
            .await
            .expect("query");

        assert!(
            hits.iter().all(|h| h.file_path != "src/drop.rs"),
            "excluded path leaked through the RRF fusion — the exclusion filter is \
             missing from at least one prefetch leg. Got: {:?}",
            hits.iter().map(|h| &h.file_path).collect::<Vec<_>>()
        );
        assert!(
            hits.iter().any(|h| h.file_path == "src/keep.rs"),
            "exclusion must not empty the result set — the accepting case needs \
             pinning too, or an always-empty result would pass the assertion above"
        );

        // Cleanup.
        wrap.client.delete_collection(coll).await.unwrap();
    }

    /// C1, end to end against a live Qdrant: the worktree delta must earn its
    /// place on the page by relevance, not collect a fixed share of it.
    ///
    /// The fixture is deliberately lopsided — 12 relevant main chunks, plus a
    /// delta holding ONE relevant chunk and five that match neither the dense
    /// nor the sparse query at all. A relevance-ranked page owes the delta
    /// exactly one slot.
    ///
    /// The test asserts BOTH shapes, because the contrast is the evidence:
    ///
    /// - `two_query_merge` reproduces the shipped composition (two
    ///   `hybrid_query` calls fused by `merge_hits`). Qdrant's RRF score is
    ///   `1/(1 + rank)` — a function of rank position only — so the delta's
    ///   6-chunk ranking produces the same score ladder as main's 12-chunk one
    ///   and a stable score sort interleaves them. The delta takes roughly half
    ///   the page whatever it contains.
    /// - the union query ranks all 18 chunks once, and the five irrelevant
    ///   delta chunks sink below main's.
    ///
    /// Run with: cargo test --features server-stack -- --ignored qdrant_worktree_union
    #[tokio::test]
    #[ignore]
    async fn qdrant_worktree_union_ranks_the_delta_by_relevance_not_by_rank_position() {
        use crate::retrieval::embedder::{EmbedOutput, SparseVector};
        use crate::retrieval::payload::{payload_to_map, CodePayload};

        let wrap = QdrantWrap::connect("http://localhost:6334")
            .await
            .expect("connect");

        let coll = "test_worktree_union_ranking";
        const MAIN: &str = "m";
        const DELTA: &str = "m@wt";
        const DIRTY: &str = "src/dirty.rs";
        const LIMIT: usize = 12;

        let _ = wrap.client.delete_collection(coll).await;
        wrap.ensure_collection(coll, 2).await.expect("ensure");

        let payload = |project: &str, file: &str, chunk_id: &str| CodePayload {
            project_id: project.into(),
            file_path: file.into(),
            language: "rust".into(),
            start_line: 1,
            end_line: 2,
            ast_header: String::new(),
            content: format!("content of {chunk_id}"),
            content_hash: "h".into(),
            last_indexed_commit: String::new(),
            chunk_id: chunk_id.into(),
        };
        let embed = |dense: Vec<f32>, idx: Vec<u32>| EmbedOutput {
            dense,
            sparse: SparseVector {
                indices: idx,
                values: vec![1.0, 1.0],
            },
        };

        let mut points = Vec::new();
        // Main: 12 relevant chunks, all close to the query on both legs.
        for i in 0..12 {
            let id = format!("main-{i}");
            points.push((
                id.clone(),
                payload_to_map(&payload(MAIN, &format!("src/m{i}.rs"), &id)),
                embed(vec![1.0, 0.01 * i as f32], vec![1, 2]),
            ));
        }
        // Main's stale copy of the worktree-changed file. Maximally relevant,
        // so if the exclusion ever stops working it lands at the very top.
        points.push((
            "main-dirty".to_string(),
            payload_to_map(&payload(MAIN, DIRTY, "main-dirty")),
            embed(vec![1.0, 0.0], vec![1, 2]),
        ));
        // The delta's own copy of that file — genuinely relevant, must appear.
        points.push((
            "delta-dirty".to_string(),
            payload_to_map(&payload(DELTA, DIRTY, "delta-dirty")),
            embed(vec![1.0, 0.005], vec![1, 2]),
        ));
        // Five delta chunks that match nothing: orthogonal dense vector and
        // disjoint sparse terms. A relevance ranking owes them nothing.
        for i in 0..5 {
            let id = format!("delta-noise-{i}");
            points.push((
                id.clone(),
                payload_to_map(&payload(DELTA, &format!("src/noise{i}.rs"), &id)),
                embed(vec![0.0, 1.0], vec![90, 91]),
            ));
        }
        wrap.upsert_points(coll, &points).await.expect("upsert");

        let q_dense = [1.0f32, 0.0];
        let q_sparse = SparseVector {
            indices: vec![1, 2],
            values: vec![1.0, 1.0],
        };
        let excl = [DIRTY.to_string()];
        let is_delta = |h: &crate::retrieval::search::Hit| h.chunk_id.starts_with("delta-");

        // --- The shipped composition: two queries fused by score. ---
        let main_hits = wrap
            .hybrid_query(
                coll,
                MAIN,
                None,
                &q_dense,
                &q_sparse,
                LIMIT,
                3.0,
                false,
                &[],
                &excl,
            )
            .await
            .expect("main query");
        let delta_hits = wrap
            .hybrid_query(
                coll,
                DELTA,
                None,
                &q_dense,
                &q_sparse,
                LIMIT,
                3.0,
                false,
                &[],
                &[],
            )
            .await
            .expect("delta query");
        let merged = crate::retrieval::search::merge_hits(main_hits, delta_hits, LIMIT);
        let noise = |h: &crate::retrieval::search::Hit| h.chunk_id.starts_with("delta-noise");
        let delta_share_merged = merged.iter().filter(|h| is_delta(h)).count();
        let noise_merged = merged.iter().filter(|h| noise(h)).count();
        let main_share_merged = merged.len() - delta_share_merged;
        eprintln!(
            "two-query merge: delta {delta_share_merged}/{} (noise {noise_merged}) — {:?}",
            merged.len(),
            merged
                .iter()
                .map(|h| (h.chunk_id.clone(), h.score))
                .collect::<Vec<_>>()
        );
        // This arm documents C1 rather than guarding it: it is a measurement of
        // the composition the fix replaced. The threshold-free statement of the
        // defect is that chunks matching NEITHER leg of the query reach the
        // page at all -- they are there purely because rank 4 in a 6-chunk
        // corpus scores the same as rank 4 in a 500k one.
        assert!(
            noise_merged > 0,
            "expected the score merge to seat delta chunks that match nothing \
             (delta share {delta_share_merged}/{}). If this ever stops \
             reproducing, Qdrant's fusion scoring changed and the reasoning in \
             `merge_hits` and `query_overlay` needs re-deriving, not deleting.",
            merged.len()
        );

        // --- The fix: one ranking over the union. ---
        //
        // Deliberately through the TRAIT method, not `hybrid_query` directly.
        // `CodeVectorStore::query_overlay` has a default (two queries +
        // `merge_hits`) that `QdrantWrap` overrides; calling the inherent
        // method would bypass the override and test a path production never
        // takes, leaving "someone deletes the override" — i.e. C1 returning in
        // full — caught by nothing at all. `RetrievalClient::search_in` is the
        // only production caller and no test constructs a Qdrant-backed client,
        // so this call is the entire regression net for that override.
        use crate::retrieval::code_store::CodeVectorStore;
        let union = wrap
            .query_overlay(
                coll,
                MAIN,
                DELTA,
                &q_dense,
                &q_sparse,
                LIMIT,
                3.0,
                false,
                &[],
                &excl,
            )
            .await
            .expect("union query");
        let delta_share_union = union.iter().filter(|h| is_delta(h)).count();
        let noise_union = union.iter().filter(|h| noise(h)).count();
        let main_share_union = union.len() - delta_share_union;
        eprintln!(
            "union query:     delta {delta_share_union}/{} (noise {noise_union}) — {:?}",
            union.len(),
            union
                .iter()
                .map(|h| (h.chunk_id.clone(), h.score))
                .collect::<Vec<_>>()
        );

        assert_eq!(
            noise_union,
            0,
            "delta chunks matching neither leg must not reach the page: {:?}",
            union.iter().map(|h| &h.chunk_id).collect::<Vec<_>>()
        );
        assert_eq!(
            delta_share_union,
            1,
            "the delta holds exactly one relevant chunk, so it earns exactly one \
             slot — got {delta_share_union}. Page: {:?}",
            union.iter().map(|h| &h.chunk_id).collect::<Vec<_>>()
        );
        assert!(
            union.iter().any(|h| h.chunk_id == "delta-dirty"),
            "the delta's relevant chunk must still be served — the worktree's own \
             edits are the entire reason the delta exists. Page: {:?}",
            union.iter().map(|h| &h.chunk_id).collect::<Vec<_>>()
        );
        assert!(
            union.iter().all(|h| h.chunk_id != "main-dirty"),
            "main's stale copy of the changed path must be excluded — serving both \
             copies is the double-serve this design prevents. Page: {:?}",
            union.iter().map(|h| &h.chunk_id).collect::<Vec<_>>()
        );
        assert!(
            main_share_union > main_share_merged,
            "main's genuine results were being truncated to make room for the \
             delta: {main_share_merged}/{} slots under the score merge vs \
             {main_share_union}/{} under the union",
            merged.len(),
            union.len()
        );

        // Cleanup.
        wrap.client.delete_collection(coll).await.unwrap();
    }
}
