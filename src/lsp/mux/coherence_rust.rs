//! Rust-specific mux coherence test.
//!
//! Two Agents share one mux. A writes a new function; B must see it.
//! The bug being regression-tested: before mux, B's direct rust-analyzer
//! still saw the pre-write file because A's didChange only went to A's LSP.

use crate::lsp::manager::LspManager;

#[tokio::test]
#[ignore = "requires rust-analyzer on PATH; gated by CI job"]
async fn two_agents_coherent_after_edit() {
    let fixture =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/lsp-mux/rust");
    let (_td, root) = super::test_support::stage_fixture(&fixture);

    // Each agent has its own LspManager, but both point at the same workspace.
    // With mux=true, the first `get_or_start` spawns the mux process and both
    // connect to the same underlying rust-analyzer instance via a Unix socket.
    let mgr_a = LspManager::new();
    let mgr_b = LspManager::new();

    let ca = mgr_a
        .get_or_start("rust", &root, Some(true))
        .await
        .expect("A get_or_start");
    let cb = mgr_b
        .get_or_start("rust", &root, Some(true))
        .await
        .expect("B get_or_start");

    // 1. Both clients open the target file so the LSP tracks it.
    let target = root.join("src/lib.rs");
    ca.did_change(&target).await.expect("A did_change initial");
    cb.did_change(&target).await.expect("B did_change initial");

    // 2. Baseline: B sees original_symbol only (fresh_symbol does not exist yet).
    let syms_before = cb
        .document_symbols(&target, "rust")
        .await
        .expect("B document_symbols before");
    let names_before: Vec<_> = syms_before.iter().map(|s| s.name.clone()).collect();
    assert!(
        !names_before.iter().any(|n| n == "fresh_symbol"),
        "fresh_symbol should not exist yet: {:?}",
        names_before
    );

    // 3. A writes a new symbol to disk.
    let updated = "pub fn original_symbol() -> &'static str { \"original\" }\n\
                   pub fn fresh_symbol() -> &'static str { \"fresh\" }\n";
    std::fs::write(&target, updated).unwrap();

    // 4. No notification has been sent. B's own request must already show fresh_symbol:
    //    `document_symbols` re-sends the files that changed on disk since the client last
    //    sent them. This step used to assert the opposite, that B stays stale until A
    //    notifies; that was the defect fixed in 43c4ca9df (bug a73c49456056fb92), and the
    //    test was ignored, so it went red unseen.
    //
    //    NOT a mux test: in a test runner `LspManager::get_or_start` cannot spawn the mux
    //    (the test binary has no `mux` subcommand) and falls back to a direct server, so
    //    A and B here are two separate rust-analyzer processes. Measured 2026-11-12: the
    //    mux socket for this workspace did not exist while the test ran. Disabling the
    //    resync in `LspClient::request` leaves this step green, because `document_symbols`
    //    resyncs on its own. The mux path is covered by
    //    `lsp::client::tests::hover_follows_a_write_made_outside_codescout_through_the_mux`.
    let syms_unnotified = cb
        .document_symbols(&target, "rust")
        .await
        .expect("B document_symbols unnotified");
    let names_unnotified: Vec<_> = syms_unnotified.iter().map(|s| s.name.clone()).collect();
    assert!(
        names_unnotified.iter().any(|n| n == "fresh_symbol"),
        "B must see fresh_symbol without any notification: {:?}",
        names_unnotified
    );

    // 5. A notification afterwards must leave B coherent too.
    //    A's didChange reaches the shared rust-analyzer, and B's documentSymbol
    //    request reflects the updated index.
    mgr_a.notify_file_changed(&target).await;

    let syms_after = cb
        .document_symbols(&target, "rust")
        .await
        .expect("B document_symbols after");
    let names_after: Vec<_> = syms_after.iter().map(|s| s.name.clone()).collect();
    assert!(
        names_after.iter().any(|n| n == "fresh_symbol"),
        "Agent B's view stale after notification: {:?}",
        names_after
    );

    mgr_a.shutdown_all().await;
    mgr_b.shutdown_all().await;
}
