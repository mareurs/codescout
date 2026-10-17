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
/// Returns the id of every `NoMarker` row whose `bug` does not name an existing issue
/// file: empty, not a `docs/issues/<name>.md` path, or absent under `root`. A `NoMarker`
/// row asserts "this is a production defect", and the assertion is only as good as the
/// filing behind it — without this check the variant would be a second place to park
/// an unescalated finding, one with a more alarming name.
fn bugs_without_a_file(
    rows: &[super::cap_probe::ProbeRow],
    root: &std::path::Path,
) -> Vec<&'static str> {
    use super::cap_probe::Coverage;

    rows.iter()
        .filter_map(|row| match &row.coverage {
            Coverage::NoMarker { bug, .. } => {
                let bug = bug.trim();
                let in_issues = bug.starts_with("docs/issues/") && bug.ends_with(".md");
                (!in_issues || !root.join(bug).is_file()).then_some(row.id)
            }
            _ => None,
        })
        .collect()
}

/// Phrases that say, in a reason's own words, that the cap discloses nothing.
/// Lowercase; matched as substrings of the lowercased reason.
const NO_MARKER_PHRASES: &[&str] = &[
    "no marker",
    "emits no",
    "appends no",
    "silently",
    "no companion field",
    "no field reports",
    "no field discloses",
    "nothing marking",
    "indistinguishable from",
];

/// Returns the id of every `Untested` row whose reason describes a cap that discloses
/// nothing. Such a row is mis-filed: `Untested` is a worklist item ("write the test"),
/// and a cap with no marker has nothing a test could assert on — it is `IC-13`
/// membership and belongs in `NoMarker`. This is a text scan, so it is a backstop for the
/// next row's author reading their own reason, not a proof that no other row discloses
/// nothing: it cannot see a use site the reason never mentions.
fn untested_rows_that_describe_a_missing_marker(
    rows: &[super::cap_probe::ProbeRow],
) -> Vec<&'static str> {
    use super::cap_probe::Coverage;

    rows.iter()
        .filter_map(|row| match &row.coverage {
            Coverage::Untested(reason) => {
                let lower = reason.to_lowercase();
                NO_MARKER_PHRASES
                    .iter()
                    .any(|p| lower.contains(p))
                    .then_some(row.id)
            }
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
    let bad_bugs =
        bugs_without_a_file(PROBE_ROWS, std::path::Path::new(env!("CARGO_MANIFEST_DIR")));
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
            Coverage::Untested(reason) => {
                if is_placeholder(reason) {
                    bad_reasons.push(row.id);
                }
            }
            Coverage::NoMarker { reason, .. } => {
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
    assert!(
        bad_bugs.is_empty(),
        "NoMarker ProbeRow ids whose `bug` is not an existing docs/issues/ file — a row is a \
         production defect only once the defect has been filed, and an unfiled one is the \
         silent backlog item this variant exists to prevent: {bad_bugs:?}"
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
            coverage: Coverage::Untested("no test drives this cap past its bound"),
        },
    ];

    assert_eq!(missing_cited_tests(&rows), vec!["fixture.empty"]);
}

/// No `Untested` row may describe, in its own reason, a cap that discloses nothing.
///
/// This is the regression for `docs/issues/archive/2026-09-03-deferred-conflates-untested-with-no-marker-exists.md`:
/// the old single `Deferred` variant held 43 "write a test" rows beside 5 (later 8, now 9) rows
/// whose reason said no marker exists, and three of those five stated `IC-13`'s defining
/// property in their own text while sitting in the backlog. The assertion reads the REAL
/// table so a new row filed under the wrong variant reds here, in the author's own diff.
#[test]
fn no_untested_row_describes_a_cap_that_discloses_nothing() {
    use super::cap_probe::PROBE_ROWS;

    let misfiled = untested_rows_that_describe_a_missing_marker(PROBE_ROWS);
    assert!(
        misfiled.is_empty(),
        "Untested rows whose own reason says the cap discloses nothing — that is not a missing \
         test, it is a production defect (IC-13). File it under docs/issues/ and use \
         Coverage::NoMarker {{ reason, bug }}: {misfiled:?}"
    );
}

/// The detector and the bug-file check each need a RED case independent of the live table:
/// with every misfiled row fixed, `no_untested_row_describes_a_cap_that_discloses_nothing`
/// passes on the table alone whether or not the scan works.
#[test]
fn the_classification_guards_flag_what_they_exist_to_flag() {
    use super::cap_probe::{Coverage, ProbeRow};

    let rows = [
        ProbeRow {
            id: "fixture.silent",
            coverage: Coverage::Untested("the helper emits no marker when the cap fires"),
        },
        ProbeRow {
            id: "fixture.honest",
            coverage: Coverage::Untested("no test seeds more items than the cap"),
        },
        ProbeRow {
            id: "fixture.filed",
            coverage: Coverage::NoMarker {
                reason: "drops rows with no count",
                bug: "docs/issues/_TEMPLATE.md",
            },
        },
        ProbeRow {
            id: "fixture.unfiled",
            coverage: Coverage::NoMarker {
                reason: "drops rows with no count",
                bug: "docs/issues/2000-01-01-no-such-bug.md",
            },
        },
        ProbeRow {
            id: "fixture.empty_bug",
            coverage: Coverage::NoMarker {
                reason: "drops rows with no count",
                bug: "  ",
            },
        },
        ProbeRow {
            id: "fixture.wrong_dir",
            coverage: Coverage::NoMarker {
                reason: "drops rows with no count",
                bug: "Cargo.toml",
            },
        },
    ];

    assert_eq!(
        untested_rows_that_describe_a_missing_marker(&rows),
        vec!["fixture.silent"],
        "only an Untested row whose reason says nothing is disclosed is mis-filed"
    );
    assert_eq!(
        bugs_without_a_file(&rows, std::path::Path::new(env!("CARGO_MANIFEST_DIR"))),
        vec!["fixture.unfiled", "fixture.empty_bug", "fixture.wrong_dir"],
        "a NoMarker row must cite a real file under docs/issues/; an existing file elsewhere \
         does not count, and an Untested row has no bug to check"
    );
}

/// A floor is a `RESULT_CAP` iff what it suppresses is ROWS OF THE RESULT.
///
/// Regression for `docs/issues/archive/2026-09-03-result-cap-conflates-truncating-ceilings-with-suppressing-floors.md`:
/// `doctor.exposure_threshold` and `context.attestation_exposure` are both `>=`/`<` floors
/// over the same constant value, and were classified oppositely — the doctor one `Deferred`,
/// the context one `Probed` + `Mutation::Killed`, inside the branch's own "17 mutation-verified".
/// They differ in the one respect that decides the class. `doctor`'s floor `continue`s past
/// worklist rows, so the caller receives a partial result and cannot tell. `context`'s floor only
/// gates the `verification` advisory block beside a neighbourhood that is returned whole either
/// way, so nothing is withheld from the result — the same shape as `LINE_SOFT_CAP`, which is
/// `NOT_A_CAP`.
///
/// The context side reads its production annotation as TEXT rather than trusting the table:
/// the table can only say "no row", and a row deleted while the annotation still read
/// `RESULT_CAP` would be red in `tests/result_caps.rs`, but a row that quietly returns would
/// pass every other check.
#[test]
fn a_floor_is_a_result_cap_only_when_it_drops_rows_of_the_result() {
    use super::cap_probe::{Coverage, PROBE_ROWS};

    let doctor = PROBE_ROWS
        .iter()
        .find(|r| r.id == "doctor.exposure_threshold")
        .expect("doctor's floor drops worklist rows, so it keeps a RESULT_CAP row");
    assert!(
        matches!(doctor.coverage, Coverage::Probed { .. }),
        "doctor's floor drops rows and now counts them in `summary.below_exposure_floor` — a \
         Probed row; a revert to NoMarker means the count was lost"
    );

    assert!(
        PROBE_ROWS
            .iter()
            .all(|r| r.id != "context.attestation_exposure"),
        "context's floor gates an advisory block and withholds no row of the result; it is \
         NOT_A_CAP and carries no probe row, so it cannot inflate the mutation-verified count"
    );

    assert!(
        PROBE_ROWS
            .iter()
            .all(|r| r.id != "read_file.json_path_route_room"),
        "read_file's route-room floor only chooses the wording of `next`; both forms reach the \
         last line, so it withholds no row and is NOT_A_CAP with no probe row"
    );

    let above_const = |src: &str, name: &str| -> String {
        src.lines()
            .collect::<Vec<_>>()
            .windows(2)
            .find(|w| w[1].contains(&format!("const {name}")))
            .map(|w| w[0].to_string())
            .unwrap_or_else(|| panic!("const {name} is not declared"))
    };
    let rf = above_const(
        include_str!("../read_file.rs"),
        "JSON_PATH_ROUTE_ROOM_FLOOR",
    );
    assert!(
        rf.contains("cap-class: NOT_A_CAP"),
        "JSON_PATH_ROUTE_ROOM_FLOOR must be annotated NOT_A_CAP; the line above it reads: {rf:?}"
    );

    let src = include_str!("../../librarian/tools/context.rs");
    let above = src
        .lines()
        .collect::<Vec<_>>()
        .windows(2)
        .find(|w| w[1].contains("const ATTESTATION_EXPOSURE_THRESHOLD"))
        .map(|w| w[0].to_string())
        .expect("the const is declared in context.rs");
    assert!(
        above.contains("cap-class: NOT_A_CAP"),
        "ATTESTATION_EXPOSURE_THRESHOLD must be annotated NOT_A_CAP (it gates an advisory, \
         it withholds no row); the line above it reads: {above:?}"
    );
}

/// Prints the tally this convention lives and dies by. Asserts nothing
/// about mutation status — Task 5a's scope is classification, and a row
/// whose `mutation` is `NotYet` is not a defect this test polices.
#[test]
fn print_mutation_tally() {
    use super::cap_probe::{tally, PROBE_ROWS};

    let t = tally(PROBE_ROWS);

    println!(
        "IC-13 probe rows: {} total, {} probed, {} untested (worklist), {} no-marker (production defects), {} mutation-verified",
        t.total, t.probed, t.untested, t.no_marker, t.mutation_verified
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
            coverage: Coverage::Untested("no test drives this cap past its bound"),
        },
        ProbeRow {
            id: "fixture.no_marker",
            coverage: Coverage::NoMarker {
                reason: "the cap drops items and nothing discloses how many",
                bug: "docs/issues/fixture.md",
            },
        },
        ProbeRow {
            id: "fixture.no_marker_too",
            coverage: Coverage::NoMarker {
                reason: "a second silent cap, so a count of 1 cannot pass by accident",
                bug: "docs/issues/fixture.md",
            },
        },
    ];

    let t = tally(&rows);

    assert_eq!(t.total, 5);
    assert_eq!(t.probed, 2, "both Probed rows should count as probed");
    assert_eq!(
        t.mutation_verified, 1,
        "only the Killed row should count as mutation-verified"
    );
    assert_eq!(
        t.untested, 1,
        "the Untested row is a worklist item and counts once"
    );
    assert_eq!(
        t.no_marker, 2,
        "NoMarker rows are production defects, counted apart from the Untested worklist \
         — folding them into one `deferred` figure is the defect this split exists to prevent"
    );
    assert_eq!(
        t.probed + t.untested + t.no_marker,
        t.total,
        "every row lands in exactly one bucket"
    );
}
