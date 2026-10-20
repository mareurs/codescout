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
test `every_schema_sql_artifact_column_survives_every_migration_path`
(`src/librarian/catalog/migrate_v6.rs`), which loops the columns `parse_create_table_columns`
(`src/librarian/catalog/mod.rs`, unit-tested by
`parse_create_table_columns_extracts_artifact_columns`) parses out of `SCHEMA_SQL` against
every legacy-seed path; regression precedent
`migration_v6_single_open_preserves_v9_entry_graph_shape` (`migrate_v6.rs`). Still current —
both test files and `drop_legacy_and_stamp` exist as named.

## User Strings in LIKE Patterns Must Be Wildcard-Escaped

Any user/data string interpolated into a SQL `LIKE` pattern must escape `%`, `_`, and the
escape char, with an explicit `ESCAPE` clause — else `%`/`_` in the input act as wildcards
(e.g. `resolve_cite_ref("foo_bar.md")` also matching `fooXbar.md`).

**Rust side — settled, use the helper.** Call `crate::librarian::util::escape_like_pattern`
and pair it with an `ESCAPE` clause; never re-inline the triple-replace. The helper carries
5 unit tests plus a DRY gate, `like_escape_idiom_is_not_inlined_outside_helper`, that fails
if the Rust idiom appears anywhere but the helper itself. Live callers:
`src/librarian/filter.rs` (`compile_leaf`, 4 call sites), `src/librarian/catalog/augmentation.rs`
(`resolve_cite_ref`), `src/librarian/catalog/gc.rs` (`detect_move_candidates`,
`plan_rehome`, `rehome_commits`) and `src/librarian/reindex_progress.rs` (`read_all`). The
extraction and its gate **shipped**, and the bug file that asked for them is closed —
`docs/issues/archive/2026-07-17-like-escape-idiom-duplicated-no-shared-helper.md`. Earlier
point fixes: `4b922ac4` (worktree `covering()`), and the Stage-2 `resolve_cite_ref` bug that
motivated the extraction.

**SQL side — FIXED, no longer a second unguarded implementation.** Where the *haystack*
column is escaped rather than the needle (`src/librarian/catalog/worktree.rs`
`covering_conn` escapes a per-row **column**, which `escape_like_pattern` can't reach since
it only escapes a Rust-held value), the same law used to be expressed as a nested
triple-`REPLACE` inlined verbatim at four sites — `src/librarian/tools/merge_worktree.rs`
(×2), `src/librarian/catalog/worktree.rs`, `src/librarian/tools/worktree.rs` — held together
only by "mirrors" comments and unreachable by the Rust-idiom gate (SQL string literals don't
match what it greps for). **Extracted as `descendant_path_like(root_expr)`**
(`src/librarian/util.rs`, beside `escape_like_pattern`) — closed as SD-2 in
`docs/trackers/structural-debt-refactor.md` at `experiments:31609aa5`. The duplicated unit
turned out larger than first described — the `|| '%' ESCAPE` tail is shared too, so the whole
strict-descendant predicate moved, not just the triple-replace. Guarded by its own DRY gate,
`sql_descendant_like_is_not_inlined_outside_helper` (sibling of the Rust-side gate — one law,
two spellings, needed two greps), plus a characterization test,
`descendant_path_like_reproduces_the_pre_extraction_sql_exactly`, pinning the exact
pre-extraction SQL byte-for-byte so the refactor is provably behaviour-preserving.
Nested-`REPLACE` occurrences in `src` went 5 → 1 (the helper itself; the test that pins its
SQL spells it a second time). Full record: `structural-debt-refactor.md` SD-2 and SD-5.

**Current call sites of `descendant_path_like` are TWO**, not four: `catalog/worktree.rs`
(`covering_conn`) and `tools/worktree.rs` (`shadow_main_pairs`). The "is this row at or under
this root" queries in `tools/merge_worktree.rs` (`call`), `tools/doctor.rs` (`prune_dead_root`,
`count_dead_root`) and `indexer.rs` (`index_repo_sync`) no longer use `LIKE` at all: they use
`crate::librarian::util::path_at_or_under_sql(col, root_param, prefix_param)` — byte-exact
`col = ?root OR substr(col, 1, length(?prefix)) = ?prefix` — with the two bound values from
`root_and_prefix(root)` (root without trailing `/`, and root plus one `/`). The reason is a
hazard escaping does not cure: SQLite `LIKE` is ASCII-case-insensitive, so `.worktrees/feat_1`
also matched the rows of `.worktrees/FEAT_1` (and `abandon` deleted them with their cascade).
For a new "under this root" query on paths, prefer `path_at_or_under_sql`; `LIKE` plus the escape
helpers is only right when case-insensitivity is genuinely wanted. Pinned by
`path_at_or_under_sql_is_exact_for_windows_shaped_paths`,
`path_at_or_under_sql_counts_characters_for_a_non_ascii_root` and
`root_and_prefix_normalizes_one_trailing_separator` (all in `util.rs`).

## `length()` on TEXT counts CHARACTERS, not bytes

`length(params)` is a character count; `length(CAST(params AS BLOB))` is the byte count.
Measured 2026-08-31 on one augmentation row: **25,455 characters / 25,521 bytes** — a 66-byte
gap from multi-byte punctuation, which is exactly the size of discrepancy someone will read
as evidence of a truncating write. Six wrong "N bytes" claims in one session came from this,
one of which reached a committed tracker before it was caught. Say which unit you measured,
or report both.

## Auditing `usage.db` — two traps that return a number rather than an error

Both measured 2026-08-31 while trying to prove a cross-machine task had not written to the
wrong host.

- **`called_at` is UTC.** A window built from local wall-clock time silently selects the
  wrong hours. On a UTC+2 host that is a two-hour shift — enough to make a task's own calls
  look absent and another task's look like yours.
- **A query containing a host's IP SELF-MATCHES.** Searching the call log for
  `'%192.168.1.162%'` to find `ssh` calls also matches the audit query you just ran, because
  that query is itself logged with the IP in its arguments. The count is therefore always at
  least one, and the extra row looks exactly like a real call.

And the boundary that makes both worse: **`usage.db` records only MCP calls.** Work done
through native `Bash` is invisible to it, so a task that ran entirely on shell tools audits
as *zero activity* — indistinguishable from a task that did nothing. When a brief names
`usage.db` as the audit path for host-safety, substitute target-side forensics instead:
`artifact.updated_at`, the event log's authorship, and `git reflog` on the remote checkout.
