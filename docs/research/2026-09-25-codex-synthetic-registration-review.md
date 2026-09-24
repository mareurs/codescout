# Codex review of the synthetic-pairs registration (2026-09-25)

> Verbatim output of `codex exec`, model `gpt-6-astra`, reasoning effort `medium`, fresh `CODEX_HOME`, `--sandbox read-only`, in this repository at `7b16d8c6` (session `01a0d553-8b3d-70b1-aee5-a5add2805c16`). It was asked to review the synthetic contrastive pairs amendment of `docs/evals/phase1-local-classifier-preregistration.md` before anything under it runs. All 10 findings were checked by the registering session and hold. The resulting corrections are that amendment's § *Corrections after a cold Codex review*. **P** below means the pre-registration at `7b16d8c6`, so its line numbers are that commit's.

**I would revise this amendment before generation.** The seed supply appears sufficient, but the audit and split procedures leave routes for test information to affect training, and the generator-transfer claim is too strong.

I reviewed the full pre-registration, generation prompt, requested Stage 2 scripts and data, and `RULES`/`SPECS`. HEAD was `7b16d8c6`; the reviewed files had no differences from that commit. No files were edited, and no generation, training, or build gate was run.

Below, **P** means `docs/evals/phase1-local-classifier-preregistration.md`, **G** means `docs/evals/data/2026-09-24-rule-tell/stage2/synthetic-generation-prompt.md`, and other Stage 2 filenames refer to that same directory.

1. **High — Test audits can determine what enters training.**

   **Evidence:** P:699–710 samples by `(generator, rule)` and drops sources/rules based on disagreement. Claude supplies both training data and T-syn-in (P:668–670), but the audit does not separate those populations. P:697 also preserves the earlier unknown-cell audit; P:273 allows that audit to admit a source’s unknown cells as negatives and contribute audited cells to training.

   **Impact:** A natural implementation pools Claude’s training and test pairs. Test audit results can then decide which training sources, rules, or negative labels survive. This contradicts the requirement that test sets are read only after choices are fixed (P:266, 722). Without explicit fold restrictions, audited test cells could also enter training.

   **Smallest fix:** Define audit populations by **generator, split, and rule**. Only training-side audits may govern training admission. Preserve every audited cell’s original fold. Audit synthetic tests separately, with their results inaccessible to model/prompt selection.

2. **High — The synthetic held-out sets are not explicitly protected by the leakage filter.**

   **Evidence:** P:690 names the seed, previously held-out texts, and mined T rows. P:691 inherits amendment 3, whose explicit cross-fold comparisons are train–validation and train–calibration (P:279). Neither specifies training versus T-syn-in/T-syn-cross. S is assigned by basename without shingle-connected components (P:658–662).

   **Impact:** Distinct seed documents can contain copied passages or describe the same incident. Generated training and synthetic-test texts can overlap even when neither copies eight tokens from its own seed. The inherited “move the smaller incident group” rule (P:280) also needs an explicit prohibition against moving held-out groups into training.

   **Smallest fix:** Protect S and both synthetic tests explicitly. Check full positive and substituted-negative paragraphs against all training-side inputs. Resolve held-out collisions by dropping the training-side group, never moving test data into training; freeze this policy before generation.

3. **High — Existing mined components miss actual context overlaps with T.**

   **Evidence:** `draw_t.py:27–34` and `mine_pairs.py:314–317` build overlap components from **positive and twin sentences only**, although examples carry `context_before` and `context_after`. P:662 reuses those components.

   A read-only `python3 -B` check using `mine_pairs.shingles` over all four text fields produced:

   ```text
   rest rows overlapping full T context: 51
   admitted rest overlap: 70, 298, 383, 580
   ```

   For example:

   ```text
   rest 70 context_after / T 942 context_before:
   if let ok v out discarded the err
   ```

   **Impact:** The existing component assignment is insufficient to guarantee separation of the actual model inputs. The older final leakage filter could catch these, but only if “text” includes contexts and is applied to mined rows too; the new construction checks are written for synthetic pairs.

   **Smallest fix:** Require a final check over **every actual input paragraph**, mined and synthetic, including substituted negatives. Keep the committed T assignment fixed and drop conflicting training-side groups.

4. **High — Seed selection permits campaign material that the miner deliberately excludes.**

   **Evidence:** P:652–656 excludes held-out texts and overlapping paragraphs, but does not preserve `mine_pairs.py:52–57`’s whole-document exclusion of campaign scoring, registration, data, and review documents. My snapshot census found **246 otherwise eligible paragraphs** matching that exclusion expression.

   **Impact:** Generation can receive campaign outcomes, tailored specifications, or discussions of held-out failures without sharing an exact eight-token sequence with the held-out example. The lexical filter does not prevent that information leakage.

   **Smallest fix:** Apply an explicit whole-document campaign exclusion before extracting seeds, at least retaining the miner’s existing exclusion and covering related reviews/results.

5. **High — The sampling and pilot protocol is not reproducibly fixed.**

   **Evidence:** P:672 supplies a random seed but no ordered paragraph population, rule traversal order, or exact sampling algorithm. “Prose paragraphs” has no parser definition. P:675 leaves scaling and rounding unspecified. P:645 says each call receives five seeds, while P:679 describes 22 pilot calls yielding **22 pairs**, rather than 110.

   **Impact:** Several materially different datasets satisfy the written procedure. Paragraph parsing, seed ordering, pilot consumption, and rounding can be chosen after inspecting data. A numeric RNG seed alone does not prevent that tailoring.

   **Smallest fix:** Commit a deterministic seed extractor and ordered manifest before the pilot, including rule order, seed IDs, draws, fold assignment, rounding, and pilot reservations. State whether pilot calls are an explicit one-seed exception. Freeze retry/replacement rules too.

6. **Medium — Audit size, denominator, and execution remain underspecified.**

   **Evidence:** P:701 specifies `max(10%, 8)` per `(generator, rule)`. Before attrition Claude has 95 pairs per rule, producing a noninteger sample size of 9.5. Attrition or scaling can leave fewer than eight pairs. P:708–711 does not define whether “source” means generator or the common `source: synthetic` field, or how unequal per-rule sampling is aggregated. No fixed audit prompt, output schema, or error policy is supplied.

   **Impact:** The >20% admission decision can change with rounding, pooling, or treatment of failed audit calls. Those choices would occur after generated data exists.

   **Smallest fix:** Freeze split-specific audit instructions and code. Define, for example, `min(N, max(ceil(.1*N), 8))`; define source identity, aggregation weights, and invalid-response handling explicitly.

7. **Medium — The prompt encourages a shared shortcut that cross-generator testing cannot exclude.**

   **Evidence:** G:15–17 directs repairs toward adding scope, units, evidence, or softened claims; G:28–29 explicitly asks surrounding sentences to be hedged. Both generators receive this recipe. Yet P:642 says cross-generator performance shows whether the model learned the rule or generator style.

   **Impact:** Both generators can produce the same label cues: qualified versus unqualified language, longer repaired sentences, or characteristic evidence phrases. Success across generators can reflect the shared prompt rather than rule understanding. The optional bag-of-words probe cannot rule out other shortcuts.

   **Smallest fix:** Narrow the claim to transfer between these two generators **under this construction prompt**. Add predefined counterbalanced constructions—clean confident assertions, violating hedged assertions where applicable, and repairs that do not systematically add length or qualification.

8. **Medium — Mechanical checks do not establish the promised sentence-level labels.**

   **Evidence:** P:685–691 checks substring occurrence and difference, but not that either sentence is exactly one unit under the training segmenter. The audit’s displayed fields omit the explicit `violating_sentence` (P:702), and it does not explicitly require examining the fully substituted paragraph. G:18–19 requires the substitution to read naturally, not explicitly to remove the contextual breach.

   **Impact:** A generated “sentence” can span multiple training units. For context-dependent rules such as `contradiction` (`phase1-span-selector.py:81`), an apparently repaired sentence can remain inconsistent with its paragraph, leaving a false negative.

   **Smallest fix:** Validate both sentences with the actual training segmenter. Give auditors the marked target and both complete paragraphs; require the replacement to eliminate the target-rule breach in context.

9. **Medium — The paired generator comparison can lose its pairing during filtering.**

   **Evidence:** P:673 says the test sets share seeds pair-for-pair and “differ only in the generator.” P:683 discards failing pairs independently, and P:709 can drop a rule for one generator.

   **Impact:** After filtering, the two reported recalls can concern different seed populations. Their difference then mixes generator effects with differential survival. Moreover, generator and auditor family are coupled: Codex audits Claude, while Claude audits Codex.

   **Smallest fix:** Report all generator-specific attrition, and preregister the generator-gap comparison on the common surviving seed IDs. Describe it as a comparison of the two generation-and-audit pipelines unless audit effects are controlled separately.

10. **Low — The shortcut prediction does not match the registered marking rule.**

   **Evidence:** P:730 defines “surface-separable” using AUC ≥0.9 on **T-syn-cross**; P:738 predicts that label on **T-syn-in**. P:728 does not freeze tokenization or logistic-regression settings. Tokenization matters particularly for `d_semicolon`, whose distinction is `&&` versus `;`.

   **Impact:** The prediction has no matching decision rule, and a conventional word tokenizer can discard the decisive shortcut.

   **Smallest fix:** Define separate in/cross indicators and freeze probe features and parameters, retaining punctuation-sensitive features.

**Checks that were sound**

- **The committed T draw reproduces exactly.** Reconstructing components and the seeded draw returned:

  ```text
  25 edges; 230 components; 75 T groups
  all 944 split rows match: True
  ```

- **Seed scarcity is not an established blocker.** Using `git ls-tree`/`git cat-file` at `7b16d8c6`, blank-line paragraphs, whitespace word counts, removal of frontmatter/fenced blocks, exclusion of heading/table/comment blocks, and the miner’s shingle tokenizer, I obtained:

  ```text
  Markdown files: 1868
  eligible paragraphs: 12400
  training side: 9500
  S side: 2900
  ```

  These exceed 1,760 training-pool seeds plus pilot reservations and 330 S seeds. These are counts under my stated extraction rule, not a missing canonical implementation of the amendment.

- `make_label_batches.py:25` really exports the stated `RULES` and form-2b `SPECS`; the selector asserts matching rule keys.
- Twin negatives remain rule-specific, and unknown cells default to masked. Those correct the earlier blanket-negative error.
- The amendment preserves the ≥50 **train-fold** positive requirement, withholds synthetic-test claims below ten surviving pairs, and keeps real-draft claims separate.
- Codex-generated examples are explicitly excluded from training, and no model or threshold selection on T-syn scores is authorized. The audit and filtering gaps above need closing to make those intentions operational.