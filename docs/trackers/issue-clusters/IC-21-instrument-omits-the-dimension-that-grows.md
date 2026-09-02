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
**Mechanism status:** none yet — **not checked against the code as of 2026-09-02**, so read this as an open question rather than an established absence.
**Valid:** dated 2026-09-01

`unfiltered-output-ref-carries-no-size-signal`: a `@cmd_*` handle is returned with no size, line count or emptiness signal, so a caller *"cannot judge whether reading the ref is worth a round-trip"* — and nothing distinguishes "we do not know what stdout was" from "stdout was empty". `audit-growth-concentrates-in-augmentation-params-health-blind-to-bytes`: `audit::health` reports `rows` and no bytes, and the distribution is the finding — **23 of 27,914 rows (0.08%) carried 88% of the payload bytes**, so a row count does not merely under-report the cost, it reports a quantity uncorrelated with it.

Both shipped fixes are the same move, which is the strongest evidence they are one class: add the magnitude field to the reporting surface — `unfiltered_output_lines`, and `payload_bytes` + `largest_payload_bytes`. Note the second names the **largest** row rather than only the sum; where a total would read as uniform growth, the distribution is the part that makes a concentrated cost visible.

**Deliberately not claimed as a third member:** the write-amplification half of the `audit-growth` file — a whole-blob `params` rewrite captured by `json_array(OLD.params, NEW.params)`, remedied by clamping oversize values in `UPDATE` diffs. Its general shape (*a mechanism sized for diffs applied to a column that **is** the blob*) is plausible and has **no second datapoint** in this corpus, so it is flagged undecided rather than made a class of one. Both readers reached that independently.

**Falsified by** a member where magnitude *was* reported and simply ignored by its reader — that is a reading failure, not an instrument that cannot say it.
