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
entry_high_water_IC: 17
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

## Index

> Hand-maintained reading surface. The `## IC-N — <title>` headings are what define the tokens
> and what `link_scan` resolves; this table is for scanning. `n` is a snapshot — re-run the
> `**Members:**` query before trusting it.
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
| IC-2 | a gate keyed on an event it cannot observe substitutes a proxy | `gate-keyed-on-unobservable-event` | 16 | `OB-6` — promoted 2026-09-01 | designed (exemplar shipped) |
| IC-3 | declaration is not execution | `declared-not-wired` | 19 | `OB-7` — promoted 2026-09-01 | **family 1 GATED** (`tests/tool_reachability.rs`); 2 of 3 families open |
| IC-4 | config propagation is additive | `config-propagation-is-additive` | 8 | `OB` — passes admission test; hook owed | none yet |
| IC-5 | the reproduction environment is not the gating environment | `repro-env-diverges-from-gate-env` | 11 | `H` — six subsystems; mechanism owed | none yet |
| IC-6 | an addressing scheme with no escape hatch | `addressing-without-an-escape-hatch` | 27 | `CLAUDE.md` § Parsers Over a Namespace — **landed** | shipped (partial) |
| IC-7 | lazy warm-up bills the first caller | `lazy-warmup-bills-the-first-caller` | 4 | not yet — 2 of 4 unconfirmed | shipped (partial) |
| IC-8 | a record asserts a completed action nothing re-checked | `record-asserts-an-unchecked-completion` | 5 | `DC` | none yet |
| IC-9 | an assertion over environment-controlled text is satisfiable by accident | `assertion-satisfiable-by-accident` | 1 | not yet — two tags withdrawn as misfits | none yet |
| IC-10 | authorship on a shared checkout is unrecoverable after the fact | `authorship-unrecoverable-after-the-fact` | 1 | not yet — below threshold | none yet |
| IC-11 | documentation denies a capability the code has since gained | `doc-contradicted-by-code` | 4 | clears count; spread unadjudicated | none yet |
| IC-12 | transient shared state lies to every reader | `transient-shared-state-lies-to-readers` | 0 | not yet — 1 instance, untagged; archive pass found none | none yet |
| IC-13 | a capped result is presented as complete | `capped-result-presented-as-complete` | 16 | clears count; spread unadjudicated | none yet |
| IC-14 | a guard's coverage is narrower than its name | `guard-narrower-than-its-name` | 8 | clears count; spread unadjudicated | none yet |
| IC-15 | a parameter is accepted then silently dropped | `accepted-parameter-silently-dropped` | 15 | clears count; spread unadjudicated | none yet |
| IC-16 | an assertion that cannot fail | `assertion-that-cannot-fail` | 3 | **clears both bars 2026-09-01**; rule already in `CLAUDE.md` — the third instance buys measurability, not a rule | designed; positive-form guard owed |
| IC-17 | a shared resource carries no owner, so enumerating the peer does not help | `shared-resource-carries-no-owner` | 15 | `OB-8` (+ OB-2) — 2026-09-01 | partial |

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
(`docs/issues/2026-08-30-core-hookspath-points-at-pre-rename-path.md` names it in both places).
Every `n` in the table above is a **file** count.

Snapshot 2026-09-01: **118 tagged of 495.** This read *"78 of the 357 files dated 2026-07-01 or
later are tagged and 279 are deliberately untagged … a further 137 pre-July files are
unbackfilled"* until then — four figures and a two-way partition, moved by `13226bda`, `77d4da06`
and `0dea2246` within one evening, and re-derived by none of the three commits that moved them.
**Every `n` above therefore remains a floor.** Covering the archive would need an explicit
`cluster/unclassified` slug meaning *looked, nothing fits* — a taxonomy decision, not a gate one.

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
**The 2026-09-01 archive pass had no independent second read, and that is the caveat to carry.**
One party (`codescout-e8`) did both the classification and the tag application for all 40 files
behind IC-13 (16), IC-14 (7), IC-15 (15) and IC-16 (2), plus the two IC-3 → IC-15 moves. Nobody
re-checked those assignments against the class claims. That is not a reason to distrust the
counts; it is the difference between them and the `IC-9` withdrawal, where a second read is
exactly what caught two misfits. A reader in a month cannot otherwise tell which of the two
regimes produced a given number.

Each tag was matched against the class's **stated claim**, never the bug file's title — title
matching is what put the two misfits in `IC-9` four hours earlier, because *"a test that passes
when it shouldn't"* is true of at least four classes. Members already carrying a `cluster/` tag
were left alone rather than re-adjudicated, with two deliberate exceptions named below.

**`IC-12` received nothing, and that is a finding rather than an omission.** Its claim requires a
*transient* mutation of shared state that the standard diagnostic reports as truth; the 279
untagged archive files contain plenty of shared-state pollution and none that is transient in
that sense. It stays at 0 on evidence, not on an unexamined queue.

**`IC-11` was deliberately not backfilled.** Roughly 15 doc-vs-code candidates exist, but its
claim turns on *"the prose was true when written"*, which is a fact about history and not about
the text — the same discriminator `claim-decay`'s inclusion test makes mandatory, and which
separates `decayed` from `never-true` only under `git log -S`. Tagging them without that probe
per file would be exactly the shortcut that ledger exists to forbid.

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
**Members:** `filter={"tags": {"contains": "cluster/gate-keyed-on-unobservable-event"}}` — n=16, 2026-08-31, by query after archive backfill.
**Blind party:** the gate itself, and therefore every reader of its output. The server process cannot see a compaction, a dead hook, a `/clear`, or what a parent session already holds; each of those is a *conversation*-scoped or *harness*-scoped event, and the gate is process-scoped.
**Promotes to:** `OB-6` — *a gate collapses "cannot observe" into the confident answer*, `docs/trackers/observer-blindness.md`, promoted 2026-09-01 at n=16. **The `OB-4` reference this field used to carry was loose and is corrected here**: `OB-4` is about the `.worktrees/bench` liveness marker and never mentions the rendezvous gate, so it was never "this class's rendezvous half". The two are siblings on one axis — `OB-4` asks *why a proxy is trusted* (an accuracy record it later spends), `OB-6` asks *what the proxy does when wrong* (returns the confident value rather than admitting it cannot tell) — and their remedies differ, so neither subsumes the other.
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
**Members:** `filter={"tags": {"contains": "cluster/declared-not-wired"}}` — n=18, 2026-09-01, by query. Was 20 until the `IC-15` boundary was settled on the remedy test; two members that accept a caller's value and drop it moved there. See the Index.
**Blind party:** the author of the declaration, specifically. They hold the mental model in which the wiring exists — writing `**Serves:** edit_file(path~/.claude)` *is* the act of believing it is served. A more careful version of the same author writes the same line.
**Promotes to:** `OB-7` — *a declaration is well-formed, and nothing in production reaches it*, `docs/trackers/observer-blindness.md`, promoted 2026-09-01. `OB-5`'s *Known-open residual* is this class seen from the **reporting** side — a check whose `extend()` line is deleted still reports `0` because the enum still declares it — so the two are cited across rather than merged: `OB-5` is about a summary that cannot say what **ran**, `OB-7` about a capability nothing **reaches**. The residual is where they touch, not where they are the same, which is why this got its own row instead of folding in.
**Mechanism status:** `partial` — decidable for one of three families, and deliberately **not** `designed` for the class. The 18 members split by what disconnects the declaration. **Dead in production (9):** the code exists and only tests call it. The partition is measured — `grep` tags every hit with its enclosing symbol and test sites carry a `tests/` prefix, and call-site granularity is *required* rather than preferable, since `references` groups `src/librarian/adapter.rs:1451` under a production file while `grep` shows it is `tests/…`. But it decides for **by-name call sites only**. *Corrected 2026-09-01 from a dispatch-side probe — this sentence used to name `dyn Trait` and `Arc<dyn CodeEmbedder>` as the blind spot, and that is false*: a trait object must be **constructed**, and construction is by name (`Arc::new(Grep)`), as is delegation (`"register" => RegisterLibrary.call(…)`). Dispatch consumes a name; it does not erase one. The genuine false-positive surface is **macro-generated names and re-export-only aliases**, much smaller than "every trait object" — and the failure direction is still the dangerous one, since a false *dead-in-production* finding is a **deletion-authorising** result on a negative search. The zero-caller state is no longer unexercised: the same probe found the first true positive (`ListFunctions`/`ListDocs`), but `references` was unavailable on it, so the finding rests on the text instrument alone. See `cluster-promotion-session-log:F-1` and `OB-7` § *PROBED*. **Schema or doc declares what the code ignores (3):** a round-trip check is only a weak proxy here, because reading a field is not using it. **A matcher that can never match (6):** this entry's original phrasing, needing the set of values production emits at a call site, and it has no mechanism at all. Recording the class as `designed` on the strength of the first family is exactly the conflation `IC-9` was corrected for.
**Valid:** dated 2026-08-31

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
**Mechanism status:** none yet.
**Valid:** dated 2026-08-31

`mcp-reconnect-applies-env-updates-but-not-env-deletions` is the sharpest statement of the class, because one edit made two changes and the outcomes diverged: `CODESCOUT_BM25_BOOST` changed from `3.0` to `5.0` and landed; `CODESCOUT_QUERY_PREFIX` was removed and is still set. The bug file's own title names the trap — *"the change that lands falsely confirms the one that did not"*.

`stale-model-dir-env-masked-by-shell` is the same asymmetry one layer down, and it is worth noting that its two stale values were masked for *different* reasons, which is why a single-cause fix would have missed one. `core-hookspath-points-at-pre-rename-path` is the rename form: `.git/config` still points `core.hooksPath` at the repository's former name, so git finds no hooks at all and `.pre-commit-config.yaml` silently never fires — while `CONTRIBUTING.md` documents the hook's behaviour in the present tense. Its archived sibling (`bench-worktree-gitdir-points-at-pre-rename-path`) is the same mechanism, which is what makes the rename form recurrent rather than incidental.

The generalisation worth extracting: **a config surface that supports removal needs a check on the removal, because the update path is the one everybody exercises.** Silence after a deletion is indistinguishable from success, and the value that remains is a live setting nobody intends.

**Falsified by** an instance where a removal *did* propagate and the failure lay elsewhere.

## IC-5 — the reproduction environment is not the gating environment

**Slug:** `cluster/repro-env-diverges-from-gate-env`
**Claim:** The environment built so failures can be reproduced locally is not the environment that gates. While the two agree the divergence is invisible; when they disagree the local run is authoritative-looking and wrong, and a genuine platform defect is indistinguishable from an environment gap.
**Members:** `filter={"tags": {"contains": "cluster/repro-env-diverges-from-gate-env"}}` — n=11, 2026-08-31, by query after archive backfill.
**Blind party:** `none — ordinary design defect`. A careful engineer comparing wine versions catches this; nobody is structurally prevented from seeing it. Recorded so the class is not mis-promoted to `OB`, whose admission test it fails.
**Promotes to:** `H` — `docs/trackers/codescout-usage-hookify.md`, adjudicated 2026-08-31 after the archive backfill. **Not `OB`**: `Blind party:` is `none`, which fails OB's admission test, so the routing question was never open. The old note read *"clears the count but not the subsystem spread; all three sit in the Windows/wine lane… Revisit if a fourth lands outside it"* — seven have. The eleven members span **six** subsystems: cargo feature config (3), wine/Windows (4), shell environment (1), cargo workspace resolution (1), toolchain (1), ambient embedder config (1). **It promotes as a worklist item, not a rule**, because `Mechanism status:` is still `none yet` and this ledger holds that a rule without one produces advice. The mechanism shape is a check diffing the documented four-command gate against `ci.yml`'s matrix; the four Windows lanes red on 2026-08-31 are a live instance of exactly what that would have caught.
**Mechanism status:** none yet. `scripts/build-windows.sh` could assert the wine version CI packages, which would convert the divergence from silent to loud at ~3 lines.
**Valid:** conditional — a member appears outside the Windows/wine lane

`scripts/build-windows.sh` exists precisely so Windows failures are reproducible without CI round-trips, and that purpose holds only while the two wines behave alike. They do not: `ubuntu-latest` packages wine 9.0, a current dev box runs wine 11.16, and in a single day the gap produced two divergences — one costing a CI cycle, one still costing a skipped test.

The other two members are the downstream cost of that gap rather than separate defects. `wine-lane-flakes-under-load` records three tests that failed together and passed on the next identical run, and its own update narrows the file to one test after CI reproduced two of the three *with a different payload* — a distinction only visible because someone compared the two environments deliberately. `windows-ci-timing-flakes` is `zombie` for the honest reason: both flakes resolve only by recurring, so no amount of effort reaches them.

This class is deliberately kept even though it does not currently promote. Its value is the **threshold rule's worked negative example**: three instances is not enough when they share a subsystem, and recording that judgement is what stops the next reader counting to three and promoting anyway.

**Falsified by** the two wine versions being shown to agree on the divergent cases, which would relocate the defect to the tests themselves.

## IC-6 — an addressing scheme with no escape hatch and no disambiguator

**Slug:** `cluster/addressing-without-an-escape-hatch`
**Claim:** An addressing scheme interprets every token in its namespace and provides no way to write one literally, or to disambiguate two that collide. The scheme is correct on every input it accepts; the defect is the input it makes unrepresentable.
**Members:** `filter={"tags": {"contains": "cluster/addressing-without-an-escape-hatch"}}` — n=27, 2026-08-31, by query.
**Blind party:** `none — ordinary design defect`. The gap is visible to anyone who tries the unrepresentable input; nobody is structurally prevented from seeing it. Recorded so it is not mis-promoted to `OB`.
**Promotes to:** `CLAUDE.md` § *Parsers Over a Namespace — owe an escape and a disambiguator* — **landed 2026-08-31**. Adjudicated the same day, after the archive backfill took it from n=2 to n=27, the largest class in the corpus. **Not `OB`**: `Blind party:` is `none`. It is codescout-specific engineering discipline, statable as one rule — *a parser over a namespace owes an escape for writing a token literally, and a disambiguator for two that collide.* Five subsystems: file-format navigation (`json_path`, `toml_key`), markdown editing (fences, heading-shaped content), the link/citation resolver (frontmatter delimiters, qualifiers, prefix collisions, doc-examples-read-as-citations), shell command gates (IL-3, dangerous-command, source gate, `run_command` — four separate gates, every one of them on heredocs), and symbol navigation (`name_path` with no disambiguator). Unlike IC-5 this one has partial mechanism already shipped, so the rule has something behind it.
**Mechanism status:** `shipped (partial)` — `edit_markdown` and `artifact(get)` gained an `occurrence` selector, closing the heading half for librarian-managed files. The `link_scan` half has no escape syntax at all.
**Valid:** dated 2026-08-31

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
**Mechanism status:** none yet.
**Valid:** dated 2026-08-31

One member, kept because the instance is unusually instructive. `bench-worktree-deletion-recorded-as-done-never-happened`: an archived bug file is `status: fixed`, `closed: 2026-08-16`, and its `## Fix` section states the worktree *"was removed with `git worktree remove --force .worktrees/bench`"*, closing with *"174 MB reclaimed, 163 MB of it regenerable `.codescout` index state."* The directory is still on disk. It is 174 MB, of which 163 MB is `.codescout`.

**The numbers match the live directory exactly, and that is the finding.** They were measured — correctly — *before* the deletion, then written in the past tense. So the record is not fabricated and not sloppy; it is a true measurement placed under a false verb. No plausibility check catches it, because every figure in it is right.

This is filed as a class of one deliberately rather than folded into `DC`. The threshold is not met and it will not promote on its own, but the *bug corpus* is where instances arrive, and a class with a defined slug is what lets the second instance find the first. If it stays at one for a long while, the correct disposition is to retire it here and keep the analysis in `DC`.

**Falsified by** the closure note turning out to have been written after a deletion that was later undone, which would make it a lost-work bug rather than an unchecked assertion.

## IC-9 — an assertion over environment-controlled text is satisfiable by accident, and fails open

**Slug:** `cluster/assertion-satisfiable-by-accident`
**Claim:** An assertion whose haystack embeds environment-controlled text — a path, a tempdir name, a hostname, a timestamp — can be satisfied by coincidence. It fails **open**: it passes on almost every machine and almost every run, so the green tick is evidence of luck rather than of the property.
**Members:** `filter={"tags": {"contains": "cluster/assertion-satisfiable-by-accident"}}` — n=1, 2026-08-31, by query. Went to 3 in the archive backfill and back to 1 the same evening: two of those tags were misfits and were withdrawn, see **Promotes to**.
**Blind party:** `none — ordinary design defect`, but with an unusually strong *detection* asymmetry: at ~1-in-800 the failure is unreachable by local reproduction, so the author's evidence is necessarily circumstantial. The file records that honestly in its own `unverified:` field.
**Promotes to:** `not yet` — n=1. When it moves, the target is `I` (`docs/trackers/test-escape-hardening.md`), because the remedy is a standing check rather than a rule anyone remembers — but see **Mechanism status**: the check is not the grep this entry used to name.
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
**Members:** `filter={"tags": {"contains": "cluster/authorship-unrecoverable-after-the-fact"}}` — n=1, 2026-08-31. The seed narrative below is not a bug file and is not counted.
**Blind party:** every party equally, which is what makes it different from an ordinary mistake. The information does not exist to be careless with: `git` collapses all sessions into one author string, and an untracked file carries no origin at all.
**Promotes to:** `not yet` — n=1. When it moves the target is likely `H` (a provenance channel is a mechanism, not a discipline), not `OB`.
**Mechanism status:** none yet.
**Valid:** dated 2026-08-31

**Split from `IC-1` deliberately, on the remedy test.** `IC-1` claims a write reaches further than the set of peers you can see, and its remedy is an ownership protocol over the shared resource. This class claims something narrower and later: once the write has happened, *who did it* is not recoverable. Its remedy is a provenance channel. Same substrate, different missing thing — which is the same test that keeps `IC-1` and `IC-2` apart despite both reducing to "a component reasoning about a scope it cannot observe". `buddy-compact-banner-names-a-peers-session-as-your-own` was filed under `IC-1` and is moved here: its defect is that `from=<sid>` names another live session as your own predecessor, which is misattribution, not blast radius.

**Seed evidence — an exchange between two sessions produced three misattributions, all while reasoning about this class.** Sessions `codescout-kat` and `codescout-23`, 2026-08-31, both actively working the `IC` ledger:

1. `codescout-kat` told `codescout-23` "your nested-hook-state bug reasons that session 3a6d634e… wrote `.buddy/`". The reasoning is in that file, but the file is not `codescout-23`'s.
2. `codescout-kat` warned `codescout-23` that "your untracked librarian-runtime bug file" would red the cluster gate. Also not theirs.
3. `codescout-23`, correcting the above, argued the file was `codescout-kat`'s because *"your own `2ed2e716` calls it 'my nested-hook-state bug'"*. `codescout-kat` authored exactly two commits that session, `351836a8` and `522675a6`; `2ed2e716` is neither.

The file belongs to a third session neither had enumerated. `git log` shows why the dispute was unresolvable from inside it: `2ed2e716`, `e14b230e`, `351836a8` and `522675a6` all read the same author and email, because git has no session dimension — the field is a constant and carries zero information. The one channel that did work was accidental: `.buddy/by-ppid/<pid>/session_id` on disk, which exists for unrelated reasons and is untracked.

Note the pattern is `OB-1`'s — *"the author, specifically"*. All three attributions were made by parties who had just read the evidence, in messages *about* attribution failure. Knowing the class prevented none of them, which is the standing argument against answering this kind of defect with care rather than mechanism.

**Falsified by** an attribution dispute on a shared checkout that a party could settle from committed state alone.

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
**Members:** `filter={"tags": {"contains": "cluster/doc-contradicted-by-code"}}` — n=4, 2026-09-01, by query after a probed archive pass. **Fourteen candidates were probed and three passed**; the ten that did not are deliberately untagged, not pending. See *The probe* below..
**Blind party:** the *reader*, routed to the document by its own scope claim and given no signal to cross-check. The author of the prose is not blind — they wrote something true. The author of the *code* change is differently blind: gaining a capability gives you no reason to search prose for sentences your feature just falsified.
**Promotes to:** `not yet` — n=1 taggable. The likely target is `DC` (`docs/trackers/claim-decay.md`): a true-when-written claim that silently decayed is that ledger's subject, and this class is the bug-corpus entry point to it rather than a competitor — the same relationship `IC-8` declares.
**Mechanism status:** none yet, and the nearest existing mechanism does not cover it. `librarian(action="audit_doc_refs")` lints *references* — paths, symbols, line numbers, link targets — so a document may cite every path correctly and still assert the opposite of what the code at those paths does. The remedy would have to check claims, not refs.
**Valid:** dated 2026-08-31

Seed instance: `2026-08-31-librarian-runtime-guide-denies-the-augmentation-sidecar`. The served `librarian-runtime` guide states augmentation has *"**No** — there is no on-disk representation"* and that sharing it is *"local-only by design"*. Both sentences were accurate when written. The sidecar shipped as `e799f29d` on 2026-08-30, and a deliberate sweep the **same day** — `e1b91221`, *"state that augmentation shape now travels, in the three places that said otherwise"* — corrected `CLAUDE.md`, `docs/conventions/cross-machine-catalog-resume.md` and `tracker-conventions.md`. Not this guide. So the drift is **one day old**, and the mechanism is an enumeration produced from memory, not neglect: "three places" reads as a finding and is a list. The guide mentions `sidecar`/`expects_augmentation` zero times; `tracker-conventions` mentions them thirteen.

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

## IC-12 — transient shared state lies to every reader, and the standard diagnostic confirms the lie

**Slug:** `cluster/transient-shared-state-lies-to-readers`
**Claim:** One session's tooling mutates shared state for the duration of an operation. Every other session's read is wrong for that window, and the standard diagnostic reports the lie as truth rather than as an outage — so the symptoms are indistinguishable from permanent loss.
**Members:** `filter={"tags": {"contains": "cluster/transient-shared-state-lies-to-readers"}}` — n=0 tagged, 2026-09-01. One measured instance, written up as *"The read-side twin"* inside `docs/issues/2026-08-31-peer-commit-captures-another-sessions-working-tree.md` rather than as its own file, so there is nothing to tag yet.
**Blind party:** the *reading* session, and note the inversion — every other class here blinds a writer. Here the writer is fine and the reader is deceived, by an operation it did not initiate and cannot see.
**Promotes to:** `not yet` — one instance, and the remedy so far is knowledge rather than mechanism.
**Mechanism status:** none yet. Documented at the point of use (`scripts/pre-commit-unreviewed-content.sh` header, `0b763983`), which is a knowledge fix and by this ledger's own rule a worklist item rather than a rule.
**Valid:** dated 2026-09-01

Measured 2026-08-31, within a minute of git hooks being enabled on this shared checkout. The pre-commit framework stashes unstaged changes while hooks run, and that stash covers **every** session's in-flight work, not only the committing one's. For the sub-second duration of a peer's commit, a session observed its own edited file revert to HEAD content, `git status` report it clean, and a `grep` for text it had just written return nothing.

**The detail that makes it a class rather than a footnote: `git stash list` is EMPTY throughout.** pre-commit writes a patch under `~/.cache/pre-commit` instead of using `git stash`, so the obvious way to detect a stash reports that there is not one. The reader is not merely misinformed — the instrument they would reach for to check confirms the false reading. There is no opt-out; `pre-commit run --help` exposes no stash flag and the stash is unconditional when unstaged changes exist.

**The danger is not the window, it is reacting inside it.** Rewriting a section from memory races the restore and can genuinely lose or duplicate work while "recovering" from a problem that has already fixed itself. So the remedy is an oracle, not a fix: for a librarian artifact, `artifact_event(action="list")`'s `field_patch` byte counts, which no git operation touches; for anything else, `wc -c <path>` against `git show HEAD:<path> | wc -c`. Never `git status`.

**Kept apart from `IC-1` on the observer, not the substrate.** `IC-1` is a write reaching further than the set of peers you can see; here no write collides at all and the shared state is correct at both ends of the window. It generalises past `pre-commit` to anything that transiently mutates shared state — a formatter run, a build that moves files, a script that checks out.

**Falsified by** an instance where the standard diagnostic correctly reported the transient state as unavailable rather than as settled truth. That is an outage, which is a different and much safer thing.

## IC-13 — a capped result is presented as complete, so a partial answer reads as the whole one

**Slug:** `cluster/capped-result-presented-as-complete`
**Claim:** A result is truncated by a limit — a page size, a byte budget, a display cap — and returned without a marker saying so. The caller reads a partial answer as the whole answer, and a **zero** from a capped scan reads as "not present" rather than "not reached".
**Members:** `filter={"tags": {"contains": "cluster/capped-result-presented-as-complete"}}` — n=16, 2026-09-01, by query after the archive backfill pass. Single-party classification — see the Index caveat. **Provenance: three or more instances identified by the 2026-08-31 archive backfill pass (`8b13b5f3`), which I have not independently verified.** The count is a peer's, the membership is unassigned, and the query is honest about that: it returns nothing until the files are tagged.
**Blind party:** the caller, who has no way to distinguish a short list from a complete one. Also the *author of a downstream count*, since an aggregate computed over a capped scan is wrong in a direction nothing signals.
**Promotes to:** `not yet` — the count is met (n=16); the **spread is unadjudicated**, and that is now the only bar. Members were tagged at `13226bda` (2026-09-01), discharging the *"needs its members tagged"* condition this field used to carry; nobody has yet counted the subsystems they span, and the classification remains single-party (see the Index caveat).
**Mechanism status:** none yet, though the shape of one is known — `link_scan` already carries a per-array `counts.truncated` flag, and `run_command`'s buffer envelope carries `unfiltered_truncated`. Both are the pattern to copy.
**Valid:** dated 2026-09-01

This class is opened deliberately **before** its membership exists, which is a departure worth stating. The ledger's rule is that a member list rots and a query does not; the cost of that rule is that a class identified but not yet tagged reads as n=0. Recording the provenance in `**Members:**` is the compromise — a reader sees both the claim and the fact that nothing has been assigned to it yet, and cannot mistake the zero for evidence of rarity.

The archived instances are not yet cited here because I have not read them. What is independently visible is that this repo has treated the shape as real for some time: `truncate_compact` cutting from the tail and destroying the overflow signal, `grep` printing a self-refuting *"Showing N of N"* when collection hit the cap, and `link_scan`'s dangling count being prefix-gated so a whole namespace could read as healthy — all closed, all the same claim.

**Falsified by** the backfill's three instances turning out to share a subsystem rather than a mechanism, which would make this a broken component rather than a class.

## IC-14 — a guard's coverage is narrower than its name, so the name is what everyone reasons with

**Slug:** `cluster/guard-narrower-than-its-name`
**Claim:** A guard's name states the property; its implementation covers a subset of it. Everything the name promises is believed protected, the uncovered remainder is protected by nothing, and the guard's own green result is what conceals the gap.
**Members:** `filter={"tags": {"contains": "cluster/guard-narrower-than-its-name"}}` — n=7, 2026-09-01, by query after the archive backfill pass. Single-party classification — see the Index caveat. **Provenance: three or more instances identified by the 2026-08-31 archive backfill pass (`8b13b5f3`), unverified by me.**
**Blind party:** everyone downstream of the name. The implementer knows the scope at the moment they write it; every later reader knows only the name, and the name is what they reason with. This is `OB-1`'s shape — the parameter the author's context supplied for free.
**Promotes to:** `not yet` — the count is met (n=7); the **spread is unadjudicated**, and that is now the only bar. Members were tagged at `13226bda` (2026-09-01), so the count is no longer unverified — it is unaudited: single-party classification, see the Index caveat.
**Mechanism status:** none yet.
**Valid:** dated 2026-09-01

Distinguish this carefully from `IC-3` (*declaration is not execution*), which they are easy to merge and should not be. In `IC-3` the mechanism is **never reached** — a selector production does not emit, a CLI flag that does not exist. Here the mechanism runs, does real work, and returns a true result about a **smaller domain than its name claims**. `IC-3` fails at zero coverage; this fails at partial coverage, which is strictly harder to see because the guard demonstrably works every time you test it.

Two live examples visible from this session without consulting the archive. `cargo test --lib` in this repo's own pre-commit config was named "cargo test" and could not reach `tests/` at all, so the cluster gate it appeared to protect was never run (`4e5f060e`). And `doctor`'s `augmentation_declared_but_absent` fires only on a *declared* sidecar that is missing, so undeclared-and-unexported — the actually dangerous state — reads identically to nothing-to-declare (`IC-11`'s member). Both are guards whose names are broader than their reach.

**The tell, and it is cheap:** read the guard's name as a claim, then ask what input satisfies the name but not the implementation. If such an input exists and no other guard covers it, the name is the defect. Renaming is a legitimate fix here and is often the honest one — a guard called `cargo-test-lib` misleads nobody.

**Falsified by** an instance where the name and implementation agreed and the failure lay in the property itself being wrong.

## IC-15 — a parameter is accepted at the boundary and silently dropped downstream

**Slug:** `cluster/accepted-parameter-silently-dropped`
**Claim:** A parameter is accepted at the boundary — it validates, the call succeeds — and some path downstream discards it. The caller has positive evidence the value was set, because nothing rejected it, and no later observation distinguishes "applied" from "accepted and dropped".
**Members:** `filter={"tags": {"contains": "cluster/accepted-parameter-silently-dropped"}}` — n=15, 2026-09-01, by query after the archive backfill pass (13 newly tagged, 2 moved from `IC-3`). Single-party classification — see the Index caveat. **Provenance: three or more instances identified by the 2026-08-31 archive backfill pass (`8b13b5f3`), unverified by me.**
**Blind party:** the caller, and specifically because acceptance is the only feedback the interface gives. Rejection is loud; silent discard is indistinguishable from success at every point they can observe.
**Promotes to:** `not yet` — the count is met (n=15); the **spread is unadjudicated**, and that is now the only bar. Members were tagged at `13226bda` (2026-09-01) — 13 newly tagged plus the two moved from `IC-3` on the remedy test — so the count is no longer unverified, it is unaudited: single-party classification, see the Index caveat.
**Mechanism status:** none yet.
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
**Promotes to:** **clears both bars as of 2026-09-01** — n=3 across three subsystems (embeddings transport, cross-process locking, MCP tool registry). What the third instance buys is **measurability, not a rule**: `CLAUDE.md` § *Testing Discipline* and § *SDD Rulings* already carry the substance (*"Ask 'what mutation would make this test fail?', never 'does it pass?'"*, and *demand a deliberate break*), so no rule is owed. The open item is the **mechanism**. This field previously read *"below threshold at n=2, which is now the only bar it fails"*; that bar is passed, and the sentence is superseded rather than deleted because the count is what moved and nothing else did.
**Mechanism status:** `designed` — the rule exists and is written down; nothing enforces it. Mutation testing per guarded site is the mechanism, applied by hand today. **The third instance names a narrower, buildable one:** an absence assertion over a name list should first assert the **positive** — that each listed name is actually produced by something in the population being searched — and only then that it is absent from the filtered subset. Without that, `!contains` cannot distinguish *correctly excluded* from *never present*. Not built; it would have caught this instance on the day it was written.
**Valid:** dated 2026-09-01

**Boundary against `IC-9`, which is a strict sub-case and must not absorb this.** `IC-9`'s assertion *can* fail — roughly 1-in-800, when a random tempdir name happens to contain the needle. Its mechanism is environment-controlled text in the haystack. This class is the harder one: **no input fails it at all**, so no run frequency, no environment and no amount of CI time will ever surface it. An `IC-9` member is a flake; a member here is a permanent zero.

That distinction is why the two withdrawn tags were withdrawn rather than left. Both read from their titles as *"a test that passes when it shouldn't"* — true of `IC-9` and true of this class and true of several others — and title-matching is what produced the misfit. The claim, not the title, is the admission test.

**This one is deliberately opened despite the rule already existing**, which reverses the usual direction: normally a cluster accumulates until it earns a rule. Here `CLAUDE.md` got the rule first, from an SDD run, and the *corpus* was never indexed against it — so the question *"which of our bugs are instances of the vacuous-assertion rule?"* has no answer, and nobody can tell whether the rule is working. Opening the class is what makes the existing rule measurable rather than merely stated.

**Falsified by** the identified members turning out to have a failing input after all, which would move each of them to `IC-9` or to an ordinary coverage gap.

## IC-17 — a shared resource carries no owner, so enumerating the peer does not help

**Slug:** `cluster/shared-resource-carries-no-owner`
**Claim:** A resource shared across sessions — the working tree, the git index, `target/`, a per-project state file, a `PREFIX-N` allocator — records *what* changed and never *who* changed it. Enumeration is not the binding constraint: a session that can name every peer still cannot tell which lines in the shared tree are its own. The remedy is isolating the resource or adding an owner field, never a better listing.
**Members:** `filter={"tags": {"contains": "cluster/shared-resource-carries-no-owner"}}` — n=15, 2026-09-01, split out of `IC-1` the same day rather than found by a corpus pass. Every member was already tagged `cluster/blast-radius-exceeds-visibility`; none is new evidence.
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
