---
id: '861b565a934fcb2c'
kind: bug
status: open
title: 'BUG: artifact(update) re-serializes the whole frontmatter block on a single-field patch, so the mandated archive step reformats hand-authored YAML'
tags:
- librarian
- frontmatter
- update
- archive
- blast-radius
opened: 2026-08-16
owner: marius
related:
- docs/issues/archive/2026-08-16-frontmatter-id-repair-reserializes-the-whole-block.md
severity: medium
---

# BUG: artifact(update) re-serializes the whole frontmatter block on a single-field patch, so the mandated archive step reformats hand-authored YAML

## Summary

BL-34 fixed the round-trip in `mv::repair_frontmatter_id`. The **same round-trip is still
live** in `librarian/tools/update.rs`, which is the path
`artifact(action="update", patch={"status": "archived"})` takes — the archive step
`get_guide("tracker-conventions")` and `CLAUDE.md` both mandate.

Patching one scalar field re-emits the entire frontmatter block in canonical form: quotes
dropped, flow sequences exploded to block style, null keys deleted, nested maps re-indented,
key order replaced by the struct's field order, and `{Placeholder}` values — valid YAML for a
*flow mapping* — expanded into nested keys and destroyed.

This is not a new mechanism. It is BL-34's mechanism at a second call site, and BL-34's own
Fix section waved it through in one sentence: *"Keep `update_in_place` for callers that want
normalization (`artifact(update)`); it is the right primitive there."* That was asserted from
the code, never measured. The doctor sweep supplied the evidence, so the fix went where the
evidence was.

## Symptom (Effect)

Measured 2026-08-16 with a purpose-built probe artifact. **One** field patched:

```
patch = {"status": "archived"}
```

Seven lines changed. Six of them unrequested:

```diff
  status: draft              ->  status: archived              # intended
- title: "Frontmatter Writer Probe"
+ title: Frontmatter Writer Probe                              # dequoted
- tags: [alpha, beta]
+ tags:
+ - alpha
+ - beta                                                       # flow -> block
- created: {YYYY-MM-DD}
+ created:
+   YYYY-MM-DD: null                                           # PLACEHOLDER DESTROYED
- topic: null                                                  # dropped, no replacement
- nested:
-     deep: value
+ nested:
+   deep: value                                                # re-indented
  owner: marius                                                # REORDERED below `nested`
```

The `created:` line is the same destruction BL-34 documented in
`eduplanner-ui`'s `docs/adr/templates/adr-template.md`.

## Reproduction

Fully scripted, ~60s, leaves nothing behind:

1. `create_file` a doc under `docs/plans/` with hand-authored frontmatter — no `id:`, a
   double-quoted title, `tags: [alpha, beta]`, a `{YYYY-MM-DD}` value, `topic: null`, a
   nested map at 4-space indent, and a key order no serializer would pick.
2. `cp` it to a scratch path as the byte reference.
3. `librarian(action="reindex")` — assigns a catalog id.
4. `artifact(action="update", id=<id>, patch={"status": "archived"})`.
5. `diff <scratch-copy> <repo-file>`.
6. `artifact(action="delete", id=<id>)` — removes file and catalog row.

**Step 3 is a clean negative control and a genuine finding on its own:** `reindex` did NOT
rewrite the file. `diff` exit 0, byte-identical. It indexes into the catalog and leaves the
bytes alone — so the reformat is attributable to `update` and to nothing else in the flow.
(Note this also corrects `get_guide("librarian")`'s "the librarian assigns `id:` on the next
`reindex`" — it assigns in the **catalog**, not in the file.)

## Environment

`experiments` at `374f71f5`. Live MCP server built 2026-08-16T20:35 — i.e. **with** BL-34's
fix (`858f22ec`, 19:42) already in it. This path was never covered by that fix.

## Root cause

`src/librarian/tools/update.rs::call` has three frontmatter-touching branches, all fed by
`apply_frontmatter_patch` (`update.rs:111-134`), whose own doc comment names them:

| branch | frontmatter write | guarded? |
|---|---|---|
| `patch.body` (full overwrite) | `frontmatter::write(&fm, …)` | no — unconditional |
| `patch.body_edits` | `update_in_place` if `fm_changing` | **yes** — body-only edits do not touch the block |
| else (plain field patch) | `update_in_place` | n/a — a field *is* the change |

The middle branch is careful work and is not the defect. Branches 1 and 3 route to
`frontmatter::write`, which re-emits every field from the parsed `Frontmatter` struct. Body
bytes survive; frontmatter bytes do not.

**The naming inverts the contracts**, which is why this was easy to wave through.
`frontmatter.rs` now holds two writers:

| primitive | actual contract | production callers |
|---|---|---|
| `update_in_place` | **normalizing** — re-emits the whole block | `tools/update.rs` x3 |
| `replace_id_line` | **preserving** — one line's bytes | `tools/mv.rs` x1 |

`update_in_place` reads as the surgical one. It is the reformatter. The next engineer needing
"change one field without disturbing the file" reaches for it by name.

A second, smaller divergence between the two writers is already observable in this repo. The
BL-34 bug file's own archive shows it:

```diff
-id: '60b5323f5e11be66'      # written by frontmatter::write   (quoted)
+id: 529a6c05895cc686        # written by replace_id_line      (bare)
```

Same field, same value class, two renderings, no shared contract.

## Evidence

Blast radius, stated honestly — this is **smaller** than BL-34's and should be weighed that
way:

- BL-34: one call rewrote 26 files across a repo the user did not have open.
- This: one deliberate call rewrites one named artifact.

What makes it worth fixing anyway is *which* call. `artifact(action="update",
patch={"status": "archived"})` is not an unusual invocation — it is the documented terminal
step of every tracker's life, it is reachable across repos via umbrella scope, and it is
issued by agents following this project's own guide. The files most likely to be on the
receiving end are sibling repos' hand-authored ADRs, which is exactly the corpus BL-34 proved
is vulnerable.

## Hypotheses tried

1. **Hypothesis:** normalization is intended at `artifact(update)`, so this is by design.
   **Test:** run the probe and read the diff.
   **Verdict:** partly. Canonical-in/canonical-out is defensible for librarian-owned files
   and is why codescout's own corpus shows no churn. It is not defensible for
   `created: {YYYY-MM-DD}` -> `created:\n  YYYY-MM-DD: null`, which is destruction of intent,
   not tidying. "Intended for the common case" and "safe in every case" are different
   properties; only the first holds.

2. **Hypothesis:** `reindex` also rewrites files, so the reformat has more than one source.
   **Test:** `diff` the probe before and after `librarian(action="reindex")`.
   **Verdict:** rejected — byte-identical, exit 0. `update` is the sole writer here.

## Fix

Not implemented. Proposed, in ADR form:

**Decision:** name the boundary in `frontmatter.rs` — a *normalizing* writer and a
*preserving* writer — and route single-scalar field patches through the preserving one.
Generalize `replace_id_line` to `replace_scalar_line(doc, key, value)`, using the identical
delimiter scan, the identical `scalar_can_be_bare` quoting, and the identical decline-rather-
than-guess contract.

**Context:** one repair path was fixed on measured evidence; its sibling was reasoned about
and left. The property that distinguishes them is not *which tool called* but *whose form the
file is* — librarian-owned or human-owned.

**Alternatives considered:**

- *Leave it.* One file per deliberate call is a real argument, and this may sit below the
  line. Rejected because the call is the mandated archive step and the failure is silent.
- *Rename only* (`update_in_place` -> `rewrite_frontmatter_normalizing`). Cheapest, and
  captures much of the value, since the defect is that the name misleads. Viable as a first
  commit even if the splice never lands.
- *Guard: refuse when the round-trip would change more than the targeted line.* Rejected for
  the same reason BL-34 rejected it — a guard sited where a fix belongs.
- *Report `reformatted: true` in the response.* Honest, still reformats.

**Consequences:**

- now easier: the documented archive step stops being destructive on hand-authored YAML;
  the two primitives' names finally describe their contracts.
- now harder: two write paths for one field. `owners`/`tags` (sequences) and *absent* fields
  have no line to splice and must still go through `write`, so the split is conditional and
  the tests must pin which path fired. That is real cost and it is the strongest argument for
  the rename-only option.

**Change scenarios absorbed:** archiving a hand-authored tracker or ADR in any repo reachable
via umbrella scope; any future single-field repair arm on `doctor`.

**Revisit-when:** if a third caller needs single-field frontmatter writes, the duplication
dictates the shape and the abstraction stops being a guess (rule of three).

**Confidence:** high on the defect (measured); medium on the fix shape — the sequence and
absent-field cases keep `write` alive, so the boundary is not as clean as BL-34's was.

## Tests added

None — not fixed. The regression test must assert **byte equality outside the patched line**
against a fixture hostile to normalization: flow sequence, double-quoted title,
`{Placeholder}` value, null key, nested map at non-canonical indent, and a key order the
struct would not produce. Asserting only that `status` changed is what let this through.

Drive it through the **real tool** — `artifact(action="update")` — not through
`update_in_place` directly. This session has now hit the "test exercises the mechanism but
not the call site" shape five times; a unit test on the primitive cannot catch a caller that
routes around it.

## Workarounds

For a hand-authored artifact in a foreign repo, flip status by editing the file directly
(`edit_markdown` frontmatter edit) and re-running `librarian(action="reindex")`, which is
measured non-destructive. Or accept the reformat, review `git diff` before committing, and
restore any `{Placeholder}` lines by hand.

## Resume

Decide between *rename-only* and *rename + `replace_scalar_line`*. The rename is a small,
safe first commit and removes the trap that caused this; the splice is the actual fix and
carries the conditional-path cost named above.

Whichever is chosen, re-run the probe in this file's Reproduction section afterwards — it is
six calls, self-cleaning, and it is what turned this from an inference into a measurement.

## References

- `docs/issues/archive/2026-08-16-frontmatter-id-repair-reserializes-the-whole-block.md` — BL-34, the same mechanism at `mv`
- `src/librarian/tools/update.rs:111-134` — `apply_frontmatter_patch` and its three branches
- `src/librarian/frontmatter.rs:180-185` — `update_in_place`, the normalizing writer
- `src/librarian/frontmatter.rs:203-244` — `replace_id_line`, the preserving writer

