//! Rust-specific mux coherence test.
//!
//! Two Agents share one mux. A writes a new function; B must see it.
//! The bug being regression-tested: before mux, B's direct rust-analyzer
//! still saw the pre-write file because A's didChange only went to A's LSP.

use super::test_support::two_agents_on_fixture;
use crate::lsp::manager::LspManager;
use crate::lsp::ops::LspClientOps as _;

#[tokio::test]
#[ignore = "requires rust-analyzer on PATH; gated by CI job"]
async fn two_agents_coherent_after_edit() {
    let (_a, _b, root, _td) = two_agents_on_fixture("rust").await;

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

    // 2. A writes a new symbol to disk and notifies the shared LSP.
    let updated = "pub fn original_symbol() -> &'static str { \"original\" }\n\
                   pub fn fresh_symbol() -> &'static str { \"fresh\" }\n";
    std::fs::write(&target, updated).unwrap();
    mgr_a.notify_file_changed(&target).await;

    // 3. B queries document symbols — must see `fresh_symbol`.
    //    Because both managers connect through the same mux socket, A's
    //    didChange notification reaches the shared rust-analyzer, and B's
    //    documentSymbol request reflects the updated index.
    let syms = cb
        .document_symbols(&target, "rust")
        .await
        .expect("B document_symbols");
    let names: Vec<_> = syms.iter().map(|s| s.name.clone()).collect();
    assert!(
        names.iter().any(|n| n == "fresh_symbol"),
        "Agent B's view is stale — fresh_symbol missing: {:?}",
        names
    );

    mgr_a.shutdown_all().await;
    mgr_b.shutdown_all().await;
}
