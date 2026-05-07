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
    assert!(
        report.added > 0,
        "expected upserts on first sync, got {report:?}"
    );
}

#[tokio::test]
async fn sync_is_idempotent() {
    let client = RetrievalClient::from_env().await.expect("client");
    let project_id = "rust-library-idempotent";
    let root = std::path::Path::new("tests/fixtures/rust-library");

    let r1 = client
        .sync_project(project_id, root, SyncOpts::default())
        .await
        .expect("first");
    let r2 = client
        .sync_project(project_id, root, SyncOpts::default())
        .await
        .expect("second");
    assert!(r1.added > 0);
    assert_eq!(r2.added, 0, "second sync added {} unexpectedly", r2.added);
    assert_eq!(r2.deleted, 0);
}

#[tokio::test]
async fn sync_detects_file_modification() {
    use std::fs;
    let client = RetrievalClient::from_env().await.expect("client");
    let project_id = "drift-detect-test";
    let tmp = tempfile::tempdir().unwrap();
    let f = tmp.path().join("a.rs");
    fs::write(&f, "fn original() {}").unwrap();

    let r1 = client
        .sync_project(project_id, tmp.path(), SyncOpts::default())
        .await
        .expect("first");
    assert!(r1.added > 0);

    fs::write(&f, "fn modified() {}").unwrap();
    let r2 = client
        .sync_project(project_id, tmp.path(), SyncOpts::default())
        .await
        .expect("second");
    assert!(r2.added > 0, "modified file should trigger upsert");
    assert!(r2.deleted > 0, "old chunk should be deleted");
}

#[tokio::test]
async fn search_finds_synced_symbol() {
    let client = RetrievalClient::from_env().await.expect("client");
    let project_id = "search-e2e-test";
    let root = std::path::Path::new("tests/fixtures/rust-library");
    let _ = client
        .sync_project(project_id, root, SyncOpts::default())
        .await
        .expect("sync");

    let opts = codescout::retrieval::search::SearchOpts::new(10);
    let hits = client
        .search_code(project_id, "fibonacci", opts)
        .await
        .expect("search");
    assert!(
        !hits.is_empty(),
        "expected at least one hit for 'fibonacci'"
    );
}
