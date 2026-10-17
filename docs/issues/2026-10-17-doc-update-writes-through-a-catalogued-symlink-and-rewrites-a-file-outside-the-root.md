---
status: fixed
opened: 2026-10-17
closed: 2026-10-17
severity: high
owner: marius
related: [docs/issues/2026-10-17-doc-create-on-a-dangling-symlink-creates-a-file-outside-the-root.md, docs/issues/2026-10-17-edit-file-on-a-leaf-symlink-to-a-deny-listed-path-reads-and-copies-the-file.md, docs/issues/2026-10-17-augmentation-sidecar-write-through-rewrites-any-yaml-a-document-names.md, docs/issues/2026-10-17-write-allowlist-canonicalizes-only-the-parent-so-a-leaf-symlink-or-a-missing-subdir-escapes.md, docs/issues/2026-10-17-atomic-write-follows-a-planted-staging-symlink-and-overwrites-a-file-outside-the-root.md]
tags:
- cluster/unclassified
kind: bug
unverified: 'The helper checks the LEAF only: a catalogued row whose PARENT directory is a link is not detected by write_catalogued_file (such a row can only be born from a doc(create) before its fix, or by hand). Same-class sites NOT fixed here: src/librarian/catalog/audit/shard.rs OpenOptions append and src/tools/onboarding.rs raw fs::write of .codescout/project.toml (see the site table). The check and the write are two syscalls, so a link planted between them still wins.'
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
- Doc-vs-code drift found on the way: the comment above the `is_symlink` block said this repo's one kept outward link is
  `.claude/skills/claude-traces`. It is a DIRECTORY link; `WalkBuilder` does not follow links and `is_file()` is false
  for it. Measured by `a_symlinked_directory_is_not_descended_into_by_the_walk` (`1f25f518`), which also corrects the
  comment. No outward link is catalogued in this repo; the exposure is a repository that ships a `.md` FILE link.

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

- `2cf7dd7b` patch-id `fd84fb63b6f18f248115a738bdbeb2e91c179b93` (this issue and the sweep)
- `1f25f518` patch-id `4cf84bfb0bccdb8034802fdc54a913cfbbca9cb3` (walk does not descend a directory link; comment corrected)
- the sidecar declared-path defect found by the sweep: `d703c12b` patch-id `f5e66cb1e077da8dd841797973a0519517ee586e`
  (own file: `2026-10-17-augmentation-sidecar-write-through-rewrites-any-yaml-a-document-names.md`)

### Per-site decision table (full sweep of `fs::write` / `File::create` / `OpenOptions` / `fs::copy` / `atomic_write` in `src/librarian` and `src/tools`, production code only, as of `27efb11d`)

"Leaf link reaches it" = a catalogued (or committed) link at the written path itself. "Parent link" = a symlinked
directory above it.

| site | path comes from | leaf link reaches it | parent link | decision |
|---|---|---|---|---|
| `tools/update.rs` `call` (was :775) | `row.abs_path` | yes, measured | only a row born below a link dir | **fixed**: `write_catalogued_file` |
| `tools/update.rs` `write_field_to_frontmatter` (was :1000) | `row.abs_path` | yes: reached from `doc(event_create)` `status_change`/`field_patch` (event_create.rs:261, :271) and `doc(link)` `supersedes` (link.rs:62) | same | **fixed**: `write_catalogued_file`; tests via the writer, via `event_create` and via `doc(link)` |
| `catalog/augmentation.rs` `append_entry` section write (was :1023) | row's `abs_path` | yes | same | **fixed**; the params row rolls back with the refusal |
| `catalog/augmentation.rs` `restore_section_after_failed_commit` (was :1091) | the path just written | only a link planted between the write and the restore (TOCTOU) | n/a | **fixed** (same door), so the rollback cannot write through a link either |
| `catalog/augmentation.rs` `allocate_entry_id` (was :1953) | row's `abs_path` | yes | same | **fixed**; refused before `tx.commit()`, so no id is consumed (asserted) |
| `catalog/augmentation.rs` `resync_snapshot_row` (was :2222) | row's `abs_path` | yes | same | **fixed**; params change rolls back (asserted) |
| `catalog/rekey.rs` `rekey_body` (was :470) | row's `abs_path` | yes | same | **fixed**; params rewrite rolls back (asserted). Guard sits after the preview/no-op returns, so a preview never fails on it |
| `tools/mv.rs` `repair_frontmatter_id` (was :638) | the moved path (a renamed link) | yes, measured | n/a | **fixed**: a link is SKIPPED with a warning (the rename has happened; best-effort by design), move still succeeds |
| `tools/doctor.rs` `repair_frontmatter_id` sweep (calls the mv writer) | row paths | yes, measured | n/a | **fixed**: the sweep refuses before calling it, so a link is reported under `failed`, not `repaired` |
| `tools/doctor.rs` `stamp_augmentation_declaration` (was :2042) | row's `abs_path` | yes via `export_augmentations` | same | **fixed**: the export loop refuses BEFORE writing the sidecar (else a sidecar nothing declares is left) and the stamp goes through the door |
| `augmentation_sidecar.rs` `write` (was :178) | repo root + rel path | yes: a committed sidecar link, or a dangling one | n/a | **fixed**: through the door |
| `augmentation_sidecar.rs` `write_through` (was :312) | git root + `rel` from the document's frontmatter | yes: `is_file()` follows the link, so it arrives as an ordinary sidecar | n/a | **fixed**: through the door; plus a SECOND defect, the declared path itself (own file, `d703c12b`) |
| `tools/create.rs` `call` (was :481) | `base_dir.join(rel_path)` | dangling: yes; existing link: already refused | **yes: `esc/new.md`** | **fixed** in the `doc(create)` issue |
| `catalog/audit/shard.rs` `OpenOptions::append` (:622) | repo root + fixed dir + `<host>-<month>.jsonl` | yes if a committed link carries the victim host's shard name | n/a | **NOT fixed.** Same class; the helper applies as is (`refuse_write_through_symlink(&path)?` before the open). Left because its test needs the audit-export fixture and it is not one of the three issues. Follow-up |
| `workspace.rs` `create_default` | `workspace.toml` | **no**: `create_new(true)` is `O_EXCL`, which refuses an existing symlink, dangling included | n/a | no change |
| `tools/edit_file/mod.rs` x3 (`atomic_write`) | caller path via the write classifier | no write escape: `atomic_write` renames over the link. The READ before it was the defect | classifier resolves the parent | **fixed** in the `edit_file` issue (`validate_leaf_link_target`) |
| `tools/symbol/edit_code.rs` symbol edits (`atomic_write`) | `resolve_write_path_for` | same as above | same | **fixed** in the `edit_file` issue (`resolve_write_path_for`) |
| `tools/symbol/edit_code.rs` rename plan (`fs::write`, :710/:713) | LSP-reported paths | **yes: `fs::write` follows**, if the LSP names the in-root spelling of a leaf link | classifier resolves the parent | **fixed** with the `Writable` policy of `validate_leaf_link_target`; the wiring is NOT integration-tested (no mock-LSP rename harness) and survives a mutation, see Tests added |
| `tools/symbol/edit_code.rs` (`atomic_write`, :1710) | `full_path` from the resolver | see symbol edits | | covered by the resolver |
| `tools/onboarding.rs` (:113, :158, :1068) | `<root>/.codescout/project.toml` | **yes**: raw `fs::write`, a repository can ship that name as a link | n/a | **NOT fixed.** Same class, outside these issues; the fix is `atomic_write` or a leaf refusal. Follow-up |
| `tools/onboarding.rs` (:464, :525, :534, :861) | a temp dir / `workspace.toml` | temp dir: no. `workspace.toml` (:861): yes, same as above | n/a | **NOT fixed** (same follow-up) |
| `tools/run_command/attribution.rs` (:265) | scratch | not examined | | excluded by the brief (another agent's file) |

Not in scope and not examined: `src/agent`, `src/lsp`, `src/memory`, `src/peer`, `src/retrieval`, `src/logging.rs`,
`src/heartbeat.rs`, `src/operator_rules`, `src/library` (internal state files; `memory`, `operator_rules` and `library`
already use `atomic_write`). The sweep covered `src/librarian` and `src/tools` as asked.

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

## Workarounds

Do not keep `.md` symlinks that point outside the project root in a repository that uses the librarian. With the fix such
a row is read-only through the librarian: edit the file the link points at under its own path.

## Resume

N/A. Follow-ups for the reviewer: the two NOT-fixed rows above (`shard.rs`, `onboarding.rs`); the leaf-only limit (a catalogued
row below a link DIRECTORY is not detected by the write helper; `doc(create)` no longer mints one).

## References

- `docs/issues/2026-10-17-write-allowlist-canonicalizes-only-the-parent-so-a-leaf-symlink-or-a-missing-subdir-escapes.md`
- `docs/issues/2026-10-17-atomic-write-follows-a-planted-staging-symlink-and-overwrites-a-file-outside-the-root.md`
- `docs/issues/2026-10-17-doc-create-on-a-dangling-symlink-creates-a-file-outside-the-root.md`
- `docs/issues/2026-10-17-edit-file-on-a-leaf-symlink-to-a-deny-listed-path-reads-and-copies-the-file.md`
- `docs/issues/2026-10-17-augmentation-sidecar-write-through-rewrites-any-yaml-a-document-names.md`
