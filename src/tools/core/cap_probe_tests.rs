//! Behavioural probe rows for `IC-13`.
//!
//! Each row drives a real tool PAST ITS OWN CAP through the same
//! `call_content` path an agent uses, and asserts a truncation marker
//! ARRIVES. Not that it is correct — `IC-13`'s clause deliberately excludes
//! a visible-but-wrong marker, whose true total is sometimes unknowable
//! (`grep`'s old `Showing N of N`). Arrival is the property.
#![cfg(test)]

use crate::server::test_support::{call_tool_checked, make_server, shared_ctx};

/// Proves the lifted driver is reachable from here at all.
///
/// Task 4's whole deliverable is that reachability: a second copy of
/// `call_tool_checked` would be a second place to get the
/// `RecoverableError`-routes-to-success subtlety wrong, and that mistake
/// makes a BAD ROW look like a finding.
#[tokio::test]
async fn the_lifted_driver_reaches_a_real_tool_from_this_module() {
    let (_dir, server) = make_server().await;
    let out = call_tool_checked(
        &server,
        "grep",
        serde_json::json!({"pattern": "fn "}),
        "smoke",
    )
    .await;
    let primary = out[0].as_text().expect("primary block is text");
    // Not a JSON-shape assertion: `Grep` declares `OutputForm::Text`, so its
    // `call_content` output is ripgrep-style plain text even for a trivial
    // zero-match result (`format_grep`'s `"0 matches"` — the fresh `make_server`
    // temp dir has no source files for "fn " to match). Parsing that as JSON
    // panics on trailing characters. The test's only claim is reachability, so
    // assert the driver produced *some* real content, not a specific shape.
    assert!(
        !primary.text.is_empty(),
        "a real tool response arrived through the lifted driver"
    );
    let _ = shared_ctx(&server);
}
