---
id: '1c3d87cc950190bd'
kind: convention
status: active
title: Testing discipline — what a green suite is evidence for, the laws in full
owners:
- marius
tags:
- testing
- conventions
- mutation
topic: test rigor and what a green suite proves
---

# Testing discipline — the laws in full

`CLAUDE.md` § *Testing Discipline* holds each law in compact form. This page keeps their full wording, with the measurements and incident history inside each, moved here unchanged on 2026-10-03. The derivations behind the laws are in [`what-green-is-evidence-for.md`](what-green-is-evidence-for.md).
The gate above tells you how to get green. This tells you what green is worth. Every derivation,
measurement, date and superseded formulation →
[`docs/conventions/what-green-is-evidence-for.md`](docs/conventions/what-green-is-evidence-for.md).

There is deliberately **no count of these laws** — a tally of the section's own contents is a
premise that every addition falsifies.

- **A test cannot detect a change its assertion is MONOTONE under.** Absence assertions
  (`is_empty()`, `!exists()`) are monotone under **removal** — a dead mechanism produces exactly the
  silence they assert. Existence assertions ("a region *containing* X is found") are monotone under
  **widening** — over-reporting satisfies them. Both look like guards, and neither fires in its own
  direction, so a property held by one of each is covered **zero** times, not weakly. Ask which
  direction each test is monotone under, and mutate the *other* way. (Measured `e6414362`: a locator
  widened to swallow its whole section killed **none of six** tests.)
- **A test cannot detect what its RECORDING filters out — the harder twin, because the standard
  remedy is a no-op against it.** The law above is about *members*: a population selected so no
  member can falsify. This one is about *observations*: the refuting outcome leaves **no artifact**.
  **"Widen the sample" fixes member-selection and changes nothing here, at any corpus size** — which
  is what makes it worse than a small sample, because the reflex answer looks responsive. Instrument
  the **doubt**, not the correction: when a re-derivation *confirms*, publish the confirmation. That
  is a **denominator**, never a catch — absorbing it as one makes the population look
  self-correcting.
- **Mutate once per guarded SITE, not once per feature.** A mutation run answers a question about
  one *line*; where a law is implemented at N call sites, one kill says nothing about the other N−1.
  (`doc(action="augment")`'s two shape-writing paths killed **different** tests, neither failing under
  the other's mutation.) **And the twin, about N guards at ONE site rather than N sites: a case only
  exercises the guard it NAMES if every OTHER guard admits its input.** Where several can refuse one
  input the first to refuse owns it, and the rest are vacuous *for that input* while the test name,
  its comment and a green suite all report coverage. Measured three times in two subsystems on
  2026-09-15/16, none visible to anything but a mutation: a test pair that killed only the
  *conjunction* of two bounds and neither bound; a third bound added later that silently un-guarded
  an older one by rejecting its cases earlier; and a disjunction where the case written for one guard
  was refused first by another. **TDD cannot reach the first of those** — the red is observed in the
  pre-fix state, which is the one configuration where both bounds are absent. So when writing a case
  for a bound, construct an input every *other* bound admits; and **after adding or changing any
  bound, re-run the mutation set for EVERY bound.** A verdict measures BYTES, so a
  behaviour-preserving refactor invalidates it too — derivations, SHAs and the corollary in
  [`docs/conventions/what-green-is-evidence-for.md`](docs/conventions/what-green-is-evidence-for.md)
  § *The sharpening*.
- **Loudness is a property of a PATH, not of a failure.** An alarm nothing reaches is exactly as
  informative as no alarm — `BL-66` *aborts the process* and survived anyway, because every in-tree
  caller installs the provider first. When adding a guard, alarm, error return or `panic!`, name the
  concrete caller that reaches it **and** the observer who acts on what it emits. The tell: ask what
  an observer would *see differently* if this were broken right now; if the answer is "nothing", it
  is decoration however loudly written. **The law reaches past guards, to features** — `ListFunctions`
  and `ListDocs` implemented the `Tool` trait, were registered nowhere, and carried a passing test
  suite for months while no agent could reach a line of it. The defect was the *tests*, not the
  tools. **And the twin, which the reached-alarm case does NOT cover: an alarm can fire, be read by
  exactly the right person, and send them somewhere useless — because a suite tests a guard's
  PREDICATE and never its REMEDY TEXT.** Every assertion is about *who is refused*; nobody writes
  one about *where the refusal sends you*, so that half is untested by construction and no mutation
  reaches it. Measured 2026-09-06: the `pre-push` foreign-session guard refused correctly on its
  first real use and its own text said *"ASK THE AUTHOR"* — a party who can report what they were
  told and **cannot grant**. Four sessions followed it and held for eight hours, each correctly
  refusing to decide what none had authority over; the pile grew 2 → 14 commits. It survived a
  54-assertion suite because every one of them was about the predicate. So when you ship a
  guard, **name the next action its message produces and ask whether that party can perform it.**
  **The remedy is untestable as PROSE and partly testable as SHAPE, and the difference is worth
  the line:** pinning sentences reds on every rewording and is rightly avoided, but asserting that
  the message still names *a second addressee* is cheap and reds exactly on the deletion. Measured
  the same day — removing the operator step from that guard killed **1** assertion, and a heavy
  rewrite of the surrounding prose that kept both addressees stayed **green**. It cannot tell you
  the remedy is *correct*, only that both steps survive; *"the sideways-only form cannot silently
  return"* is the whole claim, and it is the regression that actually happened.
  **And that shape test has a measured CEILING, found by using the guard rather than by reading it:
  naming both addressees does not check that the QUESTION asked of the first is answerable.** The
  text asked the author *whether it is withheld* — a binary — and on the guard's first real refusal
  (2026-09-07) the author's state was **neither branch: not withheld, and not cleared either**,
  because *"push only when the user asks"* is every session's standing instruction, so an author
  mid-task holds nothing to give. They replied *"do not wait on me"*, while the guard's closing line
  still read *"the author is the only one who can clear it"* — sending the reader to wait on a party
  who was waiting on a person too. A reader taking *"not withheld"* as clearance satisfies this
  guard and bypasses it in the same step. Both addressees were named, the predicate was right, and
  57 assertions were green. So **the shape test buys ARRIVAL and never ANSWERABILITY — ask what the
  addressee can reply, not only who they are**, and enumerate the states their answer can take.
  (`observer-blindness:OB-20`; raised by sessionId `4a2f34f7-0669-487d-9ce9-39b77881642f`, who
  then retracted their own *"there may be no fix"* as over-stated toward giving up; the ceiling
  reported by the author it happened to, sessionId `8dba66b0-af4b-4cda-a333-54a0605b318e`.)
- **A count of a defect population must arrive with its unit or not at all.** Derive it, don't cite
  it: one population yielded four defensible numbers inside an hour, each the right answer to a
  different question — and near enough to each other that no reader would have queried any of them.
  **On a shared checkout it needs its INSTANT and its TREE as well — and the tree half is the one a
  stamp cannot buy.** A file:line citation list decays exactly like a peer count, but for **two**
  independent reasons and only one is churn. The other is that a sweep in flight makes the
  **worktree and HEAD disagree**, so two sessions reading correctly *at the same instant* still
  differ. A timestamp dates that ambiguity; `git grep <pattern> HEAD` removes it — which makes
  naming the tree **cheaper** than stamping the moment, not merely additional to it. **The tell that
  a corpus MOVED rather than a reader ERRED is line drift** between two otherwise-agreeing readings:
  `:277` against `:278` sat in front of two sessions who each read it as the other's miscount.
  Measured 2026-09-07 — one citation list yielded four disagreeing readings inside ten minutes,
  three resolved by someone reaching for *"they made a mistake"* before *"the corpus moved"*, and
  the fourth moved toward **correct** while a hazard report about it was being written. **And count
  the LIST, never the corpus:** a headline derived from the tool beside an enumeration derived from
  the eye reconciles nowhere, and four sessions shipped that exact mismatch in one morning — the
  last of them thirty seconds after reading a retraction of it, which is this section's own claim
  about what knowing a class is worth. (Attribution, split because the halves were earned
  differently and the credited party asked that it read that way: the decay claim and *count the
  LIST* are sessionId `4eac25ba-b181-4dac-a5a1-ec88502a5bc5`'s. **The line-drift tell is theirs too
  but as HINDSIGHT** — it sat in front of two sessions unread, and they named it only after the
  retraction that produced it, so crediting it flat would read as foresight it was not. The
  two-reasons half, that a stamp cannot buy the tree, is `8dba66b0-af4b-4cda-a333-54a0605b318e`'s,
  made against a draft of mine that stopped at the stamp and would have failed exactly where the
  day's confusion lived.)
- **Annotate a fixture's load-bearing detail, on the fixture line.** Say what breaks if the detail
  goes — not in the test name, not in the assertion message, never a bare "do not edit". A tidy-up
  that removes it leaves the test passing and no longer discriminating, which no assertion can catch
  because that change is monotone too. **And annotate an inert fixture as inert**, so nobody credits
  it with coverage it does not provide: one direction guards against silent **removal**, the other
  against silent **credit**, and false coverage is the one that stops the next person looking.
- **An assertion computed over a POPULATION cannot verify a claim about a MEMBER, and the laws
  above will not catch it** — they are about the *direction* an assertion is blind to; this is about
  the **scope** it is computed over. A per-member claim checked against an aggregate is vacuous for
  every member and reads as coverage. **Two aggregates can be worse than one** when both are
  satisfied in the same wrong direction: a byte-ceiling test paired `total <= CEILING` with
  `total > 0` over six contributors, so content silently vanishing made the first *more* comfortable
  and was absorbed by the second. **And the per-member fix is not automatically the remedy** — a
  per-member assertion is only as good as the member's ability to reach the failing value, which a
  deliberate fallback floor can make unreachable; `bytes > 0` per shape was still green because a
  shape whose section is gone receives a 491-byte fallback rather than nothing. So **demand an
  observed RED, never an assertion's existence** — and **mutate the PRODUCTION path, not the
  test's inputs**: a second level asserting about its own re-implementation is indistinguishable
  from coverage until you break the thing that ships. (One such guard survived its own detector
  being disabled — 24 green — while its fixture re-typed the matching loop it claimed to share.) And where a system
  already names its own failure state, **assert on the name, not on a proxy for it**: the
  discriminator is usually already in the output, unused, while both parties reach for a number.
  (Two remedies falsified before the third worked, measured 2026-09-02 →
  [`docs/conventions/what-green-is-evidence-for.md`](docs/conventions/what-green-is-evidence-for.md).)
  **And ask whether the population is CLOSED, because an assertion can be per-member-adequate when
  written and become an aggregate later — with no edit to it, to the code it guards, or to its
  fixture.** Every instance above is a set fixed at authoring time, which an author could in
  principle have enumerated; this one cannot be caught that way, because the members did not exist
  yet. Measured 2026-09-16: a check's `v.is_empty()` over *every check a scan emits* was exact when
  one check existed — "the report is empty" and "my check is silent" picked out the same set — and
  a second check silently widened its subject. No diff to review, no carelessness, and re-reading
  the test returns a true sentence. **`is_empty()` over a growing collection is a claim whose
  meaning changes without its text changing, so where the collection is open-ended, name the
  member** — and note this is the *scope* law crossed with the *monotone* one, not the
  guard-ordering twin above: nothing races to refuse an input here, one predicate simply quantifies
  over a set that grew.
  **Do not hand-roll the mutation on this checkout — `./scripts/mutation-probe.sh` exists, and
  the reason is not convenience.** A mutation in the shared tree publishes a red to every other
  session's `cargo test`, byte-identical to a real regression, and the window is not bounded by
  your own process: `cargo test` returning *is* the shared build lock freeing, so a queued peer
  is aimed at the instant your revert runs. The script mutates an isolated worktree instead —
  measured 87 s + 2.8 G once, then **11 s per run, faster than the shared tree** — asserts the
  pattern occurs exactly once (a mutation that never applied is indistinguishable from one that
  survived), and reverts before its process exits. **Read `SURVIVED` as three readings with
  opposite repairs:** untested (write the test); *unreachable by any test you could write* (the
  code needs a seam first); or **semantically inert** — reachable and tested, but unable to be
  false on any input the FIXED code produces, because a sibling repair removed its domain, so
  the clause is dead and the repair is to delete it. The natural reading is the first, and on
  both of the others it sends you to write a test that cannot exist. **The third has a
  discriminator, which is what makes it a category rather than a guess about intent: revert the
  sibling fix and re-run the SAME mutation — if it now KILLs, the clause was inert.** Inertness
  is a property of the PAIR of sites, so the one-mutation-per-SITE law above is what surfaces
  it, paying out in reverse: a survival at one site explained by the fix at another. And the
  claim it falsifies is the one a bug file naturally writes — **independence of causes does not
  transfer to independence of repairs** (measured 2026-09-16, `fbddd86d`; the author had derived
  the independence by reading, which cannot separate them). Index row and every caveat →
  [`docs/PROBES.md`](docs/PROBES.md).
- **A red is evidence for the assertion that PRODUCED it and for no other — least of all its own
  replacement.** The laws above are about what an assertion cannot DETECT; this is about what a
  red licenses you to believe about a DIFFERENT one, and the two come apart at a single site.
  Rewriting an assertion after observing its red silently discards that evidence: the new text
  has never been observed failing, the suite is green, and the red-then-green ritual has been
  performed in full, so nothing anywhere is shaped like a gap. Measured 2026-09-16
  (`tests/mutation-probe.sh` case 21): two needles went red pre-fix, and one was then rewritten
  **toward this section's own prescription** — from a phrase to the ENTITY it names, the form
  that reds on deletion and survives rewording. The rewrite was **vacuous**: a sibling sentence
  of the same message satisfied it, so the clause it guarded could be deleted whole and the
  assertion stayed green — `50 passed, 0 failed`, SURVIVED its own mutation — while re-reading
  it returned a true sentence. Only re-running the mutation separates that from coverage.
  **So re-observe the red after ANY edit to an assertion, including — especially — one made to
  improve it:** an improvement is exactly when the old red feels most transferable, and the TDD
  cycle has no step that asks. Derivation →
  [`docs/conventions/what-green-is-evidence-for.md`](docs/conventions/what-green-is-evidence-for.md)
  § *A red does not survive its assertion being edited*.
