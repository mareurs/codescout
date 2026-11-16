---
id: dc0d65557a9577ee
kind: plan
status: draft
title: 'Artifact identity Plan E2: derived schema v2, the missing facts'
tags:
- artifact-identity
- plan-e
- derived-index
topic: artifact-identity
---

# Artifact identity Plan E2: derived schema v2, the missing facts Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The derived index of a checkout holds every per-document fact the read flip (E4) needs and today's `derived.db` lacks: relation labels on edges, event sections, entry-document fields, augmentation shapes from committed sidecars, a git-history cache, and a working-tree layer over `HEAD`. Nothing in production reads `derived.db` after this plan (Q4), so E2 may reach `experiments`. One live tool changes behaviour: `doc(link)` (Task 9, E2-R6).

**Architecture:** Task 1 creates schema v2 at once (six tables, `edge.rel`, `parse_cache.facts_json`) and rebuilds a v1 file on open. A fact that a document's own text gives (events, relation labels, entry fields, the augmentation declaration) is a parse product: it rides the parse cache in `facts_json` and the one write path `build::rebuild_from`, so the incremental update and the "update equals rebuild" tests cover it with no new mechanism. A fact that depends on other blobs is read on every build outside the cache: the declared sidecar (Task 5) and the first-parent history (Task 7, reused only when the indexed commit, the parser version and the shallow state are unchanged). The working-tree layer (Task 8) is a second scan for `rebuild_from`, offered over `HEAD` only: the commit's files, then the `.md` paths `git status` names, read through the read door. `build_full` stays commit-only.

**Tech Stack:** Rust, `rusqlite` 0.39, `git2` 0.20 (`StatusOptions`, `Repository::is_shallow`, `DiffFindOptions`), `serde_json` with `preserve_order`, `serde_yml`.

**Spec:** `docs/superpowers/specs/2026-11-11-repo-relative-artifact-key-design.md` (v5): § The store boundary, § The catalog is derived, § Writes, § Contract with ledger formats, § Index inputs, § Left to the builder (event section format, rel-label vocabulary, git-history cache). Evidence: scout `docs/research/2026-11-15-artifact-identity-plan-e-scout.md` (`02d716631c64e393`) § What the derived catalog lacks, § E2, § (e) Q1, Q3, Q4, Q8; E1 state `docs/research/2026-11-16-artifact-identity-plan-e1-execution-state.md` (`6ba1be1f51416c0e`); C2 state `docs/research/2026-11-14-artifact-identity-plan-c2-execution-state.md` § Preconditions for Plan D and Plan E; B2 rulings `docs/research/2026-11-12-artifact-identity-plan-b2-rulings-and-deferred-findings.md` (`4c82d96fc9952e98`, deferred "Plan C: no test can see a clock"); bugs `84e6e61e03beace6`, `b2ec2d2e35fbbf30`, `133504302d03e590`; plan review `docs/research/2026-11-16-plan-e2-plan-review.md` (`9a89ced57da874b7`). Line numbers were read in `.worktrees/integration-identity-lifecycle` at `725eea39a` on 2026-11-16; find code by name if they drifted.

## Rulings (E2-R1..E2-R16; the operator may overrule any)

- **E2-R1** `SCHEMA_VERSION` 1 → 2. `schema::open` of a file at `user_version` 1 whose tables are exactly the eight v1 tables drops them and applies v2 in the same immediate transaction (the file is derived; the next build fills it). Any other file below version 2, a negative version included, is refused unchanged with `SQLITE_NOTADB`, so `cutover::open_refusal` says "is not a new catalog … Move it away" and never "made by a newer codescout". A `derived.db` made by E1's `cutover build` is emptied and rebuilt by the next `cutover build`. Cost: a v1 file's rows are discarded; a v1 file with an extra table must be moved away by hand.
- **E2-R2** Task 1 creates every v2 table and column; later tasks fill them (B2 R5 precedent). Cost: tables stay empty between tasks.
- **E2-R3** `PARSER_VERSION` goes `"1"` → `"2"` once, in Task 1; Tasks 2-5 re-pin `CORPUS_DIGEST` without another bump. No field of `FactsJson` is an `Option` or `#[serde(default)]`, so a row written before a fact existed fails to decode and is a cache miss. Nothing in production reads the parse cache (`incremental::update` has no production caller; `cutover build` and `derive-shadow` call `build_full`, which never reads it). Cost: none beyond test databases.
- **E2-R4** Canonical rel (Q8): the label's words, lowercased, joined with `_` (`rel_of_label`: `Depends on` → `depends_on`). Spellings that give one label are one rel by definition, which settles bug `84e6e61e03beace6`. The vocabulary is open: any label `lifecycle::relations` reads. An edge gets `rel` when its citation is `File` or `Entry` and its line is in a relation paragraph outside every event section; otherwise `rel` is NULL. Cost: `depends-on` and `depends_on` cannot be told apart; a prose field such as `**Note:** [[x]]` is a relation `note`.
- **E2-R5** An event's `**Mutates:**` line is excluded by the event section's line span, not by its label (C2 precondition): `link_text`'s test `one_document_holds_events_and_relation_lines_in_turn` writes a real `**Mutates:**` relation under `## Relations`. Cost: none known.
- **E2-R6** Sorted insertion (Q8) is a writer property: `splice_relation` inserts before the first declaring line of `## Relations` whose text sorts after the new line (byte order), else appends; the reader stores positions and depends on no order. `doc(link)` treats a rel whose canonical form is `supersedes` as superseding and stores `supersedes` in its row (bug `b2ec2d2e35fbbf30`); other rows keep the rel as passed (E6 drops rows). Cost: live `doc(link)` on `experiments` places lines differently; an unsorted section stays unsorted around new lines; two different lines inserted at one place on two branches still conflict (only the identical-line case of `133504302d03e590` is fixed).
- **E2-R7** Events are keyed `(rel_path, heading_line)`, so a duplicate id is two rows and never a failed build. `payload_json` is `serde_json::to_string` of the parsed payload (key order as written, `preserve_order`); `float_roundtrip` stays off; any comparison with old rows goes by event id. Cost: a float payload's stored text can differ from the committed text in the last digit (equal across clones).
- **E2-R8** Entry fields come from entry documents only (frontmatter `entry_kind`, non-null), through `lifecycle::entry::parse_entry`; a failing parse is a problem row `entry_unparsed` and no row. E2 does not own fields of section-ledger entries. Measured 2026-11-16: 16 of the 25 committed sidecars (`docs/augmentations/*.yaml`) declare an `entry_collection` whose rows live only in catalog params (a sidecar never carries params, `augmentation_sidecar.rs` `to_row`), and no committed document has `entry_kind` frontmatter (the one `git grep '^entry_kind:'` hit is inside a spec body). Cost: every such tracker loses `entry_filter` at E4 unless the E4 plan names a source for section-ledger fields (open item 1).
- **E2-R9** The augmentation declaration is parsed with `tools::doctor::parse_declaration` (an outward dependency of `derived`, like `build.rs`'s import of `tools::link_scan`; moving the function would touch `doctor.rs`, `append_entry.rs`, `reindex.rs`, `cutover/sidecars.rs` and `augmentation_sidecar.rs` for no E2 behaviour, so it moves when E6 rewrites doctor or augment). The declared path passes the live lexical rule, `augmentation_sidecar::refuse_lexical_escape` (made `pub(crate)`), and is read from the commit (`scan::blob_text`) at its `Normal` components joined with `/`, so the derived and the live reader accept and refuse the same spellings. The sidecar is read on every build, not cached. A refused path, a missing, linked, non-UTF-8 or unparsable sidecar is a problem row and no shape; `expects_augmentation: true` is a row with no shape. Sidecars are read from the commit even under the working-tree layer. Cost: one blob read per declared document per build; a link inside the repository is accepted live and refused here (`blob_text` R8); `derived` depends on `tools::doctor`; a sidecar edit is invisible to the layer until committed (open item 2).
- **E2-R10** History: the first-parent line from the indexed commit; renames followed with `git::diff_trees` (pathspec `*.md`, git's 50% threshold, `RENAME_LIMIT`); rows for every `.md` path of the indexed commit's tree; committer time in seconds; author name only (no email); the frontmatter `status` of each changed blob. In a shallow repository a parent that is `NotFound` is the boundary; any other error fails the build. `build_full` always walks; `rebuild_from` with `reuse_cache` keeps the stored history only when `history_meta` holds the indexed commit, the current `PARSER_VERSION` and the current `repo.is_shallow()`. The walk has no bound. No refresh time is stored (no commit trailer exists; E6 decides it, scout Q3). Cost: a merge-heavy repository credits changes to merge commits; a rename below git's similarity starts a new history; a shallow clone differs from a full clone (G5 holds for full clones; `history_meta.shallow` says which); every `build_full` (`cutover build`, `derive-shadow`) pays the walk.
- **E2-R11** `dump_all` keeps every table; `dump_tree` (tests) leaves out `history_meta` and `doc_history` for the "same tree, two histories" test. Cost: one test helper.
- **E2-R12** Working-tree layer (Q1): `worktree::update_with_working_tree` refuses unless `input.commit` is `HEAD`'s commit, then indexes the commit's files with every `.md` path `git status` reports laid over them (untracked included, ignored and submodules excluded, no rename detection, `.md` matched case-sensitively after the pathspec). Text is read through `read_door::for_project(workdir)`. Repo name, rules, key space and sidecars stay from the commit. A conflicted path keeps the commit's text; a link is skipped unread; a path outside the key space is a `skipped outside_key_space` row, unread (as the commit scan records it); a file gone at read time is deleted; a door refusal or another I/O error fails the update. Cost: a layer over a non-HEAD commit is not offered; a dirty checkout's index is not shared by clones; libgit2 hashes changed files itself, outside the door (no bytes reach codescout); libgit2 status reads the machine's global and system git config (`core.excludesFile`, `core.ignorecase`), so the layer depends on the machine's ignore rules as well as the checkout's.
- **E2-R13** History stays a function of the commit under the layer: an uncommitted document has no `doc_history` row. Cost: E4 shows no creation time for it.
- **E2-R14** The shadow diff is unchanged. Its SQL names the columns it reads (`diff.rs:902` `load_new_edges`: `SELECT src_rel_path, kind, dst_key, target_token, line FROM edge`), so `edge.rel` and the new tables change nothing in it. Cost: no machine-wide old-versus-derived check of relations or events before E4; manual-link parity is the cutover export's item report (`cutover/links.rs::export_links`).
- **E2-R15** Task 10 records the measurements in the E2 execution state. (a) The history walk of this repository: unbounded by design; if it exceeds 5 s, the state names it a blocker for any plan that puts `build_full` on a write path. (b) On a clone with a refreshed index: a clean-tree baseline, then 500 untracked and 20 modified `.md` files, with the status, read and rebuild phases printed separately. If the status phase (`changed_markdown`) median exceeds 100 ms, or the layer phases (status plus read) exceed 2 s, the state names it an E6 blocker; the threshold does not apply to `rebuild_from`, which every update pays with or without a layer. Cost: E6 may need a cheaper path (the writer names the paths it wrote).
- **E2-R16** No tool, schema or prompt surface changes; nothing in production opens or reads `derived.db` beyond `cutover build` and `derive-shadow`'s scratch file; the E1-R9 tripwire (`tests/inline_gates.rs:705` `only_the_cutover_uses_the_checkouts_catalog_path`) is not edited. The one live behaviour change is `doc(link)` (E2-R6). Cost: none beyond E2-R6.

## Global Constraints

- `TOOL_SURFACE_CHAR_BUDGET` is exactly 57_664 (headroom 0). No MCP tool description or schema changes. No prompt surface changes; `ONBOARDING_VERSION` does not change.
- Q4: nothing in production opens `derived.db` beyond `cutover build` and `derive-shadow`'s scratch file. No new file names `derived_catalog_in` or `DERIVED_CATALOG_REL`.
- Read door: production code under `src/librarian` uses none of `fs::read`, `fs::read_to_string`, `fs::OpenOptions`, `File::open`, `File::options`, `OpenOptions::new`, `io::read_to_string`, `tokio::fs` (`read_door.rs:1421` `no_librarian_production_read_bypasses_the_read_door`). `symlink_metadata` and `git2` are not read tokens.
- Determinism: no `DEFAULT`, trigger or generated column, and no clock word in schema text (`schema.rs:586` `no_table_has_a_timestamp_or_random_default`). Every stored time comes from a commit or committed text.
- Cap gate (`tests/result_caps.rs`): no new constant whose name contains `CAP`, `LIMIT`, `MAX`, `BUDGET`, `THRESHOLD`; no new `.take(`, `.truncate(`, `truncate_compact(` or sync `.next()` in `src` without a `// cap-class:` line above it. The history walk has no bound (it ends when every path is traced or at the root).
- All parsing, blob reads, status and file reads happen before the write transaction opens (`incremental.rs` `no_parse_runs_while_the_write_transaction_is_open`).
- Command forms: lib tests `scripts/with-slot.sh cargo test --lib -- <filter> [<filter>…]`; ignored measurements `scripts/with-slot.sh cargo test --lib -- --ignored --nocapture <name>`. Implementers do not run `./scripts/gate.sh`; Task 10 does. Commit named paths only (`docs/conventions/shared-checkout-commit-sequence.md`).
- Tests that run the `git` CLI use `testkit::git` (Task 6 moves it there), which sets `GIT_CONFIG_GLOBAL=/dev/null` and `GIT_CONFIG_NOSYSTEM=1`.

## Review Focus

1. **A `.md` file deleted in the working tree, or gone between the status and the read.** Expect no document row, citations of its key `unknown_key`, and a `deleted` layer row. Tests: Task 8 `the_layer_shows_markdown_edits_and_never_the_config`, `a_file_gone_between_status_and_read_is_deleted`.
2. **A rename inside the working tree, staged (`git mv`) or not (`mv`).** Expect the old key gone, the new key present, the old citations `unknown_key`. Test: Task 8 `a_rename_in_the_working_tree_is_a_delete_and_an_add`.
3. **A conflicted (unmerged) file.** Expect the commit's text, no conflict marker parsed, a `conflicted` layer row. Test: Task 8 `a_conflicted_file_keeps_its_committed_text`.
4. **A sidecar edited while its document is unchanged.** The document's parse is cached by its own hash; expect the shape to follow the sidecar on update. Test: Task 5 `a_sidecar_edit_alone_changes_the_shape_on_update`.
5. **A shallow clone, or one tree under two histories.** Expect `shallow = 1` with history starting at the boundary, times from commits and never the clock, and two histories differing only in the history tables. Tests: Task 7 `a_shallow_clone_says_so_and_its_history_starts_at_the_boundary`, `times_and_authors_are_the_commits_never_the_clock`, the revised `the_same_tree_committed_on_different_days_by_different_identities_dumps_identically`.

Also pinned: a layer over a non-HEAD commit (Task 8), a linked, non-UTF-8 or out-of-key-space working file and CRLF text (Task 8), a duplicate event id (Task 2), a path renamed onto a freed name (Task 7), an older or negative-version catalog file (Task 1).

## Scout corrections

- Vectors keyed by chunk are E5, not E2 (the scout summary lists them among the five gaps; its E2 section does not). E2 adds no vector table and does not touch `vector-cache.db` or `ArtifactVectorStore`.
- "Ledger entry fields (reuse the lifecycle reader)": `lifecycle::entry::parse_entry` (`entry.rs:104`) reads the frontmatter of an entry document (one file per entry), not a `## F-3 — title` section. A section ledger keeps its fields only in augmentation params; no markdown format exists for them (E2-R8, open item 1).
- The history cache needs no per-commit incremental store: `git log --first-parent -M --name-status -- '*.md'` over this repository's 8,111 first-parent commits (9,822 in all, 201 merges) took 0.68 s on 2026-11-16. E2 walks again when the indexed commit changes and reuses at the same commit (E2-R10). Task 10 measures the libgit2 walk.
- Per-write status cost (scout: "not measured"): `git status --porcelain --untracked-files=all -- '*.md'` in the main checkout took 25 ms and listed 507 paths, mostly untracked. The layer must cope with hundreds of untracked files; libgit2's cost is Task 10's measurement.
- The `**Mutates:**` filter is by event span, not by label (E2-R5).
- The shadow diff does not need to learn the new tables (E2-R14); the B2 equality tests extend by themselves because `dump_all` lists every table from `sqlite_master` (`schema.rs:245`).
- Refresh time "from git history (a commit trailer)": no writer produces a trailer, so E2 stores none (E2-R10).
- Line drift: the test pinning commit-only reads is `build.rs:2125` `a_working_tree_edit_that_is_not_committed_changes_nothing`; it stays unchanged (`build_full` stays commit-only); `edge_kind` is at `build.rs:654`.

## Review corrections

Every finding of `9a89ced57da874b7` was checked against the code or a read-only run before this revision. Accepted as written: I-2 (`schema.rs:513-518` pins seven dump tables), I-3 (`repo.statuses()` compares HEAD, index and workdir only), I-4, I-5 (re-counted: `git ls-files 'docs/augmentations/*'` 25, `git grep -l '^entry_collection:' -- 'docs/augmentations/*'` 16, `^entry_kind:` only in `docs/superpowers/specs/2026-11-11-tracker-lifecycle-design.md`), M-1, M-2, M-5, M-6 (`first_h1` trims, so titles agree; chunk text and blockquote payloads keep `\r`), M-7, M-8, M-9 (`**Depends on:**` < `**Depends:**` because a space sorts before `:`), M-10, M-11, M-12 (19 `#[test]` in `relations.rs`; `edge_kind` at `build.rs:654`), M-13 (`init_schema` applies the schema only at 0, `schema.rs:185-200`), M-15, M-16, M-17. Taken with a choice:

- I-1: `SQLITE_NOTADB` for an older file the rebuild does not take, so the existing `open_refusal` text applies (`cli/cutover.rs:448-451`); a cutover test pins that the text never says "newer".
- M-3: one rule, the live one (`refuse_lexical_escape`, `augmentation_sidecar.rs:218-238`); the tree path is built from `Path::components()`'s `Normal` parts, so `docs/./augmentations/t.yaml` reads `docs/augmentations/t.yaml` in both readers. The stricter rule of the first draft is dropped.
- M-4: the import stays (E2-R9 names the cost and the reason).
- M-14: also no `Option` field in `FactsJson`, because serde reads a missing `Option` field as `None` even without `#[serde(default)]`.

Rejected: none.

## File map

- `src/librarian/derived/schema.rs` — v2 SQL, older-file handling on open, `dump_tree` (Task 1); `src/cli/cutover.rs` — one test (Task 1).
- `src/librarian/derived/build.rs` — `DERIVED_TABLES`, `PARSER_VERSION`, `ParsedDocument` and its new rows, `parse_document`, `store_document`, `rebuild_from` (Tasks 1-5, 7, 8); its test corpus and `assert_corpus_is_not_vacuous` grow with each fact.
- `src/librarian/derived/incremental.rs` — `Stored.facts_json`, `FactsJson` mirror, cache corpus and `CORPUS_DIGEST` (Tasks 1-5).
- `src/librarian/catalog/event_text.rs` — event spans (Task 2). `src/librarian/lifecycle/relations.rs` — `relation_labels`, `rel_of_label` (Task 3). `src/librarian/augmentation_sidecar.rs` — `refuse_lexical_escape` visibility (Task 5).
- `src/git/mod.rs` (`diff_trees`), `src/librarian/derived/testkit.rs` (Task 6).
- Create `src/librarian/derived/history.rs`; modify `derived/mod.rs` (Task 7).
- Create `src/librarian/derived/worktree.rs` (Task 8).
- `src/librarian/catalog/link_text.rs`, `src/librarian/tools/link.rs` (Task 9).

---

### Task 1: Schema v2 and older files

**Files:**
- Modify: `src/librarian/derived/schema.rs:10` (`SCHEMA_VERSION`), `:40-117` (`SCHEMA_SQL`), `:119-127` (dump constants), `:185-200` (`init_schema`), `:133` (`open` doc); tests `:372`, `:468`, `:547`
- Modify: `src/librarian/derived/build.rs:52` (`PARSER_VERSION`), `:72-74` (`DERIVED_TABLES`)
- Modify: `src/librarian/derived/incremental.rs:140-146` (`Stored`), `:149-216` (`encode`), `:221-290` (`decode`), `:321-342` (`load`), `:348-396` (`sync_cache`), `:1425-1450` (digest)
- Test: `src/cli/cutover.rs` (beside `build_refuses_a_file_that_is_not_a_new_catalog_and_names_no_path`, `:1762`)

**Interfaces:**
- Produces: `pub const SCHEMA_VERSION: i64 = 2;` and these SQL additions (no `DEFAULT`):
  - `edge`: new last column `rel TEXT`.
  - `parse_cache`: new last column `facts_json TEXT NOT NULL`.
  - `CREATE TABLE event (rel_path TEXT NOT NULL, heading_line INTEGER NOT NULL, end_line INTEGER NOT NULL, event_id TEXT NOT NULL, kind TEXT NOT NULL, created_at_ms INTEGER NOT NULL, author TEXT, anchor_commit TEXT, head_commit TEXT, parent TEXT, resolves TEXT, source_json TEXT, mutates TEXT NOT NULL, payload_json TEXT NOT NULL, PRIMARY KEY (rel_path, heading_line)) WITHOUT ROWID;`
  - `CREATE TABLE entry_doc (rel_path TEXT NOT NULL PRIMARY KEY, entry_kind TEXT NOT NULL, status_since TEXT NOT NULL, created TEXT NOT NULL, closed TEXT, revisit_by TEXT, valid TEXT, fields_json TEXT NOT NULL) WITHOUT ROWID;`
  - `CREATE TABLE augmentation (rel_path TEXT NOT NULL PRIMARY KEY, sidecar_rel_path TEXT, schema_version INTEGER, prompt TEXT, entry_collection TEXT, params_schema TEXT, render_template TEXT, append_mode INTEGER, history_cap INTEGER) WITHOUT ROWID;`
  - `CREATE TABLE history_meta (commit_sha TEXT NOT NULL, parser_version TEXT NOT NULL, shallow INTEGER NOT NULL);`
  - `CREATE TABLE doc_history (rel_path TEXT NOT NULL, walk_pos INTEGER NOT NULL, commit_sha TEXT NOT NULL, committed_at INTEGER NOT NULL, author TEXT NOT NULL, change_kind TEXT NOT NULL, status TEXT, PRIMARY KEY (rel_path, walk_pos)) WITHOUT ROWID;`
  - `CREATE TABLE worktree_layer (rel_path TEXT NOT NULL PRIMARY KEY, state TEXT NOT NULL, sha256 TEXT) WITHOUT ROWID;`
- Produces: private `const V1_TABLES: [&str; 8]` (`checkout`, `chunk`, `document`, `edge`, `entry`, `parse_cache`, `problem`, `skipped`); `#[cfg(test)] pub(crate) fn dump_tree(conn: &Connection) -> BTreeMap<String, Vec<Vec<String>>>` (`dump_all` minus `history_meta`, `doc_history`).
- Produces: `DERIVED_TABLES: [&str; 11]` = the seven of today plus `event`, `entry_doc`, `augmentation`, `worktree_layer` (history tables are not in it: Task 7 owns them). `PARSER_VERSION = "2"`. `incremental::FactsJson` (serde, no field yet) encoded into `Stored.facts_json` and read by `load`/`decode`.

- [ ] **Step 1: Write the failing tests** in `schema.rs`: `a_version_1_derived_index_is_emptied_and_gets_schema_2` (a file with `CREATE TABLE <name> (x)` for each of `V1_TABLES`, one row in `document`, `user_version` 1; `open` it; assert `user_version` 2, the table list equals the 14 v2 names, `document` has 0 rows and `pragma_table_info('edge')` names `rel`; mutation: leave a version-1 file as today, the list stays at 8). `an_older_derived_index_with_another_table_is_refused_and_left_unchanged` (the same plus `CREATE TABLE notes (x)`; `open` errs with `ErrorCode::NotADatabase` and text naming `user_version 1` and the path as spelled; file bytes equal before and after; mutation: drop the table-set comparison). `a_negative_user_version_is_refused_and_left_unchanged` (an empty file with `user_version` −1; same assertions; mutation: refuse only versions 1..2). Update `open_creates_the_schema_once_and_a_second_open_changes_nothing` to the 14 sorted names; `dump_all_is_sorted_and_stable_across_insert_order` to the 13 dump tables (all but `parse_cache`); and `an_edge_has_a_nullable_dst_key_after_the_target_columns_and_the_dump_shows_it` to the nine names ending `dst_key`, `rel`, finding `dst_key` by name, with `r[8] == "N"` for `rel`. In `cli/cutover.rs`: `build_refuses_an_older_catalog_it_cannot_rebuild_and_never_says_newer` (a v1 file with an extra table at `DERIVED_CATALOG_REL`: exit 2, text holds `is not a new catalog`, not `newer`, and no absolute path; mutation: refuse with `SQLITE_SCHEMA`).
- [ ] **Step 2: Run** `scripts/with-slot.sh cargo test --lib -- derived::schema cli::cutover`. Expected: the three new schema tests, the three updated ones and the new cutover test fail.
- [ ] **Step 3: Implement.** `init_schema`: when `found != SCHEMA_VERSION` and `found != 0`, inside the immediate transaction: `found == 1` with a table set (from `sqlite_master`, `sqlite_%` excluded) equal to `V1_TABLES` → `DROP TABLE` each, apply `SCHEMA_SQL`, set `user_version`; otherwise (`found < 0`, or another table set) a `SQLITE_NOTADB` failure naming the version and the path as given, before WAL is set (newer files keep today's `SQLITE_SCHEMA` refusal). `open` doc gains one sentence on the v1 rebuild. Add `facts_json` to `Stored`, `encode`, `decode`, `load`, `sync_cache`; bump `PARSER_VERSION`; add `stored.facts_json` to the digest parts and re-pin `CORPUS_DIGEST`.
- [ ] **Step 4: Run** `scripts/with-slot.sh cargo test --lib -- derived:: cli::cutover cli::derive_shadow`. Expected: PASS.
- [ ] **Step 5: Commit** `feat(derived): schema v2 tables; a v1 file is rebuilt on open (E2-R1, E2-R2)`.

### Task 2: Event sections

**Files:**
- Modify: `src/librarian/catalog/event_text.rs:303-319` (`parse_event_sections`), tests
- Modify: `src/librarian/derived/build.rs` (`ParsedDocument` `:413-422`, `parse_document` `:428-460`, `store_document` `:550-652`, test `corpus` `:1774`, `assert_corpus_is_not_vacuous` `:1829`)
- Modify: `src/librarian/derived/incremental.rs` (`FactsJson`, cache `corpus` `:1308`, `CORPUS_DIGEST`)

**Interfaces:**
- Produces: `pub(crate) struct EventSpan { pub heading_line: u32, pub end_line: u32 }` (1-based lines of the whole document, numbered as `grammar::extract` numbers them: offset `frontmatter::body_line_offset(doc, body_lenient(doc))`); `pub(crate) fn parse_event_sections_spanned(doc: &str) -> Vec<(EventText, EventSpan)>`; `parse_event_sections` maps `.0`. `end_line` is the line before the next heading of level ≤ 3, or the last line.
- Produces: `pub struct EventRow { pub heading_line: u32, pub end_line: u32, pub event_id: String, pub kind: String, pub created_at_ms: i64, pub author: Option<String>, pub anchor_commit: Option<String>, pub head_commit: Option<String>, pub parent: Option<String>, pub resolves: Option<String>, pub source_json: Option<String>, pub mutates: Vec<String>, pub payload_json: String }`; `ParsedDocument.events: Vec<EventRow>`. `mutates` is stored with the existing `json_array` (`build.rs:722`); `source_json` and `payload_json` with `serde_json::to_string` (E2-R7).

- [ ] **Step 1: Write the failing tests.** `event_text.rs`: `spanned_sections_carry_whole_document_lines` (a document with a three-line frontmatter, `## Events`, two rendered sections, then `## Later`; the first `heading_line` is the 1-based index of its `### ` line in `doc.lines()`, the second `end_line` is the line before `## Later`; mutation: drop the frontmatter offset, off by 3). `build.rs`: `event_sections_become_event_rows` (a note with text `first\nsecond` and mutates `[[b]]`, and a verdict with payload `{"outcome":"go","score":1.5}`; `SELECT event_id, kind, created_at_ms, mutates, payload_json, heading_line, end_line FROM event ORDER BY heading_line` gives both rows, `mutates` = `["[[b]]"]`, payloads `{"text":"first\nsecond"}` and `{"outcome":"go","score":1.5}`; mutation: store `mutates.join(" ")`). `one_id_twice_is_two_event_rows` (two rendered sections with one id; the build succeeds with two rows; mutation: key `event` on `(rel_path, event_id)`).
- [ ] **Step 2: Run** `scripts/with-slot.sh cargo test --lib -- event_text derived::build`. Expected: FAIL.
- [ ] **Step 3: Implement** the spanned parse, `EventRow`, the parse and the insert; add `events: Vec<EventJson>` to `FactsJson` with `encode`/`decode`. Add `docs/notes.md` (an `## Events` section with both payload forms) to both corpora; `assert_corpus_is_not_vacuous` requires `event` rows; re-pin `CORPUS_DIGEST` (E2-R3).
- [ ] **Step 4: Run** `scripts/with-slot.sh cargo test --lib -- derived:: event_text`. Expected: PASS.
- [ ] **Step 5: Commit** `feat(derived): event sections are rows of the event table (E2-R7)`.

### Task 3: Relation labels on edges

**Files:**
- Modify: `src/librarian/lifecycle/relations.rs:65-116` (`relations`), tests
- Modify: `src/librarian/derived/build.rs` (`ParsedDocument`, `parse_document`, the edge insert in `store_document` at `:613`, corpus)
- Modify: `src/librarian/derived/incremental.rs` (`FactsJson`, corpus, digest); test in `src/librarian/catalog/link_text.rs`

**Interfaces:**
- Consumes: `parse_event_sections_spanned` (Task 2).
- Produces: `pub fn relation_labels(doc: &str) -> BTreeMap<u32, String>` (line → label, the paragraph rule `relations` applies today; `relations` is rebuilt on it, behaviour unchanged); `pub fn rel_of_label(label: &str) -> String` (`split_whitespace`, ASCII lowercase, join `_`).
- Produces: `ParsedDocument.relation_rels: BTreeMap<u32, String>`: line → `rel_of_label(label)`, lines inside any `EventSpan` removed. `edge.rel` = that value for a `Reference::File` or `Reference::Entry` citation, NULL for every other reference.

- [ ] **Step 1: Write the failing tests.** `relations.rs`: `rel_of_label_is_snake_case_of_the_words` (`Depends on` → `depends_on`, `Rests  on` → `rests_on`, `Supersedes` → `supersedes`; mutation: join with a space). `link_text.rs`: `every_spelling_of_one_label_has_one_canonical_rel` (`depends_on`, `depends-on`, `Depends On`, `__depends--on ` all give `rel_of_label(&label_for_rel(r).unwrap()) == "depends_on"`; mutation: keep case). `build.rs`: `a_relation_paragraph_gives_its_citations_the_canonical_rel` (text `# A\n\n## F-1 — e\n\n**Depends on:** [[b]] [[b:B-1]] F-1 [[Bad Target]]\n[[c]]\n\nProse [[d]].\n` with `docs/b.md` defining `B-1` and `docs/c.md`, `docs/d.md`; `SELECT kind, target_key, rel FROM edge WHERE src_rel_path = 'docs/a.md' ORDER BY ord` gives `file b depends_on`, `entry b depends_on`, `local NULL NULL`, `malformed … NULL`, `file c depends_on`, `file d NULL`; mutation: give every citation on a labelled line the rel). `an_events_mutates_line_has_no_rel_and_a_relations_mutates_line_does` (`## Relations\n\n**Mutates:** [[b]]\n` plus an event section mutating `[[b]]`: two edges to `b`, rels `mutates` and NULL; mutation: skip the span filter).
- [ ] **Step 2: Run** `scripts/with-slot.sh cargo test --lib -- lifecycle::relations link_text derived::build`. Expected: the new tests fail; the 19 existing `relations` tests pass.
- [ ] **Step 3: Implement**; add `relation_rels: Vec<(u32, String)>` to `FactsJson`; add `docs/rel.md` (a two-line relation paragraph) to both corpora; `assert_corpus_is_not_vacuous` requires an `edge` row whose `rel` cell is not `N`; re-pin the digest.
- [ ] **Step 4: Run** `scripts/with-slot.sh cargo test --lib -- derived:: lifecycle:: link_text`. Expected: PASS.
- [ ] **Step 5: Commit** `feat(derived): an edge carries the canonical rel of its relation line (E2-R4, E2-R5)`.

### Task 4: Entry documents

**Files:**
- Modify: `src/librarian/derived/build.rs` (`ParsedDocument`, `parse_document`, `store_document`, problem writing in `rebuild_from` `:340-352`, corpus)
- Modify: `src/librarian/derived/incremental.rs` (`FactsJson`, corpus, digest)

**Interfaces:**
- Produces: `pub struct EntryDocRow { pub entry_kind: String, pub status_since: String, pub created: String, pub closed: Option<String>, pub revisit_by: Option<String>, pub valid: Option<String>, pub fields_json: String }` (dates `%Y-%m-%d`; `valid` = `invariant`, `dated <d>` or `conditional — <c>`; `fields_json` = `serde_json::to_string(&meta.fields)`); `pub enum EntryDoc { NotAnEntry, Unparsed, Parsed(EntryDocRow) }`; `ParsedDocument.entry_doc: EntryDoc`. A document is an entry document when its frontmatter parses and `extra["entry_kind"]` is present and not null.
- Produces: per-document problem rows. `rebuild_from` collects them, sorts them by `(kind, key, rel_paths)`, and inserts them after the index's own rows, so `build_order_does_not_change_the_dump` keeps its insert-order assertion. Kind `entry_unparsed`: `key` = the document's key (NULL when invalid), `rel_paths` = `["<rel_path>"]`.

- [ ] **Step 1: Write the failing tests** in `build.rs`: `an_entry_document_stores_its_entry_fields` (frontmatter `entry_kind: finding`, `title: X`, `status: open`, `status_since: 2026-11-01`, `created: 2026-10-30`, `revisit_by: 2026-12-01`, `valid: dated 2026-11-01`, `severity: high`, `tags: [a]`; the row is `(finding, 2026-11-01, 2026-10-30, NULL, 2026-12-01, "dated 2026-11-01", {"severity":"high"})`; mutation: `fields_json` from the whole `extra`). `an_entry_document_that_does_not_parse_is_a_problem_and_no_row` (no `created`: zero `entry_doc` rows, one problem `entry_unparsed`; mutation: store a row with an empty `created`). `a_ledger_is_not_an_entry_document` (`status: active` and `## F-1 — x`, no `entry_kind`: no row, no problem; mutation: detect by `status`).
- [ ] **Step 2: Run** `scripts/with-slot.sh cargo test --lib -- derived::build`. Expected: FAIL.
- [ ] **Step 3: Implement** with `lifecycle::entry::parse_entry`; add `entry_doc` to `FactsJson`; add one entry document to both corpora; `assert_corpus_is_not_vacuous` requires `entry_doc`; re-pin the digest.
- [ ] **Step 4: Run** `scripts/with-slot.sh cargo test --lib -- derived::`. Expected: PASS.
- [ ] **Step 5: Commit** `feat(derived): entry documents store their entry fields (E2-R8)`.

### Task 5: Augmentation shapes from committed sidecars

**Files:**
- Modify: `src/librarian/derived/build.rs` (`ParsedDocument`, `parse_document`, `rebuild_from` before the transaction, `store_document`, corpus)
- Modify: `src/librarian/derived/incremental.rs` (`FactsJson`, corpus, digest; test)
- Modify: `src/librarian/augmentation_sidecar.rs:218` (`refuse_lexical_escape` becomes `pub(crate)`)

**Interfaces:**
- Consumes: `tools::doctor::parse_declaration(&Value) -> Declaration` (`doctor.rs:7041`), `augmentation_sidecar::{AugmentationSidecar, refuse_lexical_escape}`, `scan::blob_text`.
- Produces: `pub enum AugmentationDecl { None, Declared { sidecar: Option<String> }, Unparseable }`; `ParsedDocument.augmentation: AugmentationDecl` (cached). Before the transaction, `rebuild_from` reads each declared sidecar (E2-R9): `refuse_lexical_escape(p, "read")` refusing → `sidecar_invalid`, never read; else `blob_text` at the `Normal` components of `Path::new(p)` joined with `/`: `Ok(None)` → `sidecar_missing`; `Err` (link, not UTF-8) or a YAML error → `sidecar_invalid`. Problem rows: `key` = document key, `rel_paths` = `[<doc rel_path>, <sidecar>]`; `Unparseable` is `augmentation_unparseable` with `[<doc rel_path>]`. A row: `rel_path`, `sidecar_rel_path`, the sidecar's `schema_version`, `prompt`, `entry_collection`, `params_schema` (`serde_json::to_string`), `render_template`, `append_mode` (0/1), `history_cap`; `Declared { sidecar: None }` writes `rel_path` only.

- [ ] **Step 1: Write the failing tests.** `build.rs`: `a_declared_sidecar_gives_the_shape_row` (doc declares `docs/augmentations/t.yaml`, which holds `prompt: P`, `entry_collection: rows`, `append_mode: true`, `history_cap: 3`; assert the row; mutation: read the default name `rel_path_for` instead of the declaration, with the sidecar committed under another name). `a_true_declaration_is_augmented_without_a_shape` (row with `rel_path` and NULL `sidecar_rel_path`, `prompt`; mutation: map `Declared { sidecar: None }` to no row). `a_bad_sidecar_is_a_problem_and_no_shape`, table-driven, the build succeeding each time: missing file → `sidecar_missing`; `../x.yaml`, `/docs/augmentations/t.yaml` (that file committed), a `LINK`-mode sidecar, `prompt: [` → `sidecar_invalid`; mutation: drop the `refuse_lexical_escape` call (the absolute case then gives a row). `a_dot_component_reads_like_the_live_reader` (`docs/./augmentations/t.yaml` gives the row of `docs/augmentations/t.yaml`; mutation: pass the declared string to `blob_text` unchanged). `incremental.rs`: `a_sidecar_edit_alone_changes_the_shape_on_update` (commit 1 doc + sidecar `prompt: A`; commit 2 changes only the sidecar to `prompt: B`; build at 1, update to 2: `prompt` is `B` and the dump equals `build_full` at 2; mutation: keep the shape in `FactsJson`).
- [ ] **Step 2: Run** `scripts/with-slot.sh cargo test --lib -- derived::`. Expected: FAIL.
- [ ] **Step 3: Implement.** Add the declaration to `FactsJson`; add one augmented tracker and its sidecar to the build corpus (`assert_corpus_is_not_vacuous` requires `augmentation`) and a declaration to the cache corpus; re-pin the digest.
- [ ] **Step 4: Run** `scripts/with-slot.sh cargo test --lib -- derived:: cli::cutover augmentation_sidecar`. Expected: PASS.
- [ ] **Step 5: Commit** `feat(derived): augmentation shapes from the commit's sidecars (E2-R9)`.

### Task 6: Test helpers and `diff_trees`

**Files:**
- Modify: `src/git/mod.rs:43-98` (`diff_tree_to_tree` delegates), tests `:101-187`
- Modify: `src/librarian/derived/testkit.rs` (move `git` and `clone_repo` from `build.rs` tests `:1728-1760`; add `commit_files_at`), `src/librarian/derived/build.rs` tests (use the moved helpers)

**Interfaces:**
- Produces: `pub fn diff_trees(repo: &git2::Repository, from: Option<&git2::Tree>, to: &git2::Tree, pathspec: Option<&str>) -> Result<Vec<DiffEntry>>` with today's rename options (`RENAME_THRESHOLD`, `RENAME_LIMIT`, renames on, copies off); `diff_tree_to_tree` calls it with `Some(&from_tree)` and `None`.
- Produces in `testkit.rs`: `pub(crate) fn git(dir: &Path, args: &[&str]) -> String`, `pub(crate) fn clone_repo(src: &Path, dst: &Path)` (bodies unchanged), `pub(crate) fn commit_files_at(repo: &git2::Repository, files: &[(&str, &[u8])], parents: &[&git2::Commit], message: &str, author: &git2::Signature, committer: &git2::Signature) -> git2::Oid` (`commit_files` calls it with `fixed_sig` twice).

- [ ] **Step 1: Write the failing tests** in `src/git/mod.rs`: `diff_trees_from_nothing_lists_every_file_as_added` (`from = None` over a two-file tree: two `Added`; mutation: return early on `None`). `diff_trees_keeps_to_its_pathspec_and_still_sees_a_rename` (a `.md` rename beside a changed `.rs`: with `Some("*.md")` one `Renamed` and no `.rs` entry; mutation: ignore the pathspec).
- [ ] **Step 2: Run** `scripts/with-slot.sh cargo test --lib -- git::`. Expected: FAIL (`diff_trees` does not exist).
- [ ] **Step 3: Implement** `diff_trees` and the delegation; move the two helpers and add `commit_files_at`; no behaviour change anywhere else.
- [ ] **Step 4: Run** `scripts/with-slot.sh cargo test --lib -- git:: derived::`. Expected: PASS with no test count lost.
- [ ] **Step 5: Commit** `refactor(git): diff_trees over an optional base and a pathspec; derived test helpers in testkit`.

### Task 7: Git-history cache

**Files:**
- Create: `src/librarian/derived/history.rs`; modify `derived/mod.rs` (`mod history;`)
- Modify: `src/librarian/derived/build.rs` (`rebuild_from`; test `:1916`; `derived_index_of_this_repo` `:2215`); test in `incremental.rs`

**Interfaces:**
- Consumes: `git::diff_trees`, `testkit::{git, clone_repo, commit_files_at}` (Task 6).
- Produces in `history.rs`: `pub(super) struct HistoryRow { pub rel_path: String, pub walk_pos: i64, pub commit_sha: String, pub committed_at: i64, pub author: String, pub change_kind: &'static str, pub status: Option<String> }`; `pub(super) struct History { pub commit_sha: String, pub parser_version: String, pub shallow: bool, pub rows: Vec<HistoryRow> }`; `pub(super) fn walk(repo: &git2::Repository, commit: &git2::Commit) -> anyhow::Result<History>`; `pub(super) fn reusable(conn: &Connection, repo: &git2::Repository, commit: &git2::Commit) -> rusqlite::Result<bool>` (true when `history_meta` holds this commit, `PARSER_VERSION` and `repo.is_shallow()`); `pub(super) fn write(tx: &Connection, history: &History) -> rusqlite::Result<()>` (deletes both tables, inserts).
- Walk (the tests do not fix the order rule, so it is stated):

```text
tracked = { p -> p for every `.md` path in commit's tree }
c = commit; pos = 0
while tracked is not empty:
    parent = c.parent(0): none at a root; NotFound in a shallow repository is the boundary; other errors fail
    deltas = diff_trees(parent tree, c tree, "*.md")
    first every Added p:    if tracked.remove(p) = head -> row(head, pos, "added")
          every Modified p: if tracked[p] = head     -> row(head, pos, "modified")
    then  every Renamed old->p: if tracked.remove(p) = head -> row(head, pos, "renamed"); tracked[old] = head
    no parent -> stop; else c = parent; pos += 1
```

  `committed_at` = `c.committer().when().seconds()`; `author` = lossy `c.author().name_bytes()`; `status` = frontmatter `status` of the blob at `p` in `c` (None when absent or not UTF-8); `shallow` = `repo.is_shallow()`.
- `rebuild_from`, before the transaction: `None` when `reuse_cache` and `reusable(..)`, else `Some(walk(..)?)`; inside it, `write` when `Some`.

- [ ] **Step 1: Write the failing tests** in `history.rs`: `a_documents_history_follows_modify_and_rename` (c0 adds `docs/a.md` `status: draft` at t0, c1 sets `active` at t1, c2 renames to `docs/archive/a.md` at t2, c3 adds `docs/b.md`; rows of `docs/archive/a.md` are `(1, renamed, active)`, `(2, modified, active)`, `(3, added, draft)` with t2, t1, t0; `docs/b.md` has `(0, added)`; mutation: drop `tracked[old] = head`). `times_and_authors_are_the_commits_never_the_clock` (author `Alice Early`, committer `Carol Committer` at 1_262_390_400: `author` is `Alice Early`, `committed_at` is 1_262_390_400, no row within a day of now; mutation: author time or `Utc::now`). `only_the_first_parent_line_is_walked` (a side commit modifies `docs/a.md`, a merge brings it in: one row carries the merge sha, none the side sha; mutation: a full revwalk). `a_path_deleted_and_added_again_has_only_its_new_history` (c0 adds `x.md`, c1 deletes it, c2 adds it again: one row, `(0, added)`; mutation: keep tracking a path after its `Added` row). `a_rename_onto_a_freed_path_keeps_both_histories` (one commit renames `a.md` to `b.md` and adds a new `a.md`; mutation: apply renames before additions). `a_shallow_clone_says_so_and_its_history_starts_at_the_boundary` (three commits, `refs/heads/main` pointed at the tip with `repo.reference` and `repo.set_head("refs/heads/main")` before `git clone --depth 1 file://…`: `shallow` = 1, one `added` row per path at `walk_pos` 0, the build succeeds; mutations: `shallow` = false; fail on a `NotFound` parent). `incremental.rs`: `history_is_reused_only_at_the_same_commit_parser_and_shallow_state` (build; `UPDATE doc_history SET author = 'sentinel'`; an update at the same commit keeps `sentinel`; after `UPDATE history_meta SET parser_version = 'old'` an update replaces it; `build_full` and an update to the next commit replace it; mutation: compare the commit only). `build.rs`: the same-tree test compares `dump_tree` and asserts `dump_all` differs.
- [ ] **Step 2: Run** `scripts/with-slot.sh cargo test --lib -- derived::`. Expected: FAIL.
- [ ] **Step 3: Implement.** `assert_corpus_is_not_vacuous` requires `doc_history` and `history_meta`. Extend the ignored `derived_index_of_this_repo` to print the row counts of `event`, `entry_doc`, `augmentation`, `doc_history`, the edge rels grouped, and the walk time. Add an ignored `history_walk_of_this_repo` that prints walk milliseconds, rows, paths and `shallow`.
- [ ] **Step 4: Run** `scripts/with-slot.sh cargo test --lib -- derived:: cli::cutover cli::derive_shadow`. Expected: PASS; `a_working_tree_edit_that_is_not_committed_changes_nothing` unchanged and green.
- [ ] **Step 5: Commit** `feat(derived): a first-parent git-history cache per document (E2-R10, E2-R11)`.

### Task 8: The working-tree layer

**Files:**
- Create: `src/librarian/derived/worktree.rs`; modify `derived/mod.rs` (`pub mod worktree;`)
- Modify: `src/librarian/derived/build.rs` (`rebuild_from` takes the layer rows and writes `worktree_layer`; `rebuild` and `build_order_does_not_change_the_dump` pass `&[]`)

**Interfaces:**
- Consumes: `read_door::for_project`, `ReadDoor::bytes`, `ReadError::Io`, `scan::{scan_commit, ScannedFile, Skip}`, `rebuild_from`, `testkit::{git, clone_repo}`.
- Produces: `pub(crate) enum LayerState { Added, Modified, Deleted, Conflicted }` with `fn as_str(&self) -> &'static str` (`added`, `modified`, `deleted`, `conflicted`); `pub(crate) struct LayerPath { pub rel_path: String, pub state: LayerState }`; `pub(crate) fn changed_markdown(repo: &git2::Repository, commit: &git2::Commit) -> anyhow::Result<Vec<LayerPath>>` (sorted; `StatusOptions`: pathspec `*.md`, `include_untracked`, `recurse_untracked_dirs`, `include_ignored(false)`, `exclude_submodules`, no rename flags; keep paths ending `.md`; state: `CONFLICTED` → `Conflicted`, else `symlink_metadata` not found → `Deleted`, else absent from the commit tree → `Added`, else `Modified`).
- Produces: `pub(crate) struct LayerRow { pub rel_path: String, pub state: &'static str, pub sha256: Option<String> }`; `pub(crate) struct Layer { pub upserts: Vec<ScannedFile>, pub removed: BTreeSet<String>, pub rows: Vec<LayerRow> }`; `pub(crate) fn read_layer(door: &ReadDoor, workdir: &Path, space: &KeySpace, paths: &[LayerPath]) -> anyhow::Result<Layer>` (per E2-R12: `Deleted` → removed; `Conflicted` → row only; outside the key space → an upsert with `Err(Skip::OutsideKeySpace)` and an empty `sha256`, unread; a link → `Err(Skip::Symlink)`, unread; else `door.bytes`: SHA-256 of the bytes, text or `Skip::NotUtf8`; `ReadError::Io` `NotFound` → removed with state `deleted`; any other error fails).
- Produces: `pub fn update_with_working_tree(conn: &mut Connection, input: &BuildInput) -> anyhow::Result<BuildReport>`: bails `a bare repository has no working tree` without a workdir, and `the working-tree layer is laid over HEAD only` when `input.commit.id()` is not HEAD's commit; runs status and reads before `rebuild_from(conn, input, &Parser::PRODUCTION, true, None, scan, &layer.rows)`, where `scan` is `scan_commit` minus `removed`, with `upserts` replacing or adding by `rel_path`.

- [ ] **Step 1: Write the failing tests** in `worktree.rs` (a clone of a `repo_with` origin via `testkit::clone_repo`): `the_layer_shows_markdown_edits_and_never_the_config` (the edits of `build.rs`'s commit-only twin: `docs/a.md` title `A, edited`; `docs/untracked-note.md` and `docs/staged-new.md` present, `U-1` an entry; `docs/b.md` absent and `[[b]]` `unknown_key`; `repo_name`, rules and key space from the commit; `worktree_layer` exactly those paths and states; mutation: ignore `Deleted`). `a_clean_checkout_dumps_equal_to_build_full` (equal `dump_all`, empty `worktree_layer`; mutation: a row per scanned file). `a_rename_in_the_working_tree_is_a_delete_and_an_add` (`git mv` and plain `mv`: the old path has no document, the new one does; mutation: map an index rename to a `Modified` of the new path only, leaving the old one). `a_conflicted_file_keeps_its_committed_text` (a failed `git merge` of two edits of one line: the document `sha256` is the commit blob's, the row is `conflicted` with NULL `sha256`; mutation: read conflicted files). `links_binary_and_outside_space_files_are_skipped_and_not_read` (an untracked `docs/link.md` linking to a file outside the clone holding `SECRET-7d1`: `skipped` `symlink`, the marker in no stored value; `docs/bin.md` bytes `ff fe` → `not_utf8`; untracked `contrib/new.md` under the committed exclusion → `skipped outside_key_space`, no document, NULL-hash layer row; mutations: drop the link check, the door then refuses and the update fails; drop the outside upsert, the `skipped` row is missing). `crlf_text_parses_like_its_lf_twin_and_keeps_its_own_hash` (a modified `docs/a.md` with CRLF line ends and no events, against a clone whose commit holds the LF text: equal `document` kind, status, title, owners, tags, equal `entry` rows and equal `edge` rows; different `document.sha256`; chunk rows may differ; mutation: strip `\r` before hashing, the hashes then match). `a_file_gone_between_status_and_read_is_deleted` (`changed_markdown`, delete, `read_layer`; mutation: propagate `NotFound`). `a_bare_repository_is_refused_by_name` (mutation: drop the workdir check, the update then fails with another text). `a_layer_over_a_commit_other_than_head_is_refused` (`BuildInput` of `HEAD~1`: the error names HEAD; mutation: drop the check).
- [ ] **Step 2: Run** `scripts/with-slot.sh cargo test --lib -- derived::worktree`. Expected: FAIL.
- [ ] **Step 3: Implement.** Add the ignored `working_tree_layer_cost_on_a_clone_of_this_repo`: `git clone --local` of `CARGO_MANIFEST_DIR`'s repository into a temp dir; wait 1 s and run the CLI `git status` once, so the index is not racily clean; `build_full`; time a clean-tree baseline; then write 500 untracked and 20 modified `.md` files; print, each as a median of five runs, the status phase (`changed_markdown`), the read phase (`read_layer`) and the rebuild phase (`rebuild_from`), for the clean tree and the dirty one, and the layer row count.
- [ ] **Step 4: Run** `scripts/with-slot.sh cargo test --lib -- derived:: read_door`. Expected: PASS, including `no_librarian_production_read_bypasses_the_read_door` and the unchanged `a_working_tree_edit_that_is_not_committed_changes_nothing`.
- [ ] **Step 5: Commit** `feat(derived): HEAD plus the working tree's changed markdown, read through the read door (E2-R12, E2-R13)`.

### Task 9: `doc(link)` writes canonical, sorted relation lines

**Files:**
- Modify: `src/librarian/catalog/link_text.rs:44-75` (`splice_relation`), tests `:176-185`
- Modify: `src/librarian/tools/link.rs:149` (`superseding`), the row insert near `:265`, tests near `:549`

**Interfaces:**
- Consumes: `rel_of_label` (Task 3); `label_for_rel(rel: &str) -> Result<String, String>` (`link_text.rs:23`).
- Produces: `let superseding = label_for_rel(&a.rel).is_ok_and(|label| rel_of_label(&label) == "supersedes");`; a superseding row stores `supersedes`. `splice_relation`'s signature is unchanged; the line goes, with one blank line after it, before the first column-0 `**…:**` line outside a fence in the `## Relations` home whose text (trailing `\r` removed) is greater than the new line; with none, `splice_under_home` appends as today. Idempotence and the read-back check are unchanged.

- [ ] **Step 1: Write the failing tests.** `link_text.rs`: rename `a_line_joins_the_end_of_an_existing_section_and_leaves_the_next_heading_alone` to `a_line_takes_its_sorted_place_and_leaves_the_next_heading_alone`, expecting `# T\n\n## Relations\n\n**Implements:** [[b]]\n\n**Rests on:** [[a]]\n\n## Later\n\ntext\n` (mutation: always append). `a_line_that_sorts_last_still_appends` (`**Rests on:** [[a]]` then `**Supersedes:** [[b]]`: appended at the end of the section, before the next heading; mutation: insert before the first declaring line whatever its text). `the_same_link_added_on_two_branches_merges_as_one_line` (base: `**Depends on:** [[a]]` and `**Supersedes:** [[z]]`; ours adds `Implements [[k]]` then `Supersedes [[zz]]`; theirs adds `Implements [[k]]`; `testkit::git` `merge-file -p ours base theirs` on three temp files exits 0 and prints ours; mutation: append, which conflicts). `link.rs`: `a_supersedes_rel_in_any_spelling_marks_the_destination_superseded` (`Supersedes` and `SUPERSEDES`: the destination's frontmatter `status: superseded`, `rel_rows(ctx, "supersedes") == 1` (`link.rs:1392`); mutation: the exact `a.rel == "supersedes"`).
- [ ] **Step 2: Run** `scripts/with-slot.sh cargo test --lib -- link_text tools::link`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `scripts/with-slot.sh cargo test --lib -- link_text tools::link cutover:: event_text`, then `scripts/with-slot.sh cargo test --test cli_cutover`. Expected: PASS. The cutover's `export_links` output is sorted by line text whatever order it calls in, because each insert takes its own place (`(rel, form)` order is not text order: `depends` < `depends_on`, but `**Depends on:**` < `**Depends:**`); the fixtures at `cutover/links.rs:181` and `link.rs:1008` already hold text order.
- [ ] **Step 5: Commit** `fix(link): canonical supersedes and sorted relation lines (E2-R6; bugs b2ec2d2e35fbbf30, 133504302d03e590)`.

### Task 10: Gate, measurements and the Q4 check (controller)

- [ ] **Step 1:** `./scripts/gate.sh` on the branch tip in a leased `target/`. Expected: every lane 0.
- [ ] **Step 2:** Run the three ignored measurements (`derived_index_of_this_repo`, `history_walk_of_this_repo`, `working_tree_layer_cost_on_a_clone_of_this_repo`) and apply E2-R15.
- [ ] **Step 3:** Q4: `grep -rn "derived_catalog_in\|DERIVED_CATALOG_REL\|schema::open(\|update_with_working_tree" src` names only `src/cli/cutover.rs`, `src/librarian/derived/schema.rs`, `src/cli/derive_shadow.rs` and `src/librarian/derived/`. `git diff --stat <base>..` lists no file under `src/prompts/`, no tool definition and no `.codescout/system-prompt.md`.
- [ ] **Step 4:** From the main checkout: mark bug `b2ec2d2e35fbbf30` fixed and `133504302d03e590` fixed for identical lines (different lines at one place still conflict); set `84e6e61e03beace6`'s Fix to E2-R4; write `docs/research/2026-11-1x-artifact-identity-plan-e2-execution-state.md` with the measurements, rulings, deferred findings and open items 1 and 2 for the Plan E state.

## Self-review

- Spec coverage: § The catalog is derived (relation edges, sidecar shapes, git-history caches: Tasks 3, 5, 7); § The store boundary (events and notes in a single-writer document: Task 2; entries: Task 4); § Left to the builder (rel vocabulary E2-R4, cache tables Tasks 1 and 7); Q1 (Task 8 and E2-R15); Q8 (Tasks 3, 9). Out, with owners: vectors (E5), refresh time (E6), section-entry fields (E4, open item 1), shadow-diff grains (E2-R14), the layer for sidecars (E6, open item 2).
- Names checked across tasks: `EventSpan`, `parse_event_sections_spanned`, `relation_labels`, `rel_of_label`, `FactsJson`, `dump_tree`, `diff_trees`, `history::{walk, reusable, write}`, `worktree::{changed_markdown, read_layer, update_with_working_tree}`, `testkit::{git, clone_repo, commit_files_at}`.
- Every test that guards a rule names a mutation that turns it red. Every SQL column above was checked against `schema.rs:40-117` and the existing inserts in `build.rs` and `incremental.rs`.

## Open items for later plans

1. **Section-ledger entry fields (E4 decides).** 16 of the 25 committed sidecars declare an `entry_collection` whose rows live only in catalog params, and no committed document is an entry document (E2-R8). E2 indexes entry documents only. Every such tracker loses `entry_filter` at E4 unless the E4 plan names a source: entry documents in codescout-37's format (converted before E4), or a params source kept until E6. Q3's constitution rules are one case of this.
2. **Sidecars in the working-tree layer (E6 decides).** Q1 names `.md` files; `parse_declaration` accepts a sidecar at any `.yaml` or `.yml` path. Recommendation: when E6 makes `doc(augment)` file-only, it extends the layer to every declared sidecar path, not only `docs/augmentations/*.yaml`.
