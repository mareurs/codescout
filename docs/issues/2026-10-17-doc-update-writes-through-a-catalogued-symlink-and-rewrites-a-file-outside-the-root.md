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
unverified: 'Parent-directory forms reproduced and fixed through round 6; the write door judges by one stated invariant (docs: Round 5, corrected in Round 6) and every layout found so far is a row of door_layout_table or door_layout_table_round6. Same-class sites NOT fixed: src/librarian/catalog/audit/shard.rs OpenOptions append, and src/tools/onboarding.rs raw fs::write (project.toml, workspace.toml, and .codescout/tmp/*, which is inside the repo; reachability read from the code, not run). edit_code rename wiring is untested (no mock-LSP harness). A path with no managed root, no .git above it and no ancestor resolving to a managed root gets only the leaf rule. A foreign link to the managed root itself is admitted (row H2, by decision), and a foreign non-repo directory with a link to a repository is judged inside that repository (Y1a). Verdicts depend on the order of the managed roots (C5/C5c, X6), which tools::managed_roots fixes. A path spelled with .. is refused, not normalised, so a plain-looking Q/../P/x.md is refused when nothing manages it. doc(move) refuses a file it cannot read as UTF-8 text; the mode-000 and read-only tests skip themselves as root. Checks and writes are separate syscalls, so a link planted between them still wins. P8, the librarian applying no READ deny-list, is its own issue; the remaining LIKE call sites are listed in docs/issues/2026-10-18-the-remaining-like-prefix-sites-are-case-folded-or-unbounded-and-none-is-destructive-today.md.'
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
  **Superseded in round 4, see below: this order was wrong in three ways (an umbrella root hid a link between its repositories, the
  resolution shortcut could only admit, and a canonically registered project reached through a link was not checked at all).**
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

**Decisions of round 2 that changed:** (1) the write door's root is the managed root, not the spelled `.git` ancestor. **(Round 3 claimed here
that the round-2 limit "a repository root reached through a link is not covered" was gone. That was false for a project
registered under its canonical path and reached through a link, which the door did not check at all; round 4 H-1 fixes it, and
the statement is withdrawn.)** Projects without `.git` are covered;
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
**Round 4 correction of that last line:** the second arm was not "redundant", it was MASKED by the defect that the roots
persisted past the call that armed them (L-1 below). With arming scoped to the call by a guard, the second arm is load-bearing
and its removal is killed by a test.

## Round 4 (Opus review of `7a7bfc9b`): the door's two wrong answers, an exact path compare, and arming scoped to the call

Commits `a6a293c0` (patch-id `934f4d7b7dd4989f5229131d7077fae67fa1da36`) and `c8108ca0` (`208f7cbf040b67669f0b20af71222e162f327595`, a test fix
found by the mutation probe). Every behavioural item was reproduced red first (`link_dir_tests.rs`, `doctor.rs`; 11 tests red before the
fix). The others are pins that were green from the start, said plainly: the P1 control, P4 (round 3 already fixed it), the absolute-link move
(P7), the events-only-history keep (M-A), the vanished-row-below-an-escaping-parent case, and the tool-level reindex response test (M-B,
the keys existed; nothing pinned them).

- **H-1 and M-1, the write door (measured by the reviewer's probes P1-P3, reproduced here):** two inputs were answered wrongly.
  *P3:* a project registered under its canonical path (`data/proj`), rows spelled through a link (`home/proj/...`), no `.git` above the
  link, and a link INSIDE the project to an outside directory: no managed root contains the spelling, the resolution shortcut could
  only admit, and the spelled walk skips both links and finds no `.git`, so nothing was checked and the write landed outside every
  managed root. *P1/P2:* with a legacy umbrella root (or an unrelated active project listed first) the first containing managed root
  was the umbrella, and a link from repository A into repository B stayed "inside" it; the shortcut also admitted a FOREIGN
  repository's row that resolves into the managed project. (**Superseded by round 5**: the "deeper of two" rule below reopened a round-3
  refusal, and its refinement is now accepted only if reached from the confinement root without crossing a link.)
  `refuse_catalogued_path_outside_repo` now judges against the DEEPER of (a) the
  *base* (the first managed root that lexically contains the spelling, else the deepest ancestor of the spelling that resolves to a
  managed root exactly) and (b) the *spelled repository* (nearest non-link ancestor with a `.git`), and the "resolution lies under some
  managed root, so accept" shortcut is gone. Controls: a link that stays inside its own repository, and the owning project active, still
  work. Not covered: a path with neither candidate (no managed root contains it, no `.git` above it, no ancestor resolves to a managed
  root); only the leaf rule applies there, as before.
- **H-2, a second LIKE site (own issue: `docs/issues/2026-10-18-doctor-prune-missing-matches-rows-with-like-and-deletes-the-rows-of-live-sibling-repositories.md`):**
  `prune_dead_root` and `count_dead_root` deleted and counted with unescaped, case-insensitive `LIKE`. Now exact. The same class
  remains at call sites this round did not touch (listed in that issue's caveat).
- **M-2 and L-5, one exact predicate for every prefix match here:** `util::path_at_or_under_sql` is `(col = ?root OR substr(col, 1, length(?prefix)) = ?prefix)`,
  byte-exact. The indexer's reclaim candidates and its vectorless count use it, and the Rust-side `starts_with` re-filter round 3 added
  is gone (it handled case only for the reclaim, was wrong on Windows spellings, and masked the SQL being wrong). The vectorless count
  matched a case-only sibling (red: `Some(1)` against `Some(0)`). A Linux unit test runs the helper on Windows-shaped strings.
- **L-1, arming outlived the call:** `Catalog::write_roots` was set by a tool and never cleared, so a catalog-level writer reached without
  arming inherited the previous call's roots, against the field's own doc. `arm_write_roots` now returns a guard (`#[must_use]`) that
  clears on drop; each tool binds it for the call. **This corrects round 3's survivor note:** the second arm in `sidecar_write_through`
  was not redundant, it was masked by this defect; removing it is now killed. (The first test of this item also went vacuous once H-1
  was fixed, and a mutant that never cleared survived; its fixture was rebuilt on an input whose verdict differs between armed and unarmed.)
- **L-3:** the reindex `escaping_note` names `doc(action="graft")` and says that removing the link deletes a kept row with its history
  unless that history is first folded into the row of the file's real location. A vanished row below an escaping parent is pinned as an ordinary
  vanished row (`removed`, not `reclaimed_escaping`).
- **L-4, a move that did not need refusing:** round 3's up-front refusal of any relative leaf link moved to another directory also
  refused `a/x.md -> ../real/x.md` moved to `b/x.md`, whose target still lands on the same file. `doc(move)` now compares where the
  target lands from each directory (`util::link_target_landing`) and refuses only a link that would land elsewhere (the
  sibling-name target `real.md` is the control, and the old tests used prefix `..` which a non-`..` target would not have caught).
  An absolute-target link moves freely.
- **L-6:** `rekey_prefix` runs the sidecar preflight before its transaction, in preview too, so a refused declared sidecar no longer
  leaves a committed rekey behind; its hint no longer advises deleting "the stale file", which for a refused sidecar is the file behind a link.
- **M-A, M-B:** an escaping row whose only history is an EVENT is kept (test added); the reindex TOOL response carries
  `reclaimed_escaping`, `reclaimed_escaping_ids`, `escaping_kept_ids` and the note (tool-level test, plus a null-note control).

**Decisions of rounds 2-3 that changed:** the door's root order (round 3 "Order: ...", superseded above); round 3's claim that the
canonical-path limit "is gone" (withdrawn above, fixed now); the round 3 survivor note on the second arm; round 3 LOW 2's blanket relative-link
refusal (now precise). Unchanged: refuse any leaf link; reclaim keeps rows with history; one root for the walk (git root).

**Mutation probe, round 4** (52-262 tests per run): killed — the deeper-root rule; the ancestor-resolution base; the spelled-repository
fallback; the exact predicate's root equality and its prefix clause; the indexer's candidate predicate swapped for case-insensitive `LIKE`;
the indexer's placeholder offset; the guard's clear; `sidecar_write_through`'s own arm; the relative-link landing comparison and its `..`
handling; `rekey_prefix`'s preflight; the graft naming in the note. **Survived:** none left; one (the guard's clear) survived on the first
fixture and was killed after it was rebuilt. **Not probed, equivalent by construction:** `ancestors().skip(1)` in
`ancestor_resolving_to_managed_root` (a file path cannot canonicalize to a directory root).

## Round 5 (Opus review of `e51d2d99`): the door fixed to a stated invariant, and a permanent layout table

Commit `0b187952` (patch-id `5e48b25e9e01f0ccb1e18c0a878f8d4b880333eb`), then `0dace885` (`7c44cb9e9437fd0ccea56b0519a76b0c5df2fdaa`:
table rows K1/K2 and the non-dry merge pin) and `af25d0fb` (`7b84e5064bd4b137db331eec9dd7387e4b0fe381`: these docs). Round 6
corrected several statements below that the next review found false; each is marked. This round was meant to be the last for the door: it is now one function with
one stated invariant, and every layout any round has found is a row of a table test (`door_table::door_layout_table` in
`link_dir_tests.rs`), so a fifth change that breaks an answer is red there.

**The invariant (`util::door_root`).** *The judging root is reachable from the confinement root without crossing a symbolic link;
when no such inner root exists the confinement root itself is used.*

- The **confinement root** is the FIRST managed root, in the caller's order, that lexically contains the spelling or has an ancestor of
  the spelling that resolves to it exactly (then the SHALLOWEST such ancestor). One loop for both clauses, so two spellings of a file give
  one verdict **for a fixed root order** (round 6 correction: the order is part of the input, rows X6/X6b/C5c show it; round 4 took any managed root for the resolving clause and the first lexical match for the other, so `home/proj/tom2` and
  `data/proj/tom2` disagreed: C5/C5b/C5c/C5d).
- The **refinement** is the nearest ancestor of the spelling that holds a `.git` (directory or file) and is reached from the confinement root
  by plain directories (**round 6: and plain NAMES; a `..` is not a symlink, so `P/../Q` was wrongly counted as reached from `P`**). Round 4's rule picked the nearest `.git` ancestor that was not itself a link, and a repository found BELOW a link
  (`P/up -> ..`, row `P/up/S/docs/x.md`) is whatever the link points at: the write was judged against `S`, looked inside, and landed in
  `S`. The round-3 behaviour (refuse) was lost to the "deeper of two roots" rule; this restores it without losing P3, P1 or P2.
- With no confinement root the outermost repository on the spelling is the anchor and the same clause applies; with no repository either,
  only the leaf rule applies.
- **One function for every verb.** (**Round 6 correction: this was FALSE when written.** `doc(move)` and `doc(delete)` still gated on the
  lexical `containing_root` before the door, so a row the door admitted was refused by them (layout C1), and `doc(create)` judged against the
  project root alone, a fourth rule (layout F2). Round 6 fixes both; see its section.) `doc(update)` and the other writers reach `door_root` through `write_catalogued_file`; `doc(move)`
  judges its destination before the rename (so the id repair that follows cannot be refused after the file has moved: the half-done move
  of probe MV7) and its source parent, and `doc(delete)` its parent, through `refuse_catalogued_dir_outside_repo`. The table runs every
  row through both the file door and the directory door and requires them to agree.

**What each finding became.**

- **H-1:** rows A2, A3, A4, B1, B2, G3 and C4 are REFUSE again (the reviewer measured them ADMIT at `e51d2d99`); `B2` (`P/rootlink -> /`)
  no longer admits a write into any checkout on the machine. Test `a_link_to_the_parent_cannot_reach_a_sibling_repository_through_any_verb`
  runs B1 through update, delete and move.
- **M-1:** `doc(move)` into a nested repository's link back to the superproject now refuses up front, with nothing moved; the message after
  the rename no longer says "Nothing was changed" and says the file WAS renamed. (**Round 6 correction:** I wrote that this message was reachable only if the
  filesystem changed after the check, and later that it was unreachable. Both were false: the repair's read ran after the rename and failed on a
  mode-000 file, invalid UTF-8, a link to a directory and a row that is a directory, each leaving the file moved and the row not. Round 6 reads before the rename and
  makes the repair's write best-effort.)
- **M-2:** a dangling link (relative or absolute target) is refused up front. Round 4's loosened landing check had let a dangling
  relative link move, and the id repair then failed with ENOENT after the rename.
- **M-3:** see the confinement root above; C5, C5b, C5c, C5d, and C8 (two resolving ancestors) pin it.
- **M-4:** `merge_worktree` selection, abandon `DELETE` and dry run are exact (own issue, below). The non-abandon path was probed: the
  selection included the sibling, but `reseat_one` skips it, so it is not harmful (a pin, not a defect).
- **L-1:** patch-ids corrected in both round-4 issue files (round 4 had cited the commit hash as the patch-id).
- **L-2:** the write-roots guard is a stack of arms; a dropped guard removes its own entry only, the newest live arm's roots are in force,
  and an overlapping or out-of-order drop neither clears nor resurrects another arm. `arm_write_roots`' doc no longer claims the lock is
  held for the whole call (`doc(augment)` releases and re-takes it; each arm is its own guard).
- **L-3:** `rekey_prefix` preview now agrees with apply on a declared sidecar that is a dangling in-repo link
  (`preflight_write_through` tests `is_file()`, as `write_through` does). The doc at the preview/apply split says an Ok preview does NOT
  promise the apply clears a refusal of the ledger BODY (the body door runs only on apply).
- **L-4:** decided and recorded, not changed: a FOREIGN link to the managed root itself (`Q/linkP -> P`, row H2) is ADMITTED, because it
  is the same input as a project registered canonically and reached through a link (`home/proj -> data/proj`, row C1); one cannot be
  refused without the other. A foreign link to a SUBDIRECTORY of the root (H1) is refused. The door's doc comment now says so (round 4's
  said a foreign link "names no root", which holds only for subdirectories).
- **L-5:** `doctor(fix=prune_missing, root="/gone/x/")` no longer prunes zero rows silently: `util::root_and_prefix` strips the
  trailing separator for the prune, its dry run, the indexer and `merge_worktree`.
- **L-6:** F2 and F4 (a nested repository or submodule linking into its superproject inside the managed root) stay REFUSED by the
  door, by the author's stated choice; with the shared door, `doc(move)` and `doc(delete)` now refuse them too, so the three verbs agree.
- Mutation gaps closed: the `length(CAST(.. AS BLOB))` mutant (a non-ASCII root test) and the `CurDir => pop` mutant (a `./real.md` target).
  The `skip(1)` to `skip(0)` mutant stays alive and is **equivalent**: the path itself differs from its ancestors only when it is a
  link resolving exactly to a managed root, which the write door's leaf rule refuses first and which no other verb passes (move and delete
  probe a nonexistent child). Said in the code.

**The table (every row, with its committed verdict).** ADMIT = the door accepts. The file door and the directory door must agree on every row.

| row | layout | verdict |
|---|---|---|
| A1 | umbrella U{A,B}, unrelated C active, `A/linkdir -> B/docs` | REFUSE |
| A2 | A active, `A/up -> U` (absolute), row `A/up/B/docs/x.md` | REFUSE |
| A3 | C active, same row as A2 | REFUSE |
| A4 | A active, `A/up2 -> ..` (relative), row `A/up2/B/docs/x.md` | REFUSE |
| A5 | control, `A/inlink -> A/real` | ADMIT |
| A6 | control, plain row, C active | ADMIT |
| B1 | P active, `P/up -> W`, row `P/up/S/docs/x.md` (S a sibling repo) | REFUSE |
| B2 | P active, `P/rootlink -> /`, row reaching sibling repo S | REFUSE |
| C1 | canonical P, plain row via `home/proj` | ADMIT |
| C2 | canonical P, outward linkdir via `home/proj` | REFUSE |
| C3 | canonical P, in-repo link via `home/proj` | ADMIT |
| C4 | canonical P, `P/up -> data`, row via `home/proj/up/other` (sibling repo) | REFUSE |
| C5 | canonical P then m2, `P/tom2 -> m2`, spelled `home/proj/tom2` | REFUSE |
| C5b | same, spelled `data/proj/tom2` | REFUSE |
| C5c | m2 listed FIRST, spelled `home/proj/tom2` (the root is m2) | ADMIT |
| C5d | m2 listed FIRST, spelled `data/proj/tom2` (same verdict as C5c) | ADMIT |
| C8 | two resolving ancestors (`home/proj`, `home/proj/self`); nested repo behind the extra link | ADMIT |
| C9 | same nested repo reached without the extra link: its link back into P | REFUSE |
| C9b | same, canonical spelling | REFUSE |
| C6 | `home/.git` above the link, plain row via `home/proj` | ADMIT |
| C7 | `home/.git` above the link, outward linkdir | REFUSE |
| D1 | `home/.git`, project p without `.git`, plain | ADMIT |
| D2 | home is the first root, `p/tohome -> home/other` (inside home) | ADMIT |
| D2b | only p managed, same link (outside p) | REFUSE |
| D3 | `home/.git`, `p/linkdir -> outside` | REFUSE |
| E1 | managed root is itself a link, row spelled through it | ADMIT |
| E2 | same, row canonical | ADMIT |
| E3 | same, outward link, spelled through it | REFUSE |
| E4 | same, outward link, canonical | REFUSE |
| F1 | nested repo, plain | ADMIT |
| F2 | nested repo linking into its superproject (inside the managed root): refused by choice | REFUSE |
| F3 | submodule (`.git` file), plain | ADMIT |
| F4 | submodule linking into its superproject: refused by choice | REFUSE |
| F5 | superproject link into the nested repo's docs | ADMIT |
| F6 | linked-worktree root (`.git` file), in-repo link | ADMIT |
| F7 | linked-worktree root, outward link | REFUSE |
| G1 | project without `.git`, plain | ADMIT |
| G2 | project without `.git`, outward link | REFUSE |
| G3 | project without `.git`, `P/up -> parent`, row `P/up/S2/x.md` (S2 a repo) | REFUSE |
| H1 | foreign Q, `Q/linkdocs -> P/docs` (a subdirectory of the managed root) | REFUSE |
| H2 | foreign Q, `Q/linkP -> P` (the managed root itself): decided, ambiguous with C1 | ADMIT |
| J1 | foreign Q, `Q/up -> ..`, row `Q/up/S/docs/x.md` (S a sibling repo) | REFUSE |
| J2 | foreign Q, plain row | ADMIT |
| K1 | managed root directory gone, row below it | ADMIT |
| K2 | managed root directory gone, an unresolvable `..` below it | REFUSE |

(`I1`, a leaf link inside the repo, is the leaf rule and is covered by `link_write_tests.rs`, not by the door.)

**Decisions of rounds 2-4 that changed:** round 4's "deeper of two roots" selection (the nearest `.git` ancestor not itself a link) is replaced
by the invariant above; round 4's "deepest ancestor resolving to ANY managed root" is replaced by first-root-in-order, shallowest ancestor;
`doc(move)` and `doc(delete)` no longer judge against the first containing managed root alone (they use the door), so F2/F4-shaped rows now
refuse there too; round 4's allowance of a dangling relative link in `doc(move)` is reversed (refused up front, as in round 3). Unchanged:
refuse any leaf link, reclaim keeps rows with history, one root for the walk, F2/F4 refused by the author's choice, H2 admitted.

**Mutation probe, round 5** (61-567 tests per run): killed: the missing descent check (so the table, B1 end to end and four older tests); the shallowest
ancestor replaced by the deepest; the no-root anchor replaced by the nearest repository; the lexical clause removed (K2); the resolving
clause removed; the `strip_prefix` guard flipped; `./` popping; the byte-length `length()`; the trailing-separator trim; the dangling-link
refusal; `doc(move)`'s destination door replaced by the old managed-root check; `doc(move)`'s source check; `doc(delete)`'s check; the
guard clearing everything instead of its own arm; the guard taking the oldest arm; the preview's sidecar test widened back to `exists()`;
`merge_worktree`'s exact predicate replaced by `LIKE`. **Survived:** `skip(1)` to `skip(0)` (equivalent, above); the first run of the lexical
clause (a layout with an existing root cannot tell it from the resolving clause), killed after row K2 was added. **Not covered by a test:** the
text of the error after a half-done rename (**round 6: that error no longer exists; see the round 6 section**).

## Round 6 (Opus review of `af25d0fb`): `..`, four verbs that really agree, and a move that reads before it renames

Commit `afc73ce2f` (patch-id `24b85c3f207e3138088d644c584e9cadd00056bf`). The review found no HIGH, and the live catalog (6170 rows) is refused
by the door in 0 cases under all three root derivations. Every behavioural item below was written as a test first and seen RED (9 tests red
at `af25d0fb`: six table rows, the four-verb tests, the `..` end-to-end test, and the move tests), then fixed.

- **MED-1, a `..` spelling defeated the refinement.** `strip_prefix(P)` of `P/../Q` succeeds and `..` is never a symlink, so `P/../Q` counted as
  "reached from P by plain directories" and the write was judged against `Q`: with `P` active, `doc(update)` wrote `Q`'s file, `doc(move)` moved it into `P`
  and `doc(delete)` deleted it (reachability was limited: no live row has `..`). `descends_without_link` now accepts plain NAMES only. `.` and `//`
  need no code: `Path::components` drops them, so `P/./docs/y.md` and `P//docs/y.md` stay the plain paths they spell (rows X1f, X1g ADMIT). A `..`
  spelling is NOT normalised: `P/../Q/docs/x.md` claims `P`, so it is refused (X1, X1b, X1i), and with no roots it is judged against the outermost
  repository on the spelling and refused (X1d). The price, stated: a plain-looking `Q/../P/docs/x.md` is also refused when nothing manages it.
- **MED-2, "the three verbs cannot disagree" was false.** `doc(move)` and `doc(delete)` gated on the lexical `containing_root` before the door, so layout C1
  (root `data/proj`, row `home/proj/docs/x.md`) was written by update and refused by move ("no managed root contains") and delete ("outside every managed
  root"); E2 and H2 had the same shape. `doc(create)` called `refuse_path_outside_root(&full, &base_dir)`, a fourth rule: a create at `sub/linkdir/new.md`
  in layout F2 succeeded and landed in the superproject, after which update, move and delete refused the new row. Now `doc(move)` and `doc(delete)` name
  their root with `util::managed_base` (the function the door uses) and `doc(create)` goes through the door. Tests
  `every_verb_admits_a_row_spelled_through_a_link_to_the_managed_project` (C1: update, move, delete and create all ADMIT) and
  `every_verb_refuses_a_path_below_a_nested_repositorys_link_into_its_superproject` (F2: all four REFUSE, nothing created or moved) state the verdict
  of each verb. Nothing rounds 1-2 admitted is refused now: the full librarian suite (in-repo link directories, worktree shadow rows, umbrella roots,
  monorepo subprojects) is green.
- **MED-3, a half-done move was still reachable.** The id repair read the file AFTER the rename and failed on four causes, each renaming the file while the row
  kept the old path: a mode-000 file, invalid UTF-8, a leaf link to a directory, a row that is a directory. `doc(move)` now reads the content BEFORE
  anything moves and refuses a file it cannot read as text ("cannot read it as UTF-8 text", nothing moved); that content feeds the repair and the row's
  digest. What can still fail after the rename is the repair's WRITE (a read-only file): that is now best-effort as the function's doc said, so the move completes,
  the row is re-keyed with the file's real content, and the response carries `id_repair_error` (`doctor` reports the stale `id:`). One test per cause (the
  mode-000 and read-only tests skip themselves when run as root, and say so) and `doc_move_completes_and_reports_when_only_the_id_repair_write_fails`.
  False statements corrected: the comment at the old `mv.rs:353-356`, round 5's "unreachable once the door runs first", and the `repair_frontmatter_id` doc
  (only parse errors are logged; a read or write error is returned).
- **LOW-1, a foreign anchor reached through a link** (non-repo `X`, `X/link -> repo R`): ADMIT with unrelated or no roots (Y1a), REFUSE when `X` is managed (Y1b).
  Documented, not changed: with no managed root the anchor is the repository a foreign spelling CLAIMS (J2's reasoning), and here that is the one behind the link,
  so the row is judged inside it; a managed root is never judged this way. `descends_without_link` and `holds_git` now say so (the `holds_git` doc had claimed the
  descent check covers the candidate "itself included", which is false when the candidate is the anchor).
- **LOW-2, non-UTF-8 roots.** `comparable_path` compared `to_string_lossy` strings, so the roots `n\xff` and `n\xfe` were equal and one "contained" the other's
  rows (over-refusal, and `doc(move)`/`doc(delete)` could pick the wrong root). Raw bytes on Unix (X8 ADMIT).
- **LOW-3, `.git` markers.** One rule: any `.git` entry seen WITHOUT following it marks a repository (`symlink_metadata`). Chosen over "exists()" because a `.git` that is a
  link or dangling is still the author saying "this directory is a repository"; X3 (link to a directory) and X3b (dangling) now both REFUSE, X3c (a `.git` file in a worktree
  behind a link) REFUSES.
- **Known, not changed: verdicts depend on the order of the managed roots** (C5 against C5c; X6: roots `[U,N]` ADMIT the row `U/N/l -> U/other`, `[N,U]` and `[N]` REFUSE it).
  "Two spellings of one file give one verdict" holds for a FIXED root order, and the callers fix it: `tools::managed_roots` lists the active project's git root and project root
  first, then the workspace roots in file order.

**New table rows (verdicts; the file door and the directory door agree on all of them).** `door_table::door_layout_table_round6`.

| row | layout | verdict |
|---|---|---|
| X1 | roots [P], `P/../Q/docs/x.md` (W{P,Q} both repositories) | REFUSE |
| X1b | roots [P], `P/docs/../../Q/docs/x.md` | REFUSE |
| X1c | roots [P], the canonical Q row (a foreign repository's own plain row, as J2) | ADMIT |
| X1d | no roots, `P/../Q/docs/x.md` (not normalised) | REFUSE |
| X1e | roots [P], `P/../R/x.md` (R holds no `.git`) | REFUSE |
| X1f | roots [P], `P/./docs/y.md` | ADMIT |
| X1g | roots [P], `P//docs/y.md` | ADMIT |
| X1h | roots [P,W], `P/linkq/x.md` (link into Q) | REFUSE |
| X1i | roots [P,W], `P/../Q/docs/x.md` | REFUSE |
| X3 | nested dir whose `.git` is a link to a directory, its link back into P | REFUSE |
| X3b | same with a DANGLING `.git` link | REFUSE |
| X3c | `P/towt` -> a linked worktree elsewhere (its `.git` is a file), behind a link | REFUSE |
| X6 | roots [U,N], `U/N/l -> U/other` | ADMIT |
| X6b | roots [N,U], same row | REFUSE |
| X6c | roots [N] only, same row | REFUSE |
| X8 | roots [n\xff, n\xfe], row in n\xfe | ADMIT |
| Y1a | roots [unrelated], X (no repo) with `X/link -> repo R` | ADMIT |
| Y1b | roots [X], same row | REFUSE |

**Decisions of rounds 2-5 that changed:** round 5's claim that update, move, delete (and the unmentioned create) share one decision (now true; create and the root function
changed); round 5's "plain directories" refinement (now plain NAMES); round 5's handling of an unreadable file in `doc(move)` (refused up front instead of half-moved) and of a
failing repair write (reported, not an error); `.git` marker detection (`exists()` to `symlink_metadata`). Unchanged: F2/F4 refused, H2 admitted, root-order dependence.

**Mutation probe, round 6** (104-631 tests per run): killed: the non-Normal clause; `holds_git` back to `exists()`; the byte comparison back to lossy; `doc(move)` back to
`containing_root`; `doc(delete)` back to `containing_root`; `doc(create)` back to the old base-dir check; the read-before-rename; the repair's write failure turned back into an
error (the old M6, which survived round 5 because nothing could reach it, and is killed by the read-only test). **Survived:** none new; `skip(1)` to `skip(0)` stays equivalent
(round 5).

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
