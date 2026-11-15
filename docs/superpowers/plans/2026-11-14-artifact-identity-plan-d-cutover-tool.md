# Artifact identity Plan D: the cutover tool Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** One operator command, `codescout cutover`, moves every catalog-only fact of the old catalog into committed files, renames non-canonical files that nothing outside the markdown reads, rewrites citations into the grammar, and reports every fact it does not move. After the operator commits, the same command builds the new derived catalog beside the old one. It never writes a row into the old catalog.

**Architecture:** A pure planner, `cutover::plan::plan_export`, reads committed blobs of `HEAD` and read-only rows of the old catalog, and composes every change of every file in memory (an `ExportPlan`). `cutover plan` prints the plan as a report and writes nothing. `cutover export` writes it through `write_catalogued_file` (atomic), one write per file. The operator commits. `cutover build` builds `derived.db` beside the old catalog from `HEAD`, but only when a plan of `HEAD` writes nothing.

**Tech Stack:** Rust, `rusqlite` (read-only `immutable=1` URI open), `git2` (blobs, revwalk, rename detection), `pulldown-cmark` 0.13 (through the two existing scanners), `serde_json`, `serde_yml`, `toml`, `url` (file URI), `clap`.

**Spec:** `docs/superpowers/specs/2026-11-11-repo-relative-artifact-key-design.md` (v5): § The store boundary (table, single-writer rule), § Identity: the key, § The citation grammar, § Moves and renames, § Contract with ledger formats, § Index inputs, guard, machine state and entry ids, § Cutover (steps 1 to 4), § Left to the builder, § Decision log. Evidence: scout `docs/research/2026-11-14-artifact-identity-plan-d-scout.md` (id `489339eef5f9e93b`); state records of C1 (`9bcc00c179ecbdce`), C2 (`docs/research/2026-11-14-artifact-identity-plan-c2-execution-state.md`) and C3 (`512e01311fe5d7bb`); review of the first draft: session log `abff63149e3c24e8`, F-25 to F-29 and W-13. This revision applies all of them.

## Global Constraints

- `TOOL_SURFACE_CHAR_BUDGET` is exactly 57_664 (headroom 0, `src/server.rs:6625`). The cutover is a CLI subcommand. `tool_surface_under_budget` sums `advertised_surface` (MCP tools only), so `main.rs` help text does not count. No advertised tool description or schema changes in this plan. If one must change, it is character-neutral.
- Operator-facing text (CLI help, report, refusals) and the one new manual page use STE (short sentences, one idea each, active voice) and state the current situation only: no "was", "formerly", "now".
- No absolute path in any text the cutover prints or writes into its report. Paths are repo-relative. The old catalog is "the old catalog"; the report names its own file and says "in the directory of the old catalog". `NoOldRowsUnderRoot` loses its absolute path too (Task 1).
- No prompt surface (`src/prompts/`, `.codescout/system-prompt.md`) changes, so `ONBOARDING_VERSION` does not change. The tool's OUTPUT never edits a prompt surface either: every file that code reads by path is frozen (D-R26).
- Read door: production code under `src/librarian` uses none of `fs::read`, `fs::read_to_string`, `fs::OpenOptions`, `File::open`, `File::options`, `OpenOptions::new`, `io::read_to_string`, `tokio::fs` (`path_reads`, `src/librarian/read_door.rs:1326-1337`; gate `no_librarian_production_read_bypasses_the_read_door`, `:1414`). A working-tree file is read with `ReadDoor::new(vec![git_root]).text(path)` (`read_door.rs:168`). Committed text comes from git objects (`scan::scan_commit`, `scan::blob_text`, `git2` tree walks). `Path::exists`, `symlink_metadata`, `create_dir_all`, `rename` and `rusqlite` opens are not read tokens. Tests are inline `#[cfg(test)] mod tests`, which the gate blanks.
- Cap-name gate: no constant name contains `CAP`, `LIMIT`, `MAX`, `BUDGET` or `THRESHOLD` unless the line above carries `// cap-class: NOT_A_CAP — <why>` (`is_cap_shaped`, `tests/result_caps.rs:680`; `every_cap_constant_is_classified`, `:1224`). Stage a new file before that gate runs (it lists `git ls-files src`).
- Every repository file write goes through `crate::librarian::util::write_catalogued_file(roots, path, text)` (`src/librarian/util.rs:131`): symlink refusal, outside-repository refusal, `refuse_unwritable_existing`, then `atomic_write`. It creates no directory.
- No row, table, `-wal`, `-shm` or `-journal` of the old catalog is ever written or created (spec § Cutover). The old catalog is opened only by `open_old_immutable` (Task 1), and its safety rests on the side-file refusal (W-13: an immutable open reads stale data without an error when rows lie only in the `-wal`).
- The tool never runs `git commit`, `git add` or `git mv` (D-R2).
- Targeted tests: `scripts/with-slot.sh cargo test --lib <filter>`; integration tests: `scripts/with-slot.sh cargo test --test <name>`. Implementers do not run `./scripts/gate.sh`; the controller runs it in Task 10.
- Shared checkout: stage named paths only, run `git diff --cached --name-only` as its own call, read `git diff --cached`, then `git commit -m "..." -- <paths>` (`-m` before `--`; `docs/conventions/shared-checkout-commit-sequence.md` § 4). Touch only the files a task names.

## File map

- Modify `src/librarian/derived/diff.rs`: `RootMap`, `open_old_immutable`, `side_files_present`, `OldCatalog::connection`, `NoOldRowsUnderRoot` text (Task 1).
- Modify `src/cli/derive_shadow.rs`, `src/main.rs`: `--old-root`, the immutable open, `resolve_old_catalog` (Task 1); `Commands::Cutover` (Tasks 2, 8, 9).
- Modify (span variants, no behaviour change): `src/librarian/identity/grammar.rs`, `src/librarian/tools/link_scan/extract.rs` (Task 3).
- Create `src/librarian/cutover/{mod,old_rows,plan,report,code_refs,rewrite,history,rename,entries,sidecars,events,links,apply}.rs` (Tasks 2 to 8).
- Modify `src/librarian/mod.rs` (`pub mod cutover;`), `src/librarian/derived/build.rs` (`repo_name_of` to `pub(crate)`), `src/librarian/identity/index.rs` (two queries).
- Extractions (pure text out of writers): `src/librarian/catalog/augmentation.rs` (`render_pending_section`), `src/librarian/augmentation_sidecar.rs` (`render`, `declaration_stamped`), `src/librarian/tools/doctor.rs` (callers), `src/librarian/catalog/event_text.rs` (`TWIN_KINDS`, `twin_home_refusal`, `fence_for`), `src/librarian/tools/event_create.rs` (caller), `src/cli/write_lock.rs` (`lock_root_for`).
- Create `src/cli/cutover.rs`, `tests/cli_cutover.rs`, `docs/manual/src/concepts/artifact-cutover.md`; modify `src/cli/mod.rs`, `docs/manual/src/SUMMARY.md`, `src/librarian/derived/schema.rs` (`DERIVED_CATALOG_FILE`).

## Rulings (controller D-R1..D-R9, D-R21, D-R22; mine D-R10..D-R20, D-R23..D-R30; the operator may overrule any)

- **D-R1** File-only exports. The tool opens the old catalog read-only and never migrates it, computes each export as pure text, and writes files through `write_catalogued_file`. Verified: `write_entry_into_ledger` (`catalog/dual_write.rs:120-183`) ends in `record_written_file` (`:176`), which runs `UPDATE artifact` (`:58-66`), so it cannot be called. Its pure parts are already functions (`frontmatter_mark` `:102`, `frontmatter::upsert_int_line` `frontmatter.rs:387`, `body_entry_heading_level` `augmentation.rs:2830`), except the section text inside `splice_pending_section` (`augmentation.rs:2362`), which Task 6 extracts. `doctor`'s `export_augmentation_sidecars` (`tools/doctor.rs:2025-2144`) only reads the catalog but writes files inline; Task 6 extracts its two text steps. `event_create`'s twin target check (`tools/event_create.rs:354-394`) reads through a `ToolContext`; Task 7 extracts its pure predicate. Change absorbed: a live writer and the cutover share one text format. Cost if wrong: one more caller per extracted function.
- **D-R2 (revised)** Three subcommands, no commit: `cutover plan` (dry run), `cutover export` (writes), `cutover build`. Idempotence is by content. Each detector gives status `already_exported`: an entry whose token the body defines and whose params fields the section text holds; an event or observation whose id `parse_event_sections` finds; a relation line `splice_relation` finds (`None`); a sidecar equal to the catalog's shape and declared; a rename whose result is in `HEAD` (D-R25). `find_unrecorded_section` (`dual_write.rs:281-297`) is not used: it adopts by title and body against a catalog `is_recorded` callback; the cutover asks what the file holds.
- **D-R3** `--old-root <path>` (repeatable) on `derive-shadow` and `cutover`. One `RootMap` decides scope for both. When rows exist and none maps, the run is refused (`NoOldRowsUnderRoot`).
- **D-R4** The new catalog file is `derived.db` (`DERIVED_CATALOG_FILE`) in the directory of the old catalog, so `VectorCache::beside` (`vector_cache.rs:268`) gives the same `vector-cache.db` for both. An old binary never opens it: it opens only `LIBRARIAN_DB` or `<data dir>/librarian/catalog.db` (`librarian/mod.rs:125`, `:320`) and `vector-cache.db`. An old binary started with `LIBRARIAN_DB` set to `derived.db` would apply its schema on top; the new `schema::open` refuses the reverse (`refuse_if_foreign`, `derived/schema.rs:194`).
- **D-R5** The cutover carries its own rename and citation rewrite (bug `67d5983be664de6a`): grammar-aware, over file text, never driven by `entry_cite` rows (scout D-5, `diff.rs:111-121`).
- **D-R6** Events and observations go only to single-writer homes (refined by D-R12); every other row is reported with its reason and stays a row.
- **D-R7** Best effort with a report (format in Task 2). Every catalog fact is moved, already exported, or named in a report line. Ambiguous targets are reported, never guessed.
- **D-R8** Migration window only. The tool refuses when a `-wal`, `-shm` or `-journal` file lies beside the old catalog, checks again after it has read the rows (and that the file's size and mtime did not change), and `cutover export` checks once more before it writes. `export` also refuses while another process holds the project write lock (a budget of `LOCK_ATTEMPT` = 250 ms, about six tries; a zero budget cannot work because `write_guard::acquire` refuses it; D-R30) and once `derived.db` exists.
- **D-R9 (revised by D-R22)** Order: old catalog access, then the dry run (`cutover plan`, value without writing), the scanners' spans, the rewrite, renames, exports, `export`, `build`, gate. Final-wave rule (C1): text that an export step adds (a note, an observation, an entry title) is rewritten like committed text, so the plan after `export` and a commit writes nothing. Known gap (C1-a, parked): the citation index is built before entry recovery, so a token that only a recovered params entry defines resolves differently in the first and the second plan; a second export converges.
- **D-R10 (mine)** The input is `HEAD`, not the working tree. A plan is a function of (`HEAD`, old catalog). The applier writes a file only when its working-tree bytes equal `HEAD`'s or the plan's; any other file is refused and reported. Cost: uncommitted edits are invisible to the plan; commit or stash first.
- **D-R11 (mine, corrects the brief)** Grammar forms come from the cutover's own `KeyIndex` of `HEAD` after renames, never from `cite_text::grammar_form_of`, which reads `artifact.abs_path` and stats it on disk (`cite_text.rs:61-154`) and so fails for another root spelling (D-R3) and for pre-rename keys. Every rendered form is checked with `KeyIndex::resolve` and must give `File` or `Entry`.
- **D-R12 (mine, corrects the brief)** An event's home is a document with `single_writer: true` that is neither a ledger (`entry_prefix`) nor an entry document (`entry_kind`): C2 refuses both (`event_create.rs:385-392`). Only `TWIN_KINDS` export. `field_patch`, `status_change`, `superseded_by`, `worktree_fork`, `worktree_merge` are skipped with the spec's reason. An observation is "carried by its note" when its artifact has a `note` event with the identical text (C2-R2); otherwise it becomes a `note` section with id `observation-<rowid>`, payload `{"text": …}` plus `"source"` when the row has one.
- **D-R13 (mine)** A params entry is recovered when its body defines no heading for its token. The section heading and validity stamp come from the extracted `render_pending_section`; its prose is one fenced block, info string `params-entry`, that holds the entry's JSON without `id`, with the fence rule of event payloads (`fence_for`, the longest backtick run plus one, at least three; `event_text.rs:178-183`). It goes before the first `## Events` or `## Relations` heading, else at the end of the body. `entry_high_water_<P>` rises to the number when the document has a frontmatter block. Cost: the stamp is the export date; `codescout-37` may reformat the block.
- **D-R14 (mine, extended by D-R21)** Rename rule: the new stem is the old stem in ASCII lowercase with every run of other characters replaced by one `-`, trimmed of `-`, in the same folder. Reported, never renamed: a folder index whose folder name is the problem, an empty or hex-shaped result, a target path that exists, every key collision (with a suggestion `<parent folder>-<stem>`), every D-R21 blocker, every frozen file (D-R26).
- **D-R15 (mine)** Never rewritten: fenced and indented code, inline code, frontmatter, HTML (comments included), wiki-link labels, a match inside a word that holds `://` (a bare URL), a token directly after `<word>:` that is not a recognised qualifier (so `My_Notes:F-2` never becomes `My_Notes:[[…]]`), documents outside the key space, and frozen files. A citation only inline code holds is counted (`in_code`), because the old scanner read inline code (`link_scan/extract.rs`, `Event::Code` arm) and the grammar does not. Reference-style links and links with a `#fragment` are reported with a suggested form.
- **D-R16 (mine)** The old catalog is opened by `open_old_immutable`: refuse when a side file exists, then `file:<path>?mode=ro&immutable=1` with `SQLITE_OPEN_READ_ONLY | SQLITE_OPEN_URI`, so SQLite creates nothing. `derive-shadow` uses this open for a WAL catalog with no side file (today it refuses that case, `derive_shadow.rs:115-128`, the migration-window state), keeps `open_old_read_only` with both, and refuses with one or with a `-journal`. Cost (W-13): outside the window, a server that starts during a `derive-shadow` run can make its numbers stale.
- **D-R17 (mine, revised)** The report is printed every time. `cutover export` also writes `cutover-report-<repo name>-<commit12>.md` and `.json` in the directory of the old catalog, outside every repository. An existing report is never overwritten: the next free name `…-<commit12>-<n>.md` is used.
- **D-R18 (superseded by D-R22).**
- **D-R19 (mine)** A manual link is an `artifact_link` row whose `rel` is neither `cites` (written by `link_scan`) nor `worktree_of` (overlay lineage). Its line is `**<label_for_rel(rel)>:** [[dst-key]]` under `## Relations` of the source, on any document (C2: `doc(link)` writes into any source). `entry_cite` rows with `origin='write'` are report-only.
- **D-R20 (mine)** A bare token (a grammar `Bare` reference) is rewritten to `[[definer-key:TOKEN]]` only when the source does not define it and exactly one document defines it, counting every defining document (colliding keys, invalid keys, documents the plan renames) and with that document's key unique; otherwise it is reported (spec § Cutover step 3: several owners are reported, not guessed). `<this repo's name>:TOKEN` is read as a bare token; another qualifier that names no file stem is reported as cross-repo. `cutover plan` lists every rewrite with `path:line`, so the operator reads the prose changes before `export`.
- **D-R21 (controller)** A rename is never applied when (a) its new key equals a key any `HEAD` document or another planned rename holds, or (b) any non-markdown `HEAD` file (code, scripts, config, CI) holds the old file name or the old path; such a rename is reported with its blockers as `path:line`. The report and the manual page tell the operator to extend `.codescout/key-space.toml` before the run for subtrees whose names other tools dictate. No `--no-rewrite` flag.
- **D-R22 (controller)** `derive-shadow --cutover` does not exist. The dry run is `cutover plan`. `derive-shadow` and `cutover` resolve the old catalog through one function, `resolve_old_catalog` (`--old-db`, else `LIBRARIAN_DB`, else `<data dir>/librarian/catalog.db`), so two dry runs read one catalog. This changes `derive-shadow`'s default when `LIBRARIAN_DB` is set (today it reads only the data-dir default, `derive_shadow.rs:340`).
- **D-R23 (mine)** `ParamsOther`: every params key outside the entry collection, and every key of an augmentation without an entry collection, is reported `unrecoverable` with the key names (one item per augmentation). The sidecar cannot carry them: `AugmentationSidecar` has no params field and its module states that params never travel (`augmentation_sidecar.rs:11-14`, `:49-64`; `to_row` restores `params: "{}"`, `:131`). A params snapshot file would be a new store, which the spec removes ("params cease to be a store"). A body-defined entry whose params fields the section text does not hold is `skipped` with `entry exists; fields not in text: a, b` (a field is in the text when its value, as a string, appears in the entry's section).
- **D-R24 (mine)** Sidecar comparison, not doctor's never-refresh skip (`doctor.rs:2075-2085`): no sidecar in `HEAD` → write it and stamp the declaration; a sidecar whose `drifting_fields` (`augmentation_sidecar.rs:466`) against the catalog row is empty → `already_exported` (stamp the declaration when it is missing); a sidecar that differs → write nothing and report `unrecoverable` with `shape drift: <fields>`. A committed sidecar is never overwritten and a newer catalog shape is never lost silently.
- **D-R25 (mine)** The rename map of a run after the commit is derived, not stored: an old-catalog path that `HEAD` lacks maps to `<same folder>/<canonical_stem>.md` when `HEAD` has that file and the old catalog has no live row for it (so the file did not exist before the cutover). The same map gives `renamed_keys`, so a `[[My_Notes]]` left in a file the first run did not reach still follows the rename. Chosen over persisting the map in the report because the report is one machine's file outside the repository, and a plan stays a function of (`HEAD`, old catalog). Cost if wrong: an unrelated file created with exactly the canonical name after the cutover started, while the old one was deleted, would inherit the old rows; both conditions together are unlikely inside the window.
- **D-R26 (mine)** Frozen files are never renamed and never edited; every planned change to one becomes an `unrecoverable` item `frozen: <why>`. A markdown file is frozen when: a non-markdown `HEAD` blob holds its repo-relative path, or a non-markdown blob in its own folder holds its file name (`include_str!("./render_prompt.md")`, `judge.py`'s `_HERE / "judge_prompt.md"`); it lies under a directory whose name starts with `.` (`.github` templates, names GitHub dictates); or it lies under the `src` directory of a `HEAD` `book.toml` (mdBook reads `[x](y.md)` links and would break on wiki links). Change absorbed: a new kind of path reader adds one rule in `code_refs.rs`.
- **D-R27 (mine)** Spans come from the real scanners: `identity::grammar::extract` (wiki links, bare tokens) and `link_scan::extract` (path links, `<stem>:TOKEN`, hex ids, inline-code hits), each given a span variant that the existing function is built on. A span exists only where the event text equals its source slice, as `grammar::bare_tokens` already requires (`grammar.rs:189-203`); a citation without a span is reported, never rewritten. Old-scanner spans inside a grammar wiki link are dropped. Bare tokens come only from grammar spans (which skip headings, code and opaque link text).
- **D-R28 (mine)** History binds an id only to the exact path it hashes: `artifact_id_from_abs(spelling.join(path))`. That path reaches `HEAD` when `HEAD` holds it, or through a chain of git renames (`find_similar` with renames on). Otherwise the id is reported with a suggestion (`[[key]]` when a `HEAD` document has `key_of(path)`), never rewritten. A shallow clone (`Repository::is_shallow`) skips history; the report says so and reports each id history would have needed.
- **D-R29 (mine)** `declaration_stamped` keeps doctor's behaviour: it is `frontmatter::parse` then `frontmatter::write`, which re-serialises the whole block (first-class fields first, extras after, comments dropped; `frontmatter.rs:174-219`). Its test asserts the parsed frontmatter equals the input's plus the key, and the body is byte for byte. Cost: a stamped document's frontmatter is re-serialised, as `doctor fix=export_augmentations` does today.
- **D-R30 (mine)** `export` takes the lock that the CLI's other writers take: `lock_root` semantics (the nearest `.codescout/project.toml` from the project up to the git root, `write_lock.rs:31-41`), through an extracted `lock_root_for(start, git_root)`. The side-file check remains the primary guard (an idle server holds no lock).

## Review Focus

1. A second `export`, and a `plan` after the commit, write nothing and report every moved fact as `already_exported` with no new `unrecoverable` (Task 8 `a_second_export_finds_everything_written_and_writes_nothing`; Task 9 e2e post-commit tally).
2. A file that code, a dot-directory or mdBook reads by path is never renamed or edited, and a rename never makes a key ambiguous (Task 5 `a_file_code_reads_is_frozen`, `two_files_that_would_share_a_key_are_both_reported`; Task 4 `an_mdbook_page_is_frozen`).
3. Code, inline code, frontmatter, HTML, labels, bare URLs and escaped runs are never rewritten (Task 4 `nothing_in_code_frontmatter_comments_labels_or_urls_is_rewritten`, `a_token_split_by_an_escape_is_not_rewritten`).
4. The old catalog keeps every byte and gains no side file, and a held or interrupted catalog is refused (Task 1 immutable tests; Task 9 e2e).
5. No catalog fact disappears without a report line: params keys outside the collection, sidecar drift, unmatched history ids (Task 6 `params_keys_outside_the_collection_are_reported_by_name`, `a_differing_sidecar_is_reported_and_not_overwritten`; Task 4 `a_deleted_files_id_never_maps_to_another_file_with_its_key`).
6. A note on a ledger, an entry document or an unflagged document stays a row and is reported "no home" (Task 7 `a_flagged_ledger_has_no_home`, `a_note_on_an_unflagged_document_has_no_home`).

---

### Task 1: Old catalog access: `RootMap`, `--old-root`, the immutable open, one resolver

**Files:**
- Modify: `src/librarian/derived/diff.rs` (`NoOldRowsUnderRoot` L323-341, `open_old_read_only` L372, `diff` L392, `diff_with_rules` L402, `load_old_documents` L587; `scope_of` L550 stays: `cite_text.rs:73` and `:262` call it)
- Modify: `src/cli/derive_shadow.rs` (`DeriveShadowArgs` L51-65, `refuse_if_open_would_write` L115, `compare` L177, `run` L336-340), `src/main.rs` (`DeriveShadow` help, L232-244)

**Interfaces:**
- Produces in `crate::librarian::derived::diff`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootMap { pub git_root: PathBuf, pub old_roots: Vec<PathBuf> }
impl RootMap {
    pub fn new(git_root: PathBuf) -> Self;
    pub fn with_old_roots(git_root: PathBuf, old_roots: Vec<PathBuf>) -> Self;
    /// `scope_of` against git_root, then each old root in order; the first non-OtherRoot wins.
    pub(crate) fn scope(&self, abs_path: &str) -> Scope;
    /// git_root first, then old_roots (Task 4 hashes ids under each).
    pub fn spellings(&self) -> Vec<&Path>;
}
pub struct NoOldRowsUnderRoot { pub old_roots: usize, pub other_roots: usize }   // no path field
pub fn diff(old: &OldCatalog, derived: &Connection, roots: &RootMap) -> anyhow::Result<ShadowDiff>;
pub fn diff_with_rules(old: &OldCatalog, derived: &Connection, roots: &RootMap, rules: &[CompiledRule]) -> anyhow::Result<ShadowDiff>;
/// `<path>-wal`, `<path>-shm` or `<path>-journal` exists (`symlink_metadata`).
pub fn side_files_present(path: &Path) -> bool;
/// (len, mtime) of the file, for the re-check after a load (D-R8).
pub fn file_stamp(path: &Path) -> std::io::Result<(u64, std::time::SystemTime)>;
/// Refuse when `side_files_present`; else open `file:<url>?mode=ro&immutable=1`.
pub fn open_old_immutable(path: &Path) -> anyhow::Result<OldCatalog>;
impl OldCatalog { pub(crate) fn connection(&self) -> &Connection; }
```
- In `crate::cli::derive_shadow`: `pub(crate) fn resolve_old_catalog(arg: Option<&Path>, librarian_db: Option<OsString>, data_dir: Option<PathBuf>) -> Option<PathBuf>` (D-R22); `DeriveShadowArgs` gains `#[arg(long = "old-root")] pub old_root: Vec<PathBuf>` (each absolute, else refused naming the flag).

- [ ] **Step 1: Write the failing tests** (stub every signature with `todo!()`):
  - `diff.rs` `rows_under_an_old_root_are_compared_as_if_under_the_git_root` (`matched_documents == 1`, `ignored_other_roots == 0`); `the_git_root_is_tried_before_an_old_root` (`old_path_collisions == 1`, lowest id kept); `no_row_under_the_root_or_an_old_root_is_refused_without_a_path` (downcast `NoOldRowsUnderRoot { old_roots: 1, other_roots: 1 }`; `Display` contains `--old-root` and contains neither the git root nor the old root); `the_symlink_hint_stays_when_no_old_root_is_given` (contains `symlink`, no path).
  - `an_immutable_open_of_a_clean_wal_catalog_creates_nothing_and_changes_no_byte` (`clean_wal_catalog`, L1883); `an_immutable_open_is_refused_for_each_side_file` (`-wal`, `-shm`, `-journal` in turn: the error contains `Stop every codescout process` and `PRAGMA wal_checkpoint(TRUNCATE)`, and no other side file appears); `an_immutable_open_reads_a_path_with_a_space_and_a_percent`.
  - `derive_shadow.rs`: rename `a_wal_catalog_no_process_holds_open_is_not_compared_and_nothing_appears_beside_it` (L551) to `…_is_compared_and_nothing_appears_beside_it` (code 0, listing unchanged); `a_wal_catalog_with_one_side_file_is_not_compared`; `a_leftover_journal_is_not_compared`; `rows_under_an_old_root_are_compared`; `an_old_root_that_maps_no_row_is_not_compared_and_names_the_flag`; `a_relative_old_root_is_refused`; `resolve_old_catalog_prefers_the_flag_then_librarian_db_then_the_data_dir`. Update `an_old_catalog_whose_rows_all_lie_under_another_root_is_not_compared` (L656) and `a_project_indexed_through_a_symlink_is_not_compared_instead_of_called_all_new` (L673) if they assert the path.
- [ ] **Step 2: Run** `scripts/with-slot.sh cargo test --lib derived::diff` and `scripts/with-slot.sh cargo test --lib cli::derive_shadow`. Expected: the new tests FAIL.
- [ ] **Step 3: Implement.** `load_old_documents` takes `&RootMap`. `NoOldRowsUnderRoot` text: `no old row lies under the git root or under any of the {n} --old-root paths given ({m} rows lie under other roots)`; with `n == 0`: `no old row lies under the git root ({m} rows lie under other roots; is the root spelled through a symlink? pass --old-root <the other spelling>)`. Immutable open: `url::Url::from_file_path(path)` (refuse a non-absolute path), `format!("{url}?mode=ro&immutable=1")`, flags `SQLITE_OPEN_READ_ONLY | SQLITE_OPEN_URI | SQLITE_OPEN_NO_MUTEX`, then `OldCatalog::from_connection`. Side-file refusal text: `a -wal, -shm or -journal file lies beside the old catalog. Another process holds it open, or one stopped without closing it. Stop every codescout process, run sqlite3 <old catalog file> 'PRAGMA wal_checkpoint(TRUNCATE)', and run this again.` (no repository command checkpoints a catalog today: `wal_checkpoint` appears only in tests). In `compare`: WAL with both side files → `open_old_read_only`; neither → `open_old_immutable`; otherwise refuse. `run` uses `resolve_old_catalog`. Update the `main.rs` help.
- [ ] **Step 4: Run** the two filters and `scripts/with-slot.sh cargo test --test cli_derive_shadow`. Expected: PASS.
- [ ] **Step 5: Commit** by pathspec: `feat(identity): --old-root and an immutable open of the old catalog (D task 1)`.

### Task 2: Old rows, the plan skeleton, the report, `cutover plan`

**Files:**
- Create: `src/librarian/cutover/mod.rs` (module doc: the three subcommands, D-R2, D-R10), `old_rows.rs`, `plan.rs`, `report.rs`; `src/cli/cutover.rs` (`plan` only)
- Modify: `src/librarian/mod.rs` (`pub mod cutover;`), `src/librarian/derived/build.rs` (`repo_name_of` L311 to `pub(crate)`), `src/cli/mod.rs`, `src/main.rs` (`Commands::Cutover`)

**Interfaces:**
- Consumes: `RootMap`, `open_old_immutable`, `side_files_present`, `file_stamp`, `OldCatalog::connection`, `resolve_old_catalog` (Task 1); `AugmentationRow` (`augmentation.rs:15`); `scan::scan_commit` (`scan.rs:113`), `build::key_space_from_commit` (`build.rs:136`), `build::repo_name_of`.
- Produces:

```rust
// cutover::old_rows
pub struct OldDocument { pub id: String, pub rel_path: String, pub missing: bool }
pub struct OldSource { pub kind: String, pub uri: String, pub payload: Option<serde_json::Value> }
pub struct OldEvent { pub id: String, pub artifact_id: String, pub kind: String, pub payload: serde_json::Value,
    pub anchor_commit: Option<String>, pub head_commit: Option<String>, pub author: Option<String>,
    pub created_at_ms: i64, pub parent: Option<String>, pub resolves: Option<String>,
    pub mutates: Vec<String> /* artifact ids */, pub source: Option<OldSource> }
pub struct OldObservation { pub id: i64, pub artifact_id: String, pub text: String, pub source: Option<String>, pub created_at_ms: i64 }
pub struct OldLink { pub src_id: String, pub dst_id: String, pub rel: String }
#[derive(Default, Clone, Serialize)] pub struct OutOfScope { pub other_roots: usize, pub worktrees: usize }
pub struct OldRows { pub documents: BTreeMap<String, OldDocument> /* by id */, pub augmentations: Vec<AugmentationRow>,
    pub events: Vec<OldEvent>, pub observations: Vec<OldObservation>, pub links: Vec<OldLink>,
    pub write_cites: usize, pub out_of_scope: OutOfScope }
pub fn load(old: &OldCatalog, roots: &RootMap) -> anyhow::Result<OldRows>;
// cutover::plan
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)] #[serde(rename_all = "snake_case")]
pub enum Class { ParamsEntry, ParamsOther, AugmentationShape, Event, Observation, ManualLink, WriteCite, Rename, Citation }
#[derive(Clone, Copy, PartialEq, Eq, Serialize)] #[serde(rename_all = "snake_case")]
pub enum Status { Moved, AlreadyExported, Skipped, Unrecoverable }
#[derive(Clone, Debug, Serialize)] pub struct Item { pub class: Class, pub status: Status, pub reason: String,
    pub path: Option<String>, pub line: Option<u32>, pub detail: Option<String>, pub suggestion: Option<String> }
#[derive(Clone, Debug)] pub struct FileChange { pub path: String, pub renamed_from: Option<String>,
    pub head_text: Option<String> /* None: a new file */, pub new_text: String }
#[derive(Default, Serialize)] pub struct ClassTally { pub moved: usize, pub already_exported: usize,
    pub skipped: usize, pub unrecoverable: usize, pub reasons: BTreeMap<String, usize> }
pub struct ExportPlan { pub repo_name: String, pub commit: String, pub shallow: bool,
    pub population: BTreeMap<String, usize>, pub items: Vec<Item>,
    pub files: BTreeMap<String, FileChange> /* by final path */, pub out_of_scope: OutOfScope }
impl ExportPlan { pub fn tally(&self) -> BTreeMap<Class, ClassTally>;
    /// Files whose new text differs from HEAD, plus renames: what `export` would write.
    pub fn writes(&self) -> usize; }
pub fn plan_export(repo: &git2::Repository, commit: &git2::Commit, old: &OldRows) -> anyhow::Result<ExportPlan>;
// cutover::report
pub struct ReportHeader { pub phase: &'static str /* "plan" | "export" */, pub old_roots_given: usize }
pub fn render_markdown(plan: &ExportPlan, h: &ReportHeader) -> String;
pub fn render_json(plan: &ExportPlan, h: &ReportHeader) -> serde_json::Value;
// cli::cutover
#[derive(Debug, Args)] pub struct CutoverArgs { #[command(subcommand)] pub action: CutoverAction }
#[derive(Debug, Subcommand)] pub enum CutoverAction { Plan(CommonArgs) /* Export, Build: Tasks 8, 9 */ }
#[derive(Debug, Args)] pub struct CommonArgs { #[arg(long)] pub project: Option<PathBuf>, #[arg(long)] pub old_db: Option<PathBuf>,
    #[arg(long = "old-root")] pub old_root: Vec<PathBuf>, #[arg(long)] pub json: bool }
pub struct Outcome { pub code: i32, pub stdout: String }  // 0 ran, 1 export refused some files, 2 did not run
pub struct Loaded { pub git_root: PathBuf, pub old_db: PathBuf, pub plan: ExportPlan }
/// Resolve, refuse on side files, open, load rows, plan HEAD, then re-check side files and the stamp.
pub(crate) fn load_and_plan(args: &CommonArgs, cwd: &Path, librarian_db: Option<OsString>, data_dir: Option<PathBuf>) -> anyhow::Result<Loaded>;
pub fn evaluate_plan(args: &CommonArgs, cwd: &Path, librarian_db: Option<OsString>, data_dir: Option<PathBuf>) -> Outcome;
pub async fn run(args: CutoverArgs) -> anyhow::Result<()>;
```

**Report format (D-R7).** Markdown: `# Cutover report — <repo name> at <commit12>`, a `Phase:` line, a `History:` line (`complete` or `shallow clone: history recovery skipped`), then `## Old catalog rows in scope` (class | rows, plus the two out-of-scope counts), `## Outcomes` (class | moved | already exported | skipped | unrecoverable), `## Reasons` (per class, `- <reason>: <n>`), `## Not moved` (every `unrecoverable` and `skipped` item: `- <path>[:<line>] — <reason>[ — use <suggestion>]`), `## Frozen files and blocked renames` (with `path:line` blockers, and the sentence: `To take a subtree whose names another tool dictates out of the key space, add it to .codescout/key-space.toml, commit, and run cutover plan again.`), `## Files` (`- <path>` or `- <path> (renamed from <old>)`), `## Rows with no committed home` (per document: count by kind). JSON: `{"phase", "repo_name", "commit", "shallow", "population", "out_of_scope", "tally": {class: ClassTally}, "items": [Item], "files": [{"path", "renamed_from"}]}`. Both forms are complete.

- [ ] **Step 1: Write the failing tests.** Fixture `old_catalog_with(root, |cat: &Catalog| …) -> OldCatalog`: `Catalog::open_in_memory()`, fill through `artifact::upsert`, `augmentation::upsert`, `events::insert`, `event_edges::insert_many`, `sources::upsert`, `observations::insert`, `links::insert`, `entry_cite::insert_with`, then `OldCatalog::from_connection(cat.conn)`.
  - `old_rows`: `load_keeps_rows_under_the_root_and_counts_the_rest`; `an_event_carries_its_parent_resolves_mutates_and_source`; `manual_links_exclude_cites_and_worktree_of`; `only_write_cites_are_counted`; `rows_of_an_out_of_scope_artifact_are_not_loaded`; `a_missing_table_is_named` (only `OLD_CATALOG_SCHEMA`, `testkit.rs:114`: error contains `events`).
  - `plan`: `a_catalog_with_no_rows_plans_nothing`; `the_population_counts_every_class` (`params_entry`, `params_other` (keys outside collections), `augmentation_shape`, `event:<kind>`, `observation`, `manual_link`, `write_cite`); `event_kinds_with_no_text_home_are_skipped_with_the_specs_reason` (`field_patch` → `git history of the file holds it`; `status_change`, `superseded_by` → `the frontmatter holds it`; `worktree_fork`, `worktree_merge` → `the worktree overlay is removed`); `write_cites_are_reported_not_exported`.
  - `report`: `the_markdown_has_every_section_and_no_absolute_path`; `json_and_markdown_carry_the_same_counts`.
  - `cli/cutover.rs`: `plan_writes_nothing_and_prints_the_report` (repo tree and old catalog directory unchanged, code 0); `plan_refuses_when_a_side_file_lies_beside_the_old_catalog` (code 2); `plan_refuses_when_the_catalog_changed_during_the_load` (a test seam calls a hook between `load` and the re-check that touches the file: code 2, text `the old catalog changed while it was read`).
- [ ] **Step 2: Run** `scripts/with-slot.sh cargo test --lib cutover::` and `scripts/with-slot.sh cargo test --lib cli::cutover`. Expected: FAIL.
- [ ] **Step 3: Implement.** `load` runs its own SQL on `old.connection()` in one `unchecked_transaction` dropped without commit (as `diff_with_rules` does, `diff.rs:411-414`), and prepares every query first so a missing table or column is named. An artifact is in scope when `roots.scope(abs_path)` is `Inside`. `plan_export` loads `HEAD` once (`scan_commit` with the commit's key space, `repo_name_of`, `repo.is_shallow()`) into a private `Head { files: BTreeMap<String, String>, outside: BTreeSet<String>, repo_name: RepoName }` and, in this task, fills `population`, `out_of_scope`, `shallow`, the event-kind items and the write-cite items. `load_and_plan` re-checks `side_files_present` and `file_stamp` after `plan_export`.
- [ ] **Step 4: Run** the filters again. Expected: PASS.
- [ ] **Step 5: Commit** by pathspec: `feat(identity): cutover old rows, plan skeleton, report and cutover plan (D task 2)`.

### Task 3: Spans from the two real scanners

**Files:**
- Modify: `src/librarian/identity/grammar.rs` (`extract` L98-159 built on a span variant), `src/librarian/tools/link_scan/extract.rs` (`extract` L481-604 built on a span variant; `link_target` L698 to `pub(crate)`)

**Interfaces:**
- Produces:

```rust
// identity::grammar
pub struct SpannedCitation { pub citation: Citation, pub range: std::ops::Range<usize> /* bytes of `text` */ }
pub fn extract_spanned(text: &str) -> Vec<SpannedCitation>;   // `extract` == this mapped to `.citation`
// link_scan::extract
pub(crate) struct CitationSpan { pub kind: CitationKind, pub raw: String, pub line: u32,
    /// Bytes of `text`; None when the event's text is not its source slice (an escape or an entity).
    pub range: Option<std::ops::Range<usize>>, pub in_code: bool }
pub(crate) fn citation_spans(text: &str) -> Vec<CitationSpan>;  // `extract`'s citations == these folded by push_citation
```
- No behaviour change: pinned by every test in `grammar.rs` `mod tests` (L211-570) and `link_scan/extract.rs` `mod tests` (L742-1607), and `link_scan` and `derived::build` callers.

- [ ] **Step 1: Write the failing tests:**
  - `grammar.rs` `extract_is_extract_spanned_without_ranges` (over the module's existing fixture texts); `a_wiki_link_span_covers_its_brackets` (`x [[a-doc:F-1]] y` → range of `[[a-doc:F-1]]`); `a_bare_token_span_is_its_token`.
  - `extract.rs` `extract_citations_are_the_spans_folded` (same `raw`, `kind`, `line`, `repeat_lines` as `extract` on a mixed text); `a_span_covers_exactly_its_match` (for an id, a `stem:F-1`, a path link `[t](a.md)` whose range is the whole link); `an_escaped_or_entity_run_has_no_range` (`R\-91` gives no token at all; `&amp; F-1` gives `F-1` with `range: None`); `an_inline_code_hit_is_marked_in_code`.
- [ ] **Step 2: Run** `scripts/with-slot.sh cargo test --lib identity::grammar` and `scripts/with-slot.sh cargo test --lib link_scan::extract`. Expected: only the new tests FAIL.
- [ ] **Step 3: Implement.** Move each event loop into the span function and define the old function on top of it. In `link_scan`, a match's range is `event.range.start + match.start()` only when `text.get(event.range) == Some(event_text)`. Keep `ExtractMemo` on `extract` only.
- [ ] **Step 4: Run** the filters again, plus `scripts/with-slot.sh cargo test --lib link_scan` and `scripts/with-slot.sh cargo test --lib derived::build`. Expected: PASS.
- [ ] **Step 5: Commit** by pathspec: `refactor(identity): span variants of the grammar and link_scan scanners (D task 3)`.

### Task 4: Frozen files, citation rewrite, hex-id recovery

**Files:**
- Create: `src/librarian/cutover/code_refs.rs`, `rewrite.rs`, `history.rs`
- Modify: `src/librarian/identity/index.rs` (two queries), `src/librarian/cutover/plan.rs`

**Interfaces:**
- Consumes: `extract_spanned`, `citation_spans`, `link_target` (Task 3); `KeyIndex` (`index.rs:90`), `grammar::parse_target` (`grammar.rs:51`), `ids::artifact_id_from_abs` (`ids.rs:17`), `RootMap::spellings`, `OldRows::documents`.
- Produces:

```rust
// identity::index::KeyIndex
pub fn definers_of(&self, token: &str) -> Vec<Key>;   // documents with a unique key that define `token`, sorted
pub fn key_at(&self, rel_path: &str) -> Option<&Key>;  // the key of the document at `rel_path` when it is unique
// cutover::code_refs
pub struct CodeRefs { /* HEAD's non-markdown UTF-8 blobs, and the mdBook src trees */ }
impl CodeRefs {
    pub fn load(repo: &git2::Repository, commit: &git2::Commit) -> anyhow::Result<CodeRefs>;
    /// `path:line` of every non-markdown blob holding `needle`.
    pub fn mentions(&self, needle: &str) -> Vec<String>;
    /// D-R26: Some(why) when the markdown file at `rel_path` is frozen.
    pub fn frozen(&self, rel_path: &str) -> Option<String>;
}
// cutover::history
pub struct HistoryIds { /* id → hashed historical path; rename edges */ }
impl HistoryIds {
    pub fn load(repo: &git2::Repository, head: &git2::Commit, spellings: &[&Path]) -> anyhow::Result<HistoryIds>;
    /// The HEAD path the id's own path became (unchanged, or through git renames); Err(historical path) when none.
    pub fn head_path(&self, id: &str) -> Option<Result<String, String>>;
}
// cutover::rewrite
#[derive(Clone, Copy, PartialEq, Eq)] pub enum IdSource { Catalog, History }
pub struct IdTarget { pub rel_path: String, pub source: IdSource }
pub struct Resolver<'a> { pub index: &'a KeyIndex, pub this_repo: &'a RepoName,
    pub renamed_keys: &'a BTreeMap<String, Key>,   // Task 5; empty here
    pub stems: &'a BTreeMap<String, Vec<String>>,  // exact HEAD file stem → final rel paths
    pub ids: &'a BTreeMap<String, IdTarget>,       // 16-hex → final rel path
    pub id_hints: &'a BTreeMap<String, String> }    // 16-hex → historical path, reported with a suggestion
#[derive(Clone, Copy, PartialEq, Eq, Serialize)] #[serde(rename_all = "snake_case")]
pub enum Form { RenamedKey, StemToken, RepoToken, PathLink, HexId, BareToken }
pub enum Outcome { Rewritten { to: String }, Unresolved { reason: String, suggestion: Option<String> },
    Ambiguous { candidates: Vec<String> }, InCode }
pub struct Finding { pub form: Form, pub line: u32, pub raw: String, pub outcome: Outcome }
pub fn rewrite_document(rel_path: &str, text: &str, r: &Resolver) -> (String, Vec<Finding>);
```

**Rewrite rules (D-R15, D-R20, D-R27).** Grammar spans: a `Malformed` wiki link is a `RenamedKey` candidate (Task 5); a `Bare` token the source does not define gives `[[definer:TOKEN]]` when `definers_of` returns one key, `Ambiguous` for several, nothing for none. `link_scan` spans, minus those inside a grammar wiki-link range: `RelPathLink` → `link_target` joined to the source's folder and normalised; with a `#fragment` → reported with `[[key]]`; text equal to the key, the file name or the dest → `[[key]]`, else `[[key|text]]` (`[[key\|text]]` in a table row); text with `]`, `|` or a line end → reported. `CrossRepoToken` (`<qualifier>:TOKEN`, the lowercase `cross_repo_re`, `extract.rs:287-290`): qualifier equal to `this_repo` → as a bare token; in `stems` with one path defining the token → `[[key:TOKEN]]`; two paths → `Ambiguous`; none → `Unresolved` (`cross repo`). `ArtifactId` in `ids` → `[[key]]`; in `id_hints` → `Unresolved` with the suggestion; else `Unresolved`. `MalformedQualifier` → `Unresolved`. Never rewritten, reported: `in_code` (`InCode`), `range: None` (`escape or entity in the text run`), a match whose surrounding non-space word holds `://` (`inside a URL`), a grammar bare token preceded by `<word>:` (`qualified by <word>, which is no file stem`). Every rendered form must resolve through `index.resolve` to `File` or `Entry`, else `Unresolved`. Edits apply right to left on the original bytes. Lines are 1-based in `HEAD` text.

- [ ] **Step 1: Write the failing tests** (a `KeyIndex` of a few `DocInput`s; repo name `demo`):
  - `index.rs` `definers_of_lists_unique_keys_sorted_and_skips_a_collision`; `key_at_names_a_unique_document_only`.
  - `code_refs.rs` `a_path_in_any_code_file_freezes_the_file` (`src/server.rs` holding `../docs/X_Y.md` freezes `docs/X_Y.md`, the reason names `src/server.rs:<line>`); `a_file_name_in_code_of_the_same_folder_freezes_the_file` (`src/a/render_prompt.md` and `src/a/mod.rs` holding `include_str!("./render_prompt.md")`); `a_file_name_in_code_of_another_folder_does_not_freeze`; `a_dot_directory_file_is_frozen` (`.github/ISSUE_TEMPLATE/bug_report.md`); `an_mdbook_page_is_frozen` (`docs/manual/book.toml` with `src = "src"`; `docs/manual/src/SUMMARY.md` and a page are frozen; with no `src` key the default `src` applies); `binary_and_markdown_blobs_are_not_read_as_code`.
  - `history.rs` `an_id_maps_through_a_rename_to_its_head_path` (commit 1 `old/a.md`, commit 2 renames it to `new/a.md`, same content); `a_deleted_files_id_never_maps_to_another_file_with_its_key` (`docs/a/notes.md` deleted, `docs/b/notes.md` unrelated: `Err("docs/a/notes.md")`); `every_spelling_mints_its_own_id`; `the_id_is_artifact_id_from_abs_of_the_joined_path`.
  - `rewrite.rs` `a_stem_token_becomes_an_entry_form`; `a_stem_two_files_share_is_ambiguous_and_unchanged`; `this_repos_name_as_qualifier_reads_as_a_bare_token`; `another_qualifier_is_reported_cross_repo`; `a_path_link_becomes_a_wiki_link` (three cases); `a_path_link_with_a_fragment_is_reported_with_a_suggestion`; `a_catalog_hex_id_becomes_a_file_form`; `a_history_hint_is_reported_not_rewritten`; `a_bare_token_with_one_definer_is_qualified`; `a_bare_token_with_two_definers_is_ambiguous_and_unchanged`; `a_bare_token_the_source_defines_is_left_alone`; `an_uppercase_qualifier_is_reported_and_never_half_rewritten` (`My_Notes:F-2` stays byte for byte); `nothing_in_code_frontmatter_comments_labels_or_urls_is_rewritten` (fence, inline code, frontmatter, `<!-- -->`, `[[ledger|F-1]]` label, `https://host/runs/F-1` and `https://host/0123456789abcdef`: output equals input; one `InCode`, two `inside a URL`); `a_token_split_by_an_escape_is_not_rewritten`; `a_second_pass_changes_nothing`; `a_target_outside_the_key_space_is_reported`.
  - `plan` `every_in_space_document_is_rewritten_and_frozen_ones_are_reported` (a frozen document keeps its text; each of its would-be rewrites is an `unrecoverable` `Citation` item with reason `frozen: …`).
- [ ] **Step 2: Run** `scripts/with-slot.sh cargo test --lib cutover::` and `scripts/with-slot.sh cargo test --lib identity::index`. Expected: FAIL.
- [ ] **Step 3: Implement** the rules above. `CodeRefs::load` walks `HEAD`'s tree with `git2`, keeps blobs that are not `.md`, are valid UTF-8 and hold no NUL byte, and parses each `book.toml` with `toml` (`[book] src`, default `src`). `HistoryIds::load` walks every commit from `head`; each diff to its first parent (none for a root commit) runs `find_similar` with renames on; it records every `.md` path seen and every rename edge; `head_path` follows rename edges forward from the id's own path to a path `HEAD` holds. `plan_export` fills `ids` from `old.documents` first (`Catalog`), then from `HistoryIds` (`History`) for ids that are still open; `Err(path)` goes to `id_hints` with `[[key_of(path)]]` when `HEAD` has that key. With `plan.shallow`, history is not loaded and each still-open id is reported `shallow clone: history is incomplete`.
- [ ] **Step 4: Run** the filters again. Expected: PASS.
- [ ] **Step 5: Commit** by pathspec: `feat(identity): cutover frozen files, citation rewrite and hex-id recovery (D task 4)`.

### Task 5: Renames of non-canonical files

**Files:**
- Create: `src/librarian/cutover/rename.rs`
- Modify: `src/librarian/cutover/plan.rs`, `src/librarian/cutover/rewrite.rs` (`RenamedKey`)

**Interfaces:**
- Consumes: `key::key_of` (`key.rs:127`), `key::is_folder_index` (`:113`), `IndexProblem` (`index.rs:44`), `CodeRefs` (Task 4).
- Produces:

```rust
pub fn canonical_stem(stem: &str) -> Option<String>;   // D-R14; None when nothing is left
pub struct Rename { pub from: String, pub to: String, pub old_key: String /* key_of's lowercased stem */, pub new_key: Key }
/// `paths`: HEAD markdown paths inside the key space; `keys`: every valid key HEAD holds.
pub fn plan_renames(paths: &[String], keys: &BTreeSet<Key>, problems: &[IndexProblem], refs: &CodeRefs) -> (Vec<Rename>, Vec<Item>);
/// D-R25: old-catalog paths HEAD lacks whose canonical sibling HEAD holds and the old catalog does not.
pub fn renames_already_in_head(old: &OldRows, head_paths: &BTreeSet<String>) -> Vec<Rename>;
```
- `plan_export` builds the pre-rename `KeyIndex`, calls both functions, maps old rows through both maps, builds the final `KeyIndex` over post-rename paths, and fills `renamed_keys`. A rename from `renames_already_in_head` is one `Rename` item `already_exported`.

- [ ] **Step 1: Write the failing tests:**
  - `canonical_stem_examples`: `My_Notes` → `my-notes`; `a__b` → `a-b`; `--x--` → `x`; `日本` → `None`; `CLAUDE` → `claude`.
  - `a_non_canonical_file_is_renamed_in_its_folder`; `claude_md_and_readme_md_are_not_renamed`; `a_readme_in_a_non_canonical_folder_is_reported`; `a_hex_shaped_result_is_reported`; `a_taken_target_path_is_reported`; `two_files_that_would_share_a_key_are_both_reported` (`src/a/render_prompt.md`, `src/b/render_prompt.md`: both `key collision`); `a_rename_onto_a_key_another_folder_holds_is_reported` (`docs/My_Notes.md` with `notes/my-notes.md` present); `a_file_code_reads_is_frozen` (`src/render_prompt.md` and `src/lib.rs` holding `include_str!("./render_prompt.md")`: reported, blocker `src/lib.rs:1`); `a_file_name_anywhere_in_code_blocks_its_rename` (`scripts/x.py` holding `judge_prompt.md` for `scripts/m/judge_prompt.md` in another folder: blocked, not frozen); `a_dot_directory_file_is_not_renamed`; `a_key_collision_in_head_is_reported_with_a_suggestion`.
  - `renames_already_in_head_map_the_old_path` (old row `docs/My_Notes.md`, `HEAD` holds `docs/my-notes.md`, no old row at the new path); `a_canonical_sibling_the_old_catalog_already_held_is_not_a_rename`.
  - `rewrite.rs` `a_wiki_link_to_an_old_key_follows_the_rename` (`[[My_Notes]]` → `[[my-notes]]`, `[[my_notes:F-2]]` → `[[my-notes:F-2]]`, `[[demo/My_Notes]]` → `[[demo/my-notes]]`); `a_lowercase_stem_token_and_a_path_link_follow_the_rename` (`my_notes:F-2`, `[n](My_Notes.md)`).
  - `plan` `a_renamed_file_appears_once_under_its_new_path`; `rows_of_a_renamed_document_follow_it_after_the_commit` (plan of a `HEAD` that already holds the rename: the document's rows land on `docs/my-notes.md`, no `document not in HEAD`).
- [ ] **Step 2: Run** `scripts/with-slot.sh cargo test --lib cutover::`. Expected: FAIL.
- [ ] **Step 3: Implement.** Refusal reasons: `folder index: rename the folder by hand`, `no canonical name`, `hex-shaped name`, `target exists`, `key collision` (D-R21 a), `named by code: <path:line>, …` (D-R21 b, `refs.mentions(file name)` and `refs.mentions(path)`), `frozen: <why>`.
- [ ] **Step 4: Run** the filter again. Expected: PASS.
- [ ] **Step 5: Commit** by pathspec: `feat(identity): cutover renames, with code and key-collision blockers (D task 5)`.

### Task 6: Params entries, other params, augmentation shapes

**Files:**
- Modify: `src/librarian/catalog/augmentation.rs` (extract `render_pending_section` from `splice_pending_section` L2362), `src/librarian/augmentation_sidecar.rs` (extract `render` from `write` L309; add `declaration_stamped`), `src/librarian/tools/doctor.rs` (`stamp_augmentation_declaration` L2151 calls it), `src/librarian/catalog/event_text.rs` (extract `fence_for` from `render_event_section` L178-183)
- Create: `src/librarian/cutover/entries.rs`, `src/librarian/cutover/sidecars.rs`
- Modify: `src/librarian/cutover/plan.rs`

**Interfaces:**
- Produces:

```rust
// catalog::augmentation
/// `<#×level> <id> — <title>`, a blank line, the prose with the allocator's validity stamp, a blank line.
pub(crate) fn render_pending_section(id: &str, level: usize, s: &PendingSection) -> Result<String>;
// catalog::event_text
pub(crate) fn fence_for(content: &str) -> String;   // the longest backtick run plus one, at least three
// augmentation_sidecar
pub fn render(sidecar: &AugmentationSidecar) -> Result<String>;
pub(crate) fn declaration_stamped(content: &str, sidecar_rel: &str) -> Result<String>;  // D-R29
// cutover::entries
pub fn recover_params_entries(rel_path: &str, text: &str, row: &AugmentationRow) -> (String, Vec<Item>);
pub fn other_params(rel_path: &str, row: &AugmentationRow) -> Option<Item>;   // D-R23, class ParamsOther
// cutover::sidecars
pub enum ShapeExport { AlreadyExported { stamped_doc: Option<String> },
    Export { sidecar_rel: String, sidecar_text: String, stamped_doc: String },
    Drift { fields: Vec<&'static str> }, Refused(String) }
pub fn plan_shape(doc_rel: &str, doc_text: &str, row: &AugmentationRow, head_sidecar: Option<&str>) -> ShapeExport;
```
- Old callers pinned by: `allocator_stamps_a_default_validity_into_the_section_it_writes`, `allocator_does_not_double_stamp_a_caller_declared_class`, `allocator_refuses_a_malformed_caller_declared_class_rather_than_double_stamping` (`augmentation.rs:7368-7523`), `one_file_write_carries_the_mark_and_the_section_and_the_row_records_its_sha` (`dual_write.rs:693`), `export_augmentations_writes_a_sidecar_stamps_the_declaration_and_is_idempotent` (`doctor.rs:23302`), `doctor_export_augmentations_does_not_stamp_the_file_behind_a_catalogued_link` (`link_write_tests.rs:280`), the `event_text.rs` payload-block tests.

- [ ] **Step 1: Write the failing tests:**
  - `augmentation.rs` `render_pending_section_is_the_text_splice_inserts`. `event_text.rs` `fence_for_is_one_longer_than_the_longest_run`. `augmentation_sidecar.rs` `render_is_what_write_puts_on_disk`; `declaration_stamped_parses_to_the_same_frontmatter_plus_the_key` (`frontmatter::parse` of the output equals the input's plus `expects_augmentation`; the body is byte for byte).
  - `entries.rs` `an_entry_without_a_heading_gets_a_section_and_the_mark` (frontmatter `entry_prefix: F`, body defines `F-1`; params `entries: [{id: F-1}, {id: F-2, title: "second", status: "open"}]` → `## F-2 — second`, a `params-entry` block holding `{"status":"open","title":"second"}`, `entry_high_water_F: 2`; `F-1` `already_exported`, `F-2` `moved`); `a_defined_entry_names_the_fields_its_text_lacks` (`F-1` with `severity: "high"` absent from its section → `skipped`, `entry exists; fields not in text: severity`); `a_value_with_backticks_gets_a_longer_fence`; `the_section_goes_before_events_or_relations`; `the_heading_level_follows_the_ledger`; `a_document_without_frontmatter_gets_the_section_and_no_mark`; `an_entry_id_that_is_not_a_token_is_unrecoverable`; `the_title_falls_back_to_summary_then_the_id_and_is_one_line`; `recovering_twice_adds_nothing`.
  - `params_keys_outside_the_collection_are_reported_by_name` (params `{entries: [...], gather_from: "x", notes: 1}` → one `ParamsOther` `unrecoverable` item, detail `gather_from, notes`); `all_params_of_an_augmentation_without_a_collection_are_reported`; `empty_params_give_no_item`.
  - `sidecars.rs` `a_shape_is_exported_and_declared`; `an_equal_declared_sidecar_is_already_exported`; `an_equal_undeclared_sidecar_gets_only_the_stamp`; `a_differing_sidecar_is_reported_and_not_overwritten` (`Drift { fields: ["prompt"] }`); `a_document_without_frontmatter_is_refused`.
  - `plan` `a_ledger_with_an_entry_and_a_shape_changes_once_and_gains_one_sidecar`.
- [ ] **Step 2: Run** `scripts/with-slot.sh cargo test --lib cutover::`, `scripts/with-slot.sh cargo test --lib augmentation`, `scripts/with-slot.sh cargo test --lib event_text`, `scripts/with-slot.sh cargo test --lib export_augmentations`. Expected: only the new tests FAIL.
- [ ] **Step 3: Implement.** The extractions change no behaviour. In `plan_export` a document's text goes through Task 4's rewrite, then entries, then the stamp. An augmentation whose document is not in `HEAD` (after the rename maps) is `unrecoverable` (`document not in HEAD`); `params` that is not JSON gives one `ParamsOther` `unrecoverable` item (`params is not JSON`); a frozen document gives `frozen: …` items.
- [ ] **Step 4: Run** the four filters again. Expected: PASS.
- [ ] **Step 5: Commit** by pathspec: `feat(identity): cutover exports params entries and shapes, reports other params and drift (D task 6)`.

### Task 7: Events, observations and manual links

**Files:**
- Modify: `src/librarian/catalog/event_text.rs` (add `TWIN_KINDS`, `twin_home_refusal`), `src/librarian/tools/event_create.rs` (`TWIN_KINDS` L336 removed, `single_writer_target` L354 calls the predicate)
- Create: `src/librarian/cutover/events.rs`, `src/librarian/cutover/links.rs`
- Modify: `src/librarian/cutover/plan.rs`

**Interfaces:**
- Consumes: `EventText`, `EventSource`, `splice_event` (`event_text.rs:383`), `parse_event_sections` (`:297`), `link_text::label_for_rel` (`link_text.rs:23`), `splice_relation` (`:44`).
- Produces:

```rust
// catalog::event_text
pub(crate) const TWIN_KINDS: &[&str] = &["note", "verdict", "intent", "reviewed", "external_signal"];
/// None when `doc` is a home for `kind`; else event_create's reason:
/// "kind has no text twin" | "not a single-writer document" | "ledger or entry document".
pub(crate) fn twin_home_refusal(doc: &str, kind: &str) -> Option<&'static str>;
// cutover::events
pub fn export_events(rel_path: &str, text: &str, events: &[&OldEvent], observations: &[&OldObservation],
    notes_of_artifact: &[&OldEvent], form_of: &dyn Fn(&str) -> Option<String>) -> (String, Vec<Item>);
// cutover::links
pub fn export_links(rel_path: &str, text: &str, links: &[(String /* rel */, Option<String> /* form */)]) -> (String, Vec<Item>);
```
- `form_of(artifact_id)` (built in `plan_export`): the grammar form of an old document's final path, through the rename maps and the final `KeyIndex`, or `None`.
- Old caller pinned by `a_note_on_an_unflagged_document_keeps_only_the_row` and `a_flagged_ledger_or_entry_document_gets_no_text` (`event_create.rs` tests; assertions at L1274, L1297).

- [ ] **Step 1: Write the failing tests:**
  - `event_text.rs` `twin_home_refusal_gives_event_creates_reasons` (`field_patch`; no flag, `single_writer: "true"`, unparsable frontmatter; flag plus `entry_prefix` or `entry_kind`; flag alone → `None`).
  - `events.rs` `a_note_on_a_single_writer_document_is_spliced_with_every_field`; `an_event_whose_id_the_text_holds_is_already_exported` (no error, text unchanged); `a_note_on_an_unflagged_document_has_no_home` (`skipped`, `no committed home yet: not a single-writer document`); `a_flagged_ledger_has_no_home`; `an_unnameable_mutates_target_makes_the_event_unrecoverable`; `an_unclosed_fence_makes_the_event_unrecoverable_and_the_text_unchanged`.
  - `an_observation_its_note_carries_is_skipped` (`carried by its note event`); `an_observation_without_a_note_becomes_a_note_section`; `an_observation_whose_section_the_text_holds_is_already_exported` (id `observation-<rowid>` present; no `splice_event` error); `an_observation_on_an_unflagged_document_has_no_home`.
  - `links.rs` `a_manual_link_becomes_a_relation_line`; `a_link_whose_line_the_text_holds_is_already_exported`; `a_rel_with_no_label_is_unrecoverable`; `a_target_with_no_form_is_unrecoverable`; `a_link_on_an_unflagged_document_is_exported`.
  - `plan` `events_and_links_of_a_renamed_document_land_under_its_new_path`.
- [ ] **Step 2: Run** `scripts/with-slot.sh cargo test --lib cutover::`, `scripts/with-slot.sh cargo test --lib event_text`, `scripts/with-slot.sh cargo test --lib event_create`. Expected: only the new tests FAIL.
- [ ] **Step 3: Implement.** Before a splice, the ids `parse_event_sections` returns are checked; a held id is `already_exported` and `splice_event` is not called. Events of one document go in `created_at_ms` order, then observations, then relation lines, after Task 6's steps. Any other `splice_event` or `splice_relation` error is `unrecoverable` with the error text (no path) and leaves the text as it was. A document the rename maps do not find in `HEAD` is `unrecoverable` (`document not in HEAD`).
- [ ] **Step 4: Run** the three filters again. Expected: PASS.
- [ ] **Step 5: Commit** by pathspec: `feat(identity): cutover exports events, observations and manual links (D task 7)`.

### Task 8: `cutover export`: apply, locks, report files

**Files:**
- Create: `src/librarian/cutover/apply.rs`
- Modify: `src/cli/cutover.rs` (`Export`), `src/cli/write_lock.rs` (extract `lock_root_for` from `lock_root` L32-42), `src/librarian/derived/schema.rs` (`DERIVED_CATALOG_FILE`, `derived_catalog_beside`), `src/main.rs` (help)

**Interfaces:**
- Consumes: `load_and_plan`, `CommonArgs`, `Outcome`, `render_markdown`, `render_json` (Task 2); `side_files_present`, `file_stamp` (Task 1); `write_catalogued_file`; `ReadDoor::text`; `write_lock::take_at` (`write_lock.rs:74`).
- Produces:

```rust
// derived::schema
pub const DERIVED_CATALOG_FILE: &str = "derived.db";
pub fn derived_catalog_beside(old_catalog: &Path) -> PathBuf;
// cli::write_lock
pub(crate) fn lock_root_for(start: &Path, git_root: &Path) -> PathBuf;   // `lock_root` calls it
// cutover::apply
#[derive(Default, Serialize)] pub struct ApplyOutcome { pub written: Vec<String>, pub renamed: Vec<(String, String)>,
    pub unchanged: Vec<String>, pub refused: Vec<(String, String /* reason */)> }
pub fn apply(plan: &ExportPlan, git_root: &Path) -> ApplyOutcome;
// cli::cutover: CutoverAction gains Export(CommonArgs)
pub async fn evaluate_export(args: &CommonArgs, cwd: &Path, librarian_db: Option<OsString>, data_dir: Option<PathBuf>) -> Outcome;
/// The first of `cutover-report-<name>-<commit12>.md`, `…-2.md`, … that does not exist (with its `.json` twin).
pub(crate) fn report_paths(dir: &Path, repo_name: &str, commit: &str) -> (PathBuf, PathBuf);
```

**Apply rules.** Per `FileChange`, in path order: with `renamed_from`, the source must be a regular file (not a link: `symlink_metadata`) whose bytes equal `head_text`, the target must not exist, and both pass `refuse_catalogued_path_outside_repo` (as `mv.rs:233-240` does); then `std::fs::rename`. A source that is gone with the target present is a rename already done. Then the text: read the current bytes with `ReadDoor::text` (absent: only for `head_text: None`); equal to `new_text` → `unchanged`; equal to `head_text` (or absent for a new file) → create missing parent directories inside the root, then `write_catalogued_file(&[git_root], path, new_text)` → `written`; anything else → `refused` (`changed since HEAD: commit or revert it, then run again`). A refusal never stops the other files.

- [ ] **Step 1: Write the failing tests:**
  - `schema.rs` `the_new_catalog_sits_beside_the_old_one_and_shares_its_vector_cache` (`derived_catalog_beside(dir/catalog.db) == dir/derived.db`; `VectorCache::beside(Some(&new)).path() == VectorCache::beside(Some(&old)).path()`). `write_lock.rs` `lock_root_for_matches_lock_root`.
  - `apply.rs` (a temp git repo whose working tree equals `HEAD`): `apply_writes_each_changed_file_once`; `a_file_edited_since_head_is_refused_and_left_unchanged`; `a_second_export_finds_everything_written_and_writes_nothing` (bytes and mtimes equal; every path `unchanged`); `a_rename_moves_the_file_then_writes_its_new_text`; `a_rename_already_done_is_unchanged`; `a_rename_whose_source_is_a_symlink_is_refused`; `a_new_sidecar_creates_its_directory`; `a_path_that_leaves_the_root_is_refused`.
  - `cli/cutover.rs`: `export_refuses_when_the_new_catalog_exists`; `export_refuses_while_another_process_holds_the_write_lock` (hold `take_at` at `lock_root_for`); `export_refuses_when_a_side_file_appears_before_the_writes`; `export_writes_the_report_beside_the_old_catalog_and_prints_no_absolute_path`; `a_second_export_keeps_the_first_report` (`…-2.md` appears; the first is byte for byte); `the_json_report_parses_and_carries_the_tally`; `a_run_with_refused_files_exits_1`.
- [ ] **Step 2: Run** `scripts/with-slot.sh cargo test --lib cutover::apply`, `scripts/with-slot.sh cargo test --lib cli::cutover`, `scripts/with-slot.sh cargo test --lib derived::schema`, `scripts/with-slot.sh cargo test --lib cli::write_lock`. Expected: FAIL.
- [ ] **Step 3: Implement.** `evaluate_export`: `load_and_plan`; refuse when `derived_catalog_beside(old)` exists; take the write lock at `lock_root_for(project start, git_root)` with a budget of 250 ms (`LOCK_ATTEMPT`, not timeout 0); re-check `side_files_present` and `file_stamp`; `apply`; write both report files with `write_catalogued_file(&[], …)` at `report_paths`; print the report and the `ApplyOutcome` counts; last line `Commit the changed files, then run: codescout cutover build`. The `Cutover` help in `main.rs` names the three subcommands, the refusals of D-R8, and that the tool never commits.
- [ ] **Step 4: Run** the filters again. Expected: PASS.
- [ ] **Step 5: Commit** by pathspec: `feat(identity): cutover export applies the plan and keeps every report (D task 8)`.

### Task 9: `cutover build`, the manual page, the end-to-end test

**Files:**
- Modify: `src/cli/cutover.rs` (`Build`), `docs/manual/src/SUMMARY.md` (one line after `[Entry Citations]`)
- Create: `docs/manual/src/concepts/artifact-cutover.md`, `tests/cli_cutover.rs`

**Interfaces:**
- Consumes: `load_and_plan`, `CommonArgs`, `Outcome` (Task 2); `derived_catalog_beside` (Task 8); `build_full`, `rules_from_commit`, `key_space_from_commit` (`build.rs:120-161`).
- Produces: `CutoverAction::Build(CommonArgs)`; `pub async fn evaluate_build(args: &CommonArgs, cwd: &Path, librarian_db: Option<OsString>, data_dir: Option<PathBuf>) -> Outcome;`

**Build rules.** `load_and_plan`; when `plan.writes() > 0`, refuse with code 2: `the export is not committed: <n> files would change. Run codescout cutover export, commit, then run this again`. Else `schema::open(derived_catalog_beside(old))`, `build_full` with `HEAD`, its rules and key space (as `derive_shadow.rs:217-230` does), and print the `BuildReport` counts and `the new catalog is derived.db, in the directory of the old catalog`. A second build rebuilds in place.

**Manual page (STE).** `artifact-cutover.md`: what the three subcommands do and in which order; the migration window (stop every codescout process; the side-file refusal and the `sqlite3 … wal_checkpoint(TRUNCATE)` command); what the tool never does (commit, write the old catalog, rename or edit a frozen file); how to read the report (the four statuses); and before the run, extend `.codescout/key-space.toml` with subtrees whose file names another tool dictates. `tests/manual_toc.rs` checks the SUMMARY line.

- [ ] **Step 1: Write the failing tests:**
  - `cli/cutover.rs` `build_refuses_while_the_export_is_not_committed`; `build_writes_the_new_catalog_from_head`; `build_refuses_when_a_side_file_lies_beside_the_old_catalog`; `a_second_build_rebuilds_in_place`.
  - `tests/cli_cutover.rs` `the_cutover_exports_every_class_and_leaves_the_old_catalog_unchanged`, through `CARGO_BIN_EXE_codescout` and the `git` helper of `tests/cli_derive_shadow.rs:14-38`. The repo has two commits: commit 1 holds `docs/old/plan-x.md`; commit 2 renames it to `docs/plan-x.md` (same text) and adds `docs/ledger.md` (`entry_prefix: F`, `## F-1 — first`), `docs/log.md` (`single_writer: true`), `docs/shared.md` (no flag), `docs/My_Notes.md`, `docs/a/one.md` and `docs/b/two.md` (each defines `## F-7 — …`), `src/render_prompt.md` and `src/lib.rs` (`include_str!("./render_prompt.md")`). `docs/shared.md` cites `ledger:F-1`, `[notes](My_Notes.md)`, `[[My_Notes]]`, the 16-hex id of `/nonexistent/old-root/docs/old/plan-x.md`, and a bare `F-7`. The old catalog (made with `codescout::librarian::catalog::Catalog::open`; rows under `/nonexistent/old-root`; closed, then `PRAGMA journal_mode=DELETE` if a side file remains) holds: an augmentation of `ledger` with a prompt, an entry collection `entries: [F-1, F-2]` and one more params key `gather_from`; a note on `log`; a note on `shared`; one observation on `log` with no note; a `depends_on` link from `log` to `ledger`. Run `cutover plan` then `cutover export`, both with `--old-root /nonexistent/old-root --old-db <file> --json`.
  - Assert after `export`: `docs/ledger.md` has `## F-2 — ` and `expects_augmentation:`; `docs/augmentations/docs-ledger.yaml` exists; `docs/log.md` holds the note section, the `observation-<rowid>` section and `**Depends on:** [[ledger]]`; `docs/my-notes.md` exists and `docs/My_Notes.md` does not; `docs/shared.md` holds `[[ledger:F-1]]`, `[[my-notes|notes]]`, `[[my-notes]]`, `[[plan-x]]` and the bare `F-7`; `src/render_prompt.md` exists unchanged. Tally: `params_entry` moved 1 already_exported 1; `params_other` unrecoverable 1 (detail `gather_from`); `augmentation_shape` moved 1; `event` moved 1 and skipped 1 (`no committed home yet: not a single-writer document`); `observation` moved 1; `manual_link` moved 1; `rename` moved 1 and unrecoverable 1 (`src/render_prompt.md`, reason starting `frozen:` and naming `src/lib.rs:1`); `citation` moved 4 and one `unrecoverable` `bare_token` item for `F-7`. The `plan` run's tally equals the `export` run's. The old catalog's sha256 and directory listing are unchanged.
  - Then: a second `export` reports every file `unchanged`. After `git add -A` and `git commit`, `cutover plan` reports `writes` 0 and this exact tally: every `moved` count is 0; `already_exported` is params_entry 2, augmentation_shape 1, event 1, observation 1, manual_link 1, rename 1; every `unrecoverable` and `skipped` count equals the first run's except `citation`, whose four rewritten items no longer appear. `cutover build` exits 0 and `derived.db` lies beside the old catalog, and it holds: an `entry` row `(docs/ledger.md, F-2)`; an `entry` edge from `docs/shared.md` to `ledger` with token `F-1`; a `file` edge from `docs/log.md` with `dst_key` `ledger`; no `malformed` edge from `docs/shared.md`.
- [ ] **Step 2: Run** `scripts/with-slot.sh cargo test --lib cli::cutover`, `scripts/with-slot.sh cargo test --test cli_cutover`, `scripts/with-slot.sh cargo test --test manual_toc`. Expected: FAIL.
- [ ] **Step 3: Implement** the build rules and write the page. Re-read the page against STE.
- [ ] **Step 4: Run** the three commands again. Expected: PASS.
- [ ] **Step 5: Commit** by pathspec: `feat(identity): cutover build, manual page and end-to-end test (D task 9)`.

### Task 10: Gate (controller)

- [ ] **Step 1:** Run `./scripts/gate.sh` in the background on an untouched tree and end the turn. Expected: Python lane green, `fmt-mine.sh` clean, clippy clean in both feature sets, lean and default nextest green, doc tests green; `tool_surface_under_budget` green at 57_664.
- [ ] **Step 2:** Record the execution state (commits, rulings that changed, deferred findings) in a state document, as C1 to C3 did.

## Self-review

1. **Spec coverage.** Step 1 (export): params entries T6, augmentation shapes T6 (with drift, D-R24), other params reported T6 (D-R23), notes, verdicts, intents, observations T7, manual links T7; write cites reported T2. Step 2 (rename): T5, with D-R21 and D-R26 blockers. Step 3 (rewrite): `<stem>:TOKEN`, path links, 16-hex ids (catalog and history bound to the hashed path), one-owner bare tokens, ambiguous ones reported: T4; renamed keys T5. Step 4 (build a new file, old one read-only): T9, T1. "Export and rewrite before the read flip": `build` refuses until a plan of `HEAD` writes nothing. Store-boundary rows without a home: T2 reasons. Single-writer rule: T7. Migration window: D-R8, T1, T2, T8. "A rename leaves no `[[old-key…]]`": T5 and the e2e. C3 follow-up 5: D-R4, T8 test. F-25: T4, T5, e2e. F-26: T6, e2e `derived.db` rows. F-27: D-R2, D-R25, T5, T7, e2e post-commit tally. F-28: T3, T4. F-29: D-R29, D-R13, T1 text, D-R22, D-R8, D-R17, T1 `NoOldRowsUnderRoot`, D-R30, D-R28 and T1 (`-journal`).
2. **Step scan.** Every code step names its file, signature, and the values the spec or the rulings pin. Bodies are left to the implementer except the rewrite rules, the history binding and the apply rules.
3. **Types.** `RootMap`, `open_old_immutable`, `side_files_present`, `file_stamp`, `resolve_old_catalog` (T1) → T2 (`load_and_plan`), T8. `OldRows` and its row types (T2) → T4 (`documents`), T5 (rename maps), T6 (`augmentations`), T7 (`events`, `observations`, `links`). `Class` (with `ParamsOther`), `Status` (with `AlreadyExported`), `Item`, `FileChange`, `ExportPlan`, `ClassTally` (T2) → T4 to T9. `CommonArgs`, `Outcome`, `CutoverAction` (T2) gain `Export` (T8) and `Build` (T9). `extract_spanned`, `citation_spans`, `link_target` (T3) → T4. `CodeRefs` (T4) → T5, T6, T7 (frozen checks). `Resolver.renamed_keys` (T4) is filled in T5. `fence_for` (T6) is used by `entries.rs`. `DERIVED_CATALOG_FILE`, `derived_catalog_beside` (T8) → T8, T9.
4. **Review Focus.** Line 1: T8, T9. Line 2: T4, T5. Line 3: T4. Line 4: T1, T9. Line 5: T4, T6. Line 6: T7.
5. **Proportion.** Eight implementation tasks; the longest (Task 4) is about 70 lines of plan text; most text is test names and assertions.

## Facts verified (read this session), and what this plan corrects

- **D-1 holds.** `DeriveShadowArgs` has four fields (`derive_shadow.rs:51-65`); `diff_with_rules` returns `NoOldRowsUnderRoot` when rows exist and none is under the root (`diff.rs:418-427`); scope is `scope_of` by path components (`diff.rs:550-570`), also called by `cite_text.rs:73` and `:262`.
- **D-2 is verified by bodies.** `write_entry_into_ledger` writes the old catalog through `record_written_file` (`dual_write.rs:176`, `UPDATE artifact` at `:58-66`); `event_create`'s twin path calls it too (`event_create.rs:637`). `export_augmentation_sidecars` (`doctor.rs:2025`) only reads the catalog. Plain readers take `&Catalog`, and `Catalog::from_conn` is private (`catalog/mod.rs:746`), so the cutover reads with its own SQL on the read-only connection, as `diff.rs` does.
- **D-3 is stale** (C2 landed: `event_text.rs`, `link_text.rs`). **D-4 is filed** as bug `67d5983be664de6a`. **D-5 holds** (`diff.rs:111-121`). **D-6 holds**: no history helper exists; `artifact_id_from_abs` hashes the `RepoPath` form (`ids.rs:17-23`). **D-7 holds** (`build.rs:76-83`). **D-8 refined** (D-R2).
- **The brief's D-R6 target list is wrong for ledgers and entry documents** (D-R12, `event_create.rs:385-392`). **`grammar_form_of` cannot serve the cutover** (D-R11).
- **W-13, scratch experiment (SQLite 3.54.1):** a cleanly closed WAL database opened with `immutable=1` reads every row and creates no side file; with a writer holding it and a row only in the `-wal`, the immutable open reads 1 row where a plain `mode=ro` open reads 2, with no error. So the side-file refusal (D-R8, D-R16) is the only guard, and the plan checks it three times. A live WAL connection keeps both side files, so a live server is visible.
- **W-13, seams confirmed:** `open_old_read_only` (`diff.rs:372`), `refuse_if_open_would_write` (`derive_shadow.rs:115`), `splice_pending_section` (`augmentation.rs:2362`), `body_defined_indices` (`:2767`), `TWIN_KINDS` and the ledger refusal (`event_create.rs:336, 385-392`), `splice_event`, `parse_event_sections` (`event_text.rs:383, 297`), `label_for_rel`, `splice_relation` (`link_text.rs:23, 44`), `link_target` (`extract.rs:698`), `BuildInput` (`build.rs:76-83`), `VectorCache::beside` (`vector_cache.rs:268`), `TOOL_SURFACE_CHAR_BUDGET` (`server.rs:6625`), and every old-caller test named here.
- **F-25, checked this session:** `include_str!` reads `workspace_onboarding_prompt.md` (`src/prompts/mod.rs:1088`), both `render_prompt.md` (`audit_doc_refs/mod.rs:966, 980`; `legibility_scan/mod.rs:424, 434`), `docs/PROGRESSIVE_DISCOVERABILITY.md` (`server.rs:2972`) and `docs/issues/_TEMPLATE.md` (tests, `prompts/mod.rs:2744`, `doctor.rs:25019`); `scripts/measure/judge.py:55` names `judge_prompt.md` from its own folder; no file names `.github/ISSUE_TEMPLATE/bug_report.md` or `pull_request_template.md`, so D-R26's dot-directory rule (not a code search) protects them; `docs/manual/book.toml` sets `src = "src"`. `.codescout/key-space.toml` excludes none of these.
- **F-26:** the sidecar cannot carry params (D-R23). **F-29 (1):** `frontmatter::write` re-serialises the block (`frontmatter.rs:174-219`), so D-R29 tests parse equality. **F-29 (2):** the payload fence rule is `event_text.rs:178-183`. **F-29 (3):** no repository command checkpoints a catalog; `wal_checkpoint` appears only in tests (`migrate_v6.rs:502`, `indexer.rs:6447`), so the refusal names the `sqlite3` command. **F-29 (7):** `NoOldRowsUnderRoot` prints `git_root.display()` (`diff.rs:330-341`); Task 1 removes it. **F-29 (8):** `lock_root` is the nearest `project.toml` (`write_lock.rs:31-41`); D-R30.
- **The derived index holds no vectors and no event, observation, augmentation or relation-label table** (`derived/schema.rs:31-108`). Task 9 asserts the rows it does hold.
- **The old link scanner reads inline code** (`link_scan/extract.rs`, `Event::Code` arm in `extract`); the grammar does not (`grammar.rs:98-159`). Its `cross_repo_re` is lowercase only (`extract.rs:287-290`).
- Not re-measured: the spec's 38 params entries, 73 events, 57 observations, 13 write cites and "160 of 335" hex ids. `cutover plan` measures them.

## Still open (the operator or `codescout-37`)

1. Which subtrees to add to `.codescout/key-space.toml` before the run (candidates from F-25: `src/prompts`, `.github`, `scripts/measure`, `docs/manual/src`); D-R26 freezes them either way, so the run is safe without it, but their files keep invalid keys.
2. Key collisions and non-canonical folder names: the report suggests names; the operator renames by hand.
3. Events and observations on shared or unflagged documents stay rows until `codescout-37`'s entry-document tool exists (operator ruling 2026-11-14).
4. Params keys outside entry collections (D-R23) have no committed home; the operator decides whether any of them needs one before Plan E deletes the rows.
5. The `params-entry` block and the export-date validity stamp (D-R13) may need `codescout-37`'s ledger format later.
6. Tokens held only in inline code or in headings (C1 open question): counted, not rewritten.
7. Bug `46099dbdb68e0d7b`: an exported event writes `source.uri` and `author` unchanged, as `event_create` does.
8. Vectors for `derived.db`: Plan E wires embedding; before it, run `reindex` with `reembed=true` per project (C3 precondition) or ship seeding.
9. `.MD` (uppercase extension) files are not listed by `scan_commit` (`an_uppercase_md_extension_is_not_listed`, `scan.rs:320`), so the cutover neither renames nor rewrites them.
10. Write-time cites (`origin='write'`): report-only.
