---
id: '82be9a55b2fe0746'
kind: research
status: active
title: Plan E2 — execution state (landed on integration at 56dd96fe3)
owners:
- marius
tags:
- identity
- plan-e2
- state
- landed
---

# Plan E2 — execution state (landed on integration at 56dd96fe3)

Updated 2026-11-16 after the landing. Plan `docs/superpowers/plans/2026-11-16-artifact-identity-plan-e2-derived-schema-v2.md` (`dc0d65557a9577ee`, 10 tasks, rulings E2-R1..R16), plan review `9a89ced57da874b7`, scout `02d716631c64e393` (Q1-Q8), E1 state `6ba1be1f51416c0e`.

## What landed

- Branch `feat/e2-derived-schema-v2-20261116` (worktree `.worktrees/e2-derived-schema-v2-20261116`), tip `56dd96fe3`: 14 task commits and 3 final-wave commits. Integration (`integration/identity-lifecycle-20261114`) was fast-forwarded from `725eea39a` to `56dd96fe3` in `.worktrees/integration-identity-lifecycle`. The lifecycle fork was told before and after (peer `Organizare și arhivare docs/trackers [503554]`; the old ref `[cafcab]` no longer resolves). Nothing is pushed. `experiments` held docs only at that time. It received E1 and E2 on 2026-11-16 as merge `7f801fddc` (a fast-forward from a temp branch, so other sessions' staged files stayed untouched). The lean gate lane is red on three #34 `worktree_store_tests` that also fail on the plain `experiments` tip (bug `da6b6a00432b1736`); the operator accepted that as a recorded exception. Ruling Q4 allows E1-E3 on `experiments`; E4-E8 stay off `experiments` until the operator runs the cutover window.
- The derived catalog is schema v2: tables `event`, `entry_doc`, `augmentation`, `history_meta`, `doc_history`, `worktree_layer`; column `edge.rel`; column `parse_cache.facts_json`. A v1 file with exactly the eight v1 tables is rebuilt on open; any other older file is refused ("not a new catalog").
- Task 1 schema v2. Task 2 event sections to `event` rows. Task 3 relation labels (`edge.rel`, canonical rel = snake_case of the label words). Task 4 entry documents (`entry_doc`; problem kind `entry_unparsed`). Task 5 augmentation shapes read from the commit's sidecars. Task 6 `git::diff_trees` and test helpers. Task 7 first-parent history cache (`history.rs`). Task 8 working-tree layer over HEAD (`worktree.rs`, `update_with_working_tree`, no production caller). Task 9 `doc(link)` writes sorted relation lines and one `is_superseding` predicate. Final wave: history boundary on missing objects, `sqlite_` escape, module docs, extra tests.
- Gate on the exact tree: `CODESCOUT_GATE_TMPFS_BASE=/dev/shm ./scripts/gate.sh` green (FMT 0, CLIPPY 0, LEAN 0, DEFAULT 0; nextest 10,809 and 10,904 passed; Python 1,836 passed). The same tree was red without that knob (41 unrelated test names) because empty `.git` directories again sat at `/tmp/.git` and `~/.cache/cc-tmp/.git` (both created 2026-11-16 16:21): bug `3c5fa35d9d2754a7`. The creator is still unknown.
- Q4 check: the production names `derived_catalog_in`, `DERIVED_CATALOG_REL`, `schema::open(` and `update_with_working_tree` appear only in `src/cli/cutover.rs`, `src/cli/derive_shadow.rs` and `src/librarian/derived/`. No file under `src/prompts/`, no `.codescout/system-prompt.md` and no tool definition changed. `TOOL_SURFACE_CHAR_BUDGET` is untouched (57_664).
- Bugs closed with a fix anchor: `b2ec2d2e35fbbf30` and `133504302d03e590` (fix `dfb931dbb`), `84e6e61e03beace6` (E2-R4, `fab06a181`). Their SHAs name the integration commits.

## Measurements (E2-R15)

Release build, this repository at `56dd96fe3` (2,633 documents).

- History walk (a): **15.0 s** (14,977 ms and 15,058 ms in two runs; 13,511 rows over 2,814 paths; deepest `walk_pos` 8,127; `shallow` false). A debug build took 32.7 s; the scout's `git log` equivalent took 0.68 s. This is over the 5 s line, so it is a **blocker for any plan that puts `build_full` or `incremental::update` to a new commit on a write path**. One `cutover build` or `derive-shadow` run stays usable. Options for E4/E6: walk only the new segment when the stored commit is a first-parent ancestor of the new one (shift `walk_pos`, remap renamed heads), or store the walk outside the write path.
- Working-tree layer (b), clean tree: status 22.9 ms, read 0.0 ms, rebuild 925.7 ms, 0 layer rows. Dirty tree (500 untracked and 20 modified `.md`): status 25.3 ms, read 7.5 ms, rebuild 932.6 ms, 520 layer rows. The status median is under 100 ms and status plus read is under 2 s, so this is **not an E6 blocker**. A debug build gave 28.7 and 33.7 ms for status. `rebuild_from` is out of scope for the threshold.
- Derived index of this repository: 2,633 documents, 2,258 entries, 4,936 edges (kinds: local 4,732, malformed 144, file 43, unknown_key 15, entry 2), 47,722 chunks, 181 skipped (all `outside_key_space`), 43 problems (`duplicate_key` 26, `invalid_key` 17). `event` 0, `entry_doc` 0, `augmentation` 25, `doc_history` 13,511. Edges by rel: none 4,930, `architecture` 1, `bugs` 3, `observed` 1, `related` 1. No committed document holds an event section or `entry_kind` frontmatter yet.

## Rulings made during E2 (full text with costs: the ledger `.superpowers/sdd/2026-11-16-artifact-identity-plan-e2-derived-schema-v2/progress.md`)

- F1-F14 from the pre-flight scan, applied as stated in the previous version of this note. F17 (new): the history walk never decides a path's fate from delta order; `tracked` holds regular-file `.md` blobs only (modes 100644 and 100755); a file<->symlink change at one path arrives as `[Deleted, Added]`.
- Task 6: `diff_trees` behaviour stays, with the pathspec kept (full-tree rename detection over about 9,000 commits costs more; `doc_history` is a display cache, not identity). With pathspec `*.md`, renames pair only among matching paths: two renames that each leave or enter `*.md` can mispair. Cost: a rare mispaired history row.
- Task 7: the first ruling made any walk failure except the boundary fail the build. **The final review reopened it:** an object-NotFound while reading a tree, a parent or a diff is a boundary (stop, keep the rows found, `shallow = true`). Cost: a partial clone gets a truncated history, flagged only by `history_meta.shallow`, which now means "bounded". The incremental walk is parked to E4/E6.
- Parked to a later plan (stay): see the next section. All other Minor findings were fixed in a fix round or in the final wave. The final scoped re-review found no Important issue.

## Known limits and deferred findings

- History (`history.rs`): `shallow = 1` covers two shapes. A shallow-clone boundary gives every tracked document an `added` row at the graft commit. A partial-clone boundary can leave a document with no row at all (blobless rename case: `b.md` has no rows; treeless case: `a.md` has only its `modified` row). **E4 must not read "no rows" or "no `added` row" as fact when `shallow = 1`.** `is_absent_object` (class Odb, code NotFound) cannot tell a partial clone from a repository that lost an object: such a repository builds with a bounded history and no complaint. A tighter rule (require `extensions.partialclone` or a promisor remote) is E4's call. Two raw names with one `/` spelling (`a\b.md` beside `a/b.md`) leave neither tracked. `status` is the raw frontmatter value, not the rule-derived status. libgit2 error text can hold an absolute path, and `walk` passes it through (true before E2 for any git error).
- Layer (`worktree.rs`): a layer-deleted document keeps its `doc_history` rows with no `document` row (E2-R13). A file swapped for an in-repo link between the link check and the read is read as text (the door still blocks any escape). A directory at a tracked `x.md` path fails the whole update with a door refusal. An untracked non-UTF-8 `.md` name loses its skip row. libgit2 reads the machine's git config: the tests pin `core.autocrlf` and `core.excludesFile` per clone, not the system config or `core.ignorecase`. A layered text never poisons the parse cache (keys include `sha256` and `rel_path`); the next commit-only update re-parses the edited file.
- Sidecars (Task 5): the shape is read from the commit on every rebuild, never cached. A lexically refused sidecar's problem row holds the declared string in `rel_paths` (it may be an absolute path written in the committed text; the only other absolute value stored is `checkout.root`). Any `blob_text` error becomes `sidecar_invalid` (an object-database read error heals on the next build, but two clones of one commit could dump differently). Path components follow the OS (`docs\\a.yaml` differs between Linux and Windows). A layer-edited sidecar is invisible (open item 2).
- Events and entries: a malformed event section is silently dropped, as in the live reader (Task 2 m-2; E4 decides). No events count in `BuildReport`. `entry_doc` follows "present and not null"; other readers decide differently: `event_text.rs` counts a null `entry_kind`, and the live `LifecycleTree::build` decides by collection placement and counts broken frontmatter as unparsed. `relation_rels` stores a rel for every paragraph line (34,338 entries over `docs/`, only 6 on a line with a File or Entry citation, about 0.6 MB of `facts_json`); nothing reads the extra entries.
- `doc(link)` (Task 9): a CRLF document whose greater line is last and unterminated gets LF on the inserted line (the append path writes LF as before, shared with events). A row stored before the change keeps the spelling it was written with, so a repeated `Supersedes` link adds a second `supersedes` row (`INSERT OR IGNORE` keys on rel).
- Layering to revisit at E6: `derived` depends on `tools::doctor` (E2-R9); `derived::build` and `derived::worktree` import each other (one module in practice); `git::tests` import `derived::testkit`. A similarity rename whose blobs are absent now bounds the walk (blobless clones) instead of failing it.
- Tests with no pin (structural properties): the walk and the sidecar reads run before the write transaction; the `settle` call site in `rebuild_from`; the `is_shallow()` guard in `reusable` on its own; `git_exit` output order.
- Environment: the stray `.git` directories (bug `3c5fa35d9d2754a7`) turn about 40 unrelated tests red under the default `TMPDIR`. Workaround: `CODESCOUT_GATE_TMPFS_BASE=/dev/shm ./scripts/gate.sh`, and `TMPDIR=/dev/shm scripts/with-slot.sh cargo test ...` for targeted runs (the clone-based layer test needs `--no-hardlinks` there).

## Duties and open items for the next plans

- E3 (key addressing, additive) is next. Then E4 (first production reader of `derived.db`): the E1 duty list (check `checkout.root` against the reader's own git root; open `<lookup_git_root(start)>/.codescout/derived.db`; a writer runs `refuse_unsafe_catalog_path` and `refuse_unless_ignored` first, moved into `librarian::derived`; the non-Unix hard-link gap; edit the E1-R9 tripwire and the `BuildInput` text tripwire deliberately) plus the history semantics above.
- E4 decides entry fields for section ledgers: 16 of 25 committed sidecars declare an `entry_collection` whose rows live only in catalog params, and no committed document has `entry_kind` frontmatter. E2 indexes entry documents only.
- E6 extends the working-tree layer to every declared sidecar path when `doc(augment)` becomes file-only (`parse_declaration` accepts a sidecar at any `.yaml` or `.yml` path); E6 also decides the refresh time (no commit trailer exists) and wires `update_with_working_tree`.
- E5 owns the machine-state file and vectors keyed by chunk; E8 owns gc settings and audit watermarks.
- Operator, decided 2026-11-16: integration merged into `experiments` as `7f801fddc`; the Plan D, E1 and E2 branches and worktrees are deleted, and their SDD workspaces now live in the main checkout's `.superpowers/sdd/` (git-ignored, local). Still open: when to run the cutover window.
- Plan D follow-ups: bug `28c8a999b2f3132b`, the observation id collision (M54), the entry-document tool timing owed by codescout-37, `status_since` missing from exported ledger entries.

## How E2 ran (for the next plan)

Per task: `task-brief` -> Sonnet implementer -> `review-package` -> Opus reviewer (reconnaissance; snow-lion where boundaries move) -> one fix round by resuming the implementer -> scoped re-review. Whole branch: Opus review, ONE Opus fix wave, one scoped Opus re-review, then the gate in the background. Reviews found real defects in 5 of 9 tasks (Task 6 typechange and pathspec claims, Task 7 blobless clone, Task 9 stale second predicate, the treeless clone at the end). Two reviewer claims did not reproduce (the typechange entry order); the re-reviewer measured it. Script dir: `/home/marius/.claude-stef/plugins/cache/superpowers-marketplace/superpowers/7.0.2/skills/subagent-driven-development/scripts`; run the scripts with the plan's absolute path when the plan is on `experiments`.
