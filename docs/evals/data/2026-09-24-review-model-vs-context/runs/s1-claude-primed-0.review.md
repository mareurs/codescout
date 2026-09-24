I found 14 issues. Two of them affect published conclusions, and one would send the next step (Stage 2/3) off course. Most shipped verdicts do survive when I recompute them from the committed rows.

## Findings

**1. Phase-0 precision results are published even though the corpus's own stop rule was triggered.**
- (b) `docs/evals/rule-tell-controls.md:1311-1312` says to score the five known positives first and, if any draws a NO, stop, because "the control results from that run are uninterpretable". I re-ran `scripts/analyse-rule-tell-run.py --scored …/scored-reasoned-n10.jsonl --controls docs/evals/rule-tell-controls.md`. It printed `CTL9-4 fire rate 0.00 <-- FAIL`, `CTLX-5 0.00 <-- FAIL` and `GATE: FAIL`. The scoring doc still publishes "Only **1 of the 8** near-miss controls fires" (`rule-tell-scoring-2026-09-23.md:73`) and the RTD-10 verdict. The one near-miss that fires belongs to RTD-9, the prompt whose own positive failed.
- (c) The headline precision reading comes from a run the registered protocol says cannot be read for precision.

**2. A registered prediction that was met is reported as not supported.**
- (b) The pre-registration (`rule-injection-timing-preregistration.md:123`) registers: "RTD-10 draws YES on ≥ 3 of the 12 … controls, **and does not survive**." The corpus (`rule-tell-controls.md:110-113`) says ten of the twelve are full-shape and predicts a high fire rate on exactly those. Observed: 9/12, and the analyser prints `-> MET`. The doc (`:81`) then says the "does not survive" reading is "not supported" and that the prompt "survives". It gets there by reclassifying full-shape fires as definitional, a rule adopted after the results were seen (retraction item 2).
- (c) The detector verdict for RTD-10 is reversed after the fact.

**3. The L0 "ranking carries signal" claim, which the doc and handoff name as the lead for Stage 3, is not true of the logged rows.**
- (b) I rebuilt the ranks from `l0-gate.jsonl` (22 rows per text, in gate order; the fired sets match `l0-gate.txt`):
  - `cannot`: gold p = 0.600599, exactly equal to clean-1's value, so the margin is +0.00, not "above the clean maximum". `cannot_happen` returns the identical 0.600599 on clean-1, semicolon and cannot.
  - `sessionid`: gold p = 0.5 is 4-way tied behind `lines_read` at 0.58, so its rank is 2nd–5th, not "1st or 2nd".
  - `contradiction`: rank 13 (tied 13–15), not 12.
  - Only semicolon and member clearly satisfy both halves of the claim.
- The claim appears at `rule-tell-scoring-2026-09-23.md:561` and again as the Stage-3 lead at `:617`.
- (c) Per-rule calibration is being justified by 2 texts, not 4. The quantized, text-insensitive `cannot_happen` score suggests no threshold can separate that rule.

**4. The local-route pre-registration still points Stage 4 and its comparators at superseded instruments and numbers.**
- (b) `phase1-local-classifier-preregistration.md:24` still compares against "RTD-8 5/10 … arm 1b for RTD-8 (0/10)", which are contaminated-channel `rtd8` rows. Stage 4 Score B (`:111`) still scores with `rtd8`. That checker failed its clean-channel gate (`e2s-score-rtd8.txt`: `NO, NO, NO … GATE FAILED`) and was replaced by `rtd8c` (9/10 and 2/10). No amendment updates this.
- (c) As written, the planned Score B for the local arms cannot score RTD-8, and it compares against the wrong baseline.

**5. The "12 of 12 runs" figure behind the form-2b re-registration is false.**
- (b) The claim appears at scoring doc `:341` and pre-registration `:195`. It says `member_vs_population` fired on clean-2 on every run, for both models on both channels. The logs show:
  - `p1s-gate2.txt` (Haiku, dirty): 3/3.
  - `p1s-H0clean-gate.txt` (Haiku, clean): `[['member_vs_population','question_asked'],['question_asked'],['d_history']]`, so **1/3**.
  - `p1s-S0-gate.txt` (Sonnet): 3/3, but Sonnet only ever ran on the clean channel.
  - The actual count is 7/9, and there is no "Sonnet on both channels".
- (c) The premise the operator ruled on was overstated. The ruling itself may still hold on Sonnet's 3/3.

**6. "The claim-bound arms reproduce on the clean channel exactly" is false for RTD-9.**
- (b) Scoring doc `:424` claims exact reproduction, but its own table at `:412` shows `1b† 4/9 [3/9]`. The logs agree: `e2s-score-rtd9.txt` says 4/9 and `fsc-rtd9.txt` says 3/9.
- (c) The "published denominator" says the contamination changed no comparison. For RTD-9 it did.

**7. The "s0 is not below arm 0 — holds" statements are false by the registered comparator.**
- (b) The fork-route registration (pre-registration `:240`) says predictions are read "against fork arm 0 rather than the API route's 8/10".
  - Under `rtd8c`, s0 is 7/8 = 0.875 against fork arm 0 at 9/10 = 0.90. The doc (`:456`) instead falls back on "the registered 8/10", which is the wrong comparator.
  - For RTD-9 (`:192`, "for all three rules"), s0 is 5/8 = 0.625 against arm 0 at 7/10.
- (c) The differences are small, but both statements are false as written. The finding that stripping `CLAUDE.md` changes nothing is weaker than stated.

**8. The rtd9, rtd10 and rtd3 observable counts any `doc` call as the doc write.**
- (b) `scripts/phase2-score-dp1.py:76,94,108` use `"mcp__codescout__doc" in r["tools"]`. In `phase2-dp1-rtd910-n10.jsonl`, arm 9-1a run 4 is `grep` plus `doc {"action":"find"}`, 174 characters with no write. It counts as observable and was judged (presumably compliant).
- (c) RTD-9's verbatim-rule arm is 3/9 (clean) or 4/9, not 3/10 or 4/10. Verdicts are unchanged, but the observable is not what the registration says. It is the only such row: I tallied the `action` values across every scored file.

**9. The rtd8/rtd8c observable counts turns that never reached the decision.**
- (b) The observable is `"pika_observations" in r["text"]` (`:61,155`). In `fork-dp1-n10.jsonl`, arm 1b run 2 is a Romanian "first I'll verify that nothing reads it" plus a `run_command` grep, 517 characters, yet it counts as observable. Arm s1b runs 6 and 7 are text-only, and so are e2s runs 2 and 5 in `fork-e2s-dp1.jsonl`.
- The doc (`:188,448`) says arms under 10 exclude forks that "took a different first action", which is not true for RTD-8.
- (c) This is the same pre-decision scoring the Codex review narrowed for RTD-3. Here it goes unmentioned, and the fork-route "cuts to 2/10" includes a fork scored before the decision point (2/9 without it).

**10. For the scorer runs, the "clean channel" label cannot be checked from the committed evidence, and the scorer defaults to the dirty profile.**
- (b) `phase2-score-dp1.py:212-213` defaults `JUDGE_CONFIG_DIR` to `~/.claude-kat`, prints no config line and has no cleanliness guard. `api-score-*.txt` and `rtd8c-score.txt` carry no channel stamp. Only `score-b-s0.sh` shows the export. The selector logs, by contrast, do print their config.
- (c) The "restored in full" RTD-8 verdict rests on a channel claim that no committed artifact records.

**11. The author summary uses the wrong form's number for S0.**
- (b) The summary says "On 10 of the 17 it fires no rule at all". That is the form-3 figure, and form 3 was not adopted. S0 is form 2b, where 13 of 17 are silent (scoring doc `:510`). I recomputed both from `p1s-S0b-corpus.jsonl` and `p1s-S0f3-corpus.jsonl`.
- (c) The summary understates S0's silence.

**12. The family-recall "upper bound" is not an upper bound.**
- (b) Scoring doc `:528` says "no fairer grouping can do better on these rows". Counting every wrong-rule fire among the 17 positives (RTD-4, 9, 11 and 20 under form 3) gives 7/17.
- (c) The conclusion survives, since 7/17 = 0.41 is still below 0.5, but the stated bound is wrong.

**13. The README misdescribes a file, and some reported results have no file behind them.**
- (b) `score-rtd910.txt` contains only gate lines and `BadRequestError … usage limits` tracebacks: 0 replay lines, no table. The README calls it one of "the original scores". So the API-judge arm-0 rates cited at scoring doc `:161` (RTD-9 4/10, RTD-10 5/10) have nothing behind them in this folder.
- These results also have no file: the Jev `noul` gate table (`:95-103`), Jev's `none` on 10/10 drafts (`:198`), and the phase-1A gate results (`:262-263`).
- (c) The README's "each file backs the result the doc reports" is not true for these.

**14. Transcription slips (low impact).**
- Form-1 table (`:293-298`): clean-1 fired 4–5 rules, not 3–5; cannot +11–13, not +12; contradiction +11, not +10.
- S0's contradiction extras are +1–4, not +0–4 (`:329`).
- Form 3 "1 fire in 30 clean-text runs" (`:480`): 5 texts × 3 runs = 15.
- The Score A "Misses" list (`:386`) leaves out the yes-bucket misses RTD-15, 17, 18 and 20.
- `:135` cites one Jev call (0.40) although the doc itself says never to read one call. Run 1 on that case picked `none`.

**On the recommended next step:** the claim that "Stage 2 would label training data with the same excerpt convention" is not in the pre-registration. Stage 2 labels come from the correction diffs and from generated examples (`:48-53`), not from judging excerpts. The `partial` check may still be worth running, but it does not decide how Stage 2 is labelled.

## Checks that came out clean
- **Score A (2b and 3):** 924 rows each, 22 rules per text, 0 errors. Every bucket figure, 2/21 and 5/21 negative fires, 16/21 silent positives, the silence table (3/1/13 and 3/4/10), the 5/17 family figure, the Wilson interval [0.06, 0.41], and the rules that newly fire on negatives under form 3 all match.
- **Phase 1A:** Jev top-1 12/51, top-3 27/51, `none` 30/63, mean P(none) 0.27/0.24, 33/42 stable, 42/42 distributions differ. Haiku 12/85, 30/85, 1/105. The menu has 23 options.
- **Phase 0:** the analyser reproduces the positive-gate, diagonal and off-diagonal tables exactly. The 44 rubric-versus-native disagreements all run one way, and 22 YES verdicts score below 1.0.
- **Phase-2 API route:** `phase2-dp1-score.txt` and the three clean-channel `api-score-*.txt` tables, with every margin.
- **Fork route:** `fsc-*` tables; rtd3r 8/8/9/0/0; the exploratory 5/8/1/0; the old RTD-3 3/3 with 7 unobservable. The "sentence then a tool" counts (3, 6, 3) and the arithmetic bound (≥ 5/7, ≥ 2/4) hold, and no rtd3r row has a tool call without prose.
- **Score B:** e2s rates, margins, the RTD-10 5/10 sensitivity, the RTD-3 injections (6 of 10 empty; `monotone_absence` in runs 0 and 7), and the DP1 binding details for runs 1 and 3.
- **L0:** 4/10 gate, 3/3 span gate, `noul` range 0.09–0.86, `cannot_happen` 0.44–0.60, firing on 9 of 10 texts.
- **Other:** the stripped-arm file has 17 of 30 error rows; the form-2b and form-3 gate logs match their tables; the file count is 76 plus the README.

`★ Insight ─────────────────────────────────────`
- **Where these defects sit:** almost none are arithmetic errors. They are in how things were labelled or described: an observable that admits the wrong rows (#8, #9), a comparator swapped for a friendlier one (#7), and a stop rule read as a per-prompt caveat (#1). Only recomputing from the rows catches these, because the summaries are internally consistent.
- **Tied scores:** JevK5's `noul` returns a handful of repeated values (0.600599, 0.5, 0.520387…). Ranking by those scores is fragile, and ranks read off them are ties, not orderings (#3).
- **A substring test for "observable" does not check what kind of turn it was:** `"doc" in tools` and `"pika_observations" in text` both admit pre-decision turns. That is the same recording-filter problem this campaign already found once for RTD-3.
`─────────────────────────────────────────────────`