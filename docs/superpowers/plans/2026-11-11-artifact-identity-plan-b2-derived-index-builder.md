# Artifact Identity Plan B2 — The Derived Index Builder Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build, beside the old catalog and without changing it, a derived index of one checkout at one commit: documents by key, entries, edges from the closed grammar, and chunks, in a new SQLite file; prove it rebuilds identically from scratch and from a previous commit; and add a read-only shadow command that diffs it against the old catalog on this repo.

**Architecture:** Plan B2 of the derived-catalog plan B (B1 gates and name checks, B2 builder). The builder reads a **commit**, not the working tree: it walks the commit's tree through `git2` and reads blobs, so two clones at one commit give one answer (goal G5) and no door or untracked-file question arises. Its inputs are committed files only: `.codescout/librarian.toml` rules, the built-in default rules, and `.codescout/key-space.toml` (Plan B1 Task 1). Incremental indexing caches parse products by content hash and always re-resolves edges over the whole tree, because one document's change can change another's resolution. The old indexer, catalog tables, `link_scan` and every tool output are untouched.

**Tech Stack:** Rust 2021, `git2` 0.20, `rusqlite` 0.39 (`bundled`), `sha2`, `serde_json`, `globset`; Plan A's `identity::{key,grammar,index}`; `frontmatter`, `classify`, `catalog::chunk::build_chunks` (pure).

**Spec:** `docs/superpowers/specs/2026-11-11-repo-relative-artifact-key-design.md`. Read § The catalog is derived, § Index inputs, guard, machine state and entry ids, and § Left to the builder. Evidence for every seam below: `docs/research/2026-11-11-artifact-identity-catalog-scout.md` and its `-review`; the signatures come from the Plan B2 seam scout (cited as `file:line` in the tasks).

## Global Constraints

- The derived index lives in its own SQLite file, never in `catalog.db`; it is opened with `journal_mode=WAL`, `busy_timeout=5000`, `foreign_keys=ON`, and its schema version is `PRAGMA user_version`. Model: `open_db` in `src/tools/symbol/call_edges/cache.rs:115` for WAL and the busy timeout; it sets neither `user_version` nor `foreign_keys`, so this module adds both.
- Everything stored is deterministic: rows are written in sorted order, no wall-clock value, no random id. A chunk id is `sha256(rel_path "\n" chunk_ix "\n" content_hash)` in hex; the old `replace_chunks` random uuid is not used.
- Rulings built into this plan (each is recorded in the session log by Task 7):
  1. **Listing:** tracked `*.md` files of the commit's tree (a case-sensitive `.md` extension, as the old walk), symlinks (git mode 120000) skipped; paths are git-root-relative with `/`.
  2. **One frontmatter rule:** a file whose frontmatter fails to parse is indexed with no frontmatter, the whole text as body, title from the first H1, and `parse_note = "frontmatter_error"`. A file that is not valid UTF-8 is not indexed and is listed in `skipped` with reason `not_utf8`.
  3. **Key space:** a document outside the key space (Plan B1) is not indexed; it is listed in `skipped` with reason `outside_key_space`.
  4. **Rules and ignore:** only committed inputs. Machine-global `workspace.toml` rules and ignore patterns, and the gitignored `.codescout/project.toml` (`force_include`, `chunk_grain`), are not read. The chunk grain is fixed at `Chunk` (2048 characters, `indexer.rs:247`).
  5. **Deferred to Plan C:** the git-history cache (last modification, authors, status changes) and vectors. The tables exist from Plan C on; B2 builds neither.
  6. **Commit-only reads:** the repo name (`.codescout/repo.toml`), the rules (`.codescout/librarian.toml`) and the key-space file are read from the commit's blobs, never from the working tree and never through `read_door`. The checkout row records `name_source` (`committed` or `unconfirmed`) and `rules_source` (`committed+builtin`).
- The builder reads no file through `fs::*`; it reads blobs through `git2`. Anything under `src/librarian/**` that reads a path uses `read_door` (gate `no_librarian_production_read_bypasses_the_read_door`, `read_door.rs:1414`). `ReadDoor` is `pub(crate)`: no `pub fn` signature names it.
- No `std::env::set_var`; config reaches the builder as arguments (`tests/env_mutation_isolation.rs`).
- The old indexer, `Catalog`, `link_scan` and every tool output are unchanged. The shadow command opens the old catalog with `SQLITE_OPEN_READ_ONLY` and never through `Catalog::open` (which migrates).
- Before completing any task: `./scripts/gate.sh` green. Targeted: `scripts/with-slot.sh cargo test --lib <path>`.
- Commit with `scripts/commit-mine.sh -m "<message>"` after `git add <paths>`, following `docs/conventions/shared-checkout-commit-sequence.md`.

## Review Focus

1. A file with broken frontmatter, a file that is not UTF-8, and a symlink in the same commit — expected: the first is indexed with `parse_note`, the second and third are skipped with named reasons; none aborts the build. Tests in Tasks 1 and 3.
2. A rename plus a deletion plus a modification between two commits — expected: the incremental result dumps byte-identically to a rebuild at the new commit. Test in Task 4.
3. A new document that defines a key an old citation pointed at nowhere, or that collides with an existing key — expected: the edges of unchanged documents are re-resolved (an `UnknownKey` becomes `File`; a collision becomes `KeyCollision`). Test in Task 4.
4. Two clones at one commit, made on different days by different `git` identities — expected: identical dumps; nothing in a row depends on a clone's path, name or time. Test in Task 5.
5. The shadow command run when the old catalog file is absent or locked — expected: it reports `NOT COMPARED — <reason>` and exits 2; it never creates or migrates `catalog.db`. Test in Task 6.

---

### Task 1: Scan a commit

**Files:**
- Create: `src/librarian/derived/mod.rs` (`pub mod scan;` and later modules), `src/librarian/derived/scan.rs`
- Modify: `src/librarian/mod.rs` (`pub mod derived;` beside `pub mod identity;` at :26)
- Test: `src/librarian/derived/scan.rs`

**Interfaces:**
- Consumes: `git2`; `identity::key_space::KeySpace`.
- Produces:
  - `pub struct ScannedFile { pub rel_path: String, pub sha256: String, pub text: Result<String, Skip> }` and `pub enum Skip { NotUtf8, Symlink, OutsideKeySpace }` with `pub fn reason(&self) -> &'static str`.
  - `pub fn scan_commit(repo: &git2::Repository, commit: &git2::Commit, space: &KeySpace) -> anyhow::Result<Vec<ScannedFile>>` — walks the commit tree, keeps blobs whose name ends in `.md` (case-sensitive), returns them sorted by `rel_path`. `sha256` is over the raw bytes, lower-case hex (`{:x}`, as `indexer.rs:562-566`). A blob that is not UTF-8 gets `Err(NotUtf8)` (strict, no lossy decode); a mode-120000 entry gets `Err(Symlink)`; a path the key space excludes gets `Err(OutsideKeySpace)` and its text is not read.
  - `pub fn open_commit<'r>(repo: &'r git2::Repository, rev: &str) -> anyhow::Result<git2::Commit<'r>>` (`rev` is `HEAD` or a sha).
  - `pub fn blob_text(repo: &git2::Repository, commit: &git2::Commit, rel_path: &str) -> anyhow::Result<Option<String>>` — `None` when the path is absent from the commit's tree, `Err` when the blob is not UTF-8; the one route by which the builder reads `repo.toml`, `librarian.toml` and `key-space.toml`.

- [ ] **Step 1: Write the failing tests** with a tempdir repo built through `git2` (copy the shape of `gc.rs:1218-1244` `init_git_repo` and `commit_file`, with a fixed `git2::Time` so commit ids are reproducible): `lists_only_md_blobs_sorted_by_path`; `a_non_utf8_file_is_skipped_with_a_reason` (Review Focus 1); `a_symlink_entry_is_skipped`; `an_excluded_path_is_listed_but_its_text_is_not_read`; `sha256_matches_the_old_walks_hex_form`; `an_uppercase_md_extension_is_not_listed`; `an_empty_repo_with_no_commit_is_an_error_naming_head`; `blob_text_returns_none_for_an_absent_path_and_an_error_for_non_utf8`.
- [ ] **Step 2: Run** `scripts/with-slot.sh cargo test --lib derived::scan` → FAIL. **Step 3: Implement.** **Step 4: Run** → PASS.
- [ ] **Step 5: Commit** — `feat(librarian): derived index — scan one commit's markdown`.

---

### Task 2: The schema and a dump

**Files:**
- Create: `src/librarian/derived/schema.rs`
- Modify: `src/librarian/derived/mod.rs`

**Interfaces:**
- Consumes: `rusqlite`.
- Produces:
  - `pub const SCHEMA_VERSION: i64 = 1;`
  - `pub fn open(path: &Path) -> rusqlite::Result<Connection>` (creates the file and parent directories, sets the pragmas in Global Constraints, applies the schema when `user_version` is 0, refuses a newer `user_version`); `pub fn open_in_memory() -> rusqlite::Result<Connection>` for tests.
  - Tables (all `WITHOUT ROWID` where a natural key exists; no autoincrement ids): `checkout(root TEXT, repo_name TEXT, name_source TEXT, rules_source TEXT, commit_sha TEXT)` with one row; `document(rel_path PK, key, key_error, kind, status, title, owners, tags, topic, time_scope, confidence, sha256, owner_key, parse_note)` (`owners` and `tags` are sorted JSON arrays); `entry(rel_path, token, level, heading_line, end_line, PK(rel_path, token, heading_line))`; `edge(src_rel_path, ord, line, kind, target_repo, target_key, target_token, PK(src_rel_path, ord))` (`ord` is the citation's position in its document: 444 lines in `docs/trackers` repeat one token on one line, so a key on `(line, raw)` is not unique; `grammar::Citation` has only `reference` and `line`, so there is no raw text, and the target columns come from the `Reference` variant); `chunk(chunk_id PK, rel_path, chunk_ix, start_line, end_line, entry_token, entry_part, entry_parts, content, content_hash)`; `problem(kind, key, rel_paths)` (`rel_paths` sorted JSON); `skipped(rel_path PK, reason)`; `parse_cache(sha256, rel_path, rules_hash, parser_version, parse_note, entries_json, citations_json, chunks_json, PK(sha256, rel_path, rules_hash, parser_version))` (Task 4).
  - `pub fn dump_all(conn: &Connection) -> BTreeMap<String, Vec<Vec<String>>>` — every table except `parse_cache`, with `checkout.root` and `checkout.commit_sha` left out (a path and a sha identify the clone, not the content), rows rendered as strings and sorted; the single oracle for the equality tests.

- [ ] **Step 1: Write the failing tests:** `open_creates_the_schema_once_and_a_second_open_changes_nothing`; `a_newer_user_version_is_refused_by_name`; `dump_all_is_sorted_and_stable_across_insert_order`; `the_dump_leaves_out_the_clone_identity_columns`; `no_table_has_a_timestamp_or_random_default` (query `sqlite_master` for `datetime`, `random`, `uuid`, `current_timestamp` in any column default or trigger).
- [ ] **Step 2: Run** → FAIL. **Step 3: Implement.** **Step 4: Run** → PASS.
- [ ] **Step 5: Commit** — `feat(librarian): derived index — schema and a canonical dump`.

---

### Task 3: A full rebuild

**Files:**
- Create: `src/librarian/derived/build.rs`
- Modify: `src/librarian/derived/mod.rs`, `src/librarian/identity/repo_name.rs` (an additive `pub fn parse_text(text: &str) -> anyhow::Result<RepoName>` that `read` also calls, so the blob route and the file route share one parser; `read` does not change behaviour)
- Test: `src/librarian/derived/build.rs`

**Interfaces:**
- Consumes: Tasks 1 and 2; `frontmatter::{parse, body_line_offset}`; `classify::{classify, load_rules, default_rules, CompiledRule}`; `indexer::first_h1` (pub, `indexer.rs:143-167`); `identity::{key::key_of, index::{KeyIndex, DocInput}, grammar::extract}`; `link_scan::extract::entry_sections` (pub, `extract.rs:67`); `catalog::chunk::build_chunks`; `identity::repo_name::read`.
- Produces:
  - `pub struct BuildInput<'a> { pub repo: &'a git2::Repository, pub commit: &'a git2::Commit<'a>, pub root: &'a Path, pub rules: &'a [CompiledRule], pub space: &'a KeySpace }`
  - `pub struct BuildReport { pub documents: usize, pub entries: usize, pub edges: usize, pub chunks: usize, pub skipped: usize, pub problems: usize }`
  - `pub fn rules_from_commit(repo: &git2::Repository, commit: &git2::Commit) -> anyhow::Result<Vec<CompiledRule>>` — `.codescout/librarian.toml` blob (if present) followed by `default_rules()`; a parse error is an `Err` naming the file (loud, as `load_project_rules`).
  - `pub fn key_space_from_commit(repo: &git2::Repository, commit: &git2::Commit) -> anyhow::Result<KeySpace>` — the commit's `.codescout/key-space.toml` blob through `blob_text` and `KeySpace::from_text` (absent → empty).
  - `pub fn build_full(conn: &mut Connection, input: &BuildInput) -> anyhow::Result<BuildReport>` — one transaction that deletes every derived row and rebuilds: scan (Task 1) → `key_of` per document → `KeyIndex::build(repo_name, docs)` (repo name from the commit's `.codescout/repo.toml` blob through `repo_name::parse_text`, recorded as `name_source = "committed"`; when the blob is absent use the literal `repo` recorded as `name_source = "unconfirmed"`, never `suggest`) → per document `frontmatter::parse` (broken → Global Constraints ruling 2), metadata derivation copied from `indexer.rs:578-622` (kind: frontmatter > rule > `unknown`; status: frontmatter > rule > `unknown` if kind unknown else `draft`; time_scope; confidence 1.0 if frontmatter kind is set else 0.5; title: frontmatter > `first_h1`; tags: frontmatter union rule tags; owners; topic) → `entry_sections` → `extract` + `index.resolve` for each citation → `build_chunks(rel_path, body, 2048, line_offset)` with each `chunk_id` filled by the deterministic rule. Edges whose resolution is `NotLocal` are not stored.

- [ ] **Step 1: Write the failing tests** on a fixture commit built through `git2`: `documents_get_keys_owner_and_metadata`; `a_broken_frontmatter_file_is_indexed_with_a_parse_note` (Review Focus 1); `entries_are_the_headings_that_define_tokens`; `citations_become_edges_with_their_resolution_kind` (File, Entry, Local, UnknownKey, KeyCollision, CrossRepo, Malformed each once); `chunk_ids_are_deterministic` (two builds → equal ids); `an_invalid_key_is_stored_as_a_problem_and_the_document_row_keeps_key_error`; `a_collision_excludes_both_from_the_key_maps_but_keeps_both_rows`; `rebuilding_twice_dumps_identically`; `rules_come_from_the_commit_blob_then_the_builtin_defaults` (a committed rule changes a kind; with no `.codescout/librarian.toml` blob the defaults apply; the function takes no config path, so a machine-global rule has no way in); `the_repo_name_and_key_space_come_from_the_commit_not_the_working_tree` (commit a name and a key-space file, change both in the working tree, build again: the committed ones are used); `a_token_cited_twice_on_one_line_stores_two_edges`.
- [ ] **Step 2: Run** → FAIL. **Step 3: Implement.** Reuse, do not copy, the pure functions named under Consumes; copy only the ~45-line metadata derivation, as a private fn with a comment citing `indexer.rs:578-622` and the precedence it mirrors. **Step 4: Run** → PASS.
- [ ] **Step 5: Commit** — `feat(librarian): derived index — full rebuild of one commit`.

---

### Task 4: The incremental update

**Files:**
- Create: `src/librarian/derived/incremental.rs`
- Modify: `src/librarian/derived/mod.rs`, `src/librarian/derived/build.rs` (extract the per-document parse into `fn parse_document(rel_path: &str, text: &str, rules: &[CompiledRule]) -> ParsedDocument` and store its products in `parse_cache` by `sha256`)

**Interfaces:**
- Consumes: Task 3; `git::diff_tree_to_tree` (pub, `git/mod.rs:32`).
- Produces:
  - `pub struct ParsedDocument { pub metadata: DocMetadata, pub parse_note: Option<String>, pub entries: Vec<EntryRow>, pub citations: Vec<Citation>, pub chunks: Vec<ChunkRow> }` (serialisable, stored as JSON in `parse_cache`).
  - `pub fn update(conn: &mut Connection, input: &BuildInput, from_commit: &git2::Commit) -> anyhow::Result<BuildReport>` — scans the new commit, reuses `parse_cache` rows whose key is unchanged and parses only the rest (the cache key is `(sha256, rel_path, rules_hash, PARSER_VERSION)`: classification and chunk ids depend on the path, so a rename or an archive move reparses; `rules_hash` covers the committed rules blob and the compiled-in `DEFAULT_RULES_TOML`; `PARSER_VERSION` is a `const` bumped whenever the parse logic changes), then always rebuilds the key index and re-resolves **every** document's edges over the whole tree, and writes the result in one transaction. `diff_tree_to_tree` is used only to report counts (added, modified, deleted, renamed); correctness does not depend on it.
  - The cache is a pure optimisation: `update` over any `from_commit`, including a commit that is not an ancestor, must dump equal to `build_full` at the same commit.

- [ ] **Step 1: Write the failing tests:** `update_dumps_equal_to_a_full_rebuild_after_modify_delete_and_rename` (Review Focus 2); `a_new_document_resolves_an_old_unknown_key_edge` and `a_new_colliding_document_turns_old_file_edges_into_collisions` (Review Focus 3); `unchanged_files_are_not_reparsed` (count parse calls through a counter passed to `parse_document` behind `#[cfg(test)]`); `a_changed_rules_blob_invalidates_the_cache`; `a_rename_reparses_the_file_and_matches_a_rebuild`; `the_parser_version_is_part_of_the_cache_key`; `update_from_an_unrelated_commit_still_equals_a_rebuild`; `the_parse_cache_is_excluded_from_the_dump`.
- [ ] **Step 2: Run** → FAIL. **Step 3: Implement.** **Step 4: Run** → PASS, and re-run Task 3's tests (the extraction must not change a dump).
- [ ] **Step 5: Commit** — `feat(librarian): derived index — incremental update equals a rebuild`.

---

### Task 5: The two-clone test and determinism sweep

**Files:**
- Test: `src/librarian/derived/build.rs` (`mod tests`), a helper `fn clone_repo(src: &Path, dst: &Path)` using `Command::new("git").args(["clone", …])` with a `file://` URL

**Interfaces:**
- Consumes: Tasks 3 and 4.
- Produces: tests only.

- [ ] **Step 1: Write the tests:** `two_clones_at_one_commit_dump_identically` (Review Focus 4: clone, set different `user.name`/`user.email` in each clone, build both at the same sha into two files, compare `dump_all`); `the_checkout_row_is_the_only_place_a_path_appears` (search every string `dump_all` returns for the clone's directory name: none; `checkout.root` is left out of the dump and the test reads it separately); `build_order_does_not_change_the_dump` (build from a scan list shuffled by a test hook, equal dumps); `a_working_tree_edit_that_is_not_committed_changes_nothing`.
- [ ] **Step 2: Run** → PASS (these exercise Tasks 1 to 4); if one fails, fix the cause in the owning task's code, not in the test. **Step 3: Commit** — `test(librarian): derived index — two clones and determinism`.

---

### Task 6: The shadow diff command

**Files:**
- Create: `src/librarian/derived/diff.rs`, `src/cli/derive_shadow.rs`
- Modify: `src/librarian/derived/mod.rs`, `src/cli/mod.rs` (`pub mod derive_shadow;`), `src/main.rs` (a `Commands::DeriveShadow` variant and arm; model: `BackfillChunks` :224 and :512-514)
- Test: `src/librarian/derived/diff.rs`

**Interfaces:**
- Consumes: Tasks 1 to 4; `rusqlite::OpenFlags`.
- Produces:
  - `pub struct OldCatalog` and `pub fn open_old_read_only(path: &Path) -> anyhow::Result<OldCatalog>` — `Connection::open_with_flags(path, SQLITE_OPEN_READ_ONLY | SQLITE_OPEN_NO_MUTEX)` (pattern `migrate/memories.rs:134-137`); a missing file is an `Err`; it never runs `apply_schema`, `run_migrations` or `audit::install`.
  - `pub struct ShadowDiff { pub only_old: Vec<DiffRow>, pub only_new: Vec<DiffRow>, pub changed: Vec<DiffRow>, pub counts: DiffCounts }` and `pub enum DiffCause { FileSetWalkVsCommit, ParseRule, ClosedGrammarDropsEdge, KeyOutsideSpace, Unexplained }`; every row carries its cause when one is known and `Unexplained` otherwise.
  - `pub fn diff(old: &OldCatalog, derived: &Connection, git_root: &Path) -> anyhow::Result<ShadowDiff>` — compares documents (old `artifact.abs_path` under the git root, made git-root-relative, with `.worktrees/` shadow rows excluded, against `document.rel_path`) on kind, status, title, owners, tags and `file_sha256`; chunk counts per document; old `artifact_link` and `entry_cite` against new `File` and `Entry` edges.
  - CLI: `codescout derive-shadow [--project PATH] [--rev HEAD] [--old-db PATH] [--json]` — resolves the git root (`current_project::lookup_git_root`, no `open_ctx`), builds into a scratch file in a temp directory (never inside the repo), opens the old catalog read-only (default `dirs::data_local_dir()/librarian/catalog.db`, resolved here at the edge, overridable), prints counts and the first 20 rows per cause. Exit `0` always when it ran; `2` and `NOT COMPARED — <reason>` when the old catalog is absent or cannot be read.

- [ ] **Step 1: Write the failing tests** (an in-memory "old" catalog created by hand with the few columns the diff reads, never `Catalog::open`): `a_file_the_walk_skipped_but_the_commit_tracks_is_only_new_with_the_walk_cause`; `a_file_only_the_old_catalog_has_is_only_old`; `a_title_difference_is_changed_and_names_the_field`; `a_hex_id_edge_the_grammar_drops_is_explained`; `a_missing_old_catalog_is_not_compared_and_is_never_created` (Review Focus 5: assert the path still does not exist afterwards); `the_diff_opens_the_old_file_read_only` (opening a file made read-only on disk succeeds and the file's bytes are unchanged afterwards).
- [ ] **Step 2: Run** → FAIL. **Step 3: Implement.** **Step 4: Run** → PASS.
- [ ] **Step 5: Commit** — `feat(cli): codescout derive-shadow — diff a derived index against the old catalog`.

---

### Task 7: Run it on this repo and record what it shows

**Files:**
- Modify: `docs/trackers/artifact-identity-session-log.md` (through `doc(action="append_entry", id="abff63149e3c24e8", …)`)
- Test: `src/librarian/derived/build.rs` — an `#[ignore]` test `derived_index_of_this_repo` that builds this checkout's HEAD into a temp file and prints the `BuildReport` and the skipped reasons (`cargo test --lib derived_index_of_this_repo -- --ignored --nocapture`)

**Interfaces:**
- Consumes: Tasks 1 to 6.
- Produces: measured numbers, nothing else.

- [ ] **Step 1: Run the ignored test** and `codescout derive-shadow --json` against the live catalog (read-only; do not stop other sessions: the command takes no lock). Expected: both finish; the diff's `Unexplained` count is the number to chase.
- [ ] **Step 2: Record** the counts (documents, entries, edges by kind, chunks, skipped by reason, diff rows by cause, `Unexplained`) and the six rulings of Global Constraints in one `F-` or `W-` entry, with the old catalog's own counts beside them. They size Plan E's cutover.
- [ ] **Step 3: If `Unexplained` is not zero,** list the first rows in the entry and open a follow-up in the same entry; do not widen the causes to hide them.
- [ ] **Step 4: Run `./scripts/gate.sh`** → green. **Step 5: Commit** — `test(librarian): derived index of this repo; shadow diff recorded`.

---

## Not built here, on purpose

- Sidecar shapes for augmentations (spec § The store boundary): Plan C.
- Storage scoped by checkout, with every other row joined on the checkout and the key (spec § The catalog is derived): B2 holds one checkout in one file, so the join is implicit. Plan E adds multi-checkout storage when reads flip.
- Machine-default rules for a checkout with no committed rules, and a record of which source applied (spec § Index inputs): B2 uses the compiled-in defaults only and records `rules_source = "committed+builtin"`. Plan E decides whether machine defaults return.

## After the plan

- Plan C starts from this schema: it adds the git-history cache and the content-keyed vector cache, and moves each store's writes into files as dual-write.
- Plan D (the cutover tool) reuses `scan_commit`, `build_full` and the shadow diff's old-catalog reader.
- Plan E (the flip) switches reads to this file. Nothing in B2 is read by any tool yet.
