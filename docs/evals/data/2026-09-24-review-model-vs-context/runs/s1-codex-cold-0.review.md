## Findings

1. **The campaign does not apply its preregistered falsification rule to H2’s predictor.**

   - Evidence: the preregistration predicts RTD-9 and RTD-10 injections will fail and says a scrambled partition makes the predictor “dead” ([rule-injection-timing-preregistration.md:54](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-cold-0/docs/evals/rule-injection-timing-preregistration.md:54), [line 59](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-cold-0/docs/evals/rule-injection-timing-preregistration.md:59)). Both instead ship decisively, with bound-arm rates of 0/9 and 0/10 ([rule-tell-scoring-2026-09-23.md:583](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-cold-0/docs/evals/rule-tell-scoring-2026-09-23.md:583)).
   - Impact: the preregistered structural predictor is falsified for two of four tells; future rules cannot be classified from trigger/compliance shape as proposed, but the results and handoff never record that central conclusion.

2. **The local-classifier plan still directs future Score B runs to the invalid `rtd8` checker and stale contaminated-channel baselines.**

   - Evidence: Stage 4 names `rtd8` ([phase1-local-classifier-preregistration.md:99](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-cold-0/docs/evals/phase1-local-classifier-preregistration.md:99)) and earlier baselines of 5/10 versus 0/10 ([line 28](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-cold-0/docs/evals/phase1-local-classifier-preregistration.md:28)). The scoring record later establishes that `rtd8` fails its clean-channel gate and withdraws its finding ([rule-tell-scoring-2026-09-23.md:398](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-cold-0/docs/evals/rule-tell-scoring-2026-09-23.md:398), [line 426](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-cold-0/docs/evals/rule-tell-scoring-2026-09-23.md:426)); the valid fork comparison is `rtd8c`, 9/10 versus 2/10 ([line 439](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-cold-0/docs/evals/rule-tell-scoring-2026-09-23.md:439)).
   - Impact: the planned trained-arm evaluation can either stop on a known-bad gate or calculate its ship margin against superseded rates.

3. **Stage 2 claims that diffs supply rule labels, but a correction diff identifies changed text, not which of the 22 semantic rules it violated.**

   - Evidence: the plan says labels come from construction, then says the diff supplies the mined pair’s label without registering any rule-assignment procedure ([phase1-local-classifier-preregistration.md:51](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-cold-0/docs/evals/phase1-local-classifier-preregistration.md:51)). Its audit samples only 10% and checks against the already-assigned rule ([line 58](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-cold-0/docs/evals/phase1-local-classifier-preregistration.md:58)).
   - Impact: the next 1.5–3-week data build has no reproducible ground-truth mechanism for its mined examples; incorrect semantic labels could determine the trained model’s apparent success or failure.

4. **The committed archive is insufficient to independently derive two important result classes despite claiming to contain raw rows/logs for every result.**

   - Evidence: the archive claims to be the evidence behind every reported result ([data README.md:3](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-cold-0/docs/evals/data/2026-09-24-rule-tell/README.md:3)); the scoring document specifically claims ten Jev draft classifications ([rule-tell-scoring-2026-09-23.md:196](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-cold-0/docs/evals/rule-tell-scoring-2026-09-23.md:196)) and says raw rows/logs exist for every result ([line 603](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-cold-0/docs/evals/rule-tell-scoring-2026-09-23.md:603)). A schema inventory found Jev rows only in `phase1-jev.jsonl`, containing the 126 corpus classifications, not the ten DP1 drafts. It also found no JSONL holding phase-2 per-replay votes or verdicts: the scorer computes votes in memory and prints only aggregates ([phase2-score-dp1.py:269](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-cold-0/scripts/phase2-score-dp1.py:269)).
   - Impact: the “Jev picked none on 10/10” conclusion cannot be checked at all, and the clean phase-2 rates can only be copied from aggregate logs, not recomputed from committed evidence without new model calls.

5. **`phase2-score-dp1.py` silently succeeds when a requested arm is wholly absent, and similarly excludes error rows without failing.**

   - Evidence: `--arms` only filters rows; there is no expected-arm/run validation, while error rows are counted and skipped before an unconditional return 0 ([phase2-score-dp1.py:257](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-cold-0/scripts/phase2-score-dp1.py:257)). With the judge stubbed locally, I requested arms `0,3-1b` from a file containing only arm 0; it printed only arm 0 and returned 0. The pipeline guard checks only that each generated file has ten lines ([score-b-s0.sh:31](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-cold-0/docs/evals/data/2026-09-24-rule-tell/score-b-s0.sh:31)), so ten error or duplicate rows also pass.
   - Impact: a missing comparison arm or failed generation can silently become a smaller denominator or disappear entirely, allowing an invalid ship comparison to look successful.

6. **Score A’s advertised completeness check does not detect a wholly missing text.**

   - Evidence: `report_corpus` checks rule multiplicity only for `(case, side)` groups that are present ([phase1-span-selector.py:333](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-cold-0/scripts/phase1-span-selector.py:333)), contradicting the document’s statement that partial sweeps are refused ([rule-tell-scoring-2026-09-23.md:313](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-cold-0/docs/evals/rule-tell-scoring-2026-09-23.md:313)). Removing all 22 `RTD-1` positive rows in memory produced `41 texts, 902 rows, 0 incomplete` and returned 0.
   - Impact: an omitted case silently changes recall and precision denominators while being reported as complete.

7. **The claim-span validator accepts exact multi-sentence excerpts although the protocol requires one sentence and says joined sentences are refused.**

   - Evidence: the prompt requires “the one sentence” and forbids joining sentences ([phase1-span-selector.py:103](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-cold-0/scripts/phase1-span-selector.py:103)), but `verify_span` accepts any sufficiently long normalized substring ([line 130](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-cold-0/scripts/phase1-span-selector.py:130)). Its “joined sentences refused” test uses a paraphrase rather than two exact adjacent sentences ([line 281](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-cold-0/scripts/phase1-span-selector.py:281)). Executing the source functions directly accepted `"The field is unread. Nothing in the scheduler consumes it."` as one claim.
   - Impact: future selectors can inject compound excerpts and pass claim-localisation checks even though phase 2 established only a specific sentence-bound claim. The committed Score A/e2e claims did not exercise this defect.

8. **The scoring document falsely says all three clean-channel claim-bound arms reproduced exactly.**

   - Evidence: it makes that claim at [rule-tell-scoring-2026-09-23.md:424](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-cold-0/docs/evals/rule-tell-scoring-2026-09-23.md:424), but its own table reports RTD-9 as 4/9 clean versus 3/9 contaminated ([line 412](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-cold-0/docs/evals/rule-tell-scoring-2026-09-23.md:412)). The underlying logs confirm 4/9 clean ([e2s-score-rtd9.txt:8](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-cold-0/docs/evals/data/2026-09-24-rule-tell/e2s-score-rtd9.txt:8)) and 3/9 contaminated ([fsc-rtd9.txt:8](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-cold-0/docs/evals/data/2026-09-24-rule-tell/fsc-rtd9.txt:8)).
   - Impact: the published “contamination changed no comparison” denominator is false for RTD-9, although the one-row shift does not change its ship verdict.

9. **The local preregistration has two incompatible Stage-4 gate populations.**

   - Evidence: Stage 4 still specifies the eight-text gate ([phase1-local-classifier-preregistration.md:86](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-cold-0/docs/evals/phase1-local-classifier-preregistration.md:86)); the form-2b amendment says the gate is now ten texts ([line 183](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-cold-0/docs/evals/phase1-local-classifier-preregistration.md:183)); and the L0 amendment calls the ten-text set the current, fair comparison ([line 251](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-cold-0/docs/evals/phase1-local-classifier-preregistration.md:251)).
   - Impact: a trained arm can be passed or rejected on different fixture populations, and its gate will not be comparable with S0/L0 until the registration names one set.

## Checks that came out clean

- Verified HEAD is `a8835d06b41dd6901943a796ad2b18aa8abad933`; the checkout remained clean.
- Parsed every committed JSON/JSONL file successfully.
- Recomputed phase-0 row counts, positive gates, diagonal near-miss/full-shape rates, and off-diagonal rates; they match the scoring document.
- Recomputed Jev/Haiku top-1, top-3, corrected-`none`, stability, and probability-summary figures; they match.
- Recomputed both 924-row Score A tables and L0’s 4/10 gate/ranges; they match.
- Verified every non-discarded replay arm has the expected unique run IDs and no error rows; the stripped API file has the disclosed 17 errors.
- Verified both e2e injection files join exactly by run to their fork files.
- Verified all committed Score A/e2e claim spans are single-sentence under the campaign’s splitter.
- All four Python scripts compile; `score-b-s0.sh` passes `bash -n`.
- Credential-pattern scan found no Anthropic, OpenAI, GitHub, or bearer-token patterns.
- Per instruction, I made no network/model calls and ran no Cargo or Rust tests.