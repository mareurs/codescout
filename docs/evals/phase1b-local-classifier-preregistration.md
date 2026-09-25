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

**Status: Stage 1 registered by the commit that adds its section below; Stages 2 and later are DRAFT and not registered.**
- **Stage 1,** a stable training recipe, runs after that commit.
- **The sections from "Step 1 — the cross-rule audit" onward** are the Stage-2+ draft. They will be revised in light of Stage 1 and of `docs/research/2026-09-25-phase1-training-research-synthesis.md`: rule-conditioned formulation, cue-balanced training rows, all-cells validation, and failure rates over all seeds in place of the "learned-only" causal reading, which the research showed to be selection after treatment. Each stage is registered before it runs.

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

- **The permutation null passes:** pooled val AUC 0.498, and every epoch's val loss is at least 0.68. The split carries no link but the labels, so phase 1b's precondition holds.
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

1. The audit admits at least 10 of the 14 heads.
2. Of the admitted heads, the pre-gate check removes at most 2.
3. **On the common menu, D and D2 fail the gate, and L2-1b and L2-1b-s2 pass every clean text except `clean-12`.** Re-thresholding over all cells does not stop confident clean-text fires like `d_sessionid` at p = 0.99, and the training change does.
4. L2-1b passes all 5 of phase 1's clean texts.
5. L2-1b hits every applicable phase-1 gate positive.
6. **L2-1b fires `d_semicolon` on the swap text `clean-12`, and so fails the gate.** Every training positive for that rule contains `&&`, and nothing in the data separates `&&` elsewhere from `&&` chaining the test lanes. So the cheapest thing left for the head to learn is the token. This is the Snow Pheasant review's suspicion, stated as a prediction so the swap text can refute it.

## Known limits

- **One seed and one run,** as in phase 1.
- **Phase 1's gate texts have now been seen,** and they shaped this design. The new clean texts are the counterweight. The old texts stay in the gate, so the gate is at least as hard as phase 1's.
- **The audit's two labellers are models.** Opus shares a model family with the spec author and the synthetic generator. The union rule is conservative for admission, and it can mask cells that are in fact negative, which costs data and never correctness.
- **Removing heads trades coverage for precision.** A head removed at Step 1 or Step 4 moves to Haiku, and the local menu that ships may be smaller than 14.
- **The Wilson bound assumes independent draws.** The 300 rows can include both texts of a pair, which share every unit but one, so two audit items can be near-copies. That makes the interval somewhat optimistic. The draw is reported with the count of pairs that contributed both texts.
- **The loss change interacts with calibration.** Cross-rule negatives shift every head toward NO, and temperatures and thresholds are refitted over the new cell set to absorb that. D controls for the threshold half of that interaction, not the calibration half.

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
