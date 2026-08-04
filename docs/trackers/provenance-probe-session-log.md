---
id: b0776fdb916cefaa
kind: tracker
status: active
title: Session Log — Provenance Measurement Probe
tags:
- session-log
- provenance
- measurement
- probe
topic: provenance-measurement
---

# Session Log — Provenance Measurement Probe

> **Work stream:** measurement-only probe evaluating whether a provenance /
> attribution subsystem for codescout survives contact with real data.
> Deliverables live in `scratch/provenance-probe/` (`RESULTS.md`,
> `results.json`) — nothing shipped into the product.
>
> **How to use:** append F-N / W-N entries above `## Template for new
> entries`; add the matching Index / Wins Index row. See
> `docs/templates/session-log.md` for the canonical templates and the full
> status vocabulary.

---

## Index

| ID | Date | Severity | Category | Status | Title |
|----|------|---------:|----------|--------|-------|
| F-1 | 2026-08-03 | low | plan-prose | mitigated | Brief cited a `probe_sessions.py` prototype that does not exist on this machine |
| F-2 | 2026-08-03 | high | measurement-hygiene | fixed-verified | The probe's own artifacts contaminated the corpus it measured (6× DF inflation) |
| F-3 | 2026-08-03 | med | measurement-hygiene | fixed-verified | Pooled compaction rate (3.0%) hid 100% compaction in the largest sessions |
| F-4 | 2026-08-03 | med | measurement-hygiene | fixed-verified | Source-level utilisation returned ~99% — a granularity artifact, not a result |
| F-5 | 2026-08-03 | med | measurement-hygiene | fixed-verified | Ubiquity cap conflated centrality with genericity; rejected a core repo type |
| F-6 | 2026-08-03 | med | codescout-tool | open | `create_file` default refusal discards the whole content payload on first attempt |
| F-7 | 2026-08-04 | med | measurement-hygiene | fixed-verified | Langfuse query filtered on the TRACE name against the OBSERVATIONS table — silent zero across all 5 bands |
| F-8 | 2026-08-04 | high | measurement-hygiene | fixed-verified | Reported a tunable gate at 2.15× lift; it was miss-accumulation. Caught by external review, not by me |
| F-9 | 2026-08-04 | high | measurement-hygiene | fixed-verified | Two automated verdict lines printed the OPPOSITE of the numbers directly above them |
| F-10 | 2026-08-04 | high | measurement-hygiene | fixed-verified | A classifier's catch-all default was read as a category for eight rounds |
| F-11 | 2026-08-04 | med | self-friction | fixed-verified | Declared index eviction successful on one mid-flight query; a second refuted it |
| F-12 | 2026-08-04 | med | self-friction | fixed-verified | Marked R-52 settled while 19MB still sat in the repo — rule written, not applied |
| F-13 | 2026-08-04 | high | measurement-hygiene | fixed-verified | 60% of the corpus was base64 image data; nine rounds of analysis ran on top of it |

## Wins Index

| ID | Date | Impact | Pattern | Counterfactual | Status |
|----|------|-------:|---------|----------------|--------|
| W-1 | 2026-08-03 | high | Validate a heuristic extractor against the LSP symbol index before trusting it | Whole measurement rests on an unvalidated regex; no way to defend any number | validated |
| W-2 | 2026-08-03 | high | Run the naive proxy alongside the real classifier to size the methodology delta | Would have reported M1=85%, tripping the >60% kill condition and killing the idea for the wrong reason | validated |
| W-3 | 2026-08-03 | high | Re-audit "no match" results with a rawer matcher before publishing | Would have published M3 ≈2× inflated and mislabelled matcher misses as confabulation | validated |
| W-4 | 2026-08-04 | high | Compare regimes WITHIN one instrument, never across two | Cross-instrument comparison would have read a reader difference as a 5× utilisation collapse | validated |
| W-5 | 2026-08-04 | high | Check trigger/target circularity before reporting classifier precision | Would have recommended a gate whose 100% precision was tautological | validated |
| W-6 | 2026-08-04 | high | Settle a suspected confound with a null model, not an argument | Both sides had plausible stories; the null distribution decided it in one run | validated |
| W-7 | 2026-08-04 | high | Sustained adversarial review by an independent session | Four of my conclusions reversed; none was caught by my own checks | validated |
| W-8 | 2026-08-04 | high | Prefer the design-time form of a check over its empirical twin | Three of five rules answerable before any data; two would have saved whole rounds | validated |
| W-9 | 2026-08-04 | high | Treat a domain owner's "that's not normal traffic" as a corpus-validity test, not a preference | One sentence killed the last buildable item; no internal check had questioned the corpus composition in 13 rounds | validated |

---

## F-1 — Brief cited a `probe_sessions.py` prototype that does not exist on this machine

**Observed:** 2026-08-03, opening reconnaissance of the provenance probe task.

**When:** About to use the named prototype as the parsing starting point.

**Expected (brief):** "A rough prototype exists (`probe_sessions.py`) that does
schema discovery and a crude identifier-overlap proxy."

**Got:** `find /home/marius -maxdepth 5 -name 'probe_sessions.py'` returns
nothing. The file lived in the Claude Desktop project conversation, not on the
filesystem the task was handed to.

**Probable cause:** Task brief authored in a different execution context
(Desktop project) and pasted into a Claude Code session; artifacts referenced
by name were never materialised on disk.

**Workaround:** Did schema discovery from scratch. This turned out to be a net
gain — it surfaced the `attachment` record taxonomy (`file`, `nested_memory`,
`plan_file_reference`, `hook_additional_context`, …) that the entire M1
source-type breakdown is built on, which a prototype-derived parser would
likely have skipped.

**Severity:** low — cost one `find`; the from-scratch path was better anyway.

**Status:** mitigated

**Fix idea / Pointer:** When a brief crosses execution contexts, verify every
named artifact exists before planning around it. Cross-context briefs should
inline the artifact or state that it is absent.

---

## F-2 — The probe's own artifacts contaminated the corpus it measured (6× DF inflation)

**Observed:** 2026-08-03, second build of the per-repo symbol vocabularies.

**When:** Rebuilding `vocab/*.json` after tightening the definition-extraction
patterns; comparing before/after counts as a sanity check.

**Expected:** Tighter patterns → *fewer* extracted symbols and a roughly
unchanged document-frequency vocabulary.

**Got:** codescout's DF vocabulary jumped from 35,496 → **221,214** distinct
identifiers (6.2×) while `n_files` moved only 1374 → 1383. The probe writes its
own output — `sessions.json` (3.4 MB of session metadata), `vocab/*.json`,
`raw_results.json` — into `scratch/provenance-probe/`, which sits **inside the
codescout repo**. The vocabulary walker treats every `.json` under the repo as
a source file, so the probe was indexing its own output as codescout source and
feeding it back in as "codebase-specific" evidence.

**Probable cause:** The instrument's output path lies inside its own measurement
domain, and the corpus walker's exclusion list (`SKIP_DIRS`) was written for
build artifacts (`target`, `node_modules`, `.venv`) — not for the probe itself.
Nobody scouted the overlap because the output directory was created *after* the
walker was written.

**Workaround:** Added `scratch` and `provenance-probe` to `SKIP_DIRS` in
`scratch/provenance-probe/vocab.py`, rebuilt all 8 vocabularies. codescout's DF
returned to 35,329 — consistent with the pre-contamination baseline adjusted for
the tightened patterns.

**Severity:** high — every downstream metric (M1/M2/M3/M5) is computed against
this vocabulary. Had it gone unnoticed, the contamination would have inflated
"codebase-specific" identifier counts for one of the eight repos with tokens
drawn from *the measurement's own intermediate state*, i.e. the numbers would
have partly measured the probe rather than the sessions. It was caught only
because an unrelated sanity comparison made an implausible ratio visible — not
by any deliberate check.

**Status:** fixed-verified — rebuilt vocabularies; DF back to baseline; all
80-session results produced post-fix.

**Fix idea / Pointer:** Generalised as R-51 in
`docs/trackers/reconnaissance-patterns.md`. For any analysis whose output lands
inside the corpus it reads, assert the exclusion at the top of the pipeline and
log the excluded path count, rather than relying on a ratio looking wrong later.

---

## F-3 — Pooled compaction rate (3.0%) hid 100% compaction in the largest sessions

**Observed:** 2026-08-03, validity checks preceding measurement.

**When:** Answering the brief's pre-registered invalidator: "if long sessions
compact and the transcript retains only a summary, context at emission time is
unreconstructable for exactly the sessions with the most injected context."

**Expected (my first reading):** 90 of 2,997 sessions carry `compact_boundary`
markers = 3.0%. I reported this as "benign" before conditioning on anything.

**Got (after conditioning on session length):**

| records | n | compacted |
|---|---:|---:|
| < 100 | 1964 | 0.0% |
| 100–500 | 866 | 1.9% |
| 500–1500 | 72 | 5.6% |
| 1500–5000 | 72 | 65.3% |
| ≥ 5000 | 23 | **100.0%** |

Every session in the top band is compacted. The brief's hypothesised failure
mode was exactly right, and the pooled statistic inverted the conclusion.

**Probable cause:** Reported a marginal rate for a quantity whose whole
significance is conditional. The brief even said "report distributions, not just
means — the spread matters more than the centre"; the pooled number was computed
and narrated before that instruction was applied.

**Workaround:** Corrected in-session before any measurement was run; excluded all
compacted sessions; documented the exclusion as the headline validity constraint
in `RESULTS.md` §0.2 rather than a footnote. Identified the local Langfuse
instance (65,722 observations, full `messages` array per request) as the only
route to the compacted large-session regime.

**Severity:** med — the wrong reading was stated to the user and retracted one
turn later. No measurement was contaminated because the conditioning happened
before the sampling pass.

**Status:** fixed-verified

**Fix idea / Pointer:** When a pre-registered invalidator names a *conditional*
failure mode ("for exactly the sessions with X"), compute the conditional
statistic first and never narrate the pooled one. Kin: R-50 ("the view is not
the set") — a pooled rate is a view that dropped the conditioning variable.

---

## F-4 — Source-level utilisation returned ~99% — a granularity artifact, not a result

**Observed:** 2026-08-03, first smoke test of the measurement pipeline (3
sessions per bucket).

**When:** Reading the first M1 numbers off the pipeline.

**Expected:** Context utilisation somewhere well below saturation — the entire
premise of the probe is that injected context is under-used.

**Got:** utilisation of 95.8%–100.0% across all twelve smoke-test sessions.
Cause: M1 was implemented as a *binary flag per source* — a source counted as
"used" if **any** codebase-specific identifier in it was later referenced. A
10 KB file read containing one later-mentioned symbol scored as 100% used.

**Probable cause:** The brief's wording ("tokens injected vs. tokens containing
at least one matched codebase-specific reference") is granularity-ambiguous, and
the coarsest defensible reading was implemented first without noting that the
choice was free.

**Workaround:** Re-implemented M1 at three granularities and reported all three,
with the line-level figure as headline and the identifier-level figure as the
cleanest economics reading. Final medians: source 58.2%, line 48.3%, identifier
21.1% — a 2.5× span across three defensible readings of the same metric.

**Severity:** med — caught by implausibility on the first real output, before any
sampling ran. Had the coarse reading shipped, M1 would have read ~58% against a
60% kill threshold, i.e. a near-miss decided by an undeclared implementation
choice.

**Status:** fixed-verified

**Fix idea / Pointer:** Pre-registering a *threshold* without pre-registering the
*granularity* leaves the kill decision underdetermined. Any future metric spec in
this work stream must pin the unit before the threshold.

---

## F-5 — Ubiquity cap conflated centrality with genericity; rejected a core repo type

**Observed:** 2026-08-03, validating the strict "codebase-specific" classifier.

**When:** Running a 15-token contaminant probe and a 14-token true-positive probe
against the newly-added strict filter.

**Expected:** Strict filter rejects generic English/programming words
(`description`, `handler`, `schema`, `warning`, `clippy`) and keeps genuine repo
identifiers.

**Got:** All 15 contaminants correctly rejected — but `RecoverableError`, one of
codescout's most load-bearing domain types, was **also** rejected. It appears in
353 of 1372 files (25.7%), tripping the `MAX_DF_RATIO = 0.15` ubiquity cap.

**Probable cause:** The cap treated in-repo ubiquity as evidence of genericity.
For an *unstructured* token that holds; for a structured identifier it is exactly
backwards — a `PascalCase` type used across a quarter of the repo is the most
codebase-specific thing in it. One threshold was doing two incompatible jobs.

**Workaround:** Split the rule — the ubiquity cap now applies only to
*unstructured* tokens (`RARE_DF_RATIO = 0.01`); structured identifiers get a much
looser bound and rely on IDF *weighting* rather than a hard cut. Re-validated: 0
of 57 contaminants accepted, 13 of 14 true positives kept (the single miss is the
repo's own bare name — unstructured and high-DF, an acceptable loss).

**Severity:** med — would have silently deleted the highest-signal references
from every metric, biasing M1 down and M3 up in an unquantified way.

**Status:** fixed-verified

**Fix idea / Pointer:** Hard filters remove tokens that are *not the kind of
thing being measured*; rarity weighting handles tokens that are the right kind
but common. Do not use one threshold for both.

---

## F-6 — `create_file` default refusal discards the whole content payload on first attempt

**Observed:** 2026-08-03, rewriting `scratch/provenance-probe/measure.py` after
the F-4 granularity redesign.

**When:** Submitting a full-file rewrite (~6 KB / ~6k tokens of composed Python)
via `create_file` to an existing path.

**Expected:** Either the write lands, or the refusal is cheap.

**Got:** `{"ok": false, "error": "file already exists", "hint": "Use edit_file to
modify, or pass overwrite: true"}`. The refusal is correct and the hint is good —
but the entire composed payload was already transmitted and is discarded, so
complying with the hint costs a second full re-transmission of the same content.
`edit_file` is not an alternative for a whole-file rewrite (it is exact-string
based), so `overwrite: true` was the only path.

**Probable cause:** Existence is validated after the argument payload is
accepted, which is unavoidable in a single-shot tool call — but it means the
guard's cost scales with content size rather than being constant.

**Workaround:** Re-sent the file with `overwrite: true`. Net cost ≈ 6k tokens.

**Severity:** med — pure token cost, no correctness impact, but it recurs for
every large rewrite.

**Status:** open

**Fix idea / Pointer:** Candidate: on the existence refusal, buffer the submitted
content server-side and return an `@ack_*`-style handle so the retry can
reference it instead of re-sending (the `@ack_*` pattern already exists for
dangerous commands and out-of-scope writes — this is the same shape). Would slot
naturally into the progressive-disclosure buffer system.

---

## W-1 — Validate a heuristic extractor against the LSP symbol index before trusting it

**Observed:** 2026-08-03, after building per-repo symbol vocabularies with
language-specific definition-site regexes (no `ctags`, no `tree_sitter` available
on this machine).

**Pattern:** Before using a heuristic extractor as the backbone of a measurement,
run codescout's authoritative `symbols(path=...)` on a real file and check
recall of the heuristic against it. Report the recall figure alongside the
results.

**Counterfactual:** The entire probe's unit of measurement — "codebase-specific
reference" — is defined by this vocabulary. Without the check, every number in
`RESULTS.md` would rest on an unvalidated regex, and the brief's central premise
("build the classifier using codescout's symbol index, which is the reason this
task runs here rather than elsewhere") would have been claimed but not honoured.
There would be no defensible answer to "how do you know your symbol set is
right?" — which is the first question any reader asks of a measurement like this.

**Confirming data points:**
1. `symbols(path="src/tools/grep.rs", depth=2)` returned 42 symbols (struct,
   trait impl methods, free functions, and the full `tests` module). The regex
   extractor captured 42/42 — **100% recall**, zero misses.
2. The check also confirmed the extractor reaches into nested `mod tests` blocks,
   which matters because test-function names are among the most repo-specific
   tokens in the corpus and a naive top-level-only extractor would have dropped
   them all.

**Impact:** high — converts the classifier from an assertion into a measured
component, and the recall figure is quotable in the deliverable.

**Promote-when:** A second measurement task in this repo validates a heuristic
against an authoritative codescout index before use. At 2 datapoints, promote to
codescout memory `reconnaissance` as: "before a heuristic extractor backs a
measurement, spot-check its recall against `symbols()` and report the number."

**Status:** validated

---

## W-2 — Run the naive proxy alongside the real classifier to size the methodology delta

**Observed:** 2026-08-03, designing the measurement run. The brief warned the
prototype's identifier-overlap numbers were "directional at best."

**Pattern:** When a brief flags an existing approach as unreliable, do not merely
replace it — **run both** on the same inputs and report the delta as a first-class
result. Implemented by parameterising the classifier (`Vocab(repo, strict=True|
False)`) and measuring 24 of the 80 sampled sessions under both.

**Counterfactual:** The loose proxy reports M1-line median **85.0%**; the strict
classifier reports **46.7%** on the identical sessions. The pre-registered kill
condition is "M1 utilisation > 60% → context economics is not a real win."
**The naive proxy trips it; the filtered classifier does not.** Without running
both, the probe would have produced a single number with no way to know the
methodology chose the verdict — and had the probe been run without the symbol
index at all (as the prototype did), it would have killed the idea for entirely
the wrong reason and no artifact would have recorded why.

**Confirming data points:**
1. M1-line: loose 85.0% vs strict 46.7% (38-point gap, spans the kill threshold).
2. References counted per session: loose median 225.5 vs strict 49.0 (4.6×) —
   the proxy's extra 176 refs/session are the contamination.
3. Direct probe: on a 57-token set of generic programming words, the loose
   classifier accepts 32/57; strict accepts 0/57 while keeping 13/14 true
   positives.

**Impact:** high — this is the single most decision-relevant result the probe
produced, and it exists only because both classifiers were run.

**Promote-when:** A second measurement in this repo where an A/B of the
measurement instrument (not the subject) changes a pre-registered verdict. At 2
datapoints, promote to CLAUDE.md or a measurement-conventions doc as: "when a
metric gates a decision, measure the metric's own sensitivity to instrument
choice and report it beside the metric."

**Status:** validated

---

## W-3 — Re-audit "no match" results with a rawer matcher before publishing

**Observed:** 2026-08-03, before writing up M3 (the brief's designated "crux
metric": fraction of codebase-specific references in agent output matching
nothing in context).

**Pattern:** A metric whose numerator is "found nothing" is a claim about the
*matcher* as much as about the data. Before publishing it, re-check a sample of
the "nothing" cases with a deliberately cruder, more permissive matcher — here, a
raw substring scan of all context accumulated before the reference's first use,
plus basename / stem / parent-directory variants for path-shaped tokens.

**Counterfactual:** Nominal M3 was 18.7% median. The audit of 577
nominally-unsourced references found **33.6% are exact substrings of prior
context** that per-token matching had missed, with a further 20.8% derivable via
basename / stem / parent. Only **45.6%** had no trace at all. Publishing the
nominal figure would have overstated the crux metric by roughly 2×, and — worse —
would have labelled ordinary tokenisation misses as evidence of unsourced model
output, which is precisely the inference the metric exists to support.

The audit also produced the probe's sharpest structural finding, which no amount
of staring at the aggregate would have yielded: split by token shape, **path-like
references have no trace only 2.2% of the time** (they are *composed* from
directory listings rather than copied verbatim) while **symbol-like references
are unexplained 83.4%** of the time. Exact-match attribution structurally
undercounts path provenance — a design constraint on any future attribution
system, discovered by auditing the negatives.

**Confirming data points:**
1. 577 references re-checked across 32 sessions; 33.6% full-string hits inside
   context that the primary matcher scored as unsourced.
2. Path vs symbol split: 2.2% vs 83.4% residual-unexplained — a 38× difference
   between two token classes the aggregate metric pooled together.
3. Same audit exposed that "genuinely invented" is an *unpopulated* category by
   construction (a token only counts as a reference if it resolves in the repo
   index), which reframed M3 from a confabulation metric to a recall metric.

**Impact:** high — changed both the headline number and the interpretation of the
brief's crux metric.

**Promote-when:** A second "found nothing" metric in this repo is audited with a
cruder matcher and the residual differs materially from the nominal. At 2
datapoints, promote to codescout memory `reconnaissance` as: "before publishing a
negative-result metric, re-run a sample through a more permissive matcher and
report the residual."

**Status:** validated

---

## F-7 — Langfuse query filtered on the TRACE name against the OBSERVATIONS table

**Observed:** 2026-08-04, round-2 measurement (PV-8, large-context regime).

**When:** First run of `langfuse_measure.py` across five context-size bands.

**Expected:** ~14 sampled observations per band; `traces.name` is `claude-code`,
so `observations` was queried with `where name='claude-code'`.

**Got:** `0 candidates` in every one of the five bands, exit code 0, no error.
`observations.name` is `'llm'` — `'claude-code'` is the **trace** name. The two
tables both have a `name` column with disjoint vocabularies, so the filter was
valid SQL matching nothing. The band populations were in fact healthy
(17,003 / 16,636 / 15,160 / 12,075 / 567).

**Probable cause:** Read the name vocabulary off `traces` during recon, then
applied it to `observations` without re-checking. A `where` clause that matches
nothing is indistinguishable from an empty table at the call site.

**Workaround:** `select name, count() from observations group by name` — which
is the check that should have preceded the query, not followed the zero. Fixed
the filter, re-ran, got 64 reconstructed sessions.

**Severity:** med — cost one full pipeline run (~2 min) and produced a written
`round2_langfuse.json` containing an empty result that looked like a completed
measurement. Had the run not been eyeballed, an empty bands dict would have been
reported as "no large sessions found."

**Status:** fixed-verified

**Fix idea / Pointer:** Instance of R-50 ("the view is not the set") in a new
venue: a zero-row result from a filtered query is a claim about the *filter*
before it is a claim about the data. Enumerate the filter column's actual
vocabulary before trusting a zero. Comment left in `langfuse_measure.py` at the
query site so the next reader does not repeat it.

---

## W-4 — Compare regimes WITHIN one instrument, never across two

**Observed:** 2026-08-04, designing the PV-8 large-context measurement.

**Pattern:** The question was "does M1 hold in the compacted regime the
transcript probe could not see?" The obvious design is to measure Langfuse
sessions and compare against the transcript probe's 48.3% median. Instead:
stratify *within* Langfuse into five size bands and compare large-vs-small
there, using the transcript probe only for direction, never for a delta.

**Counterfactual:** The two readers are not equivalent. Transcripts carry
`attachment` records (CLAUDE.md, hook context, skills) that Langfuse payloads
represent as inline system messages, and the source-type taxonomies therefore
differ. A cross-instrument comparison would have read transcript 48.3% vs
Langfuse-large 9.9% as a **5× utilisation collapse**, when an unknown and
probably large share of that gap is the reader. The within-instrument result
(38.1% → 6.6% monotonically from 150 KB up) is the one that actually supports
the conclusion, and it is a different, smaller, defensible number.

**Confirming data points:**
1. Langfuse small band (<400 KB) medians 24.1% line vs transcript probe 48.3% —
   a 24-point gap between instruments on nominally comparable sessions, which is
   the size of the confound that would have been mistaken for signal.
2. The band table is monotonic from 150 KB upward (38.1 / 31.9 / 29.0 / 6.6),
   so the trend survives without any cross-instrument anchor.

**Impact:** high — the difference between a defensible finding and a headline
number that is mostly instrument artifact.

**Promote-when:** A second measurement in this repo spans two instruments and
the within-instrument design prevents a confound. At 2 datapoints, promote to
codescout memory `reconnaissance` as: "when two instruments cover overlapping
domains, put the comparison inside one of them and use the other only for
direction."

**Status:** validated

---

## W-5 — Check trigger/target circularity before reporting classifier precision

**Observed:** 2026-08-04, reading the Tier 1 gate simulation output (PV-26).

**Pattern:** The gate simulation scores five triggers as classifiers against the
target "patch contains ≥1 unsourced reference." Before reporting, check each
trigger for definitional overlap with the target rather than reading the
precision column at face value.

**Counterfactual:** `claims_absent (>=3 unsourced)` scores **100% precision** at
3.5% overhead — which reads as an ideal gate and would have been the headline
recommendation. It is tautological: a patch with ≥3 unsourced refs trivially has
≥1, so the trigger is a strict subset of the target and its precision is 1.0 by
construction, carrying no information about tunability. Two of the five named
triggers had this property. Reporting them would have recommended building a
gate whose flagship number was an artifact of the target definition — and the
honest result (the design's default config gets 1.12× lift at 25.5% overhead)
points the opposite way.

**Confirming data points:**
1. `claims_absent` precision 100%, lift 3.32× — both artifacts of subset
   membership, not predictive power.
2. The same check exposed a real defect: `low_lexical_overlap` scores 65.3%
   rather than the ~100% its subset relationship implies, because `overlap` is
   `(n_refs - unsourced) / max(n_refs, 1)` which evaluates to 0 for patches with
   **zero** codebase-specific references (4.7% of all patches, e.g. pure
   markdown or config edits). The trigger misfires on empty patches — a genuine
   tuning bug found only by asking why a tautology was not tautological.

**Impact:** high — inverted the recommendation on the single largest unmeasured
risk in the architecture.

**Promote-when:** A second gate/classifier evaluation in this repo where a
trigger overlaps its target definition. At 2 datapoints, promote as: "before
reporting precision, verify the trigger is not a subset of the target."

**Status:** validated

---

## F-8 — Reported a tunable Tier 1 gate at 2.15× lift; it was miss-accumulation

**Observed:** 2026-08-04, round-2 gate simulation (PV-26), refuted in round 3.

**When:** Reporting the Tier 1 result and explicitly contesting an external
recommendation not to build Tier 1 — "a single-trigger gate reaches 2.15× lift at
8.2% overhead, so Tier 1 is buildable, just not as designed."

**Expected:** Having run W-5 (trigger/target circularity), I treated the
surviving trigger `n_refs >= p75` as clean, because it has no definitional
overlap with the target.

**Got:** It has a MECHANICAL overlap instead. The target is a count threshold
("≥1 unsourced ref") over a label the same ledger already knew was ~54%
artifact (W-3), and those artifacts are generated *per reference*. A patch in the
top reference quartile therefore has several times the false-flag opportunities
of a median patch, so P(≥1 false flag) climbs with `n_refs` on its own. A null
model dropping each flag at the measured 54.4% rate produces lift median
**2.468** [p5 2.363, p95 2.602] — the observed **2.15× sits below the null's 5th
percentile**. Pure mechanism does not merely explain the result, it over-explains
it. On a matcher-corrected rate-based target no named trigger clears ~1.4×.

**Probable cause:** W-5 was run as a *checklist item* ("is the trigger a subset
of the target?") rather than as a *question* ("can this trigger gain precision
without predictive content?"). The narrow form passed and I stopped. Two signals
already in the ledger pointed at the wider form and I did not connect them: the
median patch has 0 unsourced refs (the positive class lives in the tail of the
reference-count distribution by construction), and `>= 4 files edited` scored
*below* base rate — which is hard to explain if reference count carries real
signal, and easy to explain if it carries opportunity.

**Workaround:** Implemented PV-4's composition-aware matcher (28.1% of unsourced
flags recovered) and re-ran against count, rate, and null-model targets.
Conclusion reversed in PV-26; rule generalised as PV-31.

**Severity:** high — the claim was stated to the user as a correction of someone
else's recommendation, which is the worst place to be wrong. It would have
argued for building a gate that does not work.

**Status:** fixed-verified

**Fix idea / Pointer:** PV-31 states the rule. The transferable lesson is that a
circularity check has a narrow form and a wide form, and passing the narrow one
is not evidence about the wide one — kin R-50 ("the view is not the set"): the
check I ran was a view of the confound space that had dropped the mechanical
variant.

---

## W-6 — Settle a suspected confound with a null model, not an argument

**Observed:** 2026-08-04, responding to an external challenge that the round-2
gate lift was mechanical rather than predictive.

**Pattern:** Both sides had a plausible verbal story — "more references means
more genuine complexity, so the trigger is real" versus "more references means
more false-flag opportunities, so the trigger is noise." Rather than argue,
simulate the null: keep the trigger and the patch population fixed, and
regenerate the *target* under the noise-only hypothesis at the measured artifact
rate. Compare the observed lift against the resulting null distribution.

**Counterfactual:** Verbal argument could not have settled this, because both
mechanisms are real and the question was which dominates — a quantitative
question wearing a qualitative disguise. The null model answered it in one run
and answered it more strongly than either side predicted: not "the effect is
partly mechanical" but "the observed effect is *smaller* than pure mechanism
predicts" (2.15× observed vs 2.468× null median, below p5). Without it the most
likely outcome was a split decision — "some of the lift is probably real" — which
would have left a non-working gate on the roadmap.

**Confirming data points:**
1. Null median 2.468 [p5 2.363, p95 2.602] vs observed 2.15× — unambiguous, and
   the direction (observed *below* null) is information the argument had no way
   to produce.
2. The two independent cross-checks agreed: rate-based target 1.41–1.60×, and
   fixed-matcher rate target 1.37–1.57×. Three methods, one answer.

**Impact:** high — reversed a stated conclusion and removed a tier from the
buildable set.

**Promote-when:** A second dispute in this repo about whether a measured effect
is mechanical. At 2 datapoints, promote to codescout memory `reconnaissance` as:
"when two plausible mechanisms could produce an effect, simulate the null under
one of them rather than arguing — it is usually one run."

**Status:** validated

---

## F-9 — Two automated verdict lines printed the opposite of their own data

**Observed:** 2026-08-04, rounds 9 and 11 of the provenance measurement.

**When:** Reading pipeline output and reporting it onward to a review session.

**Expected:** A `verdict` line computed from the numbers in the same run agrees
with them.

**Got:** Twice, it did not. Round 9's redundancy test keyed its verdict on a
RATIO (top band 2.2% vs mid 1.1% = 2.1×) and printed "REDUNDANCY — top-band shell
output is largely repeated content" when the absolute level refuted it: 97.8% of
those bytes were novel lines. Round 11's single-law test used `spread < range`
(45.9% < 51.7%) and printed "ONE LAW" when a 45.9-point within-bin spread against
a 51.7-point across-bin range is not evidence of one law at all.

**Probable cause:** A verdict line arrives pre-formatted and adjacent to correct
numbers, which is exactly the position that discourages checking it. The
underlying arithmetic was right both times; only the summary was wrong.

**Workaround:** Caught by reading the numbers rather than the verdict, both times
before the claim propagated. Generalised as PV-56 with the fix: make verdict
lines state their own test inline — `ONE LAW (spread 45.9% < range 51.7%)` — which
converts a report back into a checkable claim.

**Severity:** high — both were one step from being reported as findings, and a
verdict line is a *report of a draw that already happened*, so a reader has
nothing left to settle it against.

**Status:** fixed-verified

**Fix idea / Pointer:** PV-56. Distinct from the population-error family: PV-44's
class is wrong inputs to sound arithmetic; this is sound inputs to a wrong
summary.

---

## F-10 — A classifier's catch-all default was read as a category for eight rounds

**Observed:** 2026-08-04, round 10, while chasing an unrelated discrepancy.

**When:** Shell byte totals from the redundancy pass failed to reconcile with the
share table — 0.7 MB measured against a 77.6% share of ~3.5 MB contexts.

**Expected:** The `tool_output` bucket contains shell/command output.

**Got:** `tool_source_type()` ends with `return "tool_output"`, so every
unrecognised tool joined a bucket whose NAME implied shell. In the largest
sessions that bucket was 95.6% browser-automation MCP output and 3.2% shell.
Every per-source-type figure reported for `tool_output` across rounds 1–8 was
therefore mislabelled, and the round-8 headline ("shell output is the mechanism
driving saturation") reversed entirely: shell share peaks mid-range and FALLS in
the top band.

**Probable cause:** A fallback branch sharing a name with a real category. The
label did analytical work the classifier never authorised.

**Workaround:** Split the bucket three ways (shell / mcp_other / unknown) and
re-ran every band. Recorded as PV-44 with the rule: a fallback branch must never
share a name with a real category — name defaults `other_untyped` so a large
share there reads as an unanswered question rather than a finding. Second half
added on review: any classifier used in measurement must REPORT its unclassified
fraction, the way a survey reports non-response.

**Severity:** high — propagated into a stated conclusion and a design
recommendation before being caught.

**Status:** fixed-verified

**Fix idea / Pointer:** PV-44. Fourth member of the family that cost this work
most (PV-30, F-5, PV-38, PV-44) — in every case the measurement was sound and the
population or label was wrong, and NONE was caught by inspection.

---

## F-11 — Declared index eviction successful on one mid-flight query

**Observed:** 2026-08-04, verifying remediation of the scratch/ retrieval
exposure.

**When:** After triggering a forced reindex, checking whether probe content had
left the semantic index.

**Expected:** A probe-specific query returning only `src/` means the eviction
worked.

**Got:** It did return only `src/` — and I said so. A second probe-specific query
immediately returned `scratch/provenance-probe/*.py` again. The index was still
mid-rebuild; ranking shifts while chunks re-embed, so a single clean query proves
nothing. `indexing.status` was still `running` at the time, and `file_count` was
still 1383.

**Probable cause:** Treated one sample as the state of a system that was
visibly in flux — the status field said `running` and I read the search result
instead.

**Workaround:** Waited for `indexing.status == done` (`files_deleted: 594`,
file_count 1383 → 1346) and re-verified. Correction stated plainly in the same
turn.

**Severity:** med — a false all-clear on a data-exposure check, retracted within
the turn.

**Status:** fixed-verified

**Fix idea / Pointer:** Recorded in PV-60. Do not read an index as clean until
`indexing.status == done`; a mid-flight query is a sample of a moving target.

---

## F-12 — Marked R-52 settled while the artifact was still in the repo

**Observed:** 2026-08-04, after the review session pointed out that gitignore
closes the commit path only.

**When:** Having filed R-52 ("a pipeline reading N corpora writes outside all N")
and committed it, with the probe's 19 MB still sitting in `codescout/scratch/`.

**Expected:** Recording the structural rule and gitignoring the directory closed
the issue.

**Got:** It closed the COMMIT path and the INDEX path. It left the artifact
reachable by `git add -f`, by backup or sync of the working tree, and by any
tooling that does not consult `.gitignore`. The rule had been written and not
applied, and the tracker said settled.

**Probable cause:** Recording a rule feels like discharging it. The gitignore was
a containment measure and contains only the paths it is consulted on.

**Workaround:** Relocated the artifacts to `~/.local/share/provenance-probe`,
outside all eight input repositories; verified every input repo clean and the
repo tree clean. Gitignore retained as defence-in-depth.

**Severity:** med — no exposure beyond a private working tree, but the ledger
asserted a closure that had not happened.

**Status:** fixed-verified

**Fix idea / Pointer:** R-52, now marked *applied* rather than settled. The
general form: an exclusion rule contains only the paths it is consulted on;
relocation removes the artifact from every path at once — which is why the
structural fix sits one level above the procedural one rather than being a
stronger version of it.

---

## W-7 — Sustained adversarial review by an independent session

**Observed:** 2026-08-04, across thirteen rounds of the provenance measurement.

**Pattern:** Every result was written up for a second session that could not run
the queries, only read the numbers and reason about the method. That session
challenged specific claims with named mechanisms and a proposed test attached,
rather than asking for reassurance.

**Counterfactual:** Four of my conclusions were reversed, and **none was caught
by my own checks**: the Tier-1 gate at 2.15× lift (miss-accumulation, F-8); the
shell-output mechanism (a catch-all bucket, F-10); the atomic floor as
irreducible (convergent naming); and PV-7's "structured retrieval is ~3× more
efficient" (a domain mismatch detectable with no data at all). Two of the four
were claims I had made *while correcting the reviewer*, which is the worst place
to be wrong. Without the review, a non-working gate and a mislabelled
intervention would both have gone onto a roadmap.

**Confirming data points:**
1. Four reversals across thirteen rounds, each traced to a specific challenged
   claim and settled by a measurement the reviewer proposed.
2. The reviewer's own hypotheses ran ~1-in-3 (redundancy refuted, single-law
   refuted, composition confirmed) — so the value was not in being right but in
   framing cheap, settleable bets.
3. The final query — file_read split by path class — closed the programme by
   *eliminating* the surviving finding rather than confirming it.

**Impact:** high — it is the difference between a measurement phase and a
self-confirming one, which the brief named as the failure mode to guard against.

**Promote-when:** A second multi-round measurement in this repo runs with an
independent adversarial reader and the reversal rate is materially above what
self-review produced. At 2 datapoints, promote to CLAUDE.md as: "for exploratory
measurement, write every round up for a reader who cannot run the queries."

**Status:** validated

---

## W-8 — Prefer the design-time form of a check over its empirical twin

**Observed:** 2026-08-04, in the closing rounds, once three checks turned out to
have cheaper prior forms.

**Pattern:** Several checks that were run empirically have a version answerable
from the *definition* of the artifact, before any data exists. Ask the definition
question first; keep the empirical version for cases that pass it.

**Counterfactual:** Three of the programme's five transferable rules are
design-time (PV-25 unit-per-metric, PV-53 what-can-this-not-see, PV-58
shared-domain), and two of them would have saved whole rounds:

1. **PV-53 asked of M3 at the outset** yields PV-30 for free — a confabulated
   symbol resolves nowhere, so `invented` cannot enter the numerator. That was
   discovered in round 1 by hand-labelling instead.
2. **PV-58 asked of PV-7 at the outset** yields the refutation for free —
   `symbol_lookup` can only return repo source, `file_read` can return anything,
   so the comparison was a specialist against a generalist over the generalist's
   whole caseload. That took thirteen rounds and a path-class split to find.
3. **R-52 asked before the probe was written** yields the output location for
   free — a pipeline reading eight repos writes outside all eight. That was
   caught at staging, three commits later.

**Confirming data points:** three independent instances in one week, each where
the empirical version was run and the definitional version would have been
instant.

**Impact:** high — PV-55 turns two of them into two lines of a measurement-plan
template at zero cost.

**Promote-when:** The measurement-plan template gains the three pre-registration
lines and a subsequent measurement cites them as having caught something. At that
point promote to the template's own documentation rather than the session log.

**Status:** validated

---

## F-13 — 60% of the corpus was base64 image data; nine rounds ran on top of it

**Observed:** 2026-08-04, round 14, after the project owner said the
browser-automation MCP was "not part of the normal flow" and asked for it to be
excluded.

**When:** Thirteen rounds into the measurement programme, with the sole surviving
intervention (PV-29) written up as SHIP THIS and its trigger derived from the
per-call payload-size distribution.

**Expected:** Excluding one noisy tool would shift the numbers somewhat and leave
the conclusion standing.

**Got:** `chrome-devtools__take_screenshot` was **59.8% of all tool-result bytes**
in the Langfuse corpus — 71 calls, 22.04 MB. A character-class census of all 71
payloads: 71/71 are >90% base64, median fraction inside a contiguous ≥200-char
base64 run **100.0%**, median whitespace **0.00%**, and 37/71 truncated at
`flatten()`'s 400,000-byte cap, so 22.04 MB is a floor. `flatten()` is
`json.dumps(x)[:400_000]`, so an image content block was stringified and counted
as text. Excluding it, PV-29's trigger fell from 137 calls / 61.1% of
information-bearing tokens to 63 calls / 24.9%, with **zero** non-browser MCP
calls remaining above 32 KB. The hook matches `mcp__.*`. Its trigger population is
empty.

**Probable cause:** Two compounding defects, neither of which any round checked.

1. *Unit of the corpus.* The pipeline validated its **metric's** unit carefully
   (F-4, PV-25 — source vs. line vs. identifier) and never validated its
   **corpus's** unit. Bytes were assumed to be text. Base64 holds zero
   codebase-specific tokens by construction, so it inflated every share
   denominator and depressed every utilisation figure simultaneously — pushing in
   both directions at once, which is why no single number looked absurd.
2. *Sampling frame.* The Langfuse bands were stratified by total context size, and
   base64 is what **made** those sessions large. Browser-bearing sessions by band:
   S 0/10, M 0/13, L 0/14, XL 1/14, **XXL 11/13**. Every "as context grows, X"
   conclusion from rounds 8–13 was reading a producer axis wearing a magnitude
   label. On clean sessions utilisation reads S 19.0%, M 35.7%, L 32.8%, XL 31.5%,
   XXL 6.2% (n=**2**) — rising then flat, not decaying.

**Workaround:** Rounds `round14_names.py` (census by producing tool),
`round14_probe.py` (character-class classification), `round14.py` (metrics with
exclusion), `round14_bands.py` (band axis on clean sessions),
`round14_transcripts.py` (contamination census on the transcript corpus).
Transcript corpus is only 7.73% affected across 12 of 2,557 sessions, so
transcript-derived results (M2, M3, M5, rounds 1–7) stand.

**Severity:** high — reversed the programme's only remaining build decision and
invalidated the band axis underpinning six rounds.

**Status:** fixed-verified

**Fix idea / Pointer:** PV-61, PV-62, PV-63, PV-64; R-53. The transferable rule
has two halves, and the second is the one this session lacked. *Census a corpus by
producing tool before measuring it* — a single-tool share above ~20% is a
validity question, not a distribution feature. And *when you stratify by a
magnitude, ask what makes things big in that corpus* — a stratification variable
caused by one producer is a producer axis in disguise. Kin to F-10: that entry
found a **label** covering a category that was not one; this finds a **corpus**
containing bytes that were not what they were counted as. Both were invisible to
every internal consistency check and both were caught from outside.

---

## W-9 — A domain owner's "that's not normal traffic" is a corpus-validity test

**Practice:** When the person who generated the data says a component of it is
not representative, treat the statement as a hypothesis about **corpus validity**
and test it against the data — not as a stylistic preference to be accommodated,
and not as a request to re-cut the same numbers.

**Observed:** 2026-08-04, round 14. The instruction was one sentence: the
browser-automation MCP is not part of the normal flow, take it out. It was framed
as cleaning up an outlier.

**Counterfactual:** Thirteen rounds had run, including an adversarial review by an
independent session that reversed four conclusions (W-7). None of them questioned
what the corpus was **made of**. Every check was internal — consistency between
figures, null models, domain matching, reconciliation of byte totals — and all of
them are satisfiable by a corpus that is 60% base64, because the contamination
moved numerator and denominator together. Without the challenge, PV-29 ships: a
PostToolUse hook on `mcp__.*` with a 32 KB trigger, deployed against a population
of zero qualifying calls, justified by a document citing 61.1%.

**Why it worked:** The owner has knowledge the corpus cannot contain — which
sessions were real work and which were a browser-driving experiment. That is
outside the measurement frame by construction; no internal check reaches it. The
statement was also cheap to test: one census by producing tool, ten minutes.

**Generalisation:** Adversarial review catches reasoning errors; it does not catch
corpus errors, because reviewer and analyst share the same data. Corpus errors are
caught by whoever knows the provenance of the inputs. When such a person makes an
offhand claim about representativeness, it outranks any internal check — census
first, argue after. Note the failure mode this specific phrasing invites: "it
skews the statistics" sounds like a request to drop an outlier, and dropping an
outlier is a small operation. The correct response was to ask what the bytes
actually **were**, which turned a re-cut into a defect finding.

**Status:** validated

**Promote-when:** A second instance of an owner-supplied representativeness claim
overturning an internally-consistent result. On the second, promote to the
measurement-plan template as a pre-registration line: *name who knows the
provenance of this corpus, and ask them what is in it before measuring.*

---

## Template for new entries

<!-- Insert new F-N / W-N entries above this line via:
     artifact(action="update", id=<this artifact>, patch={body_edits: [{
        heading: "## Template for new entries",
        action: "insert_before",
        content: "## F-N — title\n..."}]})
     Also update the matching Index / Wins Index table row at the top.
     Canonical entry templates + status vocabulary: docs/templates/session-log.md -->
