---
kind: bug
status: open
tags:
- cluster/addressing-without-an-escape-hatch
closed: null
opened: 2026-09-12
owner: marius
related: []
severity: medium
---

# BUG: `body_snapshot_row_indices` counts `| PREFIX-N |` rows from unrelated tables as snapshot rows

## Summary

`body_snapshot_row_indices` runs its regex over the **entire artifact body**, so any
line matching `^\|\s*PREFIX-N` counts as a rendered-snapshot row — including rows in
prose tables that have nothing to do with the snapshot. The function has no notion of
*which* table a row belongs to, and the augmentation records no snapshot-block
boundary for it to consult.

This is the 2026-08-28 narrowing (`body_claimed_indices` → `body_snapshot_row_indices`,
which excluded **headings** from the coverage set) holding one level over: the input was
narrowed from "headings and rows" to "rows", and never to "rows *of the snapshot table*".

## Symptom (Effect)

No false reading on this machine **today** — coverage is 100% either way on the one
tracker where the extra rows occur, so nothing is currently misreported. The reachable
harm is a **false negative**, which is the half that looks like health:

`snapshot_missing` is `claimed.difference(in_body)`, computed over the union of every
matching row in the body. Rows that live outside the snapshot table therefore **fill
holes the snapshot table leaves**. If the snapshot block lost rows for ids that also
appear in an unrelated table, the lag reports as clean.

That is exactly the `prompt-hamsa-audit-log` failure the 2026-08-28 change was written
to eliminate — *"the gate passed, and then `claimed.difference(in_body)` came out empty
because the headings filled the holes the rows left. A real lag reported as health"* —
reachable again through a different door.

## Reproduction

Tree `408709ea`, `docs/trackers/open-issue-work-queue.md` (artifact `9a892c2a5976e296`,
`id_prefix: BL`):

```
grep -cE '^\|[ \t]*BL-[0-9]+[ \t]*\|' docs/trackers/open-issue-work-queue.md   # 87
grep -oE '^\|[ \t]*BL-[0-9]+' docs/trackers/open-issue-work-queue.md \
  | grep -oE 'BL-[0-9]+' | sort -u | wc -l                                     # 76
```

**87 row-anchored lines, 76 distinct ids.** The snapshot block is lines 43–120 and holds
all 76. The other **11 lines** sit at lines 2020–2279, inside two-column prose tables
(`|---|---|`) in retrospective sections — handoff-status rows, not snapshot rows:

| id | snapshot row | unrelated row |
|---|---|---|
| BL-1 | 45 | 2197 |
| BL-2 | 46 | 2229 |
| BL-20 | 66 | 2198 |
| BL-21 | 67 | 2199 |
| BL-22 | 68 | 2200 |
| BL-26 | 72 | 2201 |
| BL-27 | 73 | 2202 |
| BL-31 | 77 | 2230 |
| BL-33 | 79 | 2279 |
| BL-42 | 84 | 2020 |
| BL-43 | 86 | 2021 |

Because the result is a `BTreeSet`, the duplicates collapse and coverage reads 76/76.
Delete any of those 11 rows from the snapshot block and `snapshot_missing` stays empty.

## Environment

- Tree `408709ea` on `experiments`; catalog is machine-local.
- `src/librarian/catalog/augmentation.rs` — `body_snapshot_row_indices` (regex over the
  whole body), `body_keeps_snapshot` (majority threshold over its output).

## Root cause

```rust
let re = regex::Regex::new(&format!(r"(?m)^\|[ \t]*[`*\[]*{esc}-(\d+)\b"))
re.captures_iter(body)          // <- the WHOLE body
```

`(?m)^\|` anchors to the start of any line in the document. Markdown gives a table no
name and no delimiter, so two tables sharing one row grammar are **mutually
indistinguishable** to a line-anchored regex. Nothing in the augmentation names the
snapshot block either — `render_template` produces the block but records no anchor for
finding it again.

This is `IC-6`'s *no disambiguator* half: two constructs share one addressing scheme and
neither can be named. The class predicts it — *"two byte-identical headings, both
permanently unaddressable"* — and a table is the same shape.

## Evidence

- `body_snapshot_row_indices`, `src/librarian/catalog/augmentation.rs:1481-1494`.
- `body_keeps_snapshot`, same file, `:1652-1660` — its doc comment already records the
  identical defect one narrowing earlier, including the false-negative direction.
- Counts above, re-derivable from the two `grep` commands.

## Hypotheses tried

- **"The 11 extras are duplicate snapshot rows"** — **rejected.** Read at lines
  2018–2022: they are rows of two-column tables inside prose retrospectives, different
  column count and different meaning.
- **"Coverage is wrong today"** — **rejected.** 76 of 76 either way; the defect is
  latent, not active. Recorded so nobody credits a future clean reading to health.

## Fix

Not implemented. Two directions, neither started:

1. **Bound the scan to the snapshot block.** Needs a recorded boundary — the
   augmentation would carry the block's anchor (header line, or a fenced marker the
   template emits). This is also what an auto-`index_after_line` default needs, so the
   two wants share one primitive.
2. **Require the row to match the template's column arity.** Cheaper, no schema change,
   and discriminates the observed case (5 columns vs 2). Weaker: two 5-column tables in
   one file would defeat it.

Direction 1 is the one that generalises; direction 2 is what a gate could ship today.

## Tests added

None yet. The discriminating test is a body with a snapshot table plus a second table
anchoring the same prefix, asserting the unrelated row does **not** satisfy
`snapshot_missing` — which reds under today's implementation.

## Workarounds

None needed today; no live misreading. Do not "fix" a future clean `snapshot_missing` by
trusting it on a tracker whose body holds more than one `| PREFIX-N |` table.

## Resume

Blocks BL-29's third option (`docs/trackers/open-issue-work-queue.md`): defaulting
`index_after_line` to the last snapshot row requires knowing which rows are snapshot
rows, which is exactly what this defect denies.

## References

- `docs/issues/archive/2026-08-28-body-keeps-snapshot-counts-headings-as-a-table.md` —
  the previous narrowing of this same input.
- `docs/trackers/open-issue-work-queue.md` § BL-29.
- `CLAUDE.md` § *Parsers Over a Namespace — owe an escape and a disambiguator*.
