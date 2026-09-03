---
kind: tracker
status: active
title: 'an instrument reports presence or a count where the decision turns on magnitude'
owners:
- marius
tags:
- defect-classes
- clusters
- instrument-omits-the-dimension-that-grows
topic: issue clusters and rule promotion
---

## IC-21 — an instrument reports presence or a count where the decision turns on magnitude

**Slug:** `cluster/instrument-omits-the-dimension-that-grows`
**Claim:** A surface whose purpose is to let a reader **decide** reports presence, or an item **count**, while the decision turns on **magnitude**. The reported dimension is uncorrelated with the cost, nothing errors, and the expense stays invisible until it is already large.
**Members:** `filter={"tags": {"contains": "cluster/instrument-omits-the-dimension-that-grows"}}` — `n=2`, 2026-09-01, by query. **The one pair both independent readers assigned identically**, with near-identical claims and the same remedy, so it is the least contestable of the four classes opened that day (`cluster-promotion-session-log:F-6`).
**Blind party:** the author of the instrument, who chose the dimension that was **easy to count** — rows, items, presence — at a layer where the cost had not yet accrued. A count is the natural thing to report and is right for most questions; nothing at the reporting site distinguishes the questions it is wrong for.
**Promotes to:** `not yet` — `n=2`, one short of the count bar. The two members already span two subsystems (`run_command`'s output buffer; the catalog audit trail), so a third instance meets both bars.
**Mechanism status:** `designed`, with a shipped guard rather than a sketch — **checked against the code 2026-09-03, and the previous `none yet` was wrong.** `src/librarian/tools/get.rs:122-126` carries the class as a *design constraint on a new surface*, not as a fix to an old one: a body-selected read drops the heading array but **retains** `line_count`, `total_headings` and `headings_truncated`, with the doc comment stating why — *"they report the magnitude withheld … Reporting only its absence would be the `IC-21` shape."* It is backed by an assertion, `src/librarian/tools/get.rs:1549-1555`, whose fixture is sized (25 headings) specifically to exceed the 20-heading preview cap so `total_headings` is actually stamped rather than trivially present.

That is the difference between this row and `none yet`: the class was applied *prospectively* by an author who had read it, at a site that would otherwise have shipped a bare absence signal. Both members' own fixes (`unfiltered_output_lines`; `payload_bytes` + `largest_payload_bytes`) are confirmed live in-tree — every `run_command` response carries `unfiltered_output_lines`, and `unfiltered_truncated: true` when the buffer is a prefix.

**What keeps it `designed` rather than `shipped`:** all three are per-site. Nothing enumerates the surfaces that return a count or a presence flag and asks whether the decision turns on magnitude, so the next such surface is protected only by its author having read this row. A candidate mechanism is a probe over response-shape keys — the same move `scripts/probe-cluster-census.py` makes for counts.
**Valid:** dated 2026-09-01

`unfiltered-output-ref-carries-no-size-signal`: a `@cmd_*` handle is returned with no size, line count or emptiness signal, so a caller *"cannot judge whether reading the ref is worth a round-trip"* — and nothing distinguishes "we do not know what stdout was" from "stdout was empty". `audit-growth-concentrates-in-augmentation-params-health-blind-to-bytes`: `audit::health` reports `rows` and no bytes, and the distribution is the finding — **23 of 27,914 rows (0.08%) carried 88% of the payload bytes**, so a row count does not merely under-report the cost, it reports a quantity uncorrelated with it.

Both shipped fixes are the same move, which is the strongest evidence they are one class: add the magnitude field to the reporting surface — `unfiltered_output_lines`, and `payload_bytes` + `largest_payload_bytes`. Note the second names the **largest** row rather than only the sum; where a total would read as uniform growth, the distribution is the part that makes a concentrated cost visible.

**Deliberately not claimed as a third member:** the write-amplification half of the `audit-growth` file — a whole-blob `params` rewrite captured by `json_array(OLD.params, NEW.params)`, remedied by clamping oversize values in `UPDATE` diffs. Its general shape (*a mechanism sized for diffs applied to a column that **is** the blob*) is plausible and has **no second datapoint** in this corpus, so it is flagged undecided rather than made a class of one. Both readers reached that independently.

**Falsified by** a member where magnitude *was* reported and simply ignored by its reader — that is a reading failure, not an instrument that cannot say it.
