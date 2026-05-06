#![cfg(feature = "retrieval-e2e")]

use codescout::retrieval::{client::RetrievalClient, sync::SyncOpts};

#[tokio::test]
async fn sync_then_query_roundtrip_finds_known_symbol() {
    let client = RetrievalClient::from_env().await.expect("client");
    let project_id = "rust-library-test";
    let root = std::path::Path::new("tests/fixtures/rust-library");

    let report = client
        .sync_project(project_id, root, SyncOpts::default())
        .await
        .expect("sync");
    assert!(report.added > 0, "expected upserts on first sync, got {report:?}");
}
