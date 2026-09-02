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
    // What this actually proves, and what it does not:
    //
    // - Reachability of `test_support` from `src/tools/core` is a COMPILE-time
    //   fact, not a runtime one — if `test_support` or any of these three
    //   names were not `pub(crate)`, this file would not build. The runtime
    //   assertion below is near-inert on top of that: the fresh `make_server`
    //   temp dir has no source files, so "fn " matches zero, and `format_grep`
    //   renders that as the literal text "0 matches" — any non-empty string
    //   satisfies `!primary.text.is_empty()`.
    // - It does NOT exercise `call_tool_checked`'s `RecoverableError` check
    //   (isError:false + body {"ok": false}): "0 matches" fails
    //   `serde_json::from_str::<Value>`, so that branch inside
    //   `call_tool_checked` is skipped and only the `is_error` half runs.
    //   A future probe row that actually drives a tool past its cap is what
    //   exercises that path; this smoke test does not.
    let (_dir, server) = make_server().await;
    let out = call_tool_checked(
        &server,
        "grep",
        serde_json::json!({"pattern": "fn "}),
        "smoke",
    )
    .await;
    let primary = out
        .first()
        .expect("call_tool_checked returned no content blocks at all")
        .as_text()
        .expect("primary block is text");
    assert!(
        !primary.text.is_empty(),
        "a real tool response arrived through the lifted driver"
    );
    let _ = shared_ctx(&server);
}
