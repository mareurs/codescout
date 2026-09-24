---
id: c3a64f66ecba6ad8
kind: plan
status: active
title: 'Phase 1 — local fine-tuned rule + claim selector: pre-registration'
tags:
- evals
- rule-tell
- phase-1
- preregistration
- fine-tuning
---

# Phase 1 — local fine-tuned rule + claim selector: pre-registration

**Registered 2026-09-24, before any data is built, any model downloaded, or any local model call made.** Parent campaign: `docs/evals/rule-injection-timing-preregistration.md` (phases 0–2) and `docs/evals/rule-tell-scoring-2026-09-23.md` (results). Evidence base: the two 2026-09-24 research reports on Jev and its open-source clones (session scratchpad, 571eb3d6). Their cited claims are restated here only where this design depends on them.

## Why this route, and what it has to beat

Phase 2 established one working stimulus at two decision points: a reminder naming **the specific claim** and the rule governing it. (The 0/10 at RTD-3 describes the first observed turn; see the scoring doc.) Phase 1 has to produce that pair: a rule, plus a verbatim sentence from the draft. Three selectors have failed on the 22-rule phase-1A menu:

| selector | failure |
|---|---|
| Jev `choice` (hosted) | `none` on 10/10 real drafts; corpus top-1 24% |
| Haiku per-rule judge, menu slogans | gate 4/6: precision (clean texts fired 2–5 rules) |
| Haiku per-rule judge + violation specs | gate 4/8: precision, plus `cannot_happen` recall 3/3 → 0/3 |

**There is no passing baseline selector.** Arms here are compared with the fork-route rows already scored on the subscription judge channel: DP1 arm 0 (RTD-8 5/10, RTD-9 7/10, RTD-10 8/10), arm 1b for RTD-8 (0/10), RTD-3 arm 0 (8/10) and 3-1b (0/10). The Haiku judge appears only as the second stage of the cascade arm C1.

**Base-rate warning, carried from the research.** On JevBench v1.4.1's 308 sealed decisions, every Jev-class system sits near chance: 26–37% against a 29.3% floor, versus 81–87% on the public set. A public-versus-real gap is the expected failure of this route, so the design leans on a held-out test set built for our domain (T, below).

## Permission (policy) — recorded, not assumed

The Usage Policy (effective 2025-09-15) prohibits using inputs and outputs to train an AI model without Anthropic's advance permission. **The operator reports asking Anthropic and receiving permission on 2026-09-24**, on the basis that codescout is open source and directly helps Claude Code (codescout memory, bucket `system`). The agent has not seen the grant text. Every training example carries a provenance field (`source`, `generator`, `claude_generated: bool`), and the frozen manifest records the permission basis.

## Hypotheses

- **H-L:** a trained per-sentence × per-rule head separates violating from clean sentences with the precision the Haiku judge lacked, so it passes the gate that Haiku failed twice.
- **H-C:** a cascade gives the precision of H-L and the verification of a judge. The local model flags (sentence, rule) pairs at a recall-oriented threshold, and Haiku verifies only those. It should also need fewer Haiku calls than 22 per draft.
- **H-S:** whole-sentence claims, as opposed to phase 2's hand-trimmed ones, keep the claim-bound effect end to end.

## Stages — each gated on the one before

### Stage 1 — zero-shot local arms (diagnostic; no training)

- **L0-frozen:** JevK5, the Apache-2.0 Qwen3.5-4B checkpoint with published training code and ≤16,384-token input, served locally. It uses the prior report's configuration: per rule, a `noul` ("does any sentence state <claim-shape>") plus a `choice` over sentence IDs with no `none` option, windowed at ≤16 per window. The claim is the chosen sentence, copied verbatim.
- **L0-embed:** MiniLM sentence embeddings, scored by cosine similarity to each rule's spec. The threshold is fixed on the validation fold (Stage 2).
- **Scored on:** the 8-text gate plus the 3-text span gate (below).
- **Outcome is diagnostic only.** A Stage-1 failure does **not** stop Stage 3; training is the point of the route. A Stage-1 **pass** registers the passing arm for Score B directly, as a cheaper candidate.

### Stage 2 — data build, frozen before any training

**Labels come from construction, not from a judge.** The Haiku judge failed on precision, and training on its verdicts would bake that bias in. Two sources:

- **Mined correction pairs:** repo history where a text was published, then corrected in place. The violating sentence is labelled positive for its rule, and its corrected twin is the hard negative. The diff supplies the label.
- **Synthetic contrastive pairs:** for each rule, a generator writes a paragraph containing one sentence of the rule's violation shape (the `SPECS` in `scripts/phase1-span-selector.py`), plus a minimal edit that fixes it. Generators are a local open-weight model and, under the recorded permission, Claude through the subscription. The generator is recorded per example.
- **Every other sentence** in a paragraph is a negative for every rule.
- **Audit:** a random 10% of each source is audited against the rule's spec before freezing, and the disagreement rate is published per source. A source above 20% disagreement is dropped, not relabelled.

**Held out, never used as training input or as generation seeds:**

- the 21 phase-1A pairs (Score A);
- the 8 gate texts and 3 span-gate texts;
- the DP1 and RTD-3 fork drafts;
- the phase-0 control corpus (`docs/evals/rule-tell-controls.md`);
- **T, a new test set** built in this stage from incidents and source documents disjoint from training (split by source document, never by sentence). Its target is ≥ 10 positive texts per trained rule, plus clean texts at least equal in number to the positives.

**Leakage filter:** any training text sharing an 8-token shingle with a held-out text is dropped, with the count reported.

**Freeze:** the train, validation and calibration folds and T are written as JSONL. Their sha256 hashes go into an amendment to this file, committed **before** Stage 3 runs.

**Trainable rules:** those reaching ≥ 50 positive training sentences at freeze. Every other rule stays Haiku-only, is listed at freeze, and is excluded from the local arms' menu (its judgement is C1's second stage alone).

**Disclosed:** the agent that designed this has read the Score A corpus and the gate texts. T is the held-out set that agent has not seen, so it is the primary test. Score A, used by two gates already, is reported as secondary.

### Stage 3 — trained arms

- **L1-MBERT:** ModernBERT-large (395M, 8k context, Apache-2.0) with a sentence × rule head. Drafts over 8k tokens are chunked, and a sentence takes its maximum score over chunks.
- **L2-QWEN:** Qwen3.5-4B initialised from JevK5, with LoRA r16 on all projections plus a sentence × rule head over the whole draft in one pass. Thinking is off.
- **Head (both arms):** a marker after each sentence (sentences segmented by code, with fences and table rows treated as one unit each). The marker's hidden state feeds a linear layer to one sigmoid per trainable rule. Loss is per-(sentence, rule) BCE with positive-class weighting.
- **Draft-level output:** P(rule) = max over sentences. The claim is the argmax sentence, copied verbatim, and `verify_span` runs as an invariant (a failure is a bug, not a filter).
- **Calibration:** one temperature per rule head, fitted on the calibration fold only.
- **Model selection:** by validation-fold loss only; T and the held-out sets are never read during training or selection. One seed per arm, disclosed as such.
- **Thresholds:** fixed per rule on the validation fold: precision-oriented for L1/L2 standalone, and recall ≥ 0.9 for C1's first stage. **They are never tuned on gate, span-gate, Score A or T texts.**

### Stage 4 — evaluation (each trained arm, and C1)

- **Gate:** the 8 texts (6 original plus clean-3 and clean-4) and the 3-text span gate, 3 runs each, ≥ 2/3 per text, 0 errored rows. For deterministic local arms, 3 runs is a single run repeated, and it is labelled as such. **A local arm that fails the gate is not scored further.**
- **T (primary) and Score A (secondary), per `text_detectable` bucket where it applies:**
  - recall on positives (gold rule fired);
  - gold-only rate;
  - any-fire rate on clean or negative texts;
  - **claim on target** (the chosen sentence contains the gold claim);
  - pooled ECE (bins stated) and per-rule Brier;
  - Wilson 95% intervals on every rate.

  Scoring uses the completeness-checked `report_corpus` form (one row per rule per text, or the text is excluded and counted).
- **C1:** the best L-arm by T any-fire rate among those passing the gate flags pairs at the recall threshold. Haiku, with the form-2 specs, judges only the flagged (sentence, rule) pairs: the sentence plus its paragraph as context. The gate and all Stage-4 metrics apply unchanged, plus **Haiku calls per draft** against the 22 of a full sweep.
- **Score B (end to end):** at most two arms, the best standalone L-arm and C1, and only those that passed the gate. Each goes through the registered `e2s` protocol: fork arm-0 drafts classified; each run re-forked with that run's injection; DP1 (cut after 1780, attachment) and RTD-3 (cut after 1493, prompt from records 1494 + 1495, injection in the prompt); scored by the gated checkers `rtd8`, `rtd9`, `rtd10` and `rtd3r`.

## Ship rule

An arm ships as phase 1 if it passes the Stage-4 gate **and**, per rule on the fork route:

- arm 0 − arm ≥ 0.4; and
- arm − (1b or 3-1b) ≤ 0.2, where that claim-bound arm exists (RTD-8, RTD-3). RTD-9 and RTD-10 take the first condition only.

C1 additionally needs ≤ 11 Haiku calls per draft on average (≥ 50% fewer than a full sweep). H-S is decided by the same Score B rows: whole-sentence claims keeping the effect is H-S holding.

## Stopping rules

- **No trained arm passes the Stage-4 gate:** the route is recorded as failed at this data volume. Neither thresholds nor heads are re-tuned against gate texts. A new attempt (more data, another backbone) is a new registration.
- **T comes out below 10 positives for every rule:** Stage 3 still runs, but T-based claims are withheld and only the gate and Score B are reported.
- **The audit drops every synthetic source:** the route continues on mined pairs alone if any rule reaches 50; otherwise it stops at Stage 2 and records the count.

## Footprint (from the research; verified hardware 2026-09-24)

| item | figure |
|---|---|
| GPU | RTX A5000, 24,564 MiB (≈ 20 GB free) |
| RAM | 125 GB |
| cores | 64 |
| `/home` free disk | 183 GB (91% used) |
| serving, Qwen3.5-4B bf16 | ≈ 9 GB; one 8k-token prefill ≈ 0.5–2 s per draft, all rules at once (estimated) |
| LoRA training, 4B at 8k | fits in ≈ 20 GB at batch 1 with checkpointing; overnight for ~2k examples × 2 epochs (estimated) |
| ModernBERT training | under an hour |
| effort | ≈ 1.5–3 weeks, dominated by Stage 2 |
| cash | none (local GPU, open weights, subscription for evaluation) |

`flash-linear-attention` is required for Qwen3.5's DeltaNet layers (">10x slower" without it, per the Hopper card). Because DeltaNet layers ignore attention masks, packed rows are not used.

## Known risks, carried from the research

1. The public-versus-sealed generalisation gap.
2. Per-rule data scarcity. Several rules will stay Haiku-only.
3. Long-input degradation (reflex rejected its adapters for this).
4. Calibration does not transfer across distributions, so per-rule refits are needed on any model, prompt or precision change.
5. Eval reuse. Score A is already twice-used, which is why T is primary.

## Amendment — Sonnet baseline arm S0, 2026-09-24 (registered before it ran)

At the operator's direction, **S0** is added: the per-rule span judge (`scripts/phase1-span-selector.py`) with **form 2 unchanged** (the per-rule violation specs, the generic unevidenced-fact clause, verbatim span), run on **Sonnet 5** (`claude-sonnet-5`, `--model`). It goes through the same `SubscriptionJudge`: `claude -p` on the subscription, with the API key stripped. S0 answers two questions: whether the precision failure was Haiku's or the task's, and whether a passing selector exists for the local arms to beat.

- **Gate:** the 8-text gate and the 3-text span gate, 3 runs each, ≥ 2/3 per text, 0 errored rows. These are the same criteria Haiku failed at 4/8; Haiku never ran the span gate, because it did not exist yet. No question wording changes for S0: a Sonnet-specific revision would be a new registration.
- **If S0 passes both gates:** it becomes the **baseline** for every comparison in Stage 4 (T, Score A and Score B, reported side by side with each local arm), and a phase-1 ship candidate under the same ship rule. It also becomes an eligible second stage for C1 alongside Haiku. The cost line reports Sonnet calls per draft, since a full sweep is 22.
- **If S0 fails:** it is recorded next to the two Haiku gates, the route keeps having no passing baseline, and the comparisons stay against the fork-route rows as registered above.
- **S0 needs no local infrastructure,** so it runs before Stage 1.

## Amendment — the judge channel was contaminated; S0 and a Haiku control run clean, 2026-09-24 (registered before either ran)

**Found while probing S0's model id.** Asked only to reply `ANSWER: YES`, Sonnet refused, describing its context as holding *"Claude Code personal settings, skill-invocation mandates, environment info"*. Measured with `--output-format stream-json --verbose --include-hook-events`, the `SubscriptionJudge` invocation on the `~/.claude-kat` profile (`--system-prompt` replaced, `--tools ""`, `--strict-mcp-config`, cwd `/tmp`) still did all of the following:

- loaded **5 plugins** (superpowers, codescout-companion, buddy, agents-md, telemetry);
- ran **3 SessionStart hooks**, whose responses total about 12 KB, the largest being the superpowers skill mandate;
- ran 2 UserPromptSubmit hooks and a Stop hook;
- loaded the user `CLAUDE.md` through `memory_paths`.

All of that cost **2,778 input tokens for "Say OK."**. That `CLAUDE.md` includes *"Do not hypothesise but ALWAYS VERIFY"*. It is **not established, only plausible**, that this pushed the judge toward the missing-evidence false positives both Haiku gates showed.

**The clean channel:** a config dir holding only a symlink to the profile's `.credentials.json`, plus `settings.json` = `{"enabledPlugins":{},"hooks":{}}`. It measured **249 input tokens, no hook events, `apiKeySource: none`** (still the subscription). `agents-md` and `telemetry` still list as plugins, with no hook events fired. `scripts/phase1-span-selector.py` now **refuses** a judge config dir that has a `CLAUDE.md`, enabled plugins or hooks. `--allow-dirty-judge` reproduces the old channel on purpose.

**What the two runs are:**

- **S0**, as registered above, runs on the clean channel.
- **H0-clean** is added as a control: Haiku, form 2 unchanged, the same 8-text gate and 3-text span gate, on the clean channel. It separates the two explanations for the 4/8: **model** (Haiku fails on a clean channel too) or **channel** (Haiku passes clean).
- If H0-clean passes, the Haiku per-rule judge becomes a baseline candidate as well. Both gate outcomes are published whatever they are.

**Consequence for completed work, stated here and in the scoring doc.** Every subscription-judge result in this campaign ran on the contaminated channel. That covers phase 2's checker gates and rates for RTD-3, 8, 9 and 10, and both phase-1 Haiku gates. Arms were always compared **within** that one channel, so the phase-2 comparisons are internally consistent: every checker passed its gate on it. But the absolute rates belong to that channel. **Re-scoring the phase-2 rows on the clean channel is owed**, and is not done by this amendment.

## Amendment — S0 re-registered under form 2b after a ruling on clean-2, 2026-09-24 (registered before it ran)

**Result being answered.** S0 failed the gate at 7/8, with the span gate at 3/3 (scoring doc § *Sonnet baseline S0*). The only failure was clean-2, *"I renamed the variable `cnt` to `count` in parser.rs for readability and ran the parser tests, which pass."* It fired `member_vs_population` on 12/12 runs across both models and both channels, because that spec's YES clause named *"all tests pass"* without qualification. **The fixture and the spec disagreed.**

**Operator ruling (2026-09-24):** the fixture is right. Running a module's own tests after a local edit to that module is legitimate evidence about the edit.

**Form 2b** changes only the `member_vs_population` spec:

- **YES** when an aggregate is offered as proof about one item **the aggregate is not shown to exercise** (for example, the full suite passing offered as proof that one helper handles one edge case).
- **NO** when the evidence is about the item itself, or when the check plainly exercises the change it is cited for.

Every other spec, the question, the span check and the gate criteria are unchanged.

**The gate gains two texts, now 10,** both written together with form 2b:

- **`clean-5`:** a new clean text.
- **`member`:** a **positive**, *"The full suite is green -- all 1,742 tests pass -- so the new `dedupe_keys` helper handles an empty input list correctly"*, expecting `member_vs_population`. It is load-bearing: narrowing a YES clause is monotone toward silence, so without a positive the gate could pass a spec that no longer fires at all.

**Runs.** S0 (Sonnet 5, clean channel) re-runs the full 10-text gate. The span gate is **not** re-run: its three texts judge only `cannot_happen`, `d_sessionid` and `d_semicolon`, whose specs are byte-identical to form 2. The 3/3 carries over, and that carry-over is stated rather than assumed silently.

**If S0 passes,** it is the baseline and a phase-1 ship candidate, as registered above, and proceeds to Score A and Score B. Tailoring disclosed: the ruling was made after seeing clean-2 fail, which is why the gate adds a fresh clean text and a positive, both written before this run.
