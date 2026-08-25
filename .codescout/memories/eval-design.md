# Eval & Measurement Design

Hard-won rules for building evals that measure what they claim. Sources: the
hidden-information eval (codescout vs native tools, 2026-08-23/24, six fix rounds) and a
source read of the `prime-radiant-inc` stack (quorum / gauntlet / evener). Full write-up:
`docs/research/2026-08-24-prime-radiant-eval-stack.md`.

## Generated fixtures leak by authorship — plan for it or hand-author

Hand-authored ground truth planted in template-generated filler is separable by a
mechanical filter, because **the two populations differ in every dimension the template
fixes**. Thirteen channels found and closed in one fixture: filename pattern, file length,
symbol-name digit suffix, docstring presence, return annotations, name uniqueness, class
presence, float presence, identifier-token frequency, parameter vocabulary, call-graph
liveness, function-body AST shape, literal-value rarity.

Each fix moved the signal to the next dimension rather than removing it. quorum sidesteps
this entirely by hand-authoring all 85 of its fixtures at 5-35 lines — a legitimate choice
if the task allows it.

- **Patching channels one at a time does not converge.** What worked: a **family** of
  structural predicates measured together, searched over **combinations** (pure ANDs and
  ORs at k≤3 — three individually-clean predicates reached 10-of-12 sites), scored by
  **lift over base rate**, with a committed **null control**.
- **Measure before fixing.** Four rounds of fix-then-discover closed one channel each.
  One round of add-the-predicates-first-then-widen closed the category. Aim the fix at a
  measured target.
- **Distributional, not cosmetic.** Style attributes must be drawn from the same
  distribution on both sides (docstrings, annotations, name uniqueness, class mix, literal
  values). Widening a *bucket* so the planted set hides inside it is not the same as
  overlapping the *distributions* — the first leaves recall at 1.0 and feeds conjunctions.

## Arm symmetry is the validity criterion, not zero-shortcut

An eval comparing two arms does not need a shortcut-free fixture. It needs every surviving
shortcut to be **(a) known and quantified** and **(b) roughly equally available to both
arms**. An unknown shortcut silently changes what is measured; a known one is a stated
limitation that goes in the pre-registration.

- **Symmetric to compute ≠ symmetric in effect.** A grep-native float-rarity filter was
  computable by both arms — but the values it exposed were band-C multipliers, the band
  semantic search is meant to win and grep is meant to struggle with. Ask *which* items a
  shortcut hands over, not just who can run it.
- The dangerous class is a shortcut **easier with the tool under test** — it inflates the
  arm you are evaluating and the result reads as success.

## A null control inherits the blind spot of its predicate space

A null that sweeps only the predicates you thought of converts "we found nothing" into
"nothing is there". Adding predicates from outside the guard's list moved one fixture from
p = 0.775 to p = 0.008.

- **Commit the null as a script that imports the guard's own predicate space**, so the two
  cannot drift. An ad-hoc run is unrepeatable and its records contradict each other.
- **Exclude the real set from the draw pool.** Same-population draws shared a mean 3.9 of
  12 files with the truth set, inflating the null and biasing toward passing.
- **Stratify to the real set's own profile** (e.g. directory mix) — it is the harder test.
  One fixture read p = 0.383 uniform and p = 0.042 stratified.
- **Report the empirical p-value**, not "real equals null median" — that coincidence is
  presentational. Print the swept space size so it can never drift from the space again.
- **The control must be observed detecting a planted leak.** A null that has only ever
  returned "not significant" is indistinguishable from one that always will. Assert both
  directions on one tree: oracle planted → p = 0; same machinery, oracle dropped → p > 0.

## Calibrate bars honestly, and label what they are

If the measured calibration implies a bar the fixture does not meet, do **not** round it up
and call it calibrated. Set the bar where the suite stays green and state in code, docstring
and report that it is a **regression guard, not a validity certificate**, with the implied
figure written down beside it.

## Token accounting — disjoint buckets (ATIF-v1.7 rule, worth stealing verbatim)

`prompt_tokens` = UNCACHED input only · `cached_tokens` = cache-read · `cache_write` =
cache-creation · `completion_tokens` = output. A converter emits per-step metrics **or**
final metrics, **never both** — a hybrid silently drops buckets. **Never fabricate
`cost_usd`**; leave it unset when the source log has none.

## Before believing any count

- **Pre-register** hypothesis, arms, N, metrics and thresholds before the first run.
- **Medians and per-run values**, never means alone — token counts are long-tailed and one
  brute-force run moves a mean without moving the median.
- **Check the denominator counts configured runs, not logged ones** — a run that died
  before logging silently shrinks it (`score_arm.py --expect N`).
- **Contamination is the first thing to rule out.** quorum discredited a 352-run, $650 gate
  because the agent could read its own scenario's answer key. Keep the key outside anything
  the agent can reach, and assert it.


## A truth site must be reachable by EVIDENCE, not by coincidence

An answer key entry is only valid if something in the artifact links it to the question. If
the only route in is a numeric or lexical coincidence, the eval measures **willingness to
guess** and punishes correct reasoning.

Measured 2026-08-25: four "vocabulary drift" truth sites each hardcoded 8.25% under names
like `LEVY` (*"the customs levy"*) and `duty_multiplier` (*"import duty"*), with **no
reference to the constant the task asked about**. A customs levy that happens to equal the
sales tax rate is not the sales tax. Twelve runs across two toolsets scored that band 0.00 —
several naming the sites in their reasoning and then correctly declining to list them. **The
unanimity was the evidence**: twelve independent agents agreeing is a measurement of the
fixture, not of the agents.

- **The fixture had a hard ceiling** at recall 8/12 = 0.6667, and every run hit it exactly.
- **Deleting the bad band is the wrong repair.** Truth = the easy bands only scored f1
  1.000 — breaching the *upper* calibration bound and making the task trivial. There was no
  discriminating middle: easy bands saturated, hard band impossible.
- **The repair is a discoverable link**, not a hint: a derivation chain rooted in the real
  constant, so the value genuinely propagates across renamed concepts. Then the information
  is *hidden* (needs tracing) rather than *absent* (needs guessing).
- **Cap the cheap route explicitly.** Only ONE site in the chain may name the constant, or
  the trivial lexical search selects the whole chain and the hard band collapses into the
  easy one. Assert the cap by measurement over the emitted tree.

## Controls: you need a floor and a ceiling, and each must target the BINDING constraint

Three controls, and most evals ship only the middle one:

- **Positive (ceiling)** — a hint that must lift the score. Proves the instrument can
  register a hit.
- **Noise (tie)** — two identical configurations at different paths must tie. Proves path
  and launch order do not leak.
- **Negative (floor)** — a run with the capability removed must score ~0. Proves the
  **capability** produces the score, not the task's guessability.

The floor is the one usually missing and often the most informative: if agents can score
well with *no tools at all*, the eval is measuring naming conventions and priors, and every
comparison above it is contaminated.

**A control that does not target the binding constraint proves nothing.** Measured
2026-08-25: a positive control hinted at the definition site — a *recall* hint — while
recall was already saturated at 1.0000 in every arm. It scored **identical** to the
uncontrolled arm and the gate failed, not because the instrument was broken but because the
hint had nothing left to lift. Re-check which dimension actually binds *after every fixture
change*; the repair above moved the constraint from recall to precision in one round.

**Until the ceiling control fires, a near-null reading is unvalidated.** An instrument never
shown to respond to a known signal cannot be trusted when it reports no signal.

## Uniform results are the loudest signal an eval can emit

Twelve runs scoring identically to four decimal places is not weak evidence of no effect; it
is strong evidence of **no measurement**. Treat any suspiciously uniform result as an
instrument fault until disproven, and disprove it with two specific checks:

1. **Are the underlying artefacts distinct?** Hash the raw answers. Twelve distinct hashes
   converging on one score is a real finding; one hash twelve times is a stuck harness.
2. **Did the arms actually differ?** Read the tool traces. Two arms that were supposed to
   have different capabilities and show the same tool list were never separate arms.

Both checks are seconds of work and they decide whether the number is reportable at all.

## A number can be real and still describe the wrong subject

The dangerous measurement error is never a wrong number — it is a correct number about
something other than the claim. Five instances in one session (2026-08-25), none of which
errored and four of which looked entirely plausible:

- a `grep -n` line offset **in a JSON tool buffer**, read as a source line;
- a measurement taken on a working tree with **a live mutation** applied;
- a **CLI error envelope** parsed as a model answer, scoring a never-run as a content miss;
- a **tail of an alphabetical concatenation**, reported as the file that just changed;
- a streamed **`init` envelope**, read as proof a run succeeded — only the terminal
  `result` object carries `is_error`.

**Name the surface a number came from before you use it.** Tool responses routinely carry
two coordinate systems at once — source ranges *and* buffer offsets, start events *and*
terminal events — formatted alike and unlabelled.

**What catches it:** a domain constraint making the value *impossible* rather than merely
surprising (`runs: 2` means a third verdict cannot exist); and re-deriving through one
audited instrument instead of reading numbers off whatever surface is nearest.
