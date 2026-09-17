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
entry_high_water_IC: 23
---

> **Prefix:** `IC-N` — one **defect class** the bug corpus instantiates. Declared ledger; the
> `IC` namespace is project-wide (`docs/TAXONOMY.md`).

> **THIS FILE IS THE ROSTER. The definitions are not here.** Each class's `**Slug:**` and
> `**Members:**` live in its own file under `docs/trackers/issue-clusters/`, one per class. This
> file lists every slug in the table below — which is what you want when **choosing** a tag, and
> the wrong file when **authoring** a `+1:` derivation.
>
> So `grep 'cluster/<your-slug>'` against this file returns **0**, and that zero means *wrong
> file*, not *no such class*. A generic `cluster/` grep here returns dozens — `cluster/unclassified`
> is the one real class whose field genuinely is in this file — which is what makes the wrong file
> look like the right one. Two sessions paid two refusals each for this in one morning, before the
> growth gate's refusal named the path;
> `docs/issues/archive/2026-09-08-the-cluster-refusal-names-a-field-not-a-file-and-the-index-confirms-it.md`
> carries the measurements.

## What this ledger is for

`docs/issues/` answers *what is broken*. `docs/trackers/open-issue-work-queue.md` (`BL-N`)
answers *what to pick up next*. Neither answers **what these bugs have in common** — and that
is the question an architectural problem is visible in.

Each entry here is a **class**, stated as a claim that could be false. Membership is carried by
a reserved `cluster/<slug>` tag in each bug file's frontmatter, so a class's instances are a
query rather than a list:

```
doc(action="find", kind="bug",
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
| `**Members:**` | the query, plus a per-member derivation. **Never a bare `n=`** — counts are derived (`scripts/probe-cluster-census.py`), and `no_class_field_states_a_bare_n` refuses one here. *Bare* means **outside backticks**; read the two paragraphs under this table before reaching for a backtick |
| `**Blind party:**` | who structurally cannot see it, and why — or `none — ordinary design defect` |
| `**Promotes to:**` | target surface, per the routing table below |
| `**Mechanism status:**` | `none yet` \| `designed` \| `shipped (<what>)` — borrowed from `OB` |
| `**Valid:**` | `invariant` \| `dated YYYY-MM-DD` \| `conditional — <event>` |

**What *bare* means, and the limitation it carries.** A `n=` counts as a claim only **outside** a
complete backtick pair. A backticked `` `n=27` `` is read as a QUOTATION — a superseded figure kept
with its derivation — and is deliberately not checked, which is what let the 2026-09-02 migration
wrap every live claim rather than delete it, so no sentence lost its history. **But the escape
cannot separate a quotation from a live claim: the two are byte-identical.** A current count
written in backticks therefore passes the gate and then decays exactly like the stored count the
gate was inverted to remove, with nothing to report it. This is said *here* because the refusal
text — which does spell the escape out — only ever reaches someone who already wrote a **bare**
one. The author who reaches for a backtick unprompted never sees it.

**A commit-path check for this was designed, measured and REFUSED. Do not rebuild it.** Refusing a
backticked `n=` that appears on a gated line and was not there at HEAD would have blocked 11
commits in 11 days, and of the 14 tokens involved roughly 8 were legitimate quotations or mentions
— *including the commit that filed the bug*, whose derivation quotes the token twice in order to
explain that the two readings are byte-identical. A parser keyed on the undiscriminated token
inherits the blindness it was built to remove, which is `IC-6` holding about the gate built to fix
it. The replay, and the one discriminator that did survive, are in
`docs/issues/archive/2026-09-02-the-no-stored-count-gate-is-defeated-by-house-style.md`.

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
doc(action="find", kind="bug", filter={"tags": {"contains": "cluster/<slug>"}})

# the same class, actionable only
doc(action="find", kind="bug", filter={"and": [
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
> **What the row keeps is what no query derives:** the promotion verdict and the subsystem spread.
> Those are adjudications. The row used to mix a derived counter with a
> human verdict, which is precisely why bumping the counter forced a write to the verdict's file.
>
> **The `mechanism` column was deleted 2026-09-13, and its reason is the `n` column's reason with
> one word changed.** Mechanism status is not *derived*, so the argument above never reached it —
> but it was *duplicated*, and duplication alone is enough. The authoritative copy is each class
> file's `**Mechanism status:**` field; the cell was a second one, sitting in the file 23 classes
> share. Measured at deletion — comparing the **leading verdict word** of each cell against its
> field's, parentheticals ignored, against the worktree at `4f268eb1` — **2 of 23 disagreed**:
> `IC-13` read `none yet` against a detector that had shipped 2026-09-03 as `tests/result_caps.rs`,
> and `IC-3`'s cell led with no verdict word at all. Neither was catchable, because **no parser on
> either side ever read that cell** — `parse_index_rows` takes the slug and stops, and
> `mechanism_statuses` scans field lines. `IC-4`'s own field had already recorded this exact drift
> on 2026-09-02 and left it named rather than fixed; it recurred. Read mechanism status from the
> class file, or from `python3 scripts/probe-cluster-census.py`, which renders it beside the
> verdict.
>
> **Two cells carried a sentence the class file did not, and both were migrated before deletion**
> rather than lost — `IC-11`'s *registry-keyed check over everything delivered to a model* and
> `IC-21`'s *per-site only*. A column delete done as a regex would have dropped both silently,
> which is the cost this note exists to record: the duplicate was not a pure copy, and finding
> that out is an audit, not a `sed`.
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

| id | class | slug | promotes to |
|---|---|---|---|
| IC-1 | the blast radius of a write is wider than the set of peers you can see | `blast-radius-exceeds-visibility` | `OB-3` — 2026-09-01 |
| IC-2 | a gate keyed on an event it cannot observe substitutes a proxy | `gate-keyed-on-unobservable-event` | `OB-6` — promoted 2026-09-01 |
| IC-3 | declaration is not execution | `declared-not-wired` | `OB-7` — promoted 2026-09-01 |
| IC-4 | config propagation is additive | `config-propagation-is-additive` | `OB` — passes admission test; hook owed |
| IC-5 | the reproduction environment is not the gating environment | `repro-env-diverges-from-gate-env` | `H` — seven subsystems **adjudicated over 12 members, not re-adjudicated since**; the *"mechanism owed"* clause is **withdrawn 2026-09-04** as stale |
| IC-6 | an addressing scheme with no escape hatch | `addressing-without-an-escape-hatch` | `CLAUDE.md` § Parsers Over a Namespace — **landed** |
| IC-7 | lazy warm-up bills the first caller | `lazy-warmup-bills-the-first-caller` | not yet — 2 of 4 unconfirmed |
| IC-8 | a record asserts a completed action nothing re-checked | `record-asserts-an-unchecked-completion` | `DC` |
| IC-9 | an assertion over environment-controlled text is satisfiable by accident | `assertion-satisfiable-by-accident` | not yet — two tags withdrawn as misfits |
| IC-10 | authorship on a shared checkout is unrecoverable after the fact | `authorship-unrecoverable-after-the-fact` | **clears both bars 2026-09-01** — n=3, spread 3, via second-read retag |
| IC-11 | documentation denies a capability the code has since gained | `doc-contradicted-by-code` | **CLEARS BOTH BARS — spread RE-ADJUDICATED 2026-09-04 over the whole membership: 9 doc surfaces**, superseding the four per-member deltas that ran the 2026-09-01 reading from 4 to 7. Two of the nine are new *in kind*: a **different repository** (companion-plugin hooks) and **`prompt:` YAML delivered as live instructions**, both outside the boundary of every gate this repo owns |
| IC-12 | transient shared state lies to every reader | `transient-shared-state-lies-to-readers` | not yet — n=2, and the remedy so far is knowledge rather than mechanism |
| IC-13 | a capped result is presented as complete | `capped-result-presented-as-complete` | clears both bars — **spread re-derived 2026-09-01 over the 9 then: 5 coarse / 7 fine** (was 6 / 11 over the pre-ruling 16); **not re-derived over the 12** |
| IC-14 | a guard's coverage is narrower than its name | `guard-narrower-than-its-name` | **CLEARS BOTH BARS — spread RE-ADJUDICATED 2026-09-04 over the whole membership: 16 distinct guards** (was 6 over the 11 then), 14 cited code subsystems as a corroborating floor |
| IC-15 | a parameter is accepted then silently dropped | `accepted-parameter-silently-dropped` | clears count; **spread adjudicated 2026-09-01 — 6 subsystems** |
| IC-16 | an assertion that cannot fail | `assertion-that-cannot-fail` | **clears both bars 2026-09-01**; rule already in `CLAUDE.md` — the third instance buys measurability, not a rule |
| IC-17 | a shared resource carries no owner, so enumerating the peer does not help | `shared-resource-carries-no-owner` | `OB-8` (+ OB-2) — 2026-09-01 |
| IC-18 | a selector is narrower than the population it names | `selector-narrower-than-its-population` | clears both bars 2026-09-01 — 6 subsystems; remedy already Accepted as ADR-2026-08-27 for the tool-facing half |
| IC-19 | a truncated window is ordered by a key unrelated to why it was requested | `truncated-window-ordered-by-the-wrong-key` | **clears the count bar on creation** — 4 subsystems as of 2026-09-02; spread and `OB` routing still unadjudicated |
| IC-20 | a floor is published under the name of a total | `floor-published-under-the-name-of-a-total` | not yet — `n=1`; kept apart from `IC-19` on the remedy test (rename vs re-select) |
| IC-21 | an instrument reports presence or a count where the decision turns on magnitude | `instrument-omits-the-dimension-that-grows` | not yet — `n=2`, one short; already 2 subsystems, so instance 3 meets both bars |
| IC-22 | a next-step hint **or a causal explanation** is composed from the response shape, not from the request | `hint-composed-without-the-request` | not yet — **count bar cleared 2026-09-02** at three, judgement owed; derive the live figure with `python3 scripts/probe-cluster-census.py` rather than reading one here. This cell quoted `n=4` until 2026-09-14, by which point the probe returned **16** — the decay is left named because the +2 below was written beside the stale figure and nearly inherited it. Seed **fixed** `bb4688fd`, second member open on the *preview* surface. **+2 by retag 2026-09-14, and the retag is a finding about this column.** It said *hint* alone until then, while the membership had held causal explanations since 2026-09-05 — so two bugs sat in `cluster/unclassified` under a proposed NEW class because their authors read this line, and the claim line, and correctly concluded neither covered them. Heading and slug deliberately unchanged: the heading is cited as a live `doc(get, heading=…)` example in `docs/issues/archive/2026-09-01-a-scoped-read-is-billed-the-full-heading-map.md`, and the slug keys the tags of every member |
| IC-23 | a per-item attribute is derived at the container's granularity, and is correct for the first item | `attribute-derived-at-container-granularity` | not yet — one member, one subsystem; opened 2026-09-13 by **retag out of `IC-6`** after the founding bug's diagnosis was falsified |
| IC-24 | a value correct in one frame is published under a name that states another | `value-correct-in-a-frame-its-name-does-not-state` | not yet — opened 2026-09-16 by **retag out of `cluster/unclassified`**, whose census fell 33 → 30 in the same change; count bar cleared on creation, **spread deliberately NOT adjudicated** — three of the four axes sit in librarian-adjacent surfaces |

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

**The candidate queue refilled 2026-09-16 — `IC-24`, and the evidence for it had been in this
file three times over.** Three `cluster/unclassified` members each close with a *"candidate class
if a second appears"* note naming a **narrower** version of one shape:
`chunk-line-ranges-are-body-relative-but-published-as-file-lines` proposed *a quantity published
in a coordinate space its name does not state*; `doc-tool-refs-counts-call-param-pairs-as-documents`
called itself *unit-versus-label*;
`a-move-reports-slug-and-path-citations-under-one-field-named-for-paths` proposed *a widened
detector publishes its new population under the old one's name*. Coordinate space, unit, form —
three authors, three axes, each writing *"if a second appears"* while two others already had.
**Nobody was wrong about their own member; the generalisation is the FRAME itself**, and
`embedder-stack-ops-session-log:F-7` supplies a fourth axis — the scope word — with three
instances of its own: a sha correct for a commit but not for *my push range*, a CI tally correct
for one run but not for *the range*, one author correct for eight of nine staged paths.

**What carves it from `IC-20`, which all three members checked and rejected, is `IC-20`'s own
falsification clause** — *"a member whose true total was recoverable — that is an ordinary
reporting bug"*. There the value is **unknowable** because a walk stopped, and the remedy is to
rename the quantity or refuse to print it. Here every value is exactly right and exactly
recoverable; only the binding between value and name is wrong, so the remedy inverts — state the
frame beside a value that is already correct. That clause is why three members landed in the
hatch rather than being forced into `IC-20`, which makes the hatch's own contents the derivation
for this class rather than merely its backlog.

**Blind party: the producer, for whom the two frames coincide in every check they would think to
run.** That is what makes *"verify before asserting"* a no-op here — re-verification returns the
same correct value, and agreement is precisely what a correct answer to a neighbouring question
produces. The body-relative test seeds the body and compares against the body; a session reading
`who WROTE this path` off `file-provenance.py` and publishing it as `who STAGED it` gets a true
sentence from a tool whose output says *"written by"*. Every instrument aimed at the value
confirms it; no instrument aimed at the value can read the name.

**Detector, on all four axes: a party holding the OTHER end of the binding.** The session whose
commit it was knew which of theirs was outstanding; the session watching all three CI runs knew
what the other two cost; the session that had staged the ninth path knew it was not the eighth
author's. No detection was available to a more careful re-measurement on the publishing side, and
all arrived as corrections rather than as alarms — the `OB` admission test met on the nose.

**Retag of the three members is owed and is deliberately not in this commit.** Each is a bug-file
frontmatter write through the catalog (a direct edit does not reach it — BL-48) *plus* the single
`**Members:**` line below, and `scripts/pre-commit-ledger-counts.py` checks the two agree — so it
is one coupled change, not four independent ones, and half of it landing is a red tree for every
other session.

**Slug:** `cluster/unclassified`

**Members:** +1: `a-backgrounded-gate-command-was-summarised-as-exit-0-while-its-buffer-held-the-failure` (2026-09-14) — a `run_command` summary asserted `✓ exit 0` for a `cargo clippy` that had failed to compile, and the caller acted on the summary; unreproduced across three controlled variants. Filed here rather than forced into a named class: the full derivation, including why `IC-13`, `IC-22` and `IC-2` were each weighed and rejected and what a second instance would be called, is the paragraph beginning `+1:` further down this field. `classify-conflates-two-malformed-reasons-under-one-message` (2026-09-03) — **MOVED OUT 2026-09-14 to `cluster/hint-composed-without-the-request` (IC-22)**, where the derivation now lives. It was filed here after `IC-16` and `IC-14` were weighed and rejected; `IC-22` was never weighed, because its claim line said *next-step hint* while its membership had already extended to causal explanations. Looked at `IC-16` (`assertion-that-cannot-fail`, wrong shape — the gate still reds, it just misdirects) and `IC-14` (`guard-narrower-than-its-name`, also wrong — the guard's coverage is right, only its message is ambiguous); neither fits without forcing the claim, so filed under the escape hatch instead. +1: `a-corrected-ceiling-reds-within-minutes-on-a-shared-checkout` (2026-09-02); +1: `markdown-grammar-librarian-guard-has-zero-test-coverage` (2026-09-03); +1: `worktree-guard-word-boundary-blocks-read-only-git-plumbing` (2026-09-03) — filed HERE rather than forced into `IC-14`, and the reason is that `IC-14` is its exact inverse. `git-worktree-guard.mjs:65` ends each destructive verb with `\b`, and a hyphen is a word boundary, so `merge\b` matches `git merge-base` and `commit\b` matches `git commit-tree`: six read-only plumbing commands are refused as destructive mutations. `IC-14` is a guard whose coverage is NARROWER than its name — this one's is WIDER, refusing work rather than missing it, so tagging it there would corrupt the count that class's promotion reads. Worth recording for whoever meets the second instance: codescout's own IL-3 refusal text names `merge-base` in its list of "single-line plumbing … always bounded", so two guards in one process classify the same string in opposite directions. The candidate class, once a second instance exists, is *"a guard's trigger matches a superset of what its name claims"*. Note also what the guard's PRIOR hardening pass could not have caught: three earlier fixes all concerned WHICH TEXT the verbs are matched against (heredoc stripping, per-segment splitting, forward-only `cd`), and none concerned what the verb pattern itself matches. All derivations below — **on continuation lines, which `scripts/pre-commit-ledger-counts.py` does not read.** Its `members_fields` keys on the single line beginning `**Members:**`, so a member named only in the prose beneath satisfies nothing; put the stem up here and the reasoning down there. +1: `a-reverted-ledger-burns-an-entry-id-with-no-warning` (2026-09-03); +1: `doc-overflow-never-routes-to-progressive-disclosure` (2026-09-04); +1: `patch-must-be-a-json-object-refusal-is-unreproduced` (2026-09-04); +1: `doc-update-body-appends-a-trailing-blank-line-every-write` (2026-09-05) — looked, and nothing in the closed set fits. The shape is a write path that is not a fixed point: `write(read(x)) != x`, adding one trailing newline per round-trip, so it compounds only for artifacts edited through the sanctioned whole-body route and never for a first write. Not `config-propagation-is-additive`, whose claim is about updates landing while removals and renames do not; not an instrument, guard, selector or assertion class, because nothing here is measured or gated. Recorded as unclassified rather than forced into the nearest-sounding name, per this class's own rule that the taxonomy decision is the point. +1: `supersedes-link-moves-catalog-status-without-writing-the-file` (2026-09-06) — `doc(action="link", rel="supersedes")` sets `dst.status` in the **catalog only**; the destination file's frontmatter keeps its old value, nothing is written to disk, and `git status` reports the tree clean. Observed live: catalog `superseded`, file `open`, tree clean — three readers, two answers, no error anywhere. Nothing in the closed set fits, and the two near misses miss in *different* directions: `IC-11` (`doc-contradicted-by-code`) is prose disagreeing with code, where here both halves are machine state and no doc is involved; `IC-12` (`transient-shared-state-lies-to-readers`) requires a window that closes, and this divergence is **permanent** — it survives a commit precisely because there is nothing to commit. **Candidate class, named here rather than promoted:** *a write path updates one half of the file/catalog pair and reports success.* Three instances span two directions and two fields — status via `link` (this one, catalog→file), status via `edit_markdown` (`docs/issues/archive/2026-08-29-edit-markdown-frontmatter-desyncs-catalog-status.md`, file→catalog), and id via a worktree-minted move (`docs/issues/2026-09-05-frontmatter-id-mismatch-asserts-a-move-for-worktree-minted-ids.md`). **Not** promoted on that count: the third is arguably a different mechanism, and the promotion rule wants ≥2 subsystems where all three sit inside the librarian. **What a fourth instance should check first:** whether `doctor` has grown a `frontmatter_status_mismatch` companion to its two `frontmatter_id_*` checks. Until it does, this shape's corpus-wide population is not merely unfixed but **unmeasured**, and any count cited for it will have been hand-derived from an instrument that does not exist. **+1: `a-push-publishes-commits-their-author-was-withholding` (2026-09-06) — and it lands here after BOTH near misses were rejected on their own claim text, which is the use this escape hatch is for.** `git push` sends every commit on the branch rather than the pusher's own, so one session's push publishes work on behalf of every session that has committed — including one deliberately withholding pending its operator's authorisation. Measured the day it was filed: a session pushed its own commit and carried two others whose author had told its operator it would commit and not push. **`IC-1` was rejected** because its claim is that a session *cannot know its own blast radius*, and here the blast radius was known exactly — two commits, named, with authors, read via `git log origin/<branch>..HEAD --stat` before pushing. What was unknowable was the action's **permissibility**, not its extent. **`IC-17` was rejected** because its claim is that a shared resource *records what changed and never who*: git records who, accurately, and the owner field that class asks for is present and populated. `IC-17`'s **remedy** does fit — isolate the resource — which is a reason to watch the pair rather than merge them, since classifying by remedy instead of by claim is the error this ledger warns about two sections up. **The candidate claim, if a second instance arrives:** *authorisation is not recoverable from a shared artifact, and unlike authorship it cannot be recovered by asking either, because the party who would have to be asked is invisible in the artifact.* Authorship was proven recoverable three times on this checkout the same evening, each time by a session quoting its own sessionId from its scratchpad path; authorisation lives in a conversation with an operator no peer can see or query. **What makes it worth a row rather than a mention: the diligent path and the negligent path have the same outcome.** Reading what you carry is the right check and is structurally incapable of catching this, and a push freeze — the obvious remedy — is worse than useless here, because it coordinates *timing* between parties who all intend to publish eventually, and this is a party who intends not to. +1: `no-doctor-check-measures-frontmatter-status-divergence` (2026-09-07) — the hatch used for its stated purpose, *looked, nothing fits*, and specifically because the defect is the **absence of an instrument** rather than a fault inside one: `doctor` carries `frontmatter_id_mismatch` and no `frontmatter_status_mismatch`, so file-vs-catalog `status` divergence has never been countable at all. Checked `IC-11` (`doc-contradicted-by-code` — nothing is contradicted here; no document claims the check exists, and one that omitted it would be accurate) and `IC-16` (`assertion-that-cannot-fail` — there is no assertion, which is the point, and a vacuous check differs from an absent one in that only the first can be repaired where it stands). Closest and still wrong is the parent bug's own candidate class, *a write path updates one half of the file/catalog pair and reports success*: this is not a write path, updates nothing and reports nothing — it is the missing instrument that would have **counted** that class's three known members across two directions and two fields, each of which was instead found by a human noticing an oddity. Tagging it there would enrol a class's own measurement gap as one of its instances, inflating the count that decides promotion with the very thing that cannot be an instance of it. `cluster/unclassified` drives no promotion threshold, so parking costs the taxonomy nothing. **Derivation completed 2026-09-07** after `89d91024` raised `IC-18` — the original named only `IC-11` and `IC-16`, which was a real gap in a field whose entire purpose is *looked, nothing fits*. Two more checked, and they are near misses in **opposite** directions, which is why this stays parked rather than moving. `IC-18` (`selector-narrower-than-its-population`) needs a selector that **runs** and returns a well-formed zero reading as *not present*; here nothing ran at all — `doctor` emitted no such key, so the absence was legible to anyone reading the check list rather than disguised as a count, and the harm was that the question could not be **asked**, not that it was answered narrowly. `IC-14` (`guard-narrower-than-its-name`) is the closer of the two, since `doctor` is documented as a catalog-drift scanner while covering only the `id` half of the file/catalog pair — but that naming defect is **already filed on its own** (`every doc surface describes doctor as a catalog-drift scanner, hiding ~17 of its 23 checks`), so tagging this file there would count one naming defect twice and move `IC-14` toward its threshold on a member that is really the other file. Both readings are defensible, neither is clean, and the ledger's own rule decides it: a forced tag costs every reader of that class's count, a parked one costs nothing. **+1: `staleness-reason-says-changed-but-the-count-includes-deleted`** — `workspace(status)` renders `"N of M anchored files changed"` where N counts changed **plus deleted** (observed live: `architecture` read "17 of 23" against 15 changed + 2 deleted). `IC-20` (`floor-published-under-the-name-of-a-total`) is the near miss and fails in two ways: its claim turns on the true value being **unknowable because a walk stopped**, whereas this response carries `changed_files` and `deleted_files` as separate arrays so the decomposition is right there; and the direction is inverted — `IC-20` is a subset published under the whole's name, this is the whole published under a subset's. The remedies rhyme (both are renames) and the diseases differ. **+1: `memory-concept-page-omits-staleness-and-anchors-entirely`** — the memory concept page's workflow never mentions staleness, anchors, or the review-before-refresh rule, so the correct procedure exists in exactly one file. `IC-11` (`doc-contradicted-by-code`) is the obvious reach and is wrong: nothing is contradicted, the page is **silent**, and the two have different remedies — a contradiction is repaired by correcting a sentence, an omission by first deciding whether the sentence belongs on that page at all. Counts not re-derived for these two +1s. **+1: `a-fixed-byte-window-source-guard-reports-a-correct-call-site`** (2026-09-09) — `every_resolve_scope_call_names_project_as_its_default` decides whether a `resolve_scope` call names `Scope::Project` as its default by testing whether a **fixed 240-byte window** after the call token contains that string. Both directions fail and both were observed: a correct call site with a three-line comment in its argument list is reported as an offender (the literal sits at offset 400), and a call site passing `Scope::Repo` **passes** whenever any text inside the window merely *mentions* `Scope::Project` — measured at 60 bytes, by mutating the production path rather than the test's inputs. Two further variants form the control pair, establishing that the guard is functional and that the counting method works, which is what makes the other two measurements rather than a broken harness. **Classes checked.** `IC-9` (`assertion-satisfiable-by-accident`) is nearest and still wrong: its claim turns on **environment-controlled** text and this is project source text — that ledger has already withdrawn two tags matched on the looser reading *"a test that passes when it shouldn't"*, and forcing a third would re-corrupt the very count the withdrawal restored. `IC-16` (`assertion-that-cannot-fail`) fails because this assertion has failing inputs; two of the four variants are them. `IC-14` (`guard-narrower-than-its-name`) is its exact inverse on the over-refusal half. **What it IS is the second instance of the candidate class parked in `IC-14`'s own member note** — *"a guard's trigger matches a superset of what its name claims"* — where `worktree-guard-word-boundary-blocks-read-only-git-plumbing` made a regex word boundary match `git merge-base`; here a byte window makes *"the default is absent"* match *"the default is 400 bytes away"*. Different mechanism, identical shape, both refuse valid work. **It also exceeds that candidate class,** which covers over-refusal only and says nothing about the false-GREEN half, so whoever promotes it owns deciding whether the two directions are one class or two. Diagnostic worth carrying regardless: a constant-width window standing in for a syntactic construct trades its two failure directions against each other — widening buys the false-RED and worsens the false-GREEN — so the repair is a delimiter, never a bigger number. Count not re-derived for this +1. +1: `grep-c-exit-status-answers-a-different-question-so-the-seeded-count-prints-twice` (2026-09-09) — `install-hooks.sh` writes `seeded="$(grep -c . "$seed_log" || echo 0)"`. `grep -c` prints its count on **stdout** and reports **match/no-match** through its exit status; the caller reads that status as **success/failure**, so `|| echo 0` fires on a perfectly good empty result and `$seeded` becomes the two-line string `0\n0`. The mechanism is *one channel carrying two meanings, and the reader taking the wrong one* — and there is no class for it. Checked four before reaching for the hatch: `IC-14` (`guard-narrower-than-its-name`) is the inverse shape and this is not a guard at all; `IC-18` (`selector-narrower-than-its-population`) is about scope, not about a value's provenance; `assertion-that-cannot-fail` and `assertion-satisfiable-by-accident` are both about tests and nothing here asserts. Filed here rather than forced, because tagging it into `IC-14` would corrupt a count that class's promotion threshold reads. The candidate class, once a second instance exists, is *"a status channel answers a narrower question than the caller reads it as"* — note POSIX specifies exit 1 for `grep`'s no-match, so every `grep -c` in a `||` composition is a member by construction, which is a population worth counting before promoting. Count not re-derived for this +1. +1: `doc-move-scans-for-inbound-path-citations-and-not-for-the-id-it-just-re-keyed` (2026-09-10) — `doc(action="move")` scans the repo for files citing the OLD PATH and reports them, but runs no equivalent scan for the 16-hex id the same call just re-keyed, while `artifact.rs:183` tells the caller to re-point both. Two sessions hand-repointed a stale id forty minutes apart on 2026-09-10, the second having read the response and acted on the field that existed. Looked at `IC-18` (`selector-narrower-than-its-population`) and `IC-14` (`guard-narrower-than-its-name`) — both rejected because the field is honestly NAMED `inbound_path_citations` and covers path citations completely; the defect is a missing second field, not an under-reaching first one, and forcing either would move a promotion count. Provisional shape for a second instance, offered not claimed: *a response serves the cheap half of a two-half obligation and narrates the other half*, where both halves cost the same call. Count not re-derived for this +1. — **+1: `a-conflicted-merge-yields-a-patch-id-and-it-is-the-wrong-one`** (2026-09-10). `CLAUDE.md` § *Bug Tracking*, § *Git Workflow* and `get_guide("tracker-conventions")` § *Bug files* all state that a merge commit has no patch-id because `git show` emits no diff, so the prescribed pipeline returns empty and exits 0. Measured: true of a CLEAN merge (`4485eeb0`, 457 bytes, empty patch-id) and **false of a conflicted one** (`8cf67de0`, 32,634-byte combined diff, patch-id `9817e7c6…`) — so the documented failure mode is an empty field a reader notices, and the real one is a well-formed value they record that hashes only the resolution hunks. **Why it is here rather than in a class, and this is the whole entry:** `IC-11` requires the statement to have been *true when written* and says outright *"unlike a wrong statement, this defect has no authoring error to find"* — this one has one, the claim was never true of conflicted merges and git did not change. `IC-18` is the **mirror**: here the rule's selector (*is it a merge?*) is WIDER than the population its reasoning holds for (*is it a clean merge?*), and `IC-6`'s own ledger note already rejects a tag on exactly that basis, that class being a guard covering LESS than its name. **The provisional shape, offered not claimed:** *a universal generalised from one subcase, whose other subcase returns a plausible value rather than the documented silence.* If a third instance of that turns up it is a class; two is not, and forcing this into either neighbour would move a promotion count on a reading its own ledger refutes. Count not re-derived for this +1. +1: `two-of-the-three-prompt-byte-budgets-are-documented-only-in-their-own-failure-strings` (2026-09-10) — `src/prompts/README.md` documents `TOOL_SURFACE_CHAR_BUDGET` in full and returns ZERO matches for the two guide budgets beside it (the 2500 B per-section cap and the p50 emission ceiling), so a guide edit reds three gates whose bounds live only in Rust and in their own failure strings. Looked at `IC-2` (`gate-keyed-on-unobservable-event`) and rejected it: these gates observe exactly the right thing and fail loudly and correctly — only the BOUND is unpublished, which is `OB-1` position 3 rather than a proxy substitution. No existing slug names *a correct gate whose limit is documented nowhere its author would look*. Count not re-derived for this +1. +1: `the-references-manual-page-teaches-name_path-a-parameter-the-tool-has-never-accepted` (2026-09-10) — `docs/manual/src/tools/symbol-navigation.md`'s `## references` section teaches `name_path` as the tool's required identifying parameter; the live `input_schema()` has only ever required `symbol`. Looked at `IC-11` (`doc-contradicted-by-code`) first — the shape matches its surface, a manual naming a dead parameter — and ruled it out by the same test that class prescribes: `git log -S'name_path' --all` over every historical filename this tool has had (back to its 2026-04-22 extraction into its own file), plus the schema one commit before that extraction, returns zero matching commits. The required key has been `symbol` since the earliest recorded schema, so no code ever changed under a once-true sentence — this reads instead as conflation with the neighbouring `symbols` tool, which genuinely does alias `name_path` to `symbol` (`47af2676`, formalized as a real `param_aliases()` pair by `34cad9d9`). `IC-11`'s own admission test excludes exactly this shape ("an ordinary authoring error with an author to find... does not belong here"), so it is filed under the hatch instead. Count not re-derived for this +1. +1: `subagent-transcripts-are-byte-identical-across-profile-dirs` (2026-09-11) — 228 subagent transcripts on this machine exist as byte-identical copies across 2-3 of its three Claude Code profile directories, mechanism unestablished. Looked at `IC-17` (`shared-resource-carries-no-owner`) and `IC-12` (`transient-shared-state-lies-to-readers`) — both wrong: this is cold, committed-to-disk content, not a live/transient state being misread, and no ownership or attribution claim is at stake, only unexplained duplication. Filed under the hatch because the mechanism (sync script, shared mount, manual backup, something else) has not been identified, and a class claim before knowing the mechanism would corrupt whichever real class this turns out to instantiate. +1: `the-skill-ledger-writes-to-a-deleted-worktree-and-restamps-every-first-seen-as-now` (2026-09-11) — two components of ONE plugin derive the same buddy session directory by different keys: the writer takes the hook event's `cwd`, the reader takes `workspace.current_dir` with `cwd` only as a fallback. When those disagree — a session whose worktree has been deleted — one session ends up with two ledgers, and `load_ledger` cannot tell *"no skills loaded yet"* from *"I looked in the wrong directory"*, so the second is written fresh with every skill's first-seen stamp re-set to now: five distinct stamps collapsed onto one instant, measured 2026-09-11. Looked at `IC-12` (`transient-shared-state-lies-to-readers`) first and it is the closest by EFFECT — a reader does get a wrong answer — but wrong by MECHANISM: that class needs state that is shared between readers and transient, whereas this is one session's own state, cold on disk, split across two roots by two resolution rules. Tagging it there would make the class's own definition stop discriminating, which is the cost the hatch exists to avoid. `IC-15` (`accepted-parameter-silently-dropped`) — nothing is dropped; a path is resolved differently. `IC-8` (`record-asserts-an-unchecked-completion`) — the false value is a first-seen timestamp, not an assertion that an action completed. Filed under the hatch because the candidate class has no second instance in this corpus yet: *"one logical location derived by two rules, so writer and reader address different objects"* — named here so whoever meets the second instance can promote it rather than re-derive it. Note the shape it shares with the promoted classes without belonging to any: the wrong read returns a complete, plausible value rather than an error, so nothing downstream fires. **+1: `atomic-write-rename-not-detected-by-file-watchers`** (2026-08-14, arrived via the 2026-09-11 merge of the native-Windows VDI branch) — codescout's writes go through `atomic_write()` (write to `<path>.tmp`, then `rename()` over the target), and at least one downstream dev workflow (`uvicorn --reload`, watchfiles backend, on Windows) never receives a change notification, so a running server keeps serving pre-edit code after a codescout edit. **Looked, and the two near misses fail in different directions.** `IC-2` (`gate-keyed-on-unobservable-event`) is the closest shape — the watcher genuinely substitutes a proxy (*"a write event occurred"*) for what it wants (*"the content changed"*) — but every member of that class is a gate **in this repo**, keyed by its own author; here the substituting party is a third-party watcher in another project's toolchain, and nothing codescout ships is keyed on anything. `IC-12` (`transient-shared-state-lies-to-readers`) requires a window that closes and a reader misled by live state: the file on disk is correct and permanently so, and what never happens is the **notification**, not a misread. The provisional shape, offered not claimed: *a write path chosen for atomicity is invisible to an observer that watches for writes* — the two properties are in direct tension, and the safer write is the one that hides. Count not re-derived for this +1. +1: `the-committed-scripts-gate-scans-the-filesystem-so-an-untracked-file-reds-it` (2026-09-11) — **and this one is filed here to be COUNTED rather than because nothing fits.** `no_committed_script_hardcodes_a_personal_home_path` is named for, and reports on, *committed* scripts; its population is `std::fs::read_dir` over `scripts/` and never consults git, so an untracked in-progress file reds the shared gate for every session and tells the reader their committed scripts are broken. Verified untracked at the bytes: `git ls-files --error-unmatch` exits 1, `git status` says `??`, `git log -- <path>` is empty. That is the **second instance of the candidate class this very field named when the first one landed** — *"a guard's trigger matches a superset of what its name claims"* — whose first instance is `worktree-guard-word-boundary-blocks-read-only-git-plumbing` (2026-09-03), where `merge\b` matches `git merge-base` and six read-only plumbing commands are refused as destructive. **The spread bar is already met at `n=2`**: a Node shell guard and a Rust test gate are two subsystems, so instance three clears both bars on arrival and the class should be promoted rather than re-noticed. `IC-14` (`guard-narrower-than-its-name`) and `IC-18` (`selector-narrower-than-its-population`) remain the wrong homes for the same reason as last time and it is worth restating, because tagging either would corrupt the count a promotion reads: both classes run the opposite direction — they MISS work, these REFUSE it — so a member here would make each class's own definition stop discriminating. +1: `doc-update-writes-the-file-then-fails-the-catalog-and-reports-only-the-failure` (2026-09-11) — **the FOURTH instance of the candidate class this field already names** — *a write path updates one half of the file/catalog pair and reports success* — **and it widens that claim, because this one reports FAILURE.** `doc(action="update")` committed a `body_edits` section to disk, then lost a lock race on the catalog half and returned the bare string `database is locked`. The three instances already named here all report success, which costs a reader a divergence they may never notice; this reports an error naming only the store that failed, which costs them something worse — `database is locked` is SQLite's ordinary busy-timeout response, retry is the textbook reading, and the retry appended a byte-identical duplicate of the section. **The diligent reading of the error produces the damage.** It landed on prose here; the same partial write against a ledger mints a second definer of a `## PREFIX-N` id, which `link_scan` resolves to nothing (`IC-6`) and which the pre-push guard's `git grep -c '^## PREFIX-N' >= 2` already refuses a push over. **Answering this field's own standing question for a fourth instance** — *whether `doctor` has grown a `frontmatter_status_mismatch` companion to its two `frontmatter_id_*` checks* — **it has**: `doctor.rs:372` declares it and `:656` wires `scan_frontmatter_status_mismatches`, so the class's population is no longer unmeasurable and a count cited for it need no longer be hand-derived. **Still parked rather than promoted**, and on the unchanged bar: every instance so far sits inside the librarian, so the ≥2-subsystem spread is unmet however many there turn out to be — the blocker was never the count, and a count is derivable from `scripts/probe-cluster-census.py` by anyone who wants one. Not tagged into a real class for the usual reason: `IC-12` (`transient-shared-state-lies-to-readers`) needs a *concurrent reader* misled by a live window, and the party misled here is the **caller of the failing write**; `IC-8` (`record-asserts-an-unchecked-completion`) is inverted, since nothing asserts completion — the call asserts failure and half-succeeded. Count not re-derived for this +1. +1: `edit-code-replace-with-a-renamed-body-misattributes-the-error` (2026-09-13) — **MOVED OUT 2026-09-14 to `cluster/hint-composed-without-the-request` (IC-22)**, with the one above and for the same reason; the derivation now lives there. Neither `IC-14` (`guard-narrower-than-its-name`) nor `IC-16` (`assertion-that-cannot-fail`) fits: the guard's *trigger* is correct in both cases, only the message's *stated cause* is wrong for one of them. **+1 2026-09-13: `background-command-loses-terminal-status`** — `run_command(run_in_background=true)` reports `Process running` after the job has already exited, retains no terminal status, and a later read of the log handle returns an exit code describing the READER rather than the job: `sh -c 'exit 7'` surfaced `exit_code: 0`, while a fixture echoing its own `$?` proved the 7 existed and was simply never carried. Filed here rather than forced, and the two near-misses are named so the next reader does not re-litigate them. `IC-8` (`record-asserts-an-unchecked-completion`) is closest — `Process running` IS an unchecked assertion — but `IC-8`'s members are persisted *records*, and widening it to cover a transient response field would blur the discriminator its count rests on. `IC-23` (`attribute-derived-at-container-granularity`) fits the reader-vs-job exit code only until you state it precisely: the reader's exit code is *correctly* the reader's, so nothing was derived at the wrong granularity — **no channel carries the job's status at all**, which is an absence rather than a mis-derivation. **The `IC-23` refusal has a second reason that is stronger than the fit argument and is the one to carry forward:** `IC-23` was opened the same day at `n=1`, and a one-member class is the worst possible population to admit a marginal fit into — one wrong member there does not dilute the class, it *redefines* it. Raised by a peer session reviewing this append; recorded because the reasoning is reusable and the fit argument alone would not have reached it. +1: `the-stale-server-probe-counts-lsp-muxes-under-a-name-that-excludes-them` (2026-09-14) — **the second instance of the candidate class this field itself named.** `scripts/stale-servers.sh:39` selects with `pgrep -x codescout`, which matches LSP muxes as well as MCP servers, so the printed `total=` is a mixed unit and the remedy at `:67` (*"Reconnect those sessions (/mcp)"*) names an action no reader can perform on a mux row — a mux has no session and self-heals at its `--idle-timeout`, where a server never recycles. Checked `IC-14` (`guard-narrower-than-its-name`) and `IC-18` (`selector-narrower-than-its-population`): both are the NARROW direction and this is the wide one. `IC-20` (`floor-published-under-the-name-of-a-total`) fails on its distinguishing clause — its claim turns on the walk having *stopped*, which makes the true value unknowable, whereas this walk **over-collected** and the true value is one `cmdline` read away; tagging it there would dilute exactly what keeps `IC-20` apart from `IC-19`. **The match to record is with `worktree-guard-word-boundary-blocks-read-only-git-plumbing` above**, which named the candidate class *"a guard's trigger matches a superset of what its name claims"* and asked for a second instance. This is a second instance of *superset* and **not obviously of the same consequence**: theirs refuses work that should pass, this one returns a number under the wrong unit and routes the reader to a party that does not exist. Whether the class is superset-of-TRIGGER or superset-with-a-misrouted-REMEDY is the adjudication a third instance should settle — recorded unpromoted so that it is not settled by whoever happens to file next, which is the failure mode `IC-11`'s early promotion already paid for. **THIRD DATAPOINT, 2026-09-14 08:05, and it is a datapoint rather than a member** — no bug file, because the defect was in a Resume line the second member's own file had shipped forty minutes earlier and it was fixed in place. That line told readers to run `pgrep -a -f 'cargo test'` before starting a gate; `-f` matches the whole command line, so the shell asking the question matches itself. Observed: exactly 1 hit, and the hit was the asking shell — **100% of the result was the instrument**, and the check cannot return zero when invoked that way, so it reads *"a gate is running"* every time and the action it gates is never taken. Recorded here because a count kept only in the instance's own file is the gap this roster exists to close. **It also sharpens the candidate class rather than merely incrementing it:** members one and two are supersets of an EXTERNAL population (git plumbing verbs, LSP muxes), while this one's superset contains the OBSERVER — the instrument is a member of the population it measures. Whether self-inclusion is the same class or a sibling is now the open question, and it is a better question than the one two instances posed. **A THIRD instance of the shape those two share was filed 2026-09-14 and tagged `IC-2`, not held here** — `the-fmt-refusal-names-an-owner-who-holds-none-of-the-bytes`, in a third subsystem (a shell gate, against symbol editing and result-cap classification). It is NAMED rather than counted, because it is not a member of this class: a bug carries exactly one `cluster/` tag and that one went to `IC-2` on a fit test, `scripts/fmt-mine.sh`'s owner list substituting *who wrote in a window* for *who holds the unformatted bytes*. **The population argument survives the retag, which is why this sentence stays** — the shape now has three instances across three subsystems, so whoever opens it inherits a defended inclusion test rather than inventing one. And that the third ALSO fits `IC-2` cleanly is itself evidence the shape is real but UNDER-SPECIFIED: the two readings pick out different halves of one defect, and only one half has a class to go to. +1: `the-compile-advisory-reports-a-cached-failure-as-current-state` (2026-09-14) — a `doc(update)` response asserted *"your uncommitted edit does not compile"* with 33 errors four minutes after the four-command gate ran green on that tree; `cargo check --workspace --all-targets` exited 0 in the same minute and every named symbol was present. The diagnostics were true at an earlier instant and carried no timestamp. Filed HERE rather than forced into a class, and the reason is the escape hatch's own purpose: it sits between `IC-8` (*a record asserts a completed action nothing re-checked* — right about the unchecked assertion, wrong that a compile state is a completed action) and `IC-12` (*transient shared state lies to readers* — right about the lying, but the stale result is LOCAL while only the advisory's closing claim, "peers' gates see this break too", is about shared state). `IC-12` stood at `n=2` when this was filed — a quotation, derivable with `python3 scripts/probe-cluster-census.py` — so a wrong third member trips its promotion threshold — the specific corruption this ledger warns about. Retag when the root cause is measured; the bug file records the mechanism as inferred, not established. +1: `the-ack-note-states-a-residual-obligation-and-names-no-one-to-discharge-it` (2026-09-15) — a guard's message states a residual obligation in its closing sentence and names no party, no action and no way to reach one, while holding the party list in scope. Distinct from the remedy-text law `CLAUDE.md` § Testing Discipline already carries, and from `OB-20`'s ceiling on it: both are about an addressee who cannot usefully ANSWER, and here there is no addressee at all — the sentence is true, agreed with, and actionless. Weighed and rejected: `IC-21` (`instrument-omits-the-dimension-that-grows`) is a count where the decision turns on MAGNITUDE; this is a count where it turns on IDENTITY, and the omitted value is already computed. `IC-14` (`guard-narrower-than-its-name`) fails because the guard's coverage is exactly right — it refused nothing it should have allowed. `IC-2` (`gate-keyed-on-unobservable-event`) fails because the event is fully observed; only its consequence goes unpublished. Measured with the filer as the instance: three sessions published, zero told, surfaced three hours later by one of them reconstructing the range and undercounting their own commits 5 against 12. Candidate class if a second appears: *a message states a residual obligation and names no party who can discharge it*. +1: `doctors-undeclared-valid-check-counts-citation-rows-and-reads-as-entries` (2026-09-15) — an instrument publishes a count in a unit the reader does not share: `doctor`'s `entry_cited_from_outside_but_undeclared` emits one row per citation edge, `summary.by_check` counts rows, and a triage reads it as entries — 18 against 9 distinct, with one entry contributing ten byte-identical rows that carry no key to deduplicate on. The error is not random, which is what rules out "just a bug": it scales with the citation count of the worst member, so the entry the check exists to PRIORITISE (it is priced by exposure) is the one that distorts the count most, and each row's own text says *"this is a worklist, not a verdict"* before handing over a length in the wrong unit. Weighed and rejected: `IC-20` (`floor-published-under-the-name-of-a-total`) is the nearest and still fails on direction — a floor understates and this overstates, and the remedies differ accordingly (publish the ceiling vs. publish the unit). `IC-13` (`capped-result-presented-as-complete`) fails because nothing is capped; `shown` equals `total`. `IC-21` (`instrument-omits-the-dimension-that-grows`) fails because no dimension is omitted — the number is present and countable, it is the UNIT that is unstated. `IC-18` (`selector-narrower-than-its-population`) fails because the selector is correct: it finds exactly the right entries and then projects them at the wrong grain. Candidate class if a second appears: *a count is published in the emitter's unit and consumed in the reader's*, whose tell is a report where one row can be duplicated without any field distinguishing the copies. +1: `probes-recommends-strings-to-settle-a-binarys-contents-and-a-miss-proves-nothing` (2026-09-15) — a documented instrument index recommends two probes as co-equal when one answers in both directions and the other in only one: `docs/PROBES.md`:180 offers *"probe a behaviour or `strings` the binary"*, and a `strings` hit is sound while a miss is uninterpretable. Measured across two builds of the same source — a control string present in BOTH source versions returned 0 on the 65 MB binary and 1 on the 42 MB one — so the blindness is per-BINARY, not a property of `strings` over Rust release binaries, and nothing in the output says which regime you are in. Sharper: the 65 MB binary was overwritten by the next rebuild, so a past 0 is **unauditable**, which is the normal case rather than bad luck. **WIDENED 2026-09-15 — the class is INSPECTION, not `strings`, and the row would still be wrong with `strings` deleted:** a reader who distrusts it reaches for `ldd`, `nm`, `objdump` or `readelf`, every one one-directional for the same reason — absence is a property of the SCAN, not of the binary. The family splits in two, which changes the ADVICE and not merely its scope. CONTINGENT (`strings`): the 0-then-1 measurement above is the proof, and nothing in the output says which regime you are in. STRUCTURAL (`ldd` on this crate): `crates/codescout-embed/Cargo.toml`:18 declares `local-embed` a *static* prebuilt ONNX Runtime, so a static link appears in no link map by design, always — worse in never answering the question, better in being knowable in advance from `Cargo.toml` alone. Practical consequence, and the reason the widening matters: **a reader told only "`strings` is unreliable" reaches for `ldd` and lands on the WORSE instrument** — measured, that is exactly what happened one step later and was called *"the decisive test"*. Sub-kinds raised by sessionId `f5f48b42-6d84-482e-84a4-8eaebb0ce60f`, verified at the bytes here. **Why this line needed widening at all is itself the class:** membership did not change, so the gate does not fire, so a derivation narrower than its claim decays with nothing to catch it. Weighed and rejected: `IC-20` (`floor-published-under-the-name-of-a-total`) is the near fit and fails ITS OWN falsification test — *"falsified by a member whose true total was recoverable"* — because the truth here was recovered by a behavioural probe; `IC-20` needs the walk to have STOPPED with the value gone, and `strings` does not stop, it under-collects. `IC-13` (`capped-result-presented-as-complete`) fails because nothing is capped and no marker could arrive. `IC-11` (`doc-contradicted-by-code`) fails on polarity: the doc GRANTS a capability the artifact lacks rather than denying one it has. Candidate class if a second appears: *an instrument index recommends alternatives that are not interchangeable in both directions*, whose tell is a row that caveats one instrument carefully and its neighbour not at all. +1: `three-hooks-discrimination-cases-failed-once-and-did-not-reproduce` (2026-09-16) — UNCLASSIFIED ON PURPOSE, and the honesty is the content: one run of the mutation suite reported three failures in cases the mutated file cannot reach, and it did not reproduce in 12 subsequent runs across four environments. The mechanism is unknown, so any claim-shaped class would be a hypothesis wearing a taxonomy. **It is filed at all because of what the suite IS** — this repo's mutation evidence for the shared-checkout hooks — so an intermittent failure inflates a kill count, and the same intermittency in the other direction masks a SURVIVAL, which reads as *the test discriminates* when it does not. Retag it the moment a second observation names a mechanism; until then the record's value is its 12 negatives, not its one positive. +1: `a-symlinked-instruction-file-is-cataloged-as-a-second-artifact` (2026-09-16) — `AGENTS.md` is a symlink to `CLAUDE.md`, `src/librarian/indexer.rs` has no symlink branch at all, and `id = sha256(abs_path)` therefore mints a second id for the same bytes: two artifact rows and 84 chunk rows for one document, silently. Filed here because the shape is **an identity key that is not injective over the population it addresses**, and no existing class states that. `IC-6` was weighed and rejected as the near miss: its *no-disambiguator* half is two THINGS colliding on one address, permanently unaddressable — this is the inverse, one thing with two addresses, each perfectly addressable. `IC-23` rejected — there is no container/item pair; the path is the intended key, not a coarser one stamped onto items. `IC-14` rejected — no guard is involved, so nothing is narrower than its name. `IC-24` rejected on the class's own test: the value is not correct-in-another-frame, there are simply two rows. **A second instance would be called `identity-key-is-not-injective-over-its-population`**, and the place to look for it is anywhere a hash of a path, name or handle stands in for a document — the same substitution `doc(action="move")` makes when it re-keys on rename. Note the near-relative that is NOT this: `3a9eb5153e5311a8` (`cluster/guard-narrower-than-its-name`) is the same *seam* — canonicalised versus un-canonicalised paths — reached from the guard side, and it is why fixing this by canonicalising is a choice between two live conventions rather than a free repair. **+1: `the-machine-default-dotenv-outranks-every-per-project-override`** — held here deliberately rather than forced into a neighbouring slug, and the non-fit is the informative part. `~/.config/codescout/.env` is the machine's **default** layer in practice, but `load_startup_env` injects it into the process environment, and env is the **highest**-precedence layer in `merge_embed_config` — so the layer whose role is *fallback* behaves as *override* and silently defeats `.codescout/project.toml`, the layer that exists to let a project differ from the machine. The failure is a silent success against the wrong backend: `codescout index` exits 0, reports `added=1`, and the endpoint the project configured receives nothing (measured 2026-09-17; the discriminator is pointing `CODESCOUT_ENV_FILE` at an empty file, after which the same command reaches it). Checked against three candidate classes and rejected from each for a stated reason: **not `IC-4`**, whose claim is that propagation is *additive* — updates land and removals do not — while here every value propagates correctly and only their **order** is wrong; **not `IC-15`**, since nothing is accepted-then-dropped — both values are live, one simply loses; **not `IC-14`**, since no guard is involved. The nearest true statement is that a layer's *role* and its *precedence* disagree, which no current slug claims. Left unclassified rather than mis-tagged because the honest cost of a wrong tag is a class whose membership stops predicting its own remedy — and this one's remedy is specific: not a precedence change, but **provenance**, distinguishing a value the operator exported from one a file injected. `startup_env_assignments` already returns exactly the keys it assigned and discards the list. Retag if a second instance of role-versus-precedence disagreement appears; one member is not a class.

`doc-overflow-never-routes-to-progressive-disclosure` — `LibrarianAdapter::relevant_guide_topic` never checked for overflow, so an overflowing `doc()` call could never surface `progressive-disclosure`. Looked at `IC-16` (`assertion-that-cannot-fail`) for the compounding defect — the first fix's own unit test passed by hand-inserting an `output_id` key no real `doc()` call produces at that point in the pipeline — but `IC-16`'s claim is an assertion with **no failing input at all**; this one has a failing input (the mutation-verify run proves it), it is just the WRONG input, crafted to match the buggy implementation rather than the production shape. No existing class captures a test authored against an unrealistic input rather than a vacuous one, so filed under the escape hatch instead.

`a-reverted-ledger-burns-an-entry-id-with-no-warning` — **filed here, then closed
`wontfix` the same day; do not count it as a live defect instance.** `append_entry`
allocates from `max(body_max, reserved_max, frontmatter_max) + 1` while the reservation
lives in the catalog and the entry lives in the file, so a `git checkout -- <ledger>`
reverts one half, cannot reach the other, and the next allocation silently skips the
orphaned id (burned `GG-10` in `resume-get-guide-section-grain-phases-2-3`). Reading the
allocator overturned the defect claim: `augmentation.rs:932` accepts the leak in terms
(*"Deliberate: integers are cheap"*) and `append_entry.rs:236` reasons explicitly that
only one of the three relations earns words, because a warning would train agents to
repair a state that **cannot** be repaired. Eleven classes were checked before the hatch
was used; `IC-21` was closest and still fails, since what is withheld is the *meaning* of
the three counts rather than a magnitude — and it is withheld on purpose. Candidate class
for a second instance: *an undo covers one of two stores that jointly hold one fact*,
provisional slug `partial-undo-across-split-stores`, which would be a class about the
**surprise** rather than about a defect. Kept tagged so this line and the `**Members:**`
field stay consistent; `cluster/unclassified` drives no promotion threshold, so a
by-design member distorts nothing.

+1: `a-backgrounded-gate-command-was-summarised-as-exit-0-while-its-buffer-held-the-failure`
(2026-09-14) — the hatch used for its stated purpose again, and filed here for the same reason as
the entry below rather than by analogy to it. A `run_command` with `run_in_background: true`
summarised a failed `cargo clippy` as `✓ exit 0` while the buffer it named held `could not
compile ... due to 1 previous error`; the caller read the summary, took the gate as green, and left
a truncated source file redding the shared build. Three controlled variants — slow fail, fast fail,
and 80 000 lines of output plus a fail — all returned the correct `Process running` shape with no
exit claim, so the trigger is unidentified. Weighed and rejected, and the rejections are the
useful part: `IC-13` (`capped-result-presented-as-complete`) fits the *envelope* — an overflow
response whose summary stood in for a buffer — but its claim is that a **capped** result reads as
complete, and nothing here was capped; the summary asserted an exit status it had no basis for,
which is a different defect wearing the same shape. `IC-22` (`hint-composed-without-the-request`)
is nearer than it first looks, since `✓ exit 0` is composed without consulting the state it
describes — but that class is about hints and causal explanations, and a **status field** is
neither; tagging it there would widen the claim by assertion rather than by a member that earns
it. `IC-2` needs a gate keyed on something, and this is a report, not a gate. Candidate class if a
second instance appears: *a summary asserts a property it could not establish*, provisional slug
`summary-asserts-an-unestablished-property`.

`patch-must-be-a-json-object-refusal-is-unreproduced` (2026-09-04) — the hatch used for
exactly its stated purpose, *looked, nothing fits*, and specifically because every named class that
could fit would smuggle in an **unestablished premise**. Two `doc(action="update")` calls were
refused with *"patch must be a JSON object mapping field names to new values"* on patches that were
objects with one `body_edits` key; splitting the same work into two smaller calls succeeded seconds
later against the same artifact, same session, no intervening change. The closest named class is
`cluster/hint-composed-without-the-request` — and it asserts the payload **arrived intact**, which is
the one claim the file declines to make: transport mangling of a large payload carrying literal
non-ASCII would make that error *accurate about what it received* and put the defect upstream of the
librarian entirely. A five-variant isolation, using a probe that separates parse from application (a
patch that parses fails on a deliberately absent `old_string`, so no write can occur either way),
eliminated the warning-sign codepoint, array arity, em-dash content and payload size — so the trigger
is **unknown rather than unexamined**, and the two surviving readings blame different systems. A
class assignment would read as settled while resting on the only thing not established: which system
is at fault. Since `cluster/unclassified` drives no promotion threshold, an honest *don't know yet*
costs the taxonomy nothing, whereas a plausible-looking tag would cost the next reader the
re-derivation.

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
**The next three derivations describe members that LEFT this class on 2026-09-16, and are kept
deliberately.** `chunk-line-ranges-are-body-relative-but-published-as-file-lines`,
`doc-tool-refs-counts-call-param-pairs-as-documents` and
`a-move-reports-slug-and-path-citations-under-one-field-named-for-paths` were retagged to
`cluster/value-correct-in-a-frame-its-name-does-not-state` (`IC-24`); they are no longer named on
the `**Members:**` line above, which fell from 33 entries to 31 in the same change while the
tag census fell 33 → 30. **Kept rather than deleted because the three REJECTIONS are the new
class's derivation, not merely its history:** each weighed `IC-20` and refused it on that row's
own falsification clause, and that refusal is exactly what carves `IC-24` from it. Delete these
and the next reader sees a class asserting a boundary with nothing behind it. Each also closes
with a *"candidate class if a second appears"* note naming a NARROWER version of one shape —
coordinate space, unit, form — written by three authors who could not see that the other two had
already appeared. Record:
`docs/trackers/issue-clusters/IC-24-value-correct-in-a-frame-its-name-does-not-state.md`.

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

`a-move-reports-slug-and-path-citations-under-one-field-named-for-paths` (2026-09-15) — a detector
widened to see a second citation form publishes both under the OLD form's name, and the two take
**opposite** remedies: a path citation is dead and must be re-pointed, a slug citation is untouched
by the move and must not be. Measured over three consecutive archive-moves in one session — every
`IC-N` entry the field reported was slug-only, and one call **interleaved** four real path citations
with one slug, so *"it is all slugs now"* is not available as a shortcut either. Three weighed, each
rejected on its own claim rather than by feel: `IC-18` is inverted — a selector seeing LESS than its
name claims, where this sees more and correctly, so the zero-reads-as-absent tell cannot apply
because nothing is missing from the output. `IC-20` is nearest on the remedy axis (rename the
quantity, or refuse to print it) and is excluded by its own falsification clause: there the true
value is unknowable because a walk stopped, while here every value is present and exactly right and
only the KIND is unpublished. `IC-21` is magnitude-versus-count; this is form-versus-label. Candidate
class if a second appears: *a widened detector publishes its new population under the old one's name,
so the caller cannot tell which remedy applies* — blind party being the author of the widening fix,
for whom both forms **are** citations, which is the correct reading for detection and the wrong one
for repair.

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

**`**Members:**` is ONE LINE, and the gate reads only that line.**
`scripts/pre-commit-ledger-counts.py:376` keys on `line.startswith("**Members:**")`, so every
`+1: <slug>` stem must sit on *that* line. Derivation paragraphs beneath it render as part of the
field and are **invisible to the check** — a member named only in the prose below satisfies nothing.
The refusal text does not discriminate either: *"expected the field to change and to contain one of:
<slug>"* is equally true of an edit that changed exactly that prose, so it is accurate and
under-determines the fix.

Measured 2026-09-04, and it is the **accidental passes** rather than the refusals that argue for
writing it here. Three sessions satisfied this gate without knowing what it checks — each appended
to the end of a class (`IC-2`, `IC-6`, `IC-14`) whose entire membership happens to occupy one long
line, so the stem landed on the line by luck. A refusal teaches; an accidental pass installs a wrong
model that survives and gets repeated. The rule was already written down — inside
`cluster/unclassified`'s own `**Members:**` line, which is the one place a reader reaches *after*
being refused. That is `OB-1`'s third position, a bound published to an audience that never reads it,
so this is a **move to the read surface** rather than a second copy: an author adding a member to any
other class never had cause to open `unclassified`.

**The one-line shape is deliberate, so do not "fix" it by wrapping.**
`docs/issues/archive/2026-09-04-the-chunker-budget-is-not-a-bound-a-single-line-cannot-be-split.md:166-171`
records the constraint from the other side: a `**Members:**` field whose members sat on continuation
lines *would* chunk normally — which is wanted, because a single line cannot be split and a long one
exceeds the embedder's input budget — but *"the one-line form is load-bearing for the gate as
written, and any wrapping change must move the parser with it."* So the shape is a live trade-off
between the gate and the chunker, not an oversight, and the two halves have to move together. That
is a **second** place the constraint was already documented, and neither is where an author adding a
member stands — which is the whole argument for this paragraph existing here rather than a third
restatement existing somewhere else.

*Updated 2026-09-04: that record is fixed at `8acec9c7`, and it changes what this trade-off costs
rather than removing it. `EmbeddingService::embed_artifact` now segments and mean-pools above the
model's budget, so an over-long line no longer loses its vector outright — the clause above about
exceeding the embedder's input budget is now historical. What remains is that a 26 KB line pools to
one blurry vector, so the class grows progressively less findable instead of abruptly unfindable.
The gate-vs-chunker tension stands; only its severity dropped, from data loss to retrieval quality.*

**And expect a third and a fourth, because the two prior records are not two accidents.** A
constraint gets written down by whoever it **bites**, at whatever surface they happened to be
standing on when it bit them — here, a class's own member field and an embedder-budget bug file —
and that is never where the *next* author will be standing. The accretion point is the injury site;
the authoring surface is somewhere else by construction. So for this class the remedy is always a
**move** to the authoring surface and never another copy: a copy written at the point of injury is
the failure repeating itself one address further along, which is what a third restatement of this
very rule would have been. (Generalisation contributed by a peer session, 2026-09-04, from the two
datapoints above.)
