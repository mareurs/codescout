# Tracker Lifecycle Plan L-B: Core Completion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Finish the pure lifecycle core so the spec's eight checks, as amended on 2026-11-11, are all real and every problem the tree finds reaches a finding, and give the core the serializable, printable shape and the file scan that the tools plan (L-B2) will call.

**Architecture:** Everything stays in `src/librarian/lifecycle/`, beside the old system (strangler). The first three tasks change check behaviour to match the spec amendments (`f34885c67`). Task 4 adds `Display` and `Serialize` to the finding and error types. Task 5 adds `scan.rs`, which lists documents with git, reads them through the read door and runs `build` and `check`. No tool, catalog table, indexer path, `doctor` check or guide changes.

**Tech Stack:** Rust, `chrono`, `serde`, `thiserror` 2, `tempfile` for tests, `git` on `PATH` for the scan.

**Spec:** `docs/superpowers/specs/2026-11-11-tracker-lifecycle-design.md` (§ D4 Doctor checks, § Tools). It builds on Plan L-A, `docs/superpowers/plans/2026-11-11-tracker-lifecycle-plan-a-core.md`, branch `feat/tracker-lifecycle-plan-a-20261111` (tip `ef08c5dae`, not merged; stacked on identity head `be094b671`).

**Why this is "core completion" and not the tools:** the tools (`doc(create/update/move/get)`, `doctor` wiring, legacy mode) sit on identity Plan B (derived catalog) and Plan C (writes go to files). Neither is written yet. Those tasks become Plan L-B2, written once B and C exist. This plan needs only identity Plan A.

## Global Constraints

- **Precondition:** work in a worktree whose branch contains `ef08c5dae`. Check with `git merge-base --is-ancestor ef08c5dae HEAD`; if it fails, stop.
- No existing tool, catalog table, indexer path, `doctor` check or guide changes. The only existing-file edits outside `src/librarian/lifecycle/` are none.
- Before completing any task: `./scripts/gate.sh` green (CLAUDE.md § Development Commands). Targeted runs: `scripts/with-slot.sh cargo test --lib librarian::lifecycle`.
- Commit by pathspec with `git commit -F <msgfile> -- <paths>`, following `docs/conventions/shared-checkout-commit-sequence.md`; never a bare `git commit`.
- Every bound constant carries `// cap-class: NOT_A_CAP — <reason>` or `RESULT_CAP` (gate `result_caps::every_cap_constant_is_classified`).
- Production code reads files only through `crate::librarian::read_door` (gate `no_librarian_production_read_bypasses_the_read_door`). Test code is exempt.
- No `format!` without a placeholder in a test (clippy `useless_format`). Shared test helpers are `lifecycle/testkit.rs`: `fm`, `d`, `k`, `doc`, `LEDGER`, `STREAM`, `entry`.
- Every check reads frontmatter dates and statuses only; none reads a git commit date (spec anti-goal 2).

## Review Focus

1. **A relation line with a label the builder never listed (`**Blocks:** [[x]]`)** — expected: counts as forward, so `cited_target_closed` can fire. Only the two backward labels are exempt. Test in Task 1.
2. **A stream folder hand-moved to `streams/archive/` while an entry is still `open`** — expected: one `entry_invalid` finding per open entry (`NonTerminalArchived`), not silence. Test in Task 2.
3. **A readme whose YAML frontmatter block does not parse** — expected: one `collection_invalid` finding naming the readme path, and the collection's entries do not turn into orphans. Test in Task 3.
4. **A new kind of `TreeProblem` added later** — expected: the compiler refuses `check` until the new variant is mapped (no `_ => {}` arm). Test in Task 3 builds one problem of every kind and counts the findings.
5. **A new entry file created but not yet `git add`ed, and a markdown file that is not UTF-8** — expected: the first is scanned; the second is an error that names its path, never a silently shorter tree. Tests in Task 5.

---

### Task 1: Forward-only relation labels

**Files:**
- Modify: `src/librarian/lifecycle/relations.rs`, `src/librarian/lifecycle/checks.rs`
- Test: both files, `#[cfg(test)]` modules

**Interfaces:**
- Consumes: `relations::Relation { label: String, reference: Reference, line: u32 }`; `checks::check_entry` (the `CitedTargetClosed` loop over `entry.relations`).
- Produces:
  - `pub const BACKWARD_LABELS: [&str; 2] = ["Promoted from", "Supersedes"];` in `relations.rs`.
  - `pub fn is_backward(label: &str) -> bool` — ASCII case-insensitive match against `BACKWARD_LABELS`.

- [ ] **Step 1: Write the failing tests.**
  - In `relations.rs`: `backward_labels_are_exactly_promoted_from_and_supersedes` — `is_backward("Promoted from")`, `is_backward("supersedes")` are true; `is_backward("Rests on")`, `is_backward("Promoted to")`, `is_backward("Blocks")` are false.
  - In `checks.rs`: `a_backward_relation_to_a_closed_target_never_fires` — a ledger entry (`status_since` 2026-10-01) with `**Promoted from:** [[2026-10-05-the-bug]]` and the closed bug of the existing `BUG` const gives no `CitedTargetClosed`; the same entry with `**Rests on:**` gives one; with `**Blocks:**` gives one (Review Focus 1).
- [ ] **Step 2: Run to verify failure** — `scripts/with-slot.sh cargo test --lib librarian::lifecycle` → FAIL to compile (`is_backward` undefined).
- [ ] **Step 3: Implement `is_backward` and the const in `relations.rs`; in `check_entry` filter `entry.relations` with `!is_backward(&r.label)` before `local_target`.** The const is a label list, not a bound: annotate it `// cap-class: NOT_A_CAP — a closed vocabulary of relation labels, not a limit on any result`.
- [ ] **Step 4: Run to verify pass** — same command → PASS.
- [ ] **Step 5: Commit** — `feat(librarian): only forward relation labels feed cited_target_closed`.

### Task 2: An archived entry that is not terminal is invalid

**Files:**
- Modify: `src/librarian/lifecycle/entry.rs`, `src/librarian/lifecycle/checks.rs`
- Test: `checks.rs`

**Interfaces:**
- Consumes: `EntryProblem`; `EntryNode { archived, meta, .. }`; `class_of(&CollectionSchema, &EntryMeta) -> Option<StatusClass>`.
- Produces: `EntryProblem::NonTerminalArchived { status: String }`.

- [ ] **Step 1: Write the failing tests** in `checks.rs`:
  - `a_non_terminal_entry_in_archive_is_entry_invalid` — `docs/trackers/ledgers/l/archive/2026-10-01-x.md` with status `open` gives `Finding::EntryInvalid { entry: k("2026-10-01-x"), problem: EntryProblem::NonTerminalArchived { status: "open".into() } }`; the same file with status `fixed` and `closed:` gives none.
  - `an_open_entry_of_a_stream_moved_to_streams_archive_is_reported` (Review Focus 2) — `docs/trackers/streams/archive/s/readme.md` (`STREAM`) with one `open` entry gives the same finding for that entry.
  - `an_archived_entry_with_an_undeclared_status_gives_only_status_undeclared` — no `NonTerminalArchived` on top.
- [ ] **Step 2: Run to verify failure** — FAIL to compile.
- [ ] **Step 3: Add the variant to `entry.rs`; in `check_entry`, after the `validate_entry` loop and before the `if entry.archived { return; }`, push the finding when `entry.archived` and `class_of(..)` is `Some(c)` with `c != Terminal`.** Update the module doc of `checks.rs` (the "which entries each check looks at" paragraph) to say `entry_invalid` also covers a non-terminal archived entry.
- [ ] **Step 4: Run to verify pass.**
- [ ] **Step 5: Commit** — `feat(librarian): entry_invalid covers a non-terminal archived entry`.

### Task 3: `collection_invalid` replaces `stream_without_exit`

**Files:**
- Modify: `src/librarian/lifecycle/checks.rs`; `docs/superpowers/specs/2026-11-11-tracker-lifecycle-design.md` (two sentences, through `doc(update)` with `body_edits`)
- Test: `checks.rs`

**Interfaces:**
- Consumes: `tree::TreeProblem` (all six variants), `layout::PlacementError`, `schema::SchemaError`, `identity::key::KeyError` (`Debug, Clone, PartialEq, Eq`, no `Display`).
- Produces (in `checks.rs`):
  - `pub enum CollectionProblem { Schema(SchemaError), Frontmatter(String), NestedCollection, NotMarkdown, BadEntryName(KeyError) }` — `Debug, Clone, PartialEq, Eq`.
  - `Finding::CollectionInvalid { subject: String, problem: CollectionProblem }` in place of `Finding::StreamWithoutExit`. `subject` is the collection key for `Schema`, otherwise the repo-relative path of the file.
  - Sort slot stays 4. The finding order in `check` is unchanged otherwise.

Mapping (decided here, ruling for the ledger): `TreeProblem::Schema` gives one finding per `SchemaError` (so `ExitRequired` and `ExitRefused` are two of many, not special); `Frontmatter` gives `Frontmatter(detail)` by path for both readme and entry files; `NestedCollection(p)` and `Placement(NestedCollection(p))` give one `NestedCollection` per path; `Placement(NotMarkdown(p))` gives `NotMarkdown`; `Placement(BadKey { rel_path, error })` gives `BadEntryName(error)`; `Placement(OutsideCollection(p))` and `OrphanEntry(p)` stay `OrphanEntry`; `Entry` stays `EntryInvalid(Unparsed)`. The `match` over `TreeProblem` and over `PlacementError` has no wildcard arm.

- [ ] **Step 1: Write the failing tests** in `checks.rs`:
  - `a_stream_readme_without_exit_when_is_collection_invalid` — replaces the existing `StreamWithoutExit` test; expects `CollectionInvalid { subject: "s", problem: CollectionProblem::Schema(SchemaError::ExitRequired) }`.
  - `a_ledger_readme_with_exit_when_is_collection_invalid` — `SchemaError::ExitRefused`.
  - `a_readme_with_broken_yaml_is_reported_by_path_and_its_entries_are_not_orphans` (Review Focus 3) — readme text `"---\ntitle: [unclosed\n---\n"` plus one entry file: exactly one `CollectionInvalid { subject: "docs/trackers/ledgers/l/readme.md", problem: Frontmatter(_) }` and no `OrphanEntry`.
  - `every_tree_problem_kind_gives_exactly_one_finding` (Review Focus 4) — one tree holding a bad schema, a broken-YAML entry, an entry with bad frontmatter fields, a nested collection, a non-Markdown file, a bad entry file name, an orphan and a file directly in a lifecycle folder; assert the count of findings of each kind.
  - Update every existing test that names `StreamWithoutExit`.
- [ ] **Step 2: Run to verify failure** — FAIL to compile.
- [ ] **Step 3: Implement the enum, the variant and the mapping in `check`.** Delete `StreamWithoutExit` everywhere in the module, including docs ("eight checks" stays true: `collection_invalid` is the eighth name).
- [ ] **Step 4: Run to verify pass.**
- [ ] **Step 5: Amend the spec** with `doc(action="update", id="6d16009652c88e57", patch={body_edits: [...]})`: in the `collection_invalid` table row add "a file whose frontmatter block does not parse, a file name that is no key"; in the `entry_invalid` rationale replace "An entry whose file does not parse is reported by `entry_invalid`" with "An entry whose frontmatter parses but breaks the entry rules is reported by `entry_invalid`; a file whose frontmatter block does not parse is reported by `collection_invalid`, by path". Commit it alone, by pathspec.
- [ ] **Step 6: Commit** — `feat(librarian): collection_invalid reports every collection-level problem`.

### Task 4: `Display` and `Serialize` for findings and errors

**Files:**
- Modify: `src/librarian/lifecycle/{checks,entry,schema,table}.rs`
- Test: `checks.rs`

**Interfaces:**
- Consumes: `Finding`, `CollectionProblem` (Task 3), `EntryProblem` (Task 2), `EntryError`, `SchemaError`, `FieldType`.
- Produces:
  - `pub const CHECK_NAMES: [&str; 8] = ["entry_invalid", "revisit_due", "cited_target_closed", "stream_all_terminal", "collection_invalid", "ledger_quiet", "terminal_in_live_dir", "orphan_entry"];`
  - `Finding::check_name(&self) -> &'static str`, one of `CHECK_NAMES`.
  - `impl std::fmt::Display for Finding` — one line, `<check_name>: <subject> — <detail>`, subject being the entry key, collection key or path.
  - `Display` (thiserror) for `SchemaError`, `EntryError`, `EntryProblem`, `CollectionProblem`; `KeyError` is shown with `{:?}` because it has no `Display`.
  - `Serialize` on `Finding` (`#[serde(tag = "check", rename_all = "snake_case")]`), `EntryProblem`, `EntryError`, `SchemaError`, `CollectionProblem`, `FieldType` (`#[serde(rename_all = "snake_case")]`, externally tagged). Keys serialize as strings through `table::key_as_str`, which becomes `pub(super)`.

- [ ] **Step 1: Write the failing tests** in `checks.rs`:
  - `check_names_are_the_eight_of_the_spec_and_each_variant_reports_one` — `CHECK_NAMES` has eight distinct entries; one sample finding per variant has `check_name()` in it and all eight are hit.
  - `display_is_one_line_naming_the_subject` — for each of the eight sample findings the text has no `\n`, starts with its `check_name()` and contains its key or path; pin two exact lines: `RevisitDue { entry: 2026-10-01-waits, revisit_by: 2026-11-01 }` renders `revisit_due: 2026-10-01-waits — revisit_by 2026-11-01 has passed`, and `EntryInvalid` with `StatusUndeclared("x")` renders `entry_invalid: <key> — status "x" is not declared in the collection readme`.
  - `serialized_findings_carry_the_check_name_and_string_keys` — `serde_json::to_value` of a `RevisitDue` equals `{"check":"revisit_due","entry":"2026-10-01-waits","revisit_by":"2026-11-01"}`; for all eight samples `value["check"] == check_name()`.
- [ ] **Step 2: Run to verify failure** — FAIL to compile.
- [ ] **Step 3: Implement.** Messages are the implementer's wording; each names the offending word, field or date.
- [ ] **Step 4: Run to verify pass.**
- [ ] **Step 5: Commit** — `feat(librarian): lifecycle findings and errors print and serialize`.

### Task 5: One `DocInput`, and `scan`

**Files:**
- Create: `src/librarian/lifecycle/scan.rs`
- Modify: `src/librarian/lifecycle/{mod,tree}.rs`
- Test: `scan.rs`

**Interfaces:**
- Consumes: `identity::index::DocInput { rel_path: String, text: String }` (`Clone, Debug, PartialEq, Eq`); `read_door::for_project(&Path) -> ReadDoor`; `ReadDoor::text(&self, &Path) -> Result<String, ReadError>`; `ReadError::into_anyhow(self) -> anyhow::Error`; `LifecycleTree::build`, `check`, `CheckPolicy`.
- Produces:
  - `tree.rs` no longer defines `DocInput`; it has `pub use crate::librarian::identity::index::DocInput;`. `testkit::doc` and every existing import keep compiling.
  - `pub fn load_docs(project_root: &Path) -> anyhow::Result<Vec<DocInput>>` — lists files with `git ls-files -z --cached --others --exclude-standard -- '*.md' docs/trackers` run in `project_root`, so a new unstaged entry is listed and an ignored file is not. Every `.md` file is read through the read door; a non-`.md` file under `docs/trackers` is listed with empty text (`place` refuses it by name before the text matters). Paths are repo-relative with `/`. The result is sorted by path. A failed `git` call, or a failed read, is an `Err` that names the path.
  - `pub fn lifecycle_findings(project_root: &Path, today: NaiveDate, policy: &CheckPolicy) -> anyhow::Result<Vec<Finding>>` — `load_docs`, then `build`, then `check`.

- [ ] **Step 1: Write the failing tests** in `scan.rs`, each in a `tempfile::tempdir()` with `git init`:
  - `a_new_unstaged_entry_is_scanned` (Review Focus 5) — never `git add`ed; it appears in `load_docs` and in `lifecycle_findings`.
  - `a_gitignored_file_is_not_scanned`.
  - `a_non_markdown_file_under_the_lifecycle_folders_gives_collection_invalid` — `docs/trackers/ledgers/l/notes.txt` yields a `CollectionProblem::NotMarkdown` finding; a `.txt` outside `docs/trackers` is absent from `load_docs`.
  - `a_non_utf8_markdown_file_is_an_error_naming_its_path` (Review Focus 5).
  - `outside_a_git_checkout_load_docs_is_an_error`.
  - `findings_over_a_small_repo_match_the_pure_check` — build the same documents with `build` and `check` directly and compare.
- [ ] **Step 2: Run to verify failure** — FAIL to compile.
- [ ] **Step 3: Implement `load_docs`, `lifecycle_findings`, the `mod scan;` line, and the `DocInput` re-export.** Parse the NUL-separated `git` output as bytes; a path that is not UTF-8 is an `Err`. Run the gate test `no_librarian_production_read_bypasses_the_read_door` early: if it flags `git`'s output handling, it is a read of a command, not a file, and the allow-list is not touched; if it flags anything else, route it through the door.
- [ ] **Step 4: Run to verify pass** — targeted tests, then the gate test above.
- [ ] **Step 5: Commit** — `feat(librarian): scan a checkout into lifecycle findings`.

### Task 6: Parked Plan L-A cleanups that need no other plan

**Files:**
- Modify: `src/librarian/lifecycle/layout.rs`, `src/librarian/identity/key.rs` (visibility only), `src/librarian/lifecycle/schema.rs`
- Test: both lifecycle files

**Interfaces:**
- Consumes: identity's private path splitter (named `components` in `identity/key.rs`; find it with `grep`), `layout::segments`, `layout::place`.
- Produces: `layout::segments` removed in favour of the identity helper made `pub(crate)`; `place` returns `Some(Err(PlacementError::NotMarkdown(path)))` for a path with a trailing separator under a lifecycle folder, instead of accepting it.

- [ ] **Step 1: Write the failing tests.** `a_trailing_separator_is_not_a_tracker_file` in `layout.rs`; and one test per `SchemaError` variant that has none today (`MissingTitle`, `MissingKey`, `UnknownClass`, `NoTerminalStatus`, `InitialNotOpen`, `InitialUndeclared`, `ExitRefused` — check which already exist with `grep` first; add only the missing ones), asserting the exact variant.
- [ ] **Step 2: Run to verify failure.**
- [ ] **Step 3: Implement.** Visibility change in identity is one word; it touches no identity behaviour.
- [ ] **Step 4: Run to verify pass.**
- [ ] **Step 5: Commit** — `refactor(librarian): lifecycle layout shares identity's path splitter`.

---

## After the plan

- Run the full gate once at the end (`./scripts/gate.sh`). Run the census (`census_of_this_repo_under_the_lifecycle_model`) by hand and record any change in the tracker-organization session log.
- Send the identity session the Interfaces blocks of Tasks 3 and 5 (names and types only): `lifecycle_findings` is what Plan B's catalog rebuild and `doctor` would call.
- Plan L-B2 (tools: `doc(create)` for entries and collections, `doc(update)` status handling with `status_since`, `doc(move)` D3 rules, `doc(get)` on a collection through `table::rows`, `doctor` wiring over `lifecycle_findings`, legacy mode) starts once identity Plans B and C exist. Plans L-C (consumers), L-D (guide and skills) and L-E (migration, in the identity Plan E window) follow.
