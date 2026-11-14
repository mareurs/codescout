# Artifact identity Plan C2: events and manual links get a committed text twin — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `event_create` and `doc(link)` write their fact into the committed markdown as well as the catalog, so Plan D can export old rows into the same format and Plan E can stop reading the rows.

**Architecture:** Dual-write, as in Plan C1. The catalog rows stay authoritative until Plan E. The file write comes first. A failed commit restores the file. A pure module owns each text format, so the writers, the Plan D export and the Plan E reader share one definition. A note on a shared document stays a row until codescout-37 builds its entry-document tool (operator ruling, 2026-11-14).

**Tech Stack:** Rust, `rusqlite`, `serde_json`, the existing `frontmatter`, `cite_text` and `dual_write` modules.

**Spec:** `docs/superpowers/specs/2026-11-11-repo-relative-artifact-key-design.md` (v5), § The store boundary, § Writes, § Left to the builder. Scout: `docs/research/2026-11-14-artifact-identity-plan-d-scout.md` (D-3). State: memory `artifact-identity-state`.

## Global Constraints

- Frontmatter holds scalars and lists of scalars. Every relation is a labelled grammar line in the body, for example `**Rests on:** [[key]]`. `link_scan` parses bodies, never frontmatter.
- Single-writer rule: a write tool appends to a section only in a document that one writer owns. Anything appended to a shared document becomes a document of its own. Plan C2 does not build that document. It keeps the row.
- No text that a writer adds to a file or to a tool response holds a hex id, an absolute path or a row id as a document reference. A target appears as its key.
- The catalog row is never lost, and never written without a reason when its text twin is missing. A skipped twin says why in the response.
- `TOOL_SURFACE_CHAR_BUDGET` is exactly 57_664 (headroom 0). No advertised description or schema may grow. A response field is not advertised text. A guide text change stays under its ceiling.
- Frontmatter key `single_writer` is not reserved by `lifecycle::entry::RESERVED`, but on a lifecycle entry file it would be a declared-field problem. So the writer never reads the flag on a document that has `entry_kind` or a declared `entry_prefix`.
- Relation labels: any column-0 line that matches `^\*\*([A-Z][A-Za-z ]*[a-z]):\*\*` (`lifecycle::relations::label_of`). The closed set is `BACKWARD_LABELS` = `Promoted from`, `Supersedes`. Plan C2 adds no label to it.
- Write plan, guide and tracker text in Simplified Technical English. Write trackers in English.
- Shared checkout rules: stage paths, run `git diff --cached --name-only` as its own call, commit by pathspec with `-m` before `--`.

## Rulings (controller; the operator may overrule any of them)

- **C2-R1** The home of a note, verdict, intent, reviewed or external_signal event is a section in a document that carries `single_writer: true`. Other documents keep the row only. Cost if wrong: a few documents carry the flag at first, and 73 events exist today.
- **C2-R2** An observation has no format of its own. Production code writes an observation only for a `note` event (`event_create`), so the note section is its text twin. Plan D exports an old observation as a `note` section.
- **C2-R3** `status_change` already writes the frontmatter field before the row. `field_patch` needs no text (git history is the log). `worktree_fork` and `worktree_merge` need none. Plan C2 changes none of these.
- **C2-R4** A manual link is one relation line in the source document, under a `## Relations` section at the end of the body. The label comes from the rel: words split on `_`, `-` and space, lowercased, joined with spaces, first letter capitalised. A rel that does not give a valid label (a digit, a trailing capital) keeps the row and skips the line. `doc(link)` with rel `supersedes` gives `**Supersedes:** [[dst-key]]`.
- **C2-R5** A link whose target the grammar cannot name (`grammar_form_of` refuses) keeps the row and skips the line. This differs from Plan C1, which refuses a cite. Reason: `doc(link)` joins arbitrary pairs, including documents outside the key space, and refusing would break calls that work today. Cost if wrong: rows without twins. Plan D's dry run lists them.
- **C2-R6** No doctor check in Plan C2. Plan D's dry run counts rows without a twin per class.
- **C2-R7** The derived index stores no relation label (`edge` has `kind`, `target_*`, `dst_key`). A label is read from text by `lifecycle::relations::relations`. Plan C2 does not change the derived schema.

## Review Focus

1. A note whose text holds a heading, a `**Cites:** [[x]]` line, a code fence, `-->` or CRLF line ends. The structure of the document must stay intact (Task 1).
2. A second event on one document. The `## Events` heading must appear once, and the order must stay (Task 1, Task 2).
3. The file write succeeds and the transaction commit fails. The file must return to its old bytes and no row may remain (Task 2). The file write fails (symlink, read-only). No row may remain (Task 2).
4. `single_writer: true` on a document that declares `entry_prefix` or has `entry_kind`. No text may be written (Task 2).
5. The same link made twice, and a link to a target outside the key space. No duplicate line. The row stays and the response says why the line was skipped (Task 3).

---

### Task 1: The event text format

**Files:**
- Create: `src/librarian/catalog/event_text.rs`
- Modify: `src/librarian/catalog/mod.rs` (declare `pub(crate) mod event_text;`)

**Interfaces:**
- Consumes: `crate::librarian::frontmatter::{parse, replace_body}`, `serde_json::Value`.
- Produces:
  - `pub(crate) const EVENTS_HEADING: &str = "## Events";`
  - `pub(crate) struct EventText { pub id: String, pub kind: String, pub created_at_ms: i64, pub author: Option<String>, pub anchor_commit: Option<String>, pub head_commit: Option<String>, pub parent: Option<String>, pub resolves: Option<String>, pub source: Option<(String, String)>, pub mutates: Vec<String>, pub payload: serde_json::Value }`. `mutates` holds grammar forms such as `[[key]]`.
  - `pub(crate) fn render_event_section(e: &EventText) -> String`
  - `pub(crate) fn parse_event_sections(doc: &str) -> Vec<EventText>`
  - `pub(crate) fn splice_event(doc: &str, e: &EventText) -> anyhow::Result<String>`: returns the whole document with the section added. It creates `## Events` at the end of the body when the heading is absent. It uses an existing `## Events` heading as the home. It adds the new `###` section at the end of that section, in call order. It refuses an `id` that is already present.

Format, exactly:

```
### note — 2026-11-14T06:30:00Z — <author or unknown>
<!-- event {"id":"…","kind":"note","created_at_ms":…,"author":…,…} -->

> first text line
> second text line
```

- The heading is for people. The parser reads the comment JSON only. The heading holds the kind in lowercase, so it never matches the ledger entry heading shape `<ID> — <title>`.
- The JSON is on one line. The writer replaces every `>` in it with `>`, so the payload cannot close the comment. The parser reads it back to the same value.
- A `note` whose payload is exactly `{"text": <string>}` writes the text as a blockquote: each line prefixed `> `, an empty line as `>`. The blockquote keeps a heading, a label line and a fence in the text from acting as structure.
- Any other payload writes one fenced block with the info string `event-payload` that holds the payload as pretty JSON. The fence is longer than any backtick run in the JSON.
- `mutates` writes one line `**Mutates:** [[a]] [[b]]` after the comment, only when the list is not empty.
- `source` is part of the comment JSON as `{"kind": …, "uri": …}`.

- [ ] **Step 1: Write the failing tests** in `event_text::tests`:
  - `a_note_round_trips_through_render_and_parse` (all fields set; equality after `parse_event_sections(splice_event(…))`).
  - `a_hostile_note_keeps_the_document_structure`: text `"# h\n**Cites:** [[x]]\n```\n-->\r\nend"`. Assert `lifecycle::relations::relations` of the whole document returns no relation, the document has exactly one `## ` heading and one `### ` heading, and the parsed text equals the input with `\r\n` kept.
  - `a_non_note_payload_is_a_fenced_block_and_round_trips` (payload with a nested ```` ``` ```` string).
  - `a_second_event_follows_the_first_under_one_events_heading`.
  - `an_existing_events_heading_is_the_home` and `a_duplicate_event_id_is_refused`.
  - `an_event_section_defines_no_ledger_entry`: run the derived parser (`derived::build`'s document parse, `Parser::PRODUCTION`) on a document that holds an event section and assert zero entries.
- [ ] **Step 2: Run** `scripts/with-slot.sh cargo test --lib event_text`. Expected: FAIL (module missing).
- [ ] **Step 3: Implement** the interfaces above in `event_text.rs`. Parse the comment JSON with `serde_json`. Use `frontmatter::replace_body` for the whole-document result when the document has frontmatter.
- [ ] **Step 4: Run** the same command. Expected: PASS.
- [ ] **Step 5: Commit** by pathspec: `feat(librarian): the event text format (render, parse, splice)`.

### Task 2: `event_create` writes the text twin file-first

**Files:**
- Modify: `src/librarian/tools/event_create.rs` (`call`, a new private `single_writer_target`)
- Modify: `src/librarian/catalog/dual_write.rs` only if a helper must be shared; prefer reuse of `record_written_file` and `restore_section_after_failed_commit` as they are.
- Test: `event_create::tests`

**Interfaces:**
- Consumes: Task 1 `EventText`, `splice_event`; `catalog::dual_write::{record_written_file, restore_section_after_failed_commit}`; `util::write_catalogued_file`; `cite_text::grammar_form_of(conn, source_git_root, dst_ref) -> Result<String>` for each `also_mutates` id.
- Produces: the response of `call` gains `"text_twin": "written"` or `"text_twin": "skipped: <reason>"`. The reasons are fixed strings: `not a single-writer document`, `kind has no text twin`, `ledger or entry document`, and `target not nameable: <refusal>` for a mutates target that `grammar_form_of` refuses.

Behaviour of `call`, after the rows are built and before `tx.commit()`:

1. A kind outside `note`, `verdict`, `intent`, `reviewed`, `external_signal` skips with `kind has no text twin`.
2. Read the document at the write target's `abs_path`. It is single-writer only when `frontmatter::parse` gives an `extra["single_writer"]` that equals `true`, the document declares no `entry_prefix`, and it has no `entry_kind`. Otherwise skip with the matching reason.
3. Build `EventText` from the event row and edges. Render each `also_mutates` id with `grammar_form_of`. A refusal skips the whole twin and the row stays (C2-R5 applies to events too).
4. Write the file with `write_catalogued_file`, then `record_written_file(&tx, …)`, both before `tx.commit()`. If the write fails, return the error. The transaction drops and no row remains.
5. If `tx.commit()` fails, call `restore_section_after_failed_commit` with the old and new text, and return the commit error.

The per-artifact write lock (`write_locks`) already covers the file read and the file write.

- [ ] **Step 1: Write the failing tests:**
  - `a_note_on_a_single_writer_document_gets_a_section_and_a_row`: create a real file with `single_writer: true`, an artifact row for it, call `event_create` with kind `note`. Assert the file holds the section, the row exists, `text_twin == "written"`.
  - `the_flag_is_set_through_doc_update_extra`: set the flag with `update` `patch={extra:{single_writer:true}}`, then repeat the call above.
  - `a_note_on_an_unflagged_document_keeps_only_the_row` (`text_twin` starts with `skipped:`, file bytes unchanged).
  - `a_flagged_ledger_or_entry_document_gets_no_text` (frontmatter with `entry_prefix`, and another with `entry_kind`).
  - `a_verdict_writes_a_fenced_payload_and_a_resolves_id`.
  - `a_failed_commit_restores_the_file`: arm the existing `INJECT_FAIL_AFTER_EVENT_INSERT` style hook, or a `commit_tripwire` table as `dual_write` tests do. Assert the old bytes and zero rows.
  - `a_failed_file_write_leaves_no_row` (target is a symlink; `refuse_write_through_symlink` refuses).
  - `two_notes_keep_one_events_heading_and_call_order`.
  - `a_mutates_target_outside_the_key_space_skips_the_twin_and_keeps_the_row`.
- [ ] **Step 2: Run** `scripts/with-slot.sh cargo test --lib event_create`. Expected: the new tests FAIL.
- [ ] **Step 3: Implement** the five behaviours in `call`. Move the `note` observation insert into the same transaction only if the existing test `a_note_event_also_creates_an_observation` (find it by name) stays green. Otherwise leave it where it is.
- [ ] **Step 4: Run** the same command. Expected: all PASS, including the old tests.
- [ ] **Step 5: Commit** by pathspec: `feat(librarian): event_create writes a single-writer document's note as a section, file first`.

### Task 3: `doc(link)` writes the relation line

**Files:**
- Create: `src/librarian/catalog/link_text.rs`
- Modify: `src/librarian/catalog/mod.rs`, `src/librarian/tools/link.rs` (`call`)
- Test: `link_text::tests`, `tools/link.rs` tests

**Interfaces:**
- Consumes: `cite_text::grammar_form_of`, `lifecycle::relations::{label_of, relations}`, `util::write_catalogued_file`, `dual_write::{record_written_file, restore_section_after_failed_commit}`.
- Produces:
  - `pub(crate) const RELATIONS_HEADING: &str = "## Relations";`
  - `pub(crate) fn label_for_rel(rel: &str) -> Result<String, String>`: the C2-R4 mapping. The error string says why the rel is not a label.
  - `pub(crate) fn splice_relation(doc: &str, label: &str, form: &str) -> Option<String>`: `None` when the identical line `**<label>:** <form>` is already in the `## Relations` section (idempotent). Else the whole document with the line added at the end of that section, after one blank line. It creates the heading at the end of the body when absent.
  - the response of `link::call` gains `"text_twin"` with the same two values as Task 2. Reasons: `rel is not a label: <why>`, `target not nameable: <refusal>`, `already present`.

Behaviour of `link::call`: after the destination frontmatter write (for `supersedes`) and before `links::insert`, compute the label and the form, splice the line, write the source file, and call `record_written_file`. If `links::insert` then fails, restore the file. A skipped twin never blocks the row.

- [ ] **Step 1: Write the failing tests:**
  - in `link_text::tests`: `rel_words_become_a_label` (`depends_on` → `Depends on`, `see-also` → `See also`, `implements` → `Implements`), `a_rel_with_a_digit_is_not_a_label`, `a_new_line_goes_under_one_relations_heading`, `the_same_line_twice_is_a_no_op`, and `lifecycle_relations_reads_the_written_line_back` (assert label and `Reference` equal the input).
  - in `link.rs` tests: `a_link_writes_a_relation_line_and_a_row`, `a_supersedes_link_writes_the_supersedes_line_and_keeps_the_frontmatter_write`, `a_link_to_a_target_outside_the_key_space_keeps_the_row_and_skips_the_line`, `the_same_link_twice_leaves_one_line`, `a_failed_row_insert_restores_the_source_file`.
- [ ] **Step 2: Run** `scripts/with-slot.sh cargo test --lib link_text` and `… cargo test --lib tools::link`. Expected: the new tests FAIL.
- [ ] **Step 3: Implement** `link_text.rs` and the `call` change.
- [ ] **Step 4: Run** the same commands. Expected: all PASS.
- [ ] **Step 5: Commit** by pathspec: `feat(librarian): doc(link) writes the relation line into the source document, file first`.

### Task 4: Guides, manual and probes

**Files:**
- Modify: `src/prompts/guides/librarian.md` (§ doc — Event Log, and the link text), `docs/manual/src/concepts/artifact-cli.md` or the page that names `event_create` and `doc(link)` (find with grep), `docs/PROBES.md` if a probe sentence says events or links live only in the catalog.

- [ ] **Step 1:** In the librarian guide, add the rule in two or three sentences: a document with `single_writer: true` keeps its notes, verdicts, intents, reviews and signals as `### <kind> — …` sections under `## Events`; a link is one `**Label:** [[key]]` line under `## Relations`; both are written before the row, and a skipped twin is named in `text_twin`. State the `single_writer` flag and that a ledger or an entry document ignores it.
- [ ] **Step 2:** Run the guide and prompt gates: `scripts/with-slot.sh cargo test --lib prompts`. If a guide ceiling fails, shorten the new text. Do not raise a ceiling.
- [ ] **Step 3:** Run `scripts/with-slot.sh cargo test --lib tool_surface` (the budget test; find its exact name with grep `TOOL_SURFACE_CHAR_BUDGET`). Expected: PASS with no change to the constant.
- [ ] **Step 4: Commit** by pathspec: `docs(librarian): single_writer events and relation lines`.

### Task 5: Gate

- [ ] **Step 1:** Run `./scripts/gate.sh` in the background on an untouched tree and end the turn. The wake hook reports the end.
- [ ] **Step 2:** Expected: nextest lean and default green, python green, fmt and clippy clean. Record the counts in the research doc `9bcc00c179ecbdce`'s successor for C2 (the controller writes it).

## Self-review (writing-plans checklist)

1. **Spec coverage.** Events (`note`, `verdict`, `intent`, `reviewed`, `external_signal`): Task 1 and 2. `status_change`, `superseded_by`, `field_patch`, worktree events: R3 and Task 3. Observations: R2. Manual links: Task 3. Write-time `cites`: done in Plan C1. The section format and the rel-label vocabulary (spec § Left to the builder): Task 1 and R4. The shared-document branch: R1, an operator ruling.
2. **Step scan.** Each step names the test, the file and the command. Bodies are left to the implementer, except the text formats, which the spec leaves to the builder and this plan fixes.
3. **Type consistency.** `EventText`, `splice_event`, `label_for_rel`, `splice_relation` and `text_twin` have one name each across tasks.
4. **Review Focus.** Each line has a named test: 1 → `a_hostile_note_keeps_the_document_structure`; 2 → `two_notes_keep_one_events_heading_and_call_order`; 3 → `a_failed_commit_restores_the_file`, `a_failed_file_write_leaves_no_row`; 4 → `a_flagged_ledger_or_entry_document_gets_no_text`; 5 → `the_same_link_twice_leaves_one_line`, `a_link_to_a_target_outside_the_key_space_keeps_the_row_and_skips_the_line`.
5. **Proportion.** Five tasks, about 140 lines of plan against a spec section of about 30 lines. The plan fixes two text formats, which is why it is longer.

## Facts I did not verify (the implementer must check before the step that needs it)

- The exact name of the existing test about the observation of a `note` event, and whether the observation insert can join the transaction.
- Whether `write_catalogued_file` refuses a worktree shadow path in the cases Task 2 tests. Plan C1's `merge_worktree` notes say shadow files exist for forked rows.
- The exact name of the tool-surface budget test.
- Whether `update` with `extra` accepts a boolean. The guide says custom keys round-trip. The test `the_flag_is_set_through_doc_update_extra` proves it.
