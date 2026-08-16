---
id: 82ba248228301486
kind: bug
status: fixed
title: 'BUG: artifact(update) re-serializes the whole frontmatter block on a single-field patch, so the mandated archive step reformats hand-authored YAML'
tags:
- librarian
- frontmatter
- update
- archive
- blast-radius
closed: 2026-08-16
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

Fixed on `experiments`. Both halves of the proposed decision landed, plus a third branch the
proposal did not know about.

**The rename.** `update_in_place` → `rewrite_frontmatter_normalizing`. The name now states the
contract, so the next engineer needing "change one field without disturbing the file" no
longer reaches for the reformatter by reading. Four tests carrying the old name were renamed
with it — a test named after a function that no longer exists is the same defect one layer up.

**The preserving writer, generalized.** `replace_id_line` →
`replace_scalar_line(doc, key, value)`. The key was already a hardcoded literal inside that
function; parameterizing it removes a constant rather than inventing an interface, so the
rule-of-three discipline is not in tension here. Same delimiter scan, same
`scalar_can_be_bare` quoting, same decline-rather-than-guess contract.

**Two hazards the id-only ancestor could not reach**, found while generalizing and both
mutation-verified:

- *An apostrophe in the value.* Ids are hex; a title or topic is not. Emitted raw inside
  single quotes it closes the quote early and corrupts **every following line of the block** —
  a one-field write costing the whole document. Now escaped by doubling, per YAML
  single-quoted style.
- *A newline in the value.* It cannot be one line, so there is no splice to make.
  `replace_scalar_line` declines and the caller re-serializes.

**The third branch, which measuring found and prose would have excused.** The proposal named
two call sites. Rather than repeat this bug's own root cause — a sibling waved through in
writing — the `patch.body` branch got a test instead of a sentence. It failed, with the
identical seven-line signature: a body overwrite was re-emitting the frontmatter because
`write(&fm, new_body)` rebuilds the whole document. Fixed with `replace_body(doc, new_body)`,
the mirror-image primitive: everything through the closing delimiter is the author's, only
what follows changes.

**Wiring.** `patch_changes_frontmatter(patch)` and `try_preserving_frontmatter_patch(doc,
patch)` in `update.rs`. All three branches now try the splice first and fall back to
`rewrite_frontmatter_normalizing` only when one cannot express the change — an absent key, a
sequence field (`owners`/`tags`), an `extra` map, a newline value, or no frontmatter block at
all. The splice is **all-or-nothing**: the first key with no line aborts the whole attempt,
because a half-spliced, half-stale block is worse than one honest re-serialization.
## Tests added

**Two in `update.rs`, both watched fail first, both driving the real tool** — not
`rewrite_frontmatter_normalizing` directly, because a unit test on a shared primitive cannot
catch a caller that routes around it:

- `patching_one_scalar_field_leaves_every_other_frontmatter_byte_alone` — RED reproduced all
  seven lines in the harness, including `created:` → `YYYY-MM-DD: null` and the `id` quote
  flip between the two writers.
- `overwriting_the_body_leaves_the_hand_authored_frontmatter_alone` — this one was written to
  *test* an assumption rather than confirm a known defect, and it failed. That is the whole
  reason it exists.

Both use a fixture hostile to normalization: double-quoted title, flow sequence,
`{YYYY-MM-DD}` flow mapping, explicit null, nested map at four-space indent, and a key order
the struct does not produce.

**Four new in `frontmatter.rs`**, covering the genuinely-new code paths:
`replace_scalar_line_escapes_an_apostrophe_in_the_value`,
`replace_scalar_line_declines_a_value_that_cannot_be_one_line`,
`replace_body_preserves_the_frontmatter_block_verbatim`,
`replace_body_preserves_crlf_frontmatter`.

**Mutation-verified.** Both new guards were broken deliberately and each killed exactly its
own test — the apostrophe mutation failing with `malformed frontmatter YAML: did not find
expected key`, i.e. the whole block unparseable, which is the predicted corruption. **22
sibling tests stayed green**, and that number is the measure of what the suite was blind to
before these two.

Gate: **3965 tests**, `cargo clippy --all-targets -- -D warnings`, `cargo fmt`.
## Workarounds

For a hand-authored artifact in a foreign repo, flip status by editing the file directly
(`edit_markdown` frontmatter edit) and re-running `librarian(action="reindex")`, which is
measured non-destructive. Or accept the reformat, review `git diff` before committing, and
restore any `{Placeholder}` lines by hand.

## Resume

None — all three frontmatter-touching branches fixed and covered.

One thing worth carrying forward rather than closing: this bug was *caused* by a sentence in
BL-34 excusing an unmeasured sibling, and during its own fix the same shape appeared again —
a third branch the proposal had not counted. It cost one test to find. **The probe is the
cheap move; the sentence is the expensive one.** Recorded in the Snow Lion's
`platform-law-leaks-at-call-sites` memory as datapoint (5) with a fourth standing note.
## References

- `docs/issues/archive/2026-08-16-frontmatter-id-repair-reserializes-the-whole-block.md` — BL-34, the same mechanism at `mv`
- `src/librarian/tools/update.rs:111-134` — `apply_frontmatter_patch` and its three branches
- `src/librarian/frontmatter.rs:180-185` — `update_in_place`, the normalizing writer
- `src/librarian/frontmatter.rs:203-244` — `replace_id_line`, the preserving writer
