---
kind: bug
status: fixed
tags:
- librarian
- catalog-drift
- archive-flow
- frontmatter
closed: 2026-08-16
opened: 2026-08-16
owner: marius
related:
- docs/issues/archive/2026-08-16-reindex-rekeys-moved-artifacts-and-cascades-away-their-events.md
- docs/trackers/open-issue-work-queue.md
severity: low
---

# BUG: a moved artifact's frontmatter still asserts its pre-move id

## Summary

`artifact(action="move")` mints a new id (identity is `sha256(abs_path)`) and
grafts the artifact's history onto it. Nothing rewrites the `id:` line in the
file's YAML frontmatter, so from the moment the move returns, the file asserts an
id that resolves to nothing.

Split out of
`docs/issues/archive/2026-08-16-reindex-rekeys-moved-artifacts-and-cascades-away-their-events.md`
(BL-22), which fixed the data loss. This is the cosmetic residue of the same
re-key, left deliberately rather than folded in.

## Symptom (Effect)

Observed 2026-08-16 immediately after archiving BL-1 through the sanctioned path:

```
docs/issues/archive/2026-08-15-jsonpath-subset-...md:2:   id: '875e5d03d980ceac'
catalog:                                                  2bd71246fc807cba
```

```
artifact(action="get", id="875e5d03d980ceac")  ->  {"error": "unknown id `875e5d03d980ceac`"}
```

The file and the catalog disagree, and the file is the one a human reads first.

## Reproduction

1. `artifact(action="move", id=<any>, new_rel_path="docs/issues/archive/<same>.md")`
2. `read_markdown(<archived file>, start_line=1, end_line=3)` → frontmatter carries
   the **old** id
3. `artifact(action="find", filter={"rel_path": {"contains": "<slug>"}}, include_archived=true)`
   → a different id

## Environment

Linux, codescout `0.15.0`, branch `experiments`, MCP stdio, project `codescout`.

## Root cause

*Read at the bytes 2026-08-16.*

`src/librarian/tools/mv.rs` renames the file and rewrites the catalog row; it never
touches file content. The frontmatter `id:` is written at creation and never
reconciled — `src/librarian/indexer.rs:152` derives identity from `abs_path` and
does not read `id:` from frontmatter, so nothing downstream notices the mismatch.

**This is drift, not breakage.** No code path resolves an artifact through its
frontmatter `id:`. The cost is entirely to human and agent readers, who have no
signal that the line is stale — and a wrong id that *looks* authoritative is worse
than an absent one, because it invites a lookup that fails with `unknown id`
rather than sending the reader to `find`.

## Evidence

Every artifact under `docs/issues/archive/` and `docs/trackers/archive/` that was
moved through the catalog carries this. The measured case above is one of 348
archived bug files.


### 2026-08-16 — and it is now unfixable through the tools

Archiving `2026-08-16-librarian-guard-misses-quoted-frontmatter-ids.md` re-keyed it
`a2899c126f1e7771` → `e7353641aafe0098`. The file's own frontmatter still reads:

```
id: a2899c126f1e7771
```

which resolves to nothing. Every route to repair it in place is now closed:

- `edit_markdown(..., frontmatter={set: {id: …}})` — refused, the file carries a 16-hex id
  so the librarian guard fires (`29f0c015` did not change this; the shape check always did).
- `edit_file` — refused on the same predicate, all write paths (`47abcb6d`).
- `artifact(update, patch={extra: …})` — writes custom frontmatter keys but **not** `id`.

So a moved artifact's frontmatter id can currently only be corrected by the mover, at move
time. Any fix should write the new id into the frontmatter inside `artifact(action="move")`
itself, in the same transaction as the graft — not as a later repair pass, because by then
nothing can reach the file.

Note the interaction with the guard's shape-not-value check: a stale-but-well-formed id
keeps the file *protected*, so this bug is invisible from the outside. It fails safe, which
is also why it has survived this long.

### 2026-08-16, later — the guard fix widened this bug's blast radius

`29f0c015` (BL-33) made the librarian guard quoting-insensitive, taking it from 12 to 27
protected trackers and **86 more files repo-wide**. Every one of those files was previously
reachable by `edit_markdown(frontmatter={set: {id: …}})` — the one route that could have
repaired a stale frontmatter id in place. They are now refused.

Concrete instance found the same afternoon:
`docs/issues/archive/2026-08-15-conditionally-required-params-advertised-optional.md`
carries `id: '365b599f3573b1c0'` — quoted, stale (live id `02d2d9d8a7eeec2e`), and as of
`29f0c015` guarded. Before that commit it was editable; now it is not.

This is not an argument against the guard fix — protection is the right default, and a
stale-but-well-formed id fails safe. It is an argument that **the repair must move into
`artifact(action="move")`**, which the previous section already concluded on other grounds.
What changed is the cost of not doing it: the population of unrepairable stale ids grew by
roughly 7× in one commit, and every future archive adds to it.

The generalisable shape, worth stating once: *a guard that closes a write path also closes
the repair path that used that write.* When widening a guard, enumerate what legitimately
wrote through the old hole — here, nothing did yet, which is precisely why it was invisible.
## Hypotheses tried

1. **Hypothesis:** the indexer reconciles `id:` on the next reindex.
   **Test:** read `src/librarian/indexer.rs:120-180`.
   **Verdict:** rejected — `id` comes from `artifact_id_from_abs(path)`;
   frontmatter is parsed for `kind`/`status`/`title`/`owners`/`tags`/`topic` only.

## Fix

**Half fixed 2026-08-16 on `experiments` in `ec9e63d0`.** New moves no longer create the
defect; the existing population is untouched and still needs a repair path (see *Resume*).
Status stays `open` for that reason — the mechanism is fixed, the bug's stated population is
not, and marking it `fixed` would make it indistinguishable from the genuinely-done ones in
any later triage sweep.

Verified live against the running MCP server, on a throwaway artifact created and deleted
in the same turn (archiving a real bug to satisfy a verification ritual would be
manufacturing tracker work):

```
artifact(create, rel_path="docs/tmp-bl23-move-verify.md")   -> id ceb2c39e67335191
artifact(move,   new_rel_path="docs/archive/…")            -> id e308b7f1e150d81f,
                                                               previous_id ceb2c39e67335191
head -3 docs/archive/tmp-bl23-move-verify.md
  ---
  id: e308b7f1e150d81f          <- the new id, not the old one
  kind: note
artifact(delete)                                           -> deleted, no residue
```

`artifact(action="move")` now rewrites the frontmatter `id:` to the id it just minted, in
the same call as the graft — `repair_frontmatter_id` in `src/librarian/tools/mv.rs`. Three
decisions worth keeping:

1. **Only an id already present is rewritten.** `frontmatter::update_in_place` *inserts* a
   block when none exists, so applying it unconditionally would stamp an `id:` onto files
   that never had one — and a stamped id is precisely what subjects a file to the librarian
   guard. Archiving `docs/trackers/skill-frictions.md` would have silently made
   `edit_markdown` refuse it, breaking the workflow CLAUDE.md documents. A file with no
   `id:` is not asserting anything false.
2. **`file_mtime` and `file_sha256` are taken AFTER the rewrite.** They were computed before
   `new_id` even existed; hashing pre-repair content would record a digest of a file that no
   longer exists on disk, leaving the row looking dirty on every subsequent walk.
3. **Best-effort, never fatal.** The rename has already happened when this runs, so
   unparseable frontmatter must not abort the move and strand the catalog mid-update. It
   logs and returns the original content; the re-key and graft still complete.
## Tests added

Both in `src/librarian/tools/mv.rs`, both watched fail first.

- `move_rewrites_the_frontmatter_id_it_just_invalidated` — reproduced the bug exactly on the
  red run: file `aabbccdd11223344`, catalog `e59346e2f3e5c221`. Asserts the body is
  byte-untouched and sibling frontmatter fields survive. **The `file_sha256` assertion is
  the load-bearing one** — it is what fails if the rewrite lands after the hash is taken,
  a bug the id assertion alone would not see.
- `move_does_not_stamp_an_id_onto_a_file_that_never_had_one` — the half a naive
  `update_in_place` would break. Green before the fix by construction; it exists to pin the
  behaviour the fix must not change.

Gate: 3908 tests (full `cargo test`), clippy `-D warnings`, fmt.
## Workarounds

Resolve archived artifacts by path, not by the id printed in their frontmatter:

```
artifact(action="find", filter={"rel_path": {"contains": "<slug>"}}, include_archived=true)
```

## Resume

N/A — mechanism fixed, population repaired, and a tool exists for the next one.

| step | commit |
|---|---|
| `move` rewrites the id it mints | `ec9e63d0` |
| `doctor` check + sweep fix | `05cf7ed5` |
| sweep scoped to one root before it writes | `3db48ebf` |
| the 91-file repair itself | `79c6beb8` |

**The sweep, run live:** 91 repaired, 0 failed, and a second dry-run reports 0 — idempotent.
78 archived bugs, 8 archived trackers, 5 **active** trackers, 1 README.

**`move` was not the main cause**, which the population made obvious once it was listed:
five of the 91 are active trackers that were never moved. `src/librarian/catalog/migrate_v6.rs`'s
module doc had already written down why — Round 5 of the Windows CI rehab changed
`ids::artifact_id_from_abs` to hash the forward-slash-normalized path form, *"same `abs_path`
produces a new `id`"*, and *"external citations to the old IDs go stale — that's the documented
user-visible cost."* A file's own frontmatter id is exactly such a citation. This bug's title
names the minority cause; the dominant one is a hash-form change that re-keyed rows in place.
What shipped converts that accepted cost into a repaired one.

**On the diff being bigger than predicted.** I said "the `id:` line only" from two sampled
files and was wrong: `frontmatter::write` re-serializes, dropping `topic: null` (27),
`time_scope: null` (29), `owners: []` (17), `tags: []` (4) and unquoting `opened:`/`closed:`
dates. All value-preserving — established by classifying **every** added and removed line,
which accounted for 100% of them and left no body line touched. The empty-`tags` case was
the one that mattered to check; all four were `[]`, no values lost. A bigger sample would not
have settled it — classifying the whole diff did.

**Residual:** other repos on this machine carry the same drift (a pre-scope dry-run counted
207 files across five). Repairing them is one call each with `root=<repo>`, deliberately not
done from here.
## References

- `src/librarian/tools/mv.rs` — the move, which rewrites the row but not the file
- `src/librarian/indexer.rs:120-180` — identity derived from path, frontmatter `id:` unread
- `docs/issues/archive/2026-08-16-reindex-rekeys-moved-artifacts-and-cascades-away-their-events.md` — BL-22
- `docs/trackers/open-issue-work-queue.md` — BL-23
