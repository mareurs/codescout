`★ Insight ─────────────────────────────────────`
- In this campaign, the doubtful claims sit around the data rather than in it: the row files match their summaries exactly. The problems are in the sentences that read those rows: which sentence a quote came from, what an estimate multiplies, and what a filter was keyed on.
- The held-out filter compares 8-word overlaps between texts, so it only catches copied wording. A candidate from the same commit and document as a Score A case, but worded differently, gets through.
`─────────────────────────────────────────────────`

# Review: S0 form 4 / form 4q and the Stage 2 mined-pair build (`9c0d2505..898d3ea3`)

Nothing was edited, no git state changed, and no model was called. Scratch files are in `/tmp/rv-s2-claude-primed-1/`.

## Findings, most serious first

**1. The Stage 2 claim that no rule reaches 50 positives, and that `count_unit` is nearest, is wrong on its own numbers and not derived.**
- (a) The pre-registration says *"`count_unit` is nearest at 50 raw hints, about 15 after the sample's precision"*. That estimate multiplies how often a pair is a real correction by how many rows got a hint, and it leaves out the largest group of hints.
- (b) Evidence:
  - `stage2/summary.txt` shows `closed_population` with 169 rows under any hint and 141 as the single hint, against `count_unit`'s 50. At the note's own 30–40%, 169 comes to about 51–68, which is over the bar.
  - The 30–40% is a rate of real before-and-after pairs. Nobody measured how often a hint names the right rule.
  - The 615 rows with no hint are not counted, so the hint counts are not an upper bound.
  - n = 10 gives a Wilson 95% interval of [11%, 60%] for 3/10 and [17%, 69%] for 4/10 (computed).
  - Location: `docs/evals/phase1-local-classifier-preregistration.md` § *Stage 2 status*, "Against the stopping rule".
- (c) Impact: the operator decision is partly justified by this sentence ("the route cannot run on mined pairs alone"). It may still be true, but these numbers don't establish it. The note should publish the interval and state that per-rule yield is unmeasured.

**2. The held-out filter lets through a candidate from the same commit and document as Score A cases, and one from this campaign's own tracker. The note says both kinds were dropped.**
- (a) The filter is keyed on 8-word overlaps with held-out texts plus a path regex. It does not check a Score A case's source commit or document.
- (b) Evidence, from `stage2/mined-candidates.jsonl`:
  - One kept row is sha `e365a6b3`, path `docs/adrs/2026-09-21-tracker-state-splits-…md`, incident `…@ddfce06545`. `rule-tell-detection.md:779-781,805` gives `e365a6b3` in that ADR as the source of RTD-12 and RTD-13, and `ddfce065` as the source of RTD-12's hedge. The kept row is the neighbouring "Medium-high on the boundary" confidence paragraph.
  - Another kept row is `d8e6f4d5`, `docs/trackers/prompt-hamsa-audit-log.md`: *"The claim-bound reminder stops RTD-8" is withdrawn pending `rtd8c`*. That is campaign material, but `HELD_OUT_DOC_RE` (`mine_pairs.py:54-56`) only matches the eval, research and data paths.
- (c) Impact: only 2 of 946 rows, but the claim that material from held-out sources is dropped is false. A future labeller could admit the ADR row as a training positive from Score A's own correction incident. The filter should also key on each case's `source` commit and document.

**3. The form 4q write-up says RTD-12 was "quoted on the audit's own sentence". It was not.**
- (b) Evidence:
  - The 4q row quotes *"The allocator's durability was verified at `append_entry.rs:294-324`; …**not exercised end-to-end…**"*.
  - The audit's quote for the violation is *"So **id allocation already survives a catalog loss**…"* (`partial-audit-verdicts.json`, RTD-12 `quote`).
  - The sentence 4q fired on holds the audit's `quote2`, which the audit recorded as the caveat (the outside condition), not as the violation.
  - Location: `rule-tell-scoring-2026-09-23.md`, § *S0 form 4q*, Q1 bullet ("each quoted on the audit's own sentence").
- (c) Impact: the backing claimed for "S0 now finds the violation in RTD-1 and RTD-12" is overstated. The fire does land on the wrong-allocator citation, so it is probably still a real hit. But it matches the audit's sentence only for RTD-1, and the operator summary repeats the stronger version.

**4. The form 4 reading blames the widening for the clean-4 fires without the form 3 baseline, which already fired on clean-4.**
- (b) Evidence:
  - `form3-gate.txt` has clean-4 at `hit 2/3 fired [[], [], ['count_unit', 'scope_instant']]` under the unwidened specs.
  - Form 4 moves this to 3/3 (`form4-gate.txt`).
  - The doc says *"widening the rest made the judge apply it"*.
- (c) Impact: clean-4 went from 1/3 to 3/3 on 3 runs, so it is weak evidence for "the derive-it half cannot be written as a sentence shape". Clean-3 (0/3 to 3/3) is the clean evidence, and the write-up should say so.

**5. The README says the miner regenerates its git-log input. It does not, and the input can't be rebuilt reliably.**
- (b) Evidence:
  - `README.md:78` says *"the script regenerates it"*.
  - `mine_pairs.py:238` only reads `HERE/"gitlog.patch"`. There is no subprocess call, and the `git log experiments …` command exists only in the docstring (`:8`).
  - The `experiments` tip used for the extract is not recorded, and `experiments` is rebased after every ship.
  - Today all 442 candidate SHAs are still ancestors of HEAD (checked), but no patch-id is stored.
- (c) Impact: "4,325 commits scanned" and the 946 rows can't be reproduced after the next rebase.

**6. Two committed scripts read from the main checkout's path, not from the committed tree beside them.**
- (b) Evidence: `form4q-readout.py:5` and `mine_pairs.py:38` both set `ROOT = /home/marius/work/claude/codescout`. From there they load the selector, the form 3 rows, the controls and the fork drafts.
- (c) Impact: `form4q-readout.txt` and the held-out set reflect whatever that checkout held when the scripts ran, which may differ from the committed files. The scripts can't be rerun anywhere else.

**7. The new `--out` header can't trace every checker change, as the handoff claims.**
- (b) Evidence: `phase2-score-dp1.py` hashes only `rule["question"]`. The `TAIL` appended to every judge prompt (`:223`), the `--request` recorded fixture and the `--corrected` fixture are not hashed. The gate log stores fixture names, not their contents.
- (c) Impact: a change to the prompt tail or a gate fixture leaves the header identical, so the claim "a later checker change can be traced row by row" holds only for edits to the question text.

**8. Form 4q's "no new false-positive text" rests on a narrower check than it reads, and the new gate path is untested.**
- (b) Evidence:
  - The 4q gate marks 7 other-rule positives as n/a. Each of them is a negative for `question_asked`, and under form 3 `question_asked` already fired 3/3 on `member` (`form3-gate.txt`).
  - Precision was therefore checked on only 5 clean gate texts plus the Score A negatives. RTD-13's negative did gain a `question_asked` fire.
  - No test covers `gate()`'s n/a / `applicable` logic, or the refusal of `--rules` without `--carry`. The one new test covers only the corpus merge.
- (c) Impact: low for the numbers, but the gate would not have caught the widened `question_asked` firing on those 7 other-rule gate texts.

**9. (Plausible, not proven) The plan doesn't reserve T's sources before the mined candidates are read or labelled.**
- (b) Evidence: the pre-registration's Stage 2 text (`:66`, `:74`) says T is built in this stage from source documents disjoint from training, and must be one the designing agent hasn't seen. The Stage 2 status and the handoff send "spec breadth tested on T" forward. But the miner covers all 250 document groups, and nothing sets T's documents aside before this author or a labeller reads the candidates.
- (c) Impact: the next step the write-up relies on, testing the wording where the author has not seen the texts, could be compromised.

## Checks that came out clean
- `p1s-S0f4q-corpus.jsonl`:
  - 924 rows, 42 of them fresh `question_asked` rows (one per text); 882 carried rows.
  - Carried rows are identical to `p1s-S0f3-corpus.jsonl` once `carried_from` is removed (0 differ).
  - No duplicate keys in the form 3 file.
- Score A table (form 3 → 4q), recomputed from the rows:
  - `yes` bucket: gold fired 3/8 → 3/8; gold only 2/8 → 1/8.
  - `partial` bucket: gold fired 0/9 → 2/9.
  - Negatives with any fire: 5/21 → 5/21.
  - Recall: 3/17 → 5/17. Positives firing nothing under form 3: 10 of 17. Wrong-rule fires: 4 of 17.
  - `question_asked` fires only on RTD-1, RTD-8, RTD-12 and the RTD-13 negative.
- The form 4 and form 4q gate tables in the doc match `form4-gate.txt` and `form4q-gate.txt` exactly (6/8 and 6/6, 0 errored rows).
- The P2 case list (RTD-1, 2, 4, 9, 11, 12, 16, 17, 19), the Q1 list (RTD-1, 11, 12) and the V cases (RTD-1, 12, 16) match the gold labels.
- Stage 2 counts reproduce from the committed jsonl:
  - 946 kept; marker sources 230 / 542 / 165 / 9; 513 incidents; 250 document groups.
  - 615 rows with no hint; `closed_population` 169; `count_unit` 50.
  - Dropped: 92 = 70 + 20 + 2. The 10-row sample reproduces with seed 20260924.
- My own reading of that sample agrees with the note: about 3 real pairs, 1 weak, 5 mismatched, 1 fragment.
- All 442 candidate SHAs are ancestors of HEAD.
- The three form 4 gate texts appear only in the scoring doc, which is already held out, so they don't leak into the candidates.
- `SPECS` is rebound per `--form` and read at call time (`phase1-span-selector.py:175`). The behaviour change in 4q confirms the widened wording was actually used.
- `pytest tests/test_phase1_span_selector_report.py tests/test_phase2_score_dp1.py`: 17 passed.