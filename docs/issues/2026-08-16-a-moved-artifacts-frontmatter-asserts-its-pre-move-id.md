---
status: open
opened: 2026-08-16
closed:
severity: low
owner: marius
related:
  - docs/issues/archive/2026-08-16-reindex-rekeys-moved-artifacts-and-cascades-away-their-events.md
  - docs/trackers/open-issue-work-queue.md
tags:
  - librarian
  - catalog-drift
  - archive-flow
  - frontmatter
kind: bug
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

## Hypotheses tried

1. **Hypothesis:** the indexer reconciles `id:` on the next reindex.
   **Test:** read `src/librarian/indexer.rs:120-180`.
   **Verdict:** rejected — `id` comes from `artifact_id_from_abs(path)`;
   frontmatter is parsed for `kind`/`status`/`title`/`owners`/`tags`/`topic` only.

## Fix

Two candidates, cheapest first:

1. **Have `move` rewrite the `id:` line** as part of the same call, before the
   sha256 of the content is recorded (order matters — `mv` computes `file_sha256`
   from the file it just renamed, so a rewrite has to land before that read or the
   row's hash goes stale immediately).
2. **Stop writing `id:` into frontmatter at all.** It is redundant with the
   catalog, derivable from the path, and this bug is the second time it has drifted.
   The cost is that a file read outside the catalog loses its identity hint.

Prefer 1 if the `id:` line is load-bearing for any human workflow; prefer 2 if it
is not. Worth checking which before implementing.

## Tests added

None yet — bug is `open`.

## Workarounds

Resolve archived artifacts by path, not by the id printed in their frontmatter:

```
artifact(action="find", filter={"rel_path": {"contains": "<slug>"}}, include_archived=true)
```

## Resume

Decide between the two Fix options first, and check whether anything actually
consumes the frontmatter `id:` line before choosing — `grep` for `id:` readers in
`src/librarian/` and in the companion plugin. If nothing does, option 2 removes the
class instead of patching it.

If option 1: `src/librarian/tools/mv.rs` computes `file_sha256` from the renamed
file; a frontmatter rewrite must happen before that, or the catalog row records the
hash of content the move is about to change.

## References

- `src/librarian/tools/mv.rs` — the move, which rewrites the row but not the file
- `src/librarian/indexer.rs:120-180` — identity derived from path, frontmatter `id:` unread
- `docs/issues/archive/2026-08-16-reindex-rekeys-moved-artifacts-and-cascades-away-their-events.md` — BL-22
- `docs/trackers/open-issue-work-queue.md` — BL-23
