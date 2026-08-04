---
id: e12cd7e0060ed9b8
kind: tracker
status: active
title: Provenance Subsystem — Programme Tracker (PV-N)
tags:
- provenance
- attribution
- measurement
- programme
- design-log
topic: provenance-attribution
---

# Provenance Subsystem — Programme Tracker (PV-N)

## STATE AS OF 2026-08-04 — read this first

**The measurement programme is CLOSED.** Thirteen rounds, 2,997 transcripts plus
64 Langfuse-reconstructed sessions. Do not re-open a metric without reading its
PV entry first — several were measured, reversed, and re-measured, and the entry
holds the final reading.

**Verdicts, final:**

| | outcome |
|---|---|
| Tier 4 (clustering) | **KILLED** — M2's pre-registered condition fired cleanly (PV-3) |
| Tier 1 (the gate) | **CLOSED AS A CLASS** — measured as opportunity, not signal (PV-26) |
| Confabulation detection | **DESCOPED** — addressable floor is 0.20% of references (PV-12) |
| Stale-drift (M6) | **weak NO**, repo-gated and self-administered (PV-24) |
| Retrieval granularity | **REFUTED** — was a domain mismatch, not an effect (PV-7, PV-57) |

**The one thing left to build:** PV-29. A PostToolUse hook matching `mcp__.*`
writes oversized payloads to codescout's existing `@tool_*` buffer and returns a
summary + handle via `updatedToolOutput`. Trigger on per-call size (≥ 32 KB = 137
calls carrying 61.1% of information-bearing tokens). **Buffer, never truncate** —
the rationale is load-bearing, see PV-52/PV-54. Constraint: PV-59 forbids capping
repo-source reads, which are the best-utilised category in the corpus.

**If you are tempted to re-run something,** check these first — each was a
conclusion that got reversed, and the reversal is the current state:
PV-7 (refuted), PV-26 (reversed), PV-42 (reversed), PV-36 (corrected),
PV-28 (settled), PV-32 (amended).

**Artifacts.** The probe pipeline lives at `~/.local/share/provenance-probe` —
**outside this repo, deliberately** (R-52). It holds ~19 MB of symbol
vocabularies extracted from eight repositories including client work; it must
never be moved back inside any input repo, and `/scratch/` is gitignored as
defence-in-depth. Every figure cited in this ledger is inline in its PV entry, so
the trackers are self-contained — the pipeline is for rebuilding, not for reading
the results.

**Sibling trackers.** `docs/trackers/provenance-probe-session-log.md` (F-1..12 /
W-1..8) for per-session frictions and wins; `docs/trackers/reconnaissance-patterns.md`
R-51 and R-52 for the instrument/corpus lessons; `docs/trackers/codescout-usage-frictions.md`
U-29 for the payload-discarding-guard pattern.

**Cross-session context.** The design counterpart lives in a claude.ai project
("AI thoughts") that cannot read this repo. The paste-able export is
`HANDOFF.md` in the artifact directory; regenerate it from this ledger rather
than the reverse — this tracker is the source of record.

> **Phase: MEASUREMENT. Nothing ships.** Explicit decision: measure before
> building, and do not design for other users yet. This does not reach anyone
> else until we know what is worth building.
>
> **Scope of this tracker.** Standing design decisions, hazards that must not be
> rediscovered, measurement verdicts against pre-registered kill conditions, open
> decisions, and the backlog of future work. The PV-N table (live, in params) is
> the index; the sections below are the reasoning that must survive between
> sessions.
>
> **Sibling artifacts.** Per-session frictions/wins from the measurement run live
> in `docs/trackers/provenance-probe-session-log.md` (F-N / W-N). The
> cross-cutting recon lesson is R-51 in `docs/trackers/reconnaissance-patterns.md`.
> Probe deliverables are `scratch/provenance-probe/RESULTS.md` + `results.json`.

---

## Where this came from

The thread starts with **Jaron Lanier's** argument — 2023 New Yorker essay
*"There Is No A.I."*, extended in his Oct 2025 TU Wien lecture *"Who Owns the
Future?"*. His proposal: alongside each model output, show the clusters of
training data that were **counterfactually relevant** — sources whose absence
would have changed the result. He calls the general programme **data dignity**.

In the pretraining setting this is close to intractable. The insight that
started this work is that **it becomes tractable one level up the stack.**

## The core reframe — read this before proposing anything

The proximate cause of an agentic coding output is **not the training corpus**.
It is a **context window whose contents were logged**: files read, symbols
resolved, specs injected, tool results observed, user instructions. All
observable. Attribution here is mostly **bookkeeping plus modest inference** — no
influence functions required.

Two consequences that shape every design decision downstream:

1. **Code is the easy case.** Identifiers are near-unique keys. If a patch calls
   `SessionStore::rotate` and a symbol lookup returned that forty turns earlier,
   the causal link is near-certain from string matching alone. Prose attribution
   has no equivalent. This is why the idea is more achievable for coding agents
   than for Lanier's general case.

2. **This is NOT data dignity.** Pretraining residue — idiomatic error handling,
   library conventions, the shape of a good test — is invisible to session-level
   attribution and always will be. What survives the translation is
   **provenance, auditability, and confabulation detection**. **Compensation does
   not.** Do not borrow the moral framing of the original idea for a system that
   solves a different problem: say **"provenance," not "data dignity."**

## The definition that matters most

The unit of attribution is **not the line or the span**. It is the
**codebase-specific reference**: a token in output that names something only true
of this repo — a project symbol, repo-local path, config key, crate-local type,
magic constant.

**Rationale.** A large share of what a coding agent writes has no session source
and shouldn't — imports, match arms, error handling, test scaffolding, type
signatures implied by the language. Counting those as "unsourced" makes the
metric meaningless.

**Classifier.** Resolves in the codescout symbol index, or appears as a
repo-local path / string → codebase-specific. Language keyword, stdlib, or
vendored dependency symbol → not. **Weight by rarity (IDF-style)**; common
identifiers (`new`, `id`, `config`, `value`) otherwise dominate and produce false
matches.

> **Empirically load-bearing (PV-16).** The probe measured this: an
> identifier-overlap proxy *without* the symbol-index filter reports M1 = 85% and
> trips the >60% kill condition; the filtered classifier reports 46.7% and does
> not. The classifier is not a refinement — it decides the verdict.

## Standing design decisions

**Architecture is a cost-ordered funnel, not a single classifier.** Each tier
shrinks the candidate set for the next; *"as needed"* means a tier runs only when
the one below says it is worth it.

| Tier | What | Status after measurement |
|---|---|---|
| **0** | Structural bookkeeping — result IDs, content hashes, span tagging. Free, no model, always on. Produces a superset ("could have influenced"), not attribution proper | **Validated** — join key exact (PV-1) |
| **1** | **The gate.** Small/cheap. Fires on blast radius, low lexical overlap, codebase claims absent from context, spec in candidate set, imminent commit. **The gate is the entire design** — badly tuned, attribution costs more than the work it audits | Unmeasured |
| **2** | Lexical then embedding matching. ~200 candidates → ~5 | Constraint found (PV-4) |
| **3** | Leave-one-out ablation. The real counterfactual. Feasible here precisely because the candidate set is already small | Hazards PV-19/PV-20 |
| **4** | Cluster and name, by Librarian graph locality (human-meaningful clusters), never raw chunk lists | **Descoped** — M2 kill fired (PV-3) |

**Out-of-band.** Must **not** consume the agent's context window or add loop
latency. Sidecar consumes the session tool-call log, writes to SQLite alongside
the existing index. Surfaces only via hooks, a `/provenance` command, and
commit/PR artifacts.

**~~Three states, never two~~ — FOUR states. Amended 2026-08-04 (PV-30, PV-11).**

| state | meaning | measured? |
|---|---|---|
| `attributed` | a context source contains the reference | yes |
| `unsourced` | in the repo, but nothing in context carried it | yes — but see PV-5 |
| `unrecorded` | human-written, or predates the system | yes (PV-11) |
| `invented` | names nothing that exists — confabulation | **NO — excluded by construction** |

Two corrections the measurement forced:

1. **A fourth state exists and the instrument cannot see it.** A confabulated
   symbol resolves nowhere, so the codebase-specific-reference unit filters it
   out *before* it can be counted. `invented` was silently collapsed into
   `unsourced` and nobody checked whether the two were separable (PV-30).
   Consequence for the architecture: **confabulation detection must not return
   as a tier** — as a tier it inherits the unit definition that made it
   unmeasurable. Separate project, separate unit (PV-12).

2. **"`unrecorded` will dominate early" is scope-dependent, not universal.**
   True at whole-repo scope (72–99% of lines); false at working-diff scope
   (9–17% in the primary repos). The claim was load-bearing for the default UX
   and it only holds for one of the two candidate scopes (PV-11 → PV-17).

Collapsing `unsourced` and `unrecorded` would still poison the feature. The
newer risk is collapsing `unsourced` and `invented`, which the current unit does
by default.

**Commit trailer is the durability mechanism.**

```
Derived-From: codescout://symbol/SessionStore::rotate@a1b2c3d
Derived-From: codescout://artifact/adr-014#3@f4e5d6c
Unsourced-Spans: 2
```

Content-hashed, survives in git history, lets the staleness check run in CI
without the originating session existing.

## Measurement verdicts — 2026-08-03 probe

80 sessions (20 per bucket) across 8 repos / 5 languages, 4.36 M injected tokens,
1,093 patches, 5,210 attributed references. Full numbers:
`scratch/provenance-probe/results.json`.

| Metric | Kill condition | Measured | Verdict |
|---|---|---|---|
| **M1** context utilisation | > 60% → context economics not a win | 48.3% line / 21.1% identifier (median) | **SURVIVES** — on a biased sample (PV-2); exploratory bucket alone is 67.6% |
| **M2** attribution density | median 1–2 → skip clustering | median **2**, 67.7% ≤ 2 | **KILL FIRED** — skip clustering (PV-3) |
| **M3** unsourced rate | > 40% → confabulation detection is noise | 18.7% nominal, ≈8.5% corrected | **number passes, premise void** (PV-5) |
| **M5** cluster cardinality | median > 10 after collapse → presentation fails | median **7** (p75 11, p90 13) | **SURVIVES, marginally** (PV-6) |

**The honest reading, stated plainly.** The failure mode for exploratory
measurement is a report that finds the numbers encouraging because everyone
involved wants to build the thing. Guarding against that: **M2's kill condition
fired cleanly and Tier 4 is descoped.** M3 passes its threshold but does not
measure what it was meant to measure. M1 survives only on the small/medium
session regime. The one robust, decision-relevant result is about **retrieval
shape**, not provenance (PV-7) — and it is actionable without building any
attribution machinery.

Current position: **Tier 0 is a review aid, not yet a system.** That is a useful
result, not a soft no.


### Round 2 — 2026-08-04

Four measurements the first probe left open. Method and numbers:
`scratch/provenance-probe/round2_{git,langfuse,gate}.json`.

| Item | Question | Answer |
|---|---|---|
| **PV-8 / PV-2** | Does M1 survive in the compacted large-context regime? | **Yes, and it strengthens.** M1-line collapses 24.1% → **9.9%** small→large |
| **PV-9 / M6** | Do specs change after code derives from them? | **Rarely, at any horizon that matters.** 5.7% at >30d, 2.1% at >90d |
| **PV-11** | Does `unrecorded` really dominate? | **Only at whole-repo scope.** 72–99% all-history vs **9–17%** agent-era |
| **PV-26** | Is Tier 1 tunable? | **Not as specified.** Default config: 78.8% fire rate, 1.12× lift, 25.5% overhead |

**M1 by context size** (64 sessions reconstructed from Langfuse payloads,
11.4 M injected tokens, large-vs-small compared *within* the same instrument):

| band | n | median size | M1-line | M1-ident | M3 |
|---|---:|---:|---:|---:|---:|
| 50–150 KB | 10 | 96 KB | 13.1% | 3.0% | 0.0% |
| 150–400 KB | 13 | 242 KB | 38.1% | 17.3% | 11.0% |
| 400 KB–1 MB | 14 | 639 KB | 31.9% | 16.8% | 18.4% |
| 1–2.5 MB | 14 | 1218 KB | 29.0% | 20.4% | 26.4% |
| 2.5 MB+ | 13 | 3568 KB | **6.6%** | 23.1% | **31.5%** |

Utilisation **falls** as context grows while identifier reuse **rises** — huge
contexts recycle a denser vocabulary while leaving far more lines untouched. The
>60% kill condition is nowhere near in the regime the transcript probe could not
see, so **PV-2's caveat on M1 is discharged in the programme's favour**. M3 moves
the other way and approaches its threshold (PV-28), though its premise remains
void (PV-5).

**Tier 1 gate simulation** (1,093 patches, 16,871 assistant turns; target =
patch contains ≥1 unsourced reference, base rate 30.1%):

| trigger | fire rate | lift over base | recall | overhead |
|---|---:|---:|---:|---:|
| `n_refs >= p75` | 25.2% | **2.15×** | 54.1% | 8.2% |
| `imminent_commit` | 8.8% | 1.45× | 12.8% | 2.8% |
| `spec_in_candidates` | 45.6% | 1.22× | 55.9% | 14.8% |
| `>= 4 files edited` | 60.6% | **0.95×** | 57.5% | 19.6% |
| **ANY (design default)** | **78.8%** | **1.12×** | 87.8% | **25.5%** |

The design's own warning — *"badly tuned, attribution costs more than the work it
audits"* — is what the specified configuration does: fires on four patches in
five for 1.12× lift. One trigger (`>= 4 files edited`) scores *below* base rate.
But a **single**-trigger gate on patch reference count reaches 2.15× lift at 8.2%
overhead, so Tier 1 is buildable — just not as designed.

Two triggers named in the design (`low_lexical_overlap`, `claims_absent`) are
definitionally subsets of the target; their precision is tautological and is not
evidence of tunability. The whole gate analysis also inherits M3's measurement
error — per the 577-reference audit only ~46% of "unsourced" is genuine.

### Round 3 — 2026-08-04 (adversarial review of round 2)

External review challenged the round-2 gate result. **The challenge was correct
and the round-2 conclusion is reversed.**

**Tier 1 — the 2.15× was a measurement artifact (PV-26, PV-31).** The target was
a *count* threshold ("patch has ≥1 unsourced ref") over a label carrying ~54%
per-reference artifact rate, so a count-correlated trigger accumulates false
flags and gains precision with no predictive content.

| test | lift of `n_refs >= p75` |
|---|---|
| observed, raw count target | 2.15× |
| **null model** — each flag dropped at p=0.544 | **median 2.468** [p5 2.363, p95 2.602] |
| rate target, exact matcher | 1.41–1.60× |
| count target, fixed matcher | 2.09× |
| **rate target + fixed matcher** | **1.37–1.57×** |

The observed lift lies **below the entire null distribution** — pure
miss-accumulation over-explains it. Against a matcher-corrected, rate-based
target (base 11.4%) no named trigger clears ~1.4×. **Tier 1 as specified is not
tunable, and the round-2 reading was measuring the matcher.**

This is the definitional twin of W-5 one level out: W-5 caught triggers that were
*subsets* of the target; PV-31 catches triggers merely *correlated with the
count* the target thresholds.

**PV-4 shipped (composition-aware matching).** Source-side segment/stem indexing
plus output-side basename/stem/parent probing cut unsourced references from 1,090
to 784 over 1,093 patches — **28.1% recovered**. Tier 2's prerequisite is
satisfied.

**PV-32 — the granularity reading is confirmed.** Round 1's source taxonomy
crossed with round 2's size bands, run inside the Langfuse instrument per W-4:

| source type | M (242 KB) | XXL (3.6 MB) | decay | share M → XXL |
|---|---:|---:|---:|---|
| tool output | 19.6% | **2.1%** | **0.11×** | 8.9% → **77.6%** |
| file read | 66.5% | 20.9% | 0.31× | 17.6% → 11.3% |
| symbol lookup | 81.8% | 70.5% | **0.86×** | 13.7% → **1.1%** |
| search | 50.4% | 68.4% | **1.36×** | 6.9% → 1.4% |
| tracker / memory | 40.9% | 78.4% | 1.92× | 7.5% → 0.5% |

Decay is **differential, not uniform** — coarse sources collapse, fine-grained
sources hold. So "inject at finer granularity" is the right frame and "inject
less" is not.

The unpredicted result is the share inversion: in the largest sessions **77.6% of
context is shell output at ~2% utilisation**, while symbol lookup and search have
nearly vanished (1.1% and 1.4%). The dominant waste in the large regime is tool
output, not file reads — which redirects PV-29.

### Round 4 — 2026-08-04 (corrected matcher applied to everything)

Round 3 corrected the matcher but applied it only to the gate's target. Round 4
applies it to every metric, and settles the last open challenge.

**Every metric, 80 transcript sessions, exact → corrected:**

| metric | exact matcher | corrected | moved? |
|---|---:|---:|---|
| M1-line (median) | 48.3% | **53.1%** | up — more refs resolve, more sources count as used |
| M1-ident | 21.1% | 21.3% | no |
| **M3** | 18.7% | **6.2%** | **down 3×** (IDF-weighted 6.5%) |
| M2 density | 2 | **2** | **no** |
| M5 sources / patch | 21 | **21** | **no** |
| M5 clusters / patch | 7 | **7** | **no** |

Median 50% of previously-unsourced references recovered per session. M2's kill
and M5's marginality are **robust to the correction** — the matcher fix does not
revive clustering or rescue the presentation model. Per bucket, corrected M3:
refactor 17.6%, greenfield 6.2%, exploratory 4.2%, bugfix 3.6%. The exploratory
bucket's M1-line moves to **73.1%**, further above the 60% kill.

**PV-34 — unsourced-ness is a symbol phenomenon.** Per-session medians with the
corrected matcher: path-unsourced **0.0%** (p75 3.8%), symbol-unsourced **8.6%**
(p75 18.8%), with paths making up 26% of all references. Attribution for path
references is solved bookkeeping needing no model; symbol references are the only
hard part. A design that treats both alike will spend its budget on the easy half.

**PV-28 — the M3-rises-with-size trend is real, and the challenge to it fails on
its own mechanism.** The proposed explanation was that large contexts carry more
path-shaped references. They do not — path fraction shows no trend across bands
(13.0 / 20.0 / 30.1 / 19.9 / 22.8%).

| band | M3 exact | M3 corrected | path-unsourced | symbol-unsourced |
|---|---:|---:|---:|---:|
| 50–150 KB | 0.0% | 0.0% | 0.0% | 0.0% |
| 150–400 KB | 11.0% | 2.7% | 0.0% | 3.6% |
| 400 KB–1 MB | 18.4% | 13.3% | 0.0% | 17.4% |
| 1–2.5 MB | 26.4% | 21.5% | 5.9% | 24.8% |
| 2.5 MB+ | 31.5% | 21.6% | 13.3% | 23.8% |

After correction M3 still climbs 2.7% → 21.6%, carried by **symbol**-shaped
references (3.6% → 23.8%). Two concessions: the corrected top-band figure is
21.6%, not 31.5%, so "approaches the 40% threshold" was overstated; and the
artifact rate is **not** stable across bands — recovery runs 20–44% and
path-unsourced climbs to 13.3% at the top, so composition-aware matching itself
degrades in very large contexts.

**PV-24 — the M6 outlier is structural, and yields the feature's predicate.**
41% of that repo's long-lag events (29 of 71) originate in a single agent-owned
design-system documentation tree, the rest concentrated in ADR subtrees.
Revisions-per-spec is 1 there and 1 in the comparison repo — specs do not churn
more, a *subset* gets revisited months later. **Stale-drift suits repos carrying
a durable design-system / ADR corpus that outlives the sessions referencing it,
and not repos whose docs are session-scoped plans revised in the same sitting.**

### Round 5 — 2026-08-04 (closing the gate class; decomposing the symbol residual)

**Tier 1 is now closed as a CLASS, not pending a better trigger.** Round 3's null
assumed a 54.4% artifact rate. The corrected target needs no such assumption:
assume every reference is independently unsourced at one global rate fitted from
the data (q = 0.0760) with **no per-patch signal at all**, then
`target_i = Binomial(n_refs_i, q) >= 1`.

| | value |
|---|---|
| corrected target base rate | 24.3% |
| **observed lift** | **2.092** |
| pure-opportunity null | median 1.999 [p5 1.894, **p95 2.100**] |

Observed sits **inside** the null interval — though in its upper tail (~94th
percentile), so the honest statement is *no per-patch signal detectable at the 5%
level*, not *far below*. Reference-count correlation as a trigger class is
closed. Reopening requires a trigger uncorrelated with reference count **and** a
rate-based target.

**The symbol residual is 43% irreducible (PV-36).** Review proposed splitting
qualified symbols on the language separator. The tokeniser already did that —
`IDENT_RE` matches maximal `[A-Za-z0-9_]` runs, so `::`, `.`, `->` and `#` have
always been separators and `SessionStore::rotate` has always been two
independently-checked references. The untested analogue is **morphological**:

| | count | share of residual |
|---|---:|---:|
| symbol-unsourced after composition | 689 | 100% (14.4% of symbol refs) |
| — **atomic** (one component, nothing to split) | 294 | **42.7%** |
| — multi-component | 395 | 57.3% |
|   ALL components present in context | 187 | 27.1% |
|   ANY component present in context | 373 | 54.1% |

Strict morphological recovery shrinks the residual a model-based tier would
address from **10.4% → 7.6%** of all references, of which **4.5pp is atomic**.

That answers the fork the review posed. Recovery is real but partial — so the
residual is not simply accumulated matcher debt. **The atomic 42.7% cannot be
compositional matcher error even in principle**, which is the first positive
evidence that genuine unsourced-ness exists. That is the prerequisite PV-12 was
waiting on: a confabulation instrument now has something to measure.

**PV-35 — the synthesis that makes the product argument.** As context grows,
M1-line falls 38.1% → 6.6% while symbol-unsourced rises 3.6% → 23.8%. More of
what is injected goes untouched, *and* more of what gets used was never injected.
Retrieval precision and recall degrade **together**. PV-7 alone measures waste;
the pair measures waste and failure moving in opposite directions at once.

### Round 6 — 2026-08-04 (the floor was not a floor)

Three corrections, two of them to round 5.

**1. The power study is not worth running — closure restated on
decision-relevance.** Observed excess over the null median is 0.093, ~4.7% of the
opportunity effect. Resolve the power question *maximally in the gate's favour*
— grant the entire excess is real signal — and it still cannot be tuned, because
every threshold adjustment moves the opportunity term with it. An effect that
small cannot have a gate specified around it. That closes the class on
decision-relevance rather than significance, which is more durable and avoids a
1,922-session sampling exercise.

**2. The cost model erred in a direction correlated with the trigger (PV-37).**

| patch set | n | mean Tier-2 candidates |
|---|---:|---:|
| firing (`n_refs >= 11`) | 275 | **36.20** |
| quiet (`n_refs < 11`) | 818 | 19.40 |

**1.87×.** The gate selects on reference count and the candidate set is generated
*from* references, so the error concentrates exactly where the gate fires rather
than averaging out. Scope: this understates Tier 2's **input** cost; Tier 3's
ablation is unaffected if the narrowing target stays at ~5 survivors.

**3. The atomic floor is not a floor (PV-38) — round 5's claim was wrong.**

Atomic identifiers cannot shrink by better *matching*. They can shrink by better
*classification*, which is the axis this programme has now been burned on three
times (PV-30, F-5, and here). A single-morpheme PascalCase identifier receives the
loose bound from the F-5 fix, so ordinary vocabulary that also resolves in the
repo index is admitted.

Descriptors on 221 atomic-unsourced references against 414 atomic-**sourced** as
control, with the general-English proxy built from human-typed prose across the
corpus (2,004 user-text documents, system-reminders stripped):

| descriptor | atomic-unsourced | atomic-sourced (control) |
|---|---:|---:|
| in-repo DF ratio (median) | **0.0055** | 0.0071 |
| prose rate (median) | **0.0035** | 0.0010 |
| share in > 1% of prose docs | **31.2%** | 20.5% |

Both descriptors point the same way: atomic-unsourced references are
simultaneously **rarer in the codebase and commoner in ordinary English** than
the control — the convergent-naming signature. An agent reaching for the obvious
word is neither confabulating nor failing retrieval.

The enrichment is real but partial (68.8% of the population does not carry it),
so this **lowers the floor without eliminating it**. The honest position is that
no lower bound on genuine unsourced-ness has been established yet.

**PV-12's labelling population is now defined**: 221 distinct (token, repo) pairs
— tractable. Round 5's 294 counted per-session occurrences; 221 is the
deduplicated set. Label into four buckets — *convergent naming | instrumentation
gap | cross-session recall | genuine unsourced* — because only the fourth is what
a confabulation instrument detects. If the fourth comes back small, PV-12's unit
must be something other than the resolving-reference population: the same lesson
as PV-30, one level further in.

### Round 7 — 2026-08-04 (a floor at last, and the answer on PV-12)

**Breadth beats prose rate, and reveals the mechanism the proxy missed (PV-39).**
Cross-repo breadth needs no external wordlist — it comes from the eight symbol
indexes already built — and separates the populations twice as cleanly.

| | atomic-unsourced | atomic-sourced (control) |
|---|---:|---:|
| median repos where token appears (of 8) | **6.0** | 3.0 |
| concentrated at | breadth 6–8 (52%) | breadth 1–2 (44%) |

Cross-tab on the 221 atomic-unsourced (prose > 1% of docs; breadth ≥ 3 repos):

| cell | reading | share |
|---|---|---:|
| hi prose / hi breadth | ordinary vocabulary — convergent | 27.6% |
| hi prose / lo breadth | **contamination class** (user's own jargon) | **3.6%** |
| lo prose / hi breadth | **code-convention term** (`impl`, `mux`, kin) | **51.1%** |
| lo prose / lo breadth | **strongest genuine candidates** | **17.6%** |

The largest cell is the one the prose proxy was blind to. **78.7% of the residual
is high-breadth** — convergent by one mechanism or another. The control confirms
the direction: atomic-*sourced* sits at 42.5% in the lo/lo cell versus 17.6% for
unsourced, i.e. repo-specific tokens get sourced precisely because they are in
context.

The prose-proxy caveat is quantified at **3.6%** — real but small — and its
direction is favourable: contamination inflates the convergent count, so the
genuine fraction is understated, and the round-6 finding survives its own
limitation rather than depending on it.

**A floor, at last (PV-40).** Every population so far was defined by *survival*
through filters, and a survival-defined population can only yield a ceiling —
each admission criterion is an untested source of false members and there is
always one more filter unchecked. That is the shape of PV-30, F-5 and PV-38. A
floor requires the opposite: admit only on positive evidence, and accept
undercounting, because undercounting by construction is what makes a lower bound
sound.

Built as the intersection of two independent exclusions both cutting toward
genuineness — absent from the human-prose corpus *entirely* **and** appearing in
exactly one of eight repos:

| | tokens | share of atomic-unsourced | share of ALL references |
|---|---:|---:|---:|
| strict (1 repo) | **13** | 5.9% | **0.20%** |
| relaxed (≤ 2 repos) | 28 | 12.7% | 0.42% |

**This is the first lower bound produced in seven rounds.** Every previous number
was a ceiling. The genuine rate lies in **[0.20%, 7.6%]**, and PV-39 places it
near the floor.

**PV-12 — do not build.** A confabulation instrument would be built to detect
something occurring in roughly **1 in 500** codebase-specific references. The
pre-agreed rule was that a near-zero intersection is itself the answer. It is not
literally zero; it is close enough.

### Round 8 — 2026-08-04 (shell output is the mechanism)

**The one number carrying a cross-instrument inference, fixed.** The 19.6% pooled
share (transcripts) vs 77.6% (large Langfuse sessions) violated W-4. Recomputed
**within Langfuse**, pooled per band — and the trend is the stronger of the two
hypotheses:

| band | shell share | shell util | non-shell share | non-shell util | overall |
|---|---:|---:|---:|---:|---:|
| 50–150 KB | 5.8% | 38.6% | 94.2% | 17.8% | 19.0% |
| 150–400 KB | 8.9% | 19.6% | 91.1% | 37.3% | 35.7% |
| 400 KB–1 MB | 15.2% | 27.1% | 84.8% | 33.8% | 32.8% |
| 1–2.5 MB | 28.0% | 19.4% | 72.0% | 31.6% | 28.2% |
| 2.5 MB+ | **77.6%** | **2.5%** | 22.4% | **24.0%** | **7.3%** |

*(corrected matcher; share is matcher-independent and identical under both)*

**Shell share is monotonic: 5.8 → 8.9 → 15.2 → 28.0 → 77.6%.** So shell output is
not merely inefficient — it is **the mechanism by which sessions reach the
saturated regime at all**. Capping or buffering it does not just improve
utilisation *within* large contexts, it prevents the large-context regime.

The decomposition confirms it: across the four upper bands **non-shell
utilisation falls only 37.3% → 24.0%**, while overall collapses 35.7% → 7.3%.
The collapse is shell *share* growth, not non-shell degradation.

**PV-29 is restated (and was understated).** "~28% of context at ≤26%
utilisation" is an efficiency trim. "77.6% of context at 2.5% utilisation, and it
is what drives sessions into saturation" is the dominant term in the entire
measurement. It is also a *different subsystem* from retrieval granularity, with
a different fix.

**PV-43 — the fix, and the third flat-constant.** `shell_output_limit_bytes` was
never retired functionality: it was accepted, documented, and a **silent no-op**,
removed as vestigial (archived bug 2026-06-28). Nothing was lost. The mechanism
that *does* exist is the progressive-disclosure buffer —
`TOOL_OUTPUT_BUFFER_THRESHOLD = 10,000` / `INLINE_BYTE_BUDGET = 9,000`
(`src/tools/core/types.rs:22`) — applied **per call**, with no accounting of
accumulated session context. 500 calls each just under the threshold reach ~4.5 MB
inline: precisely the top band. What PV-42 supports is a threshold **conditional
on accumulated context size**, which is a policy change on when output goes to a
`@tool_*` buffer — not new machinery. Third instance of the flat-constant
mistake, after PV-37 (cost per firing) and PV-41 (Tier-2 narrowing).

### Round 9–10 — 2026-08-04 (round 8 was wrong: it was never shell)

The redundancy question was asked, and answering it exposed a larger error.

**Round 9 — redundancy hypothesis not supported.** Repeated normalised lines are
0.0 / 1.5 / 0.7 / 1.0 / **2.2%** of shell bytes across the bands; bytes from an
already-run command 3.1 / 8.4 / 4.3 / 7.2 / 9.6%. The top/mid *ratio* is 2.1× but
the absolute level is ~2% — 97.8% of top-band shell bytes are novel lines. (The
script's verdict rule tested the ratio and printed "REDUNDANCY"; the magnitude
refutes it. A relative threshold on a tiny base is its own small instance of the
PV-44 family.)

**Round 10 — the bucket was a catch-all (PV-44), and PV-42 reverses.**
`tool_source_type()` ends with `return "tool_output"`, so every unrecognised tool
joined a bucket whose *name* implied shell. Split properly:

| bucket | S | M | L | XL | XXL |
|---|---:|---:|---:|---:|---:|
| **shell** share | 5.5% | 7.1% | 13.3% | 11.1% | **2.5%** |
| shell utilisation | 40.8% | 22.0% | 28.5% | 37.2% | **19.3%** |
| **mcp_other** share | 0.3% | 1.7% | 1.3% | 15.5% | **75.1%** |
| mcp_other utilisation | 0.0% | 10.2% | 24.4% | 8.2% | **1.9%** |

**Shell share is not monotonic** — it peaks mid-range and *falls* in the top
band, and shell utilisation never cliffs. The monotonic climber is **third-party
MCP output**, 95.6% of which is one browser-automation server in the top band
(shell: 3.2%).

Scope caveat, stated plainly: 11 of 13 top-band sessions carry >1 MB of it,
across 2 repos, 57.8% concentrated in one. Distributed enough not to be a single
project's anomaly; narrow enough that it is one tool *class*, not a general law.

**What this does to the shippable item (PV-29).** The dominant saturating term is
not codescout's to fix — **codescout cannot buffer what it does not emit**; the
progressive-disclosure threshold governs codescout's own tool results, not another
server's. What survives in scope: the granularity result is intact (file_read
utilisation decays 50.6% → 21.7% while symbol_lookup holds at 73.7% and search at
69.6%), and codescout's own shell output is **healthy** — 2.5–13.3% share at
19–41% utilisation, no cliff, 2.2% repetition. The honest scope of the shippable
item is smaller than round 8 claimed.

### Round 11 — 2026-08-04 (not one law — but a size ceiling worth shipping)

**The single-law hypothesis is refuted (PV-47).** Tested on 12,335 per-call
records: is utilisation a function of per-call payload size regardless of source?

The pooled curve is an **inverted U, not a decay**:

| payload | <500 B | 500 B–2 K | 2–8 K | 8–32 K | 32–128 K | >128 K |
|---|---:|---:|---:|---:|---:|---:|
| utilisation | 6.9% | 28.1% | **51.7%** | 32.9% | 4.7% | **0.0%** |

Small payloads are poorly utilised too, so "smaller is better" is false at the
low end. And controlling for size, **source still separates strongly** —
within-bin spread median **45.9%** against an across-bin range of 51.7%. In the
<500 B bin alone: symbol_lookup 47.2%, search 51.4%, file_read 16.5%, mcp_other
8.5%, shell 4.4% — a 10× spread at identical payload size.

*(My script printed "ONE LAW" from a `spread < range` test. 45.9% < 51.7% is
true and meaningless as a discriminator — the second badly-constructed verdict
rule in three rounds, after round 9's ratio test. Both printed the opposite of
what their own numbers showed.)*

**But there IS a ceiling effect in the tail, and it is the shippable item
(PV-48).**

| threshold | calls | share of info-bearing tokens | utilisation |
|---|---:|---:|---:|
| ≥ 8 KB | 633 | 76.3% | 7.2% |
| ≥ 32 KB | 137 | 61.1% | **0.8%** |
| ≥ 128 KB | **74** | **51.3%** | **0.0%** |

**Half of all information-bearing context arrives in 74 payloads, and none of it
is ever referenced.** Composition above 32 KB: mcp_other 80.3%, file_read 8.8%,
system_prompt 7.7%, user_prompt 3.1%.

Because it is a **size** rule rather than a source rule it is general — it
predicts unmeasured tools, and it puts codescout's own `file_read` back in scope,
which a rule scoped to a third-party MCP server would not. A threshold near 32 KB
removes ~61% of injected context at a cost of 0.8% of referenced content.

Below the tail, source still dominates, so granularity work (PV-32) remains the
second lever rather than being replaced.

**PV-51 — one unresolved question that decides ownership.** If a PostToolUse hook
can rewrite a tool *result*, PV-48's ceiling is enforceable in the companion. The
companion declares PostToolUse for exactly two matchers (`EnterWorktree`,
`mcp__.*__workspace`) and both only act on session state — no precedent for
result rewriting exists in the plugin. Whether Claude Code's hook contract
supports it is **not established by anything in this repo** and should not be
assumed either way.

### Round 12 — 2026-08-04 (the cost side is a lower bound; the fix is ours)

**PV-51 resolved — the dominant term is ours, not upstream.** A Claude Code
hook-documentation check (performed in the design-review session; **not**
independently verified here — no local copy of the hook schema exists and the
companion has no precedent) establishes both halves: PostToolUse returns
`hookSpecificOutput.updatedToolOutput`, which **replaces** the tool's original
result, and matchers accept regex over MCP tool names so `mcp__.*` catches every
third-party result. Constraints: hook output caps at 10,000 characters with file
spill beyond that — itself a buffer, which suits the use case — and PostToolBatch
explicitly cannot modify individual outputs, so bounding is per-call by
construction.

**Architecture, using machinery that already exists on both sides:** a PostToolUse
hook matching `mcp__.*` writes the full payload into codescout's existing
`@tool_*` buffer and returns a summary plus handle via `updatedToolOutput`.
Retrieval stays available; inline cost collapses. PV-43's policy, applied at the
layer that can enforce it.

**PV-52 — the 0.0% is largely the metric, not the content.** Utilisation counts
codebase-specific reference reuse, and a browser page dump contains essentially
nothing that resolves in a repo symbol index:

| | calls | contain ZERO codebase-specific tokens | median spec/KB |
|---|---:|---:|---:|
| ≥ 8 KB | 633 | 15.2% | 0.68 |
| ≥ 32 KB | 137 | 52.6% | 0.00 |
| ≥ 128 KB | **74** | **82.4%** | **0.00** |
| *control 2–8 KB* | 1707 | **7.5%** | **2.54** |

By source above 128 KB: mcp_other 81.7% zero-spec, **file_read 100%** zero-spec.
Exactly-zero across 74 payloads is what a **blind metric** looks like, not what
unused content looks like.

This is the **fifth** instance of the exclude-by-construction family (PV-30, F-5,
PV-38, PV-44, now this) and the first to land on the *benefit* side of the only
shippable item. Consequence: the benefit figure (61% of injected tokens) is
solid; **the cost figure (0.8%) is a lower bound** by an unknown amount, and the
true ratio is below 76:1.

It also settles the design question in the right direction: **buffer, do not
truncate.** When a metric cannot price content, dropping it bets on a number the
metric was never able to produce; buffering makes the bet recoverable for one
retrieval call.

**PV-47 amended — the left arm of the U is composition.** In the <500 B bin,
low-utilisation sources supply **82.5%** of tokens (shell 29.2%, user_prompt
27.8%, system_prompt 19.1%, mcp_other 6.4%) while symbol_lookup and search sit at
47.2% and 51.4% *in that same bin*. Small payloads are not intrinsically badly
used. So the size rule is justified as **robustness** — the safe variable when
sources cannot be enumerated — and explicitly **not** as mechanism.

**PV-46 gains a fourth instance, the first outside codescout.** Claude Code's
`MAX_MCP_OUTPUT_TOKENS` defaults to 25,000 tokens per call (warning at 10,000).
The guard works exactly as specified and is irrelevant at session scale — a
top-band session carries ~2.7 MB of MCP output, ~670,000 tokens, i.e. 27+ fully
compliant calls. Per-event budget, unbounded event count. Because it sits
upstream of this codebase entirely, PV-46 is not a codescout code-quality
observation but a pattern in how agent tooling is specified generally.

### Round 13 — 2026-08-04 (final: the check eliminates the second lever)

PV-53's check — *what would be invisible to this metric even if it were happening
constantly?* — applied to the surviving granularity finding. It eliminated it.

**file_read, split by path class per band:**

| band | source-file util | external-file util | external share of file_read |
|---|---:|---:|---:|
| 50–150 KB | 58.9% | 51.4% | 92.0% |
| 150–400 KB | 63.1% | 70.7% | 67.7% |
| 400 KB–1 MB | 81.9% | 34.3% | 59.5% |
| 1–2.5 MB | 81.2% | 23.8% | 52.3% |
| 2.5 MB+ | **84.4%** | **7.7%** | 79.5% |

**Source-file utilisation RISES with context size** (58.9% → 84.4%). The apparent
decay was entirely composition — large sessions read progressively more
non-repo content, and a 128 KB "file" is a lockfile, a bundle, a fixture or a
log, which is why file_read was 100% zero-spec above 128 KB (PV-52).

**PV-7 falls with it.** Pooled, file_read is 35.0% across all paths but **77.0%**
across repo source — which is only 18.4% of its token volume. Like-for-like
against symbol_lookup (72.3%) and search (71.6%), **file reads of repo source are
at or above structured retrieval**. The programme's supposedly most robust result
— "structured retrieval is ~3× more efficient than raw" — was comparing unlike
populations. The correct statement is that retrieval of repo source is uniformly
well-utilised (72–77%) regardless of tool, and everything else is not.

Second instance of composition masquerading as a size effect (after PV-47's left
arm); third time overall a pooled average concealed a changing mix.

**Consequence — the programme converges to ONE intervention.** Granularity work
on file_read is not supported: codescout's source reads are already excellent and
*improve* under context pressure. Every strand now points at PV-29 alone — buffer
oversized tool results at the companion layer, triggered on per-call size,
targeting large non-repo / non-source payloads.

### The four transferable rules

Worth more than the subsystem would have been. Five, after the final exchange —
and three of them are answerable **before any data exists**.

| | rule | when |
|---|---|---|
| **PV-25** | Pin the unit before the threshold — a threshold on an unpinned unit is not pre-registered. | design time |
| **PV-53** | Ask what would be invisible to this metric even if it were happening constantly. | design time |
| **PV-58** | Ask whether the things compared can even have the same mix, given what they are. | design time |
| **PV-40** | When every result is a ceiling, build the population from evidence of *belonging* and accept undercounting. Check first whether data in hand supports an inverted admission rule. | mid-measurement |
| **PV-46** | A per-event budget in a system with an unbounded event count is statically detectable — ask what bounds the events. | code review |

**PV-55** puts the three design-time rules into the measurement-plan template as
three lines: *state the unit for each metric, state what each metric cannot see,
and state whether the things compared share a domain.*

PV-58 is the one that would have been cheapest of all. `symbol_lookup` can only
return repo source; `file_read` can return anything — so PV-7 was comparing a
specialist against a generalist over the generalist's whole caseload, 81.6% of
which the specialist cannot touch. No measurement was needed to see it. The
empirical mix check (PV-57's method) stays necessary for comparisons that PASS
the domain test, such as file_read across context bands where the domain is
constant and only the mix moved.

### Two things recorded so they survive

**Buffer-not-truncate is load-bearing, not prudent (PV-29).** The intervention
targets external files, MCP output, logs and docs — exactly the population
PV-54's three blind spots cover. The metric prices all three at zero, so
truncating drops content whose value the evidence base was never able to
measure. The efficiency numbers do not license truncation and never did.

**Repo-source reads improve under pressure — do not cap them (PV-59).** Recorded
as a constraint precisely because it has no intervention attached and would not
otherwise survive. Source-file utilisation rises 58.9% → 84.4% across context
bands, ending above symbol_lookup (72.3%) and search (71.6%). "Large sessions
waste context, so cap file reads" is a plausible future proposal and it is
refuted in advance for repo source. Any capping or granularity policy must exempt
paths that resolve in the repo symbol index.
## Known hazards — do not rediscover these

Carried from the design exploration, plus two the probe added. Each is a PV-N
entry with its full statement; summarised here so a future session sees them
before proposing anything.

- **Sufficiency vs. necessity** (PV-19) — if two sources are each independently
  sufficient, single leave-one-out marks both irrelevant. Common where a symbol
  is reachable by several retrieval paths. Needs **group ablation designed in
  from the start**; Shapley-style is principled but combinatorial.
- **Nondeterminism** (PV-20) — ablation compares two sampled generations. Needs
  greedy decoding or multi-sample comparison.
- **"Influenced" ≠ "should have influenced"** (PV-21) — attribution answers the
  first; review usually wants the second. Do not let the feature over-promise.
- **No model internals** (PV-22) — attention/gradient attribution is unavailable
  through the API. Behavioral methods only — a hard precision ceiling.
- **Exact-match attribution undercounts paths** (PV-4) — probe-discovered.
- **The instrument must not write into its own corpus** (PV-23) — probe-discovered;
  see R-51.

## Related artifacts

Living in the claude.ai **"AI thoughts"** project — **not readable from Claude
Code**; copy across when needed:

| Document | Contents |
|---|---|
| `provenance-clusters-agentic-coding-design.md` | full design exploration |
| `provenance-measurement-plan.md` | the six metrics in detail |
| `provenance-probe-brief.md` | paste-ready task brief for the measurement run |
| `probe-sessions.py` | schema-discovery prototype — parsing scaffolding only; no symbol-index filter, numbers directional at best (and see PV-16) |
| `lanier-there-is-no-ai-essay-reading.md` | source material |
| `jaron-lanier-lecture-breakdown.md` | source material |

In-repo: `scratch/provenance-probe/` (probe pipeline + `RESULTS.md` +
`results.json`), `docs/trackers/provenance-probe-session-log.md` (F-N/W-N),
`docs/trackers/reconnaissance-patterns.md` R-51.

---

## PV-N entries

The canonical PV-N rows live in the augmentation **params**, not in this file.
The `render_template` projects them into a table when the librarian packs this
artifact into context; to read them directly use:

```
artifact(action="get", id="e12cd7e0060ed9b8", entry_filter={"type": {"eq": "decision"}})
```

Note `entry_total` reports rows **considered** (the whole collection), not rows
matched — `entries` is the match set. That asymmetry is deliberate: it is how a
typo'd filter field is detectable (considered 26, matched 0).

Narrative for entries that need more than a table row goes below, newest first.

### PV-2 — M1's verdict rests on a biased sample

The compaction check was the pre-registered invalidator and it came back
*conditionally* bad. Pooled compaction is 3.0% (90/2,997), which is benign. Split
by session length it is not:

| records | n | compacted |
|---|---:|---:|
| < 100 | 1964 | 0.0% |
| 100–500 | 866 | 1.9% |
| 500–1500 | 72 | 5.6% |
| 1500–5000 | 72 | 65.3% |
| ≥ 5000 | 23 | **100.0%** |

Every session in the top band is compacted — precisely the high-injected-context
sessions M1 most needs. All compacted sessions were excluded, so **M1 is measured
on the small-and-medium regime only** and is biased in an unknown direction with
respect to the large-session regime where context economics would matter most.

**The available fix (PV-8):** the local Langfuse instance holds 65,722
observations whose request payloads contain the **full `messages` array** — the
literal context at emission time, up to 4.7 MB per request. That is the only
route to the compacted regime. Coverage is 2026-06-19 → 2026-07-22 (~1 month),
not the full corpus.

### PV-5 — M3 passes its threshold but does not measure confabulation

**Structural, not a sampling problem.** The classifier only counts a token as a
codebase-specific reference if it **resolves** in the repo symbol index. An
invented symbol resolves nowhere, so it is filtered out before it can enter the
numerator. What M3 actually measures is *references to real repo entities that
were not in context* — a recall/instrumentation metric, not a hallucination
metric.

The inverse measure (symbol-shaped tokens resolving nowhere) was built and is
**unusable as-is**: 2,755 candidates against 2,086 resolved references (132%),
dominated by tool-parameter names (`start_line`, `old_string`, `file_path`), git
SHAs, unicode escape fragments, other repos' paths and third-party API names —
not by invented symbols.

**Consequence for the kill condition:** *"M3 > 40% → confabulation detection is
noise"* was neither confirmed nor refuted. **It was not tested.** Measuring
confabulation needs a different instrument and a ground-truth labelling pass
(PV-12).

### PV-4 — Exact-match attribution structurally undercounts path references

Audit of 577 nominally-unsourced references:

| trace in prior context | share |
|---|---:|
| full string present (matcher missed it) | 33.6% |
| basename present | 11.6% |
| stem present | 7.1% |
| parent directory present | 2.1% |
| **no trace at all** | **45.6%** |

Split by token shape the result is stark: **path-like references have no trace
only 2.2% of the time; symbol-like references 83.4%.** Paths are *composed* from
directory listings and partial paths rather than copied verbatim.

**Design constraint on Tier 2:** lexical matching must be composition-aware for
path-shaped references (basename / stem / parent-directory resolution), or path
provenance will be systematically misreported as unsourced. Symbol-shaped
references are where genuine unsourced-ness actually concentrates.

### PV-7 — The one robust result is about retrieval shape, not provenance

Per-source-type utilisation, pooled over 4.36 M injected tokens:

| source type | share of context | line util. | identifier util. |
|---|---:|---:|---:|
| file read | 29.0% | 57.2% | 31.6% |
| tool output (shell) | 19.6% | **26.3%** | 14.7% |
| symbol lookup | 15.2% | **74.4%** | 22.8% |
| search (grep/glob) | 8.1% | **73.3%** | 22.4% |
| skill / skill listing | 7.9% | **5.0%** | 10.0% |
| user prompt | 7.0% | 14.7% | **70.8%** |
| tracker / memory | 5.3% | 60.6% | 19.8% |
| hook-injected context | 3.4% | 51.1% | 9.3% |
| subagent return | 2.1% | 44.9% | 15.8% |
| edit acknowledgements | 0.8% | 2.3% | 4.5% |

**Structured retrieval is ~3× more context-efficient than raw retrieval.**
Skills + shell output + edit acks are ~28% of context at ≤26% line utilisation.
This is actionable today, needs no attribution machinery, and is arguably the
probe's highest-value output. It should not be allowed to disappear because it is
not the thing the programme set out to find.

---

## Conventions for this tracker

- **ID prefix:** `PV-N`, monotonic, never reused. Registered in
  [`docs/TAXONOMY.md`](../TAXONOMY.md).
- **Append via** `artifact(action="append_entry", id="<this>", id_prefix="PV",
  entry_collection="items", entry={...})` — assigns the next ID atomically.
  Add narrative below only when a table row is insufficient.
- **Query via** `artifact(action="get", id="<this>", entry_filter={"type":
  {"eq":"decision"}})` — the entry collection is filterable.
- **`type` vocabulary:** `finding` (settled measurement result) · `gap`
  (measurement not yet run) · `decision` (open question needing a call) ·
  `hazard` (known failure mode, do not rediscover) · `task` (buildable work).
- **`status` vocabulary:** `settled` · `open` · `blocked` · `descoped` ·
  `carried` (inherited from design exploration, not yet acted on).
