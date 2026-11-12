# Artifact Identity Plan C1 — Dual-Write Contract and Entries Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make every entry write land in the committed markdown the spec assigns, as a `## PREFIX-N — title` section with its high-water mark and its citations in the grammar, while the old catalog is still written, so the cutover (Plan D) and the read flip (Plan E) find the files already in place.

**Architecture:** Plan C1 of the spec's Plan C (C1 entries; C2 events, observations and links; C3 the content-keyed vector cache). One library module holds the dual-write contract with no `ToolContext`, so Plan D's cutover tool can call it: stage the catalog rows in the open `IMMEDIATE` transaction, write the file through `write_catalogued_file` and `record_written_file`, then commit; a failed commit is compensated or reported by name, and a retry finds the section it already wrote. `append_entry` writes every entry as markdown on both paths, bumps the committed mark, and writes its `cites` as `[[key]]`, `[[key:TOKEN]]` or `[[repo/key…]]` beside the `entry_cite` row. Operations that would make text and rows disagree (`graft`, `merge_worktree`, `rekey_prefix`) refuse instead. The CLI write commands take the project write lock. The derived index (Plan B2) is not changed; it is the test oracle.

**Tech Stack:** Rust 2021, `rusqlite` 0.39, `git2` 0.20 (tests, through `derived::testkit`), `fs4` (the existing write guard); Plan A's `identity::{key, key_space, grammar, repo_name, gate}`; Plan B2's `derived::{build, diff, schema, scan, testkit}`.

**Spec:** `docs/superpowers/specs/2026-11-11-repo-relative-artifact-key-design.md` — § The store boundary, § The citation grammar, § Writes, § Contract with ledger formats, § Index inputs, guard, machine state and entry ids, § Worktrees and machines, § Tools that change meaning. Evidence: the Plan C scout report and its review (session scratchpad `plan-c-seams.md`, `plan-c-scout-review.md`), the independent review of this plan's first draft (`plan-c1-review.md`), the B2 rulings doc (id `4c82d96fc9952e98`), session-log entries F-11, F-14, F-15 (id `abff63149e3c24e8`). Every `file:line` below was read in this worktree at `8873c012e`; line numbers are hints, find code by name (B2 ruling R7). `link_scan` lives at `crate::librarian::tools::link_scan` (`src/librarian/tools/link_scan/`).

## Global Constraints

- **Contract with ledger formats, requirement 1 (spec):** "Every entry exists as markdown: a section that defines its token, or a document of its own. No entry exists only in params." Requirement 3: "An entry's citations are written in the grammar."
- **Entry ids (spec):** "`append_entry` … allocates `max(body, frontmatter high-water) + 1`." C1 keeps `entry_reservation` (its removal is Plan E).
- **Grammar (spec):** `[[key]]`, `[[key:TOKEN]]`, `[[repo/key]]`, `[[repo/key:TOKEN]]`; a key is the lowercase stem matching `[a-z0-9]+(-[a-z0-9]+)*`, a folder index takes its folder's name, a 16-hex key is refused; "Unique per repo … There is no `Ambiguous` outcome for a key." Use `identity::key::key_of` (`identity/key.rs:127`), `identity::gate::check_tree` (`identity/gate.rs:218`), `identity::grammar::extract` (`grammar.rs:98`) and `identity::repo_name::read` (`repo_name.rs:132`); never a second parser.
- **Write door:** every file write goes through `crate::librarian::util::write_catalogued_file` (`util.rs:85`); a write into a catalogued file is followed by `record_written_file` in the same transaction, or `doctor` reports `row_behind_file`.
- **Read door:** production code under `src/librarian` reads file content only through `read_door` (`ReadDoor::new(roots).text` / `text_for_write`, `for_project`). Gate: `no_librarian_production_read_bypasses_the_read_door` (`read_door.rs:1414`). Inside an open transaction use `ReadDoor::new(roots)` with roots taken before it opened, not `for_catalog(&cat)` (the transaction holds the catalog borrow). `std::fs::metadata` is not a read under that gate.
- **No process-environment mutation** in code or tests (`tests/env_mutation_isolation.rs`, `no_site_outside_the_allowlist_mutates_the_process_environment`). Config reaches a function as an argument.
- **Caps:** a new `const` whose name holds `MAX`, `CAP`, `LIMIT` or `BUDGET` carries `// cap-class: RESULT_CAP | NOT_A_CAP` (`tests/result_caps.rs`).
- **References in output:** new response fields, refusals and hints name a document by its grammar form, or by its file name when that name is not a canonical key; never by a 16-hex id, an absolute path or a row id (spec G6, § Anti-goals). A key-collision refusal lists the colliding repo-relative paths, as `GateProblem::render` does (`gate.rs:79-100`) and as the spec's collision refusal does.
- **Agent-facing text** (tool and parameter descriptions, guides, refusals, hints, sidecar prompts) states the current situation only (`src/prompts/README.md` rule 9). Guides change, `ONBOARDING_VERSION` does not (README § Versioning).
- **Guide budgets** (`src/prompts/guide_index.rs`): the first `append_entry` call delivers at most `WRITE_CEILING_APPEND_ENTRY = 5_394 + 128` bytes (:298; "raise only with a new measurement"); every declared section stays within `MAX_DECLARED_SECTION_BYTES = 2500` (:273, `declared_sections_are_within_the_size_cap` :1480); the heading strings `"Entry ids"`, `"Entry ids — refusals: main checkout only (prose ledgers)"`, `"Entry ids — where the counter lives"` are pinned in `tracker_write_shapes_ship_their_sections_and_only_theirs` (:1198, pins at :1236-1248, ceiling loop at :1401). Guide edits are byte-neutral or smaller; if one must grow, re-measure with that test and record the new figure in the commit message. A renamed heading changes its pin in the same commit (`every_section_citation_in_the_guides_resolves_to_a_heading`, :1602, catches citations of the old name).
- **Untouched:** the derived schema, `DERIVED_TABLES`, `PARSER_VERSION`, the pinned corpus digest; the committed sidecar YAML shape (`every_committed_sidecar_parses_and_carries_no_params`, `augmentation_sidecar.rs:862`); the documented position that `doc(augment)` and `doc(create)` never create a sidecar (`augmentation_sidecar.rs:348-352`).
- **Evidence rule (B2 R6):** every test names the production mutation that turns it red; a test that passes on its first run is shown red by that mutation, then the mutation is reverted.
- **Before completing a task:** targeted tests with `scripts/with-slot.sh cargo test --lib <filter>` (or `--test <name>`); `cargo clippy --workspace --all-targets --features local-embed -- -D warnings` and with `--no-default-features`; `scripts/with-slot.sh cargo test --test result_caps`. `./scripts/gate.sh` runs after Tasks 4, 7 and 9.
- **Commit:** `git add <paths>`, then `git diff --cached --name-only` as its own call, then `scripts/commit-mine.sh -m "<message>"` (`docs/conventions/shared-checkout-commit-sequence.md`).

### Test oracles (every acceptance test names which one it uses)

- **Parse oracle (no commit):** `crate::librarian::derived::build::parse_document(rel_path, text, &classify::default_rules()?)` (`derived/build.rs:371`, pub, pure) on the bytes read back from disk: which entries a file defines and which citations it writes. It does not resolve them.
- **Commit oracle (resolved edges):** `derived::testkit::build_files(files)` (Task 3) commits the files into a temp repo through `git2` and runs `build_full`. The derived index reads commits only (`derived/scan.rs:1-4`), so a dual-written file is invisible to it until committed.
- **`codescout derive-shadow` on an uncommitted dual-write** compares the derived index of `HEAD` with the old catalog, which `record_written_file` has moved to the working-tree bytes. Expect a `changed` row on `file_sha256` with cause `Unexplained` for every dual-written file (`derived/diff.rs:35-39`, ruling R18) and edge differences for its new citations, until the file is committed and the command re-runs at the new `HEAD`. That is the dual-write window, not drift.

## Review Focus

1. **A failed catalog commit after the section reached disk, then a retry** — expected: the retry with the same title and body records the section already there and writes nothing new. Tests: Task 2 (`a_params_retry_adopts…`, `a_prose_retry_adopts…`), Task 1 (`a_prose_commit_failure_restores…`).
2. **Two writers on one ledger at once: an MCP write and a CLI `codescout doc update`** — expected: the CLI waits on the lock, names the holder when its timeout passes, and once the lock is free its write lands on the other writer's bytes instead of over them. Tests: Task 9 (`cli_doc_create_waits_for_the_project_lock_and_names_its_holder`, `a_cli_update_after_the_lock_is_released_keeps_the_other_writers_bytes`).
3. **A `cites` ref whose target has a non-canonical name, lies outside the key space, or shares its key (F-14: 26 duplicate-key groups)** — expected: the whole append aborts and nothing is allocated or written. Tests: Task 3 (unit), Task 5 (`a_cite_that_names_a_shared_key_aborts…`, `a_cite_outside_the_key_space_aborts…`).
4. **A cite of a document in another repository** — expected: `[[repo/key]]` from the target's committed repo name; a refusal naming `.codescout/repo.toml` when it has none. Tests: Task 3 (`a_foreign_target_is_qualified_by_its_committed_repo_name`, `a_foreign_target_with_no_committed_name_is_refused`).
5. **A live worktree shadow or a `//` duplicate row of the cited file in the catalog** — expected: neither counts as a second holder of the key. Test: Task 3 (`worktree_shadows_and_double_slash_rows_are_not_collisions`).
6. **A worktree session appends to a params ledger** — expected: refused as on the prose path, with the `workspace=<main root>` route named. Test: Task 7 (`params_allocation_is_refused_from_a_worktree`).
7. **A fold whose renumbered entry has a heading in the SOURCE (worktree file, or `from` document), against a survivor that has its own heading** — expected: `graft` refuses; `merge_worktree` skips the row, leaves the registration active and names `abandon` as the only exit; a source with no heading for that id renumbers as before even when the survivor has headings. Tests: Task 7 (`a_graft_that_would_renumber_a_markdown_defined_entry_is_refused`, `a_survivor_heading_alone_does_not_block_a_renumber`, `a_fold_whose_worktree_section_merged_into_main_skips_the_row`).
8. **`rekey_prefix` on a ledger another document cites in grammar text** — expected: refused, naming the citing documents. Test: Task 8.

---

### Task 1: The dual-write contract

**Files:**
- Create: `src/librarian/catalog/dual_write.rs`
- Modify: `src/librarian/catalog/mod.rs` (`pub(crate) mod dual_write;` beside `pub mod augmentation;` at :9); `src/librarian/catalog/augmentation.rs` — move `record_written_file` (:2307-2330), `SectionRollback` (:1090-1099), `restore_section_after_failed_commit` (:1108-1137), `commit_failed_after_section_write` (:1152-1200) and their tests (`restoring_a_section_still_restores_a_regular_file`, `a_file_changed_by_another_writer_is_left_alone_rather_than_restored`, `an_unreadable_path_reports_failed_not_restored`, `the_rolled_back_and_still_on_disk_remedies_point_in_opposite_directions`) into the new module; make `splice_pending_section` (:2345) `pub(crate)`; route `allocate_entry_id` (:1831-2025) and the params `append_entry` (:810-1086, section write :1010-1060) through the new functions.
- Test: `src/librarian/catalog/dual_write.rs`

**Interfaces:**
- Consumes: `augmentation::{PendingSection, splice_pending_section, entry_high_water_key, body_entry_heading_level}`, `frontmatter::{parse, upsert_int_line}` (`frontmatter.rs:387`), `util::write_catalogued_file`, `current_project::lookup_git_root` (`current_project.rs:55`), `identity::key::key_of`, `crate::librarian::tools::link_scan::extract::entry_sections` (`extract.rs:67`; `EntrySection { id, level, heading_line, end_line, text }`, `text` includes the heading line).
- Produces:
  - `pub(crate) struct FileWrite { pub path: std::path::PathBuf, pub before: String, pub after: String }`
  - `pub(crate) fn record_written_file(tx: &rusqlite::Transaction<'_>, artifact_id: &str, path: &str, written: &str) -> anyhow::Result<()>` (moved unchanged).
  - `pub(crate) fn frontmatter_mark(doc: &str, id_prefix: &str) -> Option<u64>` — the committed `entry_high_water_<P>`, a number or a numeric string, as `allocate_entry_id` reads it today (:1912-1920).
  - `pub(crate) fn write_entry_into_ledger(tx: &rusqlite::Transaction<'_>, roots: &[std::path::PathBuf], artifact_id: &str, path: &std::path::Path, doc: &str, id_prefix: &str, num: u64, section: Option<&PendingSection>, write_mark: bool, caller: &str) -> anyhow::Result<FileWrite>` — when `write_mark`, sets the mark to `max(frontmatter_mark, num)` with `upsert_int_line` (a document with no frontmatter block is refused with today's hint, nothing written); splices `section` at `body_entry_heading_level(...).unwrap_or(2)`; writes through `write_catalogued_file`; then `record_written_file`. One file write carries mark and section. The prose path passes `write_mark: true`; the params path passes `false` in this task (Task 4 turns it on), so the params-path fixture without frontmatter in `a_params_append_writes_its_section_and_index_row_in_the_same_call` (`tools/append_entry.rs:2010`) stays green here.
  - `pub(crate) fn commit_after_file_write(tx: rusqlite::Transaction<'_>, written: Option<&FileWrite>, token: &str, caller: &str) -> anyhow::Result<()>` — commits; on a commit error restores `before` only when the file still holds `after` (today's rule) and returns a `LibrarianRecoverableError` naming what reached disk and what did not, the document by `document_label`.
  - `pub(crate) fn document_label(abs_path: &std::path::Path) -> String` — `` `[[key]]` `` when the git root and `key_of` give a key, else the file name.
  - `pub(crate) fn find_unrecorded_section(doc: &str, id_prefix: &str, title: &str, section_body: &str, is_recorded: &dyn Fn(u64) -> bool) -> Option<u64>` — the lowest `N` whose heading is `<#…> PREFIX-N — <title>`, whose section text after the heading equals `section_body` once both are trimmed and a leading `**Valid:** dated YYYY-MM-DD` line is dropped from each, and for which `is_recorded(N)` is false. Pure.

**Which writers change order, and which stay:**
- `append_entry`, params path: already stages rows, writes the section, then commits, with compensation (`augmentation.rs:1001-1072`). Order stays; now through the shared functions.
- `append_entry`, prose path: mark and section inside the transaction, then a bare `tx.commit()?` (~:2015). **Changes:** the commit goes through `commit_after_file_write`.
- `doc(update)` (file first, `update.rs:854`; `file_written_but_catalog_failed` :242), `doc(create)` (catalog first, then `std::fs::write` with `undo_failed_create`, `create.rs:506-583`), `event_create` (frontmatter first `event_create.rs:391`, then the transaction :476-493) and `doc(augment)` (catalog first, then the sidecar behind `preflight_write_through`, `augment.rs:391`) stay: each already reports its partial state or (create) its order protects a retry, and C1 adds no committed home to them.
- A known window, not closed here: after a failed commit SQLite's lock is released before the restore runs, so a peer can read our section, the restore removes it, and the peer writes the bytes it read back. Task 2's adoption is what makes the caller's retry correct in that case.

- [ ] **Step 1: Write the failing tests** in `dual_write.rs`:

```rust
#[test]
fn one_file_write_carries_the_mark_and_the_section_and_the_row_records_its_sha() {
    // ledger fixture: frontmatter `entry_prefix: F`, anchor `## Template for new entries`
    let w = write_entry_into_ledger(&tx, &[], "art1", &md, &before, "F", 1, Some(&pending("first")), true, "test").unwrap();
    tx.commit().unwrap();
    let text = std::fs::read_to_string(&md).unwrap();
    assert!(text.contains("entry_high_water_F: 1"));
    assert_eq!(text.matches("## F-1 — first").count(), 1);
    assert_eq!(w.after, text);
    assert_eq!(row_sha(&cat, "art1"), crate::librarian::util::sha_of_bytes(text.as_bytes()));
}
// mutation: drop the record_written_file call → the sha assertion goes red.

#[test]
fn the_mark_never_moves_down() { /* mark 9 on disk, num 4 → file still says entry_high_water_F: 9 */ }
// mutation: write `num` instead of `max(mark, num)`.

#[test]
fn write_mark_false_leaves_the_frontmatter_alone_and_accepts_a_file_without_one() {
    // "# Q\n\n## Template\n" (no frontmatter), write_mark false → Ok, section written, no `entry_high_water`
}
// mutation: ignore `write_mark` → Err (no frontmatter block).

#[test]
fn a_document_with_no_frontmatter_is_refused_when_the_mark_is_written() { /* Err; file == before */ }
// mutation: fall back to skipping the mark when upsert_int_line returns None → Ok, red.

#[test]
fn a_prose_commit_failure_restores_the_file_and_says_retry_as_is() {
    // a deferred-FK tripwire fired by the `entry_reservation` INSERT (shape of augmentation.rs `arm_commit_tripwire`, :5843)
    let err = allocate_entry_id(&mut cat, "art1", "F", Some(&pending("first"))).unwrap_err();
    assert_eq!(std::fs::read_to_string(&md).unwrap(), before);
    assert!(hint_of(err).contains("safe to retry as-is"));
}
// mutation: put back the bare `tx.commit()?` → the file keeps `## F-1`.

#[test]
fn find_unrecorded_section_ignores_the_validity_stamp_and_skips_recorded_tokens() {
    let doc = "---\nentry_prefix: F\n---\n\n## F-4 — first\n\n**Valid:** dated 2026-01-01\n\nprose\n\n## Template for new entries\n";
    assert_eq!(find_unrecorded_section(doc, "F", "first", "prose", &|_| false), Some(4));
    assert_eq!(find_unrecorded_section(doc, "F", "first", "prose", &|n| n == 4), None);
    assert_eq!(find_unrecorded_section(doc, "F", "first", "other prose", &|_| false), None);
    assert_eq!(find_unrecorded_section(doc, "F", "second", "prose", &|_| false), None);
}
// mutation: compare titles only → the third assertion goes red.

#[test]
fn document_label_is_the_grammar_form_or_the_file_name() {
    // tempdir with `.git/`: docs/ledger.md → "`[[ledger]]`"; docs/Bad_Name.md → "Bad_Name.md"
}
// mutation: return the absolute path → both assertions red.
```

- [ ] **Step 2: Run** `scripts/with-slot.sh cargo test --lib catalog::dual_write` → FAIL (module missing).
- [ ] **Step 3: Implement** the module and the moves; the moved texts name `document_label(path)` where they named `{path}`, and the moved test's path assertions follow. No other behaviour changes.
- [ ] **Step 4: Run** `scripts/with-slot.sh cargo test --lib catalog::` and `scripts/with-slot.sh cargo test --lib tools::append_entry` → PASS; every existing test stays green.
- [ ] **Step 5: Commit** — `refactor(librarian): one dual-write contract for entry writes; the prose path compensates a failed commit`.

---

### Task 2: A retry finds the section it already wrote

**Files:**
- Modify: `src/librarian/catalog/augmentation.rs` (`append_entry`, `allocate_entry_id`, `AppendOutcome` :780, `AllocateOutcome` :1204), `src/librarian/catalog/dual_write.rs` (the still-on-disk remedy text), `src/librarian/tools/append_entry.rs` (responses and `next_step`, :397-412 and :463-470)
- Test: `src/librarian/catalog/augmentation.rs` (`mod tests`), `src/librarian/catalog/dual_write.rs`

**Interfaces:**
- Consumes: Task 1.
- Produces: `pub adopted: bool` on `AppendOutcome` and `AllocateOutcome`; the tool responses carry `"adopted": true` only when set.

Rule: when the call carries a section, `find_unrecorded_section` runs over the body read inside the transaction, before allocation. Params path: `is_recorded(n)` = params hold `<P>-n`. Prose path: `is_recorded(n)` = `reserved_max.is_some_and(|r| n <= r)`. A hit is adopted: that id is used, the params row (params path) or the reservation (prose path) and any `entry_cite` rows are written for it, and the file is written only when its mark is below `n` (`write_entry_into_ledger` with `section: None`, `write_mark: true`).

- [ ] **Step 1: Write the failing tests:**

```rust
#[test]
fn a_params_retry_adopts_a_section_the_file_has_and_params_lack() {
    // body already holds `## F-1 — first\n\n**Valid:** dated 2026-01-01\n\nprose`, params empty
    let out = append_entry(&mut cat, "art1", "failures", "F", json!({}), &[], Some(&pending("first"))).unwrap();
    assert_eq!((out.id.as_str(), out.adopted), ("F-1", true));
    assert_eq!(std::fs::read_to_string(&md).unwrap().matches("## F-").count(), 1);
    assert_eq!(params_ids(&cat, "art1"), vec!["F-1"]);
}
// mutation: skip the adoption lookup on the params path → F-2 and two sections.

#[test]
fn a_prose_retry_adopts_the_section_and_records_the_reservation() {
    // section F-1 on disk, mark 1, no entry_reservation row
    let out = allocate_entry_id(&mut cat, "art1", "F", Some(&pending("first"))).unwrap();
    assert_eq!((out.id.as_str(), out.adopted), ("F-1", true));
    assert_eq!(reserved_max(&cat, "art1", "F"), Some(1));
    assert_eq!(std::fs::read_to_string(&md).unwrap(), before);
}
// mutation: skip the adoption lookup on the prose path → F-2.

#[test]
fn a_different_body_under_the_same_title_is_a_new_entry() { /* F-2, adopted false */ }
// mutation: pass `""` as section_body to the finder → adopted, red.

#[test]
fn a_recorded_section_is_never_adopted() { /* params hold F-1 → F-2 */ }
// mutation: `is_recorded = |_| false` → adopts F-1, red.
```

In `dual_write.rs`, rename `the_rolled_back_and_still_on_disk_remedies_point_in_opposite_directions` to `the_two_remedies_name_their_own_retry`: the restored branch says `safe to retry as-is`; the still-on-disk branch says `retry with the same title and body`, names the token, and does not say `safe to retry as-is`. Mutation: give both branches one text.

- [ ] **Step 2: Run** `scripts/with-slot.sh cargo test --lib catalog::augmentation catalog::dual_write` → FAIL.
- [ ] **Step 3: Implement** the rule; an adopted outcome's `next_step` says the entry was already in the file, is now recorded, and nothing new was written.
- [ ] **Step 4: Run** those filters plus `tools::append_entry` → PASS.
- [ ] **Step 5: Commit** — `feat(librarian): an append retried after a partial write records the section it already wrote`.

---

### Task 3: Citations as grammar text, and the commit oracle

**Files:**
- Create: `src/librarian/catalog/cite_text.rs`
- Modify: `src/librarian/catalog/mod.rs`; `src/librarian/derived/diff.rs` (make `scope_of` :550-569 and its `Scope` enum `pub(crate)`); `src/librarian/derived/testkit.rs`; `src/librarian/tools/doctor.rs` (the cross-root remedy hint :1640-1645 and its pin `cross_root_cites_edges_is_published_at_zero_with_a_working_remedy` :9316-9339)
- Test: `src/librarian/catalog/cite_text.rs`, `src/librarian/derived/testkit.rs`, `src/librarian/tools/doctor.rs`

**Interfaces:**
- Consumes: the `artifact` table (`id`, `abs_path`, `slug`, `missing_since`); `derived::diff::{scope_of, Scope}`; `identity::{key::key_of, key_space::load (key_space.rs:114), gate::{check_tree, GateProblem}, repo_name::{read, RepoName}}`; `current_project::lookup_git_root`; `util::{root_and_prefix, path_at_or_under_sql}` (`util.rs:311`, `:302`); `derived::{scan::open_commit, build::{rules_from_commit, key_space_from_commit, build_full, BuildInput}, schema::open_in_memory}`, `testkit::repo_with`.
- Produces:
  - `pub(crate) const CITES_LABEL: &str = "**Cites:**";`
  - `pub(crate) fn grammar_form_of(conn: &rusqlite::Connection, source_git_root: &std::path::Path, dst_ref: &str) -> anyhow::Result<String>` — `dst_ref` is what `resolve_cite_ref` (`augmentation.rs:2457`) returns: with a `:`, `<slug>:<local>` (the artifact holding that slug, token `local`); without, an artifact id (any string: the rel_path arm returns whatever id the row has, `:2508-2530`). Steps: target git root by `lookup_git_root` (none → refused); target rel by `scope_of(target_root, abs_path)` (`Inside(rel)` only); population = `scope_of` of every non-missing artifact row under the target root, `Inside` only, so `.worktrees/` shadows and `//` duplicate rows drop out (as the shadow diff does, `diff.rs:13-20`); `check_tree(name, &key_space::load(target_root)?, &population)` where `name` is `repo_name::read(target_root)?` or the literal `repo`; refused when the target rel is outside the key space, or `check_tree` reports an `InvalidKey` for it or a `DuplicateKey` whose `rel_paths` hold it (the refusal carries `GateProblem::render()`). Qualification: same root as `source_git_root` → `[[key]]` / `[[key:TOKEN]]`; another root → its `repo_name::read` name; none committed → refused, naming `.codescout/repo.toml`; a name equal to the source root's committed name (another checkout of one repository) → unqualified; otherwise `[[name/key]]` / `[[name/key:TOKEN]]`.
  - `pub(crate) fn cites_line(forms: &[String]) -> String` — `**Cites:** [[a]], [[b:F-3]]`, input order, exact repeats dropped.
  - `pub(crate) fn with_cites(section: &PendingSection, forms: &[String]) -> PendingSection` — `body.trim_end()` + `"\n\n"` + `cites_line(forms)`; a plain clone when `forms` is empty.
  - In `testkit.rs`: `pub(crate) struct Built { pub conn: rusqlite::Connection, _dir: tempfile::TempDir }`, `pub(crate) fn build_files(files: &[(&str, &str)]) -> Built` (one commit through `repo_with`; `open_commit(HEAD)`, `rules_from_commit`, `key_space_from_commit`, `build_full` into `open_in_memory`, root `/fixture/root`), `pub(crate) fn entry_tokens(conn: &rusqlite::Connection, rel_path: &str) -> Vec<String>`, `pub(crate) fn edges_from(conn: &rusqlite::Connection, rel_path: &str) -> Vec<(String, Option<String>, Option<String>, Option<String>)>` (`kind`, `target_repo`, `target_key`, `target_token`, by `ord`; kinds are the strings of `derived/build.rs:597-610`).
  - The doctor cross-root hint adds that the foreign repository needs a committed `.codescout/repo.toml` name; its pin also asserts `repo.toml` appears.

Old-scanner side effect, recorded and not fixed here: `cross_repo_re` (`tools/link_scan/extract.rs:287-290`) matches `key:F-3` inside `[[name/key:F-3]]` and resolves it by local stem, so an entry-grain cross-repo cite can add a wrong `origin='scan'` row when the local repo has a file with that stem; the `origin='write'` row is the correct one. A folder-index target (`[[area:F-3]]`) gives the old scanner a cross-repo finding, not an edge, and a one-letter key (`[[a:F-1]]`) falls back to a bare `F-1`.

- [ ] **Step 1: Write the failing tests** (temp dirs with a `.git/` directory; in-memory catalog with artifact rows):

```rust
#[test] fn a_file_ref_becomes_a_key_and_an_entry_ref_a_key_with_its_token() {
    assert_eq!(grammar_form_of(&cat.conn, &root, "id-target").unwrap(), "[[target]]");
    assert_eq!(grammar_form_of(&cat.conn, &root, "target-slug:F-3").unwrap(), "[[target:F-3]]");
    assert_eq!(grammar_form_of(&cat.conn, &root, "id-area-readme").unwrap(), "[[area]]");
}
// mutation: use the file stem instead of key_of → "[[readme]]".
#[test] fn a_non_canonical_target_name_is_refused_by_name() { /* docs/Bad_Name.md → Err containing "Bad_Name.md" */ }
// mutation: drop the InvalidKey arm → Ok, red.
#[test] fn a_target_outside_the_key_space_is_refused() { /* .codescout/key-space.toml excludes contrib/** ; target contrib/x.md → Err */ }
// mutation: drop the is_outside check → Ok("[[x]]").
#[test] fn a_shared_key_is_refused_naming_both_files() {
    let msg = format!("{:#}", grammar_form_of(&cat.conn, &root, "id-a-notes").unwrap_err());
    assert!(msg.contains("docs/a/notes.md") && msg.contains("docs/b/notes.md"));
}
// mutation: drop the DuplicateKey arm → Ok("[[notes]]").
#[test] fn worktree_shadows_and_double_slash_rows_are_not_collisions() {
    // rows: <root>/docs/t.md, <root>/.worktrees/feat/docs/t.md, <root>//docs/t.md, and one with missing_since
    assert_eq!(grammar_form_of(&cat.conn, &root, "id-t").unwrap(), "[[t]]");
}
// mutation: build the population from raw rel paths instead of scope_of → DuplicateKey, red.
#[test] fn a_foreign_target_is_qualified_by_its_committed_repo_name() {
    // other/.git, other/.codescout/repo.toml name = "other-repo"; source root differs
    assert_eq!(grammar_form_of(&cat.conn, &root, "id-foreign").unwrap(), "[[other-repo/notes]]");
}
// mutation: ignore source_git_root → "[[notes]]".
#[test] fn a_foreign_target_with_no_committed_name_is_refused() { /* Err containing "repo.toml" */ }
// mutation: fall back to an unqualified key → Ok, red.
#[test] fn another_checkout_of_the_same_repository_stays_unqualified() { /* both roots carry name = "same" → "[[t]]" */ }
// mutation: qualify whenever the roots differ → "[[same/t]]".
#[test] fn cites_line_keeps_order_and_drops_repeats() {
    assert_eq!(cites_line(&["[[b]]".into(), "[[a:F-1]]".into(), "[[b]]".into()]), "**Cites:** [[b]], [[a:F-1]]");
}
// mutation: sort the forms → "[[a:F-1]], [[b]]".
#[test] fn the_commit_oracle_sees_what_a_commit_holds() {      // testkit.rs
    let b = build_files(&[("docs/ledger.md", "## F-1 — x\n\n**Cites:** [[target]]\n"), ("docs/target.md", "# T\n")]);
    assert_eq!(entry_tokens(&b.conn, "docs/ledger.md"), vec!["F-1"]);
    assert_eq!(edges_from(&b.conn, "docs/ledger.md"), vec![("file".into(), None, Some("target".into()), None)]);
}
// mutation: build from an empty tree → both red.
```

Doctor pin (`doctor.rs:9339`): add `assert!(hint.contains("repo.toml"))`. Mutation: revert the hint sentence.

- [ ] **Step 2: Run** `scripts/with-slot.sh cargo test --lib catalog::cite_text derived::testkit tools::doctor::tests::cross_root` → FAIL.
- [ ] **Step 3: Implement.** **Step 4: Run** → PASS, and `scripts/with-slot.sh cargo test --lib derived::` (the `scope_of` visibility change must not change a dump).
- [ ] **Step 5: Commit** — `feat(librarian): render a resolved citation as grammar text, qualified across repositories; a commit oracle`.

---

### Task 4: The params path writes the entry into the markdown

**Files:**
- Modify: `src/librarian/catalog/augmentation.rs` (`append_entry`: allocation and `write_mark: true`); `src/librarian/tools/append_entry.rs` (`call`: the section refusal on the params path; the three recipe gates `every_taxonomy_append_entry_recipe_is_one_the_code_accepts` :2824, `every_sidecar_prompt_append_entry_call_is_one_the_code_accepts` :3537, `every_ledger_append_entry_recipe_is_one_the_code_accepts` :4101 and their call reader `prompt_calls`/`SidecarCall` :3299-3340); `src/librarian/tools/artifact.rs` (`entry_collection` text :455, `anchor_heading` text :488, `id_prefix` text, the `doc` description :247; the `append_entry` case of `a_write_names_the_row_it_landed_on_in_a_worktree_session_and_the_artifact_otherwise` :596-606); `src/server.rs` (the `append_entry` call at :17799-17808); `docs/TAXONOMY.md` (rows T :121, WIN :122, PV :124, RC :125, DC :132); `src/prompts/guides/librarian.md` (:184-185); `src/prompts/guides/tracker-conventions.md` (§ Entry ids :187-195); the `docs/augmentations/*.yaml` prompts that the sidecar gate reports.
- Test: `src/librarian/catalog/augmentation.rs`, `src/librarian/tools/append_entry.rs`

**Interfaces:**
- Consumes: Tasks 1-2.
- Produces: the tool refuses a params call (`entry_collection` given) that lacks `title`, `body` or `anchor_heading`, before any catalog access, with a `LibrarianRecoverableError` saying an entry on a params ledger is written into the markdown too and naming the missing fields. The library `augmentation::append_entry` keeps `section: Option<&PendingSection>` (its only production caller is the tool, `tools/append_entry.rs:443`); `None` there still means a params-only row, which library tests (`merge_worktree.rs:624`, `:639`; `augmentation.rs:4968` `append_entry_tolerates_a_body_missing_from_disk`) and Plan D's export rely on. With a section: `next = max(params_next, body_max + 1, frontmatter_mark + 1)` and the mark is written (`write_mark: true`). No `entry_reservation` on this path.

Migration, all in this task:
- Tool-level tests that call the params path without a section gain `title`, `body`, `anchor_heading` in their args and the anchor in their fixture; fixtures with no frontmatter gain one (`a_params_append_writes_its_section_and_index_row_in_the_same_call`, `:2010`, seeds `"# Q\n\n| ID |…"`). The `append_entry` case at `artifact.rs:604` and the server walk at `server.rs:17799` gain the three fields. Step 2's run lists the rest.
- The three recipe gates also require, for every params call they read (`collection.is_some()`), the arguments `title`, `body` and `anchor_heading`; a call missing one is a finding naming the file and the missing fields.
- `docs/TAXONOMY.md` rows T, WIN, PV, RC, DC: the recipe becomes one `append_entry` call carrying `title`, `body`, `anchor_heading` (and `index_row` where the row had one); the "then a section via `doc(update)`" step goes. Edit with `edit_file`; if the librarian guard refuses the file, use `doc(action="update", patch={body_edits:[…]})`.
- The sidecar prompts the gate reports: changed through the route the repo sanctions for a shape change, `doc(action="augment", merge=true, augment={prompt: …})` (the remedy `sidecar_shape_drift` names, `doctor.rs:7386-7405`), pinned to this worktree (`workspace=<worktree root>`): `augment` forks a shadow (`augment.rs:386`) and `write_through` rewrites the worktree's own `docs/augmentations/*.yaml`. Two consequences, stated in the commit message: the global catalog now holds shadow rows for those ledgers, which `merge_worktree` folds (it carries params and frontmatter only, not the prompt, `merge_worktree.rs:415-420`); and after the branch merges, the main catalog's rows lag the committed sidecars until someone applies them with `doc(augment)` from the main checkout, which `doctor`'s `sidecar_shape_drift` reports with that remedy. Hand-editing the YAML is not the route.
- Schema texts: `entry_collection` no longer says a prose call "reserves … without writing anything" as the only way and says the params path writes the section; `anchor_heading` is no longer "prose ledgers —" only; `id_prefix` names the frontmatter mark among the inputs; the `doc` description says the params path writes the section and needs the three fields, within its cap (`every_tool_description_is_under_its_cap`, `server.rs:4054`).
- Guides: byte-neutral or smaller (Global Constraints).

- [ ] **Step 1: Write the failing tests:**

```rust
// tools/append_entry.rs
#[tokio::test] async fn a_params_append_without_a_section_is_refused_and_allocates_nothing() {
    let err = call(&ctx, json!({"id": "art1", "entry_collection": "failures", "id_prefix": "F", "entry": {}})).await.unwrap_err();
    assert!(format!("{err:#}").contains("anchor_heading"));
    assert!(params_ids(&ctx, "art1").is_empty());
}
// mutation: remove the tool-layer refusal → Ok, red.
#[test] fn a_params_call_without_a_section_is_a_recipe_finding() {
    // feed the sidecar gate's reader a prompt holding `doc(action="append_entry", id="x", entry_collection="c", id_prefix="T", entry={…})`
    assert!(findings.iter().any(|f| f.contains("anchor_heading")));
}
// mutation: drop the new condition from the gate → no finding, red.

// augmentation.rs
#[test] fn a_params_append_writes_the_section_and_raises_the_mark() {          // parse oracle
    let out = append_entry(/* … */ Some(&pending("first"))).unwrap();
    let text = std::fs::read_to_string(&md).unwrap();
    assert!(text.contains("entry_high_water_F: 1"));
    let parsed = parse_document("docs/ledger.md", &text, &default_rules().unwrap());
    assert_eq!(parsed.entries.iter().map(|e| e.token.as_str()).collect::<Vec<_>>(), vec![out.id.as_str()]);
}
// mutation: leave `write_mark: false` on the params path → no mark, red.
#[test] fn the_committed_mark_counts_on_the_params_path() { /* frontmatter mark 7, params empty, no heading → F-8 */ }
// mutation: drop `frontmatter_mark + 1` from the max → F-1.
#[test] fn a_params_ledger_with_no_frontmatter_is_refused_with_a_section() { /* Err; file == before; params unchanged */ }
// mutation: pass write_mark false again → Ok, red.
```

- [ ] **Step 2: Run** `scripts/with-slot.sh cargo test --lib catalog::augmentation tools::append_entry tools::artifact` and `scripts/with-slot.sh cargo test --lib server::tests` → FAIL, including the existing tests and gates to migrate.
- [ ] **Step 3: Implement** and migrate as listed.
- [ ] **Step 4: Run** the same, plus `scripts/with-slot.sh cargo test --lib prompts::` and `scripts/with-slot.sh cargo test --lib augmentation_sidecar` → PASS. Then `./scripts/gate.sh` → green.
- [ ] **Step 5: Commit** — `feat(librarian): a params-ledger entry is written into the markdown with its mark`.

---

### Task 5: Params-path citations as grammar text

**Files:**
- Modify: `src/librarian/catalog/augmentation.rs` (`append_entry`, `AppendOutcome`), `src/librarian/tools/append_entry.rs` (response), `src/librarian/tools/artifact.rs` (`cites` text :484), `src/prompts/guides/tracker-conventions.md` (one sentence; byte-neutral)
- Test: `src/librarian/catalog/augmentation.rs`, `src/librarian/tools/append_entry.rs`

**Interfaces:**
- Consumes: Tasks 1-4.
- Produces: `AppendOutcome` gains `pub cites: Vec<String>` (grammar forms written) and `pub key_note: Option<String>`; the tool response carries `"cites"` when non-empty and `"key_note"` when set.

Inside the transaction: each ref goes through `resolve_cite_ref` and `grammar_form_of(&tx, &ledger_git_root, &dst_ref)` (ledger git root from `lookup_git_root(abs_path)`; none → refused); any refusal aborts the call with nothing written; the section is `with_cites(section, &forms)`, and Task 2's adoption compares against that rendered body; the `entry_cite` `origin='write'` rows stay (:981-998). `key_note` is set when `key_of` fails for the ledger itself: the entry is written and its bare token resolves inside the ledger, but no `[[key:TOKEN]]` can name it until the file is renamed. The `cites` text says each ref is written into the entry as grammar text and recorded as an edge, that a ref naming a key two files share is refused, and that refs to another repository need its committed name.

The old scanner beside the write row: `cross_repo_re` matches `target:F-3` inside `[[target:F-3]]` and derives `dst_ref = <dst_slug>:F-3` (`tools/link_scan/mod.rs:183-201`), the shape `resolve_cite_ref` stores; `entry_cite`'s primary key leaves `origin` out (`entry_cite.rs:16-28`), so the scan's insert is ignored and the write row stays. A file cite has no scanner twin (the old scanner does not parse `[[`), so the write row is its only old-catalog edge until the flip.

- [ ] **Step 1: Write the failing tests** (fixtures with a `.git/` directory; `/test/<id>.md` fixtures with no git root are moved under a temp root):

```rust
#[test] fn params_cites_are_grammar_text_and_write_rows() {                    // parse oracle
    let out = append_entry(/* … */ &["id-other".into(), "target-slug:F-3".into()], Some(&pending("first"))).unwrap();
    let parsed = parse_document("docs/ledger.md", &std::fs::read_to_string(&md).unwrap(), &rules);
    assert!(parsed.citations.iter().any(|c| c.reference == Reference::File { repo: None, key: Key::parse("other").unwrap() }));
    assert!(parsed.citations.iter().any(|c| c.reference == Reference::Entry { repo: None, key: Key::parse("target").unwrap(), token: "F-3".into() }));
    assert_eq!(write_rows(&cat, &out.id), 2);
    assert_eq!(out.cites, vec!["[[other]]", "[[target:F-3]]"]);
}
// mutation: skip with_cites → no citation parsed.
#[test] fn a_cite_that_names_a_shared_key_aborts_the_whole_append() {          // Review Focus 3
    assert!(append_entry(/* cite docs/a/notes.md while docs/b/notes.md exists */).is_err());
    assert_eq!(std::fs::read_to_string(&md).unwrap(), before);
    assert!(params_ids(&cat, "art1").is_empty());
}
// mutation: resolve the grammar form after the file write → file changed, red.
#[test] fn a_cite_outside_the_key_space_aborts_the_whole_append() { /* target under an excluded subtree; file == before */ }
// mutation: catch the refusal and drop the cite → Ok, red.
#[test] fn a_dual_written_entry_resolves_in_the_derived_index_after_a_commit() { // commit oracle
    let b = build_files(&[("docs/ledger.md", &ledger_text), ("docs/target.md", "## F-3 — t\n"), ("docs/other.md", "# O\n")]);
    let edges = edges_from(&b.conn, "docs/ledger.md");
    assert!(edges.contains(&("entry".into(), None, Some("target".into()), Some("F-3".into()))));
    assert!(edges.contains(&("file".into(), None, Some("other".into()), None)));
}
// mutation: write the cites line inside an HTML comment → no edges, red.
#[test] fn a_ledger_whose_own_name_is_not_a_key_still_takes_the_entry_and_says_so() {
    // ledger docs/Ledger_One.md → Ok; key_note mentions "Ledger_One.md"
}
// mutation: never set key_note → None, red.
```

In `tools/append_entry.rs` (context set-up as `tools/link_scan/mod.rs:1477-1505`):

```rust
#[tokio::test] async fn a_write_scan_after_a_cited_append_keeps_one_write_row() {
    // append with cites ["target-slug:F-3"]; target has the F-3 params row and a `## F-3 — t` heading
    let out = link_scan::call(&ctx, json!({ "write": true })).await.unwrap();
    assert_eq!(out["counts"]["entry_edges"]["skipped_existing"], json!(1));
    assert_eq!(rows_for(&cat, &new_id), vec![("target-slug:F-3".to_string(), "write".to_string())]);
}
// mutation: insert the write row with dst_ref `target:F-3` (the key, not the slug) → written 1, red.
```

If that test shows `written: 1` before any mutation, stop and report it; the assertion is not loosened.

The existing cites tests (`append_with_cites_writes_entry_cite_and_not_artifact_link`, `:1713`, and those that seed `/test/<id>.md`, `:501-546`) move their fixtures under a temp root with `.git/`.

- [ ] **Step 2: Run** `scripts/with-slot.sh cargo test --lib catalog::augmentation tools::append_entry tools::link_scan` → FAIL. **Step 3: Implement.** **Step 4: Run** → PASS, and `every_tool_description_is_under_its_cap`.
- [ ] **Step 5: Commit** — `feat(librarian): params-ledger citations are written into the entry as grammar text`.

---

### Task 6: The prose path writes `cites` too

**Files:**
- Modify: `src/librarian/catalog/augmentation.rs` (`allocate_entry_id`), `src/librarian/tools/append_entry.rs` (the prose `cites` refusal :228-235; the call at :335; response)
- Test: both files

**Interfaces:**
- Consumes: Tasks 1-3; `artifact::ensure_slug`, `entry_cite::insert_with`.
- Produces: `pub fn allocate_entry_id_with_cites(cat: &mut Catalog, artifact_id: &str, id_prefix: &str, section: Option<&PendingSection>, cites: &[String]) -> Result<AllocateOutcome>`; `allocate_entry_id(cat, artifact_id, id_prefix, section)` calls it with `&[]`, so its 31 callers (28 in `augmentation.rs` tests, `tools/append_entry.rs:335` and `:1276`, `tools/link_dir_tests.rs:1498`) do not change. `AllocateOutcome` gains `pub cites: Vec<String>`.

Why the refusal goes: it says "cite in prose — link_scan derives the edges from the text" (`tools/append_entry.rs:229-233`); writing the citation into the section text is that, done by the server. `cites` with no section stays refused before any allocation. The prose path inserts the same `origin='write'` rows as the params path.

- [ ] **Step 1: Write the failing tests:**

```rust
#[test] fn prose_cites_are_grammar_text_and_write_rows() {                      // parse oracle
    let out = allocate_entry_id_with_cites(&mut cat, "art1", "F", Some(&pending("first")), &["id-other".into()]).unwrap();
    let parsed = parse_document("docs/ledger.md", &std::fs::read_to_string(&md).unwrap(), &rules);
    assert!(parsed.citations.iter().any(|c| matches!(&c.reference, Reference::File { key, .. } if key.as_str() == "other")));
    assert_eq!(write_rows(&cat, &out.id), 1);
}
// mutation: skip with_cites on the prose path → no citation parsed.
#[test] fn prose_cites_without_a_section_are_refused_and_reserve_nothing() { /* Err; reserved_max None; file == before */ }
// mutation: drop the no-section refusal → Ok with an id and no text, red.
#[test] fn a_prose_cite_of_a_non_canonical_name_aborts_and_reserves_nothing() { /* Err; reserved_max None */ }
// mutation: insert the reservation before resolving cites and keep it on error → Some(1), red.
```

Rename `a_prose_ledger_refuses_cites` (`tools/append_entry.rs:1046`) to `a_prose_ledger_takes_cites_with_a_section_and_refuses_them_without` and assert both halves. Mutation: restore the unconditional refusal.

- [ ] **Step 2: Run** → FAIL. **Step 3: Implement.** **Step 4: Run** `scripts/with-slot.sh cargo test --lib catalog::augmentation tools::append_entry` → PASS.
- [ ] **Step 5: Commit** — `feat(librarian): a prose-ledger entry takes cites and writes them as grammar text`.

---

### Task 7: No allocation from a worktree; no renumber that splits params from markdown

**Files:**
- Modify: `src/librarian/tools/append_entry.rs` (the prose path's shadow guards :236-300 become one function used by both paths; the params path's cites-only worktree refusal :425-440 is subsumed); `src/librarian/catalog/graft.rs` (`graft_rows` :129, `merge_augmentation` :440); `src/librarian/tools/merge_worktree.rs` (`merge_one` :231, after the fold :318-323 and before the `dry_run` return; `call` :146-148); `src/prompts/guides/tracker-conventions.md` (§ "Entry ids — refusals: main checkout only (prose ledgers)" :197-201 becomes "… (every ledger)") and its pin in `src/prompts/guide_index.rs:1241`
- Test: `tools/append_entry.rs`, `catalog/graft.rs`, `tools/merge_worktree.rs`, `tools/artifact.rs`

**Interfaces:**
- Consumes: `tools::worktree::{is_main_checkout_artifact, is_worktree_shadow, has_fork_record}` (`worktree.rs:28`, `:72`, `:44`), `tools::link_scan::extract::entry_sections`, `read_door::ReadDoor`.
- Produces:
  - `fn refuse_allocation_off_the_main_ledger(ctx: &ToolContext, cat: &Catalog, id: &str) -> Result<()>` (private, `tools/append_entry.rs`), called before `resolve_write_target` on both paths. Its hint names the route: the same call pinned to the main checkout with `workspace=<main root>`.
  - `pub(crate) fn renumbered_ids_defined_in(remap: &std::collections::BTreeMap<String, String>, source_body: &str) -> Vec<String>` (`graft.rs`, pure): the old ids in `remap` that a heading defines in the SOURCE body.
  - `pub(crate) fn ids_defined_twice(remap: &std::collections::BTreeMap<String, String>, body: &str) -> Vec<String>` (`graft.rs`, pure): the old ids that two or more headings define in `body`.
  - `merge_augmentation` gains `roots: &[std::path::PathBuf]` (its one caller, `graft_rows`, takes `cat.write_roots()` before the transaction).

Behaviour:
- Worktree: a worktree session, or any session naming a shadow, gets the prose path's refusal on the params path too. Once the params path writes `## F-N` into the worktree's file, a concurrent main-checkout append merges two definers of one token (`doctor`'s `entry_defined_twice`, `doctor.rs:4867`).
- `graft_rows`: when the fold renumbers ids and `renumbered_ids_defined_in(remap, from_body)` is non-empty, the graft is refused with a `LibrarianRecoverableError` naming the ids and the ledgers by `document_label`, saying nothing changed. Only the `from` (source) body is read, through `ReadDoor::new(roots)`, and only when `remap` is non-empty, so `doc(move)` (destination never augmented) reads nothing new and a survivor's own headings never block. An unreadable source refuses (fail closed).
- `merge_one`: the source is the shadow's file when it exists (`std::fs::metadata(..).is_file()`); when the worktree directory is gone (the normal case: `check_rebase_invariant` wants the branch merged first, `merge_worktree.rs:160-197`), the check is `ids_defined_twice(remap, main_body)` over main's post-merge file instead. A hit returns `MergeOneOutcome::skipped` with conflict kind `entry_renumber_needs_markdown`, the ids, and a remedy: no tool renumbers one entry in params and markdown (`update_entry` refuses `id`; `rekey_prefix` moves a whole prefix), so `merge_worktree(abandon=true)` is the only exit; it drops the shadow's catalog rows, and the entries remain as markdown in the merged file. `call` leaves the registration `active` (no `reg::set_status(…, "merged")`) when any row was skipped with that kind, and says so in its response.
- Params-only collisions renumber as before.

- [ ] **Step 1: Write the failing tests:**

```rust
// tools/append_entry.rs — replaces `append_from_worktree_lands_on_shadow_not_main` (:1165)
#[tokio::test] async fn params_allocation_is_refused_from_a_worktree() {        // Review Focus 6
    let err = call(&wt_ctx, json!({"id": main_id, "entry_collection": "items", "id_prefix": "F",
        "title": "t", "body": "b", "anchor_heading": "## Template for new entries"})).await.unwrap_err();
    let text = format!("{err:#}");
    assert!(text.contains("not supported from a worktree") && text.contains("workspace="));
    assert!(no_shadow_row(&ctx));
}
// mutation: call the guard after resolve_write_target → a shadow row exists, red.
#[tokio::test] async fn params_allocation_is_refused_on_a_shadow_from_a_main_checkout_session() { /* has_fork_record */ }
// mutation: drop the has_fork_record arm from the shared guard → Ok, red.
```

Also: `append_with_cites_from_worktree_is_refused` (`:1769`) asserts the shared refusal's text; the doc comment of `prose_allocation_is_refused_from_a_worktree` (`:1193-1196`), which names the reversed test as its proof, is rewritten to the current rule; the `append_entry` case of `a_write_names_the_row_it_landed_on_in_a_worktree_session_and_the_artifact_otherwise` (`artifact.rs:604`) is dropped from the worktree half and kept in the non-worktree control.

```rust
// graft.rs                                                                     // Review Focus 7
#[test] fn a_graft_that_would_renumber_a_markdown_defined_entry_is_refused() {
    // from: params [F-1], body `## F-1 — x`; into: params [F-1], body `## F-1 — y`
    assert!(format!("{:#}", graft_rows(&mut cat, "from", "into").unwrap_err()).contains("F-1"));
    assert!(row_exists(&cat.conn, "from").unwrap());
}
// mutation: skip the check → Ok, from deleted, red.
#[test] fn a_survivor_heading_alone_does_not_block_a_renumber() {
    // from: params [F-1], no heading; into: params [F-1], body `## F-1 — y` → report.remap == {F-1: F-2}
}
// mutation: check both bodies → refused, red.
#[test] fn renumbered_ids_defined_in_reads_headings_not_prose() {
    let remap = BTreeMap::from([("F-1".into(), "F-3".into()), ("F-2".into(), "F-4".into())]);
    assert_eq!(renumbered_ids_defined_in(&remap, "## F-1 — x\n\nsee F-2\n"), vec!["F-1"]);
}
// mutation: match tokens anywhere in the text → ["F-1", "F-2"].

// merge_worktree.rs
#[tokio::test] async fn a_fold_whose_worktree_section_merged_into_main_skips_the_row() {
    // worktree dir absent; main file holds `## F-2 — main` and `## F-2 — wt`; both params hold F-2
    let out = call(&ctx, json!({"root": "/repo/.worktrees/feat"})).await.unwrap();
    assert_eq!(out["conflicts"][0]["kind"], json!("entry_renumber_needs_markdown"));
    assert!(out["conflicts"][0]["remedy"].as_str().unwrap().contains("abandon"));
    assert_eq!(reg::get(&ctx.catalog.lock(), "/repo/.worktrees/feat").unwrap().unwrap().status, "active");
}
// mutation: set the registration merged unconditionally → "merged", red.
#[tokio::test] async fn a_fold_with_one_main_definer_still_renumbers() { /* main file defines F-2 once → merged, remap {F-2: F-3} */ }
// mutation: use renumbered_ids_defined_in over main's body → skipped, red.
```

`merge_folds_delta_without_duplicating_base_entries` (`merge_worktree.rs:611`) calls the library with `section: None` and keeps passing (no file, params-only).

- [ ] **Step 2: Run** `scripts/with-slot.sh cargo test --lib tools::append_entry catalog::graft tools::merge_worktree tools::artifact prompts::guide_index` → FAIL. **Step 3: Implement**; the guide heading rename and its pin change together, byte count within budget. **Step 4: Run** → PASS. Then `./scripts/gate.sh` → green.
- [ ] **Step 5: Commit** — `feat(librarian): entry ids are allocated on the main ledger only, and a fold never renumbers a markdown-defined entry`.

---

### Task 8: `rekey_prefix` refuses while another document cites the old prefix in grammar text

**Files:**
- Modify: `src/librarian/catalog/rekey.rs` (`rekey_prefix_rows` :566-833, before any write; the inbound loop it already runs is at :655-700)
- Test: `src/librarian/catalog/rekey.rs`

**Interfaces:**
- Consumes: `entry_cite` (`dst_ref LIKE '<slug>:<from>-%' AND origin = 'write'`), `identity::grammar::{extract, Reference}`, `identity::key::key_of`, `read_door::ReadDoor`, Task 1 `document_label`.
- Produces: `fn grammar_citers_of(tx: &rusqlite::Transaction<'_>, roots: &[std::path::PathBuf], ledger_slug: &str, ledger_key: &str, from_prefix: &str) -> anyhow::Result<Vec<String>>` (private): the documents other than the ledger whose inbound write rows name the old prefix AND whose text, read through `ReadDoor::new(roots)`, holds a `Reference::Entry` with `key == ledger_key` and a token `<from>-<n>` (qualified or not). Non-empty → `rekey_prefix_rows` refuses, in dry run and apply alike, naming each citer by `document_label` and saying their `**Cites:**` text must be rewritten first.

- [ ] **Step 1: Write the failing tests:**

```rust
#[test] fn a_rekey_is_refused_while_another_document_cites_the_prefix_in_grammar_text() {
    // ledger docs/ledger.md defines F-1; docs/other.md section holds `**Cites:** [[ledger:F-1]]` and a write row (other-slug, R-1, ledger-slug:F-1)
    let err = rekey_prefix_rows(&mut cat, "ledger", "F", "G", RekeyMode::Apply).unwrap_err();
    assert!(format!("{err:#}").contains("[[other]]"));
    assert!(std::fs::read_to_string(&ledger).unwrap().contains("## F-1"));
}
// mutation: return an empty Vec from grammar_citers_of → rekey applies, red.
#[test] fn a_write_row_with_no_grammar_text_does_not_block() { /* legacy row, citer text has no [[…]] → Ok */ }
// mutation: refuse on rows alone → Err, red.
```

- [ ] **Step 2: Run** `scripts/with-slot.sh cargo test --lib catalog::rekey` → FAIL. **Step 3: Implement.** **Step 4: Run** → PASS.
- [ ] **Step 5: Commit** — `feat(librarian): rekey_prefix refuses while another document cites the prefix in grammar text`.

---

### Task 9: The CLI write commands take the project write lock

**Files:**
- Create: `src/cli/write_lock.rs`, `tests/cli_write_lock.rs`
- Modify: `src/cli/mod.rs` (module); `src/cli/doc.rs` (`run_create` :497, `run_update` :634, `run_move` :663, `run_link` :697); `src/cli/doc_event.rs` (`run_create` :113); `src/cli/doc_augment.rs` (`run` :41, before `run_with_ctx`); `src/cli/doctor.rs` (`run` :166, when `--fix` is given); `src/cli/audit_doc_refs.rs` (`run` :76, unless `--no-emit-tracker`)

**Interfaces:**
- Consumes: `crate::agent::{acquire_write_guard, open_lock_file, holder_record_path, WriteGuard}` (`agent/mod.rs:10-12`); `acquire_write_guard(async_mutex: Arc<tokio::sync::Mutex<()>>, file: Arc<std::fs::File>, holder_path: PathBuf, timeout: Duration, holder: &str) -> Result<WriteGuard, crate::tools::RecoverableError>` (`agent/write_guard.rs:229-235`); `ProjectConfig::load_or_default` (`config/project.rs:862`) for `security.write_lock_timeout_secs` (default 5); `ToolContext.current_project` (`CurrentProject { abs_path, git_root, .. }`, `current_project.rs:23`).
- Produces:
  - `pub(crate) fn lock_root(cp: &CurrentProject) -> std::path::PathBuf` — the nearest directory from `cp.abs_path` up to and including `cp.git_root` that holds `.codescout/project.toml`; else `cp.git_root`.
  - `pub(crate) async fn take(ctx: &crate::librarian::tools::ToolContext, command: &str) -> anyhow::Result<WriteGuard>` — `lock_root`, the timeout, then `take_at`; no current project is an error that names `--project`.
  - `pub(crate) async fn take_at(root: &std::path::Path, timeout: std::time::Duration, command: &str) -> anyhow::Result<WriteGuard>` — a fresh `Arc<tokio::sync::Mutex<()>>`, `open_lock_file(root)`, `holder_record_path(root)`, holder `codescout-cli <command>`; the `RecoverableError` is converted into the `anyhow::Error` the CLI prints.
  - `fn takes_the_write_lock(args: &DoctorArgs) -> bool` (`fix.is_some()`, mirroring the MCP doctor arm in `adapter.rs`) and `fn takes_the_write_lock(args: &AuditArgs) -> bool` (`!no_emit_tracker`, mirroring `adapter.rs:491-493`).

How a CLI process finds the root today: `open_ctx` (`cli/mod.rs:62`) hands `--project` to `build_tool_context` (`librarian/mod.rs:309`), whose `current_project::resolve` canonicalises the path and walks up to the nearest `.git`. MCP locks `<ActiveProject.root>/.codescout/write.lock`, the canonical launch, activate or pin directory with no walk-up (`agent/mod.rs:1024-1053`, `server.rs:1109-1118`). `lock_root` reaches the same file when that directory is the nearest one holding a `project.toml`; it differs when MCP is activated at a directory without one, or for an umbrella member, and then the two processes take different lock files. The module doc says so, and that the spec's per-ledger file lock (Plan E) is the fix. The lock is taken after `open_ctx` and held until the tool body returns; MCP takes the flock before SQLite, and the CLI keeps that order. Commands that do not write (`find`, `get`, `graph`, `state-at`, `event list`, `refresh gather|list-stale`) take none. `backfill-chunks` (catalog rows only) and the non-librarian `index`, `migrate-memories` and `rebuild-memory-vectors` commands (`src/main.rs:55`, `:136`, `:155`), which write state MCP guards, are outside this task.

- [ ] **Step 1: Write the failing tests.** Unit, in `write_lock.rs`:

```rust
#[test] fn the_lock_root_is_the_nearest_project_toml_within_the_git_root() {
    // repo/.git, repo/.codescout/project.toml, cwd repo/sub/dir → repo; repo2/.git only, cwd repo2/x → repo2
}
// mutation: return cp.abs_path → repo/sub/dir, red.
#[tokio::test] async fn a_held_lock_refuses_the_cli_and_names_the_holder() {
    let held = acquire_write_guard(Arc::new(tokio::sync::Mutex::new(())), open_lock_file(&root).unwrap(),
        holder_record_path(&root), Duration::from_secs(1), "codescout:sid-beta append_entry").await.unwrap();
    let err = take_at(&root, Duration::from_millis(200), "doc update").await.unwrap_err();
    assert!(format!("{err:#}").contains("codescout:sid-beta append_entry"));
    drop(held);
    assert!(take_at(&root, Duration::from_millis(200), "doc update").await.is_ok());
}
// mutation: open the lock file under root.join("sub") → Ok while held, red.
#[test] fn doctor_takes_the_lock_only_with_a_fix() { /* None → false; Some("export_augmentations") → true */ }
// mutation: return true always → the None case red.
#[test] fn audit_doc_refs_takes_the_lock_unless_it_emits_no_tracker() { /* default → true; --no-emit-tracker → false */ }
// mutation: return false always → the default case red.
```

Integration (`CARGO_BIN_EXE_codescout` makes Cargo build the binary; the fixture's `project.toml` is `[project]\nname = "t"\n[security]\nwrite_lock_timeout_secs = 1\n`, which `ProjectConfig` parses: `ProjectSection` needs only `name`, `config/project.rs:45-85`), modelled on `tests/cross_process_write_lock.rs:110-165`:

```rust
#[test] fn cli_doc_create_waits_for_the_project_lock_and_names_its_holder() {     // Review Focus 2
    // test process: fs4 try_lock_exclusive on .codescout/write.lock; holder record "<now>\tcodescout:sid-beta append_entry"
    let out = codescout(&["doc", "create", "--project", dir, "--kind", "note", "--title", "T", "--rel-path", "docs/t.md"]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("codescout:sid-beta append_entry"));
    assert!(!dir.join("docs/t.md").exists());
}
// mutation: remove the `take` call from run_create → exit 0 and docs/t.md exists.
#[test] fn a_cli_update_after_the_lock_is_released_keeps_the_other_writers_bytes() {  // Review Focus 2
    // create docs/t.md through the CLI first (no lock held); then the test process takes the flock,
    // spawns `codescout doc update <id> --status done` with write_lock_timeout_secs = 5, appends a line
    // "peer line" to docs/t.md's body while holding the lock, and releases it after 500 ms
    let text = std::fs::read_to_string(dir.join("docs/t.md")).unwrap();
    assert!(text.contains("peer line") && text.contains("status: done"));
}
// mutation: remove the `take` call from run_update → the update reads before the peer's append and its write drops "peer line" (run it under the mutation several times; it must go red at least once in five).
```

- [ ] **Step 2: Run** `scripts/with-slot.sh cargo test --lib cli::write_lock` and `scripts/with-slot.sh cargo test --test cli_write_lock` → FAIL.
- [ ] **Step 3: Implement**, and wire `take` into the eight commands above.
- [ ] **Step 4: Run** both → PASS; `scripts/with-slot.sh cargo test --test cross_process_write_lock` → PASS. Then `./scripts/gate.sh` → green.
- [ ] **Step 5: Commit** — `feat(cli): write commands take the project write lock`.

---

## Rulings made in this plan

| # | Ruling | Why | Cost if wrong |
|---|---|---|---|
| C1-R1 | File-first means: stage rows in the open `IMMEDIATE` transaction, write the file and `record_written_file`, then commit; a failed commit restores the bytes only when untouched, else reports them by name. | The commit is the catalog's durable step; the transaction is also the allocation lock and must be open before the body is read. | A crash between write and commit leaves a file-only entry; Task 2 repairs it on retry. |
| C1-R2 | Order changes only on the prose `append_entry` path; `update`, `create`, `event_create`, `augment` stay. | Each already reports its partial state, or (create) protects its retry; C1 gives them no new committed home. | A Plan D caller that needs another order re-opens this. |
| C1-R3 | The retry key is (prefix, heading title, section text minus the `**Valid:**` stamp) over sections the catalog has not recorded. "Recorded" is machine-local (params rows; `entry_reservation`), so on a fresh clone or a second machine every committed section counts as unrecorded, and an identical title and body adopts that entry. | No new marker in committed text; also records a section a caller wrote by hand first. | Two deliberately identical entries, or another machine's identical entry, become one. |
| C1-R4 | Params path: section required, mark folded into allocation and written; no `entry_reservation` here. | Spec contract requirement 1 and § Entry ids; the params row is the claim. | A params ledger with no frontmatter block is refused until it gains one. |
| C1-R5 | Citations are one `**Cites:**` line at the end of the section, input order, repeats dropped. | Spec: relations are labelled grammar lines in the body. | A later label vocabulary (C2) renames the label. |
| C1-R6 | A cite is refused when its target has no git root, a non-canonical key, lies outside the key space, or shares its key (per `check_tree` over the old catalog's `Inside` rows of the target's repo). | Spec: one key per document; F-14's 26 duplicate groups. | Files not yet indexed are not seen; the commit oracle reports such a collision as `key_collision`. |
| C1-R7 | The `entry_cite` `origin='write'` row stays on both paths. | The old scanner does not read `[[key]]`, so file cites would leave the old catalog otherwise. | Plan D drops rows whose text twin exists. |
| C1-R8 | The prose path accepts `cites` when it writes a section, refuses them otherwise. | The refusal's reason ("cite in prose") is what the server now does. | none found. |
| C1-R9 | Params-path allocation is refused from a worktree, with the `workspace=<main root>` route in the hint. | A section in the worktree's file plus a main-checkout append merges into two definers of one token. | Worktree sessions append to params ledgers from the main checkout only; reverses `append_from_worktree_lands_on_shadow_not_main`. Open for the operator. |
| C1-R10 | `graft` refuses and `merge_worktree` skips a renumber of an id the SOURCE defines by heading (main's post-merge file when the worktree is gone); the registration stays active; `abandon` is the only exit; params-only renumbers continue. | No tool renumbers one entry in both stores; a guessed heading rewrite cannot tell two `## F-N` apart; a survivor's own headings say nothing about the source. | A legacy shadow with such a collision is abandoned and its entries survive only as markdown. |
| C1-R11 | CLI lock root: nearest `project.toml` up to the git root, else the git root; eight librarian commands guarded; holder `codescout-cli <command>`. | Same lock file as MCP in the ordinary layouts, no new config. | An MCP session activated at a directory without `project.toml`, or an umbrella member, takes another lock file; Plan E's per-ledger lock is the fix. |
| C1-R12 | New outputs and refusals name documents as grammar forms or file names. | Spec G6. | none. |
| C1-R13 | A ledger whose own name is not a key still takes entries; the response carries `key_note`. | The entry is real and its bare token resolves locally; renames belong to the cutover. | none. |
| C1-R14 | Sidecar creation is removed from C1 (controller). The documented "never create" position stays. | Nothing in C1 consumes it; Plan D's `export_augmentations` pass produces the same end state in one window without committing new files mid-stream. | Augmented documents created before the window stay catalog-only until then. |
| C1-R15 | Cross-repo cites are written `[[repo/key…]]` from the target's committed repo name and refused when it has none; the doctor cross-root hint says so (controller ruling on finding 1). | `resolve_cite_ref` resolves across the machine-global catalog; an unqualified key would resolve in the wrong repo. | Cites into unnamed repositories are refused until onboarding names them; the old scanner may add a wrong scan row for an entry-grain cross-repo cite. |
| C1-R16 | The section requirement lives in the tool; the library keeps `Option<&PendingSection>`. | The tool is the only production caller; library tests and Plan D's export write params rows without a section. | A new production caller of the library could skip the markdown; review duty. |
| C1-R17 | The sidecar prompts are changed in the branch through worktree-pinned `doc(augment, merge=true)`, not by hand. | It is the route the repo documents for a shape change (`sidecar_shape_drift` remedy). | After merge the main catalog's rows lag the committed sidecars until `doc(augment)` is applied from main, which `doctor` reports. |
| C1-R18 | `rekey_prefix` refuses while another document's text cites the old prefix in the grammar and a write row backs it (controller ruling c). | One small task; leaves no text naming a moved token. | Hand-written `[[ledger:F-n]]` text with no write row is not detected; the derived index reports it as `undefined_token` after the rekey. |
| C1-R19 | The plan has 9 tasks; Task 4 of the first draft is split into params markdown (4) and params citations (5). | One reviewable pass each. | none. |

## Open for the operator

1. **Refusing params `append_entry` from a worktree (C1-R9).** It changes what worktree sessions can do today. Recommendation: refuse, under the conditions built into Task 7 (the hint names `workspace=<main root>`; the source-only renumber check; the registration not marked merged after a skip).

## Not built here, on purpose

- **Sidecar creation on `doc(augment)` and `doc(create)`:** Plan D's `export_augmentations` pass. If it is built later, `create_for` should upsert the one `expects_augmentation:` line (as `upsert_int_line` does for the mark) rather than re-emit the frontmatter through `frontmatter::write` (`stamp_augmentation_declaration`, `doctor.rs:2147-2168`, which also does not call `record_written_file`), and it must update `src/prompts/guides/librarian-runtime.md:100-110` and the module docs of `augmentation_sidecar.rs`.
- Events, observations, links, `event_edges`, `sources`, `commit_refresh`, and ULIDs in committed text: Plan C2.
- The content-keyed vector cache and the persistent derived-file location: Plan C3.
- The git-history cache and `doc(find)` ordering by a catalog clock (`catalog/find.rs:28`): later; the review recommends Plan E.
- Indexing sidecar-declared shapes into the derived catalog: the plan that builds a persistent derived file.
- `codescout-37`'s entry-file format and the L-B tools: C1 writes today's prose-section format only.
- `update_entry` regenerating a section's prose from changed fields; params field values that live only in the catalog: Plan D's export and `codescout-37`'s format.
- `cites` given as `key:TOKEN` or `[[…]]`, and `<slug>:<local>` cites of prose-ledger entries (`resolve_cite_ref` checks params rows only).
- A `rekey_prefix` that rewrites foreign `**Cites:**` text instead of refusing; a `doctor` check for hand-written grammar citations of a rekeyed prefix: Plan E's rename work.
- A CLI lock for `backfill-chunks`, `index`, `migrate-memories`, `rebuild-memory-vectors`: outside the librarian document writes this plan covers.
- The machine-state file, `entry_reservation` removal, the per-ledger entry-id file lock, and the deletion tail: Plan E.

## After the plan

- From the main checkout, after the branch lands: apply the committed sidecar prompts to the main catalog rows with `doc(action="augment", merge=true, augment={prompt: …})` until `doctor` reports no `sidecar_shape_drift`; merge or abandon the worktree registration Task 4's prompt edits created.
- Record the rulings and any `derive-shadow` numbers taken after a dual-write in the session log (id `abff63149e3c24e8`).
- Plan D reuses `dual_write::{write_entry_into_ledger, commit_after_file_write, find_unrecorded_section, record_written_file}` and `cite_text::grammar_form_of` to export the 38 params-only entries and rewrite citations.
