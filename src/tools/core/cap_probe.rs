//! The declarative probe-row table for `IC-13`
//! (`docs/trackers/issue-clusters/IC-13-capped-result-presented-as-complete.md`).
//!
//! `tests/result_caps.rs` scans this file as TEXT — via `probe_row_ids` — to
//! cross-check the `id` field of every [`ProbeRow`] against the `RESULT_CAP`
//! ids declared in `src/`, in both directions. A text scan, not a compiled
//! import: `librarian` is a default Cargo feature, so an id gated behind
//! `#[cfg(feature = "librarian")]` still compiles out under
//! `--no-default-features`, and a gate that read this module as compiled
//! Rust would red the lean lane while passing the default one — a failure
//! reached by FOLLOWING `CLAUDE.md`'s gate order.
//!
//! Every row's `id` matches exactly one `cap-class: RESULT_CAP <id>`
//! annotation in tracked `src/`. A row records what evidence exists for
//! that cap TODAY, not what evidence a probe test could in principle
//! provide — `Coverage::Deferred` names why no behavioural test yet drives
//! the cap past its bound, and that reason must be specific enough to act
//! on: "the constant's use site was read until the reason could be stated
//! truthfully" is the bar, not "not got to it yet".

/// Where a marker must be reachable, in the shape the CALLER reads.
///
/// Both variants are needed non-hypothetically: `Grep` declares
/// `OutputForm::Text`, so its primary content block is never JSON, and a
/// `JsonPath`-only design could not name a marker for it at all.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Marker {
    /// A path into the primary content block parsed as JSON, e.g.
    /// `"$.counts.truncated"`.
    JsonPath(&'static str),
    /// A substring that must appear in the primary block rendered as TEXT.
    /// Valid evidence even for a JSON-shaped tool response — the check runs
    /// against the block's rendered text, not only for tools whose
    /// `OutputForm` is `Text`.
    TextContains(&'static str),
}

/// Has a mutation actually been run against this cap's marker-emission code?
#[derive(Debug, Clone, Copy)]
pub(crate) enum Mutation {
    /// The production marker emission was deleted, the row observed red,
    /// the deletion reverted.
    Killed,
    /// Not yet mutated. The string says why, and is not optional.
    NotYet(&'static str),
}

/// What's known about this cap's own probe coverage.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Coverage {
    /// A behavioural test drives this cap past its bound and asserts the
    /// marker arrives, through the tool's real call surface or a shared
    /// primitive whose output composes unmodified into that surface's
    /// response. Bounded: the marker check is a byte-level scan of the cited
    /// test's assertions, not a Rust tokenizer, so a cited assertion whose
    /// condition carries an unbalanced `(` in non-code text (a string
    /// literal, raw string, char literal, or trailing comment) can be
    /// certified on message text instead — see `condition_args`' doc
    /// comment in `tests/result_caps.rs` for the measured vector and what a
    /// new row's `cited_test` must therefore avoid.
    Probed {
        marker: Marker,
        mutation: Mutation,
        /// The name of the EXISTING test that drives this cap past its bound and
        /// asserts the marker. Checked by `probed_rows_cite_a_real_test` — this is
        /// a reference, not a note.
        cited_test: &'static str,
    },
    /// No behavioural test. The string says WHY — and "not got to it" is
    /// not a why.
    Deferred(&'static str),
}

/// One `RESULT_CAP` id and what's known about its probe coverage.
pub(crate) struct ProbeRow {
    /// Matches a `cap-class: RESULT_CAP <id>` annotation in `src/`.
    pub id: &'static str,
    pub coverage: Coverage,
}

/// Aggregate counts over a set of [`ProbeRow`]s, computed once so the report line
/// and its own regression test can never drift into disagreement with each other.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Tally {
    pub total: usize,
    pub probed: usize,
    pub deferred: usize,
    pub mutation_verified: usize,
}

/// Count `rows` into a [`Tally`]. The one and only place this arithmetic is
/// written — `print_mutation_tally` and
/// `tally_distinguishes_killed_from_not_yet_and_deferred` both call this
/// function rather than each keeping their own copy of the `matches!` pair, so
/// a mutation to the counting logic itself has one site to break, not two that
/// can silently diverge.
pub(crate) fn tally(rows: &[ProbeRow]) -> Tally {
    let total = rows.len();
    let probed = rows
        .iter()
        .filter(|r| matches!(r.coverage, Coverage::Probed { .. }))
        .count();
    let deferred = total - probed;
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
    Tally {
        total,
        probed,
        deferred,
        mutation_verified,
    }
}

// TASK 6 MUTATION SWEEP, 2026-09-03 — how every `Mutation::Killed` below was earned,
// published here rather than only in a report so the next reader can RE-RUN it instead
// of re-deriving it. Two constants used to live at this spot, `NOT_MUTATED_YET` ("Task
// 5a is classification-only scope") and `NOT_MUTATED_CENSUS` ("the mutation run for this
// id is Task 6's scope"); both are gone because no row still says either thing.
//
// The method, and it is the OPPOSITE direction from Task 5c's spot-checks — 5c deleted
// the cited TEST's assertion and asked whether the GATE reds, proving the citation is
// wired; this deletes the PRODUCTION marker emission and asks whether the CITED TEST
// reds, proving the test catches the marker disappearing. A row can pass the first and
// fail the second. The truncation itself is never touched: the mutated tree returns a
// result that is still capped and no longer says so, which is `IC-13` exactly. One
// mutation in the tree at a time (`target/` is shared; two live mutations can compile a
// tree that never existed), `git status --short` confirmed empty between rows.
//
// row id                                  | production line suppressed        | cited test red at
// ----------------------------------------|-----------------------------------|------------------
// tool_output.inline_tokens               | output_buffer.rs:466 "@tool_"     | core/tests.rs:1097
// run_command.inline_bytes                | run_command/output.rs:268         | run_command/tests.rs:2118
// tool_output.inline_byte_budget          | read_file.rs:350 (hint)           | read_file.rs:2060
// tool_output.compact_summary_hard_bytes  | core/types.rs:531 "(truncated)"   | core/tests.rs:1180
// doctor.caveat_chars                     | doctor.rs:4445 "…"                | doctor.rs:6636
// audit_doc_refs.files                    | audit_doc_refs/mod.rs:930 "cap"   | audit_doc_refs/mod.rs:1960
// context.max_tokens                      | context.rs:521 "packing"          | context.rs:2327
// context.attestation_exposure            | context.rs:570 verification_state | context.rs:1685
// artifact.get_lines                      | get.rs:749 "shown_lines"          | get.rs:977
// link_scan.findings                      | link_scan/mod.rs:960 "dangling"   | link_scan/mod.rs:1844
// prompts.client_instructions_chars       | prompts/mod.rs:422 trim_note      | prompts/mod.rs:1284
// grep.match_bytes                        | grep.rs:825 "truncated:"          | grep.rs:2031
// preview.default_headings                | headings.rs:93 headings_truncated | preview/default.rs:87
// preview.observation_text                | preview/memory.rs:50 "…"          | preview/memory.rs:148
// tracker_design.existing_trackers        | tracker_design.rs:665-669 hint    | tracker_design.rs:975
// index_state.skipped_sample              | NOT KILLED — see that row         | (n/a)
// command_summary.buffer_query_lines      | run_command/output.rs:269         | run_command/tests.rs:2262
// run_command.stderr_lines                | run_command/output.rs:272         | run_command/tests.rs:2224
//
// Line numbers are as of `9b40742d`. The two rows the Task 5c census does not cover
// (`audit_doc_refs.files`, `context.max_tokens`) had their emission sites located for
// this sweep: `enforce_file_cap`'s refusal message and `pack_entry_anchor`'s `packing`
// key, both cited above. Note `prompts/mod.rs`: the census recorded the `trim_note`
// writer at `:414` and `SHORT_NOTE` at `:377`; at this HEAD they are `:422` and `:385`,
// an 8-line drift, so the census's line numbers were re-verified rather than trusted.
//
// Of the 18 rows in the table above — the 2026-09-03 sweep's own population, scoped that
// way on purpose so a later append cannot falsify the ratio — 17 were killed. The one that
// did not is `index_state.skipped_sample`, and its row carries the finding: its declared
// marker names the capped payload rather than a disclosure beside it, so no mutation of the
// line the census names can suppress a marker while leaving the cap standing.
//
// ---- APPENDED 2026-09-10 (`doctor` per-project isolation merge) ----
//
// Same method, one new cap, run separately from the sweep above rather than folded into
// it. Run against the MERGED WORKTREE, not a clean checkout — stated plainly rather than
// implying the protocol's "`git status --short` empty between rows" precondition held,
// because it did not: the merge resolution was uncommitted at the time. What that
// precondition protects against is TWO live mutations compiling a tree that never existed,
// and there was exactly one. The mutated line and the observed red are recorded so the next
// reader can re-run this against a clean tree and disagree if it fails to reproduce.
//
// doctor.outside_roots_display             | doctor.rs:1356-1359 elision count | doctor.rs:10794
//   observed `left: Null / right: Number(4)`, message "14 roots seeded, 10 shown, 4 elided".
//   The truncation itself was left standing — the mutated tree still `take(10)`s and simply
//   stops saying that it did, which is `IC-13` exactly.

/// One row per `RESULT_CAP` id declared in tracked `src/` — 66 as of the
/// 2026-09-02 census in `tests/result_caps.rs`. The id list is derived from
/// the gate itself (`grep(pattern="cap-class: RESULT_CAP", path="src")`),
/// never copied from a report, and `tests/result_caps.rs`'s correspondence
/// check keeps the two from drifting apart in either direction.
pub(crate) const PROBE_ROWS: &[ProbeRow] = &[
    // -- src/tools/core/types.rs --
    ProbeRow {
        // BOUND (the condition `probed_rows_cite_a_real_test` cannot check): the cited
        // test seeds 13,685 B against the 10,000 B oversized threshold
        // (`MAX_INLINE_TOKENS` 2500 x 4). Marker written by production at
        // `src/tools/output_buffer.rs:466`.
        id: "tool_output.inline_tokens",
        coverage: Coverage::Probed {
            marker: Marker::TextContains("@tool_"),
            mutation: Mutation::Killed,
            cited_test: "call_content_buffers_at_token_threshold",
        },
    },
    ProbeRow {
        // BOUND (the condition `probed_rows_cite_a_real_test` cannot check): the cited
        // test seeds 11,000 B against a 9,700 B budget (10,000 - 300 envelope overhead -
        // 0 stderr). Marker written by production at
        // `src/tools/run_command/output.rs:268`.
        id: "run_command.inline_bytes",
        coverage: Coverage::Probed {
            marker: Marker::JsonPath("$.truncated"),
            mutation: Mutation::Killed,
            cited_test: "run_command_buffer_only_large_single_line_does_not_rebuffer",
        },
    },
    ProbeRow {
        // BOUND (the condition `probed_rows_cite_a_real_test` cannot check): the cited
        // test seeds a 13,212 B SINGLE line against `INLINE_BYTE_BUDGET` 9,000. Marker
        // written by production at `src/tools/read_file.rs:350`, literal at `:442-443`.
        id: "tool_output.inline_byte_budget",
        coverage: Coverage::Probed {
            marker: Marker::TextContains("json_path"),
            mutation: Mutation::Killed,
            cited_test: "read_file_buffer_single_oversized_line_still_fits_the_threshold",
        },
    },
    ProbeRow {
        // Task 5a's comment (preserved below in spirit) cited a COMPOSED PAIR, not one
        // test: `many_headings_escalates_to_map_shape_even_when_bytes_fit` (tests.rs)
        // drives HEADINGS_HARD_CAP past its bound through ReadMarkdown's real call
        // surface, but asserts only JSON key presence (`headings`, `file_id`) — never the
        // literal "@file_" text. `format_compact_map_shape_renders_indented_headings`
        // (tests.rs) asserts `contains("@file_xyz")`, but calls `format_compact` on a
        // hand-built JSON literal that already carries `file_id: "@file_xyz"` — it never
        // drives HEADINGS_HARD_CAP at all. `Coverage::Probed`'s `cited_test` field names
        // ONE test that both drives the cap and asserts the marker; neither of these does
        // either half completely, and Task 5b searched further (per this file's own
        // header) and found no third candidate: `read_markdown_call_content_returns_text_map_not_json`
        // (tests.rs) drives the cap cleanly (41 headings, ~1.8KB, well under the byte
        // budget) through the real `call_content()` surface, but its assertions check for
        // "lines"/'L', never the literal "@file_" substring;
        // `format_compact_live_renders_claude_md_as_map_shape` (tests.rs) does assert
        // `contains("lines  @file_")` against a live CLAUDE.md read, but CLAUDE.md has 15
        // headings against HEADINGS_HARD_CAP=40 and 40143 bytes against the
        // MAX_INLINE_TOKENS*4=10000-byte oversized threshold, so its MAP-shape escalation
        // is driven by the byte-size path, not the headings-count path — confounded
        // evidence for a different cap. Reclassifying to `Deferred` rather than inventing
        // a citation for a claim no single test backs.
        id: "markdown.headings_hard_cap",
        coverage: Coverage::Deferred(
            "Task 5a's Probed claim (marker TextContains(\"@file_\")) cited a composed \
             pair of tests, neither of which alone drives HEADINGS_HARD_CAP past its \
             bound AND asserts the \"@file_\" marker in the same body; see the comment \
             above for the full breakdown, including two further candidates ruled out on \
             direct measurement",
        ),
    },
    ProbeRow {
        id: "tool_output.compact_summary_bytes",
        coverage: Coverage::Deferred(
            "the two candidate fixtures each drive content past both the soft cap and its \
             sibling hard cap in the same call, so no assertion isolates the soft cap \
             specifically from tool_output.compact_summary_hard_bytes",
        ),
    },
    ProbeRow {
        // BOUND (the condition `probed_rows_cite_a_real_test` cannot check): the cited
        // test seeds 3,901 B against the 3,000 B hard cap. Marker written by production
        // at `src/tools/core/types.rs:531`.
        id: "tool_output.compact_summary_hard_bytes",
        coverage: Coverage::Probed {
            marker: Marker::TextContains("truncated"),
            mutation: Mutation::Killed,
            cited_test: "call_content_caps_compact_summary",
        },
    },
    ProbeRow {
        id: "tool_output.largest_array_depth",
        coverage: Coverage::Deferred(
            "the only candidate test (nested_payload_beats_a_shallow_but_smaller_array) \
             nests to depth 3; MAX_DEPTH is 4, so the cap's own threshold is never reached",
        ),
    },
    // -- src/tools/symbol/symbols.rs --
    ProbeRow {
        id: "symbols.find_results",
        coverage: Coverage::Deferred(
            "FIND_SYMBOL_MAX_RESULTS has no test seeding more than 50 matching symbols to \
             a name search; existing find-by-name tests use small fixture corpora",
        ),
    },
    ProbeRow {
        // Verified against `Symbols::output_form()` (`src/tools/symbol/symbols.rs:356`,
        // pinned by `symbols_declares_output_form_text`): the primary block is TEXT, so a
        // `JsonPath` marker is unreachable regardless of nesting — and the nesting was
        // also wrong (`by_file_overflow` lives at `$.overflow.by_file_overflow`, not
        // `$.by_file_overflow`; see `OutputGuard::overflow_json`). `format_search_symbols`
        // (`src/tools/symbol/display.rs`) now reads it back and names the count in the
        // rendered text — closing
        // docs/issues/archive/2026-09-03-symbols-by-file-overflow-is-an-unrecorded-ic-13-member.md.
        // Mutation-verified: the marker was observed absent before the fix (RED) and
        // present after (GREEN) against the same fixture.
        id: "symbols.by_file",
        coverage: Coverage::Probed {
            marker: Marker::TextContains("breakdown"),
            mutation: Mutation::Killed,
            cited_test: "symbols_with_overflow_names_the_capped_file_breakdown",
        },
    },
    ProbeRow {
        id: "symbols.per_lang_budget",
        coverage: Coverage::Deferred(
            "the per-language time budget in search_project_symbols has no test reference; \
             reaching it needs a multi-second multi-language fixture no test in the suite \
             pays for",
        ),
    },
    ProbeRow {
        id: "symbols.body_cap",
        coverage: Coverage::Deferred(
            "no test references the symbol-body truncation cap in finalize_search_results \
             or drives a returned body past it",
        ),
    },
    ProbeRow {
        id: "symbols.container_inline_lines",
        coverage: Coverage::Deferred(
            "no test references focus_single_symbol's container-inline-lines cap or drives \
             a container past it",
        ),
    },
    // -- src/tools/symbol/list_overview.rs --
    ProbeRow {
        id: "symbols.overview_files",
        coverage: Coverage::Deferred(
            "no directory-overview test seeds enough files to reach LIST_SYMBOLS_MAX_FILES",
        ),
    },
    ProbeRow {
        id: "symbols.overview_single_file",
        coverage: Coverage::Deferred(
            "the only related test declares its own local SINGLE_FILE_CAP fixture constant \
             and calls OutputGuard::cap_items directly rather than list_overview, so it \
             tests a decoupled fixture, not the shipped constant",
        ),
    },
    ProbeRow {
        id: "symbols.overview_single_file_flat",
        coverage: Coverage::Deferred(
            "two related tests exist, and neither exercises the shipped list_overview \
             capping path: symbols_overview_flat_cap_triggers_on_symbol_with_many_children \
             re-implements the greedy-capping loop locally instead of calling \
             list_overview, so a break in the production logic would not fail it, and \
             symbols_overview_flat_cap_not_triggered_for_leaf_heavy_symbols only confirms \
             a fixture safely under the cap",
        ),
    },
    ProbeRow {
        id: "symbols.overview_subdirs",
        coverage: Coverage::Deferred(
            "no directory-overview test seeds enough subdirectories to reach \
             LIST_SYMBOLS_MAX_SUBDIRS",
        ),
    },
    // -- src/librarian/adapter.rs --
    ProbeRow {
        id: "librarian.summary_items",
        coverage: Coverage::Deferred(
            "no test references matched_items_summary's item cap or seeds enough matched \
             items to reach it",
        ),
    },
    ProbeRow {
        id: "librarian.summary_title",
        coverage: Coverage::Deferred(
            "the only related assertion is a substring check that is monotone under \
             widening — it would pass identically if the title-length cap were removed",
        ),
    },
    ProbeRow {
        id: "librarian.summary_headings",
        coverage: Coverage::Deferred(
            "no test references section_headings_summary's heading cap or drives a \
             section list past it",
        ),
    },
    // -- src/librarian/preview/plan.rs --
    ProbeRow {
        // Reported ARGUABLE by the Task 5c census and ruled NO-CITE: the obvious citation
        // resolves to a DIFFERENT cap's test. This row is why
        // `probed_rows_cite_a_real_test` now refuses an ambiguous `cited_test` outright
        // (`IC-6`'s *no disambiguator* half) rather than taking the first match.
        id: "preview.plan_headings",
        coverage: Coverage::Deferred(
            "`MAX_HEADINGS` is exercised past its bound by `heading_truncation_is_signaled` \
             (`src/librarian/preview/plan.rs:168`, 25 headings vs 20), which asserts the \
             marker on production output from `headings::stamp_truncation`. It is not \
             citable: that function name is declared identically in \
             `preview/default.rs:57`, `preview/plan.rs:168` and `preview/spec.rs:76`, and \
             the gate resolves a `cited_test` by first match in `git ls-files src` order, \
             so the citation would silently bind to `default.rs:57` — the test for the \
             sibling `preview.default_headings` row. Citable as soon as one of the three \
             is given a distinguishing name.",
        ),
    },
    ProbeRow {
        // A cap that truncates and emits NO marker at all. Flagged to the whole-branch
        // review as arguably an `IC-13` MEMBER rather than a coverage gap.
        id: "preview.plan_open_next",
        coverage: Coverage::Deferred(
            "`OPEN_NEXT_LIMIT` (3) truncates the `open_next` vector at \
             `src/librarian/preview/plan.rs:41` and no companion field discloses the drop, \
             so the only observable is the array's own length. Every test in tracked \
             `src/` therefore reads the field into a `let` and asserts on that length \
             (`open_next_returns_first_three_unchecked`, `plan.rs:108`, drives 4 tasks \
             past the cap but asserts `open.len(), 3`), while the one assertion naming \
             `[\"tasks\"][\"open_next\"]` inline (`plan.rs:130`) seeds zero tasks. Uncitable \
             until the cap emits a marker of its own.",
        ),
    },
    ProbeRow {
        // Corrected: task_text_truncated_to_limit (plan.rs) seeds "x".repeat(150) against
        // TASK_TEXT_MAX=100 and asserts <=, which DOES red if truncation is removed
        // outright — it is monotone under TIGHTENING (over-truncation), not removal. The
        // real IC-13 gap is different: truncate_task_text appends no ellipsis or other
        // marker at the caller-visible surface, so a genuinely-cut task and a genuinely-
        // short one are indistinguishable to the caller reading the result.
        id: "preview.plan_task_text",
        coverage: Coverage::Deferred(
            "truncate_task_text appends no ellipsis or other marker at the caller's \
             surface, so a caller cannot tell a cut task from a genuinely short one; the \
             length-cap itself is tested (task_text_truncated_to_limit), but that is \
             orthogonal to the marker gap IC-13 is about",
        ),
    },
    // -- src/librarian/tools/doctor.rs --
    ProbeRow {
        id: "doctor.listed_roots",
        coverage: Coverage::Deferred(
            "none of the outside_managed_roots_* tests seed more than the 5-root \
             MAX_LISTED cap; the closest fixture seeds a single no-root row, an \
             unrelated case",
        ),
    },
    ProbeRow {
        // A suppression FLOOR, not a truncating ceiling. Flagged to the whole-branch
        // review: `RESULT_CAP` currently conflates ceilings that truncate with floors
        // that suppress, and "the marker arrives" means a different thing under each.
        id: "doctor.exposure_threshold",
        coverage: Coverage::Deferred(
            "`EXPOSURE_THRESHOLD` (5) is a suppression floor, not a truncating ceiling: \
             `src/librarian/tools/doctor.rs:2947` drops rows *below* it and no field \
             reports how many were dropped, while the marker \
             `$.summary.by_check.entry_conditional_past_due` counts rows that survived. \
             The marker is therefore observable non-zero only when the cap did not bite. \
             The one test that drives exposure below the floor \
             (`conditional_past_due_fires_exactly_at_the_exposure_threshold`, \
             `doctor.rs:11267`) calls `scan_conditional_past_due` directly rather than \
             `call()`, so no `summary.by_check` object exists in its output at all.",
        ),
    },
    ProbeRow {
        // BOUND (the condition `probed_rows_cite_a_real_test` cannot check): the cited
        // test seeds a 480-CHARACTER caveat against the 240-character cap (the repeated
        // unit is 4 chars / 9 bytes, x120). Marker written by production at
        // `src/librarian/tools/doctor.rs:4445`.
        id: "doctor.caveat_chars",
        coverage: Coverage::Probed {
            marker: Marker::TextContains("…"),
            mutation: Mutation::Killed,
            cited_test: "a_long_caveat_is_truncated_without_splitting_a_character",
        },
    },
    ProbeRow {
        // Added 2026-09-10 with the `doctor` per-project isolation merge, which is what
        // introduced the constant: `OUTSIDE_ROOTS_DISPLAY_LIMIT` collapses
        // `outside_roots_by_project` to its ten highest-count roots at every scope below
        // `all`. The cited test seeds FOURTEEN roots against that ten-root cap, so the
        // bound is genuinely crossed rather than merely configured.
        //
        // The marker is the elision COUNT beside the capped map, not the map itself — the
        // distinction `index_state.skipped_sample` records as a finding. And
        // `catalog_health.outside_roots_total` stays exact and unconditional at every
        // scope, so a caller is handed both the total and the shortfall.
        id: "doctor.outside_roots_display",
        coverage: Coverage::Probed {
            marker: Marker::JsonPath("$.catalog_health.outside_roots_elided"),
            mutation: Mutation::Killed,
            cited_test: "outside_roots_by_project_collapses_to_the_top_ten_by_count",
        },
    },
    // -- src/tools/format.rs --
    ProbeRow {
        id: "format.shape_keys",
        coverage: Coverage::Deferred(
            "no test references describe_payload_shape's key-count cap or seeds an object \
             with enough keys to reach it",
        ),
    },
    ProbeRow {
        // Verified: `the_generic_fallback_describes_the_payload_instead_of_the_envelope`
        // (format.rs) DOES drive this cap past its bound — it seeds a 20,000-char `body`
        // against MAX_SCALAR_LEN=60 and asserts `!shape.contains("xxxxxxxxxx")` (the raw
        // value is never inlined) and `shape.len() < 600`; raising or removing the cap
        // reds both. But `describe_payload_shape`'s scalars filter (`format.rs`) SKIPS an
        // over-length string from the `scalars:` line entirely — it emits no "+N more" or
        // any other marker that arrives when the cap fires, unlike MAX_KEYS's "… +{} more".
        // `Coverage::Probed`'s contract (this file, `Marker` doc comment) requires "a
        // behavioural test... asserts the marker arrives" — a positive, present signal.
        // This cap's only real test evidence is an ABSENCE assertion, which does not fit;
        // inventing a `TextContains` value here would claim something never arrives as
        // though it does. Reclassifying to `Deferred` with the honest reason, not `Probed`.
        id: "format.shape_scalar_len",
        coverage: Coverage::Deferred(
            "the_generic_fallback_describes_the_payload_instead_of_the_envelope drives a \
             20,000-char scalar past MAX_SCALAR_LEN=60 and asserts the raw value is never \
             inlined, but describe_payload_shape emits no marker that arrives when the cap \
             fires (an over-length scalar is silently dropped from the scalars: line, \
             unlike MAX_KEYS's '… +N more'); the only test evidence is an absence \
             assertion, which Coverage::Probed's marker contract does not cover",
        ),
    },
    ProbeRow {
        id: "format.shape_scalars",
        coverage: Coverage::Deferred(
            "no test references describe_payload_shape's scalar-count cap or seeds enough \
             scalars to reach it",
        ),
    },
    // -- src/tools/symbol/references.rs --
    ProbeRow {
        id: "references.probe_positions",
        coverage: Coverage::Deferred(
            "no test references resolve_binding_by_position's position-probe cap by name \
             or drives it past its threshold",
        ),
    },
    ProbeRow {
        id: "references.corroborate_files_scan",
        coverage: Coverage::Deferred(
            "bounds a workspace file SCAN inside corroborate_zero_references; exceeding it \
             silently under-scans rather than reporting a smaller number, so a false zero \
             (\"no references\") is indistinguishable from \"scan stopped early\" — no \
             fixture currently isolates the two",
        ),
    },
    ProbeRow {
        id: "references.corroborate_hits",
        coverage: Coverage::Deferred(
            "no test references corroborate_zero_references's hit cap or seeds enough \
             corroborating hits to reach it",
        ),
    },
    // -- src/librarian/tools/audit_doc_refs/mod.rs --
    ProbeRow {
        // Verified against `enforce_file_cap` in
        // `src/librarian/tools/audit_doc_refs/mod.rs`: exceeding this cap is a LOUD
        // `RecoverableError` refusal, not a silent truncation — the opposite of the
        // false-zero failure mode `audit_doc_refs.basename_index` names below. The
        // existing `glob_explosion_returns_recoverable` test drives the cap past its
        // bound (`enforce_file_cap(5, 1)`) and asserts `msg.contains("cap") &&
        // msg.contains('5')` — NOT "glob matched" (that phrase is unasserted
        // production text at `mod.rs:930`; a prior version of this row cited it in
        // error). "cap" is the only substring the test actually pins; it composes
        // unmodified into `call()`'s response via `?` (`get_guide("error-handling")`).
        id: "audit_doc_refs.files",
        coverage: Coverage::Probed {
            marker: Marker::TextContains("cap"),
            mutation: Mutation::Killed,
            cited_test: "glob_explosion_returns_recoverable",
        },
    },
    ProbeRow {
        id: "audit_doc_refs.basename_index",
        coverage: Coverage::Deferred(
            "bounds build_basename_index's own scan; an index that stopped early \
             silently resolves fewer refs rather than warning indexing was incomplete — \
             a false zero on a truncated index reads as 'no such basename', not 'index \
             incomplete', and no fixture currently drives it past the cap",
        ),
    },
    // -- src/librarian/tools/audit_log.rs --
    ProbeRow {
        id: "audit_log.default_limit",
        coverage: Coverage::Deferred(
            "no test omits limit while seeding more rows than the default; the largest \
             seeded corpus in any audit_log test is a handful of rows",
        ),
    },
    ProbeRow {
        id: "audit_log.max_limit",
        coverage: Coverage::Deferred(
            "no test requests a limit anywhere near the max; limit-testing tests use \
             small explicit overrides",
        ),
    },
    // -- src/librarian/tools/context.rs --
    ProbeRow {
        // Recounted directly against source (fix round 2 — the prior round's 21/11 split
        // was received as given fact from the dispatcher, not counted): of 32 total
        // call(&ctx, json!(...)) sites, exactly 7 pass an explicit max_tokens override
        // (lines 1202, 1354, 1387, 1414, 1491, 2126, 2258 — five distinct values, two
        // repeated), and 25 omit it, exercising DEFAULT_MAX_TOKENS=4000
        // (char_cap = max_tokens * 4 = 16000, context.rs:584-585).
        //
        // The qualitative half — "none of the omitting fixtures approach the ceiling" —
        // does NOT hold: `a_neighbourhood_that_does_not_fit_whole_is_excerpted_rather_than_dropped`
        // (context.rs:2284, call site context.rs:2312, no max_tokens override) seeds six
        // ~3000-byte neighbours (18000 bytes total) against the 16000-byte default
        // char_cap threaded straight into `pack_entry_anchor` (context.rs:599-608), and
        // asserts `v["overflow"]["packing"] == "excerpted"` — a value `pack_entry_anchor`
        // itself writes at context.rs:521 (`if excerpted { "excerpted" } else { "whole" }`),
        // not a test-side reconstruction. If DEFAULT_MAX_TOKENS were raised enough to fit
        // 18000 bytes under budget, or the cap bypassed, `packing` would read "whole" and
        // this assertion would fail — so the default value is in fact caller-visibly
        // mutation-sensitive through this one fixture. The other 24 omitting call sites
        // were checked (every `.repeat(`/`push_str`/`for i in` content-building site in
        // this file cross-referenced against call-site line) and build content far under
        // the 16000-byte ceiling — this is the one exception, not the rule.
        id: "context.max_tokens",
        coverage: Coverage::Probed {
            marker: Marker::JsonPath("$.overflow.packing"),
            mutation: Mutation::Killed,
            cited_test: "a_neighbourhood_that_does_not_fit_whole_is_excerpted_rather_than_dropped",
        },
    },
    ProbeRow {
        // BOUND (the condition `probed_rows_cite_a_real_test` cannot check): the cited
        // test seeds 5 citers against a threshold of 5 — this cap is a `>=` FLOOR, so 5
        // is past its bound, and the test brackets it with a negative control at 4.
        // Marker written by production at `src/librarian/tools/context.rs:568-574`.
        id: "context.attestation_exposure",
        coverage: Coverage::Probed {
            marker: Marker::JsonPath("$.verification.verification_state"),
            mutation: Mutation::Killed,
            cited_test: "a_load_bearing_statement_arms_the_tap_and_says_what_would_discharge_it",
        },
    },
    // -- src/librarian/tools/find.rs --
    ProbeRow {
        id: "artifact.find_limit",
        coverage: Coverage::Deferred(
            "the only related test (clamps_oversized_limit) seeds a single row, which \
             cannot discriminate a working clamp from none; a sibling test drives a \
             different, smaller-granularity overflow signal on a 3-row corpus, not this \
             500-item clamp",
        ),
    },
    ProbeRow {
        id: "artifact.find_offset",
        coverage: Coverage::Deferred(
            "no test references the offset cap by name; only the definition and one \
             production use site exist",
        ),
    },
    // -- src/librarian/tools/get.rs --
    ProbeRow {
        // BOUND (the condition `probed_rows_cite_a_real_test` cannot check): the cited
        // test seeds 605 lines against `SOFT_CAP_LINES` 500. Marker written by production
        // at `src/librarian/tools/get.rs:748-752`.
        id: "artifact.get_lines",
        coverage: Coverage::Probed {
            marker: Marker::JsonPath("$.overflow.shown_lines"),
            mutation: Mutation::Killed,
            cited_test: "full_true_triggers_overflow_over_cap",
        },
    },
    ProbeRow {
        id: "artifact.get_overflow_headings",
        coverage: Coverage::Deferred(
            "no test drives the top-level-heading list inside the overflow hint past its \
             own cap; existing overflow tests use a three-heading fixture",
        ),
    },
    // -- src/librarian/tools/link_scan/mod.rs --
    ProbeRow {
        id: "link_scan.artifacts",
        coverage: Coverage::Deferred(
            "no test references the artifact-scan cap by name; only the definition and \
             one production use site exist",
        ),
    },
    ProbeRow {
        // BOUND (the condition `probed_rows_cite_a_real_test` cannot check): the cited
        // test seeds 51 findings against `FINDINGS_CAP` 50. Marker written by production
        // at `src/librarian/tools/link_scan/mod.rs:960`.
        id: "link_scan.findings",
        coverage: Coverage::Probed {
            marker: Marker::JsonPath("$.counts.truncated.dangling"),
            mutation: Mutation::Killed,
            cited_test: "counts_flags_truncation_per_finding_array_when_the_cap_is_exceeded",
        },
    },
    // -- src/librarian/tools/refresh_stale.rs --
    ProbeRow {
        id: "refresh_stale.max_limit",
        coverage: Coverage::Deferred(
            "the only related test calls augmentation::list_stale directly, bypassing the \
             tool's call() surface, and uses an explicit limit override far below either \
             constant's value",
        ),
    },
    ProbeRow {
        id: "refresh_stale.default_limit",
        coverage: Coverage::Deferred(
            "the same test as refresh_stale.max_limit exercises an explicit override, \
             never the shipped default, and bypasses call()",
        ),
    },
    // -- src/prompts/mod.rs --
    ProbeRow {
        // BOUND (the condition `probed_rows_cite_a_real_test` cannot check): the cited
        // test seeds ~1,006 chars against a DYNAMIC budget of 302 (2048 ceiling - 48
        // margin - 1698 static prefix; the fixture's char count was verified with
        // `wc -m`, not eyeballed). Marker written by production at
        // `src/prompts/mod.rs:422` (the census recorded `:414`; an 8-line drift since,
        // re-verified at `9b40742d`).
        //
        // MARKER NARROWED, 2026-09-03 (Task 6). This row carried `TextContains("trimmed")`
        // and a note that the citation was NOT DISCRIMINATING as the gate reads it:
        // deleting the cited marker assertion — `rendered.contains("status trimmed: ")` at
        // `mod.rs:1285` — left `probed_rows_cite_a_real_test` GREEN, because a later
        // assertion in the same body carries the literal `trimmed` inside its OWN
        // condition, `assert!(!block.contains("trimmed"), ...)` at `:1303`. The row was
        // therefore certifiable by an assertion that the marker is ABSENT from a different
        // channel — the inverse of what `Coverage::Probed` claims. That is
        // `assertion_lines`' documented cross-assertion laxity for `TextContains`, with a
        // LIVE instance rather than a hypothetical one.
        //
        // The narrowing to `TextContains("status trimmed: ")` is independently the more
        // accurate marker, not a fix aimed at the gate: `SHORT_NOTE` (`mod.rs:385`) reads
        // "status trimmed to fit…" with NO COLON, so the colon-space form is the only
        // marker that tells the `trim_note` writer (`:422`) apart from its fallback.
        // `TextContains("trimmed")` cannot distinguish the two production paths at all.
        //
        // BOTH DIRECTIONS MEASURED at `9b40742d`, and they are different questions:
        //   - stale marker `"trimmed"`, cited assertion at `:1284-1287` deleted -> gate
        //     `probed_rows_cite_a_real_test` GREEN (1 passed). The defect, reproduced.
        //   - narrowed marker, same assertion deleted -> gate RED, naming this row.
        //   - narrowed marker restored, production `:422` suppressed
        //     (`format!("- ({list})\n")`) -> cited test `a_trim_names_what_it_dropped` RED
        //     at `mod.rs:1284`, "the note must name the losses, not just announce one".
        // The first two prove the CITATION is wired (Task 5c's direction); the third proves
        // the TEST catches the marker disappearing (this task's). Hence `Mutation::Killed`.
        id: "prompts.client_instructions_chars",
        coverage: Coverage::Probed {
            marker: Marker::TextContains("status trimmed: "),
            mutation: Mutation::Killed,
            cited_test: "a_trim_names_what_it_dropped",
        },
    },
    ProbeRow {
        // Reported ARGUABLE by the Task 5c census and ruled NO-CITE: promoting it would
        // certify a marker no caller can observe. `Coverage::Probed` claims the marker
        // ARRIVES, and through the tool surface this one cannot. The trigger that makes
        // it live is annotated at the segment site (`build_project_status_segments` in
        // `src/prompts/mod.rs`), not here — that is where someone would fire it.
        id: "prompts.trim_note_names",
        coverage: Coverage::Deferred(
            "`MAX_NAMED_DROPS` (3) caps the labels `trim_note` lists \
             (`src/prompts/mod.rs:405-412`), but its only production caller \
             `fit_dynamic_block` draws labels from non-`Anchor`, non-`Substitutable` \
             segments and exactly one such segment exists (`custom instructions`, \
             `StatusPriority::UserAuthored`, `mod.rs:242`), so `labels.len() <= 1` and the \
             `+2 more` branch cannot be reached through `build_server_instructions`. \
             `the_trim_note_caps_the_names_it_lists` (`mod.rs:1341`) drives the primitive \
             itself past the cap and asserts the marker, so this becomes citable the \
             moment a second droppable persistent segment exists — **and that is the \
             trigger to watch, not this row.**",
        ),
    },
    // -- src/tools/grep.rs --
    ProbeRow {
        // BOUND (the condition `probed_rows_cite_a_real_test` cannot check): the cited
        // test seeds a 50,013 B SINGLE line against `MAX_MATCH_BYTES` 2,000. Marker
        // written by production at `src/tools/grep.rs:825`.
        id: "grep.match_bytes",
        coverage: Coverage::Probed {
            marker: Marker::TextContains("truncated"),
            mutation: Mutation::Killed,
            cited_test: "grep_marks_a_clamped_line_instead_of_silently_cutting",
        },
    },
    ProbeRow {
        id: "grep.total_bytes",
        coverage: Coverage::Deferred(
            "the only candidate test's total stays under budget from the sibling \
             per-match MAX_MATCH_BYTES clamp alone (5×~2KB≈10KB), never approaching the \
             60KB total cap — it cannot prove MAX_TOTAL_MATCH_BYTES itself engages",
        ),
    },
    // -- src/workspace.rs --
    ProbeRow {
        id: "workspace.scan_depth",
        coverage: Coverage::Deferred(
            "the only related test (max_depth_limits_discovery) drives discover_projects's \
             own externally-parameterized max_depth argument, a different function from \
             scan_languages_by_dominance whose internal constant is not injectable",
        ),
    },
    ProbeRow {
        id: "workspace.scan_files",
        coverage: Coverage::Deferred(
            "no test constructs more than a handful of files; existing \
             workspace-discovery tests use small fixture trees, far under this scan's \
             file-count cap",
        ),
    },
    // -- src/librarian/catalog/find.rs --
    ProbeRow {
        id: "catalog.semantic_k",
        coverage: Coverage::Deferred(
            "the only related test asserts a zero-widenings result over a tiny seeded \
             corpus and never drives the candidate pool near the K_CAP threshold",
        ),
    },
    // -- src/librarian/preview/default.rs --
    ProbeRow {
        // BOUND (the condition `probed_rows_cite_a_real_test` cannot check): the cited
        // test seeds 26 headings against `MAX_HEADINGS` 20. Marker written by production
        // at `src/librarian/preview/headings.rs:93`.
        //
        // UNIQUENESS: the semantically obvious citation here is
        // `heading_truncation_is_signaled`, which is declared THREE times in tracked
        // `src/` (`preview/default.rs:57`, `plan.rs:168`, `spec.rs:76`). The uniquely
        // named sibling below is cited instead; see the `preview.plan_headings` row for
        // why the collision is disqualifying rather than merely untidy.
        id: "preview.default_headings",
        coverage: Coverage::Probed {
            marker: Marker::JsonPath("$.headings_truncated"),
            mutation: Mutation::Killed,
            cited_test: "a_truncated_preview_still_names_its_final_heading",
        },
    },
    // -- src/librarian/preview/memory.rs --
    ProbeRow {
        // BOUND (the condition `probed_rows_cite_a_real_test` cannot check): the cited
        // test seeds 300 chars against `OBSERVATION_TEXT_MAX` 200. Marker written by
        // production at `src/librarian/preview/memory.rs:50`.
        id: "preview.observation_text",
        coverage: Coverage::Probed {
            marker: Marker::TextContains("…"),
            mutation: Mutation::Killed,
            cited_test: "observation_text_truncated_to_limit",
        },
    },
    // -- src/librarian/preview/spec.rs --
    ProbeRow {
        id: "preview.spec_headings",
        coverage: Coverage::Deferred(
            "the only test for this module (extracts_headings_and_summary) seeds 2 \
             headings, far under the 20-item cap — never proven the cap engages, unlike \
             its sibling preview.default_headings/preview.plan_headings",
        ),
    },
    // -- src/librarian/preview/summary.rs --
    ProbeRow {
        id: "preview.summary_chars",
        coverage: Coverage::Deferred(
            "no test references the summary character cap or seeds a summary paragraph \
             long enough to reach it",
        ),
    },
    // -- src/librarian/tools/link_scan/resolve.rs --
    ProbeRow {
        id: "link_scan.ambiguous_candidates",
        coverage: Coverage::Deferred(
            "no test references the ambiguous-candidate cap by name or drives a \
             resolution past it",
        ),
    },
    // -- src/librarian/tools/tracker_design.rs --
    ProbeRow {
        // BOUND (the condition `probed_rows_cite_a_real_test` cannot check): the cited
        // test seeds 10 trackers against `EXISTING_TRACKERS_CAP` 5. Marker written by
        // production at `src/librarian/tools/tracker_design.rs:666`.
        id: "tracker_design.existing_trackers",
        coverage: Coverage::Probed {
            marker: Marker::JsonPath("$.existing_trackers_overflow_hint"),
            mutation: Mutation::Killed,
            cited_test: "overflow_hint_when_above_cap",
        },
    },
    // -- src/librarian/tools/workspace_state_at.rs --
    ProbeRow {
        // The marker IS reached by a real test that drives the cap past its bound; it is
        // read through `.expect()` rather than an assertion macro, so `assertion_lines`
        // never opens a block on it. A test change would make this citable — which is
        // not the same thing as an existing citation, and 5c does not write tests.
        id: "workspace_state_at.rows",
        coverage: Coverage::Deferred(
            "`MAX_ROWS` (200) bounds the `find` call at \
             `src/librarian/tools/workspace_state_at.rs:152` and the overflow signal is \
             written at `:231`. `cap_returns_hint` (`:339`) seeds 250 artifacts and \
             reaches that branch, but reads the signal through a `let` + `.expect()` at \
             `:358-360` — not an assertion macro — and its only assert conditions are \
             `arts.len() <= MAX_ROWS` and `more >= 50`. No assertion condition anywhere in \
             tracked `src/` names both `hints` and `more_in_scope`, so the marker is not \
             certifiable today. Hoisting the presence check into an `assert!` would make \
             it citable; that is a test change, not an existing citation.",
        ),
    },
    // -- src/lsp/client.rs --
    ProbeRow {
        id: "lsp.did_open_size",
        coverage: Coverage::Deferred(
            "no test references the did_open payload-size cap by name or drives a \
             document body past it",
        ),
    },
    // -- src/lsp/mod.rs --
    ProbeRow {
        // Task 5a's comment cited ONE test, but that test never drives the real
        // first-call-time-budget cap: `format_overview_symbols_file_mode_warming_marker`
        // (display.rs:548-565) hand-builds a JSON value with `"lsp": "warming"` already
        // set and asserts the renderer embeds "[lsp warming]" in the rendered text — it
        // never calls the code path that decides whether the first-call time budget
        // elapsed (`src/lsp/mod.rs`'s cold-start race). This is the render-only half of
        // the same gap `markdown.headings_hard_cap` has above: a marker-rendering test
        // standing in for a cap-driving one. `Coverage::Probed` requires a test that
        // drives the cap past its bound AND asserts the marker; this one only does the
        // latter, against a synthetic already-tripped input. Reclassifying to `Deferred`.
        id: "lsp.first_call_budget",
        coverage: Coverage::Deferred(
            "Task 5a's Probed claim (marker TextContains(\"[lsp warming]\")) cited \
             format_overview_symbols_file_mode_warming_marker, which asserts the renderer \
             embeds the marker given an already-synthetic `\"lsp\": \"warming\"` input — \
             it never drives the real first-call-budget race past its bound; no test \
             today does both",
        ),
    },
    // -- src/retrieval/index_state.rs --
    ProbeRow {
        // BOUND (the condition `probed_rows_cite_a_real_test` cannot check): the cited
        // test seeds 30 entries against `SKIPPED_SAMPLE_CAP` 20. Marker written by
        // production at `src/retrieval/index_state.rs:278`.
        //
        // MARKER NOTE, flagged not fixed: `$.last_sync_skipped.sample` is not literally a
        // nested path — the serde field is the FLAT `last_sync_skipped_sample`, with
        // `last_sync_skipped_count` beside it. The gate's per-segment substring check
        // passes either way and the cited test asserts both fields. Task 5a owns the
        // marker string; it is recorded here rather than silently re-derived.
        //
        // TASK 6 MUTATION FINDING, 2026-09-03 — the only row of the 18 not killed as
        // declared, and the reason is the marker, not the test. Every other row's marker
        // names a DISCLOSURE field emitted beside the capped payload, so deleting it
        // leaves the truncation in place and the result silently capped — the mutation
        // this task runs. Here the declared marker names the capped PAYLOAD itself:
        // `index_state.rs:278` IS `last_sync_skipped_sample`, so no edit to that line
        // suppresses a marker while leaving the cap standing. The disclosure is the
        // sibling `last_sync_skipped_count` at `:277`, which the declared JsonPath does
        // not name. Suppressing THAT (`skipped.len()` ->
        // `skipped.iter().take(SKIPPED_SAMPLE_CAP).count()`, so the count agrees with the
        // capped sample and the result reads as complete) DOES red the cited test at
        // `index_state.rs:629` — "count must stay exact even when the sample is capped",
        // left 20 right 30. So the test does observe disclosure loss; the row is left
        // NotYet because that is a mutation of a line the marker does not declare, and
        // tuning the mutation until it reds is exactly what this task must not do.
        id: "index_state.skipped_sample",
        coverage: Coverage::Probed {
            marker: Marker::JsonPath("$.last_sync_skipped.sample"),
            mutation: Mutation::NotYet(
                "the declared marker names the capped payload, not a disclosure beside it: \
                 `index_state.rs:278` IS the `last_sync_skipped_sample` field, so no edit \
                 there suppresses a marker while leaving the cap in place. Suppressing the \
                 real disclosure — the sibling `last_sync_skipped_count` at `:277` — does \
                 red the cited test at `index_state.rs:629` (left 20, right 30), but that \
                 line is not what this row's marker declares. Citable as Killed once the \
                 marker is re-declared onto `last_sync_skipped_count`; that is a marker \
                 change, which Task 5a owns",
            ),
            cited_test: "last_sync_skipped_sample_is_capped_but_count_stays_exact",
        },
    },
    // -- src/symbol/edit.rs --
    ProbeRow {
        id: "rename.text_sweep_file_bytes",
        coverage: Coverage::Deferred(
            "no test references the text-sweep file-size cap by name or drives a \
             matched file past it",
        ),
    },
    // -- src/tools/command_summary.rs --
    ProbeRow {
        // BOUND (the condition `probed_rows_cite_a_real_test` cannot check): the cited
        // test seeds 250 lines against a 90-LINE budget (`BUFFER_QUERY_INLINE_CAP` 100
        // minus 10 lines already taken by stderr). The sibling BYTE budget was checked
        // separately and does NOT bind here (9,060 B available vs 5,760 B kept), so the
        // line cap is what this row's evidence isolates. Marker written by production at
        // `src/tools/run_command/output.rs:269`.
        id: "command_summary.buffer_query_lines",
        coverage: Coverage::Probed {
            marker: Marker::JsonPath("$.stdout_shown"),
            mutation: Mutation::Killed,
            cited_test: "run_command_buffer_only_short_stderr_gives_budget_to_stdout",
        },
    },
    // -- src/tools/run_command/output.rs --
    ProbeRow {
        // BOUND (the condition `probed_rows_cite_a_real_test` cannot check): the cited
        // test seeds 25 stderr lines against `STDERR_BUDGET` 20. Marker written by
        // production at `src/tools/run_command/output.rs:272`.
        id: "run_command.stderr_lines",
        coverage: Coverage::Probed {
            marker: Marker::JsonPath("$.stderr_shown"),
            mutation: Mutation::Killed,
            cited_test: "run_command_buffer_only_stderr_gets_priority",
        },
    },
    // -- src/tools/symbol/call_graph/mod.rs --
    ProbeRow {
        id: "call_graph.workspace_files_scan",
        coverage: Coverage::Deferred(
            "bounds CachedResolver::lookup_pos_via_ts_workspace's own file-scan; \
             exceeding it silently returns fewer or no candidate positions rather than \
             reporting an incomplete scan — a false zero indistinguishable from 'no \
             callers exist', and no fixture currently isolates the two",
        ),
    },
    // -- src/agent/build_check.rs --
    ProbeRow {
        id: "agent_build_check.rendered_diagnostics",
        coverage: Coverage::Deferred(
            "no marker exists to assert. `errors_naming` breaks at MAX_RENDERED and \
             returns `hits.join(\"\\n\")`, and `render_notice` wraps that with no total, \
             so an author with 12 compile errors is shown 3 and told nothing about the \
             other 9. The existing `at_most_three_errors_are_rendered` asserts the CAP \
             (`got.lines().count() == MAX_RENDERED`), which is the bound holding, not a \
             disclosure arriving. This is therefore a new IC-13 member rather than a \
             coverage gap: the row cannot become Probed until production emits a count, \
             and tuning the row until it passed would hide the finding",
        ),
    },
    // -- src/librarian/tools/doctor.rs --
    ProbeRow {
        id: "doctor.recently_touched_walk",
        coverage: Coverage::Deferred(
            "the ceiling IS reached on every call in this repo — 5,582 commits against \
             a 4,000 walk, measured 2026-09-09 — and `paths_touched_since` returns a \
             bare HashSet, breaking at the bound with nothing marking the set partial. \
             What blocks a probe is reachability of the DEFECT, not of the cap: the walk \
             is TIME-sorted, so the truncated tail's newest member is 2026-05-17, ~115 \
             days behind the 7-day cutoff every caller passes, and no input available \
             here makes the truncation change an answer. Driving it needs a fixture repo \
             of >4,000 commits whose committer-time order is non-monotonic across the \
             boundary — precisely the condition the ceiling exists to tolerate, per \
             `paths_touched_since`'s own doc comment",
        ),
    },
];
