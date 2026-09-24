I've reviewed the whole scope and recomputed every figure the committed data can support. Of the numbers the data can check, most reproduce. The problems are in a few claims made about those numbers, in two statements that planned next steps rest on, and in results that have no committed rows behind them.

## Findings (most severe first)

**1. The "12 of 12 runs" evidence behind the form-2b ruling is false.**
- (a) The scoring doc says clean-2 "fires `member_vs_population` on every run of both models on both channels, 12 of 12 runs". The local pre-registration repeats this as the basis for the operator's ruling on clean-2.
- (b) `docs/evals/rule-tell-scoring-2026-09-23.md:341` and `phase1-local-classifier-preregistration.md:172` against the logs:
  - `p1s-gate2.txt` (Haiku, contaminated channel): 3/3.
  - `p1s-S0-gate.txt` (Sonnet): 3/3.
  - `p1s-H0clean-gate.txt` (Haiku, clean channel): fired `[['member_vs_population','question_asked'], ['question_asked'], ['d_history']]`, so **1/3**.
  - No Sonnet run on the contaminated channel exists. The real figure is 7 of 9.
- (c) On the clean channel, Haiku fails clean-2 mostly through `question_asked` and `d_history`. So "the spec, not the judge" is overstated, and narrowing `member_vs_population` would not fix Haiku's clean-2 failure. The ruling was made on an inflated count.

**2. The L0 "calibration lead" for Stage 3 does not hold on the logged probabilities.**
- (a) The doc says that on 4 of 5 violation texts the gold rule "ranks 1st or 2nd of 22 … and it scores above that rule's maximum over the five clean texts". Only 2 of 5 meet both conditions.
- (b) Method: I rebuilt text×rule from `l0-gate.jsonl`. The order is deterministic (pool=1), and the rebuilt fired sets match `l0-gate.txt` exactly. Results:
  - **cannot:** `cannot_happen` is 0.600599 on the violation text and also 0.600599 on clean-1 (and on semicolon). Margin 0.00, not "above". The doc's own table (`:568`) shows +0.00, which contradicts its prose (`:561`).
  - **sessionid:** 0.5, tied with 3 other rules behind `lines_read` at 0.58. That is a 4-way tie for 2nd, rank 2–5.
  - **contradiction:** tied at ranks 12–14.
  - The output is coarse: only 41 distinct values across 220 rows, with many exact ties.
- (c) Handoff item 3 (`:617`) cites "rank gold 1st or 2nd on 4 of 5" as the lead for per-rule calibration in Stage 3. The evidence is two texts, and one gold rule cannot be told apart from a clean text at all.

**3. "The claim-bound arms reproduce exactly" is false for RTD-9.**
- (a) `:424` says the clean-channel re-score reproduced RTD-9, RTD-10 and RTD-3 exactly.
- (b) `e2s-score-rtd9.txt` gives 1b **4/9**; `fsc-rtd9.txt` gave **3/9**. The doc's own table at `:412` prints "4/9 [3/9]".
- (c) This "published denominator" was then used as the premise of a registered prediction (`rule-injection-timing-preregistration.md:387`: "since their fork-route rows reproduced exactly").

**4. The stripped-arm prediction "s0 is not below arm 0 … for all three rules" fails under either reading.**
- (a) `:192` claims it held for all three rules. `:456` claims it holds under `rtd8c`.
- (b) Fork-route logs:

  | rule | arm 0 | s0 |
  |---|---|---|
  | RTD-8 (`fsc-rtd8.txt`) | 5/10 = 0.50 | 5/8 = 0.625 |
  | RTD-9 (`fsc-rtd9.txt`) | 0.70 | 5/8 = 0.625 |
  | RTD-10 (`fsc-rtd10.txt`) | 0.80 | 8/8 = 1.00 |
  | RTD-8, `rtd8c` | 9/10 = 0.90 | 7/8 = 0.875 |

  - Read literally ("not below"), RTD-9 fails, and so does `rtd8c` (0.875 < 0.90).
  - Read as the doc glosses it ("did not make violations more frequent"), RTD-8 and RTD-10 fail.
  - The registered wording (`prereg:205`) points the wrong way for its own rationale: "the rule was not preventing it" means s0 is not *above* arm 0.
- (c) The conclusion "the always-present copy was not what prevented them" is not supported. The small-n direction, if anything, leans the other way.

**5. The RTD-10 "judge channel moves rates" example has a second cause.**
- (a) `:161` attributes RTD-10 arm 0 going from 5/10 (API judge) to 9/10 (subscription) to the channel. But the RTD-10 checker was reworded between those two scores.
- (b) `:162` and `prereg:224` record that second rewording, done after the move to the new channel.
- (c) The RTD-10 half of the claim that the shift "is as large as some effects under test" cannot be read as a channel effect. Only RTD-9 (4 → 7, same checker) is.

**6. The local route's planned Stage-4 Score B names a checker that failed its gate and compares against superseded rows.**
- (a) The plan scores with `rtd8`, which failed its clean-channel gate, and its comparison rows are old numbers.
- (b) `phase1-local-classifier-preregistration.md:99` scores with `rtd8`; `e2s-score-rtd8.txt` shows the recorded fixture at NO×3, "GATE FAILED". `:28` fixes the comparison rows at RTD-8 arm 0 5/10 and 1b 0/10, which are contaminated-channel `rtd8` numbers; clean `rtd8c` gives 9/10 and 2/10. `:88` still says the 8-text gate; it is now 10.
- (c) Followed as written, the next registered step cannot score RTD-8, or it compares against invalid baselines.

**7. The RTD-8 observable is not "the doc write", although the doc presents it as the same as RTD-9/10's.**
- (a) The footnote at `:188` says forks missing from an arm's count "took a different first action". For RTD-8 that is not true.
- (b) `scripts/phase2-score-dp1.py:61`: the RTD-8 observable is `"pika_observations" in r["text"]`, not a doc call (`:76`, `:94`). In `fork-dp1-n10.jsonl`:
  - 1b run 2 (`run_command`, 517 characters) is judged.
  - s1b runs 6 and 7 (no tool call at all) are judged.
  - In `fork-e2s-dp1.jsonl`, runs 2 and 5 (no tools) are judged.

  All of these are excluded for RTD-9 and RTD-10 in the same tables.
- (c) RTD-8's column is computed on different rows than its neighbours. The "s1b/1b stays near 0" rates credit forks that never made the write. No verdict flips, but the tables at `:180–186` and `:439–446` are not like-for-like.

**8. Several reported results have no committed evidence, and one log is mislabelled.**
- (a) `:603` says "the raw rows and logs for every result in this document are in" the data directory, and the README's "Each file's result is the one the scoring doc reports" says the same. Neither is true.
- (b) `score-rtd910.txt` holds only four tracebacks (`BadRequestError … usage limits`), yet the README lists it among "the original scores". So the API-judge arm-0 rates for RTD-9 and RTD-10 (4/10 and 5/10, used at `:161`) are unbacked. `grep` also finds no rows for:
  - the Jev `noul` gate table (`:95–101`);
  - Jev's end-to-end picks ("`none` on 10/10, p 0.23–0.46", `:198`);
  - the phase-1A gates (Jev 6/6, Haiku 4/6);
  - the three-form mutation gate (`:34–38`).
- (c) Those claims cannot be re-checked from the repository.

**9. The RTD-3 "a sentence then a `grep`" counts are really counts of any non-doc tool call.**
- (a) `:225–229` describe these forks as "a sentence followed by a `grep`", but the counts include `Agent` dispatches.
- (b) `fork-rtd3r.jsonl`:
  - arm 0: 2 greps + 1 `Agent`;
  - arm 2: 1 grep + 5 `Agent` calls of 5.1–6.2k characters;
  - 3-1b: 2 `run_command` + 1 `Agent` (8.1k characters), and no greps.

  The `Agent` forks are long texts, not "scored before the decision point".
- (c) The worst-case arithmetic bound still holds, so the ship verdict survives. But the scope note ("3 of the 10 3-1b forks opened with a sentence and a `grep`") describes the rows wrongly.

**10. The family-recall "upper bound" is not an upper bound.**
- (a) `:528` says that since the family was drawn after seeing the confusions, "no fairer grouping can do better", giving a ceiling of 5/17.
- (b) Adding `question_asked` to the family makes RTD-11 a hit (6/17). Any regrouping is capped by "fired anything" = gold 3 + wrong-rule 4 = **7/17** (from `p1s-S0f3-corpus.txt`).
- (c) The conclusion (below 0.5, so no spec-tightening run) still holds, but the stated bound is wrong.

**11. The scorer silently defaults to the contaminated profile.**
- (a) `phase2-score-dp1.py` defaults to `~/.claude-kat` with no clean-channel check. `phase1-local-l0.py` inherits that construction when it imports.
- (b) `phase2-score-dp1.py:212–213` builds the judge at import. The "local" L0 script therefore needs Claude credentials (it exits otherwise) and prints "subscription judge" in `l0-gate.txt`.
- (c) A re-run without `JUDGE_CONFIG_DIR` scores on the dirty channel with no warning. The doc (`:622`) mentions this, but the code does not enforce it.

**12. S0 did find RTD-9's claim in Score B; it assigned the wrong rule.**
- (a) `:386` and `:392` say RTD-9 was the rule S0 missed. That is true of Score A, not of the end-to-end drafts.
- (b) `e2s-dp1.jsonl` runs 5, 7 and 9 bind "The write path has fired once, ever." to `selector_narrow`.
- (c) The handoff's "silence is the larger failure" diagnosis does not describe RTD-9's Score B failure, which was rule assignment.

**13. Small count drift in the gate tables (low).**
- Form-1 clean-1 is shown as "3–5" fires; the log says 4–5.
- "cannot +12" should be +11–13.
- S0 contradiction "+0–4" should be +1–4.
- The pre-registration's "4–13 unrelated rules" (`prereg:316`) should be 3–13.

## Checks that came out clean
- **Phase 0:** 2,850 rows, 285 tasks × 10, 0 errored, none missing reasoning. The positive fire rates (1.00 / 0.90 / 0.80 / 0.00) all match. So do the near-miss and full-shape means and counts for every prompt, "1 of 8 near-miss", RTD-10's 9/12, and the off-diagonal rates (0.33 / 0.30 / 0.22 / 0.08 / 0.07). The retraction's "44 one-directional disagreements, 22 fractional YES" matches `scored-claude-textpath.jsonl` against `native.jsonl`.
- **Phase 1A:** Jev 12/51, 27/51, 30/63; Haiku 12/85, 30/85, 1/105; mean P(none) 0.27 / 0.24; the distribution differs on 42/42 states and the top pick is stable on 33/42; the RTD-8/10/13/15 pair values; the 23-option menu composition.
- **Every phase-2 rate table against its log:** `phase2-dp1-score.txt`, the `score-rtd9/10-*sub.txt` files, `api-score-*`, `fsc-*`, `e2s-score-*`, `rtd8c-score.txt`. All ship-rule differences recomputed. The RTD-10 e2s 5/10 alternative, the RTD-3 worst-case bound (5/7, 2/4), the stripped-file error count (17/30), the superseded-date rows (3), and the e2s injection and "no injection" counts (DP1 2, RTD-3 6) all match.
- **Score A (2b and form 3):** every bucket cell, 3/17, 2/21 and 5/21. The Wilson interval [0.06, 0.41] recomputed correctly, as did the per-positive gold / wrong / silent tables (3/1/13 and 3/4/10) and the claim localisation 3/3.
- **Gates:** the H0 / H0-clean / S0 / 2b / form-3 / L0 gate and span-gate outcomes, and the "10/10, 30 clean runs, 1 fire" form-3 comparison.
- **Data integrity:** `fork-rtd3-old-all.jsonl` is exactly `arm0` + `arms`, and it shares no rows with `fork-rtd3r.jsonl`, so the exploratory re-score is separate from the main result. All 50 `rtd3r` forks carry text. The `score-b-s0.sh` partial-output guard is present.

`★ Insight ─────────────────────────────────────`
- The worst errors are all of one kind: a count summarised in prose rather than derived from the row files. Examples are "12 of 12", "4 of 5", and "reproduce exactly". The table next to each claim is right, and the sentence summarising it is not.
- Finding 7 is a case of one scorer applying two different observables to the same fork file. RTD-8 counts any reply that mentions the table; RTD-9 and RTD-10 count only doc writes. The result is that one table's columns come from different rows.
- Finding 2 shows the local model's output is quantised: 41 values over 220 rows, with frequent exact ties. That limits what per-rule calibration can recover, which is worth knowing before starting the 1.5–3 weeks of Stage 2 work that Stage 3 depends on.
`─────────────────────────────────────────────────`