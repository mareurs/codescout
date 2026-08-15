# Catalog SQL & Migration Hazards

Discovered the hard way during entry-graph Stage 2 (2026-07-17). Companion to
`gotchas`'s SQLite / link-graph entries — kept as its own topic so these two
hazards are easy to recall before writing new catalog SQL or a migration.

## Table-Copy Migration Drops Later-Added Columns

A table-copy migration — `CREATE TABLE X_new (...)` / `INSERT INTO X_new SELECT ... FROM X`
/ `DROP TABLE X` / `ALTER TABLE X_new RENAME TO X` — hardcodes its column list at the time
it was WRITTEN. If a LATER-ordered migration adds a column to X, the copy silently drops it,
and X's indexes are only whatever the copy recreates. codescout hit this when the v9 block
added `artifact.slug` BEFORE `migrate_v6::drop_legacy_and_stamp`
(`src/librarian/catalog/migrate_v6.rs`) rebuilt `artifact` without carrying `slug` →
`ux_artifact_slug` gone + `entry_cite` FK dangling for one open. It self-heals on the NEXT
open, so a twice-opening idempotency test does NOT catch it.

**Rule:** a table-copy migration must carry EVERY column the live schema declares and
recreate EVERY index. When you add a column, grep the migrations dir for table rebuilds of
that table before treating the add as isolated. **Mechanical guard:** the schema-invariant
test that loops `SCHEMA_SQL`'s declared columns against every legacy-seed path
(`src/librarian/catalog/mod.rs`); regression precedent
`migration_v6_single_open_preserves_v9_entry_graph_shape` (`migrate_v6.rs`).

## User Strings in LIKE Patterns Must Be Wildcard-Escaped

Any user/data string interpolated into a SQL `LIKE` pattern must escape `%`, `_`, and the
escape char, with an explicit `ESCAPE` clause — else `%`/`_` in the input act as wildcards
(e.g. `resolve_cite_ref("foo_bar.md")` also matching `fooXbar.md`).

**Rust side — settled, use the helper.** Call `crate::librarian::util::escape_like_pattern`
and pair it with an `ESCAPE` clause; never re-inline the triple-replace. The helper carries
5 unit tests plus a DRY gate, `like_escape_idiom_is_not_inlined_outside_helper`, that fails
if the Rust idiom appears anywhere but the helper itself. Live callers:
`src/librarian/filter.rs` (`compile_leaf`), `src/librarian/catalog/augmentation.rs`
(`resolve_cite_ref`), and `src/librarian/catalog/gc.rs` (`detect_move_candidates`,
`plan_rehome`, `rehome_commits`). The extraction and its gate **shipped**, and the bug file
that asked for them is closed —
`docs/issues/archive/2026-07-17-like-escape-idiom-duplicated-no-shared-helper.md`. Earlier
point fixes: `4b922ac4` (worktree `covering()`), and the Stage-2 `resolve_cite_ref` bug that
motivated the extraction.

**SQL side — a second implementation, still unguarded.** Where the *haystack* column is
escaped rather than the needle, the same law is expressed as a nested triple-`REPLACE`
inside the SQL string — see `src/librarian/catalog/worktree.rs` (`covering_conn`). It is
verbatim at four sites: `src/librarian/tools/merge_worktree.rs` (×2),
`src/librarian/catalog/worktree.rs`, `src/librarian/tools/worktree.rs`. The Rust gate
excludes this form **by design** (SQL string literals cannot match the Rust call signature it
greps for), so nothing enforces it — the sites are held together by "mirrors" comments. All
four are currently correct. Tracked as SD-2 in
`docs/trackers/structural-debt-refactor.md`.
