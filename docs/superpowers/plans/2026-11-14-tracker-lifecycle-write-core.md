---
id: '65d71884a8e84758'
kind: plan
status: done
title: Tracker Lifecycle Write Core — Implementation Plan
---

# Tracker Lifecycle Write Core Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `src/librarian/lifecycle/write.rs`: pure functions that render a new entry file, change an entry's status, and change an entry's declared fields, every result passing the lifecycle reader.

**Architecture:** One new module, no I/O and no catalog. It reuses `frontmatter::write` and `frontmatter::rewrite_frontmatter_normalizing` for YAML, `naming::{slug, entry_stem, entry_rel_path}` for the path, and `entry::{parse_entry, validate_entry}` as the safety net that re-reads every result.

**Tech Stack:** Rust, `chrono::NaiveDate`, `serde_json::Value`, `thiserror`.

**Spec:** `docs/superpowers/specs/2026-11-14-lifecycle-write-core-design.md` (id `0904b912ffaffdf6`), including its "Calls made" 1 to 8. Parent: `docs/superpowers/specs/2026-11-11-tracker-lifecycle-design.md` § D4, § Doctor checks, § Tools.

## Global Constraints

- Pure: no file, catalog or clock access. `today: NaiveDate` is a parameter.
- YAML is written only through `crate::librarian::frontmatter::{write, rewrite_frontmatter_normalizing}`. No second writer.
- **Every `Ok` text is re-read**: it passes `frontmatter::parse`, `entry::parse_entry` and `entry::validate_entry`, including a document returned unchanged. A document that is not an entry, or a result `parse_entry` rejects, is `Invalid(vec![EntryProblem::Unparsed(errors)])`; any other problem from `validate_entry` is `Invalid(problems)`.
- A new entry file carries `kind: entry` (typed `Frontmatter.kind`) and no `id`.
- Status rules by target class (spec calls 1 to 4): writing the current status sets `status_since = today`. `closed` is present iff the target class is terminal: kept when the entry is terminal, already has `closed` and the target is terminal; `today` for any other terminal target (including a terminal entry that lacks `closed`, which is repaired); removed for a non-terminal target. A blocked or deferred target needs `revisit_by` (the call's value wins over the stored one; neither gives `Invalid` holding `RevisitMissing`). An open or terminal target drops the stored `revisit_by`; a value passed for it is `RevisitNotNeeded(status)`, and so is `NewEntry.revisit_by` on an open or terminal status.
- The template ends at the next line starting `## ` outside a code fence; lines are read with `.lines()` (spec call 5).
- A title must be one line: an empty title, one with no slug, or one containing `\n` or `\r` is `Title(title)` (call 7).
- A reserved name is never a field: `frontmatter::RESERVED_KEYS` plus `entry::RESERVED` (made `pub(crate)` in Task 2) give `FieldUndeclared(name)`; so does any name the schema does not declare, in `new_entry` and in `change_fields` alike (call 8). The one exception: `change_fields` may delete (`None` or `Some(null)`) an undeclared, non-reserved key that the document already carries, so a stray key can be repaired.
- The frontmatter is rewritten by the normaliser: values survive by value (a hand-written `'high'` comes back as `high`) and the body byte for byte; key order, list style and CRLF frontmatter do not (call 6).
- `rustfmt --edition 2021 <file>` formats lifecycle files; `./scripts/fmt-mine.sh` may refuse files it cannot attribute to the session. A constant whose name contains CAP, LIMIT, MAX, BUDGET or THRESHOLD needs a `// cap-class:` comment; this plan adds none.
- Keep `write.rs` below 600 lines; tests live in `src/librarian/lifecycle/write/tests.rs` (`#[cfg(test)] mod tests;` in `write.rs`, as `checks.rs` does).
- **Test fixture** (a local `fn schema()` and `fn name_schema()` in `write/tests.rs`, built with `testkit::fm` and `schema::parse_schema(Lifecycle::Ledger, ..)`): `title: F`, `entry_kinds: [friction, note]`, `statuses: {open: open, waiting: blocked, parked: deferred, fixed: terminal, wontfix: terminal}`, `initial_status: open`, `key_style: dated` (`name` in `name_schema`), `fields: {severity: {type: enum, values: [low, high]}}`. If `parse_schema` reports a missing key, add it. Test documents come from `new_entry` with this schema, not from `testkit::entry`.
- Value assertions: a test that says "clean" also asserts the specific value it set (`status`, `revisit_by`, `tags`, `valid`, ...), because the re-read makes "validates clean" true of any Ok.

## Review Focus

1. A title with no slug (`"???"`, whitespace, `"日本語"`) or with a line break gives `Title`. Task 2.
2. A title with a colon, quote or leading `#` reads back identical. Task 2.
3. A template that starts with `---` stays body text. Task 2.
4. A template with `## ` inside a code fence, and one in a CRLF readme. Task 1.
5. A CRLF document, a document with no frontmatter, and one whose frontmatter is not an entry, given to `change_status`. Task 3.
6. A declared field named like a reserved key is refused, in both functions. Tasks 2 and 4.
7. A `name`-style collection with the title `readme` (`Stem(ReservedName)`) and one that slugs to a hex-shaped key. Task 2.
8. Each status transition and a hand-reopened entry that still has `closed`. Task 3.
9. Unknown extra keys and the body survive `change_status` and `change_fields`. Tasks 3 and 4.

---

### Task 1: The module, `WriteProblem` and `entry_template`

**Files:**
- Create: `src/librarian/lifecycle/write.rs`, `src/librarian/lifecycle/write/tests.rs`
- Modify: `src/librarian/lifecycle/mod.rs` (register `pub mod write;` beside its siblings)

**Interfaces:**
- Produces: `pub enum WriteProblem { KindUndeclared(String), StatusUndeclared(String), Title(String), Stem(StemError), BadYaml(String), NoFrontmatter, FieldUndeclared(String), RevisitNotNeeded(String), Invalid(Vec<EntryProblem>) }` deriving `Debug, Clone, PartialEq, Eq, thiserror::Error` (as `EntryProblem` does), each `#[error]` text naming the offending word, title or field.
- Produces: `pub fn entry_template(readme_body: &str) -> Option<String>`.
- Consumes: `crate::util::markdown_fence::FenceState` (`new`, `feed`, `in_fence`), used as `relations.rs` does.

- [ ] **Step 1: Write the failing tests** in `write/tests.rs`:
  - `the_template_is_the_section_up_to_the_next_h2`: a readme body with `## Intro`, `## Entry template` (two paragraphs and a `### Sub` heading with text), `## After` gives exactly the two paragraphs and the `### Sub` block, trimmed.
  - `the_template_is_none_when_absent_or_empty`: no such heading gives `None`; the heading followed by blank lines and then `## Next` gives `None`.
  - `a_fenced_heading_neither_starts_nor_ends_the_section`: a `## Entry template` line inside a ``` fence is ignored; a `## Other` line inside a fence inside the real section does not cut it.
  - `the_heading_must_be_exactly_level_two`: `### Entry template`, `## Entry templates` and `##Entry template` do not match.
  - `a_crlf_readme_body_works`: the first test's body with `\r\n` line endings gives the same template text.
- [ ] **Step 2: Run to verify they fail:** `scripts/with-slot.sh cargo test --lib lifecycle::write -- --nocapture`. Expected: compile error, `entry_template` not found.
- [ ] **Step 3: Implement** `WriteProblem` and `entry_template` in `write.rs`. Walk `.lines()` with `FenceState`; the section is the lines after the exact line `## Entry template` until the next line starting `## ` outside a fence.
- [ ] **Step 4: Run the tests, expect PASS.**
- [ ] **Step 5: Format and commit.** `rustfmt --edition 2021 src/librarian/lifecycle/write.rs src/librarian/lifecycle/write/tests.rs`, then `git add` those files and `mod.rs`, then commit `feat(librarian): lifecycle write core — WriteProblem and entry_template`.

### Task 2: `new_entry`

**Files:**
- Modify: `src/librarian/lifecycle/write.rs`, `src/librarian/lifecycle/write/tests.rs`, `src/librarian/lifecycle/entry.rs` (`const RESERVED` becomes `pub(crate)`)

**Interfaces:**
- Consumes: Task 1's `WriteProblem`; `schema::CollectionSchema`; `naming::{slug, entry_stem, entry_rel_path}`; `entry::{parse_entry, validate_entry, RESERVED}`; `frontmatter::{Frontmatter, write, render_body, parse, RESERVED_KEYS}`; `identity::key::Key`.
- Produces:
  - `pub struct NewEntry<'a> { pub entry_kind: &'a str, pub title: &'a str, pub status: Option<&'a str>, pub tags: Vec<String>, pub valid: Option<&'a str>, pub revisit_by: Option<NaiveDate>, pub fields: BTreeMap<String, serde_json::Value>, pub body: Option<&'a str> }`
  - `pub struct EntryFile { pub rel_path: String, pub key: Key, pub text: String }`
  - `pub fn new_entry(schema: &CollectionSchema, collection_dir: &str, template: Option<&str>, input: &NewEntry<'_>, today: NaiveDate) -> Result<EntryFile, Vec<WriteProblem>>`

- [ ] **Step 1: Write the failing tests** (fixture as in Global Constraints; a helper `input(kind, title)` builds a `NewEntry` with defaults):
  - `a_new_entry_reads_back_clean`: `new_entry(.., "docs/trackers/ledgers/f", None, &input("friction", "Wrong default"), d("2026-11-14"))` gives `rel_path == "docs/trackers/ledgers/f/2026-11-14-wrong-default.md"`, `key.as_str() == "2026-11-14-wrong-default"`, and the text parses with `frontmatter::parse`; `parse_entry` is `Ok` with `status == "open"`, `title == "Wrong default"`, `created == status_since == 2026-11-14`, `closed == None`, `revisit_by == None`; `validate_entry` is empty.
  - `the_file_carries_kind_entry_and_no_id`: the parsed `Frontmatter` has `kind == Some("entry")` and `id == None`.
  - `tags_valid_and_fields_are_written`: `tags: ["a","b"]`, `valid: Some("dated 2026-11-14")`, `fields {severity: "high"}` read back through `parse_entry` as `tags == ["a","b"]`, `valid == Some(Validity::Dated("2026-11-14".into()))` (as `entry.rs`'s `valid_is_read_through_parse_validity_and_kept_out_of_fields` asserts), `fields["severity"] == "high"`.
  - `a_terminal_status_sets_closed_today`: `status: Some("fixed")` gives `closed == Some(2026-11-14)`.
  - `an_undeclared_kind_or_status_is_refused_naming_the_word`: `Err` contains `WriteProblem::KindUndeclared("bug")` and `WriteProblem::StatusUndeclared("nope")` (the `WriteProblem` variants, not the `EntryProblem` ones), both in one call when both are wrong, and no `Invalid` problem beside them.
  - `a_blocked_or_deferred_status_needs_revisit_by`: for `waiting` and for `parked`, no `revisit_by` gives `Err` holding `Invalid` with `RevisitMissing`; with `revisit_by: Some(2026-12-01)` the entry reads back with `revisit_by == Some(2026-12-01)`.
  - `a_revisit_by_on_an_open_or_terminal_status_is_refused`: `revisit_by` with `open` and with `fixed` gives `RevisitNotNeeded("open")` / `RevisitNotNeeded("fixed")`.
  - `the_body_is_the_input_else_the_template_else_empty`: three calls; the body from `frontmatter::parse` equals `render_body` of the input body, of the template, and of the empty string.
  - `a_title_that_is_not_one_line_with_a_slug_is_a_title_problem`: `"???"`, `"   "`, `"日本語"`, `""`, `"two\nlines"` and `"two\rlines"` each give `Title(..)` holding the title.
  - `a_hard_title_reads_back_identical`: title `Fix: "quote" # hash` reads back through `parse_entry` as the identical string.
  - `a_template_starting_with_a_fence_stays_body`: template `---\nnot frontmatter\n---\ntext` gives a text whose `frontmatter::parse` body still contains that block and whose `parse_entry` is `Ok`.
  - `a_name_style_collection_refuses_the_reserved_stem_and_a_hex_shaped_one`: with `name_schema()`, title `readme` gives `Stem(ReservedName(..))`; a title that slugs to 16 lowercase hex digits (build it from the ten digits `0123456789` followed by `abcdef`) gives `Stem(BadKey(..))`.
  - `fields_are_declared_typed_and_never_reserved`: `severity: "medium"` gives `Invalid` holding `FieldType`; an undeclared `colour` gives `FieldUndeclared("colour")`; a field named `created`, and one named `status` that the schema declares (insert it into `schema.fields` by hand), each give `FieldUndeclared(..)`.
  - `a_valid_that_names_no_class_is_invalid`: `valid: Some("forever")` and `Some("dated 2026-02-30")` each give `Err` holding `Invalid` with `EntryProblem::Unparsed(..)`.
- [ ] **Step 2: Run, expect FAIL** (`new_entry` not found).
- [ ] **Step 3: Implement** `new_entry`. Make `entry::RESERVED` `pub(crate)`. Collect every problem from the checks before rendering, and render and re-read only when those checks found nothing (a bad status has no class to decide `closed` from). Order: kind, status (default `initial_status`), `revisit_by` against the status class, title (empty, line break, no slug), stem, field names (declared, not reserved), then render and run the re-read check. Put `entry_kind`, `status_since`, `created`, `closed`, `revisit_by`, `valid` and the declared fields into `Frontmatter.extra`; `title`, `status`, `tags` and `kind` into their typed fields; use `frontmatter::render_body` for the body. A `parse_entry` failure of the rendered text is `Invalid(vec![EntryProblem::Unparsed(errors)])`.
- [ ] **Step 4: Run, expect PASS.**
- [ ] **Step 5: Format and commit** `feat(librarian): lifecycle write core — new_entry`.

### Task 3: `change_status`

**Files:**
- Modify: `src/librarian/lifecycle/write.rs`, `src/librarian/lifecycle/write/tests.rs`

**Interfaces:**
- Consumes: Tasks 1 and 2.
- Produces: `pub fn change_status(doc: &str, schema: &CollectionSchema, new_status: &str, revisit_by: Option<NaiveDate>, today: NaiveDate) -> Result<String, Vec<WriteProblem>>`.

- [ ] **Step 1: Write the failing tests** (documents come from `new_entry` with the fixture schema; call the dates `d0 = 2026-11-01` for creation and `d1 = 2026-11-14` for `today`):
  - `rewriting_the_same_status_advances_status_since`: an open entry created on `d0`, `change_status(.., "open", None, d1)` gives `status_since == d1`, `closed == None`, and every other `EntryMeta` field equal to before.
  - `closing_sets_status_since_and_closed`: `open` to `fixed` gives `status == "fixed"`, `status_since == d1`, `closed == Some(d1)`.
  - `terminal_to_terminal_keeps_closed`: `fixed` (closed `d0`) to `wontfix` at `d1` gives `status_since == d1` and `closed == Some(d0)`; the same-status write on a terminal entry also keeps `closed`.
  - `reopening_removes_closed`: `fixed` to `open` gives `closed == None`, `status_since == d1`.
  - `a_hand_reopened_entry_is_repaired`: a document that is `open` but still carries `closed` (build it by editing the text) with `change_status(.., "open", ..)` gives `closed == None` and returns `Ok`.
  - `a_blocked_or_deferred_target_needs_revisit_by_and_the_call_wins`: `waiting` and `parked` without an argument or a stored value give `Err` holding `Invalid` with `RevisitMissing`; with the argument the result reads back `revisit_by == argument`; an entry that already has `revisit_by = X` moving to `parked` with `None` keeps `X`; with `Some(Y)` it becomes `Y`.
  - `an_open_or_terminal_target_drops_the_stored_revisit_by_and_refuses_an_argument`: `waiting` (revisit `X`) to `open` gives `revisit_by == None`; `waiting` to `fixed` gives `revisit_by == None` and `closed == Some(d1)`; `change_status(.., "open", Some(Y), ..)` gives `RevisitNotNeeded("open")` and `change_status(.., "fixed", Some(Y), ..)` gives `RevisitNotNeeded("fixed")`.
  - `a_terminal_entry_without_closed_is_repaired`: take a `new_entry` document with status `fixed`, delete its `closed:` line, write `wontfix` at `d1`: the result has `closed == Some(d1)` and `status_since == d1`.
  - `an_undeclared_word_is_refused`: `Err` contains `WriteProblem::StatusUndeclared("nope")`.
  - `no_frontmatter_bad_yaml_and_a_non_entry_are_named`: `"just text"` gives `NoFrontmatter`; `"---\nkind: [unclosed\n---\nbody\n"` (the input `frontmatter.rs`'s `rejects_malformed_yaml` proves fails) gives `BadYaml(_)`; frontmatter without `created` gives `Invalid` holding `EntryProblem::Unparsed(..)`.
  - `an_invalid_input_is_not_returned_as_ok`: a `new_entry` document with `entry_kind: friction` replaced by `entry_kind: bug` in the text, given its own status, gives `Err` holding `Invalid` with `KindUndeclared(..)`; the call does not return it unchanged.
  - `a_crlf_document_works`: the same open document with `\r\n` line endings gives `Ok`, `status == "fixed"`, and a body byte-identical to before.
  - `body_and_unknown_keys_survive`: the body is byte-identical before and after; an unknown extra key keeps its value.
- [ ] **Step 2: Run, expect FAIL.**
- [ ] **Step 3: Implement** with `rewrite_frontmatter_normalizing`. Run the `NoFrontmatter` check before it (it inserts a frontmatter when none exists). Parse the document with `frontmatter::parse` and `parse_entry` first, decide the edits by the target class, apply them, then re-read the result and return `Invalid` on any problem. A refusal returns before any edit is applied.
- [ ] **Step 4: Run, expect PASS.**
- [ ] **Step 5: Format and commit** `feat(librarian): lifecycle write core — change_status`.

### Task 4: `change_fields`

**Files:**
- Modify: `src/librarian/lifecycle/write.rs`, `src/librarian/lifecycle/write/tests.rs`

**Interfaces:**
- Consumes: Tasks 1 to 3.
- Produces: `pub fn change_fields(doc: &str, schema: &CollectionSchema, edits: &BTreeMap<String, Option<serde_json::Value>>) -> Result<String, Vec<WriteProblem>>`. `Some(v)` sets a declared field; `None` and `Some(Value::Null)` delete it.

- [ ] **Step 1: Write the failing tests:**
  - `a_declared_field_is_set_and_deleted`: set `severity: "high"` and read back `fields["severity"] == "high"`; delete it with `None` and with `Some(Value::Null)` and read back no `severity`.
  - `an_undeclared_field_is_refused_naming_it`: `FieldUndeclared("colour")`.
  - `a_reserved_key_is_refused_here`: an edit of `status`, `closed` and `title` each gives `FieldUndeclared(..)`.
  - `a_wrongly_typed_value_is_invalid`: `severity: "medium"` gives `Invalid` holding `FieldType`.
  - `body_and_other_keys_survive`: as in Task 3.
  - `an_empty_edit_returns_a_valid_document_unchanged_and_an_invalid_one_as_err`: a clean document is returned byte-identical; a `new_entry` document with `entry_kind: friction` replaced by `entry_kind: bug` and an empty edit gives `Err(Invalid(..))`.
  - `a_mixed_batch_is_refused_naming_the_bad_field`: one good edit (`severity`) and one undeclared edit (`colour`) in one call give exactly `Err(vec![WriteProblem::FieldUndeclared("colour".into())])`.
  - `a_stray_undeclared_key_can_be_deleted`: a document with a hand-added `colour: red` (undeclared, not reserved) is `Invalid` when read; `change_fields` with `colour: None` returns `Ok` and the key is gone; setting `colour` to a value is still `FieldUndeclared("colour")`.
- [ ] **Step 2: Run, expect FAIL.**
- [ ] **Step 3: Implement** with the same parse, edit, re-read shape as Task 3. Refuse a reserved name or an undeclared name that is being set; allow deleting an undeclared, non-reserved key the document already carries.
- [ ] **Step 4: Run, expect PASS.**
- [ ] **Step 5: Format and commit** `feat(librarian): lifecycle write core — change_fields`.

## Closing

- [ ] Run `./scripts/gate.sh` on the final merged tree; all four exits must be 0.
- [ ] After the merge into `experiments`: set this plan and spec `0904b912ffaffdf6` to their finished statuses, add the module to the memory `tracker-lifecycle-parked-items`, and note in F-3 that the write rules exist and only wiring waits on Plan E.
