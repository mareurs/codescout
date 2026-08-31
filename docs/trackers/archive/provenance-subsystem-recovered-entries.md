---
kind: tracker
status: archived
title: Provenance Subsystem — recovered entries (PV-N)
owners:
  - marius
tags:
  - provenance
  - archive
  - cross-machine
topic: provenance-attribution
---

# Provenance Subsystem — recovered entries (PV-N)

Companion to `docs/trackers/provenance-subsystem.md`. Holds the 38 `PV-N` entries
that existed only in one machine's `catalog.db` and that `CM-2` recorded as
**permanently lost**. They were not lost: `CM-2`'s own `Next:` line named the
recovery path — *"If the desktop's `catalog.db` still exists, a targeted export of
`artifact_augmentation.params` for this one id is the only real recovery path"* —
and the desktop's catalog did still exist. Recovered 2026-08-31.

Every section below is **RECOVERED-VERBATIM**: projected mechanically from
`artifact_augmentation.params`, never re-authored from the prose that cites these
ids. Source snapshot:
`~/.local/share/librarian/catalog.db.bak-20260831-preintegration`
(39,854,080 bytes, `quick_check: ok`).

**`status: archived` is deliberate.** Where two artifacts define one token the sole
*active* definer wins, so if a live stub for any of these is ever added to the
parent tracker it takes precedence automatically and no ambiguous token is created.

**No stubs were added to the live body, on purpose.** The parent tracker's
§ *Defining sections for cited entries* sets a measured policy against
mass-promotion. Checked 2026-08-31: of these 38, **22 appear cited from outside but
0 carry a load-bearing content citation** — the 22 are three bookkeeping files that
enumerate these ids *as lost*, a citation-count table, a doc-comment example of
citation syntax, and a test fixture using `PV-3`/`PV-9` as arbitrary tokens.

<!-- 38 recovered entries -->

#### PV-1 — Tier 0 join key is exact — linkability confirmed

`finding` · **settled** · priority `-`  
*Provenance: RECOVERED-VERBATIM from the desktop `catalog.db`, 2026-08-31.*

tool_use.id -> tool_result.tool_use_id matched 154,266/154,266 (rate 1.000000) across the full corpus. Two redundant paths also exist: sourceToolAssistantUUID and parentUuid. Injected context is additionally self-labelling via attachment `type`.

**Evidence:** RESULTS.md 0.1

**Gated on:** -


#### PV-3 — M2 kill condition FIRED — Tier 4 clustering is descoped

`finding` · **descoped** · priority `-`  
*Provenance: RECOVERED-VERBATIM from the desktop `catalog.db`, 2026-08-31.*

Median attribution density is 2; 67.7% of references resolve to <=2 sources; median is 2 in every one of the four task buckets. Pre-registered condition 'median 1-2 -> skip clustering entirely' fired cleanly. Stop design work on grouping/dedup/summarising multi-source attributions.

**Evidence:** RESULTS.md 2 (M2)

**Gated on:** -


#### PV-6 — M5 survives marginally — upper half of the distribution is at the failure line

`finding` · **open** · priority `med`  
*Provenance: RECOVERED-VERBATIM from the desktop `catalog.db`, 2026-08-31.*

Median 7 clusters per patch after 3.0x locality collapse (median 21 sources). But p75 = 11 and p90 = 13, both at or past the pre-registered threshold of 10. Half of all patches are near or beyond the point where the presentation model was declared to fail. Any presentation design needs a hard cap or progressive disclosure.

**Evidence:** RESULTS.md 2 (M5)

**Gated on:** -


#### PV-10 — M4 not computed — deliberately skipped in the probe brief

`gap` · **open** · priority `low`  
*Provenance: RECOVERED-VERBATIM from the desktop `catalog.db`, 2026-08-31.*

Brief said skip M4 and M6 unless they fell out cheaply. Neither did. Re-scope if the programme continues past measurement.

**Evidence:** probe brief

**Gated on:** -


#### PV-12 — DO NOT BUILD — the addressable population is 0.20% of references

`gap` · **descoped** · priority `-`  
*Provenance: RECOVERED-VERBATIM from the desktop `catalog.db`, 2026-08-31.*

Resolved by PV-40's floor construction. Every prior population in this programme was survival-defined (resolves / doesn't match / isn't decomposable), which can only yield ceilings. Built instead by positive-evidence admission — atomic-unsourced references absent from the human-prose corpus ENTIRELY and appearing in exactly one of eight repos, two independent exclusions both cutting toward genuineness — the population is 13 tokens: 5.9% of atomic-unsourced and **0.20% of all first-use references** (relaxed to <=2 repos: 28 tokens, 0.42%). The true genuine rate lies between that floor and round 5's 7.6% ceiling, but PV-39 places it near the floor: 78.7% of the atomic residual is high-breadth, i.e. convergent naming by one mechanism or another. A confabulation instrument would be built to detect something occurring in roughly 1 in 500 codebase-specific references. The pre-agreed decision rule was that a near-zero intersection is itself the answer. It is not literally zero, and it is close enough. FOOTNOTE ON THE 13 (for whoever reads '~1 in 500' in a year): the floor population is heterogeneous across three repos (7 / 5 / 1), so it is not one artifact class that survived two filters. It does share a SHAPE — terse, abbreviation-heavy, often private or internal identifiers (leading-underscore helpers, 3-6 char abbreviations, SCREAMING_SNAKE constants whose second component is a single letter). At least one member is a probable false positive (a third-party crate name that resolves in one repo's index), so the true floor is at most 13 and plausibly 12 — which lowers the floor further and strengthens the descoping rather than weakening it.

**Evidence:** round7.json; round8.json

**Gated on:** -


#### PV-13 — Single-developer corpus — no cross-developer generalisation

`gap` · **open** · priority `med`  
*Provenance: RECOVERED-VERBATIM from the desktop `catalog.db`, 2026-08-31.*

8 repositories, 3 CC profiles, one engineer. Every distribution reported is that engineer's working style. Utilisation and unsourced rates could differ materially for a team with different retrieval habits.

**Evidence:** RESULTS.md 6

**Gated on:** -


#### PV-14 — Dependency-symbol exclusion is uneven across languages

`gap` · **open** · priority `low`  
*Provenance: RECOVERED-VERBATIM from the desktop `catalog.db`, 2026-08-31.*

Two repos (one Kotlin, one Python) exposed dependency sources only as JAR-internal paths, yielding an empty exclusion set. Codebase-specific counts there may be slightly inflated; cross-repo and structural filters partially compensate. Affects per-repo comparability, not the headline verdicts.

**Evidence:** RESULTS.md 6; probe vocab build

**Gated on:** -


#### PV-15 — Does /provenance write a machine-readable sidecar every run, or on request?

`decision` · **open** · priority `med`  
*Provenance: RECOVERED-VERBATIM from the desktop `catalog.db`, 2026-08-31.*

Carried from the design exploration, undecided. Interacts with the out-of-band constraint: a sidecar every run is more useful for CI staleness checks but costs disk and write amplification on every session.

**Evidence:** design exploration

**Gated on:** -


#### PV-16 — The classifier choice decides the verdict — never run this without the symbol-index filter

`finding` · **settled** · priority `high`  
*Provenance: RECOVERED-VERBATIM from the desktop `catalog.db`, 2026-08-31.*

Same 24 sessions: loose identifier-overlap proxy reports M1-line median 85.0%, tripping the >60% kill condition; the symbol-index-filtered classifier reports 46.7% and does not. 38-point gap spanning the threshold. Reference counts differ 4.6x (225.5 vs 49.0 per session). On a 57-token generic-word probe the loose classifier accepts 32/57, strict 0/57 while keeping 13/14 true positives.

**Evidence:** RESULTS.md 1; probe W-2

**Gated on:** -


#### PV-17 — Is the default scope the working diff or the whole session/repo? — now decidable

`decision` · **open** · priority `high`  
*Provenance: RECOVERED-VERBATIM from the desktop `catalog.db`, 2026-08-31.*

PV-11 makes this a real fork rather than a preference. Whole-repo scope: 72-99% of lines are `unrecorded`, which is exactly the 'trains people to ignore the marker' failure the three-states design was meant to avoid. Working-diff scope: unrecorded is 9-17% in the primary repos. Recommendation: default to the working diff. PV-6 pushes the same way (whole-session scope multiplies cluster cardinality against an already-marginal M5).

**Evidence:** PV-11; PV-6

**Gated on:** -


#### PV-18 — Do unsourced spans ever block, or only ever warn?

`decision` · **open** · priority `high`  
*Provenance: RECOVERED-VERBATIM from the desktop `catalog.db`, 2026-08-31.*

Carried from the design exploration. Note the standing caution: codescout-companion is already aggressive about redirecting Read/Grep — stacking more hard blocks risks people disabling the plugin. PV-5 sharpens this: since M3 does not actually detect confabulation, blocking on it would gate work on a metric that does not measure what the block implies.

**Evidence:** design exploration; PV-5

**Gated on:** PV-12


#### PV-19 — Sufficiency vs. necessity — single leave-one-out marks both sources irrelevant

`hazard` · **carried** · priority `high`  
*Provenance: RECOVERED-VERBATIM from the desktop `catalog.db`, 2026-08-31.*

If two sources are each independently sufficient, single-source ablation shows no effect for either. Common in codebases where a symbol is reachable by several retrieval paths. Needs group ablation designed in from the start; Shapley-style is principled but combinatorial. Do not build Tier 3 as naive LOO.

**Evidence:** design exploration

**Gated on:** -


#### PV-20 — Nondeterminism — ablation compares two sampled generations

`hazard` · **carried** · priority `high`  
*Provenance: RECOVERED-VERBATIM from the desktop `catalog.db`, 2026-08-31.*

A leave-one-out comparison across sampled generations confounds the ablation effect with sampling variance. Needs greedy decoding or multi-sample comparison designed in, not retrofitted.

**Evidence:** design exploration

**Gated on:** -


#### PV-21 — 'Influenced' is not 'should have influenced'

`hazard` · **carried** · priority `med`  
*Provenance: RECOVERED-VERBATIM from the desktop `catalog.db`, 2026-08-31.*

Attribution answers what did influence the output. Review usually wants to know what should have. Do not let the feature over-promise; the gap is not closable by better attribution.

**Evidence:** design exploration

**Gated on:** -


#### PV-22 — No model internals — behavioral methods only, hard precision ceiling

`hazard` · **carried** · priority `med`  
*Provenance: RECOVERED-VERBATIM from the desktop `catalog.db`, 2026-08-31.*

Attention and gradient attribution are unavailable through the API. Everything must be behavioral (string matching, ablation), which imposes a precision ceiling that no amount of engineering removes.

**Evidence:** design exploration

**Gated on:** -


#### PV-23 — The instrument must not write into the corpus it measures

`hazard` · **settled** · priority `med`  
*Provenance: RECOVERED-VERBATIM from the desktop `catalog.db`, 2026-08-31.*

Probe-discovered. The probe wrote sessions.json and vocab/*.json into the repo whose symbol vocabulary it was building; document frequency inflated 6.2x (35,496 -> 221,214) with no error raised, caught only by an incidental ratio check. Not statically scoutable — the overlap is created by RUNNING the pipeline. Assert the exclusion in the walker and log the excluded-path count every run.

**Evidence:** R-51; probe F-2

**Gated on:** -


#### PV-24 — Is provenance-stale drift worth building? — weak NO, repo-dependent

`decision` · **open** · priority `low`  
*Provenance: RECOVERED-VERBATIM from the desktop `catalog.db`, 2026-08-31.*

PV-9 unblocks this. Only 5.7% of derivations see a spec change at >30 days and 2.1% at >90 days; the 58.4% headline is same-session iteration (median lag 0.67d) that ordinary review already catches. Weak no in general. The single outlier repo (31.2% at >30d) now has a STRUCTURAL explanation rather than being an anomaly: 41% of its long-lag events (29 of 71) come from one agent-owned design-system documentation tree, with the rest concentrated in ADR subtrees (architecture, design-system, calendar, features, practices). Median revisions-per-spec is 1 there and 1 in the comparison repo, so specs do not churn more — a SUBSET of specs gets revisited months later. PREDICATE FOR WHO THIS FEATURE SUITS: repos carrying a durable design-system / ADR corpus that outlives the sessions referencing it. NOT repos whose docs are session-scoped plans and trackers revised in the same sitting. GOVERNANCE (added on review): the predicate was derived retrospectively over 11,608 commits, so if ever offered as opt-in it must be gated on a repo running that same retrospective ON ITSELF, not on a maintainer's judgement. The gap between 'we have ADRs' and 'our ADRs get revised months after code derives from them' is precisely where self-assessment fails.

**Evidence:** PV-9; round2_git.json; M6 outlier analysis

**Gated on:** -


#### PV-28 — M3 rises with context size for REAL — symbol-driven, not a path artifact

`finding` · **settled** · priority `med`  
*Provenance: RECOVERED-VERBATIM from the desktop `catalog.db`, 2026-08-31.*

Challenged as possibly an artifact: larger contexts might carry more path-shaped references, and paths are where exact matching fails. Round 4 tested it. The proposed mechanism is ABSENT — path fraction does not trend with context size (13.0/20.0/30.1/19.9/22.8% across the five bands, no monotone). After composition-aware correction M3 still rises 2.7% -> 21.6% (was 11.0% -> 31.5% uncorrected), and the rise is carried by SYMBOL-shaped references (3.6% -> 23.8%) while path-unsourced stays at 0.0% through the first three bands. So the trend is real. Two concessions to the challenge: the corrected large-band figure is 21.6%, not 31.5%, so 'approaches the 40% threshold' was overstated; and the artifact rate is NOT stable across bands — recovery runs 20-44% and path-unsourced climbs to 5.9% then 13.3% in the top two bands, so composition-aware matching itself starts degrading in the very largest contexts.

**Evidence:** round4.json

**Gated on:** -


#### PV-32 — Source type x context size: the granularity reading is CONFIRMED — decay is differential, not uniform

`finding` · **settled** · priority `med`  
*Provenance: RECOVERED-VERBATIM from the desktop `catalog.db`, 2026-08-31.*

Crossed round 1's source taxonomy with round 2's size bands, run INSIDE the Langfuse instrument per W-4. Line utilisation M->XXL: file_read 66.5%->20.9% (0.31x), tool_output 19.6%->2.1% (0.11x), symbol_lookup 81.8%->70.5% (0.86x), search 50.4%->68.4% (1.36x), tracker_memory 40.9%->78.4% (1.92x). Coarse-grained sources decay hard; fine-grained sources hold or improve. This is NOT uniform dilution, so 'inject at finer granularity' is the right frame and 'inject less' is not. UNPREDICTED SECOND RESULT: share of context inverts with size — tool_output goes 5.8% -> 77.6% of all context while symbol_lookup goes 8.8% -> 1.1% and search 1.9% -> 1.4%. In the largest sessions three quarters of context is shell output at ~2% utilisation, and the efficient sources have nearly vanished. The dominant waste in the large regime is tool output, not file reads. AMENDED BY PV-57: the file_read decay (66.5% -> 20.9%) is COMPOSITION, not decay. Split by path class, source-file utilisation RISES with context size (58.9 / 63.1 / 81.9 / 81.2 / 84.4%) while external-file utilisation collapses (51.4 / 70.7 / 34.3 / 23.8 / 7.7%) and external reads are 52-92% of file_read tokens in every band. The 'coarse sources decay, fine sources hold' framing therefore does NOT hold for file_read, which was its main evidence. What survives is narrower: sources whose payloads are large and non-repo are poorly utilised, which is PV-48's size rule again rather than a granularity finding.

**Evidence:** round2_langfuse.json cross-tab; round13.json

**Gated on:** -


#### PV-33 — All metrics recomputed with the corrected matcher — M3 falls 3x, M2 and M5 unmoved

`finding` · **settled** · priority `high`  
*Provenance: RECOVERED-VERBATIM from the desktop `catalog.db`, 2026-08-31.*

Round 3 applied composition-aware matching only to the gate's patch-level target. Round 4 applied it to every metric over all 80 transcript sessions. M3 median 18.7% -> 6.2% (IDF-weighted 6.5%), a 3x reduction, with median 50% of previously-unsourced references recovered per session. M1-line RISES 48.3% -> 53.1% (more references resolve, so more sources count as used) and the exploratory bucket moves to 73.1%, further above the 60% kill. M1-ident is unmoved at 21.3%. CRITICALLY, M2 and M5 are unchanged: density median stays 2, sources-per-patch stays 21, clusters-per-patch stays 7. So the matcher correction does not rescue the presentation model or revive clustering — M2's kill and M5's marginality are robust to it. Per bucket M3 corrected: refactor 17.6%, greenfield 6.2%, exploratory 4.2%, bugfix 3.6%.

**Evidence:** round4.json

**Gated on:** -


#### PV-34 — Unsourced-ness is a SYMBOL phenomenon — paths are ~fully resolvable once matching is composition-aware

`finding` · **settled** · priority `high`  
*Provenance: RECOVERED-VERBATIM from the desktop `catalog.db`, 2026-08-31.*

With composition-aware matching, per-session median path-unsourced rate is 0.0% (p75 3.8%, p90 11.6%) against symbol-unsourced 8.6% (p75 18.8%, p90 33.3%), and path-shaped references are 26% of all references. This closes the loop on the 577-reference audit, which predicted it from the opposite direction (2.2% vs 83.4% residual-unexplained). CONSEQUENCE FOR ANY FUTURE DESIGN: attribution for path references is a solved bookkeeping problem and needs no model; attribution for symbol references is the only part that is actually hard. A system that treats both alike will spend its budget on the easy half. Caveat: path-unsourced rises to 5.9% and 13.3% in the top two Langfuse bands, so composition-aware matching degrades in very large contexts (PV-28).

**Evidence:** round4.json; RESULTS.md 2

**Gated on:** -


#### PV-35 — Retrieval precision and recall degrade TOGETHER as context grows — the product argument

`finding` · **settled** · priority `high`  
*Provenance: RECOVERED-VERBATIM from the desktop `catalog.db`, 2026-08-31.*

Synthesis of two measurements built for different purposes (contributed by design review). As context grows, M1-line falls 38.1% -> 6.6% while symbol-unsourced rises 3.6% -> 23.8%. More of what is injected goes untouched, AND more of what gets used was never injected. PV-7 alone measures waste; the pair measures waste and failure moving in opposite directions simultaneously, which is a materially stronger argument for granularity-aware retrieval than either half. Confounds stated: M3's population admits instrumentation gaps and convention-inference, and the band-dependent artifact rate makes 23.8% an upper bound. Direction holds, and direction is what the product argument needs.

**Evidence:** PV-7; PV-28; PV-32; round4.json

**Gated on:** -


#### PV-36 — The symbol residual is 43% atomic — atomic is NOT irreducible, only unshrinkable-by-matching

`finding` · **settled** · priority `med`  
*Provenance: RECOVERED-VERBATIM from the desktop `catalog.db`, 2026-08-31.*

Review proposed splitting qualified symbols on the language separator, as the symbol analogue of basename/stem/parent. FACTUAL CORRECTION: the tokeniser already does this — IDENT_RE matches maximal [A-Za-z0-9_] runs, so `::`, `.`, `->` and `#` are separators and `SessionStore::rotate` has always been two independently-checked references. The untested analogue is MORPHOLOGICAL (camelCase / snake_case components), which round 5 ran. Of 6,594 first-use references: 4,781 symbol-shaped, 689 still unsourced after composition-aware matching (14.4% of symbols). Of that residual — 42.7% ATOMIC (single component, nothing to decompose); 57.3% multi-component, of which 47.3% have ALL components present in context (27.1% of the residual) and 94.4% have ANY (54.1%). Strict morphological recovery therefore shrinks the residual any model-based tier would address from 10.4% to 7.6% of all references, with 4.5pp of that atomic and irreducible. Answers the review's fork: recovery is real but partial, and the atomic fraction is the first positive evidence of genuine unsourced-ness. CORRECTION (round 6): the claim that the atomic fraction is a floor was WRONG. Atomic identifiers cannot shrink by better MATCHING, but they can shrink by better CLASSIFICATION — the axis this programme has now been burned on three times (PV-30, F-5, here). A single-morpheme PascalCase identifier receives the loose bound from the F-5 fix, so ordinary vocabulary that also resolves in the repo index (Workspace, Project, Agent) is admitted. Measured in PV-38: atomic-unsourced references are simultaneously RARER in-repo and MORE COMMON in human prose than the atomic-sourced control, which is the convergent-naming signature. The floor is real but lower than 4.5pp and not yet bounded.

**Evidence:** round5.json; round6.json

**Gated on:** -


#### PV-37 — Gate cost models err in a direction CORRELATED with the trigger, not randomly

`hazard` · **settled** · priority `med`  
*Provenance: RECOVERED-VERBATIM from the desktop `catalog.db`, 2026-08-31.*

PV-26 charged Tier 2 a flat ~5 candidates per firing. But the gate selects on reference count and Tier 2's candidate set is generated FROM references, so firing patches carry larger candidate sets by construction. Measured: mean candidates 36.20 on firing patches (n_refs>=11) vs 19.40 on quiet ones — **1.87x**. The error does not average out; it concentrates exactly where the gate fires. Scope note: this understates Tier 2's INPUT cost (lexical+embedding over candidates); Tier 3's ablation cost is unaffected if the narrowing target stays fixed at ~5 survivors. RULE: when a gate and a downstream cost are both functions of the same quantity, cost must be measured conditional on firing, never as a corpus average.

**Evidence:** round6.json

**Gated on:** -


#### PV-39 — Cross-repo breadth beats prose rate as a discriminator, and reveals the dominant mechanism

`finding` · **settled** · priority `high`  
*Provenance: RECOVERED-VERBATIM from the desktop `catalog.db`, 2026-08-31.*

Breadth needs no external wordlist — it comes from the eight per-repo symbol indexes that already exist — and it separates the populations twice as cleanly as the prose proxy. Median repos-where-the-token-appears: atomic-UNSOURCED 6.0 of 8, atomic-SOURCED control 3.0. Unsourced concentrates at breadth 6-8 (52% of the population); sourced concentrates at breadth 1-2 (44%). Cross-tab on the 221 atomic-unsourced (prose>1% of docs, breadth>=3 repos): ordinary vocabulary (hi/hi) 27.6%; CONTAMINATION CLASS (hi prose / lo breadth) only 3.6%; code-convention term (lo prose / hi breadth) **51.1%**; strongest genuine candidates (lo/lo) 17.6%. The largest cell is the one the prose proxy could not see: terms common across codebases but absent from human prose (impl, mux, and kin). Combined, 78.7% of the residual is high-breadth and therefore convergent by one mechanism or another. Control check confirms the direction — atomic-SOURCED sits at 42.5% in the lo/lo cell versus 17.6% for unsourced, i.e. repo-specific tokens get sourced precisely because they are in context.

**Evidence:** round7.json

**Gated on:** -


#### PV-41 — Tier 3's cost exemption depends on HOW Tier 2 narrows — pin it in the spec

`hazard` · **open** · priority `med`  
*Provenance: RECOVERED-VERBATIM from the desktop `catalog.db`, 2026-08-31.*

PV-37 established that Tier 2's input cost is 1.87x understated on firing patches. The claim that Tier 3 is exempt carries an unstated assumption of exactly the shape PV-37 just caught — a flat constant that may not be one. If Tier 2 narrows by TOP-K, survivor count is fixed and the exemption holds exactly. If it narrows by a SIMILARITY or SCORE THRESHOLD, survivor count scales with input and the 1.87x propagates straight into the ablation stage, which is the expensive one since each survivor costs a generation. One line in the Tier 2 spec settles it. Pin it now rather than rediscovering it: the last two cost assumptions in this programme were both flat constants that were not.

**Evidence:** PV-37; design review

**Gated on:** -


#### PV-42 — REVERSED — shell share is NOT monotonic; the climber is THIRD-PARTY MCP output

`finding` · **settled** · priority `high`  
*Provenance: RECOVERED-VERBATIM from the desktop `catalog.db`, 2026-08-31.*

Round 8 reported shell share climbing monotonically 5.8% -> 77.6% and called shell output the mechanism driving saturation. WRONG, and the error was mine: `tool_source_type()` returns "tool_output" as its DEFAULT for any unrecognised tool, so that bucket was a catch-all conflating shell with every third-party MCP server (PV-44). Split properly, SHELL share is 5.5 / 7.1 / 13.3 / 11.1 / 2.5% across the five bands — it peaks in the middle and FALLS in the top band — and shell line-utilisation stays healthy at 40.8 / 22.0 / 28.5 / 37.2 / 19.3% with no cliff. The monotonic climber is `mcp_other` (third-party MCP servers): 0.3 / 1.7 / 1.3 / 15.5 / **75.1%**, at utilisation 0.0 / 10.2 / 24.4 / 8.2 / **1.9%**. In the top band 95.6% of that bucket is one browser-automation MCP server; shell is 3.2%. SCOPE CAVEAT: 11 of 13 top-band sessions carry >1MB of it, across 2 repos, with 57.8% concentrated in one — distributed enough not to be a single project's anomaly, narrow enough that it is one tool CLASS, not a general law. || REVERSED A SECOND TIME 2026-08-04, on sampling grounds rather than labelling grounds (PV-62). The first reversal (PV-44/F-10) found the climbing bucket was a catch-all, not shell. Round 14 finds the BAND AXIS itself was confounded: stratifying by context size selected for the artifact that creates context size. Any claim of the form 'as context grows, X' drawn from rounds 8-13 needs re-derivation on the clean subset, where the top band has n=2.

**Evidence:** round10.json; round8.json; round14_names.json; round14_probe.json; round14.json; round14_bands.json; round14_transcripts.json

**Gated on:** -


#### PV-43 — The output-buffer threshold is a PER-CALL flat constant with no cumulative accounting — third instance

`hazard` · **open** · priority `med`  
*Provenance: RECOVERED-VERBATIM from the desktop `catalog.db`, 2026-08-31.*

Answers the pointer question: `shell_output_limit_bytes` was never retired functionality — it was a config key that was accepted, documented, and a SILENT NO-OP, removed as vestigial (archived bug 2026-06-28). Nothing was lost. The mechanism that DOES exist is the progressive-disclosure buffer: TOOL_OUTPUT_BUFFER_THRESHOLD = 10,000 bytes with INLINE_BYTE_BUDGET = 9,000 (src/tools/core/types.rs:22-27), applied PER CALL with no accounting of accumulated session context. 500 calls each just under the threshold reach ~4.5MB inline — precisely the 2.5MB+ band. That is the flat-constant mistake for the THIRD time (PV-37 cost-per-firing, PV-41 Tier-2 narrowing, now this). What PV-42 supports is a threshold CONDITIONAL on accumulated context size, not a fixed byte cap — and it is a policy change on when output goes to a @tool_* buffer rather than inline, not new machinery. SCOPE CORRECTION (PV-42 reversal): the per-call flat-constant critique is still sound, but codescout's buffer governs only codescout's own tool results. The bucket that actually saturates large contexts is a third-party MCP server's output, which never passes through TOOL_OUTPUT_BUFFER_THRESHOLD at all. A cumulative-context-conditional threshold in codescout would improve codescout's own footprint — which the data says is already healthy — and would not touch the dominant term. || ROUND 14 NOTE. The critique stands as a design observation and is now the ONLY part of the context-economy strand still standing, but it has no measured benefit behind it: codescout's own tool results never exceeded 32KB anywhere in the corpus (0 calls across every mcp__codescout__* tool), which is what a working 10,000-byte buffer looks like. A cumulative-context-conditional threshold would be a correctness improvement to a mechanism that is not currently failing. Do not cite PV-48's numbers in support of it.

**Evidence:** round8.json; docs/issues/archive/2026-06-28-vestigial-shell-output-limit-bytes.md; src/tools/core/types.rs:22; round14_names.json; round14_probe.json; round14.json; round14_bands.json; round14_transcripts.json

**Gated on:** -


#### PV-45 — Shell-output redundancy hypothesis NOT supported — and moot once the bucket was split

`finding` · **settled** · priority `low`  
*Provenance: RECOVERED-VERBATIM from the desktop `catalog.db`, 2026-08-31.*

Tested whether the (apparent) top-band shell collapse was repetition rather than volume: same command re-run, near-identical output each time, near-zero marginal utilisation. Measured per band within Langfuse. Repeated normalised lines are 0.0 / 1.5 / 0.7 / 1.0 / **2.2%** of shell bytes; bytes from an already-run command 3.1 / 8.4 / 4.3 / 7.2 / **9.6%**; whole outputs with >=90% line overlap 0.0 / 0.9 / 4.8 / 5.6 / 6.7%. The top/mid ratio is 2.1x but the ABSOLUTE level is ~2% — 97.8% of top-band shell bytes are novel lines, so redundancy explains nothing. NOTE ON THE FIRST READ: the script's verdict rule tested the RATIO and printed 'REDUNDANCY'; the absolute magnitude refutes it. A relative threshold on a tiny base is its own small instance of the PV-44 family. The question is moot anyway — shell was never the saturating term.

**Evidence:** round9.json; round10.json

**Gated on:** -


#### PV-47 — REFUTED — utilisation is NOT one decay law; source predicts in the normal range, size only in the tail

`finding` · **settled** · priority `high`  
*Provenance: RECOVERED-VERBATIM from the desktop `catalog.db`, 2026-08-31.*

Hypothesis: mcp_other (whole pages), file_read (whole files) and symbol_lookup / search (small targeted results) are one relationship — utilisation decays with per-call payload size regardless of source. Tested on 12,335 per-call records within Langfuse. REFUTED on two counts. (1) The pooled curve is an INVERTED U, not a decay: 6.9% / 28.1% / **51.7%** / 32.9% / 4.7% / 0.0% across <500B / 500B-2K / 2-8K / 8-32K / 32-128K / >128K. Small payloads are poorly utilised too, so 'smaller is better' is false at the low end. (2) Controlling for size, SOURCE still separates strongly — within-bin spread median 45.9% against an across-bin range of 51.7%. In the <500B bin alone: symbol_lookup 47.2%, search 51.4%, file_read 16.5%, mcp_other 8.5%, shell 4.4% — a 10x spread at identical payload size. What IS true is a ceiling effect in the tail: >=32KB payloads run at 0.8% utilisation regardless of source. Two effects, not one law — and the tail effect is the actionable one (PV-48). AMENDED (round 12): the left arm of the inverted U is COMPOSITION, not size. In the <500B bin, low-utilisation sources supply 82.5% of tokens (shell 29.2%, user_prompt 27.8%, system_prompt 19.1%, mcp_other 6.4%) while symbol_lookup and search sit at 47.2% and 51.4% utilisation in that same bin. So small payloads are not intrinsically badly used; the bin average is dragged by its mix. The pooled curve therefore confounds a size effect with a changing source mix across bins, and the per-source-per-bin table is the one that separates them. This does not weaken the threshold but it changes its justification: size is preferable as the SAFE variable, not the causal one.

**Evidence:** round11.json; round12.json

**Gated on:** -


#### PV-48 — 74 tool calls carry 51.3% of all information-bearing context and are referenced 0.0%

`finding` · **superseded** · priority `high`  
*Provenance: RECOVERED-VERBATIM from the desktop `catalog.db`, 2026-08-31.*

The sharpest actionable number produced in eleven rounds, and it is a SIZE rule, not a source rule. Per-call payload thresholds over 12,335 information-bearing calls: >=8KB = 633 calls, 76.3% of tokens, 7.2% utilisation; >=32KB = 137 calls, 61.1% of tokens, 0.8%; >=128KB = **74 calls, 51.3% of tokens, 0.0%**. Half of all information-bearing context arrives in 74 payloads and none of it is ever referenced. Composition above 32KB: mcp_other 80.3% (74 calls), file_read 8.8% (16 calls), system_prompt 7.7%, user_prompt 3.1%. Because it is expressible as a size threshold it is general — it predicts unmeasured tools, and it puts codescout's own file_read back in scope, which a source rule scoped to a third-party MCP server would not. || SUPERSEDED 2026-08-04 (round 14, PV-61). Every figure in this entry has a denominator that is 59.8% base64 screenshot data. Corrected: >=8KB 633->519 calls, 76.3%->53.2% of tokens; >=32KB 137->63 calls, 61.1%->24.9%; >=128KB 74->14 calls, 51.3%->11.4%. The cliff at 32KB does not survive: what remains above it is file_read (44.4%), system_prompt (39.5%) and user_prompt (16.0%), i.e. protected or unbufferable categories.

**Evidence:** round11.json; round14_names.json; round14_probe.json; round14.json; round14_bands.json; round14_transcripts.json

**Gated on:** -


#### PV-49 — Redundant computation as deliberate practice — disagreement is the only detector that has worked

`hazard` · **open** · priority `high`  
*Provenance: RECOVERED-VERBATIM from the desktop `catalog.db`, 2026-08-31.*

Four population/label errors (PV-30, F-5, PV-38, PV-44) and NONE was caught by inspection. All four surfaced because a number failed to reconcile while chasing something else — the detection mechanism was having enough numbers that one COULD fail to reconcile, not vigilance. Four for four is a strong enough record to make it a rule rather than an observation: where it is cheap, compute the same aggregate two ways — not to cross-check arithmetic but because DISAGREEMENT is the only reliable detector this programme has found for population errors. Concrete instance from this round: shell byte totals from the redundancy pass failed to reconcile with the share table, which is what exposed PV-44.

**Evidence:** PV-30; F-5; PV-38; PV-44

**Gated on:** -


#### PV-50 — Within-instrument comparison does NOT validate the instrument's classification

`hazard` · **settled** · priority `med`  
*Provenance: RECOVERED-VERBATIM from the desktop `catalog.db`, 2026-08-31.*

W-4's rule ('compare regimes within one instrument, never across two') removes the cross-instrument confound. It does NOT validate the instrument's categories — a monotonic trend inside a mislabelled bucket is still monotonic, which is exactly how PV-42 survived a round. The missing follow-through: when two taxonomies differ, the difference lands in whichever bucket is the RESIDUAL, so the first check on any cross-instrument share discrepancy is whether the growing category is a catch-all — BEFORE reasoning about what the growth means. Both sides of this exchange had the discrepancy in hand, named it correctly as cross-instrument, and then did not ask where a definitional difference would go.

**Evidence:** PV-42; PV-44; W-4

**Gated on:** -


#### PV-51 — RESOLVED — PostToolUse CAN rewrite results and matchers reach third-party MCP; the dominant term is ours

`gap` · **settled** · priority `high`  
*Provenance: RECOVERED-VERBATIM from the desktop `catalog.db`, 2026-08-31.*

Resolved by a Claude Code hook-documentation check performed in the design-review session (NOT independently verified in this repo — no local copy of the hook schema was available, and the companion contains no precedent). Both halves are supported: (1) PostToolUse returns `hookSpecificOutput.updatedToolOutput` which REPLACES the tool's original result (`additionalContext` is the additive field; a top-level `{decision:"block"}` suppresses visibility entirely, the tool having already run). (2) Matchers accept regex over MCP tool names, so `mcp__.*` catches every third-party MCP result. Two design constraints: hook output strings cap at 10,000 characters, beyond which content spills to a file and a preview+path is returned — an overflow behaviour that IS a buffer and suits the use case; and PostToolBatch explicitly cannot modify individual tool outputs, so bounding is per-call by construction. ARCHITECTURE, using machinery that already exists on both sides: a PostToolUse hook matching `mcp__.*` writes the full payload into codescout's existing @tool_* output buffer and returns a summary plus handle via updatedToolOutput. Retrieval stays available; inline cost collapses. That is PV-43's policy applied at the layer that can actually enforce it.

**Evidence:** Claude Code hook docs (design-review session); codescout-companion/hooks/hooks.json

**Gated on:** -


#### PV-52 — PV-48's cost figure is a LOWER BOUND — the metric cannot price the content it drops (fifth instance)

`hazard` · **settled** · priority `high`  
*Provenance: RECOVERED-VERBATIM from the desktop `catalog.db`, 2026-08-31.*

0.0% utilisation at >=128KB deserved interrogation before anchoring a policy, and it does not survive it. Utilisation counts codebase-specific reference reuse; a browser page dump is HTML, prose and third-party markup containing essentially nothing that resolves in the repo symbol index, so its utilisation is 0% BY CONSTRUCTION whether or not the agent read it and acted on it. MEASURED: **82.4% of the 74 payloads >=128KB contain ZERO codebase-specific tokens** (median 0.00 per KB), versus only 7.5% zero in the well-utilised 2-8KB control band (median 2.54 per KB). At >=32KB, 52.6% contain zero. By source above 128KB: mcp_other 81.7% zero-spec, file_read **100%** zero-spec (12 calls). So exactly-zero across 74 payloads is what a blind metric looks like, not what unused content looks like. FIFTH instance of the exclude-by-construction family (PV-30, F-5, PV-38, PV-44, now this) and the first to land on the BENEFIT side of the only shippable item. CONSEQUENCE: benefit (61% of injected tokens) solid; cost (0.8% of referenced content) is a lower bound by an unknown amount, and the true ratio is below 76:1. It also settles the design question in the right direction — buffer, do not truncate.

**Evidence:** round12.json

**Gated on:** -


#### PV-54 — Utilisation has THREE structural blind spots, not one — and the chosen policy survives all of them

`finding` · **settled** · priority `high`  
*Provenance: RECOVERED-VERBATIM from the desktop `catalog.db`, 2026-08-31.*

Utilisation prices REFERENTIAL value: content counts as used if a codebase-specific token from it reappears later. Three kinds of value are invisible to that by construction. (1) NON-CODE CONTENT — HTML, prose, logs; PV-52. (2) VERIFICATION READS — the agent reads test output to confirm a fix worked or a build log to confirm it compiled; the content is consumed as a BOOLEAN, nothing is referenced, and the read was the entire point of the loop. (3) NEGATIVE INFORMATION — content that causes the agent to STOP: a file that turns out not to contain the bug, a search returning nothing relevant. Correct, valuable, referentially silent. IMPLICATION FOR INTERPRETATION: shell utilisation at 19-41% is understated by a meaningful margin, since verification is much of what shell output is FOR in an agent loop; same for tests and diagnostics. IMPLICATION FOR THE DECISION: none — and that is the useful part. Buffering never bets against value the metric cannot see, it only defers the cost of retrieving it; truncation would have been exposed to all three. A decision that survives an enumeration of everything its evidence cannot see is a stronger position than the 76:1 ratio ever was.

**Evidence:** PV-52; design review

**Gated on:** -


#### PV-57 — file_read's decay is COMPOSITION — source-file utilisation RISES with context size

`finding` · **settled** · priority `high`  
*Provenance: RECOVERED-VERBATIM from the desktop `catalog.db`, 2026-08-31.*

PV-53's check applied to the surviving finding, and it eliminated it. A 128KB source file is rare; files that size are lockfiles, generated code, minified bundles, fixtures, logs — which is why file_read was 100% zero-spec above 128KB (PV-52). Split file_read by path class per band: SOURCE-file utilisation 58.9 / 63.1 / 81.9 / 81.2 / **84.4%** — it RISES with context size — while EXTERNAL-file utilisation collapses 51.4 / 70.7 / 34.3 / 23.8 / **7.7%**, and external reads are 52-92% of file_read tokens in every band. Pooled: file_read is 35.0% over all paths but **77.0%** over repo source, which is 18.4% of its token volume. So the decline was never decay in how well source files are used. CONSEQUENCE: the granularity intervention is NOT supported — codescout's source reads are already excellent and improving with pressure — and PV-7's headline falls with it. Second instance of composition masquerading as a size effect after PV-47's left arm, and the third time overall that a pooled average concealed a changing mix.

**Evidence:** round13.json

**Gated on:** -


#### PV-59 — CONSTRAINT ON FUTURE WORK — repo-source reads IMPROVE under context pressure; do not cap them

`finding` · **settled** · priority `high`  
*Provenance: RECOVERED-VERBATIM from the desktop `catalog.db`, 2026-08-31.*

Recorded as a constraint precisely because it has no intervention attached and would otherwise not survive. Source-file utilisation RISES across context-size bands: 58.9 / 63.1 / 81.9 / 81.2 / **84.4%**, ending higher than symbol_lookup (72.3%) and search (71.6%). 'Large sessions waste context, so cap file reads' is a plausible future proposal and it is REFUTED IN ADVANCE for repo-source reads — they are the best-utilised category in the corpus and they get better as context gets scarcer. Any capping or granularity policy must exempt paths that resolve in the repo symbol index; the waste is entirely in external / non-source / generated content (7.7% utilisation in the top band, 79.5% of file_read tokens there).

**Evidence:** round13.json; PV-57

**Gated on:** -

