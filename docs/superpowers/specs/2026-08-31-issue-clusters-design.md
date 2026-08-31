---
id: '9d2098275804a77d'
kind: spec
status: draft
title: Issue clusters — declare a bug's defect class so architectural problems are queryable, and promote a cluster into a rule
owners:
- marius
tags:
- issues
- clusters
- defect-classes
- taxonomy
- promotion
- mineable
topic: issue clusters and rule promotion
---

## Summary

Give every bug file a **declared defect class**, so the question *"what architectural problem
do these thirty bugs share?"* becomes a query instead of a re-reading.

Three layers, in increasing cost:

- **Carrier** — a reserved `cluster/<slug>` tag in bug frontmatter. Queryable today, spans
  `docs/issues/archive/`, no code change.
- **Registry** — `docs/trackers/issue-clusters.md`, a declared `IC` ledger. One section per
  cluster holding the claim, the member **query**, the promotion target and its status.
  **It never lists members.**
- **Gate** — a `#[test]` that fails when a bug file declares no cluster, so coverage does not
  depend on anyone remembering.

The registry is where a cluster becomes a **rule**, and this spec pins what a rule must carry
and which existing surface it promotes to. No new rule surface is created: `OB`, `OP`, `H`,
`DC` and `CLAUDE.md` already cover the space.


## Why now — the measured problem

Measured 2026-08-31 over `docs/issues/` at `14997d36`, 32 open files and 494 archived:

| quantity | value | how |
|---|---|---:|
| open bug files | **32** | 30 at session start; peer sessions added two mid-analysis |
| mechanism clusters they fall into | 9 | read-through, this session |
| open files in the two largest clusters | **16 (50%)** | `IC-1` (10) + `IC-2` (6) |
| open files whose tags declare a defect **class** | **0** | every tag is topic-shaped (`windows`, `ci`, `flake`) |
| open files with a genuinely empty `tags:` block | 0 | 20 use YAML block sequence, 11 inline flow |
| open files declaring no `severity` | 4 | and one uses `med` where the vocabulary says `medium` |
| open files with a non-empty `related:` | 3 | two of the three point into `archive/` |
| open files naming any `OB` class | **0** | `grep -lE '\bOB-[0-9]+\b' docs/issues/2026-*.md` |
| open files cited *from* the `OB` ledger | 2 | `grep -n 'docs/issues/' docs/trackers/observer-blindness.md` |

Rows four and five are the carrier finding, and they are not the ones this spec first claimed.
An earlier draft asserted that 20 of 30 files had **empty** tags; that was a measurement error
— `grep -l '^tags:$'` matches the header line of a *populated* block sequence, so two thirds of
a fully-tagged corpus read as untagged. The corrected finding is stronger: **the carrier is
full, and carries no class information at all.** Every one of the ~150 tags in the open corpus
names a topic or a component. None names a mechanism. So the problem was never that authors
fail to tag — they tag reliably — it is that nothing asks them for the one field that would
make the corpus mineable, and topic tags cannot be retrofitted into classes because
`concurrency` spans three of the nine classes.

Rows eight and nine are the edge finding. `docs/TAXONOMY.md` states the relationship outright —
*"An instance is a bug file, an `F-N` or an `R-N`; only the class is an `OB`"* — and the corpus
materialises it in neither direction. So *"which live bugs are instances of the class `OB-3`
names?"* is unanswerable today, which is precisely the question a class ledger exists to answer.

The corpus also keeps discovering its own clusters and discarding the discovery. Three open
files carry a sentence of the form *"this is the third instance today of one mechanism"* —
`2026-08-30-cli-doctor-exposes-no-fix-flag.md:21`,
`2026-08-26-workspace-read-only-flips-mid-session.md:309`,
`2026-08-30-core-hookspath-points-at-pre-rename-path.md:68`. Each author counted correctly and
had nowhere to put the count.

**One measurement note, because it is evidence for `IC-1` rather than an aside.** The corpus
grew from 30 to 32 files during this analysis, written by concurrent sessions in the same
checkout, with no signal to the measuring session that it had changed — and a peer committed
one of this work's own tag edits before it could be staged, under an accurate message. Any
count in this table is therefore a fact about an instant, which is the same property that
makes a member *list* unusable and a member *query* the design.
## What already exists — do not rebuild it

Verified live this session, not read from documentation:

- **`tags` filtering works and reaches the archive.** `artifact(find, kind="bug",
  filter={"tags":{"in":["concurrency"]}})` and the `contains` form both return **14** rows,
  spanning `docs/issues/` and `docs/issues/archive/`. `BL-47` — *`tags.in` returns zero while
  `tags.contains` finds the same row* — is fixed. This is the whole carrier; nothing needs
  building.
- **`BL-N`** (`docs/trackers/open-issue-work-queue.md`, artifact `9a892c2a5976e296`) is a
  **sequencing** layer — readiness phases, blockers, "what to pick up next". A different axis
  from mechanism, and this design does not touch it.
- **`OB-N`** (`docs/trackers/observer-blindness.md`, artifact `3922c2a0fd0dfcfc`) is already
  the rule surface for classes with a structurally blind party. Three of the eight clusters
  are already `OB` classes.
- **The doc-gating test pattern** — `prompt_surfaces_reference_only_real_tools` and
  `claude_md_contains_no_deprecated_tool_names` in `src/prompts/mod.rs` — is the precedent the
  Layer-3 gate copies. A `#[test]` that reads repo markdown and asserts on it is established
  here.
- **`get_guide("tracker-conventions")`** already specifies ledger declaration, the
  `## <ID> — <title>` definition rule, `entry_high_water_<PREFIX>`, and the `**Valid:**` /
  `**Rests on:**` fields. The registry is an ordinary declared ledger under those rules.


## Why the three prior attempts rotted — the principle this design turns on

Clustering has been invented in this repo three times and has not survived once:

1. **`docs/issues/INDEX.md`** — a hand-maintained index, retired 2026-05-18 when `kind: bug`
   frontmatter made the listing derivable. `docs/issues/_TEMPLATE.md` records the decision.
2. **`open-issue-work-queue.md` § *Sequencing notes*** — names an *"overflow/handle cluster"*
   (`BL-1`, `BL-2`, `BL-6`, `BL-8`) and a *"worktree cluster"* (`BL-11`, `BL-12`, `BL-16`).
   Every one of those seven rows is now `done` or `done, archived`. The prose survived; the
   corpus it described did not.
3. **`tags:`** — the carrier exists and works, and two thirds of open files leave it empty.

All three failed the same way: **they stored a member list, and a list is a fact about an
instant.** This is the defect `docs/TAXONOMY.md` documents for a bare SHA (positional, dies on
rebase) and for a bare ordinal (correct for one arrangement), and that `CLAUDE.md` § *Observer
Blindness* answers with *"for any published claim, ship its derivation rather than its value,
so a reader re-checks it instead of re-deriving it."*

**So the registry stores the query, never the answer.** A cluster section carries
`filter={"tags":{"contains":"cluster/<slug>"}}` and a dated count next to it. The count can go
stale; the query cannot. Re-running it is one call, and a reader who trusts the stale count
loses only the count.

Attempt 3 fails differently — not staleness but coverage — and that is what Layer 3 is for.
A carrier nobody is required to fill is a carrier two thirds empty.


## Relationship to the tracker-topology spec — declared vs derived

`docs/superpowers/specs/2026-08-30-tracker-grain-and-corpus-topology-design.md` (artifact
`8ba9230c8d25b35c`) Layer A finds structure by **deriving** it from the `cites` edges
`link_scan` materialises: hubs, centrality, a generated ranked tracker in the
`legibility_scan` mould. This spec finds structure by **declaring** it.

The two are complements, and the boundary is measurable rather than stylistic: **the derived
method sees only what authors linked.** Three of thirty open bug files carry a non-empty
`related:`, and two of those three point into `archive/`. A citation-graph hub metric run over
`docs/issues/` would therefore return approximately nothing — not because the corpus lacks
structure, but because bug files do not cite each other. Mechanism similarity is a judgement
made at file-open time and recorded, or it is not available at all.

Neither spec is blocked on the other, and neither should grow into the other's half.


## The design

### Layer 1 — the carrier

A reserved tag namespace in bug-file frontmatter:

```yaml
tags: [cluster/blast-radius-exceeds-visibility, concurrency, git, shared-checkout]
```

- **Exactly one `cluster/` tag per bug file.** A bug that genuinely spans two clusters names
  the one whose *mechanism* it instantiates and cites the other in prose. Multi-membership
  makes counts non-additive, and the counts are what drive promotion.
- Free-form tags stay alongside, unchanged. The `cluster/` prefix is what makes the reserved
  namespace greppable and lets the gate distinguish "no class declared" from "no tags".
- Slugs are **claim-shaped**, not topic-shaped: `blast-radius-exceeds-visibility`, never
  `concurrency`. A topic slug re-creates the tag soup this replaces; a claim slug can be
  false, which is what makes the cluster promotable.
- Valid slugs are exactly those the registry defines. The gate enforces the closed set.

### Layer 2 — the registry

`docs/trackers/issue-clusters.md`, `kind: tracker`, `entry_prefix: IC`, one
`## IC-N — <claim>` section per cluster. Prose ledger — entries are body sections, no
`entry_collection`, appended with `artifact(action="append_entry", id_prefix="IC",
anchor_heading="## Template for new entries", title=…, body=…)`.

Each section carries, and nothing else is required:

| field | content |
|---|---|
| `**Slug:**` | the `cluster/<slug>` tag value — the closed-set entry the gate checks against |
| `**Claim:**` | the mechanism, stated so it can be false |
| `**Members:**` | the **query**, plus `n=<count>` and the date it was run |
| `**Blind party:**` | who structurally cannot see it and why — or `none — ordinary design defect` |
| `**Promotes to:**` | the target surface, per the routing table below |
| `**Mechanism status:**` | `none yet` \| `designed` \| `shipped (<what>)` — borrowed verbatim from `OB` |
| `**Valid:**` | `invariant` \| `dated YYYY-MM-DD` \| `conditional — <event>` |

An **Index** table sits at the top for reading. It is hand-maintained and says so, per
`get_guide("tracker-conventions")` § *One entry format, never two* — the heading is what makes
an entry citable; the table is a reading surface only.

### Layer 3 — the gate

A `#[test]` in the doc-gate family (`src/prompts/mod.rs` is the pattern) that walks
`docs/issues/*.md` and `docs/issues/archive/*.md` and fails when a file:

- declares no `cluster/` tag, **or**
- declares more than one, **or**
- declares a `cluster/<slug>` the registry does not define.

The third condition is what keeps the namespace closed and catches a typo'd slug, which is
otherwise a silent no-op that reads as a real cluster of size one.

**Archived files are OUT of scope, and this reverses what this spec first said.** The original
reasoning was that the archive holds most of the evidence, so exempting it would derive
promotion counts from the smaller half of the corpus. The backfill falsified the premise: of
the 357 archived files dated 2026-07-01 or later, 78 were tagged and **279 were deliberately
left untagged**, because the nine classes were derived from the open backlog and forcing a fit
would corrupt the counts promotion reads. A further 137 pre-July files are untouched. A gate
demanding a tag on all of them would either red on 416 files or coerce false classifications
into the exact numbers it exists to protect.

So absence in `archive/` is a deliberate answer, not a gap, and the gate cannot tell the two
apart. Covering the archive would first need an explicit marker — a `cluster/unclassified`
slug meaning *looked, nothing fits* — which is a taxonomy decision, not a gate decision. Left
for later; the four candidate shapes the backfill surfaced may settle it.

**Tracked files only.** An untracked bug file is a peer session's in-flight work on a shared
checkout. Gating the working tree lets one session's unfinished file red another's build,
which is `IC-1` in a new costume — and was specified that way here before the distinction was
noticed.

**The gate parses frontmatter; it must never grep.** Measured 2026-08-31: a
`grep -rho "cluster/[a-z-]*" docs/issues/` over this corpus returns two slugs that do not
exist — `cluster/blast` from a truncated tool-log line and a bare `cluster/` from a bug file
discussing the convention. A grep-based gate reports both as real classes of size one.
Reproduced deliberately as mutation M2 against the shipped checker.

Backfill happens once as a data change; after that the gate makes the tag a condition of
opening a bug, and the correct path ends in a compliant state without anyone remembering. That
is the shape `CLAUDE.md` § *Observer Blindness* asks for: *"the check that runs when nobody is
worried."*


## What a promoted rule looks like

A cluster becomes a rule when it can carry five fields. Four already exist scattered across
`OB`, the `**Valid:**` convention and `CLAUDE.md`; this spec only fixes the set.

1. **The claim, stated so it can be false.** `OB`'s own heading convention — *"the class,
   stated as a claim."* Not *"concurrency problems"* but *"the blast radius of a write is
   wider than the set of peers you can see."* A rule that cannot be false cannot be retired,
   and the ledger fills with unfalsifiable advice.
2. **Evidence as a derivation.** The member query, its count, and the date it was run — so a
   reader re-runs it. Never a bare number, and never a member list.
3. **The blind party, and why they structurally cannot see it.** `CLAUDE.md`'s admission test
   decides: *"'was careless' disqualifies it; 'holds the parameter that would reveal it'
   qualifies it."* This field routes the promotion.
4. **The check that runs when nobody is worried.** A rule with no mechanism is a worklist
   item, and `**Mechanism status:** none yet` is the honest way to say so. Promotion without
   this field produces advice, which is the failure mode the whole tracker exists to avoid.
5. **`**Valid:**`** — the decay class, per `get_guide("tracker-conventions")`.

**Field 3 picks the target. No new rule surface is created:**

| the cluster… | promotes to |
|---|---|
| has a blind party **and** fails with a plausible answer rather than an error | `OB` — `docs/trackers/observer-blindness.md` |
| needs a runtime gate or hook | `H` — `docs/trackers/codescout-usage-hookify.md`, or `I` in `docs/trackers/test-escape-hardening.md` |
| holds across every project, tool and model for this operator | `OP` — `docs/trackers/operator-rules.md` |
| is codescout-specific engineering discipline | `CLAUDE.md` |
| is a written claim that decayed with no repair trigger | `DC` — `docs/trackers/claim-decay.md` |

**Threshold: three or more instances spanning two or more subsystems.** Three is the count at
which the corpus has repeatedly noticed itself (the three *"third instance"* sentences above).
The second condition is the load-bearing one: three bugs in one subsystem are a broken
subsystem and belong in a bug file, whereas three across two subsystems are a mechanism and
belong in a rule.

Promotion does not close the cluster. The `IC` section stays as the standing membership query;
`**Promotes to:**` gains the target id, and new instances keep landing under the same tag.


## Gates

- `cargo fmt`, the long clippy form, the lean test lane, then the default lane — per
  `CLAUDE.md` § *Development Commands*, in that order.
- `librarian(action="doctor")` reports no new `entry_without_definition` or
  `ledger_defines_nothing` for the `IC` ledger.
- `librarian(action="link_scan")` resolves every `IC-N` token written in this spec — they are
  citations the moment they are written, and dangle until the registry defines them.
- The Layer-3 test fails on a deliberately untagged fixture before it passes on the corpus.
  An assertion that has never been made to fire is not a guard — `CLAUDE.md` § *Testing
  Discipline*.


## Work breakdown

1. Create `docs/trackers/issue-clusters.md` with `IC-1`…`IC-8` and the `IC` prefix declared.
2. Backfill one `cluster/<slug>` tag onto all 30 open bug files, filling the 20 empty `tags:`
   blocks in the same pass.
3. Repair `docs/issues/2026-08-31-peer-commit-captures-another-sessions-working-tree.md`,
   which carries **two** frontmatter blocks — the librarian's stamped one, and a hand-written
   one orphaned below it as body text, so its `opened`, `severity`, `owner` and `related` are
   invisible to every query. The only file in the corpus in this state.
4. Register `IC` in `docs/TAXONOMY.md` and point `CLAUDE.md` at it. An unregistered prefix is
   undiscoverable, and the convention then depends on this spec being read.
5. Backfill the archive (494 files) — a separate pass, and the gate cannot be enabled for
   `archive/` until it is done.
6. Ship the Layer-3 test.

Steps 1–4 are this session's scope. Steps 5–6 are the implementation plan.


## Not in scope

- **Reviving `docs/issues/INDEX.md`.** Retired 2026-05-18 for reasons that still hold. The
  registry holds claims, tags hold membership, and the index is a query. A rendered snapshot
  table may be added later if it is labelled a snapshot, the way the `BL-N` queue labels its
  own.
- **Changing `BL-N`.** Sequencing and mechanism are different axes; the queue keeps its own.
- **A catalog-indexed `cluster:` field.** Custom frontmatter keys land in `extra`, which is
  documented not catalog-indexed and not filterable. Adding a column buys nothing `tags`
  does not already give, verified this session.
- **Deriving clusters automatically.** Mechanism similarity is a judgement. The derived half
  of the problem belongs to artifact `8ba9230c8d25b35c` Layer A, over a corpus that actually
  has internal citations.
- **Auditing whether the six `status: fixed` files in `docs/issues/` are archive-ready.** Real
  and adjacent, needs a gate-green + regression-test check per file, and is its own sweep.
