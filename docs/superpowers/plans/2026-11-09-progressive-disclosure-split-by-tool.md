# progressive-disclosure split by overflowing tool Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** An overflow delivers the `progressive-disclosure` sections written for the overflowing tool, not the whole 13.3 KB guide.

**Architecture:** The guide gains `serves:` declarations, so `guide_blocks_for` takes its declaring branch: sections matching the call's shape are delivered once per ledger, keyed `topic#heading`; an unmatched shape (`doc`, `librarian`) gets the preamble. No change to the `overflowing` gate, to `Shape`, or to `guide_blocks_for`. Pull-only parts get `SECTION_WAIVERS` rows.

**Tech Stack:** Rust (codescout server, tests under `cargo test --lib`), Markdown guide, Python (no change).

**Spec:** `docs/superpowers/specs/2026-11-09-progressive-disclosure-split-by-tool-design.md` (commit `25017f806`). Bug: `7579b32b1cd2362f`. Recon of this plan: seams read 2026-11-09 (this plan's Global Constraints and Review Focus carry the findings).

## Global Constraints

- Declared sections are at most 2,500 B (`MAX_DECLARED_SECTION_BYTES`, enforced by `declared_sections_are_within_the_size_cap`).
- The `serves:` / `requires:` comment block sits directly under the heading, before the first blank line (`parse_declarations`). A section runs from its heading to the next heading of ANY level; a `###` child is its own section and its parent excludes it (`split_sections`).
- Core tool list, exactly: `run_command`, `grep`, `read_file`, `tree`, `semantic_search`, `symbols`, `references`, `call_graph`. Declare bare tool names (the default `selector_key` is the tool name, or `tool.action`).
- Do not declare `doc.*` or `librarian.*` shapes: `no_two_topics_declare_an_overlapping_shape` forbids it.
- Moved text keeps its wording. The sentence `or in strings the clip stops cutting at 1,000 raw bytes each` stays verbatim (`src/tools/core/tests.rs:1832` cites it).
- The heading `## Path-relative annotation` stays (cited by `workspace-state.md:87`, `src/server.rs:317`, `docs/conventions/cross-machine-catalog-resume.md:462`).
- The whole-body pins `progressive_disclosure_guide_states_the_foreground_pipe_capture`, `..._states_the_background_job_recipe` and `..._points_at_iron_laws_detail_for_the_pipe_capture_mechanics` must pass unchanged. Both new background headings start with `## Background jobs`.
- Shared checkout: commit by pathspec; format with the copy-and-diff method (`rustfmt --edition 2021 < f > copy; diff f copy`), never `cp` over a file that holds a peer's changes; run `./scripts/gate.sh` before completing.

## Review Focus

1. A re-armed ledger (`/compact`, resume) must re-deliver the overflow sections: `re_arm` clears `topic#heading` keys. Pinned in Task 1.
2. A persisted ledger that already holds the bare key `progressive-disclosure` (from before the split) must NOT suppress the sections: accepted one-time double delivery, pinned so nobody "fixes" it by accident. Pinned in Task 1.
3. A tool that routes here but declares nothing (an overflow from `doc`) must get the preamble with a usable handle paragraph, never silence and never the whole guide. Pinned in Task 1.
4. A non-overflowing call must still deliver nothing (the `overflowing` gate is untouched). Existing test `progressive_disclosure_is_withheld_when_nothing_overflowed` must keep passing.
5. An explicit `get_guide("progressive-disclosure")` must still return every section, pull-only ones included. Pinned in Task 1.

---

### Task 1: Declare the sections and prove each delivery

**Files:**
- Modify: `src/prompts/guides/progressive-disclosure.md`, `src/prompts/mod.rs` (`SECTION_WAIVERS`), `src/prompts/guide_index.rs` (one test), `src/engines/emitters.rs` (tests), `src/tools/core/guide_emit.rs` (tests), `src/tools/guide.rs` (one test), `src/server.rs` (one test; one stale doc paragraph)
- Test: the same files' `#[cfg(test)]` modules

**Interfaces:**
- Consumes: `guide_blocks_for(topic: &str, selector: Option<&str>, result: &Value, emitted: &mut GuideLedger) -> (Vec<Content>, Option<GuideDeliveryShape>)` (`guide_emit.rs`); `GUIDE_INDEX.match_sections`, `GUIDE_INDEX.declares`; `GuideLedger::re_arm(&mut self, &[&str], RearmCause)`, `GuideLedger::insert(String)`, `GuideLedger::contains(&str)`.
- Produces: a declaring `progressive-disclosure`. Section headings later tasks and tests name: `Output budgets`, `Reading a buffer`, `Reading the stderr of a buffered command: the `.err` suffix`, `Background jobs: start, end the turn, read after the wake`, `Background jobs: when no wake arrives, and waiting on another process`; pull-only: `Buffer internals`, `Path-relative annotation`, `Anti-patterns`, `Related`.

- [ ] **Step 1: Write the failing delivery tests** in `guide_emit.rs` `tests`, each with `let mut led = GuideLedger::default();` and the real index. Text of a block is `c.as_text().unwrap().text`. Let `TOPIC = "progressive-disclosure"`; let `markers(blocks)` return the `§ <heading>` of each block.
  - `an_overflow_from_grep_ships_the_two_core_sections_only`: `guide_blocks_for(TOPIC, Some("grep"), &json!({}), &mut led)` returns shape `Section` and exactly the sections `Output budgets` and `Reading a buffer`; no block text contains `Background jobs` or `.err` suffix.
  - `an_overflow_from_run_command_ships_the_core_stderr_and_background_sections`: with selector `Some("run_command")` the five declared headings above are delivered, none of `Buffer internals`, `Path-relative annotation`, `Anti-patterns`.
  - `a_later_grep_overflow_after_run_command_ships_nothing`: after the `run_command` call on the same ledger, `grep` returns an empty block list and `None`.
  - `an_overflow_from_doc_ships_the_preamble_with_the_handle_paragraph`: selector `Some("doc.get")` returns shape `Preamble`; the block text contains `output_id`, `read_file(`, `grep PATTERN @cmd_`, and `get_guide("progressive-disclosure")`; it is under 700 B.
  - `every_core_tool_matches_a_core_section`: for each of the eight core tools, `GUIDE_INDEX.match_sections(TOPIC, Some(tool), &json!({}))` is non-empty.
  - `a_re_armed_ledger_redelivers_the_overflow_sections`: deliver for `grep`, `led.re_arm(&[TOPIC], RearmCause::PostCompact)`, deliver again: non-empty. (If `re_arm` does not clear `topic#heading` keys, this fails and Review Focus 1 is a defect: stop and rule.)
  - `a_bare_topic_key_from_before_the_split_does_not_suppress_the_sections`: `led.insert(TOPIC.to_string())` first, then `grep` delivery is non-empty.
  - `the_overflow_deliveries_stay_within_their_byte_budgets`: sum of `as_text().text.len()` over the `grep` delivery and over the `run_command` delivery, each asserted `<=` a constant you set in Step 6 from the measured value plus 5%, and `grep` total `* 3 <` the whole-body wrapped size from `guide_block(TOPIC)`. In Step 1 use placeholders `usize::MAX`; Step 6 replaces them.
  Add to `src/tools/guide.rs` tests `get_guide_returns_every_section_of_progressive_disclosure`: call `get_guide` with `{"topic": "progressive-disclosure"}` and assert the returned `body` contains each of the nine headings (`## Output budgets`, `## Reading a buffer`, `## Buffer internals`, `.err` suffix, both `## Background jobs`, `## Path-relative annotation`, `## Anti-patterns`, `## Related`).
  In `src/engines/emitters.rs` replace `progressive_disclosure_ships_when_the_response_overflowed`'s `Whole` assertion: with selector `Some("t.a")` the shape is `GuideDeliveryShape::Preamble`; add `an_overflow_from_grep_is_delivered_as_sections` with `Some("grep")`, `content_topic = Some("progressive-disclosure")`, `overflowing = true`, asserting shape `Section` and hint topic `progressive-disclosure`.
  In `src/prompts/guide_index.rs` change `a_topic_without_declarations_reports_false` to use `error-handling` (confirm it declares nothing) and add the assertion `assert!(idx.declares("progressive-disclosure"))` in a new test `progressive_disclosure_declares_sections`.

- [ ] **Step 2: Run the new tests to verify they fail**

  Run: `scripts/with-slot.sh cargo test --lib -- an_overflow_from a_later_grep a_re_armed_ledger a_bare_topic_key every_core_tool the_overflow_deliveries get_guide_returns_every progressive_disclosure_declares an_overflow_from_grep_is_delivered`
  Expected: the delivery tests FAIL because the topic does not declare (shape `Whole`, one block); `get_guide_returns_every_section...` FAILS on the missing `## Reading a buffer` and `## Buffer internals`.

- [ ] **Step 3: Restructure `progressive-disclosure.md`** (wording of moved text unchanged):
  - Preamble (before the first `##`): the H1, one intro sentence, then a short paragraph of at most 450 B in total that says: a result over the inline budget is stored server-side; the response is an envelope with an `output_id` handle (`@tool_*`, `@cmd_*`, `@file_*`, `@bg_*`); read it with `read_file("@tool_xyz", json_path=...)`, `grep PATTERN @cmd_abc` or line ranges; do not re-run the tool.
  - `## Output budgets`: unchanged; add `<!-- serves: run_command, grep, read_file, tree, semantic_search, symbols, references, call_graph -->` on the line directly under the heading.
  - Rename `## The @ref buffer` to `## Reading a buffer`, same `serves:` line; it ends after the paragraph that closes with `grep -o 'PATTERN[^,]*'`.
  - New heading `## Buffer internals` (no `serves:`) before the paragraph that begins `**Reading a buffer never makes another buffer.**`; it takes everything up to, not including, the `###` stderr heading.
  - `### Reading the stderr of a buffered command: the .err suffix`: unchanged text, add `<!-- serves: run_command -->`.
  - `## Background jobs: start, end the turn, read after the wake`: add `<!-- serves: run_command -->`; it ends after step 3. Before the paragraph `**No wake arrives**` insert `## Background jobs: when no wake arrives, and waiting on another process` with `<!-- serves: run_command -->`.
  - `## Path-relative annotation`, `## Anti-patterns`, `## Related`: unchanged, no `serves:`.
  - Measure each declared section; any over 2,500 B moves its cut earlier.

- [ ] **Step 4: Add four `SECTION_WAIVERS` rows** in `src/prompts/mod.rs` for topic `progressive-disclosure`, headings `Buffer internals`, `Path-relative annotation`, `Anti-patterns`, `Related`. Each reason is over 40 characters and says why the part is pull-only (reachable through `get_guide`, not tied to one tool's overflow; for `Path-relative annotation` also that three places cite it as canonical). Fold `## Related` into the preamble instead if you prefer one row fewer.

- [ ] **Step 5: Run the new tests and every gate that reads the guide index**

  Run: `scripts/with-slot.sh cargo test --lib -- guide_index guide_emit emitters prompts::tests guide_hint_tests every_observed_shape every_declaring_topic every_section_of_a_declaring_topic no_two_topics declared_sections_are_within every_guide_topic_is_triggered progressive_disclosure get_guide`
  Expected: all PASS, including Gates 2, 5, 6, 7, the collision test, the size cap, the three whole-body pins and `run_command_with_overflow_emits_progressive_hint_once`. If any existing test fails, read it and decide: update only if it asserts whole delivery for this topic, and ledger the ruling.

- [ ] **Step 6: Measure and pin the byte budgets.** Run the budget test with `--nocapture` after temporarily printing the two totals (remove the print), set the two constants to measured `* 1.05` rounded up, and re-run. Record the measured bytes for the issue.

- [ ] **Step 7: Add the end-to-end test** `a_run_command_overflow_ships_sections_not_the_whole_guide` in `src/server.rs` `guide_hint_tests`, modelled on `run_command_with_overflow_emits_progressive_hint_once`: the overflowing result text contains `auto-injected get_guide('progressive-disclosure') § Output budgets` and `§ Reading a buffer`, contains `§ Background jobs`, and does not contain `## Path-relative annotation`. Run it.

- [ ] **Step 8: Correct the stale paragraph** in `src/server.rs` above `every_declaring_topic_has_a_live_route_to_a_declared_section` (the doc comment that says tools routing to `progressive-disclosure` return `None` for `selector_key`): say that since `30b6fc41` the default `selector_key` is the tool name or `tool.action`, and that `progressive-disclosure` now declares sections reachable through it.

- [ ] **Step 9: Format, stage, review the index, commit**

```bash
git add src/prompts/guides/progressive-disclosure.md src/prompts/mod.rs src/prompts/guide_index.rs src/engines/emitters.rs src/tools/core/guide_emit.rs src/tools/guide.rs src/server.rs
git diff --cached --name-only
git commit -m "feat(guides): progressive-disclosure ships the sections for the overflowing tool (bug 7579b32b1cd2362f)" -- src/prompts/guides/progressive-disclosure.md src/prompts/mod.rs src/prompts/guide_index.rs src/engines/emitters.rs src/tools/core/guide_emit.rs src/tools/guide.rs src/server.rs
```

### Task 2: Gate, measure, record

**Files:**
- Modify: `docs/issues/2026-08-27-guide-topics-are-atomic-nodes-in-an-unmodelled-graph.md` (via `doc(update, body_edits)`), `docs/superpowers/specs/2026-11-09-progressive-disclosure-split-by-tool-design.md` (status line)

- [ ] **Step 1: Run the gate**

  Run: `./scripts/gate.sh` in the background.
  Expected: `GATE EXITS -> FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0`. A red from a peer's uncommitted file is reported by name and re-run, not fixed here.

- [ ] **Step 2: Compute the scenario with real sizes.** From the delivered block sizes measured in Task 1 Step 6 and the census's trigger split (`scratchpad/pd_sections.py` numbers: subagents `run_command` 638, others 279; parents `run_command` 204, others 129), compute the saving over the census against the 13.7 MB delivered, and state the assumption that overflows from `doc`/`librarian` get the preamble only.

- [ ] **Step 3: Record.** Add a section to the issue with the measured per-tool delivery bytes, the computed saving, the commit, the gate result, the deferred items (explicit `get_guide` stamps only the bare key; one-time double delivery; effect on behaviour unmeasured; revisit when buffer-misuse errors rise), and set the spec status to built. Commit by pathspec.
