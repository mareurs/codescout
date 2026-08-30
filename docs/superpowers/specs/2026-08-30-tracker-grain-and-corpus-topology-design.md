---
id: '8ba9230c8d25b35c'
kind: spec
status: draft
title: Tracker grain and corpus topology — declare what a section serves; classify the corpus to find hubs
tags:
- librarian
- context
- knowledge-graph
- grain
- token-budget
- trackers
topic: librarian
---

## Summary

Two automated processes over the tracker corpus, sharing one substrate:

- **Layer A — topology.** Classify trackers and their entries from the citation graph the
  librarian already derives, so *hubs* become visible and the graph becomes a usable
  knowledge graph rather than an implicit side effect of `link_scan`.
- **Layer B — grain.** When an artifact is too large to deliver whole, deliver the
  **section that serves the call** instead of the whole body or a blind prefix.

Layer B is not a new mechanism. It is the mechanism `get_guide` already ships, pointed at a
second corpus.

## Why now — the measured problem

Measured on this repo, 2026-08-30, over `docs/trackers/*.md`:

| Quantity | Value |
|---|---:|
| Trackers | 59 |
| Total size | 3,545,004 B (3.5 MB) |
| Exceeding the **entire** 16,000-char default context budget | **36 of 59** |
| Exceeding half the budget | 52 of 59 |

The tail is where this bites:

| Tracker | Size | `## ` sections | ≈ B/section |
|---|---:|---:|---:|
| `bug-fix-session-log.md` | 635,115 | 175 | 3,629 |
| `reconnaissance-patterns.md` | 397,126 | 88 | 4,512 |
| `prompt-hamsa-audit-log.md` | 256,170 | 40 | 6,404 |
| `prompt-surface-measurement-session-log.md` | 244,936 | 78 | 3,140 |
| `release-promotion-session-log.md` | 237,211 | 62 | 3,825 |

**2.5%** of `bug-fix-session-log.md` fits the whole context budget. The natural unit already
exists and already fits: a `## <ID> — <title>` entry averages ~3.6 KB, so four of them fit a
budget the file misses by 40×.

Two distinct failure modes follow, and they need naming separately because a fix for one is
not a fix for the other:

1. **Whole-artifact reads.** `artifact(action="get", full=true)` on a large tracker returns
   the entire body. The caller wanted one entry.
2. **Neighbour packing.** Per `CTX-1`, when the neighbourhood overflows, every neighbour is
   excerpted at **1000 B**. That is not over-ingestion — it is a *blind* 1000 B, cut without
   regard to where the entry's substance sits, and `overflow.packing` exists precisely
   because a reader cannot otherwise tell a whole section from an excerpt that happened to
   end at a paragraph.

## What already exists — do not rebuild it

Verified live this session, not read from documentation:

- **`get_guide` section grain is shipped and running.** Sections carry
  `<!-- serves: <tool>.<action> -->` and `<!-- requires: <other section> -->`; a
  `selector_key` sees the call; only matching sections are delivered, each wrapped in an
  auto-inject marker naming the topic and section, with a documented fallback to the whole
  topic. Observed this session on `librarian.reindex`, `artifact.get`, `artifact.create`,
  `artifact.update` and `artifact.append_entry`.
  Design: `docs/superpowers/specs/2026-08-27-get-guide-section-grain-design.md`;
  Phase 1 plan: `docs/superpowers/plans/2026-08-27-get-guide-section-grain.md` — Phase 1
  **shipped**; Phases 2 and 3 remain, tracked as `GG-N` in
  `docs/trackers/resume-get-guide-section-grain-phases-2-3.md`.
- **`link_scan`** derives and prunes `rel="cites"` edges from prose citations, resolving
  entry tokens by their defining heading and reporting ambiguous ones rather than guessing.
- **`artifact(action="graph", depth=…)`** traverses the edges; `librarian(action="context")`
  packs a neighbourhood; `doctor` reports ledger health; `legibility_scan` is the precedent
  for *a scan that writes its own ranked backlog tracker*.

So Layer B inherits a proven declaration format, a proven selector, and a proven fallback.
Layer A inherits an already-materialised edge set and an already-established
"scan writes a backlog tracker" pattern.

## Relationship to the GG-N queue — one graph, not two

The guide corpus has already reached this design's Layer A conclusion from the other side,
and that queue is live work rather than history. Reconcile with it before building:

- **`GG-7` — *Guide topics are atomic nodes in an unmodelled graph*** (open, filed as
  `docs/issues/2026-08-27-guide-topics-are-atomic-nodes-in-an-unmodelled-graph.md`). Its
  finding: `requires:` **exists but is only used within a topic — cross-topic `requires:` is
  not modelled**, and three guides already cite sections the API cannot serve. That is the
  same missing edge type Layer A proposes for trackers, one corpus over.
- **`GG-1`** — `tracker-conventions` cannot declare `serves:` until two of its sections
  decompose. Directly relevant: `tracker-conventions` is the guide that *governs* the corpus
  Layer B would split, so its own decomposition is a dependency and a worked example.
- **`GG-2`** — eight topics still undeclared, four never auto-injected. The backfill problem
  named in *Open questions* below is already measured on the guide corpus; use those numbers
  rather than re-deriving them.

**The consequence for this spec: Layer A must not fork a second graph model.** `serves:` and
`requires:` are the declared edge types, `cites` is the derived one, and a tracker-only
topology built beside the guide topology would give the repo two knowledge graphs whose
nodes overlap and whose metrics disagree. Whatever Layer A builds should be the thing that
closes `GG-7`, generalised — not a sibling of it.

## The refutation this design must respect

`CTX-2` — measured over **all 1,472** entry-target edges — refutes classifying neighbours
into keep/drop on structural axes:

| axis | result | verdict |
|---|---|---|
| owning artifact `kind` | 98.0% `tracker` | dead — no variance |
| locality | 52% cross-ledger / 48% intra | splits, does not distinguish |
| `status` | 74.7% active, 21.3% archived | the archived-means-stale trap |
| archive path | 22.5% under `**/archive/**` | same trap |

Its ruling is unambiguous: *"No structural axis separates relevant from noise. These classes
describe where a neighbour lives, not whether it matters."* Its `Valid:` is
`conditional — a non-tracker kind reaches ~10% of neighbour edges, or a relevance signal
(Layer 5a reads) exists`. Its status is `refuted — do not re-propose without new evidence`.

**This design does not re-propose it.** The distinction is load-bearing and must survive
review, so state it plainly:

- `CTX-2`'s axis is **which neighbours to pack**, with relevance **inferred** from structure.
  Refuted.
- This design's axis is **how much of one artifact to deliver**, with relevance **declared**
  by the section itself.

`get_guide` is the existence proof that the declared route works where the inferred route
failed — same corpus shape, same budget pressure, opposite outcome.

Where Layer A does touch `CTX-2`'s territory, it obeys `CTX-2`'s own concession: the classes
are **emitted as labels, never used as filters**.

## Layer A — corpus topology and hubs

**Input:** the `cites` edges `link_scan` already materialises, plus the manual rels
(`evidence-for`, `promoted-to`, `refutes`, `supersedes`).

**Output:** a generated tracker, in the `legibility-backlog` mould — ranked, regenerable,
never hand-maintained.

**The one constraint the measurement imposes.** `CTX-2` found **41.0%** of edges are
intra-ledger (class A, same file). A hub metric that counts raw in-degree therefore ranks
ledgers by *size*, not by *centrality* — `bug-fix-session-log.md` has 175 sections citing
each other and would top any such list by construction. That is a self-validating gate in the
sense the reconnaissance skill warns about: the metric contains the quantity it is judging.

So a hub is defined on **distinct citing ledgers**, not on edge count:

```
hub_score(artifact) = |{ distinct source ledgers citing it }|
```

with intra-ledger edges excluded from the numerator and reported separately. `CTX-2`'s
five-class taxonomy (A intra-ledger, B lineage, C mutual, D cross-ledger live,
E cross-ledger archived) is carried through as a per-edge label so a reader can see *why* a
node scored.

**What the KG adds over the raw edge table:** nodes at two grains (artifact and entry),
centrality that discounts self-citation, and the manual rels as typed edges. All of it is
already in the catalog; what is missing is the view and the metric.

## Layer B — section grain for trackers

**Trigger.** Artifact body exceeds a threshold. Two candidates, both measured above:
16,000 B (the context budget) selects 36 of 59; 8,000 B selects 52 of 59. Start at the
budget — an artifact that *fits* has no grain problem, and enlarging the population before
the mechanism is proven only enlarges the backfill.

**Unit.** The `## <ID> — <title>` heading. This is not a new convention: it is already the
only shape `link_scan` accepts as an entry definition, so every entry is already an
addressable, citable node. The grain and the citation unit are the same unit, which is what
makes the KG and the delivery mechanism share a substrate.

**Declaration.** Extend `serves:` to tracker sections. The guide corpus keys on
`<tool>.<action>`; a tracker needs a richer key, and the user's framing — *"reading/writing
boundaries, but not only"* — is the right starting axis, because `get_guide` already
demonstrates it: `## Artifact Model` declares `serves: artifact.get, artifact.create`, which
is a read/write split at tool-action grain.

Proposed selector dimensions, to be narrowed by measurement, not by argument:

1. **Operation** — `<tool>.<action>`, reusing the guide vocabulary verbatim.
2. **Lifecycle phase** — scout / implement / review / archive. A section recording *what we
   measured* serves a decision; one recording *how to append* serves a write.
3. **Status** — an entry whose `Status:` is `refuted` or `wontfix-false-alarm` serves a
   "don't re-propose this" query and almost nothing else. `CTX-2` is itself the worked
   example: it is exactly the section this very spec needed, and nothing else in its
   tracker was relevant.

**Fallback — the part that must not be skipped.** `get_guide` falls back to the whole topic.
A 635 KB tracker cannot. So the fallback is: the index/summary section, plus the N
highest-`hub_score` entries — which is where Layer A pays for Layer B. Never a blind prefix;
that is the current behaviour and it is the thing being replaced.

**What this fixes in `CTX-1`'s path:** pack whole *sections* that fit, rather than truncating
every neighbour at 1000 B mid-entry.

## Open questions — measure before building

These are the reasons this is a spec and not a plan.

1. **Backfill cost is the real cost.** 175 sections in one tracker, 36 trackers over
   threshold. Who writes `serves:` on each? An inference pass that drafts declarations for
   human review is *not* barred by `CTX-2` — that refutes inferring relevance at
   **delivery** time, not proposing a declaration at **authoring** time — but the distinction
   must be explicit in the plan or a reviewer will read it as the refuted thing.
2. **Is declaration actually better than recency?** The honest base arm is "deliver the N
   most-recently-updated entries", which costs nothing and needs no backfill. Ship nothing
   until declaration beats it on a measured task set. The reconnaissance skill's promotion
   rule demands a base arm before anything reaches a session-opening surface; the same bar
   applies here.
3. **Layer 5a `reads` is the common dependency, and it is designed but not shipped**
   (`docs/superpowers/specs/2026-08-20-entry-validity-and-attestation-design.md`;
   `docs/trackers/resume-statement-validity-layers-3-5.md` still open). It measures what
   agents actually consult. It is `CTX-2`'s named re-open condition **and** the only
   non-circular way to validate Layer B. Building it unlocks both — which is an argument for
   sequencing it first, ahead of either layer here.

## Gates

- A hub metric must be shown NOT to rank by artifact size: correlate `hub_score` against
  byte size and require the correlation to be weak. A metric that reproduces the size
  ordering has measured nothing.
- Section delivery must have a deliberate-break test: a section whose `serves:` no longer
  matches must stop being delivered, and a test must fail when it is wrongly delivered.
  Per the SDD ruling log, an assertion added to close a missing-guard finding that cannot
  itself fire is the recurring defect here.
- The fallback path needs its own test. It is the path that runs when declarations are
  absent, which is every artifact on day one.

## Not in scope

- Re-opening `CTX-2`. Neighbour selection stays as shipped.
- Changing `link_scan`'s citation grammar or the `## <ID> — <title>` definition rule.
- Splitting tracker **files** on disk. This is a delivery-grain change; compaction into
  archive companions is an existing, separate discipline with its own hazards (the
  `entry_high_water_<PREFIX>` counter must survive, per `get_guide("tracker-conventions")`).
