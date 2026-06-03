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

pub struct QdrantWrap {
    pub client: Qdrant,
}
/// Qdrant point IDs must be u64 or UUID — hash the chunk_id string to u64.
fn chunk_id_to_point_id(s: &str) -> u64 {
    use sha2::Digest;
    let hash = sha2::Sha256::digest(s.as_bytes());
    u64::from_le_bytes(hash[..8].try_into().unwrap())
}

impl QdrantWrap {
    pub async fn connect(url: &str) -> Result<Self> {
        let client = Qdrant::from_url(url)
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .context("qdrant connect")?;
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
                .with_payload(true)
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
                if !chunk_id.is_empty() {
                    refs.push(crate::retrieval::drift::ChunkRef {
                        chunk_id,
                        content_hash,
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
                .with_payload(true)
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
    /// field (empty = no filter). Used for `semantic_search(mode="code")`.
    #[allow(clippy::too_many_arguments)]
    pub async fn hybrid_query(
        &self,
        collection: &str,
        project_id: &str,
        dense: &[f32],
        sparse: &crate::retrieval::embedder::SparseVector,
        limit: usize,
        bm25_boost: f32,
        disable_sparse: bool,
        exclude_languages: &[String],
    ) -> Result<Vec<crate::retrieval::search::Hit>> {
        let must = vec![Condition::matches("project_id", project_id.to_string())];
        let must_not: Vec<Condition> = exclude_languages
            .iter()
            .map(|l| Condition::matches("language", l.clone()))
            .collect();
        let filter = Filter {
            must,
            must_not,
            ..Default::default()
        };

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
}
