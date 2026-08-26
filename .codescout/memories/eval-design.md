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


## An ANCHORED eval ceilings — naming the target supplies the doubt

If the prompt names what to find ("find every place that reads the tax rate"), the
eval measures retrieval *given* that a search was triggered. That capability is not
the bottleneck, so the arm saturates.

Measured twice on this machine, independently:

- A sealed needle-in-a-haystack harness (needle drawn by `/dev/urandom` from 60
  candidates, five blind subagents, byte-identical prompts): **5/5 exact** on
  name + value + `path:line`, **0** line drift under an off-by-one-is-a-miss rule,
  **5/5 correctly CERTAIN**, mean **5.8 tool calls**. Conclusion recorded at the
  time: *"Retrieval is healthy; triggering retrieval is what's broken. Suspicion is
  the scarce resource, not capability."*
- The hidden-info eval, 2026-08-25: recall **1.0000 in 11 of 12 runs**.

**The failure population is the inverse of what an anchored eval scores.** Of 51
failures reconstructed from real transcripts: **36 confident wrong answers vs 4 pure
misses**. Recall measures misses. And **0 of the 51** were caught by unaided
self-review — 22 by execution, 12 by the human, 10 by an external reviewer.

**The corollary that closes the design space:** anchored + findable = ceiling;
anchored + unfindable = floor (twelve agents once unanimously and *correctly*
declined an impossible band). **There is no discriminating middle inside the
anchored paradigm**, so no negative test can be built there. To get failure, the
prompt must NOT name the target: give a task whose *completion* needs the fact, plant
a plausible belief that already covers it, and score whether the query was issued at
all.

**Decoys must be wrong one hop PAST the value**, not wrong values. Every real decoy
in the record fully satisfied the literal query and failed on a property one step
further: a handler that is real but default-off, a function that exists and is tested
but is wired into one of two call paths, a derived cache with the right name sitting
closer to hand than the ground truth.

**The one intervention with a measured positive effect is a scoring change, not a
rule:** require each claim to carry `CERTAIN`/`UNCERTAIN` plus its justification.
Five of five then double-verified through independent means, unprompted — because it
attaches the check to the act of asserting, the one moment the failure is guaranteed
to be present. Contrast: knowing a rule prevented nothing (three instances committed
in a session that had read the rule in full and quoted it back).

Evidence: `prompt-surface-measurement-session-log:W-12`.

## Read the pilot for RANGE, not only validity

A pilot answers two independent questions and it is easy to ask only one:

- **Validity** — is this number real? (needs ground truth; expensive)
- **Range** — can this number move? (needs only the spread of what came back; free)

Ask range FIRST. It is cheaper and it gates everything built on top.

Measured 2026-08-25: seven rounds and $8.28 into the hidden-info eval, all **12** runs
had produced exactly **three** distinct F1 values, total range **0.046**, against a
separation gate of `ΔF1 ≥ 0.10`. Because every per-run value sat in
[0.8000, 0.8462], any two arm means sat there too — the gate was **unreachable by
construction at any sample size**, and no re-pilot could have passed it.

**Means hide a low-cardinality instrument.** Report the per-run value SET with counts
(`{0.8000 ×7, 0.8276 ×4, 0.8462 ×1}`), not the mean. A mean implies a continuous
measure and makes an unreachable threshold look merely unmet.

**A diversity metric can point the opposite way.** The same round reported **12
distinct answers** — which reads as healthy variance. Twelve distinct answers
collapsed onto three distinct scores: the diversity check was measuring the model's
output, not the instrument's resolution.

Evidence: `prompt-surface-measurement-session-log:F-12`.


## An eval the subject can only WIN is an advertisement

Enumerate the ways the thing under test **loses** with the same effort spent on the
ways it wins, then **count both**. If the counts differ, the instrument is biased
before the first run, and no sample size fixes it.

Measured 2026-08-25 on this repo's own blast-radius spec. Six reference forms were
chosen to stop any single lexical pattern enumerating a dependent set. Two of them were
found by LSP `references` and missed by grep; exactly **one** was missed by LSP and
found by grep. The instrument offered codescout two available points against native's
one, **by construction**, and every gate in the spec would have passed.

**The tell is in how the list was generated.** The forms came from asking *"what would
a grep miss?"* — a question that frames the tool under test as the subject and
everything else as baseline. The mirror question was asked once and answered once. One
row is not a category; it is an afterthought.

Nothing downstream catches this. A leak sweep looks for oracles, not for asymmetry; a
noise-floor gate compares two identical binaries; a positive control proves the scorer
can register a hit. **Bias in the instrument's own construction has no gate**, so it has
to be counted at design time.

Two practices that make it checkable rather than a matter of taste:

- **Write the coverage matrix into the spec** — one row per item, one column per
  toolchain, ✓/✗ filled in. An imbalance is then visible as a column total rather than
  as a feeling.
- **Use different mechanisms for the losing cases, not the same trick twice.** One
  implementation slip should not decide whether the subject can lose at all.

Evidence: `prompt-surface-measurement-session-log:F-15`.

## A bar whose margin comes from a rounding rule is not a measurement

Measured 2026-08-26 (blast-radius, four tasks). Bars were
`round_up_half(min(uniform_p90, stratified_p90))` and `_is_leak` tripped on the **rounded**
bar. The high-recall tier read as passing at 3.49 against 3.5 — a 0.3% margin. The
underlying p90 was **3.07**, so the real value *exceeded* the percentile and cleared only
because the rounding pushed the bar up: **the rounding supplied 0.43 of the 0.43 margin.**

Trip the gate on the raw percentile. Keep the rounded value for publication if it reads
better, and say in the docstring which number is the gate and which is presentation —
that confusion is what let it stand for a round.

**The tell that it is happening:** headroom expressed as a *ratio* (`1.003x`, `1.25x`) rather
than as the null's *p-value*. A ratio is a distance; only the p-value answers "is this a
channel?". In this project a "1.25× headroom" scare turned out to sit at **p = 0.287** —
three in ten random draws reach it — while a comfortable-looking bar hid **p = 0.042**.
Pre-register **per-tier** p, not just the headline: the high-recall tier sat at 0.042 for a
whole round while only the headline (0.537) was recorded, which is why nobody noticed.

## When repairs converge on a constant, measure what the instances share

Three targeted dilution rounds against one leak channel gave **3.49 → 3.36 → 3.30 → 3.30** —
same magnitude, a *different* winning combination each time. That series shape is the
diagnosis: the instances share a cause, and each repair moves the winner to a neighbour the
cause is still feeding.

One measurement found it. Functions per file: the twelve planted dependents averaged
**18.50 (median 19.5)** against the tree's **12.29 (median 8.0)** — 1.5× the mean, 2.4× the
median. Every *existence* predicate (contains a raise, calls getattr, has an f-string, has a
return annotation) is monotone in file size, so a systematically larger truth set is jointly
enriched on all of them at once.

Matching the planted padding volume to the filler distribution moved the tier **0.29 in one
change**, against 0.13 for the whole round before it, and turned the guard green for the
first time in three tasks. See [[W-16]].

**Corollary — match the distribution, not the median.** Collapsing the truth set onto the
tree's median would replace one tell with a narrower one: a truth set of suspiciously low
variance. Keep the spread (here: range 2–31 preserved).

**Corollary — general growth dilutes where targeted batches do not.** Across seven seeds the
two *largest* trees produced the two *lowest* channel values, and of 16 purpose-built
dilution files **zero** appeared in the winning combination's matched set. A bespoke
micro-batch is the narrowest population in the tree and misses the neighbour it creates.

## Ask a reviewer what would change its mind, then measure exactly that

The strongest review in this project ended with an explicit falsification condition: *"run
the null per seed; if each seed's own p90 tracks its real value up, the tier is simply
wide-variance and I would drop the concern entirely."* Running it settled in one pass a
question that three rounds of argument had not: **real spread 1.68 against p90 spread 0.31**
— the reference is flat while the statistic ranges five times as far.

Two habits worth keeping. **Asking for the condition** turns a judgement into an experiment.
**Volunteering it** makes a review falsifiable rather than merely authoritative — a reviewer
who names what would refute them has done most of the work of being checkable.

## Before reusing a probe as a gate, check it is as strong as the guard it stands in for

A probe built to compare two *spreads* was reused as a pass/fail gate on a **0.10**
difference. It drew 150 samples from one scheme; the shipping guard drew **240 across two
schemes and took the conservative `min`**. They disagreed on the shipped tree in opposite
directions, which was an artefact of the instrument, not a fact about the fixture. A p90 of
150 samples is the 135th order statistic of a right-skewed distribution and does not resolve
0.10. See [[F-21]].

Where a project ships its own guard, a probe should **reproduce that guard's sampling rule
exactly** rather than approximate it — then a disagreement is a finding. And an instrument
that cannot resolve a difference should say so in its own output: printing an explicit
`edge` verdict below the resolution limit makes that automatic rather than remembered.


## A fixture's bucket mix is an empirical claim — measure it against real corpora, don't ask

A synthetic fixture that partitions its truth set by *mechanism* (direct reference /
reachable-only-under-a-rename / name-held-as-a-string) is asserting that real code
distributes that way. The ratio **is** the effect size: shift it and the tool under test
looks better or worse without anything about the tool changing. That claim is measurable
on any machine with real code on it, and measuring it costs one script.

Measured 2026-08-26 for the blast-radius eval, whose CHASE_REQUIRED bucket claims
**4 of 12 dependents (33.3%)** reach the defect under a rename they never spell.

**Python, AST-based** (`ast.ImportFrom`, `asname != name`) over 10 corpora — stdlib,
`site-packages`, serena, gpt-researcher, Skill_Seekers, researcher, mempalace, headroom,
prompt-engineering, topictracker: **8,517 files, 72,968 `from X import Y` binding sites.**

| statistic | value |
|---|---|
| all import sites that rename | **3.31%** (2,415 / 72,968) |
| distinct `(module, symbol)` pairs ever renamed | **5.53%** (1,312 / 23,735) |
| **sites renamed, restricted to symbols renamed at least once** | **38.1%** (2,415 / 6,331) |
| same, best-powered corpus (`site-packages`, 4,888 sites) | **39.2%** |
| range across well-powered corpora | 16.1% (mempalace) – 56.4% (stdlib) |
| `__init__.py` files carrying relative re-exports | **42.6%** (370 / 869) |

The third row is the one that answers the fixture's question, and a corpus-wide average
hides it: renaming is rare *across all imports* (3.31%) but common *for the symbols that
get renamed at all* (38.1%). A fixture about indirection necessarily draws its symbol from
that ~5.5% tail, so the conditional rate is the right comparator. **33.3% is inside the
range and slightly conservative against the pooled 38.1%** — the bucket is well calibrated,
not an authoring artifact.

**Kotlin — the same mechanism barely exists.** `import a.b.C as D` over two real repos
(EDU-Planner `backend-kotlin`, JetBrains `kotlin-lsp`): **16 aliased of 16,703 imports =
0.10%** (15/14,199 and 1/2,504). A 33% chase bucket would be wildly unrepresentative of
JVM code. **Conclusions from a Python-shaped fixture do not transfer to Kotlin without
re-deriving the mix.**

**What this measurement cannot see** — state it before attaching a conclusion:
it counts *binding sites*, not dependent *files* (the bucket requires a file to never spell
the symbol; a file may do both); it sees only import-time renaming, so runtime aliasing,
`getattr`, and dict-of-callables — the LEXICAL_ONLY route — are unmeasured; 691 star-imports
hide symbol identity entirely; and "renamed at least once" is corpus-size-dependent, so a
larger corpus qualifies more symbols by construction.

The generalisable rule: **when a fixture parameter encodes a claim about how real code is
shaped, that parameter has an empirical answer — go get it.** Asking a human to eyeball it
buys a slower, worse-calibrated version of a number a script produces in one run. See
memory `reconnaissance`, rule *"Importance × cost decides explore-vs-ask"*. The probe is
`scratchpad/rename_density.py` (session b02898c3); it is 150 lines and takes both rates
because only the conditional one answers the question.


### The other bucket: LEXICAL_ONLY is ~4× over-represented, and it tilts the eval

Same 10 corpora, 8,517 files. LEXICAL_ONLY means the callable is reached by a **string**
— `getattr(x, "name")`, `D["name"]()`, `globals()["name"]`, or a config key — so
`references()` cannot reach it by any number of hops and only a text sweep finds it. The
fixture puts **4 of 12 (33.3%)** there.

| statistic | value |
|---|---|
| files containing ANY string dispatch | **18.5%** (1,579 / 8,517) |
| distinctive callables ever reached by a string | **0.48%** (300 / 62,135) |
| **dependent files reaching a callable ONLY by string** | **8.5%** (136 / 1,608) |
| best-powered corpora | site-packages **4.4%** (756 files), headroom **14.7%** (584) |
| distinctive callables named in config files | **66** across 4,099 config files |

The first and third rows say opposite-sounding things and both are true: **string dispatch
is everywhere, but a callable reachable *only* by string is rare.** Nearly one file in five
does string-based dispatch somewhere; fewer than one distinctive callable in two hundred
has no symbolic route at all. A fixture bucket is about the second, never the first.

**So LEXICAL_ONLY at 33.3% runs ~4× the pooled 8.5%** and ~2.3× the highest well-powered
corpus. The config half is scarcer still: the fixture spends 2 of 12 dependents (16.7%) on
config-key dispatch, against 66 distinctive callables named across 4,099 real config files.

**Which way the two errors push, and why it matters more than either alone.** LEXICAL_ONLY
files are reachable by grep and *not* by `references()`, so over-weighting them favours the
lexical arm. CHASE_REQUIRED is the mirror — reachable under LSP, needing a second grep
lexically — and it is slightly *under*-weighted (33.3% against a real 38.1%). Both
non-neutral buckets therefore tilt the same way: **against the symbol-navigation arm.** A
codescout win on this fixture is conservative; a codescout loss is partly composition, not
capability. Pre-register that, exactly as with the seed sensitivity — a limitation stated up
front is a known property, and the same limitation found by a reader is a defect.

**The denominator trap, recorded because the first run fell into it.** v1 matched symbol
references on the bare name, so `get` collected every `.get(` in the corpus — 495,054
references across 1,209 names, ~409 each — and reported a 0.62% string share that was an
artifact of the denominator. The fix is to restrict to names where a bare-name match means
what it says, and the restriction is not arbitrary: it is the shape of the fixture's own
symbol. **DISTINCTIVE = len ≥ 8, snake_case, defined exactly once in the corpus.** Same
move as conditioning on "ever renamed" in the section above — matching the estimand to the
claim, not cherry-picking. Count **files**, not sites, because the bucket is a claim about
dependent files.

**What it cannot see:** the distinctive-and-unique filter excludes short and common names,
where string dispatch may well be commoner; only in-corpus definitions count, so a config
naming a callable from a dependency is invisible; and the config scan is word-boundary
regex, which the first pass got wrong in a way worth remembering — requiring quotes around
the token silently dropped YAML and TOML **bare keys** and undercounted by 5× (13 names →
66). Probe: `scratchpad/string_dispatch2.py` (session b02898c3); `string_dispatch.py` is v1
and is kept only as the worked example of the trap.
