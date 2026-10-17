---
status: fixed
opened: 2026-10-17
closed: 2026-10-17
severity: high
owner: marius
related: [docs/issues/2026-10-17-the-librarian-applies-no-read-deny-list-to-a-catalogued-outward-link.md, docs/issues/2026-10-17-doc-create-on-a-dangling-symlink-creates-a-file-outside-the-root.md, docs/issues/2026-10-17-edit-file-on-a-leaf-symlink-to-a-deny-listed-path-reads-and-copies-the-file.md, docs/issues/2026-10-17-augmentation-sidecar-write-through-rewrites-any-yaml-a-document-names.md, docs/issues/2026-10-17-write-allowlist-canonicalizes-only-the-parent-so-a-leaf-symlink-or-a-missing-subdir-escapes.md, docs/issues/2026-10-17-atomic-write-follows-a-planted-staging-symlink-and-overwrites-a-file-outside-the-root.md]
tags:
- cluster/unclassified
kind: bug
unverified: 'Parent-directory forms reproduced and fixed through round 3; the write door judges against the caller managed roots. Same-class sites NOT fixed: src/librarian/catalog/audit/shard.rs OpenOptions append, and src/tools/onboarding.rs raw fs::write (project.toml, workspace.toml, and .codescout/tmp/*, which is inside the repo; reachability read from the code, not run). edit_code rename wiring is untested (no mock-LSP harness). Two round-3 mutants survive by redundancy (the LIKE escape behind the starts_with filter; the write-through re-arm behind the preflight arm in the same call). Checks and writes are separate syscalls, so a link planted between them still wins. P8, the librarian applying no READ deny-list, is its own issue.'
---

# BUG: `doc(update)` writes with `std::fs::write(row.abs_path)`, so a catalogued in-root symlink to a file outside the root has that file rewritten

## Summary

`src/librarian/tools/update.rs` wrote the new document with `std::fs::write(&full, &new_content)` (line 775 before
the fix), where `full = row.abs_path` (line 586). `fs::write` follows a symlink. The reindex deliberately catalogs an
in-root `.md` symlink that points OUTSIDE the root (`src/librarian/indexer.rs`, the `is_symlink` block), so
`doc(update)` on such a row rewrote the outside file. Measured: it did, and so did the other librarian write sites
(table under Fix).

## Class fit

No existing class names a write that follows a link in a path the catalog, not the caller, supplied. It is not
the write-allowlist class: `doc(update)` takes an artifact id, not a path, so no write classifier is consulted at
all. Filed unclassified; it is the librarian-side sibling of the `edit_file` leaf-link result in the allowlist issue.

## Symptom (Effect)

A file outside the project root is rewritten (status, title, tags or a whole new body, whatever the patch says) and
the call returns `updated: true`. The in-root link is left in place, so nothing in the project changes.

## Reproduction

Measured 2026-10-17 on `worktree-agent-a26646b8b970ff11a` with a throwaway test, under `TempDir`s: `outside/elsewhere.md`
holding a document with frontmatter `status: draft`; `root/outward.md` a symlink to it; `mk_ctx(root)`; the real
`index_repo_sync` with a `**/*.md` rule; then `update::call(ctx, {"id": artifact_id_from_abs(root/outward.md),
"patch": {"status": "fixed"}})`:

```
PROBE reindex added=1 row_exists=true
PROBE doc update: ok=true None
PROBE victim changed=true victim_now="---\nkind: spec\nstatus: fixed\ntitle: T\n---\n\nbody\n"; link still symlink=true
```

Re-measured 2026-10-17 on `worktree-agent-a66600865e063ffd2` (experiments `27efb11d`) by the permanent tests of the fix,
run against the UNFIXED code first (red). Each wrote through the link and reported success:

```
doc_update_refuses_to_write_through_...        update::call -> Ok {"id": "2584c616f38e5a32", "updated": true}
doc_update_refuses_a_stale_row_whose_link_...  update::call -> Ok {"id": "d14b53bffb7b625e", "updated": true}
a_frontmatter_field_write_refuses_...          write_field_to_frontmatter -> Ok(())
doc_move_of_a_catalogued_outward_link_...      victim `id: 0123456789abcdef` became `id: a0e4bbc5be8b02b8` (the move's new id)
doctor_repair_frontmatter_id_...               victim `id: 0123456789abcdef` became `id: be7b586c8ccd0ac9`
```

## Environment

Linux, `cargo test --lib`, `TMPDIR=/home/marius/.cache/cc-tmp`.

## Root cause

`update.rs:586` `let full = row.abs_path.clone();` and `update.rs:775` `std::fs::write(&full, &new_content)?;`, with no
`symlink_metadata` check between them. `indexer.rs` keeps a symlink whose target resolves outside the root (skipping
it would trade a silent duplicate for a silent omission), so the row exists. The same shape recurs at every writer
that takes its path from the catalog.

## Evidence

- Measured, above. The in-root-target form needs a stale row: the indexer skips a link that resolves inside the root
  (`a_symlink_resolving_inside_the_root_is_not_indexed_as_a_second_artifact`), so the test indexes a real file and
  then replaces it with a link, which is how a pre-skip catalog reaches that state.
- Doc-vs-code drift found on the way, and corrected TWICE. The comment above the `is_symlink` block said this repo's one
  kept outward link is `.claude/skills/claude-traces`. It is a DIRECTORY link, which the MAIN walk does not descend
  (`a_symlinked_directory_is_not_descended_into_by_the_walk`, `1f25f518`). Round 2 found that sentence true of the main walk
  ONLY: `force_include_candidates` walks `WalkBuilder::new(<anchor>)`, and a walker follows a symlinked ROOT, so a repository
  that ships `docs/trackers -> <outside>` plus `force_include = ["docs/trackers/**"]` DOES catalog outside files, and a
  directory swapped for a link after indexing keeps its rows (reclamation asks `Path::exists()`, which follows the link).
  Both are measured in `link_dir_tests.rs` and both are fixed; the comment now says what each walk does. No outward FILE link
  is catalogued in this repo; the exposure is a repository that ships one, or either directory form.

## Hypotheses tried

1. **Hypothesis:** reindex catalogs only files under the root. **Test:** the probe above. **Verdict:** rejected; it
   catalogs an outward FILE link on purpose.
2. **Hypothesis:** the repo's own `claude-traces` link makes this reachable today. **Test:**
   `a_symlinked_directory_is_not_descended_into_by_the_walk`. **Verdict:** rejected (see Evidence).

## Fix

**Decision, and why** (the issue asked for one of three; recorded in `util::refuse_write_through_symlink` too):
**refuse any write whose target path is itself a symlink** (dangling or not). Not "replace the link with a regular
file": the caller asked to update a document, the call would report success while the real document stayed unchanged
and the user's link silently became a stale copy (`util::fs::atomic_write` does this for `create_file`/`edit_*`, where it
is the right shape because the caller named the file to overwrite). Not "write through only when the target is inside
the root": the indexer never produces that row (a link resolving inside the root is skipped as a duplicate name), so
the branch would be unreachable by a fresh index, and a write through it would leave the TARGET's own catalog row behind
its file (`row_behind_file`). The target has its own id and that is the route. The message names the link and where it
points and says to update the target.

**Where:** `src/librarian/util.rs` `refuse_write_through_symlink` (predicate: `symlink_metadata` is a symlink) and
`write_catalogued_file` (the one door: predicate, then `fs::write`). A refused write returns a recoverable error,
before the catalog row moves.

Fix commits, branch `worktree-agent-a66600865e063ffd2` (NOT on `experiments`):

- `2cf7dd7b` patch-id `fd84fb63b6f18f248115a738bdbeb2e91c179b93` (this issue and the sweep, leaf rule)
- `1f25f518` patch-id `4cf84bfb0bccdb8034802fdc54a913cfbbca9cb3` (walk does not descend a directory link; comment corrected)
- the sidecar declared-path defect found by the sweep: `d703c12b` patch-id `f5e66cb1e077da8dd841797973a0519517ee586e`
  (own file: `2026-10-17-augmentation-sidecar-write-through-rewrites-any-yaml-a-document-names.md`)
- ROUND 2, parent-directory links and the rest of the review:
  - `033bf24b` patch-id `b5b500849f73cad3f85234d916eadb5b1f144839`: containment in the write door, `doc(move)`, `doc(delete)`,
    the indexer (`force_include`, reclamation), every declared-sidecar access, `doc(augment)` recoverable and before the
    catalog moves, `doc(create)` normalises before validating
  - `499eb260` patch-id `32ffa40dd08696bab003ee550b2c1971953cfe28`: tests for an in-repo sidecar link and a link to another repository
  - `7579a5d3` patch-id `aafcd3fee7a6d6b0a80ab4f7b0c1e9dc019e68d8` and `e3c43bce` patch-id `43d92250ecf1c076b35010ced7edcf63c9e51131`:
    tests that kill the surviving mutants (relative link in a subdirectory, `write_through`'s refusal type, the lexical message)
  - `6bfc6f95` patch-id `a95fa19fc6bd3d80ce459d3fc7a9a796ecf3b3c3`: deny-list tests via a fake HOME in both link spellings

The refusal HINT was wrong for the same reason the first table was: it said an outward-linked document "is catalogued
there when it lives in this repository", which never applies (the indexer catalogs only outward links). It now says a
document catalogued through a link is read-only through the librarian, and what to do for an in-repo and an outward target.

### Per-site decision table (full sweep of `fs::write` / `File::create` / `OpenOptions` / `fs::copy` / `atomic_write` in `src/librarian` and `src/tools`, production code only, as of `27efb11d`)

"Leaf link" = a catalogued (or committed) link at the written path itself. "Parent link" = a symlinked directory
above it. **Round 2** (the Opus review of `8ff717a8`) found the parent column was wrong ("n/a") for several rows; every
row below is re-derived and the changed ones are marked **R2**.

The rule now has two halves, both in `util::write_catalogued_file`: the LEAF must not be a link
(`refuse_write_through_symlink`), and the whole path must resolve inside the repository it spells
(`refuse_catalogued_path_outside_repo`, judged against the caller's MANAGED roots since round 3: see "Round 3"). A directory link whose target stays INSIDE the repo works: a
symlinked tracker directory is a legitimate layout (controls: update, move, augment, force_include, reclamation).
**What "unless the row is a catalogued outward FILE link" means given the leaf rule:** such a row stays catalogued and
readable and is refused by every writer; there is no exception. Limits (round 3): a caller that arms no roots (a test, a
direct catalog call) falls back to a root whose resolution contains the path, then to the repository the path spells
(`spelled_repo_root`, which skips ancestors that are themselves links); with none of those only the leaf rule applies.
The tools that reach a writer arm the managed roots, so a project without a `.git` IS confined.

| site | path comes from | leaf link | parent link | decision |
|---|---|---|---|---|
| `tools/update.rs` `call` (was :775) | `row.abs_path` | yes, measured | **R2: yes, measured** (`force_include` or a swapped directory mints such rows) | **fixed**: `write_catalogued_file` |
| `tools/update.rs` `write_field_to_frontmatter` (was :1000) | `row.abs_path` | yes: via `doc(event_create)` (event_create.rs:261, :271) and `doc(link)` `supersedes` (link.rs:62) | same | **fixed**: same door |
| `catalog/augmentation.rs` `append_entry` section, `restore_section_after_failed_commit`, `allocate_entry_id`, `resync_snapshot_row` | row's `abs_path` | yes | same | **fixed**: same door; transactions roll back (asserted) |
| `catalog/rekey.rs` `rekey_body` | row's `abs_path` | yes | same | **fixed**: same door |
| `tools/mv.rs` destination (`create_dir_all` + `rename`) | `root.join(new_rel_path)` | n/a (must not exist) | **R2: yes, measured**: `esc/sub/moved.md` built `sub` outside and moved the file there | **fixed**: `refuse_path_outside_root(new_full, root)` BEFORE `create_dir_all` |
| `tools/mv.rs` source (`rename`) | `row.abs_path` | a rename moves the link itself: fine | **R2: yes, measured**: a row below a link dir had the OUTSIDE file renamed into the repo | **fixed**: the source's parent is containment-checked |
| `tools/mv.rs` `repair_frontmatter_id` (was :638), `doctor` repair sweep | moved path / row | yes, measured | the destination check above stops a moved file ending up behind a link | **fixed**: skipped in `doc(move)`, reported under `failed` by the sweep |
| `tools/delete.rs` `remove_file` (:177) | `row.abs_path` | an unlink removes the link, never its target: supported | **R2: yes, measured**: deleted the OUTSIDE file; **missing from the first table** | **fixed**: the parent is containment-checked against the managed root (preview too) |
| `tools/doctor.rs` `export_augmentations` (sidecar CREATE, stamp) | scope root + flattened name; row's `abs_path` | yes | **R2: yes, measured**: created files through `docs/augmentations -> <outside>` | **fixed**: `confined_path` before writing, plus the door |
| `augmentation_sidecar.rs` `write`, `write_through` | git root + declared `rel` | yes: a committed link | **R2: yes, measured**: a clean-spelled `docs/augmentations/t.yaml` overwrote an outside YAML (the first sidecar fix was lexical only) | **fixed**: `confined_path` (lexical AND resolved) + the door |
| sidecar READ: `read_in` used by `scan_sidecar_shape_drift`, `restore_declared_augmentations` | declared `rel` | n/a | **R2: yes, measured**: opened `../x.yaml` / an absolute path and the parser error echoed a bare-scalar value (`invalid type: string "ghp_…"`) into `sidecar_unparseable` | **fixed**: refused WITHOUT opening; asserted that a fake token never appears |
| sidecar READ in `append_entry.rs` (:2296, :2987, :3590) | the repo's own TAXONOMY | n/a | n/a | **not changed, not reachable by an agent**: they sit in `#[cfg(test)] mod taxonomy_recipes`, a doc-to-code gate over this repository's own docs |
| `indexer.rs` `force_include_candidates` | `WalkBuilder::new(<anchor>)` follows a symlinked ROOT | n/a | **R2: yes, measured**: `docs/trackers -> <outside>` + `force_include` catalogued the outside file | **fixed**: a candidate whose resolved path leaves the root is skipped (and recorded, so an existing row is reclaimed) |
| `indexer.rs` reclamation (:673) | `Path::exists()` follows the link | n/a | **R2: yes, measured**: a directory swapped for a link after indexing kept its rows | **fixed**: an unseen row that resolves outside the root is reclaimed; control: a link that stays inside is not |
| `tools/create.rs` `call` | `base_dir.join(rel_path)` | dangling: yes | **yes, measured** | **fixed** in the `doc(create)` issue; **R2**: `rel_path` is normalised BEFORE it is validated |
| `catalog/audit/shard.rs` `OpenOptions::append` (:622) | repo root + fixed dir + `<host>-<month>.jsonl` | yes if a committed link carries the victim host's shard name | not examined | **NOT fixed.** Same class; `write_catalogued_file`-style guard applies; its fixture is heavy. Follow-up |
| `workspace.rs` `create_default` | `workspace.toml` | **no**: `create_new(true)` is `O_EXCL`, which refuses an existing symlink, dangling included | not examined | no change |
| `tools/edit_file/mod.rs` x3, `edit_code` symbol edits (`atomic_write`) | write classifier / `resolve_write_path_for` | no write escape (rename over the link); the READ was the defect | the classifier resolves the parent | **fixed** in the `edit_file` issue |
| `tools/symbol/edit_code.rs` rename plan (`fs::write`, :710/:713) | LSP-reported paths | yes (raw `fs::write` follows) | the classifier resolves the parent | **fixed** with the `Writable` policy; the call in `do_rename` is NOT integration-tested (no mock-LSP rename harness) and survives a mutation |
| `tools/onboarding.rs` (:113, :158, :1068) | `<root>/.codescout/project.toml` | yes: raw `fs::write`, a repository can ship that name as a link | not examined | **NOT fixed.** Follow-up |
| `tools/onboarding.rs` (:461-464, :525, :534) | `<root>/.codescout/tmp/onboarding-*.md` (**R2: this is INSIDE the repository, not "a temp dir"**) | yes by reading: a repository can commit `.codescout/tmp/onboarding-prompt.md -> ~/.bashrc` and `fs::write` follows it | yes by reading: `.codescout/tmp -> <outside dir>` makes `create_dir_all` succeed and the writes land outside | **NOT fixed; reachability by READING the code, NOT run.** Follow-up |
| `tools/onboarding.rs` (:861) | `workspace.toml` | yes | not examined | **NOT fixed** (same follow-up) |
| `tools/run_command/attribution.rs` (:265) | scratch | not examined | not examined | excluded by the brief |

Not in scope and not examined: `src/agent`, `src/lsp`, `src/memory`, `src/peer`, `src/retrieval`, `src/logging.rs`,
`src/heartbeat.rs`, `src/operator_rules`, `src/library` (internal state files; `memory`, `operator_rules` and `library`
already use `atomic_write`). The sweep covered `src/librarian` and `src/tools` as asked.

**Duplication, noted so the unification is a follow-up:** `util::resolve_through_existing_ancestor` is the same walk as
`canonicalize_through_existing_ancestor` in `src/util/path_security.rs`, which is private there and which this work was
forbidden to edit. A shared public form would let both call one implementation.

## Round 3 (Opus review of `caa7c46e`): one root, a sibling-safe reclaim, and a stated reason for every reclaim

Commits `c933d47a` (patch-id `27ea7e3364970050da6f3fcd4dd37ea1cd2f7a25`), tests `5df73f73`
(`57b8c29ee65d26a2a414543c0e3e9b220ac86a24`), `da0c4516` (`a9266d3e638d574f5c1d015c925e313a61a240c2`) and `14db742a`
(`0e7601f5bcad00c36b62b4d772d3be1ae1412a92`). Every item below was reproduced red first in `link_dir_tests.rs` (12 of the new tests
failed before the fix, e.g. `removed=…`, `updated: true` through a link, a refused write inside the managed project,
`escaping row … must survive`); the controls passed throughout.

- **H1, reclaim deleted sibling rows (measured):** the candidate query was `abs_path LIKE '<root>/%'` with no `ESCAPE`, and SQLite
  `LIKE` ignores ASCII case, so reindexing `b/my_app` selected the rows of `b/myXapp` and `b/MY_APP`, and round 2's escape test
  deleted them with their augmentation while the files sat on disk. Now `escape_like_pattern` + `ESCAPE '\'`, then a
  `Path::starts_with(abs_root)` filter (the only thing that handles case). The same pattern fed the vectorless count. The
  pre-existing apostrophe bug is fixed too: the prefix was SQL-quoted (`'` -> `''`) and then BOUND, so a root containing `'`
  matched nothing and its rows could never be reclaimed.
- **H2, three roots for "outside":** the walk and reclaim now judge against the GIT root of the walk root (resolved), as `doc(move)`
  and `doc(delete)` do via the managed roots, so a row legitimately moved to `pkg/docs/x.md` (a link to `../shared/docs`, inside the
  repo) survives a reindex of `pkg`, and `force_include` through such a link is catalogued instead of skipped-and-reclaimed. The
  monorepo probe is `a_row_moved_below_an_in_repo_link_survives_a_reindex_of_the_project`.
- **H3, the write door's root:** it used the nearest `.git` ancestor of the SPELLED path, which cannot tell "a link to another
  repository" (outside; the round-2 test) from "my own project reached through a link" (inside): the same input. It now takes the
  caller's MANAGED roots: `update` and `write_field_to_frontmatter` pass `managed_roots(ctx)`; the catalog-level writers
  (`allocate_entry_id`, `append_entry`, `update_entry`, `rekey_prefix_rows`, the sidecar write-through) read
  `Catalog::write_roots`, which the tool arms under the catalog lock (`arm_write_roots`). Order: the first managed root that
  contains the spelling (strict check), else a managed root whose RESOLUTION contains the path (a project registered under its
  canonical path while rows keep the spelled one), else the spelled repository. A project without a `.git` is therefore confined.
  `doc(move)`'s id repair takes the same roots, so it no longer fails after its rename in such a project.
- **M1, a refusal after the upsert:** preflight and door now share the root, so the refusal comes before the catalog moves; if one
  still follows the upsert (a link planted in between) its text says "the augmentation WAS updated in the catalog, but its committed
  sidecar was NOT written" instead of the "Nothing was changed" the first test locked in.
- **M2, a reason for every reclaim:** `reclaimed_escaping` (count) and `reclaimed_escaping_ids` in the reindex response, an
  `escaping_note`, and rows that carry an augmentation or events are KEPT and listed in `escaping_kept_ids` on every run until the
  link is removed or re-pointed (the file is then gone and the row goes as an ordinary vanished one). No cap on the id lists, on
  purpose: they name exactly what was deleted (a subset of `removed_ids`, already unbounded), a silent cap would hide ids from the
  one report that explains the deletion, and a large response is buffered by the progressive-disclosure layer. (A first version capped
  them at 50; the `every_cap_constant_is_classified` gate then asked for a probe row for a cap that did not earn one.)
- **LOW 1:** the walk's escape test is on the PARENT chain for every kind of leaf (a leaf link inside an outward `force_include`
  anchor is no longer catalogued, so no row is stuck behind a refused delete). The old exemption comment was false for that shape.
- **LOW 2:** `doc(move)` refuses, up front, a relative leaf link moved to another directory (a rename does not retarget it; the id
  repair then read a dangling link after the rename). The round-2 table's "fine" was wrong for that case; a same-directory rename
  is the control.
- **LOW 3:** a refused declared sidecar is reported as a refusal ("not opened … do not edit or delete the file behind the link"),
  not as `sidecar_unparseable` with "Repair the YAML … or delete it"; `augmentation_declared_but_absent` confines its check and no
  longer advises `reindex` for a path `reindex` refuses.
- **LOW 4:** the heading-addressed markdown grammar is tested against a deny-listed leaf link again (in the fake-HOME child).
- **LOW 5:** the export's own containment call is now load-bearing in a test (no `.git`, no managed root at the door), and the
  export refuses an artifact below a link directory BEFORE writing its sidecar (no orphan sidecar, no stamp outside the repo).
- **LOW 6:** the refusal hint no longer says "READ-ONLY"; it says the librarian refuses to write this path and that move and delete
  still act on the link itself.
- **LOW 7:** both fake-HOME children refuse to run unless `HOME` is the throwaway directory the parent test created.

**Decisions of round 2 that changed:** (1) the write door's root is the managed root, not the spelled `.git` ancestor, so the
round-2 limit "a repository root reached through a link is not covered" is gone, and projects without `.git` are covered;
(2) reclamation judged against the walk root and exempted leaf links: now git root and parent chain; (3) reclamation deleted every
escaping row: it now keeps and reports the ones with history; (4) "a relative outward leaf link moves fine": it does not, now
refused. Unchanged: refuse any leaf link; no exception for an outward FILE link.

**Mutation probe, round 3** (189-194 tests per run): killed — the door ignoring managed roots; the resolution shortcut; the
confinement root; the arm doing nothing; `update` and the field writer's roots; arming in `append_entry` (prose and params paths),
`update_entry`, `rekey_prefix`, `doc(augment)`'s preflight; `doc(augment)` wrapping the refusal; `declared_target`'s and the
write-through door's roots; the sidecar containment; the relative-link refusal and its same-directory control; the id repair's
roots; the `starts_with` filter; the apostrophe doubling; the walk using the walk root instead of the git root; keeping rows with
history; the walk keeping an escaping row; judging the leaf instead of the parent; the export's artifact precheck and sidecar
confinement; the refusal worded as a parse failure. **Survived, by redundancy:** the LIKE escape (the `starts_with` filter
protects the same rows; the escape is a cost and defence-in-depth measure) and `doc(augment)`'s second arm inside
`sidecar_write_through` (the roots persist on the catalog from the preflight arm of the same call).

## Tests added

All red against the unfixed code first (results under Reproduction), green after. `src/librarian/tools/link_write_tests.rs`:
`doc_update_refuses_to_write_through_a_catalogued_link_to_a_file_outside_the_root`,
`doc_update_refuses_a_stale_row_whose_link_now_points_inside_the_root_but_updates_the_target` (also updates the target
directly), `a_frontmatter_field_write_refuses_...`, `a_field_patch_event_on_a_catalogued_outward_link_fails_and_leaves_the_victim`
(asserts no event row), `doc_link_supersedes_does_not_flip_the_status_of_a_file_behind_a_catalogued_link` (asserts no link
row and no status change), `doc_move_of_a_catalogued_outward_link_does_not_repair_the_id_through_the_link` (asserts the move
still succeeds and the link was renamed, not replaced), `doctor_repair_frontmatter_id_...` (asserts `failed`, not `repaired`),
`doctor_export_augmentations_...` (asserts no sidecar directory). Control: `doc_update_still_writes_a_regular_file_in_the_root`.
`catalog/augmentation.rs`: `allocate_entry_id_refuses_...`, `append_entry_refuses_...`, `update_entry_refuses_...`,
`restoring_a_section_never_writes_through_a_link` (control `restoring_a_section_still_restores_a_regular_file`).
`catalog/rekey.rs`: `a_body_rewrite_is_refused_when_the_ledger_is_a_link_to_a_file_outside_the_repo`.
`augmentation_sidecar.rs`: `write_refuses_a_link_to_an_existing_file_and_a_dangling_one`,
`write_through_refuses_a_committed_sidecar_that_is_a_link_outside_the_repo`.

Mutation probe (`scripts/mutation-probe.sh`, one run per guarded site, 72 tests each): **killed** — predicate never fires,
writer drops its guard, update `call` plain write, update field plain write, move skip removed, export precheck removed,
repair precheck removed, append_entry plain write, restore plain write, allocate plain write, resync plain write, rekey plain
write, sidecar `write` plain, sidecar `write_through` plain. **Survived, by construction**: the move's writer guard
(`M2`) and the stamp's writer guard (`D3`) — each sits behind an earlier check that refuses first (the move's skip, the export
loop's precheck), so neither is independently reachable; they are defence in depth, kept so a future caller of those two
functions takes the door.

### Round 2 tests (red first, against the code of `8ff717a8`; results recorded)

`src/librarian/tools/link_dir_tests.rs` (17 tests, 9 of which failed first) plus the `doctor`, `reindex` and `create` tests below
(4 more): 13 tests failed before the round-2 fix, each with the damage visible, and the
controls (in-root directory link for update, move, augment, force_include, reclamation; leaf-link delete) passed throughout.
Measured reds: `doc(update)` below a link directory returned `updated: true`; `doc(delete, force)` returned `deleted: true`
for the OUTSIDE file; `doc(move)` returned `moved` for both the destination (`esc/sub/moved.md`) and the source forms;
`doc(augment)` through a clean-spelled `docs/augmentations -> <outside>` returned `"ok"` and rewrote the outside YAML, and for
`../outside.yaml` surfaced as the FATAL `"the augmentation WAS updated, but its committed sidecar could not be written: …
re-run once the path is writable"` (the catalog had moved first); the export reported `exported: 1` having created a file in
the outside directory; `force_include` catalogued the outside file; a swapped directory kept its row. Also
`doc_update_refuses_a_row_below_a_link_to_another_repository` (the link's target is itself a repository),
`doc_augment_refuses_a_sidecar_that_is_an_in_repo_link_before_the_catalog_moves`.
`doctor.rs`: `a_sidecar_declared_outside_the_repo_is_refused_and_its_content_never_echoed` (control: the parser DOES echo the
fake token when asked directly) and `a_sidecar_behind_a_directory_link_out_of_the_repo_is_refused_and_never_echoed`.
`reindex.rs`: `a_declared_sidecar_outside_the_repo_is_neither_attached_nor_echoed`. `augment.rs`:
`a_refusal_from_write_through_stays_recoverable_and_keeps_its_own_remedy`. `create.rs`:
`create_validates_the_rel_path_it_will_actually_use`.

Mutation probe, one run per new guard (171-173 tests each): **killed** — write door drops containment; root accepted through a
link; containment never fires; root left unresolved; `..` term dropped; no-repo arm; move destination unchecked; move source
unchecked; delete unchecked; walk keeps outside rows; reclamation ignores an escape; `doc(augment)` without the preflight;
`doc(augment)` wrapping the refusal (after `a_refusal_from_write_through…` was added); lexical sidecar check dropped (after the
assertion was made specific); resolved sidecar check dropped; `read_in` unconfined; `declared_target` unconfined; the preflight's
leaf check; doctor read unconfined; reindex read unconfined; create validates the raw string; create containment dropped;
a `read_link`-based leaf resolver (after the relative-link fixture was fixed). **Survived**: the export's own `confined_path`
call (the write door's containment refuses the same path first: defence in depth, not independently reachable) and the
`escapes_root` leaf-link exemption in reclamation (an outward FILE link is always SEEN by the walk, so it is never an unseen
candidate; unreachable by the walk's own contract).

## Workarounds

Do not keep `.md` symlinks that point outside the project root in a repository that uses the librarian. With the fix such
a row is read-only through the librarian: edit the file the link points at under its own path.

## Resume

N/A. Follow-ups for the reviewer: the NOT-fixed rows (`shard.rs`, `onboarding.rs` including `.codescout/tmp`); the duplicated
`canonicalize_through_existing_ancestor`; and `2026-10-17-the-librarian-applies-no-read-deny-list-to-a-catalogued-outward-link.md`
(the READ twin, high, open).

## References

- `docs/issues/2026-10-17-write-allowlist-canonicalizes-only-the-parent-so-a-leaf-symlink-or-a-missing-subdir-escapes.md`
- `docs/issues/2026-10-17-atomic-write-follows-a-planted-staging-symlink-and-overwrites-a-file-outside-the-root.md`
- `docs/issues/2026-10-17-doc-create-on-a-dangling-symlink-creates-a-file-outside-the-root.md`
- `docs/issues/2026-10-17-edit-file-on-a-leaf-symlink-to-a-deny-listed-path-reads-and-copies-the-file.md`
- `docs/issues/2026-10-17-augmentation-sidecar-write-through-rewrites-any-yaml-a-document-names.md`
