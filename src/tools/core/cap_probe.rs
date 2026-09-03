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
    /// response.
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

const NOT_MUTATED_YET: &str =
    "Task 5a is classification-only scope; no mutation run has been performed for this id";

/// One row per `RESULT_CAP` id declared in tracked `src/` — 66 as of the
/// 2026-09-02 census in `tests/result_caps.rs`. The id list is derived from
/// the gate itself (`grep(pattern="cap-class: RESULT_CAP", path="src")`),
/// never copied from a report, and `tests/result_caps.rs`'s correspondence
/// check keeps the two from drifting apart in either direction.
pub(crate) const PROBE_ROWS: &[ProbeRow] = &[
    // -- src/tools/core/types.rs --
    ProbeRow {
        id: "tool_output.inline_tokens",
        coverage: Coverage::Deferred(
            "Task 5a classified this Probed (marker TextContains(\"@tool_\")) with no \
             comment citing a test; Task 5b's cited_test requirement found none named, \
             and inventing one is the exact failure this gate exists to prevent",
        ),
    },
    ProbeRow {
        id: "run_command.inline_bytes",
        coverage: Coverage::Deferred(
            "Task 5a classified this Probed (marker JsonPath(\"$.truncated\")) with no \
             comment citing a test; Task 5b's cited_test requirement found none named, \
             and inventing one is the exact failure this gate exists to prevent",
        ),
    },
    ProbeRow {
        id: "tool_output.inline_byte_budget",
        coverage: Coverage::Deferred(
            "Task 5a classified this Probed (marker TextContains(\"json_path\")) with no \
             comment citing a test; Task 5b's cited_test requirement found none named, \
             and inventing one is the exact failure this gate exists to prevent",
        ),
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
        id: "tool_output.compact_summary_hard_bytes",
        coverage: Coverage::Deferred(
            "Task 5a classified this Probed (marker TextContains(\"truncated\")) with no \
             comment citing a test; Task 5b's cited_test requirement found none named, \
             and inventing one is the exact failure this gate exists to prevent",
        ),
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
        // `$.by_file_overflow`; see `OutputGuard::overflow_json`). Worse, no compact-text
        // renderer reads it back: `format_search_symbols` groups the CAPPED matches by
        // file for the "N matches in M files" header, and `overflow_head`/`format_overflow`
        // only echo `shown`/`total`/`hint` — neither touches `by_file` or
        // `by_file_overflow`. The only existing tests (`build_by_file_sorts_desc_and_caps_at_15`,
        // `build_by_file_no_overflow_under_cap`) call the pure `build_by_file` function
        // directly, not through Symbols's real call surface, so they say nothing about
        // what a caller actually sees. No marker is reachable today.
        id: "symbols.by_file",
        coverage: Coverage::Deferred(
            "BY_FILE_CAP's overflow count is computed and JSON-embedded at \
             $.overflow.by_file_overflow, but Symbols renders via OutputForm::Text and no \
             compact-text renderer surfaces by_file or by_file_overflow; the only tests \
             drive build_by_file directly, not the tool's real call surface",
        ),
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
        id: "preview.plan_headings",
        coverage: Coverage::Deferred(
            "Task 5a classified this Probed (marker JsonPath(\"$.headings_truncated\")) with \
             no comment citing a test; Task 5b's cited_test requirement found none named, \
             and inventing one is the exact failure this gate exists to prevent",
        ),
    },
    ProbeRow {
        id: "preview.plan_open_next",
        coverage: Coverage::Deferred(
            "Task 5a classified this Probed (marker JsonPath(\"$.tasks.open_next\")) with no \
             comment citing a test; Task 5b's cited_test requirement found none named, \
             and inventing one is the exact failure this gate exists to prevent",
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
        id: "doctor.exposure_threshold",
        coverage: Coverage::Deferred(
            "Task 5a classified this Probed (marker \
             JsonPath(\"$.summary.by_check.entry_conditional_past_due\")) with no comment \
             citing a test; Task 5b's cited_test requirement found none named, and \
             inventing one is the exact failure this gate exists to prevent",
        ),
    },
    ProbeRow {
        id: "doctor.caveat_chars",
        coverage: Coverage::Deferred(
            "Task 5a classified this Probed (marker TextContains(\"…\")) with no comment \
             citing a test; Task 5b's cited_test requirement found none named, and \
             inventing one is the exact failure this gate exists to prevent",
        ),
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
            mutation: Mutation::NotYet(NOT_MUTATED_YET),
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
            mutation: Mutation::NotYet(NOT_MUTATED_YET),
            cited_test: "a_neighbourhood_that_does_not_fit_whole_is_excerpted_rather_than_dropped",
        },
    },
    ProbeRow {
        id: "context.attestation_exposure",
        coverage: Coverage::Deferred(
            "Task 5a classified this Probed (marker \
             JsonPath(\"$.verification.verification_state\")) with no comment citing a \
             test; Task 5b's cited_test requirement found none named, and inventing one \
             is the exact failure this gate exists to prevent",
        ),
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
        id: "artifact.get_lines",
        coverage: Coverage::Deferred(
            "Task 5a classified this Probed (marker JsonPath(\"$.overflow.shown_lines\")) \
             with no comment citing a test; Task 5b's cited_test requirement found none \
             named, and inventing one is the exact failure this gate exists to prevent",
        ),
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
        id: "link_scan.findings",
        coverage: Coverage::Deferred(
            "Task 5a classified this Probed (marker \
             JsonPath(\"$.counts.truncated.dangling\")) with no comment citing a test; \
             Task 5b's cited_test requirement found none named, and inventing one is the \
             exact failure this gate exists to prevent",
        ),
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
        id: "prompts.client_instructions_chars",
        coverage: Coverage::Deferred(
            "Task 5a classified this Probed (marker TextContains(\"trimmed\")) with no \
             comment citing a test; Task 5b's cited_test requirement found none named, \
             and inventing one is the exact failure this gate exists to prevent",
        ),
    },
    ProbeRow {
        id: "prompts.trim_note_names",
        coverage: Coverage::Deferred(
            "Task 5a classified this Probed (marker TextContains(\"+2 more\")) with no \
             comment citing a test; Task 5b's cited_test requirement found none named, \
             and inventing one is the exact failure this gate exists to prevent",
        ),
    },
    // -- src/tools/grep.rs --
    ProbeRow {
        id: "grep.match_bytes",
        coverage: Coverage::Deferred(
            "Task 5a classified this Probed (marker TextContains(\"truncated\")) with no \
             comment citing a test; Task 5b's cited_test requirement found none named, \
             and inventing one is the exact failure this gate exists to prevent",
        ),
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
        id: "preview.default_headings",
        coverage: Coverage::Deferred(
            "Task 5a classified this Probed (marker JsonPath(\"$.headings_truncated\")) \
             with no comment citing a test; Task 5b's cited_test requirement found none \
             named, and inventing one is the exact failure this gate exists to prevent",
        ),
    },
    // -- src/librarian/preview/memory.rs --
    ProbeRow {
        id: "preview.observation_text",
        coverage: Coverage::Deferred(
            "Task 5a classified this Probed (marker TextContains(\"…\")) with no comment \
             citing a test; Task 5b's cited_test requirement found none named, and \
             inventing one is the exact failure this gate exists to prevent",
        ),
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
        id: "tracker_design.existing_trackers",
        coverage: Coverage::Deferred(
            "Task 5a classified this Probed (marker \
             JsonPath(\"$.existing_trackers_overflow_hint\")) with no comment citing a \
             test; Task 5b's cited_test requirement found none named, and inventing one \
             is the exact failure this gate exists to prevent",
        ),
    },
    // -- src/librarian/tools/workspace_state_at.rs --
    ProbeRow {
        id: "workspace_state_at.rows",
        coverage: Coverage::Deferred(
            "Task 5a classified this Probed (marker JsonPath(\"$.hints.more_in_scope\")) \
             with no comment citing a test; Task 5b's cited_test requirement found none \
             named, and inventing one is the exact failure this gate exists to prevent",
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
        id: "index_state.skipped_sample",
        coverage: Coverage::Deferred(
            "Task 5a classified this Probed (marker \
             JsonPath(\"$.last_sync_skipped.sample\")) with no comment citing a test; \
             Task 5b's cited_test requirement found none named, and inventing one is the \
             exact failure this gate exists to prevent",
        ),
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
        id: "command_summary.buffer_query_lines",
        coverage: Coverage::Deferred(
            "Task 5a classified this Probed (marker JsonPath(\"$.stdout_shown\")) with no \
             comment citing a test; Task 5b's cited_test requirement found none named, \
             and inventing one is the exact failure this gate exists to prevent",
        ),
    },
    // -- src/tools/run_command/output.rs --
    ProbeRow {
        id: "run_command.stderr_lines",
        coverage: Coverage::Deferred(
            "Task 5a classified this Probed (marker JsonPath(\"$.stderr_shown\")) with no \
             comment citing a test; Task 5b's cited_test requirement found none named, \
             and inventing one is the exact failure this gate exists to prevent",
        ),
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
];
