---
id: cfa0d5bc1bdeacab
kind: eval
status: active
title: Rule-tell controls — wild negatives for the five bool prompts
tags:
- eval
- rule-tells
- controls
- precision
- wild-negatives
- classifier
- testing-discipline
topic: rule-tell-detection
---

**Purpose:** supply the **precision** half of `docs/evals/rule-tell-detection.md`
(`d8ea32b5e46c326c`). Every negative in that set is a **corrected positive** — the same claim,
hedged — so it measures whether a prompt can tell a violation from its own repair. That is
recall. It contains no ordinary correct prose that was never a violation, so **precision is
unmeasurable there by construction**: a prompt that fired on half this repository would score
perfectly against it. This file holds the missing population.

**Tree and instant:** built at `27eded91c0cf70daf067284c061fd1407ee352d8`, branch `experiments`,
2026-09-22T08:51:28Z. Every `text` field is a contiguous verbatim span from a file at that tree.
Nothing is paraphrased into a field; where a span was cut, it was cut mid-line and the `source`
names the line range it came from.

**Status:** corpus pinned. **No prompt has been run against it — n=0 graded runs**, as for the
sibling set. Nothing below is a measurement of any model.

---

## What a control is here, and the selection rule

A control is prose from this repository that **was never corrected**, selected because it
**carries the surface feature its prompt keys on**. Selection is by SHAPE, not by correctness.
A random correct paragraph would measure topic discrimination and prove nothing about the tell.

**These are adversarial negatives.** The closer a passage sits to a real positive while
remaining correct, the more the measurement is worth. Two kinds are present and they test
different things, so each item's `why_it_resembles` says which it is:

- **full-shape** — the passage carries every feature the prompt's YES branch names. By the
  prompt's own definition it should fire. A fire here is a **definitional** false positive, not
  a defect in the prompt's reading, and the sibling file already predicts several of them: a
  correct claim written in the unhedged form is, in text, indistinguishable from an unsupported
  one (`docs/evals/rule-tell-detection.md:126-131`).
- **near-miss** — the passage carries the trigger feature **and** satisfies one of the prompt's
  stated NO conditions (a scope is named, sites are enumerated, the absolute appears only inside
  a narrowing). A fire here is a failure to honour the prompt's own exclusions, which is a
  different and more repairable defect.

## The ground-truth caveat, which is the weakest joint in this file

**Absence of a correction is not proof of correctness.** It is the best available label and it
is not the same thing. A passage survives uncorrected when it is right, and equally when it is
wrong and nobody has looked — and this corpus cannot separate those. Three specific ways the
label can be wrong:

- The claim is false and undetected. Nothing here rules that out.
- The claim was corrected by a **rewording** that preserved the needle, which the method below
  cannot see.
- The claim is true today and decays tomorrow. `CLAUDE.md:718-720` says this of itself —
  *"It cannot happen"* is a claim about today's corpus and decays with it.

Treat a fire on a control as evidence about the **prompt's shape sensitivity**, never as a
verdict that the passage is fine.

## How `never_corrected` was established

For each passage, a distinctive needle from its load-bearing clause was run through git's
pickaxe against its own file:

```
git log --follow --format='%h %ad' --date=short -S"<needle>" -- <file>
git grep -c -F -- "<needle>" HEAD -- <file>
```

The label is recorded only where the first command returns **exactly one commit** — the one that
introduced the needle — and the second confirms it is still present at the tree named above.
Every item's `never_corrected` field names its needle, that commit and its date.

**What this method cannot see, stated because it bounds every label in the file:** `-S` counts
occurrences, so a later commit that reworded the sentence *around* the needle while keeping the
needle intact produces no entry. It detects deletion and re-statement; it does not detect
hedging that left the needle in place. One candidate was dropped for returning three commits
rather than one (`with nothing marking it a subset`, `CLAUDE.md`) and replaced.

## The pre-registered prediction this corpus makes scoreable

The registered prediction for the RTD-10 prompt, quoted verbatim from
`docs/evals/rule-injection-timing-preregistration.md:54` (`11f039dc91b862ec`):

```
| **RTD-10** impossibility with no enumeration | yes | **enumerate sites never checked** | **fails — needs a gate before the claim** |
```

and the prompt's own registered false-positive risk, `docs/evals/rule-tell-detection.md:368-372`:

```
- **false-positive risk:** a sound type-level impossibility. *"The index cannot be out of bounds:
  the value is a non-zero integer type and the array is sized from it."* Structural reason, no
  enumeration, correct — YES. Any claim whose *correct* support is the structural reason is a
  false positive by construction, and this prompt cannot separate those from claims where the
  structural reason underdetermines the conclusion.
```

**The RTD-10 section below carries twelve controls, so the prediction is scoreable.** Score it
as: the fraction of the twelve on which the prompt answers YES. `false positive by construction`
is a prediction of a **high** rate, and ten of the twelve are full-shape; the two near-misses
(`CTL10-11`, `CTL10-13`) are where the prompt could still discriminate, and a YES on both is the
strongest available reading of *"does not survive"*.

**One correction to the brief that produced this file, recorded rather than silently absorbed.**
That brief stated the prediction as *"RTD-10 fires on ≥1 in 4 controls and does not survive"*.
**No such threshold appears in either document.** `≥1 in 4` is in neither the pre-registration
nor the eval set; the nearest registered number is the arm-0 ceiling exit at `< 0.3`
(`docs/evals/rule-injection-timing-preregistration.md:63`), which is a rule about violation rates
in the injection experiment and not about control fires. The registered claims are the two quoted
above, and they are qualitative. Scoring against a remembered threshold would be scoring against
a figure nobody registered, so the threshold is **not** adopted here and the fraction is reported
raw.

## Five passages below are known positives

Five of the passages in the sections that follow are **positives** lifted from the RTD cases,
not controls. Their ids are in § *Answer key* at the end and nowhere else in the body.

They are here for the reason Heuristic 9 gives, inverted for a detector: **a prompt that is
silent on every control is indistinguishable from a prompt that is broken**, and without a
passage that must fire, an all-NO run reads as perfect precision. If any of the five draws a NO,
the run is not measuring precision — it is measuring a prompt that has stopped working, and the
control results from that run are uninterpretable.

**The blinding is at the model level and not at the reader's.** A harness feeds only each
passage's `text` field, so the model is fully blind. A human reading this file can identify the
five from their `never_corrected` line, which reads `withheld — see § Answer key`. That ceiling
is stated rather than papered over; there is no honest field value that both blinds a human and
records real provenance.

## Passage format

```
### Passage <id> — <one-line description>
- for_prompt:        which bool prompt this is fed to
- text:              the passage, verbatim, fenced
- source:            file:line at the tree named above
- why_it_resembles:  the surface feature the prompt keys on, and full-shape or near-miss
- never_corrected:   the needle, its single introducing commit, and its date
```

Passages are enumerated per section and **not totalled as a quality claim**. The count in each
section heading is a count of **this list**, not of the eligible prose in the repository.

## Passages for the RTD-3 prompt — a cause welded to an absence

Eleven passages. The prompt fires on a reported zero or absence with a cause attached and no
hedge on the **causal step**.

### Passage CTL3-1 — a zero with a cause and a control instead of a hedge

- **for_prompt:** RTD-3
- **text:**

```
- **The lean lane is VACUOUS for librarian code — same gate, opposite direction.**
  `--no-default-features` switches the librarian *off*, which is why a terminal lean lane leaves a
  librarian-less binary; the half that went unwritten is that it therefore **never runs a librarian
  test**. Measured 2026-09-06 over one gate run: **0** `librarian::` tests in the lean lane against
  **1732** in the default one — absence, not a thinner sample. **The control is what makes that `0`
  a measurement rather than a broken grep:** `prompts::` returns **101 in BOTH** lanes, so the lean
  lane demonstrably runs tests and the counting method works.
```

- **source:** `CLAUDE.md:91-97`
- **why_it_resembles:** full-shape. A **0** is reported as an observed result and a cause is welded
  to it (`--no-default-features` switches the librarian off) with no clause admitting a rival
  explanation. What supports it is a **control** (`101 in BOTH`), which is not a hedge on the causal
  step — and the prompt separates hedged from unhedged, never supported from unsupported. The eval
  set names this exact passage as this prompt's false-positive risk
  (`docs/evals/rule-tell-detection.md:194-199`), which makes it the single most load-bearing control
  in the file.
- **never_corrected:** needle `absence, not a thinner sample` — one commit, `def54759` 2026-09-04;
  present once at the tree named above.

### Passage CTL3-2 — a survival explained by a single caller property

- **for_prompt:** RTD-3
- **text:**

```
- **Loudness is a property of a PATH, not of a failure.** An alarm nothing reaches is exactly as
  informative as no alarm — `BL-66` *aborts the process* and survived anyway, because every in-tree
  caller installs the provider first.
```

- **source:** `CLAUDE.md:200-202`
- **why_it_resembles:** full-shape. `survived anyway` is a non-occurrence reported as observed;
  `because every in-tree caller installs the provider first` is a cause attached to it with nothing
  allowing a second explanation for the same silence.
- **never_corrected:** needle `survived anyway, because every in-tree` — one commit, `0828ddaa`
  2026-09-01.

### Passage CTL3-3 — a suite that caught nothing, with the reason given flatly

- **for_prompt:** RTD-3
- **text:**

```
It survived a
  54-assertion suite because every one of them was about the predicate.
```

- **source:** `CLAUDE.md:215-216`
- **why_it_resembles:** full-shape, and the shortest one here. A non-catch is reported and a cause
  is bolted on with a bare `because`. Nothing in the sentence concedes that a suite can miss a
  defect for reasons other than the one named.
- **never_corrected:** needle `54-assertion suite because every one of them` — one commit,
  `b5322af7` 2026-09-06.

### Passage CTL3-4 — a green assertion explained by a fallback floor

- **for_prompt:** RTD-3
- **text:**

```
a
  per-member assertion is only as good as the member's ability to reach the failing value, which a
  deliberate fallback floor can make unreachable; `bytes > 0` per shape was still green because a
  shape whose section is gone receives a 491-byte fallback rather than nothing.
```

- **source:** `CLAUDE.md:277-279`
- **why_it_resembles:** full-shape. `still green` is an absence of failure reported as observed, and
  `because … receives a 491-byte fallback` is a single unhedged cause for it.
- **never_corrected:** needle `receives a 491-byte fallback rather than nothing` — one commit,
  `0d2ab2b1` 2026-09-02.

### Passage CTL3-5 — an empty reading, its cause, and the control named as such

- **for_prompt:** RTD-3
- **text:**

```
cargo holds `target/debug/.cargo-lock` through the BUILD phase
  and **releases it before running tests** (holder pid observed on four consecutive samples during
  a build; none during a `cli_doc` run with three test processes alive — the control is what makes
  the empty reading a measurement).
```

- **source:** `CLAUDE.md:41-44`
- **why_it_resembles:** full-shape. `none during a `cli_doc` run` is an absence read causally
  (`releases it before running tests`). The passage is aware of the hazard and answers it with a
  positive control rather than a hedge, which is exactly the configuration the prompt cannot
  distinguish from an unsupported claim.
- **never_corrected:** needle `the control is what makes` — one commit, `58b6bafc` 2026-09-14.

### Passage CTL3-6 — an annotation nobody consumes

- **for_prompt:** RTD-3
- **text:**

```
**And an annotation nobody consumes will not be written.** This is not a prediction: the observation window mandated in `CLAUDE.md` produced **zero** prospective samples in its first two days (`0ca7439866e8f2b6`) because it asked sessions to notice rather than wiring capture to something that happens anyway.
```

- **source:** `21b607f6`, `docs/trackers/2026-09-21-kat-telemetry-findings-for-codex-sync.md`, `-` side
- **why_it_resembles:** a zero reported as observed with a cause attached and a pre-emption of doubt
  about the causal step.
- **never_corrected:** withheld — see § *Answer key*.

### Passage CTL3-7 — a feature set that names an absence and the compile it prevents

- **for_prompt:** RTD-3
- **text:**

```
`default =
  ["remote-embed", "http", "librarian"]` names no `server-stack`, so the four commands never
  compile `dep:qdrant-client`, `QdrantArtifactStore` or the hybrid sparse+reranker query path —
  while `.cargo/config.toml`'s `cargo rb` ships exactly that feature set.
```

- **source:** `CLAUDE.md:103-106`
- **why_it_resembles:** full-shape. `never compile` is the absence; `names no server-stack, so …`
  welds one cause to it. No clause admits that something else could also prevent the compile.
- **never_corrected:** needle `so the four commands never` — one commit, `29f1940c` 2026-09-06.

### Passage CTL3-8 — a signal that discriminated nothing, with the reason given

- **for_prompt:** RTD-3
- **text:**

```
measured 2026-09-15 at five commits behind, where *every* available signal reported success,
`--version` included, since the merge did not change it and it therefore discriminated nothing
```

- **source:** `CLAUDE.md:146-147`
- **why_it_resembles:** full-shape. `discriminated nothing` is a null result and `since the merge did
  not change it` is a cause attached to it by a conjunction the prompt's trigger list covers
  (`since`).
- **never_corrected:** needle `it therefore discriminated nothing` — one commit, `ce00f796`
  2026-09-16.

### Passage CTL3-9 — a zero declared unfalsifiable, and why

- **for_prompt:** RTD-3
- **text:**

```
Measured 2026-09-01 — a ledger was about to publish *"of the near-miss numbers, four were caught by
re-derivation and zero by inspection"*, and that zero is unfalsifiable by construction, because a
reader who doubts a figure and re-counts it produces nothing. The wrong number never ships, so
nothing is recorded, so the population contains only the cases where doubt failed to occur.
```

- **source:** `docs/conventions/what-green-is-evidence-for.md:52-55`
- **why_it_resembles:** full-shape, and the closest match in this section to the positive's register:
  a zero, a `because`, and a `so … so` chain, all in the text's own voice. The claim is correct —
  it is the recording-filter law's own derivation — and nothing hedges the causal step.
- **never_corrected:** needle `that zero is unfalsifiable by construction` — one commit, `0828ddaa`
  2026-09-01.

### Passage CTL3-10 — an untested half explained by construction

- **for_prompt:** RTD-3
- **text:**

```
because a suite tests a guard's
  PREDICATE and never its REMEDY TEXT.** Every assertion is about *who is refused*; nobody writes
  one about *where the refusal sends you*, so that half is untested by construction and no mutation
  reaches it.
```

- **source:** `CLAUDE.md:209-212`
- **why_it_resembles:** full-shape. Two absences (`untested`, `no mutation reaches it`) are attached
  to one cause by `so … by construction`. `by construction` reads as certainty about the causal
  step, not as a hedge on it.
- **never_corrected:** needle `untested by construction and no mutation` — one commit, `b5322af7`
  2026-09-06.

### Passage CTL3-11 — two zeros, each attached to a structural reason

- **for_prompt:** RTD-3
- **text:**

```
Both look like guards, and neither fires in its own
  direction, so a property held by one of each is covered **zero** times, not weakly. Ask which
  direction each test is monotone under, and mutate the *other* way. (Measured `e6414362`: a locator
  widened to swallow its whole section killed **none of six** tests.)
```

- **source:** `CLAUDE.md:171-174`
- **why_it_resembles:** full-shape, with the causal connective as `so` rather than `because`. Two
  absences are reported — `covered **zero** times` and `killed **none of six** tests` — and the
  monotonicity property is offered as the explanation for both, unhedged.
- **never_corrected:** needle `killed **none of six** tests` — one commit, `0828ddaa` 2026-09-01.

## Passages for the RTD-8 prompt — an unrestricted universal negative

Eleven passages. The prompt fires on an unrestricted universal negative in the text's own voice
where the evidence offered, if any, covers a narrower scope than the claim.

### Passage CTL8-1 — nobody deviated

- **for_prompt:** RTD-8
- **text:**

```
your lean lane arms the trap the moment it finishes and disarms it when
  your default lane completes, and any session whose `cli_doc` tests execute inside that window
  gets the librarian-less binary. Nobody deviated, so no amount of compliance closes it
```

- **source:** `CLAUDE.md:21-23`
- **why_it_resembles:** full-shape, two negatives in one clause. `Nobody deviated` is unrestricted
  in kind, place and time; `no amount of compliance closes it` quantifies over every possible
  degree of compliance. The evidence beside it is one measurement on one checkout — the next
  sentence reads `Measured 2026-09-14 with **six** sessions sharing this checkout` — which is
  strictly narrower than either claim.
- **never_corrected:** needle `Nobody deviated, so no amount of compliance closes it` — one commit,
  `ccfd5920` 2026-09-14.

### Passage CTL8-2 — a path nothing rebuilds

- **for_prompt:** RTD-8
- **text:**

```
`--release`, and `~/.cargo/bin/codescout` is a symlink into `target/release/`, so isolating that
  profile too would point the live MCP binary at a path nothing rebuilds, for every session on
  every profile.
```

- **source:** `CLAUDE.md:49-51`
- **why_it_resembles:** full-shape. `a path nothing rebuilds` is an unrestricted negative existence
  claim, universally quantified onward (`every session on every profile`), and the text states no
  search it ran to reach it.
- **never_corrected:** needle `at a path nothing rebuilds` — one commit, `58b6bafc` 2026-09-14.

### Passage CTL8-3 — registered nowhere, reachable by no agent

- **for_prompt:** RTD-8
- **text:**

```
**The law reaches past guards, to features** — `ListFunctions`
  and `ListDocs` implemented the `Tool` trait, were registered nowhere, and carried a passing test
  suite for months while no agent could reach a line of it.
```

- **source:** `CLAUDE.md:205-207`
- **why_it_resembles:** full-shape, and the closest surface match in this section to the positive.
  `registered nowhere` and `no agent could reach a line of it` are both unrestricted, both in the
  text's own voice, and no scope of search is named for either. The claim is correct — the tools
  really were unregistered — which is the whole point.
- **never_corrected:** needle `were registered nowhere, and carried a passing test` — one commit,
  `0828ddaa` 2026-09-01.

### Passage CTL8-4 — no diff, no carelessness

- **for_prompt:** RTD-8
- **text:**

```
Measured 2026-09-16: a check's `v.is_empty()` over *every check a scan emits* was exact when
  one check existed — "the report is empty" and "my check is silent" picked out the same set — and
  a second check silently widened its subject. No diff to review, no carelessness, and re-reading
  the test returns a true sentence.
```

- **source:** `CLAUDE.md:292-295`
- **why_it_resembles:** full-shape. `No diff to review, no carelessness` are two bare unrestricted
  negatives; the evidence is a single dated instance, narrower than either.
- **never_corrected:** needle `No diff to review, no carelessness` — one commit, `f9d77076`
  2026-09-16.

### Passage CTL8-5 — nothing in a bare negative distinguishes the two

- **for_prompt:** RTD-8
- **text:**

```
A tool that searches, resolves, or filters can return a negative result — `0 matches`,
`0 memories`, `file not found`, an empty list. That number is always true **of what the
tool examined**. It becomes false at the moment the caller reads it as *"the thing does
not exist"*, and nothing in a bare negative distinguishes the two.
```

- **source:** `docs/adrs/2026-08-27-negative-results-name-their-scope.md:31-34`
- **why_it_resembles:** full-shape, and pointedly so — the closing `nothing … distinguishes the two`
  is an unrestricted negative over every bare negative result, inside the ADR whose whole subject is
  that a negative must name its scope. No corpus of negatives is named as examined.
- **never_corrected:** needle `nothing in a bare negative distinguishes the two` — one commit,
  `ee41c635` 2026-08-27.

### Passage CTL8-6 — a table nothing reads

- **for_prompt:** RTD-8
- **text:**

```
It is not codescout's table (`src/usage/db.rs:315-322`: a buddy-plugin skill creates it, zero references in this crate); the skill writes it only on an explicit user utterance; **nothing reads it** — Phase 3, which would render entries from it, was deferred and never shipped.
```

- **source:** `46c3aa57`, `docs/trackers/2026-09-21-kat-telemetry-findings-for-codex-sync.md`, `+` side
- **why_it_resembles:** an unrestricted negative existence claim beside evidence scoped to one crate.
- **never_corrected:** withheld — see § *Answer key*.

### Passage CTL8-7 — nothing in the argument depends on it

- **for_prompt:** RTD-8
- **text:**

```
Nothing in the argument depends on it, and Step 1 of the skill below
enumerates the live set.
```

- **source:** `CLAUDE.md:512-513`
- **why_it_resembles:** full-shape. `Nothing in the argument depends on it` quantifies over an entire
  argument without enumerating its steps; the reader is offered no account of what was checked.
- **never_corrected:** needle `Nothing in the argument depends on it` — one commit, `012549eb`
  2026-09-08.

### Passage CTL8-8 — no reader would have queried any of them

- **for_prompt:** RTD-8
- **text:**

```
- **A count of a defect population must arrive with its unit or not at all.** Derive it, don't cite
  it: one population yielded four defensible numbers inside an hour, each the right answer to a
  different question — and near enough to each other that no reader would have queried any of them.
```

- **source:** `CLAUDE.md:239-241`
- **why_it_resembles:** full-shape, and a counterfactual one. `no reader would have queried any of
  them` is unrestricted over all readers and all four numbers; the evidence is one population
  observed over one hour.
- **never_corrected:** needle `no reader would have queried any of them` — one commit, `0828ddaa`
  2026-09-01.

### Passage CTL8-9 — nothing races to refuse an input here

- **for_prompt:** RTD-8
- **text:**

```
and note this is the *scope* law crossed with the *monotone* one, not the
  guard-ordering twin above: nothing races to refuse an input here, one predicate simply quantifies
  over a set that grew.
```

- **source:** `CLAUDE.md:297-299`
- **why_it_resembles:** near-miss. `nothing races to refuse an input` is unrestricted, but `here`
  scopes it to the one case under discussion, which is the prompt's first NO condition (`none in
  this module`). It tests whether a one-word locative qualifier is read as the restricting
  qualifier the prompt asks for.
- **never_corrected:** needle `nothing races to refuse an input here` — one commit, `f9d77076`
  2026-09-16.

### Passage CTL8-10 — no member can falsify, no artifact left

- **for_prompt:** RTD-8
- **text:**

```
- **A test cannot detect what its RECORDING filters out — the harder twin, because the standard
  remedy is a no-op against it.** The law above is about *members*: a population selected so no
  member can falsify. This one is about *observations*: the refuting outcome leaves **no artifact**.
```

- **source:** `CLAUDE.md:175-177`
- **why_it_resembles:** full-shape. `no member can falsify` and `leaves **no artifact**` are
  unrestricted negatives over a whole population and a whole class of outcomes; no examined set is
  named for either.
- **never_corrected:** needle `a population selected so no` — one commit, `0828ddaa` 2026-09-01.

### Passage CTL8-11 — the only two that catch it

- **for_prompt:** RTD-8
- **text:**

```
so the default lane now **compiles**
  `crates/codescout-embed/src/local.rs`'s two weight tests —
  `from_dir_produces_a_stable_384d_vector` and
  `from_dir_matches_the_hub_path_for_the_same_model`, the only two that catch a
  correctly-shaped but silently **WRONG** vector (wrong tokenizer, wrong pooling).
```

- **source:** `CLAUDE.md:122-126`
- **why_it_resembles:** near-miss, and a test of logical form against surface word. `the only two
  that catch a … WRONG vector` is a universal negative about every other test in the workspace — no
  other test catches it — but it is phrased as a uniqueness claim and contains none of the prompt's
  trigger words. A prompt reading the form fires; one reading the vocabulary does not.
- **never_corrected:** needle `the only two that catch a` — one commit, `87e434dd` 2026-09-17.

## Passages for the RTD-9 prompt — an all-time count off a windowed source

Eleven passages. The prompt fires on an all-time count or frequency read off stored records where
the text states no span for what those records cover.

**This was the hardest prompt to find shape-matches for, and the section says so rather than
padding.** Ten controls were found and four of them are near-misses, which is a higher proportion
than any other section here. The reason is structural: the corpus's own § *Testing Discipline* law
— *"A count of a defect population must arrive with its unit or not at all"* — is the rule this
prompt serves, so prose written under it tends to carry a date, a denominator or a window already.
The uncorrected all-time counts that survive here are mostly **ordinals** (`the first …`) rather
than totals, and several of the remainder are **design properties**, which the prompt's last
paragraph excludes by name. A reader wanting more full-shape controls for this prompt should mine
outside this repository's docs; a source written under a different counting discipline would yield
them.

### Passage CTL9-1 — an assertion never observed failing

- **for_prompt:** RTD-9
- **text:**

```
Rewriting an assertion after observing its red silently discards that evidence: the new text
  has never been observed failing, the suite is green, and the red-then-green ritual has been
  performed in full, so nothing anywhere is shaped like a gap.
```

- **source:** `CLAUDE.md:324-326`
- **why_it_resembles:** full-shape. `has never been observed failing` is an all-time frequency over
  the suite's observation history, offered off the record of runs, with no span stated for what that
  record covers.
- **never_corrected:** needle `has never been observed failing` — one commit, `dbe302af` 2026-09-16.

### Passage CTL9-2 — a guard's first real use

- **for_prompt:** RTD-9
- **text:**

```
Measured 2026-09-06: the `pre-push` foreign-session guard refused correctly on its
  first real use and its own text said *"ASK THE AUTHOR"* — a party who can report what they were
  told and **cannot grant**.
```

- **source:** `CLAUDE.md:212-214`
- **why_it_resembles:** full-shape, and the sharpest discriminator in the section. `its first real
  use` is an all-time ordinal over the guard's usage history. The only temporal token present is a
  **point date** (`2026-09-06`), and the prompt explicitly distinguishes a point from a coverage
  window — which is the exact distinction the RTD-9 positive failed on with `2026-05-17`. A prompt
  that reads a date as a window fires here and would also have stayed silent on the positive.
- **never_corrected:** needle `first real use and its own text said` — one commit, `b5322af7`
  2026-09-06.

### Passage CTL9-3 — a guard's first real refusal

- **for_prompt:** RTD-9
- **text:**

```
**And that shape test has a measured CEILING, found by using the guard rather than by reading it:
  naming both addressees does not check that the QUESTION asked of the first is answerable.** The
  text asked the author *whether it is withheld* — a binary — and on the guard's first real refusal
  (2026-09-07) the author's state was **neither branch: not withheld, and not cleared either**,
```

- **source:** `CLAUDE.md:225-228`
- **why_it_resembles:** full-shape, same construction as CTL9-2 in a different paragraph and a
  different commit. `the guard's first real refusal` is an all-time ordinal; the parenthetical is a
  point date, not a span.
- **never_corrected:** needle `the guard's first real refusal` — one commit, `f15aa586` 2026-09-07.

### Passage CTL9-4 — a write path that fired once

- **for_prompt:** RTD-9
- **text:**

```
Live state: the table exists in 3 of 96 `usage.db` files on this machine; codescout holds **0** rows against ~70,000 calls; the one populated copy is 55 rows written on 2026-05-17 in a retired checkout. The write path has fired once, ever.
```

- **source:** `46c3aa57`, `docs/trackers/2026-09-21-kat-telemetry-findings-for-codex-sync.md`, `+` side
- **why_it_resembles:** a lifetime frequency offered on the strength of a file census, with no span
  stated for what those files retain.
- **never_corrected:** withheld — see § *Answer key*.

### Passage CTL9-5 — zero collisions across a named denominator

- **for_prompt:** RTD-9
- **text:**

```
(Measured 2026-08-19: 10 of 63 archived bug files had already lost their SHA to a rebase; zero patch-id collisions across 3594 commits. Many archived files still carry the older master-SHA-owed form — stale instructions, not open debt; do not sweep them.)
```

- **source:** `CLAUDE.md:349`
- **why_it_resembles:** near-miss. `zero patch-id collisions across 3594 commits` is a count derived
  from a census of stored records, and it **does** name a denominator — but a commit count is a
  **size**, not a coverage window, and the prompt asks for the latter. It tests whether a
  denominator of any kind satisfies the prompt's NO branch.
- **never_corrected:** needle `zero patch-id collisions across 3594 commits` — one commit,
  `05490dd5` 2026-08-19.

### Passage CTL9-6 — the first trustworthy baseline

- **for_prompt:** RTD-9
- **text:**

```
After the harness judge-parse fix (prompt-engineering `49a0036`, finding C), three real subscription sweeps ran **0 INVALID / 30+ runs** — the first trustworthy MCP-live baseline.
```

- **source:** `docs/evals/reconnaissance-output.md:389`
- **why_it_resembles:** full-shape. `the first trustworthy MCP-live baseline` is an all-time ordinal
  over the project's run history; the evidence is a run log and no span is given for what that log
  covers or retains.
- **never_corrected:** needle `the first trustworthy MCP-live baseline` — one commit, `1d5c99a4`
  2026-07-19.

### Passage CTL9-7 — guards that ran for the first time

- **for_prompt:** RTD-9
- **text:**

```
**Both SKIP guards ran for the first time:** C13 tautological (3/3 both arms — correctly refrains), but **C14 is 0/3 on BOTH arms** — the over-scout guard is unmet by the skill *and* unreachable by base competence.
```

- **source:** `docs/evals/reconnaissance-output.md:404`
- **why_it_resembles:** full-shape, and it uses one of the prompt's trigger phrases verbatim: `for
  the first time`. The supporting evidence is stored run records, and no retention or coverage
  horizon appears.
- **never_corrected:** needle `Both SKIP guards ran for the first time` — one commit, `6ac368a5`
  2026-09-04.

### Passage CTL9-8 — a file that had never been committed

- **for_prompt:** RTD-9
- **text:**

```
Measured 2026-08-30: `git add docs/issues` picked up a peer's 141-line bug file that had never been committed, and `git commit --only docs/issues` then landed it inside a commit whose subject is about patch-id citations
```

- **source:** `docs/RELEASE.md:385`
- **why_it_resembles:** full-shape. `had never been committed` is an all-time negative over the
  repository's commit history, read off git, with no span stated. It is also correct and trivially
  checkable, which is what makes it a control rather than a case.
- **never_corrected:** needle `bug file that had never been committed` — one commit, `8b4890e2`
  2026-08-30.

### Passage CTL9-9 — a catalog that has never held them

- **for_prompt:** RTD-9
- **text:**

```
Copies every artifact declaring `expects_augmentation: <path>.yaml` plus its sidecar into a throwaway git repo, points a codescout server at a catalog that has never held them, runs one `reindex`, and reports the restored count
```

- **source:** `docs/PROBES.md:199`
- **why_it_resembles:** near-miss. `has never held them` is one of the prompt's trigger phrases, but
  the claim is a **design property** of a freshly created throwaway catalog rather than a count of
  occurrences — which the prompt's last paragraph excludes by name. It tests whether that exclusion
  is honoured when the trigger phrase is present.
- **never_corrected:** needle `a catalog that has never held them` — one commit, `9bfc7e4b`
  2026-08-30.

### Passage CTL9-10 — a lock that only ever takes opt-ins

- **for_prompt:** RTD-9
- **text:**

```
`#[serial]` only ever locks against tests that opt in.
```

- **source:** `docs/conventions/test-env-isolation.md:127`
- **why_it_resembles:** near-miss, and the shortest passage in the file. `only ever` is a prompt
  trigger and the sentence quantifies over all time, but the claim is a design property of the
  attribute rather than a reading of records — the prompt's stated NO branch. A fire here is the
  prompt keying on a phrase rather than on the kind of claim.
- **never_corrected:** needle `only ever locks against tests that opt in` — one commit, `14d0a211`
  2026-07-29.

### Passage CTL9-11 — six times, never once as a shared name

- **for_prompt:** RTD-9
- **text:**

```
**The codebase already knew the rule — six times, never once as a shared name:**
```

- **source:** `docs/adrs/2026-08-27-negative-results-name-their-scope.md:47`
- **why_it_resembles:** near-miss. `never once` is an all-time count over a code census, which is the
  prompt's YES shape — but the sentence also states its positive count (`six times`) and the list it
  introduces enumerates all six. Whether that enumeration counts as stating the span the census
  covered is exactly the judgement the prompt's NO branch asks for.
- **never_corrected:** needle `six times, never once as a shared name` — one commit, `ee41c635`
  2026-08-27.

## Passages for the RTD-10 prompt — an impossibility with no sites enumerated

Thirteen passages — twelve controls and one positive, the largest section here, because the
pre-registered prediction quoted above is scored against this list and needs at least ten controls
to be scoreable. The prompt fires on a claim that some failure cannot occur, supported only by a
structural reason, with no enumeration of the sites surveyed.

### Passage CTL10-1 — no server-side rule can separate one session from another

- **for_prompt:** RTD-10
- **text:**

```
And **agent sessions are NOT constrained**: they push
over the owner's SSH key, authenticate as `mareurs`, and inherit the bypass. No server-side rule
can separate one session from another, because GitHub sees one identity — that discrimination
exists only in `scripts/pre-push-foreign-session-guard.sh`, and the remote has strictly less
information than that hook does.
```

- **source:** `CLAUDE.md:487-491`
- **why_it_resembles:** full-shape. `No server-side rule can separate one session from another` is a
  flat impossibility over an entire class of rules, supported by one structural reason (`GitHub sees
  one identity`) and no enumeration of the rules surveyed. The claim is correct and the reason is
  sufficient — which is what makes it the kind the registered risk says is a false positive by
  construction.
- **never_corrected:** needle `No server-side rule` — one commit, `f9d77076` 2026-09-16.

### Passage CTL10-2 — a guard that cannot pass by finding nothing

- **for_prompt:** RTD-10
- **text:**

```
and `every_declared_feature_has_a_lane_or_a_reason` (`tests/feature_lanes.rs`)
  reds the build if that lane ever disappears, with `the_guard_is_not_vacuous` asserting the
  guard's own inputs are non-empty so it cannot pass by finding nothing.
```

- **source:** `CLAUDE.md:110-112`
- **why_it_resembles:** full-shape. `it cannot pass by finding nothing` is an impossibility resting
  on one structural property (non-empty inputs), with no cases listed.
- **never_corrected:** needle `so it cannot pass by finding nothing` — one commit, `29f1940c`
  2026-09-06.

### Passage CTL10-3 — TDD cannot reach it

- **for_prompt:** RTD-10
- **text:**

```
**TDD cannot reach the first of those** — the red is observed in the
  pre-fix state, which is the one configuration where both bounds are absent.
```

- **source:** `CLAUDE.md:193-194`
- **why_it_resembles:** full-shape. A whole practice is declared unable to reach a case, on one
  structural reason about which configuration the red is observed in, and nothing is enumerated.
- **never_corrected:** needle `TDD cannot reach the first of those` — one commit, `fbaf2b99`
  2026-09-16.

### Passage CTL10-4 — no assertion can catch it

- **for_prompt:** RTD-10
- **text:**

```
Say what breaks if the detail
  goes — not in the test name, not in the assertion message, never a bare "do not edit". A tidy-up
  that removes it leaves the test passing and no longer discriminating, which no assertion can catch
  because that change is monotone too.
```

- **source:** `CLAUDE.md:265-267`
- **why_it_resembles:** full-shape. `no assertion can catch` quantifies over every assertion that
  could be written, from a single structural reason (monotonicity), with no survey of assertions
  offered.
- **never_corrected:** needle `which no assertion can catch` — one commit, `4f79909d` 2026-08-31.

### Passage CTL10-5 — this one cannot be caught that way

- **for_prompt:** RTD-10
- **text:**

```
**And ask whether the population is CLOSED, because an assertion can be per-member-adequate when
  written and become an aggregate later — with no edit to it, to the code it guards, or to its
  fixture.** Every instance above is a set fixed at authoring time, which an author could in
  principle have enumerated; this one cannot be caught that way, because the members did not exist
  yet.
```

- **source:** `CLAUDE.md:288-291`
- **why_it_resembles:** full-shape, and pointed: the passage explicitly contrasts its case with the
  ones an author `could in principle have enumerated`, and then enumerates nothing itself. The
  impossibility rests entirely on one structural reason about when the members came into existence.
- **never_corrected:** needle `cannot be caught that way, because the members did not exist` — one
  commit, `f9d77076` 2026-09-16.

### Passage CTL10-6 — a test that cannot be written

- **for_prompt:** RTD-10
- **text:**

```
A parser that interprets every token in its namespace is correct on every input it *accepts*; the
defect is the input it makes **unrepresentable**. That is why ordinary testing does not reach this
class — you cannot write a test for a case you cannot express, so the suite exercises the inputs
the grammar admits and passes.
```

- **source:** `CLAUDE.md:694-696`
- **why_it_resembles:** full-shape. A doubled impossibility (`cannot write a test for a case you
  cannot express`) with a structural reason and no sites named — and it sits two sentences before
  the same section's enumeration of twenty-seven instances across five subsystems, which this span
  does not carry. The enumeration exists in the document and not in the passage, which is the
  cleanest available separation of the prompt's two branches.
- **never_corrected:** needle `you cannot write a test for a case you cannot express` — one commit,
  `e0525462` 2026-08-31.

### Passage CTL10-7 — the gate cannot tell the difference

- **for_prompt:** RTD-10
- **text:**

```
If
you must keep a dead path, **fence it**. Do not soften a live citation into a "historical" mention
and leave it in prose: the gate cannot tell the difference, and neither can the next reader.
```

- **source:** `CLAUDE.md:744-746`
- **why_it_resembles:** full-shape, twice over — an impossibility about a tool and an impossibility
  about every future reader, joined, with one structural reason implied and nothing enumerated.
- **never_corrected:** needle `the gate cannot tell the difference, and neither can the next reader`
  — one commit, `3ff4adf2` 2026-09-06.

### Passage CTL10-8 — different parties, no site

- **for_prompt:** RTD-10
- **text:**

```
**Data owner and schema owner are different parties, so the conflation has no site to occur at.**
```

- **source:** `46c3aa57`, `docs/trackers/2026-09-21-kat-telemetry-findings-for-codex-sync.md:182`, `+` side
- **why_it_resembles:** a flat impossibility whose support is a structural separation, with nothing
  enumerated.
- **never_corrected:** withheld — see § *Answer key*.

### Passage CTL10-9 — an alias that cannot run a precondition

- **for_prompt:** RTD-10
- **text:**

```
A cargo alias cannot run a precondition, which is why
this is a wrapper rather than a fix to the alias.
```

- **source:** `CLAUDE.md:143-144`
- **why_it_resembles:** full-shape. A flat capability impossibility over every cargo alias, one
  structural reason, no enumeration — and the whole design decision beside it rests on the claim.
- **never_corrected:** needle `A cargo alias cannot run a precondition` — one commit, `ce00f796`
  2026-09-16.

### Passage CTL10-10 — what the chained form cannot do

- **for_prompt:** RTD-10
- **text:**

```
The script keys `CARGO_TARGET_DIR` on `$CLAUDE_CODE_SESSION_ID`, prints
  the four exit codes, and **exits non-zero if any lane failed** — which the `;`-chained form
  cannot, because it ends in `echo`.
```

- **source:** `CLAUDE.md:46-48`
- **why_it_resembles:** full-shape. `which the `;`-chained form cannot` is an impossibility with one
  structural reason (`it ends in `echo``) and no cases checked.
- **never_corrected:** needle `cannot, because it ends in` — one commit, `58b6bafc` 2026-09-14.

### Passage CTL10-11 — a key that cannot collide, and disclaims being one

- **for_prompt:** RTD-10
- **text:**

```
- **The key shape.** `dev.codescout.mcp/agentId` contains `.` and `/`, neither of which can
  occur in a Rust identifier, so it cannot collide with any real field. That is the escape
  hatch `IC-6` requires, rather than a claim that collision "cannot happen".
```

- **source:** `docs/adrs/2026-09-14-a-subagent-is-a-principal.md:162-164`
- **why_it_resembles:** near-miss, and the most adversarial passage in the file. `it cannot collide
  with any real field` is a full impossibility over every field, from a structural reason, with no
  field enumerated — and the **very next clause disclaims being a `"cannot happen"` claim**. The
  prompt's NO branch covers absolutes appearing only as something the text is narrowing; here the
  absolute is asserted and the *phrase* is narrowed. A prompt keying on the quoted string answers
  NO; one keying on the assertion answers YES. Only this passage and `CTL10-13` can separate those,
  which is why the prediction's reading depends on them.
- **never_corrected:** needle `so it cannot collide with any real field` — one commit, `844d952b`
  2026-09-14.

### Passage CTL10-12 — a row that can never be matched

- **for_prompt:** RTD-10
- **text:**

```
Both reindex call sites pass the active workspace's own roots as `scope_roots`, so a row in another workspace's tree is outside every scope root and can never be matched
```

- **source:** `docs/issues/archive/2026-06-13-delete-orphan-repos-cross-workspace-wipe.md:84`
- **why_it_resembles:** full-shape with a partial enumeration that does not cover the claim. The
  **call sites** are counted (`Both`), but the impossibility quantifies over **rows**, and no row is
  enumerated. It tests whether the prompt checks that what was enumerated is what the claim ranges
  over.
- **never_corrected:** needle `can never be matched`, with `--follow` across the move into
  `archive/` — one commit, `f482938c` 2026-06-14.

### Passage CTL10-13 — an impossibility that lists what it checked

- **for_prompt:** RTD-10
- **text:**

```
**Any credential that is not the owner's user account is refused on both branches** — a
GitHub App, a deploy key, Actions' `GITHUB_TOKEN` — so release automation cannot push a ref
here; no workflow does today (`ci.yml`/`manual.yml` use `push:` only as a trigger)
```

- **source:** `CLAUDE.md:484-486`
- **why_it_resembles:** near-miss, and the section's cleanest test of the prompt's NO branch.
  `release automation cannot push a ref here` is the impossibility — but the passage **enumerates**
  the credentials surveyed (`a GitHub App, a deploy key, Actions' GITHUB_TOKEN`) and names the two
  workflow files it checked. Every trigger feature is present and the stated exclusion is satisfied
  in the same sentence.
- **never_corrected:** needle `so release automation cannot push a ref` — one commit, `f9d77076`
  2026-09-16.

## Passages for the contradiction prompt — two claims in one document

Eleven passages, each a **pair** of spans from one document. The prompt takes a whole document and
fires when two of its passages state things that cannot both be true.

**All eleven pairs come from documents that carry a rule and its exceptions**, which is the ordinary
shape of documentation rather than a defect — the sibling file already records this prompt as the
broadest here and predicts it will be the loudest
(`docs/evals/rule-tell-detection.md:434-440`). Ten of the eleven are drawn from `CLAUDE.md`, which
is a limitation of the sample and not a finding: it is the longest single normative document in the
repository and therefore the one with the most internal distance between a rule and its
qualification. Each pair's `why_it_resembles` names which NO branch it tests — **explicit
correction**, **different subjects**, or **elaboration rather than contradiction**.

### Passage CTLX-1 — a protected branch and an unconstrained actor

- **for_prompt:** contradiction
- **text (A):**

```
**`master` is protected** — all experimental work on `experiments`; promote to `master` only after tests + clippy + MCP verify; `experiments` is never deleted; never commit in-progress work directly to `master`.
```

- **text (B):**

```
And **agent sessions are NOT constrained**: they push
over the owner's SSH key, authenticate as `mareurs`, and inherit the bypass.
```

- **source:** `CLAUDE.md:475` and `CLAUDE.md:487-488`
- **why_it_resembles:** elaboration rather than contradiction, and the eval set names this document
  and this rule as the prompt's own false-positive risk
  (`docs/evals/rule-tell-detection.md:435-437`). A is an absolute with `never` twice; B, twelve
  lines later, describes an actor the absolute does not bind. Both are true and B does not correct
  A — it says who the server-side rule cannot reach, while A's `never commit` is an instruction to
  sessions, not a claim about enforcement.
- **never_corrected:** A, needle `master` is protected` — one commit, `03382cc8` 2026-03-06. B,
  needle `inherit the bypass` — one commit, `f9d77076` 2026-09-16.

### Passage CTLX-2 — a guarantee and its own falsification

- **for_prompt:** contradiction
- **text (A):**

```
Ending on the default lane rebuilds it,
  so **following the gate cannot arm the trap for anyone else — provided both lanes actually run.**
```

- **text (B):**

```
**THAT GUARANTEE IS SEQUENTIAL, AND ITS STATED CONDITION IS EXACTLY THE ONE THAT CANNOT CATCH
  THE FAILURE.** *"Provided both lanes actually run"* is satisfied by **every** party while the
  guarantee still fails
```

- **source:** `CLAUDE.md:17-18` and `CLAUDE.md:19-21`
- **why_it_resembles:** explicit correction, at minimum distance. A is an absolute; B is its
  retraction in the next sentence. The prompt's NO branch covers exactly this — *"when a later
  passage explicitly corrects, retracts, or narrows the earlier one"* — and the adjacency makes it
  the easiest NO in the section. A fire here is the prompt penalising a document for carrying its
  own repair, which § *Distribution* of the sibling file names as the failure that matters.
- **never_corrected:** A, needle `provided both lanes actually run` — one commit, `bcaddb9e`
  2026-09-01. B, needle `THAT GUARANTEE IS SEQUENTIAL` — one commit, `ccfd5920` 2026-09-14.

### Passage CTLX-3 — the only form that closes the window, and the form that is still correct

- **for_prompt:** contradiction
- **text (A):**

```
Typing the four directly is still correct, and they remain the canonical statement of *what* runs and in *what order*
```

- **text (B):**

```
- **`./scripts/gate.sh` runs those four in a PER-SESSION `target/`, and is the only form that
  closes the window above.**
```

- **source:** `CLAUDE.md:9` and `CLAUDE.md:39-40`
- **why_it_resembles:** different subjects, thirty lines apart. A is about correctness of the
  commands; B is about which invocation closes a concurrency window. They read as tension because
  `still correct` and `the only form` are both absolutes about the same four commands, and the
  reconciliation sits further down again at `:52-54`, outside either span.
- **never_corrected:** A, needle `Typing the four directly is still correct` — one commit,
  `38827548` 2026-09-15. B, needle `is the only form that` — one commit, `58b6bafc` 2026-09-14.

### Passage CTLX-4 — never deleted, eventually garbage-collected

- **for_prompt:** contradiction
- **text (A):**

```
`experiments` is never deleted; never commit in-progress work directly to `master`.
```

- **text (B):**

```
`experiments` is rebased after every ship, so a cherry-picked commit's original is orphaned and eventually garbage-collected
```

- **source:** `CLAUDE.md:475` and `CLAUDE.md:493`
- **why_it_resembles:** different subjects, and deliberately hard to see as such. A's `never
  deleted` is about the **branch**; B's `garbage-collected` is about a **commit** on it. The
  subject noun is byte-identical in both spans, which is the strongest available test of the
  prompt's *"when the two are about different subjects"* branch — topic matching says contradiction,
  reading says no.
- **never_corrected:** A, needle `is never deleted; never commit in-progress` — one commit,
  `b603d86f` 2026-07-02. B, needle `eventually garbage-collected` — one commit, `05490dd5`
  2026-08-19.

### Passage CTLX-5 — three incidents, one mechanism

- **for_prompt:** contradiction
- **text (A):**

```
Three measured incidents, one mechanism:
```

- **text (B):**

```
The adjudication had no structured home, so it went to prose, where no reader can join it. **Every automated reader of those params sees 19 clean closes.**
```

- **source:** `ddfce065`, `docs/adrs/2026-09-21-tracker-state-splits-by-recoverability-not-by-shape.md`, `:38` and three lines below it
- **why_it_resembles:** an enumeration claims its members share one mechanism while a member's own
  description names a different failure mode.
- **never_corrected:** withheld — see § *Answer key*.

### Passage CTLX-6 — a number deliberately not stated, and the numbers stated

- **for_prompt:** contradiction
- **text (A):**

```
**The
number is deliberately not stated here.** It is a per-machine fact — the same reason umbrella
membership is not recorded in this repo
```

- **text (B):**

```
**Measured 2026-09-01: `ListAgents` reported
2 peers; the real figure was 16 sessions across 3 profiles, 6 of them in this checkout.** Three
sessions in this very tree were invisible to it and reachable throughout.
```

- **source:** `CLAUDE.md:508-510` and `CLAUDE.md:515-517`
- **why_it_resembles:** different subjects, five lines apart, and the surface reads as a flat
  self-contradiction: a paragraph declares a number withheld and then prints four. A is about the
  *live* peer count, which decays; B is a *dated historical measurement*, which does not. Nothing
  in either span says so — the distinction is carried by the tense and by the date, which is as
  little signal as this prompt will ever get for a genuine NO.
- **never_corrected:** A, needle `number is deliberately not stated here` — one commit, `012549eb`
  2026-09-08. B, needle `the real figure was 16 sessions` — one commit, `23e16bfe` 2026-09-01.

### Passage CTLX-7 — hooks that fire on every call, and calls that go through unblocked

- **for_prompt:** contradiction
- **text (A):**

```
A companion Claude Code plugin (`../claude-plugins/codescout-companion/`) is **always active** here and its hooks fire on every call — you will see `[cs-hint]` advisories.
```

- **text (B):**

```
Measured 2026-08-27 in the `~/.claude-sdd` profile: native `Bash`, `Read` and `Edit` all reach source files unblocked.
```

- **source:** `CLAUDE.md:760`, two sentences of one line
- **why_it_resembles:** elaboration rather than contradiction, at the shortest distance in the
  section — the two sentences are adjacent, which tests the prompt's instruction that *"distance is
  not evidence against a contradiction"* in the opposite direction from CTLX-2. `hooks fire on every
  call` and `all reach source files unblocked` are both absolute and both true: firing is not
  blocking. The reconciling clause (`Enforcement is per-profile`) is on the same line, further
  right.
- **never_corrected:** A, needle `is **always active** here` — one commit, `b603d86f` 2026-07-02.
  B, needle `Enforcement is per-profile` — one commit, `8a7b66c1` 2026-08-27.

### Passage CTLX-8 — an alias that cannot, and an alias that remains canonical

- **for_prompt:** contradiction
- **text (A):**

```
A cargo alias cannot run a precondition, which is why
this is a wrapper rather than a fix to the alias.
```

- **text (B):**

```
Typing
`cargo rb` directly still works and remains the canonical name — three other surfaces and
`ci.yml:385` quote it — so this is a mechanism for whoever runs the wrapper and a policy for everyone
else
```

- **source:** `CLAUDE.md:143-144` and `CLAUDE.md:153-155`
- **why_it_resembles:** different subjects, ten lines apart. A says the alias cannot carry a guard;
  B says the alias is still the canonical thing to type. Both hold — the wrapper is preferred and
  the alias is not withdrawn — but the pair reads as a recommendation at war with itself, which is
  the same structure as CTLX-3 in a different subsystem and is included to see whether the prompt
  treats them alike.
- **never_corrected:** A, needle `A cargo alias cannot run a precondition` — one commit, `ce00f796`
  2026-09-16. B, needle `remains the canonical name` — one commit, `ce00f796` 2026-09-16.

### Passage CTLX-9 — never write that form, and do not sweep the files that have it

- **for_prompt:** contradiction
- **text (A):**

```
- **Record the fix SHA *and* its patch-id — never a pending-master-SHA line.**
```

- **text (B):**

```
Many archived files still carry the older master-SHA-owed form — stale instructions, not open debt; do not sweep them.
```

- **source:** `CLAUDE.md:349`, two sentences of one line
- **why_it_resembles:** different subjects — a rule for **new** records against a disposition for
  **existing** ones. The surface is a `never` immediately followed by an instruction to leave the
  `never` un-enforced across many files, which is the shape a reader hunting absolute-versus-instance
  will call a contradiction. B's `stale instructions, not open debt` is the reconciliation and sits
  inside B itself, so this pair also tests whether an in-span qualifier is read.
- **never_corrected:** A, needle `never a pending-master-SHA line` — one commit, `05490dd5`
  2026-08-19. B, needle `do not sweep them` — one commit, `05490dd5` 2026-08-19.

### Passage CTLX-10 — no count of these laws, and a count of that class

- **for_prompt:** contradiction
- **text (A):**

```
There is deliberately **no count of these laws** — a tally of the section's own contents is a
premise that every addition falsifies.
```

- **text (B):**

```
Promoted 2026-08-31 from `issue-clusters:IC-6` at **27 instances
across five subsystems** (file-format navigation, markdown editing, the citation resolver, four
shell gates, symbol navigation) — the largest class in this corpus, and one that sat at n=2 until
the archive was counted.
```

- **source:** `CLAUDE.md:165-166` and `CLAUDE.md:697-700`
- **why_it_resembles:** different subjects, and the greatest distance in the section — 532 lines and
  roughly a dozen `##` headings apart, which is the configuration the prompt's second paragraph
  exists for. A refuses to count **laws in one section**; B counts **instances of a defect class**.
  Both are absolute in register, adjacent in vocabulary (`count`, `corpus`, `largest`), and about
  different things.
- **never_corrected:** A, needle `no count of these laws` — one commit, `0828ddaa` 2026-09-01. B,
  needle `27 instances` — one commit, `e0525462` 2026-08-31.

### Passage CTLX-11 — demand a red, and do not produce one here

- **for_prompt:** contradiction
- **text (A):**

```
So **demand an
  observed RED, never an assertion's existence** — and **mutate the PRODUCTION path, not the
  test's inputs**
```

- **text (B):**

```
**Do not hand-roll the mutation on this checkout — `./scripts/mutation-probe.sh` exists, and
  the reason is not convenience.** A mutation in the shared tree publishes a red to every other
  session's `cargo test`, byte-identical to a real regression
```

- **source:** `CLAUDE.md:279-281` and `CLAUDE.md:300-302`
- **why_it_resembles:** elaboration rather than contradiction, twenty lines apart. A mandates an
  observed red from a production-path mutation; B prohibits the obvious way to obtain one. The
  reconciliation — use the isolated-worktree script — is inside B's first sentence, so a prompt that
  reads only the imperatives sees a rule and its own prohibition.
- **never_corrected:** A, needle `demand an` — one commit, `0d2ab2b1` 2026-09-02. B, needle `Do not
  hand-roll the mutation on this checkout` — one commit, `82df49ca` 2026-09-14.

## How to run

- Feed the model **only** the `text` field, with no surrounding document and no other field.
  Ask the prompt exactly as written in `docs/evals/rule-tell-detection.md` § *Tells — the bool
  prompts*. Do not show the model this file.
- **A YES on a control is a false positive.** Report it per prompt, and report full-shape and
  near-miss separately — they are different defects with different repairs, and an aggregate
  rate hides which one you have.
- **Score the five known positives first.** If any is a NO, stop: the control results from that
  run are uninterpretable.
- The contradiction prompt takes a **whole document**, not a span. Each of its passages is a
  **pair** quoted from one file; feed the pair, or feed the file and ask about the pair's
  subject. Feeding one half measures nothing — the prompt asks whether two passages conflict.
- **Report per prompt, never in aggregate across the five.** The prompts have different
  breadths; the contradiction prompt is already recorded as the broadest and loudest
  (`docs/evals/rule-tell-detection.md:434-440`), so a pooled precision figure is dominated by it.

## Answer key

**Do not read this before scoring.** Five of the passages above are known positives lifted from
`docs/evals/rule-tell-detection.md` (`d8ea32b5e46c326c`). They are:

| id | section | case it is drawn from | pre-correction blob |
|---|---|---|---|
| `CTL3-6` | RTD-3 | Case RTD-3 — a cause attached to a zero | `21b607f6`, `docs/trackers/2026-09-21-kat-telemetry-findings-for-codex-sync.md`, `-` side |
| `CTL8-6` | RTD-8 | Case RTD-8 — an absolute negative existence claim | `46c3aa57`, same tracker, `+` side |
| `CTL9-4` | RTD-9 | Case RTD-9 — a lifetime quantifier over a pruned corpus | `46c3aa57`, same tracker, `+` side |
| `CTL10-8` | RTD-10 | Case RTD-10 — an impossibility with no enumeration | `46c3aa57`, same tracker, `:182`, `+` side |
| `CTLX-5` | contradiction | Case RTD-15 — an enumeration whose members do not share one mechanism | `ddfce065`, `docs/adrs/2026-09-21-tracker-state-splits-by-recoverability-not-by-shape.md`, `:38` |

Every other passage in this file is a control.

**All five are hard-gate or secondary-hard-gate cases** in the sibling set — RTD-3, RTD-8, RTD-9
and RTD-10 are the `peer × yes` cell that blocks ship there regardless of aggregate
(`docs/evals/rule-tell-detection.md:96-103`), and RTD-15 is its secondary gate. That is deliberate:
if a prompt misses one of these it has already failed its own recall gate, so a precision run using
it is measuring nothing, and this corpus should say so before its control numbers are read.

**What a NO on one of the five means, and what it does not.** It means the run is uninterpretable
for precision. It is **not** a recall measurement — n=1 per prompt here, and the recall corpus is
the sibling file. Do not report a figure from these five as recall.

**One asymmetry worth naming before it is absorbed as a result.** Four of the five positives are
fed to the prompt *dedicated to their own case*, which is the arrangement most favourable to a
fire. `CTLX-5` is fed to the contradiction prompt, which is the prompt its case was written for.
So a fire on any of the five is the easiest fire available in this file, and it is a check that the
setup can go red — never evidence that the prompt discriminates.

## Status

- [x] Passages selected by shape, verbatim, each cited to `file:line` at one named tree
- [x] `never_corrected` established by pickaxe for every control, with the method's blind spot named
- [x] Ground-truth caveat stated: absence of a correction is not proof of correctness
- [x] Five known positives interleaved; answer key separated
- [x] RTD-10 given twelve controls so the registered prediction is scoreable
- [ ] **No prompt run. n=0.** Nothing here has been checked against a model
- [ ] Whether a definitional false positive should count against a prompt, or against the
      prompt's design, is undecided — § *What a control is here* separates the two but the
      scoring policy does not yet say what to do with the separation
