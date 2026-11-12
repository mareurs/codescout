# Tracker Lifecycle Plan L-B: Core Completion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Finish the pure lifecycle core so the spec's eight checks, as amended on 2026-11-11, are all real and every problem the tree finds reaches a finding, and give the core the serializable, printable shape and the file scan that the tools plan (L-B2) will call.

**Architecture:** Everything stays in `src/librarian/lifecycle/`, beside the old system (strangler). The first three tasks change check behaviour to match the spec amendments (`f34885c67`). Task 4 adds `Display` and `Serialize` to the finding and error types. Task 5 adds `scan.rs`, which lists documents with git, reads them through the read door and runs `build` and `check`. No tool, catalog table, indexer path, `doctor` check or guide changes.

**Tech Stack:** Rust, `chrono`, `serde`, `thiserror` 2, `tempfile` for tests, `git` on `PATH` for the scan.

**Spec:** `docs/superpowers/specs/2026-11-11-tracker-lifecycle-design.md` (§ D4 Doctor checks, § Tools). It builds on Plan L-A, `docs/superpowers/plans/2026-11-11-tracker-lifecycle-plan-a-core.md`, branch `feat/tracker-lifecycle-plan-a-20261111` (tip `ef08c5dae`, merged into `experiments`; it was stacked on identity head `be094b671` when this plan was written).

**Why this is "core completion" and not the tools:** the tools (`doc(create/update/move/get)`, `doctor` wiring, legacy mode) sit on identity Plan B (derived catalog) and Plan C (writes go to files). Neither is written yet. Those tasks become Plan L-B2, written once B and C exist. This plan needs only identity Plan A.

## Global Constraints

- **Precondition:** work in a worktree whose branch contains `ef08c5dae`. Check with `git merge-base --is-ancestor ef08c5dae HEAD`; if it fails, stop. The spec amendment commit `f34885c67` is on `experiments` only, not in that worktree.
- No existing tool, catalog table, indexer path, `doctor` check or guide changes. The only existing-file edits outside `src/librarian/lifecycle/` are one visibility word in `src/librarian/identity/key.rs` (Task 6) and the spec edit, which the controller makes on `experiments` (After the plan).
- Before completing any task: `./scripts/gate.sh` green (CLAUDE.md § Development Commands). Targeted runs: `scripts/with-slot.sh cargo test --lib librarian::lifecycle`.
- Commit by pathspec with `git commit -F <msgfile> -- <paths>`, following `docs/conventions/shared-checkout-commit-sequence.md`; never a bare `git commit`.
- Every bound constant carries `// cap-class: NOT_A_CAP — <reason>` or `RESULT_CAP`. The gate `result_caps::every_cap_constant_is_classified` fires only on names containing CAP, LIMIT, MAX, BUDGET or THRESHOLD, or on an existing annotation, so `BACKWARD_LABELS` and `CHECK_NAMES` need none; annotate only if you name something that matches.
- Production code reads file content only through `crate::librarian::read_door` (gate `no_librarian_production_read_bypasses_the_read_door`; it flags `fs::read`, `read_to_string`, `OpenOptions`, `File::open`, not command output). Test code is exempt.
- Every new `pub` item is reachable from outside its module (`pub mod`, not `mod`), or `clippy -D warnings` fails on dead code.
- No `format!` without a placeholder in a test (clippy `useless_format`). Shared test helpers are `lifecycle/testkit.rs`: `fm`, `d`, `k`, `doc`, `LEDGER`, `STREAM`, `entry`.
- Every check reads frontmatter dates and statuses only; none reads a git commit date (spec anti-goal 2).

## Review Focus

1. **A relation line with a label the builder never listed (`**Blocks:** [[x]]`)** — expected: counts as forward, so `cited_target_closed` can fire. Only the two backward labels are exempt. Test in Task 1.
2. **A stream folder hand-moved to `streams/archive/` while an entry is still `open`** — expected: one `entry_invalid` finding per open entry (`NonTerminalArchived`), not silence. Test in Task 2.
3. **A readme whose YAML frontmatter block does not parse** — expected: one `collection_invalid` finding naming the readme path, and the collection's entries do not turn into orphans. Test in Task 3.
4. **A new kind of `TreeProblem` or `PlacementError` added later** — expected: the compiler refuses `check` until the new variant is mapped (no wildcard arm). A runtime test cannot show a missing arm, so the reviewer checks the `match` by reading it; the Task 3 test `every_tree_problem_kind_is_reported` pins that every live kind reaches a finding.
5. **A new entry file not yet `git add`ed, a file deleted but not staged, a conflicted path, a markdown file that is not UTF-8, and a closed bug outside `docs/trackers`** — expected: the new file is scanned; the deleted file is skipped and listed; the conflicted path loads once; a non-UTF-8 tracker file is an error naming its path, outside `docs/trackers` it is skipped and listed; the bug's `closed:` date still reaches `cited_target_closed`. Tests in Task 5.

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
- Modify: `src/librarian/lifecycle/checks.rs`, `src/librarian/lifecycle/layout.rs` (delete one dead variant)
- Test: `checks.rs`

**Interfaces:**
- Consumes: `tree::TreeProblem` (six variants: `Placement`, `NestedCollection`, `OrphanEntry`, `Schema`, `Entry`, `Frontmatter`), `layout::PlacementError`, `schema::SchemaError`, `identity::key::KeyError` (it has `Display` and `Error`, but no `Serialize`).
- Produces (in `checks.rs`):
  - `pub enum CollectionProblem { Schema(SchemaError), Frontmatter(String), NestedCollection, NotMarkdown, BadName(String) }` — `Debug, Clone, PartialEq, Eq`. `BadName` holds the `Display` text of the `KeyError`, so no serializer for `KeyError` is needed.
  - `Finding::CollectionInvalid { subject: String, problem: CollectionProblem }` in place of `Finding::StreamWithoutExit`. `subject` is the collection key for `Schema`, otherwise the repo-relative path of the file.
  - Sort slot stays 4. The finding order in `check` is unchanged otherwise.

Mapping (rulings for the ledger): `TreeProblem::Schema` gives one finding per `SchemaError` (so `ExitRequired` and `ExitRefused` are two of many, not special); `Frontmatter` gives `Frontmatter(detail)` by path for readme and entry files alike; `NestedCollection(p)` gives one `NestedCollection` per path; `Placement(NotMarkdown(p))` gives `NotMarkdown`; `Placement(BadKey { rel_path, error })` gives `BadName(error.to_string())` (it also arises from a bad collection folder name, one finding per file in that folder); `Placement(OutsideCollection(p))` and `OrphanEntry(p)` stay `OrphanEntry`; `Entry` stays `EntryInvalid(Unparsed)`. A non-Markdown file in a folder with no readme is `NotMarkdown`, not `OrphanEntry`: `place` refuses the name before it looks for a readme (accepted, recorded). `PlacementError::NestedCollection` is never constructed (`place` returns only `NotMarkdown`, `OutsideCollection`, `BadKey`; `build` pushes `TreeProblem::NestedCollection`), so delete that variant from `layout.rs` and its uses; the mapping then has no dead arm. The `match` over `TreeProblem` and over `PlacementError` has no wildcard arm.

- [ ] **Step 1: Write the failing tests** in `checks.rs`:
  - `a_stream_readme_without_exit_when_is_collection_invalid` — replaces the existing `StreamWithoutExit` test; expects `CollectionInvalid { subject: "s", problem: CollectionProblem::Schema(SchemaError::ExitRequired) }`.
  - `a_ledger_readme_with_exit_when_is_collection_invalid` — `SchemaError::ExitRefused`.
  - `a_readme_with_broken_yaml_is_reported_by_path_and_its_entries_are_not_orphans` (Review Focus 3) — readme text `"---\ntitle: [unclosed\n---\n"` plus one entry file: exactly one `CollectionInvalid { subject: "docs/trackers/ledgers/l/readme.md", problem: Frontmatter(_) }` and no `OrphanEntry`.
  - `every_tree_problem_kind_is_reported` (Review Focus 4) — one tree holding: a readme with exactly one schema error, a file with broken YAML, an entry that parses as YAML but fails `parse_entry`, a nested collection, a non-Markdown file, a bad entry file name, an orphan, and a file directly in a lifecycle folder; assert the findings of each kind and their count.
  - Update every existing test that names `StreamWithoutExit`.
- [ ] **Step 2: Run to verify failure** — FAIL to compile.
- [ ] **Step 3: Implement the enum, the variant and the mapping in `check`; delete `PlacementError::NestedCollection`.** Delete `StreamWithoutExit` everywhere in the module, including docs ("eight checks" stays true: `collection_invalid` is the eighth name).
- [ ] **Step 4: Run to verify pass.**
- [ ] **Step 5: Commit** — `feat(librarian): collection_invalid reports every collection-level problem`.

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
  - `Display` (thiserror) for `SchemaError`, `EntryError`, `EntryProblem`, `CollectionProblem`.
  - `Serialize` on `Finding` (`#[serde(tag = "check", rename_all = "snake_case")]`; every variant is a struct variant, which internal tagging needs) and on `EntryProblem`, `EntryError`, `SchemaError`, `CollectionProblem`, `FieldType`, each with `#[serde(rename_all = "snake_case")]` and the default external tagging. Keys serialize as strings through `table::key_as_str`, which becomes `pub(super)`.

- [ ] **Step 1: Write the failing tests** in `checks.rs`:
  - `check_names_are_the_eight_of_the_spec_and_each_variant_reports_one` — `CHECK_NAMES` has eight distinct entries; one sample finding per variant has `check_name()` in it and all eight are hit.
  - `display_is_one_line_naming_the_subject` — for each of the eight sample findings the text has no `\n`, starts with its `check_name()` and contains its key or path; pin two exact lines: `RevisitDue { entry: 2026-10-01-waits, revisit_by: 2026-11-01 }` renders `revisit_due: 2026-10-01-waits — revisit_by 2026-11-01 has passed`, and `EntryInvalid` with `StatusUndeclared("x")` renders `entry_invalid: <key> — status "x" is not declared in the collection readme`.
  - `serialized_findings_have_this_exact_json` — three pinned values: `RevisitDue` is `{"check":"revisit_due","entry":"2026-10-01-waits","revisit_by":"2026-11-01"}`; `EntryInvalid` with `StatusUndeclared("x")` is `{"check":"entry_invalid","entry":"<key>","problem":{"status_undeclared":"x"}}`; `CollectionInvalid { subject: "s", problem: Schema(ExitRequired) }` is `{"check":"collection_invalid","subject":"s","problem":{"schema":"exit_required"}}`. For all eight samples `value["check"] == check_name()`.
- [ ] **Step 2: Run to verify failure** — FAIL to compile.
- [ ] **Step 3: Implement.** Messages are the implementer's wording; each names the offending word, field or date. This JSON is the contract for Plan L-B2 and the identity session: do not change the pinned shapes.
- [ ] **Step 4: Run to verify pass.**
- [ ] **Step 5: Commit** — `feat(librarian): lifecycle findings and errors print and serialize`.

### Task 5: One `DocInput`, and `scan`

**Files:**
- Create: `src/librarian/lifecycle/scan.rs`
- Modify: `src/librarian/lifecycle/{mod,tree}.rs`
- Test: `scan.rs`

**Interfaces:**
- Consumes: `identity::index::DocInput { rel_path: String, text: String }` (`pub`, `Clone, Debug, PartialEq, Eq`); `read_door::for_project(&Path) -> ReadDoor` and `ReadDoor::text(&self, &Path) -> Result<String, ReadError>` (`pub(crate)`, reachable from `lifecycle`); `ReadError::{Refused(anyhow::Error), Io(std::io::Error)}`; `LifecycleTree::build`, `check`, `CheckPolicy`.
- Produces:
  - `tree.rs` no longer defines `DocInput`; it has `pub use crate::librarian::identity::index::DocInput;`. `testkit::doc` and every existing import keep compiling.
  - `pub struct SkippedDoc { pub rel_path: String, pub reason: String }` and `pub struct LoadedDocs { pub docs: Vec<DocInput>, pub skipped: Vec<SkippedDoc> }`.
  - `pub fn load_docs(project_root: &Path) -> anyhow::Result<LoadedDocs>`.
  - `pub struct LifecycleReport { pub findings: Vec<Finding>, pub skipped: Vec<SkippedDoc> }`.
  - `pub fn lifecycle_findings(project_root: &Path, today: NaiveDate, policy: &CheckPolicy) -> anyhow::Result<LifecycleReport>` — `load_docs`, then `build`, then `check`.
  - `lifecycle/mod.rs` declares `pub mod scan;` (a private module leaves both functions unused and the gate red).

Behaviour of `load_docs` (rulings for the ledger):
- **Listing:** `git -C <project_root> ls-files -z --cached --others --exclude-standard -- :(icase)*.md docs/trackers`, with `GIT_DIR`, `GIT_INDEX_FILE` and `GIT_WORK_TREE` removed from the child environment. The arguments are passed to `Command` as separate strings, without shell quotes. `--others` lists a new unstaged file; `:(icase)*.md` lists `.md` files of any case anywhere in the repo, which is needed because `closed_dates` come from every input document, such as a bug under `docs/issues/` cited by an entry. The output is parsed as bytes; a path that is not UTF-8 is an `Err`. The list is sorted and de-duplicated (git lists a conflicted path three times during a merge).
- **Top level:** `project_root` must be the git top-level. `git rev-parse --show-toplevel` must equal the canonical `project_root`, else `Err` saying so. A directory that is not a checkout, or that sits inside another checkout, is therefore an `Err`.
- **Reading:** each `.md` file (any case) is read with `read_door::for_project(project_root).text(..)`. A non-`.md` file under `docs/trackers` is listed with empty text (`place` refuses it by name first).
- **Unreadable files:** a `NotFound` read (deleted or moved, not yet staged) goes to `skipped` with reason "not on disk", anywhere. Any other read failure, including non-UTF-8, of a file under `docs/trackers` is an `Err` whose message names the path (`ReadError::into_anyhow` does not add the path; add it with `.with_context`). The same failure of a file outside `docs/trackers`, which only supplies a `closed:` date, goes to `skipped` with the error text.
- Paths are repo-relative with `/`.

- [ ] **Step 1: Write the failing tests** in `scan.rs`, each in a `tempfile::tempdir()` made a git checkout with `git init`:
  - `a_new_unstaged_entry_is_scanned` (Review Focus 5) — never `git add`ed; it appears in `load_docs` and in `lifecycle_findings`.
  - `a_gitignored_file_is_not_scanned`.
  - `a_cited_closed_target_outside_docs_trackers_is_dated` (Review Focus 5) — an entry with `**Rests on:** [[2026-10-05-the-bug]]` and a closed bug `docs/issues/2026-10-05-the-bug.md` give a `CitedTargetClosed`.
  - `an_upper_case_extension_is_read_as_markdown` — `docs/trackers/ledgers/l/2026-10-01-x.MD` is read, not given empty text.
  - `a_non_markdown_file_under_the_lifecycle_folders_gives_collection_invalid` — `docs/trackers/ledgers/l/notes.txt` yields `CollectionProblem::NotMarkdown`; a `.txt` outside `docs/trackers` is absent from `load_docs`.
  - `a_file_deleted_but_not_staged_is_skipped_not_fatal` — `git add` a file, delete it from disk: `load_docs` is `Ok` and `skipped` names it.
  - `a_non_utf8_tracker_file_is_an_error_naming_its_path`; `a_non_utf8_file_outside_docs_trackers_is_skipped`.
  - `a_merge_conflicted_path_is_loaded_once` — build a real merge conflict on one file; the path appears once in `docs`.
  - `a_directory_that_is_not_the_top_level_is_an_error` — run `load_docs` on a subdirectory of a checkout; and on a plain directory with no `.git`.
  - `findings_over_a_small_repo_match_the_pure_check` — build the same documents with `build` and `check` directly and compare.
- [ ] **Step 2: Run to verify failure** — FAIL to compile.
- [ ] **Step 3: Implement `load_docs`, `lifecycle_findings`, `pub mod scan;`, and the `DocInput` re-export.**
- [ ] **Step 4: Run to verify pass** — targeted tests, then `scripts/with-slot.sh cargo test --lib no_librarian_production_read_bypasses_the_read_door`.
- [ ] **Step 5: Commit** — `feat(librarian): scan a checkout into lifecycle findings`.

### Task 6: Parked Plan L-A cleanups that need no other plan

**Files:**
- Modify: `src/librarian/lifecycle/layout.rs`, `src/librarian/lifecycle/tree.rs`, `src/librarian/lifecycle/naming.rs`, `src/librarian/identity/key.rs` (visibility only), `src/librarian/lifecycle/schema.rs`
- Test: `layout.rs`, `schema.rs`

**Interfaces:**
- Consumes: `identity::key::components` (`pub(super) fn(&str) -> Vec<String>`, key.rs:88-96), `layout::segments` (layout.rs:86-93, same meaning), `tree::parent_dir` and a splitter in `naming.rs` (both re-split a path by hand), `layout::place`.
- Produces: `components` made `pub(crate)`; `layout::segments`, the splitter in `tree::parent_dir` and the one in `naming.rs` replaced by it; `place` returns `Some(Err(PlacementError::NotMarkdown(path)))` for a path with a trailing separator under a lifecycle folder, instead of accepting it.

- [ ] **Step 1: Write the failing tests.** `a_trailing_separator_is_not_a_tracker_file` in `layout.rs`; and `a_readme_without_the_key_is_missing_key` in `schema.rs` asserting `SchemaError::MissingKey(_)` (every other `SchemaError` variant already has a test: schema.rs:387-422).
- [ ] **Step 2: Run to verify failure.**
- [ ] **Step 3: Implement.** The visibility change in identity is one word and touches no identity behaviour. Existing tests must stay green unchanged.
- [ ] **Step 4: Run to verify pass.**
- [ ] **Step 5: Commit** — `refactor(librarian): lifecycle shares identity's path splitter`.

---

## After the plan

- **Controller step, on `experiments` in the main checkout (the worktree's spec copy is the pre-amendment version):** edit the spec with `doc(action="update", id="6d16009652c88e57", patch={body_edits: [...]})`, commit it alone by pathspec.
  - In the `collection_invalid` table row add "a file whose frontmatter block does not parse, a file name that is no key".
  - In the `entry_invalid` rationale replace the sentence "An entry whose file does not parse is reported by `entry_invalid`, and it still counts as non-terminal for `stream_all_terminal`." (it spans two lines in the file; match each line) with "An entry whose frontmatter parses but breaks the entry rules is reported by `entry_invalid`; a file whose frontmatter block does not parse is reported by `collection_invalid`, by path. Either way it counts as non-terminal for `stream_all_terminal`."
  - § Tools: replace "`status` defaults to the first `open`-class word" with "`status` defaults to the collection's `initial_status`".
  - Frontmatter title: "four lifecycle folders" becomes "three".
  - Record the Task 3 mapping rulings in the ledger.
- Run the full gate once at the end (`./scripts/gate.sh`). Run the census (`census_of_this_repo_under_the_lifecycle_model`) by hand and record any change in the tracker-organization session log.
- Send the identity session the Interfaces blocks of Tasks 3, 4 and 5 (names and types only, with the pinned JSON): `lifecycle_findings` is what Plan B's catalog rebuild and `doctor` would call.
- Plan L-B2 (tools: `doc(create)` for entries and collections, `doc(update)` status handling with `status_since`, `doc(move)` D3 rules, `doc(get)` on a collection through `table::rows`, `doctor` wiring over `lifecycle_findings`, legacy mode) starts once identity Plans B and C exist. Plans L-C (consumers), L-D (guide and skills) and L-E (migration, in the identity Plan E window) follow. Carried to L-B2: a legacy `[[collection:R-91]]` citation resolves to the collection, not the migrated entry, so `cited_target_closed` misses it after migration; `table` escaping edge cases (a title `\[a](b)` renders a working link) only matter if a table is written to a file.
