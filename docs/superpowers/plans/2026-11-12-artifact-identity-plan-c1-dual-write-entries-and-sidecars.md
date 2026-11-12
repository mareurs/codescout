# Artifact Identity Plan C1 — Dual-Write Contract and Entries Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make every entry write land in the committed markdown the spec assigns, as a `## PREFIX-N — title` section with its high-water mark and its citations in the grammar, while the old catalog is still written, so the cutover (Plan D) and the read flip (Plan E) find the files already in place.

**Architecture:** Plan C1 of the spec's Plan C (C1 entries; C2 events, observations and links; C3 the content-keyed vector cache). One library module holds the dual-write contract with no `ToolContext`, so Plan D's cutover tool can call it: stage the catalog rows in the open `IMMEDIATE` transaction, write the file through `write_catalogued_file` and `record_written_file`, then commit; a failed commit is compensated or reported by name, and a params-path retry finds the section it already wrote. `append_entry` writes every entry as markdown on both paths, bumps the committed mark, and writes its `cites` as `[[key]]`, `[[key:TOKEN]]` or `[[repo/key…]]` beside the `entry_cite` row. Operations that would make text and rows disagree (`graft`, `merge_worktree`, `rekey_prefix`) refuse instead. The CLI write commands take the project write lock. The derived index (Plan B2) is not changed; it is the test oracle.

**Tech Stack:** Rust 2021, `rusqlite` 0.39, `git2` 0.20, `fs4` (the existing write guard); Plan A's `identity::{key, key_space, grammar, repo_name, gate}`; Plan B2's `derived::{build, diff, schema, scan, testkit}`.

**Spec:** `docs/superpowers/specs/2026-11-11-repo-relative-artifact-key-design.md` — § The store boundary, § The citation grammar, § Writes, § Contract with ledger formats, § Index inputs, guard, machine state and entry ids, § Worktrees and machines, § Tools that change meaning. Evidence: the Plan C scout report and its review (session scratchpad `plan-c-seams.md`, `plan-c-scout-review.md`), the two independent reviews of this plan (`plan-c1-review.md`, and the re-review of `a5b5afdae`), the B2 rulings doc (id `4c82d96fc9952e98`), session-log entries F-11, F-14, F-15 (id `abff63149e3c24e8`). Every `file:line` below was read in this worktree at `8873c012e`; line numbers are hints, find code by name (B2 ruling R7). **Both revisions were verified by reading code only; no cargo command was run.** `link_scan` lives at `crate::librarian::tools::link_scan` (`src/librarian/tools/link_scan/`).

## Global Constraints

- **Contract with ledger formats, requirement 1 (spec):** "Every entry exists as markdown: a section that defines its token, or a document of its own. No entry exists only in params." Requirement 3: "An entry's citations are written in the grammar."
- **Entry ids (spec):** "`append_entry` … allocates `max(body, frontmatter high-water) + 1`." C1 keeps `entry_reservation` (its removal is Plan E).
- **Grammar (spec):** `[[key]]`, `[[key:TOKEN]]`, `[[repo/key]]`, `[[repo/key:TOKEN]]`; a key is the lowercase stem matching `[a-z0-9]+(-[a-z0-9]+)*`, a folder index takes its folder's name, a 16-hex key is refused; "Unique per repo … There is no `Ambiguous` outcome for a key." Use `identity::key::key_of` (`identity/key.rs:127`), `identity::gate::check_tree` (`identity/gate.rs:218`), `identity::grammar::{extract, parse_target}` (`grammar.rs:98`, `:51`) and `identity::repo_name::read` (`repo_name.rs:132`); never a second parser.
- **Write door:** every file write goes through `crate::librarian::util::write_catalogued_file` (`util.rs:85`); a write into a catalogued file is followed by `record_written_file` in the same transaction, or `doctor` reports `row_behind_file`.
- **Read door:** production code under `src/librarian` reads file content only through `read_door` (`ReadDoor::new(roots).text` / `text_for_write`, `for_project`); git blobs through `git2`. Gate: `no_librarian_production_read_bypasses_the_read_door` (`read_door.rs:1414`). Inside an open transaction use `ReadDoor::new(roots)` with roots taken before it opened, not `for_catalog(&cat)`. `std::fs::metadata` is not a read under that gate. A read the door refuses is a refusal of the operation (fail closed), never a skip.
- **No process-environment mutation** in code or tests (`tests/env_mutation_isolation.rs`, `no_site_outside_the_allowlist_mutates_the_process_environment`).
- **Caps:** a new `const` whose name holds `MAX`, `CAP`, `LIMIT` or `BUDGET` carries `// cap-class: RESULT_CAP | NOT_A_CAP` (`tests/result_caps.rs`).
- **No dead code between tasks:** `cargo clippy -D warnings` runs after every task and the crate has no crate-level `allow(dead_code)` (`src/main.rs` has one item-level `cfg_attr` at :263 only). An item a later task first calls carries `#[cfg_attr(not(test), allow(dead_code))]` until that task removes it (Task 3 → Task 7).
- **References in output:** new response fields, refusals and hints name a document by its grammar form, or by its file name when that name is not a canonical key; never by a 16-hex id, an absolute path or a row id (spec G6, § Anti-goals). A key-collision refusal lists the colliding repo-relative paths, as `GateProblem::render` does (`gate.rs:79-100`).
- **Agent-facing text** (tool and parameter descriptions, guides, refusals, hints, sidecar prompts) states the current situation only (`src/prompts/README.md` rule 9). Guides change, `ONBOARDING_VERSION` does not.
- **Tool-surface budget:** `tool_surface_under_budget` (`src/server.rs:6614`) sums every advertised description and schema against `TOOL_SURFACE_CHAR_BUDGET = 57_688` (:6611), which was set to an exact measured total. A schema or description edit keeps its character count, or raises the budget to the measured total (`cargo test --lib tool_surface_report_lengths -- --nocapture`) with an entry in that constant's log saying what the bytes bought (:6644-6652). Also `every_tool_description_is_under_its_cap` (`server.rs:4054`).
- **Guide budgets** (`src/prompts/guide_index.rs`): the first `append_entry` delivers at most `WRITE_CEILING_APPEND_ENTRY = 5_394 + 128` bytes (:298; "raise only with a new measurement"); every declared section stays within `MAX_DECLARED_SECTION_BYTES = 2500` (:273, `declared_sections_are_within_the_size_cap` :1480); the headings `"Entry ids"`, `"Entry ids — refusals: main checkout only (prose ledgers)"`, `"Entry ids — where the counter lives"` are pinned in `tracker_write_shapes_ship_their_sections_and_only_theirs` (:1198, pins :1236-1248, ceiling loop :1401). Guide edits are byte-neutral or smaller; if one must grow, re-measure with that test and record the figure in the commit message. A renamed heading changes its pin in the same commit (`every_section_citation_in_the_guides_resolves_to_a_heading`, :1602).
- **Untouched:** the derived schema, `DERIVED_TABLES`, `PARSER_VERSION`, the pinned corpus digest; the committed sidecar YAML shape (`every_committed_sidecar_parses_and_carries_no_params`, `augmentation_sidecar.rs:862`); the documented position that `doc(augment)` and `doc(create)` never create a sidecar (`augmentation_sidecar.rs:348-352`).
- **Librarian-managed trackers under `docs/trackers/` are not edited from this worktree** (an edit through `doc(update)` from a worktree forks a shadow). Ledger text that needs changing is pinned in a gate's shrinking known-list and fixed from the main checkout after the plan.
- **Evidence rule (B2 R6):** every test names the production mutation that turns it red; a test that passes on its first run is shown red by that mutation, then the mutation is reverted.
- **Before completing a task:** targeted tests with `scripts/with-slot.sh cargo test --lib <filter>` (or `--test <name>`); `cargo clippy --workspace --all-targets --features local-embed -- -D warnings` and with `--no-default-features`; `scripts/with-slot.sh cargo test --test result_caps`. `./scripts/gate.sh` runs after Tasks 4, 6, 8 and 10.
- **Commit:** `git add <paths>`, then `git diff --cached --name-only` as its own call, then `scripts/commit-mine.sh -m "<message>"` (`docs/conventions/shared-checkout-commit-sequence.md`).

### Test oracles (every acceptance test names which one it uses)

- **Parse oracle (no commit):** `crate::librarian::derived::build::parse_document(rel_path, text, &classify::default_rules()?)` (`derived/build.rs:371`, pub, pure) on the bytes read back from disk.
- **Commit oracle (resolved edges):** `derived::testkit::build_files(files)` (Task 3) commits the files into a temp repo and runs `build_full`. The derived index reads commits only (`derived/scan.rs:1-4`).
- **`codescout derive-shadow` on an uncommitted dual-write** shows a `changed` row on `file_sha256` with cause `Unexplained` for every dual-written file (`derived/diff.rs:35-39`, ruling R18) and edge differences for its new citations, until the file is committed and the command re-runs at the new `HEAD`. That is the dual-write window, not drift.

## Review Focus

1. **A failed catalog commit after the section reached disk, then a retry** — expected: on the params path the retry with the same title and body records the section already there; on the prose path the refusal says to delete the section first when it could not be rolled back. Tests: Task 1 (`a_prose_commit_failure_restores…`), Task 2 (`a_params_retry_adopts…`, `the_two_remedies_name_their_own_retry`).
2. **Two writers on one ledger at once: an MCP write and a CLI `codescout doc update`** — expected: the CLI waits on the lock, names the holder when its timeout passes, and once the lock is free its write lands on the other writer's bytes. Tests: Task 10 (`cli_doc_create_waits…`, `a_cli_update_waits_for_the_lock_and_keeps_the_other_writers_bytes`).
3. **A `cites` ref whose target has a non-canonical name, lies outside the key space, or shares its key (F-14: 26 duplicate-key groups)** — expected: the whole append aborts, nothing allocated or written. Tests: Task 3 (unit), Task 7 (`a_cite_that_names_a_shared_key_aborts…`, `a_cite_outside_the_key_space_aborts…`).
4. **A cite of a document in another repository** — expected: `[[repo/key]]` from the target's committed name; refused naming `.codescout/repo.toml` when it has none. Tests: Task 3.
5. **A live worktree shadow or a `//` duplicate row of the cited file in the catalog** — expected: neither counts as a second holder of the key. Test: Task 3 (`worktree_shadows_and_double_slash_rows_are_not_collisions`).
6. **A worktree session appends to a params ledger** — expected: refused as on the prose path, the hint naming `workspace=<main root>`. Test: Task 4 (`params_allocation_is_refused_from_a_worktree`).
7. **A fold whose renumbered entry has a heading in the SOURCE (worktree file, its branch tip when the directory is gone, or the `from` document), against a survivor with its own heading or with a params-only row** — expected: `graft` refuses; `merge_worktree` skips the row, leaves the registration active and names `abandon`; a source with no heading for that id renumbers as before. Tests: Task 4.
8. **`rekey_prefix` on a ledger cited in grammar text, by another document or by itself** — expected: refused for another document; the ledger's own `[[key:F-n]]` moves with the prefix. Tests: Task 9.

---

### Task 1: The dual-write contract

**Files:**
- Create: `src/librarian/catalog/dual_write.rs`
- Modify: `src/librarian/catalog/mod.rs` (`pub(crate) mod dual_write;` beside `pub mod augmentation;` at :9); `src/librarian/catalog/augmentation.rs` — move `record_written_file` (:2307-2330), `SectionRollback` (:1090-1099), `restore_section_after_failed_commit` (:1108-1137), `commit_failed_after_section_write` (:1152-1200) and the five tests that call them (`restoring_a_section_never_writes_through_a_link` :3224, `restoring_a_section_still_restores_a_regular_file` ~:3245, `a_file_changed_by_another_writer_is_left_alone_rather_than_restored` ~:5976, `an_unreadable_path_reports_failed_not_restored` ~:6004, `the_rolled_back_and_still_on_disk_remedies_point_in_opposite_directions` ~:6028) into the new module; import `dual_write::record_written_file` where `augmentation.rs` still calls it (`append_entry` :1052, `allocate_entry_id` :2013, `resync_snapshot_row` :2293); make `splice_pending_section` (:2345) `pub(crate)`; route `allocate_entry_id` (:1831-2025) and the params `append_entry` section write (:1010-1072) through the new functions.
- Test: `src/librarian/catalog/dual_write.rs`

**Interfaces:**
- Consumes: `augmentation::{PendingSection, splice_pending_section, entry_high_water_key, body_entry_heading_level}`, `frontmatter::{parse, upsert_int_line}` (`frontmatter.rs:387`), `util::write_catalogued_file`, `current_project::lookup_git_root` (`current_project.rs:55`), `identity::key::key_of`.
- Produces:
  - `pub(crate) struct FileWrite { pub path: std::path::PathBuf, pub before: String, pub after: String }`
  - `pub(crate) fn record_written_file(tx: &rusqlite::Transaction<'_>, artifact_id: &str, path: &str, written: &str) -> anyhow::Result<()>` (moved unchanged).
  - `pub(crate) fn frontmatter_mark(doc: &str, id_prefix: &str) -> Option<u64>` — the committed `entry_high_water_<P>`, a number or numeric string, as read today (:1912-1920).
  - `pub(crate) fn write_entry_into_ledger(tx: &rusqlite::Transaction<'_>, roots: &[std::path::PathBuf], artifact_id: &str, path: &std::path::Path, doc: &str, id_prefix: &str, num: u64, section: Option<&PendingSection>, write_mark: bool, caller: &str) -> anyhow::Result<FileWrite>` — when `write_mark`, sets the mark to `max(frontmatter_mark, num)` with `upsert_int_line` (no frontmatter block → refused with today's hint, nothing written); splices `section` at `body_entry_heading_level(...).unwrap_or(2)`; writes through `write_catalogued_file`; then `record_written_file`. The prose path passes `true`; the params path `false` until Task 5, so `a_params_append_writes_its_section_and_index_row_in_the_same_call` (`tools/append_entry.rs:2010`, a fixture with no frontmatter) stays green.
  - `pub(crate) fn commit_after_file_write(tx: rusqlite::Transaction<'_>, written: Option<&FileWrite>, token: &str, caller: &str) -> anyhow::Result<()>` — commits; on a commit error restores `before` only when the file still holds `after` (today's rule) and returns a `LibrarianRecoverableError` naming what reached disk and what did not, the document by `document_label`. Remedy texts are today's ("safe to retry as-is" after a restore; "delete the `<token>` section … first" otherwise).
  - `pub(crate) fn document_label(abs_path: &std::path::Path) -> String` — `` `[[key]]` `` when the git root and `key_of` give a key, else the file name.

**Which writers change order, and which stay:**
- `append_entry`, params path: already stages rows, writes the section, then commits, with compensation (`augmentation.rs:1001-1072`). Order stays.
- `append_entry`, prose path: mark and section inside the transaction, then a bare `tx.commit()?` (~:2015). **Changes:** the commit goes through `commit_after_file_write`.
- `doc(update)` (file first, `update.rs:854`; `file_written_but_catalog_failed` :242), `doc(create)` (catalog first, then `std::fs::write` with `undo_failed_create`, `create.rs:506-583`), `event_create` (frontmatter first `event_create.rs:391`, then the transaction :476-493) and `doc(augment)` (catalog first, then the sidecar behind `preflight_write_through`, `augment.rs:391`) stay.
- A known window: after a failed commit SQLite's lock is released before the restore runs, so a peer can read our section, the restore removes it, and the peer writes it back. Task 2's params-path adoption makes a params retry correct then; the prose path keeps "delete first".

- [ ] **Step 1: Write the failing tests** in `dual_write.rs`:

```rust
#[test]
fn one_file_write_carries_the_mark_and_the_section_and_the_row_records_its_sha() {
    let w = write_entry_into_ledger(&tx, &[], "art1", &md, &before, "F", 1, Some(&pending("first")), true, "test").unwrap();
    tx.commit().unwrap();
    let text = std::fs::read_to_string(&md).unwrap();
    assert!(text.contains("entry_high_water_F: 1"));
    assert_eq!(text.matches("## F-1 — first").count(), 1);
    assert_eq!(w.after, text);
    // `sha_of_bytes` is the helper the code under test uses too; the mutation below still turns this red.
    assert_eq!(row_sha(&cat, "art1"), crate::librarian::util::sha_of_bytes(text.as_bytes()));
}
// mutation: drop the record_written_file call.

#[test] fn the_mark_never_moves_down() { /* mark 9 on disk, num 4 → still entry_high_water_F: 9 */ }
// mutation: write `num` instead of `max(mark, num)`.

#[test] fn write_mark_false_leaves_the_frontmatter_alone_and_accepts_a_file_without_one() { /* "# Q\n\n## Template\n" → Ok, no mark */ }
// mutation: ignore `write_mark` → Err.

#[test] fn a_document_with_no_frontmatter_is_refused_when_the_mark_is_written() { /* Err; file == before */ }
// mutation: skip the mark when upsert_int_line returns None → Ok.

#[test] fn a_prose_commit_failure_restores_the_file_and_says_retry_as_is() {
    // deferred-FK tripwire fired by the `entry_reservation` INSERT (shape of `arm_commit_tripwire`, augmentation.rs:5843)
    let err = allocate_entry_id(&mut cat, "art1", "F", Some(&pending("first"))).unwrap_err();
    assert_eq!(std::fs::read_to_string(&md).unwrap(), before);
    assert!(hint_of(err).contains("safe to retry as-is"));
}
// mutation: put back the bare `tx.commit()?` → the file keeps `## F-1`.

#[test] fn document_label_is_the_grammar_form_or_the_file_name() {
    // tempdir with `.git/`: docs/ledger.md → "`[[ledger]]`"; docs/Bad_Name.md → "Bad_Name.md"
}
// mutation: return the absolute path.
```

- [ ] **Step 2: Run** `scripts/with-slot.sh cargo test --lib catalog::dual_write` → FAIL (module missing).
- [ ] **Step 3: Implement** the module and the moves; the moved texts name `document_label(path)` where they named `{path}`, and the moved tests' path assertions follow. No other behaviour changes.
- [ ] **Step 4: Run** `scripts/with-slot.sh cargo test --lib catalog::` and `scripts/with-slot.sh cargo test --lib tools::append_entry` → PASS; clippy for both feature sets → clean.
- [ ] **Step 5: Commit** — `refactor(librarian): one dual-write contract for entry writes; the prose path compensates a failed commit`.

---

### Task 2: A retry finds the section it already wrote

**Files:**
- Modify: `src/librarian/catalog/dual_write.rs`, `src/librarian/catalog/augmentation.rs` (`append_entry`, `allocate_entry_id`, `AppendOutcome` :780, `AllocateOutcome` :1204), `src/librarian/tools/append_entry.rs` (responses and `next_step`, :397-412 and :463-470)
- Test: `src/librarian/catalog/dual_write.rs`, `src/librarian/catalog/augmentation.rs`

**Interfaces:**
- Consumes: Task 1; `crate::librarian::tools::link_scan::extract::entry_sections` (`extract.rs:67`; `EntrySection { id, level, heading_line, end_line, text }`, `text` includes the heading line).
- Produces:
  - `pub(crate) fn find_unrecorded_section(doc: &str, id_prefix: &str, title: &str, section_body: &str, is_recorded: &dyn Fn(u64) -> bool) -> Option<u64>` — the lowest `N` whose heading is `<#…> PREFIX-N — <title>`, whose section text after the heading equals `section_body` once both are trimmed and a leading `**Valid:** dated YYYY-MM-DD` line is dropped from each, and for which `is_recorded(N)` is false. Pure.
  - `pub(crate) enum RetryRule { DeleteFirst, Adopts }`; `commit_after_file_write` gains `retry: RetryRule`. The still-on-disk branch says "retry with the same title and body" for `Adopts` and keeps "delete the section first" for `DeleteFirst`.
  - `pub adopted: bool` on `AppendOutcome` and `AllocateOutcome`; responses carry `"adopted": true` only when set.

Rule: when the call carries a section, `find_unrecorded_section` runs over the body read inside the transaction, before allocation. Params path: `is_recorded(n)` = params hold `<P>-n` (exact), and its commit uses `RetryRule::Adopts`. Prose path: `is_recorded(n)` = `reserved_max.is_some_and(|r| n <= r)`; one `max_allocated` per (artifact, prefix) (`augmentation.rs:1924-1950`) means a peer that reserved `n+1` after our failed commit makes our orphan `F-n` count as recorded, so the prose commit keeps `RetryRule::DeleteFirst`; prose adoption still covers the case with no later reservation. A hit is adopted: that id is used, the params row (params path) or the reservation (prose path) and any `entry_cite` rows are written for it, and the file is written only when the path writes a mark (prose now; params from Task 5) and the mark is below `n` (`write_entry_into_ledger` with `section: None`).

- [ ] **Step 1: Write the failing tests:**

```rust
#[test] fn find_unrecorded_section_ignores_the_validity_stamp_and_skips_recorded_tokens() {
    let doc = "---\nentry_prefix: F\n---\n\n## F-4 — first\n\n**Valid:** dated 2026-01-01\n\nprose\n\n## Template for new entries\n";
    assert_eq!(find_unrecorded_section(doc, "F", "first", "prose", &|_| false), Some(4));
    assert_eq!(find_unrecorded_section(doc, "F", "first", "prose", &|n| n == 4), None);
    assert_eq!(find_unrecorded_section(doc, "F", "first", "other prose", &|_| false), None);
    assert_eq!(find_unrecorded_section(doc, "F", "second", "prose", &|_| false), None);
}
// mutation: compare titles only → the third assertion red.

#[test] fn a_params_retry_adopts_a_section_the_file_has_and_params_lack() {
    // body holds `## F-1 — first\n\n**Valid:** dated 2026-01-01\n\nprose`, params empty
    let out = append_entry(&mut cat, "art1", "failures", "F", json!({}), &[], Some(&pending("first"))).unwrap();
    assert_eq!((out.id.as_str(), out.adopted), ("F-1", true));
    assert_eq!(std::fs::read_to_string(&md).unwrap().matches("## F-").count(), 1);
    assert_eq!(params_ids(&cat, "art1"), vec!["F-1"]);
}
// mutation: skip the adoption lookup on the params path → F-2, two sections.

#[test] fn a_prose_retry_with_no_later_reservation_adopts_the_section() {
    // section F-1 on disk, mark 1, no entry_reservation row
    let out = allocate_entry_id(&mut cat, "art1", "F", Some(&pending("first"))).unwrap();
    assert_eq!((out.id.as_str(), out.adopted), ("F-1", true));
    assert_eq!(reserved_max(&cat, "art1", "F"), Some(1));
}
// mutation: skip the adoption lookup on the prose path → F-2.

#[test] fn a_different_body_under_the_same_title_is_a_new_entry() { /* F-2, adopted false */ }
// mutation: pass `""` as section_body → adopted, red.

#[test] fn a_recorded_section_is_never_adopted() { /* params hold F-1 → F-2; reserved_max 1 → prose F-2 */ }
// mutation: `is_recorded = |_| false` → adopts F-1, red.
```

In `dual_write.rs`, rename the moved `the_rolled_back_and_still_on_disk_remedies_point_in_opposite_directions` to `the_two_remedies_name_their_own_retry` and assert three texts: restored → `safe to retry as-is`; still on disk with `Adopts` → `retry with the same title and body` and the token, not `safe to retry as-is`; still on disk with `DeleteFirst` → `Delete the` + token, not `same title`. Mutation: give `Adopts` and `DeleteFirst` one text.

- [ ] **Step 2: Run** `scripts/with-slot.sh cargo test --lib catalog::augmentation catalog::dual_write` → FAIL.
- [ ] **Step 3: Implement**; an adopted outcome's `next_step` says the entry was already in the file, is now recorded, and nothing new was written.
- [ ] **Step 4: Run** those filters plus `tools::append_entry`; clippy → PASS / clean.
- [ ] **Step 5: Commit** — `feat(librarian): an append retried after a partial write records the section it already wrote`.

---

### Task 3: Citations as grammar text, and the commit oracle

**Files:**
- Create: `src/librarian/catalog/cite_text.rs`
- Modify: `src/librarian/catalog/mod.rs`; `src/librarian/derived/diff.rs` (make `scope_of` :550-569 and its `Scope` enum `pub(crate)`); `src/librarian/derived/testkit.rs`; `src/librarian/tools/doctor.rs` (the cross-root remedy hint :1640-1645 and its pin `cross_root_cites_edges_is_published_at_zero_with_a_working_remedy` :9316-9339)
- Test: `cite_text.rs`, `derived/testkit.rs`, `tools/doctor.rs`

**Interfaces:**
- Consumes: the `artifact` table (`id`, `abs_path`, `slug`, `missing_since`); `derived::diff::{scope_of, Scope}`; `identity::{key::key_of, key_space::load (key_space.rs:114), gate::{check_tree, GateProblem}, repo_name::{read, RepoName}, grammar::{parse_target, Reference}}`; `current_project::lookup_git_root`; `util::{root_and_prefix, path_at_or_under_sql}` (`util.rs:311`, `:302`); `derived::{scan::open_commit, build::{rules_from_commit, key_space_from_commit, build_full, BuildInput}, schema::open_in_memory}`, `testkit::repo_with`.
- Produces (each production item carries `#[cfg_attr(not(test), allow(dead_code))]` until Task 7 calls it):
  - `pub(crate) const CITES_LABEL: &str = "**Cites:**";`
  - `pub(crate) fn grammar_form_of(conn: &rusqlite::Connection, source_git_root: &std::path::Path, dst_ref: &str) -> anyhow::Result<String>` — `dst_ref` is what `resolve_cite_ref` (`augmentation.rs:2457`) returns: with a `:`, `<slug>:<local>`; without, an artifact id (any string; the rel_path arm returns whatever id the row has, `:2508-2530`). Target git root by `lookup_git_root` (none → refused); target rel by `scope_of(target_root, abs_path)` (`Inside(rel)` only); population = `scope_of` of every non-missing artifact row under the target root, `Inside` only, so `.worktrees/` shadows and `//` duplicate rows drop out (as `diff.rs:13-20`); `check_tree(name, &key_space::load(target_root)?, &population)`, `name` = `repo_name::read(target_root)?` or the literal `repo`; refused when the target rel is outside the key space, or `check_tree` reports an `InvalidKey` for it or a `DuplicateKey` whose `rel_paths` hold it (the refusal carries `GateProblem::render()`). Qualification: same root as `source_git_root` → unqualified; another root → its committed name; none → refused naming `.codescout/repo.toml`; a name equal to the source root's committed name → unqualified; else `[[name/key…]]`. Last, the produced target text is checked with `parse_target`: anything but `Reference::File`/`Reference::Entry` (for example a params id with a four-letter prefix, `[[key:ABCD-1]]`) is refused naming the token.
  - `pub(crate) fn cites_line(forms: &[String]) -> String` — `**Cites:** [[a]], [[b:F-3]]`, input order, exact repeats dropped.
  - `pub(crate) fn with_cites(section: &PendingSection, forms: &[String]) -> PendingSection` — `body.trim_end()` + `"\n\n"` + `cites_line(forms)`; a plain clone when `forms` is empty.
  - In `testkit.rs`: `pub(crate) struct Built { pub conn: rusqlite::Connection, _dir: tempfile::TempDir }`, `pub(crate) fn build_files(files: &[(&str, &str)]) -> Built`, `pub(crate) fn entry_tokens(conn: &rusqlite::Connection, rel_path: &str) -> Vec<String>`, `pub(crate) fn edges_from(conn: &rusqlite::Connection, rel_path: &str) -> Vec<(String, Option<String>, Option<String>, Option<String>)>` (`kind`, `target_repo`, `target_key`, `target_token`, by `ord`; kinds per `derived/build.rs:597-610`).
  - The doctor cross-root hint adds that the foreign repository needs a committed `.codescout/repo.toml` name.

Old-scanner side effects, recorded and not fixed: `cross_repo_re` (`tools/link_scan/extract.rs:287-290`) matches `key:F-3` inside `[[name/key:F-3]]` and resolves it by local stem, so an entry-grain cross-repo cite can add a wrong `origin='scan'` row; a folder-index target (`[[area:F-3]]`) gives a cross-repo finding, not an edge; a one-letter key (`[[a:F-1]]`) falls back to a bare `F-1`.

- [ ] **Step 1: Write the failing tests** (temp dirs with `.git/`; in-memory catalog with artifact rows):

```rust
#[test] fn a_file_ref_becomes_a_key_and_an_entry_ref_a_key_with_its_token() {
    assert_eq!(grammar_form_of(&cat.conn, &root, "id-target").unwrap(), "[[target]]");
    assert_eq!(grammar_form_of(&cat.conn, &root, "target-slug:F-3").unwrap(), "[[target:F-3]]");
    assert_eq!(grammar_form_of(&cat.conn, &root, "id-area-readme").unwrap(), "[[area]]");
}
// mutation: use the file stem instead of key_of → "[[readme]]".
#[test] fn a_non_canonical_target_name_is_refused_by_name() { /* docs/Bad_Name.md → Err containing "Bad_Name.md" */ }
// mutation: drop the InvalidKey arm.
#[test] fn a_target_outside_the_key_space_is_refused() { /* key-space.toml excludes contrib/**; contrib/x.md → Err */ }
// mutation: drop the is_outside check → Ok("[[x]]").
#[test] fn a_shared_key_is_refused_naming_both_files() {
    let msg = format!("{:#}", grammar_form_of(&cat.conn, &root, "id-a-notes").unwrap_err());
    assert!(msg.contains("docs/a/notes.md") && msg.contains("docs/b/notes.md"));
}
// mutation: drop the DuplicateKey arm → Ok("[[notes]]").
#[test] fn worktree_shadows_and_double_slash_rows_are_not_collisions() {
    // rows <root>/docs/t.md, <root>/.worktrees/feat/docs/t.md, <root>//docs/t.md, one with missing_since → Ok("[[t]]")
}
// mutation: build the population from raw rel paths instead of scope_of → DuplicateKey.
#[test] fn a_foreign_target_is_qualified_by_its_committed_repo_name() { /* other/.codescout/repo.toml name "other-repo" → "[[other-repo/notes]]" */ }
// mutation: ignore source_git_root → "[[notes]]".
#[test] fn a_foreign_target_with_no_committed_name_is_refused() { /* Err containing "repo.toml" */ }
// mutation: fall back to an unqualified key.
#[test] fn another_checkout_of_the_same_repository_stays_unqualified() { /* both roots name "same" → "[[t]]" */ }
// mutation: qualify whenever the roots differ → "[[same/t]]".
#[test] fn a_token_outside_the_grammar_is_refused() { /* "target-slug:ABCD-1" (params row exists) → Err naming "ABCD-1" */ }
// mutation: drop the parse_target check → Ok("[[target:ABCD-1]]").
#[test] fn cites_line_keeps_order_and_drops_repeats() {
    assert_eq!(cites_line(&["[[b]]".into(), "[[a:F-1]]".into(), "[[b]]".into()]), "**Cites:** [[b]], [[a:F-1]]");
}
// mutation: sort the forms.
#[test] fn with_cites_appends_one_labelled_line() { /* body "x\n" + ["[[a]]"] → "x\n\n**Cites:** [[a]]"; [] → body unchanged */ }
// mutation: put the line before the body.
#[test] fn the_commit_oracle_sees_what_a_commit_holds() {      // testkit.rs
    let b = build_files(&[("docs/ledger.md", "## F-1 — x\n\n**Cites:** [[target]]\n"), ("docs/target.md", "# T\n")]);
    assert_eq!(entry_tokens(&b.conn, "docs/ledger.md"), vec!["F-1"]);
    assert_eq!(edges_from(&b.conn, "docs/ledger.md"), vec![("file".into(), None, Some("target".into()), None)]);
}
// mutation: build from an empty tree.
```

Doctor pin (`doctor.rs:9339`): add `assert!(hint.contains("repo.toml"))`. Mutation: revert the hint sentence.

- [ ] **Step 2: Run** `scripts/with-slot.sh cargo test --lib catalog::cite_text derived::testkit tools::doctor::tests::cross_root` → FAIL.
- [ ] **Step 3: Implement.** **Step 4: Run** → PASS; `scripts/with-slot.sh cargo test --lib derived::` (the visibility change must not change a dump); clippy → clean.
- [ ] **Step 5: Commit** — `feat(librarian): render a resolved citation as grammar text, qualified across repositories; a commit oracle`.

---

### Task 4: No allocation from a worktree; no renumber that splits params from markdown

**Files:**
- Modify: `src/librarian/tools/append_entry.rs` (the prose path's shadow guards :236-300 become one function used by both paths; the params path's cites-only worktree refusal :425-440 is subsumed); `src/librarian/catalog/graft.rs` (`graft_rows` :129, `merge_augmentation` :440); `src/librarian/tools/merge_worktree.rs` (`merge_one` :231, after the fold :318-323 and before the `dry_run` return; `call` :101-158); `src/librarian/tools/doctor.rs` (the `active_worktree_registration_root_missing` remedy, doc :3515-3518 and its message ~:3540-3555); `src/prompts/guides/tracker-conventions.md` (§ "Entry ids — refusals: main checkout only (prose ledgers)" :197-201 → "… (every ledger)") and its pin `src/prompts/guide_index.rs:1241`; `docs/conventions/tracker-conventions-derivations.md:641` (the same heading)
- Test: `tools/append_entry.rs`, `catalog/graft.rs`, `tools/merge_worktree.rs`

**Interfaces:**
- Consumes: `tools::worktree::{is_main_checkout_artifact, is_worktree_shadow, has_fork_record}` (`worktree.rs:28`, `:72`, `:44`), `tools::link_scan::extract::entry_sections`, `read_door::ReadDoor`, `catalog::worktree::RegistrationRow { worktree_root, main_root, branch: Option<String>, .. }` (`catalog/worktree.rs:10-17`), `derived::scan::{open_commit, blob_text}` (`scan.rs:47`, `:56`).
- Produces:
  - `fn refuse_allocation_off_the_main_ledger(ctx: &ToolContext, cat: &Catalog, id: &str) -> Result<()>` (private, `tools/append_entry.rs`), called before `resolve_write_target` on both paths; its hint names the same call pinned to the main checkout with `workspace=<main root>`.
  - `pub(crate) fn renumbered_ids_defined_in(remap: &std::collections::BTreeMap<String, String>, source_body: &str) -> Vec<String>` (`graft.rs`, pure): old ids in `remap` that a heading defines in the source body.
  - `merge_augmentation` gains `roots: &[std::path::PathBuf]` (its one caller, `graft_rows`, takes `cat.write_roots()` before the transaction).
  - `merge_one` gains `registration: &reg::RegistrationRow`.

Behaviour:
- Worktree: a worktree session, or any session naming a shadow, gets the prose path's refusal on the params path too: once the params path writes `## F-N` into the worktree's file (Task 5), a concurrent main-checkout append merges two definers of one token (`doctor`'s `entry_defined_twice`, `doctor.rs:4867`). Following the hint writes the entry into the MAIN checkout's working tree, where peers see it as an uncommitted ledger change, as the prose path does today. The guide's refusal section names the `workspace=<main root>` route instead of "write to a worktree-local file", byte-neutral.
- `graft_rows`: when the fold renumbers ids, `renumbered_ids_defined_in(remap, from_body)` runs over the `from` (source) body only, read through `ReadDoor::new(roots)`, and only when `remap` is non-empty (so `doc(move)`, whose destination is never augmented, reads nothing new, and a survivor's own headings never block). A hit, or an unreadable source, refuses with a `LibrarianRecoverableError` naming the ids and the ledgers by `document_label`, saying nothing changed.
- `merge_one`: the source body is the shadow's file when it exists (`std::fs::metadata(..).is_file()`); otherwise the shadow file's blob at the tip of `registration.branch` in the repository at `registration.main_root` (`git2::Repository::open`, `open_commit(repo, branch)`, `blob_text` at the shadow's path relative to `registration.worktree_root`). When neither can be read (no branch recorded, branch deleted), it fails closed: the row is skipped whenever main's file defines the old id by heading. A hit returns `MergeOneOutcome::skipped` with conflict kind `entry_renumber_needs_markdown`, the ids, and a remedy: no tool renumbers one entry in params and markdown (`update_entry` refuses `id`; `rekey_prefix` moves a whole prefix), so `merge_worktree(abandon=true)` is the only exit; it drops the shadow's catalog rows, the entries remain as markdown in the merged file, and that file may hold two `## F-n` definers to rename by hand. `call` leaves the registration `active` (no `reg::set_status(…, "merged")`, :146-148) when any row was skipped with that kind, and says so; the doctor remedy for a registration whose root is gone says a merge skips such rows again.
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
// mutation: call the guard after resolve_write_target → a shadow row exists.
#[tokio::test] async fn params_allocation_is_refused_on_a_shadow_from_a_main_checkout_session() { /* has_fork_record */ }
// mutation: drop the has_fork_record arm from the shared guard.
```

`append_with_cites_from_worktree_is_refused` (`:1769`) asserts the shared refusal's text; the doc comment of `prose_allocation_is_refused_from_a_worktree` (`:1193-1196`), which names the reversed test as its proof, states the current rule.

```rust
// graft.rs                                                                     // Review Focus 7
#[test] fn a_graft_that_would_renumber_a_markdown_defined_entry_is_refused() {
    // from: params [F-1], body `## F-1 — x`; into: params [F-1], body `## F-1 — y`
    assert!(format!("{:#}", graft_rows(&mut cat, "from", "into").unwrap_err()).contains("F-1"));
    assert!(row_exists(&cat.conn, "from").unwrap());
}
// mutation: skip the check → Ok, from deleted.
#[test] fn a_survivor_heading_alone_does_not_block_a_renumber() { /* from: params [F-1], no heading; into: heading → remap {F-1: F-2} */ }
// mutation: check the into body too → refused.
#[test] fn renumbered_ids_defined_in_reads_headings_not_prose() {
    let remap = BTreeMap::from([("F-1".into(), "F-3".into()), ("F-2".into(), "F-4".into())]);
    assert_eq!(renumbered_ids_defined_in(&remap, "## F-1 — x\n\nsee F-2\n"), vec!["F-1"]);
}
// mutation: match tokens anywhere.

// merge_worktree.rs
#[tokio::test] async fn a_gone_worktree_is_read_at_its_branch_tip() {
    // temp repo at main_root; branch `feat` whose docs/trackers/t.md has `## F-2 — wt`; worktree dir absent;
    // main's F-2 is a params-only row and main's file (post-merge) has one `## F-2 — wt` heading
    let out = call(&ctx, json!({"root": wt_root})).await.unwrap();
    assert_eq!(out["conflicts"][0]["kind"], json!("entry_renumber_needs_markdown"));
    assert!(out["conflicts"][0]["remedy"].as_str().unwrap().contains("abandon"));
    assert_eq!(reg::get(&ctx.catalog.lock(), &wt_root).unwrap().unwrap().status, "active");
}
// mutation: read main's file and count definers twice instead of the branch tip → merged, red.
#[tokio::test] async fn an_unreadable_source_fails_closed_when_main_defines_the_id() { /* no branch recorded, main heading `## F-2` → skipped */ }
// mutation: treat an unreadable source as "no heading" → merged.
#[tokio::test] async fn a_source_with_no_heading_still_renumbers() { /* shadow file present, no heading for F-2; main has `## F-2 — main` → merged, remap {F-2: F-3}, registration "merged" */ }
// mutation: check main's body → skipped.
```

`merge_folds_delta_without_duplicating_base_entries` (`merge_worktree.rs:611`) calls the library with `section: None` (no file, params-only) and keeps passing.

- [ ] **Step 2: Run** `scripts/with-slot.sh cargo test --lib tools::append_entry catalog::graft tools::merge_worktree tools::doctor prompts::guide_index` → FAIL. **Step 3: Implement**; the heading rename and its pin change together. **Step 4: Run** → PASS; clippy → clean. Then `./scripts/gate.sh` → green.
- [ ] **Step 5: Commit** — `feat(librarian): entry ids are allocated on the main ledger only, and a fold never renumbers a markdown-defined entry`.

---

### Task 5: The params path writes the entry into the markdown

**Files:**
- Modify: `src/librarian/catalog/augmentation.rs` (`append_entry`: allocation and `write_mark: true`); `src/librarian/tools/append_entry.rs` (`call`: the section refusal on the params path; test migration); `src/librarian/tools/artifact.rs` (`a_write_names_the_row_it_landed_on_in_a_worktree_session_and_the_artifact_otherwise`, :596-640); `src/server.rs` (the `append_entry` call at ~:17801)
- Test: `src/librarian/catalog/augmentation.rs`, `src/librarian/tools/append_entry.rs`

**Interfaces:**
- Consumes: Tasks 1, 2, 4.
- Produces: the tool refuses a params call (`entry_collection` given) that lacks `title`, `body` or `anchor_heading`, before any catalog access, with a `LibrarianRecoverableError` saying an entry on a params ledger is written into the markdown too and naming the missing fields. The library `augmentation::append_entry` keeps `section: Option<&PendingSection>` (only production caller: `tools/append_entry.rs:443`); `None` still means a params-only row (library callers `augmentation.rs` ~:4355-4993 and `merge_worktree.rs` :624, :639 stay green; Plan D's export uses it). With a section: `next = max(params_next, body_max + 1, frontmatter_mark + 1)`, the mark is written (`write_mark: true`), and adoption now also writes the mark when it is below `n`. No `entry_reservation` here.

Migration (concrete; this task is green on its own because the recipe gates read files, not code paths):
- In `tools/append_entry.rs`, add `title`, `body`, `anchor_heading` to the params calls at ~:635, :669, :678, :712, :743, :764, :1088, :1113, :1134, :1153, :1720, :1752, :1783; give the fixtures they use (~:628 `seed_with_body`, :665, :705, :2018) a frontmatter block and the anchor heading; `seed` (:501-546) points at `/test/<id>.md` with no file, so the tests on it move to a temp file. `:1783` (`append_with_cites_from_worktree_is_refused`) keeps refusing on the worktree guard, which runs before any file is touched.
- `artifact.rs:596-640`: drop the `append_entry` case from this naming test (its fixtures `MAIN = "/repo/docs/trackers/t.md"` and the shadow path have no file, so a section write cannot succeed there); `update_entry`, `augment` and the fourth case still cover `name_the_target`.
- `server.rs:~17801`: the call gains the three fields and the tracker its anchor heading.
- `link_dir_tests.rs:~1239` stays green.

- [ ] **Step 1: Write the failing tests:**

```rust
// tools/append_entry.rs
#[tokio::test] async fn a_params_append_without_a_section_is_refused_and_allocates_nothing() {
    let err = call(&ctx, json!({"id": "art1", "entry_collection": "failures", "id_prefix": "F", "entry": {}})).await.unwrap_err();
    assert!(format!("{err:#}").contains("anchor_heading"));
    assert!(params_ids(&ctx, "art1").is_empty());
}
// mutation: remove the tool-layer refusal → Ok.

// augmentation.rs
#[test] fn a_params_append_writes_the_section_and_raises_the_mark() {          // parse oracle
    let out = append_entry(/* … */ Some(&pending("first"))).unwrap();
    let text = std::fs::read_to_string(&md).unwrap();
    assert!(text.contains("entry_high_water_F: 1"));
    let parsed = parse_document("docs/ledger.md", &text, &default_rules().unwrap());
    assert_eq!(parsed.entries.iter().map(|e| e.token.as_str()).collect::<Vec<_>>(), vec![out.id.as_str()]);
}
// mutation: leave `write_mark: false` → no mark.
#[test] fn the_committed_mark_counts_on_the_params_path() { /* frontmatter mark 7, params empty, no heading → F-8 */ }
// mutation: drop `frontmatter_mark + 1` from the max → F-1.
#[test] fn a_params_ledger_with_no_frontmatter_is_refused_with_a_section() { /* Err; file == before; params unchanged */ }
// mutation: pass write_mark false → Ok.
```

- [ ] **Step 2: Run** `scripts/with-slot.sh cargo test --lib catalog::augmentation tools::append_entry tools::artifact` and `scripts/with-slot.sh cargo test --lib server::tests` → FAIL, including the tests to migrate.
- [ ] **Step 3: Implement** and migrate as listed.
- [ ] **Step 4: Run** the same → PASS; clippy → clean.
- [ ] **Step 5: Commit** — `feat(librarian): a params-ledger entry is written into the markdown with its mark`.

---

### Task 6: The recipes, prompts and descriptions say so

**Files:**
- Modify: `src/librarian/tools/append_entry.rs` (the recipe gates `every_taxonomy_append_entry_recipe_is_one_the_code_accepts` :2824, `every_sidecar_prompt_append_entry_call_is_one_the_code_accepts` :3537, `every_ledger_append_entry_recipe_is_one_the_code_accepts` :4101, and their call reader `prompt_calls` / `SidecarCall` :3299-3340); `docs/TAXONOMY.md` (rows T :121, WIN :122, PV :124, RC :125, DC :132); the `docs/augmentations/*.yaml` whose prompts the sidecar gate reports; `docs/manual/src/concepts/entry-citations.md` (:21-27, :106-112); `src/librarian/tools/artifact.rs` (`entry_collection` text :455, `anchor_heading` :488, `id_prefix`, the `doc` description :247); `src/prompts/guides/librarian.md` (:184-185); `src/prompts/guides/tracker-conventions.md` (§ Entry ids :187-195)
- Test: `src/librarian/tools/append_entry.rs`

**Interfaces:**
- Consumes: Task 5.
- Produces: `const KNOWN_SECTIONLESS_LEDGER_RECIPES: &[(&str, &str)] = &[("docs/trackers/claim-decay.md", "DC"), ("docs/trackers/windows-platform-support.md", "WIN")];` in the gate module — a shrinking known-list in the shape of `KNOWN_ABSENT` (`tests/ledger_field_labels.rs`): an unlisted params recipe without a section fails, and a listed one that has gained its section fails until its line is removed.

Changes:
- The three recipe gates also require, for each params call they read (`collection.is_some()`), the arguments `title`, `body` and `anchor_heading`; a missing one is a finding naming the file and fields. The ledger gate exempts exactly the two known-list recipes (`claim-decay.md:142-143`, `windows-platform-support.md:62`): those are librarian-managed trackers and are fixed from the main checkout after the plan.
- `every_work_stream_taxonomy_recipe_is_one_the_code_accepts` (:4802) is unchanged: its `Append` cells for FND and FT (`TAXONOMY.md:207-208`) are `entry_collection="…"` shape labels that name the collection and no call arguments (the scanner builds `Shape::Params { collection }` from them, :4751-4763), so they state nothing the section requirement contradicts.
- `docs/TAXONOMY.md` rows T, WIN, PV, RC, DC: one `append_entry` call carrying `title`, `body`, `anchor_heading` (and `index_row` where the row had one); the "then a section via `doc(update)`" step goes. `docs/TAXONOMY.md` and the manual page are not trackers; edit them with `edit_file`.
- Sidecar prompts: a direct YAML edit of each prompt's recipe sentence, by `edit_file` on `docs/augmentations/<name>.yaml`. A sidecar is no stamped or ledger markdown file, so the librarian guard does not apply (`librarian_guard.rs:126-170`), and `every_committed_sidecar_parses_and_carries_no_params` covers the result. Not through `doc(augment)` pinned to the worktree: that forks shadow rows, a `worktree_fork` event, a lineage link and a registration; `write_through` renders the whole row and refuses when main's row drifts from the committed sidecar on a field the call does not author (`augmentation_sidecar.rs:423-435`); and `merge_one` never folds the prompt. After the merge the main catalog rows lag the committed prompts either way (`drifting_fields` compares the prompt); applying them is a named step under After the plan.
- Schema texts: `entry_collection` says the params path writes the section; `anchor_heading` is not prose-only; `id_prefix` names the frontmatter mark among the inputs; the `doc` description says the params path writes the section and needs the three fields. Each keeps its character count or the budget is raised to the measured total with a log entry (Global Constraints).
- Guides: byte-neutral or smaller.

- [ ] **Step 1: Write the failing tests:**

```rust
#[test] fn a_params_call_without_a_section_is_a_recipe_finding() {
    // the sidecar gate's reader over `doc(action="append_entry", id="x", entry_collection="c", id_prefix="T", entry={…})`
    assert!(findings.iter().any(|f| f.contains("anchor_heading")));
}
// mutation: drop the new condition → no finding.
#[test] fn a_known_sectionless_recipe_that_gains_its_section_must_leave_the_list() {
    // ledger-gate check over a fixture where claim-decay's recipe carries title/body/anchor_heading → a finding naming KNOWN_SECTIONLESS_LEDGER_RECIPES
}
// mutation: never report a stale known-list entry → no finding.
```

- [ ] **Step 2: Run** `scripts/with-slot.sh cargo test --lib tools::append_entry::tests::taxonomy_recipes` → FAIL (the extended gates report TAXONOMY rows and sidecar prompts).
- [ ] **Step 3: Implement** the gate extensions, then fix TAXONOMY, the prompts, the manual, the schema texts and the guides until the gates pass.
- [ ] **Step 4: Run** that filter, `scripts/with-slot.sh cargo test --lib augmentation_sidecar prompts:: server::tests::tool_surface_under_budget every_tool_description_is_under_its_cap` → PASS; clippy → clean. Then `./scripts/gate.sh` → green.
- [ ] **Step 5: Commit** — `docs+test(librarian): params recipes, prompts and descriptions carry the section`.

---

### Task 7: Params-path citations as grammar text

**Files:**
- Modify: `src/librarian/catalog/augmentation.rs` (`append_entry`, `AppendOutcome`), `src/librarian/catalog/cite_text.rs` (remove the `allow(dead_code)` attributes added in Task 3), `src/librarian/tools/append_entry.rs` (response), `src/librarian/tools/artifact.rs` (`cites` text :484), `src/prompts/guides/tracker-conventions.md` (one sentence; byte-neutral)
- Test: `src/librarian/catalog/augmentation.rs`, `src/librarian/tools/append_entry.rs`

**Interfaces:**
- Consumes: Tasks 1-5.
- Produces: `AppendOutcome` gains `pub cites: Vec<String>` and `pub key_note: Option<String>`; the response carries `"cites"` when non-empty and `"key_note"` when set.

Inside the transaction: each ref goes through `resolve_cite_ref` and `grammar_form_of(&tx, &ledger_git_root, &dst_ref)` (ledger git root from `lookup_git_root(abs_path)`; none → refused); any refusal aborts with nothing written; the section is `with_cites(section, &forms)`, and adoption compares against that rendered body; the `entry_cite` `origin='write'` rows stay (:981-998). `key_note` is set when `key_of` fails for the ledger itself. The `cites` text says each ref is written into the entry as grammar text and recorded as an edge, that a key two files share is refused, and that refs into another repository need its committed name; same character count or a logged budget raise.

The old scanner beside the write row: `cross_repo_re` matches `target:F-3` inside `[[target:F-3]]` and derives `dst_ref = <dst_slug>:F-3` (`tools/link_scan/mod.rs:183-201`), the shape `resolve_cite_ref` stores; `entry_cite`'s primary key leaves `origin` out (`entry_cite.rs:16-28`), so the scan's insert is ignored and the write row stays. A file cite has no scanner twin.

- [ ] **Step 1: Write the failing tests** (fixtures under a temp root with `.git/`):

```rust
#[test] fn params_cites_are_grammar_text_and_write_rows() {                    // parse oracle
    let out = append_entry(/* … */ &["id-other".into(), "target-slug:F-3".into()], Some(&pending("first"))).unwrap();
    let parsed = parse_document("docs/ledger.md", &std::fs::read_to_string(&md).unwrap(), &rules);
    assert!(parsed.citations.iter().any(|c| c.reference == Reference::File { repo: None, key: Key::parse("other").unwrap() }));
    assert!(parsed.citations.iter().any(|c| c.reference == Reference::Entry { repo: None, key: Key::parse("target").unwrap(), token: "F-3".into() }));
    assert_eq!(write_rows(&cat, &out.id), 2);
    assert_eq!(out.cites, vec!["[[other]]", "[[target:F-3]]"]);
}
// mutation: skip with_cites.
#[test] fn a_cite_that_names_a_shared_key_aborts_the_whole_append() {          // Review Focus 3
    assert!(append_entry(/* cite docs/a/notes.md while docs/b/notes.md exists */).is_err());
    assert_eq!(std::fs::read_to_string(&md).unwrap(), before);
    assert!(params_ids(&cat, "art1").is_empty());
}
// mutation: resolve the grammar form after the file write → file changed.
#[test] fn a_cite_outside_the_key_space_aborts_the_whole_append() { /* target under an excluded subtree; file == before */ }
// mutation: drop a refused cite instead of aborting → Ok.
#[test] fn a_dual_written_entry_resolves_in_the_derived_index_after_a_commit() { // commit oracle
    let b = build_files(&[("docs/ledger.md", &ledger_text), ("docs/target.md", "## F-3 — t\n"), ("docs/other.md", "# O\n")]);
    let edges = edges_from(&b.conn, "docs/ledger.md");
    assert!(edges.contains(&("entry".into(), None, Some("target".into()), Some("F-3".into()))));
    assert!(edges.contains(&("file".into(), None, Some("other".into()), None)));
}
// mutation: write the cites line inside an HTML comment → no edges.
#[test] fn a_ledger_whose_own_name_is_not_a_key_still_takes_the_entry_and_says_so() { /* docs/Ledger_One.md → Ok; key_note mentions "Ledger_One.md" */ }
// mutation: never set key_note.
```

In `tools/append_entry.rs` (context set-up as `tools/link_scan/mod.rs:1477-1505`):

```rust
#[tokio::test] async fn a_write_scan_after_a_cited_append_keeps_one_write_row() {
    let out = link_scan::call(&ctx, json!({ "write": true })).await.unwrap();
    assert_eq!(out["counts"]["entry_edges"]["skipped_existing"], json!(1));
    assert_eq!(rows_for(&cat, &new_id), vec![("target-slug:F-3".to_string(), "write".to_string())]);
}
// mutation: insert the write row with dst_ref `target:F-3` (key, not slug) → written 1.
```

If that test shows `written: 1` before any mutation, stop and report; the assertion is not loosened. The existing cites tests (`append_with_cites_writes_entry_cite_and_not_artifact_link` :1713, `append_with_unresolvable_cite_writes_nothing` :1752) move their fixtures under a temp root with `.git/`.

- [ ] **Step 2: Run** `scripts/with-slot.sh cargo test --lib catalog::augmentation tools::append_entry tools::link_scan` → FAIL. **Step 3: Implement**, and remove the Task 3 `allow(dead_code)` attributes. **Step 4: Run** → PASS, plus `server::tests::tool_surface_under_budget` and `every_tool_description_is_under_its_cap`; clippy → clean.
- [ ] **Step 5: Commit** — `feat(librarian): params-ledger citations are written into the entry as grammar text`.

---

### Task 8: The prose path writes `cites` too

**Files:**
- Modify: `src/librarian/catalog/augmentation.rs` (`allocate_entry_id`), `src/librarian/tools/append_entry.rs` (the prose `cites` refusal :228-235; the call at :335; response)
- Test: both files

**Interfaces:**
- Consumes: Tasks 1-3, 7; `artifact::ensure_slug`, `entry_cite::insert_with`.
- Produces: `pub fn allocate_entry_id_with_cites(cat: &mut Catalog, artifact_id: &str, id_prefix: &str, section: Option<&PendingSection>, cites: &[String]) -> Result<AllocateOutcome>`; `allocate_entry_id(cat, artifact_id, id_prefix, section)` calls it with `&[]`, so its 31 callers (28 in `augmentation.rs` tests, `tools/append_entry.rs:335` and `:1276`, `tools/link_dir_tests.rs:1498`) do not change. `AllocateOutcome` gains `pub cites: Vec<String>`.

Why the refusal goes: it says "cite in prose — link_scan derives the edges from the text" (`tools/append_entry.rs:229-233`); the server now writes that text. `cites` with no section stays refused before any allocation. The prose path inserts the same `origin='write'` rows as the params path.

- [ ] **Step 1: Write the failing tests:**

```rust
#[test] fn prose_cites_are_grammar_text_and_write_rows() {                      // parse oracle
    let out = allocate_entry_id_with_cites(&mut cat, "art1", "F", Some(&pending("first")), &["id-other".into()]).unwrap();
    let parsed = parse_document("docs/ledger.md", &std::fs::read_to_string(&md).unwrap(), &rules);
    assert!(parsed.citations.iter().any(|c| matches!(&c.reference, Reference::File { key, .. } if key.as_str() == "other")));
    assert_eq!(write_rows(&cat, &out.id), 1);
}
// mutation: skip with_cites on the prose path.
#[test] fn prose_cites_without_a_section_are_refused_and_reserve_nothing() { /* Err; reserved_max None; file == before */ }
// mutation: drop the no-section refusal → Ok with an id and no text.
#[test] fn a_prose_cite_of_a_non_canonical_name_aborts_and_reserves_nothing() { /* Err; reserved_max None */ }
// mutation: insert the reservation before resolving cites and commit it on error → Some(1).
```

Rename `a_prose_ledger_refuses_cites` (`tools/append_entry.rs:1046`) to `a_prose_ledger_takes_cites_with_a_section_and_refuses_them_without` and assert both halves. Mutation: restore the unconditional refusal.

- [ ] **Step 2: Run** → FAIL. **Step 3: Implement.** **Step 4: Run** `scripts/with-slot.sh cargo test --lib catalog::augmentation tools::append_entry` → PASS; clippy → clean. Then `./scripts/gate.sh` → green.
- [ ] **Step 5: Commit** — `feat(librarian): a prose-ledger entry takes cites and writes them as grammar text`.

---

### Task 9: `rekey_prefix` and grammar citations of the prefix

**Files:**
- Modify: `src/librarian/catalog/rekey.rs` (`retarget_prose` :200-226; `rekey_prefix_rows` :566-833, before any write; its inbound loop :655-700)
- Test: `src/librarian/catalog/rekey.rs`

**Interfaces:**
- Consumes: `entry_cite` (`dst_ref LIKE '<slug>:<from>-%' AND origin = 'write'`), `identity::grammar::{extract, Reference}`, `identity::key::key_of`, `read_door::ReadDoor`, Task 1 `document_label`.
- Produces:
  - `retarget_prose` gains `own_key: Option<&str>`: a qualifier equal to the ledger's own slug OR its own key moves with the prefix, so a ledger's self-citation `[[key:F-3]]` (Task 7) does not go stale when slug and key differ. Its callers in `rekey.rs` pass `key_of` of the ledger's repo-relative path.
  - `fn grammar_citers_of(tx: &rusqlite::Transaction<'_>, roots: &[std::path::PathBuf], ledger_slug: &str, ledger_key: &str, from_prefix: &str) -> anyhow::Result<Vec<String>>` (private): documents other than the ledger whose inbound write rows name the old prefix AND whose text, read through `ReadDoor::new(roots)`, holds a `Reference::Entry` with `key == ledger_key` and a token `<from>-<n>` (qualified or not). A citer the door refuses or cannot read is included (fail closed). Non-empty → `rekey_prefix_rows` refuses, in dry run and apply alike, naming each citer by `document_label` and saying its `**Cites:**` text must be rewritten first.

- [ ] **Step 1: Write the failing tests:**

```rust
#[test] fn a_rekey_is_refused_while_another_document_cites_the_prefix_in_grammar_text() {
    // docs/other.md section holds `**Cites:** [[ledger:F-1]]` and a write row (other-slug, R-1, ledger-slug:F-1)
    let err = rekey_prefix_rows(&mut cat, "ledger", "F", "G", RekeyMode::Apply).unwrap_err();
    assert!(format!("{err:#}").contains("[[other]]"));
    assert!(std::fs::read_to_string(&ledger).unwrap().contains("## F-1"));
}
// mutation: return an empty Vec from grammar_citers_of → rekey applies.
#[test] fn a_write_row_with_no_grammar_text_does_not_block() { /* legacy row, citer text without [[…]] → Ok */ }
// mutation: refuse on rows alone → Err.
#[test] fn an_unreadable_citer_refuses_the_rekey() { /* citer row whose file lies outside the roots → Err naming it */ }
// mutation: skip unreadable citers → Ok.
#[test] fn a_ledgers_own_grammar_citation_moves_with_the_prefix() {
    // ledger slug "ledger-slug", file docs/ledger.md holds `**Cites:** [[ledger:F-1]]`; rekey F → G
    assert!(std::fs::read_to_string(&ledger).unwrap().contains("[[ledger:G-1]]"));
}
// mutation: ignore own_key in retarget_prose → "[[ledger:F-1]]" stays.
```

- [ ] **Step 2: Run** `scripts/with-slot.sh cargo test --lib catalog::rekey` → FAIL. **Step 3: Implement.** **Step 4: Run** → PASS; clippy → clean.
- [ ] **Step 5: Commit** — `feat(librarian): rekey_prefix moves a ledger's own grammar citations and refuses while another document cites the prefix`.

---

### Task 10: The CLI write commands take the project write lock

**Files:**
- Create: `src/cli/write_lock.rs`, `tests/cli_write_lock.rs`
- Modify: `src/cli/mod.rs` (module); `src/cli/doc.rs` (`run_create` :497, `run_update` :634, `run_move` :663, `run_link` :697); `src/cli/doc_event.rs` (`run_create` :113); `src/cli/doc_augment.rs` (`run` :41, before `run_with_ctx`); `src/cli/doctor.rs` (`run` :166, when `--fix` is given); `src/cli/audit_doc_refs.rs` (`run` :76, unless `--no-emit-tracker`)

**Interfaces:**
- Consumes: `crate::agent::{acquire_write_guard, open_lock_file, holder_record_path, WriteGuard}` (`agent/mod.rs:10-12`); `acquire_write_guard(async_mutex: Arc<tokio::sync::Mutex<()>>, file: Arc<std::fs::File>, holder_path: PathBuf, timeout: Duration, holder: &str) -> Result<WriteGuard, crate::tools::RecoverableError>` (`agent/write_guard.rs:229-235`); `ProjectConfig::load_or_default` (`config/project.rs:862`) for `security.write_lock_timeout_secs` (default 5); `ToolContext.current_project` (`CurrentProject { abs_path, git_root, .. }`, `current_project.rs:23`).
- Produces:
  - `pub(crate) fn lock_root(cp: &CurrentProject) -> std::path::PathBuf` — the nearest directory from `cp.abs_path` up to and including `cp.git_root` holding `.codescout/project.toml`; else `cp.git_root`.
  - `pub(crate) async fn take(ctx: &crate::librarian::tools::ToolContext, command: &str) -> anyhow::Result<WriteGuard>` — `lock_root`, the timeout, then `take_at`; no current project is an error that names `--project`.
  - `pub(crate) async fn take_at(root: &std::path::Path, timeout: std::time::Duration, command: &str) -> anyhow::Result<WriteGuard>` — a fresh `Arc<tokio::sync::Mutex<()>>`, `open_lock_file(root)`, `holder_record_path(root)`, holder `codescout-cli <command>`; the `RecoverableError` becomes the `anyhow::Error` the CLI prints.
  - `fn takes_the_write_lock(args: &DoctorArgs) -> bool` (`fix.is_some()`, mirroring the MCP doctor arm in `adapter.rs`) and `fn takes_the_write_lock(args: &AuditArgs) -> bool` (`!no_emit_tracker`, mirroring `adapter.rs:491-493`).

How a CLI process finds the root: `open_ctx` (`cli/mod.rs:62`) hands `--project` to `build_tool_context` (`librarian/mod.rs:309`), whose `current_project::resolve` canonicalises the path and walks up to the nearest `.git`. MCP locks `<ActiveProject.root>/.codescout/write.lock`, the canonical launch, activate or pin directory with no walk-up (`agent/mod.rs:1024-1053`, `server.rs:1109-1118`). `lock_root` reaches the same file when that directory is the nearest one holding a `project.toml`; it differs when MCP is activated at a directory without one, or for an umbrella member. The module doc says so, and that Plan E's per-ledger file lock is the fix. The lock is taken after `open_ctx` and held until the tool body returns; MCP takes the flock before SQLite, and the CLI keeps that order. Commands that do not write take none. `backfill-chunks` (catalog rows only) and the non-librarian `index`, `migrate-memories`, `rebuild-memory-vectors` commands (`src/main.rs:55`, `:136`, `:155`) are outside this task.

- [ ] **Step 1: Write the failing tests.** Unit, in `write_lock.rs`:

```rust
#[test] fn the_lock_root_is_the_nearest_project_toml_within_the_git_root() {
    // repo/.git, repo/.codescout/project.toml, cwd repo/sub/dir → repo; repo2/.git only, cwd repo2/x → repo2
}
// mutation: return cp.abs_path.
#[tokio::test] async fn a_held_lock_refuses_the_cli_and_names_the_holder() {
    let held = acquire_write_guard(Arc::new(tokio::sync::Mutex::new(())), open_lock_file(&root).unwrap(),
        holder_record_path(&root), Duration::from_secs(1), "codescout:sid-beta append_entry").await.unwrap();
    let err = take_at(&root, Duration::from_millis(200), "doc update").await.unwrap_err();
    assert!(format!("{err:#}").contains("codescout:sid-beta append_entry"));
    drop(held);
    assert!(take_at(&root, Duration::from_millis(200), "doc update").await.is_ok());
}
// mutation: open the lock file under root.join("sub") → Ok while held.
#[test] fn doctor_takes_the_lock_only_with_a_fix() { /* None → false; Some("export_augmentations") → true */ }
// mutation: return true always.
#[test] fn audit_doc_refs_takes_the_lock_unless_it_emits_no_tracker() { /* default → true; --no-emit-tracker → false */ }
// mutation: return false always.
```

Integration (`CARGO_BIN_EXE_codescout` makes Cargo build the binary). Both fixtures: a `.git/` directory (`current_project::resolve` walks up to `.git`), `.codescout/project.toml` = `[project]\nname = "t"\n[security]\nwrite_lock_timeout_secs = <n>\n` (`ProjectSection` needs only `name`, `config/project.rs:45-85`), and the environment redirection of `tests/cross_process_write_lock.rs:150-160` (`XDG_STATE_HOME`, `LIBRARIAN_WORKSPACE`, `LIBRARIAN_DB` into a temp dir; `CLAUDE_CODE_SESSION_ID` removed):

```rust
#[test] fn cli_doc_create_waits_for_the_project_lock_and_names_its_holder() {     // Review Focus 2; timeout 1
    // test process: fs4 try_lock_exclusive on .codescout/write.lock; holder record "<now>\tcodescout:sid-beta append_entry"
    let out = codescout(&["doc", "create", "--project", dir, "--kind", "note", "--title", "T", "--rel-path", "docs/t.md"]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("codescout:sid-beta append_entry"));
    assert!(!dir.join("docs/t.md").exists());
}
// mutation: remove the `take` call from run_create → exit 0 and docs/t.md exists.
#[test] fn a_cli_update_waits_for_the_lock_and_keeps_the_other_writers_bytes() {  // Review Focus 2; timeout 5
    // (0) create docs/t.md through the CLI with no lock held
    // (1) the test process takes the flock; (2) spawns `codescout doc update <id> --status done`
    // (3) polls `child.try_wait()` for ~1.5 s: the child is still running and docs/t.md holds no "status: done"
    // (4) appends "peer line" to docs/t.md, releases the flock; (5) waits for the child: exit 0
    let text = std::fs::read_to_string(dir.join("docs/t.md")).unwrap();
    assert!(text.contains("peer line") && text.contains("status: done"));
}
// mutation: remove the `take` call from run_update → step (3) sees the child exited or "status: done" present, deterministically red.
```

- [ ] **Step 2: Run** `scripts/with-slot.sh cargo test --lib cli::write_lock` and `scripts/with-slot.sh cargo test --test cli_write_lock` → FAIL.
- [ ] **Step 3: Implement**, and wire `take` into the eight commands above.
- [ ] **Step 4: Run** both → PASS; `scripts/with-slot.sh cargo test --test cross_process_write_lock` → PASS; clippy → clean. Then `./scripts/gate.sh` → green.
- [ ] **Step 5: Commit** — `feat(cli): write commands take the project write lock`.

---

## Rulings made in this plan

| # | Ruling | Why | Cost if wrong |
|---|---|---|---|
| C1-R1 | File-first means: stage rows in the open `IMMEDIATE` transaction, write the file and `record_written_file`, then commit; a failed commit restores the bytes only when untouched, else reports them by name. | The commit is the durable step; the transaction is the allocation lock. | A crash between write and commit leaves a file-only entry until a retry. |
| C1-R2 | Order changes only on the prose `append_entry` path; `update`, `create`, `event_create`, `augment` stay. | Each already reports its partial state or protects its retry. | A Plan D caller that needs another order re-opens this. |
| C1-R3 | The retry key is (prefix, heading title, section text minus the `**Valid:**` stamp) over sections the catalog has not recorded. "Recorded" is machine-local, so on a fresh clone or another machine an identical title and body adopts a committed entry. | No new marker in committed text. | Two identical entries become one. |
| C1-R4 | Params path: section required, mark folded into allocation and written; no `entry_reservation` here. | Spec requirement 1 and § Entry ids. | A params ledger with no frontmatter block is refused until it gains one. |
| C1-R5 | Citations are one `**Cites:**` line at the end of the section, input order, repeats dropped. | Spec: relations are labelled grammar lines. | A later label vocabulary (C2) renames the label. |
| C1-R6 | A cite is refused when its target has no git root, a non-canonical key, lies outside the key space, shares its key (`check_tree` over the old catalog's `Inside` rows of the target's repo), or yields a non-grammar token. | Spec: one key per document; F-14. | Not-yet-indexed files are not seen; the catalog can also hold files the commit lacks (ignored build output, nested repositories), so a false `DuplicateKey` refusal is possible. |
| C1-R7 | The `entry_cite` `origin='write'` row stays on both paths. | The old scanner does not read `[[key]]`. | Plan D drops rows whose text twin exists. |
| C1-R8 | The prose path accepts `cites` when it writes a section, refuses them otherwise. | The refusal's reason is what the server now does. | none found. |
| C1-R9 | Params-path allocation is refused from a worktree, the hint naming `workspace=<main root>`. | Two definers of one token after a git merge. | Worktree sessions append from the main checkout only, writing into its working tree. Open for the operator. |
| C1-R10 | `graft` refuses and `merge_worktree` skips a renumber of an id the SOURCE defines by heading; the registration stays active; `abandon` is the only exit; params-only renumbers continue. | No tool renumbers one entry in both stores. | A legacy shadow with such a collision is abandoned and its entries survive only as markdown, possibly with two definers. |
| C1-R11 | CLI lock root: nearest `project.toml` up to the git root, else the git root; eight librarian commands; holder `codescout-cli <command>`. | Same lock file as MCP in ordinary layouts. | Other layouts take different lock files until Plan E's per-ledger lock. |
| C1-R12 | New outputs name documents as grammar forms or file names. | Spec G6. | none. |
| C1-R13 | A ledger whose own name is not a key still takes entries; the response carries `key_note`. | The bare token resolves locally. | none. |
| C1-R14 | Sidecar creation is removed from C1 (controller). | Plan D's `export_augmentations` pass produces the same end state in one window. | Augmented documents created before the window stay catalog-only until then. |
| C1-R15 | Cross-repo cites are `[[repo/key…]]` from the target's committed name, refused when none; the doctor hint says so. | `resolve_cite_ref` resolves across the machine-global catalog. | Cites into unnamed repositories are refused until onboarding names them. |
| C1-R16 | The section requirement lives in the tool; the library keeps `Option<&PendingSection>`. | The tool is the only production caller. | A new production caller could skip the markdown; review duty. |
| C1-R17 | Superseded by C1-R23. | — | — |
| C1-R18 | `rekey_prefix` refuses while another document's text cites the old prefix in the grammar and a write row backs it (controller). | Leaves no text naming a moved token. | Hand-written `[[ledger:F-n]]` with no write row is not detected; the derived index reports `undefined_token` after the rekey. |
| C1-R19 | Superseded by C1-R20. | — | — |
| C1-R20 | Ten tasks; the worktree and renumber task runs before the params path writes markdown, and the params work is split into library and tests (5) and recipes, prompts and descriptions (6). | Each task ends green on its own (the worktree tests that the params refusal would break are replaced first). | none. |
| C1-R21 | `find_unrecorded_section` is produced by Task 2; Task 3's items carry `#[cfg_attr(not(test), allow(dead_code))]` until Task 7 removes it. | Clippy `-D warnings` runs after every task. | Forgetting the removal leaves a dead allow; Task 7's step names it. |
| C1-R22 | The prose path's still-on-disk remedy stays "delete the section, then retry"; only the params path says "retry with the same title and body". | The prose `is_recorded` is a per-prefix maximum a peer's reservation can cover. | A prose caller deletes by hand in that rare case. |
| C1-R23 | Sidecar prompts change by direct YAML edit in the branch (controller). | A worktree-pinned `doc(augment)` forks shadows, reformats the file, can refuse on drift, and its prompt never folds back. | Main's catalog rows lag the committed prompts until the controller applies them after merge. |
| C1-R24 | `claim-decay.md` (DC) and `windows-platform-support.md` (WIN) are pinned in `KNOWN_SECTIONLESS_LEDGER_RECIPES` and fixed from the main checkout after the plan (controller). | Managed trackers are not edited from the worktree. | Agents reading those two ledgers meet the tool's refusal (whose hint names the fix) until then. |
| C1-R25 | The FND/FT `Append` cells of the work-stream table are shape labels, not recipes; that gate is unchanged. | They name a collection and no call arguments (`append_entry.rs:4751-4763`). | none. |
| C1-R26 | `merge_worktree` reads a gone worktree's source at its branch tip through `git2`, and fails closed (skip when main defines the id by heading) when it cannot. | Main's merged file cannot tell the worktree's heading from main's when main's own entry was params-only. | A recorded branch that was rebased or deleted makes the merge skip more rows than needed. |
| C1-R27 | Schema and description edits keep their character counts, or raise `TOOL_SURFACE_CHAR_BUDGET` to the measured total with a log entry. | The budget was set to an exact total. | none. |
| C1-R28 | `rekey_prefix` moves a ledger's own `[[key:F-n]]` (qualifier = its key) and fails closed on a citer it cannot read. | Self-citations from Task 7 would otherwise go stale. | A rekey blocked by an unreadable citer needs that file fixed or moved first. |
| C1-R29 | `docs/manual/src/concepts/entry-citations.md` is fixed in Task 6; `docs/trackers/provenance-subsystem.md:~1130` and `windows-platform-support.md:326` (both use the ungated `artifact(` opener) are fixed from main after the plan. | The manual is not a managed tracker. | none. |
| C1-R30 | Both revisions of this plan were verified by reading code only; no cargo command was run. | The plan is reviewed before anyone executes it. | A seam that reads right but does not compile surfaces in Task Step 2. |

## Open for the operator

1. **Refusing params `append_entry` from a worktree (C1-R9).** Recommendation: refuse, under the conditions built into Task 4: the refusal and the guide's refusal section name the `workspace=<main root>` route; the renumber check reads the SOURCE only (its branch tip when the worktree is gone, failing closed otherwise); a skipped row leaves the registration active and names `abandon`.

## Not built here, on purpose

- **Sidecar creation on `doc(augment)` and `doc(create)`:** Plan D's `export_augmentations` pass. If built later, `create_for` should upsert the one `expects_augmentation:` line rather than re-emit the frontmatter through `frontmatter::write` (`stamp_augmentation_declaration`, `doctor.rs:2147-2168`, which also skips `record_written_file`), and update `src/prompts/guides/librarian-runtime.md:100-110` and the module docs of `augmentation_sidecar.rs`.
- Events, observations, links, `event_edges`, `sources`, `commit_refresh`, ULIDs in committed text: Plan C2.
- The content-keyed vector cache and the persistent derived-file location: Plan C3.
- The git-history cache and `doc(find)` ordering by a catalog clock (`catalog/find.rs:28`): later; the review recommends Plan E.
- Indexing sidecar-declared shapes into the derived catalog: the plan that builds a persistent derived file.
- `codescout-37`'s entry-file format and the L-B tools.
- `update_entry` regenerating a section from changed fields; params field values that live only in the catalog: Plan D and `codescout-37`.
- `cites` given as `key:TOKEN` or `[[…]]`, and `<slug>:<local>` cites of prose-ledger entries.
- A `rekey_prefix` that rewrites foreign `**Cites:**` text instead of refusing; a `doctor` check for hand-written grammar citations of a rekeyed prefix: Plan E.
- A CLI lock for `backfill-chunks`, `index`, `migrate-memories`, `rebuild-memory-vectors`.
- The machine-state file, `entry_reservation` removal, the per-ledger entry-id file lock, and the deletion tail: Plan E.

## After the plan

- **Controller, at release, from the main checkout:** apply the committed sidecar prompts to the main catalog rows with `doc(action="augment", merge=true, augment={prompt: …})` for each sidecar Task 6 changed, until `doctor` reports no `sidecar_shape_drift`.
- **Controller, from the main checkout:** fix the params recipes of `docs/trackers/claim-decay.md:142-143` and `docs/trackers/windows-platform-support.md:62` (and `:326`), and `docs/trackers/provenance-subsystem.md:~1130`, through `doc(action="update", patch={body_edits:[…]})`; then remove the two lines of `KNOWN_SECTIONLESS_LEDGER_RECIPES` in one commit.
- Record the rulings and any `derive-shadow` numbers taken after a dual-write in the session log (id `abff63149e3c24e8`).
- Plan D reuses `dual_write::{write_entry_into_ledger, commit_after_file_write, find_unrecorded_section, record_written_file}` and `cite_text::grammar_form_of` to export the 38 params-only entries and rewrite citations.
