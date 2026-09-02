---
kind: tracker
status: active
title: 'a floor is published under the name of a total, and the true value is unknowable rather than unreported'
owners:
- marius
tags:
- defect-classes
- clusters
- floor-published-under-the-name-of-a-total
topic: issue clusters and rule promotion
---

## IC-20 — a floor is published under the name of a total, and the true value is unknowable rather than unreported

**Slug:** `cluster/floor-published-under-the-name-of-a-total`
**Claim:** A statistic computed over the subset a walk actually collected is published under the name of the whole population. Because the walk **stopped**, the true value is not merely unreported but **unknowable**, so neither a correction nor a marker repairs it — the remedy is to rename the quantity as a floor, or to refuse to print it.
**Members:** `filter={"tags": {"contains": "cluster/floor-published-under-the-name-of-a-total"}}` — `n=1`, 2026-09-01, by query.
**Blind party:** the caller, who receives a number *with a denominator* and has no way to learn the denominator describes the window rather than the world. A bare count invites the question "of how many?"; a ratio answers it, wrongly, and closes the inquiry.
**Promotes to:** `not yet` — `n=1`, below the count bar. Kept rather than folded into `IC-19` because the remedies differ: `IC-19`'s is a **selection** (derive against the pre-cap population), this one's is a **rename** (publish the number as a floor), and the second is what you are left with precisely when the first is impossible.
**Mechanism status:** none yet — **not checked against the code as of 2026-09-02**, so read this as an open question rather than an established absence.
**Valid:** dated 2026-09-01

`grep-showing-n-of-n-when-collection-hit-cap` is the seed and states the unknowability directly: *"The real total is not merely unreported — after `hit_cap` it is **unknown**, because the walk stopped."* Its own § Hypotheses records "report a truthful denominator instead" as **rejected — not available**, which is the whole of why this is not `IC-13`: that class's remedy is to make the marker arrive, and here there is no true value for a marker to carry. The shipped fix renames the quantity (`4 matches (capped)`, `total_is_lower_bound`).

**The entanglement with `IC-19` is real and is the argument that nearly merged them.** A second reader grouped this file with `IC-19`'s two on corpus evidence: `grep-narrowing-hint-ranks-by-capped-display-count`'s fix *reproduced this defect one level down* and had to add a floor marker of its own — *"Without this the fix would have replaced one piece of false precision with another one level down."* That is a genuine observation and it is recorded here rather than discarded: **the two classes co-occur because fixing a wrong ordering hands you a wrong denominator.** They stay apart on the remedy test, which is this ledger's stated discriminator, and `cluster-promotion-session-log:F-6` holds both sides.

**Falsified by** a member whose true total was recoverable — that is an ordinary reporting bug, fixed by reporting it, and this class claims the value is gone.
