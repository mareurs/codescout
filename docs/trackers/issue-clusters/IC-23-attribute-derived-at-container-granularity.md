---
id: '2e49fd615738a623'
kind: tracker
status: active
title: a per-item attribute is derived at the container's granularity, and is correct for the first item
owners:
- marius
tags:
- defect-classes
- clusters
- attribute-derived-at-container-granularity
topic: issue clusters and rule promotion
---

## IC-23 — a per-item attribute is derived at the container's granularity, and is correct for the first item

**Slug:** `cluster/attribute-derived-at-container-granularity`
**Claim:** A per-item attribute — a line, an offset, a position, an owner — is read off the **container** a producer emitted rather than off the item itself, so every item in a container is stamped with the container's value. The answer is well-formed and wrong for every item after the first, and **exactly right for the first**, which is what hides it: a fixture whose containers hold one item passes, and a fixture whose containers hold two makes the defect look like an off-by-one. The error is `(item_position - container_position)` — it grows with container size and is unbounded.
**Members:** `filter={"tags": {"contains": "cluster/attribute-derived-at-container-granularity"}}` — derived by query 2026-09-13, one member: `every-ref-in-a-fenced-block-is-attributed-to-the-blocks-first-line`. **Retagged INTO this class from `IC-6`**, where it had been filed on a diagnosis later falsified; see § *Why not IC-6, IC-18, IC-20 or IC-21* below, which records the membership test each failed rather than asserting the fit.
**Blind party:** the author, who takes the attribute from the producer's event or record **because that is where it is offered**. `pulldown_cmark`'s `Event::Text` carries one span; `span.start` is right there and the container/item distinction has no visible consequence at that layer. The reviewer is no better placed: the code reads correctly, and the only thing that would reveal it is a fixture whose containers are TALL, which nobody writes because height is not the variable under test.
**Promotes to:** `not yet` — one member, one subsystem. The admission test to apply to a candidate is *"is the attribute a property of the item, and is it being read from the thing that CONTAINS the item?"* — not *"is a line number wrong?"*, which would sweep in ordinary arithmetic bugs.
**Mechanism status:** `none yet`. The shape a mechanism would take is a **type-level** one rather than a scan: make the producer hand out `(intra_container_offset, item)` so a caller that ignores the offset fails to compile, which is what `5d6b87b4` did locally at `tokenize_code_span`. A lint cannot see this — the wrong code is a plain field read.
**Valid:** dated 2026-09-13

### Why not IC-6, IC-18, IC-20 or IC-21

Each was read against its own claim before this class was opened, per the ledger's
*"classify by the claim, never by adjacency"*. All four fail, and they fail for different
reasons, which is the argument that this is a class rather than a gap in one of theirs.

| class | its claim turns on | why this is not it |
|---|---|---|
| `IC-6` | an input made **unrepresentable**, no escape or disambiguator | nothing is unrepresentable — line numbers are an adequate address space, and the fix added neither an escape nor a disambiguator. It corrected arithmetic. |
| `IC-18` | a **selector narrower** than its population; a zero reads as "not present" | nothing was excluded from the scan. Every ref was found; each was stamped with the wrong line. |
| `IC-20` | the true value is **unknowable** because a walk stopped | the true value was knowable throughout and is now computed exactly. `IC-20`'s remedy is to rename the quantity as a floor; here the remedy was to compute it. |
| `IC-21` | an instrument reports **presence or a count** where the decision turns on magnitude | the right dimension was reported, at the wrong granularity. |

The near miss is `IC-20`, and it is worth stating why it is a miss: the drifted `md_line`
**is** a floor — the true line is always `>= ` the reported one. But `IC-20`'s defining
property is that the floor is all you can have, so the remedy is honesty about the number.
Here the exact value was always available from the same event, which makes it an
uncomputed value rather than an unknowable one, and the two take opposite remedies.

### The disguise, which is the part worth carrying to the next instance

A defect in this class **cannot be caught by the fixture anyone naturally writes.** Test a
container with one item and the answer is right. Test one with two and the answer is off by
one — so the bug gets filed as an off-by-one, a fix that adds `+ 1` passes, and the class
stays invisible. That is exactly what happened to the founding member, filed as
*"`md_line` shifts by one"* against a corpus whose two sampled containers were both two
lines tall.

So the test a candidate owes is a **tall container**, and its height is load-bearing and
must be annotated as such. This is `CLAUDE.md` § *Testing Discipline*'s "a count must
arrive with its unit" meeting its "annotate a fixture's load-bearing detail" — the reported
magnitude was a property of the sample, and only a fixture that varies the container's size
can tell the two explanations apart.
