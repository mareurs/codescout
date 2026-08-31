---
id: '7579b32b1cd2362f'
kind: bug
status: open
title: Guide topics are atomic nodes in a graph nobody modelled — 63% of the corpus auto-injected in one session, and three guides already cite sections the API cannot serve
tags:
- guides
- prompt-surface
- get_guide
- progressive-disclosure
- proposal
unverified: 'PARTIALLY SHIPPED, and `open` alone does not say so. Phase 1 (section-grain get_guide) SHIPPED 2026-08-27; the USE probe ran the same day. The DISTRIBUTION probe that directions (b)/(c) were gated on RAN 2026-08-31 -- n=166 sessions, two machines, instrument scripts/probe_guide_section_use.py -- so that gate is SATISFIED. It changed the recommendation: the largest lever is subagent delivery (92.5% of delivered bytes never engaged, 2.3x the main-session waste), orthogonal to BOTH Phase 2 blockers. THREE STRATIFICATION CAVEATS, all measured, none optional when quoting a number: (1) main vs subagent must never be blended -- 45% vs 92%; (2) the sample is COMMIT-MIXED, 84% of it predating the 2026-08-27 section-grain ship, and the apparent 49.0%->34.4% improvement is project MIX, not regime -- codescout-project sessions are flat at 28.0%->27.5% while their share moved 38%->57%; (3) project is the dominant stratifier, ~28% waste in codescout sessions vs ~44-62% elsewhere. Still outstanding: the 17,378 B decomposition and cross-topic shape disambiguation (topic_declaring made `serves:` a cross-topic namespace; no_two_topics_declare_an_overlapping_shape fails the build on the collision, deliberately). Also outstanding and explicitly NOT a measurement: `tracker-conventions is really six topics` is an authoring judgement that needs re-costing before anyone builds on it.'
---

## Symptom

`get_guide(topic)` serves a whole topic body or nothing. Topics are the only unit
of addressing, so a session that needs one section of a guide receives all of it,
and a guide that wants to point at part of a sibling can only point at the whole.

Measured on the corpus, 2026-08-27:

```
error-handling.md                  1,857
project-activation-bootstrap.md    2,594
symbol-navigation.md               3,145
untrusted-content.md               5,317
progressive-disclosure.md          5,669
librarian-runtime.md               9,774
workspace-state.md                10,355
iron-laws-detail.md               11,238
librarian.md                      20,545
tracker-conventions.md            34,333   <- 33% of the corpus, one topic
                                 -------
                                 104,827
```

**In a single working session** (claude-plugins, `f6ae2d77`, 2026-08-26/27) the
auto-inject path delivered `project-activation-bootstrap`, `tracker-conventions`,
`librarian`, `symbol-navigation` and `progressive-disclosure` — **66,286 bytes,
63% of the entire guide corpus** — and `project-activation-bootstrap` fired a
second time after an MCP reconnect. The session consumed perhaps five sections of
it. `tracker-conventions` alone (34 KB) auto-injects on the first `artifact` call
of any session, whatever that call was for.

## The graph already exists — in prose, unresolvable

Measured the same day: **18 `get_guide("...")` citations across 7 of the 10
guides.** They are edges. Nothing reads them.

```
librarian            -> tracker-conventions (x4), librarian-runtime
iron-laws-detail     -> workspace-state (x2), progressive-disclosure, error-handling
workspace-state      -> progressive-disclosure (x2), error-handling
librarian-runtime    -> librarian, tracker-conventions
tracker-conventions  -> librarian, librarian-runtime
progressive-disclosure -> error-handling
symbol-navigation    -> progressive-disclosure
```

**Three of the eighteen already cite a SECTION** — a granularity the tool cannot
serve, written by authors who evidently wanted it:

- `librarian.md:139` — `get_guide("tracker-conventions")` § *One entry format, never two*
- `tracker-conventions.md:604` — `get_guide("librarian-runtime")` § *Trackers as cross-session behavior*
- `workspace-state.md:81` — `get_guide("progressive-disclosure")` § *Path-relative annotation*

A reader following one of those gets the whole sibling and has to find the section
by hand. That is the defect in its cheapest observable form: **the documentation's
own cross-references are more precise than its retrieval API.**

## Counterexample — the same delivery rule, the opposite outcome

Added 2026-08-27 from a concurrent session (`77c6f4ae`) in this checkout, which
volunteered the case that cuts against the framing above. **It is the arm the
measurement was missing, and it changes the claim.**

That session took **six** auto-injected guides on the same rule —
`project-activation-bootstrap`, `progressive-disclosure`, `tracker-conventions`,
`symbol-navigation`, `librarian`, `workspace-state`, each on the first call
touching its topic, none requested. Delivery reproduced exactly.

But `tracker-conventions` **earned its 34,333 bytes there.** Archiving two bug
files, the session used the status vocabulary, the archive trigger, the
SHA-plus-patch-id rule, the citation-sweep grep, the `## PREFIX-N — title`
definition rule, and the write-the-index-row-after rule — six or seven sections,
and at least two *changed what it did* rather than confirming it. The
`--include`-list-is-a-hypothesis warning is why it re-ran a citation sweep with
`include_hidden=true` after a clean zero; the definition rule is why it checked a
heading was `## W-71 — ` rather than merely present.

**Why this matters more than either number.** A single high-delivery /
low-utilisation measurement has no resolving power between two hypotheses:
*the guide is too big*, and *the guide is delivered without regard to whether this
session needs it*. Both predict 63% delivered and five sections used. Only a
second arm separates them — and the second arm shows near-full utilisation under
the identical delivery rule. So the size is not the defect. **The absence of
targeting is.**

One corpus, one delivery rule, two sessions, opposite outcomes. That is an
argument for **addressing**, and specifically an argument against shrinking as a
standalone remedy — see the risk now recorded under (b).

(Method note: this is `claude-plugins:W-4`'s shape — a measurement that returns
the same value under every hypothesis reads exactly like a measurement that
settled something. The original text stated the delivered-vs-used limit honestly
but still led with the byte count, which is the interpretation that limit does not
support.)

## The delivery census exists — 91 sessions, and it corrects both anecdotes

Added 2026-08-27. A peer went looking for the per-session record that would fix
the sampling defect and reported it absent from three locations. It is not absent;
the search had the wrong predicate. `.gitignore:49` names `.codescout/guide_hints/`,
but `src/server.rs:438-439` resolves the real path from the XDG **state** dir:

```rust
let guide_hints_dir = env.guide_hints_dir.clone().or_else(|| {
    crate::util::fs::per_user_state_dir().map(|d| d.join("codescout").join("guide_hints"))
});
```

`~/.local/state/codescout/guide_hints/` — not `~/.cache`, not `~/.local/share`.
**91 session ledgers, 2026-08-18 to 2026-08-27.** The keyed tier is live and the
`workspace-state` guide's persistence claim is accurate on this machine.

### What 91 sessions say about delivery

| topic | sessions | % | bytes | × sessions |
|---|---|---|---|---|
| `project-activation-bootstrap` | 89 | 98% | 2,594 | 230,866 |
| `symbol-navigation` | 56 | 62% | 3,145 | 176,120 |
| `tracker-conventions` | 41 | 45% | 34,333 | **1,407,653** |
| `progressive-disclosure` | 41 | 45% | 5,669 | 232,429 |
| `librarian` | 41 | 45% | 20,545 | 842,345 |
| `workspace-state` | 13 | 14% | 10,355 | 134,615 |
| `error-handling` | 1 | 1% | 1,857 | 1,857 |
| `iron-laws-detail` | **0** | 0% | 11,238 | 0 |
| `librarian-runtime` | **0** | 0% | 9,774 | 0 |
| `untrusted-content` | **0** | 0% | 5,317 | 0 |

≈ **3.0 MB of guide text auto-delivered across 91 sessions.**
`tracker-conventions` is **46.5%** of every guide byte ever auto-delivered here;
with `librarian` it is **74.4%**.

### Three findings the anecdotes could not reach

**1. Three topics have never auto-injected — 26,329 bytes, 25% of the corpus.**
And the most telling of them is `librarian-runtime`, the topic `librarian.md:407`
split out by hand *specifically to keep the parent lean*. That split moved 9,774
bytes behind an edge, and in 91 sessions **nothing has ever followed it.** The
hand-split did not redistribute the cost; it made a quarter of the guide
unreachable by the delivery path while leaving the parent's 20 KB intact. This is
direction (b)'s outcome, already run as an experiment, now with a result.

**2. Both anecdotes in this issue are atypical, in the same direction.** Topics
per session: median **2** (38 of 91 sessions received exactly two). The filing
session received 5 — top 24%. The counterexample session received 6 — top 11%.
**Two sessions that found each other interesting were both guide-heavy outliers**,
which is exactly the selection defect `Not yet done` warned about, now measured
rather than suspected. Neither arm was representative; the disagreement between
them was real but it was a disagreement between two tails.

**3. The 63% headline was itself unrepresentative.** The typical session receives
two topics, most often `project-activation-bootstrap` (2.6 KB) plus one other.
The median session's guide burden is small. The cost is concentrated, not diffuse
— which sharpens the case for targeting `tracker-conventions` and `librarian`
specifically, and weakens any argument about the corpus as a whole.

### What this census is NOT

**It measures delivery, never use.** The delivered-is-not-used limit stands
entirely; nothing here says whether any of those 3.0 MB were read. It resolves the
sampling defect for the delivery half only.

And it is a floor, not a history. The ledger is live dedup state with **five**
deletion paths, not an audit log: `persist()` **deletes the file** when the map
empties (deliberately — an empty ledger lacks `SESSION_OPENING_GUIDE`, so the
opener re-fires), `gc()` prunes ledgers idle past `GC_MAX_IDLE_DAYS = 35`,
`clear()` and `rekey()` forget every topic, and `expire_idle()` drops topics past
TTL. So 91 is *sessions whose ledger survived*, biased toward the recent and the
non-empty. Real counts are higher and the true denominator is unrecoverable.

## No topic has ever arrived by citation — which decides the proposal

Added 2026-08-27, from the same peer, verified independently against the ledgers.

### The bundle (confirmed, exact)

`tracker-conventions` and `librarian` are not two costs. Measured over 91:
**38 sessions have both**, 3 have `tracker-conventions` alone, 3 have `librarian`
alone, 47 have neither. By comparison `progressive-disclosure` differs from
`librarian` in 24 sessions, so it is genuinely independent. They arrive by
*different* calls — one session had `artifact(find)` pull `tracker-conventions`
and `artifact_event` pull `librarian` — which is why targeting either alone leaves
the other arriving whole. **54,878 bytes is the real atom for a session that
touches the artifact family.**

### The delivery observation

`error-handling` is 1 of 91. It is the most heavily *pointed-at* topic in the
corpus:

- `server_instructions` — in every main agent's system prompt: *"error-handling —
  RecoverableError vs anyhow::bail"*
- `CLAUDE.md:241` — *"Full error decision tree: `get_guide("error-handling")`"*
- **three guides cite it, two of which auto-inject**: `progressive-disclosure:116`
  (delivered to 41 sessions), `workspace-state:158` (13), `iron-laws-detail:228` (0)
- eight plans/specs and a tracker cite it

So in at least 41 sessions the pointer was *already in context*, inside a body the
substrate had just delivered. One session in 91 fetched it.

`iron-laws-detail` is the same shape at 0 — advertised in `server_instructions`,
applies to every session using these tools, never fetched.

### What this does and does not establish

**Name the confound, because it is real and it is the same shape this issue keeps
catching.** A low count is equally consistent with *citations are not followed*
and with *most sessions did not need that topic*. `error-handling` is about this
repo's Rust conventions, and an unknown share of the 91 are sessions in other
repos with no reason to want it. Need is unmeasured, varies by topic, and the
ledger cannot see it. **So "agents ignore citations" is NOT established here**, and
anyone quoting the 1/91 as proof of that is doing what the 63% headline did.

What *is* established needs no assumption about need, because it is a statement
about observed paths rather than about intent:

> **Every topic that reaches sessions reaches them by auto-injection. No topic has
> ever reached sessions by citation.** Six topics auto-inject and land in 13-89
> sessions each. Four do not auto-inject and land in 0, 0, 0 and 1. The split is
> bimodal with a 13× gap and nothing in between.

Whether the citation path fails because unneeded or because unfollowed, **it has
not demonstrated delivery in 91 sessions**, and no proposal should assume it will.

### Consequence — direction (a) is reframed, (c) is promoted

This is the row that answers any proposal ending *"and agents will call the
sections they need."* Rewriting the directions accordingly:

- **(a) section addressing is still worth doing, but NOT as a delivery fix.** Its
  value is precision *for the auto-inject path* — letting the substrate deliver
  the right section — not enabling callers to fetch what they need. Framed the
  latter way it inherits the 0/0/0/1 track record. The three `§` citations
  resolving is a legibility win, not a delivery win.
- **(c) declaring the edges is the load-bearing one**, and it must be traversed
  **by the substrate, not by the model**. An edge only a reader can follow is
  exactly what `librarian.md:407` already is, and that edge has been followed 0
  times in 91 sessions. Edges have to be something the auto-inject path walks.
- **(b) remains contraindicated**, now for a second independent reason: splitting
  produces edges, and edges are not a delivery mechanism here.

## Phase 1 SHIPPED — 2026-08-27, branch `sdd/get-guide-section-grain`

The proposal this file frames is implemented for one topic. Spec:
`docs/superpowers/specs/2026-08-27-get-guide-section-grain-design.md`. Plan:
`docs/superpowers/plans/2026-08-27-get-guide-section-grain.md`. 25 commits, ten tasks, each
reviewed; final whole-branch review clean.

**What shipped.** The section is now the unit of delivery, selected by declarations the
guide markdown carries: `<!-- serves: artifact.append_entry -->` under a heading declares
which call shapes it answers, `<!-- requires: … -->` pulls in a section whose context it
depends on. `Tool::selector_key(&input)` projects a call shape *before* `call()` consumes
the input — the plumbing change that makes intent visible at all, and which this file's
§ *Symptom* could only work around by reading the response. `GuideLedger` dedups at
`topic#heading`. An unmatched shape gets the topic preamble plus a `get_guide` pointer,
never the whole topic and never silence. Five build gates keep the corpus honest.

**Result, against the spec's falsifiable prediction:**

| | predicted | measured |
|---|---|---|
| `librarian` p50 draw | ~10,000 B | **11,946 B** |
| direction | down | ✅ down, **42%** from 20,545 B |
| injection count | up | ✅ up — 6 slices where there was 1 dump |

**The magnitude prediction missed by ~20%** and is recorded as a miss. Direction and count
landed. A committed 12,000 B ceiling gates it, with a **54 B margin** — the corpus is at
capacity, and repairing the reachability gate is what revealed that (see below).

**Phase 1 containment held.** Only `librarian` declares; the other nine topics are
byte-identical, proven by full-`format!`-string equality rather than a `contains()`.

**What this does NOT close.** The 44.4% `contradicted` rate — guidance that arrives and is
then violated — is untouched, and was declared out of scope in the spec. That is an
enforcement problem, not a grain problem. Phases 2 (`tracker-conventions`, blocked on
decomposing a 17,378 B section) and 3 (the remaining eight topics, including the four with
zero auto-injections) are unstarted. **Status stays `open`.**

**One finding worth carrying forward.** Fixing a reachability gate that could not fail
exposed two sections no call shape can reach — and making either reachable was
*arithmetically impossible* at a 54 B margin. The broken gate had been concealing that the
corpus was already at capacity. Both were waived with reasons naming the byte constraint
and the remedy rather than narrowing more guide content; the narrowing pass had already
spent a concrete example out of the guide to buy 385 B. Recorded in
`docs/trackers/sdd-ruling-log.md`.

## Phase 1.5 — 2026-08-31: two defects in the shipped mechanism, and a second blocker on Phase 2

Phase 1 shipped section-grain delivery for `librarian`. Using it in anger found two defects
**in the mechanism itself**, both fixed the same day, and one of them adds a precondition to
Phase 2 that did not exist when this file was written.

**Defect 1 — a declared section can be unreachable, because the TOPIC is chosen first.**
`serves:` selects sections *within* a topic. The topic is selected earlier and separately by
`Tool::relevant_guide_topic`, a heuristic over the RESULT, and nothing reconciled the two. So
`librarian.md` § *doctor repairs* declared `librarian.doctor`, matched its selector, passed
every gate — and could not be delivered at all, because every `doctor` scan of a real catalog
names tracker paths and routed the call to `tracker-conventions`. Measured live: **39,106 B of
the wrong topic displacing the 1,490 B written for that call — a 26x overshoot**, with the
declaring topic never consulted. Two of Phase 1's own tests were already routing *around* this,
putting fixtures under `docs/specs/` with the comment that a `docs/trackers/` path "would starve
both calls of a section-grain hint".

Fixed at `50590b6c` (patch-id `13b643673b57831244a5ed63a5dce0bf1d43a965`) as a **fallthrough**,
not a precedence flip: the result heuristic still goes first, because it encodes what the call
*touched*, which no unqualified `serves:` shape expresses. "Declaration beats content" was
implemented as an experiment and fails
`an_artifact_call_naming_a_tracker_path_delivers_the_tracker_guide`, reverting `32736ca0`. Full
record: `docs/issues/archive/2026-08-31-a-served-section-can-be-unreachable-via-topic-routing.md`.

**Defect 2 — the ledger charged calls that delivered nothing.** `guide_blocks_for` used
`GuideLedger::insert` as its already-sent *test*, and `insert` refreshes the stamp and persists
on repeats. An all-sections-already-sent call therefore wrote once per matched section and
returned empty. Fixed at `8364e472` with `contains`-then-`insert`. The harms split by tier and
neither tier paid both: identified sessions carry `idle_ttl: None`, so their stamps are never
read for expiry; anonymous sessions carry `path: None`, so `persist` returns early.

### Phase 2 now has a SECOND blocker, independent of the 17,378 B one

The fix introduced `GuideIndex::topic_declaring`, a corpus-wide lookup from call shape to
declaring topic. That makes `serves:` a **cross-topic** namespace for the first time.
`librarian.md` declares nearly every librarian and artifact shape, so the moment
`tracker-conventions` declares `artifact.create` or `artifact.append_entry` the two collide, and
`topic_declaring` would resolve it by `BTreeMap` order — alphabetically, silently.
`no_two_topics_declare_an_overlapping_shape` fails the build on exactly that, deliberately, and
will fire on the first naive Phase 2 attempt.

So Phase 2 is blocked on two independent things: **decomposing the 17,378 B section** (size),
and **cross-topic shape disambiguation** (addressing). This section adds the second.

The resolution is already expressible, and may be worth more than the unblocking. `Shape`
carries a `path_contains` field, so `serves: artifact.create path~docs/trackers/` states
declaratively, at section grain, exactly what `names_tracker_path` states imperatively at topic
grain. Two things must land with it: a **specificity rule** in `topic_declaring` (a
`path~`-qualified shape beats an unqualified one — the same "more specific wins" principle the
fallthrough already encodes), and widening the ambiguity gate to model `path_contains`, which
today over-approximates and would flag that pair. Done together, Phase 2 could **retire**
`names_tracker_path` rather than work around it — which would also close Defect 1's residual.

**Residual, unchanged by the fix.** A session's *first* tracker-path-naming librarian call still
ships `tracker-conventions` whole. Reachability was fixed; cost was not. And **9 of the 10
topics still declare nothing** — only `librarian.md` carries `serves:`, 13 declarations — so any
call routing to one of the nine pays its full body. Phase 1's containment property and the shape
of what is left to do are the same fact.
## The DISTRIBUTION probe ran — 2026-08-31, n=166 sessions, two machines

Directions (b) and (c) were gated on this, because the sessions measured before were
selected by talking to each other. It has now run under a stated rule, on a second
observer, and **it changes the recommendation**.

Instrument: `scripts/probe_guide_section_use.py` (PROBES.md row names its blind spots).
It counts **section-attributable mechanism activity after delivery** — `artifact*` /
`librarian*` calls whose *structural* input matches what a section governs. That is
**relevance, not causation**, and it is asymmetric on purpose: a section with zero
activity certainly was not used, while a section with activity merely might have been.
**Every conclusion below runs in the zero direction.**

### The 2026-08-27 frame could not be re-read, and that is a finding

**76 of its 106 `tracker-conventions` sessions were gone from disk within four days**, and
the attrition is severely structured: 4.8% survival in `.claude` against 71.1% in
`.claude-sdd`, with the guide-heavy tail preferentially dead (max 21 injections among the
gone, max 8 among survivors). **9 of the 10 scored transcripts are gone**, and the
survivor received zero injections of this topic — so the calibration overlap is empty and
PROBES rule 1 cannot be satisfied against that study. Drawing from the survivors would
have run cleanly and returned a ~90%-one-profile convenience sample. This is a fresh
frame reported as its own period, never a continuation.

### Result — main sessions (n=79 across both machines)

| section | bytes | % of topic | laptop | desktop | **both** |
|---|---|---|---|---|---|
| Entry-level standard | 17,323 | 44.6% | 56% | 61% | **59%** |
| Bug files | 11,319 | 29.1% | 56% | 61% | **59%** |
| Declaring an augmentation | 4,098 | 10.5% | 33% | 21% | **24%** |
| Cross-linking | 2,086 | 5.4% | 39% | 48% | **46%** |
| **Querying with the librarian** | **1,965** | **5.1%** | **89%** | **82%** | **84%** |
| Tracker artifacts | 1,416 | 3.6% | 50% | 49% | **49%** |

**Never engaged: 45.2% of delivered bytes** (laptop 47.1%, desktop 44.6%). Median session
engages 4 of 6 sections; 9 of 79 engage none.

**The size ranking and the use ranking are inverted.** The most-engaged section is the
second-smallest — 1,965 B, 5.1% of the topic, engaged by 84% — while the two giants
carrying 73.7% of the bytes are engaged by 59%. Any decomposition argument reasoning from
size alone would target the wrong section first.

### Result — subagents (n=87) are a different population, and the biggest lever

**92.5% of delivered bytes never engaged** (laptop 90.1%, desktop 93.5%). Median 1 of 6
sections; **38 of 87 engage nothing at all**; `Cross-linking` is engaged by zero.

Subagents are **half the population** and their waste is **2.3× the main-session waste**
— 3.13 MB of 4.52 MB total across this corpus. **And that lever is orthogonal to both
Phase 2 blockers**: not injecting (or minimally injecting) `tracker-conventions` into
subagent sessions needs no decomposition and no cross-topic addressing. Iron Law 6
already assigns briefing to the parent, so the guidance path exists.

### What it decides

1. **Do the subagent lever first.** Largest saving, no blocker, no new mechanism.
2. **`Querying with the librarian` is the section worth serving at section grain** — 84%
   engagement for 5.1% of the bytes. And it is exactly the collision case: verified live
   this session, `librarian.md` declares `<!-- serves: artifact.find -->` and
   `<!-- serves: artifact.get, artifact.create -->`, which are the very shapes that
   section governs. So the highest-value section to serve is the one that most needs the
   `path~` specificity rule — the two findings meet on the same fix.
3. **Decomposing `Entry-level standard` is NOT the first move.** It is the biggest
   section, but at 59% engagement it is neither the least-used nor the best return.

### What this is not

Section grain still flatters — the 08-27 study measured ~0.06 utilisation at *subsection*
grain against 0.469 at section grain, and nothing here reaches inside a section. A 17 KB
section scored "engaged" on one `append_entry` call is credited whole. So **45.2% is a
floor on main-session waste**, not an estimate of it.

And a near-miss worth recording: blending the two populations gives 71.7% never-engaged,
which matches the 08-27 study's 71% for this topic almost exactly and reads as a striking
cross-instrument convergence. It is not one — split, the populations are 45% and 92%, and
the agreement is an artifact of the mix. The blend was checked only because the project
breakdown looked odd.
### The headline is a BLEND across a regime change — stratify before quoting it

Checked because the sample spans our own refactors: section-grain delivery shipped
**2026-08-27** and the routing/ledger fixes landed **2026-08-31**, so these sessions were
injected by at least three mechanism versions. Dating them by the transcript's own first
`timestamp` (never mtime — a copied corpus need not preserve it): **140 of 166 sessions,
84%, predate the section-grain ship**, and only 2 are from today.

Main sessions, split at 2026-08-27:

| slice | n | never engaged |
|---|---|---|
| pre-2026-08-27 | 58 | 49.0% |
| post-2026-08-27 | 21 | **34.4%** |

That looks like the mechanism working. **It is not.** Holding project constant:

| stratum | pre | post |
|---|---|---|
| codescout-project sessions | 28.0% (n=22) | **27.5%** (n=12) |
| non-codescout sessions | 61.9% (n=36) | 43.7% (n=9) |

The codescout stratum is **flat**; what moved was the *mix* — codescout's share of main
sessions went **38% → 57%**. A 15-point aggregate shift over a 0.5-point within-stratum
one: Simpson's paradox, and quoting either single number without the other misleads.

**The mechanical prediction agrees, which is what makes this more than a caveat.**
`tracker-conventions` declares **no** `serves:` sections — only `librarian.md` carries them
— so section-grain delivery never changed *this topic's* delivery in any regime. It has
always arrived whole. Prediction from the code and measurement from the data now say the
same thing, and that is stronger evidence than either alone.

**What the check actually bought: a confound that matters more than the one it tested
for.** Project drives far more variance than time. Waste is ~28% in codescout-development
sessions and ~44–62% elsewhere — the guide is most wasted exactly where the librarian is
least used. That is a third lever, independent of both Phase 2 blockers and of the
subagent one, and it says any evaluation of a decomposition must be stratified by project
or it will measure our own workload back at us.

`--split-at` is now in the instrument and prints this stratification warning with the
numbers, so the next reader cannot take a period difference for a regime effect.
## Measured — USE, 2026-08-27 (the probe `Not yet done` asked for)

This section answers the question the rest of the file could only frame: delivery
was measured, use never was. **n = 81 injections across 10 sessions**, plus a
corpus-scale delivery census at n = 1,705 unique sessions.

Raw data, rubric and per-session results:
`docs/evals/data/2026-08-27-guide-injection/` (10 agent result JSONs,
`corpus-frame.json`, `truebytes.json`, `rubric-BRIEF.md`).
Reusable instrument: `scripts/probe_guide_injection.py` (PROBES.md row names its
blind spots).

### Was it used

| class | n | share |
|---|---|---|
| `U0_UNUSED` | 54 | **66.7%** |
| `U2_PRESCRIBED_CALL` | 20 | 24.7% |
| `U3_CITED` | 7 | 8.6% |
| `U1_ECHO` | 0 | 0.0% |
| **`contradicted`** | 36 | **44.4%** |

`contradicted` = the session violated a rule of the guide that had just arrived.
Specific, not impressionistic: `pytest … | tail -N` ten times after
`progressive-disclosure`; native `Read("@tool_…")` — that guide's own named
anti-pattern — seven turns after it landed; hand-written `| F-1 |` row tables 114
turns after `librarian`'s *"don't hand-maintain the table"* section.

### How much of it was used

**15.5% at section grain, and that is an UPPER BOUND. Median per-injection
utilisation is 0.0%.**

| topic | injected | in touched sections | **never touched** | util |
|---|---|---|---|---|
| `tracker-conventions` | 274,664 B | 80,211 B | **71%** | 29.2% |
| `librarian` | 205,450 B | 11,429 B | **94%** | 5.6% |
| `project-activation-bootstrap` | 108,948 B | — | — | 8.3% |
| `progressive-disclosure` | 85,035 B | — | — | 8.4% |
| `symbol-navigation` | 12,580 B | 1,032 B | 92% | 8.2% |

Section grain flatters badly. Two agents quantified it independently: a 10 KB
`Bug files` section credited on ~15 lines of contact with six of seven sections
never touched; and one clean win scoring 0.469 at section grain and **~0.06 at
subsection grain**. True utilisation is low single digits.

### Was it at the right time

- **89% genuinely late**, median **320 turns** after the session's first contact
  with the class that guide governs.
- **51% `TRIGGER_ONLY`** — the governed class never recurs after the injection.
  The guide arrives for a call already made.
- Only **11%** arrived at first contact.

(The literal `LATE` verdict in the raw results is degenerate — the trigger's
`tool_use` precedes its `tool_result` by one turn, so the inequality is true by
construction. The figures above use the corrected rule
`gap = turn_index − first_opportunity_turn`, late iff `gap > 1`.)

### Share of context

Median **14.0%** of a session's readable content is auto-injected guide text
(range 5.9% – 25.2%).

### Delivery at corpus scale (n = 1,705 unique sessions)

Today's clean figure — post-fix, no `/mcp`, no compaction, n=33:
**31,899 B per session, median 3 injections, 8% duplicate.**

- Duplication is largely SOLVED: 80% of delivered bytes were repeats pre-fix,
  **8% post-fix**. The dominant mechanism was the known, fixed
  `workspace(activate)` ledger clear
  (`docs/issues/archive/2026-08-19-mcp-reconnect-leaves-rendezvous-inactive-so-activate-clears-the-ledger.md`).
  Sessions running the `cargo rb` → `/mcp` rebuild loop are 34% of main sessions
  but carried 81% of all delivered bytes — a development artifact, excluded.
- **Four topics have 0 auto-injections across 1,705 sessions** —
  `iron-laws-detail`, `librarian-runtime`, `untrusted-content`, `error-handling`:
  **28,186 B, 27% of the corpus**, compiled in and never pushed. (Predicate counts
  auto-injections only, not explicit `get_guide()` fetches — so this agrees with
  the 91-ledger census's `error-handling` 1/91, which was a fetch.)
- Largest single trigger pair: `project-activation-bootstrap` ← `workspace`, 1,722.

### Limits — none of these are hedges, all were measured

1. **Thinking text was unavailable for this study's transcripts — but that was a
   fixable defect, not model behaviour, and it is now fixed.** All 10 sampled
   transcripts store `thinking` signature-only (1,185 blocks, all zero-length), and
   Langfuse showed the same at the time (150 observations, 39 thinking blocks, 0
   non-empty). **Cause, found 2026-08-27 (`llm-proxy:6f3cb62`): two request-side
   settings, either sufficient alone** — Claude Code sends `anthropic-beta:
   redact-thinking-2026-02-12` (a client-side terminal-UI choice, not an Anthropic
   restriction), and `thinking: {"type": "adaptive"}` with no `display` key, which
   several current models default to `"omitted"`. With the beta stripped and
   `display=summarized` set, live traces carry readable thinking on both
   `claude-opus-5` (mean 1,188 chars) and `claude-sonnet-5` (mean 316), **and CC's
   own JSONL began carrying it within minutes** — so the transcript redaction was
   downstream of the same cause.

   **For these results:** `U1_ECHO` and `U3_CITED` are floors, so **66.7%
   `U0_UNUSED` is an UPPER BOUND on non-use**. `U2_PRESCRIBED_CALL` and
   `contradicted` read tool calls and are unaffected — which is why the two
   decision-relevant numbers rest entirely on behaviour.

   **For a re-run:** thinking is now visible in both instruments, so a repeat of
   this study can measure what this one could not. That is the single highest-value
   change to the method, and it costs nothing but re-running it.

   *(An earlier version of this section concluded the absence was irreducible
   Anthropic behaviour. That was wrong. The error: a proxy has a request side and a
   response side; only the response side was verified, and the whole component was
   then treated as excluded. Recorded as `prompt-surface-measurement-session-log:F-37`.)*

2. Section-grain utilisation overstates. Two independent subsection-grain estimates
   put the real figure at low single digits.

3. Half the sampled sessions predate the 2026-08-19 ledger fix. Post-fix the
   contradiction rate falls 76% → 12% and utilisation rises 5.8% → 20.1%.

4. n = 10 sessions, deterministic hash draw from 128 eligible main sessions; the
   draw's topic mix was checked against the population's before any analysis.

5. One session (OPUS-1) is confounded — it edited `tracker-conventions.md` and
   rebuilt the binary mid-session, so two of its citations are
   guide-as-rebuild-evidence rather than guidance.
### What this decides

- **(b) splitting stays contraindicated, now for a third reason.** 51%
  `TRIGGER_ONLY` means most injections have no follow-on call in which a caller
  could request a missing fragment.
- **More bytes cannot fix compliance.** 44% contradicted is delivery landing and
  being violated anyway.
- **`librarian` is the pilot target**: 5.6% utilisation, 94% of bytes never
  touched, and in normal sessions it arrives ALONE in 35 of 93 versus bundled with
  `tracker-conventions` in 18 — so it can be addressed without touching the other.
  (This qualifies § *The bundle (confirmed, exact)* above, whose 38/91 co-occurrence
  was measured on a ledger population dominated by dev sessions.)
- **`symbol-navigation` is the existence proof to generalise.** 3,145 B landed on
  a turn-29 result; the agent's very next symbols call adopted the guide's verbatim
  `name_path=` form — absent from the prior 30 turns — and held it for 15 of the
  remaining 21 calls. Small and targeted is adopted immediately; large and general
  is not.
## Root cause

Bodies are `include_str!`'d and dispatched by a hardcoded match on topic name:

- `src/prompts/mod.rs:503-513` — one arm per topic in `topic_body`
- `src/server.rs:1486` — `librarian.md` embedded a second time for `static_doc_sources`
- `src/prompts/mod.rs:1629` — the test `guide_topics_have_bodies` enforces the arm

So the topic name is simultaneously the file name, the cache key, the API surface,
and the only addressable unit. Adding a topic means editing Rust and rebuilding;
splitting one means renaming the API. Consequences worth stating plainly:

- **Granularity is frozen at whatever the file happens to contain.** `tracker-conventions`
  is really about six topics (bug files, tracker frontmatter, ledger declaration,
  entry ids, citations, compaction/archival) that grew into one file.
- **`R-89` applies twice.** Guides are fixed at build time and again at process
  start, so a long-lived MCP session serves a stale body after any rebuild.
- **Auto-inject is all-or-nothing per topic**, and it is the dominant delivery
  path — the 66 KB above arrived unbidden, not through explicit `get_guide` calls.

## Proposal — model the guides as a graph

This mirrors work happening on the `claude-plugins` side for buddy specialists,
where the same shape was found: a real schema, a latent edge set, and composition
expressed only in prose. That design settled on **primary + advisors** — one node
owns the voice and output contract, others contribute subordinate sections via a
projection rule — deliberately generalising the one composition primitive that
already worked (`_<lens>.md` addenda) rather than inventing merge semantics.

**That design explicitly scoped guides OUT as reference-only**, on the grounds
that they are compiled into this binary and served atomically. The buddy graph can
route *to* a guide; it cannot slice one. This issue is the codescout half, and it
is a prerequisite for the guide corpus ever participating in that composition.

Three directions, in increasing cost. They are not exclusive — (a) is a
prerequisite for the others and worth doing alone.

### (a) Make sections addressable — no restructuring

Add an optional `section` parameter: `get_guide(topic, section="Entry ids")`,
resolving against the body's headings the way `read_markdown` already does for
files. Nothing moves; the match arm stays; the three section-qualified citations
above start resolving. Cheapest real progress, and it converts the prose `§` into
something a caller can act on.

Open question: whether the auto-inject path can pick a section, which requires a
trigger to say what it is *about*, not merely that it fired.

### (b) Split the oversized topics

`tracker-conventions` (34 KB) and `librarian` (20 KB) are 52% of the corpus
between them. Splitting them into the topics they already contain shrinks the
atom without changing the mechanism.

**Do not do this before (a), and possibly not at all.** The counterexample above
is a session that used six or seven sections of `tracker-conventions` in one
sitting. Auto-inject fires on *the first call that touches a topic* — so after a
split, that session receives whichever fragment its first `artifact()` call maps
to, and must then know that five more exist and request them by name. **Splitting
without addressing does not reduce cost; it moves the cost onto the caller and
converts a silent over-delivery into a silent under-delivery**, which is the worse
failure because nothing in the transcript shows what was missing.

The precedent is already in the corpus and it is not encouraging.
`librarian.md:407` ends by routing to `get_guide("librarian-runtime")` — an
operational reference split out by hand, explicitly to keep the parent lean. That
split already happened, and what it produced was **an edge nothing reads**: a
reader who needs both now needs two calls and has to know to make the second. That
is the outcome (b) generalises, absent (c).

Remaining costs if it is done anyway: more match arms, and **every existing
citation of the old topic name breaks** — including shipped prompt surface and
downstream repos' skills. Sequence it after (a) so section addressing serves as
the compatibility shim.
### (c) Declare the edges

Give each guide frontmatter naming its `requires` / `see-also` / `supersedes`
edges, and have the 18 prose citations derive from that rather than restate it.
Enables: "pull this topic and its prerequisites", a lint for dangling topic
references, and a graph a router could traverse. This is the piece that makes the
corpus composable rather than merely sliceable.

## Explicitly NOT proposed

- **Moving bodies out of the binary.** Considered and rejected for now on the
  claude-plugins side: `include_str!` is what makes a guide always present with no
  install step, and that property is worth more than rebuild-free editing. The
  staleness it causes is `R-89`'s problem, not this one's.
- **A resolver that assembles guides into one payload.** The buddy design chose
  no-resolver on YAGNI grounds — load the pieces, let the model compose. Same
  reasoning applies here until something proves it insufficient.

## Not yet done

The 63% figure measures what was **delivered**, not what was **used** — stated as
a limit when this was filed, and now partly answered from the other direction by
the counterexample above, which supplies a high-utilisation arm under the same
delivery rule. Together they establish that utilisation *varies by session*, which
is precisely the case for targeting.

**RESOLVED 2026-08-27** — the probe ran; see § *Measured — USE, 2026-08-27* above.
The sampling concern below was addressed by a deterministic hash draw from a
stated eligibility filter, with the draw's topic mix checked against the
population's before analysis. The "`tracker-conventions` is really six topics"
premise remains an authoring judgement, but is now bounded by measurement: six of
its seven sections were never touched in the one session that used it most.

What was still unmeasured when this was written — the **distribution**: two sessions is two points, and
they were selected by being the two that happened to be talking to each other, not
by any sampling rule. Neither is evidence about the typical session. The probe
worth running before (b) or (c) is which sections of `tracker-conventions` are
cited back or acted on across a real sample — and note that the two arms here
would both survive a bad sampling design, so the sample is the thing to get right.

"`tracker-conventions` is really six topics" remains an authoring judgement, not a
measurement. It is the kind of premise that reads as settled because it is stated
by someone who knows the file, and it deserves re-costing before anyone builds on
it.
