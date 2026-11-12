# Tracker lifecycle: what is built, and what is parked (updated 2026-11-12, after the cleanup merge)

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

## Parked, not in any plan yet
- A `Finding::subject()` accessor would help the doctor (L-B2).
- `portable_reason` strips the root with a literal `/` (Windows, symlinked root): display only.
- `Promoted  from` (double space) counts as forward; label vocabulary belongs to the builder.
- Plan L-B2 (tools) is blocked until identity Plan C exists (identity B1 and B2 plans are on `experiments`; B2 is running in `.worktrees/artifact-identity-plan-b2`).
- Plan L-E (migration): `statements::parse_rests_on` should read through `relations()`; `is_bold_field_label` is duplicated in `relations.rs` and `statements.rs`; the loose file `docs/trackers/temp/artifact-identity-refactor.md` must move to `temp/artifact-identity-refactor/readme.md`; legacy `[[collection:R-91]]` citations: the identity session agreed its cutover rewrite emits `[[entry-key]]` for tokens that become entries.
- `table::render_markdown` escaping edge cases (title `\[a](b)` makes a working link; `\|` loses its backslash) matter only if a table is written to a file; bare tokens like `R-91` in a title still parse as `Reference::Bare`.
- No default `review_every` exists; writer collections should use `key_style: name`.
- `tests/worktree-remove.sh`: a failed exit assertion prints `want` and `got` but not `$ERR`. Bug `3c919d925fa383fe` (one red gate run, cause not established) would have been diagnosable with it. Not applied.

## Process lessons
- On this shared checkout, merge into `experiments` by merging it INTO the feature branch inside the worktree, then `git merge --ff-only` in the main checkout (a real merge there would commit a peer's staged paths). `experiments` moves every few minutes: merge and fast-forward back to back. If it moves during the gate and the new commits are docs only, merge them and fast-forward without a second gate run.
- `./scripts/fmt-mine.sh` refuses lifecycle files with a shared-workspace attribution message; `rustfmt --edition 2021 <file>` works and the commit hook checks `rustfmt --check`.
- The gate's lean lane can fail `migrate::sqlite_memory_vectors::hardening_tests::empty_rebuild_still_rechecks_every_backup_byte_before_commit` when Python runs beside it (known load-sensitive: docs/issues/2026-10-31-eight-lib-tests-fail-under-16-to-1-cpu-oversubscription-through-wall-clock-windows.md). It passed 5/5 alone and on a re-run; re-run once before investigating.
- Removing a finished worktree is blocked by untracked codescout files: see docs/issues/2026-11-12-git-worktree-remove-refuses-after-codescout-and-agent-briefs-leave-untracked-files-in-a-worktree.md. Never name a subagent scratch file outside `.superpowers/` (git-ignored).
