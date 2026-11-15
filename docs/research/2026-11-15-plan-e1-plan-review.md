---
id: e88898c6b6c948a4
kind: research
status: draft
title: Plan E1 plan review (storage location)
tags:
- artifact-identity
- plan-e
- review
topic: artifact-identity
---

# Plan E1 plan review (storage location)

Independent review of `docs/superpowers/plans/2026-11-15-artifact-identity-plan-e1-storage-location.md` (`1acdb2caa8b83767`, uncommitted), before any implementer sees it. Code read in `.worktrees/integration-identity-lifecycle` at `0529d8274` (equal to `experiments`). Every claim below cites what I read or ran. Scratch experiments ran in the session scratchpad, not in the repository.

**Verdict: ready after fixes** (I-1 to I-5; the minors are cheap and should go in the same pass).

## Findings

| # | Sev | Plan section | Evidence | Correction |
|---|---|---|---|---|
| I-1 | Important | Task 1 Steps 2 and 5; Global Constraints (targeted tests) | `cargo test --doc --lib <filter>` is refused by cargo: `error: can't mix --doc with other target selecting options` (ran in a scratch crate). | Use `scripts/with-slot.sh cargo test --doc BuildInput`. |
| I-2 | Important | Task 2 Step 5; Task 3 Step 3 (and Step 5 reuses it) | `cargo test --lib foo bar` is refused: `error: unexpected argument 'bar' found` (scratch crate). `cargo test` takes one positional TESTNAME. | `scripts/with-slot.sh cargo test --lib -- the_vector_cache_lives_in every_production_construction_site` and `... cargo test --lib -- cli::cutover derived::schema` (libtest takes several filters after `--`). |
| I-3 | Important | Task 1 Step 3, Interfaces (`pub(super)` fields) | `cli/derive_shadow.rs:207-239`: `rules` is used AFTER the build, `diff_with_rules(&old, &conn, &roots, &rules)`. With owned `pub(super)` fields, `src/cli/` cannot read `input.rules`. Also `build.rs:193` (`rebuild`) passes `input.space` to `scan_commit` and needs `&input.space`; the plan names only `rebuild_from`. `diff.rs:1134-1138` moves `rules` into `Built` (in-module partial move; compiles, but unstated). | Add `pub fn rules(&self) -> &[CompiledRule]` on `BuildInput` (one source, no second read of the blob) and use it in `compare`; or say explicitly that `compare` calls `rules_from_commit` again. Name `rebuild` (`:193`) and the `diff.rs` move in Step 3. |
| I-4 | Important | E1-R3, Architecture paragraph ("nothing left to protect"), Task 3 Step 4 | `.codescout/` is committed content: `agent/write_guard.rs:330-345` opens `write.lock` with `O_NOFOLLOW` for exactly this reason; `identity/repo_name.rs:535` and `identity/key_space.rs:316` refuse a `.codescout` linked out of the repository. `schema::open` (`derived/schema.rs:132-142`) follows links (`refuse_if_foreign`, `:213`, notes SQLite resolves them). libgit2 `git_ignore_path_is_ignored` (`ignore.c:533-600`, libgit2 1.9.2) matches patterns only: it does not see a link and returns true for a tracked file. So a committed link at `.codescout` or `.codescout/derived.db` lets `build` rebuild another checkout's `derived.db` (the F1 case: one file, two repositories) or create a file outside the checkout; a tracked `derived.db` is overwritten and the tree goes dirty. Layout C removes the shared-directory path, not this one. | In `build_run` (and `export_run`'s existence check), refuse when `symlink_metadata` of `<root>/.codescout` or of the leaf is a link, or when the canonical parent is not `<git root>/.codescout`; refuse when the index tracks `.codescout/derived.db` (`repo.index()?.get_path(..)`). Add `build_refuses_a_linked_catalog_or_codescout_dir_and_writes_nothing_outside` (mutation: drop the link check). Record an E4 obligation: a reader compares `checkout.root` with its own git root (a moved checkout or a copied file carries a stale root). |
| I-5 | Important | Review Focus 1 test `build_refuses_when_git_does_not_ignore_the_new_catalog` | The stated mutation "check only `derived.db` and not `-wal`" survives: the test uses "no rule" and then `/.codescout/derived.db*`, which treat all four names alike. | Add a case: `info/exclude` holds `/.codescout/derived.db` (no star) → code 2, text names `/.codescout/derived.db*`, no file created. |
| M-1 | Minor | E1-R9 tripwire `only_the_cutover_names_the_checkouts_catalog_path` | Its parse ("lines before the first `mod tests {`") is the rule `tests/inline_gates.rs:22-29` measured false (five files carry production code after an inline test module); files declared `#[cfg(test)] mod x;` (`derived/testkit.rs`) would count as production. The stated mutation is caught today (`librarian/mod.rs`: `build_tool_context_with` at `:311`, first `mod tests {` at `:766`). | Reuse `embedding.rs`'s `production_lines` (same rule, one place) or `read_door.rs`'s `production_code`; note the blind spot in the test doc. |
| M-2 | Minor | Task 3 Step 4 refusal text | The text names `.git/info/exclude`; in a linked worktree `.git` is a file and `info/` is a common-dir item (`repository.c:60`), so the file is the main checkout's `.git/info/exclude`. | Say "the repository's `info/exclude` (`git rev-parse --git-path info/exclude`)". |
| M-3 | Minor | Task 2 Step 4 (delete `the_cache_file_sits_beside_the_catalog_file`) | `vector_cache.rs:667-681` is the only test where the cache's parent directory does not exist; it pins `open_connection`'s `create_dir_all` (`:160-163`). The new pure-path test does not cover it. | Keep one case: `VectorCache::open(&tmp/sub/VECTOR_CACHE_FILE)`, `put`, file exists. |
| M-4 | Minor | Task 2 Step 3 `machine_paths` | `db` with no parent (`LIBRARIAN_DB=""` or `/`) is unspecified; today `beside` returns an in-memory cache (`vector_cache.rs:268-273`). Also "both construction sites": `reindex_cli` is `#[cfg(test)]` (`librarian/mod.rs:604-605`), so production has one. | State the no-parent rule (e.g. data dir) and test it; reword "both sites". |
| M-5 | Minor | Review Focus 3 test | `.codescout/` already exists after `exported_and_committed`: export takes the lock, and `open_lock_file` creates `.codescout/write.lock` (`write_guard.rs:347-349`). | Use `create_dir_all`, then chmod. |
| M-6 | Minor | Task 4 Step 1 (e2e) | The `git status --porcelain` call needs the isolation of the `git` helper (`tests/cli_cutover.rs:31-48`). The binary's libgit2 reads `~/.config/git/ignore` when `core.excludesFile` is unset (`attrcache.c:429-430`), so the mutation "drop the line from `make_repo`" is vacuous on a machine whose global ignore covers `.codescout/` (this machine's does not). | Run `git status` through the helper's env; give the binary `XDG_CONFIG_HOME`/`HOME` in a tempdir or set local `core.excludesFile`. |
| M-7 | Minor | Spec fit, E1-R6, Task 5 Step 3 | Spec § Index inputs… says the machine-state file sits "next to the derived catalog"; under layout C that is inside each checkout, which does not fit machine-wide gc settings. Scout E5 scope (`02d716631c64e393` § E5) has no machine-state item; scout E8 deletes `gc.rs` while the spec says gc settings move. Task 5 Step 3 records only the E5 deferral. | Record in the execution state: E5 owns reindex progress, embed-error marker, backfill cursor and the spec amendment (machine state in the `machine_paths` directory); E8 owns gc settings (moved, per spec) and audit watermarks. |
| M-8 | Minor | Task 3 Step 4 `IGNORE_LINE` | A `const` the refusal literal does not use is dead code; clippy runs with `-D warnings`. | Build the refusal with `format!` from `IGNORE_LINE`. |
| M-9 | Minor | Task 3 tests | Comment at `cli/cutover.rs:1570-1571` ("an open reader keeps a `-wal` beside the file, which a build refuses") becomes false. | Update or drop it. |
| M-10 | Minor | File map / Task 1 | `build.rs:21` (module doc) is listed but no step says what changes. `.gitignore` twin block is `:47-84`, not `:47-72`. Task 4 Step 2 "Expected before Task 3: FAIL" cannot happen (Task 4 runs after Task 3). Test `the_service_a_reindex_runs_on_carries_a_vector_cache_beside_the_catalog` (`librarian/mod.rs:944`) stays green but names the retired rule. | Fix the references; optionally rename the test. |

## Claims verified as correct

- Nine `BuildInput` struct literals, all in `src/` (none in `tests/`): `build.rs:712, 1770, 1978, 2179`, `incremental.rs:537`, `testkit.rs:260`, `diff.rs:1125`, `derive_shadow.rs:228`, `cli/cutover.rs:542`. Each takes rules and key space from the same commit. No code outside `librarian::derived` reads a field, apart from I-3.
- `compile_fail` idea: in a scratch crate, a `compile_fail` struct literal on a `pub(super)` field reports compile-fail OK and its `no_run` twin compiles; `librarian`, `derived`, `build` are `pub` (`lib.rs:51`, `librarian/mod.rs:24`, `derived/mod.rs:3`). The gate runs doctests: `scripts/gate.sh:247` and `:253` (`cargo test --workspace --doc`, both feature sets). No `#![doc(test(attr(deny(warnings))))]` in `lib.rs`.
- `git2` 0.20.4 / libgit2-sys 0.18.3+1.9.2 (`Cargo.lock`): `Repository::is_path_ignored<P: AsRef<Path>>(&self, P) -> Result<bool>` (`repo.rs:3077`); it walks parent directories (an ignored `.codescout/` ignores the file, `ignore.c:564-592`); reads `info/exclude` from the common dir and `core.excludesFile` with the XDG fallback. `Repository::worktree(name, path, opts)` exists (`repo.rs:3285`). `~/.config/git/ignore` here is 31 bytes and names no codescout path.
- A linked worktree and a submodule get their own root (`current_project.rs:55-63`), so their `.codescout/` is their own.
- `VectorCache::beside` callers: `librarian/mod.rs:323` (production), `:621` (test-only `reindex_cli`), tests `vector_cache.rs:657, 671, 676, 678`, `schema.rs:800-801`. Today's paths equal the plan's `machine_paths` table, including `Some("catalog.db")` → `vector-cache.db`. The embedding guard (`embedding.rs:1131-1162`) counts substrings over all of `librarian/mod.rs`; the new one-line binding fits in 100 columns.
- Q4: production opens `derived.db` only in `cli/cutover.rs:539` (build) and `derive_shadow.rs:224` (scratch). No other `schema::open`, `open as open_index` or `super::schema` use in production.
- E1-R8: `~/.local/share/librarian/` holds no `derived.db` and no `cutover-report-*`.
- A leftover 1-byte `-wal` beside a cleanly closed WAL database: SQLite opens, writes and leaves no side file (Python `sqlite3` in the scratchpad). Review Focus 4 is sound.
- Review Focus 3: `schema::open` turns a failed `create_dir_all` into `SQLITE_CANTOPEN` (`schema.rs:132-142`), and an unwritable existing directory gives CANTOPEN from SQLite.
- `RootMap::with_old_roots` refuses an old root above the git root (`old_catalog.rs:124-142`).
- `tests/hook_config.rs:710-760`: every root `/.codescout/<tail>` rule needs a `crates/*/.codescout/<tail>` twin; probe `derived.dbx` matches `derived.db*`. The plan adds the twin.
- `TOOL_SURFACE_CHAR_BUDGET = 57_664` (`server.rs:6625`). The read-door gate (`read_door.rs:1421`) scans `src/librarian` only; nothing E1 adds there reads a file. No new constant name is cap-shaped.
- Other names of the old location: `main.rs:264-284`, `librarian/cutover/mod.rs:15`, the manual page (`:39`, `:106-109`, `:273-308`, `:322-332`), `cli/cutover.rs` and `tests/cli_cutover.rs:466-517`. The JSON key `"location"` appears only in `build_run`. No CLI-help snapshot test.

## Spec fit

E1 fits § The catalog is derived (the file is the checkout scope; one `checkout` row), § Worktrees and machines (each worktree builds its own file) and § Cutover step 4 (a new file; the old one is only read). It does not contradict them. The open point is the machine-state sentence (M-7). The guard deletion is safe for the shared-directory case Plan D guarded, provided I-4 closes the link case.

Ledger checked: bug `d372d3a36336b251` (open; E1-R10 leaves it). No session-log entry written: this review is the record.
