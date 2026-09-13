---
kind: bug
status: investigating
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

Blocks BL-29's third option (`docs/trackers/open-issue-work-queue.md`): defaulting
`index_after_line` to the last snapshot row requires knowing which rows are snapshot
rows, which is exactly what this defect denies.

**BLOCKED ON ONE RULING before implementation — `bug-fix-session-log:F-136` (2026-09-13).** The
fix direction above is settled on the CHOICE and silent on the DEFAULT: it never says what an
**absent** `snapshot_anchor` means, and on day one that is every augmented tracker — **24** of them
(`doc(action="find", kind="tracker", augmented=true)`, tree `2e8a8361`; the catalog is gitignored, so
re-derive rather than cite). The two available defaults fail in opposite directions over the same 24
rows:

- **absent ⇒ scan the whole body** (today's behaviour) preserves the exact false negative this bug
  was filed to remove, for every tracker, until someone hand-declares an anchor — the fix ships
  without fixing anything observable;
- **absent ⇒ no snapshot block** makes `body_snapshot_row_indices` empty, so `body_keeps_snapshot`
  early-returns `false` and `snapshot_drift` goes silent across all 24 at once — the
  `tool-usage-patterns` false-positive direction, inverted and corpus-wide.

Neither is visible at unit-test grain: fixtures get written to whichever assumption the implementer
holds, and the suite then confirms it (§ *Testing Discipline*'s population-vs-member law).

**A third option the list above does not carry**, offered for the ruling rather than assumed:
*absent ⇒ scan the whole body **and** emit the undeclared state as a `doctor` finding*, so the 24
become a worklist that drains instead of a silent default. Backward-compatible, makes the gap
countable, and respects the ADR asymmetry this bug already cites — a **read** may fall back, a
**write** may not.

**CORRECTED 2026-09-13 — the shape below was wrong, and the primitive is ALREADY BUILT.**
`snapshot_anchor` shipped at `59e8c970` (12:29 that day) as a **frontmatter `extra` key**, read by
`declared_snapshot_anchor()` (`src/librarian/catalog/augmentation.rs:1110`) — deliberately not a
typed field and not a DB column, *"so a caller declares this with the same `doc(action="update",
patch={extra: …}`) surface that already exists."* There is **no migration to write**: the
`ALTER TABLE` / 14th-field / positional-`row_from_sql` plan this section previously carried answered
a question nobody had. It is struck rather than deleted because it is exactly the wrong turn the
next reader would otherwise take.

~~`column_exists`-guarded `ALTER TABLE artifact_augmentation ADD COLUMN snapshot_anchor TEXT`; a
14th field on `AugmentationRow`; `row_from_sql` is positional, so `row.get(13)` and every `SELECT`
must move in step.~~

**What is actually left**, verified at the bytes: `body_snapshot_row_indices` still takes
`(body, id_prefix)` and still scans the whole document with `(?m)^\|` — unchanged at `1864-1877`.
The defect is live. Threading the existing anchor into that **read** path, plus its three consumers
(`augmentation.rs:530` `snapshot_stale_note`, `augmentation.rs:724` `append_entry`,
`doctor.rs:4350` `scan_snapshot_drift`), is the whole remaining job — and `resync_snapshot_row`
(`:1534`) is the worked example of how to read it and what absent means: `Ok(false)`, a silent
no-op, falling back to the advisory.

**THE REAL BLOCKER IS NOT THE RULING, IT IS ADOPTION.** `grep -rln '^snapshot_anchor:'
docs/trackers/` returns **nothing** — no tracker in this repo declares one. Both halves of `BL-29`'s
remedy shipped 2026-09-13 (`dd5c58b5`, `59e8c970`) are inert corpus-wide, including on
`open-issue-work-queue.md`, the tracker `BL-29` was filed about. Wiring this read path without
arming any anchor produces a mechanism that is correct and reaches nothing — CLAUDE.md § *Testing
Discipline*'s *"loudness is a property of a PATH"*, in advance rather than in hindsight. Decide
adoption and wiring together. (Corpus grep by sessionId `8bd791df-5ff4-40fe-af30-69cc3fefc2f7`.)

**Claim released deliberately:** worked 2026-09-13 by sessionId
`9403d62d-116b-46ea-ac9b-004acff2b1cb` and set back to `investigating` rather than left `taken`,
because no live session holds it — the ruling is the blocker, not the typing.

## References

- `docs/issues/archive/2026-08-28-body-keeps-snapshot-counts-headings-as-a-table.md` —
  the previous narrowing of this same input.
- `docs/trackers/open-issue-work-queue.md` § BL-29.
- `CLAUDE.md` § *Parsers Over a Namespace — owe an escape and a disambiguator*.
