# Artifact Identity Plan C1 — Dual-Write Contract, Entries and Sidecars Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make every entry write and every augmentation-shape write land in the committed home the spec assigns, while still writing the old catalog, so the cutover (Plan D) and the read flip (Plan E) find the files already in place.

**Architecture:** Plan C1 of the spec's Plan C (C1 entries and sidecars; C2 events, observations and links; C3 the content-keyed vector cache). One library module holds the dual-write contract with no `ToolContext`, so Plan D's cutover tool can call it: stage the catalog rows in the open `IMMEDIATE` transaction, write the file through `write_catalogued_file` and `record_written_file`, then commit; a failed commit is compensated or reported by name, and a retry finds the section it already wrote instead of writing a second one. `append_entry` writes every entry as a `## PREFIX-N — title` section on both of its paths, bumps the committed high-water mark, and writes its `cites` as `[[key]]` / `[[key:TOKEN]]` grammar text beside the `entry_cite` row. The CLI write commands take the project write lock. The derived index (Plan B2) is not changed; it is the test oracle.

**Tech Stack:** Rust 2021, `rusqlite` 0.39, `git2` 0.20 (tests only, through `derived::testkit`), `fs4` (the existing write guard); Plan A's `identity::{key, key_space, grammar}`; Plan B2's `derived::{build, schema, scan, testkit}`.

**Spec:** `docs/superpowers/specs/2026-11-11-repo-relative-artifact-key-design.md` — § The store boundary, § Writes, § Contract with ledger formats, § Index inputs, guard, machine state and entry ids, § Worktrees and machines. Evidence: the Plan C scout report and its independent review (session scratchpad `plan-c-seams.md`, `plan-c-scout-review.md`; the review wins where they disagree), the B2 rulings doc (id `4c82d96fc9952e98`), and session-log entries F-11, F-14, F-15 (id `abff63149e3c24e8`). Every `file:line` below was read in this worktree at `8873c012e`; line numbers are hints, find code by name (B2 ruling R7).

## Global Constraints

- **Contract with ledger formats, requirement 1 (spec):** "Every entry exists as markdown: a section that defines its token, or a document of its own. No entry exists only in params." Requirement 3: "An entry's citations are written in the grammar."
- **Entry ids (spec):** "`append_entry` … allocates `max(body, frontmatter high-water) + 1`." Plan C1 keeps `entry_reservation` (its removal is Plan E).
- **Grammar (spec):** `[[key]]` names a document, `[[key:TOKEN]]` an entry of it; a key is the lowercase stem matching `[a-z0-9]+(-[a-z0-9]+)*`, a folder index takes its folder's name, a 16-hex key is refused. Use `identity::key::key_of` (`identity/key.rs:127`) and `identity::grammar::extract`; never a second parser.
- **Write door:** every file write goes through `crate::librarian::util::write_catalogued_file` (`util.rs:85`), and a write into a catalogued file is followed by `record_written_file` in the same transaction, or `doctor` reports `row_behind_file`.
- **Read door:** production code under `src/librarian` reads file content only through `read_door` (`ReadDoor::text`, `text_for_write`, `for_catalog`, `for_project`). Gate: `no_librarian_production_read_bypasses_the_read_door` (`src/librarian/read_door.rs:1414`). `std::fs::metadata` is not a read under that gate.
- **No process-environment mutation:** no `std::env::set_var`/`remove_var` in code or tests (`tests/env_mutation_isolation.rs`, `no_site_outside_the_allowlist_mutates_the_process_environment`). Config reaches a function as an argument.
- **Caps:** a new `const` whose name holds `MAX`, `CAP`, `LIMIT` or `BUDGET` carries a `// cap-class: RESULT_CAP | NOT_A_CAP` line (`tests/result_caps.rs`).
- **References in output:** new response fields, refusal and hint texts name a document by its grammar form (`[[key]]`, `[[key:TOKEN]]`), or by its file name when the name is not a canonical key; never by a 16-hex id, an absolute path or a row id (spec G6 and § Anti-goals). A key-collision refusal may list the colliding repo-relative paths, as the spec's own collision refusal does.
- **Agent-facing text** (tool descriptions, guides, refusals, hints) states the current situation only: no "was", "formerly", "no longer", dates or measurements (`src/prompts/README.md` rule 9). The `doc` description stays under its cap (`every_tool_description_is_under_its_cap`, `src/server.rs:4054`). Guides change; `ONBOARDING_VERSION` does not (README § Versioning).
- **Untouched:** the derived schema, `DERIVED_TABLES`, `PARSER_VERSION` and the pinned corpus digest; the committed sidecar YAML shape (`every_committed_sidecar_parses_and_carries_no_params`, `augmentation_sidecar.rs:862`); sidecars never carry `params`.
- **Evidence rule (B2 R6):** every test that passes on its first run is shown red by the named mutation, then the mutation is reverted.
- **Before completing a task:** targeted tests with `scripts/with-slot.sh cargo test --lib <filter>` (or `--test <name>`), `cargo clippy --workspace --all-targets --features local-embed -- -D warnings` and the same with `--no-default-features`, `scripts/with-slot.sh cargo test --test result_caps`. `./scripts/gate.sh` runs after Tasks 4 and 7 and at the end.
- **Commit:** `git add <paths>`, then `git diff --cached --name-only` as its own call, then `scripts/commit-mine.sh -m "<message>"` (`docs/conventions/shared-checkout-commit-sequence.md`).

### Test oracles (every task names which one its acceptance test uses)

- **Parse oracle (no commit):** `derived::build::parse_document(rel_path, text, &classify::default_rules()?)` (`derived/build.rs:371`, pub, pure) on the bytes read back from disk. It answers "which entries does this file define, which citations does it write". It does not resolve them.
- **Commit oracle (resolved edges):** `derived::testkit::build_files(files)` (Task 3) commits the files into a temp repo through `git2` and runs `build_full`. The derived index reads commits only (`derived/scan.rs:1-4`), so a dual-written file is invisible to it until committed.
- **`codescout derive-shadow` on an uncommitted dual-write** compares the derived index of `HEAD` with the old catalog, which `record_written_file` has moved to the working-tree bytes. Expect a `changed` row on the field `file_sha256` with cause `Unexplained` for every dual-written file (`derived/diff.rs:35-39`, ruling R18), and edge differences for its new citations, until the file is committed and the command is re-run at the new `HEAD`. That is the dual-write window, not drift.

## Review Focus

1. **A crash or a failed catalog commit after the section reached disk, then a retry** — expected: the retry with the same title and body records the section that is already there under its token and writes nothing new; no second section, no skipped id. Tests in Task 2 (adoption) and Task 1 (compensation).
2. **Two writers on one ledger at once: an MCP `append_entry` and a CLI `codescout doc update` of the same file** — expected: the CLI waits on `<project root>/.codescout/write.lock` and names the holder when its timeout passes; neither write clobbers the other. Test in Task 7.
3. **A `cites` ref whose target has a non-canonical file name, lies outside the key space, or shares its key with another file (F-14: 26 duplicate-key groups)** — expected: the whole call aborts, nothing is allocated or written, and the refusal says which of the three it is (naming both files for a collision). Tests in Tasks 3 and 4.
4. **A worktree session appends to a params ledger, or `merge_worktree` folds a shadow whose colliding entry is defined in markdown** — expected: allocation from a worktree is refused as on the prose path; a fold that would renumber a markdown-defined id skips that row with a named conflict instead of splitting params from markdown. Tests in Task 6.
5. **`doc(graft)` of two augmented ledgers whose entry ids collide** — expected: when a colliding id is defined by a heading, the graft is refused with a RecoverableError and nothing changes; a collision between params-only rows renumbers as before. Test in Task 6.

---

### Task 1: The dual-write contract

**Files:**
- Create: `src/librarian/catalog/dual_write.rs`
- Modify: `src/librarian/catalog/mod.rs` (`pub(crate) mod dual_write;` beside `pub mod augmentation;` at :9); `src/librarian/catalog/augmentation.rs` — move `record_written_file` (:2307-2330), `SectionRollback` (:1090-1099), `restore_section_after_failed_commit` (:1108-1137) and `commit_failed_after_section_write` (:1152-1200) into the new module with their tests (`restoring_a_section_still_restores_a_regular_file`, `a_file_changed_by_another_writer_is_left_alone_rather_than_restored`, `an_unreadable_path_reports_failed_not_restored`, `the_rolled_back_and_still_on_disk_remedies_point_in_opposite_directions`); make `splice_pending_section` (:2345) `pub(crate)`; route `allocate_entry_id` (:1831-2025) and `append_entry` (:810-1086) through the new functions.
- Test: `src/librarian/catalog/dual_write.rs`

**Interfaces:**
- Consumes: `augmentation::{PendingSection, splice_pending_section, entry_high_water_key, body_entry_heading_level}`, `frontmatter::{parse, upsert_int_line}` (`frontmatter.rs:387`), `util::write_catalogued_file`, `current_project::lookup_git_root` (`current_project.rs:55`), `identity::key::key_of`.
- Produces:
  - `pub(crate) struct FileWrite { pub path: std::path::PathBuf, pub before: String, pub after: String }`
  - `pub(crate) fn record_written_file(tx: &rusqlite::Transaction<'_>, artifact_id: &str, path: &str, written: &str) -> anyhow::Result<()>` (moved unchanged).
  - `pub(crate) fn frontmatter_mark(doc: &str, id_prefix: &str) -> Option<u64>` — the committed `entry_high_water_<P>` value, a number or a numeric string, as `allocate_entry_id` reads it today (:1912-1920).
  - `pub(crate) fn write_entry_into_ledger(tx: &rusqlite::Transaction<'_>, roots: &[std::path::PathBuf], artifact_id: &str, path: &std::path::Path, doc: &str, id_prefix: &str, num: u64, section: Option<&PendingSection>, caller: &str) -> anyhow::Result<FileWrite>` — sets the mark to `max(frontmatter_mark, num)` with `upsert_int_line` (a document with no frontmatter block is refused with today's hint, nothing written), splices `section` at `body_entry_heading_level(...).unwrap_or(2)`, writes through `write_catalogued_file`, then `record_written_file`. One file write carries the mark and the section.
  - `pub(crate) fn commit_after_file_write(tx: rusqlite::Transaction<'_>, written: Option<&FileWrite>, token: &str, caller: &str) -> anyhow::Result<()>` — commits; on a commit error restores `before` only when the file still holds `after` (today's rule) and returns a `LibrarianRecoverableError` that names what reached disk and what did not, with the document named by `document_label`.
  - `pub(crate) fn document_label(abs_path: &std::path::Path) -> String` — `` `[[key]]` `` when the git root and `key_of` give a key, else the file name.
  - `pub(crate) fn find_unrecorded_section(doc: &str, id_prefix: &str, title: &str, section_body: &str, is_recorded: &dyn Fn(u64) -> bool) -> Option<u64>` — over `link_scan::extract::entry_sections(doc)` (`extract.rs:67`; `EntrySection.text` includes the heading line): the lowest `N` whose heading is `<#…> PREFIX-N — <title>`, whose section text after the heading equals `section_body` once both are trimmed and a leading `**Valid:** dated YYYY-MM-DD` line is dropped from each, and for which `is_recorded(N)` is false. Pure.

**Which writers change order, and which stay (stated here once):**
- `append_entry`, params path: already stages rows, writes the section, then commits, with compensation (:1065-1072). Stays in that order; now through `write_entry_into_ledger` + `commit_after_file_write`.
- `append_entry`, prose path: writes the mark and section inside the transaction, then a bare `tx.commit()?` (`augmentation.rs` ~:2015). **Changes:** the commit goes through `commit_after_file_write`, so a failed commit restores the bytes or reports them by name.
- `doc(update)`: file first (`update.rs:854`), then the row, `file_written_but_catalog_failed` on failure (:242). Stays.
- `doc(create)`: catalog transaction first, then a plain `std::fs::write` with `undo_failed_create` (`create.rs:506-583`). Stays: the file is the new document itself, the catalog-first order keeps a refused create from leaving a file that blocks the retry, and C1 adds no committed home to it (Task 8 adds a sidecar only with operator sign-off).
- `event_create`: frontmatter first (`event_create.rs:391`), then the transaction (:476-493). Stays; Plan C2 owns it.
- `doc(augment)`: catalog first, then `write_through` of the sidecar, behind `preflight_write_through` (`augment.rs:391`). Stays: the sidecar is rendered from the upserted row.

- [ ] **Step 1: Write the failing tests** in `dual_write.rs`:

```rust
#[test]
fn one_file_write_carries_the_mark_and_the_section_and_the_row_records_its_sha() {
    // ledger fixture: frontmatter `entry_prefix: F`, anchor `## Template for new entries`
    let w = write_entry_into_ledger(&tx, &[], "art1", &md, &before, "F", 1, Some(&pending("first")), "test").unwrap();
    tx.commit().unwrap();
    let text = std::fs::read_to_string(&md).unwrap();
    assert!(text.contains("entry_high_water_F: 1"));
    assert_eq!(text.matches("## F-1 — first").count(), 1);
    assert_eq!(w.after, text);
    assert_eq!(row_sha(&cat, "art1"), crate::librarian::util::sha_of_bytes(text.as_bytes()));
}
// mutation: drop the record_written_file call → the sha assertion goes red.

#[test]
fn the_mark_never_moves_down() {
    // frontmatter carries `entry_high_water_F: 9`; write num = 4 (Plan D writing an old id)
    assert!(std::fs::read_to_string(&md).unwrap().contains("entry_high_water_F: 9"));
}
// mutation: write `num` instead of `max(mark, num)`.

#[test]
fn a_document_with_no_frontmatter_is_refused_and_left_byte_identical() { /* Err; file == before */ }

#[test]
fn a_prose_commit_failure_restores_the_file_and_says_retry_as_is() {
    // allocate_entry_id with a section; a deferred-FK tripwire on `entry_reservation` INSERT
    // (same shape as augmentation.rs `arm_commit_tripwire`, :5843) fails the COMMIT
    let err = allocate_entry_id(&mut cat, "art1", "F", Some(&pending("first"))).unwrap_err();
    assert_eq!(std::fs::read_to_string(&md).unwrap(), before);
    assert!(hint_of(err).contains("safe to retry as-is"));
}
// mutation: put back the bare `tx.commit()?` → the file keeps `## F-1` and the test goes red.

#[test]
fn find_unrecorded_section_ignores_the_validity_stamp_and_skips_recorded_tokens() {
    let doc = "---\nentry_prefix: F\n---\n\n## F-4 — first\n\n**Valid:** dated 2026-01-01\n\nprose\n\n## Template for new entries\n";
    assert_eq!(find_unrecorded_section(doc, "F", "first", "prose", &|_| false), Some(4));
    assert_eq!(find_unrecorded_section(doc, "F", "first", "prose", &|n| n == 4), None);
    assert_eq!(find_unrecorded_section(doc, "F", "first", "other prose", &|_| false), None);
    assert_eq!(find_unrecorded_section(doc, "F", "second", "prose", &|_| false), None);
}

#[test]
fn document_label_is_the_grammar_form_or_the_file_name() {
    // tempdir with a `.git` dir: docs/ledger.md -> "`[[ledger]]`"; docs/Bad_Name.md -> "Bad_Name.md"
}
```

- [ ] **Step 2: Run** `scripts/with-slot.sh cargo test --lib catalog::dual_write` → FAIL (module missing).
- [ ] **Step 3: Implement** the module and move the four items; change `commit_failed_after_section_write`'s texts to name `document_label(path)` where they name `{path}`, and update the moved test's path assertions to the label. Route both `append_entry` paths through `write_entry_into_ledger` and `commit_after_file_write`. No other behaviour changes in this task.
- [ ] **Step 4: Run** `scripts/with-slot.sh cargo test --lib catalog::` and `scripts/with-slot.sh cargo test --lib tools::append_entry` → PASS (every existing append test stays green: this task is a refactor plus the prose commit compensation).
- [ ] **Step 5: Commit** — `refactor(librarian): one dual-write contract for entry writes; the prose path compensates a failed commit`.

---

### Task 2: A retry finds the section it already wrote

**Files:**
- Modify: `src/librarian/catalog/augmentation.rs` (`append_entry`, `allocate_entry_id`, `AppendOutcome` :780, `AllocateOutcome` :1204), `src/librarian/catalog/dual_write.rs` (remedy texts), `src/librarian/tools/append_entry.rs` (response fields and `next_step` text, :397-412 and :463-470)
- Test: `src/librarian/catalog/augmentation.rs` (`mod tests`), `src/librarian/catalog/dual_write.rs`

**Interfaces:**
- Consumes: Task 1 `find_unrecorded_section`, `write_entry_into_ledger`, `commit_after_file_write`.
- Produces: `pub adopted: bool` on `AppendOutcome` and on `AllocateOutcome`; the tool responses carry `"adopted": true` only when set.

The rule: before allocating, when the call carries a section, `find_unrecorded_section` runs over the body read inside the transaction. Params path: `is_recorded(n)` = params already hold `<P>-n`. Prose path: `is_recorded(n)` = `reserved_max.is_some_and(|r| n <= r)`. A hit is adopted: that id is used, the params row (params path) or the reservation (prose path) and any `entry_cite` rows are written for it, and the file is written only when its mark is below `n` (then `write_entry_into_ledger` with `section: None`). Identical title and body therefore mean one entry, also for a caller who wrote the section by hand first.

- [ ] **Step 1: Write the failing tests:**

```rust
#[test]
fn a_params_retry_adopts_a_section_the_file_has_and_params_lack() {
    // ledger body already holds `## F-1 — first\n\n**Valid:** dated 2026-01-01\n\nprose`, params empty
    let out = append_entry(&mut cat, "art1", "failures", "F", json!({}), &[], Some(&pending("first"))).unwrap();
    assert_eq!(out.id, "F-1");
    assert!(out.adopted);
    assert_eq!(std::fs::read_to_string(&md).unwrap().matches("## F-").count(), 1);
    assert_eq!(params_ids(&cat, "art1"), vec!["F-1"]);
}
// mutation: skip the adoption lookup → id F-2 and two sections.

#[test]
fn a_prose_retry_adopts_the_section_and_records_the_reservation() {
    // section F-1 on disk, mark 1, no entry_reservation row (a commit that failed and could not restore)
    let out = allocate_entry_id(&mut cat, "art1", "F", Some(&pending("first"))).unwrap();
    assert_eq!((out.id.as_str(), out.adopted), ("F-1", true));
    assert_eq!(reserved_max(&cat, "art1", "F"), Some(1));
    assert_eq!(std::fs::read_to_string(&md).unwrap(), before); // nothing rewritten
}

#[test]
fn a_different_body_under_the_same_title_is_a_new_entry() { /* id F-2, adopted false */ }

#[test]
fn a_recorded_section_is_never_adopted() { /* params hold F-1 → new call allocates F-2 */ }
```

And in `dual_write.rs`, replace the assertions of `the_rolled_back_and_still_on_disk_remedies_point_in_opposite_directions` (rename it `the_two_remedies_name_their_own_retry`): the restored branch says `safe to retry as-is`; the still-on-disk branch says `retry with the same title and body` and names the token, and does not say `safe to retry as-is`. Mutation: give both branches one text.

- [ ] **Step 2: Run** `scripts/with-slot.sh cargo test --lib catalog::augmentation catalog::dual_write` → FAIL.
- [ ] **Step 3: Implement** the rule above; the still-on-disk hint becomes: retry with the same `title` and `body`, and the retry records the section as `<token>` instead of writing a second one. In the tool, an adopted outcome's `next_step` says the entry was already in the file and is now recorded, and that nothing new was written.
- [ ] **Step 4: Run** the same filters plus `tools::append_entry` → PASS.
- [ ] **Step 5: Commit** — `feat(librarian): an append retried after a partial write records the section it already wrote`.

---

### Task 3: Citations as grammar text, and the commit oracle

**Files:**
- Create: `src/librarian/catalog/cite_text.rs`
- Modify: `src/librarian/catalog/mod.rs`; `src/librarian/derived/testkit.rs` (the commit oracle)
- Test: `src/librarian/catalog/cite_text.rs`, `src/librarian/derived/testkit.rs`

**Interfaces:**
- Consumes: the `artifact` table (`id`, `abs_path`, `slug`, `missing_since`), `identity::key::key_of`, `identity::key_space::load` (`key_space.rs:114`, reads through `for_project`), `current_project::lookup_git_root`, `util::{root_and_prefix, path_at_or_under_sql}` (`util.rs:311`, `:302`); `derived::{scan::open_commit, build::{rules_from_commit, key_space_from_commit, build_full, BuildInput}, schema::open_in_memory}`, `testkit::repo_with`.
- Produces:
  - `pub(crate) const CITES_LABEL: &str = "**Cites:**";`
  - `pub(crate) fn grammar_form_of(conn: &rusqlite::Connection, dst_ref: &str) -> anyhow::Result<String>` — `dst_ref` is what `resolve_cite_ref` (`augmentation.rs:2457`) returns: a 16-hex artifact id → `[[key]]`; `<slug>:<local>` → the artifact holding that slug → `[[key:local]]`. Refused with a `LibrarianRecoverableError`, each naming the target's file name and the reason: no git root; `key_of` fails (non-canonical, hex-shaped); the path lies outside the key space; another non-missing artifact under the same git root, inside the key space, has the same key (the refusal lists both repo-relative paths and says a rename or a key-space rule must settle it).
  - `pub(crate) fn cites_line(forms: &[String]) -> String` — `**Cites:** [[a]], [[b:F-3]]`, input order, exact duplicates dropped.
  - `pub(crate) fn with_cites(section: &PendingSection, forms: &[String]) -> PendingSection` — `body.trim_end()` + `"\n\n"` + `cites_line(forms)`; a clone when `forms` is empty.
  - In `testkit.rs`: `pub(crate) struct Built { pub conn: rusqlite::Connection, _dir: tempfile::TempDir }`, `pub(crate) fn build_files(files: &[(&str, &str)]) -> Built` (one commit through `repo_with`, then `open_commit(HEAD)`, `rules_from_commit`, `key_space_from_commit`, `build_full` into `open_in_memory`, root `/fixture/root`), `pub(crate) fn entry_tokens(conn: &rusqlite::Connection, rel_path: &str) -> Vec<String>`, `pub(crate) fn edges_from(conn: &rusqlite::Connection, rel_path: &str) -> Vec<(String, Option<String>, Option<String>)>` (`kind`, `target_key`, `target_token`, by `ord`; kinds are `derived/build.rs:597-610`'s strings).

- [ ] **Step 1: Write the failing tests** (temp dir with a `.git` directory, in-memory catalog seeded with artifact rows):

```rust
#[test] fn a_file_ref_becomes_a_key_and_an_entry_ref_a_key_with_its_token() {
    assert_eq!(grammar_form_of(&cat.conn, &id_of("docs/target.md")).unwrap(), "[[target]]");
    assert_eq!(grammar_form_of(&cat.conn, "target-slug:F-3").unwrap(), "[[target:F-3]]");
    assert_eq!(grammar_form_of(&cat.conn, &id_of("docs/area/readme.md")).unwrap(), "[[area]]");
}
#[test] fn a_non_canonical_target_name_is_refused_by_name() { /* docs/Bad_Name.md → Err containing "Bad_Name.md" and "not a canonical key" */ }
#[test] fn a_target_outside_the_key_space_is_refused() { /* .codescout/key-space.toml excludes contrib/** */ }
#[test] fn a_shared_key_is_refused_naming_both_files() {
    // docs/a/notes.md and docs/b/notes.md
    let msg = format!("{:#}", grammar_form_of(&cat.conn, &id_of("docs/a/notes.md")).unwrap_err());
    assert!(msg.contains("docs/a/notes.md") && msg.contains("docs/b/notes.md"));
}
// mutation: drop the duplicate query → Ok("[[notes]]").
#[test] fn a_missing_row_does_not_count_as_a_collision() { /* second notes.md has missing_since set → Ok */ }
#[test] fn cites_line_keeps_order_and_drops_repeats() {
    assert_eq!(cites_line(&["[[b]]".into(), "[[a:F-1]]".into(), "[[b]]".into()]), "**Cites:** [[b]], [[a:F-1]]");
}
#[test] fn the_commit_oracle_sees_what_a_commit_holds() {      // testkit.rs
    let b = build_files(&[("docs/ledger.md", "## F-1 — x\n\n**Cites:** [[target]]\n"), ("docs/target.md", "# T\n")]);
    assert_eq!(entry_tokens(&b.conn, "docs/ledger.md"), vec!["F-1"]);
    assert_eq!(edges_from(&b.conn, "docs/ledger.md"), vec![("file".into(), Some("target".into()), None)]);
}
```

- [ ] **Step 2: Run** `scripts/with-slot.sh cargo test --lib catalog::cite_text derived::testkit` → FAIL.
- [ ] **Step 3: Implement.** The duplicate check queries `SELECT abs_path FROM artifact WHERE missing_since IS NULL AND <path_at_or_under_sql>` under the git root and compares `key_of` of each repo-relative path that the key space keeps; the old catalog is the population (a file not yet indexed is not seen; the commit oracle reports such a collision as `key_collision`).
- [ ] **Step 4: Run** → PASS.
- [ ] **Step 5: Commit** — `feat(librarian): render a resolved citation as grammar text; a commit oracle for dual-write tests`.

---

### Task 4: The params path writes the entry into the markdown

**Files:**
- Modify: `src/librarian/catalog/augmentation.rs` (`append_entry` :810-1086), `src/librarian/tools/append_entry.rs` (response, :443-486), `src/librarian/tools/artifact.rs` (description :247 and the `cites` schema text :484), `src/prompts/guides/librarian.md` (the `append_entry` example at :184), `src/prompts/guides/tracker-conventions.md` (§ Entry ids, :187-201)
- Test: `src/librarian/catalog/augmentation.rs`, `src/librarian/tools/append_entry.rs`

**Interfaces:**
- Consumes: Tasks 1-3.
- Produces: `AppendOutcome` gains `pub cites: Vec<String>` (the grammar forms written) and `pub key_note: Option<String>`; the tool response carries `"cites"` when non-empty and `"key_note"` when set. Signature of `append_entry` unchanged.

Behaviour, in order inside the transaction: (1) `section: None` is refused before the transaction opens — the message says an entry on a params ledger is written into the markdown too and names `title`, `body` and `anchor_heading`; (2) `next = max(params_next, body_max + 1, frontmatter_mark + 1)`; (3) each `cites` ref goes through `resolve_cite_ref` and `grammar_form_of` (any refusal aborts the call, nothing written); (4) the section is `with_cites(section, &forms)`, and Task 2's adoption compares against that rendered body; (5) the params row and the `entry_cite` `origin='write'` rows are staged as today (:981-998); (6) `write_entry_into_ledger` writes section and mark; (7) `commit_after_file_write`. `key_note` is set when `key_of` fails for the ledger itself: the entry is written and its bare token resolves inside the ledger, but `[[key:TOKEN]]` cannot name it until the file is renamed. No `entry_reservation` row on this path: the params row is the catalog's claim.

**The old scanner beside the write row.** `link_scan`'s `cross_repo_re` (`link_scan/extract.rs:287-290`) matches `target:F-3` inside `[[target:F-3]]` and derives `dst_ref = <dst_slug>:F-3` (`link_scan/mod.rs:183-192`), the same shape `resolve_cite_ref` stores. `entry_cite`'s primary key leaves `origin` out (`entry_cite.rs:16-28`), so the scan's insert is ignored and the write row stays. A file cite `[[target]]` has no scanner twin (the old scanner does not parse `[[`), so the write row is its only old-catalog edge until the flip.

- [ ] **Step 1: Write the failing tests:**

```rust
#[test] fn a_params_append_without_a_section_is_refused_and_allocates_nothing() {
    let err = append_entry(&mut cat, "art1", "failures", "F", json!({}), &[], None).unwrap_err();
    assert!(format!("{err:#}").contains("anchor_heading"));
    assert!(params_ids(&cat, "art1").is_empty());
    assert_eq!(std::fs::read_to_string(&md).unwrap(), before);
}
#[test] fn a_params_append_writes_the_section_and_raises_the_mark() {          // parse oracle
    let out = append_entry(/* … */ Some(&pending("first"))).unwrap();
    let text = std::fs::read_to_string(&md).unwrap();
    assert!(text.contains("entry_high_water_F: 1"));
    let parsed = parse_document("docs/ledger.md", &text, &default_rules().unwrap());
    assert_eq!(parsed.entries.iter().map(|e| e.token.as_str()).collect::<Vec<_>>(), vec![out.id.as_str()]);
}
// mutation: pass `section: None` to write_entry_into_ledger → no entry parsed.
#[test] fn the_committed_mark_counts_on_the_params_path() {
    // frontmatter `entry_high_water_F: 7`, params empty, no heading → F-8
}
// mutation: drop `frontmatter_mark + 1` from the max → F-1.
#[test] fn params_cites_are_grammar_text_and_write_rows() {                    // parse oracle
    let out = append_entry(/* … */ &[id_of("docs/other.md"), "target-slug:F-3".into()], Some(&pending("first"))).unwrap();
    let parsed = parse_document("docs/ledger.md", &std::fs::read_to_string(&md).unwrap(), &rules);
    assert!(parsed.citations.iter().any(|c| c.reference == Reference::File { repo: None, key: Key::parse("other").unwrap() }));
    assert!(parsed.citations.iter().any(|c| c.reference == Reference::Entry { repo: None, key: Key::parse("target").unwrap(), token: "F-3".into() }));
    assert_eq!(write_rows(&cat, &out.id), 2);
    assert_eq!(out.cites, vec!["[[other]]", "[[target:F-3]]"]);
}
#[test] fn a_cite_that_names_a_shared_key_aborts_the_whole_append() {          // Review Focus 3
    // docs/a/notes.md + docs/b/notes.md; cite the first
    assert!(append_entry(/* … */).is_err());
    assert_eq!(std::fs::read_to_string(&md).unwrap(), before);
    assert!(params_ids(&cat, "art1").is_empty());
}
#[test] fn a_dual_written_entry_resolves_in_the_derived_index_after_a_commit() { // commit oracle
    let b = build_files(&[("docs/ledger.md", &ledger_text), ("docs/target.md", "## F-3 — t\n"), ("docs/other.md", "# O\n")]);
    let edges = edges_from(&b.conn, "docs/ledger.md");
    assert!(edges.contains(&("entry".into(), Some("target".into()), Some("F-3".into()))));
    assert!(edges.contains(&("file".into(), Some("other".into()), None)));
}
#[test] fn a_ledger_whose_own_name_is_not_a_key_still_takes_the_entry_and_says_so() {
    // ledger file `docs/Ledger_One.md` → Ok, key_note is Some and mentions "Ledger_One.md"
}
```

In `tools/append_entry.rs`:

```rust
#[tokio::test] async fn a_write_scan_after_a_cited_append_keeps_one_write_row() {
    // append with cites ["target-slug:F-3"] (target has the F-3 params row and a `## F-3 — t` heading)
    let out = link_scan::call(&ctx, json!({ "write": true })).await.unwrap();
    assert_eq!(out["counts"]["entry_edges"]["skipped_existing"], json!(1));
    assert_eq!(rows_for(&cat, &new_id), vec![("target-slug:F-3".to_string(), "write".to_string())]);
}
```

If this test shows `written: 1` instead (the scan derived a different `dst_ref`), stop and report it: that is a finding about the old scanner, not a C1 defect, and the assertion is not to be loosened. Copy the context set-up from `link_scan/mod.rs:1477-1505`.

- [ ] **Step 2: Run** `scripts/with-slot.sh cargo test --lib catalog::augmentation tools::append_entry` → FAIL, including every existing test that calls the params path with no section.
- [ ] **Step 3: Implement** the behaviour above. Give each existing params-path test a section (the module's `pending` helper in `augmentation.rs` tests; `title`, `body`, `anchor_heading` in the tool tests' args), and give their fixtures the anchor heading. Replace the `doc` description's "with entry_collection it appends a params row" with words saying the params path also writes the section and needs `title`, `body` and `anchor_heading`, keeping the description within its cap; the `cites` schema text says each ref is written into the entry as `[[key]]`/`[[key:TOKEN]]` and recorded as an edge. Update the two guide passages the same way (current situation only).
- [ ] **Step 4: Run** `scripts/with-slot.sh cargo test --lib catalog:: tools::append_entry tools::link_scan prompts::`, `scripts/with-slot.sh cargo test --lib every_tool_description_is_under_its_cap` → PASS. Then `./scripts/gate.sh` → green.
- [ ] **Step 5: Commit** — `feat(librarian): a params-ledger entry is written into the markdown with its mark and its citations`.

---

### Task 5: The prose path writes `cites` too

**Files:**
- Modify: `src/librarian/catalog/augmentation.rs` (`allocate_entry_id`), `src/librarian/tools/append_entry.rs` (the prose `cites` refusal :228-235; the call at :335; the response)
- Test: both files

**Interfaces:**
- Consumes: Tasks 1-3; `artifact::ensure_slug`, `entry_cite::insert_with`.
- Produces: `pub fn allocate_entry_id_with_cites(cat: &mut Catalog, artifact_id: &str, id_prefix: &str, section: Option<&PendingSection>, cites: &[String]) -> Result<AllocateOutcome>`; `allocate_entry_id(cat, artifact_id, id_prefix, section)` becomes a call of it with `&[]`, so its 30 callers do not change. `AllocateOutcome` gains `pub cites: Vec<String>`.

Why the refusal exists and why it goes: the prose path refuses `cites` with "cite in prose — link_scan derives the edges from the text" (`tools/append_entry.rs:229-233`). Writing the citation into the section text is exactly that, done by the server, so the refusal's own reason now admits the call. `cites` with no section stays refused (there is no entry text to write it into), before any allocation. On the prose path the same `entry_cite` `origin='write'` rows are inserted as on the params path, so old-catalog readers see one edge set on both paths during the dual-write window.

- [ ] **Step 1: Write the failing tests:**

```rust
#[test] fn prose_cites_are_grammar_text_and_write_rows() {                      // parse oracle
    let out = allocate_entry_id_with_cites(&mut cat, "art1", "F", Some(&pending("first")), &[id_of("docs/other.md")]).unwrap();
    let parsed = parse_document("docs/ledger.md", &std::fs::read_to_string(&md).unwrap(), &rules);
    assert!(parsed.citations.iter().any(|c| matches!(&c.reference, Reference::File { key, .. } if key.as_str() == "other")));
    assert_eq!(write_rows(&cat, &out.id), 1);
}
// mutation: skip with_cites on the prose path → no citation parsed.
#[test] fn prose_cites_without_a_section_are_refused_and_reserve_nothing() { /* Err; reserved_max None; file == before */ }
#[test] fn a_prose_cite_of_a_non_canonical_name_aborts_and_reserves_nothing() { /* Err; reserved_max None */ }
```

Rename `a_prose_ledger_refuses_cites` (`tools/append_entry.rs:1046`) to `a_prose_ledger_takes_cites_with_a_section_and_refuses_them_without` and assert both halves.

- [ ] **Step 2: Run** → FAIL. **Step 3: Implement.** **Step 4: Run** `scripts/with-slot.sh cargo test --lib catalog::augmentation tools::append_entry` → PASS.
- [ ] **Step 5: Commit** — `feat(librarian): a prose-ledger entry takes cites and writes them as grammar text`.

---

### Task 6: No allocation from a worktree; no renumber that splits params from markdown

**Files:**
- Modify: `src/librarian/tools/append_entry.rs` (factor the prose path's two shadow guards, :236-300, into one function used by both paths; the params path's cites-only worktree refusal :425-440 is subsumed), `src/librarian/catalog/graft.rs` (`graft_rows` :129, `merge_augmentation` :440), `src/librarian/tools/merge_worktree.rs` (`merge_one` :231, after the fold at :318-323 and before the `dry_run` return)
- Test: all three files

**Interfaces:**
- Consumes: `tools::worktree::{is_main_checkout_artifact, is_worktree_shadow, has_fork_record}` (`worktree.rs:28`, `:72`, `:44`), `link_scan::extract::entry_sections`, `read_door::for_catalog`.
- Produces:
  - `fn refuse_allocation_off_the_main_ledger(ctx: &ToolContext, cat: &Catalog, id: &str) -> Result<()>` (private, `tools/append_entry.rs`), called before `resolve_write_target` on both paths.
  - `pub(crate) fn renumbered_ids_defined_in(remap: &std::collections::BTreeMap<String, String>, bodies: &[&str]) -> Vec<String>` (`graft.rs`, pure): the old ids in `remap` that a heading defines in any body.
  - `merge_augmentation` gains `roots: &[std::path::PathBuf]` (its one caller, `graft_rows`, passes `cat.write_roots()` taken before the transaction).

Behaviour: a worktree session, or any session naming a shadow, gets the prose path's refusal on the params path too: an entry id is ledger-wide state, and once the params path writes `## F-N` into the worktree's file, a concurrent main-checkout append makes git merge two definers of one token (`doctor`'s `entry_defined_twice`, `doctor.rs:4867`). `graft_rows`: when the fold renumbers ids and `renumbered_ids_defined_in` is non-empty over the two documents' bodies, the graft is refused with a `LibrarianRecoverableError` naming the ids by token and the ledgers by `document_label`, saying nothing changed and that the colliding sections must be renumbered in the source document first; reading the bodies happens only when `remap` is non-empty, so `doc(move)` (destination never augmented) reads nothing new. `merge_one`: the same check over the shadow's and main's files returns `MergeOneOutcome::skipped` with conflict kind `entry_renumber_needs_markdown` and the ids; an unreadable file skips with the same kind and the reason. Renumbering params-only rows is unchanged.

- [ ] **Step 1: Write the failing tests:**

```rust
// tools/append_entry.rs — replaces `append_from_worktree_lands_on_shadow_not_main` (:1165)
#[tokio::test] async fn params_allocation_is_refused_from_a_worktree() {
    let err = call(&wt_ctx, json!({"id": main_id, "entry_collection": "failures", "id_prefix": "F",
        "title": "t", "body": "b", "anchor_heading": "## Template for new entries"})).await.unwrap_err();
    assert!(format!("{err:#}").contains("not supported from a worktree"));
    assert!(no_shadow_row(&ctx));   // refused before resolve_write_target forks
}
#[tokio::test] async fn params_allocation_is_refused_on_a_shadow_from_a_main_checkout_session() { /* has_fork_record */ }

// graft.rs
#[test] fn a_graft_that_would_renumber_a_markdown_defined_entry_is_refused() {   // Review Focus 5
    // both rows augmented with `failures`; from: params [F-1], body `## F-1 — x`; into: params [F-1]
    assert!(format!("{:#}", graft_rows(&mut cat, "from", "into").unwrap_err()).contains("F-1"));
    assert!(row_exists(&cat.conn, "from").unwrap());              // nothing changed
}
#[test] fn a_graft_of_colliding_params_only_rows_still_renumbers() { /* report.remap == {F-1: F-2} */ }
#[test] fn renumbered_ids_defined_in_reads_headings_not_prose() {
    let remap = BTreeMap::from([("F-1".into(), "F-3".into()), ("F-2".into(), "F-4".into())]);
    assert_eq!(renumbered_ids_defined_in(&remap, &["## F-1 — x\n\nsee F-2\n"]), vec!["F-1"]);
}

// merge_worktree.rs                                                            // Review Focus 4
#[test] fn a_fold_that_would_renumber_a_markdown_defined_entry_skips_the_row() {
    // shadow appended F-2 (heading in the worktree file); main also holds F-2
    let out = merge_one(&mut cat, "shadow", "main", false, 0).unwrap();
    assert!(!out.merged);
    assert_eq!(out.conflicts[0]["kind"], json!("entry_renumber_needs_markdown"));
}
// mutation: drop the check → merged true and remap {F-2: F-3} while both files say F-2.
```

- [ ] **Step 2: Run** `scripts/with-slot.sh cargo test --lib tools::append_entry catalog::graft tools::merge_worktree` → FAIL. **Step 3: Implement**, and update the § Entry ids — refusals guide passage (Task 4's) to cover every ledger. **Step 4: Run** → PASS.
- [ ] **Step 5: Commit** — `feat(librarian): entry ids are allocated on the main ledger only, and a fold never renumbers a markdown-defined entry`.

---

### Task 7: The CLI write commands take the project write lock

**Files:**
- Create: `src/cli/write_lock.rs`, `tests/cli_write_lock.rs`
- Modify: `src/cli/mod.rs` (module); `src/cli/doc.rs` (`run_create` :497, `run_update` :634, `run_move` :663, `run_link` :697); `src/cli/doc_event.rs` (`run_create` :113); `src/cli/doc_augment.rs` (`run` :41, before `run_with_ctx`); `src/cli/doctor.rs` (`run` :166, when `--fix` is given); `src/cli/audit_doc_refs.rs` (`run` :76, unless `--no-emit-tracker`)

**Interfaces:**
- Consumes: `crate::agent::{acquire_write_guard, open_lock_file, holder_record_path, WriteGuard}` (`agent/mod.rs:10-12`; `acquire` signature `write_guard.rs:229-235`), `ProjectConfig::load_or_default` (`config/project.rs:862`) for `security.write_lock_timeout_secs` (default 5), `ToolContext.current_project` (`CurrentProject { abs_path, git_root, .. }`, `current_project.rs:23`).
- Produces:
  - `pub(crate) fn lock_root(cp: &CurrentProject) -> std::path::PathBuf` — the nearest directory from `cp.abs_path` up to and including `cp.git_root` that holds `.codescout/project.toml`; else `cp.git_root`.
  - `pub(crate) async fn take(ctx: &crate::librarian::tools::ToolContext, command: &str) -> anyhow::Result<crate::agent::WriteGuard>` — resolves `lock_root`, reads the timeout, and calls `take_at`; no current project is an error that names `--project`.
  - `pub(crate) async fn take_at(root: &std::path::Path, timeout: std::time::Duration, command: &str) -> anyhow::Result<crate::agent::WriteGuard>` — a fresh in-process mutex, `open_lock_file(root)`, `holder_record_path(root)`, holder label `codescout-cli <command>`; the testable core.
  - `fn takes_the_write_lock(args: &DoctorArgs) -> bool` (`fix.is_some()`, mirroring the MCP arm, `adapter.rs` "doctor and audit_log … opt IN") and `fn takes_the_write_lock(args: &AuditArgs) -> bool` (`!no_emit_tracker`, mirroring `adapter.rs:491-493`).

How a CLI process finds the root today: `open_ctx` (`cli/mod.rs:62`) hands `--project` to `build_tool_context` (`librarian/mod.rs:309`), whose `current_project::resolve` canonicalises the path and walks up to the nearest `.git`. The MCP server locks `<ActiveProject.root>/.codescout/write.lock` (`server.rs:1109-1118`), the activated project's root. `lock_root` reaches the same file whenever the activated project is the nearest directory holding a `project.toml`. The lock is taken after `open_ctx` and held until the tool body returns. Commands that do not write (`find`, `get`, `graph`, `state-at`, `event list`, `refresh gather|list-stale`) take none; `backfill-chunks` writes catalog rows only and stays outside this task.

- [ ] **Step 1: Write the failing tests.** Unit, in `write_lock.rs`:

```rust
#[test] fn the_lock_root_is_the_nearest_project_toml_within_the_git_root() {
    // repo/.git, repo/.codescout/project.toml, cwd repo/sub/dir → repo
    // repo2/.git only, cwd repo2/x → repo2 (the git root)
}
#[tokio::test] async fn a_held_lock_refuses_the_cli_and_names_the_holder() {
    let held = acquire_write_guard(Arc::new(Mutex::new(())), open_lock_file(&root).unwrap(),
        holder_record_path(&root), Duration::from_secs(1), "codescout:sid-beta append_entry").await.unwrap();
    let err = take_at(&root, Duration::from_millis(200), "doc update").await.unwrap_err();
    assert!(format!("{err:#}").contains("codescout:sid-beta append_entry"));
    drop(held);
    assert!(take_at(&root, Duration::from_millis(200), "doc update").await.is_ok());
}
#[test] fn doctor_takes_the_lock_only_with_a_fix() { /* fix None → false; Some("export_augmentations") → true */ }
#[test] fn audit_doc_refs_takes_the_lock_unless_it_emits_no_tracker() { /* default → true; --no-emit-tracker → false */ }
```

(`take_at` is the core `take` calls, listed under Interfaces.) Integration, modelled on `tests/cross_process_write_lock.rs:110-165`:

```rust
#[test] fn cli_doc_create_waits_for_the_project_lock_and_names_its_holder() {     // Review Focus 2
    // tempdir: .git dir, .codescout/project.toml = "[project]\nname = \"t\"\n[security]\nwrite_lock_timeout_secs = 1\n"
    // test process: fs4 try_lock_exclusive on .codescout/write.lock; holder record "<now>\tcodescout:sid-beta append_entry"
    let out = Command::new(env!("CARGO_BIN_EXE_codescout"))
        .args(["doc", "create", "--project", dir, "--kind", "note", "--title", "T", "--rel-path", "docs/t.md"])
        .env("LIBRARIAN_DB", state.join("catalog.db")).env("LIBRARIAN_WORKSPACE", &ws).env("XDG_STATE_HOME", &state)
        .output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("codescout:sid-beta append_entry"));
    assert!(!dir.join("docs/t.md").exists());
}
// mutation: remove the `take` call from run_create → exit 0 and docs/t.md exists.
```

- [ ] **Step 2: Run** `scripts/with-slot.sh cargo test --lib cli::write_lock` and `scripts/with-slot.sh cargo test --test cli_write_lock` → FAIL.
- [ ] **Step 3: Implement**, and wire `take` into the eight commands named above.
- [ ] **Step 4: Run** both → PASS; `scripts/with-slot.sh cargo test --test cross_process_write_lock` → PASS. Then `./scripts/gate.sh` → green.
- [ ] **Step 5: Commit** — `feat(cli): write commands take the project write lock`.

---

### Task 8: Operator sign-off required — create the sidecar when none exists

Drop this task, and nothing else changes: no other task reads its interfaces.

**Alternative (if the operator keeps the current position):** `doc(augment)` keeps updating only an already-declared, existing sidecar, and `doc(create)` writes none, as `augmentation_sidecar.rs:348-352` documents. Plan D's cutover tool then creates every missing sidecar inside the migration window with the existing export (`doctor fix=export_augmentations`, `doctor.rs:2021`), and augmented documents created before the window stay catalog-only until then.

**Files:**
- Modify: `src/librarian/augmentation_sidecar.rs` (a creating path beside `write_through` :386; the module doc and `write_through`'s doc lose the "never creates" position), `src/librarian/tools/doctor.rs` (`export_augmentation_sidecars` :2021 and `stamp_augmentation_declaration` :2147 call the shared function), `src/librarian/tools/augment.rs` (`sidecar_write_through` :65 and its callers :270, :503), `src/librarian/tools/create.rs` (the `augment` branch, :510-560 and the frontmatter at :470-482)
- Test: `src/librarian/augmentation_sidecar.rs`, `src/librarian/tools/augment.rs`, `src/librarian/tools/create.rs`

**Interfaces:**
- Consumes: `AugmentationSidecar::from_row`, `path_for`, `rel_path_for` (:79, :89), `confined_path` (:198), `write` (:309), `util::{refuse_write_through_symlink, refuse_path_outside_root}`, Task 1 `dual_write::record_written_file`.
- Produces:
  - `pub(crate) fn stamp_declaration(roots: &[std::path::PathBuf], artifact: &std::path::Path, sidecar_rel: &str) -> anyhow::Result<String>` — moved from `doctor.rs:2147`; returns the bytes written.
  - `pub(crate) fn create_for(cat: &crate::librarian::catalog::Catalog, artifact_id: &str) -> anyhow::Result<Option<String>>` — when the artifact has an augmentation row and no declared sidecar file: refuses a linked or escaping path as the export does (`doctor.rs:2088-2110`), writes the sidecar, stamps `expects_augmentation: <rel>`, records the stamped bytes with `record_written_file` in its own transaction, and returns the sidecar's repo-relative path. `Ok(None)` when a declared sidecar already exists or there is no row.
  - `sidecar_write_through` returns `Result<Option<String>>`; `doc(augment)` responses carry `"sidecar_created": "<repo-relative .yaml path>"` when one was created (a sidecar is not a document, so its path is not a document reference).
  - `doc(create)` with `augment`: `expects_augmentation: <rel_path_for(git_root, full)>` goes into the frontmatter `extra` before the one file write, and the sidecar is written after it; a sidecar failure returns an error saying the document and its catalog row exist and the sidecar does not.

- [ ] **Step 1: Write the failing tests:**

```rust
#[tokio::test] async fn augment_creates_and_declares_a_sidecar_when_none_exists() {
    let out = augment::call(&ctx, json!({"id": id, "prompt": "p"})).await.unwrap();
    assert_eq!(out["sidecar_created"], json!("docs/augmentations/docs-t.yaml"));
    let yaml = std::fs::read_to_string(root.join("docs/augmentations/docs-t.yaml")).unwrap();
    assert!(!yaml.contains("params:"));
    assert!(std::fs::read_to_string(root.join("docs/t.md")).unwrap().contains("expects_augmentation: docs/augmentations/docs-t.yaml"));
    assert_eq!(row_sha(&ctx, &id), sha_of_file(root.join("docs/t.md")));   // no row_behind_file
}
// mutation: skip create_for in sidecar_write_through → no file, field absent.
#[tokio::test] async fn augment_leaves_an_existing_declared_sidecar_to_write_through() { /* sidecar_created absent; write_through's refusal rules unchanged */ }
#[tokio::test] async fn create_with_augment_writes_the_declaration_in_its_one_file_write_and_then_the_sidecar() { /* file holds the declaration; sidecar exists; row sha == file sha */ }
```

`export_augmentations_writes_a_sidecar_stamps_the_declaration_and_is_idempotent` (`doctor.rs:23267`) and `every_committed_sidecar_parses_and_carries_no_params` stay green.

- [ ] **Step 2: Run** `scripts/with-slot.sh cargo test --lib augmentation_sidecar tools::augment tools::create tools::doctor::tests::export` → FAIL. **Step 3: Implement.** **Step 4: Run** → PASS.
- [ ] **Step 5: Commit** — `feat(librarian): doc(augment) and doc(create) write the committed sidecar when none exists`.

---

## Rulings made in this plan

| # | Ruling | Why | Cost if wrong |
|---|---|---|---|
| C1-R1 | File-first means: stage rows in the open `IMMEDIATE` transaction, write the file and `record_written_file`, then commit; a failed commit restores the bytes only when untouched, else reports them by name. | The catalog's durable step is the commit; the transaction is also the id-allocation lock and must be open before the body is read. Keeps today's tested compensation. | A crash between write and commit leaves a file-only entry; Task 2's adoption is the repair. |
| C1-R2 | Order changes only on the prose `append_entry` path; `update`, `create`, `event_create`, `augment` stay. | Each already reports its partial state, or (create) its order protects a retry; C1 gives them no new committed home. | A Plan D caller that needs another order re-opens this. |
| C1-R3 | The retry key is (prefix, heading title, section text minus the `**Valid:**` stamp) over sections the catalog has not recorded. | Needs no new marker in committed text; also records a section a caller wrote by hand first. | Two deliberately identical entries become one. |
| C1-R4 | Params path: section required (refused when absent); allocation folds the frontmatter mark; the mark is written; no `entry_reservation` on this path. | Spec contract requirement 1 and § Entry ids; the params row is the claim. | The 16 params ledgers' prompts tell agents to write the section separately; agents get a refusal with the fix until the prompts change (After the plan). |
| C1-R5 | Citations are one `**Cites:**` line at the end of the section, input order, repeats dropped. | Spec: relations are labelled grammar lines in the body; `ledger_field_labels` requires the `**Field:**` form. | A later label vocabulary (C2) renames the label. |
| C1-R6 | A cite is refused when its target has no git root, a non-canonical key, lies outside the key space, or shares its key with another catalogued file under the git root. | Spec: one key per document; F-14's 26 duplicate groups. | The duplicate check sees only indexed files; the commit oracle reports what it misses as `key_collision`. |
| C1-R7 | The `entry_cite` `origin='write'` row stays on both paths; a scan of the same edge is ignored by the primary key. | The old scanner does not read `[[key]]`, so file cites would vanish from the old catalog otherwise. | Plan D drops rows whose text twin exists. |
| C1-R8 | The prose path accepts `cites` when it writes a section, refuses them otherwise. | The refusal's reason ("cite in prose") is what the server now does. | none found. |
| C1-R9 | Params-path allocation is refused from a worktree, as on the prose path. | A section in the worktree's file plus a main-checkout append merges into two definers of one token. | Worktree sessions append to params ledgers from the main checkout only; reverses `append_from_worktree_lands_on_shadow_not_main`. Listed for the operator below. |
| C1-R10 | A renumber of a markdown-defined entry is refused by `graft` and skipped by `merge_worktree`; params-only renumbers continue. | No tool renumbers one entry in both stores, and a guessed heading rewrite cannot tell two `## F-N` apart. | A legacy shadow with such a collision needs a hand fix before it merges. |
| C1-R11 | CLI lock root: nearest `project.toml` up to the git root, else the git root; eight commands guarded; holder `codescout-cli <command>`. | Matches the MCP server's lock file in the ordinary layouts without new config. | A server activated on another root takes another lock (as two MCP projects in one repo already do). |
| C1-R12 | New outputs and refusals name documents as `[[key]]` or a file name. | Spec G6. | none. |
| C1-R13 | A ledger whose own name is not a key still takes entries; the response carries `key_note`. | The entry is real and its bare token resolves locally; the rename belongs to the cutover. | none. |

## Open for the operator

1. **Sidecar creation (Task 8).** Recommendation: create the sidecar and say so in the output. Alternative described in Task 8.
2. **Params append from a worktree (C1-R9).** The plan refuses it, which changes what worktree sessions can do today. Recommendation: refuse, for the same reason the prose path already does.

## Not built here, on purpose

- Events, observations, links, `event_edges`, `sources`, `commit_refresh`, and ULIDs in committed text: Plan C2.
- The content-keyed vector cache and the persistent derived-file location: Plan C3.
- The git-history cache and `doc(find)` ordering by a catalog clock (`catalog/find.rs:28`): later; the review recommends Plan E.
- Indexing sidecar-declared augmentation shapes into the derived catalog (B2 plan, "Not built here"): the plan that builds a persistent derived file.
- `codescout-37`'s entry-file format and the L-B tools: C1 writes today's prose-section format and defines no entry-file format.
- `update_entry` regenerating a section's prose from changed fields; params field values that live only in the catalog: Plan D's export and `codescout-37`'s format.
- `cites` given as `key:TOKEN` or `[[…]]`, and `<slug>:<local>` cites of prose-ledger entries (`resolve_cite_ref` checks params rows only): not widened here.
- The machine-state file, `entry_reservation` removal, the entry-id file lock, and the deletion tail: Plan E.

## After the plan

- From the main checkout, after the branch lands: rewrite the "add the section in the same session" sentence in the prompts of the 16 ledgers that declare an `entry_collection` (`docs/augmentations/*.yaml`) through `doc(action="augment", merge=true, augment={prompt: …})`, so `write_through` republishes each sidecar. Until then those agents meet Task 4's refusal, whose hint names the fix.
- Record the rulings and any `derive-shadow` numbers taken after a dual-write in the session log (id `abff63149e3c24e8`).
- Plan D reuses `dual_write::{write_entry_into_ledger, commit_after_file_write, find_unrecorded_section, record_written_file}` and `cite_text::grammar_form_of` to export the 38 params-only entries and rewrite citations.
