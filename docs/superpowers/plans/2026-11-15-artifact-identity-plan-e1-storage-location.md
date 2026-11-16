---
id: '1acdb2caa8b83767'
kind: plan
status: draft
title: 'Artifact identity Plan E1: storage location and machine state'
tags:
- artifact-identity
- plan-e
- storage
topic: artifact-identity
---

# Artifact identity Plan E1: storage location and machine state Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `cutover build` writes the derived catalog of a checkout to `<git root>/.codescout/derived.db` (git-ignored), the vector cache is named by the machine and not by a catalog path, and `BuildInput` can only be built from a commit. Nothing in production reads `derived.db` after this plan, so it is safe to release before the cutover runs (Q4).

**Architecture:** The checkout owns its catalog file: `derived::schema::derived_catalog_in(git_root)` replaces `derived_catalog_beside(old_catalog)`. No other checkout uses that file, so the Plan D guard that refused another repository's `derived.db` in a shared directory is deleted. `.codescout/` is committed content, so `build` refuses a planted link, a tracked file and a file git does not ignore before it opens anything; it writes no `.gitignore`. The vector cache path comes from one function of the machine (`LibrarianEnv::machine_paths`), at the same location as today.

**Tech Stack:** Rust, `rusqlite`, `git2` 0.20 (`Repository::is_path_ignored`, `Repository::index`, `Repository::worktree`), `clap` help text, mdBook manual.

**Spec:** `docs/superpowers/specs/2026-11-11-repo-relative-artifact-key-design.md` (v5): § The catalog is derived, § Worktrees and machines, § Index inputs, guard, machine state and entry ids, § Cutover. Evidence: scout `docs/research/2026-11-15-artifact-identity-plan-e-scout.md` (`02d716631c64e393`) § E1 and § (e) rulings Q2, Q4; B2 deferred findings (`docs/research/2026-11-12-artifact-identity-plan-b2-rulings-and-deferred-findings.md`, R21 and "Plan E" list); Plan D plan (D-R4, D-R8) and state (`docs/research/2026-11-15-artifact-identity-plan-d-execution-state.md`); manual `docs/manual/src/concepts/artifact-cutover.md`; review of this plan `docs/research/2026-11-15-plan-e1-plan-review.md` (`e88898c6b6c948a4`). All line numbers below were read in `.worktrees/integration-identity-lifecycle` on 2026-11-15; find code by name if they drifted.

## Rulings (E1-R1..E1-R12; the operator may overrule any)

- **E1-R1** The new catalog is `<git root>/.codescout/derived.db`, where the git root is `lookup_git_root` of the start directory (`current_project.rs:55-63`: the nearest directory holding `.git`, so a linked worktree or a submodule gets its own root). Supersedes D-R4. Cost: a nested project (`--project` below the git root) shares its checkout's file; a checkout that cannot be written cannot build.
- **E1-R2** The ignore is checked, never written: `build` refuses unless git ignores `.codescout/derived.db` and its `-wal`, `-shm`, `-journal` (any source: `.gitignore`, the repository's `info/exclude`, `core.excludesFile`), and the refusal names the line `/.codescout/derived.db*`. This repository gets the line and its `crates/*/` twin. Cost: one manual line per other repository; E4 decides whether the server writes `info/exclude` once it builds on its own.
- **E1-R3** The other-repository guard is deleted, not ported: `existing_checkout` (`cli/cutover.rs:446-489`), the name-and-commit comparison (`:520-537`) and its side-file refusal. In layout C no other checkout uses the file, so a rebuild destroys nothing another checkout reads; the planted-link and tracked-file cases are E1-R12's. Cost: a `derived.db` copied in from elsewhere is rebuilt without a word (it is derivable); a killed build's `-wal` is recovered by SQLite instead of refused.
- **E1-R4** `export` still refuses an existing `derived.db` (D-R8 ordering: export, commit, build), at the new place. The text drops "another repository". Cost: unchanged from today.
- **E1-R5** `VectorCache::beside` is removed. The production site (`build_tool_context_with`) and the test-only `reindex_cli` open `LibrarianEnv::machine_paths().vector_cache`: `vector-cache.db` in the directory of `LIBRARIAN_DB` when it is set and has a parent, else `<data dir>/librarian`, else `/tmp/librarian`. Every location equals today's (this machine: `~/.local/share/librarian/vector-cache.db`) except `LIBRARIAN_DB` with no parent (`""`, `/`), where today's cache is in memory and the new one is in the data directory. (Final review: the earlier reason "the catalog cannot open at such a path anyway" is false for `""`: `Catalog::open("")` succeeds on a temporary database, so `LIBRARIAN_DB=` would run the server on a throwaway catalog beside the real machine cache; nothing in `src`, `tests`, `scripts` or `crates` sets it.) Cost: `LIBRARIAN_DB` still moves the cache (test isolation needs it); E8 must re-home the directory when `catalog.db` retires.
- **E1-R6** No machine-state file in E1. Its four facts live in the old catalog's `catalog_meta` and only old-catalog code writes them (`indexer.rs:1680` `BACKFILL_CURSOR`, `reindex_progress.rs:129-147`, `catalog/gc.rs:14-26`). E5 owns the machine-state file (reindex progress, embed-error marker, backfill cursor) and the spec amendment it needs: under layout C "next to the derived catalog" means inside each checkout, which does not fit machine-wide facts, so E5 states where the file goes (for example the `machine_paths` directory). E8 owns the gc settings (moved, per the spec) and the audit watermarks (retired with the audit tables). Cost: spec § machine state stays open until E5.
- **E1-R7** `BuildInput` fields become `pub(super)`; the one constructor is `BuildInput::from_commit(repo, commit, root)`, and `rules()` lends the rules to a caller outside `derived` (`derive-shadow` diffs with them after the build). All nine construction sites already take rules and key space from the same commit, so no behaviour changes. Closes B2 R21 and "`build_clone` gets its rules from a test helper". Cost: a test that wants other rules commits them.
- **E1-R8** No migration. On 2026-11-15 `~/.local/share/librarian/` holds no `derived.db` and no `cutover-report-*` file: the cutover has not run for real. The manual says a `derived.db` beside the old catalog is unused and can be deleted. Cost: a stale file on another machine stays until deleted.
- **E1-R9** A tripwire test in `tests/inline_gates.rs`: in production code, only `src/cli/cutover.rs` uses the checkout's catalog path (`src/librarian/derived/schema.rs` defines it). E1 may reach `experiments` (Q4), and this test turns an E4 commit riding along into a red gate. Cost: E4 edits the test as its first change.
- **E1-R10** Bug `d372d3a36336b251` (`worktree_main_root`) is not fixed here. Layout C never calls `worktree_main_root`; the bug's Resume line assumed a layout that needed the main root. Cost: the bug stays open; the controller corrects its Resume line.
- **E1-R11** Two builds in one checkout at once are left to SQLite (busy timeout 5 s, `BUSY_TIMEOUT_MS`, `derived/schema.rs:22`; one build transaction). Cost: a build that waits longer refuses with "cannot be opened" or "building the new catalog failed"; no lock file is added.
- **E1-R12** (review I-4, controller ruling) Before it opens anything, `build` refuses: a link at `<git root>/.codescout`; a link at `.codescout/derived.db` or its `-wal`, `-shm`, `-journal` (SQLite opens the side files by name too); a hard link on any of the four names, on Unix (`nlink() > 1`; the Task 3 review found that `symlink_metadata` does not see one; on other platforms the gap stays and the operator text says "on Unix"); a `.codescout` whose canonical path is not `<git root>/.codescout`; any of the four names in the git index. `is_path_ignored` matches patterns only, so it sees neither a link nor a tracked file. `export`'s existence check uses `symlink_metadata`, which already counts a link as existing, and export never writes the file, so export needs no change. E4 inherits the reader's half: a reader checks the `checkout.root` row against its own git root. Cost: a user who keeps `.codescout` as a link cannot build; the canonical check is a belt behind the link check (the git root is canonical because the start directory is canonicalized, `cli/cutover.rs:136-139`).

## Global Constraints

- `TOOL_SURFACE_CHAR_BUDGET` is exactly 57_664 (headroom 0). No MCP tool description or schema changes. `main.rs` help text is not tool surface.
- No prompt surface (`src/prompts/`, `.codescout/system-prompt.md`) changes; `ONBOARDING_VERSION` does not change.
- Q4: nothing in production reads `derived.db`. Only `cutover build` writes it and only `derive-shadow` opens a scratch one.
- Operator text (CLI help, refusals, manual) is STE and states the current situation only. It names no absolute path: the file is `.codescout/derived.db`, relative to the checkout.
- Read door: production code under `src/librarian` uses none of `fs::read`, `fs::read_to_string`, `fs::OpenOptions`, `File::open`, `File::options`, `OpenOptions::new`, `io::read_to_string`, `tokio::fs` (gate `no_librarian_production_read_bypasses_the_read_door`, `read_door.rs:1421`). `symlink_metadata`, `canonicalize`, `git2` and `rusqlite` opens are not read tokens. Test modules are blanked.
- Cap-name gate: no new constant name contains `CAP`, `LIMIT`, `MAX`, `BUDGET`, `THRESHOLD`.
- `.gitignore`: every root `/.codescout/…` rule has a `crates/*/.codescout/…` twin (`every_root_codescout_ignore_rule_has_its_crates_twin`, `tests/hook_config.rs:710-760`).
- A test that asks git (libgit2 or the `git` CLI) whether a path is ignored first sets the repository's local `core.excludesFile` to an empty file: libgit2 and git read `~/.config/git/ignore` when it is unset, and `GIT_CONFIG_GLOBAL=/dev/null` does not stop that.
- Command forms (checked against cargo on 2026-11-15: `--doc` cannot be mixed with `--lib`, and `cargo test` takes one positional filter): lib tests `scripts/with-slot.sh cargo test --lib -- <filter> [<filter>…]`; doctests `scripts/with-slot.sh cargo test --doc <filter>`; integration tests `scripts/with-slot.sh cargo test --test <name> [-- <filter>…]`. Implementers do not run `./scripts/gate.sh`; Task 5 does.
- Commit named paths only (`docs/conventions/shared-checkout-commit-sequence.md`).

## Review Focus

1. **A repository that does not ignore `.codescout/derived.db` and its side files.** Expect `build` to refuse before it creates the file, name the line to add, and accept `info/exclude` as well as `.gitignore`. Test: Task 3 `build_refuses_when_git_does_not_ignore_the_new_catalog`.
2. **A linked worktree and a clone beside the main checkout.** Expect each to build its own file and leave the others byte-identical. Test: Task 3 `every_checkout_builds_its_own_catalog_and_leaves_the_others_unchanged`.
3. **A checkout that cannot be written.** Expect a refusal (code 2) that says the directory must be writable and names no path. Test: Task 3 `build_in_a_checkout_it_cannot_write_refuses_and_names_no_path`.
4. **A planted link or a tracked file in `.codescout/`.** Expect a refusal and no file written anywhere. Tests: Task 3 `build_refuses_a_linked_codescout_directory_and_writes_nothing_outside`, `build_refuses_a_linked_catalog_file_and_writes_nothing_outside`, `build_refuses_a_tracked_catalog_file_and_leaves_it_unchanged`, `the_catalog_directory_must_be_the_checkouts_own`. A killed build's leftover `-wal` rebuilds (E1-R3): `build_over_a_catalog_with_a_leftover_side_file_rebuilds`.
5. **A test or operator that sets `LIBRARIAN_DB`.** Expect the vector cache beside that file, never in the real data directory and never in a checkout (two checkouts share one cache). Test: Task 2 `the_vector_cache_lives_in_the_machine_directory_whatever_catalog_is_opened`.

## Scout corrections

- `vector-cache.db` already lives in the data directory in production (`librarian/mod.rs:323` puts it beside the machine-global `catalog.db`). E1 changes the API that names it, not its location.
- The machine-state facts cannot move in E1 without changing the old reindex before E5 flips it (E1-R6). Scout § E5 has no machine-state item and scout § E8 deletes `gc.rs`, while the spec moves gc settings; E1-R6 assigns both.
- Bug `d372d3a36336b251` is not tied to layout C (E1-R10).
- The scout's open point "does `git worktree remove` refuse an ignored `derived.db`?" is answered by bug `docs/issues/2026-11-12-git-worktree-remove-refuses-…` § Fix item 1: ignored runtime files show as `!!` and a plain `git worktree remove` succeeds.
- Gate risks the scout did not list: the `.gitignore` twin gate above; the export and build tests in `cli/cutover.rs` that write or assert `f.catalog_dir.join("derived.db")` (`:976-1863`), the JSON key `"location"`, and `tests/cli_cutover.rs:466-517`.
- Codescout ensures no ignore line in other repositories for its other `.codescout/` databases; only private memories do (`MemoryStore::ensure_gitignored`, `memory/mod.rs:111`). E1-R2 follows the refusal route.

## Review corrections

Every finding of `e88898c6b6c948a4` was checked against the code or with a scratch run before this revision. Accepted: I-1, I-2 (a scratch crate gave `error: can't mix --doc with other target selecting options` and `error: unexpected argument 'bar' found`; `cargo test --lib -- foo bar` and `cargo test --doc foo` run), I-3 (`derive_shadow.rs:239` uses `rules` after the build; `build.rs:193` passes `input.space`), I-4 (E1-R12; `write_guard.rs:313-345` treats `.codescout/` as committed content), I-5, M-1 (`tests/inline_gates.rs:20-30` measured the "first `mod tests {`" rule false, and `src/librarian/mod.rs` is one of the five files), M-2, M-3 (`vector_cache.rs:155-158` `create_dir_all`), M-4 (`reindex_cli` is `#[cfg(test)]`, `librarian/mod.rs:604`), M-5 (`open_lock_file` creates `.codescout/`, `write_guard.rs:331-332`), M-6 (the `git` helper sets `GIT_CONFIG_GLOBAL=/dev/null` only, `tests/cli_cutover.rs:31-48`), M-7, M-8, M-9 (`cli/cutover.rs:1570-1571`), M-10. One part not taken: I-4 asks for the link check in `export_run` too; `export` never writes `derived.db` and its `symlink_metadata` check already refuses a link (E1-R12).

## File map

- Modify `src/librarian/derived/build.rs` (`BuildInput`, `rebuild`, module doc line 21), `incremental.rs` (test helper `with_input`), `testkit.rs` (`build_files`), `diff.rs` (test `built_entries`), `src/cli/derive_shadow.rs` (`compare`) — Task 1.
- Modify `src/librarian/derived/schema.rs` (`open` doc, location API, tests) — Tasks 1, 2, 3.
- Modify `src/librarian/mod.rs` (`MachinePaths`, `build_tool_context_with`, `reindex_cli`, two tests), `src/librarian/vector_cache.rs` (`beside`, two tests, `VECTOR_CACHE_FILE` doc), `src/librarian/embedding.rs` (guard test) — Task 2.
- Modify `src/cli/cutover.rs` (export, build, tests) — Tasks 1, 3. Modify `tests/inline_gates.rs` (tripwire) — Task 3.
- Modify `tests/cli_cutover.rs`, `.gitignore` (`:85` and the twin block `:47-84`), `src/main.rs` (`Commands::Cutover` help), `src/librarian/cutover/mod.rs` (doc), `docs/manual/src/concepts/artifact-cutover.md` — Task 4.

---

### Task 1: `BuildInput` only from a commit

**Files:**
- Modify: `src/librarian/derived/build.rs:75-83` (struct), `:185-195` (`rebuild`), `:21` (doc), test helpers `build_commit` (`:704-720`), `build_clone` (`:1762-1778`), `build_order_does_not_change_the_dump` (`:1971-1985`), `derived_index_of_this_repo` (`:2171-2187`)
- Modify: `src/librarian/derived/incremental.rs:529-544`, `testkit.rs:250-268`, `diff.rs:1116-1139`, `src/cli/derive_shadow.rs:204-239`, `src/cli/cutover.rs:516-548`
- Modify: `src/librarian/derived/schema.rs:129-131` (`open` doc)

**Interfaces:**
- Produces: `impl<'a> BuildInput<'a> { pub fn from_commit(repo: &'a git2::Repository, commit: &'a git2::Commit<'a>, root: &'a Path) -> anyhow::Result<Self>; pub fn rules(&self) -> &[CompiledRule] }`. Fields `repo`, `commit`, `root` stay references; `rules: Vec<CompiledRule>` and `space: KeySpace` become owned; all five are `pub(super)` (read by `build.rs`, `incremental.rs::change_counts` `:434-436`, and the `diff.rs` test). `rules_from_commit` and `key_space_from_commit` stay `pub` (`cutover/plan.rs:198` and tests use them).

- [ ] **Step 1: Write the guard doctests** on `BuildInput`: a ` ```compile_fail ` block that builds `codescout::librarian::derived::build::BuildInput { repo: &repo, commit: &commit, root, rules: Vec::new(), space: todo!() }` from outside the crate (no import beyond `BuildInput`, so the only error is E0451, a private field), and a ` ```no_run ` twin with the same setup lines that calls `BuildInput::from_commit(&repo, &commit, root)?` and `.rules()`. The twin proves the setup compiles. Mutation: make the fields `pub` → the `compile_fail` doctest compiles and reds.
- [ ] **Step 2: Run** `scripts/with-slot.sh cargo test --doc BuildInput`. Expected: the `no_run` twin fails to compile (`from_commit` does not exist).
- [ ] **Step 3: Implement `from_commit`** (calls `rules_from_commit` then `key_space_from_commit`; errors pass through unchanged) and `rules()`, and replace the nine struct literals with `from_commit`. `rebuild` (`:193`) passes `&input.space` to `scan_commit`; `rebuild_from` passes `&input.rules`. In `build_order_does_not_change_the_dump`, `scan_commit` takes `&input.space`. `derive_shadow::compare` passes `input.rules()` to `diff_with_rules` (no second read of the blob). `diff.rs::built_entries` moves `input.rules` into `Built` after the build (an in-module field move). Module doc line 21: "`checkout`: one row; `rules_source` is always `committed+builtin`, because `BuildInput::from_commit` is the only way to build an input." Remove imports clippy reports unused.
- [ ] **Step 4: `open` doc.** `schema.rs:129-131` adds: "and refused when it is another program's database (`user_version` 0 with a table in it)". No code change.
- [ ] **Step 5: Run** `scripts/with-slot.sh cargo test --doc BuildInput`, then `scripts/with-slot.sh cargo test --lib -- derived:: cli::`. Expected: PASS, no test count lost.
- [ ] **Step 6: Commit** `refactor(derived): BuildInput is built only from a commit (E1-R7)`.

### Task 2: The vector cache is named by the machine

**Files:**
- Modify: `src/librarian/mod.rs:317-325` (`build_tool_context_with`), `:615-622` (`reindex_cli`, `#[cfg(test)]`), module `librarian_env_tests` (`:160`), test `the_service_a_reindex_runs_on_carries_a_vector_cache_beside_the_catalog` (`:943`)
- Modify: `src/librarian/vector_cache.rs:17` (doc), `:266-273` (`beside`), tests `an_unused_cache_creates_no_file` (`:654-665`), `the_cache_file_sits_beside_the_catalog_file` (`:667-681`)
- Modify: `src/librarian/embedding.rs:1131-1162` (`every_production_construction_site_attaches_the_vector_cache_with_its_model`)
- Modify: `src/librarian/derived/schema.rs:790-808` (delete `the_new_catalog_sits_beside_the_old_one_and_shares_its_vector_cache`; its premise is false by E1-R5, and Task 3 adds the new location test)

**Interfaces:**
- Produces: `pub struct MachinePaths { pub catalog: PathBuf, pub vector_cache: PathBuf }`; `pub(crate) fn machine_paths(db: Option<&Path>, data_dir: Option<PathBuf>) -> MachinePaths`; `impl LibrarianEnv { pub fn machine_paths(&self) -> MachinePaths }` (calls the free function with `self.db.as_deref()` and `dirs::data_local_dir()`). E5 consumes `vector_cache`.

- [ ] **Step 1: Write the failing test** in `librarian_env_tests`:

```rust
// mutation: take the cache from the data dir even when `db` has a parent; drop the `/tmp` fallback.
#[test]
fn the_vector_cache_lives_in_the_machine_directory_whatever_catalog_is_opened() {
    let p = machine_paths(Some(Path::new("/m/lib/catalog.db")), Some(PathBuf::from("/d")));
    assert_eq!((p.catalog, p.vector_cache), (PathBuf::from("/m/lib/catalog.db"), PathBuf::from("/m/lib/vector-cache.db")));
    let p = machine_paths(None, Some(PathBuf::from("/d")));
    assert_eq!((p.catalog, p.vector_cache), (PathBuf::from("/d/librarian/catalog.db"), PathBuf::from("/d/librarian/vector-cache.db")));
    assert_eq!(machine_paths(None, None).vector_cache, PathBuf::from("/tmp/librarian/vector-cache.db"));
    assert_eq!(machine_paths(Some(Path::new("catalog.db")), None).vector_cache, PathBuf::from("vector-cache.db"));
    // No parent at all: the data directory, never a path made from the catalog.
    assert_eq!(machine_paths(Some(Path::new("/")), Some(PathBuf::from("/d"))).vector_cache, PathBuf::from("/d/librarian/vector-cache.db"));
}
```

- [ ] **Step 2: Run** `scripts/with-slot.sh cargo test --lib -- the_vector_cache_lives_in`. Expected: FAIL, `machine_paths` not defined.
- [ ] **Step 3: Implement** `machine_paths`: the directory is `db.parent()` when that is `Some` (an empty parent stays empty), else `data_dir.unwrap_or("/tmp").join("librarian")`; `catalog` is `db` when given, else `<dir>/catalog.db`; `vector_cache` is `<dir>/VECTOR_CACHE_FILE`. In `build_tool_context_with` and in `reindex_cli`: `let paths = env.machine_paths();`, then `let cache = std::sync::Arc::new(vector_cache::VectorCache::open(&paths.vector_cache));` on one line (it fits 100 columns, so rustfmt leaves it), then `let db_path = paths.catalog;`. Delete `VectorCache::beside`. `VECTOR_CACHE_FILE` doc: "File name of the cache, in the machine's librarian directory (`LibrarianEnv::machine_paths`)".
- [ ] **Step 4: Update the guards.** Embedding guard: `bound` counts `VectorCache::open(&paths.vector_cache)`; message: "every site opens the machine's cache (`paths.vector_cache`), never a path made from a catalog it opens". Mutation: put `VectorCache::open(&db_path.with_file_name("vector-cache.db"))` at one site → `bound != sites`. `an_unused_cache_creates_no_file` uses `VectorCache::open(&tmp.path().join(VECTOR_CACHE_FILE))`. Replace `the_cache_file_sits_beside_the_catalog_file` with `a_cache_in_a_directory_that_does_not_exist_creates_it`: `VectorCache::open(&tmp.path().join("sub").join(VECTOR_CACHE_FILE))`, `put`, the file exists (pins `open_connection`'s `create_dir_all`; mutation: drop it). Rename `the_service_a_reindex_runs_on_carries_a_vector_cache_beside_the_catalog` to `…_in_the_machine_directory` (its assertions hold as written). Delete the schema test named above.
- [ ] **Step 5: Run** `scripts/with-slot.sh cargo test --lib -- vector_cache the_vector_cache_lives_in every_production_construction_site`. Expected: PASS.
- [ ] **Step 6: Commit** `refactor(librarian): the vector cache path comes from the machine, not a catalog path (E1-R5)`.

### Task 3: `cutover build` writes the checkout's own catalog

**Files:**
- Modify: `src/librarian/derived/schema.rs:11-19` (location API), tests module
- Modify: `src/cli/cutover.rs`: module doc `:7-10`, import `:39`, `CutoverAction::Build` doc `:55-58`, `export_run` `:314-322`, `evaluate_build` doc `:398-404`, `open_refusal` `:430-445`, delete `existing_checkout` `:446-489`, `build_run` `:516-566`; tests listed in the steps
- Modify: `tests/inline_gates.rs` (one test)

**Interfaces:**
- Consumes: `BuildInput::from_commit` (Task 1).
- Produces: `pub const DERIVED_CATALOG_REL: &str = ".codescout/derived.db";` and `pub fn derived_catalog_in(checkout_root: &Path) -> PathBuf` in `derived/schema.rs`. Removed: `DERIVED_CATALOG_FILE`, `derived_catalog_beside`. In `cli/cutover.rs`: `const IGNORE_LINE: &str = "/.codescout/derived.db*";` (used by the refusal through `format!`), `const CATALOG_NAMES: [&str; 4]` (the file and its three side files, relative to the git root), `fn refuse_unsafe_catalog_path(repo: &git2::Repository, git_root: &Path) -> Result<()>`, `fn catalog_dir_is_the_checkouts(git_root: &Path, canonical_dir: &Path) -> bool`, `fn refuse_unless_ignored(repo: &git2::Repository) -> Result<()>`.

- [ ] **Step 1: Fixtures.** `fixture_with` commits `(".gitignore", FILE, b"/.codescout/derived.db*\n")`. `derived(f)` opens `f.root.join(DERIVED_CATALOG_REL)`. Every `f.catalog_dir.join("derived.db")` in the build and export tests becomes `f.root.join(DERIVED_CATALOG_REL)`, except the assertions that the old directory holds none (below). The comment at `:1570-1571` ("an open reader keeps a `-wal` beside the file, which a build refuses") becomes "the reader is closed first, so the JSON run reads a closed file".
- [ ] **Step 2: Write the failing tests.**
  - `schema.rs`: `the_new_catalog_is_in_the_checkouts_codescout_directory`: `derived_catalog_in(Path::new("/r"))` equals `/r/.codescout/derived.db`. Mutation: return the old directory.
  - `tests/inline_gates.rs` (E1-R9): `only_the_cutover_uses_the_checkouts_catalog_path`. For each file of `tracked_src_files()`, take `lex(&src).code` and `excise_test_items(..)` (the file's production region with every test-gated item cut wherever it sits); collect the files whose region contains `derived_catalog_in(` or `DERIVED_CATALOG_REL`. Assert the set is exactly `{"src/cli/cutover.rs", "src/librarian/derived/schema.rs"}` (non-vacuity: both are in it). Doc line: blind spots are a test-only file whose name lacks `tests` (`derived/testkit.rs`, which names neither) and a path built from the literal text. Mutation: call `derived_catalog_in` from `build_tool_context_with` in `src/librarian/mod.rs` (production code there sits after an inline test module, which the excision handles).
  - `build_writes_the_new_catalog_from_head` (`:1486`): the text says `the new catalog is .codescout/derived.db in this checkout`; `f.root.join(DERIVED_CATALOG_REL).is_file()`; `!f.catalog_dir.join("derived.db").exists()`; JSON `v["catalog"] == ".codescout/derived.db"` and `v.get("location").is_none()`. Mutation: build beside the old catalog.
  - `export_refuses_when_the_new_catalog_exists` (`:978`): the file is written at `f.root/.codescout/derived.db`; the text contains `the new catalog (.codescout/derived.db) already exists in this checkout, and export runs before it is built. Delete it (git can rebuild it), then run export again`. A second case: a `derived.db` in `f.catalog_dir` does not stop export (code 0). Mutation: check the old directory.
  - Review Focus 1, `build_refuses_when_git_does_not_ignore_the_new_catalog`: after `exported_and_committed`, set the repository's local `core.excludesFile` to an empty file and delete the working-tree `.gitignore`. (a) No rule: code 2, the text contains `/.codescout/derived.db*` and `info/exclude`, `!f.root.join(".codescout/derived.db").exists()`, `no_absolute_path`. (b) The repository's `info/exclude` holds `/.codescout/derived.db` (no star, so the `-wal` is not ignored): code 2, same text, no file. (c) The line becomes `/.codescout/derived.db*`: code 0. Mutations: skip `refuse_unless_ignored` (a reds); check only `derived.db` and not the side files (b reds).
  - Review Focus 2, replacing `build_over_the_new_catalog_of_a_clone_of_the_same_repository_rebuilds`: `every_checkout_builds_its_own_catalog_and_leaves_the_others_unchanged`. Build in `f.root`; record the sha256 of its `derived.db`. Add a linked worktree with `git2::Repository::worktree("wt", <fresh tempdir>/wt, None)` and a clone in another tempdir (both beside `f.root`, never inside it: `RootMap::with_old_roots` refuses an old root above the git root). Build each with `old_root: vec![f.root.clone()]`. Each gets `<its root>/.codescout/derived.db` whose `checkout.root` is its own canonical root; the first file's sha256 is unchanged. Mutation: write to the main checkout's root for a worktree.
  - Review Focus 3, `#[cfg(unix)] build_in_a_checkout_it_cannot_write_refuses_and_names_no_path`: `exported_and_committed`, `create_dir_all(f.root/.codescout)` (export's lock already made it), chmod it `0o555`; if a probe write into it succeeds (running as root), return early. Build: code 2, the text contains `.codescout/derived.db cannot be created or opened: check that this checkout's .codescout directory is writable`, `no_absolute_path`. Restore `0o755` before the tempdir drops. Mutation: drop the `CannotOpen` arm of `open_refusal`.
  - Review Focus 4 (E1-R12), each after `exported_and_committed`, each asserting code 2, `no_absolute_path`, and that no `derived.db` was created anywhere:
    - `#[cfg(unix)] build_refuses_a_linked_codescout_directory_and_writes_nothing_outside`: replace `f.root/.codescout` with a link to a fresh tempdir; the text contains `.codescout in this checkout is a link`; the tempdir stays empty. Mutation: drop the directory link check.
    - `#[cfg(unix)] build_refuses_a_linked_catalog_file_and_writes_nothing_outside`: `f.root/.codescout/derived.db` is a dangling link into a fresh tempdir (and, a second case, `derived.db-wal` is); the text contains `is a link`; the link target does not exist afterwards. Mutation: check only the file and not its side files (second case reds).
    - `build_refuses_a_tracked_catalog_file_and_leaves_it_unchanged`: write bytes to `f.root/.codescout/derived.db`, stage it with `Index::add_path` and `Index::write`; the text contains `is tracked by git`; the bytes are unchanged. Mutation: drop the index check (the ignore check alone passes: libgit2 matches patterns only).
    - `the_catalog_directory_must_be_the_checkouts_own`: `catalog_dir_is_the_checkouts(Path::new("/r"), Path::new("/r/.codescout"))` is true; `Path::new("/elsewhere/.codescout")` and `Path::new("/r/sub/.codescout")` are false. Mutation: return `true`.
  - Review Focus 4, replacing `build_refuses_a_new_catalog_with_a_side_file_beside_it`: `build_over_a_catalog_with_a_leftover_side_file_rebuilds`: `schema::open` the checkout's file, drop it, write `b"x"` to `derived.db-wal`; build: code 0 and `count(&derived(&f), "document") == 4`. Mutation: restore the side-file refusal.
  - Delete `build_refuses_another_repositorys_new_catalog_and_leaves_it_unchanged` (`:1665`), `build_refuses_an_unrelated_repositorys_new_catalog_though_both_are_named_repo` (`:1717`), and `checkout_name`, `fixture_named`, `fixture_with`'s `extra` parameter if clippy finds them unused. Keep `build_over_a_new_catalog_with_no_build_in_it_builds`, `a_second_build_rebuilds_in_place` and `build_refuses_a_file_that_is_not_a_new_catalog_and_names_no_path` at the new path (its text becomes `.codescout/derived.db is not a new catalog`).
- [ ] **Step 3: Run** `scripts/with-slot.sh cargo test --lib -- cli::cutover derived::schema` and `scripts/with-slot.sh cargo test --test inline_gates -- only_the_cutover_uses`. Expected: the new and changed tests FAIL (no `derived_catalog_in`, wrong location, no checks).
- [ ] **Step 4: Implement.**
  - `schema.rs`: replace `DERIVED_CATALOG_FILE` and `derived_catalog_beside` with `DERIVED_CATALOG_REL` and `derived_catalog_in(root) = root.join(DERIVED_CATALOG_REL)`, doc: "The derived catalog of the checkout at `checkout_root`: one file per checkout, git-ignored (E1-R1, E1-R2)".
  - `refuse_unsafe_catalog_path`, in this order: `symlink_metadata(<git root>/.codescout)` is a link → `.codescout in this checkout is a link, and build writes only inside the checkout. Replace the link with a directory, then run this again`; when `.codescout` exists, `catalog_dir_is_the_checkouts(git_root, &canonicalize(..))` is false → `.codescout in this checkout resolves to another directory; build writes only inside the checkout`; any of `CATALOG_NAMES` whose `symlink_metadata` is a link → `{name} is a link, and build does not write through a link. Remove it, then run this again`; any of `CATALOG_NAMES` in `repo.index()?.get_path(name, 0)` → `{name} is tracked by git, and build would overwrite a committed file. Run git rm --cached {name}, commit, then run this again`.
  - `refuse_unless_ignored`: `repo.is_path_ignored` for each of `CATALOG_NAMES`; any `false` refuses with `format!("git does not ignore .codescout/derived.db and its side files in this checkout, and build writes them there. Add the line {IGNORE_LINE} to .gitignore or to the repository's info/exclude (git rev-parse --git-path info/exclude prints it), then run this again")` (in a linked worktree `info/` is in the common git directory); a `git2` error refuses with `git cannot tell whether .codescout/derived.db is ignored in this checkout`.
  - `build_run`: after the `HEAD` re-check, `refuse_unsafe_catalog_path(&repo, &loaded.git_root)?` then `refuse_unless_ignored(&repo)?`; `let new_catalog = derived_catalog_in(&loaded.git_root);` then `schema::open`, then `build_full(&mut conn, &BuildInput::from_commit(&repo, &commit, &loaded.git_root)?)`. Delete `existing_checkout` and the guard block. Success text and JSON per the test; drop `"location"`.
  - `export_run`: check `symlink_metadata(derived_catalog_in(&loaded.git_root))`, text per the test.
  - `open_refusal`: name `DERIVED_CATALOG_REL` in every arm; add `Some(ErrorCode::CannotOpen)` with the text of Review Focus 3.
  - Docs: module doc `:7-10`, `CutoverAction::Build` (`build the new catalog (.codescout/derived.db in this checkout, git-ignored) from HEAD`), `evaluate_build` doc and `build_run` doc lose "in the directory of the old catalog" and the other-repository sentence.
- [ ] **Step 5: Run** the Step 3 commands. Expected: PASS. Then `scripts/with-slot.sh cargo clippy --workspace --all-targets -- -D warnings` for dead helpers.
- [ ] **Step 6: Commit** `feat(cutover): build writes .codescout/derived.db in its checkout; refuse links, tracked and unignored files; the other-repository guard goes (E1-R1..R4, R9, R12)`.

### Task 4: End to end, the ignore line and the operator texts

**Files:**
- Modify: `tests/cli_cutover.rs` (`git` helper `:31-48`, `make_repo` `:76-114`, build block `:457-512`)
- Modify: `.gitignore` (beside `/.codescout/diagnostic-*.log` `:86`, and the `crates/*/` twin block `:47-84`)
- Modify: `src/main.rs:260-284`, `src/librarian/cutover/mod.rs:15-16`, `docs/manual/src/concepts/artifact-cutover.md` (lines 39, 106-109, 273-308, 322-332)

**Interfaces:**
- Consumes: Task 3 texts, `DERIVED_CATALOG_REL`.

- [ ] **Step 1: Write the e2e changes.** `make_repo` sets local `core.excludesFile` to an empty file in the repository's tempdir (the binary's libgit2 and the `git` CLI then ignore `~/.config/git/ignore`) and adds `.gitignore` = `/.codescout/derived.db*\n` to commit two. A `git_stdout(dir, args) -> String` helper runs with the `git` helper's environment and returns stdout. Before the commit: `assert!(!root.join(".codescout/derived.db").exists())` replaces `:466`. After build: `built["catalog"] == ".codescout/derived.db"`; `root.join(".codescout/derived.db").is_file()`; the listing beside the old catalog equals `dir_before` plus the four report files (no `derived.db`); `git_stdout(&root, &["status", "--porcelain"])` is empty. The derived-row checks open `root.join(".codescout/derived.db")`. Mutation: drop the `.gitignore` line from `make_repo` (build refuses with code 2).
- [ ] **Step 2: Run** `scripts/with-slot.sh cargo test --test cli_cutover`. Expected: PASS; apply the mutation once and see it red, then revert it.
- [ ] **Step 3: `.gitignore`.** Add `/.codescout/derived.db*` after `/.codescout/diagnostic-*.log` with one comment line ("the derived catalog of this checkout: built by `codescout cutover build`, derivable from git") and `crates/*/.codescout/derived.db*` in the twin block, sorted. Run `scripts/with-slot.sh cargo test --test hook_config -- every_root_codescout_ignore_rule_has_its_crates_twin`. Expected: PASS; it reds if the twin is missing.
- [ ] **Step 4: Operator texts.** `main.rs` `Commands::Cutover`: `build` builds `.codescout/derived.db` in this checkout (git must ignore it: add `/.codescout/derived.db*`; a link or a tracked file there is refused); remove the side-file and other-repository refusals of `build`; `export` refuses an existing `.codescout/derived.db` "because export runs before the build". `cutover/mod.rs:15`: "builds `.codescout/derived.db` in the checkout". Manual: the code block comment `# .codescout/derived.db appears in this checkout`; § build location, the ignore refusal and its line, the link and tracked-file refusals, the `CannotOpen` refusal; delete the side-file and other-repository bullets and "Do not cut over a second repository before Plan E…" (each checkout has its own file); the vector-cache paragraph says the cache is `vector-cache.db` in the machine's librarian directory, shared by every checkout; one line: "A `derived.db` beside the old catalog is unused; delete it." The refusal list at the end matches the code. STE, no history words.
- [ ] **Step 5: Run** `scripts/with-slot.sh cargo test --test cli_cutover`. Expected: PASS. (`scripts/gate.sh` does not build the manual; read the changed manual sections once in a markdown preview.)
- [ ] **Step 6: Commit** `docs(cutover): .codescout/derived.db per checkout; ignore line; e2e (E1)`.

### Task 5: Gate and the Q4 check (controller)

- [ ] **Step 1:** `./scripts/gate.sh` in the E1 branch. Expected: green. Read the coverage-limit lines it prints.
- [ ] **Step 2:** `grep -rn "derived_catalog_in\|DERIVED_CATALOG_REL\|schema::open(" src --include=*.rs` lists only `src/cli/cutover.rs`, `src/cli/derive_shadow.rs` (scratch file), `src/librarian/derived/schema.rs` and test modules. This, with the E1-R9 tripwire, is the Q4 evidence for releasing E1 before the cutover.
- [ ] **Step 3:** Record E1 in the Plan E execution state: E1-R1..R12 and D-R4 superseded; E4's duties from E1-R12 and the final review: (a) a reader checks `checkout.root` against its own git root, since a moved checkout or a copied file carries a stale root; (b) the reader opens `<lookup_git_root(start)>/.codescout/derived.db`, never the project root's `.codescout` (a nested `--project` shares its git root's file); (c) any E4 writer runs `refuse_unsafe_catalog_path` and `refuse_unless_ignored` before it opens the file, and both move from `cli/cutover.rs` into `librarian::derived` when E4 becomes the second writer; (d) the non-unix hard-link gap carries over to any writer; (e) the E1-R9 tripwire is edited first; (f) the fields of `BuildInput` are guarded against a crate-wide `pub(crate)` widening only by a text test in `build.rs`, so an E4 change to them edits that test deliberately; E5 owns the machine-state file and its spec amendment, E8 owns the gc settings and audit watermarks (E1-R6); the Resume line of bug `d372d3a36336b251` corrected (E1-R10).

---

## Self-review

- **Spec coverage.** § The catalog is derived, "storage is scoped by checkout": Task 3 (the file is the checkout scope; one `checkout` row). § Worktrees and machines, "a worktree is a checkout … built from its own files": Task 3 Review Focus 2. "Embedding vectors are a cache keyed by content hash": Task 2 keeps one machine cache for every checkout. § Index inputs "the index reads the commit's files and nothing else": Task 1 (rules and key space only from the commit). § machine state: assigned by E1-R6. § Cutover step 4, "a new catalog file; the old file is kept read-only": Task 3 keeps `build`'s refusals of the old catalog unchanged. B2 Plan E items: R21 and `build_clone` (Task 1), the `open` doc (Task 1). Scout E1 "deletes `derived_catalog_beside`": Task 3.
- **Types.** `BuildInput::from_commit(&Repository, &Commit, &Path) -> anyhow::Result<BuildInput>` and `BuildInput::rules(&self) -> &[CompiledRule]` (Tasks 1, 3); `MachinePaths { catalog, vector_cache }` (Task 2); `DERIVED_CATALOG_REL`, `derived_catalog_in` (Tasks 3, 4, 5); `CATALOG_NAMES`, `IGNORE_LINE` (Task 3).
- **Proportion.** About 70 lines of production code change and 20 tests; the rest is text the operator reads.
