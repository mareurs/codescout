1. The conclusion that mined pairs cannot get any rule to 50 positives is unsupported because no candidate has a rule label and the sampled “genuine-pair” rate is not rule-hint precision.

   - Evidence: all 946 rows have `rule: null`; 615 have no hint, while `closed_population` has 169 heuristic matches and `count_unit` has 50 ([mine_pairs.py:63](/home/marius/work/claude/codescout.worktrees/rmvc-s2-codex-cold-0/docs/evals/data/2026-09-24-rule-tell/stage2/mine_pairs.py:63), [summary.txt:8](/home/marius/work/claude/codescout.worktrees/rmvc-s2-codex-cold-0/docs/evals/data/2026-09-24-rule-tell/stage2/summary.txt:8)). The 10-row sample records the rows but not per-row audit verdicts. Nevertheless, the preregistration applies its overall 30–40% pair-quality estimate specifically to `count_unit` and declares it the nearest rule ([phase1-local-classifier-preregistration.md:368](/home/marius/work/claude/codescout.worktrees/rmvc-s2-codex-cold-0/docs/evals/phase1-local-classifier-preregistration.md:368)).
   - Impact: “synthetic pairs are required” and the synthetic-first handoff are not established; rule labelling could reveal ≥50 positives for a rule, particularly among the 615 unhinted candidates or 169 `closed_population` hints.

2. The candidate build is not reproducible from the committed checkout: the miner requires an omitted `gitlog.patch`, reads a hard-coded different checkout, and tells users to generate input from a moving branch.

   - Evidence: the README says the script regenerates the omitted 97 MB input ([README.md:78](/home/marius/work/claude/codescout.worktrees/rmvc-s2-codex-cold-0/docs/evals/data/2026-09-24-rule-tell/README.md:78)), but `main()` only reads `HERE / "gitlog.patch"` ([mine_pairs.py:237](/home/marius/work/claude/codescout.worktrees/rmvc-s2-codex-cold-0/docs/evals/data/2026-09-24-rule-tell/stage2/mine_pairs.py:237)). Running it returned `FileNotFoundError`. Its source root is fixed to `/home/marius/work/claude/codescout` ([mine_pairs.py:38](/home/marius/work/claude/codescout.worktrees/rmvc-s2-codex-cold-0/docs/evals/data/2026-09-24-rule-tell/stage2/mine_pairs.py:38)); that checkout was at `16c7ed4f427d`, while the reviewed checkout is `898d3ea37e16`. The documented `git log experiments` input now spans 4,347 matching commits rather than the reported 4,325.
   - Impact: the 1,038-found and 92-drop derivation cannot be rerun exactly, and later builds can silently use a different corpus or code version.

3. The published cross-document shingle-collision count is 20, but the committed candidates contain 25 colliding document-group pairs.

   - Evidence: the implementation stores only the first owner of each shingle and therefore misses pairwise combinations when three or more groups share it ([mine_pairs.py:294](/home/marius/work/claude/codescout.worktrees/rmvc-s2-codex-cold-0/docs/evals/data/2026-09-24-rule-tell/stage2/mine_pairs.py:294)). Rebuilding a complete shingle-to-groups inverted index returned 25 pairs, five more than [summary.txt:7](/home/marius/work/claude/codescout.worktrees/rmvc-s2-codex-cold-0/docs/evals/data/2026-09-24-rule-tell/stage2/summary.txt:7) and the claim at [phase1-local-classifier-preregistration.md:364](/home/marius/work/claude/codescout.worktrees/rmvc-s2-codex-cold-0/docs/evals/phase1-local-classifier-preregistration.md:364).
   - Impact: the published leakage-workload number is wrong, and reusing this algorithm for fold collision handling would omit edges.

4. The advertised 946 “candidate pairs” include nine singleton correction-note rows with no corrected twin.

   - Evidence: the miner explicitly emits note candidates with `twin = null` ([mine_pairs.py:15](/home/marius/work/claude/codescout.worktrees/rmvc-s2-codex-cold-0/docs/evals/data/2026-09-24-rule-tell/stage2/mine_pairs.py:15)); recounting the JSONL found 937 rewrite rows with twins and nine note rows without one, matching [summary.txt:4](/home/marius/work/claude/codescout.worktrees/rmvc-s2-codex-cold-0/docs/evals/data/2026-09-24-rule-tell/stage2/summary.txt:4).
   - Impact: at most 937 rows satisfy the registered positive/corrected-hard-negative pair construction, so the published 946-pair count and downstream negative-volume assumptions are overstated by nine.

5. `phase2-score-dp1.py` exits successfully for an empty arm selection and for input error rows, so a partial or vacuous score can appear successful.

   - Evidence: `--arms` silently filters rows without checking that requested arms exist ([phase2-score-dp1.py:302](/home/marius/work/claude/codescout.worktrees/rmvc-s2-codex-cold-0/scripts/phase2-score-dp1.py:302)); error rows are recorded but never make the exit status nonzero, and the function always returns 0 ([phase2-score-dp1.py:307](/home/marius/work/claude/codescout.worktrees/rmvc-s2-codex-cold-0/scripts/phase2-score-dp1.py:307)). An offline stubbed run with `--arms typo` exited 0 with only a header; a run containing one error row also exited 0.
   - Impact: a future re-score can silently omit an arm or exclude errored samples from its denominator, producing an incomplete published comparison despite a green command.

Checks that came out clean:

- Form 4’s saved gate log rederives to 6/8 with zero reported errors.
- Form 4q rederives to gate 6/6 and Score A 924 unique `(case, side, rule)` rows, with zero errors or missing cells.
- All 882 carried form-3 rows are byte-equivalent after removing `carried_from`; exactly 42 `question_asked` rows are fresh.
- Form 4q’s reported transitions and metrics match the data: four new YES verdicts, Q1 2/3, negatives still 5/21, and overall recall 5/17.
- No kept Stage 2 row overlaps the committed held-out sets under the registered 8-token shingle rule, and no kept path matches the held-out-document regex.
- All 442 candidate commit SHAs resolve, every candidate path appears in its correction commit, and there are no duplicate normalized pairs.
- All committed phase-2 replay files have unique `(arm, run)` keys.
- The 17 relevant Python unit tests passed; Python syntax compilation and `git diff --check` also passed.
- The checkout was clean before and after review. No Cargo or Rust tests, model calls, network calls, or web services were used.