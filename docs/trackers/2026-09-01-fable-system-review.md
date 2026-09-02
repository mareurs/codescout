---
id: '15deb24ba2a35978'
kind: tracker
status: draft
title: System Review by Fable — the tracker ecosystem as a self-improvement loop (SR-N)
owners:
- marius
- fable
tags:
- system-review
- emergent-behavior
- self-healing
- meta
- architecture
- promotion
topic: 'end-to-end review of the tracker/self-improvement system: validation, critique, architecture verdicts'
entry_high_water_SR: 15
entry_prefix: SR
---

> **Prefix:** `SR-N` — one finding per entry from Fable's end-to-end review of the tracker /
> self-improvement system, session of 2026-09-01. The session answered three questions in order:
> (1) validate the "emergent self-healing" hypothesis, (2) give an honest overall assessment —
> insight / friction / bad design / wrong architecture, (3) name what the operator and the model
> both *enjoy* in the corpus and what that shared taste selects for. Every measured figure below
> carries its derivation; re-run the derivation rather than trusting the number.

## What this review is

Requested by Marius mid-conversation: *"write all these findings in a tracker … some ideas in
here are worth a lot so please don't lose them."* Findings SR-1 through SR-4 are the validation
(what was verified true), SR-5 through SR-9 the critique (costs and unmeasured links), SR-10 and
SR-11 the architecture verdicts, SR-12 the register/enjoyment analysis, SR-13 live-session
frictions, SR-14 ranked recommendations, SR-15 the standing question only the operator can answer.

**The sibling review — found on a second pass (2026-09-01):**
`docs/trackers/system-retrospective-improvements.md` ("System Retrospective 2026-09 —
Improvement Tasks", artifact `6f5ec09c63aef864`), committed in `3a422b31` — the same commit that
added the catalog-audit-trail design its T-1 prescribed, exactly as Marius remembered it: the
review created the design. The first search missed it because the file's name and title contain
neither "review" nor "fable"; the semantic search's TOP hit was the design spec the review
produced, and the lineage was not chased one commit further — title-matching found the output,
not the source, the same error shape the IC ledger records for title-matched tag assignment.

The two documents are complementary units over one subject. The retrospective is TASK-shaped —
what to build; its T-1 audit trail went design → implementation → close-out within a day
(`3a422b31` → `40ab56f6` → `33fb28c9`), a same-day capture→mechanism loop that is itself
evidence for SR-1. This tracker is FINDING-shaped — what is true. Convergences, recorded so
neither document re-derives the other:

- SR-3 ≈ the retrospective's meta-insight (the observer is disqualified by construction; only
  standing mechanisms catch; the ~12% SDD wrong-ruling rate calibrates how far vigilance can be
  trusted).
- SR-7's classification pass is prescribed there as system-retrospective-improvements:T-5
  (archive back-tagging; 377/531 files untagged).
- SR-10 and SR-11 ≈ its "biggest structural liability" (machine-local silent-by-design state,
  id-by-path) plus system-retrospective-improvements:T-6 (two-representations-one-truth seams).
- SR-15 ≈ system-retrospective-improvements:T-3 — its "two products in one binary" (System A
  ~56K LOC tools vs System B ~56.5K LOC librarian) is the sharper, measured form of the
  tool-vs-methodology question.
- New here, absent there: SR-2 (meta-closure as the emergent property), SR-4 (eval as
  selection), SR-5 (metabolism derivations), SR-6 (asymmetric ratchet), SR-8 (recurrence
  unmeasured), SR-9 (synthetic independence), SR-12 (register co-evolution), SR-13 (live
  frictions).
- New there, absent here: the ~62% silent-failure signature ("a plausible value instead of an
  error" as the dominant species), the August capture step-change (299 bug files, 56% of the
  corpus), and the LOC split measurement.

## Index

| id | finding | status |
|---|---|---|
| SR-1 | The loop closes end to end, with live instances at every rung | validated |
| SR-2 | The emergent part is meta-closure — the system catches defects in its own defect-catching layer | validated |
| SR-3 | Prose does not heal; mechanisms do — and the working set is small | validated |
| SR-4 | The eval harness turns the corpus from memory into selection | validated |
| SR-5 | The metabolism: ~74% of commits touch docs; corpus ~1.4M words | validated |
| SR-6 | The ratchet is asymmetric: adding a rule is cheap, removing one is expensive | open |
| SR-7 | Autoimmune skew: many healed wounds are inflicted by the recording machinery itself | open |
| SR-8 | Prevention is unmeasured — no per-class recurrence rate exists | open |
| SR-9 | The independence is synthetic: Claude audits Claude | open |
| SR-10 | Path-derived identity is the strongest wrong-architecture candidate (convergent with TMR-4) | validated |
| SR-11 | Declare the catalog a rebuildable cache; every durable fix already moves that way | open |
| SR-12 | Register co-evolution: shared taste selects for ornament the eval measured as worthless | open |
| SR-13 | Live-session friction inventory — all five probed; 2 overclaimed, 1 rediscovery, 3 filed | probed |
| SR-14 | Ranked recommendations | open |
| SR-15 | The product-identity question: tool or methodology | open |

## SR-1 — The loop closes end to end, with live instances at every rung

**Status:** validated
**Valid:** dated 2026-09-01

Capture → classify → threshold → promote → mechanize, each rung verified live rather than from
its documentation:

- Capture: 25 open vs 521 archived bug files (`ls docs/issues/*.md | wc -l`, same for
  `archive/`) — ~95% of captured defects reach a terminal state.
- Classify: 17 claim-shaped defect classes in the IC ledger; membership is a re-runnable
  `git ls-files docs/issues` query, not a cell.
- Threshold: n≥3 across ≥2 subsystems, applied mechanically. IC-6 sat at n=2 until the archive
  was counted; the true count of 30 across five subsystems is what forced the rule.
- Promote: fixed routing table (OB / H / OP / CLAUDE.md / DC); IC-6 landed as CLAUDE.md
  § *Parsers Over a Namespace* (2026-08-31); IC-2, IC-3, IC-17 became OB-6, OB-7, OB-8
  (2026-09-01).
- Mechanize: `tests/issue_clusters.rs` gates the classification discipline in CI and caught two
  files within an hour of shipping; `tests/tool_reachability.rs` gates IC-3's largest family.

## SR-2 — The emergent part is meta-closure — the system catches defects in its own defect-catching layer

**Status:** validated
**Valid:** dated 2026-09-01

Three behaviors nobody designed as features, observed rather than claimed:

1. The IC ledger committed the premise-moved-conclusion-didn't defect twice inside the section
   that documents that defect, recorded both, and noted no instrument caught either.
2. The system generates its own error bars: the 2026-09-01 blind second read (redacted working
   copies, three independent readers, all 17 classes offered) returned 37/43 agreement with
   calibration measured by confidence tier, and published a pre-registered directional
   prediction that failed.
3. The reviewer was corrected by the system under review: the IL-3 gate refused this review
   session's own measurement command (a content-reader on a source file), twice.

The honest name for the whole is not "self-healing" but a **self-instrumenting system with a
promotion pipeline whose terminal rungs are mechanisms** — see SR-3.

## SR-3 — Prose does not heal; mechanisms do — and the working set is small

**Status:** validated
**Valid:** invariant

The corpus's own measurement (CLAUDE.md § *Observer Blindness*): four instances of one class in
one evening, every author actively writing about that class — knowledge prevented none, a
standing mechanism caught one. The part of the system that demonstrably changes behavior when
nobody is paying attention is roughly fifteen mechanisms: the four-command gate and its
load-bearing order, the CI tests over frontmatter, the companion hooks and IL gates, server-side
entry-id allocation that writes the heading itself, patch-id citation, move-grafts-history.

Two shape rules worth extracting (the review's most portable ideas):

- **Every successful fix turns an obligation into a side effect of an action people already
  take.** Every chronic defect is an obligation that survives as a manual step (citation sweeps
  after moves, refresh cycles, count re-derivation). The backlog is literally the list of
  not-yet-absorbed manual steps.
- **State that cannot be derived from the repo is where the defect classes live** — every
  double-digit IC class is at bottom one consumer reading a different place than the information
  lives. See SR-10, SR-11.

## SR-4 — The eval harness turns the corpus from memory into selection

**Status:** validated
**Valid:** dated 2026-09-01

Without prompt-engineering (prompt-tdd), the system had variation and heredity but no selection —
prose could be added, never killed on evidence. The harness supplies selection, with verdicts in
both directions (memory `research/loadbearing-mcp-guidance`, audit log A-3→A-7):

- Measured inert, then refused: persona preamble, "sacred channel" framing, in-band trust
  markers, the `<codescout-guide>` delegation envelope. A corpus that can *refuse growth*.
- Measured load-bearing, then shipped: placement + server-computed structure; the
  `untrusted-content` guide; provenance envelope keys green-lit at KEY-PRIORITY 6/6 — and
  observed live in this session's `artifact(get)` responses, two months after the memory
  recorded them as "green-lit, not yet built."
- Headline transferable finding: **trust rides the channel, never a marker the content carries
  about itself.**

Limits the harness itself records: single-turn ceiling (decay/persistence escape it), small N,
pinned-model sensitivity, and coverage — the big CLAUDE.md laws are not under arms. The
harness's own construction re-derived the Testing Discipline laws independently for a prose
substrate (prompt-surface-measurement-session-log:F-29, prompt-surface-measurement-session-log:W-19 and prompt-surface-measurement-session-log:W-24), which is evidence the
method transfers.

## SR-5 — The metabolism: ~74% of commits touch docs; corpus ~1.4M words

**Status:** validated
**Valid:** dated 2026-09-01

Derivations (re-run these, do not cite the values):

```
git log --since=2026-08-01 --oneline | wc -l                     # 1936 total
git log --since=2026-08-01 --oneline -- docs | wc -l             # 1436 touch docs (74%)
git log --since=2026-08-01 --oneline -- src crates tests | wc -l # 752 touch code (39%; overlap)
cat docs/trackers/*.md | wc -w                                   # 614,089
cat docs/issues/archive/*.md | wc -w                             # 719,366
wc -w < CLAUDE.md                                                # 6,713 (~9-10k tokens/session)
```

Per-session fixed tax: CLAUDE.md + session hooks + guide auto-injections land tens of thousands
of tokens before the first task action. The cost is real and the operator already felt it; this
entry exists so the debate has a denominator.

## SR-6 — The ratchet is asymmetric: adding a rule is cheap, removing one is expensive

**Status:** open
**Valid:** conditional — a pruning mechanism with teeth lands (budget gate, eval-gated retention)

Adding a rule costs three anecdotes and a commit. Removing one costs an eval run with real
dollars, pre-registration, and n≥10. Consequence: the corpus grows until context pressure forces
compaction, and compaction sessions themselves generate tracker entries. CLAUDE.md's only gates
are two tool-name tests; its growth is monotone under widening — the same blindness class its
own Testing Discipline section documents for assertions. Nothing budgets the file.

## SR-7 — Autoimmune skew: many healed wounds are inflicted by the recording machinery itself

**Status:** open
**Valid:** dated 2026-09-01

A visible (unquantified) share of the defect corpus is about citation grammars, ledger
addressing, catalog/disk seams, id allocation, augmentation loss, gate proxies — injuries of
being the kind of system it is. IC-6's 30 members are mostly the librarian's own addressing
machinery. Genuine product-bug classes (IC-13, IC-15, parts of IC-3 and IC-14) share the corpus
with a large meta-fraction. **Nobody has run the meta-vs-product classification pass; the ratio
is one tagging sweep away and this entry is falsifiable by it.** The 2026-09 retrospective
prescribes exactly that sweep as system-retrospective-improvements:T-5; when it runs, this
entry's ratio falls out of it. Mitigating context: the repo is
dogfooding its own substrate, so bookkeeping frictions double as product bug reports — the skew
is partly the point, but only partly, and only here.

## SR-8 — Prevention is unmeasured — no per-class recurrence rate exists

**Status:** open
**Valid:** conditional — a recurrence metric ships

What is measured: capture rate, classification agreement, promotion throughput, mechanism
shipping. What is not measured anywhere: whether a class's instance rate drops after its
mechanism ships. IC-16 noticed this for one rule ("the third instance buys measurability, not a
rule"); the observation generalizes to every promoted rule. Given the cluster tags, the metric is
one query away: instances per class per month, split at mechanism-ship date. Until it exists,
"self-healing" is a well-instrumented hypothesis — the headline claim rests on the
least-measured link.

## SR-9 — The independence is synthetic: Claude audits Claude

**Status:** open
**Valid:** conditional — an external reader participates

The blind second read was Claude reading Claude — same model family, same priors, same stylistic
attractors, redaction notwithstanding. The 86% agreement partly measures shared bias rather than
ground truth. Classification, promotion, and verdicts are single-operator + single-model-lineage
throughout. The system records this honestly ("still single-party classification") but cannot fix
it alone. Bound every agreement figure accordingly. Corollary the reviewer owes the record: a
model evaluating the system that produces the most interesting context it works in has a
conflict of interest, and this review is not exempt.

## SR-10 — Path-derived identity is the strongest wrong-architecture candidate

**Status:** validated (convergent)
**Valid:** invariant

`id = sha256(abs_path)` makes archiving — a bug file's *normal end state* — an identity-destroying
event by construction. The compensating machinery it forced into existence: move-grafts,
`id_changed` response protocol, manual citation sweeps (25 dangling refs from three moves, one
failing CI on a release tip), the `--include` filter that silently missed shell scripts, and the
rule not to cite 16-hex ids for anything likely to move. A minted immutable id in frontmatter
deletes the class.

**Convergence:** TMR-4 in `docs/trackers/tracker-management-redesign.md` accepted exactly this on
2026-07-17 ("identity decoupled from path", binding on new design) from independent survey
evidence. This review adds one thing TMR does not say: **the compensating machinery is good
enough that it masks the pressure to do the migration** — every seam bug gets caught, filed,
classified, and worked around, which lowers the felt urgency of removing the seam. The immune
system working against the organism.

## SR-11 — Declare the catalog a rebuildable cache; every durable fix already moves that way

**Status:** open
**Valid:** conditional — the cache-only target is declared and the burn-down list exists

The catalog is machine-local, gitignored, and holds authoritative state the repo doesn't
(augmentation params, edges, reservations). A large fraction of the bug corpus is the seam
(measured on a 437-commit pull: 21/23 memories invisible, 697/1117 edges absent, 22 augmentations
gone). Every fix that stuck moved authority from catalog to repo: `entry_prefix` → frontmatter,
high-water counters → frontmatter, augmentation shape → committed sidecars. The trend line has an
undeclared endpoint: **the catalog holds nothing authoritative that isn't derivable from the
repo.** Name the target explicitly, enumerate the remaining violations as a burn-down list, and
stop discovering them one bug file at a time. (TMR-3's push-based-maintenance evidence —
`refresh_count=0` on 21/23 augmented trackers — also lives on this axis: `freshness: unknown` on
every artifact this review touched.)

## SR-12 — Register co-evolution: shared taste selects for ornament the eval measured as worthless

**Status:** open
**Valid:** dated 2026-09-01

The corpus's register — recursive self-reference, aphorisms-with-a-body-count, reversal
narratives, confession-as-rigor, naming, ornamental datestamped precision — is selected for by
the *shared* enjoyment of operator and model, and there is no party in the loop with different
taste. The system's own eval found style and in-band authority buy nothing measurable; placement
and structure buy everything. Specific honest observations worth keeping:

- Self-reference feels like depth whether or not it prevents anything (Hofstadter candy).
- Confession is cheap for a stateless agent: the "I" that confesses dissolves at session end;
  the next session inherits the virtue signal without the sting.
- The archive selects for entries with a twist, because those are satisfying to write up.
- A datestamped measurement is, in part, an in-band authority marker — the thing the eval
  found worthless.
- **The defense is real: the pleasure is the funding mechanism.** A dry version of this system
  would have died of neglect; the enjoyment recruited the effort that built the ~15 mechanisms
  that work. It is the metabolism's appetite — but appetite has no stop signal, which is the
  argument for pointing the eval at the enjoyed parts first.

## SR-13 — Live-session friction inventory

**Status:** open — **all five probed 2026-09-01, same session. Two were overclaimed, one is a
rediscovery, three are now filed bug files.**
**Valid:** dated 2026-09-01

Hit first-hand during this review session, then **verified at the bytes rather than published
from session memory.** The probing changed three of the five, which is the entry worth reading
for method rather than content: a friction noticed in passing is a hypothesis, and the review
that records it is not the pass that establishes it.

| # | as first written | after probing | disposition |
|---|---|---|---|
| 1 | guide injection grain | **rediscovery** — already filed, Phase 1 shipped | cite, don't re-file |
| 2 | "3 calls to read a tracker section" | **overclaimed** — 1 call when the section fits; the real defect is the hint | filed |
| 3 | "IL-3 granularity" | **misattributed, then twice corrected** — two gates, both defective | filed |
| 4 | "read-refusal is paternalistic" | **understated** — a real bug with a documented contradiction | filed, high |
| 5 | freshness unpopulated | **confirmed, not new** — it is TMR-3 | no new file |

**1 — Guide injection grain: REDISCOVERY.** Filed as
`docs/issues/2026-08-27-guide-topics-are-atomic-nodes-in-an-unmodelled-graph.md`
(`7579b32b1cd2362f`, open), with Phase 1 — section-grain `get_guide` — **shipped 2026-08-27**
and a distribution probe run 2026-08-31 (n=166 sessions). Per the activation bootstrap, a
rediscovery is marked KNOWN and cites the ledger rather than opening a second file.

What this session adds is one cheap measured increment the bug file does not carry:
`serves:`-annotation coverage across the guide corpus, counted 2026-09-01 —
`src/prompts/guides/librarian.md` has **13** annotations at 3,097 words;
`src/prompts/guides/tracker-conventions.md` has **0** at **5,829 words**, the largest guide in
the corpus and the one that auto-injects on the first `artifact` call of any session. Eight of
the ten guides have zero. The mechanism is shipped and the biggest consumer is unwired — so
this is annotation work against an existing mechanism, not a code change, and it is
independent of both of Phase 2's blockers. Distinct from the bug file's own outstanding item
(*"`tracker-conventions` is really six topics"*), which it explicitly labels an authoring
judgement needing re-costing.

```
for f in src/prompts/guides/*.md; do printf "%s serves | %s words | %s\n" \
  "$(grep -c '<!-- serves:' "$f")" "$(wc -w < "$f")" "$f"; done
```

**2 — Read-path indirection: OVERCLAIMED, and the correct form is narrower.** The original
entry said a tracker section read costs three hops. It does not. A heading-scoped read returns
inline **in one call**, body included — verified on
`artifact(get, id=1b5a080fe2efcb6b, heading="## How a cluster becomes a rule")`, which came
back with `body` and `body_meta.bytes: 1595`. The three-hop chain only appears when the
*section itself* exceeds the inline budget (`## Index` is 25,531 bytes), where buffering is
correct behaviour and not a defect.

What survives is a real, small defect: on a buffered heading-scoped `get`, the envelope's
`hint` names `json_path="$.preview.headings[*]"` — the heading map — when the caller passed
`heading=` and wants `$.body`. Following the hint costs a wasted call. Filed:
`docs/issues/archive/2026-09-01-heading-scoped-get-overflow-hint-points-at-metadata.md`.

**3 — "IL-3 granularity": MISATTRIBUTED, and the correction was itself corrected.** Two
distinct gates fired four times this session, and the original entry blamed one label for
both. First correction: only the **source-file** gate has the scope defect, because a pipeline
genuinely is one command. **That correction was wrong too**, overturned by a probe one call
later — `echo pipe-scope-probe-ran; find . -name '*.toml' | head -2` refused with the `echo`
never running, so the pipe gate evaluates its per-pipeline predicate over the whole submitted
string as well. Both gates share one bug; they differ in that the pipe gate already **names
the offending clause** and the source gate emits only `"shell access to source files is
blocked"`. Filed:
`docs/issues/2026-09-01-source-gate-refuses-the-whole-compound-command.md`.

The method note is the point: **two successive framings of this friction were wrong, and both
were reasoned from the gates' own error text.** The probe cost one call each time. An error
message is authoritative about its predicate and says nothing about its evaluation scope.

**4 — Read-refusal on managed ledgers: UNDERSTATED. This is the session's most valuable
finding, and the original wording would have buried it.** Filed high-severity as
`docs/issues/archive/2026-09-01-artifact-create-stamps-an-id-that-guard-locks-the-file.md`.

`artifact(action="create")` writes `id: '<16-hex>'` into frontmatter; `librarian_guard`'s
`stamped` predicate (`src/util/librarian_guard.rs:95`) reads that as "the librarian owns this
file" — so **every artifact the tool creates is permanently refused by `edit_markdown` /
`read_markdown`**, whatever its `kind`, augmented or not, ledger or not. Verified by contrast
in one session: `docs/TEAM-ONBOARDING.md` (created via the tool, `kind: doc`, plain prose)
refused; `CONTRIBUTING.md` and `README.md` (catalog rows, indexed not created, no frontmatter)
edited fine. Membership is not the trigger — the stamp is.

It contradicts a ruling that was tried, probed and reverted (`bb9a94d7`), and it converts files
into the case the guard's pinned test
(`a_catalogued_but_unaugmented_file_stays_directly_editable`) exists to forbid. The test cannot
see it: its sample contains no *created* artifact — the recording-filter law, and here widening
the sample is the fix. And it was **masked until a repair**: the guard learned to read quoted
ids (archived 2026-08-16), and `create` emits the quoted form, so making the guard correct on
its stated axis switched the defect on across the whole created population.

The original SR-13 wording — *"blocking `read_markdown` protects only against the read→edit
reflex"* — argued about whether a **policy** was too strict, having never asked why the file was
guarded at all. It is the shape `CLAUDE.md` § *Observer Blindness* prescribes a probe for, and
one `head -4` answered it.

**5 — Freshness machinery live but unpopulated: CONFIRMED, and NOT new.** `freshness:
"unknown"` and `refreshed_at_commit: null` on every artifact this session touched. Already
owned by TMR-3 in `docs/trackers/tracker-management-redesign.md` (accepted 2026-07-17:
push-based maintenance, evidenced by `refresh_count=0` on 21 of 23 augmented trackers
ecosystem-wide). No new bug file; the observation is a datapoint on an accepted requirement.

**A sixth thing, found by working the other five — the ledger's own counts had drifted, and the
gate caught it rather than a reader.** Filing three bug files bumped three cluster counts.
`tests/issue_clusters.rs::every_index_count_matches_the_corpus` failed on the stale Index table
and printed the re-derivation to run. Reconciling it exposed a second, pre-existing drift: the
Index rows read 20 / 15 / 10 while the `**Members:**` lines of the same three classes still
read 19 / 14 / 9 — a count moved, the judgements quoting it did not, which is the defect that
section's own preamble documents at length. Corpus truth was 21 / 16 / 11; all six were
re-derived in one pass. **The mechanism, not the vigilance, is what caught this** — nobody was
looking at the `**Members:**` lines, and the gate does not check them, so the CI failure over
the table is what put a reader in front of them.
## SR-14 — Ranked recommendations

**Status:** open
**Valid:** dated 2026-09-01

1. **Put CLAUDE.md under the selection pressure everything else gets**: a size budget with a
   gate, eval arms for its two largest sections (does Testing Discipline's prose steer behavior
   better than a table a third its size?), compaction of measured-inert style into structure.
2. **Migrate to minted ids** (execute TMR-4). One-time cost; kills the largest recurring defect
   seam permanently. The corpus has paid the price of not doing it several times over.
3. **Declare the catalog a cache** (SR-11) and burn down remaining authoritative state.
4. **Measure recurrence per class** (SR-8) — the one number that would tell everyone whether the
   apparatus works.
5. Small: section-grade injection for tracker-conventions; allow reads on managed ledgers;
   per-clause IL-3 evaluation.

## SR-15 — The product-identity question: tool or methodology

**Status:** open
**Valid:** conditional — the operator decides and records the decision

If the product is codescout-the-tool, the current allocation (SR-5) is hard to defend — users
never see the difference between 17 IC classes and 5. If the product is the methodology — a
working study of how a stateless agent plus a repo becomes a learning system, with codescout as
substrate — the allocation is fine and the archive is the deliverable, but then the exit
criterion is a **write-up**, not a roadmap. The system currently behaves like the second while
describing itself as the first, and that ambiguity is what lets every meta-session bill itself
as product work. The teammate onboarding document (see below) is, in effect, the first
installment of the write-up.

## References

- IC ledger: `docs/trackers/issue-clusters.md` (IC-6, IC-16, the blind second read)
- Observer blindness: `docs/trackers/observer-blindness.md` (OB-6, OB-7, OB-8)
- Redesign requirements: `docs/trackers/tracker-management-redesign.md` (TMR-3, TMR-4)
- Eval evidence: memory `research/loadbearing-mcp-guidance`;
  `docs/trackers/prompt-surface-measurement-session-log.md`;
  `docs/research/2026-07-03-mcp-guidance-findings.md`
- Enforcement layer: `tests/issue_clusters.rs`, `tests/tool_reachability.rs`
- Cross-machine seams: `docs/conventions/cross-machine-catalog-resume.md`
- Sibling review (task-shaped twin): `docs/trackers/system-retrospective-improvements.md`

## Template for new entries

```
## SR-N — <claim-shaped title>

**Status:** open | validated | refuted | promoted
**Valid:** invariant | dated YYYY-MM-DD | conditional — <event>

<finding, with derivation for any figure>
```
