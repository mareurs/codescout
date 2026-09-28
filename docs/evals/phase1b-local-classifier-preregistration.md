---
id: dc7cc43e2314170a
kind: plan
status: draft
title: 'Phase 1b — local rule selector with cross-rule negatives: pre-registration'
tags:
- evals
- rule-tell
- phase-1b
- preregistration
- fine-tuning
---

# Phase 1b — local rule selector with cross-rule negatives: pre-registration

**Valid:** dated 2026-09-25

**Status: Stage 1 registered in `24426921` and complete (§ *Stage 1 results*: both recipes stable; `s1-r1` carried forward). Stage 2 registered on 2026-09-26 by the commit that marks its heading below "registered before it runs", and complete on 2026-09-27 (§ *Stage 2 results*): no checkpoint passes the gate, NC does not proceed to T, and the local route stops for this data and backbone. Later stages are not registered.**
- **Stage 1,** a stable training recipe, runs after that commit.
- **The sections from "Step 1 — the cross-rule audit" onward** are Stage 2's steps, amended by § *Stage 2* and registered with it. Of the revisions the research synthesis (`docs/research/2026-09-25-phase1-training-research-synthesis.md`) asked for, Stage 2 takes cue counterexamples (arm NC), all-cells validation, and the reading over every seed in place of the "learned-only" one. The rule-conditioned formulation is Stage 3's, run only if Stage 2 fails. Each stage is registered before it runs.

## Why this exists

Phase 1 (`docs/evals/phase1-local-classifier-preregistration.md`) stopped under its own rule, *"no trained arm passes the Stage-4 gate"*, and that rule says a new attempt is a new registration. This is that registration.

**What phase 1 showed, as corrected after the Codex review of Stages 3 and 4:**
- **L2-QWEN detects well within a rule:** all 3 applicable gate positives hit, and the span gate passed 2/2.
- **It fires on clean text:** 5 of 5 clean gate texts fire, confidently (`d_sessionid` p = 0.79–0.99).
- **On in-distribution val texts,** its heads fire at 39% (2,614/6,773) on *unlabelled* cells from other rules' texts. That is a firing rate, not a demonstrated false-positive rate.
- **The candidate explanation:** each head was trained only on its own rule's near-miss pairs, with every other cell masked, so nothing taught it to say NO to text about something else. Codex's review called this plausible and causally untested.

**Phase 1b is that causal test.** It changes one thing in training, **adding audited cross-rule negatives**, plus the evaluation changes that follow from it. Everything else stays fixed.

## Carried unchanged from phase 1

- **The frozen data:** the Stage 2 freeze, the same seven sha256s. T, `tsyn-in` and `tsyn-cross` stay unread until every choice below is fixed, as in phase 1's amendment 1.
- **The arm:** L2-QWEN only, with phase 1's Stage-3 execution settings. JevK5, LoRA r16/α32/dropout 0.05 on all linear projections, learning rates 2e-4 (body) and 1e-3 (head), 3 epochs, batch 1 × 16 accumulation, 6% warmup then linear decay, clip 1.0, zero-initialised head, seed 20260935, the `<|box_end|>` marker, and one pass over the whole draft.
  - **L1-MBERT is not retried.** Whether its settings, not the backbone, caused its failure is a separate question, and it would double the scope.
- **Hardware:** the RTX A5000 only, for training and scoring. Phase 1 measured ROCm backend noise flipping calibrated decisions.
- **The gate code:** `scripts/phase1-span-selector.py` through an adapter, as in phase 1's Stage-4 amendment. The pass criteria, the determinism rule (one run, labelled), and the ship rule are all unchanged.

## Measurements taken before registration

Registered in phase 1 (§ *Diagnostics after the stop*) before they ran. **Phase 1b is registered only if the permutation null passes.**

**Results, 2026-09-25,** from commit `02511d99`. The detail is in phase 1, § *Diagnostics — results*.

- **The permutation null passes:** pooled val AUC 0.498, and every epoch's val loss is at least 0.68. *Corrected 2026-09-26, after the Codex review (`docs/research/2026-09-26-codex-phase1b-stage1-review.md`):* this sentence first read "the split carries no link but the labels", which is more than a null can show. A near-chance shuffled-label run shows that this run recovered no held-out label signal once labels were shuffled. It does not certify that every leakage path is absent, and the same recipe sometimes failed to learn real labels. It is a diagnostic that passed its registered band, and phase 1b's precondition is that band.
- **The phase-1 recipe is seed-unstable.** At seed 20260937 it did not learn: val loss 0.694, pooled val AUC 0.525, inside the null band. Phase 1's seed learned (0.971).
  - Both seeds' train loss rises above chance after warmup. One recovers and one does not.
  - **One of two seeds learned.** A phase-1b run on this recipe can fail from instability alone, whatever the negatives do.
- **12 of 14 rules are separable by target-unit tokens** (probe val AUC ≥ 0.9).
  - `&&` marks every `d_semicolon` positive and appears in no other rule's rows, so no cross-rule negative can teach the head that `&&` alone is not the rule. That supports keeping the swap text `clean-12`.
  - The word "sessionId" marks 53 of 95 `d_sessionid` negatives and no positive. "Fire unless the fix's word is present" is a candidate mechanism for phase 1's clean-text fires. It is not tested.
- **Audit arithmetic,** from the manifest alone (`phase1b/diagnostics/audit-arithmetic.txt`):
  - A head gets about 267–284 audit cells, and is admitted only if at most 6 are flagged (7 for `member_vs_population`). That is about 2% under the union rule, so rules whose subject is common in engineering prose may not be admitted.
  - About 16.8 pairs are expected to contribute both texts to a 300-row sample.

**What these results change in the design:**

1. **A checkpoint must have learned before it counts in the causal comparison.** "Learned" means pooled val AUC over own cells of at least **0.80**, fixed now. Phase 1's seed is at 0.971, and the null band's top is 0.55.
   - A checkpoint below 0.80 is reported, and excluded from the causal reading. It is neither a pass nor a fail there.
   - **D2 (S) is at 0.525, so it is excluded.** It would fail the gate because it learned nothing, not because it lacks cross-rule negatives, and counting it as a failing control would inflate the causal claim.
   - For the ship rule, an L2-1b below 0.80 is recorded as **not learned**. It is not gated, and it counts as a failed attempt.
2. **Open, and blocking registration: the recipe itself.** With one of two seeds learning, phase 1b's gate would partly test training stability, not cross-rule negatives. There are two ways forward; the operator decides between them before this draft is registered.
   - **Keep the recipe and add seeds:** train three or more seeds per configuration, and read the causal claim only over the checkpoints that learned. It is cheaper to design, and runs may be wasted.
   - **Stabilise the recipe first:** select the recipe on val, on the phase-1 objective without cross-rule negatives, over a small registered grid (for example, peak learning rates for the body and head), two seeds per cell, choosing the cell with the best worst-seed pooled val AUC. Then retrain **both** D and L2-1b with that recipe, two seeds each, so the causal comparison is one recipe, with and without the negatives. This costs more GPU time, and the comparison is cleaner.

## Stage 1 — a stable training recipe, 2026-09-25 (registered before it runs)

**Why first.** The phase-1 recipe learned at one seed of two, and both seeds' train loss rose above chance after warmup. The research synthesis traces that to the optimiser setup:
- a head learning rate of 1e-3 on a marker feature of L2 ≈ 155, most of it one shared direction;
- 336 optimizer steps, against the documented thousands;
- a 20-step warmup.

Until a recipe learns reliably, a gate result partly measures training luck, so every later stage waits on this one.

**What changes, and what does not.** The **objective is phase 1's**, unchanged: the same frozen train, val and cal; per-rule heads; one labelled cell per row, with every other cell masked. Only the recipe changes, so Stage 1 isolates optimisation. T, the T-syn sets and the gate texts are not read.

**Code:** `train_arm.py`, `--recipe`. `phase1` stays the default and takes phase 1's path. Every value below comes from the research synthesis, and none was tuned:

| | phase1 (reference) | s1-r1 | s1-r2 |
|---|---|---|---|
| LoRA / head lr | 2e-4 / 1e-3 | **1e-4 / 1e-4** | 1e-4 / 1e-4 |
| optimizer steps | 336 (3 epochs × accum 16) | **1,120** (5 epochs × accum 8) | 1,120 |
| warmup | 6% | **10%** | 10% |
| leading-space fix | no | **yes** | yes |
| pair in one accumulation window | no | **yes** | yes |
| head and body clipped separately | no | **yes** | yes |
| LayerNorm (no affine) on the marker feature | no | no | **yes** |

Common to all three: JevK5 with LoRA r16/α32/dropout 0.05 on all linear projections (the DeltaNet `in_proj_a`/`in_proj_b` question is left open), a zero-initialised head, AdamW with default betas, weight decay 0.01 on the body, bf16 autocast, and the A5000.

**Seeds:** 20260935, 20260937 and 20260940. That is three per recipe, six runs. The literature uses 5 to 25; three is a cost-bounded floor, disclosed as one, and the decision below needs 3 of 3.

**Runs are not bit-reproducible, measured.** Two 64-row smoke runs of the same code and seed gave train losses of 1.50213 and 1.50289. So a seed labels a run and does not reproduce it, and recipes are judged by counts over runs.

**Measured per run:**
- **From `phase1b/diagnose_run.py` on the selected checkpoint:** pooled and per-rule val AUC over own cells, and cross-rule firing on val.
- **From the run's own log:** val loss by epoch, the selected epoch, the final-epoch train loss, and a step log every 50 optimizer steps (head weight norm, mean logit, mean |logit|). The step log lets the step-size mechanism be observed rather than inferred.

**Definitions, fixed now:**
- **learned:** pooled own-cell val AUC at least **0.80**;
- **failed:** pooled val AUC inside the null band [0.45, 0.55], or final-epoch train loss at least **0.65**;
- **weak:** neither; reported.

**Decision:**
- A recipe is **stable** if all three of its seeds learned.
- **If both recipes are stable,** the one with the higher worst-seed pooled val AUC is carried forward. A difference under 0.005 counts as a tie, which goes to `s1-r1`, the one with fewer changes.
- **If exactly one is stable,** it is carried forward.
- **If neither is stable,** Stage 1 fails, and phase 1b stops before Stage 2 with the failure counts reported. The next attempt changes the formulation, for example JevK5's own answer tokens with the rule in the prompt, under a new registration.
- The chosen recipe goes into Stage 2 unchanged.

**Reported, not gating:** each recipe's failure count, with Wilson 95% intervals; and cross-rule firing on val per learned run. Stage 1 does not touch the over-firing mechanism, so this is Stage 2's baseline.

**Hardware and cost:** the A5000, with two runs at a time and three per lane in sequence, about 8.7 GB each. Roughly 4–5 hours.

**Predictions:**
1. `s1-r1` learns at 3 of 3 seeds.
2. `s1-r2` learns at 3 of 3 seeds.
3. **No Stage-1 run shows the overshoot signature,** a running train loss above 0.75 in epoch 0. Phase 1's two seeds reached 0.925 and 1.316.
4. The chosen recipe's worst-seed pooled val AUC is at least 0.90.
5. **Cross-rule firing on val stays at 25% or more in every learned run.** Fixing the recipe does not fix over-firing; that is Stage 2's target.

## Stage 1 results, 2026-09-25

**Run** at `24426921` (the commit recorded in the run root), on the A5000, in two lanes of three runs each, 16:37–20:27 EEST. All six training runs and all six diagnose runs exited 0. The per-run outputs are in `phase1b/stage1/`: the diagnose output as `<recipe>-<seed>.json` and `.txt`, and the table below as `summary.json` and `summary.txt`, from `phase1b/stage1_summary.py`, which applies the registered definitions.

| run | kept epoch | its val loss | pooled val AUC | per-rule min | epoch-0 max running loss | final train loss | cross-rule firing on val | class |
|---|---|---|---|---|---|---|---|---|
| s1-r1 · 20260935 | 2 | 0.225 | 0.982 | 0.880 | 0.692 | 0.0103 | 3,537 / 6,773 (52%) | learned |
| s1-r1 · 20260937 | 0 | 0.251 | 0.963 | 0.796 | 0.692 | 0.0071 | 2,860 / 6,773 (42%) | learned |
| s1-r1 · 20260940 | 0 | 0.294 | 0.955 | 0.818 | 0.691 | 0.0095 | 2,837 / 6,773 (42%) | learned |
| s1-r2 · 20260935 | 2 | 0.254 | 0.975 | 0.827 | 0.693 | 0.0107 | 3,566 / 6,773 (53%) | learned |
| s1-r2 · 20260937 | 0 | 0.299 | 0.952 | 0.778 | 0.693 | 0.0124 | 2,981 / 6,773 (44%) | learned |
| s1-r2 · 20260940 | 2 | 0.307 | 0.966 | 0.858 | 0.693 | 0.0123 | 3,652 / 6,773 (54%) | learned |

**Decision, as registered:**
- **Both recipes are stable:** each learned at 3 of 3 seeds, with a Wilson 95% interval of 0.44–1.00, and failed at 0 of 3 (0.00–0.56).
- **Worst-seed pooled val AUC:** `s1-r1` 0.9551, `s1-r2` 0.9522. The difference, 0.0029, is under 0.005, a tie, which goes to `s1-r1`. `s1-r1` is also the higher of the two, so the tie rule and the plain comparison agree.
- **`s1-r1` is carried into Stage 2 unchanged.**

**Predictions:**
1. `s1-r1` learns at 3 of 3 seeds. **Held.**
2. `s1-r2` learns at 3 of 3 seeds. **Held.**
3. **No run shows the overshoot signature** (a running train loss above 0.75 in epoch 0). **Held.** Running means are logged every 200 rows, so this is read at that resolution, as phase 1's were. In every run the first logged mean, 0.691–0.693, is epoch 0's maximum. That is the loss at initialisation (ln 2 with a zero-initialised head), and no later logged mean in epoch 0 is higher. Phase 1's two seeds reached 0.925 and 1.316.
4. The chosen recipe's worst-seed pooled val AUC is at least 0.90. **Held,** at 0.955.
5. Cross-rule firing on val stays at 25% or more in every learned run. **Held,** at 42–54%.

**What Stage 1 shows:**
- **The recipe now learns reliably:** 6 of 6 runs, against the phase-1 recipe's 1 of 2. Three seeds per recipe still bound the failure rate only loosely, at up to 0.56 each.
- **It did not make the model more accurate.** Pooled own-cell AUC of 0.952–0.982 is the range of phase 1's one learned seed, 0.971. Own-cell AUC is read on twin pairs, which a bag-of-tokens probe separates at 0.9 or more for 12 of 14 rules.
- **Over-firing is unchanged, as prediction 5 expected.** Cross-rule firing is 42–54%, against phase 1's 39%.
  - The `d_semicolon` head fires on 479 of 479 other rules' val cells in all six runs.
  - The `d_sessionid` head fires on 470 or 471 of 471 in five runs, and 182 in the sixth (`s1-r2` · 20260937).
  - `d_loudness` and `d_visibility` fire on 0–36 of 491 in every run.
- **The selected epoch depends on the seed.** Three runs kept epoch 0 and three kept epoch 2. Every run overfit after its kept epoch: by the last epoch, train loss was about 0.01 and val loss 0.308–0.384.
- **Calibration.** In every run, the temperature is at the lower bound, 0.25, for `d_semicolon` and `d_sessionid`. It is also at that bound for `d_red` (3 runs), `d_loudness` (1) and `open_artifact` (1). A temperature at the lower bound means those heads' cal items were perfectly separated, so the fit carries no information for them. *(Correction 2026-09-28, review-catches:RC-43: a bound alone does not prove separation. What establishes it here is Codex's check of the cal logits in all 17 run/head cases (`docs/research/2026-09-26-codex-phase1b-stage1-review.md`), each of which was in fact perfectly separated. The conclusion stands on that check, not on the bound.)*

**Reported, not registered:**
- **Own-negative firing on `cal`** at each head's precision threshold: 9–21 of 180 own negatives (5–12%).
- **Own-positive recall on `cal`:** 126–154 of 179 (70–86%).
- Thresholds are chosen on `val`, so the diagnose step's `own_negatives_fired`, counted on `val`, is set by the threshold rule and is not reported (bug `2bac7e0a27fbc392`).

**Checkpoints,** outside the repo under `~/work/claude/rule-tell-runs/phase1b-s1/<run>/best.pt`, by sha256:

| run | sha256 |
|---|---|
| s1-r1 · 20260935 | `dd89ea46236173d2fba66616c868aa5a46db4edf5575a262f79a5b1b2f710c05` |
| s1-r1 · 20260937 | `5fafbcc61569163f9284ce0b5a90d70f35a9e335aaf7b864c44399e60bf387a3` |
| s1-r1 · 20260940 | `e8690c86cff4fb1607f63c28040f860f487ee21d3348b82dee46c50fff00442f` |
| s1-r2 · 20260935 | `bd97bb2b0781745ca9ae87e99e284c35b2fe7c418ea6f8cf017e94ba2aa84489` |
| s1-r2 · 20260937 | `816feb89144e5e5eb73e3d7d9d3b0b3fb9b4d35e35f110593056b97c3440bff2` |
| s1-r2 · 20260940 | `be99f4c90444f648b26ef7c84363b79343380088f04eb51cd0ecc8389e480ea1` |

The three `s1-r1` checkpoints are Stage 2's arm B.

## Stage 2 — cross-rule negatives, 2026-09-26 (registered before it runs)

**Status: registered 2026-09-26, before any Stage 2 data is drawn and before any Stage 2 run trains.** It was drafted while Stage 1's last seed was training, and registered after § *Stage 1 results*, the operator's decisions and the operator's confirmations below. Steps 1–5 below are Stage 2's steps; this section states what changes in them, and where the two disagree, this section wins.

**Registration, 2026-09-26:**
- **Committed before registration:**
  - Stage 1 results, and the recipe named (`c69a99e4`).
  - The operator's decisions and the cue list (`29506f12`).
  - The counterexample miner, with its cross-fold fix after the Codex review (`ec12b2a1`, `96b52f0c`).
  - `train_arm.py --cross` and `--extra-rows`, with their parity checks (`f566082c`).
  - Step 4's scripts, `phase1b/score_run.py` and `phase1b/step4.py`, which also take Stage 2's per-run measurements (`f78c0eba`, `be87d3e4`; § *Step 4's scripts* below).
  - Step 1's scripts, `phase1b/draw_audit_sample.py`, `phase1b/label_items.py`, `phase1b/score_audit.py` and `phase1b/run_labellers.py` (`4606a129`, `d423236b`; § *Step 1's scripts* below).
  - None of Step 1's scripts has run, and the miner has only run with `--count-only`.
- **Confirmed by the operator, 2026-09-26:**
  - every arm is scored on one cell set (§ *Step 4's scripts*);
  - the six choices listed in § *Step 1's scripts*;
  - predictions 1–5 as drafted in `2e4743e0`, unchanged after Step 4's smoke run;
  - the earlier draft's audit prediction, carried as prediction 6.
- **Then, in this order:**
  1. The three Codex clean texts, from the committed prompt (§ *Step 2*).
  2. The audit sample (`draw_audit_sample.py`) and the counterexample candidates (`mine_counterexamples.py`, which refuses to draw before the Codex texts exist).
  3. The labellers' items (`label_items.py`), the two labellers (`run_labellers.py`), then the scorer (`score_audit.py`): the admission file, NC's counterexample rows and the clean-text verdicts.
  4. N and NC training, three seeds each.
  5. Step 4 for all nine checkpoints (`score_run.py`, then `step4.py`), then Step 5.

  *Corrected at registration:* the draft listed mining after the audit labelling, but the candidates are labelled in the same runs, so they are drawn first.

- **Added after registration, 2026-09-26:** `phase1b/gen_codex_clean.py` (`58c91dad`), the Step 2 generator the registration did not commit. It implements the registered prompt and channel, edits no registered script, and has not run.
- **Amended after registration, before any labelling, 2026-09-26:** `phase1b/run_labellers.py` (`14346eb4`, patch-id `c453deb0228aff223e47e6346ee778cdcab66d6f`), from a Codex preflight review (`docs/research/2026-09-26-codex-phase1b-labelling-preflight-review.md`; bugs `ea84bb940596ddf3`, `a7f3a7c8ad7d352a`).
  - A terminal failure now stops pending batches in completion order; before, it went unseen behind a slower earlier batch.
  - An exclusive reservation refuses a relaunch; before, a second invocation re-ran every call and overwrote the first run's evidence.
  - It changes no prompt, item, retry budget or admission rule. It makes the registered "a second failure stops Step 1" and "one run" hold in the code.
- **The Claude labeller's channel, checked 2026-09-26** (`phase1b/channel-check/`; bug `1fb2cb7b4614e8b5`). Syncing was made impossible in a control directory, with the model, prompt and flags fixed. The labeller's input read 412 tokens in both directories, so the 8 claude.ai skills synced into `judge-config-main` are registered but not in its context. About 390 tokens beyond the prompt, common to both, are not attributed.

**What Stage 2 tests.** Stage 1 changes optimisation only, and Stage 1's registered prediction 5 expects its learned runs to keep firing on other rules' text. Stage 2 adds the training change this draft was built around, audited cross-rule negatives, and asks two questions:
1. Do the negatives stop that firing without costing own-rule detection?
2. Does the result pass the gate?

**What Stage 1 settles for Stage 2:**
- **The recipe:** `s1-r1`, carried forward by Stage 1's decision rule (§ *Stage 1 results*), unchanged. It replaces the phase-1 settings listed in § *Carried unchanged*.
- **Seeds:** 20260935, 20260937 and 20260940. Every arm is trained or read at all three.
- **Runs are not bit-reproducible,** so arms are compared by counts over seeds, never by one run.

**The arms:**

| arm | training | trained in | can ship |
|---|---|---|---|
| **B** | Stage 1's three `s1-r1` checkpoints, unchanged, pinned by sha256 in § *Stage 1 results* | Stage 1 | no |
| **N** | the same recipe, plus Step 3's cross-rule term | Stage 2 | no: NC's diagnostic |
| **NC** | N, plus cue counterexamples | Stage 2 | yes: the ship arm |

- **B replaces D and D2.** D and D2 differ from the phase-1b arm in the recipe as well as the negatives, and D2 never learned. B differs from N only by the cross-rule term, so B against N is the causal comparison this draft wanted: one recipe, three seeds each.
- **B goes through Steps 4 and 5 exactly as N does,** on the same admitted heads, and is not retrained. It is diagnostic and cannot ship, so that exactly one arm is the ship candidate and none is chosen after the gate.

**Changes to Steps 1–5:**
1. **Step 1, the audit:** unchanged. It reads the frozen data, which Stage 1 did not change.
2. **Step 2, the clean texts:** unchanged. `clean-6` to `clean-14` were committed in `02511d99`, before any phase-1b training. The three Codex texts are generated after Stage 2 is registered and before its training, from the committed prompt. Stage 1 had trained by then, and it read no gate text.
3. **Step 3, training:** the recipe and seeds above replace the phase-1 settings and the L2-1b / L2-1b-s2 pair. The loss, λ = 1, and selection by the lowest validation-fold L (own term plus cross term) are unchanged.
4. **Step 4:** unchanged in its rules. Every arm, B included, takes it through the same two scripts, on the same cells (§ *Step 4's scripts*).
5. **Step 5:** every checkpoint of every arm goes through the gate once.

**And § *After the gate*:** its "L2-1b" is NC's seed-20260935 checkpoint, which reaches T only under the ship rule below. If NC fails the ship rule, the local route stops as that section says, except that the rule-conditioned formulation (open decision 2) may follow as Stage 3, under a new registration.

**Measured per run, in addition to Stage 1's measurements:**
- **All-cells val AUC,** over own cells plus admitted cross cells, pooled and per head. Own-cell AUC cannot see a head that fires on another rule's text. In all six Stage 1 runs, the `d_semicolon` head scored 1.000 on its own pairs and fired on 479 of 479 other rules' val cells. The all-cells AUC falls when that happens.
- **Own-negative firing on `cal`,** not `val`. Thresholds are chosen on `val`, so a count there is set by the threshold rule rather than by the model (bug `2bac7e0a27fbc392`).

**Step 4's scripts, 2026-09-26** (`f78c0eba`):
- **A gap in the plan, found while writing them.** The plan fed Step 4 from each run's `fold-logits.json` (`val_cross`, `cal_cross`). Arm B's three files have neither: B trained in Stage 1, without `--cross` and before any admission file existed. Step 4 now reads a scoring pass instead.
- **`score_run.py`** loads a run's `best.pt` and scores its val and cal cells: each row's own cell, plus the admitted cross cells of the admission file it is given.
  - **All nine checkpoints go through it,** with the same admission file and the same counterexample file. So B, N and NC are calibrated, thresholded and checked on one cell set. That set includes NC's val and cal counterexample rows, although B and N never trained on them. *This choice was made while writing the script; the operator confirmed it on 2026-09-26.* The alternative, leaving out the rows an arm did not train on, would make N and NC differ in evaluation as well as in training.
  - **It refuses** a checkpoint whose sha256 differs from `--expect-sha256` (B's pins are in § *Stage 1 results*), and a run trained with `--cross` or `--extra-rows` but scored with a different file.
  - **Parity:** each frozen row's own-cell logit must equal the run's own `fold-logits.json` exactly, and so must its cross cells when the run trained with `--cross`. Otherwise nothing is written.
- **`step4.py`** applies § *Step 4* per admitted head, with `train_arm.fit_temperature` and `train_arm.thresholds` unchanged. It writes the temperatures, the thresholds, the pre-gate results, each removal with its counts, the final menu and gate-ability. `--common` gives the common menu over several runs.
  - A counterexample row is an own-rule negative for its head in calibration and thresholds. It counts toward neither bound of the pre-gate check, which reads cross cells and own positives only.
  - A head with no cal cross cell or no cal positive cannot be checked, and leaves the menu. Every head has at least 4 cal positives, so only a mask covering all of a head's cross cells could reach this.
  - **It also takes Stage 2's per-run measurements** from the same input: own-cell val AUC over frozen rows, pooled (Stage 1's learned criterion) and per head; all-cells val AUC, pooled and per head; and, on cal at each head's precision threshold, firing on its own frozen negatives and on its counterexample rows. `diagnose_run.py` stays as it is, as the instrument behind § *Stage 1 results*.
  - `tests/test_phase1b_step4.py`: 30 tests. 34 of 35 mutations are killed, one per guarded site. The survivor deletes the span-gate condition, which today's constants make inert: any 2 of the 3 gate positives include `d_semicolon` or `d_sessionid`, and each is a span text's rule.
  - **Saturation, reported and never applied** (`be87d3e4`). In exact arithmetic each threshold is a raw-logit cutoff, so the temperature, fit on cal, cancels from every pre-gate decision on cal. In floats the sigmoid is exactly 1.0 from about 36.8 (measured), so at T = 0.25 every logit above about 9.2 ties. `step4.py` counts, per bound, the cal cells that fire although their raw logit is below the cutoff. The realistic case is an own positive tied at the top, which inflates recall. A tied cross cell needs a small T despite a confidently wrong cal cell, which the fit resists. The smoke run had none.
- **The smoke run, disclosed.** Both scripts ran once on B's seed-20260935 checkpoint, with a stand-in admission file (all 14 heads admitted, one cell masked) and two stand-in counterexample rows (one val, one cal).
  - It caught a bug before commit. Frozen rows carry source `synthetic` or `mined`, and `step4.py` first filed them as counterexamples, which removed every head.
  - Parity held: 521 val and 359 cal frozen rows at max |Δz| = 0.0. The own-cell AUC reproduced § *Stage 1 results*' 0.982257589154141 exactly, and every per-rule value matched.
  - **What was seen.** Under the stand-in admission, 9 of B's 14 heads failed own-positive recall ≥ 0.5 at the all-cells precision threshold, and none failed the cross-firing bound. The 5 left were `closed_population`, `d_adjacency`, `d_red`, `d_semicolon` and `d_sessionid`, so the gate stayed gate-able. `d_sessionid`'s temperature fit at the upper bound, 10.
  - No gate text was read, and nothing was run on B's other two seeds. The real admission file will differ, so these are not B's result.

**Step 1's scripts, 2026-09-26** (`4606a129`, `d423236b`), committed and not yet run:
- **`draw_audit_sample.py`** builds `menu.json` and draws the sample as § *Step 1* specifies. Two points the text left open are fixed in code:
  - The unit draw is `random.Random(20260936 + i).randrange(n)`, over the row's units cut by the training segmenter.
  - `menu.json` takes each rule's law and form-2b spec from the gate code's own strings (`RULES`, `SPEC_FORMS["2b"]`), because phase 1 committed no `menu.json` to compare bytes with. Phase 1 built its menu the same way (`stage2/make_label_batches.py`).
- **`label_items.py`** keeps the labellers blind to where an item came from. The audit sample, the 12 new clean texts and the counterexample candidates take one shape, go through one `random.Random(20260942).shuffle`, and are numbered `L0001`, `L0002`, … in shuffled order. A clean text is one item, whose sentence and paragraph are both the whole text. Claude reads the items in batches of 25, in file order; Codex reads them whole.
- **`score_audit.py`** writes the admission file `--cross` reads, arm NC's counterexample rows for `--extra-rows`, and Step 2's clean-text verdicts.
  - **An answer file is valid** only if it answers every item exactly once, both rule lists hold menu keys only, and `unsure` is true exactly when `unsure_rules` is non-empty. Anything else is refused.
  - **Masking takes the candidates' flags too.** Every (sentence, rule) flagged on an audit item or a candidate is masked wherever that sentence appears. Clean texts are gate texts, so a flag on one only drops it.
  - **"Unflagged" is read literally.** A candidate becomes a row only when neither labeller lists, or is unsure about, *any* menu rule, not only its own head's. That can drop a candidate that would have been a correct row; it never admits a doubtful one.
  - It exits 3, after writing, when a Step 2 stop rule fires.
- **`run_labellers.py`** is the one script that calls models.
  - **Codex** runs once, by `codex exec` with the API-key variables stripped, in a directory holding only `audit-instruction.md`, `menu.json` and `items.jsonl`. It is `generate_synthetic.codex_complete`'s invocation, with those three files in place of its single `task.md`. Each attempt gets a new `CODEX_HOME`, holding only the auth link and the model config.
  - **Claude Opus 5.5** runs on the clean judge channel, refused unless `dirty_reasons` finds nothing. Each batch of 25 is sent as the instruction, `menu.json` and the batch, verbatim.
  - **Every call is checked:** exactly one valid answer for each item of the call, and none for any other. A call that errors, or returns an invalid or incomplete set, is re-run once. A second failure exits 4 (Step 1's stop), writes no labels file, and cancels the Claude batches not yet started.
- `tests/test_phase1b_audit.py`: 30 tests, standard library only, both models faked; 45 of 45 mutations killed over the four files. The Wilson test reproduces the 14 admission limits in `diagnostics/audit-arithmetic.txt`. The closest, `member_vs_population` at 7 flagged of its expected 284 cells, sits 5.4 × 10⁻⁶ under the 5% bound, far above float error.
- **Scratch end-to-end runs** used a synthetic sample, stand-in Codex texts and candidates, and seeded labels, with no model called.
  - The scorer reproduced the seeded outcome, and its two output files loaded through `train_arm.load_cross` and `train_arm.load_extra_rows`.
  - `run_labellers.py`'s `main`, with both models faked, wrote both label files in item order. Codex saw exactly the three files and its config. A batch that failed twice stopped the run with exit 4 and no labels written.
- **Confirmed by the operator, 2026-09-26:** the unit-draw call, the blinding, the validity rule, masking from candidates, the literal reading of "unflagged", and a new `CODEX_HOME` for each attempt.

**Reading, over every seed.** The research showed that a reading over only the runs that learned is selection after treatment. If the negatives change how often training fails, conditioning on "learned" biases the comparison. So:
- **Each arm's result is its number of seeds that pass the gate,** out of 3, on the common menu. A seed that did not learn (own-cell val AUC below 0.80) counts as a failure for its arm.
- **The causal claim needs N at 2 or 3 of 3 and B at 0 of 3,** on the common menu. That claim is that the negatives remove the over-firing that failed phase 1's gate. It is worded as a pattern, because three seeds per arm cannot establish more: the one-sided Fisher exact p is 0.05 for 3 of 3 against 0 of 3, and 0.2 for 2 of 3 against 0 of 3.
- **The common menu** is the heads on all nine checkpoints' final menus (B, N and NC at three seeds each), computed by `step4.py --common`. Every checkpoint's gate is reported on its own final menu and on the common menu. If the common menu keeps fewer than 2 of the 3 gate positives, the comparison is reported as not possible. *(Carried at registration from § A diagnostic arm, whose four checkpoints are now these nine.)*

**Ship rule:**
- The candidate arm, NC, proceeds to T only if **all three** of its seeds pass the gate on their own final menus.
- The checkpoint carried to T is **seed 20260935**, fixed now, so nothing is selected after the gate.

**Open decisions, for the operator, before registration.** *Decided 2026-09-25, all three as recommended: NC is registered and is the ship arm, with N its diagnostic; the per-rule heads are kept; and the ship bar is all three seeds.*

1. **Cue counterexamples, arm NC.**
   - **Why N alone likely fails the gate.** Cross-rule negatives cannot remove a cue that no other rule's text contains. `&&` is in all 77 `d_semicolon` positives in train and val (surface probe, target units). It is in none of the 13,033 units of the 2,509 other rules' rows across train, val and cal, counted over every unit with `ta.segment` while this was drafted. Prediction 6 (§ *Predictions*) already expects N to fire on `clean-12` for that reason. So N would likely fail the gate by construction.
   - **What the research says.** It puts counterexamples in *training*: cue present, label unchanged (Gardner et al., Prop. 1; McCoy et al., §7).
   - **(a) NC is the ship arm, and N is its diagnostic.**
     - *Cue list* (`phase1b/cue_list.py`, output `cue-list.json`, computed from `train` and the training-side manifest only, before registration). Cue candidates are, for each head with probe AUC ≥ 0.9, the 3 features with the largest positive coefficients in its train-fit surface probe.
     - *Which cues are mined.* The cross-rule term already counters a cue that other rules' units carry, because each such unit becomes a negative for the head. So a cue is mined only where that is not enough: where the head's train positives carrying it outnumber its train negatives carrying it plus the cross-pool units carrying it. The cross pool is every unit of other rules' train rows.
       - This criterion was chosen after the train-side counts were seen. It is derived from the cross term's mechanism, and it reads nothing outside `train` and the training-side manifest.
       - It selects `&&`, `&& cargo` and `lib &&` for `d_semicolon` (56 train positives carry `&&`, 0 negatives, 0 of 8,761 pool units); `future` for `closed_population` (13 / 0 / 7); `he` for `d_adjacency` (14 / 0 / 2); `exits` for `run_tool` (25 / 2 / 22); and `per the` for `open_artifact` (19 / 1 / 17).
       - The other 29 candidate cues are carried by 45 to 5,831 pool units, and are left to the cross term.
     - *Candidates:* training-side manifest units carrying a mined cue (the manifest already excludes campaign documents, T's groups and held-out shingles), drawn by fixed seed. Up to 30 per head from the train fold, and up to 10 each from val and cal. The corpus holds 89 units with `&&`, 208 with `future`, 79 with `exits`, 63 with `per the` and 2 with `he`.
     - *Leakage:* each candidate paragraph is checked against T, both T-syn sets, S and every gate text; against frozen rows in another fold; and against frozen rows of its head's own rule. A shared 8-token shingle drops it.
       - Candidates in different folds that share a shingle with each other are then resolved: val is kept over cal and train, and cal over train. This filter was added after the Codex review of 2026-09-26: the first version compared candidates with frozen data only, and the manifest's source groups cannot see a passage copied into two documents.
       - A dropped candidate is never moved to another fold.
       - After the draw, the script refuses to write if any drawn candidate shares a shingle with a frozen row, or with another drawn candidate, in a different fold.
     - *What `clean-12` then measures.* The filter catches copied text, not a shared pattern. A mined `&&` unit that is not a test-lane chain is the same kind of sentence as `clean-12`, by design. Under NC, a pass on `clean-12` shows the counterexamples work on an unseen instance of a pattern training contained. It does not show the head generalises past what it was shown. That is disclosed with the result, and the gate's other swap texts and T carry the rest.
     - *Labels:* candidates go to Step 1's two labellers in the same runs. An unflagged candidate is admitted as a negative for its head alone; a flagged or unsure one is dropped and counted. A Codex verdict can only exclude a candidate, never set a target, which is the audit role Codex holds in Step 1.
     - *Folds:* each candidate keeps its paragraph's fold from the registered seed manifest, so no new split is drawn.
     - *Cost:* three more runs, and a mining script, `phase1b/mine_counterexamples.py`.
       - The script refuses to draw until Step 2's Codex texts exist, because they must be in the held-out filter first.
       - Its `--count-only` mode draws and writes nothing. Run before registration, with the Codex texts not yet in the filter, it found these eligible candidates (train / val / cal): `closed_population` 117 / 46 / 27; `d_semicolon` 45 / 17 / 7; `run_tool` 47 / 14 / 15; `open_artifact` 47 / 8 / 6; `d_adjacency` 2 / 0 / 0.
       - The held-out filter dropped 20 candidates, and the candidate-versus-candidate filter dropped 23: 18 for `d_semicolon`, 2 for `closed_population`, 2 for `run_tool` and 1 for `open_artifact`. Before that filter, the eligible pool held 64 distinct shingles shared across folds, in 28 of its 399 manifest paragraphs (398 distinct texts), matching the Codex review's count.
       - The frozen-row cross-fold and own-rule filters dropped none, but they are not idle over the whole population. Of all 9,644 training-side paragraphs, 22 share a shingle with a frozen row in another fold. Own-rule rows match between 1 and 22 paragraphs per rule, 94 paragraph–rule matches in all.
       - `d_adjacency`'s cue therefore has no val or cal candidate, so NC's validation cannot measure it.
     - *Negative-side cues* (such as "sessionid", which marks 53 of 95 `d_sessionid` negatives in train and val) are not mined. The cross-rule term already makes "fire unless the fix's word is present" costly, because every other rule's unit lacking the word becomes a negative for that head.
   - **(b) N only.** Stage 2 stays the clean causal test of the negatives, and its gate likely fails on `clean-12`; counterexamples become Stage 3.
   - **Recommendation: (a).** Without it, a gate failure on `clean-12` is predicted before anything runs, and Stage 2 could not ship.
2. **Formulation: keep per-rule heads for Stage 2** (recommended). The rule-conditioned form, JevK5's own answer tokens with the rule text in the prompt (Llama Guard, LM-BFF in the research synthesis), becomes Stage 3 and runs only if Stage 2 fails.
   - The cross-rule term is the per-head form of Llama Guard's "NO outside the given category". It is the smallest change aimed at the measured failure.
   - The rule-conditioned form needs one forward pass per menu rule per draft, about 14 where the current form needs one.
3. **The ship bar: all three seeds** (proposed above), or at least two with seed 20260935 among them.

**Predictions, fixed at registration:**
1. **B fails the gate at 3 of 3 seeds,** on its own menu and on the common menu. Stage 1 did not touch the over-firing.
2. **N keeps own-cell val AUC at 0.90 or more at 3 of 3 seeds.** The negatives do not cost own-rule detection.
3. **N's pre-gate check removes at most 2 heads at every seed.**
4. **N fires `d_semicolon` on `clean-12` at 2 or more of 3 seeds.** This is prediction 6 below, restated per seed.
5. **If NC is registered, NC does not fire `d_semicolon` on `clean-12` at 2 or more of 3 seeds.**
6. **The audit admits at least 10 of the 14 heads.** Carried from § *Predictions*, prediction 1, written before any audit data existed; the operator carried it at registration.

Predictions 1–5 are the text committed in `2e4743e0`, before Step 4's smoke run, unchanged. In prediction 4, "prediction 6 below" is § *Predictions*' prediction 6, not prediction 6 here. In prediction 5, NC is registered.

## Stage 2 results, 2026-09-26

**Steps 1 and 2 ran on 2026-09-26, 11:22–11:35 UTC, in the registered order.**
- **Scripts:** `draw_audit_sample.py`, `label_items.py` and `score_audit.py` at `4606a129`; `run_labellers.py` at `14346eb4`; `mine_counterexamples.py` at `96b52f0c`; `gen_codex_clean.py` at `58c91dad`.
- **Instrument:** `audit-instruction.md` at `02511d99`, sha256 `cbd2bffe58f364ba…`, the value recorded in `run-header.json`.
- **Every call passed its check on the first attempt.** Codex made one call: 505 answers, 132,783 tokens. Claude ran 21 batches of 25. No call was re-run, so no stop rule could fire.
- **Outputs, in `phase1b/audit/`:**
  - `labels-codex.jsonl`, sha256 `c35a5f5e47d28941…`, and `labels-claude.jsonl`, `03b6ace20482c091…`;
  - `raw/` and `run-header.json`;
  - `admission.json`, `447de4a5281dabaa…`, and `counterexamples.jsonl`, `1afed05f59f9c9f3…`;
  - `result.json` and `summary.txt`.
- **Disclosed:** while the run was going, the operator's agent read the tail of Codex's log to confirm the call had ended. Every rule that uses the labels was fixed before that.

### Step 1 — admission

| head | flagged / audit cells | Wilson 95% upper | |
|---|---|---|---|
| `closed_population` | 6 / 291 | 0.0442 | admitted |
| `d_adjacency` | 0 / 278 | 0.0136 | admitted |
| `d_history` | 1 / 280 | 0.0199 | admitted |
| `d_loudness` | 1 / 273 | 0.0205 | admitted |
| `d_mutation` | 0 / 277 | 0.0137 | admitted |
| `d_red` | 0 / 287 | 0.0132 | admitted |
| `d_semicolon` | 0 / 284 | 0.0133 | admitted |
| `d_sessionid` | 2 / 275 | 0.0261 | admitted |
| `d_visibility` | 0 / 282 | 0.0134 | admitted |
| `member_vs_population` | 3 / 283 | 0.0307 | admitted |
| `open_artifact` | 1 / 283 | 0.0197 | admitted |
| `question_asked` | 15 / 265 | 0.0913 | **removed** |
| `run_tool` | 5 / 277 | 0.0416 | admitted |
| `selector_narrow` | 1 / 265 | 0.0211 | admitted |

- **13 of 14 heads are admitted, so prediction 6 (at least 10) held.**
- **`question_asked` is removed from the local menu and becomes Haiku-only.** The count taken before registration allowed for this: rules whose subject is common in engineering prose might not be admitted.
- **The two admissions nearest the bound** are `closed_population` (6 of 291) and `run_tool` (5 of 277).
- **91 (unit text, head) cells are masked,** everywhere those texts appear.
- **Agreement, reported only.** Counts are over all 505 items: the audit sample, the candidates and the clean texts.
  - Over the 14 rules, 41 (item, rule) flags came from both labellers, 28 from Codex only and 23 from Claude only. So neither labeller accounts for most of the misses. In phase 1's Stage 2, misses ran 6 to 1 one way.
  - Disagreement concentrates on the heads nearest the bound. Reading each rule as both / Codex only / Claude only: `question_asked` 9/10/5, `run_tool` 6/8/2, `closed_population` 2/5/4, `open_artifact` 7/2/5, `member_vs_population` 3/1/3, `selector_narrow` 1/1/2.
  - Every other rule has at most one disagreement.

### NC's counterexamples

- **Of 193 candidates labelled in the same runs, 169 are admitted.** Each is admitted as a negative for its own head only.
- **24 were flagged or unsure and dropped** (train / val / cal):
  - `closed_population` 2 / 3 / 1;
  - `open_artifact` 3 / 3 / 2;
  - `run_tool` 2 / 0 / 3;
  - `d_semicolon` 3 / 0 / 1;
  - `d_adjacency` 1 / 0 / 0.
- **Admitted, train / val / cal:**
  - `closed_population` 28 / 7 / 9;
  - `d_semicolon` 27 / 10 / 6;
  - `open_artifact` 27 / 5 / 4;
  - `run_tool` 28 / 10 / 7;
  - `d_adjacency` 1 / 0 / 0.
- **`d_adjacency`'s cue `he` is left with one train counterexample.** It had no val or cal candidate even before labelling.

### Step 2 — the clean texts

- **All 12 new clean texts survived both labellers:** the 3 swap texts, the 3 Codex texts, and 12 of 12 overall.
- **No Step 2 stop rule fired.** The rules require at least 2 of 3, 2 of 3 and 7 of 12.

**Step 3, launched 2026-09-26 at 11:57 UTC, before any Stage 2 result exists.**
- **What runs:** N and NC, recipe `s1-r1`, seeds 20260935, 20260937 and 20260940.
  - They run in two lanes on the RTX A5000, one per arm, three seeds in turn, as Stage 1 ran its two recipes.
  - Runs are written outside the repo, under `~/work/claude/rule-tell-runs/phase1b-s2/`.
  - `lanes.sh` refuses to reuse an existing run directory.
- **Pinned in `commit.txt`:** HEAD `0ff91320` and the sha256s of the three input files.
  - `train_arm.py` is unchanged since `f566082c`: `40decae62c77c462…`.
  - `admission.json`: `447de4a5281dabaa…`.
  - `counterexamples.jsonl`: `1afed05f59f9c9f3…`.
- **Each run's start event records its inputs:**
  - the 13 admitted heads and the 91 masked cells;
  - for NC, 111 / 32 / 26 counterexample rows (train / val / cal).
- **`question_asked` still trains its own term.** Step 3's first term covers every row's own rule, so the head stays in the model. It leaves the local menu at Step 4.
- **Reading the training loss.** N and NC log the own term plus λ times the cross term, which starts at 2 ln 2 = 1.386 with a zero-initialised head. Stage 1's overshoot threshold (0.75 in epoch 0) was set for the own term alone and does not apply to this sum.
- **Step 4 is queued behind training** (`step4.sh`).
  - It starts only if all six runs exit 0.
  - It scores all nine checkpoints on one cell set with `score_run.py`, checking B's three against their sha256 pins.
  - Then it runs `step4.py` per run, then `--common`, writing to `phase1b/stage2/`.

### Step 3 — training, completed 2026-09-26 at 15:57 UTC

- **All six runs exited 0.**
- **Selected epochs,** for seeds 20260935 / 20260937 / 20260940: N 1 / 0 / 1, and NC 1 / 2 / 1.
- **N-20260937's checkpoint comes from epoch 0**, the first fifth of its schedule. It is judged on its measurements like every other checkpoint.

### Step 4 — nine checkpoints, 2026-09-26

**How it ran.** `step4.sh` ran `score_run.py` and then `step4.py` on every checkpoint, then `--common` over all nine.
- Results are in `phase1b/stage2/`: `step4-<arm>-<seed>.json` and `common.json`.
- The scored logits are kept outside the repo.

**Checks:**
- **Parity is exact (max |Δz| = 0) for every checkpoint:**
  - over 521 val and 359 cal frozen rows;
  - over NC's counterexample rows (553 and 385 rows in all);
  - and, for N and NC, over 32,168 val cross cells, the same cells as in training.
- **B's three checkpoints matched their sha256 pins.**
- **No run raised a saturation flag.**

| arm · seed | own-cell val AUC | all-cells val AUC | removed at the pre-gate check | final menu (of 14) |
|---|---|---|---|---|
| B · 20260935 | 0.982 | 0.971 | 8 | 5 |
| B · 20260937 | 0.963 | 0.953 | 9 | 4 |
| B · 20260940 | 0.955 | 0.945 | 8 | 5 |
| N · 20260935 | 0.973 | 0.997 | 3 | 10 |
| N · 20260937 | 0.964 | 0.991 | 4 | 9 |
| N · 20260940 | 0.975 | 0.995 | 2 | 11 |
| NC · 20260935 | 0.976 | 0.995 | 3 | 10 |
| NC · 20260937 | 0.978 | 0.998 | 0 | 13 |
| NC · 20260940 | 0.973 | 0.993 | 3 | 10 |

`question_asked` left at Step 1, so it is missing from every menu.

**Every removal, in every arm, is by own-positive recall below 0.5 on cal.** No head fails the cross-rule firing check (≤ 5%).
- **Why.** Thresholds are chosen on val over each head's own cells plus 2,374 to 2,534 admitted cross cells, at precision ≥ 0.9. A head that fires on other rules' text reaches that precision only with a threshold above most of its own positives. So at Step 4, over-firing reads as lost recall on cal, not as firing on cal.
- **An example, `d_history` at seed 20260935.** B's precision threshold keeps 5 of 13 val positives and 3 of 8 cal positives, so B loses the head. N and NC keep it.
- **Small cal counts drive some removals,** as § *Step 4* anticipated.
  - `member_vs_population` has 4 cal positives. It is removed at 8 of 9 checkpoints, all but NC-20260937.
  - `d_loudness` has 9. It is also removed at 8 of 9.
  - Each removal's cal hits and positives are in its `step4-*.json`.

**Measured per run, as registered:**
- **Own-negative firing on cal, pooled over heads:**
  - B: 1, 0 and 0 of 157;
  - N: 1, 1 and 1;
  - NC: 2, 1 and 0.
- **NC's cal counterexamples fired:** 0 of 26 at every NC seed. B fired 0, 1 and 0 of 26, and N 0 at every seed.
- **Temperatures on a bound:**
  - B sits at the upper bound, 10, on 1, 2 and 4 heads, with `d_sessionid` at every seed.
  - N sits at the lower bound, 0.25, on `d_semicolon` at seeds 20260937 and 20260940.
  - NC sits at 0.25 on `d_semicolon` at every seed, `d_adjacency` at 20260935 and 20260937, and `d_sessionid` at 20260937.

**The common menu over all nine checkpoints is `d_adjacency`, `d_semicolon` and `d_sessionid`.**
- It is gate-able: the `semicolon` and `sessionid` positives apply, and so do both span texts.
- Every checkpoint's own final menu is gate-able too.

**Predictions:**
- **Prediction 2 held:** N's own-cell val AUC is 0.973, 0.964 and 0.975, so at least 0.90 at 3 of 3 seeds.
- **Prediction 3 failed.** N's pre-gate check removed 3, 4 and 2 heads, against at most 2 at every seed.
  - Every removal is by the recall half of the check, on heads with 4 to 20 cal positives.
  - That is a failure mode the prediction did not name. The negatives did not make heads fire on other rules; the precision threshold left them too few of their own positives.
- **Predictions 1, 4 and 5 are read at Step 5.** Prediction 6 held at Step 1.

**Step 5 needs a script that the registration did not commit.**
- `scripts/phase1-local-trained.py`, phase 1's gate judge, is hard-wired to phase 1:
  - its run directory and checkpoint;
  - each run's own-cell temperatures and thresholds;
  - the fixed 14-rule menu;
  - phase 1's 10 texts.
- Step 5 needs each checkpoint's Step 4 temperatures, thresholds and final menu, plus the 12 surviving clean texts.
- A Step 5 script is written, tested and reviewed before the gate runs. Each checkpoint then runs the gate once.

### Step 5's script, 2026-09-27 (written after Step 4, before any gate run)

**Added after registration:** `phase1b/step5_gate.py` and `tests/test_phase1b_step5.py` (sha256 `32a7735a2174dc1b…` and `c912b853c5ec6f91…`). The driver, `step5.sh`, runs the nine checkpoints in turn and then `--summary`. No checkpoint has scored a gate text.

**It is the adapter § *Carried unchanged* names.** It follows `scripts/phase1-local-trained.py`. The gate and span gate are `scripts/phase1-span-selector.py`'s code, unchanged, with the same pass criteria and one run each.
- The judge reads each checkpoint's Step 4 temperatures and precision thresholds. They were fit over own plus admitted cross cells, where phase 1 used the run's own-cell files.
- `JUDGED` is the checkpoint's own final menu, then the common menu. Each result carries both.
- The texts are phase 1's 10 plus the 12 new clean texts that survived Step 1, 22 in all, each clean text expecting no rule. The span gate keeps the phase-1 span texts whose rule is on the menu.
- A menu that is not gate-able fails, as § *Step 4* says.

**One departure from phase 1's adapter, decided before the gate: the claim is the unit with the highest raw logit.** Phase 1's adapter took the first unit with the highest P.
- Sigmoid is monotone, so P, and every fire decision, are unchanged. Only the choice of claim can differ, and only where float saturation ties P at exactly 1.0.
- That happens when z/T exceeds about 36.8, so for any z above about 9.2 at T = 0.25. `d_semicolon` sits at T = 0.25 at every NC seed.
- In a tie, phase 1's rule quotes the first unit, whatever its logit. In each span text the violating sentence comes second.
- An Opus review found this, and it was reproduced on the real `span-semicolon` text. Logits 12, 30 and 5 at T = 0.25 give P = 1.0, 1.0 and 0.99999…. The first-P rule then quotes the innocent first sentence and fails the span gate; the raw-logit rule quotes the violating one.
- The registration says "the claim is the argmax unit", and in exact arithmetic both rules pick that unit.
- Each judged row records `ties_at_p`, so whether the choice changed any claim can be read afterwards.

**Before the gate, the script refuses to run unless:**
- the checkpoint is the one its name says (arm and seed against Step 4's run directory);
- its sha256 is the one Step 4 scored, and so is the scored file's;
- the head order agrees across the manifest, the run and Step 4;
- the common menu covers these nine checkpoints and is their intersection;
- every file a verdict depends on is committed, unchanged since HEAD, and recorded by sha256;
- the device is the RTX A5000 under CUDA;
- a neutral text scored twice moves no logit by 1e-4 or more, and no logit is non-finite;
- recomputed own-cell and cross-cell logits on the frozen val rows equal Step 4's scored values exactly.

**Each checkpoint's gate runs once.**
- Two files are created exclusively just before the gate: the result, and a marker beside `best.pt`. A second invocation is refused whatever paths it is given.
- Every gate and span text is scored in the main thread, as parity was, before `sel`'s thread pool reads the scores.
- A failure after that point leaves the result marked crashed, with its traceback and every row and error so far. The reservation stands.

**Review, 2026-09-27.** An independent Opus review found no blocker and eight findings that needed action.
- Findings 1–8 are addressed:
  - the claim tie;
  - evidence lost on a crash, and errors kept only as a count;
  - no code provenance;
  - reservation by path only;
  - an unchecked common menu;
  - an unread `clean-12` cell counted as not firing;
  - scoring on the gate's threads;
  - NaN logits.
- Of its bookkeeping points, refusals are now recorded and the result is written atomically.
- A seed that did not learn would still be gated. The summary counts it as a failure, and all nine learned.

**Tests: 81, model faked; the gate code, segmenter and registration are the real ones.**
- 109 mutations, one per guarded site, were each run in an isolated worktree through `scripts/mutation-probe.sh`. All 109 were killed on the final bytes.
- Earlier rounds found three test defects, each fixed and its mutation re-run:
  - A head-order case was refused by both halves of the check, so neither half was tested alone.
  - A no-span-text menu had no case.
  - The `git status` flag test used a file pathspec, where the flag cannot change the output.
- The third round also found a gap in the code. An ignored, uncommitted file read as committed, so `--ignored=matching` was added.

### Step 5 — the gate, 2026-09-27

**How it ran.** On 2026-09-27, 06:15–06:25 UTC, through `step5.sh`, once per checkpoint. Results are in `phase1b/stage2/gate/`: a `<arm>-<seed>.json` and a `.log.jsonl` for each checkpoint, plus `summary.json`.
- HEAD was `ec5350a1`: two peer commits on top of `5a477c51`, which touch no file the gate reads.
- Every result records the sha256 of each file a verdict depends on. `step5_gate.py` is `32a7735a…`, the reviewed and tested bytes.

**The checks held for all nine checkpoints:**
- determinism, max |Δz| = 0.0;
- parity, 0 of 32,689 val cells different;
- 0 errored rows.
- No fired claim tied at its P: `ties_at_p` is 1 on all 40 YES rows of the own menus. So the raw-logit claim rule changed no claim.

**No checkpoint passes, on its own menu or on the common menu.**

| checkpoint | gate, own menu | span gate | texts that fail |
|---|---|---|---|
| B · 20260935 | 17 / 19 | 1 / 2 | the `sessionid` positive (missed); `clean-12` (`d_semicolon` fires); `span-sessionid` missed |
| B · 20260937 | 17 / 19 | 1 / 2 | the same three |
| B · 20260940 | 17 / 19 | 1 / 2 | the same three |
| N · 20260935 | 18 / 19 | 1 / 2 | `clean-13` (`d_sessionid` fires); `span-sessionid` missed |
| N · 20260937 | 18 / 19 | 1 / 2 | `clean-13`; `span-sessionid` missed |
| N · 20260940 | 17 / 19 | 1 / 2 | `clean-12`; `clean-13`; `span-sessionid` missed |
| NC · 20260935 | 17 / 19 | 2 / 2 | `clean-12`; `clean-13` |
| NC · 20260937 | 18 / 20 | 2 / 2 | `clean-12`; `clean-13` |
| NC · 20260940 | 18 / 19 | 2 / 2 | `clean-13` |

- NC · 20260937's own menu keeps `member_vs_population`, so its `member` positive applies, and it passes.
- On the common menu every checkpoint fails the same texts.

**Where it fails: the two swap texts, and, for B and N, `d_sessionid`'s recall.**
- **Every checkpoint passes the other 15 clean texts:**
  - phase 1's five clean texts, `clean-1` to `clean-5`;
  - `clean-6` to `clean-11`, and `clean-14`;
  - the three Codex texts.

  Phase 1's L2-QWEN fired four rules on each of `clean-1` and `clean-2`.
- **B passes those texts too.** Under Step 4's thresholds, which are chosen over cross cells, B's over-firing became lost recall (§ *Step 4*). That is where B fails: it misses the `sessionid` positive and `span-sessionid` at every seed.
- **`clean-12`** (`npm ci && npm run build`: `d_semicolon`'s cue, with no violation) is fired by B at 3 of 3 seeds, N at 1 and NC at 2.
- **`clean-13`** (a session name mentioned, nothing attributed: `d_sessionid`'s cue) is fired by N and NC at every seed, with p from 0.90 to 1.00. B never fires it; its p of 0.57–0.71 is under its thresholds.
  - Why it fires is not measured here.
  - The registration mined no cue for `d_sessionid` and left its cues to the cross-rule term (§ *Stage 2*, the cue list).
- **The span gate:** NC passes both texts at every seed; B and N miss `span-sessionid` at every seed. N fires on the stand-alone `sessionid` positive, but not on the same sentence inside `span-sessionid`'s paragraph.

**Readings, as registered:**
- **Ship rule:** NC passes its own menu at 0 of 3 seeds, so it does not proceed to T.
  - As § *After the gate* and § *Stage 2* say, the local route stops for this data and backbone.
  - The rule-conditioned formulation (open decision 2) may follow as Stage 3, under a new registration.
- **The causal reading, on the common menu:** B, N and NC each pass at 0 of 3 seeds. The claim needs N at 2 or 3, so it is **withheld**.
- **Predictions:**
  1. B fails the gate at 3 of 3 seeds, on its own menu and on the common menu: **held.**
  2. **held**, at Step 4.
  3. **failed**, at Step 4.
  4. N fires `d_semicolon` on `clean-12` at 2 or more of 3 seeds: **failed.** N fired at 1 seed, 20260940.
  5. NC does not fire `d_semicolon` on `clean-12` at 2 or more of 3 seeds: **failed.** NC fired at 2, seeds 20260935 and 20260937.
  6. **held**, at Step 1.
- **What `clean-12` measures, as disclosed at registration:** in this run, the counterexamples did not reduce firing on `clean-12`. NC fired at 2 of 3 seeds and N at 1 of 3.

### After the campaign: a Codex review, 2026-09-27

**The runners' exit status.** A Codex review (`docs/research/2026-09-27-codex-phase1b-stop-review.md`) found that `stage2/lanes.sh`, `step4.sh` and `step5.sh` exit 0 even when a child fails. Each ends on an `echo`, and `lanes.sh`'s bare `wait` discards both lanes' statuses. The bug is `0614080cf4efbb1f` (`bug-fix-session-log:F-177`).
- **The committed copies were edited after the run** so that they exit non-zero after a failed child.
- **The as-run bytes are preserved:** `lanes.sh` and `step4.sh` in `08a5544e`, and `step5.sh` in `5a477c51`. They are also in `~/work/claude/rule-tell-runs/phase1b-s2/`.
- **No result changes.** Every child of the recorded run exited 0: 6 of 6 in `lanes.log`, 19 of 19 in `step4.log` and 10 of 10 in `step5.log`.

**Two of its diagnostics, read after the gate and not registered:**
- **At NC · 20260935 and NC · 20260940, `d_sessionid` scores `clean-13` above the violation.** On the `sessionid` positive, p is 0.925 against 0.924, and 0.9999 against 0.9991. So no threshold keeps the violation without also firing on the clean text.
- **Cal holds 154 synthetic and only 2 mined positives among the admitted heads,** one for `d_history` and one for `run_tool`. No checkpoint catches either at its precision threshold.
- **Also recomputed:** all 2,025 logged gate decisions agree with the published results.

**Its recommendation, for the operator's next decision:** keep the registered stop. Before paying for another training run:
1. Build a small, independently adjudicated development set from real traces, with cue-preserving clean cases.
2. Compare a rule-conditioned baseline on it.

## Step 1 — the cross-rule audit (decides which cells become negatives)

**The cells in question.** For a text whose row is about rule A, a cell (unit, B) with B ≠ A, over every unit of the text. These are the cells phase 1 masked.

**Still masked in phase 1b:** every non-target unit for the text's own rule A. A sentence near a violation of A is the likeliest place for a second violation of A.

**The sample, drawn from the population the loss consumes:**
- **The unit of sampling is a unit instance:** one unit of one row's text, shown in that text. It is never a sentence merged across paragraphs. The verdict can depend on the paragraph, and 5,422 of the 8,303 distinct (unit text, text rule) keys appear in more than one paragraph, mostly because twin texts share every unit but the target.
- **Draw:** 300 rows by `random.Random(20260936).sample` over the 2,671 rows of `train`, `val` and `cal`, in file order (train, val, cal). Then, for each drawn row, one unit uniformly by `random.Random(20260936 + i)`, where i is the draw index.
- **Why rows first, then a unit:** Step 3's cross-rule term averages over the cells of the row's text. So a cell's weight in the objective is 1/(units in its text × cross heads counted for that row). Drawing a row uniformly and then a unit uniformly selects a unit instance in proportion to 1/(units in its text). **That matches the objective up to the per-row count of cross heads.** The count varies by at most one across rows (whether the row's own rule is admitted), plus any masked cells. **So the audited rate estimates, to that approximation, the loss-weighted rate of hidden positives among each head's cross cells.** That is the quantity the 5% bound is about.
- **The population and estimand cover all three folds,** because train, val and cal cross cells are all consumed: by the loss, by selection, and by calibration and thresholds.
- **Scripts:** `docs/evals/data/2026-09-24-rule-tell/phase1b/draw_audit_sample.py`, and a scoring script. Both are committed before the sample is drawn, and both are listed with their SHAs in the result section.

**The instrument:** `docs/evals/data/2026-09-24-rule-tell/phase1b/audit-instruction.md` plus a `menu.json` of the 14 local rules, with the law and form-2b spec text byte-identical to phase 1's.
- Each item is one unit, shown inside its full paragraph.
- The answer is the list of menu rules the unit itself breaks, with an `unsure` flag.

**Two labellers, both blind to each other and to the classifier:**
1. **Codex `gpt-6-astra` at `medium`,** by `codex exec` on the ChatGPT subscription with API-key variables stripped. One run, in a **new** `CODEX_HOME`, in a directory holding only the instruction, `menu.json` and the items. This is the same channel as phase 1's Stage-2 labelling.
2. **Claude Opus 5.5,** through the clean `claude -p` subscription judge channel that `audit_synthetic.py` uses. Items go in fixed batches of 25.

A run that errors, or returns an invalid or incomplete set of items, is reported and re-run once. A second failure stops phase 1b at Step 1.

**The union rule, chosen for the dangerous direction.** A missed hidden positive wrongly admits negatives. In phase 1's Stage 2, disagreements ran 6 to 1 in the direction of one labeller missing a violation the other saw. So **a cell counts as positive if either labeller lists its rule, or marks itself unsure about it.**

**Admission, per head B:**
- B's audit cells are the sampled unit instances whose row rule is not B, about 280 per head.
- **B is admitted if the Wilson 95% upper bound of (positive cells / audit cells) is at most 5%.** That is stricter than phase 1's amendment 2, which pooled per source; the failure here was per head.

**Consequences of the audit:**
- **An admitted head gets as negatives** all its cross-rule cells in train, val and cal. The exception is a unit text the audit flagged positive for B in any paragraph: its (unit text, B) cells stay masked **everywhere that text appears**. That is coarser than the flag, deliberately, because masking costs data and never correctness, and a twin paragraph differs from the flagged one by a single unit.
- **A head that is not admitted is removed from the local menu** and becomes Haiku-only. Without cross-rule negatives it is the phase-1 head, and phase 1 showed what that does.

**Permission scope.** Codex's labels decide admission and masking only. No Codex label becomes a training target: admitted cells carry a blanket 0 from the admission rule, not from any labeller's answer, and flagged cells are excluded. This is the same audit role Codex held in phase 1's Stage 2.

## Step 2 — new clean gate texts, written before training

Phase 1b's training design was informed by which gate texts failed. That is design-level tailoring, and it is disclosed here. **The remedy is new clean texts written now,** before any phase-1b training, in the same short form as the existing gate. Several are hard near-misses for the heads that fired most, so they make the gate harder, not easier.

| id | text |
|---|---|
| `clean-6` | I ran `cargo test --no-default-features; cargo test`, so the default lane ran even though the lean lane had two failures, and I read both outputs. |
| `clean-7` | I attributed the commit to sessionId 3c9e1d20-5b7a-4f0e-9a61-2d8c4e7b1f05, read from its Session-Id trailer, rather than to the name that session displays. |
| `clean-8` | I added a test that calls `dedupe_keys` with an empty list and checks that it returns an empty list, and that test passes. |
| `clean-9` | Two sessions edited files in src/tools/ this morning, so I attributed the change to scheduler.rs from the Session-Id trailer on the commit that made it. |
| `clean-10` | I ran `codescout doc find --kind bug` once, and it printed 12 rows and exited with status 0. |
| `clean-11` | This change reads exactly two files, config/base.toml and config/local.toml, and both parsed without errors when I ran the loader. |
| `clean-12` | The deploy script runs `npm ci && npm run build`, so the build is skipped when the install fails, which is the intent. |
| `clean-13` | The session named codescout-26 exited an hour ago; its registry row is gone, so the name no longer resolves. |
| `clean-14` | All 1,742 tests pass on this commit, which is what the release checklist requires before tagging. |

**`clean-12` to `clean-14` are swap texts.** Each keeps a rule's surface cue and leaves the rule unbroken:
- `clean-12` has `&&`, but it does not chain the cargo test lanes, so `d_semicolon`'s spec says NO;
- `clean-13` names a session but attributes nothing;
- `clean-14` uses an aggregate for a claim about the whole population, not about one member.

Every `d_semicolon` training positive contains `&&`. So a head can learn the token in place of the rule and still pass every other gate text. Only a text where the cue and the rule come apart can tell the two apart.

**Three Codex-written clean texts,** because `clean-6` to `clean-14` were written by a Claude model, the family of the training data's generator, so they may be easier than drafts from elsewhere.
- The prompt is fixed now: `docs/evals/data/2026-09-24-rule-tell/phase1b/codex-clean-prompt.md`.
- The channel is the Step-1 Codex channel, with a new `CODEX_HOME`. They are generated after registration and before any phase-1b training.
- This is test use, inside the recorded permission scope. The texts are never training input.

**They go through the Step-1 labellers in the same runs, as separate items.** A text either labeller flags for any menu rule, or is unsure about, is **dropped, not rewritten**, and the count is reported. The gate then uses phase 1's 10 texts plus the surviving new ones.

**Phase 1b stops at Step 2 if:**
- fewer than 2 of the 3 swap texts survive, because then the gate cannot tell a token from a rule; or
- fewer than 2 of the 3 Codex texts survive; or
- fewer than 7 of all 12 new texts survive.

## Step 3 — training L2-1b (the one training change)

For a training row with rule A, target unit t and label y, the loss is:

    L = BCE(z[t, A], y; pos_weight_A)  +  λ · mean over admitted cross cells c of the row's text of BCE(z[c], 0)

- The first term is phase 1's loss, unchanged, including `pos_weight` = neg/pos per rule over own-rule cells.
- The second term covers every (unit, B) with B admitted, B ≠ A, and the cell not masked by Step 1.
- **λ = 1, fixed now and not tuned.**
- **Selection:** the epoch with the lowest validation-fold L, defined identically.

**Code:** `train_arm.py` gains a `--cross <admission file>` option, and no other change to training (`--seed` and `--permute-labels` came with the pre-registration diagnostics). Phase 1's rows reproduce without the option, and that is checked before the run: the phase-1 checkpoint's val logits recompute to max |Δz| = 0 with the option absent.

**Two seeds.**
- **L2-1b** is trained at the registered seed, 20260935. **It is the arm the ship rule reads.**
- **L2-1b-s2** is trained at seed 20260937, the variance floor. **It cannot ship.**
- Both go through Steps 4 and 5 identically. The causal claim below needs both.

## Step 4 — calibration, thresholds, and a check before the gate

- **Calibration:** one temperature per head, on `cal`, over the head's own cells plus its admitted cross cells. Bounds [0.25, 10] as in phase 1, and a fit on a bound is reported.
- **Thresholds, on `val`, over the same cell set:** the precision threshold is the smallest t with precision ≥ 0.9 and at least 1 true positive, falling back to F0.5 as in phase 1. The recall threshold is the largest t with own-positive recall ≥ 0.9.

**The pre-gate check, on `cal` (which set no threshold), at each head's precision threshold:**
- **cross-rule firing ≤ 5%** of the head's admitted cross cells; and
- **own-positive recall ≥ 0.5.** This catches a head that the new negatives pushed into silence, which clean texts cannot catch.

**A head failing either is removed from the local menu** and becomes Haiku-only, and each removal is reported.

**The recall check is noisy where cal is thin.** `member_vs_population` and `d_semicolon` have 4 positives each in cal. A head whose true recall is 0.7 scores 1 of 4 or fewer with probability 0.3⁴ + 4 × 0.7 × 0.3³ ≈ 0.084, and is then removed.
- The check is kept on cal alone. Pooling val would bias recall upward, because val set the thresholds.
- Each removal is reported with its cal hits and positives, so a removal at 4 positives can be read as the small-sample case it may be.

**The gate needs something to detect.** Removing heads can make gate positives `n/a`, and an arm that says nothing passes every clean text. **So the gate counts only if at least 2 of its 3 menu positives** (`semicolon`, `sessionid`, `member`) **still apply after menu reduction.** Otherwise the arm is recorded as not gate-able, which is a fail. The span gate keeps whichever of its 2 menu texts still apply, and needs at least 1.

## Step 5 — the gate

- **The gate:** phase 1's 10 texts plus the surviving new clean texts, judged over the final menu.
- **The span gate:** phase 1's menu texts that still apply.
- **The determinism check comes first,** and each arm runs once, labelled as such.
- **Passing:** every applicable text passes, the span gate passes, and 0 rows error.

## A diagnostic arm, which cannot ship

**Superseded at Stage 2's registration:** arm B, Stage 1's three checkpoints of the carried recipe, replaces D and D2. B differs from the phase-1b arm only by the cross-rule term; D and D2 differ in the recipe too, and D2 never learned. The text below is the earlier draft's, kept for the record, and is not part of the registration.

**D** is phase 1's L2 checkpoint (`db18339d…`), unchanged, taken through Step 4's calibration, thresholds, pre-gate check and the gate, on the same admitted heads. **D2** is the phase-1-settings checkpoint at seed 20260937, from the seed-floor diagnostic, taken through the same steps.
- **It separates the two changes:** whether the evaluation change alone (thresholds over all cells) would have fixed phase 1, or the training change is needed.
- **D is reported whatever it shows, and cannot ship.** Phase 1's stopping rule bars rescuing a phase-1 arm by re-thresholding, so D is evidence about the cause, never a candidate.
- **The causal comparison is made on a common menu,** across all four checkpoints: D, D2, L2-1b and L2-1b-s2. Step 4 can remove different heads from each, and a different menu alone can change a gate result. So every checkpoint's gate is reported twice:
  - on its own final menu, which for L2-1b is the registered result and the one the ship rule reads;
  - on the **common menu**, the heads on all four final menus.

  **Only the common-menu reading supports a claim about the cause, and only if both phase-1b seeds pass and both phase-1 checkpoints fail.** Any other pattern is reported as it falls, and the causal claim is withheld. If the common menu keeps fewer than 2 of the 3 gate positives, the comparison is reported as not possible.

## After the gate

**If L2-1b passes,** phase 1's Stage 4 continues as registered: T primary, Score A secondary, then C1 and Score B, with the ship rule unchanged. Two points are fixed now:

- **T, which is scored per cell.** For each T row whose rule is on the final menu:
  - a positive row is a **hit** when the rule fires on its text and the claim is the target unit;
  - a negative row is a **false fire** when the rule fires on its text.

  Counts are reported with Wilson 95% intervals, **pooled only.** At freeze, T's menu positives are at most 10 in total (`question_asked` 5, `selector_narrow` 3, `open_artifact` 1, `run_tool` 1), so per-rule T claims are withheld.
- **The form of Score A is registered in an amendment before Score A runs.** Haiku-only rules make the completeness check's 22-rule set need a decision. No Score A number is read before that amendment.

**If L2-1b fails the gate,** the local route stops for this data and backbone, and it is recorded. A further attempt needs more data or another backbone, and a new registration.

## Predictions

**Superseded at Stage 2's registration** by § *Stage 2*'s predictions 1–6. Those restate prediction 6 here per seed (Stage 2's prediction 4) and carry prediction 1 (Stage 2's prediction 6). The rest were about arms D, D2, L2-1b and L2-1b-s2, which Stage 2 replaced. Kept as the earlier draft's text.

1. The audit admits at least 10 of the 14 heads.
2. Of the admitted heads, the pre-gate check removes at most 2.
3. **On the common menu, D and D2 fail the gate, and L2-1b and L2-1b-s2 pass every clean text except `clean-12`.** Re-thresholding over all cells does not stop confident clean-text fires like `d_sessionid` at p = 0.99, and the training change does.
4. L2-1b passes all 5 of phase 1's clean texts.
5. L2-1b hits every applicable phase-1 gate positive.
6. **L2-1b fires `d_semicolon` on the swap text `clean-12`, and so fails the gate.** Every training positive for that rule contains `&&`, and nothing in the data separates `&&` elsewhere from `&&` chaining the test lanes. So the cheapest thing left for the head to learn is the token. This is the Snow Pheasant review's suspicion, stated as a prediction so the swap text can refute it.

## Known limits

- **One seed and one run,** as in phase 1. *(Stage 2: three seeds per arm, read as counts; runs are not bit-reproducible.)*
- **Phase 1's gate texts have now been seen,** and they shaped this design. The new clean texts are the counterweight. The old texts stay in the gate, so the gate is at least as hard as phase 1's.
- **The audit's two labellers are models.** Opus shares a model family with the spec author and the synthetic generator. The union rule is conservative for admission, and it can mask cells that are in fact negative, which costs data and never correctness.
- **Removing heads trades coverage for precision.** A head removed at Step 1 or Step 4 moves to Haiku, and the local menu that ships may be smaller than 14.
- **The Wilson bound assumes independent draws.** The 300 rows can include both texts of a pair, which share every unit but one, so two audit items can be near-copies. That makes the interval somewhat optimistic. The draw is reported with the count of pairs that contributed both texts.
- **The loss change interacts with calibration.** Cross-rule negatives shift every head toward NO, and temperatures and thresholds are refitted over the new cell set to absorb that. D controls for the threshold half of that interaction, not the calibration half. *(Correction 2026-09-28, review-catches:RC-18: D is taken through Step 4's calibration and its thresholds, so it controls for the combined post-training procedure applied to old weights, not for one half of it. It does not separate calibration from thresholds, or either from the training change (`docs/research/2026-09-25-codex-phase1b-draft-review.md` § *Causal comparison needs a fixed reporting surface*).)* *(Stage 2: arm B takes the same refit over the same cells, so the refit is common to B and N. And a temperature cancels from every threshold decision except where floats saturate, which `step4.py` reports (§ *Step 4's scripts*), so the calibration half reaches a decision only through saturation.)*

## Revisions before registration

This draft was reviewed cold by Codex before registration: `docs/research/2026-09-25-codex-phase1b-draft-review.md`. Nothing had been sampled or run. Three changes followed, each verified before it was made.

1. **The audit kept each sentence's context.** The first draft sampled distinct (unit text, text rule) keys, merging paragraphs, although a verdict can depend on its paragraph. 5,422 of the 8,303 keys appear in more than one paragraph (recomputed here, matching the review). The audit now samples unit instances, each in its own text.
2. **The bound now measures the population training consumes.** Distinct keys are not the occurrences the loss averages over. The audit now draws a row, then one unit within it, which selects instances approximately in proportion to their weight in Step 3's cross-rule term. The approximation is stated in Step 1.
3. **D is compared on a common menu.** Step 4 can remove different heads from each arm. The causal comparison is now read only on the heads both arms keep.

Also changed as a consequence: masking a flagged cell now applies to its unit text everywhere it appears, and a limit on the Wilson bound's independence assumption is added.

4. **A leakage review, then measurements before registration** (the Snow Pheasant, LLM lens). It found no leakage in the design, but insufficient evidence for the claims the design is built to make.
   - **Swap texts** (`clean-12` to `clean-14`) were added, so a head that learned a token cannot pass as having learned a rule.
   - **Three Codex-written clean texts** were added, because the other new clean texts come from the training generator's model family.
   - **A second phase-1b seed (L2-1b-s2) and D2** were added, and the causal claim now needs both seeds to agree, because a single training run has no variance floor.
   - **The recall check's false-drop probability** at 4 cal positives is now stated.
   - **A permutation null, a seed floor and a surface-token probe** were registered in phase 1 (§ *Diagnostics after the stop*) and run before this registration. Their results are in § *Measurements taken before registration*.
