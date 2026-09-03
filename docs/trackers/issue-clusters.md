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

So: a `**Members:**` line carries the query, plus a per-member derivation saying why that
instance belongs to this class. It does **not** carry a bare count — counts are derived
(`scripts/probe-cluster-census.py`) and `no_class_field_states_a_bare_n` refuses a stored one.
Trust the query; run the probe before trusting any figure.

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
| `**Members:**` | the query, plus a per-member derivation. **Never a bare `n=`** — counts are derived (`scripts/probe-cluster-census.py`), and `no_class_field_states_a_bare_n` refuses one here |
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

**Classify by the claim, never by adjacency to a known instance.** `IC-6` and `IC-18` both
present at the surface as *"the pattern matched the wrong thing"*, and the discriminator is
**direction**: `IC-6` matches too much or cannot separate two tokens that collide, so it binds the
**wrong** target; `IC-18` matches too **little**, so the members it never saw cannot be counted.
Ask which one the finding does and the answer is usually immediate.

Proximity pulls the other way, and it is closer to anti-evidence than to evidence: a second defect
found *in the same file* as a known instance is exactly as likely to be a different class, and the
shared location is precisely what makes it look like a duplicate. Recorded because it has now
happened twice, both times reaching for `IC-6` on a too-narrow selector —
`declared-patch-ids-per-line-scan-misses-a-wrapped-value` and
`comm-filter-misses-version-pinned-claude-processes`, the second diagnosed by the peer who made
it (*"I classified by adjacency — same script, one line away — rather than by the claim"*). The
cost is not a mislabel: the buried half is systematically the class **nearest a threshold**, so
adjacency-classification suppresses exactly the counts that were about to promote. This is the
same rule `CLAUDE.md` § *Reaching a Peer Session* states for authorship — *never route by
adjacency* — arriving at classification, and it fails the same way in both places.

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

> **This file is the INDEX. It is a pointer, not a place to write.**
>
> Since 2026-09-02 each class record lives in its own file, `docs/trackers/issue-clusters/IC-N-<slug>.md`.
> An Index row is a one-line locator; the `**Members:**` derivation, the `**Promotes to:**`
> reasoning and the `**Mechanism status:**` all belong in the class file. Writing a derivation
> into an Index cell instead re-creates, one cell at a time, the exact coupling the split
> removed — **16 distinct sessions and 53 commits on this one file in a day**, 3× the next
> file in the repo.
>
> **Filing a NEW class is two steps, and the second is gated.** `append_entry` splices the new
> section into the parent artifact's own file — this one — because that is where the id and the
> citable `## IC-N — <title>` heading are allocated. So: append here, then move the section to
> its class file and leave the Index row behind, in one commit.
> `tests/issue_clusters.rs::the_index_file_holds_no_class_sections` reds until you do, and names
> the fix. It is a gate rather than a note because a step the next filer has to remember is a
> policy, not a mechanism.
>
> Erosion guard suggested by `codescout-0a` (sessionId `2cb44cd3`) on reviewing the split:
> the failure it names — a full derivation appended into a trunk cell — is the one the test
> above cannot see, because it is not a section.

## Index

> Hand-maintained reading surface. The `## IC-N — <title>` headings are what define the tokens
> and what `link_scan` resolves; this table is for scanning.
>
> **This table stores no count, and that is the design rather than an omission.** Read live
> membership with `python3 scripts/probe-cluster-census.py`. Until 2026-09-02 an `n` column sat
> between the slug and the verdict, and a stored copy of a derived value in a file 22 classes
> share made every bug filer edit it. It went stale by **concurrency**, not neglect — measured
> 2026-09-01, three separate re-derivations were invalidated inside one session (IC-3 20→22 and
> IC-6 29→30 while a blind audit was running, then IC-2, IC-13 and IC-14 each +1 two hours later),
> every one a peer filing bugs in the same checkout and none a mistake by whoever last wrote the
> cell. A sweep's own result is falsified by the next commit, so no amount of care held it.
>
> **What the row keeps is what no query derives:** the promotion verdict, the subsystem spread,
> and mechanism status. Those are adjudications. The row used to mix a derived counter with a
> human verdict, which is precisely why bumping the counter forced a write to the verdict's file.
> Historical counts in the entries below are **backticked** — `` `n=27` `` — and that is the
> ledger's escape for a quotation: preserved with its derivation, never updated, ignored by both
> parsers. A *bare* `n=` is now a gate failure rather than a claim to check.
>
> **The gate did not go away, it changed what it asks for.** The count is what made a ledger edit
> mandatory, and that is why per-member derivations exist at all — authors wrote them while
> satisfying the refusal (measured on `1b92a7de`: one bug filing added 1,508 characters of
> hand-authored, non-derivable prose across the three lines it touched for the number). So
> `tests/issue_clusters.rs` and `scripts/pre-commit-ledger-counts.py` now refuse a commit that
> **stores** a count, and refuse one where a class **gains a member without `**Members:**` naming
> it**. Deliberately not *"did the line change"* — a trailing space satisfies that, which is
> `cluster/assertion-satisfiable-by-accident`, and the count it replaced could not be satisfied by
> accident. Raised by `codescout-17` and falsified before it shipped.
>
> *(This passage read "asserting each table cell against its derived count is the missing gate"
> until 2026-09-01, after the gate had already shipped, and then described that gate until
> 2026-09-02, after it had been inverted — `cluster/doc-contradicted-by-code`, which is `IC-11`,
> twice, inside the ledger that defines it.)*
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

| id | class | slug | promotes to | mechanism |
|---|---|---|---|---|
| IC-1 | the blast radius of a write is wider than the set of peers you can see | `blast-radius-exceeds-visibility` | `OB-3` — 2026-09-01 | partial; **split taken → IC-17** |
| IC-2 | a gate keyed on an event it cannot observe substitutes a proxy | `gate-keyed-on-unobservable-event` | `OB-6` — promoted 2026-09-01 | designed (exemplar shipped) |
| IC-3 | declaration is not execution | `declared-not-wired` | `OB-7` — promoted 2026-09-01 | **family 1 GATED** (`tests/tool_reachability.rs`); 2 of 3 families open |
| IC-4 | config propagation is additive | `config-propagation-is-additive` | `OB` — passes admission test; hook owed | **partial** — 2 of 8 surfaces (`hooksPath`, worktree gitdir) |
| IC-5 | the reproduction environment is not the gating environment | `repro-env-diverges-from-gate-env` | `H` — seven subsystems; mechanism owed | none yet |
| IC-6 | an addressing scheme with no escape hatch | `addressing-without-an-escape-hatch` | `CLAUDE.md` § Parsers Over a Namespace — **landed** | shipped (partial) |
| IC-7 | lazy warm-up bills the first caller | `lazy-warmup-bills-the-first-caller` | not yet — 2 of 4 unconfirmed | shipped (partial) |
| IC-8 | a record asserts a completed action nothing re-checked | `record-asserts-an-unchecked-completion` | `DC` | none yet |
| IC-9 | an assertion over environment-controlled text is satisfiable by accident | `assertion-satisfiable-by-accident` | not yet — two tags withdrawn as misfits | none yet |
| IC-10 | authorship on a shared checkout is unrecoverable after the fact | `authorship-unrecoverable-after-the-fact` | **clears both bars 2026-09-01** — n=3, spread 3, via second-read retag | none yet — candidate is `H` (a provenance channel for working-tree state) |
| IC-11 | documentation denies a capability the code has since gained | `doc-contradicted-by-code` | clears count; **spread adjudicated 2026-09-01 — 4 doc surfaces / 4 subsystems**, not re-adjudicated for the sixth (`CLAUDE.md` § *Observer Blindness* denying a pid→session join the session registry carries) nor for the seventh (a Rust **doc comment** — a fifth surface, and the first member whose prose sits inside the file it describes) nor for the fifteenth (a **tracker worklist field** — a sixth surface, and the first outside code and schema entirely) nor for the sixteenth (a **Rust doc comment citing a moved symbol** — a seventh surface, and the first for which the deriving sweep's own scope was the blind spot) | none yet — one of three sub-shapes is mechanizable |
| IC-12 | transient shared state lies to every reader | `transient-shared-state-lies-to-readers` | not yet — n=2, and the remedy so far is knowledge rather than mechanism | none yet |
| IC-13 | a capped result is presented as complete | `capped-result-presented-as-complete` | clears both bars — **spread re-derived 2026-09-01 over the 9 then: 5 coarse / 7 fine** (was 6 / 11 over the pre-ruling 16); **not re-derived over the 12** | none yet — **clause widened + 7 non-members moved out 2026-09-01** to IC-19/20/21/22; claim was true of all 9 then, and the three 2026-09-02 additions were judged against it at filing |
| IC-14 | a guard's coverage is narrower than its name | `guard-narrower-than-its-name` | clears count; **spread adjudicated 2026-09-01 over the 11 then — 4 subsystems / 6 distinct guards**; **not re-adjudicated over the 12** | none yet — one sub-shape of three is mechanizable |
| IC-15 | a parameter is accepted then silently dropped | `accepted-parameter-silently-dropped` | clears count; **spread adjudicated 2026-09-01 — 6 subsystems** | **partial** — probe at 5 of 8 sites 2026-09-02; shared half extracted AND un-feature-gated |
| IC-16 | an assertion that cannot fail | `assertion-that-cannot-fail` | **clears both bars 2026-09-01**; rule already in `CLAUDE.md` — the third instance buys measurability, not a rule | designed; positive-form guard owed |
| IC-17 | a shared resource carries no owner, so enumerating the peer does not help | `shared-resource-carries-no-owner` | `OB-8` (+ OB-2) — 2026-09-01 | partial |
| IC-18 | a selector is narrower than the population it names | `selector-narrower-than-its-population` | clears both bars 2026-09-01 — 6 subsystems; remedy already Accepted as ADR-2026-08-27 for the tool-facing half | **partial** — nothing reaches an author-written selector |
| IC-19 | a truncated window is ordered by a key unrelated to why it was requested | `truncated-window-ordered-by-the-wrong-key` | **clears the count bar on creation** — 4 subsystems as of 2026-09-02; spread and `OB` routing still unadjudicated | none yet |
| IC-20 | a floor is published under the name of a total | `floor-published-under-the-name-of-a-total` | not yet — n=1; kept apart from `IC-19` on the remedy test (rename vs re-select) | none yet |
| IC-21 | an instrument reports presence or a count where the decision turns on magnitude | `instrument-omits-the-dimension-that-grows` | not yet — n=2, one short; already 2 subsystems, so instance 3 meets both bars | none yet |
| IC-22 | a next-step hint is composed from the response shape, not from the request | `hint-composed-without-the-request` | not yet — n=4, **count bar cleared 2026-09-02** at `n=3`, judgement owed; seed **fixed** `bb4688fd`, second member open on the *preview* surface | none yet |

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

**A minimal Slug/Members pair exists for `cluster/unclassified` itself, precisely so the escape
hatch does not fall outside its own growth-documentation gate below.** No count, no promotion
field — that would reopen exactly the gate the sentence above disclaims — but a new member still
needs `**Members:**` to name it, the same as any IC-N slug, or the tag is untaggable the moment
CHECK 2 (`scripts/pre-commit-ledger-counts.py`) runs against it.

> ✅ **ADJUDICATED 2026-09-03 — the hatch STANDS, and the archive campaign does not follow from
> it.** The flag below bundles two questions, and the bundling is why neither was decided: the
> hatch is cheap and evidence-backed, the campaign is expensive and has no evidence behind it yet.
> Deciding the first does not license the second, and they are separated here.
>
> **The hatch exists — as a staging area with an exit, never a terminus.** Three reasons, in
> order of weight:
>
> 1. **A forced tag is strictly worse than a parked one.** `n` is a partition and counts drive
>    promotion, so a bug pushed into a class it does not instantiate moves that class toward its
>    threshold on a false member. Parking costs a class nothing; a wrong tag costs every reader
>    of that class's count.
> 2. **The absence has already corrupted a promotion, and this ledger says so in its own words.**
>    `IC-11` was promoted *"forced by a taggable instance arriving against a gate with no
>    `cluster/unclassified` escape hatch, rather than by its count"*. That is the cost of not
>    having it, already paid — a class promoted on the wrong grounds because an author needed
>    somewhere to put a bug.
> 3. **Both members did the work the hatch is supposed to require.** Neither is a shrug. Each
>    names the classes it checked and why each fails, and `chunk-line-ranges-…` names the
>    candidate class for a second instance. Two independent authors reached for the same missing
>    token and both left a derivation behind rather than forcing a fit.
>
> **Admission conditions, which both current members already meet.** The tag requires (a) the
> classes actually checked, named, with why each fails, and (b) a **named candidate class** for
> the second instance. (b) is what makes this a queue rather than a parking lot: without it the
> tag is terminal and nothing ever leaves. With it, the hatch feeds `IC-N` creation — which is
> exactly what the candidate queue below did before it emptied.
>
> **The `IC-6` reading is accepted, and it argues FOR the hatch.** `e5b1c28f` was right that a
> closed set with no escape is `addressing-without-an-escape-hatch` holding about its own
> classifier. The remedy this ledger prescribes for `IC-6` is *provide the escape* — so the
> finding's own class supplies the verdict.
>
> **NOT decided: the archive-coverage campaign.** § above notes every `n` is a floor and that
> covering the archive would need this slug. It now exists, and that still does not authorise
> tagging the archive. `CLAUDE.md` § *Observer Blindness* is explicit that **a coverage ratio
> which is neither ~0% nor ~100% is a boundary someone drew before it is drift**, and that a
> campaign over a population must first grep `tests/`, `scripts/pre-commit-*` and hooks for that
> population's name — the exact sequence that once came one step from a 236-file campaign the
> gate's own header forbade in writing. Whoever wants the archive covered opens that as its own
> decision with its own evidence. **The floors stay floors — now by choice rather than by
> omission, which is the whole of what this adjudication changes for them.**
>
> *Adjudicated by session `63083c9e-cc56-4dbd-9852-820f34261eeb` on the operator's direction.
> Reversible: the remedy named below — a real slug on each member and removal of the pair —
> stays available, and costs two retags.*
>
> **The original flag, kept whole because a withdrawn claim is worth more on the record than off
> it:**
>
> ⚠ **NOT ADJUDICATED — created 2026-09-02 by a fix-round implementer, and the word "sanctioned"
> below is withdrawn.** The sentence above this block calls the hatch *"a taxonomy decision, not a
> gate one"*, and no such decision has been taken. What happened is narrower and worth stating
> exactly: an implementer filing a bug that fit no existing class added the Slug/Members pair so
> the tag would satisfy CHECK 2, then described the result as sanctioned. The mechanism works and
> nothing is blocked by it — `cargo test --test issue_clusters` is 19/19 — but a passing gate is
> not an adjudication, and this ledger's closed set is the one thing a fix round should not widen
> on its own authority.
>
> **The case for the hatch is now two instances strong**, which is an argument for deciding it
> rather than for having decided it: `IC-11` was promoted early *"forced by a taggable instance
> arriving against a gate with no `cluster/unclassified` escape hatch, rather than by its count"*
> (below), and this is the second author to reach for the same missing token. A peer session
> (`e5b1c28f-0f61-4680-add7-d76980bc8a6f`) named it as `IC-6` — an addressing scheme with no escape
> — holding about the classifier that indexes `IC-6`. **Whoever adjudicates owns both directions:**
> if the hatch should exist, it needs the taxonomy decision the sentence above defers, and the
> archive-coverage consequence that sentence names; if it should not, the remedy is a real slug on
> the one member below and the removal of this pair.
>
> Reported by `e5b1c28f` against a tree where the pair was absent and the member present, so the
> gate red for them and green here — the transient split-brain a shared checkout produces while a
> two-file change is landing, not a disagreement about the rule.

**Slug:** `cluster/unclassified`

**Members:** `a-corrected-ceiling-reds-within-minutes-on-a-shared-checkout` (2026-09-02), `chunk-line-ranges-are-body-relative-but-published-as-file-lines` (2026-09-02); +1: `doc-tool-refs-counts-call-param-pairs-as-documents` (2026-09-02); +1: `markdown-grammar-librarian-guard-has-zero-test-coverage` (2026-09-03); +1: `worktree-guard-word-boundary-blocks-read-only-git-plumbing` (2026-09-03) — filed HERE rather than forced into `IC-14`, and the reason is that `IC-14` is its exact inverse. `git-worktree-guard.mjs:65` ends each destructive verb with `\b`, and a hyphen is a word boundary, so `merge\b` matches `git merge-base` and `commit\b` matches `git commit-tree`: six read-only plumbing commands are refused as destructive mutations. `IC-14` is a guard whose coverage is NARROWER than its name — this one's is WIDER, refusing work rather than missing it, so tagging it there would corrupt the count that class's promotion reads. Worth recording for whoever meets the second instance: codescout's own IL-3 refusal text names `merge-base` in its list of "single-line plumbing … always bounded", so two guards in one process classify the same string in opposite directions. The candidate class, once a second instance exists, is *"a guard's trigger matches a superset of what its name claims"*. Note also what the guard's PRIOR hardening pass could not have caught: three earlier fixes all concerned WHICH TEXT the verbs are matched against (heredoc stripping, per-segment splitting, forward-only `cd`), and none concerned what the verb pattern itself matches. All derivations below — **on continuation lines, which `scripts/pre-commit-ledger-counts.py` does not read.** Its `members_fields` keys on the single line beginning `**Members:**`, so a member named only in the prose beneath satisfies nothing; put the stem up here and the reasoning down there. +1: `a-reverted-ledger-burns-an-entry-id-with-no-warning` (2026-09-03).

`markdown-grammar-librarian-guard-has-zero-test-coverage` — the guard on `edit_file`'s
markdown-grammar write path can be **deleted outright** with zero failures in either lane,
while the identical deletion at the raw-text call site reds immediately. Looked at `IC-16`
(`assertion-that-cannot-fail`) first and it is the closest: this is coverage that is absent
rather than vacuous, and `IC-16` claims an assertion **exists** with no failing input. There
is no assertion here at all, so the class's own shape does not hold. `IC-14`
(`guard-narrower-than-its-name`) fails for the opposite reason to the entry above: the guard
itself is correctly scoped and fires in production — what is missing is a test, so nothing
about the guard's *coverage* is narrow. `IC-3` (`declared-not-wired`) was the third look and
is the nearest miss: the guard IS wired, it is the **test suite** that never reaches its
`Err` branch. Filed under the hatch rather than stretching `IC-3` from unreached-code to
unreached-branch, which would blur the one distinction that makes it diagnosable.

`a-corrected-ceiling-reds-within-minutes-on-a-shared-checkout` —
looked at IC-5 (`repro-env-diverges-from-gate-env`), IC-12
(`transient-shared-state-lies-to-readers`) and IC-20 (`floor-published-under-the-name-of-a-total`);
none fit without forcing the claim, so filed under the escape hatch instead — which was created
by the same fix round, and was **adjudicated 2026-09-03** in the block above (the hatch stands;
the archive campaign it would enable does not follow from it).
+1: `chunk-line-ranges-are-body-relative-but-published-as-file-lines` (2026-09-02) — a value
computed in one coordinate space (lines within the frontmatter-stripped **body**) published
under a name that states another (`start_line`, read by every consumer as a **file** line).
Checked three: `IC-20` (`floor-published-under-the-name-of-a-total`) is closest by shape and
still does not fit — there the true value is **unknowable** because the walk stopped, and the
remedy is to rename the quantity or refuse to print it; here it is exactly recoverable by
adding a constant the response simply omits, so neither the claim nor the remedy transfers.
`IC-11` (`doc-contradicted-by-code`) fails because the code and its docs **agree** — both say
"start line", and both are wrong about the same thing. `IC-13`
(`capped-result-presented-as-complete`) fails because nothing is truncated. Filed under the
escape hatch rather than forced into `IC-20`. **If a second instance appears, the candidate
class is "a quantity is published in a coordinate space its name does not state"** — and the
blind party would be the producer, for whom the two spaces are the same number in every test
that seeds the body and compares against the body. *(Took three refused commits to land, and
the reason is worth one line: `members_fields` in `scripts/pre-commit-ledger-counts.py:367`
does `out[cur] = line` — the field is the SINGLE line starting `**Members:**`, not the
paragraph. A derivation appended below it satisfies neither of the gate's two conditions, so
the refusal reads as "you did not write a derivation" while the derivation sits four lines
below it. Put the slug on the line; keep the reasoning under it.)*

`doc-tool-refs-counts-call-param-pairs-as-documents` — read all 22 `**Claim:**`
lines before reaching for the hatch. `IC-20` is the near miss and **excludes it by name**: its
falsification clause reads *"a member whose true total was recoverable — that is an ordinary
reporting bug"*, and this count is recoverable by deduplicating on `(file, line, tool)`. `IC-14`
was the second candidate and fails on coverage: the guard misses no call, it **over**-reports one,
where that class requires an uncovered remainder. `IC-21` is magnitude-versus-count; this is
unit-versus-label. **The reason to record the near miss rather than just the verdict:** `IC-14`'s
own member 3 (`both-doc-citation-guards-skip-half-the-corpus-without-saying-so`) is a *different*
defect in this *same file*, so a retag into `IC-14` would have read as corroborated by adjacency —
which is the proximity-is-not-evidence failure `IC-10` names, arriving here as a taxonomy pressure
rather than an authorship one.

**The candidate queue is now empty — all five became classes on 2026-09-01, and every one opened
at n=0.** `IC-13`, `IC-14` and `IC-15` are the backfill's three remaining shapes; `IC-12` is the
read-side window the git hooks introduced; `IC-16` is the vacuous-assertion family the `IC-9`
withdrawal exposed. The fourth backfill shape needed no entry — it had already been promoted to
`IC-11` on 2026-08-31, forced by a taggable instance arriving against a gate with no
`cluster/unclassified` escape hatch, rather than by its count.

**Four of the five have since been tagged; `IC-12` alone still reported zero — run `python3
scripts/probe-cluster-census.py`, not this sentence.** *(Redirect corrected 2026-09-02. It read
"read the `n` column" until that column was removed the same day, so the remedy written for a
stale claim outlived the surface it pointed at — and the census now puts `IC-12` at 2, not zero.
`OB-12` exactly: the removing commit produced no diff hunk here, no broken link and no check, so
nothing pointed backwards. Corrected in place rather than silently rewritten, because the
paragraph below is about this failure mode and had quietly acquired a second instance of it.)* All five *opened* at n=0, and this paragraph asserted that in the **present
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
## Template for new entries

```
## IC-N — <the class, stated as a claim>

**Slug:** `cluster/<slug>`
**Claim:** <the mechanism, in mechanism-language>
**Members:** `filter={"tags": {"contains": "cluster/<slug>"}}` — <why this instance belongs to this class. Never a bare count: derive one with `scripts/probe-cluster-census.py`, which `no_class_field_states_a_bare_n` cannot enforce on this template because a placeholder has no integer to match>
**Blind party:** <who structurally cannot see it, and why> | `none — ordinary design defect`
**Promotes to:** `not yet` | <target ledger + id>
**Mechanism status:** none yet | designed | shipped (<what>)
**Valid:** dated <YYYY-MM-DD>

<Two or three paragraphs: what the instances share, what a fix would have to change, and
what would falsify the claim.>
```
