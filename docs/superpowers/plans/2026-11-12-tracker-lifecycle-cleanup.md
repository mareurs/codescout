---
id: '011a4f5022eea4c0'
kind: plan
status: draft
title: Tracker Lifecycle Cleanup — Implementation Plan
tags:
- trackers
- lifecycle
- librarian
- plan
---

# Tracker Lifecycle Cleanup Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the five parked lifecycle items that need no identity work: a finding for a duplicate readme, a dead field removed, a rustdoc pointer and an exhaustive match, a locale-proof scan test, and `checks.rs` tests in their own file.

**Architecture:** Everything stays in `src/librarian/lifecycle/`. Task 1 adds one `TreeProblem` variant and one `CollectionProblem` variant, and changes one existing behaviour (a broken first readme no longer lets a good second readme create the collection). Tasks 2 to 4 change no behaviour. Task 5 moves test code only.

**Tech Stack:** Rust, `cargo test`, `rustfmt`.

**Spec:** `docs/superpowers/specs/2026-11-12-lifecycle-cleanup-design.md` (id `f01327c4f92f9a72`). Parent: `docs/superpowers/specs/2026-11-11-tracker-lifecycle-design.md`.

## Global Constraints

- The eight check names in `checks::CHECK_NAMES` do not change.
- `Finding` keeps its serialized shape: internally tagged `{"check": "<name>", ...}`; nested enums snake_case, externally tagged. `CollectionInvalid.subject` is always a repo-relative file path.
- `Display for Finding` stays one line with control characters escaped.
- No change to the behaviour of an existing check, except the one in Task 1 (spec § 1, "A deliberate change"). Tasks 2 to 5 change none.
- "Path order" is the raw byte order of `rel_path` as `build` sorts it (`tree.rs`, the `sort_by` at the top of `build`), not the order of a normalised path.
- Format a lifecycle file with `rustfmt --edition 2021 <file>`: `./scripts/fmt-mine.sh` refuses lifecycle files.
- Run a targeted test with `scripts/with-slot.sh cargo test --lib librarian::lifecycle::`, and run it again with `--no-default-features`.
- Write each commit with: `git add <paths>`, then `git commit -m "..." -- <paths>`. Never `git add -A`.
- Test code is in English. Name a test after the behaviour it pins.

## Review Focus

1. The first readme is broken (`README.md` has bad YAML) and `readme.md` is good: the second is still a duplicate, the entries are not orphans, and the collection fails. This changes today's output; it is the red-first test.
2. A duplicate readme whose own frontmatter is broken: one `duplicate_readme` finding and no `frontmatter` or `schema` finding, because it is skipped.
3. A duplicate readme in an archived collection (`docs/trackers/streams/archive/s/`): same single finding.
4. Input order: the same files in the reverse order give the same problems.
5. The finding's subject is the skipped file, never the kept one.
6. The key of the duplicate test is the folder, not the collection key: `streams/s/` and `streams/archive/s/`, or `ledgers/x/` and `streams/x/`, are not duplicates of each other.
7. Three readmes (`README.md`, `Readme.md`, `readme.md`) give two problems, for the second and the third in byte order.
8. A readme in a nested subfolder is `NestedCollection`, and a readme directly in a lifecycle folder is an orphan. Neither is a duplicate.

---

### Task 1: A second readme in one collection folder is `collection_invalid`

**Files:**
- Modify: `src/librarian/lifecycle/tree.rs` (`TreeProblem`, `LifecycleTree::build`, tests, the doc of `NestedCollection`)
- Modify: `src/librarian/lifecycle/checks.rs` (`CollectionProblem`, the `TreeProblem` match in `check`, tests, the module doc that lists the collection-level problems)
- Modify: `src/librarian/lifecycle/table.rs` (the census `match p` over `TreeProblem`, about line 512)

**Interfaces:**
- Consumes: `place(&str) -> Option<Result<Placement, PlacementError>>`, `Placement { role, collection_dir, .. }`, `Role::CollectionReadme`.
- Produces: `TreeProblem::DuplicateReadme(String)` (the repo-relative path of the skipped readme); `CollectionProblem::DuplicateReadme`, serialized `"duplicate_readme"`, `#[error("is a second readme in its collection folder; only the first one in path order is read")]`.

- [ ] **Step 1: Write the failing tests** (they do not compile yet).
  In `tree.rs` `mod tests`, with the existing helpers `doc`, `entry`, `LEDGER`, `STREAM`; every entry file uses `&entry("open", "", "")`:
  - `a_second_readme_in_one_folder_is_a_problem_and_the_first_in_path_order_decides`: `docs/trackers/ledgers/l/README.md` (`LEDGER`), `docs/trackers/ledgers/l/readme.md` (`"---\ntitle: [unclosed\n---\n"`), and entry `docs/trackers/ledgers/l/2026-10-01-a.md`. Assert `t.problems` is exactly one `TreeProblem::DuplicateReadme(p)` with `p == "docs/trackers/ledgers/l/readme.md"` (the broken second readme gives no `Frontmatter` problem), `t.collections.len() == 1`, and that collection has one entry. Build again with the three files in reverse order and assert the same.
  - `the_first_readme_decides_even_when_it_is_broken`: `README.md` has `"---\ntitle: [unclosed\n---\n"`, `readme.md` is `LEDGER`, plus one entry. Assert `t.problems` is exactly one `Frontmatter` problem for `README.md` and one `DuplicateReadme` for `readme.md`, no `OrphanEntry`, and `t.collections` is empty. This test is red against today's `build`, where `t.collections` has one collection.
  - `a_second_readme_in_an_archived_collection_is_a_problem`: `docs/trackers/streams/archive/s/README.md` and `.../readme.md`, both `STREAM`. Assert `t.problems` is exactly one `DuplicateReadme` for `.../readme.md`.
  - `three_readmes_in_one_folder_give_two_problems`: `README.md`, `Readme.md`, `readme.md` in `docs/trackers/ledgers/l/`, all `LEDGER`, in an input order that is not sorted. Assert the problems are exactly `DuplicateReadme` for `Readme.md` and for `readme.md`.
  - `the_same_collection_key_in_two_folders_is_not_a_duplicate`: `streams/s/readme.md` and `streams/archive/s/readme.md` (both `STREAM`), `ledgers/x/readme.md` (`LEDGER`) and `streams/x/readme.md` (`STREAM`). Assert `t.problems` is empty and `t.collections.len() == 4`.
  In `checks.rs` `mod tests`:
  - `a_second_readme_is_collection_invalid_naming_the_skipped_file`: `run([README.md LEDGER, readme.md LEDGER])` in `docs/trackers/ledgers/l/` equals, in the order `run` returns, `[Finding::CollectionInvalid { subject: "docs/trackers/ledgers/l/readme.md", problem: CollectionProblem::DuplicateReadme }, Finding::LedgerQuiet { collection: k("l"), last_activity: None }]`. `LEDGER` declares `review_every: 30d` and the collection has no entry, so `LedgerQuiet` is expected; it also shows that the first readme was the one kept. Do not reorder the expected list to make the test pass: read `Finding::sort_key` and state the order you found.
  - `a_readme_directly_in_a_lifecycle_folder_is_an_orphan_not_a_duplicate`: `run([docs/trackers/temp/README.md, docs/trackers/temp/readme.md])` (both `STREAM`). Read `layout.rs` `place` first. Assert no finding is `DuplicateReadme` and both paths are `orphan_entry` findings. If `place` says otherwise, stop and report.
  - In `nested_problems_serialize_in_snake_case_with_default_tagging`: `serde_json::to_string(&CollectionProblem::DuplicateReadme)` equals `"\"duplicate_readme\""`, and its `Display` text equals the `#[error]` string above.
  - In `every_tree_problem_kind_is_reported`: add `docs/trackers/ledgers/dup/README.md` and `.../dup/readme.md` (both `LEDGER`) and `subjects_of(|p| matches!(p, CollectionProblem::DuplicateReadme))` equals `["docs/trackers/ledgers/dup/readme.md"]`. The total-count assertion there (`1 + 1 + 2 + 1 + 2`) becomes `1 + 1 + 2 + 1 + 2 + 1`. The new `dup` ledger also gives a `LedgerQuiet`, which that test does not count.
  The nested-subfolder readme (Review Focus 8) is pinned by the existing `a_subfolder_with_its_own_readme_is_a_nested_collection_and_its_files_are_skipped`; do not duplicate it.
- [ ] **Step 2: Make it compile, with the behaviour held back.** Add `TreeProblem::DuplicateReadme(String)` with a doc comment, `CollectionProblem::DuplicateReadme` with the `#[error]` text above, the full arm in `check` (`collection_invalid(rel_path, CollectionProblem::DuplicateReadme)`), and the census arm `TreeProblem::DuplicateReadme(_) => "DuplicateReadme"` in `table.rs`. Leave `build` unchanged.
- [ ] **Step 3: Run and see the tests fail on their assertions:** `scripts/with-slot.sh cargo test --lib librarian::lifecycle::`. Expected: the new tree tests and `a_second_readme_is_collection_invalid_naming_the_skipped_file` FAIL; the serialization test and the orphan test PASS. State which of them passed and why.
- [ ] **Step 4: Implement in `build`.** Keep a `BTreeSet<String>` of collection folders (`placement.collection_dir`) whose readme has been seen. After `place` succeeds and before the nested-readme check, when `placement.role == Role::CollectionReadme` and `!seen.insert(placement.collection_dir.clone())`, push `TreeProblem::DuplicateReadme(doc.rel_path.clone())` and `continue`. The parse of the frontmatter and the `closed_dates` update above the placement stay as they are.
- [ ] **Step 5: Run** `scripts/with-slot.sh cargo test --lib librarian::lifecycle::` and again with `--no-default-features`. Expected: all pass. Then mutate by hand and restore each time: (a) delete the `continue` after the duplicate push, and see `a_second_readme_in_one_folder_is_a_problem_and_the_first_in_path_order_decides` fail; (b) key the set on `placement.collection` instead of `placement.collection_dir`, and see `the_same_collection_key_in_two_folders_is_not_a_duplicate` fail.
- [ ] **Step 6: Update the docs that list the problems:** the module doc of `checks.rs` that lists the collection-level problems, the docs of `CollectionProblem` and `TreeProblem`, and the doc of `TreeProblem::NestedCollection` (it says `readme.md`; the match is case-insensitive). Format the three files with `rustfmt --edition 2021` and commit with `feat(librarian): a second readme in one collection folder is collection_invalid`.

---

### Task 2: `TreeProblem::Schema` without `collection`

**Files:**
- Modify: `src/librarian/lifecycle/tree.rs` (`TreeProblem::Schema`, its construction in `build`, one test)
- Modify: `src/librarian/lifecycle/checks.rs` (the `TreeProblem::Schema { rel_path, errors, .. }` pattern)

**Interfaces:**
- Consumes: Task 1's tree.
- Produces: `TreeProblem::Schema { rel_path: String, errors: Vec<SchemaError> }`.

- [ ] **Step 1: Remove the field** and its doc comment, and its construction in `build`. The only test that names it is in `tree.rs` (`a_failing_schema_is_reported_once_and_its_entries_are_skipped_silently`); it already asserts `rel_path`, so delete only the `collection: k("s"),` line. In `checks.rs` drop the now-empty `..` from the `Schema` pattern if the pattern has no other ignored field. `table.rs` already uses `{ .. }`.
- [ ] **Step 2: Run** the lifecycle tests with the default features and with `--no-default-features`. Expected: all pass, with the same test count as before the task.
- [ ] **Step 3: Format and commit** with `refactor(librarian): TreeProblem::Schema drops the collection key nothing reads`.

---

### Task 3: A rustdoc pointer and an exhaustive match

**Files:**
- Modify: `src/librarian/lifecycle/checks.rs` (`CheckPolicy`, `check_entry`)

**Interfaces:**
- Consumes: `Lifecycle` with the variants `Ledger`, `Stream`, `Temp`.
- Produces: no change to any signature.

- [ ] **Step 1: Rewrite the `terminal_grace_days` doc comment.** It says: days a terminal ledger entry may stay outside `archive/` after its `closed` date; the default is 30; the measurement behind the default is entry W-3 of `docs/trackers/tracker-organization-session-log.md`. No count or date of a measurement stays in the comment.
- [ ] **Step 2: Change the branch** `} else if collection.lifecycle == Lifecycle::Ledger {` in `check_entry` to `} else { match collection.lifecycle { Lifecycle::Ledger => { <the existing body> } Lifecycle::Stream | Lifecycle::Temp => {} } }`. Keep the existing comment, moved above the empty arm. The behaviour is identical.
- [ ] **Step 3: Run** the lifecycle tests, both feature sets. `a_terminal_entry_of_a_live_stream_is_not_terminal_in_live_dir` and `the_default_grace_is_thirty_days` must pass unchanged. Mutate once by hand: make the `Stream | Temp` arm run the `Ledger` body, see `a_terminal_entry_of_a_live_stream_is_not_terminal_in_live_dir` fail, restore.
- [ ] **Step 4: Format and commit** with `refactor(librarian): terminal_in_live_dir matches on Lifecycle; the grace rustdoc points at its measurement`.

---

### Task 4: The scan tests run git in the C locale

**Files:**
- Modify: `src/librarian/lifecycle/scan.rs` (`mod tests`: the function `hermetic` and the test `a_directory_that_is_not_a_checkout_is_an_error_naming_it_and_git_s_reason`, about lines 595 to 616)

**Interfaces:**
- Consumes: `hermetic(dir: &Path) -> Command`.
- Produces: no new function.

- [ ] **Step 1: Make the existing test ask for German.** In that test, the `outside` closure already builds on `hermetic(dir)`; add `.env("LANGUAGE", "de").env("LANG", "en_US.UTF-8")` after it. The existing `contains("not a git repository")` assertion stays and is the pin. Do not add a test that only reads `Command::get_envs`.
- [ ] **Step 2: Run it and see it fail.** On this machine git's German catalog is installed (`LANGUAGE=de LANG=en_US.UTF-8 git -C / rev-parse --show-toplevel` prints `Schwerwiegend: ...`), so the assertion fails before the fix. Quote the failing message in the report.
- [ ] **Step 3: Add `.env("LC_ALL", "C")` to `hermetic`**, with a comment that the assertions on git's error text need the English messages and that `LC_ALL=C` makes git ignore `LANGUAGE`. Run the `scan` tests: pass.
- [ ] **Step 4: Mutate once by hand:** remove the `LC_ALL` line, see the test fail, restore.
- [ ] **Step 5: Format and commit** with `test(librarian): the scan tests run git in the C locale`.

---

### Task 5: Move the `checks.rs` tests to `checks/tests.rs`

**Files:**
- Create: `src/librarian/lifecycle/checks/tests.rs`
- Modify: `src/librarian/lifecycle/checks.rs` (replace the body of `mod tests` with `mod tests;`)

**Interfaces:**
- Consumes: the tests as Tasks 1 and 3 left them.
- Produces: the same test paths, `librarian::lifecycle::checks::tests::<name>`.

- [ ] **Step 1: Record the names.** Run `scripts/with-slot.sh cargo test --lib librarian::lifecycle::checks -- --list`, sort the output, and save it to `before.txt` in the scratchpad.
- [ ] **Step 2: Move.** Cut everything inside `mod tests { ... }` (the `use` lines first: `use super::*;` and the four `use crate::...` lines) into `checks/tests.rs`, one indentation level less. In `checks.rs` leave `#[cfg(test)]` and `mod tests;`. Change no other line.
- [ ] **Step 3: Format** both files with `rustfmt --edition 2021`.
- [ ] **Step 4: Compare.** List the names again into `after.txt`; `diff before.txt after.txt` must print nothing. Run the lifecycle tests, both feature sets: all pass. `git diff --stat` shows `checks.rs` shrinking by about as many lines as `tests.rs` gains.
- [ ] **Step 5: Lint.** Run `scripts/with-slot.sh cargo clippy --workspace --all-targets --features local-embed -- -D warnings` and the same with `--no-default-features`. Expected: no warning.
- [ ] **Step 6: Commit** with `refactor(librarian): the checks tests live in checks/tests.rs`.

---

## Closing (the controller, after Task 5)

Run `./scripts/gate.sh` on the branch merged with `experiments`, as CLAUDE.md requires before a task is complete, and record the four exit codes. Fold the plan's status to `done` and the spec's note to match in the same bookkeeping commit.
