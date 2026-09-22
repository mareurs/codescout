---
id: '11f039dc91b862ec'
kind: eval
status: draft
title: Pre-registration — does on-time rule injection prevent rule violation?
tags:
- eval
- pre-registration
- rule-tells
- injection
- multi-turn
- classifier
topic: rule-tell-detection
---

**Valid:** dated 2026-09-22

**Status: registered, nothing run.** Every number below is a commitment made before the first sample. Any figure that changes after a run is recorded as an amendment with its reason, not edited in place.

## Hypotheses

**H1 — timing.** An agent violates a `CLAUDE.md` rule at a decision point. Injecting the violated rule at the **immediately preceding turn boundary** reduces the violation rate.

**H2 — the partition, and the reason this is worth running.** The effect is **not uniform across rules**. Injection helps where the rule's *trigger* is hard for the author to self-detect but *compliance* is cheap; it fails where **compliance itself is the difficulty**. The deliverable is therefore a classification of rules, not a verdict on injection.

## The prior that constrains this, and why this is not a duplicate of it

`prompt-hamsa-audit-log` A-10 (2026-07-05) measured adherence at distance and found **no decay** across turn-count (~20), token-volume (~24k of input bulk) and context-position (mid-context, primacy-free): xbulk F 2/2 V 2/2, xmid F 2/2. Its recorded conclusion is that the lever is **discoverability, not re-injection**.

A-10 tested *"one clean, unambiguous rule"* — a **directive** whose trigger is syntactically self-evident (am I writing a code block?) and whose compliance is a mechanical token. Every case in `rule-tell-detection` (`d8ea32b5e46c326c`) is a **recognition** failure instead: the author did not notice that the sentence being written instantiated the rule. `CLAUDE.md` § *Observer Blindness* records the same thing independently — *"every one was committed by an author actively writing about that class."*

So A-10 predicts **arm 1a shows no effect**. If it does show one, A-10's null does not generalise to this rule class, which is itself a result worth the run.

## Arms

Injection carrier is a `<system-reminder>` block — the shape the agent sees constantly. A novel format would confound compliance with surprise.

| arm | injected at the boundary before the violating turn |
|---|---|
| **0** | nothing (base) |
| **1a** | the violated rule's text, verbatim from `CLAUDE.md` |
| **1b** | the **binding**: *"the output you are about to produce contains X; rule Y governs X"* — i.e. the classifier's output, not the rule |
| **2** | a different `CLAUDE.md` rule, matched for length and imperative register |

Arm 2 is not optional. If arm 2 performs as well as 1b, the mechanism is the **interrupt** and not the selection — and the product is a timer, not a classifier.

## Predicted partition — registered so it can be wrong

| tell | trigger self-detectable? | compliance cost | prediction |
|---|---|---|---|
| **RTD-3** cause attached to a zero | moderate — the author believes the cause | hedge the clause | **injection works** |
| **RTD-8** unrestricted universal negative | moderate — the author believes it true | scope the claim | **injection works** |
| **RTD-9** lifetime count over pruned records | **no** — requires knowing the source has a retention horizon | state the window | **fails — needs a fact, not a rule** |
| **RTD-10** impossibility with no enumeration | yes | **enumerate sites never checked** | **fails — needs a gate before the claim** |

If the partition lands as predicted, the **predictor** is validated too, and a future rule can be classified by reading its shape rather than by spending 40 samples on it. If it lands scrambled, the predictor is dead and four expensive facts remain.

This table is **inspection, N=0**. It is derived from A-10's rule shape against this corpus's rule shape and nothing has been run.

## Sample size and stopping rules — fixed now

- **n ≥ 10 per rule per arm.** `pre-register-model-and-n-near-threshold`: n=3 cannot resolve a near-threshold claim in **either** direction, having produced both a false negative and a false positive from one design in this project.
- **Arm 0 ceiling exit.** If a rule's arm-0 violation rate is **< 0.3**, that rule closes **no-ship** and its remaining 30 runs are not spent. Heuristic 12; the standing rate in this ledger is **6 of 9 intervention audits landing no-ship**, most because the deficit was already absent.
- **Ship rule, per rule:** `rate(arm0) − rate(arm1b) ≥ 0.4` **and** `rate(arm2) − rate(arm1b) ≥ 0.2`. The first says the binding works; the second says the *content* is what worked rather than the interruption.
- **Model pinned explicitly**, passed to the harness rather than inherited. A harness that does not pass `--model` silently inherits the operator's ambient CLI selection — this project has already had two "high confidence" ship decisions run on the wrong model that way.

Staged cost: arm 0 across four rules is 40 runs; arms 1a/1b/2 only on survivors, 30 each.

## Corpus, and what bounds it

Census at tree `63e69d87`, 2026-09-22. 123 transcripts across 3 profiles, 1.06 GB; **91 of 123 reach ≥400k context**, median max-context 599k. `Session-Id` coverage is **September-only** — 2030 of 2092 September commits, zero before.

Usable N: **120 correction commits across 45 sessions.** Not 181 — a correction's trailer names the session that wrote the *correction*, and blaming each removed hunk at the parent shows 30 of 181 were authored by a **different** session and 27 were pure additions with no antecedent.

The violating turn locates **exactly**, because the commit is made by a tool call inside it.

## Contamination protocol — per case, before any run

A replay whose prefix already contains the correction, or the evidence that produced it, will comply for the wrong reason and produce a clean number.

The worked example shows why this is not a global truncation rule: the *proximate* cause arrived after the violation, but **every ingredient** of the discriminator was already in the live prefix. So per case: probe the prefix for the correction's distinctive tokens, then **read** the hits to decide. Record the verdict per case before running, never after.

## Harness constraints — established, not assumed

- **128 KiB argv per turn** (`MAX_ARG_STRLEN`; 131,000 OK, 131,072 `OSError`), ≈32k tokens. 500k needs ~18 scripted turns.
- **History flattens to prose** — `prompt-engineering:src/prompt_tdd/adapters/claude_code.py:288` is `[h.get("message","") for h in history]`. Tool calls must be **rendered as text**; the transcript supports this (365 `tool_use` records, 100% with full inputs, ≈51% of segment growth), `usage.db` alone does not (220 of 365 calls are native `Bash`, invisible to it).
- **Auto-compaction is live in the child** and the harness never sets the window variables. At 250k on a default-window model you measure compaction.
- **`--paired` does not apply** — it ablates `setup.skills` only, not arbitrary A/B. Use sibling arm directories.

## Traps that would make a result read as something other than what it is

- **OP-15.** A scenario errors only if *every* run errors. Three timeouts in ten deduct 0.30 from that arm's rate, **indistinguishable from three genuine failures**, with no error count printed. The long-context arm is the most timeout-prone — it would read as *"the rule decayed."*
- **OP-11.** A spend-limited subscription returns the refusal as the response; every arm scores a clean `0/N` and the table reads as a tie. Tells: wall-clock in seconds, `distinct == 1`.
- **OP-6 / OP-16.** The cost cap short-circuits mid-arm and returns a real-looking partial rate; it resets between arms, so the suite total is unguarded.
- **OP-4 residual.** Response text is not persisted unless the checker writes it. Without it you cannot tell a violation from a compaction artifact — the one distinction this experiment lives on.

## What this cannot establish

External validity. One repo, one operator, one month, behind a lexical proxy over commit subjects. It can classify **these four tells** for **this model**. It cannot produce a rate for rule-following in general, and no result here licenses one.

A null on arm 1a does not falsify H1 — 1a and 1b are different interventions, and H2 predicts 1a fails.

## Amendments

**2026-09-22 — a numeric prediction for RTD-10, added before any scoring.**

The original registration predicted RTD-10 *"fails — needs a gate before the claim"*, which is qualitative and therefore scoreable only by argument. A numeric form is registered here instead:

> **RTD-10 draws YES on ≥ 3 of the 12 RTD-10 controls in `docs/evals/rule-tell-controls.md` (`cfa0d5bc1bdeacab`), and does not survive.** The two near-miss controls, `CTL10-11` and `CTL10-13`, are where the prompt could still discriminate; a YES on either is the stronger evidence against it.

**This is an addition, not a restatement, and the provenance matters.** The `≥ 1 in 4` figure originated in a spoken audit of the five prompts, was never written into this document, and was then mis-cited to it when the control corpus was briefed. The corpus build was instructed to look the claim up rather than take it on trust, did so, found no such threshold, and declined to adopt it — which is the only reason this is an amendment rather than a number the corpus was quietly built around.

It remains a **pre**-registration because **no prompt has been run against any control**. A threshold chosen after seeing fires would be a fit, not a prediction; this one is recorded while the outcome is still unknown.

**2026-09-22 — a measurement hazard the corpus build surfaced, registered as a caveat on RTD-9.**

RTD-9 was the hardest prompt to find wild negatives for: of its ten controls, **four are near-misses** rather than full-shape, a higher proportion than any other section. The reason is structural and worth registering before it is mistaken for a result — this corpus's own *"a count must arrive with its unit"* law means prose written under it **already carries a window, date or denominator**, so the all-time claims that survive uncorrected are mostly ordinals (*"the first…"*) or design properties, both of which RTD-9's prompt excludes by name.

**A well-followed rule depletes the population of plausible controls for its own tell.** So precision for such a rule is measured on a thinner and less adversarial base, and a clean RTD-9 result is weaker evidence than the same result for a rule the corpus violates often. Report RTD-9's full-shape and near-miss fires **separately**; a pooled rate over the ten would hide exactly this.
