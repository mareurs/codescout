---
id: '67870a11f0ed7a0e'
kind: plan
status: draft
title: Tracker Lifecycle Tools, Part 1 Implementation Plan
---

# Tracker Lifecycle Tools, Part 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire the lifecycle write core into `doc(create)`, `doc(update)` and `doc(get)`, with a single-collection loader, and close the write core's gaps I-1, I-2 and I-3.

**Architecture:** Library first (`src/librarian/lifecycle/`: a revisit rule, a combined `update_entry`, a schema field-name guard, a loader and a readme renderer). Then one shared write helper extracted from `create::call`, and three tool branches in new modules that call the library: entry create, collection create, entry update. `doc(get)` gains a table branch.

**Tech Stack:** Rust, `serde_json`, `chrono::NaiveDate`, `git2` (test fixtures), the existing `frontmatter`, `lifecycle::{layout, schema, entry, tree, table, write}` modules, `read_door`.

**Spec:** `docs/superpowers/specs/2026-11-14-lifecycle-tools-part-1-design.md` (read it first: calls 1-9 and § Review focus bind this plan). Parent: `docs/superpowers/specs/2026-11-11-tracker-lifecycle-design.md` (§ Tools, D1, D4). Write core: `docs/superpowers/specs/2026-11-14-lifecycle-write-core-design.md`.

## Global Constraints

- **Net-neutral advertised surface.** `TOOL_SURFACE_CHAR_BUDGET` stays `57_664` in `src/server.rs`. The only new argument is `collection`. The trim and the schema probes are done in the same task that adds `collection` (Task 7), so no commit leaves the suite red. Nothing raises the budget (`src/prompts/README.md` § Caps follow the harness).
- No new argument, output key or field names a hex id. An entry file and a collection readme carry **no `id:`**.
- `today` is a parameter of every library function. A tool passes `chrono::Utc::now().date_naive()` (UTC, as `create` does for a bug's `opened`).
- `update` routes by `layout::place(rel_path)` giving `Role::Entry`, never by the frontmatter `kind`.
- `RevisitPast` means strictly before `today` (the doctor fires `RevisitDue` only when `revisit_by < today`, `checks.rs:305`).
- **One write path.** Entry create and collection create go through one helper extracted from `create::call` (Task 6), so every guard `create` has applies to them: `base_dir` resolution, `temp_write_guard`, `normalize_rel_path` and `validate_rel_path`, `refuse_catalogued_path_outside_repo`, `reject_reserved_extra_keys`, `reject_body_leading_frontmatter`, `requirements::enforce_on_create`, `protected_files::check_write`, `ensure_registration`. Entry and collection create obey `.codescout/requirements.toml` like every other create.
- **Exclusive create is safe for the first writer.** The helper's exclusive mode inserts the catalog row with a plain `INSERT` (no `ON CONFLICT`), refuses when the row or the file exists, and on `ErrorKind::AlreadyExists` after its own insert deletes only the row it inserted. It never calls `remove_file` in that mode. (Reason: the row id is `sha256(abs_path)`, `artifact::upsert` is `ON CONFLICT(id) DO UPDATE`, and `undo_failed_create` removes the file: a losing racer would otherwise delete the winner's row, events and file.)
- `entry_kind` is fixed; `parse_entry` failures and `KindUndeclared` are not repairable by an update, and the refusal names both.
- Run `rustfmt --edition 2021 <file>` on every `.rs` file a task commits (the pre-commit hook runs `rustfmt --check` on all committed `.rs` bytes), or `./scripts/fmt-mine.sh`. A commit message cites no 16-hex artifact id.
- Tests that need a git top level run `git init` in the temp project (the existing create tests use a plain `TempDir`). Tests that need a worktree use the shared fixture added in Task 6.
- Commit by path: `git add <paths>` then `git commit -m ... -- <paths>`; never `git add -A` on the shared checkout. No push.
- `./scripts/gate.sh` runs only in Task 11, on an untouched tree, by the controller.

## Review Focus

Inputs the spec implies and the task tests pin, most likely first:

1. Two creates of the same-day, same-title entry: the second is refused and the first file, its row and its `created_at` and `file_sha256` are unchanged, including when the key check is bypassed and the race reaches the exclusive create (Task 6, Task 7).
2. An entry key equal to a `docs/issues/` file stem is refused at create time (Task 7).
3. An update with a stray key and a bad status in one call is repaired; a body-only update of an entry applies; the catalog row follows the file (Tasks 2, 9).
4. A collection create with a missing `exit_when`, an unknown status class, a nested path, an `archive` path, or an existing non-empty folder is refused and writes nothing (Task 8).
5. A worktree session reads the schema from the worktree's checkout for create, update and the table (Tasks 7, 9, 10).

## Rulings (controller; the operator delegated them on 2026-11-14 and may overrule any)

- **P-R1** (write-core M-7) Task 9 refuses a status write that reopens an entry whose path has an `archive` segment, and names the move. Cost if wrong: one refusal branch.
- **P-R2** The key check compares `identity::key::key_of` over the tracked and untracked Markdown paths plus the candidate path. It reads no file. A new `pub(crate) fn list_checkout_paths(root: &Path) -> anyhow::Result<Vec<String>>` in `lifecycle/scan.rs` runs `require_top_level` first and then `list_paths(root, &git)` (`list_paths` lists `--cached --others --exclude-standard`, so untracked files count; it does not itself check the top level, and from a subproject root it would list only that subtree). Where `require_top_level` fails, entry create refuses with that error. Cost if wrong: a subproject root cannot create entries until a cheaper index exists. Reason: the first draft read about 4600 files per create. Keys are repository-wide: an archived collection `k` blocks a new live collection `k`.
- **P-R3** An empty `UpdateSpec` is `WriteProblem::EmptyUpdate`. The tool calls `update_entry` only when the patch carries `status`, `title`, `tags` or `extra`.
- **P-R4** `revisit_by` without `status` on an entry re-dates it, keeps `status` and `status_since`, and is allowed only when the current status class needs a revisit date (else `RevisitNotNeeded`); a past date is `RevisitPast`. Cost if wrong: one branch in `update_entry`.
- **P-R5** The collection table gains an `Archived` column and `doc(get)` passes `include_archived = true`. This changes a pinned table format; the existing table tests are updated. Cost if wrong: one column and its tests.
- **P-R6** `doc(get)` on a readme returns `table` always, and `entries` and `entry_total` only when `entry_filter` is given (today's shape; `entry_total` counts the rows considered).
- **P-R7** Update and get load a collection by its **folder** (`load_collection_dir`), so an archived collection works; only create looks a collection up by key. A readme is matched without regard to case (`is_folder_index`).
- **P-R8** A collection created by `doc(create)` gets catalog status `active`. An entry's catalog status is its own status word.
- **P-R9** Declared field names are refused when they equal a table column, an `entry::RESERVED` name or a `frontmatter::RESERVED_KEYS` name (the union), so no schema declares a field no write can set.

### Task 1: The revisit rule and the wording gaps (I-2, M-4, M-5)

**Files:**
- Modify: `src/librarian/lifecycle/write.rs` (`WriteProblem`, `new_entry`, `change_status`)
- Test: `src/librarian/lifecycle/write/tests.rs`

**Interfaces:**
- Consumes: existing `new_entry`, `change_status`, `NewEntry`.
- Produces: `WriteProblem::RevisitPast(NaiveDate)`. The `Display` of every variant has the shape `<what is wrong>; <remedy>` (a `; ` separator, then an imperative clause).

- [ ] **Step 1: Write the failing tests** in `write/tests.rs`, section `// ---- revisit rule`:
  - `new_entry_refuses_a_past_revisit_by`: status `blocked`, `revisit_by` = `today - 1 day` gives `Err` containing `RevisitPast(that date)`; `revisit_by = today` is `Ok`.
  - `change_status_refuses_a_past_revisit_by_and_a_stored_past_date`: (a) call date before today → `RevisitPast`; (b) blocked target, no call date, stored date before today → `RevisitPast(stored)`; (c) open target with a stored past date → `Ok` and the date is dropped.
  - `a_blank_body_is_the_template`: `NewEntry.body = Some("  \n")` with a template renders the template, same as `None`.
  - `every_problem_text_names_its_remedy`: build each variant by hand and assert its `Display` splits on `; ` into exactly two non-empty parts, and the second part begins with one of `pass`, `use`, `remove`, `declare`, `fix`, `give`, `name`.
- [ ] **Step 2: Run** `scripts/with-slot.sh cargo test --lib lifecycle::write`. Expected: the new tests FAIL.
- [ ] **Step 3: Implement** `RevisitPast` in `WriteProblem`; check it in `new_entry` and `change_status` per the rule above; treat a whitespace-only `NewEntry.body` as `None`; rewrite the `#[error(...)]` text of every variant to `<problem>; <remedy>`. Fix any existing assertion that pins an old tail.
- [ ] **Step 4: Run** the same command. Expected: all `lifecycle::write` tests PASS.
- [ ] **Step 5: Commit** `src/librarian/lifecycle/write.rs src/librarian/lifecycle/write/tests.rs` with `feat(lifecycle): a past revisit_by is refused; problem texts name the remedy; a blank body is the template`.

### Task 2: The combined update (I-1, I-3, M-6, P-R3, P-R4)

**Files:**
- Modify: `src/librarian/lifecycle/write.rs`
- Test: `src/librarian/lifecycle/write/tests.rs`

**Interfaces:**
- Consumes: Task 1's `RevisitPast`.
- Produces:
  ```rust
  pub struct UpdateSpec<'a> {
      pub status: Option<&'a str>,
      pub revisit_by: Option<NaiveDate>,
      pub title: Option<&'a str>,
      pub tags: Option<Vec<String>>,
      pub valid: Option<Option<&'a str>>,          // Some(None) removes `valid`
      pub fields: BTreeMap<String, Option<Value>>, // as `change_fields` edits
  }
  pub fn update_entry(doc: &str, schema: &CollectionSchema, spec: &UpdateSpec<'_>, today: NaiveDate)
      -> Result<String, Vec<WriteProblem>>;
  ```
  plus `WriteProblem::EmptyUpdate`. Private non-validating steps `apply_status(fm, schema, new_status, revisit_by, today)` and `apply_fields(fm, schema, edits)` that mutate a `Frontmatter` and return `Result<(), Vec<WriteProblem>>`; `change_status` and `change_fields` call them and keep their one re-read (Call A and Call B of the write-core spec still hold). `revisit_by` with no `status` follows P-R4.

- [ ] **Step 1: Write the failing tests:**
  - `one_update_repairs_a_stray_key_and_a_bad_status` (I-1): status `wip` (undeclared) plus undeclared key `bogus`; `update_entry` with `status: Some("open")` and `fields: {bogus: None}` is `Ok` and `validate_entry` on the result is empty. A twin asserts `change_status` then `change_fields` on the same document still fails at the first call (Call B).
  - `update_entry_sets_title_tags_and_valid_together`: tags `[" a ", "", "b", "a"]` store as `[a, b]`; `valid: Some(Some(<declared class>))` sets; `Some(None)` removes.
  - `a_title_with_a_line_break_is_refused` (`Title`); `an_empty_spec_is_refused` (`EmptyUpdate`).
  - `entry_kind_cannot_change`: `fields: {entry_kind: Some(..)}` → refused, and the text says the kind is fixed.
  - `a_document_that_does_not_parse_as_an_entry_is_not_repaired`: missing `created` → `Err(Invalid)` naming it.
  - `an_entry_with_closed_missing_is_repaired_by_a_terminal_status_write`.
  - `title_only_update_ignores_a_past_revisit_by`.
  - `an_overdue_blocked_entry_takes_a_new_date_with_a_status_write` (`status_since = today`).
  - `revisit_by_alone_redates_and_keeps_status_since` (P-R4); `revisit_by_alone_on_an_open_entry_is_refused` (`RevisitNotNeeded`); `revisit_by_alone_in_the_past_is_refused`.
- [ ] **Step 2: Run** `scripts/with-slot.sh cargo test --lib lifecycle::write`. Expected: the new tests FAIL.
- [ ] **Step 3: Implement** `UpdateSpec`, `update_entry`, `EmptyUpdate`, and extract the two private steps. `update_entry` applies status, fields, title, tags and `valid` to one `Frontmatter` and calls `reread` once on the final text. Reuse the `Title` check. Render with `rewrite_frontmatter_normalizing` as the existing functions do.
- [ ] **Step 4: Run** the same command. Expected: PASS, including every pre-existing test.
- [ ] **Step 5: Commit** both files with `feat(lifecycle): update_entry — one validation on the final text; title, tags and valid are settable`.

### Task 3: Declared fields cannot take a reserved name (P-R9)

**Files:**
- Modify: `src/librarian/lifecycle/schema.rs`; `src/librarian/lifecycle/table.rs` (test only)
- Test: the `tests` modules of both

**Interfaces:**
- Produces: `pub const TABLE_COLUMNS: &[&str]` in `schema.rs` = the serialized keys of `table::EntryRow` (`key`, `entry_kind`, `title`, `status`, `class`, `created`, `closed`, `revisit_by`, `archived`, `fields`); `SchemaError::FieldNameReserved { name: String }`. The refusal covers the union of `TABLE_COLUMNS`, `entry::RESERVED` and `frontmatter::RESERVED_KEYS`.

- [ ] **Step 1: Write the failing tests:** `a_declared_field_named_like_a_reserved_name_is_refused` (one case each from the three sets: `archived`, `created`, `owners`); in `table.rs`, `table_columns_match_the_serialised_row` (serialise an `EntryRow`; its key set equals `TABLE_COLUMNS`).
- [ ] **Step 2: Run** `scripts/with-slot.sh cargo test --lib -- lifecycle::schema lifecycle::table`. Expected: both FAIL.
- [ ] **Step 3: Implement** the constant and the refusal; rename any colliding field in existing test schemas.
- [ ] **Step 4: Run** the same command. Expected: PASS.
- [ ] **Step 5: Commit** `schema.rs table.rs` with `feat(lifecycle): a declared field may not take a reserved name`.

### Task 4: The loader

**Files:**
- Create: `src/librarian/lifecycle/collection.rs` (register `pub mod collection;` in `mod.rs`), tests in `src/librarian/lifecycle/collection/tests.rs`
- Modify: `src/librarian/lifecycle/tree.rs` (`impl Display for TreeProblem`)
- Reference (read only): `scan.rs` `load_docs_with` (the read-door pattern), `key.rs` `is_folder_index`.

**Interfaces:**
- Consumes: `layout::{Lifecycle, TRACKERS_DIR, place, Role}`, `tree::{LifecycleTree, CollectionNode, TreeProblem}`, `write::entry_template`, `identity::key::Key`, `read_door::for_project`.
- Produces:
  ```rust
  pub struct LoadedCollection {
      pub tree: LifecycleTree,   // built from this one folder
      pub key: Key, pub lifecycle: Lifecycle,
      pub dir: String,           // repo-relative, no trailing slash
      pub template: Option<String>,
  }
  impl LoadedCollection { pub fn node(&self) -> &CollectionNode; pub fn schema(&self) -> &CollectionSchema; }
  pub enum CollectionLoadError {
      BadKey(String), NotFound(String), Ambiguous { key: String, dirs: Vec<String> },
      NotACollectionDir(String), Unreadable(String), Broken(Vec<String>),
  }
  pub fn load_collection_dir(root: &Path, dir: &str) -> Result<LoadedCollection, CollectionLoadError>;
  pub fn load_collection(root: &Path, key: &str) -> Result<LoadedCollection, CollectionLoadError>; // live folders only
  ```
  `CollectionLoadError` derives `Debug, thiserror::Error`; each `Display` is `<problem>; <remedy>`. `Broken` carries the `Display` of each `TreeProblem`. `load_collection_dir` accepts a folder that `layout::place` of its readme path calls `CollectionReadme` (an archived collection included).

- [ ] **Step 1: Write the failing tests** (temp dir trees; `testkit` helpers where they fit):
  - `finds_a_collection_in_each_lifecycle_folder`; `finds_an_archived_collection_by_its_dir`.
  - `lists_live_and_archived_entries_and_the_template`: two entries plus `archive/old.md`; `node().entries.len() == 3` with exactly one entry `archived: true` (the tree keeps archived entries in the node; `table::rows` filters them) and `template` equals the readme's `## Entry template` text.
  - `a_readme_in_upper_case_is_found`.
  - `an_unknown_key_is_not_found`; `a_bad_key_is_refused_before_any_read` (`../x`, empty).
  - `a_key_in_two_lifecycle_folders_is_ambiguous`, dirs sorted.
  - `a_broken_schema_is_broken_with_a_readable_problem` (the string names the schema problem).
  - `a_sibling_collection_with_an_unreadable_file_does_not_affect_the_result` (chmod 000 on a sibling's entry; the loaded collection is `Ok`).
  - `a_readme_that_links_out_of_the_root_is_unreadable` (variant only).
  - `a_dir_that_is_not_a_collection_is_refused` (`NotACollectionDir`).
  - In `tree.rs` tests: `every_tree_problem_displays_with_its_path`.
- [ ] **Step 2: Run** `scripts/with-slot.sh cargo test --lib -- lifecycle::collection lifecycle::tree`. Expected: FAIL to compile.
- [ ] **Step 3: Implement.** `load_collection` validates the key, probes `<root>/docs/trackers/{ledgers,streams,temp}/<key>/` for a folder index (case-insensitive), more than one hit is `Ambiguous`, and calls `load_collection_dir`. `load_collection_dir` reads every `.md` under the folder (archive included) through the read door into `DocInput`s with repo-relative `/` paths, builds the tree, maps a schema problem to `Broken`, and takes the template with `write::entry_template`.
- [ ] **Step 4: Run** the same command. Expected: PASS.
- [ ] **Step 5: Commit** `collection.rs collection/tests.rs mod.rs tree.rs` with `feat(lifecycle): load_collection and load_collection_dir read one collection`.

### Task 5: The collection readme renderer

**Files:**
- Modify: `src/librarian/lifecycle/collection.rs`
- Test: `src/librarian/lifecycle/collection/tests.rs`

**Interfaces:**
- Consumes: `schema::parse_schema`, `frontmatter::write`.
- Produces:
  ```rust
  pub fn collection_readme(lifecycle: Lifecycle, title: &str, extra: &BTreeMap<String, Value>, body: &str)
      -> Result<String, Vec<SchemaError>>;
  ```
  The text starts with a frontmatter block holding `kind: collection`, `title` and the schema keys from `extra` (the nine keys `parse_schema` reads: `entry_kinds`, `statuses`, `initial_status`, `key_style`, `fields`, `owner`, `exit_when`, `writer`, `review_every`), then `body`. No `id:`. The text is re-read with `parse_schema(lifecycle, &fm)` and its errors are returned. An empty `title` is `MissingTitle`. An unknown key in `extra` is `Malformed` with the key in `detail` (`Malformed.key` is `&'static str`).

- [ ] **Step 1: Write the failing tests:** `a_full_ledger_readme_renders_and_parses`; `a_stream_without_exit_when_is_refused` (`ExitRequired`); `a_ledger_with_exit_when_is_refused` (`ExitRefused`); `an_unknown_status_class_is_refused` (`UnknownClass` names the word); `the_readme_carries_kind_collection_and_no_id`; `an_unknown_key_in_extra_is_refused_naming_the_key`; `the_rendered_readme_loads_through_load_collection_dir` (write it to a temp folder and load it).
- [ ] **Step 2: Run** `scripts/with-slot.sh cargo test --lib -- lifecycle::collection`. Expected: new tests FAIL.
- [ ] **Step 3: Implement** `collection_readme`. Export the nine key names as constants from `schema.rs` if they are not, and use them in both places.
- [ ] **Step 4: Run** the same command. Expected: PASS.
- [ ] **Step 5: Commit** with `feat(lifecycle): collection_readme renders a schema-checked readme`.

### Task 6: The shared write helper, the exclusive mode and the worktree fixture

**Files:**
- Modify: `src/librarian/tools/create.rs` (extract `resolve_base_dir` and the helper; `Args` and `preflight`), `src/librarian/catalog/artifact.rs` (`insert_new`), `src/librarian/tools/doctor.rs` (move `repo_with_registered_worktree` AND its dependency `git_fixture_with_commit` to the shared test module; the other doctor tests keep using them)
- Create: shared test fixture module `src/librarian/tools/testfixture.rs` (`#[cfg(test)]`, register in `tools/mod.rs`)
- Test: `src/librarian/tools/create.rs` tests, `src/librarian/catalog/artifact.rs` tests

**Interfaces:**
- Produces:
  ```rust
  // create.rs
  pub(super) fn resolve_base_dir(ctx: &ToolContext, repo: Option<&str>) -> Result<PathBuf>; // today's base_dir logic, extracted unchanged
  pub(super) enum WriteMode { Plain, Exclusive }
  pub(super) enum Content { Rendered(String), Compose { body: String } }
  // Rendered: the file bytes are given and carry no `id:` (entry, collection).
  // Compose: write_new builds the frontmatter from the fields below plus `id: Some(id)` and the body (plain create, as today).
  pub(super) struct NewArtifact<'a> {
      pub rel_path: String, pub content: Content,
      pub kind: String, pub status: String, pub title: String,
      pub tags: Vec<String>, pub owners: Vec<String>,
      pub topic: Option<String>, pub time_scope: Option<String>,
      pub extra: &'a BTreeMap<String, Value>, pub body_for_checks: &'a str,
      pub augment: Option<&'a AugmentSpec>,   // written in the same transaction as the row
  }
  pub(super) async fn write_new(ctx: &ToolContext, repo: Option<&str>, art: NewArtifact<'_>, mode: WriteMode) -> Result<Value>;
  // the race-critical step, callable without any pre-check:
  pub(super) fn insert_and_create_exclusive(cat: &Catalog, row: &ArtifactRow, full: &Path, content: &str) -> Result<(), anyhow::Error>;
  // artifact.rs
  pub enum InsertError { Exists, Other(anyhow::Error) }
  pub fn insert_new(cat: &Catalog, row: &ArtifactRow) -> Result<(), InsertError>; // plain INSERT; Exists when the id or the UNIQUE abs_path exists
  // testfixture.rs
  pub(crate) fn repo_with_worktree() -> (TempDir, PathBuf /*main*/, PathBuf /*worktree*/);
  ```
  `Plain` keeps today's order, slug minting (`upsert_and_mint_slug`) and `undo_failed_create`. `Exclusive`: `insert_new` and, in the same transaction, `ensure_slug` for the row (so the entry reaches the entry graph), then `OpenOptions::create_new`; an `InsertError::Exists` becomes the message `path exists: <rel_path>`; on `AlreadyExists` from `create_new` it deletes the row this call inserted and never calls `remove_file`; any other write error also deletes only the row. `create::call` builds a `NewArtifact` with `Content::Compose` and calls `write_new` in `Plain` mode; its behaviour does not change (augmentation atomicity included: `assert_failed_catalog_finalization_is_atomic` stays green). `Args` fields `rel_path`, `kind`, `title` and `body` become optional in the serde struct and gain `collection: Option<String>`; `preflight` still requires `rel_path`, `kind`, `title` and `body` when `collection` is absent (the test `doc_malformed_calls_get_their_own_error_not_the_worktree_guard` keeps passing).

- [ ] **Step 1: Write the failing tests:**
  - `insert_new_refuses_an_existing_id_and_leaves_the_row` (artifact.rs): after a second `insert_new` with the same id fails with `Exists`, the first row's `created_at` and `file_sha256` are unchanged; a second row with a different id but the same `abs_path` also gives `Exists`.
  - `insert_and_create_exclusive_on_a_planted_file_leaves_the_file_and_deletes_only_its_row` (create.rs): plant a file at `full` with no row, call `insert_and_create_exclusive` directly (no pre-check), assert the error kind is `AlreadyExists`, the planted file's bytes are unchanged, and no row remains.
  - `insert_and_create_exclusive_on_an_existing_row_leaves_file_and_row_untouched`: a file and its row exist; the call fails; bytes, `created_at` and `file_sha256` are unchanged.
  - `exclusive_write_mints_a_slug`: after a successful `Exclusive` write, the row has a slug.
  - `plain_mode_is_todays_behaviour`: the existing `create` tests are the proof (no new test).
  - `a_call_with_collection_and_no_rel_path_passes_preflight` and `a_call_with_neither_still_fails_preflight`.
- [ ] **Step 2: Run** `scripts/with-slot.sh cargo test --lib -- tools::create catalog::artifact`. Expected: the new tests FAIL.
- [ ] **Step 3: Implement** the extraction as a pure refactor first (run the existing create tests green), then add `Exclusive`, `insert_new`, the optional `Args` fields and the fixture move. Make `undo_failed_create` `pub(super)` only if the helper lives in another module.
- [ ] **Step 4: Run** `scripts/with-slot.sh cargo test --lib -- tools::create catalog::artifact tools::doctor`. Expected: PASS, every pre-existing test unchanged.
- [ ] **Step 5: Commit** with `refactor(librarian): create writes through one helper with an exclusive mode that cannot delete the first writer's file`.

### Task 7: `doc(create)` entry mode

**Files:**
- Create: `src/librarian/tools/create_lifecycle.rs` (register in `tools/mod.rs`), tests in `src/librarian/tools/create_lifecycle_tests.rs`
- Modify: `src/librarian/tools/create.rs` (call the new module when `collection` is set), `src/librarian/lifecycle/scan.rs` (add `pub(crate) fn list_checkout_paths`, see P-R2), `src/librarian/tools/artifact.rs` (advertise `collection`, trim to stay net-neutral)

**Interfaces:**
- Consumes: `collection::load_collection`, `write::{new_entry, NewEntry, EntryFile}`, `scan::list_checkout_paths`, `identity::key::key_of`, Task 6's `write_new` (`Exclusive`).
- Produces: `pub(super) async fn entry_mode(ctx: &ToolContext, a: &Args) -> Result<Value>` returning `{"id", "abs_path", "key"}`. A call with `collection` and `rel_path` is refused, and a call with `collection` and `kind: "collection"` is refused (entry mode only; the entry kind goes in `kind`).
- Mapping: `kind` = entry kind; `extra` = declared fields with `revisit_by` (ISO string) and `valid` (string) taken out by name (both pass `reject_reserved_extra_keys`, which checks only `frontmatter::RESERVED_KEYS`); `status`, `tags`, `title`, `body` as they are; `owners`, `topic`, `time_scope`, `augment` are refused in entry mode.

- [ ] **Step 1: Write the failing tests** (`git init` temp project):
  - `creates_an_entry_from_the_schema_and_template`: file at `docs/trackers/ledgers/<key>/<stem>.md`, status `initial_status`, body the template, no `id:`, `kind: entry`, result has `key`, `id`, `abs_path`, the row exists with catalog kind `entry` and has a slug.
  - `rel_path_and_collection_together_are_refused`.
  - `a_blocked_entry_takes_revisit_by_from_extra`; `a_past_revisit_by_is_refused`.
  - `a_second_create_of_the_same_title_on_the_same_day_is_refused_and_leaves_the_first` (assert file bytes, row `created_at` and `file_sha256` unchanged).
  - `a_key_equal_to_a_docs_issues_stem_is_refused`; `a_key_equal_to_another_collections_readme_folder_is_refused`.
  - `a_failed_file_write_undoes_the_row` (read-only parent).
  - `owners_topic_time_scope_and_augment_are_refused_in_entry_mode`.
  - `an_unknown_collection_names_the_remedy`.
  - `a_requirements_toml_rule_is_enforced_on_entry_create` (reuse the `enforce_on_create` test setup).
  - `creates_in_a_worktree_session_from_the_worktrees_schema` (`repo_with_worktree`; the readme exists only in the worktree).
  - `the_advertised_surface_is_net_neutral`: `tool_surface_under_budget` and the schema gates listed in Step 4 stay green (run them here).
- [ ] **Step 2: Run** `scripts/with-slot.sh cargo test --lib -- tools::create_lifecycle`. Expected: FAIL to compile.
- [ ] **Step 3: Implement** `entry_mode`: load the collection; `new_entry`; key check (P-R2: `key_of` over `list_paths` plus the candidate; a path equal to the candidate is "path exists", another path with the same key is a collision naming both); then `write_new(..., Exclusive)`. Add `collection` to the advertised schema with the shortest accurate description and trim the same number of characters from the `doc` description (`append_entry` and `rekey_prefix` prose first); do not raise the budget.
- [ ] **Step 4: Run** the same command, then `scripts/with-slot.sh cargo test --lib -- tool_surface_under_budget every_action_labelled_schema_key_is_honored_by_that_action every_schema_object_promising_to_refuse_unknown_keys_actually_refuses input_schema_has_no_phantom_update_fields every_tool_description_is_under_its_cap`. Expected: PASS.
- [ ] **Step 5: Commit** the new files, `create.rs`, `lifecycle/scan.rs`, `artifact.rs`, `tools/mod.rs` with `feat(librarian): doc(create) makes an entry in a lifecycle collection`.

### Task 8: `doc(create)` collection mode

**Files:**
- Modify: `src/librarian/tools/create_lifecycle.rs`, `src/librarian/tools/create.rs` (route `kind == "collection"`)
- Test: `src/librarian/tools/create_lifecycle_tests.rs`

**Interfaces:**
- Consumes: `collection::{collection_readme, load_collection_dir}`, `layout::place`, `write_new` (`Exclusive`).
- Produces: `pub(super) async fn collection_mode(ctx: &ToolContext, a: &Args) -> Result<Value>`: `kind: "collection"`, `rel_path` exactly `docs/trackers/<ledgers|streams|temp>/<key>/readme.md`, `title`, schema keys in `extra`, `body` the readme prose. Catalog status `active` (P-R8). Result `{id, abs_path, key}`.

- [ ] **Step 1: Write the failing tests** (Review Focus 4): `creates_a_ledger_collection_that_load_collection_reads`; `a_stream_without_exit_when_is_refused_and_writes_nothing`; `an_unknown_status_class_is_refused`; `a_nested_path_is_refused`; `an_archive_path_is_refused`; `a_folder_that_already_holds_files_is_refused`; `a_path_outside_the_lifecycle_folders_is_refused`; `a_readme_key_equal_to_an_existing_stem_is_refused`; `the_readme_has_kind_collection_and_no_id`. Each refusal asserts the folder does not exist afterwards.
- [ ] **Step 2: Run** `scripts/with-slot.sh cargo test --lib -- tools::create_lifecycle`. Expected: new tests FAIL.
- [ ] **Step 3: Implement** `collection_mode`: validate the path with `layout::place` (role `CollectionReadme`, not archived) and that the folder is absent or empty; render with `collection_readme`; same key check as Task 7; `write_new(..., Exclusive)`.
- [ ] **Step 4: Run** the same command. Expected: PASS.
- [ ] **Step 5: Commit** with `feat(librarian): doc(create) makes a lifecycle collection`.

### Task 9: `doc(update)` on an entry

**Files:**
- Create: `src/librarian/tools/lifecycle_update.rs` (register in `tools/mod.rs`), tests in `src/librarian/tools/lifecycle_update_tests.rs`
- Modify: `src/librarian/tools/update.rs` (`UpdatePatch` and its fields become `pub(super)`; one call; the `updated_row` overrides)

**Interfaces:**
- Consumes: `layout::place`, `collection::load_collection_dir`, `write::{update_entry, UpdateSpec}`.
- Produces:
  ```rust
  pub(super) struct EntryUpdate { pub text: String, pub status: String, pub title: String, pub tags: Vec<String> }
  pub(super) fn entry_update(root: &Path, rel_path: &str, original: &str, patch: &UpdatePatch, today: NaiveDate)
      -> Result<Option<EntryUpdate>>;
  ```
  `Ok(None)` when the path does not place as `Role::Entry` (the generic path runs unchanged). `Ok(Some(..))` when it does: `text` is the document after the frontmatter change, `status`, `title` and `tags` are read back from `text`.
- Rules: first refuse `owners`, `topic`, `time_scope` and `params` (below); then, when the patch carries none of `status`, `title`, `tags`, `extra`, return `Ok(None)` (a patch with only `body`, `body_edits` or `commit_refresh` takes the generic path, which is already correct and does not parse the entry); otherwise call `update_entry`. `extra`: `revisit_by` (ISO string) and `valid` (string or null) by name, every other key a declared field (null deletes). `owners`, `topic`, `time_scope`, `params` are refused with a message that says an entry has none. A status write that reopens an entry whose `rel_path` has an `archive` segment is refused with the move named (P-R1).
- Where: in `update::call`, after `original` is read and before the bug `closed` stamp (a file with `kind: bug` that places as an entry must not get `extra.closed`). Call `reject_reserved_extra_keys` again at the new seam before the mapping (it is idempotent; today it runs a little later in `call`). The root is `cp.git_root` when it contains `full`, else the managed root that contains it (`managed_roots(ctx)`); `rel_path` is `full` with that root stripped. The generic code then uses `text` as `original`, clears `patch.status`, `title`, `tags` and `extra` for the file-writing branches only, and builds `updated_row` with the returned `status`, `title` and `tags` as overrides (the row is built from the patch at `update.rs` ~L870-877). `update::call` emits no `status_change` event (only a body `field_patch`), so none is added.

- [ ] **Step 1: Write the failing tests:**
  - `a_status_update_sets_status_since_and_closed_through_the_tool`.
  - `the_catalog_row_follows_the_file_after_an_entry_update`: after a status, title and tags update, `find` by the new status returns the entry and the row's `title` and `tags` equal the file's (C2).
  - `a_body_only_edit_of_an_entry_applies` and `a_commit_refresh_only_update_of_an_entry_does_not_hit_empty_update` (C3).
  - `an_undeclared_status_is_refused_for_an_entry_and_not_for_a_tracker`.
  - `an_entry_without_kind_entry_still_takes_the_entry_path`; `an_entry_with_kind_bug_gets_no_closed_stamp`.
  - `one_call_repairs_a_stray_key_and_a_bad_status`.
  - `owners_topic_time_scope_and_params_are_refused_on_an_entry`.
  - `a_body_edit_and_a_status_change_in_one_call_both_apply`.
  - `reopening_an_archived_entry_is_refused_and_names_the_move`; `an_entry_in_an_archived_collection_is_updated_through_its_dir`.
  - `revisit_by_through_extra_redates_a_blocked_entry`.
  - `a_non_entry_document_takes_todays_path_unchanged`.
  - `an_update_in_a_worktree_reads_the_schema_from_the_worktree` (`repo_with_worktree`).
- [ ] **Step 2: Run** `scripts/with-slot.sh cargo test --lib -- tools::lifecycle_update`. Expected: FAIL to compile.
- [ ] **Step 3: Implement** `entry_update` and the call in `update.rs` as specified.
- [ ] **Step 4: Run** the same command and `scripts/with-slot.sh cargo test --lib -- tools::update`. Expected: PASS.
- [ ] **Step 5: Commit** with `feat(librarian): doc(update) on an entry validates through update_entry`.

### Task 10: `doc(get)` on a collection readme: the table

**Files:**
- Modify: `src/librarian/tools/get.rs`; `src/librarian/lifecycle/table.rs` (`Archived` column); the existing table tests
- Test: `src/librarian/tools/get_table_tests.rs`, `table.rs` tests

**Interfaces:**
- Consumes: `layout::place`, `collection::load_collection_dir`, `table::{rows, render_markdown}`, `filter::eval` (the evaluator `get` already uses; it takes flat JSON maps).
- Produces: when the row's path places as `Role::CollectionReadme` and the call has no `heading`/`headings` and no `full`, the response gains `table` (markdown). With `entry_filter` it also has `entries` (flat JSON rows: declared fields lifted to top-level keys, a row's own columns win on a name clash, which only an undeclared stray key can cause) and `entry_total` (rows considered). `rows` is called with `include_archived = true`. The loader root: in a worktree session with a main-checkout row, strip `main_root` to get `rel_path` and load from `cp.git_root` (P-R7, session's checkout).
- `render_markdown` gains an `Archived` column (`yes` or empty); the existing header and row tests are updated (P-R5).

- [ ] **Step 1: Write the failing tests:** `a_readme_id_returns_the_table`; `entry_filter_filters_on_status_and_on_a_declared_field`; `entries_are_absent_without_entry_filter` (P-R6); `a_collection_with_no_entries_gives_the_header_and_zero`; `an_archived_entry_shows_with_the_archived_column`; `a_heading_selector_still_returns_prose`; `an_archived_collections_readme_gives_a_table`; `the_table_is_built_from_the_sessions_checkout` (`repo_with_worktree`: an entry that exists only in the worktree is listed); `a_broken_collection_returns_readable_problems_not_a_table`; in `table.rs`, `the_archived_column_prints_yes_for_an_archived_row`.
- [ ] **Step 2: Run** `scripts/with-slot.sh cargo test --lib -- tools::get_table lifecycle::table`. Expected: FAIL.
- [ ] **Step 3: Implement** the branch before the existing "not augmented" entry-filter error (`get.rs` ~L561-574) and the column.
- [ ] **Step 4: Run** the same command and `scripts/with-slot.sh cargo test --lib -- tools::get`. Expected: PASS.
- [ ] **Step 5: Commit** with `feat(librarian): doc(get) on a collection readme renders the entry table`.

### Task 11: Gate and closing (controller)

- [ ] **Step 1:** `scripts/with-slot.sh cargo test --lib -- tool_surface_report_lengths --nocapture`. The total must be `<= 57_664`. If a later task moved it, trim, do not raise.
- [ ] **Step 2:** `./scripts/gate.sh` on the untouched tree. Expected: `GATE EXITS -> FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0`.
- [ ] **Step 3:** Set this plan to `done`, update the spec status line, the parked-items memory (what is built; open: doctor wiring plan, Plan E key flip), and the F-3 follow-up.
- [ ] **Step 4: Commit** those paths with `docs(lifecycle): tools part 1 merged — statuses, memory and F-3 follow-up`.

## Facts I did not verify (the implementer must check and report)

- The exact line of the seam in `update::call` and the row-override edit (Task 9), beyond the lines the review cited.
- That `filter::eval` accepts the lifted flat rows without change (Task 10).
- That `insert_new` can be a plain `INSERT` given the `abs_path UNIQUE` column and the id algorithm (Task 6).
- That `list_paths` includes untracked files: the review read `--cached --others --exclude-standard` at `scan.rs:107-116`; the implementer confirms with a test (Task 7).
- The `tools/mod.rs` registration style for a module that is not itself a tool.
