---
id: '1ed1bc1a845bd0c6'
kind: bug
status: open
title: 'BUG: rekey_prefix moves the body and the catalog but leaves entry_prefix and entry_high_water_<PREFIX> behind, splitting the ledger'
tags:
- cluster/selector-narrower-than-its-population
opened: 2026-09-24
owner: marius
related: []
severity: medium
---

## Summary

`doc(action="rekey_prefix")` moves a namespace through the body (headings, index rows, prose
self-citations), the params ids, the schema pattern, the prompt and the catalog reservation, and
**never touches the frontmatter** — so the ledger's `entry_prefix` declaration and its committed
`entry_high_water_<PREFIX>` key keep the OLD prefix. The result is a split ledger: headings define
`NEW-N`, the declaration names `OLD`, and the allocator (which reads the declaration) issues
`OLD-N+1` to the next caller while refusing `NEW` as undeclared. The response reports `applied:
true` and a `next_step` about citing files only; nothing names the frontmatter.

## Symptom (Effect)

Measured 2026-09-24 on the first real use of the action this session, renaming the observation
ledger `DCTX` → `DCX` (#59, artifact `0cc578bbc332d699`). After
`rekey_prefix(from="DCTX", to="DCX", force=true)` returned `body_lines_rewritten: 2,
reservation_moved: true`, the file read:

```
entry_prefix:
- DCTX
entry_high_water_DCTX: 1
---
…
| DCX-1 | 2026-09-18 | historical-seed | … |
## DCX-1 — Historical seed — recipient-specific guide delivery
```

On the served binary, `append_entry(id_prefix="DCTX")` — what the ledger's own recipe said —
would have allocated `DCTX-2` under a heading scheme that now defines `DCX-N`.

## Reproduction

`src/librarian/catalog/rekey.rs`, test
`the_declaration_and_high_water_mark_move_with_the_body`, red at the pre-fix tree:

```
block sequence: declaration must move:
---
kind: tracker
entry_prefix:
- DCTX
entry_high_water_DCTX: 1
---

## DCX-1 — first
```

## Environment

codescout `experiments` @ `a86086ea` + working tree, main checkout, Linux. Live repro against the
served MCP binary; test repro in the per-session gate target.

## Root cause

`rekey_body` builds the new body and writes it through `frontmatter::replace_body`, which
preserves the frontmatter block **byte-for-byte** by design (`rekey.rs`, the comment above the
`replace_body` call). Nothing else in the action writes frontmatter. The module doc names the
hazard in its own words — *"`entry_reservation` … is a second copy of the committed frontmatter
`entry_high_water_<PREFIX>` key, and moving only the frontmatter half leaves the allocator issuing
ids under the old prefix"* — and then moves only the OTHER half, the catalog one. The claim
"moves a whole PREFIX-N namespace at once" names a population whose selector excluded the two
frontmatter lines that are the allocator's committed inputs.

No test caught it because every rekey fixture's frontmatter was either absent, `kind: tracker`
alone, or asserted to survive **unchanged** (`… the frontmatter block must survive
byte-for-byte`) — the suite pinned the defect as a property.

## Evidence

- The live file state above, read with `sed -n '1,20p'` immediately after the apply.
- `git diff` of the same file showed exactly two changed lines, both body (`:84`, `:86`).
- Repaired by hand with `doc(action="update", patch={extra: {entry_prefix: ["DCX"],
  entry_high_water_DCX: 1, entry_high_water_DCTX: null}})`, then the recipe prose moved with
  `body_edits`.

## Hypotheses tried

1. **The frontmatter is rewritten in a later step the preview did not report.** Rejected — the
   apply's own file state is the one quoted above.
2. **Moving the declaration is the caller's job by design.** Rejected — the action's own
   `next_step` names only *"citing files outside this ledger"*, and the tool description promises
   the whole namespace; a caller has no signal that the declaration was left behind.

## Fix

`rekey_frontmatter` (in `rekey.rs`) rewrites, line-surgically, this ledger's member of
`entry_prefix` (scalar, flow list or block sequence; whole-token, so renaming `T` leaves `TX`) and
renames `entry_high_water_<from>` to `entry_high_water_<to>`. The write gate now fires on a
frontmatter-only change (a ledger with no entries yet). A new refusal, in `Preview` too, covers a
ledger that already declares or records `to` — without it, `[F, W]` would become `[W, W]`.
`RekeyReport` gains `frontmatter_lines_rewritten`.

## Tests added

In `src/librarian/catalog/rekey.rs`:
`the_declaration_and_high_water_mark_move_with_the_body` (three YAML forms, including `[T, TX]`
where a substring replace corrupts the sibling), `rekeying_onto_a_prefix_this_ledger_already_declares_is_refused`
(both halves of the disjunction), `a_ledger_with_no_entries_yet_still_moves_its_declaration`.

## Workarounds

After a rekey on the pre-fix binary, move the declaration by hand:
`doc(action="update", id=…, patch={extra: {entry_prefix: [<new>], entry_high_water_<NEW>: <n>,
entry_high_water_<OLD>: null}})`.

## Resume

Fix lands with #59's allocator refusal; archive once committed with SHA + patch-id.

## References

- `docs/issues/2026-09-21-a-four-letter-entry-prefix-allocates-but-cannot-be-cited.md` — the
  rename that surfaced it
- `docs/trackers/issue-clusters/IC-18-selector-narrower-than-its-population.md` — the class
