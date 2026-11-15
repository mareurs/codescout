---
id: '02d716631c64e393'
kind: research
status: active
title: 'Plan E scout — the read and write flip: measured seams and a decomposition into E1..E8'
owners:
- marius
tags:
- identity
- plan-e
- scout
---

# Plan E scout — the read and write flip: measured seams and a decomposition into E1..E8

Written 2026-11-15 by a read-only scout. Code was read in the worktree
`.worktrees/integration-identity-lifecycle` (Plans A-D and the lifecycle work). All `file:line`
references point into that tree unless they name another repository. No code was changed. No test
or tool was run against that tree; every number below comes from reading files, `grep` and `wc`.

Requirements read: spec `docs/superpowers/specs/2026-11-11-repo-relative-artifact-key-design.md`
(v5, every section), the Plan D execution state (`bb10d700b9552d01`), the Plan D plan and its
manual page, the execution states of B2 (`4c82d96fc9952e98`), C1 (`9bcc00c179ecbdce`), C2
(`bf5a4e67591b53e0`) and C3 (`512e01311fe5d7bb`), memories `artifact-identity-state`,
`lifecycle-session-state`, `collection-hot-view-state`, and the four bug files that name Plan E.

## Summary

- Nothing in production reads `derived.db` today. Only `codescout cutover` and `codescout
  derive-shadow` open it (`src/cli/cutover.rs:39`, `src/cli/derive_shadow.rs:51`).
- The old catalog has 666 `catalog.lock()` call sites in 47 files. `doctor` alone has 120.
- The derived index lacks five things the live tools need: relation labels, events and notes,
  augmentation shapes, a git-history cache, and vectors keyed by something other than a hex id.
- The derived index reads a **commit** only. A test pins that an uncommitted edit changes nothing.
  The spec says every write tool "writes files, then reindexes what it wrote". Those two do not
  meet: after the flip, a write is invisible to the next read until someone commits. This is the
  largest design gap I found (operator question Q1).
- Three consumers read catalog-only facts that the cutover does not export: the goal tracker's
  params and refresh time (`goal-stop-hook`), constitution rules in params (`constitution-check`),
  and `last_refreshed_at` / `refresh_count` (`list_stale`, `gather`). The flip drops them unless
  they get a committed home first (Q3).
- Notes on shared documents stay catalog rows until codescout-37's entry-document tool exists, and
  that tool is scheduled after Plan E. That is an ordering cycle for the write flip (Q3).
- The live rename for `doc(move)` already exists, inside the cutover module
  (`src/librarian/cutover/rename.rs`, `rewrite.rs`). The deletion tail must move it out before it
  deletes the cutover.
- Recommended multi-repository layout: one `derived.db` per checkout at
  `<git root>/.codescout/derived.db` (git-ignored), with `vector-cache.db` decoupled from the
  catalog path and kept machine-global (Q2).

## (a) Seams measured

### Size of the areas Plan E touches (lines, `wc -l`, tests included)

| Area | Files | Lines |
|---|---|---|
| Old catalog | `src/librarian/catalog/*.rs` | 24,719 |
| Cutover | `src/librarian/cutover/*.rs`, `src/cli/cutover.rs`, `tests/cli_cutover.rs` | 14,008 |
| `doctor` | `tools/doctor.rs`, `tools/doctor/*.rs` | 26,492 |
| Old indexer and `reindex` | `indexer.rs`, `tools/reindex.rs` | 11,944 |
| Derived index | `src/librarian/derived/*.rs` | 8,374 |
| Audit | `catalog/audit/*.rs`, `tools/audit_log.rs`, `tests/audit_shards_cross_machine.rs` | 5,812 |
| Shadow diff | `derived/diff.rs`, `derived/old_catalog.rs`, `cli/derive_shadow.rs` | 3,844 |
| Worktree overlay | `tools/worktree.rs`, `tools/merge_worktree.rs`, `catalog/worktree.rs` | 2,751 |
| Graft | `catalog/graft.rs`, `tools/graft.rs` | 1,690 |
| Whole `src/librarian` + `src/cli` + `main.rs` | | 207,694 |

### Readers of the old catalog

`ToolContext.catalog: Arc<Mutex<Catalog>>` (`src/librarian/tools/mod.rs:85-132`) is the one handle.
The server opens one machine-global file: `LIBRARIAN_DB`, else `<data dir>/librarian/catalog.db`
(`src/librarian/mod.rs:317-325`, and a second site at `:618-622`).

`catalog.lock()` occurrences per file (tests included), top of 47 files:

| File | Count | File | Count |
|---|---|---|---|
| `tools/doctor.rs` | 120 | `tools/link.rs` | 20 |
| `tools/read_door_tests.rs` | 54 | `tools/doctor/scope.rs` | 17 |
| `tools/update.rs` | 46 | `tools/worktree.rs` | 14 |
| `tools/link_dir_tests.rs` | 43 | `tools/mv.rs` | 13 |
| `indexer.rs` | 39 | `tools/workspace_state_at.rs` | 12 |
| `tools/merge_worktree.rs` | 31 | `tools/audit_log.rs` | 10 |
| `tools/reindex.rs` | 31 | `tools/context.rs` | 9 |
| `tools/append_entry.rs` | 29 | `tools/{delete,find,gather,refresh}.rs` | 8 each |
| `tools/event_create.rs` | 26 | `tools/update_entry.rs` | 7 |
| `tools/create.rs` | 22 | `tools/{artifact,state_at}.rs` | 6 each |
| `tools/augment.rs` | 21 | `adapter.rs`, `tools/get.rs` | 5 each |

Other readers:

- **Hex-id minting.** `ids::artifact_id_from_abs`: 175 occurrences in 28 files (`indexer.rs` 30,
  `doctor.rs` 21, `worktree.rs` 11, `reindex.rs` 10).
- **Hex ids in tool output.** JSON keys `id`, `artifact_id`, `previous_id`, `src_id`, `dst_id`,
  `into_id`, `from_id` in `src/librarian/tools/*.rs`: `update.rs` 95, `get.rs` 82, `augment.rs` 56,
  `append_entry.rs` 55, `goal_aggregation.rs` 40, `mv.rs` 35, `artifact.rs` 35 (counts include test
  fixtures). Spec G6 forbids every one of these in output.
- **Filters.** `filter.rs:77-90` `ALLOWED_FIELDS` includes `id`, `abs_path`, `created_at`,
  `updated_at`. The derived `document` table has none of these four.
- **CLI.** `codescout doc` (`src/cli/doc.rs`), `doc_augment`, `doc_event`, `doc_refresh`,
  `doctor`, `backfill_chunks`, `constitution_check`, `audit_doc_refs` all go through the catalog.
- **`audit_doc_refs`.** `resolver.rs:92-141` builds `live_artifact_ids` from the catalog and
  resolves 16-hex ids against it. The spec makes hex ids lint, not references.
- **Old link scanner.** `tools/link_scan/` still drives the catalog's edges; bug
  `fe65babc93a25084` says Plan E retires it.

Companion plugin consumers (`/home/marius/work/claude/claude-plugins`, read only):

| Consumer | What it reads | Evidence |
|---|---|---|
| `codescout-companion/hooks/goal-stop-hook.mjs` | `doc find` → `items[0].id` (hex); `doc get` → `augmentation.params.{status,criterion,blocked_reason}` and `augmentation.last_refreshed_at`; `doc event list` → a `note` whose payload tag is `gate_check` | `:64`, `:86`, `:92-101`, the `done` arm after `:107` |
| `hooks/constitution-guard.mjs`, `hooks/constitution-brief.mjs` | `codescout constitution-check` output; dedup key `${tracker_id}/${id}` | `constitution-guard.mjs:32`, `:65`; `constitution-brief.mjs:51` |
| `codescout constitution-check` (CLI, codescout repo) | `augmentation.params.rules` of active constitution trackers; emits `tracker_id` (hex) | `src/librarian/tools/constitution_check.rs:16-73`, `src/cli/constitution_check.rs:28-37` |
| `scripts/file-provenance.py` (codescout repo; used by `fmt-mine.sh`, a hook, CI) | `SELECT id, abs_path FROM artifact` read-only, to turn the hex ids in **past** transcripts into paths | `scripts/file-provenance.py:125-139`, `:448-462` |
| `hooks/pre-edit-dirty-check.mjs` | no catalog read; it names `file-provenance.py` in its advice | `:124` |
| `hooks/session-start.mjs` | a legacy sqlite drift check, marked inert | `:312-325` |

Count: four live consumers outside the server (goal-stop-hook, two constitution hooks through one
CLI, file-provenance). Only goal-stop-hook depends on catalog-only facts.

### Writers

| Writer | Order today | Evidence |
|---|---|---|
| `doc(create)` | file (exclusive create) then row | `read_door.rs` ALLOWED entry for `tools/create.rs` |
| `doc(update)` | file through `write_catalogued_file`, then row | `tools/update.rs:890-893`, `:1143` |
| `append_entry` | row and file in one old-catalog transaction (C1 dual-write); file restored on a failed commit | `catalog/dual_write.rs:120-183` (`write_entry_into_ledger`), `:355` |
| `update_entry` | params row first; the file changes only through a snapshot-row resync; returns `artifact_id` | `tools/update_entry.rs:71-121`, `catalog/augmentation.rs:2339-2340` |
| `event_create` | row in a transaction, then the text twin, only for a single-writer document; a `note` also inserts a catalog-only observation row | `tools/event_create.rs:568-655` (`observations::insert` at `:654`) |
| `doc(link)` | relation line plus `artifact_link` row (C2) | `tools/link.rs:208-251` |
| `doc(augment)` | catalog row (prompt, params, refresh fields); sidecar write-through for the shape only | `tools/augment.rs:65-120`, `augmentation_sidecar.rs:460` |
| `rekey_prefix` | file rewrite plus rows | `catalog/rekey.rs:510-513` |
| `doc(move)` | `fs::rename`, new row, graft | see § Move below |
| `doc(graft)`, `doc(delete)` | catalog only | `catalog/graft.rs`, `tools/delete.rs` |
| cutover `export` | file only, through the atomic write door | `cutover/apply.rs:155` |

The atomic write door is `util::write_catalogued_file` (`src/librarian/util.rs:131-139`). It is
file-safe already. Every catalogued text write goes through it.

### What the derived catalog lacks for the read flip

Derived schema v1 (`src/librarian/derived/schema.rs:40-117`): `checkout` (one row, no id),
`document` (primary key `rel_path`), `entry` (token and line span only), `edge`, `chunk`, `problem`,
`skipped`, `parse_cache`.

| Live query | Old source | Derived equivalent | Gap |
|---|---|---|---|
| `doc(find)` structured | `artifact` + `filter.rs` | `document` | no `created_at`/`updated_at` (needs the git-history cache), no `augmented` flag, no `id` (key instead) |
| `doc(find, semantic)` | `artifact_chunk` + `artifact_vec_v2` or Qdrant, keyed by `artifact_id` | `chunk` (`chunk_id = sha256(rel_path, ix, content_hash)`, `build.rs:405-415`) + `vector-cache.db` | `ArtifactVectorStore` is keyed by `artifact_id` (`artifact_store.rs:207-312`, `delete` `:242`, `refile` `:258`); no derived path embeds; the cache is empty for the existing corpus (C3 F-20) |
| `doc(get)` | row + file + augmentation + links + observations | `document` + file | no augmentation shape, no relation labels, no events, no observations |
| `doc(get, entry_filter)` | augmentation `params` rows | `entry` (token, lines) | no entry fields; the lifecycle reader parses fields from markdown (`lifecycle/`), the derived builder does not store them |
| `doc(graph)` | `artifact_link`, `entry_cite`, `event_edges` | `edge` | `edge` has no rel label (`build.rs:597-610`: kinds are resolution kinds only); no event edges |
| `doc(event_list)`, `state_at`, `librarian(timeline, workspace_state_at, context)` | `events`, `commits`, `event_edges` | none | no event table; `catalog/event_text.rs:303` (`parse_event_sections`) exists but the builder does not call it; no git-history cache |
| `doctor` | 47 checks (`doctor.rs:401-448`) | `problem` (key collisions and similar) | about 20 checks test catalog-only facts (path forms, `row_behind_file`, frontmatter id, params drift, worktree rows, retired objects); the rest need file-grain data the derived index has or can get. Classified by check name, not by reading each check |
| `list_stale`, `gather`, refresh | `artifact_augmentation.last_refreshed_at`, `refresh_count` | none | no committed home; the cutover does not export them (they appear only in a test fixture, `cutover/sidecars.rs:136-137`) |
| `scope=repo|umbrella|all` | one machine-global file, filtered by `abs_path` prefix | one `checkout` row per file | no multi-checkout query |
| read-after-write | the row is written in the same call | the index reads a commit only (`derived/scan.rs:113-122`) | `build.rs` test `a_working_tree_edit_that_is_not_committed_changes_nothing` (around `:2094-2107`) pins that an uncommitted edit changes nothing |

### `doc(move)` today, the overlay, the guard, audit

**Move** (`src/librarian/tools/mv.rs`, `call` at `:154-717`): `std::fs::rename` (`:372`); a new id
`artifact_id_from_abs(&new_full)` (`:389`); `repair_frontmatter_id` rewrites the stamped `id:`
(`:407`); vectors are re-filed in the store (`:486`); `graft_rows` folds the old row's history into
the new one (`:512`); the response carries `id`, `previous_id`, `id_changed`, `history_grafted`,
`vectors_refiled` (`:572-613`). The tool schema says "a move MINTS A NEW ID" (`tools/artifact.rs`).
Spec: a move that keeps the filename is a rename of the file plus a reindex; a rename changes a key
and rewrites every `[[old-key…]]`. The grammar-aware rename and rewrite already exist in
`cutover/rename.rs` (875 lines) and `cutover/rewrite.rs` (1,910 lines).

**Worktree overlay.** Shadow rows, `resolve_write_target`, `worktree_registration`
(`catalog/schema.sql:154`), `merge_worktree` and `fix=reseat_worktree` (`tools/librarian.rs:74`,
`:124`). The overlay names appear 337 times in 35 files (`doctor.rs` 101, `append_entry.rs` 38,
`catalog/mod.rs` 34, `worktree.rs` 29, `merge_worktree.rs` 28). Readers: every write tool that
calls `resolve_write_target` (for example `update_entry.rs:75`), `doctor` worktree checks, the
`librarian` action list.

**Guard.** `src/util/librarian_guard.rs:32-41` has three refusal reasons: `augmented` (an oracle the
librarian installs, `:71-73`, answering from `artifact_augmentation`), `ledger` (a declared
`entry_prefix`), and `stamped` (a frontmatter `id:`). Spec: keep only `ledger`. The sibling
`src/util/librarian_sync.rs:33-38` (`CatalogFrontmatterSync`) writes a catalog row after an
`edit_file` frontmatter change; it has no purpose once rows are derived.

**Audit.** Table `catalog_audit` and its triggers (`catalog/audit/mod.rs:213`, `:244-290`); shard
export to committed `.codescout/audit/*.jsonl` (`catalog/audit/shard.rs`); host name
(`catalog/audit/host.rs`); the `librarian(audit_log)` action (`tools/audit_log.rs:148-382`);
`tests/audit_shards_cross_machine.rs`; 13 references in `src/server.rs`. No script in either
repository reads the shards (grep for `.codescout/audit` in `*.py,*.sh,*.mjs`: only an
effort-steering test in claude-plugins and `tests/test_build_git_dirty.py`, not verified as
readers). The read-door allow-list names three audit reads (`read_door.rs:1062-1075`).

### Deletion tail inventory

- **Cutover entry points:** `src/cli/mod.rs:13`, `src/librarian/mod.rs:10`, `src/main.rs:285`,
  `src/main.rs:586`. Confirmed: no other code reference outside `src/librarian/cutover/` and
  `src/cli/cutover.rs`. Plus `tests/cli_cutover.rs` and the manual page.
- **Hidden coupling:** `catalog/augmentation.rs:7` and `catalog/cite_text.rs:11` import
  `derived::diff::{scope_of, Scope}`. `derived/diff.rs` cannot go before those two go.
- **Text writers that live under `catalog/` and must survive:** `catalog/event_text.rs`
  (`splice_event`, `parse_event_sections`), `catalog/link_text.rs`, `catalog/cite_text.rs`,
  `catalog/rekey.rs` (the spec keeps `rekey_prefix` as a file rewrite),
  `catalog/dual_write.rs::document_label` and `write_entry_into_ledger`. Deleting `catalog/` is a
  relocation first, then a delete.
- **Tables to drop with the file:** `artifact`, `artifact_link`, `artifact_observation`, `events`,
  `commits`, `sources`, `event_edges`, `artifact_augmentation`, `worktree_registration`,
  `catalog_meta` (`catalog/schema.sql:2-167`), `entry_cite`, `entry_reservation`,
  `artifact_chunk`, `artifact_vec_v2` (`catalog/mod.rs:335-438`), `catalog_audit`. The new binary
  stops opening the file; nothing needs a `DROP`.
- **Tool surface to remove:** `doc(graft)`; `librarian(merge_worktree)`, `librarian(audit_log)`;
  `doctor fix=` `prune_missing`, `reseat_worktree`, `rehome`, `repair_frontmatter_id`,
  `mint_slugs`, `export_augmentations` (`tools/librarian.rs:124`).
- **CLI to review:** `backfill_chunks` (old chunks), `derive-shadow` (compares with the old
  catalog), `cutover`.
- **Gate and dead-code tests that pin old behaviour:** `doctor.rs`
  `every_declared_check_is_scope_gated_or_a_named_exemption` (`:16836`) and
  `admits_relevance_exemption_allow_list_stays_exhaustive_over_check_all` (`:16972`) pin the check
  set; `embedding.rs:1138-1156` pins that every embedding site binds
  `VectorCache::beside(Some(&db_path))`; `read_door.rs:1421-1508` reds when an allow-listed read
  disappears; `derived/schema.rs:792-808` pins `derived.db` beside the old catalog; C3 left
  `transactions()` and `vector_cache()` behind `#[cfg_attr(not(test), allow(dead_code))]`.

### Multi-repository `derived.db`

What the spec says (§ The catalog is derived): "Storage is scoped by checkout. A checkout row holds
its root and repo name; every other row joins on the checkout and the key." § Tools that change
meaning: "`scope=repo|umbrella|all` stays, resolved over the checkouts that the name scan finds."

What the code does:

- `derived_catalog_beside(old)` puts `derived.db` in the directory of the old, machine-global
  catalog (`derived/schema.rs:12-19`). Its doc says this makes both share one `vector-cache.db`.
- `VectorCache::beside(path)` opens `vector-cache.db` in the parent directory of whatever catalog
  path it gets (`vector_cache.rs:265-272`).
- The schema has one `checkout` row and keys `document` on `rel_path` alone. It holds exactly one
  checkout.
- `cutover build` refuses a `derived.db` whose `checkout` row names another repository or a commit
  this repository does not have (`cli/cutover.rs:455-489` `existing_checkout`, refusal in
  `build_run` around `:519-537`). A worktree or a clone of the same repository rebuilds over it.
  So today one machine can hold one repository's derived index at a time, and two worktrees of
  one repository overwrite each other's index.
- `identity::checkouts::scan` (`identity/checkouts.rs:285-295`) already finds checkouts by
  `<dir>/.codescout/project.toml`, which is the name scan the spec names.
- Precedent: per-project derived databases already live in `.codescout/` and are git-ignored
  (`.gitignore:24-34` `embeddings.db`, `usage.db`, `librarian.db`; `:205-207` `code-index.db`).
  `memory::ensure_gitignored` (`src/memory/mod.rs:111`) adds an ignore line.

Layouts:

| Layout | Gain | Cost |
|---|---|---|
| **A.** One machine-global `derived.db`; a `checkout_id` column on every table (spec text, literally) | `scope=all` is one query; one `parse_cache` shared by worktrees (same blobs) | every primary key and every query gains a column; all sessions of all repositories share one WAL writer; a checkout removed from disk leaves rows behind (gc again); the B2 dump and its tests change |
| **B.** One `derived.db` per checkout under the data directory (`<data>/librarian/checkouts/<hash of root>/derived.db`) | schema v1 unchanged; no cross-repository lock contention | needs a registry from root to file; files of removed worktrees stay until a gc; a relocated checkout gets a new file |
| **C.** One `derived.db` per checkout at `<git root>/.codescout/derived.db`, git-ignored | schema v1 unchanged (one `checkout` row is already the right shape); the file dies with its worktree; a relocated checkout keeps it; the name scan that finds checkouts finds their catalogs, so no registry; matches the code index's own layout | writes a file into the repository (needs the ignore line in every repository; a read-only checkout cannot index); `scope=all` fans out over files; `vector-cache.db` must stop following the catalog path, or each checkout gets an empty cache |

**Recommendation: C, with `vector-cache.db` kept machine-global in the data directory.**
Confidence: medium-high. The spec sentence "every other row joins on the checkout" is satisfied by
the file itself: the file is the checkout scope. Change scenarios absorbed: a worktree is created
or removed (this repository has dozens), a checkout is moved, a second repository is cut over. Given
up: a single-query `scope=all`. That costs one fan-out loop over the checkouts the scan finds,
which the spec already prescribes. Revisit if `scope=all` latency over many checkouts becomes a
measured problem. The vector cache is keyed by content hash, so sharing it across checkouts is the
point of it (C3 follow-up 5 asked for exactly this decision).

## (b) Proposed decomposition

Principle for the order: readers flip before writers; writers go file-only before the overlay goes;
code is deleted only after nothing reads or writes through it. Until E6 the C1/C2 dual-write keeps
both stores in step, so a tool not yet flipped still reads correct rows.

**Activation.** E4 is the first production read of `derived.db`. A binary with E4 must not serve
sessions before the cutover has run for that repository. Recommendation: land E1..E8 on the
integration branch and release once, after the migration window (Q4). The gate stays green at every
step either way; activation is a release question, not a test question.

### E1 — Storage location and machine state

- Scope: `derived.db` per checkout at `<git root>/.codescout/derived.db` (layout C), with its
  ignore line; `vector-cache.db` decoupled from the catalog path into the data directory; the
  machine-state file (reindex progress rows, gc settings, embed-error marker, chunk backfill
  cursor; spec § Index inputs); `BuildInput` built only from a commit (B2 deferred finding, R21);
  the `open` doc says foreign databases are refused (B2); `cutover build` writes to the new place;
  bug `d372d3a36336b251` (`worktree_main_root` and submodules), which the bug file ties to
  checkout-scoped storage.
- Touches: `derived/schema.rs`, `vector_cache.rs`, `librarian/mod.rs:317-325`, `:618-622`,
  `embedding.rs`, `cli/cutover.rs`, the manual page `artifact-cutover.md`.
- Gate risks: `embedding.rs:1138-1156` pins `VectorCache::beside(Some(&db_path))` and must change
  in the same commit; the read-door gate if the machine-state file is read outside `read_door`;
  `schema.rs:792-808` pins the old location. Tool surface: none.
- Before it: Q2 answered.
- Deletes: `derived_catalog_beside`.
- Green because: nothing in production reads `derived.db` yet; only the cutover CLI and its tests
  move.

### E2 — Derived schema v2: the missing facts

- Scope: relation edges with their label (needs a canonical rel rule: bug `84e6e61e03beace6`; and
  the link format decision I-2, sorted insertion, which also fixes `133504302d03e590`); event and
  note sections through `parse_event_sections`, filtering the `**Mutates:**` line that
  `lifecycle::relations` reads as a relation (C2 precondition); augmentation shapes from sidecars;
  ledger entry fields (reuse the lifecycle reader, not a second parser); a git-history cache
  (created, last modified, status changes, authors); the working-tree layer if Q1 says so;
  `float_roundtrip` or compare event payloads by id (C2 precondition).
- Touches: `derived/{schema,build,incremental}.rs`, `catalog/event_text.rs` (reuse),
  `lifecycle/relations.rs` (reuse), `augmentation_sidecar.rs` (reuse).
- Gate risks: `PARSER_VERSION` and the pinned corpus digest; the read-door gate for working-tree
  and sidecar reads; the cap-name gate for any bound on the git walk (needs `// cap-class:`).
- Before it: Q1 answered; the rel rule and I-2 settled.
- Deletes: nothing.
- Green because: still no production reader; the B2 tests (a rebuild equals the incremental
  update, two clones dump equal) extend to the new tables.

### E3 — Key addressing, additive

- Scope: every `doc` action that takes `id`, `src_id`, `dst_id` also accepts a key or `[[key]]`,
  resolved through the derived index; output unchanged.
- Touches: `tools/artifact.rs` (schema text), the argument resolution shared by the doc verbs.
- Gate risks: `TOOL_SURFACE_CHAR_BUDGET` 57,664 has no headroom: the `id` description must be
  rewritten character-neutral or shorter (for example replace "document id (16-hex). No path
  form: find(rel_path=...) returns it."); four prompt surfaces and `ONBOARDING_VERSION` if a
  `source.md` slice changes.
- Before it: E1, E2.
- Deletes: nothing.
- Green because: hex ids keep working; the new form is an addition.

### E4 — Structured read flip and the consumers

- Scope: `doc(find)` structured, `doc(get)`, `doc(graph)`, `event_list`, `state_at`,
  `librarian(context, timeline, status, workspace_state_at, link_scan, audit_doc_refs)`,
  `constitution_check`, `goal_aggregation`, `codescout doc …` CLI. Output shows keys and grammar
  forms only (G6). `scope=repo|umbrella|all` fans out over the checkouts the scan finds. `doctor`'s
  file-grain checks read the derived index. The old link scanner retires (bug
  `fe65babc93a25084`). `audit_doc_refs` reports hex ids as lint. Companion side, shipped with it:
  `goal-stop-hook` reads `key`; constitution hooks need no change if the CLI emits a key in
  `tracker_id`; `file-provenance.py` resolves keys for new transcripts and keeps hex resolution for
  past ones (Q6).
- Touches: `tools/{find,get,graph,state_at,timeline,context,status,workspace_state_at,
  constitution_check,goal_aggregation}.rs`, `tools/link_scan/`, `tools/audit_doc_refs/`,
  `tools/doctor*`, `cli/doc.rs`, two repositories.
- Gate risks: tool-surface budget (descriptions of `find`, `get`, `graph`); prompt surfaces; the
  doctor check-set tests if a check is split; many tests assert hex ids in output (see the counts
  above).
- Before it: E3; Q3 answered (goal params, constitution rules); the cutover has run before any
  release of this binary (Q4).
- Deletes: the old link scanner's edge writer, `live_artifact_ids`.
- Green because: writes still dual-write, so the rows the unflipped tools read stay true; each
  flipped tool has a derived source from E2.

### E5 — Search and reindex flip

- Scope: `reindex` drives the derived incremental update and embeds derived chunks through the
  vector cache; `ArtifactVectorStore` keyed by `chunk_id` with the key in the payload, not by
  `artifact_id`; `doc(find, semantic)` reads it; vector seeding (C3 follow-up 1) or one
  `reembed=true` pass before the cutover rebuild; Qdrant collection naming.
- Touches: `tools/reindex.rs`, `embedding.rs`, `artifact_store.rs`, `tools/find.rs`,
  `src/retrieval/artifact.rs` (doc comment only).
- Gate risks: cap-name gate (batch sizes); `retired_vector_collection_present` doctor check;
  embedding tests.
- Before it: E1 (cache location), E4.
- Deletes: `refile` (vectors are content-keyed; a move needs no re-file).
- Green because: dual-write still keeps old chunks alive for anything unflipped; this step owns all
  remaining vector reads.

### E6 — Write flip: files only

- Scope: every write tool writes files and updates the derived index, and stops writing the old
  catalog: `append_entry` takes an advisory lock and allocates `max(body, frontmatter mark) + 1`
  (`entry_reservation` gone); `update_entry` patches the markdown entry, not params; `augment`
  writes the sidecar only and refuses params as a store; `event_create` writes the section (the
  shared-document case per Q3); `doc(link)` writes the line only; `doc(delete)` deletes a file;
  the old indexer stops. `doctor`'s catalog-only checks go (they would report a frozen file).
- Touches: `tools/{append_entry,update_entry,augment,event_create,link,update,create,delete}.rs`,
  `catalog/dual_write.rs` (callers), `indexer.rs`, `tools/doctor.rs`.
- Gate risks: doctor check-set tests; tool-surface budget (augment and params texts); prompt
  surfaces and guides (`librarian`, `tracker-conventions`).
- Before it: E4, E5 (nothing reads the old catalog any more); Q3 (the shared-document notes cycle).
- Deletes: the old-catalog side of every dual-write; `entry_reservation` use; the catalog-only
  doctor checks.
- Green because: no reader of the old catalog is left after E4 and E5.

### E7 — Move as a rename, overlay removal, guard replacement

- Scope: a move that keeps the filename is `rename` plus a reindex (no id, no graft, no re-file);
  a key rename rewrites every `[[old-key…]]` using the cutover's `rename.rs` and `rewrite.rs`,
  moved out of `cutover/`; `doc(graft)`, `fix=rehome`, `fix=prune_missing` removed; the overlay
  removed (`resolve_write_target`, shadow rows, `worktree_registration`, `merge_worktree`,
  `reseat_worktree`, the `append_entry` worktree refusal, the doctor worktree checks, the indexer's
  refusal of a linked worktree); the guard keeps only `entry_prefix`; `librarian_sync` removed or
  pointed at the derived index. Can split into E7a (move) and E7b (overlay and guard).
- Touches: `tools/{mv,graft,worktree,merge_worktree,librarian,append_entry}.rs`,
  `catalog/{graft,worktree}.rs`, `util/librarian_guard.rs`, `util/librarian_sync.rs`,
  `tools/edit_file`, `tools/markdown`, `indexer.rs`.
- Gate risks: tool-surface budget falls (the "MINTS A NEW ID" text, `graft`, `merge_worktree` and
  the `fix` enum go): lower the constant to the measured total with a log line; the doctor
  check-set tests; prompt surfaces and `ONBOARDING_VERSION`.
- Before it: E6 (the overlay exists to protect shared rows; with file-only writes it protects
  nothing). Q5 (who owns the rename tool).
- Deletes: graft, the overlay, two guard signals, `librarian_sync`.
- Green because: nothing writes shared rows any more, so removing the overlay removes no safety.

### E8 — Audit retirement and the deletion tail

- Scope: relocate the text writers out of `catalog/` (event, link, cite text, `rekey`,
  `document_label`, `write_entry_into_ledger`); delete `catalog/` (24,719 lines), the old indexer
  paths, `ids.rs`, `gc.rs`, `migrate_v6.rs`, `derive-shadow` with `derived/diff.rs` and
  `derived/old_catalog.rs`, the audit module and `audit_log`, `backfill_chunks` if it serves only
  old chunks, the doctor `fix=` modes, and — once the operator retires the old catalog — the
  cutover (`cutover/`, `cli/cutover.rs`, the four entry lines, `tests/cli_cutover.rs`, the manual
  page). Guides, manual pages, `PROBES.md` lines that name the old store.
- Gate risks: the read-door test reds unless the three audit entries (`read_door.rs:1062-1075`)
  leave in the same commit; the tool-surface floor (`rows.len() >= 15`) is unaffected (no tool is
  removed, only actions); prompt surfaces.
- Before it: E7; the migration window done; the operator retires `catalog.db` (Q6).
- Deletes: everything listed in § Deletion tail inventory.
- Green because: E4..E7 left no caller; the compiler finds any that remain.

## (c) Questions only the operator can answer

| # | Question | Recommendation | Cost of the recommendation |
|---|---|---|---|
| Q1 | After the flip, does a read see an uncommitted write? Today the derived index reads `HEAD` only, and a test pins it. | Yes: index `HEAD` plus a working-tree layer of changed `.md` files read through the read door; two clones agree on every clean tree. | G5 holds only for clean trees; one more input to the builder and its tests; per-write cost of a `git status` on `.md` paths (not measured). |
| Q2 | Where does each repository's `derived.db` live? | Layout C: `<git root>/.codescout/derived.db`, git-ignored; `vector-cache.db` machine-global. | Fan-out for `scope=all`; one ignore line per repository; a read-only checkout cannot index. |
| Q3 | The facts with no committed home that the flip would drop: goal-tracker params (`status`, `criterion`, `blocked_reason`), its refresh time and its `gate_check` note; constitution `rules` in params; `last_refreshed_at`/`refresh_count`; params keys outside entry collections (Plan D open item 4); notes on shared documents, which wait for codescout-37's entry-document tool, itself scheduled after Plan E. | Goal scalars to frontmatter; constitution rules to ledger entries in markdown; refresh time from the file's git history (a commit trailer) instead of a stored field; pull the entry-document tool (L-B2 part 1) before E6, or let E6 refuse `event_create` on a shared document with a remedy. | Two plugin readers change; `list_stale` becomes a git-log query; a cross-line dependency on the lifecycle session. |
| Q4 | Activation: may any E-step reach `experiments` (and so a `cargo rb` release) before the cutover runs? | No. Land E1..E8 on the integration branch; run the window; then the one merge. | A long-lived branch; conflicts with `experiments` grow. Alternative: a runtime refusal when the old catalog holds rows under this root and no cutover report exists. |
| Q5 | Who builds the key rename: identity E7, or codescout-37's move workflow (spec § Moves; lifecycle memory: "`doc(move)` as a key rename waits for identity Plan E")? | Identity E7, from the cutover's `rename.rs` and `rewrite.rs`; codescout-37 builds collection-aware moves on top. | Identity owns one more tool. |
| Q6 | When is `catalog.db` retired, and how does `file-provenance.py` resolve hex ids in past transcripts after that? | Retire after E8; `file-provenance` derives candidate ids from git history (`sha256(<root>/<old path>)[:16]`, the cutover's own method in `cutover/history.rs`) instead of reading the catalog. | One script change; ids of paths outside git history stay unknown. |
| Q7 | Between E3 and E8, is a hex id still accepted as input? | Yes, with a lint note naming the key; refused from E8. | Two input forms for a while. |
| Q8 | Link format I-2: sorted insertion of relation lines, and the canonical rel rule. | Sorted insertion; the label is the rel in snake_case. | Existing lines may reorder once. |

Top three: Q1, Q3, Q2.

## (d) What I could not measure

- No test, tool or command ran against the integration tree beyond `wc`, `grep` and `find`. No gate
  result is claimed.
- The 20 catalog-only doctor checks are classified by name, not by reading each check body.
- I did not read `tools/find.rs`, `tools/get.rs` or `tools/graph.rs` in full; the gap table rests on
  the schemas and the action list.
- Per-write cost of a working-tree layer (Q1) and of an incremental derived update under many
  concurrent sessions (WAL contention): not measured.
- Qdrant: whether two worktrees of one repository produce the same `chunk_id` for the same text
  (they will, by `build.rs:405-415`) and whether a delete by one checkout then removes a vector the
  other still uses. Not traced.
- Whether `git worktree remove` refuses a worktree that holds an ignored `derived.db`. I believe
  ignored files do not block it; not verified.
- Whether any plugin other than `codescout-companion` reads hex ids. The claude-plugins grep found
  mentions in `sdd/commands/review.md` and test fixtures, not read.
- The 38 params-only entries, 73 events, 57 observations and 13 write cites: not re-measured; the
  cutover's `plan` command measures them.
- Line numbers drift: `experiments` moves every few minutes. Find code by name.


## (e) Controller rulings, 2026-11-15

The operator said "continue with the plan" and earlier "you choose". I adopt the scout's recommendation for each question below. The operator may overrule any of them. Each costs what section (c) says, plus the note here.

- **Q1** Yes: the derived index covers `HEAD` plus changed `.md` files of the working tree, read through the read door. Extra cost: E2 owns the working-tree layer and must measure the per-write cost before E6 depends on it.
- **Q2** Layout C: `<git root>/.codescout/derived.db`, git-ignored; `vector-cache.db` machine-global. Extra cost: the Plan D guard (`build` refuses a `derived.db` of another repository) becomes unreachable in the new layout and goes in E1.
- **Q3** Goal scalars to frontmatter; constitution rules to markdown ledger entries; refresh time from git history; E6 refuses `event_create` on a shared document with a remedy unless codescout-37's entry-document tool has landed. Extra cost: plugin readers change in the same release as E4.
- **Q4** E1 to E3 are additive and may reach `experiments`. E4 to E8 stay on the integration branch until the operator runs the cutover window. Extra cost: the lifecycle fork merges integration into `experiments` more than once; each such merge must check that no E4+ commit rides along.
- **Q5** Identity E7 builds the key rename from the cutover's `rename.rs` and `rewrite.rs`.
- **Q6** `catalog.db` retires after E8; `file-provenance.py` derives candidate ids from git history.
- **Q7** A hex id is accepted as input between E3 and E8, with a lint note naming the key; refused from E8.
- **Q8** Sorted insertion of relation lines; the label is the rel in snake_case.

First sub-plan: E1. E2 and E3 are written after E1 lands, because E2's schema depends on the Q8 and I-2 decisions and on E1's builder input.
