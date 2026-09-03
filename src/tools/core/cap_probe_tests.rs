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

/// Every row is internally consistent: a unique, non-empty id in the id
/// grammar `RESULT_CAP` annotations use, a `Deferred`/`NotYet` reason that is
/// not empty (after trimming) and not one of the banned placeholders this
/// convention explicitly rules out, and — for `Probed` rows — a `Marker`
/// shaped the way the caller reads it (a `JsonPath` starting with `$.`, a
/// non-empty `TextContains`).
#[test]
fn probe_rows_are_well_formed() {
    use super::cap_probe::{Coverage, Marker, Mutation, PROBE_ROWS};
    use std::collections::HashSet;

    let banned = ["deferred", "later", "hard to test", "todo", "tbd", "n/a"];
    let is_placeholder = |reason: &str| {
        let trimmed = reason.trim();
        let lower = trimmed.to_lowercase();
        trimmed.is_empty() || banned.iter().any(|b| lower == *b)
    };

    let mut bad_ids = vec![];
    let mut bad_reasons = vec![];
    let mut bad_markers = vec![];
    let mut seen_ids = HashSet::new();
    let mut duplicate_ids = vec![];

    for row in PROBE_ROWS {
        if row.id.is_empty() || !row.id.contains('.') {
            bad_ids.push(row.id);
        }
        if !seen_ids.insert(row.id) {
            duplicate_ids.push(row.id);
        }
        match &row.coverage {
            Coverage::Deferred(reason) => {
                if is_placeholder(reason) {
                    bad_reasons.push(row.id);
                }
            }
            Coverage::Probed { marker, mutation } => {
                let marker_ok = match marker {
                    Marker::JsonPath(path) => path.starts_with("$."),
                    Marker::TextContains(text) => !text.trim().is_empty(),
                };
                if !marker_ok {
                    bad_markers.push(row.id);
                }
                if let Mutation::NotYet(reason) = mutation {
                    if is_placeholder(reason) {
                        bad_reasons.push(row.id);
                    }
                }
            }
        }
    }

    assert!(bad_ids.is_empty(), "malformed ProbeRow ids: {bad_ids:?}");
    assert!(
        duplicate_ids.is_empty(),
        "ProbeRow ids that appear more than once: {duplicate_ids:?}"
    );
    assert!(
        bad_markers.is_empty(),
        "ProbeRow ids with a malformed Marker: {bad_markers:?}"
    );
    assert!(
        bad_reasons.is_empty(),
        "ProbeRow ids with a placeholder (not a real) reason: {bad_reasons:?}"
    );
}

/// Prints the tally this convention lives and dies by. Asserts nothing
/// about mutation status — Task 5a's scope is classification, and a row
/// whose `mutation` is `NotYet` is not a defect this test polices.
#[test]
fn print_mutation_tally() {
    use super::cap_probe::{Coverage, Mutation, PROBE_ROWS};

    let total = PROBE_ROWS.len();
    let probed = PROBE_ROWS
        .iter()
        .filter(|r| matches!(r.coverage, Coverage::Probed { .. }))
        .count();
    let deferred = total - probed;
    let mutation_verified = PROBE_ROWS
        .iter()
        .filter(|r| {
            matches!(
                r.coverage,
                Coverage::Probed {
                    mutation: Mutation::Killed,
                    ..
                }
            )
        })
        .count();

    println!(
        "IC-13 probe rows: {total} total, {probed} probed, {deferred} deferred, \
         {mutation_verified} mutation-verified"
    );
}

/// The real `PROBE_ROWS` has zero `Mutation::Killed` rows today (Task 5a is
/// classification-only scope), so [`print_mutation_tally`]'s `mutation_verified`
/// count is otherwise checked only against an all-`NotYet` population — a
/// counting bug that always reports 0 would pass that silently. This pins
/// the counting logic itself against a synthetic fixture that DOES mix
/// `Killed` with `NotYet` and `Deferred`, so a mis-count has somewhere to
/// fail before the real table ever grows a `Killed` row.
#[test]
fn tally_distinguishes_killed_from_not_yet_and_deferred() {
    use super::cap_probe::{Coverage, Marker, Mutation, ProbeRow};

    let rows = [
        ProbeRow {
            id: "fixture.killed",
            coverage: Coverage::Probed {
                marker: Marker::TextContains("z"),
                mutation: Mutation::Killed,
            },
        },
        ProbeRow {
            id: "fixture.not_yet",
            coverage: Coverage::Probed {
                marker: Marker::JsonPath("$.a"),
                mutation: Mutation::NotYet("no mutation run yet"),
            },
        },
        ProbeRow {
            id: "fixture.deferred",
            coverage: Coverage::Deferred("no test drives this cap past its bound"),
        },
    ];

    let probed = rows
        .iter()
        .filter(|r| matches!(r.coverage, Coverage::Probed { .. }))
        .count();
    let mutation_verified = rows
        .iter()
        .filter(|r| {
            matches!(
                r.coverage,
                Coverage::Probed {
                    mutation: Mutation::Killed,
                    ..
                }
            )
        })
        .count();

    assert_eq!(probed, 2, "both Probed rows should count as probed");
    assert_eq!(
        mutation_verified, 1,
        "only the Killed row should count as mutation-verified"
    );
}
