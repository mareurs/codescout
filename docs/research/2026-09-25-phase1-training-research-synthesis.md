---
id: '03a50e2c922832bb'
kind: research
status: draft
title: 'Why the phase-1 local arms failed: four research reports, synthesised'
tags:
- evals
- rule-tell
- phase-1b
- research
---

# Why the phase-1 local arms failed: four research reports, synthesised

**Valid:** dated 2026-09-25

**Provenance.** On 2026-09-25 four research subagents were each briefed with the exact phase-1 recipe and its measured symptoms. They covered ModernBERT fine-tuning, Qwen and decoder LoRA classification, small-data stability with missing-label multi-label learning and artifacts in minimal pairs, and rule-conditioned detection with feedback injection. Each was told to cite only sources it had opened.

**Verification status** is marked per claim:
- **[verified here]:** re-measured or re-read by the coordinating session.
- **[cited]:** a source the subagent reports opening, not re-read here.
- **[derivation]:** the subagent's own reasoning.

Several venues were reported from memory; the subagents flagged these, and they are omitted below.

## 1. The optimisation recipe was unstable, for both arms

- **Qwen's head step size is large against its feature.**
  - **[verified here]** Marker hidden states of frozen JevK5 have L2 norm 155.0 (144–161) and L1 norm 5,571. Their mean pairwise cosine is 0.712, and the shared mean vector is 0.846 of the average norm (115 markers, 12 train rows, A5000).
  - **[derivation, Qwen subagent]** Adam's update is roughly a sign step (Balles & Hennig, arXiv 1705.07774). At the registered head lr of 1e-3, one step moves a logit by about 5.6 on its own example and about 3.8 on others.
  - On balanced data, a shared logit offset c gives loss |c|/2 + ln(1+e^−|c|). The observed running losses fit |c| ≈ 1.4 (0.93) and |c| ≈ 2.5 (1.32). **[derivation; the formula is re-checked here]**
  - Both seeds and the permutation null rose above chance after warmup, consistent with this. **Why one seed recovers is not reproduced.**
- **Too few optimiser steps.**
  - **[verified here]** 1,791 rows at 16 per step is 112 steps per epoch: 336 for L2 and 560 for L1. Warmup is 20 steps.
  - **[cited]** Failed runs end with training loss about ln 2, an optimisation failure. The number of iterations, not the dataset size, governs it. The baseline is 20 epochs with 10% warmup: Mosbach et al., ICLR 2021, arXiv 2006.04884, §4–§6; Zhang et al., ICLR 2021, arXiv 2006.05987, §6.
  - **[cited]** A comparable large encoder at lr 3e-5 for 5 epochs failed in about half of 20 runs: Du & Nguyen, arXiv 2302.07778, §5.2.
- **Our head design is off-recipe.**
  - **[cited]** ModernBERT's official classification head is dense → GELU → LayerNorm, with a truncated-normal classifier and one lr for all parameters. The authors' best lrs for the large model are 1e-5 to 8e-5, over longer schedules. Small tasks such as RTE start from an MNLI checkpoint: ModernBERT paper, arXiv 2412.13663, App. E.1 and Table 6; local transformers 5.17 source.
  - **[cited]** The HF LoRA recipe trains the `score` head at the body's lr (HF blog, LoRA for sequence classification).
  - **[verified here]** JevK5 itself was LoRA-trained at lr 3e-5, warmup 20, 2 epochs, according to the documented command in `jevk5/training/lora.py`.
- **An input defect: every sentence start loses its leading space.**
  - **[verified here]** For both tokenizers. Filed as `docs/issues/2026-09-25-encode-units-drops-sentence-leading-space.md`.
- **What the literature supports as remedies:**
  - normalise the marker feature, or train the head at the body's lr (≤ 1e-4), and clip the head separately;
  - LP-FT: fit the head on frozen features first (Kumar et al., ICLR 2022, arXiv 2202.10054) **[cited]**;
  - LoRA lr 5e-5 to 1e-4, at least 10% warmup, and about 1,000 or more optimiser steps;
  - put both texts of a pair in the same accumulation window, so their shared feature cancels **[derivation]**;
  - 5 or more seeds, stopping failed runs early (Dodge et al., arXiv 2002.06305, §5) **[cited]**.

## 2. The formulation: condition on the rule, and include "no violation"

- **[cited]** Classifying marked sentences beats start/end span extraction: ContractNLI, arXiv 2110.01799, §3 and Table 4. **Our marker-per-sentence design is supported.**
- **[cited]** Guard models put the policy text in the input. Llama Guard's augmentation drops categories from the input and relabels the example safe, which teaches a conditioned model to say NO outside the given rule (arXiv 2312.06674, §3.4). Laurer et al. pair texts with a random wrong hypothesis (arXiv 2312.17543, §3.3).
- **[cited]** Models need an explicit no-answer option, with a deliberately controlled ratio. Without negatives they guess (SQuAD 2.0, arXiv 1806.03822); with 99% empty inputs they always abstain (CUAD, arXiv 2103.06268, §4.1).
- **[cited]** Negatives drawn from outside the training distribution generalise, and diversity matters more than size (Outlier Exposure, ICLR 2019, arXiv 1812.04606, §5).
- **[cited]** Prompted LLMs flag spans at low precision (GPT-4-turbo 18.4% on RAGTruth, arXiv 2401.00396); fine-tuned detectors do far better. That matches our Haiku-judge failure.
- **[cited]** Fine-tuning through a model's own answer tokens (a verbalizer) beats a new head in low-resource settings (LM-BFF, arXiv 2012.15723). JevK5 already answers through letter logits, and its `training/lora.py` trains that path.
- **[cited]** "Ignore missing labels" drifts toward always predicting positive (Cole et al., arXiv 2106.09708, §4.2). Ignoring un-annotated labels above an estimated 5% prior and treating the rest as negative is the closest published analogue to our audit-then-admit plan (Ben-Baruch et al., arXiv 2110.10955, §4.1).

## 3. The data guarantees cue artifacts

- **[cited]** Edited data is unbiased only if each edited feature flips the label about half the time. Contrast sets where every edit flipped the label showed no reduction in artifacts, and more data from the same process does not help (Gardner et al., arXiv 2104.08646, Prop. 1 and §5).
  - **[verified here, earlier]** In our pairs, every edit flips the label. `&&` marks 77/77 `d_semicolon` positives; "sessionId" marks 53/95 `d_sessionid` negatives.
- **[cited]** The counterexamples belong in **training**, not only in the test (McCoy et al., arXiv 1902.01007, §7; Joshi & He, arXiv 2107.00753, §2.2 and §3.4; Gururangan et al., arXiv 1803.02324, §4).
- **Cross-rule negatives cannot remove a within-rule cue.** `&&` appears in 0 of 2,158 other rules' rows, so no admitted negative ever carries it. **[verified here, earlier]**

## 4. Evaluation and causal design

- **[derivation, stability subagent]** Reading the causal claim only over checkpoints that "learned" is **selection after treatment**, because the negatives may change how often training fails. Each arm's failure rate has to be reported as an outcome. **This invalidates the "learned ≥ 0.80" reading added to the phase-1b draft on 2026-09-25, as the only causal reading.**
- **[cited]** Report per-rule false-positive rate on clean inputs, and recall at a fixed low false-positive rate (Granite Guardian, arXiv 2412.07724, §5.1). Report claim-on-target separately from firing, because every span paper shows a large drop from example level to span level.

## 5. The injection half

- **[cited]** Specific feedback beats generic feedback (Self-Refine, arXiv 2303.17651, Table 2), and error location matters (LLMRefine, arXiv 2311.09336, §6.3). That fits phase 2's finding that the claim-bound reminder works where a bare label does not.
- **[cited]** Inject late, directly before generation (Li et al., arXiv 2402.10962; Laban et al., arXiv 2505.06120; Lost in the Middle, arXiv 2307.03172).
- **[cited]** False feedback does harm. Challenged with "Are you sure?", Claude 1.3 changed a correct answer 86% of the time (Sharma et al., arXiv 2310.13548, App. A.4). **Detector precision is the product,** and the harm of a false reminder should be measured directly.
- **[cited]** Cheap-detector → LLM-verifier cascades are well supported (Constitutional Classifiers++, arXiv 2601.04603; activation-probe cascades, arXiv 2506.10805).

## A correction to this session's own reasoning

Earlier on 2026-09-25 the coordinating session called the "head learning rate too high" hypothesis refuted. The basis was a 32-row overfit check on **ModernBERT**, whose marker L1 norm is 522. That check never measured Qwen. Qwen's marker L1 norm is 5,571, and for Qwen the hypothesis is now the leading, measured-magnitude mechanism. The refutation was over-generalised from one model to the other.

## What remains unverified

- That the head step size **caused** the seed split. That needs a run logging each rule's mean logit per step, or a rerun with a normalised feature or a lower head lr.
- Whether flash-linear-attention's DeltaNet backward pass is correct on an A5000 (sm_86). No report either way.
- A tolerable hidden-positive rate for assume-negative training. None was found in the literature; the 5% bound matches Ben-Baruch's cut-off, but is not derived from it.
- Evidence on injecting span-specific reminders mid-trajectory into an **agent** loop. None found; the nearest evidence is refinement and sycophancy studies.
