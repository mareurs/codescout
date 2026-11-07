use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use anyhow::Result;
use codescout::librarian::{
    artifact_store::ArtifactVectorStore,
    catalog::Catalog,
    classify::load_rules,
    embedding::EmbeddingService,
    tools::{reindex, TempGuardEnv, ToolContext},
    workspace::{Root, WorkspaceConfig},
};
use codescout_embed::Embedder;
use parking_lot::Mutex;
use serde_json::json;
use tempfile::TempDir;

const ARTIFACT_COUNT: usize = 70;

/// An embedder that records how many `embed` calls overlap. Each call yields to the scheduler a
/// number of times that shrinks with the artifact's index, so a later artifact can finish before
/// an earlier one: completion order differs from submission order, and a result filed under the
/// wrong chunk shows up as a wrong vector.
#[derive(Default)]
struct ConcurrentEmbedder {
    active: AtomicUsize,
    peak: AtomicUsize,
    calls: AtomicUsize,
}

#[async_trait::async_trait]
impl Embedder for ConcurrentEmbedder {
    fn dimensions(&self) -> usize {
        4
    }

    async fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
        self.peak.fetch_max(active, Ordering::SeqCst);
        let index = (0..ARTIFACT_COUNT)
            .find(|index| texts[0].contains(&format!("spec-{index:02}.md")))
            .unwrap();
        for _ in 0..(ARTIFACT_COUNT - index) {
            tokio::task::yield_now().await;
        }
        self.active.fetch_sub(1, Ordering::SeqCst);
        if index == 3 {
            anyhow::bail!("one remote request failed");
        }
        Ok(vec![vec![1.0, index as f32, 0.0, 0.0]])
    }
}

#[derive(Default)]
struct RecordingStore {
    vectors: Mutex<HashMap<String, Vec<f32>>>,
}

#[async_trait::async_trait]
impl ArtifactVectorStore for RecordingStore {
    async fn upsert(
        &self,
        _project_id: &str,
        chunk_id: &str,
        _artifact_id: &str,
        vector: &[f32],
    ) -> Result<()> {
        assert!(self
            .vectors
            .lock()
            .insert(chunk_id.to_string(), vector.to_vec())
            .is_none());
        Ok(())
    }

    async fn delete(&self, _artifact_id: &str) -> Result<()> {
        Ok(())
    }

    async fn refile(&self, _old_artifact_id: &str, _new_artifact_id: &str) -> Result<u64> {
        Ok(0)
    }

    async fn knn(
        &self,
        _project_id: Option<&str>,
        _query: &[f32],
        _count: usize,
    ) -> Result<Vec<(String, f32)>> {
        Ok(Vec::new())
    }
}

#[tokio::test]
async fn a_service_set_to_eight_runs_eight_requests_and_keeps_chunk_identity() {
    assert_reindex_concurrency(Some(8), 8).await;
}

#[tokio::test]
async fn the_default_service_keeps_one_request_in_flight() {
    // Load-bearing: this service is built WITHOUT `with_concurrency`, which is the state every
    // existing caller is in. It is what proves the default is the sequential loop, not merely
    // that `with_concurrency(1)` is.
    assert_reindex_concurrency(None, 1).await;
}

#[tokio::test]
async fn a_service_set_to_zero_runs_one_request_in_flight() {
    assert_reindex_concurrency(Some(0), 1).await;
}

async fn assert_reindex_concurrency(set_to: Option<usize>, expected_peak: usize) {
    let temp = TempDir::new().unwrap();
    let root = std::fs::canonicalize(temp.path()).unwrap();
    std::fs::create_dir_all(root.join("docs/specs")).unwrap();
    for index in 0..ARTIFACT_COUNT {
        let name = format!("spec-{index:02}.md");
        std::fs::write(
            root.join("docs/specs").join(&name),
            format!("# {name}\nbody\n"),
        )
        .unwrap();
    }
    let embedder = Arc::new(ConcurrentEmbedder::default());
    let store = Arc::new(RecordingStore::default());
    let service = EmbeddingService::new(embedder.clone());
    let service = match set_to {
        Some(n) => service.with_concurrency(n),
        None => service,
    };
    let ctx = ToolContext {
        catalog: Arc::new(Mutex::new(Catalog::open_in_memory().unwrap())),
        workspace: Arc::new(WorkspaceConfig {
            roots: vec![Root {
                name: "r".into(),
                path: root,
            }],
            ignore: vec![],
            rules: vec![],
            umbrellas: vec![],
        }),
        rules: Arc::new(
            load_rules("[[rule]]\nglob = \"**/docs/specs/*.md\"\nkind = \"spec\"\n").unwrap(),
        ),
        embedding: Some(Arc::new(service)),
        artifact_store: Some(store.clone()),
        current_project: None,
        lsp: codescout::lsp::MockLspProvider::with_client(codescout::lsp::MockLspClient::default()),
        temp_guard: TempGuardEnv::from_env(),
        progress: None,
    };

    let result = reindex::call(&ctx, json!({})).await.unwrap();

    assert_eq!(embedder.peak.load(Ordering::SeqCst), expected_peak);
    assert_eq!(embedder.active.load(Ordering::SeqCst), 0);
    assert_eq!(embedder.calls.load(Ordering::SeqCst), 70);
    assert_eq!(result["embedded"], 69);
    assert_eq!(result["embed_error_count"], 1);
    assert_eq!(store.vectors.lock().len(), 69);
    let catalog = ctx.catalog.lock();
    let mut statement = catalog
        .conn
        .prepare(
            "SELECT c.chunk_id, a.title, c.embedded_input_hash, a.embedded_sha256
             FROM artifact_chunk c JOIN artifact a ON a.id = c.artifact_id",
        )
        .unwrap();
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        })
        .unwrap();
    let vectors = store.vectors.lock();
    let mut checked = 0;
    for row in rows {
        let (chunk_id, title, marker, stamp) = row.unwrap();
        let index: usize = title
            .strip_prefix("spec-")
            .unwrap()
            .strip_suffix(".md")
            .unwrap()
            .parse()
            .unwrap();
        if index == 3 {
            assert!(!vectors.contains_key(&chunk_id));
            assert!(marker.is_none());
            assert!(stamp.is_none());
        } else {
            assert_eq!(vectors[&chunk_id], vec![1.0, index as f32, 0.0, 0.0]);
            assert!(marker.is_some());
            assert!(stamp.is_some());
        }
        checked += 1;
    }
    assert_eq!(checked, 70);
}
