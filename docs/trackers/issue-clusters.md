---
id: '1b5a080fe2efcb6b'
kind: tracker
status: active
title: Issue Clusters — the defect class a bug instantiates (IC-N)
owners:
- marius
tags:
- defect-classes
- issues
- clusters
- promotion
- mineable
topic: issue clusters and rule promotion
entry_prefix: IC
entry_high_water_IC: 22
---

> **Prefix:** `IC-N` — one **defect class** the bug corpus instantiates. Declared ledger; the
> `IC` namespace is project-wide (`docs/TAXONOMY.md`).

## What this ledger is for

`docs/issues/` answers *what is broken*. `docs/trackers/open-issue-work-queue.md` (`BL-N`)
answers *what to pick up next*. Neither answers **what these bugs have in common** — and that
is the question an architectural problem is visible in.

Each entry here is a **class**, stated as a claim that could be false. Membership is carried by
a reserved `cluster/<slug>` tag in each bug file's frontmatter, so a class's instances are a
query rather than a list:

```
artifact(action="find", kind="bug",
         filter={"tags": {"contains": "cluster/<slug>"}})
```

**This ledger never lists its members.** That is deliberate, and it is the whole reason this
surface exists rather than the three that preceded it. `docs/issues/INDEX.md` listed files and
was retired 2026-05-18. `open-issue-work-queue.md` § *Sequencing notes* listed `BL-1`, `BL-2`,
`BL-6`, `BL-8`, `BL-11`, `BL-12`, `BL-16` as two clusters — all seven are now `done` or
`done, archived`, and the prose still names them. A member list is a fact about an instant, the
same defect `docs/TAXONOMY.md` records for a bare SHA and a bare ordinal. A query is re-evaluated
on read and cannot go stale.

So: a `**Members:**` line carries the query, plus `n=<count>` and the date it was run. Trust the
query; re-run it before trusting the count.

**And the query's POPULATION is not the whole corpus — say which one you mean before comparing
two classes' counts.** A tag is required of `docs/issues/*.md`, and 34 of 34 carry one; it is
**not** required of `docs/issues/archive/*.md`, and 373 of 529 carry none (measured 2026-09-02 —
29.5% coverage). That is a deliberate bound rather than drift: `tests/issue_clusters.rs`'s module
header records that the classes were derived from the **open** backlog and that 279 archived files
in the backfilled window match none of them, so *"forcing a fit would corrupt the counts that
promotion reads"*. The count gate reads open **and** archive, so every tagged archived file does
count. An `n` is therefore **exact** over *files carrying the tag* and a **floor** over *files
instantiating the class* — and the two readings diverge most for the oldest classes.

Which is why the paragraph above defends only half of what it looks like it defends: re-running
the query refreshes the **count** and never widens the **population**. Read
`tests/issue_clusters.rs`'s module header before proposing any retro-tagging pass over the
archive. A 2026-09-02 audit measured that 29.5%, read it as convention drift, and was one step
from a 236-file campaign this bound forbids — `reconnaissance-patterns:R-170`, whose lesson is
that a number and the scope validating it must co-locate at the point of **reading**, not at the
point of enforcement.

Design: `docs/superpowers/specs/2026-08-31-issue-clusters-design.md`.

## The entry shape

Seven fields, and nothing else is required.

| field | content |
|---|---|
| `**Slug:**` | the `cluster/<slug>` tag value — the closed-set entry the gate checks against |
| `**Claim:**` | the mechanism, stated so it can be false |
| `**Members:**` | the query, plus `n=<count>` and the date it was run |
| `**Blind party:**` | who structurally cannot see it, and why — or `none — ordinary design defect` |
| `**Promotes to:**` | target surface, per the routing table below |
| `**Mechanism status:**` | `none yet` \| `designed` \| `shipped (<what>)` — borrowed from `OB` |
| `**Valid:**` | `invariant` \| `dated YYYY-MM-DD` \| `conditional — <event>` |

A **slug is claim-shaped, never topic-shaped**: `blast-radius-exceeds-visibility`, not
`concurrency`. A topic slug re-creates the tag soup this replaces; a claim slug can be false,
which is what makes the cluster promotable.

**One `cluster/` tag per bug file.** A bug spanning two classes names the one whose *mechanism*
it instantiates and cites the other in prose. Multi-membership makes counts non-additive, and
the counts are what drive promotion.

## How a cluster becomes a rule

**Threshold: three or more instances spanning two or more subsystems.**

Three is the count at which this corpus has repeatedly noticed itself unaided — three separate
open bug files carry a sentence of the form *"this is the third instance today of one
mechanism"*. The second condition is the load-bearing one: three bugs in **one** subsystem are a
broken subsystem and belong in a bug file; three across **two** are a mechanism and belong in a
rule.

`**Blind party:**` picks the target. **No new rule surface is created** — the space is covered:

| the cluster… | promotes to |
|---|---|
| has a blind party **and** fails with a plausible answer rather than an error | `OB` — `docs/trackers/observer-blindness.md` |
| needs a runtime gate or hook | `H` — `docs/trackers/codescout-usage-hookify.md`, or `I` in `docs/trackers/test-escape-hardening.md` |
| holds across every project, tool and model for this operator | `OP` — `docs/trackers/operator-rules.md` |
| is codescout-specific engineering discipline | `CLAUDE.md` |
| is a written claim that decayed with no repair trigger | `DC` — `docs/trackers/claim-decay.md` |

**A rule with no mechanism is a worklist item, not a rule.** `**Mechanism status:** none yet` is
the honest way to record that, and promotion without the field produces advice — which is the
failure mode this ledger exists to avoid.

**Promotion does not close the cluster.** The entry stays as the standing membership query;
`**Promotes to:**` gains the target's id, and new instances keep landing under the same tag.

## Relationship to the neighbouring trackers — read before adding a row

- **`OB` (observer blindness)** is a *narrower* ledger, not a parent. Its admission test is
  *"would a more careful version of the same party have caught it?"* — if yes it is not an `OB`.
  Several classes here fail that test and are ordinary design defects; they still belong here.
  Where a class passes it, this ledger's entry carries the class **membership query** and the
  `OB` entry carries the **class analysis**. Do not duplicate the analysis.
- **`BL` (open-issue work queue)** is the sequencing axis — readiness, blockers, what to pick up.
  Orthogonal. A bug has one `IC` class and may or may not have a `BL` row.
- **`DC` (claim decay)** takes the *written claim* that rotted. A bug whose defect is a decayed
  record belongs to both: `IC` for the mechanism, `DC` for the missing repair trigger.
- **Derived structure is somebody else's half.** `docs/superpowers/specs/2026-08-30-tracker-grain-and-corpus-topology-design.md`
  Layer A finds hubs by deriving them from `cites` edges. That method sees only what authors
  linked, and bug files barely link — 3 of 30 open files carry a non-empty `related:`, two of
  them pointing into `archive/`. Mechanism similarity is declared at file-open time or it is not
  available at all.

## How to mine this

```
# every live bug in one class, archive included
artifact(action="find", kind="bug", filter={"tags": {"contains": "cluster/<slug>"}})

# the same class, actionable only
artifact(action="find", kind="bug", filter={"and": [
  {"tags": {"contains": "cluster/<slug>"}},
  {"status": {"in": ["open", "investigating"]}}]})

# classes owed a rule: n >= 3 and Promotes to: is still `not yet`
grep -A6 '^## IC-' docs/trackers/issue-clusters.md | grep -B4 'not yet'

# a bug file that declares no class (what the gate will enforce)
grep -L 'cluster/' docs/issues/2026-*.md
```

**The `not yet` token in `Promotes to:` is load-bearing — keep it when you add a verdict to that
field.** The third query above keys on it. Adjudicating a class tempts you to overwrite the field
with the *spread* verdict, which answers a different question, and the class then leaves the query
without anything reporting that it did. Measured 2026-09-01: `IC-13`, `IC-14` and `IC-15` were
adjudicated that day, each had `not yet` overwritten by the same author in the same sitting, and
all three vanished from a query returning six classes — while every one of them still read
`Mechanism status: none yet`, i.e. still owed exactly what the query looks for. Found by running
the query, not by re-reading the edits; restored the same day. `IC-16` is correctly absent — its
rule already exists in `CLAUDE.md`, so it is not owed one.

Note what the query returns: field blocks identified by `**Slug:**`, never by `## IC-N`. `-B4`
reaches Blind party at the furthest, and the heading is five lines up. That is usable — the slug
names the class — but do not expect an IC number back.
### One slug, two spellings — a `cluster/`-prefixed pattern cannot see the Index

**Write `(?:cluster/)?<slug>` in any hand query over this file.** The slug is spelled
two ways here and the difference is invisible until a count comes back wrong:

| surface | spelling |
|---|---|
| Index table, slug column | bare, in backticks — `` `capped-result-presented-as-complete` `` |
| `**Members:**` field | `cluster/`-prefixed, inside the filter JSON |
| bug-file frontmatter `tags:` | `cluster/`-prefixed |

So a pattern requiring `cluster/` is **structurally incapable** of matching the Index
row, and one requiring backticks cannot match frontmatter. Neither errors; both return
a smaller number that looks like an answer.

**Measured 2026-09-02: this caught two sessions in one night, in this file.** Once at
the session's start, building the count gate, and once ~6 hours later in a peer review
of staged edits, where `grep -cE "^\+.*cluster/<slug>"` reported **1** changed line per
slug when the true figure was **2** — the Index row was unmatchable by construction. The
wrong number supported the conclusion its author already held, which is why it was not
questioned.

**Why the gate does not protect you.** `tests/issue_clusters.rs` already carries
`` `(?:cluster/)?([a-z][a-z-]+)` `` — an optional group that exists *solely* because
these two surfaces disagree. That accommodation made the machine-readable path correct
and left the corpus exactly as it was, so the trap stayed armed for every hand query and
fired again the same day. **A parser hardened against a naming inconsistency does not
harden the corpus — it removes the pressure to fix it**, which is how the inconsistency
survives to catch the next reader. The note you are reading is the read-surface half the
regex fix could not supply.

**The real repair is one spelling, not two patterns** — either the Index column carries
`cluster/<slug>` like `**Members:**` does, or the slug column is dropped in favour of the
prefixed form. It is owed and deliberately not taken here: it rewrites 22 Index rows, and
this file currently carries four sessions' staged edits, so a whole-column rewrite would
collide mid-repair. Whoever takes it should do it when the file is uncontended, and
delete this subsection in the same commit.

## Index

> Hand-maintained reading surface. The `## IC-N — <title>` headings are what define the tokens
> and what `link_scan` resolves; this table is for scanning. `n` is a snapshot — re-run the
> `**Members:**` query before trusting it. **Measured 2026-09-01: three separate re-derivations
> were invalidated inside one session** — IC-3 20→22 and IC-6 29→30 while a blind audit was
> running, then IC-2, IC-13 and IC-14 each +1 about two hours later — every one of them a peer
> session filing bugs in the same checkout, none of them a mistake by whoever last wrote the cell.
> So these cells are stale **by concurrency**, not by neglect, and no manual sweep can hold them:
> the sweep's own result is invalidated by the next commit. **That gate now exists.**
> `tests/issue_clusters.rs` parses this file's `**Slug:**` set, walks the corpus for tag validity,
> and — in `every_index_count_matches_the_corpus` — asserts every `n` cell below against its
> derived count, so a drifted cell is unmergeable rather than noticed later; a pre-commit hook
> (*refuse a commit whose ledger counts disagree with its staged corpus*) runs the same check at
> commit time. *(This passage read "asserting each table cell against its derived count is the
> missing gate" until 2026-09-01, after the gate had already shipped — `cluster/doc-contradicted-by-code`,
> which is `IC-11`, inside the ledger that defines it.)*
>
> **What stays ungated is the prose that reads those cells.** `**Members:**` and `**Promotes to:**`
> quote counts in text, and the gate's own assert message hands re-deriving them back to a human.
> Four were reasoning from superseded numbers on 2026-09-01 — `IC-3`, `IC-10`, `IC-11`, `IC-14`,
> repaired at `0c5bab41` (patch-id `88bcbdba8b1c9b705442a73a3258d2b5a1c82638`) — and `IC-10`'s read
> `not yet` — n=2 beside a cell already saying the class cleared both bars. So the gated surface and
> the surface a promotion decision is actually read off are not the same surface.
>
> **When a count moves, re-derive every judgement that quotes it, in the same pass.** The
> 2026-08-31 backfill updated `**Members:**` and this table but not `**Promotes to:**` — the
> field that records the *decision* — so four entries carried a correct count beside a judgement
> reasoned from the superseded one, `IC-6` reading `not yet — n=2, below threshold` while holding
> the largest count in the corpus at 27. Because the ledger's own mining query is
> `grep -B4 'not yet'`, the effect is a class surfacing as owed-a-rule next to a reason telling
> the reader to dismiss it. Repaired for IC-4, IC-5, IC-6, IC-7 and IC-9; the sentence below the
> table carried the same defect, reading "six" through three rewrites because each rewrite listed
> only the classes it was adjudicating.
>
> **Re-derive against `git ls-files docs/issues`, not a bare recursive grep.** `docs/issues/` also
> holds untracked session-log directories (`.buddy/`, `.codescout/`) whose tool logs quote
> `cluster/<slug>` verbatim from the commands that counted them, so a recursive grep can read its
> own measurement back as corpus — observed 2026-08-31 inflating three cells at once. `git
> ls-files` is also the definition `tests/issue_clusters.rs` enforces, so it is the one that
> matches the gate.
>
> **One bug file carries exactly one `cluster/` tag, so a bug instantiating two classes is
> counted for one — and the loss is not random.** Whichever class the author framed as
> *secondary* is the one that disappears, which is systematically the less-developed class, i.e.
> the one nearest a threshold. Measured 2026-09-01: the vacuous `pinnable` assertion was found
> *inside* the `GetUsageStats` reachability sweep and belonged to `IC-3` by framing and to
> `IC-16` by claim; `IC-16` sat at n=2 and read *"what is missing is a third instance"* while a
> third existed in the corpus, invisible to its own membership query. Resolved by filing the
> assertion as its **own** bug file rather than as a paragraph inside another — which is the
> general remedy: **if a finding satisfies a second class's claim, it is a second bug file.** The
> one-tag rule is worth keeping (it is what makes `n` a partition rather than a tally); what it
> requires is that the unit of filing be the *claim satisfied*, not the investigation that found
> it.

| id | class | slug | n | promotes to | mechanism |
|---|---|---|---:|---|---|
| IC-1 | the blast radius of a write is wider than the set of peers you can see | `blast-radius-exceeds-visibility` | 3 | `OB-3` — 2026-09-01 | partial; **split taken → IC-17** |
| IC-2 | a gate keyed on an event it cannot observe substitutes a proxy | `gate-keyed-on-unobservable-event` | 23 | `OB-6` — promoted 2026-09-01 | designed (exemplar shipped) |
| IC-3 | declaration is not execution | `declared-not-wired` | 24 | `OB-7` — promoted 2026-09-01 | **family 1 GATED** (`tests/tool_reachability.rs`); 2 of 3 families open |
| IC-4 | config propagation is additive | `config-propagation-is-additive` | 8 | `OB` — passes admission test; hook owed | **partial** — 2 of 8 surfaces (`hooksPath`, worktree gitdir) |
| IC-5 | the reproduction environment is not the gating environment | `repro-env-diverges-from-gate-env` | 13 | `H` — seven subsystems; mechanism owed | none yet |
| IC-6 | an addressing scheme with no escape hatch | `addressing-without-an-escape-hatch` | 30 | `CLAUDE.md` § Parsers Over a Namespace — **landed** | shipped (partial) |
| IC-7 | lazy warm-up bills the first caller | `lazy-warmup-bills-the-first-caller` | 4 | not yet — 2 of 4 unconfirmed | shipped (partial) |
| IC-8 | a record asserts a completed action nothing re-checked | `record-asserts-an-unchecked-completion` | 5 | `DC` | none yet |
| IC-9 | an assertion over environment-controlled text is satisfiable by accident | `assertion-satisfiable-by-accident` | 2 | not yet — two tags withdrawn as misfits | none yet |
| IC-10 | authorship on a shared checkout is unrecoverable after the fact | `authorship-unrecoverable-after-the-fact` | 3 | **clears both bars 2026-09-01** — n=3, spread 3, via second-read retag | none yet — candidate is `H` (a provenance channel for working-tree state) |
| IC-11 | documentation denies a capability the code has since gained | `doc-contradicted-by-code` | 13 | clears count; **spread adjudicated 2026-09-01 — 4 doc surfaces / 4 subsystems**, not re-adjudicated for the sixth (`CLAUDE.md` § *Observer Blindness* denying a pid→session join the session registry carries) nor for the seventh (a Rust **doc comment** — a fifth surface, and the first member whose prose sits inside the file it describes) | none yet — one of three sub-shapes is mechanizable |
| IC-12 | transient shared state lies to every reader | `transient-shared-state-lies-to-readers` | 2 | not yet — n=2, and the remedy so far is knowledge rather than mechanism | none yet |
| IC-13 | a capped result is presented as complete | `capped-result-presented-as-complete` | 12 | clears both bars — **spread re-derived 2026-09-01 over the 9 then: 5 coarse / 7 fine** (was 6 / 11 over the pre-ruling 16); **not re-derived over the 12** | none yet — **clause widened + 7 non-members moved out 2026-09-01** to IC-19/20/21/22; claim was true of all 9 then, and the three 2026-09-02 additions were judged against it at filing |
| IC-14 | a guard's coverage is narrower than its name | `guard-narrower-than-its-name` | 12 | clears count; **spread adjudicated 2026-09-01 over the 11 then — 4 subsystems / 6 distinct guards**; **not re-adjudicated over the 12** | none yet — one sub-shape of three is mechanizable |
| IC-15 | a parameter is accepted then silently dropped | `accepted-parameter-silently-dropped` | 17 | clears count; **spread adjudicated 2026-09-01 — 6 subsystems** | **partial** — probe at 5 of 8 sites 2026-09-02; shared half extracted AND un-feature-gated |
| IC-16 | an assertion that cannot fail | `assertion-that-cannot-fail` | 3 | **clears both bars 2026-09-01**; rule already in `CLAUDE.md` — the third instance buys measurability, not a rule | designed; positive-form guard owed |
| IC-17 | a shared resource carries no owner, so enumerating the peer does not help | `shared-resource-carries-no-owner` | 21 | `OB-8` (+ OB-2) — 2026-09-01 | partial |
| IC-18 | a selector is narrower than the population it names | `selector-narrower-than-its-population` | 7 | clears both bars 2026-09-01 — 6 subsystems; remedy already Accepted as ADR-2026-08-27 for the tool-facing half | **partial** — nothing reaches an author-written selector |
| IC-19 | a truncated window is ordered by a key unrelated to why it was requested | `truncated-window-ordered-by-the-wrong-key` | 3 | **clears the count bar on creation** — 3 subsystems; spread and `OB` routing unadjudicated | none yet |
| IC-20 | a floor is published under the name of a total | `floor-published-under-the-name-of-a-total` | 1 | not yet — n=1; kept apart from `IC-19` on the remedy test (rename vs re-select) | none yet |
| IC-21 | an instrument reports presence or a count where the decision turns on magnitude | `instrument-omits-the-dimension-that-grows` | 2 | not yet — n=2, one short; already 2 subsystems, so instance 3 meets both bars | none yet |
| IC-22 | a next-step hint is composed from the response shape, not from the request | `hint-composed-without-the-request` | 3 | not yet — n=3, **count bar cleared 2026-09-02**, judgement owed; seed **fixed** `bb4688fd`, second member open on the *preview* surface | none yet |

**Every class at n≥3 clears the count threshold; spread is adjudicated per entry.** Read the `n`
column — that is the derivation, and it cannot go stale when a count moves. This sentence used to
publish the *value* alongside a hand-maintained list of which classes qualified, and it rotted
three times in one evening: each editor updated the list for the classes they were adjudicating
and let the number ride along, so "six of ten" became "six of eleven" became eight, the middle
one never re-derived by anyone. `CLAUDE.md` § *Observer Blindness* prescribes exactly this — ship
a claim's derivation rather than its value, so a reader re-checks it instead of re-deriving it
under a counting rule of their own choosing. **IC-6 is the
first to land its rule** — `CLAUDE.md` § *Parsers Over a Namespace*, 2026-08-31. **IC-5 and IC-6
are now adjudicated rather than flagged**, and both route away from `OB` — each declares
`Blind party: none`, which fails OB's admission test, so the count was never the only thing
stopping them. IC-5 spans six subsystems (cargo feature config, wine, shell env, workspace
resolution, toolchain, ambient embedder config), seven of its eleven members outside the
Windows/wine lane its old note said contained all of them. IC-6 spans five. **IC-4 routes the
other way on the same test**, adjudicated 2026-08-31: it names a blind party — the operator who
made the edit, whose successful check of the value that landed is *positive evidence for the
wrong proposition* — and it fails with a plausible answer rather than an error, so it satisfies
the routing table's first row. Its old field doubted this by conflating the recording surface
with the remedy; the `H` hook that diffs intended against effective config is the mechanism it
owes, not an alternative home. IC-7 still fails on
premise confidence, IC-8 routes to `DC` regardless of n, and IC-10 / IC-11 are newly opened at
n=1.

**IC-9's flag is withdrawn, and the error was mine.** I tagged two archive files into it that do
not instantiate its claim: `ollama_large_batch_exceeding_batch_size` was vacuous the day it was
written and `cross-process-write-lock-test-passes-when-it-does-not-run` is vacuous when skipped —
neither turns on **environment-controlled text**, which is the whole of IC-9's claim. Both were
matched from their titles, which read as "a test that passes when it shouldn't" — true of IC-9
and true of a wider family. Tags withdrawn, n back to 1, below threshold. Their own pre-existing
`vacuous-assertion` and `green-proves-nothing` tags say what they actually are.

**IC-10 was split out of IC-1 on the remedy test**, not on a count. `IC-1` wants an ownership
protocol over a shared resource; `IC-10` wants a provenance channel. The
`buddy-compact-banner` bug moved with it, so IC-1's 18 is a different 18 than the backfill
reported — one gained (`nested-hook-state`), one lost. A count that holds steady across a
re-partition is the clearest argument for re-running the query rather than trusting the cell.

**Coverage, 2026-08-31.** The open corpus is no longer maintained by hand: `tests/issue_clusters.rs`
(shipped `522675a6`) fails when any **tracked** file directly under `docs/issues/` declares no
`cluster/` tag, more than one, or a slug this ledger does not define. So open-corpus coverage is
whatever the gate last allowed through, and there is no number here to go stale. It fired on its
first run and caught two files committed within the hour.

The **archive is deliberately outside the gate**, because the classes were derived from the open
backlog and forcing a fit would corrupt the counts promotion reads. **Re-derive the coverage
rather than read it here.** This paragraph published four figures and a partition, and every one
moved inside a single evening:

```
git grep -l -E 'cluster/[a-z0-9-]+' -- ':(glob)docs/issues/archive/*.md' | wc -l   # tagged
git ls-files ':(glob)docs/issues/archive/*.md' | wc -l                             # total
```

`git grep -l` is the load-bearing form: it counts **files**, where `grep -o | sort | uniq -c`
counts **occurrences**, and a bug file that names its own slug in prose as well as in frontmatter
is then counted twice. That is not hypothetical — it is why `cluster/config-propagation-is-additive`
reads as 9 by occurrence against a true membership of 8
(`docs/issues/archive/2026-08-30-core-hookspath-points-at-pre-rename-path.md` names it in both places).
Every `n` in the table above is a **file** count.

**No snapshot is kept here — two were, and both rotted inside a day.** The first read *"78 of the
357 files dated 2026-07-01 or later are tagged and 279 are deliberately untagged … a further 137
pre-July files are unbackfilled"* — four figures and a two-way partition, moved by `13226bda`,
`77d4da06` and `0dea2246` within one evening and re-derived by none of the three commits that moved
them. The second read *"118 tagged of 495"* and was **152 of 525** when re-derived at `0c5bab41`, on
the same date it was written. The archive is outside the gate, so nothing holds its coverage and any
figure here decays at the rate bugs are archived; run the two commands above instead. (Re-derived
the same day under both instruments: `git grep -l` counts a file whose only mention is in prose,
where the gate reads frontmatter alone — at **this** unit, *is the file tagged at all*, zero files
differ today. **At the per-slug unit the two already disagree, and that is the unit every `n` in the
table uses.**)

**So re-derive a single class with the ANCHORED form, never a bare `git grep -l 'cluster/<slug>'`:**

```
git grep -clE '^[[:space:]]*-[[:space:]]*cluster/<slug>[[:space:]]*$' -- 'docs/issues/*.md'
```

The unanchored form counts any file that *mentions* the slug, and the files that mention a slug they
do not declare are **the files that were retagged** — a bug file recording its own move names the
class it left. Measured 2026-09-01: `guard-narrower-than-its-name` reads **12 unanchored against 11
anchored** and `assertion-that-cannot-fail` **4 against 3**, both inflated by
`2026-09-01-foreign-index-guard-passed-a-peers-staged-deletion.md` and its sibling, which say in
prose which class they came from. The error therefore lands precisely on the classes under active
adjudication, where the count is being read to make a decision. Found by a peer session
(`codescout-3c`) while re-deriving the figure above, and it very nearly shipped the other way — as a
refutation of a correct measurement. The sentence before this one used to call the two instruments
*"interchangeable"* and name a file quoting an undeclared slug as the hypothetical that would part
them; two such files already existed.

**Every `n` in the table above therefore remains a floor.** Covering the archive would need an
explicit `cluster/unclassified` slug meaning *looked, nothing fits* — a taxonomy decision, not a
gate one.

**The candidate queue is now empty — all five became classes on 2026-09-01, and every one opened
at n=0.** `IC-13`, `IC-14` and `IC-15` are the backfill's three remaining shapes; `IC-12` is the
read-side window the git hooks introduced; `IC-16` is the vacuous-assertion family the `IC-9`
withdrawal exposed. The fourth backfill shape needed no entry — it had already been promoted to
`IC-11` on 2026-08-31, forced by a taggable instance arriving against a gate with no
`cluster/unclassified` escape hatch, rather than by its count.

**Four of the five have since been tagged; `IC-12` alone still reports zero — read the `n` column,
not this sentence.** All five *opened* at n=0, and this paragraph asserted that in the **present
tense** until `13226bda` (2026-09-01) tagged the members of `IC-13`–`IC-16` and left the claim
standing beside a table that already contradicted it. That is the defect this section's own
preamble names, committed inside the section that names it — and nothing reported it: `doctor`'s
entry-validity checks are gated on `EXPOSURE_THRESHOLD = 5` citing files
(`src/librarian/tools/doctor.rs:2682`), which day-old entries never clear, so the four fired
conditionals below were invisible to every instrument until a human read them.

The zeros were never evidence of rarity. The ledger stores a query, so a class whose members are
not yet tagged reports zero, and the `**Members:**` field of each says whose count it rests on.
`IC-12`'s remaining zero is the one kind that *is* evidence — it survived an archive pass that
looked, rather than waiting for one (see below). For the four that moved, `**Promotes to:** not
yet` now rests on **spread**, never on tagging, and every one is still single-party
classification.

`IC-16` inverts the usual direction and is worth reading for that alone: the rule came first, from
an SDD run, and lives in `CLAUDE.md` § *Testing Discipline* already. What never happened is
indexing the corpus against it — so *"which of our bugs instantiate the vacuous-assertion rule?"*
has no answer, and nobody can tell whether the rule is working. The class exists to make an
existing rule measurable rather than to earn a new one.
**The 2026-09-01 archive pass was single-party; an independent blind second read has since run, and agreed on 37 of 43.**
One party (`codescout-e8`) did both the classification and the tag application for all 40 files
behind IC-13 (16), IC-14 (7), IC-15 (15) and IC-16 (2), plus the two IC-3 → IC-15 moves. Nobody
re-checked those assignments against the class claims. That is not a reason to distrust the
counts; it is the difference between them and the `IC-9` withdrawal, where a second read is
exactly what caught two misfits. A reader in a month cannot otherwise tell which of the two
regimes produced a given number.

**The second read, run 2026-09-01 — 37 of 43 agreed.** The population was every file *currently*
carrying one of the four tags: **43**, being the pass's 40 plus three that acquired the tag
elsewhere. Blindness was structural rather than instructed — working copies had the `cluster/`
slug **and** every `IC-N` token redacted (the second mattered: these files cite each other's
classes in prose, so the obvious redaction alone leaks), all seventeen class claims were offered
rather than the four under audit so a misfit could surface as *any* class, and the ledger and
`docs/issues/` were off-limits to the readers. Three independent readers, one per batch.

Agreement **37/43 (86%)**, by on-disk class: IC-13 14/16, IC-14 7/8, IC-15 13/16, IC-16 3/3.
Calibration held — 27 of 28 high-confidence rows agreed, against 9 of 13 med and 1 of 2 low — so
the six disagreements are the arguable boundary cases and not scatter. **Five were adjudicated and
applied 2026-09-01**, each re-verified against the file's own text before the retag rather than
taken from the reader's summary; the sixth was declined. Counts moved: IC-3 20→22 (one retag plus
one peer filing that landed mid-audit), IC-10 2→3, IC-13 16→14, IC-14 8→9, IC-15 16→15.

| file | on-disk | second read | conf |
|---|---|---|---|
| `foreign-index-guard-passed-a-peers-staged-deletion` | IC-14 | **IC-10** (2nd IC-17) | med |
| `doctor-outside-roots-sample-is-unranked-and-unreachable` | IC-13 | **IC-15** | med |
| `capped-get-body-round-trips-into-truncating-write` | IC-13 | **IC-14** (2nd IC-13) | high |
| `cli-artifact-drops-time-scope-and-extra` | IC-15 | **IC-3** | med |
| `update-entry-drops-entry-silently-when-fields-is-also-present` | IC-15 | **IC-14** (2nd IC-15) | med |
| `friction-target-omits-command-and-file-path` | IC-15 | **IC-14** (2nd IC-13) | low |

**Five were applied**; `friction-target-omits-command-and-file-path` was declined — its reader said
no class's *mechanism* is present, which makes it evidence for a new class rather than a retag.
The first row was the one with a consequence: `IC-10` sat at n=2 with *"instance 3 meets it"*
recorded as its own promotion condition, and the retag tripped it. That is the
preamble's systematic-loss mechanism caught in the act — an instance filed under a well-developed
class (n=8) whose claim belongs to a threshold-adjacent one — and it is the argument for
adjudicating these rather than letting them stand.

**The preamble's predicted DIRECTION of loss was pre-registered and did not survive.** After the
first batch returned, two disagreements both moving toward the smaller class looked like
confirmation, and the test was stated before the other two batches ran. It failed: across all 43
the recorded second choices concentrate on `IC-2` (6) and `IC-3` (6) — the two largest classes
after IC-6, not threshold-adjacent ones. This is a null rather than a refutation, and the
confound is why: IC-2 and IC-3 are broad *mechanism* classes overlapping everything, so their
frequency as seconds may measure class breadth rather than promotion pressure. Published because
a re-derivation that disconfirms otherwise leaves no artifact at all (`CLAUDE.md` § *Testing
Discipline*).

**Two readers independently hit the same wall in `IC-13`'s claim text.** Its *"returned WITHOUT A
MARKER"* clause is strictly false for at least four members where the truncation signal is
computed **correctly** and then fails to reach the reader — dropped at the buffering boundary, or
left as an inert JSON key the text renderer discards. Both placed those under IC-13 as
nearest-available while naming the mismatch, and one reached for IC-3's *"no call site connects
them"* as the better mechanism. **The class's claim, not the tags, is what needs the ruling.**

Also surfaced and not acted on: three **split candidates** (the `ollama-large-batch` file's second
vacuity — a not-compiled test reporting `0 passed; 19 filtered out`, exit 0, character-identical
to a pass; `residual-workspace-pin-gaps`' finding 6, which the file itself calls *"never wired for
pinning in the first place"* and so states IC-3's claim standing alone; and
`artifact-find-ignores-workspace-pin`'s `scope="all"` hint pointing at the parameter already
passed), and three **no-fit** candidates for which no class's *mechanism* is present — a selector
silently narrower than the population it names, nondeterministic sampling (`SELECT` with no
`ORDER BY`), and a front-anchored window that structurally omits the item the caller needs.

Each tag was matched against the class's **stated claim**, never the bug file's title — title
matching is what put the two misfits in `IC-9` four hours earlier, because *"a test that passes
when it shouldn't"* is true of at least four classes. Members already carrying a `cluster/` tag
were left alone rather than re-adjudicated, with two deliberate exceptions named below.

**`IC-12` is off zero as of 2026-09-01, and how it got there is the point.** It stood at n=0 *on evidence* — an archive pass that looked and found nothing transient — while its own entry described a measured instance in prose, filed as a paragraph inside `2026-08-31-peer-commit-captures-another-sessions-working-tree.md` rather than as its own file. Its `**Members:**` line said so: *"nothing to tag yet"*. The remedy was the one this preamble already prescribes — **if a finding satisfies a second class's claim, it is a second bug file** — and `2026-09-01-pre-commit-stash-removes-every-peers-unstaged-work.md` is that file. So the zero was never evidence of rarity either; it was evidence about the *unit of filing*, which is the same lesson in a second place. The class was not found today, only filed today.

**`IC-11` was not backfilled with the others — and then was, on 2026-09-01, by a probe of its own.** Roughly 15 doc-vs-code candidates existed, and its
claim turns on *"the prose was true when written"*, which is a fact about history and not about
the text — the same discriminator `claim-decay`'s inclusion test makes mandatory, and which
separates `decayed` from `never-true` only under `git log -S`. Tagging them without that probe
per file would be exactly the shortcut that ledger exists to forbid. Fourteen went to that probe on
2026-09-01: three passed and are tagged, one was refuted, ten are recorded as
probed-and-not-established. The entry's own *The probe* section holds the working.

*(This paragraph asserted "**deliberately not backfilled**" in the present tense until then,
standing beside an `n` column that already read 4 — the same premise-moved-conclusion-didn't
defect the paragraph two above this one records happening to itself. Twice in one section, and
the second instance was committed by the author of the first. Neither was reported by anything.)*

**The `IC-3` / `IC-15` boundary, settled 2026-09-01 on the remedy test** — the same test that
keeps `IC-1` and `IC-2` apart. The discriminator is: **was a caller-supplied value accepted?**
If yes, the code path *ran* and discarded the value, and the remedy is to round-trip it or refuse
it — `IC-15`. If no, the capability exists and no call site reaches it, and the remedy is to find
a caller — `IC-3`. Applied to `IC-3`'s twenty, exactly two move: `audit-doc-refs-scope-param-ignored`
and `audit-doc-refs-fail-on-doc-mismatch`, both of which take a caller's value and drop it.
`cli-doctor-exposes-no-fix-flag` stays in `IC-3` — the flag does not exist at the boundary, so
nothing is accepted to be dropped — and `audit-doc-refs-lsp-stubbed-off` stays for the same
reason. `constitution-rule-malformed-glob-silent-fail-open` and `drift-detection-enabled-is-a-dead-config-key`
were left in `IC-3` as genuinely arguable rather than moved on a coin-flip.
## IC-1 — the blast radius of a write is wider than the set of peers you can see

**Slug:** `cluster/blast-radius-exceeds-visibility`
**Claim:** A session's writes reach every peer sharing the filesystem; its peer listing reaches only peers sharing its config profile — and the listing reports that short population as a definite count. What follows is that a session cannot know its own blast radius. What does **not** follow is that coordination is impossible: this entry asserted that until 2026-09-01, and `IC-17` falsified it. See *the falsification* below.
**Members:** `filter={"tags": {"contains": "cluster/blast-radius-exceeds-visibility"}}` — n=3, 2026-09-01, by query after the `IC-17` split. Was 18; the 15 members whose defect survives complete enumeration moved. All three that remain are *instrument* bugs, which is what makes the remedy uniform — and is the property the pre-split membership did not have.
**Blind party:** the session doing the writing. Not carelessness — it *holds* the listing that would reveal the peer, and the listing is scoped narrower than the sharing. `ListAgents` answering *"Peer sessions (2)"* is a confident small number, which survives review in a way a suspicious zero would not.
**Promotes to:** `OB-3` — *a peer listing is arbitrary with respect to the real population*. `OB-8` and `OB-2` moved to `IC-17` with the split: they are the ownership half, and this entry no longer holds their members.
**Mechanism status:** partial — OS enumeration shipped for `OB-3`, which is this class's whole remedy surface now. `scripts/peer-sessions.sh` is that instrument, and the third member above says what it still does not compare. The unowned-resource mechanisms (`target/`, the working tree, the git index, `entry_high_water_<PREFIX>`) moved to `IC-17` with their bugs.
**Valid:** dated 2026-09-01

What the instances share is not concurrency in the ordinary sense — there is no lock to take, because nothing models the resource as shared at all. `target/debug/codescout` is written by feature set and read by path; the working tree is written by whoever runs an editor and read by whoever runs `git commit -a`; `entry_high_water_IC` is read-modify-written by each host from its own committed copy. In every case a second writer is *representable* and simply not *represented*.

The two directions compound. You cannot build an ownership protocol over peers you cannot enumerate, and the enumeration is scoped to the config profile while the sharing is scoped to the filesystem. That compounding is real where both apply, but the sentence this paragraph used to end on is not: it read *"that is why `cross-account-agents-cannot-see-each-other` … is the reason `peer-commit-captures-another-sessions-working-tree` has no remedy available today"*, and the falsification below refutes exactly that causal link. The capture bug is `IC-17`'s now, and it lacks a remedy for a reason of its own — the resource carries no owner field — not because its peer was unlistable.

The two **suspected, not proven** members — `workspace-read-only-flips-mid-session` and `sdd-ledger-and-catalog-rows-vanished` — moved to `IC-17` in the split, and carry their caveat with them. Both are unexplained state changes with no actor found in the owning session's own history, which is what an unseen peer looks like from inside. Neither has a peer identified, so neither class is credited with them as evidence.

**Falsified by** a member whose defect survives a complete and correct listing — that member belongs in `IC-17`. (This entry's original falsifier, stated in these terms, fired on 2026-09-01 and moved 15 of 18 members. The replacement above is the same test applied per-member rather than to the class.)

**This class was demonstrated during its own writing.** The open corpus grew from 30 to 31 files while this entry was being drafted — a peer session in the same checkout filed `peer-sessions-never-compares-start-time-to-build-time`, and the measuring session had no signal that its count had changed. The new file is also a member on its own merits: `scripts/peer-sessions.sh` prints each peer's start time but never compares it to the served binary's build time, so **9 of 13 live processes serving pre-rebuild bytes read as healthy** (measured 2026-08-31T21:47). Same shape as `listagents-omits-cross-profile-sessions` — a peer instrument presenting an incomplete characterisation as a sufficient one.

**This entry's falsification condition FIRED on 2026-09-01, and the class should split.** The
condition was stated here as: *an instance where the writing session could enumerate the peer
and still collided — that would move the defect from visibility to coordination and split this
class in two.*

It fired **four times in 34 minutes**. Two sessions enumerated each other — `ListAgents` named
both, and they exchanged eight messages about this exact mechanism — and collided at
`e0525462` (23:53), `3a5aec7a` (23:55), `1b40dabd` (00:06) and `77d4da06` (00:27). Every one
post-dates mutual enumeration; the fourth landed with a warning message **in flight**.
Enumeration was complete **for this pair** and changed nothing.

**Narrowed 2026-09-01, and the narrowing is confirming rather than damaging.** Enumeration was
not complete as a *population*. Measured on this host, in units matched to the question this
entry asks — *can the writing session see the peers who can reach its files?* — **`ListAgents`
returns 1 of the 4 live peer sessions whose `cwd` is inside this checkout**, while
simultaneously reporting 3 peers in an unrelated checkout. The instrument is wrong in both
directions at once: 75% of the peers who share this working tree are invisible, and 75% of what
it does report cannot reach it.

The falsifier needs only the **pair**, and the pair was mutually enumerable, so `IC-17` stands
on unchanged evidence. But the sentence above, read as a population claim, is refutable from
this project's own data — and a reader who refuted it would discard a correct partition.

Two consequences, both favourable to the split and neither available by argument. A 1-of-4
instrument makes **this** class considerably worse than the entry assumed, so the measurement
strengthens `IC-1` and leaves `IC-17` untouched — two halves of a former single class moving
independently under new evidence is what a correct split looks like. And the colliding pair sat
inside the visible quarter, so the falsification ran on the *most favourable sample available
for coordination* and coordination failed four times regardless.

*(Supersedes a `4 of 20` figure cited here for six minutes. That ratio compared a profile-scoped
instrument against a raw socket-file count of which 7 were dead processes — a units mismatch
whose danger is that 20% and 25% are close enough to read as agreement. Derivation and the full
count table: `cluster-promotion-session-log:F-3`.)*

**What that breaks is the *therefore* in this entry's own Claim.** The claim reads *"its peer
listing reaches only peers sharing its config profile. Coordination is **therefore** impossible
by construction"* — asserting the short listing is what makes coordination impossible. It is
not. Coordination failed with the listing complete, because knowing **who** your peer is does
not tell you **which lines in a shared tree are theirs**: `git status` returns a correct,
complete listing with nothing marking whose changes are whose. The binding constraint is a
missing **ownership field**, not a missing peer.

**So the two halves need separating, and they have different remedies** — the same test that
kept `IC-1`/`IC-2` and `IC-3`/`IC-15` apart. *Visibility*: the peer cannot be enumerated —
`cross-account-agents-cannot-see-each-other`, `listagents-omits-cross-profile-sessions`,
`peer-sessions-never-compares-start-time-to-build-time`; remedy is a better instrument, and
`OB-3` already carries it. *Coordination/ownership*: the peer **is** enumerable and the shared
resource has no owner — the six peer-capture instances, `entry_high_water_<PREFIX>`, `target/`;
remedy is isolating the resource, and `OB-8` carries it.

**Both sides of the split are now taken.** `OB-8` landed at `d710e58d`; `IC-17` was minted on
2026-09-01 and the 15 ownership members re-tagged through the catalog, verified
catalog-against-disk — 15 carrying the new slug, 3 the old, and the 3 are the instrument bugs
named above.

**The timing was re-verified independently before the split, because the whole argument rests
on it.** The reporting session put mutual enumeration at ~23:51; this session's own transcript
puts its `ListAgents` call at 2026-08-31T20:19:02Z — **23:19 local** — and its first outbound
message at 23:23. So the margin to the first capture is 34 minutes, not 2, and three of the
four captures are this session's own. Re-verification widened the evidence rather than
weakening it, which is the outcome to expect least often and to record most carefully.

**`IC-10`'s warning applies, and is discharged here rather than waived.** A re-partition is
exactly when a steady-looking count stops meaning what it did. `IC-1` reads n=3 today and read
n=18 yesterday, and **no bug was added, removed, fixed, or re-examined** — only the boundary
moved. Any trend drawn across 2026-09-01 on this row or `IC-17`'s is measuring this edit.
## IC-2 — a gate keyed on an event it cannot observe substitutes a proxy, and the proxy fails silently

**Slug:** `cluster/gate-keyed-on-unobservable-event`
**Claim:** A gate whose condition is an event outside its observation boundary substitutes a proxy — a caller-supplied flag, a monotone stamp, a per-process map — and the substitution fails silently, because a proxy returns a plausible answer rather than an error.
**Members:** `filter={"tags": {"contains": "cluster/gate-keyed-on-unobservable-event"}}` — **n=23, 2026-09-02, by the anchored file-count form.** (The unanchored `grep -rl` returns **24** on this slug, inflated by a single prose mention in `docs/issues/2026-09-01-cluster-count-gate-lists-the-index-but-reads-the-worktree.md` — `R-167` holding on this very field, caught here only because the anchored form was used second. The 2026-09-01 reading was 22, re-derived then with the same prescribed form.) (It read `n=19` here while the Index row read 20 and the corpus held 21 — a count moving without the judgements quoting it, which is this section's own preamble rule; the three counts were reconciled in one pass rather than by delta.) Two members arrived after the 19 reading, one of them `docs/issues/archive/2026-09-01-artifact-create-stamps-an-id-that-guard-locks-the-file.md` — whose tag is **flagged arguable in its own `unverified:` field**, because it fails LOUDLY and this entry's *Falsified by* clause excludes exactly that. It is tagged here only on the exact match to the **monotone stamp** proxy shape this claim names. A second read owes a ruling: either this claim widens to cover fail-closed proxies, or that file leaves for a new class. Newest member `docs/issues/archive/2026-09-01-library-is-gated-on-the-precondition-only-it-can-establish.md` is the **circular** form, and the sharpest one here: the gate wants *"does this project use libraries?"*, cannot observe it, and substitutes *"is one already registered?"* — a proxy **downstream of the very action the gate hides**. It also fails in a way the others do not: the tool stayed dispatchable, so discovery and dispatch disagreed and only discovery is a surface an agent can read. The 22nd member, `docs/issues/archive/2026-09-01-subagent-told-to-skip-guides-it-never-received.md`, is the form this entry already described in prose one paragraph down (*"the subagent ledger cannot see what the parent holds"*) before it had a filed instance: the auto-inject gate's real condition is *"has THIS agent received topic T?"*, it cannot observe agent identity because `agent_id` rides harness events and no MCP call carries it, and it substitutes the per-process `guide_hints_emitted` map keyed by `session_id` — which a subagent shares with its parent. Squarely **harness-scoped**, so it joins the nine and not the seven, and it is open for exactly the reason that split predicts: no substrate exists to answer the gate from inside the server. Its unfixable residual is `OB-11`. **The 23rd**, `docs/issues/archive/2026-09-02-foreign-index-prescribes-a-remedy-git-refuses.md` (**fixed** `74b9cc67`, patch-id `0e7feedf232c5ed9e22fd975c6fe36baa109e1d2`; the proxy half is untouched — only the prescribed-remedy half was closed), is this claim's proxy shape carrying a *second* defect the claim does not name, and the pair is why it is filed rather than folded in. The proxy half is textbook: `pre-commit-foreign-index.sh` cannot observe **who staged a path**, substitutes *"does `$git_dir/session-stage-log` claim it?"*, and git plumbing — a rebase or cherry-pick replay — stages without writing an entry, so a session's own work reads as `(unrecorded)` and is reported as a peer's. Plausible rather than an error, exactly as the claim says. What is new is that the refusal **prescribes an escape git then refuses**: in a `CHERRY_PICK_HEAD` / `MERGE_HEAD` state the pathspec form the hint names answers `fatal: cannot do a partial commit during a cherry-pick`, while the bare form the guard is blocking would have worked — so the intersection is empty and the caller has no compliant route. **It is also the measured counter-example to `1efc6488cb2b8946`'s third fix direction**, *"per-session worktrees, which dissolve this and most of `IC-17`"*: the deadlock was hit inside a **private linked worktree**, by a different route than the entangled-index case that file documents. Two proposed remedies were rejected on measurement rather than on taste — exempting every linked worktree tests for the wrong property (`--git-dir != --git-common-dir` proves *linked worktree*, not *unshared index*; two sessions in one worktree share its index and the guard handles them correctly today), and exempting on `$GIT_DIR/rebase-merge` is wider than the defect, since a conflicted rebase with `CHERRY_PICK_HEAD` unset commits by pathspec fine. Reported by a peer session and verified here; both rejections were accepted by the reporter, who noted they had diagnosed a proxy-keyed gate and then proposed a fix keyed on a proxy for their own property — the `CLAUDE.md` § *Observer Blindness* measurement holding again, one level up, inside the sentence naming it.
**Blind party:** the gate itself, and therefore every reader of its output. The server process cannot see a compaction, a dead hook, a `/clear`, or what a parent session already holds; each of those is a *conversation*-scoped or *harness*-scoped event, and the gate is process-scoped.
**Promotes to:** `OB-6` — *a gate collapses "cannot observe" into the confident answer*, `docs/trackers/observer-blindness.md`, promoted 2026-09-01 at `n=16`. **The `OB-4` reference this field used to carry was loose and is corrected here**: `OB-4` is about the `.worktrees/bench` liveness marker and never mentions the rendezvous gate, so it was never "this class's rendezvous half". The two are siblings on one axis — `OB-4` asks *why a proxy is trusted* (an accuracy record it later spends), `OB-6` asks *what the proxy does when wrong* (returns the confident value rather than admitting it cannot tell) — and their remedies differ, so neither subsumes the other.
**Mechanism status:** `designed`, with a shipped exemplar rather than a sketch. `b9cc75b4` (patch-id `5a5c761072c44b54dc80f224d89355dd2d31e498`) closed one member and demonstrates the three moves `OB-6` prescribes: a **third state** for *indeterminate* rather than defaulting to the strong claim; **omit rather than zero** an unmeasured quantity; and **extract the proxy→claim mapping into a pure function**, which is the check itself — the hardcoded `"up_to_date"` survived because its arm sat behind a live client constructor no test could reach, and a pure function admits a table test whose `None` row is the one that was missing. Nothing yet applies the pattern corpus-wide.
**Valid:** dated 2026-08-31

Every member is one substitution. `workspace(post_compact=true)` cannot check that a compaction happened, so it trusts the caller's flag and clears the whole ledger on a mistaken `/mcp` reconnect. The rendezvous gate cannot check that the companion hook is still alive, so it trusts a monotone stamp that nothing can un-set — and its twin defect is the same stamp never landing, which leaves the gate shut for the life of the process. The subagent ledger cannot see what the parent holds, so 84% of measured subagent sessions re-receive a topic their parent already has. `get_guide` cannot see which section a caller needs, so it serves the topic — or, since section grain shipped, serves the section attached to the response of the call it was meant to inform.

The unifying property is that **none of these fail loudly**. A wrong proxy produces a plausible delivery: an extra guide body, an open gate, a closed gate, a re-cleared ledger. Nothing throws, so nothing downstream fires either. That is what makes the class survive review chains that catch louder bugs, and it is why `rendezvous-slot-never-stamped` could be closed `wontfix` on the reasoning that the failure is invisible — correct about the observation, and exactly the property that should have counted against it.

Note the shape shared with `cluster/blast-radius-exceeds-visibility`: both are *a component reasoning about a scope larger than the one it can observe*. They are kept separate because the remedies differ — that one needs an ownership protocol over a shared resource, this one needs an authoritative signal for an event. Merging them would produce a class too abstract to prescribe anything.

**Falsified by** a member whose proxy failure surfaced as an error rather than a plausible result; that would belong to an ordinary-correctness class instead.

**The 16 members split on one question, and the split predicts their fate: does the substrate
that would answer the gate exist at all?** **Filesystem/repo-scoped (7)** — git sync, index
coverage, lock state, an external checkout, a `build.rs` snapshot, a worktree flag, a test-mode
env var — are cases where the process *could* have looked and did not. **All seven are closed**
(six fixed, one wontfix). **Harness/peer-scoped (9)** — a compaction, a `/clear`, an `/mcp`
reconnect, what a parent session holds, whether a companion hook is alive — are cases where the
process holds no channel to the fact. **Five of those nine are still live**, including all four
open members and the one `investigating`.

That asymmetry is the argument for `OB-6`'s first move rather than an excuse for the backlog.
Where the substrate exists, this class is an ordinary bug and the corpus fixes it. Where it does
not, **a third state is still available even when the event is not** — a gate that cannot tell
can say so, and the harness-scoped members are open precisely because each one instead picked a
proxy and reported its output as knowledge.
## IC-3 — declaration is not execution — a surface declares a capability production never reaches

**Slug:** `cluster/declared-not-wired`
**Claim:** A surface declares a capability that production never reaches. Every piece is individually correct — the selector, the matcher, the ledger entry, the schema — and no call site connects them, so the declaration reads as a shipped feature and tests pass in isolation.
**Members:** `filter={"tags": {"contains": "cluster/declared-not-wired"}}` — **n=24, 2026-09-02, re-derived** by anchored file count. The 24th is `docs/issues/2026-09-02-recoverable-error-outcome-is-unreachable-in-production.md`: `usage.db`'s `outcome` column declares a three-value taxonomy and two SQL queries filter on `recoverable_error`, which production **never writes** — 0 rows in 57k calls, because `record_content` classifies `Err(RecoverableError)` as `"error"` before `route_tool_error` runs. The tests that pin the value call `classify_content_result` directly and never traverse that ordering. Everything from here on describes the population up to 23. Superseded figure, kept for its derivation: `n=23`, 2026-09-01, by query. Two movements the same day, opposite directions: 20 → 18 when the `IC-15` boundary was settled on the remedy test (two members that accept a caller's value and drop it moved there; see the Index), then 18 → 20 as two archive additions landed after the settlement — `docs/issues/archive/2026-09-01-listfunctions-and-listdocs-are-unregistered-tools.md` and `docs/issues/archive/2026-09-01-graft-requires-two-params-the-schema-never-advertises.md`. The three-family split in **Mechanism status:** was measured over the 18 and does not yet place these two. Then 20 → 22: the blind second read moved `docs/issues/archive/2026-08-30-cli-artifact-drops-time-scope-and-extra.md` here from `IC-15` — no CLI flag exists, so nothing is accepted to be dropped, which is the settled discriminator and makes the file consistent with its structural twin `cli-doctor-exposes-no-fix-flag` — and a peer session filed `docs/issues/archive/2026-09-01-git-apply-cached-stages-but-records-no-owner.md` while the audit was running. Then 22 → 23: `docs/issues/archive/2026-09-01-heading-miss-discards-the-available-headings-hint.md`, filed 2026-09-01 (fixed and archived the same day at `a35a9c35`, patch-id `4f7f84946b904368bb2b5c01593c4a7ad3899be8` — and the fix found a **second** unwired site the file never named, the plural `headings=[…]` branch, which builds its own missing-member list and never calls the helper) — `RecoverableError::with_hint` builds an "Available headings" hint at `src/tools/file_summary/file_summary.rs:447-450` and `heading_miss_meta`'s absent arm never reads it, while its *ambiguous* sibling three lines above does. The filer first flagged the tag as contestable on in-code-vs-surface grounds, then settled it against this ledger's own discriminator — *what fix does the defect require?* The repair wires an existing in-code declaration to a live route (one line into `heading_miss_meta`'s absent arm); nothing new is declared at any surface. That is IC-3's remedy, so the tag holds. Description decided nothing; the remedy did.
**Blind party:** the author of the declaration, specifically. They hold the mental model in which the wiring exists — writing `**Serves:** edit_file(path~/.claude)` *is* the act of believing it is served. A more careful version of the same author writes the same line.
**Promotes to:** `OB-7` — *a declaration is well-formed, and nothing in production reaches it*, `docs/trackers/observer-blindness.md`, promoted 2026-09-01. `OB-5`'s *Known-open residual* is this class seen from the **reporting** side — a check whose `extend()` line is deleted still reports `0` because the enum still declares it — so the two are cited across rather than merged: `OB-5` is about a summary that cannot say what **ran**, `OB-7` about a capability nothing **reaches**. The residual is where they touch, not where they are the same, which is why this got its own row instead of folding in.
**Mechanism status:** `partial` — decidable for one of three families, and deliberately **not** `designed` for the class. The members split by what disconnects the declaration — **20 of the 23.** The partition below was measured over **18**; four arrived after it (2026-09-01, named at the end of this field), of which two slot into families and two fit none of the three, so what follows is a partition of 20 with a named remainder rather than of the class. **Dead in production (10):** the code exists and only tests call it. The partition is measured — `grep` tags every hit with its enclosing symbol and test sites carry a `tests/` prefix, and call-site granularity is *required* rather than preferable, since `references` groups `src/librarian/adapter.rs:1451` under a production file while `grep` shows it is `tests/…`. But it decides for **by-name call sites only**. *Corrected 2026-09-01 from a dispatch-side probe — this sentence used to name `dyn Trait` and `Arc<dyn CodeEmbedder>` as the blind spot, and that is false*: a trait object must be **constructed**, and construction is by name (`Arc::new(Grep)`), as is delegation (`"register" => RegisterLibrary.call(…)`). Dispatch consumes a name; it does not erase one. The genuine false-positive surface is **macro-generated names and re-export-only aliases**, much smaller than "every trait object" — and the failure direction is still the dangerous one, since a false *dead-in-production* finding is a **deletion-authorising** result on a negative search. The zero-caller state is no longer unexercised: the same probe found the first true positive (`ListFunctions`/`ListDocs`), but `references` was unavailable on it, so the finding rests on the text instrument alone. See `cluster-promotion-session-log:F-1` and `OB-7` § *PROBED*. **Schema or doc declares what the code ignores (3):** a round-trip check is only a weak proxy here, because reading a field is not using it. **A matcher that can never match (7):** this entry's original phrasing, needing the set of values production emits at a call site, and it has no mechanism at all. Recording the class as `designed` on the strength of the first family is exactly the conflation `IC-9` was corrected for.

**The four post-partition arrivals, assigned 2026-09-01 — two fit, two do not.** *Into `Dead in production`, taking it to 10:* `2026-09-01-listfunctions-and-listdocs-are-unregistered-tools.md` — both implemented the `Tool` trait, `src/server.rs` registered neither name, and their own test suite was the only caller (both deleted the same day; `symbols(name="ListFunctions")` now returns 0). *Into `A matcher that can never match`, taking it to 7:* `2026-09-01-git-apply-cached-stages-but-records-no-owner.md` — `apply` is listed among the seven verbs that may claim a staged pair, and `argv_paths()` returns the patch file rather than the staged path, so `names_path()` cannot match on any input. **The other two are left unassigned rather than forced into the nearest family**, because a wrong family is a wrong input to the mechanism question this field exists to answer: `2026-09-01-graft-requires-two-params-the-schema-never-advertises.md`, where the action is advertised and its two **required** params are not, so the declaration is reachable only through keys it does not publish — not dead code, not an unmatchable matcher, and not a declared-then-ignored field; and `2026-08-30-cli-artifact-drops-time-scope-and-extra.md`, where the capability is reachable from the MCP surface and unreachable from the CLI, which is a **surface-parity** shape it shares with `2026-08-30-cli-doctor-exposes-no-fix-flag.md` already inside the class (both kept in `IC-3` by the `IC-3`/`IC-15` boundary ruling: no flag exists at the boundary, so nothing is accepted to be dropped). Two members sharing one shape is what a fourth family would be built from; naming them is the input to that call, not the call.
**Valid:** dated 2026-09-01

`op-4-path-predicate-can-never-fire` and `triggered-operator-rules-route-nothing-in-production` are the pure form: three operator rules declare `binding: triggered` against tools that emit no `selector_key` in production, so `route()` is never called with anything that could match them. The routing mechanism exists and is unit-tested; the tests construct the selector the production path never produces.

`cli-doctor-exposes-no-fix-flag` is the same shape at a different seam. `librarian(action="doctor", fix=…)` offers six repairs; `codescout doctor`'s clap struct offers none, so every repair is unreachable from the command line. Its own body names this as *"the third instance today of one mechanism: the CLI keeps its own clap structs and hand-marshals into the MCP tool's JSON"* — which makes it a member of both this class and a narrower CLI/MCP parity family. It is filed here because the *defect* is the unreachable capability; the hand-marshalling is the mechanism by which it became unreachable.

The reason ordinary testing does not catch this class is structural rather than accidental: a unit test constructs its own inputs, so it exercises the matcher with a selector production never emits, and passes. The test is not weak; it is *scoped to the half that works*. Only a check that starts from the production emission side can see it — which is the same shape as `CLAUDE.md` § *Testing Discipline*'s "name the concrete caller that reaches it".

**Falsified by** a member where the wiring existed and the declaration was merely wrong, which is an ordinary bug rather than this class.

**The compiler is a blind party here, literally, and it explains the dead-in-production family's
survival.** Rust's `dead_code` lint cannot fire on `pub` items in a **library** crate: it does
not know the crate's consumers, so it must assume reachability. codescout is lib-plus-bin, so
every `pub fn` under `src/` is exempt **by construction**, however many callers it has. That is
this ledger's own structure appearing in a tool rather than a person — the party best placed to
notice holds a parameter (the set of external consumers) that makes noticing impossible — and it
is why nine members sat under `-D warnings` on every gate run for months without one warning.
The remedy is not a stricter lint setting; it is a different question, asked from the caller
side.

**The `IC-15` boundary, raised before the archive tagging pass and settled during it — the rule
matters more than the outcome.** Members here stated `IC-15`'s claim (*a parameter accepted at
the boundary and silently dropped*) rather than this one's, because this entry files by
**defect** — the unreachable capability — as its cli-doctor paragraph says, and under that rule
every `IC-15` member is also one of these, which would have made `IC-15` a sub-family rather than
a peer.

**The remedy test settled it, as it did for `IC-1` vs `IC-2`, and it reduces to one question you
can ask of a file: was a caller-supplied value accepted?** If **yes**, the code path *ran* and
discarded the value; the remedy is to round-trip it or refuse it — `IC-15`. If **no**, the
capability exists and no call site reaches it; the remedy is to find a caller — `IC-3`. Two
different remedies, so two classes, and this entry's file-by-defect rule was what needed
narrowing.

Applied to the twenty, **exactly two moved**: `audit-doc-refs-scope-param-ignored` and
`audit-doc-refs-fail-on-doc-mismatch`, where a caller passes `scope`, or `fail_on: med`, and the
value is taken and dropped. `cli-doctor-exposes-no-fix-flag` **stays** — the flag does not exist
at the boundary at all, so nothing is accepted to be dropped — and `audit-doc-refs-lsp-stubbed-off`
stays for the same reason. `constitution-rule-malformed-glob-silent-fail-open` and
`drift-detection-enabled-is-a-dead-config-key` were left here as **visibly undecided** rather
than moved on a coin-flip; both are defensible either way and neither has been probed.

Note what the discriminator is *not*: all of these read as *"declares X but does not do X"*, a
sentence fitting at least four classes in this ledger. Matching on it is how `IC-9` acquired two
misfits, and the **remedy**, never the description, is what separates them. One weak
corroboration that the pass was claim-based rather than title-based: the moves were predicted to
fall in the *schema-declares-what-code-ignores* family if titles had driven the classification,
and they did — but only **2 of that family of 5**, which is the signature of a boundary gap
rather than of systematic leakage. That is not an independent check and is not offered as one;
classification and application were the same party throughout.
## IC-4 — config propagation is additive — updates land, removals and renames do not

**Slug:** `cluster/config-propagation-is-additive`
**Claim:** Configuration propagates as an overlay rather than a replace. An added or changed key lands; a **removed** key, or a **renamed** path, does not — and the change that does land is read as confirmation that the whole edit applied.
**Members:** `filter={"tags": {"contains": "cluster/config-propagation-is-additive"}}` — n=8, 2026-08-31, by query after archive backfill.
**Blind party:** the operator who made the edit. They verify the change they can see — the new value — and that verification is *positive evidence for the wrong proposition*. Nothing in the successful check distinguishes "the edit applied" from "the additive half of the edit applied".
**Promotes to:** `OB` — `docs/trackers/observer-blindness.md`, adjudicated 2026-08-31. This field read *"three instances across three subsystems (MCP env, shell env, git config)"* until then — a count from before the archive backfill. It is **8**, spanning MCP env, shell env, git config, worktree gitdir, hook scripts, sweep scripts, the env-copy flow and memory keys. **Unlike `IC-5` and `IC-6`, this class passes OB's admission test**: `Blind party:` names one — the operator who made the edit — and the failure is a plausible answer (the additive half landing) rather than an error, which is the routing table's first row. The earlier *"arguably `H` rather than `OB`"* conflated *where the class is recorded* with *what the remedy is*: an `H` hook diffing intended against effective config is the mechanism this class owes, and `IC-1`/`IC-2`/`IC-3` likewise sit in `OB` with mechanisms outstanding. Record in `OB`; build the hook.
**Mechanism status:** `partial` — two surfaces of eight, both closed 2026-09-02. *(This row read `none yet` in the Index table while this field read `partial`, from 2026-09-02 until the second surface landed the same day. The count columns are gated by `tests/issue_clusters.rs`; the mechanism column is not, and `no_mechanism_status_is_a_bare_verdict` scans `**Mechanism status:**` field lines only — a table cell is outside it. Named rather than silently corrected, because it is this ledger's own instance of `IC-18`.)* **Surface 1 —** `core.hooksPath`. `tests/hook_config.rs::a_set_core_hookspath_must_point_at_a_directory_that_exists` fails when `core.hooksPath` names a directory that does not exist, which is the **rename form** this entry calls recurrent (`core-hookspath-points-at-pre-rename-path` and its archived sibling `bench-worktree-gitdir-points-at-pre-rename-path`). *The check itself already existed* — `scripts/install-hooks.sh` has carried the trap and a `--check` mode since the original fix — but **nothing ran it**: no CI job, no test, no task-runner reference, only a line of prose asking the operator to run it after a clone. That is a policy, and this class's `Blind party:` is the operator, so a policy addressed to them is addressed to the party defined as having no signal. What shipped is not a new check but an unprompted one. **A test rather than a hook, necessarily:** a hook cannot verify that hooks are wired — if the wiring is broken it does not run, and its silence is indistinguishable from its approval. Measured 2026-09-02 in a throwaway repo: under a stale `core.hooksPath`, a `pre-commit` hook whose only job is to `exit 1` never runs and the commit succeeds. Not a weakened guard — an absent one, reported as success. **Surface 2 — a linked worktree's forward pointer**, closed 2026-09-02 by `tests/config_propagation.rs::no_linked_worktree_points_at_a_gitdir_that_is_gone`. `git worktree add` writes an **absolute** `gitdir:` into `<worktree>/.git`, so renaming the repo kills it while the main checkout keeps working — this claim with nothing left over. **The measured surprise is that git is not blind to this; its report EXPIRES.** Probed 2026-09-02 in a throwaway repo: immediately after the rename `git worktree list` does show the entry, tagged `prunable gitdir file points to non-existent location` — loud, and repairable. Then `git gc` runs `git worktree prune --expire 3.months.ago` on its own (`gc.worktreePruneExpire`) and **deletes the admin directory**, for a worktree whose files are still on disk, because git is judging it by a path that moved. After that the entry is absent from `git worktree list`, `.git/worktrees/` is gone, and `git -C <worktree>` answers `fatal: not a git repository: (null)`. So a gate built on `git worktree list` would start passing at precisely the moment the defect became invisible — the instrument that reports it is the one that later erases the evidence, which is why the test scans the filesystem. This also **completes the archived instance's record**: `docs/issues/archive/2026-08-16-bench-worktree-gitdir-points-at-pre-rename-path.md` documents the post-expiry state (no `bench` admin dir, absent from the list, every corpus file present) without the prune step that produced it, and its own `unverified:` field reports the defect still live 14 days after closure. Two further measurements went into the failure message because the obvious remedy is a silent no-op: **bare `git worktree repair` exits 0, prints nothing and fixes nothing** — the absolute-path argument is load-bearing — and running it from *inside* the broken worktree cannot work at all. Verified by mutation over the live corpus rather than by fixture alone: a manufactured orphan carrying the archived bug's own pre-rename `code-explorer` path reds the gate, and its removal greens it. **The first version of this mechanism was itself an instance of this class, and the second is not** (`1428faa9`). It scanned a hardcoded `SCAN_ROOTS = [".worktrees", ".claude/worktrees"]` — the only two roots the whole corpus mentions, established by grep rather than guessed, and still wrong: a new tool adding a third root propagates into the tool and not into the list, `read_dir` on an absent root returns `Err` and is skipped in silence, and the gate reports green over a place it never opened. That is `IC-18` wearing this entry's own decay. Replaced by a walk from the repo root keyed on `.git` being a *file*, so a worktree is found because it **is** one rather than because someone predicted where it would live; measured, it catches orphans at `.superpowers/worktrees/` and at an arbitrary depth-4 path, neither reachable by the old list. **The walk needed a positive control**, because the primary check is an absence assertion (`broken.is_empty()`) and a walk that finds nothing satisfies it perfectly — so `the_walk_finds_every_worktree_git_itself_reports` cross-checks discovery against `git worktree list`, an instrument reading admin dirs rather than the filesystem. Verified by mutating the control rather than by writing it: adding `.worktrees` to the prune list left the primary check **green over zero coverage** and reddened the control. Note the trap the walk had to avoid — `ignore::WalkBuilder`, this repo's usual walker and already a dependency, honours `.gitignore`, and `.worktrees/` is gitignored, so the obvious tool skips the one directory the check exists for and returns a clean zero. **What remains, re-derived 2026-09-02 against the member list rather than carried — the previous sentence here read *"Six surfaces remain and none is covered"* and named seven things, which is the wrong count of the wrong quantity.** The eight surfaces are the eight tagged members, and by `status` they are **7 fixed, 1 open**. So "remaining" splits three ways, and only one part is anybody's work:

| | surfaces | state |
|---|---|---|
| **guarded** | `core.hooksPath`, worktree gitdir | fixed **and** a standing check; 2026-09-02, this entry |
| **open** | MCP env (`docs/issues/2026-08-30-mcp-reconnect-applies-env-updates-but-not-env-deletions.md`) | the only live defect — and tagged `not-codescout-source`: a Claude Code harness behaviour, **not fixable from this repo**, so it is not buildable work here either |
| **fixed, unguarded** | sweep scripts, the env-copy flow, hook/state-protocol scripts, memory keys, shell env | closed once, nothing stops a silent regression |

**All five were then measured 2026-09-02, and not one has a live population to guard.** *Sweep scripts*: `scripts/` contains **zero** hardcoded `/home/` or `/Users/` absolute paths. *Hook scripts*: probed in a throwaway repo, a `language: system` entry naming a missing script fails **loud** — `Executable scripts/does-not-exist.sh not found`, hook `Failed`, commit refused — a fail-closed error, therefore **not this class at all**, whose signature is a plausible answer rather than an error. *Env-copy flow* and *shell env* share one population, the repo-root `.env`: its `CODESCOUT_MODEL_DIR` now reads `./models` and resolves, so the archived `/home/marius/models` value is gone. **And its `CODESCOUT_RETRIEVAL_PROFILE=amd` is NOT the stale value it looks like** — `lspci` reports both an NVIDIA RTX A5000 and an AMD Radeon RX 7700/7800 XT in this box, so `amd` is defensible here; the archived bug's *"this laptop has an NVIDIA GTX 1660 Ti"* finding was about a **different machine**, and reading it as a live defect would have been `R-172` a second time in one hour. *Memory keys* is the one the instrument cannot settle: `memory(recall)` returns nothing for `prompt-tdd-skill-eval-confounds` above 0.35 under codescout's scope — but **a recall miss and a deleted point are indistinguishable**, so this is *unresolved*, not clean, and saying "fixed" would be an absence assertion over an instrument that cannot express the failure. **Net: four surfaces have no population, one is unmeasurable with what exists, and the single open member is a harness defect out of this repo's reach — so "build the next IC-4 surface" is not available work, and the guard shape this class actually owes is a *population*, not a check.** Building either of the first two would have been a guard over an empty set and a guard over a non-defect, both claiming coverage. The general remedy this entry names — a diff of intended against effective config — is still owed; what is now demonstrated is that the *shape* is buildable and that the honest unit is one surface at a time.
**Valid:** dated 2026-08-31

`mcp-reconnect-applies-env-updates-but-not-env-deletions` is the sharpest statement of the class, because one edit made two changes and the outcomes diverged: `CODESCOUT_BM25_BOOST` changed from `3.0` to `5.0` and landed; `CODESCOUT_QUERY_PREFIX` was removed and is still set. The bug file's own title names the trap — *"the change that lands falsely confirms the one that did not"*.

`stale-model-dir-env-masked-by-shell` is the same asymmetry one layer down, and it is worth noting that its two stale values were masked for *different* reasons, which is why a single-cause fix would have missed one. `core-hookspath-points-at-pre-rename-path` is the rename form: `.git/config` still points `core.hooksPath` at the repository's former name, so git finds no hooks at all and `.pre-commit-config.yaml` silently never fires — while `CONTRIBUTING.md` documents the hook's behaviour in the present tense. Its archived sibling (`bench-worktree-gitdir-points-at-pre-rename-path`) is the same mechanism, which is what makes the rename form recurrent rather than incidental.

The generalisation worth extracting: **a config surface that supports removal needs a check on the removal, because the update path is the one everybody exercises.** Silence after a deletion is indistinguishable from success, and the value that remains is a live setting nobody intends.

**Falsified by** an instance where a removal *did* propagate and the failure lay elsewhere.

## IC-5 — the reproduction environment is not the gating environment

**Slug:** `cluster/repro-env-diverges-from-gate-env`
**Claim:** The environment built so failures can be reproduced locally is not the environment that gates. While the two agree the divergence is invisible; when they disagree the local run is authoritative-looking and wrong, and a genuine platform defect is indistinguishable from an environment gap.
**Members:** `filter={"tags": {"contains": "cluster/repro-env-diverges-from-gate-env"}}` — n=13, 2026-09-02, by query. **The 13th is `docs/issues/archive/2026-09-02-lockfileex-refuses-an-append-only-handle-on-windows.md`, and it is this claim in its purest form yet:** `export` opens each shard file append-only and then locks it, and `fs4`'s `LockFileEx` **requires the handle to carry `GENERIC_READ` or `GENERIC_WRITE`** — which std's append-only access mode (`FILE_GENERIC_WRITE & !FILE_WRITE_DATA`) is neither. Every export dies on `Access is denied. (os error 5)`. **21 tests red on every `windows-latest` lane and 2 on `windows-gnu`, with Linux and macOS entirely green**, because `flock(2)` ignores a descriptor's access mode and Windows' `LockFileEx` does not — so no local run, on any developer machine, can express the failure. The merge commit was reported as *"gate green"* — **true of the gate that ran**, false of CI, and that gap is the entry rather than an authoring error. It is also the first member found *by* this entry's own remedy work: the wine pin (`58d85263`) needed a lane run, and the run that verified the pin surfaced this. **This entry asserted a different root cause for about an hour** — a forward-slash path literal — and that reading was *falsified by shipping it*: the fix landed at `9a156bd4` and the next run failed identically, 21 tests, same names, same lines. The wrong cause came from generalising a 2-of-21 sample taken from the **wine** lane while the 19 native-Windows panics went unread, which is this class one level up: a sample drawn from the non-gating environment, describing the gating one.
**Blind party:** `none — ordinary design defect`. A careful engineer comparing wine versions catches this; nobody is structurally prevented from seeing it. Recorded so the class is not mis-promoted to `OB`, whose admission test it fails.
**Promotes to:** `H` — `docs/trackers/codescout-usage-hookify.md`, adjudicated 2026-08-31 after the archive backfill. **Not `OB`**: `Blind party:` is `none`, which fails OB's admission test, so the routing question was never open. The old note read *"clears the count but not the subsystem spread; all three sit in the Windows/wine lane… Revisit if a fourth lands outside it"* — eight have. The twelve members span **seven** subsystems: cargo feature config (3), wine/Windows (4), shell environment (1), cargo workspace resolution (1), toolchain (1), ambient embedder config (1), test-suite parallelism (1). **It promotes as a worklist item, not a rule**, because `Mechanism status:` is still `none yet` and this ledger holds that a rule without one produces advice. The mechanism shape is a check diffing the documented four-command gate against `ci.yml`'s matrix; the four Windows lanes red on 2026-08-31 are a live instance of exactly what that would have caught.
**Mechanism status:** `shipped (partial)` — `scripts/build-windows.sh` (`4816d64f`) prints this box's `wine --version` and names where CI's is decided, at the top of every `test` run, so a local green and a lane green are comparable rather than conflatable. *This field read `none yet`, and proposed asserting the CI version "at ~3 lines", until 2026-09-02. Both halves were stale, and the second was **refuted** rather than merely superseded: the script deliberately does NOT assert a version, because `ubuntu-latest`'s wine moves and a hardcoded `9.0` "would be a constant that decays while still reading as fact — the shape docs/trackers/issue-clusters.md files as IC-11". The code declines this ledger's proposal by citing this ledger's own class.* Probed with `git log -S` per `IC-11`'s inclusion test rather than assumed: the field was written at `351836a8` (08-31 22:39) when no mechanism existed, and the script shipped at `4816d64f` (09-01 00:44) — **true when written, decayed two hours later**, which is an `IC-11` instance and not an authoring error. What remains is the CI half: nothing catches the divergence *inside* the lane, and `.github/workflows/ci.yml` records the un-skip protocol that would close it — pin the lane's wine to the local version via the WineHQ apt repo, then drop both skips and re-measure. That is the pin, not an assertion, and it is not ~3 lines.
**Valid:** conditional — a member appears outside the Windows/wine lane

`scripts/build-windows.sh` exists precisely so Windows failures are reproducible without CI round-trips, and that purpose holds only while the two wines behave alike. They do not: `ubuntu-latest` packages wine 9.0, a current dev box runs wine 11.16, and in a single day the gap produced two divergences — one costing a CI cycle, one still costing a skipped test.

The other two members are the downstream cost of that gap rather than separate defects. `wine-lane-flakes-under-load` records three tests that failed together and passed on the next identical run, and its own update narrows the file to one test after CI reproduced two of the three *with a different payload* — a distinction only visible because someone compared the two environments deliberately. `windows-ci-timing-flakes` is `zombie` for the honest reason: both flakes resolve only by recurring, so no amount of effort reaches them.

This class is deliberately kept even though it does not currently promote. Its value is the **threshold rule's worked negative example**: three instances is not enough when they share a subsystem, and recording that judgement is what stops the next reader counting to three and promoting anyway.

**Falsified by** the two wine versions being shown to agree on the divergent cases, which would relocate the defect to the tests themselves.

## IC-6 — an addressing scheme with no escape hatch and no disambiguator

**Slug:** `cluster/addressing-without-an-escape-hatch`
**Claim:** An addressing scheme interprets every token in its namespace and provides no way to write one literally, or to disambiguate two that collide. The scheme is correct on every input it accepts; the defect is the input it makes unrepresentable.
**Members:** `filter={"tags": {"contains": "cluster/addressing-without-an-escape-hatch"}}` — n=30, 2026-09-01, by query. Three beyond the 2026-08-31 adjudication's 27: `docs/issues/archive/2026-09-01-an-unbalanced-fence-silently-disables-every-line-anchored-field.md`, `docs/issues/archive/2026-09-01-staging-op-reads-a-detached-flag-value-as-the-subcommand.md`, and `docs/issues/archive/2026-09-01-status-locator-reads-any-table-row-as-a-status-row.md` — all post-date the promotion, which rested on `n=27`, so the count moving up only strengthens it. The third is the disambiguator half in its purest form and was filed by a peer session while this ledger was being re-derived, which is the standing argument for re-running the query rather than reading the cell.
**Blind party:** `none — ordinary design defect`. The gap is visible to anyone who tries the unrepresentable input; nobody is structurally prevented from seeing it. Recorded so it is not mis-promoted to `OB`.
**Promotes to:** `CLAUDE.md` § *Parsers Over a Namespace — owe an escape and a disambiguator* — **landed 2026-08-31**. Adjudicated the same day, after the archive backfill took it from `n=2` to `n=27`, the largest class in the corpus. **Not `OB`**: `Blind party:` is `none`. It is codescout-specific engineering discipline, statable as one rule — *a parser over a namespace owes an escape for writing a token literally, and a disambiguator for two that collide.* Five subsystems: file-format navigation (`json_path`, `toml_key`), markdown editing (fences, heading-shaped content), the link/citation resolver (frontmatter delimiters, qualifiers, prefix collisions, doc-examples-read-as-citations), shell command gates (IL-3, dangerous-command, source gate, `run_command` — four separate gates, every one of them on heredocs), and symbol navigation (`name_path` with no disambiguator). Unlike IC-5 this one has partial mechanism already shipped, so the rule has something behind it.
**Mechanism status:** `shipped (partial)` — `edit_markdown` and `artifact(get)` gained an `occurrence` selector, closing the heading half for librarian-managed files. The `link_scan` half has no escape syntax at all.
**Valid:** dated 2026-09-01

Two instances, one shape. `identical-headings-make-a-section-permanently-unaddressable`: two byte-identical headings make both sections unreachable, and the refusal is *correct* — the query is genuinely ambiguous. The defect was that no disambiguator existed, compounded by an error hint prescribing `start_line`/`end_line` parameters absent from the schema of a tool gated off `.md` files entirely. A wrong remedy in an error message is worse than none, because it costs the reader a round trip before they distrust it.

`an-entry-id-cannot-be-mentioned-without-citing-it`: `link_scan` derives a `cites` edge from any `\b[A-Z]{1,3}-\d+\b` token in a body, and there is no way to write an id as a literal. Inline backticks — the notation an author reaches for to mean *"this is a token, not a reference"* — are deliberately scanned. The only escape is a fenced block, which cannot sit mid-sentence. The consequence is narrow and self-demonstrating: a document about id allocation cannot name an id without citing it, which is why this ledger's own template writes its example inside a fence.

The generalisable point is that **an interpreting scheme owes a quoting mechanism**, and the cost of omitting one is paid by exactly the documents that discuss the scheme — the ones most worth writing.

**Falsified by** a third instance in an unrelated addressing scheme, which would raise this to threshold rather than falsify it; falsification proper would be showing the two members share no mechanism.

## IC-7 — lazy warm-up bills the first caller, and the bill can look like a negative result

**Slug:** `cluster/lazy-warmup-bills-the-first-caller`
**Claim:** Work deferred to first use is charged to whichever call arrives first, and that caller has no way to distinguish "cold" from "broken" — so the bill can surface as a timeout or as a negative result that looks like an answer.
**Members:** `filter={"tags": {"contains": "cluster/lazy-warmup-bills-the-first-caller"}}` — n=4, 2026-08-31, by query after archive backfill.
**Blind party:** the caller. It receives `symbol not found` or `0 matches` — well-formed, plausible results that are *indistinguishable from the true answer*. This is the property `docs/adrs/2026-08-27-negative-results-name-their-scope.md` exists to address, which makes the class an argument for that ADR rather than a new one.
**Promotes to:** `not yet` — the count is met but two of four members are `zombie` with unconfirmed root causes, so promoting now would rest a rule on unresolved premises. (This field read *"two of three"* until 2026-08-31, from before the archive backfill; the fraction moved, the argument did not.)
**Mechanism status:** `shipped (partial)` — a false-zero guard covers the cold-start path for `references`; the deliberate cold-start reproduction showed the guard *does* fire there, which refuted the original filing.
**Valid:** conditional — either zombie member recurs

`post-compact-flush-leaves-first-nav-call-to-pay-cold-start` is the unambiguous member and the loudest: `workspace(post_compact=true)` flushes every LSP client and returns a hint promising *"no disruption to the session"*, then the first navigation call pays the entire cold start — on this 1697-file crate, past the 60s tool timeout, dying with no result. The mechanism is not in dispute; the hint's claim is simply false for the first caller.

The other two are `zombie` and are held here **with their uncertainty intact**, which is the point of recording them rather than the reason to leave them out. `references-symbol-not-found-while-lsp-warms` was filed with a root cause that has since been **refuted** — a deliberate cold-start reproduction showed the cold path produces the *guarded* false-zero, i.e. the opposite of the filed mechanism. `symbols-overview-include-body-ignored-and-search-flake` has one bug fixed and one mitigated-and-instrumented, unrecurred, root cause unconfirmed.

So the class is real and its evidence is thin in a specific way: one confirmed instance and two whose mechanism is unproven. That is exactly the state the threshold's second condition is meant to catch, and the honest disposition is to hold rather than promote. If either zombie fires again with instrumentation attached, this becomes promotable in one step.

**Falsified by** the two zombies resolving to unrelated mechanisms, which would leave one instance and no class.

## IC-8 — a record asserts a completed action that nothing re-checked

**Slug:** `cluster/record-asserts-an-unchecked-completion`
**Claim:** A record states that an action completed, and nothing is scheduled to re-check it. The assertion is written at the moment of *intent* and read forever after as *outcome*, so a step that silently did not happen leaves a closure note that reads exactly like a successful one.
**Members:** `filter={"tags": {"contains": "cluster/record-asserts-an-unchecked-completion"}}` — n=5, 2026-08-31, by query after archive backfill.
**Blind party:** the reader of the record, who has no signal distinguishing a verified closure from an asserted one. The author is not blind — they simply wrote what they intended to do.
**Promotes to:** `DC` — `docs/trackers/claim-decay.md`, whose `undecidable-green` and `premature-assertion` types already name this shape. This class is the bug-corpus entry point to that ledger, not a competitor.
**Mechanism status:** none yet — **not checked against the code as of 2026-09-02**, so read this as an open question rather than an established absence.
**Valid:** dated 2026-08-31

One member, kept because the instance is unusually instructive. `bench-worktree-deletion-recorded-as-done-never-happened`: an archived bug file is `status: fixed`, `closed: 2026-08-16`, and its `## Fix` section states the worktree *"was removed with `git worktree remove --force .worktrees/bench`"*, closing with *"174 MB reclaimed, 163 MB of it regenerable `.codescout` index state."* The directory is still on disk. It is 174 MB, of which 163 MB is `.codescout`.

**The numbers match the live directory exactly, and that is the finding.** They were measured — correctly — *before* the deletion, then written in the past tense. So the record is not fabricated and not sloppy; it is a true measurement placed under a false verb. No plausibility check catches it, because every figure in it is right.

This is filed as a class of one deliberately rather than folded into `DC`. The threshold is not met and it will not promote on its own, but the *bug corpus* is where instances arrive, and a class with a defined slug is what lets the second instance find the first. If it stays at one for a long while, the correct disposition is to retire it here and keep the analysis in `DC`.

**Falsified by** the closure note turning out to have been written after a deletion that was later undone, which would make it a lost-work bug rather than an unchecked assertion.

## IC-9 — an assertion over environment-controlled text is satisfiable by accident, and fails open

**Slug:** `cluster/assertion-satisfiable-by-accident`
**Claim:** An assertion whose haystack embeds environment-controlled text — a path, a tempdir name, a hostname, a timestamp — can be satisfied by coincidence. It fails **open**: it passes on almost every machine and almost every run, so the green tick is evidence of luck rather than of the property.
**Members:** `filter={"tags": {"contains": "cluster/assertion-satisfiable-by-accident"}}` — n=2, 2026-09-02, by query. Went to 3 in the archive backfill and back to 1 the same evening: two of those tags were misfits and were withdrawn, see **Promotes to**. **The 2nd is `docs/issues/2026-09-02-a-test-fixture-interpolates-a-path-into-json.md`, and it matches this claim's own wording rather than its spirit** — the claim names *"a path, a tempdir name"* as the environment-controlled text, and that is literally what decided the outcome. Two fixtures built an audit payload by interpolating `tmp.path().join("gone.md")` into a JSON string literal. On Unix the tempdir path is `/tmp/.tmpAbC/gone.md` and the JSON parses; on Windows it is `C:\Users\RUNNER~1\…`, where `\U` and `\A` are illegal JSON escapes — so parsing failed, the row came back `unattributed`, and 2 tests reddened on all four Windows lanes while every ubuntu and macos lane stayed green. **The fixture's validity depended on the alphabet of a directory name it did not choose.** It also sharpens the class's *fails-open* clause: the green tick was luck about a character set, and the luck holds on every developer machine, so no amount of local re-running raises the odds of catching it. **Unlike most members, this one is guardable where it is invisible** — the trigger is the CONTENT of the path rather than the platform, so `a_backslash_path_survives_the_delete_payload_round_trip` states a Windows-shaped path outright and reds on Linux under mutation. The remedy this class implies is exactly that: *state the hostile input, never inherit it from the environment.*
**Blind party:** `none — ordinary design defect`, but with an unusually strong *detection* asymmetry: at ~1-in-800 the failure is unreachable by local reproduction, so the author's evidence is necessarily circumstantial. The file records that honestly in its own `unverified:` field.
**Promotes to:** `not yet` — n=2. When it moves, the target is `I` (`docs/trackers/test-escape-hardening.md`), because the remedy is a standing check rather than a rule anyone remembers — but see **Mechanism status**: the check is not the grep this entry used to name. *(This field read `n=1` while the Index and `Members:` read 2, for the minutes between adding the second member and the gate catching it — the same field-lags-the-count drift `IC-2` recorded an hour earlier, in the same ledger, by the same hand. A count and every judgement quoting it move independently, and there are three quoting sites per class, not two.)*
**Mechanism status:** `none yet`. This field read `designed` until 2026-08-31, on the reading that the member *"names the check"*. The member names it and **records it as insufficient**: it ran the grep — 466 hits across 99 files under `src/` — and concluded *"not a worklist … the defect needs the haystack to embed environment text, which the regex cannot see. Recorded so the next person knows the bare grep does not narrow it and does not repeat the measurement."* Whether a haystack interpolates a `Path` is a dataflow question, so no text search can decide it; the starting population is not the finding set. **The mis-stated field cost exactly what the member predicted**: a reader took `designed` at face value on 2026-08-31 and re-ran the measurement (120 hits at the narrower `assert!(!x.contains(` form) before reaching the member that says not to. A real mechanism has to start from the emitting side — which formatters interpolate a `Path` — and work outward to their assertions.
**Valid:** dated 2026-08-31

**Two tags were withdrawn on 2026-08-31, and the mistake is instructive.** The archive backfill put `ollama_large_batch_exceeding_batch_size` and `cross-process-write-lock-test-passes-when-it-does-not-run` in here, taking n to 3 and appearing to meet the threshold. Neither instantiates the claim: the first was vacuous the day it was written and the second is vacuous when skipped, and **neither turns on environment-controlled text**, which is the entire content of this class. They were matched from their titles — both read as *"a test that passes when it shouldn't"*, which is true of this class and true of a wider one. That is precisely how a class inflates past its threshold on members that do not instantiate it, which is the failure this ledger's own *"a wrong class corrupts the counts that promotion reads"* names. Their real family, *an assertion that cannot fail*, is recorded as a candidate class in the Index; their own `vacuous-assertion` and `green-proves-nothing` tags already said so.

The single member states its own general form better than a summary would: *"This is not 'a flaky test'; it is an assertion whose input contains environment-controlled text. Any `!haystack.contains(needle)` where the haystack embeds a path, a hostname, a timestamp or a temp name has the same defect, and it always fails open."*

The direction matters and is the reason this is not a duplicate of `CLAUDE.md` § *Testing Discipline*'s monotone rule. That rule says an assertion cannot detect a change it is monotone under, and prescribes mutating the other way. This class is narrower and concerns the **haystack** rather than the assertion's direction: the positive form (`assert contains`) is safe here, because a coincidental match makes it pass *when it should already pass*. Only the negative form can be satisfied by an accident that the property being tested does not license. The two are complements, and the monotone rule is the more general of the two.

Kept as a class of one for the same reason as `IC-8`: the bug corpus is where the second instance will arrive, and a defined slug is what lets it find the first.

**Falsified by** an emitting-side sweep finding no other negative `contains` over an interpolated path, which would make this a one-off rather than a class.

## IC-10 — authorship on a shared checkout is unrecoverable after the fact, so every party infers it from proximity

**Slug:** `cluster/authorship-unrecoverable-after-the-fact`
**Claim:** On a shared checkout there is no attribution channel, so authorship cannot be recovered after the fact. Every party therefore infers it from proximity — who else was active, which file appeared when — and proximity is not evidence.
**Members:** `filter={"tags": {"contains": "cluster/authorship-unrecoverable-after-the-fact"}}` — n=3, 2026-09-01, by query. The seed narrative below is not a bug file and is not counted. Second member `docs/issues/2026-09-01-un-wired-function-reds-the-shared-build-with-no-author.md` is the **read-side**: the write-side asks *who wrote this*, the read-side asks *is this mine* — and on a shared checkout neither is answerable for uncommitted state. Subsystem spread is now 2 (companion plugin banner; shared build + git tooling), so a third instance meets the promotion threshold. **That third arrived the same day, and it is a third subsystem** — `docs/issues/archive/2026-09-01-foreign-index-guard-passed-a-peers-staged-deletion.md`, retagged here from `IC-14` by the independent blind second read on the file's own measured root cause: *"the cause is attribution, not enumeration"*, and the guard *"behaved correctly throughout … a correct consumer of corrupted input"* — which is `IC-14`'s claim falsified rather than merely outweighed. Its stage log assigned each `(blob, path)` pair to whichever session's hook **observed** it first, and `git status` fires that hook, so a staged batch was claimed by whoever polled next: proximity read as authorship, exactly. Spread is therefore 3 (companion plugin banner; shared build + git tooling; the pre-commit stage log) and **both promotion bars clear as of 2026-09-01**.
**Blind party:** every party equally, which is what makes it different from an ordinary mistake. The information does not exist to be careless with: `git` collapses all sessions into one author string, and an untracked file carries no origin at all.
**Promotes to:** **clears both bars as of 2026-09-01 — n=3, spread 3** (the buddy compact banner, the shared-build red with no author, the foreign-index pre-commit guard). The target is `H` (a provenance channel is a mechanism, not a discipline), not `OB`; nothing is written yet, so *cleared* is not *promoted*. **The third member arrived by retag, not by filing** — the blind second read moved `2026-09-01-foreign-index-guard-passed-a-peers-staged-deletion.md` here from `IC-14`, which is the retag this entry's own promotion condition (*"instance 3 meets it"*) was waiting on. *(This field read `not yet` — `n=2` for hours after that retag tripped it, standing beside an Index row that already read `n=3`, while `IC-14`'s field still claimed the same file as its 8th member: one file, two fields, both reasoning from before the move. The Index cells are gated by `every_index_count_matches_the_corpus`; these fields are not, which is the whole distance between the two.)*
**Mechanism status:** none yet — **not checked against the code as of 2026-09-02**, so read this as an open question rather than an established absence. The candidate named under `**Promotes to:**` is a provenance channel (`H`).
**Valid:** dated 2026-08-31

**Split from `IC-1` deliberately, on the remedy test.** `IC-1` claims a write reaches further than the set of peers you can see, and its remedy is an ownership protocol over the shared resource. This class claims something narrower and later: once the write has happened, *who did it* is not recoverable. Its remedy is a provenance channel. Same substrate, different missing thing — which is the same test that keeps `IC-1` and `IC-2` apart despite both reducing to "a component reasoning about a scope it cannot observe". `buddy-compact-banner-names-a-peers-session-as-your-own` was filed under `IC-1` and is moved here: its defect is that `from=<sid>` names another live session as your own predecessor, which is misattribution, not blast radius.

**Seed evidence — an exchange between two sessions produced three misattributions, all while reasoning about this class.** Sessions `codescout-kat` and `codescout-23`, 2026-08-31, both actively working the `IC` ledger:

1. `codescout-kat` told `codescout-23` "your nested-hook-state bug reasons that session 3a6d634e… wrote `.buddy/`". The reasoning is in that file, but the file is not `codescout-23`'s.
2. `codescout-kat` warned `codescout-23` that "your untracked librarian-runtime bug file" would red the cluster gate. Also not theirs.
3. `codescout-23`, correcting the above, argued the file was `codescout-kat`'s because *"your own `2ed2e716` calls it 'my nested-hook-state bug'"*. `codescout-kat` authored exactly two commits that session, `351836a8` and `522675a6`; `2ed2e716` is neither.

The file belongs to a third session neither had enumerated. `git log` shows why the dispute was unresolvable from inside it: `2ed2e716`, `e14b230e`, `351836a8` and `522675a6` all read the same author and email, because git has no session dimension — the field is a constant and carries zero information. The one channel that did work was accidental: `.buddy/by-ppid/<pid>/session_id` on disk, which exists for unrelated reasons and is untracked.

Note the pattern is `OB-1`'s — *"the author, specifically"*. All three attributions were made by parties who had just read the evidence, in messages *about* attribution failure. Knowing the class prevented none of them, which is the standing argument against answering this kind of defect with care rather than mechanism.

**Falsified by** an attribution dispute on a shared checkout that a party could settle from committed state alone.

**The instrument exists, and it splits by state — established 2026-09-01, three misattributions of ONE file in one evening.** A fourth, fifth and sixth instance of the seed pattern above, and the sharpest yet because the parties were mid-argument *about this class* and citing F-80 by name:

| state | instrument |
|---|---|
| committed | **`Session-Id` commit trailer.** Positive, exact, one `git log`, reaches sessions no socket enumerates including exited ones. |
| **uncommitted** | **none exists.** Directory adjacency, `ListAgents`, `git status`, dirty-file lists and conversational proximity are all elimination in disguise. Ask the session; until it answers the supportable claim is **"not mine"**, never "yours". |

The sequence: a session running the gate hit a red build from a peer's un-wired function, established "not mine" correctly by `git status` + `git grep HEAD`, then asserted an owner from conversational proximity — wrong. The correcting peer named a different session from *directory* adjacency — also wrong, and filed against themselves as `bug-fix-session-log:F-89` for committing the corrected error inside the correction. The true owner was settled only once the work was **committed** and its trailer existed, then volunteered by that session unprompted.

**What this adds to the class: the trailer is a real positive instrument, and reaching for it on uncommitted state is an instrument swap that reads as rigour.** The sentence "the trailer is the positive instrument" was true and did not apply to the object in front of either party. The remedy the evening actually supports is the terminal state, not a better inference — stop at "not mine".

**A TOOL commits this class too, which is the strongest evidence the ledger has for it.** The three misattributions above were made by agents, so "be more careful" remains an available (wrong) reading. `pre-commit` removes it. Its post-hook check is an unconditional whole-tree diff — `files_modified = diff_before != diff_after`, `pre_commit/commands/run.py:203-206`, no per-hook opt-out in 4.6.2 — and it reports any difference as *"files were modified by this hook"*. The tool has no way to ask who wrote them, so it attributes by **proximity in time**, exactly as the agents attributed by proximity in the working tree.

Measured 2026-09-01: it refused a push on a **green** `cargo test --workspace` run, naming `docs/trackers/claim-decay.md` — a file nothing under `src/` writes (`claim-decay` appears there three times, all citations in comments) and which a peer session was editing at that moment. The false-failure window is the hook's runtime: sub-second for the commit-stage checks, 30-80s for the workspace test. The push stage was withdrawn for this reason (`5fbc65fb`), and the reasoning is inline in `.pre-commit-config.yaml` so nobody re-adds one without meeting it.

That this arrived *inside infrastructure built by the session that opened the class, four hours after opening it*, is the `OB-1` signature again — and it is why the remedy field reads `H` (a provenance channel) rather than any amount of care.

**Adjacent, and deliberately NOT counted as a member: the verification reflex is trained on the
technical domain and does not fire on the social one.** Named by `codescout-e8` about itself on
2026-09-01, after three wrong assertions to a peer in one evening — that peer reachability was
partitioned (it was comparing a send-by-address against a send-by-name, two different
operations); that five newly-opened classes had "landed with instances" (all five read `n=0
tagged`, and the commit subject says so); and a near-miss on routing a pattern to `IC-8` that
was caught only because it happened to read the admission test first. Its own diagnosis: *"I
have been rigorous about every claim concerning the corpus and casual about every claim
concerning the collaboration. Code claims get a probe; claims about who did what and why get an
inference."*

**It generalises to at least two sessions.** The receiving session made the same move earlier
the same evening: told that `IC-9`'s count had gone from 1 to 3, it asserted a mechanism — that
a recursive grep over `docs/issues/` had counted slug strings inside untracked session logs —
and published it before checking. The real cause was two mis-tagged archive files, a decision by
another party. Both errors are the same shape: **a mechanism inferred for another party's action
and asserted at the confidence reserved for measured facts**, while every claim either session
made about the corpus that evening was probed first.

**Kept out of the member count on purpose, and the reason is the same test that governs this
ledger.** `IC-10`'s claim is that the information *does not exist* to be careless with — git
collapses sessions into one author string. This is the opposite: in all four cases the answer
**was** available and cheap (a Members line, a tag history, two tool signatures), and nobody
looked. It also fails `OB`'s admission test outright, since a more careful version of the same
party would have caught it, which is precisely what disqualifies an `OB` row. Folding it in
would inflate `IC-10` on a family resemblance — the error `IC-9` was corrected for four hours
earlier. Recorded here because this is where a reader of `IC-10` will look for it, and because
the standing remedy is cheap and already written down: *verify before contradicting a peer* —
which holds equally for **agreeing** with one, the direction three of these four ran.
## IC-11 — documentation denies a capability the code has since gained, because the prose was true when written

**Slug:** `cluster/doc-contradicted-by-code`
**Claim:** A document states a behaviour the code contradicts. The statement was *true when written*; the code later gained or lost the capability. Nothing checks prose against code systematically, and the corrective pass that *does* happen is a hand-enumerated sweep whose completeness is unfalsifiable — it reports the surfaces it changed, never the ones it missed. Unlike a wrong statement, this defect has no authoring error to find.
**Members:** `filter={"tags": {"contains": "cluster/doc-contradicted-by-code"}}` — **n=13, 2026-09-02** — four added by the 2026-09-02 tool-surface review, all on the `tools/list` surface itself: `workspace-schema-requires-an-action-the-code-does-not`, `artifact-patch-schema-describes-a-failure-that-no-longer-happens`, `index-description-omits-the-verify-action`, `artifact-action-labels-omit-delete-move-and-update-entry` (the last is a partial fit — two of its three omitted actions predate the label, so it is incomplete-from-birth rather than decayed; kept here on the mechanism, prose-about-code with no check reading one against the other). **Spread not re-adjudicated for these four**; they add a sixth surface, the MCP tool schema. Previously **`n=7`, 2026-09-02**, re-derived with the anchored `git grep -clE` form over `docs/issues/*.md` + `docs/issues/archive/*.md`. **That form read 6 and a plain `grep` read 7 for as long as the new member was untracked** — the blindness this field's own parenthetical predicted (*"blind until the file is tracked"*), met by the next person to add a member; and the obvious remedy is wrong. *Superseded 2026-09-02:* this field closed **"`git add` first, then count"**, which is correct on a solo tree and, on a shared index, instructs a session to stage a file it did not write. It did exactly that — `docs/issues/2026-09-02-one-ledger-file-serializes-every-class-edit.md` is the record, and its third arrival route is a **reader**, holding nothing to commit, mutating state six sessions were using solely to obtain a number. Instead: derive over **tracked** files with the anchored form above and issue no `git add`; add only the member **this same operation commits**, which you know without consulting the index; and never count a `git ls-files -o --exclude-standard` hit, because that member is a peer's and counting it predicts a commit nobody has made. The union of tracked-plus-untracked was proposed as the fix and falsified the same day: it answered 21 for one class, and 21 was right **only because** that file's owner chose to commit — had they abandoned it the gate would have reddened, with nothing differing on the deriving session's side. A prediction about other sessions' future commits reads identically to a measurement. The field asks how many are **tracked**, and `tests/issue_clusters.rs:511-527` — `actual_counts` over `tracked_all_bug_files()` — is the definition of that question rather than a proxy for it. (Defect and corrected rule: `codescout-05`. Union falsified: `codescout-20`. Record carried: `codescout-ca`.)

**The twelfth is a DIAGNOSTIC STRING rather than a document**, which widens the surface set to a seventh — hook output. `foreign-index-refusal-names-a-cause-no-route-produces`: `scripts/pre-commit-foreign-index.sh:209` explains an unrecorded owner as *"staged before this guard was installed"*. That was plausibly true at `e3c75306` (2026-09-01 02:01:38), when it was the only route to `-`; `fa9b3aff` (03:58:14) and `92dfa4e4` (14:00:58) added two further routes to the same state, and neither had any reason to read a sentence in the other script. The decay is measured by `git log -S` on the three strings rather than asserted. Kept here on the same mechanism as the fourth member above — prose-about-code with no check reading one against the other — and it is the first member where the prose is *emitted at runtime to the party it misleads*, which is why the reader is not merely uninformed but actively told the tree is not theirs.

**The seventh is the first member whose prose lives INSIDE the code file it describes**, and the first whose claim is **forward-looking**: `2026-09-02-a-doc-comment-announcing-unbuilt-work-outlives-the-work` — `scan_dated_stale`'s doc comment said Task 7 was *"not-yet-shipped"*, Task 7 shipped as `scan_cited_but_undeclared` two weeks earlier, and a reader wrote a worklist item proposing to build it. Two things it adds that the other six do not.

**(a) The decay trigger inverts.** The six existing members are positive claims that rot when the code *changes*. A forward reference is a claim of **absence**, so it rots when the project **succeeds** — and the party holding the falsifying fact is the implementer, whose diff adds the new symbol and never touches the sentence three functions above it. Self-review of that diff structurally cannot surface it. Measured population: **3 doc-comment forward references in the Rust corpus, 1 stale.** The two survivors split usefully — `tests/librarian/goal_eval.rs:64` is wired to a failing rubric and an `#[ignore]` naming its precondition, so shipping the thing changes a test's state; `src/agent/mod.rs:672` is pure prose and is one feature away from rotting identically. **The forward reference that survives success is the one tied to a check.**

**(b) Proximity is refuted as a remedy.** Same file, three functions away — the tightest coupling short of the same line — and it still rotted. Any "keep the docs next to the code" answer to this class is answered by this member.

**Previous reading, kept because the count-vs-prose lag is this field's own recurring defect:** n=6, 2026-09-01, by query after a probed archive pass. **Fourteen candidates were probed and three passed**; the ten that did not are deliberately untagged, not pending. See *The probe* below. **Four are `fixed` and archived; a fifth opened the same day** — `2026-09-01-librarian-mcp-page-describes-a-separate-server-that-was-collapsed`, a manual page still framing librarian as a separate sister MCP server after the tool collapse. *(This sentence read "all four … the class now has no live instance" for two hours, until its own author opened the fifth. Premise moved, conclusion did not — the fourth instance of that in this file today, and the one thing that would have caught it is `every_index_count_matches_the_corpus`, which is blind until the file is tracked.)* **And a sixth, opened later the same day** — `2026-09-01-claude-md-denies-a-pid-to-session-join-the-registry-carries`: `CLAUDE.md` § *Observer Blindness* denies a pid→session join that live registry entries carry. So the class has **three** live instances rather than one, and the Index row named the sixth before this field did — the fifth time in this file that a count moved and the prose beside it did not.
**Blind party:** the *reader*, routed to the document by its own scope claim and given no signal to cross-check. The author of the prose is not blind — they wrote something true. The author of the *code* change is differently blind: gaining a capability gives you no reason to search prose for sentences your feature just falsified.
**Promotes to:** `not yet` — n=13 (was `n=7` until the 2026-09-02 review added four schema-surface members), and the count bar has been cleared since the probe (this line read `n=6` until 2026-09-02, `n=4` until 2026-09-01, and `n=1 taggable` before that — backticked, so the gate reads them as quotations and leaves them alone). **The 2026-09-02 update missed this line and `every_bare_n_in_a_class_field_matches_the_corpus` caught it**, which is the sixth occurrence of *count moved, prose beside it did not* on this entry and the first one a gate found rather than a reader: three surfaces carry the number — `**Members:**`, this field, and the Index row — and updating two of three is the default outcome, not a lapse. **It recurred the same day and the same gate caught it again**: the twelfth member's author updated the Index row and `**Members:**`, missed this field, and was refused — the seventh occurrence of the drift, and the second a gate found rather than a reader. The aggravating detail is worth the line: that author had read this very sentence, in this session, minutes before making the edit. A warning sitting one line above the number does not survive an edit made two fields away. The gate does. What holds promotion is not the count but the three-way remedy split adjudicated below — **and the spread, which the Index row records as adjudicated at 4 doc surfaces / 4 subsystems and explicitly *not* re-adjudicated for the sixth or the seventh** (the seventh adds a fifth surface, a Rust doc comment). The likely target is `DC` (`docs/trackers/claim-decay.md`): a true-when-written claim that silently decayed is that ledger's subject, and this class is the bug-corpus entry point to it rather than a competitor — the same relationship `IC-8` declares.
**Mechanism status:** none yet, and the nearest existing mechanism does not cover it. `librarian(action="audit_doc_refs")` lints *references* — paths, symbols, line numbers, link targets — so a document may cite every path correctly and still assert the opposite of what the code at those paths does. The remedy would have to check claims, not refs. **That is the right conclusion for the wrong reason on one of the four members** — see *The spread* below, where a member citing four names that do not exist is missed anyway, and not because it asserts anything.
**Valid:** dated 2026-09-01

Seed instance: `2026-08-31-librarian-runtime-guide-denies-the-augmentation-sidecar`. The served `librarian-runtime` guide states augmentation has *"**No** — there is no on-disk representation"* and that sharing it is *"local-only by design"*. Both sentences were accurate when written. The sidecar shipped as `e799f29d` on 2026-08-30, and a deliberate sweep the **same day** — `e1b91221`, *"state that augmentation shape now travels, in the three places that said otherwise"* — corrected `CLAUDE.md`, `docs/conventions/cross-machine-catalog-resume.md` and `tracker-conventions.md`. Not this guide. So the drift is **one day old**, and the mechanism is an enumeration produced from memory, not neglect: "three places" reads as a finding and is a list. The guide mentioned `sidecar`/`expects_augmentation` zero times against `tracker-conventions`' thirteen.

**Fixed 2026-09-01** — SHA `0523b823`, patch-id `9ec0e7c8911be27700318ba60b945454275391e7`. **Four**
sentences corrected, not the two the bug filed: reading the rest of the section found *"An augment
produces no git diff … `git status` stays clean"* false since write-through (`sidecar_write_through`,
`augment.rs:248`), and the `reindex` bullet true but silently incomplete. The bug's own enumeration
had stopped at the examples that prompted it — the same mechanism it was filed against, one level in.

**And the zero-times count above decayed the instant the fix landed** — it now reads 5. That is this
class holding about its own seed paragraph, which is why the sentence is now past tense rather than
corrected in place: re-pointing it to 5 would schedule the identical decay for the next reader. The
standing guard is `prompts::redesign_invariants::no_guide_denies_the_augmentation_sidecar`, sibling
to the test written for the *2026-08-16* miss of this same section — two three-place sweeps, one
file. It asserts both directions, because an absence half alone is monotone under removal, and each
half was mutation-verified separately.

**The cost is not that a reader is misinformed — it is that the reader stops.** A sentence saying a capability does not exist terminates the search that would have found it. Measured downstream the same day: a consumer repo held two augmentations in a machine-local catalog with no sidecar and no declaration, one clone away from silent loss, because the guide consulted for exactly that question said there was nothing to export. `doctor`'s `augmentation_declared_but_absent` could not report it either — that check fires only on a *declared* sidecar that is missing, so undeclared-and-unexported reads identically to nothing-to-declare.

**The same guide, the same section, fifteen days earlier — and it is already tagged.**
`docs/issues/archive/2026-08-16-librarian-runtime-guide-claims-move-preserves-id.md` is
`status: fixed`, and reports that this *same* § *Where catalog state lives* section claimed
`artifact(action="move")` "preserves `id`" when a move necessarily re-keys the row. It
carries the tag `doc-vs-code` — the shape had an informal label before it had a class, which
is the ordinary way a class announces itself. It stays untagged for `cluster/` purposes under
the archive policy, so it does not raise the count. Two of the three open files carrying
`doc-vs-code` are correctly filed under `IC-2`; the tag is a secondary descriptor there, not
the primary defect.

**Kept apart from `IC-3` and `IC-8` on their own falsifiers, not on judgement.** `IC-3` is a surface declaring a capability production never reaches; this is its mirror — production reaches a capability the surface denies — and `IC-3`'s falsifier explicitly ejects the mirror case (*"the wiring existed and the declaration was merely wrong, which is an ordinary bug rather than this class"*). `IC-8` is an assertion written at the moment of intent and read forever after as outcome; this prose was not intent, it was correct observation, which is why no plausibility check catches either one.

**Falsified by** an instance where the documentation was wrong on the day it was written. That is an ordinary authoring error with an author to find, and it does not belong here.

**The probe, run 2026-09-01 — and it is the reason this class was backfilled last.** This entry's
admission question is *"was the prose true when written"*, which is a fact about **history**, not
about the text. No amount of reading either surface answers it: a document that contradicts the
code reads identically whether the code moved under it or the author was simply wrong. So the
archive pass that tagged `IC-13`–`IC-16` from claims alone could not tag this one, and fourteen
candidates went to `git log -S` instead.

**Three passed** and are now tagged. `recoverableerror-display-doc-contradicts-code` is the
cleanest: the `Display` doc comment said it omitted `hint`/`guidance`, and `dc8f0f1f`
(2026-05-09) is titled *"Display surfaces RecoverableError guidance text"* — the code **gained**
the behaviour, so the comment was true until that commit and false after it, with no author to
find. `tools-semantic-search-manual-page-describes-legacy-interface` describes the pre-Phase-7
interface, accurate until Phase 7 replaced it. `test-env-isolation-doc-prescribes-rejected-remedy`
prescribed option B until `a656f8ce` marked that remedy non-viable.

**One was refuted, and it is the one this entry singles out.**
`docs/issues/archive/2026-08-16-librarian-runtime-guide-claims-move-preserves-id.md` — same guide,
same § *Where catalog state lives* — looked like the strongest member available. Its own
`## Hypotheses tried` states the question and defers it: *"the guide is describing an older
behaviour that was correct when written. **Test** — `git log` on `src/librarian/tools/mv.rs` for a
commit removing id-stability. **Verdict** — deferred; irrelevant to the fix either way."* Running
that deferred test settles it against the class: `mv.rs`'s full history contains **no commit that
removed id-stability**. The id has been `sha256(abs_path)` since the file existed, and the
2026-08-16 commits nearest the question — *"move now grafts history onto the new id instead of
stranding it"*, *"repair the frontmatter id the move just invalidated"* — **cope with** the
re-keying rather than introduce it. The guide was wrong on the day it was written. That is this
entry's own falsifier (*"an ordinary authoring error with an author to find"*), so it stays out,
and the earlier note that it "stays untagged under the archive policy" is superseded by a reason
that survives the policy changing.

**Ten remain untagged because the probe did not establish them, which is not the same as pending.**
The recurring shape among them: the doc and the contradicting code appear to have coexisted from
the start — `path_security.rs`'s module doc promises `RecoverableError` while `bail!` is present
in the file's own creation commits; `scope`'s documented `project` default against a compiled
`Repo` that only two commits ever touched, the second being the fix. Each is *probably* an
authoring error, and "probably" is exactly the standard this entry exists to refuse. They are
listed as probed-and-not-established so the next reader does not re-derive the same fourteen.

**The spread, adjudicated 2026-09-01 — and the unit matters, as it did for `IC-14`.** Four members,
**four distinct documentation surfaces** (user manual, conventions doc, Rust doc comment, served
guide) and **four distinct subsystems** (retrieval, test infrastructure, error handling, librarian).
The two counts agree at 4, which is worth stating rather than picking one — agreement is evidence,
and a bare *"4"* hides which question it answers. `IC-14`'s did **not** agree (8 members, 6 guards),
so the agreement here is a fact about this population and not a property of the ledger.

**But the four split three ways on what the prose is a claim ABOUT, and the three have three
different remedies** — the same shape `IC-14` turned out to have, and the same reason one promoted
rule would be right about a third of it:

- **Behavioural claim** (2) — the prose asserts what the code does or does not do, and every path
  and symbol it cites is correct. `recoverableerror-display-doc-contradicts-code`, where the doc
  comment claims an omission the `fmt` body does not make, and the sidecar guide. **Unreachable by
  any reference check, by construction.**
- **Decision claim** (1) — `test-env-isolation-doc-prescribes-rejected-remedy` prescribes option B
  after `a656f8ce` recorded that remedy **NOT VIABLE**. The falsifying artifact is *another
  document*, so neither reading the code nor checking its refs reaches it. This is the member with
  a measured live cost: its own summary records the doc having made engineers reproduce a purged
  bug "at least twice in one session".
- **Named-entity claim** (1) — `tools-semantic-search-manual-page-describes-legacy-interface` names
  four things that do not exist (`score`, `language`, `detail_level`, `offset`). It is not an
  assert-the-opposite at all, and it is the mechanizable one.

**The mechanism finding corrects this entry's own stated reason.** `Mechanism status` above says
`audit_doc_refs` cannot cover the class because a document "may cite every path correctly and still
assert the opposite". True of the first two shapes; the **wrong reason** for the third, which cites
nothing correctly — four dead names — and is missed anyway. Two hypotheses were tried before the
right one, and both are recorded because each is the plausible answer:

1. *The names sit in a fenced JSON block and the scanner skips fences.* **Refuted.** `parser.rs:48`
   emits candidates while `in_code_block`, pinned by `parser_walks_fenced_code_blocks`
   (`parser.rs:1021`). A fenced ref is **severity-capped** to `code_block`, never dropped
   (`severity::cap_code_block`, applied at `resolver.rs:671`, pinned at `resolver.rs:866`). Same
   structure as the forced-`Med` on code comments: found, then downgraded below `--fail-on high`.
2. *So it is found and downgraded.* Also wrong. `RefKind` has exactly five variants — `FilePath`,
   `FileLine`, `FileSymbol`, `ModulePath`, `Link` (`src/librarian/tools/audit_doc_refs/mod.rs:11`)
   — and **all five are locations**. A JSON response field and a tool parameter are neither. The
   instrument does not downgrade them; it never sees them.

So the class has one buildable sub-remedy with a concrete shape — a candidate kind for tool params
and response fields, checkable against the live schema, which is the same set-difference the tool
registry guard uses — against "check claims, not refs" for the other two, which is not buildable
today. **`Mechanism status` stays `none yet` because the majority shape has no instrument, but it
is `none yet` for two reasons now, and only one of them is hard.**

*(Both hypotheses above were mine, stated confidently, and each was refuted by one grep. The
fence one is the instructive failure: fenced content being illustrative rather than real is
`IC-6`'s subject, so the wrong answer was the one the neighbouring class made most available.)*
## IC-12 — transient shared state lies to every reader, and the standard diagnostic confirms the lie

**Slug:** `cluster/transient-shared-state-lies-to-readers`
**Claim:** One session's tooling mutates shared state for the duration of an operation. Every other session's read is wrong for that window, and the standard diagnostic reports the lie as truth rather than as an outage — so the symptoms are indistinguishable from permanent loss.
**Members:** `filter={"tags": {"contains": "cluster/transient-shared-state-lies-to-readers"}}` — n=2, 2026-09-01, by query. The instance below — measured 2026-08-31 — finally has a file of its own: `docs/issues/2026-09-01-pre-commit-stash-removes-every-peers-unstaged-work.md`, carrying a deterministic isolated reproduction and the mechanism at `pre_commit/staged_files_only.py:108`. This line read *"`n=0` tagged … nothing to tag yet"* until then, which was accurate and is exactly why the class read as empty: the finding had been filed as a paragraph inside another bug file, so no query could reach it. **The second member is `docs/issues/2026-09-01-cluster-count-gate-lists-the-index-but-reads-the-worktree.md`**, and it is this class holding about a gate whose own module header argues *against* reading the working tree: the bound is enforced on the file LIST (`git ls-files`) and not on the file CONTENT (`fs::read_to_string`), so a peer's half-written bug file reds another session's build for the duration of the write. It is also the first member caught **by its own transience** — the red named a cluster the change under test never touched, a hand re-derivation over the identical population returned the ledger's own figure, and a re-run passed 18/18 with nothing altered in between. Its sibling `scripts/pre-commit-ledger-counts.py` reads the index and says so in its header, which makes this a divergence between two implementations of one rule rather than an open problem.
**Blind party:** the *reading* session, and note the inversion — every other class here blinds a writer. Here the writer is fine and the reader is deceived, by an operation it did not initiate and cannot see.
**Promotes to:** `not yet` — n=2, and the remedy so far is knowledge rather than mechanism. What changed on 2026-09-01 is legibility, not the count's meaning: the first instance became queryable rather than newly discovered, and the second was filed the same day. **Both members are shared-checkout reads and neither is a race in the usual sense** — the pre-commit stash removes a peer's unstaged work for the duration of someone else's hooks, and the cluster-count gate reads a peer's half-written file as corpus. Spread is therefore 2 across one subsystem (shared-checkout tooling), so this clears neither bar; a third instance **outside** that tooling is what would move it.
**Mechanism status:** none yet. Documented at the point of use (`scripts/pre-commit-unreviewed-content.sh` header, `0b763983`), which is a knowledge fix and by this ledger's own rule a worklist item rather than a rule.
**Valid:** dated 2026-09-01

Measured 2026-08-31, within a minute of git hooks being enabled on this shared checkout. The pre-commit framework stashes unstaged changes while hooks run, and that stash covers **every** session's in-flight work, not only the committing one's. For the sub-second duration of a peer's commit, a session observed its own edited file revert to HEAD content, `git status` report it clean, and a `grep` for text it had just written return nothing.

**The detail that makes it a class rather than a footnote: `git stash list` is EMPTY throughout.** pre-commit writes a patch under `~/.cache/pre-commit` instead of using `git stash`, so the obvious way to detect a stash reports that there is not one. The reader is not merely misinformed — the instrument they would reach for to check confirms the false reading. There is no opt-out; `pre-commit run --help` exposes no stash flag and the stash is unconditional when unstaged changes exist.

**The danger is not the window, it is reacting inside it.** Rewriting a section from memory races the restore and can genuinely lose or duplicate work while "recovering" from a problem that has already fixed itself. So the remedy is an oracle, not a fix: for a librarian artifact, `artifact_event(action="list")`'s `field_patch` byte counts, which no git operation touches; for anything else, `wc -c <path>` against `git show HEAD:<path> | wc -c`. Never `git status`.

**Kept apart from `IC-1` on the observer, not the substrate.** `IC-1` is a write reaching further than the set of peers you can see; here no write collides at all and the shared state is correct at both ends of the window. It generalises past `pre-commit` to anything that transiently mutates shared state — a formatter run, a build that moves files, a script that checks out.

**Falsified by** an instance where the standard diagnostic correctly reported the transient state as unavailable rather than as settled truth. That is an outage, which is a different and much safer thing.

## IC-13 — a capped result is presented as complete, so a partial answer reads as the whole one

**Slug:** `cluster/capped-result-presented-as-complete`
**Claim:** A result is truncated by a limit — a page size, a byte budget, a display cap — and returned **without a marker the caller can see**. The marker may be absent, or computed correctly and then lost before it arrives: dropped at a buffering or serialization boundary, buried in a nested key the envelope never names, or attached to a different object than the one served. The caller reads a partial answer as the whole answer, and a **zero** from a capped scan reads as "not present" rather than "not reached".

> **Clause widened 2026-09-01, on measurement.** It read *"without a marker saying so"* until then, and that wording was true of only **4 of the 16** members. Five more had a marker that was computed **correctly** and never reached the reader — the class's own `link-scan-truncation-is-accurate-and-unreachable` is the member that names the distinction: *"The information exists and is simply not where the decision is made."* Accuracy was never the property that mattered; **arrival** is. What the widening deliberately does **not** absorb is a marker the caller *can* see — whether sound (those files file a different defect) or **wrong** (`grep`'s `Showing N of N`), because a visible-but-false signal defeats a different remedy and its true total is often unknowable rather than unreported. Those remain outside, and the membership ruling below is what moves them.
**Members:** `filter={"tags": {"contains": "cluster/capped-result-presented-as-complete"}}` — **n=12, 2026-09-02, re-derived** by anchored file count. Three additions on 2026-09-02, all judged against the widened clause at filing and none re-read by a second party: `parse-rests-on-truncates-at-line-one` (the parser returns line 1 of a `**Rests on:**` declaration and `context.rs:427` renders that fragment as the whole field — 70% of declarations are hard-wrapped, so truncation is the common case and it is marked nowhere) and `artifacts-are-embedded-from-their-first-chunk-only` (`indexer.rs:69` keeps `chunk_markdown(…).next()`, so a 189-entry ledger is embedded from 0.18% of its body and semantic search returns a ranked hit with no indication of what was searched), and `symbols-renders-a-wrapped-signature-truncated-at-the-paren` (`symbols(name=…)` asks the language server, rust-analyzer answers with a name-only range, and `focus_single_symbol`'s `end < start` guard passes on equality — so it inlines a **one-line body slice**; for a wrapped signature that renders `pub fn discover_projects(`, arity 0 and no return type, with no truncation marker because nothing on that path knows it cut anything. Fits the clause exactly: the cap is not announced, and the partial value is well-formed enough to be used. **Invisible without a warm language server** — with none, codescout falls back to AST extraction, reports the true range, and inlines the whole body correctly, so any test that does not wait for the LSP observes the correct behaviour and passes). **The spread and claim-holds figures below were derived over the 9 and have NOT been re-derived over the 12.** Superseded figure, kept for its derivation: `n=9`, 2026-09-01, re-derived by file count, **after both rulings closed**. Was 16 until the seven non-members moved to `IC-19`/`IC-20`/`IC-21`/`IC-22`; the widened clause is true of all nine that remain. Every figure in the rest of this field describes the **pre-ruling population of 16** and is kept for its derivation, not as current state. The newest at that time was `docs/issues/archive/2026-09-01-heading-scoped-get-overflow-hint-points-at-metadata.md`, and it lands in the **open ruling** below rather than under the headline claim: nothing is presented as complete there (the envelope marks itself buffered and reports `buffered_bytes`), and what fails is the follow-up route the correctly-computed signal names. A third independent datapoint for the same clause. **No longer single-party** — an independent blind second read (see the Index) agreed on 14 of 16 and moved two out (**an earlier pass over a different population — both were already retagged before the `n=16` measured below, verified at `1e459f4a`, so they are *not* among the seven; the chain is 16 → 9 by seven departures, and the two are a separate, prior subtraction**): `doctor-outside-roots-sample-is-unranked-and-unreachable` to `IC-15`, its own summary reading *"The cap is announced, which is right"*, which is this claim's negation; and `capped-get-body-round-trips-into-truncating-write` to `IC-14`, whose filed defect is the byte-only shrink guard rather than the cap that fed it. **Open ruling, MEASURED 2026-09-01 rather than sampled.** All sixteen bodies were read against the claim's *"without a marker"* clause, one reader per file, every verdict carrying a direct quote. **The claim as written holds for 4.** **Five** are the *signal computed correctly and unreachable* shape — dropped at a buffering boundary, buried in a nested key the envelope never names, or pointing at the wrong object — so the clause is strictly false for those. **Seven are neither, and that is the larger finding:** four announce their cap reachably and file a different defect; **two involve no truncation at all**; and one carries a marker that is *present and wrong* (`grep`'s `Showing N of N`, whose true total after `hit_cap` is **unknowable** rather than unreported, so "add a marker" is not even an available remedy). Two of the seven **disclaim this class in their own text** — `grep-narrowing-hint` (*"the overflow signal itself is sound — this is not the silent-cap class of bug"*) and `append-entry-anchor` (*"loosely grouped with the day's silent-partial-result findings; it is not one of them"*). This line previously read *"at least four"*, taken from two readers' incidental samples during a different audit: that floor was carried as bounding the members the claim **fails** for, and it in fact bounds the ones it **holds** for — the true failure figure is **12 of 16**. Full working in `cluster-promotion-session-log:F-5`. **Outcome: all seven moved out 2026-09-01**, so of the sixteen measured, the four *claim-holds* and five *signal-unreachable* members are the nine this class now has.

**Both rulings are CLOSED as of 2026-09-01.** Ruling 1 widened the clause above, taking the claim from true-of-4 to true-of-9. Ruling 2 moved the seven non-members out, on a **two-reader** adjudication over the same seven files — the first classification pass in this work stream that was not single-party — returning **7 of 7 `NONE`**: not one belonged to any existing class. Both readers produced the *same remedy for all seven*, so on this ledger's discriminator the substance was agreed; they differed only on grouping, **three classes or four**, and the four-class partition was taken. The seven now sit in `IC-19` (3), `IC-20` (1), `IC-21` (2) and `IC-22` (1). Working, both readers' tables and both sides of the granularity argument: `cluster-promotion-session-log:F-6`.
**Blind party:** the caller, who has no way to distinguish a short list from a complete one. Also the *author of a downstream count*, since an aggregate computed over a capped scan is wrong in a direction nothing signals.
**Promotes to:** `not yet` — **spread RE-DERIVED 2026-09-01 over the post-ruling 9 — it clears on both units, and the interesting number is still not the spread.** Two defensible counts, stated with their units because they answer different questions. Coarse unit (*the top-level surface owning the cap*): **5** — librarian (4), `read_file`/`file_summary` (2), `run_command` (1), the shared output buffer / `truncate_compact` (1), and one cross-cutting audit (1). Fine unit (*the module carrying the cap*): **7**, splitting librarian into `artifact`/`get` (2), `preview` (1) and `link_scan` (1). **The superseded figures were 6 and 11 over the pre-ruling 16**, kept because the delta is the check: the seven departures removed `grep` entirely (both members, to `IC-19` and `IC-20`) and took three further fine surfaces with them — `audit_doc_refs` and `append_entry` to `IC-19`, `audit_log` to `IC-21` — which is exactly 6−1 and 11−4. A file-by-file re-classification and an arithmetic subtraction from the old figure agree, and neither alone would be quotable. **One error in the old figure, found only because the re-derivation was done rather than the subtraction:** its fine list named `doctor` as a librarian sub-surface, and no member of the 16 was ever a `doctor` bug — the member is `2026-09-01-audit-growth-concentrates-…`, whose surface is `audit_log` (`src/librarian/tools/audit_log.rs`); the word *doctor* appears in that file's prose, which is how it got there. **The total 11 was right and one of its six names was wrong**, so this corrects a label rather than a count — and it is this ledger's own *never classify on description* rule failing on the ledger itself. **One caveat kept rather than smoothed, running the same direction on both units:** the cross-cutting audit counts as **one** member while alone naming **15 sites across 12 further surfaces** (librarian `find`/`gather`/`timeline`/`context`/`refresh_stale`/`legibility_scan`/`schema_validate`, `memory(recall)`, `semantic_search`, `edit_code` rename, `file_summary`, `output`). Counting those would double-count `file_summary` and re-add librarian surfaces whose own bug files have since left, so it is held at 1 — which makes **both figures floors**, and means that one member would clear the spread bar with no sibling at all. Either way the bar is met; the classification is **no longer title-and-tag deep** — all sixteen bodies were read on 2026-09-01, which is what produced the *Open ruling* measurement above. It remains **one reader per file** with no cross-check, so 4/16 is a measurement and not yet a corroborated one; treat it as this entry's own single-party caveat, of the same kind the Index records for the archive pass.

> **The finding that matters is concentration, not spread: 4 of the 9 carry the `progressive-disclosure` tag** — 10 when this was written, **corrected to 9 of 16** on 2026-09-01, re-derived by a peer session (`codescout-b1`) and independently confirmed here before the cell was touched. This is not a defect scattered across unrelated components — it is very largely **one architectural layer**, codescout's own output-budget machinery, emitting the same defect at many exits. That makes the class *more* tractable than its spread suggests, and it inverts the usual promotion reasoning: a wide spread normally argues for a rule because no single fix reaches the members, whereas here a single invariant on the capping layer would reach most of them.
>
> **That last clause did not survive the ruling, and the paragraph below called it in advance.** At **4 of 9** it is no longer *most*, so the concentration argument is **withdrawn as stated**: the layer hosts a plurality of this class, not a majority, and a single invariant on it is a partial remedy rather than a near-complete one. What the concentration turned out to mark was **`IC-19`/`IC-21`/`IC-22` territory** — the output-budget layer emits several distinct defects, and only some of them are this class. That is a better finding than the one withdrawn, and it is why the composition note was worth writing before the ruling closed rather than after.
>
> **The composition is what weakens that argument — not the digit.** Joining those 9 against the *Open ruling*'s per-file measurement gives **A=1, B=3, C=5**: a *majority* of the concentration evidence sits in files that fail this class's own claim and that the membership ruling moves out. Among the 7 members **without** the tag the split is A=3 / B=2 / C=2 — so the tag correlates with **non-membership**, which is the opposite of what a concentration argument needs. If the ruling lands as measured, IC-13 keeps 9 members of which **4** carry the tag, and *"a single invariant on the capping layer would reach most of them"* is false as stated. Re-derive the ratio **and** the sentence when the ruling closes; this is the ledger's own rule about counts and the judgements quoting them, firing on a judgement I wrote yesterday.
>
> **Two ways this number comes out wrong, both returning a plausible figure rather than an error** (measured 2026-09-01, one of them by the peer and one by me): a recursive `grep -rl 'cluster/…' docs/issues/` returns **17** members, because `docs/issues/.buddy/**/cs_tool_log.jsonl` holds the slug as logged tool-call text — the corpus reading back a measurement of itself; and a block-sequence-only predicate `^- progressive-disclosure$` returns **8**, because some bug files write `tags: […]` inline. **9** is with both YAML styles, frontmatter only, tracked `.md` only — which is the form `tests/issue_clusters.rs` uses and is immune to both.

**Mechanism status:** none yet, and the shape is now specific rather than gestural. Two members already ship the fix and are the pattern to copy — `link_scan`'s per-array `counts.truncated` and `run_command`'s `unfiltered_truncated`. **No cross-cutting guard exists**: a sweep for one found 16 hits across 14 files, all per-site assertions, none a sweep.

- **The invariant to gate:** a response that was capped must carry a marker saying so, and the marker must be reachable from the *shape the caller reads* — `2026-08-30-link-scan-truncation-is-accurate-and-unreachable` is the member proving accuracy is not sufficient.
- **Why it is harder than `IC-15`'s probe, stated so nobody re-derives it as easy:** that probe needed only an ill-typed value, which any schema supplies. This one needs each surface driven *past its own cap*, and the caps differ per tool (a byte budget, a page size, a heading count, a display limit). The per-tool half is the expensive half, where `IC-15`'s was cheap.
- **Where to start, given the concentration above:** the shared layer rather than the sixteen exits — `truncate_compact` and the buffer envelope, whose own member (`2026-08-15-truncate-compact-tail-cut-destroys-overflow-signal`) is the one that destroys the signal for every caller downstream of it.
**Valid:** dated 2026-09-01

This class is opened deliberately **before** its membership exists, which is a departure worth stating. The ledger's rule is that a member list rots and a query does not; the cost of that rule is that a class identified but not yet tagged reads as n=0. Recording the provenance in `**Members:**` is the compromise — a reader sees both the claim and the fact that nothing has been assigned to it yet, and cannot mistake the zero for evidence of rarity.

The archived instances were unread when this entry was written. All sixteen have since been read (2026-09-01) and the result is the *Open ruling* above — so the paragraph below stands as a record of what this entry rested on **before** the measurement, not as its current evidence. Note that one of its three examples, `grep`'s self-refuting *"Showing N of N"*, is precisely the member the measurement puts **outside** the headline claim. What is independently visible is that this repo has treated the shape as real for some time: `truncate_compact` cutting from the tail and destroying the overflow signal, `grep` printing a self-refuting *"Showing N of N"* when collection hit the cap, and `link_scan`'s dangling count being prefix-gated so a whole namespace could read as healthy — all closed, all the same claim.

**Falsified by** the backfill's three instances turning out to share a subsystem rather than a mechanism, which would make this a broken component rather than a class.

## IC-14 — a guard's coverage is narrower than its name, so the name is what everyone reasons with

**Slug:** `cluster/guard-narrower-than-its-name`
**Claim:** A guard's name states the property; its implementation covers a subset of it. Everything the name promises is believed protected, the uncovered remainder is protected by nothing, and the guard's own green result is what conceals the gap.
**Members:** `filter={"tags": {"contains": "cluster/guard-narrower-than-its-name"}}` — **n=12, 2026-09-02, re-derived** by anchored file count. The 12th is `docs/issues/archive/2026-09-02-is-write-omits-five-mutating-actions-so-the-write-guard-never-fires.md`: `acquire_write_guard_if_writing` is gated on `LibrarianAdapter::is_write`, which enumerates mutating actions by name and misses five — `artifact.append_entry`, `artifact.update_entry`, `artifact.graft`, `librarian.doctor(fix=…, confirm=true)` and `librarian.merge_worktree` — so calls everyone reasons about as "writes" take neither the mutex nor the fd lock. Second drift of this same match arm; the first is archived as `2026-06-01-librarian-adapter-stale-is-write`. **The spread below was adjudicated over the 11 and has NOT been re-adjudicated over the 12.** Superseded figure, kept for its derivation: `n=11`, 2026-09-01, re-derived by file count rather than carried. One of the two additions since the 9 reading is `docs/issues/2026-09-01-source-gate-refuses-the-whole-compound-command.md`, an **axis omission** instance: the source-file shell gate's predicate is per-command while its evaluation scope is per-string, so one offending clause refuses every unrelated clause beside it and the message names the rule but not the position. **No longer single-party** — an independent blind second read (see the Index) agreed on 7 of 8. It moved `foreign-index-guard-passed-a-peers-staged-deletion` **out** to `IC-10` (that guard was not narrower than its name; it consumed a corrupted log correctly, and the old tag described the file's disclaimer rather than its defect), and moved two **in**: `capped-get-body-round-trips-into-truncating-write`, whose byte-only shrink guard passed a 68% line loss at 29% byte loss, and `update-entry-drops-entry-silently-when-fields-is-also-present`, whose guard carries `&& args.get("fields").is_none()` — a subset of the condition its name states.
**Blind party:** everyone downstream of the name. The implementer knows the scope at the moment they write it; every later reader knows only the name, and the name is what they reason with. This is `OB-1`'s shape — the parameter the author's context supplied for free.
**Promotes to:** `not yet` — **it clears on both bars: n=12, and spread adjudicated 2026-09-01 at 4 subsystems / 6 distinct guards over the 11 then — not re-adjudicated over the 12.** *Membership claim withdrawn 2026-09-01:* this field read *"it clears, and n is 8 rather than 7"* and named `2026-09-01-foreign-index-guard-passed-a-peers-staged-deletion.md` as the 8th. That file is **`IC-10`'s third member** — the blind second read retagged it, on the reading that a session-attribution hook that cannot name an owner is a *provenance* failure rather than a narrow guard, and that retag is what tripped `IC-10`'s promotion condition. The observation the withdrawn sentence carried is still true of the file and now lives at `IC-10`: the hook covered the **cross-path** case and not the **intra-path** one that motivated it — one session's file gaining another's lines between the check and the `git add` — and shipping it as *"prevents `d617051b`"* would have been this class inside a guard against capture; its header says so instead. What is corrected here is only which class owns it, and the count, which had moved 8 → 11 independently.

Two units, both defensible. By **subsystem: 4** — the librarian-managed-file guard family (3), worktree awareness (3), the shell/`run_command` gate (1), git pre-commit hooks (1). By **distinct guard: 6** — `is_librarian_artifact` carries two members and the `EnterWorktree` post-hook carries two, so the member count overstates how many *guards* are implicated. Classification remains single-party: summaries read, not full bodies.

> **Three sub-shapes, and they do not share a remedy — which is why "name it better" is a third of the answer, not the answer.**
>
> 1. **Axis omission (3)** — the guard covers one axis of an operation space and the name covers all of it: writes-but-not-reads, cross-path-but-not-intra-path, one-of-three-write-paths. **The mechanizable third.**
> 2. **Predicate narrowness (3)** — the *in-scope?* test is narrower than the concept: quoted frontmatter ids, artifacts with no id at all, tilde/home paths. **This sub-shape is `IC-6` seen from the guard side** — `is_librarian_artifact` pattern-matching frontmatter text for a 16-hex `id:` is a parser with no disambiguator, and its two members are failures of that parser rather than of the guard's placement. Cited across rather than double-tagged.
> 3. **Context blindness (2)** — the guard does not know a category of entity exists: worktree shadows. No renaming reaches this one; the guard has to learn the category.

**Mechanism status:** none yet, and **only one of the three sub-shapes admits one.** Comparing a name's *semantics* to an implementation is not automatable; this entry's own tell — read the name as a claim, ask what input satisfies the name but not the implementation — is a review question, not a check. Saying so is the point: a class whose remedy is mostly discipline should not carry a `none yet` that reads as unfinished work.

- **The mechanizable third is axis omission**, and `edit_file` is the worked example: it guarded 1 of its 3 write paths, and *"the two unguarded paths are precisely the ones the caller reaches for."* The check has the shape already shipped for `IC-3` — **every entry point to a guarded operation routes through the guard** — expressible as `references()` on the guard function differenced against the public write entry points. Same set-difference as the tool-registry guard, with call sites in place of registrations.
- **Renaming stays legitimate and is often the honest fix** — a guard called `cargo-test-lib` misleads nobody — but it settles sub-shape 1 and part of 2 only.
- **State coverage at the refusal site.** The `IC-15` probe's `accepts_any_json` and the foreign-index hook's header are this done right: the limitation is written where a reader meets the guard, not in a tracker they will not open.
**Valid:** dated 2026-09-01

Distinguish this carefully from `IC-3` (*declaration is not execution*), which they are easy to merge and should not be. In `IC-3` the mechanism is **never reached** — a selector production does not emit, a CLI flag that does not exist. Here the mechanism runs, does real work, and returns a true result about a **smaller domain than its name claims**. `IC-3` fails at zero coverage; this fails at partial coverage, which is strictly harder to see because the guard demonstrably works every time you test it.

Two live examples visible from this session without consulting the archive. `cargo test --lib` in this repo's own pre-commit config was named "cargo test" and could not reach `tests/` at all, so the cluster gate it appeared to protect was never run (`4e5f060e`). And `doctor`'s `augmentation_declared_but_absent` fires only on a *declared* sidecar that is missing, so undeclared-and-unexported — the actually dangerous state — reads identically to nothing-to-declare (`IC-11`'s member). Both are guards whose names are broader than their reach.

**The tell, and it is cheap:** read the guard's name as a claim, then ask what input satisfies the name but not the implementation. If such an input exists and no other guard covers it, the name is the defect. Renaming is a legitimate fix here and is often the honest one — a guard called `cargo-test-lib` misleads nobody.

**Falsified by** an instance where the name and implementation agreed and the failure lay in the property itself being wrong.

## IC-15 — a parameter is accepted at the boundary and silently dropped downstream

**Slug:** `cluster/accepted-parameter-silently-dropped`
**Claim:** A parameter is accepted at the boundary — it validates, the call succeeds — and some path downstream discards it. The caller has positive evidence the value was set, because nothing rejected it, and no later observation distinguishes "applied" from "accepted and dropped".
**Members:** `filter={"tags": {"contains": "cluster/accepted-parameter-silently-dropped"}}` — n=17, 2026-09-02: two added by the tool-surface review, both reproduced live — `read-markdown-silently-ignores-offset-and-limit` (file tools; `read_file` fixed the identical drop on 2026-06-14 and the sibling never got the normaliser) and `activation-banner-names-a-project-param-symbols-does-not-have` (symbol navigation — a **seventh subsystem**, not counted in the spread below, which is **not re-adjudicated** for these two). Previously `n=15`, 2026-09-01, by query. **No longer single-party** — an independent blind second read (see the Index) agreed on 13 of 16. The count fell by one and the composition changed by three: **out** went `cli-artifact-drops-time-scope-and-extra` (to `IC-3` — no CLI flag exists, so nothing is accepted) and `update-entry-drops-entry-silently-when-fields-is-also-present` (to `IC-14` — the filed defect is the guard's conjunct); **in** came `doctor-outside-roots-sample-is-unranked-and-unreachable`, whose `limit` validates at the boundary and is read by no `doctor` code path.
**Blind party:** the caller, and specifically because acceptance is the only feedback the interface gives. Rejection is loud; silent discard is indistinguishable from success at every point they can observe.
**Promotes to:** `not yet` — **spread adjudicated 2026-09-01 — it clears.** Six subsystems across the 16 members: librarian `artifact`/`find`/`update_entry` (7), librarian `audit_doc_refs` (2), the cross-cutting `workspace=` pin (3 — `edit_code`, `memory`, `artifact(find)`), file tools (1), the friction probe (1), the CLI (1), plus `tracker_design` (1, added 2026-09-01). **One caveat kept rather than smoothed:** the workspace-pin trio is arguably *one* mechanism at three call sites, not three members, so an independent-mechanism count is lower than 16 — the raw tag count is a partition of *bug files*, not of causes. Both numbers are defensible for different questions; neither is quotable without its unit.
**Mechanism status:** **partial** — a behavioural probe exists and works, at **5 of 8** candidate tools as of 2026-09-02, and its shared half is no longer behind a feature gate. This field read `2 of ~7` until then, naming `artifact_event` and `artifact_refresh` as Owed after both had already shipped.

- **Shipped:** `every_action_labelled_schema_key_is_honored_by_that_action`, at **five** sites — `artifact` and `librarian` (2026-09-01), `artifact_event` and `artifact_refresh`, and `library` (2026-09-02, the first outside `src/librarian/`). For each schema key labelled `<action>:`, it calls that action twice — once with required params, once with the key set to a value **ill-typed for its declared schema type**. An honoured key is type-checked, so the calls differ; a discarded key is dropped by serde, so they are identical, and *identical is the defect*. The "extract it at the third site" instruction below is **discharged**: the shared half is `src/tools/param_probe.rs` (`94445195`, patch-id `dba8b18cf7926629194d49740ec950adf6d64526`).
- **Why it is NOT under `src/librarian/`, which is where it was written.** That tree is `#[cfg(feature = "librarian")]`, and every remaining candidate — `workspace`, `index`, `library`, `edit_markdown` — is exposed to this class while depending on that feature not at all. Reaching the probe from there meant gating a `workspace` guard on `feature = "librarian"`, which deletes it from the lean lane (`cargo test --workspace --no-default-features`), where a filtered-out test reads as *"filtered out"* and never as a failure: **a guard that is ABSENT reporting as a guard that PASSED.** Measured on the intermediate state, where the module had moved but no non-librarian caller existed yet: all four gate commands passed with **12** new dead-code warnings behind them, because the lean test lane carries no `-D warnings` and the clippy step runs `--features local-embed`, where the librarian callers do exist. Wiring `library` took that to **0 warnings and 1 lean-lane caller** — and it is the *caller* that discharges this, not the warnings; the two coincided here by luck rather than by rule.
- **The label convention is a SILENT filter, and it excluded exactly what it was built to count.** `sweep` derives a label as `desc.split(':').next()`. All four unprobed tools wrote their keys `For action='X': …`, which yields the label `For action='X'`, matches no action, and is skipped without a word. Wiring any of them before relabelling would have **checked zero keys and reported success** — and `floor` cannot see that per-key, only the convention breaking wholesale. *"Checked 0 keys, passed" and "checked 40 keys, passed" are the same green.* So the prerequisite at each remaining site is a relabel, not a wiring.
- **Why not a static check.** The obvious design — assert every declared param is a field on the `Args` struct — would have **missed the defect found on 2026-09-01**: `tracker_design::Args` declares both flagged fields, and the bug was one line further down, in `from_value(args).unwrap_or_default()` swallowing the error. Behaviour is the only surface where the two are distinguishable.
- **Why not `deny_unknown_fields`.** Measured, not assumed: it was tried and *"adding it once broke every `artifact(update)` call"* — the dispatcher passes `action` down and the shared schema holds sibling actions' keys, so every `Args` sees keys that are not its own. That prohibition is recorded at `find::Args` and is correct.
- **Known blindness, recorded as such:** a param read through an **untyped accessor** (`args.get(k).and_then(Value::as_str)`) has no ill-typed value, so the probe cannot speak for it — `doctor`'s `fix` and `offset` are in `ACCEPTS_ANY_JSON` for exactly this reason. Those reads are themselves a softer instance of this class: `doctor(fix=[])` runs a read-only scan and reports success.
- **Owed:** `workspace`, `index`, and `edit_markdown` — the third is a candidate this field never named, found by grepping the label form rather than by reading the list, and it is a peer's working file as of 2026-09-02. Each needs the relabel above *before* wiring, plus a floor **derived rather than chosen**: `library`'s reads 1, not 3, because `name` and `language` are read `input["name"].as_str()` — an untyped accessor where every wrong type reads as *absent*, so they sit in `accepts_any_json` as an admission, while `path` goes through `require_str_param_or_hint` and does type-check. That 1 was measured by setting the floor to 99 and reading `covered 1` back, which also proves the assertion fires: a floor nobody has watched fail is not evidence. `workspace(activate)` additionally mutates **process-wide** state, so its base args must fail *before* activating rather than after.
**Valid:** dated 2026-09-01

The class is well-attested in this repo outside the backfill. `artifact(create)`'s `augment` silently discarded five of its seven fields; the CLI's `artifact create`/`update` dropped `time_scope` and `extra`; `read_file`'s `force=true` was silently discarded on whole-file reads; `update_entry`'s entry-param guard fired only when `fields` was absent. All closed, all the same claim — a value the caller passed and the system took, then did not use.

**The frontmatter defect filed today is the same shape at document grain rather than parameter grain** (`docs/issues/2026-08-31-a-body-that-already-has-frontmatter-becomes-two-blocks.md`): the keys in the orphaned block were accepted onto disk and dropped from the catalog, so `status: fixed` in a file read `open` to every query. It is filed under `IC-6` because its *mechanism* is the absent escape hatch, and it is cited here rather than double-tagged — the one-tag rule.

Note the asymmetry that makes this worth a class rather than a bug-by-bug fix: the remedy is almost always to **refuse** rather than to start honouring the value. Honouring a long-dropped parameter changes behaviour for every existing caller who has unknowingly relied on it being ignored; refusing is loud, immediate, and tells them the truth. The frontmatter bug's `## Fix` argues exactly this and is the worked example.

**Falsified by** an instance where the parameter was honoured and the defect lay in what it did.

## IC-16 — an assertion that cannot fail is zero coverage wearing a passing test's clothes

**Slug:** `cluster/assertion-that-cannot-fail`
**Claim:** An assertion has **no input that would make it fail**. It is not weak coverage — it is zero coverage wearing a passing test's clothes, and it is added most often in the very commit that closes a missing-guard finding.
**Members:** `filter={"tags": {"contains": "cluster/assertion-that-cannot-fail"}}` — **n=3, 2026-09-01, by query.** Third instance filed 2026-09-01: `docs/issues/archive/2026-09-01-pinnable-assertion-vacuous-for-an-unregistered-tool.md` — `server_advertises_workspace_param_only_for_pinnable_tools` asserted `!pinnable.contains("get_usage_stats")` where `pinnable` is built from the **registry** and the tool was never registered, so no input could fail it. The other two are `ollama_large_batch_exceeding_batch_size` (vacuous the day it was written) and `cross-process-write-lock-test-passes-when-it-does-not-run` (vacuous when skipped). `CLAUDE.md` records four more from a single SDD run, untagged.
**Blind party:** the reviewer, structurally — a passing test is the evidence they are given, and vacuity is invisible in exactly that evidence. `CLAUDE.md` measures it: of four found in one run, *"the fourth only because the final reviewer was told to hunt for one."* Care does not find these; a changed question does. **The third instance is a clean confirmation:** it was not found by reading the test, but while resolving whether a *tool* was reachable — a different question that happened to pass through the same three lines.
**Promotes to:** **clears both bars as of 2026-09-01** — n=3 across three subsystems (embeddings transport, cross-process locking, MCP tool registry). What the third instance buys is **measurability, not a rule**: `CLAUDE.md` § *Testing Discipline* and § *SDD Rulings* already carry the substance (*"Ask 'what mutation would make this test fail?', never 'does it pass?'"*, and *demand a deliberate break*), so no rule is owed. The open item is the **mechanism**. This field previously read *"below threshold at `n=2`, which is now the only bar it fails"*; that bar is passed, and the sentence is superseded rather than deleted because the count is what moved and nothing else did.
**Mechanism status:** `designed` — the rule exists and is written down; nothing enforces it. Mutation testing per guarded site is the mechanism, applied by hand today. **The third instance names a narrower, buildable one:** an absence assertion over a name list should first assert the **positive** — that each listed name is actually produced by something in the population being searched — and only then that it is absent from the filtered subset. Without that, `!contains` cannot distinguish *correctly excluded* from *never present*. Not built; it would have caught this instance on the day it was written.
**Valid:** dated 2026-09-01

**Boundary against `IC-9`, which is a strict sub-case and must not absorb this.** `IC-9`'s assertion *can* fail — roughly 1-in-800, when a random tempdir name happens to contain the needle. Its mechanism is environment-controlled text in the haystack. This class is the harder one: **no input fails it at all**, so no run frequency, no environment and no amount of CI time will ever surface it. An `IC-9` member is a flake; a member here is a permanent zero.

That distinction is why the two withdrawn tags were withdrawn rather than left. Both read from their titles as *"a test that passes when it shouldn't"* — true of `IC-9` and true of this class and true of several others — and title-matching is what produced the misfit. The claim, not the title, is the admission test.

**This one is deliberately opened despite the rule already existing**, which reverses the usual direction: normally a cluster accumulates until it earns a rule. Here `CLAUDE.md` got the rule first, from an SDD run, and the *corpus* was never indexed against it — so the question *"which of our bugs are instances of the vacuous-assertion rule?"* has no answer, and nobody can tell whether the rule is working. Opening the class is what makes the existing rule measurable rather than merely stated.

**Falsified by** the identified members turning out to have a failing input after all, which would move each of them to `IC-9` or to an ordinary coverage gap.

## IC-17 — a shared resource carries no owner, so enumerating the peer does not help

**Slug:** `cluster/shared-resource-carries-no-owner`
**Claim:** A resource shared across sessions — the working tree, the git index, `target/`, a per-project state file, a `PREFIX-N` allocator — records *what* changed and never *who* changed it. Enumeration is not the binding constraint: a session that can name every peer still cannot tell which lines in the shared tree are its own. The remedy is isolating the resource or adding an owner field, never a better listing.
**Members:** `filter={"tags": {"contains": "cluster/shared-resource-carries-no-owner"}}` — n=21, 2026-09-02, re-derived with the gate's prescribed file-count form, not adjusted by delta. The first 15 were split out of `IC-1` the same day rather than found by a corpus pass: every one was already tagged `cluster/blast-radius-exceeds-visibility`, so none was new evidence. **The 16th is** — `docs/issues/2026-09-01-workspace-activation-is-process-wide-and-a-subagent-can-flip-it.md`, filed independently and caught by `tests/issue_clusters.rs`'s new count gate on its first run, about 40 minutes after a hand re-derivation had reported all 17 cells matching. **The 17th** is `docs/issues/2026-09-01-an-unstaged-pre-commit-config-blocks-every-session.md`, and it is the claim's remedy clause holding exactly: the refusal names the FILE and never the holder, so a blocked peer's only routes were an unbounded wait, a guess, or `--no-verify` — and a better peer listing would not have helped any of them. What is owed is an owner field on the resource, which this repo can already build: `scripts/pre-commit-foreign-index.sh` resolves a staged path to a `Session-Id` and prints its `SendMessage` address. **The 18th is `docs/issues/2026-09-01-two-correct-pre-commit-guards-have-an-empty-intersection.md`, and it is the claim's remedy clause read from the guard side:** the two hooks built to work around the missing owner field make **opposite** assumptions about which commit form is safe — `foreign-index` exits 0 only on a pathspec index (`scripts/pre-commit-foreign-index.sh:95-98`, re-checked against HEAD 2026-09-02 after `3bf2f5f5` shifted it by one), `ledger-counts` has no pathspec exemption at all (`scripts/pre-commit-ledger-counts.py`, `main()`) — so on an entangled index their intersection is empty and no commit form satisfies both. Neither guard is wrong; the field they are each substituting for is the one that does not exist. Give the index an owner and the tension dissolves, because `foreign-index` could then accept a bare commit restricted to the caller's own staged subset, which is exactly the form `ledger-counts` wants. **Filing it cost two peer round-trips to two different sessions**, because the count bump and the member sit in files a peer was writing in — the class obstructing the record of itself. **The 19th is `docs/issues/2026-09-02-the-concurrent-activation-guard-substitutes-proximity-for-identity.md`, and it is the class defeating its own mitigation:** the concurrent-activation guard exists *because* the active-project slot has no owner, and the missing owner field is precisely what stops the guard being accurate — it substitutes wall-clock proximity (different root within 5s) for caller identity, so one session's documented activate-foreign-then-return-home is indistinguishable from a two-caller race and warns identically. The tempting repair, exempting `HintScenario::ReturnToHome` (already computed one line above the call site), trades the false positive for a false negative on the *harmful* case, so the entry is filed with no fix proposed. It also records the misdirection the same gap produces: the warning is attached to the response of the call that moved the slot, so it reaches the switcher and never the party later resolved against a root it did not choose. Found not by an incident but by verifying a plan item that turned out to be moot — the only member so far reached that way. **The 20th is `docs/issues/2026-09-02-one-ledger-file-serializes-every-class-edit.md`, and it is this ledger holding about itself:** the file's *commit* granularity (one file) is coarser than its *edit* granularity (22 class records), and no per-hunk staging exists in this harness — so a session editing `IC-11` blocks a session editing `IC-22` with zero textual overlap. Measured 2026-09-02: the seven uncommitted hunks touched `IC-15` and three citation lines, `git diff -U0 | grep -c '^[+-].*IC-11'` returned **0**, and the blocked edit was an `IC-11` bump. Five sessions routed around the file in one night and three half-states accumulated. It is the claim's *enumeration is not the binding constraint* clause at its sharpest — every writer was positively identified by socket enumeration and three candidates positively excluded, and the file was still unsplittable, so a better listing was not merely insufficient but already complete. Two properties are stronger than "contention", both measured on peers rather than inferred: **the coupling forces exclusion or adoption**, because the Index cell must match the *staged* corpus, so a session bumping for its own member must carry every other pending member's file — `655c0b6f` documents choosing exclusion and saying so — which converts a queueing problem into a consent problem; and **the ledger's own remediation text is a participant**, since `IC-11`'s `**Members:**` instructs "`git add` first, then count", which on a shared index is an instruction to stage another session's file in order to *read* a number. The read-side half is separable, but **not by the union — that prescription was FALSIFIED at `8343d6ca`, and this sentence asserted it as verified until `e813be50`+.** `actual_counts` (`tests/issue_clusters.rs:511-527`) iterates `tracked_all_bug_files()`, which shells `git ls-files docs/issues` (`:320`), and the module header at `:1173` states the intent — *"the count gate sees TRACKED files only, so a local green defers rather than clears"*. So this field carries the **tracked** count, and a union including peers' untracked members reds `every_bare_n_in_a_class_field_matches_the_corpus`. The union measured clean only because its verifier held the untracked member personally and was committing it in the same operation, where union ≡ post-commit tracked necessarily — the one posture a *reader* never occupies. The correct rule: derive with the anchored `git grep -clE` and **no `git add`**; untracked members of the class are their owner's to count when they commit. **What survives untouched is the finding itself**, which the falsification does not reach: the `git add` prescribed by `IC-11`'s `**Members:**` is a *read* of the corpus whose only prescribed instrument mutates state every session shares, and a reader cannot opt out by declining to write.
**Blind party:** the session performing the write — and note it is blind *while holding two correct listings*. `git status` is complete and accurate; so, in these cases, is the peer listing. Neither carries the attribute that separates one session's work from another's. This is the reason "be careful" is the wrong instrument here rather than a weak one: care operates on what an instrument reports, and no instrument reports ownership because the field does not exist.
**Promotes to:** `OB-8` — *a shared resource carries no owner, so seeing the peer does not help*, promoted 2026-09-01 at `d710e58d`, before this entry existed. `OB-2` (the shared `target/` clobber, seen from the arming side) is the same class from a different standpoint.
**Mechanism status:** partial — an outbound `unreviewed-content` pre-commit gate for the commit path, and the gate reorder at `73066479` closing `target/`'s terminal state. Nothing owns the working tree, the git index, or `entry_high_water_<PREFIX>`. The **inbound** half is not closeable by any per-session behaviour.
**Valid:** dated 2026-09-01

### Why this is a separate class, and not a sub-family of `IC-1`

`IC-1` pre-registered its own falsifier: *"an instance where the writing session could enumerate
the peer and still collided — that would move the defect from visibility to coordination and
split this class in two."* It fired four times in 34 minutes on 2026-08-31/09-01, at `e0525462`
(23:53), `3a5aec7a` (23:55), `1b40dabd` (00:06) and `77d4da06` (00:27).

**Verified independently of the session that reported it**, because the timing claim is the whole
argument. This session's own transcript puts its `ListAgents` call at 2026-08-31T20:19:02Z —
**23:19 local** — and its first outbound message at 23:23. The margin is therefore wider than
first reported (~23:51): the earliest capture is 34 minutes after enumeration and 30 minutes
after first contact, and three of the four captures are this session's own. Enumeration was not
merely possible; it had already happened, been acted on, and produced eight messages about this
exact mechanism.

**And the sample was biased *against* this conclusion, which is what makes it strong.** Measured
2026-09-01: `ListAgents` returns **1 of the 4** live peer sessions whose `cwd` is inside this
checkout. The colliding pair was that one — mutually enumerable, while three of four
same-checkout peers were invisible to each other. So the falsification did not run on a typical
pair. It ran on the **most favourable sample available for coordination**, and coordination
failed four times anyway. A result that survives its own best case needs no argument about
typicality: had the pair been drawn from the invisible three-quarters, the same collision would
have been evidence for `IC-1` rather than against it. Counts: `cluster-promotion-session-log:F-3`.

**The discriminator is executable, and it is the remedy test** — the same one that kept
`IC-1`/`IC-2` and `IC-3`/`IC-15` apart. Ask of each member: *would a complete, correct peer
listing have prevented this?*

- **Yes** → `IC-1`. The instrument was short and reported the short count as sufficient.
- **No** → here. You would collide knowing exactly who your peer was.

Applied to `IC-1`'s 18 members it moves 15 and leaves 3, and the three that stay are all
*instrument* bugs — `cross-account-agents-cannot-see-each-other`,
`listagents-omits-cross-profile-sessions`, `peer-sessions-never-compares-start-time-to-build-time`.
The partition survives its own falsifier: no member moved here is one a listing would have saved.
The clearest case is `subagent-activate-mutates-parent-active-project`, where the parent
**spawned** the peer — enumeration is perfect by construction, and the collision happens anyway.

### The git index is a shared surface, and it was absent from every earlier remedy discussion

`git add` writes to the one per-checkout index. Staging a path does not make the *commit* about
that path: `git commit` with no pathspec commits the whole index, including whatever a peer
staged in the interval. That is how `1b40dabd` took a peer's entire `OB-6` entry — this session
staged one file, printed `git diff --cached --name-only`, and chained `&& git commit`, so the
check ran and changed nothing.

This is a **third** capture layer, under the two already recorded. `git add -A` sweeps untracked
peer files (`9741e418`); a co-edited file merges in the working tree regardless of pathspec
(`e0525462`); and the index carries pre-staged peer work into a commit that names neither the
file nor the peer. Remedies aimed at the first two do not touch the third — which is the general
shape of this class, and the reason a fix here needs the resource named rather than the behaviour
corrected.

### Residue, stated rather than forced

Two members arrive already flagged **suspected, not proven** — `workspace-read-only-flips-mid-session`
and `sdd-ledger-and-catalog-rows-vanished`. Both are unexplained state changes with no actor in
the owning session's history. They move here rather than staying with `IC-1` because the mechanism
they would instantiate, if a peer did it, is an unowned shared resource: any session's `activate`
mutates shared workspace state, which is `subagent-activate-mutates-parent-active-project` exactly.
They remain unproven, and this class is not credited with them as evidence.

## IC-18 — a selector is narrower than the population it names, and the members it never saw cannot be counted

**Slug:** `cluster/selector-narrower-than-its-population`
**Claim:** A selector — a glob, a regex, an `--include` list, a heading level, a resolution path — is narrower than the population its name or its caller's intent implies. It runs to completion over a subset and returns a **well-formed answer**, and because the excluded members were never examined there is no count to report and nothing to mark. A zero reads as *"not present"* rather than *"not looked at"*.
**Members:** `filter={"tags": {"contains": "cluster/selector-narrower-than-its-population"}}` — **n=7, re-derived 2026-09-02** by file count, not adjusted by delta. Four were tagged in the commit that opened this entry; the fifth was opened later the same day, and the sixth and seventh on 2026-09-02. Superseded figure, kept for its derivation: `n=6`, 2026-09-02, before the seventh was filed. Found by reading titles and then bodies; the title pass alone was itself a too-narrow selector, which is recorded below rather than smoothed over.
**Blind party:** the **reader of the result**, who is handed a number or a list with no surface on which under-coverage could have been reported. Distinct from `IC-13`'s blind party, who is handed a result the mechanism *knew* was partial. The author of the selector is differently blind: the selector is written from a model of where the population lives, so it fails exactly on the members that model omits — the blind spot is shaped like the author's own habits and does not look like a gap from inside them.
**Promotes to:** `docs/adrs/2026-08-27-negative-results-name-their-scope.md` (`713ca66260a29ed7`) — **already Accepted, and this class is the bug-corpus entry point to it rather than a competitor**, the same relationship `IC-11` declares to `DC` and `IC-16` to `CLAUDE.md`. The ADR's three clauses are this class's remedy for the *tool-facing* half: name the scope on a suspicious negative, stay silent on a trustworthy one, claim only what is proven. What the ADR does **not** reach is the other half — see *Mechanism status*.
**Mechanism status:** partial, and the split is by who wrote the selector. **Tool-facing:** covered in principle by the ADR above, with three established shapes (pre-flight predicate → `RecoverableError`; post-hoc audit counter beside the result; scope carried on an existing error). **Author-facing:** nothing, and nothing obvious is available — an ad-hoc `grep` typed into a shell has no output surface on which to annotate its own scope, so the ADR's remedy is structurally inapplicable. That asymmetry is the entry's open question, not a gap someone forgot to fill.
**Valid:** dated 2026-09-01

**Kept apart from `IC-13` on a falsifier, not on judgement.** `IC-13` is *a capped result presented as complete*: a limit was reached, the mechanism **knows** how much it dropped, and the remedy is to emit that marker. Here the selector never saw the excluded members at all, so **there is no count to emit** — "add a truncation marker" is not an available fix, and the remedy is to widen the selector or to name its scope. The discriminator to apply: *could the mechanism, unchanged, report how many it missed?* Yes → `IC-13`. No → this.

**Kept apart from `IC-14` on the same test, and the corpus already sorts itself this way.**
`IC-14` is *a guard's coverage is narrower than its name*; a guard's failure is that something **gets through**, and its remedy is to widen or rename the guard. A selector's failure is that a **result is silently partial**, and its remedy is to widen it or name its scope. The boundary was not imposed: `2026-08-17-librarian-guard-blind-to-artifacts-with-no-frontmatter` already carries `cluster/guard-narrower-than-its-name`, and every member below is a selector producing a *result*, none a guard producing a refusal. Where a single mechanism does both, file it by which failure the bug actually reports.

**The seven members, and what each one's selector excluded:**

- `2026-07-28-memory-sections-filter-matches-h3-only` — `memory(read)`'s `sections` filter matched `###` only; **15 of 21** memories use `##`. Section targeting silently returned nothing for the majority of the corpus it was aimed at.
- `2026-08-08-audit-doc-refs-never-scans-changelog-or-contributing` — the gate's file selector never reached `CHANGELOG.md` or `CONTRIBUTING.md`; **18 broken refs and 8 high findings** sat outside the scan while the gate reported green.
- `2026-08-26-archive-citation-sweep-grep-cannot-see-shell-or-yaml` — the sweep recipe `get_guide("tracker-conventions")` *hands the reader* covered `*.md`, `*.rs` and `.env*`; **six live surfaces** cite `docs/issues/` from file types that list cannot see, and one broke that day. This is the class holding about its own documented remedy.
- `2026-08-27-activate-by-path-bypasses-workspace-memory-resolution` — activating a sub-project by **path** rather than by **id** takes a resolution route that reports an **empty memory set** for a project that holds topics.
- `2026-09-01-tool-call-recorder-cannot-see-the-arm-under-evaluation` — `usage.db`'s recorder is mounted on the **MCP server boundary**, so native `Bash` calls are absent rather than filtered: **0 of 52,769** rows across 26 distinct `tool_name` values. A zero for shell-mediated misuse reads as *"none"* rather than *"never looked"*, and `security.shell_command_mode` is a **live eval arm** comparing exactly those two paths. The first member whose selector sits in the **writer** rather than the reader — see *Mechanism status*, since neither the tool-facing nor the author-facing half reaches it.

- `2026-09-02-tracked-only-staging-commits-half-an-archive-move` — `git add -u` is defined over paths that already hold an index entry, so the **untracked half** of an `artifact(move)` archive is never enumerated: three bug files staged as deletions with no archive twin, **703 of the tree's 953 deletions**, behind a green commit. The first member whose selector is a **stock git flag** rather than project code — there is no glob to widen and no response on which to annotate a scope, which is the author-facing gap named in *Mechanism status* rather than a new one.

- `2026-09-02-a-finished-bug-record-has-no-queryable-way-to-say-so` — a *terminal-but-unarchived* triage predicate is `status ∈ {fixed, mitigated, wontfix} AND path ∉ archive/`, so it never examines a record whose status **understates** its own body: two files carrying a fix SHA, a patch-id and a green gate sat at `status: open`, and the scan returned a well-formed **0** that was carried into a peer's backlog report before a reader opened the bodies. The first member where the excluded population is defined by the *field the selector keys on* rather than by a path, glob or boundary — widening is not available, because "actually finished" is not expressible over frontmatter at all. Its remedy is therefore neither of *Mechanism status*'s two halves: a **body-reading** sibling check (`non_terminal_status_with_fix_anchor`), which is the cost asymmetry that left one direction built and the other not.

Seven members, seven subsystems — the memory tool, `audit_doc_refs`, a documented shell recipe, workspace/memory resolution, usage recording, the bug-file archive flow, and bug-file triage. The spread is real rather than one component seen six times, which is this entry's own falsifier below.

**Found by a too-narrow selector, which is worth recording rather than smoothing.** The title pass that opened this class grepped for `only (scans|counts|reads|matches)|narrower|subset|blind to|misses` and returned **4 candidates, of which 2 survived** — the other two already belonged to `IC-14` and `IC-2`. Widening to bodies returned **60**. The population is therefore bounded below by 4 and not bounded above by anything measured here; **n=7 is a floor, not a census**, and the next reader should widen rather than trust it. Two further instances may arrive from `codescout-3c`'s IC-13 re-adjudication, whose no-fit readers pointed at this shape and are what prompted the entry.

**Falsified by** the members turning out to share one selector *mechanism* rather than the shape — four different globs in four subsystems is a class; four call sites of one helper is a bug in that helper.

## IC-19 — a truncated window is ordered by a key unrelated to why it was requested

**Slug:** `cluster/truncated-window-ordered-by-the-wrong-key`
**Claim:** A view that must drop items chooses which to keep by an ordering independent of — or exactly opposite to — the criterion that made the call worth making, so the shown sample **systematically** excludes the item that motivated it. The cap is honestly announced, and announcing it harder changes nothing: the remedy is the **selection**, never the marker.
**Members:** `filter={"tags": {"contains": "cluster/truncated-window-ordered-by-the-wrong-key"}}` — n=3, 2026-09-01, by query. Opened with its membership rather than before it: all three were `IC-13` members until the 2026-09-01 claim measurement found none of them satisfied that class, and two independent readers then agreed no existing class did (`cluster-promotion-session-log:F-6`).
**Blind party:** the author of the window, who holds the **fill order** and not the **criterion** — the two live at different layers, and scan order, line order or insertion order are all locally reasonable defaults. Recorded as a candidate; **not adjudicated for `OB`**, because it is arguable that any caller who inspects the sample can see the mismatch.
**Promotes to:** `not yet` — but note it **clears the count bar on creation**: 3 instances across 3 subsystems (`audit_doc_refs` findings, `grep`'s narrowing hint, `preview::headings`/`resolve_section_range`). Spread and `OB`-routing are unadjudicated, so this is a count that clears rather than a promotion earned.
**Mechanism status:** none yet — **not checked against the code as of 2026-09-02**, so read this as an open question rather than an established absence.
**Valid:** dated 2026-09-01

Three fill orders, one shape. `audit-doc-refs-gate-hides-its-own-cause` fills by **scan order** while the exit code turns on **severity**, so `exit 1` was returned with zero `high` findings visible in a 50-of-46572 window that honestly reported both numbers. `grep-narrowing-hint-ranks-by-capped-display-count` ranks candidates by **post-cap** counts, so it recommends 3-match files and never names the 20-match one. `append-entry-anchor-is-undiscoverable-through-the-surface-its-error-names` fills **head-first** while the anchor a caller needs is by convention the **last** heading — and that file states the discriminator this class turns on better than any restatement: *"Disclosure and discoverability are different properties… So the remedy is not 'disclose the truncation'."*

The class exists because that remedy is genuinely unavailable to `IC-13`. Its members already ship `headings_truncated: true`, `"shown": 50, "total": 46572`, and a sound overflow signal respectively — the marker is present, correct and useless, because it describes *that* something was dropped and never *that the dropped part is the part you asked about*.

**A fourth member is plausible and deliberately not claimed:** `docs/issues/archive/2026-08-08-doctor-outside-roots-sample-is-unranked-and-unreachable.md` (an unranked `SELECT` with no `ORDER BY`, so which 10 rows appear can change after a `VACUUM`) currently sits in `IC-15` on a dropped-`limit` argument, moved there by the 2026-09-01 blind second read. It is a *different* half of the same file, and under this ledger's own rule that a finding satisfying a second class's claim is a second bug file, it wants splitting rather than re-tagging. Left alone pending that.

**Falsified by** a member whose shown window excludes the wanted item by chance rather than by an ordering *systematically* unrelated to the request — that is an ordinary sampling limitation, and this class claims a structural mismatch.

## IC-20 — a floor is published under the name of a total, and the true value is unknowable rather than unreported

**Slug:** `cluster/floor-published-under-the-name-of-a-total`
**Claim:** A statistic computed over the subset a walk actually collected is published under the name of the whole population. Because the walk **stopped**, the true value is not merely unreported but **unknowable**, so neither a correction nor a marker repairs it — the remedy is to rename the quantity as a floor, or to refuse to print it.
**Members:** `filter={"tags": {"contains": "cluster/floor-published-under-the-name-of-a-total"}}` — n=1, 2026-09-01, by query.
**Blind party:** the caller, who receives a number *with a denominator* and has no way to learn the denominator describes the window rather than the world. A bare count invites the question "of how many?"; a ratio answers it, wrongly, and closes the inquiry.
**Promotes to:** `not yet` — n=1, below the count bar. Kept rather than folded into `IC-19` because the remedies differ: `IC-19`'s is a **selection** (derive against the pre-cap population), this one's is a **rename** (publish the number as a floor), and the second is what you are left with precisely when the first is impossible.
**Mechanism status:** none yet — **not checked against the code as of 2026-09-02**, so read this as an open question rather than an established absence.
**Valid:** dated 2026-09-01

`grep-showing-n-of-n-when-collection-hit-cap` is the seed and states the unknowability directly: *"The real total is not merely unreported — after `hit_cap` it is **unknown**, because the walk stopped."* Its own § Hypotheses records "report a truthful denominator instead" as **rejected — not available**, which is the whole of why this is not `IC-13`: that class's remedy is to make the marker arrive, and here there is no true value for a marker to carry. The shipped fix renames the quantity (`4 matches (capped)`, `total_is_lower_bound`).

**The entanglement with `IC-19` is real and is the argument that nearly merged them.** A second reader grouped this file with `IC-19`'s two on corpus evidence: `grep-narrowing-hint-ranks-by-capped-display-count`'s fix *reproduced this defect one level down* and had to add a floor marker of its own — *"Without this the fix would have replaced one piece of false precision with another one level down."* That is a genuine observation and it is recorded here rather than discarded: **the two classes co-occur because fixing a wrong ordering hands you a wrong denominator.** They stay apart on the remedy test, which is this ledger's stated discriminator, and `cluster-promotion-session-log:F-6` holds both sides.

**Falsified by** a member whose true total was recoverable — that is an ordinary reporting bug, fixed by reporting it, and this class claims the value is gone.

## IC-21 — an instrument reports presence or a count where the decision turns on magnitude

**Slug:** `cluster/instrument-omits-the-dimension-that-grows`
**Claim:** A surface whose purpose is to let a reader **decide** reports presence, or an item **count**, while the decision turns on **magnitude**. The reported dimension is uncorrelated with the cost, nothing errors, and the expense stays invisible until it is already large.
**Members:** `filter={"tags": {"contains": "cluster/instrument-omits-the-dimension-that-grows"}}` — n=2, 2026-09-01, by query. **The one pair both independent readers assigned identically**, with near-identical claims and the same remedy, so it is the least contestable of the four classes opened that day (`cluster-promotion-session-log:F-6`).
**Blind party:** the author of the instrument, who chose the dimension that was **easy to count** — rows, items, presence — at a layer where the cost had not yet accrued. A count is the natural thing to report and is right for most questions; nothing at the reporting site distinguishes the questions it is wrong for.
**Promotes to:** `not yet` — n=2, one short of the count bar. The two members already span two subsystems (`run_command`'s output buffer; the catalog audit trail), so a third instance meets both bars.
**Mechanism status:** none yet — **not checked against the code as of 2026-09-02**, so read this as an open question rather than an established absence.
**Valid:** dated 2026-09-01

`unfiltered-output-ref-carries-no-size-signal`: a `@cmd_*` handle is returned with no size, line count or emptiness signal, so a caller *"cannot judge whether reading the ref is worth a round-trip"* — and nothing distinguishes "we do not know what stdout was" from "stdout was empty". `audit-growth-concentrates-in-augmentation-params-health-blind-to-bytes`: `audit::health` reports `rows` and no bytes, and the distribution is the finding — **23 of 27,914 rows (0.08%) carried 88% of the payload bytes**, so a row count does not merely under-report the cost, it reports a quantity uncorrelated with it.

Both shipped fixes are the same move, which is the strongest evidence they are one class: add the magnitude field to the reporting surface — `unfiltered_output_lines`, and `payload_bytes` + `largest_payload_bytes`. Note the second names the **largest** row rather than only the sum; where a total would read as uniform growth, the distribution is the part that makes a concentrated cost visible.

**Deliberately not claimed as a third member:** the write-amplification half of the `audit-growth` file — a whole-blob `params` rewrite captured by `json_array(OLD.params, NEW.params)`, remedied by clamping oversize values in `UPDATE` diffs. Its general shape (*a mechanism sized for diffs applied to a column that **is** the blob*) is plausible and has **no second datapoint** in this corpus, so it is flagged undecided rather than made a class of one. Both readers reached that independently.

**Falsified by** a member where magnitude *was* reported and simply ignored by its reader — that is a reading failure, not an instrument that cannot say it.

## IC-22 — a next-step hint is composed from the response shape, not from the request

**Slug:** `cluster/hint-composed-without-the-request`
**Claim:** A system-authored next-step hint is derived from the **response's shape** rather than from the **request** that produced it, so it names a route answering a question the caller did not ask. Because that route returns real, plausible data, following it reads as *progress* rather than as an error, and the caller spends a call to arrive no closer.
**Members:** `filter={"tags": {"contains": "cluster/hint-composed-without-the-request"}}` — n=3, 2026-09-02, by query.
**Uncounted instances — two, deliberately not filed, and the reason is the count's own integrity.** Two archived bug files each carry an `IC-22` half as a *sub-finding*, invisible to the query above because a bug file carries exactly one `cluster/` tag and each is tagged for its other half:

| parent file | its tag | the `IC-22` half | state |
|---|---|---|---|
| `archive/2026-08-27-append-entry-anchor-is-undiscoverable-…` | `IC-19` | surface 2 — `allocate_entry_id`'s anchor error prescribed `artifact(action="get")`, *the surface that structurally cannot answer*: `get` windows headings from the **front**, the append anchor is by convention the **last** heading | **fixed** `ca8c550b`; the string *"Read the current headings"* returns 0 hits in-tree |
| `archive/2026-07-17-artifact-find-ignores-workspace-pin` | `IC-15` | sub-finding #2 — `scope="all"` downgraded to `umbrella` emitted `hints.expand: ["scope=\"all\""]`, suggesting **the parameter that was already passed** | **fixed**; regression test `scope_all_does_not_self_reference_expand_hint`, `src/librarian/tools/find.rs:1397` |

An earlier two-reader pass proposed **splitting** these into their own bug files. Re-entered 2026-09-01 and **declined**, on a fact neither reader had checked: both halves were already fixed *with regression tests*, so the split would create retroactive, mis-dated files for closed work — and would move this class from n=2 to **n=4**, crossing the ≥3 bar **by re-partitioning rather than by recurrence.** A promotion bought that way is indistinguishable, at the point of use, from one a real third instance earned.

So the honest split is between the two questions the count is asked to answer. **File count is 2** — what the Index cell declares and `every_index_count_matches_the_corpus` gates. **Instance count is 4**, and it is *that* number the ≥3 promotion bar names (*"three or more instances"*, not three or more files). This class therefore **clears the count bar on instances and is held below it on files**, which is a real disagreement between two defensible units and is left standing rather than resolved by filing paperwork. Promote it when a third instance arrives that someone *files* — or when this ledger decides the bar counts instances, which is a change to the rule and not to this row.

*(This is `IC-15`'s caveat — "the raw tag count is a partition of bug files, not of causes" — arriving with a live cost rather than as a note: here the two units give **different promotion verdicts**, where there it only made one number larger than another.)*
**Blind party:** the hint's author, who is writing at the **response** layer where the request's arguments are no longer in scope. The hint is correct *about the payload it can see*, which is exactly what makes it read as helpful.
**Promotes to:** `not yet` — n=3. The seed was **fixed the same day** (`bb4688fd`, patch-id `5e6ff450ad5eaf822283499492288b7ded15faf3`), so the hint surface is no longer live at `HEAD` — a later reader who reproduces it there has found a regression, not this class. The second member, `a-scoped-read-is-billed-the-full-heading-map`, carries the **preview** surface, which that fix explicitly declined as *"not owed"* for want of a measurement; the measurement has since been supplied (~81% envelope-to-payload on a scoped read). **It shipped and archived the same day** — `f3a76f81` / `aee9dd6b` / `b9bcfee4` on `experiments`, each with its patch-id recorded in the archived file. So **neither of the first two members is live at `HEAD`**. **The third is.** Added 2026-09-02, `cluster-gate-failure-text-prescribes-the-blindness-that-caused-it` reproduces at `HEAD` today — which is precisely the state this line previously named as the one in which a third instance would be most informative, arriving to collect. **The count bar is therefore cleared at n=3, by recurrence and not by the re-partitioning declined above.** The promotion judgement is owed and is deliberately not made here: the new member also widens the grain (see below), and a reader should decide those together rather than inherit one as a side effect of a count moving. **Grain question, unresolved:** this claim says a *next-step hint*, and a `preview` block is advisory payload that is not a hint. Admitting member 2 either widens the claim to "system-authored advisory" or wants a sibling class — flagged rather than silently widened.
**Mechanism status:** none yet — **not checked against the code as of 2026-09-02**, so read this as an open question rather than an established absence.
**Valid:** dated 2026-09-01

`heading-scoped-get-overflow-hint-points-at-metadata` is the seed: `artifact(action="get", heading=…)` buffers a large response and returns a hint naming `$.preview.headings[*]` — the envelope's own metadata — when the section the caller asked for is sitting at `$.body`. The `heading=` argument **was honoured**; the payload is correct and marked buffered. Nothing is capped-and-unmarked, which is why `IC-13` does not reach it, and nothing is accepted-then-dropped, which is why `IC-15` does not: that class's own *Falsified by* clause excludes a parameter that was honoured.

**It reproduced three times in one session, on the ledger that documents it.** On 2026-09-01 both independent readers hit it while classifying — one via `artifact(get, heading="## Index")`, one via `start_line=` — and the session's own controller had hit it hours earlier and worked around it by reaching for `$.body` without noticing the bug was already filed. A defect whose remedy is *"try the other key"* is cheap enough per instance that nobody reports it, which is a fair statement of why it stayed open while three parties tripped on it in an afternoon.

**A second member is already named and not yet split out:** `append-entry-anchor-is-undiscoverable-through-the-surface-its-error-names` carries this shape as its *second* surface — an error prescribing `artifact(action="get")`, the one surface that cannot answer the question — while its filed defect is the head-anchored window (`IC-19`). Under this ledger's rule that a finding satisfying a second class's claim is a second bug file, that half wants its own file rather than a second tag. Both readers reached the same conclusion independently. The same is true of `artifact-find-ignores-workspace-pin`'s `scope="all"` hint, which points at the parameter the caller already passed.

**Falsified by** a member whose hint was simply wrong — a typo, a stale route, a copy-paste — rather than *correctly derived from the wrong input*. This class claims the hint is right about the response and blind to the request.

## Template for new entries

```
## IC-N — <the class, stated as a claim>

**Slug:** `cluster/<slug>`
**Claim:** <the mechanism, in mechanism-language>
**Members:** `filter={"tags": {"contains": "cluster/<slug>"}}` — n=<count>, <YYYY-MM-DD>
**Blind party:** <who structurally cannot see it, and why> | `none — ordinary design defect`
**Promotes to:** `not yet` | <target ledger + id>
**Mechanism status:** none yet | designed | shipped (<what>)
**Valid:** dated <YYYY-MM-DD>

<Two or three paragraphs: what the instances share, what a fix would have to change, and
what would falsify the claim.>
```
