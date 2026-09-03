---
id: '3e02f30a7b6a2ab3'
kind: bug
status: open
title: 'BUG: doc(action="move") writes a stale snapshot to the destination and does not delete the source, while reporting moved: true'
owners:
- marius
tags:
- cluster/record-asserts-an-unchecked-completion
opened: 2026-09-03
severity: high
---

# BUG: `doc(action="move")` writes a stale snapshot to the destination and does not delete the source, while reporting `moved: true`

## Summary

Archiving a bug file with `doc(action="move")` produced a destination file whose content
**predated two `doc(action="update")` calls made minutes earlier**, and left the source file in
place holding the current content. The response reported `"moved": true`, `"id_changed": true`,
a `history_grafted` count, and a `stage_hint` instructing the caller to expect *"a single `R`
rename line"* — none of which was true of what landed on disk.

**The non-deletion is the only reason no content was lost.** Had `move` deleted the source as
its own `stage_hint` implies it does, the newer content would be gone with nothing to recover
it from — the destination looks like a complete, well-formed bug file.

## Symptom (Effect)

`git status --short` after the move, where a single `R` was expected:

```
M  docs/issues/2026-09-03-a-bare-heading-query-cannot-reach-the-exact-match-tiers.md
A  docs/issues/archive/2026-09-03-a-bare-heading-query-cannot-reach-the-exact-match-tiers.md
```

Two files, both present, **differing**: source 15532 bytes / 295 lines, destination 14352
bytes / 279 lines.

```
$ diff <archive> <source>
< status: open                 |  > status: fixed
<                              |  > closed: 2026-09-03
< **Primary defect FIXED … The disclosure half is NOT, so this file stays open**
                               |  > **FIXED on `experiments`, both halves…**
< ### Still owed — the third remedy, and it survives this fix
                               |  > ### The second half — disclosure, and why it was worth a separate remedy
```

The destination is the file **as it stood before** `doc(action="update", patch={status,
body_edits})` and `doc(action="update", patch={extra:{closed}})`, both of which had returned
`"updated": true` and both of which were correctly present on disk at the source path.

## Reproduction

`git rev-parse HEAD` → `66e7a563` on `experiments`. Live MCP, release binary built 23:28.

1. `doc(action="update", id=X, patch={status:"fixed", body_edits:[…]})` → `updated: true`
2. `doc(action="update", id=X, patch={extra:{closed:"2026-09-03"}})` → `updated: true`
3. Confirm on disk: source path carries both changes.
4. `doc(action="move", id=X, new_rel_path="docs/issues/archive/…")` → `moved: true`
5. Destination carries **neither** change; source still exists and still carries both.

Not yet reduced to a minimal case — in particular it is not established whether step 1's
`body_edits` is required, or whether any update followed by a move reproduces it.

## Environment

Linux, `experiments`, codescout MCP over stdio, project `codescout`, catalog at
`~/.local/share/librarian/catalog.db`. Six sessions share the checkout; the write lock was
free at the time (verified via `/proc/locks` on `.codescout/write.lock`).

## Root cause

**Unknown — not established, and this file deliberately does not guess.** What is measured is
the divergence, not the mechanism. The shape is consistent with `move` sourcing the destination
body from a catalog-held copy rather than re-reading the file at `abs_path`, but that is a
hypothesis, and `src/librarian/tools/` has not been read for this. Whoever takes it should start
by finding where `move` obtains the bytes it writes.

The second half — the source not being deleted — may be the same defect or a separate one. The
response's own `stage_together` + `stage_hint` establish that deletion is intended: it tells the
caller to expect `R`, and warns that `git add -u` "takes the deletion and never enumerates the
addition", which is a statement about a deletion that did not occur here.

measured 2026-09-03 23:47: `diff`, `wc -l`, `ls -l` on both paths, quoted above.

## Evidence

### The response claimed a rename

```json
{"id": "288a1c5fcda2e73d", "previous_id": "af3a7ffe8626562c", "id_changed": true,
 "history_grafted": {"events": 4, ...}, "moved": true,
 "stage_hint": "…then confirm that `git status --short` shows a single `R` rename line."}
```

The graft is real — the catalog row at the new id resolves, carries `status: fixed`, and its
`body` field holds the *current* Fix section. So **the catalog was right and the file it wrote
was wrong**, which narrows the suspect surface to whatever serialises the body to the
destination path.

### The destination is a plausible, complete file

It has valid frontmatter, a coherent body, and the correct new `id`. Nothing in it is
malformed. A reader who did not diff it against the source would find no signal at all — which
is what makes the missing deletion load-bearing rather than merely untidy.

## Hypotheses tried

1. **Hypothesis** — a peer had edited the source after the move. **Test** — `diff` direction:
   the source is a strict superset of the destination's edits plus my two updates, and every
   difference is one I authored. **Verdict** — rejected.
2. **Hypothesis** — the updates never reached disk and only the catalog had them. **Test** —
   `head -12` on the source before repair. **Verdict** — rejected; disk carried
   `status: fixed` and `closed: 2026-09-03`.

## Fix

Not fixed. Repaired by hand for the one artifact: current source content re-stamped with the
new catalog id, written to the archive path, source removed, re-staged as a true `R` rename
(`66e7a563`).

Two things a fix owes, and they are separable:

- **Write what is on disk**, or state at the refusal site that the caller must sync first.
- **Delete the source, or stop claiming a rename.** Of the two, leaving the source is the
  *safe* half — a fix that starts deleting without also fixing the staleness converts this
  from a recoverable inconsistency into silent content loss.

## Tests added

None yet. A regression test must assert the destination is **byte-identical to the source as
it stood immediately before the call** — not merely that it exists, is non-empty, or parses.
Every one of those weaker assertions passes against the stale file this bug produced.

## Workarounds

After any `doc(action="move")`, diff the two paths before staging. If the source still exists,
that is itself the signal — treat a non-`R` `git status` as a failed move rather than a
staging mistake, which is the reading the `stage_hint` invites.

## Resume

Find where `move` obtains the destination bytes (`src/librarian/tools/`, the `move` action) and
determine whether it re-reads `abs_path` or serialises a catalog-held body. Then decide whether
the missing `unlink` is the same defect or a second one; the answer changes whether the fix is
one change or two.

## References

- Repaired in `66e7a563`; the archived artifact is `288a1c5fcda2e73d`.
- `get_guide("librarian")` § *Archiving / Moving Trackers* documents the intended contract,
  including the `R`-line expectation this call did not meet.
- Class note: filed `IC-8` on the claim test — `moved: true`, `history_grafted`, and a
  `stage_hint` describing a rename are a **record asserting a completed action that nothing
  re-checked**, and the party best placed to notice is the one who later diffs the two files,
  which nobody has reason to do. It is a new subsystem for that class (librarian artifact
  moves; existing members are bug-file frontmatter and `status: taken` claims).

