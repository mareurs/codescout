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

/// Returns the id of every `Probed` row whose `cited_test` is empty (after
/// trimming). Split out from `probe_rows_are_well_formed`'s main loop so an
/// empty citation gets its OWN failure message rather than being lumped into
/// "placeholder reason" — a `Probed` row's `cited_test` is not a `Deferred`
/// or `NotYet` reason at all, and a reader hitting that message would look
/// in the wrong place (the `banned` placeholder-word list) for a defect
/// that is really "nobody named a test yet".
fn missing_cited_tests(rows: &[super::cap_probe::ProbeRow]) -> Vec<&'static str> {
    use super::cap_probe::Coverage;

    rows.iter()
        .filter_map(|row| match &row.coverage {
            Coverage::Probed { cited_test, .. } if cited_test.trim().is_empty() => Some(row.id),
            _ => None,
        })
        .collect()
}

/// Every row is internally consistent: a unique, non-empty id in the id
/// grammar `RESULT_CAP` annotations use, a `Deferred`/`NotYet` reason that is
/// not empty (after trimming) and not one of the banned placeholders this
/// convention explicitly rules out, a `Marker` shaped the way the caller
/// reads it (a `JsonPath` starting with `$.`, a non-empty `TextContains`) for
/// `Probed` rows, and — also for `Probed` rows, checked separately by
/// [`missing_cited_tests`] — a non-empty `cited_test`:
/// `probed_rows_cite_a_real_test` (in `tests/result_caps.rs`) can only check
/// that a NAMED test holds up, so a row naming none would need catching
/// here instead.
#[test]
fn probe_rows_are_well_formed() {
    use super::cap_probe::{Coverage, Marker, Mutation, PROBE_ROWS};
    use std::collections::HashSet;

    assert!(
        !PROBE_ROWS.is_empty(),
        "PROBE_ROWS must not be empty — an empty table would pass every check below vacuously"
    );

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
            Coverage::Probed {
                marker, mutation, ..
            } => {
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
    let bad_cited_tests = missing_cited_tests(PROBE_ROWS);

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
    assert!(
        bad_cited_tests.is_empty(),
        "Probed ProbeRow ids with an empty cited_test — probed_rows_cite_a_real_test cannot \
         check a citation that names no test at all: {bad_cited_tests:?}"
    );
}

/// Negative-direction check for [`missing_cited_tests`]: a `Probed` row with
/// an empty `cited_test` must be reported, a `Probed` row with a non-empty
/// one must not, and a `Deferred` row (no `cited_test` field to check at
/// all) must not either. Without this, `missing_cited_tests` could report
/// nothing on every run just because the real `PROBE_ROWS` table happens to
/// have no empty `cited_test` today — this fixture forces the RED case to
/// exist somewhere, independent of the live table's current contents.
#[test]
fn missing_cited_tests_flags_only_probed_rows_with_an_empty_cited_test() {
    use super::cap_probe::{Coverage, Marker, Mutation, ProbeRow};

    let rows = [
        ProbeRow {
            id: "fixture.empty",
            coverage: Coverage::Probed {
                marker: Marker::TextContains("z"),
                mutation: Mutation::NotYet("no mutation run yet"),
                cited_test: "",
            },
        },
        ProbeRow {
            id: "fixture.named",
            coverage: Coverage::Probed {
                marker: Marker::TextContains("z"),
                mutation: Mutation::NotYet("no mutation run yet"),
                cited_test: "some_real_test",
            },
        },
        ProbeRow {
            id: "fixture.deferred",
            coverage: Coverage::Deferred("no test drives this cap past its bound"),
        },
    ];

    assert_eq!(missing_cited_tests(&rows), vec!["fixture.empty"]);
}

/// Prints the tally this convention lives and dies by. Asserts nothing
/// about mutation status — Task 5a's scope is classification, and a row
/// whose `mutation` is `NotYet` is not a defect this test polices.
#[test]
fn print_mutation_tally() {
    use super::cap_probe::{tally, PROBE_ROWS};

    let t = tally(PROBE_ROWS);

    println!(
        "IC-13 probe rows: {} total, {} probed, {} deferred, {} mutation-verified",
        t.total, t.probed, t.deferred, t.mutation_verified
    );
}

/// The real `PROBE_ROWS` carried zero `Mutation::Killed` rows until the Task 6
/// sweep of 2026-09-03; it now carries 17 of 18 `Probed` rows killed, so
/// [`print_mutation_tally`]'s `mutation_verified` count is no longer checked
/// only against an all-`NotYet` population. This fixture stays, and stays
/// load-bearing, for a reason that outlives that: it is the ONLY mixed
/// `Killed`/`NotYet`/`Deferred` population **independent of the real table**, so
/// a future edit that flips the last `NotYet` row — or that empties the table —
/// cannot silently take the mix away and leave a counting bug that always
/// reports 0 (or always reports `probed`) with nowhere to fail. Do not fold it
/// into an assertion over `PROBE_ROWS`: that would make the detector a function
/// of the data it is meant to police. It pins `cap_probe::tally` — the SAME
/// function `print_mutation_tally` calls.
#[test]
fn tally_distinguishes_killed_from_not_yet_and_deferred() {
    use super::cap_probe::{tally, Coverage, Marker, Mutation, ProbeRow};

    let rows = [
        ProbeRow {
            id: "fixture.killed",
            coverage: Coverage::Probed {
                marker: Marker::TextContains("z"),
                mutation: Mutation::Killed,
                cited_test: "fixture_test_that_kills_the_marker",
            },
        },
        ProbeRow {
            id: "fixture.not_yet",
            coverage: Coverage::Probed {
                marker: Marker::JsonPath("$.a"),
                mutation: Mutation::NotYet("no mutation run yet"),
                cited_test: "fixture_test_that_asserts_the_marker",
            },
        },
        ProbeRow {
            id: "fixture.deferred",
            coverage: Coverage::Deferred("no test drives this cap past its bound"),
        },
    ];

    let t = tally(&rows);

    assert_eq!(t.probed, 2, "both Probed rows should count as probed");
    assert_eq!(
        t.mutation_verified, 1,
        "only the Killed row should count as mutation-verified"
    );
}
