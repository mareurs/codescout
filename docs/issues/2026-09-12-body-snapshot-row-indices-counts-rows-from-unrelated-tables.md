---
kind: bug
status: taken
tags:
- cluster/addressing-without-an-escape-hatch
claimed_by: 8bd791df-5ff4-40fe-af30-69cc3fefc2f7
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

**Direction settled 2026-09-13: the augmentation carries an explicit anchor.** Not
implemented.

1. **Bound the scan to a recorded boundary — CHOSEN.** The augmentation gains a
   `snapshot_anchor` field holding the block's header line verbatim, author-declared and
   server-validated. This is also what an auto-`index_after_line` default needs, so the
   two wants share one primitive.

2. **Require the row to match the template's column arity — MEASURED WEAK, do not
   reach for it.** Filed as "cheaper, no schema change, discriminates the observed case
   (5 columns vs 2)". Measured on this file: **83 lines** match a generic 5-column
   pattern, against **1** for the template's literal header row. Arity does not
   discriminate here; it only appeared to because the counter-example examined was
   2-column.

**Deriving the anchor was considered and is FORBIDDEN, which is the part worth keeping.**
The obvious third option — extract the template's static header and locate the block by
matching it, no schema change — was written into a plan and rejected on review against
`docs/adrs/2026-07-10-repair-and-continue-input-handling.md` § *The boundary*:

> **Asymmetry — writes get a higher bar than reads.** Auto-accepting an *explicit* write
> target is safe; auto-*guessing* a write target must still hard-error.

An omitted `index_after_line` is **absent**, not malformed, and the same ADR reserves the
teaching error for absent input. So deriving it is the guess the law forbids rather than
the deterministic repair it permits. The code says so twice independently:
`PendingIndexRow::after_line`'s doc — *"Explicit, never inferred… a wrong guess about
placement on a WRITE needs manual repair"* — and `insert_index_row`'s — *"a write accepts
an explicit target and never infers one"*.

The tell that made derivation look safe, recorded because it is the reusable half: the
justification was *"the header is unique **in this file**"*. That is a property of today's
corpus, not of the scheme — the same *"it cannot happen"* reasoning `CLAUDE.md` §
*Parsers Over a Namespace* says decays with the corpus, and the very thing this bug is
about. The DRY objection to a schema field ("a second source of truth for something
derivable") was also wrong: `render_template` defines the block's **shape**, an anchor
defines its **location**, and this bug's own Root cause section is the argument that
location is not recoverable from shape.
## Tests added

None yet. The discriminating test is a body with a snapshot table plus a second table
anchoring the same prefix, asserting the unrelated row does **not** satisfy
`snapshot_missing` — which reds under today's implementation.

## Workarounds

None needed today; no live misreading. Do not "fix" a future clean `snapshot_missing` by
trusting it on a tracker whose body holds more than one `| PREFIX-N |` table.

## Resume

**THE RULING IS DISCHARGED — and not by deciding it. The blocker was a wrong POPULATION.**

This section previously said the fix was blocked because the two candidate defaults
*"fail in opposite directions over the same 24 rows"*. Derived 2026-09-14 under a single
rule, they differ on **zero files**, and the 24 was never the population — it counted
augmented trackers, when the question only reaches a tracker that (a) passes
`body_keeps_snapshot` and (b) anchors its ids in **more than one** table. With one table
the block IS the document and both defaults agree by construction; with none, the majority
gate already returns `false` and the advisory is silent either way.

Measured over this repo's **13** params-backed ledgers (catalog is gitignored — re-derive,
do not cite):

| ledger | prefix | claimed | in body | cov | gate | tables | anchor |
|---|---|---:|---:|---:|---|---:|---|
| `prompt-hamsa-audit-log` | A | 39 | 39 | 100% | KEEPS | 1 | — |
| `windows-platform-support` | WIN | 35 | 35 | 100% | KEEPS | 1 | — |
| `2026-08-16-iron-law-gate-firing-audit` | GF | 8 | 8 | 100% | KEEPS | 1 | — |
| `open-issue-work-queue` | BL | 77 | 77 | 100% | KEEPS | 6 | **yes** |
| `provenance-subsystem` | PV | 68 | 10 | 15% | silent | 4 | — |
| 8 others | — | — | 0 | 0% | silent | 0 | — |

**4** pass the gate; of those **1** is multi-table, and it is the one that declares. So:

- **absent ⇒ scan the whole body** — a no-op on all 13 today. **Chosen.**
- **absent ⇒ no snapshot block** — would silence `prompt-hamsa-audit-log`,
  `windows-platform-support` and `2026-08-16-iron-law-gate-firing-audit`, three advisories
  that work, to fix nothing observable. Rejected on the measurement, not on taste.
- **the third option** (emit undeclared as a `doctor` finding) — would produce a worklist
  of 12 ledgers of which **9 have no rows to anchor**, 1 is 15%-coverage prose, and 3 are
  single-table where an anchor buys nothing. **12 findings, 0 actionable.** Withdrawn; it
  was offered by the same reasoning that produced the 24.

That the ruling dissolved rather than resolved is the reusable part: it had sat blocked
for a day on a fork whose two branches have no members. § *Testing Discipline*'s
*"a count of a defect population must arrive with its unit or not at all"* — the cost here
was not a wrong decision, it was a decision nobody could make because the number framing
it answered a different question.

## What shipped

- `snapshot_block_range(doc, anchor) -> Option<(usize, usize)>` — the walk, now returning
  its range. `snapshot_block_last_line` becomes a projection of it, keeping its four
  existing tests as the walk's guard.
- `snapshot_rows_in_declared_block(doc, id_prefix)` — `body_snapshot_row_indices` narrowed
  to the declared block, falling back to the whole document when the anchor is absent,
  drifted or ambiguous. The primitive is unchanged and still feeds the narrowed call, so
  there is **one** definition of what a snapshot row looks like.
- Wired at all three consumers: `augmentation.rs` `snapshot_stale_note`,
  `augmentation.rs` `append_entry`, `doctor.rs` `scan_snapshot_drift`.

**A correction to this file's own earlier citation:** `augmentation.rs:724` is **not** the
id allocator. Allocation reads `body_claimed_indices` (headings *and* rows, deliberately
wide, so a heading claiming `F-33` still blocks reissuing it); `:724` feeds `snapshot_missing`
alone. The narrowing therefore cannot affect id assignment in any direction — a smaller
blast radius than the text here previously implied.

**Adoption — the blocker this file called the real one — is discharged.**
`open-issue-work-queue.md` declares `snapshot_anchor` as of `6a0c2597`, and
`update_entry` returns `row_resynced: true` against it live. The mechanism reaches
something before this read path was wired, which is the order that section asked for.

## Resume

Mutation-verify the three guarded sites, then gate. Each test names in its own doc comment
the single production mutation it must red on:

1. `snapshot_rows_in_declared_block_cannot_reach_an_unrelated_table` — pass `doc` instead
   of the sliced `block`.
2. `..._without_an_anchor_scans_the_whole_document` — return `Default::default()` from the
   `declared_snapshot_anchor` `None` arm.
3. `..._falls_back_when_the_anchor_does_not_identify_one` — return `Default::default()`
   from the `snapshot_block_range` `None` arm.

One kill per site, not one per feature. **Not yet done at the time of writing**: the shared
lib does not compile, from another session's in-flight `audit_doc_refs` work
(`RefKind::ArtifactId` / `Verdict::ArtifactMissing` arms), so a red cannot be attributed to
an injected mutation. Holder informed; parked, not abandoned.

Then the four-command gate, chained with `;`. Read `librarian::` test names out of the
**default** lane — the lean lane compiles no librarian code, so its green says nothing here.
## References

- `docs/issues/archive/2026-08-28-body-keeps-snapshot-counts-headings-as-a-table.md` —
  the previous narrowing of this same input.
- `docs/trackers/open-issue-work-queue.md` § BL-29.
- `CLAUDE.md` § *Parsers Over a Namespace — owe an escape and a disambiguator*.
