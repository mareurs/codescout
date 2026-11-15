# Tracker lifecycle: what is built, and what is parked (updated 2026-11-14, after tools part 1 landed on the integration branch)

Plans L-A (core), L-B (core completion), a follow-up (30-day grace, ledger-only `terminal_in_live_dir`) and the lifecycle cleanup plan (`docs/superpowers/plans/2026-11-12-tracker-lifecycle-cleanup.md`, spec `…/specs/2026-11-12-lifecycle-cleanup-design.md`) are on `experiments` (cleanup merged fast-forward, tip 3c7a2d3d9). Code: `src/librarian/lifecycle/` (layout, schema, entry, relations, naming, tree, checks, checks/tests, table, scan, testkit). Spec: `docs/superpowers/specs/2026-11-11-tracker-lifecycle-design.md`. Plans: `docs/superpowers/plans/2026-11-11-tracker-lifecycle-plan-{a-core,b-core-completion}.md`.

## Pinned contracts (do not change without telling the identity session)
- Eight check names: entry_invalid, revisit_due, cited_target_closed, stream_all_terminal, collection_invalid, ledger_quiet, terminal_in_live_dir, orphan_entry (`checks::CHECK_NAMES`).
- `Finding` serializes internally tagged `{"check": "<name>", ...}`; nested enums snake_case, externally tagged. `CollectionInvalid.subject` is always a repo-relative file path.
- `Display for Finding` is one line, control characters escaped.
- `identity::index::DocInput` is the single document-input type (identity session's rule).

## Decided (measured 2026-11-11 on docs/issues, entry W-3 in tracker-organization-session-log)
- Default grace for `terminal_in_live_dir` is 30 days (69 findings at 14 days, 1 at 30).
- `terminal_in_live_dir` applies to ledgers only; a stream is archived whole.
- `cited_target_closed` covers every non-terminal entry (blocked and deferred included).
- Two readmes in one collection folder: the first in path order is the readme; every other one is `TreeProblem::DuplicateReadme(rel_path)`, reported as `collection_invalid` with problem `duplicate_readme`. A duplicate's frontmatter is parsed once, so its `closed:` still counts. Consequence (ruling R1, F-2 in the session log): a broken first readme now fails the collection even when a second readme is good.

## Closed by the cleanup (SHA / patch-id prefix)
- Duplicate readme: 1f6c2876d / 10fda70e26dc.
- `TreeProblem::Schema` is `{ rel_path, errors }`, no `collection`: ded35ad3f / ac222daa5c80.
- `CheckPolicy` rustdoc points at W-3; the ledger-scope check is an exhaustive `match` on `Lifecycle`: 7f79b8b7c / 61fc37e235d9.
- `scan.rs` tests run git with `LC_ALL=C`; one test asks for German to pin it: 943afee9e / 2123926a66d8.
- `checks.rs` tests moved to `checks/tests.rs`: a30b674b6 / 31001c8e9b53.
- Wording fixes from the reviews: 4d9a90d04 / 5c62af639a2e and 9939b4829 / 325603fba89d.

## Built: the write core (merged 2026-11-14, fast-forward to 941a5ad19)
Pure library functions in `src/librarian/lifecycle/write.rs` (tests in `write/tests.rs`): `entry_template`, `new_entry`, `change_status`, `change_fields`, `WriteProblem`; tools part 1 (next section) added `update_entry`. No I/O, no catalog. Plan `docs/superpowers/plans/2026-11-14-tracker-lifecycle-write-core.md`, spec `docs/superpowers/specs/2026-11-14-lifecycle-write-core-design.md`. Every `Ok` text is re-read through `frontmatter::parse`, `entry::parse_entry` and `entry::validate_entry`.

## Built: lifecycle tools, part 1 (on `integration/identity-lifecycle-20261114`, NOT on `experiments`; landed 2026-11-14, `18e0ad3c2`)
Plan `docs/superpowers/plans/2026-11-14-tracker-lifecycle-tools-part-1.md`, subagent-driven, 11 tasks plus one fix wave; gate green on the merged tree (lean 10277, default 10372, python 1836). Commits carry `Session-Id: 31731474-…` (the lifecycle session). What it adds: the combined `update_entry` and the revisit rule, the reserved-field-name rule, `load_collection`/`load_collection_dir`, `collection_readme`, an exclusive write helper (`insert_and_create_exclusive_with`, create.rs), `doc(create)` entry mode and collection mode, `doc(update)` on an entry (routes by `layout::place` giving `Role::Entry`), `doc(get)` on a collection readme (entry table; a body selector `full`/`heading`/`headings`/`start_line`/`end_line` returns prose plus entries and no table). Path helpers `root_and_rel_path`, `rel_path_below`, `unrooted_placement`, `containing_root`, `windows_comparable` live in `tools/mod.rs`; never use `Path::strip_prefix` on a catalog abs_path (Windows `//?/C:/..` spelling). The read-door gate flags any `OpenOptions::new()` in production code: a write-only use needs an `ALLOWED` entry in `read_door.rs` (create.rs has one). Tool surface: 57599 of `TOOL_SURFACE_CHAR_BUDGET` 57664 (65 left). Collections cannot be tagged at create (refused); `doc(update)` sets tags afterwards. Per-task reviews run no suite: a green per-task review proves nothing about the gate-only tests (read_door was red until the gate ran).
Landing rule agreed with the identity session (both operators' answers): Plan D and these tools land on the shared integration branch; `experiments` gets ONE merge from it when the operator says all work is done. Each line: merge integration into its branch, gate that tree, announce by message, `git merge --ff-only` from `.worktrees/integration-identity-lifecycle`, announce again. Docs-only commits (plans, memories, bug files) still go on `experiments` and flow into integration by merging `experiments` into it.

## Parked from tools part 1 (rulings were in the SDD ledger, which dies with its worktree; this is the copy)
Rulings that cost something if wrong:
- Windows: the nested-worktree narrowing (`door_root` on the first containing root) probably does not apply when the catalog spelling is `//?/C:/..` and the root is native `\\?\C:\..`; the nested-worktree entry then takes the generic path silently. Unverified, no test.
- `extra:{}` + `body_edits` on an entry takes the generic path and re-serialises the frontmatter (key order, YAML comments, flow lists become block lists; values kept). The same patch did this before tools part 1. One-line fix: drop an empty `extra` before the generic path. `extra:{}` + `commit_refresh` is refused as EmptyUpdate (the old code did too).
- A lifecycle-looking path with no `.git` between the root and the file still takes the generic path.
- An unreadable `.md` in the loaded folder fails the load, so one bad sibling blocks a status update on another entry of that collection (spec-consistent with scan.rs).
- `resolve_status` order changed (Task 6): a bad bug status plus another fault reports the status error first. Cheap restore: default status in the call, `resolve_status` in `write_new` after `enforce_on_create`.
- Collection create refuses tags; a collection cannot be tagged at create.
- A bare `doc(get)` on a readme whose file is unreadable is refused (one extra call, `full=true`), where before it returned prose.
Open defects, none loses data:
- Key uniqueness is check-then-write: concurrent `ledgers/k` and `streams/k` creates both pass (the doctor can see it). A narrow cross-process window lets a reindex attach the loser's slug or augmentation to the winner's file.
- `path exists: <rel>` has no remedy and is wrong for a row with no file (create.rs ~768); tags are cleaned on update but not on create; `revisit_by: null` is refused on update and accepted on create; a null-valued required field reads `missing field X`.
- `NotACollectionDir`/`Unreadable` texts print the machine root; `portable_reason` is duplicated (collection.rs, scan.rs), has drifted, and misses Windows `\`; `TreeProblem` Display does not escape control characters in paths.
- A collection folder that is a symlink out of the root is walked before it is refused; `has_readme` hides I/O errors; the BadKey text is wrong for a hex16 key.
- Wording and docs: Task 7 remedy texts (BadKey echoes a raw key, Ambiguous/NotFound tell an entry create to pass a folder, the top-level refusal reads awkwardly); Task 9 archived-collection remedy, `UpdatePatch` fields not destructured exhaustively, `original` is the post-update text for the shrink-guard baseline, `problems_text` belongs in `lifecycle/write.rs`, one doc comment misses a refusal; Task 10 `librarian.md` filter paragraph, `entry_total` described as matches (it counts rows considered), the stale-row `full=true` remedy points at a missing file; `SCHEMA_KEYS` doc claims readme order but render is BTreeMap order; `new_entry` accepts a title with a newline (collection create refuses it).
- Test gaps: `contains(...)` assertions in `get_table_tests.rs` that would also pass if a heading were ignored; a census blind spot for a future `skip_serializing_if` field on `EntryRow`; `dirs.sort()` untested; no test of the take-back cascade with augmentation; a few missing blank lines (write.rs, schema.rs, table.rs, collection.rs, get_table_tests.rs).

## Open design inputs for the L-B2 plan (from the write-core final review)
- I-1 (closed by tools part 1: `update_entry` is the one combined update), I-2 (closed: a past `revisit_by` is refused), I-3 (closed: `valid`, `tags`, `title` are settable; `entry_kind` stays fixed), M-4 and M-5 (closed: problem texts name the remedy; a blank body is the template). Still open: M-6 tags normalisation on create (see Open defects), M-7 reopening an archived entry gives `NonTerminalArchived` (the remedy text for an archived collection is a Minor above).
- Cosmetic, not fixed: `NewEntry` has no `Debug`, `set_date` should be private, the lifecycle `mod.rs` doc does not mention writing, `write/tests.rs` is long (split along its section markers if more tests arrive).
- Rules held by the core: Call A (`change_fields` may delete a stray key the document carries, refuses one it does not); Call B (`change_status` stays strict about a stray key).

## Parked, not in any plan yet
- A `Finding::subject()` accessor would help the doctor (L-B2 part 2).
- `portable_reason` strips the root with a literal `/` (Windows, symlinked root): display only.
- `Promoted  from` (double space) counts as forward; label vocabulary belongs to the builder.
- Lifecycle tools part 1 is built (section above). The tools that address a collection by key and `doc(move)` as a key rename wait for identity Plan E: every `doc` action still takes a 16-hex `id` (F-3 in the session log); per the operator (2026-11-14) a note on a shared document stays a catalog row until the entry-document tool exists. C1, C2 and C3 landed; D is being built (feat/cutover-tool-20261114); E is not planned.
- Plan L-E (migration): `statements::parse_rests_on` should read through `relations()`; `is_bold_field_label` is duplicated in `relations.rs` and `statements.rs`; the loose file `docs/trackers/temp/artifact-identity-refactor.md` must move to `temp/artifact-identity-refactor/readme.md`; legacy `[[collection:R-91]]` citations: the identity session agreed its cutover rewrite emits `[[entry-key]]` for tokens that become entries.
- `table::render_markdown` escaping edge cases (title `\[a](b)` makes a working link; `\|` loses its backslash) matter only if a table is written to a file; bare tokens like `R-91` in a title still parse as `Reference::Bare`.
- No default `review_every` exists; writer collections should use `key_style: name`.
- `tests/worktree-remove.sh`: a failed exit assertion prints `want` and `got` but not `$ERR`. Bug `3c919d925fa383fe` (one red gate run, cause not established) would have been diagnosable with it. Not applied.

## Process lessons
- On this shared checkout, merge into `experiments` by merging it INTO the feature branch inside the worktree, then `git merge --ff-only` in the main checkout (a real merge there would commit a peer's staged paths). `experiments` moves every few minutes: merge and fast-forward back to back. If it moves during the gate and the new commits are docs only, merge them and fast-forward without a second gate run. For the integration branch, the same route applies with `integration/identity-lifecycle-20261114` in its own worktree.
- `./scripts/fmt-mine.sh` refuses lifecycle files with a shared-workspace attribution message; `rustfmt --edition 2021 <file>` works and the commit hook checks `rustfmt --check`.
- The gate's lean lane can fail `migrate::sqlite_memory_vectors::hardening_tests::empty_rebuild_still_rechecks_every_backup_byte_before_commit` when Python runs beside it (known load-sensitive: docs/issues/2026-10-31-eight-lib-tests-fail-under-16-to-1-cpu-oversubscription-through-wall-clock-windows.md). It passed 5/5 alone and on a re-run; re-run once before investigating.
- Removing a finished worktree is blocked by untracked codescout files: see docs/issues/2026-11-12-git-worktree-remove-refuses-after-codescout-and-agent-briefs-leave-untracked-files-in-a-worktree.md. Never name a subagent scratch file outside `.superpowers/` (git-ignored).
- Ask the owner of a branch by its commits: every commit made by a Claude session carries a `Session-Id:` trailer (`git log --format='%(trailers:key=Session-Id,valueonly)'`). A guessed owner was wrong once; the trailer was right.
- A peer-session name changes after compaction (this session was `codescout-7e`, `-4a`, `-51`; the identity session `codescout-2a`, `-5f`); the session id does not. Take the SID from an Enumerate row.
