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


### The empty fourth cell — what could fill it, and why we chose not to yet

The blast-radius fixture's three buckets are not three mechanisms. They are three points in
a 2×2 of **which tool reaches the dependent**, and the fourth cell is empty:

|  | `references()` reaches | `references()` misses |
|---|---|---|
| **grep-by-name finds** | BOTH_FIND | LEXICAL_ONLY |
| **grep-by-name misses** | CHASE_REQUIRED | *— empty —* |

Six candidate mechanisms, measured 2026-08-26 over 8 corpora / 8,379 files
(`scripts/probe_dependency_vectors.py`), as % of files containing at least one:

| vector | % files | sites | grep-by-name? | `references()`? | verdict |
|---|---|---|---|---|---|
| `inherit` | **48.0%** | 18,441 | usually | usually | drop — lands in BOTH_FIND |
| `callback` | **21.6%** | 14,606 | no | **wrong direction** | **a new axis, not a cell** |
| `assembled` | **9.4%** | 2,598 | **no** | **no** | **true fourth cell** |
| `registry` | **5.2%** | 4,453 | **no** | reaches the table, stops | fourth cell, other flavour |
| `monkeypatch` | 2.6% | 672 | yes | partly | drop — spells the name, and rare |
| `entrypoint` | 0.2% | 24 | `.toml` only | no | drop — config rows cover it |

**The design test is not "is this mechanism real" — all six are. It is "does it occupy a
distinct point in the tool-advantage space", because that is what the partition partitions.**
By that test the most *common* vector (`inherit`, 48%) is worthless — LSP resolves overrides
and grep finds the method name, so it is BOTH_FIND under another name. Prevalence and
discriminating power are close to uncorrelated here.

- **`assembled`** — `getattr(mod, "duty_" + kind)`. The token `duty_multiplier` exists
  *nowhere* in the dependent, so neither tool can reach it; only reading and reasoning can.
- **`registry`** — `@register("duty")` plus `REGISTRY["duty"]()`. The name exists as a
  string, but a *different* string. Strictly harder than LEXICAL_ONLY, which at least leaves
  the real name greppable.
- **`callback`** — the arrow points the other way: the defect calls the dependent. An agent
  asking *"who depends on this?"* runs callers and structurally cannot arrive, whatever tool
  it holds. Orthogonal to the 2×2 rather than a hole in it.

`Decision (2026-08-26): do NOT add any of them to this eval yet.` Two reasons, and the
second is the one that generalises. **(1) Cost:** every addition moves the generated tree,
re-deriving the leak sweep, the null-control bars — the lift ceiling is `N/n`, so changing
the truth-set size changes what is even *detectable* — and the edge-margin seed calibration.
Task 2b was exactly this change and cost a full task plus two fix rounds. **(2) A fourth cell
where neither tool can win REDUCES the eval's power.** Every dependent placed there is a
dependent that cannot express a difference between the arms. A bucket measuring *reasoning*
does not belong in an eval whose hypothesis is about *retrieval*; it is a second eval, and a
tool-independent one.

**Revisit trigger — read the pilot's traces first.** If BOTH_FIND / CHASE_REQUIRED /
LEXICAL_ONLY all saturate or all floor, the instrument is not resolving the three buckets it
already has and a fourth is premature. That read is free: it comes from data Task 9 collects
anyway. If they *do* separate, the highest-value addition is the **inverted axis**, and it
likely deserves its own fixture — it changes the question (*"who depends on this?"* →
*"what does this touch?"*), and a question change is a new eval, not a new row.

**Two more vectors named but NOT measured**, because AST shape does not find them: **value
coupling** (the dependent hardcodes the constant, so grep-by-*value* works and
grep-by-*name* does not) and **test golden files** (an expected value baked into an
assertion, naming nothing). Both are real, both are classic agent blind spots, and both need
a different detector.


## The instrument/subject verdict boundary — allow-list it, never deny-list it

Any eval that classifies run outcomes has a boundary between *the subject did something* and
*the instrument broke*. Crossing it in the dangerous direction produces a **confident finding
about the subject out of an instrument fault** — which looks like a number, not an error, and
is therefore indistinguishable from a result. It happened **three times in one task** on
blast-radius, each time surviving a green suite:

| the fault | how it scored |
|---|---|
| `BLAST_GOLDEN_AFTER_ROOT` not exported | `broken-after-tree` — *"the agent's own edit broke the tree"* |
| Stop hook could not locate `golden.py` once installed | same, **unconditionally, on every arm** |
| checker raised, caught as `FAIL(checker-error:KeyError)` | `!! GATE 5 FAILED -- TOOL DENIAL LEAKED` |

**The structural fix is an allow-list with a safe default, and the reason is not stylistic.**
The instrument-failure set is **unbounded by construction**: `surface_lib.run` writes
`FAIL(checker-error:<ExcType>)`, interpolating *any* exception type name. No deny-list can
enumerate that. So a run counts only if its verdict is in an explicit, justified set —
blast-radius's is exactly three (`PASS`, `FAIL(broken-after-tree)`, `FAIL(native-tool-used)`),
each traceable to a branch in the checker where the **arm** did something. Everything else,
including verdicts nobody anticipated, is an instrument failure and excludes the run.

Two properties make it hold:

- **Direction of default decides the failure mode.** Deny-listing counts every unforeseen
  class; allow-listing excludes it. Same information, opposite outcome — and only one fails
  toward *"we cannot tell"* rather than toward a confident wrong number.
- **Pin the set from BOTH sides.** Narrowing it must break tests (it drops real results) and
  widening it must break tests (it manufactures fake ones). A one-directional mutation proves
  half a set. Measured: narrowing to `PASS` alone failed 14 tests; widening to swallow
  `checker-error` failed 3.

**The generalisation:** a deny-list inherited from a sibling eval keeps its syntax and loses
its guarantee. `hidden-info`'s `"indeterminate:" not in verdict` was correct *there*, where the
instrument classes were closed. Copied into an eval whose checker can raise arbitrary
exceptions, the same predicate silently changed meaning. **When copying a classifier across
evals, re-derive what its complement contains.**

## A green suite is not evidence about the thing it guards

Three consecutive tasks shipped guards that could not fail, every one under a passing suite:

- **39/39 tests validated a module against a log format the harness never writes.**
  `surface_lib.py:199` wraps every predicate return as `"PASS"` / `f"FAIL({cls})"` before
  logging; the module classified against the raw class. Fed real logs, all five gates REFUSED
  with exit 1. The tests were real, they passed, and they measured a format nothing produces.
- A proposed remedy iterated `_PATH_KEYS + _TEXT_KEYS` to check `_PATH_KEYS + _TEXT_KEYS` —
  so the mutation shrank the test's own loop and it stayed GREEN while the detector had gone
  blind to `pattern`, the only key that sees a `grep` in **either** arm.
- A gate on detector symmetry read the L2 variant carrying a structural floor, so it passed on
  precisely the input it existed to catch.

**What separated truth from colour every time was the same move: apply the mutation and look.**
Not read the test and reason about it. The reusable rules:

- **Never let a guard iterate the thing it guards.** Pin literals (`_ALL_DETECTOR_KEYS`), so a
  shrunken constant shows up as a diff instead of a smaller loop.
- **A fix's own verification must exercise the deployed artifact, not the authored one.** The
  hook worked in place and failed as installed, because every test ran the in-place script;
  `install_hooks` copies only `hook.source`.
- **A mutation harness needs a null control**, or "the mutation was caught" is
  indistinguishable from "the copied tree does not run at all" — the same shape as a checker
  missing its exec bit reporting a clean `0/N`.
- **A surviving mutation is a finding, not a nuisance.** It names a behaviour nothing tests.

## Measure the floor and BOTH ceilings, or you cannot tell "no effect" from "no attempt"

Measured 2026-08-26, blast-radius pilot, 10 runs / $1.79. The eval asked whether an agent
handed a narrow bug report goes looking for who else depends on the code it is changing, and
whether better tools change the answer. Five arms: a no-tools floor, two plain-prompt arms
(shell vs symbol navigation), and the same two arms with the sub-goal explicitly prompted.

| step | dependents reached (of 11) | what it isolates |
|---|---|---|
| no tools → shell, plain prompt | 0 → 1 | having tools at all |
| shell → symbol nav, **plain** | 1 → **1** | **better tools, no sub-goal: ZERO** |
| shell, plain → hinted | 1 → 5 | the sub-goal, with a shell |
| symbol nav, plain → hinted | 1 → 9 | the sub-goal, with symbol nav |
| shell → symbol nav, **hinted** | 5 → **9** | better tools, GIVEN the sub-goal |

**A design with only the two treatment arms would have measured 1 vs 1 and concluded the
tools do nothing.** The true statement is stronger and completely different: tool quality is
worth nothing until the sub-goal exists, and nearly doubles reach once it does. The prompt
creates the behaviour; the tools scale it.

The generalisable rule: **when an eval compares capabilities, a null result between treatment
arms is ambiguous between "the capability does not help" and "the behaviour was never
attempted."** Only a prompted ceiling separates them, and it costs one extra arm. Add the
hinted variant of every arm you care about, not just a floor.

Corollary for reading: the plain-prompt arms fixed the bug correctly (l0 = 1.0) while silently
changing 11-12 other outputs they never mentioned. **"Did it fix the bug?" and "does it know
what it broke?" are near-independent**, and only the first is usually instrumented.

## A negative control's job is to prove the metric CAN fail

Same pilot. Four arms scored `l0 = 1.0` — a perfect fix rate. That figure means nothing on its
own: a checker that says "fixed" too easily produces exactly that. The no-tools arm scored
`l0 = 0.0` on both runs, having made exactly one tool call (a `Bash` the deny-list refused)
before it had no way to act.

**That single 0.0 is what makes the other four 1.0s interpretable.** Before it, "every arm
fixed the bug" and "the checker cannot distinguish a fix from a non-fix" were the same
observation.

So a floor arm is not a formality to satisfy a gate. Budget it whenever a metric saturates
across your real arms — saturation is precisely when you cannot tell a working detector from a
broken one, and precisely when the floor is cheapest ($0.19 here, ~10% of the run).

## A metric that reads tool ARGS measures what the agent had to SPELL

The sharpest instrument defect this project has produced (F-26). L2 — "how many dependents did
the run reach" — extracted paths from each tool call's **arguments**. That silently encodes an
assumption about how tools are addressed:

- **path-addressed** tools (`Read(file_path=…)`, `cat X`): the target is in the args, so
  grep-then-open scores one hit per file.
- **query-addressed** tools (`references(symbol=…)`, `grep(pattern=…)`, `semantic_search`):
  the targets come back in the **result**; the args hold only the query.

So the metric scored ~zero for exactly the tools whose value proposition is *not having to
name the files*. One run called `references()` on the shared symbol — literally the behaviour
under test — and scored 0/12, because the definition site is not a dependent. **Args-only
made the metric an inverse proxy for the capability it was measuring**, and every verdict was
PASS while it did so.

The rule: **before scoring tool use, ask where the answer physically lands for each tool
family you compare.** If it lands in different places, an args-only or results-only metric is
arm-biased by construction. Read both, dedupe across them (a file named in args *and* listed
in a result is one reach), and pre-register the rule before the fix is written.

The related trap, same session (F-27): a verdict that fires on a tool_use block cannot tell
"used a forbidden tool" from "attempted one and was refused". A denied call means the
restriction **held**; scoring it penalises an arm for its own isolation working. `is_error`
and the result text distinguish them — if your pipeline still carries them.

## The per-run log is a projection — read the primary record before believing a number

Three hops in prompt-tdd discard evidence, none of them an error, each leaving something that
still looks like a complete record:

    Claude Code transcript (.jsonl)   every tool_use, tool_result, is_error
      -> parse_transcript             ToolCall(name, args, result, error)
      -> assertions.py:537-540        {name, args, result}        ERROR DROPPED
      -> surface_lib.collect_facts    {name, args}                RESULT DROPPED
      -> the per-run .log             facts block only
      -> assertions.py:574-577        trace file UNLINKED

Both of that day's defects lived in that gap. Both were settled in one look at the transcript,
which was complete the whole time and which Claude Code does **not** delete with the temp dir
(`~/.prompt-tdd/profiles/<profile>/projects/<sanitised-workdir>/<uuid>.jsonl`).

Two habits follow. **Preserve the primary record into the round directory at capture time** —
a profile accumulates every run ever made under it, so "the newest one" stops being
unambiguous the moment a second round exists. And **when a result is uniform, read the
transcripts before writing it up**: `span 0.0000` is the signature of a broken detector *and*
of a genuine floor, and only the primary record tells you which. In this pilot it was a
genuine floor — confirmed by `distinct == 2` and by the cs arm calling `references` zero
times — but the two preceding uniform-looking results had both been artifacts.

Tooling: `prompt-engineering:scripts/inspect_eval_run.py` (`--profile` / `--round` /
`--transcripts`, with `--denied`, `--tool X --full`, `--summary`, `--json`). It reports DENIED
separately from ERROR, because they are the same field to the API and opposite facts to an
eval.

## Instrument "did it ASK?" separately from "could it FIND?" — they fail differently

The blast-radius pilot set out to measure a search capability and instead measured whether a
*goal got formed*. Handed a narrow bug report, Sonnet reached 1 of 11 dependents **with a
purpose-built dependency-navigation tool indexed, available, and called zero times.** The
failure was not "couldn't find". It was "didn't look".

Those are different failures with different fixes, and a single completeness score collapses
them. A tool eval that only measures *how much was found* will report a working tool as
useless whenever the agent never forms the sub-goal that uses it — which is exactly what
happened here: 1 vs 1 between the arms, with the tool doing nothing because it was never
called.

**So split the metric.** One signal for whether the behaviour was attempted at all, one for how
completely it succeeded. And be careful that the "attempted" signal is not saturated by
construction — ours was, because the file the bug report names is also one of the truth sites,
so every run scored a freebie. A saturated attempt-metric is worse than none: it reads as
"always asks" while the completeness metric floors, and the pair looks like a tool failure.

## An affordance is worth nothing until the goal that uses it exists

Same pilot, the numbers that make the point:

| condition | reached /11 |
|---|---|
| shell, plain prompt | 1 |
| symbol navigation, plain prompt | **1** |
| shell, + one sentence asking for impact analysis | 5 |
| symbol navigation, + that sentence | **9** |

The identical tool is worth **zero** and then **four**, and what flips it is two sentences that
name neither the shared function nor any dependent. **Tool value is conditional on intent.**

Two consequences for design:

- **A tool eval that fixes the prompt at "natural" will systematically understate the tool**,
  because it is measuring the joint event (forms the goal AND executes it well) and reporting
  it as the second.
- **The prompt and the tool are substitutes, not complements, when they buy the same thing.**
  Both here supply "get the dependents looked at", so once either provides it the other adds
  little. Do not expect their effects to add.

## Model strength substitutes for tooling — so a single-model tool eval measures the wrong thing

The plain prompt on a stronger model reached **8-10 of 11 unprompted**, beating the weaker
model's *hinted* ceiling. The whole apparatus — tool plus prompt — was replaceable by the model.

The mechanism is the useful part, read from the transcripts: on the weaker model the tool's
entire advantage was one bucket, dependents reachable only by chasing a rename (4/4 against
0/4, perfect separation). On the stronger model the plain shell cracks that bucket too, by
grepping and reading enough to follow the indirection itself. **`references()` was compressing a
multi-step inference into one call — worth exactly as much as the inference the model was not
going to perform.**

The generalisable claim, and it is the opposite of how tools are usually pitched: **a
navigation tool's value is highest where the model is weakest, and decays as the model
strengthens.** Which means a tool measured on one model tier does not generalise to another —
run at least two, and treat the *gap between tiers* as the result rather than either number.

Cost belongs in the same sentence, because it is what makes the finding actionable: weaker
model + tool + prompt reached 9/11 at $0.34/run; stronger model unprompted reached 10/11 at
$0.61. **The cheap stack buys roughly the expensive model's unprompted result at half the
price, and the prompt half of it is free.** Order the interventions that way: prompt first
(free), tool second (cheap), model last (dear).

## "Fixed it" and "knows what it broke" are near-independent — measure the second

Every arm with tools scored a **perfect fix rate**, including the arms that reached 1 dependent
of 11. Those runs shipped a correct patch alongside **11-12 unannounced behavioural changes**.

Nothing catches that in the ordinary course. The fix is genuinely correct for the reported
symptom, tests written against the symptom pass, and the diff is one character. The blast
radius is invisible precisely because the thing that would reveal it — going to look — is the
step that did not happen.

So if an eval measures task success and stops, it will rate the most dangerous configuration
as a complete success. Add a **silent-change** metric: outputs that moved which the answer
never mentioned. It is the only one of our four that distinguished the plain arms from the
hinted ones in the direction that matters (11-12 against 8.5-9).

## A label in a results table is read as a property of the CONDITION, never of the run

Cheap lesson, expensive way to learn it. A tracker entry recorded a behaviour — "the
unrestricted arm does most of its searching through Bash" — and prescribed a *wording* remedy:
relabel that arm "shell". Applied to the results table, it became a false claim about what the
arm was **permitted**, since a column header reads as the condition. The arm in fact had the
full native toolkit; the *restricted* arm was the other one. It shipped in the headline table
of both the in-repo results and the published write-up, and the user caught it, not any gate.

No test, gate or mutation can catch this class: **the number was right and only its name was
wrong.** Every other instrument failure in that eval produced a wrong value; this one produced
a correct value under a false description, which is invisible to everything that checks values.

The rule: behavioural observations go in prose or in a column of their own — never in the
column that names the arm. *"It chose X"* and *"it could only do X"* differ by one word in a
header and completely in meaning. State the permission matrix explicitly, before any number.

## Budget n by EFFECT SIZE — a between-arm difference is always the smallest claim you make

Measured the expensive way, 2026-08-27. Three headline claims published from **n=2** were
falsified by twenty more runs. Every one was a claim about a **difference between two treatment
arms**. Every claim about a *large* effect survived untouched.

| survived n=2 → n=6 | died |
|---|---|
| the floor (0.00, 4/4 runs) | "arm A reaches exactly 1, zero spread" → 2.00 / 2.25 |
| the manipulation (~2 → ~7) | "A beats B 9 to 5" → 7.67 vs 6.67, supports overlapping |
| the model effect (2.0 → 7.8, non-overlapping) | "bucket X separates 4/4 vs 0/4" → 3.17 vs 1.33 |

**The pattern is structural, not luck. A between-arm difference is what is left after the
floor, the manipulation and the model have each taken their share** — so it is the smallest
effect in the design, needs the most runs, and is invariably the thing the eval was built to
measure. Plan n against *that* claim, never against the ones that will look convincing early.

**Why n=2 looked sufficient, which is the trap.** The numbers were not noisy-looking: two arms
read `{1, 1}` — *zero* spread — and the bucket read 0/4 twice against 4/4 twice, in the
direction the fixture had predicted **before any run**. A confirmed prediction at n=2 is still
n=2, and apparent zero-variance from two draws is the *least* informative agreement, not the
most. The support block printed `{1.0000 x2}` and I read consistency into a sample too small to
show any.

**So: state n next to each claim, not once in a caveats section.** A caveat at the bottom does
not travel with a number that gets quoted, and the number is what gets quoted. Sort the claims
by effect size before publishing and draw a line: large effects may go out at low n; a
between-arm difference may not.

## When higher n kills a claim, read what replaced it before recording a loss

Same event, the other half — and the reason thickening is worth doing even when you expect it
to hurt. The three dead claims were **crude versions of a truer one**, and the truer one is
more defensible:

| | n=2 | n=6 |
|---|---|---|
| the bucket the tool should win | 4/4 vs 0/4 | **3.17 vs 1.33** (+1.84) |
| the bucket the tool should lose | noise | **1.83 vs 2.33** (−0.50) |
| net | +4, strictly dominant | **+1.0, a trade** |

At n=2 the tool looked *strictly dominant* — it swept one bucket and the other was noise. At n=6
it shows the **trade the design predicted**: better at one mechanism, worse at another. *A tool
better at one thing and worse at another is a more credible finding than one that wins
everywhere.* The sweep was the artifact; the trade was the result.

Thickening also produced a finding n=2 could not contain: over 6 runs the strong model called
the navigation tool **zero** times where the weaker model called it in 5 of 6 — so the tool's
advantage tracks **usage, not availability**, and its bucket score was *lower* on the stronger
model with the same tools. At n=2 that reads as variance.

**One aggregation choice decided whether any of this was visible.** The bucket probe first
reported a **union** across an arm's runs — "could this arm ever reach it". A union *saturates*
as n grows, so more runs would have made every arm look better and all arms look more alike,
concealing exactly the spread the extra runs were bought to expose. Switching to per-bucket
**means** is what let n=6 speak. **A union is the wrong aggregate for anything you intend to
compare** — it answers a capability question while you are asking a frequency one.

## A control only tests what it is pointed at when something pushes on it

Measured 2026-08-27, two independent instrument defects found by one control arm on its
first run — after both had silently corrupted an entire study.

**The mechanism, and it decides WHICH control to add.** In most evals nearly every arm
behaves the same way on most runs. So an instrument broken in a way *all* your arms share
pairs, denies, or scores things that **happen to be equal**, and its output is
indistinguishable from correct. **The arm that detects it is the one whose runs DIFFER from
the others.**

Both defects fit exactly:

- A FIFO queue paired each run's post-edit tree with a checker by arrival order, and was
  never isolated per round. Every run was scored against an *earlier* run's tree. Invisible
  while every tree was the fixed one; glaring the moment a floor run left the tree unfixed.
  The offset showed up inverted — the floor "fixed the bug" while reaching zero dependents;
  the ceilings "failed" while reaching 11 of 12.
- A no-tools deny-list never named a tool that takes a `command` and runs it. Invisible
  while the weaker model never reached for it; glaring the moment the stronger one did and
  used it as a full shell — `find`, `grep`, `cat`, `python3`, and a patch to the fixture.

**A passing control is a claim about the runs that produced it, nothing more.** Four clean
0.00 runs were read as "tool denial holds." They meant "this model did not go looking."

**Corollary — an opt-in mitigation is an off mitigation.** The queue fix already existed, was
read by both sides at import time, and was documented in the README naming this exact hazard
in this exact wording. Nothing made the driver use it. Full understanding plus a working fix
plus no default is the same as no fix.

**Corollary — separate the verified fact from the inference drawn from it.** The un-denied
tool was left off *deliberately*, with a written reason: absent from one registry, "therefore
not on the headless surface at all." The first half was verified; the second was never tested;
both were written in one sentence at one confidence. When an exclusion carries a reason, the
reason is the thing to re-check — and check it *empirically*, not by re-reading it.

## When a control fails, ask what it measured before calling it a finding

Same session, twice, and the obvious cause was wrong both times:

- The floor scoring above zero read as *"the stronger model answers from priors"* — a
  publishable finding. The transcripts showed it had run a shell. Not a model fact at all.
- The deny-list gap read as version drift; the tool was present in *both* the old and new
  CLI bundles. Not drift.

**A control's failure is evidence about the INSTRUMENT until you have shown otherwise.**
Reading it as a result inverts the one thing the control exists for. The check is cheap —
read the transcript, diff the two versions — and in both cases it changed the published
cause, not merely its confidence.

## Fix the class, not the instance — and ask where else the pattern lives

Measured 2026-08-27. Sweeping a sibling eval for one known defect turned up **four**, every
one of them already recorded and marked `fixed` — fixed in the scenario that surfaced it,
and nowhere else. The sibling shared most of the code and, in one case, the justifying
comment character-for-character.

The worst was a defect whose own title is *"Fixed output path destroyed the evidence for
the headline figure"*: the code that does exactly that was still running in the sibling, on
the ordinary path, deleting the evidence behind published numbers on every invocation.

**A green suite in each place proves nothing about a defect both share.** Each scenario
pinned *its own copy* of the same wrong constant, so each suite confirmed the scenario
matched its own expectation and neither could see that both expectations were wrong. The
only test that can see it is one that compares them — and it has to live where the default
test run collects it, not in either scenario.

**Derive the comparison set, never enumerate it.** A parity test listing the scenarios by
name reproduces the bug: the next one added is covered the day someone remembers to extend
a literal, which is never the day it is added. Glob for the shape instead.

**Verify a new guard by mutation, not by watching it pass.** A parity test is green in
exactly two situations — everything agrees, or it is comparing nothing. Break one side on
purpose and confirm it fails, naming what diverged.

**Watch for defects that a fix makes REACHABLE.** One of the four was latent in the sibling
only because its driver kept a single wiped directory — there was never more than one round
to collide. Fixing *that* made the collision reachable. Porting a fix without its companion
guard would have introduced the bug rather than avoided it. Before shipping a fix, ask what
it makes possible that was previously impossible.

**Rescue evidence before repairing the thing that destroys it.** The script being fixed was
the script that would have deleted the data on its next run. Copy first, verify the copy
re-scores, then edit.

## Match results by an identifier the runs carry — never by searching for content

Measured 2026-08-27, the hard way. To find which of my probe runs produced a signal, I
queried a shared observability log and selected the relevant traces by **searching each
trace body for the prompt string**. The resulting table showed a clean, large,
expected-direction effect between two conditions. It was fabricated.

**The prompt string was in my own session's traces, because I had typed it into the
shell commands that launched the probes.** Running the experiment put the experiment's
marker into the observer's own record; a content match then swept both in and credited
all of it to the subject. Re-attributing by `session_id` showed every positive belonged
to my session and every actual probe was negative.

**When you search a shared log for evidence of your experiment, you are in that log
too.** Anything that logs your activity alongside the run's — a session transcript, a
proxy, an APM, a CI log, a shared database — has this property.

Why it is dangerous rather than merely wrong: it fails with **no error, no sparseness,
and no noise**. It yields a clean table with a large effect in the direction you
predicted, which reads as strong evidence. Mine was half true — the negative rows were
genuine — and half-true tables are the convincing kind.

**The remedy is two rules, both cheap:**

- **Capture an identifier from each run's own output** (`session_id`, request id, run
  id) and match on that. If a run does not emit one, make it emit one before measuring.
- **Vary exactly one input per comparison**, and hold the rest byte-identical. The A/B
  that settled this differed only in one CLI flag; everything else — profile, model,
  permission mode, deny-list, cwd — was the same string.

**And replicate before publishing a between-condition claim**, especially after any
earlier claim in the same session turned out wrong. Two prompts, same result, is cheap
insurance against a third retraction.

**Corollary — a striking figure that cannot discriminate is not evidence.** A sibling
entry the same day rested on "95% of billed output tokens were never returned", which is
equally consistent with *the model omitted it* and *our own request asked it to*. A
measurement that cannot separate the hypotheses cannot choose between them, however
dramatic it looks.
