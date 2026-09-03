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
**Mechanism status:** `partial` — **checked against the code 2026-09-03; `none yet` understated it in two different ways.**

**Per-member, shipped and gated.** The seed's rename is live at `src/tools/grep.rs:486` and `:1067` (`overflow["total_is_lower_bound"] = json!(true)`), and it is regression-tested **in both directions**, which matters for this class specifically: `src/tools/grep.rs:1935` asserts the flag is *absent* when nothing was cut (*"nothing was cut off, so the total is exact and must not be flagged a floor"*) and `:1827`/`:1966` assert it is *present* when it was. A one-directional test here would be monotone under the failure that matters — stamping every total as a floor satisfies a presence-only assertion and destroys the signal.

**Class-level, and in flight rather than absent.** Branch `result-cap-marker-gate` (unmerged as of 2026-09-03, `2a32c043`) builds the general instrument: `src/tools/core/cap_probe.rs` (992 lines) plus `tests/result_caps.rs` (3001 lines), annotating cap sites across ~50 files. Its own commit log shows it has already been through several falsification rounds (`9d240f4d` — *"make the marker check discriminate on the assertion condition, not the message"*; `e777324b` — *"mutate all 18 probed rows — 17 killed, and the 18th is the finding"*).

**The open question that branch has surfaced is this row's own discriminator**, and it is worth watching rather than assuming settled: `docs/issues/2026-09-03-result-cap-conflates-truncating-ceilings-with-suppressing-floors.md` reports the probe currently folding **truncating ceilings** and **suppressing floors** into one class. That is precisely the `IC-19`/`IC-20` split this row exists to hold apart — a ceiling wants a *selection* fix, a floor wants a *rename*. If the merged gate keeps them merged, it will report this class as covered while remedying it in the wrong direction.
**Valid:** dated 2026-09-01

`grep-showing-n-of-n-when-collection-hit-cap` is the seed and states the unknowability directly: *"The real total is not merely unreported — after `hit_cap` it is **unknown**, because the walk stopped."* Its own § Hypotheses records "report a truthful denominator instead" as **rejected — not available**, which is the whole of why this is not `IC-13`: that class's remedy is to make the marker arrive, and here there is no true value for a marker to carry. The shipped fix renames the quantity (`4 matches (capped)`, `total_is_lower_bound`).

**The entanglement with `IC-19` is real and is the argument that nearly merged them.** A second reader grouped this file with `IC-19`'s two on corpus evidence: `grep-narrowing-hint-ranks-by-capped-display-count`'s fix *reproduced this defect one level down* and had to add a floor marker of its own — *"Without this the fix would have replaced one piece of false precision with another one level down."* That is a genuine observation and it is recorded here rather than discarded: **the two classes co-occur because fixing a wrong ordering hands you a wrong denominator.** They stay apart on the remedy test, which is this ledger's stated discriminator, and `cluster-promotion-session-log:F-6` holds both sides.

**Falsified by** a member whose true total was recoverable — that is an ordinary reporting bug, fixed by reporting it, and this class claims the value is gone.
