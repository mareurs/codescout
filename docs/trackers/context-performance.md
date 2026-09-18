---
id: '5d7f5c0a41d0b6f3'
kind: tracker
status: active
title: Context Performance — packing measurements and optimization points
tags:
- context
- performance
- token-budget
- progressive-disclosure
- entry-graph
entry_prefix: CTX
entry_high_water_CTX: 2
---

> **Scope:** how `librarian(action="context")` spends its token budget — what it packs, what
> it truncates, and what the corpus actually looks like underneath. Entries are `CTX-N`.
>
> **What belongs here:** a measurement of packing behaviour, a tuning constant and the data
> that chose it, or an optimization point deferred with its cost. **What does not:** feature
> work on `context` (that is a spec or a plan) and one-off bugs (those are `docs/issues/`).

## Why this tracker exists

Packing constants get chosen once, from intuition, and then outlive the corpus that
justified them. Two live examples, both found on 2026-08-21:

- the file-grain packer's **30-line** neighbour preview is right for prose artifact bodies
  and wrong for ledger entries, which run 40–200+ bytes per line;
- the entry-grain packer shipped with **no** neighbour cap at all, which was a decision
  nobody made — it was an omission, and it cost 26% of anchors a complete answer.

Neither was visible without measuring the corpus. So the rule for this file: **a tuning
constant may only be changed here alongside the measurement that moved it**, and the
measurement names its instrument and its blind spot (per `docs/PROBES.md`).

## Standing instruments

Re-runnable, and worth re-running before trusting any number below — the corpus grows.

| what | how |
|---|---|
| neighbourhood size distribution | `entry_cite` self-join on in/out degree per `<slug>:<local>` |
| section size + line count | awk over each ledger's `## PREFIX-N — ` sections, from `artifact.abs_path` |
| cap sweep | join the two, sum per anchor, count anchors fitting `max_tokens * 4` |
| neighbour taxonomy | `entry_cite` joined to `artifact.status` / `abs_path LIKE '%/archive/%'` |

**Blind spot shared by all four:** they read the catalog, so they describe whatever the last
`link_scan write=true` materialized. Re-scan before measuring, or the numbers describe a
previous world.

## CTX-1 — entry-anchor packing — the corpus that chose the 1000-byte neighbour cap

**Valid:** conditional — the fully-served share at the 1000 B cap falls below ~90%

**Re-checked 2026-09-18 — has NOT fired, and it is the closest of this corpus's conditionals
to its own line.** The standing value is the **94%** in the re-measure table below
(2026-09-12, shipped 1000 B cap) against a ~90% trigger: a four-point margin that moved
98% → 94% in three weeks while the corpus grew. Falling, not stable.

**Deliberately NOT re-measured today, and the reason is this entry's own method note.** The
sweep is a re-implementation of the two-pass packer in `src/librarian/tools/context.rs`, and
the section below records that its FIRST version disagreed with the shipped packer by one
neighbour on `R-3` — caught only by cross-checking against the real thing on two anchors
spanning both branches. A quick re-run that skipped that cross-check would produce a number
with no claim on being right, which is worse here than a number six days old. Stated so the
next reader knows which it is holding.

**Status:** applied

**Measured 2026-08-21**, project-scoped, against `entry_cite` (1513 `origin='scan'` rows) and
1598 entry sections across 88 ledgers.

### The corpus

| | |
|---|---|
| entry nodes in the graph | 944 |
| anchors with ≥1 edge | 931 |
| **mean neighbours** | **3.0** |
| neighbourhood ≤4 | **81%** of entries |
| neighbourhood ≤8 | 95% |
| neighbourhood ≥17 | **6 entries** |
| mean section | 2656 bytes |
| sections 2–4 KB | 45% |
| sections >8 KB | 2.5% |

`reconnaissance-patterns:R-3` — the entry this design was first diagnosed against — has **22
neighbours and an 80 KB neighbourhood**. It is one of the six. **Do not tune from it.** The
first version of this analysis did, and every conclusion drawn that way was wrong in the same
direction: it treated the tail as the case.

### The cap sweep (budget = default 16000 chars)

| policy | anchors fully served | mean pack |
|---|---:|---:|
| as shipped (dupes kept, neighbours whole) | 74% | — |
| mutual-dedup only | 76% | 12.8 KB |
| dedup + **1000 B** neighbour cap | **98%** | 5.9 KB |
| dedup + 1500 B | 94% | 7.2 KB |
| dedup + 2000 B | 90% | 8.3 KB |
| dedup + 3000 B | 85% | 9.9 KB |
| dedup + 4000 B | 82% | 10.9 KB |

The curve is near-linear — **there is no knee to read the constant off.** 1000 B was chosen
because the excerpt is still *useful* at that size, verified by looking at one: the largest
neighbour in R-3's set (`tracker-hygiene-log:HY-11`, 162 lines / 10.5 KB) yields its full
heading, its `Kind/Sweep/Status` line including `promoted 2026-08-20` and its commit, and the
opening of the claim. That is what a neighbour is for — *what rests on this and how widely* is
a shape question. The anchor is the thing you came to read.

### Re-measured 2026-09-12 — the corpus moved, the constant did not

**The condition fired on its first clause** and the entry is re-declared above. The two-clause
trigger is replaced by the single metric that actually governs the design: *mean* neighbourhood
was the weaker half and would **not** have tripped — it moved 3.0 → 3.69 while the served share
fell four points, so an aggregate that stayed comfortable was silent about the tail it governs.
That is this entry's own *Optimization points NOT taken* lesson holding about its own `**Valid:**`
line.

**Method validated before it was trusted, and the cross-check is why the numbers below are not
off by one.** The sweep is a re-implementation of the two-pass packer in
`src/librarian/tools/context.rs`, checked against the shipped one on two anchors spanning both
branches: `reconnaissance-patterns:R-3` (49 candidates / 12 included / 37 omitted, `excerpted`)
and `open-issue-work-queue-bl-n:BL-42` (14 / 14 / 0, `excerpted`). Both exact. The **first**
attempt predicted 13 included for `R-3` against the real 12 — a 150-byte per-neighbour overhead
estimate against a measured **258 B** (render header plus `EXCERPT_MARKER`, and whole neighbours
carry no marker so the two modes need different constants). A simulation agreeing with itself
would have shipped that.

| | 2026-08-21 | 2026-09-12 |
|---|---:|---:|
| entry sections | 1598 | 1749 |
| ledgers | 88 | 122 |
| mean section | 2656 B | 2983 B |
| sections >8 KB | 2.5% | 4% |
| `entry_cite` scan rows | 1513 | 2893 |
| anchors with ≥1 edge | 931 | 1329 |
| **mean neighbours** | **3.0** | **3.69** |
| neighbourhood ≥17 | 6 | **22** |
| max neighbourhood | 22 (`R-3`) | 48 (`R-3`) |

**Scope the scan-row count or it overstates by a factor.** Whole-catalog that row reads **5340**;
the 1513 it is compared against was project-scoped, and pairing the two reports 3.5× growth where
the truth is 1.9×. The catalog spans every indexed repo on the machine; this ledger's numbers do
not.

| policy | 2026-08-21 | 2026-09-12 |
|---|---:|---:|
| as shipped (dupes kept, neighbours whole) | 74% | — |
| whole only (no cap) | 76% | **61%** |
| dedup + 300 B | — | 98% |
| dedup + 500 B | — | 97% |
| dedup + 750 B | — | 96% |
| **dedup + 1000 B (shipped)** | **98%** | **94%** |
| dedup + 1500 B | 94% | 88% |
| dedup + 2000 B | 90% | 84% |
| dedup + 3000 B | 85% | 75% |
| dedup + 4000 B | 82% | 70% |

**The cap is KEPT at 1000 B: the four-point loss is corpus drift, not mis-tuning.** The sub-1000
rows are new — the original sweep only ran *upward*, so it could not say what restoring 98% would
cost. It costs the excerpt. 98% needs a **300 B** cap, at which a neighbour yields its heading and
essentially nothing else, failing the usefulness test this entry applied when it chose 1000 B in
the first place (heading, status line, opening of the claim). **500 B is the only alternative
worth a look** at 97%, and it is deliberately not taken here: the test for it is *reading one*,
which is a judgement, not a share this sweep can compute.

**The two-pass design is more load-bearing than when it was justified, not less.** Whole packing
fell 76% → 61%, so **39%** of anchors now need excerpting against 24% then — the *"excerpting
unconditionally would degrade three anchors in four for nothing"* figure is now closer to two in
three. The measurement that argued for two passes has strengthened while the constant it chose
has weakened, which is why re-running the sweep changed one number and no decisions.
### Why the cap is in BYTES

Copying the file-grain packer's **30-line** preview would have been the obvious move and is
wrong here. These ledgers run **40 to 200+ bytes per line** (mean 104), so a 30-line cap
leaves **1074 of 1598 sections (67%) completely untouched** while cutting the long-line-count
minority hard. Two real neighbours make it concrete:

- `HY-11` — 162 lines, 10.5 KB → 30-line preview **2.0 KB** (81% cut)
- `reconnaissance-patterns-archived-entries:R-77` — **10 lines, 2.8 KB** → 30-line preview
  **2.8 KB** (0% cut)

`R-77` is ten lines of 281-byte wrapped prose. A line cap cannot see it. **The sibling's
number was copied without checking that its unit transferred** — the same shape as
[[statement-validity-layers-1-2-session-log:W-3]].

### What shipped

Two-pass, because neither fixed policy dominates: sum the neighbourhood whole; if it fits the
budget pack it whole (76% of anchors), otherwise excerpt every neighbour at 1000 B (takes the
served share to 98%). `overflow.packing` reports which ran — a reader cannot tell a whole
section from an excerpt that happened to end at a paragraph.

Mutual pairs are deduped to one node labelled `mutual`: 182 bidirectional rows (91 pairs)
corpus-wide. Worth **+2 points** on its own, so it is a **correctness** fix (duplicate text is
simply wrong), not the budget lever an early reading of R-3 suggested.

### Optimization points NOT taken

- **Raising `DEFAULT_MAX_TOKENS`** — taxes every other `context` caller to serve one anchor.
- ~~**Ordering policy.**~~ **This deferral fired on the FIRST live call, and it is the most
  useful thing in this entry.** The reasoning was sound corpus-wide and wrong exactly where
  it mattered: ordering decides nothing at 98% served, but the anchors that *overflow* are
  by definition the ones it decides everything for — and plain alphabetical `direction`
  sorts `cited-by` < `cites` < **`mutual`**, putting the strongest class LAST.

  Verified live at `711a25cf`: `reconnaissance-patterns:R-3` served 11 one-way `cited-by`
  neighbours and dropped **both** its mutual partners, `R-77` and `R-79` — which are its own
  documented chain (`R-3 → R-113 → R-77 → R-79`). The class the packer had just been taught
  to label specially was the class it buried.

  Fixed: `mutual` ranks first, then the one-way classes alphabetically. `cites` and
  `cited-by` are **deliberately not ranked against each other** — they answer different
  questions (proof route vs blast radius) and that choice belongs to the caller.

  **The lesson is about deferrals, not ordering.** A deferral justified by an aggregate
  ("98% served") is silent about the tail, and the tail is precisely the population the
  deferred mechanism governs. Kin: [[reconnaissance-patterns:R-95]] — a deferral rationale is
  a claim, and the least-audited kind.
- **A relevance-based ordering** would come from Layer 5a's `reads`, not from structure — see
  [[CTX-2]], which refutes every structural axis tried.
- **Sweeping the budget itself.** The cap was swept at 16000 only.

## CTX-2 — neighbours cannot be classified into keep and drop — the differential-treatment hypothesis, refuted

**Valid:** conditional — a non-tracker kind reaches ~10% of neighbour edges, or a relevance signal (Layer 5a `reads`) exists

**Status:** refuted — do not re-propose without new evidence

**The question:** of all the neighbours an entry-grain anchor could pack, which *earn* their
place? If a structural axis separated useful from noise, the packer could treat classes
differently — drop some, excerpt some harder, order by class.

**Measured 2026-08-21 over all 1472 entry-target edges.** Four candidate axes:

| axis | result | verdict |
|---|---|---|
| owning artifact **kind** | 98.0% `tracker`, 1.8% `memory`, 0.3% `spec` | **dead** — no variance to classify on |
| **locality** | 52% cross-ledger / 48% intra-ledger | splits, but does not distinguish |
| owning artifact **status** | 74.7% active, 21.3% archived, 3.8% draft | looked promising — **see the trap** |
| **archive path** | 22.5% under `**/archive/**` | same trap |

### The archived-means-stale trap

Archived-ness is the axis that most *looks* like a relevance signal, and on this corpus it is
inverted. Of the archived-neighbour edges: **207 same-ledger, 40 the anchor's own archive
companion, 84 unrelated.**

`reconnaissance-patterns:R-3` is the clean case. Seven of its 22 neighbours live in
`reconnaissance-patterns-archived-entries`, and two of those — `R-77` and `R-79` — are its
**mutual** partners and its own documented chain (`R-3 → R-113 → R-77 → R-79`). Its most
relevant neighbours are archived, because this project's compaction discipline *moves related
history there on purpose*. `get_guide("tracker-conventions")` states it directly: *"archived
is not nonexistent."*

### The taxonomy that does emerge

Structural, and useful as a **label**, not as a filter:

| class | edges | pct |
|---|---:|---:|
| **A** intra-ledger (same file) | 603 | 41.0% |
| **D** cross-ledger, live | 537 | 36.5% |
| **C** mutual (reciprocal tie) | 182 | 12.4% |
| **B** lineage (anchor's own archive companion) | 78 | 5.3% |
| **E** cross-ledger, archived | 72 | 4.9% |

**The only class that reads as plausibly droppable is E, at 4.9%.** Suppressing it saves
nothing and risks the 84 unrelated-archived edges that are genuine cross-work-stream history.

### Ruling

**No structural axis separates relevant from noise.** These classes describe *where a
neighbour lives*, not *whether it matters*. Two specific speculations are refuted, including
one of mine:

- *intra-ledger deserves less room because it is cheap to fetch* — locality is a
  **retrieval-cost** signal, not a relevance one, and R-3's intra-ledger neighbours
  (`R-41`, `R-93`, `R-96`) are among its most substantive.
- *archived neighbours are history* — inverted here, per the trap above.

The classes are worth **emitting as a label** (free to compute, tells a reader why a neighbour
is present and whether fetching it is cheap). They are not worth filtering on.

**Re-open when** a real relevance signal exists — Layer 5a's `reads` measures what agents
actually consult, which is the property every axis above was standing in for. Ordering by
class is only defensible once class is known to correlate with something a reader wanted.

## Template for new entries
