use anyhow::{Context, Result};
use qdrant_client::qdrant::{Distance, Modifier};
use qdrant_client::qdrant::{
    CreateCollectionBuilder, SparseVectorParamsBuilder, SparseVectorsConfigBuilder,
    VectorParamsBuilder, VectorsConfigBuilder,
};
use qdrant_client::Qdrant;

pub struct QdrantWrap {
    pub client: Qdrant,
}

impl QdrantWrap {
    pub async fn connect(url: &str) -> Result<Self> {
        let client = Qdrant::from_url(url).build().context("qdrant connect")?;
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
        vectors.add_named_vector_params(
            "dense",
            VectorParamsBuilder::new(dim, Distance::Cosine),
        );

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

    /// Scroll all chunk refs for a project. Implemented in Task 3.4.
    pub async fn scroll_chunk_refs(
        &self,
        _collection: &str,
        _project_id: &str,
    ) -> Result<Vec<crate::retrieval::drift::ChunkRef>> {
        // TODO(3.4): implement scroll with filter on project_id payload field
        Ok(vec![])
    }

    /// Upsert points with dense+sparse vectors and payload. Implemented in Task 3.4.
    pub async fn upsert_points(
        &self,
        _collection: &str,
        _points: &[(
            String,
            std::collections::HashMap<String, qdrant_client::qdrant::Value>,
            crate::retrieval::embedder::EmbedOutput,
        )],
    ) -> Result<()> {
        // TODO(3.4): implement batch upsert
        Ok(())
    }

    /// Delete points by chunk_id. Implemented in Task 3.4.
    pub async fn delete_points(&self, _collection: &str, _ids: &[String]) -> Result<()> {
        // TODO(3.4): implement delete by payload filter
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
