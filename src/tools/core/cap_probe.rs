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
    Probed { marker: Marker, mutation: Mutation },
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
        coverage: Coverage::Probed {
            marker: Marker::TextContains("@tool_"),
            mutation: Mutation::NotYet(NOT_MUTATED_YET),
        },
    },
    ProbeRow {
        id: "run_command.inline_bytes",
        coverage: Coverage::Probed {
            marker: Marker::JsonPath("$.truncated"),
            mutation: Mutation::NotYet(NOT_MUTATED_YET),
        },
    },
    ProbeRow {
        id: "tool_output.inline_byte_budget",
        coverage: Coverage::Probed {
            marker: Marker::TextContains("json_path"),
            mutation: Mutation::NotYet(NOT_MUTATED_YET),
        },
    },
    ProbeRow {
        id: "markdown.headings_hard_cap",
        coverage: Coverage::Probed {
            marker: Marker::JsonPath("$.file_id"),
            mutation: Mutation::NotYet(NOT_MUTATED_YET),
        },
    },
    ProbeRow {
        id: "tool_output.compact_summary_bytes",
        coverage: Coverage::Deferred(
            "the only candidate fixture drives content past both the soft cap and its \
             sibling hard cap in the same call, so no assertion isolates the soft cap \
             specifically from tool_output.compact_summary_hard_bytes",
        ),
    },
    ProbeRow {
        id: "tool_output.compact_summary_hard_bytes",
        coverage: Coverage::Probed {
            marker: Marker::TextContains("truncated"),
            mutation: Mutation::NotYet(NOT_MUTATED_YET),
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
        id: "symbols.by_file",
        coverage: Coverage::Probed {
            marker: Marker::JsonPath("$.by_file_overflow"),
            mutation: Mutation::NotYet(NOT_MUTATED_YET),
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
            "the only related test re-implements the greedy-capping loop locally instead \
             of calling list_overview, so a break in the production capping logic would \
             not fail this test",
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
        coverage: Coverage::Probed {
            marker: Marker::JsonPath("$.headings_truncated"),
            mutation: Mutation::NotYet(NOT_MUTATED_YET),
        },
    },
    ProbeRow {
        id: "preview.plan_open_next",
        coverage: Coverage::Probed {
            marker: Marker::JsonPath("$.tasks.open_next"),
            mutation: Mutation::NotYet(NOT_MUTATED_YET),
        },
    },
    ProbeRow {
        id: "preview.plan_task_text",
        coverage: Coverage::Deferred(
            "truncate_task_text appends no ellipsis or other marker, so no assertion can \
             distinguish a working cap from a removed one; the only candidate test's \
             assertion is monotone under cap removal",
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
        coverage: Coverage::Probed {
            marker: Marker::JsonPath("$.summary.by_check.entry_conditional_past_due"),
            mutation: Mutation::NotYet(NOT_MUTATED_YET),
        },
    },
    ProbeRow {
        id: "doctor.caveat_chars",
        coverage: Coverage::Probed {
            marker: Marker::TextContains("…"),
            mutation: Mutation::NotYet(NOT_MUTATED_YET),
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
        id: "format.shape_scalar_len",
        coverage: Coverage::Deferred(
            "no test drives a scalar value long enough to exercise the per-scalar length \
             cap; existing describe_payload_shape tests use short fixture values",
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
        // existing `glob_explosion_returns_recoverable` test already drives the cap
        // past its bound and asserts the message names "glob matched" and the count;
        // that assertion composes unmodified into `call()`'s response via `?`, so it
        // is real evidence through a shared primitive (`get_guide("error-handling")`).
        id: "audit_doc_refs.files",
        coverage: Coverage::Probed {
            marker: Marker::TextContains("glob matched"),
            mutation: Mutation::NotYet(NOT_MUTATED_YET),
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
        id: "context.max_tokens",
        coverage: Coverage::Deferred(
            "every context test supplies an explicit max_tokens override; none omit the \
             param to exercise the shipped default budget",
        ),
    },
    ProbeRow {
        id: "context.attestation_exposure",
        coverage: Coverage::Probed {
            marker: Marker::JsonPath("$.verification.verification_state"),
            mutation: Mutation::NotYet(NOT_MUTATED_YET),
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
        id: "artifact.get_lines",
        coverage: Coverage::Probed {
            marker: Marker::JsonPath("$.overflow.shown_lines"),
            mutation: Mutation::NotYet(NOT_MUTATED_YET),
        },
    },
    ProbeRow {
        id: "artifact.get_overflow_headings",
        coverage: Coverage::Deferred(
            "no test drives the top-level-heading list inside the overflow hint past its \
             own cap; existing overflow tests use a two-heading fixture",
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
        coverage: Coverage::Probed {
            marker: Marker::JsonPath("$.counts.truncated.dangling"),
            mutation: Mutation::NotYet(NOT_MUTATED_YET),
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
        id: "prompts.client_instructions_chars",
        coverage: Coverage::Probed {
            marker: Marker::TextContains("trimmed"),
            mutation: Mutation::NotYet(NOT_MUTATED_YET),
        },
    },
    ProbeRow {
        id: "prompts.trim_note_names",
        coverage: Coverage::Probed {
            marker: Marker::TextContains("+2 more"),
            mutation: Mutation::NotYet(NOT_MUTATED_YET),
        },
    },
    // -- src/tools/grep.rs --
    ProbeRow {
        id: "grep.match_bytes",
        coverage: Coverage::Probed {
            marker: Marker::TextContains("truncated"),
            mutation: Mutation::NotYet(NOT_MUTATED_YET),
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
        id: "preview.default_headings",
        coverage: Coverage::Probed {
            marker: Marker::JsonPath("$.headings_truncated"),
            mutation: Mutation::NotYet(NOT_MUTATED_YET),
        },
    },
    // -- src/librarian/preview/memory.rs --
    ProbeRow {
        id: "preview.observation_text",
        coverage: Coverage::Probed {
            marker: Marker::TextContains("…"),
            mutation: Mutation::NotYet(NOT_MUTATED_YET),
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
        id: "tracker_design.existing_trackers",
        coverage: Coverage::Probed {
            marker: Marker::JsonPath("$.existing_trackers_overflow_hint"),
            mutation: Mutation::NotYet(NOT_MUTATED_YET),
        },
    },
    // -- src/librarian/tools/workspace_state_at.rs --
    ProbeRow {
        id: "workspace_state_at.rows",
        coverage: Coverage::Probed {
            marker: Marker::JsonPath("$.hints.more_in_scope"),
            mutation: Mutation::NotYet(NOT_MUTATED_YET),
        },
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
        id: "lsp.first_call_budget",
        coverage: Coverage::Probed {
            marker: Marker::JsonPath("$.lsp"),
            mutation: Mutation::NotYet(NOT_MUTATED_YET),
        },
    },
    // -- src/retrieval/index_state.rs --
    ProbeRow {
        id: "index_state.skipped_sample",
        coverage: Coverage::Probed {
            marker: Marker::JsonPath("$.last_sync_skipped.sample"),
            mutation: Mutation::NotYet(NOT_MUTATED_YET),
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
        id: "command_summary.buffer_query_lines",
        coverage: Coverage::Probed {
            marker: Marker::JsonPath("$.stdout_shown"),
            mutation: Mutation::NotYet(NOT_MUTATED_YET),
        },
    },
    // -- src/tools/run_command/output.rs --
    ProbeRow {
        id: "run_command.stderr_lines",
        coverage: Coverage::Probed {
            marker: Marker::JsonPath("$.stderr_shown"),
            mutation: Mutation::NotYet(NOT_MUTATED_YET),
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
];
